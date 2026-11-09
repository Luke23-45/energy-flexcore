// P3.23 — Schedule comparison workflow (report §23 Stage 3, §19.4; ledger
//         P3.23). Decision record:
//         `ledger/phase-03-flexibility-scheduling/22_24_revisions_comparison_rolling.md`.
//
// The comparison lets alternatives (LP vs MILP, Tier 1 vs Tier 2, different
// formulations under one objective) be compared ON THE SAME INPUTS
// (ledger P3.23 step 2: same envelope, same forecasts, same objective
// config — only the formulation/solver tier varies).
//
// ## THE CROSS-INPUT POLICY, DECIDED ONCE (D-P3.23-2)
//
//   **A comparison across different modeling inputs is REJECTED with the
//   named error `ComparisonError::IncomparableInputs`** — rejection is
//   cleaner than a labelled record because an incomparable comparison is a
//   caller bug, not data. The labelling path
//   (`incomparability_label`) exists for DISPLAY contexts that must render
//   an already-recorded comparison next to other data without re-running
//   the gate; it is not a second acceptance path.
//
//   Comparability is enforced on the MODELING constituents — envelope
//   version, forecast version (+ provenance), objective configuration
//   (record fields verbatim), objective name, and the telemetry reference —
//   NOT on the P3.21 snapshot reference string, which embeds the
//   solver-config digest and therefore legitimately differs across the
//   tier-varying comparison (the recorded decision D-P3.23-2; each
//   candidate's own §12.6 snapshot reference is recorded verbatim on its
//   entry). The comparison record itself carries the shared modeling-snapshot
//   ID once.
//
// ## The four §19.4 scheduling metrics (ledger step 1)
//
//   objective value (+ units), schedule deviation, feasible interval count,
//   runtime — REQUIRED per candidate. A comparison is over PRODUCED
//   schedules: a candidate whose §12.6 status is not `optimal`/`feasible` is
//   rejected with a named error (both compared sides remain CANDIDATES —
//   the comparison is diagnostic and never a feasibility verdict; Phase 4
//   owns that gate).
//
// ## API exposure (ledger step 4) — recorded, not built
//
//   The comparison is exposed NOW as a pure application function plus the
//   storage queries (`flexcore-storage::schedule_store::ScheduleComparisonStore`
//   + the PostgreSQL adapter, migration 0014). The REST routes
//   `/api/v1/schedules` and `/api/v1/optimization-runs` (§15.2) are mounted
//   in P3.28 — see [`COMPARISON_API_WIRING_NOTE`]. No UI code exists in
//   Phase 3 (the Phase 5 console consumes the P3.28 API, §20.1 item 6).
//
// ## The first comparison case (ledger step 3)
//
//   [`tier1_tier2_comparison_case`] builds the record from the P3.18
//   `Tier1Tier2ComparisonRecord` — the recorded 1 MW linearisation gap on
//   the shared inverter fixture (`flexcore-optimization-api::conic_fixture`).
//   The types are REUSED from flexcore-optimization-api, never restated.
//
// Control status: pure comparison bookkeeping for the simulation-only 0.x
// line. No solver call, no I/O, no dispatch, no UI. Both compared sides are
// CANDIDATES — never feasible, approved, or dispatchable schedules.

use std::time::Duration;

use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::ids::{PortfolioId, SiteId, TenantId};
use flexcore_optimization_api::conic_fixture::Tier1Tier2ComparisonRecord;
use flexcore_optimization_api::result::SolveStatus;

// --- Recorded statements ------------------------------------------------------

/// The four §19.4 scheduling metrics, stated once (ledger P3.23 step 1).
pub const COMPARISON_METRICS_STATEMENT: &str = "Every compared candidate \
     carries the four §19.4 scheduling metrics: objective value (with its \
     declared units), schedule deviation (MW; the caller's declared \
     aggregation basis), feasible interval count, and runtime. A comparison \
     record without all four metrics per candidate is not a comparison.";

/// The cross-input policy, decided once (D-P3.23-2).
pub const CROSS_INPUT_POLICY: &str = "A comparison is taken over a FIXED \
     modeling-input snapshot only: same envelope version, same forecast \
     version (+ provenance), same objective configuration, same telemetry \
     reference — only the formulation/solver tier varies. A cross-input \
     comparison is REJECTED with the named error IncomparableInputs \
     (decided once; never silently accepted). The comparability fields are \
     the modeling constituents, not the P3.21 snapshot reference string, \
     which embeds the solver-config digest and legitimately differs across \
     the tier-varying comparison (each candidate's own snapshot reference is \
     recorded verbatim on its entry).";

/// Where the REST routes land (ledger P3.23 step 4 — recorded, not built).
pub const COMPARISON_API_WIRING_NOTE: &str = "The comparison is exposed as a \
     pure application function plus storage queries \
     (flexcore-storage::schedule_store::ScheduleComparisonStore + the \
     PostgreSQL adapter, migration 0014). The REST routes \
     /api/v1/schedules and /api/v1/optimization-runs (report §15.2) are \
     mounted in P3.28 — the wiring is P3.28's, not this subtask's. No UI \
     code exists in Phase 3; the Phase 5 console consumes the P3.28 API \
     (§20.1 item 6).";

/// The candidate-status statement for comparisons (mirrors the P3.21
/// `CANDIDATE_STATUS_STATEMENT` — both compared sides are candidates).
pub const COMPARISON_CANDIDATE_STATEMENT: &str = "Both compared sides are \
     CANDIDATES: the comparison is diagnostic evidence about formulation and \
     solver tier, never a feasibility, authorization, or dispatch verdict \
     (Phase 4 gates; report §12.5).";

// --- Inputs -------------------------------------------------------------------

/// The SHARED modeling-input snapshot of a comparison (D-P3.23-2). Every
/// candidate must attest exactly these values; a difference anywhere is the
/// named rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedComparisonInputs {
    /// The shared modeling-snapshot reference (deterministic; recorded once
    /// on the comparison record — this is the modeling-input snapshot, NOT
    /// the per-candidate §12.6 snapshot reference, which embeds the
    /// solver-config digest).
    pub input_snapshot_id: String,
    /// The shared envelope version (P3.12/P3.13).
    pub envelope_version: String,
    /// The shared forecast version (P3.31).
    pub forecast_version: String,
    /// The shared forecast provenance label (always `synthetic` in v0.2.0).
    pub forecast_provenance: String,
    /// The shared objective name (P3.14).
    pub objective_name: String,
    /// The shared objective configuration as ordered record fields
    /// (verbatim `ObjectiveConfig::record_fields`) — the anti-conflation
    /// mechanism: two candidates computed under different objectives are
    /// never comparable.
    pub objective_config_record: Vec<(String, String)>,
    /// The shared telemetry snapshot reference.
    pub telemetry_snapshot_reference: String,
}

impl SharedComparisonInputs {
    /// Validates the shared inputs: every field non-empty, the objective
    /// record non-empty.
    pub fn validate(&self) -> Result<(), ComparisonError> {
        for (field, value) in [
            ("input_snapshot_id", self.input_snapshot_id.as_str()),
            ("envelope_version", self.envelope_version.as_str()),
            ("forecast_version", self.forecast_version.as_str()),
            ("forecast_provenance", self.forecast_provenance.as_str()),
            ("objective_name", self.objective_name.as_str()),
            ("telemetry_snapshot_reference", self.telemetry_snapshot_reference.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(ComparisonError::InvalidSharedInput { field });
            }
        }
        if self.objective_config_record.is_empty() {
            return Err(ComparisonError::InvalidSharedInput {
                field: "objective_config_record",
            });
        }
        Ok(())
    }
}

/// The per-candidate modeling-input attestation — the values the gate
/// compares against [`SharedComparisonInputs`] (same field set, so a drift
/// in any constituent is the named rejection).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateModelingInputs {
    pub envelope_version: String,
    pub forecast_version: String,
    pub forecast_provenance: String,
    pub objective_name: String,
    pub objective_config_record: Vec<(String, String)>,
    pub telemetry_snapshot_reference: String,
}

/// One candidate entry of a comparison (ledger P3.23 step 1). The
/// formulation/solver identity may differ across candidates — that is the
/// point of the workflow; the modeling attestation may not.
#[derive(Debug, Clone, PartialEq)]
pub struct CandidateComparisonEntry {
    /// The candidate's §12.6 candidate ID (canonical UUID text).
    pub candidate_id: String,
    /// The candidate's own §12.6 input-snapshot reference (recorded
    /// verbatim; may differ across tiers by the solver-config digest — the
    /// recorded D-P3.23-2).
    pub result_input_snapshot_reference: String,
    /// The modeling-input attestation (gated against the shared inputs).
    pub modeling: CandidateModelingInputs,
    /// The declared problem class (stable short identifier, e.g. `lp`,
    /// `socp`) — the varied factor.
    pub problem_class: String,
    pub solver_name: String,
    pub solver_version: String,
    pub provider_id: String,
    pub provider_version: String,
    /// The §12.6 status string; must be `optimal` or `feasible` (a
    /// comparison is over produced schedules).
    pub status: String,
    /// §19.4 metric 1: objective value.
    pub objective_value: f64,
    /// Objective units (non-empty — the metric is meaningless without them).
    pub objective_units: String,
    /// §19.4 metric 2: schedule deviation, MW (the caller's declared
    /// aggregation basis; recorded here as the metric value).
    pub schedule_deviation_mw: f64,
    /// §19.4 metric 3: feasible interval count.
    pub feasible_interval_count: u64,
    /// §19.4 metric 4: runtime of the solve.
    pub runtime: Duration,
}

// --- The comparison record ------------------------------------------------------

/// The assembled comparison record (ledger P3.23 step 1): one shared input
/// snapshot, the compared candidates, and the per-candidate §19.4 metrics.
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduleComparisonRecord {
    comparison_id: FlexUuid,
    tenant_id: TenantId,
    portfolio_id: PortfolioId,
    site_id: SiteId,
    shared: SharedComparisonInputs,
    candidates: Vec<CandidateComparisonEntry>,
    notes: Vec<String>,
}

impl ScheduleComparisonRecord {
    /// The comparison identity.
    pub fn comparison_id(&self) -> FlexUuid {
        self.comparison_id
    }

    /// The owning tenant.
    pub fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }

    /// The owning portfolio.
    pub fn portfolio_id(&self) -> PortfolioId {
        self.portfolio_id
    }

    /// The site the compared schedules were computed for.
    pub fn site_id(&self) -> SiteId {
        self.site_id
    }

    /// The shared modeling inputs (recorded once).
    pub fn shared_inputs(&self) -> &SharedComparisonInputs {
        &self.shared
    }

    /// The compared candidates (in input order), each with all four §19.4
    /// metrics.
    pub fn candidates(&self) -> &[CandidateComparisonEntry] {
        &self.candidates
    }

    /// The candidate IDs in input order.
    pub fn candidate_ids(&self) -> Vec<&str> {
        self.candidates
            .iter()
            .map(|c| c.candidate_id.as_str())
            .collect()
    }

    /// The recorded notes (comparability, candidate status, and any
    /// case-specific evidence such as the P3.18 linearisation gap).
    pub fn notes(&self) -> &[String] {
        &self.notes
    }
}

// --- Errors --------------------------------------------------------------------

/// The comparison errors. `IncomparableInputs` is THE cross-input rejection
/// (D-P3.23-2): named, per-field, never silently accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComparisonError {
    /// The candidate's modeling attestation differs from the shared inputs
    /// (`field` names the differing constituent). The comparison was NOT
    /// produced.
    IncomparableInputs {
        candidate_id: String,
        field: &'static str,
        expected: String,
        found: String,
    },
    /// The candidate's §12.6 status is not a produced schedule
    /// (`optimal`/`feasible` only).
    NotAScheduleCandidate {
        candidate_id: String,
        status: String,
    },
    /// A §19.4 metric that must be present is missing or malformed
    /// (`metric` names it).
    MissingMetric {
        candidate_id: String,
        metric: &'static str,
    },
    /// A shared-input field is missing or empty (`field` names it).
    InvalidSharedInput {
        field: &'static str,
    },
    /// A comparison needs at least two candidates.
    TooFewCandidates {
        found: usize,
    },
    /// An identifier field is empty (`field` names it).
    InvalidIdentifier {
        field: &'static str,
    },
}

impl std::fmt::Display for ComparisonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComparisonError::IncomparableInputs { candidate_id, field, expected, found } => {
                write!(
                    f,
                    "candidate {candidate_id} is incomparable: its {field} is '{found}' \
                     but the shared comparison input is '{expected}'; a comparison is \
                     taken over one fixed modeling-input snapshot only (P3.23)"
                )
            }
            ComparisonError::NotAScheduleCandidate { candidate_id, status } => write!(
                f,
                "candidate {candidate_id} has §12.6 status '{status}'; a comparison is \
                 over produced schedules (optimal/feasible) only"
            ),
            ComparisonError::MissingMetric { candidate_id, metric } => write!(
                f,
                "candidate {candidate_id} is missing the §19.4 metric '{metric}'"
            ),
            ComparisonError::InvalidSharedInput { field } => write!(
                f,
                "the shared comparison input '{field}' is missing or empty"
            ),
            ComparisonError::TooFewCandidates { found } => write!(
                f,
                "a comparison needs at least two candidates; {found} supplied"
            ),
            ComparisonError::InvalidIdentifier { field } => write!(
                f,
                "the comparison identifier '{field}' is missing or empty"
            ),
        }
    }
}

impl std::error::Error for ComparisonError {}

// --- The pure comparison gate --------------------------------------------------

/// Builds the named cross-input rejection (D-P3.23-2).
fn incomparable(
    candidate_id: &str,
    field: &'static str,
    expected: String,
    found: String,
) -> ComparisonError {
    ComparisonError::IncomparableInputs {
        candidate_id: candidate_id.to_string(),
        field,
        expected,
        found,
    }
}

/// Compares candidate schedules over ONE fixed modeling-input snapshot
/// (ledger P3.23 steps 1–2). The gate enforces, in order:
///
/// 1. the shared inputs are complete;
/// 2. at least two candidates are supplied;
/// 3. every candidate is a produced schedule (`optimal`/`feasible`);
/// 4. every candidate carries all four §19.4 metrics (non-empty units,
///    finite objective and deviation);
/// 5. every candidate's modeling attestation equals the shared inputs —
///    any difference is [`ComparisonError::IncomparableInputs`].
///
/// The produced record is pure data; persisting it goes through
/// `flexcore-storage::schedule_store::ScheduleComparisonStore` (the
/// composition root serializes the candidates into the payload JSON).
pub fn compare_candidate_schedules(
    tenant_id: TenantId,
    portfolio_id: PortfolioId,
    site_id: SiteId,
    comparison_id: FlexUuid,
    shared: SharedComparisonInputs,
    candidates: Vec<CandidateComparisonEntry>,
) -> Result<ScheduleComparisonRecord, ComparisonError> {
    shared.validate()?;
    if candidates.len() < 2 {
        return Err(ComparisonError::TooFewCandidates {
            found: candidates.len(),
        });
    }

    let mut checked = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        if candidate.candidate_id.trim().is_empty() {
            return Err(ComparisonError::InvalidIdentifier { field: "candidate_id" });
        }
        if candidate.result_input_snapshot_reference.trim().is_empty() {
            return Err(ComparisonError::InvalidIdentifier {
                field: "result_input_snapshot_reference",
            });
        }
        for (field, value) in [
            ("problem_class", candidate.problem_class.as_str()),
            ("solver_name", candidate.solver_name.as_str()),
            ("solver_version", candidate.solver_version.as_str()),
            ("provider_id", candidate.provider_id.as_str()),
            ("provider_version", candidate.provider_version.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(ComparisonError::InvalidIdentifier { field });
            }
        }
        // Gate 3: a comparison is over produced schedules.
        match SolveStatus::parse(&candidate.status) {
            Some(SolveStatus::Optimal) | Some(SolveStatus::Feasible) => {}
            _ => {
                return Err(ComparisonError::NotAScheduleCandidate {
                    candidate_id: candidate.candidate_id.clone(),
                    status: candidate.status.clone(),
                });
            }
        }
        // Gate 4: the four §19.4 metrics, all required.
        if candidate.objective_units.trim().is_empty() {
            return Err(ComparisonError::MissingMetric {
                candidate_id: candidate.candidate_id.clone(),
                metric: "objective_units",
            });
        }
        if !candidate.objective_value.is_finite() {
            return Err(ComparisonError::MissingMetric {
                candidate_id: candidate.candidate_id.clone(),
                metric: "objective_value",
            });
        }
        if !candidate.schedule_deviation_mw.is_finite() {
            return Err(ComparisonError::MissingMetric {
                candidate_id: candidate.candidate_id.clone(),
                metric: "schedule_deviation_mw",
            });
        }
        // The runtime metric is a required Duration field (always present by
        // construction); no lower bound is imposed — a measured zero is a
        // measurement, not a fabrication (§19.4: no arbitrary targets).
        // Gate 5: the modeling attestation equals the shared inputs (the
        // cross-input policy, D-P3.23-2 — rejection with the named error).
        let modeling = &candidate.modeling;
        let candidate_id = candidate.candidate_id.as_str();
        if modeling.envelope_version != shared.envelope_version {
            return Err(incomparable(
                candidate_id,
                "envelope_version",
                shared.envelope_version.clone(),
                modeling.envelope_version.clone(),
            ));
        }
        if modeling.forecast_version != shared.forecast_version {
            return Err(incomparable(
                candidate_id,
                "forecast_version",
                shared.forecast_version.clone(),
                modeling.forecast_version.clone(),
            ));
        }
        if modeling.forecast_provenance != shared.forecast_provenance {
            return Err(incomparable(
                candidate_id,
                "forecast_provenance",
                shared.forecast_provenance.clone(),
                modeling.forecast_provenance.clone(),
            ));
        }
        if modeling.objective_name != shared.objective_name {
            return Err(incomparable(
                candidate_id,
                "objective_name",
                shared.objective_name.clone(),
                modeling.objective_name.clone(),
            ));
        }
        if modeling.objective_config_record != shared.objective_config_record {
            return Err(incomparable(
                candidate_id,
                "objective_config_record",
                format!("{:?}", shared.objective_config_record),
                format!("{:?}", modeling.objective_config_record),
            ));
        }
        if modeling.telemetry_snapshot_reference != shared.telemetry_snapshot_reference {
            return Err(incomparable(
                candidate_id,
                "telemetry_snapshot_reference",
                shared.telemetry_snapshot_reference.clone(),
                modeling.telemetry_snapshot_reference.clone(),
            ));
        }
        checked.push(candidate);
    }

    let mut notes = vec![
        CROSS_INPUT_POLICY.to_string(),
        COMPARISON_CANDIDATE_STATEMENT.to_string(),
    ];
    let classes: Vec<String> = checked
        .iter()
        .map(|c| format!("{} ({})", c.candidate_id, c.problem_class))
        .collect();
    notes.push(format!("compared candidates: {}", classes.join(", ")));

    Ok(ScheduleComparisonRecord {
        comparison_id,
        tenant_id,
        portfolio_id,
        site_id,
        shared,
        candidates: checked,
        notes,
    })
}

/// The DISPLAY-side labelling path (D-P3.23-2): for contexts that must
/// render modeling inputs of two artifacts side by side WITHOUT running the
/// comparison gate. This is a rendering helper only — it is not, and never
/// becomes, a second acceptance path for cross-input comparisons.
pub fn incomparability_label(field: &str, expected: &str, found: &str) -> String {
    format!("INCOMPARABLE ({field}): '{found}' vs the comparison's '{expected}'")
}

// --- The first comparison case: P3.18 Tier 1 vs Tier 2 --------------------------

/// The inputs for the FIRST comparison case (ledger P3.23 step 3): the P3.18
/// Tier 1 (linearised) vs Tier 2 (conic) cross-check — the recorded 1 MW
/// linearisation gap on the shared inverter fixture
/// (`flexcore-optimization-api::conic_fixture`). The fixture has a single
/// interval and no commitment data, so the schedule-deviation metric and the
/// feasible-interval count are SUPPLIED by the caller with their basis
/// recorded (the tests use 0.0 MW and 1 interval respectively and record
/// that basis in the notes).
pub struct Tier1Tier2ComparisonCaseInput<'a> {
    /// The P3.18 cross-check record (types reused, never restated).
    pub cross_check: &'a Tier1Tier2ComparisonRecord,
    pub tenant_id: TenantId,
    pub portfolio_id: PortfolioId,
    pub site_id: SiteId,
    /// The comparison identity (a fresh UUID from the composition root).
    pub comparison_id: FlexUuid,
    /// The shared modeling inputs both sides were computed against. Both
    /// sides of the P3.18 fixture pair are built on the SAME shared fixture
    /// scenario by construction, so the shared attestation applies to both.
    pub shared: SharedComparisonInputs,
    /// The Tier 1 side's §12.6 candidate ID.
    pub tier1_candidate_id: String,
    /// The Tier 2 side's §12.6 candidate ID.
    pub tier2_candidate_id: String,
    /// The Tier 1 side's own §12.6 snapshot reference (verbatim).
    pub tier1_result_input_snapshot_reference: String,
    /// The Tier 2 side's own §12.6 snapshot reference (verbatim).
    pub tier2_result_input_snapshot_reference: String,
    /// The Tier 1 side's solver/provider identity (the P3.18 record does not
    /// carry it — supplied explicitly, never invented).
    pub tier1_solver_name: String,
    pub tier1_solver_version: String,
    pub tier1_provider_id: String,
    pub tier1_provider_version: String,
    /// The Tier 2 side's solver/provider identity.
    pub tier2_solver_name: String,
    pub tier2_solver_version: String,
    pub tier2_provider_id: String,
    pub tier2_provider_version: String,
    pub tier1_schedule_deviation_mw: f64,
    pub tier2_schedule_deviation_mw: f64,
    pub tier1_feasible_interval_count: u64,
    pub tier2_feasible_interval_count: u64,
    pub tier1_runtime: Duration,
    pub tier2_runtime: Duration,
}

/// Builds the first comparison case from the P3.18 cross-check record
/// (ledger P3.23 step 3). Both sides must be produced schedules; the
/// linearisation gap (`tier1.objective_value − tier2.objective_value`, the
/// recorded 1 MW on the shared inverter fixture) is added to the notes with
/// its direction explained — interpret against the declared objective sense
/// and units (the P3.18 record's own guidance).
pub fn tier1_tier2_comparison_case(
    input: Tier1Tier2ComparisonCaseInput<'_>,
) -> Result<ScheduleComparisonRecord, ComparisonError> {
    let cross_check = input.cross_check;
    let tier1 = build_tier_side(
        &input.tier1_candidate_id,
        &input.tier1_result_input_snapshot_reference,
        &cross_check.tier1,
        &input.shared,
        &input.tier1_solver_name,
        &input.tier1_solver_version,
        &input.tier1_provider_id,
        &input.tier1_provider_version,
        input.tier1_schedule_deviation_mw,
        input.tier1_feasible_interval_count,
        input.tier1_runtime,
    )?;
    let tier2 = build_tier_side(
        &input.tier2_candidate_id,
        &input.tier2_result_input_snapshot_reference,
        &cross_check.tier2,
        &input.shared,
        &input.tier2_solver_name,
        &input.tier2_solver_version,
        &input.tier2_provider_id,
        &input.tier2_provider_version,
        input.tier2_schedule_deviation_mw,
        input.tier2_feasible_interval_count,
        input.tier2_runtime,
    )?;

    let mut record = compare_candidate_schedules(
        input.tenant_id,
        input.portfolio_id,
        input.site_id,
        input.comparison_id,
        input.shared,
        vec![tier1, tier2],
    )?;

    // The P3.18 cross-check notes travel into the comparison record (never
    // dropped), followed by the linearisation-gap evidence when both sides
    // produced objectives.
    let cross_check_notes: Vec<String> = cross_check.notes.iter().cloned().collect();
    record.notes.splice(0..0, cross_check_notes);
    if let Some(difference) = cross_check.objective_difference {
        record.notes.push(format!(
            "linearisation gap (tier1.objective_value - tier2.objective_value) = {difference}; \
             a positive difference on a maximisation means the linearised approximation \
             overestimates the deliverable objective — interpret against the declared \
             objective sense and units (the P3.18 record's guidance)"
        ));
    }
    Ok(record)
}

/// Builds one comparison entry from one side of the P3.18 cross-check
/// record. Objective value/units must be present (the comparison is over
/// produced schedules). The modeling attestation is the case's shared
/// modeling inputs (both sides of the P3.18 fixture pair are built on the
/// SAME shared fixture scenario by construction); the gate in
/// `compare_candidate_schedules` still runs, so a caller assembling the
/// case with modelling inputs that differ from the shared declaration is
/// rejected with the named error like any other path.
fn build_tier_side(
    candidate_id: &str,
    result_input_snapshot_reference: &str,
    side: &flexcore_optimization_api::conic_fixture::TierComparisonSide,
    shared_modeling: &SharedComparisonInputs,
    solver_name: &str,
    solver_version: &str,
    provider_id: &str,
    provider_version: &str,
    schedule_deviation_mw: f64,
    feasible_interval_count: u64,
    runtime: Duration,
) -> Result<CandidateComparisonEntry, ComparisonError> {
    let status_str = side.status.as_str().to_string();
    let objective_value = side.objective_value.ok_or(ComparisonError::MissingMetric {
        candidate_id: candidate_id.to_string(),
        metric: "objective_value",
    })?;
    let objective_units = side
        .objective_units
        .clone()
        .ok_or(ComparisonError::MissingMetric {
            candidate_id: candidate_id.to_string(),
            metric: "objective_units",
        })?;
    // The status gate of `compare_candidate_schedules` would also catch a
    // non-solving side; the explicit check here gives the named error before
    // the modeling gate runs.
    match side.status {
        SolveStatus::Optimal | SolveStatus::Feasible => {}
        _ => {
            return Err(ComparisonError::NotAScheduleCandidate {
                candidate_id: candidate_id.to_string(),
                status: status_str,
            });
        }
    }
    Ok(CandidateComparisonEntry {
        candidate_id: candidate_id.to_string(),
        result_input_snapshot_reference: result_input_snapshot_reference.to_string(),
        modeling: CandidateModelingInputs {
            envelope_version: shared_modeling.envelope_version.clone(),
            forecast_version: shared_modeling.forecast_version.clone(),
            forecast_provenance: shared_modeling.forecast_provenance.clone(),
            objective_name: shared_modeling.objective_name.clone(),
            objective_config_record: shared_modeling.objective_config_record.clone(),
            telemetry_snapshot_reference: shared_modeling.telemetry_snapshot_reference.clone(),
        },
        problem_class: side.problem_class.as_str().to_string(),
        solver_name: solver_name.to_string(),
        solver_version: solver_version.to_string(),
        provider_id: provider_id.to_string(),
        provider_version: provider_version.to_string(),
        status: status_str,
        objective_value,
        objective_units,
        schedule_deviation_mw,
        feasible_interval_count,
        runtime,
    })
}
