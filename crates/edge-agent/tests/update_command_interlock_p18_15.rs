// P18.15 — Integration tests for the update/command-lifecycle interlock
//          (`crates/edge-agent/src/update_command_interlock.rs`; decision
//          record `docs/phases-11-20/phase-18-edge-fleet-lifecycle-ota/
//          records/28_p18_15_update_command_interlock.md`).
//
// WRITTEN-UNRUN: these tests have NEVER been executed — running them is
// the recorded human gate (loop ban 3; conventions §1.3). They compose the
// PUBLIC surfaces only: the P18.15 interlock, the UNCHANGED wave-1
// `UpdateSessionStateMachine` (P18.04), and the P6.13 conformance harness.
// No command exists, no update has ever been applied, no fleet exists;
// every exercise below is against in-memory test doubles.
//
// THE EXACT SURFACE P18.14's concurrent-command tests consume (record 28
// §5 itemizes it): `UpdateCommandInterlock::authorize_update_attempt` →
// `Ok(UpdateAttemptAuthorization { in_flight_count: 0, .. })` or
// `Err(UpdateBlockRefusal)` with stable names
// `UPDATE_BLOCKED_COMMANDS_IN_FLIGHT` / `UPDATE_INTERLOCK_STATE_UNKNOWN` /
// `UPDATE_CONFORMANCE_RECHECK_OUTSTANDING` / `UPDATE_CONFORMANCE_RECHECK_FAILED`;
// the composition is the two-step
// `interlock.authorize_update_attempt()?` then
// `session.begin_download(auth.in_flight_count, now)?` — the session rule
// is fed the interlock verdict verbatim, so the two rules cannot disagree
// in the unsafe direction.

use std::sync::{Arc, Mutex};

use flexcore_edge_agent::command_path::FROZEN_COMMAND_STATE_NAMES;
use flexcore_edge_agent::ports::{AuditSink, EdgeAuditEvent, EdgeError};
use flexcore_edge_agent::update_command_interlock::{
    DeviceCommandObservation, DrainPhase, InMemoryCommandStateSource, InterlockError,
    RecheckOverride, RecheckPhase, UpdateBlockRefusal, UpdateCommandInterlock,
    FROZEN_TERMINAL_COMMAND_STATE_NAMES, INTERLOCK_STATEMENT,
};
use flexcore_edge_agent::update_session::{
    UpdateProgressState, UpdateSessionError, UpdateSessionStateMachine,
};
use flexcore_ports::clock::Clock;

const BASE_MILLIS: i64 = 1_700_000_000_000;

// --- Test doubles ----------------------------------------------------------

struct FixedClock(i64);
impl Clock for FixedClock {
    fn now_event_time(&self) -> flexcore_contracts::time_model::EventTime {
        flexcore_contracts::time_model::EventTime::from_unix_millis(self.0).expect("valid instant")
    }
    fn now_ingest_time(&self) -> flexcore_contracts::time_model::IngestTime {
        flexcore_contracts::time_model::IngestTime::from_unix_millis(self.0).expect("valid instant")
    }
}

#[derive(Default)]
struct CollectingAuditSink {
    events: Mutex<Vec<String>>,
}

impl CollectingAuditSink {
    fn lines(&self) -> Vec<String> {
        self.events.lock().expect("poisoned test sink mutex").clone()
    }
}

impl AuditSink for CollectingAuditSink {
    fn record(&self, event: EdgeAuditEvent) -> Result<(), EdgeError> {
        self.events
            .lock()
            .expect("poisoned test sink mutex")
            .push(event.detail);
        Ok(())
    }
}

fn observation(command_id: &str, state_name: &str) -> DeviceCommandObservation {
    DeviceCommandObservation {
        command_id: command_id.to_string(),
        state_name: state_name.to_string(),
    }
}

fn interlock(
    source: Arc<InMemoryCommandStateSource>,
) -> (UpdateCommandInterlock, Arc<CollectingAuditSink>) {
    let sink = Arc::new(CollectingAuditSink::default());
    let interlock = UpdateCommandInterlock::new(
        source,
        sink.clone() as Arc<dyn AuditSink>,
        Arc::new(FixedClock(BASE_MILLIS)),
    );
    (interlock, sink)
}

fn session(attempt: &str) -> UpdateSessionStateMachine {
    UpdateSessionStateMachine::new(attempt, "device-1", "1.2.0", 3, BASE_MILLIS)
        .expect("session opens")
}

// --- The blocking rule -------------------------------------------------------

#[test]
fn an_update_is_blocked_while_a_command_is_issued() {
    // The exact phase-plan P18.15 sentence: updates BLOCKED while commands
    // are ISSUED on the device — through the real interlock, with the
    // recorded stable refusal name, and the refusal audited.
    let source = Arc::new(InMemoryCommandStateSource::reporting(vec![observation(
        "cmd-1",
        "ISSUED",
    )]));
    let (interlock, sink) = interlock(source);
    let refusal = interlock
        .authorize_update_attempt()
        .expect_err("the update is blocked");
    assert_eq!(
        refusal,
        UpdateBlockRefusal::CommandsInFlight { issued_or_tracking: 1 }
    );
    assert_eq!(refusal.name(), "UPDATE_BLOCKED_COMMANDS_IN_FLIGHT");
    assert!(sink
        .lines()
        .iter()
        .any(|l| l.contains("EDGE_UPDATE_INTERLOCK_BLOCKED")
            && l.contains("UPDATE_BLOCKED_COMMANDS_IN_FLIGHT")));
}

#[test]
fn an_update_is_blocked_while_a_command_is_tracking() {
    let source = Arc::new(InMemoryCommandStateSource::reporting(vec![observation(
        "cmd-1",
        "TRACKING",
    )]));
    let (interlock, _sink) = interlock(source);
    assert!(matches!(
        interlock.authorize_update_attempt(),
        Err(UpdateBlockRefusal::CommandsInFlight { issued_or_tracking: 1 })
    ));
}

#[test]
fn the_drain_gap_states_block_too_strictly_safer_than_the_citation() {
    // ACKNOWLEDGED / FAILED / EXPIRED are non-terminal in the frozen table
    // (FAILED and EXPIRED exit only via FALLBACK) — the interlock blocks
    // them even though the recorded citation names only ISSUED/TRACKING.
    // This is the recorded strengthening (record 28 §2), never a
    // relaxation.
    for state in ["ACKNOWLEDGED", "FAILED", "EXPIRED"] {
        let source = Arc::new(InMemoryCommandStateSource::reporting(vec![observation(
            "cmd-1", state,
        )]));
        let (interlock, _sink) = interlock(source);
        assert!(
            interlock.authorize_update_attempt().is_err(),
            "state {state} must block the update"
        );
    }
}

#[test]
fn only_the_frozen_terminal_states_count_as_drained() {
    // Every frozen TERMINAL state alone authorizes; every non-terminal
    // frozen state alone blocks. The terminal set is EXACTLY the frozen
    // five (cited, never forked) and every name is inside the frozen 14.
    assert_eq!(
        FROZEN_TERMINAL_COMMAND_STATE_NAMES,
        ["COMPLETED", "REJECTED", "CANCELLED", "SUPERSEDED", "FALLBACK"]
    );
    for state in FROZEN_COMMAND_STATE_NAMES {
        let source = Arc::new(InMemoryCommandStateSource::reporting(vec![observation(
            "cmd-1", state,
        )]));
        let (interlock, _sink) = interlock(source);
        let blocked = interlock.authorize_update_attempt().is_err();
        assert_eq!(
            blocked,
            !FROZEN_TERMINAL_COMMAND_STATE_NAMES.contains(&state),
            "state {state}: blocked must equal non-terminal"
        );
    }
}

#[test]
fn an_unknown_device_command_state_blocks_fail_closed() {
    // A device whose command state cannot be established is NEVER read as
    // "no commands in flight" (the P9.34 fail-closed posture applied to
    // the interlock's own input).
    let source = Arc::new(InMemoryCommandStateSource::default());
    source.set_unobservable("the command-path tracking map is unreachable".to_string());
    let (interlock, sink) = interlock(source);
    let refusal = interlock
        .authorize_update_attempt()
        .expect_err("fail-closed");
    assert!(matches!(refusal, UpdateBlockRefusal::DeviceStateUnknown { .. }));
    assert_eq!(refusal.name(), "UPDATE_INTERLOCK_STATE_UNKNOWN");
    assert!(sink
        .lines()
        .iter()
        .any(|l| l.contains("UPDATE_INTERLOCK_STATE_UNKNOWN")));

    // A state name outside the frozen 14-state vocabulary is refused,
    // never guessed at.
    let source = Arc::new(InMemoryCommandStateSource::reporting(vec![observation(
        "cmd-1",
        "VENDOR_UNKNOWN_STATE",
    )]));
    let (interlock, _sink) = interlock(source);
    assert!(matches!(
        interlock.authorize_update_attempt(),
        Err(UpdateBlockRefusal::DeviceStateUnknown { .. })
    ));
}

// --- The two-step composition with the wave-1 session rule -------------------

#[test]
fn the_interlock_feeds_the_wave1_session_rule_and_never_disagrees_unsafely() {
    // BLOCKED path: the interlock refuses; the session rule would refuse
    // on the same count.
    let source = Arc::new(InMemoryCommandStateSource::reporting(vec![
        observation("cmd-1", "ISSUED"),
        observation("cmd-2", "ACKNOWLEDGED"),
    ]));
    let (interlock, _sink) = interlock(source);
    assert!(interlock.authorize_update_attempt().is_err());
    let mut blocked_session = session("attempt-1");
    assert!(matches!(
        blocked_session.begin_download(2, BASE_MILLIS + 1),
        Err(UpdateSessionError::CommandsInFlight { issued_or_tracking: 2 })
    ));

    // DRAINED path: authorize returns in_flight_count == 0 and the
    // UNCHANGED session rule accepts it verbatim.
    let source = Arc::new(InMemoryCommandStateSource::reporting(vec![
        observation("cmd-1", "COMPLETED"),
        observation("cmd-2", "CANCELLED"),
    ]));
    let (interlock, _sink) = interlock(source);
    let authorization = interlock
        .authorize_update_attempt()
        .expect("drained — authorized");
    assert_eq!(authorization.in_flight_count, 0);
    let mut session = session("attempt-1");
    let state = session
        .begin_download(authorization.in_flight_count, BASE_MILLIS + 1)
        .expect("the session rule accepts the interlock verdict");
    assert_eq!(state, UpdateProgressState::Downloading);
}

// --- The drain procedure (report §5.6, cited, never rewritten) ---------------

#[test]
fn the_drain_walks_issued_to_terminal_and_then_authorizes() {
    // Full §5.6 walk: drain starts (new work stopped), the operator's
    // lifecycle action drains the command through the frozen table
    // (ISSUED → CANCELLED, frozen edge 17 — performed by the operator,
    // never by the update path), the drain completes, the state is
    // re-read at authorize time, and the session opens.
    let source = Arc::new(InMemoryCommandStateSource::reporting(vec![observation(
        "cmd-1",
        "ISSUED",
    )]));
    let (mut interlock, sink) = interlock(source);

    let phase = interlock.begin_drain().expect("drain starts").clone();
    assert!(matches!(phase, DrainPhase::Draining { .. }));
    assert!(!interlock.allows_new_command_work());

    // Still ISSUED: the drain continues (observed, never forced).
    assert!(matches!(
        interlock.poll_drain().expect("observed").clone(),
        DrainPhase::Draining { .. }
    ));

    // The device becomes unobservable mid-drain: the drain cannot complete
    // (fail-closed), the phase is retained.
    source.set_unobservable("tracking lost".to_string());
    assert!(matches!(
        interlock.poll_drain(),
        Err(InterlockError::DeviceStateUnknown { .. })
    ));
    assert!(matches!(interlock.drain_phase(), DrainPhase::Draining { .. }));

    // The operator drains the command (lifecycle action, frozen table).
    source.set_observations(vec![observation("cmd-1", "CANCELLED")]);
    assert!(matches!(
        interlock.poll_drain().expect("observed").clone(),
        DrainPhase::Drained { .. }
    ));
    assert!(interlock.allows_new_command_work());
    assert!(sink
        .lines()
        .iter()
        .any(|l| l.contains("EDGE_UPDATE_DRAIN_STARTED")));
    assert!(sink
        .lines()
        .iter()
        .any(|l| l.contains("EDGE_UPDATE_DRAIN_COMPLETE")));

    let authorization = interlock
        .authorize_update_attempt()
        .expect("drained — authorized");
    let mut session = session("attempt-1");
    session
        .begin_download(authorization.in_flight_count, BASE_MILLIS + 1)
        .expect("session opens after the drain");
}

#[test]
fn the_drain_is_observer_only_toward_the_command_lifecycle() {
    // The update path never transitions a command state: the "finish or
    // cancel pending operations" leg is the operator's action through the
    // normal frozen lifecycle (report §5.6; plan §4 — no update path
    // bypasses the command lifecycle). The interlock's begin/poll drain
    // calls leave the command path's record byte-identical.
    let source = Arc::new(InMemoryCommandStateSource::reporting(vec![
        observation("cmd-1", "TRACKING"),
    ]));
    let (mut interlock, _sink) = interlock(source);
    interlock.begin_drain().expect("drain starts");
    interlock.poll_drain().expect_err("still TRACKING");
    interlock.poll_drain().expect_err("still TRACKING");
    let observed = source.device_command_states().expect("readable");
    assert_eq!(
        observed,
        vec![observation("cmd-1", "TRACKING")],
        "the drain must not have written any command state"
    );
}

// --- The post-update conformance re-check (P6.13 interplay) ------------------

#[test]
fn after_activation_the_conformance_recheck_blocks_further_updates() {
    let source = Arc::new(InMemoryCommandStateSource::reporting(vec![]));
    let (mut interlock, sink) = interlock(source);

    // The wave-1 session reaches Active (the terminal success state).
    let mut session = session("attempt-1");
    session.begin_download(0, BASE_MILLIS + 1).expect("ok");
    session.download_complete(BASE_MILLIS + 2).expect("ok");
    session.verify_passed(BASE_MILLIS + 3).expect("ok");
    session.begin_install(BASE_MILLIS + 4).expect("ok");
    session.install_complete(BASE_MILLIS + 5).expect("ok");
    session.health_confirmed(BASE_MILLIS + 6).expect("active");
    assert_eq!(session.state(), UpdateProgressState::Active);
    assert!(session.state().is_terminal());

    // The composition records the activation: the re-check becomes
    // OUTSTANDING and further updates are blocked.
    interlock.record_update_activated("attempt-1").expect("recorded");
    let refusal = interlock
        .authorize_update_attempt()
        .expect_err("re-check outstanding");
    assert!(matches!(
        refusal,
        UpdateBlockRefusal::ConformanceRecheckOutstanding { attempt_id } if attempt_id == "attempt-1"
    ));
    assert!(sink
        .lines()
        .iter()
        .any(|l| l.contains("EDGE_UPDATE_ACTIVATED_RECHECK_OUTSTANDING")));
}

#[test]
fn a_clean_p6_13_report_satisfies_the_recheck() {
    use flexcore_edge_agent::adapters::acl::{
        AntiCorruptionLayer, NativeSign, NativeUnit, PointMapping,
    };
    use flexcore_edge_agent::adapters::port::PointKey;
    use flexcore_edge_agent::adapters::simulated::{
        SimulatedEmsSource, SimulatedFixture, SimulatedPointFixture,
    };
    use flexcore_edge_agent::conformance::{ConformanceHarness, ConformanceLimits};

    // The REAL §19.2(6) harness against the REAL simulated adapter — the
    // exact P6.13 interplay, consumed (never reimplemented).
    let acl = AntiCorruptionLayer::new(
        vec![PointMapping {
            point: PointKey::new("ess0", "ActivePower"),
            asset_key: "ess-01".to_string(),
            quantity: flexcore_ports::telemetry_source::TelemetryQuantity::ActivePower,
            native_unit: NativeUnit::Kw,
            native_sign: Some(NativeSign::ExportPositive),
            reactive_source_convention: None,
        }],
        "edge:pilot-site-A:simulated-test",
        vec![],
    )
    .expect("valid mapping table");
    let fixture = SimulatedFixture {
        points: vec![SimulatedPointFixture {
            point: PointKey::new("ess0", "ActivePower"),
            native_unit: NativeUnit::Kw,
            values_per_tick: vec![1_500.0],
            quality_per_tick: vec!["GOOD".to_string()],
        }],
        start_millis: BASE_MILLIS,
        step_millis: 1_000,
        reactive_source_convention:
            flexcore_contracts::units::ReactiveSignConvention::InjectionPositive,
        cadence_millis: 5_000,
    };
    let harness = ConformanceHarness::new(
        acl,
        BASE_MILLIS + 5_000,
        ConformanceLimits {
            max_clock_skew_past_millis: 3_600_000,
        },
    )
    .expect("valid limits");
    let mut adapter = SimulatedEmsSource::new(fixture);
    let report = harness
        .run_fault_injection_set(&mut adapter, &PointKey::new("ess0", "ActivePower"));
    assert!(report.is_clean(), "{}", report.render());

    let source = Arc::new(InMemoryCommandStateSource::reporting(vec![]));
    let (mut interlock, _sink) = interlock(source);
    interlock.record_update_activated("attempt-1").expect("recorded");
    interlock
        .record_conformance_report(&report)
        .expect("recorded");
    assert_eq!(interlock.recheck_phase(), &RecheckPhase::Satisfied);
    assert!(interlock.authorize_update_attempt().is_ok());
}

#[test]
fn a_violating_report_fails_the_recheck_terminal_until_a_recorded_override() {
    use flexcore_edge_agent::conformance::{CaseOutcome, CaseRecord, ConformanceCase, ConformanceReport};

    // A §19.2(6) contract violation after the update: the `Valid` case
    // REJECTED (the updated adapter dropped the mapping) — the report
    // class contract fails.
    let report = ConformanceReport {
        records: vec![CaseRecord {
            case: ConformanceCase::Valid,
            point: "ess0/ActivePower".to_string(),
            outcome: CaseOutcome::Rejected {
                diagnostic: "the updated adapter dropped the mapping".to_string(),
            },
        }],
    };
    assert!(!report.is_clean());

    let source = Arc::new(InMemoryCommandStateSource::reporting(vec![]));
    let (mut interlock, sink) = interlock(source);
    interlock.record_update_activated("attempt-1").expect("recorded");
    let phase = interlock
        .record_conformance_report(&report)
        .expect("recorded")
        .clone();
    assert!(matches!(phase, RecheckPhase::Failed { violations: 1 }));

    // Further updates refused with the FAILED variant; re-running the
    // harness to fish for a pass is refused (terminal until a recorded
    // override).
    assert!(matches!(
        interlock.authorize_update_attempt(),
        Err(UpdateBlockRefusal::ConformanceRecheckFailed { violations: 1, .. })
    ));
    assert!(interlock.record_conformance_report(&report).is_err());

    // An override without a named actor/authority is not a recorded
    // override (the P9.34 RollbackOverride pattern, cited).
    assert!(matches!(
        interlock.clear_conformance_failure(&RecheckOverride {
            actor: String::new(),
            authority_reference: "ticket-1".to_string(),
        }),
        Err(InterlockError::InvalidInput { .. })
    ));

    // The recorded override RE-OPENS the re-check as OUTSTANDING — it
    // never grants a pass; a clean report is still required.
    interlock
        .clear_conformance_failure(&RecheckOverride {
            actor: "J. Doe (ops)".to_string(),
            authority_reference: "ticket-1".to_string(),
        })
        .expect("cleared");
    assert_eq!(interlock.recheck_phase(), &RecheckPhase::Outstanding);
    assert!(interlock.authorize_update_attempt().is_err());
    assert!(sink
        .lines()
        .iter()
        .any(|l| l.contains("EDGE_UPDATE_CONFORMANCE_FAILURE_CLEARED")));
}

#[test]
fn each_new_activation_requires_a_fresh_recheck() {
    use flexcore_edge_agent::conformance::{CaseOutcome, CaseRecord, ConformanceCase, ConformanceReport};

    let clean = ConformanceReport {
        records: vec![CaseRecord {
            case: ConformanceCase::Valid,
            point: "ess0/ActivePower".to_string(),
            outcome: CaseOutcome::Accepted,
        }],
    };
    let source = Arc::new(InMemoryCommandStateSource::reporting(vec![]));
    let (mut interlock, _sink) = interlock(source);
    interlock.record_update_activated("attempt-1").expect("recorded");
    interlock.record_conformance_report(&clean).expect("satisfied");
    assert_eq!(interlock.recheck_phase(), &RecheckPhase::Satisfied);

    interlock.record_update_activated("attempt-2").expect("recorded");
    assert_eq!(interlock.recheck_phase(), &RecheckPhase::Outstanding);
    assert_eq!(interlock.recheck_attempt_id(), Some("attempt-2"));
    assert!(interlock.authorize_update_attempt().is_err());
}

// --- Statement pins ----------------------------------------------------------

#[test]
fn the_interlock_statement_pins_the_honesty_bounds() {
    assert!(INTERLOCK_STATEMENT.contains("strict superset"));
    assert!(INTERLOCK_STATEMENT.contains("No command exists"));
    assert!(INTERLOCK_STATEMENT.contains("no update has ever been applied"));
    assert!(INTERLOCK_STATEMENT.contains("no fleet exists"));
    assert!(INTERLOCK_STATEMENT.contains("never protocol conformance"));
    for terminal in FROZEN_TERMINAL_COMMAND_STATE_NAMES {
        assert!(
            FROZEN_COMMAND_STATE_NAMES.contains(&terminal),
            "terminal state {terminal} must be a frozen §14.1 state"
        );
    }
}
