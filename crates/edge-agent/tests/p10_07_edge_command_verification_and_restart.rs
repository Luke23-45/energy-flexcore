// P10.07 — Edge offline/degraded-mode tests (release-gate slice, edge
//         crate): command verification at the edge (forged / replayed /
//         expired / out-of-sequence rejected BEFORE forwarding), the
//         clock-drift hold, the restart checkpoint survival, and the §14.4
//         per-asset fallback discipline (report §22.3 gate 6; §18.1–§18.2;
//         §17.4; §17.5 rules 5–6; §14.3–§14.4; ledger
//         `ledger/phase-10-v1-release-hardening/ledger_phase_10.md` P10.07;
//         record:
//         `ledger/phase-10-v1-release-hardening/07_edge_offline_tests.md`).
//
// WHAT THIS FILE PROVES — over the REAL code paths, by exercise:
//
// 1. FOUR-CHECK VERIFICATION (`crates/edge-agent/src/command_verify.rs`,
//    P6.15): every gate-admitted command runs signature → expiry (with the
//    fail-closed clock-quality check) → allowlist → scope → sequence LAST
//    (a refused command never advances the window — the P4.28 SequenceGuard
//    rule). Forged (tampered / unsigned / wrong signer / payload-binding
//    mismatch), replayed (`duplicate`), expired, out-of-sequence,
//    out-of-scope, and non-allowlisted commands are each rejected with their
//    DISTINCT named code and audited — never one combined "valid" boolean.
//    This exercises the SAME controls the P9.31 pilot rejection proof runs
//    (`crates/edge-agent/src/pilot_rejection_proof.rs` — cross-referenced,
//    not redone; the prover is re-exercised here so the release-gate slice
//    carries its own record).
// 2. CLOCK-DRIFT HOLD (`crates/edge-agent/src/clock_quality.rs`, P6.31 +
//    the P6.15 verifier seam): expiry checks are trusted ONLY on
//    `Synchronized`. A degraded (drifted) or unsynchronized or unknown edge
//    clock FAILS the expiry check CLOSED — including against an otherwise
//    perfectly valid, in-window, correctly signed command (the §17.4 clock
//    drift / §17.5 rule 5 control; the recorded risk "an expired command
//    accepted because the edge clock drifted" is closed in the safe
//    direction). The hold is AUDITED (`EDGE_CLOCK_HOLD_ON_DRIFT`).
// 3. RESTART WITH CHECKPOINT SURVIVAL: the per-channel last-accepted
//    sequence persists in the `FileSequenceCheckpoint` journal — a restart
//    does NOT reset the replay window, so the replay of an
//    already-accepted command after a restart is still refused
//    (`duplicate`). A corrupt checkpoint tail is truncated and COUNTED
//    (the buffer_file discipline), and the idempotency binding journal
//    (`pilot_rejection_proof.rs::FileCommandIdBinding`, the P9.31 additive
//    control) also survives restart: a conflicting payload under an
//    already-used command id is refused with the named
//    `ConflictingIdempotencyPayload` error.
// 4. §14.4 FALLBACK DISCIPLINE (`crates/edge-agent/src/command_forward.rs`,
//    P6.16): "There is no universal 'set power to zero' fallback." The
//    closed fallback option set is pinned VERBATIM (exactly four variants;
//    unknown variants refuse with the full set named and the no-zero-power
//    clause stated); HOLD requires a positive BOUNDED interval (an
//    unbounded hold is a load error); every policy entry requires its
//    recorded operator review (a policy without a review cannot load — the
//    review is recorded, NOT SIGNED in this loop, a human-gate input).
//    Zero-loss / zero-duplication / guarantee language is never used: the
//    signature underneath is the recorded P6.15 keyed-checksum scheme
//    (tamper-EVIDENCE, not a cryptographic guarantee) and the proof suites
//    are finite test sets.
//
// HAND-COMPUTED BOUNDARIES: command window `valid_from <= now <
// valid_until` — `now == valid_from` is valid, `now == valid_until - 1` is
// valid, `now == valid_until` is EXPIRED (exclusive); the authorization
// reference expiry is `now >= valid_until` → expired; the pilot clock
// tolerance is inclusive at exactly 100 ms and Degraded at 101 ms (the
// recorded P6.31 DEFAULT); the sync-timeout boundary keeps the derived
// state at exactly 30 000 ms and moves to `Unsynchronized` at 30 001 ms.
//
// STATUS: WRITTEN-UNRUN (ENTRY_NOTE boundary 3; no cargo command was
// executed in this loop; zero observed failures is NOT claimed — the human
// build/test gate owns:
// `cargo test -p flexcore-edge-agent --test p10_07_edge_command_verification_and_restart`).
// Expected strings are the modules' own constants.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use flexcore_contracts::time_model::ClockQuality;
use flexcore_ports::fakes::FixedClock;

use flexcore_edge_agent::clock_quality::{
    audited_expiry_check, ClockQualityError, ClockQualityMonitor, ClockQualityState,
    SyncSourceConfig, TimeSamplePair, TimeSyncProfile, DEFAULT_PILOT_SYNC_TIMEOUT_MILLIS,
    DEFAULT_PILOT_TOLERANCE_MILLIS, MAX_ABS_OFFSET_MILLIS,
};
use flexcore_edge_agent::command_forward::{FallbackPolicyConfig, FallbackVariant};
use flexcore_edge_agent::command_path::{
    CommandPathGateInputs, CommandPathRuntime, CommandReceipt, EdgeCommand, EnablementKey,
    GateRefusal,
};
use flexcore_edge_agent::command_verify::{
    canonical_command_binding, sign_command, AssetCommandScope, CommandAuthorization,
    CommandSigningKey, CommandVerificationRejection, CommandVerifier, FileSequenceCheckpoint,
    IncomingCommand,
};
use flexcore_edge_agent::pilot_rejection_proof::{
    idempotency_binding, FileCommandIdBinding, PilotAttackCase, PilotRejectionProver,
    PilotRejectionProofError,
};
use flexcore_edge_agent::ports::{AuditSink, InMemoryAuditSink};

const BASE_MILLIS: i64 = 1_700_000_000_000;

fn scratch_dir(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "flexcore-edge-p10-07-verify-{}-{}-{}",
        std::process::id(),
        label,
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir creates");
    dir
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

fn signing_key() -> CommandSigningKey {
    CommandSigningKey::new(
        "lab-command-signing-key-material".to_string(),
        "deployment:lab-command-key-1".to_string(),
    )
    .expect("valid key")
}

fn authorization() -> CommandAuthorization {
    CommandAuthorization {
        site_id: "pilot-site-A".to_string(),
        valid_until_unix_millis: BASE_MILLIS + 86_400_000,
        adapter_allowlist: vec!["simulated-test".to_string()],
        asset_scopes: vec![AssetCommandScope {
            asset_key: "ess-01".to_string(),
            allowed_quantities: vec!["active_power".to_string()],
            min_value: -5.0,
            max_value: 5.0,
        }],
    }
}

/// An incoming command with the given identity/value/window. When `signed`
/// is true the command is signed under the REAL key (the central platform's
/// signing side); when false it arrives unsigned (the signature refusal
/// case). The command id is parameterized so the idempotency-binding guard
/// binds each attack under its own id.
fn incoming(id: &str, sequence: u64, value: f64, signed: bool) -> IncomingCommand {
    let edge = EdgeCommand::new(
        id.to_string(),
        "ess-01".to_string(),
        "ISSUED".to_string(),
        sequence,
    )
    .expect("valid command");
    let mut command = IncomingCommand::new(
        edge,
        "simulated-test".to_string(),
        String::new(),
        "edge-session-1".to_string(),
        "active_power".to_string(),
        value,
        BASE_MILLIS,
        BASE_MILLIS + 3_600_000,
    )
    .expect("valid incoming");
    if signed {
        command.signature_reference = sign_command(&signing_key(), &command);
    }
    command
}

fn receipt(command_id: &str) -> CommandReceipt {
    CommandReceipt {
        command_id: command_id.to_string(),
        target_adapter: "simulated-test".to_string(),
        admitted_at_unix_millis: BASE_MILLIS,
        approver: "J. Doe (security lead)".to_string(),
        enablement_valid_until_unix_millis: BASE_MILLIS + 3_600_000,
        scope_note: "gate admission only (test)".to_string(),
    }
}

fn verifier(dir: &Path, now: i64) -> CommandVerifier {
    let checkpoint =
        FileSequenceCheckpoint::open(&dir.join("seq.ckpt")).expect("checkpoint opens");
    CommandVerifier::new(
        signing_key(),
        authorization(),
        Box::new(checkpoint),
        Arc::new(InMemoryAuditSink::new()) as Arc<dyn AuditSink>,
        Arc::new(FixedClock::at_unix_millis(now)),
    )
    .expect("verifier composes")
}

// =================================================================================
// 1. Four-check verification: forged / replayed / expired / out-of-sequence
//    / out-of-scope / non-allowlisted — each refused with its DISTINCT code.
// =================================================================================

#[test]
fn the_six_rejection_codes_are_pairwise_distinct() {
    let rejections = [
        CommandVerificationRejection::Signature { detail: "d".to_string() },
        CommandVerificationRejection::Expiry { detail: "d".to_string() },
        CommandVerificationRejection::Sequence { detail: "d".to_string() },
        CommandVerificationRejection::Duplicate { detail: "d".to_string() },
        CommandVerificationRejection::Allowlist { adapter_id: "a".to_string() },
        CommandVerificationRejection::Scope { detail: "d".to_string() },
    ];
    let mut codes: Vec<&str> = rejections.iter().map(|r| r.code()).collect();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), 6, "every rejection reason is DISTINCT: {codes:?}");
}

#[test]
fn a_forged_command_is_rejected_before_any_forwarding() {
    let dir = scratch_dir("forged");
    let mut verifier = verifier(&dir, BASE_MILLIS + 1_000);
    let receipt = receipt("cmd-1");
    // Tampered payload: a validly-signed command whose value was changed
    // after signing — the payload integrity check refuses (signature).
    let signed = incoming("cmd-1", 1, 1.5, true);
    let mut tampered = incoming("cmd-1", 1, 1.5, true);
    tampered.value = 9.9; // the attacker's edit, unsigned by the real key
    assert_ne!(
        canonical_command_binding(&tampered),
        canonical_command_binding(&signed),
        "the tampered payload differs from what was signed"
    );
    let rejection = verifier
        .verify(&receipt, &tampered, ClockQuality::Synchronized)
        .expect_err("a tampered payload is refused");
    assert_eq!(rejection.code(), "signature");
    // Unsigned.
    let unsigned = incoming("cmd-1", 2, 1.5, false);
    let rejection = verifier
        .verify(&receipt, &unsigned, ClockQuality::Synchronized)
        .expect_err("an unsigned command is refused");
    assert_eq!(rejection.code(), "signature");
    assert!(rejection.detail().contains("no signature"));
    // Wrong signer (a foreign key reference).
    let foreign_key =
        CommandSigningKey::new("attacker-key".to_string(), "attacker:key-1".to_string()).expect("valid");
    let mut foreign_signed = incoming("cmd-1", 3, 1.5, false);
    foreign_signed.signature_reference = sign_command(&foreign_key, &foreign_signed);
    let rejection = verifier
        .verify(&receipt, &foreign_signed, ClockQuality::Synchronized)
        .expect_err("a foreign signer is refused");
    assert_eq!(rejection.code(), "signature");
    assert!(rejection.detail().contains("wrong signer"));
    // Payload-binding mismatch: a signature bound to another sequence of
    // the same command id.
    let mut wrong_binding = incoming("cmd-1", 4, 1.5, false);
    let other = incoming("cmd-1", 5, 1.5, false);
    wrong_binding.signature_reference = sign_command(&signing_key(), &other);
    let rejection = verifier
        .verify(&receipt, &wrong_binding, ClockQuality::Synchronized)
        .expect_err("a signature bound to a different payload is refused");
    assert_eq!(rejection.code(), "signature");
    cleanup(&dir);
}

#[test]
fn a_replayed_command_is_refused_as_a_duplicate_and_a_refusal_never_advances_the_window() {
    let dir = scratch_dir("replay");
    let mut verifier = verifier(&dir, BASE_MILLIS + 1_000);
    let receipt = receipt("cmd-1");
    let command = incoming("cmd-1", 1, 1.5, true);
    let verified = verifier
        .verify(&receipt, &command, ClockQuality::Synchronized)
        .expect("first delivery verifies");
    assert_eq!(verified.sequence, 1);
    // The SAME command re-delivered (the replay) is refused: sequence 1
    // equals the last accepted.
    let rejection = verifier
        .verify(&receipt, &command, ClockQuality::Synchronized)
        .expect_err("a replay is refused");
    assert_eq!(rejection.code(), "duplicate");
    // A REFUSED command never advances the window: a forged command at
    // sequence 9 is refused at check 1 (signature), and the replay is STILL
    // a duplicate afterwards — the refused sequence never moved the window
    // (the P4.28 SequenceGuard rule; the sequence check runs LAST in the
    // recorded fixed order).
    let forged_high = incoming("cmd-1", 9, 9.9, false);
    let rejection = verifier
        .verify(&receipt, &forged_high, ClockQuality::Synchronized)
        .expect_err("forged refused");
    assert_eq!(rejection.code(), "signature");
    let rejection = verifier
        .verify(&receipt, &command, ClockQuality::Synchronized)
        .expect_err("still a duplicate");
    assert_eq!(rejection.code(), "duplicate");
    cleanup(&dir);
}

#[test]
fn an_expired_command_is_refused_and_the_window_boundary_is_hand_computed() {
    let receipt = receipt("cmd-1");
    // `valid_from <= now < valid_until` (§14.3): with the command window
    // [BASE, BASE + 3_600_000):
    //   now == valid_until - 1 → valid;
    //   now == valid_until     → EXPIRED (exclusive);
    //   now == valid_from - 1  → not yet valid.
    let dir = scratch_dir("expiry-valid");
    let mut verifier = verifier(&dir, BASE_MILLIS + 3_600_000 - 1);
    let command = incoming("cmd-1", 1, 1.5, true);
    verifier
        .verify(&receipt, &command, ClockQuality::Synchronized)
        .expect("one millis before expiry the command is still valid");
    cleanup(&dir);

    let dir = scratch_dir("expiry-at");
    let mut verifier = verifier(&dir, BASE_MILLIS + 3_600_000);
    let command = incoming("cmd-1", 1, 1.5, true);
    let rejection = verifier
        .verify(&receipt, &command, ClockQuality::Synchronized)
        .expect_err("AT the expiry instant the command is expired");
    assert_eq!(rejection.code(), "expiry");
    cleanup(&dir);

    let dir = scratch_dir("expiry-before");
    let mut verifier = verifier(&dir, BASE_MILLIS - 1);
    let command = incoming("cmd-1", 1, 1.5, true);
    let rejection = verifier
        .verify(&receipt, &command, ClockQuality::Synchronized)
        .expect_err("before valid_from the command is not yet valid");
    assert_eq!(rejection.code(), "expiry");
    cleanup(&dir);

    // An expired AUTHORIZATION reference refuses every command even when
    // the command window itself is fine.
    let dir = scratch_dir("auth-expired");
    let expired_auth = CommandAuthorization {
        valid_until_unix_millis: BASE_MILLIS + 500,
        ..authorization()
    };
    let checkpoint =
        FileSequenceCheckpoint::open(&dir.join("seq.ckpt")).expect("checkpoint opens");
    let mut verifier = CommandVerifier::new(
        signing_key(),
        expired_auth,
        Box::new(checkpoint),
        Arc::new(InMemoryAuditSink::new()) as Arc<dyn AuditSink>,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 500)),
    )
    .expect("composes");
    let command = incoming("cmd-1", 1, 1.5, true);
    let rejection = verifier
        .verify(&receipt, &command, ClockQuality::Synchronized)
        .expect_err("an expired authorization refuses");
    assert_eq!(rejection.code(), "expiry");
    assert!(rejection.detail().contains("authorization"));
    cleanup(&dir);
}

#[test]
fn an_out_of_sequence_or_unsequenced_command_is_refused_and_an_outside_scope_command_too() {
    let dir = scratch_dir("sequence-scope");
    let mut verifier = verifier(&dir, BASE_MILLIS + 1_000);
    let receipt = receipt("cmd-1");
    // Sequence 2 accepted; sequence 1 (behind) refused; sequence 0
    // (unsequenced) refused.
    let command = incoming("cmd-1", 2, 1.5, true);
    verifier
        .verify(&receipt, &command, ClockQuality::Synchronized)
        .expect("seq 2 verifies");
    let behind = incoming("cmd-1", 1, 1.5, true);
    let rejection = verifier
        .verify(&receipt, &behind, ClockQuality::Synchronized)
        .expect_err("out of sequence");
    assert_eq!(rejection.code(), "sequence");
    let unsequenced = incoming("cmd-1", 0, 1.5, true);
    let rejection = verifier
        .verify(&receipt, &unsequenced, ClockQuality::Synchronized)
        .expect_err("sequence 0 is unsequenced");
    assert_eq!(rejection.code(), "sequence");
    // Scope: value outside the approved limits [-5.0, 5.0] (re-signed so
    // the signature check passes and the SCOPE check is what refuses).
    let mut over_limit = incoming("cmd-1", 3, 5.1, false);
    over_limit.signature_reference = sign_command(&signing_key(), &over_limit);
    let rejection = verifier
        .verify(&receipt, &over_limit, ClockQuality::Synchronized)
        .expect_err("outside the approved limits");
    assert_eq!(rejection.code(), "scope");
    // Scope: unknown asset (never defaulted).
    let mut unknown_asset = incoming("cmd-1", 3, 1.5, false);
    unknown_asset.edge_command = EdgeCommand::new(
        "cmd-1".to_string(),
        "ess-99".to_string(),
        "ISSUED".to_string(),
        3,
    )
    .expect("valid");
    unknown_asset.signature_reference = sign_command(&signing_key(), &unknown_asset);
    let rejection = verifier
        .verify(&receipt, &unknown_asset, ClockQuality::Synchronized)
        .expect_err("an asset with no scope is refused");
    assert_eq!(rejection.code(), "scope");
    assert!(rejection.detail().contains("no scope"));
    // Allowlist: a target adapter not on the site allowlist (re-signed for
    // the mutated target).
    let mut foreign_adapter = incoming("cmd-1", 3, 1.5, false);
    foreign_adapter.target_adapter = "openems-rest".to_string();
    foreign_adapter.signature_reference = sign_command(&signing_key(), &foreign_adapter);
    let rejection = verifier
        .verify(&receipt, &foreign_adapter, ClockQuality::Synchronized)
        .expect_err("not on the allowlist");
    assert_eq!(rejection.code(), "allowlist");
    cleanup(&dir);
}

// =================================================================================
// 2. The clock-drift hold (P6.31 + the P6.15 verifier seam): a drifted edge
//    clock fails the expiry check CLOSED for otherwise-valid commands.
// =================================================================================

fn pilot_monitor() -> ClockQualityMonitor {
    let config =
        SyncSourceConfig::for_profile(TimeSyncProfile::Pilot, "site-time-service-1".to_string());
    ClockQualityMonitor::new(
        config,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)),
        Arc::new(InMemoryAuditSink::new()) as Arc<dyn AuditSink>,
    )
    .expect("valid config")
}

fn pair(offset_millis: i64, at: i64) -> TimeSamplePair {
    TimeSamplePair {
        reference_unix_millis: at - offset_millis,
        edge_unix_millis: at,
        observed_at_unix_millis: at,
    }
}

#[test]
fn a_degraded_clock_holds_every_command_fail_closed_even_when_fully_valid() {
    let dir = scratch_dir("clock-hold");
    let monitor = pilot_monitor();
    // Hand-computed tolerance boundary (pilot DEFAULT 100 ms), over the
    // monitor's MEDIAN estimate of its bounded sample window: a single
    // in-tolerance pair (offset 100) estimates 100 → Synchronized
    // (inclusive); a single out-of-tolerance pair (offset 101) estimates
    // 101 → Degraded. (See F-P10.07-5 in the record: the even-count median
    // arithmetic means TWO observations of 100 and 101 still estimate 100 —
    // the median is robust, so single-pair probes are the boundary test.)
    let state = monitor
        .observe_pair(pair(DEFAULT_PILOT_TOLERANCE_MILLIS, BASE_MILLIS))
        .expect("pair");
    assert_eq!(state, ClockQualityState::Synchronized);
    let drifted = pilot_monitor();
    let state = drifted
        .observe_pair(pair(DEFAULT_PILOT_TOLERANCE_MILLIS + 1, BASE_MILLIS + 1_000))
        .expect("pair");
    assert_eq!(state, ClockQualityState::Degraded);
    // The drifted clock maps onto the verifier seam as NOT synchronized.
    assert_eq!(
        drifted.p6_15_clock_quality(BASE_MILLIS + 1_000),
        ClockQuality::Unsynchronized
    );
    // THE HOLD: a perfectly valid, signed, in-window command is refused —
    // the expiry check does not trust a drifted clock (§17.5 rule 5). The
    // recorded risk ("an expired command accepted because the edge clock
    // drifted") closes in the SAFE direction: nothing is trusted, not even
    // the valid commands.
    let mut verifier = verifier(&dir, BASE_MILLIS + 1_000);
    let receipt = receipt("cmd-1");
    let command = incoming("cmd-1", 1, 1.5, true);
    let rejection = verifier
        .verify(&receipt, &command, drifted.p6_15_clock_quality(BASE_MILLIS + 1_000))
        .expect_err("the drifted clock holds the command");
    assert_eq!(rejection.code(), "expiry");
    assert!(rejection.detail().contains("clock quality"));
    // A synchronized clock restores trust for the same command shape (fresh
    // verifier + fresh window).
    let dir2 = scratch_dir("clock-ok");
    let monitor2 = pilot_monitor();
    monitor2.observe_pair(pair(5, BASE_MILLIS)).expect("pair");
    assert_eq!(monitor2.p6_15_clock_quality(BASE_MILLIS), ClockQuality::Synchronized);
    let mut verifier2 = verifier(&dir2, BASE_MILLIS + 1_000);
    verifier2
        .verify(&receipt, &command, monitor2.p6_15_clock_quality(BASE_MILLIS))
        .expect("a synchronized clock trusts the expiry check");
    cleanup(&dir);
    cleanup(&dir2);
}

#[test]
fn unknown_and_unsynchronized_clocks_hold_too_and_the_hold_is_audited() {
    let dir = scratch_dir("clock-audit");
    // UNKNOWN (no sync observation yet — never healthy-by-default): holds.
    let monitor = pilot_monitor();
    assert_eq!(monitor.quality_at(BASE_MILLIS), ClockQualityState::Unknown);
    let sink = Arc::new(InMemoryAuditSink::new());
    let error =
        audited_expiry_check(&monitor, sink.as_ref(), BASE_MILLIS).expect_err("an unknown clock holds");
    assert!(error.contains("UNKNOWN"));
    // The hold is AUDITED with the dedicated event name.
    let events = sink.events();
    assert_eq!(events.len(), 1);
    assert!(events[0].detail.starts_with("EDGE_CLOCK_HOLD_ON_DRIFT"));
    // UNSYNCHRONIZED (the sync source goes quiet past the timeout):
    // hand-computed boundary — at exactly 30 000 ms the derived state
    // stands, at 30 001 ms the source is quiet → Unsynchronized.
    monitor.observe_pair(pair(5, BASE_MILLIS)).expect("pair");
    assert_eq!(
        monitor.quality_at(BASE_MILLIS + DEFAULT_PILOT_SYNC_TIMEOUT_MILLIS),
        ClockQualityState::Synchronized
    );
    assert_eq!(
        monitor.quality_at(BASE_MILLIS + DEFAULT_PILOT_SYNC_TIMEOUT_MILLIS + 1),
        ClockQualityState::Unsynchronized
    );
    let sink2 = Arc::new(InMemoryAuditSink::new());
    let error = audited_expiry_check(
        &monitor,
        sink2.as_ref(),
        BASE_MILLIS + DEFAULT_PILOT_SYNC_TIMEOUT_MILLIS + 1,
    )
    .expect_err("an unsynchronized clock holds");
    assert!(error.contains("UNSYNCHRONIZED"));
    assert!(
        sink2.events().iter().any(|e| e.detail.starts_with("EDGE_CLOCK_HOLD_ON_DRIFT")),
        "the hold is audited"
    );
    // A corrupt pair (offset ≥ one day) is refused, never estimated, and
    // leaves the state untouched.
    let error = monitor
        .observe_pair(pair(MAX_ABS_OFFSET_MILLIS + 1, BASE_MILLIS + 2_000))
        .expect_err("a corrupt pair is refused");
    assert!(matches!(error, ClockQualityError::InvalidSample { .. }));
    cleanup(&dir);
}

#[test]
fn the_verifier_refuses_every_non_synchronized_clock_quality_value() {
    // Direct over the verifier parameter seam: Unknown AND Unsynchronized
    // values both fail the expiry check closed. Each iteration gets its own
    // scratch dir (the checkpoint files must not collide).
    let dir_unknown = scratch_dir("clock-verifier-unknown");
    let mut verifier = verifier(&dir_unknown, BASE_MILLIS + 1_000);
    let receipt = receipt("cmd-1");
    let command = incoming("cmd-1", 1, 1.5, true);
    let rejection = verifier
        .verify(&receipt, &command, ClockQuality::Unknown)
        .expect_err("an unknown clock fails the expiry check closed");
    assert_eq!(rejection.code(), "expiry");
    assert!(rejection.detail().contains("FAIL CLOSED"));
    cleanup(&dir_unknown);

    let dir_unsync = scratch_dir("clock-verifier-unsync");
    let mut verifier = verifier(&dir_unsync, BASE_MILLIS + 1_000);
    let command = incoming("cmd-1", 1, 1.5, true);
    let rejection = verifier
        .verify(&receipt, &command, ClockQuality::Unsynchronized)
        .expect_err("an unsynchronized clock fails the expiry check closed");
    assert_eq!(rejection.code(), "expiry");
    assert!(rejection.detail().contains("FAIL CLOSED"));
    cleanup(&dir_unsync);
}

// =================================================================================
// 3. Restart with checkpoint survival: the replay window and the idempotency
//    bindings both survive an edge restart.
// =================================================================================

#[test]
fn the_sequence_checkpoint_survives_a_restart_and_a_post_restart_replay_is_refused() {
    let dir = scratch_dir("checkpoint-restart");
    let path = dir.join("seq.ckpt");
    // Before the restart: sequence 1 accepted (durable store BEFORE the
    // in-memory state moves).
    {
        let checkpoint = FileSequenceCheckpoint::open(&path).expect("opens");
        checkpoint
            .store_last_accepted("ess-01/command", 1)
            .expect("stores");
    }
    // The restart: the replayed window is intact.
    let checkpoint = FileSequenceCheckpoint::open(&path).expect("reopens");
    let state = checkpoint.load_all().expect("loads");
    assert_eq!(
        state,
        vec![("ess-01/command".to_string(), 1u64)],
        "the last-accepted sequence survives the restart"
    );
    // A verifier composed over the reloaded checkpoint refuses the REPLAY
    // of the already-accepted command (same sequence) after the restart —
    // the ledger's recorded restart-replay risk is closed.
    let mut verifier = CommandVerifier::new(
        signing_key(),
        authorization(),
        Box::new(checkpoint),
        Arc::new(InMemoryAuditSink::new()) as Arc<dyn AuditSink>,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000)),
    )
    .expect("composes");
    let receipt = receipt("cmd-1");
    let command = incoming("cmd-1", 1, 1.5, true);
    let rejection = verifier
        .verify(&receipt, &command, ClockQuality::Synchronized)
        .expect_err("the post-restart replay of an accepted command is refused");
    assert_eq!(rejection.code(), "duplicate");
    // A GENUINE next command (sequence 2) verifies after the restart.
    let command = incoming("cmd-1", 2, 1.5, true);
    verifier
        .verify(&receipt, &command, ClockQuality::Synchronized)
        .expect("the genuine next command verifies after the restart");
    cleanup(&dir);
}

#[test]
fn a_corrupt_checkpoint_tail_is_truncated_counted_and_the_last_good_state_survives() {
    let dir = scratch_dir("checkpoint-corrupt");
    let path = dir.join("seq.ckpt");
    {
        let checkpoint = FileSequenceCheckpoint::open(&path).expect("opens");
        checkpoint
            .store_last_accepted("ess-01/command", 9)
            .expect("stores");
    }
    // A torn/partial write: a record header claiming an absurd length.
    {
        use std::io::Write as _;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("appends");
        let mut garbage = 0xFFFF_FFF0u32.to_le_bytes().to_vec();
        garbage.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        file.write_all(&garbage).expect("writes garbage");
    }
    let reopened = FileSequenceCheckpoint::open(&path).expect("reopens (truncating)");
    assert_eq!(reopened.corrupt_tail_truncated(), 1, "the loss is COUNTED, never silent");
    let state = reopened.load_all().expect("loads");
    assert_eq!(
        state,
        vec![("ess-01/command".to_string(), 9u64)],
        "the last good record survives the truncation"
    );
    cleanup(&dir);
}

#[test]
fn a_wrong_magic_checkpoint_file_is_refused_not_guessed() {
    let dir = scratch_dir("checkpoint-foreign");
    let path = dir.join("not-a-checkpoint");
    std::fs::write(&path, b"something else entirely").expect("writes");
    let error = FileSequenceCheckpoint::open(&path).expect_err("refused");
    assert!(matches!(
        error,
        flexcore_edge_agent::command_verify::CheckpointError::NotACheckpointFile { .. }
    ));
    cleanup(&dir);
}

#[test]
fn the_idempotency_binding_survives_a_restart_and_a_conflicting_payload_is_refused() {
    let dir = scratch_dir("binding-restart");
    let path = dir.join("bindings.journal");
    // Before the restart: command cmd-1 is bound to its canonical payload.
    {
        let bindings = FileCommandIdBinding::open(&path, 100).expect("opens");
        let command = incoming("cmd-1", 1, 1.5, true);
        bindings
            .bind("cmd-1", &idempotency_binding(&command), BASE_MILLIS)
            .expect("binds");
    }
    // After the restart: the binding is replayed from the journal.
    let bindings = FileCommandIdBinding::open(&path, 100).expect("reopens");
    let command = incoming("cmd-1", 1, 1.5, true);
    assert!(
        bindings.binding_of("cmd-1").is_some(),
        "the idempotency binding survives the restart"
    );
    // A conflicting payload under the SAME command id is refused with the
    // named error — not double-executed (§14.3; the P9.31 step-4 control).
    let conflicting = incoming("cmd-1", 2, 2.5, true); // same id, DIFFERENT payload
    let error = bindings
        .bind("cmd-1", &idempotency_binding(&conflicting), BASE_MILLIS + 1_000)
        .expect_err("a conflicting payload is refused");
    assert!(matches!(
        error,
        PilotRejectionProofError::ConflictingIdempotencyPayload { .. }
    ));
    // The SAME payload under the same id passes the guard (the verifier
    // owns the replay verdict).
    bindings
        .bind("cmd-1", &idempotency_binding(&command), BASE_MILLIS + 2_000)
        .expect("the same payload passes the guard");
    cleanup(&dir);
}

#[test]
fn the_pilot_rejection_prover_rejects_every_attack_case_with_its_expected_code() {
    // P9.31 cross-reference, re-exercised for the release-gate slice (P9.31
    // owns the full proof record; this run is the P10.07 slice). Each
    // attack case is injected against the composed verifier through the
    // P6.14 receipt — the gate is not bypassed. Each attack carries its OWN
    // command id (the binding guard binds on every guard-pass, so shared
    // ids would cross-contaminate the cases).
    let dir = scratch_dir("prover");
    let bindings = Arc::new(
        FileCommandIdBinding::open(&dir.join("bindings.journal"), 100).expect("opens"),
    );
    let audit = Arc::new(InMemoryAuditSink::new());
    let mut prover = PilotRejectionProver::new(
        Arc::clone(&bindings),
        Arc::clone(&audit) as Arc<dyn AuditSink>,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000)),
    )
    .expect("composes");
    let mut verifier = verifier(&dir, BASE_MILLIS + 1_000);
    let receipt = receipt("atk-forged");

    // ForgedSignature: an unsigned command, fully valid otherwise.
    let forged = incoming("atk-forged", 1, 1.5, false);
    let record = prover
        .run_attack(
            &mut verifier,
            &receipt,
            &forged,
            PilotAttackCase::ForgedSignature,
            ClockQuality::Synchronized,
        )
        .expect("attack runs");
    assert!(record.rejected_with_expected_code(), "{}", record.audit_text());

    // ExpiredWindow: a correctly signed command whose window elapsed (the
    // clock is at BASE + 1_000; the window ended at BASE + 1).
    let mut expired = incoming("atk-expired", 2, 1.5, false);
    expired.valid_until_unix_millis = BASE_MILLIS + 1;
    expired.signature_reference = sign_command(&signing_key(), &expired);
    let receipt_expired = receipt("atk-expired");
    let record = prover
        .run_attack(
            &mut verifier,
            &receipt_expired,
            &expired,
            PilotAttackCase::ExpiredWindow,
            ClockQuality::Synchronized,
        )
        .expect("attack runs");
    assert!(record.rejected_with_expected_code(), "{}", record.audit_text());

    // A legitimate command: accepted, and bound into the idempotency memory
    // (the real flow binds too).
    let legit = incoming("cmd-legit", 3, 1.5, true);
    let verified = verifier
        .verify(&receipt("cmd-legit"), &legit, ClockQuality::Synchronized)
        .expect("the legitimate command verifies");
    prover.bind_accepted(&verified, &legit).expect("binds");

    // OutOfSequence: behind the last accepted (3).
    let behind = incoming("atk-seq", 1, 1.5, true);
    let record = prover
        .run_attack(
            &mut verifier,
            &receipt("atk-seq"),
            &behind,
            PilotAttackCase::OutOfSequence,
            ClockQuality::Synchronized,
        )
        .expect("attack runs");
    assert!(record.rejected_with_expected_code(), "{}", record.audit_text());

    // ReplayedCaptured: the accepted command again (same id, same payload,
    // same sequence) — a replay of an already-accepted command.
    let record = prover
        .run_attack(
            &mut verifier,
            &receipt("cmd-legit"),
            &legit,
            PilotAttackCase::ReplayedCaptured,
            ClockQuality::Synchronized,
        )
        .expect("attack runs");
    assert!(record.rejected_with_expected_code(), "{}", record.audit_text());

    // ConflictingIdempotencyPayload: the accepted id with a DIFFERENT
    // payload (re-signed by the real key so every verifier check would
    // pass — the binding guard refuses BEFORE the verifier).
    let conflicting = incoming("cmd-legit", 7, 7.7, true);
    let record = prover
        .run_attack(
            &mut verifier,
            &receipt("cmd-legit"),
            &conflicting,
            PilotAttackCase::ConflictingIdempotencyPayload,
            ClockQuality::Synchronized,
        )
        .expect("attack runs");
    assert!(record.rejected_with_expected_code(), "{}", record.audit_text());
    assert_eq!(
        record.outcome.rejection_code(),
        Some("idempotency_conflict"),
        "the additive guard's own code"
    );
    // ZERO successes: no attack was accepted (an acceptance is the
    // fail-loud outcome; a finite test set is still a TEST outcome, not a
    // guarantee — the honesty bound recorded in the P9.31 decision record).
    cleanup(&dir);
}

// =================================================================================
// 4. §14.4 fallback discipline: no universal "set power to zero" fallback —
//    the closed fallback option set, verbatim.
// =================================================================================

#[test]
fn the_fallback_option_set_is_the_closed_verbatim_set_with_no_zero_power_variant() {
    // §14.4: "There is no universal 'set power to zero' fallback." The
    // closed set is exactly these FOUR variants (command_forward.rs, the
    // recorded P6.16 vocabulary); each parses; there is no fifth.
    for (name, interval) in [
        ("HOLD_LAST_ACCEPTED_SETPOINT", 300_000i64),
        ("RETURN_TO_LOCAL_EMS_AUTONOMOUS", 0),
        (
            "PRESERVE_RESERVE_SOC_CONSTRAINTS_AND_STOP_ACCEPTING_NEW_COMMANDS",
            0,
        ),
        ("REQUIRE_OPERATOR_INTERVENTION", 0),
    ] {
        let variant = FallbackVariant::parse(name, interval).expect("each closed name parses");
        assert_eq!(variant.name(), name, "the stable name is the §14.4 option, verbatim");
    }
    // The zero-power fallback is STRUCTURALLY ABSENT: the name does not
    // parse, and the named error states the full closed set AND the
    // no-zero-power rule verbatim.
    let error = FallbackVariant::parse("SET_POWER_TO_ZERO", 0).expect_err("no such fallback");
    let message = error.to_string();
    assert!(message.contains("no universal \"set power to zero\" fallback"));
    for name in [
        "HOLD_LAST_ACCEPTED_SETPOINT",
        "RETURN_TO_LOCAL_EMS_AUTONOMOUS",
        "PRESERVE_RESERVE_SOC_CONSTRAINTS_AND_STOP_ACCEPTING_NEW_COMMANDS",
        "REQUIRE_OPERATOR_INTERVENTION",
    ] {
        assert!(message.contains(name), "the closed set is named in the refusal: {message}");
    }
}

#[test]
fn a_hold_fallback_requires_a_positive_bounded_interval_and_every_entry_requires_review() {
    // §14.4: hold is approved only for a BOUNDED interval — "A last-known
    // setpoint is not automatically safe". An unbounded (non-positive)
    // HOLD interval is a load error.
    let config = FallbackPolicyConfig {
        format_version: flexcore_edge_agent::command_forward::FALLBACK_POLICY_FORMAT_VERSION,
        entries: vec![flexcore_edge_agent::command_forward::FallbackPolicyEntry {
            asset_key: "ess-01".to_string(),
            variant: FallbackVariant::HoldLastAcceptedSetpoint { bounded_interval_millis: 0 },
            triggers: vec![flexcore_edge_agent::command_forward::FallbackTrigger::Nack],
            review: flexcore_edge_agent::command_forward::OperatorReview {
                reviewed_by: "A. Operator (site operator)".to_string(),
                reviewed_on: "2026-11-13".to_string(),
            },
        }],
    };
    let error = config.validate().expect_err("an unbounded hold is forbidden");
    assert!(error.to_string().contains("positive bounded interval"));
    // A policy WITHOUT its operator review cannot load (the review is
    // required to load; recorded, NOT SIGNED in this loop — human gate).
    let unreviewed = FallbackPolicyConfig {
        format_version: flexcore_edge_agent::command_forward::FALLBACK_POLICY_FORMAT_VERSION,
        entries: vec![flexcore_edge_agent::command_forward::FallbackPolicyEntry {
            asset_key: "ess-01".to_string(),
            variant: FallbackVariant::ReturnToLocalEmsAutonomous,
            triggers: vec![flexcore_edge_agent::command_forward::FallbackTrigger::Timeout],
            review: flexcore_edge_agent::command_forward::OperatorReview {
                reviewed_by: String::new(),
                reviewed_on: "2026-11-13".to_string(),
            },
        }],
    };
    let error = unreviewed.validate().expect_err("a policy without review cannot load");
    assert!(error.to_string().contains("reviewed_by"));
    // A policy configured for NO trigger is a mistake, not a no-op.
    let no_trigger = FallbackPolicyConfig {
        format_version: flexcore_edge_agent::command_forward::FALLBACK_POLICY_FORMAT_VERSION,
        entries: vec![flexcore_edge_agent::command_forward::FallbackPolicyEntry {
            asset_key: "ess-01".to_string(),
            variant: FallbackVariant::RequireOperatorIntervention,
            triggers: Vec::new(),
            review: flexcore_edge_agent::command_forward::OperatorReview {
                reviewed_by: "A. Operator".to_string(),
                reviewed_on: "2026-11-13".to_string(),
            },
        }],
    };
    let error = no_trigger.validate().expect_err("an empty trigger list refuses");
    assert!(error.to_string().contains("trigger"));
    // And the versioned document parses the recorded format with the
    // operator-review fields present.
    let parsed = FallbackPolicyConfig::parse(&format!(
        "{}\nasset = ess-01\nvariant = HOLD_LAST_ACCEPTED_SETPOINT\n\
         bounded_interval_millis = 300000\ntriggers = nack,timeout\n\
         reviewed_by = A. Operator (site operator)\nreviewed_on = 2026-11-13\n",
        flexcore_edge_agent::command_forward::FALLBACK_POLICY_FORMAT
    ))
    .expect("parses");
    assert_eq!(parsed.entries.len(), 1);
    assert_eq!(parsed.entries[0].asset_key, "ess-01");
    assert!(matches!(
        parsed.entries[0].variant,
        FallbackVariant::HoldLastAcceptedSetpoint { bounded_interval_millis: 300_000 }
    ));
}

#[test]
fn the_gate_refuses_in_fixed_order_and_never_defaults_to_enabled() {
    // Belt-and-braces for the offline story: no gate input combination that
    // is ABSENT ever enables. The fixed evaluation order reports the FIRST
    // missing condition; nothing defaults to allow.
    let key = enablement_key();
    let record = flexcore_edge_agent::command_path::SignedEnablementRecord {
        site_id: "pilot-site-A".to_string(),
        approver: "J. Doe (security lead)".to_string(),
        adapter_scope: vec!["simulated-test".to_string()],
        enablement_token: "lab-token-1".to_string(),
        issued_at_unix_millis: BASE_MILLIS,
        valid_until_unix_millis: BASE_MILLIS + 3_600_000,
        key_reference: "deployment:lab-enablement-key-1".to_string(),
        digest: None,
    };
    let signed = record.sign(&key).expect("signs");
    // Flag on, allowlist empty → AllowlistEmpty (step 2 of the fixed order).
    let runtime = CommandPathRuntime::evaluate(
        &CommandPathGateInputs {
            site_id: "pilot-site-A".to_string(),
            config_flag_enabled: true,
            command_allowlist: Vec::new(),
            ..Default::default()
        },
        &key,
        BASE_MILLIS,
    );
    assert!(matches!(
        runtime.enablement_at(BASE_MILLIS),
        flexcore_edge_agent::command_path::CommandPathEnablement::Disabled {
            reason: GateRefusal::AllowlistEmpty
        }
    ));
    // Flag on, allowlist set, D6 absent → D6RecordAbsent (step 3).
    let runtime = CommandPathRuntime::evaluate(
        &CommandPathGateInputs {
            site_id: "pilot-site-A".to_string(),
            config_flag_enabled: true,
            command_allowlist: vec!["simulated-test".to_string()],
            ..Default::default()
        },
        &key,
        BASE_MILLIS,
    );
    assert!(matches!(
        runtime.enablement_at(BASE_MILLIS),
        flexcore_edge_agent::command_path::CommandPathEnablement::Disabled {
            reason: GateRefusal::D6RecordAbsent
        }
    ));
    // Everything except the signed record → NoSignedRecord (step 4).
    let runtime = CommandPathRuntime::evaluate(
        &CommandPathGateInputs {
            site_id: "pilot-site-A".to_string(),
            config_flag_enabled: true,
            command_allowlist: vec!["simulated-test".to_string()],
            d6_record_reference: Some("d6".to_string()),
            ..Default::default()
        },
        &key,
        BASE_MILLIS,
    );
    assert!(matches!(
        runtime.enablement_at(BASE_MILLIS),
        flexcore_edge_agent::command_path::CommandPathEnablement::Disabled {
            reason: GateRefusal::NoSignedRecord
        }
    ));
    // A record binding a DIFFERENT site → SiteMismatch (step 6).
    let runtime = CommandPathRuntime::evaluate(
        &CommandPathGateInputs {
            site_id: "other-site".to_string(),
            config_flag_enabled: true,
            command_allowlist: vec!["simulated-test".to_string()],
            d6_record_reference: Some("d6".to_string()),
            signed_record: Some(signed.clone()),
            environment_token: Some("lab-token-1".to_string()),
        },
        &key,
        BASE_MILLIS,
    );
    assert!(matches!(
        runtime.enablement_at(BASE_MILLIS),
        flexcore_edge_agent::command_path::CommandPathEnablement::Disabled {
            reason: GateRefusal::SiteMismatch
        }
    ));
}

fn enablement_key() -> EnablementKey {
    EnablementKey::new(
        "lab-enablement-key-material".to_string(),
        "deployment:lab-enablement-key-1".to_string(),
    )
    .expect("valid key")
}
