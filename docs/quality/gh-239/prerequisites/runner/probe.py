import hashlib,json,os,pathlib,platform,subprocess
expected=json.loads(pathlib.Path('/probe/host-library-inputs.json').read_text())
observed={}
for name,row in expected.items():
 digest=hashlib.sha256(pathlib.Path(row['path']).read_bytes()).hexdigest()
 assert digest==row['sha256'],name
 observed[name]=digest
assert os.getuid()==1000
assert not pathlib.Path('/var/run/docker.sock').exists()
assert not pathlib.Path('/home/gem').exists()
assert not pathlib.Path('/root/.ssh').exists()
for path in ['/etc/gh239-probe','/opt/actions-runner/gh239-probe']:
 try:
  pathlib.Path(path).write_text('unexpected')
 except OSError:pass
 else:raise AssertionError('writable trusted root: '+path)
status=pathlib.Path('/proc/self/status').read_text()
assert 'NoNewPrivs:\t1' in status
assert 'CapEff:\t0000000000000000' in status
commands={}
for name,args in {'runner':['/opt/actions-runner/bin/Runner.Listener','--version'],'node':['/opt/actions-runner/externals/node24/bin/node','--version'],'git':['git','--version'],'python':['python3','--version'],'openssl':['openssl','version'],'gh':['gh','--version']}.items():
 commands[name]=subprocess.check_output(args,text=True,stderr=subprocess.STDOUT).strip()
assert commands['runner']=='2.337.0'
print(json.dumps({'status':'local-isolation-and-startup-pass','scope':'offline local container; no Actions job or production certification','host_libraries':observed,'kernel':platform.release(),'uid':os.getuid(),'commands':commands},indent=2))
