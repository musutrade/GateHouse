"""Compose live disabled-entry prerequisites; no installer, activation or admin CLI."""
from __future__ import annotations

import hashlib
import io
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import time

VOLUME = Path('/srv/gatehouse/linux')
PATHS = {'archive', 'host', 'volume_recipe', 'preservation'}
PINS = {'archive', 'input', 'seal', 'core', 'configuration', 'inspection', 'host', 'volume_recipe', 'preservation'}
UNITS = ('gatehouse-reconcile.service', 'gatehouse-reconcile.timer')
UNIT_ROOTS = tuple(map(Path, ('/etc/systemd/system', '/run/systemd/system', '/usr/local/lib/systemd/system',
    '/usr/lib/systemd/system', '/etc/systemd/system.control', '/run/systemd/system.control',
    '/etc/systemd/system.attached', '/run/systemd/system.attached', '/run/systemd/transient',
    '/run/systemd/generator', '/run/systemd/generator.early', '/run/systemd/generator.late')))
MAX_SECONDS = 600


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def same(B, actual, expected, message):
    B.need(B.canonical(actual) == B.canonical(expected), message)


def recipe(B, path, pin, head, manifest_pin):
    path = B.owned(B.path_operand(path), private=True)
    raw = B.read(path, 64 * 1024)
    B.need(stat.S_IMODE(path.stat().st_mode) == 0o400 and digest(raw) == pin,
           'installation input recipe is not the sealed reviewed bytes')
    value = json.loads(raw, object_pairs_hook=B.unique)
    B.need(raw == B.canonical(value) and set(value) == {'schema', 'head', 'helper_manifest_sha256', 'paths', 'pins', 'api_proxy'}
           and value['schema'] == 'gatehouse-installation-inputs/v1' and value['head'] == head
           and value['helper_manifest_sha256'] == manifest_pin and value['paths'].keys() == PATHS
           and value['pins'].keys() == PINS and all(type(item) is str and re.fullmatch('[0-9a-f]{64}', item)
                                                  for item in value['pins'].values())
           and (value['api_proxy'] is None or type(value['api_proxy']) is str and len(value['api_proxy']) <= 256),
           'installation input recipe identity or inventory differs')
    for name, item in value['paths'].items():
        B.owned(B.path_operand(item), directory=name == 'preservation')
    return value


def autostart(B):
    """The fixed accepted pilot has no reconcile units; reject newly present ones."""
    for root in UNIT_ROOTS:
        if not root.exists() and not root.is_symlink():continue
        B.owned(root, directory=True)
        for unit in UNITS:
            for path in (root / unit, root / (unit + '.d')):
                B.need(not path.exists() and not path.is_symlink(), 'reconcile unit or drop-in appeared during installation')
    keys = {'Id', 'LoadState', 'ActiveState', 'SubState', 'UnitFileState', 'FragmentPath',
            'DropInPaths', 'Triggers', 'TriggeredBy', 'NeedDaemonReload'}
    rows = {}
    for unit in UNITS:
        result = subprocess.run(['/usr/bin/systemctl', 'show', unit, '--property=' + ','.join(sorted(keys))],
            env=B.ENV, capture_output=True, timeout=10, check=True)
        B.need(not result.stderr and len(result.stdout) <= 64 * 1024, 'reconcile unit observation is ambiguous')
        row = {}
        for line in result.stdout.decode('ascii').splitlines():
            key, separator, value = line.partition('=')
            B.need(separator and key in keys and key not in row, 'reconcile unit properties differ')
            row[key] = value
        same(B, row, {'Id': unit, 'LoadState': 'not-found', 'ActiveState': 'inactive', 'SubState': 'dead',
             'UnitFileState': '', 'FragmentPath': '', 'DropInPaths': '', 'Triggers': '', 'TriggeredBy': '',
             'NeedDaemonReload': 'no'}, 'reconcile autostart is not proven absent')
        rows[unit] = row
    return rows


def inspection(B, helpers, manifest, manifest_pin, value, lease_fd, checker):
    p, pins = value['paths'], value['pins']
    result = checker.verify_completed(B, helpers, manifest, manifest_pin, p['archive'], pins['archive'],
        pins['input'], pins['seal'], pins['core'], pins['configuration'], pins['inspection'], p['host'], pins['host'],
        p['volume_recipe'], pins['volume_recipe'], p['preservation'], pins['preservation'], lease_fd)
    B.need(result['head'] == value['head'] and result['helper_manifest_sha256'] == manifest_pin
           and result['preservation_sha256'] == pins['preservation']
           and result['configuration_binding_sha256'] == pins['configuration']
           and result['inspection_binding_sha256'] == pins['inspection']
           and result['captured_core_policy_checked'] is True
           and result['installation_admitted'] is False and result['administrator_ready'] is False
           and result['production_gate_enabled'] is False, 'completed preparation/inspection binding differs')
    return result


def materials(B, helpers, manifest, manifest_pin, code, value, checked):
    """Derive the exact file engine inputs, never accept a caller's destination map."""
    contract = B.load(helpers, 'bootstrap-release.py');preflight = B.load(helpers, 'preflight-history-pilot.py')
    compose = B.load(helpers, 'compose-linux-host.py');builder = B.load(helpers, 'build-protected-core.py')
    keep = B.load(helpers, 'preserve-pilot-state.py');files = B.load(helpers, 'install-reviewed-files.py')
    p, pins = value['paths'], value['pins']
    archive = B.read(B.owned(p['archive'], private=True), contract.MAX_ARCHIVE)
    payload, _ = contract.decode(B, io.BytesIO(archive), len(archive), pins['archive'], code, B.read(manifest, 64 * 1024))
    old_raw = compose.retained_configuration(B, keep, preflight, contract, builder,
        Path(p['preservation']), pins['preservation'], code['revision'], manifest_pin)
    accepted = preflight.accepted_release(B, contract)
    root = VOLUME / 'bootstrap-host-config';core = B.owned(VOLUME / 'bootstrap-core/harness-gate')
    B.need(stat.S_IMODE(core.stat().st_mode) == 0o555, 'captured installation Core mode differs')
    new_raw = B.read(B.owned(root / 'host.json', private=True), 1024**2)
    plan = B.read(B.owned(root / 'install-plan.json', private=True), 1024**2)
    B.need(digest(new_raw) == checked['configuration_sha256'] and digest(plan) == checked['plan_sha256'],
           'installation materials differ from completed inspection')
    return files.materialize(B, contract, payload, code['revision'], accepted['bin/harness-gate'],
        B.read(core, builder.MAX_BINARY), old_raw, new_raw)


def generation(B, fence, files, contract, material, lease_fd, phase):
    """Checkpoint mode leaves full file/temp validation to the enclosing engine.

    Never synthesize a fresh-temp proof here: only the writing engine owns it.
    Standalone completion requires full inspect with no temporary exception.
    """
    original = fence.read_receipt(B);files.barrier(B, fence, original, lease_fd)
    descriptor = files.descriptor(B, contract, material)
    B.need(original['head'] == material['head']
           and original['configuration_sha256'] == descriptor['files'][str(files.CONFIG)]['after_sha256']
           and original['before_configuration_sha256'] == descriptor['files'][str(files.CONFIG)]['before_sha256'],
           'installation material differs from original fenced generation')
    if phase == 'before-files':
        fence.fenced(B, lease_fd)
        for path in (files.RECEIPT, files.RECEIPT.with_name('.' + files.RECEIPT.name + '.tmp')):
            B.need(not path.exists() and not path.is_symlink(), 'retain existing file installation receipt')
        for target, row in material['files'].items():
            files.probe(B, Path(target), row['before'], row['before_mode'])
            temp = files.temporary(Path(target), original['transaction'])
            B.need(not temp.exists() and not temp.is_symlink(), 'retain existing file installation temporary')
        for path in files.DIRECTORIES:
            B.owned(path.parent, directory=True)
            B.need(not path.exists() and not path.is_symlink(), 'fresh installation directory required')
        state = None
    else:
        state = files.metadata(B, files.RECEIPT)
        B.need(state['head'] == material['head'] and state['transaction'] == original['transaction']
               and state['material_sha256'] == digest(B.canonical(descriptor))
               and state['fence_sha256'] == digest(B.canonical(original))
               and state['phase'] in ('intent', 'installing', 'installed-disabled')
               and state['entries_enabled'] is False and state['administrator_ready'] is False
               and state['production_gate_enabled'] is False, 'file checkpoint context differs')
        if phase == 'installed-disabled':
            B.need(state['phase'] == 'installed-disabled', 'file installation is not complete and disabled')
            files.inspect(B, fence, contract, material, state, original, lease_fd)
    entries = {}
    for path in files.ENTRIES:
        info = B.owned(path).stat();row = material['files'][str(path)]
        B.need(stat.S_IMODE(info.st_mode) == 0 and B.read(path, 1024**2) in (row['before'], row['after'])
               and files.identity(path.stat()) == files.identity(info), 'delegated installation entry changed or reopened')
        entries[str(path)] = files.identity(info)
    raw = B.read(B.owned(files.CONFIG, private=True), 1024**2)
    B.need(digest(raw) in (descriptor['files'][str(files.CONFIG)]['before_sha256'],
                          descriptor['files'][str(files.CONFIG)]['after_sha256']), 'installed configuration is unknown')
    return {'head': original['head'], 'transaction': original['transaction'], 'entries': entries,
        'configuration_sha256': digest(raw), 'file_state': state,
        'material_sha256': digest(B.canonical(descriptor)), 'fence_sha256': digest(B.canonical(original))}


def observe(B, helpers, manifest, manifest_pin, recipe_path, recipe_pin, lease_fd, *, phase):
    """Live prerequisites only; checkpoint phase MUST be called inside file engine.

    Future coordinator code must call this afresh, not trust a cached report, and
    return True only to the immediately enclosing engine checkpoint. This is not
    a complete administrator coordinator or authority to activate either entry.
    """
    B.need(os.geteuid() == 0, 'administrator identity required for installation input observation')
    B.need(os.uname().sysname == 'Linux' and os.uname().machine == 'x86_64', 'unsupported installation input platform')
    B.need(type(lease_fd) is int and lease_fd >= 0 and phase in ('before-files', 'file-checkpoint', 'installed-disabled'),
           'known installation phase and inherited lease required')
    started = time.monotonic();code = B.authenticate(helpers, manifest, manifest_pin)
    helpers, manifest, recipe_path = map(B.path_operand, (helpers, manifest, recipe_path))
    value = recipe(B, recipe_path, recipe_pin, code['revision'], manifest_pin)
    fence = B.load(helpers, 'install-transaction.py');files = B.load(helpers, 'install-reviewed-files.py')
    contract = B.load(helpers, 'bootstrap-release.py');checker = B.load(helpers, 'check-core-policy.py')
    effects = B.load(helpers, 'admit-upgrade-effects.py');drain = B.load(helpers, 'controller-drain.py')
    builder = B.load(helpers, 'build-protected-core.py')

    def guards():
        B.need(time.monotonic() - started <= MAX_SECONDS, 'installation prerequisite observation exceeded time budget')
        fence.lease(B, lease_fd);builder.worker_identity(B);builder.quiet_units(B);builder.idle(B)
        return autostart(B)

    def external():
        result = effects.observe(B, helpers, manifest, manifest_pin, value['paths']['preservation'],
                                 value['pins']['preservation'], lease_fd, api_proxy=value['api_proxy'])
        B.need(result['head'] == value['head'] and result['helper_manifest_sha256'] == manifest_pin
               and result['preservation_sha256'] == value['pins']['preservation'] and result['phase'] == 'retired'
               and result['remote_revocation_confirmed'] is True and result['retained_publication_verified'] is True
               and result['api_transport_sha256'] == digest(B.canonical({'https_proxy': value['api_proxy']}))
               and result['installation_admitted'] is False and result['administrator_ready'] is False
               and result['production_gate_enabled'] is False, 'retired remote/publication prerequisites differ')
        return result

    units = guards();context = drain.proc_context()
    checked = inspection(B, helpers, manifest, manifest_pin, value, lease_fd, checker)
    material = materials(B, helpers, manifest, manifest_pin, code, value, checked)
    remote = external();before = generation(B, fence, files, contract, material, lease_fd, phase)
    view = drain.ProcView();first = view.scan(os.getpid());second = view.scan(os.getpid())
    same(B, first, second, 'native process observations changed')
    same(B, drain.proc_context(), context, 'native installation namespace changed')
    same(B, guards(), units, 'installation autostart state changed')
    same(B, external(), remote, 'installation remote/publication state changed')
    same(B, inspection(B, helpers, manifest, manifest_pin, value, lease_fd, checker), checked,
         'installation preparation/inspection changed')
    same(B, files.descriptor(B, contract, materials(B, helpers, manifest, manifest_pin, code, value, checked)),
         files.descriptor(B, contract, material), 'installation materials changed during observation')
    same(B, generation(B, fence, files, contract, material, lease_fd, phase), before,
         'installation generation changed during observation')
    same(B, recipe(B, recipe_path, recipe_pin, code['revision'], manifest_pin), value, 'installation recipe changed')
    B.authenticate(helpers, manifest, manifest_pin);same(B, guards(), units, 'installation final guards changed')
    return {'schema': 'gatehouse-installation-input-observation/v1', 'state': 'observed', 'head': code['revision'],
        'recipe_sha256': recipe_pin, 'helper_manifest_sha256': manifest_pin, 'phase': phase,
        'transaction': before['transaction'], 'material_sha256': before['material_sha256'],
        'configuration_sha256': checked['configuration_sha256'], 'plan_sha256': checked['plan_sha256'],
        'api_transport_sha256': remote['api_transport_sha256'], 'scans': 2, 'processes': len(second),
        'process_observation_sha256': digest(B.canonical(second)), 'controller_drain_proven': False,
        'enclosing_file_checkpoint_required': phase != 'installed-disabled', 'installation_admitted': False,
        'administrator_ready': False, 'production_gate_enabled': False,
        'scope': 'live disabled-entry prerequisites only; immediate file-engine validation and complete coordinator required'}


if __name__ == '__main__':
    raise SystemExit('installation input observation is an internal library; no administrator entry is available')
