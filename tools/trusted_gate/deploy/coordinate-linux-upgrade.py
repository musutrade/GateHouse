"""Internal fixed-659 forward upgrade lifecycle; stops with both entries disabled."""
from __future__ import annotations

from contextlib import contextmanager
import hashlib
import importlib.abc
import importlib.util
import json
import os
from pathlib import Path
import stat
import sys
from types import SimpleNamespace
import uuid

CONTROL = Path('/srv/gatehouse/pilot/control')
RECEIPT = CONTROL / 'linux-upgrade.json'
PHASES = ('prepared', 'fenced', 'revoking', 'revoked', 'copying', 'copied', 'retiring', 'retired',
          'installing', 'installed-disabled')
CODE_PINS = {
    '__init__': '940941e57a44a32f112601a2f9717dfe9dcfaf498d3a10705e0ac8dfc3178194',
    'cache': 'f214e02a9ad2a25bfee900be1d407931dc220f4d3951987bc65cd5a6a3b98182',
    'cli': 'dee95270883c1d9cd4f8ee9fe4fe71e37e68f78e94b10471104ed23a165f0f54',
    'common': '8596d8b9595d9a27870ca61dc37bb36b5cc23971244d57810a240bade7726d7c',
    'control': '02c3ecaef0a5dee1f96addb34cec15ef9ae6f59aa599f95a53652a2d34f2f3ef',
    'execution': 'aee8652bf41f362728096b0261f1596af068897f7c62936550d592154b371fc0',
    'github': 'ebc151fe3e562ff31f0b967efc06317371dcc8720b0532f20073d641732263b9',
    'history': 'cd9daa329b492a6b5a43d12dd5e52b866b9645b4654e823a12c248d831d8174c',
    'host': 'a0b97fac16bdffbc01630562faf585e43fc8c27bf6fd13679d2714f299c23186',
    'isolation': '7a492e225a5090b708d38b8c596fca43a4dbfb07ecd4fde5ffd6641c872976ca',
    'journal': 'e9c6c3b69f9ed5c6f29b1bda2e7dfdebdfc3d06653b0e4e6b3d010fc476aa457',
    'publication': 'ba548a1e91f1e13e197ed35967878c48fef22f240311003f7ee9c01dcd044ba7',
    'source': 'a107b3be218040c7194a06ff26c98225798d2d7f2d0e16fe7296c28b2543ca35'}


def digest(raw):return hashlib.sha256(raw).hexdigest()


def same(B, actual, expected, message):
    B.need(B.canonical(actual) == B.canonical(expected), message)


class ClosedController(importlib.abc.MetaPathFinder, importlib.abc.Loader):
    def __init__(self, B, payload):
        self.prefix = '_gatehouse_upgrade_' + uuid.uuid4().hex
        self.codes = {}
        for name, pin in CODE_PINS.items():
            raw = payload['trusted_gate/' + name + '.py']
            B.need(type(raw) is bytes and digest(raw) == pin, 'accepted controller source differs')
            self.codes[name] = compile(raw, '<accepted659-' + name + '>', 'exec', dont_inherit=True)

    def find_spec(self, fullname, path=None, target=None):
        if fullname != self.prefix and not fullname.startswith(self.prefix + '.'):return None
        name = '__init__' if fullname == self.prefix else fullname[len(self.prefix)+1:]
        if name not in self.codes or name in ('cli', 'host', 'publication'):
            raise ImportError('upgrade may load only the closed old revocation/retirement modules')
        return importlib.util.spec_from_loader(fullname, self, is_package=fullname == self.prefix)

    def create_module(self, spec):return None

    def exec_module(self, module):
        name = '__init__' if module.__name__ == self.prefix else module.__name__[len(self.prefix)+1:]
        module.__file__ = '<accepted659-' + name + '>'
        if name == '__init__':module.__path__ = []
        exec(self.codes[name], module.__dict__)


@contextmanager
def old_controller(B, payload):
    loader = ClosedController(B, payload)
    B.need(not any(name == loader.prefix or name.startswith(loader.prefix + '.') for name in sys.modules),
           'upgrade controller namespace is occupied')
    sys.meta_path.insert(0, loader)
    try:
        yield SimpleNamespace(**{name: __import__(loader.prefix + '.' + name, fromlist=[name])
            for name in ('common', 'control', 'execution', 'github', 'journal', 'source')}, prefix=loader.prefix)
    finally:
        sys.meta_path.remove(loader)
        for name in tuple(sys.modules):
            if name == loader.prefix or name.startswith(loader.prefix + '.'):sys.modules.pop(name)


class RevokeOnly:
    """Old Control receives no method capable of posting success or delivering a PR."""
    def __init__(self, B, raw, config, keep, preflight):
        self.B, self.raw, self.config, self.keep, self.preflight = B, raw, config, keep, preflight

    def binding(self, run, name):
        self.B.need(run['id'] == self.keep.RUN and run['head'] == self.keep.HEAD
                    and run['repository'] == self.preflight.REPOSITORY and name == self.config['check_name'],
                    'revocation belongs to another old run')

    def find_check(self, run, name):
        self.binding(run, name);check = self.raw.find_check(run, name)
        if check is not None:
            self.B.need(type(check['id']) is int and check['id'] > 0 and check['id'] != self.preflight.NEGATIVE_CHECK
                        and check['external_id'] == self.keep.RUN, 'revocation Check identity differs')
        return check

    def update_check(self, run, name, status, conclusion=None, summary=''):
        self.binding(run, name)
        self.B.need(status == 'completed' and conclusion == 'failure', 'upgrade may only withdraw an old Check')
        self.find_check(run, name)
        result = self.raw.update_check(run, name, status, conclusion, summary)
        self.B.need(type(result) is int and result > 0 and result != self.preflight.NEGATIVE_CHECK,
                    'revocation response Check identity differs')
        return result


class Context:
    def __init__(self, B, helpers, manifest, manifest_pin, recipe, recipe_pin, lease_fd):
        B.need(os.geteuid() == 0 and os.uname().sysname == 'Linux' and os.uname().machine == 'x86_64',
               'native administrator identity required for upgrade coordination')
        B.need(type(lease_fd) is int and lease_fd >= 0, 'upgrade requires inherited controller lease')
        self.B, self.helpers, self.manifest = B, B.path_operand(helpers), B.path_operand(manifest)
        self.manifest_pin, self.recipe_path, self.recipe_pin, self.fd = manifest_pin, B.path_operand(recipe), recipe_pin, lease_fd
        self.code = B.authenticate(self.helpers, self.manifest, manifest_pin)
        for attr, name in (('inputs','observe-installation-inputs.py'),('fence','install-transaction.py'),
                          ('files','install-reviewed-files.py'),('contract','bootstrap-release.py'),
                          ('preflight','preflight-history-pilot.py'),('keep','preserve-pilot-state.py'),
                          ('journal','admit-upgrade-journal.py'),('effects','admit-upgrade-effects.py'),
                          ('drain','controller-drain.py'),('builder','build-protected-core.py'),
                          ('checker','check-core-policy.py'),('compose','compose-linux-host.py')):
            setattr(self, attr, B.load(self.helpers, name))
        self.value = self.inputs.recipe(B, self.recipe_path, recipe_pin, self.code['revision'], manifest_pin)
        self.guard()
        checked = self.inputs.inspection(B, self.helpers, self.manifest, manifest_pin, self.value, lease_fd, self.checker)
        self.material = self.inputs.materials(B, self.helpers, self.manifest, manifest_pin, self.code, self.value, checked)
        self.material_pin = digest(B.canonical(self.files.descriptor(B, self.contract, self.material)))
        self.payload = self.preflight.accepted_release(B, self.contract)
        p, pins = self.value['paths'], self.value['pins']
        raw = self.compose.retained_configuration(B, self.keep, self.preflight, self.contract, self.builder,
            Path(p['preservation']), pins['preservation'], self.code['revision'], manifest_pin)
        self.config = json.loads(raw, object_pairs_hook=B.unique)

    def guard(self, *, fenced=False):
        B = self.B;self.fence.lease(B, self.fd)
        self.preflight.accounts(B);self.builder.worker_identity(B);self.builder.quiet_units(B);self.builder.idle(B)
        self.inputs.autostart(B)
        same(B, self.inputs.recipe(B, self.recipe_path, self.recipe_pin, self.code['revision'], self.manifest_pin),
             self.value, 'upgrade recipe changed')
        B.authenticate(self.helpers, self.manifest, self.manifest_pin)
        if fenced:self.drain.observe(B, self.helpers, self.manifest, self.manifest_pin, self.fd)

    def live(self):
        return self.journal.observe(self.B, self.helpers, self.manifest, self.manifest_pin,
            self.value['paths']['preservation'], self.value['pins']['preservation'], self.fd)

    def effect(self):
        return self.effects.observe(self.B, self.helpers, self.manifest, self.manifest_pin,
            self.value['paths']['preservation'], self.value['pins']['preservation'], self.fd, api_proxy=self.value['api_proxy'])

    def observe_files(self, phase):
        return self.inputs.observe(self.B, self.helpers, self.manifest, self.manifest_pin,
                                   self.recipe_path, self.recipe_pin, self.fd, phase=phase)

    @contextmanager
    def mutators(self, *, revoke):
        with old_controller(self.B, self.payload) as modules:
            journal = modules.journal.Journal(self.B.owned(CONTROL / 'journal.sqlite3', private=True))
            client = None
            try:
                executor = modules.execution.Executor(self.config)
                if revoke:
                    client = modules.github.GitHub(self.config['app_id'], self.config['installation_id'],
                        self.config['app_key'], self.config['repository'], permissions={'checks':'write','contents':'read'})
                    self.effects.transport(self.B, client, self.value['api_proxy'])
                yield modules, journal, executor, client
            finally:
                if client is not None:client.token, client.expires = None, 0
                journal.close()


def applicability(ctx, client):
    B = ctx.B;repository = client.repo('GET', '');branch = client.repo('GET', '/git/ref/heads/main')
    B.need(type(repository['id']) is int and repository['id'] == ctx.preflight.REPOSITORY_ID
           and repository['full_name'] == ctx.preflight.REPOSITORY and repository['default_branch'] == 'main'
           and branch['ref'] == 'refs/heads/main' and branch['object']['type'] == 'commit'
           and branch['object']['sha'] == ctx.contract.BASE, 'upgrade repository or baseline changed')


def fixed(ctx, transaction):
    info = os.fstat(ctx.fd)
    return {'schema':'gatehouse-linux-upgrade/v1','head':ctx.code['revision'],'transaction':transaction,
        'helper_manifest_sha256':ctx.manifest_pin,'recipe_sha256':ctx.recipe_pin,'material_sha256':ctx.material_pin,
        'old_run':ctx.keep.RUN,'old_head':ctx.keep.HEAD,'lease':[info.st_dev,info.st_ino],
        'entries_enabled':False,'administrator_ready':False,'production_gate_enabled':False}


def receipt(ctx):
    B = ctx.B;value = ctx.files.metadata(B, RECEIPT)
    B.need(set(value) == set(fixed(ctx, value['transaction'])) | {'phase','entry_fence_sha256'}
           and value['phase'] in PHASES, 'unknown upgrade phase or fields')
    same(B, {key:value[key] for key in fixed(ctx, value['transaction'])}, fixed(ctx, value['transaction']),
         'upgrade receipt belongs to another generation')
    ctx.fence.identifier(B, value['transaction'], value['head'],
        digest(ctx.material['files'][str(ctx.files.CONFIG)]['after']))
    B.need((value['entry_fence_sha256'] is None) == (value['phase'] == 'prepared'), 'upgrade fence phase differs')
    if value['entry_fence_sha256'] is not None:
        same(B, digest(B.canonical(ctx.fence.read_receipt(B))), value['entry_fence_sha256'], 'upgrade original fence changed')
    return value


def advance(ctx, value, phase):
    ctx.B.need(PHASES.index(phase) == PHASES.index(value['phase']) + 1, 'upgrade phase may only advance once')
    value = {**value, 'phase':phase};ctx.B.record(RECEIPT, value)
    return receipt(ctx)


def prepare(ctx):
    B = ctx.B;ctx.guard()
    destination = CONTROL/ctx.keep.RUN/'publication'
    B.need(not destination.exists() and not destination.is_symlink(), 'retain publication without an upgrade transaction')
    for path in (RECEIPT, RECEIPT.with_name('.'+RECEIPT.name+'.tmp'), ctx.fence.RECEIPT, ctx.fence.BARRIER,
                 ctx.fence.RECEIPT.with_name('.'+ctx.fence.RECEIPT.name+'.tmp'),
                 ctx.fence.BARRIER.with_name('.'+ctx.fence.BARRIER.name+'.tmp'),
                 ctx.files.RECEIPT, ctx.files.RECEIPT.with_name('.'+ctx.files.RECEIPT.name+'.tmp')):
        B.need(not path.exists() and not path.is_symlink(), 'retain existing upgrade state')
    B.need(ctx.live()['phase'] == 'prepared', 'upgrade must preserve the original pending run first')
    for target, row in ctx.material['files'].items():
        ctx.files.probe(B, Path(target), row['before'], 0o755 if Path(target) in ctx.files.ENTRIES else row['before_mode'])
    for path in ctx.files.DIRECTORIES:
        B.owned(path.parent, directory=True);B.need(not path.exists() and not path.is_symlink(), 'fresh installation directory required')
    with old_controller(B, ctx.payload) as modules:
        client = modules.github.GitHub(ctx.config['app_id'],ctx.config['installation_id'],ctx.config['app_key'],
                                      ctx.config['repository'],permissions={'contents':'read'})
        try:ctx.effects.transport(B, client, ctx.value['api_proxy']);applicability(ctx, client)
        finally:client.token,client.expires = None,0
    value = {**fixed(ctx, str(uuid.uuid4())), 'phase':'prepared', 'entry_fence_sha256':None}
    B.record(RECEIPT, value);return receipt(ctx)


def fence_identity(ctx, value):
    B = ctx.B;original = ctx.fence.read_receipt(B)
    expected = {'transaction':value['transaction'],'head':value['head'],
        'configuration_sha256':digest(ctx.material['files'][str(ctx.files.CONFIG)]['after']),
        'before_configuration_sha256':digest(ctx.material['files'][str(ctx.files.CONFIG)]['before'])}
    same(B, {key:original[key] for key in expected}, expected, 'upgrade entry fence belongs to another operation')
    same(B, {path:row['sha256'] for path,row in original['entries'].items()},
        {str(path):digest(ctx.material['files'][str(path)]['before']) for path in ctx.files.ENTRIES}, 'upgrade old entry pins differ')


def saved_run(ctx):
    p = Path(ctx.value['paths']['preservation']);saved = ctx.keep.verify(ctx.B, p, ctx.value['pins']['preservation'])
    identity, rows = ctx.journal.snapshot(ctx.B, ctx.keep, p/'journal.sqlite3', live=False)
    same(ctx.B, identity, saved['journal'], 'upgrade saved journal differs')
    return p, saved, ctx.keep.old_run(ctx.B, rows)


def copy_tree(ctx, value):
    B = ctx.B;p, saved, run = saved_run(ctx)
    destination = CONTROL/ctx.keep.RUN/'publication'
    temporary = destination.with_name('.publication-'+value['transaction']+'.new')
    if run['evidence'] is None:
        B.need(not temporary.exists() and not temporary.is_symlink(), 'retain unexpected publication temporary')
        ctx.effects.publication(B, ctx.keep, saved, p, ctx.config, run);return
    evidence = json.loads(run['evidence'], object_pairs_hook=B.unique);source = B.path_operand(evidence['directory'])
    B.need(evidence['run'] == ctx.keep.RUN and source.parent == Path(ctx.config['source'])/'.gatehouse-host/reports/invocations',
           'upgrade retained invocation identity differs')
    prefix = 'source/'+str(source.relative_to(Path(ctx.config['source'])))
    files = {key[len(prefix)+1:]:row for key,row in saved['inventory']['files'].items() if key.startswith(prefix+'/')}
    dirs = {key[len(prefix)+1:]:mode for key,mode in saved['inventory']['directories'].items() if key.startswith(prefix+'/')}
    B.need(prefix in saved['inventory']['directories'] and files, 'upgrade invocation was not preserved')
    if destination.exists() or destination.is_symlink():
        B.need(not temporary.exists() and not temporary.is_symlink(), 'ambiguous publication destinations')
        ctx.effects.publication(B, ctx.keep, saved, p, ctx.config, run);ctx.files.sync_parent(destination);return
    B.owned(destination.parent, directory=True, private=True)
    exists = temporary.exists() or temporary.is_symlink()
    if not exists:
        temporary.mkdir(mode=0o700)
        for name in sorted(dirs, key=lambda item:(len(Path(item).parts),item)):
            (temporary/name).mkdir(mode=0o700)
        for name,row in sorted(files.items()):
            raw = B.read(p/'tree'/prefix/name, ctx.keep.MAX_FILE)
            B.need([len(raw),digest(raw)] == row[:2], 'saved publication bytes changed')
            B.write(temporary/name, raw, row[2])
        for name,mode in sorted(dirs.items(), key=lambda item:len(Path(item[0]).parts), reverse=True):
            (temporary/name).chmod(mode)
        temporary.chmod(saved['inventory']['directories'][prefix])
    actual_files, actual_dirs, entries, total = set(),set(),0,0
    B.owned(temporary,directory=True,private=True)
    B.need(stat.S_IMODE(temporary.stat().st_mode) == saved['inventory']['directories'][prefix], 'publication temporary root mode differs')
    for parent,names,leaves in os.walk(temporary):
        parent = B.owned(parent,directory=True);relative = str(parent.relative_to(temporary))
        entries += 1 + len(leaves)
        B.need(entries <= ctx.keep.MAX_ENTRIES, 'publication temporary entry budget exceeded')
        if parent != temporary:
            B.need(relative in dirs and stat.S_IMODE(parent.stat().st_mode) == dirs[relative], 'unknown publication temporary directory')
            actual_dirs.add(relative)
        for name in names:B.owned(parent/name,directory=True)
        for name in leaves:
            path = B.owned(parent/name);key = str(path.relative_to(temporary))
            B.need(key in files, 'unknown publication temporary file')
            total += path.stat().st_size
            B.need(total <= ctx.keep.MAX_TOTAL, 'publication temporary byte budget exceeded')
            raw = B.read(path, ctx.keep.MAX_FILE)
            B.need([len(raw),digest(raw),stat.S_IMODE(path.stat().st_mode)] == files[key], 'publication temporary differs')
            actual_files.add(key)
        fd = os.open(parent,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW)
        try:os.fsync(fd)
        finally:os.close(fd)
    B.need(actual_files == files.keys() and actual_dirs == dirs.keys(), 'retain incomplete publication temporary')
    ctx.guard(fenced=True);ctx.files.sync_parent(temporary)
    B.need(not destination.exists() and not destination.is_symlink(), 'publication appeared before rename')
    os.replace(temporary,destination);ctx.files.sync_parent(destination)
    ctx.effects.publication(B,ctx.keep,saved,p,ctx.config,run)


def resume(ctx):
    B = ctx.B;value = receipt(ctx);ctx.guard()
    if value['phase'] == 'prepared':
        B.need(ctx.live()['phase'] == 'prepared', 'old run advanced before upgrade fence')
        if not ctx.fence.RECEIPT.exists():
            ctx.fence.prepare(B,value['transaction'],value['head'],digest(ctx.material['files'][str(ctx.files.CONFIG)]['after']),
                {str(path):digest(ctx.material['files'][str(path)]['before']) for path in ctx.files.ENTRIES},ctx.fd)
        fence_identity(ctx,value);ctx.fence.fence(B,ctx.fd)
        value['entry_fence_sha256'] = digest(B.canonical(ctx.fence.read_receipt(B)));value = advance(ctx,value,'fenced')
    if value['phase'] == 'fenced':
        ctx.guard(fenced=True);B.need(ctx.live()['phase'] == 'prepared', 'old run advanced before revocation intent')
        value = advance(ctx,value,'revoking')
    if value['phase'] == 'revoking':
        ctx.guard(fenced=True);B.need(ctx.live()['phase'] in ('prepared','revoking','revoked'), 'unexpected revocation recovery phase')
        with ctx.mutators(revoke=True) as (modules,journal,executor,client):
            applicability(ctx,client);wrapped = RevokeOnly(B,client,ctx.config,ctx.keep,ctx.preflight)
            modules.control.Control(journal,wrapped,executor,ctx.config).revoke(ctx.keep.RUN,ctx.journal.REASON)
        B.need(ctx.live()['phase'] == 'revoked', 'old journal revocation is incomplete');value = advance(ctx,value,'revoked')
    if value['phase'] == 'revoked':
        ctx.guard(fenced=True);B.need(ctx.live()['phase'] == 'revoked', 'unexpected publication preparation phase')
        destination = CONTROL/ctx.keep.RUN/'publication';temporary = destination.with_name('.publication-'+value['transaction']+'.new')
        B.need(not destination.exists() and not destination.is_symlink() and not temporary.exists() and not temporary.is_symlink(),
               'retain publication state without a coordinator copy intent')
        value = advance(ctx,value,'copying')
    if value['phase'] == 'copying':
        ctx.guard(fenced=True);B.need(ctx.live()['phase'] == 'revoked', 'source advanced before complete publication')
        copy_tree(ctx,value);B.need(ctx.effect()['phase'] == 'revoked', 'remote/copy revocation is not confirmed')
        value = advance(ctx,value,'copied')
    if value['phase'] == 'copied':
        ctx.guard(fenced=True);B.need(ctx.effect()['phase'] == 'revoked', 'retirement prerequisites changed')
        value = advance(ctx,value,'retiring')
    if value['phase'] == 'retiring':
        ctx.guard(fenced=True);phase = ctx.effect()['phase']
        B.need(phase in ('revoked','retiring','retired'), 'unknown source retirement phase')
        if phase != 'retired':
            with ctx.mutators(revoke=False) as (modules,journal,executor,client):
                modules.source.retire(ctx.config,executor,journal,ctx.keep.RUN)
        B.need(ctx.effect()['phase'] == 'retired', 'old source retirement is incomplete');value = advance(ctx,value,'retired')
    if value['phase'] == 'retired':
        ctx.observe_files('before-files');value = advance(ctx,value,'installing')
    if value['phase'] == 'installing':
        def guard():
            ctx.observe_files('file-checkpoint' if ctx.files.RECEIPT.exists() else 'before-files');return True
        if not ctx.files.RECEIPT.exists():ctx.files.prepare(B,ctx.fence,ctx.contract,ctx.material,ctx.fd,guard)
        ctx.files.resume(B,ctx.fence,ctx.contract,ctx.material,ctx.fd,guard)
        ctx.observe_files('installed-disabled');value = advance(ctx,value,'installed-disabled')
    ctx.observe_files('installed-disabled');return value


def coordinate(B, helpers, manifest, manifest_pin, recipe, recipe_pin, lease_fd):
    ctx = Context(B,helpers,manifest,manifest_pin,recipe,recipe_pin,lease_fd)
    if not RECEIPT.exists() and not RECEIPT.is_symlink():prepare(ctx)
    return resume(ctx)


if __name__ == '__main__':
    raise SystemExit('upgrade lifecycle is internal and stops disabled; no administrator entry is available')
