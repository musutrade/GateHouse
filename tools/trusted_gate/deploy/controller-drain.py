"""Bounded pre-install process observations under a fenced ingress; never kill or activate."""
from __future__ import annotations

import ctypes
import hashlib
import json
import os
from pathlib import Path
import stat
import sys
import time

PROC = Path('/proc')
MAX_PIDS = 4096
MAX_FILE = 64 * 1024
MAX_TOTAL = 64 * 1024**2
MAX_SECONDS = 10
KTHREAD = 0x00200000
MANAGED = (b'/opt/gatehouse', b'/etc/gatehouse', b'/srv/gatehouse/pilot/source',
           b'/srv/gatehouse/pilot/repository', b'/srv/gatehouse/linux/bootstrap-source/source',
           b'/srv/gatehouse/linux/bootstrap-source/mirror.git')
UNITS = (b'gatehouse-core-', b'gatehouse-step-', b'gatehouse-bootstrap-')


def need(value):
    if not value:
        raise ValueError('controller process observation is unavailable, changed or unsafe')


def number(raw, maximum=2**64-1):
    need(raw and len(raw) <= 20 and raw.isdigit())
    value = int(raw);need(value <= maximum)
    return value


def process_stat(raw, pid):
    need(0 < len(raw) <= MAX_FILE and b' (' in raw and b') ' in raw)
    left, right = raw.rsplit(b') ', 1)
    observed, comm = left.split(b' (', 1)
    fields = right.split()
    need(number(observed) == pid and len(fields) >= 20 and len(comm) <= 64
         and fields[0] in (b'R', b'S', b'D', b'Z', b'T', b't', b'X', b'I', b'P'))
    return {'pid': pid, 'state': fields[0], 'ppid': number(fields[1], 2**32-1),
            'flags': number(fields[6]), 'start': number(fields[19])}


def process_status(raw, pid):
    need(0 < len(raw) <= MAX_FILE)
    keys = {b'Pid', b'Tgid', b'PPid', b'Uid', b'Gid', b'NSpid'}
    selected = {}
    for line in raw.splitlines():
        key, colon, value = line.partition(b':')
        if colon and key in keys:
            need(key not in selected);selected[key] = value.split()
    need(selected.keys() == keys and len(selected[b'Uid']) == len(selected[b'Gid']) == 4
         and 1 <= len(selected[b'NSpid']) <= 32
         and all(len(selected[key]) == 1 for key in (b'Pid', b'Tgid', b'PPid')))
    values = {key: tuple(number(value, 2**32-1) for value in items) for key, items in selected.items()}
    need(values[b'Pid'] == values[b'Tgid'] == (pid,) and values[b'NSpid'][0] == pid)
    return values


def sensitive(argv, cwd, executable, cgroup):
    # Bytes avoid lossy argv decoding; never log argv, environment or raw paths.
    fields = (*argv, cwd, executable)
    paths = any(prefix in value for value in fields for prefix in MANAGED)
    units = any(value.split(b'=')[-1].startswith(UNITS) for value in argv)
    units = units or any(part.startswith(UNITS) for part in cgroup.split(b'/'))
    return paths or units


def stat_identity(value):
    return value['pid'], value['ppid'], value['flags'], value['start']


class ProcView:
    def __init__(self, root=PROC):
        self.root = Path(root)
        self.total = 0
        self.deadline = time.monotonic() + MAX_SECONDS

    def budget(self, count=0):
        self.total += count
        need(self.total <= MAX_TOTAL and time.monotonic() <= self.deadline)

    def pids(self):
        values = []
        for path in self.root.iterdir():
            self.budget()
            if path.name.isascii() and path.name.isdigit():
                need(path.name == str(int(path.name)) and 0 < int(path.name) <= 2**32-1)
                values.append(int(path.name));need(len(values) <= MAX_PIDS)
        return sorted(values)

    def read(self, fd, name):
        self.budget()
        value = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC, dir_fd=fd)
        with os.fdopen(value, 'rb') as stream:
            raw = stream.read(MAX_FILE+1)
        self.budget(len(raw));need(len(raw) <= MAX_FILE)
        return raw

    def link(self, fd, name):
        self.budget()
        value = os.fsencode(os.readlink(name, dir_fd=fd))
        self.budget(len(value));need(0 < len(value) <= MAX_FILE)
        return value

    def observe(self, pid):
        # An opened proc PID directory prevents following a recycled numeric PID.
        fd = os.open(self.root / str(pid), os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        try:
            first = process_stat(self.read(fd, 'stat'), pid)
            status = process_status(self.read(fd, 'status'), pid)
            need(first['ppid'] == status[b'PPid'][0])
            privileged = 0 in status[b'Uid']
            restricted = bool({995, 999} & set(status[b'Uid']))
            dead = first['state'] in (b'Z', b'X')
            kernel = bool(first['flags'] & KTHREAD)
            markers = False
            observation = b''
            if not dead and (privileged or restricted):
                if kernel:
                    need(privileged and status[b'Uid'] == (0, 0, 0, 0)
                         and not restricted and not self.read(fd, 'cmdline'))
                else:
                    argv_raw = self.read(fd, 'cmdline')
                    need(argv_raw and argv_raw.endswith(b'\0'))
                    argv = argv_raw[:-1].split(b'\0')
                    cwd, executable, cgroup = self.link(fd, 'cwd'), self.link(fd, 'exe'), self.read(fd, 'cgroup')
                    need(cgroup and len(cgroup.splitlines()) <= 64)
                    markers = sensitive(argv, cwd, executable, cgroup)
                    observation = hashlib.sha256(argv_raw + b'\0' + cwd + b'\0' + executable + b'\0' + cgroup).digest()
                    # exec, credential and path changes in a PID invalidate this observation.
                    need(self.read(fd, 'cmdline') == argv_raw and self.link(fd, 'cwd') == cwd
                         and self.link(fd, 'exe') == executable and self.read(fd, 'cgroup') == cgroup)
            second_status = process_status(self.read(fd, 'status'), pid)
            second = process_stat(self.read(fd, 'stat'), pid)
            need(stat_identity(first) == stat_identity(second) and status == second_status
                 and (first['state'] in (b'Z', b'X')) == (second['state'] in (b'Z', b'X')))
            return {'pid': pid, 'ppid': first['ppid'], 'start': first['start'], 'uids': list(status[b'Uid']),
                    'dead': dead, 'kernel': kernel, 'managed': markers,
                    'restricted': restricted, 'identity_sha256': observation.hex()}
        finally:
            os.close(fd)

    def scan(self, caller):
        before = self.pids()
        need(caller in before)
        values = [self.observe(pid) for pid in before]
        need(self.pids() == before)
        own = next(value for value in values if value['pid'] == caller)
        need(own['uids'] == [0, 0, 0, 0] and not own['dead'] and not own['kernel'])
        need(not any(value['pid'] != caller and not value['dead']
                     and (value['restricted'] or value['managed']) for value in values))
        return values


def proc_context():
    need(os.geteuid() == 0 and os.uname().machine == 'x86_64')
    info = PROC.lstat()
    need(stat.S_ISDIR(info.st_mode) and info.st_uid == 0 and not info.st_mode & 0o022)
    fd = os.open(PROC, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
    try:
        # statfs begins with f_type on this fixed Linux/x86_64 host. The buffer
        # exceeds the native struct size; no process-selected ABI or shell command.
        buffer = ctypes.create_string_buffer(256)
        call = ctypes.CDLL(None, use_errno=True).fstatfs
        call.argtypes = (ctypes.c_int, ctypes.c_void_p);call.restype = ctypes.c_int
        need(call(fd, buffer) == 0 and int.from_bytes(buffer.raw[:ctypes.sizeof(ctypes.c_long)],
                                                    sys.byteorder, signed=True) == 0x9fa0)
    finally:
        os.close(fd)
    namespaces = {}
    for name in ('pid', 'user', 'mnt'):
        own = os.readlink(PROC / 'self/ns' / name)
        need(own == os.readlink(PROC / '1/ns' / name))
        namespaces[name] = own
    with (PROC / 'self/uid_map').open('rb') as stream:
        mapping = stream.read(4097)
    need(len(mapping) <= 4096 and mapping.split() == [b'0', b'0', b'4294967295'])
    return namespaces


def observe(B, helpers, manifest, manifest_pin, lease_fd):
    """Authenticate and reobserve a completed fence; write no receipt or drain claim."""
    B.need(os.geteuid() == 0, 'administrator identity required for controller observation')
    code = B.authenticate(helpers, manifest, manifest_pin)
    fence = B.load(helpers, 'install-transaction.py')
    builder = B.load(helpers, 'build-protected-core.py')
    before = fence.fenced(B, lease_fd)
    context = proc_context()
    builder.quiet_units(B)
    view = ProcView()
    first = view.scan(os.getpid())
    B.need(fence.fenced(B, lease_fd) == before, 'entry fence changed during process observation')
    second = view.scan(os.getpid())
    B.need([(row['pid'], row['start']) for row in first] == [(row['pid'], row['start']) for row in second]
           and proc_context() == context, 'process identities or namespace changed')
    builder.quiet_units(B)
    B.need(fence.fenced(B, lease_fd) == before, 'entry fence changed after process observation')
    B.authenticate(helpers, manifest, manifest_pin)
    return {'schema': 'gatehouse-controller-observation/v1', 'state': 'observed-empty', 'head': before['head'],
            'helper_revision': code['revision'], 'helper_manifest_sha256': manifest_pin,
            'transaction': before['transaction'], 'namespaces': context, 'scans': 2,
            'processes': len(second), 'observation_sha256': hashlib.sha256(B.canonical(second)).hexdigest(),
            'controller_drain_proven': False, 'administrator_ready': False, 'production_gate_enabled': False,
            'scope': 'read-only pre-install process observations only; recheck under continuous lease before every transition'}


if __name__ == '__main__':
    raise SystemExit('controller observation is an internal library; no administrator entry is available')
