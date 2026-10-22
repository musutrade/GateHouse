//! A protected service resource fence, distinct from legacy lease v2.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: String,
    fence: u64,
    released: bool,
    resolve: Request,
    plan: CreatePlan,
    healthcheck: Vec<String>,
}
impl Record {
    fn validate(&self) -> Result<()> {
        if self.schema != "gatehouse-lifecycle-service-slot/v1" || self.fence == 0 {
            return Err(Invalid("invalid service slot schema/fence"));
        }
        self.plan.validate(&self.resolve)?;
        if !self.healthcheck.is_empty() {
            let mut health = self.resolve.clone();
            health.operation = Operation::HealthExec {
                target: Target {
                    object_id: "0".repeat(64),
                    name: self.plan.name.clone(),
                    labels: self.plan.labels.clone(),
                    container_port: self.plan.container_port,
                },
                argv: self.healthcheck.clone(),
                timeout_ms: 30000,
            };
            health.binding.operation_key = health.operation_key()?;
            health.validate()?;
        }
        Ok(())
    }
    fn bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        canonical::encode(&serde_json::to_value(self).map_err(|_| Invalid("slot encoding failed"))?)
    }
    fn journal_name(&self) -> String {
        format!("journal-{}", self.resolve.binding.lease_invocation)
    }
}

/// Holds the resource lock and immutable start specification. Drop never releases
/// the durable fence. Only the bound journal's verified absence can release it.
pub struct ServiceSlot {
    root: PathBuf,
    directory: LockedDirectory,
    record: Record,
    bytes: Vec<u8>,
    reserved: bool,
}
impl ServiceSlot {
    pub fn acquire(
        root: &Path,
        resolve: Request,
        plan: CreatePlan,
        healthcheck: Vec<String>,
    ) -> Result<Self> {
        Self::acquire_with(root, resolve, plan, healthcheck, |_| Ok(()))
    }
    fn acquire_with(
        root: &Path,
        resolve: Request,
        plan: CreatePlan,
        healthcheck: Vec<String>,
        mut stage: impl FnMut(u8) -> Result<()>,
    ) -> Result<Self> {
        let directory = Journal::directory(root)?;
        let previous = Journal::read(&directory, "service.json")?;
        if previous.is_none() {
            for member in ["active", "lease.json", "reservation.json", "tombstone.json"] {
                if Journal::read(&directory, member)?.is_some() {
                    return Err(Invalid(
                        "slot cannot adopt an existing standalone or legacy lease",
                    ));
                }
            }
        }
        let mut record = Record {
            schema: "gatehouse-lifecycle-service-slot/v1".into(),
            fence: 1,
            released: false,
            resolve,
            plan,
            healthcheck,
        };
        record.validate()?;
        if let Some(bytes) = previous {
            record.fence = Self::released_fence(root, &directory, &record, &bytes)?;
        }
        let name = CString::new(record.journal_name()).unwrap();
        if Journal::at(
            &directory,
            name.to_str().unwrap(),
            libc::O_RDONLY | libc::O_DIRECTORY,
        )?
        .is_some()
        {
            return Err(Invalid("service invocation reused"));
        }
        let bytes = record.bytes()?;
        // Persist the fence before creating any journal or yielding a token.
        Journal::atomic_with(&directory, "service.json", &bytes, &mut stage)?;
        stage(3)?;
        // SAFETY: fixed prefix and validated hexadecimal invocation; held directory.
        if unsafe { libc::mkdirat(directory.as_raw_fd(), name.as_ptr(), 0o700) } != 0 {
            return Err(Invalid("cannot provision slot journal; fence retained"));
        }
        io(directory.sync_all())?;
        let slot = Self {
            root: root.into(),
            directory,
            record,
            bytes,
            reserved: false,
        };
        slot.check()?;
        Ok(slot)
    }
    fn released_fence(root: &Path, directory: &File, record: &Record, bytes: &[u8]) -> Result<u64> {
        let old: Record = serde_json::from_value(canonical::parse_canonical(bytes)?)
            .map_err(|_| Invalid("invalid service slot record"))?;
        old.validate()?;
        if !old.released
            || old.resolve.binding.project_id != record.resolve.binding.project_id
            || old.resolve.binding.resource_id != record.resolve.binding.resource_id
        {
            return Err(Invalid(
                "service slot retained or belongs to another resource",
            ));
        }
        // Never trust a released boolean alone. Revalidate the exact reserved
        // journal, immutable proof and matching durable tombstone under lock.
        let mut journal =
            Journal::open_reserved(&root.join(old.journal_name()), &old.resolve, &old.plan)?;
        journal.finish_release()?;
        let archive = format!("service-history-{}.json", old.fence);
        match Journal::read(directory, &archive)? {
            Some(prior) if prior != bytes => return Err(Invalid("slot history changed")),
            Some(_) => {}
            None => Journal::atomic(directory, &archive, bytes)?,
        }
        let fence = old
            .fence
            .checked_add(1)
            .ok_or(Invalid("slot fence exhausted"))?;
        Ok(fence)
    }
    pub fn check(&self) -> Result<()> {
        let now = io(fs::symlink_metadata(&self.root))?;
        let held = io(self.directory.metadata())?;
        private(&now, true)?;
        if now.dev() != held.dev()
            || now.ino() != held.ino()
            || Journal::read(&self.directory, "service.json")?.as_deref()
                != Some(self.bytes.as_slice())
        {
            return Err(Invalid("service slot changed outside lock"));
        }
        Ok(())
    }
    pub fn resolve_request(&self) -> &Request {
        &self.record.resolve
    }
    pub fn plan(&self) -> &CreatePlan {
        &self.record.plan
    }
    pub fn healthcheck(&self) -> &[String] {
        &self.record.healthcheck
    }
    pub fn fence(&self) -> u64 {
        self.record.fence
    }
    pub fn reserve(&mut self) -> Result<Reservation> {
        self.check()?;
        if self.reserved || self.record.released {
            return Err(Invalid("slot reservation already consumed"));
        }
        self.reserved = true;
        Reservation::create(
            &self.root.join(self.record.journal_name()),
            &self.record.resolve,
            self.record.plan.clone(),
        )
    }
    pub fn finish(&mut self, mut journal: Journal) -> Result<()> {
        self.check()?;
        let proof = journal
            .record
            .resolution
            .as_ref()
            .ok_or(Invalid("slot requires resolution proof"))?;
        if journal.root != self.root.join(self.record.journal_name())
            || proof.request != self.record.resolve
            || proof.plan != self.record.plan
        {
            return Err(Invalid("journal does not belong to service slot"));
        }
        journal.finish_release()?;
        let mut released = self.record.clone();
        released.released = true;
        let bytes = released.bytes()?;
        Journal::atomic(&self.directory, "service.json", &bytes)?;
        self.record = released;
        self.bytes = bytes;
        Ok(())
    }
}

#[cfg(test)]
mod tests;

mod mapping;
mod recovery;
pub use recovery::OperatorRecovery;
