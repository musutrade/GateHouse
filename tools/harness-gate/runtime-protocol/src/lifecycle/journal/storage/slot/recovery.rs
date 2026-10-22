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
    pub fn finish(self) -> Result<()> {
        self.finish_with(|| Ok(()))
    }
    fn finish_with(mut self, after_proof: impl FnOnce() -> Result<()>) -> Result<()> {
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
        after_proof()?;
        self.slot.finish(self.journal)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle::{journal::storage::tests::{absent, object, reply}, wire::ContainerState};

    #[test]
    fn persisted_operator_proof_never_substitutes_for_slot_release() {
        let (d, resolve, plan) = super::super::tests::fixture();
        let mut slot = ServiceSlot::acquire_mapped(d.path(), resolve.clone(), plan.clone(), vec![]).unwrap();
        let root = slot.root.clone();
        let mut reservation = slot.reserve().unwrap();
        let image = reply(&resolve, Outcome::Image { image_id: format!("sha256:{}", "e".repeat(64)), repo_digests: vec![] });
        reservation.dispatch().unwrap().observe(Some(&image)).unwrap();
        let mut journal = reservation.into_journal(&"3".repeat(32)).unwrap();
        let token = journal.dispatch().unwrap();
        let create = token.request().clone();
        let created = reply(&create, Outcome::Created { object: object(&create, ContainerState::Created) });
        token.observe(Some(&created)).unwrap();
        drop(journal);
        drop(slot);
        let open = |auth: &str| OperatorRecovery::open_mapped(d.path(), &resolve.binding.project_id,
            &resolve.binding.resource_id, 1, &resolve.binding.lease_invocation, auth);
        let first = "9".repeat(32);
        let mut recovery = open(&first).unwrap();
        let target = recovery.target().unwrap();
        let token = recovery.begin(Operation::Status { target }, &"4".repeat(32)).unwrap();
        let status = token.request().clone();
        token.observe(Some(&absent(&status))).unwrap();
        let before = fs::read(root.join("service.json")).unwrap();
        let failure = recovery.finish_with(|| {
            assert!(root.join(format!("operator-{first}.proof.json")).exists());
            Err(Invalid("injected failure after proof before slot release"))
        });
        assert!(failure.unwrap_err().to_string().contains("injected failure"));
        assert_eq!(fs::read(root.join("service.json")).unwrap(), before);
        let journal_root = root.join(format!("journal-{}", resolve.binding.lease_invocation));
        assert!(journal_root.join("tombstone.json").exists());
        assert!(!journal_root.join("active").exists());
        assert!(ServiceSlot::acquire_mapped(d.path(), resolve.clone(), plan, vec![]).is_err());
        assert!(open(&first).err().unwrap().to_string().contains("replay"));
        let second = "8".repeat(32);
        let recovery = open(&second).unwrap();
        assert_eq!(recovery.phase(), Phase::VerifiedAbsent);
        // No dispatch: durable absence and the exact tombstone finish the slot.
        recovery.finish().unwrap();
        let record: Record = serde_json::from_slice(&fs::read(root.join("service.json")).unwrap()).unwrap();
        assert!(record.released);
        assert!(root.join(format!("operator-{second}.proof.json")).exists());
        assert!(open(&"7".repeat(32)).err().unwrap().to_string().contains("released slot"));
    }
}
