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

#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum CommitStage {
    FileSync,
    Publish,
    DirectorySync,
}
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
    prepared_verified: bool,
    #[cfg(test)]
    fail_commit: Option<CommitStage>,
}

/// A single, store-bound authorization created only after Unknown is synced.
/// Not Clone: a mutation host must consume this value, never accept a separate
/// request. The mutable borrow keeps the locked store alive until consumption.
/// This is sequencing authority only, not adapter authentication.
#[must_use]
pub struct DispatchAuthorization<'a> {
    store: &'a mut Store,
}

impl DispatchAuthorization<'_> {
    pub fn request(&self) -> &Request {
        &self.store.expected
    }

    /// All mutation outcomes remain Unknown. Dropping without observation also
    /// leaves the persisted Unknown intact; it never authorizes another attempt.
    pub fn observe(self, response: Option<&Response>) -> Result<()> {
        self.store.observe_dispatch(response)
    }
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
            prepared_verified: false,
            #[cfg(test)]
            fail_commit: None,
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

    /// Informational only, possibly stale after a failed commit; never authority.
    pub fn phase(&self) -> Option<Phase> {
        self.intent.as_ref().map(Intent::phase)
    }

    pub fn prepare(&mut self, inspection: &Response, labels: &[Label]) -> Result<()> {
        if self.intent.is_some() {
            return Err(Invalid("intent already exists"));
        }
        let next = Intent::prepare(&self.expected, inspection, labels)?;
        self.commit(next)?;
        self.prepared_verified = true;
        Ok(())
    }

    /// Reopened Prepared records cannot authorize dispatch until a fresh,
    /// authenticated exact-ID inspection passes independently trusted labels.
    /// Caller authentication is required; this method validates data bindings.
    pub fn resume_prepared(&mut self, inspection: &Response, labels: &[Label]) -> Result<()> {
        self.prepared_verified = false;
        if self.current()?.phase() != Phase::Prepared {
            return Err(Invalid("intent is not prepared for ownership revalidation"));
        }
        Intent::prepare(&self.expected, inspection, labels)?;
        if self.read()? != self.bytes {
            self.poisoned = true;
            return Err(Invalid("intent changed outside its held lock"));
        }
        self.prepared_verified = true;
        Ok(())
    }

    /// Success means Unknown was synced. This method never starts a process.
    pub fn begin_dispatch(&mut self) -> Result<DispatchAuthorization<'_>> {
        if !self.prepared_verified {
            return Err(Invalid("fresh prepared ownership verification required"));
        }
        let next = self.current()?.begin_dispatch(&self.expected)?;
        self.prepared_verified = false;
        self.commit(next)?;
        Ok(DispatchAuthorization { store: self })
    }

    /// Unknown recovery can only inspect the bound exact ID. No mutation token
    /// is issued and no transition back to Prepared exists.
    pub fn recovery_request(&self) -> Result<Request> {
        if self.current()?.phase() != Phase::Unknown {
            return Err(Invalid("intent is not awaiting reconciliation"));
        }
        let Operation::Remove { object_id } = &self.expected.operation else {
            return Err(Invalid("intent requires removal"));
        };
        Ok(Request {
            binding: self.expected.binding.clone(),
            operation: Operation::Inspect {
                object_id: object_id.clone(),
            },
        })
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

    #[cfg(test)]
    fn fail_at(&self, stage: CommitStage) -> Result<()> {
        if self.fail_commit == Some(stage) {
            return Err(Invalid("injected intent commit failure"));
        }
        Ok(())
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
        #[cfg(test)]
        self.fail_at(CommitStage::FileSync)?;
        io(temporary.as_file().sync_all())?;
        self.check_directory()?;
        #[cfg(test)]
        self.fail_at(CommitStage::Publish)?;
        if self.bytes.is_none() {
            temporary
                .persist_noclobber(self.root.join(RECORD))
                .map_err(|_| Invalid("cannot create intent record; reopen required"))?;
        } else {
            temporary
                .persist(self.root.join(RECORD))
                .map_err(|_| Invalid("cannot commit intent record; reopen required"))?;
        }
        #[cfg(test)]
        self.fail_at(CommitStage::DirectorySync)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent::Intent;
    use crate::{Binding, Engine, Namespace, Object, Outcome, Protocol, Response};
    use serde_json::json;
    use std::io::{BufRead, BufReader};
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Child, Command, Stdio};
    use tempfile::TempDir;

    // These are real temporary-filesystem tests. I/O failures are injected with
    // permissions; no power-loss or hostile same-user claim is made.

    fn id() -> String {
        "a".repeat(64)
    }

    fn binding() -> Binding {
        Binding {
            protocol: Protocol::V1,
            adapter_sha256: "b".repeat(64),
            adapter_version: "1.0.0".into(),
            invocation: "inv-1".into(),
            operation_key: "op-1".into(),
            generation: "gen-1".into(),
            namespace: Namespace {
                engine: Engine::Docker,
                endpoint: "unix:///run/docker.sock".into(),
                context: "ctx-1".into(),
                principal: "uid:1000".into(),
                rootless: true,
                scope: "scope-1".into(),
            },
        }
    }

    fn removal() -> Request {
        Request {
            binding: binding(),
            operation: Operation::Remove { object_id: id() },
        }
    }

    fn labels() -> Vec<Label> {
        vec![Label {
            key: "harness-gate.owner".into(),
            value: "harness-gate".into(),
        }]
    }

    fn present() -> Response {
        Response {
            binding: binding(),
            operation: Operation::Inspect { object_id: id() },
            outcome: Outcome::Present {
                object: Object {
                    object_id: id(),
                    labels: labels(),
                },
            },
        }
    }

    fn absent() -> Response {
        Response {
            binding: binding(),
            operation: Operation::Inspect { object_id: id() },
            outcome: Outcome::Absent,
        }
    }

    fn private_root() -> (TempDir, PathBuf) {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().join("intent");
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        (temporary, fs::canonicalize(directory).unwrap())
    }

    fn open(root: &Path) -> Store {
        Store::open(root, &removal(), Duration::from_secs(1), || false).unwrap()
    }

    fn prepared_record() -> Vec<u8> {
        serde_json::to_vec(&Intent::prepare(&removal(), &present(), &labels()).unwrap()).unwrap()
    }

    fn write_record(root: &Path, bytes: &[u8]) {
        fs::write(root.join(RECORD), bytes).unwrap();
        fs::set_permissions(root.join(RECORD), fs::Permissions::from_mode(0o600)).unwrap();
    }

    fn prepare(store: &mut Store) {
        store.prepare(&present(), &labels()).unwrap();
    }

    fn open_prepared(root: &Path) {
        let mut store = open(root);
        prepare(&mut store);
    }

    fn binding_mutations() -> Vec<Binding> {
        let mut values = Vec::new();
        let mutations: [fn(&mut Binding); 11] = [
            |b| b.adapter_sha256 = "c".repeat(64),
            |b| b.adapter_version = "2".into(),
            |b| b.invocation = "inv-2".into(),
            |b| b.operation_key = "op-2".into(),
            |b| b.generation = "gen-2".into(),
            |b| b.namespace.engine = Engine::Podman,
            |b| b.namespace.endpoint = "unix:///other".into(),
            |b| b.namespace.context = "ctx-2".into(),
            |b| b.namespace.principal = "uid:1001".into(),
            |b| b.namespace.rootless = false,
            |b| b.namespace.scope = "scope-2".into(),
        ];
        for mutate in mutations {
            let mut value = binding();
            mutate(&mut value);
            values.push(value);
        }
        values
    }

    struct LockChild(Child);

    impl Drop for LockChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn hold_lock(root: &Path) -> LockChild {
        let mut child = Command::new("python3")
            .args([
                "-c",
                "import fcntl,os,sys,time\nfd=os.open(sys.argv[1],os.O_RDONLY|os.O_DIRECTORY)\nfcntl.flock(fd,fcntl.LOCK_EX|fcntl.LOCK_NB)\nprint('ready',flush=True)\ntime.sleep(30)",
            ])
            .arg(root)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut line = String::new();
        BufReader::new(stdout).read_line(&mut line).unwrap();
        assert_eq!(line.trim(), "ready");
        LockChild(child)
    }

    #[test]
    fn prepare_unknown_reopen_reconcile_completed_cycle() {
        let (_temporary, root) = private_root();
        {
            let mut store = open(&root);
            assert_eq!(store.phase(), None);
            assert!(store.begin_dispatch().is_err());
            prepare(&mut store);
            assert_eq!(store.phase(), Some(Phase::Prepared));
            assert!(store.prepare(&present(), &labels()).is_err());
        }
        {
            let mut store = open(&root);
            assert_eq!(store.phase(), Some(Phase::Prepared));
            store.begin_dispatch().unwrap();
            assert_eq!(store.phase(), Some(Phase::Unknown));
        }
        let mut store = open(&root);
        assert_eq!(store.phase(), Some(Phase::Unknown));
        store.observe_dispatch(None).unwrap();
        assert_eq!(store.phase(), Some(Phase::Unknown));
        store.reconcile(&absent()).unwrap();
        assert_eq!(store.phase(), Some(Phase::Completed));
        assert!(store.begin_dispatch().is_err());
        assert!(store.observe_dispatch(None).is_err());
        assert!(store.reconcile(&absent()).is_err());
        drop(store);
        let store = open(&root);
        assert_eq!(store.phase(), Some(Phase::Completed));
    }

    #[test]
    fn conflicting_expected_is_rejected() {
        let (_temporary, root) = private_root();
        open_prepared(&root);
        for changed in binding_mutations() {
            let expected = Request {
                binding: changed,
                operation: removal().operation,
            };
            assert!(Store::open(&root, &expected, Duration::from_secs(1), || false).is_err());
        }
        let expected = Request {
            binding: binding(),
            operation: Operation::Remove {
                object_id: "b".repeat(64),
            },
        };
        assert!(Store::open(&root, &expected, Duration::from_secs(1), || false).is_err());
    }

    #[test]
    fn lock_timeout_and_cancel_are_bounded_in_a_subprocess() {
        let (_temporary, root) = private_root();
        let child = hold_lock(&root);
        assert!(Store::open(&root, &removal(), Duration::from_millis(100), || false).is_err());
        drop(child);
        let store = open(&root);
        drop(store);

        let child = hold_lock(&root);
        let calls = std::cell::Cell::new(0_u32);
        let result = Store::open(&root, &removal(), Duration::from_secs(5), || {
            let current = calls.get();
            calls.set(current + 1);
            current > 0
        });
        assert!(result.is_err());
        drop(child);
    }

    #[test]
    fn record_inputs_reject_links_fifo_modes_size_and_legacy() {
        type Case = (&'static str, fn(&Path));
        let cases: [Case; 6] = [
            ("symlink", |root| {
                let target = root.join("record-target");
                write_record(root, &prepared_record());
                fs::rename(root.join(RECORD), &target).unwrap();
                std::os::unix::fs::symlink(&target, root.join(RECORD)).unwrap();
            }),
            ("hardlink", |root| {
                let target = root.join("record-target");
                write_record(root, &prepared_record());
                fs::hard_link(root.join(RECORD), &target).unwrap();
            }),
            ("fifo", |root| {
                let status = Command::new("mkfifo")
                    .arg(root.join(RECORD))
                    .status()
                    .unwrap();
                assert!(status.success());
            }),
            ("mode", |root| {
                write_record(root, &prepared_record());
                fs::set_permissions(root.join(RECORD), fs::Permissions::from_mode(0o644)).unwrap();
            }),
            ("oversize", |root| {
                let mut bytes = prepared_record();
                bytes.extend(std::iter::repeat_n(b' ', MAX_MESSAGE_BYTES));
                write_record(root, &bytes);
            }),
            ("legacy-v2", |root| {
                write_record(
                    root,
                    &serde_json::to_vec(&json!({
                        "owner_marker": "harness-gate",
                        "schema_version": 2,
                        "runtime": "docker",
                        "runtime_object_id": id()
                    }))
                    .unwrap(),
                );
            }),
        ];
        for (name, setup) in cases {
            let (_temporary, root) = private_root();
            setup(&root);
            assert!(
                Store::open(&root, &removal(), Duration::from_secs(1), || false).is_err(),
                "{name}"
            );
        }
    }

    #[test]
    fn record_change_and_directory_rebind_are_rejected() {
        let (_temporary, root) = private_root();
        open_prepared(&root);
        let mut store = open(&root);
        fs::write(root.join(RECORD), b"{}").unwrap();
        assert!(store.begin_dispatch().is_err());
        assert!(store.reconcile(&absent()).is_err());
        drop(store);

        let (_temporary, root) = private_root();
        let moved = root.parent().unwrap().join("moved");
        let mut store = open(&root);
        fs::rename(&root, &moved).unwrap();
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(store.prepare(&present(), &labels()).is_err());
    }

    #[test]
    fn initial_noclobber_and_commit_failure_poison_until_reopen() {
        let (_temporary, root) = private_root();
        let mut store = open(&root);
        write_record(&root, &prepared_record());
        assert!(store.prepare(&present(), &labels()).is_err());

        let (_temporary, root) = private_root();
        let mut store = open(&root);
        fs::set_permissions(&root, fs::Permissions::from_mode(0o500)).unwrap();
        assert!(store.prepare(&present(), &labels()).is_err());
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(store.begin_dispatch().is_err());
        assert!(store.prepare(&present(), &labels()).is_err());
        drop(store);
        let mut store = open(&root);
        assert_eq!(store.phase(), None);
        prepare(&mut store);
        assert_eq!(store.phase(), Some(Phase::Prepared));
    }
}
