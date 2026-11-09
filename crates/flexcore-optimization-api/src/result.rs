// P3.17 — The §12.6 optimization result contract (report §12.6, §9.2).
//
// Every solve returns this record; a result without provenance and
// diagnostics is not eligible for operational consideration (§9.2). The
// status vocabulary is **closed** — exactly the six §12.6 strings, no others
// — and [`OptimizationResult::validate_complete`] enforces field
// completeness so P3.21's persistence/completeness wiring builds on a
// contract that cannot be satisfied by a half-filled record.
//
// P3.21 completes the §12.6 wiring (persistence, event, snapshot plumbing);
// the contract types themselves live here so the port can return them.
//
// This module contains **no solver-specific type**: solver outcomes arrive
// through the provider already mapped onto this vocabulary.

use flexcore_contracts::ids::FlexUuid;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Solver outcome status — the §12.6 vocabulary, closed.
///
/// Exactly six statuses exist: `optimal / feasible / infeasible / timeout /
/// numerical_failure / unsupported`. No other status string is legal
/// (ledger P3.19 step 2); the `as_str`/`parse` pair and the serialization
/// rename keep the on-the-wire and in-memory vocabularies identical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SolveStatus {
    /// Proven optimal for the declared problem class.
    Optimal,
    /// A feasible solution was returned without a proven-optimality claim
    /// (e.g. a MIP terminated within its gap).
    Feasible,
    /// No feasible solution exists; infeasibility evidence is carried on the
    /// result. Never accompanied by a relaxed schedule (P3.19 step 3).
    Infeasible,
    /// A resource limit (time or iteration budget) stopped the solve.
    Timeout,
    /// The solver failed numerically, reported an error, or ended in a state
    /// that cannot be classified into the meaningful statuses above
    /// (e.g. an unbounded objective — see the decision note §3).
    NumericalFailure,
    /// The provider does not support the declared problem class; no solve
    /// was attempted (e.g. an SOCP formulation must never reach an LP/QP/MIP
    /// solver — report §12.3).
    Unsupported,
}

impl SolveStatus {
    /// The exact §12.6 status string.
    pub fn as_str(self) -> &'static str {
        match self {
            SolveStatus::Optimal => "optimal",
            SolveStatus::Feasible => "feasible",
            SolveStatus::Infeasible => "infeasible",
            SolveStatus::Timeout => "timeout",
            SolveStatus::NumericalFailure => "numerical_failure",
            SolveStatus::Unsupported => "unsupported",
        }
    }

    /// Parses the exact §12.6 status string. There is no lossy parsing: any
    /// other string is `None`, because the vocabulary is closed.
    pub fn parse(text: &str) -> Option<SolveStatus> {
        let status = match text {
            "optimal" => SolveStatus::Optimal,
            "feasible" => SolveStatus::Feasible,
            "infeasible" => SolveStatus::Infeasible,
            "timeout" => SolveStatus::Timeout,
            "numerical_failure" => SolveStatus::NumericalFailure,
            "unsupported" => SolveStatus::Unsupported,
            _ => return None,
        };
        Some(status)
    }
}

/// A diagnostic block: either available with its payload, or explicitly
/// not available with a recorded reason (§12.6 "where available"; P3.21
/// acceptance: "explicitly 'not available' with a reason per solver").
///
/// A `NotAvailable` with an empty reason is incomplete and fails
/// [`OptimizationResult::validate_complete`] — silence is never legal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Diagnostics<T> {
    /// The diagnostic is available.
    Available(T),
    /// The diagnostic is not available for this solve; the reason is
    /// recorded (non-empty enforced by completeness validation).
    NotAvailable { reason: String },
}

impl<T> Diagnostics<T> {
    /// `true` when [`Diagnostics::Available`].
    pub fn is_available(&self) -> bool {
        matches!(self, Diagnostics::Available(_))
    }

    /// The recorded not-available reason, when not available.
    pub fn not_available_reason(&self) -> Option<&str> {
        match self {
            Diagnostics::Available(_) => None,
            Diagnostics::NotAvailable { reason } => Some(reason.as_str()),
        }
    }
}

/// One solved variable value with its declared identity and unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariableValue {
    /// Variable name (from the formulation).
    pub name: String,
    /// Declared canonical unit (from the formulation).
    pub unit: String,
    /// Primal value.
    pub value: f64,
}

/// Primal diagnostics: the solution vector, aligned with the formulation's
/// variables (the provider checks the alignment and refuses to publish a
/// misaligned vector).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrimalDiagnostics {
    /// One value per formulation variable, in formulation order.
    pub values: Vec<VariableValue>,
}

/// Dual diagnostics: reduced costs (per variable) and row duals (per row),
/// when the solver exposes them for the solve.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DualDiagnostics {
    /// Reduced cost per variable, in formulation order.
    pub reduced_costs: Vec<f64>,
    /// Dual value per linear row, in formulation order.
    pub row_duals: Vec<f64>,
}

/// Constraint residuals / violations and solve effort (§12.6 "constraint
/// residuals and violations", §9.2 "execution time and resource use").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResidualReport {
    /// Maximum primal constraint violation reported by the solver, if
    /// reported.
    pub max_primal_violation: Option<f64>,
    /// Maximum dual infeasibility reported by the solver, if reported.
    pub max_dual_violation: Option<f64>,
    /// Solver iteration count, if reported.
    pub iteration_count: Option<u64>,
}

/// Infeasibility evidence returned with an `infeasible` result (P3.19 step 3:
/// conflict evidence where the solver exposes it, never a relaxed schedule).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InfeasibilityEvidence {
    /// Names of variables the solver's conflict refinement implicated.
    pub conflict_variables: Vec<String>,
    /// Names of rows the solver's conflict refinement implicated.
    pub conflict_rows: Vec<String>,
    /// The verbatim backend evidence report, for audit.
    pub backend_report: String,
}

/// Determinism metadata recorded with every result (§19.2 layer 5; P3.19).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeterminismRecord {
    /// Pinned solver version used for the solve.
    pub solver_version: String,
    /// Fixed thread count used for the solve (default 1).
    pub thread_count: u32,
    /// Configured time limit in milliseconds, when set.
    pub time_limit_ms: Option<u64>,
    /// Digest of the solver configuration (stable serialization + FNV-1a 64;
    /// see [`crate::config::SolverConfig::config_digest`]).
    pub config_digest: String,
}

/// The complete optimization result record (report §12.6, §9.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptimizationResult {
    /// Solver outcome status (closed six-status vocabulary).
    pub status: SolveStatus,
    /// The declared problem class this result was produced for.
    pub problem_class: crate::problem::ProblemClass,
    /// Solver name (identity of the pinned solver behind the provider).
    pub solver_name: String,
    /// Pinned solver version.
    pub solver_version: String,
    /// Provider identifier.
    pub provider_id: String,
    /// Provider version.
    pub provider_version: String,

    /// Objective value, when a solution was produced.
    pub objective_value: Option<f64>,
    /// Declared objective units (present whenever `objective_value` is).
    pub objective_units: Option<String>,

    /// Primal diagnostics.
    pub primal: Diagnostics<PrimalDiagnostics>,
    /// Dual diagnostics.
    pub dual: Diagnostics<DualDiagnostics>,
    /// Residuals / violations / effort.
    pub residuals: Diagnostics<ResidualReport>,
    /// Infeasibility evidence (populated for `infeasible` results).
    pub infeasibility_evidence: Diagnostics<InfeasibilityEvidence>,

    /// Wall-clock runtime of the solve.
    pub runtime: Duration,
    /// Resource-use note when the backend reports one (§9.2).
    pub resources: Option<String>,

    /// Candidate identity (§12.6 "output candidate ID").
    pub candidate_id: Option<FlexUuid>,
    /// Input snapshot reference (§12.6 "input snapshot").
    pub input_snapshot_id: Option<String>,
    /// Data timestamp of the input snapshot, when known (§9.2).
    pub data_timestamp: Option<time::OffsetDateTime>,
    /// Grid-model version traceable to the Phase 2 Site grid-model reference,
    /// when known (§12.6; P3.21 completes the required wiring).
    pub grid_model_version: Option<String>,

    /// Assumptions and uncertainty declared with the formulation (§9.2).
    pub uncertainty_and_assumptions: Vec<String>,
    /// Warnings emitted during formulation or mapping. Never empty without
    /// reason — providers add warnings for every degraded or ambiguous
    /// outcome.
    pub warnings: Vec<String>,
    /// Determinism metadata (pinned version, thread count, config digest).
    pub determinism: Option<DeterminismRecord>,
}

/// Completeness violations of the §12.6 result contract. P3.21's
/// completeness test builds directly on this error set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultCompletenessError {
    /// Candidate identity missing.
    MissingCandidateId,
    /// Input snapshot reference missing or empty.
    MissingInputSnapshotId,
    /// A solver/provider identification field is empty (`field` names it).
    EmptyIdentification { field: &'static str },
    /// Determinism metadata missing (pinned version + thread count must be
    /// recorded on every result; §19.2 layer 5).
    MissingDeterminismRecord,
    /// Objective value present without units, or units present without a
    /// value where a value is required.
    ObjectiveIncomplete { reason: String },
    /// A diagnostic is `NotAvailable` with an empty reason — silence is not
    /// a recorded reason.
    EmptyNotAvailableReason { diagnostic: &'static str },
    /// An `optimal`/`feasible` result must carry primal diagnostics.
    MissingPrimalDiagnostics { status: SolveStatus },
}

impl std::fmt::Display for ResultCompletenessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResultCompletenessError::MissingCandidateId => {
                write!(f, "result is missing the candidate id (§12.6)")
            }
            ResultCompletenessError::MissingInputSnapshotId => {
                write!(f, "result is missing the input snapshot id (§12.6)")
            }
            ResultCompletenessError::EmptyIdentification { field } => {
                write!(f, "result identification field '{field}' is empty")
            }
            ResultCompletenessError::MissingDeterminismRecord => {
                write!(
                    f,
                    "result is missing the determinism record (pinned version + thread count)"
                )
            }
            ResultCompletenessError::ObjectiveIncomplete { reason } => {
                write!(f, "objective incomplete: {reason}")
            }
            ResultCompletenessError::EmptyNotAvailableReason { diagnostic } => {
                write!(
                    f,
                    "diagnostic '{diagnostic}' is not available but no reason is recorded"
                )
            }
            ResultCompletenessError::MissingPrimalDiagnostics { status } => {
                write!(f, "a {} result must carry primal diagnostics", status.as_str())
            }
        }
    }
}

impl std::error::Error for ResultCompletenessError {}

impl OptimizationResult {
    /// Validates §12.6 completeness: a result missing any required field
    /// fails with a [`ResultCompletenessError`]. Required at the port level:
    /// candidate ID, input snapshot, solver/provider identification, and the
    /// determinism record; per-status requirements add the objective and
    /// primal diagnostics for `optimal`/`feasible` results, and forbid
    /// empty not-available reasons everywhere.
    pub fn validate_complete(&self) -> Result<(), ResultCompletenessError> {
        let candidate_ok = self
            .candidate_id
            .is_some_and(|id| id != flexcore_contracts::ids::FlexUuid::NIL);
        if !candidate_ok {
            return Err(ResultCompletenessError::MissingCandidateId);
        }

        if self
            .input_snapshot_id
            .as_deref()
            .is_none_or(|id| id.trim().is_empty())
        {
            return Err(ResultCompletenessError::MissingInputSnapshotId);
        }

        for (field, value) in [
            ("solver_name", self.solver_name.as_str()),
            ("solver_version", self.solver_version.as_str()),
            ("provider_id", self.provider_id.as_str()),
            ("provider_version", self.provider_version.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(ResultCompletenessError::EmptyIdentification { field });
            }
        }

        if self.determinism.is_none() {
            return Err(ResultCompletenessError::MissingDeterminismRecord);
        }

        for (diagnostic, reason) in [
            ("primal", self.primal.not_available_reason()),
            ("dual", self.dual.not_available_reason()),
            ("residuals", self.residuals.not_available_reason()),
            (
                "infeasibility_evidence",
                self.infeasibility_evidence.not_available_reason(),
            ),
        ] {
            if let Some(reason) = reason {
                if reason.trim().is_empty() {
                    return Err(ResultCompletenessError::EmptyNotAvailableReason {
                        diagnostic,
                    });
                }
            }
        }

        if matches!(self.status, SolveStatus::Optimal | SolveStatus::Feasible) {
            match (self.objective_value, self.objective_units.as_deref()) {
                (Some(_), Some(units)) if !units.trim().is_empty() => {}
                (Some(_), _) => {
                    return Err(ResultCompletenessError::ObjectiveIncomplete {
                        reason: "objective value present without declared units".to_string(),
                    });
                }
                (None, _) => {
                    return Err(ResultCompletenessError::ObjectiveIncomplete {
                        reason: format!(
                            "a {} result must carry the objective value",
                            self.status.as_str()
                        ),
                    });
                }
            }
            if !self.primal.is_available() {
                return Err(ResultCompletenessError::MissingPrimalDiagnostics {
                    status: self.status,
                });
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The closed vocabulary: exactly these six strings, no others are
    /// parseable, and every variant round-trips.
    #[test]
    fn status_vocabulary_is_exactly_the_six_strings() {
        let all = [
            (SolveStatus::Optimal, "optimal"),
            (SolveStatus::Feasible, "feasible"),
            (SolveStatus::Infeasible, "infeasible"),
            (SolveStatus::Timeout, "timeout"),
            (SolveStatus::NumericalFailure, "numerical_failure"),
            (SolveStatus::Unsupported, "unsupported"),
        ];
        for (status, text) in all {
            assert_eq!(status.as_str(), text);
            assert_eq!(SolveStatus::parse(text), Some(status));
        }
        // The enum has no other variants: exhaustiveness proves it.
        let exhaustive = match SolveStatus::Optimal {
            SolveStatus::Optimal
            | SolveStatus::Feasible
            | SolveStatus::Infeasible
            | SolveStatus::Timeout
            | SolveStatus::NumericalFailure
            | SolveStatus::Unsupported => "closed",
        };
        assert_eq!(exhaustive, "closed");
        for illegal in [
            "",
            "OPTIMAL",
            "Optimal",
            "unbounded",
            "error",
            "not_solved",
            "optimal ",
        ] {
            assert_eq!(SolveStatus::parse(illegal), None, "illegal: {illegal:?}");
        }
    }

    #[test]
    fn status_serialization_uses_the_canonical_strings() {
        let text = serde_json::to_string(&SolveStatus::NumericalFailure).expect("serializes");
        assert_eq!(text, "\"numerical_failure\"");
        let back: SolveStatus =
            serde_json::from_str(&text).expect("deserializes");
        assert_eq!(back, SolveStatus::NumericalFailure);
    }

    #[test]
    fn diagnostics_expose_their_not_available_reason() {
        let available: Diagnostics<u8> = Diagnostics::Available(1);
        assert!(available.is_available());
        assert!(available.not_available_reason().is_none());

        let unavailable: Diagnostics<u8> = Diagnostics::NotAvailable {
            reason: "solver did not return duals".to_string(),
        };
        assert!(!unavailable.is_available());
        assert_eq!(
            unavailable.not_available_reason(),
            Some("solver did not return duals")
        );
    }
}
