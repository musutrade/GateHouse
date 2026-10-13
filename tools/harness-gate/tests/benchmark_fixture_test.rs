use std::process::Command;

#[test]
fn parallel_benchmark_fixture_preserves_five_sample_validation() {
    let python = if cfg!(windows) { "python" } else { "python3" };
    let output = Command::new(python)
        .args([
            "-B",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../quality/tests/benchmark_fixture_probe.py"
            ),
            "--binary",
            env!("CARGO_BIN_EXE_harness-gate"),
        ])
        .output()
        .expect("start real parallel benchmark fixture probe");
    assert!(
        output.status.success(),
        "parallel benchmark fixture: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
