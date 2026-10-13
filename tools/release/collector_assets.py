"""Independent collector assets. Imported only from the host-trusted installer.

One RSA/SHA-256 signature covers the exact inventory bytes; the inventory binds
all other subjects. The public key and OpenSSL executable come from host trust,
never from the release being inspected. No code in an archive is executed.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import platform
import re
import stat
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'quality'))
import rust_collector_contract as contract

REPOSITORY = 'musutrade/Harness-Gate'
WORKFLOW = '.github/workflows/rust-collector-release.yml'
ASSETS = ('collector.tar', 'manifest.json', 'sbom.spdx.json', 'provenance.json')
CONTROL = ('release-inventory.json', 'release-inventory.sig')
VERSION = re.compile(r'(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)(?:-rc\.[1-9][0-9]*)?')
require = contract.require


def version(value):
    require(isinstance(value, str) and VERSION.fullmatch(value), 'expected exact version or rc.N')
    return value


def sha(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def unique(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate JSON key')
        result[key] = value
    return result


def read(path):
    return json.loads(Path(path).read_bytes(), object_pairs_hook=unique,
                      parse_constant=lambda _: require(False, 'nonfinite JSON'))


def write(path, value):
    Path(path).write_text(json.dumps(value, sort_keys=True, indent=2) + '\n')


def regular(path):
    mode = path.lstat()
    require(stat.S_ISREG(mode.st_mode) and mode.st_nlink == 1, 'unsafe file: ' + str(path))


def safe_name(name):
    require(isinstance(name, str) and name and not name.startswith('/')
            and '\\' not in name and ':' not in name and '\x00' not in name
            and all(p not in ('', '.', '..') for p in name.split('/')), 'unsafe payload path')
    return name


def probe_host(trust):
    """The host administrator supplies paths; release metadata cannot pick them."""
    require(platform.system() == 'Linux' and platform.machine() == 'x86_64', 'unsupported host ABI')
    libraries = {}
    for name, value in trust['host_libraries'].items():
        path = Path(value)
        require(isinstance(name, str) and name and path.is_absolute(), 'invalid host library inventory')
        regular(path)
        libraries[name] = sha(path)
    require(bool(libraries), 'host library trust inventory is empty')
    return {'target': 'x86_64-unknown-linux-gnu', 'glibc': os.confstr('CS_GNU_LIBC_VERSION'),
            'kernel': platform.release(), 'runtime_dependencies_sha256': contract.fingerprint(libraries)}


def verify_signature(directory, trust):
    if trust.get('schema') == 'rust-collector-host-trust/v2':
        from collector_sigstore import verify
        return verify(directory, trust)
    require(set(trust) == {'schema', 'openssl', 'openssl_sha256', 'public_key',
                          'public_key_sha256', 'rsa_signature_bytes', 'host_libraries'},
            'invalid host trust fields')
    require(trust['schema'] == 'rust-collector-host-trust/v1', 'unknown host trust')
    size = trust['rsa_signature_bytes']
    require(type(size) is int and 256 <= size <= 1024
            and (directory / CONTROL[1]).stat().st_size == size, 'invalid release signature length')
    for field in ('openssl', 'public_key'):
        path = Path(trust[field])
        require(path.is_absolute(), 'trust paths must be absolute')
        regular(path)
        require(sha(path) == trust[field + '_sha256'], 'host trust pin changed: ' + field)
        require(not path.is_relative_to(directory.resolve()), 'release cannot supply host trust')
    result = subprocess.run([trust['openssl'], 'dgst', '-sha256', '-verify', trust['public_key'],
                             '-signature', str(directory / CONTROL[1]), str(directory / CONTROL[0])],
                            capture_output=True, timeout=30)
    require(result.returncode == 0, 'missing/invalid release signature')


def sbom(manifest):
    return {'spdxVersion': 'SPDX-2.3', 'dataLicense': 'CC0-1.0', 'SPDXID': 'SPDXRef-DOCUMENT',
            'name': 'harness-gate-rust-collector-' + manifest['collector']['version'],
            'documentNamespace': 'https://github.com/' + REPOSITORY + '/rust-collector/'
                                 + contract.fingerprint(manifest),
            'creationInfo': {'creators': ['Tool: Harness-Gate collector release tooling'],
                             'created': '1970-01-01T00:00:00Z'},
            'files': [{'SPDXID': 'SPDXRef-File-' + str(i), 'fileName': p['path'],
                       'checksums': [{'algorithm': 'SHA256', 'checksumValue': p['sha256']}],
                       'licenseInfoInFiles': ['NOASSERTION'],
                       'licenseConcluded': 'NOASSERTION', 'copyrightText': 'NOASSERTION'}
                      for i, p in enumerate(manifest['payloads'])]}


def subjects(directory, names):
    return [{'name': name, 'digest': {'sha256': sha(directory / name)}} for name in names]


def prepare(directory, eligibility):
    """Generate unsigned subjects; signing is a separate protected operation."""
    manifest = contract.load_manifest((directory / 'manifest.json').read_bytes())
    v = version(manifest['collector']['version'])
    require(eligibility['tag'] == 'rust-collector-v' + v
            and eligibility['commit'] == manifest['source_commit'], 'wrong release eligibility identity')
    require(eligibility['status'] == 'pass', 'release is ineligible')
    write(directory / 'sbom.spdx.json', sbom(manifest))
    write(directory / 'provenance.json', {
        '_type': 'https://in-toto.io/Statement/v1',
        'subject': subjects(directory, ASSETS[:3]),
        'predicateType': 'https://harness-gate.dev/collector-release/v1',
        'predicate': {'repository': REPOSITORY, 'workflow': WORKFLOW,
                      'ref': 'refs/tags/' + eligibility['tag'],
                      'source_commit': manifest['source_commit'], 'eligibility': eligibility}})
    write(directory / CONTROL[0], {'schema': 'rust-collector-release/v1',
        'tag': eligibility['tag'], 'source_commit': manifest['source_commit'],
        'assets': subjects(directory, ASSETS)})


def verify(directory, trust, tag, *, archive_sha256=None):
    names = ASSETS + CONTROL if archive_sha256 is None else ASSETS[1:] + CONTROL
    allowed = set(names)
    if archive_sha256 is not None:
        allowed.add('archive-receipt.json')
        if (directory / 'collector.tar').exists():
            allowed.add('collector.tar')  # recoverable migration before archive removal
    require(set(p.name for p in directory.iterdir()) == allowed, 'missing/extra release assets')
    for name in allowed:
        regular(directory / name)
    verify_signature(directory, trust)
    inventory = read(directory / CONTROL[0])
    manifest = contract.load_manifest((directory / 'manifest.json').read_bytes())
    require(tag == 'rust-collector-v' + version(manifest['collector']['version']), 'wrong exact tag')
    def bound_subjects(names):
        return [{'name': name, 'digest': {'sha256': archive_sha256}}
                if name == 'collector.tar' and archive_sha256 is not None
                else {'name': name, 'digest': {'sha256': sha(directory / name)}} for name in names]
    require(inventory == {'schema': 'rust-collector-release/v1', 'tag': tag,
                           'source_commit': manifest['source_commit'],
                           'assets': bound_subjects(ASSETS)}, 'release inventory mismatch')
    require(read(directory / 'sbom.spdx.json') == sbom(manifest), 'SBOM payload inventory mismatch')
    provenance = read(directory / 'provenance.json')
    predicate = provenance['predicate']
    require(provenance == {'_type': 'https://in-toto.io/Statement/v1',
        'subject': bound_subjects(ASSETS[:3]),
        'predicateType': 'https://harness-gate.dev/collector-release/v1',
        'predicate': predicate}, 'provenance subjects mismatch')
    require(set(predicate) == {'repository', 'workflow', 'ref', 'source_commit', 'eligibility'}
            and predicate['repository'] == REPOSITORY and predicate['workflow'] == WORKFLOW
            and predicate['ref'] == 'refs/tags/' + tag
            and predicate['source_commit'] == manifest['source_commit'], 'untrusted workflow identity')
    from collector_release_policy import validate_receipt
    validate_receipt(predicate['eligibility'], tag, manifest['source_commit'])
    require(probe_host(trust) == manifest['host_abi'], 'unsupported host ABI')
    return manifest
