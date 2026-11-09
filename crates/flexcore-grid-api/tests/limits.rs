// P4.06 — Step-5 limit-check integration tests (ledger P4.06 acceptance;
// report §12.5 step 5). Decision record:
// ledger/phase-04-grid-feasibility-dispatch/06_09_gate_completion.md
// (D-P4.06-1..3).
//
// Covered here:
// 1. Per-limit-category single-violation fixtures (low voltage, high
//    voltage, branch overload, transformer overload, generator S_max) —
//    each fails on EXACTLY that named constraint with the hand-computed
//    margin (value, limit, margin, margin fraction).
// 2. A passing fixture passes ALL limits with positive margins.
// 3. Margin arithmetic (hand-computed, exact to 1e-12) and the §12.5
//    step 7 diagnostics (worst margin per category, binding constraint by
//    smallest margin fraction, per-element loading percentages).
// 4. Multi-violation reporting: a fixture violating three limits at once
//    names EVERY violated constraint.
// 5. Boundary-inclusive limit handling (value == limit PASSES with margin
//    0; epsilon beyond fails) — decided once, D-P4.06-3, tested both
//    sides.
// 6. Fail-closed shapes: missing generator capability, non-convergence,
//    invalid limit configuration — never a within-limits report
//    (§17.5 invariant 9).
// 7. Scope-declared limit gaps are recorded, never silently skipped.

//! The fixtures hand-compute the loadings through the recorded model
//! bases: `line_2_3` is rated 500 A (0.38 kA = loading 0.76) and
//! `trafo_1_2` is rated 0.4 MVA (0.25 MVA = loading 0.625) — the same
//! bases the P4.02 contract fixtures use.

use flexcore_grid_api::fail_closed::FailClosedTrigger;
use flexcore_grid_api::limits::{
    check_limits, is_hard_violation, GeneratorLimit, GeneratorLimitSet, LimitCategory,
    LimitCheckStatus, BOUNDARY_CONVENTION, HARD_LIMIT_STATEMENT, LIMIT_SOURCE_STATEMENT,
    MARGIN_STATEMENT, PHYSICAL_RATING_FRACTION,
};
use flexcore_grid_api::port::{
    AppliedInjectionRecord, BranchLoading, BusVoltage, CoverageLimitation, Diagnostic,
    GridModelRef, NetworkLimits, PowerFlowResult, PowerFlowResiduals, PowerFlowStatus,
    TransformerLoading,
};
use flexcore_grid_api::scope::{
    NotModelled, PhaseScope, ScopeDeclaration, TopologyScope,
};
use flexcore_contracts::units::ReactiveSignConvention;
use std::time::Duration;

const EPS: f64 = 1.0e-12;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn model_ref() -> GridModelRef {
    GridModelRef {
        model_id: "feeder-limits-test".to_string(),
        version: "1".to_string(),
        content_hash: "hash-limits-test".to_string(),
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

/// The declared generator/inverter apparent-power limits for the fixture
/// model: bus_3 hosts the candidate injection with S_max = 4.0 MVA.
fn generators() -> GeneratorLimitSet {
    GeneratorLimitSet {
        limits: vec![GeneratorLimit {
            bus_id: "bus_3".to_string(),
            s_max_mva: 4.0,
            source_note: "fixture capability model cap-1 (§11-derived)".to_string(),
        }],
    }
}

fn scope() -> ScopeDeclaration {
    ScopeDeclaration::new(
        "limits-test-evaluator".to_string(),
        PhaseScope::Balanced,
        TopologyScope::RadialOnly,
        vec![
            flexcore_grid_api::scope::EquipmentKind::Line,
            flexcore_grid_api::scope::EquipmentKind::TwoWindingTransformer,
            flexcore_grid_api::scope::EquipmentKind::ExternalGrid,
        ],
        vec![
            NotModelled::new(
                "unbalanced_three_phase".to_string(),
                "the evaluator does not model unbalanced three-phase asymmetry".to_string(),
            )
            .expect("valid"),
            NotModelled::new(
                "ac_opf".to_string(),
                "the evaluator runs an AC power-flow evaluation; it is NOT an \
                 AC-OPF optimizer"
                    .to_string(),
            )
            .expect("valid"),
        ],
        "runs an AC power-flow evaluation of specified injections and controls; \
         it is NOT an AC-OPF optimizer"
            .to_string(),
        vec!["P4.06 limit-check test scope".to_string()],
    )
    .expect("valid scope")
}

/// Builds a converged power-flow result from hand-computed values:
/// `(bus, vm_pu)` voltages, `(branch, loading_fraction)` loadings,
/// `(trafo, loading_fraction)` loadings, and `(id, bus, P MW, Q MVAr)`
/// applied injections.
fn converged_result(
    voltages: &[(&str, f64)],
    branches: &[(&str, f64)],
    trafos: &[(&str, f64)],
    injections: &[(&str, &str, f64, f64)],
) -> PowerFlowResult {
    let kv = |bus: &str, vm_pu: f64| -> f64 {
        // The per-unit base is the bus nominal voltage (20 kV slack,
        // 0.4 kV elsewhere) — the translation layer's tested conversion.
        let nominal = if bus == "bus_1" { 20.0 } else { 0.4 };
        vm_pu * nominal
    };
    PowerFlowResult {
        status: PowerFlowStatus::Converged,
        model_ref: model_ref(),
        provider_id: "test-provider".to_string(),
        provider_version: "3.5.6".to_string(),
        bus_voltages: voltages
            .iter()
            .map(|(bus, vm_pu)| BusVoltage {
                bus_id: (*bus).to_string(),
                vm_pu: *vm_pu,
                vm_kv: kv(bus, *vm_pu),
            })
            .collect(),
        branch_loadings: branches
            .iter()
            .map(|(id, fraction)| BranchLoading {
                branch_id: (*id).to_string(),
                // 500 A rating: 0.76 of it is 380 A (hand-computed).
                current_from_a: fraction * 500.0,
                current_to_a: fraction * 500.0 - 0.5,
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
            provider_status_text: "converged in 4 iterations".to_string(),
        }),
        coverage_limitations: vec![CoverageLimitation {
            label: "unbalanced_three_phase".to_string(),
            statement: "not modelled (test scope)".to_string(),
        }],
        warnings: Vec::new(),
        runtime: Duration::from_millis(1),
    }
}

fn non_converged_result() -> PowerFlowResult {
    let mut result = converged_result(
        &[("bus_1", 1.02), ("bus_2", 1.01), ("bus_3", 0.985)],
        &[("line_2_3", 0.76)],
        &[("trafo_1_2", 0.625)],
        &[("inj-1", "bus_3", 2.5, 0.5)],
    );
    result.status = PowerFlowStatus::DidNotConverge;
    result.residuals = Diagnostic::NotAvailable {
        reason: "no converged solution; no residuals exist".to_string(),
    };
    result
}

/// The passing fixture: all limits within their bands (the P4.02
/// contract-fixture magnitudes).
fn passing_result() -> PowerFlowResult {
    converged_result(
        &[("bus_1", 1.02), ("bus_2", 1.01), ("bus_3", 0.985)],
        &[("line_2_3", 0.76)],
        &[("trafo_1_2", 0.625)],
        &[("inj-1", "bus_3", 2.5, 0.5)],
    )
}

fn check(result: &PowerFlowResult) -> flexcore_grid_api::limits::LimitCheckReport {
    check_limits(result, &limits(), &generators(), &scope())
}

// ---------------------------------------------------------------------------
// The passing fixture passes ALL limits
// ---------------------------------------------------------------------------

#[test]
fn the_passing_fixture_passes_every_limit_with_positive_margins() {
    let report = check(&passing_result());
    assert_eq!(report.status, LimitCheckStatus::WithinLimits);
    assert!(report.violations.is_empty());
    // 3 buses × 2 voltage checks + 1 branch + 1 trafo + 1 generator.
    assert_eq!(report.checks.len(), 9);

    // Hand-computed margins (value − limit for the lower band; limit −
    // value for the upper/loading bands).
    let bus_3_lower = report
        .checks
        .iter()
        .find(|c| c.category == LimitCategory::BusVoltageLower && c.element_id == "bus_3")
        .expect("bus_3 lower check");
    assert!((bus_3_lower.measured - 0.985).abs() < EPS);
    assert!((bus_3_lower.margin - 0.085).abs() < EPS);
    assert!((bus_3_lower.margin_fraction.unwrap() - 0.085 / 0.9).abs() < EPS);

    let bus_1_upper = report
        .checks
        .iter()
        .find(|c| c.category == LimitCategory::BusVoltageUpper && c.element_id == "bus_1")
        .expect("bus_1 upper check");
    assert!((bus_1_upper.margin - 0.08).abs() < EPS);

    let branch = report
        .checks
        .iter()
        .find(|c| c.category == LimitCategory::BranchThermal)
        .expect("branch check");
    assert!((branch.margin - 0.14).abs() < EPS);
    assert!((branch.margin_fraction.unwrap() - 0.14 / 0.9).abs() < EPS);
    assert!((branch.measured - 0.76).abs() < EPS);

    let trafo = report
        .checks
        .iter()
        .find(|c| c.category == LimitCategory::TransformerLoading)
        .expect("trafo check");
    assert!((trafo.margin - 0.275).abs() < EPS);

    // Generator: P = 2.5 MW, Q = 0.5 MVAr → S = sqrt(6.5) MVA vs 4.0.
    let generator = report
        .checks
        .iter()
        .find(|c| c.category == LimitCategory::GeneratorApparentPower)
        .expect("generator check");
    let expected_s = (2.5_f64 * 2.5 + 0.5_f64 * 0.5).sqrt();
    assert!((generator.measured - expected_s).abs() < EPS);
    assert!((generator.margin - (4.0 - expected_s)).abs() < EPS);
    assert!(generator.passed);
}

// ---------------------------------------------------------------------------
// Single-violation fixtures: each fails on EXACTLY that named constraint
// ---------------------------------------------------------------------------

#[test]
fn a_low_voltage_fixture_fails_exactly_the_lower_band_with_its_margin() {
    let result = converged_result(
        &[("bus_1", 1.02), ("bus_2", 1.01), ("bus_3", 0.88)],
        &[("line_2_3", 0.76)],
        &[("trafo_1_2", 0.625)],
        &[("inj-1", "bus_3", 2.5, 0.5)],
    );
    let report = check(&result);
    assert_eq!(report.status, LimitCheckStatus::Violated);
    assert_eq!(report.violated_constraint_names(), vec!["bus_voltage_lower:bus_3"]);
    let violation = &report.violations[0];
    assert!((violation.measured - 0.88).abs() < EPS);
    assert!((violation.limit - 0.9).abs() < EPS);
    // margin = measured − limit for the AtLeast direction.
    assert!((violation.margin - (-0.02)).abs() < EPS);
    assert!((violation.margin_fraction.unwrap() - (-0.02 / 0.9)).abs() < EPS);
    assert!(!is_hard_violation(violation));
}

#[test]
fn a_high_voltage_fixture_fails_exactly_the_upper_band_with_its_margin() {
    let result = converged_result(
        &[("bus_1", 1.02), ("bus_2", 1.12), ("bus_3", 0.985)],
        &[("line_2_3", 0.76)],
        &[("trafo_1_2", 0.625)],
        &[("inj-1", "bus_3", 2.5, 0.5)],
    );
    let report = check(&result);
    assert_eq!(report.status, LimitCheckStatus::Violated);
    assert_eq!(report.violated_constraint_names(), vec!["bus_voltage_upper:bus_2"]);
    let violation = &report.violations[0];
    // margin = limit − measured for the AtMost direction.
    assert!((violation.margin - (-0.02)).abs() < EPS);
    assert!((violation.margin_fraction.unwrap() - (-0.02 / 1.1)).abs() < EPS);
}

#[test]
fn a_branch_overload_fixture_fails_exactly_the_branch_thermal_limit() {
    let result = converged_result(
        &[("bus_1", 1.02), ("bus_2", 1.01), ("bus_3", 0.985)],
        &[("line_2_3", 0.95)],
        &[("trafo_1_2", 0.625)],
        &[("inj-1", "bus_3", 2.5, 0.5)],
    );
    let report = check(&result);
    assert_eq!(report.status, LimitCheckStatus::Violated);
    assert_eq!(report.violated_constraint_names(), vec!["branch_thermal:line_2_3"]);
    let violation = &report.violations[0];
    assert!((violation.margin - (-0.05)).abs() < EPS);
    assert!((violation.margin_fraction.unwrap() - (-0.05 / 0.9)).abs() < EPS);
    // 0.95 is above the operational limit but below the physical rating.
    assert!(!is_hard_violation(violation));
}

#[test]
fn a_transformer_overload_fixture_fails_exactly_the_transformer_limit() {
    let result = converged_result(
        &[("bus_1", 1.02), ("bus_2", 1.01), ("bus_3", 0.985)],
        &[("line_2_3", 0.76)],
        &[("trafo_1_2", 0.93)],
        &[("inj-1", "bus_3", 2.5, 0.5)],
    );
    let report = check(&result);
    assert_eq!(report.status, LimitCheckStatus::Violated);
    assert_eq!(
        report.violated_constraint_names(),
        vec!["transformer_loading:trafo_1_2"]
    );
    assert!((report.violations[0].margin - (-0.03)).abs() < EPS);
}

#[test]
fn a_generator_s_max_fixture_fails_exactly_the_apparent_power_limit() {
    // P = 4.0 MW, Q = 3.0 MVAr → S = 5.0 MVA vs S_max 4.0 MVA.
    let result = converged_result(
        &[("bus_1", 1.02), ("bus_2", 1.01), ("bus_3", 0.985)],
        &[("line_2_3", 0.76)],
        &[("trafo_1_2", 0.625)],
        &[("inj-1", "bus_3", 4.0, 3.0)],
    );
    let report = check(&result);
    assert_eq!(report.status, LimitCheckStatus::Violated);
    assert_eq!(
        report.violated_constraint_names(),
        vec!["generator_apparent_power:bus_3"]
    );
    let violation = &report.violations[0];
    assert!((violation.measured - 5.0).abs() < EPS);
    assert!((violation.limit - 4.0).abs() < EPS);
    assert!((violation.margin - (-1.0)).abs() < EPS);
    assert!((violation.margin_fraction.unwrap() - (-0.25)).abs() < EPS);
    // A capability violation is never a "hard" loading violation.
    assert!(!is_hard_violation(violation));
}

// ---------------------------------------------------------------------------
// Multi-violation reporting: EVERY violated constraint is named
// ---------------------------------------------------------------------------

#[test]
fn a_multi_violation_fixture_names_every_violated_constraint() {
    let result = converged_result(
        &[("bus_1", 1.02), ("bus_2", 1.01), ("bus_3", 0.88)],
        &[("line_2_3", 0.95)],
        &[("trafo_1_2", 0.93)],
        &[("inj-1", "bus_3", 4.0, 3.0)],
    );
    let report = check(&result);
    assert_eq!(report.status, LimitCheckStatus::Violated);
    let names = report.violated_constraint_names();
    assert_eq!(names.len(), 4);
    assert!(names.contains(&"bus_voltage_lower:bus_3".to_string()));
    assert!(names.contains(&"branch_thermal:line_2_3".to_string()));
    assert!(names.contains(&"transformer_loading:trafo_1_2".to_string()));
    assert!(names.contains(&"generator_apparent_power:bus_3".to_string()));
    // Every violation is also a full check record with margins.
    assert!(report
        .violations
        .iter()
        .all(|violation| violation.margin < 0.0 && !violation.passed));
}

// ---------------------------------------------------------------------------
// Boundary-inclusive handling: value == limit PASSES (decided once)
// ---------------------------------------------------------------------------

#[test]
fn the_boundary_is_inclusive_on_both_sides_of_every_category() {
    assert!(!BOUNDARY_CONVENTION.trim().is_empty());
    // Everything exactly AT its limit.
    let at_limit = converged_result(
        &[("bus_1", 1.1), ("bus_2", 0.9), ("bus_3", 0.985)],
        &[("line_2_3", 0.9)],
        &[("trafo_1_2", 0.9)],
        &[("inj-1", "bus_3", 4.0, 0.0)], // S = 4.0 == S_max
    );
    let report = check(&at_limit);
    assert_eq!(report.status, LimitCheckStatus::WithinLimits);
    assert!(report.violations.is_empty());
    // Each check AT its boundary carries margin exactly 0 and passes;
    // the remaining checks (bus_3, off-boundary) pass with positive
    // margins — the overall status is within-limits either way.
    let boundary_names = [
        "bus_voltage_upper:bus_1",
        "bus_voltage_lower:bus_2",
        "branch_thermal:line_2_3",
        "transformer_loading:trafo_1_2",
        "generator_apparent_power:bus_3",
    ];
    for name in boundary_names {
        let check_record = report
            .checks
            .iter()
            .find(|c| c.constraint_name() == name)
            .unwrap_or_else(|| panic!("boundary check {name} missing"));
        assert!(
            check_record.margin.abs() < EPS,
            "boundary check {name} must carry margin 0, got {}",
            check_record.margin
        );
        assert!(check_record.passed);
    }

    // One epsilon BEYOND each limit fails that exact category.
    let beyond = converged_result(
        &[("bus_1", 1.1 + 1.0e-9), ("bus_2", 0.9 - 1.0e-9), ("bus_3", 0.985)],
        &[("line_2_3", 0.9 + 1.0e-9)],
        &[("trafo_1_2", 0.9)],
        &[("inj-1", "bus_3", 4.0 + 1.0e-9, 0.0)],
    );
    let report = check(&beyond);
    assert_eq!(report.status, LimitCheckStatus::Violated);
    let names = report.violated_constraint_names();
    assert!(names.contains(&"bus_voltage_upper:bus_1".to_string()));
    assert!(names.contains(&"bus_voltage_lower:bus_2".to_string()));
    assert!(names.contains(&"branch_thermal:line_2_3".to_string()));
    assert!(names.contains(&"generator_apparent_power:bus_3".to_string()));
}

// ---------------------------------------------------------------------------
// Fail-closed shapes: never a within-limits report
// ---------------------------------------------------------------------------

#[test]
fn a_missing_generator_declaration_is_fail_closed() {
    let result = passing_result();
    let empty_generators = GeneratorLimitSet { limits: Vec::new() };
    let report = check_limits(&result, &limits(), &empty_generators, &scope());
    match report.status {
        LimitCheckStatus::FailClosed { trigger, reason } => {
            assert_eq!(trigger, FailClosedTrigger::CapabilityDataMissing);
            assert!(!reason.is_empty());
            assert!(reason.contains("bus_3"));
        }
        other => panic!("missing capability must be fail-closed, got {other:?}"),
    }
    assert!(report.violations.is_empty());
    // The checks recorded before the failure are preserved as evidence.
    assert!(!report.checks.is_empty());
}

#[test]
fn a_non_converged_result_is_inconclusive_never_a_pass() {
    let report = check(&non_converged_result());
    match report.status {
        LimitCheckStatus::Inconclusive { reason } => {
            assert!(!reason.is_empty());
            assert!(reason.contains("never a pass"));
        }
        other => panic!("non-convergence must be inconclusive, got {other:?}"),
    }
    assert!(report.checks.is_empty());
    assert!(report.violations.is_empty());
}

#[test]
fn an_invalid_limit_configuration_is_fail_closed() {
    let invalid = NetworkLimits {
        voltage_min_pu: 1.1,
        voltage_max_pu: 0.9,
        max_branch_loading_fraction: 0.9,
        max_transformer_loading_fraction: 0.9,
    };
    let report = check_limits(&passing_result(), &invalid, &generators(), &scope());
    match report.status {
        LimitCheckStatus::FailClosed { trigger, .. } => {
            assert_eq!(trigger, FailClosedTrigger::ValidatorFailed);
        }
        other => panic!("invalid configuration must be fail-closed, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Diagnostics (§12.5 step 7): worst margins, binding constraint, loadings
// ---------------------------------------------------------------------------

#[test]
fn the_diagnostics_carry_worst_margins_binding_constraint_and_loadings() {
    let report = check(&passing_result());
    let diagnostics = &report.diagnostics;

    // Per-element loading percentages (hand-computed: 0.76 → 76.0,
    // 0.625 → 62.5).
    assert_eq!(diagnostics.per_element_loading_percent.len(), 2);
    assert!((diagnostics.per_element_loading_percent[0].loading_percent - 76.0).abs() < EPS);
    assert!((diagnostics.per_element_loading_percent[1].loading_percent - 62.5).abs() < EPS);

    // Worst margin per category: all five categories are present.
    assert_eq!(diagnostics.worst_margin_by_category.len(), 5);
    let lower_worst = diagnostics
        .worst_margin_by_category
        .iter()
        .find(|w| w.category == LimitCategory::BusVoltageLower)
        .expect("lower category");
    assert_eq!(lower_worst.element_id, "bus_3");
    assert!((lower_worst.margin - 0.085).abs() < EPS);

    // The binding constraint is the smallest margin FRACTION across all
    // categories: bus_1 upper = (1.1 − 1.02)/1.1 ≈ 0.0727 beats
    // bus_3 lower = 0.085/0.9 ≈ 0.0944 and the loadings.
    let binding = diagnostics.binding_constraint.as_ref().expect("binding");
    assert_eq!(binding.category, LimitCategory::BusVoltageUpper);
    assert_eq!(binding.element_id, "bus_1");
    assert!((binding.margin_fraction - (1.1 - 1.02) / 1.1).abs() < EPS);
    assert!(binding.statement.contains("bus_voltage_upper:bus_1"));
}

// ---------------------------------------------------------------------------
// Scope-declared limit gaps are recorded, never silently skipped
// ---------------------------------------------------------------------------

#[test]
fn scope_declared_limit_gaps_are_recorded_on_the_report() {
    assert!(!LIMIT_SOURCE_STATEMENT.trim().is_empty());
    assert!(!MARGIN_STATEMENT.trim().is_empty());
    assert!(!HARD_LIMIT_STATEMENT.trim().is_empty());
    assert!((PHYSICAL_RATING_FRACTION - 1.0).abs() < EPS);

    let report = check(&passing_result());
    assert_eq!(report.status, LimitCheckStatus::WithinLimits);
    // Even a within-limits report records the scope's unverifiable limit
    // classes — never silently skipped (§12.5 step 6).
    assert_eq!(report.scope_limit_gaps.len(), 2);
    assert!(report
        .scope_limit_gaps
        .iter()
        .any(|gap| gap.label == "unbalanced_three_phase"));
    assert!(report
        .scope_limit_gaps
        .iter()
        .all(|gap| gap.statement.contains("cannot be verified")));
}

// ---------------------------------------------------------------------------
// Hard violations (the D-P4.07-1 routing input)
// ---------------------------------------------------------------------------

#[test]
fn a_loading_above_the_physical_rating_is_a_hard_violation() {
    let result = converged_result(
        &[("bus_1", 1.02), ("bus_2", 1.01), ("bus_3", 0.985)],
        &[("line_2_3", 1.05)],
        &[("trafo_1_2", 0.625)],
        &[("inj-1", "bus_3", 2.5, 0.5)],
    );
    let report = check(&result);
    assert_eq!(report.status, LimitCheckStatus::Violated);
    assert_eq!(report.violated_constraint_names(), vec!["branch_thermal:line_2_3"]);
    assert!(is_hard_violation(&report.violations[0]));
}
