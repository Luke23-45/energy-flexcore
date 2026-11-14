// P9.32 — Integration tests: audit integrity and the edge audit-buffer
//         proof (ledger P9.32; report §17.3, §18.1, §16.3).
//
// EVERY TEST BELOW IS UNRUN (the loop bans in
// `ledger/phase-09-pilot-qualification/ENTRY_NOTE.md` boundary 5 forbid
// building and running tests; the human build/test pass owns execution).
// Nothing here claims a pass — these are the tamper and outage cases the
// ledger names, WRITTEN so a human run produces the tamper-suite and
// outage-test evidence.
//
// THE PROOF DISCIPLINE: the durable audit buffer under test is the EXISTING
// P6.18 `FileAuditBuffer` (`flexcore_edge_agent::audit_buffer`) — CRC-32
// length-prefixed framing, disk-first appends, truncate-and-audit recovery,
// audited drop-of-oldest overflow, in-order at-least-once upload with the
// fail-closed out-of-order commit refusal. The tests PROVE IT BY EXERCISE;
// nothing is re-implemented. The P9.32 additive surface
// (`flexcore_edge_agent::audit_proof`) computes the hash chain and the
// platform-retained `ChainAnchor` over the drained records.
//
// HONEST BOUNDS (the §13.3 discipline, recorded in
// `32_audit_integrity_proof.md`): a test suite is a finite test set — zero
// observed tampering-missed is a TEST OUTCOME, not a guarantee. The chain
// digest is FNV-1a 64 (no secret in the chain): the anchor is tamper-
// EVIDENCE retained by the platform at export time, NOT tamper-PROOF against
// a determined adversary with sustained write access to the edge host. The
// tests below assert exactly the detection semantics that hold and nothing
// stronger.
//
// The matrix:
// - the chain commits to content, order, count, and sequence range; every
//   tamper class (edit, delete-middle, reorder, tail truncation, duplicate,
//   prepend) produces a NAMED error, never a vacuous pass;
// - empty and disordered record sets are refused outright (an empty export
//   is not evidence);
// - the platform-retained anchor survives independently of the claimed
//   copy: a re-exported, tampered copy fails against the retained anchor;
// - the export bundle verifies end-to-end and produces the authority-review
//   evidence record with the honest tamper-EVIDENCE wording;
// - the outage proof (ledger step 3): audit events recorded while the
//   central platform is down survive an edge crash, upload exactly once
//   after commit (no loss, no duplication), in persisted-sequence order;
//   a crash in the drain→upload→commit window re-delivers (at-least-once);
//   an out-of-order commit is refused and removes nothing;
// - the CRC-32 framing detects a flipped payload byte and truncates with a
//   COUNTED loss (the records before the corruption are intact);
// - the audited drop-of-oldest overflow policy is re-proven in the pilot
//   matrix (counter + latched flag + reconciliation).

use std::path::Path;
use std::sync::Arc;

use flexcore_ports::clock::Clock;
use flexcore_ports::fakes::FixedClock;

use flexcore_edge_agent::audit_buffer::{BufferedAuditKind, BufferedAuditRecord, FileAuditBuffer};
use flexcore_edge_agent::audit_proof::{
    compute_chain_anchor, recompute_chain_head, verify_against_anchor, verify_bundle,
    verify_bundle_to_evidence, AuditChainError, AuditExportBundle, ChainAnchor,
    AUDIT_CHAIN_ALGORITHM, AUDIT_CHAIN_VERSION, AUDIT_EXPORT_FORMAT,
};
use flexcore_edge_agent::audit_buffer::AuditUploadError;

const BASE_MILLIS: i64 = 1_700_000_000_000;
const SITE: &str = "pilot-site-A";

// --- Shared fixtures ----------------------------------------------------------------

/// A std-only unique scratch directory per call (the P6.18 test pattern).
fn scratch_dir(label: &str) -> std::path::PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "flexcore-edge-p932-it-{}-{}-{}",
        std::process::id(),
        label,
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir creates");
    dir
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

fn clock() -> Arc<dyn Clock + Send + Sync> {
    Arc::new(FixedClock::at_unix_millis(BASE_MILLIS))
}

/// Records one audit event with explicit actor/object (the P6.21 shape).
fn record_event(
    buffer: &FileAuditBuffer,
    sequence_hint: usize,
    detail: &str,
) -> Result<(), flexcore_edge_agent::ports::EdgeError> {
    buffer.record_event(
        BufferedAuditKind::CommandRejected,
        "edge:pilot-site-A",
        &format!("command-cmd-{sequence_hint}"),
        detail.to_string(),
    )
}

/// A standalone record set for the chain tests (persisted sequences 1..=n).
fn chain_record(sequence: u64, detail: &str) -> BufferedAuditRecord {
    BufferedAuditRecord {
        sequence,
        recorded_at: flexcore_contracts::time_model::IngestTime::from_unix_millis(
            BASE_MILLIS + sequence as i64,
        )
        .expect("valid instant"),
        kind: BufferedAuditKind::CommandRejected,
        actor: "edge:pilot-site-A".to_string(),
        object: format!("command-cmd-{sequence}"),
        detail: detail.to_string(),
    }
}

/// Flips one byte inside the payload of the Nth journal frame (0-based,
/// after the 28-byte header: the 26-byte label + version byte per the P6.18
/// format). This simulates a bit-flip ON DISK — not an in-memory tamper.
fn flip_byte_in_frame_n(path: &Path, n: usize) {
    let mut bytes = std::fs::read(path).expect("reads the journal");
    // The P6.18 header: b"FLEXCORE-EDGE-AUDIT-JOURNAL" (26) + version (1).
    let header_len = b"FLEXCORE-EDGE-AUDIT-JOURNAL".len() + 1;
    let mut offset = header_len;
    let mut frame_index = 0usize;
    while offset + 8 <= bytes.len() {
        let len = u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("4 bytes"))
            as usize;
        if frame_index == n {
            let payload_start = offset + 8;
            assert!(
                payload_start < bytes.len(),
                "frame {n} payload exists"
            );
            bytes[payload_start] ^= 0x01;
            std::fs::write(path, &bytes).expect("writes the corrupted journal");
            return;
        }
        frame_index += 1;
        offset += 8 + len;
    }
    panic!("frame {n} not found in {}", path.display());
}

// --- 1. The chain commits to content, order, count, and range ------------------------

#[test]
fn every_tamper_class_produces_a_named_error_never_a_vacuous_pass() {
    let records = vec![
        chain_record(1, "EDGE_COMMAND_REJECTED: forged signature refused"),
        chain_record(2, "EDGE_COMMAND_REJECTED: replay refused"),
        chain_record(3, "EDGE_COMMAND_REJECTED: expired command refused"),
    ];
    let anchor = compute_chain_anchor(SITE, &records, BASE_MILLIS + 500).expect("anchor computes");
    assert_eq!(anchor.chain_version, AUDIT_CHAIN_VERSION);
    assert_eq!(anchor.algorithm, AUDIT_CHAIN_ALGORITHM);
    verify_against_anchor(SITE, &records, &anchor).expect("the honest copy verifies");

    // TAMPER CLASS 1 — CONTENT EDIT: one detail byte changes → the head no
    // longer matches (count and range still match, so the finding is the
    // specific HEAD MISMATCH).
    let mut edited = records.clone();
    edited[1].detail = "EDGE_COMMAND_REJECTED: replay (EDITED BY AN ATTACKER)".to_string();
    assert!(matches!(
        verify_against_anchor(SITE, &edited, &anchor),
        Err(AuditChainError::HeadMismatch { .. })
    ));

    // TAMPER CLASS 2 — DELETE A MIDDLE RECORD: the count is the first named
    // finding (2 vs 3).
    let deleted = vec![records[0].clone(), records[2].clone()];
    assert!(matches!(
        verify_against_anchor(SITE, &deleted, &anchor),
        Err(AuditChainError::CountMismatch { expected: 3, computed: 2 })
    ));

    // TAMPER CLASS 3 — REORDER: a swapped pair is no longer in strictly
    // increasing persisted-sequence order — refused as DISORDERED outright
    // (the disorder itself is the finding).
    let reordered = vec![records[0].clone(), records[2].clone(), records[1].clone()];
    assert!(matches!(
        verify_against_anchor(SITE, &reordered, &anchor),
        Err(AuditChainError::NonMonotonicSequence { .. })
    ));

    // TAMPER CLASS 4 — TAIL TRUNCATION: the last record is dropped → count
    // mismatch (the deletion an outage must never silently cause).
    let truncated = vec![records[0].clone(), records[1].clone()];
    assert!(matches!(
        verify_against_anchor(SITE, &truncated, &anchor),
        Err(AuditChainError::CountMismatch { expected: 3, computed: 2 })
    ));

    // TAMPER CLASS 5 — DUPLICATED RECORD: the count is the named finding.
    let duplicated = vec![
        records[0].clone(),
        records[1].clone(),
        records[2].clone(),
        records[2].clone(),
    ];
    assert!(matches!(
        verify_against_anchor(SITE, &duplicated, &anchor),
        Err(AuditChainError::CountMismatch { expected: 3, computed: 4 })
    ));

    // TAMPER CLASS 6 — PADDED/PREPENDED SET (same count, different range):
    // the sequence range is the named finding.
    let padded = vec![chain_record(1, "a"), chain_record(2, "b")];
    let anchor_for_5_6 = compute_chain_anchor(
        SITE,
        &[chain_record(5, "x"), chain_record(6, "y")],
        BASE_MILLIS + 500,
    )
    .expect("anchor computes");
    assert!(matches!(
        verify_against_anchor(SITE, &padded, &anchor_for_5_6),
        Err(AuditChainError::SequenceRangeMismatch { .. })
    ));

    // AN EMPTY CLAIMED COPY is never a vacuous pass: it is a count mismatch
    // against what the anchor committed to.
    assert!(matches!(
        verify_against_anchor(SITE, &[], &anchor),
        Err(AuditChainError::CountMismatch { .. })
    ));
}

#[test]
fn empty_and_anchorless_inputs_are_refused_not_vacuously_anchored() {
    // An EMPTY export is not evidence: no anchor is computed for it.
    assert!(matches!(
        compute_chain_anchor(SITE, &[], BASE_MILLIS),
        Err(AuditChainError::EmptyRecords)
    ));
    // A NON-ORDERED set cannot be anchored either.
    let disordered = vec![chain_record(2, "b"), chain_record(1, "a")];
    assert!(matches!(
        compute_chain_anchor(SITE, &disordered, BASE_MILLIS),
        Err(AuditChainError::NonMonotonicSequence { .. })
    ));
    // The chain head is deterministic over the same ordered set.
    let records = vec![chain_record(1, "a"), chain_record(2, "b")];
    assert_eq!(
        recompute_chain_head(&records).expect("computes"),
        recompute_chain_head(&records).expect("computes")
    );
}

// --- 2. The platform-retained anchor vs a tampered re-export --------------------------

#[test]
fn the_retained_anchor_detects_a_tampered_re_export_independently() {
    let dir = scratch_dir("retained-anchor");
    let journal = dir.join("audit.journal");

    // The edge records 3 rejections while the platform is reachable; the
    // export drains them and the PLATFORM RETAINS the anchor (a variable
    // here — the point is that the anchor's integrity does not depend on
    // the edge host's continued honesty).
    let buffer = FileAuditBuffer::open(&journal, 100, clock()).expect("opens");
    for n in 1..=3usize {
        record_event(&buffer, n, &format!("rejection {n}")).expect("records");
    }
    let drained = buffer.drain(100);
    assert_eq!(drained.len(), 3);
    let retained_anchor: ChainAnchor =
        compute_chain_anchor(SITE, &drained, BASE_MILLIS + 500).expect("anchor computes");
    buffer
        .commit_uploaded(&drained)
        .expect("the platform confirms the upload");
    drop(buffer);

    // LATER, FROM A CLAIMED COPY: an honest re-export verifies against the
    // retained anchor.
    let reopened = FileAuditBuffer::open(&journal, 100, clock()).expect("reopens");
    assert_eq!(reopened.pending_len(), 0, "committed records do not redeliver");
    verify_against_anchor(SITE, &drained, &retained_anchor)
        .expect("the honest claimed copy verifies");

    // A TAMPERED claimed copy (one detail edited) FAILS against the
    // retained anchor — the attacker controls the copy but not the anchor.
    let mut tampered = drained.clone();
    tampered[0].detail = "rejection 1 (altered)".to_string();
    let error = verify_against_anchor(SITE, &tampered, &retained_anchor)
        .expect_err("a tampered copy is detected");
    assert!(
        matches!(error, AuditChainError::HeadMismatch { .. }),
        "the named finding is the head mismatch: {error}"
    );
    cleanup(&dir);
}

// --- 3. The authority-review export bundle (ledger step 4) ------------------------------

#[test]
fn the_export_bundle_verifies_and_carries_the_honest_evidence_wording() {
    let records = vec![
        chain_record(1, "a"),
        chain_record(2, "b"),
        chain_record(3, "c"),
    ];
    let bundle = AuditExportBundle::new(SITE, records, BASE_MILLIS + 500).expect("bundle builds");
    assert_eq!(bundle.format, AUDIT_EXPORT_FORMAT);
    assert_eq!(bundle.anchor.site_id, SITE);

    // The verdict evidence is produced ONLY for a verified bundle.
    let evidence = verify_bundle_to_evidence(&bundle).expect("verifies");
    assert!(evidence.verdict_text.contains("tamper-EVIDENCE"));
    assert!(evidence.verdict_text.contains("not tamper-PROOF"));
    assert!(evidence.verdict_text.contains("retained by the platform"));
    // The anchor text an authority reads carries the same honest bound.
    assert!(bundle.anchor.audit_text().contains("tamper-EVIDENCE"));

    // A bundle that does not carry the versioned format id is REFUSED.
    let mut wrong_format = bundle.clone();
    wrong_format.format = "some-other-format";
    assert!(matches!(
        verify_bundle(&wrong_format),
        Err(AuditChainError::BundleFormatMismatch { .. })
    ));

    // A bundle whose site binding differs from the anchor's is REFUSED.
    let mut wrong_site = bundle.clone();
    wrong_site.site_id = "some-other-site".to_string();
    assert!(matches!(
        verify_bundle(&wrong_site),
        Err(AuditChainError::AnchorSiteMismatch { .. })
    ));

    // A tampered bundle fails end-to-end.
    let mut tampered = bundle.clone();
    tampered.records[2].detail = "c (edited)".to_string();
    assert!(matches!(
        verify_bundle(&tampered),
        Err(AuditChainError::HeadMismatch { .. })
    ));
}

// --- 4. The outage proof: zero loss, zero duplication, in order (ledger step 3) --------

#[test]
fn buffered_audit_events_survive_an_outage_and_upload_exactly_once() {
    let dir = scratch_dir("outage");
    let journal = dir.join("audit.journal");

    // PHASE 1 — THE CENTRAL PLATFORM IS OFFLINE: audit events occur at the
    // edge and are appended to the durable buffer (disk-first: a crash
    // cannot lose an appended record).
    const OUTAGE_EVENTS: usize = 7;
    {
        let buffer = FileAuditBuffer::open(&journal, 100, clock()).expect("opens");
        for n in 1..=OUTAGE_EVENTS {
            record_event(&buffer, n, &format!("outage event {n}")).expect("records");
        }
        assert_eq!(buffer.pending_len(), OUTAGE_EVENTS);
        // The edge CRASHES mid-outage (no drain, no commit, no flush beyond
        // the per-record syncs the buffer already performed).
    }

    // PHASE 2 — THE EDGE RESTARTS, STILL OFFLINE: every buffered event is
    // pending, IN PERSISTED-SEQUENCE ORDER (the order survives restarts).
    {
        let buffer = FileAuditBuffer::open(&journal, 100, clock()).expect("reopens");
        assert_eq!(buffer.pending_len(), OUTAGE_EVENTS, "zero audit-event loss");
        assert_eq!(buffer.corrupt_tail_truncated(), 0, "no corruption");
        let drained = buffer.drain(100);
        let sequences: Vec<u64> = drained.iter().map(|r| r.sequence).collect();
        let expected: Vec<u64> = (1..=OUTAGE_EVENTS as u64).collect();
        assert_eq!(sequences, expected, "in order, exactly the recorded events");

        // PHASE 3 — CONNECTIVITY RESTORES: the upload happens, the platform
        // confirms, the commit compacts the journal. EXACTLY ONCE: the
        // committed records do not redeliver after the next restart.
        let anchor = compute_chain_anchor(SITE, &drained, BASE_MILLIS + 900).expect("anchor");
        verify_against_anchor(SITE, &drained, &anchor).expect("the upload set is self-consistent");
        let committed = buffer.commit_uploaded(&drained).expect("commits");
        assert_eq!(committed, OUTAGE_EVENTS);
        assert_eq!(buffer.pending_len(), 0, "nothing left pending after commit");
        drop(buffer);
    }

    // PHASE 4 — A FINAL RESTART: zero pending, zero redelivery — NO LOSS and
    // NO DUPLICATION across the whole outage.
    {
        let buffer = FileAuditBuffer::open(&journal, 100, clock()).expect("reopens");
        assert_eq!(
            buffer.pending_len(),
            0,
            "no duplication after commit (ledger P9.32 acceptance (c))"
        );
    }
    cleanup(&dir);
}

#[test]
fn a_crash_in_the_upload_window_redelivers_at_least_once_in_order() {
    let dir = scratch_dir("upload-window-crash");
    let journal = dir.join("audit.journal");

    // Events recorded; the upload driver DRAINS them and CRASHES before the
    // platform confirms (no commit_uploaded).
    {
        let buffer = FileAuditBuffer::open(&journal, 100, clock()).expect("opens");
        for n in 1..=4usize {
            record_event(&buffer, n, &format!("event {n}")).expect("records");
        }
        let drained = buffer.drain(4);
        assert_eq!(drained.len(), 4);
        // CRASH: `drained` was never committed. drop(buffer) without commit.
    }

    // The restart re-delivers ALL FOUR records, in order, with their
    // persisted sequences — at-least-once (§15.4); the platform-side
    // idempotency (P4.24 discipline, cited) consumes the duplicates.
    {
        let buffer = FileAuditBuffer::open(&journal, 100, clock()).expect("reopens");
        assert_eq!(buffer.pending_len(), 4, "the uncommitted records were not lost");
        let drained = buffer.drain(100);
        assert_eq!(
            drained.iter().map(|r| r.sequence).collect::<Vec<_>>(),
            vec![1, 2, 3, 4],
            "in order, with the persisted sequences"
        );
        // The upload now succeeds and commits: exactly-once from here on.
        buffer.commit_uploaded(&drained).expect("commits");
        assert_eq!(buffer.pending_len(), 0);
    }
    cleanup(&dir);
}

#[test]
fn an_out_of_order_commit_is_refused_and_removes_nothing_in_the_pilot_matrix() {
    let dir = scratch_dir("out-of-order-commit");
    let journal = dir.join("audit.journal");
    let buffer = FileAuditBuffer::open(&journal, 100, clock()).expect("opens");
    for n in 1..=3usize {
        record_event(&buffer, n, &format!("event {n}")).expect("records");
    }
    // A malicious/buggy uploader presents a batch that does NOT start at the
    // journal's front (skipping an un-uploaded predecessor).
    let bogus = vec![BufferedAuditRecord {
        sequence: 2,
        recorded_at: flexcore_contracts::time_model::IngestTime::from_unix_millis(BASE_MILLIS)
            .expect("valid"),
        kind: BufferedAuditKind::CommandRejected,
        actor: "edge:pilot-site-A".to_string(),
        object: "command-cmd-2".to_string(),
        detail: "skipping seq 1".to_string(),
    }];
    let error = buffer
        .commit_uploaded(&bogus)
        .expect_err("the out-of-order commit is refused");
    assert!(matches!(
        error,
        AuditUploadError::OutOfOrderCommit { .. }
    ));
    // NOTHING was removed: all three records are still pending, in order.
    assert_eq!(buffer.pending_len(), 3);
    let pending = buffer.drain(10);
    assert_eq!(
        pending.iter().map(|r| r.sequence).collect::<Vec<_>>(),
        vec![1, 2, 3],
        "an un-uploaded predecessor is never removed out of order"
    );
    cleanup(&dir);
}

// --- 5. The CRC-32 framing: a flipped bit on disk is detected and counted ----------------

#[test]
fn a_flipped_payload_byte_is_detected_and_truncated_with_a_counted_loss() {
    let dir = scratch_dir("crc-framing");
    let journal = dir.join("audit.journal");
    {
        let buffer = FileAuditBuffer::open(&journal, 100, clock()).expect("opens");
        for n in 1..=3usize {
            record_event(&buffer, n, &format!("event {n}")).expect("records");
        }
    }
    // Flip one byte INSIDE the third frame's payload (bit corruption on the
    // disk, not an in-memory tamper).
    flip_byte_in_frame_n(&journal, 2);

    // The reopen: the corrupt tail is TRUNCATED and COUNTED, never silent;
    // the records BEFORE the corruption are intact and in order.
    let reopened = FileAuditBuffer::open(&journal, 100, clock()).expect("reopens");
    assert_eq!(
        reopened.corrupt_tail_truncated(),
        1,
        "the loss is counted (the truncate-and-audit discipline)"
    );
    let pending = reopened.drain(10);
    assert_eq!(
        pending.iter().map(|r| r.sequence).collect::<Vec<_>>(),
        vec![1, 2],
        "the records before the corruption survive intact"
    );
    assert_eq!(pending[0].detail, "event 1");
    assert_eq!(pending[1].detail, "event 2");
    cleanup(&dir);
}

// --- 6. The chain over a REAL buffer drain + the audited overflow policy ----------------

#[test]
fn the_chain_anchors_a_real_buffer_drain_and_survives_nothing_unsaid() {
    let dir = scratch_dir("chain-over-drain");
    let journal = dir.join("audit.journal");
    let buffer = FileAuditBuffer::open(&journal, 100, clock()).expect("opens");
    for n in 1..=5usize {
        record_event(&buffer, n, &format!("event {n}")).expect("records");
    }
    let drained = buffer.drain(100);
    let anchor = compute_chain_anchor(SITE, &drained, BASE_MILLIS + 700).expect("anchor");
    assert_eq!(anchor.record_count, 5);
    assert_eq!(anchor.first_sequence, 1);
    assert_eq!(anchor.last_sequence, 5);
    verify_against_anchor(SITE, &drained, &anchor).expect("the drain verifies");
    cleanup(&dir);
}

#[test]
fn the_overflow_policy_drops_oldest_with_a_counted_audited_loss() {
    let dir = scratch_dir("overflow");
    let journal = dir.join("audit.journal");
    let buffer = FileAuditBuffer::open(&journal, 2, clock()).expect("opens");
    for n in 1..=3usize {
        record_event(&buffer, n, &format!("event {n}")).expect("records");
    }
    assert_eq!(buffer.pending_len(), 2);
    assert_eq!(buffer.dropped_oldest(), 1, "the drop is COUNTED");
    assert!(buffer.has_overflowed(), "the health flag LATCHES");
    // The reconciliation identity within one open (the explicit loss report).
    assert_eq!(
        buffer.appended_total(),
        (buffer.pending_len() + buffer.dropped_oldest() as usize) as u64,
        "appended == pending + dropped — the loss is reported, never silent"
    );
    let pending = buffer.drain(10);
    assert_eq!(
        pending.iter().map(|r| r.sequence).collect::<Vec<_>>(),
        vec![2, 3],
        "the OLDEST was dropped, order preserved"
    );
    cleanup(&dir);
}
