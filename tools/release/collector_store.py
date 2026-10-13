"""Private, mode-aware shared payload store. Caller holds the lifecycle lock."""
import os
from pathlib import Path
import re
import shutil
import stat

import collector_assets as assets
import collector_receipt as receipt

KEY = re.compile(r'[0-9a-f]{64}\.[0-7]{4}')


def directory(root):
    path = root / 'objects'
    assets.require(not path.is_symlink(), 'unsafe shared object directory')
    path.mkdir(mode=0o700, exist_ok=True)
    assets.require(path.stat().st_uid == os.getuid() and path.stat().st_mode & 0o077 == 0,
                   'shared object directory must be private')
    return path


def share(root, path, digest):
    receipt.regular(path)
    assets.require(assets.sha(path) == digest, 'shared payload checksum mismatch')
    mode = stat.S_IMODE(path.stat().st_mode)
    assets.require(mode in (0o644, 0o755), 'unsafe shared payload mode')
    store = directory(root)
    target = store / (digest + '.' + format(mode, '04o'))
    temporary = store / '.incoming'
    if temporary.exists() or temporary.is_symlink():
        receipt.regular(temporary)
        temporary.unlink()
    if target.exists() or target.is_symlink():
        receipt.regular(target)
        assets.require(assets.sha(target) == digest and stat.S_IMODE(target.stat().st_mode) == mode,
                       'corrupt shared payload')
    else:
        try:
            shutil.copyfile(path, temporary)
            temporary.chmod(mode)
            with temporary.open('rb') as stream:
                os.fsync(stream.fileno())
            os.replace(temporary, target)
        finally:
            temporary.unlink(missing_ok=True)
    try:
        os.link(target, temporary)
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def usage(root):
    """Allocated bytes are counted once per inode, including shared versions."""
    seen, groups = set(), {}
    for name in ('versions', 'objects', 'staging'):
        logical, allocated, files = 0, 0, 0
        base = root / name
        assets.require(not base.is_symlink(), 'unsafe usage directory')
        for current, dirs, entries in os.walk(base, followlinks=False):
            for child in dirs:
                assets.require(not (Path(current) / child).is_symlink(), 'unsafe usage link')
            for entry in entries:
                path = Path(current) / entry
                receipt.regular(path)
                info = path.stat()
                logical += info.st_size
                files += 1
                inode = (info.st_dev, info.st_ino)
                if inode not in seen:
                    allocated += info.st_blocks * 512
                    seen.add(inode)
        groups[name] = {'logical_bytes': logical, 'unique_allocated_bytes': allocated, 'files': files}
    groups['total_allocated_bytes'] = sum(v['unique_allocated_bytes'] for v in groups.values())
    return groups


def collect(root):
    store = directory(root)
    candidates = []
    for path in store.iterdir():
        assets.require(KEY.fullmatch(path.name) or path.name == '.incoming', 'unknown shared object')
        receipt.regular(path)
        if path.stat().st_nlink == 1:
            candidates.append(path)
    freed = sum(p.stat().st_blocks * 512 for p in candidates)
    for path in candidates:
        path.unlink()
    return freed
