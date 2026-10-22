//! Immutable pre-create plan and image evidence; no execution authority itself.
use super::*;
use crate::lifecycle::wire::Entry;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreatePlan {
    pub name: String,
    pub labels: Vec<Entry>,
    pub environment: Vec<Entry>,
    pub container_port: u16,
}
impl CreatePlan {
    pub fn validate(&self, resolve: &Request) -> Result<()> {
        resolve.validate()?;
        if !matches!(resolve.operation, Operation::ResolveImage { .. })
            || resolve.binding.generation != 1
        {
            return Err(Invalid("reservation requires initial image resolution"));
        }
        self.request(
            resolve,
            &format!("sha256:{}", "0".repeat(64)),
            &resolve.binding.invocation,
        )?
        .validate()
    }
    fn request(&self, resolve: &Request, image_id: &str, invocation: &str) -> Result<Request> {
        let mut request = resolve.clone();
        request.operation = Operation::Create {
            name: self.name.clone(),
            image_id: image_id.into(),
            labels: self.labels.clone(),
            environment: self.environment.clone(),
            container_port: self.container_port,
        };
        request.binding.generation = 2;
        request.binding.invocation = invocation.into();
        request.binding.operation_key = request.operation_key()?;
        Ok(request)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ResolutionProof {
    pub request: Request,
    pub plan: CreatePlan,
    pub response: Response,
}
impl ResolutionProof {
    pub fn create(&self, invocation: &str) -> Result<Request> {
        self.plan.validate(&self.request)?;
        self.response.validate(&self.request)?;
        let Outcome::Image { image_id, .. } = &self.response.outcome else {
            return Err(Invalid("image resolution not proven"));
        };
        if invocation == self.request.binding.invocation {
            return Err(Invalid("create invocation must be fresh"));
        }
        let create = self.plan.request(&self.request, image_id, invocation)?;
        create.validate()?;
        Ok(create)
    }
    pub fn validate_create(&self, create: &Request) -> Result<()> {
        if self.create(&create.binding.invocation)? != *create {
            return Err(Invalid("create differs from reserved plan/image evidence"));
        }
        Ok(())
    }
}
