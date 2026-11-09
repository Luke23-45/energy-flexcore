// P3.22 — Schedule identity, revision chain, and supersede (report §12.6,
//         §14.2; ledger P3.22). Decision record:
//         `ledger/phase-03-flexibility-scheduling/22_24_revisions_comparison_rolling.md`.
//
// A schedule has a STABLE IDENTITY and an immutable revision history: a
// newer revision SUPERSEDES its predecessor — it never erases, mutates, or
// deletes it (§14.2: SUPERSEDED, retained for audit, distinct from
// CANCELLED; the P3.03 rollover rule: a new horizon supersedes the prior
// schedule revision, never a silent overwrite).
//
// ## The revision identity scheme, decided once (D-P3.22-1)
//
// - A schedule CHAIN is identified by `schedule_id` (canonical UUID text) —
//   the identity a Phase 4 dispatch command will reference together with
//   the revision number (cross-reference P4.12: a command referencing a
//   superseded revision is rejected in Phase 4; this module exposes the
//   `ScheduleRevisionChainStore::is_revision_superseded` check the Phase 4
//   gate will call — NO command path is built here).
// - `revision` is MONOTONICALLY increasing per schedule chain, with NO
//   gaps: revision 1 is the initial solve; every revision n > 1 supersedes
//   exactly revision n − 1 (enforced in the database by a CHECK on the
//   chain link and by the UNIQUE primary key `(schedule_id, tenant_id,
//   revision)`; `mark_revision_superseded` enforces that the target is
//   exactly `revision + 1`).
// - The scope of the monotonicity is the portfolio/horizon: every revision
//   row records the owning portfolio and the EXACT horizon it covers
//   (start-inclusive / end-exclusive UTC millis), so
//   (portfolio, horizon, revision) is fully determined by the chain, and a
//   revision of another horizon is never confused with this one.
// - Every new revision is a CANDIDATE: the row is inserted in the `active`
//   state and the §12.6 candidate id it carries is a solver-outcome
//   candidate — never a feasible, approved, or dispatchable schedule
//   (feasibility, authorization, and dispatch are Phase 4 gates).
//
// ## The supersede discipline, decided once (D-P3.22-2)
//
// - When a new revision is produced, the prior revision is marked
//   SUPERSEDED (stored text `superseded`) — the ONLY state change a
//   revision row ever undergoes. Everything else about the row is
//   immutable.
// - The link is COMPLETE IN BOTH DIRECTIONS: the new revision records
//   `supersedes_revision` + `supersedes_candidate_id`; the superseded
//   revision records `superseded_by_revision` + `superseded_by_candidate_id`.
//   Both directions are enforced at rest (CHECK constraints + the trigger
//   in migration 0014) and tested (the adapter's DB-gated suite).
// - Immutability at the storage layer: revision content has NO update path
//   — the migration grants `flexcore_app` INSERT + SELECT plus a
//   COLUMN-LEVEL UPDATE grant on exactly the three supersede-transition
//   columns (the recorded exception anticipated by D-P3.21-8), and a BEFORE
//   UPDATE trigger rejects any other column change and any state movement
//   other than `active → superseded`.
//
// ## What this module can and cannot do
//
// - It CAN make broken chains unrepresentable in the row type: the
//   constructor enforces revision >= 1, the exact chain rule
//   (revision 1 ↔ no supersedes link; revision n > 1 ↔ supersedes exactly
//   n − 1 with a candidate reference), the closed state/trigger
//   vocabularies, and a sane horizon (end-exclusive end after the
//   start-inclusive start).
// - It CANNOT enforce atomicity, tenant scoping, or the one-active-revision
//   invariant. Those are adapter obligations (binding, stated on the trait)
//   and enforced by the adapter's tests; the database adds the mechanical
//   backstop (the partial unique index
//   `schedule_revisions_one_active` admits exactly ONE `active` revision
//   per schedule chain, so the supersede + insert pair must run in ONE
//   transaction — the same unit-of-work pattern as the P3.21 record store).
//
// Control status: schedule bookkeeping for the simulation-only 0.x line.
// No dispatch path, no command lifecycle, no device write — Phase 4 owns
// all of those. Nothing here implies a schedule was approved.

use flexcore_domain::ids::TenantId;

/// The closed revision-state vocabulary, as stored text. `active` is the
/// current candidate revision of the chain; `superseded` is the §14.2
/// SUPERSEDED state — retained for audit, distinct from CANCELLED. No other
/// state exists (enforced by the database CHECK).
pub const SCHEDULE_REVISION_STATES: [&str; 2] = ["active", "superseded"];

/// The stored text of the `active` state.
pub const SCHEDULE_REVISION_STATE_ACTIVE: &str = "active";

/// The stored text of the §14.2 `SUPERSEDED` state.
pub const SCHEDULE_REVISION_STATE_SUPERSEDED: &str = "superseded";

/// The closed roll-trigger vocabulary, as stored text (ledger P3.24 step 1:
/// "the trigger is RECORDED in each new schedule revision — required
/// field"). The set is the P3.24 trigger set plus `initial_solve` for the
/// first revision of a chain (a first solve has no re-solve trigger;
/// recording it as a named value keeps the column required and total).
/// The application layer (flexcore-application `rolling_horizon`) carries
/// the typed form with the SAME strings; both sets are pinned by tests.
pub const SCHEDULE_ROLL_TRIGGERS: [&str; 6] = [
    "initial_solve",
    "horizon_advance",
    "envelope_invalidation",
    "asset_opt_out_dropout",
    "forecast_update",
    "operator_request",
];

/// The revision-identity scheme, stated once (D-P3.22-1).
pub const REVISION_IDENTITY_STATEMENT: &str = "A schedule chain is identified \
     by schedule_id; revisions are monotonically increasing per chain with no \
     gaps (revision 1 = initial solve; revision n > 1 supersedes exactly \
     n - 1); every revision records its portfolio and its exact horizon. A \
     re-solve creates a NEW revision and never mutates the old one. Every new \
     revision is a CANDIDATE — never a feasible, approved, or dispatchable \
     schedule (Phase 4 gates).";

/// The supersede discipline, stated once (D-P3.22-2).
pub const SUPERSEDE_TRANSITION_STATEMENT: &str = "When a new revision is \
     produced, the prior revision is marked SUPERSEDED (§14.2, retained for \
     audit) — the only state change a revision row ever undergoes. The link \
     is complete in both directions: the new revision records \
     supersedes_revision + supersedes_candidate_id, the superseded revision \
     records superseded_by_revision + superseded_by_candidate_id. Revision \
     content has no update path (INSERT-only rows; the dedicated column-level \
     transition grant + trigger are the only writers of the supersede state).";

/// The Phase 4 cross-reference note (ledger P3.22 step 3): the check the
/// P4.12 dispatch gate will call is exposed on the trait. NO command path is
/// built in Phase 3.
pub const PHASE4_GATE_NOTE: &str = "Phase 4 dispatch commands reference \
     schedule ID + revision; a command referencing a superseded revision is \
     rejected in Phase 4 (P4.12). The gate will call \
     ScheduleRevisionChainStore::is_revision_superseded — None means the \
     revision does not exist in the caller's tenant scope (unknown is also \
     rejected, by the gate, never treated as usable). No command path exists \
     in Phase 3.";

// --- Errors -------------------------------------------------------------------

/// Errors of the schedule-revision store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleRevisionError {
    /// The row or transition argument is incomplete or malformed (`field`
    /// names the problem). Nothing was written.
    InvalidRevision {
        field: &'static str,
    },
    /// A revision with the same (schedule_id, revision) already exists —
    /// revisions are immutable and a re-solve is a NEW revision.
    Conflict {
        schedule_id: String,
        revision: u64,
    },
    /// The insert of revision n found no valid chain predecessor (revision
    /// n − 1 must exist, be `superseded`, and name this new revision as its
    /// `superseded_by` target). The chain would have a gap or a double
    /// supersede; nothing was written.
    ChainBroken {
        schedule_id: String,
        revision: u64,
        detail: String,
    },
    /// The referenced revision does not exist in the caller's tenant scope
    /// (existence never leaks across a tenant boundary, P0.16 rule 5).
    NotFound {
        schedule_id: String,
        revision: u64,
    },
    /// The transition was requested on a revision that is already
    /// `superseded` — the SUPERSEDED marking is a one-way transition.
    AlreadySuperseded {
        schedule_id: String,
        revision: u64,
    },
    /// The storage backend failed. `code` and `message` are opaque and
    /// human-safe — never raw storage text.
    Backend {
        code: String,
        message: String,
    },
}

impl std::fmt::Display for ScheduleRevisionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScheduleRevisionError::InvalidRevision { field } => {
                write!(f, "the schedule revision is invalid: '{field}' is missing or malformed")
            }
            ScheduleRevisionError::Conflict { schedule_id, revision } => write!(
                f,
                "a revision {revision} of schedule {schedule_id} already exists; \
                 revisions are immutable and a re-solve is a new revision"
            ),
            ScheduleRevisionError::ChainBroken { schedule_id, revision, detail } => write!(
                f,
                "the revision chain for schedule {schedule_id} cannot accept revision \
                 {revision}: {detail}"
            ),
            ScheduleRevisionError::NotFound { schedule_id, revision } => write!(
                f,
                "revision {revision} of schedule {schedule_id} does not exist in this \
                 tenant scope"
            ),
            ScheduleRevisionError::AlreadySuperseded { schedule_id, revision } => write!(
                f,
                "revision {revision} of schedule {schedule_id} is already SUPERSEDED; \
                 the transition is one-way"
            ),
            ScheduleRevisionError::Backend { code, message } => {
                write!(f, "storage backend failure ({code}): {message}")
            }
        }
    }
}

impl std::error::Error for ScheduleRevisionError {}

// --- Rows ---------------------------------------------------------------------

/// One schedule revision to INSERT (the write row). A row is always created
/// in the `active` state — every new revision is a CANDIDATE and the current
/// one of its chain; the SUPERSEDED marking is applied exclusively through
/// `ScheduleRevisionChainStore::mark_revision_superseded`, never by
/// re-inserting or mutating (D-P3.22-2). Construction goes exclusively
/// through [`ScheduleRevisionRow::new`].
///
/// Identifier fields are canonical hyphenated UUID text (the recorded
/// FlexUuid ↔ PG uuid mapping binds text and casts `::uuid` in SQL).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleRevisionRow {
    /// The owning tenant (P0.16 rule 1 — `tenant_id` on every tenant-owned
    /// record, present on the row itself so RLS and the explicit filter
    /// agree).
    pub tenant_id: TenantId,
    /// The owning portfolio (canonical UUID text). The revision-identity
    /// scope names the portfolio explicitly (D-P3.22-1).
    pub portfolio_id: String,
    /// The site the schedule was computed for (canonical UUID text).
    pub site_id: String,
    /// The schedule-chain identity (canonical UUID text; D-P3.22-1).
    pub schedule_id: String,
    /// The revision number (monotonically increasing per chain, no gaps;
    /// 1 = initial solve).
    pub revision: u64,
    /// The start-inclusive horizon start, UTC millis — the exact horizon
    /// this revision covers (part of the recorded identity scope).
    pub horizon_start_millis: i64,
    /// The end-exclusive horizon end, UTC millis (strictly after the start;
    /// §10.3 interval semantics).
    pub horizon_end_millis: i64,
    /// The roll trigger that produced this revision — a required field
    /// (ledger P3.24 step 1), one of [`SCHEDULE_ROLL_TRIGGERS`].
    pub roll_trigger: String,
    /// The prior revision this revision supersedes. `None` exactly when
    /// `revision == 1`; `Some(revision - 1)` otherwise (enforced by the
    /// constructor and the database CHECK).
    pub supersedes_revision: Option<u64>,
    /// The candidate identity of the superseded prior revision — required
    /// whenever `supersedes_revision` is `Some` (the link carries BOTH the
    /// revision number and the §12.6 candidate it points at).
    pub supersedes_candidate_id: Option<String>,
    /// The §12.6 candidate identity this revision realizes (canonical UUID
    /// text; FK to the P3.21 `schedule_results` record — the candidate and
    /// the revision are two views of ONE solve).
    pub candidate_id: String,
    /// The schedule content (per-asset per-interval setpoint plan) as JSON
    /// text — opaque to this crate (no serialization dependency here; the
    /// composition root serializes, the adapter validates it parses as JSON
    /// on insert). Immutable once written.
    pub schedule_payload_json: String,
}

impl ScheduleRevisionRow {
    /// Builds and validates one revision row (always `active`; see the type
    /// doc). Storage-level rules: every identifier non-empty; `revision`
    /// in `1..=i64::MAX`; horizon end strictly after horizon start; the
    /// roll trigger inside the closed vocabulary; the chain rule exact
    /// (revision 1 ↔ no supersedes link; revision n > 1 ↔ supersedes exactly
    /// n − 1 with a non-empty candidate reference); non-empty payload.
    pub fn new(
        tenant_id: TenantId,
        portfolio_id: String,
        site_id: String,
        schedule_id: String,
        revision: u64,
        horizon_start_millis: i64,
        horizon_end_millis: i64,
        roll_trigger: String,
        supersedes_revision: Option<u64>,
        supersedes_candidate_id: Option<String>,
        candidate_id: String,
        schedule_payload_json: String,
    ) -> Result<Self, ScheduleRevisionError> {
        for (field, value) in [
            ("portfolio_id", portfolio_id.as_str()),
            ("site_id", site_id.as_str()),
            ("schedule_id", schedule_id.as_str()),
            ("candidate_id", candidate_id.as_str()),
            ("schedule_payload_json", schedule_payload_json.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(ScheduleRevisionError::InvalidRevision { field });
            }
        }
        if revision == 0 || revision > i64::MAX as u64 {
            return Err(ScheduleRevisionError::InvalidRevision { field: "revision" });
        }
        if horizon_end_millis <= horizon_start_millis {
            return Err(ScheduleRevisionError::InvalidRevision {
                field: "horizon_end_millis",
            });
        }
        if !SCHEDULE_ROLL_TRIGGERS.contains(&roll_trigger.as_str()) {
            return Err(ScheduleRevisionError::InvalidRevision { field: "roll_trigger" });
        }
        // The exact chain rule (D-P3.22-1): no gaps, no skipped supersedes.
        match (revision, supersedes_revision, supersedes_candidate_id.as_deref()) {
            (1, None, None) => {}
            (1, _, _) => {
                return Err(ScheduleRevisionError::InvalidRevision {
                    field: "supersedes_revision",
                });
            }
            (n, Some(prior), Some(cand)) => {
                if prior != n - 1 {
                    return Err(ScheduleRevisionError::InvalidRevision {
                        field: "supersedes_revision",
                    });
                }
                if cand.trim().is_empty() {
                    return Err(ScheduleRevisionError::InvalidRevision {
                        field: "supersedes_candidate_id",
                    });
                }
            }
            (n, Some(_), None) => {
                let _ = n;
                return Err(ScheduleRevisionError::InvalidRevision {
                    field: "supersedes_candidate_id",
                });
            }
            (n, None, Some(_)) => {
                let _ = n;
                return Err(ScheduleRevisionError::InvalidRevision {
                    field: "supersedes_revision",
                });
            }
            (n, None, None) => {
                let _ = n;
                return Err(ScheduleRevisionError::InvalidRevision {
                    field: "supersedes_revision",
                });
            }
        }
        Ok(Self {
            tenant_id,
            portfolio_id,
            site_id,
            schedule_id,
            revision,
            horizon_start_millis,
            horizon_end_millis,
            roll_trigger,
            supersedes_revision,
            supersedes_candidate_id,
            candidate_id,
            schedule_payload_json,
        })
    }
}

/// One stored schedule revision as READ (the immutable row plus its
/// supersede state). Constructed exclusively through
/// [`ScheduleRevisionRecord::new`], which validates the stored state against
/// the closed vocabulary and the state-link consistency rules (the adapter
/// fails closed on a malformed row rather than smuggling it through).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleRevisionRecord {
    /// The immutable revision content.
    pub row: ScheduleRevisionRow,
    /// The stored state: [`SCHEDULE_REVISION_STATE_ACTIVE`] or
    /// [`SCHEDULE_REVISION_STATE_SUPERSEDED`].
    pub state: String,
    /// The revision that superseded THIS revision — the forward link of the
    /// bidirectional supersede pair (set exactly when `state` is
    /// `superseded`).
    pub superseded_by_revision: Option<u64>,
    /// The candidate of the superseding revision (set exactly when
    /// `state` is `superseded`).
    pub superseded_by_candidate_id: Option<String>,
}

impl ScheduleRevisionRecord {
    /// Builds and validates one stored record. Consistency rules (mirrored
    /// by the database CHECKs): `active` ↔ no superseded-by link;
    /// `superseded` ↔ superseded-by revision + candidate both present and
    /// strictly greater than this revision.
    pub fn new(
        row: ScheduleRevisionRow,
        state: String,
        superseded_by_revision: Option<u64>,
        superseded_by_candidate_id: Option<String>,
    ) -> Result<Self, ScheduleRevisionError> {
        if !SCHEDULE_REVISION_STATES.contains(&state.as_str()) {
            return Err(ScheduleRevisionError::InvalidRevision { field: "state" });
        }
        match (state.as_str(), superseded_by_revision, superseded_by_candidate_id.as_deref()) {
            (SCHEDULE_REVISION_STATE_ACTIVE, None, None) => {}
            (SCHEDULE_REVISION_STATE_ACTIVE, _, _) => {
                return Err(ScheduleRevisionError::InvalidRevision {
                    field: "superseded_by_revision",
                });
            }
            (SCHEDULE_REVISION_STATE_SUPERSEDED, Some(by), Some(by_cand)) => {
                if by <= row.revision {
                    return Err(ScheduleRevisionError::InvalidRevision {
                        field: "superseded_by_revision",
                    });
                }
                if by_cand.trim().is_empty() {
                    return Err(ScheduleRevisionError::InvalidRevision {
                        field: "superseded_by_candidate_id",
                    });
                }
            }
            (SCHEDULE_REVISION_STATE_SUPERSEDED, _, _) => {
                return Err(ScheduleRevisionError::InvalidRevision {
                    field: "superseded_by_candidate_id",
                });
            }
        }
        Ok(Self {
            row,
            state,
            superseded_by_revision,
            superseded_by_candidate_id,
        })
    }

    /// `true` when this revision is SUPERSEDED (§14.2). This is the pure
    /// form of the check; the persistence-backed check the Phase 4 gate
    /// (P4.12) will call is `ScheduleRevisionChainStore::is_revision_
    /// superseded`.
    pub fn is_superseded(&self) -> bool {
        self.state == SCHEDULE_REVISION_STATE_SUPERSEDED
    }
}

/// The target of a SUPERSEDED marking: the revision being created (the
/// marking on the prior revision must point exactly at it, both directions
/// complete). Constructed through [`ScheduleSupersedeTarget::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleSupersedeTarget {
    /// The new revision number (must be exactly `prior + 1` — the chain has
    /// no gaps).
    pub revision: u64,
    /// The new revision's §12.6 candidate identity (non-empty).
    pub candidate_id: String,
}

impl ScheduleSupersedeTarget {
    /// Builds and validates the supersede target: non-empty candidate
    /// identity, revision in `1..=i64::MAX`.
    pub fn new(revision: u64, candidate_id: String) -> Result<Self, ScheduleRevisionError> {
        if revision == 0 || revision > i64::MAX as u64 {
            return Err(ScheduleRevisionError::InvalidRevision { field: "revision" });
        }
        if candidate_id.trim().is_empty() {
            return Err(ScheduleRevisionError::InvalidRevision {
                field: "candidate_id",
            });
        }
        Ok(Self {
            revision,
            candidate_id,
        })
    }
}

/// The receipt of one persisted schedule revision or one SUPERSEDED marking.
/// Its existence proves the write was ISSUED (the same receipt convention
/// as the P3.21 record store).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleRevisionReceipt {
    /// The schedule chain the write belongs to (canonical UUID text).
    pub schedule_id: String,
    /// The revision the write concerns.
    pub revision: u64,
    /// The owning tenant.
    pub tenant_id: TenantId,
}

impl ScheduleRevisionReceipt {
    /// Builds the receipt (adapter-side; values are taken as given from the
    /// executed statement).
    pub fn new(schedule_id: String, revision: u64, tenant_id: TenantId) -> Self {
        Self {
            schedule_id,
            revision,
            tenant_id,
        }
    }
}

/// The schedule-revision persistence contract (ledger P3.22; §12.6
/// candidate identity, §14.2 supersede discipline). An append-oriented
/// revision chain store:
///
/// - records are immutable (INSERT + SELECT; the ONLY write besides the
///   insert is the dedicated SUPERSEDED transition below);
/// - every operation runs inside the CALLER's unit of work (adapter
///   obligation, binding) — the supersede marking of revision n − 1 and the
///   insert of revision n belong to ONE transaction, so the chain never
///   observably lacks a current revision;
/// - every query is tenant-scoped: RLS via the transaction-scoped
///   `flexcore.tenant_id` setting (P1.12 §5.1) PLUS an explicit `tenant_id`
///   filter; a foreign-tenant revision reads as `None` — existence never
///   leaks (P0.16 rule 5);
/// - storage failures surface as [`ScheduleRevisionError::Backend`] with an
///   opaque code + message only.
pub trait ScheduleRevisionChainStore {
    /// Inserts one NEW revision (always `active`). For `revision > 1` the
    /// adapter verifies inside the transaction that revision − 1 exists,
    /// is `superseded`, and names this revision as its superseded-by target
    /// (a violation is [`ScheduleRevisionError::ChainBroken`]). A
    /// (schedule, revision) collision is
    /// [`ScheduleRevisionError::Conflict`].
    fn insert_schedule_revision(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        row: &ScheduleRevisionRow,
    ) -> Result<ScheduleRevisionReceipt, ScheduleRevisionError>;

    /// The dedicated SUPERSEDED transition (§14.2; the ONLY permitted state
    /// change). Marks `revision` superseded with the forward link pointing
    /// at `superseded_by` (which must be exactly `revision + 1`, keeping the
    /// chain gapless in both directions). One-way: an already-superseded
    /// revision is [`ScheduleRevisionError::AlreadySuperseded`].
    fn mark_revision_superseded(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        schedule_id: &str,
        revision: u64,
        superseded_by: &ScheduleSupersedeTarget,
    ) -> Result<ScheduleRevisionReceipt, ScheduleRevisionError>;

    /// Looks up one revision (immutable content + state) in the caller's
    /// tenant scope. A foreign-tenant revision reads as `None`.
    fn find_revision(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        schedule_id: &str,
        revision: u64,
    ) -> Result<Option<ScheduleRevisionRecord>, ScheduleRevisionError>;

    /// The CURRENT (active) revision of the chain, if any. Superseded
    /// revisions remain queryable through [`Self::find_revision`] and
    /// [`Self::revision_chain`] (retained for audit, §14.2).
    fn current_revision(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        schedule_id: &str,
    ) -> Result<Option<ScheduleRevisionRecord>, ScheduleRevisionError>;

    /// The full revision chain of the schedule, ordered by revision — the
    /// audit view (every superseded revision still retrievable).
    fn revision_chain(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        schedule_id: &str,
    ) -> Result<Vec<ScheduleRevisionRecord>, ScheduleRevisionError>;

    /// The Phase 4 dispatch-gate check (cross-reference P4.12; NO command
    /// path here): `Some(true)` when the revision exists and is SUPERSEDED,
    /// `Some(false)` when it exists and is active, `None` when it does not
    /// exist in the caller's tenant scope. The Phase 4 gate rejects BOTH
    /// the superseded and the unknown case; `None` is never "usable".
    fn is_revision_superseded(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        schedule_id: &str,
        revision: u64,
    ) -> Result<Option<bool>, ScheduleRevisionError>;

    /// Lists revisions tenant-scoped (P3.28, added additively: the
    /// per-tenant listing read behind `GET /api/v1/schedules`). Keyset
    /// over `(schedule_id, revision)` ascending — the row's primary key
    /// (D-P3.22-1), a stable compound sort key — bounded by `limit` (the
    /// route layer owns the page-size limits), filtered by the optional
    /// portfolio filter, starting strictly after the optional
    /// `(schedule_id, revision)` cursor position. Revisions of other
    /// tenants never appear (RLS + explicit filter). No offset pagination.
    fn list_revisions(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        portfolio_id: Option<&str>,
        after_schedule_id: Option<&str>,
        after_revision: Option<u64>,
        limit: usize,
    ) -> Result<Vec<ScheduleRevisionRecord>, ScheduleRevisionError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tenant() -> TenantId {
        let mut bytes = [0u8; 16];
        bytes[15] = 2;
        TenantId::new(flexcore_contracts::ids::FlexUuid::from_bytes(bytes))
    }

    fn base_row(revision: u64) -> ScheduleRevisionRow {
        let (supersedes, supersedes_candidate) = if revision == 1 {
            (None, None)
        } else {
            (
                Some(revision - 1),
                Some(format!("0192a5e0-0000-7000-8000-{revision:012x}")),
            )
        };
        ScheduleRevisionRow::new(
            tenant(),
            "0192a5e0-0000-7000-8000-000000000001".to_string(),
            "0192a5e0-0000-7000-8000-000000000002".to_string(),
            "0192a5e0-0000-7000-8000-00000000000a".to_string(),
            revision,
            1_700_000_000_000,
            1_700_008_640_000,
            "initial_solve".to_string(),
            supersedes,
            supersedes_candidate,
            "0192a5e0-0000-7000-8000-000000000003".to_string(),
            "{\"intervals\":[]}".to_string(),
        )
        .expect("base row builds")
    }

    #[test]
    fn the_state_and_trigger_vocabularies_are_closed() {
        assert_eq!(SCHEDULE_REVISION_STATES, ["active", "superseded"]);
        assert_eq!(
            SCHEDULE_ROLL_TRIGGERS,
            [
                "initial_solve",
                "horizon_advance",
                "envelope_invalidation",
                "asset_opt_out_dropout",
                "forecast_update",
                "operator_request",
            ]
        );
    }

    #[test]
    fn a_revision_row_enforces_the_exact_chain_rule() {
        // Revision 1 without a link builds.
        let row = base_row(1);
        assert_eq!(row.revision, 1);
        assert_eq!(row.supersedes_revision, None);
        // Revision 3 claiming to supersede revision 1 is rejected.
        let broken = ScheduleRevisionRow::new(
            row.tenant_id,
            row.portfolio_id.clone(),
            row.site_id.clone(),
            row.schedule_id.clone(),
            3,
            row.horizon_start_millis,
            row.horizon_end_millis,
            row.roll_trigger.clone(),
            Some(1),
            Some("0192a5e0-0000-7000-8000-000000000009".to_string()),
            row.candidate_id.clone(),
            row.schedule_payload_json.clone(),
        );
        assert_eq!(
            broken,
            Err(ScheduleRevisionError::InvalidRevision { field: "supersedes_revision" })
        );
        // Revision 2 without a supersede link is rejected.
        let linkless = ScheduleRevisionRow::new(
            row.tenant_id,
            row.portfolio_id.clone(),
            row.site_id.clone(),
            row.schedule_id.clone(),
            2,
            row.horizon_start_millis,
            row.horizon_end_millis,
            row.roll_trigger.clone(),
            None,
            None,
            row.candidate_id.clone(),
            row.schedule_payload_json.clone(),
        );
        assert!(matches!(
            linkless,
            Err(ScheduleRevisionError::InvalidRevision { field: "supersedes_revision" })
        ));
    }

    #[test]
    fn malformed_revision_rows_are_rejected_with_the_field_named() {
        assert_eq!(
            ScheduleRevisionRow::new(
                tenant(),
                "p".to_string(),
                "s".to_string(),
                "sched".to_string(),
                0,
                0,
                1,
                "initial_solve".to_string(),
                None,
                None,
                "c".to_string(),
                "{}".to_string(),
            ),
            Err(ScheduleRevisionError::InvalidRevision { field: "revision" })
        );
        let row = base_row(1);
        assert_eq!(
            ScheduleRevisionRow::new(
                row.tenant_id,
                row.portfolio_id.clone(),
                row.site_id.clone(),
                row.schedule_id.clone(),
                1,
                row.horizon_start_millis,
                row.horizon_start_millis, // end == start is invalid
                row.roll_trigger.clone(),
                None,
                None,
                row.candidate_id.clone(),
                row.schedule_payload_json.clone(),
            ),
            Err(ScheduleRevisionError::InvalidRevision { field: "horizon_end_millis" })
        );
        assert_eq!(
            ScheduleRevisionRow::new(
                row.tenant_id,
                row.portfolio_id.clone(),
                row.site_id.clone(),
                row.schedule_id.clone(),
                1,
                row.horizon_start_millis,
                row.horizon_end_millis,
                "because".to_string(), // not in the closed vocabulary
                None,
                None,
                row.candidate_id.clone(),
                row.schedule_payload_json.clone(),
            ),
            Err(ScheduleRevisionError::InvalidRevision { field: "roll_trigger" })
        );
        assert_eq!(
            ScheduleRevisionRow::new(
                row.tenant_id,
                row.portfolio_id.clone(),
                row.site_id.clone(),
                row.schedule_id.clone(),
                1,
                row.horizon_start_millis,
                row.horizon_end_millis,
                row.roll_trigger.clone(),
                None,
                None,
                row.candidate_id.clone(),
                "  ".to_string(),
            ),
            Err(ScheduleRevisionError::InvalidRevision { field: "schedule_payload_json" })
        );
    }

    #[test]
    fn a_new_revision_is_active_and_supersede_state_is_consistent() {
        // The record constructor builds an active record with no forward link.
        let active = ScheduleRevisionRecord::new(
            base_row(1),
            SCHEDULE_REVISION_STATE_ACTIVE.to_string(),
            None,
            None,
        )
        .expect("active record builds");
        assert!(!active.is_superseded());

        // The superseded state requires BOTH forward-link fields.
        let superseded = ScheduleRevisionRecord::new(
            base_row(1),
            SCHEDULE_REVISION_STATE_SUPERSEDED.to_string(),
            Some(2),
            Some("0192a5e0-0000-7000-8000-000000000004".to_string()),
        )
        .expect("superseded record builds");
        assert!(superseded.is_superseded());

        assert_eq!(
            ScheduleRevisionRecord::new(
                base_row(1),
                SCHEDULE_REVISION_STATE_SUPERSEDED.to_string(),
                Some(1), // not strictly greater than the record's revision
                Some("0192a5e0-0000-7000-8000-000000000004".to_string()),
            ),
            Err(ScheduleRevisionError::InvalidRevision { field: "superseded_by_revision" })
        );
        assert_eq!(
            ScheduleRevisionRecord::new(
                base_row(1),
                SCHEDULE_REVISION_STATE_ACTIVE.to_string(),
                Some(2),
                None,
            ),
            Err(ScheduleRevisionError::InvalidRevision { field: "superseded_by_revision" })
        );
        assert_eq!(
            ScheduleRevisionRecord::new(
                base_row(1),
                "cancelled".to_string(), // not in the closed vocabulary
                None,
                None,
            ),
            Err(ScheduleRevisionError::InvalidRevision { field: "state" })
        );
    }

    #[test]
    fn the_supersede_target_and_receipt_carry_their_identities() {
        let target = ScheduleSupersedeTarget::new(
            2,
            "0192a5e0-0000-7000-8000-000000000004".to_string(),
        )
        .expect("target builds");
        assert_eq!(target.revision, 2);
        assert_eq!(
            ScheduleSupersedeTarget::new(0, "c".to_string()),
            Err(ScheduleRevisionError::InvalidRevision { field: "revision" })
        );
        assert_eq!(
            ScheduleSupersedeTarget::new(2, " ".to_string()),
            Err(ScheduleRevisionError::InvalidRevision { field: "candidate_id" })
        );

        let receipt = ScheduleRevisionReceipt::new("sched".to_string(), 2, tenant());
        assert_eq!(receipt.revision, 2);
        assert_eq!(receipt.tenant_id, tenant());
    }

    #[test]
    fn the_trait_is_object_safe_for_the_unit_of_work() {
        fn takes_dyn_store(_: &dyn ScheduleRevisionChainStore) {}
        let _ = takes_dyn_store as fn(&dyn ScheduleRevisionChainStore);
    }
}
