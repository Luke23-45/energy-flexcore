// P7.36 — Replay, sequence, and expiry protection on the plugin path (test
// file written, NOT run — the loop bans hold; the human build/test pass
// executes these).
//
// Coverage (ledger P7.36 acceptance criteria; task steps):
//
// 1. Replay refused AFTER checkpoint restore (restart survival — the
//    `buffer_file.rs` framing discipline checkpoint).
// 2. Duplicate vs out-of-sequence are DISTINCT named refusals.
// 3. Expired request refused (and not-yet-valid refused); the injected
//    clock is the only time source.
// 4. Digest mismatch refused (tampered payload under a captured identity).
// 5. The proposal envelope carries NO dispatch-authority surface:
//    type-level assertions (exhaustive match over the closed
//    `PluginPathOutcome` variant set; the statement constants) and the
//    protected-contract refusal restated at the path AND pinned at the
//    resolver (ADR-010).
// 6. Deterministic ordering (digest equality across runs, sorted
//    checkpoint load, sorted refusal selection).
// 7. Checkpoint corruption is truncate-and-audit, never silent; a foreign
//    file is refused, never guessed.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use flexcore_kernel::lifecycle::FixedClock;
use flexcore_kernel::manifest::parse_manifest_value;
use flexcore_kernel::plugin_path_protection::{
    refuse_protected_contract_claim, sequence_channel, CheckpointError,
    FilePluginSequenceCheckpoint, PluginPathOutcome, PluginPathError, PluginPathVerifier,
    PluginProposalEnvelope, PluginRequestEnvelope, PluginResponseEnvelope,
    PROPOSAL_ENVELOPE_STATEMENT, PLUGIN_PATH_PROTECTION_STATEMENT,
};
use flexcore_kernel::resolve::{resolve, ExplicitSelections, ResolveError};
use flexcore_kernel::signing::DIGEST_ALGORITHM_FNV1A64_TEST;

const BASE_MS: u64 = 1_700_000_000_000;
const PLUGIN_ID: &str = "neryva.test-optimizer";
const PLUGIN_VERSION: &str = "0.1.0";

// --- Helpers -----------------------------------------------------------------

/// A unique scratch path per call (std-only; no tempfile dep — the
/// edge-agent test pattern).
fn scratch_path(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut path = std::env::temp_dir();
    path.push(format!(
        "flexcore-kernel-p736-{}-{}-{}",
        std::process::id(),
        label,
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_file(&path);
    path
}

/// A request envelope with a 60-second validity window starting at BASE_MS.
fn request(sequence: u64, payload: serde_json::Value) -> PluginRequestEnvelope {
    PluginRequestEnvelope::new(
        "corr-001",
        PLUGIN_ID,
        PLUGIN_VERSION,
        sequence,
        BASE_MS,
        BASE_MS + 60_000,
        payload,
    )
    .expect("the reference envelope is valid")
}

fn payload(sequence: u64) -> serde_json::Value {
    serde_json::json!({ "asset": "ess-01", "action": "set-power-mw", "value": 2.5, "seq": sequence })
}

/// A verifier over a fresh file checkpoint and a clock AT BASE_MS.
fn verifier_at(clock: &FixedClock, path: &std::path::Path) -> PluginPathVerifier<'_> {
    let checkpoint =
        Box::new(FilePluginSequenceCheckpoint::open(path).expect("the checkpoint opens"));
    PluginPathVerifier::new(clock, checkpoint).expect("the verifier loads its history")
}

// --- Envelope and digest -------------------------------------------------------

#[test]
fn the_envelope_digest_is_deterministic_and_binds_every_field() {
    let envelope = request(1, payload(1));
    // Determinism: equal inputs construct equal digests, across runs.
    let again = request(1, payload(1));
    assert_eq!(envelope.digest_hex, again.digest_hex);
    assert_eq!(envelope.digest_hex.len(), 16, "16 lowercase hex digits");
    // The digest is the recorded test-grade algorithm label (the FNV-1a 64
    // honesty: a production digest is a DEPLOYMENT REQUIREMENT).
    assert!(PLUGIN_PATH_PROTECTION_STATEMENT.contains(DIGEST_ALGORITHM_FNV1A64_TEST));
    assert!(PLUGIN_PATH_PROTECTION_STATEMENT.contains("DEPLOYMENT REQUIREMENT"));
    // The binding covers every field: a change in ANY field changes the
    // canonical string (and thus the recorded digest no longer verifies).
    // The explicit fn-pointer type lets the closures coerce to one type.
    let mutations: [fn(&mut PluginRequestEnvelope); 7] = [
        |e: &mut PluginRequestEnvelope| e.correlation_id = "corr-002".to_string(),
        |e: &mut PluginRequestEnvelope| e.plugin_id = "neryva.other".to_string(),
        |e: &mut PluginRequestEnvelope| e.plugin_version = "0.2.0".to_string(),
        |e: &mut PluginRequestEnvelope| e.sequence = 2,
        |e: &mut PluginRequestEnvelope| e.valid_from_ms += 1,
        |e: &mut PluginRequestEnvelope| e.valid_until_ms += 1,
        |e: &mut PluginRequestEnvelope| e.payload = payload(2),
    ];
    for mutate in mutations {
        let mut tampered = request(1, payload(1));
        mutate(&mut tampered);
        let error = tampered.verify_digest().expect_err("a mutated envelope mismatches");
        assert_eq!(error.name(), "REQUEST_DIGEST_MISMATCH");
    }
}

#[test]
fn a_tampered_payload_is_refused_before_any_window_or_sequence_check() {
    // The P6.15 evaluation order: envelope → digest → window → sequence.
    // A tampered payload is refused as DIGEST_MISMATCH even when the
    // request is ALSO expired and out of sequence — the first failing
    // check in the fixed order names the refusal.
    let mut tampered = request(1, payload(1));
    tampered.payload = serde_json::json!({ "value": 999.0 });
    let clock = FixedClock::new(BASE_MS + 120_000); // past the window too
    let path = scratch_path("order-digest-first");
    let mut verifier = verifier_at(&clock, &path);
    let error = verifier.verify_and_admit(&tampered).expect_err("digest first");
    assert_eq!(error.name(), "REQUEST_DIGEST_MISMATCH");
    // The window is UNCHANGED by the refusal.
    assert_eq!(verifier.last_accepted(&sequence_channel(PLUGIN_ID, "invocation")), None);
}

// --- Validity window (injected clock) --------------------------------------------

#[test]
fn an_expired_request_is_refused_and_never_executed() {
    let envelope = request(1, payload(1));
    // Exactly at valid_until: expired (the window is half-open).
    let clock = FixedClock::new(BASE_MS + 60_000);
    let path = scratch_path("expired-at-boundary");
    let mut verifier = verifier_at(&clock, &path);
    let error = verifier.verify_and_admit(&envelope).expect_err("expired at the boundary");
    assert_eq!(error.name(), "REQUEST_EXPIRED");
    // Well past the window.
    let clock = FixedClock::new(BASE_MS + 120_000);
    let path = scratch_path("expired-past");
    let mut verifier = verifier_at(&clock, &path);
    let error = verifier.verify_and_admit(&envelope).expect_err("expired");
    assert_eq!(error.name(), "REQUEST_EXPIRED");
    assert!(error.to_string().contains("refused, never executed"));
}

#[test]
fn a_not_yet_valid_request_is_refused() {
    let envelope = request(1, payload(1));
    let clock = FixedClock::new(BASE_MS - 1);
    let path = scratch_path("not-yet-valid");
    let mut verifier = verifier_at(&clock, &path);
    let error = verifier.verify_and_admit(&envelope).expect_err("before the window");
    assert_eq!(error.name(), "REQUEST_NOT_YET_VALID");
    // At exactly valid_from: valid (half-open window).
    let clock = FixedClock::new(BASE_MS);
    let path = scratch_path("valid-at-boundary");
    let mut verifier = verifier_at(&clock, &path);
    verifier.verify_and_admit(&envelope).expect("valid at the window open");
}

#[test]
fn an_invalid_envelope_is_refused_before_everything() {
    // Sequence 0, empty correlation id, inverted window — each is
    // REQUEST_ENVELOPE_INVALID (check 1).
    let cases = vec![
        PluginRequestEnvelope::new("corr-001", PLUGIN_ID, PLUGIN_VERSION, 0, BASE_MS, BASE_MS + 1_000, payload(1)),
        PluginRequestEnvelope::new("  ", PLUGIN_ID, PLUGIN_VERSION, 1, BASE_MS, BASE_MS + 1_000, payload(1)),
        PluginRequestEnvelope::new("corr-001", PLUGIN_ID, PLUGIN_VERSION, 1, BASE_MS + 2_000, BASE_MS + 1_000, payload(1)),
    ];
    for case in cases {
        let error = case.expect_err("invalid envelope");
        assert_eq!(error.name(), "REQUEST_ENVELOPE_INVALID");
    }
}

// --- Sequence: duplicate vs out-of-sequence, DISTINCT -----------------------------

#[test]
fn duplicate_and_out_of_sequence_are_distinct_named_refusals() {
    let clock = FixedClock::new(BASE_MS + 1_000);
    let path = scratch_path("dup-vs-oos");
    let mut verifier = verifier_at(&clock, &path);
    let channel = sequence_channel(PLUGIN_ID, "invocation");

    verifier.verify_and_admit(&request(1, payload(1))).expect("seq 1 admitted");
    verifier.verify_and_admit(&request(2, payload(2))).expect("seq 2 admitted");

    // Duplicate: the SAME sequence as the last accepted — a replay.
    let error = verifier
        .verify_and_admit(&request(2, payload(2)))
        .expect_err("sequence 2 replayed");
    assert_eq!(error.name(), "SEQUENCE_REPLAYED");
    // Even with a DIFFERENT payload under the old sequence: the sequence is
    // still a duplicate (the digest check passes — the envelope was
    // rebuilt — but the window refuses it).
    let error = verifier
        .verify_and_admit(&request(2, serde_json::json!({ "conflicting": true })))
        .expect_err("same sequence with a conflicting payload is still a duplicate");
    assert_eq!(error.name(), "SEQUENCE_REPLAYED");

    // Out of sequence: LOWER than the last accepted.
    let error = verifier
        .verify_and_admit(&request(1, payload(1)))
        .expect_err("sequence 1 is out of sequence now");
    assert_eq!(error.name(), "SEQUENCE_OUT_OF_ORDER");

    // The names are pairwise distinct.
    assert_ne!(PluginPathError::SequenceReplayed { channel: "c".to_string(), last_accepted: 1 }.name(),
        PluginPathError::SequenceOutOfOrder { channel: "c".to_string(), last_accepted: 1 }.name());

    // A refusal NEVER advances the window (fail-closed).
    assert_eq!(verifier.last_accepted(&channel), Some(2));
    // And a LATER sequence is still admitted.
    verifier.verify_and_admit(&request(3, payload(3))).expect("seq 3 admitted");
    assert_eq!(verifier.last_accepted(&channel), Some(3));
}

#[test]
fn a_refused_request_never_advances_the_window_in_any_refusal_class() {
    let clock = FixedClock::new(BASE_MS + 1_000);
    let path = scratch_path("no-advance");
    let mut verifier = verifier_at(&clock, &path);
    let channel = sequence_channel(PLUGIN_ID, "invocation");
    verifier.verify_and_admit(&request(5, payload(5))).expect("seq 5 admitted");

    // Expired (window closed): refused, window unchanged. The envelope is
    // CONSTRUCTED with the already-past window (mutating fields would trip
    // the digest check first — the fixed evaluation order).
    let expired = PluginRequestEnvelope::new(
        "corr-001",
        PLUGIN_ID,
        PLUGIN_VERSION,
        6,
        BASE_MS - 10,
        BASE_MS, // the clock reads BASE_MS + 1_000: past the window
        payload(6),
    )
    .expect("the expired envelope is self-valid");
    let error = verifier.verify_and_admit(&expired).expect_err("expired");
    assert_eq!(error.name(), "REQUEST_EXPIRED");
    assert_eq!(verifier.last_accepted(&channel), Some(5));

    // Out of sequence: refused, window unchanged.
    let error = verifier.verify_and_admit(&request(2, payload(2))).expect_err("out of sequence");
    assert_eq!(error.name(), "SEQUENCE_OUT_OF_ORDER");
    assert_eq!(verifier.last_accepted(&channel), Some(5));
}

// --- Replay refused after checkpoint restore (restart survival) --------------------

#[test]
fn replay_is_refused_after_checkpoint_restore() {
    let path = scratch_path("restart-survival");
    // Phase 1 — first process: admit sequences 1..=3.
    {
        let clock = FixedClock::new(BASE_MS + 1_000);
        let mut verifier = verifier_at(&clock, &path);
        for seq in 1..=3u64 {
            verifier.verify_and_admit(&request(seq, payload(seq))).expect("admitted");
        }
        drop(verifier);
        // The checkpoint holds the durable advance.
        let checkpoint = FilePluginSequenceCheckpoint::open(&path).expect("reopens");
        assert!(checkpoint.records_written() >= 3);
    }
    // Phase 2 — the process "restarts": a NEW verifier over the SAME
    // checkpoint file (and a fresh clock). The captured request 3 is
    // replayed — REFUSED, because the window survived the restart.
    {
        let clock = FixedClock::new(BASE_MS + 2_000);
        let mut verifier = verifier_at(&clock, &path);
        let error = verifier
            .verify_and_admit(&request(3, payload(3)))
            .expect_err("the replay is refused after the checkpoint restore");
        assert_eq!(error.name(), "SEQUENCE_REPLAYED");
        // And the sequence continues where it left off — 4 is admitted.
        verifier.verify_and_admit(&request(4, payload(4))).expect("seq 4 admitted");
    }
    // Phase 3 — a THIRD restart sees 4: the durable advance survived.
    let checkpoint = FilePluginSequenceCheckpoint::open(&path).expect("reopens again");
    let mut state = checkpoint.load_all().expect("loads");
    state.sort();
    assert_eq!(
        state,
        vec![(sequence_channel(PLUGIN_ID, "invocation"), 4u64)],
        "the last accepted sequence per channel, restart-surviving"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn checkpoint_state_is_loaded_sorted_and_a_foreign_file_is_refused() {
    // Sorted load (determinism): two plugins' channels come back in
    // channel order.
    let path = scratch_path("sorted-load");
    {
        let checkpoint = FilePluginSequenceCheckpoint::open(&path).expect("opens");
        checkpoint.store_last_accepted("zzz-plugin/invocation", 7).expect("stores");
        checkpoint.store_last_accepted("aaa-plugin/invocation", 3).expect("stores");
    }
    let checkpoint = FilePluginSequenceCheckpoint::open(&path).expect("reopens");
    let loaded = checkpoint.load_all().expect("loads");
    let channels: Vec<&str> = loaded.iter().map(|(c, _)| c.as_str()).collect();
    assert_eq!(channels, vec!["aaa-plugin/invocation", "zzz-plugin/invocation"]);

    // A foreign file (wrong magic) is refused, never guessed at.
    let foreign = scratch_path("foreign");
    std::fs::write(&foreign, b"not a flexcore plugin-path checkpoint").expect("writes");
    let error = FilePluginSequenceCheckpoint::open(&foreign)
        .expect_err("a foreign file is refused");
    assert!(
        matches!(error, CheckpointError::NotACheckpointFile { .. }),
        "a foreign file is CHECKPOINT_FILE_FOREIGN: {error:?}"
    );
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&foreign);
}

#[test]
fn a_corrupt_checkpoint_tail_is_truncated_and_counted_never_silent() {
    let path = scratch_path("corrupt-tail");
    {
        let checkpoint = FilePluginSequenceCheckpoint::open(&path).expect("opens");
        checkpoint.store_last_accepted(&sequence_channel(PLUGIN_ID, "invocation"), 9)
            .expect("stores");
    }
    // Append garbage (the simulated partial write / disk corruption).
    {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("opens for append");
        file.write_all(&[0x99, 0x00, 0x00, 0x00, 0xAB, 0xCD, 0x12, 0x34, 0xFF])
            .expect("appends garbage");
    }
    let checkpoint = FilePluginSequenceCheckpoint::open(&path).expect("reopens (truncated)");
    assert_eq!(
        checkpoint.corrupt_tail_truncated(),
        1,
        "the corruption is COUNTED, never silent"
    );
    // The good record survived.
    let loaded = checkpoint.load_all().expect("loads");
    assert_eq!(loaded, vec![(sequence_channel(PLUGIN_ID, "invocation"), 9u64)]);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_verifier_cannot_run_without_its_checkpoint_history() {
    // A broken checkpoint store (unavailable) refuses verification
    // fail-closed: no history, no admission.
    struct BrokenStore;
    impl flexcore_kernel::plugin_path_protection::SequenceCheckpointStore for BrokenStore {
        fn load_all(
            &self,
        ) -> Result<Vec<(String, u64)>, flexcore_kernel::plugin_path_protection::CheckpointError> {
            Err(flexcore_kernel::plugin_path_protection::CheckpointError::Io {
                reason: "the store is down".to_string(),
            })
        }
        fn store_last_accepted(
            &self,
            _channel: &str,
            _sequence: u64,
        ) -> Result<(), flexcore_kernel::plugin_path_protection::CheckpointError> {
            Err(flexcore_kernel::plugin_path_protection::CheckpointError::Io {
                reason: "the store is down".to_string(),
            })
        }
    }
    let clock = FixedClock::new(BASE_MS);
    let error = PluginPathVerifier::new(&clock, Box::new(BrokenStore))
        .expect_err("no history, no verification");
    assert_eq!(error.name(), "CHECKPOINT_UNAVAILABLE");
}

// --- ADR-010: the proposal envelope carries no dispatch-authority surface ---------

#[test]
fn the_path_outcome_has_no_dispatch_authorizing_variant() {
    // TYPE-LEVEL ASSERTION (exhaustive match, no wildcard arm): the closed
    // `PluginPathOutcome` variant set is exactly
    // { ProposalRecorded, ResultObserved } — adding a dispatch-authorizing
    // variant would fail THIS match to compile, so the type system makes a
    // dispatch-authorizing response structurally not constructible through
    // this module (ADR-010; report §13.4).
    let proposal =
        PluginProposalEnvelope::new("prop-1", PLUGIN_ID, PLUGIN_VERSION, payload(1), 1)
            .expect("the proposal envelope is valid");
    let outcome: PluginPathOutcome = PluginPathOutcome::ProposalRecorded(proposal);
    let description = match outcome {
        PluginPathOutcome::ProposalRecorded(proposal) => {
            // A proposal carries DATA only: id, producer identity, payload,
            // sequence. No authority field exists on the type.
            assert_eq!(proposal.proposal_id, "prop-1");
            assert_eq!(proposal.producer_plugin_id, PLUGIN_ID);
            assert_eq!(proposal.producer_plugin_version, PLUGIN_VERSION);
            assert_eq!(proposal.sequence, 1);
            format!("proposal {}", proposal.proposal_id)
        }
        PluginPathOutcome::ResultObserved { correlation_id } => {
            format!("observed {correlation_id}")
        }
    };
    assert_eq!(description, "proposal prop-1");

    // The restated statement is pinned (what IS and IS NOT claimed).
    assert!(PROPOSAL_ENVELOPE_STATEMENT.contains("no dispatch-authorizing variant"));
    assert!(PROPOSAL_ENVELOPE_STATEMENT.contains("no authority"));
}

#[test]
fn a_verified_response_is_recorded_as_a_proposal_or_an_observation() {
    let clock = FixedClock::new(BASE_MS + 1_000);
    let path = scratch_path("record-outcome");
    let mut verifier = verifier_at(&clock, &path);
    let request = request(1, payload(1));
    verifier.verify_and_admit(&request).expect("admitted");

    // Matching echo → recorded.
    let response = PluginResponseEnvelope {
        correlation_id: request.correlation_id.clone(),
        sequence: request.sequence,
        payload: payload(1),
    };
    let proposal =
        PluginProposalEnvelope::new("prop-9", PLUGIN_ID, PLUGIN_VERSION, payload(1), 1)
            .expect("valid");
    let outcome = verifier
        .record_outcome(&request, &response, Some(proposal))
        .expect("the echo matches");
    assert!(matches!(outcome, PluginPathOutcome::ProposalRecorded(_)));
    let outcome = verifier
        .record_outcome(&request, &response, None)
        .expect("the echo matches");
    assert!(matches!(outcome, PluginPathOutcome::ResultObserved { .. }));

    // A mismatched echo is REFUSED on the record path too.
    let wrong_id = PluginResponseEnvelope {
        correlation_id: "other-correlation".to_string(),
        sequence: request.sequence,
        payload: payload(1),
    };
    let error = verifier
        .record_outcome(&request, &wrong_id, None)
        .expect_err("a response that does not echo the correlation id is refused");
    assert_eq!(error.name(), "RESPONSE_CORRELATION_MISMATCH");
    let wrong_seq = PluginResponseEnvelope {
        correlation_id: request.correlation_id.clone(),
        sequence: request.sequence + 1,
        payload: payload(1),
    };
    let error = verifier
        .record_outcome(&request, &wrong_seq, None)
        .expect_err("a response that does not echo the sequence is refused");
    assert_eq!(error.name(), "RESPONSE_SEQUENCE_MISMATCH");
}

// --- The protected-contract guard: restated at the path AND pinned at the resolver --

fn manifest_with_provider(provider_of: &[&str]) -> serde_json::Value {
    serde_json::json!({
        "manifest-schema": "flexcore-plugin-manifest-v1",
        "plugin-id": "neryva.hostile-plugin",
        "name": "Hostile Plugin",
        "version": "0.1.0",
        "api-version": ">=0.5,<1.0",
        "class": "builtin-rust",
        "entry-point": { "registered-type": "neryva.builtin/hostile" },
        "provider-of": provider_of,
        "consumes": [],
        "permissions": [],
        "configuration-schema": {
            "type": "object",
            "additionalProperties": false,
            "properties": {}
        },
        "secrets": [],
        "resources": { "max-memory-mb": 64, "timeout-ms": 30000 },
        "metadata": {
            "license": "Apache-2.0",
            "description": "Hostile manifest for the P7.36 guard tests.",
            "source": "crates/flexcore-kernel/tests"
        }
    })
}

#[test]
fn a_protected_contract_claim_is_refused_at_the_path_and_at_the_resolver() {
    // (a) The restated path guard (a manifest handed to the path directly).
    let hostile = parse_manifest_value(&manifest_with_provider(&["kernel.dispatch-gateway"]))
        .expect("the manifest itself validates");
    let error = refuse_protected_contract_claim(&hostile).expect_err("protected claim");
    assert_eq!(error.name(), "PROTECTED_CONTRACT_CLAIM");
    assert!(error.to_string().contains("kernel.dispatch-gateway"));

    // (b) The AUTHORITATIVE guard: the P7.06 resolver refuses the same
    // claim with PROTECTED_KERNEL_REPLACE (ADR-005/ADR-010; the reserved-id
    // list is the single source of truth).
    let error = resolve(&[hostile], &ExplicitSelections::new())
        .expect_err("the resolver refuses first");
    assert!(matches!(error, ResolveError::ProtectedKernelReplace { .. }));
    let name = match &error {
        ResolveError::ProtectedKernelReplace { .. } => error.name(),
        other => panic!("expected ProtectedKernelReplace, found {other:?}"),
    };
    assert_eq!(name, "PROTECTED_KERNEL_REPLACE");

    // (c) A benign manifest passes the restated guard.
    let benign = parse_manifest_value(&manifest_with_provider(&["ess.simulation"]))
        .expect("validates");
    refuse_protected_contract_claim(&benign).expect("no protected claim");
}

// --- Determinism ------------------------------------------------------------------

#[test]
fn equal_inputs_produce_equal_decisions_across_runs() {
    // Two verifiers over SEPARATE fresh checkpoints, same envelopes, same
    // clock readings: identical admission/refusal behaviour (P7.42's
    // plugin-path leg).
    let clock = FixedClock::new(BASE_MS + 1_000);
    let path_a = scratch_path("determinism-a");
    let path_b = scratch_path("determinism-b");
    let mut a = verifier_at(&clock, &path_a);
    let mut b = verifier_at(&clock, &path_b);
    for seq in 1..=3u64 {
        let envelope = request(seq, payload(seq));
        let ra = a.verify_and_admit(&envelope);
        let rb = b.verify_and_admit(&envelope);
        assert_eq!(ra.is_ok(), rb.is_ok());
        ra.expect("admitted");
        rb.expect("admitted");
    }
    let refused_a = a.verify_and_admit(&request(3, payload(3)));
    let refused_b = b.verify_and_admit(&request(3, payload(3)));
    assert_eq!(
        refused_a.expect_err("replayed").name(),
        refused_b.expect_err("replayed").name()
    );
    let _ = std::fs::remove_file(&path_a);
    let _ = std::fs::remove_file(&path_b);
}

#[test]
fn the_named_error_table_is_pairwise_distinct() {
    let names = [
        PluginPathError::RequestEnvelopeInvalid { reason: "r".to_string() }.name(),
        PluginPathError::DigestMismatch { correlation_id: "c".to_string() }.name(),
        PluginPathError::RequestNotYetValid { correlation_id: "c".to_string(), valid_from_ms: 1 }.name(),
        PluginPathError::RequestExpired { correlation_id: "c".to_string(), valid_until_ms: 1 }.name(),
        PluginPathError::SequenceReplayed { channel: "c".to_string(), last_accepted: 1 }.name(),
        PluginPathError::SequenceOutOfOrder { channel: "c".to_string(), last_accepted: 1 }.name(),
        PluginPathError::CheckpointUnavailable { reason: "r".to_string() }.name(),
        PluginPathError::CheckpointFileForeign { reason: "r".to_string() }.name(),
        PluginPathError::ResponseCorrelationMismatch {
            expected: "e".to_string(),
            found: "f".to_string(),
        }
        .name(),
        PluginPathError::ResponseSequenceMismatch { expected: 1, found: 2 }.name(),
        PluginPathError::ProposalEnvelopeInvalid { reason: "r".to_string() }.name(),
        PluginPathError::ProtectedContractClaim {
            plugin_id: "p".to_string(),
            contract_id: "k".to_string(),
        }
        .name(),
    ];
    let mut sorted = names.to_vec();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), names.len(), "error names are pairwise distinct: {sorted:?}");
}
