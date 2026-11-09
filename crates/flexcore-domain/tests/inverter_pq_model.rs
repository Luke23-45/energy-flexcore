// P3.08 — Inverter P-Q envelope test suite (integration, from outside the
// crate).
//
// Covers the ledger acceptance criteria end to end, from OUTSIDE
// flexcore-domain:
//
// - The basic apparent-power constraint P² + Q² ≤ S_max² holds in every test
//   output and is boundary-inclusive (a point exactly on the circle is
//   feasible). It is documented as "necessary but not complete" (§11.3).
// - Bidirectional (negative-P) cases pass; charge AND discharge modes are
//   both represented by their own per-mode limit tables (configuration).
// - The test `negative_p_point_fails_if_a_power_factor_assumption_is_introduced`
//   FAILS if anyone introduces a power-factor inequality that assumes P ≥ 0
//   (§11.3; ledger P3.08 step 3): such an inequality would reject the
//   negative-P charging point asserted here.
// - Per-mode and voltage-dependent limits are configuration; a MISSING or
//   undocumented voltage defaults to the most restrictive documented value,
//   recorded on the output (ledger P3.08 risk note).
// - Unknown provenance narrows: undeclared modes credit nothing; unknown-
//   provenance modes credit nothing; uncovered Q points collapse to zero
//   (never "unconstrained").
// - IEEE 1547-2018 Clause 5 [R18] scope: interconnection/interoperability
//   requirements only — NOT a universal BMS or internal-protection
//   specification, and implementing the envelope claims NO conformance
//   (stated in the module docs and the decision document).
//
// Tolerance: `PQ_EVALUATION_TOLERANCE` = 1e-9, the module's API contract.
//
// Decision record:
// `ledger/phase-03-flexibility-scheduling/05_08_bess_pq_models.md`.

use flexcore_contracts::capability::Provenance;
use flexcore_contracts::units::ReactiveSignConvention;

use flexcore_domain::inverter_pq_model::{
    apparent_power_mva, InverterPqConfig, InverterPqEnvelope, PBandQRow, PqModeLimits, PqPoint,
    PqViolation, PVoltageRow, PQ_EVALUATION_TOLERANCE, VoltageTreatment,
};

const TOL: f64 = PQ_EVALUATION_TOLERANCE;

fn p_row(voltage: Option<f64>, p_min: f64, p_max: f64) -> PVoltageRow {
    PVoltageRow { voltage_kv: voltage, p_min_mw: p_min, p_max_mw: p_max }
}

fn q_row(p_band: (f64, f64), voltage: Option<f64>, q_min: f64, q_max: f64) -> PBandQRow {
    PBandQRow {
        p_band_min_mw: p_band.0,
        p_band_max_mw: p_band.1,
        voltage_kv: voltage,
        q_min_mvar: q_min,
        q_max_mvar: q_max,
    }
}

/// The synthetic-portfolio inverter profile: S_max = 3 MVA, injection-
/// positive reactive convention (declared, never assumed), separate
/// CHARGE (negative P) and DISCHARGE (positive P) mode tables.
fn envelope() -> InverterPqEnvelope {
    InverterPqEnvelope::new(InverterPqConfig {
        s_max_mva: 3.0,
        reactive_sign: ReactiveSignConvention::InjectionPositive,
        modes: vec![
            PqModeLimits::configured(
                "DISCHARGE",
                vec![p_row(None, 0.0, 3.0)],
                vec![q_row((0.0, 3.0), None, -1.5, 1.5)],
            ),
            PqModeLimits::configured(
                "CHARGE",
                vec![p_row(None, -2.0, 0.0)],
                vec![q_row((-2.0, 0.0), None, -1.5, 1.5)],
            ),
        ],
    })
    .unwrap()
}

fn point(mode: &str, p: f64, q: f64, voltage: Option<f64>) -> PqPoint {
    PqPoint { mode: mode.to_string(), p_mw: p, q_mvar: q, voltage_kv: voltage }
}

// --- Apparent power (necessary but not complete) ------------------------------

#[test]
fn apparent_power_limit_holds_and_is_boundary_inclusive() {
    let envelope = envelope();
    // Exactly on the circle: p² + q² = S_max², with q = 1.5 (the declared Q
    // maximum, within the window) and p = √(9 − 2.25) inside the P window.
    let p = (9.0 - 1.5 * 1.5).sqrt();
    let evaluation = envelope.evaluate(&point("DISCHARGE", p, 1.5, None)).unwrap();
    assert!(evaluation.violations.is_empty(), "{:?}", evaluation.violations);
    assert!((evaluation.apparent_power_mva - 3.0).abs() <= TOL);
    assert_eq!(evaluation.effective_limits.s_max_mva, 3.0);

    // Beyond the circle (p² + q² = 10.25 > 9): named violation with both
    // values.
    let evaluation = envelope.evaluate(&point("DISCHARGE", 2.5, 2.0, None)).unwrap();
    assert!(matches!(
        evaluation.violations.iter().find(|v| matches!(v, PqViolation::ApparentPowerExceeded { .. })),
        Some(PqViolation::ApparentPowerExceeded { s_mva, s_max_mva: 3.0 } if *s_mva > 3.0)
    ));

    // The helper and the evaluation agree.
    assert!((apparent_power_mva(1.5, 2.0) - 2.5).abs() <= TOL);
}

// --- Bidirectional representation and the PF-assumption tripwire --------------

#[test]
fn charge_and_discharge_modes_are_both_represented() {
    let envelope = envelope();
    let discharge = envelope.evaluate(&point("DISCHARGE", 2.0, 0.5, None)).unwrap();
    assert!(discharge.violations.is_empty(), "{:?}", discharge.violations);
    let charge = envelope.evaluate(&point("CHARGE", -2.0, 0.5, None)).unwrap();
    assert!(charge.violations.is_empty(), "{:?}", charge.violations);
    // Both mode tables are declared configuration.
    assert_eq!(envelope.modes().len(), 2);
}

#[test]
fn negative_p_point_fails_if_a_power_factor_assumption_is_introduced() {
    // P = −2 MW (charging), Q = 1 MVAr: P² + Q² = 5 ≤ S_max² = 9 and inside
    // the CHARGE window [−2, 0]. A power-factor inequality assuming P ≥ 0
    // (|Q| ≤ P·tan φ, or √(S² − P²) applied only for P ≥ 0) would REJECT
    // this point and this test would FAIL (§11.3; ledger P3.08 step 3).
    let envelope = envelope();
    let evaluation = envelope.evaluate(&point("CHARGE", -2.0, 1.0, None)).unwrap();
    assert!(
        evaluation.violations.is_empty(),
        "a negative-P point inside the envelope must be accepted (no P >= 0 \
power-factor assumption may exist); violations: {:?}",
        evaluation.violations
    );
    assert!((evaluation.effective_limits.p_min_mw - (-2.0)).abs() <= TOL);

    // The mirrored negative-Q case too: no sign-dependent asymmetry.
    let evaluation = envelope.evaluate(&point("CHARGE", -1.0, -1.4, None)).unwrap();
    assert!(evaluation.violations.is_empty(), "{:?}", evaluation.violations);
}

#[test]
fn per_mode_p_and_q_windows_are_enforced_as_configuration() {
    let envelope = envelope();
    // Discharge above the declared mode maximum.
    let evaluation = envelope.evaluate(&point("DISCHARGE", 3.5, 0.0, None)).unwrap();
    assert!(matches!(
        evaluation.violations.first(),
        Some(PqViolation::PAboveMax { p_mw: 3.5, limit_mw: 3.0 })
    ));
    // Charge below the declared mode minimum.
    let evaluation = envelope.evaluate(&point("CHARGE", -2.5, 0.0, None)).unwrap();
    assert!(matches!(
        evaluation.violations.first(),
        Some(PqViolation::PBelowMin { p_mw: -2.5, limit_mw: -2.0 })
    ));
    // Q at its declared maximum (inclusive) with P inside the band and
    // inside the circle: p = 2.5, q = 1.5 → S = √8.5 ≤ S_max.
    let evaluation = envelope.evaluate(&point("DISCHARGE", 2.5, 1.5, None)).unwrap();
    assert!(evaluation.violations.is_empty(), "{:?}", evaluation.violations);
    // Q beyond the declared maximum.
    let evaluation = envelope.evaluate(&point("DISCHARGE", 1.0, 1.6, None)).unwrap();
    assert!(matches!(
        evaluation.violations.iter().find(|v| matches!(v, PqViolation::QAboveMax { .. })),
        Some(PqViolation::QAboveMax { q_mvar: 1.6, limit_mvar: 1.5 })
    ));
}

// --- Voltage dependence ---------------------------------------------------------

#[test]
fn voltage_dependent_limits_are_configuration() {
    InverterPqEnvelope::new(InverterPqConfig {
        s_max_mva: 3.0,
        reactive_sign: ReactiveSignConvention::InjectionPositive,
        modes: vec![PqModeLimits {
            mode: "DISCHARGE".to_string(),
            provenance: Provenance::Configured,
            p_rows: vec![p_row(Some(11.0), 0.0, 3.0), p_row(Some(0.4), 0.0, 1.0)],
            q_rows: vec![
                q_row((0.0, 3.0), Some(11.0), -1.5, 1.5),
                q_row((0.0, 3.0), Some(0.4), -0.5, 0.5),
            ],
        }],
    })
    .unwrap();
}

#[test]
fn missing_voltage_defaults_to_the_most_restrictive_documented_value() {
    let envelope = InverterPqEnvelope::new(InverterPqConfig {
        s_max_mva: 3.0,
        reactive_sign: ReactiveSignConvention::InjectionPositive,
        modes: vec![PqModeLimits {
            mode: "DISCHARGE".to_string(),
            provenance: Provenance::Configured,
            p_rows: vec![p_row(Some(11.0), 0.0, 3.0), p_row(Some(0.4), 0.0, 1.0)],
            q_rows: vec![q_row((0.0, 3.0), None, -0.5, 0.5)],
        }],
    })
    .unwrap();

    // Missing voltage: the intersection of the documented windows — the most
    // restrictive documented value [0, 1] MW — is used AND recorded.
    let evaluation = envelope.evaluate(&point("DISCHARGE", 2.0, 0.0, None)).unwrap();
    assert!((evaluation.effective_limits.p_max_mw - 1.0).abs() <= TOL);
    assert!(matches!(
        evaluation.violations.first(),
        Some(PqViolation::PAboveMax { limit_mw, .. } if (limit_mw - 1.0).abs() <= TOL)
    ));
    assert!(matches!(
        evaluation.provenance.voltage_treatment,
        VoltageTreatment::MostRestrictiveDocumented { .. }
    ));

    // Documented voltage: that row's window applies.
    let evaluation = envelope.evaluate(&point("DISCHARGE", 2.0, 0.0, Some(11.0))).unwrap();
    assert!(evaluation.violations.is_empty(), "{:?}", evaluation.violations);
    assert!((evaluation.effective_limits.p_max_mw - 3.0).abs() <= TOL);
    assert!(matches!(
        evaluation.provenance.voltage_treatment,
        VoltageTreatment::DocumentedForVoltage { voltage_kv: 11.0 }
    ));

    // Undocumented voltage: same recorded most-restrictive default.
    let evaluation = envelope.evaluate(&point("DISCHARGE", 2.0, 0.0, Some(6.6))).unwrap();
    assert!((evaluation.effective_limits.p_max_mw - 1.0).abs() <= TOL);
    assert!(matches!(
        evaluation.provenance.voltage_treatment,
        VoltageTreatment::MostRestrictiveDocumented { .. }
    ));
}

// --- Unknown narrows --------------------------------------------------------------

#[test]
fn undeclared_mode_credits_nothing() {
    let envelope = envelope();
    let evaluation = envelope.evaluate(&point("GRID_SUPPORT", 0.0, 0.0, None)).unwrap();
    assert!(matches!(
        evaluation.violations.first(),
        Some(PqViolation::ModeNotDeclared { mode } if mode == "GRID_SUPPORT")
    ));
    assert_eq!(evaluation.provenance.mode_provenance, Provenance::Unknown);
}

#[test]
fn unknown_provenance_mode_credits_nothing() {
    let envelope = InverterPqEnvelope::new(InverterPqConfig {
        s_max_mva: 3.0,
        reactive_sign: ReactiveSignConvention::InjectionPositive,
        modes: vec![PqModeLimits {
            mode: "WATT_VAR".to_string(),
            provenance: Provenance::Unknown,
            p_rows: vec![p_row(None, 0.0, 3.0)],
            q_rows: vec![q_row((0.0, 3.0), None, -1.5, 1.5)],
        }],
    })
    .unwrap();
    // Even the origin is not credited: mode credit requires known provenance
    // (§11.1; u is declared, never assumed).
    let evaluation = envelope.evaluate(&point("WATT_VAR", 0.0, 0.0, None)).unwrap();
    assert!(matches!(
        evaluation.violations.first(),
        Some(PqViolation::ModeNotCredited { mode, .. } if mode == "WATT_VAR")
    ));
    assert_eq!(evaluation.provenance.mode_provenance, Provenance::Unknown);
}

#[test]
fn uncovered_q_point_collapses_to_zero_never_unconstrained() {
    let envelope = InverterPqEnvelope::new(InverterPqConfig {
        s_max_mva: 3.0,
        reactive_sign: ReactiveSignConvention::InjectionPositive,
        modes: vec![PqModeLimits::configured(
            "DISCHARGE",
            vec![p_row(None, 0.0, 3.0)],
            vec![q_row((0.0, 1.0), None, -1.5, 1.5)],
        )],
    })
    .unwrap();
    // P = 2.0 is outside every Q row's P-band → Q collapses to [0, 0] MVAr
    // (the recorded P3.02 collapse-to-zero fallback).
    let evaluation = envelope.evaluate(&point("DISCHARGE", 2.0, 0.1, None)).unwrap();
    assert!((evaluation.effective_limits.q_max_mvar - 0.0).abs() <= TOL);
    assert!(matches!(
        evaluation.violations.iter().find(|v| matches!(v, PqViolation::QAboveMax { .. })),
        Some(PqViolation::QAboveMax { limit_mvar, .. } if (limit_mvar - 0.0).abs() <= TOL)
    ));
    assert!(evaluation
        .provenance
        .unknown_dimensions
        .iter()
        .any(|d| d.dimension == "REACTIVE_POWER"));
}

// --- Declared reactive sign ---------------------------------------------------------

#[test]
fn the_reactive_sign_is_declared_never_assumed() {
    // Both conventions are declarable; there is no default (§10.2).
    for sign in [
        ReactiveSignConvention::InjectionPositive,
        ReactiveSignConvention::ConsumptionPositive,
    ] {
        let envelope = InverterPqEnvelope::new(InverterPqConfig {
            s_max_mva: 3.0,
            reactive_sign: sign,
            modes: vec![PqModeLimits::configured(
                "DISCHARGE",
                vec![p_row(None, 0.0, 3.0)],
                vec![q_row((0.0, 3.0), None, -1.5, 1.5)],
            )],
        })
        .unwrap();
        assert_eq!(envelope.reactive_sign(), sign);
        let evaluation = envelope.evaluate(&point("DISCHARGE", 1.0, 0.5, None)).unwrap();
        assert_eq!(evaluation.provenance.reactive_sign, sign);
    }
}

// --- Construction validation ---------------------------------------------------------

#[test]
fn construction_rejects_structurally_invalid_configuration() {
    let mut config = InverterPqConfig {
        s_max_mva: 3.0,
        reactive_sign: ReactiveSignConvention::InjectionPositive,
        modes: vec![],
    };
    assert_eq!(
        InverterPqEnvelope::new(config.clone()),
        Err(flexcore_domain::inverter_pq_model::PqEnvelopeError::NoModesDeclared)
    );
    config.s_max_mva = -1.0;
    assert!(matches!(
        InverterPqEnvelope::new(config),
        Err(flexcore_domain::inverter_pq_model::PqEnvelopeError::NonPositiveSMax { .. })
    ));

    let envelope_error = InverterPqEnvelope::new(InverterPqConfig {
        s_max_mva: 3.0,
        reactive_sign: ReactiveSignConvention::InjectionPositive,
        modes: vec![PqModeLimits::configured(
            "DISCHARGE",
            vec![p_row(None, 4.0, 3.0)],
            vec![],
        )],
    })
    .unwrap_err();
    assert!(matches!(envelope_error, flexcore_domain::inverter_pq_model::PqEnvelopeError::InvertedPLimits { .. }));
    assert!(
        envelope_error.to_string().contains("DISCHARGE") && envelope_error.to_string().contains("P_min"),
        "the error names the mode and the rule: {envelope_error}"
    );
}

#[test]
fn non_finite_operating_points_are_rejected() {
    let envelope = envelope();
    let bad = PqPoint { mode: "DISCHARGE".to_string(), p_mw: f64::NAN, q_mvar: 0.0, voltage_kv: None };
    assert!(matches!(
        envelope.evaluate(&bad),
        Err(flexcore_domain::inverter_pq_model::PqEnvelopeError::NonFiniteValue { field: "p_mw", .. })
    ));
}
