//! Descriptor-relative Unix persistence for the new lifecycle journal only.
//! Cooperative same-user locking, not protection from a hostile same-UID writer.
use super::*;
use crate::lifecycle::canonical;
use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

fn io<T>(r: std::io::Result<T>) -> Result<T> {
    r.map_err(|_| Invalid("lifecycle journal I/O failure; retain state"))
}
fn private(m: &fs::Metadata, directory: bool) -> Result<()> {
    // SAFETY: geteuid has no pointer arguments or preconditions.
    if m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o777 != if directory { 0o700 } else { 0o600 }
        || if directory {
            !m.is_dir()
        } else {
            !m.is_file() || m.nlink() != 1
        }
    {
        return Err(Invalid("lifecycle journal must be private"));
    }
    Ok(())
}

/// Holds a nonblocking exclusive resource-directory lock. Dispatch is unavailable
/// after reopen until a new request has been durably prepared. Directory must be
/// provisioned by Core; this store never discovers or migrates another lease.
pub struct Journal {
    root: PathBuf,
    directory: File,
    record: Record,
    bytes: Vec<u8>,
    poisoned: bool,
    ready: bool,
    fresh_status: bool,
}

/// Single-use, non-Clone, borrowed from a held journal. The future installed host
/// must accept this token, not a free Request. No engine execution exists here.
///
/// ```compile_fail
/// let forged = gatehouse_runtime_protocol::lifecycle::journal::Dispatch { journal: unimplemented!() };
/// ```
/// ```compile_fail
/// fn duplicate(token: gatehouse_runtime_protocol::lifecycle::journal::Dispatch<'_>) {
///     let _ = token.clone();
/// }
/// ```
#[must_use]
pub struct Dispatch<'a> {
    journal: &'a mut Journal,
}
impl Dispatch<'_> {
    pub fn request(&self) -> &Request {
        &self
            .journal
            .record
            .pending
            .as_ref()
            .expect("prepared token")
            .request
    }
    /// Call only after host exit/transport checks. None means an uncertain result.
    pub fn observe(self, bytes: Option<&[u8]>) -> Result<()> {
        let next = match self.journal.record.observe(bytes) {
            Ok(next) => next,
            Err(e) => {
                self.journal.poisoned = true;
                return Err(e);
            }
        };
        let fresh = matches!(
            next.attempts
                .last()
                .and_then(|a| a.response.as_ref())
                .map(|r| (&r.operation, &r.outcome)),
            Some((Operation::Status { .. }, Outcome::Status { .. }))
        );
        self.journal.commit(next)?;
        self.journal.fresh_status = fresh;
        Ok(())
    }
}
impl Journal {
    fn directory(root: &Path) -> Result<File> {
        if !root.is_absolute() || io(fs::canonicalize(root))? != root {
            return Err(Invalid("journal path must be canonical"));
        }
        // Ancestors must not be writable by another principal; ownership may be
        // root or current UID. This also rejects /tmp and symlink ancestors.
        for parent in root.ancestors().skip(1) {
            let m = io(fs::symlink_metadata(parent))?;
            if !m.is_dir()
                || m.mode() & 0o022 != 0
                || (m.uid() != 0 && m.uid() != unsafe { libc::geteuid() })
            {
                return Err(Invalid("unprotected journal ancestor"));
            }
        }
        let directory = io(OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(root))?;
        private(&io(directory.metadata())?, true)?;
        // SAFETY: the file owns a live directory descriptor for the guard lifetime.
        if unsafe { libc::flock(directory.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(Invalid("lifecycle resource is locked"));
        }
        Ok(directory)
    }
    fn at(directory: &File, name: &str, flags: i32) -> Result<Option<File>> {
        let name = CString::new(name).map_err(|_| Invalid("invalid journal member"))?;
        // SAFETY: NUL-terminated fixed internal name, valid directory descriptor.
        let fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                name.as_ptr(),
                flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
                0o600,
            )
        };
        if fd < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::NotFound && flags & libc::O_CREAT == 0 {
                return Ok(None);
            }
            return Err(Invalid("cannot open lifecycle journal member"));
        }
        // SAFETY: successful openat returns an owned descriptor.
        let f = unsafe { File::from_raw_fd(fd) };
        private(&io(f.metadata())?, false)?;
        Ok(Some(f))
    }
    fn read(directory: &File, name: &str) -> Result<Option<Vec<u8>>> {
        let Some(f) = Self::at(directory, name, libc::O_RDONLY)? else {
            return Ok(None);
        };
        let mut b = vec![];
        io(f.take((crate::MAX_MESSAGE_BYTES + 1) as u64)
            .read_to_end(&mut b))?;
        if b.len() > crate::MAX_MESSAGE_BYTES {
            return Err(Invalid("journal member too large"));
        }
        Ok(Some(b))
    }
    fn check(&self) -> Result<()> {
        if self.poisoned {
            return Err(Invalid("journal poisoned; reopen required"));
        }
        let now = io(fs::symlink_metadata(&self.root))?;
        let held = io(self.directory.metadata())?;
        private(&now, true)?;
        if now.dev() != held.dev() || now.ino() != held.ino() {
            return Err(Invalid("journal directory replaced"));
        }
        if Self::read(&self.directory, "lease.json")?.as_deref() != Some(self.bytes.as_slice()) {
            return Err(Invalid("journal changed outside lock"));
        }
        if self.record.state.phase() != Phase::VerifiedAbsent
            && Self::read(&self.directory, "active")?.as_deref()
                != Some(self.record.create.binding.operation_key.as_bytes())
        {
            return Err(Invalid("active lifecycle marker changed"));
        }
        Ok(())
    }
    fn encode(record: &Record) -> Result<Vec<u8>> {
        record.validate()?;
        canonical::encode(
            &serde_json::to_value(record).map_err(|_| Invalid("journal encoding failed"))?,
        )
    }
    fn rename(directory: &File, from: &str, to: &str) -> Result<()> {
        let from = CString::new(from).unwrap();
        let to = CString::new(to).unwrap();
        // SAFETY: both internal names and descriptor are valid for this call.
        if unsafe {
            libc::renameat(
                directory.as_raw_fd(),
                from.as_ptr(),
                directory.as_raw_fd(),
                to.as_ptr(),
            )
        } != 0
        {
            return Err(Invalid("journal publish failed"));
        }
        Ok(())
    }
    fn atomic(directory: &File, name: &str, bytes: &[u8]) -> Result<()> {
        Self::atomic_with(directory, name, bytes, |_| Ok(()))
    }
    fn atomic_with(
        directory: &File,
        name: &str,
        bytes: &[u8],
        mut stage: impl FnMut(u8) -> Result<()>,
    ) -> Result<()> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let temp = format!(
            "next-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let mut f = Self::at(
            directory,
            &temp,
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
        )?
        .ok_or(Invalid("missing journal temp"))?;
        io(f.write_all(bytes))?;
        stage(0)?;
        io(f.sync_all())?;
        stage(1)?;
        Self::rename(directory, &temp, name)?;
        stage(2)?;
        io(directory.sync_all())?;
        Ok(())
    }
    pub fn create(root: &Path, create: &Request) -> Result<Self> {
        let directory = Self::directory(root)?;
        let record = Record::new(create)?;
        let bytes = Self::encode(&record)?;
        for name in ["lease.json", "active", "tombstone.json"] {
            if Self::read(&directory, name)?.is_some() {
                return Err(Invalid("lifecycle resource already recorded"));
            }
        }
        // Active marker first: a crash before lease persistence still blocks reuse.
        let mut marker = Self::at(
            &directory,
            "active",
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
        )?
        .ok_or(Invalid("missing active marker"))?;
        io(marker.write_all(create.binding.operation_key.as_bytes()))?;
        io(marker.sync_all())?;
        io(directory.sync_all())?;
        Self::atomic(&directory, "lease.json", &bytes)?;
        Ok(Self {
            root: root.into(),
            directory,
            record,
            bytes,
            poisoned: false,
            ready: true,
            fresh_status: false,
        })
    }
    pub fn open(root: &Path, create: &Request) -> Result<Self> {
        create.validate()?;
        let directory = Self::directory(root)?;
        let bytes = Self::read(&directory, "lease.json")?
            .ok_or(Invalid("missing journal; no automatic adoption"))?;
        let value = canonical::parse_canonical(&bytes)?;
        let record: Record = serde_json::from_value(value)
            .map_err(|_| Invalid("invalid lifecycle journal schema"))?;
        if Self::encode(&record)? != bytes || record.create != *create {
            return Err(Invalid("journal binding mismatch"));
        }
        let marker = Self::read(&directory, "active")?;
        let tombstone = Self::read(&directory, "tombstone.json")?;
        if let Some(t) = &tombstone {
            if record.state.phase() != Phase::VerifiedAbsent || t != &bytes {
                return Err(Invalid("journal tombstone mismatch"));
            }
        }
        if let Some(m) = &marker {
            if m != create.binding.operation_key.as_bytes() {
                return Err(Invalid("journal marker mismatch"));
            }
        } else if record.state.phase() != Phase::VerifiedAbsent
            || tombstone.as_ref() != Some(&bytes)
        {
            return Err(Invalid("missing active marker without durable absence"));
        }
        let mut store = Self {
            root: root.into(),
            directory,
            record,
            bytes,
            poisoned: false,
            ready: false,
            fresh_status: false,
        };
        let recovered = store.record.recover()?;
        if recovered != store.record {
            store.commit(recovered)?;
        }
        Ok(store)
    }
    pub fn phase(&self) -> Phase {
        self.record.state.phase()
    }
    pub fn revision(&self) -> u64 {
        self.record.revision
    }
    pub fn target(&self) -> Result<Target> {
        self.record.target()
    }
    fn commit(&mut self, next: Record) -> Result<()> {
        self.commit_with(next, |_| Ok(()))
    }
    fn commit_with(&mut self, next: Record, stage: impl FnMut(u8) -> Result<()>) -> Result<()> {
        self.check()?;
        self.poisoned = true;
        self.ready = false;
        self.fresh_status = false;
        let bytes = Self::encode(&next)?;
        Self::atomic_with(&self.directory, "lease.json", &bytes, stage)?;
        self.record = next;
        self.bytes = bytes;
        self.poisoned = false;
        Ok(())
    }
    pub fn begin(&mut self, request: &Request, purpose: Purpose) -> Result<Dispatch<'_>> {
        self.check()?;
        let next = self.record.begin(request, purpose, self.fresh_status)?;
        self.commit(next)?;
        self.ready = true;
        self.dispatch()
    }
    /// Only the initial just-created record can use this without begin().
    /// Reopened records never have ready=true; borrowing then dropping a token
    /// cannot authorize a second dispatch.
    pub fn dispatch(&mut self) -> Result<Dispatch<'_>> {
        self.check()?;
        if !self.ready || self.record.pending.is_none() {
            return Err(Invalid("no fresh durable dispatch authorization"));
        }
        self.ready = false;
        Ok(Dispatch { journal: self })
    }
    /// Evidence is already committed in the verified-absent record. Write its
    /// tombstone durably before unlinking active, then fsync the directory. This
    /// method is idempotent after a crash; it never removes the lease/evidence.
    pub fn finish_release(&mut self) -> Result<()> {
        self.check()?;
        if self.record.state.phase() != Phase::VerifiedAbsent {
            return Err(Invalid("absence evidence required before release"));
        }
        self.poisoned = true;
        match Self::read(&self.directory, "tombstone.json")? {
            Some(b) if b != self.bytes => return Err(Invalid("conflicting tombstone")),
            Some(_) => {}
            None => Self::atomic(&self.directory, "tombstone.json", &self.bytes)?,
        }
        let name = CString::new("active").unwrap();
        // SAFETY: fixed name, live directory descriptor; never a container path.
        if unsafe { libc::unlinkat(self.directory.as_raw_fd(), name.as_ptr(), 0) } != 0
            && std::io::Error::last_os_error().kind() != std::io::ErrorKind::NotFound
        {
            return Err(Invalid("cannot remove lifecycle marker"));
        }
        io(self.directory.sync_all())?;
        self.poisoned = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
