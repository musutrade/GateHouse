"""Verified migration, bounded rollback retention and explicit offline export."""
import os
from pathlib import Path
import shutil
import tempfile

import collector_assets as assets
import collector_receipt as receipt
import collector_store as store
import install_collector as lifecycle


def versions(root):
    result = []
    for path in (root / 'versions').iterdir():
        assets.version(path.name)
        assets.require(path.is_dir() and not path.is_symlink(), 'unsafe version directory')
        result.append(path)
    return sorted(result, key=lambda p: (p.stat().st_mtime_ns, p.name), reverse=True)


def cleanup_locked(root, trust, keep=2, *, dry_run=False):
    assets.require(type(keep) is int and keep >= 0, 'rollback count must be nonnegative')
    paths = versions(root)
    current = root / 'current.json'
    selected = None
    if current.exists():
        assets.regular(current)
        selected = assets.version(assets.read(current)['version'])
        assets.require(any(p.name == selected for p in paths), 'missing current version')
    retained = {p.name for p in paths if p.name == selected}
    retained.update([p.name for p in paths if p.name != selected][:keep])
    # Validate every version before deleting anything, including rollback references.
    checked = {p.name: lifecycle.installed(root, p.name, trust) for p in paths}
    removing = [p.name for p in paths if p.name not in retained]
    before = store.usage(root)
    if not dry_run:
        for name in removing:
            path, names = checked[name]
            lifecycle.atomic(root / 'transaction.json', {'directory': str(path.relative_to(root)), 'files': names})
            lifecycle.recover(root)
        store.collect(root)
        lifecycle.sync(store.directory(root))
    after = store.usage(root)
    return {'retained': sorted(retained), 'removed' if not dry_run else 'would_remove': removing,
            'before': before, 'after': after,
            'reclaimed_allocated_bytes': before['total_allocated_bytes'] - after['total_allocated_bytes']}


def migrate(root, trust):
    with lifecycle.locked(root):
        checked = [lifecycle.installed(root, p.name, trust)[0] for p in versions(root)]
        before = store.usage(root)
        for path in checked:
            lifecycle.compact(root, path, trust, 'rust-collector-v' + path.name)
            lifecycle.installed(root, path.name, trust)
        store.collect(root)
        lifecycle.sync(store.directory(root))
        after = store.usage(root)
        return {'versions': [p.name for p in checked], 'before': before, 'after': after,
                'reclaimed_allocated_bytes': before['total_allocated_bytes'] - after['total_allocated_bytes']}


def export(root, selected, trust, output):
    """Reconstitute the original six authenticated release assets, on explicit request."""
    assets.require(output.is_absolute() and not output.exists() and not output.is_symlink(),
                   'export needs a new absolute output directory')
    with lifecycle.locked(root):
        path, _ = lifecycle.installed(root, selected, trust)
        with tempfile.TemporaryDirectory(prefix='.collector-export-', dir=output.parent) as temporary:
            stage = Path(temporary) / 'release'
            stage.mkdir(mode=0o700)
            metadata = path / '.delivery'
            for name in assets.ASSETS[1:] + assets.CONTROL:
                shutil.copyfile(metadata / name, stage / name)
            if (metadata / receipt.NAME).exists():
                receipt.export(assets.read(metadata / receipt.NAME), path, stage / 'collector.tar')
            else:
                shutil.copyfile(metadata / 'collector.tar', stage / 'collector.tar')
            assets.verify(stage, trust, 'rust-collector-v' + selected)
            for entry in stage.iterdir():
                with entry.open('rb') as stream:
                    os.fsync(stream.fileno())
            lifecycle.sync(stage)
            os.rename(stage, output)
            lifecycle.sync(output.parent)
    return str(output)
