// P3.02/P3.03/P3.04 — Capability boundary test suite (integration, from
// outside the crate).
//
// Covers the Phase 3 capability boundary end to end, from OUTSIDE
// flexcore-domain:
//
// - P3.02: the unknown-narrows rule per asset class (BESS, inverter P-Q,
//   flexible load, EV fleet) — all-unknown ramp/temperature limits produce a
//   strictly smaller feasible set than measured limits, unknown never widens,
//   and provenance is visible in the envelope/schedule record types.
// - P3.03: scheduling-grid fixtures on the same horizon produce identical
//   interval boundaries; boundary timestamps follow start-inclusive /
//   end-exclusive semantics; the aggregation quality-precedence table holds;
//   rollover preserves the prior revision.
// - P3.04: every malformed capability input is rejected with its NAMED error;
//   valid boundary inputs (SOC exactly 0 and exactly 1, zero powers, equal
//   energy bounds) are accepted; error messages are audit-safe.
//
// Decision record:
// `ledger/phase-03-flexibility-scheduling/01_04_capability_contract.md`.

use flexcore_contracts::capability::{
    CAPABILITY_SCHEMA_VERSION, ConstraintExpression, ConstraintKind, ConstraintSignConvention,
    ConstraintUnit, DeclaredConstraint, Provenance, TimeIndexedFeasibleSet,
};
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{EventTime, ValidFrom, ValidUntil, ValidityWindow};

use flexcore_domain::capability_provenance::{
    AssetClass, BoundInterval, EnvelopeProvenance, NarrowedSet, ProvenancedLimit,
    ProvenanceRuleError, ScheduleProvenanceEntry, narrow_interval, strictly_narrower,
};
use flexcore_domain::capability_validate::{
    CapabilityValidationRule, validate_active_power_sign, validate_bess_capability,
    validate_charge_power, validate_discharge_power, validate_energy_window,
    validate_ramp_rate, validate_reactive_power_sign, validate_soc, validate_unit,
};
use flexcore_domain::scheduling_time::{
    DEFAULT_HORIZON_MILLIS, DEFAULT_INTERVAL_MILLIS, RolloverOutcome, SchedulingGrid,
    SchedulingGridConfig, ScheduleRevisionRef, aggregate_bucket_quality,
    aggregate_subinterval_values, plan_horizon_rollover, ValueAggregation,
};

const ASSET: &str = "0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8b";
const START_MILLIS: i64 = 1_700_000_000_000;

fn asset_id() -> FlexUuid {
    FlexUuid::parse(ASSET).unwrap()
}

fn window() -> ValidityWindow {
    ValidityWindow::new(
        ValidFrom::from_unix_millis(0).unwrap(),
        ValidUntil::from_unix_millis(3_600_000).unwrap(),
    )
    .unwrap()
}

fn interval(lower: f64, upper: f64) -> BoundInterval {
    BoundInterval::new(Some(lower), Some(upper)).unwrap()
}

// ===========================================================================
// P3.02 — unknown narrows, per asset class
// ===========================================================================

#[test]
fn unknown_ramp_narrows_strictly_for_every_asset_class() {
    // Ledger P3.02 step 4: an asset with all-unknown ramp limits produces a
    // strictly smaller feasible set than the same asset with measured limits.
    let measured_ramp = interval(0.0, 5.0); // MW/min
    for class in AssetClass::ALL {
        let measured = narrow_interval(class, ConstraintKind::RampRate, Provenance::Measured, measured_ramp);
        let unknown = narrow_interval(class, ConstraintKind::RampRate, Provenance::Unknown, measured_ramp);
        assert_eq!(
            measured,
            NarrowedSet::Interval { lower: Some(0.0), upper: Some(5.0) },
            "measured ramp is used as declared for {}",
            class.name()
        );
        assert_eq!(
            unknown,
            NarrowedSet::Interval { lower: Some(0.0), upper: Some(0.0) },
            "unknown ramp narrows to the recorded default for {}",
            class.name()
        );
        assert!(
            strictly_narrower(&measured_ramp, &unknown),
            "unknown ramp must be strictly smaller than measured for {}",
            class.name()
        );
    }
}

#[test]
fn unknown_temperature_health_narrows_strictly_for_every_asset_class() {
    // Ledger P3.02 step 4: all-unknown temperature limits vs measured limits.
    let measured_health = interval(0.0, 0.8); // dimensionless availability factor
    for class in AssetClass::ALL {
        let measured = narrow_interval(
            class,
            ConstraintKind::TemperatureHealth,
            Provenance::Measured,
            measured_health,
        );
        let unknown = narrow_interval(
            class,
            ConstraintKind::TemperatureHealth,
            Provenance::Unknown,
            measured_health,
        );
        assert_eq!(measured, NarrowedSet::Interval { lower: Some(0.0), upper: Some(0.8) });
        assert_eq!(unknown, NarrowedSet::Interval { lower: Some(0.0), upper: Some(0.0) });
        assert!(strictly_narrower(&measured_health, &unknown));
    }
}

#[test]
fn provenance_is_visible_in_envelope_and_schedule_records() {
    // Ledger P3.02 step 3: provenance propagates into the envelope output and
    // the schedule record, so a downstream reader sees which limits were
    // inferred or unknown.
    for class in AssetClass::ALL {
        let limits = vec![
            ProvenancedLimit::new(
                class,
                ConstraintKind::ActivePower,
                ConstraintUnit::Megawatt,
                Provenance::Measured,
                interval(-2.0, 3.0),
            ),
            ProvenancedLimit::new(
                class,
                ConstraintKind::RampRate,
                ConstraintUnit::MegawattPerMinute,
                Provenance::Unknown,
                interval(0.0, 5.0),
            ),
        ];
        let envelope = EnvelopeProvenance::new(class, limits);
        assert_eq!(envelope.asset_class, class);
        assert_eq!(envelope.summary.unknown, 1);
        assert_eq!(envelope.summary.measured, 1);
        let unknown_row = envelope
            .limits
            .iter()
            .find(|l| l.provenance == Provenance::Unknown)
            .expect("the unknown limit is visible in the record");
        assert_eq!(unknown_row.effective, NarrowedSet::Interval { lower: Some(0.0), upper: Some(0.0) });

        let entry = ScheduleProvenanceEntry::new(
            flexcore_domain::ids::AssetId::new(asset_id()),
            CAPABILITY_SCHEMA_VERSION,
            envelope,
        )
        .unwrap();
        assert_eq!(entry.envelope.summary.unknown, 1);
    }
}

#[test]
fn schedule_provenance_rejects_a_foreign_schema_version() {
    let err = ScheduleProvenanceEntry::new(
        flexcore_domain::ids::AssetId::new(asset_id()),
        CAPABILITY_SCHEMA_VERSION + 1,
        EnvelopeProvenance::new(AssetClass::Bess, vec![]),
    );
    assert_eq!(
        err,
        Err(ProvenanceRuleError::SchemaVersionMismatch {
            found: CAPABILITY_SCHEMA_VERSION + 1,
            supported: CAPABILITY_SCHEMA_VERSION,
        })
    );
}

// ===========================================================================
// P3.03 — grid boundaries, aggregation precedence, rollover
// ===========================================================================

fn grid_a() -> SchedulingGrid {
    SchedulingGrid::new(
        ValidFrom::from_unix_millis(START_MILLIS).unwrap(),
        SchedulingGridConfig::default(),
    )
}

fn grid_b() -> SchedulingGrid {
    // Independently built: explicit configuration values instead of Default.
    SchedulingGrid::new(
        ValidFrom::from_unix_millis(START_MILLIS).unwrap(),
        SchedulingGridConfig::new(DEFAULT_HORIZON_MILLIS, DEFAULT_INTERVAL_MILLIS).unwrap(),
    )
}

#[test]
fn two_independent_fixtures_on_the_same_horizon_have_identical_boundaries() {
    let a = grid_a();
    let b = grid_b();
    assert_eq!(a.interval_starts(), b.interval_starts());
    assert_eq!(a.interval_count(), 96);
    for i in 0..a.interval_count() {
        assert_eq!(a.interval_bounds(i), b.interval_bounds(i));
    }
}

#[test]
fn boundary_timestamps_follow_start_inclusive_end_exclusive() {
    let grid = grid_a();
    let interval_millis: i64 = 15 * 60 * 1000;
    let at = |millis: i64| EventTime::from_unix_millis(millis).unwrap();

    // Start-inclusive: the horizon start belongs to interval 0.
    assert_eq!(grid.interval_index_at(at(START_MILLIS)), Some(0));
    // End-exclusive: the instant at the 0/1 boundary belongs to interval 1.
    assert_eq!(grid.interval_index_at(at(START_MILLIS + interval_millis)), Some(1));
    // One millisecond inside interval 0 stays interval 0.
    assert_eq!(grid.interval_index_at(at(START_MILLIS + interval_millis - 1)), Some(0));
    // Exactly at the horizon end: no interval of this horizon.
    assert_eq!(grid.interval_index_at(at(START_MILLIS + DEFAULT_HORIZON_MILLIS)), None);
    // Before the horizon start: no interval.
    assert_eq!(grid.interval_index_at(at(START_MILLIS - 1)), None);

    // Interval bounds: interval i's end is interval i+1's start.
    let (s0, e0) = grid.interval_bounds(0).unwrap();
    let (s1, _) = grid.interval_bounds(1).unwrap();
    assert_eq!(s0.unix_millis(), i128::from(START_MILLIS));
    assert_eq!(e0.unix_millis(), s1.unix_millis());
    // Out of range is a named error, not a wraparound.
    assert!(grid.interval_bounds(96).is_err());
}

#[test]
fn aggregation_quality_precedence_table_holds() {
    // The recorded precedence (highest wins):
    //   MISSING > BAD > STALE > SUSPECT > ESTIMATED > GOOD
    let precedence = [
        TelemetryQuality::Missing,
        TelemetryQuality::Bad,
        TelemetryQuality::Stale,
        TelemetryQuality::Suspect,
        TelemetryQuality::Estimated,
        TelemetryQuality::Good,
    ];
    // Empty bucket and single states.
    assert_eq!(aggregate_bucket_quality(&[]), TelemetryQuality::Missing);
    for q in TelemetryQuality::ALL {
        assert_eq!(aggregate_bucket_quality(&[q]), q);
    }
    // Every pair resolves to the higher-precedence state; GOOD survives only
    // if all inputs are GOOD; ESTIMATED stays ESTIMATED.
    for a in TelemetryQuality::ALL {
        for b in TelemetryQuality::ALL {
            let rank = |q: TelemetryQuality| {
                precedence.iter().position(|p| *p == q).unwrap()
            };
            let expected = if rank(a) <= rank(b) { a } else { b };
            assert_eq!(aggregate_bucket_quality(&[a, b]), expected);
        }
    }
    // Documented mapping table spot checks.
    assert_eq!(
        aggregate_bucket_quality(&[
            TelemetryQuality::Good,
            TelemetryQuality::Good,
            TelemetryQuality::Missing,
        ]),
        TelemetryQuality::Missing
    );
    assert_eq!(
        aggregate_bucket_quality(&[TelemetryQuality::Good, TelemetryQuality::Bad]),
        TelemetryQuality::Bad
    );
    assert_eq!(
        aggregate_bucket_quality(&[TelemetryQuality::Good, TelemetryQuality::Stale]),
        TelemetryQuality::Stale
    );
    assert_eq!(
        aggregate_bucket_quality(&[TelemetryQuality::Good, TelemetryQuality::Estimated]),
        TelemetryQuality::Estimated
    );
}

#[test]
fn value_aggregation_survives_quality_and_rejects_malformed_input() {
    let points = vec![(2.0, TelemetryQuality::Good), (4.0, TelemetryQuality::Good)];
    assert_eq!(
        aggregate_subinterval_values(&points, ValueAggregation::Mean).unwrap(),
        flexcore_domain::scheduling_time::AggregatedValue { value: 3.0, quality: TelemetryQuality::Good }
    );
    assert_eq!(
        aggregate_subinterval_values(&points, ValueAggregation::Sum).unwrap().value,
        6.0
    );
    // A bucket containing a MISSING marker keeps the mean but not the GOOD.
    let mixed = vec![(2.0, TelemetryQuality::Good), (4.0, TelemetryQuality::Missing)];
    let agg = aggregate_subinterval_values(&mixed, ValueAggregation::Mean).unwrap();
    assert_eq!(agg.quality, TelemetryQuality::Missing);
    // Malformed: empty bucket and non-finite values are named errors.
    assert!(aggregate_subinterval_values(&[], ValueAggregation::Mean).is_err());
    assert!(
        aggregate_subinterval_values(&[(f64::NAN, TelemetryQuality::Good)], ValueAggregation::Mean)
            .is_err()
    );
}

#[test]
fn rollover_preserves_the_prior_revision() {
    // Ledger P3.03 step 3 / §14.2: a new horizon supersedes the previous
    // schedule revision — never silently overwrites. The rule-level record
    // carries the prior reference unchanged.
    let schedule_id = flexcore_domain::ids::ScheduleId::new(asset_id());
    let prior = ScheduleRevisionRef { schedule_id, revision: 7 };
    let outcome = plan_horizon_rollover(Some(prior), schedule_id).unwrap();
    match outcome {
        RolloverOutcome::Supersedes { superseded, new_revision } => {
            assert_eq!(superseded, prior, "the prior revision reference is preserved");
            assert_eq!(new_revision.revision, 8);
        }
        RolloverOutcome::Initial { .. } => panic!("a prior revision must supersede, not start over"),
    }
    // No prior revision: the first horizon is revision 1.
    assert_eq!(
        plan_horizon_rollover(None, schedule_id).unwrap(),
        RolloverOutcome::Initial {
            new_revision: ScheduleRevisionRef { schedule_id, revision: 1 }
        }
    );
}

// ===========================================================================
// P3.04 — malformed inputs rejected with named errors; boundaries accepted
// ===========================================================================

#[test]
fn unit_mix_kw_and_kwh_rejected_with_named_error() {
    for (field, unit) in [("power_unit", "kW"), ("energy_unit", "kWh")] {
        let err = validate_unit(field, unit).unwrap_err();
        assert_eq!(err.rule, CapabilityValidationRule::NonCanonicalUnit);
        assert_eq!(err.field, field);
        assert_eq!(err.offending_value, unit);
        assert!(err.to_string().contains("UNIT_NOT_CANONICAL"));
        assert!(err.to_string().contains(unit));
    }
    // Canonical spellings pass.
    for unit in ["MW", "MVAr", "MVA", "MWh"] {
        assert_eq!(validate_unit("unit", unit), Ok(()));
    }
}

#[test]
fn sign_flip_rejected_with_named_error() {
    // P > 0 = export (§10.2): a sign-flipped active-power declaration is
    // rejected by name.
    let err =
        validate_active_power_sign("intervals[0].constraints[0].sign", ConstraintSignConvention::ConsumptionPositive)
            .unwrap_err();
    assert_eq!(err.rule, CapabilityValidationRule::ActivePowerSignNotExport);
    assert!(err.to_string().contains("ACTIVE_POWER_SIGN_NOT_EXPORT"));
    assert!(err.to_string().contains("export"));
    assert_eq!(
        validate_active_power_sign("sign", ConstraintSignConvention::ExportPositive),
        Ok(())
    );
}

#[test]
fn soc_boundary_inclusive_zero_and_one_accepted() {
    assert_eq!(validate_soc("soc", 0.0), Ok(()));
    assert_eq!(validate_soc("soc", 1.0), Ok(()));
    assert_eq!(validate_soc("soc", 0.42), Ok(()));
}

#[test]
fn soc_out_of_range_rejected_with_named_error() {
    for fraction in [-0.01, 1.01] {
        let err = validate_soc("soc", fraction).unwrap_err();
        assert_eq!(err.rule, CapabilityValidationRule::SocOutOfRange);
        assert_eq!(err.offending_value, fraction.to_string());
        assert!(err.to_string().contains("SOC_OUT_OF_RANGE"));
    }
}

#[test]
fn inverted_energy_limits_rejected_with_named_error() {
    let err = validate_energy_window("energy_window", 10.0, 5.0).unwrap_err();
    assert_eq!(err.rule, CapabilityValidationRule::EnergyLimitsInverted);
    assert!(err.to_string().contains("ENERGY_LIMITS_INVERTED"));
    // Equal bounds are a valid single-point window.
    assert_eq!(validate_energy_window("energy_window", 5.0, 5.0), Ok(()));
}

#[test]
fn negative_ramp_rejected_with_named_error() {
    let err = validate_ramp_rate("ramp_mw_per_min", -0.5).unwrap_err();
    assert_eq!(err.rule, CapabilityValidationRule::NegativeRampRate);
    assert!(err.to_string().contains("NEGATIVE_RAMP_RATE"));
    assert_eq!(validate_ramp_rate("ramp_mw_per_min", 0.0), Ok(()));
}

#[test]
fn negative_charge_and_discharge_rejected_with_named_errors() {
    let err = validate_charge_power("p_ch_max", -1.0).unwrap_err();
    assert_eq!(err.rule, CapabilityValidationRule::NegativeChargePower);
    let err = validate_discharge_power("p_dis_max", -1.0).unwrap_err();
    assert_eq!(err.rule, CapabilityValidationRule::NegativeDischargePower);
    assert_eq!(validate_charge_power("p_ch_max", 0.0), Ok(()));
    assert_eq!(validate_discharge_power("p_dis_max", 0.0), Ok(()));
}

#[test]
fn bess_boundary_accepts_valid_inputs_including_soc_zero_and_one() {
    assert_eq!(validate_bess_capability(0.0, 2.0, 2.0, 0.0, 10.0, 1.0), Ok(()));
    assert_eq!(validate_bess_capability(1.0, 2.0, 2.0, 0.0, 10.0, 1.0), Ok(()));
}

#[test]
fn error_messages_are_audit_safe() {
    // Ledger P3.04 step 3: no internal paths or secrets in messages. Every
    // catalogue message is plain rule text plus the field locator.
    let inputs: Vec<Result<(), flexcore_domain::capability_validate::CapabilityValidationError>> = vec![
        validate_unit("power_unit", "kW"),
        validate_active_power_sign("sign", ConstraintSignConvention::ConsumptionPositive),
        validate_reactive_power_sign("sign", ConstraintSignConvention::ExportPositive),
        validate_soc("soc", 1.5),
        validate_charge_power("p_ch_max", -1.0),
        validate_discharge_power("p_dis_max", -1.0),
        validate_energy_window("energy_window", 10.0, 5.0),
        validate_ramp_rate("ramp", -1.0),
    ];
    for result in inputs {
        let display = result.unwrap_err().to_string();
        assert!(!display.contains('\\'), "no filesystem path separators: {display}");
        assert!(!display.contains(".rs"), "no source file references: {display}");
        assert!(!display.contains("C:"), "no drive paths: {display}");
    }
}

// ===========================================================================
// Cross-cutting: a malformed capability never reaches the envelope calculator
// ===========================================================================

#[test]
fn malformed_schema_constraints_are_caught_by_the_model_walk() {
    // Build a model whose only constraint is a sign-flipped active-power
    // declaration; the P3.04 model walk rejects it with the named error
    // before it can enter an envelope or a solve.
    let bad = DeclaredConstraint::new(
        ConstraintKind::ActivePower,
        ConstraintUnit::Megawatt,
        ConstraintSignConvention::ConsumptionPositive,
        Provenance::Measured,
        window(),
        ConstraintExpression::Interval { lower: Some(-2.0), upper: Some(3.0) },
    )
    .unwrap();
    let horizon_start = ValidFrom::from_unix_millis(START_MILLIS).unwrap().timestamp();
    let set = TimeIndexedFeasibleSet::new(
        horizon_start,
        900,
        1,
        vec![flexcore_contracts::capability::IntervalConstraints {
            interval_index: 0,
            constraints: vec![bad],
        }],
    )
    .unwrap();
    let model = flexcore_contracts::capability::CapabilityModel::new(
        CAPABILITY_SCHEMA_VERSION,
        asset_id(),
        window(),
        set,
    )
    .unwrap();
    let err = flexcore_domain::capability_validate::validate_capability_model(&model).unwrap_err();
    assert_eq!(err.rule, CapabilityValidationRule::ActivePowerSignNotExport);
    assert_eq!(err.field, "intervals[0].constraints[0].ACTIVE_POWER");
}
