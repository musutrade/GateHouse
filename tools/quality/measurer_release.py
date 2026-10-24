"""One fixed release declaration for Core download, trust and provenance."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tarfile
import tempfile

from production_coverage import require
from quality_common import ROOT, sha256

MEASURER_PIN = ROOT / 'tools/quality/rust-measure-release.json'
IDENTITY = {'analyzer': 'harness-gate-rust-measure', 'version': '0.4.0',
            'rule': 'mccabe-rust-3', 'executable': 'harness-gate-rust-measure'}


def load_measurer_pin(path: Path = MEASURER_PIN) -> dict:
    return validate_pin(json.loads(path.read_text()))


def validate_pin(pin: dict) -> dict:
    require(isinstance(pin, dict) and pin.get('schema') == 'rust-measure-release-pin/v2',
            'unknown measurer pin schema')
    for key, value in IDENTITY.items():
        require(pin.get(key) == value, f'unsupported measurer pin {key}')
    for key, pattern in {'repository': r'[A-Za-z0-9_-]+/[A-Za-z0-9_.-]+',
                         'release': r'[A-Za-z0-9][A-Za-z0-9._-]*',
                         'sha256': r'[0-9a-f]{64}', 'binary_sha256': r'[0-9a-f]{64}',
                         'source_commit': r'[0-9a-f]{40}',
                         'workflow': r'\.github/workflows/[A-Za-z0-9_-]+\.yml'}.items():
        require(isinstance(pin.get(key), str) and re.fullmatch(pattern, pin[key]) is not None,
                f'invalid measurer pin {key}')
    require(pin['release'] != 'latest', 'unfixed measurer release')
    require(pin.get('asset') == f'harness-gate-rust-measure-{pin["version"]}-linux-amd64.tar.gz',
            'measurer pin asset/version mismatch')
    require(pin.get('oidc_issuer') == 'https://token.actions.githubusercontent.com',
            'unsupported measurer OIDC issuer')
    require(pin.get('certificate_identity') ==
            f'https://github.com/{pin["repository"]}/{pin["workflow"]}@refs/tags/{pin["release"]}',
            'measurer certificate identity mismatch')
    return pin


def verify_measurer_asset(path: Path, pin: dict) -> None:
    require(sha256(path) == pin['sha256'], 'pinned measurer asset digest mismatch')


def release_url(pin: dict, asset: str) -> str:
    return f'https://github.com/{pin["repository"]}/releases/download/{pin["release"]}/{asset}'


def run(name, argv):
    subprocess.run(argv, check=True, timeout=180)


def verify_attestation(path: Path, pin: dict, command=run) -> None:
    # gh verifies the signed SLSA subject and certificate against Sigstore trust.
    # Bind both source and signer digests, exact ref, workflow SAN and OIDC issuer.
    command('measurer-attestation-' + path.name, ['gh', 'attestation', 'verify', str(path),
        '--repo', pin['repository'], '--cert-identity', pin['certificate_identity'],
        '--cert-oidc-issuer', pin['oidc_issuer'], '--source-ref', 'refs/tags/' + pin['release'],
        '--source-digest', pin['source_commit'], '--signer-digest', pin['source_commit'],
        '--deny-self-hosted-runners'])


def extract_measurer(archive: Path, cache: Path, pin: dict) -> Path:
    """Validate every member before writing into a fresh, bounded directory."""
    verify_measurer_asset(archive, pin)
    root = pin['asset'].removesuffix('.tar.gz')
    with tempfile.TemporaryDirectory(prefix='extract-', dir=cache) as directory:
        destination = Path(directory)
        with tarfile.open(archive) as source:
            members, size = [], 0
            for member in source:
                members.append(member)
                size += member.size
                require(len(members) <= 32 and size <= 32 * 1024 * 1024,
                        'measurer archive exceeds extraction budget')
            require(members, 'empty measurer release archive')
            names = set()
            for member in members:
                parts = Path(member.name).parts
                require(not member.name.startswith('/') and '..' not in parts and '\\' not in member.name,
                        f'unsafe measurer archive member: {member.name}')
                require(member.isfile() or member.isdir(), f'unsupported measurer archive member: {member.name}')
                require(parts and parts[0] == root, f'measurer archive member outside the release root: {member.name}')
                require(parts not in names, 'duplicate measurer archive member')
                names.add(parts)
            source.extractall(destination, filter='data')
        binary = destination / root / pin['executable']
        require(binary.is_file(), 'pinned measurer executable missing from the release archive')
        require(os.access(binary, os.X_OK), 'pinned measurer executable is not executable')
        require(sha256(binary) == pin['binary_sha256'], 'measurer binary digest mismatch')
        manifest = json.loads((binary.parent / 'plugin.json').read_text())
        require(manifest.get('schema') == 'rust-measure-plugin/v1', 'unknown measurer capability schema')
        require(set(manifest) == {'schema', 'name', 'version', 'executable', 'analyzer', 'rule',
                'series_rule', 'target', 'authority', 'consumer'} and
                manifest.get('name') == pin['analyzer'], 'unknown measurer capability')
        for key, value in IDENTITY.items():
            require(manifest.get(key) == value, f'measurer manifest {key} mismatch')
        require(manifest.get('authority') == 'measurement-only' and
                manifest.get('series_rule') == pin['rule'] + '/1' and
                manifest.get('target') == 'x86_64-unknown-linux-gnu', 'unknown measurer capability')
        # A digest-specific install avoids stale files, shared-root collisions and
        # following symlinks planted in an earlier extraction.
        install = cache / (pin['sha256'] + '-installed')
        install.mkdir(exist_ok=True)
        require(not install.is_symlink(), 'unsafe measurer install directory')
        for name in (pin['executable'], 'plugin.json'):
            target = install / name
            require(not target.is_symlink(), 'unsafe measurer install file')
            target.write_bytes((binary.parent / name).read_bytes())
        (install / pin['executable']).chmod(0o755)
        return install / pin['executable']


def ensure_measurer(cache: Path, *, command=run, pin=None) -> Path:
    pin = load_measurer_pin() if pin is None else validate_pin(pin)
    # Namespace all downloads by archive digest: old and new assets share a name.
    cache = cache / pin['sha256']
    cache.mkdir(parents=True, exist_ok=True)
    archive = cache / pin['asset']
    if not archive.is_file():
        command('measurer-download', ['curl', '--fail', '--show-error', '--location',
            '--proto', '=https', '--proto-redir', '=https', '--tlsv1.2', '--retry', '3',
            '--output', str(archive), release_url(pin, pin['asset'])])
    verify_measurer_asset(archive, pin)
    # A cached archive is re-verified, never treated as signed by a marker file.
    verify_attestation(archive, pin, command)
    inventory = cache / 'release-inventory.json'
    command('measurer-inventory-download', ['curl', '--fail', '--show-error', '--location',
        '--proto', '=https', '--proto-redir', '=https', '--tlsv1.2', '--output', str(inventory),
        release_url(pin, inventory.name)])
    verify_attestation(inventory, pin, command)
    data = json.loads(inventory.read_text())
    require(data.get('schema_version') == 1 and data.get('tag') == pin['release'] and
            data.get('source') == {'repository': 'https://github.com/' + pin['repository'],
                                   'commit': pin['source_commit']}, 'measurer release provenance mismatch')
    matches = [asset for asset in data.get('assets', []) if asset.get('name') == pin['asset']]
    require(len(matches) == 1 and matches[0].get('kind') == 'package' and
            matches[0].get('sha256') == pin['sha256'] and matches[0].get('size_bytes') == archive.stat().st_size,
            'measurer release inventory mismatch')
    return extract_measurer(archive, cache, pin)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--pin', type=Path, default=MEASURER_PIN)
    args = parser.parse_args()
    print(ensure_measurer(args.cache, pin=load_measurer_pin(args.pin)))
