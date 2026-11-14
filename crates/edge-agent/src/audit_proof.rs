// P9.32 — The audit-integrity PROOF surface: hash-chain append verification
//         over the edge audit records, the platform-retained
//         `ChainAnchor` comparison, and the honest tamper-EVIDENCE bounds —
//         proving that pilot audit evidence cannot be altered, deleted,
//         reordered, or silently lost without DETECTION (report §17.3 audit
//         logging, §18.1 local audit buffer, §16.3 retention; ledger P9.32).
//
// WHAT THIS MODULE PROVES BY EXERCISE (never re-implements):
// - the durable offline audit buffer is the EXISTING
//   `crate::audit_buffer::FileAuditBuffer` (`crates/edge-agent/src/audit_buffer.rs`,
//   P6.18): CRC-32 length-prefixed framing, disk-first appends,
//   truncate-and-audit recovery, audited drop-oldest overflow, in-order
//   at-least-once upload with the fail-closed out-of-order commit refusal.
//   The P9.32 outage tests (WRITTEN-UNRUN) drive THAT type — the survive-
//   crash replay, the exactly-once-after-commit upload, and the in-order
//   guarantees are its recorded behaviour, proven by exercise.
// - the record shape is the EXISTING `BufferedAuditRecord` (persisted
//   monotonic sequence, actor, object, detail — the P6.21 denial-audit
//   fields). The chain below consumes drained records AS IS.
//
// WHAT THIS MODULE ADDS (the ledger P9.32 step-1 integrity mechanism):
// the HASH CHAIN and the platform-retained [`ChainAnchor`]. Every audit
// record is folded, in persisted-sequence order, into a running digest that
// ALSO covers the previous digest — so the head digest commits to the whole
// ordered history, not just the last record. The anchor (head digest, first/
// last sequence, record count, version, algorithm, site) is computed at
// export/upload time and retained by the CENTRAL PLATFORM alongside the
// uploaded records (the Phase 6 offline-isolation discipline: the platform
// keeps what the edge host cannot be trusted to keep). Later verification —
// an authority re-run, an acceptance re-check, an incident review —
// recomputes the chain over any claimed copy of the records and compares
// every anchor field. Every mismatch is a NAMED error.
//
// TAMPER-EVIDENCE, NOT TAMPER-PROOF (the honesty bound, AGENTS.md rule 1,
// §13.3 discipline — recorded in `32_audit_integrity_proof.md` and repeated
// in the anchor doc-comment):
// - The chain digest is FNV-1a 64 — the repo's existing non-cryptographic
//   hash pattern (the P4.28/P6.14/P6.15 family), an INTEGRITY CHECKSUM, not
//   a security primitive. It detects accidental corruption and casual/
//   unsophisticated tampering reliably, and any edit/reorder/deletion that
//   does not include a full chain recomputation.
// - A determined adversary WITH WRITE ACCESS TO THE EDGE HOST can recompute
//   a consistent chain (there is no secret in the chain) and re-export a
//   self-consistent record set. Detection then rests on the PLATFORM-
//   RETAINED anchor computed at export time: the adversary must also defeat
//   the upload channel's anchor delivery. That is tamper-EVIDENCE with an
//   honest residual risk, NOT tamper-PROOF.
// - Replacing the digest with HMAC-SHA-256 (or better) under reviewed key
//   management is a recorded DEPLOYMENT REQUIREMENT, exactly as for the
//   P6.15 command signature; the versioned `algorithm` field makes that
//   replacement a detectable format change, never a silent one.
// - What the proof tests establish and what they do NOT (a finite test set;
//   zero observed tampering-missed is a test outcome, not a guarantee) are
//   itemised in `32_audit_integrity_proof.md`.
//
// RELATIONSHIP TO RETENTION (ledger step 2): retention ENFORCEMENT is a
// deployment `RetentionPolicy` decision (P0.16 §4; P9.10's per-class table)
// — the edge deliberately carries NO age-based audit drop (the P6.18
// recorded decision: audit evidence retention is not a period in code).
// This module contributes the integrity EVIDENCE an authority review (P9.29)
// reads; the retention non-deletion proof is the audit buffer's own
// no-age-drop behaviour plus the deployment configuration check.

use flexcore_contracts::time_model::IngestTime;

use crate::audit_buffer::BufferedAuditRecord;

// --- Constants -----------------------------------------------------------------

/// The chain/anchor format version. A format change is a detectable format
/// change, never a silent one (the versioned-scheme discipline).
pub const AUDIT_CHAIN_VERSION: u32 = 1;
/// The chain digest algorithm label. `fnv1a64-chain` is the repo's existing
/// non-cryptographic hash pattern — an integrity checksum, NOT a security
/// primitive (see the module-header honesty note).
pub const AUDIT_CHAIN_ALGORITHM: &str = "fnv1a64-chain";
/// The versioned export-bundle format id (the authority-review export,
/// ledger step 4).
pub const AUDIT_EXPORT_FORMAT: &str = "flexcore-edge-audit-export-v1";

// --- Errors -----------------------------------------------------------------------

/// Named errors of the chain verification. Every failure is named — a
/// verification NEVER reports "probably fine"; the specific mismatch is the
/// finding an operator or authority reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditChainError {
    /// No records were supplied. An EMPTY audit export is not evidence —
    /// refuse rather than compute a vacuous anchor.
    EmptyRecords,
    /// The records are not in strictly increasing persisted-sequence order
    /// (the `FileAuditBuffer` append invariant) — the chain cannot be
    /// computed over a non-ordered set, and a non-ordered set is itself a
    /// finding.
    NonMonotonicSequence {
        previous_sequence: u64,
        found_sequence: u64,
    },
    /// The recomputed head digest differs from the retained anchor's — the
    /// record CONTENT or ORDER does not match what was exported.
    HeadMismatch { expected: u64, computed: u64 },
    /// The record COUNT differs from the retained anchor's — records were
    /// deleted from (or forged into) the claimed copy.
    CountMismatch { expected: u64, computed: u64 },
    /// The first/last persisted sequence differs from the retained anchor's
    /// — a tail truncation or a prepended/padded record set.
    SequenceRangeMismatch {
        expected_first: u64,
        expected_last: u64,
        computed_first: u64,
        computed_last: u64,
    },
    /// The retained anchor was computed under a different chain version —
    /// refused, never guessed at (the versioned-format discipline).
    AnchorVersionMismatch { expected: u32, found: u32 },
    /// The retained anchor was computed under a different algorithm —
    /// refused, never guessed at.
    AnchorAlgorithmMismatch {
        expected: &'static str,
        found: String,
    },
    /// The retained anchor binds a different site — refused, never guessed
    /// at.
    AnchorSiteMismatch {
        expected: String,
        found: String,
    },
    /// The export bundle does not carry the expected format id — refused,
    /// never guessed at (the versioned-format discipline).
    BundleFormatMismatch {
        expected: &'static str,
        found: String,
    },
}

impl std::fmt::Display for AuditChainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditChainError::EmptyRecords => {
                write!(f, "an empty audit record set is not evidence — no anchor is computed")
            }
            AuditChainError::NonMonotonicSequence {
                previous_sequence,
                found_sequence,
            } => write!(
                f,
                "the audit records are not in strictly increasing persisted-sequence order \
                 (previous {previous_sequence}, found {found_sequence}) — the set is itself a \
                 finding"
            ),
            AuditChainError::HeadMismatch { expected, computed } => write!(
                f,
                "AUDIT CHAIN HEAD MISMATCH: the retained anchor commits to head {expected:#018x} \
                 but the claimed records recompute to {computed:#018x} — the record content or \
                 order does not match what was exported (tamper evidence, not proof)"
            ),
            AuditChainError::CountMismatch { expected, computed } => write!(
                f,
                "AUDIT RECORD COUNT MISMATCH: the retained anchor commits to {expected} records \
                 but the claimed copy holds {computed} — records were deleted or forged"
            ),
            AuditChainError::SequenceRangeMismatch {
                expected_first,
                expected_last,
                computed_first,
                computed_last,
            } => write!(
                f,
                "AUDIT SEQUENCE RANGE MISMATCH: the retained anchor spans \
                 [{expected_first}, {expected_last}] but the claimed copy spans \
                 [{computed_first}, {computed_last}] — a tail truncation or padded record set"
            ),
            AuditChainError::AnchorVersionMismatch { expected, found } => write!(
                f,
                "the retained anchor carries chain version {found}; this verifier computes \
                 version {expected} — refused, never guessed at"
            ),
            AuditChainError::AnchorAlgorithmMismatch { expected, found } => write!(
                f,
                "the retained anchor carries algorithm '{found}'; this verifier computes \
                 '{expected}' — refused, never guessed at"
            ),
            AuditChainError::AnchorSiteMismatch { expected, found } => write!(
                f,
                "the retained anchor binds site '{found}' but verification was requested for \
                 '{expected}' — refused, never guessed at"
            ),
            AuditChainError::BundleFormatMismatch { expected, found } => write!(
                f,
                "the export bundle carries format id '{found}'; this verifier accepts '{expected}' \
                 — refused, never guessed at"
            ),
        }
    }
}

impl std::error::Error for AuditChainError {}

// --- The chain digest ----------------------------------------------------------------

/// Folds ONE record into the running chain digest: the previous digest is
/// mixed FIRST, then the record's persisted fields in stable order
/// (sequence, recorded-at millis, actor, object, detail — the exact
/// `BufferedAuditRecord` content, length-prefixed so field boundaries are
/// unambiguous). Because the previous digest is an input, the head commits
/// to the whole ORDERED history: any edit, deletion, reorder, or insertion
/// changes the head.
fn chain_step(previous: u64, record: &BufferedAuditRecord) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut state = FNV_OFFSET;
    let fold = |state: &mut u64, bytes: &[u8]| {
        for byte in bytes {
            *state ^= u64::from(*byte);
            *state = state.wrapping_mul(FNV_PRIME);
        }
    };
    fold(&mut state, &previous.to_le_bytes());
    fold(&mut state, &record.sequence.to_le_bytes());
    fold(&mut state, &(record.recorded_at.unix_millis() as i64).to_le_bytes());
    fold(&mut state, &(record.actor.len() as u64).to_le_bytes());
    fold(&mut state, record.actor.as_bytes());
    fold(&mut state, &(record.object.len() as u64).to_le_bytes());
    fold(&mut state, record.object.as_bytes());
    fold(&mut state, &(record.detail.len() as u64).to_le_bytes());
    fold(&mut state, record.detail.as_bytes());
    state
}

/// Recomputes the chain over a record set IN PERSISTED-SEQUENCE ORDER and
/// returns the head digest. Fails closed on a non-increasing sequence (the
/// set itself is a finding) and on an empty set (an empty export is not
/// evidence).
pub fn recompute_chain_head(records: &[BufferedAuditRecord]) -> Result<u64, AuditChainError> {
    if records.is_empty() {
        return Err(AuditChainError::EmptyRecords);
    }
    let mut head = 0u64; // the domain-separated chain genesis
    let mut previous_sequence: Option<u64> = None;
    for record in records {
        if let Some(previous) = previous_sequence {
            if record.sequence <= previous {
                return Err(AuditChainError::NonMonotonicSequence {
                    previous_sequence: previous,
                    found_sequence: record.sequence,
                });
            }
        }
        head = chain_step(head, record);
        previous_sequence = Some(record.sequence);
    }
    Ok(head)
}

// --- The platform-retained anchor ------------------------------------------------------

/// The chain anchor computed at export/upload time and RETAINED BY THE
/// CENTRAL PLATFORM (the Phase 6 offline-isolation discipline: the platform
/// keeps what the edge host cannot be trusted to keep). A later verification
/// of any claimed copy of the records compares EVERY field below against a
/// recomputation — every mismatch is a named [`AuditChainError`].
///
/// HONESTY (repeated here from the module header because this is the type
/// an authority reads): the anchor is TAMPER-EVIDENCE, not tamper-PROOF.
/// The chain has no secret; an adversary with write access to the edge host
/// can recompute a consistent chain. Detection rests on the anchor being
/// retained at the platform at export time, before the edge could be
/// tampered with. The `algorithm` field is versioned so the recorded
/// replacement with a real MAC under reviewed key management is a
/// detectable format change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainAnchor {
    /// The chain format version ([`AUDIT_CHAIN_VERSION`]).
    pub chain_version: u32,
    /// The digest algorithm ([`AUDIT_CHAIN_ALGORITHM`]).
    pub algorithm: &'static str,
    /// The site the export belongs to (P9.29 authority-pack binding).
    pub site_id: String,
    /// The final chain digest (commits to the whole ordered history).
    pub head_digest: u64,
    /// The first persisted sequence in the export.
    pub first_sequence: u64,
    /// The last persisted sequence in the export.
    pub last_sequence: u64,
    /// The number of records the anchor commits to.
    pub record_count: u64,
    /// When the anchor was computed (the INJECTED clock — never wall time).
    pub computed_at_unix_millis: i64,
}

impl ChainAnchor {
    /// The anchor summary line (audit/authority-pack text).
    pub fn audit_text(&self) -> String {
        format!(
            "audit-chain-anchor v{} alg={} site={} head={:016x} sequences=[{},{}] count={} \
             computed_at={} (tamper-EVIDENCE retained by the platform, not tamper-PROOF)",
            self.chain_version,
            self.algorithm,
            self.site_id,
            self.head_digest,
            self.first_sequence,
            self.last_sequence,
            self.record_count,
            self.computed_at_unix_millis,
        )
    }
}

/// Computes the [`ChainAnchor`] for an audit export: the records must be in
/// strictly increasing persisted-sequence order (the `FileAuditBuffer`
/// append invariant) and non-empty. `computed_at_unix_millis` comes from the
/// INJECTED clock (never `SystemTime::now()`).
pub fn compute_chain_anchor(
    site_id: &str,
    records: &[BufferedAuditRecord],
    computed_at_unix_millis: i64,
) -> Result<ChainAnchor, AuditChainError> {
    let head = recompute_chain_head(records)?;
    Ok(ChainAnchor {
        chain_version: AUDIT_CHAIN_VERSION,
        algorithm: AUDIT_CHAIN_ALGORITHM,
        site_id: site_id.to_string(),
        head_digest: head,
        first_sequence: records[0].sequence,
        last_sequence: records[records.len() - 1].sequence,
        record_count: records.len() as u64,
        computed_at_unix_millis,
    })
}

/// Verifies a claimed copy of the audit records against the PLATFORM-
/// RETAINED [`ChainAnchor`]. Every anchor field is compared against a
/// recomputation over the claimed copy; every mismatch is a named
/// [`AuditChainError`] carrying the specific finding. Verification of a
/// non-ordered set fails with [`AuditChainError::NonMonotonicSequence`] —
/// the disorder itself is a finding.
pub fn verify_against_anchor(
    site_id: &str,
    records: &[BufferedAuditRecord],
    anchor: &ChainAnchor,
) -> Result<(), AuditChainError> {
    // Format gates first (refused, never guessed at).
    if anchor.chain_version != AUDIT_CHAIN_VERSION {
        return Err(AuditChainError::AnchorVersionMismatch {
            expected: AUDIT_CHAIN_VERSION,
            found: anchor.chain_version,
        });
    }
    if anchor.algorithm != AUDIT_CHAIN_ALGORITHM {
        return Err(AuditChainError::AnchorAlgorithmMismatch {
            expected: AUDIT_CHAIN_ALGORITHM,
            found: anchor.algorithm.to_string(),
        });
    }
    if anchor.site_id != site_id {
        return Err(AuditChainError::AnchorSiteMismatch {
            expected: site_id.to_string(),
            found: anchor.site_id.clone(),
        });
    }
    if records.is_empty() {
        // An empty claimed copy against a retained anchor is a count
        // mismatch (the anchor committed to something), never a vacuous
        // pass — unless the anchor itself committed to zero records, which
        // `compute_chain_anchor` refuses to produce.
        return Err(AuditChainError::CountMismatch {
            expected: anchor.record_count,
            computed: 0,
        });
    }
    let computed_head = recompute_chain_head(records)?;
    let computed_first = records[0].sequence;
    let computed_last = records[records.len() - 1].sequence;
    let computed_count = records.len() as u64;
    if computed_count != anchor.record_count {
        return Err(AuditChainError::CountMismatch {
            expected: anchor.record_count,
            computed: computed_count,
        });
    }
    if computed_first != anchor.first_sequence || computed_last != anchor.last_sequence {
        return Err(AuditChainError::SequenceRangeMismatch {
            expected_first: anchor.first_sequence,
            expected_last: anchor.last_sequence,
            computed_first,
            computed_last,
        });
    }
    if computed_head != anchor.head_digest {
        return Err(AuditChainError::HeadMismatch {
            expected: anchor.head_digest,
            computed: computed_head,
        });
    }
    Ok(())
}

// --- The authority-review export bundle (ledger step 4) ---------------------------------

/// The audit export an authority review (P9.29) reads: the claimed records,
/// the platform-retained anchor, and the versioned format id. The bundle is
/// the UNIT the acceptance review stores; verification is
/// [`verify_bundle`] (which re-checks the format id, not just the chain).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditExportBundle {
    /// The format id ([`AUDIT_EXPORT_FORMAT`]).
    pub format: &'static str,
    /// The site the export belongs to.
    pub site_id: String,
    /// The platform-retained anchor (computed at export time).
    pub anchor: ChainAnchor,
    /// The claimed records, in persisted-sequence order.
    pub records: Vec<BufferedAuditRecord>,
}

impl AuditExportBundle {
    /// Builds the bundle (verification happens in [`verify_bundle`]; a
    /// bundle is a CLAIM, not a verdict).
    pub fn new(
        site_id: &str,
        records: Vec<BufferedAuditRecord>,
        computed_at_unix_millis: i64,
    ) -> Result<Self, AuditChainError> {
        let anchor = compute_chain_anchor(site_id, &records, computed_at_unix_millis)?;
        Ok(Self {
            format: AUDIT_EXPORT_FORMAT,
            site_id: site_id.to_string(),
            anchor,
            records,
        })
    }
}

/// Verifies an export bundle end-to-end: format id, site binding, and the
/// chain-vs-anchor comparison. Every failure is a named [`AuditChainError`].
pub fn verify_bundle(bundle: &AuditExportBundle) -> Result<(), AuditChainError> {
    if bundle.format != AUDIT_EXPORT_FORMAT {
        return Err(AuditChainError::BundleFormatMismatch {
            expected: AUDIT_EXPORT_FORMAT,
            found: bundle.format.to_string(),
        });
    }
    verify_against_anchor(&bundle.site_id, &bundle.records, &bundle.anchor)
}

/// The verified-export result an acceptance review records: the bundle plus
/// the verification verdict text (the integrity evidence the authority pack
/// attaches, ledger step 4). Never constructed for a failing bundle — a
/// failure is the named error, and no "verified" record is written for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedAuditExport {
    pub bundle: AuditExportBundle,
    pub verdict_text: String,
}

/// Verifies a bundle and, on success, produces the [`VerifiedAuditExport`]
/// evidence record (with the honest tamper-EVIDENCE wording in the verdict).
pub fn verify_bundle_to_evidence(bundle: &AuditExportBundle) -> Result<VerifiedAuditExport, AuditChainError> {
    verify_bundle(bundle)?;
    Ok(VerifiedAuditExport {
        bundle: bundle.clone(),
        verdict_text: format!(
            "{} :: VERIFIED against the platform-retained anchor: content, order, count, and \
             sequence range all match. This is tamper-EVIDENCE (chain algorithm '{}', no \
             secret in the chain): it does not prove a determined host-level adversary could \
             not have re-signed a copy — the anchor was retained by the platform at export \
             time.",
            bundle.anchor.audit_text(),
            AUDIT_CHAIN_ALGORITHM,
        ),
    })
}

// Convenience: rebuild an IngestTime in tests without depending on the
// buffer's internals. (Public because the proof tests construct records.)
/// Builds a valid [`IngestTime`] for proof fixtures.
pub fn ingest_time(unix_millis: i64) -> Result<IngestTime, String> {
    IngestTime::from_unix_millis(unix_millis).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(sequence: u64, detail: &str) -> BufferedAuditRecord {
        BufferedAuditRecord {
            sequence,
            recorded_at: IngestTime::from_unix_millis(1_700_000_000_000 + sequence as i64)
                .expect("valid instant"),
            kind: crate::audit_buffer::BufferedAuditKind::CommandRejected,
            actor: "edge:pilot-site-A".to_string(),
            object: format!("command-cmd-{sequence}"),
            detail: detail.to_string(),
        }
    }

    #[test]
    fn the_chain_commits_to_content_order_and_count() {
        let records = vec![record(1, "a"), record(2, "b"), record(3, "c")];
        let head = recompute_chain_head(&records).expect("computes");
        // Same records, same order: identical head.
        assert_eq!(recompute_chain_head(&records).expect("computes"), head);
        // A content edit changes the head.
        let mut edited = records.clone();
        edited[1].detail = "tampered".to_string();
        assert_ne!(recompute_chain_head(&edited).expect("computes"), head);
        // A reorder changes the head.
        let reordered = vec![record(1, "a"), record(3, "c"), record(2, "b")];
        assert_ne!(recompute_chain_head(&reordered).expect("computes"), head);
        // A deletion changes the head AND the count.
        let deleted = vec![record(1, "a"), record(3, "c")];
        assert_ne!(recompute_chain_head(&deleted).expect("computes"), head);
        // An empty set is refused outright.
        assert!(matches!(
            recompute_chain_head(&[]),
            Err(AuditChainError::EmptyRecords)
        ));
        // A non-increasing sequence set is refused outright.
        let disordered = vec![record(2, "b"), record(1, "a")];
        assert!(matches!(
            recompute_chain_head(&disordered),
            Err(AuditChainError::NonMonotonicSequence { .. })
        ));
    }

    #[test]
    fn the_anchor_round_trips_through_verification() {
        let records = vec![record(1, "a"), record(2, "b")];
        let anchor =
            compute_chain_anchor("pilot-site-A", &records, 1_700_000_000_500).expect("computes");
        assert_eq!(anchor.chain_version, AUDIT_CHAIN_VERSION);
        assert_eq!(anchor.algorithm, AUDIT_CHAIN_ALGORITHM);
        assert_eq!(anchor.record_count, 2);
        assert_eq!(anchor.first_sequence, 1);
        assert_eq!(anchor.last_sequence, 2);
        verify_against_anchor("pilot-site-A", &records, &anchor).expect("verifies");
        // The anchor text carries the honest tamper-EVIDENCE wording.
        assert!(anchor.audit_text().contains("tamper-EVIDENCE"));
        assert!(anchor.audit_text().contains("not tamper-PROOF"));
    }
}
