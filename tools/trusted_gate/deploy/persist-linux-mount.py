"""Durable mount-unit enablement under activation admission; never start a unit."""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess

UNIT = 'srv-gatehouse-linux.mount'
FILE = Path('/etc/systemd/system') / UNIT
LINK = FILE.parent / 'multi-user.target.wants' / UNIT
RECEIPT = Path('/srv/gatehouse/pilot/control/mount-persistence.json')
PHASES = ('reload-intent', 'reloaded', 'enable-intent', 'enabled')
KEYS = {'Id','LoadState','ActiveState','SubState','UnitFileState','FragmentPath','DropInPaths','NeedDaemonReload'}


def digest(raw):return hashlib.sha256(raw).hexdigest()


def same(B,a,b,message):B.need(B.canonical(a)==B.canonical(b),message)


def present(path):return path.exists() or path.is_symlink()
def root_owned(info):return info.st_uid==0


def link(B):
    if not present(LINK):return None
    B.owned(LINK.parent,directory=True)
    before=LINK.lstat()
    B.need(stat.S_ISLNK(before.st_mode) and root_owned(before) and before.st_nlink==1
           and os.readlink(LINK)==str(FILE), 'mount enablement link differs')
    fields=lambda info:(info.st_dev,info.st_ino,info.st_mode,info.st_uid,info.st_gid,info.st_nlink,info.st_mtime_ns,info.st_ctime_ns)
    B.need(fields(before)==fields(LINK.lstat()),'mount enablement link changed')
    return fields(before)


def disk(ctx, *, allow_link):
    B=ctx.B;row=ctx.material['files'][str(FILE)]
    B.need(row['mode']==0o644 and row['action']=='replace' and row['before'] is None,
           'mount unit is not the new reviewed file')
    unit=B.owned(FILE)
    B.need(stat.S_IMODE(unit.stat().st_mode)==0o644 and B.read(unit,4096)==row['after'], 'installed mount unit changed')
    count=0
    for root in ctx.inputs.UNIT_ROOTS:
        if not present(root):continue
        B.owned(root,directory=True)
        for name in ('mount.d','srv-.mount.d','srv-gatehouse-.mount.d',UNIT+'.d'):
            B.need(not present(root/name), 'mount drop-in appeared')
        if root!=FILE.parent:B.need(not present(root/UNIT),'mount unit override or alias appeared')
        for child in root.iterdir():
            count+=1;B.need(count<=20000,'unit enablement inventory exceeds budget')
            if child.name.endswith(('.wants','.requires')):
                B.owned(child,directory=True)
                candidate=child/UNIT
                B.need(candidate==LINK or not present(candidate),'unknown mount enablement slot')
    linked=link(B)
    B.need(allow_link or linked is None,'mount already enabled without its durable intent')
    volume=B.load(ctx.helpers,'prepare-linux-volume.py')
    record=volume.admit(B,ctx.helpers,ctx.manifest,ctx.manifest_pin,
                       ctx.value['paths']['volume_recipe'],ctx.value['pins']['volume_recipe'])
    raw=B.read(volume.MARKER,64*1024)
    same(B,record,json.loads(raw,object_pairs_hook=B.unique),'registered mounted storage changed')
    return {'unit_sha256':digest(row['after']),'storage_sha256':digest(raw),'link':linked}


def command(ctx,*args):
    result=subprocess.run(['/usr/bin/systemctl',*args],env=ctx.B.ENV,capture_output=True,timeout=20,check=True)
    ctx.B.need(len(result.stdout)<=64*1024 and len(result.stderr)<=64*1024,'mount manager response exceeds budget')
    return result


def manager(ctx, *, enabled):
    result=command(ctx,'show',UNIT,'--property='+','.join(sorted(KEYS)))
    ctx.B.need(not result.stderr,'mount manager properties are ambiguous')
    row={}
    for line in result.stdout.decode('ascii').splitlines():
        key,separator,value=line.partition('=')
        ctx.B.need(separator and key in KEYS and key not in row,'mount manager properties differ');row[key]=value
    same(ctx.B,row,{'Id':UNIT,'LoadState':'loaded','ActiveState':'active','SubState':'mounted',
         'UnitFileState':'enabled' if enabled else 'disabled','FragmentPath':str(FILE),
         'DropInPaths':'','NeedDaemonReload':'no'},'mount manager did not adopt the reviewed mounted unit')
    return row


def fixed(ctx,value,facts):
    return {'schema':'gatehouse-mount-persistence/v1',
            **{name:value[name] for name in ('head','transaction','helper_manifest_sha256','recipe_sha256',
                'material_sha256','lease','upgrade_receipt_sha256','file_receipt_sha256','entry_fence_sha256')},
            'unit_sha256':facts['unit_sha256'],'storage_sha256':facts['storage_sha256'],
            'administrator_ready':False,'production_gate_enabled':False}


def receipt(ctx,identity):
    value=ctx.files.metadata(ctx.B,RECEIPT)
    ctx.B.need(set(value)==set(identity)|{'phase'} and value['phase'] in PHASES,'unknown mount persistence phase')
    same(ctx.B,{name:value[name] for name in identity},identity,'mount persistence belongs to another generation')
    return value


def sync(ctx):
    for path in (LINK.parent,FILE.parent):
        ctx.B.owned(path,directory=True)
        fd=os.open(path,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW)
        try:os.fsync(fd)
        finally:os.close(fd)


def observe(ctx,value):
    facts=disk(ctx,allow_link=True);identity=fixed(ctx,value,facts);state=receipt(ctx,identity)
    ctx.B.need(state['phase']=='enabled' and facts['link'] is not None,'mount persistence is incomplete')
    manager(ctx,enabled=True);return state


def persist(ctx,value,guard):
    """Caller supplies the actual full activation live guard on the same lease."""
    B=ctx.B;B.need(os.geteuid()==0 and type(ctx.fd) is int,'administrator and original lease required')
    ctx.fence.lease(B,ctx.fd);guard()
    exists=present(RECEIPT)
    B.need(not present(RECEIPT.with_name('.'+RECEIPT.name+'.tmp')),'retain partial mount persistence metadata')
    if not exists:
        B.need(value['phase']=='prepared','mount persistence cannot begin after activation advanced')
        facts=disk(ctx,allow_link=False);identity=fixed(ctx,value,facts)
        B.record(RECEIPT,{**identity,'phase':'reload-intent'})
    # Before an enable intent, an existing link is never silently adopted.
    preliminary=ctx.files.metadata(B,RECEIPT)
    facts=disk(ctx,allow_link=preliminary.get('phase') in ('enable-intent','enabled'))
    identity=fixed(ctx,value,facts);state=receipt(ctx,identity)
    def check(*,enabled=False,allow_link=False):
        ctx.fence.lease(B,ctx.fd);guard()
        current=disk(ctx,allow_link=allow_link)
        same(B,fixed(ctx,value,current),identity,'mount persistence inputs changed')
        same(B,receipt(ctx,identity),state,'mount persistence progress changed')
        if enabled:B.need(current['link'] is not None,'mount persistence link disappeared')
    def advance(phase):
        nonlocal state
        B.need(PHASES.index(phase)==PHASES.index(state['phase'])+1,'mount persistence must advance once')
        B.record(RECEIPT,{**identity,'phase':phase});state=receipt(ctx,identity)
    if state['phase']=='reload-intent':
        check();command(ctx,'daemon-reload');check();manager(ctx,enabled=False);advance('reloaded')
    if state['phase']=='reloaded':
        check();manager(ctx,enabled=False);advance('enable-intent')
    if state['phase']=='enable-intent':
        check(allow_link=True)
        if link(B) is None:
            manager(ctx,enabled=False)
            command(ctx,'--no-reload','enable',UNIT)
        check(enabled=True,allow_link=True);sync(ctx);manager(ctx,enabled=True);advance('enabled')
    check(enabled=True,allow_link=True);return observe(ctx,value)


if __name__=='__main__':raise SystemExit('mount persistence is internal to authenticated activation; no standalone entry')
