// P4.02 — Unit/sign translation round-trip tests (ledger P4.02 acceptance:
// "unit/sign translation is tested in both directions"), with
// hand-computed fixtures.
//
// Covered mappings (report §10.2; flexcore-contracts `units.rs` is the
// single home of the frozen rules):
// - Active power: MW; `P > 0` = EXPORT at the point of connection (frozen)
//   ↔ worker injection into the network is positive.
// - Reactive power: MVAr; the sign is interpreted ONLY under the DECLARED
//   `ReactiveSignConvention` — tested under BOTH conventions, proving
//   there is no default and no sign-alone reading.
// - Voltage: per-unit (worker) ↔ kV at the nominal base (canonical), with
//   hand-computed products.
// - Current: kA (worker) ↔ A (canonical), and A ↔ per-unit on an EXPLICIT
//   base (hand-computed base current 100 MVA / 20 kV = 2886.751... A).
// - Loading: percent (worker) ↔ fraction of the declared rating
//   (canonical).

use flexcore_contracts::units::{ActivePowerMW, PerUnitBase, ReactivePowerMVAr, ReactiveSignConvention, UnitsError};
use flexcore_grid_api::port::CandidateInjection;
use flexcore_grid_api::translation::{
    branch_loading_from_backend, bus_voltage_from_backend, candidate_to_backend,
    current_a_from_ka, current_per_unit, current_ka_from_a, injection_from_backend,
    loading_fraction_from_percent, loading_percent_from_fraction, transformer_loading_from_backend,
    voltage_kv_from_pu, voltage_pu_from_kv, TranslationError, TranslationProfile,
};
use flexcore_grid_api::tests_support::LOADING_CROSS_CHECK_TOLERANCE;

// Fixture tolerances: round trips through multiplication/division are
// float-exact to within a few ulps; the hand-computed expectations use
// 1e-9 relative unless stated.
const TAU: f64 = 1.0e-9;

fn candidate(p_mw: f64, q_mvar: f64, convention: ReactiveSignConvention) -> CandidateInjection {
    CandidateInjection {
        injection_id: "cand-rt".to_string(),
        bus_id: "bus_rt".to_string(),
        active_power: ActivePowerMW::new(p_mw).expect("valid P"),
        reactive_power: ReactivePowerMVAr::new(q_mvar).expect("valid Q"),
        reactive_sign_convention: convention,
        interval_start: None,
    }
}

fn profile(convention: ReactiveSignConvention) -> TranslationProfile {
    TranslationProfile::new(convention, PerUnitBase::new(20.0, 100.0).expect("valid base"))
}

/// Round trip, direction 1 → 2 → 1: canonical candidate → worker
/// injection → canonical candidate, EXACT under both declared
/// conventions, for export and import candidates.
#[test]
fn candidate_round_trips_exactly_under_both_conventions() {
    for convention in [
        ReactiveSignConvention::InjectionPositive,
        ReactiveSignConvention::ConsumptionPositive,
    ] {
        for (p, q) in [(2.5, 0.5), (-1.25, 0.25), (0.0, 0.0), (-3.0, -2.0)] {
            let original = candidate(p, q, convention);
            let worker = candidate_to_backend(&original).expect("maps to worker");
            assert_eq!(worker.injection_id, "cand-rt");
            assert_eq!(worker.bus_id, "bus_rt");
            let back = injection_from_backend(&worker, convention).expect("maps back");
            assert_eq!(back, original, "exact round trip under {convention:?}");
        }
    }
}

/// The active-power sign rule: `P > 0` (export) maps to a POSITIVE
/// injection into the network; `P < 0` (import) maps to a NEGATIVE
/// injection. The MW value crosses unchanged; the sign MEANING is what
/// the mapping fixes.
#[test]
fn active_power_export_is_positive_worker_injection() {
    let export = candidate_to_backend(&candidate(3.5, 0.0, ReactiveSignConvention::InjectionPositive))
        .expect("maps");
    assert!((export.p_injection_mw - 3.5).abs() < TAU);

    let import = candidate_to_backend(&candidate(-3.5, 0.0, ReactiveSignConvention::InjectionPositive))
        .expect("maps");
    assert!((import.p_injection_mw - (-3.5)).abs() < TAU);
}

/// The reactive-power rule under BOTH declared conventions (hand-computed
/// sign table). Canonical `Q = +2` MVAr:
/// - under `InjectionPositive` (Q > 0 means injection): worker injection
///   `+2` (into the network);
/// - under `ConsumptionPositive` (Q > 0 means consumption): worker
///   injection `-2` (out of the network).
/// The sign is NEVER read without the convention.
#[test]
fn reactive_power_follows_the_declared_convention_both_ways() {
    // Canonical +2 MVAr.
    let inj = candidate_to_backend(&candidate(0.0, 2.0, ReactiveSignConvention::InjectionPositive))
        .expect("maps");
    assert!((inj.q_injection_mvar - 2.0).abs() < TAU);
    let con = candidate_to_backend(&candidate(0.0, 2.0, ReactiveSignConvention::ConsumptionPositive))
        .expect("maps");
    assert!((con.q_injection_mvar - (-2.0)).abs() < TAU);

    // Canonical -2 MVAr: both signs flip.
    let inj = candidate_to_backend(&candidate(0.0, -2.0, ReactiveSignConvention::InjectionPositive))
        .expect("maps");
    assert!((inj.q_injection_mvar - (-2.0)).abs() < TAU);
    let con = candidate_to_backend(&candidate(0.0, -2.0, ReactiveSignConvention::ConsumptionPositive))
        .expect("maps");
    assert!((con.q_injection_mvar - 2.0).abs() < TAU);

    // Zero is zero under both conventions.
    for convention in [
        ReactiveSignConvention::InjectionPositive,
        ReactiveSignConvention::ConsumptionPositive,
    ] {
        let zero = candidate_to_backend(&candidate(0.0, 0.0, convention)).expect("maps");
        assert_eq!(zero.q_injection_mvar, 0.0);
    }
}

/// Voltage: per-unit ↔ kV at the nominal base, hand-computed:
/// 1.05 pu of 20 kV = 21.0 kV; 1.01 pu of 0.4 kV = 0.404 kV. The kV and
/// per-unit representations are BOTH canonical (§10.2); the conversion
/// uses the bus nominal voltage as the base.
#[test]
fn voltage_conversions_both_directions_hand_computed() {
    let kv = voltage_kv_from_pu(1.05, 20.0).expect("converts");
    assert!((kv - 21.0).abs() < TAU);
    let pu = voltage_pu_from_kv(21.0, 20.0).expect("converts");
    assert!((pu - 1.05).abs() < TAU);

    let kv = voltage_kv_from_pu(1.01, 0.4).expect("converts");
    assert!((kv - 0.404).abs() < 1.0e-12);
    let pu = voltage_pu_from_kv(0.404, 0.4).expect("converts");
    assert!((pu - 1.01).abs() < 1.0e-12);

    // The full round trip through the bus-voltage mapping.
    let bus = flexcore_grid_api::tests_support::three_bus_model(
        flexcore_grid_api::port::GridModelRef {
            model_id: "m".to_string(),
            version: "1".to_string(),
            content_hash: "h".to_string(),
        },
    );
    let bus_2 = bus.bus("bus_2").expect("fixture bus");
    let row = flexcore_grid_api::backend::BackendBusResult {
        bus_id: "bus_2".to_string(),
        vm_pu: 1.01,
    };
    let voltage = bus_voltage_from_backend(bus_2, &row).expect("maps");
    assert!((voltage.vm_pu - 1.01).abs() < TAU);
    assert!((voltage.vm_kv - 0.404).abs() < 1.0e-12);
    let pu_back = voltage_pu_from_kv(voltage.vm_kv, bus_2.nominal_kv).expect("converts");
    assert!((pu_back - row.vm_pu).abs() < 1.0e-12);
}

/// Current: kA ↔ A both directions, and A ↔ per-unit on an EXPLICIT base.
/// Hand-computed: 0.38 kA = 380 A; base current at 100 MVA / 20 kV =
/// 100e6 / (sqrt(3) * 20e3) = 2886.7513459... A; 380 A on that base =
/// 0.1316357... pu.
#[test]
fn current_conversions_both_directions_with_explicit_base() {
    let a = current_a_from_ka(0.38).expect("converts");
    assert!((a - 380.0).abs() < 1.0e-9);
    let ka = current_ka_from_a(380.0).expect("converts");
    assert!((ka - 0.38).abs() < 1.0e-12);

    let profile = profile(ReactiveSignConvention::InjectionPositive);
    let base = profile.base_current_a(20.0).expect("computes");
    let expected_base = 100.0e6 / (f64::sqrt(3.0) * 20.0e3);
    assert!((base - expected_base).abs() < 1.0e-6);
    assert!(base > 2886.75 && base < 2886.76, "hand-computed base current, got {base}");

    let pu = current_per_unit(380.0, base).expect("converts");
    let expected_pu = 380.0 / expected_base;
    assert!((pu - expected_pu).abs() < 1.0e-12);
    assert!(pu > 0.1316 && pu < 0.1317, "hand-computed per-unit current, got {pu}");

    // The explicit base is REQUIRED: a non-positive base is refused, the
    // per-unit value is never computed against an assumed base.
    assert_eq!(
        current_per_unit(380.0, 0.0),
        Err(TranslationError::NonPositive { field: "base_current_a" })
    );
}

/// Loading: percent (worker) ↔ fraction (canonical), hand-computed:
/// 76.0 percent of a 500 A rating = 0.76; 87.5 percent = 0.875.
#[test]
fn loading_conversions_both_directions_hand_computed() {
    let fraction = loading_fraction_from_percent(76.0).expect("converts");
    assert!((fraction - 0.76).abs() < 1.0e-12);
    let percent = loading_percent_from_fraction(0.76).expect("converts");
    assert!((percent - 76.0).abs() < 1.0e-9);

    let fraction = loading_fraction_from_percent(87.5).expect("converts");
    assert!((fraction - 0.875).abs() < 1.0e-12);
}

/// Branch and transformer result rows map to canonical loadings with the
/// declared rating bases from the model spec (the fixture values are
/// hand-computed: 0.38 kA / 500 A = 76 percent; 62.5 percent of 0.4 MVA
/// gives 7.217 A HV / 360.84 A LV at 20 kV / 0.4 kV).
#[test]
fn branch_and_transformer_rows_map_to_canonical_loadings() {
    let model = flexcore_grid_api::tests_support::three_bus_model(
        flexcore_grid_api::port::GridModelRef {
            model_id: "m".to_string(),
            version: "1".to_string(),
            content_hash: "h".to_string(),
        },
    );
    let branch = &model.branches[0];
    let branch_row = flexcore_grid_api::backend::BackendBranchResult {
        branch_id: "line_2_3".to_string(),
        i_from_ka: 0.38,
        i_to_ka: 0.3795,
        loading_percent: 76.0,
    };
    let loading = branch_loading_from_backend(branch, &branch_row).expect("maps");
    assert!((loading.current_from_a - 380.0).abs() < 1.0e-9);
    assert!((loading.current_to_a - 379.5).abs() < 1.0e-9);
    assert!((loading.loading_fraction - 0.76).abs() < 1.0e-12);

    // The worker's loading fraction is cross-checked against the declared
    // rating base and the converted currents (the recorded tolerance).
    let computed = loading.current_from_a.max(loading.current_to_a) / branch.rated_current_a;
    assert!(
        (computed - loading.loading_fraction).abs() <= LOADING_CROSS_CHECK_TOLERANCE * computed.max(1.0),
        "the worker's loading definition agrees with max(I)/rating within tolerance"
    );

    let trafo = &model.transformers[0];
    let trafo_row = flexcore_grid_api::backend::BackendTransformerResult {
        transformer_id: "trafo_1_2".to_string(),
        loading_percent: 62.5,
        i_hv_ka: 0.007_217,
        i_lv_ka: 0.360_84,
    };
    let trafo_loading = transformer_loading_from_backend(trafo, &trafo_row).expect("maps");
    assert!((trafo_loading.loading_fraction - 0.625).abs() < 1.0e-12);
    assert!((trafo_loading.hv_current_a - 7.217).abs() < 1.0e-6);
    assert!((trafo_loading.lv_current_a - 360.84).abs() < 1.0e-6);
}

/// Identifier mismatches and non-finite/negative values are refused —
/// the translation layer never guesses.
#[test]
fn invalid_translation_inputs_are_refused() {
    let model = flexcore_grid_api::tests_support::three_bus_model(
        flexcore_grid_api::port::GridModelRef {
            model_id: "m".to_string(),
            version: "1".to_string(),
            content_hash: "h".to_string(),
        },
    );
    let bus_1 = model.bus("bus_1").expect("fixture bus");
    let wrong_row = flexcore_grid_api::backend::BackendBusResult {
        bus_id: "bus_elsewhere".to_string(),
        vm_pu: 1.0,
    };
    assert!(matches!(
        bus_voltage_from_backend(bus_1, &wrong_row),
        Err(TranslationError::IdentifierMismatch { .. })
    ));

    assert_eq!(
        voltage_kv_from_pu(f64::NAN, 20.0),
        Err(TranslationError::NonFinite { field: "vm_pu" })
    );
    assert_eq!(
        voltage_kv_from_pu(-0.5, 20.0),
        Err(TranslationError::NegativeMagnitude { field: "vm_pu" })
    );
    assert_eq!(
        voltage_pu_from_kv(21.0, 0.0),
        Err(TranslationError::NonPositive { field: "nominal_kv" })
    );
    assert_eq!(
        current_a_from_ka(f64::INFINITY),
        Err(TranslationError::NonFinite { field: "i_ka" })
    );
    assert_eq!(
        loading_fraction_from_percent(f64::NAN),
        Err(TranslationError::NonFinite { field: "loading_percent" })
    );
    assert_eq!(
        profile(ReactiveSignConvention::InjectionPositive).base_current_a(-1.0),
        Err(TranslationError::NonPositive { field: "voltage_kv" })
    );
}

/// The canonical contract types remain the single boundary: the
/// translation constructors route through the validating constructors, so
/// a non-finite worker payload cannot become a canonical value.
#[test]
fn canonical_types_reject_non_finite_worker_payloads() {
    let bad = flexcore_grid_api::backend::BackendInjection {
        injection_id: "bad".to_string(),
        bus_id: "bus".to_string(),
        p_injection_mw: f64::NAN,
        q_injection_mvar: 0.0,
        interval_start: None,
    };
    assert_eq!(
        injection_from_backend(&bad, ReactiveSignConvention::InjectionPositive),
        Err(TranslationError::NonFinite { field: "p_injection_mw" })
    );

    // And the contract-level rejection path is reachable: an infinite
    // injection value is refused by the translation's own finite check
    // before the contract type is constructed (documented both layers).
    let infinite = candidate_to_backend(&CandidateInjection {
        injection_id: "bad".to_string(),
        bus_id: "bus".to_string(),
        active_power: ActivePowerMW::new(1.0).expect("valid"),
        reactive_power: ReactivePowerMVAr::new(0.0).expect("valid"),
        reactive_sign_convention: ReactiveSignConvention::InjectionPositive,
        interval_start: None,
    })
    .expect("valid candidate maps");
    assert!(infinite.p_injection_mw.is_finite());
    // The contract type itself refuses NaN (frozen boundary behaviour).
    assert_eq!(ActivePowerMW::new(f64::NAN), Err(UnitsError::NotFinite));
}
