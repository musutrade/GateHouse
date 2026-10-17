"""Offline bootstrap source staging while the accepted 659 slot remains occupied."""
from __future__ import annotations

import fcntl
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import resource
import subprocess
import tempfile

PILOT = Path('/srv/gatehouse/pilot')
VOLUME = Path('/srv/gatehouse/linux')
OLD_HEAD = '659803a2f144926997e56d072c7fe8760d6ed176'
BASE = '8994cf53a44677ecb36acc6f9a9ce1d547c57728'
MAX_BUNDLE = 128 * 1024**2
MAX_SOURCE = 256 * 1024**2
MAX_REPLY = 4 * 1024**2


def git_limits():
    resource.setrlimit(resource.RLIMIT_AS, (2 * 1024**3, 2 * 1024**3))
    resource.setrlimit(resource.RLIMIT_CPU, (120, 120))
    resource.setrlimit(resource.RLIMIT_FSIZE, (MAX_BUNDLE, MAX_BUNDLE))
    resource.setrlimit(resource.RLIMIT_NOFILE, (128, 128))


def git(B, directory, *args, spool):
    B.owned('/usr/bin/git')
    # Bounded private files keep subprocess output out of the controller's RAM.
    with tempfile.TemporaryFile(dir=spool) as stdout, tempfile.TemporaryFile(dir=spool) as stderr:
        result = subprocess.run(['/usr/bin/git', '--no-replace-objects', '-c', 'core.hooksPath=/dev/null',
            '-c', 'core.fsmonitor=false', '-c', 'protocol.ext.allow=never', '-c', 'pack.threads=1',
            '-c', 'pack.windowMemory=32m', '-C', str(directory), *args], env=B.ENV,
            stdout=stdout, stderr=stderr, timeout=120, preexec_fn=git_limits)
        B.need(result.returncode == 0 and stdout.tell() <= MAX_REPLY and stderr.tell() <= MAX_REPLY,
               'bounded staging Git operation failed')
        stdout.seek(0)
        return stdout.read(MAX_REPLY + 1)


def tree(B, mirror, head):
    entries = B.git(mirror, 'ls-tree', '-r', '-l', '-z', head).split(b'\0')
    B.need(len(entries) <= 20001, 'staged source entry count exceeds budget')
    total, count = 0, 0
    for entry in filter(None, entries):
        meta, raw_name = entry.split(b'\t', 1)
        mode, kind, object_id, size = meta.split()
        name = raw_name.decode('utf-8')
        path = PurePosixPath(name)
        B.need(mode in (b'100644', b'100755') and kind == b'blob' and path.parts
               and not path.is_absolute() and path.as_posix() == name and '\\' not in name
               and all(part not in ('', '.', '..', '.git') for part in name.split('/'))
               and path.parts[0] != '.gatehouse-host', 'unsupported staged source member')
        size = int(size)
        total += size
        count += 1
        B.need(0 <= size <= MAX_BUNDLE and total <= MAX_SOURCE, 'staged source bytes exceed budget')
    return {'tree': B.git(mirror, 'rev-parse', head + '^{tree}').decode().strip(), 'files': count, 'bytes': total}


def stage(B, helpers, manifest, manifest_pin, bundle, bundle_pin, head, recipe, recipe_pin, lock_fd=None):
    B.need(os.geteuid() == 0, 'administrator authentication required for source staging')
    os.umask(0o077)
    code = B.authenticate(helpers, manifest, manifest_pin)
    B.need(re.fullmatch('[0-9a-f]{40}', head) and head == code['revision'], 'source differs from reviewed helper revision')
    builder = B.load(helpers, 'build-protected-core.py')
    volume = B.load(helpers, 'prepare-linux-volume.py')
    lease = B.owned(PILOT / 'control/controller.lock', private=True)
    own_lease = lock_fd is None
    if own_lease:
        lock_fd = os.open(lease, os.O_RDWR | os.O_NOFOLLOW)
    try:
        actual, expected = os.fstat(lock_fd), lease.stat()
        B.need((actual.st_dev, actual.st_ino) == (expected.st_dev, expected.st_ino), 'staging lease differs')
        fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        builder.pilot_slot(B)
        builder.quiet_units(B)
        builder.idle(B)
        volume.admit(B, helpers, manifest, manifest_pin, recipe, recipe_pin)
        old_source = PILOT / 'source'
        old_lock = B.frozen_lock(old_source, OLD_HEAD, BASE)
        old_freeze = B.read(PILOT / 'repository/frozen.json', 1024**2)
        freeze = json.loads(old_freeze, object_pairs_hook=B.unique)
        B.need(freeze['repository'] == 'musutrade/GateHouse' and freeze['repository_id'] == 1414069625
               and freeze['head'] == OLD_HEAD and freeze['base'] == BASE, 'retained baseline identity differs')
        payload = B.read(B.owned(bundle, private=True), MAX_BUNDLE)
        B.need(re.fullmatch('[0-9a-f]{64}', bundle_pin) and hashlib.sha256(payload).hexdigest() == bundle_pin,
               'captured source bundle pin differs')
        destination = VOLUME / 'bootstrap-source'
        B.need(not destination.exists() and not destination.is_symlink(), 'retain previous source staging; never reuse')
        B.need(all(destination != path and destination not in path.parents and path not in destination.parents
                   for path in map(Path, (helpers, manifest, bundle, recipe))), 'source stage overlaps an input')
        destination.mkdir(mode=0o700)
        state = {'schema': 'gatehouse-bootstrap-source/v1', 'state': 'incomplete', 'head': head, 'base': BASE,
                 'helper_manifest_sha256': manifest_pin, 'bundle_sha256': bundle_pin,
                 'baseline_authority': 'retained protected pilot freeze, not a current remote-base observation',
                 'production_gate_enabled': False, 'scope': 'offline bootstrap staging only; no delivery or quality authorization'}
        record = destination / 'stage.json'
        B.record(record, state)
        try:
            # Use the scoped bounded reader for candidate data; the old frozen
            # store has already passed the protected metadata/size checks.
            from types import SimpleNamespace
            G = SimpleNamespace(**vars(B))
            G.git = lambda directory, *args: git(B, directory, *args, spool=destination)
            captured = destination / 'candidate.bundle'
            B.write(captured, payload, 0o400)
            mirror, source = destination / 'mirror.git', destination / 'source'
            # Clone only the already protected store, never an Agent checkout or
            # its settings. No hardlinks or alternates into the old publication.
            G.git(destination, 'clone', '--mirror', '--no-hardlinks', str(PILOT / 'repository/mirror.git'), str(mirror))
            B.need(G.git(mirror, 'cat-file', '-t', BASE).strip() == b'commit', 'retained baseline object is missing')
            B.need(G.git(mirror, 'bundle', 'list-heads', str(captured)) == (head + ' HEAD\n').encode(),
                   'bundle advertises another or ambiguous candidate')
            G.git(mirror, 'bundle', 'verify', str(captured))
            G.git(mirror, '-c', 'protocol.file.allow=always', '-c', 'fetch.fsckObjects=true',
                  'fetch', '--no-tags', '--no-recurse-submodules', str(captured), head)
            G.git(mirror, 'merge-base', '--is-ancestor', BASE, head)
            summary = tree(G, mirror, head)
            for name, pin in code['files'].items():
                raw = G.git(mirror, 'show', head + ':' + name)
                B.need(hashlib.sha256(raw).hexdigest() == pin and raw == B.read(Path(helpers) / name, 1024**2),
                       'candidate helper bytes differ from authenticated release')
            G.git(mirror, 'worktree', 'add', '--detach', str(source), head)
            for parent, directories, files in os.walk(source):
                parent = B.owned(parent, directory=True)
                parent.chmod(0o755)
                for name in files:
                    path = B.owned(parent / name)
                    path.chmod(0o755 if path.stat().st_mode & 0o111 else 0o644)
            B.frozen_lock(source, head, BASE)
            B.need(not G.git(source, 'status', '--porcelain', '--untracked-files=all'), 'staged source is not clean')
            B.need(B.frozen_lock(old_source, OLD_HEAD, BASE) == old_lock
                   and B.read(PILOT / 'repository/frozen.json', 1024**2) == old_freeze, 'old source/freeze changed')
            B.authenticate(helpers, manifest, manifest_pin)
            volume.admit(B, helpers, manifest, manifest_pin, recipe, recipe_pin)
            builder.pilot_slot(B)
            builder.quiet_units(B)
            builder.idle(B)
            B.write(destination / 'frozen.json', B.canonical({**freeze, 'head': head, 'bundle_sha256': bundle_pin}), 0o400)
            state.update(state='staged', **summary)
            B.record(record, state)
            return state
        except BaseException:
            state['state'] = 'blocked'
            B.record(record, state)
            raise
    finally:
        if own_lease:
            os.close(lock_fd)
