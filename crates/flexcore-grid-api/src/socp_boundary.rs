// P4.09 — The SOCP relaxation boundary for Tier 2 candidates (report
// §12.5 branch-flow SOCP paragraph, §12.3; ledger P4.09). Decision record:
// `ledger/phase-04-grid-feasibility-dispatch/06_09_gate_completion.md`
// (decisions D-P4.09-1..4).
//
// THE BOUNDARY (D-P4.09-1): a Tier 2 (SOCP) candidate's relaxation solution
// MAY be inexact. It is therefore verified with the independent AC
// power-flow engine (the P4.06 limit check over the P4.05 result) — NEVER
// accepted on the solver's word. The gate accepts a Tier 2 candidate only
// WITH its P3.18 relaxation-residual record attached (per-branch DistFlow
// residual results + the tolerance they were assessed against); a missing
// or malformed/failed record is FAIL-CLOSED (the P4.08 trigger
// `RELAXATION_RECORD_MISSING`). The AC verification runs REGARDLESS of the
// relaxation result — the relaxation never substitutes for the power-flow
// check.
//
// THE CAVEAT TRAVELS (D-P4.09-2): [`SOCP_EXACTNESS_CAVEAT`] appears on the
// verdict evidence for every Tier 2 candidate (P4.07 carries
// [`SocpBoundaryGate::Tier2VerifiedForAcCheck`]'s caveats onto the
// verdict). The verdict is a CANDIDATE verdict for Phase 5 / operations —
// never a blanket relaxation-exactness claim.
//
// CONDITIONS ARE CHECKED, NOT ASSUMED (D-P4.09-3): the §12.5 rule — do not
// assume exactness for reverse power flow, voltage upper bounds, or
// arbitrary network constraints — is implemented as an explicit condition
// checklist ([`check_relaxation_conditions`]) whose outcomes travel on the
// gate result.
//
// THE CROSS-CHECK HOOK (D-P4.09-4): the gate's limit-check results for a
// Tier 2 candidate feed the P3.18 Tier 1-vs-Tier 2 comparison record
// (objective and binding sets already exist in
// `flexcore-optimization-api::conic_fixture::Tier1Tier2ComparisonRecord`;
// this module adds the FEASIBILITY-VERDICT dimension). flexcore-grid-api
// cannot name flexcore-optimization-api (the core-crate dependency policy
// pinned by `tests/dependency_direction.rs`), so the dimension is a
// mirrored, self-contained record the composition root attaches — the same
// seam pattern as the P4.04 injected closures.
//
// MIRRORED TYPES (the seam): the residual-record shapes mirror the P3.18
// machinery (`BranchResidualSpec`, `distflow_branch_residual`,
// `SOCP_EXACTNESS_CAVEAT`, `DEFAULT_RELAXATION_RESIDUAL_TOLERANCE`) —
// the composition root converts the provider's own record into these
// types. The residual VALUE is computed upstream by
// `flexcore-optimization-api::clarabel_provider::distflow_branch_residual`
// (g_ij = v_i·l_ij − (P_ij² + Q_ij²)); this module consumes recorded
// values, it does not recompute them from primal vectors.
//
// FINDINGS ARE NEVER HIDDEN (D-P4.09-1 companion): a residual outside its
// tolerance (cone violated below −tolerance, or relaxation loose above
// +tolerance) is a RECORDED FINDING that travels to the verdict evidence —
// it does not fail the gate by itself, because the independent AC
// power-flow verification is the authority; it also does not disappear.
//
// SOLVER-CLASS DISCIPLINE: Clarabel is a convex conic solver (SOCP), not
// MISOCP and not an AC-OPF engine; nothing here claims relaxation
// exactness, global optimality, or solver-class capabilities beyond the
// recorded P3.18 boundaries.
//
// Control status: simulation-only 0.x line; a passed boundary check is
// evidence for the AC verification step, never an operational clearance.

use crate::fail_closed::{fail_closed_outcome, FailClosedOutcome, FailClosedTrigger};
use crate::port::VersionedNetworkModel;
use crate::scope::{PhaseScope, ScopeDeclaration, TopologyScope};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Recorded statements and mirrored constants
// ---------------------------------------------------------------------------

/// The §12.5 SOCP relaxation exactness caveat, mirrored VERBATIM from
/// `flexcore-optimization-api::clarabel_provider::SOCP_EXACTNESS_CAVEAT`
/// (grid-api cannot name that crate — the core-crate dependency policy).
/// The composition root converts the provider's result, whose warnings
/// carry the original constant; both texts are the recorded caveat.
pub const SOCP_EXACTNESS_CAVEAT: &str =
    "SOCP relaxation exactness is conditional and must be checked, never \
     assumed (report §12.5): this Tier 2 result is a CANDIDATE for Phase 4 \
     validation, never a feasibility verdict; verify with an independent AC \
     power-flow engine before any operational use.";

/// The per-branch relaxation residual formula, mirrored verbatim from
/// `flexcore-optimization-api::clarabel_provider::DISTFLOW_RESIDUAL_FORMULA`
/// (foundations §7.8) so the diagnostic is traceable to its source.
pub const DISTFLOW_RESIDUAL_FORMULA: &str =
    "g_ij = v_i·l_ij − (P_ij² + Q_ij²) (foundations §7.8)";

/// The mirrored default per-branch residual tolerance
/// (`flexcore-optimization-api::clarabel_provider::
/// DEFAULT_RELAXATION_RESIDUAL_TOLERANCE` = 1e-4). The ACTIVE tolerance is
/// the one carried on the [`RelaxationResidualReport`] — the composition
/// root passes the Tier 2 provider's own configured tolerance; this
/// constant is the documented fallback a caller may adopt explicitly.
pub const MIRRORED_DEFAULT_RESIDUAL_TOLERANCE: f64 = 1.0e-4;

/// The Tier 2 verdict statement (D-P4.09-2): what a passed gate means for
/// a Tier 2 candidate, and what it never means.
pub const TIER2_VERDICT_STATEMENT: &str = "A Tier 2 candidate's gate verdict \
     is a CANDIDATE verdict for Phase 5 / operations, evidence-backed by \
     the independent AC power-flow check — NEVER a blanket \
     relaxation-exactness claim and never a solver-trust transfer (§12.5).";

/// The seam statement for the mirrored types (the D-P4.04-2 pattern).
pub const SOCP_SEAM_STATEMENT: &str = "flexcore-grid-api depends only on \
     flexcore-contracts + serde/serde_json/time (the dependency-direction \
     test pins the set), so the P3.18 relaxation-residual record is MIRRORED \
     here and converted by the composition root: the residual VALUES are \
     computed upstream by `distflow_branch_residual` (g_ij = v_i·l_ij − \
     (P_ij² + Q_ij²)) and arrive as recorded numbers; this module validates \
     and assesses them against the carried tolerance.";

// ---------------------------------------------------------------------------
// Tier provenance (ledger P4.09 step 1)
// ---------------------------------------------------------------------------

/// The solve-tier provenance of a candidate, recorded in the feasibility
/// input (ledger P4.09 step 1: "Mark every candidate that came from a
/// Tier 2 SOCP solve with its provenance"). The composition root derives
/// it from the Phase 3 result record's tier marking (P3.18).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TierProvenance {
    /// The candidate came from a Tier 1 (LP/QP/MILP — HiGHS-class) solve;
    /// no relaxation boundary applies and the AC verification runs
    /// unchanged.
    Tier1Linearized,
    /// The candidate came from a Tier 2 (continuous SOCP — Clarabel-class)
    /// solve: the relaxation boundary applies (D-P4.09-1).
    Tier2SocpRelaxation,
}

impl TierProvenance {
    /// The frozen provenance code.
    pub const fn code(self) -> &'static str {
        match self {
            TierProvenance::Tier1Linearized => "tier_1_linearized",
            TierProvenance::Tier2SocpRelaxation => "tier_2_socp_relaxation",
        }
    }
}

// ---------------------------------------------------------------------------
// The relaxation-condition checklist (ledger P4.09 step 2)
// ---------------------------------------------------------------------------

/// One relaxation condition the §12.5 paragraph names or relies on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelaxationCondition {
    /// The feeder is radial (the branch-flow DistFlow relaxation's core
    /// structural condition).
    RadialFeeder,
    /// The network phases are balanced (the balanced AC power-flow model
    /// the verification runs on).
    BalancedPhases,
    /// Reverse power flow: exactness is NOT assumed (§12.5 verbatim).
    ReversePowerFlow,
    /// Voltage upper bounds: exactness is NOT assumed (§12.5 verbatim).
    VoltageUpperBound,
    /// Arbitrary network constraints: exactness is NOT assumed (§12.5
    /// verbatim).
    ArbitraryNetworkConstraints,
}

impl RelaxationCondition {
    /// The frozen condition code.
    pub const fn code(self) -> &'static str {
        match self {
            RelaxationCondition::RadialFeeder => "radial_feeder",
            RelaxationCondition::BalancedPhases => "balanced_phases",
            RelaxationCondition::ReversePowerFlow => "reverse_power_flow",
            RelaxationCondition::VoltageUpperBound => "voltage_upper_bound",
            RelaxationCondition::ArbitraryNetworkConstraints => {
                "arbitrary_network_constraints"
            }
        }
    }
}

/// The recorded outcome of one condition check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum ConditionOutcome {
    /// The condition holds for this case (evidence recorded).
    Satisfied,
    /// The condition does NOT hold (or cannot be confirmed) for this case.
    NotSatisfied { statement: String },
    /// The condition is one the §12.5 rule forbids assuming: recorded as
    /// explicitly NOT assumed, every case, no exceptions.
    NotAssumedExact { statement: String },
}

/// One checked condition with its outcome (ledger P4.09 step 2: "record
/// which conditions were checked and their outcome").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelaxationConditionRecord {
    /// The condition.
    pub condition: RelaxationCondition,
    /// The recorded outcome.
    pub outcome: ConditionOutcome,
}

/// The condition checklist for one Tier 2 candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelaxationConditionCheck {
    /// The checked conditions, in fixed checklist order.
    pub records: Vec<RelaxationConditionRecord>,
}

impl RelaxationConditionCheck {
    /// A rendered one-line summary for evidence and the cross-check hook.
    pub fn summary(&self) -> String {
        let parts: Vec<String> = self
            .records
            .iter()
            .map(|record| {
                let outcome = match &record.outcome {
                    ConditionOutcome::Satisfied => "satisfied".to_string(),
                    ConditionOutcome::NotSatisfied { .. } => "not_satisfied".to_string(),
                    ConditionOutcome::NotAssumedExact { .. } => "not_assumed_exact".to_string(),
                };
                format!("{}={}", record.condition.code(), outcome)
            })
            .collect();
        format!("relaxation conditions: {}", parts.join(", "))
    }
}

/// Checks the relaxation conditions for one Tier 2 case (ledger P4.09
/// step 2). Radiality and phase balance are read from the provider's
/// DECLARED scope (a scope that also covers meshed networks cannot confirm
/// radiality for the case); the three §12.5 no-assumption conditions are
/// recorded as `NotAssumedExact` on every case — they are never
/// "satisfied" by fiat.
pub fn check_relaxation_conditions(
    model: &VersionedNetworkModel,
    scope: &ScopeDeclaration,
) -> RelaxationConditionCheck {
    let radial = if scope.topology == TopologyScope::RadialOnly {
        ConditionOutcome::Satisfied
    } else {
        ConditionOutcome::NotSatisfied {
            statement: format!(
                "the declared scope of '{}' is '{}' — radiality of model '{}' \
                 cannot be confirmed from the declaration alone; confirm per case",
                scope.adapter_id,
                scope.topology.name(),
                model.model_ref.model_id
            ),
        }
    };
    let balanced = if scope.phase == PhaseScope::Balanced {
        ConditionOutcome::Satisfied
    } else {
        ConditionOutcome::NotSatisfied {
            statement: format!(
                "the declared scope of '{}' is unbalanced; the balanced DistFlow \
                 relaxation conditions do not transfer",
                scope.adapter_id
            ),
        }
    };
    let not_assumed = |what: &str| ConditionOutcome::NotAssumedExact {
        statement: format!(
            "relaxation exactness for {what} is NOT assumed (§12.5); the \
             independent AC power-flow verification is the authority"
        ),
    };
    RelaxationConditionCheck {
        records: vec![
            RelaxationConditionRecord {
                condition: RelaxationCondition::RadialFeeder,
                outcome: radial,
            },
            RelaxationConditionRecord {
                condition: RelaxationCondition::BalancedPhases,
                outcome: balanced,
            },
            RelaxationConditionRecord {
                condition: RelaxationCondition::ReversePowerFlow,
                outcome: not_assumed("reverse power flow"),
            },
            RelaxationConditionRecord {
                condition: RelaxationCondition::VoltageUpperBound,
                outcome: not_assumed("voltage upper bounds"),
            },
            RelaxationConditionRecord {
                condition: RelaxationCondition::ArbitraryNetworkConstraints,
                outcome: not_assumed("arbitrary network constraints"),
            },
        ],
    }
}

// ---------------------------------------------------------------------------
// The relaxation-residual record (the mirrored P3.18 record)
// ---------------------------------------------------------------------------

/// Errors of a relaxation-residual record. A record with ANY of these is a
/// FAILED record and gates fail-closed (D-P4.09-1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResidualRecordError {
    /// An identification field is empty (`field` names it).
    EmptyField { field: &'static str },
    /// A numeric field is NaN/infinite (`field` names it).
    NonFinite { field: &'static str },
    /// The record set is empty — a Tier 2 solve on a model with no
    /// recorded branches means the record was not produced.
    EmptyRecords,
    /// The tolerance is not finite and strictly positive.
    InvalidTolerance,
}

impl std::fmt::Display for ResidualRecordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResidualRecordError::EmptyField { field } => {
                write!(f, "residual record field '{field}' is empty")
            }
            ResidualRecordError::NonFinite { field } => {
                write!(f, "residual record field '{field}' is not finite")
            }
            ResidualRecordError::EmptyRecords => {
                write!(f, "the relaxation-residual record set is empty")
            }
            ResidualRecordError::InvalidTolerance => {
                write!(f, "the residual tolerance must be finite and > 0")
            }
        }
    }
}

impl std::error::Error for ResidualRecordError {}

/// One per-branch relaxation-residual record (the mirrored P3.18 record:
/// `flexcore-optimization-api::clarabel_provider` computes
/// `g_ij = v_i·l_ij − (P_ij² + Q_ij²)` per [`DISTFLOW_RESIDUAL_FORMULA`]
/// and the composition root converts). `g_ij ≥ 0` means the relaxation
/// held at the recorded point; `g_ij = 0` means the branch cone is tight;
/// `g_ij < 0` means it is violated. The residual is DIAGNOSTIC evidence —
/// it never substitutes for the AC power-flow verification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelaxationResidualRecord {
    /// The branch identifier (as the P3.18 spec recorded it).
    pub branch: String,
    /// The name of the cone encoding this branch's relaxation.
    pub cone_name: String,
    /// The recorded residual value g_ij.
    pub residual: f64,
}

impl RelaxationResidualRecord {
    /// Validates one record: identifiers non-empty, residual finite.
    /// A NEGATIVE residual is a legitimate VALUE (a violated cone) — it is
    /// assessed, not rejected here.
    pub fn validate(&self) -> Result<(), ResidualRecordError> {
        if self.branch.trim().is_empty() {
            return Err(ResidualRecordError::EmptyField { field: "branch" });
        }
        if self.cone_name.trim().is_empty() {
            return Err(ResidualRecordError::EmptyField { field: "cone_name" });
        }
        if !self.residual.is_finite() {
            return Err(ResidualRecordError::NonFinite { field: "residual" });
        }
        Ok(())
    }
}

/// The complete relaxation-residual record set attached to a Tier 2
/// candidate: one record per DistFlow branch cone, plus the tolerance the
/// assessment runs against (the Tier 2 provider's own configured
/// tolerance — the composition root passes it through; the mirrored
/// default is [`MIRRORED_DEFAULT_RESIDUAL_TOLERANCE`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelaxationResidualReport {
    /// One record per branch cone (non-empty).
    pub records: Vec<RelaxationResidualRecord>,
    /// The tolerance the assessment runs against (finite, > 0).
    pub tolerance: f64,
}

impl RelaxationResidualReport {
    /// Validates the report: a usable record set and a valid tolerance.
    /// A report failing this is a FAILED record → fail-closed (D-P4.09-1).
    pub fn validate(&self) -> Result<(), ResidualRecordError> {
        if !self.tolerance.is_finite() || self.tolerance <= 0.0 {
            return Err(ResidualRecordError::InvalidTolerance);
        }
        if self.records.is_empty() {
            return Err(ResidualRecordError::EmptyRecords);
        }
        for record in &self.records {
            record.validate()?;
        }
        Ok(())
    }
}

/// The kind of a residual finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResidualFindingKind {
    /// The cone is VIOLATED at the recorded point: `g_ij < −tolerance`.
    ConeViolated,
    /// The relaxation is LOOSE (not tight) at the recorded point:
    /// `g_ij > +tolerance` — exactness is not demonstrated there.
    RelaxationLoose,
}

impl ResidualFindingKind {
    /// The frozen finding-kind code.
    pub const fn code(self) -> &'static str {
        match self {
            ResidualFindingKind::ConeViolated => "cone_violated",
            ResidualFindingKind::RelaxationLoose => "relaxation_loose",
        }
    }
}

/// One recorded residual finding. Findings are NEVER hidden: they travel
/// to the verdict evidence (P4.07) and to the P3.18 cross-check dimension
/// (D-P4.09-4). A finding does not fail the gate by itself — the
/// independent AC power-flow verification decides — and it does not
/// disappear.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResidualFinding {
    /// The branch the finding is about.
    pub branch: String,
    /// The cone name.
    pub cone_name: String,
    /// The recorded residual value.
    pub residual: f64,
    /// The tolerance it was assessed against.
    pub tolerance: f64,
    /// The finding kind.
    pub kind: ResidualFindingKind,
    /// The recorded statement (audit text).
    pub statement: String,
}

/// The assessment of a valid residual report against its tolerance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResidualAssessment {
    /// Every recorded finding (empty when the relaxation is tight within
    /// tolerance on every branch).
    pub findings: Vec<ResidualFinding>,
    /// True exactly when there are no findings.
    pub all_within_tolerance: bool,
    /// The rendered assessment summary.
    pub summary: String,
}

/// Assesses a VALID report (call [`RelaxationResidualReport::validate`]
/// first): per branch, `g_ij < −tolerance` is a [`ResidualFindingKind::
/// ConeViolated`] finding and `g_ij > +tolerance` is a
/// [`ResidualFindingKind::RelaxationLoose`] finding. Everything in between
/// (including the boundary values ±tolerance) is tight within tolerance.
pub fn assess_residuals(report: &RelaxationResidualReport) -> ResidualAssessment {
    let mut findings = Vec::new();
    for record in &report.records {
        let kind = if record.residual < -report.tolerance {
            Some(ResidualFindingKind::ConeViolated)
        } else if record.residual > report.tolerance {
            Some(ResidualFindingKind::RelaxationLoose)
        } else {
            None
        };
        if let Some(kind) = kind {
            let statement = match kind {
                ResidualFindingKind::ConeViolated => format!(
                    "branch '{}': residual {:+e} < −{:+e} — the DistFlow cone is \
                     VIOLATED at the recorded point ({})",
                    record.branch, record.residual, report.tolerance, DISTFLOW_RESIDUAL_FORMULA
                ),
                ResidualFindingKind::RelaxationLoose => format!(
                    "branch '{}': residual {:+e} > +{:+e} — the relaxation is LOOSE \
                     (not tight); exactness is not demonstrated there ({})",
                    record.branch, record.residual, report.tolerance, DISTFLOW_RESIDUAL_FORMULA
                ),
            };
            findings.push(ResidualFinding {
                branch: record.branch.clone(),
                cone_name: record.cone_name.clone(),
                residual: record.residual,
                tolerance: report.tolerance,
                kind,
                statement,
            });
        }
    }
    let all_within_tolerance = findings.is_empty();
    let summary = if all_within_tolerance {
        format!(
            "all {} recorded branch residual(s) within ±{:+e} of zero — the \
             relaxation is tight within tolerance on every recorded branch",
            report.records.len(),
            report.tolerance
        )
    } else {
        format!(
            "{} of {} recorded branch residual(s) outside ±{:+e} — findings \
             recorded, never hidden; the independent AC power-flow \
             verification remains the authority",
            findings.len(),
            report.records.len(),
            report.tolerance
        )
    };
    ResidualAssessment {
        findings,
        all_within_tolerance,
        summary,
    }
}

// ---------------------------------------------------------------------------
// The boundary gate
// ---------------------------------------------------------------------------

/// The result of the SOCP boundary check for one candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "boundary")]
pub enum SocpBoundaryGate {
    /// The candidate is Tier 1: no relaxation boundary applies; the AC
    /// verification runs unchanged.
    NotTier2,
    /// The candidate is Tier 2 AND carries a valid residual record: the AC
    /// verification runs (it always does), the caveat and findings travel
    /// onto the verdict evidence (P4.07).
    Tier2VerifiedForAcCheck {
        /// The rendered condition checklist.
        condition_summary: String,
        /// The rendered residual assessment.
        residual_summary: String,
        /// The caveats for the verdict evidence — ALWAYS contains
        /// [`SOCP_EXACTNESS_CAVEAT`] (D-P4.09-2).
        caveats: Vec<String>,
        /// The recorded findings (condition gaps + residual findings),
        /// never hidden.
        findings: Vec<String>,
    },
    /// The candidate is Tier 2 WITHOUT a usable residual record: FAIL-
    /// CLOSED (the P4.08 trigger `RELAXATION_RECORD_MISSING`; the P4.07
    /// routing maps this to the recorded REJECT outcome).
    FailClosed(FailClosedOutcome),
}

/// Gates the SOCP relaxation boundary for one candidate (ledger P4.09;
/// D-P4.09-1): Tier 1 passes through; Tier 2 requires a valid residual
/// record (missing/failed → fail-closed) and produces the caveats and
/// findings that travel. The AC verification itself is NOT optional for
/// either tier — this gate only decides whether a Tier 2 candidate may
/// enter it with its relaxation evidence attached.
pub fn gate_socp_boundary(
    provenance: &TierProvenance,
    conditions: &RelaxationConditionCheck,
    residuals: Option<&RelaxationResidualReport>,
) -> SocpBoundaryGate {
    match provenance {
        TierProvenance::Tier1Linearized => SocpBoundaryGate::NotTier2,
        TierProvenance::Tier2SocpRelaxation => {
            let report = match residuals {
                Some(report) => report,
                None => {
                    return SocpBoundaryGate::FailClosed(fail_closed_outcome(
                        FailClosedTrigger::RelaxationRecordMissing,
                        "the Tier 2 candidate carries no relaxation-residual record; \
                         a relaxation solution may be inexact and is never accepted \
                         on the solver's word (§12.5)",
                    ));
                }
            };
            if let Err(error) = report.validate() {
                return SocpBoundaryGate::FailClosed(fail_closed_outcome(
                    FailClosedTrigger::RelaxationRecordMissing,
                    &format!("the attached relaxation-residual record is unusable: {error}"),
                ));
            }
            let assessment = assess_residuals(report);
            let mut findings: Vec<String> = Vec::new();
            for record in &conditions.records {
                match &record.outcome {
                    ConditionOutcome::Satisfied => {}
                    ConditionOutcome::NotSatisfied { statement } => {
                        findings.push(format!(
                            "condition {}: {}",
                            record.condition.code(),
                            statement
                        ));
                    }
                    ConditionOutcome::NotAssumedExact { statement } => {
                        findings.push(format!(
                            "condition {}: {}",
                            record.condition.code(),
                            statement
                        ));
                    }
                }
            }
            for finding in &assessment.findings {
                findings.push(finding.statement.clone());
            }
            SocpBoundaryGate::Tier2VerifiedForAcCheck {
                condition_summary: conditions.summary(),
                residual_summary: assessment.summary,
                caveats: vec![SOCP_EXACTNESS_CAVEAT.to_string()],
                findings,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The P3.18 cross-check dimension (D-P4.09-4)
// ---------------------------------------------------------------------------

/// The feasibility-verdict dimension of the P3.18 Tier 1-vs-Tier 2
/// comparison record. The comparison record's objective and binding sets
/// already exist in flexcore-optimization-api; THIS is the added
/// dimension. The composition root attaches it (see the `note` field).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeasibilityVerdictDimension {
    /// The candidate's tier provenance code.
    pub tier: String,
    /// The gate verdict's canonical outcome code (P4.07
    /// `GateVerdict::outcome_code`).
    pub gate_outcome_code: String,
    /// Every violated constraint the gate named (from the P4.06 report).
    pub violated_constraints: Vec<String>,
    /// The gate's worst margin fraction across all categories, when a
    /// limit check ran (from the P4.06 diagnostics).
    pub worst_margin_fraction: Option<f64>,
    /// The caveats that travelled on the verdict evidence (for Tier 2,
    /// always contains [`SOCP_EXACTNESS_CAVEAT`]).
    pub caveats: Vec<String>,
    /// The recorded attachment note.
    pub note: String,
}

/// Builds the feasibility-verdict dimension for the P3.18 comparison
/// record (D-P4.09-4). The inputs are the gate's own outputs; the
/// composition root renders the verdict outcome code and attaches the
/// dimension to the comparison record.
///
/// VERIFY-BUILD: flexcore-grid-api cannot name flexcore-optimization-api
/// (the core-crate dependency policy), so the exact attachment point on
/// `Tier1Tier2ComparisonRecord` — a dedicated field versus an entry in the
/// record's `notes` — is finalized when flexcore-optimization-api consumes
/// this type; until then the composition root renders it into the
/// record's `notes`. The dimension itself is complete.
pub fn feasibility_verdict_dimension(
    provenance: &TierProvenance,
    gate_outcome_code: &str,
    violated_constraints: &[String],
    worst_margin_fraction: Option<f64>,
    caveats: &[String],
) -> FeasibilityVerdictDimension {
    FeasibilityVerdictDimension {
        tier: provenance.code().to_string(),
        gate_outcome_code: gate_outcome_code.to_string(),
        violated_constraints: violated_constraints.to_vec(),
        worst_margin_fraction,
        caveats: caveats.to_vec(),
        note: "feasibility-verdict dimension for the P3.18 Tier 1-vs-Tier 2 \
               comparison record (D-P4.09-4): the comparison is diagnostic; \
               neither tier's objective or binding set is a feasibility \
               verdict — THIS dimension is the gate's evidence-backed outcome"
            .to_string(),
    }
}

// ---------------------------------------------------------------------------
// In-crate unit tests (the integration tests live in tests/socp_boundary.rs)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn record(branch: &str, residual: f64) -> RelaxationResidualRecord {
        RelaxationResidualRecord {
            branch: branch.to_string(),
            cone_name: "cone_12".to_string(),
            residual,
        }
    }

    #[test]
    fn boundary_values_are_within_tolerance() {
        let report = RelaxationResidualReport {
            records: vec![record("12", 1.0e-4), record("23", -1.0e-4)],
            tolerance: 1.0e-4,
        };
        let assessment = assess_residuals(&report);
        assert!(assessment.all_within_tolerance);
        assert!(assessment.findings.is_empty());
    }

    #[test]
    fn violation_and_looseness_are_findings() {
        let report = RelaxationResidualReport {
            records: vec![record("12", -2.0e-3), record("23", 5.0e-3)],
            tolerance: 1.0e-4,
        };
        let assessment = assess_residuals(&report);
        assert!(!assessment.all_within_tolerance);
        assert_eq!(assessment.findings.len(), 2);
        assert_eq!(assessment.findings[0].kind, ResidualFindingKind::ConeViolated);
        assert_eq!(assessment.findings[1].kind, ResidualFindingKind::RelaxationLoose);
    }

    #[test]
    fn malformed_records_are_rejected() {
        assert!(record("", 0.0).validate().is_err());
        assert!(record("12", f64::NAN).validate().is_err());
        let empty = RelaxationResidualReport {
            records: Vec::new(),
            tolerance: 1.0e-4,
        };
        assert_eq!(empty.validate(), Err(ResidualRecordError::EmptyRecords));
        let bad_tolerance = RelaxationResidualReport {
            records: vec![record("12", 0.0)],
            tolerance: 0.0,
        };
        assert_eq!(
            bad_tolerance.validate(),
            Err(ResidualRecordError::InvalidTolerance)
        );
    }
}
