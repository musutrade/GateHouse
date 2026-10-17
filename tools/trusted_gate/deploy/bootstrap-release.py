"""Exact bootstrap source release contract; inspection never authorizes installation."""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import stat
import tarfile

OLD_HEAD = '659803a2f144926997e56d072c7fe8760d6ed176'
OLD_RELEASE = '04190f57c76f9625be1e805ab212c48802181f458fecdc34f5478f796efd77ef'
OLD_INPUTS = '56e6cb625a201477911a1fa764d143c644502c1c1af93d8c84c2235cdc762067'
BASE = '8994cf53a44677ecb36acc6f9a9ce1d547c57728'
CORE_TREE = 'a6918d6a7ff7fadc04170ff54aaac0db771763db'
MODULES = ('__init__', 'cache', 'cli', 'common', 'control', 'execution', 'github', 'history',
           'host', 'isolation', 'journal', 'publication', 'runtime', 'source')
OLD_MODULES = tuple(name for name in MODULES if name != 'runtime')
DEPLOY = 'tools/trusted_gate/deploy/'
MAX_BUNDLE = 128 * 1024**2
MAX_ARCHIVE = 160 * 1024**2
MAX_MEMBER = 1024**2
ORDER = ['authenticate-release-and-fixed659', 'verify-current-repository-and-base',
         'admit-separate-storage', 'preserve-and-pin659', 'stage-source-and-bind-payload',
         'import-official-and-locked-inputs', 'build-and-capture-core-under995',
         'check-captured-core-policy', 'reverify-retention-and-applicability',
         'confirm-revocation', 'retire-old-source', 'install-reviewed-files-and-config',
         'accept-transition-workload-and-recovery']
EXTERNAL = ['official_archives', 'locked_crate_archives', 'test_data_archives',
            'onboarding_source', 'host_build_recipe', 'volume_tool_recipe']


def source_map(modules=MODULES):
    values = {'tools/trusted_gate/' + name + '.py': ('/opt/gatehouse/trusted_gate/' + name + '.py', 0o644, 'replace')
              for name in modules}
    values.update({DEPLOY + name: ('/opt/gatehouse/bin/' + name, 0o755, 'replace')
                   for name in ('gatehouse-control', 'pilot-probe.py')})
    values.update({DEPLOY + 'pilot.flow.toml': ('/etc/gatehouse/policy/flow.toml', 0o644, 'replace'),
                   DEPLOY + 'gatehouse-step': ('/etc/gatehouse/policy/gatehouse-step', 0o755, 'replace'),
                   'tools/harness-gate/presets/empty.audit.toml': ('/etc/gatehouse/policy/audit.toml', 0o644, 'replace'),
                   'tools/harness-gate/presets/default.secrets.toml': ('/etc/gatehouse/policy/secrets.toml', 0o644, 'replace'),
                   DEPLOY + 'srv-gatehouse-pilot.mount': ('/etc/systemd/system/srv-gatehouse-pilot.mount', 0o644, 'retain'),
                   DEPLOY + 'sudoers.example': ('/etc/sudoers.d/gatehouse-pilot', 0o440, 'retain')})
    return values


def old_sources():
    return set(source_map(OLD_MODULES)) | {DEPLOY + 'install-pilot.py', DEPLOY + 'upgrade-history-pilot.py'}


def metadata(head):
    return {'schema': 'gatehouse-bootstrap-release/v1', 'repository': 'musutrade/GateHouse',
            'repository_id': 1414069625, 'head': head, 'base': BASE, 'old_head': OLD_HEAD,
            'old_release_sha256': OLD_RELEASE, 'core_tree': CORE_TREE,
            'core_provenance': 'protected-worker-build-required', 'core_binary_included': False,
            'core_destination': '/opt/gatehouse/bin/harness-gate', 'coverage_acceptance': 'transition',
            'policy_template': 'pilot-only-not-full-project', 'external_inputs_required': EXTERNAL,
            'required_migration_order': ORDER, 'current_remote_base_verified': False,
            'captured_core_policy_checked': False, 'administrator_ready': False,
            'production_gate_enabled': False}


def rows(payload):
    return {name: [len(raw), hashlib.sha256(raw).hexdigest()] for name, raw in sorted(payload.items())}


def plan(payload):
    return {name: {'destination': destination, 'mode': mode, 'action': action,
                   'before_sha256': hashlib.sha256(payload['before/' + name]).hexdigest()
                   if name in source_map(OLD_MODULES) else None,
                   'after_sha256': hashlib.sha256(payload['files/' + name]).hexdigest()}
            for name, (destination, mode, action) in source_map().items()}


def json_value(B, raw):
    value = json.loads(raw, object_pairs_hook=B.unique)
    B.need(raw == B.canonical(value), 'release metadata is not canonical')
    return value


def validate(B, payload, code, manifest_raw):
    head = code['revision']
    B.need(re.fullmatch('[0-9a-f]{40}', head), 'release requires exact reviewed revision')
    names = {'metadata.json', 'helper-manifest.json', 'source.bundle', 'payload-manifest.json'}
    names |= {'helpers/' + name for name in code['files']}
    names |= {'files/' + name for name in source_map()}
    names |= {'before/' + name for name in old_sources()}
    B.need(payload.keys() == names, 'release inventory differs')
    json_value(B, payload['metadata.json'])
    B.need(payload['metadata.json'] == B.canonical(metadata(head)), 'bootstrap scope or provenance differs')
    B.need(payload['helper-manifest.json'] == manifest_raw and json_value(B, manifest_raw) == code,
           'release helper manifest differs')
    for name, pin in code['files'].items():
        B.need(hashlib.sha256(payload['helpers/' + name]).hexdigest() == pin, 'packaged helper changed')
    before = {name: payload['before/' + name] for name in old_sources()}
    B.need(hashlib.sha256(B.canonical(rows(before))).hexdigest() == OLD_INPUTS,
           'accepted659 source inventory differs')
    B.need(all(action != 'retain' or payload['files/' + name] == payload['before/' + name]
               for name, (destination, mode, action) in source_map().items()), 'retained mount/delegation template changed')
    manifest = json_value(B, payload['payload-manifest.json'])
    expected = {'schema': 'gatehouse-bootstrap-payload/v1',
                'files': rows({name: raw for name, raw in payload.items() if name != 'payload-manifest.json'}),
                'installed_plan': plan(payload), 'source_binding_required': True}
    B.need(payload['payload-manifest.json'] == B.canonical(expected), 'release file digests or installed plan differ')
    B.need(0 < len(payload['source.bundle']) <= MAX_BUNDLE, 'source bundle exceeds budget')
    return {'schema': 'gatehouse-bootstrap-inspection/v1', 'head': head, 'members': len(payload),
            'source_binding_required': True, 'administrator_ready': False, 'production_gate_enabled': False,
            'scope': 'reviewed source inputs only; no installation, quality or delivery authorization'}


def decode(B, stream, size, expected, code, manifest_raw):
    """Hash first, then parse only bounded regular USTAR headers; never extract."""
    B.need(0 < size <= MAX_ARCHIVE and size % 512 == 0 and re.fullmatch('[0-9a-f]{64}', expected),
           'release size or pin differs')
    stream.seek(0)
    digest = hashlib.sha256()
    total = 0
    while chunk := stream.read(1024**2):
        total += len(chunk)
        B.need(total <= size, 'release changed while hashing')
        digest.update(chunk)
    B.need(total == size and digest.hexdigest() == expected, 'release digest differs')
    stream.seek(0)
    payload = {}
    while stream.tell() < size:
        raw = stream.read(512)
        B.need(len(raw) == 512, 'truncated release header')
        if raw == bytes(512):
            trailing = 512
            while chunk := stream.read(1024**2):
                B.need(not any(chunk), 'nonzero data after release end')
                trailing += len(chunk)
            B.need(trailing >= 1024, 'incomplete tar terminator')
            return payload, validate(B, payload, code, manifest_raw)
        # TarInfo.frombuf handles exactly one header, without PAX processing or
        # allocation from extension lengths; extensions/links are forbidden.
        B.need(raw[257:265] == b'ustar\x0000' and not any(raw[345:500]), 'only plain USTAR members supported')
        member = tarfile.TarInfo.frombuf(raw, 'utf-8', 'strict')
        B.need(member.type in (b'0', b'\0') and not member.linkname and member.uid == member.gid == member.mtime == 0
               and member.mode == 0o400 and not member.uname and not member.gname,
               'unsafe release member metadata')
        name = member.name
        allowed = {'metadata.json', 'helper-manifest.json', 'source.bundle', 'payload-manifest.json'}
        allowed |= {'helpers/' + value for value in code['files']}
        allowed |= {'files/' + value for value in source_map()} | {'before/' + value for value in old_sources()}
        B.need(name in allowed and name not in payload and len(payload) < len(allowed),
               'unknown or duplicate release member')
        limit = MAX_BUNDLE if name == 'source.bundle' else MAX_MEMBER
        B.need(0 <= member.size <= limit and stream.tell() + ((member.size + 511) // 512) * 512 <= size,
               'oversized or truncated release member')
        value = stream.read(member.size)
        B.need(len(value) == member.size, 'truncated release body')
        payload[name] = value
        padding = stream.read((-member.size) % 512)
        B.need(not any(padding), 'nonzero member padding')
    raise ValueError('release lacks tar terminator')


def inspect(B, helpers, manifest, manifest_pin, archive, archive_pin):
    B.need(os.geteuid() == 0, 'administrator authentication required for release inspection')
    code = B.authenticate(helpers, manifest, manifest_pin)
    manifest_raw = B.read(manifest, 64 * 1024)
    archive = B.owned(archive, private=True)
    with os.fdopen(os.open(archive, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), 'rb') as stream:
        before = os.fstat(stream.fileno())
        B.need(stat.S_ISREG(before.st_mode) and before.st_nlink == 1, 'release is not a private regular file')
        payload, result = decode(B, stream, before.st_size, archive_pin, code, manifest_raw)
        fields = lambda value: (value.st_dev, value.st_ino, value.st_uid, value.st_gid, value.st_mode,
                                value.st_nlink, value.st_size, value.st_mtime_ns, value.st_ctime_ns)
        B.need(fields(before) == fields(os.fstat(stream.fileno())) == fields(archive.lstat()), 'release changed during inspection')
    for name in code['files']:
        B.need(payload['helpers/' + name] == B.read(Path(helpers) / name, MAX_MEMBER), 'protected helper differs')
    B.authenticate(helpers, manifest, manifest_pin)
    return result


def bind_source(B, payload, source, head, manifest_pin):
    """Mandatory independent Git binding before a future upgrader may install."""
    B.need(hashlib.sha256(payload['helper-manifest.json']).hexdigest() == manifest_pin,
           'source binding requires the authenticated helper manifest pin')
    code = json_value(B, payload['helper-manifest.json'])
    validate(B, payload, code, payload['helper-manifest.json'])
    B.need(head == code['revision'], 'release source head differs')
    B.frozen_lock(source, head, BASE)
    for prefix, names in (('files/', source_map()), ('helpers/', code['files'])):
        for name in names:
            B.need(B.git(source, 'show', head + ':' + name) == payload[prefix + name],
                   'proposed installed/helper bytes differ from protected Git source')
    return {'head': head, 'source_bound': True, 'administrator_ready': False, 'production_gate_enabled': False}
