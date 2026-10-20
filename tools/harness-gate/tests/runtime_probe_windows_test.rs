#![cfg(windows)]

use std::process::Command;

#[test]
fn non_unix_probe_rejects_before_reading_inputs() {
    let root = std::env::temp_dir().join(format!(
        "gatehouse-runtime-probe-windows-{}",
        std::process::id()
    ));
    let request = root.join("missing-request.json");
    let host = root.join("missing-host.json");
    let output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .args(["adapter", "runtime-probe"])
        .arg("--request")
        .arg(&request)
        .arg("--trusted-host")
        .arg(&host)
        .output()
        .unwrap();
    assert!(!output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    assert!(
        stderr.contains("unix") || stderr.contains("supported"),
        "{stderr}"
    );
    assert!(
        !stderr.contains("cannot open runtime input"),
        "non-Unix rejection must precede input reads: {stderr}"
    );
}
