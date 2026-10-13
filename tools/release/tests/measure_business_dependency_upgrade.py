"""Native local path-dependency upgrade diagnostic (runtime and output arguments).

Run with the runtime's private Python, libraries and bin PATH; see the GH-255
cost report. Does not adopt a project baseline or constitute release approval.
"""
import json,subprocess,sys
from pathlib import Path
runtime=Path(sys.argv[1]).resolve(); work=Path(sys.argv[2]).resolve(); work.mkdir()
sys.path.insert(0,str(runtime/'app'))
import rust_native_driver as native
project=work/'project'
for name in ('src','tests','business/src'): (project/name).mkdir(parents=True,exist_ok=True)
(project/'Cargo.toml').write_text('[package]\nname="dependency-upgrade-fixture"\nversion="0.1.0"\nedition="2021"\n[workspace]\nexclude=["business"]\n[dependencies]\nbusiness={path="business"}\n')
(project/'src/lib.rs').write_text('pub fn value()->u32 { business::value() }\n')
(project/'tests/contract.rs').write_text('#[test] fn contract(){ assert!(dependency_upgrade_fixture::value()>0); }\n')
results=[]
for number in (1,2):
 (project/'business/Cargo.toml').write_text('[package]\nname="business"\nversion="0.1.'+str(number)+'"\nedition="2021"\n')
 (project/'business/src/lib.rs').write_text('pub fn value()->u32 {'+str(number)+'}\n')
 subprocess.run([str(runtime/'rust/bin/cargo'),'generate-lockfile','--offline','--manifest-path',str(project/'Cargo.toml')],check=True)
 output=work/('capture-'+str(number))
 anchor=native.collect_cargo(project/'Cargo.toml',output,runtime/'bin/harness-gate-rust-native-driver',runtime/'rust',['contract'],runtime=runtime)
 report=native.certify(output/'raw',anchor)
 results.append({'business_version':'0.1.'+str(number),'anchor':anchor,'certified':True})
captures=[json.loads((work/('capture-'+str(n))/'raw/capture.json').read_text()) for n in (1,2)]
assert captures[0]['tools']==captures[1]['tools'], 'measurement tool identity changed'
assert captures[0]['cargo_inputs']!=captures[1]['cargo_inputs'], 'dependency upgrade not captured'
(work/'report.json').write_text(json.dumps({'same_runtime':str(runtime),'same_tool_identity':True,'changed_cargo_inputs':True,'results':results},indent=2)+'\n')
print('Both changed business dependency captures certified using the unchanged private runtime')
