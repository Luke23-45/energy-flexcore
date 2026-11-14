// P4.22 — Deterministic rejection: infeasible commands (report §23 Stage 4
//         exit gate, §12.5, §14.1; ledger P4.22; dependencies P4.06–P4.07
//         and P4.12, all done). Decision record with the PRE-REGISTERED
//         per-fixture expectation table (written BEFORE the assertions, and
//         UNRUN at authoring time):
//         `ledger/phase-04-grid-feasibility-dispatch/
//         20_22_rejection_suites_a.md`.
//
// Coverage per the ledger acceptance criteria:
//
// 1. Fixed single-violation fixtures, each violating EXACTLY ONE network
//    limit (asserted: the limit report carries exactly one violation): low
//    voltage, high voltage, branch/thermal overload, transformer overload
//    (§12.5 step 5; the P4.06 single-violation discipline).
// 2. A power-flow non-convergence case and a stale-model case, per the
//    P4.08 fail-closed policy (non-convergence renders as the first-class
//    INCONCLUSIVE outcome, retry NONE; a stale model is the
//    MODEL_STALE_OR_INCOMPLETE HOLD trigger — never a pass).
// 3. The full VALIDATED-candidate → feasibility-gate path: the candidate
//    runs through `run_feasibility_gate` (§12.5 steps 1–4, through the
//    PORT against the scripted backend double — no real solver, no I/O),
//    then the step-5 limit check (`check_limits`), then the steps 6–8
//    routing (`route_pipeline_outcome`).
// 4. The failing constraint NAMED in the verdict matches the fixture's
//    DESIGNED violation — never a different, incidental one.
//
// Pre-registered outcomes (the frozen routing rule D-P4.07-1 is the
// designed behaviour this suite pins; the command-layer consequence —
// VALIDATED → REJECTED with `network_limit:<constraint>` — is the P4.12
// gate mapping, pinned in `flexcore-application/tests/command_gates.rs`):
//
// | Fixture | Designed violation         | Pre-registered verdict        |
// |---|---|---|
// | F-IF-1  | bus_3 vm_pu 0.88 < 0.9     | RETURNED for re-optimization, `bus_voltage_lower:bus_3` named |
// | F-IF-2  | bus_2 vm_pu 1.12 > 1.1     | RETURNED for re-optimization, `bus_voltage_upper:bus_2` named |
// | F-IF-3  | line_2_3 loading 1.05 (hard, above the physical rating) | REJECTED, `branch_thermal:line_2_3` named |
// | F-IF-4  | trafo_1_2 loading 1.05 (hard) | REJECTED, `transformer_loading:trafo_1_2` named |
// | F-IF-5  | power flow does not converge | INCONCLUSIVE (retry NONE; never a pass, never a limit-violation) |
// | F-IF-6  | stale model (source change beyond the health bound) | step-3 ABORT before any power flow → HELD per P4.08 (MODEL_STALE_OR_INCOMPLETE) |
// | control | none                       | WITHIN-LIMITS (a control, not a rejection fixture) |
//
// Determinism: every fixture is run TWICE with identical inputs and the
// two verdicts are asserted identical. The gate clock is a fixed constant;
// the backend is a scripted double; there is no randomness and no I/O.
//
// CI wiring: an integration test under `crates/flexcore-grid-api/tests/`
// is CI-runnable by construction — the `ci` workflow's `unit-tests` job
// runs `cargo test --workspace` (equivalently `cargo test -p
// flexcore-grid-api` for this suite), and the §22.2 `scenario-regression`
// job is the regression-gate job the ledger names. No CI file was edited.

mod common;

use common::{model_ref, test_limits, EXPECTED_PIN, ScriptedBackend};
use flexcore_contracts::units::{ActivePowerMW, ReactivePowerMVAr, ReactiveSignConvention};
use flexcore_grid_api::adapter::PandapowerGridProvider;
use flexcore_grid_api::backend::{
    BackendBusResult, BackendBranchResult, BackendPowerFlowOutcome, BackendTransformerResult,
};
use flexcore_grid_api::fail_closed::{FailClosedAction, FailClosedTrigger};
use flexcore_grid_api::feasibility::{
    run_feasibility_gate, AttemptOutput, CandidateSetpoint, CandidateValidationConfig,
    DeclaredBounds, EnvelopeFreshness, FeasibilityCandidate, GateAbort, GatePipelineOutcome,
    GateRunRequest, GateSources, GateStep, RevisionState, TelemeteredState,
};
use flexcore_grid_api::limits::{
    check_limits, is_hard_violation, GeneratorLimit, GeneratorLimitSet, LimitCheckReport,
    LimitCheckStatus,
};
use flexcore_grid_api::model_health::{
    HealthFinding, ModelCoverage, ModelHealthConfig, ModelLifecycleState, ModelVersionRegistry,
    RegistryElement, RegistryElementKind, VersionedModel,
};
use flexcore_grid_api::port::{
    GridModelProvider, PowerFlowStatus, VersionedNetworkModel,
};
use flexcore_grid_api::scope::{NotModelled, PhaseScope, ScopeDeclaration, TopologyScope};
use flexcore_grid_api::tests_support::three_bus_model;
use flexcore_grid_api::verdict::{route_pipeline_outcome, GateVerdict, VerdictOutcome};
use std::sync::Arc;

const NOW: i64 = 10_000;

// ---------------------------------------------------------------------------
// Fixtures (the P4.02 three-bus contract model; hand-computed loadings)
// ---------------------------------------------------------------------------

/// The model bases the hand-computed loadings use (the same as the P4.02
/// contract fixtures): `line_2_3` is rated 500 A (0.38 kA = 0.76 loading)
/// and `trafo_1_2` is rated 0.4 MVA (full-load currents 11.547 A HV /
/// 577.35 A LV; 62.5 percent = 7.217 A / 360.84 A).
const BRANCH_RATED_KA: f64 = 0.5;
const TRAFO_FULL_LOAD_HV_KA: f64 = 0.011_547;
const TRAFO_FULL_LOAD_LV_KA: f64 = 0.577_35;

/// A converged-shaped worker outcome with the given bus voltages and
/// element loading percentages, and currents CONSISTENT with the reported
/// loadings (the adapter cross-checks the branch loading against its
/// currents within 1e-4 relative tolerance).
fn backend_outcome(
    bus_voltages: &[(&str, f64)],
    branch_loading_percent: f64,
    trafo_loading_percent: f64,
    converged: bool,
) -> BackendPowerFlowOutcome {
    let branch_fraction = branch_loading_percent / 100.0;
    let trafo_fraction = trafo_loading_percent / 100.0;
    BackendPowerFlowOutcome {
        converged,
        reported_error: None,
        provider_version_reported: Some(EXPECTED_PIN.to_string()),
        bus_results: bus_voltages
            .iter()
            .map(|(bus_id, vm_pu)| BackendBusResult {
                bus_id: (*bus_id).to_string(),
                vm_pu: *vm_pu,
            })
            .collect(),
        branch_results: vec![BackendBranchResult {
            branch_id: "line_2_3".to_string(),
            i_from_ka: branch_fraction * BRANCH_RATED_KA,
            i_to_ka: branch_fraction * BRANCH_RATED_KA - 0.000_5,
            loading_percent: branch_loading_percent,
        }],
        transformer_results: vec![BackendTransformerResult {
            transformer_id: "trafo_1_2".to_string(),
            loading_percent: trafo_loading_percent,
            i_hv_ka: trafo_fraction * TRAFO_FULL_LOAD_HV_KA,
            i_lv_ka: trafo_fraction * TRAFO_FULL_LOAD_LV_KA,
        }],
        iterations: Some(4),
        worker_status_text: "converged in 4 iterations".to_string(),
        warnings: Vec::new(),
    }
}

/// The all-within-limits outcome (the control and the base every
/// single-violation fixture differs from in EXACTLY ONE value).
fn passing_outcome() -> BackendPowerFlowOutcome {
    backend_outcome(
        &[("bus_1", 1.02), ("bus_2", 1.01), ("bus_3", 0.985)],
        76.0,
        62.5,
        true,
    )
}

fn coverage() -> ModelCoverage {
    ModelCoverage {
        covered_feeder_ids: vec!["feeder-contract-test".to_string()],
        covered_element_ids: vec!["asset-1".to_string()],
        coverage_statement: "covers the contract-test feeder and asset-1 at bus_3".to_string(),
    }
}

fn fresh_model() -> VersionedNetworkModel {
    let mut reference = model_ref();
    reference.content_hash = String::new(); // the versioning layer assigns it
    three_bus_model(reference)
}

fn versioned_model() -> VersionedModel {
    VersionedModel::register(
        fresh_model(),
        ModelLifecycleState::Active,
        coverage(),
        NOW - 1_000,
    )
    .expect("registers")
}

fn registry_with_model(versioned: &VersionedModel) -> ModelVersionRegistry {
    let mut registry = ModelVersionRegistry::new();
    registry.register_version(versioned.clone()).expect("registers");
    registry
}

fn portfolio() -> Vec<RegistryElement> {
    vec![
        RegistryElement {
            element_id: "feeder-contract-test".to_string(),
            kind: RegistryElementKind::Feeder,
        },
        RegistryElement {
            element_id: "asset-1".to_string(),
            kind: RegistryElementKind::Element,
        },
    ]
}

fn sources() -> GateSources {
    GateSources {
        capability_bounds: Arc::new(|_asset, _at| {
            Ok(DeclaredBounds {
                p_min_mw: -5.0,
                p_max_mw: 5.0,
                q_min_mvar: -2.0,
                q_max_mvar: 2.0,
                q_convention: ReactiveSignConvention::InjectionPositive,
                ramp_mw_per_min: 10.0,
            })
        }),
        envelope_freshness: Arc::new(|_| Ok(EnvelopeFreshness::Fresh)),
        revision_superseded: Arc::new(|_, _| Ok(RevisionState::Active)),
        ramp_baseline: Arc::new(|_| {
            Ok(TelemeteredState {
                active_power_mw: 0.0,
                quality: flexcore_contracts::quality::TelemetryQuality::Good,
                observed_at_millis: NOW - 100,
            })
        }),
    }
}

fn candidate() -> FeasibilityCandidate {
    FeasibilityCandidate {
        candidate_id: "cand-rej-if-1".to_string(),
        schedule_id: "0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8b".to_string(),
        revision: 2,
        envelope_version: "env-v1".to_string(),
        result_record_snapshot_ref: "schedule_input_snapshot:env-v1".to_string(),
        created_at_millis: NOW - 100,
        valid_from_millis: NOW,
        valid_until_millis: NOW + 3_000,
        stated_duration_millis: 3_000,
        setpoints: vec![CandidateSetpoint {
            setpoint_id: "sp-1".to_string(),
            asset_id: "asset-1".to_string(),
            bus_id: "bus_3".to_string(),
            active_power: ActivePowerMW::new(2.5).expect("valid"),
            reactive_power: ReactivePowerMVAr::new(0.5).expect("valid"),
            reactive_sign_convention: ReactiveSignConvention::InjectionPositive,
            interval_start_millis: NOW,
        }],
    }
}

fn generators() -> GeneratorLimitSet {
    GeneratorLimitSet {
        limits: vec![GeneratorLimit {
            bus_id: "bus_3".to_string(),
            s_max_mva: 4.0,
            source_note: "fixture capability cap-1 (§11-derived)".to_string(),
        }],
    }
}

fn scope() -> ScopeDeclaration {
    ScopeDeclaration::new(
        "rejection-infeasible-test-evaluator".to_string(),
        PhaseScope::Balanced,
        TopologyScope::RadialOnly,
        vec![
            flexcore_grid_api::scope::EquipmentKind::Line,
            flexcore_grid_api::scope::EquipmentKind::TwoWindingTransformer,
            flexcore_grid_api::scope::EquipmentKind::ExternalGrid,
        ],
        vec![NotModelled::new(
            "unbalanced_three_phase".to_string(),
            "the evaluator does not model unbalanced three-phase asymmetry".to_string(),
        )
        .expect("valid")],
        "runs an AC power-flow evaluation of specified injections and controls; \
         it is NOT an AC-OPF optimizer"
            .to_string(),
        vec!["P4.22 rejection-suite scope".to_string()],
    )
    .expect("valid scope")
}

/// One full gate run: steps 1–4 through the port (`run_feasibility_gate`
/// over the scripted backend), then the step-5 limit check, then the
/// steps 6–8 routing. Returns the pipeline outcome, the limit report (for
/// a completed attempt), and the routed verdict.
fn run_full_gate(
    outcome: BackendPowerFlowOutcome,
    versioned: &VersionedModel,
    registry: &ModelVersionRegistry,
) -> (GatePipelineOutcome, Option<LimitCheckReport>, GateVerdict) {
    let backend = Arc::new(ScriptedBackend::new(outcome));
    let provider =
        PandapowerGridProvider::new(backend, flexcore_grid_api::adapter::balanced_worker_scope())
            .expect("balanced scope accepted");
    let pipeline = run_feasibility_gate(GateRunRequest {
        candidate: &candidate(),
        sources: &sources(),
        config: &CandidateValidationConfig::default(),
        versioned_model: versioned,
        registry,
        portfolio_elements: &portfolio(),
        health_config: &ModelHealthConfig::default(),
        provider: &provider,
        limits: &test_limits(),
        attempt_id: "attempt-rej-if-1".to_string(),
        now_millis: NOW,
    });
    let limit_report = match &pipeline {
        GatePipelineOutcome::Completed { attempt, .. } => {
            Some(check_limits(attempt.result(), &test_limits(), &generators(), &scope()))
        }
        _ => None,
    };
    let verdict = route_pipeline_outcome(&pipeline, limit_report.as_ref(), None, NOW);
    (pipeline, limit_report, verdict)
}

/// One fresh model + registry pair (each run builds its own, so the runs
/// share nothing mutable).
fn model_and_registry() -> (VersionedModel, ModelVersionRegistry) {
    let versioned = versioned_model();
    let registry = registry_with_model(&versioned);
    (versioned, registry)
}

/// Runs the fixture twice with identical inputs and asserts the two
/// verdicts are identical (the determinism requirement, in-suite). Returns
/// the two verdicts for the caller's own assertions.
fn run_fixture_twice(outcome: BackendPowerFlowOutcome) -> (GateVerdict, GateVerdict) {
    let (versioned_a, registry_a) = model_and_registry();
    let (versioned_b, registry_b) = model_and_registry();
    let (_, _, first) = run_full_gate(outcome.clone(), &versioned_a, &registry_a);
    let (_, _, second) = run_full_gate(outcome, &versioned_b, &registry_b);
    assert_eq!(first, second, "the two runs must produce identical verdicts");
    (first, second)
}

/// A stale-model registry (the fixture F-IF-6 condition): a source change
/// after registration puts the model beyond the health staleness bound.
fn stale_model_and_registry() -> (VersionedModel, ModelVersionRegistry) {
    let (versioned, mut registry) = model_and_registry();
    registry
        .record_source_change("chg-1".to_string(), NOW - 500, "topology change".to_string())
        .expect("records");
    (versioned, registry)
}

// ---------------------------------------------------------------------------
// Pre-registration pins
// ---------------------------------------------------------------------------

/// The routing decision reference this suite's expectations are
/// pre-registered against (the frozen D-P4.07-1 rule).
#[test]
fn the_expectations_are_pinned_to_the_frozen_routing_decision() {
    assert_eq!(
        flexcore_grid_api::verdict::ROUTING_DECISION_REF,
        "D-P4.07-1"
    );
    assert!(!flexcore_grid_api::fail_closed::FAIL_CLOSED_STATEMENT.trim().is_empty());
    assert!(!flexcore_grid_api::fail_closed::HOLD_RELEASE_POLICY.trim().is_empty());
}

// ---------------------------------------------------------------------------
// F-IF-1 — low voltage (exactly one violation)
// ---------------------------------------------------------------------------

#[test]
fn low_voltage_fixture_rejects_with_the_designed_constraint_named() {
    // DESIGNED violation: bus_3 at 0.88 pu, below the 0.9 lower band.
    // Everything else stays at the passing values, so the fixture violates
    // exactly one limit.
    let mut outcome = passing_outcome();
    outcome.bus_results[2].vm_pu = 0.88;
    let (versioned, registry) = model_and_registry();
    let (pipeline, report, verdict) = run_full_gate(outcome.clone(), &versioned, &registry);

    // The pipeline completed (the power flow converged; the violation is a
    // limit outcome, not a pipeline fault).
    assert!(matches!(pipeline, GatePipelineOutcome::Completed { .. }));

    // EXACTLY ONE violation, and it is the DESIGNED one.
    let report = report.expect("a completed attempt has a limit report");
    assert_eq!(report.violations.len(), 1, "single-violation fixture");
    assert_eq!(
        report.violated_constraint_names(),
        vec!["bus_voltage_lower:bus_3"]
    );
    let violation = &report.violations[0];
    assert!((violation.measured - 0.88).abs() < 1.0e-12);
    assert!((violation.limit - 0.9).abs() < 1.0e-12);

    // Pre-registered verdict (rule 6 of D-P4.07-1): a voltage-band
    // violation returns for re-optimization with the constraint named —
    // the command layer (P4.12) maps this to VALIDATED → REJECTED with
    // `network_limit:bus_voltage_lower:bus_3`. NEVER within limits.
    assert_eq!(verdict.outcome, VerdictOutcome::ReturnedForReOptimization);
    assert_eq!(
        verdict.violated_constraints,
        vec!["bus_voltage_lower:bus_3"]
    );
    assert_ne!(verdict.outcome, VerdictOutcome::WithinLimits);
    assert_ne!(verdict.outcome, VerdictOutcome::Rejected);
    assert_ne!(verdict.outcome, VerdictOutcome::Constrained);

    // Determinism: the same inputs run twice give identical verdicts.
    let (first, second) = run_fixture_twice(outcome);
    assert_eq!(first.outcome, VerdictOutcome::ReturnedForReOptimization);
    assert_eq!(first, second);
}

// ---------------------------------------------------------------------------
// F-IF-2 — high voltage (exactly one violation)
// ---------------------------------------------------------------------------

#[test]
fn high_voltage_fixture_rejects_with_the_designed_constraint_named() {
    // DESIGNED violation: bus_2 at 1.12 pu, above the 1.1 upper band.
    let mut outcome = passing_outcome();
    outcome.bus_results[1].vm_pu = 1.12;
    let (versioned, registry) = model_and_registry();
    let (_, report, verdict) = run_full_gate(outcome.clone(), &versioned, &registry);

    let report = report.expect("a completed attempt has a limit report");
    assert_eq!(report.violations.len(), 1, "single-violation fixture");
    assert_eq!(
        report.violated_constraint_names(),
        vec!["bus_voltage_upper:bus_2"]
    );

    assert_eq!(verdict.outcome, VerdictOutcome::ReturnedForReOptimization);
    assert_eq!(
        verdict.violated_constraints,
        vec!["bus_voltage_upper:bus_2"]
    );
    assert_ne!(verdict.outcome, VerdictOutcome::WithinLimits);

    let (first, second) = run_fixture_twice(outcome);
    assert_eq!(first, second);
}

// ---------------------------------------------------------------------------
// F-IF-3 — branch/thermal overload, HARD (above the physical rating)
// ---------------------------------------------------------------------------

#[test]
fn hard_branch_overload_fixture_is_rejected_with_the_designed_constraint_named() {
    // DESIGNED violation: line_2_3 loading 105 percent — strictly above the
    // physical equipment rating (1.0), the hard-violation class. The
    // currents are consistent (0.525 kA of the 0.5 kA rating).
    let mut outcome = passing_outcome();
    outcome.branch_results[0].loading_percent = 105.0;
    outcome.branch_results[0].i_from_ka = 1.05 * BRANCH_RATED_KA;
    outcome.branch_results[0].i_to_ka = 1.05 * BRANCH_RATED_KA - 0.000_5;
    let (versioned, registry) = model_and_registry();
    let (_, report, verdict) = run_full_gate(outcome.clone(), &versioned, &registry);

    let report = report.expect("a completed attempt has a limit report");
    assert_eq!(report.violations.len(), 1, "single-violation fixture");
    assert_eq!(
        report.violated_constraint_names(),
        vec!["branch_thermal:line_2_3"]
    );
    assert!(
        is_hard_violation(&report.violations[0]),
        "the designed violation is the hard class (above the physical rating)"
    );

    // Pre-registered verdict (rule 4 of D-P4.07-1): any HARD violation is
    // REJECTED with every violated constraint named. The command layer
    // maps this to VALIDATED → REJECTED with
    // `network_limit:branch_thermal:line_2_3`.
    assert_eq!(verdict.outcome, VerdictOutcome::Rejected);
    assert_eq!(verdict.outcome_code, "rejected");
    assert_eq!(
        verdict.violated_constraints,
        vec!["branch_thermal:line_2_3"]
    );
    assert_eq!(
        verdict.evidence.violations_named,
        vec!["branch_thermal:line_2_3"]
    );
    assert_ne!(verdict.outcome, VerdictOutcome::WithinLimits);
    // A hard violation is never "fixed" by a gate-side constraining
    // proposal.
    assert!(verdict.constrain_proposal.is_none());

    let (first, second) = run_fixture_twice(outcome);
    assert_eq!(first.outcome, VerdictOutcome::Rejected);
    assert_eq!(first, second);
}

// ---------------------------------------------------------------------------
// F-IF-4 — transformer overload, HARD (above the physical rating)
// ---------------------------------------------------------------------------

#[test]
fn hard_transformer_overload_fixture_is_rejected_with_the_designed_constraint_named() {
    // DESIGNED violation: trafo_1_2 loading 105 percent of the 0.4 MVA
    // rating — the hard class. Everything else at the passing values.
    let mut outcome = passing_outcome();
    outcome.transformer_results[0].loading_percent = 105.0;
    outcome.transformer_results[0].i_hv_ka = 1.05 * TRAFO_FULL_LOAD_HV_KA;
    outcome.transformer_results[0].i_lv_ka = 1.05 * TRAFO_FULL_LOAD_LV_KA;
    let (versioned, registry) = model_and_registry();
    let (_, report, verdict) = run_full_gate(outcome.clone(), &versioned, &registry);

    let report = report.expect("a completed attempt has a limit report");
    assert_eq!(report.violations.len(), 1, "single-violation fixture");
    assert_eq!(
        report.violated_constraint_names(),
        vec!["transformer_loading:trafo_1_2"]
    );
    assert!(is_hard_violation(&report.violations[0]));

    assert_eq!(verdict.outcome, VerdictOutcome::Rejected);
    assert_eq!(
        verdict.violated_constraints,
        vec!["transformer_loading:trafo_1_2"]
    );
    assert_ne!(verdict.outcome, VerdictOutcome::WithinLimits);
    assert!(verdict.constrain_proposal.is_none());

    let (first, second) = run_fixture_twice(outcome);
    assert_eq!(first, second);
}

// ---------------------------------------------------------------------------
// F-IF-5 — power-flow non-convergence (P4.08 / D-P4.05-2)
// ---------------------------------------------------------------------------

#[test]
fn non_convergence_is_inconclusive_never_a_pass_and_never_rejected_by_limit() {
    // DESIGNED fault: the worker reports non-convergence. No converged
    // diagnostics exist, so no limit verdict exists either.
    let outcome = backend_outcome(
        &[("bus_1", 1.02), ("bus_2", 1.01), ("bus_3", 0.985)],
        76.0,
        62.5,
        false,
    );
    let (versioned, registry) = model_and_registry();
    let (pipeline, report, verdict) = run_full_gate(outcome.clone(), &versioned, &registry);

    // The pipeline completes IN-BAND: non-convergence is a RESULT (the
    // attempt record exists), never silently retried and never a pass.
    let attempt = match &pipeline {
        GatePipelineOutcome::Completed { attempt, .. } => attempt,
        other => panic!("non-convergence is in-band, got {other:?}"),
    };
    match &attempt.output {
        AttemptOutput::Inconclusive { result, reason } => {
            assert_eq!(result.status, PowerFlowStatus::DidNotConverge);
            assert!(reason.contains("never a pass"));
            assert!(reason.contains("retry policy: NONE"));
        }
        other => panic!("expected the inconclusive output, got {other:?}"),
    }

    // The limit report is inconclusive too (nothing was fabricated) and no
    // violations are named.
    let report = report.expect("a completed attempt has a limit report");
    assert!(report.violations.is_empty());
    assert!(matches!(report.status, LimitCheckStatus::Inconclusive { .. }));

    // Pre-registered verdict: the first-class INCONCLUSIVE outcome —
    // never a pass, never a limit-rejection, retry policy NONE.
    assert_eq!(verdict.outcome, VerdictOutcome::Inconclusive);
    assert_eq!(verdict.outcome_code, "inconclusive");
    assert_ne!(verdict.outcome, VerdictOutcome::WithinLimits);
    assert_ne!(verdict.outcome, VerdictOutcome::Rejected);
    assert_ne!(verdict.outcome, VerdictOutcome::Constrained);
    assert_ne!(verdict.outcome, VerdictOutcome::ReturnedForReOptimization);
    assert!(verdict.reason.contains("Retry policy: NONE"));
    assert!(verdict.violated_constraints.is_empty());

    let (first, second) = run_fixture_twice(outcome);
    assert_eq!(first.outcome, VerdictOutcome::Inconclusive);
    assert_eq!(first, second);
}

// ---------------------------------------------------------------------------
// F-IF-6 — stale model (P4.08: the MODEL_STALE_OR_INCOMPLETE HOLD trigger)
// ---------------------------------------------------------------------------

#[test]
fn a_stale_model_aborts_before_the_power_flow_and_routes_held_per_p4_08() {
    // DESIGNED fault: a source change after registration makes the model
    // stale beyond the health bound. The gate must abort at step 3 BEFORE
    // any power flow, and the P4.08 policy table classifies
    // MODEL_STALE_OR_INCOMPLETE as HOLD — the candidate is parked, never
    // passed and never silently rejected-by-limit.
    let (versioned, registry) = stale_model_and_registry();
    let backend = Arc::new(ScriptedBackend::new(passing_outcome()));
    let provider =
        PandapowerGridProvider::new(backend.clone(), flexcore_grid_api::adapter::balanced_worker_scope())
            .expect("balanced scope accepted");
    let pipeline = run_feasibility_gate(GateRunRequest {
        candidate: &candidate(),
        sources: &sources(),
        config: &CandidateValidationConfig::default(),
        versioned_model: &versioned,
        registry: &registry,
        portfolio_elements: &portfolio(),
        health_config: &ModelHealthConfig::default(),
        provider: &provider,
        limits: &test_limits(),
        attempt_id: "attempt-rej-if-6".to_string(),
        now_millis: NOW,
    });

    // The abort happened at step 3, BEFORE the backend was ever consulted.
    match &pipeline {
        GatePipelineOutcome::Aborted { step, abort, evidence } => {
            assert_eq!(*step, GateStep::Step3ModelApplication);
            match abort {
                GateAbort::ModelNotHealthy { findings } => {
                    assert!(findings
                        .iter()
                        .any(|finding| matches!(finding, HealthFinding::Stale { .. })));
                }
                other => panic!("expected ModelNotHealthy, got {other:?}"),
            }
            let last = evidence.last().expect("row");
            assert_eq!(last.rule_code, "MODEL_NOT_HEALTHY");
        }
        other => panic!("expected an abort, got {other:?}"),
    }
    assert_eq!(
        backend.call_count(),
        0,
        "a stale model must abort before the power flow"
    );

    // The routed verdict: HELD per the P4.08 policy table (never a pass).
    let verdict = route_pipeline_outcome(&pipeline, None, None, NOW);
    assert_eq!(verdict.outcome, VerdictOutcome::Held);
    assert_ne!(verdict.outcome, VerdictOutcome::WithinLimits);
    let fail_closed = verdict
        .evidence
        .fail_closed
        .as_ref()
        .expect("the fail-closed outcome travels on the evidence");
    assert_eq!(fail_closed.trigger, FailClosedTrigger::ModelStaleOrIncomplete);
    assert_eq!(fail_closed.action, FailClosedAction::Hold);
    assert!(!fail_closed.reason.is_empty(), "silence is never legal");

    // Determinism: the same fixture run twice gives identical verdicts.
    let (versioned2, registry2) = stale_model_and_registry();
    let (_, _, second) = run_full_gate(passing_outcome(), &versioned2, &registry2);
    assert_eq!(second.outcome, VerdictOutcome::Held);
    assert_eq!(second, verdict, "the two runs must produce identical verdicts");
}

// ---------------------------------------------------------------------------
// Control — the all-within-limits outcome passes (not a rejection fixture)
// ---------------------------------------------------------------------------

/// A control, not a rejection fixture: it proves the harness can detect a
/// pass, so every rejection above is a designed rejection, not a harness
/// artifact that rejects everything.
#[test]
fn the_control_fixture_passes_all_limits() {
    let (versioned, registry) = model_and_registry();
    let (_, report, verdict) = run_full_gate(passing_outcome(), &versioned, &registry);
    let report = report.expect("a completed attempt has a limit report");
    assert_eq!(report.status, LimitCheckStatus::WithinLimits);
    assert!(report.violations.is_empty());
    assert_eq!(verdict.outcome, VerdictOutcome::WithinLimits);
    assert!(verdict.violated_constraints.is_empty());
}
