use std::{env, fs, path::PathBuf};
fn main() {
    println!("cargo:rerun-if-env-changed=GH292_SOURCE_ROOT");
    let root = env::var_os("GH292_SOURCE_ROOT").map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3).unwrap().to_path_buf())
        .canonicalize().expect("resolve selected source root");
    let core = root.join("tools/harness-gate/quality-core/mod.rs");
    let text = root.join("tools/harness-gate/src/utils/redaction.rs");
    let report = root.join("tools/harness-gate/src/verify/report.rs");
    for path in [&core, &text, &report] { println!("cargo:rerun-if-changed={}",path.display()); }
    for entry in fs::read_dir(core.parent().unwrap()).unwrap() {
        let path = entry.unwrap().path(); if path.is_file() { println!("cargo:rerun-if-changed={}",path.display()); }
    }
    let dest = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::write(dest.join("source-bindings.rs"), format!("#[path = {:?}] mod quality;\n#[path = {:?}] mod text_redaction;\n", core, text)).unwrap();
    let bytes = fs::read_to_string(report).unwrap();
    let start = bytes.find("fn redact_json(").expect("report JSON helper start");
    let end = bytes[start..].find("fn write_generated_json(").expect("report JSON helper end") + start;
    fs::write(dest.join("report-redaction-functions.rs"), &bytes[start..end]).unwrap();
}
