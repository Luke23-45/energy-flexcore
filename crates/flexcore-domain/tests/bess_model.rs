// P3.05/P3.06/P3.07 — BESS capability-model test suite (integration, from
// outside the crate).
//
// Covers the ledger acceptance criteria end to end, from OUTSIDE
// flexcore-domain:
//
// - P3.05: the §11.2 discrete energy balance reproduces a HAND-COMPUTED
//   4-interval worked example (charge, discharge, idle, charge-to-full) with
//   exact expected SOC values; the auxiliary-load term (foundations §7.2) is
//   applied where data exists and RECORDED (never silently zero) where it
//   does not; simultaneous charge/discharge is flagged by the mandatory
//   post-solution detector in every case.
// - P3.06: the degradation cost is the configurable linear THROUGHPUT
//   placeholder with default zero and visible placeholder metadata; the
//   reserve floor (configuration, not physics) narrows the usable energy
//   window and is never violated by any schedule the model accepts
//   (property over generated schedules, deterministic splitmix64 seeds).
// - P3.07: the five edge fixtures (SOC exactly E_min, exactly E_max, one
//   interval from full while discharging, one from empty while charging,
//   charge→discharge transition) pass; no fixture produces SOC outside
//   [E_min, E_max]; charge-when-full is REJECTED (the recorded policy — no
//   clipping); ramp/dwell are enforced where configured.
//
// TOLERANCE (stated explicitly, ledger P3.07 risk note): all numeric
// assertions use `BESS_EVALUATION_TOLERANCE` = 1e-9 (the module's API
// contract). Boundary values are feasible; floating-point SOC drift is
// absorbed by this tolerance, and the P3.21 residual check reuses the same
// constant.
//
// Decision record:
// `ledger/phase-03-flexibility-scheduling/05_08_bess_pq_models.md`.

use flexcore_contracts::capability::Provenance;

use flexcore_domain::bess_model::{
    AuxLoadPoint, AuxLoadState, BESS_EVALUATION_TOLERANCE, BessModel, BessModelConfig,
    BessOperatingMode, BessSchedule, BessSetpoint, BessViolation, DegradationConfig,
    DegradationMetadata, ProvenancedValue, ReserveConfig, SocPowerBand, TemperatureDerate,
    TransitionConstraints,
};
use flexcore_domain::capability_provenance::AssetClass;

/// The stated evaluation tolerance (ledger P3.05/P3.07: exact expected SOC
/// values are asserted within this explicitly stated tolerance).
const TOL: f64 = BESS_EVALUATION_TOLERANCE;

fn assert_close(actual: f64, expected: f64, context: &str) {
    assert!(
        (actual - expected).abs() <= TOL,
        "{context}: expected {expected}, got {actual} (tolerance {TOL})"
    );
}

/// The worked-example model (hand computation below): E_min = 0, E_max = 8
/// MWh, η_ch = η_dis = 0.9, Δt = 1 h (3600 s), P_ch,max = P_dis,max = 5 MW.
fn worked_example_model() -> BessModel {
    BessModel::new(BessModelConfig {
        e_min_mwh: ProvenancedValue::configured(0.0),
        e_max_mwh: ProvenancedValue::configured(8.0),
        p_ch_max_mw: ProvenancedValue::configured(5.0),
        p_dis_max_mw: ProvenancedValue::configured(5.0),
        eta_ch: 0.9,
        eta_dis: 0.9,
        interval_seconds: 3_600,
        soc_power_bands: None,
        temperature_derate: None,
        transitions: None,
        reserve: None,
        degradation: None,
    })
    .unwrap()
}

// --- P3.05: hand-computed 4-interval worked example ---------------------------

/// Hand computation (η_ch = η_dis = 0.9, Δt = 1 h, E in MWh):
///
/// ```text
///   E_0 = 0.0                                (SOC 0.0)
///   i0 CHARGE    P_ch = 5.0 : E_1 = 0 + 0.9·5·1        = 4.5   (SOC 0.5625)
///   i1 DISCHARGE P_dis = 0.9: E_2 = 4.5 − 0.9/0.9·1    = 3.5   (SOC 0.4375)
///   i2 IDLE                : E_3 = 3.5                          (SOC 0.4375)
///   i3 CHARGE    P_ch = 5.0 : E_4 = 3.5 + 0.9·5·1        = 8.0   (SOC 1.0, FULL)
/// ```
#[test]
fn p3_05_worked_example_reproduces_the_hand_computation() {
    let model = worked_example_model();
    let schedule = BessSchedule {
        initial_soc: 0.0,
        setpoints: vec![
            BessSetpoint::charge(5.0),
            BessSetpoint::discharge(0.9),
            BessSetpoint::idle(),
            BessSetpoint::charge(5.0),
        ],
        auxiliary_load: vec![AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured }; 4],
    };
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    assert!(
        evaluation.violations.is_empty(),
        "the worked example must satisfy every constraint; violations: {:?}",
        evaluation.violations
    );
    let expected_energies = [0.0, 4.5, 3.5, 3.5, 8.0];
    let expected_socs = [0.0, 0.5625, 0.4375, 0.4375, 1.0];
    for (boundary, expected) in expected_energies.iter().enumerate() {
        assert_close(
            evaluation.interval_energies_mwh[boundary],
            *expected,
            &format!("energy at boundary {boundary}"),
        );
    }
    for (boundary, expected) in expected_socs.iter().enumerate() {
        assert_close(
            evaluation.interval_socs[boundary],
            *expected,
            &format!("SOC at boundary {boundary}"),
        );
    }
    // The final interval ends FULL (SOC exactly 1.0, boundary-inclusive).
    assert_close(evaluation.interval_socs[4], 1.0, "final SOC is exactly full");
    // Provenance is propagated: the auxiliary load was known (configured zero).
    assert_eq!(evaluation.provenance.asset_class, AssetClass::Bess);
    assert_eq!(
        evaluation.provenance.auxiliary_load,
        AuxLoadState::Known { provenance: Provenance::Configured }
    );
}

/// Hand computation with the foundations §7.2 auxiliary term present on the
/// idle interval (P_aux = 0.5 MW):
///
/// ```text
///   E_3 = 3.5 − 0.5·1 = 3.0     (SOC 0.375)
///   E_4 = 3.0 + 4.5   = 7.5     (SOC 0.9375)
/// ```
#[test]
fn p3_05_auxiliary_term_is_applied_where_data_exists() {
    let model = worked_example_model();
    let schedule = BessSchedule {
        initial_soc: 0.0,
        setpoints: vec![
            BessSetpoint::charge(5.0),
            BessSetpoint::discharge(0.9),
            BessSetpoint::idle(),
            BessSetpoint::charge(5.0),
        ],
        auxiliary_load: vec![
            AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured },
            AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured },
            AuxLoadPoint::Known { p_aux_mw: 0.5, provenance: Provenance::Measured },
            AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured },
        ],
    };
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    assert!(evaluation.violations.is_empty(), "{:?}", evaluation.violations);
    assert_close(evaluation.interval_energies_mwh[3], 3.0, "E_3 with aux drain");
    assert_close(evaluation.interval_energies_mwh[4], 7.5, "E_4 with aux drain");
    assert_close(evaluation.interval_socs[3], 0.375, "SOC_3 with aux drain");
    // Weakest per-interval provenance is recorded (Measured < Configured).
    assert_eq!(
        evaluation.provenance.auxiliary_load,
        AuxLoadState::Known { provenance: Provenance::Measured }
    );
}

/// Where auxiliary data does NOT exist the term is omitted and the omission
/// is RECORDED — never a silent zero (ledger P3.05 step 1; foundations §7.2).
#[test]
fn p3_05_unknown_auxiliary_load_is_recorded_never_silently_zero() {
    let model = worked_example_model();
    let schedule = BessSchedule {
        initial_soc: 0.0,
        setpoints: vec![BessSetpoint::idle(); 4],
        auxiliary_load: vec![AuxLoadPoint::Unknown; 4],
    };
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    // The balance computes the no-aux trajectory (no value is invented).
    assert_close(evaluation.interval_energies_mwh[4], 0.0, "no invented aux drain");
    // ...and the omission is visible in the provenance record.
    match &evaluation.provenance.auxiliary_load {
        AuxLoadState::Unknown { intervals, note } => {
            assert_eq!(intervals, &[0, 1, 2, 3]);
            assert!(note.contains("understate"), "the note must state the drain risk");
            assert!(note.contains("P_aux"), "the note must name the missing term");
        }
        other => panic!("unknown auxiliary load must be recorded, got {other:?}"),
    }
}

/// Mixed known/unknown auxiliary data records exactly the unknown intervals.
#[test]
fn p3_05_mixed_auxiliary_provenance_records_the_unknown_intervals() {
    let model = worked_example_model();
    let schedule = BessSchedule {
        initial_soc: 0.0,
        setpoints: vec![BessSetpoint::idle(); 4],
        auxiliary_load: vec![
            AuxLoadPoint::Known { p_aux_mw: 0.2, provenance: Provenance::Configured },
            AuxLoadPoint::Unknown,
            AuxLoadPoint::Known { p_aux_mw: 0.2, provenance: Provenance::Configured },
            AuxLoadPoint::Unknown,
        ],
    };
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    match &evaluation.provenance.auxiliary_load {
        AuxLoadState::Unknown { intervals, .. } => assert_eq!(intervals, &[1, 3]),
        other => panic!("expected recorded unknown intervals, got {other:?}"),
    }
    // The known intervals applied their drain: E_1 = 0 − 0.2·1 = −0.2 → the
    // trajectory honestly shows the consequence of the declared value.
    assert_close(evaluation.interval_energies_mwh[1], -0.2, "E_1 with declared aux");
}

// --- P3.05: mandatory simultaneous charge/discharge post-solution check ------

#[test]
fn p3_05_simultaneous_charge_discharge_is_flagged_in_every_case() {
    let model = worked_example_model();
    // Within the relaxed per-interval set (both magnitudes inside limits and
    // the energy window) — flagged, not prevented, by the pure model.
    let schedule = BessSchedule {
        initial_soc: 0.5,
        setpoints: vec![
            BessSetpoint::charge(1.0),
            BessSetpoint { p_ch_mw: 1.0, p_dis_mw: 1.0 },
            BessSetpoint::idle(),
        ],
        auxiliary_load: vec![AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured }; 3],
    };
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    let flagged: Vec<_> = evaluation
        .violations
        .iter()
        .filter(|v| matches!(v, BessViolation::SimultaneousChargeDischarge { .. }))
        .collect();
    assert_eq!(flagged.len(), 1, "exactly the simultaneous interval is flagged");
    assert!(matches!(
        flagged[0],
        BessViolation::SimultaneousChargeDischarge { interval: 1, p_ch_mw: 1.0, p_dis_mw: 1.0 }
    ));

    // The public detector returns the same verdict for the same schedule —
    // the single implementation the Tier 1 MILP path (P3.17) and P3.21 call.
    assert_eq!(
        BessModel::detect_simultaneous_charge_discharge(&schedule.setpoints),
        vec![1]
    );

    // A clean schedule never trips the detector.
    let clean = vec![BessSetpoint::charge(5.0), BessSetpoint::discharge(0.9)];
    assert!(BessModel::detect_simultaneous_charge_discharge(&clean).is_empty());
}

// --- P3.06: degradation placeholder + reserve floor ---------------------------

#[test]
fn p3_06_degradation_defaults_to_zero_and_is_explicitly_a_placeholder_when_used() {
    let model = worked_example_model();

    // Not configured: cost zero, placeholder flag false (nothing is used).
    let schedule = BessSchedule {
        initial_soc: 0.0,
        setpoints: vec![BessSetpoint::charge(5.0), BessSetpoint::discharge(0.9)],
        auxiliary_load: vec![AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured }; 2],
    };
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    assert_eq!(
        evaluation.degradation,
        DegradationMetadata {
            configured: false,
            linear_cost_per_mwh_throughput: 0.0,
            throughput_mwh: 5.9,
            cost: 0.0,
            is_placeholder: false,
            placeholder_statement: flexcore_domain::bess_model::DEGRADATION_PLACEHOLDER_STATEMENT,
        }
    );

    // Configured (a rate of 2.0 cost-units/MWh — a configuration value, no
    // verified degradation law exists): throughput = (5.0 + 0.9)·1 h = 5.9
    // MWh; cost = 2.0 · 5.9 = 11.8. The placeholder metadata travels on the
    // output (visible in the type, not a comment).
    let mut config = worked_example_config();
    config.degradation = Some(DegradationConfig {
        linear_cost_per_mwh_throughput: 2.0,
        provenance: Provenance::Configured,
    });
    let model = BessModel::new(config).unwrap();
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    assert!(evaluation.degradation.is_placeholder);
    assert_eq!(evaluation.degradation.configured, true);
    assert_close(evaluation.degradation.throughput_mwh, 5.9, "throughput");
    assert_close(evaluation.degradation.cost, 11.8, "placeholder cost");
    assert!(
        evaluation
            .degradation
            .placeholder_statement
            .contains("not a measured degradation law"),
        "the placeholder statement must be explicit"
    );
}

fn worked_example_config() -> BessModelConfig {
    BessModelConfig {
        e_min_mwh: ProvenancedValue::configured(0.0),
        e_max_mwh: ProvenancedValue::configured(8.0),
        p_ch_max_mw: ProvenancedValue::configured(5.0),
        p_dis_max_mw: ProvenancedValue::configured(5.0),
        eta_ch: 0.9,
        eta_dis: 0.9,
        interval_seconds: 3_600,
        soc_power_bands: None,
        temperature_derate: None,
        transitions: None,
        reserve: Some(ReserveConfig {
            reserve_floor_soc: 0.3,
            provenance: Provenance::Configured,
        }),
        degradation: None,
    }
}

#[test]
fn p3_06_reserve_floor_narrows_the_usable_window_and_is_enforced() {
    // floor 0.3 over [0, 8] → usable from 2.4 MWh (configuration, not
    // physics).
    let model = BessModel::new(worked_example_config()).unwrap();
    assert_eq!(model.usable_energy_window(), Some((2.4, 8.0)));

    // A schedule that dips below the floor is rejected with the named
    // violation at the named boundary.
    let schedule = BessSchedule {
        initial_soc: 0.5,
        setpoints: vec![BessSetpoint::discharge(2.0), BessSetpoint::idle()],
        auxiliary_load: vec![AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured }; 2],
    };
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    // E_1 = 4 − 2/0.9 = 1.777... < 2.4 → floor violated at boundary 1.
    assert!(matches!(
        evaluation.violations.iter().find(|v| matches!(v, BessViolation::EnergyBelowReserveFloor { .. })),
        Some(BessViolation::EnergyBelowReserveFloor { boundary: 1, floor_mwh, .. } if (floor_mwh - 2.4).abs() <= TOL)
    ));
    // An idle schedule exactly ON the floor boundary is feasible
    // (boundary-inclusive): initial SOC 0.3 → E = 2.4 exactly.
    let schedule = BessSchedule {
        initial_soc: 0.3,
        setpoints: vec![BessSetpoint::idle(); 2],
        auxiliary_load: vec![AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured }; 2],
    };
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    assert!(evaluation.violations.is_empty(), "{:?}", evaluation.violations);
}

// --- P3.06 property: the reserve floor is never violated by an ACCEPTED
// --- schedule (deterministic splitmix64; the domain crate takes no test
// --- dependencies, so the generator mirrors the in-crate `test_rng`).

/// splitmix64 (Steele et al., 2014; public domain / CC0) — fixed seeds make
/// every property run deterministic and reproducible.
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
    fn f64_in(&mut self, low: f64, high: f64) -> f64 {
        let unit = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        low + unit * (high - low)
    }
}

#[test]
fn p3_06_property_reserve_floor_holds_in_every_accepted_schedule() {
    let model = BessModel::new(worked_example_config()).unwrap();
    let usable_min = 2.4;
    let e_max = 8.0;
    let mut rng = SplitMix64::new(0x5EED_2026_11_07);
    let mut accepted = 0usize;
    for _ in 0..400 {
        let setpoints: Vec<BessSetpoint> = (0..6)
            .map(|_| {
                // Roughly half the draws charge, half discharge.
                if rng.next_u64() % 2 == 0 {
                    BessSetpoint::charge(rng.f64_in(0.0, 5.0))
                } else {
                    BessSetpoint::discharge(rng.f64_in(0.0, 5.0))
                }
            })
            .collect();
        let schedule = BessSchedule {
            initial_soc: rng.f64_in(0.0, 1.0),
            auxiliary_load: vec![AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured }; 6],
            setpoints,
        };
        let evaluation = model.evaluate_schedule(&schedule).unwrap();
        if evaluation.violations.is_empty() {
            accepted += 1;
            for (boundary, &energy) in evaluation.interval_energies_mwh.iter().enumerate() {
                assert!(
                    energy >= usable_min - TOL,
                    "accepted schedule violated the reserve floor at boundary {boundary}: {energy} < {usable_min}"
                );
                assert!(
                    energy <= e_max + TOL,
                    "accepted schedule exceeded E_max at boundary {boundary}: {energy}"
                );
                let soc = evaluation.interval_socs[boundary];
                assert!(
                    (0.0..=1.0).contains(&soc),
                    "accepted schedule SOC outside [0,1] at boundary {boundary}: {soc}"
                );
            }
        } else {
            // Rejected schedules must name a concrete violation — the REJECT
            // policy never silently repairs anything.
            assert!(!evaluation.violations.is_empty());
        }
    }
    assert!(accepted > 0, "the property must exercise accepted schedules, not only rejections");
}

#[test]
fn p3_07_property_no_accepted_schedule_leaves_the_energy_window() {
    let model = worked_example_model();
    let mut rng = SplitMix64::new(0xB355_0001);
    let mut accepted = 0usize;
    for _ in 0..400 {
        let setpoints: Vec<BessSetpoint> = (0..8)
            .map(|_| {
                if rng.next_u64() % 3 == 0 {
                    BessSetpoint::charge(rng.f64_in(0.0, 5.0))
                } else if rng.next_u64() % 2 == 0 {
                    BessSetpoint::discharge(rng.f64_in(0.0, 5.0))
                } else {
                    BessSetpoint::idle()
                }
            })
            .collect();
        let schedule = BessSchedule {
            initial_soc: rng.f64_in(0.0, 1.0),
            auxiliary_load: vec![AuxLoadPoint::Unknown; 8],
            setpoints,
        };
        let evaluation = model.evaluate_schedule(&schedule).unwrap();
        if evaluation.violations.is_empty() {
            accepted += 1;
            for (boundary, &energy) in evaluation.interval_energies_mwh.iter().enumerate() {
                assert!(
                    energy >= 0.0 - TOL && energy <= 8.0 + TOL,
                    "accepted schedule left [E_min, E_max] at boundary {boundary}: {energy}"
                );
            }
        }
    }
    assert!(accepted > 0);
}

// --- P3.07: edge fixtures ------------------------------------------------------

#[test]
fn p3_07_fixture_soc_exactly_at_e_min() {
    let model = worked_example_model();
    // Idle at the floor is feasible (boundary-inclusive).
    let schedule = BessSchedule {
        initial_soc: 0.0,
        setpoints: vec![BessSetpoint::idle(); 2],
        auxiliary_load: vec![AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured }; 2],
    };
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    assert!(evaluation.violations.is_empty(), "{:?}", evaluation.violations);
    assert_close(evaluation.interval_socs[2], 0.0, "SOC stays exactly at E_min");

    // Discharging from empty is REJECTED (no excursion, no clipping): the
    // trajectory shows the below-min energy and the named violation.
    let schedule = BessSchedule {
        initial_soc: 0.0,
        setpoints: vec![BessSetpoint::discharge(0.5), BessSetpoint::idle()],
        auxiliary_load: vec![AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured }; 2],
    };
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    assert!(matches!(
        evaluation.violations.iter().find(|v| matches!(v, BessViolation::EnergyBelowMin { .. })),
        Some(BessViolation::EnergyBelowMin { boundary: 1, .. })
    ));
    // The REJECT policy keeps the declared setpoint's consequence VISIBLE:
    // the trajectory dips below E_min unclipped and the violation names the
    // boundary (no silent clipping, no silent repair).
    assert!(
        evaluation.interval_socs[1] < 0.0,
        "the below-min consequence must stay visible (REJECT, not clip)"
    );
}

#[test]
fn p3_07_fixture_soc_exactly_at_e_max_charge_when_full_is_rejected_not_clipped() {
    let model = worked_example_model();
    // Idle while full is feasible (boundary-inclusive).
    let schedule = BessSchedule {
        initial_soc: 1.0,
        setpoints: vec![BessSetpoint::idle(); 2],
        auxiliary_load: vec![AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured }; 2],
    };
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    assert!(evaluation.violations.is_empty(), "{:?}", evaluation.violations);
    assert_close(evaluation.interval_socs[2], 1.0, "SOC stays exactly at E_max");

    // Charging when full: the recorded policy is REJECT — the trajectory
    // keeps the DECLARED setpoint's consequence (8 + 0.9·0.5 = 8.45, unclipped)
    // and the violation names the boundary.
    let schedule = BessSchedule {
        initial_soc: 1.0,
        setpoints: vec![BessSetpoint::charge(0.5), BessSetpoint::idle()],
        auxiliary_load: vec![AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured }; 2],
    };
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    assert_close(
        evaluation.interval_energies_mwh[1],
        8.45,
        "declared setpoint is never clipped (REJECT policy)",
    );
    assert!(matches!(
        evaluation.violations.iter().find(|v| matches!(v, BessViolation::EnergyAboveMax { .. })),
        Some(BessViolation::EnergyAboveMax { boundary: 1, energy_mwh, e_max_mwh: 8.0 }
            if (energy_mwh - 8.45).abs() <= TOL)
    ));
}

#[test]
fn p3_07_fixture_one_interval_from_full_while_discharging() {
    let model = worked_example_model();
    // E_1 = 8 − 1/0.9 = 6.888... — within the window, no violation.
    let schedule = BessSchedule {
        initial_soc: 1.0,
        setpoints: vec![BessSetpoint::discharge(1.0)],
        auxiliary_load: vec![AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured }],
    };
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    assert!(evaluation.violations.is_empty(), "{:?}", evaluation.violations);
    assert_close(
        evaluation.interval_energies_mwh[1],
        8.0 - 1.0 / 0.9,
        "one interval from full while discharging",
    );
    assert_close(evaluation.interval_socs[1], (8.0 - 1.0 / 0.9) / 8.0, "SOC after the interval");
}

#[test]
fn p3_07_fixture_one_interval_from_empty_while_charging() {
    let model = worked_example_model();
    let schedule = BessSchedule {
        initial_soc: 0.0,
        setpoints: vec![BessSetpoint::charge(5.0)],
        auxiliary_load: vec![AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured }],
    };
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    assert!(evaluation.violations.is_empty(), "{:?}", evaluation.violations);
    assert_close(evaluation.interval_energies_mwh[1], 4.5, "one interval from empty while charging");
    assert_close(evaluation.interval_socs[1], 0.5625, "SOC after the interval");
}

#[test]
fn p3_07_fixture_charge_to_discharge_transition_respects_dwell_where_configured() {
    let model = worked_example_model();
    // Without a configured dwell the transition is free: i0 charge 2 MW
    // (E: 4 → 5.8), i1 discharge 1 MW (E: 5.8 → 4.688...).
    let setpoints = vec![BessSetpoint::charge(2.0), BessSetpoint::discharge(1.0)];
    let schedule = BessSchedule {
        initial_soc: 0.5,
        auxiliary_load: vec![AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured }; 2],
        setpoints: setpoints.clone(),
    };
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    assert!(evaluation.violations.is_empty(), "{:?}", evaluation.violations);
    assert_close(evaluation.interval_energies_mwh[1], 5.8, "charge interval");
    assert_close(evaluation.interval_energies_mwh[2], 5.8 - 1.0 / 0.9, "discharge interval");

    // With min dwell 2 intervals the one-interval charge episode is rejected.
    let mut config = worked_example_config();
    config.reserve = None;
    config.transitions = Some(TransitionConstraints {
        ramp_limit_mw_per_minute: None,
        min_mode_dwell_intervals: Some(2),
    });
    let model = BessModel::new(config).unwrap();
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    assert!(matches!(
        evaluation.violations.iter().find(|v| matches!(v, BessViolation::DwellViolation { .. })),
        Some(BessViolation::DwellViolation {
            interval: 1,
            mode: BessOperatingMode::Charge,
            held_intervals: 1,
            required_intervals: 2
        })
    ));

    // An episode truncated by the horizon end is exempt.
    let schedule = BessSchedule {
        initial_soc: 0.5,
        auxiliary_load: vec![AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured }; 1],
        setpoints: vec![BessSetpoint::charge(2.0)],
    };
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    assert!(evaluation.violations.is_empty(), "{:?}", evaluation.violations);
}

#[test]
fn p3_07_ramp_limit_enforced_between_intervals_where_configured() {
    let mut config = worked_example_config();
    config.reserve = None;
    // 0.05 MW/min over a 3600 s interval = 3 MW per interval.
    config.transitions = Some(TransitionConstraints {
        ramp_limit_mw_per_minute: Some(0.05),
        min_mode_dwell_intervals: None,
    });
    let model = BessModel::new(config).unwrap();
    let schedule = BessSchedule {
        initial_soc: 0.5,
        auxiliary_load: vec![AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured }; 2],
        setpoints: vec![BessSetpoint::charge(1.0), BessSetpoint::discharge(3.0)],
    };
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    // |−1 − 3| = 4 MW > 3 MW per interval.
    assert!(matches!(
        evaluation.violations.iter().find(|v| matches!(v, BessViolation::RampViolation { .. })),
        Some(BessViolation::RampViolation {
            interval: 1,
            net_change_mw,
            limit_mw_per_interval
        } if (net_change_mw - 4.0).abs() <= TOL && (limit_mw_per_interval - 3.0).abs() <= TOL)
    ));
    // The first interval is exempt (no prior setpoint exists).
    let schedule = BessSchedule {
        initial_soc: 0.5,
        auxiliary_load: vec![AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured }],
        setpoints: vec![BessSetpoint::charge(2.0)],
    };
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    assert!(evaluation.violations.is_empty(), "{:?}", evaluation.violations);
}

// --- P3.05 step 3: limits only where data is available ------------------------

#[test]
fn p3_05_soc_dependent_power_limits_apply_only_where_configured() {
    // Without a band table: full limits everywhere, dimension recorded
    // unknown.
    let model = worked_example_model();
    let schedule = BessSchedule {
        initial_soc: 0.05,
        auxiliary_load: vec![AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured }],
        setpoints: vec![BessSetpoint::discharge(0.3)],
    };
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    assert!(evaluation.violations.is_empty(), "{:?}", evaluation.violations);
    assert!(evaluation
        .provenance
        .unknown_dimensions
        .iter()
        .any(|d| d.dimension == "SOC_DEPENDENT_POWER_LIMITS"));

    // With a band table: the low-SOC band narrows the discharge limit.
    let mut config = worked_example_config();
    config.reserve = None;
    config.soc_power_bands = Some(vec![
        SocPowerBand { soc_upper: 0.2, p_ch_max_mw: None, p_dis_max_mw: Some(1.0) },
        SocPowerBand { soc_upper: 1.0, p_ch_max_mw: None, p_dis_max_mw: None },
    ]);
    let model = BessModel::new(config).unwrap();
    let schedule = BessSchedule {
        initial_soc: 0.1,
        auxiliary_load: vec![AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured }],
        setpoints: vec![BessSetpoint::discharge(2.0)],
    };
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    assert!(matches!(
        evaluation.violations.first(),
        Some(BessViolation::DischargePowerAboveLimit { limit_mw, .. } if (limit_mw - 1.0).abs() <= TOL)
    ));
}

#[test]
fn p3_05_temperature_derate_applies_only_where_data_exists() {
    // Configured, known provenance: the derate narrows both limits.
    let mut config = worked_example_config();
    config.reserve = None;
    config.temperature_derate = Some(TemperatureDerate {
        provenance: Provenance::Measured,
        derate_mw_per_interval: vec![1.0],
    });
    let model = BessModel::new(config).unwrap();
    let schedule = BessSchedule {
        initial_soc: 0.5,
        auxiliary_load: vec![AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured }],
        setpoints: vec![BessSetpoint::discharge(4.5)],
    };
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    assert!(matches!(
        evaluation.violations.first(),
        Some(BessViolation::DischargePowerAboveLimit { limit_mw, .. } if (limit_mw - 4.0).abs() <= TOL)
    ));

    // UNKNOWN provenance: no derate applied (no credit taken from unknown
    // data), and the absence is recorded.
    let mut config = worked_example_config();
    config.temperature_derate = Some(TemperatureDerate {
        provenance: Provenance::Unknown,
        derate_mw_per_interval: vec![1.0],
    });
    let model = BessModel::new(config).unwrap();
    let schedule = BessSchedule {
        initial_soc: 0.5,
        auxiliary_load: vec![AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured }],
        setpoints: vec![BessSetpoint::discharge(3.0)],
    };
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    assert!(evaluation.violations.is_empty(), "{:?}", evaluation.violations);
    assert!(evaluation
        .provenance
        .unknown_dimensions
        .iter()
        .any(|d| d.dimension == "TEMPERATURE_HEALTH"));
}

#[test]
fn p3_05_bms_constraints_are_always_recorded_unknown() {
    // No BMS data exists anywhere in the inputs; the model never invents
    // values and never claims the dimension is covered.
    let model = worked_example_model();
    let schedule = BessSchedule {
        initial_soc: 0.5,
        auxiliary_load: vec![AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured }],
        setpoints: vec![BessSetpoint::idle()],
    };
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    let bms = evaluation
        .provenance
        .unknown_dimensions
        .iter()
        .find(|d| d.dimension == "BMS_CONSTRAINTS")
        .expect("BMS constraints must always be recorded as an unknown dimension");
    assert!(bms.note.contains("no BMS constraint data"));
}

// --- entry-point errors ---------------------------------------------------------

#[test]
fn evaluation_rejects_malformed_schedules_with_named_errors() {
    let model = worked_example_model();
    // Aux/setpoint length mismatch.
    let schedule = BessSchedule {
        initial_soc: 0.5,
        auxiliary_load: vec![AuxLoadPoint::Unknown; 1],
        setpoints: vec![BessSetpoint::idle(); 2],
    };
    assert!(matches!(
        model.evaluate_schedule(&schedule),
        Err(flexcore_domain::bess_model::BessModelError::ScheduleLengthMismatch { .. })
    ));

    // Initial SOC outside [0,1].
    let schedule = BessSchedule {
        initial_soc: 1.5,
        auxiliary_load: vec![AuxLoadPoint::Unknown],
        setpoints: vec![BessSetpoint::idle()],
    };
    assert!(matches!(
        model.evaluate_schedule(&schedule),
        Err(flexcore_domain::bess_model::BessModelError::InitialSocOutOfRange { .. })
    ));

    // Initial SOC exactly 0 and exactly 1 are valid (boundary-inclusive).
    for soc in [0.0, 1.0] {
        let schedule = BessSchedule {
            initial_soc: soc,
            auxiliary_load: vec![AuxLoadPoint::Unknown],
            setpoints: vec![BessSetpoint::idle()],
        };
        assert!(model.evaluate_schedule(&schedule).is_ok(), "SOC {soc} must be accepted");
    }

    // Non-finite setpoint.
    let schedule = BessSchedule {
        initial_soc: 0.5,
        auxiliary_load: vec![AuxLoadPoint::Unknown],
        setpoints: vec![BessSetpoint { p_ch_mw: f64::NAN, p_dis_mw: 0.0 }],
    };
    assert!(matches!(
        model.evaluate_schedule(&schedule),
        Err(flexcore_domain::bess_model::BessModelError::NonFiniteSetpoint { interval: 0, field: "p_ch_mw", .. })
    ));

    // Negative auxiliary magnitude.
    let schedule = BessSchedule {
        initial_soc: 0.5,
        auxiliary_load: vec![AuxLoadPoint::Known { p_aux_mw: -0.1, provenance: Provenance::Configured }],
        setpoints: vec![BessSetpoint::idle()],
    };
    assert!(matches!(
        model.evaluate_schedule(&schedule),
        Err(flexcore_domain::bess_model::BessModelError::InvalidAuxiliaryLoad { interval: 0, .. })
    ));

    // Negative charge magnitude: reported as a violation, contributes zero
    // to the balance (REJECT policy — reported, never silent).
    let schedule = BessSchedule {
        initial_soc: 0.5,
        auxiliary_load: vec![AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured }],
        setpoints: vec![BessSetpoint { p_ch_mw: -1.0, p_dis_mw: 0.0 }],
    };
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    assert!(matches!(
        evaluation.violations.first(),
        Some(BessViolation::NegativeChargePower { interval: 0, p_ch_mw: -1.0 })
    ));
    assert_close(
        evaluation.interval_energies_mwh[1],
        4.0,
        "a negative magnitude contributes zero to the balance",
    );
}

#[test]
fn negative_power_limit_setpoints_are_reported_not_clipped() {
    let model = worked_example_model();
    let schedule = BessSchedule {
        initial_soc: 0.0,
        auxiliary_load: vec![AuxLoadPoint::Known { p_aux_mw: 0.0, provenance: Provenance::Configured }],
        setpoints: vec![BessSetpoint::charge(6.0)], // above the 5 MW limit
    };
    let evaluation = model.evaluate_schedule(&schedule).unwrap();
    // The declared over-limit setpoint is used as declared in the balance:
    // E_1 = 0.9·6 = 5.4 (shows the consequence; REJECT policy).
    assert_close(evaluation.interval_energies_mwh[1], 5.4, "over-limit setpoint unclipped");
    assert!(matches!(
        evaluation.violations.first(),
        Some(BessViolation::ChargePowerAboveLimit { interval: 0, p_ch_mw: 6.0, limit_mw: 5.0 })
    ));
}
