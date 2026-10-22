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
