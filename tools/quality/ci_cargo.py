#!/usr/bin/env python3
"""Resolve and retain CI cache identity before any compilation cache restore."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

from quality_common import CRATE, ROOT, command_output, sha256, write_json


def configuration_hash():
    paths = subprocess.check_output(
        ['git', 'ls-files', '-z', '--', '**/Cargo.toml', '**/Cargo.lock',
         '**/.cargo/*', '.cargo/*', '**/rust-toolchain*', '.github', 'tools/quality'],
        cwd=ROOT).decode().split('\0')
    return hashlib.sha256(json.dumps(
        [(name, sha256(ROOT / name)) for name in sorted(paths) if name],
        separators=(',', ':')).encode()).hexdigest()


def target_directory():
    return Path(json.loads(command_output([
        'cargo', 'metadata', '--manifest-path', str(CRATE / 'Cargo.toml'),
        '--locked', '--no-deps', '--format-version', '1']))['target_directory']).resolve()


def configure(job_class, profiles):
    if not re.fullmatch(r'[a-z][a-z0-9-]*', job_class):
        raise ValueError('invalid Cargo job class')
    target = (ROOT / 'target' / 'ci' / os.environ['RUNNER_OS'] /
              os.environ['RUNNER_ARCH'] / job_class).resolve()
    os.environ['CARGO_TARGET_DIR'] = str(target)
    actual = target_directory()
    if actual != target:
        raise ValueError(f'Cargo target mismatch: {actual} != {target}')
    cargo_home = Path(os.environ.get('CARGO_HOME', Path.home() / '.cargo')).resolve()
    identity = {
        'schema': 1, 'os': os.environ['RUNNER_OS'], 'arch': os.environ['RUNNER_ARCH'],
        'class': job_class, 'profiles': profiles, 'target_directory': str(actual),
        'rustc': command_output(['rustc', '-vV']),
        'cargo': command_output(['cargo', '--version']),
        'configuration_sha256': configuration_hash(),
        'lock_sha256': sha256(CRATE / 'Cargo.lock'),
        'cargo_home_configuration': {
            name: sha256(cargo_home / name) for name in ('config', 'config.toml')
            if (cargo_home / name).is_file()},
        'build_environment': {key: os.environ.get(key, '') for key in (
            'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_BUILD_TARGET',
            'RUSTDOCFLAGS', 'CARGO_INCREMENTAL', 'CARGO_LLVM_COV_TARGET_DIR')},
    }
    digest = hashlib.sha256(json.dumps(identity, sort_keys=True).encode()).hexdigest()
    identity['cache_identity'] = digest
    write_json(ROOT / 'target/quality/cache' / f'{job_class}.json', identity)
    with open(os.environ['GITHUB_ENV'], 'a') as output:
        output.write(f'CARGO_TARGET_DIR={target}\n')
    with open(os.environ['GITHUB_OUTPUT'], 'a') as output:
        output.write(f'target={target}\ncargo-home={cargo_home}\nidentity={digest}\nlock={identity["lock_sha256"]}\n')
    print(json.dumps(identity, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--class', dest='job_class', required=True)
    parser.add_argument('--profiles', required=True)
    args = parser.parse_args()
    configure(args.job_class, args.profiles)
