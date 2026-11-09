// P3.10 — Flexible load model test suite (integration, from outside the
// crate).
//
// Covers the ledger acceptance criteria end to end, from OUTSIDE
// flexcore-domain, with a load configured from the synthetic-portfolio
// scenario (scenario-v2.json `site-north-load-1` where the scenario declares
// a value) and fixture values labelled ASSUMPTION where it does not:
//
// - Duration/recovery/rebound hold in every test schedule: an event
//   exceeding the 2-hour maximum duration, or starting before the 1-hour
//   minimum recovery completes, is flagged AND excluded from credited
//   flexibility (the decided EXCLUDE-AND-FLAG policy); boundary events of
//   exactly 2 hours with an exactly-1-hour recovery gap are feasible.
// - Rebound is REQUIRED and accounted: net flexibility never overstates
//   (net = credited reduction − max(owed, scheduled increase)); zero
//   rebound is accepted only with a recorded reason.
// - The baseline method is named ("scenario-declared-baseline-profile"),
//   locked at construction (no market/DR baseline can enter), and labelled
//   synthetic on every evaluation output.
// - No load is dispatched outside its availability window.
// - Unknown-provenance ranges narrow to zero credit (the frozen P3.02 rule).
// - The pure `enforce_event` rule (the single implementation the P3.12
//   envelope and P3.17 scheduler reuse) behaves identically at the limits.
//
// Tolerance: `LOAD_EVALUATION_TOLERANCE` = 1e-9, the module's API contract.
//
// Decision record:
// `ledger/phase-03-flexibility-scheduling/09_11_load_ev_models.md`.

use flexcore_domain::bess_model::ProvenancedValue;
use flexcore_domain::flexible_load_model::{
    enforce_event, BaselineMethod, DeclaredEvent, FlexibleLoadConfig, FlexibleLoadModel,
    LoadSchedule, LoadSetpoint, LoadViolation, ReboundEnergy, EventViolation,
    DURATION_RECOVERY_POLICY_STATEMENT, LOAD_EVALUATION_TOLERANCE,
    SCENARIO_DECLARED_BASELINE_METHOD, SYNTHETIC_BASELINE_DISCLAIMER,
};

const TOL: f64 = LOAD_EVALUATION_TOLERANCE;

/// 2026-06-01T00:00:00Z — the scenario-v2 horizon start.
const HORIZON_START_MILLIS: i64 = 1_780_272_000_000;
const INTERVAL_SECONDS: u32 = 900; // 15 minutes (the phase proposal, open question 1)
const INTERVALS: usize = 96; // 24 hours

fn approx(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= TOL
}

fn assert_approx(actual: f64, expected: f64, what: &str) {
    assert!(
        approx(actual, expected),
        "{what}: expected {expected}, found {actual}"
    );
}

/// Builds a full-horizon setpoint vector from reduction events given as
/// (first_interval, end_interval_exclusive, power_mw) triples.
fn reduction_events(events: &[(usize, usize, f64)]) -> Vec<LoadSetpoint> {
    let mut setpoints = vec![LoadSetpoint::idle(); INTERVALS];
    for (start, end, power) in events {
        for interval in setpoints.iter_mut().take(*end).skip(*start) {
            *interval = LoadSetpoint::reduce(*power);
        }
    }
    setpoints
}

/// Appends increase intervals (first_interval, end_interval_exclusive,
/// power_mw) to a setpoint vector.
fn with_increase(mut setpoints: Vec<LoadSetpoint>, events: &[(usize, usize, f64)]) -> Vec<LoadSetpoint> {
    for (start, end, power) in events {
        for interval in setpoints.iter_mut().take(*end).skip(*start) {
            *interval = LoadSetpoint::increase(*power);
        }
    }
    setpoints
}

/// The ledger P3.10 test load: reducible 0.3 MW (scenario-v2.json
/// `site-north-load-1` `reducible_range_mw`), 2-hour maximum event duration,
/// 1-hour minimum recovery, rebound 0.3 MWh per full event (ASSUMPTION — the
/// scenario declares no rebound), 0.2 MW increasable (ASSUMPTION — the
/// scenario declares no increasable range), flat 0.5 MW baseline (synthetic
/// fixture — the scenario declares the `morning_evening_peaks` shape without
/// a numeric series), availability 06:00–22:00 UTC (ASSUMPTION fixture).
fn model() -> FlexibleLoadModel {
    FlexibleLoadModel::new(FlexibleLoadConfig {
        baseline_method: BaselineMethod::scenario_declared(),
        baseline_demand_mw: vec![0.5; INTERVALS],
        reducible_range_mw: ProvenancedValue::configured(0.3),
        increasable_range_mw: ProvenancedValue::configured(0.2),
        response_time_seconds: 300.0,
        max_event_duration_seconds: 7200.0, // 2 hours
        min_recovery_seconds: 3600.0,       // 1 hour
        rebound: ReboundEnergy::nonzero(0.3).unwrap(),
        comfort_constraints: vec![
            "process continuity: the served process must not be interrupted"
                .to_string(),
        ],
        availability_windows_millis: vec![(
            HORIZON_START_MILLIS + 6 * 3_600_000, // 06:00 UTC
            HORIZON_START_MILLIS + 22 * 3_600_000, // 22:00 UTC
        )],
        interval_seconds: INTERVAL_SECONDS,
        horizon_start_millis: HORIZON_START_MILLIS,
    })
    .unwrap()
}

fn evaluate(setpoints: Vec<LoadSetpoint>) -> flexcore_domain::flexible_load_model::FlexibleLoadEvaluation {
    model()
        .evaluate_schedule(&LoadSchedule { setpoints })
        .unwrap()
}

// --- Availability windows -------------------------------------------------------

#[test]
fn no_dispatch_outside_availability_windows() {
    // Interval 0 (00:00 UTC) is before the 06:00 window: dispatching there
    // is a named violation (ledger acceptance: no load dispatched outside
    // its availability window).
    let evaluation = evaluate(reduction_events(&[(0, 4, 0.3)]));
    assert!(matches!(
        evaluation.violations.iter().find(|v| matches!(v, LoadViolation::DispatchOutsideAvailabilityWindow { interval: 0, .. })),
        Some(LoadViolation::DispatchOutsideAvailabilityWindow { interval_start_millis, .. } if *interval_start_millis == HORIZON_START_MILLIS)
    ));

    // The same reduction inside the window (06:00 UTC = interval 24) is not
    // flagged for availability.
    let evaluation = evaluate(reduction_events(&[(24, 28, 0.3)]));
    assert!(
        !evaluation
            .violations
            .iter()
            .any(|v| matches!(v, LoadViolation::DispatchOutsideAvailabilityWindow { .. })),
        "inside-window dispatch must not be flagged: {:?}",
        evaluation.violations
    );
}

#[test]
fn construction_requires_at_least_one_availability_window() {
    // Availability is never "always" by omission: an empty window list is a
    // configuration error.
    let result = FlexibleLoadModel::new(FlexibleLoadConfig {
        baseline_method: BaselineMethod::scenario_declared(),
        baseline_demand_mw: vec![0.5; INTERVALS],
        reducible_range_mw: ProvenancedValue::configured(0.3),
        increasable_range_mw: ProvenancedValue::configured(0.2),
        response_time_seconds: 300.0,
        max_event_duration_seconds: 7200.0,
        min_recovery_seconds: 3600.0,
        rebound: ReboundEnergy::nonzero(0.3).unwrap(),
        comfort_constraints: vec![],
        availability_windows_millis: vec![],
        interval_seconds: INTERVAL_SECONDS,
        horizon_start_millis: HORIZON_START_MILLIS,
    });
    assert!(matches!(
        result,
        Err(flexcore_domain::flexible_load_model::LoadModelError::NoAvailabilityWindowDeclared)
    ));

    // An inverted window is rejected too.
    let result = FlexibleLoadModel::new(FlexibleLoadConfig {
        baseline_method: BaselineMethod::scenario_declared(),
        baseline_demand_mw: vec![0.5; INTERVALS],
        reducible_range_mw: ProvenancedValue::configured(0.3),
        increasable_range_mw: ProvenancedValue::configured(0.2),
        response_time_seconds: 300.0,
        max_event_duration_seconds: 7200.0,
        min_recovery_seconds: 3600.0,
        rebound: ReboundEnergy::nonzero(0.3).unwrap(),
        comfort_constraints: vec![],
        availability_windows_millis: vec![(HORIZON_START_MILLIS + 100, HORIZON_START_MILLIS)],
        interval_seconds: INTERVAL_SECONDS,
        horizon_start_millis: HORIZON_START_MILLIS,
    });
    assert!(matches!(
        result,
        Err(flexcore_domain::flexible_load_model::LoadModelError::InvertedAvailabilityWindow { .. })
    ));
}

// --- Duration and recovery (EXCLUDE-AND-FLAG) ------------------------------------

#[test]
fn event_exceeding_max_duration_is_flagged_and_excluded() {
    // 9 intervals = 2.25 h above the 2-hour maximum: the event is flagged
    // AND its energy is excluded from credit (decided EXCLUDE-AND-FLAG
    // policy; the setpoints are never clipped).
    let evaluation = evaluate(reduction_events(&[(24, 33, 0.3)]));
    assert!(matches!(
        evaluation.violations.iter().find(|v| matches!(v, LoadViolation::EventDurationExceeded { .. })),
        Some(LoadViolation::EventDurationExceeded {
            event_start_interval: 24,
            duration_seconds,
            max_event_duration_seconds: 7200.0,
        } if approx(duration_seconds, 8100.0)
    ));
    assert_approx(evaluation.declared_reduction_mwh, 0.675, "declared reduction");
    assert_approx(evaluation.credited_reduction_mwh, 0.0, "credited reduction (excluded)");
    // The excluded event owes no rebound.
    assert_approx(evaluation.rebound_owed_mwh, 0.0, "rebound owed by the excluded event");
    // Net flexibility never overstates: nothing is credited.
    assert_approx(evaluation.net_flexibility_mwh, 0.0, "net flexibility");
    // The policy statement travels on the output.
    assert_eq!(evaluation.duration_recovery_policy, DURATION_RECOVERY_POLICY_STATEMENT);
}

#[test]
fn event_starting_before_recovery_completes_is_flagged_and_excluded() {
    // Event A: 06:00–07:00 (4 intervals), credited. Event B starts 07:15 —
    // a 15-minute gap, below the 1-hour minimum: B is flagged AND excluded.
    let evaluation = evaluate(reduction_events(&[(24, 28, 0.3), (29, 33, 0.3)]));
    assert!(matches!(
        evaluation.violations.iter().find(|v| matches!(v, LoadViolation::RecoveryIncomplete { .. })),
        Some(LoadViolation::RecoveryIncomplete {
            event_start_interval: 29,
            gap_seconds,
            min_recovery_seconds: 3600.0,
        } if approx(gap_seconds, 900.0)
    ));
    assert_approx(evaluation.declared_reduction_mwh, 0.6, "declared reduction");
    assert_approx(evaluation.credited_reduction_mwh, 0.3, "credited reduction (A only)");
    // Rebound is owed only by the credited event A: 0.3 × (3600/7200) = 0.15.
    assert_approx(evaluation.rebound_owed_mwh, 0.15, "rebound owed");
    // Unpaid rebound is subtracted; the net never overstates.
    assert_approx(evaluation.unpaid_rebound_mwh, 0.15, "unpaid rebound");
    assert_approx(evaluation.net_flexibility_mwh, 0.15, "net flexibility");
    assert!(evaluation
        .violations
        .iter()
        .any(|v| matches!(v, LoadViolation::ReboundPaybackIncomplete { .. })));
}

#[test]
fn boundary_events_exactly_at_limits_are_feasible() {
    // Event A: exactly 2 hours (intervals 24–31, 06:00–08:00). Event B
    // starts at 09:00 — exactly the 1-hour recovery gap. Both are credited;
    // no duration/recovery violation exists.
    let setpoints = with_increase(
        reduction_events(&[(24, 32, 0.3), (36, 44, 0.3)]),
        &[(48, 60, 0.2)], // payback: 12 intervals × 0.2 MW = 0.6 MWh = owed
    );
    let evaluation = evaluate(setpoints);
    assert!(
        !evaluation
            .violations
            .iter()
            .any(|v| matches!(
                v,
                LoadViolation::EventDurationExceeded { .. }
                    | LoadViolation::RecoveryIncomplete { .. }
                    | LoadViolation::ReboundPaybackIncomplete { .. }
            )),
        "boundary events at exactly the configured limits must be feasible: {:?}",
        evaluation.violations
    );
    assert_approx(evaluation.declared_reduction_mwh, 1.2, "declared reduction");
    assert_approx(evaluation.credited_reduction_mwh, 1.2, "credited reduction");
    assert_approx(evaluation.rebound_owed_mwh, 0.6, "rebound owed (2 full events)");
    assert_approx(evaluation.scheduled_increase_mwh, 0.6, "scheduled increase");
    assert_approx(evaluation.unpaid_rebound_mwh, 0.0, "unpaid rebound");
    // Net = credited − max(owed, scheduled) = 1.2 − 0.6.
    assert_approx(evaluation.net_flexibility_mwh, 0.6, "net flexibility");
    assert_eq!(evaluation.events.len(), 2);
    assert!(evaluation.events.iter().all(|event| event.credited));
}

// --- Rebound accounting -----------------------------------------------------------

#[test]
fn unpaid_rebound_means_net_flexibility_never_overstates() {
    // One full 2-hour event at 0.3 MW = 0.6 MWh delivered, no payback
    // scheduled: the net is 0.6 − 0.3 = 0.3 MWh, never 0.6.
    let evaluation = evaluate(reduction_events(&[(24, 32, 0.3)]));
    assert_approx(evaluation.credited_reduction_mwh, 0.6, "credited reduction");
    assert_approx(evaluation.rebound_owed_mwh, 0.3, "rebound owed");
    assert_approx(evaluation.unpaid_rebound_mwh, 0.3, "unpaid rebound");
    assert_approx(evaluation.net_flexibility_mwh, 0.3, "net flexibility");
    assert!(matches!(
        evaluation.violations.iter().find(|v| matches!(v, LoadViolation::ReboundPaybackIncomplete { .. })),
        Some(LoadViolation::ReboundPaybackIncomplete { rebound_owed_mwh, scheduled_increase_mwh, unpaid_mwh }
            if approx(rebound_owed_mwh, 0.3)
                && approx(scheduled_increase_mwh, 0.0)
                && approx(unpaid_mwh, 0.3))
    ));
}

#[test]
fn rebound_is_a_required_field_zero_only_with_a_recorded_reason() {
    // Zero rebound without a reason is rejected — rebound is never
    // silently defaulted.
    assert_eq!(
        ReboundEnergy::explicit_zero(String::new()),
        Err(flexcore_domain::flexible_load_model::LoadModelError::EmptyReasonField {
            field: "rebound_zero_reason"
        })
    );
    // Negative or non-finite rebound is rejected.
    assert!(matches!(
        ReboundEnergy::nonzero(-0.1),
        Err(flexcore_domain::flexible_load_model::LoadModelError::InvalidReboundEnergy { .. })
    ));
    assert!(matches!(
        ReboundEnergy::nonzero(f64::NAN),
        Err(flexcore_domain::flexible_load_model::LoadModelError::InvalidReboundEnergy { .. })
    ));
    // Explicit zero WITH a recorded reason is accepted, and the model then
    // owes no rebound at all.
    let zero = ReboundEnergy::explicit_zero(
        "ASSUMPTION: the served process is fully deferrable with no post-event \
payback recorded in the scenario fixture"
            .to_string(),
    )
    .unwrap();
    assert_eq!(zero.energy_mwh(), 0.0);
    assert!(zero.zero_reason().is_some());

    let zero_rebound_model = FlexibleLoadModel::new(FlexibleLoadConfig {
        baseline_method: BaselineMethod::scenario_declared(),
        baseline_demand_mw: vec![0.5; INTERVALS],
        reducible_range_mw: ProvenancedValue::configured(0.3),
        increasable_range_mw: ProvenancedValue::configured(0.2),
        response_time_seconds: 300.0,
        max_event_duration_seconds: 7200.0,
        min_recovery_seconds: 3600.0,
        rebound: zero,
        comfort_constraints: vec![],
        availability_windows_millis: vec![(
            HORIZON_START_MILLIS + 6 * 3_600_000,
            HORIZON_START_MILLIS + 22 * 3_600_000,
        )],
        interval_seconds: INTERVAL_SECONDS,
        horizon_start_millis: HORIZON_START_MILLIS,
    })
    .unwrap();
    let evaluation = zero_rebound_model
        .evaluate_schedule(&LoadSchedule { setpoints: reduction_events(&[(24, 32, 0.3)]) })
        .unwrap();
    assert_approx(evaluation.rebound_owed_mwh, 0.0, "rebound owed (explicit zero)");
    assert!(
        !evaluation
            .violations
            .iter()
            .any(|v| matches!(v, LoadViolation::ReboundPaybackIncomplete { .. })),
        "an explicitly-zero rebound with a recorded reason owes no payback"
    );
    assert_approx(evaluation.net_flexibility_mwh, 0.6, "net flexibility (zero rebound)");
}

// --- The baseline rule (decided once) ----------------------------------------------

#[test]
fn baseline_method_is_named_and_locked() {
    // The decided synthetic method is the ONLY accepted name: a market/DR
    // baseline cannot enter through configuration (report §11.4; D1/D2
    // open).
    assert_eq!(
        BaselineMethod::try_new("market-baseline-v1".to_string()),
        Err(flexcore_domain::flexible_load_model::LoadModelError::UnknownBaselineMethod {
            name: "market-baseline-v1".to_string(),
            accepted: SCENARIO_DECLARED_BASELINE_METHOD,
        })
    );
    let method = BaselineMethod::scenario_declared();
    assert_eq!(method.name(), SCENARIO_DECLARED_BASELINE_METHOD);

    // Every evaluation carries the method name and the synthetic-baseline
    // disclaimer.
    let evaluation = evaluate(reduction_events(&[(24, 28, 0.3)]));
    assert_eq!(evaluation.baseline_method_name, SCENARIO_DECLARED_BASELINE_METHOD);
    assert_eq!(evaluation.baseline_disclaimer, SYNTHETIC_BASELINE_DISCLAIMER);
    assert!(evaluation.baseline_disclaimer.contains("NOT a market/DR baseline"));
    assert!(evaluation.baseline_disclaimer.contains("D1/D2 open"));
}

// --- Provenance narrowing ------------------------------------------------------------

#[test]
fn unknown_provenance_range_narrows_to_zero_credit() {
    // The frozen P3.02 rule: an UNKNOWN-provenance reducible range collapses
    // to zero credit — never "unconstrained".
    let narrowed_model = FlexibleLoadModel::new(FlexibleLoadConfig {
        baseline_method: BaselineMethod::scenario_declared(),
        baseline_demand_mw: vec![0.5; INTERVALS],
        reducible_range_mw: ProvenancedValue::unknown(),
        increasable_range_mw: ProvenancedValue::configured(0.2),
        response_time_seconds: 300.0,
        max_event_duration_seconds: 7200.0,
        min_recovery_seconds: 3600.0,
        rebound: ReboundEnergy::nonzero(0.3).unwrap(),
        comfort_constraints: vec![],
        availability_windows_millis: vec![(
            HORIZON_START_MILLIS + 6 * 3_600_000,
            HORIZON_START_MILLIS + 22 * 3_600_000,
        )],
        interval_seconds: INTERVAL_SECONDS,
        horizon_start_millis: HORIZON_START_MILLIS,
    })
    .unwrap();
    assert_approx(narrowed_model.effective_reducible_mw(), 0.0, "effective reducible range");
    // The unknown marker carries no declared number (BESS `ProvenancedValue`
    // convention).
    assert!(narrowed_model.declared_reducible_mw().is_nan());
    let evaluation = narrowed_model
        .evaluate_schedule(&LoadSchedule { setpoints: reduction_events(&[(24, 28, 0.1)]) })
        .unwrap();
    assert!(matches!(
        evaluation.violations.iter().find(|v| matches!(v, LoadViolation::ReductionAboveRange { .. })),
        Some(LoadViolation::ReductionAboveRange { reduction_mw, limit_mw, .. }
            if approx(reduction_mw, 0.1) && approx(limit_mw, 0.0))
    ));
}

// --- Setpoint and schedule-shape validation -------------------------------------------

#[test]
fn schedule_shape_and_value_errors_are_named() {
    // Length mismatch.
    let short = LoadSchedule { setpoints: vec![LoadSetpoint::idle(); INTERVALS - 1] };
    assert_eq!(
        model().evaluate_schedule(&short),
        Err(flexcore_domain::flexible_load_model::LoadModelError::ScheduleLengthMismatch {
            setpoints: INTERVALS - 1,
            intervals: INTERVALS,
        })
    );
    // Non-finite setpoint.
    let nan = LoadSchedule {
        setpoints: reduction_events(&[(24, 28, f64::NAN)]),
    };
    assert!(matches!(
        model().evaluate_schedule(&nan),
        Err(flexcore_domain::flexible_load_model::LoadModelError::NonFiniteSetpoint { field: "reduction_mw", .. })
    ));
    // Above-range reduction and negative magnitude.
    let evaluation = evaluate(reduction_events(&[(24, 28, 0.4)]));
    assert!(matches!(
        evaluation.violations.iter().find(|v| matches!(v, LoadViolation::ReductionAboveRange { .. })),
        Some(LoadViolation::ReductionAboveRange { reduction_mw, limit_mw, .. }
            if approx(reduction_mw, 0.4) && approx(limit_mw, 0.3))
    ));
    let mut negative = reduction_events(&[]);
    negative[24] = LoadSetpoint { reduction_mw: -0.1, increase_mw: 0.0 };
    let evaluation = evaluate(negative);
    assert!(matches!(
        evaluation.violations.iter().find(|v| matches!(v, LoadViolation::NegativeMagnitude { .. })),
        Some(LoadViolation::NegativeMagnitude { field: "reduction_mw", value_mw, .. } if approx(value_mw, -0.1))
    ));
}

// --- Sign convention --------------------------------------------------------------------

#[test]
fn net_export_follows_the_export_positive_rule() {
    // P > 0 = export (§10.2). Baseline demand 0.5 MW is consumption:
    // net export = reduction − increase − baseline.
    let evaluation = evaluate(reduction_events(&[(24, 28, 0.3)]));
    // interval 24: 0.3 − 0 − 0.5 = −0.2 MW (still importing).
    assert_approx(evaluation.net_export_mw[24], -0.2, "net export with reduction");
    // interval 0 (idle): 0 − 0 − 0.5 = −0.5 MW.
    assert_approx(evaluation.net_export_mw[0], -0.5, "net export at baseline");
    let evaluation = evaluate(with_increase(reduction_events(&[]), &[(24, 28, 0.2)]));
    // interval 24: 0 − 0.2 − 0.5 = −0.7 MW (increased consumption).
    assert_approx(evaluation.net_export_mw[24], -0.7, "net export with increase");
}

// --- The pure event-enforcement function (P3.12 / P3.17 reuse) ----------------------------

#[test]
fn enforce_event_duration_and_recovery_boundaries() {
    let t0 = HORIZON_START_MILLIS + 6 * 3_600_000;
    // Exactly 2 hours: feasible.
    let event = DeclaredEvent { start_millis: t0, end_millis: t0 + 7_200_000, power_mw: 0.3 };
    assert!(enforce_event(&event, None, 7200.0, 3600.0).is_empty());
    // One millisecond more: flagged.
    let event = DeclaredEvent { start_millis: t0, end_millis: t0 + 7_200_001, power_mw: 0.3 };
    assert!(matches!(
        enforce_event(&event, None, 7200.0, 3600.0).as_slice(),
        [EventViolation::DurationExceeded { duration_seconds, max_event_duration_seconds: 7200.0 }]
            if (*duration_seconds - 7200.001).abs() < 1e-6
    ));
    // Exactly 1 hour gap: feasible.
    let previous = DeclaredEvent { start_millis: t0, end_millis: t0 + 7_200_000, power_mw: 0.3 };
    let next = DeclaredEvent {
        start_millis: t0 + 7_200_000 + 3_600_000,
        end_millis: t0 + 7_200_000 + 3_600_000 + 3_600_000,
        power_mw: 0.3,
    };
    assert!(enforce_event(&next, Some(&previous), 7200.0, 3600.0).is_empty());
    // One millisecond less: flagged.
    let next = DeclaredEvent {
        start_millis: t0 + 7_200_000 + 3_600_000 - 1,
        end_millis: t0 + 7_200_000 + 3_600_000 - 1 + 3_600_000,
        power_mw: 0.3,
    };
    assert!(matches!(
        enforce_event(&next, Some(&previous), 7200.0, 3600.0).as_slice(),
        [EventViolation::RecoveryIncomplete { gap_seconds, min_recovery_seconds: 3600.0 }]
            if (*gap_seconds - 3599.999).abs() < 1e-6
    ));
    // The first event of the horizon has no recovery obligation.
    let first = DeclaredEvent { start_millis: t0, end_millis: t0 + 3_600_000, power_mw: 0.3 };
    assert!(enforce_event(&first, None, 7200.0, 3600.0).is_empty());
}

// --- Recorded unknown dimensions ------------------------------------------------------------

#[test]
fn response_time_lag_is_recorded_not_silent() {
    let evaluation = evaluate(reduction_events(&[(24, 28, 0.3)]));
    let lag = evaluation
        .unknown_dimensions
        .iter()
        .find(|d| d.dimension == "RESPONSE_TIME_LAG")
        .expect("the response-time lag must be recorded as an unknown dimension");
    assert!(lag.note.contains("P3.17"), "the note names the formulation concern: {}", lag.note);
    // The response time itself is recorded configuration on the model.
    assert_approx(model().response_time_seconds(), 300.0, "response time");
}
