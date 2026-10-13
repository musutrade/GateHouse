//! Coordination for report directories only. The lock is separate from the
//! renewable JSON marker: replacing that marker must not open a deletion gap.

use super::lease::{lease_directory, resource_key, LeaseRecord};
use crate::project::Project;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

#[derive(Debug)]
pub(crate) struct ReportDirectoryGuard {
    file: File,
    marker: PathBuf,
    root: PathBuf,
    invocation_id: String,
    project_identity: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleasedDirectory {
    schema_version: u32,
    invocation_id: String,
    project_identity: String,
    root: PathBuf,
}

impl ReportDirectoryGuard {
    /// A busy lock or any filesystem/locking uncertainty cannot grant deletion.
    /// Lock files are never unlinked: a waiter must not lock an obsolete inode
    /// while another process creates a replacement lock at the same path.
    pub(crate) fn try_acquire(project: &Project, invocation_id: &str) -> Result<Option<Self>> {
        let mut components = Path::new(invocation_id).components();
        if !matches!(components.next(), Some(Component::Normal(_)))
            || components.next().is_some()
            || invocation_id.contains('/')
            || invocation_id.contains('\\')
        {
            bail!("invalid report-directory invocation identity");
        }
        let leases = lease_directory(project)?;
        let locks = leases.join("report-directory-locks");
        crate::utils::fs::ensure_parent_components(&locks)?;
        fs::create_dir_all(&locks).context("create report-directory lock directory")?;
        crate::utils::fs::ensure_parent_components(&locks)?;
        let key = resource_key(&format!("invocation:{invocation_id}"));
        let path = locks.join(format!("{key}.lock"));
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // A separately opened handle cannot share this file until close.
            // FILE_FLAG_OPEN_REPARSE_POINT opens a link itself for rejection,
            // instead of following it to an unrelated coordination file.
            options.share_mode(0).custom_flags(0x0020_0000);
        }
        let file = match options.open(&path) {
            Ok(file) => file,
            #[cfg(windows)]
            Err(error) if matches!(error.raw_os_error(), Some(32 | 33)) => return Ok(None),
            Err(error) => return Err(error).context("open report-directory lock"),
        };
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            bail!("report-directory lock handle is not a regular file");
        }
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            // SAFETY: this live, owned descriptor remains open for the guard's
            // lifetime; closing it releases the nonblocking exclusive flock.
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::WouldBlock {
                    return Ok(None);
                }
                return Err(error).context("lock report directory");
            }
        }
        #[cfg(not(any(unix, windows)))]
        bail!("report-directory locking is unavailable on this platform");
        let root = leases
            .parent()
            .context("lease directory has no report parent")?
            .join("invocations")
            .join(invocation_id);
        Ok(Some(Self {
            file,
            marker: leases.join(format!("{key}.json")),
            root,
            invocation_id: invocation_id.into(),
            project_identity: project.input().project_identity.clone(),
        }))
    }

    pub(super) fn validate_record(&self, record: &LeaseRecord) -> Result<()> {
        if record.resource_kind != "report-directory"
            || record.resource_id != format!("invocation:{}", self.invocation_id)
            || record.invocation_id != self.invocation_id
            || record.project_identity != self.project_identity
            || record.resource_name.as_deref().map(Path::new) != Some(self.root.as_path())
        {
            bail!("report-directory lease binding is uncertain");
        }
        Ok(())
    }

    pub(super) fn invalidate_release(&self) -> Result<()> {
        self.write_state(b"held\n")
    }

    pub(super) fn mark_released(&self, record: &LeaseRecord) -> Result<()> {
        self.validate_record(record)?;
        self.write_state(&serde_json::to_vec(&ReleasedDirectory {
            schema_version: 1,
            invocation_id: self.invocation_id.clone(),
            project_identity: self.project_identity.clone(),
            root: self.root.clone(),
        })?)
    }

    fn write_state(&self, state: &[u8]) -> Result<()> {
        // Readers use the same cross-process lock, including on Windows where
        // opening this path through a second handle would be denied.
        let mut file = &self.file;
        file.seek(SeekFrom::Start(0))?;
        file.set_len(0)?;
        file.write_all(state)?;
        file.sync_all()
            .context("persist report-directory release state")
    }

    pub(crate) fn release_is_proven(&self, root: &Path) -> Result<bool> {
        if root != self.root {
            return Ok(false);
        }
        match fs::symlink_metadata(&self.marker) {
            Ok(_) => return Ok(false), // Every existing marker is retained.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("inspect report-directory lease marker"),
        }
        let mut file = &self.file;
        file.seek(SeekFrom::Start(0))?;
        let mut contents = Vec::new();
        file.take(16 * 1024 + 1).read_to_end(&mut contents)?;
        if contents.len() > 16 * 1024 {
            return Ok(false);
        }
        let Ok(released) = serde_json::from_slice::<ReleasedDirectory>(&contents) else {
            return Ok(false); // Empty/held/malformed/legacy state is uncertain.
        };
        Ok(released.schema_version == 1
            && released.invocation_id == self.invocation_id
            && released.project_identity == self.project_identity
            && released.root == self.root)
    }
}

#[cfg(unix)]
impl Drop for ReportDirectoryGuard {
    fn drop(&mut self) {
        use std::os::fd::AsRawFd;
        // A forked child can temporarily retain the same open file description
        // before exec closes its CLOEXEC descriptor. End this owned guard's
        // lock lifetime explicitly; inherited descriptors are not new guards.
        // SAFETY: this guard still owns the live descriptor. Failure grants no
        // release certificate: File still closes, and inherited copies may
        // conservatively keep the lock busy until their exec or exit.
        let _ = unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
    }
}
