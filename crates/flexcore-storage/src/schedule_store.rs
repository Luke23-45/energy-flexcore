// P3.21 — ScheduleRecordStore: the persistence contract for §12.6
//         schedule-result records (report §12.6, §16.1, §15.5, §16.2;
//         ledger P3.21 step 3).
//
// "Persist the record in PostgreSQL before any downstream use" (§16.1), made
// concrete at the storage-crate level: a store trait + the row/receipt/error
// types, mirroring how this crate holds [`crate::unit_of_work::UnitOfWork`]
// and [`crate::outbox_write::TransactionalWrite`] — a vocabulary the adapter
// must honour, with the mechanical enforcement in the adapter's tests.
//
// ## Recorded home decision (deviation, honestly stated)
//
// The P1.10 single-home rule puts repository-abstraction traits in
// flexcore-ports, NOT here. The P3.21 subtask's file ownership granted the
// schedule-record store to flexcore-storage (the ports crate was frozen for
// the batch), so the trait lives HERE. It is deliberately shaped as a
// **record store**, not a Stage 1 aggregate repository: the §12.6 result
// record is an immutable, append-oriented computation artifact (insert +
// tenant-scoped read; no update, no delete), which is a different lifecycle
// from the registry aggregates. If a later subtask relocates the trait to
// flexcore-ports, the trait surface is designed to move verbatim. Full
// rationale: `ledger/phase-03-flexibility-scheduling/21_result_contract.md`
// (D-P3.21-4).
//
// ## What this module can and cannot do
//
// - It CAN make incomplete records unrepresentable: [`ScheduleRecordRow`]
//   has exactly one constructor, which validates every required field
//   (status inside the closed §12.6 vocabulary, non-empty identification,
//   objective completeness for `optimal`/`feasible`, grid-model
//   traceability present) — a half-filled record cannot reach an adapter.
// - It CANNOT enforce atomicity or tenant scoping. Those are adapter
//   obligations, stated on the trait (binding, not advisory) and enforced
//   by the adapter's tests (the P1.13 atomicity pattern; RLS per the
//   P1.12 §5.1 conventions).
//
// ## Adapter obligations (binding)
//
// 1. `insert_schedule_record` executes inside the CALLER's unit of work
//    ([`crate::unit_of_work::UnitOfWork`]) — the same native transaction
//    that carries the paired `ScheduleProposed` outbox row (report §15.5;
//    the outbox row's idempotency key is recorded on the row itself as
//    [`ScheduleRecordRow::schedule_proposed_key`], so state and event are
//    provably paired). An adapter that opens its own transaction for the
//    insert creates the dual-write hole §15.5 forbids.
// 2. Every query is tenant-scoped: RLS via the transaction-scoped
//    `flexcore.tenant_id` setting (P1.12 §5.1) PLUS an explicit
//    `tenant_id` filter; a record of another tenant reads as `None` —
//    existence never leaks across a tenant boundary (P0.16 rule 5).
// 3. Storage failures surface as [`ScheduleStoreError::Backend`] with an
//    opaque code + message only (never raw SQL or driver text).
// 4. The records are immutable: adapters grant INSERT and SELECT only;
//    revisions and supersede state are new records (P3.22 owns revision
//    identity), never in-place mutation.
//
// ## Row shape
//
// Structured columns carry every field a reader filters or audits on
// (status, problem class, solver/provider identity, objective, snapshot and
// grid-model traceability). The full §12.6 record travels as
// [`ScheduleRecordRow::record_payload_json`] — opaque JSON text at this
// layer (this crate imports no serialization library; the composition root
// serializes, the adapter validates it parses as JSON on insert).

use flexcore_domain::ids::TenantId;

/// The closed §12.6 status vocabulary, as stored text. Exactly the six
/// report strings (the vocabulary's single home is
/// `flexcore-optimization-api::result::SolveStatus`; this constant is the
/// stored-text form used for row validation so the storage layer does not
/// depend on the optimization API crate).
pub const SCHEDULE_RESULT_STATUSES: [&str; 6] = [
    "optimal",
    "feasible",
    "infeasible",
    "timeout",
    "numerical_failure",
    "unsupported",
];

/// The statuses that must carry a complete objective (value + units) on a
/// stored row — the storage-level mirror of the port contract's rule for
/// `optimal`/`feasible` results.
const OBJECTIVE_REQUIRED_STATUSES: [&str; 2] = ["optimal", "feasible"];

/// Errors of the schedule-record store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleStoreError {
    /// The row is incomplete or malformed (`field` names the problem). The
    /// record was NOT written.
    InvalidRecord {
        field: &'static str,
    },
    /// A record with the same candidate identity already exists. The
    /// record was NOT written — schedule records are immutable, and a
    /// re-solve is a new candidate (P3.22), never an overwrite.
    Conflict {
        candidate_id: String,
    },
    /// The storage backend failed. `code` and `message` are opaque and
    /// human-safe — never raw storage text.
    Backend {
        code: String,
        message: String,
    },
}

impl std::fmt::Display for ScheduleStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScheduleStoreError::InvalidRecord { field } => {
                write!(f, "the schedule-result record is invalid: '{field}' is missing or malformed")
            }
            ScheduleStoreError::Conflict { candidate_id } => write!(
                f,
                "a schedule-result record for candidate {candidate_id} already exists; \
                 records are immutable and a re-solve is a new candidate"
            ),
            ScheduleStoreError::Backend { code, message } => {
                write!(f, "storage backend failure ({code}): {message}")
            }
        }
    }
}

impl std::error::Error for ScheduleStoreError {}

/// One persisted §12.6 schedule-result record (the insert/read row).
///
/// Construction goes exclusively through [`ScheduleRecordRow::new`], which
/// validates the storage-level completeness rules; the struct fields are
/// then readable for adapters. Identifier fields are the canonical
/// hyphenated UUID text forms (the recorded FlexUuid ↔ PG uuid mapping
/// binds text and casts `::uuid` in SQL).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleRecordRow {
    /// The owning tenant (P0.16 rule 1 — `tenant_id` on every tenant-owned
    /// record, present on the row itself so RLS and the explicit filter
    /// agree).
    pub tenant_id: TenantId,
    /// The owning portfolio (canonical UUID text).
    pub portfolio_id: String,
    /// The site the schedule was computed for (canonical UUID text).
    pub site_id: String,
    /// The candidate identity (§12.6 "output candidate ID"; canonical UUID
    /// text; the record's primary identity).
    pub candidate_id: String,
    /// The solver outcome status — one of [`SCHEDULE_RESULT_STATUSES`].
    pub status: String,
    /// The declared problem class (stable short identifier, e.g. `lp`,
    /// `socp`).
    pub problem_class: String,
    /// Solver name and pinned version.
    pub solver_name: String,
    pub solver_version: String,
    /// Provider identifier and version.
    pub provider_id: String,
    pub provider_version: String,
    /// Objective value, when a solution was produced.
    pub objective_value: Option<f64>,
    /// Objective units, present whenever `objective_value` is (enforced
    /// for `optimal`/`feasible` rows by [`ScheduleRecordRow::new`]).
    pub objective_units: Option<String>,
    /// The input-snapshot reference the result cites (non-empty).
    pub input_snapshot_id: String,
    /// The grid-model version (non-empty; traceable to the site's Phase 2
    /// reference per [`Self::grid_model_site_reference`]).
    pub grid_model_version: String,
    /// The site whose Phase 2 grid-model reference supplied the version
    /// (canonical UUID text; ledger P3.21 step 2 traceability).
    pub grid_model_site_id: String,
    /// The verbatim site grid-model reference (P2.02).
    pub grid_model_site_reference: String,
    /// The deterministic `ScheduleProposed` idempotency key the record was
    /// persisted with — the pairing proof that the outbox row and this
    /// state row belong to one transaction (report §15.5).
    pub schedule_proposed_key: String,
    /// The full §12.6 record as JSON text — opaque to this crate (no
    /// serialization dependency here); the adapter validates it parses as
    /// JSON on insert. Carries the diagnostics, warnings, assumptions, and
    /// determinism fields verbatim.
    pub record_payload_json: String,
}

impl ScheduleRecordRow {
    /// Builds and validates one row. Storage-level rules: every
    /// identification and traceability field non-empty; `status` inside
    /// the closed §12.6 vocabulary; `optimal`/`feasible` rows carry BOTH
    /// the objective value and non-empty units; the payload JSON is
    /// non-empty.
    pub fn new(
        tenant_id: TenantId,
        portfolio_id: String,
        site_id: String,
        candidate_id: String,
        status: String,
        problem_class: String,
        solver_name: String,
        solver_version: String,
        provider_id: String,
        provider_version: String,
        objective_value: Option<f64>,
        objective_units: Option<String>,
        input_snapshot_id: String,
        grid_model_version: String,
        grid_model_site_id: String,
        grid_model_site_reference: String,
        schedule_proposed_key: String,
        record_payload_json: String,
    ) -> Result<Self, ScheduleStoreError> {
        for (field, value) in [
            ("portfolio_id", portfolio_id.as_str()),
            ("site_id", site_id.as_str()),
            ("candidate_id", candidate_id.as_str()),
            ("problem_class", problem_class.as_str()),
            ("solver_name", solver_name.as_str()),
            ("solver_version", solver_version.as_str()),
            ("provider_id", provider_id.as_str()),
            ("provider_version", provider_version.as_str()),
            ("input_snapshot_id", input_snapshot_id.as_str()),
            ("grid_model_version", grid_model_version.as_str()),
            ("grid_model_site_id", grid_model_site_id.as_str()),
            ("grid_model_site_reference", grid_model_site_reference.as_str()),
            ("schedule_proposed_key", schedule_proposed_key.as_str()),
            ("record_payload_json", record_payload_json.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(ScheduleStoreError::InvalidRecord { field });
            }
        }
        if !SCHEDULE_RESULT_STATUSES.contains(&status.as_str()) {
            return Err(ScheduleStoreError::InvalidRecord { field: "status" });
        }
        if OBJECTIVE_REQUIRED_STATUSES.contains(&status.as_str()) {
            match (&objective_value, objective_units.as_deref()) {
                (Some(_), Some(units)) if !units.trim().is_empty() => {}
                _ => {
                    return Err(ScheduleStoreError::InvalidRecord {
                        field: "objective_value/objective_units",
                    });
                }
            }
        }
        Ok(Self {
            tenant_id,
            portfolio_id,
            site_id,
            candidate_id,
            status,
            problem_class,
            solver_name,
            solver_version,
            provider_id,
            provider_version,
            objective_value,
            objective_units,
            input_snapshot_id,
            grid_model_version,
            grid_model_site_id,
            grid_model_site_reference,
            schedule_proposed_key,
            record_payload_json,
        })
    }
}

/// The receipt of one persisted schedule-result record. Its existence
/// proves the insert was ISSUED (the same receipt convention as the
/// repository ports' `StateChangeReceipt`); the composition root converts
/// it into the application's persistence proof before any
/// `ScheduleProposed` assembly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleRecordReceipt {
    /// The persisted record's candidate identity (canonical UUID text).
    pub candidate_id: String,
    /// The owning tenant.
    pub tenant_id: TenantId,
}

impl ScheduleRecordReceipt {
    /// Builds the receipt (adapter-side; both fields are taken as given
    /// from the executed insert).
    pub fn new(candidate_id: String, tenant_id: TenantId) -> Self {
        Self {
            candidate_id,
            tenant_id,
        }
    }
}

/// The schedule-record persistence contract (§16.1: PostgreSQL is the
/// primary transactional source of truth for schedules). One trait, two
/// operations — an append-oriented record store, not a CRUD repository:
/// records are immutable (see the adapter obligations in the module header).
///
/// ## Home decision (recorded deviation from the P1.10 single-home rule)
///
/// See the module header: the trait lives in flexcore-storage by the P3.21
/// subtask's recorded file ownership (D-P3.21-4); it is shaped to move to
/// flexcore-ports verbatim if a later subtask relocates it.
pub trait ScheduleRecordStore {
    /// Inserts one §12.6 schedule-result record inside the CALLER's unit of
    /// work (adapter obligation 1: the same native transaction carries the
    /// paired `ScheduleProposed` outbox row). Conflicts on the candidate
    /// identity ([`ScheduleStoreError::Conflict`] — records are immutable);
    /// storage failures are [`ScheduleStoreError::Backend`] with an opaque
    /// code + message.
    fn insert_schedule_record(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        row: &ScheduleRecordRow,
    ) -> Result<ScheduleRecordReceipt, ScheduleStoreError>;

    /// Looks a record up by candidate identity within the caller's tenant
    /// scope. A record of another tenant reads as `None` (no existence
    /// leak, P0.16 rule 5).
    fn find_schedule_record(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        candidate_id: &str,
    ) -> Result<Option<ScheduleRecordRow>, ScheduleStoreError>;

    /// Lists records tenant-scoped (P3.28, added additively: the
    /// per-tenant listing read behind `GET /api/v1/optimization-runs`).
    /// Keyset over `candidate_id` ascending (a UUIDv7, so identifier order
    /// is a stable time order — the recorded P2.12 rationale), bounded by
    /// `limit` (the route layer owns the page-size limits), filtered by
    /// the optional portfolio/site/status filters. Records of other
    /// tenants never appear (RLS + explicit filter). No offset pagination.
    fn list_schedule_records(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        portfolio_id: Option<&str>,
        site_id: Option<&str>,
        status: Option<&str>,
        after_candidate_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ScheduleRecordRow>, ScheduleStoreError>;
}

// --- P3.23 — Schedule-comparison record storage (appended additively) --------

/// The comparison-record storage statement (ledger P3.23 steps 1–2): a
/// comparison is taken over a FIXED modeling-input snapshot only — same
/// envelope version, same forecast version (+ provenance), same objective
/// configuration — with ONLY the formulation/solver tier varying. The
/// comparability gate itself lives in the application layer
/// (flexcore-application `schedule_comparison`); this store persists the
/// resulting record. The persisted row records the shared modeling inputs
/// as structured columns (filterable/auditable) and the per-candidate
/// entries — including the four §19.4 scheduling metrics per candidate —
/// as opaque JSON validated by the adapter.
pub const SCHEDULE_COMPARISON_STATEMENT: &str = "A schedule comparison record \
     shares ONE modeling-input snapshot (envelope version, forecast version + \
     provenance, objective configuration, telemetry reference) across all \
     candidates; only the formulation/solver tier varies. Cross-input \
     comparisons are REJECTED by the application gate (D-P3.23-2). Both \
     compared sides are CANDIDATES; the comparison is diagnostic and never a \
     feasibility verdict (Phase 4 owns that gate).";

/// Errors of the comparison-record store. Separate from
/// [`ScheduleStoreError`] so the existing error surface is unchanged
/// (additive extension only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComparisonStoreError {
    /// The row is incomplete or malformed (`field` names the problem). The
    /// record was NOT written.
    InvalidRecord {
        field: &'static str,
    },
    /// A comparison with the same identity already exists — comparison
    /// records are immutable.
    Conflict {
        comparison_id: String,
    },
    /// The storage backend failed (opaque code + message only).
    Backend {
        code: String,
        message: String,
    },
}

impl std::fmt::Display for ComparisonStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComparisonStoreError::InvalidRecord { field } => {
                write!(f, "the schedule-comparison record is invalid: '{field}' is missing or malformed")
            }
            ComparisonStoreError::Conflict { comparison_id } => write!(
                f,
                "a schedule-comparison record {comparison_id} already exists; \
                 comparison records are immutable"
            ),
            ComparisonStoreError::Backend { code, message } => {
                write!(f, "storage backend failure ({code}): {message}")
            }
        }
    }
}

impl std::error::Error for ComparisonStoreError {}

/// One persisted schedule-comparison record (the insert/read row;
/// ledger P3.23 step 1). Construction goes exclusively through
/// [`ScheduleComparisonRow::new`]. The full record — the per-candidate
/// entries with their §19.4 scheduling metrics (objective value, schedule
/// deviation, feasible interval count, runtime) and the solver/tier
/// identities — travels as [`ScheduleComparisonRow::comparison_payload_json`],
/// opaque to this crate (the adapter validates it parses as JSON).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleComparisonRow {
    /// The owning tenant (P0.16 rule 1).
    pub tenant_id: TenantId,
    /// The comparison identity (canonical UUID text; the record's primary
    /// identity).
    pub comparison_id: String,
    /// The owning portfolio (canonical UUID text).
    pub portfolio_id: String,
    /// The site the compared schedules were computed for (canonical UUID
    /// text).
    pub site_id: String,
    /// The shared modeling-input snapshot reference — the SAME snapshot all
    /// candidates were computed against (non-empty; equality enforced by the
    /// application gate, recorded here for audit).
    pub shared_input_snapshot_id: String,
    /// The shared envelope version (P3.12/P3.13).
    pub shared_envelope_version: String,
    /// The shared forecast version (P3.31).
    pub shared_forecast_version: String,
    /// The shared forecast provenance label (always `synthetic` in v0.2.0).
    pub shared_forecast_provenance: String,
    /// The shared objective name (P3.14 — the same objective config for
    /// every candidate).
    pub shared_objective_name: String,
    /// The number of compared candidates (at least 2 — a comparison over
    /// fewer is not a comparison; mirrored by the database CHECK).
    pub candidate_count: u32,
    /// The full comparison record as JSON text — the per-candidate entries
    /// (candidate ids, tier/solver identities, §12.6 status, and the four
    /// §19.4 metrics). Opaque at this layer; adapter-validated JSON.
    pub comparison_payload_json: String,
}

impl ScheduleComparisonRow {
    /// Builds and validates one comparison row. Storage-level rules: every
    /// field non-empty; at least two candidates; non-empty payload.
    pub fn new(
        tenant_id: TenantId,
        comparison_id: String,
        portfolio_id: String,
        site_id: String,
        shared_input_snapshot_id: String,
        shared_envelope_version: String,
        shared_forecast_version: String,
        shared_forecast_provenance: String,
        shared_objective_name: String,
        candidate_count: u32,
        comparison_payload_json: String,
    ) -> Result<Self, ComparisonStoreError> {
        for (field, value) in [
            ("comparison_id", comparison_id.as_str()),
            ("portfolio_id", portfolio_id.as_str()),
            ("site_id", site_id.as_str()),
            ("shared_input_snapshot_id", shared_input_snapshot_id.as_str()),
            ("shared_envelope_version", shared_envelope_version.as_str()),
            ("shared_forecast_version", shared_forecast_version.as_str()),
            ("shared_forecast_provenance", shared_forecast_provenance.as_str()),
            ("shared_objective_name", shared_objective_name.as_str()),
            ("comparison_payload_json", comparison_payload_json.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(ComparisonStoreError::InvalidRecord { field });
            }
        }
        if candidate_count < 2 {
            return Err(ComparisonStoreError::InvalidRecord { field: "candidate_count" });
        }
        Ok(Self {
            tenant_id,
            comparison_id,
            portfolio_id,
            site_id,
            shared_input_snapshot_id,
            shared_envelope_version,
            shared_forecast_version,
            shared_forecast_provenance,
            shared_objective_name,
            candidate_count,
            comparison_payload_json,
        })
    }
}

/// The receipt of one persisted comparison record (the receipt convention of
/// this crate).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleComparisonReceipt {
    /// The persisted comparison's identity (canonical UUID text).
    pub comparison_id: String,
    /// The owning tenant.
    pub tenant_id: TenantId,
}

impl ScheduleComparisonReceipt {
    /// Builds the receipt (adapter-side).
    pub fn new(comparison_id: String, tenant_id: TenantId) -> Self {
        Self {
            comparison_id,
            tenant_id,
        }
    }
}

/// The comparison-record persistence contract (ledger P3.23; §16.1).
/// Immutable, append-oriented: insert + tenant-scoped find, no update, no
/// delete. Same adapter obligations as [`ScheduleRecordStore`]: the insert
/// runs inside the CALLER's unit of work, every query is tenant-scoped
/// (RLS + explicit filter), and failures surface as
/// [`ComparisonStoreError::Backend`] with an opaque code + message.
pub trait ScheduleComparisonStore {
    /// Inserts one comparison record inside the CALLER's unit of work. A
    /// comparison-identity collision is [`ComparisonStoreError::Conflict`].
    fn insert_schedule_comparison(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        row: &ScheduleComparisonRow,
    ) -> Result<ScheduleComparisonReceipt, ComparisonStoreError>;

    /// Looks a comparison record up by identity within the caller's tenant
    /// scope. A foreign-tenant record reads as `None` (P0.16 rule 5).
    fn find_schedule_comparison(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        comparison_id: &str,
    ) -> Result<Option<ScheduleComparisonRow>, ComparisonStoreError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;

    fn tenant() -> TenantId {
        let mut bytes = [0u8; 16];
        bytes[15] = 1;
        TenantId::new(FlexUuid::from_bytes(bytes))
    }

    fn complete_row_fields() -> (TenantId, String, String, String, String, String, String, String, String, String, Option<f64>, Option<String>, String, String, String, String, String, String) {
        (
            tenant(),
            "0192a5e0-0000-7000-8000-000000000001".to_string(),
            "0192a5e0-0000-7000-8000-000000000002".to_string(),
            "0192a5e0-0000-7000-8000-000000000003".to_string(),
            "optimal".to_string(),
            "lp".to_string(),
            "highs".to_string(),
            "1.15.1".to_string(),
            "tier1-python-worker".to_string(),
            "0.1.0".to_string(),
            Some(12.5),
            Some("currency-units".to_string()),
            "schedule_input_snapshot:env-v1:v1:synthetic:fnv1a64:0000000000000000".to_string(),
            "grid-model-v1".to_string(),
            "0192a5e0-0000-7000-8000-000000000002".to_string(),
            "site-grid-ref-1".to_string(),
            "schedule_proposed:0192a5e0-0000-7000-8000-000000000001:0192a5e0-0000-7000-8000-000000000002:0192a5e0-0000-7000-8000-000000000003".to_string(),
            "{\"status\":\"optimal\"}".to_string(),
        )
    }

    fn row_from(
        parts: (TenantId, String, String, String, String, String, String, String, String, String, Option<f64>, Option<String>, String, String, String, String, String, String),
    ) -> ScheduleRecordRow {
        ScheduleRecordRow::new(
            parts.0, parts.1, parts.2, parts.3, parts.4, parts.5, parts.6, parts.7, parts.8,
            parts.9, parts.10, parts.11, parts.12, parts.13, parts.14, parts.15, parts.16,
            parts.17,
        )
        .expect("complete fields build")
    }

    #[test]
    fn the_status_vocabulary_is_exactly_the_six_strings() {
        assert_eq!(
            SCHEDULE_RESULT_STATUSES,
            ["optimal", "feasible", "infeasible", "timeout", "numerical_failure", "unsupported"]
        );
    }

    #[test]
    fn a_complete_row_builds_and_carries_every_field() {
        let row = row_from(complete_row_fields());
        assert_eq!(row.status, "optimal");
        assert_eq!(row.objective_value, Some(12.5));
        assert_eq!(row.grid_model_site_reference, "site-grid-ref-1");
        assert!(row
            .schedule_proposed_key
            .starts_with("schedule_proposed:"));
    }

    #[test]
    fn an_unknown_status_is_rejected() {
        let mut parts = complete_row_fields();
        parts.4 = "unbounded".to_string();
        assert_eq!(
            ScheduleRecordRow::new(parts.0, parts.1, parts.2, parts.3, parts.4, parts.5, parts.6, parts.7, parts.8, parts.9, parts.10, parts.11, parts.12, parts.13, parts.14, parts.15, parts.16, parts.17),
            Err(ScheduleStoreError::InvalidRecord { field: "status" })
        );
    }

    #[test]
    fn an_optimal_row_without_a_complete_objective_is_rejected() {
        // Value without units.
        let mut parts = complete_row_fields();
        parts.11 = None;
        assert!(ScheduleRecordRow::new(parts.0, parts.1, parts.2, parts.3, parts.4, parts.5, parts.6, parts.7, parts.8, parts.9, parts.10, parts.11, parts.12, parts.13, parts.14, parts.15, parts.16, parts.17).is_err());
        // Blank units.
        let mut parts = complete_row_fields();
        parts.11 = Some("   ".to_string());
        assert!(ScheduleRecordRow::new(parts.0, parts.1, parts.2, parts.3, parts.4, parts.5, parts.6, parts.7, parts.8, parts.9, parts.10, parts.11, parts.12, parts.13, parts.14, parts.15, parts.16, parts.17).is_err());
    }

    #[test]
    fn a_non_solving_status_may_omit_the_objective() {
        let mut parts = complete_row_fields();
        parts.4 = "unsupported".to_string();
        parts.10 = None;
        parts.11 = None;
        let row = ScheduleRecordRow::new(parts.0, parts.1, parts.2, parts.3, parts.4, parts.5, parts.6, parts.7, parts.8, parts.9, parts.10, parts.11, parts.12, parts.13, parts.14, parts.15, parts.16, parts.17)
            .expect("an unsupported row without an objective records");
        assert_eq!(row.status, "unsupported");
        assert_eq!(row.objective_value, None);
    }

    #[test]
    fn empty_required_fields_are_rejected_with_the_field_named() {
        for blanked in [
            "portfolio_id", "site_id", "candidate_id", "problem_class", "solver_name",
            "solver_version", "provider_id", "provider_version", "input_snapshot_id",
            "grid_model_version", "grid_model_site_id", "grid_model_site_reference",
            "schedule_proposed_key", "record_payload_json",
        ] {
            let mut parts = complete_row_fields();
            match blanked {
                "portfolio_id" => parts.1 = String::new(),
                "site_id" => parts.2 = String::new(),
                "candidate_id" => parts.3 = String::new(),
                "problem_class" => parts.5 = String::new(),
                "solver_name" => parts.6 = String::new(),
                "solver_version" => parts.7 = String::new(),
                "provider_id" => parts.8 = String::new(),
                "provider_version" => parts.9 = String::new(),
                "input_snapshot_id" => parts.12 = String::new(),
                "grid_model_version" => parts.13 = String::new(),
                "grid_model_site_id" => parts.14 = String::new(),
                "grid_model_site_reference" => parts.15 = String::new(),
                "schedule_proposed_key" => parts.16 = String::new(),
                "record_payload_json" => parts.17 = String::new(),
                _ => unreachable!(),
            }
            let error = ScheduleRecordRow::new(parts.0, parts.1, parts.2, parts.3, parts.4, parts.5, parts.6, parts.7, parts.8, parts.9, parts.10, parts.11, parts.12, parts.13, parts.14, parts.15, parts.16, parts.17)
                .expect_err("a blanked required field must fail");
            assert_eq!(
                error,
                ScheduleStoreError::InvalidRecord { field: blanked },
                "blanking {blanked} must name the field"
            );
        }
    }

    #[test]
    fn the_receipt_carries_the_record_identity() {
        let receipt = ScheduleRecordReceipt::new("cand-1".to_string(), tenant());
        assert_eq!(receipt.candidate_id, "cand-1");
        assert_eq!(receipt.tenant_id, tenant());
    }

    // --- P3.23 comparison-row constructor (additive) -------------------------

    fn comparison_row_fields() -> (TenantId, String, String, String, String, String, String, String, String, u32, String) {
        (
            tenant(),
            "0192a5e0-0000-7000-8000-0000000000c0".to_string(),
            "0192a5e0-0000-7000-8000-000000000001".to_string(),
            "0192a5e0-0000-7000-8000-000000000002".to_string(),
            "schedule_input_snapshot:env-v1:v1:synthetic:telemetry=t1".to_string(),
            "env-v1".to_string(),
            "v1".to_string(),
            "synthetic".to_string(),
            "default-synthetic-v1".to_string(),
            2,
            "[{\"candidate_id\":\"c1\"},{\"candidate_id\":\"c2\"}]".to_string(),
        )
    }

    #[test]
    fn a_complete_comparison_row_builds_and_carries_every_field() {
        let p = comparison_row_fields();
        let row = ScheduleComparisonRow::new(
            p.0, p.1.clone(), p.2, p.3, p.4, p.5, p.6, p.7, p.8, p.9, p.10,
        )
        .expect("complete comparison row builds");
        assert_eq!(row.comparison_id, p.1);
        assert_eq!(row.candidate_count, 2);
        assert_eq!(row.shared_forecast_provenance, "synthetic");
    }

    #[test]
    fn a_comparison_row_with_fewer_than_two_candidates_is_rejected() {
        let mut p = comparison_row_fields();
        p.9 = 1;
        assert_eq!(
            ScheduleComparisonRow::new(p.0, p.1, p.2, p.3, p.4, p.5, p.6, p.7, p.8, p.9, p.10),
            Err(ComparisonStoreError::InvalidRecord { field: "candidate_count" })
        );
    }

    #[test]
    fn empty_comparison_required_fields_are_rejected_with_the_field_named() {
        for blanked in [
            "comparison_id",
            "portfolio_id",
            "site_id",
            "shared_input_snapshot_id",
            "shared_envelope_version",
            "shared_forecast_version",
            "shared_forecast_provenance",
            "shared_objective_name",
            "comparison_payload_json",
        ] {
            let mut p = comparison_row_fields();
            match blanked {
                "comparison_id" => p.1 = String::new(),
                "portfolio_id" => p.2 = String::new(),
                "site_id" => p.3 = String::new(),
                "shared_input_snapshot_id" => p.4 = String::new(),
                "shared_envelope_version" => p.5 = String::new(),
                "shared_forecast_version" => p.6 = String::new(),
                "shared_forecast_provenance" => p.7 = String::new(),
                "shared_objective_name" => p.8 = String::new(),
                "comparison_payload_json" => p.10 = "  ".to_string(),
                _ => unreachable!(),
            }
            assert_eq!(
                ScheduleComparisonRow::new(p.0, p.1, p.2, p.3, p.4, p.5, p.6, p.7, p.8, p.9, p.10),
                Err(ComparisonStoreError::InvalidRecord { field: blanked }),
                "blanking {blanked} must name the field"
            );
        }
    }

    #[test]
    fn the_comparison_receipt_carries_the_record_identity() {
        let receipt = ScheduleComparisonReceipt::new("cmp-1".to_string(), tenant());
        assert_eq!(receipt.comparison_id, "cmp-1");
        assert_eq!(receipt.tenant_id, tenant());
    }
}
