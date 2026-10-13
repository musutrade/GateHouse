use std::{path::Path, process::Command};

#[test]
fn shipped_cli_matches_both_corpora_and_negative_matrix_without_python_runtime() {
    let work = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new("python3")
        .arg(root.join("../quality/fixtures/generic-core/authority.py"))
        .arg("--harness-gate")
        .arg(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("--output")
        .arg(work.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
        std::fs::read_to_string(work.path().join("acceptance.json")).unwrap_or_default()
    );
}

#[test]
fn partial_base_context_is_rejected_before_evaluation() {
    let output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .args([
            "quality",
            "evaluate",
            "--project",
            "project.json",
            "--policy",
            "policy.json",
            "--evidence",
            "evidence.json",
            "--expected",
            "expected.json",
            "--source-root",
            ".",
            "--artifact-root",
            ".",
            "--output",
            "report.json",
            "--base-project",
            "base.json",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("--base-evidence"));
}

#[test]
fn compiled_configuration_matches_direct_rust_and_rejects_stale_inputs() {
    let work = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new("python3")
        .arg(root.join("fixtures/workflow/compiler/acceptance.py"))
        .arg("--harness-gate")
        .arg(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("--output")
        .arg(work.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn trusted_baselines_preserve_lineage_and_reject_stale_unknown_ecosystem_inputs() {
    let work = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new("python3")
        .arg(root.join("fixtures/workflow/baseline/acceptance.py"))
        .arg("--harness-gate")
        .arg(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("--output")
        .arg(work.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn explicit_policy_scopes_include_failing_components_and_reject_invalid_input() {
    let work = tempfile::tempdir().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new("python3")
        .arg(root.join("fixtures/workflow/compiler/scope_acceptance.py"))
        .arg("--harness-gate")
        .arg(env!("CARGO_BIN_EXE_harness-gate"))
        .arg("--output")
        .arg(work.path())
        .output()
        .unwrap();
    if !output.status.success() {
        // Keep the failing explicit inputs and CLI logs available for diagnosis.
        let retained = work.keep();
        panic!(
            "fixture retained at {}\n{}\n{}",
            retained.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
