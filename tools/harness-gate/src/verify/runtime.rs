//! Measured runtime facts, separate from quality policy. This is not an
//! isolation attestation: a local process cannot certify its own host boundary.
use crate::{config::quality::identity::digest, process::adapter::HostPolicy, project::Project};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, io::Read, sync::OnceLock};

fn core_digest() -> Result<&'static str> {
    static CORE: OnceLock<Result<String, String>> = OnceLock::new();
    CORE.get_or_init(|| {
        (|| -> Result<String> {
            let executable = fs::File::open(std::env::current_exe()?)?;
            let size = executable.metadata()?.len();
            ensure!(
                size <= 512 * 1024 * 1024,
                "Core executable exceeds identity limit"
            );
            let mut hash = Sha256::new();
            let count = std::io::copy(&mut executable.take(size + 1), &mut hash)?;
            ensure!(
                count == size,
                "Core executable changed while identifying runtime"
            );
            Ok(format!("sha256:{:x}", hash.finalize()))
        })()
        .map_err(|error| format!("{error:#}"))
    })
    .as_deref()
    .map_err(|error| anyhow::anyhow!("cannot identify executing Core: {error}"))
}

pub(super) fn facts(project: &Project, host: &HostPolicy) -> Result<Value> {
    let keys = host
        .trusted_keys
        .iter()
        .map(|key| (&key.key_id, &key.public_key))
        .collect::<std::collections::BTreeMap<_, _>>();
    let execution = json!({
        "core_sha256": core_digest()?, "core_version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
        "execution": project.config.execution,
        "steps": project.config.steps.iter().map(|step| json!({
            "id": step.id, "program": step.program, "args": step.args,
            "cwd": step.cwd, "runner": step.runner, "input": step.input,
            "timeout_secs": step.timeout_secs, "timeout_env": step.timeout_env,
            "remove_env": step.remove_env, "services": step.services,
        })).collect::<Vec<_>>(),
        "services": project.config.services, "service_checks": project.config.service_checks,
        "host_policy": {
            "trusted_keys": keys, "network": host.capabilities.network,
            "resources": host.capabilities.resources, "environment": host.capabilities.environment,
            "max_timeout": host.max_timeout, "max_request_age": host.max_request_age,
            "clock_skew": host.clock_skew, "max_stdout_bytes": host.max_stdout_bytes,
            "max_stderr_bytes": host.max_stderr_bytes, "max_output_bytes": host.max_output_bytes,
            "reader_deadline": host.reader_deadline, "max_artifact_bytes": host.max_artifact_bytes,
            "durable_replay": host.replay_state_dir.is_some(),
        },
        "environment_binding": "not_attested",
        "isolation_attestation": "not_provided",
    });
    // Arguments and service configuration can contain credentials. Bind their
    // bytes without copying them into a report or reading ambient secret values.
    let configuration = json!({
        "execution": execution["execution"], "steps": execution["steps"],
        "services": execution["services"], "service_checks": execution["service_checks"],
    });
    let facts = json!({
        "core_sha256": execution["core_sha256"], "core_version": execution["core_version"],
        "os": execution["os"], "arch": execution["arch"],
        "configuration_digest": digest("runtime-configuration/v1", &configuration)?,
        "host_policy_digest": digest("collector-host-policy/v1", &execution["host_policy"])?,
        "environment_binding": "not_attested", "isolation_attestation": "not_provided",
    });
    Ok(json!({"facts": facts, "runtime_bundle_digest": digest("runtime-bundle/v1", &facts)?}))
}

pub(super) fn bind_collection(
    runtime: &mut Value,
    inputs: &Value,
    producers: &Value,
    collectors: &Value,
) -> Result<()> {
    let facts = runtime.get_mut("facts").context("runtime facts missing")?;
    // Bind collector tool/runtime/source series, selected adapter settings and
    // collected/retained origin states together. The origin labels are not
    // signatures or an independent host attestation.
    facts["series"] = inputs["project"]["series"].clone();
    facts["collectors"] = collectors.clone();
    facts["collector_producers"] = producers.clone();
    let identity = digest("runtime-bundle/v1", facts)?;
    runtime["runtime_bundle_digest"] = json!(identity);
    Ok(())
}

pub(super) fn local_facts(project: &Project) -> Result<Value> {
    let mut runtime = facts(project, &HostPolicy::default())?;
    runtime["facts"]["host_policy_digest"] = Value::Null;
    runtime["runtime_bundle_digest"] = json!(digest("runtime-bundle/v1", &runtime["facts"])?);
    Ok(runtime)
}
