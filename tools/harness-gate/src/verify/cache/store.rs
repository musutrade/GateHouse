//! Private host namespace, authenticated records, bounded atomic publication.
use super::identity;
use crate::{config::StepConfig, process::TaskResult, project::Project};
use anyhow::{ensure, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use ed25519_dalek::{Signature, Signer, SigningKey};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    ffi::CString,
    fs::{self, File},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::MetadataExt,
    },
    path::Path,
};

const PREFIX: &str = "gh-step-v1-";
const KEY: &str = "step-cache-v1.key";
const MAX_ENTRY: u64 = 8 * 1024 * 1024;
const MAX_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ENTRIES: usize = 128;
const MAX_AGE_SECONDS: i64 = 7 * 24 * 60 * 60;

pub(super) struct Store {
    ledger: crate::process::replay::Ledger,
    signing: SigningKey,
}
struct Lock(File);
impl Drop for Lock {
    fn drop(&mut self) {
        unsafe {
            libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    version: String,
    key: String,
    pub step: String,
    pub invocation: String,
    created: i64,
    pub execution_ms: u128,
    pub outputs: BTreeMap<String, String>,
    pub log: String,
    pub log_digest: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    record: Record,
    signature: String,
}

impl Store {
    pub(super) fn open(path: &Path) -> Result<Self> {
        ensure!(path.is_absolute(), "cache directory must be absolute");
        let ledger = crate::process::replay::Ledger::open(path)?;
        ensure!(
            ledger.directory.metadata()?.mode() & 0o077 == 0,
            "cache directory must be private (0700)"
        );
        let lock = lock(&ledger)?;
        let key = match open_file(&ledger, KEY, libc::O_RDONLY) {
            Ok(mut file) => {
                let mut bytes = Vec::new();
                Read::by_ref(&mut file).take(33).read_to_end(&mut bytes)?;
                <[u8; 32]>::try_from(bytes).map_err(|_| anyhow::anyhow!("invalid cache key"))?
            }
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
            {
                // Do not adopt a directory containing unrelated files or abandoned entries.
                ensure!(
                    fs::read_dir(path)?
                        .all(|v| v.is_ok_and(|e| e.file_name() == "step-cache-v1.lock")),
                    "cache namespace is not empty"
                );
                let mut bytes = [0; 32];
                File::open("/dev/urandom")?.read_exact(&mut bytes)?;
                let mut file =
                    open_file(&ledger, KEY, libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL)?;
                file.write_all(&bytes)?;
                file.sync_all()?;
                ledger.directory.sync_all()?;
                bytes
            }
            Err(error) => return Err(error),
        };
        drop(lock);
        Ok(Self {
            ledger,
            signing: SigningKey::from_bytes(&key),
        })
    }

    pub(super) fn key(&self, identity: &[u8]) -> String {
        // A deterministic keyed identity prevents reports from exposing low-entropy secrets.
        let mut input = b"gatehouse-step-cache-key/v1\0".to_vec();
        input.extend_from_slice(identity);
        identity::digest(&self.signing.sign(&input).to_bytes())
    }

    pub(super) fn load(&self, key: &str) -> Result<Option<Record>> {
        let _lock = lock(&self.ledger)?;
        let name = entry_name(key)?;
        match self.read(&name) {
            Ok(record) => {
                ensure!(
                    record.key == key && fresh(&record),
                    "cache identity or age mismatch"
                );
                Ok(Some(record))
            }
            Err(error)
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    fn read(&self, name: &str) -> Result<Record> {
        let mut file = open_file(&self.ledger, name, libc::O_RDONLY)?;
        ensure!(
            file.metadata()?.len() <= MAX_ENTRY,
            "cache entry size limit"
        );
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take(MAX_ENTRY + 1)
            .read_to_end(&mut bytes)?;
        ensure!(bytes.len() as u64 <= MAX_ENTRY, "cache entry grew");
        let envelope: Envelope = serde_json::from_slice(&bytes)?;
        let signature = Signature::from_slice(&STANDARD.decode(envelope.signature)?)?;
        let message = signed_bytes(&envelope.record)?;
        self.signing
            .verifying_key()
            .verify_strict(&message, &signature)?;
        ensure!(
            envelope.record.version == "step-cache/v1"
                && (entry_name(&envelope.record.key)? == name
                    || name == format!("{PREFIX}pending"))
                && !envelope.record.invocation.is_empty(),
            "invalid cache ownership"
        );
        Ok(envelope.record)
    }

    pub(super) fn save(
        &self,
        project: &Project,
        step: &StepConfig,
        result: &TaskResult,
        key: &str,
    ) -> Result<()> {
        let outputs = identity::outputs(project, step)?;
        let bytes = fs::read(&result.log)?;
        ensure!(bytes.len() <= 2 * 1024 * 1024, "cache log size limit");
        // Cache copies are safe to replay into a fresh report. Parsing is repeated on a hit.
        let text = std::str::from_utf8(&bytes)?;
        let log = crate::utils::redaction::redact_text(text).into_bytes();
        ensure!(
            log == bytes,
            "sensitive or truncated logs are not cacheable"
        );
        let record = Record {
            version: "step-cache/v1".into(),
            key: key.into(),
            step: step.id.clone(),
            invocation: project.invocation_id(),
            created: chrono::Utc::now().timestamp(),
            execution_ms: result.duration_ms,
            outputs,
            log_digest: identity::digest(&log),
            log: STANDARD.encode(log),
        };
        let signature = STANDARD.encode(self.signing.sign(&signed_bytes(&record)?).to_bytes());
        let bytes = serde_json::to_vec(&Envelope { record, signature })?;
        ensure!(bytes.len() as u64 <= MAX_ENTRY, "cache entry size limit");
        let _lock = lock(&self.ledger)?;
        ensure!(!crate::process::cancelled(), "cancelled cache publication");
        let name = entry_name(key)?;
        // An invalid existing file is never overwritten or deleted as owned material.
        if open_file(&self.ledger, &name, libc::O_RDONLY).is_ok() {
            self.read(&name)?;
        }
        let temporary = format!("{PREFIX}pending");
        if fs::symlink_metadata(self.ledger.path.join(&temporary)).is_ok() {
            self.remove(&temporary)?;
        }
        self.evict(bytes.len() as u64, &name)?;
        // Exclusive creation proves ownership. A partial file from a crash is retained.
        let mut file = open_file(
            &self.ledger,
            &temporary,
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
        )?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        ensure!(!crate::process::cancelled(), "cancelled cache publication");
        let from = CString::new(temporary)?;
        let to = CString::new(name)?;
        let fd = self.ledger.directory.as_raw_fd();
        ensure!(
            unsafe { libc::renameat(fd, from.as_ptr(), fd, to.as_ptr()) } == 0,
            "cache publication failed"
        );
        self.ledger.directory.sync_all()?;
        self.ledger.validate()?;
        Ok(())
    }

    fn entries(&self) -> Result<Vec<(String, Record, u64)>> {
        self.ledger.validate()?;
        let mut entries = Vec::new();
        let mut examined = 0;
        for item in fs::read_dir(&self.ledger.path)? {
            examined += 1;
            ensure!(examined <= 1024, "cache directory entry limit");
            let item = item?;
            let name = item.file_name().to_string_lossy().into_owned();
            if name.starts_with(PREFIX) && name.ends_with(".json") {
                // Unknown/tampered entries count as occupied space and block automatic mutation.
                let record = self.read(&name)?;
                entries.push((name, record, item.metadata()?.len()));
            }
        }
        entries.sort_by_key(|(name, r, _)| (r.created, name.clone()));
        Ok(entries)
    }

    fn evict(&self, incoming: u64, replacement: &str) -> Result<()> {
        let entries = self.entries()?;
        let mut bytes: u64 = entries.iter().map(|v| v.2).sum();
        let mut count = entries.len();
        for (name, record, size) in entries {
            if name == replacement
                || !fresh(&record)
                || bytes + incoming > MAX_BYTES
                || count >= MAX_ENTRIES
            {
                self.remove(&name)?;
                bytes -= size;
                count -= 1;
            }
        }
        ensure!(
            bytes + incoming <= MAX_BYTES && count < MAX_ENTRIES,
            "cache capacity exhausted"
        );
        Ok(())
    }

    fn remove(&self, name: &str) -> Result<()> {
        // Verify the signed ownership record immediately before unlinking this one file.
        self.read(name)?;
        let name = CString::new(name)?;
        ensure!(
            unsafe { libc::unlinkat(self.ledger.directory.as_raw_fd(), name.as_ptr(), 0) } == 0,
            "cache unlink failed"
        );
        self.ledger.directory.sync_all()?;
        Ok(())
    }

    pub(super) fn clear(&self) -> Result<serde_json::Value> {
        let _lock = lock(&self.ledger)?;
        let mut removed = 0;
        let mut retained = 0;
        let mut examined = 0;
        for item in fs::read_dir(&self.ledger.path)? {
            examined += 1;
            ensure!(examined <= 1024, "cache directory entry limit");
            let name = item?.file_name().to_string_lossy().into_owned();
            if name == KEY || name == "step-cache-v1.lock" {
                continue;
            }
            if name.starts_with(PREFIX)
                && (name.ends_with(".json") || name == format!("{PREFIX}pending"))
                && self.remove(&name).is_ok()
            {
                removed += 1;
            } else {
                retained += 1;
            }
        }
        Ok(
            serde_json::json!({"schema_version":"step-cache-maintenance/v1", "removed":removed, "retained":retained}),
        )
    }
}

fn fresh(record: &Record) -> bool {
    let age = chrono::Utc::now().timestamp() - record.created;
    (0..=MAX_AGE_SECONDS).contains(&age)
}
fn entry_name(key: &str) -> Result<String> {
    ensure!(
        key.len() == 64
            && key
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "invalid cache key"
    );
    Ok(format!("{PREFIX}{key}.json"))
}
fn signed_bytes(record: &Record) -> Result<Vec<u8>> {
    let mut bytes = b"gatehouse-step-cache-record/v1\0".to_vec();
    bytes.extend_from_slice(&serde_json::to_vec(record)?);
    Ok(bytes)
}
fn lock(ledger: &crate::process::replay::Ledger) -> Result<Lock> {
    ledger.validate()?;
    let file = open_file(ledger, "step-cache-v1.lock", libc::O_RDWR | libc::O_CREAT)?;
    ensure!(
        unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
        "cache busy; execute normally"
    );
    Ok(Lock(file))
}
fn open_file(ledger: &crate::process::replay::Ledger, name: &str, flags: i32) -> Result<File> {
    let name = CString::new(name)?;
    let fd = unsafe {
        libc::openat(
            ledger.directory.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
            0o600,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let file = unsafe { File::from_raw_fd(fd) };
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file()
            && metadata.uid() == unsafe { libc::geteuid() }
            && metadata.mode() & 0o077 == 0
            && metadata.nlink() == 1,
        "cache file is not private owned regular material"
    );
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestWorkspace;

    fn record(key: &str) -> Record {
        Record {
            version: "step-cache/v1".into(),
            key: key.into(),
            step: "check".into(),
            invocation: "fixture-invocation".into(),
            created: chrono::Utc::now().timestamp(),
            execution_ms: 100,
            outputs: BTreeMap::new(),
            log: String::new(),
            log_digest: identity::digest(b""),
        }
    }
    fn put(store: &Store, record: Record) -> String {
        let name = entry_name(&record.key).unwrap();
        let signature = STANDARD.encode(
            store
                .signing
                .sign(&signed_bytes(&record).unwrap())
                .to_bytes(),
        );
        let bytes = serde_json::to_vec(&Envelope { record, signature }).unwrap();
        let mut file = open_file(
            &store.ledger,
            &name,
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
        )
        .unwrap();
        file.write_all(&bytes).unwrap();
        name
    }

    #[test]
    fn expiry_capacity_and_authenticated_clear_are_bounded() {
        let host = TestWorkspace::new("cache-capacity");
        let store = Store::open(&host.root.canonicalize().unwrap().join("cache")).unwrap();
        for i in 0..MAX_ENTRIES {
            let key = format!("{i:064x}");
            let mut record = record(&key);
            if i == 0 {
                record.created -= MAX_AGE_SECONDS + 1;
            }
            put(&store, record);
        }
        assert!(store.load(&format!("{:064x}", 0)).is_err());
        assert!(store.load(&format!("{:064x}", 1)).unwrap().is_some());
        let _lock = lock(&store.ledger).unwrap();
        store.evict(1024, "new").unwrap();
        let entries = store.entries().unwrap();
        assert!(entries.len() < MAX_ENTRIES);
        assert!(!entries
            .iter()
            .any(|(_, r, _)| r.key == format!("{:064x}", 0)));
        // The byte budget evicts until the requested publication fits.
        store.evict(MAX_BYTES, "new").unwrap();
        assert!(store.entries().unwrap().is_empty());
        drop(_lock);
        put(&store, record(&format!("{:064x}", 200)));
        assert_eq!(store.clear().unwrap()["removed"], 1);
    }

    #[test]
    fn wrong_key_links_partial_files_and_unknown_ownership_are_preserved() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let host = TestWorkspace::new("cache-integrity");
        let root = host.root.canonicalize().unwrap().join("cache");
        let store = Store::open(&root).unwrap();
        let name = put(&store, record(&format!("{:064x}", 1)));
        let outside = host.root.join("outside");
        fs::write(&outside, "untouched").unwrap();
        fs::set_permissions(&outside, fs::Permissions::from_mode(0o600)).unwrap();
        let link = entry_name(&format!("{:064x}", 2)).unwrap();
        symlink(&outside, root.join(&link)).unwrap();
        let hardlink = entry_name(&format!("{:064x}", 3)).unwrap();
        fs::hard_link(&outside, root.join(&hardlink)).unwrap();
        fs::write(root.join(format!("{PREFIX}pending")), "partial").unwrap();
        assert!(store.load(&format!("{:064x}", 2)).is_err());
        assert!(store.load(&format!("{:064x}", 3)).is_err());
        let report = store.clear().unwrap();
        assert_eq!(report["removed"], 1);
        assert_eq!(report["retained"], 3);
        assert!(!root.join(name).exists());
        assert_eq!(fs::read_to_string(&outside).unwrap(), "untouched");
        assert!(root
            .join(&link)
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(
            Store::open(&host.root).is_err(),
            "never adopt a populated host directory"
        );
        let other = Store::open(&host.root.canonicalize().unwrap().join("other-cache")).unwrap();
        let key = format!("{:064x}", 10);
        let record = record(&key);
        let name = put(&store, record);
        fs::copy(root.join(&name), other.ledger.path.join(&name)).unwrap();
        fs::set_permissions(
            other.ledger.path.join(&name),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        assert!(
            other.load(&key).is_err(),
            "another host key must not authenticate copied entries"
        );
    }

    #[test]
    fn lock_contention_and_host_permissions_fail_to_normal_execution() {
        use std::os::unix::fs::PermissionsExt;
        let host = TestWorkspace::new("cache-lock");
        let path = host.root.canonicalize().unwrap().join("cache");
        let store = Store::open(&path).unwrap();
        let _lock = lock(&store.ledger).unwrap();
        assert!(Store::open(&path).is_err());
        assert!(store.load(&format!("{:064x}", 1)).is_err());
        drop(_lock);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(Store::open(&path).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(Store::open(&path).is_ok());
    }
}
