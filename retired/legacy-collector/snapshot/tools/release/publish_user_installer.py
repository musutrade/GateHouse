"""Publish a new installer version from exact reviewed inputs in the existing workflow."""
import argparse
import json
import os
from pathlib import Path
import subprocess

import build_user_installer as builder
import collector_assets as assets
import collector_release_policy as policy
import production_collector_release as production


def publish_assets(output, catalog, tag, source, body):
    """Verify every draft shard before making the installer discoverable."""
    groups = {tag: {}}
    locations = catalog['object_base_urls']
    object_tags = {row['name']: locations[digest].rsplit('/', 1)[1]
                   for digest, row in catalog['components']['objects'].items()}
    for path in output.iterdir():
        assets.regular(path)
        groups.setdefault(object_tags.get(path.name, tag), {})[path.name] = assets.sha(path)
    for release_tag, expected in groups.items():
        assets.require(len(expected) <= 900, 'installer release exceeds asset budget')
        production.create_or_verify_tag(release_tag, source)
        subprocess.run(['gh', 'release', 'create', release_tag, '--repo', assets.REPOSITORY,
                        '--verify-tag', '--draft', '--prerelease', '--title', release_tag,
                        '--notes-file', str(body),
                        *[str(output / name) for name in sorted(expected)]], check=True)
        draft = json.loads(subprocess.check_output(
            ['gh', 'api', 'repos/' + assets.REPOSITORY + '/releases/tags/' + release_tag]))
        pages = json.loads(subprocess.check_output(
            ['gh', 'api', '--paginate', '--slurp', 'repos/' + assets.REPOSITORY +
             '/releases/' + str(draft['id']) + '/assets?per_page=100']))
        uploaded = [row for page in pages for row in page]
        assets.require(draft['draft'] and len(uploaded) == len(expected) and
                       {a['name']: a['digest'] for a in uploaded} ==
                       {n: 'sha256:' + d for n, d in expected.items()}, 'uploaded installer assets differ')
    # Partially published component shards cannot expose a usable installer.
    for release_tag in [t for t in groups if t != tag] + [tag]:
        subprocess.run(['gh', 'release', 'edit', release_tag, '--repo', assets.REPOSITORY,
                        '--draft=false', '--latest=false'], check=True)
    return groups


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--packet', type=Path, required=True)
    parser.add_argument('--sha256', required=True)
    parser.add_argument('--source', required=True)
    args = parser.parse_args()
    packet = assets.read(production.pinned(args.packet, args.sha256))
    assets.require(packet['schema'] == 'rust-collector-installer-approval/v1', 'wrong installer approval')
    assets.require(packet['source'] == args.source, 'wrong installer source')
    client = policy.core.GitHubClient('https://api.github.com', assets.REPOSITORY, os.environ['GH_TOKEN'])
    for ref in ('HEAD', 'refs/remotes/origin/main'):
        assets.require(policy.core._resolve_commit(Path.cwd(), ref, ref) == args.source, 'installer must use protected main')
    policy.core.verify_ci_run(client, args.source, 'main')
    policy.protected_environment(client.get_json('/repos/' + assets.REPOSITORY + '/environments/' + policy.ENVIRONMENT))
    trust = assets.read(production.pinned(Path(packet['trust']['path']), packet['trust']['sha256']))
    assets.require(trust['schema'] == 'rust-collector-host-trust/v2', 'dual-signature production trust required')
    for name in ('openssl','cosign','public_key','trusted_root'):
        production.pinned(Path(trust[name]),trust[name+'_sha256'])
    release = Path(packet['release'])
    for name, digest in packet['signed_assets'].items(): production.pinned(release/name,digest)
    tag = 'rust-collector-installer-v' + assets.version(packet['version'])
    output = Path('target/collector-user-installer')
    catalog = builder.build(release, Path(packet['runtime']), trust, output,
                            'https://github.com/' + assets.REPOSITORY + '/releases/download/' + tag)
    production.pinned(output/'install-rust.sh',packet['installer_sha256'])
    production.create_or_verify_tag(tag,args.source)
    subprocess.run([trust['cosign'],'sign-blob','--yes','--bundle',str(output/'install-rust.sh.sigstore.json'),
                    str(output/'install-rust.sh')],check=True)
    # The publication source and operator-reviewed script pin authenticate the
    # bootstrap and transport. Underlying collector signatures remain unchanged.
    notes = ('Rust plugin installer. Run bash install-rust.sh --plan, then use --download-policy allow or --offline DIR. '
             'Exact compatible components are reused; only missing content objects are acquired. '
             'Installation automatically verifies signatures and compiles a diagnostic before activation. '
             'For an offline kit also provision the pinned cosign verifier listed in installer-build.json.\n\n'
             'Installer version '+packet['version']+' delivers the unchanged signed Rust plugin '+catalog['version']+'. '
             'Supported host: '+json.dumps(catalog['host_abi'],sort_keys=True)+'.\n\n'
             'Corresponding source, full notices and relink materials: '
             'https://github.com/'+assets.REPOSITORY+'/releases/tag/rust-collector-materials-7165558-v1 .\n\n'
             'This product includes software developed by the NetBSD Foundation, Inc. and its contributors.')
    body = output.parent/'installer-release-notes.md';body.write_text(notes)
    expected = publish_assets(output, catalog, tag, args.source, body)
    assets.write(output.parent/'installer-publication.json',{'status':'pass','source':args.source,'tag':tag,
                 'installer_sha256':packet['installer_sha256'],'assets':expected})


if __name__=='__main__': main()
