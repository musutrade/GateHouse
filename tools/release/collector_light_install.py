"""User-facing component planning and acquisition under shared lifecycle locks."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

import collector_assets as assets
import collector_components as components
import collector_store as store
import install_collector as lifecycle


def self_check(root, manifest=None):
    """Run only after full release authentication; do not create project evidence."""
    root = root.resolve(strict=True)
    manifest = manifest or assets.read(root / '.delivery/manifest.json')
    env = {'PATH': str(root / 'bin') + ':/usr/bin:/bin', 'LANG': 'C',
           'LD_LIBRARY_PATH': str(root / 'lib') + ':' + str(root / 'rust/lib'),
           'PYTHONDONTWRITEBYTECODE': '1'}
    for row in manifest['tools']:
        if row['name'] in ('driver', 'rustc-driver-library'):
            assets.require(row['version'] == 'sha256:' + row['sha256'], 'wrong compiler-private identity')
            continue
        result = subprocess.run([str(root / row['path']), '-vV' if row['name'] == 'rustc' else '--version'],
                                env=env, capture_output=True, text=True, timeout=30)
        observed = ' | '.join(result.stdout.strip().splitlines())
        assets.require(result.returncode == 0 and observed == row['version'],
                       'incompatible runtime component: ' + row['name'])
    result = subprocess.run([str(root / 'bin/harness-gate-rust-collector'), '--help'],
                            env=env, capture_output=True, timeout=30)
    assets.require(result.returncode == 0, 'collector startup self-check failed')

    # Compile, execute and export coverage using only the private runtime.
    # This disposable diagnostic never adopts a project's policy or baseline.
    with tempfile.TemporaryDirectory(prefix='collector-selfcheck-', dir=root.parent) as temporary:
        work = Path(temporary)
        source = work / 'fixture.rs'
        source.write_text('fn main() { let n = std::hint::black_box(1); assert!(n > 0); }\n')
        env.update(PATH=str(root / 'bin'), TMPDIR=str(work), CARGO_HOME=str(work / 'cargo'),
                   CARGO_NET_OFFLINE='true')
        code = ('import sys; from pathlib import Path; r=Path(sys.argv[1]); '
                'sys.path.insert(0,str(r/"app")); import rust_native_driver as n; '
                'n.collect_fixture(Path(sys.argv[2]),Path(sys.argv[3]),'
                'r/"bin/harness-gate-rust-native-driver",r/"rust")')
        result = subprocess.run([str(root / 'python/bin/python3'), '-I', '-S', '-B', '-c',
                                 code, str(root), str(source), str(work / 'capture')],
                                cwd=work, env=env, capture_output=True, text=True, timeout=180)
        assets.require(result.returncode == 0, 'compiler/measurement self-check failed: ' + result.stderr[-4000:])


def cached_bytes(row, cache):
    path = cache / (row['sha256'] + '-' + row['name'])
    return 0 if components.matches(path, row, shared=True) else row['size']


def prune_cache(cache, maximum, protected=()):
    """Only complete digest-addressed downloads; trust and partials are preserved."""
    import re
    assets.require(maximum >= 0, 'cache capacity must be nonnegative')
    candidates = []
    protected_size = 0
    for path in cache.iterdir():
        if path.name in protected:
            assets.regular(path)
            protected_size += path.stat().st_size
            continue
        if re.fullmatch(r'[0-9a-f]{64}-[^/]+', path.name) and path.name not in protected and not path.name.endswith(('.part', '.part-ranges')):
            assets.regular(path)
            if assets.sha(path) == path.name[:64]:
                candidates.append(path)
    size = protected_size + sum(p.stat().st_size for p in candidates)
    freed = 0
    for path in sorted(candidates, key=lambda p: (p.stat().st_mtime_ns, p.name)):
        if size <= maximum:
            break
        count = path.stat().st_size
        path.unlink()
        size -= count
        freed += count
    return freed


def install(catalog, root, cache, offline, *, mode, policy, reuse, plan_only, cache_limit):
    assets.require(root.resolve() != cache.resolve(), 'installation root and cache must differ')
    from friendly_collector_install import fetch, provision
    descriptor = catalog['components']  # authenticated capsule embeds the entire plan
    assets.require(descriptor['archive_sha256'] == catalog['archive_sha256'], 'wrong component archive')
    assets.require(assets.probe_host(catalog['trust']['host']) == catalog['host_abi'],
                   'incompatible host ABI; no compatible components published for this host')
    with lifecycle.locked(cache), lifecycle.locked(root):
        plan, sources = components.plan(descriptor, root, cache, mode=mode, reuse=reuse)
        controls = catalog['controls']
        verifier = catalog['trust']['verifier']
        extras = [{'name': row['name'], 'download_bytes': cached_bytes(row, cache),
                   'reason': 'signature verification' if row is verifier else 'signed release metadata'}
                  for row in [verifier, *controls]]
        candidate = shutil.which('cosign')
        if candidate and components.matches(Path(candidate), verifier):
            extras[0]['download_bytes'] = 0
        plan['metadata'] = extras
        plan['download_bytes'] += sum(r['download_bytes'] for r in extras)
        plan['compatibility'] = catalog['compatibility']
        plan['offline'] = str(offline) if offline else None
        # Offline reads are reported separately from actual network bytes.
        plan['network_bytes'] = 0 if offline else plan['download_bytes']
        print(json.dumps(plan, sort_keys=True, indent=2), flush=True)
        if plan_only:
            return plan
        assets.require(policy in ('allow', 'deny'), 'choose --download-policy allow or deny after reviewing --plan')
        assets.require(offline or policy == 'allow' or plan['download_bytes'] == 0,
                       'download denied by CI policy; use --download-policy allow or --offline')
        # Reserve conservative extra copies across both filesystems before acquisition.
        required = 2 * plan['missing_expanded_bytes'] + plan['download_bytes'] + 16 * 1024**2
        for path in (root, cache):
            assets.require(shutil.disk_usage(path).free >= required, 'insufficient disk space before preparation')
        selected = assets.version(catalog['version'])
        trust = provision(catalog, cache / 'trust', cache, offline)
        if (root / 'versions' / selected).exists():
            path, _ = lifecycle.installed(root, selected, trust)
            self_check(path)
            lifecycle.atomic(root / 'current.json', {'version': selected})
            return path / 'bin/harness-gate-rust-collector'
        with tempfile.TemporaryDirectory(prefix='components-', dir=cache) as temporary:
            work = Path(temporary)
            metadata = work / 'metadata'
            metadata.mkdir()
            for row in controls:
                source = fetch(row, catalog['release_url'], cache, offline)
                shutil.copyfile(source, metadata / row['name'])
            # Authenticate signature and exact metadata before expanding any payload.
            assets.write(metadata / 'archive-receipt.json', descriptor['receipt'])
            manifest = assets.verify(metadata, trust, 'rust-collector-v' + selected,
                                     archive_sha256=descriptor['archive_sha256'])
            assets.require(all(manifest[key] == value for key, value in catalog['compatibility'].items()),
                           'component compatibility differs from signed manifest')
            for row in plan['objects']:
                base_url = catalog.get('object_base_urls', {}).get(row['object'], catalog['base_url'])
                compressed = fetch(row, base_url, cache, offline)
                destination = work / row['object']
                components.expand(compressed, destination,
                                  {'size': row['expanded_size'], 'sha256': row['object']})
                sources[row['object']] = destination
            result = components.install(root, metadata, descriptor, sources, trust,
                                        'rust-collector-v' + selected, self_check=self_check)
        # Keep at most two previous versions; shared inodes protect referenced tools.
        import collector_maintenance as maintenance
        maintenance.cleanup_locked(root, trust, keep=2)
        removed = prune_cache(cache, cache_limit,
                              protected=(verifier['sha256'] + '-' + verifier['name'],))
        print(json.dumps({'installed': str(result), 'storage': store.usage(root),
                          'cache_reclaimed_bytes': removed}, sort_keys=True), flush=True)
        return result


def cache_usage(cache):
    """Account for downloads and resumable partials without following links."""
    import re
    result = {'complete_bytes': 0, 'partial_bytes': 0, 'trust_bytes': 0}
    for path in cache.iterdir():
        if re.fullmatch(r'[0-9a-f]{64}-[^/]+\.part-ranges', path.name):
            assets.require(path.is_dir() and not path.is_symlink(), 'unsafe range cache')
            for entry in path.iterdir():
                assets.require(entry.name.removesuffix('.headers').isdigit(), 'unknown range file')
                assets.regular(entry)
                result['partial_bytes'] += entry.stat().st_size
        elif re.fullmatch(r'[0-9a-f]{64}-[^/]+', path.name):
            assets.regular(path)
            result['partial_bytes' if path.name.endswith('.part') else 'complete_bytes'] += path.stat().st_size
        elif path.name == 'trust':
            assets.require(path.is_dir() and not path.is_symlink(), 'unsafe trust directory')
            for entry in path.iterdir():
                assets.regular(entry)
                result['trust_bytes'] += entry.stat().st_size
    return result


def manage(catalog, args):
    """Maintenance uses the same authenticated bootstrap and automatic host trust."""
    from friendly_collector_install import provision
    import collector_maintenance as maintenance
    root, cache = args.root.absolute(), args.cache_dir.absolute()
    assets.require(root.resolve() != cache.resolve(), 'installation root and cache must differ')
    with lifecycle.locked(cache):
        if args.action == 'usage':
            with lifecycle.locked(root):
                result = {'runtime': store.usage(root), 'cache': cache_usage(cache)}
        else:
            verifier = catalog['trust']['verifier']
            needed = cached_bytes(verifier, cache)
            candidate = shutil.which('cosign')
            if candidate and components.matches(Path(candidate), verifier):
                needed = 0
            print(json.dumps({'action': args.action, 'verifier_bytes': needed,
                              'network_bytes': 0 if args.offline else needed,
                              'root': str(root), 'cache': str(cache)}, sort_keys=True), flush=True)
            if args.plan:
                return
            assets.require(args.offline or args.download_policy == 'allow' or needed == 0,
                           'verifier download denied; use --offline or --download-policy allow')
            trust = provision(catalog, cache / 'trust', cache, args.offline)
            if args.action == 'cleanup':
                with lifecycle.locked(root):
                    result = maintenance.cleanup_locked(root, trust, args.keep, dry_run=args.dry_run)
                result['cache_before'] = cache_usage(cache)
                result['cache_reclaimed_bytes'] = 0 if args.dry_run else prune_cache(
                    cache, args.cache_limit_bytes, protected=(Path(trust['cosign']).name,))
                result['cache_after'] = cache_usage(cache)
            elif args.action == 'migrate':
                result = maintenance.migrate(root, trust)
            else:
                assets.require(args.version is not None, '--version is required')
                if args.action == 'rollback':
                    result = {'selected': str(lifecycle.select(root, args.version, trust))}
                else:
                    assets.require(args.export_output is not None, '--export-output is required')
                    result = maintenance.export(root, args.version, trust, args.export_output)
        print(json.dumps(result, sort_keys=True, indent=2), flush=True)
        return result
