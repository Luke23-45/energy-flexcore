// P7.33 — Audit-integrity proof: plugin audit records are TAMPER-EVIDENT
// (report §5.1, §5.7, §17.3, §17.5, §22.3; ledger `ledger_phase_7.md`
// §P7.33; decision record
// `ledger/phase-07-plugin-sdk/33_34_audit_integrity_trust_boundary.md`).
//
// THE MECHANISM (ledger §P7.33 step 1 — the mechanism is RECORDED, not just
// "records are written")
//
// Every plugin-path audit record the host issues — broker capability
// grants/uses/denials, enforcement verdicts, validation forwards, health
// transitions (the P7.11–P7.13/P7.20/P7.23 surfaces) — is chained into a
// per-plugin, per-host-session HASH CHAIN:
//
//     digest(n) = FNV1a64( canonical_encoding(session, chain_key,
//                                          sequence(n), prev_digest(n),
//                                          body fields) )
//     prev_digest(1) = GENESIS_DIGEST (= 0)
//     prev_digest(n) = digest(n-1)
//
// A verifier that walks the chain can name the FIRST BROKEN LINK of any
// mutation: a modified record fails its own digest self-check, an inserted
// or reordered record fails its `prev_digest` continuity check, a deleted
// record leaves a sequence gap, and a truncated tail is detected against
// the ANCHOR (the last head digest + count, which the platform retains —
// see the boundary statement below). The chain is built per plugin and per
// host session (`session_id` + `chain_key`), so chains from different
// sessions and plugins never alias.
//
// TEST-GRADE HONESTY (the signing.rs pattern; D-P4.28-1)
//
// The digest function is FNV-1a 64 — the repo's existing NON-cryptographic
// integrity-checksum pattern. It is an integrity CHECKSUM, not a security
// primitive: no cryptographic property is claimed, and no key material
// exists anywhere in this module (the task bans new crypto deps and keys).
// PRODUCTION HASH = DEPLOYMENT REQUIREMENT: before any non-simulation use,
// the chain digest MUST be replaced by a Stage 6/9-qualified cryptographic
// hash (e.g. SHA-256) with the same chaining structure. The canonical
// encoding is version-tagged (`flexcore-audit-chain-v1`) so a later digest
// swap is a deliberate, detectable format change.
//
// THE BOUNDARY (hash-chaining proves TAMPER-EVIDENCE, not TAMPER-PROOF)
//
// An attacker who controls the host process can rebuild the whole chain
// (recompute every digest, re-anchor). See [`AUDIT_INTEGRITY_BOUNDARY`]:
// authoritative audit integrity lives in the PLATFORM's append-only audit
// store (migration 0008: INSERT/SELECT-only grants + the unconditional
// append-only trigger + RLS; the P4.36 SELECT-only read contract), and the
// plugin-path chain is the LOCAL evidence layer. The kernel's in-module
// `AuditSink` ports (lifecycle.rs, signing.rs) do NOT chain — that seam is
// recorded in the decision record (the kernel crate was read-only this
// wave); this module covers the HOST-ISSUED records.
//
// AUDIT-OR-REFUSE PRESERVED (the P4.28/P4.30 discipline; broker.rs header)
//
// [`ChainedAuditSink`] decorates any [`BrokerAuditSink`]: the event is
// forwarded to the inner (durable-side) sink FIRST; only a successful
// inner write is chained. A failed inner write returns
// [`BrokerError::AuditUnavailable`] and the local chain gains NOTHING —
// the chain never leads the durable store, so a verifier comparing the
// chain against the anchors sees exactly what was acknowledged.
//
// House rules: std only (no new dependency), no `unwrap`/`expect` on
// untrusted input, named errors, simulation-only.

use std::collections::BTreeMap;
use std::fmt;

use crate::broker::{BrokerAuditEvent, BrokerAuditSink, BrokerError};

// --- The boundary statement (asserted as documentation by the P7.33 tests) -----

/// The audit-integrity boundary statement (ledger §P7.33; decision record
/// §3). The P7.33 tests assert its content so it cannot silently drift.
pub const AUDIT_INTEGRITY_BOUNDARY: &str = concat!(
    "Hash-chaining proves tamper-EVIDENCE, not tamper-PROOF: an attacker who ",
    "controls the host process can rebuild the whole chain. Authoritative ",
    "audit integrity lives in the platform's append-only audit store ",
    "(PostgreSQL `audit_records`, migration 0008: INSERT/SELECT-only grants, ",
    "the unconditional append-only trigger, and tenant-isolation RLS; the ",
    "SELECT-only read contract `flexcore-storage/src/audit_read.rs`). The ",
    "plugin-path hash chain is the LOCAL evidence layer: it detects any ",
    "mutation, insertion, reordering, or deletion inside the host's own ",
    "audit sequence against the platform-retained anchor, but it does not ",
    "survive an attacker who controls the host AND the anchor. The kernel's ",
    "in-module AuditSink ports (lifecycle, signing) do not chain — recorded ",
    "seam; this chain covers host-issued records. TEST-GRADE digest: ",
    "FNV-1a 64; production hash = DEPLOYMENT REQUIREMENT."
);

// --- The digest (TEST-GRADE — see the module header) -----------------------------

/// FNV-1a 64 offset basis.
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
/// FNV-1a 64 prime.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// The TEST-GRADE chain digest: FNV-1a 64 over the canonical encoding.
/// NON-cryptographic (module header; D-P4.28-1 pattern) — labelled
/// TEST-GRADE everywhere it matters; the production hash is a DEPLOYMENT
/// REQUIREMENT and must preserve this chaining structure.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// The `prev_digest` of the FIRST record of every chain.
pub const GENESIS_DIGEST: u64 = 0;

/// The canonical-encoding version tag. A digest-function or field-order
/// change MUST change this tag (a deliberate, detectable format change).
pub const CANONICAL_ENCODING_VERSION: &str = "flexcore-audit-chain-v1";

/// Field separator of the canonical encoding (ASCII unit separator — a
/// control character that must not appear inside audit fields; the broker's
/// audit details are identifier/scope renderings).
const FIELD_SEPARATOR: char = '\u{1f}';

/// Appends one length-prefixed field to the canonical encoding. Length
/// prefixing makes the encoding injective (unambiguous) even if a field
/// ever contained the separator.
fn push_field(encoding: &mut String, field: &str) {
    encoding.push_str(&field.len().to_string());
    encoding.push(':');
    encoding.push_str(field);
}

// --- The chained record ----------------------------------------------------------

/// One record of the plugin-path audit hash chain. The BODY fields mirror
/// [`BrokerAuditEvent`] (the broker's closed audit shape); the CHAIN fields
/// (`session_id`, `chain_key`, `sequence`, `prev_digest`, `digest`) are the
/// tamper-evidence envelope.
///
/// Data record (like [`BrokerAuditEvent`]): public fields, no secret
/// material can appear in any of them (the broker's audit detail rule).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditChainRecord {
    /// The host session this record belongs to (one chain space per host
    /// session; records never chain across sessions).
    pub session_id: String,
    /// The chain key: the plugin the record concerns, or [`HOST_CHAIN_KEY`]
    /// for host-level events that name no plugin (e.g. an operator-initiated
    /// grant revocation).
    pub chain_key: String,
    /// The 1-based position of this record WITHIN its chain.
    pub sequence: u64,
    /// The digest of the previous record of the same chain
    /// ([`GENESIS_DIGEST`] for sequence 1).
    pub prev_digest: u64,
    /// The digest of THIS record's canonical encoding (self-check).
    pub digest: u64,
    // Body fields (mirror of BrokerAuditEvent):
    /// When the underlying event was recorded (Unix ms, host-supplied).
    pub recorded_at_unix_ms: u64,
    /// The stable audit-kind name (`BrokerAuditKind::name`).
    pub kind_name: String,
    /// The capability id concerned (empty where not applicable).
    pub capability: String,
    /// Human-readable detail (identifiers/scope renderings; never secrets).
    pub detail: String,
}

impl AuditChainRecord {
    /// Builds the next chain record from a broker audit event. `prev_digest`
    /// is the current head of the (session, chain_key) chain.
    pub fn from_event(
        session_id: &str,
        chain_key: &str,
        sequence: u64,
        prev_digest: u64,
        event: &BrokerAuditEvent,
    ) -> AuditChainRecord {
        let record = AuditChainRecord {
            session_id: session_id.to_string(),
            chain_key: chain_key.to_string(),
            sequence,
            prev_digest,
            digest: 0, // replaced by `with_digest` below
            recorded_at_unix_ms: event.recorded_at_unix_ms,
            kind_name: event.kind.name().to_string(),
            capability: event.capability.clone(),
            detail: event.detail.clone(),
        };
        record.with_digest()
    }

    /// Returns a copy with `digest` set to the FNV-1a 64 of the canonical
    /// encoding (the chaining step).
    fn with_digest(mut self) -> AuditChainRecord {
        self.digest = fnv1a64(self.canonical_encoding().as_bytes());
        self
    }

    /// The canonical encoding: version tag + length-prefixed fields in a
    /// FIXED order (chain envelope first, then body). Deterministic and
    /// injective (length prefixes); the same record always encodes to the
    /// same bytes, so any mutation of any field changes the digest.
    pub fn canonical_encoding(&self) -> String {
        let mut encoding = String::new();
        encoding.push_str(CANONICAL_ENCODING_VERSION);
        for field in [
            self.session_id.as_str(),
            self.chain_key.as_str(),
            &self.sequence.to_string(),
            &self.prev_digest.to_string(),
            &self.recorded_at_unix_ms.to_string(),
            self.kind_name.as_str(),
            self.capability.as_str(),
            self.detail.as_str(),
        ] {
            encoding.push(FIELD_SEPARATOR);
            push_field(&mut encoding, field);
        }
        encoding
    }

    /// Recomputes the digest from the canonical encoding (the verifier's
    /// self-check).
    pub fn recomputed_digest(&self) -> u64 {
        fnv1a64(self.canonical_encoding().as_bytes())
    }
}

/// The chain key for host-level events that name no plugin (the broker's
/// grant-revocation events initiated by an operator carry an empty
/// `plugin_id`; they still must be chained — under the host's own key).
pub const HOST_CHAIN_KEY: &str = "host";

/// Resolves the chain key of an event: the plugin id, or [`HOST_CHAIN_KEY`]
/// when the event names no plugin.
pub fn chain_key_for(plugin_id: &str) -> String {
    let trimmed = plugin_id.trim();
    if trimmed.is_empty() {
        HOST_CHAIN_KEY.to_string()
    } else {
        trimmed.to_string()
    }
}

// --- The chained writer ----------------------------------------------------------

/// One chain's head (the ANCHOR material the platform retains).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainHead {
    /// The last chained sequence number of this chain (0 = empty chain).
    pub last_sequence: u64,
    /// The digest of the record at `last_sequence` ([`GENESIS_DIGEST`] for
    /// an empty chain).
    pub head_digest: u64,
}

struct ChainState {
    /// The append-only local chain (insertion order).
    records: Vec<AuditChainRecord>,
    /// Per (session_id, chain_key) head state.
    heads: BTreeMap<(String, String), ChainHead>,
}

/// The chained audit-sink writer: a [`BrokerAuditSink`] decorator that
/// hashes every event into the per-plugin, per-session hash chain BEFORE
/// the caller sees `Ok`, and forwards the event to the inner sink FIRST
/// (audit-or-refuse: the broker upstream refuses on any `Err`, and the
/// chain never gains a record the durable side did not acknowledge).
///
/// This is the LOCAL evidence layer of [`AUDIT_INTEGRITY_BOUNDARY`] — the
/// durable append-only store remains the platform's (reached over the
/// recorded P7.20 gRPC seam; the in-memory inner sink here is this wave's
/// composition/test stand-in, the same role `InMemoryBrokerAuditSink`
/// already plays in `broker.rs`).
pub struct ChainedAuditSink {
    session_id: String,
    inner: Box<dyn BrokerAuditSink>,
    state: std::sync::Mutex<ChainState>,
}

impl ChainedAuditSink {
    /// Composes the chained writer over an inner sink. `session_id` scopes
    /// every chain this writer builds (one host session = one chain space);
    /// an empty session id is refused at composition (fail-closed — a chain
    /// without a session boundary could alias across host restarts).
    pub fn new(
        session_id: &str,
        inner: Box<dyn BrokerAuditSink>,
    ) -> Result<ChainedAuditSink, BrokerError> {
        if session_id.trim().is_empty() {
            return Err(BrokerError::AuditUnavailable {
                reason: "the chained audit sink requires a non-empty host session id \
                         (fail-closed: chains never alias across host sessions)"
                    .to_string(),
            });
        }
        Ok(ChainedAuditSink {
            session_id: session_id.to_string(),
            inner,
            state: std::sync::Mutex::new(ChainState {
                records: Vec::new(),
                heads: BTreeMap::new(),
            }),
        })
    }

    /// A snapshot of the local chain records (insertion order) — the
    /// verifier's input in tests and composition inspection.
    pub fn chain_records(&self) -> Vec<AuditChainRecord> {
        match self.state.lock() {
            Ok(state) => state.records.clone(),
            Err(_poisoned) => Vec::new(),
        }
    }

    /// The current head of every chain (the ANCHOR material: in composition
    /// the platform retains a copy of these after each flush so a truncated
    /// or rebuilt local chain is detectable — see [`verify_with_anchors`]).
    pub fn anchors(&self) -> Vec<ChainAnchor> {
        match self.state.lock() {
            Ok(state) => state
                .heads
                .iter()
                .map(|((session, key), head)| ChainAnchor {
                    session_id: session.clone(),
                    chain_key: key.clone(),
                    last_sequence: head.last_sequence,
                    head_digest: head.head_digest,
                })
                .collect(),
            Err(_poisoned) => Vec::new(),
        }
    }

    fn lock_state(&self) -> Result<std::sync::MutexGuard<'_, ChainState>, BrokerError> {
        self.state.lock().map_err(|_| BrokerError::AuditUnavailable {
            reason: "the chained audit sink's state lock is poisoned (the chain refuses \
                     to proceed past a broken state; audit-or-refuse)"
                .to_string(),
        })
    }
}

impl BrokerAuditSink for ChainedAuditSink {
    /// Forwards the event to the inner sink FIRST, then chains it. A failed
    /// inner write propagates as [`BrokerError::AuditUnavailable`] and the
    /// chain gains NOTHING (the local evidence layer never leads the
    /// durable store). The only post-forward failure is a poisoned state
    /// lock — also [`BrokerError::AuditUnavailable`], surfaced honestly
    /// (the durable store then holds a record the local chain does not; a
    /// verifier with anchors flags the divergence — the safe direction).
    fn record(&self, event: BrokerAuditEvent) -> Result<(), BrokerError> {
        // Forward first (audit-or-refuse, the broker.rs discipline).
        self.inner.record(event.clone())?;
        let key = chain_key_for(&event.plugin_id);
        let mut state = self.lock_state()?;
        let head = state
            .heads
            .entry((self.session_id.clone(), key.clone()))
            .or_insert(ChainHead {
                last_sequence: 0,
                head_digest: GENESIS_DIGEST,
            });
        let sequence = head.last_sequence + 1;
        let record = AuditChainRecord::from_event(
            &self.session_id,
            &key,
            sequence,
            head.head_digest,
            &event,
        );
        head.last_sequence = sequence;
        head.head_digest = record.digest;
        state.records.push(record);
        Ok(())
    }
}

// --- The verifier ------------------------------------------------------------------

/// The platform-retained anchor of one chain (a snapshot of [`ChainHead`]
/// with its identity). Truncation and tail rebuilds are detectable ONLY
/// against such an external anchor — a pure hash chain cannot detect the
/// deletion of its own tail (recorded in the boundary statement).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainAnchor {
    pub session_id: String,
    pub chain_key: String,
    /// The anchored last sequence number.
    pub last_sequence: u64,
    /// The anchored head digest.
    pub head_digest: u64,
}

/// The FIRST broken link of a failed verification, named (ledger §P7.33:
/// "a verifier … that walks the chain and names the first broken link").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainBreak {
    /// The session of the broken chain.
    pub session_id: String,
    /// The broken chain's key.
    pub chain_key: String,
    /// The sequence number of the first offending record (the expected
    /// sequence when a gap was found).
    pub sequence: u64,
    /// Why the link is broken (record mutated / predecessor mismatch /
    /// sequence gap).
    pub reason: String,
}

impl fmt::Display for ChainBreak {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "broken audit-chain link: session '{}' chain '{}' sequence {}: {}",
            self.session_id, self.chain_key, self.sequence, self.reason
        )
    }
}

/// One detected truncation/extension against an anchor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainTruncation {
    pub session_id: String,
    pub chain_key: String,
    /// The anchored last sequence.
    pub expected_last_sequence: u64,
    /// The actual last sequence of the walked chain.
    pub actual_last_sequence: u64,
    /// Why this is a truncation (tail records missing) or an extension
    /// (records appeared after the anchor — a post-anchor append is only
    /// legitimate when the platform re-anchored).
    pub reason: String,
}

/// One verified chain head (what the verifier confirmed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedChainHead {
    pub session_id: String,
    pub chain_key: String,
    pub last_sequence: u64,
    pub head_digest: u64,
}

/// The result of walking the chains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainVerification {
    /// The first broken link, if any (deterministic order: sorted by
    /// (session, chain key, sequence)).
    pub broken: Option<ChainBreak>,
    /// Truncations/extensions found against the supplied anchors.
    pub truncations: Vec<ChainTruncation>,
    /// The head of every chain that walked clean up to its end (or to its
    /// first break — a broken chain's head is NOT reported here).
    pub chains: Vec<VerifiedChainHead>,
}

impl ChainVerification {
    /// True when every chain walked intact AND matched every anchor.
    pub fn is_intact(&self) -> bool {
        self.broken.is_none() && self.truncations.is_empty()
    }
}

/// Walks the records and names the FIRST BROKEN LINK, or `None` when every
/// chain is internally intact. Records are walked IN THE ORDER PRESENTED
/// (the append order of the evidence store — the local `Vec` order), grouped
/// by (session, chain key); within a chain the sequences must be encountered
/// in strictly ascending consecutive order. Checks, per chain:
///
/// 1. **sequence continuity in encounter order** — the first record of a
///    chain is sequence 1 and each next record of that chain is the previous
///    sequence + 1; a gap names the missing position (deletion), a
///    regressed sequence names the reorder;
/// 2. **digest self-check** — each record's `digest` equals the FNV-1a 64
///    of its canonical encoding (ANY mutated field fails here);
/// 3. **prev-digest continuity** — each record's `prev_digest` equals the
///    previous record's `digest` (insertion, reordering, or a mutated
///    predecessor fails here).
///
/// The verifier deliberately does NOT re-sort by sequence: sorting would
/// silently undo a reordering attack before any check ran. A compliant
/// evidence store exposes records in append order.
///
/// An intact walk does NOT prove tamper-PROOFNESS (the boundary statement):
/// an attacker who controls the host can rebuild a self-consistent chain —
/// which is why [`verify_with_anchors`] exists.
pub fn first_broken_link(records: &[AuditChainRecord]) -> Option<ChainBreak> {
    verify_with_anchors(records, &[]).broken
}

/// Full verification: the chain walk of [`first_broken_link`] PLUS the
/// anchor comparison that detects truncation (and post-anchor extension).
/// The anchors are the platform's retained copies of the chain heads (in
/// tests: [`ChainedAuditSink::anchors`] snapshots taken before tampering).
pub fn verify_with_anchors(
    records: &[AuditChainRecord],
    anchors: &[ChainAnchor],
) -> ChainVerification {
    // Group records per (session, chain key), then walk each chain in
    // ascending sequence order.
    let mut per_chain: BTreeMap<(String, String), Vec<&AuditChainRecord>> = BTreeMap::new();
    for record in records {
        per_chain
            .entry((record.session_id.clone(), record.chain_key.clone()))
            .or_default()
            .push(record);
    }

    let mut broken: Option<ChainBreak> = None;
    let mut chains = Vec::new();
    for ((session_id, chain_key), chain_records) in per_chain {
        // NO re-sort: records are walked in encounter order (see
        // [`first_broken_link`] — sorting would silently undo a
        // reordering attack).
        let mut expected_prev = GENESIS_DIGEST;
        let mut expected_sequence: u64 = 1;
        for record in &chain_records {
            // 1. sequence continuity.
            if record.sequence != expected_sequence {
                broken = Some(ChainBreak {
                    session_id: session_id.clone(),
                    chain_key: chain_key.clone(),
                    sequence: expected_sequence,
                    reason: format!(
                        "sequence discontinuity: expected sequence {expected_sequence}, \
                         found {} (a record was deleted or reordered)",
                        record.sequence
                    ),
                });
                break;
            }
            // 2. digest self-check (any mutated field fails here).
            let recomputed = record.recomputed_digest();
            if recomputed != record.digest {
                broken = Some(ChainBreak {
                    session_id: session_id.clone(),
                    chain_key: chain_key.clone(),
                    sequence: record.sequence,
                    reason: format!(
                        "record digest does not match its canonical encoding \
                         (stored {:#018x}, recomputed {:#018x} — the record was mutated)",
                        record.digest, recomputed
                    ),
                });
                break;
            }
            // 3. prev-digest continuity.
            if record.prev_digest != expected_prev {
                broken = Some(ChainBreak {
                    session_id: session_id.clone(),
                    chain_key: chain_key.clone(),
                    sequence: record.sequence,
                    reason: format!(
                        "prev_digest does not match the digest of the previous record \
                         (stored {:#018x}, expected {:#018x} — the record was inserted, \
                         reordered, or its predecessor mutated)",
                        record.prev_digest, expected_prev
                    ),
                });
                break;
            }
            expected_prev = record.digest;
            expected_sequence += 1;
        }
        if broken.is_some() {
            break;
        }
        let last = chain_records
            .last()
            .map(|record| VerifiedChainHead {
                session_id: session_id.clone(),
                chain_key: chain_key.clone(),
                last_sequence: record.sequence,
                head_digest: record.digest,
            })
            .unwrap_or_else(|| VerifiedChainHead {
                session_id: session_id.clone(),
                chain_key: chain_key.clone(),
                last_sequence: 0,
                head_digest: GENESIS_DIGEST,
            });
        chains.push(last);
    }

    // Anchor comparison: every anchor must match a walked chain head (or a
    // walked-empty chain). Anchors for chains that broke are moot (the
    // break is reported first).
    let mut truncations = Vec::new();
    if broken.is_none() {
        for anchor in anchors {
            let actual = chains
                .iter()
                .find(|head| {
                    head.session_id == anchor.session_id && head.chain_key == anchor.chain_key
                })
                .cloned()
                .unwrap_or_else(|| VerifiedChainHead {
                    session_id: anchor.session_id.clone(),
                    chain_key: anchor.chain_key.clone(),
                    last_sequence: 0,
                    head_digest: GENESIS_DIGEST,
                });
            if actual.last_sequence != anchor.last_sequence {
                let (reason, expected, actual_seq) = if actual.last_sequence
                    < anchor.last_sequence
                {
                    (
                        format!(
                            "truncation: the chain ends at sequence {} but the anchor \
                             holds sequence {} (tail records were deleted)",
                            actual.last_sequence, anchor.last_sequence
                        ),
                        anchor.last_sequence,
                        actual.last_sequence,
                    )
                } else {
                    (
                        format!(
                            "extension: the chain holds sequence {} but the anchor \
                             holds sequence {} (records appeared after the anchor was \
                             taken; legitimate only if the platform re-anchored)",
                            actual.last_sequence, anchor.last_sequence
                        ),
                        anchor.last_sequence,
                        actual.last_sequence,
                    )
                };
                truncations.push(ChainTruncation {
                    session_id: anchor.session_id.clone(),
                    chain_key: anchor.chain_key.clone(),
                    expected_last_sequence: expected,
                    actual_last_sequence: actual_seq,
                    reason,
                });
            } else if actual.head_digest != anchor.head_digest {
                // Same length, different head: the tail record was replaced
                // by a rebuilt one whose digest the anchor does not know.
                // (A mutated tail record already fails the self-check; this
                // catches a REBUILT tail that is internally consistent.)
                truncations.push(ChainTruncation {
                    session_id: anchor.session_id.clone(),
                    chain_key: anchor.chain_key.clone(),
                    expected_last_sequence: anchor.last_sequence,
                    actual_last_sequence: actual.last_sequence,
                    reason: format!(
                        "head mismatch at the anchored sequence {}: the anchor holds \
                         {:#018x}, the chain holds {:#018x} (the tail was rebuilt)",
                        anchor.last_sequence, anchor.head_digest, actual.head_digest
                    ),
                });
            }
        }
    }

    ChainVerification {
        broken,
        truncations,
        chains,
    }
}
