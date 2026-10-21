//! Protected local input loader for the Linux v2 read-only host.
//! Local euid/root-controlled ancestors are a trust prerequisite, not an OS
//! isolation guarantee. Filesystem reads have no hard deadline. No ambient pins.
use gatehouse_runtime_protocol::v2::Response;
use std::path::Path;

#[cfg(target_os = "linux")]
pub(crate) fn run_files(actual: &Path, host: &Path) -> anyhow::Result<Response> {
    linux::run(actual, host).map_err(anyhow::Error::msg)
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(crate) fn run_files(actual: &Path, host: &Path) -> anyhow::Result<Response> {
    let _ = (actual, host);
    anyhow::bail!("runtime v2 is supported only on Linux");
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn run_files(actual: &Path, host: &Path) -> anyhow::Result<Response> {
    let _ = (actual, host);
    anyhow::bail!("runtime v2 is supported only on Linux");
}

/// Installed entry is separate: no optional descriptor or downgrade to v2.
#[cfg(target_os = "linux")]
pub(crate) fn run_installed(actual: &Path, host: &Path) -> anyhow::Result<Response> {
    linux::run_mode(actual, host, true).map_err(anyhow::Error::msg)
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(crate) fn run_installed(actual: &Path, host: &Path) -> anyhow::Result<Response> {
    let _ = (actual, host);
    anyhow::bail!("installed runtime is supported only on Linux");
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn run_installed(actual: &Path, host: &Path) -> anyhow::Result<Response> {
    let _ = (actual, host);
    anyhow::bail!("installed runtime is supported only on Linux");
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use crate::process::{
        runtime_host_v2::{run_readonly, ReadonlyHostInput},
        signal,
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
    const ERROR: &str = "runtime protected input rejected";
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
        executable: Executable,
        config: FilePin,
        expected: FilePin,
        cwd: PathBuf,
        timeout_ms: u64,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct InstalledHost {
        schema: String,
        installation: FilePin,
        executable: Executable,
        config: FilePin,
        expected: FilePin,
        cwd: PathBuf,
        timeout_ms: u64,
    }
    /// Provenance is operator-supplied audit data, not publisher authentication.
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
    fn hex(value: &str, size: usize) -> bool {
        value.len() == size
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }
    fn audit_text(value: &str) -> bool {
        !value.trim().is_empty() && !value.chars().any(char::is_control)
    }
    fn installed_host(bytes: &[u8]) -> Result<(Host, Option<Held>)> {
        let installed: InstalledHost = serde_json::from_slice(bytes).map_err(|_| ERROR)?;
        if installed.schema != "gate-runtime-host/installed-v1" {
            return Err(ERROR);
        }
        let held = Held::pinned(&installed.installation, "gate-runtime-installation/v1")?;
        let descriptor: Installation = serde_json::from_slice(&held.bytes).map_err(|_| ERROR)?;
        validate_installation(&descriptor, &installed.executable)?;
        Ok((
            Host {
                schema: installed.schema,
                executable: installed.executable,
                config: installed.config,
                expected: installed.expected,
                cwd: installed.cwd,
                timeout_ms: installed.timeout_ms,
            },
            Some(held),
        ))
    }
    fn validate_installation(descriptor: &Installation, executable: &Executable) -> Result<()> {
        if descriptor.schema != "gate-runtime-installation/v1"
            || !audit_text(&descriptor.repository)
            || !audit_text(&descriptor.tag)
            || !hex(&descriptor.source_commit, 40)
            || !hex(&descriptor.archive_sha256, 64)
            || descriptor.executable != *executable
        {
            return Err(ERROR);
        }
        protected_path(&executable.path)
    }
    fn load_host(bytes: &[u8], installed: bool) -> Result<(Host, Option<Held>)> {
        if installed {
            return installed_host(bytes);
        }
        let host: Host = serde_json::from_slice(bytes).map_err(|_| ERROR)?;
        if host.schema != "gate-runtime-host/v2" {
            return Err(ERROR);
        }
        Ok((host, None))
    }
    fn revalidate_installation(installation: &Option<Held>) -> Result<()> {
        if let Some(held) = installation {
            held.revalidate()?;
        }
        Ok(())
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
    pub(super) fn run(actual: &Path, host_path: &Path) -> Result<Response> {
        run_mode(actual, host_path, false)
    }
    pub(super) fn run_mode(actual: &Path, host_path: &Path, installed: bool) -> Result<Response> {
        // Actual input is untrusted. No ownership requirement or pin is taken
        // from it. Trusted files stay held and are rechecked around execution.
        let host_file = Held::open(host_path)?;
        let (host, installation) = load_host(&host_file.bytes, installed)?;
        let config = Held::pinned(&host.config, "gate-runtime-engine-config/v2")?;
        let expected = Held::pinned(&host.expected, "gate-runtime/v2")?;
        let request =
            gatehouse_runtime_protocol::v2::parse_request(&expected.bytes).map_err(|_| ERROR)?;
        let actual = read(&open(actual)?)?;
        host_file.revalidate()?;
        revalidate_installation(&installation)?;
        config.revalidate()?;
        expected.revalidate()?;
        let response = run_readonly(
            ReadonlyHostInput {
                expected: request,
                config: config.bytes.clone(),
                actual,
                executable: host.executable.path,
                executable_sha256: host.executable.sha256,
                executable_version: host.executable.version,
                cwd: host.cwd,
                timeout_ms: host.timeout_ms,
            },
            signal::cancelled,
        )?;
        host_file.revalidate()?;
        revalidate_installation(&installation)?;
        config.revalidate()?;
        expected.revalidate()?;
        if signal::cancelled() {
            return Err("runtime cancelled");
        }
        Ok(response)
    }
}
