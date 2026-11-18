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
//   does not include a full chain recomputation. TEST-GRADE, per LR-203:
//   it is never a release signature and never a security primitive.
// - A determined adversary WITH WRITE ACCESS TO THE EDGE HOST can recompute
//   a consistent chain (there is no secret in the FNV chain) and re-export a
//   self-consistent record set. Detection then rests on the PLATFORM-
//   RETAINED anchor computed at export time: the adversary must also defeat
//   the upload channel's anchor delivery. That is tamper-EVIDENCE with an
//   honest residual risk, NOT tamper-PROOF.
// - P11.15 ADDS the keyed construction below (the P11-E-1 DEFAULT, option
//   a): a keyed HMAC-SHA-256 linear chain (`hmac-sha256-chain-v1`, chain
//   version 2) with the per-site key supplied by the DEPLOYMENT, an
//   anchor-continuation seed (the periodic anchor-export design), and
//   bundle verification. The FNV-1a chain above stays byte-identical and
//   labeled TEST-GRADE (LR-203 discipline). The keyed chain is IMPLEMENTED
//   CODE, never deployed or exercised: KEY MANAGEMENT (generation,
//   distribution, rotation, storage) is a HUMAN-GATED DEPLOYMENT
//   REQUIREMENT (program conventions §6.6) — until an operator provisions
//   reviewed keys, the keyed chain is not running, and nothing here is a
//   release signature. A keyed chain resists recomputation by an adversary
//   WITHOUT the key; key compromise defeats it — tamper-EVIDENCE with a
//   smaller attack surface, still not tamper-PROOF.
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
    // --- P11.15 keyed-chain additions (chain version 2) ---
    /// The deployment-supplied keyed-chain key material was shorter than
    /// the keyed chain requires. Fail closed — a short key is a deployment
    /// error, never a silent downgrade.
    KeyedKeyTooShort {
        /// The material length that was supplied.
        length: usize,
        /// The minimum length the keyed chain accepts.
        required: usize,
    },
    /// The recomputed KEYED head differs from the retained keyed anchor's —
    /// the record CONTENT or ORDER does not match what was exported.
    KeyedHeadMismatch {
        /// The head the keyed anchor commits to (32 bytes, hex in Display).
        expected: [u8; 32],
        /// The head the claimed records recompute to.
        computed: [u8; 32],
    },
    /// The recomputed KEYED digest at this sequence differs from the
    /// claimed copy's — a content edit the anchor catches row-wise.
    KeyedDigestMismatch {
        /// The persisted sequence where the mismatch occurred.
        sequence: u64,
    },
    /// The keyed anchor's continuation seed differs from the claimed
    /// export's seed — the window does not continue the claimed history
    /// (the periodic anchor-export continuity gate).
    KeyedContinuationMismatch {
        /// The seed verification was requested with (hex, or None = genesis).
        expected: Option<String>,
        /// The seed the keyed anchor names.
        found: Option<String>,
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
            AuditChainError::KeyedKeyTooShort { length, required } => write!(
                f,
                "the keyed-chain key material is {length} bytes; the keyed audit chain requires \
                 at least {required} bytes — refusing rather than silently downgrading"
            ),
            AuditChainError::KeyedHeadMismatch { expected, computed } => write!(
                f,
                "KEYED CHAIN HEAD MISMATCH: the retained keyed anchor commits to head {} \
                 but the claimed records recompute to {} — the record content or order does \
                 not match what was exported (keyed tamper-EVIDENCE, not tamper-PROOF)",
                hex_encode_bytes(expected),
                hex_encode_bytes(computed)
            ),
            AuditChainError::KeyedDigestMismatch { sequence } => write!(
                f,
                "KEYED CHAIN DIGEST MISMATCH at persisted sequence {sequence}: the recomputed \
                 keyed digest differs from the claimed copy's — the record content, the stored \
                 digest, or the order was altered"
            ),
            AuditChainError::KeyedContinuationMismatch { expected, found } => write!(
                f,
                "the keyed anchor names continuation seed {:?}; the claimed export seeds from \
                 {:?} — the window does not continue the claimed history",
                found, expected
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

// --- P11.15 — the keyed HMAC-SHA-256 chain (the P11-E-1 DEFAULT) ------------
//
// The P11-E-1 DEFAULT (option a) implemented at the edge: a keyed
// HMAC-SHA-256 linear chain over the same `BufferedAuditRecord` content the
// test-grade FNV chain folds, with chain VERSION 2 and the algorithm label
// `hmac-sha256-chain-v1`, a deployment-supplied per-site key, and the
// periodic anchor-export design (each anchor names the previous anchor's
// head as its continuation seed, so exported anchors form a second-order
// chain an authority review can walk without the records).
//
// DEPLOYMENT REQUIREMENT (repeated at the use sites because this is the
// boundary an operator reads): this construction is IMPLEMENTED CODE — it
// has never been compiled, keyed, deployed, or exercised in this program.
// Key management (generation, distribution, rotation, storage, the operator
// procedure) is a HUMAN-GATED DEPLOYMENT concern (program conventions §6.6);
// this module consumes a key supplied by the deployment and never generates,
// loads, or stores one. The keyed chain is never a release signature; the
// FNV-1a chain above stays labeled TEST-GRADE (LR-203).
//
// WHY THE CRYPTO IS DUPLICATED HERE (the recorded tradeoff): the platform
// chain lives in `flexcore-audit::chain` (the port side); this module
// cannot depend on it without a new internal dependency edge, which this
// wave's file-ownership rules forbid. SHA-256 (FIPS 180-4) and HMAC (RFC
// 2104) are fully specified, so the duplication is mechanical and both
// copies are anchored to the same published test vectors (RFC 4231 test
// case 2 asserted in BOTH crates' test modules). Consolidation behind one
// shared module is a recorded follow-up for a wave that owns both crates'
// manifests and the dependency-rule checker's edge table.

/// The keyed-chain domain-separation prefix every keyed digest input
/// carries.
const KEYED_CHAIN_DOMAIN: &[u8] = b"flexcore-edge-audit-keyed-chain-v1";

/// The keyed-chain format version. Version 1 is the FNV-1a test-grade
/// chain above; version 2 is keyed HMAC-SHA-256 — a format change is a
/// detectable format change, never a silent one.
pub const KEYED_CHAIN_VERSION: u32 = 2;

/// The keyed-chain digest algorithm label.
pub const KEYED_CHAIN_ALGORITHM: &str = "hmac-sha256-chain-v1";

/// The versioned keyed export-bundle format id (the authority-review keyed
/// export; the periodic anchor-export design).
pub const KEYED_EXPORT_FORMAT: &str = "flexcore-edge-audit-keyed-export-v1";

/// The minimum keyed-chain key material length (RFC 2104 recommends a key
/// at least as long as the hash output; shorter material is refused).
pub const KEYED_CHAIN_MIN_KEY_BYTES: usize = 32;

const KEYED_HMAC_BLOCK_SIZE: usize = 64;

/// SHA-256 per FIPS 180-4 (the duplicated platform copy; see the section
/// header for why the duplication is recorded, not accidental).
fn keyed_sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    const H0: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut h = H0;
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut message = Vec::with_capacity(data.len() + 72);
    message.extend_from_slice(data);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());
    for block in message.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, word) in block.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for t in 16..64 {
            let s0 = w[t - 15].rotate_right(7) ^ w[t - 15].rotate_right(18) ^ (w[t - 15] >> 3);
            let s1 = w[t - 2].rotate_right(17) ^ w[t - 2].rotate_right(19) ^ (w[t - 2] >> 10);
            w[t] = w[t - 16]
                .wrapping_add(s0)
                .wrapping_add(w[t - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d) = (h[0], h[1], h[2], h[3]);
        let (mut e, mut f, mut g, mut hh) = (h[4], h[5], h[6], h[7]);
        for t in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[t])
                .wrapping_add(w[t]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// HMAC-SHA-256 per RFC 2104 for a 64-byte-block hash (the duplicated
/// platform copy; see the section header).
fn keyed_hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut block_key = [0u8; KEYED_HMAC_BLOCK_SIZE];
    if key.len() > KEYED_HMAC_BLOCK_SIZE {
        block_key[..32].copy_from_slice(&keyed_sha256(key));
    } else {
        block_key[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; KEYED_HMAC_BLOCK_SIZE];
    let mut opad = [0x5cu8; KEYED_HMAC_BLOCK_SIZE];
    for i in 0..KEYED_HMAC_BLOCK_SIZE {
        ipad[i] ^= block_key[i];
        opad[i] ^= block_key[i];
    }
    let mut inner = Vec::with_capacity(KEYED_HMAC_BLOCK_SIZE + message.len());
    inner.extend_from_slice(&ipad);
    inner.extend_from_slice(message);
    let inner_digest = keyed_sha256(&inner);
    let mut outer = Vec::with_capacity(KEYED_HMAC_BLOCK_SIZE + 32);
    outer.extend_from_slice(&opad);
    outer.extend_from_slice(&inner_digest);
    keyed_sha256(&outer)
}

/// Lower-case hex for digest bytes (anchors, bundle text, error Display).
fn hex_encode_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

/// The deployment-supplied keyed-chain key. Holds a SECRET: no `Debug` or
/// `Clone` derive that would print or copy the material, best-effort
/// volatile zeroization on drop. The key is CONSUMED, never created — key
/// generation, distribution, rotation, and storage are the DEPLOYMENT
/// REQUIREMENT gate (P11.15 decision record; P11-E-1 stays OPEN).
pub struct EdgeChainKey([u8; 32]);

impl EdgeChainKey {
    /// Derives the working key from deployment-supplied material via HMAC
    /// domain separation. Refuses material shorter than
    /// [`KEYED_CHAIN_MIN_KEY_BYTES`] (fail closed).
    pub fn from_bytes(material: &[u8]) -> Result<Self, AuditChainError> {
        if material.len() < KEYED_CHAIN_MIN_KEY_BYTES {
            return Err(AuditChainError::KeyedKeyTooShort {
                length: material.len(),
                required: KEYED_CHAIN_MIN_KEY_BYTES,
            });
        }
        let mut domain =
            Vec::with_capacity(KEYED_CHAIN_DOMAIN.len() + 1 + material.len());
        domain.extend_from_slice(KEYED_CHAIN_DOMAIN);
        domain.push(0);
        domain.extend_from_slice(material);
        Ok(Self(keyed_hmac_sha256(KEYED_CHAIN_DOMAIN, &domain)))
    }
}

impl Drop for EdgeChainKey {
    fn drop(&mut self) {
        // Best-effort zeroization (volatile writes; compiler-made copies
        // are out of scope — the honest wording is best-effort).
        // SAFETY: `byte` is a live `&mut u8` into `self.0`; the write is
        // in-bounds and the pointer is valid for the write (the standard
        // zeroize idiom; same shape as the audit-chain `Drop` in
        // flexcore-audit).
        for byte in self.0.iter_mut() {
            unsafe {
                std::ptr::write_volatile(byte as *mut u8, 0);
            }
        }
    }
}

/// Folds ONE record into the keyed chain: HMAC under the site key over
/// (domain, version, previous head, and the record's persisted fields in
/// stable order — sequence, recorded-at millis, the KIND discriminant
/// (declaration order; a variant-order change is a format change the
/// version gates), actor, object, detail — length-prefixed so field
/// boundaries are unambiguous). The keyed chain folds the COMPLETE record
/// including the kind, which the test-grade FNV chain omits (that
/// difference is recorded; the FNV chain stays byte-identical per LR-203).
fn keyed_chain_step(
    key: &EdgeChainKey,
    previous: &[u8; 32],
    record: &BufferedAuditRecord,
) -> [u8; 32] {
    let mut message = Vec::with_capacity(96 + 256);
    message.extend_from_slice(KEYED_CHAIN_DOMAIN);
    message.extend_from_slice(&KEYED_CHAIN_VERSION.to_le_bytes());
    message.extend_from_slice(previous);
    message.extend_from_slice(&record.sequence.to_le_bytes());
    message.extend_from_slice(&(record.recorded_at.unix_millis() as i64).to_le_bytes());
    message.push(record.kind as u8);
    for field in [
        record.actor.as_bytes(),
        record.object.as_bytes(),
        record.detail.as_bytes(),
    ] {
        message.extend_from_slice(&(field.len() as u64).to_le_bytes());
        message.extend_from_slice(field);
    }
    keyed_hmac_sha256(&key.0, &message)
}

/// The keyed chain's genesis head: HMAC over the domain string and a fixed
/// genesis label.
fn keyed_genesis_head(key: &EdgeChainKey) -> [u8; 32] {
    let mut message = Vec::with_capacity(KEYED_CHAIN_DOMAIN.len() + 16);
    message.extend_from_slice(KEYED_CHAIN_DOMAIN);
    message.push(0);
    message.extend_from_slice(b"genesis");
    keyed_hmac_sha256(&key.0, &message)
}

/// Recomputes the keyed chain head over a record set in persisted-sequence
/// order, seeded from `continues_from` (`None` = genesis; the periodic
/// anchor-export design seeds continuation windows from the previous
/// anchor's head). Fails closed on an empty set (an empty export is not
/// evidence) and on a non-increasing sequence (the set itself is a
/// finding).
pub fn recompute_keyed_chain_head(
    key: &EdgeChainKey,
    continues_from: Option<[u8; 32]>,
    records: &[BufferedAuditRecord],
) -> Result<[u8; 32], AuditChainError> {
    if records.is_empty() {
        return Err(AuditChainError::EmptyRecords);
    }
    let mut head = match continues_from {
        Some(seed) => seed,
        None => keyed_genesis_head(key),
    };
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
        head = keyed_chain_step(key, &head, record);
        previous_sequence = Some(record.sequence);
    }
    Ok(head)
}

/// The keyed chain anchor computed at export/upload time and RETAINED BY
/// THE CENTRAL PLATFORM. Carries the continuation seed so exported anchors
/// chain across windows (the periodic anchor-export design: the cadence is
/// a deployment decision, never coded here — the operator exports at a
/// registered interval and the platform retains what the edge host cannot
/// be trusted to keep).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyedChainAnchor {
    /// The chain format version ([`KEYED_CHAIN_VERSION`]).
    pub chain_version: u32,
    /// The digest algorithm ([`KEYED_CHAIN_ALGORITHM`]).
    pub algorithm: &'static str,
    /// The site the export belongs to (P9.29 authority-pack binding).
    pub site_id: String,
    /// The previous window's anchor head this window continues from
    /// (lower-case hex, or `None` = genesis).
    pub continues_from_head: Option<String>,
    /// The final keyed head digest (commits to the whole ordered history).
    pub head_digest: [u8; 32],
    /// The first persisted sequence in the export.
    pub first_sequence: u64,
    /// The last persisted sequence in the export.
    pub last_sequence: u64,
    /// The number of records the anchor commits to.
    pub record_count: u64,
    /// When the anchor was computed (the INJECTED clock — never wall time).
    pub computed_at_unix_millis: i64,
}

impl KeyedChainAnchor {
    /// The keyed anchor summary line (audit/authority-pack text). Carries
    /// the honest keyed wording: tamper-EVIDENCE under reviewed key
    /// management, DEPLOYMENT REQUIREMENT, never a release signature.
    pub fn audit_text(&self) -> String {
        format!(
            "keyed-audit-chain-anchor v{} alg={} site={} continues_from={} head={} sequences=[{},{}] \
             count={} computed_at={} (keyed tamper-EVIDENCE; DEPLOYMENT REQUIREMENT: valid only \
             under reviewed key management, never a release signature)",
            self.chain_version,
            self.algorithm,
            self.site_id,
            self.continues_from_head.as_deref().unwrap_or("genesis"),
            hex_encode_bytes(&self.head_digest),
            self.first_sequence,
            self.last_sequence,
            self.record_count,
            self.computed_at_unix_millis,
        )
    }
}

/// Computes the [`KeyedChainAnchor`] for a keyed audit export. The records
/// must be in strictly increasing persisted-sequence order (the
/// `FileAuditBuffer` append invariant) and non-empty;
/// `continues_from_head` is the previous window's anchor head (`None` =
/// genesis) and seeds the recomputation, so a continuation anchor verifies
/// ONLY against a chain that really started at the named previous head.
/// `computed_at_unix_millis` comes from the INJECTED clock.
pub fn compute_keyed_chain_anchor(
    site_id: &str,
    key: &EdgeChainKey,
    continues_from_head: Option<[u8; 32]>,
    records: &[BufferedAuditRecord],
    computed_at_unix_millis: i64,
) -> Result<KeyedChainAnchor, AuditChainError> {
    let head = recompute_keyed_chain_head(key, continues_from_head, records)?;
    Ok(KeyedChainAnchor {
        chain_version: KEYED_CHAIN_VERSION,
        algorithm: KEYED_CHAIN_ALGORITHM,
        site_id: site_id.to_string(),
        continues_from_head: continues_from_head.map(|seed| hex_encode_bytes(&seed)),
        head_digest: head,
        first_sequence: records[0].sequence,
        last_sequence: records[records.len() - 1].sequence,
        record_count: records.len() as u64,
        computed_at_unix_millis,
    })
}

/// Verifies a claimed copy of the audit records against the PLATFORM-
/// RETAINED [`KeyedChainAnchor`]: version, algorithm, site, continuation
/// seed, count, sequence range, and the keyed head. Every mismatch is a
/// named [`AuditChainError`]. Recomputation requires the SAME deployment
/// key the anchor was computed under — an adversary without the key cannot
/// forge a passing set (the keyed upgrade over the test-grade chain).
pub fn verify_against_keyed_anchor(
    site_id: &str,
    key: &EdgeChainKey,
    continues_from: Option<[u8; 32]>,
    records: &[BufferedAuditRecord],
    anchor: &KeyedChainAnchor,
) -> Result<(), AuditChainError> {
    // Format gates first (refused, never guessed at).
    if anchor.chain_version != KEYED_CHAIN_VERSION {
        return Err(AuditChainError::AnchorVersionMismatch {
            expected: KEYED_CHAIN_VERSION,
            found: anchor.chain_version,
        });
    }
    if anchor.algorithm != KEYED_CHAIN_ALGORITHM {
        return Err(AuditChainError::AnchorAlgorithmMismatch {
            expected: KEYED_CHAIN_ALGORITHM,
            found: anchor.algorithm.to_string(),
        });
    }
    if anchor.site_id != site_id {
        return Err(AuditChainError::AnchorSiteMismatch {
            expected: site_id.to_string(),
            found: anchor.site_id.clone(),
        });
    }
    let seed_text = continues_from.map(|seed| hex_encode_bytes(&seed));
    if anchor.continues_from_head != seed_text {
        return Err(AuditChainError::KeyedContinuationMismatch {
            expected: seed_text,
            found: anchor.continues_from_head.clone(),
        });
    }
    if records.is_empty() {
        return Err(AuditChainError::CountMismatch {
            expected: anchor.record_count,
            computed: 0,
        });
    }
    let computed_count = records.len() as u64;
    if computed_count != anchor.record_count {
        return Err(AuditChainError::CountMismatch {
            expected: anchor.record_count,
            computed: computed_count,
        });
    }
    let computed_first = records[0].sequence;
    let computed_last = records[records.len() - 1].sequence;
    if computed_first != anchor.first_sequence || computed_last != anchor.last_sequence {
        return Err(AuditChainError::SequenceRangeMismatch {
            expected_first: anchor.first_sequence,
            expected_last: anchor.last_sequence,
            computed_first,
            computed_last,
        });
    }
    // Row-wise digests first (names the sequence), then the head.
    let mut head = match continues_from {
        Some(seed) => seed,
        None => keyed_genesis_head(key),
    };
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
        let computed = keyed_chain_step(key, &head, record);
        // The claimed set carries no per-row keyed digests (the export
        // shape is records + anchor), so the row-wise check is the
        // recomputation itself; the head comparison below is the binding.
        head = computed;
        previous_sequence = Some(record.sequence);
    }
    if head != anchor.head_digest {
        return Err(AuditChainError::KeyedHeadMismatch {
            expected: anchor.head_digest,
            computed: head,
        });
    }
    Ok(())
}

/// The keyed audit export an authority review (P9.29) reads: the claimed
/// records, the platform-retained keyed anchor, and the versioned keyed
/// format id. Verification is [`verify_keyed_bundle`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyedAuditExportBundle {
    /// The format id ([`KEYED_EXPORT_FORMAT`]).
    pub format: &'static str,
    /// The site the export belongs to.
    pub site_id: String,
    /// The platform-retained keyed anchor (computed at export time).
    pub anchor: KeyedChainAnchor,
    /// The claimed records, in persisted-sequence order.
    pub records: Vec<BufferedAuditRecord>,
}

impl KeyedAuditExportBundle {
    /// Builds the bundle. The export window continues `continues_from_head`
    /// (the previous anchor's head, or `None` = genesis) — the periodic
    /// anchor-export design.
    pub fn new(
        site_id: &str,
        key: &EdgeChainKey,
        continues_from_head: Option<[u8; 32]>,
        records: Vec<BufferedAuditRecord>,
        computed_at_unix_millis: i64,
    ) -> Result<Self, AuditChainError> {
        let anchor = compute_keyed_chain_anchor(
            site_id,
            key,
            continues_from_head,
            &records,
            computed_at_unix_millis,
        )?;
        Ok(Self {
            format: KEYED_EXPORT_FORMAT,
            site_id: site_id.to_string(),
            anchor,
            records,
        })
    }
}

/// Verifies a keyed export bundle end-to-end: format id, and the full
/// keyed anchor comparison. Every failure is a named [`AuditChainError`].
pub fn verify_keyed_bundle(
    site_id: &str,
    key: &EdgeChainKey,
    continues_from: Option<[u8; 32]>,
    bundle: &KeyedAuditExportBundle,
) -> Result<(), AuditChainError> {
    if bundle.format != KEYED_EXPORT_FORMAT {
        return Err(AuditChainError::BundleFormatMismatch {
            expected: KEYED_EXPORT_FORMAT,
            found: bundle.format.to_string(),
        });
    }
    verify_against_keyed_anchor(site_id, key, continues_from, &bundle.records, &bundle.anchor)
}

/// The verified keyed-export result an acceptance review records. Never
/// constructed for a failing bundle — a failure is the named error, and no
/// "verified" record is written for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedKeyedAuditExport {
    pub bundle: KeyedAuditExportBundle,
    pub verdict_text: String,
}

/// Verifies a keyed bundle and, on success, produces the
/// [`VerifiedKeyedAuditExport`] evidence record with the honest keyed
/// wording: tamper-EVIDENCE under reviewed key management, DEPLOYMENT
/// REQUIREMENT, never a release signature.
pub fn verify_keyed_bundle_to_evidence(
    site_id: &str,
    key: &EdgeChainKey,
    continues_from: Option<[u8; 32]>,
    bundle: &KeyedAuditExportBundle,
) -> Result<VerifiedKeyedAuditExport, AuditChainError> {
    verify_keyed_bundle(site_id, key, continues_from, bundle)?;
    Ok(VerifiedKeyedAuditExport {
        bundle: bundle.clone(),
        verdict_text: format!(
            "{} :: VERIFIED against the platform-retained keyed anchor: content, order, count, \
             sequence range, and the keyed head all match. This is tamper-EVIDENCE under \
             reviewed key management (chain algorithm '{}'): recomputation requires the \
             deployment key, and the residual risk is key compromise — it is NOT tamper-PROOF \
             and never a release signature. DEPLOYMENT REQUIREMENT gate: the key management \
             plan must be reviewed and signed off before this verdict is relied on.",
            bundle.anchor.audit_text(),
            KEYED_CHAIN_ALGORITHM,
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
        // A reorder is refused outright: the reordered set is non-monotonic
        // (3 then 2), and the chain never computes a head over a reordered
        // set -- the tamper is detected by the refusal, not by a changed head.
        let reordered = vec![record(1, "a"), record(3, "c"), record(2, "b")];
        assert!(matches!(
            recompute_chain_head(&reordered),
            Err(AuditChainError::NonMonotonicSequence {
                previous_sequence: 3,
                found_sequence: 2
            })
        ));
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

    // --- P11.15 keyed chain (chain version 2) --------------------------------

    fn keyed_test_key() -> EdgeChainKey {
        // Test key material ONLY — never a real deployment key.
        EdgeChainKey::from_bytes(&[0x37u8; 32]).expect("valid test key")
    }

    #[test]
    fn the_duplicated_hmac_matches_rfc_4231_test_case_2() {
        // RFC 4231 test case 2 ("Jefe") — anchors the duplicated crypto in
        // this crate to the same published vector the platform-side chain
        // asserts (flexcore-audit::chain tests).
        let digest = keyed_hmac_sha256(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(
            hex_encode_bytes(&digest),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn short_key_material_is_refused() {
        assert!(matches!(
            EdgeChainKey::from_bytes(&[0u8; 8]),
            Err(AuditChainError::KeyedKeyTooShort { length: 8, required: 32 })
        ));
    }

    #[test]
    fn the_keyed_chain_commits_to_content_order_count_and_kind() {
        let key = keyed_test_key();
        let records = vec![record(1, "a"), record(2, "b"), record(3, "c")];
        let head = recompute_keyed_chain_head(&key, None, &records).expect("computes");
        assert_eq!(recompute_keyed_chain_head(&key, None, &records).expect("computes"), head);

        // A content edit changes the head.
        let mut edited = records.clone();
        edited[1].detail = "tampered".to_string();
        assert_ne!(
            recompute_keyed_chain_head(&key, None, &edited).expect("computes"),
            head
        );
        // The kind is part of the keyed fold (unlike the FNV chain): a kind
        // change alone changes the head.
        let mut kind_changed = records.clone();
        kind_changed[2].kind = crate::audit_buffer::BufferedAuditKind::EnrollmentRejected;
        assert_ne!(
            recompute_keyed_chain_head(&key, None, &kind_changed).expect("computes"),
            head
        );
        // A reorder is refused outright (the reordered set is non-monotonic:
        // 3 then 2); a deletion changes the head.
        let reordered = vec![record(1, "a"), record(3, "c"), record(2, "b")];
        assert!(matches!(
            recompute_keyed_chain_head(&key, None, &reordered),
            Err(AuditChainError::NonMonotonicSequence {
                previous_sequence: 3,
                found_sequence: 2
            })
        ));
        let deleted = vec![record(1, "a"), record(3, "c")];
        assert_ne!(
            recompute_keyed_chain_head(&key, None, &deleted).expect("computes"),
            head
        );
        // Empty set refused; non-monotonic refused.
        assert!(matches!(
            recompute_keyed_chain_head(&key, None, &[]),
            Err(AuditChainError::EmptyRecords)
        ));
        let disordered = vec![record(2, "b"), record(1, "a")];
        assert!(matches!(
            recompute_keyed_chain_head(&key, None, &disordered),
            Err(AuditChainError::NonMonotonicSequence { .. })
        ));
    }

    #[test]
    fn the_keyed_anchor_round_trips_and_gates_the_continuation() {
        let key = keyed_test_key();
        let records = vec![record(1, "a"), record(2, "b")];
        let anchor = compute_keyed_chain_anchor("pilot-site-A", &key, None, &records, 1_700_000_000_500)
            .expect("computes");
        assert_eq!(anchor.chain_version, KEYED_CHAIN_VERSION);
        assert_eq!(anchor.algorithm, KEYED_CHAIN_ALGORITHM);
        assert_eq!(anchor.record_count, 2);
        assert!(anchor.continues_from_head.is_none());
        verify_against_keyed_anchor("pilot-site-A", &key, None, &records, &anchor)
            .expect("verifies");
        // The keyed anchor text carries the honest DEPLOYMENT REQUIREMENT
        // wording, never a release signature.
        assert!(anchor.audit_text().contains("DEPLOYMENT REQUIREMENT"));
        assert!(anchor.audit_text().contains("never a release signature"));

        // A continuation window seeds from the previous head.
        let previous_head =
            recompute_keyed_chain_head(&key, None, &records).expect("computes");
        let window_two = vec![record(3, "c"), record(4, "d")];
        let anchor_two = compute_keyed_chain_anchor(
            "pilot-site-A",
            &key,
            Some(previous_head),
            &window_two,
            1_700_000_001_000,
        )
        .expect("computes");
        assert_eq!(
            anchor_two.continues_from_head.as_deref(),
            Some(hex_encode_bytes(&previous_head).as_str())
        );
        verify_against_keyed_anchor(
            "pilot-site-A",
            &key,
            Some(previous_head),
            &window_two,
            &anchor_two,
        )
        .expect("verifies");
        // The same window WITHOUT the seed is refused: the anchor claims to
        // continue a specific history.
        assert!(matches!(
            verify_against_keyed_anchor("pilot-site-A", &key, None, &window_two, &anchor_two),
            Err(AuditChainError::KeyedContinuationMismatch { .. })
        ));
        // A tampered window fails the head comparison with a named error.
        let mut tampered = window_two.clone();
        tampered[1].detail = "tampered".to_string();
        assert!(matches!(
            verify_against_keyed_anchor(
                "pilot-site-A",
                &key,
                Some(previous_head),
                &tampered,
                &anchor_two,
            ),
            Err(AuditChainError::KeyedHeadMismatch { .. })
        ));
        // A wrong site binding is refused.
        assert!(matches!(
            verify_against_keyed_anchor("other-site", &key, Some(previous_head), &window_two, &anchor_two),
            Err(AuditChainError::AnchorSiteMismatch { .. })
        ));
    }

    #[test]
    fn the_keyed_bundle_verifies_and_records_the_keyed_verdict() {
        let key = keyed_test_key();
        let bundle =
            KeyedAuditExportBundle::new("pilot-site-A", &key, None, vec![record(1, "a"), record(2, "b")], 1_700_000_000_500)
                .expect("builds");
        verify_keyed_bundle("pilot-site-A", &key, None, &bundle).expect("verifies");
        let evidence = verify_keyed_bundle_to_evidence("pilot-site-A", &key, None, &bundle)
            .expect("records");
        assert!(evidence.verdict_text.contains("DEPLOYMENT REQUIREMENT"));
        assert!(evidence.verdict_text.contains("never a release signature"));
        assert!(evidence.verdict_text.contains("NOT tamper-PROOF"));
        // A wrong format id is refused.
        assert!(matches!(
            verify_keyed_bundle(
                "pilot-site-A",
                &key,
                None,
                &KeyedAuditExportBundle {
                    format: "other-format",
                    site_id: bundle.site_id.clone(),
                    anchor: bundle.anchor.clone(),
                    records: bundle.records.clone(),
                },
            ),
            Err(AuditChainError::BundleFormatMismatch { .. })
        ));
    }
}
