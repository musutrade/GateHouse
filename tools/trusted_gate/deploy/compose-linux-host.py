"""Compose pinned transition configuration; never install or grant quality authority."""
from __future__ import annotations

import fcntl
import hashlib
import io
import json
import os
from pathlib import Path
import re
import stat
import sys
from types import ModuleType
import uuid

VOLUME = Path('/srv/gatehouse/linux')
CONTROL = Path('/srv/gatehouse/pilot/control')
INBOX = Path('/srv/gatehouse/pilot/inbox')
CONFIG = Path('/etc/gatehouse/host.json')
BASE = '8994cf53a44677ecb36acc6f9a9ce1d547c57728'
CARGO = (b'[source.crates-io]\nreplace-with = "gatehouse-locked"\n'
         b'[source.gatehouse-locked]\ndirectory = "/work/dependencies/sources"\n[net]\noffline = true\n')


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def value(B, path, limit=1024**2):
    raw = B.read(B.owned(path, private=True), limit)
    parsed = json.loads(raw, object_pairs_hook=B.unique)
    B.need(raw == B.canonical(parsed), 'configuration input metadata is not canonical')
    return parsed


def sealed_inputs(B, builder, root, pin, head, manifest_pin, input_pin, inputs):
    binding = builder.json_pin(B, B.owned(root / 'role-binding.json', private=True), pin)
    state = value(B, root / 'seal.json')
    common = {'schema': 'gatehouse-linux-input-seal/v1', 'state': 'sealed', 'head': head, 'base': BASE,
              'helper_manifest_sha256': manifest_pin, 'input_binding_sha256': input_pin,
              'administrator_ready': False, 'production_gate_enabled': False}
    B.need(B.canonical(state) == B.canonical({**common, 'binding_sha256': pin})
           and all(B.canonical(binding.get(key)) == B.canonical(expected) for key, expected in common.items())
           and binding['helper_revision'] == head, 'Linux sealing has no matching complete control record')
    roles = {'inventory': str(root / 'linux-inputs.json'), 'directory_sources': str(inputs / 'dependencies/sources'),
             'test_data_archives': str(root / 'test-data-archives'), 'onboarding': str(root / 'onboarding')}
    pins = {str(path): digest(B.read(path, 64 * 1024**2)) for path in (root / 'linux-inputs.json', root / 'cargo-config.toml')}
    B.need(binding['linux_inputs'] == roles and binding['cargo_config'] == str(root / 'cargo-config.toml')
           and binding['protected_files'] == pins and B.read(root / 'cargo-config.toml', 1024**2) == CARGO,
           'sealed Linux paths or metadata differ')
    return binding


def captured_core(B, builder, helpers, source, inputs, root, pin, head, manifest_pin, input_pin, host, host_pin):
    record = builder.json_pin(B, B.owned(root / 'build.json', private=True), pin)
    expected = {'schema': 'gatehouse-core-bootstrap/v1', 'state': 'captured', 'head': head, 'base': BASE,
                'helper_revision': head, 'helper_manifest_sha256': manifest_pin, 'input_binding_sha256': input_pin,
                'host_recipe_sha256': host_pin, 'profile': 'release', 'release_incremental': False,
                'worker_uid': 995, 'worker_gid': 981, 'target': 'x86_64-unknown-linux-gnu',
                'worker_stop_proven': True, 'exit_code': 0, 'production_gate_enabled': False}
    B.need(all(B.canonical(record.get(key)) == B.canonical(item) for key, item in expected.items()),
           'Core record is not the completed protected release build')
    B.need(re.fullmatch(r'gatehouse-bootstrap-[0-9a-f-]{36}\.service', record['unit'])
           and record['source']['tree'] == builder.CORE_TREE
           == B.git(source, 'rev-parse', head + ':tools/harness-gate').decode().strip(), 'Core source or unit differs')
    identity = record['unit'].removeprefix('gatehouse-bootstrap-').removesuffix('.service')
    parsed = uuid.UUID(identity)
    B.need(parsed.version == 4 and str(parsed) == identity, 'Core build unit identity differs')
    host_value = builder.host_inputs(B, host, host_pin)
    argv, environment = builder.command(B, root / 'source', inputs, root / 'worker', root / 'cargo-config.toml',
                                        record['unit'], host_value)
    B.need(record['argv'] == argv and B.canonical(record['environment']) == B.canonical(environment)
           and B.read(root / 'cargo-config.toml', 1024**2) == CARGO, 'Core build recipe differs')
    builder.stopped(B, record['unit'])
    builder.verify_snapshot(B, helpers, root / 'source', record['source'])
    binary = B.owned(root / 'harness-gate')
    raw = B.read(binary, builder.MAX_BINARY)
    B.need(stat.S_IMODE(binary.stat().st_mode) == 0o555 and 0 < len(raw) <= builder.MAX_BINARY
           and B.canonical(record['binary']) == B.canonical({'bytes': len(raw), 'sha256': digest(raw)}),
           'captured Core bytes or mode differ')
    return record


def configuration(B, policy, contract, payload, old, head, archive_pin, manifest_pin, input_pin, seal_pin, core_pin,
                  imported, sealed, captured, os_pins):
    """Pure composition over independently verified facts; callers must authenticate them."""
    source = VOLUME / 'bootstrap-source/source'
    inputs = VOLUME / 'bootstrap-inputs'
    tool_inventory = str(inputs / 'tools/inventory.json')
    pins = {destination: digest(payload['files/' + name])
            for name, (destination, mode, action) in contract.source_map().items() if action != 'retain'}
    pins.update(os_pins)
    pins.update({'/opt/gatehouse/bin/harness-gate': captured['binary']['sha256'],
                 tool_inventory: imported['tool_inventory_sha256'], **sealed['protected_files']})
    B.need(len(pins) == len(contract.source_map()) - 2 + 6
           and set(os_pins) == {'/usr/bin/python3.14', '/usr/bin/git'}
           and all(isinstance(pin, str) and re.fullmatch('[0-9a-f]{64}', pin) for pin in pins.values()),
           'configuration pin inventory differs')
    dimensions = dict(repository='musutrade/GateHouse', platform='linux', architecture='x86_64',
        toolchain='resolved-from-installed-files', target='native-python', profile='pilot-contracts', features='none',
        flags='python-no-bytecode', dependency_digest='resolved', build_config_digest='resolved',
        instrumentation='none', release_incremental='not-applicable', executor_identity='resolved')
    tools = ['/usr/bin/python3.14', '/usr/bin/git', tool_inventory]
    steps = {
        'pilot-boundary': {'argv': ['/usr/bin/python3.14', '-I', '/opt/gatehouse/bin/pilot-probe.py',
            'executor-history', '@gatehouse:head@', '@gatehouse:base@'], 'git_history': True, 'run_arguments': True},
        'pilot-host-contracts': {'argv': ['/usr/bin/python3.14', '-B', '-m', 'unittest', 'discover', '-s',
            '/work/source/tools/quality/tests', '-p', 'test_trusted_gate.py', '-q']}}
    for step in steps.values():
        step.update(timeout_seconds=120, cache_identity=dict(dimensions), toolchain_files=list(tools),
                    cache_inputs={'dependencies': [], 'configuration': []})
    steps['linux-test'] = policy.worker()
    steps['linux-test'].update(toolchain_files=list(tools), cache_identity={**dimensions,
        'target': 'x86_64-unknown-linux-gnu', 'profile': 'linux-test-debug', 'features': 'workflow-default',
        'flags': 'reviewed-linux-test-workflow'})
    # Explicit inventory: no optional old overlays, environment or evidence URL survive.
    config = {'schema': 'gatehouse-trusted-host/v1', 'repository': 'musutrade/GateHouse', 'repository_id': 1414069625,
        'base_branch': 'main', 'profile': 'full', 'coverage_acceptance': 'transition',
        'check_name': 'GateHouse Trusted Verification', 'app_id': 4867361, 'installation_id': 159921491,
        'app_key': '/etc/gatehouse/checks.private-key.pem',
        'delivery': {'app_id': 5069731, 'installation_id': 164696480, 'app_key': '/etc/gatehouse/delivery.private-key.pem'},
        'core': '/opt/gatehouse/bin/harness-gate', 'policy_directory': '/etc/gatehouse/policy',
        'source': str(source), 'mirror': str(VOLUME / 'bootstrap-source/mirror.git'),
        'records': str(CONTROL), 'inbox': str(INBOX),
        'scratch': str(VOLUME / 'scratch'), 'executor_user': 'gatehouse-executor', 'developer_uid': 999,
        'executor_memory_bytes': 8 * 1024**3, 'executor_file_bytes': 1024**3,
        'max_bundle_bytes': 128 * 1024**2, 'max_source_files': 20000, 'max_source_bytes': 256 * 1024**2,
        'timeout_seconds': 1800, 'freshness_seconds': 3600,
        'environment': {'acceptance': 'linux-test-transition-only', 'source_revision': head, 'release_sha256': archive_pin,
            'bootstrap_helper_manifest_sha256': manifest_pin, 'core_build_record_sha256': core_pin,
            'linux_role_binding_sha256': seal_pin, 'input_binding_sha256': input_pin},
        'coverage': list(policy.COVERAGE), 'steps': steps, 'protected_files': pins,
        'runtime': {'root': str(inputs / 'tools/toolchain'), 'inventory': tool_inventory,
                    'tool_lock_sha256': imported['tool_lock_sha256']},
        'cargo_config': sealed['cargo_config'], 'linux_inputs': dict(sealed['linux_inputs']),
        'cache': {'root': str(VOLUME / 'cache'), 'max_bytes': 2 * 1024**3, 'max_entries': 4, 'ttl_seconds': 86400}}
    B.need(old['records'] == config['records'] and old['inbox'] == config['inbox']
           and old['app_id'] == config['app_id'] and old['delivery'] == config['delivery']
           and old['coverage_acceptance'] == 'transition', 'old ingress, journal or App identity differs')
    return config


def runtime_verification(B, payload, config):
    """Import only the two source-bound controller modules in a private in-memory package."""
    name = '_gatehouse_config_' + uuid.uuid4().hex
    package = ModuleType(name);package.__path__ = []
    sys.modules[name] = package
    try:
        for module in ('common', 'runtime'):
            full = name + '.' + module
            value = ModuleType(full);value.__package__ = name
            sys.modules[full] = value
            raw = payload['files/tools/trusted_gate/' + module + '.py']
            exec(compile(raw, '<source-bound-' + module + '>', 'exec'), value.__dict__)
            if module == 'common':
                # Retain the already authenticated bootstrap ownership guard,
                # including its stricter one-link rule, in this temporary package.
                value.protected = B.owned
        return sys.modules[name + '.runtime'].verify(config)
    finally:
        for full in (name + '.runtime', name + '.common', name):sys.modules.pop(full, None)


def artifact_bytes(B, contract, payload, config, old, old_raw, captured, core_root, destination, head):
    raw = B.canonical(config);B.need(len(raw) <= 1024**2, 'host configuration exceeds budget')
    plan = contract.plan(payload)
    plan['captured-Core'] = {'destination': '/opt/gatehouse/bin/harness-gate', 'mode': 0o555, 'action': 'replace',
        'source': str(core_root / 'harness-gate'), 'before_sha256': old['protected_files'][config['core']],
        'after_sha256': captured['binary']['sha256']}
    plan['host-configuration'] = {'destination': str(CONFIG), 'mode': 0o600, 'action': 'replace',
        'source': str(destination / 'host.json'), 'before_sha256': digest(old_raw), 'after_sha256': digest(raw)}
    plan_value = {'schema': 'gatehouse-linux-install-plan/v1', 'head': head, 'files': plan,
        'directories': {str(VOLUME / name): 0o700 for name in ('scratch', 'cache')},
        'required_migration_order': contract.ORDER, 'required_before_installation': [
            'captured-Core-config-and-profile-plan-checks', 'reverified-pilot-preservation',
            'confirmed-old-run-revocation-and-source-retirement', 'guarded-install-and-startup-recovery'],
        'administrator_ready': False, 'production_gate_enabled': False}
    return raw, B.canonical(plan_value)


def preparation_facts(B, builder, contract, helpers, payload, old, head, manifest_pin,
                      input_pin, seal_pin, core_pin, host, host_pin):
    """Read immutable preparation facts; no installed-generation or slot admission."""
    inputs, sealed_root, core_root, source_root = (VOLUME / name for name in (
        'bootstrap-inputs', 'bootstrap-linux-inputs', 'bootstrap-core', 'bootstrap-source'))
    source = source_root / 'source'
    contract.bind_source(B, payload, source, head, manifest_pin)
    B.need(not B.git(source, 'status', '--porcelain', '--untracked-files=all', '--ignored').strip(),
           'staged source is not pristine')
    stage = value(B, source_root / 'stage.json')
    freeze = value(B, source_root / 'frozen.json')
    B.need(stage['schema'] == 'gatehouse-bootstrap-source/v1' and stage['state'] == 'staged'
           and stage['head'] == head and stage['base'] == BASE and stage['helper_manifest_sha256'] == manifest_pin
           and stage['bundle_sha256'] == digest(payload['source.bundle'])
           and stage['tree'] == B.git(source, 'rev-parse', 'HEAD^{tree}').decode().strip()
           and B.canonical(freeze) == B.canonical({'repository': 'musutrade/GateHouse', 'repository_id': 1414069625,
               'head': head, 'base': BASE, 'bundle_sha256': digest(payload['source.bundle'])})
           and B.read(source_root / 'candidate.bundle', contract.MAX_BUNDLE) == payload['source.bundle'],
           'staged source/freeze differs from reviewed release')
    imported = builder.imported_inputs(B, inputs, input_pin, source, head, BASE, helpers)
    B.need(imported['helper_revision'] == head and imported['helper_manifest_sha256'] == manifest_pin,
           'imported inputs belong to another helper release')
    sealed = sealed_inputs(B, builder, sealed_root, seal_pin, head, manifest_pin, input_pin, inputs)
    captured = captured_core(B, builder, helpers, source, inputs, core_root, core_pin,
                             head, manifest_pin, input_pin, host, host_pin)
    os_pins = {name: digest(builder.host_file(name)) for name in ('/usr/bin/python3.14', '/usr/bin/git')}
    B.need(all(old['protected_files'][name] == pin for name, pin in os_pins.items()), 'old OS tool pins changed')
    return imported, sealed, captured, os_pins, stage


def candidate(B, policy, contract, payload, old, old_raw, head, archive_pin, manifest_pin,
              input_pin, seal_pin, core_pin, facts):
    """Deterministic configuration and receipt from independently authenticated facts."""
    imported, sealed, captured, os_pins, _ = facts
    core_root = VOLUME / 'bootstrap-core';destination = VOLUME / 'bootstrap-host-config'
    config = configuration(B, policy, contract, payload, old, head, archive_pin, manifest_pin,
                           input_pin, seal_pin, core_pin, imported, sealed, captured, os_pins)
    verified = runtime_verification(B, payload, config)
    B.need(B.canonical([verified['linux_inputs']['files'], verified['linux_inputs']['bytes']]) ==
           B.canonical([sealed['files'], sealed['bytes']]), 'sealed Linux totals differ')
    status = {'schema': 'gatehouse-linux-host-config/v1', 'state': 'incomplete', 'head': head, 'base': BASE,
              'helper_manifest_sha256': manifest_pin, 'administrator_ready': False, 'production_gate_enabled': False,
              'captured_core_policy_checked': False, 'coverage_acceptance': 'transition', 'authoritative_publish': 'blocked'}
    raw, plan_raw = artifact_bytes(B, contract, payload, config, old, old_raw, captured, core_root, destination, head)
    result = {**status, 'state': 'composed', 'configuration_sha256': digest(raw), 'plan_sha256': digest(plan_raw),
        'old_configuration_sha256': digest(old_raw), 'release_sha256': archive_pin,
        'input_binding_sha256': input_pin, 'linux_role_binding_sha256': seal_pin, 'core_build_record_sha256': core_pin,
        'runtime_verification': verified, 'execution_pins': len(config['protected_files']),
        'scope': 'pinned configuration candidate only; no captured-Core policy check, installation or quality acceptance'}
    return config, verified, status, raw, plan_raw, result


def completed(B, builder, destination, existing_pin, status, result, raw, plan_raw):
    B.owned(destination, directory=True, private=True)
    modes = {'host.json': 0o600, 'install-plan.json': 0o400, 'binding.json': 0o400, 'compose.json': 0o600}
    B.need({path.name for path in destination.iterdir()} == modes.keys(), 'composed configuration inventory differs')
    for name, mode in modes.items():
        path = B.owned(destination / name, private=True)
        B.need(stat.S_IMODE(path.stat().st_mode) == mode, 'composed configuration mode differs')
    saved = builder.json_pin(B, destination / 'binding.json', existing_pin)
    B.need(B.canonical(saved) == B.canonical(result), 'composed configuration binding differs')
    B.need(B.canonical(value(B, destination / 'compose.json')) == B.canonical(
        {**status, 'state': 'composed', 'binding_sha256': existing_pin}), 'configuration has no complete control record')
    B.need(B.read(destination / 'host.json', 1024**2) == raw
           and B.read(destination / 'install-plan.json', 1024**2) == plan_raw, 'configuration outputs changed')


def retained_configuration(B, keep, preflight, contract, builder, preservation, preservation_pin, head, manifest_pin):
    saved = keep.verify(B, preservation, preservation_pin)
    B.need(saved['helper_revision'] == head and saved['helper_manifest_sha256'] == manifest_pin,
           'preservation belongs to another helper release')
    raw = B.read(preservation / 'host.json', 1024**2)
    old, marker, accepted = (json.loads(item, object_pairs_hook=B.unique) for item in (
        raw, B.read(preservation / 'marker.json', 1024**2), B.read(preservation / 'acceptance.json', 1024**2)))
    payload = preflight.accepted_release(B, contract)
    pins = preflight.accepted_pins(B, contract, payload, builder)
    preflight.configuration(B, old, marker, accepted, pins, contract)
    database = keep.connect(preservation / 'journal.sqlite3')
    try:
        identity, rows = keep.database_identity(B, database)
        B.need(identity == saved['journal'], 'retained configuration journal changed')
        run = keep.old_run(B, rows)
    finally:
        database.close()
    inputs = json.loads(run['input'], object_pairs_hook=B.unique)
    policy = json.dumps(old, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()
    B.need(inputs['policy'] == digest(policy) and inputs['repository'] == preflight.REPOSITORY
           and type(inputs['repository_id']) is int and inputs['repository_id'] == preflight.REPOSITORY_ID
           and inputs['base'] == BASE, 'retained accepted configuration identity differs')
    return raw


def verify_prepared(B, helpers, manifest, manifest_pin, archive, archive_pin, input_pin, seal_pin, core_pin,
                    configuration_pin, host, host_pin, recipe, recipe_pin, preservation, preservation_pin, lock_fd):
    """Internal readonly replay, usable after retirement; not installation admission.

    The caller must continuously hold the original controller lease and separately
    admit the current installation phase, current journal/base, entry fence, drain,
    passive Core inspection and retained publication before any mutation. Saved
    pending journal state is evidence only and is never restored or compared to
    the deliberately advanced live journal. There is no administrator entry point.
    """
    B.need(os.geteuid() == 0, 'administrator authentication required for prepared replay')
    B.need(os.uname().sysname == 'Linux' and os.uname().machine == 'x86_64', 'unsupported replay platform')
    B.need(type(lock_fd) is int and lock_fd >= 0, 'prepared replay requires an inherited controller lease')
    code = B.authenticate(helpers, manifest, manifest_pin);head = code['revision']
    helpers, manifest, archive, host, recipe, preservation = map(B.path_operand,
        (helpers, manifest, archive, host, recipe, preservation))
    builder = B.load(helpers, 'build-protected-core.py');volume = B.load(helpers, 'prepare-linux-volume.py')
    contract = B.load(helpers, 'bootstrap-release.py');preflight = B.load(helpers, 'preflight-history-pilot.py')
    policy = B.load(helpers, 'linux-transition-policy.py');keep = B.load(helpers, 'preserve-pilot-state.py')
    fence = B.load(helpers, 'install-transaction.py')
    destination = VOLUME / 'bootstrap-host-config'
    B.need(preservation != VOLUME and VOLUME not in preservation.parents and preservation not in VOLUME.parents,
           'preservation must remain separate from staging storage')

    def guards():
        fence.lease(B, lock_fd)
        builder.worker_identity(B);builder.quiet_units(B);builder.idle(B)
        return volume.admit(B, helpers, manifest, manifest_pin, recipe, recipe_pin)

    before = guards()
    old_raw = retained_configuration(B, keep, preflight, contract, builder, preservation,
                                     preservation_pin, head, manifest_pin)
    old = json.loads(old_raw, object_pairs_hook=B.unique)
    contract.inspect(B, helpers, manifest, manifest_pin, archive, archive_pin)
    archive_raw = B.read(archive, contract.MAX_ARCHIVE)
    payload, _ = contract.decode(B, io.BytesIO(archive_raw), len(archive_raw), archive_pin, code,
                                 B.read(manifest, 64 * 1024))

    def facts():
        return preparation_facts(B, builder, contract, helpers, payload, old, head, manifest_pin,
                                 input_pin, seal_pin, core_pin, host, host_pin)

    initial = facts()
    config, verified, status, raw, plan_raw, result = candidate(B, policy, contract, payload, old, old_raw,
        head, archive_pin, manifest_pin, input_pin, seal_pin, core_pin, initial)
    completed(B, builder, destination, configuration_pin, status, result, raw, plan_raw)
    B.need(facts() == initial and runtime_verification(B, payload, config) == verified,
           'prepared inputs changed during replay')
    B.need(retained_configuration(B, keep, preflight, contract, builder, preservation,
        preservation_pin, head, manifest_pin) == old_raw and B.read(archive, contract.MAX_ARCHIVE) == archive_raw,
        'retained configuration or source release changed during replay')
    completed(B, builder, destination, configuration_pin, status, result, raw, plan_raw)
    B.authenticate(helpers, manifest, manifest_pin)
    B.need(B.canonical(guards()) == B.canonical(before), 'prepared storage changed during replay')
    return {'schema': 'gatehouse-prepared-configuration-replay/v1', 'state': 'verified', 'head': head,
        'helper_manifest_sha256': manifest_pin, 'preservation_sha256': preservation_pin,
        'configuration_binding_sha256': configuration_pin, 'configuration_sha256': result['configuration_sha256'],
        'plan_sha256': result['plan_sha256'], 'administrator_ready': False, 'production_gate_enabled': False,
        'installation_admitted': False, 'captured_core_policy_checked': False,
        'scope': 'immutable prepared configuration only; live phase, retirement, drain and passive inspection admission required'}


def compose(B, helpers, manifest, manifest_pin, archive, archive_pin, input_pin, seal_pin, core_pin,
            host, host_pin, recipe, recipe_pin, lock_fd=None, *, existing_pin=None):
    B.need(os.geteuid() == 0, 'administrator authentication required for host configuration')
    B.need(os.uname().sysname == 'Linux' and os.uname().machine == 'x86_64', 'unsupported configuration platform')
    os.umask(0o077)
    code = B.authenticate(helpers, manifest, manifest_pin);head = code['revision']
    helpers, manifest, archive, host, recipe = map(B.path_operand, (helpers, manifest, archive, host, recipe))
    builder = B.load(helpers, 'build-protected-core.py');volume = B.load(helpers, 'prepare-linux-volume.py')
    contract = B.load(helpers, 'bootstrap-release.py');preflight = B.load(helpers, 'preflight-history-pilot.py')
    policy = B.load(helpers, 'linux-transition-policy.py')
    inputs, sealed_root, core_root, source_root = (VOLUME / name for name in (
        'bootstrap-inputs', 'bootstrap-linux-inputs', 'bootstrap-core', 'bootstrap-source'))
    source = source_root / 'source';destination = VOLUME / 'bootstrap-host-config'
    B.need(all(a != b and a not in b.parents and b not in a.parents for a in (destination,)
               for b in (helpers, manifest, archive, host, recipe, inputs, sealed_root, core_root, source_root)),
           'host configuration output overlaps an input')
    lease = B.owned(CONTROL / 'controller.lock', private=True);own_lease = lock_fd is None
    if own_lease:lock_fd = os.open(lease, os.O_RDWR | os.O_NOFOLLOW)
    try:
        info, expected = os.fstat(lock_fd), lease.stat()
        B.need(stat.S_ISREG(info.st_mode) and (info.st_dev, info.st_ino) == (expected.st_dev, expected.st_ino),
               'host configuration lease differs')
        fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        def guards():
            builder.pilot_slot(B);builder.worker_identity(B);builder.quiet_units(B);builder.idle(B)
            volume.admit(B, helpers, manifest, manifest_pin, recipe, recipe_pin)
            return preflight.observe(B, helpers, manifest, manifest_pin, lock_fd=lock_fd)
        before = guards()
        if existing_pin is None:
            B.need(not destination.exists() and not destination.is_symlink(), 'retain previous host configuration; never reuse')
        else:
            B.owned(destination, directory=True, private=True)
        old_raw = B.read(B.owned(CONFIG, private=True), 1024**2)
        old = json.loads(old_raw, object_pairs_hook=B.unique)
        contract.inspect(B, helpers, manifest, manifest_pin, archive, archive_pin)
        archive_raw = B.read(archive, contract.MAX_ARCHIVE)
        payload, _ = contract.decode(B, io.BytesIO(archive_raw), len(archive_raw), archive_pin, code,
                                     B.read(manifest, 64 * 1024))
        def facts():
            return preparation_facts(B, builder, contract, helpers, payload, old, head, manifest_pin,
                                     input_pin, seal_pin, core_pin, host, host_pin)
        initial = facts()
        config, verified, status, raw, plan_raw, result = candidate(B, policy, contract, payload, old, old_raw,
            head, archive_pin, manifest_pin, input_pin, seal_pin, core_pin, initial)
        def recheck():
            B.need(facts() == initial and runtime_verification(B, payload, config) == verified
                   and B.read(CONFIG, 1024**2) == old_raw and B.read(archive, contract.MAX_ARCHIVE) == archive_raw
                   and B.read(destination / 'host.json', 1024**2) == raw
                   and B.read(destination / 'install-plan.json', 1024**2) == plan_raw,
                   'configuration inputs or outputs changed')
            B.authenticate(helpers, manifest, manifest_pin)
            B.need(guards() == before, 'old pilot changed during configuration')
        if existing_pin is not None:
            completed(B, builder, destination, existing_pin, status, result, raw, plan_raw)
            recheck()
            return result
        destination.mkdir(mode=0o700)
        B.record(destination / 'compose.json', status)
        try:
            B.write(destination / 'host.json', raw, 0o600)
            B.write(destination / 'install-plan.json', plan_raw, 0o400)
            recheck()
            B.write(destination / 'binding.json', B.canonical(result), 0o400)
            B.record(destination / 'compose.json', {**status, 'state': 'composed', 'binding_sha256': digest(B.canonical(result))})
            return result
        except BaseException:
            B.record(destination / 'compose.json', {**status, 'state': 'blocked'})
            raise
    finally:
        if own_lease:os.close(lock_fd)
