"""Authenticated fixed-659 preparation/dispatch; never run a quality workload."""
from __future__ import annotations

import hashlib
import io
import json
import os
from pathlib import Path
import re
import stat
from types import SimpleNamespace
import urllib.request
import uuid

CONTROL = Path('/srv/gatehouse/pilot/control')
VOLUME = Path('/srv/gatehouse/linux')
RETENTION = Path('/var/lib/gatehouse')
RECEIPT = CONTROL / 'administrator-upgrade.json'
INPUT_RECIPE = VOLUME / 'administrator-installation-inputs.json'
BUNDLE = VOLUME / 'administrator-source.bundle'
STEPS = ('volume', 'preservation', 'source', 'input', 'seal', 'core', 'configuration', 'inspection', 'recipe')
PATHS = {'release', 'host_recipe', 'tool_recipe', 'tool_archives', 'crate_archives', 'test_archives', 'onboarding'}
PINS = {'release', 'host_recipe', 'tool_recipe'}


def digest(raw):return hashlib.sha256(raw).hexdigest()


def same(B, actual, expected, message):
    B.need(B.canonical(actual) == B.canonical(expected), message)


def exists(path):return path.exists() or path.is_symlink()


def recipe(B, path, pin, head, manifest_pin):
    path = B.owned(B.path_operand(path), private=True);raw = B.read(path, 64 * 1024)
    B.need(stat.S_IMODE(path.stat().st_mode) == 0o400 and digest(raw) == pin,
           'administrator recipe must be the sealed reviewed bytes')
    value = json.loads(raw, object_pairs_hook=B.unique)
    B.need(raw == B.canonical(value) and set(value) == {'schema', 'head', 'helper_manifest_sha256', 'paths', 'pins', 'api_proxy'}
           and value['schema'] == 'gatehouse-administrator-upgrade/v1' and value['head'] == head
           and value['helper_manifest_sha256'] == manifest_pin and value['paths'].keys() == PATHS
           and value['pins'].keys() == PINS and all(type(item) is str and re.fullmatch('[0-9a-f]{64}', item)
                                                  for item in value['pins'].values())
           and (value['api_proxy'] is None or type(value['api_proxy']) is str and len(value['api_proxy']) <= 256),
           'administrator recipe identity or inventory differs')
    for name, item in value['paths'].items():
        target = B.owned(B.path_operand(item), directory=name not in PINS)
        B.need(target != VOLUME and VOLUME not in target.parents and target not in VOLUME.parents
               and target != CONTROL and CONTROL not in target.parents and target not in CONTROL.parents,
               'administrator input overlaps mutable deployment storage')
    return value


class Context:
    def __init__(self, B, helpers, manifest, manifest_pin, recipe_path, recipe_pin, fd):
        B.need(os.geteuid() == 0 and os.uname().sysname == 'Linux' and os.uname().machine == 'x86_64',
               'native administrator identity required')
        B.need(type(fd) is int and fd >= 0, 'original administrator lease required')
        self.B, self.helpers, self.manifest = B, B.path_operand(helpers), B.path_operand(manifest)
        self.manifest_pin, self.recipe_path, self.recipe_pin, self.fd = manifest_pin, B.path_operand(recipe_path), recipe_pin, fd
        self.code = B.authenticate(self.helpers, self.manifest, manifest_pin)
        for attr, name in (('fence','install-transaction.py'), ('files','install-reviewed-files.py'),
                          ('contract','bootstrap-release.py'), ('preflight','preflight-history-pilot.py'),
                          ('builder','build-protected-core.py'), ('volume','prepare-linux-volume.py'),
                          ('keep','preserve-pilot-state.py'), ('staging','stage-protected-source.py'),
                          ('sealer','seal-linux-inputs.py'), ('compose','compose-linux-host.py'),
                          ('checker','check-core-policy.py'), ('journal','admit-upgrade-journal.py'),
                          ('lifecycle','coordinate-linux-upgrade.py'), ('activation','activate-linux-controller.py'),
                          ('effects','admit-upgrade-effects.py'), ('inputs','observe-installation-inputs.py')):
            setattr(self, attr, B.load(self.helpers, name))
        self.value = recipe(B, self.recipe_path, recipe_pin, self.code['revision'], manifest_pin)
        self.p, self.pins = self.value['paths'], self.value['pins']
        self.preservation = RETENTION / ('retained-linux-' + self.code['revision'][:12])
        # Validate the explicit route without creating any token or network request.
        route = SimpleNamespace(opener=urllib.request.build_opener(urllib.request.ProxyHandler({})))
        self.effects.transport(B, route, self.value['api_proxy'])
        raw = B.read(self.p['release'], self.contract.MAX_ARCHIVE)
        self.payload, _ = self.contract.decode(B, io.BytesIO(raw), len(raw), self.pins['release'], self.code,
                                               B.read(self.manifest, 64 * 1024))
        self.builder.host_inputs(B, self.p['host_recipe'], self.pins['host_recipe'])
        self.volume.tools(B, self.builder, self.p['tool_recipe'], self.pins['tool_recipe'])
        self.guard()

    def guard(self, *, preparing=False):
        B = self.B;self.fence.lease(B, self.fd)
        B.owned(CONTROL, directory=True, private=True)
        self.preflight.accounts(B);self.builder.worker_identity(B);self.builder.quiet_units(B);self.builder.idle(B)
        self.inputs.autostart(B)
        same(B, recipe(B, self.recipe_path, self.recipe_pin, self.code['revision'], self.manifest_pin), self.value,
             'administrator recipe changed')
        B.authenticate(self.helpers, self.manifest, self.manifest_pin)
        if preparing:self.builder.pilot_slot(B)

    def target(self, step):
        return {'volume':self.volume.MARKER, 'preservation':self.preservation/'preservation.json',
            'source':VOLUME/'bootstrap-source/stage.json', 'input':VOLUME/'bootstrap-inputs/binding.json',
            'seal':VOLUME/'bootstrap-linux-inputs/role-binding.json', 'core':VOLUME/'bootstrap-core/build.json',
            'configuration':VOLUME/'bootstrap-host-config/binding.json',
            'inspection':VOLUME/'bootstrap-core-inspection/binding.json', 'recipe':INPUT_RECIPE}[step]

    def fresh(self, step):
        targets = {'volume':(self.volume.MARKER, self.volume.IMAGE), 'preservation':(self.preservation,),
            'source':(VOLUME/'bootstrap-source', BUNDLE),
            'input':(VOLUME/'bootstrap-inputs', VOLUME/'bootstrap-inputs-import.json'),
            'seal':(VOLUME/'bootstrap-linux-inputs',), 'core':(VOLUME/'bootstrap-core',),
            'configuration':(VOLUME/'bootstrap-host-config',), 'inspection':(VOLUME/'bootstrap-core-inspection',),
            'recipe':(INPUT_RECIPE,)}[step]
        for path in targets:
            self.B.need(not exists(path) and not exists(path.with_name('.'+path.name+'.tmp')),
                        'retain preparation output without its administrator intent')

    def verify(self, step, pins, pin):
        B = self.B;head = self.code['revision'];source = VOLUME/'bootstrap-source/source';inputs = VOLUME/'bootstrap-inputs'
        B.need(artifact_pin(self, step) == pin, 'administrator completed artifact changed')
        if step == 'volume':
            result = self.volume.admit(B, self.helpers, self.manifest, self.manifest_pin,
                                      self.p['tool_recipe'], self.pins['tool_recipe'])
        elif step == 'preservation':
            result = self.keep.verify(B, self.preservation, pin)
            B.need(result['helper_revision'] == head and result['helper_manifest_sha256'] == self.manifest_pin,
                   'preservation belongs to another administrator release')
            B.need(self.journal.observe(B, self.helpers, self.manifest, self.manifest_pin,
                       self.preservation, pin, self.fd)['phase'] == 'prepared', 'old run advanced during preparation')
        elif step == 'source':
            self.contract.bind_source(B, self.payload, source, head, self.manifest_pin)
            B.need(not B.git(source, 'status', '--porcelain', '--untracked-files=all', '--ignored'),
                   'administrator source stage is not pristine')
            result = self.compose.value(B, self.target(step))
            B.need(result['schema'] == 'gatehouse-bootstrap-source/v1' and result['state'] == 'staged'
                   and result['head'] == head and result['base'] == self.contract.BASE
                   and result['helper_manifest_sha256'] == self.manifest_pin
                   and result['bundle_sha256'] == digest(self.payload['source.bundle'])
                   and result['tree'] == B.git(source, 'rev-parse', 'HEAD^{tree}').decode().strip(), 'source stage receipt differs')
            same(B, self.compose.value(B, VOLUME/'bootstrap-source/frozen.json'),
                 {'repository':'musutrade/GateHouse', 'repository_id':1414069625, 'head':head,
                  'base':self.contract.BASE, 'bundle_sha256':digest(self.payload['source.bundle'])}, 'source stage freeze differs')
            B.need(B.read(VOLUME/'bootstrap-source/candidate.bundle', self.contract.MAX_BUNDLE) == self.payload['source.bundle']
                   and B.read(BUNDLE, self.contract.MAX_BUNDLE) == self.payload['source.bundle'], 'source stage bundle changed')
        elif step == 'input':
            result = self.builder.imported_inputs(B, inputs, pin, source, head, self.contract.BASE, self.helpers)
            B.need(result['helper_revision'] == head and result['helper_manifest_sha256'] == self.manifest_pin,
                   'import belongs to another administrator release')
        elif step == 'seal':
            result = self.compose.sealed_inputs(B, self.builder, VOLUME/'bootstrap-linux-inputs', pin,
                                               head, self.manifest_pin, pins['input'], inputs)
            imported = self.builder.imported_inputs(B, inputs, pins['input'], source, head, self.contract.BASE, self.helpers)
            runtime = {'source':str(source), 'runtime':{'root':str(inputs/'tools/toolchain'),
                'inventory':str(inputs/'tools/inventory.json'), 'tool_lock_sha256':imported['tool_lock_sha256']},
                'linux_inputs':result['linux_inputs'], 'cargo_config':result['cargo_config'],
                'protected_files':{str(inputs/'tools/inventory.json'):imported['tool_inventory_sha256'], **result['protected_files']}}
            self.compose.runtime_verification(B, self.payload, runtime)
        elif step == 'core':
            result = self.compose.captured_core(B, self.builder, self.helpers, source, inputs, VOLUME/'bootstrap-core',
                pin, head, self.manifest_pin, pins['input'], self.p['host_recipe'], self.pins['host_recipe'])
        elif step == 'configuration':
            result = self.compose.compose(B, self.helpers, self.manifest, self.manifest_pin, self.p['release'],
                self.pins['release'], pins['input'], pins['seal'], pins['core'], self.p['host_recipe'],
                self.pins['host_recipe'], self.p['tool_recipe'], self.pins['tool_recipe'], self.fd, existing_pin=pin,
                api_proxy=self.value['api_proxy'])
        elif step == 'inspection':
            result = self.checker.verify_completed(B, self.helpers, self.manifest, self.manifest_pin,
                self.p['release'], self.pins['release'], pins['input'], pins['seal'], pins['core'],
                pins['configuration'], pin, self.p['host_recipe'], self.pins['host_recipe'],
                self.p['tool_recipe'], self.pins['tool_recipe'], self.preservation, pins['preservation'], self.fd)
        else:
            result = self.inputs.recipe(B, INPUT_RECIPE, pin, head, self.manifest_pin)
            same(B, result, self.installation_recipe(pins), 'sealed installation recipe differs')
        return result

    def installation_recipe(self, pins):
        return {'schema':'gatehouse-installation-inputs/v1', 'head':self.code['revision'],
            'helper_manifest_sha256':self.manifest_pin, 'api_proxy':self.value['api_proxy'],
            'paths':{'archive':self.p['release'], 'host':self.p['host_recipe'],
                     'volume_recipe':self.p['tool_recipe'], 'preservation':str(self.preservation)},
            'pins':{'archive':self.pins['release'], 'host':self.pins['host_recipe'], 'volume_recipe':self.pins['tool_recipe'],
                    **{name:pins[name] for name in ('input','seal','core','configuration','inspection','preservation')}}}

    def perform(self, step, pins):
        B = self.B;head = self.code['revision'];source = VOLUME/'bootstrap-source/source';inputs = VOLUME/'bootstrap-inputs'
        shared = (B, self.helpers, self.manifest, self.manifest_pin)
        if step == 'volume':self.volume.prepare(*shared, self.p['tool_recipe'], self.pins['tool_recipe'], self.fd)
        elif step == 'preservation':self.keep.preserve(*shared, self.preservation, self.fd)
        elif step == 'source':
            B.write(BUNDLE, self.payload['source.bundle'], 0o400);self.files.sync_parent(BUNDLE)
            self.staging.stage(*shared, BUNDLE, digest(self.payload['source.bundle']), head,
                               self.p['tool_recipe'], self.pins['tool_recipe'], self.fd)
        elif step == 'input':
            B.import_inputs(self.helpers, self.manifest, self.manifest_pin, source, head, self.contract.BASE,
                            self.p['tool_archives'], self.p['crate_archives'], inputs)
        elif step == 'seal':
            self.sealer.seal(*shared, source, head, inputs, pins['input'], self.p['test_archives'], self.p['onboarding'],
                             self.p['tool_recipe'], self.pins['tool_recipe'], self.fd)
        elif step == 'core':
            self.builder.build(*shared, source, head, self.contract.BASE, inputs, pins['input'],
                               self.p['host_recipe'], self.pins['host_recipe'], VOLUME/'bootstrap-core', self.fd)
        elif step == 'configuration':
            self.compose.compose(*shared, self.p['release'], self.pins['release'], pins['input'], pins['seal'], pins['core'],
                                 self.p['host_recipe'], self.pins['host_recipe'], self.p['tool_recipe'], self.pins['tool_recipe'], self.fd,
                                 api_proxy=self.value['api_proxy'])
        elif step == 'inspection':
            self.checker.inspect(*shared, self.p['release'], self.pins['release'], pins['input'], pins['seal'], pins['core'],
                                 pins['configuration'], self.p['host_recipe'], self.pins['host_recipe'],
                                 self.p['tool_recipe'], self.pins['tool_recipe'], self.fd, api_proxy=self.value['api_proxy'])
        else:B.write(INPUT_RECIPE, B.canonical(self.installation_recipe(pins)), 0o400);self.files.sync_parent(INPUT_RECIPE)

    def dispatch(self, recipe_pin):
        shared = (self.B, self.helpers, self.manifest, self.manifest_pin, INPUT_RECIPE, recipe_pin, self.fd)
        # After an active marker, old fenced-file observation correctly rejects.
        # Resume activation directly, never replay that historical file phase.
        if not exists(self.activation.RECEIPT):self.lifecycle.coordinate(*shared)
        result = self.activation.activate(*shared)
        self.B.need(result['phase'] == 'entries-enabled' and result['head'] == self.code['revision']
                    and result['administrator_ready'] is False and result['production_gate_enabled'] is False,
                    'administrator activation did not complete its reviewed component')
        return result


def fixed(ctx, transaction):
    info = os.fstat(ctx.fd)
    return {'schema':'gatehouse-administrator-dispatch/v1', 'head':ctx.code['revision'], 'transaction':transaction,
        'helper_manifest_sha256':ctx.manifest_pin, 'recipe_sha256':ctx.recipe_pin, 'lease':[info.st_dev,info.st_ino],
        'administrator_ready':False, 'production_gate_enabled':False}


def receipt(ctx):
    B = ctx.B;value = ctx.files.metadata(B, RECEIPT);identity = fixed(ctx, value['transaction'])
    parsed = uuid.UUID(value['transaction'])
    B.need(parsed.version == 4 and str(parsed) == value['transaction'] and set(value) == set(identity) | {'phase','completed','inflight'}
           and value['phase'] in ('prepared','preparing','dispatching','complete'), 'unknown administrator transaction')
    same(B, {name:value[name] for name in identity}, identity, 'administrator transaction belongs to another release')
    rows = value['completed']
    B.need(isinstance(rows, list) and len(rows) <= len(STEPS) and all(isinstance(row, dict)
           and set(row) == {'step','sha256'} and type(row['sha256']) is str
           and re.fullmatch('[0-9a-f]{64}', row['sha256']) for row in rows)
           and [row['step'] for row in rows] == list(STEPS[:len(rows)]), 'administrator preparation is not an ordered prefix')
    remaining = STEPS[len(rows):]
    B.need(value['inflight'] is None or bool(remaining) and value['inflight'] == remaining[0], 'unknown preparation intent')
    B.need(value['phase'] != 'prepared' or not rows and value['inflight'] is None,
           'initial administrator preparation already advanced')
    B.need(value['phase'] not in ('dispatching','complete') or not remaining and value['inflight'] is None,
           'administrator dispatch preceded complete preparation')
    return value


def record(ctx, value):
    ctx.B.record(RECEIPT, value);return receipt(ctx)


def artifact_pin(ctx, step):
    target = ctx.target(step)
    ctx.B.need(not exists(target.with_name('.'+target.name+'.tmp')),
               'retain unfinished preparation artifact metadata')
    return digest(ctx.B.read(ctx.B.owned(target, private=step != 'input'), 1024**2))


def prepare(ctx):
    B = ctx.B;ctx.guard(preparing=True)
    B.need(not exists(RECEIPT) and not exists(RECEIPT.with_name('.'+RECEIPT.name+'.tmp')),
           'retain existing administrator transaction')
    for path in (ctx.lifecycle.RECEIPT, ctx.activation.RECEIPT, ctx.files.RECEIPT, ctx.fence.RECEIPT, ctx.fence.BARRIER):
        B.need(not exists(path) and not exists(path.with_name('.'+path.name+'.tmp')), 'retain existing installation state')
    # No complete output is adopted without a prior, durable intent from this run.
    for step in STEPS:ctx.fresh(step)
    ctx.preflight.observe(B, ctx.helpers, ctx.manifest, ctx.manifest_pin, lock_fd=ctx.fd, api_proxy=ctx.value['api_proxy'])
    return record(ctx, {**fixed(ctx, str(uuid.uuid4())), 'phase':'prepared', 'completed':[], 'inflight':None})


def resume(ctx):
    value = receipt(ctx);ctx.guard()
    for row in value['completed']:
        ctx.B.need(artifact_pin(ctx, row['step']) == row['sha256'], 'completed stage hash changed')
    if value['phase'] == 'prepared':value = record(ctx, {**value, 'phase':'preparing'})
    if value['phase'] == 'preparing':
        pins = {}
        for row in value['completed']:
            ctx.guard(preparing=True);ctx.verify(row['step'], pins, row['sha256']);pins[row['step']] = row['sha256']
        for step in STEPS[len(value['completed']):]:
            ctx.guard(preparing=True)
            if value['inflight'] is None:
                ctx.fresh(step);value = record(ctx, {**value, 'inflight':step})
                ctx.perform(step, pins)
            # On restart, verify the known completed artifact; never repeat build
            # or silently continue a partial output after an ambiguous effect.
            target = ctx.target(step);pin = artifact_pin(ctx, step)
            ctx.verify(step, pins, pin);ctx.files.sync_parent(target);ctx.guard(preparing=True)
            value = record(ctx, {**value, 'completed':[*value['completed'], {'step':step, 'sha256':pin}], 'inflight':None})
            pins[step] = pin
        value = record(ctx, {**value, 'phase':'dispatching'})
    recipe_pin = value['completed'][-1]['sha256']
    ctx.inputs.recipe(ctx.B, INPUT_RECIPE, recipe_pin, ctx.code['revision'], ctx.manifest_pin)
    ctx.dispatch(recipe_pin)
    if value['phase'] == 'dispatching':value = record(ctx, {**value, 'phase':'complete'})
    return value


def run(B, helpers, manifest, manifest_pin, recipe_path, recipe_pin):
    # Authentication and protected lease-path validation precede helper imports.
    B.need(os.geteuid() == 0, 'administrator authentication required')
    B.authenticate(helpers, manifest, manifest_pin)
    helpers = B.path_operand(helpers);fence = B.load(helpers, 'install-transaction.py')
    lease = B.owned(fence.LEASE, private=True)
    fd = os.open(lease, os.O_RDWR | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        fence.lease(B, fd)
        ctx = Context(B, helpers, manifest, manifest_pin, recipe_path, recipe_pin, fd)
        if not exists(RECEIPT):prepare(ctx)
        return resume(ctx)
    finally:os.close(fd)


if __name__ == '__main__':
    raise SystemExit('use the authenticated importer administrator-upgrade entry; no standalone invocation')
