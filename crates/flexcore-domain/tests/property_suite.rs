// P3.25 — Property + unit gap suite for the Phase 3 domain models (§19.2
// layers 1–2; ledger P3.25 steps 1–2).
//
// SCOPE — this file closes GAPS in the existing per-model suites; it does
// not duplicate them. Existing coverage (crates/flexcore-domain/tests/):
// bess_model.rs (worked example, aux load, degradation, reserve, edge
// fixtures, ramp/dwell, a first reserve/energy-window property pair),
// inverter_pq_model.rs (S_max boundary, mode windows, voltage rows, sign),
// flexible_load_model.rs (availability, duration/recovery boundaries),
// ev_fleet_model.rs (departure hard constraint, connection windows, ramp,
// transformer limit, V2X, opt-out), capability_validation.rs (provenance
// narrowing fixtures, units, sign, SOC validation), security_tenant_context.rs.
//
// WHAT IS NEW HERE — property tests with a STATED generated-case count per
// property, over a seeded deterministic generator (splitmix64, Steele et al.
// 2014 — the same generator the crate uses under `cfg(test)`; re-implemented
// here because `test_rng` is `pub(crate)` and integration tests cannot see
// it, and because the dependency rule forbids external property-test crates):
//
// 1. BESS: SOC never leaves [E_min, E_max] on any violation-free evaluation,
//    and every boundary outside the window is flagged — both directions,
//    with boundary values (SOC 0.0/1.0, exact limits, negative magnitudes,
//    over-limit magnitudes) IN THE GENERATOR (the ledger's vacuity risk).
//    512 generated schedules.
// 2. Provenance: unknown-provenance narrowing never widens the declared
//    bound, for every (class, kind, provenance) over generated bounds that
//    include [0,0], [x,0], [0,x], single points, and one-sided bounds.
//    1024 generated bounds.
// 3. Inverter P-Q: a generated point whose apparent power is inside
//    S_max (boundary-inclusive within tolerance) AND inside the declared
//    windows is violation-free; a point outside S_max is ALWAYS flagged.
//    512 generated points per direction (S_max boundary included).
// 4. EV fleet: whenever the model accepts a generated schedule (zero
//    violations), the hard departure-energy constraint is met
//    (`soc_at_departure >= required`); and a schedule that idles past a
//    reachable target is always flagged `DepartureEnergyNotMet`.
//    256 generated feasible schedules.
// 5. Flexible load: duration/recovery enforcement over generated event
//    pairs — any event pair with duration ≤ max and gap ≥ min produces no
//    `enforce_event` violation; any pair violating either bound produces at
//    least one. Boundary-exact durations and gaps are in the generator.
//    512 generated event pairs.
// 6. Unit gaps (not covered elsewhere): the declared SOC↔energy mapping
//    convention (including the degenerate E_min == E_max window), the
//    standalone simultaneous charge/discharge detector, and the
//    UNKNOWN-provenance reserve floor (narrows to 1.0 — no usable energy).
//
// NO COVERAGE FIGURE IS CLAIMED. Coverage is measured by the human with
// `cargo llvm-cov` (command recorded in
// ledger/phase-03-flexibility-scheduling/25_26_tests_scenarios.md); this
// file contributes the static coverage map recorded there.
//
// A violation-free evaluation makes a schedule a CANDIDATE with respect to
// asset constraints only — feasibility, authorization, and dispatch remain
// Phase 4 gates (never implied here).

use flexcore_contracts::capability::{ConstraintKind, Provenance};
use flexcore_contracts::units::ReactiveSignConvention;

use flexcore_domain::bess_model::{
    AuxLoadPoint, BESS_EVALUATION_TOLERANCE, BessModel, BessModelConfig, BessSchedule,
    BessSetpoint, BessViolation, ProvenancedValue, ReserveConfig,
};
use flexcore_domain::capability_provenance::{
    narrowing_never_widens, narrow_interval, AssetClass, BoundInterval,
};
use flexcore_domain::ev_fleet_model::{
    EvFleetConfig, EvFleetModel, EvFleetSchedule, EvSetpoint, EvViolation,
    EvseConfig, EvVehicleConfig, V2xConfig, EV_FLEET_EVALUATION_TOLERANCE,
};
use flexcore_domain::flexible_load_model::{
    enforce_event, DeclaredEvent, EventViolation, LOAD_EVALUATION_TOLERANCE,
};
use flexcore_domain::inverter_pq_model::{
    apparent_power_mva, InverterPqConfig, InverterPqEnvelope, PBandQRow, PQ_EVALUATION_TOLERANCE,
    PqModeLimits, PqPoint, PqViolation, PVoltageRow,
};

/// The stated evaluation tolerances (each model's own API contract).
const BESS_TOL: f64 = BESS_EVALUATION_TOLERANCE;
const PQ_TOL: f64 = PQ_EVALUATION_TOLERANCE;
const EV_TOL: f64 = EV_FLEET_EVALUATION_TOLERANCE;
const LOAD_TOL: f64 = LOAD_EVALUATION_TOLERANCE;

// --- The seeded generator (splitmix64; see the module header) ---------------

/// Deterministic pseudo-random generator for property tests. Only the raw
/// mixing function is needed — fixed seeds make every property run
/// reproducible (the crate's recorded `test_rng` pattern, re-stated here).
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// A value in `[0, bound)`.
    fn below(&mut self, bound: usize) -> usize {
        debug_assert!(bound > 0);
        (self.next_u64() % bound as u64) as usize
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
    /// A finite f64 uniformly spread over `[low, high]`.
    fn f64_in(&mut self, low: f64, high: f64) -> f64 {
        debug_assert!(low <= high && low.is_finite() && high.is_finite());
        let unit = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64; // [0, 1]
        low + unit * (high - low)
    }
    /// A value from `items`, or the supplied boundary value with probability
    /// ~1/4 — the boundary-inclusion mechanism (the ledger's vacuity-risk
    /// mitigation: a generator that never touches the boundary can pass
    /// vacuously).
    fn value_or_boundary<'a>(&mut self, items: &'a [f64], boundary: &'a [f64]) -> f64 {
        if self.below(4) == 0 {
            *self.pick(boundary)
        } else {
            *self.pick(items)
        }
    }
}

// --- Property 1: BESS SOC never leaves [E_min, E_max] ------------------------

/// BESS model fixture: E ∈ [0.5, 9.5] MWh, P limits 3 MW, η = 0.95, Δt = 15
/// min, no optional dimensions configured (their absence is recorded per
/// P3.02 and is irrelevant to the energy-window property).
fn property_bess_model() -> BessModel {
    BessModel::new(BessModelConfig {
        e_min_mwh: ProvenancedValue::configured(0.5),
        e_max_mwh: ProvenancedValue::configured(9.5),
        p_ch_max_mw: ProvenancedValue::configured(3.0),
        p_dis_max_mw: ProvenancedValue::configured(3.0),
        eta_ch: 0.95,
        eta_dis: 0.95,
        interval_seconds: 900,
        soc_power_bands: None,
        temperature_derate: None,
        transitions: None,
        reserve: None,
        degradation: None,
    })
    .expect("the property fixture model is structurally valid")
}

/// One generated setpoint: a magnitude drawn from a spread that INCLUDES the
/// boundary values 0.0 and exactly the power limit, plus over-limit and
/// negative magnitudes (the REJECT policy's inputs), and mixed
/// known/unknown auxiliary-load data.
fn generated_bess_setpoint(rng: &mut SplitMix64, with_aux: bool) -> (BessSetpoint, AuxLoadPoint) {
    // Boundary set: zero, exactly at the limit, one quantum over, negative.
    let boundaries = [0.0, 3.0, 3.0 + 1e-6, -0.5];
    let spread = [0.0, 0.25, 0.75, 1.5, 2.25, 2.999_999_999, 1e-9];
    let charge = rng.value_or_boundary(&spread, &boundaries);
    let discharge = if rng.below(4) == 0 {
        // Occasionally simultaneous (both magnitudes above tolerance) — the
        // mandatory post-solution check must flag it, never hide it.
        rng.value_or_boundary(&spread, &boundaries)
    } else {
        0.0
    };
    let aux = if with_aux {
        match rng.below(4) {
            0 => AuxLoadPoint::Unknown,
            _ => AuxLoadPoint::Known {
                p_aux_mw: rng.f64_in(0.0, 0.05),
                provenance: Provenance::Configured,
            },
        }
    } else {
        AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured }
    };
    (BessSetpoint { p_ch_mw: charge, p_dis_mw: discharge }, aux)
}

/// A generated schedule: initial SOC from the boundary set {0.0, 1.0} and a
/// uniform interior spread; 1–8 intervals of generated setpoints.
fn generated_bess_schedule(rng: &mut SplitMix64) -> BessSchedule {
    let initial_boundaries = [0.0, 1.0];
    let initial_soc = match rng.below(4) {
        0 => *rng.pick(&initial_boundaries),
        _ => rng.f64_in(0.0, 1.0),
    };
    let intervals = 1 + rng.below(8);
    let mut setpoints = Vec::with_capacity(intervals);
    let mut auxiliary = Vec::with_capacity(intervals);
    for _ in 0..intervals {
        let (setpoint, aux) = generated_bess_setpoint(rng, true);
        setpoints.push(setpoint);
        auxiliary.push(aux);
    }
    BessSchedule { initial_soc, setpoints, auxiliary_load: auxiliary }
}

/// PROPERTY (512 generated schedules, seed 0xB315_0001): for every generated
/// schedule, (a) if the evaluation is violation-free then every trajectory
/// boundary SOC lies in [0, 1] (equivalently energy in [E_min, E_max],
/// boundary-inclusive within the module tolerance); and (b) contrapositive —
/// every trajectory boundary outside the energy window is reported by at
/// least one energy-window violation. Direction (b) is what keeps the
/// property from passing vacuously when the generator produces wild
/// schedules.
#[test]
fn property_bess_soc_stays_in_the_energy_window_on_accepted_schedules() {
    const GENERATED_CASES: usize = 512;
    let model = property_bess_model();
    let mut rng = SplitMix64::new(0xB315_0001);
    let mut accepted = 0usize;
    let mut flagged = 0usize;
    for case in 0..GENERATED_CASES {
        let schedule = generated_bess_schedule(&mut rng);
        let evaluation = model
            .evaluate_schedule(&schedule)
            .unwrap_or_else(|e| panic!("case {case}: generated schedule must be evaluable: {e}"));

        let has_window_violation = evaluation.violations.iter().any(|v| {
            matches!(
                v,
                BessViolation::EnergyAboveMax { .. }
                    | BessViolation::EnergyBelowMin { .. }
                    | BessViolation::EnergyWindowInfeasible { .. }
            )
        });
        let all_boundaries_inside = evaluation.interval_socs.iter().all(|&soc| {
            (0.0..=1.0).contains(&soc) || soc.abs() <= BESS_TOL || (soc - 1.0).abs() <= BESS_TOL
        });

        if evaluation.violations.is_empty() {
            accepted += 1;
            assert!(
                all_boundaries_inside,
                "case {case}: a violation-free evaluation must keep every SOC in [0,1] \
                 (socs: {:?})",
                evaluation.interval_socs
            );
            let e_min = model.e_min_mwh();
            let e_max = model.e_max_mwh();
            for (boundary, &energy) in evaluation.interval_energies_mwh.iter().enumerate() {
                assert!(
                    energy >= e_min - BESS_TOL && energy <= e_max + BESS_TOL,
                    "case {case}: boundary {boundary} energy {energy} MWh left the window \
                     [{e_min}, {e_max}] on a violation-free evaluation"
                );
            }
        } else if has_window_violation {
            flagged += 1;
        }
        // Both directions together: an evaluation whose SOCs leave the window
        // must not be violation-free, and must carry a window violation.
        if !all_boundaries_inside {
            assert!(
                has_window_violation,
                "case {case}: SOCs left the energy window but no window violation was \
                 reported (violations: {:?})",
                evaluation.violations
            );
            assert!(
                !evaluation.violations.is_empty(),
                "case {case}: an out-of-window trajectory must not be violation-free"
            );
        }
    }
    // Non-vacuity: the generator produced both accepted and flagged cases.
    assert!(
        accepted > 0,
        "the generator never produced a violation-free schedule — the property ran vacuously"
    );
    assert!(
        flagged > 0,
        "the generator never produced a window-violating schedule — the boundary values \
         were not exercised"
    );
}

// --- Property 2: unknown provenance never widens -----------------------------

/// PROPERTY (1024 generated declared bounds, seed 0xB315_0002, over every
/// asset class, every constraint kind, and every provenance): the narrowed
/// set is always a subset of the declared bound (`narrowing_never_widens`),
/// and for UNKNOWN provenance it is additionally a subset of the recorded
/// conservative default [0, 0]. The generator includes bounds that exclude
/// zero, single points, one-sided bounds, and bounds with an endpoint at
/// exactly 0.0 (the vacuity-risk boundary).
#[test]
fn property_unknown_provenance_never_widens_over_generated_bounds() {
    const GENERATED_CASES: usize = 1024;
    let mut rng = SplitMix64::new(0xB315_0002);
    let default = BoundInterval::new(Some(0.0), Some(0.0)).unwrap();

    for case in 0..GENERATED_CASES {
        // Generated declared bound, with boundary shapes forced in.
        let (lower, upper) = match rng.below(8) {
            // Exactly [0, 0].
            0 => (Some(0.0), Some(0.0)),
            // [0, x] — endpoint exactly at the default boundary.
            1 => (Some(0.0), Some(rng.f64_in(0.0, 10.0))),
            // [x, 0] — excludes the default interior from above.
            2 => (Some(-rng.f64_in(0.0, 10.0)), Some(0.0)),
            // Strictly positive (empty intersection with the default).
            3 => {
                let l = rng.f64_in(0.5, 10.0);
                (Some(l), Some(l + rng.f64_in(0.0, 5.0)))
            }
            // Strictly negative (empty intersection with the default).
            4 => {
                let l = -rng.f64_in(1.0, 10.0);
                (Some(l), Some(l + rng.f64_in(0.0, 5.0)))
            }
            // One-sided lower.
            5 => (Some(rng.f64_in(-5.0, 5.0)), None),
            // One-sided upper.
            6 => (None, Some(rng.f64_in(-5.0, 5.0))),
            // General two-sided.
            _ => {
                let l = rng.f64_in(-10.0, 5.0);
                (Some(l), Some(l + rng.f64_in(0.0, 15.0)))
            }
        };
        let declared = BoundInterval::new(lower, upper)
            .unwrap_or_else(|e| panic!("case {case}: generated bound must be valid: {e}"));

        let class = *rng.pick(AssetClass::ALL.as_slice());
        let kind = *rng.pick(ConstraintKind::ALL.as_slice());
        let provenance = *rng.pick(&Provenance::ALL);

        let narrowed = narrow_interval(class, kind, provenance, declared);
        assert!(
            narrowing_never_widens(&declared, &narrowed),
            "case {case}: {}/{}/{}: the narrowed set exceeded the declared bound \
             (declared {:?}, narrowed {:?})",
            class.name(),
            kind.name(),
            provenance.name(),
            declared,
            narrowed
        );
        if provenance == Provenance::Unknown {
            assert!(
                narrowing_never_widens(&default, &narrowed),
                "case {case}: {}/{}: unknown narrowed outside the recorded [0,0] default \
                 (narrowed {:?})",
                class.name(),
                kind.name(),
                narrowed
            );
        }
    }
}

// --- Property 3: P-Q points stay inside S_max --------------------------------

/// P-Q envelope fixture: S_max 2.5 MVA, one CHARGE and one DISCHARGE mode
/// with wide voltage-independent windows ([-2, 2] MW / [-1.5, 1.5] MVAr) so
/// the S_max circle is the binding dimension for the property.
fn property_pq_envelope() -> InverterPqEnvelope {
    let modes = vec![
        PqModeLimits::configured(
            "CHARGE",
            vec![PVoltageRow { voltage_kv: None, p_min_mw: -2.0, p_max_mw: 2.0 }],
            vec![PBandQRow {
                p_band_min_mw: -2.0,
                p_band_max_mw: 2.0,
                voltage_kv: None,
                q_min_mvar: -1.5,
                q_max_mvar: 1.5,
            }],
        ),
        PqModeLimits::configured(
            "DISCHARGE",
            vec![PVoltageRow { voltage_kv: None, p_min_mw: -2.0, p_max_mw: 2.0 }],
            vec![PBandQRow {
                p_band_min_mw: -2.0,
                p_band_max_mw: 2.0,
                voltage_kv: None,
                q_min_mvar: -1.5,
                q_max_mvar: 1.5,
            }],
        ),
    ];
    InverterPqEnvelope::new(InverterPqConfig {
        s_max_mva: 2.5,
        reactive_sign: ReactiveSignConvention::InjectionPositive,
        modes,
    })
    .expect("the property P-Q fixture is structurally valid")
}

/// PROPERTY (512 generated points inside the circle + 512 generated points
/// outside it, seed 0xB315_0003): a point with apparent power ≤ S_max
/// (boundary-inclusive within the module tolerance) and inside the declared
/// windows is violation-free; a point with apparent power > S_max is ALWAYS
/// flagged `ApparentPowerExceeded`. The generator includes points exactly ON
/// the circle (P = S_max, Q = 0) and exactly at the window corners.
#[test]
fn property_pq_points_stay_inside_s_max() {
    const INSIDE_CASES: usize = 512;
    const OUTSIDE_CASES: usize = 512;
    let envelope = property_pq_envelope();
    let s_max = 2.5;
    let mut rng = SplitMix64::new(0xB315_0003);
    let modes = ["CHARGE", "DISCHARGE"];

    // Direction (a): points inside the circle must not be flagged on S_max.
    let mut on_circle = 0usize;
    for case in 0..INSIDE_CASES {
        // Radius strictly below S_max, or exactly S_max for boundary cases.
        let radius = if rng.below(6) == 0 {
            on_circle += 1;
            s_max - PQ_TOL / 2.0 // exactly on the circle, within tolerance
        } else {
            rng.f64_in(0.0, s_max - 1e-6)
        };
        let angle = rng.f64_in(0.0, std::f64::consts::TAU);
        let p = radius * angle.cos();
        let q = radius * angle.sin();
        let point = PqPoint {
            mode: rng.pick(&modes).to_string(),
            p_mw: p,
            q_mvar: q,
            voltage_kv: None,
        };
        let evaluation = envelope
            .evaluate(&point)
            .unwrap_or_else(|e| panic!("case {case}: inside point must evaluate: {e}"));
        assert!(
            !evaluation
                .violations
                .iter()
                .any(|v| matches!(v, PqViolation::ApparentPowerExceeded { .. })),
            "case {case}: point (P={}, Q={}) with apparent power {} MVA ≤ S_max {s_max} \
             must not be flagged on S_max (violations: {:?})",
            p,
            q,
            evaluation.apparent_power_mva,
            evaluation.violations
        );
        // The recorded apparent power must equal √(P² + Q²).
        assert!(
            (evaluation.apparent_power_mva - apparent_power_mva(p, q)).abs() <= PQ_TOL,
            "case {case}: recorded apparent power does not match √(P² + Q²)"
        );
    }
    assert!(on_circle > 0, "the generator never placed a point exactly on the S_max circle");

    // Direction (b): points outside the circle must ALWAYS be flagged.
    for case in 0..OUTSIDE_CASES {
        let radius = rng.f64_in(s_max + 1e-6, s_max * 2.0);
        let angle = rng.f64_in(0.0, std::f64::consts::TAU);
        let p = radius * angle.cos();
        let q = radius * angle.sin();
        let point = PqPoint {
            mode: rng.pick(&modes).to_string(),
            p_mw: p,
            q_mvar: q,
            voltage_kv: None,
        };
        let evaluation = envelope
            .evaluate(&point)
            .unwrap_or_else(|e| panic!("case {case}: outside point must evaluate: {e}"));
        assert!(
            evaluation
                .violations
                .iter()
                .any(|v| matches!(v, PqViolation::ApparentPowerExceeded { .. })),
            "case {case}: point (P={}, Q={}) with apparent power {} MVA > S_max {s_max} \
             must be flagged ApparentPowerExceeded (violations: {:?})",
            p,
            q,
            evaluation.apparent_power_mva,
            evaluation.violations
        );
    }
}

// --- Property 4: EV departure SOC met whenever the model declares feasible ---

/// The EV fixture horizon: 2026-06-01T00:00:00Z (the pinned scenario clock).
const EV_HORIZON_START_MILLIS: i64 = 1_780_272_000_000;
/// 15-minute scheduling intervals.
const EV_INTERVAL_SECONDS: u32 = 900;
const EV_INTERVAL_MILLIS: i64 = 900_000;

/// One generated vehicle with a REACHABLE departure target: the required
/// departure SOC is chosen so that full-power charging over the connected
/// intervals before departure can reach it (and the boundary case "exactly
/// reachable" is included). Connection window [start, start + span) ending
/// exactly at the departure instant.
fn generated_reachable_vehicle(rng: &mut SplitMix64, key: &str) -> EvVehicleConfig {
    let capacity = rng.f64_in(0.02, 0.1); // MWh
    let initial_soc = rng.f64_in(0.1, 0.5);
    let max_power = rng.f64_in(0.01, 0.05); // MW, ≤ the EVSE rating 0.05
    let connected_intervals = 4 + rng.below(8);
    let departure = EV_HORIZON_START_MILLIS + connected_intervals * EV_INTERVAL_MILLIS;
    // Maximum reachable SOC at departure (idealised, η = 1 conservative):
    // initial + P·t / capacity.
    let reachable =
        initial_soc + max_power * (connected_intervals as f64 * 0.25) / capacity;
    // Required SOC: at most reachable. Boundary case: EXACTLY reachable.
    let required = if rng.below(5) == 0 {
        reachable.min(1.0)
    } else {
        rng.f64_in(initial_soc, reachable.min(1.0))
    };
    EvVehicleConfig {
        vehicle_key: key.to_string(),
        battery_capacity_mwh: capacity,
        initial_soc,
        required_departure_soc: required,
        departure_millis: departure,
        connection_windows_millis: vec![(EV_HORIZON_START_MILLIS, departure)],
        max_charging_power_mw: max_power,
        min_charging_power_mw: 0.0,
        ramp_limit_mw_per_minute: None,
        charging_priority: 1,
        opt_out_from_millis: None,
    }
}

/// A generated charge-by-departure schedule: charge at the full declared
/// power while the required SOC is not yet reached, then idle. The energy
/// needed is reachable by construction of the vehicle.
fn generated_charge_to_target_schedule(vehicle: &EvVehicleConfig, intervals: usize) -> EvFleetSchedule {
    let mut setpoints = Vec::with_capacity(intervals);
    let mut stored = vehicle.initial_soc * vehicle.battery_capacity_mwh;
    let target = vehicle.required_departure_soc * vehicle.battery_capacity_mwh;
    let departure_interval =
        ((vehicle.departure_millis - EV_HORIZON_START_MILLIS) / EV_INTERVAL_MILLIS) as usize;
    for interval in 0..intervals {
        let setpoint = if interval < departure_interval && stored < target - EV_TOL {
            // Charge exactly to the target, or at full power if the target
            // is further away (never past it).
            let dt_h = 0.25;
            let needed = (target - stored) / dt_h;
            let power = needed.min(vehicle.max_charging_power_mw);
            stored += power * dt_h;
            EvSetpoint::charge(power)
        } else {
            EvSetpoint::idle()
        };
        setpoints.push(setpoint);
    }
    EvFleetSchedule { per_vehicle: vec![(vehicle.vehicle_key.clone(), setpoints)] }
}

/// PROPERTY (256 generated vehicles, seed 0xB315_0004): whenever the model
/// accepts a generated charge-to-target schedule (zero violations), the hard
/// departure-energy constraint is MET (`departure_energy_met == true` and
/// `soc_at_departure >= required_departure_soc`); and the boundary vehicle
/// whose target is EXACTLY reachable is met too. The contrapositive fixture
/// (an idle schedule for the same vehicle) is always flagged
/// `DepartureEnergyNotMet` whenever the target exceeds the initial SOC —
/// the constraint is never silently satisfied.
#[test]
fn property_ev_departure_soc_met_whenever_the_schedule_is_accepted() {
    const GENERATED_CASES: usize = 256;
    let mut rng = SplitMix64::new(0xB315_0004);
    let mut accepted_with_target_above_initial = 0usize;

    for case in 0..GENERATED_CASES {
        let vehicle = generated_reachable_vehicle(&mut rng, "ev-1");
        let intervals =
            ((vehicle.departure_millis - EV_HORIZON_START_MILLIS) / EV_INTERVAL_MILLIS) as usize;
        let model = EvFleetModel::new(EvFleetConfig {
            evse: EvseConfig {
                rated_power_mw: 0.05,
                phase: "L1".to_string(),
            },
            vehicles: vec![vehicle.clone()],
            site_transformer_limit_mw: 1.0,
            v2x: V2xConfig::disabled(),
            interval_seconds: EV_INTERVAL_SECONDS,
            horizon_start_millis: EV_HORIZON_START_MILLIS,
        })
        .unwrap_or_else(|e| panic!("case {case}: generated fleet must construct: {e}"));

        let schedule = generated_charge_to_target_schedule(&vehicle, intervals);
        let evaluation = model
            .evaluate_schedule(&schedule)
            .unwrap_or_else(|e| panic!("case {case}: generated schedule must evaluate: {e}"));

        if evaluation.violations.is_empty() {
            let record = &evaluation.per_vehicle[0];
            assert!(
                record.departure_energy_met,
                "case {case}: a violation-free schedule must meet the hard departure \
                 constraint (soc_at_departure {}, required {})",
                record.soc_at_departure, record.required_departure_soc
            );
            assert!(
                record.soc_at_departure >= record.required_departure_soc - EV_TOL,
                "case {case}: departure SOC below the required target within tolerance"
            );
            if record.required_departure_soc > vehicle.initial_soc + EV_TOL {
                accepted_with_target_above_initial += 1;
            }
        }

        // Contrapositive: idling never meets a target above the initial SOC.
        if vehicle.required_departure_soc > vehicle.initial_soc + EV_TOL {
            let idle = EvFleetSchedule {
                per_vehicle: vec![(
                    vehicle.vehicle_key.clone(),
                    vec![EvSetpoint::idle(); intervals],
                )],
            };
            let idle_evaluation = model
                .evaluate_schedule(&idle)
                .unwrap_or_else(|e| panic!("case {case}: idle schedule must evaluate: {e}"));
            assert!(
                idle_evaluation.violations.iter().any(|v| {
                    matches!(v, EvViolation::DepartureEnergyNotMet { .. })
                }),
                "case {case}: an idle schedule with target {} > initial {} must flag \
                 DepartureEnergyNotMet (violations: {:?})",
                vehicle.required_departure_soc,
                vehicle.initial_soc,
                idle_evaluation.violations
            );
        }
    }
    assert!(
        accepted_with_target_above_initial > 0,
        "the generator never produced an accepted schedule with a real charging target — \
         the property ran vacuously"
    );
}

// --- Property 5: load duration/recovery enforcement ---------------------------

/// PROPERTY (512 generated event pairs, seed 0xB315_0005, max duration 3600
/// s, min recovery 900 s): `enforce_event` returns no violation exactly when
/// the event duration ≤ max AND the recovery gap ≥ min (both
/// boundary-inclusive within the module tolerance); any violation of either
/// bound produces the corresponding named violation. The generator includes
/// durations and gaps EXACTLY at the limits (the vacuity-risk boundary).
#[test]
fn property_load_duration_and_recovery_enforcement() {
    const GENERATED_CASES: usize = 512;
    const MAX_DURATION_S: f64 = 3600.0;
    const MIN_RECOVERY_S: f64 = 900.0;
    let mut rng = SplitMix64::new(0xB315_0005);
    let mut feasible_pairs = 0usize;
    let mut flagged_pairs = 0usize;

    for case in 0..GENERATED_CASES {
        // Duration: boundary-exact, one quantum under/over, or uniform.
        let duration_ms = match rng.below(4) {
            0 => (MAX_DURATION_S * 1000.0) as i64, // exactly at the limit
            1 => (MAX_DURATION_S * 1000.0) as i64 + 60_000, // over
            2 => (MAX_DURATION_S * 1000.0) as i64 - 60_000, // under
            _ => (rng.f64_in(60.0, 7200.0) * 1000.0) as i64,
        };
        // Recovery gap: boundary-exact, one quantum under/over, or uniform.
        let gap_ms = match rng.below(4) {
            0 => (MIN_RECOVERY_S * 1000.0) as i64, // exactly at the limit
            1 => (MIN_RECOVERY_S * 1000.0) as i64 - 1_000, // under
            2 => (MIN_RECOVERY_S * 1000.0) as i64 + 1_000, // over
            _ => (rng.f64_in(0.0, 3600.0) * 1000.0) as i64,
        };

        let first = DeclaredEvent {
            start_millis: EV_HORIZON_START_MILLIS,
            end_millis: EV_HORIZON_START_MILLIS + duration_ms,
            power_mw: 0.2,
        };
        let second = DeclaredEvent {
            start_millis: first.end_millis + gap_ms,
            end_millis: first.end_millis + gap_ms + duration_ms,
            power_mw: 0.2,
        };

        // The FIRST event has no predecessor: duration only.
        let first_violations = enforce_event(&first, None, MAX_DURATION_S, MIN_RECOVERY_S);
        let duration_ok = duration_ms as f64 / 1000.0 <= MAX_DURATION_S + LOAD_TOL;
        assert_eq!(
            first_violations.is_empty(),
            duration_ok,
            "case {case}: first event (duration {} s) violations {:?} disagree with the \
             duration bound",
            duration_ms as f64 / 1000.0,
            first_violations
        );

        // The SECOND event has a predecessor: duration AND recovery.
        let second_violations =
            enforce_event(&second, Some(&first), MAX_DURATION_S, MIN_RECOVERY_S);
        let gap_ok = gap_ms as f64 / 1000.0 + LOAD_TOL >= MIN_RECOVERY_S;
        let has_duration = second_violations
            .iter()
            .any(|v| matches!(v, EventViolation::DurationExceeded { .. }));
        let has_recovery = second_violations
            .iter()
            .any(|v| matches!(v, EventViolation::RecoveryIncomplete { .. }));
        assert_eq!(
            has_duration,
            !duration_ok,
            "case {case}: duration violation flag disagrees with the bound"
        );
        assert_eq!(
            has_recovery,
            !gap_ok,
            "case {case}: recovery violation flag disagrees with the bound (gap {} s, \
             violations {:?})",
            gap_ms as f64 / 1000.0,
            second_violations
        );
        if second_violations.is_empty() {
            feasible_pairs += 1;
        } else {
            flagged_pairs += 1;
        }
    }
    assert!(
        feasible_pairs > 0 && flagged_pairs > 0,
        "the generator produced only one side of the boundary — the property ran one-sided \
         (feasible {feasible_pairs}, flagged {flagged_pairs})"
    );
}

// --- Unit gaps (not covered by the existing suites) ---------------------------

/// UNIT GAP 1: the declared SOC↔energy mapping convention. SOC is defined
/// over the EFFECTIVE window, `SOC = (E − E_min)/(E_max − E_min)` — not
/// over [0, rated]; the mapping round-trips, and a degenerate window
/// (E_min == E_max) maps its single value to SOC 0.0.
#[test]
fn unit_soc_energy_mapping_uses_the_effective_window_and_degenerates_to_zero() {
    let model = property_bess_model();
    // The window is [0.5, 9.5]: SOC 0.5 must be 5.0 MWh, NOT 0.5 MWh.
    let at_half = model.soc_to_energy(0.5);
    assert!(
        (at_half - 5.0).abs() <= BESS_TOL,
        "SOC 0.5 must map to the effective-window midpoint 5.0 MWh (got {at_half})"
    );
    assert!((model.energy_to_soc(5.0) - 0.5).abs() <= BESS_TOL);
    // Round trip over the boundaries and the midpoint.
    for soc in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let energy = model.soc_to_energy(soc);
        assert!(
            (model.energy_to_soc(energy) - soc).abs() <= BESS_TOL,
            "the mapping must round-trip at SOC {soc}"
        );
    }
    // Degenerate window: E_min == E_max maps its single value to 0.0.
    let degenerate = BessModel::new(BessModelConfig {
        e_min_mwh: ProvenancedValue::configured(2.0),
        e_max_mwh: ProvenancedValue::configured(2.0),
        p_ch_max_mw: ProvenancedValue::configured(1.0),
        p_dis_max_mw: ProvenancedValue::configured(1.0),
        eta_ch: 0.9,
        eta_dis: 0.9,
        interval_seconds: 900,
        soc_power_bands: None,
        temperature_derate: None,
        transitions: None,
        reserve: None,
        degradation: None,
    })
    .expect("a degenerate but ordered window is valid");
    assert!((degenerate.energy_to_soc(2.0)).abs() <= BESS_TOL);
}

/// UNIT GAP 2: the standalone mandatory post-solution simultaneous
/// charge/discharge detector returns exactly the intervals where BOTH
/// magnitudes exceed the tolerance — and nothing at tolerance or below.
#[test]
fn unit_simultaneous_charge_discharge_detector_reports_exactly_both_above_tolerance() {
    let setpoints = vec![
        BessSetpoint::charge(1.0),                             // clean charge
        BessSetpoint { p_ch_mw: 0.5, p_dis_mw: 0.5 },          // simultaneous
        BessSetpoint { p_ch_mw: BESS_TOL, p_dis_mw: BESS_TOL }, // at tolerance: not flagged
        BessSetpoint::idle(),
        BessSetpoint::discharge(2.0),                          // clean discharge
        BessSetpoint { p_ch_mw: -1.0, p_dis_mw: -1.0 },        // negatives: not simultaneous
    ];
    let flagged = BessModel::detect_simultaneous_charge_discharge(&setpoints);
    assert_eq!(flagged, vec![1], "exactly interval 1 carries both magnitudes above tolerance");
}

/// UNIT GAP 3: an UNKNOWN-provenance reserve floor narrows to the most
/// conservative reading — floor 1.0, i.e. no usable energy is credited
/// (unknown never widens, P3.02) — and the usable window collapses to a
/// single point at E_max.
#[test]
fn unit_unknown_reserve_floor_narrows_to_no_usable_energy() {
    let model = BessModel::new(BessModelConfig {
        e_min_mwh: ProvenancedValue::configured(0.0),
        e_max_mwh: ProvenancedValue::configured(4.0),
        p_ch_max_mw: ProvenancedValue::configured(1.0),
        p_dis_max_mw: ProvenancedValue::configured(1.0),
        eta_ch: 0.95,
        eta_dis: 0.95,
        interval_seconds: 900,
        soc_power_bands: None,
        temperature_derate: None,
        transitions: None,
        reserve: Some(ReserveConfig {
            reserve_floor_soc: 0.2, // the declared value is IRRELEVANT: unknown wins
            provenance: Provenance::Unknown,
        }),
        degradation: None,
    })
    .expect("the unknown-reserve fixture is structurally valid");

    let (usable_min, usable_max) = model
        .usable_energy_window()
        .expect("the energy window itself is feasible");
    assert!(
        (usable_min - 4.0).abs() <= BESS_TOL && (usable_max - 4.0).abs() <= BESS_TOL,
        "an unknown reserve floor must leave no usable energy below E_max \
         (window: [{usable_min}, {usable_max}])"
    );
    // And a schedule that discharges below E_max is flagged against the
    // reserve floor — the narrowing is enforced, not just recorded.
    let schedule = BessSchedule {
        initial_soc: 1.0,
        setpoints: vec![BessSetpoint::discharge(1.0)],
        auxiliary_load: vec![AuxLoadPoint::Known {
            p_aux_mw: 0.0,
            provenance: Provenance::Configured,
        }],
    };
    let evaluation = model.evaluate_schedule(&schedule).expect("evaluates");
    assert!(
        evaluation
            .violations
            .iter()
            .any(|v| matches!(v, BessViolation::EnergyBelowReserveFloor { .. })),
        "discharging below the collapsed usable window must be flagged \
         (violations: {:?})",
        evaluation.violations
    );
}
