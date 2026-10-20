//! Pure transitions for a single removal intent. No filesystem, dispatch,
//! namespace discovery or lease migration. A durable coordinator must commit
//! the returned state under its generation lock BEFORE the corresponding effect.
use crate::{require_absence, Invalid, Label, Operation, Request, Response};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", try_from = "String")]
pub enum Phase {
    Prepared,
    Unknown,
    Completed,
}

impl TryFrom<String> for Phase {
    type Error = Invalid;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "prepared" => Ok(Self::Prepared),
            "unknown" => Ok(Self::Unknown),
            "completed" => Ok(Self::Completed),
            _ => Err(Invalid("unsupported intent phase")),
        }
    }
}

/// Fields are private so ordinary callers cannot skip ownership validation.
/// Restore accepts only an operator-protected state record with independent
/// expected request pins; it does not authenticate disk bytes or prevent rollback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Intent {
    schema: String,
    request: Request,
    phase: Phase,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: String,
    request: Request,
    phase: Phase,
}

const SCHEMA: &str = "gate-runtime-intent/v1";

fn removal(request: &Request) -> Result<(), Invalid> {
    request.validate()?;
    if !matches!(request.operation, Operation::Remove { .. }) {
        return Err(Invalid("intent requires exact-ID removal"));
    }
    Ok(())
}

fn inspection_request(removal: &Request) -> Request {
    let Operation::Remove { object_id } = &removal.operation else {
        unreachable!("private helper called only after removal validation");
    };
    Request {
        binding: removal.binding.clone(),
        operation: Operation::Inspect {
            object_id: object_id.clone(),
        },
    }
}

impl Intent {
    /// The coordinator must independently authenticate the inspection producer
    /// and pin required labels from its lease, never from the adapter response.
    pub fn prepare(
        request: &Request,
        inspection: &Response,
        required_labels: &[Label],
    ) -> Result<Self, Invalid> {
        removal(request)?;
        inspection.require_ownership(&inspection_request(request), required_labels)?;
        Ok(Self {
            schema: SCHEMA.into(),
            request: request.clone(),
            phase: Phase::Prepared,
        })
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn request(&self) -> &Request {
        &self.request
    }

    /// Commit this Unknown state before dispatch. A crash between commit and
    /// dispatch is conservative: recovery must inspect, never blindly retry.
    pub fn begin_dispatch(&self, expected: &Request) -> Result<Self, Invalid> {
        self.request.require_same_intent(expected)?;
        if self.phase != Phase::Prepared {
            return Err(Invalid("intent is not prepared for dispatch"));
        }
        let mut next = self.clone();
        next.phase = Phase::Unknown;
        Ok(next)
    }

    /// None represents transport loss/cancellation/error. Every valid mutation
    /// response, including completed/rejected-before-effect, leaves Unknown.
    /// Invalid responses return an error without modifying the original state.
    pub fn observe_dispatch(
        &self,
        expected: &Request,
        response: Option<&Response>,
    ) -> Result<Self, Invalid> {
        self.request.require_same_intent(expected)?;
        if self.phase != Phase::Unknown {
            return Err(Invalid("intent has no unresolved dispatch"));
        }
        if let Some(response) = response {
            response.validate(&self.request)?;
        }
        Ok(self.clone())
    }

    /// Only independently authenticated exact-ID absence can finish Unknown.
    /// This pure operation does not remove a marker or grant cleanup authority.
    pub fn reconcile(&self, expected: &Request, inspection: &Response) -> Result<Self, Invalid> {
        self.request.require_same_intent(expected)?;
        if self.phase != Phase::Unknown {
            return Err(Invalid("intent is not awaiting reconciliation"));
        }
        require_absence(&self.request, inspection)?;
        let mut next = self.clone();
        next.phase = Phase::Completed;
        Ok(next)
    }

    /// Read only trusted persisted state. Legacy lease v2, missing pins, duplicate
    /// fields and unknown schema/phase reject; no ambient values are consulted.
    pub fn restore(bytes: &[u8], expected: &Request) -> Result<Self, Invalid> {
        let record: Record = crate::parse(bytes)?;
        if record.schema != SCHEMA {
            return Err(Invalid("unsupported intent schema"));
        }
        removal(&record.request)?;
        record.request.require_same_intent(expected)?;
        Ok(Self {
            schema: record.schema,
            request: record.request,
            phase: record.phase,
        })
    }
}
