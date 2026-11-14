// P7.07 — Plugin lifecycle state machine: transition, drain, crash, audit
// tests (ledger P7.07; test file written, NOT run — the loop bans hold; the
// human build/test pass executes these).
//
// Coverage (ledger P7.07 acceptance criteria):
//
// 1. The full happy path walks the frozen §5.6 table and audits every
//    transition (registration, five gates, drain, stop).
// 2. Every illegal transition lands on its NAMED error
//    (`ILLEGAL_TRANSITION`), and the legal-set predicate is EXACTLY the
//    frozen table plus the recorded crash edges — the §5.6 property-test
//    seed: no sequence of legal moves reaches an undefined state because
//    the state space is closed and every edge is table-checked.
// 3. Drain-before-replace is honoured: a replacement is BLOCKED while the
//    incumbent is active or merely draining, and allowed only after
//    quiescence (`Stopped`).
// 4. A drain-deadline expiry REFUSES the stop (fail-closed), leaves the
//    incumbent active (`Ready`), and audits the refusal — the operator
//    decides.
// 5. Crash → `Failed` (held, with `failed_from` retained for the P7.30
//    drill) → the EXPLICIT-restart path; no auto-restart exists (a gated
//    forward step out of `Failed` is illegal).
// 6. Clock-injected determinism: the same injected clock readings yield
//    the identical audit sequence, twice.

use flexcore_kernel::lifecycle::{
    is_legal_transition, AuditSink, Clock, CRASH_TRANSITIONS, DrainOutcome, DrainPolicy, DrainPort,
    FixedClock, FROZEN_TRANSITIONS, GateOutcome, LifecycleAuditKind, LifecycleAuditRecord,
    LifecycleError, LifecycleState, LIFECYCLE_STATES, MemoryAuditSink, PluginLifecycle,
};

// --- Fixtures ------------------------------------------------------------------

const PLUGIN_ID: &str = "builtin.highs-optimizer";
const VERSION: &str = "0.1.0";

fn pass() -> GateOutcome {
    GateOutcome::Passed
}

fn failed_gate(reason: &str) -> GateOutcome {
    GateOutcome::Failed { reason: reason.to_string() }
}

/// A scripted drain port: either quiesces (reporting the cancelled count)
/// or reports the deadline exceeded with work outstanding. Records the
/// deadline it was handed (the configured policy on the injected clock) in
/// an Rc-shared cell, so the test can read it after the port itself has
/// been moved into the lifecycle.
struct ScriptedDrain {
    outstanding: usize,
    quiesce: bool,
    cancelled: usize,
    observed_deadline: std::rc::Rc<std::cell::Cell<Option<u64>>>,
}

impl ScriptedDrain {
    fn quiescing(cancelled: usize) -> (ScriptedDrain, std::rc::Rc<std::cell::Cell<Option<u64>>>) {
        let observed = std::rc::Rc::new(std::cell::Cell::new(None));
        (
            ScriptedDrain {
                outstanding: 0,
                quiesce: true,
                cancelled,
                observed_deadline: std::rc::Rc::clone(&observed),
            },
            observed,
        )
    }

    fn stuck(outstanding: usize) -> ScriptedDrain {
        ScriptedDrain {
            outstanding,
            quiesce: false,
            cancelled: 0,
            observed_deadline: std::rc::Rc::new(std::cell::Cell::new(None)),
        }
    }
}

impl DrainPort for ScriptedDrain {
    fn outstanding_work(&self) -> usize {
        self.outstanding
    }

    fn drain(&mut self, deadline_millis: u64) -> DrainOutcome {
        self.observed_deadline.set(Some(deadline_millis));
        if self.quiesce {
            DrainOutcome::Quiesced { cancelled: self.cancelled }
        } else {
            DrainOutcome::DeadlineExceeded { outstanding: self.outstanding }
        }
    }
}

fn registered<'a>(clock: &'a dyn Clock, sink: &'a dyn AuditSink) -> PluginLifecycle<'a> {
    PluginLifecycle::register(PLUGIN_ID, VERSION, clock, sink, DrainPolicy::default())
}

/// Walk the machine from `Discovered` to `Ready` through every gate.
fn to_ready(lifecycle: &mut PluginLifecycle) {
    lifecycle.verify(pass(), "sdk-installer", "signature and provenance accepted").expect("verify");
    lifecycle
        .mark_compatible(pass(), "sdk-installer", "manifest and API compatibility pass")
        .expect("compatible");
    lifecycle.configure(pass(), "sdk-installer", "schema and secret references valid").expect("configured");
    lifecycle
        .start(pass(), "sdk-installer", "dependency graph resolved (P7.06 plan)")
        .expect("started");
    lifecycle.mark_ready(pass(), "health-check", "health checks pass").expect("ready");
}

fn assert_error(err: LifecycleError, expected_name: &str) -> LifecycleError {
    assert_eq!(err.name(), expected_name, "unexpected error: {err}");
    err
}

// --- 1. Happy path ----------------------------------------------------------------

#[test]
fn happy_path_walks_the_frozen_table_and_audits_every_transition() {
    let clock = FixedClock::new(1_000);
    let sink = MemoryAuditSink::new();
    let mut lifecycle = registered(&clock, &sink);
    assert_eq!(lifecycle.state(), LifecycleState::Discovered);

    to_ready(&mut lifecycle);
    assert_eq!(lifecycle.state(), LifecycleState::Ready);
    assert!(lifecycle.failed_from().is_none());

    // Drain-before-replace happy path: begin, quiesce, stop.
    let (mut drain, observed_deadline) = ScriptedDrain::quiescing(2);
    lifecycle
        .begin_drain(&mut drain, "operator", "upgrade requested")
        .expect("Ready -> Draining");
    assert_eq!(lifecycle.state(), LifecycleState::Draining);
    // The configured deadline (30 s DEFAULT) on the injected clock.
    assert_eq!(observed_deadline.get(), Some(31_000));

    let outcome = lifecycle.complete_drain("operator", "active work completed").expect("quiesced");
    assert_eq!(outcome, DrainOutcome::Quiesced { cancelled: 2 });
    assert_eq!(lifecycle.state(), LifecycleState::Stopped);
    assert!(lifecycle.state().is_terminal());

    // Every transition is audited, in order, with from/to/actor/timestamp.
    let records = sink.records();
    let kinds: Vec<LifecycleAuditKind> = records.iter().map(|r| r.kind).collect();
    assert_eq!(
        kinds,
        vec![
            LifecycleAuditKind::Registration,
            LifecycleAuditKind::Transition,
            LifecycleAuditKind::Transition,
            LifecycleAuditKind::Transition,
            LifecycleAuditKind::Transition,
            LifecycleAuditKind::Transition,
            LifecycleAuditKind::Transition, // Ready -> Draining
            LifecycleAuditKind::Transition, // Draining -> Stopped
        ]
    );
    assert_eq!(records[0].from, None);
    assert_eq!(records[0].to, Some(LifecycleState::Discovered));
    assert_eq!(records[1].from, Some(LifecycleState::Discovered));
    assert_eq!(records[1].to, Some(LifecycleState::Verified));
    assert_eq!(records[6].from, Some(LifecycleState::Ready));
    assert_eq!(records[6].to, Some(LifecycleState::Draining));
    assert_eq!(records[7].from, Some(LifecycleState::Draining));
    assert_eq!(records[7].to, Some(LifecycleState::Stopped));
    for record in records.iter() {
        assert_eq!(record.plugin_id, PLUGIN_ID);
        assert_eq!(record.version, VERSION);
        assert!(!record.actor.is_empty());
        assert!(!record.reason.is_empty());
        assert_eq!(record.timestamp_millis, 1_000, "the injected clock never advanced in this test");
    }
}

#[test]
fn default_drain_deadline_is_thirty_seconds() {
    assert_eq!(DrainPolicy::default().deadline_ms, 30_000);
}

// --- 2. Illegal transitions are named ----------------------------------------------

#[test]
fn the_legal_set_is_exactly_the_frozen_table_plus_crash_edges() {
    for from in LIFECYCLE_STATES {
        for to in LIFECYCLE_STATES {
            let frozen = FROZEN_TRANSITIONS.iter().any(|(f, t)| *f == from && *t == to);
            let crash = CRASH_TRANSITIONS.iter().any(|(f, t)| *f == from && *t == to);
            assert_eq!(
                is_legal_transition(from, to),
                frozen || crash,
                "the legal predicate must be exactly the frozen table plus crash edges for {from:?} -> {to:?}"
            );
        }
    }
    // The §5.6 core edges are present verbatim.
    assert!(is_legal_transition(LifecycleState::Discovered, LifecycleState::Verified));
    assert!(is_legal_transition(LifecycleState::Ready, LifecycleState::Draining));
    assert!(is_legal_transition(LifecycleState::Draining, LifecycleState::Stopped));
    assert!(is_legal_transition(LifecycleState::Verified, LifecycleState::Rejected));
    assert!(is_legal_transition(LifecycleState::Failed, LifecycleState::Draining));
    // Terminal states have no outgoing edge.
    for from in LIFECYCLE_STATES {
        if from.is_terminal() {
            for to in LIFECYCLE_STATES {
                assert!(
                    !is_legal_transition(from, to),
                    "terminal state {from:?} must have no outgoing edge to {to:?}"
                );
            }
        }
    }
}

#[test]
fn skipping_a_gate_is_an_illegal_transition() {
    let clock = FixedClock::new(0);
    let sink = MemoryAuditSink::new();
    let mut lifecycle = registered(&clock, &sink);
    // Discovered -> Started directly (skipping Verified/Compatible/Configured).
    let err = assert_error(
        lifecycle.start(pass(), "sdk-installer", "skipping every gate").unwrap_err(),
        "ILLEGAL_TRANSITION",
    );
    match &err {
        LifecycleError::IllegalTransition { from, to, .. } => {
            assert_eq!(*from, LifecycleState::Discovered);
            assert_eq!(*to, LifecycleState::Started);
        }
        other => panic!("unexpected error {other:?}"),
    }
    // The state is unchanged and the attempt is not silently committed.
    assert_eq!(lifecycle.state(), LifecycleState::Discovered);
}

#[test]
fn every_exit_method_respects_its_frozen_sources() {
    // reject() from Discovered: no Discovered -> Rejected edge exists.
    let clock = FixedClock::new(0);
    let sink = MemoryAuditSink::new();
    let mut lifecycle = registered(&clock, &sink);
    assert_error(
        lifecycle.reject("operator", "too early").unwrap_err(),
        "ILLEGAL_TRANSITION",
    );

    // fail() from Configured: §5.6 names Started and Ready only.
    let clock2 = FixedClock::new(0);
    let sink2 = MemoryAuditSink::new();
    let mut configured = registered(&clock2, &sink2);
    configured.verify(pass(), "a", "r").expect("verify");
    configured.mark_compatible(pass(), "a", "r").expect("compatible");
    configured.configure(pass(), "a", "r").expect("configure");
    assert_error(configured.fail("health-check", "not a §5.6 source").unwrap_err(), "ILLEGAL_TRANSITION");

    // complete_drain() from Ready (never began draining).
    let clock3 = FixedClock::new(0);
    let sink3 = MemoryAuditSink::new();
    let mut active = registered(&clock3, &sink3);
    to_ready(&mut active);
    assert_error(
        active.complete_drain("operator", "no drain was begun").unwrap_err(),
        "ILLEGAL_TRANSITION",
    );

    // restart_explicitly() from Ready: explicit restart is a Failed-only path.
    let clock5 = FixedClock::new(0);
    let sink5 = MemoryAuditSink::new();
    let mut active3 = registered(&clock5, &sink5);
    to_ready(&mut active3);
    let err = match active3.restart_explicitly("operator", "not failed") {
        Ok(_) => panic!("explicit restart from Ready must be refused"),
        Err(err) => err,
    };
    assert_error(err, "ILLEGAL_TRANSITION");

    // begin_drain() from Started: only Ready (or Failed) may drain.
    let clock6 = FixedClock::new(0);
    let sink6 = MemoryAuditSink::new();
    let mut started = registered(&clock6, &sink6);
    started.verify(pass(), "a", "r").expect("verify");
    started.mark_compatible(pass(), "a", "r").expect("compatible");
    started.configure(pass(), "a", "r").expect("configure");
    started.start(pass(), "a", "r").expect("start");
    let mut drain = ScriptedDrain::quiescing(0);
    assert_error(
        started.begin_drain(&mut drain, "operator", "Started cannot drain").unwrap_err(),
        "ILLEGAL_TRANSITION",
    );
}

#[test]
fn lifecycle_error_names_are_pairwise_distinct() {
    let names: Vec<&str> = vec![
        LifecycleError::IllegalTransition {
            from: LifecycleState::Discovered,
            to: LifecycleState::Started,
            reason: "r".to_string(),
        }
        .name(),
        LifecycleError::GateRefused {
            from: LifecycleState::Discovered,
            to: LifecycleState::Verified,
            gate: "PROVENANCE",
            reason: "r".to_string(),
        }
        .name(),
        LifecycleError::DrainPortMissing.name(),
        LifecycleError::DrainDeadlineExceeded { outstanding: 1 }.name(),
        LifecycleError::ReplacementBlockedUndrained {
            plugin_id: "p".to_string(),
            state: LifecycleState::Ready,
        }
        .name(),
    ];
    let mut sorted = names.clone();
    sorted.sort();
    for pair in sorted.windows(2) {
        assert_ne!(pair[0], pair[1], "lifecycle error names must be pairwise distinct");
    }
    assert_eq!(sorted.len(), 5);
}

// --- 3. Gate refusals are fail-closed and audited ----------------------------------

#[test]
fn a_failed_gate_refuses_the_transition_and_is_audited_then_the_exit_edge_applies() {
    let clock = FixedClock::new(5_000);
    let sink = MemoryAuditSink::new();
    let mut lifecycle = registered(&clock, &sink);

    lifecycle.verify(pass(), "a", "r").expect("verify");
    // Incompatible API at the compatibility gate.
    let err = assert_error(
        lifecycle
            .mark_compatible(failed_gate("api-version ranges are disjoint"), "sdk-installer", "compat check")
            .unwrap_err(),
        "GATE_REFUSED",
    );
    match &err {
        LifecycleError::GateRefused { from, to, gate, .. } => {
            assert_eq!(*from, LifecycleState::Verified);
            assert_eq!(*to, LifecycleState::Compatible);
            assert_eq!(*gate, "MANIFEST_API_COMPATIBILITY");
        }
        other => panic!("unexpected error {other:?}"),
    }
    // Fail-closed: state unchanged...
    assert_eq!(lifecycle.state(), LifecycleState::Verified);
    // ...the refusal is audited as an attempted edge...
    {
        let records = sink.records();
        let refused = records.last().expect("a refusal record exists");
        assert_eq!(refused.kind, LifecycleAuditKind::Refused);
        assert_eq!(refused.from, Some(LifecycleState::Verified));
        assert_eq!(refused.to, Some(LifecycleState::Compatible));
    }
    // ...and the §5.6 exit edge is applied EXPLICITLY: Verified -> Rejected.
    lifecycle.reject("sdk-installer", "incompatible API").expect("rejected");
    assert_eq!(lifecycle.state(), LifecycleState::Rejected);
    assert!(lifecycle.state().is_terminal());
}

#[test]
fn the_provenance_gate_rejects_at_the_earliest_point() {
    let clock = FixedClock::new(0);
    let sink = MemoryAuditSink::new();
    let mut lifecycle = registered(&clock, &sink);
    let err = assert_error(
        lifecycle.verify(failed_gate("unsigned package"), "sdk-installer", "P7.14 hookpoint").unwrap_err(),
        "GATE_REFUSED",
    );
    match &err {
        LifecycleError::GateRefused { gate, .. } => assert_eq!(*gate, "PROVENANCE"),
        other => panic!("unexpected error {other:?}"),
    }
    // No Verified -> Rejected edge applies here: the plugin never advanced
    // past Discovered, so it simply never proceeds (recorded DEFAULT — see
    // the decision record; the §5.6 Rejected edges are from Verified and
    // Compatible only).
    assert_eq!(lifecycle.state(), LifecycleState::Discovered);
    // And there is no Discovered -> Rejected edge to fall back on either.
    assert_error(
        lifecycle.reject("sdk-installer", "no Discovered -> Rejected edge").unwrap_err(),
        "ILLEGAL_TRANSITION",
    );
}

// --- 4. Drain-before-replace ---------------------------------------------------------

#[test]
fn drain_before_replace_blocks_the_replacement_until_quiescence() {
    let clock = FixedClock::new(1_000);
    let sink = MemoryAuditSink::new();
    let mut lifecycle = registered(&clock, &sink);
    to_ready(&mut lifecycle);

    // Active (Ready) without draining: the replacement is REFUSED, named.
    let err = assert_error(
        lifecycle.require_fully_drained("operator", "swap attempt while active").unwrap_err(),
        "REPLACEMENT_BLOCKED_UNDRAINED",
    );
    match &err {
        LifecycleError::ReplacementBlockedUndrained { plugin_id, state } => {
            assert_eq!(plugin_id, PLUGIN_ID);
            assert_eq!(*state, LifecycleState::Ready);
        }
        other => panic!("unexpected error {other:?}"),
    }
    assert_eq!(lifecycle.state(), LifecycleState::Ready, "the incumbent stays active");

    // Draining but not yet quiesced: still blocked.
    let mut drain = ScriptedDrain::stuck(3);
    lifecycle.begin_drain(&mut drain, "operator", "upgrade requested").expect("draining");
    assert_error(
        lifecycle.require_fully_drained("operator", "swap attempt mid-drain").unwrap_err(),
        "REPLACEMENT_BLOCKED_UNDRAINED",
    );
    assert_eq!(lifecycle.state(), LifecycleState::Draining);
}

#[test]
fn drain_deadline_expiry_refuses_the_stop_and_leaves_the_incumbent_active() {
    let clock = FixedClock::new(2_000);
    let sink = MemoryAuditSink::new();
    let mut lifecycle = registered(&clock, &sink);
    to_ready(&mut lifecycle);

    let mut drain = ScriptedDrain::stuck(4);
    lifecycle.begin_drain(&mut drain, "operator", "upgrade requested").expect("draining");
    let err = assert_error(
        lifecycle.complete_drain("operator", "attempted stop").unwrap_err(),
        "DRAIN_DEADLINE_EXCEEDED",
    );
    match &err {
        LifecycleError::DrainDeadlineExceeded { outstanding } => assert_eq!(*outstanding, 4),
        other => panic!("unexpected error {other:?}"),
    }
    // Fail-closed: the stop was NOT committed; the incumbent is active
    // (Ready) again — the operator decides. Nothing was forced.
    assert_eq!(lifecycle.state(), LifecycleState::Ready);
    {
        let records = sink.records();
        let refusal = &records[records.len() - 2];
        assert_eq!(refusal.kind, LifecycleAuditKind::Refused);
        assert_eq!(refusal.from, Some(LifecycleState::Draining));
        assert_eq!(refusal.to, Some(LifecycleState::Stopped));
        assert!(refusal.reason.contains("4"));
        let restoration = records.last().expect("restoration record");
        assert_eq!(restoration.kind, LifecycleAuditKind::Transition);
        assert_eq!(restoration.from, Some(LifecycleState::Draining));
        assert_eq!(restoration.to, Some(LifecycleState::Ready));
    }

    // The operator decides: the drain is retried and this time the plugin
    // quiesces — only then does the replacement path open.
    let (mut drain2, _observed2) = ScriptedDrain::quiescing(4);
    lifecycle.begin_drain(&mut drain2, "operator", "operator-approved retry").expect("draining again");
    lifecycle.complete_drain("operator", "active work completed").expect("quiesced");
    assert_eq!(lifecycle.state(), LifecycleState::Stopped);
    lifecycle.require_fully_drained("operator", "swap now allowed").expect("fully drained");
}

#[test]
fn a_failed_plugin_drains_before_removal() {
    // §5.6: Failed -> Draining (a Failed plugin that never drains would
    // block the registry — the ledger's P7.07 risk).
    let clock = FixedClock::new(0);
    let sink = MemoryAuditSink::new();
    let mut lifecycle = registered(&clock, &sink);
    lifecycle.verify(pass(), "a", "r").expect("verify");
    lifecycle.fail("health-check", "initialization failure").expect("failed");
    assert_eq!(lifecycle.state(), LifecycleState::Failed);

    let mut drain = ScriptedDrain::quiescing(0);
    lifecycle.begin_drain(&mut drain, "operator", "failed-plugin cleanup").expect("Failed -> Draining");
    lifecycle.complete_drain("operator", "nothing outstanding").expect("stopped");
    assert_eq!(lifecycle.state(), LifecycleState::Stopped);
}

// --- 5. Crash handling and the explicit-restart path ----------------------------------

#[test]
fn crash_enters_failed_from_any_non_terminal_state_and_holds_the_last_state() {
    for start_state in [
        LifecycleState::Discovered,
        LifecycleState::Verified,
        LifecycleState::Compatible,
        LifecycleState::Configured,
        LifecycleState::Started,
        LifecycleState::Ready,
        LifecycleState::Draining,
    ] {
        let clock = FixedClock::new(0);
        let sink = MemoryAuditSink::new();
        let mut lifecycle = registered(&clock, &sink);
        // Walk to the requested start state (Draining via Ready).
        if start_state != LifecycleState::Discovered {
            lifecycle.verify(pass(), "a", "r").expect("verify");
        }
        if matches!(
            start_state,
            LifecycleState::Compatible | LifecycleState::Configured | LifecycleState::Started
                | LifecycleState::Ready | LifecycleState::Draining
        ) {
            lifecycle.mark_compatible(pass(), "a", "r").expect("compatible");
        }
        if matches!(
            start_state,
            LifecycleState::Configured | LifecycleState::Started | LifecycleState::Ready
                | LifecycleState::Draining
        ) {
            lifecycle.configure(pass(), "a", "r").expect("configure");
        }
        if matches!(
            start_state,
            LifecycleState::Started | LifecycleState::Ready | LifecycleState::Draining
        ) {
            lifecycle.start(pass(), "a", "r").expect("start");
        }
        if matches!(start_state, LifecycleState::Ready | LifecycleState::Draining) {
            lifecycle.mark_ready(pass(), "a", "r").expect("ready");
        }
        if start_state == LifecycleState::Draining {
            let mut drain = ScriptedDrain::stuck(1);
            lifecycle.begin_drain(&mut drain, "operator", "upgrade").expect("draining");
        }
        assert_eq!(lifecycle.state(), start_state);

        lifecycle.crash("host", "plugin-host crash (P7.30 fault)").expect("crash recorded");
        assert_eq!(lifecycle.state(), LifecycleState::Failed);
        assert_eq!(lifecycle.failed_from(), Some(start_state), "the failed plugin holds its last state for the drill");
    }
}

#[test]
fn crash_is_refused_from_terminal_states() {
    let clock = FixedClock::new(0);
    let sink = MemoryAuditSink::new();
    let mut lifecycle = registered(&clock, &sink);
    lifecycle.verify(pass(), "a", "r").expect("verify");
    lifecycle.mark_compatible(pass(), "a", "r").expect("compatible");
    lifecycle.reject("sdk-installer", "incompatible API").expect("rejected");
    assert_error(
        lifecycle.crash("host", "a rejected plugin cannot crash — it already holds its outcome").unwrap_err(),
        "ILLEGAL_TRANSITION",
    );
}

#[test]
fn the_only_restart_path_is_explicit_and_audited_and_there_is_no_auto_restart() {
    let clock = FixedClock::new(7_000);
    let sink = MemoryAuditSink::new();
    let mut lifecycle = registered(&clock, &sink);
    lifecycle.verify(pass(), "a", "r").expect("verify");
    lifecycle.mark_compatible(pass(), "a", "r").expect("compatible");
    lifecycle.configure(pass(), "a", "r").expect("configure");
    lifecycle.fail("health-check", "initialization failure").expect("failed");

    // NO auto-restart: the gated forward step out of Failed is illegal.
    assert_error(
        lifecycle.start(pass(), "host", "auto-restart attempt").unwrap_err(),
        "ILLEGAL_TRANSITION",
    );
    assert_eq!(lifecycle.state(), LifecycleState::Failed, "still Failed — nothing auto-restarted");

    // The EXPLICIT path: audited, and the fresh lifecycle starts over.
    let restarted = lifecycle
        .restart_explicitly("operator", "operator-approved restart after fix")
        .expect("explicit restart");
    assert_eq!(restarted.state(), LifecycleState::Discovered);
    assert_eq!(restarted.plugin_id(), PLUGIN_ID);
    let records = sink.records();
    let restart_record = records.last().expect("restart record");
    assert_eq!(restart_record.kind, LifecycleAuditKind::ExplicitRestart);
    assert_eq!(restart_record.from, Some(LifecycleState::Failed));
    assert_eq!(restart_record.to, Some(LifecycleState::Discovered));
    assert_eq!(restart_record.timestamp_millis, 7_000);
}

// --- 6. Clock-injected determinism -----------------------------------------------------

/// One full lifecycle run against fresh fixtures; returns the audit trail.
fn run_deterministic_lifecycle() -> Vec<LifecycleAuditRecord> {
    let clock = FixedClock::new(1_000);
    let sink = MemoryAuditSink::new();
    let mut lifecycle = registered(&clock, &sink);
    to_ready(&mut lifecycle);
    clock.advance_by(100);
    let (mut drain, _observed) = ScriptedDrain::quiescing(1);
    lifecycle.begin_drain(&mut drain, "operator", "upgrade requested").expect("draining");
    clock.advance_by(50);
    lifecycle.complete_drain("operator", "quiesced").expect("stopped");
    sink.records().iter().cloned().collect()
}

#[test]
fn the_same_injected_clock_yields_the_identical_audit_sequence() {
    let run1 = run_deterministic_lifecycle();
    let run2 = run_deterministic_lifecycle();
    assert_eq!(run1, run2, "same clock readings must give the same audit ordering and timestamps");
    assert_eq!(run1.len(), 8);
    // The advanced clock readings are part of the deterministic output.
    assert_eq!(run1[6].timestamp_millis, 1_100, "the drain-start record carries the advanced reading");
    assert_eq!(run1[7].timestamp_millis, 1_150, "the stop record carries the second advance");
}
