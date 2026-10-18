#!/usr/bin/python3 -I
"""Copy externally pinned administrator inputs into a private Root stage only."""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys

PARENT = Path('/root')
MAX_FILE = 256 * 1024**2
MAX_TOTAL = 1024**3
MAX_FILES = 4096
MAX_METADATA = 2 * 1024**2
META = 'transport.json'
COMPLETE = 'stage-complete.json'
FIXED = {'helper-manifest.json', 'bootstrap-source.tar', 'host-recipe.json', 'volume-tools.json', 'administrator.json'}
ROLES = {'helpers', 'tool-archives', 'crate-archives', 'test-archives', 'onboarding'}


def need(value, message):
    if not value:raise ValueError(message)


def canonical(value):return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()+b'\n'
def digest(raw):return hashlib.sha256(raw).hexdigest()
def root_owned(info):return info.st_uid==0


def unique(pairs):
    value = {}
    for key, item in pairs:
        need(key not in value, 'duplicate staging field');value[key] = item
    return value


def relative(name):
    need(type(name) is str and len(name) <= 1024 and re.fullmatch('[A-Za-z0-9_./+@=-]+', name)
         and all(part not in ('', '.', '..') for part in name.split('/')), 'unsafe staging member')
    return Path(name)


def identity(info):
    return (info.st_dev, info.st_ino, info.st_uid, info.st_gid, info.st_mode,
            info.st_nlink, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


@contextmanager
def directory(path, *, protected=False):
    """Open every ancestor without following a link; keep the terminal fd pinned."""
    path = Path(path)
    need(path.is_absolute() and '..' not in path.parts and str(path) != '/', 'absolute staging directory required')
    fd = os.open('/', os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        for name in path.parts[1:]:
            child = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=fd)
            os.close(fd);fd = child
            info = os.fstat(fd)
            need(not protected or root_owned(info) and not info.st_mode & 0o022, 'unprotected staging ancestor')
        yield fd
    finally:os.close(fd)


@contextmanager
def member(root_fd, name):
    path = relative(name);fd = os.dup(root_fd)
    try:
        for part in path.parts[:-1]:
            child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=fd)
            os.close(fd);fd = child
        leaf = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=fd)
        try:
            info = os.fstat(leaf)
            need(stat.S_ISREG(info.st_mode) and info.st_nlink == 1 and not info.st_mode & 0o7000
                 and info.st_size <= MAX_FILE, 'linked, special, privileged or oversized staging input')
            yield leaf, info
            need(identity(info) == identity(os.fstat(leaf)) == identity(os.stat(path.name, dir_fd=fd, follow_symlinks=False)),
                 'staging input changed while reading')
        finally:os.close(leaf)
    finally:os.close(fd)


def stream(root_fd, name, expected=None, output=None, *, limit=MAX_FILE):
    with member(root_fd, name) as (fd, info):
        need(info.st_size <= limit, 'staging file exceeds budget')
        sha = hashlib.sha256();count = 0
        while data := os.read(fd, 1024**2):
            count += len(data);need(count <= info.st_size, 'growing staging file')
            sha.update(data)
            if output is not None:output.write(data)
        need(count == info.st_size, 'short staging file')
        row = [count, sha.hexdigest(), 0o500 if info.st_mode & 0o111 else 0o400]
        need(expected is None or row[:2] == expected[:2], 'staging input digest differs')
        return row


def read(root_fd, name, limit):
    with member(root_fd, name) as (fd, info):
        need(info.st_size <= limit, 'staging metadata exceeds budget')
        chunks=[];count=0
        while chunk := os.read(fd, min(1024**2,limit+1-count)):
            count+=len(chunk);need(count<=limit,'growing staging metadata');chunks.append(chunk)
        need(count==info.st_size,'short staging metadata');return b''.join(chunks)


def names(root_fd, *, protected=False):
    files, dirs = set(), set()
    def visit(fd, prefix):
        before=os.fstat(fd)
        need(not protected or root_owned(before) and stat.S_IMODE(before.st_mode)==0o700,
             'protected staging directory mode differs')
        for name in sorted(os.listdir(fd)):
            key=prefix+name;relative(key)
            info=os.stat(name,dir_fd=fd,follow_symlinks=False)
            if stat.S_ISDIR(info.st_mode):
                dirs.add(key);need(len(files)+len(dirs)<=MAX_FILES,'staging member budget exceeded')
                child=os.open(name,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW|os.O_NONBLOCK,dir_fd=fd)
                try:
                    need(identity(info)==identity(os.fstat(child)),'staging directory changed')
                    visit(child,key+'/')
                finally:os.close(child)
            else:
                need(stat.S_ISREG(info.st_mode) and info.st_nlink==1 and not info.st_mode&0o7000,
                     'unexpected staging member type');files.add(key)
                need(not protected or root_owned(info) and stat.S_IMODE(info.st_mode) in (0o400,0o500),
                     'protected staging member mode differs')
                need(len(files)+len(dirs)<=MAX_FILES,'staging member budget exceeded')
        need(identity(before)==identity(os.fstat(fd)),'staging directory changed during inventory')
    visit(root_fd,'');return files,dirs


def manifest(raw, pin):
    need(type(pin) is str and re.fullmatch('[0-9a-f]{64}',pin) and digest(raw)==pin,'transport manifest pin differs')
    value=json.loads(raw,object_pairs_hook=unique)
    need(raw==canonical(value) and set(value)=={'schema','head','helper_manifest_sha256','files'}
         and value['schema']=='gatehouse-administrator-transport/v1'
         and type(value['head']) is str and re.fullmatch('[0-9a-f]{40}',value['head'])
         and type(value['helper_manifest_sha256']) is str and re.fullmatch('[0-9a-f]{64}',value['helper_manifest_sha256'])
         and type(value['files']) is dict and FIXED<=value['files'].keys()
         and len(value['files'])<=MAX_FILES, 'transport manifest identity differs')
    total=0;roles=set()
    for name,row in value['files'].items():
        path=relative(name)
        need(name in FIXED or len(path.parts)>1 and path.parts[0] in ROLES,'unexpected transport role')
        if len(path.parts)>1:roles.add(path.parts[0])
        need(type(row) is list and len(row)==3 and type(row[0]) is int and 0<=row[0]<=MAX_FILE
             and type(row[1]) is str and re.fullmatch('[0-9a-f]{64}',row[1])
             and type(row[2]) is int and row[2] in (0o400,0o500)
             and (row[2]==0o400 or path.parts[0]=='onboarding'), 'invalid transport file record')
        total+=row[0];need(total<=MAX_TOTAL,'transport exceeds byte budget')
    need(roles==ROLES,'incomplete transport roles')
    need(value['files']['helper-manifest.json'][1]==value['helper_manifest_sha256'],'helper manifest identity differs')
    need(not any(str(parent) in value['files'] for name in value['files'] for parent in Path(name).parents if str(parent)!='.'),
         'transport file/directory collision')
    return value


def layout(value):
    return {str(p) for name in value['files'] for p in Path(name).parents if str(p)!='.'}


def bindings(fd,value):
    raw=read(fd,'helper-manifest.json',64*1024);code=json.loads(raw,object_pairs_hook=unique)
    need(raw==canonical(code) and set(code)=={'schema','revision','files'}
         and code['schema']=='gatehouse-bootstrap-code/v1' and code['revision']==value['head']
         and type(code['files']) is dict, 'staged helper identity differs')
    helpers={name[8:]:row[1] for name,row in value['files'].items() if name.startswith('helpers/')}
    need(code['files']==helpers and {'tools/trusted_gate/deploy/import-protected-inputs.py',
         'tools/trusted_gate/deploy/stage-administrator-inputs.py'}<=helpers.keys(), 'staged helper inventory differs')
    raw=read(fd,'administrator.json',64*1024);recipe=json.loads(raw,object_pairs_hook=unique)
    destination=PARENT/('gatehouse-issue98-linux-'+value['head'][:12])
    expected={'release':'bootstrap-source.tar','host_recipe':'host-recipe.json','tool_recipe':'volume-tools.json',
              'tool_archives':'tool-archives','crate_archives':'crate-archives','test_archives':'test-archives','onboarding':'onboarding'}
    need(raw==canonical(recipe) and set(recipe)=={'schema','head','helper_manifest_sha256','paths','pins','api_proxy'}
         and recipe['schema']=='gatehouse-administrator-upgrade/v1' and recipe['head']==value['head']
         and recipe['helper_manifest_sha256']==value['helper_manifest_sha256']
         and recipe['paths']=={key:str(destination/name) for key,name in expected.items()}
         and recipe['pins']=={key:value['files'][expected[name]][1] for key,name in
                             (('release','release'),('host_recipe','host_recipe'),('tool_recipe','tool_recipe'))},
         'staged administrator recipe differs')


def verify(root_fd, value, raw, *, protected=False):
    expected=set(value['files'])|{META}|({COMPLETE} if protected else set())
    need(names(root_fd,protected=protected)==(expected,layout(value)),'transport inventory differs')
    need(read(root_fd,META,MAX_METADATA)==raw,'transport manifest changed')
    if protected:
        for name in (META,COMPLETE):
            with member(root_fd,name) as (_,info):
                need(stat.S_IMODE(info.st_mode)==0o400,'protected staging metadata mode differs')
    for name,row in value['files'].items():
        stream(root_fd,name,row)
        if protected:
            with member(root_fd,name) as (_,info):
                need(root_owned(info) and stat.S_IMODE(info.st_mode)==row[2],'protected stage file mode differs')
    bindings(root_fd,value)
    return True


def sync(path):
    fd=os.open(path,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW)
    try:os.fsync(fd)
    finally:os.close(fd)


def write(path,raw,mode=0o400):
    with os.fdopen(os.open(path,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600),'wb') as out:
        out.write(raw);out.flush();os.fchmod(out.fileno(),mode);os.fsync(out.fileno())


def stage(source,pin):
    need(os.geteuid()==0 and sys.platform=='linux','native administrator authentication required')
    os.umask(0o077)
    with directory(source) as source_fd:
        raw=read(source_fd,META,MAX_METADATA);value=manifest(raw,pin)
        destination=PARENT/('gatehouse-issue98-linux-'+value['head'][:12]);temporary=destination.with_name('.'+destination.name+'.tmp')
        with directory(PARENT,protected=True):pass
        need(not temporary.exists() and not temporary.is_symlink(),'retain unfinished protected stage')
        if destination.exists() or destination.is_symlink():
            with directory(destination,protected=True) as fd:
                verify(fd,value,raw,protected=True)
                need(read(fd,COMPLETE,MAX_METADATA)==canonical(completion(value,pin)),'protected stage completion differs')
            return completion(value,pin)
        verify(source_fd,value,raw)
        temporary.mkdir(mode=0o700);sync(PARENT)
        for name in sorted(layout(value),key=lambda n:(len(Path(n).parts),n)):(temporary/name).mkdir(mode=0o700)
        for name,row in sorted(value['files'].items()):
            with os.fdopen(os.open(temporary/name,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600),'wb') as out:
                stream(source_fd,name,row,out);out.flush();os.fchmod(out.fileno(),row[2]);os.fsync(out.fileno())
        write(temporary/META,raw);write(temporary/COMPLETE,canonical(completion(value,pin)))
        for name in sorted(layout(value),key=lambda n:len(Path(n).parts),reverse=True):sync(temporary/name)
        sync(temporary)
        with directory(temporary,protected=True) as fd:verify(fd,value,raw,protected=True)
        verify(source_fd,value,raw)
        need(not destination.exists() and not destination.is_symlink(),'protected stage appeared during copy')
        os.rename(temporary,destination);sync(PARENT)
        return completion(value,pin)


def completion(value,pin):
    return {'schema':'gatehouse-administrator-stage/v1','head':value['head'],'transport_sha256':pin,
            'helper_manifest_sha256':value['helper_manifest_sha256'],'administrator_ready':False,'production_gate_enabled':False}


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--source',type=Path,required=True)
    parser.add_argument('--transport-sha256',required=True);args=parser.parse_args()
    try:result=stage(args.source,args.transport_sha256)
    except Exception as error:
        print('administrator staging blocked ('+type(error).__name__+'); retain private partial stage',file=sys.stderr);return 1
    print(json.dumps(result,sort_keys=True));return 0


if __name__=='__main__':raise SystemExit(main())
