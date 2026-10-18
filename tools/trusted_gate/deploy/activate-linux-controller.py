"""Internal forward controller activation; no administrator CLI or quality verdict."""
from __future__ import annotations

import hashlib
import io
import json
import os
from pathlib import Path
import stat
import sys
import time
from types import ModuleType
import uuid

RECEIPT = Path('/srv/gatehouse/pilot/control/controller-activation.json')
SOURCE = Path('/opt/gatehouse/trusted_gate')
INTERPRETER = Path('/usr/bin/python3.14')
PHASES = ('prepared', 'marker-intent', 'marker-active', 'internal-intent', 'internal-enabled',
          'public-intent', 'entries-enabled')
MAX_SECONDS = 600


def digest(raw):return hashlib.sha256(raw).hexdigest()


def same(B, actual, expected, message):
    B.need(B.canonical(actual) == B.canonical(expected), message)


class Context:
    def __init__(self, B, helpers, manifest, manifest_pin, recipe, recipe_pin, lease_fd):
        lifecycle = B.load(helpers, 'coordinate-linux-upgrade.py')
        self.upgrade = lifecycle.Context(B, helpers, manifest, manifest_pin, recipe, recipe_pin, lease_fd)
        self.lifecycle = lifecycle
        archive = B.read(B.owned(self.value['paths']['archive'], private=True), self.contract.MAX_ARCHIVE)
        self.new_payload, _ = self.contract.decode(B, io.BytesIO(archive), len(archive), self.value['pins']['archive'],
                                                 self.code, B.read(self.manifest, 64 * 1024))
        raw = self.material['files'][str(self.files.CONFIG)]['after']
        self.new_config = json.loads(raw, object_pairs_hook=B.unique)

    def __getattr__(self, name):return getattr(object.__getattribute__(self, 'upgrade'), name)


def fixed(ctx):
    B = ctx.B;old = ctx.lifecycle.receipt(ctx)
    B.need(old['phase'] == 'installed-disabled', 'activation requires completed disabled installation')
    return {'schema':'gatehouse-controller-activation/v1', 'head':old['head'], 'transaction':old['transaction'],
        'helper_manifest_sha256':ctx.manifest_pin, 'recipe_sha256':ctx.recipe_pin,
        'material_sha256':ctx.material_pin, 'lease':old['lease'],
        'upgrade_receipt_sha256':digest(B.read(ctx.lifecycle.RECEIPT, ctx.files.MAX_RECEIPT)),
        'file_receipt_sha256':digest(B.read(ctx.files.RECEIPT, ctx.files.MAX_RECEIPT)),
        'entry_fence_sha256':old['entry_fence_sha256'], 'administrator_ready':False, 'production_gate_enabled':False}


def receipt(ctx):
    value = ctx.files.metadata(ctx.B, RECEIPT);expected = fixed(ctx)
    ctx.B.need(set(value) == set(expected) | {'phase'} and value['phase'] in PHASES,
               'unknown controller activation phase or fields')
    same(ctx.B, {key:value[key] for key in expected}, expected, 'activation belongs to another installed generation')
    return value


def entry_modes(ctx, phase):
    public, internal = ctx.files.ENTRIES
    return {public: ((0o755,) if phase == 'entries-enabled' else
                     (0, 0o755) if phase == 'public-intent' else (0,)),
            internal: ((0o755,) if PHASES.index(phase) >= PHASES.index('internal-enabled') else
                       (0, 0o755) if phase == 'internal-intent' else (0,))}


def generation(ctx, value):
    """Known post-install modes only; never relax the disabled file engine's checks."""
    B = ctx.B;fence, files = ctx.fence, ctx.files
    fence.lease(B, ctx.fd);same(B, receipt(ctx), value, 'activation progress changed')
    original = fence.read_receipt(B)
    B.need(original['phase'] == 'fenced' and original['transaction'] == value['transaction']
           and original['head'] == value['head'], 'activation original fence differs')
    same(B, digest(B.canonical(original)), value['entry_fence_sha256'], 'activation original fence changed')
    marker = files.metadata(B, fence.BARRIER)
    inactive, active = fence.marker(B, original, 'fenced'), fence.marker(B, original, 'active')
    allowed = (inactive,) if value['phase'] == 'prepared' else ((inactive, active)
              if value['phase'] == 'marker-intent' else (active,))
    B.need(any(B.canonical(marker) == B.canonical(item) for item in allowed), 'activation barrier is unknown or moved backwards')
    state = files.metadata(B, files.RECEIPT)
    B.need(state['phase'] == 'installed-disabled' and state['inflight'] is None
           and state['transaction'] == value['transaction'] and state['head'] == value['head']
           and state['material_sha256'] == value['material_sha256']
           and state['fence_sha256'] == value['entry_fence_sha256'], 'activation file installation differs')
    B.need([row['path'] for row in state['files_done']] == files.file_order(ctx.material)
           and [row['path'] for row in state['directories_done']] == sorted(map(str, files.DIRECTORIES)),
           'activation file or directory inventory is incomplete')
    for row in state['directories_done']:
        path = Path(row['path']);info = B.owned(path, directory=True).stat()
        same(B, files.directory_identity(info), row['identity'], 'activation directory identity changed')
        B.need(stat.S_IMODE(info.st_mode) == files.DIRECTORIES[path], 'activation directory mode changed')
        names = {Path(target).name for target in ctx.material['files'] if Path(target).parent == path}
        B.need({p.name for p in path.iterdir()} <= names, 'unknown activation directory entry')
    modes = entry_modes(ctx, value['phase']);entries = {}
    for row in state['files_done']:
        path = Path(row['path']);material = ctx.material['files'][str(path)]
        temporary = files.temporary(path, value['transaction'])
        B.need(not temporary.exists() and not temporary.is_symlink(), 'retain installed-file temporary during activation')
        if path in modes:
            info = B.owned(path).stat();mode = stat.S_IMODE(info.st_mode)
            B.need(mode in modes[path], 'activation entry mode is outside its durable intent')
            actual = files.probe(B, path, material['after'], mode)
            # In-place fchmod necessarily changes ctime/mode, but no other identity.
            keys = set(actual) - {'ctime_ns', 'mode'}
            same(B, {key:actual[key] for key in keys}, {key:row['identity'][key] for key in keys},
                 'activation installed entry inode changed')
            entries[str(path)] = actual
        else:
            same(B, files.probe(B, path, material['after'], material['mode']), row['identity'],
                 'activation installed file changed')
    return {'marker':marker, 'entries':entries, 'file_receipt_sha256':value['file_receipt_sha256']}


def host_module(ctx):
    raw = ctx.material['files'][str(SOURCE/'host.py')]['after']
    module = ModuleType('_gatehouse_activation_host_' + uuid.uuid4().hex)
    module.__file__ = str(SOURCE/'host.py')
    exec(compile(raw, module.__file__, 'exec', dont_inherit=True), module.__dict__)
    return module


def snapshot_configuration(ctx, host):
    """Use the actual active marker and installed Snapshot; never invoke CLI.main."""
    B = ctx.B
    B.need(not any(name == 'trusted_gate' or name.startswith('trusted_gate.') for name in sys.modules),
           'activation controller namespace is occupied')
    snapshot = host.Snapshot(SOURCE, ctx.files.CONFIG, INTERPRETER, ctx.fence.BARRIER, RECEIPT)
    finder = host.SourceFinder(snapshot);sys.meta_path.insert(0, finder)
    try:
        cli = __import__('trusted_gate.cli', fromlist=['configuration'])
        same(B, cli.configuration(), ctx.new_config, 'installed controller configuration differs')
        B.need(snapshot.configuration(ctx.files.CONFIG) == ctx.material['files'][str(ctx.files.CONFIG)]['after'],
               'installed Snapshot configuration bytes differ')
        snapshot.verify()
    finally:
        sys.meta_path.remove(finder)
        for name in tuple(sys.modules):
            if name == 'trusted_gate' or name.startswith('trusted_gate.'):sys.modules.pop(name)


def persist_storage(ctx,value):
    module=ctx.B.load(ctx.helpers,'persist-linux-mount.py')
    return module.persist(ctx,value,lambda:live(ctx,value,storage=False))


def observe_storage(ctx,value):
    return ctx.B.load(ctx.helpers,'persist-linux-mount.py').observe(ctx,value)


def live(ctx, value, *, storage=True):
    B = ctx.B;started = time.monotonic();ctx.guard()
    if storage:observe_storage(ctx,value)
    first = generation(ctx, value);context = ctx.drain.proc_context()
    checked = ctx.inputs.inspection(B, ctx.helpers, ctx.manifest, ctx.manifest_pin, ctx.value, ctx.fd, ctx.checker)
    current = ctx.inputs.materials(B, ctx.helpers, ctx.manifest, ctx.manifest_pin, ctx.code, ctx.value, checked)
    same(B, ctx.files.descriptor(B, ctx.contract, current), ctx.files.descriptor(B, ctx.contract, ctx.material),
         'activation prepared materials changed')
    B.need(B.canonical(ctx.new_config) == ctx.material['files'][str(ctx.files.CONFIG)]['after'],
           'activation runtime configuration differs from installed materials')
    B.need(ctx.new_config.get('https_proxy') == ctx.value['api_proxy'],
           'activation HTTPS route differs from sealed installation recipe')
    effect = ctx.effect()
    B.need(effect['phase'] == 'retired' and effect['head'] == value['head']
           and effect['helper_manifest_sha256'] == ctx.manifest_pin
           and effect['preservation_sha256'] == ctx.value['pins']['preservation']
           and effect['remote_revocation_confirmed'] is True and effect['retained_publication_verified'] is True
           and effect['api_transport_sha256'] == digest(B.canonical({'https_proxy':ctx.value['api_proxy']}))
           and effect['installation_admitted'] is False and effect['administrator_ready'] is False
           and effect['production_gate_enabled'] is False, 'activation retired remote/publication inputs differ')
    host = host_module(ctx);host.inventory(SOURCE)
    for name in host.MODULES:
        raw = ctx.material['files'][str(SOURCE/(name+'.py'))]['after']
        compile(raw, str(SOURCE/(name+'.py')), 'exec', dont_inherit=True)
    interpreter, _ = host.read(INTERPRETER, 128 * host.MAX_SOURCE, mode=0o755)
    B.need(digest(interpreter) == ctx.new_config['protected_files'][str(INTERPRETER)], 'activation interpreter changed')
    ctx.compose.runtime_verification(B, ctx.new_payload, ctx.new_config)
    if first['marker']['phase'] == 'active':snapshot_configuration(ctx, host)
    view = ctx.drain.ProcView();one = view.scan(os.getpid());two = view.scan(os.getpid())
    same(B, one, two, 'activation native processes changed')
    same(B, ctx.drain.proc_context(), context, 'activation native namespace changed')
    ctx.guard();same(B, generation(ctx, value), first, 'activation generation changed during observation')
    if storage:observe_storage(ctx,value)
    B.need(time.monotonic() - started <= MAX_SECONDS, 'activation observation exceeded time budget')
    return first


def prepare(ctx):
    B = ctx.B;ctx.guard()
    for path in (RECEIPT, RECEIPT.with_name('.'+RECEIPT.name+'.tmp')):
        B.need(not path.exists() and not path.is_symlink(), 'retain existing controller activation state')
    ctx.observe_files('installed-disabled')
    value = {**fixed(ctx), 'phase':'prepared'}
    B.record(RECEIPT, value)
    return receipt(ctx)


def advance(ctx, value, phase):
    ctx.B.need(PHASES.index(phase) == PHASES.index(value['phase']) + 1, 'activation phase must advance once')
    value = {**value, 'phase':phase};ctx.B.record(RECEIPT, value)
    return receipt(ctx)


def enable(ctx, value, path):
    B = ctx.B;live(ctx, value)
    expected = generation(ctx, value)['entries'][str(path)]
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        same(B, ctx.files.identity(os.fstat(fd)), expected, 'activation entry changed before mode update')
        os.fchmod(fd, 0o755);os.fsync(fd)
        same(B, ctx.fence.identity(os.fstat(fd)), ctx.fence.identity(path.stat()), 'activation entry path changed')
    finally:os.close(fd)
    ctx.files.sync_parent(path);live(ctx, value)


def resume(ctx):
    value = receipt(ctx);persist_storage(ctx,value);live(ctx, value)
    if value['phase'] == 'prepared':value = advance(ctx, value, 'marker-intent')
    if value['phase'] == 'marker-intent':
        observed = live(ctx, value)
        if observed['marker']['phase'] != 'active':
            ctx.B.record(ctx.fence.BARRIER, ctx.fence.marker(ctx.B, ctx.fence.read_receipt(ctx.B), 'active'))
        # A recovered rename must also sync its parent before recording completion.
        ctx.files.sync_parent(ctx.fence.BARRIER);live(ctx, value)
        value = advance(ctx, value, 'marker-active')
    if value['phase'] == 'marker-active':
        live(ctx, value);value = advance(ctx, value, 'internal-intent')
    if value['phase'] == 'internal-intent':
        enable(ctx, value, ctx.files.ENTRIES[1]);value = advance(ctx, value, 'internal-enabled')
    if value['phase'] == 'internal-enabled':
        live(ctx, value);value = advance(ctx, value, 'public-intent')
    if value['phase'] == 'public-intent':
        enable(ctx, value, ctx.files.ENTRIES[0]);value = advance(ctx, value, 'entries-enabled')
    live(ctx, value)
    return value


def activate(B, helpers, manifest, manifest_pin, recipe, recipe_pin, lease_fd):
    ctx = Context(B, helpers, manifest, manifest_pin, recipe, recipe_pin, lease_fd)
    if not RECEIPT.exists() and not RECEIPT.is_symlink():prepare(ctx)
    return resume(ctx)


if __name__ == '__main__':
    raise SystemExit('controller activation is internal; no administrator entry or quality acceptance is available')
