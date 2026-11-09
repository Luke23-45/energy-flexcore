// P3.11 — EV fleet model test suite (integration, from outside the crate).
//
// Covers the ledger acceptance criteria end to end, from OUTSIDE
// flexcore-domain, with fixtures configured from the synthetic-portfolio
// scenario (scenario-v2.json `site-north-evse-1` / `site-south-evse-1`
// where the scenario declares a value) and fixture values labelled
// ASSUMPTION where it does not:
//
// - Departure energy is a HARD constraint: a vehicle that does not reach
//   its required SOC by departure is a named violation in every test
//   schedule; the `DepartureEnergyPolicy::Hard` declaration is exposed for
//   the problem IR (P3.17) and the relaxation ledger (P3.19). No test
//   relaxes it.
// - V2X is OFF by default: any discharge is a named violation; the named
//   fixture `v2x-enabled-fixture-001` documents ALL FIVE §11.5 conditions
//   with per-condition evidence, and an incomplete evidence set is a
//   construction error.
// - Opt-out mid-horizon: an opted-out vehicle drops out of the feasible set
//   FROM THE OPT-OUT TIME ONWARD — it leaves the active-vehicle lists and
//   the dispatchable capacity from that instant (capacity is not silently
//   retained; the envelope re-derivation itself is P3.24's job).
// - OCPP 2.1 [R19] is cited only for capability SCOPE (asserted on the
//   recorded statement) — never as a claim that a device supports it.
//
// Tolerance: `EV_FLEET_EVALUATION_TOLERANCE` = 1e-9, the module's API
// contract.
//
// Decision record:
// `ledger/phase-03-flexibility-scheduling/09_11_load_ev_models.md`.

use flexcore_domain::ev_fleet_model::{
    EvFleetConfig, EvFleetModel, EvFleetSchedule, EvSetpoint, EvVehicleConfig, EvseConfig,
    DepartureEnergyPolicy, V2xConditionEvidence, V2xConfig, V2xEvidence,
    DEPARTURE_ENERGY_POLICY_STATEMENT, EV_FLEET_EVALUATION_TOLERANCE, OCPP_SCOPE_STATEMENT,
    V2X_DISABLED_BY_DEFAULT_REASON,
};

const TOL: f64 = EV_FLEET_EVALUATION_TOLERANCE;

/// 2026-06-01T00:00:00Z — the scenario-v2 horizon start.
const HORIZON_START_MILLIS: i64 = 1_780_272_000_000;
const INTERVAL_SECONDS: u32 = 900; // 15 minutes

fn approx(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= TOL
}

/// The scenario-declared connection window of `site-north-evse-1` expressed
/// on the test horizon: connected 00:00–08:00, departure at 08:00 (the
/// window end — the vehicle charges up to but excluding 08:00, §10.3
/// end-exclusive semantics).
fn window_a() -> (i64, i64) {
    (HORIZON_START_MILLIS, HORIZON_START_MILLIS + 8 * 3_600_000)
}

/// Vehicle A: one aggregate EV per EVSE (scenario assumption), battery
/// 0.06 MWh (ASSUMPTION — the scenario declares no capacity), initial SOC
/// 0.3 and required departure SOC 0.9 (scenario `initial_soc`/`soc_target`),
/// charging 0.05 MW (scenario `charging_power_mw`).
fn vehicle_a(opt_out_from_millis: Option<i64>) -> EvVehicleConfig {
    EvVehicleConfig {
        vehicle_key: "vehicle-aggregate-north".to_string(),
        battery_capacity_mwh: 0.06,
        initial_soc: 0.3,
        required_departure_soc: 0.9,
        departure_millis: HORIZON_START_MILLIS + 8 * 3_600_000,
        connection_windows_millis: vec![window_a()],
        max_charging_power_mw: 0.05,
        min_charging_power_mw: 0.0,
        ramp_limit_mw_per_minute: None,
        charging_priority: 1,
        opt_out_from_millis,
    }
}

/// Vehicle B: second vehicle for the fleet-level tests, connected
/// 00:00–10:00 with departure at 10:00; battery 0.08 MWh and the SOC figures
/// from scenario `site-south-evse-1` (`initial_soc` 0.25, `soc_target`
/// 0.85); charging 0.05 MW (ASSUMPTION fixture under the shared EVSE).
fn vehicle_b(opt_out_from_millis: Option<i64>, departure_hours: i64) -> EvVehicleConfig {
    EvVehicleConfig {
        vehicle_key: "vehicle-aggregate-south".to_string(),
        battery_capacity_mwh: 0.08,
        initial_soc: 0.25,
        required_departure_soc: 0.85,
        departure_millis: HORIZON_START_MILLIS + departure_hours * 3_600_000,
        connection_windows_millis: vec![(
            HORIZON_START_MILLIS,
            HORIZON_START_MILLIS + departure_hours * 3_600_000,
        )],
        max_charging_power_mw: 0.05,
        min_charging_power_mw: 0.0,
        ramp_limit_mw_per_minute: None,
        charging_priority: 1,
        opt_out_from_millis,
    }
}

/// The EVSE of `site-north-evse-1`: rated 0.05 MW (scenario
/// `charging_power_mw`/`rated_power_mw`); the phase is an ASSUMPTION
/// fixture (the scenario declares no phase).
fn evse(rated_power_mw: f64) -> EvseConfig {
    EvseConfig { rated_power_mw, phase: "three-phase".to_string() }
}

/// A single-vehicle fleet on vehicle A (the scenario shape: one aggregate
/// EV per EVSE). The horizon ends at A's departure.
fn single_vehicle_fleet(v2x: V2xConfig) -> EvFleetModel {
    EvFleetModel::new(EvFleetConfig {
        evse: evse(0.05),
        vehicles: vec![vehicle_a(None)],
        // ASSUMPTION fixture: the scenario declares no transformer limit.
        site_transformer_limit_mw: 0.05,
        v2x,
        interval_seconds: INTERVAL_SECONDS,
        horizon_start_millis: HORIZON_START_MILLIS,
    })
    .unwrap()
}

/// Charges `intervals` intervals at the EVSE rating from the connection
/// start, then idles.
fn charge_from_start(vehicle_key: &str, intervals: usize, power_mw: f64, total_intervals: usize) -> (String, Vec<EvSetpoint>) {
    let mut points = vec![EvSetpoint::idle(); total_intervals];
    for point in points.iter_mut().take(intervals) {
        *point = EvSetpoint::charge(power_mw);
    }
    (vehicle_key.to_string(), points)
}

fn schedule(model: &EvFleetModel, per_vehicle: Vec<(String, Vec<EvSetpoint>)>) -> EvFleetSchedule {
    let _ = model;
    EvFleetSchedule { per_vehicle }
}

// --- The hard departure-energy constraint ----------------------------------------

#[test]
fn departure_energy_is_a_hard_constraint() {
    let model = single_vehicle_fleet(V2xConfig::disabled());
    // The policy is declared Hard and carries its statement — the
    // declaration the problem IR (P3.17) and the relaxation ledger (P3.19)
    // consume.
    assert_eq!(model.departure_policy(), DepartureEnergyPolicy::Hard);
    assert_eq!(
        model.departure_policy().statement(),
        DEPARTURE_ENERGY_POLICY_STATEMENT
    );
    assert!(DEPARTURE_ENERGY_POLICY_STATEMENT.contains("HARD constraint"));

    // Vehicle A needs (0.9 − 0.3) × 0.06 = 0.036 MWh; 3 intervals at
    // 0.05 MW deliver 0.0375 MWh: the required SOC is reached by departure.
    let evaluation = model
        .evaluate_schedule(&schedule(&model, vec![charge_from_start(
            "vehicle-aggregate-north",
            3,
            0.05,
            model.interval_count(),
        )]))
        .unwrap();
    assert!(
        evaluation.violations.is_empty(),
        "a feasible charging schedule must be violation-free: {:?}",
        evaluation.violations
    );
    let vehicle = &evaluation.per_vehicle[0];
    assert!(vehicle.departure_energy_met);
    // 0.018 + 0.0375 = 0.0555 MWh → SOC 0.925.
    assert!(approx(vehicle.soc_at_departure, 0.925));

    // Two intervals deliver 0.025 MWh — 0.043 MWh stored against the
    // required 0.9 × 0.06 = 0.054 MWh: the named HARD violation is
    // reported, the schedule is not repaired, and no relaxation exists in
    // the module to call.
    let evaluation = model
        .evaluate_schedule(&schedule(&model, vec![charge_from_start(
            "vehicle-aggregate-north",
            2,
            0.05,
            model.interval_count(),
        )]))
        .unwrap();
    assert!(matches!(
        evaluation.violations.iter().find(|v| matches!(v, flexcore_domain::ev_fleet_model::EvViolation::DepartureEnergyNotMet { .. })),
        Some(flexcore_domain::ev_fleet_model::EvViolation::DepartureEnergyNotMet { required_mwh, achieved_mwh, .. }
            if approx(required_mwh, 0.054) && approx(achieved_mwh, 0.043))
    ));
    assert!(!evaluation.per_vehicle[0].departure_energy_met);
}

#[test]
fn charging_outside_connection_window_is_rejected_and_gated() {
    let mut vehicle = vehicle_a(None);
    // Connected only 02:00–08:00; charging from the horizon start is
    // outside the window.
    vehicle.connection_windows_millis =
        vec![(HORIZON_START_MILLIS + 2 * 3_600_000, HORIZON_START_MILLIS + 8 * 3_600_000)];
    let model = EvFleetModel::new(EvFleetConfig {
        evse: evse(0.05),
        vehicles: vec![vehicle],
        site_transformer_limit_mw: 0.05,
        v2x: V2xConfig::disabled(),
        interval_seconds: INTERVAL_SECONDS,
        horizon_start_millis: HORIZON_START_MILLIS,
    })
    .unwrap();
    let evaluation = model
        .evaluate_schedule(&schedule(&model, vec![charge_from_start(
            "vehicle-aggregate-north",
            3,
            0.05,
            model.interval_count(),
        )]))
        .unwrap();
    assert!(matches!(
        evaluation.violations.iter().find(|v| matches!(v, flexcore_domain::ev_fleet_model::EvViolation::ChargingOutsideConnectionWindow { interval: 0, .. })),
        Some(flexcore_domain::ev_fleet_model::EvViolation::ChargingOutsideConnectionWindow { interval: 0, .. })
    ));
    // The connection window is a physical gate on the balance: intervals
    // 0–2 apply no energy (the initial SOC carries through), the violation
    // is still reported — nothing is repaired, nothing is fantasized.
    let initial_energy = 0.3 * 0.06;
    assert!(approx(evaluation.per_vehicle[0].energy_trajectory_mwh[3], initial_energy));
}

// --- Minimum power and ramp --------------------------------------------------------

#[test]
fn below_minimum_charging_power_is_rejected() {
    let mut vehicle = vehicle_a(None);
    vehicle.min_charging_power_mw = 0.02;
    let model = EvFleetModel::new(EvFleetConfig {
        evse: evse(0.05),
        vehicles: vec![vehicle],
        site_transformer_limit_mw: 0.05,
        v2x: V2xConfig::disabled(),
        interval_seconds: INTERVAL_SECONDS,
        horizon_start_millis: HORIZON_START_MILLIS,
    })
    .unwrap();
    let mut points = vec![EvSetpoint::idle(); model.interval_count()];
    points[4] = EvSetpoint::charge(0.01);
    let evaluation = model
        .evaluate_schedule(&schedule(&model, vec![("vehicle-aggregate-north".to_string(), points)]))
        .unwrap();
    assert!(matches!(
        evaluation.violations.iter().find(|v| matches!(v, flexcore_domain::ev_fleet_model::EvViolation::BelowMinimumChargingPower { .. })),
        Some(flexcore_domain::ev_fleet_model::EvViolation::BelowMinimumChargingPower { charge_mw, min_mw, .. }
            if approx(charge_mw, 0.01) && approx(min_mw, 0.02))
    ));
    // Exactly at the minimum (boundary-inclusive) is feasible.
    let mut points = vec![EvSetpoint::idle(); model.interval_count()];
    points[4] = EvSetpoint::charge(0.02);
    let evaluation = model
        .evaluate_schedule(&schedule(&model, vec![("vehicle-aggregate-north".to_string(), points)]))
        .unwrap();
    assert!(
        !evaluation
            .violations
            .iter()
            .any(|v| matches!(v, flexcore_domain::ev_fleet_model::EvViolation::BelowMinimumChargingPower { .. }))
    );
}

#[test]
fn ramp_violations_are_detected_and_the_first_interval_is_exempt() {
    let mut vehicle = vehicle_a(None);
    // 0.1 MW/min → 0.025 MW per 15-minute interval.
    vehicle.ramp_limit_mw_per_minute = Some(0.1);
    let model = EvFleetModel::new(EvFleetConfig {
        evse: evse(0.05),
        vehicles: vec![vehicle],
        site_transformer_limit_mw: 0.05,
        v2x: V2xConfig::disabled(),
        interval_seconds: INTERVAL_SECONDS,
        horizon_start_millis: HORIZON_START_MILLIS,
    })
    .unwrap();
    // The first interval is exempt (no prior setpoint exists in the model).
    let mut points = vec![EvSetpoint::idle(); model.interval_count()];
    points[0] = EvSetpoint::charge(0.05);
    let evaluation = model
        .evaluate_schedule(&schedule(&model, vec![("vehicle-aggregate-north".to_string(), points)]))
        .unwrap();
    assert!(
        !evaluation
            .violations
            .iter()
            .any(|v| matches!(v, flexcore_domain::ev_fleet_model::EvViolation::RampViolation { .. })),
        "the first interval must be ramp-exempt: {:?}",
        evaluation.violations
    );
    // 0 → 0.05 MW one interval later exceeds the 0.025 MW per-interval
    // limit.
    let mut points = vec![EvSetpoint::idle(); model.interval_count()];
    points[4] = EvSetpoint::charge(0.05);
    let evaluation = model
        .evaluate_schedule(&schedule(&model, vec![("vehicle-aggregate-north".to_string(), points)]))
        .unwrap();
    assert!(matches!(
        evaluation.violations.iter().find(|v| matches!(v, flexcore_domain::ev_fleet_model::EvViolation::RampViolation { .. })),
        Some(flexcore_domain::ev_fleet_model::EvViolation::RampViolation { net_change_mw, limit_mw_per_interval, .. }
            if approx(net_change_mw, 0.05) && approx(limit_mw_per_interval, 0.025))
    ));
}

// --- The site transformer limit ------------------------------------------------------

#[test]
fn site_transformer_limit_is_enforced_per_interval() {
    let model = EvFleetModel::new(EvFleetConfig {
        evse: evse(0.15),
        vehicles: vec![vehicle_a(None), vehicle_b(None, 10)],
        // ASSUMPTION fixture limit below the simultaneous draw.
        site_transformer_limit_mw: 0.08,
        v2x: V2xConfig::disabled(),
        interval_seconds: INTERVAL_SECONDS,
        horizon_start_millis: HORIZON_START_MILLIS,
    })
    .unwrap();
    let total = model.interval_count();
    let a = charge_from_start("vehicle-aggregate-north", 3, 0.05, total);
    let b = charge_from_start("vehicle-aggregate-south", 3, 0.05, total);
    let evaluation = model
        .evaluate_schedule(&schedule(&model, vec![a, b]))
        .unwrap();
    // Both vehicles charge during intervals 0–11 (A is connected 00:00–08:00,
    // B 00:00–10:00): the fleet import is 0.1 MW, above the 0.08 MW limit.
    assert!(matches!(
        evaluation.violations.iter().find(|v| matches!(v, flexcore_domain::ev_fleet_model::EvViolation::SiteTransformerLimitExceeded { interval: 0, .. })),
        Some(flexcore_domain::ev_fleet_model::EvViolation::SiteTransformerLimitExceeded { fleet_import_mw, limit_mw, .. }
            if approx(fleet_import_mw, 0.1) && approx(limit_mw, 0.08))
    ));
    assert!(approx(evaluation.fleet_import_mw[0], 0.1));
    // P > 0 = export: charging is NEGATIVE net export.
    assert!(approx(evaluation.fleet_net_export_mw[0], -0.1));
    // Exactly at the limit (boundary-inclusive) is feasible.
    let model = EvFleetModel::new(EvFleetConfig {
        evse: evse(0.15),
        vehicles: vec![vehicle_a(None), vehicle_b(None, 10)],
        site_transformer_limit_mw: 0.1,
        v2x: V2xConfig::disabled(),
        interval_seconds: INTERVAL_SECONDS,
        horizon_start_millis: HORIZON_START_MILLIS,
    })
    .unwrap();
    let total = model.interval_count();
    let evaluation = model
        .evaluate_schedule(&schedule(&model, vec![
            charge_from_start("vehicle-aggregate-north", 3, 0.05, total),
            charge_from_start("vehicle-aggregate-south", 3, 0.05, total),
        ]))
        .unwrap();
    assert!(
        !evaluation
            .violations
            .iter()
            .any(|v| matches!(v, flexcore_domain::ev_fleet_model::EvViolation::SiteTransformerLimitExceeded { .. }))
    );
}

// --- V2X: off by default, enabled only with all five conditions -------------------------

#[test]
fn v2x_is_disabled_by_default_and_discharge_is_a_named_violation() {
    let model = single_vehicle_fleet(V2xConfig::disabled());
    assert!(!model.v2x_enabled());
    assert_eq!(model.v2x(), &V2xConfig::DisabledByDefault);
    let mut points = vec![EvSetpoint::idle(); model.interval_count()];
    points[4] = EvSetpoint::discharge(0.02);
    let evaluation = model
        .evaluate_schedule(&schedule(&model, vec![("vehicle-aggregate-north".to_string(), points)]))
        .unwrap();
    assert!(matches!(
        evaluation.violations.iter().find(|v| matches!(v, flexcore_domain::ev_fleet_model::EvViolation::DischargeWithV2xDisabled { .. })),
        Some(flexcore_domain::ev_fleet_model::EvViolation::DischargeWithV2xDisabled { discharge_mw, .. }
            if approx(discharge_mw, 0.02))
    ));
    assert!(!evaluation.v2x_enabled);
    assert_eq!(evaluation.v2x_default_statement, V2X_DISABLED_BY_DEFAULT_REASON);
    // The V2X-disabled discharge is a hardware-impossible energy flow: the
    // balance applies none (the violation is still reported).
    let initial_energy = 0.3 * 0.06;
    assert!(approx(evaluation.per_vehicle[0].energy_trajectory_mwh[5], initial_energy));
}

/// The NAMED test fixture `v2x-enabled-fixture-001`: the only V2X-enabled
/// case in the synthetic portfolio. ALL FIVE §11.5 conditions are documented
/// here, each evidence string labelling its synthetic status — this is the
/// documented enablement the ledger requires, not a device claim.
fn v2x_enabled_fixture() -> V2xConfig {
    V2xConfig::enable(V2xEvidence {
        ev: V2xConditionEvidence {
            supported: true,
            evidence: "v2x-enabled-fixture-001 ASSUMPTION: the fixture vehicle \
is declared bidirectional-capable; no real EV datasheet exists in the scenario"
                .to_string(),
        },
        evse: V2xConditionEvidence {
            supported: true,
            evidence: "v2x-enabled-fixture-001 ASSUMPTION: the fixture EVSE is \
declared bidirectional-capable; no real EVSE datasheet exists in the scenario"
                .to_string(),
        },
        protocol: V2xConditionEvidence {
            supported: true,
            evidence: "v2x-enabled-fixture-001 SCOPE citation: OCPP 2.1 includes \
expanded smart-charging and V2X-related capabilities [R19] — cited for \
capability scope only; the fixture protocol is declared to support the \
required bidirectional control, which is NOT a claim that any device \
implements or conforms to OCPP 2.1"
                .to_string(),
        },
        interconnection_agreement: V2xConditionEvidence {
            supported: true,
            evidence: "v2x-enabled-fixture-001 ASSUMPTION: the fixture site \
interconnection agreement is declared to permit vehicle export; no real \
agreement exists in the scenario"
                .to_string(),
        },
        market_rules: V2xConditionEvidence {
            supported: true,
            evidence: "v2x-enabled-fixture-001 ASSUMPTION: the fixture market \
rules are declared to permit the bidirectional service; D1 is open and all \
market data is synthetic and labelled"
                .to_string(),
        },
    })
    .unwrap()
}

#[test]
fn v2x_enablement_requires_all_five_documented_conditions() {
    // Each of the five conditions unsupported → rejected.
    let base = || V2xEvidence {
        ev: V2xConditionEvidence { supported: true, evidence: "evidence".to_string() },
        evse: V2xConditionEvidence { supported: true, evidence: "evidence".to_string() },
        protocol: V2xConditionEvidence { supported: true, evidence: "evidence".to_string() },
        interconnection_agreement: V2xConditionEvidence {
            supported: true,
            evidence: "evidence".to_string(),
        },
        market_rules: V2xConditionEvidence { supported: true, evidence: "evidence".to_string() },
    };
    let mut missing_ev = base();
    missing_ev.ev.supported = false;
    assert!(matches!(
        V2xConfig::enable(missing_ev),
        Err(flexcore_domain::ev_fleet_model::EvFleetModelError::V2xEnablementIncomplete { .. })
    ));
    let mut missing_evse = base();
    missing_evse.evse.supported = false;
    assert!(V2xConfig::enable(missing_evse).is_err());
    let mut missing_protocol = base();
    missing_protocol.protocol.supported = false;
    assert!(V2xConfig::enable(missing_protocol).is_err());
    let mut missing_agreement = base();
    missing_agreement.interconnection_agreement.supported = false;
    assert!(V2xConfig::enable(missing_agreement).is_err());
    let mut missing_market = base();
    missing_market.market_rules.supported = false;
    assert!(V2xConfig::enable(missing_market).is_err());
    // Empty evidence is rejected even when the condition is "supported".
    let mut unevidenced = base();
    unevidenced.protocol.evidence = "   ".to_string();
    assert!(matches!(
        V2xConfig::enable(unevidenced),
        Err(flexcore_domain::ev_fleet_model::EvFleetModelError::V2xEnablementIncomplete { .. })
    ));
    // The named fixture documents all five and enables.
    let fixture = v2x_enabled_fixture();
    assert!(fixture.is_enabled());
}

#[test]
fn v2x_enabled_fixture_allows_discharge_within_the_declared_windows() {
    let model = EvFleetModel::new(EvFleetConfig {
        evse: evse(0.05),
        vehicles: vec![vehicle_a(None)],
        site_transformer_limit_mw: 0.05,
        v2x: v2x_enabled_fixture(),
        interval_seconds: INTERVAL_SECONDS,
        horizon_start_millis: HORIZON_START_MILLIS,
    })
    .unwrap();
    assert!(model.v2x_enabled());
    // Discharge while connected is now feasible; the vehicle must still
    // meet its departure SOC (the hard constraint is untouched by V2X).
    // 3 charge intervals reach 0.0555 MWh; a 0.005 MW discharge over one
    // interval draws 0.00125 MWh, leaving 0.05425 ≥ the required 0.054.
    let mut points = vec![EvSetpoint::idle(); model.interval_count()];
    points[0] = EvSetpoint::charge(0.05);
    points[1] = EvSetpoint::charge(0.05);
    points[2] = EvSetpoint::charge(0.05);
    points[8] = EvSetpoint::discharge(0.005);
    let evaluation = model
        .evaluate_schedule(&schedule(&model, vec![("vehicle-aggregate-north".to_string(), points)]))
        .unwrap();
    assert!(
        !evaluation
            .violations
            .iter()
            .any(|v| matches!(v, flexcore_domain::ev_fleet_model::EvViolation::DischargeWithV2xDisabled { .. })),
        "discharge is feasible with the documented V2X fixture: {:?}",
        evaluation.violations
    );
    assert!(
        !evaluation
            .violations
            .iter()
            .any(|v| matches!(v, flexcore_domain::ev_fleet_model::EvViolation::DepartureEnergyNotMet { .. })),
        "the vehicle must still meet its departure SOC: {:?}",
        evaluation.violations
    );
    // The discharge exports: fleet net export = +0.005 MW in interval 8.
    assert!(approx(evaluation.fleet_net_export_mw[8], 0.005));
    // The discharge drained the battery by 0.005 × 0.25 h = 0.00125 MWh.
    let energy_before = 0.3 * 0.06 + 3.0 * 0.05 * 0.25;
    assert!(approx(evaluation.per_vehicle[0].energy_trajectory_mwh[9], energy_before - 0.00125));
    // Departure energy stays a HARD constraint under V2X as well.
    assert_eq!(evaluation.departure_policy, DepartureEnergyPolicy::Hard);
}

// --- Opt-out mid-horizon --------------------------------------------------------------

#[test]
fn opted_out_vehicle_drops_out_of_the_feasible_set_from_the_opt_out_instant() {
    // B opts out at 02:00 UTC (interval 8), having finished charging in the
    // first hour. From the opt-out instant onward B contributes nothing.
    let model = EvFleetModel::new(EvFleetConfig {
        evse: evse(0.15),
        vehicles: vec![vehicle_a(None), vehicle_b(Some(HORIZON_START_MILLIS + 2 * 3_600_000), 10)],
        site_transformer_limit_mw: 0.5,
        v2x: V2xConfig::disabled(),
        interval_seconds: INTERVAL_SECONDS,
        horizon_start_millis: HORIZON_START_MILLIS,
    })
    .unwrap();
    let total = model.interval_count();
    let mut b_points = vec![EvSetpoint::idle(); total];
    for point in b_points.iter_mut().take(4) {
        *point = EvSetpoint::charge(0.05);
    }
    let evaluation = model
        .evaluate_schedule(&schedule(&model, vec![
            charge_from_start("vehicle-aggregate-north", 4, 0.05, total),
            ("vehicle-aggregate-south".to_string(), b_points),
        ]))
        .unwrap();

    // Before the opt-out instant both vehicles are active; capacity 0.1 MW.
    assert!(approx(evaluation.dispatchable_capacity_mw[0], 0.1));
    assert!(approx(evaluation.dispatchable_capacity_mw[7], 0.1));
    assert!(evaluation.active_vehicle_keys_per_interval[0]
        .contains(&"vehicle-aggregate-south".to_string()));

    // FROM THE OPT-OUT TIME ONWARD the vehicle is out of the feasible set:
    // not in the active lists, no capacity retained.
    for interval in 8..total {
        assert!(
            !evaluation.active_vehicle_keys_per_interval[interval]
                .contains(&"vehicle-aggregate-south".to_string()),
            "interval {interval} is at or after the opt-out instant"
        );
        assert!(
            approx(evaluation.dispatchable_capacity_mw[interval], 0.05),
            "capacity is not silently retained at interval {interval}"
        );
    }
    // The capacity step happens exactly at the opt-out interval.
    assert!(approx(evaluation.dispatchable_capacity_mw[7], 0.1));
    assert!(approx(evaluation.dispatchable_capacity_mw[8], 0.05));
    // The model-level helper agrees: at the opt-out instant the vehicle is
    // already out (start-inclusive).
    let opt_out = HORIZON_START_MILLIS + 2 * 3_600_000;
    assert!(model.vehicle_active_at("vehicle-aggregate-south", opt_out - 1));
    assert!(!model.vehicle_active_at("vehicle-aggregate-south", opt_out));
    // B met its departure energy (it finished charging before opting out).
    let b = evaluation
        .per_vehicle
        .iter()
        .find(|v| v.vehicle_key == "vehicle-aggregate-south")
        .unwrap();
    assert!(b.departure_energy_met);
    // The envelope re-derivation itself is P3.24's job; this model only
    // removes the vehicle from the feasible set from the opt-out instant.
}

#[test]
fn dispatched_after_opt_out_is_a_named_violation_and_gated() {
    let model = EvFleetModel::new(EvFleetConfig {
        evse: evse(0.05),
        vehicles: vec![vehicle_a(Some(HORIZON_START_MILLIS + 3_600_000))],
        site_transformer_limit_mw: 0.05,
        v2x: V2xConfig::disabled(),
        interval_seconds: INTERVAL_SECONDS,
        horizon_start_millis: HORIZON_START_MILLIS,
    })
    .unwrap();
    let total = model.interval_count();
    let mut points = vec![EvSetpoint::idle(); total];
    points[0] = EvSetpoint::charge(0.05); // interval 0 = 00:00, before the opt-out: feasible
    points[8] = EvSetpoint::charge(0.05); // interval 8 = 02:00, at/after opt-out: violation
    let evaluation = model
        .evaluate_schedule(&schedule(&model, vec![("vehicle-aggregate-north".to_string(), points)]))
        .unwrap();
    assert!(matches!(
        evaluation.violations.iter().find(|v| matches!(v, flexcore_domain::ev_fleet_model::EvViolation::DispatchedAfterOptOut { interval: 8, .. })),
        Some(flexcore_domain::ev_fleet_model::EvViolation::DispatchedAfterOptOut { interval: 8, opt_out_millis, .. }
            if *opt_out_millis == HORIZON_START_MILLIS + 3_600_000)
    ));
    // The opt-out is a physical gate: interval 8 applies no energy.
    let energy_before_opt_out = evaluation.per_vehicle[0].energy_trajectory_mwh[8];
    assert!(approx(evaluation.per_vehicle[0].energy_trajectory_mwh[9], energy_before_opt_out));
    // A feasible pre-opt-out interval is not flagged.
    assert!(
        !evaluation
            .violations
            .iter()
            .any(|v| matches!(v, flexcore_domain::ev_fleet_model::EvViolation::DispatchedAfterOptOut { interval: 4, .. }))
    );
}

#[test]
fn opt_out_mid_charge_leaves_the_departure_constraint_hard_and_unmet() {
    // B opts out at 01:00 (interval 4) after charging only the first two
    // intervals: it cannot reach its required departure SOC, and the HARD
    // constraint is reported unmet — never softened. The schedule is not
    // repaired and no relaxation exists in the module to call.
    let model = EvFleetModel::new(EvFleetConfig {
        evse: evse(0.15),
        vehicles: vec![vehicle_b(Some(HORIZON_START_MILLIS + 3_600_000), 10)],
        site_transformer_limit_mw: 0.5,
        v2x: V2xConfig::disabled(),
        interval_seconds: INTERVAL_SECONDS,
        horizon_start_millis: HORIZON_START_MILLIS,
    })
    .unwrap();
    let total = model.interval_count();
    let evaluation = model
        .evaluate_schedule(&schedule(&model, vec![charge_from_start(
            "vehicle-aggregate-south",
            2,
            0.05,
            total,
        )]))
        .unwrap();
    // 2 intervals × 0.05 MW × 0.25 h = 0.025 MWh → 0.02 + 0.025 = 0.045 MWh
    // at the gate; required 0.85 × 0.08 = 0.068 MWh: unmet.
    assert!(matches!(
        evaluation.violations.iter().find(|v| matches!(v, flexcore_domain::ev_fleet_model::EvViolation::DepartureEnergyNotMet { .. })),
        Some(flexcore_domain::ev_fleet_model::EvViolation::DepartureEnergyNotMet { required_mwh, achieved_mwh, .. }
            if approx(required_mwh, 0.068) && approx(achieved_mwh, 0.045))
    ));
    // No post-opt-out dispatch was attempted: no DispatchedAfterOptOut
    // violation — the shortfall is solely the honest departure-energy fact.
    assert!(
        !evaluation
            .violations
            .iter()
            .any(|v| matches!(v, flexcore_domain::ev_fleet_model::EvViolation::DispatchedAfterOptOut { .. }))
    );
    // The gate froze the battery at the opt-out boundary; the vehicle is out
    // of the feasible set from the opt-out instant (interval 4) onward.
    assert!(approx(evaluation.dispatchable_capacity_mw[3], 0.05));
    assert!(approx(evaluation.dispatchable_capacity_mw[4], 0.0));
}

// --- Construction validation -------------------------------------------------------------

#[test]
fn construction_rejects_structurally_invalid_configuration() {
    // Empty fleet.
    assert_eq!(
        EvFleetModel::new(EvFleetConfig {
            evse: evse(0.05),
            vehicles: vec![],
            site_transformer_limit_mw: 0.05,
            v2x: V2xConfig::disabled(),
            interval_seconds: INTERVAL_SECONDS,
            horizon_start_millis: HORIZON_START_MILLIS,
        }),
        Err(flexcore_domain::ev_fleet_model::EvFleetModelError::NoVehiclesDeclared)
    );
    // Departure not on an interval boundary.
    let mut vehicle = vehicle_a(None);
    vehicle.departure_millis = HORIZON_START_MILLIS + 8 * 3_600_000 + 1;
    assert!(matches!(
        EvFleetModel::new(EvFleetConfig {
            evse: evse(0.05),
            vehicles: vec![vehicle],
            site_transformer_limit_mw: 0.05,
            v2x: V2xConfig::disabled(),
            interval_seconds: INTERVAL_SECONDS,
            horizon_start_millis: HORIZON_START_MILLIS,
        }),
        Err(flexcore_domain::ev_fleet_model::EvFleetModelError::DepartureNotOnIntervalBoundary { .. })
    ));
    // Departure not covered by a connection window.
    let mut vehicle = vehicle_a(None);
    vehicle.connection_windows_millis =
        vec![(HORIZON_START_MILLIS, HORIZON_START_MILLIS + 4 * 3_600_000)];
    assert!(matches!(
        EvFleetModel::new(EvFleetConfig {
            evse: evse(0.05),
            vehicles: vec![vehicle],
            site_transformer_limit_mw: 0.05,
            v2x: V2xConfig::disabled(),
            interval_seconds: INTERVAL_SECONDS,
            horizon_start_millis: HORIZON_START_MILLIS,
        }),
        Err(flexcore_domain::ev_fleet_model::EvFleetModelError::DepartureOutsideConnectionWindows { .. })
    ));
    // Minimum charging power above the maximum.
    let mut vehicle = vehicle_a(None);
    vehicle.min_charging_power_mw = 0.06;
    assert!(matches!(
        EvFleetModel::new(EvFleetConfig {
            evse: evse(0.05),
            vehicles: vec![vehicle],
            site_transformer_limit_mw: 0.05,
            v2x: V2xConfig::disabled(),
            interval_seconds: INTERVAL_SECONDS,
            horizon_start_millis: HORIZON_START_MILLIS,
        }),
        Err(flexcore_domain::ev_fleet_model::EvFleetModelError::MinChargingAboveMax { .. })
    ));
    // Vehicle maximum above the EVSE rating.
    let mut vehicle = vehicle_a(None);
    vehicle.max_charging_power_mw = 0.5;
    assert!(matches!(
        EvFleetModel::new(EvFleetConfig {
            evse: evse(0.05),
            vehicles: vec![vehicle],
            site_transformer_limit_mw: 0.05,
            v2x: V2xConfig::disabled(),
            interval_seconds: INTERVAL_SECONDS,
            horizon_start_millis: HORIZON_START_MILLIS,
        }),
        Err(flexcore_domain::ev_fleet_model::EvFleetModelError::VehiclePowerAboveEvseRating { .. })
    ));
    // SOC out of range.
    let mut vehicle = vehicle_a(None);
    vehicle.required_departure_soc = 1.1;
    assert!(matches!(
        EvFleetModel::new(EvFleetConfig {
            evse: evse(0.05),
            vehicles: vec![vehicle],
            site_transformer_limit_mw: 0.05,
            v2x: V2xConfig::disabled(),
            interval_seconds: INTERVAL_SECONDS,
            horizon_start_millis: HORIZON_START_MILLIS,
        }),
        Err(flexcore_domain::ev_fleet_model::EvFleetModelError::SocOutOfRange { field: "required_departure_soc", .. })
    ));
    // Duplicate vehicle key.
    assert!(matches!(
        EvFleetModel::new(EvFleetConfig {
            evse: evse(0.05),
            vehicles: vec![vehicle_a(None), vehicle_a(None)],
            site_transformer_limit_mw: 0.05,
            v2x: V2xConfig::disabled(),
            interval_seconds: INTERVAL_SECONDS,
            horizon_start_millis: HORIZON_START_MILLIS,
        }),
        Err(flexcore_domain::ev_fleet_model::EvFleetModelError::InvalidVehicleKey { .. })
    ));
    // Opt-out outside the horizon (after the last departure).
    let mut vehicle = vehicle_a(None);
    vehicle.opt_out_from_millis = Some(HORIZON_START_MILLIS + 9 * 3_600_000);
    assert!(matches!(
        EvFleetModel::new(EvFleetConfig {
            evse: evse(0.05),
            vehicles: vec![vehicle],
            site_transformer_limit_mw: 0.05,
            v2x: V2xConfig::disabled(),
            interval_seconds: INTERVAL_SECONDS,
            horizon_start_millis: HORIZON_START_MILLIS,
        }),
        Err(flexcore_domain::ev_fleet_model::EvFleetModelError::OptOutOutsideHorizon { .. })
    ));
}

// --- Recorded unknown dimensions and the OCPP scope statement ------------------------------

#[test]
fn unknown_dimensions_are_recorded_never_silent() {
    let model = single_vehicle_fleet(V2xConfig::disabled());
    let evaluation = model
        .evaluate_schedule(&schedule(&model, vec![charge_from_start(
            "vehicle-aggregate-north",
            3,
            0.05,
            model.interval_count(),
        )]))
        .unwrap();
    // No charging-efficiency value exists anywhere; the lossless balance is
    // a recorded limitation, never an invented efficiency.
    let efficiency = evaluation
        .unknown_dimensions
        .iter()
        .find(|d| d.dimension == "CHARGING_EFFICIENCY")
        .expect("the charging efficiency must be recorded unknown");
    assert!(efficiency.note.contains("no value is invented"));
    // No export-side site limit data exists.
    assert!(evaluation
        .unknown_dimensions
        .iter()
        .any(|d| d.dimension == "SITE_EXPORT_LIMIT"));
    // Ramp unconfigured → recorded unknown.
    assert!(evaluation.unknown_dimensions.iter().any(|d| d.dimension == "RAMP_RATE"));
}

#[test]
fn ocpp_2_1_is_cited_for_capability_scope_only() {
    // The recorded statement pins the scope discipline: OCPP 2.1 [R19] is a
    // capability-scope citation, never a device-support or conformance
    // claim.
    assert!(OCPP_SCOPE_STATEMENT.contains("OCPP 2.1"));
    assert!(OCPP_SCOPE_STATEMENT.contains("CAPABILITY SCOPE ONLY"));
    assert!(OCPP_SCOPE_STATEMENT.contains("never as a claim"));
    assert!(OCPP_SCOPE_STATEMENT.contains("[R19]"));
    // The named V2X fixture's protocol evidence repeats the discipline.
    let fixture = v2x_enabled_fixture();
    if let V2xConfig::Enabled { evidence } = &fixture {
        assert!(evidence.protocol.evidence.contains("capability scope only"));
        assert!(evidence.protocol.evidence.contains("NOT a claim"));
    } else {
        panic!("the named fixture must enable V2X");
    }
}
