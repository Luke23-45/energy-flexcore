// P3.28 — In-memory fakes for the Phase 3 record stores and the
//         failure-injection outbox (Test-backend composition support).
//
// The flexcore-ports fakes cover the Stage 1/2 repository ports and the
// outbox, but the Phase 3 record stores live in flexcore-storage (the
// recorded D-P3.21-4 home) with no fakes there. The Test-variant backend
// needs them: this module holds the three in-memory fakes the P3.28 routes
// run against in tests — [`FakeEnvelopeRecordStore`],
// [`FakeScheduleRecordStore`], [`FakeScheduleRevisionStore`] — honouring
// the same rules the adapters must: tenant scoping (records of another
// tenant never read), immutability conflicts, the revision chain
// discipline (revision n > 1 requires a superseded predecessor with the
// forward link), and keyset listing in primary-key order. No
// transactionality is claimed: the Test backend's atomicity evidence is
// the outbox-write pairing on the writes themselves (each
// `OutboxWrite` carries the receipt of the state change it was paired
// with) plus the database-gated fault tests; the fakes exist to exercise
// the route contracts, not the adapter obligations.
//
// [`FailingEventOutbox`] is the forced-failure injection for the Test
// backend: `append` always fails, so a route that reaches the outbox
// append fails the request — used with the fail-before-write assertions
// (a rejected request leaves NOTHING stored) and the ordering evidence.
//
// These types are test composition support compiled into the library (the
// integration tests use them through the public lib API); they contain no
// production behaviour.

use std::sync::Mutex;

use flexcore_domain::ids::FlexUuid;
use flexcore_domain::tenant_context::TenantContext;
use flexcore_ports::backend_error::BackendError;
use flexcore_ports::event_outbox::{EventOutbox, EventOutboxError, OutboxWrite, PendingOutboxEntry};
use flexcore_storage::envelope_store::{
    EnvelopeListFilter, EnvelopeRecordReceipt, EnvelopeRecordRow, EnvelopeRecordStore,
    EnvelopeStoreError,
};
use flexcore_storage::schedule_revision::{
    ScheduleRevisionChainStore, ScheduleRevisionError, ScheduleRevisionReceipt,
    ScheduleRevisionRecord, ScheduleRevisionRow, ScheduleSupersedeTarget,
    SCHEDULE_REVISION_STATE_SUPERSEDED,
};
use flexcore_storage::schedule_store::{
    ScheduleRecordReceipt, ScheduleRecordRow, ScheduleRecordStore, ScheduleStoreError,
};

fn tenant_matches(ctx: &TenantContext, row_tenant: FlexUuid) -> bool {
    row_tenant == ctx.tenant_id().uuid()
}

/// The identity bytes of a canonical-UUID text field (the fakes' keyset
/// order key; the adapters order in SQL over the uuid columns).
fn uuid_bytes(text: &str) -> [u8; 16] {
    FlexUuid::parse(text)
        .map(|id| id.to_bytes())
        .unwrap_or([0u8; 16])
}

fn opaque_backend() -> BackendError {
    BackendError::new(
        "E_FAKE_BACKEND".to_string(),
        "the in-memory fake backend failed (injected)".to_string(),
    )
}

// ---------------------------------------------------------------------------
// Envelope records
// ---------------------------------------------------------------------------

/// In-memory [`EnvelopeRecordStore`]: keyed by envelope identity, the
/// (tenant, portfolio, version) uniqueness rule enforced
/// ([`EnvelopeStoreError::Conflict`]), tenant-scoped reads, keyset listing
/// in ascending envelope-id order.
#[derive(Default)]
pub struct FakeEnvelopeRecordStore {
    rows: Mutex<Vec<EnvelopeRecordRow>>,
}

impl FakeEnvelopeRecordStore {
    /// Creates an empty fake.
    pub fn new() -> Self {
        Self::default()
    }

    /// A snapshot of every stored row (test inspection helper).
    pub fn rows(&self) -> Vec<EnvelopeRecordRow> {
        self.rows.lock().expect("poisoned mutex in test fake").clone()
    }
}

impl EnvelopeRecordStore for FakeEnvelopeRecordStore {
    fn insert_envelope_record(
        &self,
        ctx: &TenantContext,
        row: &EnvelopeRecordRow,
    ) -> Result<EnvelopeRecordReceipt, EnvelopeStoreError> {
        let mut rows = self.rows.lock().expect("poisoned mutex in test fake");
        if rows.iter().any(|r| {
            r.tenant_id == ctx.tenant_id()
                && r.portfolio_id == row.portfolio_id
                && r.envelope_version == row.envelope_version
        }) {
            return Err(EnvelopeStoreError::Conflict {
                portfolio_id: row.portfolio_id.clone(),
                envelope_version: row.envelope_version.clone(),
            });
        }
        rows.push(row.clone());
        Ok(EnvelopeRecordReceipt::new(
            row.envelope_id.clone(),
            ctx.tenant_id(),
        ))
    }

    fn find_envelope_record(
        &self,
        ctx: &TenantContext,
        envelope_id: &str,
    ) -> Result<Option<EnvelopeRecordRow>, EnvelopeStoreError> {
        let rows = self.rows.lock().expect("poisoned mutex in test fake");
        Ok(rows
            .iter()
            .find(|r| r.envelope_id == envelope_id && tenant_matches(ctx, r.tenant_id.uuid()))
            .cloned())
    }

    fn list_envelope_records(
        &self,
        ctx: &TenantContext,
        filter: &EnvelopeListFilter,
        limit: usize,
    ) -> Result<Vec<EnvelopeRecordRow>, EnvelopeStoreError> {
        let rows = self.rows.lock().expect("poisoned mutex in test fake");
        let mut matched: Vec<&EnvelopeRecordRow> = rows
            .iter()
            .filter(|r| tenant_matches(ctx, r.tenant_id.uuid()))
            .filter(|r| match &filter.portfolio_id {
                Some(p) => &r.portfolio_id == p,
                None => true,
            })
            .filter(|r| match &filter.after_envelope_id {
                Some(after) => FlexUuid::parse(after)
                    .map(|a| a.to_bytes() < uuid_bytes(&r.envelope_id))
                    .unwrap_or(false),
                None => true,
            })
            .collect();
        matched.sort_by(|a, b| uuid_bytes(&a.envelope_id).cmp(&uuid_bytes(&b.envelope_id)));
        Ok(matched.into_iter().take(limit).cloned().collect())
    }
}

// ---------------------------------------------------------------------------
// Schedule-result records (§12.6 optimization runs)
// ---------------------------------------------------------------------------

/// In-memory [`ScheduleRecordStore`]: keyed by candidate identity (the
/// immutability conflict), tenant-scoped reads, keyset listing in ascending
/// candidate-id order.
#[derive(Default)]
pub struct FakeScheduleRecordStore {
    rows: Mutex<Vec<ScheduleRecordRow>>,
}

impl FakeScheduleRecordStore {
    /// Creates an empty fake.
    pub fn new() -> Self {
        Self::default()
    }

    /// A snapshot of every stored row (test inspection helper).
    pub fn rows(&self) -> Vec<ScheduleRecordRow> {
        self.rows.lock().expect("poisoned mutex in test fake").clone()
    }
}

impl ScheduleRecordStore for FakeScheduleRecordStore {
    fn insert_schedule_record(
        &self,
        ctx: &TenantContext,
        row: &ScheduleRecordRow,
    ) -> Result<ScheduleRecordReceipt, ScheduleStoreError> {
        let mut rows = self.rows.lock().expect("poisoned mutex in test fake");
        if rows.iter().any(|r| {
            r.tenant_id == ctx.tenant_id() && r.candidate_id == row.candidate_id
        }) {
            return Err(ScheduleStoreError::Conflict {
                candidate_id: row.candidate_id.clone(),
            });
        }
        rows.push(row.clone());
        Ok(ScheduleRecordReceipt::new(
            row.candidate_id.clone(),
            ctx.tenant_id(),
        ))
    }

    fn find_schedule_record(
        &self,
        ctx: &TenantContext,
        candidate_id: &str,
    ) -> Result<Option<ScheduleRecordRow>, ScheduleStoreError> {
        let rows = self.rows.lock().expect("poisoned mutex in test fake");
        Ok(rows
            .iter()
            .find(|r| r.candidate_id == candidate_id && tenant_matches(ctx, r.tenant_id.uuid()))
            .cloned())
    }

    fn list_schedule_records(
        &self,
        ctx: &TenantContext,
        portfolio_id: Option<&str>,
        site_id: Option<&str>,
        status: Option<&str>,
        after_candidate_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ScheduleRecordRow>, ScheduleStoreError> {
        let rows = self.rows.lock().expect("poisoned mutex in test fake");
        let mut matched: Vec<&ScheduleRecordRow> = rows
            .iter()
            .filter(|r| tenant_matches(ctx, r.tenant_id.uuid()))
            .filter(|r| match portfolio_id {
                Some(p) => r.portfolio_id == p,
                None => true,
            })
            .filter(|r| match site_id {
                Some(s) => r.site_id == s,
                None => true,
            })
            .filter(|r| match status {
                Some(s) => r.status == s,
                None => true,
            })
            .filter(|r| match after_candidate_id {
                Some(after) => FlexUuid::parse(after)
                    .map(|a| a.to_bytes() < uuid_bytes(&r.candidate_id))
                    .unwrap_or(false),
                None => true,
            })
            .collect();
        matched.sort_by(|a, b| uuid_bytes(&a.candidate_id).cmp(&uuid_bytes(&b.candidate_id)));
        Ok(matched.into_iter().take(limit).cloned().collect())
    }
}

// ---------------------------------------------------------------------------
// Schedule revision chains
// ---------------------------------------------------------------------------

/// One stored revision in the fake: the immutable row plus the state and
/// the forward supersede link (the [`ScheduleRevisionRecord`] shape).
struct StoredRevision {
    record: ScheduleRevisionRecord,
}

/// In-memory [`ScheduleRevisionChainStore`]: the revision chain with the
/// §14.2 SUPERSEDED transition, the chain discipline
/// (revision n > 1 requires a predecessor that exists, is superseded, and
/// points its forward link at THIS revision), the one-active invariant,
/// and the keyset listing in `(schedule_id, revision)` order.
#[derive(Default)]
pub struct FakeScheduleRevisionStore {
    rows: Mutex<Vec<StoredRevision>>,
}

impl FakeScheduleRevisionStore {
    /// Creates an empty fake.
    pub fn new() -> Self {
        Self::default()
    }

    /// A snapshot of every stored record (test inspection helper).
    pub fn rows(&self) -> Vec<ScheduleRevisionRecord> {
        self.rows
            .lock()
            .expect("poisoned mutex in test fake")
            .iter()
            .map(|s| s.record.clone())
            .collect()
    }
}

impl ScheduleRevisionChainStore for FakeScheduleRevisionStore {
    fn insert_schedule_revision(
        &self,
        ctx: &TenantContext,
        row: &ScheduleRevisionRow,
    ) -> Result<ScheduleRevisionReceipt, ScheduleRevisionError> {
        let mut rows = self.rows.lock().expect("poisoned mutex in test fake");
        if rows.iter().any(|s| {
            s.record.row.tenant_id == ctx.tenant_id()
                && s.record.row.schedule_id == row.schedule_id
                && s.record.row.revision == row.revision
        }) {
            return Err(ScheduleRevisionError::Conflict {
                schedule_id: row.schedule_id.clone(),
                revision: row.revision,
            });
        }
        // Chain verification (D-P3.22-1), mirroring the adapter: revision
        // n > 1 must find its exact predecessor — present, already
        // SUPERSEDED, pointing its forward link at THIS revision.
        if row.revision > 1 {
            let prior = rows.iter().find(|s| {
                s.record.row.tenant_id == ctx.tenant_id()
                    && s.record.row.schedule_id == row.schedule_id
                    && s.record.row.revision == row.revision - 1
            });
            match prior {
                None => {
                    return Err(ScheduleRevisionError::ChainBroken {
                        schedule_id: row.schedule_id.clone(),
                        revision: row.revision,
                        detail: format!("the predecessor revision {} does not exist", row.revision - 1),
                    });
                }
                Some(prior) => {
                    if prior.record.state != SCHEDULE_REVISION_STATE_SUPERSEDED {
                        return Err(ScheduleRevisionError::ChainBroken {
                            schedule_id: row.schedule_id.clone(),
                            revision: row.revision,
                            detail: format!(
                                "the predecessor revision {} is still active",
                                row.revision - 1
                            ),
                        });
                    }
                    if prior.record.superseded_by_revision != Some(row.revision) {
                        return Err(ScheduleRevisionError::ChainBroken {
                            schedule_id: row.schedule_id.clone(),
                            revision: row.revision,
                            detail: format!(
                                "the predecessor revision {} is superseded by a different revision",
                                row.revision - 1
                            ),
                        });
                    }
                }
            }
        }
        let record = ScheduleRevisionRecord::new(
            row.clone(),
            flexcore_storage::schedule_revision::SCHEDULE_REVISION_STATE_ACTIVE.to_string(),
            None,
            None,
        )
        .map_err(|_| ScheduleRevisionError::InvalidRevision { field: "row" })?;
        rows.push(StoredRevision { record });
        Ok(ScheduleRevisionReceipt::new(
            row.schedule_id.clone(),
            row.revision,
            ctx.tenant_id(),
        ))
    }

    fn mark_revision_superseded(
        &self,
        ctx: &TenantContext,
        schedule_id: &str,
        revision: u64,
        superseded_by: &ScheduleSupersedeTarget,
    ) -> Result<ScheduleRevisionReceipt, ScheduleRevisionError> {
        if superseded_by.revision != revision.checked_add(1).ok_or(
            ScheduleRevisionError::InvalidRevision { field: "superseded_by_revision" },
        )? {
            return Err(ScheduleRevisionError::InvalidRevision {
                field: "superseded_by_revision",
            });
        }
        let mut rows = self.rows.lock().expect("poisoned mutex in test fake");
        let found = rows.iter_mut().find(|s| {
            s.record.row.tenant_id == ctx.tenant_id()
                && s.record.row.schedule_id == schedule_id
                && s.record.row.revision == revision
        });
        match found {
            None => Err(ScheduleRevisionError::NotFound {
                schedule_id: schedule_id.to_string(),
                revision,
            }),
            Some(stored) => {
                if stored.record.state == SCHEDULE_REVISION_STATE_SUPERSEDED {
                    return Err(ScheduleRevisionError::AlreadySuperseded {
                        schedule_id: schedule_id.to_string(),
                        revision,
                    });
                }
                let record = ScheduleRevisionRecord::new(
                    stored.record.row.clone(),
                    SCHEDULE_REVISION_STATE_SUPERSEDED.to_string(),
                    Some(superseded_by.revision),
                    Some(superseded_by.candidate_id.clone()),
                )
                .map_err(|_| ScheduleRevisionError::InvalidRevision { field: "supersede_link" })?;
                stored.record = record;
                Ok(ScheduleRevisionReceipt::new(
                    schedule_id.to_string(),
                    revision,
                    ctx.tenant_id(),
                ))
            }
        }
    }

    fn find_revision(
        &self,
        ctx: &TenantContext,
        schedule_id: &str,
        revision: u64,
    ) -> Result<Option<ScheduleRevisionRecord>, ScheduleRevisionError> {
        let rows = self.rows.lock().expect("poisoned mutex in test fake");
        Ok(rows
            .iter()
            .find(|s| {
                s.record.row.tenant_id == ctx.tenant_id()
                    && s.record.row.schedule_id == schedule_id
                    && s.record.row.revision == revision
            })
            .map(|s| s.record.clone()))
    }

    fn current_revision(
        &self,
        ctx: &TenantContext,
        schedule_id: &str,
    ) -> Result<Option<ScheduleRevisionRecord>, ScheduleRevisionError> {
        let rows = self.rows.lock().expect("poisoned mutex in test fake");
        Ok(rows
            .iter()
            .find(|s| {
                s.record.row.tenant_id == ctx.tenant_id()
                    && s.record.row.schedule_id == schedule_id
                    && s.record.state
                        == flexcore_storage::schedule_revision::SCHEDULE_REVISION_STATE_ACTIVE
            })
            .map(|s| s.record.clone()))
    }

    fn revision_chain(
        &self,
        ctx: &TenantContext,
        schedule_id: &str,
    ) -> Result<Vec<ScheduleRevisionRecord>, ScheduleRevisionError> {
        let rows = self.rows.lock().expect("poisoned mutex in test fake");
        let mut chain: Vec<ScheduleRevisionRecord> = rows
            .iter()
            .filter(|s| {
                s.record.row.tenant_id == ctx.tenant_id()
                    && s.record.row.schedule_id == schedule_id
            })
            .map(|s| s.record.clone())
            .collect();
        chain.sort_by_key(|r| r.row.revision);
        Ok(chain)
    }

    fn is_revision_superseded(
        &self,
        ctx: &TenantContext,
        schedule_id: &str,
        revision: u64,
    ) -> Result<Option<bool>, ScheduleRevisionError> {
        let rows = self.rows.lock().expect("poisoned mutex in test fake");
        Ok(rows
            .iter()
            .find(|s| {
                s.record.row.tenant_id == ctx.tenant_id()
                    && s.record.row.schedule_id == schedule_id
                    && s.record.row.revision == revision
            })
            .map(|s| s.record.state == SCHEDULE_REVISION_STATE_SUPERSEDED))
    }

    fn list_revisions(
        &self,
        ctx: &TenantContext,
        portfolio_id: Option<&str>,
        after_schedule_id: Option<&str>,
        after_revision: Option<u64>,
        limit: usize,
    ) -> Result<Vec<ScheduleRevisionRecord>, ScheduleRevisionError> {
        let rows = self.rows.lock().expect("poisoned mutex in test fake");
        let mut matched: Vec<&ScheduleRevisionRecord> = rows
            .iter()
            .map(|s| &s.record)
            .filter(|r| r.row.tenant_id == ctx.tenant_id())
            .filter(|r| match portfolio_id {
                Some(p) => r.row.portfolio_id == p,
                None => true,
            })
            .filter(|r| match (after_schedule_id, after_revision) {
                (Some(after), Some(after_rev)) => {
                    let after_bytes = FlexUuid::parse(after)
                        .map(|a| a.to_bytes())
                        .unwrap_or([0u8; 16]);
                    (after_bytes, after_rev)
                        < (uuid_bytes(&r.row.schedule_id), r.row.revision)
                }
                _ => true,
            })
            .collect();
        matched.sort_by(|a, b| {
            (uuid_bytes(&a.row.schedule_id), a.row.revision)
                .cmp(&(uuid_bytes(&b.row.schedule_id), b.row.revision))
        });
        Ok(matched.into_iter().take(limit).cloned().collect())
    }
}

// ---------------------------------------------------------------------------
// Failure injection
// ---------------------------------------------------------------------------

/// An [`EventOutbox`] whose `append` ALWAYS fails with the opaque backend
/// error — the Test-backend forced-failure injection. Every other
/// operation fails too (nothing was ever stored).
pub struct FailingEventOutbox;

impl EventOutbox for FailingEventOutbox {
    fn append(&self, _write: OutboxWrite) -> Result<(), EventOutboxError> {
        Err(EventOutboxError::Backend(opaque_backend()))
    }

    fn pending(&self, _limit: u32) -> Result<Vec<PendingOutboxEntry>, EventOutboxError> {
        Err(EventOutboxError::Backend(opaque_backend()))
    }

    fn record_attempt(&self, idempotency_key: &str) -> Result<(), EventOutboxError> {
        Err(EventOutboxError::UnknownKey {
            idempotency_key: idempotency_key.to_string(),
        })
    }

    fn mark_delivered(&self, idempotency_key: &str) -> Result<(), EventOutboxError> {
        Err(EventOutboxError::UnknownKey {
            idempotency_key: idempotency_key.to_string(),
        })
    }

    fn mark_failed(&self, idempotency_key: &str) -> Result<(), EventOutboxError> {
        Err(EventOutboxError::UnknownKey {
            idempotency_key: idempotency_key.to_string(),
        })
    }
}
