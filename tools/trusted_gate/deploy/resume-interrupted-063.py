#!/usr/bin/python3 -I
"""Execute the same pinned administrator dispatch once, exposing only safe failures."""
import ast
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys
from types import ModuleType, SimpleNamespace

HEAD = '063eac64c53b69371ce603f87396f5b2710c8cbb'
STAGE = Path('/root/gatehouse-issue98-linux-063eac64c53b')
HELPERS = STAGE/'helpers'
MANIFEST = 'eeab28487397d64dcd7690c8692b94fa55aabfbcfc1db097645c8fe21e966438'
RECIPE = 'beac6ca0e956ae9c9a32be0810f6ba53f8717e1097e9d889d9a5d2a4028739a6'
ENTRY = 'tools/trusted_gate/deploy/import-protected-inputs.py'
ENTRY_PIN = 'ed041a7a60be365219efafae4d1963e05a4a7aa83b1cd4312902f465b03411fa'
CONTEXT_NAMES = ('need', 'owned', 'path_operand', 'read', 'write', 'record', 'canonical', 'unique',
                 'git', 'frozen_lock', 'load', 'authenticate', 'readonly', 'import_inputs', 'ENV')
SAFE = set()


def need(condition, message):
    if not condition:raise ValueError(message)


def read(path, limit):
    path = Path(path)
    for parent in (path, *path.parents):
        info = parent.lstat()
        need(info.st_uid == 0 and not stat.S_ISLNK(info.st_mode) and not info.st_mode & 0o022,
             'reviewed input is not Root-protected')
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as stream:
        before = os.fstat(stream.fileno())
        need(stat.S_ISREG(before.st_mode) and before.st_nlink == 1 and before.st_size <= limit,
             'reviewed input type or size differs')
        raw = stream.read(limit+1)
        identity = lambda i:(i.st_dev,i.st_ino,i.st_uid,i.st_gid,i.st_mode,i.st_nlink,
                            i.st_size,i.st_mtime_ns,i.st_ctime_ns)
        need(len(raw) == before.st_size and identity(before) == identity(os.fstat(stream.fileno()))
             == identity(path.lstat()), 'reviewed input changed while reading')
    return raw


def literals(raw):
    for node in ast.walk(ast.parse(raw)):
        if isinstance(node, ast.Call):
            target = node.func
            name = target.id if isinstance(target, ast.Name) else target.attr if isinstance(target, ast.Attribute) else ''
            index = 1 if name in ('need','require') else 0 if name in ('ValueError','RuntimeError') else None
            if index is not None and len(node.args) > index:
                value = node.args[index]
                if isinstance(value, ast.Constant) and type(value.value) is str:SAFE.add(value.value)


def failure(error):
    frames = []
    tb = error.__traceback__
    while tb:
        filename = tb.tb_frame.f_code.co_filename
        path = Path(filename)
        if HELPERS in path.parents:
            frames.append({'file':str(path.relative_to(HELPERS)), 'line':tb.tb_lineno,
                           'function':tb.tb_frame.f_code.co_name})
        elif re.fullmatch('<accepted659-[a-z_]+>', filename):
            frames.append({'file':filename, 'line':tb.tb_lineno, 'function':tb.tb_frame.f_code.co_name})
        tb = tb.tb_next
    message = str(error)
    return {'type':type(error).__name__, 'guard':message if message in SAFE else 'unlisted exception text omitted',
            'helper_frames':frames[-8:]}


def progress():
    rows = []
    for name in ('administrator-upgrade.json','entry-fence.json','linux-upgrade.json','file-install.json',
                 'controller-activation.json','mount-persistence.json'):
        path = Path('/srv/gatehouse/pilot/control')/name
        row = {'record':name}
        try:
            row['exists'] = path.exists() or path.is_symlink()
            if row['exists']:
                value = json.loads(read(path,1024**2))
                for key in ('phase','head','inflight'):
                    item = value.get(key)
                    if item is None or type(item) is str and re.fullmatch('[A-Za-z0-9_-]{1,64}',item):row[key] = item
                if type(value.get('completed')) is list:row['completed_count'] = len(value['completed'])
        except Exception as error:row['read_error_type'] = type(error).__name__
        rows.append(row)
    return rows


def dispatch(B, A):
    """This is the original importer main() context and one original run() call."""
    B.authenticate(HELPERS,STAGE/'helper-manifest.json',MANIFEST)
    context = SimpleNamespace(**{name:getattr(B,name) for name in CONTEXT_NAMES})
    return A.run(context,HELPERS,STAGE/'helper-manifest.json',MANIFEST,STAGE/'administrator.json',RECIPE)


def main():
    if os.geteuid() != 0:raise SystemExit('administrator authentication required for the reviewed migration attempt')
    sys.dont_write_bytecode = True
    os.umask(0o077)
    literals(Path(__file__).read_bytes())
    report = {'head':HEAD,'dispatch_attempted':False,'automatic_retries':0,
              'scope':'same authenticated administrator dispatch; candidate migration, production remains disabled'}
    try:
        raw = read(STAGE/'helper-manifest.json',64*1024)
        need(hashlib.sha256(raw).hexdigest() == MANIFEST,'staged manifest differs from reviewed pin')
        code = json.loads(raw)
        need(code['revision'] == HEAD and len(code['files']) == 31 and code['files'][ENTRY] == ENTRY_PIN,
             'reviewed source identity differs')
        entry_source = None
        for name,pin in code['files'].items():
            need(not Path(name).is_absolute() and '..' not in Path(name).parts,'unsafe reviewed helper name')
            source = read(HELPERS/name,1024**2)
            need(hashlib.sha256(source).hexdigest() == pin,'staged helper differs from reviewed pin')
            if name == ENTRY:entry_source = source
            if name.endswith('.py'):literals(source)
        need(hashlib.sha256(read(STAGE/'administrator.json',64*1024)).hexdigest() == RECIPE,
             'staged recipe differs from reviewed pin')
        B = ModuleType('reviewed_single_attempt_importer')
        B.__file__ = str(HELPERS/ENTRY)
        exec(compile(entry_source,B.__file__,'exec'),B.__dict__)
        B.authenticate(HELPERS,STAGE/'helper-manifest.json',MANIFEST)
        A = B.load(HELPERS,'administrator-upgrade.py')
        report['dispatch_attempted'] = True
        result = dispatch(B,A)
        need(result['head'] == HEAD and result['phase'] == 'complete'
             and result['administrator_ready'] is False and result['production_gate_enabled'] is False,
             'administrator dispatch returned an unexpected result')
        report.update({key:result[key] for key in ('schema','head','phase','administrator_ready','production_gate_enabled')})
        report['status'] = 'component-complete'
        status = 0
    except Exception as error:
        report.update(status='blocked',failure=failure(error),receipt_progress=progress())
        status = 1
    print(json.dumps(report,indent=2,ensure_ascii=True))
    return status


if __name__ == '__main__':raise SystemExit(main())
