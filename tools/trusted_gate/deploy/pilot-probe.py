#!/usr/bin/python3 -I
"""Live access checks; never read or print credential bytes, never grant delivery."""
import errno
import grp
import json
import os
from pathlib import Path
import pwd
import socket
import subprocess
import sys


def probe(role):
    expected = pwd.getpwnam('gatehouse-' + role)
    if os.getuid() != expected.pw_uid or os.geteuid() != expected.pw_uid:
        raise ValueError('probe must run as the actual restricted identity')
    groups = {grp.getgrgid(g).gr_name for g in os.getgroups()}
    if groups & {'root', 'sudo', 'docker', 'lxd', 'adm', 'disk'}:
        raise ValueError('restricted identity has privileged groups')
    results = []

    def denied(path, flags):
        try:
            fd = os.open(path, flags | os.O_CLOEXEC | os.O_NOFOLLOW)
        except OSError as error:
            # The installer first confirms these exact files exist. Masked worker
            # paths can then return ENOENT; no absent prerequisite is accepted by itself.
            if error.errno not in (errno.EACCES, errno.EPERM, errno.ENOENT):
                raise
            results.append({'path': path, 'operation': 'write' if flags & os.O_WRONLY else 'read',
                            'denied_errno': error.errno})
        else:
            os.close(fd)  # No write/truncate and no credential bytes read, even on failure.
            raise ValueError('unexpected protected-file access: ' + path)

    for path in ('/etc/gatehouse/checks.private-key.pem', '/etc/gatehouse/delivery.private-key.pem',
                 '/etc/gatehouse/host.json', '/srv/gatehouse/pilot/control/journal.sqlite3',
                 '/home/gem/.secrets/my-disposable-bot.2026-09-08.private-key.pem',
                 '/home/gem/.secrets/symphony-delivery.private-key.pem'):
        denied(path, os.O_RDONLY)
    for path in ('/opt/gatehouse/trusted_gate/host.py', '/etc/gatehouse/host.json'):
        denied(path, os.O_WRONLY)
    for path in ('/run/docker.sock', '/var/run/docker.sock', '/var/snap/lxd/common/lxd/unix.socket',
                 '/var/lib/lxd/unix.socket'):
        with socket.socket(socket.AF_UNIX) as client:
            try:
                client.connect(path)
            except OSError as error:
                if error.errno not in (errno.EACCES, errno.EPERM, errno.ENOENT, errno.ECONNREFUSED):
                    raise
                results.append({'socket': path, 'connect_errno': error.errno})
            else:
                raise ValueError('unexpected management socket access')
    if role == 'developer':
        for arguments in (['/usr/bin/id'], ['/opt/gatehouse/bin/gatehouse-control', 'step', 'pilot-boundary']):
            result = subprocess.run(['/usr/bin/sudo', '-n', *arguments], capture_output=True, timeout=15)
            if result.returncode == 0:
                raise ValueError('unexpected delegated internal/root operation')
            results.append({'sudo_command': arguments, 'exit_code': result.returncode})
    else:
        status = Path('/proc/self/status').read_text()
        if int(next(line.split()[1] for line in status.splitlines() if line.startswith('CapEff:')), 16):
            raise ValueError('worker retains effective capabilities')
        denied('/work/source/.git', os.O_RDONLY)
        denied('/work/source/.gatehouse-host/flow.toml', os.O_RDONLY)
        denied('/work/source/tools/trusted_gate/host.py', os.O_WRONLY)
        with socket.socket() as client:
            client.settimeout(1)
            if client.connect_ex(('192.0.2.1', 443)) == 0:
                raise ValueError('unexpected worker network access')
        # Query the actual namespace, independently of one destination being down.
        # ProcSubset=pid deliberately hides /proc/net from this worker.
        if {name for _, name in socket.if_nameindex()} != {'lo'}:
            raise ValueError('worker has an external network interface')
    return {'role': role, 'uid': os.getuid(), 'groups': sorted(groups), 'denials': results,
            'scope': 'live OS access probe only; not full project verification'}


if __name__ == '__main__':
    try:
        if len(sys.argv) != 2 or sys.argv[1] not in ('developer', 'executor'):
            raise ValueError('expected developer or executor role')
        print(json.dumps(probe(sys.argv[1]), sort_keys=True))
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(json.dumps({'blocked': str(error)}), file=sys.stderr)
        raise SystemExit(1)
