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
