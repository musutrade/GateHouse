//! Canonical same-context evidence codec. No storage or dispatch authority.
use super::{
    native_image::{decode_image, image_route},
    ownership::FixedPort,
    profile::{Profile, SlotIdentity},
    protected_context::ProtectedContext,
    readonly::CapturedImage,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
type Result<T> = std::result::Result<T, &'static str>;
const SCHEMA: &str = "gate-runtime/podman-plan-evidence-v1";
const LIMIT: usize = 16 * 1024 * 1024;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanInputs {
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub port: Option<PlanPort>,
}
impl PlanInputs {
    /// Checks only caller-known fields. Empty operator labels are legal until
    /// image inheritance is merged; final plan validation still applies.
    pub fn validate(&self) -> Result<()> {
        super::ownership::validate_plan_fields(
            &self.name,
            &self.labels,
            self.port
                .as_ref()
                .map(|p| FixedPort {
                    container: p.container,
                    host: p.host,
                })
                .as_ref(),
            false,
        )
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanPort {
    pub container: u16,
    pub host: u16,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    schema: String,
    profile: Profile,
    asserted_installation_sha256: String,
    asserted_executable_sha256: String,
    config_json: String,
    config_sha256: String,
    operator_epoch: String,
    image_id: String,
    image_json: String,
    image_sha256: String,
    inputs: PlanInputs,
    plan_json: String,
    plan_sha256: String,
}
/// Validated saved evidence only. No public raw constructor or serde implementation.
pub struct PlanRecord {
    wire: Wire,
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
impl PlanRecord {
    pub fn capture(
        context: &ProtectedContext,
        captured: &CapturedImage,
        identity: &SlotIdentity,
        inputs: PlanInputs,
    ) -> Result<Self> {
        inputs.validate()?;
        captured.match_context(context)?;
        identity.validate()?;
        if identity.config_sha256 != hash(captured.config_bytes()) {
            return Err("slot config mismatch");
        }
        let plan = derive(captured.facts(), &inputs)?;
        let wire = Wire {
            schema: SCHEMA.into(),
            profile: identity.profile.clone(),
            asserted_installation_sha256: identity.installation_sha256.clone(),
            asserted_executable_sha256: identity.executable_sha256.clone(),
            config_json: std::str::from_utf8(captured.config_bytes())
                .map_err(|_| "config UTF8")?
                .into(),
            config_sha256: identity.config_sha256.clone(),
            operator_epoch: captured.operator_epoch().into(),
            image_id: captured.facts().id().into(),
            image_json: std::str::from_utf8(captured.image_bytes())
                .map_err(|_| "image UTF8")?
                .into(),
            image_sha256: hash(captured.image_bytes()),
            inputs,
            plan_sha256: hash(plan.as_bytes()),
            plan_json: plan,
        };
        let record = Self { wire };
        record.validate(context)?;
        Ok(record)
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        let bytes = serde_json::to_vec(&self.wire).map_err(|_| "record encoding")?;
        if bytes.len() > LIMIT {
            return Err("record too large");
        }
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8], context: &ProtectedContext) -> Result<Self> {
        if bytes.is_empty() || bytes.len() > LIMIT {
            return Err("record size");
        }
        let wire = serde_json::from_slice(bytes).map_err(|_| "record fields")?;
        let record = Self { wire };
        if record.encode()? != bytes {
            return Err("noncanonical record");
        }
        record.validate(context)?;
        Ok(record)
    }
    pub(super) fn match_installation_snapshot(
        &self,
        identity: &SlotIdentity,
        config: &[u8],
    ) -> Result<()> {
        identity.validate()?;
        if self.wire.profile != identity.profile
            || self.wire.asserted_installation_sha256 != identity.installation_sha256
            || self.wire.asserted_executable_sha256 != identity.executable_sha256
            || self.wire.config_sha256 != identity.config_sha256
            || self.wire.config_json.as_bytes() != config
        {
            return Err("record installation snapshot mismatch");
        }
        Ok(())
    }
    pub fn plan_bytes(&self) -> &[u8] {
        self.wire.plan_json.as_bytes()
    }
    pub fn installation_verified(&self) -> bool {
        false
    }
    fn validate(&self, context: &ProtectedContext) -> Result<()> {
        let w = &self.wire;
        let (expected, bytes) = context.snapshot()?;
        if w.schema != SCHEMA
            || w.config_json.len() > 1024 * 1024
            || w.image_json.len() > 1024 * 1024
            || bytes != w.config_json.as_bytes()
            || expected.operator_epoch != w.operator_epoch
        {
            return Err("record context mismatch");
        }
        SlotIdentity {
            profile: w.profile.clone(),
            installation_sha256: w.asserted_installation_sha256.clone(),
            executable_sha256: w.asserted_executable_sha256.clone(),
            config_sha256: w.config_sha256.clone(),
        }
        .validate()?;
        if hash(bytes) != w.config_sha256
            || hash(w.image_json.as_bytes()) != w.image_sha256
            || hash(w.plan_json.as_bytes()) != w.plan_sha256
        {
            return Err("record digest mismatch");
        }
        let facts = decode_image(
            &w.image_id,
            "GET",
            &image_route(&w.image_id)?,
            200,
            w.image_json.as_bytes(),
        )?;
        if derive(&facts, &w.inputs)? != w.plan_json {
            return Err("record plan mismatch");
        }
        context.revalidate()
    }
}
fn derive(facts: &super::native_image::ImageFacts, inputs: &PlanInputs) -> Result<String> {
    inputs.validate()?;
    let plan = facts.create_plan(
        inputs.name.clone(),
        inputs.labels.clone(),
        inputs.port.as_ref().map(|p| FixedPort {
            container: p.container,
            host: p.host,
        }),
    )?;
    String::from_utf8(plan.body()?).map_err(|_| "plan UTF8")
}
