"""A separate immutable Git view; never expose the controller's repository/config."""
from __future__ import annotations

import os
from pathlib import Path
import stat
import uuid

from .common import atomic_json, checked, digest, protected, read_json, read_regular, require, sha
from .execution import git


def inventory(directory):
    files = {}
    total = 0
    for root, directories, names in os.walk(directory):
        protected(root, directory=True)
        for name in directories:
            protected(Path(root) / name, directory=True)
        for name in names:
            path = protected(Path(root) / name)
            info = path.stat()
            require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1,
                    'worker Git view contains a linked or special file')
            total += info.st_size
            require(len(files) < 20000 and total <= 256 * 1024 * 1024,
                    'worker Git history exceeds its input budget')
            value = read_regular(path, 128 * 1024 * 1024)
            files[str(path.relative_to(directory))] = {'bytes': len(value), 'sha256': digest(value)}
    return files


@checked
def prepare(config, run):
    require(str(uuid.UUID(run['id'])) == run['id'], 'invalid Git-view run identity')
    source = protected(config['source'], directory=True)
    records = protected(config['records'], directory=True, private=True)
    owner = protected(records / run['id'], directory=True, private=True)
    directory, receipt = owner / 'git-history', owner / 'git-history.json'
    pointer = owner / 'git-pointer'
    pointer_bytes = b'gitdir: /work/history\n'
    head, base, tree = (sha(run['input'][name]) for name in ('head', 'base', 'tree'))
    require(head == run['head'] == git(source, 'rev-parse', 'HEAD')
            and tree == git(source, 'rev-parse', 'HEAD^{tree}'), 'Git view candidate differs')
    require(not git(source, 'status', '--porcelain', '--untracked-files=no'),
            'Git view source bytes changed')
    git(source, 'merge-base', '--is-ancestor', base, head)
    binding = {'run': run['id'], 'head': head, 'base': base, 'tree': tree}
    if directory.exists() or receipt.exists() or pointer.exists():
        previous = read_json(protected(receipt, private=True))
        require(previous['input'] == binding and previous['files'] == inventory(directory)
                and read_regular(protected(pointer), 1024) == pointer_bytes,
                'retained worker Git view changed; preserve it for reconciliation')
        return directory
    # Explicit empty template and fresh config: no hooks, remotes, proxy, tokens,
    # credential helper, includes, alternates or Agent Git configuration are copied.
    git(owner, 'init', '--bare', '--template=', str(directory))
    git(directory, '-c', 'protocol.file.allow=always', 'fetch', '--no-tags', '--no-auto-maintenance',
        str(source), head, base)
    git(directory, 'update-ref', 'refs/heads/candidate', head)
    git(directory, 'update-ref', 'refs/gatehouse/base', base)
    git(directory, 'symbolic-ref', 'HEAD', 'refs/heads/candidate')
    git(directory, 'config', '--local', 'core.hooksPath', '/dev/null')
    git(directory, '--work-tree=' + str(source), 'reset', '--mixed', '--quiet', head)
    # FETCH_HEAD names the controller source path; only the fixed refs are needed.
    (directory / 'FETCH_HEAD').unlink()
    require(not (directory / 'objects/info/alternates').exists(), 'external Git objects are forbidden')
    require(git(directory, 'rev-parse', 'HEAD') == head
            and git(directory, 'rev-parse', 'refs/gatehouse/base') == base,
            'worker Git view has the wrong fixed refs')
    # Native discovery follows the synthetic .git file in this work tree only.
    # A global GIT_DIR would incorrectly retarget Git in nested test fixtures.
    git(directory, 'config', '--local', 'core.bare', 'false')
    with pointer.open('xb') as stream:
        stream.write(pointer_bytes)
        stream.flush()
        os.fsync(stream.fileno())
    pointer.chmod(0o444)
    for root, directories, files in os.walk(directory):
        for name in files:
            os.chmod(Path(root) / name, 0o444)
        os.chmod(root, 0o555)
    files = inventory(directory)
    atomic_json(receipt, {'input': binding, 'files': files})
    return directory
