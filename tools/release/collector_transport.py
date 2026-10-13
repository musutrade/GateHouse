"""Compressed, deduplicated transport of an unchanged signed collector archive.

Transport descriptors are not trust roots. Reconstruction must match the archive
digest in the authenticated installer catalog and pass the original dual verifier.
"""
import base64
import gzip
import hashlib
import io
import json
from pathlib import Path
import re
import shutil
import tarfile

import collector_assets as assets

TOOLS = ('rust/', 'link/', 'python/', 'lib/', 'licenses/rust/', 'licenses/crates/')


def blob_name(value):
    assets.require(isinstance(value, str) and re.fullmatch('[0-9a-f]{64}', value), 'invalid blob digest')
    return value


def pack(archive, output):
    """Store equal file contents once; preserve original tar headers byte for byte."""
    output.mkdir(parents=True, exist_ok=False)
    groups = {'toolchain': {}, 'plugin': {}}
    recipe, cursor = [], 0
    with archive.open('rb') as raw, tarfile.open(archive) as source:
        for entry in source:
            if entry.isdir():
                assets.safe_name(entry.name.rstrip('/'))
                continue
            assets.require(entry.isfile(), 'transport requires regular archive payloads')
            assets.safe_name(entry.name)
            raw.seek(cursor)
            prefix = raw.read(entry.offset_data - cursor)
            data = raw.read(entry.size)
            digest = hashlib.sha256(data).hexdigest()
            group = 'toolchain' if entry.name.startswith(TOOLS) else 'plugin'
            groups[group].setdefault(digest, (entry.offset_data, entry.size))
            recipe.append({'prefix': base64.b64encode(prefix).decode(), 'blob': digest, 'size': entry.size})
            cursor = entry.offset_data + entry.size
        raw.seek(cursor)
        tail = base64.b64encode(raw.read()).decode()
    # A shared blob always belongs to the reusable toolchain layer.
    for digest in groups['toolchain']:
        groups['plugin'].pop(digest, None)
    bundles = []
    for group, objects in groups.items():
        path = output / (group + '.tar.gz')
        with archive.open('rb') as raw, path.open('xb') as compressed:
            with gzip.GzipFile(filename='', fileobj=compressed, mode='wb', mtime=0, compresslevel=6) as gz:
                with tarfile.open(fileobj=gz, mode='w|') as target:
                    for digest, (offset, size) in sorted(objects.items()):
                        raw.seek(offset)
                        info = tarfile.TarInfo(digest)
                        info.size, info.mode = size, 0o600
                        target.addfile(info, io.BytesIO(raw.read(size)))
        digest = assets.sha(path)
        name = group + '-' + digest + '.tar.gz'
        path.rename(output / name)
        bundles.append({'name': name, 'sha256': digest, 'size': (output / name).stat().st_size,
                        'objects': {key: value[1] for key, value in sorted(objects.items())}})
    descriptor = {'schema': 'rust-collector-transport/v1', 'archive_sha256': assets.sha(archive),
                  'archive_size': archive.stat().st_size, 'bundles': bundles, 'recipe': recipe, 'tail': tail}
    assets.write(output / 'transport.json', descriptor)
    return descriptor


def reconstruct(descriptor, bundles, destination, cache):
    """No archive-selected paths, links, executables, or unbounded decompression."""
    assets.require(descriptor['schema'] == 'rust-collector-transport/v1', 'unknown transport')
    limit = descriptor['archive_size']
    assets.require(isinstance(limit, int) and 0 < limit <= 8 * 1024**3, 'invalid archive limit')
    cache.mkdir(parents=True, exist_ok=True, mode=0o700)
    assets.require(not cache.is_symlink(), 'unsafe blob cache')
    sizes = {}
    for bundle in descriptor['bundles']:
        for digest, size in bundle['objects'].items():
            blob_name(digest)
            assets.require(type(size) is int and 0 <= size <= limit, 'invalid object size')
            assets.require(digest not in sizes, 'duplicate object ownership')
            sizes[digest] = size
    assets.require(sum(sizes.values()) <= limit, 'expanded objects exceed archive bound')
    for bundle in descriptor['bundles']:
        path = bundles / assets.safe_name(bundle['name'])
        assets.regular(path)
        assets.require(assets.sha(path) == bundle['sha256'] and path.stat().st_size == bundle['size'], 'bundle checksum mismatch')
        found = set()
        with tarfile.open(path, 'r|gz') as archive:
            for entry in archive:
                digest = blob_name(entry.name)
                assets.require(entry.isfile() and digest in bundle['objects'] and digest not in found
                               and entry.size == sizes[digest], 'invalid bundle entry')
                found.add(digest)
                target = cache / digest
                if target.exists() or target.is_symlink():
                    assets.regular(target)
                    assets.require(target.stat().st_size == entry.size and assets.sha(target) == digest, 'corrupt cached blob')
                    continue
                temporary = cache / (digest + '.part')
                assets.require(not temporary.exists() and not temporary.is_symlink(), 'stale blob write')
                try:
                    with temporary.open('xb') as stream:
                        shutil.copyfileobj(archive.extractfile(entry), stream)
                    assets.require(assets.sha(temporary) == digest, 'blob checksum mismatch')
                    temporary.rename(target)
                finally:
                    temporary.unlink(missing_ok=True)
        assets.require(found == set(bundle['objects']), 'incomplete bundle')
    with destination.open('xb') as output:
        for row in descriptor['recipe']:
            digest = blob_name(row['blob'])
            assets.require(digest in sizes and row['size'] == sizes[digest], 'unknown recipe object')
            prefix = base64.b64decode(row['prefix'], validate=True)
            assets.require(output.tell() + len(prefix) + row['size'] <= limit, 'recipe exceeds archive bound')
            output.write(prefix)
            with (cache / digest).open('rb') as stream:
                shutil.copyfileobj(stream, output)
        tail = base64.b64decode(descriptor['tail'], validate=True)
        assets.require(output.tell() + len(tail) == limit, 'wrong reconstructed size')
        output.write(tail)
    assets.require(assets.sha(destination) == descriptor['archive_sha256'], 'reconstructed archive checksum mismatch')
