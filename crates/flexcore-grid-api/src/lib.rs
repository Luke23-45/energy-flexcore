// P4.01+P4.02 — The grid-model provider contract: the primary-provider
// decision record (P4.01, a RECOMMENDATION — D3 stays OPEN) and the
// GridModelProvider port with its mandatory scope declarations, backend
// seam, and translation layer (P4.02). Decisions:
// ledger/phase-04-grid-feasibility-dispatch/01_02_grid_provider.md.
//
//! Neryva FlexCore — the grid-model provider contract (P4.02; report §9.1,
//! §12.4, §12.5, §7.2).
//!
//! The Phase 4 feasibility gate depends on this contract, never on a grid
//! provider directly (report §5.2, §12.4): the [`port::GridModelProvider`]
//! port declares what a provider evaluates and the provider's **mandatory
//! scope declaration** states what it does not model — an undeclared-scope
//! adapter is a construction error, and a request outside the declared
//! scope is refused, never approximated.
//!
//! | Module | Contract | Content |
//! |---|---|---|
//! | [`port`] | report §9.1, §12.5 | The `GridModelProvider` port: versioned network-model IR, candidate injections (canonical contract types), the power-flow result (bus voltages in both canonical representations, branch currents/loadings, transformer loadings, closed convergence vocabulary, residuals, model-coverage limitations), the limit verdict, and the fail-closed completeness gate |
//! | [`scope`] | ledger P4.02 step 3 | The mandatory `ScopeDeclaration`: balanced vs unbalanced, radial vs meshed, modelled equipment, the non-empty `not_modelled` limitation record, the AC-power-flow-not-AC-OPF solver statement, and the scope gate |
//! | [`backend`] | decision note D-P4.02-1 | The transport-neutral `GridModelBackend` seam the Python numerical worker adapter implements in a later wave (the D-P3.17-1 pattern); worker-contract payload vocabulary with the recorded VERIFY-BUILD items; no binding, no transport, no gRPC |
//! | [`translation`] | report §10.2 | The owned, tested canonical-units translation layer: MW with `P > 0` = export, MVAr under the DECLARED reactive sign convention, kV at the nominal base and per-unit in the solver representation, A (or per-unit on an explicit base), fraction-of-rating loadings — both directions, round-trip guaranteed |
//! | [`adapter`] | decision note D-P4.01-1/2/3, D-P4.02-1 | The Rust-side provider for the pandapower path: scope gating → translation → backend call → result mapping → completeness gate; the recorded pin (pandapower 3.5.6), the §12.4 boundary statement, the D3-OPEN statement, and the provider-addition rule |
//!
//! # Recorded decisions (decision note
//! `ledger/phase-04-grid-feasibility-dispatch/01_02_grid_provider.md`)
//!
//! 1. **D-P4.01-1 — provider selection (a RECOMMENDATION, never a D3
//!    decision).** The primary provider is the recommended default of
//!    report §25.2 (pandapower). D3 (network class and model fidelity)
//!    stays OPEN — the balanced-radial default is a recommendation, not a
//!    decision; the decision gate remains Phase 0, owner power systems.
//! 2. **D-P4.01-2 — provider boundary (§12.4).** The provider is the
//!    balanced AC power-flow path with SOME three-phase/asymmetric
//!    calculations; it is NOT a universal validator. Model and solver
//!    limitations are verified per case; the limitation list lives in the
//!    declared scope ([`adapter::balanced_worker_scope`]) and travels as
//!    model-coverage limitations on every result. The provider's OPF
//!    capability is not the feasibility gate and is not wired in.
//! 3. **D-P4.01-3 — provider-addition rule.** A second provider (OpenDSS,
//!    GridLAB-D, PyPSA) is added ONLY when a named test case cannot be
//!    expressed or validated with the primary provider; the case is
//!    recorded. No familiarity-based additions.
//! 4. **D-P4.02-1 — provider path (the D-P3.17-1 pattern).** The AC
//!    power-flow evaluation executes in the Python numerical worker (§7.2:
//!    Python is the numerical worker, never the command authority) via the
//!    pinned provider version, behind the transport-neutral
//!    [`backend::GridModelBackend`] seam — no gRPC dependency now; the
//!    worker adapter lands in a later wave (the P3.19 precedent).
//! 5. **Pin discipline (P4.01).** pandapower 3.5.6 (RESEARCH.md §1,
//!    verified 2026-11-07; released 7 October 2026; BSD-3-Clause — D4
//!    audit input, the formal audit is OPEN). The pin is recorded in the
//!    worker lockfile note and carried as a REQUIRED field on every
//!    result; the worker's reported installed version is cross-checked
//!    against the pin and a mismatch is a contract violation.
//! 6. **Scope discipline (P4.02 step 3).** Every adapter declares its
//!    model scope ([`scope::ScopeDeclaration`]); the `not_modelled` list
//!    is mandatory and non-empty; the scope gate refuses requests outside
//!    the declaration; the scope's limitations surface as model-coverage
//!    limitations on every result (§12.5 step 6).
//!
//! # Solver-class discipline (absolute)
//!
//! This crate runs an **AC power-flow evaluation of specified injections
//! and controls**. It is NOT an AC-OPF optimizer; the feasibility gate
//! never optimizes (§12.5). AC power flow ≠ AC-OPF; the terms are never
//! interchanged. Non-convergence is a RESULT, never retried away and never
//! treated as feasible. Model coverage limitations are carried on every
//! result; a model that does not cover part of the network cannot pass
//! that part.
//!
//! # Scope and control status
//!
//! Simulation-only (release ladder v0.3.0: simulation and shadow). Nothing
//! in this crate touches a device or a live grid, and nothing here
//! authorises field control. FlexCore is a supervisory orchestration
//! layer: no primary frequency response, no synthetic inertia, no relay
//! protection, no inverter inner-loop control, no sub-cycle control. A
//! within-limits verdict means the candidate satisfied the declared model
//! and limits within stated tolerances — not that it is safe in the field,
//! certified, or approved.

pub mod adapter;
pub mod backend;
pub mod fail_closed;
pub mod feasibility;
pub mod limits;
pub mod model_health;
pub mod port;
pub mod scope;
pub mod socp_boundary;
pub mod translation;
pub mod verdict;

pub use adapter::{PandapowerGridProvider, PROVIDER_ADDITION_RULE, PROVIDER_BOUNDARY_STATEMENT, PROVIDER_ID, PROVIDER_PINNED_VERSION};
pub use port::{
    evaluate_limits, CandidateInjection, FeasibilityVerdict, GridModelProvider, GridPortError,
    GridProviderCapabilities, NetworkLimits, PowerFlowResult, PowerFlowStatus, VersionedNetworkModel,
};
pub use scope::{EquipmentKind, NotModelled, PhaseScope, ScopeDeclaration, ScopeError, TopologyScope};

// P4.03 — the versioned-model / model-health surface (additive re-exports).
pub use model_health::{
    assert_usable_for_feasibility, check_model_health, model_content_hash, HealthFinding,
    ModelCoverage, ModelHealthConfig, ModelHealthError, ModelHealthReport, ModelLifecycleState,
    ModelVersionRegistry, RegistryElement, RegistryElementKind, SourceChangeRecord, VersionedModel,
    CONTENT_HASH_STATEMENT, MODEL_LIFECYCLE_DECISION, MODEL_STALENESS_POLICY,
};

// P4.04+P4.05 — the feasibility-gate steps 1–4 surface (additive re-exports).
pub use feasibility::{
    apply_and_run_power_flow, derive_injections, run_feasibility_gate, validate_candidate,
    AttemptOutput, AttemptRecordError, CandidateRefs, CandidateSetpoint, CandidateValidationConfig,
    DeclaredBounds, DerivedCandidate, DerivedInjection, EnvelopeFreshness, EvidenceOutcome,
    FeasibilityAttemptRecord, FeasibilityCandidate, GateAbort, GateEvidenceRow, GatePipelineOutcome,
    GateRejection, GateRule, GateSources, GateStep, RevisionState, SeamError, TelemeteredState,
    AC_POWER_FLOW_STATEMENT, GATE_SEAM_STATEMENT, INJECTION_QUALITY_POLICY, NO_RETRY_POLICY,
    UNBALANCED_DEFERRAL_NOTE,
};

// P4.06 — the step-5 limit-check surface (additive re-exports).
pub use limits::{
    check_limits, is_hard_violation, BindingConstraint, CategoryWorstMargin, ElementLoadingPercent,
    GeneratorLimit, GeneratorLimitSet, LimitCategory, LimitCheckRecord, LimitCheckReport,
    LimitCheckStatus, LimitDirection, ScopeLimitGap, BOUNDARY_CONVENTION, HARD_LIMIT_STATEMENT,
    LIMIT_SOURCE_STATEMENT, MARGIN_STATEMENT, PHYSICAL_RATING_FRACTION,
};

// P4.07 — the steps 6–8 verdict / routing surface (additive re-exports).
pub use verdict::{
    route_candidate, route_fail_closed, route_pipeline_outcome, ConstrainProposal, GateVerdict,
    InputSnapshotEcho, ProposedInjectionSetpoint, RoutingInput, VerdictEvidence, VerdictOutcome,
    CONSTRAIN_PROPOSAL_STATEMENT, FEASIBILITY_CHECK_COMPLETED_DECISION, ROUTING_DECISION_REF,
    ROUTING_RULE_STATEMENT,
};

// P4.08 — the fail-closed policy surface (additive re-exports).
pub use fail_closed::{
    classify_pipeline_error, fail_closed_action, fail_closed_outcome, FailClosedAction,
    FailClosedOutcome, FailClosedTrigger, PipelineError, EXHAUSTIVE_MATCH_STATEMENT,
    FAIL_CLOSED_STATEMENT, HOLD_RELEASE_POLICY,
};

// P4.09 — the SOCP relaxation boundary surface (additive re-exports).
pub use socp_boundary::{
    assess_residuals, check_relaxation_conditions, feasibility_verdict_dimension,
    gate_socp_boundary, ConditionOutcome, FeasibilityVerdictDimension, RelaxationCondition,
    RelaxationConditionCheck, RelaxationConditionRecord, RelaxationResidualRecord,
    RelaxationResidualReport, RelaxationResidualRecordError, ResidualAssessment, ResidualFinding,
    ResidualFindingKind, SocpBoundaryGate, TierProvenance, DISTFLOW_RESIDUAL_FORMULA,
    MIRRORED_DEFAULT_RESIDUAL_TOLERANCE, SOCP_EXACTNESS_CAVEAT, SOCP_SEAM_STATEMENT,
    TIER2_VERDICT_STATEMENT,
};

/// Shared fixtures for the crate's integration tests (public so
/// `tests/*.rs` can build the same model IR and outcome shapes the
/// unit tests use). Not part of the port surface — the
/// dependency-direction test deliberately scans only the port-surface
/// modules, and this module is provider-neutral anyway.
pub mod tests_support {
    use crate::backend::{
        BackendBranchResult, BackendBusResult, BackendPowerFlowOutcome, BackendTransformerResult,
    };
    use crate::port::{BranchSpec, BusSpec, ExternalGridSpec, GridModelRef, TransformerSpec, VersionedNetworkModel};

    /// The recorded tolerance for the loading cross-check (shared with
    /// `adapter::map_outcome`'s documentation).
    pub const LOADING_CROSS_CHECK_TOLERANCE: f64 = 1.0e-4;

    /// A small three-bus radial model (hand-built fixture):
    /// `bus_1` (20 kV, slack) — transformer — `bus_2` (0.4 kV) — line —
    /// `bus_3` (0.4 kV). Parameters are illustrative magnitudes for the
    /// contract tests; the physics runs in the worker, not here.
    pub fn three_bus_model(model_ref: GridModelRef) -> VersionedNetworkModel {
        VersionedNetworkModel {
            model_ref,
            base_mva: 100.0,
            buses: vec![
                BusSpec {
                    bus_id: "bus_1".to_string(),
                    nominal_kv: 20.0,
                    is_slack: true,
                },
                BusSpec {
                    bus_id: "bus_2".to_string(),
                    nominal_kv: 0.4,
                    is_slack: false,
                },
                BusSpec {
                    bus_id: "bus_3".to_string(),
                    nominal_kv: 0.4,
                    is_slack: false,
                },
            ],
            branches: vec![BranchSpec {
                branch_id: "line_2_3".to_string(),
                from_bus: "bus_2".to_string(),
                to_bus: "bus_3".to_string(),
                r_ohm: 0.21,
                x_ohm: 0.07,
                rated_current_a: 500.0,
            }],
            transformers: vec![TransformerSpec {
                transformer_id: "trafo_1_2".to_string(),
                hv_bus: "bus_1".to_string(),
                lv_bus: "bus_2".to_string(),
                rated_mva: 0.4,
                vk_percent: 4.0,
                vk_r_percent: 1.2,
            }],
            external_grids: vec![ExternalGridSpec {
                bus_id: "bus_1".to_string(),
                vm_pu: 1.02,
                va_degree: 0.0,
            }],
        }
    }

    /// A converged worker outcome consistent with `three_bus_model`: one
    /// row per element, hand-chosen magnitudes used by the contract tests.
    pub fn converged_outcome(
        provider_version: &str,
    ) -> BackendPowerFlowOutcome {
        BackendPowerFlowOutcome {
            converged: true,
            reported_error: None,
            provider_version_reported: Some(provider_version.to_string()),
            bus_results: vec![
                BackendBusResult {
                    bus_id: "bus_1".to_string(),
                    vm_pu: 1.02,
                },
                BackendBusResult {
                    bus_id: "bus_2".to_string(),
                    vm_pu: 1.01,
                },
                BackendBusResult {
                    bus_id: "bus_3".to_string(),
                    vm_pu: 0.985,
                },
            ],
            branch_results: vec![BackendBranchResult {
                branch_id: "line_2_3".to_string(),
                i_from_ka: 0.38,
                i_to_ka: 0.3795,
                // 0.38 kA = 380 A of 500 A rating = 76.0 percent.
                loading_percent: 76.0,
            }],
            transformer_results: vec![BackendTransformerResult {
                transformer_id: "trafo_1_2".to_string(),
                // 62.5 percent of 0.4 MVA: full-load currents are
                // 0.4e6/(sqrt(3)*20000) = 11.547 A (HV) and
                // 0.4e6/(sqrt(3)*400) = 577.35 A (LV), so 62.5 percent
                // gives 7.217 A and 360.84 A.
                loading_percent: 62.5,
                i_hv_ka: 0.007_217,
                i_lv_ka: 0.360_84,
            }],
            iterations: Some(4),
            worker_status_text: "converged in 4 iterations".to_string(),
            warnings: Vec::new(),
        }
    }
}
