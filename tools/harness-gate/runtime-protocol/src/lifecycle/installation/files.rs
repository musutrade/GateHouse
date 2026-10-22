use super::*;
use sha2::{Digest, Sha256};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
};

fn io<T>(r: std::io::Result<T>) -> Result<T> {
    r.map_err(|_| Invalid("protected lifecycle installation rejected"))
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
    fn of(m: &fs::Metadata) -> Self {
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
fn protected(path: &Path) -> Result<()> {
    if !path.is_absolute() || io(fs::canonicalize(path))? != path {
        return Err(Invalid(
            "lifecycle installation path must be protected absolute",
        ));
    }
    for parent in path.ancestors().skip(1) {
        let m = io(fs::symlink_metadata(parent))?;
        // SAFETY: geteuid takes no arguments.
        if !m.is_dir()
            || m.mode() & 0o022 != 0
            || ![0, unsafe { libc::geteuid() }].contains(&m.uid())
        {
            return Err(Invalid("lifecycle installation ancestor is unprotected"));
        }
    }
    Ok(())
}
struct Held {
    path: PathBuf,
    file: File,
    identity: Identity,
    bytes: Vec<u8>,
    sha256: String,
}
impl Held {
    fn open(path: &Path, executable: bool) -> Result<Self> {
        protected(path)?;
        let file = io(OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(path))?;
        let m = io(file.metadata())?;
        let mode = m.mode() & 0o7777;
        let permitted = if executable {
            matches!(mode, 0o500 | 0o700 | 0o555 | 0o755)
        } else {
            matches!(mode, 0o400 | 0o600)
        };
        // SAFETY: geteuid takes no arguments.
        if !m.is_file() || m.nlink() != 1 || m.uid() != unsafe { libc::geteuid() } || !permitted {
            return Err(Invalid("invalid protected lifecycle file"));
        }
        let max = if executable {
            64 * 1024 * 1024
        } else {
            crate::MAX_MESSAGE_BYTES as u64
        };
        if m.len() > max {
            return Err(Invalid("lifecycle installation file too large"));
        }
        let mut bytes = vec![];
        io((&file).take(max + 1).read_to_end(&mut bytes))?;
        if bytes.len() as u64 > max {
            return Err(Invalid("lifecycle installation file too large"));
        }
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        let held = Self {
            path: path.into(),
            file,
            identity: Identity::of(&m),
            bytes: if executable { vec![] } else { bytes },
            sha256,
        };
        held.revalidate()?;
        Ok(held)
    }
    fn revalidate(&self) -> Result<()> {
        protected(&self.path)?;
        if Identity::of(&io(self.file.metadata())?) != self.identity
            || Identity::of(&io(fs::symlink_metadata(&self.path))?) != self.identity
        {
            return Err(Invalid("lifecycle installation changed"));
        }
        Ok(())
    }
}
/// A held file snapshot, not a runnable host or mutation capability.
pub struct VerifiedInstallation {
    host: Held,
    installation: Held,
    config: Held,
    executable: Held,
    engine: EngineBinding,
}
impl VerifiedInstallation {
    pub fn load(host_path: &Path) -> Result<Self> {
        let host = Held::open(host_path, false)?;
        let h: Host = parse(&host.bytes)?;
        if h.schema != "gate-runtime/lifecycle-host-v1" {
            return Err(Invalid("unsupported lifecycle host"));
        }
        digest(&h.installation_sha256)?;
        digest(&h.config_sha256)?;
        let installation = Held::open(Path::new(&h.installation_path), false)?;
        if installation.sha256 != h.installation_sha256 {
            return Err(Invalid("lifecycle installation digest mismatch"));
        }
        let i: Installation = parse(&installation.bytes)?;
        if i.schema != "gate-runtime/lifecycle-installation-v1"
            || i.product != "docker-lifecycle"
            || i.version != "0.1.0"
            || i.protocol != PROTOCOL
            || i.config_schema != "gate-runtime/lifecycle-config-v1"
        {
            return Err(Invalid("unsupported lifecycle installation"));
        }
        digest(&i.executable_sha256)?;
        let config = Held::open(Path::new(&h.config_path), false)?;
        if config.sha256 != h.config_sha256 {
            return Err(Invalid("lifecycle config digest mismatch"));
        }
        let c: Config = parse(&config.bytes)?;
        if c.schema != i.config_schema {
            return Err(Invalid("unsupported lifecycle config"));
        }
        c.engine_binding.validate()?;
        if c.engine_binding.principal_uid != unsafe { libc::geteuid() } {
            return Err(Invalid("lifecycle principal mismatch"));
        }
        let executable = Held::open(Path::new(&i.executable_path), true)?;
        if executable.sha256 != i.executable_sha256 {
            return Err(Invalid("lifecycle executable digest mismatch"));
        }
        let verified = Self {
            host,
            installation,
            config,
            executable,
            engine: c.engine_binding,
        };
        verified.revalidate()?;
        Ok(verified)
    }
    pub fn revalidate(&self) -> Result<()> {
        for held in [
            &self.host,
            &self.installation,
            &self.config,
            &self.executable,
        ] {
            held.revalidate()?;
        }
        Ok(())
    }
    /// Binds an already journal-authorized request to this exact installed
    /// snapshot; does not infer a request, endpoint or identity from environment.
    pub fn validate_request(&self, r: &Request) -> Result<()> {
        r.validate()?;
        self.revalidate()?;
        // SAFETY: geteuid takes no arguments.
        if r.binding.installation_sha256 != self.installation.sha256
            || r.binding.executable_sha256 != self.executable.sha256
            || r.binding.config_sha256 != self.config.sha256
            || r.binding.engine_binding != self.engine
            || r.binding.engine_binding.principal_uid != unsafe { libc::geteuid() }
        {
            return Err(Invalid("request does not match protected installation"));
        }
        Ok(())
    }
    /// Future host must execute this held descriptor, not reopen an ambient path.
    /// Reading this descriptor is not permission to bypass journal authorization.
    pub fn config_bytes(&self) -> Result<&[u8]> {
        self.revalidate()?;
        Ok(&self.config.bytes)
    }
    pub fn engine_binding(&self) -> Result<&EngineBinding> {
        self.revalidate()?;
        Ok(&self.engine)
    }
    pub fn executable(&self) -> Result<&File> {
        self.revalidate()?;
        Ok(&self.executable.file)
    }
}
#[cfg(test)]
mod tests;
