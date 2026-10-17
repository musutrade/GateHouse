#!/usr/bin/python3 -I
"""Prepare disposable worker inputs; this is not a quality result or root entry."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import tempfile

MAX_BYTES = 256 * 1024 * 1024


def need(value, message):
    if not value:
        raise ValueError(message)


def regular(path, limit=MAX_BYTES):
    path = Path(path)
    for item in (path, *path.parents):
        need(not item.is_symlink(), 'linked workspace input')
    info = path.stat()
    need(stat.S_ISREG(info.st_mode) and info.st_nlink == 1 and info.st_size <= limit,
         'special, linked or oversized workspace input')
    return path.read_bytes()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def unique(pairs):
    result = {}
    for key, value in pairs:
        need(key not in result, 'duplicate workspace manifest field')
        result[key] = value
    return result


def environment():
    return {'PATH': '/usr/bin:/bin', 'LANG': 'C.UTF-8', 'GIT_CONFIG_NOSYSTEM': '1',
            'GIT_CONFIG_GLOBAL': '/dev/null', 'GIT_OPTIONAL_LOCKS': '0',
            'GIT_TERMINAL_PROMPT': '0', 'PYTHONDONTWRITEBYTECODE': '1'}


def git(directory, *args, safe=()):
    argv = ['/usr/bin/git', '--no-replace-objects', '-c', 'core.hooksPath=/dev/null',
            '-c', 'core.fsmonitor=false', '-c', 'protocol.ext.allow=never']
    for path in (directory, *safe):
        argv.extend(['-c', 'safe.directory=' + str(path)])
    return subprocess.check_output([*argv, '-C', str(directory), *args],
        env=environment(), stderr=subprocess.PIPE, timeout=120)


def prepare(source, history, destination, cache, head, base, manifest_sha256):
    need(os.geteuid() != 0, 'workspace preparation must run as the unprivileged worker')
    source, history, destination, cache = map(Path, (source, history, destination, cache))
    paths = (source, history, destination, cache)
    for path in paths:
        need(path.is_absolute() and '..' not in path.parts, 'workspace paths must be absolute')
        for parent in (path, *path.parents):
            need(not parent.is_symlink(), 'linked workspace path')
    need(all(destination != path and path not in destination.parents and destination not in path.parents
             for path in (source, history, cache)), 'workspace must be separate from immutable inputs')
    need(not destination.exists(), 'workspace already exists; retain it for diagnosis')
    log = destination.parent / (destination.name + '-input-preparation.json')
    need(not log.exists() and not log.is_symlink(), 'preparation record already exists')
    need(all(re.fullmatch('[0-9a-f]{40}', value) for value in (head, base))
         and re.fullmatch('[0-9a-f]{64}', manifest_sha256), 'exact candidate/base/manifest pins required')
    manifest_bytes = regular(source / 'test-data/manifest.json', 4 * 1024 * 1024)
    need(digest(manifest_bytes) == manifest_sha256, 'regression manifest differs from reviewed pin')
    manifest = json.loads(manifest_bytes, object_pairs_hook=unique)
    # Bound restoration before executing the existing worker-side archive tool.
    members, download_bytes, restored_bytes = set(), 0, 0
    need(manifest['schema'] == 'external-test-data/v1' and manifest['archives'], 'unknown input manifest')
    for archive in manifest['archives']:
        need(type(archive['archive_size']) is int and archive['archive_size'] > 0,
             'invalid archive size')
        download_bytes += archive['archive_size']
        for name, member in archive['files'].items():
            path = Path(name)
            need(not path.is_absolute() and path.as_posix() == name and '..' not in path.parts
                 and '\\' not in name and ':' not in name
                 and name not in members and name.startswith(('docs/quality/', 'docs/dogfood/',
                                                             'tools/quality/fixtures/')),
                 'escaping or repeated restored input path')
            need(type(member['size']) is int and 0 <= member['size'] <= 128 * 1024 * 1024,
                 'invalid restored input size')
            members.add(name)
            restored_bytes += member['size']
            need(len(members) <= 20000 and restored_bytes <= MAX_BYTES, 'restored inputs exceed budget')
        need(download_bytes <= MAX_BYTES, 'archive inputs exceed budget')
    need(regular(source / '.git', 4096) == ('gitdir: ' + str(history) + '\n').encode(),
         'source does not select the sanitized history view')
    need(git(source, 'rev-parse', 'HEAD', safe=(history,)).decode().strip() == head
         and not git(source, 'status', '--porcelain', '--untracked-files=no', safe=(history,)),
         'frozen candidate identity or tracked bytes changed')
    tree = git(source, 'rev-parse', 'HEAD^{tree}', safe=(history,)).decode().strip()
    # Only the fresh root-produced Git view is an eligible input, never a
    # controller mirror with remotes, credentials, includes or custom pack hooks.
    regular(history / 'config', 1024 * 1024)
    local = git(history, 'config', '--local', '--no-includes', '--null', '--list').decode()
    pairs = [item.split('\n', 1) for item in local.rstrip('\0').split('\0')]
    configuration = dict(pairs)
    need(len(pairs) == 4 and configuration == {'core.repositoryformatversion': '0', 'core.filemode': 'true',
         'core.bare': 'false', 'core.hookspath': '/dev/null'}, 'history configuration is not sanitized')
    need(all(not path.exists() and not path.is_symlink() for path in
             (history / 'objects/info/alternates', history / 'objects/info/http-alternates')),
         'external history objects are forbidden')
    need(git(history, 'rev-parse', 'HEAD').decode().strip() == head
         and git(history, 'rev-parse', 'refs/gatehouse/base').decode().strip() == base
         and git(history, 'rev-parse', 'HEAD^{tree}').decode().strip() == tree,
         'history view candidate/base/tree differs')
    git(history, 'merge-base', '--is-ancestor', base, head)
    entries = git(history, 'ls-tree', '-r', '-l', '-z', head).split(b'\0')
    total, count, tracked = 0, 0, []
    for entry in filter(None, entries):
        meta, name = entry.split(b'\t', 1)
        mode, kind, blob, size = meta.split()
        need(mode in (b'100644', b'100755') and kind == b'blob', 'linked/submodule source is unsupported')
        path = Path(os.fsdecode(name))
        need(not path.is_absolute() and '..' not in path.parts and '\\' not in str(path)
             and path.parts[0] not in ('.git', '.gatehouse-host'), 'reserved or escaping source path')
        total += int(size)
        count += 1
        need(total <= MAX_BYTES and count <= 20000, 'workspace source exceeds input budget')
        tracked.append((str(path), mode.decode(), blob.decode(), int(size)))
    def verify_files(directory):
        # Do not trust a worker-writable Git index or its skip-worktree flags.
        for name, mode, blob, size in tracked:
            path = directory / name
            data = regular(path)
            need(len(data) == size and hashlib.sha1(b'blob ' + str(size).encode() + b'\0' + data).hexdigest() == blob
                 and bool(path.stat().st_mode & 0o111) == (mode == '100755'),
                 'tracked workspace bytes or mode differ: ' + name)
    verify_files(source)
    git(destination.parent, 'init', '--template=', str(destination))
    git(destination, '-c', 'protocol.file.allow=always', 'fetch', '--no-tags',
        '--no-auto-maintenance', str(history), head, base, safe=(history,))
    git(destination, 'checkout', '--detach', head)
    git(destination, 'update-ref', 'refs/gatehouse/base', base)
    git(destination, 'config', '--local', 'core.hooksPath', '/dev/null')
    (destination / '.git/FETCH_HEAD').unlink()
    need(not (destination / '.git/objects/info/alternates').exists(), 'workspace shares external Git objects')
    command = ['/usr/bin/python3', '-B', str(destination / 'tools/maintenance/test_data.py'),
               '--root', str(destination), '--cache', str(cache), '--offline']
    # Retain failure logs outside the candidate copy; never delete/reuse a partial workspace.
    record = {'head': head, 'base': base, 'tree': tree, 'manifest_sha256': manifest_sha256,
              'workspace': str(destination), 'source_files': count, 'source_bytes': total,
              'source_inventory_sha256': digest(json.dumps(tracked, separators=(',', ':')).encode()),
              'restore_argv': command,
              'state': 'incomplete', 'scope': 'Disposable input preparation only; no quality acceptance'}
    def save():
        fd, temporary = tempfile.mkstemp(prefix='.' + log.name + '-', dir=log.parent)
        try:
            with os.fdopen(fd, 'w') as stream:
                json.dump(record, stream, sort_keys=True, indent=2)
                stream.write('\n')
                stream.flush()
                os.fsync(stream.fileno())
            os.replace(temporary, log)
        finally:
            if os.path.exists(temporary):
                os.unlink(temporary)
    save()
    result = subprocess.run(command, env=environment(), cwd=destination,
                            capture_output=True, timeout=120, check=False)
    # Only bounded diagnostics from the existing unprivileged offline helper.
    need(len(result.stdout) + len(result.stderr) <= 1024 * 1024, 'restore diagnostics exceed budget')
    record.update(restore_exit_code=result.returncode, stdout=result.stdout.decode(errors='replace'),
                  stderr=result.stderr.decode(errors='replace'))
    save()
    need(result.returncode == 0, 'offline input restoration failed; retain the workspace and preparation record')
    files, size = 0, 0
    for archive in manifest['archives']:
        for name, member in archive['files'].items():
            path = Path(name)
            need(not path.is_absolute() and '..' not in path.parts, 'escaping restored input path')
            data = regular(destination / path)
            need(len(data) == member['size'] and digest(data) == member['sha256'], 'restored input differs')
            files += 1
            size += len(data)
            need(files <= 20000 and size <= MAX_BYTES, 'restored inputs exceed budget')
    need(git(destination, 'rev-parse', 'HEAD').decode().strip() == head
         and git(destination, 'rev-parse', 'HEAD^{tree}').decode().strip() == tree
         and not git(destination, 'status', '--porcelain', '--untracked-files=no')
         and git(source, 'rev-parse', 'HEAD', safe=(history,)).decode().strip() == head
         and git(history, 'rev-parse', 'refs/gatehouse/base').decode().strip() == base
         and not git(source, 'status', '--porcelain', '--untracked-files=no', safe=(history,)),
         'tracked input changed during preparation')
    verify_files(destination)
    verify_files(source)
    record.update(state='prepared', restored_files=files, restored_bytes=size)
    save()
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for field in ('source', 'history', 'destination', 'cache'):
        parser.add_argument('--' + field, type=Path, required=True)
    for field in ('head', 'base', 'manifest-sha256'):
        parser.add_argument('--' + field, required=True)
    args = parser.parse_args()
    try:
        print(json.dumps(prepare(args.source, args.history, args.destination, args.cache,
                                 args.head, args.base, args.manifest_sha256), sort_keys=True))
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print(json.dumps({'state': 'blocked', 'reason': str(error)}))
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
