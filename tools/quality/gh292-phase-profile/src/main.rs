include!(concat!(env!("OUT_DIR"), "/source-bindings.rs"));
use anyhow::{bail,Context,Result};
use serde::Serialize;
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{collections::BTreeMap,fs,path::Path,time::Instant};
use text_redaction::{redact_text,REDACTION_TEXT_LIMIT};
const MAX_INVOCATION_EVIDENCE_BYTES:u64=256*1024*1024;
include!(concat!(env!("OUT_DIR"), "/report-redaction-functions.rs"));
fn hash(bytes:&[u8])->String{format!("{:x}",Sha256::digest(bytes))}
fn stats()->Value{
    let mut usage:libc::rusage=unsafe{std::mem::zeroed()};
    assert_eq!(unsafe{libc::getrusage(libc::RUSAGE_SELF,&mut usage)},0);
    let cpu=usage.ru_utime.tv_sec as f64+usage.ru_utime.tv_usec as f64/1e6
          +usage.ru_stime.tv_sec as f64+usage.ru_stime.tv_usec as f64/1e6;
    let image_peak=fs::read_to_string("/proc/self/status").unwrap().lines().find_map(|line| line.strip_prefix("VmHWM:").map(|v| v.split_whitespace().next().unwrap().parse::<u64>().unwrap())).expect("Linux VmHWM missing");
    let io:BTreeMap<String,u64>=fs::read_to_string("/proc/self/io").unwrap().lines().filter_map(|l|{
        let (k,v)=l.split_once(':')?;Some((k.to_owned(),v.trim().parse().ok()?))}).collect();
    json!({"cpu_seconds":cpu,"peak_rss_kib":image_peak,"io":io})
}
fn phase<T>(name:&str,rows:&mut Vec<Value>,f:impl FnOnce()->Result<T>)->Result<T>{
    let before=stats();let start=Instant::now();let result=f();let seconds=start.elapsed().as_secs_f64();let after=stats();
    let mut delta=serde_json::Map::new();
    for (key,val) in after["io"].as_object().unwrap(){delta.insert(key.clone(),json!(val.as_u64().unwrap()-before["io"][key].as_u64().unwrap()));}
    rows.push(json!({"phase":name,"wall_seconds":seconds,"cpu_seconds":after["cpu_seconds"].as_f64().unwrap()-before["cpu_seconds"].as_f64().unwrap(),"peak_rss_kib_cumulative":after["peak_rss_kib"],"kernel_io_delta":delta,"success":result.is_ok()}));result
}
fn load(p:&Path)->Result<Value>{Ok(serde_json::from_slice(&fs::read(p)?)?)}
fn main()->Result<()>{
    let args:Vec<_>=std::env::args().collect();if args.len()!=3{bail!("usage: phase-profile INPUT_DIR NEW_OUTPUT_DIR");}
    let input=Path::new(&args[1]);let out=Path::new(&args[2]);fs::create_dir(out)?;
    let start=Instant::now();let initial=stats();let mut phases=Vec::new();
    let (project,policy,records,expected,configuration)=phase("read_parse_inputs",&mut phases,||Ok((load(&input.join("project.json"))?,load(&input.join("policy.json"))?,load(&input.join("records.json"))?,load(&input.join("expected.json"))?,load(&input.join("fixture.json"))?)))?;
    let source_root=Path::new(configuration["source_root"].as_str().unwrap());let artifact_root=Path::new(configuration["artifact_root"].as_str().unwrap());
    let context=quality::evidence::ValidationContext{project:&project,source_root,artifact_root,expected:&expected};
    phase("standalone_evidence_validation",&mut phases,||Ok(quality::evidence::validate_evidence(&records,&context)?))?;
    let evaluated=phase("policy_evaluation_includes_validation",&mut phases,||Ok(quality::policy::evaluate(&policy,&records,&context,&Default::default())?))?;
    assert_eq!(evaluated["aggregate"]["state"],configuration["expected_state"]);
    let report=phase("project_report_build",&mut phases,||Ok(quality::project_report::report(&evaluated,&project,&policy)?))?;
    assert_eq!(report["gates"].as_object().unwrap().len(),evaluated["results"].as_array().unwrap().len());
    let mut value=phase("report_value_conversion",&mut phases,||Ok(serde_json::to_value(&report)?))?;
    phase("report_json_redaction",&mut phases,||redact_json(&mut value))?;
    let bytes=phase("report_pretty_serialization",&mut phases,||Ok(serde_json::to_vec_pretty(&value)?))?;
    assert!(bytes.len() as u64<=MAX_INVOCATION_EVIDENCE_BYTES);
    let digest=hash(&bytes);
    phase("report_write",&mut phases,||Ok(fs::write(out.join("quality-project-report.json"),&bytes)?))?;
    let mut delivery=Vec::new();
    for complete in [false,true]{
        // Diagnostic delivery envelopes preserve a fixed unchanged quality model.
        // These are synthetic; they do not claim to be production MachineResult.
        let model=json!({"fixture":"gh292-diagnostic-delivery/v1","complete":complete,"state":evaluated["aggregate"]["state"],"report_sha256":digest,"gate_count":evaluated["results"].as_array().unwrap().len()});
        let label=if complete{"final_delivery_serialize_redact"}else{"preliminary_delivery_serialize_redact"};
        let content=phase(label,&mut phases,||redacted_json_bytes(&model))?;
        let path=if complete{"diagnostic-final.json"}else{"diagnostic-preliminary.json"};
        phase(if complete{"final_delivery_write"}else{"preliminary_delivery_write"},&mut phases,||Ok(fs::write(out.join(path),&content)?))?;
        delivery.push(json!({"path":path,"bytes":content.len(),"sha256":hash(&content)}));
    }
    let final_stats=stats();let result=json!({"fixture":configuration,"input_sha256":hash(&fs::read(input.join("records.json"))?),"phases":phases,"wall_seconds":start.elapsed().as_secs_f64(),"cpu_seconds":final_stats["cpu_seconds"].as_f64().unwrap()-initial["cpu_seconds"].as_f64().unwrap(),"peak_rss_kib":final_stats["peak_rss_kib"],"gate_count":evaluated["results"].as_array().unwrap().len(),"report_bytes":bytes.len(),"report_sha256":digest,"diagnostic_delivery":delivery,"boundaries":["External collectors absent; historical consumer report stage not attributed.","Standalone evidence validation is an explicit diagnostic repetition; policy evaluation also validates.","Phase RSS is cumulative process high-water; phase kernel reads include instrumentation snapshots.","Fresh process with uncontrolled OS page cache; no cold disk/cache claim.","Diagnostic preliminary/final envelopes are not production MachineResult; actual publication regressions require original product tests."]});
    fs::write(out.join("measurement.json"),serde_json::to_vec_pretty(&result)?)?;
    println!("{}",json!({"gates":result["gate_count"],"bytes":bytes.len(),"wall_seconds":result["wall_seconds"],"report_sha256":digest}));Ok(())
}
