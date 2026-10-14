"""Replay an original signed tar from private payload bytes, without storing it.

The receipt is untrusted. Its stream must match the signed archive digest AND
its parsed members must match the manifest and actual installed file modes.
"""
import base64
import hashlib
import io
from pathlib import Path
import stat
import tarfile

import collector_assets as assets

NAME = 'archive-receipt.json'
MAX_ARCHIVE = 8 * 1024**3
MAX_HEADERS = 16 * 1024**2


def regular(path):
    """Private payloads may share inodes; metadata and external inputs may not."""
    assets.require(stat.S_ISREG(path.lstat().st_mode), 'unsafe installed payload link/type')


def create(archive):
    rows, cursor = [], 0
    with archive.open('rb') as raw, tarfile.open(archive, 'r:') as source:
        for entry in source:
            if entry.isdir():
                continue
            assets.require(entry.isfile(), 'receipt requires regular payloads')
            name = assets.safe_name(entry.name)
            raw.seek(cursor)
            prefix = raw.read(entry.offset_data - cursor)
            rows.append({'path': name, 'size': entry.size,
                         'prefix': base64.b64encode(prefix).decode()})
            cursor = entry.offset_data + entry.size
        raw.seek(cursor)
        tail = base64.b64encode(raw.read()).decode()
    return {'schema': 'rust-collector-archive-receipt/v1', 'size': archive.stat().st_size,
            'rows': rows, 'tail': tail}


def chunks(receipt, root):
    assets.require(receipt['schema'] == 'rust-collector-archive-receipt/v1', 'unknown archive receipt')
    assets.require(type(receipt['size']) is int and 0 < receipt['size'] <= MAX_ARCHIVE,
                   'invalid receipt archive size')
    seen, headers, total = set(), 0, 0
    for row in receipt['rows']:
        name = assets.safe_name(row['path'])
        assets.require(name not in seen, 'duplicate receipt payload')
        seen.add(name)
        path = root / name
        for parent in path.parents:
            if parent == root:
                break
            assets.require(not parent.is_symlink(), 'unsafe payload ancestor')
        regular(path)
        size = row['size']
        assets.require(type(size) is int and size >= 0 and path.stat().st_size == size,
                       'receipt payload checksum/size mismatch')
        prefix = base64.b64decode(row['prefix'], validate=True)
        headers += len(prefix)
        total += len(prefix) + size
        assets.require(headers <= MAX_HEADERS and total <= receipt['size'], 'receipt exceeds bounds')
        yield prefix
        with path.open('rb') as source:
            while data := source.read(1024 * 1024):
                yield data
    tail = base64.b64decode(receipt['tail'], validate=True)
    assets.require(headers + len(tail) <= MAX_HEADERS and total + len(tail) == receipt['size'],
                   'receipt tail size mismatch')
    yield tail


class Replay(io.RawIOBase):
    def __init__(self, source):
        self.source = iter(source)
        self.pending = b''
        self.digest = hashlib.sha256()

    def readable(self):
        return True

    def readinto(self, buffer):
        while not self.pending:
            try:
                self.pending = next(self.source)
            except StopIteration:
                return 0
        count = min(len(buffer), len(self.pending))
        buffer[:count] = self.pending[:count]
        self.digest.update(self.pending[:count])
        self.pending = self.pending[count:]
        return count


def verify(receipt, root, manifest):
    expected = {p['path']: p['sha256'] for p in manifest['payloads']}
    assets.require({r['path'] for r in receipt['rows']} == set(expected), 'receipt payload inventory mismatch')
    source = Replay(chunks(receipt, root))
    stream = io.BufferedReader(source)
    seen = set()
    with tarfile.open(fileobj=stream, mode='r|') as archive:
        for member in archive:
            name = assets.safe_name(member.name.rstrip('/') if member.isdir() else member.name)
            if member.isdir():
                assets.require(any(p.startswith(name + '/') for p in expected), 'extra archive directory')
                continue
            assets.require(member.isfile() and name in expected and name not in seen,
                           'invalid replay archive member')
            seen.add(name)
            mode = (root / name).stat()
            assets.require(member.size == mode.st_size and member.mode == stat.S_IMODE(mode.st_mode),
                           'installed payload mode/size mismatch')
            digest = hashlib.file_digest(archive.extractfile(member), 'sha256').hexdigest()
            assets.require(digest == expected[name], 'installed payload checksum mismatch')
    assets.require(seen == set(expected), 'missing replay archive payload')
    while stream.read(1024 * 1024):
        pass
    return source.digest.hexdigest()


def export(receipt, root, destination):
    with destination.open('xb') as output:
        for data in chunks(receipt, root):
            output.write(data)
