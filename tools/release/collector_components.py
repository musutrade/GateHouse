"""Independently addressed delivery objects; exact bytes, never SemVer guesses."""
import gzip
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import sysconfig
import tarfile
import uuid

import collector_assets as assets
import collector_receipt as receipt
import collector_store as store


def pack(archive, output, *, previous=None):
    output.mkdir(parents=True, exist_ok=False)
    replay = receipt.create(archive)
    rows, objects = [], {}
    with tarfile.open(archive, 'r:') as source:
        for member in source:
            if member.isdir():
                continue
            assets.require(member.isfile() and member.mode in (0o644, 0o755), 'unsafe component member')
            digest = assets.hashlib.file_digest(source.extractfile(member), 'sha256').hexdigest()
            rows.append({'path': assets.safe_name(member.name), 'sha256': digest,
                         'size': member.size, 'mode': member.mode})
            if digest not in objects:
                path = output / (digest + '.gz')
                old = previous[0]['objects'].get(digest) if previous else None
                if old and matches(previous[1] / old['name'], old, shared=True):
                    shutil.copyfile(previous[1] / old['name'], path)
                else:
                    with path.open('xb') as target, gzip.GzipFile(filename='', fileobj=target, mode='wb', mtime=0) as gz:
                        shutil.copyfileobj(source.extractfile(member), gz)
                objects[digest] = {'name': path.name, 'sha256': assets.sha(path), 'size': path.stat().st_size,
                                   'expanded_size': member.size}
    value = {'schema': 'rust-collector-components/v1', 'archive_sha256': assets.sha(archive),
             'receipt': replay, 'payloads': rows, 'objects': objects}
    assets.write(output / 'transport.json', value)
    return value


def validate(descriptor):
    assets.require(descriptor['schema'] == 'rust-collector-components/v1', 'unknown component schema')
    paths, sizes = set(), {}
    for row in descriptor['payloads']:
        path = assets.safe_name(row['path'])
        assets.require(path not in paths and path != '.delivery' and not path.startswith('.delivery/'), 'duplicate/reserved component path')
        paths.add(path)
        digest = row['sha256']
        assets.require(store.KEY.fullmatch(digest + '.0644'), 'invalid component digest')
        assets.require(type(row['size']) is int and 0 <= row['size'] <= receipt.MAX_ARCHIVE
                       and row['mode'] in (0o644, 0o755), 'invalid component size/mode')
        assets.require(digest not in sizes or sizes[digest] == row['size'], 'conflicting component size')
        sizes[digest] = row['size']
    assets.require(not paths & {str(p) for name in paths for p in Path(name).parents}, 'component path collision')
    assets.require(set(sizes) == set(descriptor['objects']) and sum(sizes.values()) <= receipt.MAX_ARCHIVE,
                   'component object inventory mismatch')
    for digest, row in descriptor['objects'].items():
        assets.require(row['expanded_size'] == sizes[digest] and type(row['size']) is int and 0 < row['size'] <= receipt.MAX_ARCHIVE and
                       row['name'] == digest + '.gz' and store.KEY.fullmatch(row['sha256'] + '.0644'),
                       'invalid compressed component size')
    assets.require({r['path'] for r in descriptor['receipt']['rows']} == paths, 'component receipt inventory mismatch')


def host_candidates(row, sysroot):
    """Known layout probes only. Reads do not mutate external environments."""
    name = row['path']
    if sysroot and name.startswith('rust/'):
        yield sysroot / name[5:]
    if name == 'python/bin/python3':
        yield Path(sys.executable)
    prefix = 'python/lib/' + Path(sysconfig.get_path('stdlib')).name + '/'
    if name.startswith(prefix):
        yield Path(sysconfig.get_path('stdlib')) / name[len(prefix):]
    if name.startswith('link/sysroot/usr/'):
        yield Path('/usr') / name[len('link/sysroot/usr/'):]
    if name.startswith('link/bin/'):
        yield Path('/usr/bin') / name[len('link/bin/'):]
    if name.startswith('link/gcc/'):
        suffix = name[len('link/gcc/'):]
        for base in ('/usr/lib/gcc/x86_64-linux-gnu/15', '/usr/libexec/gcc/x86_64-linux-gnu/15'):
            yield Path(base) / suffix
        if suffix in ('as', 'ld'):
            yield Path('/usr/bin') / suffix
    if name.startswith(('lib/', 'rust/lib/')):
        yield Path('/usr/lib/x86_64-linux-gnu') / Path(name).name


def matches(path, row, *, shared=False):
    try:
        path = path if shared else path.resolve(strict=True)
        receipt.regular(path)
        return path.stat().st_size == row['size'] and assets.sha(path) == row['sha256']
    except (OSError, ValueError):
        return False


def plan(descriptor, root, cache, *, mode='auto', reuse=None):
    validate(descriptor)
    store.directory(root)
    assets.require(mode in ('auto', 'pinned'), 'unknown environment mode')
    sysroot = None
    if mode == 'auto':
        rustc = shutil.which('rustc')
        if rustc:
            result = subprocess.run([rustc, '--print', 'sysroot'], capture_output=True, text=True, timeout=30)
            if result.returncode == 0:
                sysroot = Path(result.stdout.strip())
    sources, reasons = {}, {}
    for row in descriptor['payloads']:
        digest = row['sha256']
        if digest in sources:
            continue
        candidates = [(root / 'objects' / (digest + '.' + format(row['mode'], '04o')), 'private shared object')]
        if mode == 'auto':
            if reuse:
                candidates.append((reuse / row['path'], 'explicit compatible runtime'))
            candidates.extend((p, 'exact compatible host bytes') for p in host_candidates(row, sysroot))
        for path, reason in candidates:
            if matches(path, row, shared=reason == 'private shared object'):
                sources[digest] = path.resolve(strict=True)
                reasons[digest] = reason
                break
    downloads, paths_by_digest = [], {}
    for payload in descriptor['payloads']:
        paths_by_digest.setdefault(payload['sha256'], []).append(payload['path'])
    for digest, row in descriptor['objects'].items():
        if digest in sources:
            continue
        cached = cache / (row['sha256'] + '-' + row['name'])
        present = matches(cached, {'size': row['size'], 'sha256': row['sha256']}, shared=True)
        reasons[digest] = 'verified compressed cache' if present else 'missing exact compatible bytes'
        downloads.append({'object': digest, **row, 'download_bytes': 0 if present else row['size'],
                          'paths': paths_by_digest[digest], 'reason': reasons[digest]})
    return {'schema': 'rust-collector-install-plan/v1', 'root': str(root), 'cache': str(cache),
            'mode': mode, 'component_versions': component_versions(descriptor), 'download_bytes': sum(r['download_bytes'] for r in downloads),
            'missing_expanded_bytes': sum(r['size'] for r in descriptor['payloads']
                                          if reasons[r['sha256']] != 'private shared object'),
            'objects': downloads, 'reuse': reasons}, sources


def expand(compressed, destination, expected):
    """Bound decompression by authenticated expanded size; remove failed output."""
    try:
        with gzip.open(compressed, 'rb') as source, destination.open('xb') as target:
            remaining = expected['size']
            while remaining:
                data = source.read(min(1024 * 1024, remaining))
                assets.require(data, 'truncated component')
                target.write(data)
                remaining -= len(data)
            assets.require(not source.read(1), 'component exceeds declared size')
        assets.require(assets.sha(destination) == expected['sha256'], 'component checksum mismatch')
    except BaseException:
        destination.unlink(missing_ok=True)
        raise


def install(root, metadata, descriptor, sources, trust, tag, *, self_check=None, checkpoint=lambda _: None):
    """Caller holds root lock throughout planning, acquisition and activation."""
    import install_collector as lifecycle
    validate(descriptor)
    selected = assets.version(tag.removeprefix('rust-collector-v'))
    destination = root / 'versions' / selected
    assets.require(not destination.exists(), 'version already installed; use select')
    name = 'staging/' + uuid.uuid4().hex
    stage = root / name
    controls = list(assets.ASSETS[1:] + assets.CONTROL) + [receipt.NAME]
    names = [r['path'] for r in descriptor['payloads']] + ['.delivery/' + n for n in controls]
    lifecycle.atomic(root / 'transaction.json', {'directory': name, 'files': names})
    (stage / '.delivery').mkdir(parents=True, mode=0o700)
    for control in controls[:-1]:
        assets.regular(metadata / control)
        shutil.copyfile(metadata / control, stage / '.delivery' / control)
    assets.write(stage / '.delivery' / receipt.NAME, descriptor['receipt'])
    for row in descriptor['payloads']:
        path = stage / row['path']
        path.parent.mkdir(parents=True, exist_ok=True)
        source = sources[row['sha256']]
        assets.require(matches(source, row, shared=True), 'component changed after planning')
        shared = store.directory(root) / (row['sha256'] + '.' + format(row['mode'], '04o'))
        if source == shared:
            assets.require(stat.S_IMODE(source.stat().st_mode) == row['mode'], 'shared component mode mismatch')
            os.link(source, path)
        else:
            shutil.copyfile(source, path)
            path.chmod(row['mode'])
            store.share(root, path, row['sha256'])
    lifecycle.tree_check(stage, names, complete=True)
    manifest, _ = lifecycle.verify_directory(stage, trust, tag)
    assets.require({r['path']: r['sha256'] for r in manifest['payloads']} ==
                   {r['path']: r['sha256'] for r in descriptor['payloads']}, 'component manifest mismatch')
    checkpoint('verified')
    if self_check:
        self_check(stage)
        lifecycle.verify_directory(stage, trust, tag)
        lifecycle.tree_check(stage, names, complete=True)
    checkpoint('self-checked')
    for current, _, files in os.walk(stage):
        for entry in files:
            with (Path(current) / entry).open('rb') as stream:
                os.fsync(stream.fileno())
        lifecycle.sync(Path(current))
    os.rename(stage, destination)
    lifecycle.sync(root / 'versions')
    lifecycle.sync(root / 'staging')
    checkpoint('committed')
    lifecycle.atomic(root / 'current.json', {'version': selected})
    checkpoint('activated')
    (root / 'transaction.json').unlink()
    lifecycle.sync(root)
    return destination / 'bin/harness-gate-rust-collector'


def component_versions(descriptor):
    """Content revisions can advance independently; compatibility stays manifest-bound."""
    groups = {}
    for row in descriptor['payloads']:
        path = row['path']
        name = ('rust-llvm' if path.startswith('rust/') else
                'c-linker' if path.startswith('link/') else
                'python' if path.startswith('python/') else
                'licenses' if path.startswith('licenses/') else
                'runtime-libraries' if path.startswith('lib/') else 'plugin')
        groups.setdefault(name, []).append(row)
    return {name: {'revision': 'sha256:' + assets.contract.fingerprint(sorted(rows, key=lambda r: r['path'])),
                   'expanded_bytes': sum(r['size'] for r in rows), 'files': len(rows)}
            for name, rows in sorted(groups.items())}
