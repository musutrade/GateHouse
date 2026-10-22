//! Explicit trusted Core operator policy. Not human authentication or auto recovery.
use super::*;
use sha2::{Digest, Sha256};

pub struct OperatorRecovery {
    slot: ServiceSlot,
    journal: Journal,
    authorization: String,
}
impl OperatorRecovery {
    /// Caller must originate in an explicit operator command, never normal start.
    /// Exact fence and lease invocation are required; authority is never inferred.
    pub fn open_mapped(
        base: &Path,
        project: &str,
        resource: &str,
        fence: u64,
        lease_invocation: &str,
        authorization: &str,
    ) -> Result<Self> {
        if authorization.len() != 32
            || !authorization
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Invalid(
                "operator authorization must be fresh 128-bit identity",
            ));
        }
        let _parent = Journal::directory(base)?;
        let root = ServiceSlot::mapped_root(base, project, resource)?;
        let directory = Journal::directory(&root)?;
        let bytes =
            Journal::read(&directory, "service.json")?.ok_or(Invalid("missing service slot"))?;
        let record: Record = serde_json::from_value(canonical::parse_canonical(&bytes)?)
            .map_err(|_| Invalid("invalid operator slot record"))?;
        record.validate()?;
        if record.released {
            return Err(Invalid("released slot does not require operator recovery"));
        }
        if record.fence != fence
            || record.resolve.binding.lease_invocation != lease_invocation
            || record.resolve.binding.project_id != project
            || record.resolve.binding.resource_id != resource
        {
            return Err(Invalid(
                "operator authorization does not match current resource fence",
            ));
        }
        let slot = ServiceSlot {
            root,
            directory,
            record,
            bytes,
            reserved: true,
        };
        slot.check()?;
        let journal = Journal::open_reserved(
            &slot.root.join(slot.record.journal_name()),
            &slot.record.resolve,
            &slot.record.plan,
        )?;
        // An ambiguous Create without an exact ID cannot be discovered/adopted.
        journal.target()?;
        let name = format!("operator-{authorization}.json");
        if Journal::read(&slot.directory, &name)?.is_some() {
            return Err(Invalid("operator authorization replay"));
        }
        let intent = canonical::encode(&serde_json::json!({
            "schema":"gatehouse-lifecycle-operator/v1", "action":"cleanup-known-id",
            "authorization":authorization, "fence":fence, "lease_invocation":lease_invocation,
            "slot_sha256":format!("{:x}",Sha256::digest(&slot.bytes)),
            "target":journal.target()?
        }))?;
        Journal::atomic(&slot.directory, &name, &intent)?;
        Ok(Self {
            slot,
            journal,
            authorization: authorization.into(),
        })
    }
    pub fn resolve_request(&self) -> &Request {
        self.slot.resolve_request()
    }
    pub fn target(&self) -> Result<Target> {
        self.journal.target()
    }
    pub fn phase(&self) -> Phase {
        self.journal.phase()
    }
    pub fn check(&self) -> Result<()> {
        self.slot.check()
    }
    pub fn begin(&mut self, operation: Operation, invocation: &str) -> Result<Dispatch<'_>> {
        self.slot.check()?;
        if !matches!(
            operation,
            Operation::Status { .. } | Operation::Stop { .. } | Operation::Remove { .. }
        ) {
            return Err(Invalid(
                "operator recovery forbids start/create/health/resolution",
            ));
        }
        let mut request = self.journal.record.create.clone();
        request.operation = operation;
        request.binding.generation = self
            .journal
            .revision()
            .checked_add(1)
            .ok_or(Invalid("operator generation exhausted"))?;
        request.binding.invocation = invocation.into();
        request.binding.operation_key = request.operation_key()?;
        self.journal.begin(&request, Purpose::OperatorCleanup)
    }
    pub fn finish(mut self) -> Result<()> {
        self.slot.check()?;
        self.journal.finish_release()?;
        let evidence = canonical::encode(&serde_json::json!({
            "schema":"gatehouse-lifecycle-operator-proof/v1", "authorization":self.authorization,
            "fence":self.slot.fence(), "slot_sha256":format!("{:x}",Sha256::digest(&self.slot.bytes)),
            "journal_sha256":format!("{:x}",Sha256::digest(&self.journal.bytes)),
            "target":self.journal.target()?
        }))?;
        Journal::atomic(
            &self.slot.directory,
            &format!("operator-{}.proof.json", self.authorization),
            &evidence,
        )?;
        self.slot.finish(self.journal)
    }
}
