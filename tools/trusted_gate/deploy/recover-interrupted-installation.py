"""Narrow administrator readmission for the interrupted063 installation, never quality delivery.

This operator add-on is copied separately with an external code digest. It does
not belong in or modify the original31-helper tree. The original source/base,
receipts, preparation and file engine remain bound to their original generation.
Only the historical-revocation observer receives an explicitly re-admitted live
main baseline, with truthful API results and an independent ancestry/tree proof.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
from types import SimpleNamespace

HEAD = '063eac64c53b69371ce603f87396f5b2710c8cbb'
MANIFEST = 'eeab28487397d64dcd7690c8692b94fa55aabfbcfc1db097645c8fe21e966438'
RECIPE = 'beac6ca0e956ae9c9a32be0810f6ba53f8717e1097e9d889d9a5d2a4028739a6'
SOURCE_BASE = '8994cf53a44677ecb36acc6f9a9ce1d547c57728'
APPROVED_MAIN = '0d969e03ab2a58591eb39ca67d7dba4d5cf04581'
APPROVED_PARENTS = [SOURCE_BASE, '5d906b0b98089467efd8f6a2fc0189cd03c7e332']
OLD_HEAD = '659803a2f144926997e56d072c7fe8760d6ed176'
OLD_RUN = 'ce86a2a4-ff18-435f-bb5d-aafc313ceca6'
STAGE = Path('/root/gatehouse-issue98-linux-063eac64c53b')
CONTROL = Path('/srv/gatehouse/pilot/control')
ADMISSION = Path('/var/lib/gatehouse/operator-runs/063eac64c53b/readmission.json')
STEPS = ('volume','preservation','source','input','seal','core','configuration','inspection','recipe')
CONTEXT_NAMES = ('need','owned','path_operand','read','write','record','canonical','unique',
                 'git','frozen_lock','load','authenticate','readonly','import_inputs','ENV')
# Informational only: the separately pinned worker supplies a bounded reporter.
CHECKPOINT = lambda label:None


def sha(value):return hashlib.sha256(value).hexdigest()


def batch_blobs(B, source, head, members):
    """Read all exact Git blobs once per admission; no observation is cached.

    The caller has authenticated the payload and checked the protected Git store.
    Compare every byte, object type, size and object ID, including framing/EOF.
    This replaces dozens of Git processes, not any source or admission checks.
    """
    B.need(re.fullmatch('[a-f0-9]{40}',head) and 0 < len(members) <= 128,
           'batch source requires an exact bounded Git inventory')
    for name,raw in members:
        B.need(type(name) is str and re.fullmatch('[A-Za-z0-9_./-]+',name)
               and not name.startswith('/') and '..' not in Path(name).parts
               and type(raw) is bytes and len(raw) <= 1024**2,
               'batch source operand or member exceeds its contract')
    B.owned('/usr/bin/git')
    result = subprocess.run(['/usr/bin/git','--no-replace-objects','-c','core.hooksPath=/dev/null',
        '-c','core.fsmonitor=false','-c','protocol.ext.allow=never','-C',str(source),'cat-file','--batch'],
        input=''.join(head+':'+name+'\n' for name,_ in members).encode('ascii'),env=B.ENV,
        capture_output=True,check=True,timeout=120)
    data = result.stdout
    B.need(not result.stderr and len(data) <= sum(len(raw)+100 for _,raw in members),
           'batch source response exceeds its exact payload budget')
    offset = 0
    for _,raw in members:
        end = data.find(b'\n',offset,offset+100)
        B.need(end >= 0,'batch source header is truncated')
        object_id = hashlib.sha1(b'blob '+str(len(raw)).encode()+b'\0'+raw).hexdigest()
        expected = (object_id+' blob '+str(len(raw))).encode('ascii')
        B.need(data[offset:end] == expected,'batch source object identity, type or size differs')
        offset = end+1
        B.need(data[offset:offset+len(raw)] == raw and data[offset+len(raw):offset+len(raw)+1] == b'\n',
               'proposed installed/helper bytes differ from protected Git source')
        offset += len(raw)+1
    B.need(offset == len(data),'batch source response contains extra objects')


def bind_source(B, contract, payload, source, head, manifest_pin):
    B.need(sha(payload['helper-manifest.json']) == manifest_pin,
           'source binding requires the authenticated helper manifest pin')
    code = contract.json_value(B,payload['helper-manifest.json'])
    contract.validate(B,payload,code,payload['helper-manifest.json'])
    B.need(head == code['revision'],'release source head differs')
    B.frozen_lock(source,head,contract.BASE)
    members = [(name,payload[prefix+name]) for prefix,names in
               (('files/',contract.source_map()),('helpers/',code['files'])) for name in names]
    batch_blobs(B,source,head,members)
    # Recheck the same protected source after the batch as well as before it.
    B.frozen_lock(source,head,contract.BASE)
    return {'head':head,'source_bound':True,'administrator_ready':False,'production_gate_enabled':False}


class Readmission:
    def __init__(self, B, fd):
        self.B, self.fd = B, fd
        self.fence = B.load(STAGE/'helpers','install-transaction.py')
        self.fence.lease(B,fd)
        program = B.owned(Path(__file__),private=True)
        B.need(stat.S_IMODE(program.stat().st_mode) == 0o400,'readmission code must be externally pinned private input')
        self.code_pin = sha(B.read(program,65536))
        self.initial = self.identity()
        if not ADMISSION.exists() and not ADMISSION.is_symlink():
            files = self.value(CONTROL/'file-install.json')
            B.need(files['phase'] == 'installing' and files['inflight'] is None
                   and len(files['directories_done']) == 2 and files['files_done'] == [],
                   'first readmission requires the observed interrupted boundary before any file replacement')
            for name in ('controller-activation.json','mount-persistence.json'):
                path = CONTROL/name
                B.need(not path.exists() and not path.is_symlink(),'first readmission cannot adopt advanced activation')
        else:self.saved()

    def value(self, path):
        B = self.B;raw = B.read(B.owned(path,private=True),1024**2)
        value = json.loads(raw,object_pairs_hook=B.unique)
        B.need(raw == B.canonical(value),'recovery metadata is not canonical')
        return value

    def identity(self):
        B = self.B;self.fence.lease(B,self.fd)
        admin = self.value(CONTROL/'administrator-upgrade.json')
        lifecycle = self.value(CONTROL/'linux-upgrade.json')
        files = self.value(CONTROL/'file-install.json');fenced = self.fence.read_receipt(B)
        for name in ('administrator-upgrade.json','linux-upgrade.json','file-install.json',
                     'entry-fence.json','controller-activation.json','mount-persistence.json'):
            path = CONTROL/('.'+name+'.tmp')
            B.need(not path.exists() and not path.is_symlink(),'retain unfinished recovery metadata')
        B.need(admin['schema'] == 'gatehouse-administrator-dispatch/v1'
               and admin['head'] == lifecycle['head'] == files['head'] == fenced['head'] == HEAD
               and admin['phase'] in ('dispatching','complete') and admin['inflight'] is None
               and [row['step'] for row in admin['completed']] == list(STEPS)
               and admin['helper_manifest_sha256'] == lifecycle['helper_manifest_sha256'] == MANIFEST
               and admin['recipe_sha256'] == RECIPE
               and lifecycle['recipe_sha256'] == admin['completed'][-1]['sha256']
               and lifecycle['phase'] in ('installing','installed-disabled')
               and files['phase'] in ('installing','installed-disabled')
               and lifecycle['transaction'] == files['transaction'] == fenced['transaction']
               and files['fence_sha256'] == lifecycle['entry_fence_sha256'] == sha(B.canonical(fenced))
               and files['material_sha256'] == lifecycle['material_sha256']
               and lifecycle['old_head'] == OLD_HEAD and lifecycle['old_run'] == OLD_RUN,
               'readmission does not match the original completed preparation and installation generation')
        lease = [os.fstat(self.fd).st_dev,os.fstat(self.fd).st_ino]
        B.need(admin['lease'] == lifecycle['lease'] == lease,'readmission lease belongs to another generation')
        return {'schema':'gatehouse-administrator-baseline-readmission/v1','head':HEAD,
                'helper_manifest_sha256':MANIFEST,'administrator_recipe_sha256':RECIPE,
                'administrator_transaction':admin['transaction'],'installation_transaction':lifecycle['transaction'],
                'material_sha256':lifecycle['material_sha256'],'entry_fence_sha256':files['fence_sha256'],
                'lease':lease,'source_base':SOURCE_BASE,'approved_main':APPROVED_MAIN,
                'readmission_code_sha256':self.code_pin,'administrator_ready':False,'production_gate_enabled':False,
                'scope':'finish this existing installation only; source and quality/delivery baseline guards unchanged'}

    def saved(self):
        B = self.B;path = B.owned(ADMISSION,private=True)
        B.need(stat.S_IMODE(path.stat().st_mode) == 0o400,'readmission record must remain immutable')
        raw = B.read(path,65536);value = json.loads(raw,object_pairs_hook=B.unique)
        B.need(raw == B.canonical(value) and set(value) == set(self.initial)|{'ancestry','host_subtrees'}
               and {key:value[key] for key in self.initial} == self.initial
               and self.identity() == self.initial,'readmission identity or original generation changed')
        self.validate_proof(value['ancestry'],value['host_subtrees'])
        return sha(raw)

    def host_subtrees(self, client, commit):
        B = self.B;value = client.repo('GET','/git/commits/'+commit)
        B.need(value['sha'] == commit and re.fullmatch('[a-f0-9]{40}',value['tree']['sha']),
               'readmission commit identity differs')
        def entries(tree):
            value = client.repo('GET','/git/trees/'+tree)
            B.need(value['sha'] == tree and value.get('truncated') is False and len(value['tree']) <= 10000,
                   'readmission tree observation is incomplete')
            rows = {row['path']:row for row in value['tree']}
            B.need(len(rows) == len(value['tree']),'readmission tree paths are duplicated')
            return rows
        root = entries(value['tree']['sha']);tools = root['tools']
        B.need(tools['type'] == 'tree' and re.fullmatch('[a-f0-9]{40}',tools['sha']),
               'readmission tools tree differs')
        rows = entries(tools['sha']);result = {}
        for name in ('harness-gate','trusted_gate'):
            row = rows.get(name)
            if row is None:result[name] = None
            else:
                B.need(row['type'] == 'tree' and re.fullmatch('[a-f0-9]{40}',row['sha']),
                       'readmission host subtree type differs')
                result[name] = row['sha']
        B.need(result['harness-gate'] is not None,'readmission Core subtree is absent')
        return result

    def validate_proof(self, ancestry, subtrees):
        self.B.need(ancestry == {'base':SOURCE_BASE,'head':APPROVED_MAIN,
                                'method':'exact-direct-first-parent','parents':APPROVED_PARENTS}
                    and set(subtrees) == {'source_base','approved_main'}
                    and subtrees['source_base'] == subtrees['approved_main']
                    and set(subtrees['source_base']) == {'harness-gate','trusted_gate'}
                    and re.fullmatch('[a-f0-9]{40}',subtrees['source_base']['harness-gate'])
                    and (subtrees['source_base']['trusted_gate'] is None
                         or re.fullmatch('[a-f0-9]{40}',subtrees['source_base']['trusted_gate'])),
                    'approved main is not a forward baseline with unchanged host subtrees')

    def prove(self, client):
        # The accepted client forbids '..' in every request path. Use the exact
        # reviewed merge's direct first parent, never an encoded compare route
        # or a relaxation of the original path/traversal guard.
        B = self.B;commit = client.repo('GET','/git/commits/'+APPROVED_MAIN)
        B.need(commit['sha'] == APPROVED_MAIN and type(commit['parents']) is list
               and [row['sha'] for row in commit['parents']] == APPROVED_PARENTS,
               'readmission baseline does not have the exact reviewed direct parents')
        ancestry = {'base':SOURCE_BASE,'head':APPROVED_MAIN,
                    'method':'exact-direct-first-parent','parents':list(APPROVED_PARENTS)}
        subtrees = {'source_base':self.host_subtrees(client,SOURCE_BASE),
                    'approved_main':self.host_subtrees(client,APPROVED_MAIN)}
        self.validate_proof(ancestry,subtrees)
        return ancestry,subtrees

    def remote(self, original, B, client, config, run, contract, preflight):
        self.fence.lease(B,self.fd)
        B.need(self.identity() == self.initial and contract.BASE == SOURCE_BASE
               and config['coverage_acceptance'] == 'transition' and run['id'] == OLD_RUN
               and run['head'] == OLD_HEAD and run['local'] == 'invalid' and run['remote'] == 'revoked',
               'administrative readmission is restricted to this retired transition pilot')
        # Only the administrative remote observer sees the new expected baseline.
        # The original observer still checks the real repository/ref/old commit,
        # tree, dedicated App and completed failure Check without altered replies.
        CHECKPOINT('github-repository-and-check')
        result = original(B,client,config,run,SimpleNamespace(BASE=APPROVED_MAIN),preflight)
        B.need(result['base'] == APPROVED_MAIN and result['head'] == OLD_HEAD,
               'readmission remote result is not the truthful approved baseline')
        if not ADMISSION.exists() and not ADMISSION.is_symlink():
            CHECKPOINT('github-direct-parent-and-trees')
            ancestry,subtrees = self.prove(client)
            B.need(self.identity() == self.initial,'generation changed during baseline readmission')
            B.owned(ADMISSION.parent,directory=True,private=True)
            B.write(ADMISSION,B.canonical({**self.initial,'ancestry':ancestry,'host_subtrees':subtrees}),0o400)
            fd = os.open(ADMISSION.parent,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW)
            try:os.fsync(fd)
            finally:os.close(fd)
        CHECKPOINT('immutable-readmission-record')
        return {**result,'administrative_readmission_sha256':self.saved()}

    def load(self, root, name):
        module = self.B.load(root,name)
        if name == 'admit-upgrade-effects.py':
            original = module.remote
            module.remote = lambda *args:self.remote(original,*args)
        elif name == 'bootstrap-release.py':
            module.bind_source = lambda B,*args:bind_source(B,module,*args)
        elif name == 'check-core-policy.py':
            original = module.verify_completed
            def checked(*args,**kwargs):
                CHECKPOINT('retained-core-verification')
                return original(*args,**kwargs)
            module.verify_completed = checked
        return module


def dispatch(B, A):
    """Resume original Context/resume with one original FD, never repeat preparation."""
    B.need(os.geteuid() == 0,'administrator identity required for interrupted installation recovery')
    B.authenticate(STAGE/'helpers',STAGE/'helper-manifest.json',MANIFEST)
    fence = B.load(STAGE/'helpers','install-transaction.py')
    lease = B.owned(fence.LEASE,private=True)
    fd = os.open(lease,os.O_RDWR|os.O_NOFOLLOW|os.O_NONBLOCK)
    try:
        fence.lease(B,fd);readmission = Readmission(B,fd)
        context = SimpleNamespace(**{name:getattr(B,name) for name in CONTEXT_NAMES})
        context.load = readmission.load
        CHECKPOINT('administrator-context')
        ctx = A.Context(context,STAGE/'helpers',STAGE/'helper-manifest.json',MANIFEST,
                        STAGE/'administrator.json',RECIPE,fd)
        value = A.receipt(ctx)
        B.need(value['phase'] in ('dispatching','complete') and value['inflight'] is None
               and [row['step'] for row in value['completed']] == list(STEPS),
               'recovery cannot rebuild or restart incomplete preparation')
        CHECKPOINT('installation-resume')
        return A.resume(ctx)
    finally:os.close(fd)


if __name__ == '__main__':
    raise SystemExit('operator add-on only; use its separately reviewed pinned detached recovery launcher')
