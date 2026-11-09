// P4.19 — Dispatch timing honesty tests (report §14.5, §0.3, §17.5, §19.4,
//         §24 step 8; ledger P4.19 acceptance criteria; decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/17_19_fallback_timing.md`).
//
// Coverage per the ledger acceptance criteria:
//
// - Latency RECORDING (never a guarantee): acknowledgement latency + tracking
//   error are recorded on the §24 step 8 simulation path (a command driven
//   through ISSUED → ACKNOWLEDGED → TRACKING on the P4.10 lifecycle engine,
//   with a controlled clock — never wall-clock).
// - The reporting type carries the MEASURED DISTRIBUTION (histogram +
//   p50/p95/p99 quantiles), never a single best case; a recorder with no
//   samples has no distribution (error, never an empty report).
// - Figures are always reported WITH their conditions (simulator, load,
//   measurement method) — a figure without conditions is invalid output.
// - No latency guarantee and no target anywhere: the report text embeds the
//   no-guarantee and §14.5 boundary statements; the distribution type exposes
//   no best-case accessor and no target field (structural).
// - The §14.5 boundary constants exist and name every function that is NOT
//   promised (structural; the full claim scan is the decision document's
//   recorded scan).

use std::sync::Mutex;

use flexcore_application::timing_honesty::{
    measure_from_journal, TimingConditions, TimingHonestyError, TimingRecorder,
    MEASUREMENT_CONDITIONS_RULE, NO_LATENCY_GUARANTEE_STATEMENT, NOT_PROMISED_FUNCTIONS,
    SUPERVISORY_ONLY_STATEMENT, TIMING_BOUNDARY_STATEMENT,
};
use flexcore_contracts::time_model::ValidUntil;
use flexcore_domain::command_lifecycle::{
    Actor, AuthorizationOutcome, CommandLifecycle, EvidenceReference, FailureReason, GateOutcome,
    LifecycleClock,
};
use flexcore_domain::ids::CommandId;
use flexcore_contracts::ids::FlexUuid;

// --- Controlled clock (never wall-clock) ------------------------------------------

struct TestClock {
    now_unix_millis: Mutex<i64>,
}

impl TestClock {
    fn at(now_unix_millis: i64) -> Self {
        Self {
            now_unix_millis: Mutex::new(now_unix_millis),
        }
    }

    fn advance_to(&self, now_unix_millis: i64) {
        *self.now_unix_millis.lock().expect("test clock") = now_unix_millis;
    }
}

impl LifecycleClock for TestClock {
    fn now_unix_millis(&self) -> i64 {
        *self.now_unix_millis.lock().expect("test clock")
    }
}

// --- The §24 step 8 simulation path fixture ---------------------------------------

const VALID_UNTIL_MILLIS: i64 = 100_000;

/// Drives ONE command through the §24 step 8 simulation sequence: PROPOSED →
/// VALIDATED → FEASIBLE → APPROVED → ISSUED (step 7: simulator adapter only)
/// → ACKNOWLEDGED (step 8: simulated acknowledgement) → TRACKING (simulated
/// tracking) — with the controlled clock advanced between steps so the
/// journal carries the measured instants.
fn simulate_step8_path(command: CommandId, clock: &TestClock, ack_delay_millis: i64) -> CommandLifecycle {
    let valid_until = ValidUntil::from_unix_millis(VALID_UNTIL_MILLIS).expect("valid bound");
    let mut lifecycle = CommandLifecycle::propose(command, valid_until).expect("propose");
    let actor = Actor::new("service:dispatch-adapter").expect("actor");
    clock.advance_to(1_000);
    lifecycle
        .record_validation(
            GateOutcome::pass(EvidenceReference::new("validation:sim").expect("evidence")),
            actor.clone(),
            clock,
        )
        .expect("validation");
    clock.advance_to(2_000);
    lifecycle
        .record_feasibility(
            GateOutcome::pass(EvidenceReference::new("feasibility:sim").expect("evidence")),
            actor.clone(),
            clock,
        )
        .expect("feasibility");
    clock.advance_to(3_000);
    lifecycle
        .record_authorization(
            AuthorizationOutcome::approve(
                EvidenceReference::new("authz:sim").expect("evidence"),
                actor.clone(),
            ),
            clock,
        )
        .expect("authorization");
    // Step 7: the command is sent ONLY to the simulator adapter.
    clock.advance_to(4_000);
    lifecycle
        .record_issue(
            EvidenceReference::new("signature:simulator-adapter").expect("evidence"),
            actor.clone(),
            clock,
        )
        .expect("issue");
    // Step 8: simulated acknowledgement after the configured delay.
    clock.advance_to(4_000 + ack_delay_millis);
    lifecycle
        .record_acknowledgement(
            EvidenceReference::new("acceptance:simulator").expect("evidence"),
            actor.clone(),
            clock,
        )
        .expect("acknowledgement");
    clock.advance_to(4_000 + ack_delay_millis + 500);
    lifecycle
        .record_tracking_started(
            EvidenceReference::new("telemetry:simulator").expect("evidence"),
            actor,
            clock,
        )
        .expect("tracking");
    lifecycle
}

fn command_id(n: u8) -> CommandId {
    CommandId::new(FlexUuid::from_bytes((0x0192a5e0_7e2f_9a1b_3c4d_5e6f7c0b0000u128 + n as u128).to_be_bytes()))
}

fn conditions() -> TimingConditions {
    TimingConditions::new(
        "flexcore dispatch simulator (P4.16 adapter seam; §24 step 8)",
        "synthetic feeder fixture; four simulated assets; nominal load",
        "ack latency from the P4.10 transition journal (ISSUED → ACKNOWLEDGED, controlled \
         clock); tracking deviation from the simulator tracking telemetry",
    )
    .expect("valid conditions")
}

// --- The measurement test (ledger P4.19 step 2) --------------------------------------

#[test]
fn acknowledgement_latency_and_tracking_error_are_recorded_on_the_step8_path() {
    let clock = TestClock::at(1_000);
    // Five simulated commands with different acknowledgement delays (the
    // simulator's artificial transport behaviour) and tracking deviations.
    let runs: [(u8, i64, f64); 5] = [
        (1, 120, 0.4),
        (2, 250, 1.2),
        (3, 90, 0.1),
        (4, 480, 2.5),
        (5, 310, 0.9),
    ];
    let mut recorder = TimingRecorder::new(conditions());
    for (n, delay, deviation) in runs {
        let lifecycle = simulate_step8_path(command_id(n), &clock, delay);
        let sample = measure_from_journal(lifecycle.journal(), deviation)
            .expect("the §24 step 8 path measures cleanly");
        assert_eq!(sample.command_id, command_id(n));
        // The measured latency equals the journal's ISSUED → ACKNOWLEDGED
        // distance (4_000 → 4_000 + delay).
        assert_eq!(sample.acknowledgement_latency_millis, delay);
        assert_eq!(sample.tracking_deviation_mw, deviation);
        assert_eq!(sample.measured_at_unix_millis, 4_000 + delay);
        recorder.record(sample);
    }

    // The MEASURED DISTRIBUTION — never a single best case.
    let distribution = recorder.report().expect("distribution");
    assert_eq!(distribution.sample_count, 5);
    assert_eq!(distribution.conditions, conditions());
    // Sorted latencies: 90, 120, 250, 310, 480.
    assert_eq!(distribution.quantiles.p50_millis, 250);
    assert_eq!(distribution.quantiles.p95_millis, 480);
    assert_eq!(distribution.quantiles.p99_millis, 480);
    assert_eq!(distribution.max_ack_latency_millis, 480);
    assert_eq!(distribution.max_tracking_deviation_mw, 2.5);
    // The histogram covers every sample exactly once.
    let counted: usize = distribution.histogram.iter().map(|bin| bin.count).sum();
    assert_eq!(counted, 5);
    for pair in distribution.histogram.windows(2) {
        assert_eq!(pair[0].upper_exclusive_millis, pair[1].lower_inclusive_millis);
    }
}

#[test]
fn the_report_always_carries_the_conditions_and_the_boundary_statements() {
    let clock = TestClock::at(1_000);
    let lifecycle = simulate_step8_path(command_id(6), &clock, 200);
    let mut recorder = TimingRecorder::new(conditions());
    recorder.record(
        measure_from_journal(lifecycle.journal(), 0.7).expect("measurable"),
    );
    let distribution = recorder.report().expect("distribution");
    let text = distribution.report_text();
    // Conditions (simulator, load, measurement method) are embedded.
    assert!(text.contains("simulator=flexcore dispatch simulator"));
    assert!(text.contains("load=synthetic feeder fixture"));
    assert!(text.contains("measurement_method="));
    // The no-guarantee and §14.5 boundary statements are embedded — a bare
    // figure is never the output form.
    assert!(text.contains(NO_LATENCY_GUARANTEE_STATEMENT));
    assert!(text.contains(TIMING_BOUNDARY_STATEMENT));
    // The measured distribution is present (quantiles + histogram), not a
    // single best case.
    assert!(text.contains("p50="));
    assert!(text.contains("histogram"));
    assert!(MEASUREMENT_CONDITIONS_RULE.contains("conditions"));
}

#[test]
fn an_empty_recorder_has_no_distribution_and_never_an_empty_report() {
    let recorder = TimingRecorder::new(conditions());
    assert_eq!(recorder.sample_count(), 0);
    assert_eq!(recorder.report(), Err(TimingHonestyError::NoSamples));
}

#[test]
fn a_command_that_never_reached_acknowledgement_yields_no_latency_sample() {
    // A FAILED command (NACK before acknowledgement) is a FAILED case, not
    // a latency sample — fail-closed, never estimated around.
    let clock = TestClock::at(1_000);
    let valid_until = ValidUntil::from_unix_millis(VALID_UNTIL_MILLIS).expect("valid bound");
    let mut lifecycle = CommandLifecycle::propose(command_id(7), valid_until).expect("propose");
    let actor = Actor::new("service:dispatch-adapter").expect("actor");
    clock.advance_to(1_000);
    lifecycle
        .record_validation(
            GateOutcome::pass(EvidenceReference::new("validation:sim").expect("evidence")),
            actor.clone(),
            &clock,
        )
        .expect("validation");
    clock.advance_to(2_000);
    lifecycle
        .record_issue(
            EvidenceReference::new("signature:simulator-adapter").expect("evidence"),
            actor.clone(),
            &clock,
        )
        .expect("issue");
    clock.advance_to(3_000);
    lifecycle
        .record_failure(FailureReason::TransportTimeout, actor, &clock)
        .expect("failure");
    let error = measure_from_journal(lifecycle.journal(), 0.0)
        .expect_err("no ACKNOWLEDGED entry — no latency sample");
    assert!(matches!(
        error,
        TimingHonestyError::MissingJournalMilestone { milestone: "ACKNOWLEDGED", .. }
    ));
}

#[test]
fn a_non_finite_or_negative_tracking_deviation_is_rejected() {
    let clock = TestClock::at(1_000);
    let lifecycle = simulate_step8_path(command_id(8), &clock, 100);
    assert!(matches!(
        measure_from_journal(lifecycle.journal(), f64::NAN),
        Err(TimingHonestyError::InvalidSample { .. })
    ));
    assert!(matches!(
        measure_from_journal(lifecycle.journal(), -1.0),
        Err(TimingHonestyError::InvalidSample { .. })
    ));
}

// --- The structural boundary assertions (ledger P4.19 steps 1 and 4) ------------------

#[test]
fn the_timing_boundary_constants_exist_and_state_the_section_14_5_boundary() {
    // The §14.5 boundary is quoted verbatim (the mandatory boundary
    // statement; the decision document carries the full timing-model note
    // and the recorded claim scan).
    assert!(TIMING_BOUNDARY_STATEMENT.contains(
        "should not claim primary-frequency, synthetic-inertia, relay, inverter inner-loop, \
         or sub-cycle control capabilities"
    ));
    assert!(TIMING_BOUNDARY_STATEMENT.contains("local certified controllers and hardware"));
    assert!(SUPERVISORY_ONLY_STATEMENT.contains("supervisory setpoint delivery"));
    assert!(NO_LATENCY_GUARANTEE_STATEMENT.contains("no latency target is set"));
}

#[test]
fn the_unpromised_function_list_is_complete_against_the_report_wording() {
    // §14.5/§0.3/AGENTS.md rule 3: the functions FlexCore does NOT perform.
    for expected in [
        "hard real-time dispatch",
        "primary frequency response",
        "synthetic inertia",
        "relay protection",
        "inverter inner-loop control",
        "sub-cycle control",
    ] {
        assert!(
            NOT_PROMISED_FUNCTIONS.contains(&expected),
            "the unpromised-functions list must name `{expected}`"
        );
    }
    assert_eq!(NOT_PROMISED_FUNCTIONS.len(), 6);
}

#[test]
fn the_distribution_type_exposes_no_best_case_and_no_target() {
    // Structural: the sanctioned report form carries the distribution and
    // the statements; there is no "best case" accessor and no target field
    // on TimingDistribution (checked here through the public surface the
    // report text compiles from — the struct's fields are the histogram,
    // quantiles, maxima, sample count, and conditions only).
    let clock = TestClock::at(1_000);
    let lifecycle = simulate_step8_path(command_id(9), &clock, 150);
    let mut recorder = TimingRecorder::new(conditions());
    recorder.record(
        measure_from_journal(lifecycle.journal(), 0.3).expect("measurable"),
    );
    let distribution = recorder.report().expect("distribution");
    // Only distribution facts are exposed:
    let _ = (
        distribution.sample_count,
        distribution.histogram.len(),
        distribution.quantiles.p50_millis,
        distribution.quantiles.p95_millis,
        distribution.quantiles.p99_millis,
        distribution.max_ack_latency_millis,
        distribution.max_tracking_deviation_mw,
        distribution.conditions.clone(),
    );
    // …and the report text never contains a guarantee or target phrasing
    // beyond the embedded prohibition itself.
    let text = distribution.report_text();
    assert!(!text.contains("guarantees a latency of"));
    assert!(!text.contains("latency target:"));
}
