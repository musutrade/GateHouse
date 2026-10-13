//! Configuration-driven collection. Ecosystem metadata is opaque to this module.
use super::{
    compiler::{self, CompiledInputs, TrustedState},
    validation::path,
    QualityConfig,
};
use crate::{
    config::{FlowConfig, DEFAULT_CONFIG_PATH},
    process::adapter::{self, AdapterRequest, HostPolicy},
};
use anyhow::{ensure, Context, Result};
use harness_gate::quality::{
    canonical,
    evidence::{self, ValidationContext},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

#[derive(Debug, Serialize)]
pub(crate) struct Collection {
    schema: &'static str,
    pub inputs: CompiledInputs,
    pub evidence: Value,
    pub producers: BTreeMap<String, &'static str>,
    pub responses: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    schema_version: String,
    status: String,
    invocation_id: String,
    artifacts: Vec<Value>,
    collection: Measurements,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Measurements {
    schema: String,
    evidence: Vec<Value>,
    error: Option<Value>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RetainedResponse {
    schema: String,
    binding_digest: String,
    response: Value,
}

type Claims = BTreeSet<(String, String, String)>;

fn claims(config: &QualityConfig, state: &TrustedState, id: &str) -> Result<Claims> {
    let mut claims = BTreeSet::new();
    for expectation in &config.collectors[id].produces {
        let subjects = compiler::target_subjects(&expectation.target, config, state);
        ensure!(!subjects.is_empty(), "collector has no selected subjects");
        for subject in subjects {
            ensure!(
                claims.insert((
                    subject,
                    expectation.capability.clone(),
                    expectation.series.clone()
                )),
                "duplicate producer binding"
            );
        }
    }
    Ok(claims)
}

// Exclude signed request bytes to avoid a self-referential digest. Their exact
// bytes are separately pinned by compilation and their complete payload signed.
fn binding_digest(
    config: &QualityConfig,
    state: &TrustedState,
    inputs: &CompiledInputs,
) -> Result<String> {
    let requests: BTreeSet<_> = config.collectors.values().map(|c| &c.request).collect();
    let files: BTreeMap<_, _> = state
        .config_files
        .iter()
        .filter(|(p, _)| !requests.contains(p))
        .collect();
    Ok(format!(
        "{:x}",
        Sha256::digest(canonical(&json!({
            "schema":"quality-collector-binding/v1", "config_files":files,
            "project":inputs.project, "policy":inputs.policy, "expected":inputs.expected,
            "selection":inputs.selection, "series":state.series, "profile":state.profile,
            "mappings":inputs.mappings, "exceptions":inputs.exceptions
        }))?)
    ))
}

fn input(inputs: &CompiledInputs, state: &TrustedState, id: &str, claims: &Claims) -> Value {
    json!({"schema":"harness-project-collector-request/v1", "project":inputs.project["id"],
        "collector":state.series[id].collector, "context":inputs.expected,
        "workspace_root":inputs.source_root, "output_root":inputs.artifact_root,
        "selection":inputs.selection, "bindings":claims.iter().map(|(subject, capability, series)| json!({
            "subject":subject,"capability":capability,"series":series
        })).collect::<Vec<_>>()})
}

fn inventory(root: &Path) -> Result<BTreeMap<String, String>> {
    fn visit(root: &Path, dir: &Path, files: &mut BTreeMap<String, String>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            ensure!(!kind.is_symlink(), "collector artifact symlink");
            let file = entry.path();
            if kind.is_dir() {
                visit(root, &file, files)?;
            } else {
                ensure!(kind.is_file(), "collector artifact is not regular");
                let relative = file
                    .strip_prefix(root)?
                    .to_str()
                    .context("artifact path UTF-8")?
                    .replace('\\', "/");
                files.insert(relative, format!("{:x}", Sha256::digest(fs::read(file)?)));
            }
        }
        Ok(())
    }
    let mut files = BTreeMap::new();
    visit(root, root, &mut files)?;
    Ok(files)
}

fn response(
    value: Value,
    request: &AdapterRequest,
    claims: &Claims,
    series: &Value,
) -> Result<Vec<Value>> {
    let response: Response = serde_json::from_value(value)?;
    ensure!(
        response.schema_version == "1"
            && response.status == "PASS"
            && response.invocation_id == request.invocation_id,
        "collector transport did not succeed"
    );
    ensure!(
        response.collection.schema == "harness-project-collector-response/v1"
            && response.collection.error.is_none(),
        "collector measurement error or incompatible protocol"
    );
    let mut observed = BTreeSet::new();
    let mut artifacts = BTreeMap::new();
    for record in &response.collection.evidence {
        ensure!(
            &record["series"] == series,
            "incompatible collector/tool/series identity"
        );
        for capability in record["capabilities"]
            .as_array()
            .context("missing capabilities")?
        {
            let claim = (
                record["subject"]["id"]
                    .as_str()
                    .context("subject ID")?
                    .to_owned(),
                capability["metric"]
                    .as_str()
                    .context("capability name")?
                    .to_owned(),
                record["series"]["id"]
                    .as_str()
                    .context("series ID")?
                    .to_owned(),
            );
            ensure!(observed.insert(claim), "duplicate producer evidence");
        }
        for artifact in record["artifacts"]
            .as_array()
            .context("missing artifacts")?
        {
            let name = artifact["path"]
                .as_str()
                .context("artifact path")?
                .to_owned();
            if let Some(previous) = artifacts.insert(name, artifact.clone()) {
                ensure!(previous == *artifact, "conflicting artifact descriptors");
            }
        }
    }
    ensure!(
        &observed == claims,
        "missing or unexpected subject/capability/series"
    );
    let mut declared = BTreeMap::new();
    for artifact in response.artifacts {
        ensure!(
            declared
                .insert(
                    artifact["path"]
                        .as_str()
                        .context("artifact path")?
                        .to_owned(),
                    artifact
                )
                .is_none(),
            "duplicate artifact inventory"
        );
    }
    ensure!(declared == artifacts, "mixed collector artifact inventory");
    Ok(response.collection.evidence)
}

type ProducerRequest = (String, AdapterRequest, Claims);
type RetainedRecords = BTreeMap<String, (Vec<Value>, Value)>;

fn prepare_requests(
    config: &QualityConfig,
    state: &TrustedState,
    inputs: &CompiledInputs,
    digest: &str,
) -> Result<Vec<ProducerRequest>> {
    let root = &inputs.source_root;
    let mut owners = BTreeSet::new();
    let mut requests = Vec::new();
    for id in &config.profiles[&state.profile].collectors {
        let claims = claims(config, state, id)?;
        for claim in &claims {
            ensure!(
                owners.insert((claim.0.clone(), claim.1.clone())),
                "duplicate authoritative producer"
            );
        }
        let file = path(
            root,
            &config.collectors[id].request,
            "collector request",
            true,
        )?;
        // Strict parsing rejects duplicate keys before typed adapter deserialization.
        let request = adapter::read_request(&file)?;
        ensure!(
            request.config_digest == digest,
            "stale collector config identity"
        );
        ensure!(
            request.step_id == *id && request.invocation_id == state.expected.run,
            "collector invocation identity mismatch"
        );
        ensure!(
            request.adapter.name == state.series[id].collector.name
                && request.adapter.version == state.series[id].collector.version,
            "collector package identity mismatch"
        );
        ensure!(
            request.artifact_root == inputs.artifact_root
                && request.input == input(inputs, state, id, &claims),
            "collector roots/selection/capability request mismatch"
        );
        requests.push((id.clone(), request, claims));
    }
    Ok(requests)
}

fn validate_retained(
    state: &TrustedState,
    inputs: &CompiledInputs,
    digest: &str,
    requests: &[ProducerRequest],
) -> Result<RetainedRecords> {
    let root = &inputs.source_root;
    // Validate all retained responses before launching any remaining producer.
    // A missing or stale pin is a failure, never permission to recollect.
    let mut retained = BTreeMap::new();
    for (id, request, claims) in requests {
        if let Some(reference) = state.retained.get(id) {
            let bytes = fs::read(path(root, &reference.path, "retained response", true)?)?;
            ensure!(
                format!("{:x}", Sha256::digest(&bytes)) == reference.sha256,
                "retained response digest mismatch"
            );
            let envelope: RetainedResponse = serde_json::from_value(harness_gate::quality::parse(
                std::str::from_utf8(&bytes)?,
            )?)?;
            ensure!(
                envelope.schema == "quality-retained-response/v1"
                    && envelope.binding_digest == digest,
                "stale retained configuration/profile/selection identity"
            );
            let records = response(
                envelope.response.clone(),
                request,
                claims,
                &serde_json::to_value(&state.series[id])?,
            )?;
            evidence::validate_evidence(
                &json!(records),
                &ValidationContext {
                    project: &inputs.project,
                    source_root: root,
                    artifact_root: &inputs.artifact_root,
                    expected: &inputs.expected,
                },
            )?;
            retained.insert(id.clone(), (records, serde_json::to_value(envelope)?));
        }
    }
    Ok(retained)
}

/// All selected producers fail closed. Requiredness remains owned by policy;
/// unavailable capabilities are preserved as evidence, never converted to values.
pub(crate) fn collect(
    root: &Path,
    state: &TrustedState,
    policy: &HostPolicy,
) -> Result<Collection> {
    let inputs = compiler::compile(root, state)?;
    let root = &inputs.source_root;
    let flow = FlowConfig::load_with_diagnostics(&root.join(DEFAULT_CONFIG_PATH), Some(root))?;
    let config = QualityConfig::load_optional(root, &flow)?.context("missing quality config")?;
    ensure!(
        inventory(&inputs.artifact_root)? == state.artifacts
            && (!state.retained.is_empty() || state.artifacts.is_empty()),
        "collection requires a fresh artifact root or pinned retained artifacts"
    );
    ensure!(
        state
            .retained
            .keys()
            .all(|id| config.profiles[&state.profile].collectors.contains(id)),
        "retained evidence references an inactive producer"
    );
    let digest = binding_digest(&config, state, &inputs)?;
    let requests = prepare_requests(&config, state, &inputs, &digest)?;
    let mut retained = validate_retained(state, &inputs, &digest, &requests)?;
    let mut records = Vec::new();
    let mut producers = BTreeMap::new();
    let mut responses = BTreeMap::new();
    for (id, request, claims) in requests {
        if let Some((reused, response)) = retained.remove(&id) {
            records.extend(reused);
            producers.insert(id.clone(), "retained");
            responses.insert(id.clone(), response);
            continue;
        }
        let outcome =
            adapter::run(request.clone(), policy).with_context(|| format!("collector {id}"))?;
        records.extend(response(
            outcome.response.clone(),
            &request,
            &claims,
            &serde_json::to_value(&state.series[&id])?,
        )?);
        producers.insert(id.clone(), "collected");
        responses.insert(
            id.clone(),
            json!({
                "schema":"quality-retained-response/v1", "binding_digest":digest,
                "response":outcome.response
            }),
        );
    }
    let records = Value::Array(records);
    // A profile with no selected producers has no measurement batch. Preserve
    // that omission without weakening the nonempty core evidence contract.
    if !config.profiles[&state.profile].collectors.is_empty() {
        evidence::validate_evidence(
            &records,
            &ValidationContext {
                project: &inputs.project,
                source_root: root,
                artifact_root: &inputs.artifact_root,
                expected: &inputs.expected,
            },
        )?;
    }
    let mut collected = state.clone();
    collected.artifacts = inventory(&inputs.artifact_root)?;
    ensure!(
        state
            .artifacts
            .iter()
            .all(|(name, digest)| collected.artifacts.get(name) == Some(digest)),
        "retained artifacts changed during collection"
    );
    let referenced: BTreeSet<_> = records
        .as_array()
        .context("evidence array")?
        .iter()
        .flat_map(|r| r["artifacts"].as_array().into_iter().flatten())
        .filter_map(|a| a["path"].as_str())
        .collect();
    ensure!(
        referenced
            .into_iter()
            .eq(collected.artifacts.keys().map(String::as_str)),
        "undeclared collector artifact"
    );
    // Recheck pinned config/source bytes after every collector has finished.
    let inputs = compiler::compile(root, &collected)?;
    inputs.validate_bindings(&records)?;
    Ok(Collection {
        schema: "quality-collection/v1",
        inputs,
        evidence: records,
        producers,
        responses,
    })
}

#[cfg(all(test, unix))]
mod tests;
