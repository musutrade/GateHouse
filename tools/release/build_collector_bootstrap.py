"""Build a separately authenticated installer capsule from pinned runtime inputs.

No release asset, eligibility assertion or downloaded executable supplies trust.
The resulting digest must be distributed through an administrator trust channel.
"""
import argparse
import io
from pathlib import Path
import tarfile

import collector_assets as assets

RELEASE = Path(__file__).resolve().parent


def build(runtime, output, *, user_entry=False):
    inventory = assets.read(runtime / 'runtime.json')['payload']
    files = {}
    for name, row in inventory.items():
        if name.startswith(('python/', 'lib/', 'licenses/', 'app/')):
            source = runtime / name
            assets.regular(source)
            assets.require(assets.sha(source) == row['sha256'], 'bootstrap runtime pin changed')
            destination = 'tools/quality/' + name[4:] if name.startswith('app/') else name
            files[destination] = (source.read_bytes(), row['mode'])
    for name in ('collector_assets.py', 'collector_sigstore.py', 'collector_release_policy.py',
                 'release_policy.py', 'install_collector.py', 'collector_receipt.py',
                 'collector_store.py', 'collector_maintenance.py', 'production_installer.py'):
        files['tools/release/' + name] = ((RELEASE / name).read_bytes(), 0o644)
    if user_entry:
        for name in ('collector_transport.py', 'collector_components.py', 'collector_light_install.py',
                     'friendly_collector_install.py'):
            files['tools/release/' + name] = ((RELEASE / name).read_bytes(), 0o644)
    assets.require('python/bin/python3' in files, 'missing private bootstrap Python')
    # Inventory is descriptive; the out-of-band capsule SHA authenticates it.
    import hashlib
    import json
    manifest = {name: {'sha256': hashlib.sha256(raw).hexdigest(), 'mode': mode}
                for name, (raw, mode) in sorted(files.items())}
    files['bootstrap-inputs.json'] = ((json.dumps(manifest, sort_keys=True, indent=2) + '\n').encode(), 0o644)
    with output.open('xb') as destination, tarfile.open(fileobj=destination, mode='w') as archive:
        for name, (raw, mode) in sorted(files.items()):
            assets.safe_name(name)
            info = tarfile.TarInfo(name)
            info.size, info.mode = len(raw), mode
            archive.addfile(info, io.BytesIO(raw))
    return {'schema': 'rust-collector-bootstrap-build/v1', 'sha256': assets.sha(output),
            'size': output.stat().st_size, 'files': manifest,
            'trust_distribution': 'operator provisioning required; no self-authentication'}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--receipt', type=Path, required=True)
    args = parser.parse_args()
    assets.write(args.receipt, build(args.runtime, args.output))
