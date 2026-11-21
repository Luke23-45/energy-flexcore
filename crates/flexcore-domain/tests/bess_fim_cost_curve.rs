// FIM-F11.05 — Cost-curve pinned test suite (integration, from outside the
// crate).
//
// Every expected value in this file was computed OFFLINE with `python -I` in
// exact rational arithmetic (the pinned parameters, bin midpoints, and cycle
// counts are dyadic, so the f64 results are exact). The tests are PINNED
// EXPECTATIONS, never runs: no build and no test execution has happened
// under the FIM program — the first human build gates everything.
//
// Pinned here:
// - the cost-curve projection on the pinned rainflow histogram (LFP and
//   NMC): exact totals, linear-equivalent rate, and piecewise-linear
//   segments, with the PWL-total identity and convexity pinned;
// - the projection on the P15.07 ledger histogram (whole counted cycles)
//   with the same linear-equivalent rate (per-MWh rates are count-scale
//   invariant);
// - the DIVERGENCE from the P3.06 zero placeholder on the same scenario:
//   the placeholder evaluates 0.0 cost; the curve evaluates the pinned
//   nonzero total;
// - the UNVERIFIED-SECONDARY honesty flags and the P15-E-3 out-of-scope
//   refusal (unchanged discipline, enforced in the new module too);
// - the optimizer-input mirror (field-for-field by citation) and the
//   refusal family (empty histogram, zero/unknown usable energy).

use flexcore_contracts::capability::Provenance;
use flexcore_contracts::ids::FlexUuid;

use flexcore_domain::bess_cost_curve::{
    bess_degradation_cost_input, project_ledger_histogram, project_rainflow_histogram,
    BessCycleCostParameters, BessDegradationCurveShape, CostCurveError, CostCurveParameter,
    CostParameterStatus, DEGRADATION_COST_MAPPING_STATEMENT,
};
use flexcore_domain::bess_degradation::{DegradationChemistry, DodMeanSocHistogram};
use flexcore_domain::bess_model::{
    AuxLoadPoint, BessModel, BessModelConfig, BessSchedule, BessSetpoint, DegradationConfig,
    ProvenancedValue,
};
use flexcore_domain::ids::AssetId;
use flexcore_domain::rainflow::{count_soc_series, RainflowDodMeanSocHistogram};

/// The pinned SOC series (see `tests/bess_fim_rainflow.rs`): five half
/// cycles — (DOD 0.25, mean 0.375), (DOD 0.5, mean 0.5) x3, (DOD 0.25,
/// mean 0.625).
const PINNED_SOC_SERIES: [f64; 6] = [0.5, 0.25, 0.75, 0.25, 0.75, 0.5];

fn asset() -> AssetId {
    AssetId::new(FlexUuid::parse("0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a9f").unwrap())
}

fn rainflow_histogram() -> RainflowDodMeanSocHistogram {
    let mut histogram = RainflowDodMeanSocHistogram::new(
        vec![0.0, 0.25, 0.5, 0.75, 1.0],
        vec![0.0, 0.25, 0.5, 0.75, 1.0],
    )
    .unwrap();
    let cycles = count_soc_series(&PINNED_SOC_SERIES).unwrap();
    histogram.record_cycles(&cycles, Provenance::Measured).unwrap();
    histogram
}

fn ledger_histogram() -> DodMeanSocHistogram {
    // The same three cycle classes recorded through the P15.07 histogram as
    // THREE FULL cycles (each recorded cycle counts as one full cycle — the
    // ledger type's recorded semantics are unchanged).
    let mut histogram =
        DodMeanSocHistogram::new(vec![0.0, 0.25, 0.5, 0.75, 1.0], vec![0.0, 0.25, 0.5, 0.75, 1.0])
            .unwrap();
    histogram.record_cycle(0.25, 0.375, Provenance::Measured).unwrap();
    for _ in 0..3 {
        histogram.record_cycle(0.5, 0.5, Provenance::Measured).unwrap();
    }
    histogram.record_cycle(0.25, 0.625, Provenance::Measured).unwrap();
    histogram
}

fn usable() -> ProvenancedValue {
    ProvenancedValue::configured(10.0)
}

// --- parameter honesty ------------------------------------------------------

#[test]
fn the_planning_aid_defaults_are_unverified_secondary_by_type() {
    // The HUMAN LAB GATE, visible in the type: every recorded default is
    // UNVERIFIED-SECONDARY (no measured fleet exists; P15-E-3 stays OPEN).
    let lfp = BessCycleCostParameters::lfp_planning_aid_default().unwrap();
    assert!(lfp.is_unverified_secondary());
    assert_eq!(
        lfp.base_cost_per_full_equivalent_cycle.status,
        CostParameterStatus::UnverifiedSecondary
    );
    assert_eq!(lfp.dod_exponent.status, CostParameterStatus::UnverifiedSecondary);
    assert_eq!(
        lfp.mean_soc_dwell_coefficient.status,
        CostParameterStatus::UnverifiedSecondary
    );
    assert_eq!(lfp.base_cost_per_full_equivalent_cycle.provenance, Provenance::Configured);
    let nmc = BessCycleCostParameters::nmc_planning_aid_default().unwrap();
    assert!(nmc.is_unverified_secondary());
    // A caller-supplied recorded-secondary table is legal and flagged false.
    let recorded = BessCycleCostParameters::new(
        DegradationChemistry::Lfp,
        CostCurveParameter::new(
            20.0,
            "a recorded source class".to_string(),
            CostParameterStatus::RecordedSecondary,
            Provenance::Configured,
        )
        .unwrap(),
        CostCurveParameter::new(
            2.0,
            "a recorded source class".to_string(),
            CostParameterStatus::RecordedSecondary,
            Provenance::Configured,
        )
        .unwrap(),
        CostCurveParameter::new(
            0.5,
            "a recorded source class".to_string(),
            CostParameterStatus::RecordedSecondary,
            Provenance::Configured,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(!recorded.is_unverified_secondary());
}

#[test]
fn the_out_of_scope_chemistry_refusal_is_unchanged_and_enforced_here() {
    // The P15-E-3 discipline is unchanged: out-of-scope chemistries are
    // recorded limitations, and the new module refuses them wherever
    // parameters would be credited (never a silent extrapolation).
    let chemistry = DegradationChemistry::OutOfScope { name: "LTO".to_string() };
    assert!(!chemistry.is_in_scope());
    assert_eq!(chemistry.label(), "OUT_OF_SCOPE(LTO)");
    let error = BessCycleCostParameters::new(
        chemistry,
        parameter(20.0),
        parameter(2.0),
        parameter(0.5),
    );
    assert!(matches!(
        error,
        Err(CostCurveError::OutOfScopeChemistry { label }) if label == "OUT_OF_SCOPE(LTO)"
    ));
}

fn parameter(value: f64) -> CostCurveParameter {
    CostCurveParameter::new(
        value,
        "a declared source class".to_string(),
        CostParameterStatus::RecordedSecondary,
        Provenance::Configured,
    )
    .unwrap()
}

// --- the pinned projections ---------------------------------------------------

/// The LFP projection over the pinned rainflow histogram (usable energy
/// 10 MWh). Offline exact computation (all dyadic):
///   bin (1,1): 0.5 cycles, cost/cycle 3.33984375, throughput/cycle 7.5
///   bin (2,2): 1.5 cycles, cost/cycle 10.25390625, throughput/cycle 12.5
///   bin (1,2): 0.5 cycles, cost/cycle 3.69140625, throughput/cycle 7.5
///   total cost 18.896484375, total throughput 26.25 MWh
///   linear-equivalent 0.7198660714285714 currency-units/MWh
///   PWL segments: (3.75, 0.4453125), (7.5, 0.4921875), (26.25, 0.8203125)
#[test]
fn the_lfp_projection_on_the_rainflow_histogram_is_pinned() {
    let parameters = BessCycleCostParameters::lfp_planning_aid_default().unwrap();
    let projection =
        project_rainflow_histogram(&parameters, &usable(), &rainflow_histogram()).unwrap();
    assert_pinned_lfp_projection(&projection);
    // The PWL total cost at full consumption equals the histogram total
    // cost exactly (the recorded identity).
    let mut previous_limit = 0.0;
    let mut pwl_total = 0.0;
    for segment in &projection.piecewise_segments {
        pwl_total += (segment.cumulative_throughput_limit_mwh - previous_limit)
            * segment.marginal_cost_per_mwh;
        previous_limit = segment.cumulative_throughput_limit_mwh;
    }
    assert!((pwl_total - projection.total_cost).abs() < 1e-12);
    // Convex by construction: marginal costs never decrease.
    for pair in projection.piecewise_segments.windows(2) {
        assert!(pair[0].marginal_cost_per_mwh <= pair[1].marginal_cost_per_mwh);
    }
    // Provenance: Measured histogram + Configured usable energy and
    // parameters -> the weakest is Configured.
    assert_eq!(projection.provenance, Provenance::Configured);
    assert!(projection.parameters_unverified_secondary);
    assert!(projection.provenance_note.contains("UNVERIFIED-SECONDARY"));
    assert!(projection.provenance_note.len() <= 256);
    assert_eq!(projection.statement, DEGRADATION_COST_MAPPING_STATEMENT);
}

#[test]
fn the_nmc_projection_diverges_from_lfp_and_is_pinned() {
    let parameters = BessCycleCostParameters::nmc_planning_aid_default().unwrap();
    let projection =
        project_rainflow_histogram(&parameters, &usable(), &rainflow_histogram()).unwrap();
    // Offline exact computation (NMC: base 25, dwell coefficient 0.8, over
    // the rainflow counts 0.5 / 1.5 / 0.5 cycles): total cost 26.89453125,
    // linear-equivalent 1.0245535714285714/MWh — a different chemistry's
    // table prices the same measured histogram differently.
    assert!((projection.total_cost - 26.89453125).abs() < 1e-12);
    assert!((projection.linear_equivalent_cost_per_mwh - 1.0245535714285714).abs() < 1e-15);
    assert_eq!(projection.curve_version, "fim-f11-nmc-planning-aid-v0-unverified");
}

fn assert_pinned_lfp_projection(projection: &flexcore_domain::bess_cost_curve::DegradationCostProjection) {
    assert!((projection.total_cost - 18.896484375).abs() < 1e-12);
    assert!((projection.total_throughput_mwh - 26.25).abs() < 1e-12);
    assert!((projection.linear_equivalent_cost_per_mwh - 0.7198660714285714).abs() < 1e-15);
    assert_eq!(projection.piecewise_segments.len(), 3);
    assert!((projection.piecewise_segments[0].cumulative_throughput_limit_mwh - 3.75).abs() < 1e-12);
    assert!((projection.piecewise_segments[0].marginal_cost_per_mwh - 0.4453125).abs() < 1e-15);
    assert!((projection.piecewise_segments[1].cumulative_throughput_limit_mwh - 7.5).abs() < 1e-12);
    assert!((projection.piecewise_segments[1].marginal_cost_per_mwh - 0.4921875).abs() < 1e-15);
    assert!((projection.piecewise_segments[2].cumulative_throughput_limit_mwh - 26.25).abs() < 1e-12);
    assert!((projection.piecewise_segments[2].marginal_cost_per_mwh - 0.8203125).abs() < 1e-15);
    assert_eq!(projection.curve_version, "fim-f11-lfp-planning-aid-v0-unverified");
    assert_eq!(projection.chemistry, DegradationChemistry::Lfp);
}

/// The LFP projection over the pinned LEDGER histogram (the same three
/// cycle classes recorded as THREE FULL cycles instead of five halves):
/// offline exact computation — total cost 37.79296875, total throughput
/// 52.5 MWh, linear-equivalent 0.7198660714285714 (the SAME per-MWh rate:
/// per-MWh rates are count-scale invariant), PWL segments
/// (7.5, 0.4453125), (15.0, 0.4921875), (52.5, 0.8203125).
#[test]
fn the_ledger_histogram_projection_is_pinned_with_the_same_rate() {
    let parameters = BessCycleCostParameters::lfp_planning_aid_default().unwrap();
    let projection = project_ledger_histogram(&parameters, &usable(), &ledger_histogram()).unwrap();
    assert!((projection.total_cost - 37.79296875).abs() < 1e-12);
    assert!((projection.total_throughput_mwh - 52.5).abs() < 1e-12);
    assert!((projection.linear_equivalent_cost_per_mwh - 0.7198660714285714).abs() < 1e-15);
    assert_eq!(projection.piecewise_segments.len(), 3);
    assert!((projection.piecewise_segments[0].cumulative_throughput_limit_mwh - 7.5).abs() < 1e-12);
    assert!((projection.piecewise_segments[2].cumulative_throughput_limit_mwh - 52.5).abs() < 1e-12);
    assert!((projection.piecewise_segments[2].marginal_cost_per_mwh - 0.8203125).abs() < 1e-15);
    assert_eq!(projection.provenance, Provenance::Configured);
}

// --- the divergence from the zero placeholder -----------------------------------

#[test]
fn the_curve_diverges_from_the_zero_placeholder_on_the_pinned_scenario() {
    // The P3.06 placeholder: a configurable LINEAR throughput term, default
    // ZERO. On the pinned scenario (52.5 MWh of throughput — the ledger-
    // histogram totals), the placeholder evaluates exactly 0.0 cost; the
    // measured-data curve evaluates the pinned nonzero total. This is the
    // recorded acceptance criterion: dispatch stops pricing battery wear at
    // zero.
    let model = BessModel::new(BessModelConfig {
        e_min_mwh: ProvenancedValue::configured(0.0),
        e_max_mwh: ProvenancedValue::configured(10.0),
        p_ch_max_mw: ProvenancedValue::configured(10.0),
        p_dis_max_mw: ProvenancedValue::configured(10.0),
        eta_ch: 1.0,
        eta_dis: 1.0,
        interval_seconds: 3_600,
        soc_power_bands: None,
        temperature_derate: None,
        transitions: None,
        reserve: None,
        degradation: Some(DegradationConfig {
            linear_cost_per_mwh_throughput: 0.0, // the recorded default
            provenance: Provenance::Configured,
        }),
    })
    .unwrap();
    // A feasible scenario with exactly 52.5 MWh of throughput over the
    // 10 MWh window (eta 1.0, delta-t 1 h): two full round trips
    // (10 + 10 + 10 + 10 MWh) plus a half round trip (6.25 + 6.25 MWh).
    let setpoints = vec![
        BessSetpoint::charge(10.0),
        BessSetpoint::discharge(10.0),
        BessSetpoint::charge(10.0),
        BessSetpoint::discharge(10.0),
        BessSetpoint::charge(6.25),
        BessSetpoint::discharge(6.25),
    ];
    let aux = vec![AuxLoadPoint::Unknown; setpoints.len()];
    let schedule = BessSchedule {
        initial_soc: 0.0,
        setpoints,
        auxiliary_load: aux,
    };
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    assert!(evaluation.violations.is_empty(), "the scenario must be feasible");
    let throughput = evaluation.degradation.throughput_mwh;
    assert!((throughput - 52.5).abs() < 1e-9, "pinned throughput, got {throughput}");
    assert_eq!(evaluation.degradation.cost, 0.0, "the placeholder default is zero");
    assert!(evaluation.degradation.is_placeholder);

    let parameters = BessCycleCostParameters::lfp_planning_aid_default().unwrap();
    let projection = project_ledger_histogram(&parameters, &usable(), &ledger_histogram()).unwrap();
    let curve_cost = throughput * projection.linear_equivalent_cost_per_mwh;
    // Offline exact: 52.5 * 0.7198660714285714 = 37.79296875 (exact in f64).
    assert!((curve_cost - 37.79296875).abs() < 1e-9);
    assert!(curve_cost > evaluation.degradation.cost, "the divergence is pinned");
}

// --- the optimizer-input mirror -------------------------------------------------

#[test]
fn the_optimizer_input_mirror_carries_the_pinned_shape() {
    let parameters = BessCycleCostParameters::lfp_planning_aid_default().unwrap();
    let projection = project_ledger_histogram(&parameters, &usable(), &ledger_histogram()).unwrap();
    let linear =
        bess_degradation_cost_input(asset(), &projection, CurveShapeChoice::LinearEquivalent, None)
            .unwrap();
    match &linear.curve {
        BessDegradationCurveShape::Linear { cost_per_mwh_throughput } => {
            assert!((*cost_per_mwh_throughput - 0.7198660714285714).abs() < 1e-15);
        }
        other => panic!("expected the linear shape, got {other:?}"),
    }
    assert!(linear.provenance_note.len() <= 256);
    assert!(linear.parameters_unverified_secondary);
    assert_eq!(linear.curve_version, "fim-f11-lfp-planning-aid-v0-unverified");
    let piecewise = bess_degradation_cost_input(
        asset(),
        &projection,
        CurveShapeChoice::PiecewiseLinear,
        None,
    )
    .unwrap();
    match &piecewise.curve {
        BessDegradationCurveShape::PiecewiseLinear { segments } => {
            assert_eq!(segments.len(), 3);
        }
        other => panic!("expected the piecewise-linear shape, got {other:?}"),
    }
    // An override note must satisfy the optimizer contract's bounds
    // (non-empty, <= 256 characters) so the mapping into a
    // DegradationCostDeclaration cannot fail downstream.
    assert!(matches!(
        bess_degradation_cost_input(
            asset(),
            &projection,
            CurveShapeChoice::LinearEquivalent,
            Some("   ".to_string()),
        ),
        Err(CostCurveError::EmptyField { field: "provenance_note" })
    ));
}

// --- the refusal family ------------------------------------------------------------

#[test]
fn an_empty_histogram_is_refused_and_no_zero_rate_is_invented() {
    let parameters = BessCycleCostParameters::lfp_planning_aid_default().unwrap();
    let empty = RainflowDodMeanSocHistogram::new(vec![0.0, 1.0], vec![0.0, 1.0]).unwrap();
    assert!(matches!(
        project_rainflow_histogram(&parameters, &usable(), &empty),
        Err(CostCurveError::EmptyHistogram)
    ));
}

#[test]
fn unknown_or_degenerate_usable_energy_is_refused() {
    let parameters = BessCycleCostParameters::lfp_planning_aid_default().unwrap();
    // UNKNOWN-provenance usable energy: no MWh normalization is credited
    // from unknown data (P3.02 — unknown narrows, never widens).
    assert!(matches!(
        project_ledger_histogram(&parameters, &ProvenancedValue::unknown(), &ledger_histogram()),
        Err(CostCurveError::UsableEnergyNotCreditable)
    ));
    assert!(matches!(
        project_ledger_histogram(
            &parameters,
            &ProvenancedValue::configured(0.0),
            &ledger_histogram()
        ),
        Err(CostCurveError::NonPositiveValue { field: "usable_energy_mwh", .. })
    ));
}
