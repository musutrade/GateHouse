#!/usr/bin/python3.14 -I
"""Root bootstrap: compile a closed, configuration-pinned source snapshot, never pyc."""
from __future__ import annotations

import hashlib
import importlib.abc
import importlib.util
import json
import os
from pathlib import Path
import stat
import sys
from types import MappingProxyType


MODULES = ('__init__', 'cache', 'cli', 'common', 'control', 'execution', 'github', 'history',
           'host', 'isolation', 'journal', 'publication', 'runtime', 'source')
CONFIG = Path('/etc/gatehouse/host.json')
INTERPRETER = Path('/usr/bin/python3.14')
MAX_SOURCE = 1024 * 1024


class HostBlocked(ValueError):
    """No controller or publication state may be opened for this generation."""


def need(condition):
    if not condition:
        raise HostBlocked('controller source/configuration generation is unavailable or changed')


def identity(info):
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid, info.st_nlink,
            info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def protected(path, *, directory=False, mode=None):
    need(path.is_absolute() and '..' not in path.parts)
    for item in (path, *path.parents):
        info = item.lstat()
        need(info.st_uid == 0 and not info.st_mode & 0o022 and not stat.S_ISLNK(info.st_mode))
        if item != path:
            need(stat.S_ISDIR(info.st_mode))
    info = path.lstat()
    need(stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode) and info.st_nlink == 1)
    need(mode is None or stat.S_IMODE(info.st_mode) == mode)
    return identity(info)


def read(path, limit, *, mode):
    before = protected(path, mode=mode)
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as stream:
        info = os.fstat(stream.fileno())
        need(identity(info) == before and 0 < info.st_size <= limit)
        raw = stream.read(limit + 1)
        need(identity(os.fstat(stream.fileno())) == before)
    need(len(raw) == info.st_size and protected(path, mode=mode) == before)
    return raw, before


def unique(pairs):
    result = {}
    for key, value in pairs:
        need(key not in result)
        result[key] = value
    return result


def inventory(root):
    protected(root, directory=True)
    names = set()
    for path in root.iterdir():
        if path.name == '__pycache__':
            # Existing cache is retained as data. It never participates in importing.
            protected(path, directory=True)
        else:
            names.add(path.name)
    need(names == {name + '.py' for name in MODULES})


class Snapshot:
    def __init__(self, root, config_path=CONFIG, interpreter=INTERPRETER):
        self.root = Path(root)
        self.config_path = Path(config_path)
        self.interpreter = Path(interpreter)
        inventory(self.root)
        raw, stamp = read(self.config_path, MAX_SOURCE, mode=0o600)
        config = json.loads(raw, object_pairs_hook=unique, parse_constant=lambda _: need(False))
        need(isinstance(config, dict) and isinstance(config.get('protected_files'), dict))
        pins = config['protected_files']
        self.raw = raw
        self.stamps = {self.config_path: (stamp, 0o600)}
        sources = {}
        for name in MODULES:
            path = self.root / (name + '.py')
            data, stamp = read(path, MAX_SOURCE, mode=0o644)
            need(hashlib.sha256(data).hexdigest() == pins.get(str(path)))
            self.stamps[path] = (stamp, 0o644)
            sources[name] = data
        data, stamp = read(self.interpreter, 128 * MAX_SOURCE, mode=0o755)
        need(hashlib.sha256(data).hexdigest() == pins.get(str(self.interpreter)))
        self.stamps[self.interpreter] = (stamp, 0o755)
        # Compile every module before executing even the package initializer. Explicit
        # buffers avoid timestamp/size-valid stale caches, including with Python -B.
        self.codes = MappingProxyType({name: compile(data, str(self.root / (name + '.py')), 'exec',
                                                   dont_inherit=True, optimize=0)
                                      for name, data in sources.items()})
        self.stamps = MappingProxyType(self.stamps)
        self.verify()

    def verify(self):
        inventory(self.root)
        for path, (stamp, mode) in self.stamps.items():
            need(protected(path, mode=mode) == stamp)

    def configuration(self, path):
        need(Path(path) == self.config_path)
        self.verify()
        return self.raw


class SourceFinder(importlib.abc.MetaPathFinder, importlib.abc.Loader):
    def __init__(self, snapshot):
        self.snapshot = snapshot

    def find_spec(self, fullname, path=None, target=None):
        if fullname != 'trusted_gate' and not fullname.startswith('trusted_gate.'):
            return None
        key = '__init__' if fullname == 'trusted_gate' else fullname.removeprefix('trusted_gate.')
        if key not in self.snapshot.codes or '.' in key:
            raise ImportError('controller import is outside the pinned source inventory')
        return importlib.util.spec_from_loader(fullname, self, is_package=fullname == 'trusted_gate')

    def create_module(self, spec):
        return None

    def exec_module(self, module):
        self.snapshot.verify()
        key = '__init__' if module.__name__ == 'trusted_gate' else module.__name__.removeprefix('trusted_gate.')
        module.__file__ = str(self.snapshot.root / (key + '.py'))
        module.__cached__ = None
        if key == '__init__':
            module.__path__ = []
            module.__host_configuration__ = self.snapshot.configuration
        exec(self.snapshot.codes[key], module.__dict__)


def run(snapshot, argv=None):
    need(not any(name == 'trusted_gate' or name.startswith('trusted_gate.') for name in sys.modules))
    snapshot.verify()
    finder = SourceFinder(snapshot)
    sys.meta_path.insert(0, finder)
    try:
        from trusted_gate.cli import main
        snapshot.verify()
        return main(argv)
    finally:
        sys.meta_path.remove(finder)
        for name in list(sys.modules):
            if name == 'trusted_gate' or name.startswith('trusted_gate.'):
                del sys.modules[name]


def main():
    try:
        need(os.geteuid() == 0 and Path(sys.executable) == INTERPRETER
             and os.path.samefile('/proc/self/exe', INTERPRETER))
        snapshot = Snapshot(Path(__file__).absolute().parent)
        need(os.path.samefile('/proc/self/exe', INTERPRETER))
        return run(snapshot)
    except (HostBlocked, OSError, ValueError, SyntaxError, ImportError):
        # No paths, configuration values, source text, or credentials in this diagnostic.
        print('{"status":"blocked","reason":"controller source/configuration generation rejected"}',
              file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
