#!/usr/bin/python3 -I
"""Administrator-reviewed, fixed-path isolation pilot; never enable authoritative success."""
from __future__ import annotations

import argparse
import hashlib
import grp
import io
import json
import os
from pathlib import Path
import pwd
import re
import stat
import subprocess
import sys
import tarfile
import time

MODULES = ('__init__', 'cache', 'cli', 'common', 'control', 'execution', 'github', 'history',
           'host', 'isolation', 'journal', 'publication', 'runtime', 'source')
FILES = {*(f'trusted_gate/{name}.py' for name in MODULES), 'bin/harness-gate',
         'bin/gatehouse-control', 'bin/pilot-probe.py', 'policy/flow.toml',
         'policy/gatehouse-step', 'policy/audit.toml', 'policy/secrets.toml',
         'srv-gatehouse-pilot.mount', 'sudoers', 'source.bundle', 'metadata.json'}
VOLUME = Path('/srv/gatehouse/pilot')
IMAGE = Path('/var/lib/gatehouse/pilot.ext4')
MARKER = Path('/var/lib/gatehouse/pilot-install.json')
SAFE_ENV = {'PATH': '/usr/sbin:/usr/bin:/sbin:/bin', 'LANG': 'C.UTF-8',
            'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': '/dev/null', 'GIT_TERMINAL_PROMPT': '0'}


def need(condition, message):
    if not condition:
        raise ValueError(message)


def owned(path, *, private=False):
    path = Path(path)
    for item in (path, *path.parents):
        info = item.lstat()
        need(info.st_uid == 0 and not info.st_mode & 0o022 and not stat.S_ISLNK(info.st_mode),
             'unprotected administrator path: ' + str(item))
    if private:
        need(not path.stat().st_mode & 0o077, 'private file is exposed: ' + str(path))
    return path


def release(data, expected):
    need(re.fullmatch('[0-9a-f]{64}', expected) is not None and hashlib.sha256(data).hexdigest() == expected,
         'release digest differs from the reviewed value')
    payload = {}
    with tarfile.open(fileobj=io.BytesIO(data), mode='r:') as archive:
        for member in archive:
            need(member.name in FILES and member.name not in payload and member.isfile()
                 and 0 <= member.size <= 128 * 1024 * 1024, 'unexpected/duplicate/unsafe release member')
            need(sum(map(len, payload.values())) + member.size <= 256 * 1024 * 1024, 'release exceeds budget')
            payload[member.name] = archive.extractfile(member).read()
    need(set(payload) == FILES, 'release inventory is incomplete')
    meta = json.loads(payload['metadata.json'])
    need(meta['repository'] == 'musutrade/GateHouse' and re.fullmatch('[0-9a-f]{40}', meta['head'])
         and meta['coverage_acceptance'] == 'transition', 'release is not the reviewed transition pilot')
    need(meta['core_profile'] == 'release' and meta['release_incremental'] is False,
         'Core must use the reviewed non-incremental release build')
    need(meta['core_build']['sha256'] == hashlib.sha256(payload['bin/harness-gate']).hexdigest(),
         'Core bytes differ from build provenance')
    return payload, meta


def command(argv, *, capture=False):
    return subprocess.run(argv, env=SAFE_ENV, check=True, capture_output=capture, timeout=600)


def directory(path, mode=0o755):
    path = Path(path)
    if not path.exists():
        owned(path.parent)
        path.mkdir(mode=mode)
    owned(path)
    need(path.is_dir(), 'expected directory: ' + str(path))
    os.chmod(path, mode)
    return path


def install(path, data, mode=0o644):
    path = Path(path)
    owned(path.parent)
    if path.exists() or path.is_symlink():
        owned(path)
        need(path.is_file() and path.read_bytes() == data, 'existing installation differs; explicit upgrade required: ' + str(path))
    else:
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, mode)
        with os.fdopen(fd, 'wb') as output:
            output.write(data)
            output.flush()
            os.fsync(output.fileno())
    os.chmod(path, mode)


def atomic(path, value):
    path = Path(path)
    temporary = path.with_name(path.name + '.tmp')
    install(temporary, json.dumps(value, sort_keys=True, indent=2).encode() + b'\n', 0o600)
    os.replace(temporary, path)


def setup(payload, meta, digest):
    users = [pwd.getpwnam('gatehouse-developer'), pwd.getpwnam('gatehouse-executor')]
    need(users[0].pw_uid != users[1].pw_uid and all(u.pw_uid > 0 for u in users), 'separate provisioned identities required')
    for account in users:
        groups = {grp.getgrgid(g).gr_name for g in os.getgrouplist(account.pw_name, account.pw_gid)}
        need(not groups & {'root', 'sudo', 'docker', 'lxd', 'adm', 'disk'}, 'privileged restricted identity')
    need(users[1].pw_shell == '/usr/sbin/nologin', 'worker must have no login shell')
    active = subprocess.run(['/usr/bin/pgrep', '-u', ','.join(str(u.pw_uid) for u in users)],
                            env=SAFE_ENV, capture_output=True, check=False, timeout=10)
    need(active.returncode == 1, 'restricted identity has active/unknown processes; stop and reconcile before installation')
    submission_group = grp.getgrnam('gatehouse-submit')
    need(set(submission_group.gr_mem) == {users[0].pw_name}
         and not any(p.pw_gid == submission_group.gr_gid for p in pwd.getpwall()),
         'submission group has unexpected writers')
    for location in ('/opt/gatehouse', '/etc/gatehouse', '/srv/gatehouse', '/var/lib/gatehouse'):
        owned(location)
    if not MARKER.exists():
        need(not IMAGE.exists() and not VOLUME.is_mount()
             and (not VOLUME.exists() or not any(VOLUME.iterdir())), 'unregistered pilot storage requires administrator reconciliation')
    # Check credentials before modifying installed state. Bytes never enter the archive or output.
    key_sources = {'checks': Path('/home/gem/.secrets/my-disposable-bot.2026-09-08.private-key.pem'),
                   'delivery': Path('/home/gem/.secrets/symphony-delivery.private-key.pem')}
    keys = {}
    for name, path in key_sources.items():
        info = path.lstat()
        need(stat.S_ISREG(info.st_mode) and info.st_uid == pwd.getpwnam('gem').pw_uid
             and info.st_size < 32768, 'unexpected operator signing-key source')
        keys[name] = path.read_bytes()
    directory('/opt/gatehouse/bin')
    directory('/opt/gatehouse/trusted_gate')
    directory('/etc/gatehouse/policy')
    for name in MODULES:
        install('/opt/gatehouse/trusted_gate/' + name + '.py', payload['trusted_gate/' + name + '.py'])
    for name in ('harness-gate', 'gatehouse-control', 'pilot-probe.py'):
        install('/opt/gatehouse/bin/' + name, payload['bin/' + name], 0o755)
    for name in ('flow.toml', 'gatehouse-step', 'audit.toml', 'secrets.toml'):
        install('/etc/gatehouse/policy/' + name, payload['policy/' + name], 0o755 if name == 'gatehouse-step' else 0o644)
    for name, data in keys.items():
        install('/etc/gatehouse/' + name + '.private-key.pem', data, 0o600)
    # A fixed 2-GiB persistent filesystem bounds inbox, Git expansion, logs, reports,
    # scratch and cache together. No shared filesystem mount options are changed.
    directory(VOLUME)
    if MARKER.exists():
        previous = json.loads(owned(MARKER, private=True).read_text())
        need(previous['release_sha256'] == digest, 'another release owns this pilot; explicit upgrade required')
        owned(IMAGE, private=True)
        actual = command(['/usr/sbin/blkid', '-s', 'UUID', '-o', 'value', str(IMAGE)], capture=True).stdout.decode().strip()
        need(actual == previous['volume_uuid'], 'pilot volume identity differs')
    else:
        need(not IMAGE.exists(), 'unregistered pilot image exists; administrator reconciliation required')
        fd = os.open(IMAGE, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        try:
            os.ftruncate(fd, 2 * 1024 * 1024 * 1024)
        finally:
            os.close(fd)
        command(['/usr/sbin/mkfs.ext4', '-q', '-F', str(IMAGE)], capture=True)
        uuid = command(['/usr/sbin/blkid', '-s', 'UUID', '-o', 'value', str(IMAGE)], capture=True).stdout.decode().strip()
        atomic(MARKER, {'release_sha256': digest, 'head': meta['head'], 'volume_uuid': uuid})
    install('/etc/systemd/system/srv-gatehouse-pilot.mount', payload['srv-gatehouse-pilot.mount'])
    command(['/usr/bin/systemctl', 'daemon-reload'])
    command(['/usr/bin/systemctl', 'enable', '--now', 'srv-gatehouse-pilot.mount'], capture=True)
    need(VOLUME.is_mount(), 'bounded pilot filesystem is not mounted')
    mounted_uuid = command(['/usr/bin/findmnt', '-n', '-o', 'UUID', '--target', str(VOLUME)], capture=True).stdout.decode().strip()
    need(mounted_uuid == json.loads(MARKER.read_text())['volume_uuid'], 'unexpected mounted pilot volume')
    directory(VOLUME)
    for name in ('control', 'repository', 'scratch', 'cache'):
        directory(VOLUME / name, 0o700)
    # Close the old empty unbounded inbox before delegating; the pilot uses the bounded one.
    old = Path('/var/spool/gatehouse/inbox')
    if old.exists():
        info = old.lstat()
        need(stat.S_ISDIR(info.st_mode) and info.st_uid == 0 and not any(old.iterdir()), 'old inbox is not empty/root-owned')
        os.chown(old, 0, 0)
        os.chmod(old, 0o700)
    inbox = VOLUME / 'inbox'
    if not inbox.exists():
        inbox.mkdir(mode=0o700)
    info = inbox.lstat()
    need(stat.S_ISDIR(info.st_mode) and info.st_uid == 0, 'unexpected pilot inbox')
    os.chown(inbox, 0, grp.getgrnam('gatehouse-submit').gr_gid)
    os.chmod(inbox, 0o1730)
    mirror = VOLUME / 'repository/mirror.git'
    if not mirror.exists():
        command(['/usr/bin/git', '-c', 'core.hooksPath=/dev/null', 'init', '--bare', str(mirror)], capture=True)
    owned(mirror, private=True)
    steps = {
        'pilot-boundary': ['/usr/bin/python3.14', '-I', '/opt/gatehouse/bin/pilot-probe.py',
                           'executor-history', meta['head'], meta['base_at_packaging']],
        'pilot-host-contracts': ['/usr/bin/python3.14', '-B', '-m', 'unittest', 'discover', '-s',
                                 '/work/source/tools/quality/tests', '-p', 'test_trusted_gate.py', '-q']}
    dimensions = dict(repository='musutrade/GateHouse', platform='linux', architecture=os.uname().machine,
                      toolchain='resolved-from-installed-files', target='native-python', profile='pilot-contracts',
                      features='none', flags='python-no-bytecode', dependency_digest='resolved',
                      build_config_digest='resolved', instrumentation='none', release_incremental='not-applicable',
                      executor_identity='resolved')
    config = {'schema': 'gatehouse-trusted-host/v1', 'repository': 'musutrade/GateHouse', 'repository_id': 1414069625,
              'base_branch': 'main', 'profile': 'full', 'coverage_acceptance': 'transition',
              'check_name': 'GateHouse Trusted Verification', 'app_id': 4867361, 'installation_id': 159921491,
              'app_key': '/etc/gatehouse/checks.private-key.pem',
              'delivery': {'app_id': 5069731, 'installation_id': 164696480, 'app_key': '/etc/gatehouse/delivery.private-key.pem'},
              'core': '/opt/gatehouse/bin/harness-gate', 'policy_directory': '/etc/gatehouse/policy',
              'source': str(VOLUME / 'source'), 'mirror': str(mirror), 'records': str(VOLUME / 'control'),
              'scratch': str(VOLUME / 'scratch'), 'inbox': str(inbox), 'executor_user': users[1].pw_name,
              'developer_uid': users[0].pw_uid, 'executor_memory_bytes': 1073741824,
              'executor_file_bytes': 16777216, 'max_bundle_bytes': 134217728, 'max_source_files': 20000,
              'max_source_bytes': 268435456, 'timeout_seconds': 600, 'freshness_seconds': 3600,
              'environment': {'acceptance': 'isolation-pilot-only', 'source_revision': meta['head'], 'release_sha256': digest},
              'coverage': ['builtin.secret-scan', 'builtin.architecture-audit', *steps],
              'cache': {'root': str(VOLUME / 'cache'), 'max_bytes': 67108864, 'max_entries': 4, 'ttl_seconds': 86400},
              'steps': {name: {'argv': argv, 'timeout_seconds': 120, 'cache_identity': dimensions,
                              'toolchain_files': ['/usr/bin/python3.14', '/usr/bin/git'],
                              'cache_inputs': {'dependencies': [], 'configuration': []}} for name, argv in steps.items()}}
    pins = [*Path('/opt/gatehouse/trusted_gate').glob('*.py'), *Path('/opt/gatehouse/bin').iterdir(),
            *Path('/etc/gatehouse/policy').iterdir(), Path('/usr/bin/python3.14'), Path('/usr/bin/git')]
    config['protected_files'] = {str(path): hashlib.sha256(owned(path).read_bytes()).hexdigest() for path in pins}
    config['steps']['pilot-boundary']['git_history'] = True
    install('/etc/gatehouse/host.json', json.dumps(config, sort_keys=True, indent=2).encode() + b'\n', 0o600)
    # Syntax is checked before installing the single narrowly delegated entry.
    candidate = Path('/etc/gatehouse/pilot.sudoers')
    install(candidate, payload['sudoers'], 0o440)
    command(['/usr/sbin/visudo', '-cf', str(candidate)], capture=True)
    install('/etc/sudoers.d/gatehouse-pilot', payload['sudoers'], 0o440)
    bundle = inbox / 'candidate.bundle'
    if not bundle.exists():
        fd = os.open(bundle, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        with os.fdopen(fd, 'wb') as output:
            output.write(payload['source.bundle'])
        os.chown(bundle, users[0].pw_uid, users[0].pw_gid)
    sys.path.insert(0, '/opt/gatehouse')
    from trusted_gate.common import read_submission
    captured = read_submission(bundle, inbox, users[0].pw_uid, config['max_bundle_bytes'])
    need(hashlib.sha256(captured).digest() == hashlib.sha256(payload['source.bundle']).digest(), 'pilot bundle changed')
    return config


def accept(config, meta, digest):
    sys.path.insert(0, '/opt/gatehouse')
    from trusted_gate.github import GitHub
    from trusted_gate.journal import Journal
    journal = Journal(VOLUME / 'control/journal.sqlite3')
    journal.close()
    for settings in (config, config['delivery']):
        client = GitHub(settings['app_id'], settings['installation_id'], settings['app_key'], config['repository'],
                        permissions={'contents': 'read', 'pull_requests': 'read'})
        need(client.repo('GET', '')['id'] == config['repository_id'], 'App cannot independently read GateHouse')
    report = {'head': meta['head'], 'release_sha256': digest, 'scope': 'isolation pilot only; not complete issue98',
              'coverage_acceptance': 'transition', 'started': time.time()}
    result = command(['/usr/sbin/runuser', '-u', 'gatehouse-developer', '--', '/usr/bin/python3.14', '-I',
                      '/opt/gatehouse/bin/pilot-probe.py', 'developer'], capture=True)
    report['developer'] = json.loads(result.stdout)
    validate = subprocess.run(['/usr/sbin/runuser', '-u', 'gatehouse-developer', '--', '/usr/bin/sudo', '-n',
        '/opt/gatehouse/bin/gatehouse-control', 'validate', '--bundle', config['inbox'] + '/candidate.bundle',
        '--head', meta['head']], env=SAFE_ENV, capture_output=True, timeout=660)
    attempt = str(time.time_ns())
    install(VOLUME / ('control/pilot-validation-' + attempt + '.stdout'), validate.stdout, 0o600)
    install(VOLUME / ('control/pilot-validation-' + attempt + '.stderr'), validate.stderr, 0o600)
    need(validate.returncode == 0, 'pilot validation blocked; retain private pilot-validation stdout/stderr and Core log for diagnosis')
    run = json.loads(validate.stdout)
    need(run['local'] == 'passed' and run['head'] == meta['head'], 'no accepted protected pilot execution')
    report['run'] = run['id']
    need(config['steps']['pilot-boundary'].get('git_history') is True,
         'history pilot did not select the protected Git view')
    report['worker_git_history'] = 'passed within the protected Core boundary step'
    rejected = subprocess.run(['/usr/sbin/runuser', '-u', 'gatehouse-developer', '--', '/usr/bin/sudo', '-n',
        '/opt/gatehouse/bin/gatehouse-control', 'publish', run['id']], env=SAFE_ENV, capture_output=True, timeout=30)
    need(rejected.returncode != 0 and b'coverage migration is not accepted' in rejected.stderr,
         'transition pilot unexpectedly permitted authoritative publication')
    # Exercise the real App and recovery identity with a separately named negative Check.
    # This never grants success and is not installed as a required branch rule.
    client = GitHub(config['app_id'], config['installation_id'], config['app_key'], config['repository'])
    probe_run = {'id': 'isolation-pilot-' + digest[:32], 'head': meta['head'], 'check_id': None, 'previous_id': None}
    name = 'GateHouse Host Isolation Pilot (negative)'
    existing = client.find_check(probe_run, name)
    if not (existing and existing['status'] == 'completed' and existing['conclusion'] == 'failure'):
        probe_run['check_id'] = client.update_check(probe_run, name, 'in_progress', summary='Isolation pilot; complete project coverage is pending.')
        client.update_check(probe_run, name, 'completed', 'failure',
                            'Expected negative acceptance: transition configuration cannot authorize delivery. Existing required CI remains unchanged.')
    check = client.find_check(probe_run, name)
    need(check['conclusion'] == 'failure', 'negative App Check was not read back')
    report.update(finished=time.time(), authoritative_publish='blocked', negative_check=check['id'],
                  negative_conclusion='failure', production_gate_enabled=False)
    atomic(VOLUME / 'control/pilot-acceptance.json', report)
    return report


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--release', type=Path, required=True)
    parser.add_argument('--sha256', required=True)
    args = parser.parse_args(argv)
    need(os.geteuid() == 0, 'administrator authentication required; no alternate elevation is attempted')
    os.umask(0o077)
    archive = owned(args.release, private=True)
    need(archive.is_file() and archive.stat().st_size <= 256 * 1024 * 1024, 'release archive exceeds budget')
    payload, meta = release(archive.read_bytes(), args.sha256)
    config = setup(payload, meta, args.sha256)
    print(json.dumps(accept(config, meta, args.sha256), sort_keys=True))
    return 0


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError, tarfile.TarError) as error:
        print(json.dumps({'status': 'blocked', 'reason': str(error), 'production_gate_enabled': False}), file=sys.stderr)
        raise SystemExit(1)
