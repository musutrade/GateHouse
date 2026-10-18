#!/usr/bin/python3 -I
"""Stage reviewed operator programs and start one detached native recovery service."""
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys

SOURCE = Path('/mnt/dev-ssd/gatehouse-issue98-ancestor-transport-observation/reviewed-recovery')
OPERATION = Path('/root/gatehouse-issue98-detached-063eac64c53b-r2')
OUTPUT = Path('/var/lib/gatehouse/operator-runs/063eac64c53b-r2')
UNIT = 'gatehouse-issue98-recovery-063eac64c53b-r2.service'
PREVIOUS_UNIT = 'gatehouse-issue98-recovery-063eac64c53b.service'
PREVIOUS_OUTPUT = Path('/var/lib/gatehouse/operator-runs/063eac64c53b')
HEAD = '063eac64c53b69371ce603f87396f5b2710c8cbb'
PINS = {
    'resume.py':('resume-reviewed-linux.py','a75d76caeeb5a7d692b2f89d25c3e6679e43bcbc5c99d5a7672ec04fd07b8429'),
    'worker.py':('detached-linux-worker.py','1b921bc9bfe05ffe09ae307723b6fe4bc33e1849ba19a688ed071ff9527c169d'),
    'recovery.py':('recover-interrupted-installation.py','faf879f3f6474f11bbe8f402597501786931483252523ec1457398fdf8e8a6a6'),
}
ENV = {'PATH':'/usr/bin:/bin','LANG':'C.UTF-8'}


def need(value, message):
    if not value:raise ValueError(message)


def identity(info):
    return (info.st_dev,info.st_ino,info.st_uid,info.st_gid,info.st_mode,info.st_nlink,
            info.st_size,info.st_mtime_ns,info.st_ctime_ns)


def inspect(path, root_only=True, directory=False):
    for parent in (path,*path.parents):
        info = parent.lstat()
        need(info.st_uid in ((0,) if root_only else (0,1000))
             and not stat.S_ISLNK(info.st_mode) and not info.st_mode & 0o022,
             'unsafe reviewed operation path')
    info = path.lstat()
    need(stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode) and info.st_nlink == 1,
         'reviewed operation type differs')
    return info


def read(path, root_only=True):
    inspect(path,root_only)
    with os.fdopen(os.open(path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK),'rb') as stream:
        info = os.fstat(stream.fileno());need(info.st_size <= 65536,'reviewed operation exceeds budget')
        raw = stream.read(65537)
        need(len(raw) == info.st_size and identity(info) == identity(os.fstat(stream.fileno()))
             == identity(path.lstat()),'reviewed operation changed')
    return raw


def sync(path):
    fd = os.open(path,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW)
    try:os.fsync(fd)
    finally:os.close(fd)


def directory(path):
    inspect(path.parent,directory=True)
    if not path.exists() and not path.is_symlink():path.mkdir(mode=0o700)
    info = inspect(path,directory=True);need(stat.S_IMODE(info.st_mode) == 0o700,'private operation directory required')
    sync(path.parent)


def write(path, raw, mode):
    with os.fdopen(os.open(path,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600),'wb') as stream:
        stream.write(raw);stream.flush();os.fchmod(stream.fileno(),mode);os.fsync(stream.fileno())
    sync(path.parent)


def manager(unit=None):
    unit = UNIT if unit is None else unit
    result = subprocess.run(['/usr/bin/systemctl','show',unit,
        '--property=Id,LoadState,ActiveState,SubState,Result,ExecMainStatus'],
        env=ENV,capture_output=True,timeout=10)
    need(len(result.stdout) <= 65536 and not result.stderr,'ambiguous recovery service observation')
    rows = dict(line.partition('=')[::2] for line in result.stdout.decode('ascii').splitlines())
    need(rows.get('Id') == unit and rows.get('LoadState') in ('loaded','not-found'),'unknown recovery unit identity')
    return rows


def previous_failure():
    """An explicit new attempt, retaining the failed service, code and outputs."""
    old = manager(PREVIOUS_UNIT)
    need(all(old.get(k) == v for k,v in {'LoadState':'loaded','ActiveState':'failed','SubState':'failed',
         'Result':'exit-code','ExecMainStatus':'1'}.items()),'previous recovery must be stopped at its known failure')
    path = PREVIOUS_OUTPUT/'result.json'
    need(stat.S_IMODE(inspect(path).st_mode) == 0o600,'previous private result mode differs')
    value = json.loads(read(path))
    need(value.get('head') == HEAD and value.get('status') == 'blocked'
         and value.get('dispatch_attempted') is True and value.get('automatic_retries') == 0
         and value.get('failure',{}).get('type') == 'Blocked'
         and {'file':'<accepted659-github>','function':'request','line':42}
             in value['failure'].get('helper_frames',[]),
         'previous result is not the reviewed request-path rejection')
    for name in ('readmission.json','.readmission.json.tmp'):
        path = PREVIOUS_OUTPUT/name
        need(not path.exists() and not path.is_symlink(),'retain unexpected earlier baseline admission')


def command():
    return ['/usr/bin/systemd-run','--quiet','--unit='+UNIT,'--service-type=exec','--remain-after-exit',
        '--working-directory=/','--expand-environment=no','--setenv=PATH=/usr/bin:/bin','--setenv=LANG=C.UTF-8',
        '--property=UMask=0077','--property=Restart=no','--property=StandardInput=null',
        '--property=StandardOutput=append:'+str(OUTPUT/'stdout.log'),
        '--property=StandardError=append:'+str(OUTPUT/'stderr.log'),
        '/usr/bin/python3.14','-I',str(OPERATION/'worker.py')]


def main():
    if os.geteuid() != 0:raise SystemExit('administrator authentication required to start reviewed recovery')
    os.umask(0o077)
    before = manager()
    if before['LoadState'] != 'not-found':
        print(json.dumps({'action':'existing-unit-no-new-dispatch','unit':UNIT,'manager':before},indent=2));return
    previous_failure()
    programs = {}
    for target,(name,pin) in PINS.items():
        raw = read(SOURCE/name,root_only=False)
        need(hashlib.sha256(raw).hexdigest() == pin,'operator program differs from reviewed pin')
        compile(raw,str(OPERATION/target),'exec');programs[target] = raw
    directory(OPERATION)
    need(set(os.listdir(OPERATION)) <= set(PINS),'retain unknown private operation content')
    for target,raw in programs.items():
        path = OPERATION/target
        if not path.exists() and not path.is_symlink():write(path,raw,0o400)
        need(read(path) == raw and stat.S_IMODE(path.stat().st_mode) == 0o400,'private operation program differs')
    directory(OUTPUT.parent)
    need(not OUTPUT.exists() and not OUTPUT.is_symlink(),'retain earlier recovery output without a known manager unit')
    directory(OUTPUT)
    for name in ('stdout.log','stderr.log'):write(OUTPUT/name,b'',0o600)
    result = subprocess.run(command(),env=ENV,capture_output=True,timeout=30)
    need(result.returncode == 0,'recovery service start was not confirmed; inspect the existing unit before retrying')
    print(json.dumps({'action':'started-detached','unit':UNIT,
        'public_progress':'/run/gatehouse-issue98-upgrade-063eac64c53b.json',
        'private_result':str(OUTPUT/'result.json'),'manager':manager(),
        'administrator_ready':False,'production_gate_enabled':False},indent=2))


if __name__ == '__main__':main()
