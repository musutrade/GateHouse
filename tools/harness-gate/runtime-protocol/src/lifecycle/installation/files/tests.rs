use super::*;
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
struct Fixture {
    dir: tempfile::TempDir,
    host: Value,
    installation: Value,
    config: Value,
}
fn save(path: &Path, value: &Value) -> String {
    let bytes = canonical::encode(value).unwrap();
    fs::write(path, &bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    format!("{:x}", Sha256::digest(bytes))
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::var_os("GATEHOUSE_JOURNAL_TEST_ROOT")
            .or_else(|| std::env::var_os("HOME"))
            .unwrap();
        let dir = tempfile::Builder::new()
            .prefix("installation-test-")
            .tempdir_in(root)
            .unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let executable = dir.path().join("adapter");
        fs::write(&executable, b"fixture never executed").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o500)).unwrap();
        let expected = super::super::super::wire::parse_request(include_bytes!(
            "../../wire/status-request.json"
        ))
        .unwrap();
        let mut engine = expected.binding.engine_binding;
        engine.principal_uid = unsafe { libc::geteuid() };
        let config = json!({"schema":"gate-runtime/lifecycle-config-v1","engine_binding":engine});
        let installation = json!({"schema":"gate-runtime/lifecycle-installation-v1","product":"docker-lifecycle","version":"0.1.0","protocol":PROTOCOL,"executable_path":executable,"executable_sha256":format!("{:x}",Sha256::digest(b"fixture never executed")),"config_schema":"gate-runtime/lifecycle-config-v1"});
        let host = json!({"schema":"gate-runtime/lifecycle-host-v1","installation_path":dir.path().join("installation.json"),"installation_sha256":"","config_path":dir.path().join("config.json"),"config_sha256":""});
        let mut f = Self {
            dir,
            host,
            installation,
            config,
        };
        f.flush();
        f
    }
    fn podman() -> Self {
        let mut f = Self::new();
        f.installation["product"] = "podman-lifecycle".into();
        f.installation["version"] = "0.3.0".into();
        f.config["engine_binding"]["engine"] = "podman".into();
        f.config["engine_binding"]["api_version"] = "5.7.0".into();
        f.config["engine_binding"]["server_version"] = "5.7.0".into();
        f.config["engine_binding"]["rootless"] = true.into();
        f.flush();
        f
    }
    fn path(&self) -> PathBuf {
        self.dir.path().join("host.json")
    }
    fn flush(&mut self) {
        self.host["installation_sha256"] = save(
            &self.dir.path().join("installation.json"),
            &self.installation,
        )
        .into();
        self.host["config_sha256"] =
            save(&self.dir.path().join("config.json"), &self.config).into();
        save(&self.path(), &self.host);
    }
    fn request(&self) -> Request {
        let mut r = super::super::super::wire::parse_request(include_bytes!(
            "../../wire/status-request.json"
        ))
        .unwrap();
        r.binding.installation_sha256 = self.host["installation_sha256"].as_str().unwrap().into();
        r.binding.config_sha256 = self.host["config_sha256"].as_str().unwrap().into();
        r.binding.executable_sha256 = self.installation["executable_sha256"]
            .as_str()
            .unwrap()
            .into();
        r.binding.engine_binding =
            serde_json::from_value(self.config["engine_binding"].clone()).unwrap();
        r.binding.operation_key = r.operation_key().unwrap();
        r
    }
}
#[test]
fn exact_snapshot_and_request_binding_are_required() {
    let f = Fixture::new();
    let loaded = VerifiedInstallation::load(&f.path()).unwrap();
    let r = f.request();
    loaded.validate_request(&r).unwrap();
    let snapshot = loaded.snapshot().unwrap();
    assert_eq!(snapshot.installation_sha256, r.binding.installation_sha256);
    assert_eq!(snapshot.executable_sha256, r.binding.executable_sha256);
    assert_eq!(snapshot.config_sha256, r.binding.config_sha256);
    assert_eq!(snapshot.engine_binding, r.binding.engine_binding);
    assert!(loaded.executable().is_ok());
    for field in ["installation_sha256", "config_sha256", "executable_sha256"] {
        let mut v = serde_json::to_value(&r).unwrap();
        v["binding"][field] = json!("a".repeat(64));
        let mut wrong: Request = serde_json::from_value(v).unwrap();
        wrong.binding.operation_key = wrong.operation_key().unwrap();
        assert!(loaded.validate_request(&wrong).is_err());
    }
}
#[test]
fn old_hosts_public_expected_and_bad_engine_cannot_be_loaded() {
    for schema in ["gate-runtime/installed-host-v2", "gate-runtime/v3"] {
        let mut f = Fixture::new();
        f.host["schema"] = schema.into();
        f.flush();
        assert!(VerifiedInstallation::load(&f.path()).is_err());
    }
    let mut f = Fixture::new();
    f.host["expected"] = "/tmp/public.json".into();
    f.flush();
    assert!(VerifiedInstallation::load(&f.path()).is_err());
    let mut f = Fixture::new();
    f.config["engine_binding"]["engine"] = "podman".into();
    f.flush();
    assert!(VerifiedInstallation::load(&f.path()).is_err());
}
#[test]
fn executable_substitution_even_identical_bytes_invalidates_held_identity() {
    let f = Fixture::new();
    let loaded = VerifiedInstallation::load(&f.path()).unwrap();
    let replacement = f.dir.path().join("replacement");
    fs::write(&replacement, b"fixture never executed").unwrap();
    fs::set_permissions(&replacement, fs::Permissions::from_mode(0o500)).unwrap();
    fs::rename(replacement, f.dir.path().join("adapter")).unwrap();
    assert!(loaded.revalidate().is_err());
    assert!(loaded.executable().is_err());
    assert!(loaded.snapshot().is_err());
}
#[test]
fn config_tamper_symlink_hardlink_and_unsafe_permissions_fail() {
    let f = Fixture::new();
    fs::write(f.dir.path().join("config.json"), b"{}").unwrap();
    assert!(VerifiedInstallation::load(&f.path()).is_err());
    let f = Fixture::new();
    let link = f.dir.path().join("host-link");
    std::os::unix::fs::symlink(f.path(), &link).unwrap();
    assert!(VerifiedInstallation::load(&link).is_err());
    let f = Fixture::new();
    fs::hard_link(f.dir.path().join("adapter"), f.dir.path().join("second")).unwrap();
    assert!(VerifiedInstallation::load(&f.path()).is_err());
    let f = Fixture::new();
    fs::set_permissions(f.path(), fs::Permissions::from_mode(0o644)).unwrap();
    assert!(VerifiedInstallation::load(&f.path()).is_err());
}
#[test]
fn duplicate_descriptor_fields_and_wrong_executable_digest_fail() {
    let f = Fixture::new();
    let bytes = fs::read_to_string(f.path()).unwrap();
    fs::write(
        f.path(),
        bytes.replace(
            "\"schema\":",
            "\"schema\":\"gate-runtime/lifecycle-host-v1\",\"schema\":",
        ),
    )
    .unwrap();
    assert!(VerifiedInstallation::load(&f.path()).is_err());
    let mut f = Fixture::new();
    f.installation["executable_sha256"] = "0".repeat(64).into();
    f.flush();
    assert!(VerifiedInstallation::load(&f.path()).is_err());
}

#[test]
fn held_config_snapshot_and_every_descriptor_reject_same_byte_replacement() {
    for member in ["host.json", "installation.json", "config.json"] {
        let f = Fixture::new();
        let loaded = VerifiedInstallation::load(&f.path()).unwrap();
        let r = f.request();
        assert_eq!(
            loaded.config_bytes().unwrap(),
            fs::read(f.dir.path().join("config.json")).unwrap()
        );
        assert_eq!(loaded.engine_binding().unwrap(), &r.binding.engine_binding);
        let path = f.dir.path().join(member);
        let replacement = f.dir.path().join("replacement");
        fs::write(&replacement, fs::read(&path).unwrap()).unwrap();
        fs::set_permissions(&replacement, fs::Permissions::from_mode(0o600)).unwrap();
        fs::rename(replacement, path).unwrap();
        assert!(loaded.validate_request(&r).is_err());
        assert!(loaded.config_bytes().is_err());
        assert!(loaded.engine_binding().is_err());
        assert!(loaded.executable().is_err());
        assert!(loaded.snapshot().is_err());
    }
}

#[test]
fn podman_5_7_mutation_profile_loads_and_capture_only_or_mixed_versions_do_not() {
    let f = Fixture::podman();
    let loaded = VerifiedInstallation::load(&f.path()).unwrap();
    let r = f.request();
    loaded.validate_request(&r).unwrap();
    assert_eq!(loaded.engine_binding().unwrap().engine, "podman");

    for (product, version, engine, api) in [
        ("podman-lifecycle", "0.2.0", "podman", "5.7.0"),
        ("podman-lifecycle", "0.3.0", "podman", "6.1.4"),
        ("podman-lifecycle", "0.3.0", "docker", "5.7.0"),
        ("docker-lifecycle", "0.3.0", "podman", "5.7.0"),
        ("docker-lifecycle", "0.1.0", "podman", "5.7.0"),
    ] {
        let mut f = Fixture::new();
        f.installation["product"] = product.into();
        f.installation["version"] = version.into();
        f.config["engine_binding"]["engine"] = engine.into();
        f.config["engine_binding"]["api_version"] = api.into();
        f.config["engine_binding"]["server_version"] = api.into();
        if engine == "podman" {
            f.config["engine_binding"]["rootless"] = true.into();
        }
        f.flush();
        assert!(
            VerifiedInstallation::load(&f.path()).is_err(),
            "{product}/{version}/{engine}/{api}"
        );
    }
}
