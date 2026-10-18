#!/usr/bin/python3 -I
"""Export pinned administrator transport data; never install or execute Root work."""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import textwrap
from types import SimpleNamespace
import urllib.request


def load(name):
    spec=importlib.util.spec_from_file_location('stage_package_'+name.replace('-','_'),Path(__file__).with_name(name))
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module


S=load('stage-administrator-inputs.py')


def launcher(source,head,pin,stager_pin):
    """The reviewed literal bootstrap copies one bounded program before execution."""
    body=textwrap.dedent('''\
        import hashlib, os, stat
        from pathlib import Path
        def identity(info):
            return (info.st_dev,info.st_ino,info.st_uid,info.st_gid,info.st_mode,info.st_nlink,info.st_size,info.st_mtime_ns,info.st_ctime_ns)
        if os.geteuid()!=0:raise ValueError('administrator authentication required')
        os.umask(0o077)
        source=Path(SOURCE_LITERAL)
        parent=Path('/root')
        info=parent.lstat()
        if not stat.S_ISDIR(info.st_mode) or info.st_uid!=0 or stat.S_IMODE(info.st_mode)!=0o700:
            raise ValueError('private Root loader parent required')
        loader=parent/LOADER_LITERAL
        if not loader.exists() and not loader.is_symlink():
            loader.mkdir(mode=0o700)
        info=loader.lstat()
        if not stat.S_ISDIR(info.st_mode) or info.st_uid!=0 or stat.S_IMODE(info.st_mode)!=0o700:
            raise ValueError('protected loader differs')
        target=loader/'stage.py'
        if not target.exists() and not target.is_symlink():
            source_fd=os.open(source/'helpers/tools/trusted_gate/deploy/stage-administrator-inputs.py',os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK)
            with os.fdopen(source_fd,'rb') as stream:
                before=os.fstat(stream.fileno())
                if not stat.S_ISREG(before.st_mode) or before.st_nlink!=1 or before.st_mode&0o7000 or before.st_size>1024*1024:
                    raise ValueError('loader input type or budget differs')
                raw=stream.read(1024*1024+1)
                if len(raw)!=before.st_size or identity(before)!=identity(os.fstat(stream.fileno())) or hashlib.sha256(raw).hexdigest()!=STAGER_PIN_LITERAL:
                    raise ValueError('loader bytes differ from review')
            with os.fdopen(os.open(target,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600),'wb') as stream:
                stream.write(raw);stream.flush();os.fchmod(stream.fileno(),0o400);os.fsync(stream.fileno())
        if set(os.listdir(loader))!={'stage.py'}:raise ValueError('retain unknown loader contents')
        with os.fdopen(os.open(target,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK),'rb') as stream:
            before=os.fstat(stream.fileno())
            if not stat.S_ISREG(before.st_mode) or before.st_uid!=0 or before.st_nlink!=1 or stat.S_IMODE(before.st_mode)!=0o400 or before.st_size>1024*1024:
                raise ValueError('protected loader file differs')
            raw=stream.read(1024*1024+1)
            if len(raw)!=before.st_size or identity(before)!=identity(os.fstat(stream.fileno())) or hashlib.sha256(raw).hexdigest()!=STAGER_PIN_LITERAL:
                raise ValueError('protected loader bytes differ')
            os.fsync(stream.fileno())
        for directory in (loader,parent):
            fd=os.open(directory,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW)
            try:os.fsync(fd)
            finally:os.close(fd)
        os.execve('/usr/bin/python3.14',['/usr/bin/python3.14','-I',str(target),'--source',str(source),'--transport-sha256',TRANSPORT_PIN_LITERAL],{'PATH':'/usr/bin:/bin','LANG':'C.UTF-8'})
    ''')
    values={'SOURCE_LITERAL':str(source),'LOADER_LITERAL':'gatehouse-issue98-stage-loader-'+head[:12],
            'STAGER_PIN_LITERAL':stager_pin,'TRANSPORT_PIN_LITERAL':pin}
    for key,value in values.items():body=body.replace(key,repr(value))
    compile(body,'reviewed-stage-loader','exec')
    return ('#!/bin/sh\nset -eu\numask 077\n# Review these pinned literals before administrator execution; stages inputs only.\n'
            "/usr/bin/sudo /usr/bin/python3.14 -I - <<'GATEHOUSE_REVIEWED_STAGE'\n"+body+'GATEHOUSE_REVIEWED_STAGE\n').encode()


def plain_read(path,limit):
    path=Path(path)
    with S.directory(path.parent) as fd:return S.read(fd,path.name,limit)


def copy(source,destination,expected=None,mode=0o400):
    source=Path(source)
    with S.directory(source.parent) as fd:
        with os.fdopen(os.open(destination,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600),'wb') as out:
            row=S.stream(fd,source.name,expected,out);out.flush();os.fchmod(out.fileno(),mode);os.fsync(out.fileno())
    return [row[0],row[1],mode]


def host_recipes(root,host,tools):
    B=load('import-protected-inputs.py');builder=load('build-protected-core.py');volume=load('prepare-linux-volume.py')
    reader=SimpleNamespace(need=B.need,path_operand=B.path_operand,read=plain_read,unique=B.unique,canonical=B.canonical)
    host_raw=plain_read(host,1024**2);tool_raw=plain_read(tools,64*1024)
    builder.host_inputs(reader,host,S.digest(host_raw));volume.tools(reader,builder,tools,S.digest(tool_raw))
    return host_raw,tool_raw


def package(root,output,host_recipe,tool_recipe,tool_archives,crate_archives,test_archives,onboarding,api_proxy=None):
    S.need(os.geteuid()!=0,'administrator transport export requires an unprivileged operator')
    B=load('import-protected-inputs.py');R=load('bootstrap-release.py');release=load('package-bootstrap-release.py')
    crates=load('verify-locked-crates.py');tools=load('acquire-linux-tools.py');stage=load('stage-linux-inputs.py')
    effects=load('admit-upgrade-effects.py')
    root=B.path_operand(root);output=B.path_operand(output)
    def git(*args):return subprocess.check_output(['/usr/bin/git','-C',str(root),*args],env=B.ENV,stderr=subprocess.PIPE)
    S.need(not git('status','--porcelain'),'commit reviewed administrator source before exporting')
    head=git('rev-parse','HEAD').decode().strip()
    for path in (host_recipe,tool_recipe,tool_archives,crate_archives,test_archives,onboarding):
        path=B.path_operand(path)
        S.need(path!=output and output not in path.parents and path not in output.parents,'package overlaps input')
    host_raw,tool_raw=host_recipes(root,host_recipe,tool_recipe)
    effects.transport(B,SimpleNamespace(opener=urllib.request.build_opener(urllib.request.ProxyHandler({}))),api_proxy)
    with S.directory(output.parent):pass
    S.need(not output.exists() and not output.is_symlink(),'retain previous administrator package')
    output.mkdir(mode=0o700)
    exported=release.package(root,output/'source-export')
    raw_manifest=plain_read(output/'source-export/helper-manifest.json',64*1024)
    code=json.loads(raw_manifest,object_pairs_hook=B.unique)
    with (output/'source-export/bootstrap-source.tar').open('rb') as stream:
        payload,_=R.decode(B,stream,exported['bytes'],exported['sha256'],code,raw_manifest)
    destination=output/'inputs';destination.mkdir(mode=0o700);rows={}
    def put(name,raw,mode=0o400):
        path=destination/S.relative(name);path.parent.mkdir(parents=True,exist_ok=True,mode=0o700)
        S.write(path,raw,mode);rows[name]=[len(raw),S.digest(raw),mode]
    def take(name,source,expected=None,mode=0o400):
        path=destination/S.relative(name);path.parent.mkdir(parents=True,exist_ok=True,mode=0o700)
        rows[name]=copy(source,path,expected,mode)
    for name in sorted(B.FILES):put('helpers/'+name,payload['helpers/'+name])
    put('helper-manifest.json',raw_manifest)
    take('bootstrap-source.tar',output/'source-export/bootstrap-source.tar',[exported['bytes'],exported['sha256']])
    put('host-recipe.json',host_raw);put('volume-tools.json',tool_raw)
    _,tool_rows=tools.lock(root/'tools/trusted_gate/deploy/linux-tools.lock.json',S.digest(payload['helpers/tools/trusted_gate/deploy/linux-tools.lock.json']))
    for row in tool_rows.values():
        name=row['url'].rsplit('/',1)[1];source=Path(tool_archives)/name
        with S.directory(source.parent) as fd:found=S.stream(fd,name)
        S.need(found[0]<=row['max_bytes'] and found[1]==row['sha256'],'official archive differs from source lock')
        take('tool-archives/'+name,source,found)
    lock=git('show',head+':tools/harness-gate/Cargo.lock')
    packages,_=crates.locked_packages(lock,S.digest(lock))
    for name,pin in sorted(packages.items()):
        source=Path(crate_archives)/(name+'.crate')
        with S.directory(source.parent) as fd:found=S.stream(fd,source.name)
        S.need(found[1]==pin,'crate archive differs from source lock');take('crate-archives/'+source.name,source,found)
    tests=json.loads(git('show',head+':test-data/manifest.json'),object_pairs_hook=B.unique)
    S.need(tests['schema']=='external-test-data/v1','source test archive schema differs')
    for row in tests['archives']:
        name=row['archive_sha256']+'.tar.gz';take('test-archives/'+name,Path(test_archives)/name,[row['archive_size'],row['archive_sha256']])
    S.need(stage.JOB.directory_digest(onboarding)==stage.JOB.ONBOARDING_SHA256,'onboarding source differs')
    with S.directory(onboarding) as fd:
        files,_=S.names(fd)
        for name in sorted(files):
            found=S.stream(fd,name);take('onboarding/'+name,Path(onboarding)/name,found,found[2])
    S.need(stage.JOB.directory_digest(destination/'onboarding')==stage.JOB.ONBOARDING_SHA256,'copied onboarding differs')
    protected=S.PARENT/('gatehouse-issue98-linux-'+head[:12])
    value={'schema':'gatehouse-administrator-upgrade/v1','head':head,'helper_manifest_sha256':exported['helper_manifest_sha256'],
           'paths':{key:str(protected/name) for key,name in {
               'release':'bootstrap-source.tar','host_recipe':'host-recipe.json','tool_recipe':'volume-tools.json',
               'tool_archives':'tool-archives','crate_archives':'crate-archives','test_archives':'test-archives','onboarding':'onboarding'}.items()},
           'pins':{'release':exported['sha256'],'host_recipe':S.digest(host_raw),'tool_recipe':S.digest(tool_raw)},'api_proxy':api_proxy}
    put('administrator.json',B.canonical(value))
    transport={'schema':'gatehouse-administrator-transport/v1','head':head,
               'helper_manifest_sha256':exported['helper_manifest_sha256'],'files':rows}
    raw=B.canonical(transport);S.need(len(raw)<=S.MAX_METADATA,'transport manifest exceeds budget')
    pin=S.digest(raw);S.manifest(raw,pin);S.write(destination/S.META,raw)
    with S.directory(destination) as fd:S.verify(fd,transport,raw)
    S.need(head==git('rev-parse','HEAD').decode().strip() and not git('status','--porcelain'),'source changed during export')
    result={'head':head,'transport_sha256':pin,'helper_manifest_sha256':exported['helper_manifest_sha256'],
            'stager_sha256':rows['helpers/tools/trusted_gate/deploy/stage-administrator-inputs.py'][1],
            'recipe_sha256':rows['administrator.json'][1],'files':len(rows),'bytes':sum(row[0] for row in rows.values()),
            'source_release':exported,'protected_destination':str(protected),'administrator_ready':False,
            'production_gate_enabled':False,'scope':'reviewable transport only; protected stage and live migration acceptance pending'}
    entry=launcher(destination,head,pin,result['stager_sha256']);S.write(output/'stage-reviewed-inputs.sh',entry,0o500)
    result['launcher_sha256']=S.digest(entry)
    S.write(output/'package.json',B.canonical(result));return result


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ('output','host-recipe','tool-recipe','tool-archives','crate-archives','test-archives','onboarding'):
        parser.add_argument('--'+name,type=Path,required=True)
    parser.add_argument('--api-proxy');args=parser.parse_args()
    try:result=package(Path(__file__).resolve().parents[3],**vars(args))
    except Exception as error:
        print('administrator export blocked ('+type(error).__name__+'); retain partial package');return 1
    print(json.dumps(result,indent=2,sort_keys=True));return 0


if __name__=='__main__':raise SystemExit(main())
