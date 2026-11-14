// P7.33 — Audit-integrity proof: plugin audit records are tamper-EVIDENT
// (test file written, NOT run — the loop bans hold; the first human
// `cargo test -p flexcore-plugin-host` run is the verification gate).
//
// THE BOUNDARY STATEMENT (asserted as documentation by
// `the_boundary_statement_is_asserted_as_documentation` below; the
// authoritative copy is `flexcore_plugin_host::audit_chain::
// AUDIT_INTEGRITY_BOUNDARY` and the decision record
// `ledger/phase-07-plugin-sdk/33_34_audit_integrity_trust_boundary.md` §3):
//
// > Hash-chaining proves tamper-EVIDENCE, not tamper-PROOF: an attacker
// > who controls the host process can rebuild the whole chain.
// > Authoritative audit integrity lives in the platform's append-only
// > audit store (PostgreSQL `audit_records`, migration 0008: INSERT/
// > SELECT-only grants, the unconditional append-only trigger, and
// > tenant-isolation RLS; the SELECT-only read contract
// > `flexcore-storage/src/audit_read.rs`). The plugin-path hash chain is
// > the LOCAL evidence layer: it detects any mutation, insertion,
// > reordering, or deletion inside the host's own audit sequence against
// > the platform-retained anchor, but it does not survive an attacker who
// > controls the host AND the anchor. The kernel's in-module AuditSink
// > ports (lifecycle, signing) do not chain — recorded seam; this chain
// > covers host-issued records. TEST-GRADE digest: FNV-1a 64; production
// > hash = DEPLOYMENT REQUIREMENT.
//
// Coverage (ledger §P7.33 steps 1–2, the parts testable without the
// platform's durable store — the durable store sits behind the recorded
// P7.20 gRPC seam; the in-memory inner sink stands in for it):
//
// - the chain builds (every audited event lands in the chain and in the
//   inner sink; genesis prev_digest = 0; per-plugin and per-session
//   chains stay independent);
// - ANY single-record mutation is detected and the FIRST BROKEN LINK is
//   named (middle record, last record);
// - reordering is detected (records walked in stored order — the
//   verifier never re-sorts);
// - a middle deletion is detected (sequence gap named);
// - truncation is detected AGAINST THE ANCHOR (the pure chain honestly
//   cannot detect its own tail deletion — the anchor is the
//   platform-retained head; this is exactly the boundary statement);
// - the verifier passes on an honest chain (including against anchors);
// - audit-or-refuse is preserved: a failed inner write refuses and the
//   chain gains NOTHING;
// - host-level events (no plugin named) chain under the `host` key.
//
// What these tests deliberately do NOT cover: the ledger's host-kill
// survival drill (step 3) and the profile-switch audit-coverage refusal
// (step 4) — the survival drill needs the durable store over the
// transport seam, and the coverage refusal lives in the kernel's
// recomposition gate (kernel crate read-only this wave); both seams are
// recorded in the decision record §5, not silently skipped.

use std::sync::Arc;

use flexcore_plugin_host::audit_chain::{
    chain_key_for, first_broken_link, verify_with_anchors, AuditChainRecord, ChainedAuditSink,
    GENESIS_DIGEST, HOST_CHAIN_KEY,
};
use flexcore_plugin_host::broker::{
    BrokerAuditEvent, BrokerAuditKind, BrokerAuditSink, BrokerError, InMemoryBrokerAuditSink,
};

const SESSION: &str = "host-session-2026-11-13T00:00:00Z";
const PLUGIN_A: &str = "neryva.plugin-a";
const PLUGIN_B: &str = "neryva.plugin-b";

fn event(
    now: u64,
    kind: BrokerAuditKind,
    plugin: &str,
    capability: &str,
    detail: &str,
) -> BrokerAuditEvent {
    BrokerAuditEvent {
        recorded_at_unix_ms: now,
        kind,
        plugin_id: plugin.to_string(),
        capability: capability.to_string(),
        detail: detail.to_string(),
    }
}

/// A shared handle to the in-memory durable-side sink (`InMemoryBrokerAuditSink`
/// is not `Clone`; the shared wrapper lets the test inspect the inner events
/// while the chained writer holds the boxed port).
#[derive(Clone)]
struct SharedInner(Arc<InMemoryBrokerAuditSink>);

impl BrokerAuditSink for SharedInner {
    fn record(&self, event: BrokerAuditEvent) -> Result<(), BrokerError> {
        self.0.record(event)
    }
}

/// Records a fixed, realistic sequence of plugin-path audit events
/// (grants, uses, denials, a validation forward, an operator revocation
/// with no plugin named — the broker's closed audit kinds). Insertion
/// order: A1, A2, B1, A3, B2, H1, B3 (A = plugin A's chain, B = plugin B's,
/// H = the host chain for the operator revocation that names no plugin).
fn record_sample_sequence(sink: &ChainedAuditSink) {
    let sample: [BrokerAuditEvent; 7] = [
        event(1_000, BrokerAuditKind::GrantIssued, PLUGIN_A, "telemetry.read",
              "manifest neryva.plugin-a:0.1.0 scope data:tenant=t-1"),
        event(1_100, BrokerAuditKind::CapabilityUse, PLUGIN_A, "telemetry.read",
              "grant grant/neryva.plugin-a/telemetry.read/1 scope data:tenant=t-1"),
        event(1_200, BrokerAuditKind::GrantIssued, PLUGIN_B, "schedule.propose",
              "manifest neryva.plugin-b:0.2.0 scope send-only:scheduler-inbox"),
        event(1_300, BrokerAuditKind::CapabilityDenied, PLUGIN_A, "dispatch.propose",
              "denied (CAPABILITY_NOT_GRANTED): plugin 'neryva.plugin-a' holds no grant \
               for 'dispatch.propose' (defaults deny)"),
        event(1_400, BrokerAuditKind::ValidationForwardRequested, PLUGIN_B,
              "dispatch.validate.request",
              "forward fwd-1 dispatched to 'protected-validator' under grant \
               grant/neryva.plugin-b/dispatch.validate.request/2"),
        event(1_500, BrokerAuditKind::GrantRevoked, "",
              "telemetry.read",
              "grant grant/neryva.plugin-a/telemetry.read/1 revoked (effective \
               immediately at the next call)"),
        event(1_600, BrokerAuditKind::CapabilityUse, PLUGIN_B, "schedule.propose",
              "grant grant/neryva.plugin-b/schedule.propose/2 scope \
               send-only:scheduler-inbox"),
    ];
    for sample_event in &sample {
        sink.record(sample_event.clone())
            .expect("the sample sequence must record cleanly");
    }
}

fn composed_sink() -> (ChainedAuditSink, Arc<InMemoryBrokerAuditSink>) {
    let inner = Arc::new(InMemoryBrokerAuditSink::new());
    let sink = ChainedAuditSink::new(SESSION, Box::new(SharedInner(Arc::clone(&inner))))
        .expect("a non-empty session id composes");
    (sink, inner)
}

// --- 1. The chain builds; the verifier passes on an honest chain -------------------

#[test]
fn the_chain_builds_and_an_honest_chain_verifies() {
    let (sink, inner) = composed_sink();
    record_sample_sequence(&sink);

    // Every event reached the durable-side sink (nothing swallowed).
    assert_eq!(inner.events().len(), 7, "the inner sink holds every event");
    // Every event is chained.
    let records = sink.chain_records();
    assert_eq!(records.len(), 7, "the chain holds every event");

    // Genesis: the first record of each chain chains from GENESIS_DIGEST.
    for record in &records {
        if record.sequence == 1 {
            assert_eq!(
                record.prev_digest, GENESIS_DIGEST,
                "sequence 1 of every chain chains from the genesis digest"
            );
        }
    }

    // The honest chain walks clean...
    assert!(
        first_broken_link(&records).is_none(),
        "an honest chain has no broken link"
    );
    // ...and matches its own anchors (the platform-retained heads).
    let anchors = sink.anchors();
    assert!(!anchors.is_empty(), "anchors exist for every chain");
    let verification = verify_with_anchors(&records, &anchors);
    assert!(
        verification.is_intact(),
        "an honest chain is intact against its anchors (broken: {:?}, truncations: {:?})",
        verification.broken,
        verification.truncations
    );
}

// --- 2. Chains are per plugin and per host session ---------------------------------

#[test]
fn chains_are_per_plugin_and_per_session_with_independent_sequences() {
    let (sink, _inner) = composed_sink();
    record_sample_sequence(&sink);
    let records = sink.chain_records();

    // PLUGIN_A: events 1, 2, 4 -> sequences 1..3 under its own chain key.
    // PLUGIN_B: events 3, 5, 7 -> sequences 1..3 under its own chain key.
    // The operator revocation (no plugin named) chains under `host`.
    let mut sequences: std::collections::BTreeMap<String, Vec<u64>> =
        std::collections::BTreeMap::new();
    for record in &records {
        sequences
            .entry(record.chain_key.clone())
            .or_default()
            .push(record.sequence);
    }
    assert_eq!(
        sequences.get(PLUGIN_A).map(Vec::as_slice),
        Some(&[1, 2, 3][..]),
        "plugin A has its own 1-based chain"
    );
    assert_eq!(
        sequences.get(PLUGIN_B).map(Vec::as_slice),
        Some(&[1, 2, 3][..]),
        "plugin B has its own 1-based chain, independent of A"
    );
    assert_eq!(
        sequences.get(HOST_CHAIN_KEY).map(Vec::as_slice),
        Some(&[1][..]),
        "host-level events (no plugin named) chain under the host key"
    );
    // The resolver maps empty plugin ids to the host key (the same rule
    // the writer applies).
    assert_eq!(chain_key_for(""), HOST_CHAIN_KEY);
    assert_eq!(chain_key_for(PLUGIN_A), PLUGIN_A);
    // Sessions never alias: every record carries THIS session id.
    assert!(
        records.iter().all(|record| record.session_id == SESSION),
        "every record belongs to this host session"
    );
}

// --- 3. Any single-record mutation is detected; the first broken link is named -----

#[test]
fn a_single_record_mutation_is_detected_and_the_first_broken_link_is_named() {
    let (sink, _inner) = composed_sink();
    record_sample_sequence(&sink);
    let mut records = sink.chain_records();

    // Mutate a MIDDLE record's detail (sequence 2 of plugin A's chain —
    // the denial an attacker would want to disappear).
    let mutated = records
        .iter_mut()
        .find(|record| record.chain_key == PLUGIN_A && record.sequence == 2)
        .expect("plugin A sequence 2 exists");
    mutated.detail = "denied (nothing happened, all quiet)".to_string();

    let broken = first_broken_link(&records).expect("the mutation must be detected");
    assert_eq!(broken.session_id, SESSION);
    assert_eq!(broken.chain_key, PLUGIN_A);
    assert_eq!(
        broken.sequence, 2,
        "the FIRST broken link is the mutated record"
    );
    assert!(
        broken.reason.contains("mutated"),
        "the break names the mutation: {}",
        broken.reason
    );
}

#[test]
fn a_last_record_mutation_is_detected_by_the_digest_self_check() {
    let (sink, _inner) = composed_sink();
    record_sample_sequence(&sink);
    let mut records = sink.chain_records();

    // Mutate the LAST record of plugin B's chain: no successor exists to
    // catch a prev-digest mismatch, so the digest SELF-CHECK must catch it.
    let mutated = records
        .iter_mut()
        .find(|record| record.chain_key == PLUGIN_B && record.sequence == 3)
        .expect("plugin B sequence 3 exists");
    mutated.kind_name = "PLUGIN_CAPABILITY_DENIED".to_string();

    let broken = first_broken_link(&records).expect("the mutation must be detected");
    assert_eq!(broken.chain_key, PLUGIN_B);
    assert_eq!(broken.sequence, 3);
    assert!(broken.reason.contains("mutated"), "{}", broken.reason);
}

// --- 4. Reordering is detected ------------------------------------------------------

#[test]
fn reordering_is_detected() {
    let (sink, _inner) = composed_sink();
    record_sample_sequence(&sink);
    let mut records = sink.chain_records();

    // Swap two adjacent records of plugin A's chain (sequences 1 and 2) in
    // the evidence store. The verifier walks records in STORED order and
    // never re-sorts, so the swapped pair breaks the chain at the first
    // position where the sequence order regresses (the chain's first
    // encountered record now carries sequence 2 where 1 is expected).
    let pos_a = records
        .iter()
        .position(|record| record.chain_key == PLUGIN_A && record.sequence == 1)
        .expect("plugin A sequence 1 exists");
    let pos_b = records
        .iter()
        .position(|record| record.chain_key == PLUGIN_A && record.sequence == 2)
        .expect("plugin A sequence 2 exists");
    records.swap(pos_a, pos_b);

    let broken = first_broken_link(&records).expect("the reordering must be detected");
    assert_eq!(broken.chain_key, PLUGIN_A);
    assert_eq!(
        broken.sequence, 1,
        "the first broken link is the position where the order regresses"
    );
    assert!(
        broken.reason.contains("discontinuity"),
        "the break names the discontinuity: {}",
        broken.reason
    );
    assert!(
        broken.reason.contains("reordered"),
        "the break names reordering as a cause: {}",
        broken.reason
    );
}

// --- 5. A middle deletion is detected ------------------------------------------------

#[test]
fn a_middle_record_deletion_is_detected_as_a_sequence_gap() {
    let (sink, _inner) = composed_sink();
    record_sample_sequence(&sink);
    let mut records = sink.chain_records();

    // Delete plugin B's middle record (sequence 2 — the validation
    // forward an attacker wants to erase).
    records.retain(|record| !(record.chain_key == PLUGIN_B && record.sequence == 2));

    let broken = first_broken_link(&records).expect("the deletion must be detected");
    assert_eq!(broken.chain_key, PLUGIN_B);
    assert_eq!(
        broken.sequence, 2,
        "the break names the EXPECTED sequence where the gap opens"
    );
    assert!(
        broken.reason.contains("deleted"),
        "the break names the gap: {}",
        broken.reason
    );
}

// --- 6. Truncation is detected against the anchor ------------------------------------

#[test]
fn truncation_is_detected_against_the_anchor() {
    let (sink, _inner) = composed_sink();
    record_sample_sequence(&sink);

    // The platform retains the heads BEFORE the tampering.
    let anchors = sink.anchors();
    let mut truncated = sink.chain_records();
    let honest_len = truncated.len();

    // Truncate: drop the last three records of the stored sequence
    // (B3, H1, B2 — the tails of the plugin-B and host chains).
    truncated.truncate(honest_len - 3);

    // HONEST LIMIT (the boundary statement): a pure hash chain cannot
    // detect the deletion of its own tail — the shortened chain is
    // internally consistent. The verifier says so, in exactly this case:
    assert!(
        first_broken_link(&truncated).is_none(),
        "a pure chain does not detect its own tail deletion (the boundary \
         statement's tamper-evident-not-tamper-proof line)"
    );

    // Against the ANCHOR (the platform-retained heads), the truncation is
    // detected and names the chains that lost records.
    let verification = verify_with_anchors(&truncated, &anchors);
    assert!(!verification.is_intact(), "truncation is not intact");
    assert!(
        verification.broken.is_none(),
        "the truncation is an ANCHOR finding, not a link break"
    );
    assert_eq!(
        verification.truncations.len(),
        2,
        "both affected chains (plugin B and the host chain) are named"
    );
    for truncation in &verification.truncations {
        assert!(
            truncation.reason.contains("truncation"),
            "the finding names the truncation: {}",
            truncation.reason
        );
        assert!(
            truncation.actual_last_sequence < truncation.expected_last_sequence,
            "the chain ends before its anchored sequence"
        );
    }
}

// --- 7. Audit-or-refuse is preserved --------------------------------------------------

/// An inner sink that always fails (the broken durable side).
struct FailingAuditSink;

impl BrokerAuditSink for FailingAuditSink {
    fn record(&self, _event: BrokerAuditEvent) -> Result<(), BrokerError> {
        Err(BrokerError::AuditUnavailable {
            reason: "the durable audit store is unreachable (simulated)".to_string(),
        })
    }
}

#[test]
fn a_failed_inner_write_refuses_and_the_chain_gains_nothing() {
    let sink = ChainedAuditSink::new(SESSION, Box::new(FailingAuditSink))
        .expect("a non-empty session id composes");

    let result = sink.record(event(
        2_000,
        BrokerAuditKind::GrantIssued,
        PLUGIN_A,
        "telemetry.read",
        "manifest neryva.plugin-a:0.1.0 scope data:tenant=t-1",
    ));

    // Audit-or-refuse: the failure surfaces (the broker upstream refuses
    // the very action whose audit failed — broker.rs discipline).
    let error = result.expect_err("a failed inner write must refuse");
    assert_eq!(error.name(), "AUDIT_UNAVAILABLE");
    // The LOCAL chain gains NOTHING: the evidence layer never leads the
    // durable store.
    assert!(
        sink.chain_records().is_empty(),
        "no record may be chained when the durable write failed"
    );
    assert!(
        sink.anchors().is_empty(),
        "no anchor may advance when the durable write failed"
    );
}

// --- 8. An empty session id is refused at composition ---------------------------------

#[test]
fn an_empty_session_id_refuses_composition() {
    let inner = InMemoryBrokerAuditSink::new();
    let error = ChainedAuditSink::new("  ", Box::new(inner)).expect_err(
        "a blank session id must refuse (chains never alias across sessions)",
    );
    assert_eq!(error.name(), "AUDIT_UNAVAILABLE");
}

// --- 9. The boundary statement is asserted as documentation ---------------------------

/// THE BOUNDARY, carried by this test file per the task: hash-chaining
/// proves tamper-EVIDENCE, not tamper-PROOF. The authoritative audit
/// integrity of the platform lives in the append-only audit store
/// (migration 0008 + `audit_read.rs`); the plugin-path chain is the LOCAL
/// evidence layer; the kernel's in-module AuditSinks (lifecycle, signing)
/// do not chain — recorded seam; the digest is TEST-GRADE FNV-1a 64 and
/// the production hash is a DEPLOYMENT REQUIREMENT. The assertions below
/// pin the shipped `AUDIT_INTEGRITY_BOUNDARY` constant so the statement
/// cannot silently drift from the tests that rely on it.
#[test]
fn the_boundary_statement_is_asserted_as_documentation() {
    let boundary = flexcore_plugin_host::audit_chain::AUDIT_INTEGRITY_BOUNDARY;
    for required_phrase in [
        "tamper-EVIDENCE, not tamper-PROOF",
        "controls the host process can rebuild the whole chain",
        "append-only audit store",
        "migration 0008",
        "audit_read.rs",
        "LOCAL evidence layer",
        "lifecycle, signing",
        "recorded seam",
        "TEST-GRADE",
        "FNV-1a 64",
        "DEPLOYMENT REQUIREMENT",
    ] {
        assert!(
            boundary.contains(required_phrase),
            "the boundary statement must state '{required_phrase}' — got: {boundary}"
        );
    }
}

// --- 10. The canonical encoding is deterministic and mutation-sensitive ----------------

#[test]
fn the_canonical_encoding_is_deterministic_and_mutation_sensitive() {
    let base = AuditChainRecord::from_event(
        SESSION,
        PLUGIN_A,
        1,
        GENESIS_DIGEST,
        &event(
            1_000,
            BrokerAuditKind::GrantIssued,
            PLUGIN_A,
            "telemetry.read",
            "manifest neryva.plugin-a:0.1.0 scope data:tenant=t-1",
        ),
    );
    // Deterministic: the same record always encodes (and digests) the same.
    assert_eq!(base.canonical_encoding(), base.canonical_encoding());
    assert_eq!(base.recomputed_digest(), base.digest);
    // Mutation-sensitive: changing ANY field changes the digest (here: the
    // detail — the encoding is length-prefixed, so a change that does not
    // shift field boundaries still changes the bytes).
    let mut tampered = base.clone();
    tampered.detail = "manifest neryva.plugin-a:0.1.0 scope data:tenant=t-9".to_string();
    assert_ne!(
        tampered.recomputed_digest(),
        base.recomputed_digest(),
        "a one-character payload mutation must change the digest"
    );
    // The version tag is part of the encoding (a digest-function swap is a
    // deliberate, detectable format change).
    assert!(
        base.canonical_encoding()
            .starts_with("flexcore-audit-chain-v1"),
        "the canonical encoding carries the format version tag"
    );
}
