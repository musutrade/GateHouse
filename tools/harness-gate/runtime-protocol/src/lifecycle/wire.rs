//! Strict lifecycle wire facts. No I/O, journal, authorization tokens or effects.
use super::canonical;
use crate::{digest, nonempty, Invalid, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PROTOCOL: &str = "gate-runtime/lifecycle-v1";
const DOMAIN: &[u8] = b"gate-runtime/lifecycle-v1/request\0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineBinding {
    pub engine: String,
    pub transport: String,
    pub socket_path: String,
    pub socket_uid: u32,
    pub principal_uid: u32,
    pub rootless: bool,
    pub api_version: String,
    pub server_version: String,
    pub daemon_id: String,
    pub scope: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub installation_sha256: String,
    pub executable_sha256: String,
    pub config_sha256: String,
    pub engine_binding: EngineBinding,
    pub project_id: String,
    pub resource_id: String,
    pub lease_invocation: String,
    pub invocation: String,
    pub generation: u64,
    pub operation_key: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub key: String,
    pub value: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub container_port: u16,
    pub object_id: String,
    pub name: String,
    pub labels: Vec<Entry>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Operation {
    ResolveImage {
        image: String,
    },
    Create {
        name: String,
        image_id: String,
        labels: Vec<Entry>,
        environment: Vec<Entry>,
        container_port: u16,
        /// Explicit argv for Podman 5.7 lifecycle profiles. Docker lifecycle
        /// requests omit this field and retain their original wire bytes.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        command: Option<Vec<String>>,
    },
    Status {
        target: Target,
    },
    Start {
        target: Target,
    },
    Stop {
        target: Target,
        grace_seconds: u8,
    },
    Remove {
        target: Target,
    },
    HealthExec {
        target: Target,
        argv: Vec<String>,
        timeout_ms: u32,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub protocol: String,
    pub binding: Binding,
    pub operation: Operation,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContainerState {
    Created,
    Running,
    Paused,
    Restarting,
    Removing,
    Exited,
    Dead,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Port {
    pub container_port: u16,
    pub protocol: String,
    pub host_ip: String,
    pub host_port: u16,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Object {
    pub image_id: String,
    pub object_id: String,
    pub name: String,
    pub labels: Vec<Entry>,
    pub state: ContainerState,
    pub ports: Vec<Port>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub api_version: String,
    pub server_version: String,
    pub daemon_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observations {
    pub observed_before: Observation,
    pub observed_after: Observation,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Absence {
    pub object_id: String,
    pub route: String,
    pub http_status: u16,
    pub error_message_sha256: String,
}

fn absence_route(engine: &EngineBinding, object_id: &str) -> Result<String> {
    digest(object_id)?;
    Ok(match engine.engine.as_str() {
        "docker" => format!("/v{}/containers/{object_id}/json", engine.api_version),
        "podman" => format!(
            "/v{}/libpod/containers/{object_id}/json",
            engine.api_version
        ),
        _ => return Err(Invalid("invalid lifecycle engine")),
    })
}

/// Engine-specific exact missing-object message. This is a route/message fact,
/// never absence authority by itself.
fn absence_message(engine: &EngineBinding, object_id: &str) -> Result<String> {
    digest(object_id)?;
    Ok(match engine.engine.as_str() {
        "docker" => format!("No such container: {object_id}"),
        "podman" => {
            format!("no container with name or ID \"{object_id}\" found: no such container")
        }
        _ => return Err(Invalid("invalid lifecycle engine")),
    })
}

/// Build exact absence evidence for a product adapter or a test. Core still
/// requires a matching validated response and the durable journal transition.
pub fn exact_absence(engine: &EngineBinding, target: &Target) -> Result<Absence> {
    engine.validate()?;
    let route = absence_route(engine, &target.object_id)?;
    let message = absence_message(engine, &target.object_id)?;
    Ok(Absence {
        object_id: target.object_id.clone(),
        route,
        http_status: 404,
        error_message_sha256: format!("{:x}", Sha256::digest(message.as_bytes())),
    })
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reason {
    InvalidInput,
    IdentityMismatch,
    Transport,
    Deadline,
    Cancelled,
    InvalidResponse,
    UnsupportedState,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Outcome {
    Image {
        image_id: String,
        repo_digests: Vec<String>,
    },
    Created {
        object: Object,
    },
    Status {
        object: Object,
    },
    Started {
        object: Object,
    },
    Stopped {
        object: Object,
    },
    Removed,
    Absent {
        evidence: Absence,
    },
    Health {
        exec_id: String,
        exit_code: i32,
        object: Object,
    },
    RejectedBeforeEffect {
        reason: Reason,
    },
    Unknown {
        reason: Reason,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub protocol: String,
    pub binding: Binding,
    pub operation: Operation,
    pub outcome: Outcome,
    #[serde(deserialize_with = "required_observations")]
    pub observations: Option<Observations>,
}
fn required_observations<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<Observations>, D::Error> {
    Deserialize::deserialize(d)
}
fn hex32(s: &str) -> Result<()> {
    if s.len() == 32
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        Ok(())
    } else {
        Err(Invalid("invalid lifecycle nonce"))
    }
}
fn name(s: &str) -> Result<()> {
    if s.is_empty()
        || s.len() > 128
        || !s.as_bytes()[0].is_ascii_alphanumeric()
        || !s
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
    {
        return Err(Invalid("invalid container name"));
    }
    Ok(())
}
fn image_id(s: &str) -> Result<()> {
    digest(
        s.strip_prefix("sha256:")
            .ok_or(Invalid("invalid image ID"))?,
    )
}
fn entries(items: &[Entry]) -> Result<()> {
    if items.len() > 128 {
        return Err(Invalid("too many lifecycle entries"));
    }
    let mut last: Option<&str> = None;
    for e in items {
        nonempty(&e.key)?;
        if e.key.len() > 256
            || e.value.len() > 4096
            || e.value.contains('\0')
            || last.is_some_and(|p| p >= e.key.as_str())
        {
            return Err(Invalid("invalid lifecycle entries"));
        }
        last = Some(&e.key);
    }
    Ok(())
}
fn ownership(labels: &[Entry], b: &Binding) -> Result<()> {
    entries(labels)?;
    for (key, expected) in [
        (
            "gatehouse.lifecycle.authority",
            "container-lifecycle/mutate",
        ),
        ("gatehouse.lifecycle.project", b.project_id.as_str()),
        ("gatehouse.lifecycle.resource", b.resource_id.as_str()),
        (
            "gatehouse.lifecycle.invocation",
            b.lease_invocation.as_str(),
        ),
    ] {
        if !labels.iter().any(|e| e.key == key && e.value == expected) {
            return Err(Invalid("lifecycle ownership mismatch"));
        }
    }
    Ok(())
}
impl Binding {
    pub fn validate(&self) -> Result<()> {
        for d in [
            &self.installation_sha256,
            &self.executable_sha256,
            &self.config_sha256,
            &self.operation_key,
        ] {
            digest(d)?;
        }
        hex32(&self.lease_invocation)?;
        hex32(&self.invocation)?;
        for s in [
            &self.project_id,
            &self.resource_id,
            &self.engine_binding.server_version,
            &self.engine_binding.daemon_id,
            &self.engine_binding.scope,
        ] {
            nonempty(s)?;
        }
        if self.generation == 0 {
            return Err(Invalid("invalid lifecycle generation"));
        }
        self.engine_binding.validate()
    }
}
impl EngineBinding {
    pub fn validate(&self) -> Result<()> {
        let e = self;
        for s in [&e.server_version, &e.daemon_id, &e.scope] {
            nonempty(s)?;
        }
        if e.transport != "unix-http"
            || !e.socket_path.starts_with('/')
            || e.socket_path[1..]
                .split('/')
                .any(|s| matches!(s, "" | "." | ".."))
        {
            return Err(Invalid("invalid lifecycle endpoint"));
        }
        nonempty(&e.socket_path)?;
        match e.engine.as_str() {
            "docker" => {}
            // Native Libpod has no Docker API version negotiation and no daemon
            // UUID. The selected 5.7 profile is rootless-only; `daemon_id` is the
            // observed host name and `scope` the observed graph root, not an
            // authentication claim or restart proof.
            "podman" => {
                if e.api_version != "5.7.0"
                    || e.server_version != "5.7.0"
                    || !e.rootless
                    || e.principal_uid == 0
                {
                    return Err(Invalid("unsupported Podman lifecycle profile"));
                }
            }
            _ => return Err(Invalid("invalid lifecycle engine")),
        }
        if e.engine == "docker" {
            let api: Vec<_> = e.api_version.split('.').collect();
            if api.len() != 2
                || api
                    .iter()
                    .any(|v| !v.parse::<u16>().is_ok_and(|n| n.to_string() == *v))
            {
                return Err(Invalid("invalid lifecycle API version"));
            }
        }
        Ok(())
    }
}
impl Operation {
    fn target(&self) -> Option<&Target> {
        match self {
            Self::Status { target }
            | Self::Start { target }
            | Self::Stop { target, .. }
            | Self::Remove { target }
            | Self::HealthExec { target, .. } => Some(target),
            _ => None,
        }
    }
    fn validate(&self, b: &Binding) -> Result<()> {
        if let Some(t) = self.target() {
            if t.container_port == 0 {
                return Err(Invalid("invalid target port"));
            }
            digest(&t.object_id)?;
            name(&t.name)?;
            ownership(&t.labels, b)?;
        }
        match self {
            Self::ResolveImage { image } => crate::v3::validate_image(image)?,
            Self::Create {
                name: n,
                image_id: id,
                labels,
                environment,
                container_port,
                command,
            } => {
                name(n)?;
                image_id(id)?;
                ownership(labels, b)?;
                entries(environment)?;
                if *container_port == 0 || environment.iter().any(|e| e.key.contains('=')) {
                    return Err(Invalid("invalid create parameters"));
                }
                if let Some(command) = command {
                    if command.is_empty()
                        || command.len() > 64
                        || command.iter().any(|a| a.is_empty() || a.contains('\0'))
                        || command.iter().map(String::len).sum::<usize>() > 8192
                    {
                        return Err(Invalid("invalid create command"));
                    }
                }
            }
            Self::Stop { grace_seconds, .. } if !(1..=30).contains(grace_seconds) => {
                return Err(Invalid("invalid stop grace"))
            }
            Self::HealthExec {
                argv, timeout_ms, ..
            } if argv.is_empty()
                || argv.len() > 64
                || !(1..=30000).contains(timeout_ms)
                || argv.iter().any(|a| a.is_empty() || a.contains('\0'))
                || argv.iter().map(String::len).sum::<usize>() > 8192 =>
            {
                return Err(Invalid("invalid health command"));
            }
            _ => {}
        }
        Ok(())
    }
}
impl Request {
    pub fn operation_key(&self) -> Result<String> {
        let mut value =
            serde_json::to_value(self).map_err(|_| Invalid("invalid request encoding"))?;
        value["binding"]
            .as_object_mut()
            .ok_or(Invalid("missing binding"))?
            .remove("operation_key");
        let mut hash = Sha256::new();
        hash.update(DOMAIN);
        hash.update(canonical::encode(&value)?);
        Ok(format!("{:x}", hash.finalize()))
    }
    pub fn validate(&self) -> Result<()> {
        if self.protocol != PROTOCOL {
            return Err(Invalid("unsupported lifecycle protocol"));
        }
        self.binding.validate()?;
        self.operation.validate(&self.binding)?;
        if self.operation_key()? != self.binding.operation_key {
            return Err(Invalid("lifecycle request digest mismatch"));
        }
        Ok(())
    }
    /// Canonical bytes for an already minted, validated request. Does not mint
    /// authority or obtain random nonces on behalf of a caller.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        canonical::encode(
            &serde_json::to_value(self).map_err(|_| Invalid("invalid request encoding"))?,
        )
    }
}

pub fn parse_request(bytes: &[u8]) -> Result<Request> {
    let value = canonical::parse_canonical(bytes)?;
    let request: Request =
        serde_json::from_value(value).map_err(|_| Invalid("invalid lifecycle request schema"))?;
    request.validate()?;
    if request.canonical_bytes()? != bytes {
        return Err(Invalid("lifecycle request fields changed during parsing"));
    }
    Ok(request)
}
/// Both byte inputs must originate from separately bounded host channels. This
/// validates equality, not the file ownership/provenance of the protected input.
pub fn parse_protected_request(stdin: &[u8], protected: &[u8]) -> Result<Request> {
    let expected = parse_request(protected)?;
    let supplied = parse_request(stdin)?;
    if stdin != protected || supplied != expected {
        return Err(Invalid("lifecycle protected input mismatch"));
    }
    Ok(expected)
}

fn object_matches(object: &Object, target: &Target) -> Result<()> {
    if object.object_id != target.object_id
        || object.name != target.name
        || !target.labels.iter().all(|e| object.labels.contains(e))
    {
        return Err(Invalid("lifecycle object mismatch"));
    }
    image_id(&object.image_id)?;
    digest(&object.object_id)?;
    name(&object.name)?;
    entries(&object.labels)?;
    if object.ports.len() > 1 {
        return Err(Invalid("too many port bindings"));
    }
    for p in &object.ports {
        if p.protocol != "tcp"
            || p.host_ip != "127.0.0.1"
            || p.container_port != target.container_port
            || p.host_port == 0
        {
            return Err(Invalid("invalid port binding"));
        }
    }
    for (i, p) in object.ports.iter().enumerate() {
        if object.ports[..i]
            .iter()
            .any(|old| old.container_port == p.container_port)
        {
            return Err(Invalid("ambiguous port binding"));
        }
    }
    Ok(())
}
impl Response {
    pub fn validate(&self, expected: &Request) -> Result<()> {
        expected.validate()?;
        if self.protocol != expected.protocol
            || self.binding != expected.binding
            || self.operation != expected.operation
        {
            return Err(Invalid("lifecycle response binding mismatch"));
        }
        if matches!(
            self.outcome,
            Outcome::Unknown { .. } | Outcome::RejectedBeforeEffect { .. }
        ) {
            return if self.observations.is_none() {
                Ok(())
            } else {
                Err(Invalid("invalid failure observations"))
            };
        }
        self.validate_observations(expected)?;
        self.validate_outcome(expected)
    }
    fn validate_observations(&self, expected: &Request) -> Result<()> {
        let obs = self
            .observations
            .as_ref()
            .ok_or(Invalid("missing lifecycle observations"))?;
        let e = &expected.binding.engine_binding;
        for o in [&obs.observed_before, &obs.observed_after] {
            if o.api_version != e.api_version
                || o.server_version != e.server_version
                || o.daemon_id != e.daemon_id
            {
                return Err(Invalid("lifecycle observation mismatch"));
            }
        }
        Ok(())
    }
    fn validate_outcome(&self, expected: &Request) -> Result<()> {
        let e = &expected.binding.engine_binding;
        use ContainerState as S;
        match (&expected.operation, &self.outcome) {
            (
                Operation::ResolveImage { image },
                Outcome::Image {
                    image_id: id,
                    repo_digests,
                },
            ) => {
                validate_resolved_image(image, id, repo_digests)?;
            }
            (
                Operation::Create {
                    name,
                    labels,
                    image_id,
                    container_port,
                    ..
                },
                Outcome::Created { object },
            ) => {
                object_matches(
                    object,
                    &Target {
                        container_port: *container_port,
                        object_id: object.object_id.clone(),
                        name: name.clone(),
                        labels: labels.clone(),
                    },
                )?;
                if object.state != S::Created || &object.image_id != image_id {
                    return Err(Invalid("created state required"));
                }
            }
            (Operation::Status { target }, Outcome::Status { object }) => {
                object_matches(object, target)?
            }
            (Operation::Start { target }, Outcome::Started { object }) => {
                object_matches(object, target)?;
                if object.state != S::Running || object.ports.len() != 1 {
                    return Err(Invalid("running state required"));
                }
            }
            (Operation::Stop { target, .. }, Outcome::Stopped { object }) => {
                object_matches(object, target)?;
                if !matches!(object.state, S::Created | S::Exited) {
                    return Err(Invalid("terminal state required"));
                }
            }
            (Operation::Remove { .. }, Outcome::Removed) => {}
            (Operation::Status { target }, Outcome::Absent { evidence }) => {
                if *evidence != exact_absence(e, target)? {
                    return Err(Invalid("invalid absence evidence"));
                }
            }
            (
                Operation::HealthExec { target, .. },
                Outcome::Health {
                    exec_id, object, ..
                },
            ) => {
                digest(exec_id)?;
                object_matches(object, target)?;
                if object.state != S::Running || object.ports.len() != 1 {
                    return Err(Invalid("health container not running"));
                }
            }
            _ => return Err(Invalid("lifecycle operation outcome mismatch")),
        }
        Ok(())
    }
}
fn validate_resolved_image(image: &str, id: &str, repo_digests: &[String]) -> Result<()> {
    image_id(id)?;
    if repo_digests.len() > 128 {
        return Err(Invalid("too many repo digests"));
    }
    for (i, d) in repo_digests.iter().enumerate() {
        crate::v3::validate_image(d)?;
        if !d.contains('@') || repo_digests[..i].contains(d) {
            return Err(Invalid("invalid repo digests"));
        }
    }
    if image.contains('@') && !repo_digests.iter().any(|digest| digest == image) {
        return Err(Invalid("image digest pin mismatch"));
    }
    Ok(())
}
/// Only validated responses may subsequently enter the durable journal layer.
/// Validation does not consume a generation; replay rejection belongs to storage.
pub fn parse_response(bytes: &[u8], expected: &Request) -> Result<Response> {
    let value = canonical::parse_canonical(bytes)?;
    let response: Response =
        serde_json::from_value(value).map_err(|_| Invalid("invalid lifecycle response schema"))?;
    response.validate(expected)?;
    let typed =
        serde_json::to_value(&response).map_err(|_| Invalid("invalid response encoding"))?;
    if canonical::encode(&typed)? != bytes {
        return Err(Invalid("lifecycle response fields changed during parsing"));
    }
    Ok(response)
}

#[cfg(test)]
mod tests;
