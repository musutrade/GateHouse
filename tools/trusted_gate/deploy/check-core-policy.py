"""Validate passive Core configuration/plan replies, never quality results.

The privileged caller must separately authenticate captured Core, source, policy,
configuration, runtime inputs and the inherited lease before running commands.
"""
from __future__ import annotations

import hashlib
import json
import math
import fcntl
import io
import os
from pathlib import Path
import resource
import stat
import subprocess
import time
import uuid

MAX_REPLY = 2 * 1024**2
IDS = ['builtin.secret-scan', 'builtin.architecture-audit', 'pilot-boundary', 'pilot-host-contracts', 'linux-test']
PROFILES = ['full', 'hook']
# Core hashes Rust's TOML serialization of the effective FlowConfig, including
# defaults (project/discovery.rs), rather than raw input bytes. These separate
# reviewed pins bind this fixed transition policy to the independently reviewed
# Core tree a6918d6a7ff7fadc04170ff54aaac0db771763db. The privileged caller must
# verify that captured tree; arbitrary Core output cannot select a new pin.
FLOW_SHA256 = 'c594c9a5faaecd69dc3b5a957b50cf4b63dc45f460296ac7e7674ef1b6a84320'
CONFIGURATION_DIGEST = 'sha256:fbcd84676bca83b305c92958c88732ab7e3aa10ef5e5d31a55ae9688ed419013'
VOLUME = Path('/srv/gatehouse/linux')
CONTROL = Path('/srv/gatehouse/pilot/control')
BASE = '8994cf53a44677ecb36acc6f9a9ce1d547c57728'


def decode(B, raw):
    B.need(isinstance(raw, bytes) and 0 < len(raw) <= MAX_REPLY, 'Core inspection response exceeds bound')
    value = json.loads(raw, object_pairs_hook=B.unique)
    # Reject NaN/Infinity and retain JSON boolean/integer distinctions in comparisons.
    B.canonical(value)
    return value


def same(B, actual, expected, message):
    B.need(B.canonical(actual) == B.canonical(expected), message)


def reason(B, rows, code):
    B.need(isinstance(rows, list) and len(rows) == 1 and isinstance(rows[0], dict)
           and set(rows[0]) == {'code', 'message'} and rows[0]['code'] == code
           and isinstance(rows[0]['message'], str) and 0 < len(rows[0]['message']) <= 2048,
           'Core plan explanation differs from required selection semantics')


def check_report(B, raw):
    value = decode(B, raw)
    same(B, value, {'schema_version': 1, 'valid': True, 'truncated': False, 'diagnostics': []},
         'captured Core configuration is invalid, truncated or has diagnostics')
    return {'schema_version': 1, 'valid': True, 'sha256': hashlib.sha256(raw).hexdigest()}


def check_plan(B, raw, profile, flow_raw):
    B.need(profile in PROFILES, 'unsupported Core inspection profile')
    value = decode(B, raw)
    B.need(isinstance(value, dict) and set(value) == {'schema', 'status', 'verification_result', 'plan', 'diagnostics'}
           and value['schema'] == 'verification-plan/v1' and value['status'] == 'planned'
           and value['verification_result'] is None and value['diagnostics'] == [],
           'Core inspection must return a passive successful plan')
    plan = value['plan']
    B.need(isinstance(plan, dict) and set(plan) == {'components', 'configuration_digest', 'execution_order',
        'nodes', 'notice', 'paths', 'profile', 'profiles', 'project', 'quality', 'rules', 'runtime_inputs', 'scope', 'services'},
        'Core plan fields differ from the reviewed API')
    B.need(hashlib.sha256(flow_raw).hexdigest() == FLOW_SHA256, 'Core inspection flow differs from reviewed bytes')
    expected_digest = CONFIGURATION_DIGEST
    B.need(plan['configuration_digest'] == expected_digest and plan['profile'] == profile
           and plan['project'] == 'gatehouse-linux-test-transition' and plan['runtime_inputs'] == 'not_validated',
           'Core plan configuration/profile or runtime scope differs')
    B.need(isinstance(plan['notice'], str) and 0 < len(plan['notice']) <= 2048, 'Core plan notice is missing')
    same(B, plan['execution_order'], IDS, 'Core required execution order differs')
    same(B, plan['services'], [], 'Core inspection cannot select services')
    same(B, plan['paths'], [], 'Core inspection must use all mode without changed path scope')
    same(B, plan['scope'], {'changed_files': [], 'components': ['linux-test', 'pilot'],
        'mode': 'all', 'unmatched_files': []}, 'Core plan scope drops or changes coverage')
    same(B, plan['quality'], {'enabled': False, 'reasons': [{'code': 'QUALITY_NOT_CONFIGURED',
        'message': 'No quality.toml is configured.'}]}, 'transition plan cannot imply complete quality migration')
    nodes = plan['nodes']
    B.need(isinstance(nodes, list) and len(nodes) == len(IDS) and [node['id'] for node in nodes] == IDS,
           'Core plan node inventory differs')
    for index, node in enumerate(nodes):
        B.need(set(node) == {'component', 'depends_on', 'id', 'input', 'kind', 'label', 'profiles', 'reasons',
                           'required_by', 'selected', 'services'}, 'Core plan node fields differ')
        B.need(node['selected'] is True and isinstance(node['reasons'], list) and node['reasons'],
               'Core plan has an unselected or unexplained required node')
        builtin = index < 2
        reason(B, node['reasons'], 'MANDATORY_BUILTIN_GATE' if builtin else 'COMPONENT_AND_PROFILE')
        same(B, {key: node[key] for key in ('kind', 'input', 'profiles', 'services', 'component')},
             {'kind': 'builtin-gate' if builtin else 'external-step', 'input': None if builtin else 'snapshot',
              'profiles': None if builtin else PROFILES, 'services': None if builtin else [],
              'component': None if builtin else ('linux-test' if index == 4 else 'pilot')},
             'Core plan node kind, profile, component or execution inputs differ')
        same(B, node['depends_on'], [] if index == 0 else [IDS[0] if index == 1 else IDS[1]],
             'Core plan no longer preserves mandatory builtin dependencies')
        same(B, node['required_by'], [IDS[1]] if index == 0 else
             (['linux-test', 'pilot-boundary', 'pilot-host-contracts'] if index == 1 else []),
             'Core plan requirement chain differs')
    profiles = plan['profiles']
    B.need(isinstance(profiles, list) and [item['id'] for item in profiles] == PROFILES,
           'Core plan profile inventory differs')
    for item in profiles:
        B.need(set(item) == {'id', 'reasons', 'selected'} and item['selected'] is (item['id'] == profile),
               'Core plan selected profile differs')
        reason(B, item['reasons'], 'PROFILE_REQUESTED' if item['selected'] else 'PROFILE_NOT_SELECTED')
    components = plan['components']
    B.need(isinstance(components, list) and [item['id'] for item in components] == ['linux-test', 'pilot']
           and all(set(item) == {'id', 'selected', 'depends_on_components', 'impact_sources', 'impact_chain', 'rules', 'reasons'}
                   for item in components), 'Core plan component selection differs')
    for item in components:
        same(B, {key: item[key] for key in item if key != 'reasons'}, {'id': item['id'], 'selected': True,
             'depends_on_components': [], 'impact_sources': [], 'impact_chain': [], 'rules': [0]},
             'Core plan component coverage differs')
        reason(B, item['reasons'], 'ALL_COMPONENTS')
    rules = plan['rules']
    B.need(isinstance(rules, list) and len(rules) == 1
           and set(rules[0]) == {'index', 'patterns', 'components', 'paths', 'selected', 'reasons'},
           'Core all-scope rule fields differ')
    same(B, {key: rules[0][key] for key in rules[0] if key != 'reasons'}, {'index': 0, 'patterns': ['**'],
         'components': ['linux-test', 'pilot'], 'paths': [], 'selected': False}, 'Core all-scope rule differs')
    reason(B, rules[0]['reasons'], 'RULE_NOT_MATCHED')
    return {'profile': profile, 'configuration_digest': expected_digest, 'selected_nodes': list(IDS),
            'runtime_inputs': 'not_validated', 'verification_result': None, 'sha256': hashlib.sha256(raw).hexdigest()}


def commands(B, core, root, flow):
    core, root, flow = map(B.path_operand, (core, root, flow))
    B.need(flow == root / '.gatehouse-host/flow.toml' and core != root and root not in core.parents,
           'Core inspection paths differ from the private policy view')
    prefix = [str(core), '--project-root', str(root), '--config', str(flow)]
    return [('configuration', [*prefix, 'config', 'check', '--format', 'json']),
            *[(profile, [*prefix, 'verify', '--all', '--profile', profile, '--plan', '--json']) for profile in PROFILES]]


def view_inventory(B, stage, destination, head):
    B.frozen_lock(destination / 'source', head, BASE)
    rows = {}
    for name in ('source', 'mirror.git'):
        files, directories, total = stage.inventory(destination / name)
        rows[name] = {'files': files, 'directories': directories, 'bytes': total}
    bundle = B.read(destination / 'candidate.bundle', 128 * 1024**2)
    rows['bundle'] = [len(bundle), hashlib.sha256(bundle).hexdigest()]
    B.need(sum(row['bytes'] for key, row in rows.items() if key != 'bundle') + len(bundle) <= 512 * 1024**2,
           'Core inspection view exceeds budget')
    return rows


def copy_view(B, helpers, payload, head, destination):
    """Independent Git store and working tree; never overlay the active source."""
    stage = B.load(helpers, 'stage-linux-inputs.py')
    git_api = B.load(helpers, 'stage-protected-source.py')
    def git(where, *args):return git_api.git(B, where, *args, spool=destination)
    bundle = destination / 'candidate.bundle';B.write(bundle, payload['source.bundle'], 0o400)
    mirror, source = destination / 'mirror.git', destination / 'source'
    git(destination, 'clone', '--mirror', '--no-hardlinks', str(VOLUME / 'bootstrap-source/mirror.git'), str(mirror))
    B.need(git(mirror, 'bundle', 'list-heads', str(bundle)) == (head + ' HEAD\n').encode(),
           'inspection bundle advertises another candidate')
    git(mirror, 'bundle', 'verify', str(bundle))
    git(mirror, '-c', 'protocol.file.allow=always', '-c', 'fetch.fsckObjects=true',
        'fetch', '--no-tags', '--no-recurse-submodules', str(bundle), head)
    git(mirror, 'worktree', 'add', '--detach', str(source), head)
    B.need(not git(source, 'status', '--porcelain', '--untracked-files=all', '--ignored'),
           'inspection source is not pristine')
    overlay = source / '.gatehouse-host';overlay.mkdir(mode=0o700)
    contract = B.load(helpers, 'bootstrap-release.py')
    names = set()
    for name, (target, mode, action) in contract.source_map().items():
        if target.startswith('/etc/gatehouse/policy/'):
            basename = Path(target).name;names.add(basename)
            B.write(overlay / basename, payload['files/' + name], mode)
    same(B, sorted(names), ['audit.toml', 'flow.toml', 'gatehouse-step', 'linux-transition.json', 'secrets.toml'],
         'inspection policy inventory differs')
    B.need(hashlib.sha256(B.read(overlay / 'flow.toml', 1024**2)).hexdigest() == FLOW_SHA256,
           'inspection policy bytes differ')
    B.readonly(source)
    return view_inventory(B, stage, destination, head)


def limits():
    resource.setrlimit(resource.RLIMIT_AS, (1024**3, 1024**3))
    resource.setrlimit(resource.RLIMIT_CPU, (45, 45))
    resource.setrlimit(resource.RLIMIT_FSIZE, (MAX_REPLY, MAX_REPLY))
    resource.setrlimit(resource.RLIMIT_NOFILE, (64, 64))


def unit_command(B, destination, label, argv, env_program, unit):
    """One deterministic command contract for execution and retained-record replay."""
    allowed = dict(commands(B, '/work/inspection-core', '/work/source', '/work/source/.gatehouse-host/flow.toml'))
    B.need(label in allowed and argv == allowed[label], 'only fixed passive Core commands may run')
    identity = unit.removeprefix('gatehouse-core-inspection-').removesuffix('.service')
    parsed = uuid.UUID(identity)
    B.need(parsed.version == 4 and str(parsed) == identity
           and unit == 'gatehouse-core-inspection-' + identity + '.service', 'Core inspection unit identity differs')
    properties = {'User': 'root', 'Group': 'root', 'SupplementaryGroups': '', 'UMask': '0077', 'NoNewPrivileges': 'yes',
        'CapabilityBoundingSet': '', 'AmbientCapabilities': '', 'ProtectSystem': 'strict',
        'ProtectHome': 'yes', 'PrivateNetwork': 'yes', 'PrivateDevices': 'yes', 'PrivateTmp': 'yes',
        'ProtectKernelTunables': 'yes', 'ProtectKernelModules': 'yes', 'ProtectControlGroups': 'yes',
        'ProtectKernelLogs': 'yes', 'ProtectClock': 'yes', 'RestrictNamespaces': 'yes',
        'RestrictSUIDSGID': 'yes', 'LockPersonality': 'yes', 'KillMode': 'control-group',
        'RuntimeMaxSec': '30', 'TimeoutStopSec': '15', 'MemoryMax': str(1024**3),
        'TasksMax': '32', 'LimitFSIZE': str(MAX_REPLY), 'WorkingDirectory': '/work/source',
        'TemporaryFileSystem': '/work:mode=0755,nosuid,nodev',
        'BindReadOnlyPaths': str(destination / 'source') + ':/work/source ' +
            str(VOLUME / 'bootstrap-core/harness-gate') + ':/work/inspection-core',
        'InaccessiblePaths': '/home /root /run /etc/gatehouse /srv/gatehouse/pilot'}
    outer = ['/usr/bin/systemd-run', '--quiet', '--wait', '--pipe', '--collect', '--unit=' + unit]
    outer += ['--property=' + key + '=' + value for key, value in properties.items()]
    outer += ['--', str(B.path_operand(env_program)), '-i', *[key + '=' + val for key, val in B.ENV.items()], *argv]
    return outer


def run_command(B, builder, destination, label, argv, env_program):
    """A transient Root parser unit, with no project execution or credential access."""
    unit = 'gatehouse-core-inspection-' + str(uuid.uuid4()) + '.service'
    outer = unit_command(B, destination, label, argv, env_program, unit)
    stdout, stderr = destination / (label + '.stdout'), destination / (label + '.stderr')
    invocation = {'unit': unit, 'argv': outer, 'state': 'starting', 'worker_stop_proven': False, 'exit_code': None}
    B.record(destination / (label + '.unit.json'), invocation)
    try:
        with stdout.open('xb') as output, stderr.open('xb') as errors:
            result = subprocess.run(outer, env=B.ENV, stdin=subprocess.DEVNULL, stdout=output, stderr=errors,
                                    timeout=50, preexec_fn=limits)
        B.need(result.returncode == 0, 'captured Core inspection command failed')
    finally:
        subprocess.run(['/usr/bin/systemctl', 'stop', unit], env=B.ENV, capture_output=True, timeout=30, check=False)
        builder.stopped(B, unit)
        B.record(destination / (label + '.unit.json'), {**invocation, 'state': 'stopped', 'worker_stop_proven': True,
            'exit_code': result.returncode if 'result' in locals() else None})
    raw, errors = B.read(stdout, MAX_REPLY), B.read(stderr, MAX_REPLY)
    B.need(not errors, 'captured Core inspection produced unexpected stderr')
    return raw


def inspection_status(head, manifest_pin, configuration_pin, core_pin):
    return {'schema': 'gatehouse-core-policy-inspection/v1', 'state': 'incomplete', 'head': head, 'base': BASE,
        'helper_manifest_sha256': manifest_pin, 'configuration_binding_sha256': configuration_pin,
        'core_build_record_sha256': core_pin, 'captured_core_policy_checked': False,
        'administrator_ready': False, 'production_gate_enabled': False,
        'coverage_acceptance': 'transition', 'authoritative_publish': 'blocked'}


def verified_view(B, stage, contract, payload, destination, head, manifest_pin):
    """Authenticate the independent Git/policy view, not just its saved manifest."""
    entries, total = 0, 0
    for name in ('source', 'mirror.git'):
        root = B.owned(destination / name, directory=True)
        for parent, directories, files in os.walk(root):
            for child in (None, *directories, *files):
                path = Path(parent) if child is None else Path(parent) / child
                directory = child is None or child in directories
                info = B.owned(path, directory=directory).stat()
                entries += 1
                total += 0 if directory else info.st_size
                B.need(entries <= 40000 and total <= 512 * 1024**2, 'inspection view exceeds replay budget')
                B.need(not info.st_mode & 0o7000, 'privileged inspection view mode')
                if name == 'source':
                    B.need(stat.S_IMODE(info.st_mode) == (0o555 if directory or info.st_mode & 0o111 else 0o444),
                           'inspection source is no longer readonly')
    contract.bind_source(B, payload, destination / 'source', head, manifest_pin)
    overlay = destination / 'source/.gatehouse-host'
    expected = {Path(target).name: (payload['files/' + source], 0o555 if mode & 0o111 else 0o444)
                for source, (target, mode, action) in contract.source_map().items()
                if target.startswith('/etc/gatehouse/policy/')}
    B.need({path.name for path in B.owned(overlay, directory=True).iterdir()} == expected.keys(),
           'inspection policy inventory changed')
    for name, (raw, mode) in expected.items():
        path = B.owned(overlay / name)
        B.need(stat.S_IMODE(path.stat().st_mode) == mode and B.read(path, 1024**2) == raw,
               'inspection policy differs from authenticated release')
    B.need(hashlib.sha256(expected['flow.toml'][0]).hexdigest() == FLOW_SHA256,
           'inspection flow differs from reviewed policy')
    # The five policy files are the only deliberate additions to the pristine
    # copied candidate. Include ignored files and force mode observation as well.
    status = B.git(destination / 'source', '-c', 'core.filemode=true', 'status', '--porcelain=v1', '-z',
                   '--untracked-files=all', '--ignored')
    B.need(sorted(filter(None, status.split(b'\0'))) == sorted(
        ('?? .gatehouse-host/' + name).encode() for name in expected), 'inspection working tree changed')
    B.need(B.read(destination / 'candidate.bundle', 128 * 1024**2) == payload['source.bundle'],
           'inspection bundle differs from authenticated release')
    return view_inventory(B, stage, destination, head)


def completed_inspection(B, builder, stage, contract, payload, destination, head, manifest_pin,
                         configuration_pin, core_pin, prepared, inspection_pin, env_program):
    """Revalidate retained passive replies and stopped invocations; execute nothing."""
    B.owned(destination, directory=True, private=True)
    labels = ['configuration', *PROFILES]
    names = {'source', 'mirror.git', 'candidate.bundle', 'view.json', 'inspection.json', 'binding.json'}
    names |= {label + suffix for label in labels for suffix in ('.stdout', '.stderr', '.unit.json')}
    B.need({path.name for path in destination.iterdir()} == names, 'completed inspection inventory differs')
    for name in names - {'source', 'mirror.git'}:
        path = B.owned(destination / name, private=True)
        B.need(stat.S_IMODE(path.stat().st_mode) == (0o400 if name in (
            'view.json', 'binding.json', 'candidate.bundle') else 0o600), 'completed inspection mode differs')
    saved = builder.json_pin(B, destination / 'binding.json', inspection_pin)
    seconds = saved['seconds']
    B.need(type(seconds) in (int, float) and math.isfinite(seconds) and seconds >= 0,
           'inspection observation time differs')
    view_raw = B.read(destination / 'view.json', 16 * 1024**2)
    view = json.loads(view_raw, object_pairs_hook=B.unique)
    B.need(view_raw == B.canonical(view), 'inspection view metadata is not canonical')
    same(B, verified_view(B, stage, contract, payload, destination, head, manifest_pin), view,
         'completed inspection view changed')
    replies, units = {}, set()
    for label, argv in commands(B, '/work/inspection-core', '/work/source', '/work/source/.gatehouse-host/flow.toml'):
        response = B.read(destination / (label + '.stdout'), MAX_REPLY)
        B.need(not B.read(destination / (label + '.stderr'), MAX_REPLY), 'retained inspection stderr is not empty')
        reply = check_report(B, response) if label == 'configuration' else check_plan(
            B, response, label, payload['files/tools/trusted_gate/deploy/linux-transition.flow.toml'])
        raw = B.read(destination / (label + '.unit.json'), 64 * 1024)
        invocation = json.loads(raw, object_pairs_hook=B.unique)
        B.need(raw == B.canonical(invocation), 'inspection invocation is not canonical')
        unit = invocation['unit']
        B.need(unit not in units, 'inspection invocations reused a unit')
        same(B, invocation, {'unit': unit, 'argv': unit_command(B, destination, label, argv, env_program, unit),
            'state': 'stopped', 'worker_stop_proven': True, 'exit_code': 0}, 'inspection invocation contract differs')
        builder.stopped(B, unit)
        units.add(unit);reply['invocation_sha256'] = hashlib.sha256(raw).hexdigest();replies[label] = reply
    status = inspection_status(head, manifest_pin, configuration_pin, core_pin)
    result = {**status, 'state': 'checked', 'captured_core_policy_checked': True,
        'configuration_sha256': prepared['configuration_sha256'], 'plan_sha256': prepared['plan_sha256'],
        'view_sha256': hashlib.sha256(view_raw).hexdigest(), 'replies': replies, 'seconds': seconds,
        'scope': 'captured-Core configuration and passive plans only; no runtime execution or quality acceptance'}
    same(B, saved, result, 'completed inspection binding differs')
    raw = B.read(destination / 'inspection.json', 1024**2)
    expected = {**status, 'state': 'checked', 'captured_core_policy_checked': True, 'binding_sha256': inspection_pin}
    B.need(raw == B.canonical(expected), 'inspection has no matching completed control record')
    return result


def verify_completed(B, helpers, manifest, manifest_pin, archive, archive_pin, input_pin, seal_pin, core_pin,
                     configuration_pin, inspection_pin, host, host_pin, recipe, recipe_pin,
                     preservation, preservation_pin, lock_fd):
    """Internal immutable inspection replay; no CLI or current-phase admission."""
    B.need(os.geteuid() == 0, 'administrator authentication required for inspection replay')
    B.need(os.uname().sysname == 'Linux' and os.uname().machine == 'x86_64', 'unsupported inspection replay platform')
    B.need(type(lock_fd) is int and lock_fd >= 0, 'inspection replay requires an inherited controller lease')
    code = B.authenticate(helpers, manifest, manifest_pin);head = code['revision']
    helpers, manifest, archive, host, recipe, preservation = map(B.path_operand,
        (helpers, manifest, archive, host, recipe, preservation))
    composer = B.load(helpers, 'compose-linux-host.py');builder = B.load(helpers, 'build-protected-core.py')
    stage = B.load(helpers, 'stage-linux-inputs.py');contract = B.load(helpers, 'bootstrap-release.py')
    fence = B.load(helpers, 'install-transaction.py')
    destination = VOLUME / 'bootstrap-core-inspection'
    fence.lease(B, lock_fd)
    def review():
        return composer.verify_prepared(B, helpers, manifest, manifest_pin, archive, archive_pin, input_pin, seal_pin,
            core_pin, configuration_pin, host, host_pin, recipe, recipe_pin, preservation, preservation_pin, lock_fd)
    initial = review()
    archive_raw = B.read(archive, contract.MAX_ARCHIVE)
    payload, _ = contract.decode(B, io.BytesIO(archive_raw), len(archive_raw), archive_pin, code,
                                 B.read(manifest, 64 * 1024))
    host_value = builder.host_inputs(B, host, host_pin)
    def retained():
        return completed_inspection(B, builder, stage, contract, payload, destination, head, manifest_pin,
                                    configuration_pin, core_pin, initial, inspection_pin, host_value['env'])
    result = retained()
    same(B, review(), initial, 'prepared configuration changed during inspection replay')
    same(B, retained(), result, 'retained inspection changed during replay')
    B.need(B.read(archive, contract.MAX_ARCHIVE) == archive_raw, 'inspection source release changed')
    B.authenticate(helpers, manifest, manifest_pin);fence.lease(B, lock_fd)
    return {'schema': 'gatehouse-core-policy-replay/v1', 'state': 'verified', 'head': head,
        'helper_manifest_sha256': manifest_pin, 'preservation_sha256': preservation_pin,
        'configuration_binding_sha256': configuration_pin, 'inspection_binding_sha256': inspection_pin,
        'configuration_sha256': result['configuration_sha256'], 'plan_sha256': result['plan_sha256'],
        'captured_core_policy_checked': True, 'administrator_ready': False, 'production_gate_enabled': False,
        'installation_admitted': False,
        'scope': 'immutable passive inspection only; current phase, base, journal, entry and drain admission required'}


def inspect(B, helpers, manifest, manifest_pin, archive, archive_pin, input_pin, seal_pin, core_pin,
            configuration_pin, host, host_pin, recipe, recipe_pin, lock_fd=None, *, api_proxy=None):
    B.need(os.geteuid() == 0, 'administrator authentication required for Core inspection')
    os.umask(0o077)
    code = B.authenticate(helpers, manifest, manifest_pin);head = code['revision']
    helpers, manifest, archive, host, recipe = map(B.path_operand, (helpers, manifest, archive, host, recipe))
    composer = B.load(helpers, 'compose-linux-host.py');builder = B.load(helpers, 'build-protected-core.py')
    stage = B.load(helpers, 'stage-linux-inputs.py');contract = B.load(helpers, 'bootstrap-release.py')
    destination = VOLUME / 'bootstrap-core-inspection'
    B.need(all(destination != path and destination not in path.parents and path not in destination.parents
               for path in (helpers, manifest, archive, host, recipe)), 'inspection output overlaps an input')
    lease = B.owned(CONTROL / 'controller.lock', private=True);own_lease = lock_fd is None
    if own_lease:lock_fd = os.open(lease, os.O_RDWR | os.O_NOFOLLOW)
    try:
        actual, expected = os.fstat(lock_fd), lease.stat()
        B.need(stat.S_ISREG(actual.st_mode) and (actual.st_dev, actual.st_ino) == (expected.st_dev, expected.st_ino),
               'Core inspection lease differs')
        fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        def review():
            options = {} if api_proxy is None else {'api_proxy': api_proxy}
            return composer.compose(B, helpers, manifest, manifest_pin, archive, archive_pin, input_pin, seal_pin,
                core_pin, host, host_pin, recipe, recipe_pin, lock_fd=lock_fd, existing_pin=configuration_pin, **options)
        initial = review()
        B.need(not destination.exists() and not destination.is_symlink(), 'retain prior Core inspection; never reuse')
        raw = B.read(archive, contract.MAX_ARCHIVE)
        payload, _ = contract.decode(B, io.BytesIO(raw), len(raw), archive_pin, code, B.read(manifest, 64 * 1024))
        contract.bind_source(B, payload, VOLUME / 'bootstrap-source/source', head, manifest_pin)
        host_value = builder.host_inputs(B, host, host_pin)
        destination.mkdir(mode=0o700)
        status = inspection_status(head, manifest_pin, configuration_pin, core_pin)
        B.record(destination / 'inspection.json', status)
        try:
            view = copy_view(B, helpers, payload, head, destination)
            view_raw = B.canonical(view);B.need(len(view_raw) <= 16 * 1024**2, 'inspection metadata exceeds bound')
            B.write(destination / 'view.json', view_raw, 0o400)
            replies = {};started = time.monotonic()
            for label, argv in commands(B, '/work/inspection-core', '/work/source', '/work/source/.gatehouse-host/flow.toml'):
                response = run_command(B, builder, destination, label, argv, host_value['env'])
                replies[label] = check_report(B, response) if label == 'configuration' else check_plan(
                    B, response, label, payload['files/tools/trusted_gate/deploy/linux-transition.flow.toml'])
                same(B, view_inventory(B, stage, destination, head), view, 'Core inspection view changed')
            same(B, review(), initial, 'configuration or protected inputs changed during Core inspection')
            for label, reply in replies.items():
                B.need(hashlib.sha256(B.read(destination / (label + '.stdout'), MAX_REPLY)).hexdigest() == reply['sha256']
                       and not B.read(destination / (label + '.stderr'), MAX_REPLY), 'inspection response changed')
                invocation_raw = B.read(B.owned(destination / (label + '.unit.json'), private=True), 64 * 1024)
                invocation = json.loads(invocation_raw, object_pairs_hook=B.unique)
                same(B, {key: invocation[key] for key in ('state', 'worker_stop_proven', 'exit_code')},
                     {'state': 'stopped', 'worker_stop_proven': True, 'exit_code': 0}, 'inspection worker stop is not proven')
                reply['invocation_sha256'] = hashlib.sha256(invocation_raw).hexdigest()
            B.need(B.read(destination / 'view.json', 16 * 1024**2) == view_raw, 'inspection metadata changed')
            B.authenticate(helpers, manifest, manifest_pin)
            result = {**status, 'state': 'checked', 'captured_core_policy_checked': True,
                'configuration_sha256': initial['configuration_sha256'], 'plan_sha256': initial['plan_sha256'],
                'view_sha256': hashlib.sha256(view_raw).hexdigest(), 'replies': replies,
                'seconds': time.monotonic() - started,
                'scope': 'captured-Core configuration and passive plans only; no runtime execution or quality acceptance'}
            B.write(destination / 'binding.json', B.canonical(result), 0o400)
            B.record(destination / 'inspection.json', {**status, 'state': 'checked', 'captured_core_policy_checked': True,
                'binding_sha256': hashlib.sha256(B.canonical(result)).hexdigest()})
            return result
        except BaseException:
            B.record(destination / 'inspection.json', {**status, 'state': 'blocked'})
            raise
    finally:
        if own_lease:os.close(lock_fd)
