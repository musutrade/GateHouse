"""Protected Core build primitives, loaded only by the authenticated importer."""
from __future__ import annotations

import fcntl
import grp
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import pwd
import re
import sqlite3
import stat
import subprocess
import time
import uuid

CORE_TREE = 'a6918d6a7ff7fadc04170ff54aaac0db771763db'
CONTROL = Path('/srv/gatehouse/pilot/control')
PILOT_HEAD = '659803a2f144926997e56d072c7fe8760d6ed176'
PILOT_RUN = 'ce86a2a4-ff18-435f-bb5d-aafc313ceca6'
UID, GID = 995, 981
TIMEOUT = 1200
MAX_BINARY = 128 * 1024 * 1024


def json_pin(B, path, expected, limit=1024 * 1024):
    raw = B.read(path, limit)
    B.need(isinstance(expected, str) and re.fullmatch('[0-9a-f]{64}', expected)
           and hashlib.sha256(raw).hexdigest() == expected, 'build input metadata pin differs')
    value = json.loads(raw, object_pairs_hook=B.unique)
    B.need(raw == B.canonical(value), 'build metadata is not canonical')
    return value


def host_file(path):
    """OS-managed command hardlinks are allowed; input/code hardlinks are not."""
    path = Path(path)
    for item in (path, *path.parents):
        info = item.lstat()
        if info.st_uid != 0 or stat.S_ISLNK(info.st_mode) or info.st_mode & 0o022:
            raise ValueError('host build file is not Root-protected')
    with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), 'rb') as source:
        before = os.fstat(source.fileno())
        if not stat.S_ISREG(before.st_mode) or before.st_size > MAX_BINARY:
            raise ValueError('host build file exceeds budget or type')
        raw = source.read(MAX_BINARY + 1)
        fields = lambda info: (info.st_dev, info.st_ino, info.st_uid, info.st_gid, info.st_mode,
                              info.st_nlink, info.st_size, info.st_mtime_ns, info.st_ctime_ns)
        if len(raw) != before.st_size or fields(before) != fields(os.fstat(source.fileno())) or fields(before) != fields(path.lstat()):
            raise ValueError('host build file changed while reading')
    return raw


def host_inputs(B, path, pin):
    value = json_pin(B, path, pin)
    B.need(set(value) == {'schema', 'files', 'cc', 'ar', 'env'} and value['schema'] == 'gatehouse-core-build-host/v1',
           'unsupported Core build host recipe')
    files = value['files']
    required = {'/usr/bin/git', '/usr/bin/systemd-run', '/usr/bin/systemctl', '/usr/bin/pgrep',
                *(str(B.path_operand(value[name])) for name in ('cc', 'ar', 'env'))}
    B.need(isinstance(files, dict) and required <= files.keys() and len(files) <= 64,
           'pin reviewed host compiler and management tools')
    total = 0
    for name, expected in files.items():
        B.path_operand(name)
        raw = host_file(name)
        total += len(raw)
        B.need(total <= 512 * 1024 * 1024 and isinstance(expected, str)
               and hashlib.sha256(raw).hexdigest() == expected, 'host build input differs from reviewed pin')
    return value


def readonly_inputs(B, root):
    count = 0
    for parent, directories, files in os.walk(root):
        for name in (None, *directories, *files):
            path = Path(parent) if name is None else Path(parent) / name
            info = path.lstat()
            directory = stat.S_ISDIR(info.st_mode)
            B.owned(path, directory=directory)
            B.need(stat.S_IMODE(info.st_mode) == (0o555 if directory or info.st_mode & 0o111 else 0o444),
                   'imported build input is not readonly')
            count += 1
            B.need(count <= 300000, 'imported build input count exceeds budget')


def imported_inputs(B, root, pin, source, head, base, helpers):
    root = B.owned(root, directory=True)
    binding = json_pin(B, root / 'binding.json', pin)
    state = root.with_name(root.name + '-import.json')
    B.owned(state, private=True)
    B.need(B.read(state, 1024 * 1024) == B.canonical(binding) and binding['state'] == 'imported'
           and binding['schema'] == 'gatehouse-protected-import/v1'
           and binding['head'] == head and binding['base'] == base,
           'protected import has no matching complete control record')
    lock = B.frozen_lock(source, head, base)
    B.need(hashlib.sha256(lock).hexdigest() == binding['lock_sha256'], 'build source lock differs from imported dependencies')
    official = B.load(helpers, 'prepare-official-tools.py')
    directory = B.load(helpers, 'prepare-locked-directory.py')
    tools = json_pin(B, root / 'tools/inventory.json', binding['tool_inventory_sha256'], official.MAX_INVENTORY)
    deps = json_pin(B, root / 'dependencies/inventory.json', binding['dependency_inventory_sha256'], directory.MAX_INVENTORY)
    raw, tool_rows = official.TOOLS.lock(root / 'tools/inputs.lock.json', binding['tool_lock_sha256'])
    B.need(official.CRATES.read(helpers / 'tools/trusted_gate/deploy/linux-tools.lock.json') == raw,
           'imported tool lock differs from reviewed helper release')
    B.need(tools['schema'] == 'gatehouse-official-tools/v1' and tools['tool_lock_sha256'] == binding['tool_lock_sha256'],
           'imported tool inventory belongs to another lock')
    actual = official.STAGE.inventory(root / 'tools/toolchain')
    B.need(actual == (tools['files'], tools['directories'], tools['bytes']), 'imported tool payload changed')
    B.need({p.name for p in (root / 'tools/archives').iterdir()} ==
           {row['url'].rsplit('/', 1)[1] for row in tool_rows.values()}, 'official archive inventory changed')
    for row in tool_rows.values():
        archive = root / 'tools/archives' / row['url'].rsplit('/', 1)[1]
        with official.CRATES.regular(archive, row['max_bytes']) as (stream, info):
            B.need(official.CRATES.content(stream, info.st_size) == row['sha256'], 'retained official archive changed')
    B.need(directory.CRATES.read(root / 'dependencies/Cargo.lock') == lock and deps['lock_sha256'] == binding['lock_sha256'],
           'retained dependency lock changed')
    packages, local = directory.CRATES.locked_packages(lock, binding['lock_sha256'])
    B.need(deps['schema'] == 'gatehouse-locked-directory/v1' and deps['packages'].keys() == packages.keys()
           and {p.name for p in (root / 'dependencies/sources').iterdir()} == packages.keys()
           and {p.name for p in (root / 'dependencies/archives').iterdir()} == {p + '.crate' for p in packages},
           'imported dependency inventory changed')
    count, total = 0, 0
    for package, expected in packages.items():
        path = root / 'dependencies/sources' / package
        B.need(not (path / '.cargo-ok').exists() and not (path / '.cargo-ok').is_symlink(), 'unexpected Cargo cache marker')
        files, directories, size = directory.CRATES.source_inventory(path)
        metadata = directory.CRATES.read(path / '.cargo-checksum.json')
        row = files.pop('.cargo-checksum.json', None)
        item = deps['packages'][package]
        B.need(row == [len(metadata), item['cargo_checksum_sha256'], False]
               and hashlib.sha256(metadata).hexdigest() == item['cargo_checksum_sha256']
               and metadata == directory.checksum(files, expected)
               and hashlib.sha256(directory.canonical(files)).hexdigest() == item['source_inventory_sha256']
               and len(files) == item['files'] and size - len(metadata) == item['bytes']
               and item['archive_sha256'] == expected, 'imported dependency source/checksum changed')
        expected_dirs = {str(parent) for name in files for parent in PurePosixPath(name).parents if str(parent) != '.'}
        B.need(directories == expected_dirs, 'unexpected dependency source directory')
        archive = root / 'dependencies/archives' / (package + '.crate')
        with directory.CRATES.regular(archive, directory.CRATES.MAX_FILE) as (stream, info):
            B.need(directory.CRATES.content(stream, info.st_size) == expected, 'retained crate archive changed')
        count += len(files)
        total += item['bytes']
    B.need(count == deps['files'] == binding['dependency_files'] and total == deps['bytes'] == binding['dependency_bytes']
           and len(packages) == binding['dependency_packages'] and len(tools['files']) == binding['tool_files']
           and tools['bytes'] == binding['tool_bytes'], 'imported build input totals differ')
    B.need({p.name for p in root.iterdir()} == {'tools', 'dependencies', 'binding.json'}
           and {p.name for p in (root / 'tools').iterdir()} == {'toolchain', 'archives', 'inputs.lock.json', 'inventory.json'}
           and {p.name for p in (root / 'dependencies').iterdir()} == {'sources', 'archives', 'Cargo.lock', 'inventory.json'},
           'unexpected imported build input')
    readonly_inputs(B, root)
    return binding


def source_snapshot(B, source, head, destination):
    tree = B.git(source, 'rev-parse', head + ':tools/harness-gate').decode().strip()
    B.need(tree == CORE_TREE, 'Core source tree lacks independent review')
    def tree_object(identity):
        raw = B.git(source, 'cat-file', 'tree', identity)
        B.need(hashlib.sha1(b'tree ' + str(len(raw)).encode() + b'\0' + raw).hexdigest() == identity,
               'reviewed Core tree object changed')
    tree_object(tree)
    entries = B.git(source, 'ls-tree', '-r', '-t', '-l', '-z', tree).split(b'\0')
    B.need(len(entries) <= 20001, 'Core tree entry count exceeds budget')
    rows, total = {}, 0
    for entry in filter(None, entries):
        meta, name = entry.split(b'\t', 1)
        mode, kind, blob, size = meta.split()
        if kind == b'tree':
            B.need(mode == b'040000' and size == b'-', 'unsupported Core tree entry')
            tree_object(blob.decode())
            continue
        name = name.decode('utf-8')
        path = PurePosixPath(name)
        B.need(mode in (b'100644', b'100755') and kind == b'blob' and path.parts
               and not path.is_absolute() and path.as_posix() == name
               and all(part not in ('', '.', '..', '.git') for part in name.split('/'))
               and '\\' not in name, 'unsupported reviewed Core source entry')
        size = int(size)
        total += size
        B.need(0 <= size <= 4 * 1024 * 1024 and total <= 256 * 1024 * 1024 and len(rows) < 20000,
               'Core source snapshot exceeds budget')
        rows[name] = (blob.decode(), size, 0o555 if mode == b'100755' else 0o444)
    B.need({'Cargo.toml', 'Cargo.lock'} <= rows.keys(), 'reviewed Core source is incomplete')
    destination.mkdir(mode=0o700)
    files, directories = {}, set()
    for name, (blob, size, mode) in sorted(rows.items()):
        raw = B.git(source, 'cat-file', 'blob', blob)
        B.need(len(raw) == size and hashlib.sha1(b'blob ' + str(size).encode() + b'\0' + raw).hexdigest() == blob,
               'reviewed Core blob identity changed')
        files[name] = [size, hashlib.sha256(raw).hexdigest(), mode]
        directories.update(str(parent) for parent in PurePosixPath(name).parents if str(parent) != '.')
        path = destination.joinpath(*PurePosixPath(name).parts)
        path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        B.write(path, raw, mode)
    B.readonly(destination)
    value = {'files': files, 'directories': sorted(directories), 'bytes': total}
    return {'tree': tree, 'files': len(rows), 'bytes': total,
            'inventory_sha256': hashlib.sha256(B.canonical(value)).hexdigest()}


def verify_snapshot(B, helpers, root, expected):
    stage = B.load(helpers, 'stage-linux-inputs.py')
    files, directories, total = stage.inventory(root)
    value = {'files': files, 'directories': directories, 'bytes': total}
    B.need(len(files) == expected['files'] and total == expected['bytes']
           and hashlib.sha256(B.canonical(value)).hexdigest() == expected['inventory_sha256'],
           'reviewed Core source snapshot changed')
    readonly_inputs(B, root)


def worker_identity(B):
    account = pwd.getpwuid(UID)
    groups = {g.gr_name for g in grp.getgrall() if account.pw_name in g.gr_mem}
    groups.add(grp.getgrgid(account.pw_gid).gr_name)
    B.need(account.pw_uid == UID and account.pw_name == 'gatehouse-executor' and account.pw_gid == GID
           and account.pw_shell in ('/usr/sbin/nologin', '/sbin/nologin', '/bin/false')
           and groups == {'gatehouse-executor'}, 'Core build requires the isolated executor identity')


def idle(B):
    result = subprocess.run(['/usr/bin/pgrep', '-u', str(UID)], env=B.ENV, capture_output=True, timeout=10)
    B.need(result.returncode == 1 and not result.stdout and not result.stderr, 'executor still has active or unknown processes')


def pilot_slot(B):
    """Build before revocation: a crash must leave the old ingress slot occupied."""
    journal = B.owned(CONTROL / 'journal.sqlite3', private=True)
    database = sqlite3.connect('file:' + str(journal) + '?mode=ro', uri=True)
    try:
        rows = database.execute("SELECT id,head,local FROM runs WHERE remote NOT IN ('merged','revoked')").fetchall()
    finally:
        database.close()
    B.need(len(rows) == 1 and rows[0][0] == PILOT_RUN and rows[0][1] == PILOT_HEAD
           and rows[0][2] in ('passed', 'failed'), 'retain the accepted idle 659 pilot slot until Core capture completes')


def quiet_units(B):
    result = subprocess.run(['/usr/bin/systemctl', 'list-units', '--all', '--plain', '--no-legend', '--no-pager',
        'gatehouse-core-*', 'gatehouse-step-*', 'gatehouse-bootstrap-*'],
        env=B.ENV, capture_output=True, check=True, timeout=10)
    for line in result.stdout.decode().splitlines():
        fields = line.split()
        B.need(len(fields) >= 4 and fields[2] in ('inactive', 'failed'), 'a previous trusted host unit is active or unknown')
        stopped(B, fields[0])


def stopped(B, unit):
    result = subprocess.run(['/usr/bin/systemctl', 'list-units', '--all', '--plain', '--no-legend',
        '--no-pager', unit], env=B.ENV, capture_output=True, check=True, timeout=10)
    for line in result.stdout.decode().splitlines():
        fields = line.split()
        B.need(len(fields) >= 4 and fields[0] == unit and fields[2] in ('inactive', 'failed'), 'Core build unit is still active or unknown')
        group = subprocess.run(['/usr/bin/systemctl', 'show', '--property=ControlGroup', '--value', unit],
            env=B.ENV, capture_output=True, check=True, timeout=10).stdout.decode().strip()
        if group:
            B.need(group.startswith('/') and '..' not in Path(group).parts, 'unsafe Core build cgroup')
            events = Path('/sys/fs/cgroup') / group.lstrip('/') / 'cgroup.events'
            with B.owned(events).open('rb') as source:
                raw = source.read(4097)
            B.need(len(raw) <= 4096 and b'populated 0' in raw.splitlines(), 'Core build cgroup is not proven empty')
    idle(B)


def command(B, source, inputs, worker, cargo_config, unit, host):
    properties = {'User': str(UID), 'Group': str(GID), 'SupplementaryGroups': '', 'UMask': '0077',
        'NoNewPrivileges': 'yes', 'CapabilityBoundingSet': '', 'AmbientCapabilities': '',
        'ProtectSystem': 'strict', 'ProtectHome': 'yes', 'PrivateTmp': 'yes', 'PrivateDevices': 'yes',
        'PrivateNetwork': 'yes', 'RestrictNamespaces': 'yes', 'RestrictSUIDSGID': 'yes',
        'ProtectKernelTunables': 'yes', 'ProtectKernelModules': 'yes', 'ProtectControlGroups': 'yes',
        'ProtectKernelLogs': 'yes', 'ProtectClock': 'yes', 'LockPersonality': 'yes',
        'ProtectProc': 'invisible', 'ProcSubset': 'pid', 'KillMode': 'control-group',
        'RuntimeMaxSec': str(TIMEOUT), 'TimeoutStopSec': '15', 'MemoryMax': str(8 * 1024**3),
        'TasksMax': '512', 'LimitFSIZE': str(1024**3), 'WorkingDirectory': '/work/source',
        'TemporaryFileSystem': '/work:mode=0755,nosuid,nodev', 'UnsetEnvironment': 'LD_PRELOAD LD_LIBRARY_PATH',
        'BindReadOnlyPaths': str(source) + ':/work/source ' + str(inputs / 'tools/toolchain') +
            ':/opt/gatehouse/toolchain ' + str(inputs / 'dependencies/sources') + ':/work/dependencies/sources ' +
            str(cargo_config) + ':/work/output/cargo-home/config.toml',
        'BindPaths': str(worker) + ':/work/output', 'ReadWritePaths': '/work/output',
        'InaccessiblePaths': '/home /root /run /mnt /etc/gatehouse /var/lib/gatehouse /srv/gatehouse/pilot -/.cargo'}
    environment = {'PATH': '/opt/gatehouse/toolchain/bin:/usr/bin:/bin', 'LANG': 'C.UTF-8',
        'HOME': '/work/output/home', 'TMPDIR': '/work/output/tmp', 'CARGO_HOME': '/work/output/cargo-home',
        'CARGO_TARGET_DIR': '/work/output/target', 'CARGO_NET_OFFLINE': 'true', 'CARGO_BUILD_JOBS': '4',
        'CARGO_INCREMENTAL': '0', 'CARGO_PROFILE_RELEASE_INCREMENTAL': 'false',
        'CC': host['cc'], 'AR': host['ar'], 'CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER': host['cc']}
    argv = ['/usr/bin/systemd-run', '--quiet', '--wait', '--pipe', '--collect', '--unit=' + unit]
    argv += ['--property=' + key + '=' + value for key, value in properties.items()]
    return [*argv, '--', host['env'], '-i', *[key + '=' + value for key, value in environment.items()],
            '/opt/gatehouse/toolchain/bin/cargo', 'build', '--release', '--locked', '--offline',
            '--manifest-path', '/work/source/Cargo.toml', '--bin', 'harness-gate'], environment


def capture(B, worker, destination):
    path = worker / 'target/release/harness-gate'
    for parent in (path.parent, path.parent.parent, worker):
        info = parent.lstat()
        B.need(stat.S_ISDIR(info.st_mode) and info.st_uid == UID and not info.st_mode & 0o022,
               'unsafe generated Core directory')
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as source:
        before = os.fstat(source.fileno())
        B.need(stat.S_ISREG(before.st_mode) and before.st_uid == UID and before.st_mode & stat.S_IXUSR
               and not before.st_mode & 0o7022 and 0 < before.st_size <= MAX_BINARY,
               'unsafe generated Core executable')
        raw = source.read(MAX_BINARY + 1)
        after = os.fstat(source.fileno())
        fields = lambda info: (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid,
                              info.st_nlink, info.st_size, info.st_mtime_ns, info.st_ctime_ns)
        B.need(len(raw) == before.st_size and fields(before) == fields(after) == fields(path.lstat()),
               'generated Core changed during capture')
    B.write(destination, raw, 0o555)
    return {'sha256': hashlib.sha256(raw).hexdigest(), 'bytes': len(raw)}


def build(B, helpers, manifest, manifest_pin, source, head, base, inputs, input_pin,
          host_path, host_pin, destination, lock_fd=None):
    B.need(os.geteuid() == 0, 'administrator authentication required for Core bootstrap')
    B.need(os.uname().sysname == 'Linux' and os.uname().machine == 'x86_64', 'unsupported Core bootstrap platform')
    os.umask(0o077)
    code = B.authenticate(helpers, manifest, manifest_pin)
    helpers, source, inputs, destination, host_path = map(B.path_operand, (helpers, source, inputs, destination, host_path))
    B.owned(destination.parent, directory=True, private=True)
    domains = (helpers, Path(manifest), source, inputs, host_path)
    B.need(all(destination != path and destination not in path.parents and path not in destination.parents
               for path in domains), 'Core build output overlaps an input')
    B.need(not destination.exists() and not destination.is_symlink(), 'retain previous Core bootstrap; never reuse')
    B.owned(CONTROL, directory=True, private=True)
    lease = CONTROL / 'controller.lock'
    own_lease = lock_fd is None
    if own_lease:
        lock_fd = os.open(lease, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    try:
        B.owned(lease, private=True)
        info, expected = os.fstat(lock_fd), lease.stat()
        B.need(stat.S_ISREG(info.st_mode) and (info.st_dev, info.st_ino) == (expected.st_dev, expected.st_ino),
               'Core bootstrap lease descriptor differs')
        fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        host = host_inputs(B, host_path, host_pin)
        pilot_slot(B)
        worker_identity(B)
        quiet_units(B)
        idle(B)
        binding = imported_inputs(B, inputs, input_pin, source, head, base, helpers)
        destination.mkdir(mode=0o700)
        record = {'schema': 'gatehouse-core-bootstrap/v1', 'state': 'incomplete', 'head': head, 'base': base,
                  'helper_revision': code['revision'], 'helper_manifest_sha256': manifest_pin,
                  'input_binding_sha256': input_pin, 'host_recipe_sha256': host_pin,
                  'profile': 'release', 'release_incremental': False,
                  'worker_uid': UID, 'worker_gid': GID, 'target': 'x86_64-unknown-linux-gnu',
                  'production_gate_enabled': False,
                  'worker_stop_proven': False,
                  'scope': 'Core bootstrap only; no installation, full verification or delivery authorization'}
        record_path = destination / 'build.json'
        B.record(record_path, record)
        unit = 'gatehouse-bootstrap-' + str(uuid.uuid4()) + '.service'
        try:
            snapshot = destination / 'source'
            record['source'] = source_snapshot(B, source, head, snapshot)
            verify_snapshot(B, helpers, snapshot, record['source'])
            B.need(hashlib.sha256(B.read(snapshot / 'Cargo.lock', 4 * 1024**2)).hexdigest() == binding['lock_sha256'],
                   'reviewed Core snapshot lock differs')
            worker = destination / 'worker'
            worker.mkdir(mode=0o700)
            for name in ('home', 'tmp', 'cargo-home', 'target'):
                path = worker / name
                path.mkdir(mode=0o700)
                os.chown(path, UID, GID)
            B.write(worker / 'cargo-home/config.toml', b'', 0o600)
            os.chown(worker / 'cargo-home/config.toml', UID, GID)
            os.chown(worker, UID, GID)
            cargo_config = destination / 'cargo-config.toml'
            B.write(cargo_config, b'[source.crates-io]\nreplace-with = "gatehouse-locked"\n'
                    b'[source.gatehouse-locked]\ndirectory = "/work/dependencies/sources"\n[net]\noffline = true\n')
            argv, environment = command(B, snapshot, inputs, worker, cargo_config, unit, host)
            record.update(unit=unit, argv=argv, environment=environment, state='running')
            B.record(record_path, record)
            started = time.monotonic()
            try:
                with (destination / 'build.log').open('xb') as log:
                    result = subprocess.run(argv, env=B.ENV, stdout=log, stderr=subprocess.STDOUT, timeout=TIMEOUT + 60)
            except BaseException:
                subprocess.run(['/usr/bin/systemctl', 'stop', unit], env=B.ENV,
                               capture_output=True, timeout=30, check=False)
                stopped(B, unit)
                record['worker_stop_proven'] = True
                raise
            if result.returncode != 0:
                subprocess.run(['/usr/bin/systemctl', 'stop', unit], env=B.ENV,
                               capture_output=True, timeout=30, check=False)
            stopped(B, unit)
            record['worker_stop_proven'] = True
            record.update(worker_seconds=time.monotonic() - started, exit_code=result.returncode)
            B.need(result.returncode == 0, 'Core release build failed')
            pilot_slot(B)
            B.authenticate(helpers, manifest, manifest_pin)
            host_inputs(B, host_path, host_pin)
            imported_inputs(B, inputs, input_pin, source, head, base, helpers)
            verify_snapshot(B, helpers, snapshot, record['source'])
            B.need(B.git(source, 'rev-parse', head + ':tools/harness-gate').decode().strip() == CORE_TREE,
                   'reviewed Core tree changed')
            record['binary'] = capture(B, worker, destination / 'harness-gate')
            record['state'] = 'captured'
            B.record(record_path, record)
            return record
        except BaseException:
            record['state'] = 'blocked'
            B.record(record_path, record)
            raise
    finally:
        if own_lease:
            os.close(lock_fd)
