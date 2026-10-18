#!/usr/bin/python3.14 -I
"""Root bootstrap: compile a closed, configuration-pinned source snapshot, never pyc."""
from __future__ import annotations

import hashlib
import importlib.abc
import importlib.util
import json
import os
from pathlib import Path
import re
import stat
import sys
from types import MappingProxyType
import uuid


MODULES = ('__init__', 'cache', 'cli', 'common', 'control', 'execution', 'github', 'history',
           'host', 'isolation', 'journal', 'publication', 'runtime', 'source')
CONFIG = Path('/etc/gatehouse/host.json')
INTERPRETER = Path('/usr/bin/python3.14')
BARRIER = Path('/var/lib/gatehouse/installation.json')
ACTIVATION = Path('/srv/gatehouse/pilot/control/controller-activation.json')
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


def installation_barrier(path):
    protected(path.parent, directory=True)
    try:
        path.lstat()
    except FileNotFoundError:
        return None
    raw, stamp = read(path, 4096, mode=0o600)
    value = json.loads(raw, object_pairs_hook=unique, parse_constant=lambda _: need(False))
    need(isinstance(value, dict) and set(value) == {'schema', 'transaction', 'phase', 'head',
                                                  'configuration_sha256', 'plan_sha256'})
    need(value['schema'] == 'gatehouse-installation-barrier/v1' and value['phase'] == 'active'
         and all(isinstance(value[key], str) for key in value))
    transaction = uuid.UUID(value['transaction'])
    need(transaction.version == 4 and str(transaction) == value['transaction']
         and re.fullmatch('[0-9a-f]{40}', value['head'])
         and all(re.fullmatch('[0-9a-f]{64}', value[key]) for key in ('configuration_sha256', 'plan_sha256')))
    need(raw == json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode() + b'\n')
    return value, stamp


def controller_activation(path, barrier, config):
    temporary = path.with_name('.' + path.name + '.tmp')
    need(not temporary.exists() and not temporary.is_symlink())
    if barrier is None:
        need(not path.exists() and not path.is_symlink())
        return None
    raw, stamp = read(path, 4096, mode=0o600)
    value = json.loads(raw, object_pairs_hook=unique, parse_constant=lambda _: need(False))
    pins = ('helper_manifest_sha256', 'recipe_sha256', 'material_sha256', 'upgrade_receipt_sha256',
            'file_receipt_sha256', 'entry_fence_sha256')
    need(isinstance(value, dict) and set(value) == {'schema', 'head', 'transaction', 'lease', 'phase',
         'administrator_ready', 'production_gate_enabled', *pins})
    need(value['schema'] == 'gatehouse-controller-activation/v1' and value['phase'] == 'entries-enabled'
         and value['head'] == barrier[0]['head'] and value['transaction'] == barrier[0]['transaction']
         and value['administrator_ready'] is False and value['production_gate_enabled'] is False
         and all(isinstance(value[key], str) and re.fullmatch('[0-9a-f]{64}', value[key]) for key in pins)
         and value['helper_manifest_sha256'] == config.get('environment', {}).get('bootstrap_helper_manifest_sha256')
         and isinstance(value['lease'], list) and len(value['lease']) == 2
         and all(type(number) is int and number >= 0 for number in value['lease']) and value['lease'][1] > 0)
    need(raw == json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode() + b'\n')
    return value, stamp


class Snapshot:
    def __init__(self, root, config_path=CONFIG, interpreter=INTERPRETER, barrier_path=BARRIER,
                 activation_path=ACTIVATION):
        self.root = Path(root)
        self.config_path = Path(config_path)
        self.interpreter = Path(interpreter)
        self.barrier_path = Path(barrier_path)
        self.activation_path = Path(activation_path)
        self.armed = False
        # Stop incomplete installations before reading configuration or importing
        # any controller module, including direct administrative host invocation.
        self.barrier = installation_barrier(self.barrier_path)
        inventory(self.root)
        raw, stamp = read(self.config_path, MAX_SOURCE, mode=0o600)
        config = json.loads(raw, object_pairs_hook=unique, parse_constant=lambda _: need(False))
        need(isinstance(config, dict) and isinstance(config.get('protected_files'), dict))
        if self.barrier is not None:
            value = self.barrier[0]
            need(hashlib.sha256(raw).hexdigest() == value['configuration_sha256']
                 and isinstance(config.get('environment'), dict)
                 and config.get('environment', {}).get('source_revision') == value['head'])
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
        need(installation_barrier(self.barrier_path) == self.barrier)
        if self.armed:
            need(controller_activation(self.activation_path, self.barrier,
                 json.loads(self.raw, object_pairs_hook=unique)) == self.activation)
        inventory(self.root)
        for path, (stamp, mode) in self.stamps.items():
            need(protected(path, mode=mode) == stamp)

    def arm(self):
        """Effects require completion; passive installed Snapshot checks remain read-only."""
        self.verify()
        if not self.armed:
            self.activation = controller_activation(self.activation_path, self.barrier,
                                                   json.loads(self.raw, object_pairs_hook=unique))
            self.armed = True
        self.verify()

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
    snapshot.arm()
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
