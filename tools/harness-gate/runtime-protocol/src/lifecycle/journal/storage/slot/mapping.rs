//! Fixed resource mapping below one protected Core state root.
use super::*;
use sha2::{Digest, Sha256};
fn name(project: &str, resource: &str) -> Result<String> {
    crate::nonempty(project)?;
    crate::nonempty(resource)?;
    let mut hash = Sha256::new();
    for value in ["gatehouse-lifecycle-resource/v1", project, resource] {
        hash.update((value.len() as u64).to_be_bytes());
        hash.update(value.as_bytes());
    }
    Ok(format!("resource-{:x}", hash.finalize()))
}
impl ServiceSlot {
    /// Stable mapping is independent of image, host path, invocation or config
    /// changes: those changes cannot select a new slot for an unresolved resource.
    pub fn mapped_root(base: &Path, project: &str, resource: &str) -> Result<PathBuf> {
        Ok(base.join(name(project, resource)?))
    }
    pub fn acquire_mapped(
        base: &Path,
        resolve: Request,
        plan: CreatePlan,
        healthcheck: Vec<String>,
    ) -> Result<Self> {
        resolve.validate()?;
        let parent = Journal::directory(base)?;
        let name = name(&resolve.binding.project_id, &resolve.binding.resource_id)?;
        let child = CString::new(name.clone()).unwrap();
        // SAFETY: bounded digest name and held directory descriptor. Never chmod
        // an existing entry; Journal::directory validates its exact protection.
        if unsafe { libc::mkdirat(parent.as_raw_fd(), child.as_ptr(), 0o700) } != 0
            && std::io::Error::last_os_error().kind() != std::io::ErrorKind::AlreadyExists
        {
            return Err(Invalid("cannot provision lifecycle resource root"));
        }
        io(parent.sync_all())?;
        let root = base.join(name);
        let slot = Self::acquire(&root, resolve, plan, healthcheck)?;
        slot.check()?;
        Ok(slot)
    }
}

impl ServiceSlot {
    /// Fixed Core namespace below the canonical project root. No environment or
    /// host/config path selects the base. Existing unsafe entries are never fixed.
    pub fn project_state_base(project: &Path) -> Result<PathBuf> {
        if !project.is_absolute() || io(fs::canonicalize(project))? != project {
            return Err(Invalid("project state root must be canonical"));
        }
        for path in project.ancestors() {
            let m = io(fs::symlink_metadata(path))?;
            if !m.is_dir()
                || m.mode() & 0o022 != 0
                || ![0, unsafe { libc::geteuid() }].contains(&m.uid())
            {
                return Err(Invalid("unprotected project state ancestor"));
            }
        }
        let mut directory = io(OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(project))?;
        let mut path = project.to_path_buf();
        for component in [".harness-gate", "lifecycle-services"] {
            let name = CString::new(component).unwrap();
            if unsafe { libc::mkdirat(directory.as_raw_fd(), name.as_ptr(), 0o700) } != 0
                && std::io::Error::last_os_error().kind() != std::io::ErrorKind::AlreadyExists
            {
                return Err(Invalid("cannot provision fixed lifecycle state"));
            }
            io(directory.sync_all())?;
            let fd = unsafe {
                libc::openat(
                    directory.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(Invalid("invalid lifecycle state directory"));
            }
            let child = unsafe { File::from_raw_fd(fd) };
            let m = io(child.metadata())?;
            if m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o022 != 0 {
                return Err(Invalid("unprotected lifecycle state directory"));
            }
            path.push(component);
            directory = child;
        }
        // Require exact private final directory, canonical ancestors and lockability.
        let verified = Journal::directory(&path)?;
        let actual = io(verified.metadata())?;
        let held = io(directory.metadata())?;
        if (actual.dev(), actual.ino()) != (held.dev(), held.ino()) {
            return Err(Invalid("lifecycle state directory changed"));
        }
        Ok(path)
    }
}
