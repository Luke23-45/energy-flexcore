// P18.14 — Integration tests (WRITTEN-UNRUN; execution is a human gate):
//          CONCURRENT-COMMAND BLOCKING tests, consuming Agent B's LANDED
//          P18.15 rule (`crates/edge-agent/src/update_command_interlock.rs`,
//          decision record `docs/phases-11-20/phase-18-edge-fleet-lifecycle-
//          ota/records/28_p18_15_update_command_interlock.md` §4 — the
//          wait-check PASSED: the interlock module, record 28, and B's own
//          integration file landed during this agent's session) and testing
//          the EXTENDED GATE's behavior under the interlock — the
//          composition B's own test file does not cover (it exercises the
//          interlock against the session rule and the P6.13 harness; this
//          file exercises the interlock against the MOCK DEVICE's update
//          path: the P18.05 extended gate, the P18.02 counter, the P9.34
//          register).
//
// THE HONESTY BOUNDARY: no command exists, no device exists, no update has
// ever been produced, signed, staged, or applied, no fleet exists — every
// flow below runs against `MockUpdateDevice` (a TEST DOUBLE) and
// `InMemoryCommandStateSource` (B's labeled TEST DOUBLE). The command
// lifecycle is never written by the update path (the interlock is
// OBSERVER-only; the operator's frozen-lifecycle actions are simulated by
// changing what the in-memory source REPORTS, never by the update code).
//
// THE MATRIX (each test hand-computed; the two-step composition is record
// 28 §4's):
// - a command ISSUED/TRACKING on the device blocks the attempt
//   (`UPDATE_BLOCKED_COMMANDS_IN_FLIGHT` — ONE stable code for the wave-1
//   session rule and this file) and the extended gate is NEVER REACHED
//   (no verification, no counter movement, no register record);
// - the drain-gap states (ACKNOWLEDGED/FAILED/EXPIRED) block too — the
//   recorded strict-superset strengthening;
// - an UNKNOWN device command state fails closed before anything runs;
// - the drain walk: ISSUED → (the operator's frozen-lifecycle action,
//   simulated in the source) CANCELLED → drained → authorized → the
//   two-step composition feeds `begin_download(0, now)` → the extended
//   gate verifies → the register records the activation;
// - the post-update conformance re-check: after `record_update_activated`,
//   the NEXT attempt is blocked (`UPDATE_CONFORMANCE_RECHECK_OUTSTANDING`)
//   even with zero commands in flight; a CLEAN §19.2(6) report (consumed
//   from the P6.13 module — never reimplemented) re-opens authorization;
//   the claim boundary stands: the clean report proves the §19.2(6) edge
//   read-path contract ONLY — never protocol conformance, never
//   certification.

mod p18_14_support;

use std::sync::Arc;

use flexcore_ports::clock::Clock;
use flexcore_ports::fakes::FixedClock;

use flexcore_edge_agent::conformance::{CaseOutcome, CaseRecord, ConformanceCase, ConformanceReport};
use flexcore_edge_agent::ports::AuditSink;
use flexcore_edge_agent::update_command_interlock::{
    DeviceCommandObservation, InMemoryCommandStateSource, RecheckPhase, UpdateBlockRefusal,
    UpdateCommandInterlock,
};
use flexcore_edge_agent::update_session::{
    UpdateProgressState, UpdateSessionStateMachine, UPDATE_FAILURE_VERIFICATION_REFUSED,
};

use p18_14_support::{
    cleanup, fixture_bundle, v2_manifest, MockUpdateDevice, MockVerifierChoice, BASE_MILLIS,
};

const VALID_EXPIRY: i64 = BASE_MILLIS + 10 * 365 * 24 * 60 * 60_000;

// --- Fixtures ---------------------------------------------------------------------

fn observation(command_id: &str, state_name: &str) -> DeviceCommandObservation {
    DeviceCommandObservation {
        command_id: command_id.to_string(),
        state_name: state_name.to_string(),
    }
}

/// The interlock composed the way P18.14 consumes it: B's in-memory
/// source (the labeled TEST DOUBLE), the mock's audit sink (so the
/// interlock's lines land in the same audit the device's gate writes), a
/// fixed injected clock.
fn interlock_for(device: &MockUpdateDevice, source: Arc<InMemoryCommandStateSource>)
-> UpdateCommandInterlock {
    let audit: Arc<dyn AuditSink> = device.audit.clone();
    let clock: Arc<dyn Clock + Send + Sync> = Arc::new(FixedClock::at_unix_millis(BASE_MILLIS));
    UpdateCommandInterlock::new(source, audit, clock)
}

fn clean_report() -> ConformanceReport {
    ConformanceReport {
        records: vec![CaseRecord {
            case: ConformanceCase::Valid,
            point: "ess0/ActivePower".to_string(),
            outcome: CaseOutcome::Accepted,
        }],
    }
}

/// The two-step composition from record 28 §4, applied to the mock
/// device: the interlock verdict feeds the UNCHANGED wave-1 session rule,
/// then the session walks download → present-to-gate → verify-passed.
/// Returns the session (in `VerifiedPendingInstall`); any refusal along
/// the way is a test failure via `expect` (the callers assert the
/// positive path; the negative paths have their own tests).
fn authorize_download_and_verify(
    device: &MockUpdateDevice,
    interlock: &UpdateCommandInterlock,
    attempt_id: &str,
    sequence: u64,
    bundle: &[u8],
    manifest: &flexcore_edge_agent::update_bundle::ExtendedUpdateManifest,
) -> UpdateSessionStateMachine {
    let authorization = interlock
        .authorize_update_attempt()
        .expect("the interlock authorizes the drained device");
    assert_eq!(authorization.in_flight_count, 0, "the verdict is always 0 on Ok");
    let mut session = UpdateSessionStateMachine::new(
        attempt_id,
        device.device_id(),
        &manifest.base.version,
        sequence,
        device.now_unix_millis(),
    )
    .expect("session opens");
    session
        .begin_download(authorization.in_flight_count, device.now_unix_millis() + 100)
        .expect("the session rule accepts the interlock verdict");
    session
        .download_complete(device.now_unix_millis() + 200)
        .expect("download completes");
    device
        .present_bundle(Some(manifest), bundle)
        .expect("the extended gate verifies");
    session
        .verify_passed(device.now_unix_millis() + 300)
        .expect("the session records the pass");
    session
}

// --- 1. The blocking rule reaches the mock device ------------------------------------

#[test]
fn a_tracked_command_blocks_before_the_extended_gate_is_reached() {
    let device = MockUpdateDevice::new("block-gate", MockVerifierChoice::TestOnlyFnv1a64);
    let source = Arc::new(InMemoryCommandStateSource::reporting(vec![observation(
        "cmd-1",
        "TRACKING",
    )]));
    let interlock = interlock_for(&device, source);

    // THE BLOCK: the refusal carries the recorded stable code.
    let refusal = interlock.authorize_update_attempt().expect_err("blocked");
    assert_eq!(
        refusal,
        UpdateBlockRefusal::CommandsInFlight { issued_or_tracking: 1 }
    );
    assert_eq!(refusal.name(), "UPDATE_BLOCKED_COMMANDS_IN_FLIGHT");

    // The update path NEVER RAN: the extended gate was not reached (no
    // verification happened), the counter did not move, the register is
    // empty, and no session was composed.
    assert_eq!(device.last_seen_sequence(), 0, "no verification ran");
    assert!(device.open_ledger().is_empty(), "no activation was recorded");
    let lines: Vec<String> = device
        .audit
        .events()
        .iter()
        .map(|event| event.detail.clone())
        .collect();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("EDGE_UPDATE_INTERLOCK_BLOCKED")
                && l.contains("UPDATE_BLOCKED_COMMANDS_IN_FLIGHT")),
        "the blocking refusal is audited: {lines:?}"
    );
    assert!(
        !lines.iter().any(|l| l.contains("EDGE_FLEET_UPDATE_VERIFIED")),
        "no verification is audited for the blocked attempt: {lines:?}"
    );
    cleanup(device.scratch());
}

#[test]
fn every_in_flight_command_count_is_carried_in_the_refusal() {
    let device = MockUpdateDevice::new("block-count", MockVerifierChoice::TestOnlyFnv1a64);
    let source = Arc::new(InMemoryCommandStateSource::reporting(vec![
        observation("cmd-1", "ISSUED"),
        observation("cmd-2", "TRACKING"),
        observation("cmd-3", "ACKNOWLEDGED"),
    ]));
    let interlock = interlock_for(&device, source);
    let refusal = interlock.authorize_update_attempt().expect_err("blocked");
    assert_eq!(
        refusal,
        UpdateBlockRefusal::CommandsInFlight { issued_or_tracking: 3 },
        "the count includes the drain-gap state (the recorded strengthening)"
    );
    cleanup(device.scratch());
}

#[test]
fn the_drain_gap_states_block_the_mock_device_update_too() {
    // The recorded strict-superset rule (record 28 §2): ACKNOWLEDGED,
    // FAILED, and EXPIRED are non-terminal in the frozen table and block.
    for state in ["ACKNOWLEDGED", "FAILED", "EXPIRED"] {
        let device = MockUpdateDevice::new("gap-block", MockVerifierChoice::TestOnlyFnv1a64);
        let source = Arc::new(InMemoryCommandStateSource::reporting(vec![observation(
            "cmd-1",
            state,
        )]));
        let interlock = interlock_for(&device, source);
        assert!(matches!(
            interlock.authorize_update_attempt(),
            Err(UpdateBlockRefusal::CommandsInFlight { issued_or_tracking: 1 })
        ));
        assert_eq!(device.last_seen_sequence(), 0);
        assert!(device.open_ledger().is_empty());
        cleanup(device.scratch());
    }
}

#[test]
fn an_unknown_device_command_state_fails_closed_before_anything_runs() {
    let device = MockUpdateDevice::new("unknown-state", MockVerifierChoice::TestOnlyFnv1a64);
    let source = Arc::new(InMemoryCommandStateSource::default());
    source.set_unobservable("the command-path tracking map is unreachable".to_string());
    let interlock = interlock_for(&device, source);
    let refusal = interlock.authorize_update_attempt().expect_err("blocked");
    assert!(matches!(refusal, UpdateBlockRefusal::DeviceStateUnknown { .. }));
    assert_eq!(refusal.name(), "UPDATE_INTERLOCK_STATE_UNKNOWN");
    // Fail-closed end to end: the extended gate never ran, nothing moved.
    assert_eq!(device.last_seen_sequence(), 0);
    assert!(device.open_ledger().is_empty());
    cleanup(device.scratch());
}

// --- 2. The drain walk, ending in a REAL verification ---------------------------------

#[test]
fn the_drain_walk_ends_in_an_extended_gate_verification_and_a_register_activation() {
    let device = MockUpdateDevice::new("drain-walk", MockVerifierChoice::TestOnlyFnv1a64);
    let bundle = fixture_bundle();
    let manifest = v2_manifest("1.2.0", "1.0.0", 5, VALID_EXPIRY, &bundle);
    let source = Arc::new(InMemoryCommandStateSource::reporting(vec![observation(
        "cmd-1",
        "ISSUED",
    )]));
    let mut interlock = interlock_for(&device, source.clone());

    // Blocked while ISSUED.
    assert!(matches!(
        interlock.authorize_update_attempt(),
        Err(UpdateBlockRefusal::CommandsInFlight { issued_or_tracking: 1 })
    ));
    // The report §5.6 drain: start, still draining, no new command work.
    let phase = interlock.begin_drain().expect("drain starts").clone();
    assert!(matches!(phase, flexcore_edge_agent::update_command_interlock::DrainPhase::Draining { .. }));
    assert!(!interlock.allows_new_command_work());
    assert!(matches!(
        interlock.authorize_update_attempt(),
        Err(UpdateBlockRefusal::CommandsInFlight { issued_or_tracking: 1 })
    ));
    // The OPERATOR's "finish or cancel pending operations" leg (a frozen
    // lifecycle action, ISSUED → CANCELLED) — simulated by changing what
    // the in-memory source REPORTS; the update path never wrote it.
    source.set_observations(vec![observation("cmd-1", "CANCELLED")]);
    let phase = interlock.poll_drain().expect("drained").clone();
    assert!(matches!(
        phase,
        flexcore_edge_agent::update_command_interlock::DrainPhase::Drained { .. }
    ));
    assert!(interlock.allows_new_command_work());

    // The drained device authorizes: the two-step composition feeds
    // `begin_download(0, now)`, the session walks, and the EXTENDED GATE
    // verifies (the counter advances durably).
    let session = authorize_download_and_verify(&device, &interlock, "attempt-drain-1", 5, &bundle, &manifest);
    assert_eq!(session.state(), UpdateProgressState::VerifiedPendingInstall);
    assert_eq!(device.last_seen_sequence(), 5);
    // Re-presenting the SAME manifest is refused (the sequence was
    // consumed by the pass — the anti-rollback rule composes with the
    // interlock, never under it).
    let replay_error = device
        .present_bundle(Some(&manifest), &bundle)
        .expect_err("the consumed manifest cannot be re-presented");
    assert!(matches!(
        replay_error,
        flexcore_edge_agent::update_bundle::UpdateBundleError::SequenceNotAhead { .. }
    ));
    // A SECOND drained attempt (fresh sequence) verifies too: the drain
    // is per-attempt, the interlock re-reads the state each time.
    let manifest_6 = v2_manifest("1.3.0", "1.0.0", 6, VALID_EXPIRY, &bundle);
    let session_6 = authorize_download_and_verify(
        &device, &interlock, "attempt-drain-2", 6, &bundle, &manifest_6,
    );
    assert_eq!(session_6.state(), UpdateProgressState::VerifiedPendingInstall);
    assert_eq!(device.last_seen_sequence(), 6);
    let lines: Vec<String> = device
        .audit
        .events()
        .iter()
        .map(|event| event.detail.clone())
        .collect();
    assert!(lines.iter().any(|l| l.contains("EDGE_FLEET_UPDATE_VERIFIED")));
    assert!(lines.iter().any(|l| l.contains("EDGE_UPDATE_DRAIN_COMPLETE")));
    cleanup(device.scratch());
}

// --- 3. The post-update conformance re-check gates the NEXT update ----------------------

#[test]
fn after_activation_the_next_update_is_blocked_until_a_clean_recheck_is_recorded() {
    let device = MockUpdateDevice::new("recheck-gate", MockVerifierChoice::TestOnlyFnv1a64);
    let bundle = fixture_bundle();
    let source = Arc::new(InMemoryCommandStateSource::reporting(vec![]));
    let mut interlock = interlock_for(&device, source);

    // Attempt 1: authorized, verified, and the activation is recorded
    // into the interlock (the composition's post-update hook).
    let manifest_1 = v2_manifest("1.2.0", "1.0.0", 5, VALID_EXPIRY, &bundle);
    let session_1 = authorize_download_and_verify(
        &device, &interlock, "attempt-r1", 5, &bundle, &manifest_1,
    );
    // The session reaches Active (terminal success) in the composition;
    // record the activation with the interlock.
    let mut session_1 = session_1;
    session_1.begin_install(device.now_unix_millis() + 400).expect("ok");
    session_1.install_complete(device.now_unix_millis() + 500).expect("ok");
    session_1.health_confirmed(device.now_unix_millis() + 600).expect("Active");
    interlock
        .record_update_activated("attempt-r1")
        .expect("the activation is recorded");

    // The NEXT update is blocked by the re-check even with ZERO commands
    // in flight (the blocking order: re-check first, then command state).
    let refusal = interlock.authorize_update_attempt().expect_err("blocked");
    assert!(matches!(
        refusal,
        UpdateBlockRefusal::ConformanceRecheckOutstanding { ref attempt_id } if attempt_id == "attempt-r1"
    ));
    assert_eq!(refusal.name(), "UPDATE_CONFORMANCE_RECHECK_OUTSTANDING");
    assert_eq!(device.last_seen_sequence(), 5, "the blocked attempt verified nothing new");

    // A CLEAN §19.2(6) report (the P6.13 module's shape, consumed) is
    // recorded → the re-check is Satisfied → authorization re-opens.
    let phase = interlock
        .record_conformance_report(&clean_report())
        .expect("recorded")
        .clone();
    assert_eq!(phase, RecheckPhase::Satisfied);
    let manifest_2 = v2_manifest("1.3.0", "1.0.0", 6, VALID_EXPIRY, &bundle);
    authorize_download_and_verify(&device, &interlock, "attempt-r2", 6, &bundle, &manifest_2);
    assert_eq!(device.last_seen_sequence(), 6);

    // The claim boundary, pinned at the composition level: the audit text
    // of the re-check pass names the P6.13 contract ONLY — never
    // protocol conformance, never certification.
    let lines: Vec<String> = device
        .audit
        .events()
        .iter()
        .map(|event| event.detail.clone())
        .collect();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("EDGE_UPDATE_CONFORMANCE_RECHECK_PASSED")
                && l.contains("never protocol conformance, never certification")),
        "the claim boundary rides the audit line: {lines:?}"
    );
    cleanup(device.scratch());
}

#[test]
fn a_blocked_session_refusal_keeps_the_explicit_failure_state_named() {
    // When the composition hands a non-zero count to the session rule
    // (a defect — the interlock would never do that), the wave-1 rule
    // still refuses by name: the two rules cannot disagree in the unsafe
    // direction. The session records the explicit verification-refused
    // failure when the EXTENDED GATE refuses after download.
    let device = MockUpdateDevice::new("failure-state", MockVerifierChoice::TestOnlyFnv1a64);
    let bundle = fixture_bundle();
    let mut manifest = v2_manifest("1.2.0", "1.0.0", 5, VALID_EXPIRY, &bundle);
    // Corrupt the payload digest so the gate refuses AFTER the download.
    manifest.payloads[0].digest_hex = "0000000000000000".to_string();
    let source = Arc::new(InMemoryCommandStateSource::reporting(vec![]));
    let interlock = interlock_for(&device, source);
    let mut session = UpdateSessionStateMachine::new(
        "attempt-f1",
        device.device_id(),
        "1.2.0",
        5,
        device.now_unix_millis(),
    )
    .expect("session opens");
    session
        .begin_download(0, device.now_unix_millis() + 100)
        .expect("no commands in flight — download begins");
    session
        .download_complete(device.now_unix_millis() + 200)
        .expect("download completes");
    // The extended gate refuses: the session records the named failure.
    assert!(device.present_bundle(Some(&manifest), &bundle).is_err());
    session
        .verify_refused(device.now_unix_millis() + 300)
        .expect("the refusal is a session failure state");
    assert_eq!(
        session.failure_state(),
        Some(UPDATE_FAILURE_VERIFICATION_REFUSED)
    );
    assert_eq!(device.last_seen_sequence(), 0, "a refused gate never advanced");
    assert!(device.open_ledger().is_empty());
    cleanup(device.scratch());
}

#[test]
fn the_interlock_statement_pins_the_observer_only_rule() {
    // B's module tests pin the statement text; this file pins the
    // observer-only phrase and the composed behavior.
    assert!(flexcore_edge_agent::update_command_interlock::INTERLOCK_STATEMENT
        .contains("as an OBSERVER"));
    // The composed behavior pin: with a command in flight, the mock
    // device's audit shows the interlock refusal and NOTHING else from
    // the update path.
    let device = MockUpdateDevice::new("observer-pin", MockVerifierChoice::TestOnlyFnv1a64);
    let source = Arc::new(InMemoryCommandStateSource::reporting(vec![observation(
        "cmd-1",
        "ISSUED",
    )]));
    let interlock = interlock_for(&device, source);
    let _ = interlock.authorize_update_attempt().expect_err("blocked");
    let lines: Vec<String> = device
        .audit
        .events()
        .iter()
        .map(|event| event.detail.clone())
        .collect();
    let update_path_lines = lines
        .iter()
        .filter(|l| l.contains("EDGE_FLEET_UPDATE_") || l.contains("EDGE_UPDATE_VERIFIED"))
        .count();
    assert_eq!(
        update_path_lines, 0,
        "the observer-only interlock wrote no update-path lines: {lines:?}"
    );
    cleanup(device.scratch());
}
