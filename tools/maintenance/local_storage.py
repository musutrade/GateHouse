#!/usr/bin/env python3
"""Explicit, reviewable cleanup of registered regenerable local directories."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import stat
import subprocess
import sys
import time


class SafetyError(ValueError):
    """Cleanup cannot establish the required safety conditions."""


def git(root, *args):
    return subprocess.check_output(['git', '-C', str(root), *args], stderr=subprocess.PIPE)


def repository(root):
    root = Path(root).absolute()
    if root != root.resolve(strict=True):
        raise SafetyError('repository root must not use symlinks')
    if Path(os.fsdecode(git(root, 'rev-parse', '--show-toplevel')).strip()) != root:
        raise SafetyError('--root must be the exact repository root')
    return root


def git_state(root):
    head = git(root, 'rev-parse', 'HEAD').decode().strip()
    index = git(root, 'ls-files', '--stage', '-z')
    names = git(root, 'ls-files', '-z') + git(root, 'ls-tree', '-r', '--name-only', '-z', 'HEAD')
    return {'head': head, 'index_sha256': hashlib.sha256(index).hexdigest()}, {
        os.fsdecode(name) for name in names.split(b'\0') if name}


def relative_path(value):
    if not isinstance(value, str) or not value or '\\' in value:
        raise SafetyError('entry path must be a nonempty POSIX relative path')
    path = PurePosixPath(value)
    if path.is_absolute() or str(path) != value or any(p in ('.', '..', '.git') for p in path.parts):
        raise SafetyError(f'unsafe relative path: {value!r}')
    if not path.parts:
        raise SafetyError('repository root cannot be cleaned')
    return value


def below(path, parent):
    return path == parent or path.startswith(parent + '/')


def select(manifest):
    if manifest.get('version') != 1 or not isinstance(manifest.get('entries'), list):
        raise SafetyError('manifest requires version=1 and entries=[]')
    rows, groups = [], {}
    for entry in manifest['entries']:
        if not isinstance(entry, dict):
            raise SafetyError('each manifest entry must be an object')
        path = relative_path(entry.get('path'))
        if any(below(path, row['path']) or below(row['path'], path) for row in rows):
            raise SafetyError(f'duplicate or nested manifest path: {path}')
        row = {'path': path, 'action': 'keep', 'reason': ''}
        rows.append(row)
        if entry.get('kind') not in ('cache', 'run'):
            row['reason'] = 'unknown, evidence, and backup categories are protected'
        elif not (entry.get('reviewed') is True and entry.get('reproducible') is True
                  and entry.get('contains_evidence') is False and entry.get('contains_backup') is False
                  and isinstance(entry.get('reason'), str) and entry['reason'].strip()):
            row['reason'] = 'explicit review, regenerability, and absence of evidence/backups required'
        elif entry.get('protected', False) is not False:
            row['reason'] = 'explicit protection'
        elif entry['kind'] == 'run':
            if not (entry.get('state') == 'completed' and entry.get('failure_closed') is True
                    and entry.get('archived') is True and entry.get('review_pending') is False):
                row['reason'] = 'run is active, pending review, unarchived, or has unresolved failure'
            elif (not isinstance(entry.get('group'), str) or not entry['group'].strip()
                  or type(entry.get('completed_at')) not in (int, float)
                  or not 0 < entry['completed_at'] <= time.time()):
                row['reason'] = 'run needs a group and a valid completion Unix timestamp'
            else:
                groups.setdefault(entry['group'], []).append((entry['completed_at'], row))
        else:
            row.update(action='delete', reason=entry['reason'])
    for group in groups.values():
        for i, (_, row) in enumerate(sorted(group, key=lambda item: (-item[0], item[1]['path']))):
            row.update(action='keep' if i < 2 else 'delete',
                       reason='retain the two most recent eligible runs' if i < 2 else 'older completed regenerable run')
    return rows


def snapshot(root, relative, tracked):
    path = root / relative
    for name in tracked:
        if below(name, relative) or below(relative, name):
            raise SafetyError(f'tracked content intersects candidate: {relative}')
    current = root
    root_info = root.stat()
    ancestors = []
    for part in PurePosixPath(relative).parts:
        current /= part
        info = current.lstat()
        if info.st_dev != root_info.st_dev:
            raise SafetyError(f'mounted candidate rejected: {relative}')
        if not stat.S_ISDIR(info.st_mode):
            raise SafetyError(f'candidate ancestors must be real directories: {relative}')
        ancestors.append([info.st_dev, info.st_ino])
    digest, logical, allocated, seen, inodes = hashlib.sha256(), 0, 0, set(), set()
    todo = [path]
    while todo:
        item = todo.pop()
        info = item.lstat()
        if info.st_uid != root_info.st_uid:
            raise SafetyError(f'candidate contains another owner: {item}')
        if info.st_dev != ancestors[-1][0]:
            raise SafetyError(f'nested mount rejected: {relative}')
        if item.name == '.git' or not (stat.S_ISDIR(info.st_mode) or stat.S_ISREG(info.st_mode)):
            raise SafetyError(f'symlink, special file, or nested repository rejected: {item}')
        metadata = [str(item.relative_to(path)), info.st_dev, info.st_ino, info.st_mode,
                    info.st_size, info.st_mtime_ns, info.st_ctime_ns, info.st_nlink]
        digest.update(json.dumps(metadata, ensure_ascii=True).encode() + b'\n')
        inode = (info.st_dev, info.st_ino)
        inodes.add(inode)
        if inode not in seen:
            seen.add(inode)
            logical += info.st_size
            allocated += info.st_blocks * 512
        if stat.S_ISDIR(info.st_mode):
            todo.extend(sorted(item.iterdir(), reverse=True))
    return {'sha256': digest.hexdigest(), 'ancestors': ancestors, 'entries': len(inodes),
            'logical_bytes': logical, 'allocated_bytes_upper_bound': allocated}, inodes


def process_identity(process):
    fields = (process / 'stat').read_text().rsplit(')', 1)[1].split()
    return {'pid': int(process.name), 'starttime': int(fields[19]),
            'uid': process.stat().st_uid, 'comm': (process / 'comm').read_text().rstrip('\n'),
            'cmdline': [os.fsdecode(part) for part in (process / 'cmdline').read_bytes().split(b'\0') if part],
            'cgroup': (process / 'cgroup').read_text().rstrip('\n'), 'ppid': int(fields[1])}


def active_references(paths, inodes, owner_uid, exemptions=(), proc=Path('/proc')):
    """Inspect observable processes; exact reviewed instances may waive unreadable owners."""
    if sys.platform != 'linux' or not proc.is_dir():
        raise SafetyError('activity inspection requires Linux /proc; unsupported hosts are read-only')
    required = {'pid', 'starttime', 'uid', 'comm', 'cmdline', 'cgroup', 'ppid', 'reason'}
    if (not isinstance(exemptions, (list, tuple)) or any(
            not isinstance(item, dict) or set(item) != required
            or not isinstance(item['reason'], str) or not item['reason'].strip() for item in exemptions)):
        raise SafetyError('process exemptions require exact process identity and a review reason')
    receipt = {'inspected_processes': 0, 'used_process_exemptions': [],
               'other_uid_unobservable': [], 'maintenance_window_required': True}
    for process in proc.iterdir():
        if not process.name.isdecimal() or int(process.name) == os.getpid():
            continue
        try:
            uid = process.stat().st_uid
            state = (process / 'stat').read_text().rsplit(')', 1)[1].split()[0]
            if state == 'Z':
                continue  # Zombies have no executable, file descriptors, or address space.
            references, unreadable = [], []
            for name in ('cwd', 'exe', 'root'):
                try:
                    references.append(os.readlink(process / name))
                except FileNotFoundError:
                    pass
                except PermissionError:
                    unreadable.append(name)
            try:
                for fd in (process / 'fd').iterdir():
                    try:
                        references.append(os.readlink(fd))
                        info = fd.stat()
                        if (info.st_dev, info.st_ino) in inodes:
                            raise SafetyError(f'active file descriptor in PID {process.name}')
                    except FileNotFoundError:
                        pass
            except PermissionError:
                unreadable.append('fd')
            try:
                for line in (process / 'maps').read_text().splitlines():
                    fields = line.split(maxsplit=5)
                    if len(fields) == 6 and fields[5].startswith('/'):
                        references.append(fields[5])
                    if len(fields) >= 5:
                        major, minor = (int(value, 16) for value in fields[3].split(':'))
                        if (os.makedev(major, minor), int(fields[4])) in inodes:
                            raise SafetyError(f'active memory mapping in PID {process.name}')
            except PermissionError:
                unreadable.append('maps')
            for ref in references:
                if ref.endswith(' (deleted)'):
                    ref = ref[:-10]
                if any(below(ref, path) for path in paths):
                    raise SafetyError(f'active path in PID {process.name}: {ref}')
            if unreadable:
                if uid != owner_uid and os.getuid() != 0:
                    receipt['other_uid_unobservable'].append(int(process.name))
                    continue
                identity = process_identity(process)
                match = next((item for item in exemptions if
                              {key: item[key] for key in identity} == identity), None)
                if match is None:
                    raise SafetyError(f'cannot inspect owner PID {process.name}; cleanup refused')
                receipt['used_process_exemptions'].append(match)
            else:
                receipt['inspected_processes'] += 1
        except FileNotFoundError:
            continue  # Process exited during inspection.
        except PermissionError as exc:
            raise SafetyError(f'cannot establish process identity for PID {process.name}') from exc
    for line in (proc / 'locks').read_text().splitlines():
        for token in line.split():
            pieces = token.split(':')
            if len(pieces) != 3:
                continue
            try:
                inode = (os.makedev(int(pieces[0], 16), int(pieces[1], 16)), int(pieces[2]))
            except ValueError:
                continue
            if inode in inodes:
                raise SafetyError('active kernel file lock within a candidate')
    return receipt


def inspect_candidates(root, rows, tracked, exemptions=()):
    inodes, paths = set(), []
    for row in rows:
        if row['action'] != 'delete':
            continue
        path = root / row['path']
        info = path.lstat()
        if info.st_uid != root.stat().st_uid:
            raise SafetyError(f'candidate belongs to another user: {row["path"]}')
        row['snapshot'], objects = snapshot(root, row['path'], tracked)
        paths.append(str(path))
        inodes.update(objects)
    if paths:
        return active_references(paths, inodes, root.stat().st_uid, exemptions)
    return {'inspected_processes': 0, 'used_process_exemptions': [],
            'other_uid_unobservable': [], 'maintenance_window_required': True}


def make_plan(root, manifest, ttl=3600):
    if type(ttl) is not int or not 1 <= ttl <= 86400:
        raise SafetyError('TTL must be between 1 and 86400 seconds')
    root = repository(root)
    state, tracked = git_state(root)
    rows = select(manifest)
    receipt = inspect_candidates(root, rows, tracked, manifest.get('process_exemptions', []))
    now = time.time()
    return {'version': 1, 'root': str(root), 'created_at': now, 'expires_at': now + ttl,
            'git': state, 'manifest': manifest, 'entries': rows, 'activity': receipt,
            'root_identity': [root.stat().st_dev, root.stat().st_ino]}


def open_directory(path):
    descriptor = os.open('/', os.O_RDONLY | os.O_DIRECTORY)
    try:
        for part in path.parts[1:]:
            next_descriptor = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                                      dir_fd=descriptor)
            os.close(descriptor)
            descriptor = next_descriptor
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def apply(plan, execute=False):
    if plan.get('version') != 1:
        raise SafetyError('unsupported plan version')
    created, expiry = plan['created_at'], plan['expires_at']
    if not created <= time.time() <= expiry or not 0 < expiry - created <= 86400:
        raise SafetyError('plan expired or has an invalid lifetime; create a new plan')
    fresh = make_plan(plan['root'], plan['manifest'])
    if (fresh['git'] != plan['git'] or fresh['entries'] != plan['entries']
            or fresh['root_identity'] != plan['root_identity']):
        raise SafetyError('plan is stale: Git, candidate contents, or retention selection changed')
    deleted = []
    if execute:
        if not shutil.rmtree.avoids_symlink_attacks:
            raise SafetyError('this Python lacks descriptor-safe directory deletion')
        for row in fresh['entries']:
            if row['action'] != 'delete':
                continue
            root = Path(fresh['root'])
            # Reinspect just before each deletion. This is not a lock on arbitrary builders;
            # callers must keep builds/runs stopped during the operation.
            state, tracked = git_state(root)
            if state != fresh['git']:
                raise SafetyError('Git changed during apply')
            check = {'path': row['path'], 'action': 'delete'}
            fresh['activity'] = inspect_candidates(
                root, [check], tracked, plan['manifest'].get('process_exemptions', []))
            if check['snapshot'] != row['snapshot']:
                raise SafetyError('candidate changed immediately before deletion')
            parts = PurePosixPath(row['path']).parts
            descriptor = open_directory(root)
            info = os.fstat(descriptor)
            if [info.st_dev, info.st_ino] != fresh['root_identity']:
                os.close(descriptor)
                raise SafetyError('repository root was replaced')
            try:
                for part, identity in zip(parts[:-1], row['snapshot']['ancestors']):
                    next_descriptor = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                                              dir_fd=descriptor)
                    os.close(descriptor)
                    descriptor = next_descriptor
                    info = os.fstat(descriptor)
                    if [info.st_dev, info.st_ino] != identity:
                        raise SafetyError('candidate ancestor was replaced')
                info = os.stat(parts[-1], dir_fd=descriptor, follow_symlinks=False)
                if [info.st_dev, info.st_ino] != row['snapshot']['ancestors'][-1]:
                    raise SafetyError('candidate root was replaced')
                shutil.rmtree(parts[-1], dir_fd=descriptor)
                deleted.append(row['path'])
            finally:
                os.close(descriptor)
    return {'executed': execute, 'deleted': deleted, 'entries': fresh['entries'],
            'activity': fresh['activity']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    planning = commands.add_parser('plan')
    planning.add_argument('--root', required=True)
    planning.add_argument('--manifest', required=True, type=Path)
    planning.add_argument('--output', required=True, type=Path)
    planning.add_argument('--ttl', type=int, default=3600)
    applying = commands.add_parser('apply')
    applying.add_argument('--plan', required=True, type=Path)
    applying.add_argument('--execute', action='store_true', help='delete only after revalidation')
    args = parser.parse_args()
    try:
        if args.command == 'plan':
            plan = make_plan(args.root, json.loads(args.manifest.read_text()), args.ttl)
            # Never put a plan into a candidate or replace a preexisting report.
            output = args.output.absolute()
            if any(below(str(output.resolve()), str(Path(plan['root']) / row['path']))
                   for row in plan['entries']):
                raise SafetyError('plan output must be outside every registered directory')
            with output.open('x') as stream:
                json.dump(plan, stream, indent=2)
                stream.write('\n')
            print(json.dumps(plan, indent=2))
        else:
            print(json.dumps(apply(json.loads(args.plan.read_text()), args.execute), indent=2))
    except (SafetyError, OSError, ValueError, KeyError, subprocess.CalledProcessError) as exc:
        print(f'local-storage: {exc}', file=sys.stderr)
        return 2
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
