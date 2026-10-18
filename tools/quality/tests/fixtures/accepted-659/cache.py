"""Bounded local acceleration state. Cache entries never contain accepted gate results."""
from __future__ import annotations

import os
from pathlib import Path
import shutil
import time
import platform

from .common import canonical, checked, digest, protected, read_regular, require


DIMENSIONS = {'repository', 'platform', 'architecture', 'toolchain', 'target', 'profile',
              'features', 'flags', 'dependency_digest', 'build_config_digest', 'instrumentation',
              'release_incremental', 'executor_identity'}


def identity(dimensions):
    require(isinstance(dimensions, dict) and set(dimensions) == DIMENSIONS,
            'cache identity must declare every compatibility dimension')
    require(all(isinstance(value, str) and value for value in dimensions.values()),
            'cache identity cannot have unknown/empty dimensions')
    require(dimensions['release_incremental'] in ('true', 'false', 'not-applicable'),
            'release incremental compilation must be explicit')
    return digest(canonical(dimensions))


@checked
def dimensions(source, step, repository, executor_identity):
    """Resolve mutable compatibility inputs from this frozen source, never stale labels."""
    source = Path(source)
    result = dict(step['cache_identity'])
    identity(result)
    inputs = step['cache_inputs']
    require(set(inputs) == {'dependencies', 'configuration'}, 'declare dependency and build-configuration cache inputs')
    def files(names):
        require(isinstance(names, list) and len(names) <= 128 and len(set(names)) == len(names), 'invalid cache input inventory')
        values = []
        for name in sorted(names):
            path = Path(name)
            require(path.parts and not path.is_absolute() and '..' not in path.parts and '\\' not in name,
                    'cache input must stay in the frozen source')
            target = source / path
            require(not target.is_symlink() and not any(parent.is_symlink() for parent in target.parents
                    if parent == source or source in parent.parents), 'symbolic link in cache input')
            values.append((name, digest(read_regular(target, 16 * 1024 * 1024)) if target.exists() else 'absent'))
        return values
    toolchain = step['toolchain_files']
    require(isinstance(toolchain, list) and toolchain and len(toolchain) <= 64, 'pin the actual toolchain files')
    tools = [(str(protected(name)), digest(read_regular(name, 256 * 1024 * 1024))) for name in sorted(toolchain)]
    if result['release_incremental'] != 'not-applicable':
        require(step.get('env', {}).get('CARGO_PROFILE_RELEASE_INCREMENTAL') == result['release_incremental'],
                'declare the actual release incremental setting in the Cargo environment')
        expected = '1' if result['release_incremental'] == 'true' else '0'
        require(step.get('env', {}).get('CARGO_INCREMENTAL', expected) == expected,
                'CARGO_INCREMENTAL conflicts with the declared release setting')
    result.update(repository=repository, platform=platform.system().lower(), architecture=platform.machine(),
                  executor_identity=executor_identity, toolchain=digest(canonical(tools)),
                  dependency_digest=digest(canonical(files(inputs['dependencies']))),
                  build_config_digest=digest(canonical({'files': files(inputs['configuration']),
                                                       'argv': step['argv'], 'env': step.get('env', {})})))
    return result


def tree_bytes(path):
    total = 0
    for root, directories, files in os.walk(path, followlinks=False):
        for name in (*directories, *files):
            item = Path(root) / name
            require(not item.is_symlink(), 'cache contains a symbolic link; quarantine for operator review')
        for name in files:
            total += (Path(root) / name).stat().st_size
    return total


class Cache:
    def __init__(self, root, max_bytes, max_entries, ttl_seconds):
        self.root = Path(root)
        self.max_bytes, self.max_entries, self.ttl = max_bytes, max_entries, ttl_seconds
        require(all(type(v) is int and v > 0 for v in (max_bytes, max_entries, ttl_seconds)),
                'cache limits must be positive integers')

    def directory(self, dimensions):
        name = identity(dimensions)
        path = self.root / name
        require(not path.is_symlink(), 'cache entry cannot be a symbolic link')
        path.mkdir(mode=0o700, exist_ok=True)
        os.utime(path, None)
        return path

    def maintain(self, active=(), now=None):
        """Caller holds the host's global execution lock; never prune active/unknown state."""
        now = time.time() if now is None else now
        active = set(active)
        entries = []
        for path in self.root.iterdir():
            require(path.name and len(path.name) == 64 and all(c in '0123456789abcdef' for c in path.name)
                    and path.is_dir() and not path.is_symlink(), 'unknown cache entry; refuse cleanup')
            entries.append((path.stat().st_mtime, path, tree_bytes(path)))
        entries.sort(key=lambda item: (item[0], item[1].name))
        total = sum(row[2] for row in entries)
        count = len(entries)
        removed = []
        for stamp, path, size in entries:
            if path.name in active:
                continue
            if now - stamp > self.ttl or total > self.max_bytes or count > self.max_entries:
                # root ownership must be checked in the deployed controller before cleanup;
                # symlinks anywhere in a candidate entry have already been rejected.
                shutil.rmtree(path)
                total -= size
                count -= 1
                removed.append(path.name)
        require(total <= self.max_bytes and count <= self.max_entries, 'active caches exceed configured capacity')
        return {'bytes': total, 'entries': count, 'removed': removed}
