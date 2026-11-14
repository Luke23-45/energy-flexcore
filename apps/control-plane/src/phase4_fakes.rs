// P4.30 — In-memory fakes for the Phase 4 record stores (Test-backend
//         composition support).
//
// The flexcore-ports fakes cover the Stage 1/2 repository ports and the
// outbox, and `phase3_fakes` covers the Phase 3 record stores, but the
// Phase 4 record stores live in flexcore-storage (the recorded D-P3.21-4
// home) with no fakes there. This module holds the two in-memory fakes the
// P4.30 read routes run against in tests — [`FakeCommandRecordStore`] and
// [`FakeFeasibilityCheckStore`] — honouring the same rules the adapters
// must: tenant scoping (records of another tenant never read — a
// foreign-tenant record reads as `None` / is absent from listings),
// immutability conflicts, and bounded keyset listing. No transactionality
// is claimed: the Test backend's atomicity evidence is the outbox-write
// pairing plus the database-gated adapter tests; the fakes exist to
// exercise the route contracts, not the adapter obligations.
//
// P4.36 appended additively: [`FakeAuditEventReader`], the in-memory
// audit-events reader (SELECT-only port; seeding is a TEST-ONLY helper on
// the fake — the port itself has no write method).
//
// These types are test composition support compiled into the library (the
// integration tests use them through the public lib API); they contain no
// production behaviour.

use std::sync::Mutex;

use flexcore_domain::ids::FlexUuid;
use flexcore_domain::tenant_context::TenantContext;
use flexcore_storage::audit_read::{
    AuditEventFilter, AuditEventReadError, AuditEventReader, AuditEventRow,
};
use flexcore_storage::command_store::{
    CommandListFilter, CommandRecord, CommandRecordRow, CommandRecordStore, CommandReceipt,
    CommandStoreError, CommandTransitionReceipt, CommandTransitionRecord,
};
use flexcore_storage::feasibility_check::{
    FeasibilityCheckFilter, FeasibilityCheckRow, FeasibilityCheckStore,
    FeasibilityCheckStoreError,
};

fn tenant_matches(ctx: &TenantContext, row_tenant: FlexUuid) -> bool {
    row_tenant == ctx.tenant_id().uuid()
}

// ---------------------------------------------------------------------------
// Command records
// ---------------------------------------------------------------------------

/// One stored command with its journal (the fake's in-memory shape).
#[derive(Clone)]
struct StoredCommand {
    record: CommandRecord,
    journal: Vec<CommandTransitionRecord>,
}

/// In-memory [`CommandRecordStore`]: keyed by (command_id, tenant_id),
/// the immutability conflict enforced, the compare-and-swap transition
/// honoured (a transition applies only from the state the caller saw),
/// tenant-scoped reads, newest-first listing. The supersede backward-link
/// derivation of the real adapter is NOT re-implemented here (the fake
/// stores the transition as given; the frozen-edge validation is the
/// storage constructor's, exercised in flexcore-storage's own tests).
#[derive(Default)]
pub struct FakeCommandRecordStore {
    rows: Mutex<Vec<StoredCommand>>,
}

impl FakeCommandRecordStore {
    /// Creates an empty fake.
    pub fn new() -> Self {
        Self::default()
    }

    /// A snapshot of every stored record (test inspection helper).
    pub fn records(&self) -> Vec<CommandRecord> {
        self.rows
            .lock()
            .expect("poisoned mutex in test fake")
            .iter()
            .map(|stored| stored.record.clone())
            .collect()
    }
}

impl CommandRecordStore for FakeCommandRecordStore {
    fn insert_command_record(
        &self,
        ctx: &TenantContext,
        row: &CommandRecordRow,
    ) -> Result<CommandReceipt, CommandStoreError> {
        let mut rows = self.rows.lock().expect("poisoned mutex in test fake");
        if rows
            .iter()
            .any(|stored| stored.record.row.command_id == row.command_id && tenant_matches(ctx, row.tenant_id.uuid()))
        {
            return Err(CommandStoreError::Conflict {
                command_id: row.command_id.clone(),
            });
        }
        let record = CommandRecord::new(row.clone(), "PROPOSED".to_string(), None)
            .map_err(|_| CommandStoreError::InvalidRecord { field: "state" })?;
        rows.push(StoredCommand {
            record,
            journal: Vec::new(),
        });
        Ok(CommandReceipt::new(row.command_id.clone(), ctx.tenant_id()))
    }

    fn record_command_transition(
        &self,
        ctx: &TenantContext,
        transition: &CommandTransitionRecord,
    ) -> Result<CommandTransitionReceipt, CommandStoreError> {
        let mut rows = self.rows.lock().expect("poisoned mutex in test fake");
        let stored = rows
            .iter_mut()
            .find(|stored| {
                stored.record.row.command_id == transition.command_id
                    && tenant_matches(ctx, stored.record.row.tenant_id.uuid())
            })
            .ok_or_else(|| CommandStoreError::NotFound {
                command_id: transition.command_id.clone(),
            })?;
        // The compare-and-swap: the transition applies only from the state
        // the caller saw (a mismatch is the named conflict — the
        // one-active-transition invariant's first half).
        if stored.record.state != transition.from_state {
            return Err(CommandStoreError::TransitionConflict {
                command_id: transition.command_id.clone(),
                expected_from: transition.from_state.clone(),
                actual_state: stored.record.state.clone(),
            });
        }
        // The SUPERSEDED backward link (D-P4.10-3): derived from the
        // `command:<uuid>` evidence reference, exactly like the adapter.
        let superseded_by = match (transition.to_state.as_str(), transition.evidence_reference.as_deref()) {
            ("SUPERSEDED", Some(reference)) => {
                let replacing = reference.strip_prefix("command:").ok_or(
                    CommandStoreError::InvalidRecord {
                        field: "evidence_reference",
                    },
                )?;
                FlexUuid::parse(replacing).map_err(|_| CommandStoreError::InvalidRecord {
                    field: "superseded_by_command_id",
                })?;
                Some(replacing.to_string())
            }
            ("SUPERSEDED", None) => {
                return Err(CommandStoreError::InvalidRecord {
                    field: "evidence_reference",
                })
            }
            _ => None,
        };
        let seq = stored.journal.len() as i64 + 1;
        stored.record.state = transition.to_state.clone();
        stored.record.superseded_by_command_id = superseded_by;
        stored.journal.push(transition.clone());
        Ok(CommandTransitionReceipt {
            command_id: transition.command_id.clone(),
            seq,
            tenant_id: ctx.tenant_id(),
        })
    }

    fn find_command(
        &self,
        ctx: &TenantContext,
        command_id: &str,
    ) -> Result<Option<CommandRecord>, CommandStoreError> {
        let rows = self.rows.lock().expect("poisoned mutex in test fake");
        Ok(rows
            .iter()
            .find(|stored| {
                stored.record.row.command_id == command_id && tenant_matches(ctx, stored.record.row.tenant_id.uuid())
            })
            .map(|stored| stored.record.clone()))
    }

    fn transition_journal(
        &self,
        ctx: &TenantContext,
        command_id: &str,
    ) -> Result<Vec<CommandTransitionRecord>, CommandStoreError> {
        let rows = self.rows.lock().expect("poisoned mutex in test fake");
        Ok(rows
            .iter()
            .find(|stored| {
                stored.record.row.command_id == command_id && tenant_matches(ctx, stored.record.row.tenant_id.uuid())
            })
            .map(|stored| stored.journal.clone())
            .unwrap_or_default())
    }

    fn list_commands(
        &self,
        ctx: &TenantContext,
        filter: &CommandListFilter,
        limit: u32,
    ) -> Result<Vec<CommandRecord>, CommandStoreError> {
        if let Some(state) = filter.state.as_deref() {
            if !flexcore_storage::command_store::COMMAND_STATES.contains(&state) {
                return Err(CommandStoreError::InvalidRecord { field: "state" });
            }
        }
        let rows = self.rows.lock().expect("poisoned mutex in test fake");
        let mut matches: Vec<&StoredCommand> = rows
            .iter()
            .filter(|stored| {
                tenant_matches(ctx, stored.record.row.tenant_id.uuid())
                    && filter
                        .state
                        .as_deref()
                        .is_none_or(|state| stored.record.state == state)
                    && filter
                        .after_command_id
                        .as_deref()
                        .is_none_or(|after| uuid_bytes(&stored.record.row.command_id) < uuid_bytes(after))
            })
            .collect();
        // Newest first (UUIDv7 id order is a stable time order).
        matches.sort_by(|a, b| {
            uuid_bytes(&b.record.row.command_id).cmp(&uuid_bytes(&a.record.row.command_id))
        });
        Ok(matches
            .into_iter()
            .take(limit as usize)
            .map(|stored| stored.record.clone())
            .collect())
    }
}

fn uuid_bytes(text: &str) -> [u8; 16] {
    FlexUuid::parse(text)
        .map(|id| id.to_bytes())
        .unwrap_or([0u8; 16])
}

// ---------------------------------------------------------------------------
// Feasibility-check records
// ---------------------------------------------------------------------------

/// In-memory [`FeasibilityCheckStore`]: keyed by (check_id, tenant_id),
/// the immutability conflict enforced, tenant-scoped reads, listing in
/// `(checked_at, check_id)` order.
#[derive(Default)]
pub struct FakeFeasibilityCheckStore {
    rows: Mutex<Vec<FeasibilityCheckRow>>,
}

impl FakeFeasibilityCheckStore {
    /// Creates an empty fake.
    pub fn new() -> Self {
        Self::default()
    }

    /// A snapshot of every stored row (test inspection helper).
    pub fn rows(&self) -> Vec<FeasibilityCheckRow> {
        self.rows.lock().expect("poisoned mutex in test fake").clone()
    }
}

impl FeasibilityCheckStore for FakeFeasibilityCheckStore {
    fn insert_feasibility_check(
        &self,
        ctx: &TenantContext,
        row: &FeasibilityCheckRow,
    ) -> Result<(), FeasibilityCheckStoreError> {
        let mut rows = self.rows.lock().expect("poisoned mutex in test fake");
        if rows
            .iter()
            .any(|r| r.check_id == row.check_id && tenant_matches(ctx, row.tenant_id.uuid()))
        {
            return Err(FeasibilityCheckStoreError::Conflict {
                check_id: row.check_id.clone(),
            });
        }
        rows.push(row.clone());
        Ok(())
    }

    fn find_feasibility_check(
        &self,
        ctx: &TenantContext,
        check_id: &str,
    ) -> Result<Option<FeasibilityCheckRow>, FeasibilityCheckStoreError> {
        let rows = self.rows.lock().expect("poisoned mutex in test fake");
        Ok(rows
            .iter()
            .find(|r| r.check_id == check_id && tenant_matches(ctx, r.tenant_id.uuid()))
            .cloned())
    }

    fn list_feasibility_checks(
        &self,
        ctx: &TenantContext,
        filter: &FeasibilityCheckFilter,
        limit: u32,
    ) -> Result<Vec<FeasibilityCheckRow>, FeasibilityCheckStoreError> {
        let rows = self.rows.lock().expect("poisoned mutex in test fake");
        let mut matches: Vec<FeasibilityCheckRow> = rows
            .iter()
            .filter(|r| {
                tenant_matches(ctx, r.tenant_id.uuid())
                    && filter
                        .command_id
                        .as_deref()
                        .is_none_or(|command| r.command_id.as_deref() == Some(command))
                    && filter
                        .schedule_id
                        .as_deref()
                        .is_none_or(|schedule| r.schedule_id == schedule)
            })
            .cloned()
            .collect();
        matches.sort_by(|a, b| {
            (a.checked_at_unix_millis, a.check_id.clone())
                .cmp(&(b.checked_at_unix_millis, b.check_id.clone()))
        });
        if let Some(after) = filter.after_check_id.as_deref() {
            if let Some(anchor) = matches.iter().find(|r| r.check_id == after) {
                let anchor_stamp = (anchor.checked_at_unix_millis, anchor.check_id.clone());
                matches.retain(|r| (r.checked_at_unix_millis, r.check_id.clone()) > anchor_stamp);
            }
        }
        matches.truncate(limit as usize);
        Ok(matches)
    }
}

// ---------------------------------------------------------------------------
// Audit events (P4.36 — the SELECT-only read port)
// ---------------------------------------------------------------------------

/// In-memory [`AuditEventReader`]: tenant-scoped reads over seeded rows,
/// listing in `(event_time, audit_id)` order with the same filter and
/// keyset-cursor semantics the Postgres adapter implements (exact
/// action/actor/object matches, INCLUSIVE time bounds, the anchor
/// `(event_time, audit_id)` cursor). The port is SELECT-only; `seed` is a
/// TEST-ONLY helper on the fake (the real write path is the P1.10
/// `AuditSink`, which this fake does not model).
#[derive(Default)]
pub struct FakeAuditEventReader {
    rows: Mutex<Vec<AuditEventRow>>,
}

impl FakeAuditEventReader {
    /// Creates an empty fake.
    pub fn new() -> Self {
        Self::default()
    }

    /// Seeds one audit row (test helper — the port itself has no write
    /// method; the row constructor's storage-level rules are enforced
    /// before the row is stored).
    pub fn seed(&self, row: AuditEventRow) {
        self.rows
            .lock()
            .expect("poisoned mutex in test fake")
            .push(row);
    }

    /// A snapshot of every stored row (test inspection helper).
    pub fn rows(&self) -> Vec<AuditEventRow> {
        self.rows.lock().expect("poisoned mutex in test fake").clone()
    }
}

impl AuditEventReader for FakeAuditEventReader {
    fn find_audit_event(
        &self,
        ctx: &TenantContext,
        audit_id: &str,
    ) -> Result<Option<AuditEventRow>, AuditEventReadError> {
        if audit_id.trim().is_empty() {
            return Err(AuditEventReadError::InvalidFilter { field: "audit_id" });
        }
        let rows = self.rows.lock().expect("poisoned mutex in test fake");
        Ok(rows
            .iter()
            .find(|r| r.audit_id == audit_id.trim() && r.tenant_id.map(|t| t.uuid()) == Some(ctx.tenant_id().uuid()))
            .cloned())
    }

    fn list_audit_events(
        &self,
        ctx: &TenantContext,
        filter: &AuditEventFilter,
        limit: u32,
    ) -> Result<Vec<AuditEventRow>, AuditEventReadError> {
        for (field, value) in [
            ("action", filter.action.as_deref()),
            ("actor", filter.actor.as_deref()),
            ("object_type", filter.object_type.as_deref()),
            ("object_id", filter.object_id.as_deref()),
        ] {
            if let Some(value) = value {
                if value.trim().is_empty() {
                    return Err(AuditEventReadError::InvalidFilter { field });
                }
            }
        }
        if let (Some(from), Some(to)) =
            (filter.from_event_unix_millis, filter.to_event_unix_millis)
        {
            if from > to {
                return Err(AuditEventReadError::InvalidFilter {
                    field: "from_event_unix_millis",
                });
            }
        }
        let rows = self.rows.lock().expect("poisoned mutex in test fake");
        let mut matches: Vec<AuditEventRow> = rows
            .iter()
            .filter(|r| {
                r.tenant_id.map(|t| t.uuid()) == Some(ctx.tenant_id().uuid())
                    && filter
                        .action
                        .as_deref()
                        .is_none_or(|action| r.action == action)
                    && filter
                        .actor
                        .as_deref()
                        .is_none_or(|actor| r.actor == actor)
                    && filter
                        .object_type
                        .as_deref()
                        .is_none_or(|object_type| r.object_type == object_type)
                    && filter
                        .object_id
                        .as_deref()
                        .is_none_or(|object_id| r.object_id == object_id)
                    && filter
                        .from_event_unix_millis
                        .is_none_or(|from| r.event_time_unix_millis >= from)
                    && filter
                        .to_event_unix_millis
                        .is_none_or(|to| r.event_time_unix_millis <= to)
            })
            .cloned()
            .collect();
        matches.sort_by(|a, b| {
            (a.event_time_unix_millis, a.audit_id.clone())
                .cmp(&(b.event_time_unix_millis, b.audit_id.clone()))
        });
        if let Some(after) = filter.after_audit_id.as_deref() {
            if let Some(anchor) = matches.iter().find(|r| r.audit_id == after.trim()) {
                let anchor_stamp =
                    (anchor.event_time_unix_millis, anchor.audit_id.clone());
                matches.retain(|r| {
                    (r.event_time_unix_millis, r.audit_id.clone()) > anchor_stamp
                });
            }
        }
        matches.truncate(limit as usize);
        Ok(matches)
    }
}
