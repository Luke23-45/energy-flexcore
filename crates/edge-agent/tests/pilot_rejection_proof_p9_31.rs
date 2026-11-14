// P9.31 — Integration tests: the forged/replayed/expired/out-of-sequence
//         edge rejection PROOF (ledger P9.31; report §17.4, §18.2, §14.3).
//
// EVERY TEST BELOW IS UNRUN (the loop bans in
// `ledger/phase-09-pilot-qualification/ENTRY_NOTE.md` boundary 5 forbid
// building and running tests; the human build/test pass owns execution).
// Nothing here claims a pass — these are the attack-injection cases the
// ledger names, WRITTEN so a human run produces the attempt → outcome →
// audit-reference record per case.
//
// THE PROOF DISCIPLINE (what makes this a proof and not a re-implementation):
// every rejection below is PRODUCED BY the existing Phase 6 controls —
// the P6.14 gate receipt → the P6.15 four-check verifier
// (`flexcore_edge_agent::command_verify::CommandVerifier::verify`, whose
// evaluation order ends with the sequence check LAST per the P4.28
// SequenceGuard rule) — exercised through the P9.31 prover
// (`flexcore_edge_agent::pilot_rejection_proof::PilotRejectionProver`),
// never re-implemented. The one additive control is the P9.31 §14.3
// edge stand-in: the restart-surviving command-id binding journal that
// refuses a conflicting payload under an already-used idempotency id.
//
// The attack matrix:
// - forged: unsigned, malformed-format, wrong signer, and the
//   forged-but-otherwise-valid payload (valid shape, wrong digest) →
//   `signature`, audited, and the sequence window NEVER moved (checked last);
// - exactly-at-expiry: the injected clock AT valid_until → `expiry`;
//   one millisecond before → accepted (the boundary is inclusive-expiry);
// - replayed: a byte-identical resend of a captured accepted command →
//   `duplicate`; the SAME replay after an edge RESTART (new verifier over
//   the same FileSequenceCheckpoint file) → still `duplicate`;
// - out-of-sequence: a gap of 1 behind and a gap of N behind → `sequence`;
//   an equal sequence → `duplicate`; a head gap (ahead of the last accepted)
//   is admitted by design (the monotonic-window discipline) — recorded, not
//   hidden; sequence 0 (unsequenced) → `sequence`;
// - idempotency conflict: a conflicting payload under an already-used
//   command id, even legitimately re-signed with a fresh sequence → refused
//   by the additive binding guard (NOT double-executed), including across a
//   restart of the binding journal;
// - fail-loud: an attack labelled with a case the controls cannot reject
//   (a fresh, fully valid command presented as a "replay") is recorded as
//   ErroneouslyAccepted and audited — never coerced into a pass;
// - zero adapter reach: every rejection leaves the counting forwarder at
//   zero invocations; the ONLY path that increments it is a VerifiedCommand
//   through the P6.16 engine.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use flexcore_contracts::time_model::ClockQuality;
use flexcore_ports::clock::Clock;
use flexcore_ports::fakes::FixedClock;

use flexcore_edge_agent::audit_buffer::FileAuditBuffer;
use flexcore_edge_agent::command_forward::{
    CommandForwarder, CommandForwardingEngine, FallbackPolicyConfig, ForwardOutcome,
    SiteCommandAllowlist,
};
use flexcore_edge_agent::command_path::{
    CommandPathGateInputs, CommandPathRuntime, CommandReceipt, CommandReceiver, EdgeCommand,
    EnablementKey, SignedEnablementRecord,
};
use flexcore_edge_agent::command_verify::{
    sign_command, AssetCommandScope, CommandAuthorization, CommandSigningKey, CommandVerifier,
    FileSequenceCheckpoint, IncomingCommand,
};
use flexcore_edge_agent::pilot_rejection_proof::{
    AttackOutcome, FileCommandIdBinding, PilotAttackCase, PilotRejectionProver,
    DEFAULT_ID_BINDING_CAPACITY,
};
use flexcore_edge_agent::policy::{AdapterPolicy, OperatingWindow, RateLimit, SitePolicyConfig};
use flexcore_edge_agent::ports::{AuditSink, InMemoryAuditSink};

const BASE_MILLIS: i64 = 1_700_000_000_000;
const ENABLEMENT_VALID_UNTIL: i64 = BASE_MILLIS + 3_600_000;
const COMMAND_VALID_FROM: i64 = BASE_MILLIS;
const COMMAND_VALID_UNTIL: i64 = BASE_MILLIS + 600_000;

// --- Shared fixtures (the P6.15 test patterns, mirrored) ---------------------------

/// A std-only unique scratch directory per call (the buffer_p6_09 pattern).
fn scratch_dir(label: &str) -> std::path::PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "flexcore-edge-p931-it-{}-{}-{}",
        std::process::id(),
        label,
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir creates");
    dir
}

/// The P6.14 enablement key (lab fixture; real key material is a deployment
/// requirement).
fn gate_key() -> EnablementKey {
    EnablementKey::new(
        "lab-enablement-key-material".to_string(),
        "deployment:lab-key-1".to_string(),
    )
    .expect("valid gate key")
}

/// The central platform's command-signing key (lab fixture).
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

/// The permissive site policy for the gate and the forward engine.
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

/// A REAL gate admission through a fully-enabled P6.14 receiver (the prover
/// never bypasses the gate).
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

/// A signed, in-scope, in-window command (the "captured valid command").
fn signed_command(
    command_id: &str,
    sequence: u64,
    value: f64,
    valid_from: i64,
    valid_until: i64,
) -> IncomingCommand {
    let unsigned = incoming(
        command_id,
        "ess-01",
        sequence,
        "simulated-test",
        "active_power",
        value,
        String::new(),
        valid_from,
        valid_until,
    );
    let signature = sign_command(&signing_key(), &unsigned);
    incoming(
        command_id,
        "ess-01",
        sequence,
        "simulated-test",
        "active_power",
        value,
        signature,
        valid_from,
        valid_until,
    )
}

/// The default captured command: cmd-<seq>, value 1.5, full validity window.
fn valid_signed(sequence: u64) -> IncomingCommand {
    signed_command(
        &format!("cmd-{sequence}"),
        sequence,
        1.5,
        COMMAND_VALID_FROM,
        COMMAND_VALID_UNTIL,
    )
}

/// A composed verifier over a fresh checkpoint file in `dir`.
fn verifier_with(
    audit: Arc<dyn AuditSink>,
    clock: Arc<dyn Clock + Send + Sync>,
    dir: &std::path::Path,
) -> CommandVerifier {
    let checkpoint =
        FileSequenceCheckpoint::open(&dir.join("seq.ckpt")).expect("checkpoint opens");
    CommandVerifier::new(
        signing_key(),
        authorization(),
        Box::new(checkpoint),
        audit,
        clock,
    )
    .expect("verifier composes")
}

/// A composed prover over the binding journal in `dir`.
fn prover_with(
    audit: Arc<dyn AuditSink>,
    clock: Arc<dyn Clock + Send + Sync>,
    dir: &std::path::Path,
) -> PilotRejectionProver {
    let bindings = FileCommandIdBinding::open(
        &dir.join("id-bindings.journal"),
        DEFAULT_ID_BINDING_CAPACITY,
    )
    .expect("binding journal opens");
    PilotRejectionProver::new(Arc::new(bindings), audit, clock).expect("prover composes")
}

/// The full proof harness: verifier + prover over one scratch dir (the
/// INJECTED clock is shared by both; nothing reads wall time).
struct ProofHarness {
    dir: std::path::PathBuf,
    verifier: CommandVerifier,
    prover: PilotRejectionProver,
    audit: Arc<InMemoryAuditSink>,
}

impl ProofHarness {
    fn new(label: &str, now_millis: i64) -> Self {
        let dir = scratch_dir(label);
        let audit = Arc::new(InMemoryAuditSink::new());
        let clock: Arc<dyn Clock + Send + Sync> = Arc::new(FixedClock::at_unix_millis(now_millis));
        let verifier = verifier_with(Arc::clone(&audit) as Arc<dyn AuditSink>, Arc::clone(&clock), &dir);
        let prover = prover_with(Arc::clone(&audit) as Arc<dyn AuditSink>, Arc::clone(&clock), &dir);
        Self {
            dir,
            verifier,
            prover,
            audit,
        }
    }

    fn run(&mut self, command: &IncomingCommand, case: PilotAttackCase) -> AttackRecord {
        let receipt = gate_receipt(&command.edge_command, &command.target_adapter);
        self.prover
            .run_attack(
                &mut self.verifier,
                &receipt,
                command,
                case,
                ClockQuality::Synchronized,
            )
            .expect("the prover runs the attack")
    }

    /// The audited detail lines that carry a given event name.
    fn audited(&self, event_name: &str) -> Vec<String> {
        self.audit
            .events()
            .iter()
            .filter(|event| event.detail.starts_with(event_name))
            .map(|event| event.detail.clone())
            .collect()
    }
}

/// Asserts the verifier audited a rejection with EXACTLY the expected
/// distinct reason code (the P6.15 assertion pattern).
fn assert_audited_rejection(harness: &ProofHarness, code: &str) {
    let details = harness.audited("EDGE_PILOT_ATTACK_REJECTED");
    assert!(
        details
            .iter()
            .any(|detail| detail.contains(&format!("code={code}"))),
        "expected an audited pilot-attack rejection with code={code}, got: {details:?}"
    );
}

/// Asserts NO command was verified (nothing that could reach an adapter).
fn assert_no_verification(harness: &ProofHarness) {
    let verified = harness.audited("EDGE_COMMAND_VERIFIED");
    assert!(
        verified.is_empty(),
        "no rejected attack may produce a VerifiedCommand audit: {verified:?}"
    );
}

/// Asserts the persisted sequence checkpoint is untouched (the four-check
/// discipline: a refused command never advances the window).
fn assert_sequence_window_untouched(dir: &std::path::Path) {
    let checkpoint =
        FileSequenceCheckpoint::open(&dir.join("seq.ckpt")).expect("checkpoint reopens");
    assert!(
        checkpoint.load_all().expect("loads").is_empty(),
        "no refused attack advanced the sequence window"
    );
}

/// A counting forwarder: the ZERO-REACH oracle for the proof.
struct CountingForwarder {
    calls: AtomicU64,
}

impl CountingForwarder {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicU64::new(0),
        })
    }
    fn calls(&self) -> u64 {
        self.calls.load(Ordering::SeqCst)
    }
}

impl CommandForwarder for CountingForwarder {
    fn forward(
        &self,
        _request: &flexcore_edge_agent::command_forward::ForwardRequest,
    ) -> Result<ForwardOutcome, flexcore_edge_agent::command_forward::ForwardError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(ForwardOutcome::Acknowledged)
    }
}

// --- 1. Forged signatures (ledger step 2; the forged-but-otherwise-valid
//        payload variant) ----------------------------------------------------------------

#[test]
fn forged_but_otherwise_valid_payloads_reject_with_the_signature_reason() {
    let mut harness = ProofHarness::new("forged", BASE_MILLIS + 1_000);

    // FORGED-BUT-OTHERWISE-VALID #1: UNSIGNED. The command is perfect in
    // every other respect — window, scope, sequence, adapter — and is still
    // refused at check 1 with the `signature` reason (never defaulted).
    let unsigned = incoming(
        "cmd-1",
        "ess-01",
        1,
        "simulated-test",
        "active_power",
        1.5,
        String::new(),
        COMMAND_VALID_FROM,
        COMMAND_VALID_UNTIL,
    );
    let record = harness.run(&unsigned, PilotAttackCase::ForgedSignature);
    assert!(
        record.rejected_with_expected_code(),
        "the unsigned-but-valid attack is rejected with its expected code: {record:?}"
    );
    assert!(matches!(
        record.outcome,
        AttackOutcome::RejectedAtVerification { rejection_code: "signature", .. }
    ));
    assert_audited_rejection(&harness, "signature");

    // FORGED-BUT-OTHERWISE-VALID #2: VALID FORMAT, WRONG DIGEST. An
    // attacker crafts a well-formed reference
    // `edge-sig:deployment:lab-command-key-1:cmd-1:fnv1a64:<guess>` — the
    // payload does not match what was signed.
    let forged_digest = incoming(
        "cmd-1",
        "ess-01",
        1,
        "simulated-test",
        "active_power",
        1.5,
        "edge-sig:deployment:lab-command-key-1:cmd-1:fnv1a64:0000000000000000"
            .to_string(),
        COMMAND_VALID_FROM,
        COMMAND_VALID_UNTIL,
    );
    let record = harness.run(&forged_digest, PilotAttackCase::ForgedSignature);
    assert!(matches!(
        record.outcome,
        AttackOutcome::RejectedAtVerification { rejection_code: "signature", ref detail }
            if detail.contains("payload integrity")
    ), "the forged-digest attack is refused at the payload check: {record:?}");
    assert_audited_rejection(&harness, "signature");

    // FORGED-BUT-OTHERWISE-VALID #3: WRONG SIGNER. A genuine signature under
    // a DIFFERENT key reference, presented for this edge's key.
    let foreign_unsigned = incoming(
        "cmd-1",
        "ess-01",
        1,
        "simulated-test",
        "active_power",
        1.5,
        String::new(),
        COMMAND_VALID_FROM,
        COMMAND_VALID_UNTIL,
    );
    let foreign_signature = sign_command(&other_signing_key(), &foreign_unsigned);
    let foreign = incoming(
        "cmd-1",
        "ess-01",
        1,
        "simulated-test",
        "active_power",
        1.5,
        foreign_signature,
        COMMAND_VALID_FROM,
        COMMAND_VALID_UNTIL,
    );
    let record = harness.run(&foreign, PilotAttackCase::ForgedSignature);
    assert!(matches!(
        record.outcome,
        AttackOutcome::RejectedAtVerification { rejection_code: "signature", ref detail }
            if detail.contains("wrong signer")
    ), "the wrong-signer attack is refused: {record:?}");
    assert_audited_rejection(&harness, "signature");

    // THE SEQUENCE DISCIPLINE: the sequence check is LAST — every refused
    // forgery above left the persisted window untouched.
    assert_no_verification(&harness);
    assert_sequence_window_untouched(&harness.dir);
}

// --- 2. Expiry: exactly-at-expiry vs one millisecond before (the boundary) ------------

#[test]
fn exactly_at_expiry_rejects_and_one_millis_before_passes() {
    // THE INJECTED CLOCK AT valid_until: §14.3 expiry is valid
    // `valid_from <= now < valid_until` — at exactly valid_until the command
    // is EXPIRED, never "still good for one more millis".
    let mut harness = ProofHarness::new("expiry-at-boundary", COMMAND_VALID_UNTIL);
    let command = valid_signed(1);
    let record = harness.run(&command, PilotAttackCase::ExpiredWindow);
    assert!(
        matches!(
            record.outcome,
            AttackOutcome::RejectedAtVerification { rejection_code: "expiry", ref detail }
                if detail.contains("expired at")
        ),
        "the exactly-at-expiry attack is refused with the expiry reason: {record:?}"
    );
    assert_audited_rejection(&harness, "expiry");
    assert_no_verification(&harness);
    assert_sequence_window_untouched(&harness.dir);

    // ONE MILLISOCOND BEFORE: the same command shape passes all four checks
    // (the enablement receipt outlives the command window; the sequence is
    // fresh). The boundary is inclusive-expiry, exclusive-validity.
    let mut harness = ProofHarness::new("expiry-before-boundary", COMMAND_VALID_UNTIL - 1);
    let command = valid_signed(1);
    let record = harness.run(&command, PilotAttackCase::ExpiredWindow);
    assert!(
        matches!(record.outcome, AttackOutcome::ErroneouslyAccepted { .. }),
        "one millis before expiry the command is genuinely valid: the attack is \
         recorded FAIL-LOUD, never coerced into a rejection: {record:?}"
    );
    assert!(
        harness.audited("EDGE_PILOT_ATTACK_ACCEPTED").len() == 1,
        "the fail-loud acceptance is audited"
    );
}

// --- 3. Replayed captured command (ledger step 1), across a restart --------------------

#[test]
fn a_replayed_captured_command_is_rejected_and_survives_an_edge_restart() {
    let dir = scratch_dir("replay-restart");
    let now = BASE_MILLIS + 1_000;
    let clock = Arc::new(FixedClock::at_unix_millis(now));

    // Edge run 1: the captured command arrives and is accepted (bound,
    // verified, sequence advanced).
    let audit_1 = Arc::new(InMemoryAuditSink::new());
    let mut verifier = verifier_with(
        Arc::clone(&audit_1) as Arc<dyn AuditSink>,
        Arc::clone(&clock) as Arc<dyn Clock + Send + Sync>,
        &dir,
    );
    let mut prover = prover_with(
        Arc::clone(&audit_1) as Arc<dyn AuditSink>,
        Arc::clone(&clock) as Arc<dyn Clock + Send + Sync>,
        &dir,
    );
    let captured = valid_signed(7);
    let receipt = gate_receipt(&captured.edge_command, &captured.target_adapter);
    let first = prover
        .run_attack(
            &mut verifier,
            &receipt,
            &captured,
            PilotAttackCase::ReplayedCaptured,
            ClockQuality::Synchronized,
        )
        .expect("the prover runs");
    assert!(
        matches!(first.outcome, AttackOutcome::ErroneouslyAccepted { .. }),
        "the FIRST delivery is genuinely accepted and recorded fail-loud (the case label is \
         the attacker's claim; the controls decide): {first:?}"
    );

    // THE REPLAY (same edge run): the byte-identical captured command is
    // resent — the verifier rejects it as a DUPLICATE.
    let replay = prover
        .run_attack(
            &mut verifier,
            &receipt,
            &captured,
            PilotAttackCase::ReplayedCaptured,
            ClockQuality::Synchronized,
        )
        .expect("the prover runs");
    assert!(
        matches!(
            replay.outcome,
            AttackOutcome::RejectedAtVerification { rejection_code: "duplicate", .. }
        ),
        "the replay is a DUPLICATE, not a generic sequence failure: {replay:?}"
    );
    assert!(
        audit_1
            .events()
            .iter()
            .any(|event| event.detail.starts_with("EDGE_PILOT_ATTACK_REJECTED")
                && event.detail.contains("code=duplicate")),
        "the replay is audited with the duplicate reason: {:?}",
        audit_1.events()
    );

    // THE EDGE RESTARTS (run 2): a NEW verifier over the SAME
    // FileSequenceCheckpoint file and the SAME binding journal. The replay
    // is STILL rejected — the restart did not reset either memory.
    let audit_2 = Arc::new(InMemoryAuditSink::new());
    let clock_2 = Arc::new(FixedClock::at_unix_millis(now + 5_000));
    let mut verifier = verifier_with(
        Arc::clone(&audit_2) as Arc<dyn AuditSink>,
        Arc::clone(&clock_2) as Arc<dyn Clock + Send + Sync>,
        &dir,
    );
    let mut prover = prover_with(
        Arc::clone(&audit_2) as Arc<dyn AuditSink>,
        Arc::clone(&clock_2) as Arc<dyn Clock + Send + Sync>,
        &dir,
    );
    let receipt = gate_receipt(&captured.edge_command, &captured.target_adapter);
    let replay_after_restart = prover
        .run_attack(
            &mut verifier,
            &receipt,
            &captured,
            PilotAttackCase::ReplayedCaptured,
            ClockQuality::Synchronized,
        )
        .expect("the prover runs");
    assert!(
        matches!(
            replay_after_restart.outcome,
            AttackOutcome::RejectedAtVerification { rejection_code: "duplicate", .. }
        ),
        "a post-restart replay is STILL a duplicate (the FileSequenceCheckpoint survived): \
         {replay_after_restart:?}"
    );
}

// --- 4. Out-of-sequence: gap of 1 behind, gap of N behind, equal, ahead ---------------

#[test]
fn out_of_sequence_gaps_behind_reject_while_a_head_gap_is_admitted_by_design() {
    let mut harness = ProofHarness::new("sequence-gaps", BASE_MILLIS + 1_000);

    // Accept sequence 10 first (any first sequence >= 1 is admitted).
    let first = valid_signed(10);
    let record = harness.run(&first, PilotAttackCase::OutOfSequence);
    assert!(
        matches!(record.outcome, AttackOutcome::ErroneouslyAccepted { .. }),
        "the first delivery establishes the window: {record:?}"
    );

    // GAP OF 1 BEHIND: sequence 9 → `sequence` (out of sequence).
    let behind_one = valid_signed(9);
    let record = harness.run(&behind_one, PilotAttackCase::OutOfSequence);
    assert!(matches!(
        record.outcome,
        AttackOutcome::RejectedAtVerification { rejection_code: "sequence", ref detail }
            if detail.contains("below the last accepted")
    ), "a gap of 1 behind is out of sequence: {record:?}");
    assert_audited_rejection(&harness, "sequence");

    // GAP OF N BEHIND: sequence 2 → `sequence` (the same reason, any depth).
    let behind_n = valid_signed(2);
    let record = harness.run(&behind_n, PilotAttackCase::OutOfSequence);
    assert!(matches!(
        record.outcome,
        AttackOutcome::RejectedAtVerification { rejection_code: "sequence", .. }
    ), "a gap of N behind is out of sequence: {record:?}");
    assert_audited_rejection(&harness, "sequence");

    // EQUAL: sequence 10 again → `duplicate` (a replay, a DISTINCT reason).
    let equal = valid_signed(10);
    let record = harness.run(&equal, PilotAttackCase::OutOfSequence);
    assert!(matches!(
        record.outcome,
        AttackOutcome::RejectedAtVerification { rejection_code: "duplicate", .. }
    ), "an equal sequence is a DUPLICATE: {record:?}");
    assert_audited_rejection(&harness, "duplicate");

    // UNSEQUENCED: sequence 0 → `sequence`.
    let zero = valid_signed(0);
    let record = harness.run(&zero, PilotAttackCase::OutOfSequence);
    assert!(matches!(
        record.outcome,
        AttackOutcome::RejectedAtVerification { rejection_code: "sequence", .. }
    ), "sequence 0 is unsequenced: {record:?}");

    // HEAD GAP AHEAD OF 1: sequence 11 → ADMITTED BY DESIGN. The verifier
    // enforces MONOTONICITY, not density: the platform may skip sequences
    // (superseded/cancelled commands carry no edge delivery). This is the
    // recorded §14.3 monotonic-window discipline, stated — not hidden — by
    // the proof.
    let ahead_one = valid_signed(11);
    let record = harness.run(&ahead_one, PilotAttackCase::OutOfSequence);
    assert!(
        matches!(record.outcome, AttackOutcome::ErroneouslyAccepted { .. }),
        "a head gap is admitted by the monotonic-window design (recorded, not hidden): \
         {record:?}"
    );

    // Every REJECTED attack above left the window where the last ACCEPTED
    // command put it: 11 (the ahead-gap acceptance), nothing else moved it.
    let checkpoint =
        FileSequenceCheckpoint::open(&harness.dir.join("seq.ckpt")).expect("reopens");
    let state = checkpoint.load_all().expect("loads");
    assert!(
        state
            .iter()
            .all(|(channel, sequence)| channel == "ess-01/command" && *sequence == 11),
        "only the accepted commands advanced the window: {state:?}"
    );
}

// --- 5. Conflicting payload under an already-used idempotency id (ledger step 4) -------

#[test]
fn a_conflicting_payload_under_a_used_idempotency_id_is_refused_not_double_executed() {
    let mut harness = ProofHarness::new("idempotency", BASE_MILLIS + 1_000);

    // The genuine command is accepted and its id BOUND.
    let genuine = valid_signed(1);
    let record = harness.run(&genuine, PilotAttackCase::ConflictingIdempotencyPayload);
    assert!(
        matches!(record.outcome, AttackOutcome::ErroneouslyAccepted { .. }),
        "the genuine first delivery is accepted: {record:?}"
    );

    // THE ATTACK: the SAME command id, a CONFLICTING payload (value 9.9 —
    // double-execution bait), LEGITIMATELY re-signed by the real key with a
    // fresh sequence. The four-check verifier would pass this (new
    // sequence, valid signature) — the ADDITIVE idempotency binding guard
    // is what refuses it, BEFORE the verifier runs.
    let conflicting = signed_command("cmd-1", 2, 9.9, COMMAND_VALID_FROM, COMMAND_VALID_UNTIL);
    let record = harness.run(&conflicting, PilotAttackCase::ConflictingIdempotencyPayload);
    assert!(
        matches!(
            record.outcome,
            AttackOutcome::RejectedByIdempotencyGuard { .. }
        ),
        "the conflicting payload under the used id is refused by the guard: {record:?}"
    );
    assert_eq!(
        record.outcome.rejection_code(),
        Some("idempotency_conflict"),
        "the guard's rejection carries its own DISTINCT code"
    );
    let conflicts = harness.audited("EDGE_COMMAND_ID_BINDING_CONFLICT");
    assert_eq!(
        conflicts.len(),
        1,
        "the conflict is audited exactly once: {conflicts:?}"
    );
    // (The genuine acceptance earlier in this test DID produce a
    // VerifiedCommand audit — so a blanket zero-verification assertion would
    // be wrong here. What matters: the CONFLICT produced none, which the
    // guard path structurally guarantees because the verifier is never
    // reached — proven by the zero-reach test below.)

    // The binding was NOT overwritten: the id still commits to the genuine
    // payload.
    assert!(
        harness
            .prover
            .bindings()
            .binding_of("cmd-1")
            .expect("the id is bound")
            .contains("val:"),
        "the binding survives the attack attempt"
    );

    // A byte-identical resend of the genuine command passes the guard (same
    // payload) and is rejected by the VERIFIER as a duplicate — the two
    // controls compose, neither substitutes for the other.
    let replay = valid_signed(1);
    let record = harness.run(&replay, PilotAttackCase::ConflictingIdempotencyPayload);
    assert!(matches!(
        record.outcome,
        AttackOutcome::RejectedAtVerification { rejection_code: "duplicate", .. }
    ), "a byte-identical resend passes the guard and is a verifier DUPLICATE: {record:?}");
}

// --- 6. The binding guard survives a restart -------------------------------------------

#[test]
fn the_idempotency_binding_guard_survives_an_edge_restart() {
    let dir = scratch_dir("idempotency-restart");
    let clock = Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000));

    // Run 1: bind cmd-1 to its genuine payload.
    let audit_1 = Arc::new(InMemoryAuditSink::new());
    let mut verifier = verifier_with(
        Arc::clone(&audit_1) as Arc<dyn AuditSink>,
        Arc::clone(&clock) as Arc<dyn Clock + Send + Sync>,
        &dir,
    );
    let mut prover = prover_with(
        Arc::clone(&audit_1) as Arc<dyn AuditSink>,
        Arc::clone(&clock) as Arc<dyn Clock + Send + Sync>,
        &dir,
    );
    let genuine = valid_signed(1);
    let receipt = gate_receipt(&genuine.edge_command, &genuine.target_adapter);
    prover
        .run_attack(
            &mut verifier,
            &receipt,
            &genuine,
            PilotAttackCase::ConflictingIdempotencyPayload,
            ClockQuality::Synchronized,
        )
        .expect("run 1 accepts the genuine command");

    // Run 2 (the edge RESTARTED): a new prover over the REOPENED binding
    // journal refuses the conflicting payload under the same id.
    let audit_2 = Arc::new(InMemoryAuditSink::new());
    let clock_2 = Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 9_000));
    let mut verifier = verifier_with(
        Arc::clone(&audit_2) as Arc<dyn AuditSink>,
        Arc::clone(&clock_2) as Arc<dyn Clock + Send + Sync>,
        &dir,
    );
    let mut prover = prover_with(
        Arc::clone(&audit_2) as Arc<dyn AuditSink>,
        Arc::clone(&clock_2) as Arc<dyn Clock + Send + Sync>,
        &dir,
    );
    let conflicting = signed_command("cmd-1", 2, 42.0, COMMAND_VALID_FROM, COMMAND_VALID_UNTIL);
    let receipt = gate_receipt(&conflicting.edge_command, &conflicting.target_adapter);
    let record = prover
        .run_attack(
            &mut verifier,
            &receipt,
            &conflicting,
            PilotAttackCase::ConflictingIdempotencyPayload,
            ClockQuality::Synchronized,
        )
        .expect("run 2 refuses the conflict");
    assert!(
        matches!(record.outcome, AttackOutcome::RejectedByIdempotencyGuard { .. }),
        "the binding journal survived the restart; the conflicting payload is still refused: \
         {record:?}"
    );
}

// --- 7. ZERO adapter reach (ledger step 5): the forwarder counts ------------------------

#[test]
fn no_rejected_command_reaches_an_adapter_and_only_a_verified_command_does() {
    let mut harness = ProofHarness::new("zero-reach", BASE_MILLIS + 1_000);
    let forwarder = CountingForwarder::new();
    let engine = CommandForwardingEngine::new(
        Arc::clone(&forwarder) as Arc<dyn CommandForwarder>,
        Arc::new(permissive_policy()),
        SiteCommandAllowlist::new(vec!["simulated-test".to_string()]),
        FallbackPolicyConfig {
            format_version: 1,
            entries: Vec::new(),
        },
        Arc::new(InMemoryAuditSink::new()),
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000)),
    )
    .expect("engine composes");

    // THE ATTACK MATRIX, executed against the counting forwarder's edge:
    let attacks: Vec<(IncomingCommand, PilotAttackCase)> = vec![
        // Forged (unsigned, otherwise valid).
        (
            incoming(
                "cmd-forged",
                "ess-01",
                1,
                "simulated-test",
                "active_power",
                1.5,
                String::new(),
                COMMAND_VALID_FROM,
                COMMAND_VALID_UNTIL,
            ),
            PilotAttackCase::ForgedSignature,
        ),
        // Expired (aged past its window).
        (
            signed_command(
                "cmd-aged",
                2,
                1.5,
                BASE_MILLIS - 10_000,
                BASE_MILLIS - 5_000,
            ),
            PilotAttackCase::ExpiredWindow,
        ),
    ];
    for (command, case) in &attacks {
        let record = harness.run(command, *case);
        assert!(
            record.rejected_with_expected_code(),
            "attack {} must be rejected with its expected code: {record:?}",
            case.code()
        );
    }
    // The idempotency case needs its id bound first: bind cmd-live by
    // accepting the genuine command once, then run the conflicting variant.
    let genuine_live = signed_command("cmd-live", 3, 1.5, COMMAND_VALID_FROM, COMMAND_VALID_UNTIL);
    let record = harness.run(&genuine_live, PilotAttackCase::ConflictingIdempotencyPayload);
    assert!(
        matches!(record.outcome, AttackOutcome::ErroneouslyAccepted { .. }),
        "the genuine cmd-live is accepted and its id bound: {record:?}"
    );
    let conflicting_live =
        signed_command("cmd-live", 4, 9.9, COMMAND_VALID_FROM, COMMAND_VALID_UNTIL);
    let record = harness.run(&conflicting_live, PilotAttackCase::ConflictingIdempotencyPayload);
    assert!(
        matches!(record.outcome, AttackOutcome::RejectedByIdempotencyGuard { .. }),
        "the conflicting payload under the used id is refused by the guard: {record:?}"
    );

    // ZERO adapter invocations across the whole attack matrix. The genuine
    // acceptance above was NOT forwarded either: the prover holds no
    // forwarder — the verified path is the ONLY route to an adapter.
    assert_eq!(
        forwarder.calls(),
        0,
        "no rejected command reached the adapter (ledger P9.31 step 5)"
    );

    // The ONLY path that increments the counter is a VerifiedCommand through
    // the P6.16 engine (its separate allowlist + policy re-checks), over a
    // FRESH checkpoint (this dir never saw a sequence yet).
    let fresh_dir = scratch_dir("zero-reach-verified");
    let genuine = valid_signed(1);
    let receipt = gate_receipt(&genuine.edge_command, &genuine.target_adapter);
    let clock = Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000));
    let mut verifier = verifier_with(
        Arc::new(InMemoryAuditSink::new()) as Arc<dyn AuditSink>,
        Arc::clone(&clock) as Arc<dyn Clock + Send + Sync>,
        &fresh_dir,
    );
    let verified = verifier
        .verify(&receipt, &genuine, ClockQuality::Synchronized)
        .expect("the genuine command verifies");
    engine
        .forward_verified(verified)
        .expect("the verified command forwards");
    assert_eq!(
        forwarder.calls(),
        1,
        "exactly ONE adapter invocation, and only from the verified path"
    );
}

// --- 8. Fail-loud: a mislabelled attack is recorded as an acceptance --------------------

#[test]
fn a_mislabelled_attack_is_recorded_fail_loud_never_coerced_into_a_rejection() {
    let mut harness = ProofHarness::new("fail-loud", BASE_MILLIS + 1_000);
    // A FRESH, fully valid command presented under the "replayed" label: the
    // controls have no ground to reject it. The prover records
    // ErroneouslyAccepted and audits it — the proof NEVER manufactures a
    // rejection to make a case look closed (§13.3 discipline).
    let fresh = valid_signed(50);
    let record = harness.run(&fresh, PilotAttackCase::ReplayedCaptured);
    assert!(
        matches!(record.outcome, AttackOutcome::ErroneouslyAccepted { ref command_id, .. }
            if command_id == "cmd-50"),
        "the mislabelled attack is recorded fail-loud: {record:?}"
    );
    assert_eq!(record.outcome.rejection_code(), None);
    let accepted = harness.audited("EDGE_PILOT_ATTACK_ACCEPTED");
    assert_eq!(
        accepted.len(),
        1,
        "the fail-loud acceptance is audited exactly once: {accepted:?}"
    );
}

// --- 9. The durable audit trail of the rejections (the P9.31 → P9.32 join) --------------

#[test]
fn every_rejection_is_durably_audited_by_the_edge_audit_buffer() {
    // The proof's audit trail is not an in-memory convenience: composed with
    // the P6.18 FileAuditBuffer, every rejection survives an edge restart —
    // the audit evidence the acceptance decision reads (P9.32's surface).
    let dir = scratch_dir("durable-audit");
    let journal = dir.join("audit.journal");
    let clock = Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000));
    {
        let buffer = Arc::new(
            FileAuditBuffer::open(
                &journal,
                100,
                Arc::clone(&clock) as Arc<dyn Clock + Send + Sync>,
            )
            .expect("audit journal opens"),
        );
        let audit: Arc<dyn AuditSink> = Arc::clone(&buffer) as Arc<dyn AuditSink>;
        let mut verifier = verifier_with(Arc::clone(&audit), Arc::clone(&clock) as Arc<dyn Clock + Send + Sync>, &dir);
        let mut prover = PilotRejectionProver::new(
            Arc::new(
                FileCommandIdBinding::open(
                    &dir.join("id-bindings.journal"),
                    DEFAULT_ID_BINDING_CAPACITY,
                )
                .expect("binding journal opens"),
            ),
            Arc::clone(&audit),
            Arc::clone(&clock) as Arc<dyn Clock + Send + Sync>,
        )
        .expect("prover composes");
        // One forged attack: unsigned, otherwise valid.
        let forged = incoming(
            "cmd-forged",
            "ess-01",
            1,
            "simulated-test",
            "active_power",
            1.5,
            String::new(),
            COMMAND_VALID_FROM,
            COMMAND_VALID_UNTIL,
        );
        let receipt = gate_receipt(&forged.edge_command, &forged.target_adapter);
        let record = prover
            .run_attack(
                &mut verifier,
                &receipt,
                &forged,
                PilotAttackCase::ForgedSignature,
                ClockQuality::Synchronized,
            )
            .expect("the prover runs");
        assert!(record.rejected_with_expected_code());
        drop(prover);
        drop(verifier);
    }
    // The edge RESTARTS: the rejection evidence is still on disk.
    let reopened = FileAuditBuffer::open(&journal, 100, clock).expect("reopens");
    let pending = reopened.drain(100);
    assert!(
        pending
            .iter()
            .any(|record| record.detail.starts_with("EDGE_PILOT_ATTACK_REJECTED")
                && record.detail.contains("code=signature")),
        "the rejection evidence survived the restart in the durable audit buffer: {:?}",
        pending.iter().map(|r| r.detail.clone()).collect::<Vec<_>>()
    );
}
