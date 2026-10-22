//! Held installation bytes, not executable trust, dispatch or lease authority.
use super::{
    plan_record::PlanRecord,
    profile::{Profile, SlotIdentity, PRODUCT, PROTOCOL, VERSION},
    protected_context::{protected, Identity, ProtectedContext, CONTEXT_SCHEMA},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    os::unix::fs::{FileExt, MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};
type Result<T> = std::result::Result<T, &'static str>;
fn io<T>(r: std::io::Result<T>) -> Result<T> {
    r.map_err(|_| "installation filesystem failure")
}
fn digest(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Host {
    schema: String,
    installation_path: String,
    installation_sha256: String,
    config_path: String,
    config_sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Installation {
    schema: String,
    product: String,
    version: String,
    protocol: String,
    executable_path: String,
    executable_sha256: String,
    config_schema: String,
}
fn parse<T: serde::de::DeserializeOwned + Serialize>(bytes: &[u8]) -> Result<T> {
    // Typed duplicate checks + exact struct-order roundtrip; no unordered maps.
    let value: T = serde_json::from_slice(bytes).map_err(|_| "installation fields rejected")?;
    if serde_json::to_vec(&value).map_err(|_| "installation encode")? != bytes {
        return Err("noncanonical installation");
    }
    Ok(value)
}
struct Snapshot {
    path: PathBuf,
    file: File,
    identity: Identity,
    bytes: Vec<u8>,
}
impl Snapshot {
    fn open(path: &Path, executable: bool) -> Result<Self> {
        protected(path)?;
        let file = io(OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path))?;
        let m = io(file.metadata())?;
        let uid = unsafe { libc::geteuid() };
        let mode = m.mode() & 0o7777;
        let mode_ok = if executable {
            if m.uid() == 0 {
                matches!(mode, 0o555 | 0o755)
            } else {
                m.uid() == uid && matches!(mode, 0o500 | 0o700)
            }
        } else {
            m.uid() == uid && matches!(mode, 0o400 | 0o600)
        };
        let limit = if executable {
            128 * 1024 * 1024
        } else {
            1024 * 1024
        };
        if !m.is_file() || m.nlink() != 1 || !mode_ok || m.len() > limit {
            return Err("unprotected installation member");
        }
        let bytes = read(&file, limit)?;
        if executable && !bytes.starts_with(b"\x7fELF") {
            return Err("ELF executable required");
        }
        let held = Self {
            path: path.into(),
            file,
            identity: Identity::of(&m),
            bytes,
        };
        held.check()?;
        Ok(held)
    }
    fn check(&self) -> Result<()> {
        protected(&self.path)?;
        if Identity::of(&io(self.file.metadata())?) != self.identity
            || Identity::of(&io(fs::symlink_metadata(&self.path))?) != self.identity
        {
            return Err("installation member changed");
        }
        if read(&self.file, self.bytes.len() as u64)? != self.bytes {
            return Err("installation bytes changed");
        }
        if Identity::of(&io(self.file.metadata())?) != self.identity
            || Identity::of(&io(fs::symlink_metadata(&self.path))?) != self.identity
        {
            return Err("installation member changed during read");
        }
        Ok(())
    }
}
fn read(file: &File, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut buf = [0; 8192];
    while bytes.len() as u64 <= limit {
        let n = io(file.read_at(&mut buf, bytes.len() as u64))?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&buf[..n]);
    }
    if bytes.len() as u64 > limit {
        return Err("installation byte limit");
    }
    Ok(bytes)
}
/// Holds every verified file; fields cannot be supplied by a caller or disk record.
pub struct VerifiedPodmanInstallation {
    host: Snapshot,
    installation: Snapshot,
    config: Snapshot,
    executable: Snapshot,
    context: ProtectedContext,
    identity: SlotIdentity,
}
impl VerifiedPodmanInstallation {
    pub fn load(host_path: &Path) -> Result<Self> {
        let host = Snapshot::open(host_path, false)?;
        let h: Host = parse(&host.bytes)?;
        if h.schema != "gate-runtime/podman-host-v1" {
            return Err("Podman host schema required");
        }
        let installation = Snapshot::open(Path::new(&h.installation_path), false)?;
        if digest(&installation.bytes) != h.installation_sha256 {
            return Err("installation digest mismatch");
        }
        let i: Installation = parse(&installation.bytes)?;
        if i.schema != "gate-runtime/podman-installation-v1"
            || i.product != PRODUCT
            || i.version != VERSION
            || i.protocol != PROTOCOL
            || i.config_schema != CONTEXT_SCHEMA
        {
            return Err("Podman installation tuple required");
        }
        let config = Snapshot::open(Path::new(&h.config_path), false)?;
        if digest(&config.bytes) != h.config_sha256 {
            return Err("config digest mismatch");
        }
        let context =
            ProtectedContext::from_held_versioned(&config.path, io(config.file.try_clone())?)?;
        if context.snapshot()?.1 != config.bytes {
            return Err("held config snapshot mismatch");
        }
        let executable = Snapshot::open(Path::new(&i.executable_path), true)?;
        if digest(&executable.bytes) != i.executable_sha256 {
            return Err("executable digest mismatch");
        }
        let identity = SlotIdentity {
            profile: Profile {
                protocol: PROTOCOL.into(),
                product: PRODUCT.into(),
                version: VERSION.into(),
                engine: "podman".into(),
            },
            installation_sha256: digest(&installation.bytes),
            config_sha256: digest(&config.bytes),
            executable_sha256: digest(&executable.bytes),
        };
        identity.validate()?;
        let result = Self {
            host,
            installation,
            config,
            executable,
            context,
            identity,
        };
        result.revalidate()?;
        Ok(result)
    }
    pub fn revalidate(&self) -> Result<()> {
        self.host.check()?;
        self.installation.check()?;
        self.config.check()?;
        self.executable.check()?;
        self.context.revalidate()
    }
    pub fn context(&self) -> Result<&ProtectedContext> {
        self.revalidate()?;
        Ok(&self.context)
    }
    pub fn identity(&self) -> Result<&SlotIdentity> {
        self.revalidate()?;
        Ok(&self.identity)
    }
    /// Borrow for future held-FD execution; this does not spawn or prove entrypoint ABI.
    pub fn executable_file(&self) -> Result<&File> {
        self.revalidate()?;
        Ok(&self.executable.file)
    }
    pub fn match_record<'a>(
        &'a self,
        record: &'a PlanRecord,
    ) -> Result<RecordInstallationMatch<'a>> {
        let matched = RecordInstallationMatch {
            installation: self,
            record,
        };
        matched.revalidate()?;
        Ok(matched)
    }
}
/// Live borrowed comparison only. Cannot be serialized or recovered from disk.
pub struct RecordInstallationMatch<'a> {
    installation: &'a VerifiedPodmanInstallation,
    record: &'a PlanRecord,
}
impl RecordInstallationMatch<'_> {
    pub fn revalidate(&self) -> Result<()> {
        self.installation.revalidate()?;
        self.record.match_installation_snapshot(
            &self.installation.identity,
            &self.installation.config.bytes,
        )
    }
}
