use super::*;

#[cfg(unix)]
pub(crate) fn install_fixture(project: &mut Project, service: &str, success: bool) {
    let path = project.root.join(format!("check-{service}"));
    let response = serde_json::json!({"protocol": PROTOCOL, "plugin": "fixture",
        "version": "1", "mode": "external", "success": success});
    std::fs::write(&path, format!("#!/bin/sh\nprintf '%s' '{}'\n", response)).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    project.config.service_checks.insert(
        service.into(),
        vec![ServiceCheck {
            executable: path.to_string_lossy().into(),
            sha256: format!("{:x}", Sha256::digest(std::fs::read(path).unwrap())),
            protocol: PROTOCOL.into(),
            plugin: "fixture".into(),
            version: "1".into(),
            timeout_secs: 1,
            pass_env: Default::default(),
        }],
    );
}

#[cfg(unix)]
fn check_fixture(body: &str) -> (tempfile::TempDir, ServiceCheck) {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("check");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    let check = ServiceCheck {
        executable: path.to_string_lossy().into(),
        sha256: format!("{:x}", Sha256::digest(std::fs::read(path).unwrap())),
        protocol: PROTOCOL.into(),
        plugin: "fixture".into(),
        version: "1".into(),
        timeout_secs: 1,
        pass_env: Default::default(),
    };
    (root, check)
}

#[cfg(unix)]
#[test]
fn required_check_rejects_missing_tampered_failed_and_incompatible_results() {
    for response in [
        "",
        "garbage",
        "{}",
        r#"{"protocol":"gate-service-check/v2","plugin":"fixture","version":"1","mode":"external","success":true}"#,
        r#"{"protocol":"gate-service-check/v1","plugin":"other","version":"1","mode":"external","success":true}"#,
        r#"{"protocol":"gate-service-check/v1","plugin":"fixture","version":"2","mode":"external","success":true}"#,
        r#"{"protocol":"gate-service-check/v1","plugin":"fixture","version":"1","mode":"managed","success":true}"#,
        r#"{"protocol":"gate-service-check/v1","plugin":"fixture","version":"1","mode":"external","success":false}"#,
        r#"{"protocol":"gate-service-check/v1","plugin":"fixture","version":"1","mode":"external","success":true,"extra":1}"#,
    ] {
        let (root, check) = check_fixture(&format!("printf '%s' '{response}'"));
        assert!(
            run_check(&check, root.path(), Some("opaque"), Duration::from_secs(1)).is_err(),
            "{response}"
        );
    }
    for body in ["exit 7", "kill -KILL $$", "/bin/sleep 2"] {
        let (root, check) = check_fixture(body);
        assert!(run_check(
            &check,
            root.path(),
            Some("opaque"),
            Duration::from_millis(100)
        )
        .is_err());
    }
    let (root, check) = check_fixture("exit 0");
    std::fs::write(&check.executable, "tampered").unwrap();
    assert!(run_check(&check, root.path(), None, Duration::from_secs(1)).is_err());
    std::fs::remove_file(&check.executable).unwrap();
    assert!(run_check(&check, root.path(), None, Duration::from_secs(1)).is_err());
}

#[cfg(unix)]
#[test]
fn malformed_check_diagnostics_do_not_publish_private_values() {
    for response in [
        r#"{"protocol":"gate-service-check/v1","plugin":"fixture","version":"1","mode":"external","success":"PRIVATE_SERVICE_VALUE"}"#,
        r#"{"PRIVATE_SERVICE_VALUE":true}"#,
    ] {
        let (root, check) = check_fixture(&format!("printf '%s' '{response}'"));
        let error =
            run_check(&check, root.path(), Some("opaque"), Duration::from_secs(1)).unwrap_err();
        let diagnostic = format!("{error:#}");
        assert!(diagnostic.contains("result missing or malformed"));
        assert!(
            !diagnostic.contains("PRIVATE_SERVICE_VALUE"),
            "{diagnostic}"
        );
    }
}

#[cfg(unix)]
#[test]
fn required_check_receives_private_value_and_explicit_environment_only() {
    let body = r#"[ "$GATE_CHECK_VALUE" = 'opaque-secret' ] || exit 1
[ "$GATE_CHECK_MODE" = 'external' ] || exit 2
[ "$HOME" = '' ] || exit 3
printf '%s' '{"protocol":"gate-service-check/v1","plugin":"fixture","version":"1","mode":"external","success":true}'"#;
    let (root, check) = check_fixture(body);
    run_check(
        &check,
        root.path(),
        Some("opaque-secret"),
        Duration::from_secs(1),
    )
    .unwrap();
}

#[test]
fn invalid_check_contract_fails_before_execution() {
    let mut check = ServiceCheck {
        executable: "/uninstalled/check".into(),
        sha256: "0".repeat(64),
        protocol: PROTOCOL.into(),
        plugin: "fixture".into(),
        version: "1".into(),
        timeout_secs: 1,
        pass_env: Default::default(),
    };
    assert!(validate_check(&check).is_ok());
    check.timeout_secs = 0;
    assert!(validate_check(&check).is_err());
    check.timeout_secs = 1;
    check.protocol = "v2".into();
    assert!(validate_check(&check).is_err());
    check.protocol = PROTOCOL.into();
    check.executable = "check".into();
    assert!(validate_check(&check).is_err());
    check.executable = "/check".into();
    check.sha256 = "0".into();
    assert!(validate_check(&check).is_err());
}
