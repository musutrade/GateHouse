"""Observe the fixed accepted 659 pilot; never validate, revoke, retire or install."""
from __future__ import annotations

import fcntl
import grp
import hashlib
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
from types import ModuleType

STAGE = Path('/root/gatehouse-issue98-history-659803a')
VOLUME = Path('/srv/gatehouse/pilot')
IMAGE = Path('/var/lib/gatehouse/pilot.ext4')
CONFIG = Path('/etc/gatehouse/host.json')
MARKER = Path('/var/lib/gatehouse/pilot-install.json')
INSTALLER = '00b7f3db58332c0fb5785d29ace1f524679087a241a4318e937c075add6a2690'
UPGRADER = '631e8bc5c91c8c3c537333882107cc595e267fb77d2600c515fb916827b12c58'
HEAD = '659803a2f144926997e56d072c7fe8760d6ed176'
RUN = 'ce86a2a4-ff18-435f-bb5d-aafc313ceca6'
RELEASE = '04190f57c76f9625be1e805ab212c48802181f458fecdc34f5478f796efd77ef'
NEGATIVE_CHECK = 117423517761
REPOSITORY = 'musutrade/GateHouse'
REPOSITORY_ID = 1414069625
KEY_SOURCES = {'checks': Path('/home/gem/.secrets/my-disposable-bot.2026-09-08.private-key.pem'),
               'delivery': Path('/home/gem/.secrets/symphony-delivery.private-key.pem')}


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def value(B, path, limit=1024**2):
    raw = B.read(B.owned(path, private=True), limit)
    return raw, json.loads(raw, object_pairs_hook=B.unique)


def member_name(destination):
    if destination.startswith('/opt/gatehouse/'):
        return destination.removeprefix('/opt/gatehouse/')
    if destination.startswith('/etc/gatehouse/policy/'):
        return 'policy/' + Path(destination).name
    if destination == '/etc/systemd/system/srv-gatehouse-pilot.mount':
        return 'srv-gatehouse-pilot.mount'
    if destination == '/etc/sudoers.d/gatehouse-pilot':
        return 'sudoers'
    raise ValueError('unexpected accepted installed destination')


def accepted_release(B, contract):
    """Hash all old stage inputs before parsing; execute none of the old installer."""
    references = {}
    for name, pin in (('install-pilot.py', INSTALLER), ('upgrade-history-pilot.py', UPGRADER)):
        raw = B.read(B.owned(STAGE / name, private=True), 1024**2)
        B.need(digest(raw) == pin, 'accepted stage script changed')
        references[contract.DEPLOY + name] = raw
    raw = B.read(B.owned(STAGE / 'release.tar', private=True), 256 * 1024**2)
    B.need(digest(raw) == RELEASE, 'accepted stage release changed')
    mapping = contract.old_source_map()
    names = {member_name(row[0]) for row in mapping.values()} | {
        'bin/harness-gate', 'source.bundle', 'metadata.json'}
    payload = {}
    with tarfile.open(fileobj=io.BytesIO(raw), mode='r:') as archive:
        for item in archive:
            B.need(item.name in names and item.name not in payload and item.isfile()
                   and not item.issparse() and 0 <= item.size <= 128 * 1024**2,
                   'accepted stage member differs')
            payload[item.name] = archive.extractfile(item).read()
    B.need(payload.keys() == names and len(names) == 24, 'accepted 659 inventory differs')
    meta = json.loads(payload['metadata.json'], object_pairs_hook=B.unique)
    B.need(meta['repository'] == REPOSITORY and meta['head'] == HEAD
           and meta['base_at_packaging'] == contract.BASE and meta['coverage_acceptance'] == 'transition'
           and meta['core_profile'] == 'release' and meta['release_incremental'] is False
           and meta['worker_git_history_probe'] is True
           and meta['core_build']['sha256'] == digest(payload['bin/harness-gate']),
           'accepted release metadata differs')
    references.update({source: payload[member_name(row[0])] for source, row in mapping.items()})
    B.need(digest(B.canonical(contract.rows(references))) == contract.OLD_INPUTS,
           'accepted 659 source references differ')
    return payload


def accepted_pins(B, contract, payload, builder):
    """Expected pins from the authenticated old release, independent of installed paths."""
    pins = {destination: digest(payload[member_name(destination)])
            for destination, mode, action in contract.old_source_map().values()
            if destination.startswith(('/opt/gatehouse/', '/etc/gatehouse/policy/'))}
    pins['/opt/gatehouse/bin/harness-gate'] = digest(payload['bin/harness-gate'])
    for path in ('/usr/bin/python3.14', '/usr/bin/git'):
        pins[path] = digest(builder.host_file(path))
    return pins


def installed(B, contract, payload, builder):
    expected = {}
    for source, (destination, mode, action) in contract.old_source_map().items():
        expected[destination] = (payload[member_name(destination)], mode)
    expected['/opt/gatehouse/bin/harness-gate'] = (payload['bin/harness-gate'], 0o755)
    expected['/etc/gatehouse/pilot.sudoers'] = (payload['sudoers'], 0o440)
    for destination, (raw, mode) in expected.items():
        path = B.owned(destination)
        B.need(stat.S_IMODE(path.stat().st_mode) == mode and B.read(path, 128 * 1024**2) == raw,
               'installed accepted input differs')
    # Never import these files or their bytecode. Root-generated caches are data
    # here; unknown modules, unsafe cache types and delegated writes still block.
    modules = B.owned('/opt/gatehouse/trusted_gate', directory=True)
    B.need({p.name for p in modules.iterdir()} <= {name + '.py' for name in contract.OLD_MODULES} | {'__pycache__'},
           'unknown installed controller module')
    cache = modules / '__pycache__'
    if cache.exists() or cache.is_symlink():
        B.owned(cache, directory=True)
        entries = list(cache.iterdir())
        B.need(len(entries) <= 3 * len(contract.OLD_MODULES), 'installed bytecode exceeds budget')
        for path in entries:
            match = re.fullmatch(r'([a-z_]+)\.cpython-314(?:\.opt-[12])?\.pyc', path.name)
            B.need(match and match[1] in contract.OLD_MODULES, 'unknown installed bytecode')
            B.read(B.owned(path), 1024**2)
    for directory, names in (('/opt/gatehouse', {'trusted_gate', 'bin'}),
                              ('/opt/gatehouse/bin', {'harness-gate', 'gatehouse-control', 'pilot-probe.py'}),
                              ('/etc/gatehouse/policy', {'flow.toml', 'audit.toml', 'secrets.toml', 'gatehouse-step'})):
        B.need({p.name for p in B.owned(directory, directory=True).iterdir()} == names,
               'installed code/policy inventory differs')
    return accepted_pins(B, contract, payload, builder)


def accounts(B):
    expected = [('gatehouse-developer', 999, 982, '/srv/gatehouse/developer', '/bin/bash',
                 {'gatehouse-developer', 'gatehouse-submit'}),
                ('gatehouse-executor', 995, 981, '/nonexistent', '/usr/sbin/nologin', {'gatehouse-executor'})]
    for name, uid, gid, home, shell, groups in expected:
        account = pwd.getpwnam(name)
        actual = {grp.getgrgid(g).gr_name for g in os.getgrouplist(name, account.pw_gid)}
        B.need((account.pw_uid, account.pw_gid, account.pw_dir, account.pw_shell) == (uid, gid, home, shell)
               and actual == groups and pwd.getpwuid(uid).pw_name == name, 'restricted identity changed')
        group = grp.getgrnam(name)
        B.need(group.gr_gid == gid and grp.getgrgid(gid).gr_name == name
               and set(group.gr_mem) <= {name}
               and {p.pw_name for p in pwd.getpwall() if p.pw_gid == gid} == {name},
               'restricted primary group has another identity')
    submit = grp.getgrnam('gatehouse-submit')
    B.need(submit.gr_gid == 980 and set(submit.gr_mem) == {'gatehouse-developer'}
           and grp.getgrgid(980).gr_name == 'gatehouse-submit'
           and not any(p.pw_gid == 980 for p in pwd.getpwall()), 'submission group changed')
    result = subprocess.run(['/usr/bin/pgrep', '-u', '999,995'], env=B.ENV, capture_output=True, timeout=10)
    B.need(result.returncode == 1 and not result.stdout and not result.stderr,
           'restricted identities have active or unknown processes')


def keys(B):
    operator = pwd.getpwnam('gem').pw_uid
    pins = {}
    for name, path in KEY_SOURCES.items():
        for parent in (path, *path.parents):
            info = parent.lstat()
            B.need(info.st_uid in (0, operator) and not stat.S_ISLNK(info.st_mode)
                   and not info.st_mode & 0o022, 'operator key path changed')
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        with os.fdopen(fd, 'rb') as stream:
            before = os.fstat(stream.fileno())
            B.need(stat.S_ISREG(before.st_mode) and before.st_uid == operator and before.st_nlink == 1
                   and not before.st_mode & 0o077 and 0 < before.st_size <= 32768, 'operator key source changed')
            raw = stream.read(32769)
            def identity(info):
                return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid, info.st_nlink,
                        info.st_size, info.st_mtime_ns, info.st_ctime_ns)
            B.need(len(raw) == before.st_size and identity(before) == identity(os.fstat(stream.fileno()))
                   == identity(path.lstat()), 'operator key changed while reading')
        copied = B.owned('/etc/gatehouse/' + name + '.private-key.pem', private=True)
        B.need(B.read(copied, 32768) == raw, 'key rotation requires separate reconciliation')
        pins[name] = digest(raw)
    return pins


def command(B, argv):
    result = subprocess.run(argv, env=B.ENV, capture_output=True, check=True, timeout=10)
    B.need(len(result.stdout) <= 256 * 1024 and not result.stderr, 'ambiguous host observation')
    return json.loads(result.stdout, object_pairs_hook=B.unique)


def mount_identity(B, marker):
    B.owned(VOLUME, directory=True)
    image = B.owned(IMAGE, private=True)
    info = image.stat()
    B.need(info.st_size == 2 * 1024**3, 'accepted pilot image size differs')
    rows = command(B, ['/usr/bin/findmnt', '--json', '--list', '--submounts', '--mountpoint', str(VOLUME),
        '--output', 'TARGET,SOURCE,FSTYPE,OPTIONS,UUID,FSROOT,MAJ:MIN'])['filesystems']
    B.need(len(rows) == 1, 'pilot mount has nested or unknown filesystems')
    row = rows[0]
    B.need(row['target'] == str(VOLUME) and row['fstype'] == 'ext4' and row['fsroot'] == '/'
           and row['uuid'] == marker['volume_uuid'] and not row.get('children')
           and {'rw', 'nodev', 'nosuid', 'noatime'} <= set(row['options'].split(','))
           and not {'ro', 'dev', 'suid', 'noexec'} & set(row['options'].split(','))
           and re.fullmatch('/dev/loop[0-9]+', row['source']), 'accepted pilot mount differs')
    loops = command(B, ['/usr/sbin/losetup', '--json', '--list', '--output',
                        'NAME,BACK-FILE,BACK-INO,BACK-MAJ:MIN,OFFSET,SIZELIMIT,RO,MAJ:MIN'])['loopdevices']
    matches = [item for item in loops if item['back-file'] == str(IMAGE) or item['name'] == row['source']
               or (item['back-ino'] == info.st_ino and item['back-maj:min'] ==
                   str(os.major(info.st_dev)) + ':' + str(os.minor(info.st_dev)))]
    B.need(len(matches) == 1, 'pilot loop device has aliases or unknown backing')
    loop = matches[0]
    B.need(loop['name'] == row['source'] and loop['back-file'] == str(IMAGE)
           and loop['maj:min'] == row['maj:min'] and type(loop['back-ino']) is int and loop['back-ino'] == info.st_ino
           and loop['back-maj:min'] == str(os.major(info.st_dev)) + ':' + str(os.minor(info.st_dev))
           and type(loop['offset']) is int and loop['offset'] == 0
           and type(loop['sizelimit']) is int and loop['sizelimit'] == 0
           and loop['ro'] is False, 'pilot backing image differs')
    aliases = command(B, ['/usr/bin/findmnt', '--json', '--source', row['source'], '--output', 'TARGET,FSROOT'])
    B.need(aliases['filesystems'] == [{'target': str(VOLUME), 'fsroot': '/'}], 'pilot image has another mount alias')
    device = Path(row['source']).stat()
    B.need(stat.S_ISBLK(device.st_mode) and device.st_rdev == VOLUME.stat().st_dev,
           'mounted pilot block device differs')
    return digest(B.canonical({'mount': row, 'loop': loop}))


def journal(B, keep, config):
    path = VOLUME / 'control/journal.sqlite3'
    for item in (path, path.with_name(path.name + '-wal'), path.with_name(path.name + '-shm')):
        if item.exists() or item.is_symlink():
            B.need(B.owned(item, private=True).stat().st_size <= keep.MAX_DB, 'journal exceeds budget')
    B.owned(path, private=True)
    # Never create a missing WAL shared-memory sidecar during observation.
    B.need(not path.with_name(path.name + '-wal').exists() or path.with_name(path.name + '-shm').exists(),
           'WAL observation requires existing shared-memory state')
    database = keep.connect(path)
    deadline = time.monotonic() + 10
    database.set_progress_handler(lambda: int(time.monotonic() > deadline), 1000)
    try:
        database.execute('BEGIN')
        identity, rows = keep.database_identity(B, database)
        run = keep.old_run(B, rows)
    finally:
        database.close()
    inputs = json.loads(run['input'], object_pairs_hook=B.unique)
    policy = json.dumps(config, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()
    B.need(inputs['policy'] == digest(policy), 'accepted host policy changed')
    B.need(run['head'] == HEAD and run['id'] == RUN, 'accepted run differs')
    return identity, run, inputs


def configuration(B, config, marker, accepted, pins, contract):
    B.need(config['schema'] == 'gatehouse-trusted-host/v1' and config['repository'] == REPOSITORY
           and type(config['repository_id']) is int and config['repository_id'] == REPOSITORY_ID
           and config['base_branch'] == 'main' and config['coverage_acceptance'] == 'transition'
           and config['profile'] == 'full' and config['developer_uid'] == 999
           and config['executor_user'] == 'gatehouse-executor' and config['protected_files'] == pins
           and config['core'] == '/opt/gatehouse/bin/harness-gate'
           and config['policy_directory'] == '/etc/gatehouse/policy'
           and config['environment']['source_revision'] == HEAD
           and config['environment']['release_sha256'] == RELEASE, 'accepted configuration differs')
    for key, relative in (('source', 'source'), ('mirror', 'repository/mirror.git'), ('records', 'control'),
                          ('inbox', 'inbox'), ('scratch', 'scratch')):
        B.need(config[key] == str(VOLUME / relative), 'accepted storage configuration differs')
    B.need(marker['head'] == accepted['head'] == HEAD and accepted['run'] == RUN
           and marker['release_sha256'] == accepted['release_sha256'] == RELEASE
           and accepted['coverage_acceptance'] == 'transition' and accepted['production_gate_enabled'] is False
           and accepted['authoritative_publish'] == 'blocked' and accepted['negative_conclusion'] == 'failure'
           and type(accepted['negative_check']) is int and accepted['negative_check'] == NEGATIVE_CHECK
           and accepted['worker_git_history'] == 'passed within the protected Core boundary step',
           'accepted pilot marker/report differs')
    B.need((config['app_id'], config['installation_id'], config['app_key']) ==
           (4867361, 159921491, '/etc/gatehouse/checks.private-key.pem')
           and config['delivery'] == {'app_id': 5069731, 'installation_id': 164696480,
                                     'app_key': '/etc/gatehouse/delivery.private-key.pem'}, 'accepted App identity differs')


def frozen(B, inputs, payload, contract):
    B.need(inputs['repository'] == REPOSITORY and inputs['repository_id'] == REPOSITORY_ID
           and inputs['base'] == contract.BASE, 'accepted repository/base differs')
    raw, freeze = value(B, VOLUME / 'repository/frozen.json')
    bundle = B.read(B.owned(VOLUME / 'repository/candidate.bundle', private=True), 128 * 1024**2)
    B.need(bundle == payload['source.bundle'] and freeze == {'repository': REPOSITORY,
           'repository_id': REPOSITORY_ID, 'head': HEAD, 'base': contract.BASE,
           'bundle_sha256': digest(bundle)}, 'accepted frozen bundle differs')
    source = VOLUME / 'source'
    lock = B.frozen_lock(source, HEAD, contract.BASE)
    B.need(B.git(source, 'rev-parse', 'HEAD^{tree}').decode().strip() == inputs['tree']
           and not B.git(source, 'status', '--porcelain', '--untracked-files=no'), 'accepted source changed')
    return digest(raw + lock + bundle)


def client_class(payload):
    """Load only two already authenticated old source blobs, without .pyc or sys.path."""
    package_name = '_gatehouse_preflight_readonly'
    names = [package_name, package_name + '.common', package_name + '.github']
    if any(name in sys.modules for name in names):
        raise ValueError('preflight client namespace already occupied')
    package = ModuleType(package_name)
    package.__path__ = []
    sys.modules[package_name] = package
    try:
        for name in ('common', 'github'):
            module = ModuleType(package_name + '.' + name)
            module.__package__ = package_name
            sys.modules[module.__name__] = module
            exec(compile(payload['trusted_gate/' + name + '.py'], '<accepted659-' + name + '>', 'exec'), module.__dict__)
        return sys.modules[package_name + '.github'].GitHub
    finally:
        for name in names:
            sys.modules.pop(name, None)


def remote(B, config, payload, contract, *, api_proxy=None, transport=None):
    client = client_class(payload)(config['app_id'], config['installation_id'], config['app_key'],
                                   config['repository'], permissions={'contents': 'read'})
    try:
        if api_proxy is not None:
            B.need(callable(transport), 'authenticated explicit API transport required')
            transport(B, client, api_proxy)
        repository = client.repo('GET', '')
        branch = client.repo('GET', '/git/ref/heads/main')
        B.need(type(repository['id']) is int and repository['id'] == REPOSITORY_ID
               and repository['full_name'] == REPOSITORY and repository['default_branch'] == 'main'
               and branch['ref'] == 'refs/heads/main' and branch['object']['type'] == 'commit'
               and branch['object']['sha'] == contract.BASE, 'current repository/base differs')
    finally:
        client.token, client.expires = None, 0
    return {'repository_id': REPOSITORY_ID, 'base': contract.BASE, 'permissions': {'contents': 'read'}}


def observe(B, helpers, manifest, manifest_pin, lock_fd=None, *, api_proxy=None):
    B.need(os.geteuid() == 0, 'administrator authentication required for pilot preflight')
    code = B.authenticate(helpers, manifest, manifest_pin)
    contract = B.load(helpers, 'bootstrap-release.py')
    keep = B.load(helpers, 'preserve-pilot-state.py')
    builder = B.load(helpers, 'build-protected-core.py')
    control = B.owned(VOLUME / 'control', directory=True, private=True)
    lease = B.owned(control / 'controller.lock', private=True)
    own_lease = lock_fd is None
    if own_lease:
        lock_fd = os.open(lease, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        actual, expected = os.fstat(lock_fd), lease.stat()
        B.need((actual.st_dev, actual.st_ino) == (expected.st_dev, expected.st_ino), 'preflight lease differs')
        fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        payload = accepted_release(B, contract)
        def snapshot():
            accounts(B)
            builder.quiet_units(B)
            pins = installed(B, contract, payload, builder)
            raw = {name: value(B, path) for name, path in {
                'config': CONFIG, 'marker': MARKER, 'acceptance': control / 'pilot-acceptance.json'}.items()}
            config, marker, accepted = (raw[name][1] for name in ('config', 'marker', 'acceptance'))
            configuration(B, config, marker, accepted, pins, contract)
            key_pins = keys(B)
            identity, run, inputs = journal(B, keep, config)
            return config, {'metadata': {name: digest(item[0]) for name, item in raw.items()},
                            'journal': identity, 'source': frozen(B, inputs, payload, contract),
                            'mount': mount_identity(B, marker), 'keys': key_pins}
        config, before = snapshot()
        if api_proxy is None:
            observed = remote(B, config, payload, contract)
        else:
            effects = B.load(helpers, 'admit-upgrade-effects.py')
            observed = remote(B, config, payload, contract, api_proxy=api_proxy, transport=effects.transport)
        B.need(snapshot()[1] == before, 'pilot changed during preflight')
        B.need(accepted_release(B, contract) == payload, 'accepted stage changed during preflight')
        B.authenticate(helpers, manifest, manifest_pin)
        return {'schema': 'gatehouse-pilot-preflight/v1', 'state': 'observed', 'head': HEAD, 'run': RUN,
                'helper_revision': code['revision'], 'observation_sha256': digest(B.canonical(before)),
                'remote': observed, 'administrator_ready': False, 'production_gate_enabled': False,
                'scope': 'read-only logical pilot observation; recheck under continuous upgrade lease; no quality or upgrade authorization'}
    finally:
        if own_lease:
            os.close(lock_fd)
