//! Protected v3 installation loader. Expected requests exist only as fresh sealed
//! per-invocation bytes; no host field or CLI accepts an expected/request path.
use gatehouse_runtime_protocol::v3::Response;
use std::path::Path;
#[cfg(target_os = "linux")]
pub(crate) fn run_installed(host: &Path, image: &str, timeout_ms: u64) -> anyhow::Result<Response> {
    linux::run(host, image, timeout_ms).map_err(anyhow::Error::msg)
}
#[cfg(not(target_os = "linux"))]
pub(crate) fn run_installed(_: &Path, _: &str, _: u64) -> anyhow::Result<Response> {
    anyhow::bail!("v3 runtime requires Linux")
}
#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use crate::process::{
        runtime_host_v3::{run_readonly, ReadonlyHostInput},
        runtime_request_v3, signal,
    };
    use serde::Deserialize;
    use sha2::{Digest, Sha256};
    use std::{
        fs::{self, File, Metadata, OpenOptions},
        io::Read,
        os::unix::fs::{MetadataExt, OpenOptionsExt},
        path::PathBuf,
    };
    type Result<T> = std::result::Result<T, &'static str>;
    const ERROR: &str = "runtime protected v3 input rejected";
    const LIMIT: u64 = 65_536;
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct FilePin {
        path: PathBuf,
        sha256: String,
        schema: String,
    }
    #[derive(Deserialize, PartialEq, Eq)]
    #[serde(deny_unknown_fields)]
    struct Executable {
        path: PathBuf,
        sha256: String,
        version: String,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Host {
        schema: String,
        installation: FilePin,
        executable: Executable,
        config: FilePin,
        cwd: PathBuf,
        timeout_ms: u64,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Installation {
        schema: String,
        repository: String,
        tag: String,
        source_commit: String,
        archive_sha256: String,
        executable: Executable,
    }
    fn hex(s: &str, len: usize) -> bool {
        s.len() == len
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }
    fn audit(s: &str) -> bool {
        !s.trim().is_empty() && !s.chars().any(char::is_control)
    }
    #[derive(PartialEq, Eq)]
    struct Identity {
        dev: u64,
        ino: u64,
        mode: u32,
        uid: u32,
        gid: u32,
        links: u64,
        len: u64,
        mtime: i64,
        mtime_ns: i64,
        ctime: i64,
        ctime_ns: i64,
    }
    impl Identity {
        fn of(m: &Metadata) -> Self {
            Self {
                dev: m.dev(),
                ino: m.ino(),
                mode: m.mode(),
                uid: m.uid(),
                gid: m.gid(),
                links: m.nlink(),
                len: m.len(),
                mtime: m.mtime(),
                mtime_ns: m.mtime_nsec(),
                ctime: m.ctime(),
                ctime_ns: m.ctime_nsec(),
            }
        }
    }
    fn protected_path(path: &Path) -> Result<()> {
        if !path.is_absolute() || fs::canonicalize(path).map_err(|_| ERROR)? != path {
            return Err(ERROR);
        }
        for parent in path.ancestors().skip(1) {
            let m = fs::symlink_metadata(parent).map_err(|_| ERROR)?;
            if !m.is_dir()
                || ![0, unsafe { libc::geteuid() }].contains(&m.uid())
                || m.mode() & 0o022 != 0
            {
                return Err(ERROR);
            }
        }
        Ok(())
    }
    struct Held {
        path: PathBuf,
        file: File,
        identity: Identity,
        bytes: Vec<u8>,
    }
    impl Held {
        fn executable(path: &Path) -> Result<Self> {
            protected_path(path)?;
            let file = open(path)?;
            let m = file.metadata().map_err(|_| ERROR)?;
            if !m.is_file()
                || m.uid() != unsafe { libc::geteuid() }
                || m.nlink() != 1
                || !matches!(m.mode() & 0o7777, 0o500 | 0o700 | 0o555 | 0o755)
            {
                return Err(ERROR);
            }
            let held = Self {
                path: path.to_owned(),
                file,
                identity: Identity::of(&m),
                bytes: Vec::new(),
            };
            held.revalidate()?;
            Ok(held)
        }
        fn open(path: &Path) -> Result<Self> {
            protected_path(path)?;
            let file = open(path)?;
            let m = file.metadata().map_err(|_| ERROR)?;
            if !m.is_file()
                || m.uid() != unsafe { libc::geteuid() }
                || m.nlink() != 1
                || !matches!(m.mode() & 0o7777, 0o400 | 0o600)
            {
                return Err(ERROR);
            }
            let bytes = read(&file)?;
            let held = Self {
                path: path.to_owned(),
                file,
                identity: Identity::of(&m),
                bytes,
            };
            held.revalidate()?;
            Ok(held)
        }
        fn revalidate(&self) -> Result<()> {
            protected_path(&self.path)?;
            let held = self.file.metadata().map_err(|_| ERROR)?;
            let named = fs::symlink_metadata(&self.path).map_err(|_| ERROR)?;
            if !named.is_file()
                || Identity::of(&held) != self.identity
                || Identity::of(&named) != self.identity
            {
                return Err(ERROR);
            }
            Ok(())
        }
        fn pinned(pin: &FilePin, schema: &str) -> Result<Self> {
            if pin.schema != schema {
                return Err(ERROR);
            }
            let held = Self::open(&pin.path)?;
            if format!("{:x}", Sha256::digest(&held.bytes)) != pin.sha256 {
                return Err(ERROR);
            }
            Ok(held)
        }
    }
    fn open(path: &Path) -> Result<File> {
        OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(path)
            .map_err(|_| ERROR)
    }
    fn read(file: &File) -> Result<Vec<u8>> {
        if !file.metadata().map_err(|_| ERROR)?.is_file() {
            return Err(ERROR);
        }
        let mut bytes = Vec::new();
        file.take(LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| ERROR)?;
        if bytes.len() as u64 > LIMIT {
            return Err(ERROR);
        }
        Ok(bytes)
    }
    pub(super) fn run(host_path: &Path, image: &str, timeout_ms: u64) -> Result<Response> {
        if !(1..=30_000).contains(&timeout_ms) {
            return Err(ERROR);
        }
        let started = std::time::Instant::now();
        let host_file = Held::open(host_path)?;
        let host: Host = serde_json::from_slice(&host_file.bytes).map_err(|_| ERROR)?;
        if host.schema != "gate-runtime-host/installed-v2"
            || !(1..=30_000).contains(&host.timeout_ms)
        {
            return Err(ERROR);
        }
        let installation = Held::pinned(&host.installation, "gate-runtime-installation/v1")?;
        let descriptor: Installation =
            serde_json::from_slice(&installation.bytes).map_err(|_| ERROR)?;
        if descriptor.schema != "gate-runtime-installation/v1"
            || !audit(&descriptor.repository)
            || !audit(&descriptor.tag)
            || !hex(&descriptor.source_commit, 40)
            || !hex(&descriptor.archive_sha256, 64)
            || descriptor.executable != host.executable
        {
            return Err(ERROR);
        }
        let executable = Held::executable(&host.executable.path)?;
        let config = Held::pinned(&host.config, "gate-runtime-engine-config/v3")?;
        let expected = runtime_request_v3::mint(
            &config.bytes,
            &host.executable.sha256,
            &host.executable.version,
            image,
        )?;
        let actual = serde_json::to_vec(&expected).map_err(|_| ERROR)?;
        host_file.revalidate()?;
        installation.revalidate()?;
        executable.revalidate()?;
        config.revalidate()?;
        let remaining = std::time::Duration::from_millis(timeout_ms.min(host.timeout_ms))
            .checked_sub(started.elapsed())
            .ok_or("runtime readiness timed out")?;
        let remaining_ms = remaining.as_millis() as u64;
        if remaining_ms == 0 {
            return Err("runtime readiness timed out");
        }
        let response = run_readonly(
            ReadonlyHostInput {
                expected,
                config: config.bytes.clone(),
                actual,
                executable: host.executable.path,
                executable_sha256: host.executable.sha256,
                executable_version: host.executable.version,
                cwd: host.cwd,
                timeout_ms: remaining_ms,
            },
            signal::cancelled,
        )?;
        host_file.revalidate()?;
        installation.revalidate()?;
        executable.revalidate()?;
        config.revalidate()?;
        if signal::cancelled() {
            return Err("runtime cancelled");
        }
        Ok(response)
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};
    use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, process::Command};
    const HELPER: &str = r#"
#include <stdio.h>
#include <string.h>
int main(void) {
 char mode[32]={0}; FILE *f=fopen("mode","r"); if(f){fgets(mode,sizeof mode,f);fclose(f);}
 char req[65540]; if(!fgets(req,sizeof req,stdin)) return 2;
 f=fopen("request-observed","w"); fputs(req,f); fclose(f);
 if(!strcmp(mode,"replay")){f=fopen("last-response","r");if(!f)return 3;int c;while((c=fgetc(f))!=EOF)putchar(c);fclose(f);return 0;}
 size_t n=strlen(req);while(n && (req[n-1]=='\n'||req[n-1]=='\r'))n--;if(!n||req[n-1]!='}')return 4;req[--n]=0;
 const char *tail=",\"outcome\":{\"readiness\":{\"image_present\":true,\"image_id\":\"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}},\"observations\":{\"observed_before\":{\"api_version\":\"1.47\",\"server_version\":\"27.0.1\",\"daemon_id\":\"daemon-1\"},\"observed_after\":{\"api_version\":\"1.47\",\"server_version\":\"27.0.1\",\"daemon_id\":\"daemon-1\"}}}";
 f=fopen("last-response","w");fputs(req,f);fputs(tail,f);fclose(f);fputs(req,stdout);fputs(tail,stdout);return 0;
}
"#;
    fn hash(b: &[u8]) -> String {
        format!("{:x}", Sha256::digest(b))
    }
    struct Fixture {
        root: tempfile::TempDir,
        host: PathBuf,
        config: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let base =
                PathBuf::from(std::env::var_os("HOME").expect("protected HOME")).join(".cache");
            let root = tempfile::Builder::new()
                .prefix("core-v3-host-")
                .tempdir_in(base)
                .unwrap();
            let dir = fs::canonicalize(root.path()).unwrap();
            let source = dir.join("helper.c");
            let executable = dir.join("helper");
            fs::write(&source, HELPER).unwrap();
            let output = Command::new("cc")
                .args(["-O0", "-o"])
                .arg(&executable)
                .arg(&source)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
            let exe = json!({"path":executable,"sha256":hash(&fs::read(&executable).unwrap()),"version":"0.2.0"});
            let config = dir.join("config.json");
            let cfg = json!({"schema":"gate-runtime-engine-config/v3","engine":"docker","transport":"unix-http","socket_path":"/run/fixture.sock","socket_uid":1000,"principal_uid":1000,"rootless":false,"api_version":"1.47","server_version":"27.0.1","daemon_id":"daemon-1","scope":"test"});
            write(&config, &cfg);
            let installation = dir.join("installation.json");
            write(
                &installation,
                &json!({"schema":"gate-runtime-installation/v1","repository":"fixture","tag":"unreleased","source_commit":"a".repeat(40),"archive_sha256":"b".repeat(64),"executable":exe}),
            );
            let host = dir.join("host.json");
            write(
                &host,
                &json!({"schema":"gate-runtime-host/installed-v2","installation":{"path":installation,"sha256":hash(&fs::read(&installation).unwrap()),"schema":"gate-runtime-installation/v1"},"executable":exe,"config":{"path":config,"sha256":hash(&fs::read(&config).unwrap()),"schema":"gate-runtime-engine-config/v3"},"cwd":dir,"timeout_ms":2000}),
            );
            Self { root, host, config }
        }
        fn run(&self) -> anyhow::Result<Response> {
            run_installed(&self.host, "registry:5000/repo:tag", 2000)
        }
    }
    fn write(path: &Path, v: &Value) {
        fs::write(path, serde_json::to_vec(v).unwrap()).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    #[test]
    fn fresh_expected_is_sealed_per_invocation_and_replay_is_refused() {
        let f = Fixture::new();
        let first = f.run().unwrap();
        let second = f.run().unwrap();
        assert_ne!(first.binding.invocation, second.binding.invocation);
        assert_eq!(first.binding.generation, second.binding.generation);
        fs::write(f.root.path().join("mode"), "replay").unwrap();
        assert!(f.run().is_err());
    }
    #[test]
    fn fixed_expected_schema_downgrade_and_host_pin_tampering_fail_before_spawn() {
        for field in ["expected", "invocation", "generation", "operation_key"] {
            let f = Fixture::new();
            let mut host: Value = serde_json::from_slice(&fs::read(&f.host).unwrap()).unwrap();
            host[field] = json!("untrusted");
            write(&f.host, &host);
            assert!(f.run().is_err());
            assert!(!f.root.path().join("request-observed").exists());
        }
        let f = Fixture::new();
        fs::write(&f.config, b"changed").unwrap();
        assert!(f.run().is_err());
        assert!(!f.root.path().join("request-observed").exists());
        let f = Fixture::new();
        let mut host: Value = serde_json::from_slice(&fs::read(&f.host).unwrap()).unwrap();
        host["schema"] = json!("gate-runtime-host/installed-v1");
        write(&f.host, &host);
        assert!(f.run().is_err());
    }
    #[test]
    fn unsafe_host_mode_and_invalid_image_are_rejected() {
        let f = Fixture::new();
        fs::set_permissions(&f.host, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(f.run().is_err());
        let f = Fixture::new();
        assert!(run_installed(&f.host, "bad?image", 2000).is_err());
        assert!(!f.root.path().join("request-observed").exists());
    }
}
