// P4.07 — Steps 6–8 integration tests: the inconclusive category, the
// diagnostics/snapshot echo, and the reject / constrain / return routing
// (ledger P4.07 acceptance; report §12.5 steps 6–8). Decision record:
// ledger/phase-04-grid-feasibility-dispatch/06_09_gate_completion.md
// (D-P4.07-1..3).
//
// Covered here:
// 1. EVERY routing outcome is reachable on a designed fixture:
//    WithinLimits, Rejected (hard violation AND fail-closed triggers),
//    Constrained (single operational thermal violation, proposal
//    attached), ReturnedForReOptimization (voltage, capability, and
//    multi-violation fixtures), Inconclusive (non-convergence), Held
//    (HOLD-class fail-closed triggers).
// 2. Inconclusive NEVER routes to reject (and never to constrain/return).
// 3. Every routing carries its evidence, and the decision reference
//    (D-P4.07-1) is on EVERY verdict.
// 4. The constraining proposal is evidence: scale factor, proposed
//    values, and the evidence-not-applied statement.
// 5. The §12.5 step 7 diagnostics and input-snapshot echo travel on the
//    evidence; the FeasibilityCheckCompleted payload carries the outcome,
//    the model version + hash, and the evidence references.
// 6. Coverage limitations and scope limit gaps are stated, not hidden.
// 7. Tier 2 candidates carry the SOCP exactness caveat on the evidence
//    (the P4.09 travel rule, exercised through the boundary gate).

use flexcore_grid_api::fail_closed::{
    fail_closed_outcome, FailClosedTrigger, HOLD_RELEASE_POLICY,
};
use flexcore_grid_api::feasibility::{
    AttemptOutput, CandidateRefs, FeasibilityAttemptRecord, GateAbort, GatePipelineOutcome,
    UNBALANCED_DEFERRAL_NOTE,
};
use flexcore_grid_api::limits::{
    check_limits, GeneratorLimit, GeneratorLimitSet, LimitCheckReport,
};
use flexcore_grid_api::port::{
    AppliedInjectionRecord, BranchLoading, BusVoltage, CoverageLimitation, Diagnostic,
    GridModelRef, NetworkLimits, PowerFlowResult, PowerFlowResiduals, PowerFlowStatus,
    TransformerLoading,
};
use flexcore_grid_api::scope::{NotModelled, PhaseScope, ScopeDeclaration, TopologyScope};
use flexcore_grid_api::socp_boundary::{
    gate_socp_boundary, check_relaxation_conditions, TierProvenance, RelaxationResidualRecord,
    RelaxationResidualReport, SOCP_EXACTNESS_CAVEAT,
};
use flexcore_grid_api::verdict::{
    route_candidate, route_fail_closed, route_pipeline_outcome, RoutingInput, VerdictOutcome,
    ROUTING_DECISION_REF,
};
use flexcore_contracts::units::ReactiveSignConvention;
use std::time::Duration;

const NOW: i64 = 10_000;
const EPS: f64 = 1.0e-12;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn model_ref() -> GridModelRef {
    GridModelRef {
        model_id: "feeder-verdict-test".to_string(),
        version: "3".to_string(),
        content_hash: "hash-verdict-test".to_string(),
    }
}

fn limits() -> NetworkLimits {
    NetworkLimits {
        voltage_min_pu: 0.9,
        voltage_max_pu: 1.1,
        max_branch_loading_fraction: 0.9,
        max_transformer_loading_fraction: 0.9,
    }
}

fn generators() -> GeneratorLimitSet {
    GeneratorLimitSet {
        limits: vec![GeneratorLimit {
            bus_id: "bus_3".to_string(),
            s_max_mva: 4.0,
            source_note: "fixture capability cap-1".to_string(),
        }],
    }
}

fn scope() -> ScopeDeclaration {
    ScopeDeclaration::new(
        "verdict-test-evaluator".to_string(),
        PhaseScope::Balanced,
        TopologyScope::RadialOnly,
        vec![
            flexcore_grid_api::scope::EquipmentKind::Line,
            flexcore_grid_api::scope::EquipmentKind::TwoWindingTransformer,
            flexcore_grid_api::scope::EquipmentKind::ExternalGrid,
        ],
        vec![NotModelled::new(
            "unbalanced_three_phase".to_string(),
            "not modelled by this evaluator".to_string(),
        )
        .expect("valid")],
        "runs an AC power-flow evaluation of specified injections and controls; \
         it is NOT an AC-OPF optimizer"
            .to_string(),
        vec!["P4.07 verdict-test scope".to_string()],
    )
    .expect("valid scope")
}

/// A converged result with hand-computed values (the P4.02 fixture bases:
/// line rated 500 A, trafo rated 0.4 MVA).
fn converged(
    voltages: &[(&str, f64)],
    branches: &[(&str, f64)],
    trafos: &[(&str, f64)],
    injections: &[(&str, &str, f64, f64)],
) -> PowerFlowResult {
    let kv = |bus: &str, vm_pu: f64| vm_pu * if bus == "bus_1" { 20.0 } else { 0.4 };
    PowerFlowResult {
        status: PowerFlowStatus::Converged,
        model_ref: model_ref(),
        provider_id: "test-provider".to_string(),
        provider_version: "3.5.6".to_string(),
        bus_voltages: voltages
            .iter()
            .map(|(bus, vm)| BusVoltage {
                bus_id: (*bus).to_string(),
                vm_pu: *vm,
                vm_kv: kv(bus, *vm),
            })
            .collect(),
        branch_loadings: branches
            .iter()
            .map(|(id, fraction)| BranchLoading {
                branch_id: (*id).to_string(),
                current_from_a: fraction * 500.0,
                current_to_a: fraction * 500.0,
                loading_fraction: *fraction,
            })
            .collect(),
        transformer_loadings: trafos
            .iter()
            .map(|(id, fraction)| TransformerLoading {
                transformer_id: (*id).to_string(),
                loading_fraction: *fraction,
                hv_current_a: fraction * 11.547,
                lv_current_a: fraction * 577.35,
            })
            .collect(),
        applied_injections: injections
            .iter()
            .map(|(id, bus, p, q)| AppliedInjectionRecord {
                injection_id: (*id).to_string(),
                bus_id: (*bus).to_string(),
                active_power_mw: *p,
                reactive_power_mvar: *q,
                reactive_sign_convention: ReactiveSignConvention::InjectionPositive,
                interval_start: None,
            })
            .collect(),
        residuals: Diagnostic::Available(PowerFlowResiduals {
            iterations: Some(4),
            max_power_mismatch_mw: Some(1.0e-9),
            max_voltage_update_pu: Some(1.0e-10),
            provider_status_text: "converged".to_string(),
        }),
        coverage_limitations: vec![CoverageLimitation {
            label: "unbalanced_three_phase".to_string(),
            statement: "not modelled (test scope)".to_string(),
        }],
        warnings: Vec::new(),
        runtime: Duration::from_millis(1),
    }
}

fn passing_result() -> PowerFlowResult {
    converged(
        &[("bus_1", 1.02), ("bus_2", 1.01), ("bus_3", 0.985)],
        &[("line_2_3", 0.76)],
        &[("trafo_1_2", 0.625)],
        &[("inj-1", "bus_3", 2.5, 0.5)],
    )
}

fn non_converged_result() -> PowerFlowResult {
    let mut result = passing_result();
    result.status = PowerFlowStatus::DidNotConverge;
    result.residuals = Diagnostic::NotAvailable {
        reason: "no converged solution; no residuals exist".to_string(),
    };
    result
}

fn candidate_refs() -> CandidateRefs {
    CandidateRefs {
        candidate_id: "cand-verdict-1".to_string(),
        schedule_id: "0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8b".to_string(),
        revision: 2,
        envelope_version: "env-v1".to_string(),
        result_record_snapshot_ref: "schedule_input_snapshot:env-v1".to_string(),
    }
}

fn attempt_for(result: PowerFlowResult) -> FeasibilityAttemptRecord {
    let output = if result.status == PowerFlowStatus::Converged {
        AttemptOutput::PowerFlowEvaluated { result }
    } else {
        let result = result;
        AttemptOutput::Inconclusive {
            result,
            reason: "the power flow did not converge".to_string(),
        }
    };
    FeasibilityAttemptRecord {
        attempt_id: "attempt-verdict-1".to_string(),
        started_at_millis: NOW - 10,
        finished_at_millis: NOW,
        candidate_refs: candidate_refs(),
        model_ref: model_ref(),
        injection_derivation_refs: vec![
            "derived_injection:sp-1:asset=asset-1:baseline@9900:quality=GOOD".to_string(),
        ],
        output,
        provider_id: "test-provider".to_string(),
        provider_version: "3.5.6".to_string(),
        unbalanced_deferral_note: UNBALANCED_DEFERRAL_NOTE.to_string(),
    }
}

fn report_for(result: &PowerFlowResult) -> LimitCheckReport {
    check_limits(result, &limits(), &generators(), &scope())
}

fn route(attempt: &FeasibilityAttemptRecord, report: &LimitCheckReport) -> flexcore_grid_api::verdict::GateVerdict {
    route_candidate(RoutingInput {
        attempt,
        limit_report: report,
        socp_gate: None,
        fail_closed: None,
        pipeline_evidence: &[],
        now_millis: NOW,
    })
}

// ---------------------------------------------------------------------------
// Within limits: evidence, snapshot echo, event, coverage qualifications
// ---------------------------------------------------------------------------

#[test]
fn a_within_limits_verdict_carries_evidence_echo_and_event() {
    let result = passing_result();
    let attempt = attempt_for(result.clone());
    let report = report_for(&result);
    let verdict = route(&attempt, &report);

    assert_eq!(verdict.outcome, VerdictOutcome::WithinLimits);
    assert_eq!(verdict.outcome_code, "within_limits");
    assert_eq!(verdict.decision_ref, ROUTING_DECISION_REF);
    assert!(!verdict.reason.is_empty());
    assert!(verdict.reason.contains("not a field-safety"));

    // Input-snapshot echo: model version + hash, provider version,
    // derivation refs (D-P4.07-2).
    let echo = verdict.evidence.snapshot_echo.as_ref().expect("echo");
    assert_eq!(echo.model_id, "feeder-verdict-test");
    assert_eq!(echo.model_version, "3");
    assert_eq!(echo.model_content_hash, "hash-verdict-test");
    assert_eq!(echo.provider_version, "3.5.6");
    assert_eq!(echo.injection_derivation_refs.len(), 1);

    // Coverage limitations travel; scope gaps are stated, not hidden.
    assert_eq!(verdict.evidence.coverage_limitations.len(), 1);
    assert_eq!(verdict.evidence.scope_limit_gaps.len(), 1);
    assert!(verdict.evidence.scope_limit_gaps[0].contains("cannot be verified"));

    // Diagnostics finalised onto the evidence.
    assert_eq!(verdict.evidence.checks_recorded, 9);
    assert!(verdict.evidence.worst_margin_fraction.is_some());
    assert!(verdict.evidence.binding_constraint.is_some());

    // The completed event: outcome, model version, evidence references.
    let event = &verdict.completed_event;
    assert_eq!(event.outcome_code, "within_limits");
    assert_eq!(event.model_id, "feeder-verdict-test");
    assert_eq!(event.model_version, "3");
    assert_eq!(event.model_content_hash, "hash-verdict-test");
    assert!(event
        .evidence_refs
        .contains(&"attempt-verdict-1".to_string()));
    assert!(event.evidence_refs.contains(&"schedule_input_snapshot:env-v1".to_string()));
    assert!(event
        .evidence_refs
        .iter()
        .any(|reference| reference.contains("derived_injection:sp-1")));
    assert_eq!(event.occurred_at_millis, NOW);
}

// ---------------------------------------------------------------------------
// CONSTRAIN: a single operational thermal violation, proposal = evidence
// ---------------------------------------------------------------------------

#[test]
fn a_single_branch_overload_routes_to_constrain_with_a_recorded_proposal() {
    let result = converged(
        &[("bus_1", 1.02), ("bus_2", 1.01), ("bus_3", 0.985)],
        &[("line_2_3", 0.95)],
        &[("trafo_1_2", 0.625)],
        &[("inj-1", "bus_3", 2.5, 0.5)],
    );
    let attempt = attempt_for(result.clone());
    let report = report_for(&result);
    let verdict = route(&attempt, &report);

    assert_eq!(verdict.outcome, VerdictOutcome::Constrained);
    assert_eq!(verdict.outcome_code, "constrained");
    assert_eq!(verdict.decision_ref, ROUTING_DECISION_REF);
    assert_eq!(
        verdict.violated_constraints,
        vec!["branch_thermal:line_2_3"]
    );

    let proposal = verdict.constrain_proposal.as_ref().expect("proposal");
    assert_eq!(proposal.violated_constraint, "branch_thermal:line_2_3");
    // scale = limit / measured = 0.9 / 0.95 (hand-computed).
    assert!((proposal.scale_factor - 0.9 / 0.95).abs() < EPS);
    assert!((proposal.limit - 0.9).abs() < EPS);
    assert!((proposal.predicted_value - 0.9).abs() < EPS);
    // Every applied injection is scaled, values recorded.
    assert_eq!(proposal.proposed_injections.len(), 1);
    let proposed = &proposal.proposed_injections[0];
    assert_eq!(proposed.injection_id, "inj-1");
    assert!((proposed.proposed_p_mw - 2.5 * (0.9 / 0.95)).abs() < EPS);
    assert!((proposed.proposed_q_mvar - 0.5 * (0.9 / 0.95)).abs() < EPS);
    // The proposal is EVIDENCE, not an applied mutation.
    assert!(proposal.statement.contains("EVIDENCE"));
    assert!(proposal.statement.contains("re-enter the feasibility gate"));
    assert!(proposal.statement.contains("P4.37"));
}

#[test]
fn a_single_transformer_overload_also_routes_to_constrain() {
    let result = converged(
        &[("bus_1", 1.02), ("bus_2", 1.01), ("bus_3", 0.985)],
        &[("line_2_3", 0.76)],
        &[("trafo_1_2", 0.93)],
        &[("inj-1", "bus_3", 2.5, 0.5)],
    );
    let attempt = attempt_for(result.clone());
    let report = report_for(&result);
    let verdict = route(&attempt, &report);
    assert_eq!(verdict.outcome, VerdictOutcome::Constrained);
    assert_eq!(
        verdict.violated_constraints,
        vec!["transformer_loading:trafo_1_2"]
    );
    assert!(verdict.constrain_proposal.is_some());
}

// ---------------------------------------------------------------------------
// RETURN: voltage, capability, and multi-violation fixtures
// ---------------------------------------------------------------------------

#[test]
fn a_single_voltage_violation_routes_to_return_for_re_optimization() {
    let result = converged(
        &[("bus_1", 1.02), ("bus_2", 1.01), ("bus_3", 0.88)],
        &[("line_2_3", 0.76)],
        &[("trafo_1_2", 0.625)],
        &[("inj-1", "bus_3", 2.5, 0.5)],
    );
    let attempt = attempt_for(result.clone());
    let report = report_for(&result);
    let verdict = route(&attempt, &report);

    assert_eq!(verdict.outcome, VerdictOutcome::ReturnedForReOptimization);
    assert_eq!(
        verdict.violated_constraints,
        vec!["bus_voltage_lower:bus_3"]
    );
    assert!(verdict.constrain_proposal.is_none());
    assert!(verdict.reason.contains("never optimizes"));
}

#[test]
fn a_single_capability_violation_routes_to_return_with_the_constraint_named() {
    let result = converged(
        &[("bus_1", 1.02), ("bus_2", 1.01), ("bus_3", 0.985)],
        &[("line_2_3", 0.76)],
        &[("trafo_1_2", 0.625)],
        &[("inj-1", "bus_3", 4.0, 3.0)],
    );
    let attempt = attempt_for(result.clone());
    let report = report_for(&result);
    let verdict = route(&attempt, &report);
    assert_eq!(verdict.outcome, VerdictOutcome::ReturnedForReOptimization);
    assert_eq!(
        verdict.violated_constraints,
        vec!["generator_apparent_power:bus_3"]
    );
    assert!(verdict.reason.contains("capability"));
}

#[test]
fn a_multi_violation_fixture_routes_to_return_with_every_constraint_named() {
    let result = converged(
        &[("bus_1", 1.02), ("bus_2", 1.01), ("bus_3", 0.985)],
        &[("line_2_3", 0.95)],
        &[("trafo_1_2", 0.93)],
        &[("inj-1", "bus_3", 2.5, 0.5)],
    );
    let attempt = attempt_for(result.clone());
    let report = report_for(&result);
    let verdict = route(&attempt, &report);
    assert_eq!(verdict.outcome, VerdictOutcome::ReturnedForReOptimization);
    assert_eq!(verdict.violated_constraints.len(), 2);
    assert!(verdict
        .violated_constraints
        .contains(&"branch_thermal:line_2_3".to_string()));
    assert!(verdict
        .violated_constraints
        .contains(&"transformer_loading:trafo_1_2".to_string()));
    assert!(verdict.reason.contains("EVERY violated constraint"));
}

// ---------------------------------------------------------------------------
// REJECT: a hard violation (above the physical rating)
// ---------------------------------------------------------------------------

#[test]
fn a_hard_violation_routes_to_reject_with_all_constraints_named() {
    let result = converged(
        &[("bus_1", 1.02), ("bus_2", 1.01), ("bus_3", 0.985)],
        &[("line_2_3", 1.05)],
        &[("trafo_1_2", 0.625)],
        &[("inj-1", "bus_3", 2.5, 0.5)],
    );
    let attempt = attempt_for(result.clone());
    let report = report_for(&result);
    let verdict = route(&attempt, &report);
    assert_eq!(verdict.outcome, VerdictOutcome::Rejected);
    assert_eq!(verdict.outcome_code, "rejected");
    assert_eq!(verdict.decision_ref, ROUTING_DECISION_REF);
    assert_eq!(verdict.violated_constraints, vec!["branch_thermal:line_2_3"]);
    assert!(verdict.constrain_proposal.is_none());
    // The evidence is attached (attempt echo + named violations).
    assert_eq!(verdict.evidence.violations_named, vec!["branch_thermal:line_2_3"]);
    assert!(verdict.evidence.snapshot_echo.is_some());
}

// ---------------------------------------------------------------------------
// INCONCLUSIVE: non-convergence is first-class and NEVER routes to reject
// ---------------------------------------------------------------------------

#[test]
fn non_convergence_routes_to_inconclusive_and_never_to_reject() {
    let result = non_converged_result();
    let attempt = attempt_for(result.clone());
    let report = report_for(&result);
    let verdict = route(&attempt, &report);

    assert_eq!(verdict.outcome, VerdictOutcome::Inconclusive);
    assert_eq!(verdict.outcome_code, "inconclusive");
    assert_ne!(verdict.outcome, VerdictOutcome::Rejected);
    assert_ne!(verdict.outcome, VerdictOutcome::Constrained);
    assert_ne!(verdict.outcome, VerdictOutcome::ReturnedForReOptimization);
    assert_ne!(verdict.outcome, VerdictOutcome::WithinLimits);
    // The retry policy is NONE and the category is named inconclusive —
    // never a pass and never a limit-violation.
    assert!(verdict.reason.contains("Retry policy: NONE"));
    assert!(verdict.reason.contains("never a pass"));
    assert!(verdict.reason.contains("never a limit-violation"));
}

// ---------------------------------------------------------------------------
// Fail-closed routing: REJECT-class and HOLD-class triggers
// ---------------------------------------------------------------------------

#[test]
fn a_reject_class_fail_closed_trigger_routes_to_rejected() {
    let outcome = fail_closed_outcome(
        FailClosedTrigger::TelemetryQualityBlocked,
        "baseline quality STALE",
    );
    let verdict = route_fail_closed(&candidate_refs(), &outcome, Some(&model_ref()), NOW);
    assert_eq!(verdict.outcome, VerdictOutcome::Rejected);
    assert_eq!(verdict.decision_ref, ROUTING_DECISION_REF);
    assert!(verdict.evidence.fail_closed.is_some());
    assert!(verdict.reason.contains("STALE"));
}

#[test]
fn a_hold_class_fail_closed_trigger_routes_to_held_never_a_pass() {
    assert!(!HOLD_RELEASE_POLICY.trim().is_empty());
    let outcome =
        fail_closed_outcome(FailClosedTrigger::ModelHashMismatch, "mid-gate model change");
    let verdict = route_fail_closed(&candidate_refs(), &outcome, Some(&model_ref()), NOW);
    assert_eq!(verdict.outcome, VerdictOutcome::Held);
    assert_ne!(verdict.outcome, VerdictOutcome::WithinLimits);
    assert!(verdict.evidence.fail_closed.is_some());
}

#[test]
fn an_explicit_fail_closed_input_overrides_the_limit_routing() {
    let result = passing_result();
    let attempt = attempt_for(result.clone());
    let report = report_for(&result);
    let outcome = fail_closed_outcome(FailClosedTrigger::SnapshotRefUnresolvable, "");
    let verdict = route_candidate(RoutingInput {
        attempt: &attempt,
        limit_report: &report,
        socp_gate: None,
        fail_closed: Some(&outcome),
        pipeline_evidence: &[],
        now_millis: NOW,
    });
    assert_eq!(verdict.outcome, VerdictOutcome::Rejected);
    assert!(verdict.reason.contains("snapshot"));
}

// ---------------------------------------------------------------------------
// Tier 2: the SOCP caveat travels onto the verdict evidence (P4.09)
// ---------------------------------------------------------------------------

#[test]
fn a_tier_2_candidate_carries_the_socp_caveat_on_its_evidence() {
    let result = passing_result();
    let attempt = attempt_for(result.clone());
    let report = report_for(&result);
    let conditions = check_relaxation_conditions(&model_ref_ir(), &scope());
    let residuals = RelaxationResidualReport {
        records: vec![RelaxationResidualRecord {
            branch: "line_2_3".to_string(),
            cone_name: "cone_23".to_string(),
            residual: 0.0,
        }],
        tolerance: 1.0e-4,
    };
    let socp = gate_socp_boundary(&TierProvenance::Tier2SocpRelaxation, &conditions, Some(&residuals));
    let verdict = route_candidate(RoutingInput {
        attempt: &attempt,
        limit_report: &report,
        socp_gate: Some(&socp),
        fail_closed: None,
        pipeline_evidence: &[],
        now_millis: NOW,
    });
    assert_eq!(verdict.outcome, VerdictOutcome::WithinLimits);
    assert!(verdict.evidence.caveats.iter().any(|caveat| caveat == SOCP_EXACTNESS_CAVEAT));
    assert!(verdict
        .evidence
        .findings
        .iter()
        .any(|finding| finding.contains("reverse_power_flow")));
}

#[test]
fn a_tier_2_candidate_without_a_residual_record_is_fail_closed_rejected() {
    let result = passing_result();
    let attempt = attempt_for(result.clone());
    let report = report_for(&result);
    let conditions = check_relaxation_conditions(&model_ref_ir(), &scope());
    let socp = gate_socp_boundary(&TierProvenance::Tier2SocpRelaxation, &conditions, None);
    let verdict = route_candidate(RoutingInput {
        attempt: &attempt,
        limit_report: &report,
        socp_gate: Some(&socp),
        fail_closed: None,
        pipeline_evidence: &[],
        now_millis: NOW,
    });
    assert_eq!(verdict.outcome, VerdictOutcome::Rejected);
    assert!(verdict.reason.contains("relaxation-residual record"));
}

/// A minimal valid model IR for the condition checklist.
fn model_ref_ir() -> flexcore_grid_api::port::VersionedNetworkModel {
    use flexcore_grid_api::port::{BranchSpec, BusSpec, ExternalGridSpec, VersionedNetworkModel};
    VersionedNetworkModel {
        model_ref: model_ref(),
        base_mva: 100.0,
        buses: vec![BusSpec {
            bus_id: "bus_1".to_string(),
            nominal_kv: 20.0,
            is_slack: true,
        }],
        branches: Vec::new(),
        transformers: Vec::new(),
        external_grids: vec![ExternalGridSpec {
            bus_id: "bus_1".to_string(),
            vm_pu: 1.02,
            va_degree: 0.0,
        }],
    }
}

// ---------------------------------------------------------------------------
// The decision reference and a non-empty reason are on EVERY verdict
// ---------------------------------------------------------------------------

#[test]
fn every_verdict_carries_the_decision_reference_and_a_reason() {
    let results = vec![
        passing_result(),
        converged(
            &[("bus_1", 1.02), ("bus_2", 1.01), ("bus_3", 0.985)],
            &[("line_2_3", 0.95)],
            &[("trafo_1_2", 0.625)],
            &[("inj-1", "bus_3", 2.5, 0.5)],
        ),
        converged(
            &[("bus_1", 1.02), ("bus_2", 1.01), ("bus_3", 0.88)],
            &[("line_2_3", 0.76)],
            &[("trafo_1_2", 0.625)],
            &[("inj-1", "bus_3", 2.5, 0.5)],
        ),
        converged(
            &[("bus_1", 1.02), ("bus_2", 1.01), ("bus_3", 0.985)],
            &[("line_2_3", 1.05)],
            &[("trafo_1_2", 0.625)],
            &[("inj-1", "bus_3", 2.5, 0.5)],
        ),
        non_converged_result(),
    ];
    for result in &results {
        let attempt = attempt_for(result.clone());
        let report = report_for(result);
        let verdict = route(&attempt, &report);
        assert_eq!(verdict.decision_ref, ROUTING_DECISION_REF);
        assert!(!verdict.reason.trim().is_empty());
        assert_eq!(verdict.outcome_code, verdict.outcome.as_str());
        assert!(!verdict.completed_event.reason.trim().is_empty());
    }
}

// ---------------------------------------------------------------------------
// Fail-closed routing totality: every trigger routes to REJECTED or HELD
// ---------------------------------------------------------------------------

#[test]
fn every_fail_closed_trigger_routes_to_rejected_or_held_never_a_pass() {
    for trigger in FailClosedTrigger::ALL {
        let outcome = fail_closed_outcome(trigger, "");
        let verdict = route_fail_closed(&candidate_refs(), &outcome, Some(&model_ref()), NOW);
        assert!(
            verdict.outcome == VerdictOutcome::Rejected
                || verdict.outcome == VerdictOutcome::Held,
            "trigger {} routed to {:?} through the verdict layer",
            trigger.code(),
            verdict.outcome
        );
        assert_ne!(verdict.outcome, VerdictOutcome::WithinLimits);
        assert!(!verdict.reason.trim().is_empty());
    }
}

// ---------------------------------------------------------------------------
// Pipeline-outcome routing: aborts map to their fail-closed triggers
// ---------------------------------------------------------------------------

#[test]
fn a_pipeline_abort_maps_to_its_fail_closed_trigger_and_holds() {
    let outcome = GatePipelineOutcome::Aborted {
        step: flexcore_grid_api::feasibility::GateStep::Step3ModelApplication,
        abort: GateAbort::ModelHashMismatch {
            recorded: "hash-a".to_string(),
            computed: "hash-b".to_string(),
        },
        evidence: Vec::new(),
    };
    let verdict = route_pipeline_outcome(&outcome, None, None, NOW);
    assert_eq!(verdict.outcome, VerdictOutcome::Held);
    assert_ne!(verdict.outcome, VerdictOutcome::WithinLimits);
    assert!(verdict.reason.contains("hash"));
    let recorded = verdict.evidence.fail_closed.as_ref().expect("fail-closed");
    assert_eq!(recorded.trigger, FailClosedTrigger::ModelHashMismatch);
}

#[test]
fn a_completed_attempt_without_a_limit_report_is_inconclusive_never_a_pass() {
    let result = passing_result();
    let attempt = attempt_for(result);
    let outcome = GatePipelineOutcome::Completed {
        attempt,
        evidence: Vec::new(),
    };
    let verdict = route_pipeline_outcome(&outcome, None, None, NOW);
    assert_eq!(verdict.outcome, VerdictOutcome::Inconclusive);
    assert!(verdict.reason.contains("no limit report"));
}
