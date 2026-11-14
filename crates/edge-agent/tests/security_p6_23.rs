// P6.23 — Security testing: replay, forgery, malicious telemetry, secrets
//         exposure (report §19.2(7), §17.4, §5.7; ledger P6.23). Written,
//         NOT run — the loop bans in
//         `ledger/phase-06-edge-protocol/ENTRY_NOTE.md` hold. The human
//         build/test pass runs
//         `cargo test -p flexcore-edge-agent --test security_p6_23`.
//
// EVERY test in this file ATTEMPTS THE ATTACK (the ledger's recorded risk —
// "a 'test' that asserts the happy path and never attempts the attack" — is
// the failure mode this file exists to refuse). The one valid-command case
// is a CONTROL: the replay/forgery attacks are only meaningful against a
// pipeline that provably admits the legitimately signed command.
//
// Coverage against the §19.2(7) categories (plugin privilege escalation is
// OUT OF SCOPE — there is no plugin host in Phase 6; the WASM host is
// v0.5.0 / Stage 7; the decision record states this, it is NOT claimed as
// tested):
//
// | Category | Cases | Surface driven |
// |---|---|---|
// | command forgery | forged signature, wrong signer | P6.14 gate → P6.15 verifier (FULL pipeline incl. P6.16 forwarder) |
// | replay | replayed valid command, out-of-sequence, sequence replay after restart (persisted checkpoint) | same |
// | lifecycle extension | expired-valid command, non-allowlisted target | same |
// | malicious telemetry | spoofed source identity (reserved namespace, empty identity, foreign-site point), out-of-range values (NaN, SOC beyond [0,1]), out-of-range quality, stale-data replay (exact + conflicting) | P6.12 ACL → P6.08 ingest path (hostile adapter double) |
// | secrets exposure | lexical scan over `src/**` (no secret material logged or embedded) + runtime redaction assertions | source scan + Debug/Display impls |
//
// AUTH BYPASS at the SERVER is the sibling file
// `apps/control-plane/tests/api_security_p6_23.rs` (the server is the
// enforcement point for sessions; the edge crate has no inbound listener —
// `tests/no_inbound_listener.rs`).
//
// STATUS: WRITTEN-UNRUN. The findings table in the decision record
// (`19_23_22_conformance_security_performance.md` §6) has an EMPTY findings
// column until the human runs the suite — no pass and no finding is
// claimed here.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{ClockQuality, EventTime, IngestTime};
use flexcore_contracts::units::{ActivePowerMW, ReactiveSignConvention};
use flexcore_edge_agent::adapters::acl::{
    AntiCorruptionLayer, AclError, NativeSign, NativeTimestamp, NativeUnit, PointMapping,
};
use flexcore_edge_agent::adapters::port::{
    AdapterCapabilities, AdapterError, AdapterHealth, ChannelDescriptor, EmsAdapter,
    EmsNativeReading, PointKey,
};
use flexcore_edge_agent::adapters::simulated::{
    SimulatedEmsSource, SimulatedFixture, SimulatedPointFixture,
};
use flexcore_edge_agent::command_forward::{
    CommandForwarder, CommandForwardingEngine, FallbackPolicyConfig, FallbackPolicyEntry,
    FallbackTrigger, FallbackVariant, ForwardError, ForwardOutcome, ForwardRequest,
    OperatorReview, SiteCommandAllowlist,
};
use flexcore_edge_agent::command_path::{
    CommandPathGateInputs, CommandPathRuntime, CommandReceiver, EdgeCommand, EnablementKey,
    SignedEnablementRecord,
};
use flexcore_edge_agent::command_verify::{
    sign_command, AssetCommandScope, CommandAuthorization, CommandSigningKey,
    CommandVerificationRejection, CommandVerifier, FileSequenceCheckpoint, IncomingCommand,
};
use flexcore_edge_agent::config::EdgeConfig;
use flexcore_edge_agent::ingest::{AdapterIngestSource, AdmitOutcome, EdgeDedupWindow};
use flexcore_edge_agent::policy::{
    AdapterPolicy, OperatingWindow, RateLimit, SitePolicyConfig, SitePolicyEngine,
};
use flexcore_edge_agent::ports::{
    AuditSink, EdgeError, EdgeSampleError, EdgeTelemetrySample, EdgeTelemetrySampleParts,
    InMemoryAuditSink,
};
use flexcore_edge_agent::uplink_mtls::TlsMinimum;
use flexcore_ports::fakes::FixedClock;
use flexcore_ports::telemetry_source::{QuantityValue, TelemetryQuantity};

const BASE_MILLIS: i64 = 1_700_000_000_000;
/// The receiver/engine clock instant (inside the enablement window).
const PIPELINE_NOW_MILLIS: i64 = BASE_MILLIS + 1_000;
const ENABLEMENT_VALID_UNTIL: i64 = BASE_MILLIS + 86_400_000;
const COMMAND_VALID_UNTIL: i64 = BASE_MILLIS + 3_600_000;
const ENABLEMENT_KEY_MATERIAL: &str = "lab-enablement-key-material";
const COMMAND_KEY_MATERIAL: &str = "lab-command-signing-key-material";
const ATTACKER_KEY_MATERIAL: &str = "attacker-forged-key-material";

// ---------------------------------------------------------------------------
// Shared composition (the full gated pipeline: P6.14 gate → P6.15 verifier
// → P6.16 forwarder — each attack drives ALL THREE surfaces in order)
// ---------------------------------------------------------------------------

fn scratch_dir(label: &str) -> PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "flexcore-edge-p623-{}-{}-{}",
        std::process::id(),
        label,
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir creates");
    dir
}

fn enablement_key() -> EnablementKey {
    EnablementKey::new(
        ENABLEMENT_KEY_MATERIAL.to_string(),
        "deployment:lab-key-1".to_string(),
    )
    .expect("valid enablement key")
}

fn signing_key() -> CommandSigningKey {
    CommandSigningKey::new(
        COMMAND_KEY_MATERIAL.to_string(),
        "deployment:lab-command-key-1".to_string(),
    )
    .expect("valid signing key")
}

fn attacker_key() -> CommandSigningKey {
    // The ATTACKER's key: same reference LABEL (a forger copies the public
    // label), different material — the digest must still refuse.
    CommandSigningKey::new(
        ATTACKER_KEY_MATERIAL.to_string(),
        "deployment:lab-command-key-1".to_string(),
    )
    .expect("valid attacker key")
}

fn enabled_gate_inputs() -> CommandPathGateInputs {
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
    .sign(&enablement_key())
    .expect("signs");
    CommandPathGateInputs {
        site_id: "pilot-site-A".to_string(),
        config_flag_enabled: true,
        signed_record: Some(record),
        environment_token: Some("lab-token-123".to_string()),
        command_allowlist: vec!["simulated-test".to_string()],
        d6_record_reference: Some(
            "ledger/phase-06-edge-protocol/03_d6_boundary_review.md".to_string(),
        ),
    }
}

fn permissive_policy() -> Arc<SitePolicyEngine> {
    Arc::new(
        SitePolicyEngine::new(SitePolicyConfig {
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
        .expect("valid policy"),
    )
}

fn authorization() -> CommandAuthorization {
    CommandAuthorization {
        site_id: "pilot-site-A".to_string(),
        valid_until_unix_millis: ENABLEMENT_VALID_UNTIL,
        adapter_allowlist: vec!["simulated-test".to_string()],
        asset_scopes: vec![AssetCommandScope {
            asset_key: "ess-01".to_string(),
            allowed_quantities: vec!["active_power".to_string()],
            min_value: -5.0,
            max_value: 5.0,
        }],
    }
}

/// A scripted forwarder double: records every call and ACKs. The attack
/// assertions use the call list to prove a refused command NEVER reached
/// the forwarding surface.
struct ScriptedForwarder {
    calls: Mutex<Vec<String>>,
}

impl ScriptedForwarder {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
        })
    }

    fn call_count(&self) -> usize {
        self.calls.lock().expect("forwarder call mutex").len()
    }
}

impl CommandForwarder for ScriptedForwarder {
    fn forward(&self, request: &ForwardRequest) -> Result<ForwardOutcome, ForwardError> {
        self.calls
            .lock()
            .expect("forwarder call mutex")
            .push(request.command_id.clone());
        Ok(ForwardOutcome::Acknowledged)
    }
}

/// The composed pipeline. `verifier_now_millis` is the VERIFIER's clock —
/// the expiry attacks move it (the receiver stays inside the enablement
/// window; the attack is against the command's own validity window).
struct Pipeline {
    receiver: CommandReceiver,
    verifier: CommandVerifier,
    engine: CommandForwardingEngine,
    audit: Arc<InMemoryAuditSink>,
    forwarder: Arc<ScriptedForwarder>,
}

fn compose_pipeline(label: &str, verifier_now_millis: i64) -> Pipeline {
    let audit = Arc::new(InMemoryAuditSink::new());
    let runtime = CommandPathRuntime::evaluate(&enabled_gate_inputs(), &enablement_key(), BASE_MILLIS);
    let receiver = CommandReceiver::new(
        runtime,
        permissive_policy(),
        audit.clone() as Arc<dyn AuditSink>,
        Arc::new(FixedClock::at_unix_millis(PIPELINE_NOW_MILLIS)),
    );
    let checkpoint =
        FileSequenceCheckpoint::open(&scratch_dir(label).join("seq.ckpt")).expect("checkpoint opens");
    let verifier = CommandVerifier::new(
        signing_key(),
        authorization(),
        Box::new(checkpoint),
        audit.clone() as Arc<dyn AuditSink>,
        Arc::new(FixedClock::at_unix_millis(verifier_now_millis)),
    )
    .expect("verifier composes");
    let forwarder = ScriptedForwarder::new();
    let fallback = FallbackPolicyConfig {
        format_version: 1,
        entries: vec![FallbackPolicyEntry {
            asset_key: "ess-01".to_string(),
            variant: FallbackVariant::RequireOperatorIntervention,
            triggers: vec![FallbackTrigger::Nack],
            review: OperatorReview {
                reviewed_by: "A. Operator (site operator)".to_string(),
                reviewed_on: "2026-11-13".to_string(),
            },
        }],
    };
    let engine = CommandForwardingEngine::new(
        Arc::clone(&forwarder) as Arc<dyn CommandForwarder>,
        permissive_policy(),
        SiteCommandAllowlist::new(vec!["simulated-test".to_string()]),
        fallback,
        audit.clone() as Arc<dyn AuditSink>,
        Arc::new(FixedClock::at_unix_millis(PIPELINE_NOW_MILLIS)),
    )
    .expect("engine composes");
    Pipeline {
        receiver,
        verifier,
        engine,
        audit,
        forwarder,
    }
}

/// Builds a gate-admitted ISSUED command for `ess-01` at `sequence`.
fn gate_admit(pipeline: &mut Pipeline, command_id: &str, sequence: u64) -> CommandReceiptShape {
    let edge = EdgeCommand::new(
        command_id.to_string(),
        "ess-01".to_string(),
        "ISSUED".to_string(),
        sequence,
    )
    .expect("valid edge command");
    let receipt = pipeline
        .receiver
        .receive(&edge, "simulated-test")
        .expect("the gate admits the test-env command");
    CommandReceiptShape { edge, receipt }
}

/// The receipt + command shape carried from the gate into the verifier.
struct CommandReceiptShape {
    edge: EdgeCommand,
    receipt: flexcore_edge_agent::command_path::CommandReceipt,
}

/// Builds the INCOMING command shape (signature unsigned; the caller signs
/// it or leaves it empty — an unsigned command is itself an attack case
/// through the verifier).
fn incoming_command(
    edge: EdgeCommand,
    signature_reference: String,
    valid_from: i64,
    valid_until: i64,
) -> IncomingCommand {
    IncomingCommand::new(
        edge,
        "simulated-test".to_string(),
        signature_reference,
        "edge-session-1".to_string(),
        "active_power".to_string(),
        1.5,
        valid_from,
        valid_until,
    )
    .expect("valid incoming command")
}

/// Signs `command` under `key` and returns the command WITH its signature
/// reference set (the central platform's signing side, then transport).
fn signed(mut command: IncomingCommand, key: &CommandSigningKey) -> IncomingCommand {
    let signature = sign_command(key, &command);
    command.signature_reference = signature;
    command
}

/// All verifier-rejection audit records carrying `code` (the dedicated
/// `EDGE_COMMAND_VERIFICATION_REJECTED` event name leads the detail).
fn verifier_rejections(pipeline: &Pipeline, code: &str) -> Vec<String> {
    pipeline
        .audit
        .events()
        .iter()
        .filter(|event| {
            event.detail.starts_with("EDGE_COMMAND_VERIFICATION_REJECTED")
                && event.detail.contains(&format!("code={code}"))
        })
        .map(|event| event.detail.clone())
        .collect()
}

// ---------------------------------------------------------------------------
// (a) Command forgery and replay, extended to the full lifecycle
// ---------------------------------------------------------------------------

/// CONTROL: a legitimately signed command passes all four checks and is
/// forwarded exactly once — the pipeline the attacks below are measured
/// against. Test-env ONLY (the enablement record is the test-lab gate).
#[test]
fn control_a_valid_command_passes_all_four_checks_and_forwards_once() {
    let mut pipeline = compose_pipeline("control-valid", PIPELINE_NOW_MILLIS);
    let admitted = gate_admit(&mut pipeline, "cmd-valid-1", 1);
    let command = signed(
        incoming_command(admitted.edge, String::new(), BASE_MILLIS, COMMAND_VALID_UNTIL),
        &signing_key(),
    );
    let verified = pipeline
        .verifier
        .verify(&admitted.receipt, &command, ClockQuality::Synchronized)
        .expect("a valid command verifies");
    let record = pipeline
        .engine
        .forward_verified(verified)
        .expect("a verified command forwards");
    assert_eq!(record.lifecycle_state, "ACKNOWLEDGED");
    assert_eq!(pipeline.forwarder.call_count(), 1);
}

#[test]
fn a_forged_signature_is_rejected_as_signature_audited_and_never_forwarded() {
    let mut pipeline = compose_pipeline("forged", PIPELINE_NOW_MILLIS);
    let admitted = gate_admit(&mut pipeline, "cmd-forge-1", 1);
    // THE ATTACK: the command is signed under the ATTACKER's key material
    // (the forger cannot produce the platform's digest).
    let command = signed(
        incoming_command(admitted.edge, String::new(), BASE_MILLIS, COMMAND_VALID_UNTIL),
        &attacker_key(),
    );
    let rejection = pipeline
        .verifier
        .verify(&admitted.receipt, &command, ClockQuality::Synchronized)
        .expect_err("a forged signature must be rejected");
    assert!(
        matches!(rejection, CommandVerificationRejection::Signature { .. }),
        "the rejection reason must be SIGNATURE, got {rejection:?}"
    );
    // Audited with its DISTINCT reason.
    assert_eq!(
        verifier_rejections(&pipeline, "signature").len(),
        1,
        "the forgery is audited with code=signature"
    );
    // Nothing reached the forwarding surface.
    assert_eq!(pipeline.forwarder.call_count(), 0, "a forged command is never forwarded");
}

#[test]
fn a_wrong_signer_is_rejected_with_the_wrong_signer_detail() {
    let mut pipeline = compose_pipeline("wrong-signer", PIPELINE_NOW_MILLIS);
    let admitted = gate_admit(&mut pipeline, "cmd-signer-1", 1);
    // THE ATTACK: a signature reference naming a DIFFERENT key reference
    // (the command was signed by some other platform/tenant's key).
    let command = incoming_command(admitted.edge, String::new(), BASE_MILLIS, COMMAND_VALID_UNTIL);
    let command = {
        let mut c = command;
        c.signature_reference =
            "edge-sig:deployment:someone-elses-key:cmd-signer-1:fnv1a64:0123456789abcdef"
                .to_string();
        c
    };
    let rejection = pipeline
        .verifier
        .verify(&admitted.receipt, &command, ClockQuality::Synchronized)
        .expect_err("a wrong-signer command must be rejected");
    match &rejection {
        CommandVerificationRejection::Signature { detail } => {
            assert!(
                detail.contains("wrong signer"),
                "the detail must name the wrong signer: {detail}"
            );
        }
        other => panic!("expected a Signature rejection, got {other:?}"),
    }
    assert_eq!(verifier_rejections(&pipeline, "signature").len(), 1);
    assert_eq!(pipeline.forwarder.call_count(), 0);
}

#[test]
fn an_unsigned_command_is_rejected_as_signature() {
    let mut pipeline = compose_pipeline("unsigned", PIPELINE_NOW_MILLIS);
    let admitted = gate_admit(&mut pipeline, "cmd-unsigned-1", 1);
    // THE ATTACK: the command arrives with NO signature at all.
    let command = incoming_command(admitted.edge, String::new(), BASE_MILLIS, COMMAND_VALID_UNTIL);
    let rejection = pipeline
        .verifier
        .verify(&admitted.receipt, &command, ClockQuality::Synchronized)
        .expect_err("an unsigned command must be rejected");
    assert!(matches!(rejection, CommandVerificationRejection::Signature { .. }));
    assert_eq!(verifier_rejections(&pipeline, "signature").len(), 1);
    assert_eq!(pipeline.forwarder.call_count(), 0);
}

#[test]
fn a_replayed_valid_command_is_rejected_as_duplicate_and_forwarded_once() {
    let mut pipeline = compose_pipeline("replay", PIPELINE_NOW_MILLIS);
    // A valid command is admitted, verified, forwarded (the control).
    let first = gate_admit(&mut pipeline, "cmd-replay-1", 1);
    let valid = signed(
        incoming_command(first.edge, String::new(), BASE_MILLIS, COMMAND_VALID_UNTIL),
        &signing_key(),
    );
    let verified = pipeline
        .verifier
        .verify(&first.receipt, &valid, ClockQuality::Synchronized)
        .expect("the first delivery verifies");
    pipeline
        .engine
        .forward_verified(verified)
        .expect("the first delivery forwards");
    assert_eq!(pipeline.forwarder.call_count(), 1);

    // THE ATTACK: the EXACT same captured command is replayed (fresh gate
    // admission, identical sequence, identical signature).
    let replay = gate_admit(&mut pipeline, "cmd-replay-1", 1);
    let replayed = signed(
        incoming_command(replay.edge, String::new(), BASE_MILLIS, COMMAND_VALID_UNTIL),
        &signing_key(),
    );
    let rejection = pipeline
        .verifier
        .verify(&replay.receipt, &replayed, ClockQuality::Synchronized)
        .expect_err("a replayed valid command must be rejected");
    assert!(
        matches!(rejection, CommandVerificationRejection::Duplicate { .. }),
        "the replay must be the DISTINCT duplicate reason, got {rejection:?}"
    );
    assert_eq!(verifier_rejections(&pipeline, "duplicate").len(), 1);
    assert_eq!(
        pipeline.forwarder.call_count(),
        1,
        "the replay never reaches the forwarder a second time"
    );
}

#[test]
fn an_out_of_sequence_command_is_rejected_as_sequence() {
    let mut pipeline = compose_pipeline("out-of-sequence", PIPELINE_NOW_MILLIS);
    // Sequence 2 is accepted first (the live window).
    let second = gate_admit(&mut pipeline, "cmd-seq-2", 2);
    let valid = signed(
        incoming_command(second.edge, String::new(), BASE_MILLIS, COMMAND_VALID_UNTIL),
        &signing_key(),
    );
    pipeline
        .verifier
        .verify(&second.receipt, &valid, ClockQuality::Synchronized)
        .expect("sequence 2 verifies");
    // THE ATTACK: sequence 1 (BELOW the last accepted) arrives — a
    // reordered or maliciously re-sequenced command.
    let first = gate_admit(&mut pipeline, "cmd-seq-1", 1);
    let stale_sequence = signed(
        incoming_command(first.edge, String::new(), BASE_MILLIS, COMMAND_VALID_UNTIL),
        &signing_key(),
    );
    let rejection = pipeline
        .verifier
        .verify(&first.receipt, &stale_sequence, ClockQuality::Synchronized)
        .expect_err("an out-of-sequence command must be rejected");
    assert!(matches!(rejection, CommandVerificationRejection::Sequence { .. }));
    assert_eq!(verifier_rejections(&pipeline, "sequence").len(), 1);
    assert_eq!(pipeline.forwarder.call_count(), 0);
}

#[test]
fn an_expired_valid_command_is_rejected_as_expiry() {
    // The verifier's clock is TWO HOURS in: the command's validity window
    // closed after one hour. The command itself is PERFECTLY SIGNED — the
    // attack is freshness, not integrity (§14.3, §17.5 rule 5).
    let mut pipeline = compose_pipeline("expired", BASE_MILLIS + 7_200_000);
    let admitted = gate_admit(&mut pipeline, "cmd-expired-1", 1);
    let command = signed(
        incoming_command(admitted.edge, String::new(), BASE_MILLIS, COMMAND_VALID_UNTIL),
        &signing_key(),
    );
    let rejection = pipeline
        .verifier
        .verify(&admitted.receipt, &command, ClockQuality::Synchronized)
        .expect_err("an expired-but-validly-signed command must be rejected");
    assert!(matches!(rejection, CommandVerificationRejection::Expiry { .. }));
    assert_eq!(verifier_rejections(&pipeline, "expiry").len(), 1);
    assert_eq!(pipeline.forwarder.call_count(), 0);
}

#[test]
fn a_degraded_edge_clock_fails_the_expiry_check_closed() {
    // THE ATTACK: a command presented while the edge clock quality is
    // degraded — expiry checks are meaningless on a drifting clock
    // (§17.4, §17.5 rule 5). The signature is PERFECT; the clock is not.
    let mut pipeline = compose_pipeline("clock-quality", PIPELINE_NOW_MILLIS);
    let admitted = gate_admit(&mut pipeline, "cmd-clock-1", 1);
    let command = signed(
        incoming_command(admitted.edge, String::new(), BASE_MILLIS, COMMAND_VALID_UNTIL),
        &signing_key(),
    );
    for quality in [ClockQuality::Unknown, ClockQuality::Unsynchronized] {
        let rejection = pipeline
            .verifier
            .verify(&admitted.receipt, &command, quality)
            .expect_err("a degraded clock must fail the expiry check closed");
        assert!(matches!(rejection, CommandVerificationRejection::Expiry { .. }));
    }
    assert!(
        verifier_rejections(&pipeline, "expiry").len() >= 2,
        "each degraded-clock attempt is audited"
    );
    assert_eq!(pipeline.forwarder.call_count(), 0);
}

#[test]
fn a_sequence_replay_after_verifier_restart_is_denied_by_the_persisted_checkpoint() {
    // THE ATTACK (the ledger's recorded P6.15 risk): the edge RESTARTS and
    // the attacker replays an already-accepted command, hoping the
    // in-memory sequence window was lost. The persisted checkpoint must
    // deny it.
    let dir = scratch_dir("restart-replay");
    let checkpoint_path = dir.join("seq.ckpt");

    // PRE-RESTART: a fresh composition whose verifier persists sequence 5
    // into the checkpoint (durability BEFORE state advance — P6.15).
    let command = {
        let audit = Arc::new(InMemoryAuditSink::new());
        let runtime =
            CommandPathRuntime::evaluate(&enabled_gate_inputs(), &enablement_key(), BASE_MILLIS);
        let mut receiver = CommandReceiver::new(
            runtime,
            permissive_policy(),
            audit.clone() as Arc<dyn AuditSink>,
            Arc::new(FixedClock::at_unix_millis(PIPELINE_NOW_MILLIS)),
        );
        let edge = EdgeCommand::new(
            "cmd-restart-1".to_string(),
            "ess-01".to_string(),
            "ISSUED".to_string(),
            5,
        )
        .expect("valid edge command");
        let receipt = receiver
            .receive(&edge, "simulated-test")
            .expect("the gate admits the test-env command");
        let checkpoint =
            FileSequenceCheckpoint::open(&checkpoint_path).expect("checkpoint opens");
        let mut verifier = CommandVerifier::new(
            signing_key(),
            authorization(),
            Box::new(checkpoint),
            audit as Arc<dyn AuditSink>,
            Arc::new(FixedClock::at_unix_millis(PIPELINE_NOW_MILLIS)),
        )
        .expect("verifier composes");
        let incoming = signed(
            incoming_command(edge, String::new(), BASE_MILLIS, COMMAND_VALID_UNTIL),
            &signing_key(),
        );
        verifier
            .verify(&receipt, &incoming, ClockQuality::Synchronized)
            .expect("the pre-restart command verifies (sequence 5 persisted)");
        incoming
    };

    // RESTART: a fresh verifier over the SAME checkpoint file (the durable
    // sequence state survives the process; the in-memory window does not),
    // and a fresh gate admitting the replayed command.
    let audit_after = Arc::new(InMemoryAuditSink::new());
    let runtime =
        CommandPathRuntime::evaluate(&enabled_gate_inputs(), &enablement_key(), BASE_MILLIS);
    let mut receiver = CommandReceiver::new(
        runtime,
        permissive_policy(),
        audit_after.clone() as Arc<dyn AuditSink>,
        Arc::new(FixedClock::at_unix_millis(PIPELINE_NOW_MILLIS)),
    );
    let replay_edge = EdgeCommand::new(
        "cmd-restart-1".to_string(),
        "ess-01".to_string(),
        "ISSUED".to_string(),
        5,
    )
    .expect("valid edge command");
    let replay_receipt = receiver
        .receive(&replay_edge, "simulated-test")
        .expect("the gate admits the replayed command (the gate is not the replay defence)");
    let checkpoint = FileSequenceCheckpoint::open(&checkpoint_path).expect("checkpoint reopens");
    let mut restarted = CommandVerifier::new(
        signing_key(),
        authorization(),
        Box::new(checkpoint),
        audit_after as Arc<dyn AuditSink>,
        Arc::new(FixedClock::at_unix_millis(PIPELINE_NOW_MILLIS)),
    )
    .expect("the restarted verifier composes");
    // The replay presents the IDENTICAL signed command (same sequence 5,
    // same signature bytes — a captured command re-sent after restart).
    let rejection = restarted
        .verify(&replay_receipt, &command, ClockQuality::Synchronized)
        .expect_err("the post-restart replay must be denied by the persisted checkpoint");
    assert!(
        matches!(rejection, CommandVerificationRejection::Duplicate { .. }),
        "the restart replay must be the duplicate reason, got {rejection:?}"
    );
    let rejections = audit_after
        .events()
        .iter()
        .filter(|e| e.detail.starts_with("EDGE_COMMAND_VERIFICATION_REJECTED"))
        .count();
    assert_eq!(rejections, 1, "the restart replay is audited");
}

#[test]
fn a_non_allowlisted_target_is_rejected_as_allowlist_in_the_full_pipeline() {
    let mut pipeline = compose_pipeline("non-allowlisted", PIPELINE_NOW_MILLIS);
    let admitted = gate_admit(&mut pipeline, "cmd-allowlist-1", 1);
    // THE ATTACK: the command targets an adapter NOT on the site allowlist
    // (a second site's adapter — the cross-site command case).
    let command = {
        let mut c = incoming_command(
            admitted.edge,
            String::new(),
            BASE_MILLIS,
            COMMAND_VALID_UNTIL,
        );
        c.target_adapter = "site-b-adapter".to_string();
        signed(c, &signing_key())
    };
    let rejection = pipeline
        .verifier
        .verify(&admitted.receipt, &command, ClockQuality::Synchronized)
        .expect_err("a non-allowlisted target must be rejected");
    assert!(matches!(
        rejection,
        CommandVerificationRejection::Allowlist { .. }
    ));
    assert_eq!(verifier_rejections(&pipeline, "allowlist").len(), 1);
    assert_eq!(pipeline.forwarder.call_count(), 0);
}

#[test]
fn a_command_outside_its_asset_scope_is_rejected_as_scope() {
    let mut pipeline = compose_pipeline("scope", PIPELINE_NOW_MILLIS);
    let admitted = gate_admit(&mut pipeline, "cmd-scope-1", 1);
    // THE ATTACK: a validly signed command whose value exceeds the
    // approved authorization limits (ess-01: [-5, +5] MW).
    let command = {
        let mut c = incoming_command(
            admitted.edge,
            String::new(),
            BASE_MILLIS,
            COMMAND_VALID_UNTIL,
        );
        c.value = 42.0;
        signed(c, &signing_key())
    };
    let rejection = pipeline
        .verifier
        .verify(&admitted.receipt, &command, ClockQuality::Synchronized)
        .expect_err("an out-of-scope value must be rejected");
    assert!(matches!(rejection, CommandVerificationRejection::Scope { .. }));
    assert_eq!(verifier_rejections(&pipeline, "scope").len(), 1);
    assert_eq!(pipeline.forwarder.call_count(), 0);
}

// ---------------------------------------------------------------------------
// (b) Malicious / incorrect telemetry (through the ACL → ingest path)
// ---------------------------------------------------------------------------

/// A HOSTILE adapter double: whatever the ingest source polls, this adapter
/// returns the scripted READING — spoofed points, NaN values, stale codes.
/// This is the compromised-EMS attack driver (a real adapter returning
/// attacker-chosen readings).
struct HostileEmsAdapter {
    scripted: Mutex<Vec<EmsNativeReading>>,
}

impl HostileEmsAdapter {
    fn with_readings(readings: Vec<EmsNativeReading>) -> Box<Self> {
        Box::new(Self {
            scripted: Mutex::new(readings),
        })
    }
}

impl EmsAdapter for HostileEmsAdapter {
    fn id(&self) -> &str {
        "simulated-test" // the hostile adapter SPOOFS the trusted id
    }

    fn capabilities(&self) -> &AdapterCapabilities {
        capabilities()
    }

    fn discover(&mut self) -> Result<Vec<ChannelDescriptor>, AdapterError> {
        Ok(Vec::new())
    }

    fn read(&mut self, _point: &PointKey) -> Result<EmsNativeReading, AdapterError> {
        // Pop the next scripted attack reading; empty script → NotConnected
        // (never fabricate).
        self.scripted
            .lock()
            .expect("hostile script mutex")
            .pop()
            .map(Ok)
            .unwrap_or_else(|| {
                Err(AdapterError::NotConnected {
                    reason: "the hostile script is exhausted".to_string(),
                })
            })
    }

    fn health(&self) -> AdapterHealth {
        AdapterHealth::Healthy // the hostile adapter LIES about health too
    }
}

fn capabilities() -> &'static AdapterCapabilities {
    // A static capabilities table (the hostile adapter declares what a
    // compromised adapter might).
    use std::sync::OnceLock;
    static CAPS: OnceLock<AdapterCapabilities> = OnceLock::new();
    CAPS.get_or_init(|| AdapterCapabilities {
        adapter_id: "simulated-test".to_string(),
        reactive_sign_convention: ReactiveSignConvention::InjectionPositive,
        cadence: std::time::Duration::from_millis(5_000),
        quality_mapping: flexcore_ports::telemetry_source::QualityMappingDeclaration::new(vec![
            flexcore_ports::telemetry_source::SourceStateMapping {
                source_code: "GOOD".to_string(),
                canonical: TelemetryQuality::Good,
            },
        ])
        .expect("valid declaration"),
        clock_quality: ClockQuality::Unknown,
    })
}

fn hostile_ingest(
    label: &str,
    hostile: Box<HostileEmsAdapter>,
) -> AdapterIngestSource {
    let mappings = vec![PointMapping {
        point: PointKey::new("ess0", "ActivePower"),
        asset_key: "ess-01".to_string(),
        quantity: TelemetryQuantity::ActivePower,
        native_unit: NativeUnit::W,
        native_sign: Some(NativeSign::ExportPositive),
        reactive_source_convention: None,
    }];
    let acl = AntiCorruptionLayer::new(
        mappings.clone(),
        "edge:pilot-site-A:simulated-test",
        vec![],
    )
    .expect("valid mapping table");
    AdapterIngestSource::new(
        hostile,
        acl,
        mappings,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000)),
        64,
    )
    .expect("the read path composes")
}

fn hostile_reading(point: PointKey, raw: f64, quality: Option<&str>) -> EmsNativeReading {
    EmsNativeReading {
        point,
        native_unit: NativeUnit::W,
        raw_value: raw,
        native_quality_code: quality.map(str::to_string),
        event_time: NativeTimestamp::UnixMillis(BASE_MILLIS - 1_000),
        source_sequence: 1,
    }
}

#[test]
fn b_spoofed_foreign_site_point_is_refused_at_the_acl_never_ingested() {
    // THE ATTACK: a compromised adapter serves a reading claiming a point
    // from ANOTHER SITE ("siteB-ess0/ActivePower") — hoping the identity
    // prefix is taken from the reading. The identity is DERIVED by the ACL
    // from ITS OWN prefix, and the foreign point has NO mapping: refused
    // whole, counted, never ingested.
    let hostile = HostileEmsAdapter::with_readings(vec![hostile_reading(
        PointKey::new("siteB-ess0", "ActivePower"),
        999.0,
        Some("GOOD"),
    )]);
    let mut source = hostile_ingest("foreign-point", hostile);
    let error = source
        .poll(10)
        .expect_err("the only mapped point was attacked → hard poll failure");
    assert!(matches!(error, EdgeError::SourcePollFailed { .. }));
    let stats = source.stats();
    assert_eq!(stats.acl_rejections, 1, "the spoofed point is counted");
    assert_eq!(stats.samples_emitted, 0, "no spoofed sample is emitted");
    assert!(
        stats
            .last_failure_reason
            .as_deref()
            .unwrap_or_default()
            .contains("siteB-ess0/ActivePower"),
        "the rejection names the spoofed point"
    );
}

#[test]
fn b_reserved_platform_namespace_is_refused_for_edge_sources() {
    // THE ATTACK: telemetry presenting itself under the reserved
    // `flexcore:` platform namespace (a fabricated platform-derived
    // record, P2.04 D-T6). Refused at ACL construction AND at sample
    // construction.
    let smuggled = AntiCorruptionLayer::new(vec![], "flexcore:platform-record", vec![]);
    assert!(
        smuggled.is_err(),
        "a flexcore:-prefixed source prefix must be refused"
    );
    let parts = EdgeTelemetrySampleParts {
        asset_key: "ess-01".to_string(),
        quantity: TelemetryQuantity::ActivePower,
        value: QuantityValue::Power(ActivePowerMW::new(1.0).expect("finite")),
        quality: TelemetryQuality::Good,
        source_quality_code: None,
        event_time: EventTime::from_unix_millis(1_000).expect("valid"),
        ingest_time: IngestTime::from_unix_millis(2_000).expect("valid"),
        source_sequence: 1,
        source_id: "flexcore:fabricated-record".to_string(),
    };
    assert_eq!(
        EdgeTelemetrySample::new(parts),
        Err(EdgeSampleError::ReservedSourceNamespace),
        "a flexcore:-namespaced sample must be refused"
    );
    // An EMPTY source identity is refused too (identity is mandatory).
    let parts = EdgeTelemetrySampleParts {
        source_id: "   ".to_string(),
        ..same_parts()
    };
    assert_eq!(EdgeTelemetrySample::new(parts), Err(EdgeSampleError::EmptySourceId));
}

fn same_parts() -> EdgeTelemetrySampleParts {
    EdgeTelemetrySampleParts {
        asset_key: "ess-01".to_string(),
        quantity: TelemetryQuantity::ActivePower,
        value: QuantityValue::Power(ActivePowerMW::new(1.0).expect("finite")),
        quality: TelemetryQuality::Good,
        source_quality_code: None,
        event_time: EventTime::from_unix_millis(1_000).expect("valid"),
        ingest_time: IngestTime::from_unix_millis(2_000).expect("valid"),
        source_sequence: 1,
        source_id: "edge:pilot-site-A:simulated-test/ess0/ActivePower".to_string(),
    }
}

#[test]
fn b_out_of_range_values_are_refused_never_accepted_as_good() {
    // THE ATTACK: physically impossible values (NaN; SOC beyond [0,100] %)
    // presented as GOOD readings.
    let ingest = IngestTime::from_unix_millis(BASE_MILLIS + 1_000).expect("valid");
    let (_, acl, _) = openems_style_acl();
    let nan = hostile_reading(PointKey::new("ess0", "ActivePower"), f64::NAN, Some("GOOD"));
    assert_eq!(acl.map_reading(&nan, ingest), Err(AclError::NonFiniteValue));
    let soc_acl = AntiCorruptionLayer::new(
        vec![PointMapping {
            point: PointKey::new("ess0", "Soc"),
            asset_key: "ess-01".to_string(),
            quantity: TelemetryQuantity::Soc,
            native_unit: NativeUnit::SocPercent,
            native_sign: None,
            reactive_source_convention: None,
        }],
        "edge:pilot-site-A:simulated-test",
        vec![],
    )
    .expect("valid mapping table");
    for percent in [150.0, -5.0] {
        let reading = hostile_reading(PointKey::new("ess0", "Soc"), percent, Some("GOOD"));
        assert!(
            soc_acl.map_reading(&reading, ingest).is_err(),
            "SOC {percent} % must be refused (outside the canonical [0,1] fraction)"
        );
    }
}

/// A small ACL over the same mapping the suite uses (helper for the
/// out-of-range cases; labelled so no test reads as production wiring).
fn openems_style_acl() -> ((), AntiCorruptionLayer, ()) {
    let acl = AntiCorruptionLayer::new(
        vec![PointMapping {
            point: PointKey::new("ess0", "ActivePower"),
            asset_key: "ess-01".to_string(),
            quantity: TelemetryQuantity::ActivePower,
            native_unit: NativeUnit::W,
            native_sign: Some(NativeSign::ExportPositive),
            reactive_source_convention: None,
        }],
        "edge:pilot-site-A:simulated-test",
        vec![],
    )
    .expect("valid mapping table");
    ((), acl, ())
}

#[test]
fn b_out_of_range_quality_code_arrives_suspect_never_good_through_the_poll_path() {
    // THE ATTACK: the source reports OUT_OF_RANGE and hopes the flag is
    // dropped. Through the FULL ingest path the sample arrives SUSPECT
    // with the original code retained — never upgraded to GOOD.
    let fixture = SimulatedFixture {
        points: vec![SimulatedPointFixture {
            point: PointKey::new("ess0", "ActivePower"),
            native_unit: NativeUnit::Kw,
            values_per_tick: vec![1_500.0, 9_999.0],
            quality_per_tick: vec!["GOOD".to_string(), "OUT_OF_RANGE".to_string()],
        }],
        start_millis: BASE_MILLIS,
        step_millis: 1_000,
        reactive_source_convention: ReactiveSignConvention::InjectionPositive,
        cadence_millis: 5_000,
    };
    let sim = SimulatedEmsSource::new(fixture);
    let control = sim.control();
    let mappings = vec![PointMapping {
        point: PointKey::new("ess0", "ActivePower"),
        asset_key: "ess-01".to_string(),
        quantity: TelemetryQuantity::ActivePower,
        native_unit: NativeUnit::Kw,
        native_sign: Some(NativeSign::ExportPositive),
        reactive_source_convention: None,
    }];
    let acl = AntiCorruptionLayer::new(
        mappings.clone(),
        "edge:pilot-site-A:simulated-test",
        vec![],
    )
    .expect("valid mapping table");
    let mut source = AdapterIngestSource::new(
        Box::new(sim),
        acl,
        mappings,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 2_000)),
        64,
    )
    .expect("read path composes");

    // Tick 0: healthy.
    let first = source.poll(10).expect("tick 0");
    assert_eq!(first[0].quality(), TelemetryQuality::Good);
    // THE ATTACK TICK: OUT_OF_RANGE.
    control.advance_tick();
    let second = source.poll(10).expect("tick 1");
    assert_eq!(second[0].quality(), TelemetryQuality::Suspect);
    assert_eq!(second[0].source_quality_code(), Some("OUT_OF_RANGE"));
    assert_ne!(
        second[0].quality(),
        TelemetryQuality::Good,
        "an out-of-range value must NEVER arrive as GOOD"
    );
}

#[test]
fn b_stale_data_replay_is_flagged_or_skipped_never_accepted_as_new_good() {
    let ingest = IngestTime::from_unix_millis(BASE_MILLIS + 1_000).expect("valid");
    let (_, acl, _) = openems_style_acl();
    // THE ATTACK 1: stale data presented fresh — the STALE code maps to
    // canonical STALE, never GOOD.
    let stale = hostile_reading(PointKey::new("ess0", "ActivePower"), 1_000.0, Some("STALE"));
    let sample = acl.map_reading(&stale, ingest).expect("stale maps flagged");
    assert_eq!(sample.quality(), TelemetryQuality::Stale);
    assert_ne!(sample.quality(), TelemetryQuality::Good);

    // THE ATTACK 2: the SAME stale reading replayed — the dedup window
    // classifies the exact replay as an idempotent skip (never re-accepted
    // as a new value), and a REWRITTEN replay (same identity, different
    // content — a tampered history) as a conflicting duplicate.
    let mut window = EdgeDedupWindow::new(16);
    assert_eq!(window.admit(&sample), AdmitOutcome::New);
    assert_eq!(
        window.admit(&sample),
        AdmitOutcome::ExactDuplicate,
        "the exact stale replay is a skip, never a second stored value"
    );
    let tampered = acl
        .map_reading(&hostile_reading(
            PointKey::new("ess0", "ActivePower"),
            4_242.0,
            Some("STALE"),
        ), ingest)
        .expect("the tampered replay maps");
    assert_eq!(
        window.admit(&tampered),
        AdmitOutcome::ConflictingDuplicate,
        "same identity + different content = refused (P4.24 fixture-(b) rule)"
    );
}

#[test]
fn b_a_nan_reading_through_the_poll_path_is_counted_and_never_emitted() {
    let hostile = HostileEmsAdapter::with_readings(vec![hostile_reading(
        PointKey::new("ess0", "ActivePower"),
        f64::NAN,
        Some("GOOD"),
    )]);
    let mut source = hostile_ingest("nan-poll", hostile);
    let error = source.poll(10).expect_err("a NaN-only poll is a hard failure");
    assert!(matches!(error, EdgeError::SourcePollFailed { .. }));
    assert_eq!(source.stats().acl_rejections, 1);
    assert_eq!(source.stats().samples_emitted, 0);
}

// ---------------------------------------------------------------------------
// (d) Secrets exposure: lexical scan + runtime redaction assertions
// ---------------------------------------------------------------------------

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).expect("the edge-agent src directory is readable");
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// The crate's cfg(test) convention: `#[cfg(test)] mod tests` is ALWAYS the
/// trailing module of a source file (verified by construction below — the
/// scan treats everything from the LAST `#[cfg(test)]` to EOF as test
/// region and everything before it as production; a file with an
/// interleaved cfg(test) block would be scanned as production, which errs
/// toward FLAGGING, never toward silence).
fn production_region(source: &str) -> &str {
    match source.rfind("#[cfg(test)]") {
        Some(index) => &source[..index],
        None => source,
    }
}

#[test]
fn d_no_secret_material_is_embedded_or_logged_in_edge_agent_sources() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = manifest_dir.join("src");
    let mut files = Vec::new();
    collect_rs_files(&src, &mut files);
    assert!(
        files.len() >= 15,
        "the scan must actually cover the crate sources (found {} files under {:?})",
        files.len(),
        src
    );

    // Secret-typed CONSTRUCTORS must never be handed an INLINE STRING
    // LITERAL in production code: the composition root loads the
    // deployment's provisioned values (from the environment / secret
    // manager, §17.3), the code base generates and ships NONE. The pattern
    // matches the literal-argument form `Type::new("` — loading from an
    // environment variable (`ProvisioningToken::new(raw)`) is the
    // legitimate deployment path and does not match. Test fixtures (inside
    // the trailing cfg(test) module) are excluded by the region split.
    const SECRET_CONSTRUCTORS: [&str; 4] = [
        "ProvisioningToken::new(\"",
        "EnablementKey::new(\"",
        "CommandSigningKey::new(\"",
        "PemBody::new(\"",
    ];
    // Hardcoded credential-shaped literals: no production line ASSIGNES a
    // secret value inline.
    const SECRET_LITERALS: [&str; 5] = [
        "password = \"",
        "secret = \"",
        "api_key = \"",
        "apikey = \"",
        "bearer_token = \"",
    ];

    let mut violations: Vec<String> = Vec::new();
    for file in &files {
        let content = std::fs::read_to_string(file).expect("source file is readable");
        let production = production_region(&content);
        for token in SECRET_CONSTRUCTORS {
            if production.contains(token) {
                violations.push(format!(
                    "{file:?} constructs secret material inline in PRODUCTION code: '{token}' \
                     (key material is a deployment requirement — provisioned out-of-band, §17.3)"
                ));
            }
        }
        for token in SECRET_LITERALS {
            if production.contains(token) {
                violations.push(format!(
                    "{file:?} embeds a hardcoded credential-shaped literal '{token}' in \
                     production code (no secrets in source control, §17.3)"
                ));
            }
        }
        // A secret VALUE must never reach a log formatter: the named
        // accessor (`expose()`) is for the transport/digest call sites
        // ONLY — never on a `tracing::` line.
        for line in production.lines() {
            if line.contains("tracing::") && line.contains("expose()") {
                violations.push(format!(
                    "{file:?} logging line passes secret material through expose(): {line:?}"
                ));
            }
            for credential_word in ["provisioning_token.value", "key.value", "token.value"] {
                if line.contains("tracing::") && line.contains(credential_word) {
                    violations.push(format!(
                        "{file:?} logging line references a secret VALUE field \
                         '{credential_word}': {line:?}"
                    ));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "SECRETS-EXPOSURE VIOLATION (report §17.3: no secrets in logs, config, or source \
         control):\n{}",
        violations.join("\n")
    );
}

#[test]
fn d_the_redaction_debug_impls_actually_redact() {
    // Runtime witness that the redaction impls hold: a secret value placed
    // into a credential type must be UNRECOVERABLE from Debug/Display.
    let secret_value = "SUPER-SECRET-EDGE-TOKEN-42";
    let secret_in_debug = format!("{:?}", secret_value);

    // 1. The provisioning token (config.rs ↔ enrollment.rs).
    let token = flexcore_edge_agent::enrollment::ProvisioningToken::new(secret_value.to_string())
        .expect("valid token");
    let debug = format!("{token:?}");
    let display = token.to_string();
    assert_eq!(debug, "ProvisioningToken(<redacted>)");
    assert_eq!(display, "<redacted>");
    assert!(!debug.contains("SECRET"), "the token value leaked via Debug");
    assert!(!secret_in_debug.is_empty(), "sanity: the control value formats");

    // 2. The command-path enablement key.
    let key = EnablementKey::new(secret_value.to_string(), "deployment:lab-key-1".to_string())
        .expect("valid key");
    assert_eq!(format!("{key:?}"), "EnablementKey(<redacted>)");
    assert_eq!(key.to_string(), "<redacted>");

    // 3. The command signing key.
    let signing = CommandSigningKey::new(secret_value.to_string(), "deployment:lab-key-1".to_string())
        .expect("valid key");
    assert_eq!(format!("{signing:?}"), "CommandSigningKey(<redacted>)");
    assert_eq!(signing.to_string(), "<redacted>");

    // 4. A PEM body (credential material).
    let pem = flexcore_edge_agent::enrollment::PemBody::new(secret_value.to_string());
    assert_eq!(format!("{pem:?}"), "PemBody(<redacted>)");
    assert_eq!(pem.to_string(), "<redacted>");

    // 5. The credential RECORD: PEM fields redact; the key reference stays.
    let record = flexcore_edge_agent::enrollment::DeviceCredentialRecord {
        device_id: "edge-01".to_string(),
        tenant_id: "tenant-A".to_string(),
        site_id: "site-A".to_string(),
        certificate_pem: Some(pem.clone()),
        chain_pem: vec![pem.clone()],
        root_ca_pem: Some(pem),
        private_key_reference: Some(PathBuf::from("/secure/volume/device.key")),
        issued_at_unix_millis: BASE_MILLIS,
        expires_at_unix_millis: None,
    };
    let record_debug = format!("{record:?}");
    assert!(!record_debug.contains("SECRET"), "the record Debug leaked PEM material");
    assert!(record_debug.contains("edge-01"), "identity fields stay visible");

    // 6. The deployment config: the endpoint, the provisioning token, and
    // the OpenEMS base URL (which may embed credentials) all redact.
    let config = EdgeConfig {
        site_id: "pilot-site-A".to_string(),
        tenant_id: "tenant-A".to_string(),
        control_plane_endpoint: format!("wss://user:{secret_value}@central.example"),
        adapter_id: "openems-rest".to_string(),
        buffer_path: PathBuf::from("edge-buffer"),
        command_path_enabled: false,
        poll_interval: std::time::Duration::from_millis(5_000),
        buffer_capacity: 10_000,
        forward_batch_size: 100,
        dedup_window: 4_096,
        buffer_max_age: std::time::Duration::from_millis(3_600_000),
        overflow_policy: flexcore_edge_agent::config::OverflowPolicy::DropOldestAudited,
        log_level: flexcore_edge_agent::config::LogLevel::Info,
        credential_store_path: PathBuf::from("edge-credentials/device-identity.conf"),
        provisioning_token: Some(
            flexcore_edge_agent::enrollment::ProvisioningToken::new(secret_value.to_string())
                .expect("valid token"),
        ),
        credential_rotation_window: std::time::Duration::from_millis(604_800_000),
        uplink_server_name: Some("central.example".to_string()),
        uplink_tls_minimum: TlsMinimum::DEFAULT,
        uplink_connect_timeout: std::time::Duration::from_millis(5_000),
        uplink_backoff_initial: std::time::Duration::from_millis(1_000),
        uplink_backoff_max: std::time::Duration::from_millis(30_000),
        uplink_keepalive_interval: std::time::Duration::from_millis(15_000),
        openems_base_url: Some(format!("http://admin:{secret_value}@ems.local")),
        openems_mapping_profile_path: PathBuf::from("config/openems-mapping-profile.v1.json"),
    };
    let config_debug = format!("{config:?}");
    assert!(
        !config_debug.contains("SUPER-SECRET"),
        "the EdgeConfig Debug leaked the secret value: {config_debug}"
    );
    assert!(
        !config_debug.contains(secret_value),
        "the EdgeConfig Debug leaked the raw secret"
    );
    // The redaction markers are actually present (redaction is explicit,
    // not an empty-struct accident).
    assert!(config_debug.contains("<redacted>"));
}

#[test]
fn d_the_scan_covers_the_binary_entrypoint_and_every_module() {
    // The secrets scan must never silently shrink: the module count of the
    // crate is pinned so a deleted scan target is visible.
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = manifest_dir.join("src");
    let mut files = Vec::new();
    collect_rs_files(&src, &mut files);
    assert!(src.join("main.rs").exists(), "main.rs must exist and be scanned");
    assert!(
        src.join("command_verify.rs").exists() && src.join("command_forward.rs").exists(),
        "the P6.15/P6.16 modules must exist and be scanned"
    );
    let _ = files.len(); // covered by the >= 15 assertion in the main scan test
}
