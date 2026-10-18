"""Launch only an administrator-pinned step under a different system identity."""
from __future__ import annotations

import grp
import os
from pathlib import Path
import pwd
import subprocess
import time

from .cache import Cache, dimensions
from .common import atomic_json, digest, protected, require
from .execution import clean_environment
from .history import prepare as prepare_history


def unprivileged_identity(uid):
    require(type(uid) is int and uid > 0, 'a non-root developer/executor identity is required')
    account = pwd.getpwuid(uid)
    memberships = {group.gr_name for group in grp.getgrall() if account.pw_name in group.gr_mem}
    memberships.add(grp.getgrgid(account.pw_gid).gr_name)
    require(not memberships & {'root', 'sudo', 'docker', 'lxd', 'adm', 'disk'},
            'developer/executor identity has privileged group membership; use an isolated execution domain')
    return account


def sandbox_arguments(config, run, step_id, scratch, cache, history=None):
    step = config['steps'][step_id]
    require(step['argv'] and all(isinstance(arg, str) and '\0' not in arg for arg in step['argv']), 'invalid approved step argv')
    require(Path(step['argv'][0]).is_absolute(), 'approved executable must be absolute')
    require(step['argv'][0] in config['protected_files'], 'step executable is not pinned')
    require(type(step.get('git_history', False)) is bool, 'Git history selection must be explicit')
    require(bool(history) == step.get('git_history', False), 'reviewed Git history view is missing or unexpected')
    core_unit = 'gatehouse-core-' + run['id'] + '.service'
    unit = 'gatehouse-step-' + run['id'] + '-' + digest(step_id.encode())[:12]
    properties = {
        'User': config['executor_user'], 'UMask': '0077', 'NoNewPrivileges': 'yes',
        'CapabilityBoundingSet': '', 'AmbientCapabilities': '', 'ProtectSystem': 'strict',
        'ProtectHome': 'yes', 'PrivateTmp': 'yes', 'PrivateDevices': 'yes',
        'ProtectKernelTunables': 'yes', 'ProtectKernelModules': 'yes', 'ProtectControlGroups': 'yes',
        'ProtectKernelLogs': 'yes', 'ProtectClock': 'yes', 'RestrictSUIDSGID': 'yes',
        'RestrictNamespaces': 'yes', 'LockPersonality': 'yes', 'ProtectProc': 'invisible',
        'ProcSubset': 'pid', 'PrivateNetwork': 'yes', 'KillMode': 'control-group',
        'RuntimeMaxSec': str(step['timeout_seconds']), 'TimeoutStopSec': '15',
        'MemoryMax': str(config['executor_memory_bytes']), 'TasksMax': '512',
        'LimitFSIZE': str(config['executor_file_bytes']),
        'BindsTo': core_unit, 'PartOf': core_unit, 'After': core_unit,
        'WorkingDirectory': '/work/source',
        'BindReadOnlyPaths': config['source'] + ':/work/source',
        'BindPaths': str(scratch) + ':/work/output ' + str(cache) + ':/work/cache',
        'ReadWritePaths': '/work/output /work/cache',
        'InaccessiblePaths': '/home /root /run /etc/gatehouse ' + config['records'] +
                             ' /work/source/.gatehouse-host' + ('' if history else ' /work/source/.git'),
    }
    for field, name in (('cargo_registry', 'registry'), ('cargo_git', 'git')):
        if config.get(field):
            properties['BindReadOnlyPaths'] += ' ' + config[field] + ':/work/output/cargo-home/' + name
    if history:
        properties['BindReadOnlyPaths'] += ' ' + str(history) + ':/work/history'
        properties['BindReadOnlyPaths'] += ' ' + str(Path(history).with_name('git-pointer')) + ':/work/source/.git'
    argv = ['/usr/bin/systemd-run', '--quiet', '--wait', '--pipe', '--collect', '--unit=' + unit]
    argv.extend('--property=' + key + '=' + value for key, value in properties.items())
    environment = {'PATH': '/opt/gatehouse/toolchain/bin:/usr/local/bin:/usr/bin:/bin',
                   'LANG': 'C.UTF-8', 'TMPDIR': '/work/output', 'CARGO_HOME': '/work/output/cargo-home',
                   'CARGO_TARGET_DIR': '/work/cache/target', 'CARGO_NET_OFFLINE': 'true'}
    if history:
        environment.update(GIT_OPTIONAL_LOCKS='0', GIT_CONFIG_NOSYSTEM='1', GIT_CONFIG_GLOBAL='/dev/null',
                           GIT_CONFIG_COUNT='2', GIT_CONFIG_KEY_0='safe.directory',
                           GIT_CONFIG_VALUE_0='/work/source', GIT_CONFIG_KEY_1='safe.directory',
                           GIT_CONFIG_VALUE_1='/work/history')
    require(not any(name.startswith('GIT_') for name in step.get('env', {})),
            'Git input/configuration overrides are forbidden')
    require(not set(step.get('env', {})) & {'HOME', 'PATH', 'LD_PRELOAD', 'LD_LIBRARY_PATH',
            'PYTHONPATH', 'GIT_CONFIG_GLOBAL', 'CARGO_HOME', 'RUSTC_WRAPPER', 'GH_TOKEN',
            'GITHUB_TOKEN', 'GITHUB_APP_PRIVATE_KEY', 'SSH_AUTH_SOCK'}, 'unsafe step environment override')
    environment.update(step.get('env', {}))
    argv.extend('--setenv=' + key + '=' + value for key, value in environment.items())
    return [*argv, '--', *step['argv']]


def launch(config, journal, run_id, step_id):
    require(os.geteuid() == 0, 'sandbox launcher requires the protected system service')
    run = journal.get(run_id)
    require(run['local'] == 'running', 'step has no active trusted Core invocation')
    require(step_id in config['steps'], 'step not in reviewed policy')
    account = unprivileged_identity(pwd.getpwnam(config['executor_user']).pw_uid)
    require(account.pw_uid != 0 and account.pw_uid != config['developer_uid'], 'executor must have a separate identity')
    require(account.pw_shell in ('/usr/sbin/nologin', '/sbin/nologin', '/bin/false'), 'executor must not have a login shell')
    scratch_root = protected(config['scratch'], directory=True)
    scratch = scratch_root / (run_id + '-' + digest(step_id.encode())[:12])
    scratch.mkdir(mode=0o700)
    os.chown(scratch, account.pw_uid, account.pw_gid)
    cargo_home = scratch / 'cargo-home'
    cargo_home.mkdir(mode=0o700)
    os.chown(cargo_home, account.pw_uid, account.pw_gid)
    for field in ('cargo_registry', 'cargo_git'):
        if config.get(field):
            protected(config[field], directory=True)
    cache_config = config['cache']
    cache_root = protected(cache_config['root'], directory=True)
    cache = Cache(cache_root, cache_config['max_bytes'], cache_config['max_entries'], cache_config['ttl_seconds'])
    step = config['steps'][step_id]
    require(all(name in config['protected_files'] for name in step['toolchain_files']), 'unreviewed cache toolchain input')
    selected = dimensions(config['source'], step, config['repository'],
                          str(account.pw_uid) + ':' + str(account.pw_gid) + ':' + run['input']['policy'])
    path = cache.directory(selected)
    maintenance_started = time.monotonic()
    before = cache.maintain(active=(path.name,))
    maintenance_seconds = time.monotonic() - maintenance_started
    os.chown(path, account.pw_uid, account.pw_gid)
    history = prepare_history(config, run) if step.get('git_history', False) else None
    argv = sandbox_arguments(config, run, step_id, scratch, path, history)
    records = protected(config['records'], directory=True, private=True) / run_id / 'cache'
    records.mkdir(mode=0o700, exist_ok=True)
    record_path = records / (digest(step_id.encode()) + '.json')
    record = {'run': run_id, 'head': run['head'], 'step': step_id, 'namespace': path.name,
              'dimensions': selected, 'before': before, 'status': 'running'}
    atomic_json(record_path, record)
    # systemd, rather than Python's wait timeout, owns stop/reap of the whole worker cgroup.
    started = time.monotonic()
    result = subprocess.run(argv, env=clean_environment(), check=False)
    record['step_wall_seconds'] = time.monotonic() - started
    maintenance_started = time.monotonic()
    after = cache.maintain(active=(path.name,))
    maintenance_seconds += time.monotonic() - maintenance_started
    record.update(status='completed', exit_code=result.returncode, after=after,
                  cache_maintenance_seconds=maintenance_seconds)
    atomic_json(record_path, record)
    return result.returncode
