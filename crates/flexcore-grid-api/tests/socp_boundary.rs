// P4.09 — SOCP relaxation boundary integration tests (ledger P4.09
// acceptance; report §12.5 branch-flow SOCP paragraph, §12.3). Decision
// record: ledger/phase-04-grid-feasibility-dispatch/06_09_gate_completion.md
// (D-P4.09-1..4).
//
// Covered here:
// 1. A Tier 2 candidate WITH a valid residual record passes the boundary
//    and enters the AC verification normally; the caveat is attached.
// 2. A Tier 2 candidate WITHOUT a residual record (and with a
//    malformed/failed record) is FAIL-CLOSED (the RELAXATION_RECORD_MISSING
//    trigger, REJECT-class per the P4.08 table).
// 3. The relaxation-condition checklist is checked and recorded per case;
//    reverse power flow, voltage upper bounds, and arbitrary network
//    constraints are NEVER assumed exact (§12.5 verbatim).
// 4. A residual-tolerance breach is a RECORDED finding, never hidden —
//    and it does not silently fail OR pass the gate: the independent AC
//    power-flow verification remains the authority.
// 5. A Tier 1 candidate requires no boundary and no residual record.
// 6. The P3.18 cross-check dimension carries the feasibility-verdict
//    fields (tier, outcome code, violated constraints, worst margin
//    fraction, caveats).
// 7. The gate runs normally for a Tier 2 candidate whose relaxation is
//    exact within tolerance: no findings from the residual side.

use flexcore_grid_api::fail_closed::{FailClosedAction, FailClosedTrigger};
use flexcore_grid_api::limits::{
    check_limits, GeneratorLimit, GeneratorLimitSet, LimitCheckStatus,
};
use flexcore_grid_api::port::{
    AppliedInjectionRecord, BranchLoading, BusVoltage, CoverageLimitation, Diagnostic,
    GridModelRef, NetworkLimits, PowerFlowResult, PowerFlowResiduals, PowerFlowStatus,
    TransformerLoading, VersionedNetworkModel, BranchSpec, BusSpec, ExternalGridSpec,
};
use flexcore_grid_api::scope::{NotModelled, PhaseScope, ScopeDeclaration, TopologyScope};
use flexcore_grid_api::socp_boundary::{
    assess_residuals, check_relaxation_conditions, feasibility_verdict_dimension,
    gate_socp_boundary, ConditionOutcome, RelaxationCondition, RelaxationResidualRecord,
    RelaxationResidualReport, ResidualFindingKind, SocpBoundaryGate, TierProvenance,
    DISTFLOW_RESIDUAL_FORMULA, MIRRORED_DEFAULT_RESIDUAL_TOLERANCE, SOCP_EXACTNESS_CAVEAT,
    SOCP_SEAM_STATEMENT, TIER2_VERDICT_STATEMENT,
};
use flexcore_contracts::units::ReactiveSignConvention;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn model_ref() -> GridModelRef {
    GridModelRef {
        model_id: "feeder-socp-test".to_string(),
        version: "1".to_string(),
        content_hash: "hash-socp-test".to_string(),
    }
}

/// The three-bus radial fixture model (the P4.02 fixture shape).
fn model() -> VersionedNetworkModel {
    VersionedNetworkModel {
        model_ref: model_ref(),
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
        transformers: Vec::new(),
        external_grids: vec![ExternalGridSpec {
            bus_id: "bus_1".to_string(),
            vm_pu: 1.02,
            va_degree: 0.0,
        }],
    }
}

fn radial_scope() -> ScopeDeclaration {
    ScopeDeclaration::new(
        "socp-test-evaluator".to_string(),
        PhaseScope::Balanced,
        TopologyScope::RadialOnly,
        vec![flexcore_grid_api::scope::EquipmentKind::Line],
        vec![NotModelled::new(
            "ac_opf".to_string(),
            "the evaluator runs an AC power-flow evaluation; it is NOT an \
             AC-OPF optimizer"
                .to_string(),
        )
        .expect("valid")],
        "runs an AC power-flow evaluation of specified injections and controls; \
         it is NOT an AC-OPF optimizer"
            .to_string(),
        vec!["P4.09 boundary-test scope".to_string()],
    )
    .expect("valid scope")
}

fn meshed_scope() -> ScopeDeclaration {
    ScopeDeclaration::new(
        "socp-test-evaluator".to_string(),
        PhaseScope::Balanced,
        TopologyScope::RadialAndMeshed,
        vec![flexcore_grid_api::scope::EquipmentKind::Line],
        vec![NotModelled::new(
            "ac_opf".to_string(),
            "not an AC-OPF optimizer".to_string(),
        )
        .expect("valid")],
        "runs an AC power-flow evaluation of specified injections and controls; \
         it is NOT an AC-OPF optimizer"
            .to_string(),
        Vec::new(),
    )
    .expect("valid scope")
}

fn converged_result() -> PowerFlowResult {
    PowerFlowResult {
        status: PowerFlowStatus::Converged,
        model_ref: model_ref(),
        provider_id: "test-provider".to_string(),
        provider_version: "3.5.6".to_string(),
        bus_voltages: vec![
            BusVoltage {
                bus_id: "bus_1".to_string(),
                vm_pu: 1.02,
                vm_kv: 20.4,
            },
            BusVoltage {
                bus_id: "bus_3".to_string(),
                vm_pu: 0.99,
                vm_kv: 0.396,
            },
        ],
        branch_loadings: vec![BranchLoading {
            branch_id: "line_2_3".to_string(),
            current_from_a: 300.0,
            current_to_a: 299.0,
            loading_fraction: 0.6,
        }],
        transformer_loadings: Vec::new(),
        applied_injections: vec![AppliedInjectionRecord {
            injection_id: "inj-1".to_string(),
            bus_id: "bus_3".to_string(),
            active_power_mw: 2.0,
            reactive_power_mvar: 0.5,
            reactive_sign_convention: ReactiveSignConvention::InjectionPositive,
            interval_start: None,
        }],
        residuals: Diagnostic::Available(PowerFlowResiduals {
            iterations: Some(3),
            max_power_mismatch_mw: Some(1.0e-9),
            max_voltage_update_pu: Some(1.0e-10),
            provider_status_text: "converged".to_string(),
        }),
        coverage_limitations: vec![CoverageLimitation {
            label: "ac_opf".to_string(),
            statement: "not an AC-OPF optimizer".to_string(),
        }],
        warnings: Vec::new(),
        runtime: Duration::from_millis(1),
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

fn residual_record(branch: &str, residual: f64) -> RelaxationResidualRecord {
    RelaxationResidualRecord {
        branch: branch.to_string(),
        cone_name: "cone_line_2_3".to_string(),
        residual,
    }
}

/// The gate result, asserting the "runs normally" shape: the Tier 2
/// candidate with a valid record enters the AC verification (the limit
/// check over the power-flow result runs and is decisive).
fn tier2_gate(
    residuals: Option<&RelaxationResidualReport>,
) -> SocpBoundaryGate {
    let conditions = check_relaxation_conditions(&model(), &radial_scope());
    gate_socp_boundary(&TierProvenance::Tier2SocpRelaxation, &conditions, residuals)
}

// ---------------------------------------------------------------------------
// 1. Tier 2 WITH a valid residual record: the gate runs normally
// ---------------------------------------------------------------------------

#[test]
fn a_tier_2_candidate_with_a_valid_residual_record_enters_ac_verification() {
    assert!(!SOCP_SEAM_STATEMENT.trim().is_empty());
    assert!(!TIER2_VERDICT_STATEMENT.trim().is_empty());
    let report = RelaxationResidualReport {
        records: vec![residual_record("line_2_3", 0.0)],
        tolerance: MIRRORED_DEFAULT_RESIDUAL_TOLERANCE,
    };
    let socp = tier2_gate(Some(&report));
    match &socp {
        SocpBoundaryGate::Tier2VerifiedForAcCheck {
            condition_summary,
            residual_summary,
            caveats,
            findings,
        } => {
            // The caveat is ALWAYS attached (D-P4.09-2).
            assert!(caveats.iter().any(|caveat| caveat == SOCP_EXACTNESS_CAVEAT));
            // Radial + balanced are satisfied for this case.
            assert!(condition_summary.contains("radial_feeder=satisfied"));
            assert!(condition_summary.contains("balanced_phases=satisfied"));
            // The no-assumption conditions are recorded, every case.
            assert!(condition_summary.contains("reverse_power_flow=not_assumed_exact"));
            assert!(condition_summary.contains("voltage_upper_bound=not_assumed_exact"));
            assert!(condition_summary.contains(
                "arbitrary_network_constraints=not_assumed_exact"
            ));
            // The only findings are the three recorded no-assumption
            // conditions (§12.5); a tight relaxation records no residual
            // findings.
            assert_eq!(findings.len(), 3);
            assert!(findings.iter().all(|finding| finding.contains("NOT assumed")));
            assert!(residual_summary.contains("within"));
        }
        other => panic!("valid Tier 2 record must verify, got {other:?}"),
    }

    // The AC verification runs normally and is decisive: within limits.
    let limit_report = check_limits(&converged_result(), &limits(), &generators(), &radial_scope());
    assert_eq!(limit_report.status, LimitCheckStatus::WithinLimits);
}

// ---------------------------------------------------------------------------
// 2. Tier 2 WITHOUT a usable residual record: FAIL-CLOSED
// ---------------------------------------------------------------------------

#[test]
fn a_tier_2_candidate_without_a_residual_record_is_fail_closed() {
    let socp = tier2_gate(None);
    match &socp {
        SocpBoundaryGate::FailClosed(outcome) => {
            assert_eq!(outcome.trigger, FailClosedTrigger::RelaxationRecordMissing);
            assert_eq!(outcome.action, FailClosedAction::Reject);
            assert!(!outcome.reason.is_empty());
            assert!(outcome.reason.contains("never accepted on the solver's word"));
        }
        other => panic!("missing record must be fail-closed, got {other:?}"),
    }
}

#[test]
fn a_malformed_or_failed_residual_record_is_fail_closed() {
    // Empty record set.
    let empty = RelaxationResidualReport {
        records: Vec::new(),
        tolerance: 1.0e-4,
    };
    assert!(matches!(
        tier2_gate(Some(&empty)),
        SocpBoundaryGate::FailClosed(_)
    ));
    // Non-finite residual.
    let non_finite = RelaxationResidualReport {
        records: vec![residual_record("line_2_3", f64::NAN)],
        tolerance: 1.0e-4,
    };
    assert!(matches!(
        tier2_gate(Some(&non_finite)),
        SocpBoundaryGate::FailClosed(_)
    ));
    // Invalid tolerance.
    let bad_tolerance = RelaxationResidualReport {
        records: vec![residual_record("line_2_3", 0.0)],
        tolerance: 0.0,
    };
    assert!(matches!(
        tier2_gate(Some(&bad_tolerance)),
        SocpBoundaryGate::FailClosed(_)
    ));
    // Empty branch identifier.
    let unnamed = RelaxationResidualReport {
        records: vec![residual_record("", 0.0)],
        tolerance: 1.0e-4,
    };
    assert!(matches!(
        tier2_gate(Some(&unnamed)),
        SocpBoundaryGate::FailClosed(_)
    ));
}

// ---------------------------------------------------------------------------
// 3. The condition checklist: no exactness assumptions, ever
// ---------------------------------------------------------------------------

#[test]
fn the_condition_checklist_never_assumes_exactness_for_the_three_named_cases() {
    let conditions = check_relaxation_conditions(&model(), &radial_scope());
    assert_eq!(conditions.records.len(), 5);
    for condition in [
        RelaxationCondition::ReversePowerFlow,
        RelaxationCondition::VoltageUpperBound,
        RelaxationCondition::ArbitraryNetworkConstraints,
    ] {
        let record = conditions
            .records
            .iter()
            .find(|record| record.condition == condition)
            .expect("recorded");
        match &record.outcome {
            ConditionOutcome::NotAssumedExact { statement } => {
                assert!(statement.contains("NOT assumed"));
            }
            other => panic!(
                "condition {} must be recorded as NOT assumed, got {other:?}",
                condition.code()
            ),
        }
    }
    // Radial + balanced are satisfied for the radial balanced scope.
    let radial = conditions
        .records
        .iter()
        .find(|record| record.condition == RelaxationCondition::RadialFeeder)
        .expect("radial record");
    assert_eq!(radial.outcome, ConditionOutcome::Satisfied);
}

#[test]
fn a_non_radial_scope_records_the_radial_condition_as_not_satisfied() {
    let conditions = check_relaxation_conditions(&model(), &meshed_scope());
    let radial = conditions
        .records
        .iter()
        .find(|record| record.condition == RelaxationCondition::RadialFeeder)
        .expect("radial record");
    match &radial.outcome {
        ConditionOutcome::NotSatisfied { statement } => {
            assert!(statement.contains("radiality"));
        }
        other => panic!("meshed-capable scope must not confirm radiality, got {other:?}"),
    }
    // The gap is a RECORDED FINDING on the gate result, never hidden.
    let report = RelaxationResidualReport {
        records: vec![residual_record("line_2_3", 0.0)],
        tolerance: 1.0e-4,
    };
    let socp = gate_socp_boundary(&TierProvenance::Tier2SocpRelaxation, &conditions, Some(&report));
    match socp {
        SocpBoundaryGate::Tier2VerifiedForAcCheck { findings, .. } => {
            assert!(findings
                .iter()
                .any(|finding| finding.contains("radial_feeder")));
        }
        other => panic!("expected a verified Tier 2 gate with findings, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 4. Tolerance breaches: recorded findings, never hidden
// ---------------------------------------------------------------------------

#[test]
fn a_residual_tolerance_breach_is_a_recorded_finding_that_travels() {
    assert!(DISTFLOW_RESIDUAL_FORMULA.contains("v_i·l_ij"));
    // g_ij = -1e-3 < -1e-4: the cone is VIOLATED at the recorded point.
    let report = RelaxationResidualReport {
        records: vec![residual_record("line_2_3", -1.0e-3)],
        tolerance: 1.0e-4,
    };
    let assessment = assess_residuals(&report);
    assert!(!assessment.all_within_tolerance);
    assert_eq!(assessment.findings.len(), 1);
    assert_eq!(assessment.findings[0].kind, ResidualFindingKind::ConeViolated);
    assert!(assessment.findings[0].statement.contains("VIOLATED"));

    // The finding travels onto the gate result — never hidden.
    let socp = tier2_gate(Some(&report));
    match socp {
        SocpBoundaryGate::Tier2VerifiedForAcCheck { findings, .. } => {
            assert_eq!(findings.len(), 1);
            assert!(findings[0].contains("VIOLATED"));
            assert!(findings[0].contains("line_2_3"));
        }
        other => panic!(
            "a breach is a finding, not a gate failure: the AC verification \
             remains the authority — got {other:?}"
        ),
    }
}

#[test]
fn a_loose_relaxation_is_also_a_recorded_finding() {
    // g_ij = +1e-2 > +1e-4: the relaxation is loose (not tight) —
    // exactness not demonstrated at that branch.
    let report = RelaxationResidualReport {
        records: vec![residual_record("line_2_3", 1.0e-2)],
        tolerance: 1.0e-4,
    };
    let assessment = assess_residuals(&report);
    assert_eq!(assessment.findings.len(), 1);
    assert_eq!(assessment.findings[0].kind, ResidualFindingKind::RelaxationLoose);
    assert!(assessment.findings[0].statement.contains("not demonstrated"));
}

// ---------------------------------------------------------------------------
// 5. Tier 1: no boundary, no residual record required
// ---------------------------------------------------------------------------

#[test]
fn a_tier_1_candidate_requires_no_boundary_and_no_record() {
    let conditions = check_relaxation_conditions(&model(), &radial_scope());
    let socp = gate_socp_boundary(&TierProvenance::Tier1Linearized, &conditions, None);
    assert_eq!(socp, SocpBoundaryGate::NotTier2);
}

// ---------------------------------------------------------------------------
// 6. The P3.18 cross-check dimension (D-P4.09-4)
// ---------------------------------------------------------------------------

#[test]
fn the_feasibility_verdict_dimension_carries_the_gate_fields() {
    // The gate's limit-check results feed the dimension: a violating
    // result produces the violated constraint names and the worst margin
    // fraction.
    let mut violating = converged_result();
    violating.branch_loadings[0].loading_fraction = 0.95;
    let limit_report = check_limits(&violating, &limits(), &generators(), &radial_scope());
    assert_eq!(limit_report.status, LimitCheckStatus::Violated);

    let dimension = feasibility_verdict_dimension(
        &TierProvenance::Tier2SocpRelaxation,
        "rejected",
        &limit_report.violated_constraint_names(),
        limit_report
            .diagnostics
            .binding_constraint
            .as_ref()
            .map(|binding| binding.margin_fraction),
        &[SOCP_EXACTNESS_CAVEAT.to_string()],
    );
    assert_eq!(dimension.tier, "tier_2_socp_relaxation");
    assert_eq!(dimension.gate_outcome_code, "rejected");
    assert_eq!(
        dimension.violated_constraints,
        vec!["branch_thermal:line_2_3"]
    );
    assert!(dimension.worst_margin_fraction.is_some());
    assert!(dimension.caveats.iter().any(|caveat| caveat == SOCP_EXACTNESS_CAVEAT));
    assert!(dimension.note.contains("D-P4.09-4"));
    assert!(dimension.note.contains("neither tier's objective or binding set is a feasibility verdict"));
}
