mod container;

use container::{verify_runtime_record, RetiredRuntimeOperations, RuntimeOperations};

use super::report_directory::ReportDirectoryGuard;
use crate::config::ContainerRuntimeKind;
use crate::project::Project;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub(crate) const LEASE_SCHEMA_VERSION: u32 = 2;
/// Cleanup evidence has its own public schema.  Lease records may evolve
/// independently without invalidating consumers of `cleanup.json`.
pub(crate) const CLEANUP_REPORT_SCHEMA_VERSION: u32 = 1;
pub(crate) const OWNER_MARKER: &str = "harness-gate";
pub(crate) const LABEL_OWNER: &str = "harness-gate.owner";
pub(crate) const LABEL_SCHEMA: &str = "harness-gate.schema";
pub(crate) const LABEL_PROJECT: &str = "harness-gate.project";
pub(crate) const LABEL_RESOURCE: &str = "harness-gate.resource";
pub(crate) const LABEL_KIND: &str = "harness-gate.kind";
pub(crate) const LABEL_INVOCATION: &str = "harness-gate.invocation";
const LEASE_TTL: Duration = Duration::from_secs(15 * 60);
const RENEW_AFTER: Duration = Duration::from_secs(30);
#[cfg(not(test))]
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);
#[cfg(test)]
const HEARTBEAT_INTERVAL: Duration = Duration::from_millis(250);
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LeaseRecord {
    pub(crate) owner_marker: String,
    pub(crate) schema_version: u32,
    pub(crate) project_identity: String,
    pub(crate) resource_id: String,
    pub(crate) resource_kind: String,
    pub(crate) invocation_id: String,
    pub(crate) pid: u32,
    pub(crate) process_start_identity: String,
    pub(crate) created_at: u64,
    pub(crate) heartbeat_at: u64,
    pub(crate) expires_at: u64,
    #[serde(default)]
    pub(crate) resource_name: Option<String>,
    #[serde(default)]
    pub(crate) runtime: Option<String>,
    pub(crate) runtime_labels: BTreeMap<String, String>,
    pub(crate) runtime_object_id: Option<String>,
}

#[derive(Debug)]
pub(crate) struct ResourceLease {
    path: PathBuf,
    record: Arc<Mutex<LeaseRecord>>,
    heartbeat_error: Arc<Mutex<Option<String>>>,
    stop_heartbeat: Option<mpsc::Sender<()>>,
    heartbeat: Option<JoinHandle<()>>,
    release_on_drop: bool,
    // Closed only after Drop stops heartbeat and attempts the checked release.
    report_directory_guard: Option<ReportDirectoryGuard>,
}

impl ResourceLease {
    pub(crate) fn acquire(
        project: &Project,
        resource_id: impl Into<String>,
        resource_kind: impl Into<String>,
        invocation_id: impl Into<String>,
        resource_name: Option<String>,
        runtime: Option<ContainerRuntimeKind>,
    ) -> Result<Self> {
        let resource_id = resource_id.into();
        let resource_kind = resource_kind.into();
        let invocation_id = invocation_id.into();
        let directory = lease_directory(project)?;
        fs::create_dir_all(&directory)
            .with_context(|| format!("create lease directory {}", directory.display()))?;
        let path = directory.join(format!("{}.json", resource_key(&resource_id)));
        let now = epoch_seconds();
        let project_identity = project.input().project_identity.clone();
        let runtime_labels = ownership_labels(
            &project_identity,
            &resource_id,
            &resource_kind,
            &invocation_id,
        );
        let record = LeaseRecord {
            owner_marker: OWNER_MARKER.into(),
            schema_version: LEASE_SCHEMA_VERSION,
            project_identity,
            resource_id: resource_id.clone(),
            resource_kind,
            invocation_id,
            pid: std::process::id(),
            process_start_identity: process_start_identity(std::process::id()),
            created_at: now,
            heartbeat_at: now,
            expires_at: now.saturating_add(LEASE_TTL.as_secs()),
            resource_name,
            runtime: runtime.map(|kind| kind.executable().to_string()),
            runtime_labels,
            runtime_object_id: None,
        };
        if !identity_is_proven(&record) {
            bail!(
                "LEASE_OWNERSHIP_UNCERTAIN: platform process identity is unavailable; resource allocation rejected"
            );
        }

        let report_directory_guard = if resource_kind_is_report(&record) {
            let guard = ReportDirectoryGuard::try_acquire(project, &record.invocation_id)?
                .ok_or_else(|| {
                    anyhow::anyhow!("report-directory lease conflict: resource is in use")
                })?;
            guard.validate_record(&record)?;
            // Invalidate any earlier release before creating/renewing a marker.
            // A failed allocation or failed release never certifies completion.
            guard.invalidate_release()?;
            Some(guard)
        } else {
            None
        };

        loop {
            match create_record(&path, &record) {
                Ok(()) => {
                    let record = Arc::new(Mutex::new(record));
                    let last_renewed = Arc::new(Mutex::new(Instant::now()));
                    let heartbeat_error = Arc::new(Mutex::new(None));
                    let (stop_heartbeat, receiver) = mpsc::channel();
                    let heartbeat_path = path.clone();
                    let heartbeat_record = Arc::clone(&record);
                    let heartbeat_last = Arc::clone(&last_renewed);
                    let heartbeat_failure = Arc::clone(&heartbeat_error);
                    let heartbeat = thread::spawn(move || loop {
                        match receiver.recv_timeout(HEARTBEAT_INTERVAL) {
                            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                            Err(mpsc::RecvTimeoutError::Timeout) => {
                                if let Err(error) = renew_parts(
                                    &heartbeat_path,
                                    &heartbeat_record,
                                    &heartbeat_last,
                                    true,
                                ) {
                                    if let Ok(mut failure) = heartbeat_failure.lock() {
                                        if failure.is_none() {
                                            *failure = Some(format!("{error:#}"));
                                        }
                                    }
                                }
                            }
                        }
                    });
                    return Ok(Self {
                        path,
                        record,
                        heartbeat_error,
                        stop_heartbeat: Some(stop_heartbeat),
                        heartbeat: Some(heartbeat),
                        release_on_drop: true,
                        report_directory_guard,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    let existing = read_record(&path).with_context(|| {
                        format!("inspect existing lease for resource {resource_id:?}")
                    })?;
                    validate_record(&existing, &resource_id, &path, &directory, project)?;
                    if resource_kind_is_report(&record) {
                        // Retained report markers must not be converted into
                        // fresh ownership through the generic expiry fallback.
                        bail!("report-directory marker is retained; explicit proven cleanup is required");
                    }
                    let state = lease_state(&existing, epoch_seconds());
                    if state == LeaseState::AliveIdentityUnknown {
                        bail!(
                            "LEASE_OWNERSHIP_UNCERTAIN: lease holder pid {} for {resource_id:?} is alive but its process identity cannot be read; resource retained",
                            existing.pid
                        );
                    }
                    if state != LeaseState::Stale {
                        bail!(
                            "resource lease conflict for {resource_id:?}: invocation {} (pid {}) owns it",
                            existing.invocation_id,
                            existing.pid
                        );
                    }
                    if !identity_is_proven(&existing) {
                        bail!(
                            "LEASE_OWNERSHIP_UNCERTAIN: stale lease identity cannot be proven; resource retained"
                        );
                    }
                    reclaim_resource(project, &path, &existing)?;
                }
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("create lease for resource {resource_id:?}"));
                }
            }
        }
    }

    /// Stop renewal while retaining the marker for operator cleanup. This is
    /// used when an external resource may have been created but its ownership
    /// or identity could not be proved.
    #[cfg(test)]
    pub(crate) fn retain(mut self) {
        self.stop_heartbeat();
        self.release_on_drop = false;
    }

    /// Stop the heartbeat before attempting an explicit release. If release
    /// cannot prove current ownership, dropping this value keeps the marker so
    /// an operator can inspect it instead of silently deleting it.
    #[cfg(test)]
    pub(crate) fn release_checked(mut self) -> Result<()> {
        self.finish_release()
    }

    fn finish_release(&mut self) -> Result<()> {
        self.stop_heartbeat();
        let result = self.release();
        self.release_on_drop = false;
        result
    }

    #[cfg(test)]
    pub(crate) fn release_before_guard_close(&mut self) -> Result<()> {
        // Exercise the real checked-release transition while a child process
        // pauses before dropping the same guard that production Drop closes.
        self.finish_release()
    }

    pub(crate) fn release(&self) -> Result<()> {
        self.ensure_heartbeat_healthy()?;
        if self.report_directory_guard.is_some() && self.heartbeat.is_some() {
            bail!("report-directory release requires stopped heartbeat");
        }
        let record = self
            .record
            .lock()
            .map_err(|_| anyhow::anyhow!("lease record lock was poisoned"))?;
        let contents = match fs::read(&self.path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if self.report_directory_guard.is_some() {
                    bail!("report-directory marker disappeared before checked release");
                }
                return Ok(());
            }
            Err(error) => return Err(error).with_context(|| "read lease before release"),
        };
        let current: LeaseRecord =
            serde_json::from_slice(&contents).context("parse lease before release")?;
        ensure_owner(&current, &record)?;
        // Persist the certificate while the JSON marker still protects the
        // resource. A certificate write/sync failure leaves that marker intact;
        // nothing fallible follows a successful marker removal.
        if let Some(guard) = &self.report_directory_guard {
            guard.mark_released(&record)?;
        }
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    && self.report_directory_guard.is_some() =>
            {
                if let Some(guard) = &self.report_directory_guard {
                    let _ = guard.invalidate_release();
                }
                Err(error).context("report-directory marker disappeared during release")
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => {
                Err(error).with_context(|| format!("release lease {}", self.path.display()))
            }
        }
    }

    fn ensure_heartbeat_healthy(&self) -> Result<()> {
        if let Some(error) = self
            .heartbeat_error
            .lock()
            .map_err(|_| anyhow::anyhow!("lease heartbeat error lock was poisoned"))?
            .as_ref()
            .cloned()
        {
            bail!("LEASE_OWNERSHIP_UNCERTAIN: lease heartbeat failed: {error}");
        }
        Ok(())
    }
}

impl Drop for ResourceLease {
    fn drop(&mut self) {
        if self.release_on_drop {
            let _ = self.finish_release();
        } else {
            self.stop_heartbeat();
        }
    }
}

impl ResourceLease {
    fn stop_heartbeat(&mut self) {
        if let Some(stop) = self.stop_heartbeat.take() {
            let _ = stop.send(());
        }
        if let Some(heartbeat) = self.heartbeat.take() {
            let _ = heartbeat.join();
        }
    }
}

fn renew_parts(
    path: &Path,
    record: &Mutex<LeaseRecord>,
    last_renewed: &Mutex<Instant>,
    force: bool,
) -> Result<()> {
    let mut last = last_renewed
        .lock()
        .map_err(|_| anyhow::anyhow!("lease renewal lock was poisoned"))?;
    if !force && last.elapsed() < RENEW_AFTER {
        return Ok(());
    }
    let mut record = record
        .lock()
        .map_err(|_| anyhow::anyhow!("lease record lock was poisoned"))?;
    let current = read_record(path).with_context(|| format!("read lease {}", path.display()))?;
    ensure_owner(&current, &record)?;
    let now = epoch_seconds();
    record.heartbeat_at = now;
    record.expires_at = now.saturating_add(LEASE_TTL.as_secs());
    write_record(path, &record)?;
    *last = Instant::now();
    Ok(())
}

#[derive(Debug, Serialize)]
pub(crate) struct CleanupReport {
    pub(crate) schema_version: u32,
    pub(crate) owner_marker: &'static str,
    pub(crate) dry_run: bool,
    pub(crate) scanned: usize,
    pub(crate) active: usize,
    pub(crate) stale: usize,
    pub(crate) reclaimed: usize,
    pub(crate) resources: Vec<CleanupResource>,
    pub(crate) failures: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct CleanupResource {
    pub(crate) resource_id: String,
    pub(crate) resource_kind: String,
    pub(crate) invocation_id: String,
    pub(crate) state: String,
    pub(crate) action: String,
    pub(crate) lease_file: String,
}

pub(crate) fn cleanup(project: &Project, dry_run: bool) -> Result<CleanupReport> {
    cleanup_with_runtime(project, dry_run, &RetiredRuntimeOperations)
}

fn cleanup_with_runtime<O: RuntimeOperations + ?Sized>(
    project: &Project,
    dry_run: bool,
    runtime: &O,
) -> Result<CleanupReport> {
    let directory = lease_directory(project)?;
    let mut report = CleanupReport {
        schema_version: CLEANUP_REPORT_SCHEMA_VERSION,
        owner_marker: OWNER_MARKER,
        dry_run,
        scanned: 0,
        active: 0,
        stale: 0,
        reclaimed: 0,
        resources: Vec::new(),
        failures: Vec::new(),
    };
    if !directory.is_dir() {
        return Ok(report);
    }
    for entry in fs::read_dir(&directory)
        .with_context(|| format!("read lease directory {}", directory.display()))?
    {
        let entry = entry.context("read lease entry")?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        report.scanned += 1;
        let record = match read_record(&path) {
            Ok(record) => record,
            Err(error) => {
                report.failures.push(format!("{}: {error}", path.display()));
                continue;
            }
        };
        if let Err(error) =
            validate_record(&record, &record.resource_id, &path, &directory, project)
        {
            // Unknown or malformed markers are intentionally never reclaimed,
            // but the failure is retained as structured cleanup evidence.
            report.failures.push(format!(
                "{}: ownership validation failed: {error:#}",
                path.display()
            ));
            continue;
        }
        // Report retention, allocation, release and operator cleanup share this
        // guard. A heartbeat's JSON replacement cannot look like a released lease.
        let report_guard = if resource_kind_is_report(&record) {
            match ReportDirectoryGuard::try_acquire(project, &record.invocation_id) {
                Ok(Some(guard)) => {
                    let checked = read_record(&path).and_then(|current| {
                        ensure_owner(&current, &record)?;
                        guard.validate_record(&current)
                    });
                    if let Err(error) = checked {
                        report
                            .failures
                            .push(format!("{}: {error:#}", path.display()));
                        continue;
                    }
                    Some(guard)
                }
                Ok(None) => {
                    report.active += 1;
                    report.resources.push(CleanupResource {
                        resource_id: record.resource_id,
                        resource_kind: record.resource_kind,
                        invocation_id: record.invocation_id,
                        state: "active".into(),
                        action: "retained".into(),
                        lease_file: path.display().to_string(),
                    });
                    continue;
                }
                Err(error) => {
                    report
                        .failures
                        .push(format!("{}: {error:#}", path.display()));
                    continue;
                }
            }
        } else {
            None
        };
        let stale = if report_guard.is_some() {
            // Expiry is not proof of death when process observation is unknown.
            match report_owner_has_ended(&record) {
                Some(ended) => ended,
                None => {
                    report.failures.push(format!(
                        "{}: LEASE_OWNERSHIP_UNCERTAIN: report owner state is unknown; retained",
                        path.display()
                    ));
                    continue;
                }
            }
        } else {
            match lease_state(&record, epoch_seconds()) {
                LeaseState::AliveIdentityUnknown => {
                    report.active += 1;
                    report.failures.push(format!(
                        "{}: LEASE_OWNERSHIP_UNCERTAIN: holder pid {} is alive but its process identity cannot be read; resource retained",
                        record.resource_id, record.pid
                    ));
                    report.resources.push(CleanupResource {
                        resource_id: record.resource_id,
                        resource_kind: record.resource_kind,
                        invocation_id: record.invocation_id,
                        state: "ownership-uncertain".into(),
                        action: "retained".into(),
                        lease_file: path.display().to_string(),
                    });
                    continue;
                }
                state => state == LeaseState::Stale,
            }
        };
        let lease_file = path.display().to_string();
        if !identity_is_proven(&record) {
            if stale {
                report.stale += 1;
            } else {
                report.active += 1;
            }
            report.failures.push(format!(
                "{}: LEASE_OWNERSHIP_UNCERTAIN: platform process identity is unavailable; resource retained",
                record.resource_id
            ));
            report.resources.push(CleanupResource {
                resource_id: record.resource_id,
                resource_kind: record.resource_kind,
                invocation_id: record.invocation_id,
                state: "ownership-uncertain".into(),
                action: "retained".into(),
                lease_file,
            });
            continue;
        }
        if !stale {
            report.active += 1;
            report.resources.push(CleanupResource {
                resource_id: record.resource_id,
                resource_kind: record.resource_kind,
                invocation_id: record.invocation_id,
                state: "active".into(),
                action: "保留".into(),
                lease_file,
            });
            continue;
        }
        report.stale += 1;
        let mut action = if dry_run {
            "would-reclaim"
        } else {
            "reclaimed"
        };
        if dry_run && record.resource_kind == "container" {
            action = "retained";
            report.failures.push(format!(
                "legacy container {}: CLI service operations are retired; lease retained without migration; resolve through the prior approved version or explicit operator procedure",
                record.resource_id
            ));
        } else if !dry_run {
            if let Err(error) = reclaim_resource_with_runtime(project, &path, &record, runtime) {
                action = "failed";
                report.failures.push(format!(
                    "reclaim {} ({}) failed: {error:#}",
                    record.resource_id, record.resource_kind
                ));
            } else {
                report.reclaimed += 1;
            }
        }
        report.resources.push(CleanupResource {
            resource_id: record.resource_id,
            resource_kind: record.resource_kind,
            invocation_id: record.invocation_id,
            state: "stale".into(),
            action: action.into(),
            lease_file,
        });
    }
    Ok(report)
}

fn reclaim_resource(project: &Project, path: &Path, record: &LeaseRecord) -> Result<()> {
    reclaim_resource_with_runtime(project, path, record, &RetiredRuntimeOperations)
}

fn reclaim_resource_with_runtime<O: RuntimeOperations + ?Sized>(
    project: &Project,
    path: &Path,
    record: &LeaseRecord,
    runtime_operations: &O,
) -> Result<()> {
    if record.resource_kind == "container" {
        let name = record
            .resource_name
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("container lease has no resource name"))?;
        if !name.starts_with("harness-gate-") {
            bail!("container lease resource name is not Harness-Gate managed");
        }
        let runtime = match record.runtime.as_deref() {
            Some("docker") => ContainerRuntimeKind::Docker,
            Some("podman") => ContainerRuntimeKind::Podman,
            Some(value) => bail!("unsupported container runtime {value:?}"),
            None => bail!("container lease has no runtime"),
        };
        let object_id = verify_runtime_record(runtime_operations, project, path, record)?;
        runtime_operations
            .stop(runtime, &project.root, &object_id, Duration::from_secs(5))
            .with_context(|| format!("stop owned container {name:?}"))?;
    }
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("remove lease {}", path.display())),
    }
}

fn create_record(path: &Path, record: &LeaseRecord) -> std::io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    let contents = serde_json::to_vec_pretty(record)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    file.write_all(&contents)?;
    file.write_all(b"\n")?;
    file.sync_all()
}

fn read_record(path: &Path) -> Result<LeaseRecord> {
    let contents = fs::read(path).with_context(|| format!("read lease {}", path.display()))?;
    serde_json::from_slice(&contents).with_context(|| format!("parse lease {}", path.display()))
}

fn write_record(path: &Path, record: &LeaseRecord) -> Result<()> {
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temporary = path.with_file_name(format!(".lease-{}-{counter}.tmp", std::process::id()));
    // A failed create_new grants no ownership of this path. In particular,
    // cleanup must never unlink another writer's pre-existing temporary.
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .with_context(|| format!("create temporary lease {}", temporary.display()))?;
    let publish_from = temporary.as_path();
    // This closure owns the successfully created handle. Every early error
    // closes it before the outer cleanup, including on Windows.
    let result = (move || -> Result<()> {
        let contents = serde_json::to_vec_pretty(record).context("serialize lease")?;
        file.write_all(&contents)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);
        publish_replacement(publish_from, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn publish_replacement(temporary: &Path, target: &Path) -> Result<()> {
    #[cfg(windows)]
    if fs::symlink_metadata(target).is_ok() {
        fs::remove_file(target)
            .with_context(|| format!("replace existing lease {}", target.display()))?;
    }
    fs::rename(temporary, target).with_context(|| format!("publish lease {}", target.display()))?;
    Ok(())
}

fn validate_record(
    record: &LeaseRecord,
    resource_id: &str,
    path: &Path,
    lease_directory: &Path,
    project: &Project,
) -> Result<()> {
    if record.owner_marker != OWNER_MARKER {
        bail!("lease owner marker is not Harness-Gate");
    }
    if record.schema_version != LEASE_SCHEMA_VERSION {
        bail!("unsupported lease schema version {}", record.schema_version);
    }
    if record.resource_id != resource_id {
        bail!("lease resource identity mismatch");
    }
    let expected_name = format!("{}.json", resource_key(&record.resource_id));
    if path.file_name().and_then(|name| name.to_str()) != Some(expected_name.as_str()) {
        bail!("lease filename does not match the deterministic resource key");
    }
    if path.parent() != Some(lease_directory) {
        bail!("lease is not directly inside the project lease directory");
    }
    if record.project_identity != project.input().project_identity {
        bail!("lease project identity does not match the current project");
    }
    if record.resource_id.trim().is_empty()
        || record.resource_kind.trim().is_empty()
        || record.invocation_id.trim().is_empty()
    {
        bail!("lease ownership fields must be non-empty");
    }
    if record.resource_kind == "container" {
        if record
            .resource_name
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            bail!("container lease has no resource name");
        }
        if record.runtime.is_none() {
            bail!("container lease has no runtime");
        }
        if record
            .runtime_object_id
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            bail!("container lease has no immutable runtime object ID");
        }
        let expected = ownership_labels(
            &record.project_identity,
            &record.resource_id,
            &record.resource_kind,
            &record.invocation_id,
        );
        for (key, value) in expected {
            if record.runtime_labels.get(&key) != Some(&value) {
                bail!("container lease is missing expected runtime label {key:?}");
            }
        }
    }
    Ok(())
}

fn ensure_owner(current: &LeaseRecord, expected: &LeaseRecord) -> Result<()> {
    if current.owner_marker != OWNER_MARKER
        || current.schema_version != LEASE_SCHEMA_VERSION
        || current.resource_id != expected.resource_id
        || current.project_identity != expected.project_identity
        || current.resource_kind != expected.resource_kind
    {
        bail!("lease ownership changed while operating on the resource");
    }
    if current.invocation_id != expected.invocation_id
        || current.pid != expected.pid
        || current.process_start_identity != expected.process_start_identity
    {
        bail!("lease ownership changed while operating on the resource");
    }
    Ok(())
}

/// Ownership state of an existing lease as observed by this host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LeaseState {
    Active,
    Stale,
    /// The holder PID is alive but its start identity cannot be read, so PID
    /// reuse cannot be distinguished from the original owner. Expiry is not
    /// proof of death here; the lease must be retained (#311).
    AliveIdentityUnknown,
}

#[cfg(test)]
fn is_stale(record: &LeaseRecord, now: u64) -> bool {
    lease_state(record, now) == LeaseState::Stale
}

#[cfg(not(test))]
fn lease_state(record: &LeaseRecord, now: u64) -> LeaseState {
    classify_lease(record, now, process_alive(record.pid), || {
        process_start_identity_checked(record.pid)
    })
}

/// Test builds may replace host process observation for one thread.
#[cfg(test)]
fn lease_state(record: &LeaseRecord, now: u64) -> LeaseState {
    let observe = tests::observation_for(std::thread::current().id());
    match observe {
        Some(observe) => {
            let (alive, identity) = observe(record.pid);
            classify_lease(record, now, alive, || identity)
        }
        None => classify_lease(record, now, process_alive(record.pid), || {
            process_start_identity_checked(record.pid)
        }),
    }
}

fn classify_lease(
    record: &LeaseRecord,
    now: u64,
    alive: Option<bool>,
    identity: impl FnOnce() -> Option<String>,
) -> LeaseState {
    let expired = if now > record.expires_at {
        LeaseState::Stale
    } else {
        LeaseState::Active
    };
    match alive {
        Some(true) => match identity() {
            Some(identity) if identity != record.process_start_identity => LeaseState::Stale,
            Some(_) => LeaseState::Active,
            None => LeaseState::AliveIdentityUnknown,
        },
        Some(false) => LeaseState::Stale,
        None => expired,
    }
}

fn identity_is_proven(record: &LeaseRecord) -> bool {
    #[cfg(target_os = "linux")]
    {
        record
            .process_start_identity
            .strip_prefix("linux:")
            .and_then(|value| value.parse::<u64>().ok())
            .is_some_and(|value| value > 0)
    }
    #[cfg(target_os = "macos")]
    {
        let mut fields = record.process_start_identity.split(':');
        let prefix = fields.next();
        let seconds = fields.next().and_then(|value| value.parse::<u64>().ok());
        let micros = fields.next().and_then(|value| value.parse::<u64>().ok());
        matches!(fields.next(), None)
            && matches!(prefix, Some("macos"))
            && seconds
                .zip(micros)
                .is_some_and(|(seconds, micros)| seconds > 0 || micros > 0)
    }
    #[cfg(target_os = "windows")]
    {
        record
            .process_start_identity
            .strip_prefix("windows:")
            .and_then(|value| value.parse::<u64>().ok())
            .is_some_and(|value| value > 0)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        false
    }
}

fn resource_kind_is_report(record: &LeaseRecord) -> bool {
    record.resource_kind == "report-directory"
}

fn report_owner_has_ended(record: &LeaseRecord) -> Option<bool> {
    if !identity_is_proven(record) {
        return None;
    }
    if process_alive(record.pid) == Some(false) {
        return Some(true);
    }
    process_start_identity_checked(record.pid)
        .map(|identity| identity != record.process_start_identity)
}

pub(super) fn resource_key(resource_id: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(resource_id.as_bytes());
    let encoded = format!("{:x}", digest.finalize());
    encoded[..16].to_string()
}

pub(crate) fn ownership_labels(
    project_identity: &str,
    resource_id: &str,
    resource_kind: &str,
    invocation_id: &str,
) -> BTreeMap<String, String> {
    BTreeMap::from([
        (LABEL_OWNER.into(), OWNER_MARKER.into()),
        (LABEL_SCHEMA.into(), LEASE_SCHEMA_VERSION.to_string()),
        (LABEL_PROJECT.into(), project_identity.into()),
        (LABEL_RESOURCE.into(), resource_id.into()),
        (LABEL_KIND.into(), resource_kind.into()),
        (LABEL_INVOCATION.into(), invocation_id.into()),
    ])
}

pub(super) fn lease_directory(project: &Project) -> Result<PathBuf> {
    let repository = project
        .root
        .canonicalize()
        .with_context(|| format!("resolve project root {}", project.root.display()))?;
    let path = &project.resource_leases;
    let resolved = if fs::symlink_metadata(path).is_ok() {
        path.canonicalize()
            .with_context(|| format!("resolve lease directory {}", path.display()))?
    } else {
        let mut ancestor = path.as_path();
        while fs::symlink_metadata(ancestor).is_err() {
            ancestor = ancestor
                .parent()
                .ok_or_else(|| anyhow::anyhow!("lease directory has no resolvable parent"))?;
        }
        let resolved_ancestor = ancestor
            .canonicalize()
            .with_context(|| format!("resolve lease directory parent {}", ancestor.display()))?;
        if !resolved_ancestor.starts_with(&repository) {
            bail!("lease directory escapes project root");
        }
        fs::create_dir_all(path)
            .with_context(|| format!("create lease directory {}", path.display()))?;
        path.canonicalize()
            .with_context(|| format!("resolve lease directory {}", path.display()))?
    };
    if !resolved.starts_with(&repository) {
        bail!("lease directory escapes project root");
    }
    Ok(resolved)
}

fn epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn process_start_identity(pid: u32) -> String {
    process_start_identity_checked(pid).unwrap_or_else(|| format!("unavailable:{pid}"))
}

#[cfg(target_os = "linux")]
fn process_start_identity_checked(pid: u32) -> Option<String> {
    let contents = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (_, rest) = contents.rsplit_once(") ")?;
    let start_time = rest.split_whitespace().nth(19)?;
    let start_time = start_time.parse::<u64>().ok()?;
    (start_time > 0).then(|| format!("linux:{start_time}"))
}

#[cfg(target_os = "macos")]
fn process_start_identity_checked(pid: u32) -> Option<String> {
    // SAFETY: libc's C-layout proc_bsdinfo contains only integer fields and
    // fixed byte arrays, for which an all-zero representation is valid.
    let mut info = unsafe { std::mem::zeroed::<libc::proc_bsdinfo>() };
    let expected_size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    // SAFETY: info is aligned, writable storage for exactly expected_size
    // bytes and lives through this synchronous call. PROC_PIDTBSDINFO selects
    // this struct layout; fields are used only after a full-size response.
    let observed = unsafe {
        libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut libc::proc_bsdinfo).cast(),
            expected_size,
        )
    };
    if observed != expected_size {
        return None;
    }
    (info.pbi_start_tvsec > 0 || info.pbi_start_tvusec > 0)
        .then(|| format!("macos:{}:{}", info.pbi_start_tvsec, info.pbi_start_tvusec))
}

#[cfg(target_os = "windows")]
fn process_start_identity_checked(pid: u32) -> Option<String> {
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    if pid == 0 {
        return None;
    }
    // SAFETY: the API takes only scalar arguments. A non-null result is an
    // owned process handle with the query rights required by GetProcessTimes.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }
    let mut creation = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut exit = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut kernel = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut user = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    // SAFETY: handle is live and owned here; the four distinct FILETIME
    // values are aligned writable outputs valid for the entire call.
    let available =
        unsafe { GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) != 0 };
    // SAFETY: this is the sole close of the non-null OpenProcess handle,
    // after its last use, on both success and failure of the query.
    unsafe {
        CloseHandle(handle);
    }
    if !available {
        return None;
    }
    let ticks = (u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime);
    (ticks > 0).then(|| format!("windows:{ticks}"))
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn process_start_identity_checked(_pid: u32) -> Option<String> {
    None
}

#[cfg(unix)]
fn process_alive(pid: u32) -> Option<bool> {
    if pid == 0 {
        return Some(false);
    }
    // kill(pid, 0) checks existence without sending a signal. EPERM means the
    // process exists but is not inspectable by the current user.
    // SAFETY: kill takes scalar arguments and no borrowed storage; signal 0
    // performs an existence/permission check without delivering a signal.
    let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
    if result == 0 {
        Some(true)
    } else {
        match std::io::Error::last_os_error().raw_os_error() {
            Some(libc::ESRCH) => Some(false),
            Some(libc::EPERM) => Some(true),
            _ => None,
        }
    }
}

#[cfg(not(unix))]
fn process_alive(pid: u32) -> Option<bool> {
    if pid == std::process::id() {
        Some(true)
    } else {
        // Unknown Windows process state is treated conservatively; expiry is
        // still required before a lease can be reclaimed.
        None
    }
}

#[cfg(test)]
mod tests {
    use super::process_start_identity_checked;

    type ProcessObservation = fn(u32) -> (Option<bool>, Option<String>);

    /// Test-only replacement for host process observation. The entry records
    /// the installing thread, so parallel `cargo test` threads never observe
    /// another test's replacement even though the slot is process-global.
    pub(super) static OBSERVE_PROCESS: std::sync::Mutex<
        Option<(std::thread::ThreadId, ProcessObservation)>,
    > = std::sync::Mutex::new(None);

    pub(super) fn observation_for(thread: std::thread::ThreadId) -> Option<ProcessObservation> {
        OBSERVE_PROCESS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .filter(|(owner, _)| *owner == thread)
            .map(|(_, observe)| *observe)
    }
    use super::container::RuntimeInspection;
    use super::{
        cleanup_with_runtime, is_stale, ownership_labels, read_record, resource_key, write_record,
        LeaseRecord, RuntimeOperations, LEASE_SCHEMA_VERSION, OWNER_MARKER,
    };
    use crate::config::ContainerRuntimeKind;
    use crate::project::Project;
    use crate::test_support::TestWorkspace;
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    #[derive(Clone)]
    struct FakeRuntime {
        expected_kind: ContainerRuntimeKind,
        inspection: Option<RuntimeInspection>,
        remove_calls: Arc<AtomicUsize>,
        stop_fails: bool,
    }

    impl RuntimeOperations for FakeRuntime {
        fn inspect(
            &self,
            runtime: ContainerRuntimeKind,
            _project: &Project,
            _name: &str,
            _timeout: Duration,
        ) -> anyhow::Result<RuntimeInspection> {
            if runtime != self.expected_kind {
                anyhow::bail!("fake runtime kind mismatch");
            }
            self.inspection
                .clone()
                .ok_or_else(|| anyhow::anyhow!("fake inspection failed"))
        }

        fn stop(
            &self,
            runtime: ContainerRuntimeKind,
            _cwd: &Path,
            _name: &str,
            _timeout: Duration,
        ) -> anyhow::Result<()> {
            if runtime != self.expected_kind {
                anyhow::bail!("fake runtime kind mismatch");
            }
            self.remove_calls.fetch_add(1, Ordering::SeqCst);
            if self.stop_fails {
                anyhow::bail!("injected runtime removal failure");
            }
            Ok(())
        }
    }

    fn runtime_project(name: &str) -> (TestWorkspace, Project) {
        let workspace = TestWorkspace::new(name);
        crate::preset::init(&workspace.root, "generic", false).expect("initialize fixture");
        workspace.init_git();
        let project =
            Project::discover(Some(workspace.root.clone()), None).expect("discover fixture");
        (workspace, project)
    }

    fn container_lease(
        project: &Project,
        runtime: ContainerRuntimeKind,
        stale: bool,
    ) -> (PathBuf, LeaseRecord, RuntimeInspection) {
        let resource_id = "service:database";
        let invocation_id = "invocation-runtime-fixture";
        let resource_name = "harness-gate-fixture-container";
        let mut lease = super::ResourceLease::acquire(
            project,
            resource_id,
            "container",
            invocation_id,
            Some(resource_name.into()),
            Some(runtime),
        )
        .expect("acquire container lease");
        let path = lease.path.clone();
        // Freeze the owner before writing the synthetic runtime identity and
        // stale/active state so heartbeat writes cannot overwrite the fixture.
        lease.stop_heartbeat();
        let mut record = read_record(&path).expect("read fixture lease");
        let inspection = RuntimeInspection {
            object_id: "runtime-object-1".into(),
            name: resource_name.into(),
            labels: ownership_labels(
                &project.input().project_identity,
                resource_id,
                "container",
                invocation_id,
            ),
        };
        record.runtime_object_id = Some(inspection.object_id.clone());
        record.runtime_labels = inspection.labels.clone();
        if stale {
            record.pid = 0;
            record.process_start_identity = proven_identity_fixture();
            record.expires_at = 0;
        } else {
            record.expires_at = record.expires_at.saturating_add(3600);
        }
        write_record(&path, &record).expect("write fixture lease");
        // The fixture intentionally leaves the marker for cleanup to inspect.
        lease.retain();
        (path, record, inspection)
    }

    fn proven_identity_fixture() -> String {
        #[cfg(target_os = "linux")]
        {
            "linux:1".into()
        }
        #[cfg(target_os = "macos")]
        {
            "macos:1:0".into()
        }
        #[cfg(target_os = "windows")]
        {
            "windows:1".into()
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            "unavailable:test".into()
        }
    }

    fn fake_runtime(
        kind: ContainerRuntimeKind,
        inspection: Option<RuntimeInspection>,
    ) -> (FakeRuntime, Arc<AtomicUsize>) {
        let remove_calls = Arc::new(AtomicUsize::new(0));
        (
            FakeRuntime {
                expected_kind: kind,
                inspection,
                remove_calls: Arc::clone(&remove_calls),
                stop_fails: false,
            },
            remove_calls,
        )
    }

    fn assert_failed_without_remove(
        project: &Project,
        path: &Path,
        runtime: &FakeRuntime,
        expected_code: &str,
    ) {
        let report = cleanup_with_runtime(project, false, runtime).expect("cleanup report");
        assert_eq!(runtime.remove_calls.load(Ordering::SeqCst), 0);
        assert!(path.exists(), "ambiguous lease must remain available");
        assert!(
            report
                .failures
                .iter()
                .any(|failure| failure.contains(expected_code)),
            "expected {expected_code} in {:?}",
            report.failures
        );
    }

    #[test]
    fn resource_keys_are_stable_and_path_safe() {
        let key = resource_key("service:database");
        assert_eq!(key.len(), 16);
        assert!(key.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(key, resource_key("service:database"));
    }

    #[test]
    fn current_process_identity_is_available_on_supported_platforms() {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        assert!(process_start_identity_checked(std::process::id()).is_some());
    }

    #[test]
    fn expired_process_lease_is_stale() {
        let record = LeaseRecord {
            owner_marker: OWNER_MARKER.into(),
            schema_version: LEASE_SCHEMA_VERSION,
            project_identity: "fixture-project".into(),
            resource_id: "fixture".into(),
            resource_kind: "workspace".into(),
            invocation_id: "invocation".into(),
            pid: 0,
            process_start_identity: "unknown".into(),
            created_at: 1,
            heartbeat_at: 1,
            expires_at: 1,
            resource_name: None,
            runtime: None,
            runtime_labels: BTreeMap::new(),
            runtime_object_id: None,
        };
        assert!(is_stale(&record, 2));
    }

    #[test]
    fn lease_heartbeat_renews_marker_during_a_long_step() {
        let (_workspace, project) = runtime_project("lease-heartbeat");
        let lease = super::ResourceLease::acquire(
            &project,
            "step:long-running",
            "workspace",
            "invocation-heartbeat",
            None,
            None,
        )
        .expect("acquire heartbeat lease");
        let path = lease.path.clone();
        let mut stale = read_record(&path).expect("read heartbeat lease");
        stale.heartbeat_at = 0;
        stale.expires_at = 0;
        write_record(&path, &stale).expect("write stale heartbeat fixture");

        let deadline = Instant::now() + Duration::from_secs(15);
        let renewed = loop {
            match read_record(&path) {
                Ok(record)
                    if record.heartbeat_at > 0 && record.expires_at > record.heartbeat_at =>
                {
                    break record;
                }
                Ok(_) | Err(_) => {
                    // `write_record` replaces the marker by removing and
                    // renaming on Windows, so a concurrent read can briefly
                    // observe a missing file between the two operations.
                    assert!(
                        Instant::now() < deadline,
                        "heartbeat did not renew the lease marker before the deadline"
                    );
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        };
        assert!(renewed.heartbeat_at > 0);
        assert!(renewed.expires_at > renewed.heartbeat_at);

        drop(lease);
        // Windows can briefly delay marker removal when the heartbeat thread
        // just closed the same file (antivirus/indexer scanning or handle
        // release timing). Poll instead of asserting an immediate delete so a
        // loaded CI runner does not turn a transient deletion delay into a
        // flaky failure.
        let cleanup_deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if !path.exists() {
                break;
            }
            assert!(
                Instant::now() < cleanup_deadline,
                "drop must stop heartbeat and release marker before the cleanup deadline"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    #[test]
    fn heartbeat_failure_blocks_release_and_retains_the_marker() {
        let (_workspace, project) = runtime_project("lease-heartbeat-failure");
        let mut lease = super::ResourceLease::acquire(
            &project,
            "step:heartbeat-failure",
            "workspace",
            "invocation-heartbeat-failure",
            None,
            None,
        )
        .expect("acquire heartbeat lease");
        let path = lease.path.clone();
        // Make the real heartbeat's filesystem read fail, without racing an
        // in-flight renewal or replacing its error field with a mock result.
        let original = {
            let _record = lease.record.lock().expect("lock renewal");
            let original = std::fs::read(&path).expect("read ownership evidence");
            std::fs::remove_file(&path).expect("remove marker");
            std::fs::create_dir(&path).expect("obstruct marker read");
            original
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        while lease
            .heartbeat_error
            .lock()
            .expect("heartbeat error lock")
            .is_none()
        {
            assert!(
                Instant::now() < deadline,
                "heartbeat failed to observe filesystem fault"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        // Join the heartbeat while reads are still obstructed. Otherwise a
        // later renewal can rewrite the restored marker before release_checked
        // stops the thread, racing the byte-for-byte retention assertion.
        lease.stop_heartbeat();
        std::fs::remove_dir(&path).expect("remove obstruction");
        std::fs::write(&path, &original).expect("restore ownership evidence");

        let error = lease
            .release_checked()
            .expect_err("uncertain ownership must block release");
        assert!(format!("{error:#}").contains("LEASE_OWNERSHIP_UNCERTAIN"));
        assert!(path.exists(), "failed release must retain its marker");
        assert_eq!(std::fs::read(path).unwrap(), original);
    }

    fn retained_report_project(name: &str) -> (Option<tempfile::TempDir>, Project) {
        let (temporary, root) = match std::env::var_os("GH286_RETENTION_EVIDENCE") {
            Some(parent) => {
                std::fs::create_dir_all(&parent).unwrap();
                let temporary = tempfile::Builder::new()
                    .prefix(name)
                    .tempdir_in(parent)
                    .unwrap();
                (None, temporary.keep())
            }
            None => {
                let temporary = tempfile::Builder::new().prefix(name).tempdir().unwrap();
                let root = temporary.path().to_path_buf();
                (Some(temporary), root)
            }
        };
        crate::preset::init(&root, "generic", false).unwrap();
        let output = std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(output.status.success());
        let project = Project::discover(Some(root), None).unwrap();
        (temporary, project)
    }

    fn report_lease(project: &Project, id: &str) -> (super::ResourceLease, PathBuf) {
        let root = project.reports.join("invocations").join(id);
        std::fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let lease = super::ResourceLease::acquire(
            project,
            format!("invocation:{id}"),
            "report-directory",
            id,
            Some(root.to_string_lossy().into_owned()),
            None,
        )
        .unwrap();
        (lease, root)
    }

    #[test]
    fn report_cleanup_shares_live_guard_and_preserves_expired_unknown_ownership() {
        let (_workspace, project) = retained_report_project("gh286-report-cleanup-");
        let (lease, root) = report_lease(&project, "inv-cleanup-guard");
        let path = lease.path.clone();
        let (runtime, stop_calls) = fake_runtime(ContainerRuntimeKind::Docker, None);
        let report = cleanup_with_runtime(&project, false, &runtime).unwrap();
        std::fs::write(
            project.root.join("live-cleanup.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        assert_eq!(report.active, 1);
        assert_eq!(report.reclaimed, 0);
        assert!(path.is_file());
        assert!(
            super::ReportDirectoryGuard::try_acquire(&project, "inv-cleanup-guard")
                .unwrap()
                .is_none()
        );
        lease.retain();
        let mut unknown = read_record(&path).unwrap();
        unknown.pid = 0;
        unknown.process_start_identity = "unavailable:expired-fixture".into();
        unknown.heartbeat_at = 0;
        unknown.expires_at = 0;
        write_record(&path, &unknown).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let report = cleanup_with_runtime(&project, false, &runtime).unwrap();
        std::fs::write(
            project.root.join("unknown-cleanup.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
        assert_eq!(report.reclaimed, 0);
        assert!(report
            .failures
            .iter()
            .any(|failure| failure.contains("LEASE_OWNERSHIP_UNCERTAIN")));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(stop_calls.load(Ordering::SeqCst), 0);
        let guard = super::ReportDirectoryGuard::try_acquire(&project, "inv-cleanup-guard")
            .unwrap()
            .unwrap();
        assert!(!guard.release_is_proven(&root).unwrap());
    }

    #[test]
    fn report_real_heartbeat_failure_cannot_certify_a_release_even_without_marker() {
        let (_workspace, project) = retained_report_project("gh286-report-heartbeat-fault-");
        let (mut lease, root) = report_lease(&project, "inv-heartbeat-fault");
        let path = lease.path.clone();
        let original = {
            let _record = lease.record.lock().unwrap();
            let original = std::fs::read(&path).unwrap();
            std::fs::remove_file(&path).unwrap();
            std::fs::create_dir(&path).unwrap();
            original
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        while lease.heartbeat_error.lock().unwrap().is_none() {
            assert!(
                Instant::now() < deadline,
                "real heartbeat did not observe the read fault"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        std::fs::write(
            project.root.join("heartbeat-error.txt"),
            lease.heartbeat_error.lock().unwrap().as_deref().unwrap(),
        )
        .unwrap();
        lease.stop_heartbeat();
        std::fs::remove_dir(&path).unwrap();
        std::fs::write(&path, &original).unwrap();
        let error = lease.release_checked().unwrap_err();
        std::fs::write(project.root.join("release-error.txt"), format!("{error:#}")).unwrap();
        assert!(format!("{error:#}").contains("lease heartbeat failed"));
        assert_eq!(std::fs::read(&path).unwrap(), original);
        // Missing JSON cannot bypass the failed-release state in the stable,
        // locked certificate. No PID or TTL inference is used by retention.
        std::fs::remove_file(&path).unwrap();
        let guard = super::ReportDirectoryGuard::try_acquire(&project, "inv-heartbeat-fault")
            .unwrap()
            .unwrap();
        assert!(!guard.release_is_proven(&root).unwrap());
    }

    #[test]
    fn runtime_removal_failure_retains_evidence_and_allows_a_proven_retry() {
        for kind in [ContainerRuntimeKind::Docker, ContainerRuntimeKind::Podman] {
            let (_workspace, project) = runtime_project("lease-removal-fault");
            let (path, _, inspection) = container_lease(&project, kind, true);
            let original = std::fs::read(&path).unwrap();
            let (mut runtime, calls) = fake_runtime(kind, Some(inspection));
            runtime.stop_fails = true;
            let report = cleanup_with_runtime(&project, false, &runtime).unwrap();
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert_eq!(report.reclaimed, 0);
            assert!(report
                .failures
                .iter()
                .any(|failure| failure.contains("injected runtime removal failure")));
            assert_eq!(std::fs::read(&path).unwrap(), original);
            // Loss of inspection certainty must not trigger a second remove.
            let inspection = runtime.inspection.take();
            assert_failed_without_remove(
                &project,
                &path,
                &fake_runtime(kind, None).0,
                "fake inspection failed",
            );
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            runtime.inspection = inspection;
            runtime.stop_fails = false;
            let report = cleanup_with_runtime(&project, false, &runtime).unwrap();
            assert_eq!(calls.load(Ordering::SeqCst), 2);
            assert_eq!(report.reclaimed, 1);
            assert!(report.failures.is_empty());
            assert!(!path.exists());
        }
    }

    #[test]
    fn poisoned_lease_lock_fails_closed_and_retains_the_marker() {
        let (_workspace, project) = runtime_project("lease-lock-poison");
        let mut lease = super::ResourceLease::acquire(
            &project,
            "step:poisoned-lock",
            "workspace",
            "invocation-poisoned-lock",
            None,
            None,
        )
        .expect("acquire lease");
        let path = lease.path.clone();
        lease.stop_heartbeat();
        let record = std::sync::Arc::clone(&lease.record);
        let poisoned = std::thread::spawn(move || {
            let _guard = record.lock().expect("lock lease record");
            panic!("poison lease record fixture");
        })
        .join();
        assert!(poisoned.is_err());

        let error = lease
            .release_checked()
            .expect_err("poisoned ownership state must block release");
        assert!(format!("{error:#}").contains("lease record lock was poisoned"));
        assert!(path.exists(), "poisoned ownership marker must be retained");
    }

    #[test]
    fn alive_holder_with_unreadable_identity_is_never_stale() {
        let mut record = LeaseRecord {
            owner_marker: OWNER_MARKER.into(),
            schema_version: LEASE_SCHEMA_VERSION,
            project_identity: "fixture-project".into(),
            resource_id: "fixture".into(),
            resource_kind: "workspace".into(),
            invocation_id: "invocation".into(),
            pid: 42,
            process_start_identity: "linux:42".into(),
            created_at: 1,
            heartbeat_at: 1,
            expires_at: 1,
            resource_name: None,
            runtime: None,
            runtime_labels: BTreeMap::new(),
            runtime_object_id: None,
        };
        let state = |alive, identity: Option<&str>| {
            super::classify_lease(&record, 2, alive, || identity.map(str::to_string))
        };
        assert_eq!(
            state(Some(true), None),
            super::LeaseState::AliveIdentityUnknown
        );
        assert_eq!(
            state(Some(true), Some("linux:42")),
            super::LeaseState::Active
        );
        assert_eq!(state(Some(true), Some("linux:7")), super::LeaseState::Stale);
        assert_eq!(state(Some(false), None), super::LeaseState::Stale);
        assert_eq!(state(None, None), super::LeaseState::Stale);
        record.expires_at = 3;
        let unexpired = super::classify_lease(&record, 2, None, || None);
        assert_eq!(unexpired, super::LeaseState::Active);
    }

    #[test]
    fn cleanup_retains_expired_lease_of_live_holder_with_unreadable_identity() {
        let (_workspace, project) = runtime_project("lease-alive-identity-unknown");
        let mut lease = super::ResourceLease::acquire(
            &project,
            "step:alive-unknown",
            "workspace",
            "invocation-alive-unknown",
            None,
            None,
        )
        .expect("acquire lease");
        let path = lease.path.clone();
        lease.stop_heartbeat();
        let mut expired = read_record(&path).expect("read lease");
        expired.heartbeat_at = 0;
        expired.expires_at = 0;
        write_record(&path, &expired).expect("write expired fixture");
        lease.retain();

        *OBSERVE_PROCESS.lock().unwrap() =
            Some((std::thread::current().id(), |_| (Some(true), None)));
        let (fake, remove_calls) = fake_runtime(ContainerRuntimeKind::Docker, None);
        for dry_run in [true, false] {
            let report = cleanup_with_runtime(&project, dry_run, &fake).expect("cleanup report");
            assert_eq!(report.reclaimed, 0);
            assert_eq!(report.resources.len(), 1);
            assert_eq!(report.resources[0].state, "ownership-uncertain");
            assert_eq!(report.resources[0].action, "retained");
            assert!(report.failures.iter().any(|failure| {
                failure.contains("LEASE_OWNERSHIP_UNCERTAIN")
                    && failure.contains("process identity cannot be read")
            }));
        }
        let Err(error) = super::ResourceLease::acquire(
            &project,
            "step:alive-unknown",
            "workspace",
            "invocation-contender",
            None,
            None,
        ) else {
            panic!("live holder keeps the lease");
        };
        *OBSERVE_PROCESS.lock().unwrap() = None;
        assert!(format!("{error:#}").contains("LEASE_OWNERSHIP_UNCERTAIN"));
        assert_eq!(remove_calls.load(Ordering::SeqCst), 0);
        assert!(path.exists(), "live holder's marker must be retained");
    }

    #[test]
    fn uncertain_process_identity_is_retained_without_reclaim() {
        let (_workspace, project) = runtime_project("lease-identity-uncertain");
        let mut lease = super::ResourceLease::acquire(
            &project,
            "step:uncertain",
            "workspace",
            "invocation-uncertain",
            None,
            None,
        )
        .expect("acquire uncertain identity lease");
        let path = lease.path.clone();
        // Stop renewal before forging an expired/uncertain fixture. Otherwise
        // the heartbeat can race this test and restore the live identity.
        lease.stop_heartbeat();
        let mut forged = read_record(&path).expect("read uncertain identity lease");
        forged.pid = 0;
        forged.process_start_identity = "unavailable:test".into();
        forged.heartbeat_at = 0;
        forged.expires_at = 0;
        write_record(&path, &forged).expect("write uncertain identity fixture");
        lease.retain();

        let (fake, remove_calls) = fake_runtime(ContainerRuntimeKind::Docker, None);
        let report = cleanup_with_runtime(&project, false, &fake).expect("cleanup report");
        assert_eq!(report.reclaimed, 0);
        assert_eq!(remove_calls.load(Ordering::SeqCst), 0);
        assert!(path.exists(), "uncertain ownership must be retained");
        assert!(report
            .failures
            .iter()
            .any(|failure| failure.contains("LEASE_OWNERSHIP_UNCERTAIN")));
        assert_eq!(
            report.resources[0].state, "ownership-uncertain",
            "cleanup evidence must expose the uncertain state"
        );
    }

    #[test]
    fn reclaim_rejects_invalid_container_targets_before_runtime_removal() {
        for (index, (name, runtime, expected)) in [
            (None, Some("docker"), "no resource name"),
            (
                Some("foreign-container"),
                Some("docker"),
                "not Harness-Gate managed",
            ),
            (
                Some("harness-gate-fixture-container"),
                Some("unknown"),
                "unsupported container runtime",
            ),
            (Some("harness-gate-fixture-container"), None, "no runtime"),
        ]
        .into_iter()
        .enumerate()
        {
            let (_workspace, project) = runtime_project(&format!("reclaim-invalid-{index}"));
            let (path, mut record, inspection) =
                container_lease(&project, ContainerRuntimeKind::Docker, true);
            record.resource_name = name.map(str::to_string);
            record.runtime = runtime.map(str::to_string);
            let (fake, calls) = fake_runtime(ContainerRuntimeKind::Docker, Some(inspection));
            let error =
                super::reclaim_resource_with_runtime(&project, &path, &record, &fake).unwrap_err();
            assert!(format!("{error:#}").contains(expected), "{error:#}");
            assert_eq!(calls.load(Ordering::SeqCst), 0);
            assert!(path.is_file(), "invalid target must retain its marker");
        }
    }

    #[test]
    fn generic_reclaim_is_idempotent_but_reports_marker_removal_errors() {
        let (_workspace, project) = runtime_project("reclaim-marker-errors");
        let (path, mut record, _) = container_lease(&project, ContainerRuntimeKind::Docker, true);
        record.resource_kind = "workspace".into();
        let (fake, calls) = fake_runtime(ContainerRuntimeKind::Docker, None);
        std::fs::remove_file(&path).unwrap();
        super::reclaim_resource_with_runtime(&project, &path, &record, &fake).unwrap();
        std::fs::create_dir(&path).unwrap();
        let error =
            super::reclaim_resource_with_runtime(&project, &path, &record, &fake).unwrap_err();
        assert!(format!("{error:#}").contains("remove lease"));
        assert!(path.is_dir(), "unexpected marker type must not be removed");
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn cleanup_targets_verified_object_id_after_name_replacement() {
        struct ReplacedNameRuntime {
            inspection: RuntimeInspection,
            removed_replacement: Arc<AtomicUsize>,
        }
        impl RuntimeOperations for ReplacedNameRuntime {
            fn inspect(
                &self,
                _runtime: ContainerRuntimeKind,
                _project: &Project,
                _name: &str,
                _timeout: Duration,
            ) -> anyhow::Result<RuntimeInspection> {
                // Return the original object. By the time stop is called, its
                // reusable name belongs to a different, unowned object.
                Ok(self.inspection.clone())
            }
            fn stop(
                &self,
                _runtime: ContainerRuntimeKind,
                _cwd: &Path,
                target: &str,
                _timeout: Duration,
            ) -> anyhow::Result<()> {
                if target == self.inspection.name {
                    self.removed_replacement.fetch_add(1, Ordering::SeqCst);
                }
                anyhow::ensure!(
                    target == self.inspection.object_id,
                    "stop target must be the verified object ID"
                );
                Ok(())
            }
        }
        for (index, runtime) in [ContainerRuntimeKind::Docker, ContainerRuntimeKind::Podman]
            .into_iter()
            .enumerate()
        {
            let (_workspace, project) =
                runtime_project(&format!("runtime-replaced-after-inspect-{index}"));
            let (path, _record, inspection) = container_lease(&project, runtime, true);
            let removed_replacement = Arc::new(AtomicUsize::new(0));
            let fake = ReplacedNameRuntime {
                inspection,
                removed_replacement: Arc::clone(&removed_replacement),
            };
            let report = cleanup_with_runtime(&project, false, &fake).expect("cleanup report");
            assert_eq!(
                removed_replacement.load(Ordering::SeqCst),
                0,
                "cleanup must not remove an unowned replacement through a reused name"
            );
            assert_eq!(report.reclaimed, 1);
            assert!(report.failures.is_empty());
            assert!(!path.exists());
        }
    }

    #[test]
    fn legacy_engine_cleanup_retains_marker_without_implicit_migration() {
        for runtime in [ContainerRuntimeKind::Docker, ContainerRuntimeKind::Podman] {
            let (_workspace, project) = runtime_project("retired-engine-lease");
            let (path, _, _) = container_lease(&project, runtime, true);
            let bytes = std::fs::read(&path).unwrap();
            let report = super::cleanup(&project, false).unwrap();
            assert_eq!(report.reclaimed, 0);
            assert!(report
                .failures
                .iter()
                .any(|f| f.contains("CLI service operations are retired")));
            let error = super::RetiredRuntimeOperations
                .stop(
                    runtime,
                    &project.root,
                    "recorded-id",
                    Duration::from_secs(1),
                )
                .unwrap_err();
            assert!(error
                .to_string()
                .contains("lease retained without migration"));
            assert_eq!(std::fs::read(path).unwrap(), bytes);
        }
    }

    #[test]
    fn fake_docker_and_podman_cleanup_requires_fresh_complete_ownership() {
        for (index, runtime) in [ContainerRuntimeKind::Docker, ContainerRuntimeKind::Podman]
            .into_iter()
            .enumerate()
        {
            let (workspace, project) = runtime_project(&format!("runtime-owned-{index}"));
            let (path, _record, inspection) = container_lease(&project, runtime, true);
            let (fake, remove_calls) = fake_runtime(runtime, Some(inspection.clone()));
            let report = cleanup_with_runtime(&project, false, &fake).expect("cleanup report");
            assert_eq!(report.reclaimed, 1);
            assert!(report.failures.is_empty());
            assert_eq!(remove_calls.load(Ordering::SeqCst), 1);
            assert!(!path.exists());
            drop(workspace);

            let (workspace, project) = runtime_project(&format!("runtime-object-{index}"));
            let (path, _record, mut mismatch) = container_lease(&project, runtime, true);
            mismatch.object_id = "replacement-object".into();
            let (fake, _) = fake_runtime(runtime, Some(mismatch));
            assert_failed_without_remove(&project, &path, &fake, "runtime object identity changed");
            drop(workspace);

            let (workspace, project) = runtime_project(&format!("runtime-label-{index}"));
            let (path, _record, mut mismatch) = container_lease(&project, runtime, true);
            mismatch
                .labels
                .insert(super::LABEL_PROJECT.into(), "other-project".into());
            let (fake, _) = fake_runtime(runtime, Some(mismatch));
            assert_failed_without_remove(&project, &path, &fake, "ownership label");
            drop(workspace);

            let (workspace, project) = runtime_project(&format!("runtime-renamed-{index}"));
            let (path, _record, mut mismatch) = container_lease(&project, runtime, true);
            mismatch.name = "harness-gate-renamed-container".into();
            let (fake, _) = fake_runtime(runtime, Some(mismatch));
            assert_failed_without_remove(&project, &path, &fake, "object name");
            drop(workspace);

            let (workspace, project) = runtime_project(&format!("runtime-inspect-{index}"));
            let (path, _record, _inspection) = container_lease(&project, runtime, true);
            let (fake, _) = fake_runtime(runtime, None);
            assert_failed_without_remove(&project, &path, &fake, "fake inspection failed");
            drop(workspace);

            let (workspace, project) = runtime_project(&format!("runtime-cross-project-{index}"));
            let (path, mut record, inspection) = container_lease(&project, runtime, true);
            record.project_identity = "other-project".into();
            write_record(&path, &record).expect("forge cross-project lease");
            let (fake, _) = fake_runtime(runtime, Some(inspection));
            assert_failed_without_remove(&project, &path, &fake, "project identity");
            drop(workspace);

            let (workspace, project) = runtime_project(&format!("runtime-forged-{index}"));
            let (path, mut record, inspection) = container_lease(&project, runtime, true);
            record.owner_marker = "forged".into();
            write_record(&path, &record).expect("forge owner marker");
            let (fake, _) = fake_runtime(runtime, Some(inspection));
            assert_failed_without_remove(&project, &path, &fake, "owner marker");
            drop(workspace);

            let (workspace, project) = runtime_project(&format!("runtime-renamed-lease-{index}"));
            let (path, _record, inspection) = container_lease(&project, runtime, true);
            let renamed = path.with_file_name("renamed-lease.json");
            std::fs::rename(&path, &renamed).expect("rename lease marker");
            let (fake, _) = fake_runtime(runtime, Some(inspection));
            assert_failed_without_remove(&project, &renamed, &fake, "deterministic resource key");
            drop(workspace);

            let (workspace, project) = runtime_project(&format!("runtime-malformed-{index}"));
            let directory = super::lease_directory(&project).expect("lease directory");
            let malformed = directory.join(format!("{}.json", resource_key("service:database")));
            std::fs::write(&malformed, b"not-json").expect("write malformed lease");
            let (fake, _) = fake_runtime(runtime, None);
            assert_failed_without_remove(&project, &malformed, &fake, "parse");
            drop(workspace);

            let (workspace, project) = runtime_project(&format!("runtime-active-{index}"));
            let (path, _record, inspection) = container_lease(&project, runtime, false);
            let (fake, remove_calls) = fake_runtime(runtime, Some(inspection));
            let report = cleanup_with_runtime(&project, false, &fake).expect("cleanup report");
            assert_eq!(report.active, 1);
            assert_eq!(report.reclaimed, 0);
            assert!(report.failures.is_empty());
            assert_eq!(remove_calls.load(Ordering::SeqCst), 0);
            assert!(path.exists());
            drop(workspace);
        }
    }

    fn report_lock_path(project: &Project, invocation_id: &str) -> PathBuf {
        super::lease_directory(project)
            .unwrap()
            .join("report-directory-locks")
            .join(format!(
                "{}.lock",
                resource_key(&format!("invocation:{invocation_id}"))
            ))
    }

    fn assert_report_guard_held(
        lease: &super::ResourceLease,
        project: &Project,
        root: &Path,
        id: &str,
    ) {
        assert!(lease.report_directory_guard.is_some());
        assert!(super::ReportDirectoryGuard::try_acquire(project, id)
            .unwrap()
            .is_none());
        assert!(!lease
            .report_directory_guard
            .as_ref()
            .unwrap()
            .release_is_proven(root)
            .unwrap());
    }

    #[test]
    fn direct_report_release_requires_stopped_heartbeat_and_keeps_held_state() {
        let (_workspace, project) = retained_report_project("gh286-release-live-");
        let id = "inv-release-live";
        let (lease, root) = report_lease(&project, id);
        // Prevent an actual renewal from changing the bytes during assertions.
        // release must reject before taking this record lock; checked release
        // is performed only after the lock has been dropped.
        {
            let _record = lease.record.lock().unwrap();
            let bytes = std::fs::read(&lease.path).unwrap();
            let error = lease.release().unwrap_err();
            assert!(format!("{error:#}").contains("requires stopped heartbeat"));
            assert_eq!(std::fs::read(&lease.path).unwrap(), bytes);
            assert_report_guard_held(&lease, &project, &root, id);
        }
        lease.retain();
        assert_eq!(
            std::fs::read(report_lock_path(&project, id)).unwrap(),
            b"held\n"
        );
    }

    #[test]
    fn stopped_report_release_rejects_missing_marker_without_certificate() {
        let (_workspace, project) = retained_report_project("gh286-release-missing-");
        let id = "inv-release-missing";
        let (mut lease, root) = report_lease(&project, id);
        lease.stop_heartbeat();
        std::fs::remove_file(&lease.path).unwrap();
        let error = lease.release().unwrap_err();
        assert!(format!("{error:#}").contains("marker disappeared before checked release"));
        assert!(!lease.path.exists());
        assert!(root.is_dir());
        assert_report_guard_held(&lease, &project, &root, id);
        lease.retain();
        assert_eq!(
            std::fs::read(report_lock_path(&project, id)).unwrap(),
            b"held\n"
        );
        let guard = super::ReportDirectoryGuard::try_acquire(&project, id)
            .unwrap()
            .unwrap();
        assert!(!guard.release_is_proven(&root).unwrap());
    }

    #[test]
    fn stopped_legacy_release_is_idempotent_when_marker_is_missing() {
        let (_workspace, project) = runtime_project("release-missing-legacy");
        let mut lease = super::ResourceLease::acquire(
            &project,
            "step:release-missing",
            "workspace",
            "inv-release-missing-legacy",
            None,
            None,
        )
        .unwrap();
        lease.stop_heartbeat();
        assert!(lease.report_directory_guard.is_none());
        let path = lease.path.clone();
        std::fs::remove_file(&path).unwrap();
        lease.release().unwrap();
        lease.release_checked().unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn report_release_real_read_error_keeps_obstruction_and_held_certificate() {
        let (_workspace, project) = retained_report_project("gh286-release-read-");
        let id = "inv-release-read";
        let (mut lease, root) = report_lease(&project, id);
        lease.stop_heartbeat();
        let path = lease.path.clone();
        let original = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        let actual = std::fs::read(&path).unwrap_err();
        assert_ne!(actual.kind(), std::io::ErrorKind::NotFound);
        let error = lease.release().unwrap_err();
        assert!(format!("{error:#}").contains("read lease before release"));
        assert_eq!(
            error.downcast_ref::<std::io::Error>().unwrap().kind(),
            actual.kind()
        );
        assert!(path.is_dir());
        assert_eq!(std::fs::read_dir(&path).unwrap().count(), 0);
        assert_report_guard_held(&lease, &project, &root, id);
        lease.retain();
        assert_eq!(
            std::fs::read(report_lock_path(&project, id)).unwrap(),
            b"held\n"
        );
        std::fs::remove_dir(&path).unwrap();
        std::fs::write(&path, &original).unwrap();
        let guard = super::ReportDirectoryGuard::try_acquire(&project, id)
            .unwrap()
            .unwrap();
        assert!(!guard.release_is_proven(&root).unwrap());
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }

    #[test]
    fn report_release_malformed_marker_is_retained_without_certificate() {
        let (_workspace, project) = retained_report_project("gh286-release-parse-");
        let id = "inv-release-parse";
        let (mut lease, root) = report_lease(&project, id);
        lease.stop_heartbeat();
        std::fs::write(&lease.path, b"{not-json").unwrap();
        let error = lease.release().unwrap_err();
        assert!(format!("{error:#}").contains("parse lease before release"));
        assert_eq!(std::fs::read(&lease.path).unwrap(), b"{not-json");
        assert_report_guard_held(&lease, &project, &root, id);
        lease.retain();
        assert_eq!(
            std::fs::read(report_lock_path(&project, id)).unwrap(),
            b"held\n"
        );
    }

    #[test]
    fn report_release_unknown_owner_is_retained_without_certificate() {
        let (_workspace, project) = retained_report_project("gh286-release-owner-");
        let id = "inv-release-owner";
        let (mut lease, root) = report_lease(&project, id);
        lease.stop_heartbeat();
        let mut unknown = read_record(&lease.path).unwrap();
        unknown.invocation_id = "different-invocation".into();
        unknown.process_start_identity = "unavailable:unknown-owner".into();
        write_record(&lease.path, &unknown).unwrap();
        let bytes = std::fs::read(&lease.path).unwrap();
        let error = lease.release().unwrap_err();
        assert!(format!("{error:#}").contains("lease ownership changed"));
        assert_eq!(std::fs::read(&lease.path).unwrap(), bytes);
        assert_report_guard_held(&lease, &project, &root, id);
        lease.retain();
        assert_eq!(
            std::fs::read(report_lock_path(&project, id)).unwrap(),
            b"held\n"
        );
    }

    #[cfg(unix)]
    struct RestoreDirectoryPermissions {
        path: PathBuf,
        original: std::fs::Permissions,
    }

    #[cfg(unix)]
    impl Drop for RestoreDirectoryPermissions {
        fn drop(&mut self) {
            std::fs::set_permissions(&self.path, self.original.clone())
                .expect("restore lease directory permissions, including while unwinding");
        }
    }

    #[test]
    #[cfg(unix)]
    fn report_release_real_remove_error_retains_marker_then_allows_checked_retry() {
        use std::os::unix::fs::PermissionsExt;
        // Root or DAC-bypass privileges could turn a permissions fixture into
        // a successful remove. They are an explicit unsupported test identity,
        // never a silent skip or a mocked permission error.
        assert_ne!(
            unsafe { libc::geteuid() },
            0,
            "run this fixture as an unprivileged Unix identity"
        );
        let (_workspace, project) = retained_report_project("gh286-release-remove-");
        let id = "inv-release-remove";
        let (mut lease, root) = report_lease(&project, id);
        lease.stop_heartbeat();
        let path = lease.path.clone();
        let bytes = std::fs::read(&path).unwrap();
        let directory = path.parent().unwrap().to_path_buf();
        let restore = RestoreDirectoryPermissions {
            original: std::fs::metadata(&directory).unwrap().permissions(),
            path: directory.clone(),
        };
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o555)).unwrap();
        // Prove write denial against a real disposable sibling before release;
        // traversing/reading the existing marker must remain possible.
        let probe = directory.join("release-permission-probe");
        let actual = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe)
            .unwrap_err();
        assert_eq!(actual.kind(), std::io::ErrorKind::PermissionDenied);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let error = lease.release().unwrap_err();
        assert!(format!("{error:#}").contains("release lease"));
        assert_eq!(
            error.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::PermissionDenied
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_report_guard_held(&lease, &project, &root, id);
        // The release certificate was persisted, but the still-present marker
        // blocks retention. Its exact bound fields are independently checked.
        let certificate: serde_json::Value =
            serde_json::from_slice(&std::fs::read(report_lock_path(&project, id)).unwrap())
                .unwrap();
        assert_eq!(
            certificate,
            serde_json::json!({
                "schema_version": 1,
                "invocation_id": id,
                "project_identity": project.input().project_identity,
                "root": root,
            })
        );
        drop(restore);
        lease.release_checked().unwrap();
        assert!(!path.exists());
        let guard = super::ReportDirectoryGuard::try_acquire(&project, id)
            .unwrap()
            .unwrap();
        assert!(guard.release_is_proven(&root).unwrap());
        assert!(root.is_dir());
    }

    #[test]
    fn report_release_invalid_certificate_binding_keeps_marker_and_held_state() {
        let (_workspace, project) = retained_report_project("gh286-release-binding-");
        let id = "inv-release-binding";
        let (mut lease, root) = report_lease(&project, id);
        lease.stop_heartbeat();
        // Ordinary ownership fields still agree. The real report guard must
        // independently reject a different root before writing its certificate.
        let invalid = {
            let mut record = lease.record.lock().unwrap();
            record.resource_name = Some(root.join("different-root").to_string_lossy().into_owned());
            record.clone()
        };
        write_record(&lease.path, &invalid).unwrap();
        let bytes = std::fs::read(&lease.path).unwrap();
        let error = lease.release().unwrap_err();
        assert!(format!("{error:#}").contains("report-directory lease binding is uncertain"));
        assert_eq!(std::fs::read(&lease.path).unwrap(), bytes);
        assert_report_guard_held(&lease, &project, &root, id);
        lease.retain();
        assert_eq!(
            std::fs::read(report_lock_path(&project, id)).unwrap(),
            b"held\n"
        );
    }

    fn bounded_fixture_status(
        child: &mut std::process::Child,
        timeout: Duration,
    ) -> std::io::Result<std::process::ExitStatus> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = child.try_wait()? {
                return Ok(status);
            }
            if Instant::now() >= deadline {
                break;
            }
            std::thread::yield_now();
        }
        let kill = child.kill();
        let reap_deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    return Err(std::io::Error::other(format!(
                        "fixture child {} timed out; kill={kill:?}; reaped={status:?}",
                        child.id()
                    )));
                }
                Err(error) => return Err(error),
                Ok(None) if Instant::now() < reap_deadline => std::thread::yield_now(),
                Ok(None) => {
                    return Err(std::io::Error::other(format!(
                        "fixture child {} could not be reaped within deadline; kill={kill:?}",
                        child.id()
                    )));
                }
            }
        }
    }

    struct FixtureProcess(std::process::Child);

    impl Drop for FixtureProcess {
        fn drop(&mut self) {
            match self.0.try_wait() {
                Ok(Some(_)) => {}
                state => {
                    let kill = self.0.kill();
                    if let Err(error) = bounded_fixture_status(&mut self.0, Duration::from_secs(2))
                    {
                        eprintln!(
                            "fixture cleanup FAIL: initial={state:?}; kill={kill:?}; {error}"
                        );
                    }
                }
            }
        }
    }

    fn run_temp_ownership_child(mode: &str) {
        let (_workspace, project) = retained_report_project("gh286-temp-ownership-");
        let control = project.root.join("temp-ownership-child");
        std::fs::create_dir(&control).unwrap();
        let args = [
            "--exact",
            "service::lease::tests::lease_temp_ownership_child",
            "--nocapture",
            "--test-threads=1",
        ];
        std::fs::write(
            control.join("command.json"),
            serde_json::to_vec(&args).unwrap(),
        )
        .unwrap();
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(args)
            .env("GH286_TEMP_CHILD", mode)
            .env("GH286_TEMP_ROOT", &control)
            .stdout(std::fs::File::create(control.join("stdout.log")).unwrap())
            .stderr(std::fs::File::create(control.join("stderr.log")).unwrap())
            .spawn()
            .unwrap();
        let mut child = FixtureProcess(child);
        let result = bounded_fixture_status(&mut child.0, Duration::from_secs(20));
        std::fs::write(
            control.join("status.json"),
            serde_json::to_vec(&serde_json::json!({
                "pid": child.0.id(), "result": format!("{result:?}"),
            }))
            .unwrap(),
        )
        .unwrap();
        let stdout = std::fs::read_to_string(control.join("stdout.log"));
        let stderr = std::fs::read_to_string(control.join("stderr.log"));
        let status = result.unwrap_or_else(|error| {
            panic!("temp ownership child cleanup failed: {error}; stdout={stdout:?}; stderr={stderr:?}")
        });
        assert!(
            status.success(),
            "temp ownership child failed: {status:?}; stdout={stdout:?}; stderr={stderr:?}"
        );
    }

    #[test]
    fn write_record_create_collision_preserves_foreign_temporary() {
        run_temp_ownership_child("collision");
    }

    #[test]
    fn write_record_publish_failure_cleans_only_owned_temporary() {
        run_temp_ownership_child("publish-failure");
    }

    #[test]
    fn lease_temp_ownership_child() {
        let Ok(mode) = std::env::var("GH286_TEMP_CHILD") else {
            return;
        };
        let directory = PathBuf::from(std::env::var_os("GH286_TEMP_ROOT").unwrap());
        // This exact child runs no other test and creates no heartbeat. Inspect
        // the real initial counter; never reset it or race another writer.
        let counter = super::TEMP_COUNTER.load(Ordering::Relaxed);
        assert_eq!(counter, 1);
        let pid = std::process::id();
        let temporary = directory.join(format!(".lease-{pid}-{counter}.tmp"));
        let legacy_temporary = directory.join(format!(".lease-{counter}.tmp"));
        let target = directory.join("marker.json");
        let record = LeaseRecord {
            owner_marker: OWNER_MARKER.into(),
            schema_version: LEASE_SCHEMA_VERSION,
            project_identity: "temp-ownership-project".into(),
            resource_id: "fixture".into(),
            resource_kind: "workspace".into(),
            invocation_id: "temp-ownership-invocation".into(),
            pid,
            process_start_identity: "isolated-counter-fixture".into(),
            created_at: 1,
            heartbeat_at: 1,
            expires_at: 1,
            resource_name: None,
            runtime: None,
            runtime_labels: BTreeMap::new(),
            runtime_object_id: None,
        };
        let sentinel = b"pre-existing writer temporary must survive\n";
        std::fs::write(
            directory.join("temp-identity.json"),
            serde_json::to_vec(&serde_json::json!({
                "pid": pid, "counter": counter, "mode": mode,
                "temporary": temporary, "legacy_temporary": legacy_temporary,
            }))
            .unwrap(),
        )
        .unwrap();
        if mode == "collision" {
            std::fs::write(&target, b"original marker\n").unwrap();
            std::fs::write(&temporary, sentinel).unwrap();
            // The same fixture on the old source reaches its old-name real
            // create_new failure and exposes its unowned-temp deletion.
            std::fs::write(&legacy_temporary, sentinel).unwrap();
            let error = write_record(&target, &record).unwrap_err();
            assert_eq!(
                error.downcast_ref::<std::io::Error>().unwrap().kind(),
                std::io::ErrorKind::AlreadyExists
            );
            assert_eq!(std::fs::read(&target).unwrap(), b"original marker\n");
            assert_eq!(std::fs::read(&temporary).unwrap(), sentinel);
            assert_eq!(std::fs::read(&legacy_temporary).unwrap(), sentinel);
        } else {
            assert_eq!(mode, "publish-failure");
            std::fs::create_dir(&target).unwrap();
            std::fs::write(target.join("sentinel"), b"original target directory\n").unwrap();
            let foreign = directory.join(".lease-foreign.tmp");
            std::fs::write(&foreign, sentinel).unwrap();
            let error = write_record(&target, &record).unwrap_err();
            assert!(error.downcast_ref::<std::io::Error>().is_some());
            assert!(
                format!("{error:#}").contains("publish lease")
                    || format!("{error:#}").contains("replace existing lease")
            );
            assert!(!temporary.exists());
            assert!(!legacy_temporary.exists());
            assert_eq!(std::fs::read(&foreign).unwrap(), sentinel);
            assert_eq!(
                std::fs::read(target.join("sentinel")).unwrap(),
                b"original target directory\n"
            );
        }
    }

    #[cfg(target_os = "linux")]
    struct ForkGuardBarrier {
        ready: std::fs::File,
        gate: Option<std::fs::File>,
        worker: Option<std::thread::JoinHandle<std::io::Result<std::process::Child>>>,
        pidfd: Option<std::os::fd::OwnedFd>,
        raw_ready: Vec<u8>,
        cleanup_attempted: bool,
    }

    #[cfg(target_os = "linux")]
    impl ForkGuardBarrier {
        fn start(guard_fd: i32, control: &Path) -> std::io::Result<Self> {
            use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
            use std::os::unix::process::CommandExt;
            fn pipe() -> std::io::Result<(OwnedFd, OwnedFd)> {
                let mut pair = [-1; 2];
                // SAFETY: pair has exactly the two writable descriptor slots.
                if unsafe { libc::pipe2(pair.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                // SAFETY: successful pipe2 returned two new owned descriptors.
                Ok(unsafe { (OwnedFd::from_raw_fd(pair[0]), OwnedFd::from_raw_fd(pair[1])) })
            }
            let (ready_read, ready_write) = pipe()?;
            let (gate_read, gate_write) = pipe()?;
            let read_in_parent = ready_read.as_raw_fd();
            let write_in_parent = gate_write.as_raw_fd();
            let write_in_child = ready_write.as_raw_fd();
            let read_in_child = gate_read.as_raw_fd();
            // Keep the single-byte parent gate command nonblocking too.
            // SAFETY: write_in_parent is the live, owned pipe writer.
            let flags = unsafe { libc::fcntl(write_in_parent, libc::F_GETFL) };
            if flags < 0
                || unsafe { libc::fcntl(write_in_parent, libc::F_SETFL, flags | libc::O_NONBLOCK) }
                    < 0
            {
                return Err(std::io::Error::last_os_error());
            }
            let stdout = std::fs::File::create(control.join("stdout.log"))?;
            let stderr = std::fs::File::create(control.join("stderr.log"))?;
            let worker = std::thread::spawn(move || {
                let mut command = std::process::Command::new("/bin/true");
                command.stdout(stdout).stderr(stderr);
                // Preallocate every callback buffer before fork.
                let mut stat = std::mem::MaybeUninit::<libc::stat>::zeroed();
                let mut gate = 0_u8;
                let mut frame: [u64; 8] = [
                    0x4748_3238_3646_4f52,
                    0,
                    guard_fd as u64,
                    0,
                    0,
                    0,
                    read_in_child as u64,
                    write_in_child as u64,
                ];
                // SAFETY: the callback uses only fixed stack storage and
                // async-signal-safe syscalls. All allocation and file setup
                // happens in the parent. OwnedFd drops run only in the parent.
                unsafe {
                    command.pre_exec(move || {
                        if libc::close(read_in_parent) != 0 || libc::close(write_in_parent) != 0 {
                            libc::_exit(81);
                        }
                        if libc::fstat(guard_fd, stat.as_mut_ptr()) != 0 {
                            libc::_exit(82);
                        }
                        let actual = stat.assume_init_ref();
                        let flags = libc::fcntl(guard_fd, libc::F_GETFD);
                        if flags < 0 {
                            libc::_exit(83);
                        }
                        frame[1] = libc::getpid() as u64;
                        frame[3] = actual.st_dev;
                        frame[4] = actual.st_ino;
                        frame[5] = flags as u64;
                        let mut sent = 0;
                        while sent < std::mem::size_of_val(&frame) {
                            let count = libc::write(
                                write_in_child,
                                frame.as_ptr().cast::<u8>().add(sent).cast(),
                                std::mem::size_of_val(&frame) - sent,
                            );
                            if count > 0 {
                                sent += count as usize;
                            } else if count < 0 && *libc::__errno_location() == libc::EINTR {
                                continue;
                            } else {
                                libc::_exit(84);
                            }
                        }
                        // No fallible operation after ready can let this child
                        // advance to exec without the parent's gate command.
                        loop {
                            let count = libc::read(read_in_child, (&mut gate as *mut u8).cast(), 1);
                            if count == 1 && gate == b'x' {
                                break;
                            }
                            if count < 0 && *libc::__errno_location() == libc::EINTR {
                                continue;
                            }
                            libc::_exit(85); // Includes EOF during parent cleanup.
                        }
                        libc::close(write_in_child);
                        libc::close(read_in_child);
                        Ok(())
                    });
                }
                let result = command.spawn();
                drop(ready_write);
                drop(gate_read);
                result
            });
            Ok(Self {
                ready: std::fs::File::from(ready_read),
                gate: Some(std::fs::File::from(gate_write)),
                worker: Some(worker),
                pidfd: None,
                raw_ready: Vec::new(),
                cleanup_attempted: false,
            })
        }

        fn receive_ready(&mut self) -> std::io::Result<[u64; 8]> {
            use std::io::Read;
            use std::os::fd::AsRawFd;
            let deadline = Instant::now() + Duration::from_secs(20);
            let mut frame = [0_u8; 64];
            while self.raw_ready.len() < frame.len() {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "fork ready deadline",
                    ));
                }
                let mut poll = libc::pollfd {
                    fd: self.ready.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                };
                let milliseconds = remaining.as_millis().clamp(1, 20_000) as i32;
                // SAFETY: poll points to one live descriptor record.
                let count = unsafe { libc::poll(&mut poll, 1, milliseconds) };
                if count < 0 {
                    let error = std::io::Error::last_os_error();
                    if error.kind() == std::io::ErrorKind::Interrupted {
                        continue;
                    }
                    return Err(error);
                }
                if count == 0 {
                    continue;
                }
                match self.ready.read(&mut frame[self.raw_ready.len()..]) {
                    Ok(0) => {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "fork ready EOF",
                        ))
                    }
                    Ok(count) => self
                        .raw_ready
                        .extend_from_slice(&frame[self.raw_ready.len()..][..count]),
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(error) => return Err(error),
                }
            }
            let mut words = [0_u64; 8];
            for (word, bytes) in words.iter_mut().zip(self.raw_ready.as_chunks::<8>().0) {
                let mut native = [0_u8; 8];
                native.copy_from_slice(bytes);
                *word = u64::from_ne_bytes(native);
            }
            Ok(words)
        }

        fn bind_owned_child(&mut self, pid: u32) -> std::io::Result<()> {
            use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
            // SAFETY: pidfd_open binds the actual ready PID to a kernel handle.
            let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
            if fd < 0 {
                return Err(std::io::Error::last_os_error());
            }
            // SAFETY: the syscall returned one newly owned descriptor.
            let owned = unsafe { OwnedFd::from_raw_fd(fd as i32) };
            let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
            // SAFETY: WNOWAIT proves this is our child without reaping it or
            // racing Command's own spawn/wait ownership. Unrelated PIDfds fail.
            if unsafe {
                libc::waitid(
                    libc::P_PIDFD,
                    owned.as_raw_fd() as libc::id_t,
                    info.as_mut_ptr(),
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            } != 0
            {
                return Err(std::io::Error::last_os_error());
            }
            self.pidfd = Some(owned);
            Ok(())
        }

        fn blocked_child_is_alive(&self) -> std::io::Result<bool> {
            use std::os::fd::AsRawFd;
            let fd = self
                .pidfd
                .as_ref()
                .ok_or_else(|| std::io::Error::other("no owned child pidfd"))?;
            let mut poll = libc::pollfd {
                fd: fd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: this is a nonblocking observation of our owned child.
            let result = unsafe { libc::poll(&mut poll, 1, 0) };
            if result < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(result == 0 && self.gate.is_some())
        }

        fn signal_owned_child(&self) -> std::io::Result<()> {
            use std::os::fd::AsRawFd;
            if let Some(pidfd) = &self.pidfd {
                // SAFETY: this handle was validated by non-reaping waitid.
                // No bare-PID kill can hit a reused, unrelated process.
                if unsafe {
                    libc::syscall(
                        libc::SYS_pidfd_send_signal,
                        pidfd.as_raw_fd(),
                        libc::SIGKILL,
                        std::ptr::null::<libc::siginfo_t>(),
                        0,
                    )
                } < 0
                {
                    let error = std::io::Error::last_os_error();
                    if error.raw_os_error() != Some(libc::ESRCH) {
                        return Err(error);
                    }
                }
            }
            Ok(())
        }

        fn reap_after_finished_spawn_failure(&self) -> std::io::Result<()> {
            use std::os::fd::AsRawFd;
            self.signal_owned_child()?;
            let Some(pidfd) = &self.pidfd else {
                return Ok(()); // No validated ready PID; spawn's own error cleanup owns it.
            };
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
                // SAFETY: the worker is already finished, so it cannot race
                // this bounded reap of our previously validated child pidfd.
                let result = unsafe {
                    libc::waitid(
                        libc::P_PIDFD,
                        pidfd.as_raw_fd() as libc::id_t,
                        info.as_mut_ptr(),
                        libc::WEXITED | libc::WNOHANG,
                    )
                };
                if result != 0 {
                    let error = std::io::Error::last_os_error();
                    if error.raw_os_error() == Some(libc::ECHILD) {
                        return Ok(()); // Command's failed spawn already reaped it.
                    }
                    if error.kind() != std::io::ErrorKind::Interrupted {
                        return Err(error);
                    }
                } else if unsafe { info.assume_init().si_pid() } != 0 {
                    return Ok(());
                }
                if Instant::now() >= deadline {
                    return Err(std::io::Error::other(
                        "failed spawn child could not be reaped within deadline",
                    ));
                }
                std::thread::yield_now();
            }
        }

        fn finish(&mut self, abort: bool) -> std::io::Result<std::process::ExitStatus> {
            use std::io::Write;
            self.cleanup_attempted = true;
            let mut failures = Vec::new();
            // Abort a known blocked child safely BEFORE closing the gate. If
            // ready was unavailable, gate EOF is the cancellation protocol.
            if abort {
                if let Err(error) = self.signal_owned_child() {
                    failures.push(error.to_string());
                }
            } else if let Some(gate) = &mut self.gate {
                if let Err(error) = gate.write_all(b"x") {
                    failures.push(error.to_string());
                    if let Err(error) = self.signal_owned_child() {
                        failures.push(error.to_string());
                    }
                }
            }
            drop(self.gate.take());
            let worker = self
                .worker
                .take()
                .ok_or_else(|| std::io::Error::other("spawn worker already collected"))?;
            let deadline = Instant::now() + Duration::from_secs(20);
            while !worker.is_finished() && Instant::now() < deadline {
                std::thread::yield_now();
            }
            if !worker.is_finished() {
                let kill = self.signal_owned_child();
                let stop_deadline = Instant::now() + Duration::from_secs(2);
                while !worker.is_finished() && Instant::now() < stop_deadline {
                    std::thread::yield_now();
                }
                if !worker.is_finished() {
                    // An OS-blocked spawn cannot be cancelled portably. This
                    // is an explicit FAIL, never an unbounded join or cleanup PASS.
                    return Err(std::io::Error::other(format!("spawn worker cannot be joined within deadline; owned-child kill={kill:?}; cleanup NOT certified")));
                }
                failures.push(format!(
                    "spawn deadline exceeded; owned-child kill={kill:?}"
                ));
            }
            // Joining is safe only after is_finished. The returned Child is
            // then reaped with bounded try_wait, never blocking wait().
            let spawned = match worker.join() {
                Ok(spawned) => spawned,
                Err(_) => {
                    let cleanup = self.reap_after_finished_spawn_failure();
                    return Err(std::io::Error::other(format!(
                        "spawn worker panicked; child cleanup={cleanup:?}"
                    )));
                }
            };
            let mut child = match spawned {
                Ok(child) => FixtureProcess(child),
                Err(error) => {
                    let cleanup = self.reap_after_finished_spawn_failure();
                    return Err(std::io::Error::other(format!(
                        "spawn failed: {error}; child cleanup={cleanup:?}"
                    )));
                }
            };
            let status = bounded_fixture_status(&mut child.0, Duration::from_secs(20))?;
            if !failures.is_empty() {
                return Err(std::io::Error::other(failures.join("; ")));
            }
            Ok(status)
        }
    }

    #[cfg(target_os = "linux")]
    impl Drop for ForkGuardBarrier {
        fn drop(&mut self) {
            if !self.cleanup_attempted {
                if let Err(error) = self.finish(true) {
                    eprintln!("fork guard cleanup FAIL: {error}");
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn actual_guard_fd(lock: &Path) -> std::io::Result<i32> {
        use std::os::unix::fs::MetadataExt;
        let expected = std::fs::metadata(lock)?;
        let mut matches = Vec::new();
        for entry in std::fs::read_dir("/proc/self/fd")? {
            let entry = entry?;
            let metadata = match std::fs::metadata(entry.path()) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            if (metadata.dev(), metadata.ino()) == (expected.dev(), expected.ino()) {
                let fd = entry
                    .file_name()
                    .to_string_lossy()
                    .parse::<i32>()
                    .map_err(std::io::Error::other)?;
                matches.push(fd);
            }
        }
        if matches.len() != 1 {
            return Err(std::io::Error::other(format!(
                "expected unique owned lock FD, got {matches:?}"
            )));
        }
        Ok(matches[0])
    }

    #[cfg(target_os = "linux")]
    fn consumed_report_guard_releases_while_fork_child_is_blocked(checked: bool) {
        use std::os::unix::fs::MetadataExt;
        let (_workspace, project) = retained_report_project("gh286-fork-guard-");
        let id = if checked {
            "inv-fork-checked"
        } else {
            "inv-fork-retain"
        };
        let (mut lease, root) = report_lease(&project, id);
        lease.stop_heartbeat();
        let marker = lease.path.clone();
        let original = std::fs::read(&marker).unwrap();
        let lock = report_lock_path(&project, id);
        let metadata = std::fs::metadata(&lock).unwrap();
        let guard_fd = actual_guard_fd(&lock).unwrap();
        let control = project.root.join("fork-guard-child");
        std::fs::create_dir(&control).unwrap();
        std::fs::write(
            control.join("command.json"),
            b"{\"program\":\"/bin/true\",\"args\":[],\"pre_exec\":\"guard-fd-ready/gate\"}\n",
        )
        .unwrap();
        let mut child = ForkGuardBarrier::start(guard_fd, &control).unwrap();
        // Keep all errors as values until the gate has been resolved and the
        // spawn worker/child reaped. The old-source None negative cannot hang.
        let observation = (|| -> anyhow::Result<_> {
            let ready = child.receive_ready()?;
            anyhow::ensure!(
                ready[0] == 0x4748_3238_3646_4f52,
                "invalid fork ready magic"
            );
            let pid = u32::try_from(ready[1])?;
            child.bind_owned_child(pid)?;
            anyhow::ensure!(pid != std::process::id(), "child must have a distinct PID");
            anyhow::ensure!(
                ready[2] == guard_fd as u64
                    && ready[3] == metadata.dev()
                    && ready[4] == metadata.ino(),
                "inherited actual guard FD/inode differs"
            );
            anyhow::ensure!(
                ready[5] & libc::FD_CLOEXEC as u64 != 0,
                "inherited guard FD must be CLOEXEC"
            );
            anyhow::ensure!(
                child.blocked_child_is_alive()?,
                "child exited before lifecycle transition"
            );
            let before_busy = super::ReportDirectoryGuard::try_acquire(&project, id)?.is_none();
            if checked {
                lease.release_checked()?;
            } else {
                lease.retain();
            }
            // Exactly one post-consumption attempt, while the gate is closed.
            let acquired = super::ReportDirectoryGuard::try_acquire(&project, id)?;
            let still_blocked = child.blocked_child_is_alive()?;
            std::fs::write(
                control.join("observation.json"),
                serde_json::to_vec(&serde_json::json!({
                    "parent_pid": std::process::id(), "child_pid": pid, "guard_fd": guard_fd,
                    "lock": lock, "dev": metadata.dev(), "inode": metadata.ino(),
                    "child_frame": ready, "checked_release": checked, "before_busy": before_busy,
                    "after_acquired": acquired.is_some(), "child_alive_gate_closed": still_blocked,
                }))?,
            )?;
            Ok((acquired, before_busy, still_blocked))
        })();
        let cleanup = child.finish(observation.is_err());
        std::fs::write(control.join("ready.bin"), &child.raw_ready).unwrap();
        std::fs::write(control.join("cleanup.json"), serde_json::to_vec(&serde_json::json!({
            "result": format!("{cleanup:?}"), "observation_error": observation.as_ref().err().map(|error| format!("{error:#}")),
        })).unwrap()).unwrap();
        let status = cleanup.expect("fork fixture must join/reap within its explicit deadlines");
        assert!(status.success(), "fork child status: {status:?}");
        let (acquired, before_busy, still_blocked) =
            observation.expect("actual fork observation failed");
        assert!(before_busy, "parent still owns guard before consumption");
        assert!(still_blocked, "observation must precede child exec");
        let guard = acquired
            .expect("consumed guard must unlock even while inherited child FD remains open");
        if checked {
            assert!(!marker.exists());
            assert!(guard.release_is_proven(&root).unwrap());
        } else {
            assert_eq!(std::fs::read(&marker).unwrap(), original);
            assert_eq!(std::fs::read(&lock).unwrap(), b"held\n");
            assert!(!guard.release_is_proven(&root).unwrap());
        }
        assert!(root.is_dir());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn retain_ends_guard_lifetime_before_fork_child_exec() {
        consumed_report_guard_releases_while_fork_child_is_blocked(false);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn checked_release_ends_guard_lifetime_before_fork_child_exec() {
        consumed_report_guard_releases_while_fork_child_is_blocked(true);
    }
}
