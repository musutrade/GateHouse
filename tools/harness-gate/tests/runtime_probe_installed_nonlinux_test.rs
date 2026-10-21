#![cfg(not(target_os = "linux"))]

use std::process::Command;

#[test]
fn installed_probe_refuses_before_reading_inputs_on_nonlinux() {
    let output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .args([
            "adapter",
            "runtime-probe-installed",
            "--request",
            "/definitely/missing/actual.json",
            "--trusted-host",
            "/definitely/missing/host.json",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("only on Linux"));
}
