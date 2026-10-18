#!/usr/bin/python3 -I
"""Observe and persist one original pinned migration attempt, outside the login session."""
import contextlib
import hashlib
import io
import json
import os
from pathlib import Path
import stat
import sys
import threading
import time
from types import ModuleType

HEAD = '063eac64c53b69371ce603f87396f5b2710c8cbb'
RECOVERY_PIN = 'e00d7678dcbb603d647d54f2ae96a76a5c126ceafdca38a04edf3c31863ec75c'
RESUME_PIN = '6f441f33be9668024a327237657f4fc91a508c8cffaa979c7660d6e9fbeef09d'
OPERATION = Path('/root/gatehouse-issue98-detached-063eac64c53b')
OUTPUT = Path('/var/lib/gatehouse/operator-runs/063eac64c53b')
PUBLIC = Path('/run/gatehouse-issue98-upgrade-063eac64c53b.json')
CONTROL = Path('/srv/gatehouse/pilot/control')
UNIT = 'gatehouse-issue98-recovery-063eac64c53b.service'
RECORDS = {'preparation':'administrator-upgrade.json','lifecycle':'linux-upgrade.json',
           'files':'file-install.json','activation':'controller-activation.json',
           'mount':'mount-persistence.json'}
PHASES = {'prepared','preparing','dispatching','complete','fenced','revoking','revoked','copying','copied',
          'retiring','retired','installing','installed-disabled','intent','marker-intent','marker-active',
          'internal-intent','internal-enabled','public-intent','entries-enabled','reload-intent',
          'reloaded','enable-intent','enabled'}
STEPS = {'volume','preservation','source','input','seal','core','configuration','inspection','recipe'}


def need(value, message):
    if not value:raise ValueError(message)


def protected(path, directory=False):
    for part in (path,*path.parents):
        info = part.lstat()
        need(info.st_uid == 0 and not stat.S_ISLNK(info.st_mode) and not info.st_mode & 0o022,
             'unprotected operation path')
    info = path.lstat()
    need(stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode) and info.st_nlink == 1,
         'operation path type differs')
    return info


def read(path, limit):
    protected(path)
    with os.fdopen(os.open(path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK),'rb') as stream:
        before = os.fstat(stream.fileno())
        need(before.st_size <= limit,'operation input exceeds budget')
        raw = stream.read(limit+1)
        need(len(raw) == before.st_size,'operation input grew or shortened')
    return raw


def atomic(path, value, mode):
    protected(path.parent,directory=True)
    if path.exists() or path.is_symlink():
        info = protected(path);need(stat.S_IMODE(info.st_mode) == mode,'operation output mode differs')
    raw = json.dumps(value,sort_keys=True,separators=(',',':'),allow_nan=False).encode()+b'\n'
    need(len(raw) <= 65536,'operation output exceeds budget')
    temporary = path.with_name('.'+path.name+'.'+str(os.getpid())+'.tmp')
    with os.fdopen(os.open(temporary,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600),'wb') as stream:
        stream.write(raw);stream.flush();os.fchmod(stream.fileno(),mode);os.fsync(stream.fileno())
    os.replace(temporary,path)
    fd = os.open(path.parent,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW)
    try:os.fsync(fd)
    finally:os.close(fd)


def snapshot():
    result = {}
    for role,name in RECORDS.items():
        path = CONTROL/name
        if not path.exists() and not path.is_symlink():continue
        try:
            value = json.loads(read(path,1024**2))
            need(value.get('head') == HEAD,'progress belongs to another head')
            phase = value.get('phase');need(phase in PHASES,'unknown progress phase')
            row = {'phase':phase}
            if role == 'preparation':
                pending = value.get('inflight');need(pending is None or pending in STEPS,'unknown preparation step')
                row.update(inflight=pending,completed=len(value['completed']),total=9)
            if role == 'files':
                row.update(completed_files=len(value['files_done']),total_files=28)
                pending = value.get('inflight')
                row['inflight_kind'] = pending.get('kind') if type(pending) is dict else None
                need(row['inflight_kind'] in (None,'file','directory'),'unknown file progress')
            result[role] = row
        except Exception as error:result[role] = {'read_error_type':type(error).__name__}
    return result


class Monitor:
    def __init__(self):
        self.started = time.monotonic();self.stop = threading.Event();self.history = []
        self.last = None;self.changed = self.started;self.error = None

    def update(self, status='running', final=None):
        now = time.monotonic();progress = snapshot()
        if progress != self.last:
            if self.last is not None:
                self.history.append({'progress':self.last,'seconds':round(now-self.changed,3)})
                self.history = self.history[-128:]
            self.last = progress;self.changed = now
        value = {'schema':'gatehouse-operation-progress/v1','head':HEAD,'unit':UNIT,'status':status,
                 'elapsed_seconds':round(now-self.started,1),'current_phase_seconds':round(now-self.changed,1),
                 'updated_unix':round(time.time(),3),
                 'progress':progress,'administrator_ready':False,'production_gate_enabled':False,
                 'acceptance':'operator progress only; installation/quality acceptance is separate'}
        if final is not None:
            value['exit_code'] = final
        atomic(OUTPUT/'progress.json',value,0o600)
        atomic(PUBLIC,value,0o644)
        atomic(OUTPUT/'timings.json',{'head':HEAD,'transitions':self.history},0o600)

    def observe(self):
        while not self.stop.wait(10):
            try:self.update()
            except Exception as error:
                self.error = type(error).__name__
                return


class BoundedOutput(io.StringIO):
    def write(self, value):
        need(self.tell()+len(value) <= 65536,'bounded operator summary overflow')
        return super().write(value)


def main():
    need(os.geteuid() == 0,'administrator identity required')
    sys.dont_write_bytecode = True;os.umask(0o077)
    protected(OPERATION,directory=True);protected(OUTPUT,directory=True)
    source = read(OPERATION/'resume.py',65536)
    need(hashlib.sha256(source).hexdigest() == RESUME_PIN,'reviewed resume program changed')
    module = ModuleType('detached_reviewed_resume');module.__file__ = str(OPERATION/'resume.py')
    exec(compile(source,module.__file__,'exec'),module.__dict__)
    recovery_source = read(OPERATION/'recovery.py',65536)
    need(hashlib.sha256(recovery_source).hexdigest() == RECOVERY_PIN,'reviewed recovery program changed')
    recovery = ModuleType('detached_reviewed_recovery');recovery.__file__ = str(OPERATION/'recovery.py')
    exec(compile(recovery_source,recovery.__file__,'exec'),recovery.__dict__)
    module.literals(recovery_source);module.dispatch = recovery.dispatch
    monitor = Monitor();monitor.update()
    observer = threading.Thread(target=monitor.observe,name='operator-progress',daemon=True);observer.start()
    output = BoundedOutput()
    try:
        with contextlib.redirect_stdout(output):code = module.main()
        result = json.loads(output.getvalue())
    except BaseException as error:
        code = 1;result = {'head':HEAD,'status':'blocked','failure':{'type':type(error).__name__}}
    finally:
        monitor.stop.set();observer.join(15)
    if monitor.error:result['progress_observer_error_type'] = monitor.error
    try:monitor.update('component-complete' if code == 0 else 'blocked',code)
    except Exception as error:result['final_progress_error_type'] = type(error).__name__
    atomic(OUTPUT/'result.json',result,0o600)
    print(json.dumps(result,sort_keys=True))
    return code


if __name__ == '__main__':raise SystemExit(main())
