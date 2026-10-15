//! Offline CLI acceptance; all identities and signatures are created by the test operator.
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ed25519_dalek::{Signer, SigningKey};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

struct Fixture {
    temp: tempfile::TempDir,
    key: SigningKey,
    manifest: Value,
    lock: Value,
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn write(path: &Path, bytes: impl AsRef<[u8]>) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let key = SigningKey::from_bytes(&[35; 32]);
        let flow = include_bytes!("../presets/generic.flow.toml");
        let catalog = br#"{"schema":"harness-preset-catalog/v1","presets":{"strange-ecosystem":{"description":"External name","flow":"flow.toml"}},"packs":{}}"#;
        let manifest = json!({"schema":"harness-plugin/v1", "name":"unknown-ecosystem", "version":"1.2.3", "source":"https://example.invalid/reviewed",
            "core_versions":[env!("CARGO_PKG_VERSION")], "protocol":"harness-preset-catalog/v1",
            "platforms":[format!("{}-{}",std::env::consts::OS,std::env::consts::ARCH)],
            "presets":"presets", "programs":["git"], "dependencies":{},
            "files":{"presets/flow.toml":hash(flow), "presets/catalog.json":hash(catalog)}});
        let lock = json!({"schema":"harness-plugin-lock/v1","plugins":[{"name":"unknown-ecosystem","version":"1.2.3","source":"https://example.invalid/reviewed","manifest_sha256":"", "signer":"operator"}]});
        write(
            &temp
                .path()
                .join("store/unknown-ecosystem/1.2.3/presets/flow.toml"),
            flow,
        );
        write(
            &temp
                .path()
                .join("store/unknown-ecosystem/1.2.3/presets/catalog.json"),
            catalog,
        );
        write(&temp.path().join("operator.json"),serde_json::to_vec(&json!({"key_id":"operator","public_key":BASE64.encode(key.verifying_key().as_bytes())})).unwrap());
        let mut fixture = Self {
            temp,
            key,
            manifest,
            lock,
        };
        fixture.seal();
        fixture
    }
    fn root(&self) -> std::path::PathBuf {
        self.temp.path().join("project")
    }
    fn seal(&mut self) {
        let bytes = serde_json::to_vec(&self.manifest).unwrap();
        self.lock["plugins"][0]["manifest_sha256"] = json!(hash(&bytes));
        let payload = [b"harness-gate/plugin-manifest/v1\0".as_slice(), &bytes].concat();
        let signature = self.key.sign(&payload);
        write(
            &self
                .temp
                .path()
                .join("store/unknown-ecosystem/1.2.3/plugin.json"),
            bytes,
        );
        write(
            &self
                .temp
                .path()
                .join("store/unknown-ecosystem/1.2.3/plugin.sig"),
            serde_json::to_vec(
                &json!({"key_id":"operator","signature":BASE64.encode(signature.to_bytes())}),
            )
            .unwrap(),
        );
        self.save_lock();
    }
    fn save_lock(&self) {
        write(
            &self.root().join(".harness-gate/plugins.lock.json"),
            serde_json::to_vec(&self.lock).unwrap(),
        );
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_harness-gate"))
            .env_remove("HARNESS_GATE_PRESETS_DIR")
            .arg("--project-root")
            .arg(self.root())
            .arg("--plugin-store")
            .arg(self.temp.path().join("store"))
            .arg("--plugin-trusted-key")
            .arg(self.temp.path().join("operator.json"))
            .args(args)
            .output()
            .unwrap()
    }
    fn check(&self) -> Output {
        self.run(&[
            "plugins",
            "--lock",
            self.root()
                .join(".harness-gate/plugins.lock.json")
                .to_str()
                .unwrap(),
            "--json",
        ])
    }
    fn init(&self) {
        success(&self.run(&[
            "init",
            "--plugin",
            "unknown-ecosystem",
            "--preset",
            "strange-ecosystem",
        ]));
    }
}

fn success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
fn failure(output: &Output, code: &str) {
    assert!(!output.status.success());
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(all.contains(code), "expected {code}: {all}");
}

#[test]
fn unknown_ecosystem_works_offline_through_init_check_doctor_plan_and_verify() {
    let f = Fixture::new();
    success(&f.check());
    let before = fs::read(f.root().join(".harness-gate/plugins.lock.json")).unwrap();
    f.init();
    assert_eq!(
        before,
        fs::read(f.root().join(".harness-gate/plugins.lock.json")).unwrap()
    );
    success(&f.run(&["config", "check"]));
    success(&f.run(&["config", "check", "--format", "json"]));
    for args in [
        &["init"][..],
        &["config", "user.email", "fixture@example.invalid"],
        &["config", "user.name", "Fixture"],
        &["config", "gc.auto", "0"],
        &["config", "maintenance.auto", "false"],
        &["add", "."],
        &["-c", "commit.gpgsign=false", "commit", "-m", "fixture"],
    ] {
        success(
            &Command::new("git")
                .args(args)
                .current_dir(f.root())
                .output()
                .unwrap(),
        );
    }
    success(&f.run(&["doctor", "--json"]));
    success(&f.run(&["verify", "--plan", "--json", "--all"]));
    success(&f.run(&["verify", "--plan", "--json", "--staged"]));
    success(&f.run(&["verify", "--all"]));
    success(&f.run(&["verify", "--staged"]));
    // Staged commands consume the indexed lock, never the contradictory worktree pin.
    write(
        &f.root().join(".harness-gate/plugins.lock.json"),
        b"broken worktree lock",
    );
    success(&f.run(&["verify", "--plan", "--json", "--staged"]));
    success(&f.run(&["verify", "--staged"]));
    failure(
        &f.run(&["config", "check", "--format", "json"]),
        "HGPLUGIN-LOCK",
    );
    failure(&f.run(&["doctor", "--json"]), "HGPLUGIN-LOCK");
    failure(&f.run(&["verify", "--all"]), "HGPLUGIN-LOCK");
    failure(
        &f.run(&["verify", "--plan", "--json", "--all"]),
        "HGPLUGIN-LOCK",
    );
}

#[test]
fn compatibility_and_dependency_failures_are_stable_and_actionable() {
    for (field, value, code) in [
        ("schema", json!("harness-plugin/v99"), "HGPLUGIN-MANIFEST"),
        ("core_versions", json!(["0.0.0"]), "HGPLUGIN-CORE"),
        ("protocol", json!("invented/v99"), "HGPLUGIN-PROTOCOL"),
        (
            "platforms",
            json!(["alien-unavailable"]),
            "HGPLUGIN-PLATFORM",
        ),
        ("name", json!("different"), "HGPLUGIN-VERSION"),
        ("version", json!("9.0.0"), "HGPLUGIN-VERSION"),
        ("source", json!("changed-source"), "HGPLUGIN-VERSION"),
        (
            "programs",
            json!(["harness_issue30_missing_tool_951"]),
            "HGPLUGIN-DEPENDENCY",
        ),
        ("programs", json!(["../tool"]), "HGPLUGIN-DEPENDENCY"),
        ("programs", json!(["git", "git"]), "HGPLUGIN-DEPENDENCY"),
        (
            "dependencies",
            json!({"other":"2.0.0"}),
            "HGPLUGIN-DEPENDENCY",
        ),
        ("presets", json!("../escape"), "HGPLUGIN-PATH"),
        ("presets", json!("absent"), "HGPLUGIN-MISSING"),
        ("files", json!({"../escape":"abc"}), "HGPLUGIN-PATH"),
        ("files", json!({}), "HGPLUGIN-LIMIT"),
    ] {
        let mut f = Fixture::new();
        f.manifest[field] = value;
        f.seal();
        let out = f.check();
        failure(&out, code);
        let report: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(report["valid"], false);
        assert!(!report["diagnostics"][0]["help"]
            .as_str()
            .unwrap()
            .is_empty());
        assert!(!f.root().join(".harness-gate/flow.toml").exists());
    }
}

#[test]
fn lock_and_signature_never_grant_project_trust_or_silently_upgrade() {
    let mut f = Fixture::new();
    f.lock["trusted_keys"] = json!(["project-self-approved"]);
    f.save_lock();
    failure(&f.check(), "HGPLUGIN-LOCK");
    f.lock.as_object_mut().unwrap().remove("trusted_keys");
    f.lock["plugins"][0]["version"] = json!("latest");
    f.save_lock();
    failure(&f.check(), "HGPLUGIN-VERSION");
    f.lock["plugins"][0]["version"] = json!("1.2.3");
    f.lock["plugins"][0]["signer"] = json!("project-self-approved");
    f.save_lock();
    failure(&f.check(), "HGPLUGIN-SIGNATURE");
    f.lock["plugins"][0]["signer"] = json!("operator");
    let pin = f.lock["plugins"][0].clone();
    f.lock["plugins"].as_array_mut().unwrap().push(pin);
    f.save_lock();
    failure(&f.check(), "HGPLUGIN-LOCK");
    f.lock["plugins"].as_array_mut().unwrap().pop();
    f.save_lock();
    write(
        &f.temp
            .path()
            .join("store/unknown-ecosystem/1.2.3/plugin.sig"),
        br#"{"key_id":"operator","signature":"corrupt"}"#,
    );
    failure(&f.check(), "HGPLUGIN-SIGNATURE");
    f.seal();
    write(
        &f.temp
            .path()
            .join("store/unknown-ecosystem/1.2.3/plugin.json"),
        b"tampered",
    );
    failure(&f.check(), "HGPLUGIN-DIGEST");
    f.seal();
    write(
        &f.temp
            .path()
            .join("store/unknown-ecosystem/1.2.3/presets/flow.toml"),
        b"tampered",
    );
    failure(&f.check(), "HGPLUGIN-DIGEST");
    f.seal();
    let output = Command::new(env!("CARGO_BIN_EXE_harness-gate"))
        .args([
            "--project-root",
            f.root().to_str().unwrap(),
            "init",
            "--plugin",
            "unknown-ecosystem",
            "--preset",
            "strange-ecosystem",
        ])
        .output()
        .unwrap();
    failure(&output, "HGPLUGIN-STORE");
    assert!(!f.root().join(".harness-gate/flow.toml").exists());
}

#[test]
fn missing_and_unsafe_inputs_fail_without_executing_or_fetching() {
    let f = Fixture::new();
    fs::remove_file(f.temp.path().join("operator.json")).unwrap();
    failure(&f.check(), "HGPLUGIN-SIGNATURE");
    let f = Fixture::new();
    fs::remove_file(
        f.temp
            .path()
            .join("store/unknown-ecosystem/1.2.3/presets/catalog.json"),
    )
    .unwrap();
    failure(&f.check(), "HGPLUGIN-MISSING");
    let f = Fixture::new();
    write(
        &f.temp
            .path()
            .join("store/unknown-ecosystem/1.2.3/plugin.sig"),
        b"{}",
    );
    failure(&f.check(), "HGPLUGIN-SIGNATURE");
    let f = Fixture::new();
    write(
        &f.root().join(".harness-gate/plugins.lock.json"),
        br#"{"schema":"bad","schema":"harness-plugin-lock/v1","plugins":[]}"#,
    );
    failure(&f.check(), "HGPLUGIN-LOCK");
}

#[cfg(unix)]
#[test]
fn signed_symlink_and_nonregular_inputs_are_rejected() {
    let f = Fixture::new();
    let path = f
        .temp
        .path()
        .join("store/unknown-ecosystem/1.2.3/presets/flow.toml");
    fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink("catalog.json", &path).unwrap();
    failure(&f.check(), "HGPLUGIN-PATH");
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    failure(&f.check(), "HGPLUGIN-PATH");
}

#[test]
fn invalid_signature_blocks_execution() {
    let f = Fixture::new();
    f.init();
    let path = f
        .temp
        .path()
        .join("store/unknown-ecosystem/1.2.3/plugin.sig");
    let mut attestation: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let mut bytes = BASE64
        .decode(attestation["signature"].as_str().unwrap())
        .unwrap();
    bytes[0] ^= 1;
    attestation["signature"] = json!(BASE64.encode(bytes));
    write(&path, serde_json::to_vec(&attestation).unwrap());
    let output = f.run(&["step", "project.diff-check"]);
    failure(&output, "HGPLUGIN-SIGNATURE");
    assert!(!output.status.success());
    assert!(!f.root().join(".harness-gate/reports/invocations").exists());
    failure(
        &f.run(&["config", "check", "--format", "json"]),
        "HGPLUGIN-SIGNATURE",
    );
    failure(&f.run(&["doctor", "--json"]), "HGPLUGIN-SIGNATURE");
}

#[test]
fn malformed_keys_limits_catalogs_and_operator_selection_are_diagnosed() {
    let mut f = Fixture::new();
    for bytes in [
        b"not JSON".as_slice(),
        br#"{"key_id":"operator","public_key":"!!!!"}"#,
        br#"{"key_id":"operator","public_key":"YQ=="}"#,
    ] {
        write(&f.temp.path().join("operator.json"), bytes);
        failure(&f.check(), "HGPLUGIN-SIGNATURE");
    }
    let f2 = Fixture::new();
    write(
        &f.temp.path().join("operator.json"),
        fs::read(f2.temp.path().join("operator.json")).unwrap(),
    );
    failure(
        &f.run(&[
            "--plugin-trusted-key",
            f.temp.path().join("operator.json").to_str().unwrap(),
            "plugins",
            "--lock",
            f.root()
                .join(".harness-gate/plugins.lock.json")
                .to_str()
                .unwrap(),
            "--json",
        ]),
        "HGPLUGIN-SIGNATURE",
    );
    failure(
        &f.run(&[
            "init",
            "--plugin",
            "not-selected",
            "--preset",
            "strange-ecosystem",
        ]),
        "HGPLUGIN-SELECTION",
    );
    failure(
        &f.run(&["init", "--preset", "strange-ecosystem"]),
        "HGPLUGIN-SELECTION",
    );
    success(&f.run(&["init", "--preset", "generic"]));
    f.lock["schema"] = json!("harness-plugin-lock/v99");
    f.save_lock();
    failure(&f.check(), "HGPLUGIN-LOCK");
    f.lock["schema"] = json!("harness-plugin-lock/v1");
    f.lock["plugins"] = json!([]);
    f.save_lock();
    failure(&f.check(), "HGPLUGIN-LOCK");
    let mut f = Fixture::new();
    f.manifest["files"]
        .as_object_mut()
        .unwrap()
        .remove("presets/flow.toml");
    f.seal();
    failure(&f.check(), "HGPLUGIN-CATALOG");
    let f = Fixture::new();
    write(
        &f.temp
            .path()
            .join("store/unknown-ecosystem/1.2.3/presets/flow.toml"),
        vec![b'a'; 1024 * 1024 + 1],
    );
    failure(&f.check(), "HGPLUGIN-LIMIT");
    let f = Fixture::new();
    fs::rename(
        f.temp.path().join("store"),
        f.temp.path().join("moved-store"),
    )
    .unwrap();
    failure(&f.check(), "HGPLUGIN-STORE");
    let f = Fixture::new();
    fs::remove_file(f.root().join(".harness-gate/plugins.lock.json")).unwrap();
    failure(
        &f.run(&["init", "--plugin", "unknown-ecosystem"]),
        "HGPLUGIN-LOCK",
    );
}
