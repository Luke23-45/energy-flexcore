// P3.20 — The Tier 3/4 boundary guard (report §12.3, §13; ledger P3.20).
//
// The recorded boundary (decision note
// `ledger/phase-03-flexibility-scheduling/19_20_determinism_tiers.md`):
// Tier 3 (MISOCP/MINLP) is built only when discrete decisions materially
// require it AND a suitable solver exists; Tier 4 (nonlinear AC-OPF) is
// advanced research / validated use cases only. NEITHER is built in v0.2.0.
//
// This module is the enforcement point: a public function the scheduling
// path (P3.24) calls BEFORE provider selection, and which the port surface
// treats as the pre-port gate. It rejects a Tier 3/4 formulation request
// with the §12.6-equivalent outcome `unsupported` PLUS a pointer to the
// recorded decision — rather than attempting the solve. (Defence in depth:
// the Tier 1 and Tier 2 providers additionally return in-band `unsupported`
// for every class outside their supported set, tested in the P3.17/P3.18
// test suites — the guard stops the request earlier, with the decision
// reference.)
//
// Reformulation rule ([`REFORMULATION_RULE`]): a formulation needing Tier 3
// is reformulated as Tier 1 MILP or Tier 2 continuous SOCP, or the gap is
// recorded as an open Stage-8 item — never forced through an unsuitable
// solver.
//
// IPOPT honesty note ([`IPOPT_LOCALITY_NOTE`]): IPOPT performs LOCAL
// nonlinear optimization only and carries NO global-optimality guarantee
// (report §7.4). It is not wired as a Phase 3 scheduler solver; any future
// use is a Stage 8 item. Its Eclipse Public License (EPL) is recorded for
// the D4 license audit (RESEARCH.md §4).
//
// This module names no concrete solver and adds no dependency; it reasons
// over the declared problem class only.

use crate::port::SolverCapabilities;
use crate::problem::{OptimizationProblem, ProblemClass};
use serde::{Deserialize, Serialize};

/// The pointer/reference carried by every Tier 3/4 rejection (ledger P3.20
/// step 3: "`unsupported` and a pointer to this decision").
pub const TIER_BOUNDARY_DECISION_REFERENCE: &str =
    "ledger/phase-03-flexibility-scheduling/19_20_determinism_tiers.md \
     (P3.20 tier-boundary decision; report §12.3, §13)";

/// The recorded tier-boundary policy, stated once.
pub const TIER_BOUNDARY_POLICY: &str = "Tier 3 (MISOCP/MINLP) is built only \
     when discrete decisions materially require it AND a suitable solver \
     exists; Tier 4 (nonlinear AC-OPF) is advanced research / validated use \
     cases only. NEITHER is built in v0.2.0 (report §12.3; ledger P3.20). \
     Tiers are problem classes, not quality rankings.";

/// The reformulation rule for a formulation that seems to need Tier 3
/// (ledger P3.20 step 2).
pub const REFORMULATION_RULE: &str = "A formulation needing Tier 3 (MISOCP/\
     MINLP) is reformulated as a Tier 1 MILP (e.g. linearising the conic \
     terms — the P3.18 inverter fixture pair shows exactly this translation \
     and its approximation gap) or a Tier 2 continuous SOCP (integrality \
     removed by an explicit, reviewed modelling decision), or the gap is \
     recorded as an open Stage-8 item — never forced through an unsuitable \
     solver.";

/// The IPOPT honesty note (report §7.4; ledger P3.20 step 4; RESEARCH.md §4).
pub const IPOPT_LOCALITY_NOTE: &str = "IPOPT performs LOCAL nonlinear \
     optimization only and carries NO global-optimality guarantee (report \
     §7.4). It is NOT wired as a Phase 3 scheduler solver; any future use is \
     justified only by a specific Tier 4 research case and lands in Stage 8, \
     not Phase 3. License: Eclipse Public License (EPL) — recorded for the \
     D4 license/dependency audit (still OPEN; RESEARCH.md §4).";

/// A Tier 3/4 boundary rejection: the §12.6-equivalent outcome is
/// `unsupported` (no solve attempted), and the rejection always carries the
/// decision reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TierBoundaryRejection {
    /// The declared problem class that was rejected.
    pub requested_class: ProblemClass,
    /// The §12.3 tier of the rejected class (3 or 4).
    pub requested_tier: u8,
    /// The §12.6-equivalent outcome string; always `unsupported` (the
    /// closed six-status vocabulary, result.rs).
    pub outcome: String,
    /// Pointer to the recorded tier-boundary decision.
    pub decision_reference: String,
    /// The reformulation guidance for this class (the P3.20 step 2 rule).
    pub reformulation_guidance: String,
}

impl std::fmt::Display for TierBoundaryRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "unsupported: declared class '{}' is Tier {} — not built in Phase 3 \
             (v0.2.0); see {}",
            self.requested_class.as_str(),
            self.requested_tier,
            self.decision_reference
        )
    }
}

impl std::error::Error for TierBoundaryRejection {}

/// `true` when the declared class is within the built tiers of this phase
/// (Tier 1 and Tier 2 only — report §12.3, ledger P3.20).
pub fn tier_supported(problem_class: ProblemClass) -> bool {
    problem_class.tier() <= 2
}

/// The Tier 3/4 boundary guard (ledger P3.20 step 3): rejects a Tier 3/4
/// formulation request with `unsupported` and the decision reference. The
/// scheduling path (P3.24) calls this BEFORE provider selection; a passing
/// class proceeds to provider selection and the providers' own class gates.
pub fn check_tier_boundary(
    problem_class: ProblemClass,
) -> Result<(), TierBoundaryRejection> {
    if tier_supported(problem_class) {
        return Ok(());
    }
    Err(TierBoundaryRejection {
        requested_class: problem_class,
        requested_tier: problem_class.tier(),
        outcome: crate::result::SolveStatus::Unsupported.as_str().to_string(),
        decision_reference: TIER_BOUNDARY_DECISION_REFERENCE.to_string(),
        reformulation_guidance: reformulation_guidance(problem_class),
    })
}

/// The guard over a full formulation (declared class).
pub fn guard_problem(
    problem: &OptimizationProblem,
) -> Result<(), TierBoundaryRejection> {
    check_tier_boundary(problem.declared_class)
}

/// Checks a provider capability declaration against the boundary: a provider
/// in this phase must never declare a Tier 3/4 class (the capability surface
/// is the "no Tier 3/4 code path" guarantee made checkable).
pub fn check_capabilities_within_boundary(
    capabilities: &SolverCapabilities,
) -> Result<(), TierBoundaryRejection> {
    for class in &capabilities.problem_classes {
        check_tier_boundary(*class)?;
    }
    Ok(())
}

/// The per-class reformulation guidance (the P3.20 step 2 rule, stated per
/// rejected class).
fn reformulation_guidance(problem_class: ProblemClass) -> String {
    match problem_class {
        ProblemClass::MixedIntegerSecondOrderConeProgram => format!(
            "reformulate as a Tier 1 MILP (linearise the conic terms) or a Tier 2 \
             continuous SOCP (integrality removed by an explicit, reviewed modelling \
             decision), or record the gap as an open Stage-8 item. {REFORMULATION_RULE}"
        ),
        ProblemClass::NonlinearProgram => format!(
            "reformulate with the linearised network/asset models of Tier 1 or a convex \
             Tier 2 approximation, or record the gap as an open Stage-8 research item. \
             {TIER_BOUNDARY_POLICY} {IPOPT_LOCALITY_NOTE}"
        ),
        other => format!(
            "no reformulation needed: class '{}' is Tier {} and is supported in this \
             phase",
            other.as_str(),
            other.tier()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture;

    #[test]
    fn tier1_and_tier2_classes_pass_the_guard() {
        for class in [
            ProblemClass::LinearProgram,
            ProblemClass::ConvexQuadraticProgram,
            ProblemClass::MixedIntegerLinearProgram,
            ProblemClass::ContinuousSecondOrderConeProgram,
        ] {
            assert!(tier_supported(class), "class {}", class.as_str());
            check_tier_boundary(class)
                .unwrap_or_else(|e| panic!("class {} must pass: {e}", class.as_str()));
        }
        guard_problem(&fixture::reference_lp()).expect("lp passes");
        guard_problem(&fixture::reference_milp()).expect("milp passes");
    }

    #[test]
    fn tier3_and_tier4_requests_are_rejected_with_the_decision_reference() {
        for problem in [
            fixture::reference_misocp_declaration(),
            fixture::reference_nonlinear_declaration(),
        ] {
            let rejection = guard_problem(&problem)
                .expect_err("Tier 3/4 requests are rejected");
            assert_eq!(rejection.outcome, "unsupported");
            assert_eq!(
                rejection.requested_class, problem.declared_class,
                "class {}",
                problem.declared_class.as_str()
            );
            assert!(
                rejection.requested_tier >= 3,
                "tier {} must be beyond the boundary",
                rejection.requested_tier
            );
            assert!(
                rejection
                    .decision_reference
                    .contains("19_20_determinism_tiers.md"),
                "every rejection carries the decision pointer"
            );
            assert!(!rejection.reformulation_guidance.is_empty());
        }
        assert!(!tier_supported(ProblemClass::MixedIntegerSecondOrderConeProgram));
        assert!(!tier_supported(ProblemClass::NonlinearProgram));
    }

    #[test]
    fn the_guard_is_callable_through_the_public_surface() {
        // The scheduling path (P3.24) and the port surface consume exactly
        // this function set; compile-check the dyn-safe usage shape.
        fn scheduling_gate(problem: &OptimizationProblem) -> Result<(), String> {
            guard_problem(problem).map_err(|e| e.to_string())
        }
        assert!(scheduling_gate(&fixture::reference_socp_declaration()).is_ok());
        assert!(scheduling_gate(&fixture::reference_misocp_declaration()).is_err());
    }
}
