"""Protected workflow helpers. Approval inputs are independently reviewed files.

Preparation does not generate these approval inputs or an eligibility pass.
The workflow template must be reviewed and installed separately before use.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

import collector_assets as assets
import collector_release_policy as policy
import collector_sigstore as sigstore
import prepare_collector_candidate as candidate

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'quality'))
import build_rust_collector as builder


def pinned(path, digest):
    assets.regular(path)
    assets.require(assets.sha(path) == digest, 'approved input digest changed: ' + str(path))
    return path


def pinned_receipt(row):
    path = Path(row['path'])
    assets.require(path.is_absolute(), 'receipt paths must be absolute')
    return pinned(path, row['sha256'])


def reviewed_checks(manifest, manifest_sha, licenses, validation):
    assets.require(licenses['status'] == 'approved' and licenses['reviewer'] and
                   licenses['manifest_sha256'] == manifest_sha, 'license review required')
    expected = {p['path']: p['sha256'] for p in manifest['payloads']}
    rows = licenses['payloads']
    assets.require(len(rows) == len(expected) and
                   {r['path']: r['sha256'] for r in rows} == expected and
                   all(r['redistribution'] == 'approved' and r['license'] not in
                       ('', 'NOASSERTION', 'NONE') for r in rows), 'incomplete dependency/license closure')
    assets.require(validation['source_commit'] == manifest['source_commit'] and
                   validation['manifest_sha256'] == manifest_sha, 'stale validation')
    for check in ('fresh_host_install', 'native_capture_reexport', 'generic_core', 'lifecycle_negatives',
                  'bootstrap_negatives', 'real_sigstore_positive_and_negatives'):
        row = validation['checks'][check]
        assets.require(row['status'] == 'pass' and row['receipt_url'].startswith('https://'),
                       'missing successful validation: ' + check)
        pinned_receipt(row)


def preflight(packet_path, digest, source, client):
    packet = assets.read(pinned(packet_path, digest))
    assets.require(packet['schema'] == 'rust-collector-publication-approval/v1', 'wrong approval schema')
    assets.require(packet['source_commit'] == source, 'wrong approved source/version')
    assets.require('-rc.' in assets.version(packet['version']), 'approved RC version required')
    for ref in ('HEAD', 'refs/remotes/origin/main'):
        assets.require(policy.core._resolve_commit(Path.cwd(), ref, ref) == source,
                       'publication requires exact checked-out protected main')
    assets.require(client.repository == assets.REPOSITORY, 'wrong repository')
    policy.core.verify_ci_run(client, source, 'main')
    policy.protected_environment(client.get_json(
        '/repos/' + assets.REPOSITORY + '/environments/' + policy.ENVIRONMENT))
    inputs = {}
    for name in ('manifest', 'build_lock', 'bootstrap', 'bootstrap_launcher', 'trust', 'compatibility',
                 'observed_environment', 'license_review', 'validation'):
        row = packet['inputs'][name]
        path = Path(row['path'])
        assets.require(path.is_absolute(), 'approval input paths must be absolute')
        inputs[name] = pinned(path, row['sha256'])
    manifest = assets.contract.load_manifest(inputs['manifest'].read_bytes())
    assets.require(manifest['source_commit'] == source and
                   manifest['collector']['version'] == packet['version'], 'wrong candidate identity')
    trust = assets.read(inputs['trust'])
    assets.require(trust['schema'] == 'rust-collector-host-trust/v2', 'production trust v2 required')
    for name in ('openssl', 'public_key', 'cosign', 'trusted_root'):
        assets.require(Path(trust[name]).is_absolute(), 'trust paths must be absolute')
        pinned(Path(trust[name]), trust[name + '_sha256'])
    assets.require(assets.probe_host(trust) == manifest['host_abi'], 'unsupported signing host ABI')
    # Contract preflight proves the exact approved matrix row, not broad Linux support.
    receipt = assets.contract.preflight(manifest, assets.read(inputs['compatibility']),
                                        assets.read(inputs['observed_environment']))
    # Matrix metadata is not verification of its evidence bytes.
    pinned_receipt(receipt)
    licenses = assets.read(inputs['license_review'])
    validation = assets.read(inputs['validation'])
    reviewed_checks(manifest, assets.sha(inputs['manifest']), licenses, validation)
    assets.require(packet['retention']['immutable_url'] and packet['retention']['owner'] and
                   packet['retention']['policy'], 'durable retention approval required')
    assets.require(set(packet['unsigned_assets']) == set(assets.ASSETS[:3]), 'wrong unsigned inventory')
    return packet, inputs, trust


def approved_unsigned(packet, directory):
    for name in assets.ASSETS[:3]:
        row = packet['unsigned_assets'][name]
        pinned(directory / name, row['sha256'])
        assets.require((directory / name).stat().st_size == row['size'], 'approved asset size changed')


def exact_download(directory, expected):
    for name in assets.ASSETS + assets.CONTROL:
        assets.regular(expected / name)
        pinned(directory / name, assets.sha(expected / name))
        assets.require((directory / name).stat().st_size == (expected / name).stat().st_size,
                       'downloaded asset size changed')


def assemble(packet, inputs, output):
    assets.require(not output.exists(), 'production output must be fresh')
    runtime = output.parent / (output.name + '-runtime')
    builder.build(assets.read(inputs['build_lock']), runtime)
    output.mkdir()
    shutil.copyfile(str(runtime) + '.tar', output / 'collector.tar')
    shutil.copyfile(inputs['manifest'], output / 'manifest.json')
    manifest = assets.contract.load_manifest((output / 'manifest.json').read_bytes())
    candidate.verify_payloads(output, manifest)
    assets.write(output / 'sbom.spdx.json', assets.sbom(manifest))
    approved_unsigned(packet, output)


def sign(directory, trust, private_key):
    # Called only in the protected production job, after real tag eligibility.
    raw = directory.parent / 'inventory.rsa'
    bundle = directory.parent / 'inventory.sigstore.json'
    subprocess.run([trust['openssl'], 'dgst', '-sha256', '-sign', str(private_key),
                    '-out', str(raw), str(directory / 'release-inventory.json')], check=True)
    subprocess.run([trust['cosign'], 'sign-blob', '--yes', '--bundle', str(bundle),
                    str(directory / 'release-inventory.json')], check=True)
    assets.write(directory / 'release-inventory.sig', sigstore.envelope(raw.read_bytes(), assets.read(bundle)))
    sigstore.verify(directory, trust)


def create_or_verify_tag(tag, source):
    """Reuse only the exact immutable commit tag created for private acceptance."""
    rows = json.loads(subprocess.check_output(['gh', 'api', 'repos/' + assets.REPOSITORY +
                                              '/git/matching-refs/tags/' + tag]))
    matches = [row for row in rows if row['ref'] == 'refs/tags/' + tag]
    if matches:
        assets.require(len(matches) == 1 and matches[0]['object']['type'] == 'commit' and
                       matches[0]['object']['sha'] == source, 'existing collector tag differs')
    else:
        subprocess.run(['gh', 'api', '--method', 'POST', 'repos/' + assets.REPOSITORY + '/git/refs',
                        '-f', 'ref=refs/tags/' + tag, '-f', 'sha=' + source],
                       check=True, stdout=subprocess.DEVNULL)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('stage', choices=('assemble', 'sign', 'verify'))
    parser.add_argument('--packet', type=Path, required=True)
    parser.add_argument('--packet-sha256', required=True)
    parser.add_argument('--source', required=True)
    parser.add_argument('--directory', type=Path, required=True)
    parser.add_argument('--private-key', type=Path)
    parser.add_argument('--receipt', type=Path)
    parser.add_argument('--expected-directory', type=Path)
    args = parser.parse_args()
    client = policy.core.GitHubClient('https://api.github.com', assets.REPOSITORY, os.environ['GH_TOKEN'])
    packet, inputs, trust = preflight(args.packet, args.packet_sha256, args.source, client)
    tag = 'rust-collector-v' + packet['version']
    if args.stage == 'assemble':
        assemble(packet, inputs, args.directory)
    elif args.stage == 'sign':
        approved_unsigned(packet, args.directory)
        eligibility = policy.verify(Path.cwd(), inputs['manifest'], tag, args.source, client)
        assets.prepare(args.directory, eligibility)
        assets.require(args.private_key is not None, 'protected RSA private key required')
        sign(args.directory, trust, args.private_key)
        assets.verify(args.directory, trust, tag)
    else:
        assets.require(args.expected_directory is not None and
                       args.expected_directory.resolve() != args.directory.resolve(),
                       'independent download and original six-asset directory required')
        assets.require(args.receipt is not None and not args.receipt.exists() and
                       not args.receipt.resolve().is_relative_to(args.directory.resolve()),
                       'fresh verification receipt outside the six-asset directory required')
        approved_unsigned(packet, args.directory)
        exact_download(args.directory, args.expected_directory)
        assets.verify(args.directory, trust, tag)
        assets.write(args.receipt, {'schema': 'rust-collector-download-verification/v1',
            'status': 'pass', 'source_commit': args.source, 'tag': tag,
            'approval_sha256': args.packet_sha256,
            'assets': [dict(row, size=(args.directory / row['name']).stat().st_size)
                       for row in assets.subjects(args.directory, assets.ASSETS + assets.CONTROL)]})


if __name__ == '__main__':
    main()
