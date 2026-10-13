"""Sign a private RC for installation acceptance; never publish a GitHub release.

Final publication still requires production_collector_release.preflight and its
complete license, compatibility and six-check validation receipts.
"""
import argparse
import os
from pathlib import Path
import shutil
import subprocess

import collector_assets as assets
import collector_release_policy as policy
import prepare_collector_candidate as candidate
import production_collector_release as production


def preflight(packet_path, digest, source, client):
    packet = assets.read(production.pinned(packet_path, digest))
    assets.require(set(packet) == {'schema', 'source_commit', 'version', 'directory',
        'output', 'trust', 'unsigned_assets'}, 'wrong signing packet fields')
    assets.require(packet['schema'] == 'rust-collector-private-signing/v1', 'wrong signing packet schema')
    assets.require(packet['source_commit'] == source, 'wrong signing source')
    version = assets.version(packet['version'])
    assets.require('-rc.' in version, 'private signing requires an RC version')
    for ref in ('HEAD', 'refs/remotes/origin/main'):
        assets.require(policy.core._resolve_commit(Path.cwd(), ref, ref) == source,
                       'signing requires exact checked-out protected main')
    assets.require(client.repository == assets.REPOSITORY, 'wrong repository')
    policy.core.verify_ci_run(client, source, 'main')
    policy.protected_environment(client.get_json('/repos/' + assets.REPOSITORY +
                                               '/environments/' + policy.ENVIRONMENT))
    directory, output = Path(packet['directory']), Path(packet['output'])
    assets.require(directory.is_absolute() and output.is_absolute(), 'absolute candidate paths required')
    assets.require(not output.exists(), 'signed candidate output must be fresh')
    assets.require(set(p.name for p in directory.iterdir()) == set(assets.ASSETS[:3]),
                   'unsigned candidate must have exactly three assets')
    assets.require(set(packet['unsigned_assets']) == set(assets.ASSETS[:3]), 'wrong unsigned inventory')
    production.approved_unsigned(packet, directory)
    manifest = assets.contract.load_manifest((directory / 'manifest.json').read_bytes())
    assets.require(manifest['source_commit'] == source and manifest['collector']['version'] == version,
                   'wrong candidate identity')
    candidate.verify_payloads(directory, manifest)
    assets.require(assets.read(directory / 'sbom.spdx.json') == assets.sbom(manifest), 'wrong candidate SBOM')
    trust_path = Path(packet['trust']['path'])
    assets.require(trust_path.is_absolute(), 'absolute host trust path required')
    trust = assets.read(production.pinned(trust_path, packet['trust']['sha256']))
    assets.require(trust['schema'] == 'rust-collector-host-trust/v2', 'production trust v2 required')
    for name in ('openssl', 'public_key', 'cosign', 'trusted_root'):
        path = Path(trust[name])
        assets.require(path.is_absolute() and not path.resolve().is_relative_to(directory.resolve()),
                       'independent absolute host verifier required')
        production.pinned(path, trust[name + '_sha256'])
    assets.require(assets.probe_host(trust) == manifest['host_abi'], 'unsupported signing host ABI')
    return packet, directory, output, trust


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('stage', choices=('preflight', 'sign'))
    parser.add_argument('--packet', type=Path, required=True)
    parser.add_argument('--packet-sha256', required=True)
    parser.add_argument('--source', required=True)
    parser.add_argument('--private-key', type=Path)
    args = parser.parse_args()
    client = policy.core.GitHubClient('https://api.github.com', assets.REPOSITORY, os.environ['GH_TOKEN'])
    packet, directory, output, trust = preflight(args.packet, args.packet_sha256, args.source, client)
    if args.stage == 'preflight':
        print('Private RC signing inputs verified; publication acceptance not evaluated.')
        return
    assets.require(args.private_key is not None, 'protected RSA key required')
    tag = 'rust-collector-v' + packet['version']
    # GitHub refuses an existing ref. Never replace a tag, even on a retry.
    subprocess.run(['gh', 'api', '--method', 'POST', 'repos/' + assets.REPOSITORY + '/git/refs',
                    '-f', 'ref=refs/tags/' + tag, '-f', 'sha=' + args.source], check=True,
                   stdout=subprocess.DEVNULL)
    subprocess.run(['git', 'fetch', '--no-tags', 'origin',
                    'refs/tags/' + tag + ':refs/tags/' + tag], check=True)
    eligibility = policy.verify(Path.cwd(), directory / 'manifest.json', tag, args.source, client)
    output.mkdir(parents=True)
    for name in assets.ASSETS[:3]:
        shutil.copyfile(directory / name, output / name)
    production.approved_unsigned(packet, output)
    assets.prepare(output, eligibility)
    production.sign(output, trust, args.private_key)
    assets.verify(output, trust, tag)
    assets.write(output.parent / (output.name + '-signing-receipt.json'), {
        'schema': 'rust-collector-private-signing-receipt/v1', 'source_commit': args.source,
        'tag': tag, 'packet_sha256': args.packet_sha256,
        'signature_verified': True, 'published': False, 'installation_acceptance': 'pending',
        'assets': assets.subjects(output, assets.ASSETS + assets.CONTROL)})
    print('Private RC signatures verified. Installation acceptance and publication remain pending.')


if __name__ == '__main__':
    main()
