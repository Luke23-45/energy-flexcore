// P3.19 — The infeasibility-evidence policy (ledger P3.19 step 3; report
// §12.6 status vocabulary, §9.2 diagnostics channel).
//
// POLICY (stated once, verbatim, as [`INFEASIBILITY_POLICY`]): an infeasible
// run returns CONFLICT EVIDENCE — which constraints conflict, per the
// solver's diagnostics where available — and NEVER a silently relaxed
// schedule. Where the pinned solver cannot provide evidence (the Tier 2
// conic solver exposes no conflict/IIS refinement; the Tier 1 evidence
// depends on the pinned binding's API surface — VERIFY-BUILD item 2 of
// decision note 17_optimizer_port.md), the record carries an explicit
// not-available reason, never silence.
//
// Relaxation: ANY relaxation of a declared-hard constraint (e.g. softening
// the EV departure-energy requirement — `flexcore-domain`'s
// `DepartureEnergyPolicy::Hard`, the ONLY variant of that policy) requires a
// NAMED, REVIEWED decision recorded in the schedule. This module exposes the
// types the schedule record (P3.21/P3.22) embeds — [`RelaxationDecision`]
// and [`RelaxedScheduleRecord`] — and [`RelaxedScheduleRecord::new`] is the
// ONLY way to construct a relaxed-schedule record: there is no code path
// that applies a relaxation without a complete, validated decision.
//
// This module contains no solver-specific type; evidence arrives already
// mapped onto the port's result vocabulary. The name `InfeasibilityEvidence`
// here is the POLICY-level evidence record (conflicting constraint refs +
// solver excerpt + explicit not-available reason); the per-result payload on
// `crate::result::InfeasibilityEvidence` remains the §12.6 field type, and
// [`evidence_from_result`] bridges the two.

use crate::result::{Diagnostics, OptimizationResult};
use flexcore_contracts::ids::FlexUuid;
use serde::{Deserialize, Serialize};

/// The infeasibility policy, stated once (ledger P3.19 step 3).
pub const INFEASIBILITY_POLICY: &str = "An infeasible run returns CONFLICT \
     EVIDENCE (which constraints conflict, per the solver's diagnostics where \
     available) and NEVER a silently relaxed schedule. Where the solver cannot \
     provide evidence, the record carries an explicit not-available reason. \
     Any relaxation of a declared-hard constraint (e.g. the EV \
     DepartureEnergyPolicy::Hard departure-energy requirement) requires a \
     NAMED, REVIEWED decision recorded in the schedule (RelaxationDecision); \
     there is no code path that applies a relaxation without one.";

/// Policy-level infeasibility evidence (ledger P3.19 step 3): either the
/// conflict evidence the solver exposed, or an explicit not-available
/// reason. Exactly one of the two is present — validated by
/// [`InfeasibilityEvidence::validate`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfeasibilityEvidence {
    /// The conflicting constraint references (row names, cone names, or
    /// variable-bound references as the solver's conflict refinement
    /// reported them; empty when no evidence is available).
    pub conflicting_constraints: Vec<String>,
    /// The verbatim solver diagnostics excerpt, for audit (empty when no
    /// evidence is available).
    pub solver_excerpt: String,
    /// The explicit not-available reason when the solver could not provide
    /// evidence (`Some` ⟹ no evidence is present, by the validate rule).
    pub not_available_reason: Option<String>,
}

impl InfeasibilityEvidence {
    /// Evidence available: at least one conflicting constraint reference and
    /// a verbatim solver excerpt.
    pub fn from_solver(
        conflicting_constraints: Vec<String>,
        solver_excerpt: impl Into<String>,
    ) -> Self {
        Self {
            conflicting_constraints,
            solver_excerpt: solver_excerpt.into(),
            not_available_reason: None,
        }
    }

    /// Evidence explicitly not available, with the recorded reason (never
    /// silence).
    pub fn not_available(reason: impl Into<String>) -> Self {
        Self {
            conflicting_constraints: Vec::new(),
            solver_excerpt: String::new(),
            not_available_reason: Some(reason.into()),
        }
    }

    /// `true` when the solver's conflict evidence is present.
    pub fn is_available(&self) -> bool {
        self.not_available_reason.is_none()
    }

    /// Validates the exactly-one-of-two rule: available evidence must name
    /// at least one conflicting constraint and carry a non-empty solver
    /// excerpt; not-available evidence must record a non-empty reason.
    pub fn validate(&self) -> Result<(), InfeasibilityPolicyError> {
        match &self.not_available_reason {
            Some(reason) => {
                if reason.trim().is_empty() {
                    return Err(InfeasibilityPolicyError::EmptyNotAvailableReason);
                }
                if !self.conflicting_constraints.is_empty() || !self.solver_excerpt.is_empty() {
                    return Err(InfeasibilityPolicyError::EvidenceAndReasonBothPresent);
                }
                Ok(())
            }
            None => {
                if self.conflicting_constraints.is_empty() || self.solver_excerpt.trim().is_empty()
                {
                    return Err(InfeasibilityPolicyError::EvidenceIncomplete);
                }
                Ok(())
            }
        }
    }
}

/// Violations of the infeasibility/relaxation policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InfeasibilityPolicyError {
    /// A not-available evidence record carries an empty reason — silence is
    /// never a recorded reason.
    EmptyNotAvailableReason,
    /// An evidence record carries BOTH solver evidence and a not-available
    /// reason; exactly one of the two is legal.
    EvidenceAndReasonBothPresent,
    /// An available evidence record names no conflicting constraint or
    /// carries no solver excerpt.
    EvidenceIncomplete,
    /// A relaxation decision is incomplete (`field` names the missing or
    /// empty part): a decision must be NAMED and REVIEWED (a non-empty
    /// review reference) and must name what it relaxes.
    IncompleteRelaxationDecision { field: &'static str },
    /// An attempt to record a relaxed schedule whose applied constraint
    /// references are not covered by the reviewed decision — a relaxation
    /// beyond the reviewed scope is never legal.
    RelaxationBeyondReviewedScope { constraint: String },
}

impl std::fmt::Display for InfeasibilityPolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InfeasibilityPolicyError::EmptyNotAvailableReason => {
                write!(f, "infeasibility evidence is not available but no reason is recorded (silence is never legal — P3.19)")
            }
            InfeasibilityPolicyError::EvidenceAndReasonBothPresent => {
                write!(f, "infeasibility evidence carries both solver evidence and a not-available reason; exactly one is legal")
            }
            InfeasibilityPolicyError::EvidenceIncomplete => {
                write!(f, "available infeasibility evidence must name at least one conflicting constraint and carry a solver excerpt")
            }
            InfeasibilityPolicyError::IncompleteRelaxationDecision { field } => {
                write!(
                    f,
                    "relaxation decision is incomplete: '{field}' is missing or empty \
                     (a decision must be named and reviewed — P3.19 step 3)"
                )
            }
            InfeasibilityPolicyError::RelaxationBeyondReviewedScope { constraint } => {
                write!(
                    f,
                    "relaxation applied to constraint '{constraint}' is not covered by \
                     the reviewed decision's scope"
                )
            }
        }
    }
}

impl std::error::Error for InfeasibilityPolicyError {}

/// A NAMED, REVIEWED relaxation decision (ledger P3.19 step 3, referencing
/// P3.11): the record embedded in the schedule whenever a declared-hard
/// constraint (e.g. `DepartureEnergyPolicy::Hard`) is softened. All fields
/// are mandatory and validated — an unnamed or unreviewed relaxation cannot
/// be represented.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelaxationDecision {
    /// The decision's name (non-empty) — the identifier the review and the
    /// schedule reference.
    pub decision_name: String,
    /// The review reference (non-empty) — a pointer to the recorded review
    /// that approved the relaxation (decision record, ticket, or review
    /// minute). A relaxation without a review reference is not a decision.
    pub review_reference: String,
    /// The constraint references the decision authorizes relaxing
    /// (non-empty; the scope boundary for [`RelaxedScheduleRecord`]).
    pub relaxed_constraint_refs: Vec<String>,
    /// The rationale (non-empty) — why the relaxation is operationally
    /// acceptable, recorded for audit.
    pub rationale: String,
}

impl RelaxationDecision {
    /// Validates the decision: name, review reference, rationale, and scope
    /// all present (non-empty), with at least one authorized constraint
    /// reference.
    pub fn validate(&self) -> Result<(), InfeasibilityPolicyError> {
        if self.decision_name.trim().is_empty() {
            return Err(InfeasibilityPolicyError::IncompleteRelaxationDecision {
                field: "decision_name",
            });
        }
        if self.review_reference.trim().is_empty() {
            return Err(InfeasibilityPolicyError::IncompleteRelaxationDecision {
                field: "review_reference",
            });
        }
        if self.rationale.trim().is_empty() {
            return Err(InfeasibilityPolicyError::IncompleteRelaxationDecision {
                field: "rationale",
            });
        }
        if self
            .relaxed_constraint_refs
            .iter()
            .any(|reference| reference.trim().is_empty())
            || self.relaxed_constraint_refs.is_empty()
        {
            return Err(InfeasibilityPolicyError::IncompleteRelaxationDecision {
                field: "relaxed_constraint_refs",
            });
        }
        Ok(())
    }
}

/// The relaxed-schedule record the schedule record (P3.21/P3.22) embeds when
/// — and ONLY when — a named, reviewed relaxation was applied. Construction
/// goes exclusively through [`RelaxedScheduleRecord::new`], which validates
/// the decision and enforces that every applied relaxation is within the
/// reviewed scope: there is no code path that applies a relaxation without a
/// complete decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelaxedScheduleRecord {
    /// The candidate identity of the schedule carrying this relaxation
    /// (P3.22 assigns the typed schedule identity; this is the reference the
    /// relaxation is recorded against).
    pub base_candidate_id: FlexUuid,
    /// The named, reviewed decision, embedded verbatim.
    pub decision: RelaxationDecision,
    /// The constraint references actually softened in the produced
    /// formulation (each must be within `decision.relaxed_constraint_refs`).
    pub applied_to: Vec<String>,
}

impl RelaxedScheduleRecord {
    /// The ONLY constructor of a relaxed-schedule record: requires a valid,
    /// reviewed decision and an applied set within the reviewed scope.
    pub fn new(
        base_candidate_id: FlexUuid,
        decision: RelaxationDecision,
        applied_to: Vec<String>,
    ) -> Result<Self, InfeasibilityPolicyError> {
        decision.validate()?;
        for constraint in &applied_to {
            if !decision
                .relaxed_constraint_refs
                .iter()
                .any(|authorized| authorized == constraint)
            {
                return Err(
                    InfeasibilityPolicyError::RelaxationBeyondReviewedScope {
                        constraint: constraint.clone(),
                    },
                );
            }
        }
        if applied_to.is_empty() {
            return Err(InfeasibilityPolicyError::IncompleteRelaxationDecision {
                field: "applied_to",
            });
        }
        Ok(Self {
            base_candidate_id,
            decision,
            applied_to,
        })
    }
}

/// Bridges a §12.6 result record's infeasibility diagnostic (the
/// `Diagnostics<result::InfeasibilityEvidence>` the P3.17/P3.18 providers
/// produce) onto this module's policy-level evidence. An infeasible result
/// always carries either evidence or an explicit not-available reason (the
/// providers enforce this); the bridge rejects anything else with the
/// policy error.
pub fn evidence_from_result(
    result: &OptimizationResult,
) -> Result<InfeasibilityEvidence, InfeasibilityPolicyError> {
    match &result.infeasibility_evidence {
        Diagnostics::Available(evidence) => Ok(InfeasibilityEvidence::from_solver(
            evidence
                .conflict_rows
                .iter()
                .chain(evidence.conflict_variables.iter())
                .cloned()
                .collect(),
            evidence.backend_report.clone(),
        )),
        Diagnostics::NotAvailable { reason } => {
            Ok(InfeasibilityEvidence::not_available(reason.clone()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate() -> FlexUuid {
        FlexUuid::parse("00000000-0000-0000-0000-0000000c4d1d").expect("parses")
    }

    #[test]
    fn evidence_validate_enforces_exactly_one_of_two() {
        let available =
            InfeasibilityEvidence::from_solver(vec!["export_floor".to_string()], "IIS: rows 1, 2");
        assert!(available.is_available());
        available.validate().expect("complete evidence");

        let unavailable =
            InfeasibilityEvidence::not_available("the pinned conic solver exposes no IIS");
        assert!(!unavailable.is_available());
        unavailable.validate().expect("explicit reason");

        // Silence is never legal.
        let silent = InfeasibilityEvidence {
            conflicting_constraints: Vec::new(),
            solver_excerpt: String::new(),
            not_available_reason: Some("   ".to_string()),
        };
        assert_eq!(
            silent.validate(),
            Err(InfeasibilityPolicyError::EmptyNotAvailableReason)
        );

        // Both present is never legal.
        let both = InfeasibilityEvidence {
            conflicting_constraints: vec!["row".to_string()],
            solver_excerpt: "text".to_string(),
            not_available_reason: Some("reason".to_string()),
        };
        assert_eq!(
            both.validate(),
            Err(InfeasibilityPolicyError::EvidenceAndReasonBothPresent)
        );

        // Evidence without a conflicting constraint or excerpt is incomplete.
        let incomplete = InfeasibilityEvidence::from_solver(Vec::new(), "text");
        assert_eq!(
            incomplete.validate(),
            Err(InfeasibilityPolicyError::EvidenceIncomplete)
        );
    }

    #[test]
    fn relaxation_decision_requires_name_review_and_scope() {
        let complete = RelaxationDecision {
            decision_name: "departure-energy-softening-2026-11".to_string(),
            review_reference: "review/2026-11-07-departure-energy".to_string(),
            relaxed_constraint_refs: vec!["ev_departure_energy_floor".to_string()],
            rationale: "fleet shortfall documented and accepted for this scenario".to_string(),
        };
        complete.validate().expect("complete decision");

        let unnamed = RelaxationDecision {
            decision_name: String::new(),
            ..complete.clone()
        };
        assert_eq!(
            unnamed.validate(),
            Err(InfeasibilityPolicyError::IncompleteRelaxationDecision {
                field: "decision_name"
            })
        );
        let unreviewed = RelaxationDecision {
            review_reference: String::new(),
            ..complete.clone()
        };
        assert_eq!(
            unreviewed.validate(),
            Err(InfeasibilityPolicyError::IncompleteRelaxationDecision {
                field: "review_reference"
            })
        );
        let scopeless = RelaxationDecision {
            relaxed_constraint_refs: Vec::new(),
            ..complete.clone()
        };
        assert_eq!(
            scopeless.validate(),
            Err(InfeasibilityPolicyError::IncompleteRelaxationDecision {
                field: "relaxed_constraint_refs"
            })
        );
        let unjustified = RelaxationDecision {
            rationale: String::new(),
            ..complete
        };
        assert_eq!(
            unjustified.validate(),
            Err(InfeasibilityPolicyError::IncompleteRelaxationDecision {
                field: "rationale"
            })
        );
    }

    #[test]
    fn relaxed_schedule_record_requires_a_decision_within_scope() {
        let decision = RelaxationDecision {
            decision_name: "departure-energy-softening-2026-11".to_string(),
            review_reference: "review/2026-11-07-departure-energy".to_string(),
            relaxed_constraint_refs: vec!["ev_departure_energy_floor".to_string()],
            rationale: "recorded rationale".to_string(),
        };
        let record = RelaxedScheduleRecord::new(
            candidate(),
            decision.clone(),
            vec!["ev_departure_energy_floor".to_string()],
        )
        .expect("relaxation within the reviewed scope records");
        assert_eq!(record.applied_to, vec!["ev_departure_energy_floor"]);
        assert_eq!(record.decision, decision);

        // A constraint outside the reviewed scope is rejected.
        assert_eq!(
            RelaxedScheduleRecord::new(
                candidate(),
                decision.clone(),
                vec!["unrelated_row".to_string()],
            ),
            Err(InfeasibilityPolicyError::RelaxationBeyondReviewedScope {
                constraint: "unrelated_row".to_string()
            })
        );

        // An incomplete decision cannot back a relaxed schedule.
        let mut unreviewed = decision.clone();
        unreviewed.review_reference = String::new();
        assert!(matches!(
            RelaxedScheduleRecord::new(candidate(), unreviewed, vec!["ev_departure_energy_floor".to_string()]),
            Err(InfeasibilityPolicyError::IncompleteRelaxationDecision { field: "review_reference" })
        ));

        // Nothing applied is not a relaxation record.
        assert!(matches!(
            RelaxedScheduleRecord::new(candidate(), decision, Vec::new()),
            Err(InfeasibilityPolicyError::IncompleteRelaxationDecision { field: "applied_to" })
        ));
    }
}
