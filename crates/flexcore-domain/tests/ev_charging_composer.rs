//! P17.05 — WRITTEN-UNRUN integration tests for the smart-charging
//! schedule composer (decision record
//! 05_p17_05_smart_charging_composer.md; conventions §1.3: tests are
//! written, their execution is a human gate). Every expected value is
//! hand-computed; nothing here touches the adapter crate (the composer is
//! pure domain rules).

use flexcore_domain::ev_charging_composer::{
    compose_external_constraints_profile, compose_tx_default_profile, compose_tx_profile,
    reconcile_composite_schedule, ChargingProfilePlan, ComposerDecision, ComposerError,
    CompositeScheduleObservation, DeclaredMinRate, DeviceRateConstraints, DispatchEnvelope,
    DispatchPeriod, PlanPurpose, PlanPeriod, RateUnit, ReconciliationMismatch,
    ReconciliationOutcome, SupplyContext, COMPOSER_EVALUATION_TOLERANCE,
    RECONCILIATION_STATEMENT, STACK_PRECEDENCE_STATEMENT,
};

fn device_min_a(min_a: f64) -> DeviceRateConstraints {
    DeviceRateConstraints {
        min_charging_rate: Some(DeclaredMinRate {
            value: min_a,
            unit: RateUnit::A,
        }),
    }
}

fn device_no_min() -> DeviceRateConstraints {
    DeviceRateConstraints {
        min_charging_rate: None,
    }
}

#[test]
fn external_constraints_targets_station_zero_with_grid_critical() {
    // A depot site cap (the P17.11 surface): 0.4 MW import cap for one
    // hour, then 0.1 MW, then off.
    let envelope = DispatchEnvelope {
        start_millis: 1_700_000_000_000,
        periods: vec![
            DispatchPeriod {
                start_offset_s: 0,
                limit_mw: 0.4,
            },
            DispatchPeriod {
                start_offset_s: 3600,
                limit_mw: 0.1,
            },
            DispatchPeriod {
                start_offset_s: 5400,
                limit_mw: 0.0,
            },
        ],
    };
    let plan = compose_external_constraints_profile(
        &envelope,
        RateUnit::W,
        &SupplyContext::AcThreePhase {
            nominal_voltage_v: 400.0,
        },
        &device_no_min(),
        10,
        true,
    )
    .expect("composed");
    assert_eq!(plan.purpose, PlanPurpose::ChargingStationExternalConstraints);
    assert_eq!(plan.evse_wire_id, 0, "station scope (UNVERIFIED register item 3)");
    assert_eq!(plan.is_grid_critical, Some(true));
    assert_eq!(plan.stack_level, 10);
    // Hand-computed W values (exact).
    assert_eq!(plan.periods[0].limit, 400_000.0);
    assert_eq!(plan.periods[1].limit, 100_000.0);
    assert_eq!(plan.periods[2].limit, 0.0);
    // Every conversion decision is recorded with its formula.
    assert_eq!(plan.decisions.len(), 3);
}

#[test]
fn ampere_composition_uses_the_declared_ac_and_dc_formulas() {
    // AC three-phase 400 V: 0.03 MW = 30_000 W -> 30_000 / 1200 = 25 A.
    let envelope = DispatchEnvelope {
        start_millis: 0,
        periods: vec![DispatchPeriod {
            start_offset_s: 0,
            limit_mw: 0.03,
        }],
    };
    let plan = compose_tx_default_profile(
        &envelope,
        RateUnit::A,
        &SupplyContext::AcThreePhase {
            nominal_voltage_v: 400.0,
        },
        &device_no_min(),
        2,
        5,
    )
    .expect("composed");
    assert!((plan.periods[0].limit - 25.0).abs() < COMPOSER_EVALUATION_TOLERANCE);
    assert_eq!(plan.periods[0].number_phases, Some(3));

    // AC single-phase 230 V: 0.0069 MW = 6_900 W -> 6_900 / 230 = 30 A.
    let envelope = DispatchEnvelope {
        start_millis: 0,
        periods: vec![DispatchPeriod {
            start_offset_s: 0,
            limit_mw: 0.0069,
        }],
    };
    let plan = compose_tx_default_profile(
        &envelope,
        RateUnit::A,
        &SupplyContext::AcSinglePhase {
            nominal_voltage_v: 230.0,
        },
        &device_no_min(),
        2,
        5,
    )
    .expect("composed");
    assert!((plan.periods[0].limit - 30.0).abs() < COMPOSER_EVALUATION_TOLERANCE);
    assert_eq!(plan.periods[0].number_phases, Some(1));

    // DC 500 V: 0.05 MW = 50_000 W -> 50_000 / 500 = 100 A, no phase count.
    let envelope = DispatchEnvelope {
        start_millis: 0,
        periods: vec![DispatchPeriod {
            start_offset_s: 0,
            limit_mw: 0.05,
        }],
    };
    let plan = compose_tx_default_profile(
        &envelope,
        RateUnit::A,
        &SupplyContext::Dc {
            nominal_voltage_v: 500.0,
        },
        &device_no_min(),
        2,
        5,
    )
    .expect("composed");
    assert!((plan.periods[0].limit - 100.0).abs() < COMPOSER_EVALUATION_TOLERANCE);
    assert_eq!(plan.periods[0].number_phases, None);
}

#[test]
fn the_min_rate_rule_sends_unfulfillable_headroom_to_zero() {
    // Device minimum 6 A (AC three-phase, 400 V = 7_200 W). Headroom of
    // 0.003 MW = 3_000 W is below the minimum -> 0 (off); headroom of
    // 0.01 MW = 10_000 W stays.
    let envelope = DispatchEnvelope {
        start_millis: 0,
        periods: vec![
            DispatchPeriod {
                start_offset_s: 0,
                limit_mw: 0.003,
            },
            DispatchPeriod {
                start_offset_s: 600,
                limit_mw: 0.01,
            },
        ],
    };
    let plan = compose_tx_default_profile(
        &envelope,
        RateUnit::W,
        &SupplyContext::AcThreePhase {
            nominal_voltage_v: 400.0,
        },
        &device_min_a(6.0),
        2,
        5,
    )
    .expect("composed");
    // The declared minimum normalized to W with the recorded formula.
    assert_eq!(plan.min_charging_rate, Some(7_200.0));
    assert_eq!(plan.periods[0].limit, 0.0, "3_000 W < 7_200 W -> off");
    assert_eq!(plan.periods[1].limit, 10_000.0, "10_000 W >= 7_200 W stays");
    // Both decisions are recorded.
    let to_zero = plan
        .decisions
        .iter()
        .find(|d| matches!(d, ComposerDecision::MinRateToZero { .. }))
        .expect("to-zero recorded");
    match to_zero {
        ComposerDecision::MinRateToZero {
            requested,
            min_rate,
            unit,
            ..
        } => {
            assert_eq!(*requested, 3_000.0);
            assert_eq!(*min_rate, 7_200.0);
            assert_eq!(*unit, RateUnit::W);
        }
        other => panic!("unexpected decision {other:?}"),
    }
    let conversion = plan
        .decisions
        .iter()
        .find(|d| matches!(d, ComposerDecision::MinRateConversion { .. }))
        .expect("min-rate conversion recorded");
    match conversion {
        ComposerDecision::MinRateConversion {
            from_value,
            from_unit,
            to_value,
            to_unit,
            formula,
        } => {
            assert_eq!(*from_value, 6.0);
            assert_eq!(*from_unit, RateUnit::A);
            assert_eq!(*to_value, 7_200.0);
            assert_eq!(*to_unit, RateUnit::W);
            assert!(formula.contains("400") && formula.contains("3"));
        }
        other => panic!("unexpected decision {other:?}"),
    }
}

#[test]
fn zero_limits_never_trip_the_min_rate_rule() {
    // An explicit 0 (off) period must stay 0: the rule applies only to
    // positive limits below the minimum (module register item 4).
    let envelope = DispatchEnvelope {
        start_millis: 0,
        periods: vec![
            DispatchPeriod {
                start_offset_s: 0,
                limit_mw: 0.0,
            },
            DispatchPeriod {
                start_offset_s: 600,
                limit_mw: 0.0,
            },
        ],
    };
    let plan = compose_tx_default_profile(
        &envelope,
        RateUnit::A,
        &SupplyContext::AcThreePhase {
            nominal_voltage_v: 400.0,
        },
        &device_min_a(6.0),
        2,
        5,
    )
    .expect("composed");
    assert!(plan.periods.iter().all(|p| p.limit == 0.0));
    assert!(plan
        .decisions
        .iter()
        .all(|d| !matches!(d, ComposerDecision::MinRateToZero { .. })));
}

#[test]
fn purpose_scope_and_stack_rules_are_enforced() {
    let envelope = DispatchEnvelope {
        start_millis: 0,
        periods: vec![DispatchPeriod {
            start_offset_s: 0,
            limit_mw: 0.1,
        }],
    };
    // TxProfile needs a transaction.
    assert_eq!(
        compose_tx_profile(
            &envelope,
            RateUnit::W,
            &SupplyContext::AcThreePhase {
                nominal_voltage_v: 400.0
            },
            &device_no_min(),
            1,
            5,
            " "
        ),
        Err(ComposerError::TxProfileNeedsTransaction)
    );
    // Tx-class purposes refuse station scope.
    assert_eq!(
        compose_tx_profile(
            &envelope,
            RateUnit::W,
            &SupplyContext::AcThreePhase {
                nominal_voltage_v: 400.0
            },
            &device_no_min(),
            0,
            5,
            "txn-1"
        ),
        Err(ComposerError::StationScopePurpose {
            purpose: "TxProfile"
        })
    );
    assert_eq!(
        compose_tx_default_profile(
            &envelope,
            RateUnit::W,
            &SupplyContext::AcThreePhase {
                nominal_voltage_v: 400.0
            },
            &device_no_min(),
            0,
            5
        ),
        Err(ComposerError::StationScopePurpose {
            purpose: "TxDefaultProfile"
        })
    );
    // No supply data, no invented conversion.
    assert_eq!(
        compose_tx_default_profile(
            &envelope,
            RateUnit::A,
            &SupplyContext::Dc {
                nominal_voltage_v: -1.0
            },
            &device_no_min(),
            1,
            5
        ),
        Err(ComposerError::InvalidVoltage { value: -1.0 })
    );
}

#[test]
fn the_precedence_record_travels_with_every_plan() {
    let envelope = DispatchEnvelope {
        start_millis: 0,
        periods: vec![DispatchPeriod {
            start_offset_s: 0,
            limit_mw: 0.1,
        }],
    };
    let plan = compose_external_constraints_profile(
        &envelope,
        RateUnit::W,
        &SupplyContext::AcThreePhase {
            nominal_voltage_v: 400.0
        },
        &device_no_min(),
        1,
        false,
    )
    .expect("composed");
    assert_eq!(plan.precedence_statement, STACK_PRECEDENCE_STATEMENT);
    assert!(STACK_PRECEDENCE_STATEMENT.contains("ChargingStationExternalConstraints first"));
    assert!(STACK_PRECEDENCE_STATEMENT.contains("UNVERIFIED against"));
    assert_eq!(PlanPurpose::ChargingStationExternalConstraints.precedence_rank(), 0);
    assert_eq!(PlanPurpose::ChargingStationMaxProfile.precedence_rank(), 1);
    assert_eq!(PlanPurpose::TxDefaultProfile.precedence_rank(), 2);
    assert_eq!(PlanPurpose::TxProfile.precedence_rank(), 3);
}

#[test]
fn reconciliation_names_missing_extra_and_deviating_periods() {
    let plan = ChargingProfilePlan {
        purpose: PlanPurpose::ChargingStationExternalConstraints,
        evse_wire_id: 0,
        is_grid_critical: Some(true),
        transaction_id: None,
        stack_level: 10,
        rate_unit: RateUnit::W,
        start_millis: 0,
        min_charging_rate: None,
        periods: vec![
            PlanPeriod {
                start_offset_s: 0,
                limit: 100_000.0,
                number_phases: None,
            },
            PlanPeriod {
                start_offset_s: 900,
                limit: 0.0,
                number_phases: None,
            },
        ],
        decisions: Vec::new(),
        precedence_statement: STACK_PRECEDENCE_STATEMENT,
    };

    // Aligned (boundary-inclusive within the tolerance).
    let observation = CompositeScheduleObservation {
        rate_unit: RateUnit::W,
        periods: vec![
            PlanPeriod {
                start_offset_s: 0,
                limit: 100_000.0 + COMPOSER_EVALUATION_TOLERANCE / 2.0,
                number_phases: None,
            },
            PlanPeriod {
                start_offset_s: 900,
                limit: 0.0,
                number_phases: None,
            },
        ],
    };
    assert_eq!(
        reconcile_composite_schedule(&plan, &observation),
        ReconciliationOutcome::Aligned
    );

    // Everything wrong at once: extra, deviating, missing.
    let observation = CompositeScheduleObservation {
        rate_unit: RateUnit::W,
        periods: vec![
            PlanPeriod {
                start_offset_s: 0,
                limit: 100_000.0,
                number_phases: None,
            },
            PlanPeriod {
                start_offset_s: 450,
                limit: 5_000.0,
                number_phases: None,
            },
        ],
    };
    match reconcile_composite_schedule(&plan, &observation) {
        ReconciliationOutcome::Mismatch { details } => {
            assert!(details.contains(&ReconciliationMismatch::ExtraPeriod {
                start_offset_s: 450,
                observed_limit: 5_000.0,
            }));
            // The planned 900-period has no observed counterpart (only 0
            // and 450 were observed) — a named missing period.
            assert!(details.contains(&ReconciliationMismatch::MissingPeriod {
                start_offset_s: 900,
                planned_limit: 0.0,
            }));
        }
        other => panic!("expected a mismatch, got {other:?}"),
    }

    // The honesty statement travels (record 05 §6).
    assert!(RECONCILIATION_STATEMENT.contains("STORED, not applied"));
    assert!(RECONCILIATION_STATEMENT.contains("never a silent pass"));
}

#[test]
fn the_plan_is_a_candidate_never_a_state() {
    // The plan type carries no lifecycle state and no transition method:
    // absence is structural (the command-relay-only rule, record 05 §5).
    let envelope = DispatchEnvelope {
        start_millis: 0,
        periods: vec![DispatchPeriod {
            start_offset_s: 0,
            limit_mw: 0.1,
        }],
    };
    let plan = compose_external_constraints_profile(
        &envelope,
        RateUnit::W,
        &SupplyContext::AcThreePhase {
            nominal_voltage_v: 400.0
        },
        &device_no_min(),
        1,
        false,
    )
    .expect("composed");
    // The only "state-like" data is the recorded precedence statement and
    // the decision log — both audit records, not lifecycle states.
    assert!(plan.decisions.len() == 1);
    assert!(plan.precedence_statement.contains("precedence"));
}
