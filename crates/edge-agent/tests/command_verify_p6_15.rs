// P6.15 — Integration tests: the four-check edge command verifier.
//
// EVERY TEST BELOW IS UNRUN (the loop bans in
// `ledger/phase-06-edge-protocol/ENTRY_NOTE.md` forbid building and running
// tests; the human build/test pass owns execution). Nothing here claims a
// pass — these are the negative/positive cases the ledger names:
//
// - forged signature, unsigned command, wrong signer → `signature`;
// - replayed valid command (a resend of a captured one) → `duplicate`;
// - aged/expired (and not-yet-valid) command → `expiry`;
// - duplicate sequence vs out-of-sequence as DISTINCT rejections;
// - non-allowlisted target → `allowlist`;
// - out-of-scope limits / quantity / asset → `scope`;
// - degraded or unknown edge clock → the expiry check FAILS CLOSED;
// - a valid command passes all four checks in the TEST ENV ONLY (the
//   test-lab marker carried through the receipts and audits);
// - sequence state survives an edge restart via the persisted checkpoint
//   (a replay after restart is rejected as a duplicate);
// - a checkpoint store failure refuses the command fail-closed.

use std::sync::Arc;

use flexcore_contracts::time_model::ClockQuality;
use flexcore_ports::clock::Clock;
use flexcore_ports::fakes::FixedClock;

use flexcore_edge_agent::command_forward::SiteCommandAllowlist;
use flexcore_edge_agent::command_path::{
    CommandPathGateInputs, CommandPathRuntime, CommandReceipt, CommandReceiver, EdgeCommand,
    EnablementKey, GateRefusal, SignedEnablementRecord,
};
use flexcore_edge_agent::command_verify::{
    sign_command, AssetCommandScope, CommandAuthorization, CommandSigningKey,
    CommandVerificationRejection, CommandVerifier, FileSequenceCheckpoint, IncomingCommand,
    SequenceCheckpointStore,
};
use flexcore_edge_agent::policy::{AdapterPolicy, OperatingWindow, RateLimit, SitePolicyConfig};
use flexcore_edge_agent::ports::{AuditSink, InMemoryAuditSink};

const BASE_MILLIS: i64 = 1_700_000_000_000;
const ENABLEMENT_VALID_UNTIL: i64 = BASE_MILLIS + 3_600_000;

// --- Shared fixtures -------------------------------------------------------------

/// A std-only unique scratch directory per call (the buffer_p6_09 pattern).
fn scratch_dir(label: &str) -> std::path::PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "flexcore-edge-p615-it-{}-{}-{}",
        std::process::id(),
        label,
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir creates");
    dir
}

/// The P6.14 enablement key (lab fixture; real key material is a
/// deployment requirement).
fn gate_key() -> EnablementKey {
    EnablementKey::new(
        "lab-enablement-key-material".to_string(),
        "deployment:lab-key-1".to_string(),
    )
    .expect("valid gate key")
}

/// The central platform's command-signing key (lab fixture; real key
/// material is a deployment requirement).
fn signing_key() -> CommandSigningKey {
    CommandSigningKey::new(
        "lab-command-signing-key-material".to_string(),
        "deployment:lab-command-key-1".to_string(),
    )
    .expect("valid signing key")
}

fn other_signing_key() -> CommandSigningKey {
    CommandSigningKey::new(
        "some-other-key-material".to_string(),
        "deployment:some-other-key".to_string(),
    )
    .expect("valid other key")
}

/// A REAL gate admission: a receiver whose four P6.14 conditions all hold,
/// admitting the command through `receive` (the verifier is only ever fed
/// gate-admitted commands in composition).
fn gate_receipt(command: &EdgeCommand, target_adapter: &str) -> CommandReceipt {
    let key = gate_key();
    let record = SignedEnablementRecord {
        site_id: "pilot-site-A".to_string(),
        approver: "J. Doe (security lead)".to_string(),
        adapter_scope: vec!["simulated-test".to_string()],
        enablement_token: "lab-token-123".to_string(),
        issued_at_unix_millis: BASE_MILLIS,
        valid_until_unix_millis: ENABLEMENT_VALID_UNTIL,
        key_reference: "deployment:lab-key-1".to_string(),
        digest: None,
    }
    .sign(&key)
    .expect("the record signs");
    let inputs = CommandPathGateInputs {
        site_id: "pilot-site-A".to_string(),
        config_flag_enabled: true,
        signed_record: Some(record),
        environment_token: Some("lab-token-123".to_string()),
        command_allowlist: vec!["simulated-test".to_string()],
        d6_record_reference: Some(
            "ledger/phase-06-edge-protocol/03_d6_boundary_review.md".to_string(),
        ),
    };
    let runtime = CommandPathRuntime::evaluate(&inputs, &key, BASE_MILLIS);
    assert!(runtime.enablement_at(BASE_MILLIS + 1_000).is_enabled_at(BASE_MILLIS + 1_000));
    let mut receiver = CommandReceiver::new(
        runtime,
        Arc::new(permissive_policy()),
        Arc::new(InMemoryAuditSink::new()),
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000)),
    );
    receiver
        .receive(command, target_adapter)
        .unwrap_or_else(|refusal| {
            panic!("the gate must admit the fixture command, refused: {refusal}")
        })
}

/// The permissive site policy for the gate (rate 100/60 s, all-day window).
fn permissive_policy() -> flexcore_edge_agent::policy::SitePolicyEngine {
    flexcore_edge_agent::policy::SitePolicyEngine::new(SitePolicyConfig {
        format_version: 1,
        adapters: vec![AdapterPolicy {
            adapter_id: "simulated-test".to_string(),
            enabled: true,
            rate_limit: RateLimit {
                max_commands: 100,
                window_millis: 60_000,
            },
            operating_window: OperatingWindow {
                start_millis_of_day: 0,
                end_millis_of_day: 86_400_000,
            },
        }],
    })
    .expect("valid policy")
}

/// The site command authorization reference (§17.5 rule 6).
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

/// Builds an incoming command with the given fields and signature.
fn incoming(
    command_id: &str,
    asset_key: &str,
    sequence: u64,
    target_adapter: &str,
    quantity: &str,
    value: f64,
    signature: String,
    valid_from: i64,
    valid_until: i64,
) -> IncomingCommand {
    let edge = EdgeCommand::new(
        command_id.to_string(),
        asset_key.to_string(),
        "ISSUED".to_string(),
        sequence,
    )
    .expect("valid edge command");
    IncomingCommand::new(
        edge,
        target_adapter.to_string(),
        signature,
        "edge-outbound-session-1".to_string(),
        quantity.to_string(),
        value,
        valid_from,
        valid_until,
    )
    .expect("valid incoming command")
}

/// A signed, in-scope, in-window command (the "captured valid command" the
/// replay tests resend).
fn valid_signed(sequence: u64) -> IncomingCommand {
    let unsigned = incoming(
        "cmd-1",
        "ess-01",
        sequence,
        "simulated-test",
        "active_power",
        1.5,
        String::new(),
        BASE_MILLIS,
        BASE_MILLIS + 600_000,
    );
    let signature = sign_command(&signing_key(), &unsigned);
    incoming(
        "cmd-1",
        "ess-01",
        sequence,
        "simulated-test",
        "active_power",
        1.5,
        signature,
        BASE_MILLIS,
        BASE_MILLIS + 600_000,
    )
}

/// A verifier over a fresh file checkpoint in its own scratch dir, sharing
/// the given audit sink.
fn verifier_with(
    audit: Arc<dyn AuditSink>,
    clock: Arc<dyn Clock + Send + Sync>,
    dir: &std::path::Path,
) -> CommandVerifier {
    let checkpoint = FileSequenceCheckpoint::open(&dir.join("seq.ckpt")).expect("checkpoint opens");
    CommandVerifier::new(
        signing_key(),
        authorization(),
        Box::new(checkpoint),
        audit,
        clock,
    )
    .expect("verifier composes")
}

fn verifier(dir: &std::path::Path) -> CommandVerifier {
    verifier_with(
        Arc::new(InMemoryAuditSink::new()),
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000)),
        dir,
    )
}

fn receipt_for(command: &IncomingCommand) -> CommandReceipt {
    gate_receipt(&command.edge_command, &command.target_adapter)
}

/// Asserts the verifier audited a rejection with EXACTLY the expected
/// distinct reason code on the given sink.
fn assert_audited_rejection(audit: &InMemoryAuditSink, code: &str) {
    let details: Vec<String> = audit
        .events()
        .iter()
        .map(|event| event.detail.clone())
        .collect();
    assert!(
        details
            .iter()
            .any(|detail| detail.starts_with("EDGE_COMMAND_VERIFICATION_REJECTED")
                && detail.contains(&format!("code={code}"))),
        "expected an audited verification rejection with code={code}, got: {details:?}"
    );
}

// --- 1. Signature: unsigned, forged, wrong signer ---------------------------------

#[test]
fn unsigned_forged_and_wrong_signer_commands_reject_with_the_signature_reason() {
    let dir = scratch_dir("signature");
    let audit = Arc::new(InMemoryAuditSink::new());
    let mut verifier = verifier_with(
        Arc::clone(&audit) as Arc<dyn AuditSink>,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000)),
        &dir,
    );
    let receipt = receipt_for(&valid_signed(1));

    // UNSIGNED: no signature at all.
    let unsigned = incoming(
        "cmd-1",
        "ess-01",
        1,
        "simulated-test",
        "active_power",
        1.5,
        String::new(),
        BASE_MILLIS,
        BASE_MILLIS + 600_000,
    );
    let error = verifier
        .verify(&receipt, &unsigned, ClockQuality::Synchronized)
        .expect_err("an unsigned command is refused");
    assert!(matches!(
        error,
        CommandVerificationRejection::Signature { .. }
    ));
    assert_eq!(error.code(), "signature");
    assert_audited_rejection(&audit, "signature");

    // FORGED: a valid signature for value 1.5, presented for value 9.9 —
    // the payload does not match what was signed.
    let genuine = valid_signed(2);
    let forged = incoming(
        "cmd-1",
        "ess-01",
        2,
        "simulated-test",
        "active_power",
        9.9,
        genuine.signature_reference.clone(),
        BASE_MILLIS,
        BASE_MILLIS + 600_000,
    );
    let error = verifier
        .verify(&receipt, &forged, ClockQuality::Synchronized)
        .expect_err("a forged payload is refused");
    assert!(matches!(
        error,
        CommandVerificationRejection::Signature { detail } if detail.contains("payload integrity")
    ));
    assert_audited_rejection(&audit, "signature");

    // WRONG SIGNER: a valid signature under a different key reference.
    let foreign = incoming(
        "cmd-1",
        "ess-01",
        3,
        "simulated-test",
        "active_power",
        1.5,
        String::new(),
        BASE_MILLIS,
        BASE_MILLIS + 600_000,
    );
    let foreign = {
        let signature = sign_command(&other_signing_key(), &foreign);
        incoming(
            "cmd-1",
            "ess-01",
            3,
            "simulated-test",
            "active_power",
            1.5,
            signature,
            BASE_MILLIS,
            BASE_MILLIS + 600_000,
        )
    };
    let error = verifier
        .verify(&receipt, &foreign, ClockQuality::Synchronized)
        .expect_err("a wrong-signer command is refused");
    assert!(matches!(
        error,
        CommandVerificationRejection::Signature { detail } if detail.contains("wrong signer")
    ));
    assert_audited_rejection(&audit, "signature");

    // Nothing was accepted: the persisted sequence window never moved.
    let checkpoint = FileSequenceCheckpoint::open(&dir.join("seq.ckpt")).expect("reopens");
    assert!(
        checkpoint.load_all().expect("loads").is_empty(),
        "no refused command advanced the sequence window"
    );
}

// --- 2. Replay: a resend of a captured valid command -------------------------------

#[test]
fn a_replayed_valid_command_is_rejected_as_a_duplicate() {
    let dir = scratch_dir("replay");
    let audit = Arc::new(InMemoryAuditSink::new());
    let mut verifier = verifier_with(
        Arc::clone(&audit) as Arc<dyn AuditSink>,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000)),
        &dir,
    );
    // The captured command arrives once and passes.
    let captured = valid_signed(7);
    let receipt = receipt_for(&captured);
    let verified = verifier
        .verify(&receipt, &captured, ClockQuality::Synchronized)
        .expect("the first capture is admitted");
    assert_eq!(verified.sequence, 7);

    // The SAME captured command is resent (replay attack): every check is
    // byte-identical, and the duplicate-sequence rejection is what fires.
    let error = verifier
        .verify(&receipt, &captured, ClockQuality::Synchronized)
        .expect_err("a replayed valid command is refused");
    assert!(
        matches!(error, CommandVerificationRejection::Duplicate { .. }),
        "a replay is a DUPLICATE, not a generic sequence failure: {error}"
    );
    assert_audited_rejection(&audit, "duplicate");
}

// --- 3. Expiry: aged, not-yet-valid, and the fail-closed clock ----------------------

#[test]
fn an_aged_or_not_yet_valid_command_rejects_with_the_expiry_reason() {
    let dir = scratch_dir("expiry");
    let audit = Arc::new(InMemoryAuditSink::new());
    let mut verifier = verifier_with(
        Arc::clone(&audit) as Arc<dyn AuditSink>,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000)),
        &dir,
    );
    let receipt = receipt_for(&valid_signed(1));

    // AGED: valid_until has passed (the enablement window is still open, so
    // this is the COMMAND window failing, not the gate).
    let aged = incoming(
        "cmd-1",
        "ess-01",
        1,
        "simulated-test",
        "active_power",
        1.5,
        String::new(),
        BASE_MILLIS - 10_000,
        BASE_MILLIS,
    );
    let aged = {
        let signature = sign_command(&signing_key(), &aged);
        incoming(
            "cmd-1",
            "ess-01",
            1,
            "simulated-test",
            "active_power",
            1.5,
            signature,
            BASE_MILLIS - 10_000,
            BASE_MILLIS,
        )
    };
    let error = verifier
        .verify(&receipt, &aged, ClockQuality::Synchronized)
        .expect_err("an aged command is refused");
    assert!(matches!(
        error,
        CommandVerificationRejection::Expiry { detail } if detail.contains("expired")
    ));
    assert_audited_rejection(&audit, "expiry");

    // NOT YET VALID: valid_from is in the future.
    let future = incoming(
        "cmd-2",
        "ess-01",
        2,
        "simulated-test",
        "active_power",
        1.5,
        String::new(),
        BASE_MILLIS + 2_000,
        BASE_MILLIS + 3_000,
    );
    let future = {
        let signature = sign_command(&signing_key(), &future);
        incoming(
            "cmd-2",
            "ess-01",
            2,
            "simulated-test",
            "active_power",
            1.5,
            signature,
            BASE_MILLIS + 2_000,
            BASE_MILLIS + 3_000,
        )
    };
    let error = verifier
        .verify(&receipt, &future, ClockQuality::Synchronized)
        .expect_err("a not-yet-valid command is refused");
    assert!(matches!(
        error,
        CommandVerificationRejection::Expiry { detail } if detail.contains("not yet valid")
    ));
    assert_audited_rejection(&audit, "expiry");
}

#[test]
fn a_degraded_or_unknown_edge_clock_fails_the_expiry_check_closed() {
    let dir = scratch_dir("clock");
    let audit = Arc::new(InMemoryAuditSink::new());
    let mut verifier = verifier_with(
        Arc::clone(&audit) as Arc<dyn AuditSink>,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000)),
        &dir,
    );
    let receipt = receipt_for(&valid_signed(1));

    // The command itself is perfectly valid and in-window — but the edge
    // clock is UNSYNCHRONIZED: the expiry check FAILS CLOSED (§17.5 rule 5;
    // P6.31 owns producing the clock-quality status).
    for quality in [ClockQuality::Unsynchronized, ClockQuality::Unknown] {
        let error = verifier
            .verify(&receipt, &valid_signed(1), quality)
            .expect_err("a degraded/unknown clock fails the expiry check closed");
        assert!(
            matches!(error, CommandVerificationRejection::Expiry { .. }),
            "clock quality {quality:?} must fail the EXPIRY check closed: {error}"
        );
        assert!(
            error
                .detail()
                .contains("FAIL CLOSED"),
            "the detail names the fail-closed rule: {}",
            error.detail()
        );
        assert_audited_rejection(&audit, "expiry");
    }
}

// --- 4. Sequence: duplicate vs out-of-sequence, zero, restart, store failure -------

#[test]
fn duplicate_and_out_of_sequence_are_distinct_rejections() {
    let dir = scratch_dir("sequence");
    let audit = Arc::new(InMemoryAuditSink::new());
    let mut verifier = verifier_with(
        Arc::clone(&audit) as Arc<dyn AuditSink>,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000)),
        &dir,
    );
    let receipt = receipt_for(&valid_signed(5));

    // Accept sequence 5 (any first sequence >= 1 is admitted).
    verifier
        .verify(&receipt, &valid_signed(5), ClockQuality::Synchronized)
        .expect("the first sequence is admitted");

    // The SAME sequence again: DUPLICATE (a replay).
    let error = verifier
        .verify(&receipt, &valid_signed(5), ClockQuality::Synchronized)
        .expect_err("an equal sequence is a duplicate");
    assert!(matches!(error, CommandVerificationRejection::Duplicate { .. }));
    assert_audited_rejection(&audit, "duplicate");

    // A LOWER sequence: OUT OF SEQUENCE (never a "late but fine" delivery).
    let older = valid_signed(4);
    let receipt_older = receipt_for(&older);
    let error = verifier
        .verify(&receipt_older, &older, ClockQuality::Synchronized)
        .expect_err("a lower sequence is out of order");
    assert!(matches!(
        error,
        CommandVerificationRejection::Sequence { detail } if detail.contains("below the last accepted")
    ));
    assert_audited_rejection(&audit, "sequence");

    // SEQUENCE ZERO (unsequenced) on a fresh channel: out of sequence.
    let zero = valid_signed(0);
    let receipt_zero = receipt_for(&zero);
    let error = verifier
        .verify(&receipt_zero, &zero, ClockQuality::Synchronized)
        .expect_err("sequence 0 is unsequenced");
    assert!(matches!(error, CommandVerificationRejection::Sequence { .. }));
    assert_audited_rejection(&audit, "sequence");
}

#[test]
fn sequence_state_survives_an_edge_restart_via_the_checkpoint() {
    let dir = scratch_dir("restart");
    // Edge run 1: admits sequences 5 (the checkpoint file is written).
    {
        let mut verifier = verifier(&dir);
        let receipt = receipt_for(&valid_signed(5));
        verifier
            .verify(&receipt, &valid_signed(5), ClockQuality::Synchronized)
            .expect("run 1 admits sequence 5");
    }
    // The edge RESTARTS: a NEW verifier over the SAME checkpoint file.
    {
        let audit = Arc::new(InMemoryAuditSink::new());
        let mut verifier = verifier_with(
            Arc::clone(&audit) as Arc<dyn AuditSink>,
            Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 2_000)),
            &dir,
        );
        let receipt = receipt_for(&valid_signed(5));
        // The replay attempt after restart is rejected as a DUPLICATE —
        // the restart did not reset the window.
        let error = verifier
            .verify(&receipt, &valid_signed(5), ClockQuality::Synchronized)
            .expect_err("a post-restart replay is refused");
        assert!(matches!(error, CommandVerificationRejection::Duplicate { .. }));
        assert_audited_rejection(&audit, "duplicate");
        // The NEXT sequence is admitted: the restored state is usable.
        let next = valid_signed(6);
        let receipt = receipt_for(&next);
        let verified = verifier
            .verify(&receipt, &next, ClockQuality::Synchronized)
            .expect("run 2 admits the next sequence");
        assert_eq!(verified.sequence, 6);
    }
}

/// A checkpoint store whose writes ALWAYS fail (the fail-closed harness).
struct FailingCheckpointStore;
impl SequenceCheckpointStore for FailingCheckpointStore {
    fn load_all(&self) -> Result<Vec<(String, u64)>, flexcore_edge_agent::command_verify::CheckpointError> {
        Ok(Vec::new())
    }
    fn store_last_accepted(
        &self,
        _channel: &str,
        _sequence: u64,
    ) -> Result<(), flexcore_edge_agent::command_verify::CheckpointError> {
        Err(flexcore_edge_agent::command_verify::CheckpointError::Io {
            reason: "the checkpoint device is unwritable (injected)".to_string(),
        })
    }
}

#[test]
fn a_checkpoint_store_failure_refuses_the_command_fail_closed() {
    let audit = Arc::new(InMemoryAuditSink::new());
    let mut verifier = CommandVerifier::new(
        signing_key(),
        authorization(),
        Box::new(FailingCheckpointStore),
        Arc::clone(&audit) as Arc<dyn AuditSink>,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000)),
    )
    .expect("composes (the empty history loads)");
    let command = valid_signed(1);
    let receipt = receipt_for(&command);
    // Every earlier check passes, but the durable sequence store fails: the
    // command is refused (an unpersisted advance could be replayed after a
    // restart), and the rejection carries the sequence reason with the
    // persistence detail.
    let error = verifier
        .verify(&receipt, &command, ClockQuality::Synchronized)
        .expect_err("a checkpoint failure refuses the command");
    assert!(matches!(
        error,
        CommandVerificationRejection::Sequence { detail } if detail.contains("checkpoint")
    ));
    assert_audited_rejection(&audit, "sequence");
}

// --- 5. Allowlist and scope (§17.5 rule 6) -------------------------------------------

#[test]
fn a_non_allowlisted_target_rejects_with_the_allowlist_reason() {
    let dir = scratch_dir("allowlist");
    let audit = Arc::new(InMemoryAuditSink::new());
    let mut verifier = verifier_with(
        Arc::clone(&audit) as Arc<dyn AuditSink>,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000)),
        &dir,
    );
    // Signed, in-window, correct sequence — but targeting an adapter that
    // is NOT on the site command allowlist in the authorization reference.
    let command = incoming(
        "cmd-1",
        "ess-01",
        1,
        "openems-rest",
        "active_power",
        1.5,
        String::new(),
        BASE_MILLIS,
        BASE_MILLIS + 600_000,
    );
    let command = {
        let signature = sign_command(&signing_key(), &command);
        incoming(
            "cmd-1",
            "ess-01",
            1,
            "openems-rest",
            "active_power",
            1.5,
            signature,
            BASE_MILLIS,
            BASE_MILLIS + 600_000,
        )
    };
    let receipt = receipt_for(&command);
    let error = verifier
        .verify(&receipt, &command, ClockQuality::Synchronized)
        .expect_err("a non-allowlisted target is refused");
    assert!(matches!(
        error,
        CommandVerificationRejection::Allowlist { ref adapter_id } if adapter_id == "openems-rest"
    ));
    assert_audited_rejection(&audit, "allowlist");
}

#[test]
fn out_of_scope_limits_quantities_and_assets_reject_with_the_scope_reason() {
    let dir = scratch_dir("scope");
    let audit = Arc::new(InMemoryAuditSink::new());
    let mut verifier = verifier_with(
        Arc::clone(&audit) as Arc<dyn AuditSink>,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000)),
        &dir,
    );

    // VALUE above the approved limit (max 5.0).
    let over_limit = incoming(
        "cmd-1",
        "ess-01",
        1,
        "simulated-test",
        "active_power",
        9.9,
        String::new(),
        BASE_MILLIS,
        BASE_MILLIS + 600_000,
    );
    let over_limit = {
        let signature = sign_command(&signing_key(), &over_limit);
        incoming(
            "cmd-1",
            "ess-01",
            1,
            "simulated-test",
            "active_power",
            9.9,
            signature,
            BASE_MILLIS,
            BASE_MILLIS + 600_000,
        )
    };
    let receipt = receipt_for(&over_limit);
    let error = verifier
        .verify(&receipt, &over_limit, ClockQuality::Synchronized)
        .expect_err("a value above the approved limit is refused");
    assert!(matches!(
        error,
        CommandVerificationRejection::Scope { detail } if detail.contains("outside the approved limits")
    ));
    assert_audited_rejection(&audit, "scope");

    // QUANTITY not in the asset's approved scope.
    let wrong_quantity = incoming(
        "cmd-2",
        "ess-01",
        2,
        "simulated-test",
        "reactive_power",
        1.0,
        String::new(),
        BASE_MILLIS,
        BASE_MILLIS + 600_000,
    );
    let wrong_quantity = {
        let signature = sign_command(&signing_key(), &wrong_quantity);
        incoming(
            "cmd-2",
            "ess-01",
            2,
            "simulated-test",
            "reactive_power",
            1.0,
            signature,
            BASE_MILLIS,
            BASE_MILLIS + 600_000,
        )
    };
    let receipt = receipt_for(&wrong_quantity);
    let error = verifier
        .verify(&receipt, &wrong_quantity, ClockQuality::Synchronized)
        .expect_err("an out-of-scope quantity is refused");
    assert!(matches!(
        error,
        CommandVerificationRejection::Scope { detail } if detail.contains("not within the approved scope")
    ));
    assert_audited_rejection(&audit, "scope");

    // ASSET with no scope in the authorization reference.
    let unknown_asset = incoming(
        "cmd-3",
        "ess-unknown",
        3,
        "simulated-test",
        "active_power",
        1.0,
        String::new(),
        BASE_MILLIS,
        BASE_MILLIS + 600_000,
    );
    let unknown_asset = {
        let signature = sign_command(&signing_key(), &unknown_asset);
        incoming(
            "cmd-3",
            "ess-unknown",
            3,
            "simulated-test",
            "active_power",
            1.0,
            signature,
            BASE_MILLIS,
            BASE_MILLIS + 600_000,
        )
    };
    let receipt = receipt_for(&unknown_asset);
    let error = verifier
        .verify(&receipt, &unknown_asset, ClockQuality::Synchronized)
        .expect_err("an asset without a scope is refused");
    assert!(matches!(
        error,
        CommandVerificationRejection::Scope { detail } if detail.contains("no scope")
    ));
    assert_audited_rejection(&audit, "scope");
}

// --- 6. The valid path: all four checks pass, TEST ENV ONLY --------------------------

#[test]
fn a_valid_command_passes_all_four_checks_in_the_test_env_only() {
    let dir = scratch_dir("valid");
    let gate_audit = Arc::new(InMemoryAuditSink::new());
    let verify_audit = Arc::new(InMemoryAuditSink::new());

    // The REAL gate admits the command first (the verifier is only fed
    // gate-admitted commands).
    let command = valid_signed(1);
    let key = gate_key();
    let record = SignedEnablementRecord {
        site_id: "pilot-site-A".to_string(),
        approver: "J. Doe (security lead)".to_string(),
        adapter_scope: vec!["simulated-test".to_string()],
        enablement_token: "lab-token-123".to_string(),
        issued_at_unix_millis: BASE_MILLIS,
        valid_until_unix_millis: ENABLEMENT_VALID_UNTIL,
        key_reference: "deployment:lab-key-1".to_string(),
        digest: None,
    }
    .sign(&key)
    .expect("the record signs");
    let inputs = CommandPathGateInputs {
        site_id: "pilot-site-A".to_string(),
        config_flag_enabled: true,
        signed_record: Some(record),
        environment_token: Some("lab-token-123".to_string()),
        command_allowlist: vec!["simulated-test".to_string()],
        d6_record_reference: Some(
            "ledger/phase-06-edge-protocol/03_d6_boundary_review.md".to_string(),
        ),
    };
    let runtime = CommandPathRuntime::evaluate(&inputs, &key, BASE_MILLIS);
    let mut receiver = CommandReceiver::new(
        runtime,
        Arc::new(permissive_policy()),
        Arc::clone(&gate_audit) as Arc<dyn AuditSink>,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000)),
    );
    let receipt = receiver
        .receive(&command.edge_command, &command.target_adapter)
        .expect("the four gate conditions hold; the command is admitted");

    // The four-check verifier passes the command.
    let mut verifier = verifier_with(
        Arc::clone(&verify_audit) as Arc<dyn AuditSink>,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000)),
        &dir,
    );
    let verified = verifier
        .verify(&receipt, &command, ClockQuality::Synchronized)
        .expect("a valid command passes all four checks");

    // Identity and payload carried through intact.
    assert_eq!(verified.command_id, "cmd-1");
    assert_eq!(verified.asset_key, "ess-01");
    assert_eq!(verified.target_adapter, "simulated-test");
    assert_eq!(verified.quantity, "active_power");
    assert_eq!(verified.value, 1.5);
    assert_eq!(verified.sequence, 1);

    // The TEST-ENV markers are carried through: the verified scope note
    // says TEST ENVIRONMENT ONLY and never delivery; the receipt carries
    // the named approver from the signed enablement record.
    assert!(verified.scope_note.contains("TEST ENVIRONMENT ONLY"));
    assert!(verified.scope_note.contains("acknowledgement is not physical delivery"));
    assert_eq!(receipt.approver, "J. Doe (security lead)");

    // The gate admission and the verification are BOTH audited.
    let gate_details: Vec<String> = gate_audit
        .events()
        .iter()
        .map(|event| event.detail.clone())
        .collect();
    assert!(
        gate_details
            .iter()
            .any(|detail| detail.starts_with("EDGE_COMMAND_ACCEPTED")),
        "the gate admission is audited: {gate_details:?}"
    );
    let verify_details: Vec<String> = verify_audit
        .events()
        .iter()
        .map(|event| event.detail.clone())
        .collect();
    assert!(
        verify_details
            .iter()
            .any(|detail| detail.starts_with("EDGE_COMMAND_VERIFIED")
                && detail.contains("cmd-1")
                && detail.contains("all four checks")),
        "the verification is audited: {verify_details:?}"
    );

    // The sequence checkpoint advanced (durable before memory).
    // (Re-open the checkpoint and read its state.)
    let checkpoint = FileSequenceCheckpoint::open(&dir.join("seq.ckpt")).expect("reopens");
    let state = checkpoint.load_all().expect("loads");
    assert!(
        state
            .iter()
            .any(|(channel, sequence)| channel == "ess-01/command" && *sequence == 1),
        "the checkpoint recorded the accepted sequence: {state:?}"
    );

    // The DEFAULT gate is still disabled: the test-env enablement is the
    // ONLY thing that let this command through.
    let default_runtime =
        CommandPathRuntime::evaluate(&CommandPathGateInputs::default(), &key, BASE_MILLIS);
    assert!(matches!(
        default_runtime.enablement_at(BASE_MILLIS),
        flexcore_edge_agent::command_path::CommandPathEnablement::Disabled {
            reason: GateRefusal::ConfigFlagOff
        }
    ));
}

// --- 7. The forward-side allowlist type is a DIFFERENT list (seam check) -------------

#[test]
fn the_forward_side_allowlist_is_a_distinct_type_with_its_own_semantics() {
    // The P6.15 verifier checks the AUTHORIZATION reference's allowlist;
    // the P6.16 engine re-checks the forward-side SiteCommandAllowlist.
    // They are deliberately distinct types/lists — this test pins that the
    // forward-side type exists here and behaves independently.
    let allowlist = SiteCommandAllowlist::new(vec!["simulated-test".to_string()]);
    assert!(allowlist.contains("simulated-test"));
    assert!(!allowlist.contains("openems-rest"));
}
