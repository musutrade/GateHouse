//! Opt-in append-only local snapshots and a recoverable display index.
mod filesystem;
use super::input::{self, Sample};
use anyhow::{ensure, Context, Result};
use serde_json::json;
use std::{collections::BTreeSet, fs, path::Path};

const MAX_SNAPSHOTS: usize = 300;
const MAX_TOTAL: u64 = 1024 * 1024 * 1024;
const MAX_MACHINE: usize = 64 * 1024 * 1024;

pub(crate) fn archive_path(path: &str, reports: &str, templates: Option<&str>) -> Result<()> {
    ensure!(
        filesystem::relative(path),
        "history archive requires a canonical repository-relative path"
    );
    for other in [Some(reports), templates].into_iter().flatten() {
        ensure!(
            !Path::new(path).starts_with(other) && !Path::new(other).starts_with(path),
            "history archive must be separate from reports and templates"
        );
    }
    Ok(())
}

pub(crate) fn record(project: &crate::project::Project) -> Result<()> {
    let Some(path) = &project.config.report_history.archive else {
        return Ok(());
    };
    archive_path(
        path,
        &project.config.paths.reports,
        project.config.report_templates.root.as_deref(),
    )?;
    let root = filesystem::directory(&project.root, path)?;
    let _lock = filesystem::Lock::acquire(&root)?;
    initialize(&root, &project.input().project_identity)?;
    let source = project.reports.canonicalize()?;
    ensure!(
        !source.starts_with(&root) && !root.starts_with(&source),
        "archive overlaps invocation"
    );
    append(&root, &source, &project.input().project_identity)
}

fn append(root: &Path, source: &Path, identity: &str) -> Result<()> {
    let parent = source.parent().context("invocation parent")?;
    // Bound filesystem work before the shared validator hashes manifested artifacts.
    filesystem::inventory(source)?;
    let sample = input::local_sample(parent, source)?;
    let report = input::completed(parent, sample.clone())?;
    ensure!(
        report.machine["project_identity"] == identity,
        "archive input project differs"
    );
    let (mut samples, mut invocations, total, machine_bytes) = scan(root, identity)?;
    let name = format!(
        "snap-{}",
        input::digest(format!("{}:{}", sample.machine_sha256, sample.manifest_sha256).as_bytes())
    );
    let new = Sample {
        directory: format!("snapshots/{name}/publication"),
        ..sample
    };
    if !samples.contains(&new) {
        ensure!(
            samples.len() < MAX_SNAPSHOTS,
            "history archive is full (300 snapshots); select a new archive directory"
        );
        ensure!(
            invocations.insert(input::invocation_id(&report.machine)?.to_owned()),
            "history invocation changed after archival"
        );
        ensure!(
            machine_bytes + report.machine_bytes <= MAX_MACHINE,
            "history machine reports exceed 64 MiB; select a new archive directory"
        );
        snapshot(root, source, &name, &new, MAX_TOTAL - total)?;
        samples.push(new);
    }
    publish_index(root, samples)
}

fn initialize(root: &Path, identity: &str) -> Result<()> {
    let marker = root.join("archive.json");
    let expected = json!({"schema":"harness-report-archive/v1","project_identity":identity});
    if marker.try_exists()? {
        ensure!(
            input::json(&input::read(&marker, 16 * 1024)?)? == expected,
            "archive belongs to a different project or format"
        );
    } else {
        ensure!(
            fs::read_dir(root)?.all(|entry| entry.is_ok_and(|e| e.file_name() == ".archive.lock")),
            "refusing to initialize a nonempty history archive"
        );
        filesystem::write_new(&marker, &serde_json::to_vec(&expected)?)?;
    }
    ignore(root)?;
    filesystem::directory(root, "snapshots")?;
    namespace(root)
}
fn ignore(root: &Path) -> Result<()> {
    let ignore = root.join(".gitignore");
    if !ignore.try_exists()? {
        filesystem::write_new(&ignore, b"*\n")?;
    }
    ensure!(
        input::read(&ignore, 1024)? == b"*\n",
        "archive ignore file changed; preserving it"
    );
    Ok(())
}
fn namespace(root: &Path) -> Result<()> {
    for entry in fs::read_dir(root)? {
        let name = entry?.file_name();
        ensure!(
            [
                "archive.json",
                "index.json",
                "snapshots",
                ".archive.lock",
                ".gitignore"
            ]
            .iter()
            .any(|allowed| name == *allowed),
            "unexpected archive root entry; preserving it"
        );
    }
    Ok(())
}

type Scan = (Vec<Sample>, BTreeSet<String>, u64, usize);
fn scan(root: &Path, identity: &str) -> Result<Scan> {
    let prior = prior_index(root)?;
    let mut samples = Vec::new();
    let mut invocations = BTreeSet::new();
    let mut total = 0;
    let mut machine_bytes = 0;
    for entry in fs::read_dir(root.join("snapshots"))? {
        ensure!(
            samples.len() < MAX_SNAPSHOTS,
            "history archive exceeds 300 snapshots"
        );
        let entry = entry?;
        let path = entry.path();
        let sample = snapshot_entry(&path, &entry.file_name().to_string_lossy())?;
        total += filesystem::inventory(&path)?.1;
        ensure!(total <= MAX_TOTAL, "history archive exceeds byte budget");
        let report = input::completed(root, sample.clone())?;
        ensure!(
            report.machine["project_identity"] == identity,
            "history archive mixes projects"
        );
        ensure!(
            invocations.insert(input::invocation_id(&report.machine)?.to_owned()),
            "duplicate archived invocation"
        );
        machine_bytes += report.machine_bytes;
        ensure!(
            machine_bytes <= MAX_MACHINE,
            "history archive exceeds byte budget"
        );
        samples.push(sample);
    }
    ensure!(
        prior.iter().all(|old| samples.contains(old)),
        "indexed history changed or disappeared; preserving index"
    );
    Ok((samples, invocations, total, machine_bytes))
}
fn prior_index(root: &Path) -> Result<Vec<Sample>> {
    Ok(if root.join("index.json").try_exists()? {
        let (_, index, _) = input::index(&root.join("index.json"))?;
        ensure!(
            index.schema == "harness-report-history/v2",
            "managed archive requires v2 index"
        );
        let mut directories = BTreeSet::new();
        ensure!(
            index
                .samples
                .iter()
                .all(|sample| directories.insert(&sample.directory)),
            "duplicate managed history index entry"
        );
        index.samples
    } else {
        Vec::new()
    })
}

fn snapshot_entry(path: &Path, name: &str) -> Result<Sample> {
    ensure!(
        fs::symlink_metadata(path)?.is_dir(),
        "archive snapshot must be a regular directory"
    );
    for entry in fs::read_dir(path)? {
        let name = entry?.file_name();
        ensure!(
            name == "entry.json" || name == "publication",
            "unexpected snapshot entry; preserving it"
        );
    }
    let sample: Sample =
        serde_json::from_value(input::json(&input::read(&path.join("entry.json"), 4096)?)?)?;
    let expected = format!(
        "snap-{}",
        input::digest(format!("{}:{}", sample.machine_sha256, sample.manifest_sha256).as_bytes())
    );
    ensure!(
        name == expected && sample.directory == format!("snapshots/{name}/publication"),
        "archive entry identity differs from snapshot name"
    );
    Ok(sample)
}

fn snapshot(root: &Path, source: &Path, name: &str, sample: &Sample, remaining: u64) -> Result<()> {
    let (files, total) = filesystem::inventory(source)?;
    let entry = serde_json::to_vec(sample)?;
    ensure!(
        total + entry.len() as u64 <= remaining.min(256 * 1024 * 1024),
        "history archive exceeds 1 GiB; select a new archive directory"
    );
    let snapshots = root.join("snapshots");
    let stage = tempfile::Builder::new()
        .prefix(".pending-")
        .tempdir_in(&snapshots)?;
    stage_snapshot(stage.path(), source, &files, sample, &entry)?;
    ensure!(
        filesystem::inventory(stage.path())?.1 <= remaining.min(256 * 1024 * 1024),
        "copied history snapshot exceeds archive budget"
    );
    let target = snapshots.join(name);
    ensure!(!target.try_exists()?, "history snapshot already exists");
    fs::rename(stage.path(), target).context("publish complete history snapshot")?;
    Ok(())
}
fn stage_snapshot(
    stage: &Path,
    source: &Path,
    files: &[std::path::PathBuf],
    sample: &Sample,
    entry: &[u8],
) -> Result<()> {
    let publication = filesystem::directory(stage, "publication")?;
    filesystem::copy(source, &publication, files)?;
    let copied = input::local_sample(stage, &publication)?;
    ensure!(
        copied.machine_sha256 == sample.machine_sha256
            && copied.manifest_sha256 == sample.manifest_sha256,
        "archive input changed during copy"
    );
    input::completed(stage, copied)?;
    filesystem::write_new(&stage.join("entry.json"), entry)?;
    Ok(())
}

fn publish_index(root: &Path, mut samples: Vec<Sample>) -> Result<()> {
    samples.sort_by(|a, b| a.directory.cmp(&b.directory));
    let bytes = serde_json::to_vec_pretty(
        &json!({"schema":"harness-report-history/v2","samples":samples}),
    )?;
    ensure!(bytes.len() <= 256 * 1024, "history index exceeds 256 KiB");
    filesystem::replace(&root.join("index.json"), &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn archive_namespace_rejects_foreign_and_cross_project_state() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        fs::write(root.join("foreign"), b"keep").unwrap();
        assert!(initialize(&root, "project").is_err());
        assert_eq!(fs::read(root.join("foreign")).unwrap(), b"keep");
        fs::remove_file(root.join("foreign")).unwrap();
        initialize(&root, "project").unwrap();
        initialize(&root, "project").unwrap();
        assert!(initialize(&root, "different").is_err());
        assert!(scan(&root, "project").unwrap().0.is_empty());
        fs::write(root.join(".gitignore"), b"custom\n").unwrap();
        assert!(initialize(&root, "project").is_err());
        assert_eq!(fs::read(root.join(".gitignore")).unwrap(), b"custom\n");
        fs::write(root.join(".gitignore"), b"*\n").unwrap();
        fs::write(root.join("unexpected"), b"keep").unwrap();
        assert!(initialize(&root, "project").is_err());
        assert!(archive_path(".git/archive", "reports", None).is_err());
        assert!(archive_path(".GIT/archive", "reports", None).is_err());
        assert!(archive_path("a/.. ", "reports", None).is_err());
        assert!(archive_path("reports/history", "reports", None).is_err());
        assert!(archive_path("templates/history", "reports", Some("templates")).is_err());
    }
    #[test]
    fn malformed_entry_and_prior_index_are_never_rebuilt_silently() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        initialize(&root, "project").unwrap();
        let entry = filesystem::directory(&root, "snapshots/foreign").unwrap();
        let sample = Sample {
            directory: "snapshots/foreign/publication".into(),
            machine_sha256: "a".repeat(64),
            manifest_sha256: "b".repeat(64),
        };
        fs::write(
            entry.join("entry.json"),
            serde_json::to_vec(&sample).unwrap(),
        )
        .unwrap();
        assert!(snapshot_entry(&entry, "foreign").is_err());
        fs::write(entry.join("extra"), b"keep").unwrap();
        assert!(snapshot_entry(&entry, "foreign").is_err());
        fs::write(root.join("index.json"), b"{bad").unwrap();
        assert!(scan(&root, "project").is_err());
        assert_eq!(fs::read(root.join("index.json")).unwrap(), b"{bad");
        fs::write(
            root.join("index.json"),
            serde_json::to_vec(&json!({"schema":"harness-report-history/v1","samples":[sample]}))
                .unwrap(),
        )
        .unwrap();
        assert!(prior_index(&root).is_err());
        fs::write(
            root.join("index.json"),
            serde_json::to_vec(&json!({
                "schema":"harness-report-history/v2", "samples":[sample.clone(), sample]
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(prior_index(&root).is_err());
    }
}
