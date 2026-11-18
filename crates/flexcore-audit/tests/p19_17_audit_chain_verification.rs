// P19.17 — the audit-chain VERIFICATION suite (integration level), written
// against the P11.15 chain design (`crates/flexcore-audit/src/chain.rs`,
// the P11-E-1 DEFAULT: keyed HMAC-SHA-256 linear chain, per-tenant
// namespace, periodic anchor export). Decision record:
// `docs/phases-11-20/phase-11-operational-robustness/records/
// 31_p11_15_audit_chain_upgrade.md`; the CR 6.1 anchor row lives in the
// P19.03 record §4 (`docs/phases-11-20/phase-19-security-certification-
// compliance/records/03_p19_03_iec_62443_4_2_component_mapping.md`).
//
// WRITTEN-UNRUN (P19.17; conventions §1 item 3): this file has NEVER been
// compiled or executed — execution is the human build/test gate. It
// consumes ONLY the real public API of `flexcore_audit::chain` and
// `flexcore_audit::record` (no nonexistent API is referenced; no new
// dependency edge is created). Key material below is TEST MATERIAL ONLY —
// never a deployment key; nothing here keys, deploys, or operates a
// production chain, and no certification, attestation, or tamper-resistance
// CLAIM is made: the chain under test is keyed tamper-EVIDENCE under
// reviewed key management (a DEPLOYMENT REQUIREMENT), never tamper-PROOF
// and never a release signature.
//
// What is under test, mapped to the P11.15 design:
// - The linear chain commits to content, order, and count: a content edit,
//   a deletion, an insertion, or a reorder is a named `ChainError`, never
//   a silent pass.
// - The per-tenant namespace: tenant chains are cryptographically
//   independent; a digest cannot be replayed across namespaces; short key
//   material fails closed.
// - The periodic anchor export: anchors chain across windows via the
//   continuation seed and a verifier can walk the anchors without the
//   records; every anchor-field mismatch has a named error variant.
// - The honesty wording: verdicts and anchor text carry the
//   "tamper-EVIDENCE / DEPLOYMENT REQUIREMENT / never a release signature"
//   posture verbatim.

use flexcore_audit::chain::{
    chain_step, export_tenant_anchor, genesis_head, hex_encode_digest, parse_digest_hex,
    verify_against_anchor, verify_tenant_chain, ChainError, ChainKey, TenantChainAnchor,
    TenantChainEntry, AUDIT_CHAIN_ALGORITHM, AUDIT_CHAIN_FORMAT_VERSION, CHAIN_KEY_MIN_BYTES,
};
use flexcore_audit::record::{
    ActorIdentity, AuditAction, AuditDecision, AuditObject, AuditRecord, AuditRecordFields,
    DecisionOutcome, EvidenceReference, RecordReference,
};
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{EventTime, IngestTime};
use flexcore_domain::ids::TenantId;

// --- Fixtures -----------------------------------------------------------------

fn sample_uuid(tag: u8) -> FlexUuid {
    let mut bytes = [0u8; 16];
    bytes[15] = tag;
    FlexUuid::from_bytes(bytes)
}

fn tenant(tag: u8) -> TenantId {
    TenantId::new(sample_uuid(tag))
}

fn sample_record(tag: u8, tenant_id: Option<TenantId>) -> AuditRecord {
    AuditRecord::new(AuditRecordFields {
        audit_id: sample_uuid(tag),
        tenant_id,
        actor: ActorIdentity::new("operator-1".to_string()).expect("valid actor"),
        action: AuditAction::new("asset.register".to_string()).expect("valid action"),
        object: AuditObject::new("asset".to_string(), format!("asset-{tag}")).expect("valid object"),
        before_ref: None,
        after_ref: Some(RecordReference::new(format!("asset-{tag}:v2")).expect("valid reference")),
        decision: None,
        evidence: None,
        event_time: EventTime::from_unix_millis(1_000).expect("valid event time"),
        ingest_time: IngestTime::from_unix_millis(2_000).expect("valid ingest time"),
        correlation_id: sample_uuid(3),
    })
    .expect("valid record")
}

/// The namespace key, derived from TEST material only (32 arbitrary bytes —
/// the minimum accepted length). Never a deployment key.
fn test_namespace_key() -> ChainKey {
    ChainKey::from_bytes(&[0x42u8; CHAIN_KEY_MIN_BYTES]).expect("valid test key")
}

/// Builds an honestly chained window: digests computed from genesis (or
/// from `seed`) over the records IN ORDER, with strictly increasing
/// sequence numbers starting at `first_sequence`.
fn chained_entries<'a>(
    namespace_key: &ChainKey,
    seed: Option<[u8; 32]>,
    first_sequence: i64,
    records: &'a [AuditRecord],
) -> Vec<TenantChainEntry<'a>> {
    let mut head = match seed {
        Some(s) => s,
        None => genesis_head(namespace_key),
    };
    let mut entries = Vec::with_capacity(records.len());
    for (i, record) in records.iter().enumerate() {
        let digest = chain_step(namespace_key, &head, record);
        head = digest;
        entries.push(TenantChainEntry {
            chain_sequence: first_sequence + i as i64,
            stored_digest: digest,
            record,
        });
    }
    entries
}

// --- The linear chain: commits to content, order, and count ----------------------

#[test]
fn a_tenant_chain_verifies_from_genesis_and_the_head_is_deterministic() {
    let tenant_a = tenant(2);
    let namespace_key = test_namespace_key().chain_key_for_namespace(Some(tenant_a));
    let records = vec![
        sample_record(1, Some(tenant_a)),
        sample_record(2, Some(tenant_a)),
        sample_record(3, Some(tenant_a)),
    ];
    let entries = chained_entries(&namespace_key, None, 1, &records);

    // The honest chain verifies and the walk is deterministic: two walks
    // return the same final head (the head commits to the whole ordered
    // history).
    let head_one =
        verify_tenant_chain(&namespace_key, &entries).expect("the honest chain verifies");
    let head_two = verify_tenant_chain(&namespace_key, &entries).expect("the walk is deterministic");
    assert_eq!(head_one, head_two);

    // The final head IS the last entry's stored digest by construction:
    // `verify_tenant_chain` returns the last stored digest as the head
    // (chain.rs: the walk sets `head = entry.stored_digest` at each step).
    // What makes it a commitment to the whole history is that each stored
    // digest chains the previous head — so the head differs from genesis
    // and from every EARLIER stored digest.
    assert_eq!(head_one, entries.last().expect("three entries").stored_digest);
    for entry in &entries[..entries.len() - 1] {
        assert_ne!(head_one, entry.stored_digest);
    }
    assert_ne!(head_one, genesis_head(&namespace_key));

    // The same records under the SAME key produce the same digests — the
    // construction is deterministic (a verifier recomputes, never guesses).
    let rebuilt = chained_entries(&namespace_key, None, 1, &records);
    for (built, rebuilt) in entries.iter().zip(rebuilt.iter()) {
        assert_eq!(built.stored_digest, rebuilt.stored_digest);
    }
}

#[test]
fn a_content_edit_is_a_named_digest_mismatch_at_the_edited_sequence() {
    let tenant_a = tenant(2);
    let namespace_key = test_namespace_key().chain_key_for_namespace(Some(tenant_a));
    let mut records = vec![
        sample_record(1, Some(tenant_a)),
        sample_record(2, Some(tenant_a)),
        sample_record(3, Some(tenant_a)),
    ];
    let entries = chained_entries(&namespace_key, None, 1, &records);
    verify_tenant_chain(&namespace_key, &entries).expect("the honest chain verifies");

    // The tamper check pairs the ORIGINAL stored digests with MUTATED
    // records: the entries borrow their records, so the records cannot be
    // mutated in place while the entries live. The tampered copies below
    // carry the same digests, so the walk sees exactly what the original
    // in-place edit would have shown.
    // Pairing the ORIGINAL stored digests with the tampered records,
    // built inline per check: a closure cannot express that the returned
    // entries borrow the tampered records (a closure's return lifetime is
    // independent of its parameter's).
    let mut tampered = records.clone();

    // An edit to the LAST record (actor changed) breaks the digest AT
    // sequence 3 — the head commits to every record, not just the early
    // ones. (Checked first so the walk's first-discrepancy rule is
    // exercised at both positions in one honest window.)
    tampered[2].actor = ActorIdentity::new("attacker".to_string()).expect("valid actor");
    let tampered_entries: Vec<TenantChainEntry<'_>> = entries
        .iter()
        .zip(tampered.iter())
        .map(|(entry, record)| TenantChainEntry {
            chain_sequence: entry.chain_sequence,
            stored_digest: entry.stored_digest,
            record,
        })
        .collect();
    match verify_tenant_chain(&namespace_key, &tampered_entries) {
        Err(ChainError::DigestMismatch { sequence }) => assert_eq!(sequence, 3),
        other => panic!("expected DigestMismatch at sequence 3, got: {other:?}"),
    }

    // An edit to record 2 (after_ref changed) breaks the digest AT sequence
    // 2 — the walk fails at the FIRST discrepancy (a verification never
    // reports "probably fine").
    tampered[1].after_ref =
        Some(RecordReference::new("asset-2:TAMPERED".to_string()).expect("valid reference"));
    let tampered_entries: Vec<TenantChainEntry<'_>> = entries
        .iter()
        .zip(tampered.iter())
        .map(|(entry, record)| TenantChainEntry {
            chain_sequence: entry.chain_sequence,
            stored_digest: entry.stored_digest,
            record,
        })
        .collect();
    match verify_tenant_chain(&namespace_key, &tampered_entries) {
        Err(ChainError::DigestMismatch { sequence }) => assert_eq!(sequence, 2),
        other => panic!("expected DigestMismatch at sequence 2, got: {other:?}"),
    }
}

#[test]
fn a_deletion_an_insertion_and_a_reorder_are_named_against_the_anchor() {
    let tenant_a = tenant(2);
    let namespace_key = test_namespace_key().chain_key_for_namespace(Some(tenant_a));
    let records = vec![
        sample_record(1, Some(tenant_a)),
        sample_record(2, Some(tenant_a)),
        sample_record(3, Some(tenant_a)),
    ];
    let entries = chained_entries(&namespace_key, None, 1, &records);
    let anchor =
        export_tenant_anchor(&namespace_key, Some(tenant_a), None, &entries, 1_700_000_000_000)
            .expect("a non-empty window exports");

    // DELETION: a two-record window is self-consistent as its own chain,
    // but the anchor commits to THREE records — the deleted record is a
    // count mismatch, never a silent pass.
    let deleted_window = chained_entries(&namespace_key, None, 1, &records[..2]);
    verify_tenant_chain(&namespace_key, &deleted_window)
        .expect("the shortened chain is self-consistent");
    assert!(matches!(
        verify_against_anchor(&namespace_key, Some(tenant_a), None, &deleted_window, &anchor),
        Err(ChainError::AnchorCountMismatch { expected: 3, computed: 2 })
    ));

    // INSERTION: the honest three digests relabeled [1, 2, 4] keep the
    // count at three but break the SEQUENCE RANGE — a forged tail is a
    // range mismatch (the count gate passes, the range gate names it).
    let inserted = vec![
        TenantChainEntry {
            chain_sequence: 1,
            stored_digest: entries[0].stored_digest,
            record: &records[0],
        },
        TenantChainEntry {
            chain_sequence: 2,
            stored_digest: entries[1].stored_digest,
            record: &records[1],
        },
        TenantChainEntry {
            chain_sequence: 4,
            stored_digest: entries[2].stored_digest,
            record: &records[2],
        },
    ];
    match verify_against_anchor(&namespace_key, Some(tenant_a), None, &inserted, &anchor) {
        Err(ChainError::AnchorSequenceRangeMismatch {
            expected_first: 1,
            expected_last: 3,
            computed_first: 1,
            computed_last: 4,
        }) => {}
        other => panic!("expected AnchorSequenceRangeMismatch for the padded window, got: {other:?}"),
    }

    // REORDER: honest-looking digests for a DIFFERENT order still fail —
    // the swapped middle entries carry each other's digests, so the walk
    // names the digest mismatch at the first swapped sequence.
    let swapped = vec![
        TenantChainEntry {
            chain_sequence: 1,
            stored_digest: entries[0].stored_digest,
            record: &records[0],
        },
        TenantChainEntry {
            chain_sequence: 2,
            stored_digest: entries[2].stored_digest,
            record: &records[2],
        },
        TenantChainEntry {
            chain_sequence: 3,
            stored_digest: entries[1].stored_digest,
            record: &records[1],
        },
    ];
    match verify_against_anchor(&namespace_key, Some(tenant_a), None, &swapped, &anchor) {
        Err(ChainError::DigestMismatch { sequence: 2 }) => {}
        other => panic!("expected DigestMismatch at sequence 2 for the reordered window, got: {other:?}"),
    }
}

#[test]
fn a_non_monotonic_sequence_is_itself_the_finding() {
    let tenant_a = tenant(2);
    let namespace_key = test_namespace_key().chain_key_for_namespace(Some(tenant_a));
    let records = vec![sample_record(1, Some(tenant_a)), sample_record(2, Some(tenant_a))];
    let entries = chained_entries(&namespace_key, None, 1, &records);

    // Sequences supplied out of order are refused BEFORE any digest is
    // trusted — the disorder is the finding (the walk refuses; it never
    // re-sorts on the caller's behalf).
    let disordered = vec![
        TenantChainEntry {
            chain_sequence: 2,
            stored_digest: entries[0].stored_digest,
            record: &records[0],
        },
        TenantChainEntry {
            chain_sequence: 1,
            stored_digest: entries[1].stored_digest,
            record: &records[1],
        },
    ];
    assert!(matches!(
        verify_tenant_chain(&namespace_key, &disordered),
        Err(ChainError::NonMonotonicSequence { previous: 2, found: 1 })
    ));

    // A repeated sequence is equally non-monotonic.
    let repeated = vec![
        TenantChainEntry {
            chain_sequence: 1,
            stored_digest: entries[0].stored_digest,
            record: &records[0],
        },
        TenantChainEntry {
            chain_sequence: 1,
            stored_digest: entries[1].stored_digest,
            record: &records[1],
        },
    ];
    assert!(matches!(
        verify_tenant_chain(&namespace_key, &repeated),
        Err(ChainError::NonMonotonicSequence { previous: 1, found: 1 })
    ));
}

// --- The per-tenant namespace ---------------------------------------------------

#[test]
fn key_material_fails_closed_at_the_minimum_length() {
    // 31 bytes: refused — a short key is a deployment error, never a
    // silent downgrade (RFC 2104 recommends a key at least as long as the
    // hash output; the type pins 32).
    match ChainKey::from_bytes(&[0x42u8; CHAIN_KEY_MIN_BYTES - 1]) {
        Err(ChainError::KeyTooShort {
            length: 31,
            required: 32,
        }) => {}
        Err(error) => panic!("expected KeyTooShort for 31 bytes, got error: {error:?}"),
        Ok(_) => panic!("expected KeyTooShort for 31 bytes, got an accepted key"),
    }
    // Exactly 32 bytes: accepted.
    ChainKey::from_bytes(&[0x42u8; CHAIN_KEY_MIN_BYTES]).expect("the minimum length is accepted");
    // Longer material is accepted too (derived via HMAC domain separation).
    ChainKey::from_bytes(&[0x42u8; 131]).expect("longer material is accepted");
}

#[test]
fn tenant_namespaces_are_cryptographically_independent() {
    let tenant_a = tenant(2);
    let tenant_b = tenant(9);
    let namespace_key = test_namespace_key();
    let key_a = namespace_key.chain_key_for_namespace(Some(tenant_a));
    let key_b = namespace_key.chain_key_for_namespace(Some(tenant_b));
    let key_platform = namespace_key.chain_key_for_namespace(None);

    // The SAME record digests differently in every namespace — a digest
    // cannot be replayed across tenants, and the platform namespace is its
    // own namespace (P1.12 §4.7: tenant_id NULL marks platform-level
    // actions).
    let record = sample_record(1, Some(tenant_a));
    let head_a = chain_step(&key_a, &genesis_head(&key_a), &record);
    let head_b = chain_step(&key_b, &genesis_head(&key_b), &record);
    let head_platform = chain_step(&key_platform, &genesis_head(&key_platform), &record);
    assert_ne!(head_a, head_b);
    assert_ne!(head_a, head_platform);
    assert_ne!(head_b, head_platform);

    // Per-tenant verification never needs another tenant's records: each
    // namespace's chain verifies in isolation...
    let records_a = vec![sample_record(1, Some(tenant_a)), sample_record(2, Some(tenant_a))];
    let entries_a = chained_entries(&key_a, None, 1, &records_a);
    verify_tenant_chain(&key_a, &entries_a).expect("tenant A's chain verifies under key A");

    // ...and a chain verified under the WRONG namespace fails with a named
    // digest mismatch (the rows do not silently re-key).
    assert!(matches!(
        verify_tenant_chain(&key_b, &entries_a),
        Err(ChainError::DigestMismatch { sequence: 1 })
    ));
    assert!(matches!(
        verify_tenant_chain(&key_platform, &entries_a),
        Err(ChainError::DigestMismatch { sequence: 1 })
    ));

    // The derivation is deterministic in the key material: the same
    // material derives the same namespace key; different material derives
    // an independent one.
    let same_material = ChainKey::from_bytes(&[0x42u8; CHAIN_KEY_MIN_BYTES])
        .expect("valid test key")
        .chain_key_for_namespace(Some(tenant_a));
    let other_material = ChainKey::from_bytes(&[0x43u8; CHAIN_KEY_MIN_BYTES])
        .expect("valid test key")
        .chain_key_for_namespace(Some(tenant_a));
    let derived_record = sample_record(7, Some(tenant_a));
    assert_eq!(
        chain_step(&key_a, &genesis_head(&key_a), &derived_record),
        chain_step(&same_material, &genesis_head(&same_material), &derived_record),
    );
    assert_ne!(
        chain_step(&key_a, &genesis_head(&key_a), &derived_record),
        chain_step(&other_material, &genesis_head(&other_material), &derived_record),
    );
}

// --- The periodic anchor export (the designed, human-cadenced half) --------------

#[test]
fn anchors_chain_across_windows_and_a_verifier_walks_them_without_the_records() {
    let tenant_a = tenant(2);
    let namespace_key = test_namespace_key().chain_key_for_namespace(Some(tenant_a));

    // Window 1: sequences 1–3, exported from genesis.
    let window_one_records = vec![
        sample_record(1, Some(tenant_a)),
        sample_record(2, Some(tenant_a)),
        sample_record(3, Some(tenant_a)),
    ];
    let window_one = chained_entries(&namespace_key, None, 1, &window_one_records);
    let anchor_one =
        export_tenant_anchor(&namespace_key, Some(tenant_a), None, &window_one, 1_700_000_000_000)
            .expect("a non-empty window exports");
    assert!(anchor_one.continues_from_head.is_none());
    assert_eq!(anchor_one.first_sequence, 1);
    assert_eq!(anchor_one.last_sequence, 3);
    assert_eq!(anchor_one.record_count, 3);
    verify_against_anchor(&namespace_key, Some(tenant_a), None, &window_one, &anchor_one)
        .expect("window one verifies against its anchor");

    // The exported head equals the walk's recomputed head — the anchor
    // commits to exactly what the chain computes.
    let walked_head = verify_tenant_chain(&namespace_key, &window_one).expect("walk verifies");
    assert_eq!(anchor_one.head_digest, hex_encode_digest(&walked_head));

    // Window 2: sequences 4–5, seeded from window one's head.
    let window_two_records = vec![sample_record(4, Some(tenant_a)), sample_record(5, Some(tenant_a))];
    let window_two = chained_entries(&namespace_key, Some(walked_head), 4, &window_two_records);
    let anchor_two = export_tenant_anchor(
        &namespace_key,
        Some(tenant_a),
        Some(walked_head),
        &window_two,
        1_700_000_001_000,
    )
    .expect("a non-empty continuation window exports");

    // THE SECOND-ORDER CHAIN: the anchors alone form a walkable chain —
    // anchor two names anchor one's head as its continuation seed, without
    // any record present.
    assert_eq!(
        anchor_two.continues_from_head.as_deref(),
        Some(anchor_one.head_digest.as_str())
    );
    // The seed text parses back to the walked head digest (the anchors
    // carry the canonical lower-case hex form).
    assert_eq!(
        parse_digest_hex(anchor_two.continues_from_head.as_deref().expect("seed present"))
            .expect("the continuation seed is canonical hex"),
        walked_head
    );
    verify_against_anchor(&namespace_key, Some(tenant_a), Some(walked_head), &window_two, &anchor_two)
        .expect("window two verifies against its anchor with the seed");

    // The continuation gate: the same records WITHOUT the seed are refused
    // — a window claims to continue a SPECIFIC history.
    assert!(matches!(
        verify_against_anchor(&namespace_key, Some(tenant_a), None, &window_two, &anchor_two),
        Err(ChainError::AnchorContinuationMismatch { .. })
    ));

    // The platform namespace exports its own anchors, independent of any
    // tenant's.
    let platform_key = namespace_key.chain_key_for_namespace(None);
    let platform_records = vec![sample_record(10, None)];
    let platform_window = chained_entries(&platform_key, None, 1, &platform_records);
    let platform_anchor = export_tenant_anchor(
        &platform_key,
        None,
        None,
        &platform_window,
        1_700_000_002_000,
    )
    .expect("the platform window exports");
    assert!(platform_anchor.tenant_id.is_none());
    assert_ne!(platform_anchor.head_digest, anchor_one.head_digest);
}

#[test]
fn every_anchor_field_mismatch_has_its_named_error() {
    let tenant_a = tenant(2);
    let namespace_key = test_namespace_key().chain_key_for_namespace(Some(tenant_a));
    let records = vec![sample_record(1, Some(tenant_a)), sample_record(2, Some(tenant_a))];
    let entries = chained_entries(&namespace_key, None, 1, &records);
    let anchor =
        export_tenant_anchor(&namespace_key, Some(tenant_a), None, &entries, 1_700_000_000_000)
            .expect("a non-empty window exports");

    // An EMPTY export window is refused outright (an empty anchor commits
    // to nothing and is not evidence).
    assert!(matches!(
        export_tenant_anchor(&namespace_key, Some(tenant_a), None, &[], 1_700_000_000_000),
        Err(ChainError::EmptyAnchorExport)
    ));
    // ...and an empty claimed window fails the anchor verification too.
    assert!(matches!(
        verify_against_anchor(&namespace_key, Some(tenant_a), None, &[], &anchor),
        Err(ChainError::AnchorCountMismatch { expected: 2, computed: 0 })
    ));

    // VERSION: an anchor claiming a different format version is refused,
    // never guessed at.
    let mut wrong_version = anchor.clone();
    wrong_version.chain_version = AUDIT_CHAIN_FORMAT_VERSION + 1;
    assert!(matches!(
        verify_against_anchor(&namespace_key, Some(tenant_a), None, &entries, &wrong_version),
        Err(ChainError::AnchorVersionMismatch { .. })
    ));

    // ALGORITHM: an anchor claiming a different algorithm is refused (the
    // test-grade FNV label is not an acceptable algorithm here).
    let mut wrong_algorithm = anchor.clone();
    wrong_algorithm.algorithm = "fnv1a64-test";
    assert!(matches!(
        verify_against_anchor(&namespace_key, Some(tenant_a), None, &entries, &wrong_algorithm),
        Err(ChainError::AnchorAlgorithmMismatch { .. })
    ));

    // TENANT: an anchor verified against the wrong namespace is refused.
    let tenant_b = tenant(9);
    assert!(matches!(
        verify_against_anchor(&namespace_key, Some(tenant_b), None, &entries, &anchor),
        Err(ChainError::AnchorTenantMismatch { .. })
    ));
    // ...including the tenant/platform confusion in both directions.
    assert!(matches!(
        verify_against_anchor(&namespace_key, None, None, &entries, &anchor),
        Err(ChainError::AnchorTenantMismatch { .. })
    ));

    // CONTINUATION: an anchor that names a seed the claimed window does not
    // use is refused (a window cannot silently detach from its history).
    let mut wrong_seed = anchor.clone();
    wrong_seed.continues_from_head = Some(hex_encode_digest(&[7u8; 32]));
    assert!(matches!(
        verify_against_anchor(&namespace_key, Some(tenant_a), None, &entries, &wrong_seed),
        Err(ChainError::AnchorContinuationMismatch { .. })
    ));

    // COUNT: a truncated claimed window is a count mismatch.
    assert!(matches!(
        verify_against_anchor(&namespace_key, Some(tenant_a), None, &entries[..1], &anchor),
        Err(ChainError::AnchorCountMismatch { expected: 2, computed: 1 })
    ));

    // SEQUENCE RANGE: the count gate passes (two entries) but the claimed
    // window skips a sequence — the range gate names the gap.
    let gapped = vec![
        TenantChainEntry {
            chain_sequence: 1,
            stored_digest: entries[0].stored_digest,
            record: &records[0],
        },
        TenantChainEntry {
            chain_sequence: 3,
            stored_digest: entries[1].stored_digest,
            record: &records[1],
        },
    ];
    match verify_against_anchor(&namespace_key, Some(tenant_a), None, &gapped, &anchor) {
        Err(ChainError::AnchorSequenceRangeMismatch {
            expected_first: 1,
            expected_last: 2,
            computed_first: 1,
            computed_last: 3,
        }) => {}
        other => panic!("expected AnchorSequenceRangeMismatch for the gapped window, got: {other:?}"),
    }

    // HEAD: a window whose digests verify but whose anchor head was
    // altered is a head mismatch.
    let mut wrong_head = anchor.clone();
    wrong_head.head_digest = hex_encode_digest(&[0xaau8; 32]);
    assert!(matches!(
        verify_against_anchor(&namespace_key, Some(tenant_a), None, &entries, &wrong_head),
        Err(ChainError::AnchorHeadMismatch { .. })
    ));
}

// --- Digest hex helpers -----------------------------------------------------------

#[test]
fn digest_hex_round_trips_and_fails_closed_on_malformed_input() {
    let zero = [0u8; 32];
    let encoded = hex_encode_digest(&zero);
    assert_eq!(encoded.len(), 64);
    assert!(encoded.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    assert_eq!(parse_digest_hex(&encoded).expect("canonical hex parses"), zero);

    // A non-zero digest round trips too.
    let mut digest = [0u8; 32];
    for (i, b) in digest.iter_mut().enumerate() {
        *b = (i as u8) * 8 + 1;
    }
    assert_eq!(
        parse_digest_hex(&hex_encode_digest(&digest)).expect("canonical hex parses"),
        digest
    );

    // Fail closed on any other shape — a malformed stored digest is a
    // finding, never a guess.
    assert!(matches!(
        parse_digest_hex(""),
        Err(ChainError::DigestHexMalformed { length: 0 })
    ));
    assert!(matches!(
        parse_digest_hex("abcd"),
        Err(ChainError::DigestHexMalformed { length: 4 })
    ));
    assert!(matches!(
        parse_digest_hex(&"z".repeat(64)),
        Err(ChainError::DigestHexMalformed { length: 64 })
    ));
    // 63 hex characters: malformed even though hex-clean.
    let short = &encoded[..63];
    assert!(matches!(
        parse_digest_hex(short),
        Err(ChainError::DigestHexMalformed { length: 63 })
    ));
}

// --- The honesty wording and the construction constants ----------------------------

#[test]
fn the_construction_constants_carry_the_p11_15_design() {
    // The algorithm label is VERSIONED (a construction change is a
    // detectable format change, never a silent one).
    assert_eq!(AUDIT_CHAIN_ALGORITHM, "hmac-sha256-chain-v1");
    assert_eq!(AUDIT_CHAIN_FORMAT_VERSION, 1);
    // The minimum key length is the RFC 2104 recommendation for SHA-256.
    assert_eq!(CHAIN_KEY_MIN_BYTES, 32);
}

#[test]
fn verdicts_and_anchor_text_carry_the_honest_posture_verbatim() {
    let tenant_a = tenant(2);
    let namespace_key = test_namespace_key().chain_key_for_namespace(Some(tenant_a));
    let records = vec![sample_record(1, Some(tenant_a)), sample_record(2, Some(tenant_a))];
    let entries = chained_entries(&namespace_key, None, 1, &records);
    let anchor =
        export_tenant_anchor(&namespace_key, Some(tenant_a), None, &entries, 1_700_000_000_000)
            .expect("a non-empty window exports");

    // The anchor text carries the DEPLOYMENT REQUIREMENT wording: keyed
    // tamper-EVIDENCE under reviewed key management, NEVER a release
    // signature (the P11.15 §2 gate; no certification claim anywhere).
    let text = anchor.audit_text();
    assert!(text.contains("DEPLOYMENT REQUIREMENT"), "anchor text: {text}");
    assert!(text.contains("never a release signature"), "anchor text: {text}");
    assert!(text.contains("tamper-EVIDENCE"), "anchor text: {text}");
    assert!(text.contains(AUDIT_CHAIN_ALGORITHM), "anchor text: {text}");
    // A tenant anchor names its tenant namespace, not the platform label.
    assert!(!text.contains("tenant=platform"), "anchor text: {text}");

    // A tamper verdict names the evidence boundary, not a strength claim.
    let mismatch = ChainError::DigestMismatch { sequence: 2 };
    let rendered = mismatch.to_string();
    assert!(rendered.contains("CHAIN DIGEST MISMATCH"), "verdict: {rendered}");
    assert!(rendered.contains("tamper-EVIDENCE, not tamper-PROOF"), "verdict: {rendered}");
    let head_mismatch = ChainError::AnchorHeadMismatch {
        expected: "aa".repeat(32),
        computed: "bb".repeat(32),
    };
    assert!(
        head_mismatch.to_string().contains("tamper-EVIDENCE, not tamper-PROOF"),
        "verdict: {head_mismatch}"
    );
}

// --- Chain inputs: the full §10.1 record shape -------------------------------------

#[test]
fn records_with_decisions_evidence_and_corrections_chain_and_verify() {
    let tenant_a = tenant(2);
    let namespace_key = test_namespace_key().chain_key_for_namespace(Some(tenant_a));

    // A record with an authorization decision (outcome + reason reference,
    // never a bare allow/deny)...
    let decided = {
        let mut record = sample_record(1, Some(tenant_a));
        record.decision = Some(
            AuditDecision::new(
                DecisionOutcome::Denied,
                "policy:phase4:operator:command.create:asset".to_string(),
            )
            .expect("valid decision"),
        );
        record
    };
    // ...a record with an evidence reference...
    let evidenced = {
        let mut record = sample_record(2, Some(tenant_a));
        record.evidence =
            Some(EvidenceReference::new("evidence-pack-1".to_string()).expect("valid evidence"));
        record
    };
    // ...and a CORRECTION record: a NEW record referencing the original
    // (the append-orientation rule — the original is never edited), whose
    // linkage folds into its digest.
    let correction = sample_record(3, Some(tenant_a))
        .link_correction(sample_uuid(1), "wrong after-ref recorded".to_string())
        .expect("valid correction");
    assert!(correction.corrects.is_some(), "the correction carries its linkage");

    let records = vec![decided, evidenced, correction];
    let entries = chained_entries(&namespace_key, None, 1, &records);
    verify_tenant_chain(&namespace_key, &entries).expect("the full-shape chain verifies");

    // Each full-shape record digests differently from its plain-shape
    // counterpart (every §10.1 field the record carries is committed to —
    // the chain cannot be narrowed by an unpersisted blind spot here).
    let plain = sample_record(1, Some(tenant_a));
    assert_ne!(
        chain_step(&namespace_key, &genesis_head(&namespace_key), &records[0]),
        chain_step(&namespace_key, &genesis_head(&namespace_key), &plain),
    );
    let plain_two = sample_record(2, Some(tenant_a));
    assert_ne!(
        chain_step(&namespace_key, &genesis_head(&namespace_key), &records[1]),
        chain_step(&namespace_key, &genesis_head(&namespace_key), &plain_two),
    );
}

#[test]
fn a_record_written_before_its_event_is_unconstructable_as_chain_input() {
    // The P0.09 ordering rule (ingest_time must not precede event_time)
    // holds on every chain input: the record constructor refuses, so no
    // chain digest can commit to an impossible ordering.
    let result = AuditRecord::new(AuditRecordFields {
        audit_id: sample_uuid(1),
        tenant_id: None,
        actor: ActorIdentity::new("operator-1".to_string()).expect("valid actor"),
        action: AuditAction::new("asset.register".to_string()).expect("valid action"),
        object: AuditObject::new("asset".to_string(), "asset-1".to_string()).expect("valid object"),
        before_ref: None,
        after_ref: None,
        decision: None,
        evidence: None,
        event_time: EventTime::from_unix_millis(2_000).expect("valid event time"),
        ingest_time: IngestTime::from_unix_millis(1_000).expect("valid ingest time"),
        correlation_id: sample_uuid(3),
    });
    assert!(result.is_err(), "ingest before event must be refused (P0.09)");
}

#[test]
fn an_empty_entry_set_verifies_as_the_absence_of_records_not_as_evidence() {
    // The design's own wording: an EMPTY set verifies trivially — a tenant
    // with no records has no chain to verify; this is the ABSENCE of
    // records, never evidence that anything held. The assertion pins that
    // documented behavior (and its honest reading).
    let namespace_key = test_namespace_key().chain_key_for_namespace(Some(tenant(2)));
    assert!(verify_tenant_chain(&namespace_key, &[]).is_ok());
    // But an anchor can never be exported over emptiness.
    assert!(matches!(
        export_tenant_anchor(&namespace_key, Some(tenant(2)), None, &[], 1_700_000_000_000),
        Err(ChainError::EmptyAnchorExport)
    ));
}
