"""Materialize fixed Git inputs independently; do not execute Agent checkout hooks/config."""
from __future__ import annotations

import os
import base64
from pathlib import Path
import shutil
import subprocess

from .common import Blocked, atomic_json, checked, digest, protected, read_json, read_regular, require, sha
from .execution import HOST_DIRECTORY, clean_environment, git


@checked
def prepare(config, github, journal, bundle, head):
    sha(head)
    require(not journal.outstanding(), 'previous batch must be merged or fully revoked before freezing input')
    source = Path(config['source'])
    mirror = protected(config['mirror'], directory=True)
    branch = github.repo('GET', '/git/ref/heads/' + config['base_branch'])
    base = sha(branch['object']['sha'])
    repository = github.repo('GET', '')
    require(repository['id'] == config['repository_id'], 'repository identity differs from protected configuration')
    require(isinstance(bundle, bytes), 'source preparation accepts only captured submission bytes')
    payload = bundle
    require(len(payload) <= config['max_bundle_bytes'], 'bundle exceeds budget')
    freeze_path = mirror.parent / 'frozen.json'
    freeze = {'repository': config['repository'], 'repository_id': repository['id'],
              'head': head, 'base': base, 'bundle_sha256': digest(payload)}
    if freeze_path.exists():
        require(read_json(protected(freeze_path, private=True)) == freeze,
                'an existing freeze has different input/baseline; retain it for reconciliation')
    else:
        atomic_json(freeze_path, freeze)
    if source.exists():
        protected(source, directory=True)
        require(git(source, 'rev-parse', 'HEAD') == head and
                not git(source, 'status', '--porcelain', '--untracked-files=no'), 'interrupted source slot differs')
        git(source, 'merge-base', '--is-ancestor', base, head)
        return {'repository': config['repository'], 'repository_id': repository['id'], 'head': head,
                'tree': sha(git(source, 'rev-parse', 'HEAD^{tree}')), 'base': base}
    env = clean_environment()
    # Token remains in a root-owned Git process environment, never in argv/logs/Agent namespace.
    env.update(GIT_CONFIG_COUNT='1', GIT_CONFIG_KEY_0='http.https://github.com/.extraheader',
               GIT_CONFIG_VALUE_0='AUTHORIZATION: basic ' + base64.b64encode(
                   ('x-access-token:' + github.installation_token()).encode()).decode())
    try:
        result = subprocess.run(['/usr/bin/git', '-c', 'core.hooksPath=/dev/null',
            '-c', 'protocol.ext.allow=never', '-C', str(mirror), 'fetch', '--no-tags',
            'https://github.com/' + config['repository'] + '.git', base], env=env,
            capture_output=True, timeout=120)
    except subprocess.TimeoutExpired:
        raise Blocked('independent baseline fetch timed out; reconcile the administrator-managed Git network route') from None
    require(result.returncode == 0, 'independent fetch of target baseline failed')
    # Copy one bounded bundle to protected storage before parsing; never fetch a user-controlled
    # repository or its Git configuration as root. Git validates the content-addressed objects.
    bundle_path = mirror.parent / 'candidate.bundle'
    if bundle_path.exists():
        require(read_regular(bundle_path, config['max_bundle_bytes']) == payload, 'staged bundle changed')
    else:
        with bundle_path.open('xb') as output:
            output.write(payload)
            output.flush()
            os.fsync(output.fileno())
    git(mirror, 'bundle', 'verify', str(bundle_path))
    git(mirror, '-c', 'protocol.file.allow=always', 'fetch', '--no-tags', str(bundle_path), head)
    git(mirror, 'merge-base', '--is-ancestor', base, head)
    tree = sha(git(mirror, 'rev-parse', head + '^{tree}'))
    entries = git(mirror, 'ls-tree', '-r', '-l', head).splitlines()
    total = 0
    require(len(entries) <= config['max_source_files'], 'candidate has too many source files')
    for entry in entries:
        metadata, name = entry.split('\t', 1)
        mode, kind, object_id, size = metadata.split()
        require(mode in ('100644', '100755') and kind == 'blob', 'symlinks/submodules require explicit supported materialization')
        require(name != HOST_DIRECTORY and not name.startswith(HOST_DIRECTORY + '/'), 'candidate uses reserved host path')
        total += int(size)
    require(total <= config['max_source_bytes'], 'candidate exceeds source byte budget')
    git(mirror, 'worktree', 'add', '--detach', str(source), head)
    for root, directories, files in os.walk(source):
        os.chmod(root, 0o755)
        for name in files:
            path = Path(root) / name
            os.chmod(path, 0o755 if path.stat().st_mode & 0o111 else 0o644)
    return {'repository': config['repository'], 'repository_id': repository['id'],
            'head': head, 'tree': tree, 'base': base}


def retire(config, executor, journal, run_id):
    run = journal.get(run_id)
    require(run['remote'] in ('merged', 'revoked'), 'retain the source until merge or confirmed revocation')
    require(not journal.outstanding(), 'a later active epoch owns the source slot; cannot retire an old run')
    require(journal.latest(run['repository'], run['head'])['id'] == run_id,
            'a later epoch exists for this candidate; only its owner can retire the source')
    executor.confirm_stopped(run)
    source = Path(config['source'])
    freeze_path = Path(config['mirror']).parent / 'frozen.json'
    if freeze_path.exists():
        require(read_json(protected(freeze_path, private=True))['head'] == run['head'], 'freeze belongs to another candidate')
    destination = Path(config['records']) / run_id / 'publication'
    if source.exists() and run['evidence'] and not destination.exists():
        executor.verify_evidence(run, run['evidence'])
        shutil.copytree(run['evidence']['directory'], destination, symlinks=False)
    journal.event(run_id, 'retire-intent', {'head': run['head'], 'retained': str(destination)})
    if source.exists():
        require(git(source, 'rev-parse', 'HEAD') == run['head'], 'source slot now belongs to another candidate')
        git(Path(config['mirror']), 'worktree', 'remove', '--force', str(source))
    bundle = Path(config['mirror']).parent / 'candidate.bundle'
    if bundle.exists():
        require(not bundle.is_symlink(), 'unknown bundle entry')
        bundle.unlink()
    if freeze_path.exists():
        freeze_path.unlink()
    journal.event(run_id, 'retired', {'head': run['head']})
    return journal.get(run_id)
