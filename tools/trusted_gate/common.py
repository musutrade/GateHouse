"""Bounded, deterministic control records for the trusted host."""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys
import tempfile
from functools import wraps
import subprocess


class Blocked(ValueError):
    """An input cannot authorize delivery."""


def checked(function):
    """Malformed/unavailable evidence is a semantic rejection, not an unhandled crash."""
    @wraps(function)
    def wrapped(*args, **kwargs):
        try:
            return function(*args, **kwargs)
        except Blocked:
            raise
        except (OSError, ValueError, KeyError, TypeError, IndexError, AttributeError, subprocess.SubprocessError):
            raise Blocked(function.__name__ + ': trusted input is malformed, missing or unavailable') from None
    return wrapped


def require(condition, message):
    if not condition:
        raise Blocked(message)


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()


def digest(value):
    return hashlib.sha256(value).hexdigest()


def sha(value):
    require(isinstance(value, str) and re.fullmatch(r'[0-9a-f]{40}', value), 'expected full Git SHA-1')
    return value


def unique(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate JSON field')
        result[key] = value
    return result


@checked
def controller_configuration(path):
    """Only the source-snapshot bootstrap can authorize installed configuration."""
    package = sys.modules.get(__package__)
    reader = getattr(package, '__host_configuration__', None)
    require(callable(reader), 'controller requires a source-bound host bootstrap')
    return json.loads(reader(path), object_pairs_hook=unique)


@checked
def read_json(path, limit=1024 * 1024):
    data = read_regular(path, limit)
    return json.loads(data, object_pairs_hook=unique)


@checked
def read_regular(path, limit):
    path = Path(path)
    for parent in (path, *path.parents):
        require(not parent.is_symlink(), 'symbolic links are forbidden in trusted paths')
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as stream:
        info = os.fstat(stream.fileno())
        require(stat.S_ISREG(info.st_mode) and info.st_size <= limit, 'not a bounded regular file')
        data = stream.read(limit + 1)
    require(len(data) <= limit, 'file exceeds limit')
    return data


@checked
def protected(path, *, directory=False, private=False):
    """Production trust requires root ownership, including every parent."""
    path = Path(path)
    require(path.is_absolute(), 'trusted paths must be absolute')
    for item in (path, *path.parents):
        info = item.lstat()
        require(not stat.S_ISLNK(info.st_mode) and info.st_uid == 0
                and not info.st_mode & 0o022, 'trusted path must be root-owned and not group/other writable')
    require(path.is_dir() if directory else path.is_file(), 'unexpected trusted path type')
    if private:
        require(not path.stat().st_mode & 0o077, 'private state/key must be inaccessible to other identities')
    return path


@checked
def submission_inbox(inbox):
    inbox = Path(inbox)
    require(inbox.is_absolute() and '..' not in inbox.parts, 'inbox must be an absolute canonical path')
    protected(inbox.parent, directory=True)
    info = inbox.lstat()
    require(stat.S_ISDIR(info.st_mode) and info.st_uid == 0 and info.st_mode & stat.S_ISVTX
            and not info.st_mode & 0o007, 'inbox must be a root-owned sticky directory without world access')
    return inbox


@checked
def read_submission(path, inbox, developer_uid, limit):
    """A delegated root entry must never turn --body-file into arbitrary root file read."""
    path, inbox = Path(path), submission_inbox(inbox)
    require(path.is_absolute() and path.parent == inbox and path.name not in ('.', '..'),
            'submission must be one file in the configured inbox')
    directory = os.open(inbox, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        fd = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=directory)
        with os.fdopen(fd, 'rb') as stream:
            item = os.fstat(stream.fileno())
            require(stat.S_ISREG(item.st_mode) and item.st_uid == developer_uid and item.st_nlink == 1
                    and item.st_size <= limit, 'submission must be a bounded developer-owned ordinary file')
            data = stream.read(limit + 1)
        require(len(data) <= limit, 'submission grew beyond limit')
        return data
    finally:
        os.close(directory)


def atomic_json(path, value):
    path = Path(path)
    fd, name = tempfile.mkstemp(prefix='.' + path.name, dir=path.parent)
    try:
        with os.fdopen(fd, 'wb') as stream:
            stream.write(canonical(value) + b'\n')
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(name, path)
        parent = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(parent)
        finally:
            os.close(parent)
    finally:
        if os.path.exists(name):
            os.unlink(name)
