// P4.30 — Feasibility-check evidence record persistence contract (report
//         §15.2–§15.3, §16.1–§16.2, §21.3; ledger P4.30; decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/30_persistence_wiring.md`).
//
// The append-only persistence contract for the §12.5 feasibility-check
// evidence: ONE immutable row per completed check — the command/schedule/
// scope identity, the model id/version/hash the check evaluated against,
// the verdict (`PASSED` | `LIMIT_VIOLATED` | `FAIL_CLOSED`), the failing
// constraint or fail-closed reason, the margins/coverage summaries (§12.5
// steps 6–7), the acting identity, and the correlation (trace) id (§21.3).
//
// What this table is for:
//
// - The §15.2 `/api/v1/feasibility-checks` READ resource (this record is
//   what the resource serves; the resource is read-only — no execute-
//   anything endpoint exists, §15.2);
// - the evidence behind the frozen §15.3 `FeasibilityCheckCompleted` event
//   (the payload carries identity + time only — the recorded payload-fit
//   convention; the record carries the verdict detail);
// - reconstruction (§21.3): every check names the command, the schedule
//   chain + revision, and THE MODEL it was checked against.
//
// The row mirrors migration 0017's `feasibility_check_records` exactly
// (the command records and the transition journal — which carry the
// validation/feasibility/authorization EVIDENCE REFERENCES on every
// journal row — persist since migration 0016 / the P4.10
// `CommandRecordStore`; this contract does not recreate them). The row is
// append-only: INSERT + SELECT only, no update, no delete (migration 0017
// refuses both by grant and by trigger).
//
// Control status: persistence bookkeeping for the simulation-only 0.x
// line. A stored check record is evidence of a simulated/validated gate
// run — never a permission to issue, and never a dispatch (§14.5).

use flexcore_domain::ids::TenantId;

/// The closed outcome vocabulary of a completed feasibility check, as
/// stored text — the three §12.5/§14.1 verdict arms the command gate
/// consumes (`FeasibilityGateVerdict`), in gate order.
pub const FEASIBILITY_CHECK_OUTCOMES: [&str; 3] = ["PASSED", "LIMIT_VIOLATED", "FAIL_CLOSED"];

/// The stored text of the pass outcome.
pub const FEASIBILITY_OUTCOME_PASSED: &str = "PASSED";
/// The stored text of the limit-violation outcome.
pub const FEASIBILITY_OUTCOME_LIMIT_VIOLATED: &str = "LIMIT_VIOLATED";
/// The stored text of the fail-closed outcome (§12.5 closing rule: a
/// validator/model/infrastructure failure is never a pass).
pub const FEASIBILITY_OUTCOME_FAIL_CLOSED: &str = "FAIL_CLOSED";

// --- Errors -------------------------------------------------------------------

/// Errors of the feasibility-check record store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeasibilityCheckStoreError {
    /// The row argument is incomplete or malformed (`field` names the
    /// problem). Nothing was written.
    InvalidRecord {
        field: &'static str,
    },
    /// A check with the same (check_id, tenant_id) already exists — check
    /// rows are immutable; a re-checked candidate is a NEW check id.
    Conflict {
        check_id: String,
    },
    /// The check does not exist in the caller's tenant scope (existence
    /// never leaks across a tenant boundary, P0.16 rule 5).
    NotFound {
        check_id: String,
    },
    /// The storage backend failed. `code` and `message` are opaque and
    /// human-safe — never raw storage text.
    Backend {
        code: String,
        message: String,
    },
}

impl std::fmt::Display for FeasibilityCheckStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeasibilityCheckStoreError::InvalidRecord { field } => {
                write!(f, "the feasibility-check record is invalid: '{field}' is missing or malformed")
            }
            FeasibilityCheckStoreError::Conflict { check_id } => write!(
                f,
                "a feasibility check {check_id} already exists; check records are immutable \
                 and a re-checked candidate is a NEW check id"
            ),
            FeasibilityCheckStoreError::NotFound { check_id } => write!(
                f,
                "feasibility check {check_id} does not exist in this tenant scope"
            ),
            FeasibilityCheckStoreError::Backend { code, message } => {
                write!(f, "storage backend failure ({code}): {message}")
            }
        }
    }
}

impl std::error::Error for FeasibilityCheckStoreError {}

// --- Rows ---------------------------------------------------------------------

/// One completed feasibility-check record (the write AND read shape — the
/// row is immutable, so one type serves both). Construction goes
/// exclusively through [`FeasibilityCheckRow::new`], which enforces the
/// closed outcome vocabulary and the migration 0017 verdict-shape rules
/// BEFORE anything is written (the database CHECKs are defence in depth).
///
/// Identifier fields are canonical hyphenated UUID text (the recorded
/// FlexUuid ↔ PG uuid mapping) — except `check_id`, which is the attempt
/// id or the pipeline-level `gate:<candidate>` id (the recorded
/// `FeasibilityCheckCompleted` check-id convention; TEXT, not a UUID).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeasibilityCheckRow {
    /// The owning tenant (P0.16 rule 1 — `tenant_id` on every tenant-owned
    /// record, present on the row itself so RLS and the explicit filter
    /// agree).
    pub tenant_id: TenantId,
    /// The check identity (the §12.5 attempt id or the pipeline-level
    /// `gate:<candidate>` id; non-empty text).
    pub check_id: String,
    /// The command the check ran for, when one exists (canonical UUID
    /// text; `None` for a candidate check that precedes command creation).
    pub command_id: Option<String>,
    /// The owning portfolio (canonical UUID text).
    pub portfolio_id: String,
    /// The target site (canonical UUID text).
    pub site_id: String,
    /// The target asset (canonical UUID text).
    pub asset_id: String,
    /// The source schedule chain (canonical UUID text; P3.22).
    pub schedule_id: String,
    /// The source schedule revision (≥ 1; P3.22).
    pub schedule_revision: u64,
    /// The model identifier the check evaluated against (§12.6
    /// traceability), when the verdict carries one.
    pub model_id: Option<String>,
    /// The model version string, when the composition root records one
    /// (§21.3 correlation; the hash below is the binding evidence).
    pub model_version: Option<String>,
    /// THE MODEL HASH the evidence binds to (P4.12: a model change
    /// invalidates the evidence; §17.5 invariant 3). Required exactly for
    /// a PASSED check.
    pub model_content_hash: Option<String>,
    /// The verdict — one of [`FEASIBILITY_CHECK_OUTCOMES`].
    pub outcome: String,
    /// The violated network constraint (§14.1 edge 4 naming), present
    /// exactly for `LIMIT_VIOLATED`.
    pub failing_constraint: Option<String>,
    /// The fail-closed reason (§12.5 closing rule), present exactly for
    /// `FAIL_CLOSED`.
    pub reason: Option<String>,
    /// The margin/diagnostics summary (§12.5 step 7), when produced.
    pub margins: Option<String>,
    /// The coverage statement/limitations (§12.5 step 6), when produced.
    pub coverage: Option<String>,
    /// The acting identity recorded for the check (non-empty; a service
    /// or operator identity reference — never a credential, §21.3).
    pub actor: String,
    /// The correlation (trace) id the check ran under (§21.3; non-empty
    /// text — the outbox envelope's correlation id or the request trace).
    pub correlation_id: String,
    /// The completion timestamp (UTC unix millis, from the controlled
    /// clock — never wall-clock).
    pub checked_at_unix_millis: i64,
}

impl FeasibilityCheckRow {
    /// Builds and validates one check row. Rules (mirrored by migration
    /// 0017's CHECK constraints): every identity non-empty; outcome in
    /// the closed vocabulary; `schedule_revision` in `1..=i64::MAX`; a
    /// PASSED check names no constraint and no failure reason and binds
    /// THE MODEL HASH; a LIMIT_VIOLATED check names the constraint; a
    /// FAIL_CLOSED check names the reason.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tenant_id: TenantId,
        check_id: String,
        command_id: Option<String>,
        portfolio_id: String,
        site_id: String,
        asset_id: String,
        schedule_id: String,
        schedule_revision: u64,
        model_id: Option<String>,
        model_version: Option<String>,
        model_content_hash: Option<String>,
        outcome: String,
        failing_constraint: Option<String>,
        reason: Option<String>,
        margins: Option<String>,
        coverage: Option<String>,
        actor: String,
        correlation_id: String,
        checked_at_unix_millis: i64,
    ) -> Result<Self, FeasibilityCheckStoreError> {
        for (field, value) in [
            ("check_id", check_id.as_str()),
            ("portfolio_id", portfolio_id.as_str()),
            ("site_id", site_id.as_str()),
            ("asset_id", asset_id.as_str()),
            ("schedule_id", schedule_id.as_str()),
            ("actor", actor.as_str()),
            ("correlation_id", correlation_id.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(FeasibilityCheckStoreError::InvalidRecord { field });
            }
        }
        if let Some(command) = command_id.as_deref() {
            if command.trim().is_empty() {
                return Err(FeasibilityCheckStoreError::InvalidRecord { field: "command_id" });
            }
        }
        if schedule_revision == 0 || schedule_revision > i64::MAX as u64 {
            return Err(FeasibilityCheckStoreError::InvalidRecord {
                field: "schedule_revision",
            });
        }
        if !FEASIBILITY_CHECK_OUTCOMES.contains(&outcome.as_str()) {
            return Err(FeasibilityCheckStoreError::InvalidRecord { field: "outcome" });
        }
        for (field, value) in [
            ("model_id", model_id.as_deref()),
            ("model_version", model_version.as_deref()),
            ("model_content_hash", model_content_hash.as_deref()),
            ("failing_constraint", failing_constraint.as_deref()),
            ("reason", reason.as_deref()),
            ("margins", margins.as_deref()),
            ("coverage", coverage.as_deref()),
        ] {
            if let Some(value) = value {
                if value.trim().is_empty() {
                    return Err(FeasibilityCheckStoreError::InvalidRecord { field });
                }
            }
        }
        // The verdict-shape rules (migration 0017 CHECKs mirror them).
        match outcome.as_str() {
            FEASIBILITY_OUTCOME_PASSED => {
                if failing_constraint.is_some() || reason.is_some() {
                    return Err(FeasibilityCheckStoreError::InvalidRecord {
                        field: "failing_constraint",
                    });
                }
                if model_content_hash.is_none() {
                    return Err(FeasibilityCheckStoreError::InvalidRecord {
                        field: "model_content_hash",
                    });
                }
            }
            FEASIBILITY_OUTCOME_LIMIT_VIOLATED => {
                if failing_constraint.is_none() {
                    return Err(FeasibilityCheckStoreError::InvalidRecord {
                        field: "failing_constraint",
                    });
                }
            }
            FEASIBILITY_OUTCOME_FAIL_CLOSED => {
                if reason.is_none() {
                    return Err(FeasibilityCheckStoreError::InvalidRecord { field: "reason" });
                }
            }
            _ => {
                return Err(FeasibilityCheckStoreError::InvalidRecord { field: "outcome" });
            }
        }
        Ok(Self {
            tenant_id,
            check_id,
            command_id,
            portfolio_id,
            site_id,
            asset_id,
            schedule_id,
            schedule_revision,
            model_id,
            model_version,
            model_content_hash,
            outcome,
            failing_constraint,
            reason,
            margins,
            coverage,
            actor,
            correlation_id,
            checked_at_unix_millis,
        })
    }
}

/// The listing filter of [`FeasibilityCheckStore::list_feasibility_checks`].
/// Every field is optional; an absent filter lists the tenant's records in
/// `(checked_at, check_id)` order. There is NO unbounded query: the page
/// size is always applied by the caller (the recorded P2.12/P3.28 listing
/// convention).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FeasibilityCheckFilter {
    /// Restrict to one command (canonical UUID text).
    pub command_id: Option<String>,
    /// Restrict to one schedule chain (canonical UUID text).
    pub schedule_id: Option<String>,
    /// Keyset cursor: return records strictly AFTER this (check_id) in the
    /// listing order (the opaque cursor of the previous page).
    pub after_check_id: Option<String>,
}

// --- The port -------------------------------------------------------------------

/// The feasibility-check evidence persistence contract (ledger P4.30 step
/// 1; report §15.2, §16.1–§16.2). An append-only record store:
///
/// - rows are immutable (INSERT + SELECT only; migration 0017 refuses
///   UPDATE and DELETE by grant AND by trigger);
/// - every operation runs inside the CALLER's unit of work (adapter
///   obligation, binding) — the check record and the paired outbox row
///   (`FeasibilityCheckCompleted`) commit or roll back TOGETHER (§15.5);
/// - every query is tenant-scoped: RLS via the transaction-scoped
///   `flexcore.tenant_id` setting (P1.12 §5.1) PLUS an explicit `tenant_id`
///   filter; a foreign-tenant check reads as `None` — existence never
///   leaks (P0.16 rule 5);
/// - storage failures surface as [`FeasibilityCheckStoreError::Backend`]
///   with an opaque code + message only.
pub trait FeasibilityCheckStore {
    /// Inserts one NEW check record (a duplicate (check_id, tenant_id) is
    /// [`FeasibilityCheckStoreError::Conflict`] — check rows are
    /// immutable; a re-checked candidate is a NEW check id).
    fn insert_feasibility_check(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        row: &FeasibilityCheckRow,
    ) -> Result<(), FeasibilityCheckStoreError>;

    /// Looks up one check record in the caller's tenant scope. A
    /// foreign-tenant check reads as `None`.
    fn find_feasibility_check(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        check_id: &str,
    ) -> Result<Option<FeasibilityCheckRow>, FeasibilityCheckStoreError>;

    /// Lists check records in the caller's tenant scope, ordered by
    /// `(checked_at_unix_millis, check_id)` ascending, applying the
    /// filter. The caller ALWAYS bounds the result (the recorded listing
    /// convention: default page 200, maximum 1000 — no unbounded query).
    fn list_feasibility_checks(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        filter: &FeasibilityCheckFilter,
        limit: u32,
    ) -> Result<Vec<FeasibilityCheckRow>, FeasibilityCheckStoreError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;

    fn tenant() -> TenantId {
        let mut bytes = [0u8; 16];
        bytes[15] = 7;
        TenantId::new(FlexUuid::from_bytes(bytes))
    }

    fn base_row(outcome: &str) -> FeasibilityCheckRow {
        FeasibilityCheckRow::new(
            tenant(),
            "attempt-7".to_string(),
            Some("0192a5e0-0000-7000-8000-0000000000a1".to_string()),
            "0192a5e0-0000-7000-8000-0000000000b1".to_string(),
            "0192a5e0-0000-7000-8000-0000000000c1".to_string(),
            "0192a5e0-0000-7000-8000-0000000000d1".to_string(),
            "0192a5e0-0000-7000-8000-0000000000e1".to_string(),
            2,
            Some("feeder-a".to_string()),
            Some("grid-model-v1".to_string()),
            Some("fnv1a64:deadbeef".to_string()),
            outcome.to_string(),
            None,
            None,
            Some(" margins ok ".to_string()),
            Some("balanced radial feeder".to_string()),
            "svc:grid-gate".to_string(),
            "0192a5e0-0000-7000-8000-0000000000f1".to_string(),
            1_700_000_100_000,
        )
        .expect("base row builds")
    }

    #[test]
    fn the_outcome_vocabulary_is_closed() {
        assert_eq!(
            FEASIBILITY_CHECK_OUTCOMES,
            ["PASSED", "LIMIT_VIOLATED", "FAIL_CLOSED"]
        );
    }

    #[test]
    fn a_passed_row_requires_the_model_hash_and_names_no_failure() {
        let row = base_row(FEASIBILITY_OUTCOME_PASSED);
        assert_eq!(row.outcome, "PASSED");
        assert!(row.failing_constraint.is_none());
        assert!(row.reason.is_none());
        assert!(row.model_content_hash.is_some());
        // PASSED without the model hash is rejected (the evidence-binding
        // rule, §17.5 invariant 3).
        let build = |hash: Option<String>| {
            FeasibilityCheckRow::new(
                tenant(),
                "attempt-8".to_string(),
                None,
                "0192a5e0-0000-7000-8000-0000000000b1".to_string(),
                "0192a5e0-0000-7000-8000-0000000000c1".to_string(),
                "0192a5e0-0000-7000-8000-0000000000d1".to_string(),
                "0192a5e0-0000-7000-8000-0000000000e1".to_string(),
                1,
                Some("feeder-a".to_string()),
                None,
                hash,
                "PASSED".to_string(),
                None,
                None,
                None,
                None,
                "svc:grid-gate".to_string(),
                "trace-1".to_string(),
                1,
            )
        };
        assert_eq!(
            build(None),
            Err(FeasibilityCheckStoreError::InvalidRecord {
                field: "model_content_hash"
            })
        );
        assert!(build(Some("fnv1a64:aa".to_string())).is_ok());
    }

    #[test]
    fn verdict_shapes_are_enforced_before_anything_is_written() {
        // LIMIT_VIOLATED without the constraint is rejected.
        let mut row = base_row(FEASIBILITY_OUTCOME_LIMIT_VIOLATED);
        row.failing_constraint = None;
        let rebuilt = FeasibilityCheckRow::new(
            row.tenant_id,
            row.check_id.clone(),
            row.command_id.clone(),
            row.portfolio_id.clone(),
            row.site_id.clone(),
            row.asset_id.clone(),
            row.schedule_id.clone(),
            row.schedule_revision,
            row.model_id.clone(),
            row.model_version.clone(),
            row.model_content_hash.clone(),
            row.outcome.clone(),
            None,
            None,
            row.margins.clone(),
            row.coverage.clone(),
            row.actor.clone(),
            row.correlation_id.clone(),
            row.checked_at_unix_millis,
        );
        assert_eq!(
            rebuilt,
            Err(FeasibilityCheckStoreError::InvalidRecord {
                field: "failing_constraint"
            })
        );
        // FAIL_CLOSED requires the reason.
        assert_eq!(
            FeasibilityCheckRow::new(
                row.tenant_id,
                row.check_id.clone(),
                None,
                row.portfolio_id.clone(),
                row.site_id.clone(),
                row.asset_id.clone(),
                row.schedule_id.clone(),
                1,
                None,
                None,
                None,
                "FAIL_CLOSED".to_string(),
                None,
                None,
                None,
                None,
                "svc:grid-gate".to_string(),
                "trace-1".to_string(),
                1,
            ),
            Err(FeasibilityCheckStoreError::InvalidRecord { field: "reason" })
        );
        // An outcome outside the closed vocabulary is rejected.
        assert_eq!(
            FeasibilityCheckRow::new(
                row.tenant_id,
                row.check_id.clone(),
                None,
                row.portfolio_id.clone(),
                row.site_id.clone(),
                row.asset_id.clone(),
                row.schedule_id.clone(),
                1,
                None,
                None,
                None,
                "PROBABLY_FINE".to_string(),
                None,
                None,
                None,
                None,
                "svc:grid-gate".to_string(),
                "trace-1".to_string(),
                1,
            ),
            Err(FeasibilityCheckStoreError::InvalidRecord { field: "outcome" })
        );
    }

    #[test]
    fn identity_and_revision_fields_are_validated() {
        // An empty check id is rejected.
        let mut row = base_row(FEASIBILITY_OUTCOME_PASSED);
        let bad = FeasibilityCheckRow::new(
            row.tenant_id,
            "  ".to_string(),
            row.command_id.clone(),
            row.portfolio_id.clone(),
            row.site_id.clone(),
            row.asset_id.clone(),
            row.schedule_id.clone(),
            row.schedule_revision,
            row.model_id.clone(),
            row.model_version.clone(),
            row.model_content_hash.clone(),
            row.outcome.clone(),
            row.failing_constraint.clone(),
            row.reason.clone(),
            row.margins.clone(),
            row.coverage.clone(),
            row.actor.clone(),
            row.correlation_id.clone(),
            row.checked_at_unix_millis,
        );
        assert_eq!(bad, Err(FeasibilityCheckStoreError::InvalidRecord { field: "check_id" }));
        // A zero revision is rejected.
        assert_eq!(
            FeasibilityCheckRow::new(
                row.tenant_id,
                row.check_id.clone(),
                row.command_id.clone(),
                row.portfolio_id.clone(),
                row.site_id.clone(),
                row.asset_id.clone(),
                row.schedule_id.clone(),
                0,
                row.model_id.clone(),
                row.model_version.clone(),
                row.model_content_hash.clone(),
                row.outcome.clone(),
                row.failing_constraint.clone(),
                row.reason.clone(),
                row.margins.clone(),
                row.coverage.clone(),
                row.actor.clone(),
                row.correlation_id.clone(),
                row.checked_at_unix_millis,
            ),
            Err(FeasibilityCheckStoreError::InvalidRecord {
                field: "schedule_revision"
            })
        );
        // An empty actor or correlation id is rejected (§21.3 correlation
        // is mandatory on the record).
        for (field, corrupt) in [("actor", 0), ("correlation_id", 1)] {
            let mut actor = row.actor.clone();
            let mut correlation = row.correlation_id.clone();
            if corrupt == 0 {
                actor = "".to_string();
            } else {
                correlation = " ".to_string();
            }
            assert_eq!(
                FeasibilityCheckRow::new(
                    row.tenant_id,
                    row.check_id.clone(),
                    row.command_id.clone(),
                    row.portfolio_id.clone(),
                    row.site_id.clone(),
                    row.asset_id.clone(),
                    row.schedule_id.clone(),
                    row.schedule_revision,
                    row.model_id.clone(),
                    row.model_version.clone(),
                    row.model_content_hash.clone(),
                    row.outcome.clone(),
                    row.failing_constraint.clone(),
                    row.reason.clone(),
                    row.margins.clone(),
                    row.coverage.clone(),
                    actor,
                    correlation,
                    row.checked_at_unix_millis,
                ),
                Err(FeasibilityCheckStoreError::InvalidRecord { field }),
                "empty {field} must be rejected"
            );
        }
    }

    #[test]
    fn the_trait_is_object_safe_for_the_unit_of_work() {
        fn takes_dyn_store(_: &dyn FeasibilityCheckStore) {}
        let _ = takes_dyn_store as fn(&dyn FeasibilityCheckStore);
    }
}
