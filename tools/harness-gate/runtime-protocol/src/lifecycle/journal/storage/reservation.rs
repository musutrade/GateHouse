//! Durable pre-create reservation. No implicit resume, adoption or engine call.
use super::*;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Phase {
    Pending,
    Resolved,
    Retained,
    Abandoned,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReservationRecord {
    schema: String,
    request: Request,
    plan: CreatePlan,
    phase: Phase,
    response: Option<Response>,
    abandonment_reason: Option<String>,
}
impl ReservationRecord {
    fn validate(&self) -> Result<()> {
        if self.schema != "gatehouse-lifecycle-reservation/v1" {
            return Err(Invalid("invalid reservation schema"));
        }
        self.plan.validate(&self.request)?;
        if let Some(response) = &self.response {
            response.validate(&self.request)?;
        }
        let image = matches!(
            self.response.as_ref().map(|r| &r.outcome),
            Some(Outcome::Image { .. })
        );
        let unknown = matches!(
            self.response.as_ref().map(|r| &r.outcome),
            None | Some(Outcome::Unknown { .. } | Outcome::RejectedBeforeEffect { .. })
        );
        let valid = match self.phase {
            Phase::Pending => self.response.is_none(),
            Phase::Resolved => image,
            Phase::Retained => unknown,
            Phase::Abandoned => image || unknown,
        };
        if !valid
            || (self.phase == Phase::Abandoned)
                != (self.abandonment_reason.as_deref() == Some("operator-abandoned-before-create"))
            || (self.phase != Phase::Abandoned && self.abandonment_reason.is_some())
        {
            return Err(Invalid("reservation evidence/phase mismatch"));
        }
        Ok(())
    }
    fn bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        canonical::encode(
            &serde_json::to_value(self).map_err(|_| Invalid("reservation encoding failed"))?,
        )
    }
}
pub struct Reservation {
    root: PathBuf,
    directory: File,
    record: ReservationRecord,
    bytes: Vec<u8>,
    ready_resolve: bool,
    ready_create: bool,
    poisoned: bool,
}
/// Non-Clone, borrowed, one-use authorization for only the reserved ResolveImage.
#[must_use]
pub struct ResolutionDispatch<'a> {
    reservation: &'a mut Reservation,
}
impl ResolutionDispatch<'_> {
    pub fn request(&self) -> &Request {
        &self.reservation.record.request
    }
    /// Call only after process/installation/cancellation checks, like Dispatch.
    pub fn observe(self, bytes: Option<&[u8]>) -> Result<()> {
        let mut next = self.reservation.record.clone();
        next.response = match bytes {
            Some(bytes) => match wire::parse_response(bytes, &next.request) {
                Ok(response) => Some(response),
                Err(error) => {
                    self.reservation.poisoned = true;
                    return Err(error);
                }
            },
            None => None,
        };
        next.phase = if matches!(
            next.response.as_ref().map(|r| &r.outcome),
            Some(Outcome::Image { .. })
        ) {
            Phase::Resolved
        } else {
            Phase::Retained
        };
        self.reservation.commit(next)?;
        self.reservation.ready_create = self.reservation.record.phase == Phase::Resolved;
        Ok(())
    }
}
impl Reservation {
    pub fn create(root: &Path, request: &Request, plan: CreatePlan) -> Result<Self> {
        let directory = Journal::directory(root)?;
        let record = ReservationRecord {
            schema: "gatehouse-lifecycle-reservation/v1".into(),
            request: request.clone(),
            plan,
            phase: Phase::Pending,
            response: None,
            abandonment_reason: None,
        };
        let bytes = record.bytes()?;
        for name in [
            "active",
            "reservation.json",
            "reservation-tombstone.json",
            "lease.json",
            "tombstone.json",
        ] {
            if Journal::read(&directory, name)?.is_some() {
                return Err(Invalid("resource already reserved/recorded"));
            }
        }
        let mut marker = Journal::at(
            &directory,
            "active",
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
        )?
        .ok_or(Invalid("reservation marker missing"))?;
        io(marker.write_all(request.binding.operation_key.as_bytes()))?;
        io(marker.sync_all())?;
        io(directory.sync_all())?;
        Journal::atomic(&directory, "reservation.json", &bytes)?;
        Ok(Self {
            root: root.into(),
            directory,
            record,
            bytes,
            ready_resolve: true,
            ready_create: false,
            poisoned: false,
        })
    }
    pub fn open(root: &Path, request: &Request, plan: &CreatePlan) -> Result<Self> {
        request.validate()?;
        plan.validate(request)?;
        let directory = Journal::directory(root)?;
        let bytes = Journal::read(&directory, "reservation.json")?
            .ok_or(Invalid("reservation missing; no adoption"))?;
        let record: ReservationRecord = serde_json::from_value(canonical::parse_canonical(&bytes)?)
            .map_err(|_| Invalid("reservation schema rejected"))?;
        if record.bytes()? != bytes || record.request != *request || record.plan != *plan {
            return Err(Invalid("reservation binding changed"));
        }
        let mut value = Self {
            root: root.into(),
            directory,
            record,
            bytes,
            ready_resolve: false,
            ready_create: false,
            poisoned: false,
        };
        value.check()?;
        if value.record.phase == Phase::Pending {
            let mut next = value.record.clone();
            next.phase = Phase::Retained;
            value.commit(next)?;
        }
        Ok(value)
    }
    fn check(&self) -> Result<()> {
        if self.poisoned {
            return Err(Invalid("reservation poisoned; reopen required"));
        }
        let now = io(fs::symlink_metadata(&self.root))?;
        let held = io(self.directory.metadata())?;
        private(&now, true)?;
        if now.dev() != held.dev() || now.ino() != held.ino() {
            return Err(Invalid("reservation directory replaced"));
        }
        if Journal::read(&self.directory, "reservation.json")?.as_deref()
            != Some(self.bytes.as_slice())
            || Journal::read(&self.directory, "lease.json")?.is_some()
            || Journal::read(&self.directory, "tombstone.json")?.is_some()
        {
            return Err(Invalid(
                "reservation changed/transferred; use journal recovery",
            ));
        }
        let marker = Journal::read(&self.directory, "active")?;
        let tombstone = Journal::read(&self.directory, "reservation-tombstone.json")?;
        if let Some(t) = &tombstone {
            if self.record.phase != Phase::Abandoned || t != &self.bytes {
                return Err(Invalid("reservation tombstone mismatch"));
            }
        }
        if let Some(marker) = marker {
            if marker != self.record.request.binding.operation_key.as_bytes() {
                return Err(Invalid("reservation marker changed"));
            }
        } else if self.record.phase != Phase::Abandoned || tombstone.as_ref() != Some(&self.bytes) {
            return Err(Invalid("reservation marker missing"));
        }
        Ok(())
    }
    fn commit(&mut self, next: ReservationRecord) -> Result<()> {
        self.check()?;
        self.poisoned = true;
        self.ready_resolve = false;
        self.ready_create = false;
        let bytes = next.bytes()?;
        Journal::atomic(&self.directory, "reservation.json", &bytes)?;
        self.bytes = bytes;
        self.record = next;
        self.poisoned = false;
        Ok(())
    }
    pub fn dispatch(&mut self) -> Result<ResolutionDispatch<'_>> {
        self.check()?;
        if !self.ready_resolve || self.record.phase != Phase::Pending {
            return Err(Invalid("no fresh resolution authority"));
        }
        self.ready_resolve = false;
        Ok(ResolutionDispatch { reservation: self })
    }
    pub fn retain_unknown(&mut self) -> Result<()> {
        self.check()?;
        if self.record.phase != Phase::Pending {
            return Err(Invalid("no pending resolution"));
        }
        let mut next = self.record.clone();
        next.phase = Phase::Retained;
        self.commit(next)
    }
    /// Explicit trusted operator policy, not human authentication. Normal recovery
    /// must not call this. No lease file may exist and image evidence is retained.
    pub fn authorize_operator_resume_create(&mut self) -> Result<()> {
        self.check()?;
        if self.record.phase != Phase::Resolved {
            return Err(Invalid("no resolved reservation"));
        }
        self.ready_create = true;
        Ok(())
    }
    pub fn into_journal(self, invocation: &str) -> Result<Journal> {
        self.transfer_with(invocation, |_| Ok(()))
    }
    fn transfer_with(
        self,
        invocation: &str,
        stage: impl FnMut(u8) -> Result<()>,
    ) -> Result<Journal> {
        self.check()?;
        if !self.ready_create || self.record.phase != Phase::Resolved {
            return Err(Invalid("create transfer not authorized"));
        }
        let proof = ResolutionProof {
            request: self.record.request.clone(),
            plan: self.record.plan.clone(),
            response: self
                .record
                .response
                .clone()
                .ok_or(Invalid("resolution proof missing"))?,
        };
        let create = proof.create(invocation)?;
        let record = Record::new_resolved(&create, Some(proof))?;
        let bytes = Journal::encode(&record)?;
        // Keep the original reservation marker. A durable lease is the transfer
        // commit: any reopen then uses journal recovery and cannot resolve again.
        Journal::atomic_with(&self.directory, "lease.json", &bytes, stage)?;
        Ok(Journal {
            root: self.root,
            directory: self.directory,
            record,
            bytes,
            poisoned: false,
            ready: true,
            fresh_status: false,
        })
    }
    /// Explicit abandonment is allowed only before any lease/creation intent.
    /// Tombstone is persisted before releasing the active marker; records remain.
    pub fn abandon(&mut self) -> Result<()> {
        self.check()?;
        if self.record.phase == Phase::Pending {
            return Err(Invalid("retain incomplete resolution before abandonment"));
        }
        if self.record.phase != Phase::Abandoned {
            let mut next = self.record.clone();
            next.phase = Phase::Abandoned;
            next.abandonment_reason = Some("operator-abandoned-before-create".into());
            self.commit(next)?;
        }
        Journal::atomic(&self.directory, "reservation-tombstone.json", &self.bytes)?;
        let name = CString::new("active").unwrap();
        if unsafe { libc::unlinkat(self.directory.as_raw_fd(), name.as_ptr(), 0) } != 0
            && std::io::Error::last_os_error().kind() != std::io::ErrorKind::NotFound
        {
            return Err(Invalid("reservation marker release failed"));
        }
        io(self.directory.sync_all())?;
        self.ready_create = false;
        self.ready_resolve = false;
        Ok(())
    }
}
impl Journal {
    /// Reopen a transferred lease only with the exact trusted reservation inputs.
    /// Even pending Create is consumed as unknown; no automatic second dispatch.
    pub fn open_reserved(root: &Path, request: &Request, plan: &CreatePlan) -> Result<Self> {
        plan.validate(request)?;
        let directory = Self::directory(root)?;
        let bytes =
            Self::read(&directory, "lease.json")?.ok_or(Invalid("transferred journal missing"))?;
        let record: Record = serde_json::from_value(canonical::parse_canonical(&bytes)?)
            .map_err(|_| Invalid("journal schema rejected"))?;
        if Self::encode(&record)? != bytes {
            return Err(Invalid("journal encoding mismatch"));
        }
        let proof = record
            .resolution
            .as_ref()
            .ok_or(Invalid("journal has no resolution proof"))?;
        if proof.request != *request || proof.plan != *plan {
            return Err(Invalid("reservation/journal identity mismatch"));
        }
        Self::open_held(root, &record.create, directory)
    }
}

#[cfg(test)]
mod tests;
