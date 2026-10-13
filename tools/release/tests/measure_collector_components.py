"""Reproducible native-byte storage diagnostic; NEVER production release approval.

Requires a freshly built private runtime, actual Core and fresh native binding.
Uses a disposable RSA key and test-only eligibility, with zero network calls.
"""
import argparse
import copy
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import collector_assets as assets
import collector_components as components
import collector_light_install as light
import collector_maintenance as maintenance
import collector_store as store
import install_collector as lifecycle
import stage_collector_candidate as candidate
import test_collector_delivery as fixtures


def measure_host_reuse(output, runtime):
    """Actual auto-mode acquisition and native activation against this host."""
    release, objects = output / 'release-1', output / 'transport-1'
    descriptor, trust = assets.read(objects / 'transport.json'), assets.read(output / 'trust.json')
    root, cache = output / 'auto-installed', output / 'auto-cache'
    cache.mkdir(mode=0o700)
    started = time.monotonic()
    with lifecycle.locked(root):
        plan, sources = components.plan(descriptor, root, cache, mode='auto')
        with tempfile.TemporaryDirectory(dir=cache) as temporary:
            for row in plan['objects']:
                cached = cache / (row['sha256'] + '-' + row['name'])
                shutil.copyfile(objects / row['name'], cached)
                expanded = Path(temporary) / row['object']
                components.expand(cached, expanded, {'size': row['expanded_size'], 'sha256': row['object']})
                sources[row['object']] = expanded
            components.install(root, release, descriptor, sources, trust,
                               'rust-collector-v0.1.0-rc.1', self_check=light.self_check)
        result = {'mode': 'auto', 'offline_acquired_component_bytes': plan['download_bytes'],
                  'network_bytes': 0, 'objects': plan['objects'], 'storage': store.usage(root),
                  'cache_bytes': sum(p.stat().st_size for p in cache.iterdir()),
                  'native_self_check': True, 'seconds': time.monotonic() - started}
    assets.write(output / 'auto-report.json', result)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('runtime', 'core', 'binding', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--resume', action='store_true', help='reuse packing after a failure before the first successful install')
    args = parser.parse_args()
    runtime, core, output = args.runtime.resolve(), args.core.resolve(), args.output.absolute()
    output.mkdir(parents=True, exist_ok=args.resume)
    assets.require(not (output / 'report.json').exists(), 'completed install present; use a fresh output directory')
    source = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
    core_id = {'version': subprocess.check_output([str(core), '--version'], text=True).strip().split()[-1],
               'commit': source, 'sha256': assets.sha(core)}
    release = output / 'release-1'
    binding = assets.read(args.binding)
    # Diagnostic packaging label; this copy is never used as capture authority.
    binding['input']['collector']['version'] = '0.1.0-rc.1'
    if not args.resume or not release.exists():
        candidate.stage(runtime, core, core_id, source, binding, release)
    inventory = assets.read(runtime / 'runtime.json')
    openssl = str(Path(shutil.which('openssl')).resolve())
    if not (output / 'key.pem').exists():
        subprocess.run([openssl, 'genpkey', '-algorithm', 'RSA', '-pkeyopt', 'rsa_keygen_bits:2048',
                        '-out', str(output / 'key.pem')], check=True, capture_output=True)
    subprocess.run([openssl, 'pkey', '-in', str(output / 'key.pem'), '-pubout',
                    '-out', str(output / 'public.pem')], check=True, capture_output=True)
    trust = {'schema': 'rust-collector-host-trust/v1', 'openssl': openssl, 'rsa_signature_bytes': 256,
             'openssl_sha256': assets.sha(openssl), 'public_key': str(output / 'public.pem'),
             'public_key_sha256': assets.sha(output / 'public.pem'),
             'host_libraries': {k: str(Path(v['source']).resolve()) for k, v in inventory['host']['libraries'].items()}}
    assets.write(output / 'trust.json', trust)
    report = {'schema': 'collector-native-storage-diagnostic/v1', 'production_acceptance': False,
              'signature': 'disposable local RSA, test eligibility; no Sigstore certification',
              'network_bytes': 0, 'core': core_id, 'runtime_archive_bytes': Path(str(runtime) + '.tar').stat().st_size,
              'runs': [], 'plans': {}}
    launcher_name = 'bin/harness-gate-rust-collector'
    base_manifest = assets.read(release / 'manifest.json')
    previous_descriptor = None
    for number in (1, 2, 3, 4):
        started = time.monotonic()
        if number != 1:
            release = output / ('release-' + str(number))
            release.mkdir()
            manifest = copy.deepcopy(base_manifest)
            manifest['collector']['version'] = '0.1.0-rc.' + str(number)
            launcher = (runtime / launcher_name).read_bytes() + ('\n# local upgrade diagnostic ' + str(number) + '\n').encode()
            changed_inventory = copy.deepcopy(inventory)
            changed_inventory['payload'][launcher_name]['sha256'] = assets.hashlib.sha256(launcher).hexdigest()
            changed = {launcher_name: launcher, 'runtime.json': (json.dumps(changed_inventory, sort_keys=True, indent=2) + '\n').encode()}
            for row in manifest['payloads']:
                if row['path'] in changed:
                    row['sha256'] = assets.hashlib.sha256(changed[row['path']]).hexdigest()
            manifest['dependency_inventory_sha256'] = assets.hashlib.sha256(changed['runtime.json']).hexdigest()
            with tarfile.open(str(runtime) + '.tar', 'r:') as old, tarfile.open(release / 'collector.tar', 'w') as new:
                for member in old:
                    if member.name in changed:
                        data = changed[member.name]
                        member.size = len(data)
                        new.addfile(member, io.BytesIO(data))
                    else:
                        new.addfile(member, old.extractfile(member) if member.isfile() else None)
            assets.write(release / 'manifest.json', manifest)
        manifest = assets.read(release / 'manifest.json')
        assets.prepare(release, fixtures.receipt(manifest))
        subprocess.run([openssl, 'dgst', '-sha256', '-sign', str(output / 'key.pem'),
                        '-out', str(release / assets.CONTROL[1]), str(release / assets.CONTROL[0])], check=True)
        objects = output / ('transport-' + str(number))
        if args.resume and objects.exists():
            descriptor = assets.read(objects / 'transport.json')
            components.validate(descriptor)
            assets.require(descriptor['archive_sha256'] == assets.sha(release / 'collector.tar'), 'wrong resumed archive')
        else:
            descriptor = components.pack(release / 'collector.tar', objects, previous=(previous_descriptor, output / ('transport-' + str(number - 1))) if previous_descriptor else None)
        previous_descriptor = descriptor
        if number == 1:
            for mode in ('auto', 'pinned'):
                plan_root = output / ('plan-' + mode)
                with lifecycle.locked(plan_root):
                    plan, _ = components.plan(descriptor, plan_root, output / 'plan-cache', mode=mode)
                    report['plans'][mode] = plan
            with lifecycle.locked(output / 'plan-explicit'):
                plan, _ = components.plan(descriptor, output / 'plan-explicit', output / 'plan-cache', reuse=runtime)
                report['plans']['explicit_compatible_runtime'] = plan
            # Reproduce the old on-disk format exclusively in this diagnostic root.
            from unittest.mock import patch
            with patch.object(lifecycle, 'compact'):
                lifecycle.install(output / 'legacy', release, trust, 'rust-collector-v0.1.0-rc.1')
            report['migration'] = maintenance.migrate(output / 'legacy', trust)
            lifecycle.select(output / 'legacy', '0.1.0-rc.1', trust)
            report['export'] = maintenance.export(output / 'legacy', '0.1.0-rc.1', trust, output / 'export')
        root, cache = output / 'installed', output / 'cache'
        cache.mkdir(mode=0o700, exist_ok=True)
        with lifecycle.locked(root):
            plan, sources = components.plan(descriptor, root, cache, mode='pinned')
            checkpoints = {}
            with tempfile.TemporaryDirectory(dir=cache) as temporary:
                for row in plan['objects']:
                    cached = cache / (row['sha256'] + '-' + row['name'])
                    if not cached.exists():
                        shutil.copyfile(objects / row['name'], cached)
                    expanded = Path(temporary) / row['object']
                    components.expand(cached, expanded, {'size': row['expanded_size'], 'sha256': row['object']})
                    sources[row['object']] = expanded
                expanded_bytes = sum(p.stat().st_blocks * 512 for p in Path(temporary).iterdir())
                installed = components.install(root, release, descriptor, sources, trust,
                    'rust-collector-v' + manifest['collector']['version'], self_check=light.self_check,
                    checkpoint=lambda name: checkpoints.update({name: store.usage(root)['total_allocated_bytes']}))
            cleanup = maintenance.cleanup_locked(root, trust, keep=2)
            run = {'version': manifest['collector']['version'], 'objects': plan['objects'],
                   'offline_acquired_component_bytes': plan['download_bytes'], 'network_bytes': 0,
                   'signed_metadata_bytes': sum((release / n).stat().st_size for n in assets.ASSETS[1:] + assets.CONTROL),
                   'installed': str(installed), 'storage': store.usage(root), 'cleanup': cleanup,
                   'cache_bytes': sum(p.stat().st_size for p in cache.iterdir() if p.is_file()),
                   'expanded_temporary_allocated_bytes': expanded_bytes, 'storage_checkpoints': checkpoints,
                   'peak_preparation_allocated_upper_bound': max(checkpoints.values()) + expanded_bytes + max(r['size'] for r in descriptor['payloads']) * 2,
                   'seconds': time.monotonic() - started}
            report['runs'].append(run)
        assets.write(output / 'report.json', report)
        print(json.dumps({k: v for k, v in run.items() if k not in ('objects', 'cleanup', 'storage_checkpoints')}, sort_keys=True), flush=True)
    lifecycle.select(root, '0.1.0-rc.3', trust)
    report['rollback'] = assets.read(root / 'current.json')
    report['auto_install'] = measure_host_reuse(output, runtime)
    assets.write(output / 'report.json', report)


if __name__ == '__main__':
    main()
