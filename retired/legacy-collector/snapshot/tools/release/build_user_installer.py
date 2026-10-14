"""Prepare user delivery alongside the existing protected collector publication.

The resulting installer is a separately provisioned trust root, like install.sh.
It embeds exact bootstrap, verifier and release pins, never learns them from an
untrusted plugin. Existing signed release bytes are retained unchanged.
"""
import argparse
import base64
import gzip
import json
from pathlib import Path
import shutil

import build_collector_bootstrap as bootstrap
import collector_assets as assets
import collector_components as transport
import collector_light_install as light
import install_collector as lifecycle


TEMPLATE = r'''#!/usr/bin/env bash
set -Eeuo pipefail
umask 077
if [[ "${1:-}" == --help || "${1:-}" == -h ]]; then
    echo 'Install the optional Harness-Gate Rust plugin (Linux x86_64).'
    echo 'Usage: install-rust.sh [--plan] [--download-policy allow|deny] [--mode auto|pinned]'
    echo '       [--root DIR] [--cache-dir DIR] [--offline DIR] [--reuse-runtime DIR] [--cache-limit-bytes N]'
    echo 'Maintenance: --action usage|cleanup|migrate|rollback|export [--version V] [--keep N] [--dry-run] [--export-output DIR]'
    echo 'Default download policy: deny. Auto mode reuses exact compatible bytes.'
    echo 'An uncached --plan reports the bootstrap and complete upper bound without downloading.'
    exit 0
fi
[[ "$(uname -s)/$(uname -m)" == Linux/x86_64 ]] || { echo 'This Rust plugin supports Linux x86_64 only.' >&2; exit 1; }
offline='' policy=deny plan=false
cache="${HOME}/.cache/harness-gate/collector"
args=("$@")
for ((i=0;i<${#args[@]};i++)); do
    case "${args[i]}" in
        --offline) offline="${args[i+1]:?--offline requires a directory}" ;;
        --cache-dir) cache="${args[i+1]:?--cache-dir requires a directory}" ;;
        --download-policy) policy="${args[i+1]:?--download-policy requires allow or deny}" ;;
        --plan) plan=true ;;
    esac
done
[[ "$policy" == allow || "$policy" == deny ]] || { echo 'Invalid download policy' >&2; exit 1; }
[[ "$cache" == /* ]] || { echo 'Cache must be absolute' >&2; exit 1; }
# Do not traverse caller-controlled links when bootstrapping the private interpreter.
parent="$cache"
while [[ "$parent" != / ]]; do
    [[ ! -L "$parent" ]] || { echo 'Unsafe cache symlink' >&2; exit 1; }
    parent=$(dirname -- "$parent")
done
mkdir -p -- "$cache"
[[ "$(stat -c %u "$cache")" == "$(id -u)" && "$(stat -c %a "$cache")" == 700 ]] || { echo 'Cache must be private (0700)' >&2; exit 1; }
[[ ! -L "$cache/lock" ]] || exit 1
exec 9>>"$cache/lock"
[[ "$(stat -c %h "$cache/lock")" == 1 ]] || exit 1
flock -x 9
capsule="$cache/@SHA@-@BOOTSTRAP@"
valid=false
if [[ -f "$capsule" && ! -L "$capsule" && "$(stat -c %h "$capsule")" == 1 ]]; then
    if echo '@SHA@  '"$capsule" | sha256sum --check --status; then valid=true; fi
fi
cat <<'HARNESS_GATE_BOOTSTRAP_PLAN'
@BOOTSTRAP_PLAN@
HARNESS_GATE_BOOTSTRAP_PLAN
printf 'Bootstrap cache: %s; verified cache hit: %s\n' "$cache" "$valid"
if [[ "$valid" == false && "$plan" == true ]]; then
    echo 'Exact host reuse is resolved by the pinned interpreter after bootstrap provisioning.'
    exit 0
fi
if [[ "$valid" == false && -z "$offline" && "$policy" != allow ]]; then
    echo 'Network disabled by policy. Use --download-policy allow or --offline DIR.' >&2
    exit 1
fi
unset PYTHONHOME PYTHONPATH PYTHONSTARTUP LD_PRELOAD LD_LIBRARY_PATH
stage=$(mktemp -d "$cache/bootstrap-XXXXXXXX")
trap 'rm -rf -- "$stage"' EXIT
if [[ "$valid" == false ]]; then
    if [[ -n "$offline" ]]; then
        cp -- "$offline/@BOOTSTRAP@" "$stage/bootstrap.tar.gz"
    else
        curl --fail --show-error --location --proto '=https' --proto-redir '=https' --http1.1 \
            --retry 5 --retry-all-errors --connect-timeout 15 \
            '@BASE@/@BOOTSTRAP@' -o "$stage/bootstrap.tar.gz"
    fi
    echo '@SHA@  '"$stage/bootstrap.tar.gz" | sha256sum --check --status || { echo 'Installer checksum mismatch' >&2; exit 1; }
    mv -- "$stage/bootstrap.tar.gz" "$capsule"
fi
mkdir "$stage/runtime"
tar -xzf "$capsule" -C "$stage/runtime" --no-same-owner --no-same-permissions
cat > "$stage/catalog.json" <<'HARNESS_GATE_CATALOG'
@CATALOG@
HARNESS_GATE_CATALOG
# Python acquires the same cache lock itself. A concurrent cleanup may evict the
# capsule now; this invocation uses its already verified private extracted copy.
flock -u 9
exec 9>&-
export LD_LIBRARY_PATH="$stage/runtime/lib"
"$stage/runtime/python/bin/python3" -I -S -B "$stage/runtime/tools/release/friendly_collector_install.py" \
    --catalog "$stage/catalog.json" "$@"
'''


def object_locations(descriptor, base_url):
    """Keep each immutable GitHub release below its 1,000-asset limit."""
    return {digest: base_url + '-objects-' + str(index // 900 + 1)
            for index, digest in enumerate(sorted(descriptor['objects']))}


def build(release, runtime, trust, output, base_url, prepared_transport=None):
    assets.require(base_url.startswith('https://') and "'" not in base_url and '\n' not in base_url, 'invalid installer release URL')
    manifest = assets.read(release / 'manifest.json')
    version = manifest['collector']['version']
    assets.verify(release, trust, 'rust-collector-v' + version)
    # Authenticate every executable/runtime byte before the release-side smoke
    # test. Publisher acceptance and user activation exercise the same tools.
    lifecycle.tree_check(runtime, lifecycle.payload_names(manifest), complete=True)
    for row in manifest['payloads']:
        path = runtime / row['path']
        assets.regular(path)
        assets.require(assets.sha(path) == row['sha256'], 'release runtime differs: ' + row['path'])
    light.self_check(runtime, manifest)
    if prepared_transport:
        output.mkdir(parents=True, exist_ok=False)
        for path in prepared_transport.iterdir():
            assets.regular(path); shutil.copyfile(path, output / path.name)
        descriptor = assets.read(output / 'transport.json')
        assets.require(descriptor['archive_sha256'] == assets.sha(release / 'collector.tar'), 'wrong prepared transport')
    else:
        descriptor = transport.pack(release / 'collector.tar', output)
    transport.validate(descriptor)
    capsule = output / 'installer-bootstrap.tar'
    receipt = bootstrap.build(runtime, capsule, user_entry=True)
    compressed = output / 'installer-bootstrap.tar.gz'
    with capsule.open('rb') as src, compressed.open('xb') as dst:
        with gzip.GzipFile(filename='', fileobj=dst, mode='wb', mtime=0) as gz:
            shutil.copyfileobj(src, gz)
    capsule.unlink()
    def row(path):
        return {'name': path.name, 'sha256': assets.sha(path), 'size': path.stat().st_size}
    controls = []
    for name in assets.ASSETS + assets.CONTROL:
        if name == 'collector.tar': continue
        shutil.copyfile(release / name, output / name)
        controls.append(row(output / name))
    host = dict(trust)
    # Host paths are reviewed inputs; release payloads cannot choose them.
    profile = {'host': host, **{key: base64.b64encode(Path(trust[key]).read_bytes()).decode()
                              for key in ('public_key', 'trusted_root')}}
    profile['verifier'] = row(Path(trust['cosign'])) | {
        'name': 'cosign-linux-amd64',
        'url': 'https://github.com/sigstore/cosign/releases/download/v3.1.3/cosign-linux-amd64'}
    catalog = {'schema': 'rust-collector-user-install/v2', 'version': version,
               'base_url': base_url, 'release_url': base_url,
               'object_base_urls': object_locations(descriptor, base_url),
               'archive_sha256': descriptor['archive_sha256'], 'host_abi': manifest['host_abi'],
               'transport': row(output / 'transport.json'), 'trust': profile, 'controls': controls,
               'components': descriptor, 'component_versions': transport.component_versions(descriptor),
               'compatibility': {key: manifest[key] for key in
                   ('collector', 'protocol', 'core_compatibility', 'host_abi', 'tools', 'measurement', 'capabilities')}}
    maximum = sum(b['size'] for b in descriptor['objects'].values()) + compressed.stat().st_size + sum(r['size'] for r in controls) + profile['verifier']['size']
    bootstrap_plan = {'phase': 'bootstrap', 'bootstrap': row(compressed),
        'reason': 'pinned private installer interpreter; reused from digest cache on subsequent runs',
        'maximum_total_download_bytes_before_reuse': maximum,
        'components': [{'path': r['path'], 'object': r['sha256'], 'expanded_bytes': r['size'], 'compressed_bytes': descriptor['objects'][r['sha256']]['size']} for r in descriptor['payloads']],
        'metadata': controls, 'verifier': profile['verifier']}
    script = TEMPLATE.replace('@BOOTSTRAP_PLAN@', json.dumps(bootstrap_plan, sort_keys=True)).replace('@CATALOG@', json.dumps(catalog, sort_keys=True))
    script = script.replace('@BASE@', base_url).replace('@BOOTSTRAP@', compressed.name).replace('@SHA@', assets.sha(compressed))
    (output / 'install-rust.sh').write_text(script)
    (output / 'install-rust.sh').chmod(0o755)
    assets.write(output / 'installer-build.json', {'schema': 'rust-collector-user-installer-build/v1',
        'native_self_check': 'compile, execute and coverage export passed',
        'original_archive_sha256': descriptor['archive_sha256'], 'bootstrap': receipt,
        'download_bytes': maximum,
        'installer': row(output / 'install-rust.sh'), 'catalog': catalog})
    return catalog


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('release', 'runtime', 'trust', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--base-url', required=True)
    parser.add_argument('--prepared-transport', type=Path)
    args = parser.parse_args()
    build(args.release, args.runtime, assets.read(args.trust), args.output, args.base_url, args.prepared_transport)
