// P18.14 — Integration tests (WRITTEN-UNRUN; execution is a human gate):
//          ROLLBACK SCENARIOS + INTERRUPTED-UPDATE (POWER-LOSS)
//          SIMULATION for the phase-18 update path (plan §2 P18.14;
//          decision record `docs/phases-11-20/phase-18-edge-fleet-
//          lifecycle-ota/records/41_p18_14_update_test_suite_mock_device.md`).
//
// THE HONESTY BOUNDARY: no device exists — the flows below run against
// `MockUpdateDevice`, a TEST DOUBLE composing the real library surfaces
// (P9.34 register/activation, P18.02 counter, P18.04 state machine,
// P18.05 extended gate) over temp-dir files. An "activation" here is an
// append to the mock's register — a RECORD, never a physical install; NO
// UPDATE HAS EVER BEEN PRODUCED, SIGNED, STAGED, OR APPLIED anywhere in
// this program. The A/B slot mechanics, watchdogs, and boot counters are
// DEVICE concerns the platform SPECIFIES but does not implement (plan §4
// non-goals; `UPDATE_SLOT_DECISION`): the health-check evidence in these
// scenarios is a fixture boolean, not hardware behavior, and every
// hardware determination stays PENDING PILOT HARDWARE SPEC (P6.34).
//
// THE MATRIX (each test hand-computed):
// - the whole recorded flow interleaved: session download → verify (the
//   extended gate) → session verify-passed → register activation →
//   install → health-confirm → Active (terminal), the register reporting
//   the new version throughout;
// - the AUTO-ROLLBACK flow: health-failed → RollingBack → register
//   rollback with the recorded reason → RolledBack (terminal), the active
//   version reverting to the previously-activated one;
// - the never-silent-downgrade refusals: never-activated target,
//   not-behind target, below-floor without an override; the override
//   riding the register record AND the audit line when presented;
// - POWER LOSS #1 (between the durable counter advance and the
//   install): on reboot the same manifest is a replay — refused; the
//   recorded recovery is re-issued metadata with a higher sequence;
// - POWER LOSS #2 (mid-append on the register): the torn tail is
//   truncated and counted, the good activation survives the replay;
// - POWER LOSS #3 (after activation, before health): the register's
//   active version is derived by replay on reboot — the re-activation
//   (rollback) target set is intact;
// - a cancel at VerifiedPendingInstall ends the session with the named
//   CANCELLED failure state and leaves ZERO register records.

mod p18_14_support;

use flexcore_edge_agent::update_session::{
    UpdateProgressState, UpdateSessionError, UpdateSessionStateMachine,
    UPDATE_FAILURE_CANCELLED, UPDATE_FAILURE_VERIFICATION_REFUSED,
};
use flexcore_edge_agent::update_bundle::UpdateBundleError;
use flexcore_edge_agent::update_verification::{
    FileUpdateLedger, RollbackOverride, UpdateLedgerEventKind, UpdateLedgerRecord,
    UpdateVerificationError,
};

use p18_14_support::{
    cleanup, fixture_bundle, v2_manifest, MockUpdateDevice, MockVerifierChoice, BASE_MILLIS,
};

const VALID_EXPIRY: i64 = BASE_MILLIS + 10 * 365 * 24 * 60 * 60_000;

// --- Flow helpers -----------------------------------------------------------------

/// Drives the session from Idle through download and presents the bundle
/// to the extended gate, returning the session (in `Verifying`) and the
/// verification verdict.
fn download_and_verify(
    device: &MockUpdateDevice,
    session: &mut UpdateSessionStateMachine,
    bundle: &[u8],
    manifest: &flexcore_edge_agent::update_bundle::ExtendedUpdateManifest,
) -> Result<
    flexcore_edge_agent::update_session::VerifiedExtendedUpdate,
    flexcore_edge_agent::update_bundle::UpdateBundleError,
> {
    session
        .begin_download(0, device.now_unix_millis() + 100)
        .expect("download begins (no commands in flight)");
    session
        .download_complete(device.now_unix_millis() + 200)
        .expect("download completes (the session is now Verifying)");
    device.present_bundle(Some(manifest), bundle)
}

/// The audit lines the mock collected.
fn audit_lines(device: &MockUpdateDevice) -> Vec<String> {
    device
        .audit
        .events()
        .iter()
        .map(|event| event.detail.clone())
        .collect()
}

// --- 1. The whole recorded flow, interleaved -----------------------------------------

#[test]
fn the_full_recorded_flow_walks_session_gate_and_register_together() {
    let device = MockUpdateDevice::new("full-flow", MockVerifierChoice::TestOnlyFnv1a64);
    let bundle = fixture_bundle();
    let manifest = v2_manifest("1.2.0", "1.0.0", 5, VALID_EXPIRY, &bundle);

    let mut session = UpdateSessionStateMachine::new(
        "attempt-full-1",
        device.device_id(),
        "1.2.0",
        5,
        device.now_unix_millis(),
    )
    .expect("session opens");

    let verified = download_and_verify(&device, &mut session, &bundle, &manifest)
        .expect("verification passes");
    session
        .verify_passed(device.now_unix_millis() + 300)
        .expect("the session records the pass");
    assert_eq!(session.state(), UpdateProgressState::VerifiedPendingInstall);

    // The register records the ACTIVATION (a RECORD — never a physical
    // install; the P9.34 ledger step 2 surface).
    let activation = device.activation();
    let record = activation
        .activate(&device.open_ledger(), &verified.base)
        .expect("activation recorded");
    assert_eq!(record.kind, UpdateLedgerEventKind::Activation);
    assert_eq!(record.version, "1.2.0");
    assert_eq!(record.previous_version, "", "the first activation has no predecessor");
    assert_eq!(record.approval_or_reason, "fixture-approval-001");

    session
        .begin_install(device.now_unix_millis() + 400)
        .expect("install begins");
    session
        .install_complete(device.now_unix_millis() + 500)
        .expect("install completes");
    session
        .health_confirmed(device.now_unix_millis() + 600)
        .expect("health confirms");
    assert_eq!(session.state(), UpdateProgressState::Active);
    assert!(session.state().is_terminal());

    // The register is the source of truth for the active version.
    let ledger = device.open_ledger();
    assert_eq!(ledger.active_version().as_deref(), Some("1.2.0"));
    // The audit carries the three honest layers.
    let lines = audit_lines(&device);
    assert!(lines.iter().any(|l| l.contains("EDGE_FLEET_UPDATE_VERIFIED")));
    assert!(lines.iter().any(|l| l.contains("EDGE_UPDATE_VERIFIED")));
    assert!(lines.iter().any(|l| l.contains("EDGE_UPDATE_ACTIVATED")));
    assert!(lines.iter().any(|l| l.contains("EDGE_FLEET_UPDATE_SEQUENCE_ADVANCED")));
    cleanup(device.scratch());
}

#[test]
fn a_duplicate_activation_of_the_active_version_is_refused() {
    let device = MockUpdateDevice::new("dup-activation", MockVerifierChoice::TestOnlyFnv1a64);
    let bundle = fixture_bundle();
    let manifest = v2_manifest("1.2.0", "1.0.0", 5, VALID_EXPIRY, &bundle);
    let mut session = UpdateSessionStateMachine::new(
        "attempt-dup",
        device.device_id(),
        "1.2.0",
        5,
        device.now_unix_millis(),
    )
    .expect("session opens");
    let verified = download_and_verify(&device, &mut session, &bundle, &manifest)
        .expect("verification passes");
    let activation = device.activation();
    let ledger = device.open_ledger();
    activation
        .activate(&ledger, &verified.base)
        .expect("first activation recorded");
    // The SAME version again: refused (a duplicate activation is not a
    // state change).
    assert!(matches!(
        activation.activate(&ledger, &verified.base),
        Err(UpdateVerificationError::VersionAlreadyActive { version }) if version == "1.2.0"
    ));
    cleanup(device.scratch());
}

// --- 2. The auto-rollback flow ----------------------------------------------------------

#[test]
fn the_auto_rollback_flow_reactivates_the_previous_version_with_a_recorded_reason() {
    let device = MockUpdateDevice::new("auto-rollback", MockVerifierChoice::TestOnlyFnv1a64);
    let bundle = fixture_bundle();
    let activation = device.activation();

    // Sequence 5 activates 1.2.0 over a previously-activated 1.1.0.
    let ledger = device.open_ledger();
    activation
        .activate(
            &ledger,
            &{
                let v1 = v2_manifest("1.1.0", "1.0.0", 1, VALID_EXPIRY, &bundle);
                device
                    .present_bundle(Some(&v1), &bundle)
                    .expect("1.1.0 verifies")
                    .base
            },
        )
        .expect("1.1.0 activated first");
    let v2 = v2_manifest("1.2.0", "1.0.0", 5, VALID_EXPIRY, &bundle);
    let verified_v2 = device
        .present_bundle(Some(&v2), &bundle)
        .expect("1.2.0 verifies");
    activation
        .activate(&device.open_ledger(), &verified_v2.base)
        .expect("1.2.0 activated");
    assert_eq!(device.open_ledger().active_version().as_deref(), Some("1.2.0"));

    // The health check FAILS: the session walks the auto-rollback edges.
    let mut session = UpdateSessionStateMachine::new(
        "attempt-rollback-1",
        device.device_id(),
        "1.2.0",
        5,
        device.now_unix_millis(),
    )
    .expect("session opens");
    session.begin_download(0, device.now_unix_millis() + 100).expect("ok");
    session.download_complete(device.now_unix_millis() + 200).expect("ok");
    session.verify_passed(device.now_unix_millis() + 300).expect("ok");
    session.begin_install(device.now_unix_millis() + 400).expect("ok");
    session.install_complete(device.now_unix_millis() + 500).expect("ok");
    session
        .health_failed(device.now_unix_millis() + 600)
        .expect("the health evidence failed the new version");
    assert_eq!(session.state(), UpdateProgressState::RollingBack);
    assert_eq!(
        session.failure_state(),
        None,
        "RollingBack is not yet terminal — the failure state rides the terminal outcome"
    );
    // The P9.34 rollback: a RE-ACTIVATION of a previously-activated
    // version, with the recorded reason, appended to the register.
    let record = activation
        .rollback(
            &device.open_ledger(),
            "1.0.0",
            "1.1.0",
            "health-check failed on 1.2.0 (fixture evidence)",
            None,
        )
        .expect("rollback recorded");
    assert_eq!(record.kind, UpdateLedgerEventKind::Rollback);
    assert_eq!(record.version, "1.1.0");
    assert_eq!(record.previous_version, "1.2.0");
    assert_eq!(
        record.approval_or_reason,
        "health-check failed on 1.2.0 (fixture evidence)"
    );
    assert_eq!(record.override_reference, "", "above the floor: no override needed");
    session
        .rollback_complete(device.now_unix_millis() + 700)
        .expect("rollback completes");
    assert_eq!(session.state(), UpdateProgressState::RolledBack);
    assert!(session.state().is_terminal());
    assert_eq!(
        device.open_ledger().active_version().as_deref(),
        Some("1.1.0"),
        "the register is the source of truth: the previous version is re-activated"
    );
    // The rollback is audited.
    let lines = audit_lines(&device);
    assert!(lines.iter().any(|l| l.contains("EDGE_UPDATE_ROLLBACK")));
    cleanup(device.scratch());
}

#[test]
fn the_rollback_rules_refuse_impossible_targets_by_name() {
    let device = MockUpdateDevice::new("rollback-rules", MockVerifierChoice::TestOnlyFnv1a64);
    let bundle = fixture_bundle();
    let activation = device.activation();
    let ledger = device.open_ledger();

    // Never-activated target: refused (a rollback re-activates a
    // PREVIOUSLY-activated version, never an unknown one).
    let v1 = v2_manifest("1.1.0", "1.0.0", 1, VALID_EXPIRY, &bundle);
    let verified = device.present_bundle(Some(&v1), &bundle).expect("verifies");
    activation.activate(&ledger, &verified.base).expect("activated");
    assert!(matches!(
        activation.rollback(&device.open_ledger(), "1.0.0", "9.9.9", "target never seen", None),
        Err(UpdateVerificationError::RollbackTargetNeverActivated { version }) if version == "9.9.9"
    ));
    // Not-behind target: refused (that is an upgrade wearing a rollback's
    // clothes). Activate 2.0.0 and try to "roll back" to 1.1.0 — wait,
    // 1.1.0 IS behind; instead try rolling back to the SAME version
    // (not strictly behind) and to a never-behind HIGHER one.
    let v2 = v2_manifest("2.0.0", "1.0.0", 7, VALID_EXPIRY, &bundle);
    let verified_v2 = device.present_bundle(Some(&v2), &bundle).expect("verifies");
    activation
        .activate(&device.open_ledger(), &verified_v2.base)
        .expect("2.0.0 activated");
    // 1.1.0 is genuinely behind: legal (also proves the not-behind check
    // does not over-refuse).
    activation
        .rollback(&device.open_ledger(), "1.0.0", "1.1.0", "planned re-check", None)
        .expect("legal rollback");
    // Re-activate 2.0.0 (a legal upgrade via a new manifest, sequence 9)
    // and attempt a rollback to it (equal = not behind).
    let v3 = v2_manifest("2.0.0", "1.0.0", 9, VALID_EXPIRY, &bundle);
    let verified_v3 = device.present_bundle(Some(&v3), &bundle).expect("verifies");
    activation
        .activate(&device.open_ledger(), &verified_v3.base)
        .expect("2.0.0 re-activated after the rollback");
    assert!(matches!(
        activation.rollback(&device.open_ledger(), "1.0.0", "2.0.0", "not a rollback", None),
        Err(UpdateVerificationError::RollbackTargetNotBehindActive { .. })
    ));
    cleanup(device.scratch());
}

#[test]
fn a_below_floor_rollback_is_refused_without_an_override_and_rides_the_record_with_one() {
    let device = MockUpdateDevice::new("below-floor", MockVerifierChoice::TestOnlyFnv1a64);
    let bundle = fixture_bundle();
    let activation = device.activation();
    let ledger = device.open_ledger();

    // 0.9.0 was activated historically (below today's floor of 1.0.0).
    let old = v2_manifest("0.9.0", "0.0.0", 1, VALID_EXPIRY, &bundle);
    let verified_old = device.present_bundle(Some(&old), &bundle).expect("verifies");
    activation.activate(&ledger, &verified_old.base).expect("0.9.0 activated");
    let newer = v2_manifest("1.5.0", "1.0.0", 4, VALID_EXPIRY, &bundle);
    let verified_newer = device
        .present_bundle(Some(&newer), &bundle)
        .expect("verifies");
    activation
        .activate(&device.open_ledger(), &verified_newer.base)
        .expect("1.5.0 activated");

    // WITHOUT an override: refused (never a silent downgrade).
    assert!(matches!(
        activation.rollback(
            &device.open_ledger(),
            "1.0.0",
            "0.9.0",
            "below the floor, no override",
            None,
        ),
        Err(UpdateVerificationError::RollbackBelowFloorRefused { target, floor })
            if target == "0.9.0" && floor == "1.0.0"
    ));
    // An INVALID override (no actor): refused.
    assert!(matches!(
        activation.rollback(
            &device.open_ledger(),
            "1.0.0",
            "0.9.0",
            "invalid override attempt",
            Some(&RollbackOverride {
                actor: String::new(),
                authority_reference: "ticket-1".to_string(),
            }),
        ),
        Err(UpdateVerificationError::RollbackOverrideInvalid { .. })
    ));
    // WITH a valid override: the rollback is recorded AND the override
    // rides the register record AND the audit line (never silent).
    let record = activation
        .rollback(
            &device.open_ledger(),
            "1.0.0",
            "0.9.0",
            "operator-directed recovery",
            Some(&RollbackOverride {
                actor: "operator-on-call".to_string(),
                authority_reference: "authority/ticket-42".to_string(),
            }),
        )
        .expect("the override makes the downgrade recorded, not silent");
    assert_eq!(record.kind, UpdateLedgerEventKind::Rollback);
    assert_eq!(
        record.override_reference,
        "operator-on-call under authority/ticket-42"
    );
    let lines = audit_lines(&device);
    assert!(
        lines
            .iter()
            .any(|l| l.contains("EDGE_UPDATE_ROLLBACK") && l.contains("RECORDED")),
        "the override rides the audit line: {lines:?}"
    );
    cleanup(device.scratch());
}

// --- 3. Interrupted-update (POWER-LOSS) simulations --------------------------------------

#[test]
fn power_loss_between_the_counter_advance_and_the_install_makes_the_manifest_a_replay() {
    // THE RECORDED FAIL-SAFE DIRECTION (ANTI_ROLLBACK_SEMANTICS_STATEMENT):
    // the counter advances durably BEFORE the verdict is returned, so a
    // power loss after verification but before the install leaves the
    // manifest CONSUMED — a reboot cannot re-present it, and a retry
    // requires re-issued metadata with a higher sequence. This is the
    // honest cost of refusing rollback-by-representation.
    let mut device = MockUpdateDevice::new("power-loss-1", MockVerifierChoice::TestOnlyFnv1a64);
    let bundle = fixture_bundle();
    let manifest = v2_manifest("1.2.0", "1.0.0", 5, VALID_EXPIRY, &bundle);

    let mut session = UpdateSessionStateMachine::new(
        "attempt-pl1",
        device.device_id(),
        "1.2.0",
        5,
        device.now_unix_millis(),
    )
    .expect("session opens");
    let verified = download_and_verify(&device, &mut session, &bundle, &manifest)
        .expect("verification passes");
    assert_eq!(device.last_seen_sequence(), 5, "durable BEFORE the verdict");

    // POWER LOSS: no activation, no install — the device reboots.
    device.power_cycle();
    let mut rebooted = UpdateSessionStateMachine::new(
        "attempt-pl1-retry",
        device.device_id(),
        "1.2.0",
        5,
        device.now_unix_millis(),
    )
    .expect("session opens");
    rebooted.begin_download(0, device.now_unix_millis() + 100).expect("ok");
    rebooted.download_complete(device.now_unix_millis() + 200).expect("ok");
    // The SAME manifest is now a replay: refused (nothing was forgotten
    // — the durable counter is the source of truth).
    let error = device
        .present_bundle(Some(&manifest), &bundle)
        .expect_err("the consumed manifest cannot be re-presented");
    assert!(matches!(
        error,
        UpdateBundleError::SequenceNotAhead { declared: 5, last_seen: 5 }
    ));
    // The session records the explicit verification-refused failure.
    rebooted
        .verify_refused(device.now_unix_millis() + 300)
        .expect("the refusal is a named failure state");
    assert_eq!(rebooted.failure_state(), Some(UPDATE_FAILURE_VERIFICATION_REFUSED));
    // The recovery: re-issued metadata with a HIGHER sequence passes.
    let retry = v2_manifest("1.2.0", "1.0.0", 6, VALID_EXPIRY, &bundle);
    device
        .present_bundle(Some(&retry), &bundle)
        .expect("the higher sequence is a fresh verification");
    // And no activation ever happened for the lost attempt.
    assert!(device.open_ledger().active_version().is_none());
    cleanup(device.scratch());
}

#[test]
fn power_loss_mid_append_on_the_register_truncates_the_torn_tail_and_keeps_the_good_record() {
    use std::io::Write;
    let mut device = MockUpdateDevice::new("power-loss-2", MockVerifierChoice::TestOnlyFnv1a64);
    let bundle = fixture_bundle();
    let activation = device.activation();

    // A verified 1.1.0 activates cleanly.
    let v1 = v2_manifest("1.1.0", "1.0.0", 1, VALID_EXPIRY, &bundle);
    let verified = device.present_bundle(Some(&v1), &bundle).expect("verifies");
    let ledger = device.open_ledger();
    activation.activate(&ledger, &verified.base).expect("activated");

    // POWER LOSS MID-APPEND: the next activation's record frame is torn.
    device.power_cycle();
    {
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(device.ledger_path())
            .expect("register file opens for the torn-write simulation");
        // A partial frame: a payload length claiming 40 bytes, followed
        // by 5 — the CRC check catches it on replay.
        let mut torn = Vec::new();
        torn.extend_from_slice(&40u32.to_le_bytes());
        torn.extend_from_slice(&[0x01, 0x02, 0x03, 0x04, 0x05]);
        file.write_all(&torn).expect("torn frame written");
        file.sync_all().expect("torn frame durable");
    }
    // Reboot: the torn tail is truncated and COUNTED; the good
    // activation survives the replay.
    device.power_cycle();
    let reopened = device.open_ledger();
    assert_eq!(reopened.len(), 1, "only the good record survives");
    assert_eq!(reopened.active_version().as_deref(), Some("1.1.0"));
    // The count lives on the instance that truncated the tail (this open);
    // a LATER direct open sees a clean file and counts 0.
    assert_eq!(reopened.corrupt_tail_truncated(), 1, "counted, never silent");
    let direct = FileUpdateLedger::open(device.ledger_path()).expect("reopens");
    assert_eq!(direct.len(), 1, "the good record survives the second open too");
    cleanup(device.scratch());
}

#[test]
fn power_loss_after_activation_leaves_the_rollback_targets_intact_by_replay() {
    let mut device = MockUpdateDevice::new("power-loss-3", MockVerifierChoice::TestOnlyFnv1a64);
    let bundle = fixture_bundle();
    let activation = device.activation();

    // 1.1.0 then 1.2.0 activate; the new version is mid-health-check
    // (Installing/HealthCheckPending) when the power is lost.
    let ledger = device.open_ledger();
    let v1 = v2_manifest("1.1.0", "1.0.0", 1, VALID_EXPIRY, &bundle);
    let verified_v1 = device.present_bundle(Some(&v1), &bundle).expect("verifies");
    activation.activate(&ledger, &verified_v1.base).expect("1.1.0 activated");
    let v2 = v2_manifest("1.2.0", "1.0.0", 5, VALID_EXPIRY, &bundle);
    let verified_v2 = device.present_bundle(Some(&v2), &bundle).expect("verifies");
    activation
        .activate(&device.open_ledger(), &verified_v2.base)
        .expect("1.2.0 activated");

    // POWER LOSS. On reboot the register is replayed from disk only.
    device.power_cycle();
    let rebooted = device.open_ledger();
    assert_eq!(rebooted.active_version().as_deref(), Some("1.2.0"));
    let targets = rebooted.previously_activated();
    assert!(targets.contains("1.1.0"), "the rollback target survived the reboot");
    assert!(targets.contains("1.2.0"));
    // The auto-rollback completes after the reboot: 1.1.0 re-activates.
    let record = device
        .activation()
        .rollback(
            &device.open_ledger(),
            "1.0.0",
            "1.1.0",
            "post-reboot health evidence failed for 1.2.0 (fixture)",
            None,
        )
        .expect("the rollback target is intact");
    assert_eq!(record.version, "1.1.0");
    assert_eq!(device.open_ledger().active_version().as_deref(), Some("1.1.0"));
    cleanup(device.scratch());
}

#[test]
fn a_cancelled_session_leaves_zero_register_records() {
    let device = MockUpdateDevice::new("cancel", MockVerifierChoice::TestOnlyFnv1a64);
    let bundle = fixture_bundle();
    let manifest = v2_manifest("1.2.0", "1.0.0", 3, VALID_EXPIRY, &bundle);
    let mut session = UpdateSessionStateMachine::new(
        "attempt-cancel",
        device.device_id(),
        "1.2.0",
        3,
        device.now_unix_millis(),
    )
    .expect("session opens");
    let _verified = download_and_verify(&device, &mut session, &bundle, &manifest)
        .expect("verification passes");
    session.verify_passed(device.now_unix_millis() + 300).expect("ok");
    // Cancel at VerifiedPendingInstall: legal, ends the session with the
    // named failure state.
    session.cancel(device.now_unix_millis() + 400).expect("cancelled");
    assert_eq!(session.state(), UpdateProgressState::UpdateFailed);
    assert_eq!(session.failure_state(), Some(UPDATE_FAILURE_CANCELLED));
    // No activation was ever recorded, and no rollback is possible from
    // a terminal session.
    let ledger = device.open_ledger();
    assert!(ledger.is_empty(), "a cancelled attempt leaves zero register records");
    assert!(matches!(
        session.rollback_complete(device.now_unix_millis() + 500),
        Err(UpdateSessionError::IllegalTransition { .. })
    ));
    // And the counter DID advance (the verification passed before the
    // cancel — the anti-rollback consequence is recorded, not hidden).
    assert_eq!(device.last_seen_sequence(), 3);
    cleanup(device.scratch());
}

#[test]
fn the_register_rejects_foreign_files_and_empty_versions() {
    let dir = p18_14_support::scratch_dir("foreign-ledger");
    let path = dir.join("not-a-ledger.bin");
    std::fs::write(&path, b"random bytes, no magic").expect("writes");
    assert!(matches!(
        FileUpdateLedger::open(&path),
        Err(UpdateVerificationError::NotAnUpdateLedger { .. })
    ));
    // An empty version is refused at append (a register event without a
    // version records nothing).
    let ledger_path = dir.join("register.ledger");
    let ledger = FileUpdateLedger::open(&ledger_path).expect("opens");
    assert!(matches!(
        ledger.append(UpdateLedgerRecord {
            kind: UpdateLedgerEventKind::Activation,
            at_unix_millis: BASE_MILLIS,
            version: String::new(),
            previous_version: String::new(),
            manifest_digest_hex: String::new(),
            approval_or_reason: "x".to_string(),
            override_reference: String::new(),
        }),
        Err(UpdateVerificationError::InvalidUpdateInput { .. })
    ));
    cleanup(&dir);
}
