import json,os,subprocess,time
from pathlib import Path
root=Path.cwd();base=root/'target/release-0.4.0';env=os.environ.copy()
env.update(CARGO_TARGET_DIR=str(base/'cargo'),TMPDIR=str(base/'tmp'),GIT_CEILING_DIRECTORIES=str(base/'tmp'),OPENSPEC_TELEMETRY='0')
for k in ('HTTP_PROXY','HTTPS_PROXY','ALL_PROXY','http_proxy','https_proxy','all_proxy'):env.pop(k,None)
commands=[
 ('nextest',['cargo','nextest','run','--manifest-path','tools/harness-gate/Cargo.toml','--locked','--no-fail-fast']),
 ('clippy',['cargo','clippy','--manifest-path','tools/harness-gate/Cargo.toml','--locked','--all-targets','--all-features','--','-D','warnings']),
 ('python',['python3','-m','unittest','discover','-s','tools/quality/tests','-v']),
 ('contracts',['python3','tools/quality/contracts.py','--output',str(base/'contracts.json')]),
 ('docs',['python3','tools/quality/docs_consistency.py','--output',str(base/'docs.json')]),
 ('package',['cargo','package','--manifest-path','tools/harness-gate/Cargo.toml','--locked','--allow-dirty']),
]
rows=[]
for name,command in commands:
 started=time.monotonic()
 with (base/'logs'/f'{name}.log').open('w') as log:r=subprocess.run(command,env=env,stdout=log,stderr=subprocess.STDOUT)
 rows.append({'name':name,'command':command,'exit_code':r.returncode,'seconds':time.monotonic()-started})
 (base/'checks.json').write_text(json.dumps(rows,indent=2));print(name,r.returncode,flush=True)
