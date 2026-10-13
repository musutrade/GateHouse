//! Host-owned nonce storage. Paths are opened without following links; claims
//! use the pinned directory, never a collector-controlled path resolution.
use super::adapter::{AdapterError, HostPolicy};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::Write,
    path::{Component, Path, PathBuf},
};

fn failure(error: impl std::fmt::Display) -> AdapterError {
    AdapterError::Protocol(format!("adapter replay state: {error}"))
}

pub(crate) fn collector_policy(
    mut policy: HostPolicy,
    repository_root: &Path,
    override_dir: Option<&Path>,
    artifact_root: &Path,
) -> Result<HostPolicy, AdapterError> {
    let path = override_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| repository_root.join(".harness-gate/collector-nonces"));
    let ledger = Ledger::open(&path)?;
    ledger.ensure_separate(artifact_root)?;
    policy.replay_state_dir = Some(ledger.path.clone());
    policy.replay_guard.bind(ledger)?;
    Ok(policy)
}

#[derive(Debug)]
pub(super) struct Ledger {
    pub(super) path: PathBuf,
    directory: File,
    // Windows handles deny delete sharing, pinning every path component.
    #[cfg(windows)]
    _ancestors: Vec<File>,
}

impl Ledger {
    pub(super) fn open(path: &Path) -> Result<Self, AdapterError> {
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir().map_err(failure)?.join(path)
        };
        if path.components().any(|c| matches!(c, Component::ParentDir)) {
            return Err(failure("parent traversal is forbidden"));
        }
        let mut ledger = platform::open(&path).map_err(failure)?;
        ledger.check_writable()?;
        // Normalize only after the no-follow walk. In particular, Windows
        // canonical paths use a verbatim prefix whereas CLI paths often do not.
        // Revalidate the normalized name against the pinned directory so a
        // concurrent replacement cannot redirect subsequent claims.
        ledger.path = path.canonicalize().map_err(failure)?;
        ledger.validate()?;
        Ok(ledger)
    }

    pub(super) fn matches_path(&self, path: &Path) -> Result<bool, AdapterError> {
        let requested = Self::open(path)?;
        self.validate()?;
        Ok(self.path == requested.path)
    }

    pub(super) fn ensure_separate(&self, artifact_root: &Path) -> Result<(), AdapterError> {
        self.validate()?;
        let artifacts = artifact_root.canonicalize().map_err(failure)?;
        if self.path.starts_with(&artifacts) || artifacts.starts_with(&self.path) {
            return Err(failure(
                "nonce ledger must be separate from adapter artifacts",
            ));
        }
        Ok(())
    }

    pub(super) fn validate(&self) -> Result<(), AdapterError> {
        let current = platform::open(&self.path).map_err(failure)?;
        platform::same_directory(&self.directory, &current.directory).map_err(failure)?;
        self.check_writable()
    }

    fn check_writable(&self) -> Result<(), AdapterError> {
        let metadata = self.directory.metadata().map_err(failure)?;
        if !metadata.is_dir() || metadata.permissions().readonly() {
            return Err(failure("ledger must be a writable directory"));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            // A same-uid untrusted child must be isolated by the host's sandbox;
            // mode bits cannot distinguish processes with the same identity.
            if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o022 != 0 {
                return Err(failure(
                    "ledger must be host-owned and not writable by group or others",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn claim(
        &self,
        nonce: &str,
        issued: u64,
        expires: u64,
        now: u64,
    ) -> Result<(), AdapterError> {
        self.validate()?;
        let name = format!("nonce-{:x}.json", Sha256::digest(nonce.as_bytes()));
        let mut file = platform::create_marker(self, &name).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                AdapterError::Protocol("adapter request nonce has already been used".into())
            } else {
                failure(error)
            }
        })?;
        let record = serde_json::json!({"nonce": nonce, "issued_at_ms": issued,
            "expires_at_ms": expires, "claimed_at_ms": now});
        // Keep even a partial marker on error. A failed/crashed invocation must
        // never free a nonce that could already have been consumed.
        file.write_all(record.to_string().as_bytes())
            .map_err(failure)?;
        file.sync_all().map_err(failure)?;
        platform::sync_directory(&self.directory).map_err(failure)?;
        self.validate()
    }
}

#[cfg(unix)]
mod platform {
    use super::*;
    use std::{
        ffi::CString,
        io,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::{ffi::OsStrExt, fs::MetadataExt},
        },
    };

    fn open_at(parent: &File, name: &CString) -> io::Result<File> {
        // SAFETY: name is NUL-terminated, parent stays open, and ownership of
        // each successful descriptor is transferred exactly once to File.
        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(unsafe { File::from_raw_fd(fd) })
        }
    }

    pub(super) fn open(path: &Path) -> io::Result<Ledger> {
        let mut directory = File::open("/")?;
        let root_owner = directory.metadata()?.uid();
        let names: Vec<_> = path
            .components()
            .filter_map(|c| match c {
                Component::Normal(name) => Some(name),
                _ => None,
            })
            .collect();
        let mut ancestor = PathBuf::from("/");
        for (index, name) in names.iter().enumerate() {
            trusted_parent(&directory, root_owner, &ancestor)?;
            ancestor.push(name);
            let name = CString::new(name.as_bytes())?;
            directory = match open_at(&directory, &name) {
                Ok(next) => next,
                Err(error)
                    if error.kind() == io::ErrorKind::NotFound && index + 1 == names.len() =>
                {
                    // Only the leaf may be provisioned automatically; its parent
                    // must already belong to the host's control plane.
                    let result =
                        unsafe { libc::mkdirat(directory.as_raw_fd(), name.as_ptr(), 0o700) };
                    if result != 0
                        && io::Error::last_os_error().kind() != io::ErrorKind::AlreadyExists
                    {
                        return Err(io::Error::last_os_error());
                    }
                    directory.sync_all()?;
                    open_at(&directory, &name)?
                }
                Err(error) => return Err(error),
            };
        }
        Ok(Ledger {
            path: path.to_path_buf(),
            directory,
        })
    }

    fn trusted_parent(directory: &File, root_owner: u32, path: &Path) -> io::Result<()> {
        let metadata = directory.metadata()?;
        let owner = metadata.uid();
        let current = unsafe { libc::geteuid() };
        let mode = metadata.mode() & 0o7777;
        let shared = mode & 0o022 != 0;
        let sticky_root = owner == root_owner && mode & 0o1000 != 0;
        let remedy = "or pass --replay-state-dir with a private host-owned directory";
        if owner != current && owner != root_owner {
            return Err(io::Error::other(format!(
                "ledger ancestor is not protected by the host: {} is owned by uid {owner}, \
                 not the current user ({current}) or root; use a directory owned by the \
                 current user {remedy}",
                path.display()
            )));
        }
        if shared && !sticky_root {
            return Err(io::Error::other(format!(
                "ledger ancestor is not protected by the host: {} has mode {mode:04o} and is \
                 writable by group or others; run `chmod go-w {}` {remedy} \
                 (a umask of 0002 creates such directories)",
                path.display(),
                path.display()
            )));
        }
        Ok(())
    }

    pub(super) fn same_directory(first: &File, second: &File) -> io::Result<()> {
        let a = first.metadata()?;
        let b = second.metadata()?;
        if (a.dev(), a.ino()) != (b.dev(), b.ino()) {
            return Err(io::Error::other("ledger directory was replaced"));
        }
        Ok(())
    }

    pub(super) fn create_marker(ledger: &Ledger, name: &str) -> io::Result<File> {
        let name = CString::new(name)?;
        let fd = unsafe {
            libc::openat(
                ledger.directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(unsafe { File::from_raw_fd(fd) })
        }
    }

    pub(super) fn sync_directory(directory: &File) -> io::Result<()> {
        directory.sync_all()
    }
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::{
        fs::{self, OpenOptions},
        io,
        os::windows::fs::{MetadataExt, OpenOptionsExt},
    };

    const FILE_SHARE_READ_WRITE: u32 = 0x00000001 | 0x00000002;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x00200000;
    const FILE_FLAG_WRITE_THROUGH: u32 = 0x80000000;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x00000400;

    fn open_directory(path: &Path) -> io::Result<File> {
        let file = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(io::Error::other(
                "ledger path contains a reparse point or non-directory",
            ));
        }
        Ok(file)
    }

    pub(super) fn open(path: &Path) -> io::Result<Ledger> {
        let mut current = PathBuf::new();
        let mut handles = Vec::new();
        let parts: Vec<_> = path.components().collect();
        for (index, part) in parts.iter().enumerate() {
            current.push(part.as_os_str());
            if matches!(part, Component::Prefix(_)) {
                continue;
            }
            let file = match open_directory(&current) {
                Ok(file) => file,
                Err(error)
                    if error.kind() == io::ErrorKind::NotFound && index + 1 == parts.len() =>
                {
                    match fs::create_dir(&current) {
                        Ok(()) => (),
                        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => (),
                        Err(error) => return Err(error),
                    }
                    open_directory(&current)?
                }
                Err(error) => return Err(error),
            };
            handles.push(file);
        }
        let directory = handles
            .pop()
            .ok_or_else(|| io::Error::other("missing ledger directory"))?;
        Ok(Ledger {
            path: path.to_path_buf(),
            directory,
            _ancestors: handles,
        })
    }

    pub(super) fn same_directory(_first: &File, _second: &File) -> io::Result<()> {
        // Delete/rename is forbidden while any pinned directory handle lives.
        Ok(())
    }

    pub(super) fn create_marker(ledger: &Ledger, name: &str) -> io::Result<File> {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .share_mode(0)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_WRITE_THROUGH)
            .open(ledger.path.join(name))
    }

    pub(super) fn sync_directory(_directory: &File) -> io::Result<()> {
        // Windows uses FILE_FLAG_WRITE_THROUGH plus FlushFileBuffers on the
        // marker. Directory FlushFileBuffers is unsupported by Windows.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn existing_corrupt_markers_stay_consumed_after_restart() {
        let root = tempdir().unwrap();
        let path = root.path().canonicalize().unwrap().join("ledger");
        let ledger = Ledger::open(&path).unwrap();
        let marker = path.join(format!("nonce-{:x}.json", Sha256::digest(b"used")));
        std::fs::write(marker, "corrupt or partial record").unwrap();
        assert!(ledger
            .claim("used", 1, 2, 1)
            .unwrap_err()
            .to_string()
            .contains("already been used"));
        ledger.claim("fresh", 1, 2, 1).unwrap();
        let reopened = Ledger::open(&path).unwrap();
        assert!(reopened.claim("fresh", 1, 2, 1).is_err());
    }

    #[test]
    fn shared_host_policy_cannot_switch_ledger_scope() {
        let root = tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        let artifacts = root.join("artifacts");
        std::fs::create_dir(&artifacts).unwrap();
        let first = root.join("first");
        let policy =
            collector_policy(HostPolicy::default(), &root, Some(&first), &artifacts).unwrap();
        collector_policy(policy.clone(), &root, Some(&first), &artifacts).unwrap();
        let error =
            collector_policy(policy, &root, Some(&root.join("second")), &artifacts).unwrap_err();
        assert!(error.to_string().contains("scope changed"));
    }

    #[test]
    fn equivalent_path_spellings_share_one_canonical_ledger_scope() {
        let root = tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        let artifacts = root.join("ledger-artifacts");
        std::fs::create_dir(&artifacts).unwrap();
        let path = root.join("ledger");
        let policy =
            collector_policy(HostPolicy::default(), &root, Some(&path), &artifacts).unwrap();
        let alternate = root.join(".").join("ledger");
        let policy = collector_policy(policy, &root, Some(&alternate), &artifacts).unwrap();
        assert_eq!(
            policy.replay_state_dir.as_ref().unwrap(),
            &path.canonicalize().unwrap()
        );
        let ledger = Ledger::open(&alternate).unwrap();
        assert!(ledger.matches_path(&path).unwrap());
        assert!(ledger.matches_path(&alternate).unwrap());
        ledger.ensure_separate(&artifacts).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_ordinary_verbatim_and_case_spellings_share_one_ledger_scope() {
        let root = tempdir().unwrap();
        let canonical = root.path().canonicalize().unwrap();
        let ordinary = PathBuf::from(canonical.to_string_lossy().strip_prefix(r"\\?\").unwrap());
        let path = ordinary.join("MiXeD-ledger");
        let ledger = Ledger::open(&path).unwrap();
        let verbatim = path.canonicalize().unwrap();
        assert!(ledger.matches_path(&path).unwrap());
        assert!(ledger.matches_path(&verbatim).unwrap());
        assert!(ledger.matches_path(&ordinary.join("mixed-ledger")).unwrap());
        let artifacts = ordinary.join("artifacts");
        std::fs::create_dir(&artifacts).unwrap();
        let policy =
            collector_policy(HostPolicy::default(), &ordinary, Some(&path), &artifacts).unwrap();
        collector_policy(policy, &ordinary, Some(&verbatim), &artifacts).unwrap();
        assert!(ledger.ensure_separate(&path).is_err());
        let nested = verbatim.join("artifacts");
        std::fs::create_dir(&nested).unwrap();
        assert!(ledger.ensure_separate(&nested).is_err());
        assert!(ledger.ensure_separate(&ordinary).is_err());
    }

    #[test]
    fn rejects_artifact_overlap_and_non_directory_or_missing_parent() {
        let root = tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        let artifact = root.join("artifacts");
        std::fs::create_dir(&artifact).unwrap();
        assert!(collector_policy(
            HostPolicy::default(),
            &root,
            Some(&artifact.join("ledger")),
            &artifact
        )
        .is_err());
        let file = root.join("file");
        std::fs::write(&file, "not a directory").unwrap();
        assert!(Ledger::open(&file).is_err());
        assert!(Ledger::open(&root.join("missing/ledger")).is_err());
        // PathBuf::join normalizes '..' when the Windows base is verbatim.
        // Append raw native separators so Ledger receives the traversal input.
        let separator = std::path::MAIN_SEPARATOR;
        let mut traversal = root.as_os_str().to_os_string();
        traversal.push(format!(
            "{separator}artifacts{separator}..{separator}ledger"
        ));
        let traversal = PathBuf::from(traversal);
        assert!(traversal
            .components()
            .any(|c| matches!(c, Component::ParentDir)));
        assert!(Ledger::open(&traversal)
            .unwrap_err()
            .to_string()
            .contains("parent traversal is forbidden"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_unwritable_and_shared_ledgers() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempdir().unwrap();
        let path = root.path().canonicalize().unwrap().join("ledger");
        let ledger = Ledger::open(&path).unwrap();
        for mode in [0o500, 0o770, 0o777] {
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
            assert!(ledger.claim("new", 1, 2, 1).is_err());
            assert!(!path
                .join(format!("nonce-{:x}.json", Sha256::digest(b"new")))
                .exists());
        }
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o777)).unwrap();
        let message = Ledger::open(&path).err().unwrap().to_string();
        let shared = root.path().canonicalize().unwrap();
        assert!(message.contains(&format!("{} has mode 0777", shared.display())));
        assert!(message.contains(&format!("chmod go-w {}", shared.display())));
        assert!(message.contains("--replay-state-dir"));
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o775)).unwrap();
        assert!(Ledger::open(&path)
            .err()
            .unwrap()
            .to_string()
            .contains("umask of 0002"));
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_leaf_ancestor_and_post_binding_directory_replacement() {
        use std::os::unix::fs::symlink;
        let root = tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        let path = root.join("ledger");
        let ledger = Ledger::open(&path).unwrap();
        let moved = root.join("old-ledger");
        std::fs::rename(&path, &moved).unwrap();
        symlink(&moved, &path).unwrap();
        assert!(ledger.claim("new", 1, 2, 1).is_err());
        assert!(Ledger::open(&path).is_err());
        assert!(Ledger::open(&path.join("nested")).is_err());
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(ledger
            .claim("new", 1, 2, 1)
            .unwrap_err()
            .to_string()
            .contains("replaced"));
        assert!(std::fs::read_dir(&path).unwrap().next().is_none());
        assert!(std::fs::read_dir(&moved).unwrap().next().is_none());
    }
}
