//! Canonical read-only wire. Parsing does not establish protected FD provenance.
use super::{
    capture_binding::operation_key,
    native_image::image_route,
    plan_record::{PlanInputs, PlanRecord},
    profile::{Profile, SlotIdentity},
    protected_context::ProtectedContext,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
type Result<T> = std::result::Result<T, &'static str>;
const REQUEST_LIMIT: usize = 64 * 1024;
const EXPECTED_LIMIT: usize = 16 * 1024;
const RESPONSE_LIMIT: usize = 24 * 1024 * 1024;
const RECORD_LIMIT: usize = 16 * 1024 * 1024;
fn hash(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
fn hex(s: &str, n: usize) -> bool {
    s.len() == n
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn encode<T: Serialize>(v: &T, limit: usize) -> Result<Vec<u8>> {
    let b = serde_json::to_vec(v).map_err(|_| "wire encoding")?;
    if b.len() > limit {
        return Err("wire limit");
    }
    Ok(b)
}
fn decode<T: serde::de::DeserializeOwned + Serialize>(b: &[u8], limit: usize) -> Result<T> {
    if b.is_empty() || b.len() > limit {
        return Err("wire limit");
    }
    let v = serde_json::from_slice(b).map_err(|_| "wire fields")?;
    if encode(&v, limit)? != b {
        return Err("noncanonical wire");
    }
    Ok(v)
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureRequest {
    pub schema: String,
    pub profile: Profile,
    pub invocation: String,
    pub image_id: String,
    pub inputs: PlanInputs,
    pub timeout_ms: u32,
}
impl CaptureRequest {
    pub fn validate(&self) -> Result<()> {
        if self.schema != "gate-runtime/podman-capture-v1"
            || !hex(&self.invocation, 32)
            || !(1..=30000).contains(&self.timeout_ms)
        {
            return Err("capture request rejected");
        }
        self.profile.validate()?;
        image_route(&self.image_id)?;
        self.inputs.validate()
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate()?;
        encode(self, REQUEST_LIMIT)
    }
    pub fn decode(b: &[u8]) -> Result<Self> {
        let r: Self = decode(b, REQUEST_LIMIT)?;
        r.validate()?;
        Ok(r)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureExpected {
    pub schema: String,
    pub invocation: String,
    pub request_sha256: String,
    pub host_sha256: String,
    pub installation_sha256: String,
    pub config_sha256: String,
    pub executable_sha256: String,
    pub config_path: String,
    pub operation_key: String,
}
impl CaptureExpected {
    /// Pure binding constructor; future host must derive arguments from held files.
    pub fn bind(
        request: &CaptureRequest,
        identity: &SlotIdentity,
        host_sha256: String,
        config_path: String,
    ) -> Result<Self> {
        identity.validate()?;
        if identity.profile != request.profile {
            return Err("capture profile mismatch");
        }
        let request_sha256 = hash(&request.encode()?);
        let key = operation_key(
            &request_sha256,
            &host_sha256,
            &identity.installation_sha256,
            &identity.config_sha256,
            &identity.executable_sha256,
        )?;
        let expected = Self {
            schema: "gate-runtime/podman-capture-expected-v1".into(),
            invocation: request.invocation.clone(),
            request_sha256,
            host_sha256,
            installation_sha256: identity.installation_sha256.clone(),
            config_sha256: identity.config_sha256.clone(),
            executable_sha256: identity.executable_sha256.clone(),
            config_path,
            operation_key: key,
        };
        expected.validate(request)?;
        Ok(expected)
    }
    pub fn validate(&self, request: &CaptureRequest) -> Result<()> {
        let path = &self.config_path;
        if self.schema != "gate-runtime/podman-capture-expected-v1"
            || self.invocation != request.invocation
            || self.request_sha256 != hash(&request.encode()?)
            || !path.starts_with('/')
            || path.len() > 4096
            || path.chars().any(char::is_control)
            || path
                .split('/')
                .skip(1)
                .any(|s| s.is_empty() || s == "." || s == "..")
        {
            return Err("capture expected mismatch");
        }
        if self.operation_key
            != operation_key(
                &self.request_sha256,
                &self.host_sha256,
                &self.installation_sha256,
                &self.config_sha256,
                &self.executable_sha256,
            )?
        {
            return Err("capture key mismatch");
        }
        Ok(())
    }
    pub fn encode(&self, r: &CaptureRequest) -> Result<Vec<u8>> {
        self.validate(r)?;
        encode(self, EXPECTED_LIMIT)
    }
    pub fn decode(b: &[u8], r: &CaptureRequest) -> Result<Self> {
        let e: Self = decode(b, EXPECTED_LIMIT)?;
        e.validate(r)?;
        Ok(e)
    }
    fn identity(&self, r: &CaptureRequest) -> SlotIdentity {
        SlotIdentity {
            profile: r.profile.clone(),
            installation_sha256: self.installation_sha256.clone(),
            config_sha256: self.config_sha256.clone(),
            executable_sha256: self.executable_sha256.clone(),
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    schema: String,
    invocation: String,
    operation_key: String,
    request_sha256: String,
    record_b64: String,
    record_sha256: String,
}
/// No stderr/exit/deadline checks here: those belong to the future host runner.
pub fn encode_response(
    record: &PlanRecord,
    request: &CaptureRequest,
    expected: &CaptureExpected,
    context: &ProtectedContext,
) -> Result<Vec<u8>> {
    expected.validate(request)?;
    record.match_capture_request(&request.image_id, &request.inputs)?;
    record.match_installation_snapshot(&expected.identity(request), context.snapshot()?.1)?;
    let bytes = record.encode()?;
    encode(
        &Response {
            schema: "gate-runtime/podman-capture-response-v1".into(),
            invocation: request.invocation.clone(),
            operation_key: expected.operation_key.clone(),
            request_sha256: expected.request_sha256.clone(),
            record_b64: STANDARD.encode(&bytes),
            record_sha256: hash(&bytes),
        },
        RESPONSE_LIMIT,
    )
}
pub fn decode_response(
    bytes: &[u8],
    request: &CaptureRequest,
    expected: &CaptureExpected,
    context: &ProtectedContext,
) -> Result<PlanRecord> {
    expected.validate(request)?;
    let response: Response = decode(bytes, RESPONSE_LIMIT)?;
    if response.schema != "gate-runtime/podman-capture-response-v1"
        || response.invocation != request.invocation
        || response.operation_key != expected.operation_key
        || response.request_sha256 != expected.request_sha256
        || !hex(&response.record_sha256, 64)
        || response.record_b64.len() > RECORD_LIMIT.div_ceil(3) * 4
    {
        return Err("capture response binding rejected");
    }
    let raw = STANDARD
        .decode(&response.record_b64)
        .map_err(|_| "record base64")?;
    if raw.len() > RECORD_LIMIT
        || STANDARD.encode(&raw) != response.record_b64
        || hash(&raw) != response.record_sha256
    {
        return Err("record bytes rejected");
    }
    let record = PlanRecord::decode(&raw, context)?;
    record.match_capture_request(&request.image_id, &request.inputs)?;
    record.match_installation_snapshot(&expected.identity(request), context.snapshot()?.1)?;
    Ok(record)
}
