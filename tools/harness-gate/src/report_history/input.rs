//! Explicit, bounded completed-report inputs. Caller pins are not signatures.
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

const INDEX_LIMIT: u64 = 64 * 1024;
const REPORT_LIMIT: u64 = 32 * 1024 * 1024;
const CONTROL_LIMIT: u64 = 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Index {
    schema: String,
    samples: Vec<Sample>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sample {
    directory: String,
    machine_sha256: String,
    manifest_sha256: String,
}

pub(super) struct Completed {
    pub machine: Value,
    pub machine_bytes: usize,
    pub directory: PathBuf,
    pub machine_sha256: String,
    pub manifest_sha256: String,
}

pub(super) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn no_symlinks(path: &Path) -> Result<()> {
    for parent in path.ancestors() {
        if parent.as_os_str().is_empty() {
            continue;
        }
        ensure!(
            !fs::symlink_metadata(parent)?.file_type().is_symlink(),
            "history inputs cannot contain symbolic links"
        );
    }
    Ok(())
}

fn read(path: &Path, limit: u64) -> Result<Vec<u8>> {
    no_symlinks(path)?;
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_file() && metadata.len() <= limit,
        "history input must be a bounded regular file: {}",
        path.display()
    );
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= limit,
        "history input grew beyond its limit"
    );
    Ok(bytes)
}

fn json(bytes: &[u8]) -> Result<Value> {
    harness_gate::quality::parse(std::str::from_utf8(bytes)?).map_err(Into::into)
}

fn pin(expected: &str, bytes: &[u8]) -> Result<()> {
    ensure!(
        expected.len() == 64
            && expected
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "history digest must be 64 lowercase hexadecimal characters"
    );
    ensure!(
        digest(bytes) == expected,
        "history input digest differs from selected pin"
    );
    Ok(())
}

fn directory(root: &Path, relative: &str) -> Result<PathBuf> {
    ensure!(
        !relative.is_empty()
            && !relative.contains(['\\', ':'])
            && Path::new(relative)
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
        "history directories must be confined relative paths beneath the index directory"
    );
    let path = root.join(relative);
    no_symlinks(&path)?;
    ensure!(
        path.is_dir(),
        "history sample must be an invocation directory"
    );
    let path = path.canonicalize()?;
    ensure!(
        path.starts_with(root),
        "history directory escaped index root"
    );
    Ok(path)
}

fn verify_controls(directory: &Path, manifest: &[u8]) -> Result<()> {
    // Bound both control documents before invoking the shared publication validator.
    let _ = json(manifest)?;
    let _ = json(&read(
        &directory.join("artifact-registry.json"),
        CONTROL_LIMIT,
    )?)?;
    crate::verify::verify_manifest_at(directory)?;
    Ok(())
}
fn verified_bytes(directory: &Path, sample: &Sample) -> Result<Vec<u8>> {
    let bytes = read(&directory.join("test_result.json"), REPORT_LIMIT)?;
    pin(&sample.machine_sha256, &bytes)?;
    let manifest = read(&directory.join("manifest.json"), CONTROL_LIMIT)?;
    pin(&sample.manifest_sha256, &manifest)?;
    verify_controls(directory, &manifest)?;
    ensure!(
        bytes == read(&directory.join("test_result.json"), REPORT_LIMIT)?
            && manifest == read(&directory.join("manifest.json"), CONTROL_LIMIT)?,
        "history control bytes changed during validation"
    );
    Ok(bytes)
}

fn verify_identity(directory: &Path, machine: &Value) -> Result<()> {
    let metadata = json(&read(&directory.join("invocation.json"), CONTROL_LIMIT)?)?;
    for field in [
        "invocation_id",
        "input_mode",
        "project_identity",
        "source_identity",
        "execution_root",
        "configuration_digest",
    ] {
        ensure!(
            machine[field].is_string() && machine[field] == metadata[field],
            "history invocation metadata differs: {field}"
        );
    }
    ensure!(
        machine["evidence_complete"] == true,
        "history requires completed evidence"
    );
    if let Some(project_report) = machine
        .pointer("/quality/project_report")
        .filter(|v| !v.is_null())
    {
        let bound = json(&read(
            &directory.join("quality-project-report.json"),
            REPORT_LIMIT,
        )?)?;
        ensure!(
            &bound == project_report,
            "machine quality report differs from manifested project report"
        );
    }
    Ok(())
}

fn completed(root: &Path, sample: Sample) -> Result<Completed> {
    let directory = directory(root, &sample.directory)?;
    let bytes = verified_bytes(&directory, &sample)?;
    let machine = json(&bytes)?;
    verify_identity(&directory, &machine)?;
    Ok(Completed {
        machine_bytes: bytes.len(),
        machine,
        directory,
        machine_sha256: sample.machine_sha256,
        manifest_sha256: sample.manifest_sha256,
    })
}

fn index(path: &Path) -> Result<(PathBuf, Index)> {
    let absolute = std::path::absolute(path)?;
    ensure!(
        !fs::symlink_metadata(&absolute)?.file_type().is_symlink(),
        "history index cannot be a symbolic link"
    );
    let root = absolute
        .parent()
        .context("history index parent")?
        .canonicalize()?;
    let path = root.join(absolute.file_name().context("history index filename")?);
    let index: Index = serde_json::from_value(json(&read(&path, INDEX_LIMIT)?)?)?;
    ensure!(
        index.schema == "harness-report-history/v1",
        "unsupported history index schema"
    );
    ensure!(
        !index.samples.is_empty() && index.samples.len() <= 30,
        "history requires 1 to 30 explicit samples"
    );
    Ok((root, index))
}

fn invocation_id(machine: &Value) -> Result<&str> {
    let invocation = machine["invocation_id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .context("missing invocation identity")?;
    ensure!(
        invocation.len() <= 128
            && invocation
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_'),
        "invalid history invocation identifier"
    );
    Ok(invocation)
}

pub(super) fn load(path: &Path) -> Result<Vec<Completed>> {
    let (root, index) = index(path)?;
    let mut directories = BTreeSet::new();
    let mut invocations = BTreeSet::new();
    let mut reports = Vec::new();
    let mut total_bytes = 0;
    for sample in index.samples {
        let report = completed(&root, sample)?;
        total_bytes += report.machine_bytes;
        ensure!(
            total_bytes <= 64 * 1024 * 1024,
            "history report batch exceeds 64 MiB"
        );
        ensure!(
            directories.insert(report.directory.clone()),
            "duplicate history directory"
        );
        let invocation = invocation_id(&report.machine)?;
        ensure!(
            invocations.insert(invocation.to_owned()),
            "duplicate history invocation"
        );
        reports.push(report);
    }
    Ok(reports)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn index_is_bounded_and_paths_are_explicit() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("index.json");
        for value in [
            serde_json::json!({"schema":"future","samples":[]}),
            serde_json::json!({"schema":"harness-report-history/v1","samples":[]}),
            serde_json::json!({"schema":"harness-report-history/v1","samples":[],"unknown":true}),
        ] {
            fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
            assert!(load(&path).is_err());
        }
        fs::write(&path, b"{\"schema\":1,\"schema\":2}").unwrap();
        assert!(load(&path).is_err());
        fs::write(&path, vec![b' '; INDEX_LIMIT as usize + 1]).unwrap();
        assert!(load(&path).is_err());
        for name in ["..", "/outside", "sub/../outside", "C:\\outside", ""] {
            assert!(directory(root.path(), name).is_err());
        }
    }
    #[cfg(unix)]
    #[test]
    fn symbolic_link_inputs_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let actual = root.path().join("actual");
        fs::create_dir(&actual).unwrap();
        std::os::unix::fs::symlink(&actual, root.path().join("alias")).unwrap();
        assert!(directory(root.path(), "alias").is_err());
        let index = actual.join("index.json");
        fs::write(
            &index,
            br#"{"schema":"harness-report-history/v1","samples":[]}"#,
        )
        .unwrap();
        let error = load(&root.path().join("alias/index.json")).err().unwrap();
        assert!(error.to_string().contains("1 to 30"));
        std::os::unix::fs::symlink(&index, actual.join("index-link.json")).unwrap();
        assert!(load(&actual.join("index-link.json"))
            .err()
            .unwrap()
            .to_string()
            .contains("symbolic link"));
    }
}
