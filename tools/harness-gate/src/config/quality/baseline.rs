//! Trusted, ecosystem-opaque baseline transport. The generic core owns debt.
mod git;

use super::{
    compiler::{self, CompiledInputs, TrustedState},
    model::BaselineProvider,
    QualityConfig,
};
use crate::config::{FlowConfig, DEFAULT_CONFIG_PATH};
use anyhow::{ensure, Context, Result};
use harness_gate::quality::{
    evidence::{self, ValidationContext},
    parse, ratchet,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path},
};

/// Supplied by the trusted host after authenticating the CI producer, never by
/// the downloaded artifact. Pins the exact accepted base state and manifest.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request {
    schema: String,
    state: TrustedState,
    /// Repository-relative bundle manifest (also used to load Git measurements).
    manifest: String,
    manifest_sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: String,
    state: TrustedState,
    evidence: Value,
    /// Exact inventory of source/config/artifact files, relative to this manifest.
    files: BTreeMap<String, String>,
}

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn relative(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty()
            && !name.contains('\\')
            && !name.contains(':')
            && !name.chars().any(char::is_control)
            && Path::new(name)
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
            && !name
                .split('/')
                .any(|p| p.is_empty() || p == "." || p == ".." || p.eq_ignore_ascii_case(".git")),
        "unsafe baseline path: {name}"
    );
    Ok(())
}

fn file(root: &Path, name: &str) -> Result<std::path::PathBuf> {
    relative(name)?;
    let mut current = root.to_path_buf();
    for part in Path::new(name).components() {
        current.push(part);
        ensure!(
            !fs::symlink_metadata(&current)?.file_type().is_symlink(),
            "baseline symlink: {name}"
        );
    }
    ensure!(current.is_file(), "baseline file missing: {name}");
    Ok(current)
}

fn write(root: &Path, name: &str, value: &Value) -> Result<()> {
    fs::write(
        root.join(name),
        format!("{}\n", serde_json::to_string_pretty(value)?),
    )?;
    Ok(())
}

/// Output is a new directory outside the working tree. Failed materializations
/// are removed, and existing directories are never reused as a favorable base.
pub(crate) fn resolve(
    root: &Path,
    head_state: &TrustedState,
    request: Option<&Request>,
    output: &Path,
) -> Result<()> {
    let root = root.canonicalize()?;
    let parent = output
        .parent()
        .context("baseline output parent")?
        .canonicalize()?;
    let output = parent.join(output.file_name().context("baseline output name")?);
    ensure!(
        !output.starts_with(&root),
        "baseline output must be outside the working tree"
    );
    fs::create_dir(&output).context("baseline output must be a new directory")?;
    let result = resolve_into(&root, head_state, request, &output);
    if result.is_err() {
        fs::remove_dir_all(&output).context("remove failed baseline materialization")?;
    }
    result
}

fn unavailable(output: &Path, required: bool, reason: &str) -> Result<()> {
    ensure!(!required, "required baseline unavailable: {reason}");
    write(
        output,
        "resolution.json",
        &json!({"schema":"quality-baseline-resolution/v1", "status":"unavailable", "reason":reason}),
    )
}

fn resolve_into(
    root: &Path,
    head_state: &TrustedState,
    request: Option<&Request>,
    output: &Path,
) -> Result<()> {
    let head = compiler::compile(root, head_state)?;
    let flow = FlowConfig::load_with_diagnostics(&root.join(DEFAULT_CONFIG_PATH), Some(root))?;
    let config = QualityConfig::load_optional(root, &flow)?
        .context("baseline requires quality configuration")?;
    if matches!(config.baseline.provider, BaselineProvider::None) {
        return unavailable(
            output,
            config.baseline.required,
            "no baseline provider configured",
        );
    }
    let commit = match &config.baseline.provider {
        BaselineProvider::Git {
            reference,
            merge_base,
        } => {
            ensure!(
                git::commit(root, "HEAD")? == head_state.expected.commit,
                "head commit differs from trusted state"
            );
            match git::resolve(root, reference, *merge_base, &head_state.expected.commit) {
                Ok(commit) => commit,
                Err(error) => {
                    return unavailable(output, config.baseline.required, &format!("{error:#}"))
                }
            }
        }
        _ => head_state.expected.base_commit.clone(),
    };
    ensure!(
        commit != head_state.expected.commit && commit == head_state.expected.base_commit,
        "baseline cannot substitute head or differ from head lineage"
    );
    let Some(request) = request else {
        return unavailable(
            output,
            config.baseline.required,
            "trusted baseline request missing",
        );
    };
    let Some(manifest) = read_manifest(root, head_state, request, &commit, &config)? else {
        return unavailable(
            output,
            config.baseline.required,
            "baseline manifest missing",
        );
    };
    let manifest_path = root.join(&request.manifest);
    let source = output.join("source");
    fs::create_dir(&source)?;
    let from_git = matches!(config.baseline.provider, BaselineProvider::Git { .. });
    if from_git {
        git::materialize(root, &commit, &source)?;
    }
    materialize_files(
        &manifest,
        manifest_path.parent().context("manifest parent")?,
        &source,
        from_git,
    )?;
    let base = compiler::compile(&source, &request.state)?;
    base.validate_bindings(&manifest.evidence)?;
    let base_context = context(&base);
    evidence::validate_evidence_transport(&manifest.evidence, &base_context)?;
    ensure!(
        base.project["id"] == head.project["id"],
        "baseline project identity mismatch"
    );
    let default =
        json!({"schema":"subject-mappings/v1", "project":head.project["id"], "mappings":[]});
    ratchet::validate_mappings(
        &base.project,
        &head.project,
        head.mappings.as_ref().unwrap_or(&default),
    )?;
    write(output, "project.json", &base.project)?;
    write(output, "expected.json", &base.expected)?;
    write(output, "evidence.json", &manifest.evidence)?;
    write(output, "inputs.json", &serde_json::to_value(&base)?)?;
    write(
        output,
        "resolution.json",
        &json!({"schema":"quality-baseline-resolution/v1", "status":"available", "commit":commit, "manifest_sha256":request.manifest_sha256, "identity":base.identity, "source_root":base.source_root, "artifact_root":base.artifact_root}),
    )
}

fn read_manifest(
    root: &Path,
    head_state: &TrustedState,
    request: &Request,
    commit: &str,
    config: &QualityConfig,
) -> Result<Option<Manifest>> {
    ensure!(
        request.schema == "quality-baseline-request/v1",
        "unknown baseline request schema"
    );
    ensure!(
        request.state.expected.commit == commit
            && request.state.expected.target == head_state.expected.target,
        "baseline source/target identity mismatch"
    );
    ensure!(
        request.state.profile == head_state.profile && request.state.series == head_state.series,
        "incompatible baseline profile/tool/measurement series"
    );
    if let BaselineProvider::RetainedArtifact { manifest } = &config.baseline.provider {
        ensure!(
            *manifest == request.manifest,
            "baseline manifest differs from configuration"
        );
    }
    relative(&request.manifest)?;
    // Only absence is optional. A present but corrupt/incompatible bundle errors.
    if let Err(error) = fs::symlink_metadata(root.join(&request.manifest)) {
        if error.kind() == std::io::ErrorKind::NotFound {
            return Ok(None);
        }
        return Err(error.into());
    }
    let manifest_path = file(root, &request.manifest)?;
    let bytes = fs::read(&manifest_path)?;
    ensure!(
        sha(&bytes) == request.manifest_sha256,
        "baseline manifest hash mismatch"
    );
    let manifest: Manifest = serde_json::from_value(parse(std::str::from_utf8(&bytes)?)?)?;
    ensure!(
        manifest.schema == "quality-baseline-manifest/v1",
        "unknown baseline manifest schema"
    );
    ensure!(
        serde_json::to_value(&manifest.state)? == serde_json::to_value(&request.state)?,
        "stale baseline source/config/tool/series/state identity"
    );
    Ok(Some(manifest))
}

fn context(inputs: &CompiledInputs) -> ValidationContext<'_> {
    ValidationContext {
        project: &inputs.project,
        expected: &inputs.expected,
        source_root: &inputs.source_root,
        artifact_root: &inputs.artifact_root,
    }
}

fn materialize_files(
    manifest: &Manifest,
    root: &Path,
    destination: &Path,
    from_git: bool,
) -> Result<()> {
    let state = &manifest.state;
    let mut expected = state.config_files.clone();
    for subject in state.subjects.values().flatten() {
        if let Some(old) = expected.insert(subject.path.clone(), subject.source_sha256.clone()) {
            ensure!(
                old == subject.source_sha256,
                "conflicting baseline source pins"
            );
        }
    }
    let source_files = expected.clone();
    for (name, digest) in &state.artifacts {
        relative(&state.artifact_root)?;
        relative(name)?;
        ensure!(
            expected
                .insert(format!("{}/{name}", state.artifact_root), digest.clone())
                .is_none(),
            "baseline artifact aliases source/config"
        );
    }
    ensure!(
        expected == manifest.files,
        "baseline manifest file inventory mismatch"
    );
    for (name, digest) in expected {
        let bytes = fs::read(file(root, &name)?)?;
        ensure!(sha(&bytes) == digest, "baseline file hash mismatch: {name}");
        let target = destination.join(&name);
        if from_git && (source_files.contains_key(&name) || target.exists()) {
            ensure!(
                fs::read(&target).is_ok_and(|b| b == bytes),
                "baseline file differs from Git commit: {name}"
            );
        } else {
            fs::create_dir_all(target.parent().context("baseline file parent")?)?;
            fs::write(target, bytes)?;
        }
    }
    Ok(())
}
