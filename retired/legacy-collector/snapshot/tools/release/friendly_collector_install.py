"""User entry point carried in the independently pinned installer capsule."""
import argparse
import base64
import concurrent.futures
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parent))
import collector_assets as assets
import collector_transport as transport
import install_collector as lifecycle


def download_parts(url, temporary, size):
    """Retain completed ranges across failures; never accept a full-body range reply."""
    directory = temporary.with_name(temporary.name + '-ranges')
    assets.require(not directory.is_symlink(), 'unsafe range cache')
    directory.mkdir(mode=0o700, exist_ok=True)
    pairs = [(a, min(a + 4*1024**2, size)-1) for a in range(0, size, 4*1024**2)]
    def get(pair):
        a,b=pair; path=directory/str(a); headers=directory/(str(a)+'.headers')
        for p in (path, headers):
            if p.exists() or p.is_symlink(): assets.regular(p)
        expected=f'content-range: bytes {a}-{b}/{size}'
        def complete():
            return path.exists() and path.stat().st_size==b-a+1 and headers.exists() and expected in headers.read_text().lower().splitlines()
        if complete(): return
        for _ in range(5):
            result=subprocess.run(['curl','--fail','--silent','--show-error','--location',
                '--proto','=https','--proto-redir','=https','--http1.1','--connect-timeout','15',
                '--max-time','240','--speed-limit','1024','--speed-time','60','--range',f'{a}-{b}',
                '--dump-header',str(headers),'--output',str(path),url],capture_output=True)
            if result.returncode==0 and complete(): return
        raise ValueError('Download interrupted; rerun to resume verified ranges.')
    with concurrent.futures.ThreadPoolExecutor(max_workers=16) as pool:
        list(pool.map(get,pairs))
    with temporary.open('wb') as output:
        for a,b in pairs:
            with (directory/str(a)).open('rb') as source: shutil.copyfileobj(source,output)


def fetch(row, base, cache, offline=None):
    name = assets.safe_name(row['name'])
    assets.require('/' not in name, 'download name must be a basename')
    transport.blob_name(row['sha256'])
    target = cache / (row['sha256'] + '-' + name)
    if target.exists() or target.is_symlink():
        assets.regular(target)
        assets.require(target.stat().st_size == row['size'] and assets.sha(target) == row['sha256'], 'corrupt download cache')
        print('Using cached ' + name, flush=True)
        return target
    temporary = cache / (target.name + '.part')
    if temporary.exists() or temporary.is_symlink():
        assets.regular(temporary)
    if offline:
        source = offline / name
        assets.regular(source)
        shutil.copyfile(source, temporary)
    else:
        url = row.get('url', base + '/' + name)
        assets.require(url.startswith('https://'), 'HTTPS download required')
        print('Downloading ' + name + ' (' + str(round(row['size']/1024**2, 1)) + ' MiB)', flush=True)
        if row['size'] > 16*1024**2:
            download_parts(url,temporary,row['size'])
        else:
            subprocess.run(['curl', '--fail', '--show-error', '--location', '--proto', '=https',
                '--proto-redir', '=https', '--http1.1', '--retry', '5', '--retry-all-errors',
                '--connect-timeout', '15', '--speed-limit', '1024', '--speed-time', '60',
                '--output', str(temporary), url], check=True)
    assets.require(temporary.stat().st_size == row['size'] and assets.sha(temporary) == row['sha256'], 'download checksum mismatch')
    temporary.rename(target)
    ranges = temporary.with_name(temporary.name + '-ranges')
    if ranges.exists():
        assets.require(not ranges.is_symlink(), 'unsafe range cache')
        for entry in ranges.iterdir():
            assets.regular(entry)
            assets.require(entry.name.removesuffix('.headers').isdigit(), 'unknown download range file')
        for entry in ranges.iterdir():
            entry.unlink()
        ranges.rmdir()
    return target


def provision(catalog, directory, cache, offline):
    profile = catalog['trust']
    trust = dict(profile['host'])
    assets.require(trust['schema'] == 'rust-collector-host-trust/v2', 'dual-signature production trust required')
    # The profile is authenticated with the installer, never chosen by a plugin.
    assets.require(assets.probe_host(trust) == catalog['host_abi'],
                   'This Rust plugin does not support this host yet; Core can be installed separately.')
    assets.require(assets.sha(Path(trust['openssl'])) == trust['openssl_sha256'], 'unsupported host verifier')
    directory.mkdir(parents=True, exist_ok=True, mode=0o700)
    assets.require(not directory.is_symlink(), 'unsafe trust directory')
    for key in ('public_key', 'trusted_root'):
        raw = base64.b64decode(profile[key], validate=True)
        path = directory / key
        if path.exists() or path.is_symlink(): assets.regular(path)
        path.write_bytes(raw)
        assets.require(assets.sha(path) == trust[key + '_sha256'], 'invalid installer trust profile')
        trust[key] = str(path)
    verifier = profile['verifier']
    cached = cache / (verifier['sha256'] + '-' + assets.safe_name(verifier['name']))
    if not cached.exists() and not cached.is_symlink():
        candidate = shutil.which('cosign')
        if candidate:
            candidate = Path(candidate).resolve()
            try:
                assets.regular(candidate)
                reusable = candidate.stat().st_size == verifier['size'] and assets.sha(candidate) == verifier['sha256']
            except (OSError, ValueError):
                reusable = False
            if reusable:
                shutil.copyfile(candidate, cached)
                print('Reusing the pinned signature verifier already available on this host.', flush=True)
    cosign = fetch(verifier, catalog['base_url'], cache, offline)
    cosign.chmod(0o700)
    trust['cosign'] = str(cosign)
    assets.write(directory / 'trust.json', trust)
    return trust


def install(catalog, root, cache, offline=None, *, mode='auto', policy='deny', reuse=None,
            plan_only=False, cache_limit=512 * 1024**2):
    if catalog['schema'] == 'rust-collector-user-install/v2':
        import collector_light_install
        return collector_light_install.install(catalog, root.absolute(), cache.absolute(), offline,
            mode=mode, policy=policy, reuse=reuse, plan_only=plan_only, cache_limit=cache_limit)
    assets.require(catalog['schema'] == 'rust-collector-user-install/v1', 'unknown installer catalog')
    root, cache = root.absolute(), cache.absolute()
    # Serialize cache writes and reject unsafe ancestors/permissions using the
    # existing lifecycle lock; a partial download is retained for retry.
    with lifecycle.locked(cache):
        trust = provision(catalog, cache / 'trust', cache, offline)
        selected = assets.version(catalog['version'])
        if (root / 'versions' / selected).exists():
            result = lifecycle.select(root, selected, trust)
            print('Rust plugin already installed and verified: ' + str(result))
            return result
        row = catalog['transport']
        descriptor = assets.read(fetch(row, catalog['base_url'], cache, offline))
        assets.require(descriptor['archive_sha256'] == catalog['archive_sha256'], 'wrong plugin archive')
        with tempfile.TemporaryDirectory(prefix='install-', dir=cache) as temporary:
            work = Path(temporary)
            bundles, release = work / 'bundles', work / 'release'
            bundles.mkdir(); release.mkdir()
            for row in descriptor['bundles']:
                source = fetch(row, catalog['base_url'], cache, offline)
                shutil.copyfile(source, bundles / row['name'])
            for row in catalog['controls']:
                source = fetch(row, catalog['release_url'], cache, offline)
                shutil.copyfile(source, release / row['name'])
            transport.reconstruct(descriptor, bundles, release / 'collector.tar', cache / 'objects')
            result = lifecycle.install(root, release, trust, 'rust-collector-v' + selected)
        print('Installed Rust plugin: ' + str(result))
        print('Toolchain cache retained at ' + str(cache))
        return result


def main():
    parser = argparse.ArgumentParser(description='Install the optional Rust plugin with automatic verification and cached tools.')
    parser.add_argument('--catalog', type=Path, required=True)
    parser.add_argument('--root', type=Path, default=Path.home()/'.local/share/harness-gate/rust-collector')
    parser.add_argument('--cache-dir', type=Path, default=Path.home()/'.cache/harness-gate/collector')
    parser.add_argument('--offline', type=Path)
    parser.add_argument('--mode', choices=('auto', 'pinned'), default='auto')
    parser.add_argument('--download-policy', choices=('allow', 'deny'), default='deny')
    parser.add_argument('--reuse-runtime', type=Path)
    parser.add_argument('--plan', action='store_true', help='show exact object/byte plan without downloads')
    parser.add_argument('--cache-limit-bytes', type=int, default=512 * 1024**2)
    parser.add_argument('--action', choices=('install', 'usage', 'cleanup', 'migrate', 'rollback', 'export'), default='install')
    parser.add_argument('--version')
    parser.add_argument('--export-output', type=Path)
    parser.add_argument('--keep', type=int, default=2)
    parser.add_argument('--dry-run', action='store_true')
    args = parser.parse_args()
    assets.require(args.cache_limit_bytes >= 0, 'cache limit must be nonnegative')
    if args.action != 'install':
        import collector_light_install
        collector_light_install.manage(assets.read(args.catalog), args)
        return
    install(assets.read(args.catalog), args.root, args.cache_dir, args.offline, mode=args.mode,
            policy=args.download_policy, reuse=args.reuse_runtime, plan_only=args.plan,
            cache_limit=args.cache_limit_bytes)


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        raise SystemExit('Installation failed: ' + str(error))
