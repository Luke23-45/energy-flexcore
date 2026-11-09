//! Neryva FlexCore — the solver/provider contract (P3.17; report §9.1, §12.3,
//! §7.4).
//!
//! Scheduling depends on this contract, never on a solver directly (report
//! §5.2): the [`port::Optimizer`] port declares what a provider supports and
//! providers reject everything else with the §12.6 status `unsupported`.
//!
//! | Module | Contract | Content |
//! |---|---|---|
//! | [`port`] | report §9.1, §7.4 | The `Optimizer` port: `SolverCapabilities` (declared problem classes, constraint features, scaling, tolerances, warm start, licensing, determinism, diagnostics channel) and `solve(problem, config)` |
//! | [`problem`] | report §12.3, §10.2 | The solver-neutral problem IR: every formulation declares its `ProblemClass` per run; canonical units (MW/MVAr/MVA/MWh, `P > 0` = export); the Tier 2 `SecondOrderCone` element the Tier 1 path never consumes |
//! | [`result`] | report §12.6, §9.2 | The result contract: the closed six-status vocabulary, objective + units, primal/dual diagnostics (available / not-available-with-reason), residuals, runtime, provenance, warnings, candidate ID, determinism record; `validate_complete` |
//! | [`config`] | P3.17 step 4; P3.19 | The provider-owned solver configuration: time limit, MIP gap, tolerances, scaling, fixed thread count (default 1), stable config digest |
//! | [`backend`] | decision note D-P3.17-1 | The transport-neutral `Tier1Backend` seam the Python optimization worker adapter implements (P3.19); no binding, no transport, no solver-specific type |
//! | [`highs_provider`] | P3.17 step 2 | The Tier 1 provider: class gate → formulation assembly → backend call → §12.6 status mapping; rejects SOCP/MISOCP/nonlinear with `unsupported` before any solver invocation |
//! | [`clarabel_provider`] | P3.18 | The Tier 2 (continuous SOCP) provider via the native Rust crate Clarabel 0.11.1: integrality gate (no integer variables — not MISOCP) and class gate before any solver invocation, cone assembly, §12.6 status mapping, the §12.5 exactness caveat, and the foundations §7.8 per-cone/branch relaxation residual against a configured tolerance |
//! | [`conic_fixture`] | P3.18 | Shared conic fixtures: the §11.3 inverter apparent-power cone (Tier 2) with its Tier 1 linearised counterpart, a canonical DistFlow branch relaxation cone, and the Tier 1 / Tier 2 cross-check record consumed by P3.23 |
//! | [`fixture`] | P3.17 tests | Synthetic reference LP/QP/MILP fixtures with hand-computed optima, plus declared SOCP/MISOCP/nonlinear formulations used by the rejection tests |
//! | [`determinism`] | P3.19 step 4 | The deterministic-replay machinery: replay identity (canonical serialization of problem IR + config digest + pinned versions + thread count), the record comparison (byte-identical or tolerance-identical with the tolerance stated), and the replay harness P3.26's scenario-regression calls |
//! | [`infeasibility`] | P3.19 step 3 | The infeasibility-evidence policy: conflict evidence or an explicit not-available reason, never a silently relaxed schedule; the `RelaxationDecision`/`RelaxedScheduleRecord` types the schedule record (P3.21/P3.22) embeds — there is no code path that applies a relaxation without a named, reviewed decision |
//! | [`tier_guard`] | P3.20 | The Tier 3/4 boundary guard: MISOCP/MINLP and nonlinear AC-OPF requests are rejected with `unsupported` and a pointer to the recorded decision; the reformulation rule and the IPOPT locality note (local solve only, no global-optimality claim) |
//!
//! # Recorded decisions (decision note
//! `ledger/phase-03-flexibility-scheduling/17_optimizer_port.md`)
//!
//! 1. **D-P3.17-1 — provider path.** Tier 1 (LP / convex QP / MILP) solves
//!    execute in the **Python optimization worker** through the pinned
//!    binding (highspy 1.15.1, verified 2026-11-07, RESEARCH.md §1). No Rust
//!    binding version was researched; the decision note closes the binding
//!    question for v0.2.0. The worker adapter lands with P3.19 behind the
//!    [`backend::Tier1Backend`] seam.
//! 2. **Solver-class discipline (AGENTS.md rule 3.4 — absolute).** HiGHS is
//!    an LP/QP/MIP solver. It is NOT an SOCP solver. A formulation declaring
//!    SOCP, MISOCP, or nonlinear is rejected by the Tier 1 provider with the
//!    §12.6 status `unsupported` before any solver invocation; a conic
//!    formulation cannot even be assembled into a Tier 1 model.
//! 3. **Status mapping.** The closed six-status vocabulary is produced by a
//!    recorded, tested mapping table (decision note §3); unbounded/ambiguous
//!    outcomes map to `numerical_failure` with the verbatim backend status
//!    recorded, never to `infeasible`.
//! 4. **Infeasibility.** An `infeasible` result carries the backend's
//!    conflict evidence where exposed, or an explicit not-available reason —
//!    never silence, never a relaxed schedule (P3.19 step 3 policy).
//!
//! # Recorded decisions (decision note
//! `ledger/phase-03-flexibility-scheduling/19_20_determinism_tiers.md`)
//!
//! 5. **P3.19 — determinism and failure handling.** Every result records its
//!    determinism metadata (pinned solver version, fixed thread count —
//!    default 1 — and the config digest); the replay identity and the
//!    record-comparison harness live in [`determinism`]. An infeasible run
//!    returns conflict evidence or an explicit not-available reason, never a
//!    relaxed schedule ([`infeasibility`]); any relaxation requires a named,
//!    reviewed decision recorded in the schedule.
//! 6. **P3.20 — the Tier 3/4 boundary.** Tier 3 (MISOCP/MINLP) and Tier 4
//!    (nonlinear AC-OPF) are NOT built in v0.2.0; requests are rejected with
//!    `unsupported` and the decision reference ([`tier_guard`]). A
//!    formulation needing Tier 3 is reformulated as Tier 1 MILP or Tier 2
//!    continuous SOCP, or the gap is recorded as an open Stage-8 item.
//!    IPOPT performs LOCAL nonlinear optimization only — no
//!    global-optimality guarantee — and is not wired as a Phase 3 scheduler
//!    solver (EPL license noted for the D4 audit).
//!
//! # License position (D4 audit input)
//!
//! HiGHS is MIT (project repository); Clarabel is Apache-2.0; IPOPT (not
//! wired in this phase) is EPL. The formal D4 license/dependency audit
//! is OPEN and owned by legal + engineering; the decision notes are input to
//! it, not the audit itself.
//!
//! # Scope and control status
//!
//! Capability contract for the design baseline only. The entire 0.x line is
//! simulation-only (release ladder v0.2.0); nothing in this crate touches a
//! device or a live grid, and nothing here authorises field control. FlexCore
//! is a supervisory orchestration layer: no primary frequency response, no
//! synthetic inertia, no relay protection, no inverter inner-loop control, no
//! sub-cycle control. A solved schedule is a **candidate** — feasibility is
//! the Phase 4 gate (§12.5).

pub mod backend;
pub mod clarabel_provider;
pub mod config;
pub mod conic_fixture;
pub mod determinism;
pub mod fixture;
pub mod highs_provider;
pub mod infeasibility;
pub mod port;
pub mod problem;
pub mod result;
pub mod tier_guard;

pub use clarabel_provider::{ClarabelTier2Provider, Tier2ProviderConfig};
pub use determinism::{ReplayComparison, ReplayHarness, ReplayIdentity, ReplayVerification};
pub use infeasibility::{INFEASIBILITY_POLICY, RelaxationDecision, RelaxedScheduleRecord};
pub use port::{Optimizer, OptimizerError, SolverCapabilities};
pub use tier_guard::{
    check_tier_boundary, guard_problem, IPOPT_LOCALITY_NOTE, REFORMULATION_RULE,
    TIER_BOUNDARY_DECISION_REFERENCE, TIER_BOUNDARY_POLICY, TierBoundaryRejection,
};

#[cfg(test)]
mod tests {
    use crate::backend::{BackendOptions, BackendOutcome, BackendError, Tier1Backend, Tier1Model};
    use crate::config::SolverConfig;
    use crate::fixture;
    use crate::port::Optimizer;
    use std::sync::Arc;

    /// A minimal in-crate double proving the whole crate assembles: the port
    /// is object-safe, the provider implements it, and the Tier 1 rejection
    /// path returns `unsupported` without touching the backend.
    struct NeverSolvesBackend;
    impl Tier1Backend for NeverSolvesBackend {
        fn solve(
            &self,
            _model: &Tier1Model,
            _options: &BackendOptions,
        ) -> Result<BackendOutcome, BackendError> {
            Err(BackendError::Unavailable("must not be called".to_string()))
        }
        fn describe(&self) -> crate::backend::BackendDescription {
            crate::backend::BackendDescription {
                name: "never-solves".to_string(),
                version: "0".to_string(),
                notes: Vec::new(),
            }
        }
    }

    #[test]
    fn tier1_provider_rejects_socp_before_touching_the_backend() {
        let provider =
            crate::highs_provider::HighsTier1Provider::new(Arc::new(NeverSolvesBackend));
        let problem = fixture::reference_socp_declaration();
        let result = provider
            .solve(&problem, &SolverConfig::default())
            .expect("unsupported is an in-band status, not an error");
        assert_eq!(result.status, crate::result::SolveStatus::Unsupported);
        assert_eq!(result.problem_class, crate::problem::ProblemClass::ContinuousSecondOrderConeProgram);
        assert!(
            result
                .warnings
                .iter()
                .any(|w| w.contains("not an SOCP"))
        );
        result
            .validate_complete()
            .expect("the unsupported record is complete");
    }

    #[test]
    fn tier1_provider_rejects_nonlinear_before_touching_the_backend() {
        let provider =
            crate::highs_provider::HighsTier1Provider::new(Arc::new(NeverSolvesBackend));
        let problem = fixture::reference_nonlinear_declaration();
        let result = provider
            .solve(&problem, &SolverConfig::default())
            .expect("unsupported is an in-band status, not an error");
        assert_eq!(result.status, crate::result::SolveStatus::Unsupported);
    }
}
