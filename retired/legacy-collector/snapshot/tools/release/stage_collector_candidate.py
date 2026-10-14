"""Stage actual unsigned RC bytes from a pinned runtime and fresh capture binding.

The binding is diagnostic input. This command does not approve its measurement
series, runtime capture trust, compatibility, licensing or release eligibility.
"""
import argparse
from pathlib import Path
import shutil
import sys

import collector_assets as assets
import prepare_collector_candidate as candidate

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'quality'))
import rust_collector_delivery as delivery
import rust_native_driver as native


def stage(runtime, core, core_id, source, binding, output):
    assets.require(len(source) == 40 and all(c in '0123456789abcdef' for c in source), 'full source SHA required')
    assets.require(binding['input']['collector']['version'] == '0.1.0-rc.1', 'fresh RC binding required')
    assets.require(not output.exists(), 'candidate directory must be fresh')
    observed, paths = delivery.observe(runtime, core, core_id)
    inventory = assets.read(runtime / 'runtime.json')
    for name, row in inventory['payload'].items():
        assets.regular(runtime / name)
        assets.require(assets.sha(runtime / name) == row['sha256'], 'runtime inventory changed')
    manifest = {'schema': 'rust-collector-delivery/v2', 'collector': binding['input']['collector'],
        'source_commit': source, 'protocol': delivery.PROTOCOL,
        'host_abi': observed['host_abi'], 'core_compatibility': [core_id],
        'tools': [dict(t, path=str(paths[t['name']].relative_to(runtime))) for t in observed['tools']],
        'payloads': [{'path': str(p.relative_to(runtime)), 'sha256': assets.sha(p), 'role': 'runtime'}
                     for p in sorted(runtime.rglob('*')) if p.is_file()],
        'capabilities': [dict(c, scope='single-file-fixture') for c in binding['capabilities']],
        'measurement': {'native_series': native.SERIES,
            'compiler_commit': native.RUSTC_COMMIT, 'llvm_version': '22.1.6',
            'compiler_inventory_schema': native.SCHEMA,
            'adapter_sha256': assets.sha(runtime / 'app/rust_native_driver.py'),
            'projection_sha256': assets.sha(runtime / 'app/rust_collector_project.py'),
            'classifier_sha256': assets.sha(runtime / 'app/rust_native_classify.py'),
            'normalization': 'exact owner counts and rational CRAP; rust_collector_project',
            'source_boundary': 'single-file-fixture production owners',
            'configuration_authority': 'quality-trusted-state/v1'},
        'dependency_inventory_sha256': assets.sha(runtime / 'runtime.json'),
        'license_inventory_sha256': assets.contract.fingerprint({k: v for k, v in inventory['payload'].items()
                                                               if k.startswith('licenses/')})}
    assets.contract.validate_manifest(manifest)
    output.mkdir(parents=True)
    shutil.copyfile(str(runtime) + '.tar', output / 'collector.tar')
    assets.write(output / 'manifest.json', manifest)
    return candidate.prepare(output), observed


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('runtime', 'core', 'core-identity', 'binding', 'output', 'receipt', 'observed'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--source', required=True)
    args = parser.parse_args()
    receipt, observed = stage(args.runtime.resolve(), args.core.resolve(), assets.read(args.core_identity),
                             args.source, assets.read(args.binding), args.output)
    assets.write(args.receipt, receipt)
    assets.write(args.observed, observed)
