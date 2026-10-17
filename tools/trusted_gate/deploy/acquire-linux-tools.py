#!/usr/bin/python3 -I
"""Acquire pinned official Linux tool archives for review; never extract/install."""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import sys
import time
import urllib.parse
import urllib.request

SPEC = importlib.util.spec_from_file_location('tool_archive_inputs',
    Path(__file__).with_name('verify-locked-crates.py'))
INPUTS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(INPUTS)
need = INPUTS.need
MAX_ARCHIVE = 512 * 1024 * 1024
MAX_TOTAL = 2 * 1024 * 1024 * 1024
RUST = {'rustc': 'rustc', 'cargo': 'cargo', 'rust-std': 'rust-std',
        'rustfmt-preview': 'rustfmt', 'clippy-preview': 'clippy', 'llvm-tools-preview': 'llvm-tools'}
TARGET = 'x86_64-unknown-linux-gnu'
VERSIONS = {'rust': '1.99.1', 'cargo-nextest': '0.9.143', 'node': '24.18.0'}
RUST_BASE = 'https://static.rust-lang.org/dist/2026-10-15/'
NEXTEST_BASE = 'https://github.com/nextest-rs/nextest/releases/download/cargo-nextest-0.9.143/'


def lock(path, expected_sha256):
    raw = INPUTS.read(path)
    need(isinstance(expected_sha256, str) and re.fullmatch('[0-9a-f]{64}', expected_sha256)
         and hashlib.sha256(raw).hexdigest() == expected_sha256, 'tool lock differs from reviewed digest')
    value = json.loads(raw, object_pairs_hook=INPUTS.unique)
    need(set(value) == {'schema', 'target', 'versions', 'metadata', 'artifacts', 'scope'}
         and value['schema'] == 'gatehouse-linux-tool-archives/v1'
         and value['target'] == TARGET and value['versions'] == VERSIONS, 'unsupported tool lock')
    expected_urls = {name: RUST_BASE + stem + '-1.99.1-' + TARGET + '.tar.xz'
                     for name, stem in RUST.items()}
    expected_urls.update({'cargo-nextest': NEXTEST_BASE + 'cargo-nextest-0.9.143-' + TARGET + '.tar.gz',
                         'node': 'https://nodejs.org/dist/v24.18.0/node-v24.18.0-linux-x64.tar.xz'})
    rows = value['artifacts']
    need(isinstance(rows, dict) and rows.keys() == expected_urls.keys(), 'tool archive inventory differs')
    for role, row in rows.items():
        need(isinstance(row, dict) and set(row) == {'url', 'sha256', 'max_bytes'}
             and row['url'] == expected_urls[role]
             and isinstance(row['sha256'], str) and re.fullmatch('[0-9a-f]{64}', row['sha256'])
             and type(row['max_bytes']) is int and 0 < row['max_bytes'] <= MAX_ARCHIVE,
             'invalid tool archive pin: ' + role)
    return raw, rows


class ReviewedRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, message, headers, newurl):
        before, after = urllib.parse.urlparse(request.full_url), urllib.parse.urlparse(newurl)
        allowed = {before.hostname}
        if before.hostname in ('github.com', 'release-assets.githubusercontent.com'):
            allowed |= {'release-assets.githubusercontent.com'}
        need(after.scheme == 'https' and after.hostname in allowed and after.port in (None, 443)
             and after.username is None and after.password is None and not after.fragment,
             'unreviewed tool download redirect')
        return super().redirect_request(request, fp, code, message, headers, newurl)


def download(row, destination, remaining):
    limit = min(row['max_bytes'], remaining)
    need(limit > 0, 'combined tool archive budget exhausted')
    # Ordinary operator network settings may route HTTPS; never copy them into
    # a manifest, report or deployment. The independently pinned bytes decide.
    opener = urllib.request.build_opener(ReviewedRedirect())
    request = urllib.request.Request(row['url'], headers={'User-Agent': 'GateHouse-input-review'})
    digest, size, started = hashlib.sha256(), 0, time.monotonic()
    with opener.open(request, timeout=30) as source, destination.open('xb') as target:
        os.fchmod(target.fileno(), 0o600)
        declared = source.headers.get('Content-Length')
        if declared is not None:
            need(declared.isdecimal() and 0 < int(declared) <= limit, 'declared tool archive exceeds budget')
        while block := source.read(1024 * 1024):
            need(time.monotonic() - started <= 180, 'tool archive acquisition timed out')
            size += len(block)
            need(size <= limit, 'tool archive exceeds budget')
            digest.update(block)
            target.write(block)
        need(size > 0 and (declared is None or int(declared) == size), 'truncated or empty tool archive')
        need(digest.hexdigest() == row['sha256'], 'tool archive differs from reviewed checksum')
        target.flush()
        os.fsync(target.fileno())
        os.fchmod(target.fileno(), 0o444)
    return size


def record(path, value):
    temporary = path.with_name('.' + path.name + '.tmp')
    fd = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'w') as stream:
        json.dump(value, stream, sort_keys=True, indent=2)
        stream.write('\n')
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temporary, path)


def acquire(manifest, expected_sha256, destination, progress=None):
    need(os.geteuid() != 0, 'archive acquisition candidate requires the unprivileged operator; never installs')
    raw, rows = lock(manifest, expected_sha256)
    destination = Path(destination)
    need(destination.is_absolute() and '..' not in destination.parts, 'absolute fresh acquisition path required')
    INPUTS.plain(destination.parent, directory=True)
    state = destination.with_name(destination.name + '-acquisition.json')
    need(not destination.exists() and not destination.is_symlink()
         and not state.exists() and not state.is_symlink(), 'retain old or partial tool acquisition; never reuse')
    destination.mkdir(mode=0o700)
    value = {'schema': 'gatehouse-tool-acquisition/v1', 'state': 'incomplete',
             'tool_lock_sha256': expected_sha256, 'archives': {}, 'bytes': 0,
             'scope': 'unprivileged pinned archive acquisition; no extraction, install, signatures or quality acceptance'}
    record(state, value)
    try:
        with (destination / 'inputs.lock.json').open('xb') as output:
            output.write(raw)
            os.fchmod(output.fileno(), 0o444)
        for role, row in sorted(rows.items()):
            name = urllib.parse.urlparse(row['url']).path.rsplit('/', 1)[1]
            INPUTS.safe_path(name)
            temporary = destination / (name + '.partial')
            size = download(row, temporary, MAX_TOTAL - value['bytes'])
            os.rename(temporary, destination / name)
            value['archives'][role] = {'file': name, 'sha256': row['sha256'], 'bytes': size}
            value['bytes'] += size
            record(state, value)
            if progress:
                progress(role, size)
        need(INPUTS.read(manifest) == raw, 'tool lock changed during acquisition')
        destination.chmod(0o555)
        value['state'] = 'acquired'
        record(state, value)
        return value
    except BaseException:
        # Retain partial files and completed archives. Errors may contain signed
        # redirect/proxy URLs, so never serialize exception messages or secrets.
        value['state'] = 'blocked'
        record(state, value)
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--lock', type=Path, default=Path(__file__).with_name('linux-tools.lock.json'))
    parser.add_argument('--lock-sha256', required=True)
    parser.add_argument('--destination', type=Path, required=True)
    args = parser.parse_args()
    try:
        value = acquire(args.lock, args.lock_sha256, args.destination,
            progress=lambda role, size: print(f'acquired {role}: {size} bytes', file=sys.stderr, flush=True))
    except Exception as error:
        parser.exit(1, 'tool acquisition blocked (' + type(error).__name__ + '); retain private partial state\n')
    print(json.dumps(value, sort_keys=True))


if __name__ == '__main__':
    main()
