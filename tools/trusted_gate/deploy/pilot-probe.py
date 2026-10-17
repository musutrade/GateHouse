#!/usr/bin/python3 -I
"""Live access checks; never read or print credential bytes, never grant delivery."""
import errno
import grp
import hashlib
import json
import os
from pathlib import Path
import pwd
import re
import socket
import subprocess
import sys
import tempfile


def history_probe(head, base, source=Path('/work/source'), history=Path('/work/history')):
    if not all(re.fullmatch('[0-9a-f]{40}', value) for value in (head, base)):
        raise ValueError('history probe needs exact head/base')
    if 'GIT_DIR' in os.environ or 'GIT_WORK_TREE' in os.environ:
        raise ValueError('global Git retargeting would corrupt fixture identity')
    if (source / '.git').read_bytes() != ('gitdir: ' + str(history) + '\n').encode():
        raise ValueError('unexpected worker Git pointer')
    def git(*args, cwd=source):
        result = subprocess.run(['/usr/bin/git', *args], cwd=cwd, capture_output=True, check=True, timeout=30)
        if len(result.stdout) > 32 * 1024 * 1024:
            raise ValueError('history probe output exceeds budget')
        return result.stdout
    if git('rev-parse', 'HEAD').decode().strip() != head or git('rev-parse', 'refs/gatehouse/base').decode().strip() != base:
        raise ValueError('worker Git refs differ from frozen input')
    git('merge-base', '--is-ancestor', base, head)
    if git('status', '--porcelain', '--untracked-files=no'):
        raise ValueError('worker source/index is not clean')
    keys = set(git('config', '--local', '--name-only', '--null', '--list').decode().strip('\0').split('\0'))
    if keys != {'core.repositoryformatversion', 'core.filemode', 'core.bare', 'core.hookspath'}:
        raise ValueError('unexpected worker Git configuration')
    changes = git('diff', '--name-only', base, head)
    archive = git('archive', '--format=tar', head)
    with tempfile.TemporaryDirectory(prefix='history-probe-') as temporary:
        nested = Path(temporary)
        git('init', '--quiet', '--bare', '--template=', str(nested / 'fixture.git'))
        actual = git('--git-dir=' + str(nested / 'fixture.git'), 'rev-parse', '--absolute-git-dir').decode().strip()
        if actual != str(nested / 'fixture.git'):
            raise ValueError('fixture Git identity was redirected')
        # Also exercise ordinary discovery, without an explicit --git-dir.
        git('init', '--quiet', '--template=', str(nested / 'work'))
        actual = git('rev-parse', '--absolute-git-dir', cwd=nested / 'work').decode().strip()
        if actual != str(nested / 'work/.git'):
            raise ValueError('nested fixture Git discovery was redirected')
    return {'head': head, 'base': base, 'clean': True, 'nested_fixture_identity': True,
            'changed_paths': len(changes.splitlines()), 'archive_bytes': len(archive),
            'archive_sha256': hashlib.sha256(archive).hexdigest()}


def probe(role, head=None, base=None):
    account_role = 'executor' if role == 'executor-history' else role
    expected = pwd.getpwnam('gatehouse-' + account_role)
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
            if error.errno not in (errno.EACCES, errno.EPERM, errno.ENOENT, errno.EROFS):
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
        if role == 'executor-history':
            denied('/work/history/config', os.O_WRONLY)
            denied('/work/source/.git', os.O_WRONLY)
            denied('/srv/gatehouse/pilot/repository/mirror.git/config', os.O_RDONLY)
            history_result = history_probe(head, base)
        else:
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
    report = {'role': role, 'uid': os.getuid(), 'groups': sorted(groups), 'denials': results,
              'scope': 'live OS access/history probe only; not full project verification'}
    if role == 'executor-history':
        report['git_history'] = history_result
    return report


if __name__ == '__main__':
    try:
        if len(sys.argv) == 4 and sys.argv[1] == 'executor-history':
            report = probe(sys.argv[1], sys.argv[2], sys.argv[3])
        elif len(sys.argv) == 2 and sys.argv[1] in ('developer', 'executor'):
            report = probe(sys.argv[1])
        else:
            raise ValueError('expected developer/executor, or executor-history HEAD BASE')
        print(json.dumps(report, sort_keys=True))
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(json.dumps({'blocked': str(error)}), file=sys.stderr)
        raise SystemExit(1)
