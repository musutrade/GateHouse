//! Unix cooperative durable state, not an executor. One private directory per
//! intent, provisioned by the trusted operator. Never deletes a completed record.
use crate::intent::{Intent, Phase};
use crate::{Invalid, Label, Operation, Request, Response, MAX_MESSAGE_BYTES};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const RECORD: &str = "intent.json";
type Result<T> = std::result::Result<T, Invalid>;

fn io<T>(value: std::io::Result<T>) -> Result<T> {
    value.map_err(|_| Invalid("intent store I/O failure; retain state"))
}

/// Keeps the directory lock until drop. Do not release this guard between the
/// durable Unknown transition and a future dispatch/result commit. A failed
/// commit poisons this guard; reopen and reconcile rather than retry mutation.
pub struct Store {
    root: PathBuf,
    directory: File,
    expected: Request,
    bytes: Option<Vec<u8>>,
    intent: Option<Intent>,
    poisoned: bool,
}

fn private(metadata: &fs::Metadata) -> Result<()> {
    // SAFETY: geteuid has no preconditions or borrowed pointers.
    if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o077 != 0 {
        return Err(Invalid("intent store must be private to the current user"));
    }
    Ok(())
}

impl Store {
    pub fn open(
        root: &Path,
        expected: &Request,
        timeout: Duration,
        cancelled: impl Fn() -> bool,
    ) -> Result<Self> {
        expected.validate()?;
        if !matches!(expected.operation, Operation::Remove { .. }) {
            return Err(Invalid("intent store requires a removal request"));
        }
        if timeout.is_zero() || timeout > Duration::from_secs(30) {
            return Err(Invalid("invalid intent lock timeout"));
        }
        if !root.is_absolute() || io(fs::canonicalize(root))? != root {
            return Err(Invalid("intent directory must be canonical and absolute"));
        }
        let directory = io(OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(root))?;
        private(&io(directory.metadata())?)?;
        lock(&directory, timeout, &cancelled)?;
        let mut store = Self {
            root: root.into(),
            directory,
            expected: expected.clone(),
            bytes: None,
            intent: None,
            poisoned: false,
        };
        store.check_directory()?;
        let bytes = store.read()?;
        store.intent = bytes
            .as_deref()
            .map(|bytes| Intent::restore(bytes, expected))
            .transpose()?;
        store.bytes = bytes;
        if cancelled() {
            return Err(Invalid("intent open cancelled"));
        }
        Ok(store)
    }

    pub fn phase(&self) -> Option<Phase> {
        self.intent.as_ref().map(Intent::phase)
    }

    pub fn prepare(&mut self, inspection: &Response, labels: &[Label]) -> Result<()> {
        if self.intent.is_some() {
            return Err(Invalid("intent already exists"));
        }
        let next = Intent::prepare(&self.expected, inspection, labels)?;
        self.commit(next)
    }

    /// Success means Unknown was synced. This method never starts a process.
    pub fn begin_dispatch(&mut self) -> Result<()> {
        let next = self.current()?.begin_dispatch(&self.expected)?;
        self.commit(next)
    }

    pub fn observe_dispatch(&mut self, response: Option<&Response>) -> Result<()> {
        let next = self.current()?.observe_dispatch(&self.expected, response)?;
        self.commit(next)
    }

    pub fn reconcile(&mut self, inspection: &Response) -> Result<()> {
        let next = self.current()?.reconcile(&self.expected, inspection)?;
        self.commit(next)
    }

    fn current(&self) -> Result<&Intent> {
        if self.poisoned {
            return Err(Invalid("intent store requires reopen after failed commit"));
        }
        self.intent.as_ref().ok_or(Invalid("intent not prepared"))
    }

    fn check_directory(&self) -> Result<()> {
        let held = io(self.directory.metadata())?;
        let named = io(fs::symlink_metadata(&self.root))?;
        private(&held)?;
        if !named.is_dir() || held.dev() != named.dev() || held.ino() != named.ino() {
            return Err(Invalid("intent directory identity changed"));
        }
        Ok(())
    }

    fn read(&self) -> Result<Option<Vec<u8>>> {
        self.check_directory()?;
        let file = match OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(self.root.join(RECORD))
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(Invalid("cannot open intent record")),
        };
        let metadata = io(file.metadata())?;
        private(&metadata)?;
        if !metadata.is_file() || metadata.nlink() != 1 {
            return Err(Invalid(
                "intent record must be a private regular single-link file",
            ));
        }
        let mut bytes = Vec::new();
        io(file
            .take(MAX_MESSAGE_BYTES as u64 + 1)
            .read_to_end(&mut bytes))?;
        if bytes.len() > MAX_MESSAGE_BYTES {
            return Err(Invalid("intent record exceeds size limit"));
        }
        Ok(Some(bytes))
    }

    fn commit(&mut self, next: Intent) -> Result<()> {
        if self.poisoned {
            return Err(Invalid("intent store requires reopen after failed commit"));
        }
        // Any ambiguous write/sync failure prohibits another action on this
        // handle, even if the rename actually succeeded.
        self.poisoned = true;
        if self.read()? != self.bytes {
            return Err(Invalid("intent changed outside its held lock"));
        }
        let bytes = serde_json::to_vec(&next).map_err(|_| Invalid("cannot serialize intent"))?;
        if bytes.len() > MAX_MESSAGE_BYTES {
            return Err(Invalid("intent record exceeds size limit"));
        }
        let mut temporary = io(tempfile::NamedTempFile::new_in(&self.root))?;
        io(temporary.write_all(&bytes))?;
        io(temporary.as_file().sync_all())?;
        self.check_directory()?;
        if self.bytes.is_none() {
            temporary
                .persist_noclobber(self.root.join(RECORD))
                .map_err(|_| Invalid("cannot create intent record; reopen required"))?;
        } else {
            temporary
                .persist(self.root.join(RECORD))
                .map_err(|_| Invalid("cannot commit intent record; reopen required"))?;
        }
        io(self.directory.sync_all())?;
        self.bytes = Some(bytes);
        self.intent = Some(next);
        self.poisoned = false;
        Ok(())
    }
}

fn lock(directory: &File, timeout: Duration, cancelled: &impl Fn() -> bool) -> Result<()> {
    let started = Instant::now();
    loop {
        if cancelled() {
            return Err(Invalid("intent lock cancelled"));
        }
        if started.elapsed() >= timeout {
            return Err(Invalid("intent lock deadline exceeded"));
        }
        // SAFETY: directory owns this live descriptor until the Store is dropped.
        if unsafe { libc::flock(directory.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            return Ok(());
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::WouldBlock
            && error.kind() != std::io::ErrorKind::Interrupted
        {
            return Err(Invalid("cannot lock intent directory"));
        }
        std::thread::sleep(
            Duration::from_millis(10).min(timeout.saturating_sub(started.elapsed())),
        );
    }
}
