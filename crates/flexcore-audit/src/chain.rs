// P11.15 — the production-grade audit chain (the P11-E-1 DEFAULT, option a):
//         a keyed HMAC-SHA-256 linear chain over the §10.1 `AuditRecord`,
//         with a per-tenant chain namespace and the periodic anchor-export
//         design.
//
// DEPLOYMENT REQUIREMENT (program conventions §6.6 — read this first):
//
// This module is IMPLEMENTED CODE, not a deployed or exercised system. It
// has never been compiled, keyed with real key material, or run against a
// live audit trail (the entire workspace is WRITTEN-UNRUN; the loop bans of
// `docs/phases-11-20/01_PROGRAM_CONVENTIONS.md` §1 forbid every form of
// execution). Key management — key generation, distribution, rotation,
// storage, and the operator procedure around them — is a HUMAN-GATED
// DEPLOYMENT concern recorded in the P11.15 decision record; this module
// consumes key material supplied by the deployment and never generates,
// loads, or stores any. Until an operator provisions reviewed keys and
// signs off, the production chain is NOT running; rows written without a
// configured key carry NULL chain columns and are verifiable only by the
// test-grade FNV-1a 64 path (`crates/edge-agent/src/audit_proof.rs`),
// which stays labeled TEST-GRADE per LR-203 and is NEVER a release
// signature or a security primitive.
//
// The construction (P11-E-1 DEFAULT option (a), per the phase plan §4):
//
// - LINEAR CHAIN: each record's digest is HMAC-SHA-256 under a chain key
//   over (previous head digest || canonical record bytes). The head commits
//   to the whole ordered history: any edit, deletion, reorder, or insertion
//   changes every subsequent digest. RFC 2104 defines the HMAC construction;
//   SHA-256 is FIPS 180-4 [S-88] (the industry hash-chained-audit-log
//   baseline — no FlexCore-specific security claim attaches to it).
// - PER-TENANT NAMESPACE: the chain actually used per record is derived
//   from the deployment's namespace key by HMAC domain separation
//   (`chain_key_for_tenant`): tenant chains are cryptographically
//   independent — a digest in one tenant's chain cannot be replayed into
//   another's, and per-tenant verification never needs another tenant's
//   records. Platform-level records (`tenant_id IS NULL`) have their own
//   namespace.
// - PERIODIC ANCHOR EXPORT (the designed, human-cadenced half): at a
//   DEPLOYMENT-CONFIGURED cadence (never coded here — the cadence is an
//   operator decision, like every retention/config value), the current
//   per-tenant head is exported as a [`TenantChainAnchor`] and retained by
//   a party other than the audit writer (the Phase 6 offline-isolation
//   discipline). Anchors chain across windows: each anchor names the
//   previous anchor's head as its continuation seed, so the exported
//   anchors form their own second-order chain a verifier can walk without
//   the records. Verification re-derives the per-tenant chain key, walks
//   the claimed records, and compares every anchor field; every mismatch
//   is a named [`ChainError`].
//
// WHY THE CHAIN IS KEYED (the honesty upgrade over the FNV-1a chain): the
// test-grade FNV chain has no secret — an adversary with write access to
// the audit store can recompute a self-consistent chain. A keyed chain
// moves that adversary's cost to obtaining the KEY: digests cannot be
// recomputed without the per-tenant chain key. The residual risk stays
// honest: key compromise (or a superuser who can rewrite both the rows AND
// the anchor store) defeats detection — this is tamper-EVIDENCE with a
// smaller attack surface, never tamper-PROOF, and never a certification or
// conformance claim of any kind.
//
// WHY THE CRYPTO IS HAND-WRITTEN (the recorded tradeoff, not a preference):
// the program conventions prefer std-only crypto primitives where feasible
// and this wave's file-ownership rules forbid new internal dependency
// edges and manifest edits. SHA-256 (FIPS 180-4) and HMAC (RFC 2104) are
// fully specified, small, and deterministic; this implementation is
// anchored to published test vectors (FIPS 180-4 examples; RFC 4231 test
// cases 1, 2, and 7 — the block-size-crossing key case) written below and
// verified only at the first human `cargo test`. ASSUMPTION: the
// implementation is correct per those vectors. An independent review of
// this module's crypto is part of the DEPLOYMENT REQUIREMENT gate — no
// production keying without it.
//
// Scope: supervisory orchestration evidence only. Nothing here touches
// dispatch authority, control states, or the simulation-only ceiling.

use crate::record::AuditRecord;
use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::ids::TenantId;

// --- Constants -----------------------------------------------------------------

/// The chain digest algorithm label. Versioned: a construction change is a
/// detectable format change, never a silent one (the versioned-scheme
/// discipline carried from the edge-agent anchor).
pub const AUDIT_CHAIN_ALGORITHM: &str = "hmac-sha256-chain-v1";

/// The chain/anchor format version.
pub const AUDIT_CHAIN_FORMAT_VERSION: u32 = 1;

/// The HMAC domain-separation prefix every derived input carries, so a
/// digest computed here cannot collide with a digest of the same bytes
/// under a different protocol role.
pub const AUDIT_CHAIN_DOMAIN: &[u8] = b"flexcore-audit-chain-v1";

/// The minimum namespace-key material length. RFC 2104 recommends a key at
/// least as long as the hash output (32 bytes for SHA-256); shorter keys
/// are refused rather than accepted silently.
pub const CHAIN_KEY_MIN_BYTES: usize = 32;

// --- std-only SHA-256 (FIPS 180-4) ---------------------------------------------

/// The FIPS 180-4 round constants: the first 32 bits of the fractional
/// parts of the cube roots of the first 64 primes.
const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// The FIPS 180-4 initial hash values: the first 32 bits of the fractional
/// parts of the square roots of the first 8 primes.
const SHA256_H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// Computes SHA-256 over `data` exactly as FIPS 180-4 specifies: pad to a
/// 56-mod-64 byte boundary with a 0x80 terminator and zeros, append the
/// 64-bit big-endian bit length, and compress each 512-bit block.
///
/// Visibility note (P18.05 — decision record
/// `docs/phases-11-20/phase-18-edge-fleet-lifecycle-ota/records/
/// 05_p18_05_edge_agent_update_extension.md` §2; applied at the P18.16
/// close, resolving record 28 §5 finding F-1): `pub` so the edge agent's
/// `Sha256UpdateVerifier` can CONSUME this implementation across the
/// crate boundary (`crates/edge-agent/src/update_session.rs`) — the
/// consume-don't-duplicate rule. Behavior unchanged.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h = SHA256_H0;
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
                .wrapping_add(SHA256_K[t])
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

// --- std-only HMAC-SHA-256 (RFC 2104) --------------------------------------------

const HMAC_BLOCK_SIZE: usize = 64;

/// Computes HMAC-SHA-256 exactly as RFC 2104 specifies for a 64-byte-block
/// hash: normalize the key (hash it first when it exceeds the block size),
/// pad to the block size, and hash `H(opad || H(ipad || message))`.
pub(crate) fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut block_key = [0u8; HMAC_BLOCK_SIZE];
    if key.len() > HMAC_BLOCK_SIZE {
        block_key[..32].copy_from_slice(&sha256(key));
    } else {
        block_key[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; HMAC_BLOCK_SIZE];
    let mut opad = [0x5cu8; HMAC_BLOCK_SIZE];
    for i in 0..HMAC_BLOCK_SIZE {
        ipad[i] ^= block_key[i];
        opad[i] ^= block_key[i];
    }
    let mut inner = Vec::with_capacity(HMAC_BLOCK_SIZE + message.len());
    inner.extend_from_slice(&ipad);
    inner.extend_from_slice(message);
    let inner_digest = sha256(&inner);
    let mut outer = Vec::with_capacity(HMAC_BLOCK_SIZE + 32);
    outer.extend_from_slice(&opad);
    outer.extend_from_slice(&inner_digest);
    sha256(&outer)
}

// --- The chain key ----------------------------------------------------------------

/// The deployment-supplied chain namespace key (32 bytes of derived key
/// material). This type holds a SECRET: it is never `Debug`- or
/// `Clone`-derivable (no derive that would copy or print the material), and
/// it best-effort zeroizes its bytes on drop (volatile writes; a garbage
/// collector-free language can still leave copies — the honest wording is
/// "best-effort", not "guaranteed").
///
/// The key is CONSUMED, never created: generation, distribution, rotation,
/// and storage are DEPLOYMENT REQUIREMENTS gated on human sign-off (the
/// P11.15 decision record; P11-E-1 stays OPEN). `from_bytes` derives the
/// fixed-width working key from arbitrary-length deployment material via
/// HMAC domain separation, so a deployment may supply any secret of at
/// least [`CHAIN_KEY_MIN_BYTES`] bytes.
pub struct ChainKey([u8; 32]);

impl ChainKey {
    /// Derives the working key from deployment-supplied material. Refuses
    /// material shorter than [`CHAIN_KEY_MIN_BYTES`] (fail closed — a short
    /// key is a deployment error, never a silent downgrade).
    pub fn from_bytes(material: &[u8]) -> Result<Self, ChainError> {
        if material.len() < CHAIN_KEY_MIN_BYTES {
            return Err(ChainError::KeyTooShort {
                length: material.len(),
                required: CHAIN_KEY_MIN_BYTES,
            });
        }
        let mut domain = Vec::with_capacity(AUDIT_CHAIN_DOMAIN.len() + 1 + material.len());
        domain.extend_from_slice(AUDIT_CHAIN_DOMAIN);
        domain.push(0);
        domain.extend_from_slice(material);
        Ok(Self(hmac_sha256(AUDIT_CHAIN_DOMAIN, &domain)))
    }

    /// Derives the per-tenant chain key: HMAC over the domain string, the
    /// namespace role ("tenant" with the tenant's 16 bytes, or "platform"
    /// for the platform-level namespace). Tenant chains are independent —
    /// a digest from one namespace cannot be replayed into another (the
    /// per-tenant namespace of the P11-E-1 DEFAULT).
    pub fn chain_key_for_namespace(&self, tenant: Option<TenantId>) -> Self {
        let mut message = Vec::with_capacity(AUDIT_CHAIN_DOMAIN.len() + 8 + 16);
        message.extend_from_slice(AUDIT_CHAIN_DOMAIN);
        message.push(0);
        match tenant {
            Some(t) => {
                message.extend_from_slice(b"tenant");
                message.push(0);
                message.extend_from_slice(&t.uuid().to_bytes());
            }
            None => {
                message.extend_from_slice(b"platform");
                message.push(0);
            }
        }
        Self(hmac_sha256(&self.0, &message))
    }
}

impl Drop for ChainKey {
    fn drop(&mut self) {
        // Best-effort zeroization (volatile writes defeat dead-store
        // elimination; copies the compiler may have made are out of scope
        // — the honest bound stated in the type's doc comment).
        for byte in self.0.iter_mut() {
            std::ptr::write_volatile(byte as *mut u8, 0);
        }
    }
}

// --- The canonical record encoding ----------------------------------------------

/// Appends one length-prefixed byte string (u64 little-endian length, then
/// the bytes) so field boundaries are unambiguous in the canonical form.
fn push_field(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

/// The marker byte for an absent optional field (a present field starts
/// with its length prefix, which can never be 0xFF…FF as a practical
/// length — the marker is unambiguous).
const ABSENT_FIELD: u8 = 0xFF;

fn push_optional_field(out: &mut Vec<u8>, value: Option<&str>) {
    match value {
        Some(text) => push_field(out, text.as_bytes()),
        None => out.push(ABSENT_FIELD),
    }
}

/// The canonical byte encoding of one audit record: every PERSISTED §10.1
/// field the record carries, in fixed order, length-prefixed. The chain
/// digests THIS encoding — adding a field to [`AuditRecord`] without
/// extending this encoding would silently narrow what the chain commits
/// to; the unit tests assert the encoding covers the full field set.
///
/// RECORDED LIMITATION (the honest boundary): the correction linkage is
/// folded as the original record's identifier ONLY — the correction
/// REASON is not part of the digest because it is not a persisted column
/// of the `audit_records` storage design (P1.12 §4.7 maps `corrects` to
/// `corrected_audit_id` alone); a digest over an unpersisted field could
/// never be recomputed from the stored rows, so it would be unverifiable.
/// The reason remains enforced at the record-construction layer (a
/// correction cannot exist without one) and travels in the record TYPE;
/// it is the storage design, not this chain, that does not retain it.
fn canonical_record_bytes(record: &AuditRecord) -> Vec<u8> {
    let mut out = Vec::with_capacity(512);
    push_field(&mut out, AUDIT_CHAIN_DOMAIN);
    out.push(AUDIT_CHAIN_FORMAT_VERSION as u8);
    push_field(&mut out, &record.audit_id.to_bytes());
    match record.tenant_id {
        Some(tenant) => push_field(&mut out, &tenant.uuid().to_bytes()),
        None => push_field(&mut out, b"platform"),
    }
    push_field(&mut out, record.actor.as_str().as_bytes());
    push_field(&mut out, record.action.as_str().as_bytes());
    push_field(&mut out, record.object.object_type.as_str().as_bytes());
    push_field(&mut out, record.object.object_id.as_str().as_bytes());
    push_optional_field(&mut out, record.before_ref.as_ref().map(|r| r.as_str()));
    push_optional_field(&mut out, record.after_ref.as_ref().map(|r| r.as_str()));
    match &record.decision {
        Some(decision) => {
            out.push(match decision.outcome {
                crate::record::DecisionOutcome::Allowed => 1,
                crate::record::DecisionOutcome::Denied => 2,
            });
            push_field(&mut out, decision.reason_reference.as_str().as_bytes());
        }
        None => out.push(ABSENT_FIELD),
    }
    push_optional_field(&mut out, record.evidence.as_ref().map(|r| r.as_str()));
    // The P0.09 timestamps as 16-byte little-endian i128 millis (the
    // contracts types expose `unix_millis()` as i128; no truncation).
    out.extend_from_slice(&record.event_time.unix_millis().to_le_bytes());
    out.extend_from_slice(&record.ingest_time.unix_millis().to_le_bytes());
    push_field(&mut out, &record.correlation_id.to_bytes());
    match &record.corrects {
        Some(correction) => {
            out.push(1);
            push_field(&mut out, &correction.original_audit_id().to_bytes());
        }
        None => out.push(ABSENT_FIELD),
    }
    out
}

// --- The chain step -------------------------------------------------------------

/// The domain-separated genesis head of a per-tenant chain: HMAC over the
/// domain string and a fixed genesis label. A chain that has not yet seen
/// a record is AT genesis — the first record's digest covers this head.
pub fn genesis_head(namespace_key: &ChainKey) -> [u8; 32] {
    let mut message = Vec::with_capacity(AUDIT_CHAIN_DOMAIN.len() + 16);
    message.extend_from_slice(AUDIT_CHAIN_DOMAIN);
    message.push(0);
    message.extend_from_slice(b"genesis");
    hmac_sha256(&namespace_key.0, &message)
}

/// Folds one record into the chain: HMAC under the namespace key over
/// (previous head || canonical record bytes). Deterministic; the previous
/// head being an input is what makes the head commit to the whole ordered
/// history.
pub fn chain_step(
    namespace_key: &ChainKey,
    previous_head: &[u8; 32],
    record: &AuditRecord,
) -> [u8; 32] {
    let mut message = Vec::with_capacity(32 + 512);
    message.extend_from_slice(previous_head);
    message.extend_from_slice(&canonical_record_bytes(record));
    hmac_sha256(&namespace_key.0, &message)
}

// --- Digest hex helpers ------------------------------------------------------------

/// Encodes a 32-byte digest as lower-case hex (the stored/anchor form).
pub fn hex_encode_digest(digest: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push(hex_digit(byte >> 4));
        out.push(hex_digit(byte & 0x0f));
    }
    out
}

fn hex_digit(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'a' + (nibble - 10)) as char,
    }
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Parses a 64-character hex digest back into bytes (fail closed on any
/// other shape — a malformed stored digest is a finding, never a guess).
pub fn parse_digest_hex(hex: &str) -> Result<[u8; 32], ChainError> {
    let bytes = hex.as_bytes();
    if bytes.len() != 64 {
        return Err(ChainError::DigestHexMalformed { length: bytes.len() });
    }
    let mut out = [0u8; 32];
    for (i, pair) in bytes.chunks_exact(2).enumerate() {
        let high = hex_value(pair[0]).ok_or(ChainError::DigestHexMalformed { length: 64 })?;
        let low = hex_value(pair[1]).ok_or(ChainError::DigestHexMalformed { length: 64 })?;
        out[i] = (high << 4) | low;
    }
    Ok(out)
}

// --- Verification -------------------------------------------------------------------

/// One per-tenant chain entry to verify: the stored row's sequence, the
/// stored digest, and the decoded record. Built by the persistence adapter
/// from the `audit_records` chain columns (migration 0019).
pub struct TenantChainEntry<'a> {
    /// The stored per-tenant chain sequence (1-based; migration 0019).
    pub chain_sequence: i64,
    /// The digest stored on the row, parsed via [`parse_digest_hex`].
    pub stored_digest: [u8; 32],
    /// The decoded audit record the row holds.
    pub record: &'a AuditRecord,
}

/// Walks a tenant's chain entries IN SEQUENCE ORDER and recomputes every
/// digest from genesis. Returns the recomputed final head. Fails with a
/// named [`ChainError`] on the first discrepancy — a verification never
/// reports "probably fine". An EMPTY entry set verifies trivially (a
/// tenant with no records has no chain to verify — this is not evidence of
/// anything, only the absence of records).
pub fn verify_tenant_chain(
    namespace_key: &ChainKey,
    entries: &[TenantChainEntry<'_>],
) -> Result<[u8; 32], ChainError> {
    let mut head = genesis_head(namespace_key);
    let mut previous_sequence: Option<i64> = None;
    for entry in entries {
        if let Some(previous) = previous_sequence {
            if entry.chain_sequence <= previous {
                return Err(ChainError::NonMonotonicSequence {
                    previous,
                    found: entry.chain_sequence,
                });
            }
        }
        let computed = chain_step(namespace_key, &head, entry.record);
        if computed != entry.stored_digest {
            return Err(ChainError::DigestMismatch {
                sequence: entry.chain_sequence,
            });
        }
        head = entry.stored_digest;
        previous_sequence = Some(entry.chain_sequence);
    }
    Ok(head)
}

// --- The periodic anchor export (the designed, human-cadenced half) ------------------

/// The per-tenant chain anchor computed at EXPORT time and retained by a
/// party other than the audit writer (the Phase 6 offline-isolation
/// discipline). Anchors chain across export windows: `continues_from_head`
/// names the previous window's anchor head, which seeded this window's
/// chain — the anchors alone form a second-order chain a verifier can walk
/// without the records.
///
/// THE CADENCE IS A DEPLOYMENT DECISION, never coded here: how often the
/// operator exports anchors, and where they are retained, are operator
/// configuration (the same discipline as every retention value). This type
/// and [`export_tenant_anchor`] are the export mechanism; the scheduler is
/// deliberately absent (WRITTEN, NOT DEPLOYED).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantChainAnchor {
    /// The anchor format version ([`AUDIT_CHAIN_FORMAT_VERSION`]).
    pub chain_version: u32,
    /// The digest algorithm ([`AUDIT_CHAIN_ALGORITHM`]).
    pub algorithm: &'static str,
    /// The tenant namespace the anchor commits to: the canonical hyphenated
    /// tenant UUID, or `None` for the platform-level namespace.
    pub tenant_id: Option<String>,
    /// The head the chain seeded from: the previous anchor's head, or
    /// `None` for the first window (genesis).
    pub continues_from_head: Option<String>,
    /// The final head digest of this window's chain (lower-case hex).
    pub head_digest: String,
    /// The first chain sequence in the window.
    pub first_sequence: i64,
    /// The last chain sequence in the window.
    pub last_sequence: i64,
    /// The number of records the anchor commits to.
    pub record_count: u64,
    /// When the anchor was computed (the INJECTED clock — never wall time).
    pub computed_at_unix_millis: i64,
}

impl TenantChainAnchor {
    /// The anchor summary line (audit/operator-pack text). Carries the
    /// honest DEPLOYMENT REQUIREMENT wording: this is tamper-EVIDENCE under
    /// reviewed key management, never a release signature.
    pub fn audit_text(&self) -> String {
        let tenant = match &self.tenant_id {
            Some(id) => id.as_str(),
            None => "platform",
        };
        format!(
            "tenant-chain-anchor v{} alg={} tenant={} continues_from={} head={} sequences=[{},{}] \
             count={} computed_at={} (keyed tamper-EVIDENCE; DEPLOYMENT REQUIREMENT: valid only \
             under reviewed key management, never a release signature)",
            self.chain_version,
            self.algorithm,
            tenant,
            self.continues_from_head.as_deref().unwrap_or("genesis"),
            self.head_digest,
            self.first_sequence,
            self.last_sequence,
            self.record_count,
            self.computed_at_unix_millis,
        )
    }
}

/// Computes the per-tenant anchor for one export window. The entries must
/// be in strictly increasing chain-sequence order and non-empty (an empty
/// window has nothing to commit to — refuse rather than export a vacuous
/// anchor). `continues_from_head` is the previous window's anchor head
/// (`None` for the first window): it seeds the recomputation, so an anchor
/// for a continuation window verifies ONLY against a chain that really
/// started at the named previous head.
pub fn export_tenant_anchor(
    namespace_key: &ChainKey,
    tenant: Option<TenantId>,
    continues_from_head: Option<[u8; 32]>,
    entries: &[TenantChainEntry<'_>],
    computed_at_unix_millis: i64,
) -> Result<TenantChainAnchor, ChainError> {
    if entries.is_empty() {
        return Err(ChainError::EmptyAnchorExport);
    }
    // A continuation window re-verifies from the named previous head, not
    // from genesis, so the anchor commits to the CONTINUOUS history.
    let mut head = match continues_from_head {
        Some(seed) => seed,
        None => genesis_head(namespace_key),
    };
    let mut previous_sequence: Option<i64> = None;
    for entry in entries {
        if let Some(previous) = previous_sequence {
            if entry.chain_sequence <= previous {
                return Err(ChainError::NonMonotonicSequence {
                    previous,
                    found: entry.chain_sequence,
                });
            }
        }
        let computed = chain_step(namespace_key, &head, entry.record);
        if computed != entry.stored_digest {
            return Err(ChainError::DigestMismatch {
                sequence: entry.chain_sequence,
            });
        }
        head = entry.stored_digest;
        previous_sequence = Some(entry.chain_sequence);
    }
    Ok(TenantChainAnchor {
        chain_version: AUDIT_CHAIN_FORMAT_VERSION,
        algorithm: AUDIT_CHAIN_ALGORITHM,
        tenant_id: tenant.map(|t| t.uuid().to_hyphenated()),
        continues_from_head: continues_from_head.map(|seed| hex_encode_digest(&seed)),
        head_digest: hex_encode_digest(&head),
        first_sequence: entries[0].chain_sequence,
        last_sequence: entries[entries.len() - 1].chain_sequence,
        record_count: entries.len() as u64,
        computed_at_unix_millis,
    })
}

/// Verifies a claimed set of per-tenant entries against a retained
/// [`TenantChainAnchor`]: version, algorithm, tenant namespace,
/// continuation seed, count, sequence range, and every digest. Every
/// mismatch is a named [`ChainError`].
pub fn verify_against_anchor(
    namespace_key: &ChainKey,
    tenant: Option<TenantId>,
    continues_from_head: Option<[u8; 32]>,
    entries: &[TenantChainEntry<'_>],
    anchor: &TenantChainAnchor,
) -> Result<(), ChainError> {
    // Format gates first (refused, never guessed at).
    if anchor.chain_version != AUDIT_CHAIN_FORMAT_VERSION {
        return Err(ChainError::AnchorVersionMismatch {
            expected: AUDIT_CHAIN_FORMAT_VERSION,
            found: anchor.chain_version,
        });
    }
    if anchor.algorithm != AUDIT_CHAIN_ALGORITHM {
        return Err(ChainError::AnchorAlgorithmMismatch {
            expected: AUDIT_CHAIN_ALGORITHM,
            found: anchor.algorithm.to_string(),
        });
    }
    let anchor_tenant_text = tenant.map(|t| t.uuid().to_hyphenated());
    if anchor.tenant_id != anchor_tenant_text {
        return Err(ChainError::AnchorTenantMismatch {
            expected: anchor_tenant_text,
            found: anchor.tenant_id.clone(),
        });
    }
    let seed_text = continues_from_head.map(|seed| hex_encode_digest(&seed));
    if anchor.continues_from_head != seed_text {
        return Err(ChainError::AnchorContinuationMismatch {
            expected: seed_text,
            found: anchor.continues_from_head.clone(),
        });
    }
    if entries.is_empty() {
        return Err(ChainError::AnchorCountMismatch {
            expected: anchor.record_count,
            computed: 0,
        });
    }
    let computed_count = entries.len() as u64;
    if computed_count != anchor.record_count {
        return Err(ChainError::AnchorCountMismatch {
            expected: anchor.record_count,
            computed: computed_count,
        });
    }
    let computed_first = entries[0].chain_sequence;
    let computed_last = entries[entries.len() - 1].chain_sequence;
    if computed_first != anchor.first_sequence || computed_last != anchor.last_sequence {
        return Err(ChainError::AnchorSequenceRangeMismatch {
            expected_first: anchor.first_sequence,
            expected_last: anchor.last_sequence,
            computed_first,
            computed_last,
        });
    }
    // Re-derive the head with the anchor's continuation seed and compare.
    let head = match continues_from_head {
        Some(seed) => seed,
        None => genesis_head(namespace_key),
    };
    let mut previous_sequence: Option<i64> = None;
    let mut running = head;
    for entry in entries {
        if let Some(previous) = previous_sequence {
            if entry.chain_sequence <= previous {
                return Err(ChainError::NonMonotonicSequence {
                    previous,
                    found: entry.chain_sequence,
                });
            }
        }
        let computed = chain_step(namespace_key, &running, entry.record);
        if computed != entry.stored_digest {
            return Err(ChainError::DigestMismatch {
                sequence: entry.chain_sequence,
            });
        }
        running = entry.stored_digest;
        previous_sequence = Some(entry.chain_sequence);
    }
    if hex_encode_digest(&running) != anchor.head_digest {
        return Err(ChainError::AnchorHeadMismatch {
            expected: anchor.head_digest.clone(),
            computed: hex_encode_digest(&running),
        });
    }
    Ok(())
}

// --- Errors -------------------------------------------------------------------------

/// Named errors of the keyed chain construction and verification. Every
/// failure is named — a verification NEVER reports "probably fine"; the
/// specific mismatch is the finding an operator reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainError {
    /// The deployment-supplied key material was shorter than
    /// [`CHAIN_KEY_MIN_BYTES`]. Fail closed.
    KeyTooShort {
        /// The material length that was supplied.
        length: usize,
        /// The minimum length ([`CHAIN_KEY_MIN_BYTES`]).
        required: usize,
    },
    /// A stored digest was not 64 hex characters.
    DigestHexMalformed {
        /// The length that was found.
        length: usize,
    },
    /// An anchor export was attempted over an empty window — refused (an
    /// empty anchor commits to nothing and is not evidence).
    EmptyAnchorExport,
    /// The entries are not in strictly increasing chain-sequence order —
    /// the disorder itself is a finding.
    NonMonotonicSequence {
        /// The previous sequence seen.
        previous: i64,
        /// The sequence that violated the order.
        found: i64,
    },
    /// The recomputed digest at this sequence differs from the stored one —
    /// the record content, the stored digest, or the order was altered.
    DigestMismatch {
        /// The chain sequence where the mismatch occurred.
        sequence: i64,
    },
    /// The retained anchor was computed under a different chain version —
    /// refused, never guessed at.
    AnchorVersionMismatch {
        /// The version this verifier computes.
        expected: u32,
        /// The version the anchor carries.
        found: u32,
    },
    /// The retained anchor was computed under a different algorithm —
    /// refused, never guessed at.
    AnchorAlgorithmMismatch {
        /// The algorithm this verifier computes.
        expected: &'static str,
        /// The algorithm the anchor carries.
        found: String,
    },
    /// The retained anchor binds a different tenant namespace — refused,
    /// never guessed at.
    AnchorTenantMismatch {
        /// The namespace verification was requested for.
        expected: Option<String>,
        /// The namespace the anchor binds.
        found: Option<String>,
    },
    /// The retained anchor's continuation seed differs from the claimed
    /// chain's seed — the window does not continue the claimed history.
    AnchorContinuationMismatch {
        /// The seed verification was requested with.
        expected: Option<String>,
        /// The seed the anchor names.
        found: Option<String>,
    },
    /// The record COUNT differs from the retained anchor's — records were
    /// deleted from (or forged into) the claimed window.
    AnchorCountMismatch {
        /// The count the anchor commits to.
        expected: u64,
        /// The count the claimed window holds.
        computed: u64,
    },
    /// The first/last chain sequence differs from the retained anchor's —
    /// a tail truncation or a padded window.
    AnchorSequenceRangeMismatch {
        /// The first sequence the anchor commits to.
        expected_first: i64,
        /// The last sequence the anchor commits to.
        expected_last: i64,
        /// The first sequence of the claimed window.
        computed_first: i64,
        /// The last sequence of the claimed window.
        computed_last: i64,
    },
    /// The recomputed head differs from the retained anchor's head.
    AnchorHeadMismatch {
        /// The head the anchor commits to.
        expected: String,
        /// The head the claimed window recomputes to.
        computed: String,
    },
}

impl std::fmt::Display for ChainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChainError::KeyTooShort { length, required } => write!(
                f,
                "the chain key material is {length} bytes; the keyed audit chain requires at \
                 least {required} bytes — refusing rather than silently downgrading"
            ),
            ChainError::DigestHexMalformed { length } => write!(
                f,
                "a stored chain digest is {length} characters; the canonical digest form is \
                 64 hex characters — the stored value is malformed and a finding"
            ),
            ChainError::EmptyAnchorExport => write!(
                f,
                "an empty export window commits to nothing — no anchor is exported"
            ),
            ChainError::NonMonotonicSequence { previous, found } => write!(
                f,
                "the chain entries are not in strictly increasing chain-sequence order \
                 (previous {previous}, found {found}) — the disorder itself is a finding"
            ),
            ChainError::DigestMismatch { sequence } => write!(
                f,
                "CHAIN DIGEST MISMATCH at chain sequence {sequence}: the recomputed digest \
                 differs from the stored one — the record content, the stored digest, or the \
                 order was altered (keyed tamper-EVIDENCE, not tamper-PROOF)"
            ),
            ChainError::AnchorVersionMismatch { expected, found } => write!(
                f,
                "the retained anchor carries chain version {found}; this verifier computes \
                 version {expected} — refused, never guessed at"
            ),
            ChainError::AnchorAlgorithmMismatch { expected, found } => write!(
                f,
                "the retained anchor carries algorithm '{found}'; this verifier computes \
                 '{expected}' — refused, never guessed at"
            ),
            ChainError::AnchorTenantMismatch { expected, found } => write!(
                f,
                "the retained anchor binds tenant namespace {:?}; verification was requested \
                 for {:?} — refused, never guessed at",
                found, expected
            ),
            ChainError::AnchorContinuationMismatch { expected, found } => write!(
                f,
                "the retained anchor names continuation seed {:?}; the claimed window seeds \
                 from {:?} — the window does not continue the claimed history",
                found, expected
            ),
            ChainError::AnchorCountMismatch { expected, computed } => write!(
                f,
                "ANCHOR RECORD COUNT MISMATCH: the retained anchor commits to {expected} \
                 records but the claimed window holds {computed} — records were deleted or \
                 forged"
            ),
            ChainError::AnchorSequenceRangeMismatch {
                expected_first,
                expected_last,
                computed_first,
                computed_last,
            } => write!(
                f,
                "ANCHOR SEQUENCE RANGE MISMATCH: the retained anchor spans \
                 [{expected_first}, {expected_last}] but the claimed window spans \
                 [{computed_first}, {computed_last}] — a tail truncation or padded window"
            ),
            ChainError::AnchorHeadMismatch { expected, computed } => write!(
                f,
                "ANCHOR HEAD MISMATCH: the retained anchor commits to head {expected} but the \
                 claimed window recomputes to {computed} — the window content does not match \
                 what was exported (keyed tamper-EVIDENCE, not tamper-PROOF)"
            ),
        }
    }
}

impl std::error::Error for ChainError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::{
        ActorIdentity, AuditAction, AuditObject, AuditRecordFields, DecisionOutcome,
        EvidenceReference, RecordReference,
    };
    use flexcore_contracts::time_model::{EventTime, IngestTime};

    fn sample_uuid(tag: u8) -> FlexUuid {
        let mut bytes = [0u8; 16];
        bytes[15] = tag;
        FlexUuid::from_bytes(bytes)
    }

    fn sample_record(tag: u8, tenant: Option<TenantId>) -> AuditRecord {
        AuditRecord::new(AuditRecordFields {
            audit_id: sample_uuid(tag),
            tenant_id: tenant,
            actor: ActorIdentity::new("operator-1".to_string()).expect("valid actor"),
            action: AuditAction::new("asset.register".to_string()).expect("valid action"),
            object: AuditObject::new("asset".to_string(), format!("asset-{tag}"))
                .expect("valid object"),
            before_ref: None,
            after_ref: Some(
                RecordReference::new(format!("asset-{tag}:v2")).expect("valid reference"),
            ),
            decision: None,
            evidence: None,
            event_time: EventTime::from_unix_millis(1_000).expect("valid event time"),
            ingest_time: IngestTime::from_unix_millis(2_000).expect("valid ingest time"),
            correlation_id: sample_uuid(3),
        })
        .expect("valid record")
    }

    fn test_namespace_key() -> ChainKey {
        // Test key material ONLY — never a real deployment key.
        ChainKey::from_bytes(&[0x42u8; 32]).expect("valid test key")
    }

    // --- FIPS 180-4 SHA-256 vectors (the standard's published examples) ---

    #[test]
    fn sha256_matches_the_fips_180_4_abc_vector() {
        let digest = sha256(b"abc");
        assert_eq!(
            hex_encode_digest(&digest),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sha256_matches_the_fips_180_4_empty_vector() {
        let digest = sha256(b"");
        assert_eq!(
            hex_encode_digest(&digest),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    // --- RFC 4231 HMAC-SHA-256 vectors (test cases 1, 2, 7) -------------

    #[test]
    fn hmac_matches_rfc_4231_test_case_1() {
        let digest = hmac_sha256(&[0x0bu8; 20], b"Hi There");
        assert_eq!(
            hex_encode_digest(&digest),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    #[test]
    fn hmac_matches_rfc_4231_test_case_2() {
        let digest = hmac_sha256(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(
            hex_encode_digest(&digest),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn hmac_matches_rfc_4231_test_case_7_block_size_key_hashing() {
        // RFC 4231 test case 7: a 131-byte key (larger than the SHA-256
        // block size) — exercises the hash-the-key-first path.
        let key = [0xaau8; 131];
        let data = b"This is a test using a larger than block-size key and a \
                     larger than block-size data. The key needs to be hashed \
                     before being used by the HMAC algorithm.";
        let digest = hmac_sha256(&key, data);
        assert_eq!(
            hex_encode_digest(&digest),
            "9b09ffa71b942fcb27635fbcd5b0e944bfdc63644f0713938a7f51535c3a35e2"
        );
    }

    // --- The chain and the per-tenant namespace --------------------------

    fn chained_entries<'a>(
        namespace_key: &ChainKey,
        records: &'a [AuditRecord],
    ) -> Vec<TenantChainEntry<'a>> {
        let mut head = genesis_head(namespace_key);
        let mut entries = Vec::with_capacity(records.len());
        for (i, record) in records.iter().enumerate() {
            let digest = chain_step(namespace_key, &head, record);
            head = digest;
            entries.push(TenantChainEntry {
                chain_sequence: (i + 1) as i64,
                stored_digest: digest,
                record,
            });
        }
        entries
    }

    #[test]
    fn the_chain_commits_to_content_order_and_count() {
        let tenant = TenantId::new(sample_uuid(2));
        let namespace_key = test_namespace_key().chain_key_for_namespace(Some(tenant));
        let records = vec![
            sample_record(1, Some(tenant)),
            sample_record(2, Some(tenant)),
            sample_record(3, Some(tenant)),
        ];
        let entries = chained_entries(&namespace_key, &records);
        verify_tenant_chain(&namespace_key, &entries).expect("verifies");

        // A content edit breaks the digest at the edited sequence.
        let mut tampered = vec![
            sample_record(1, Some(tenant)),
            sample_record(2, Some(tenant)),
            sample_record(3, Some(tenant)),
        ];
        tampered[1].object = AuditObject::new("asset".to_string(), "tampered".to_string())
            .expect("valid object");
        let tampered_entries = chained_entries(&namespace_key, &records);
        // Recompute the chain over the tampered content directly: the
        // stored digests from the honest chain no longer match.
        let mut head_recomputed = genesis_head(&namespace_key);
        let mut mismatch_sequence = 0i64;
        for (i, record) in tampered.iter().enumerate() {
            let digest = chain_step(&namespace_key, &head_recomputed, record);
            if digest != tampered_entries[i].stored_digest {
                mismatch_sequence = (i + 1) as i64;
                break;
            }
            head_recomputed = digest;
        }
        assert_eq!(mismatch_sequence, 2, "the tamper is caught at record 2");

        // A deletion changes the head AND the count (verify against an
        // anchor built from three records over a two-record window).
        let anchor = export_tenant_anchor(
            &namespace_key,
            Some(tenant),
            None,
            &entries,
            1_700_000_000_000,
        )
        .expect("exports");
        let deleted = chained_entries(&namespace_key, &records[..2]);
        assert!(matches!(
            verify_against_anchor(&namespace_key, Some(tenant), None, &deleted, &anchor),
            Err(ChainError::AnchorCountMismatch { .. })
        ));

        // A reorder is a non-monotonic sequence or a digest mismatch —
        // either way a named failure, never a pass.
        let reordered_records = vec![records[0].clone(), records[2].clone(), records[1].clone()];
        let reordered_entries = chained_entries(&namespace_key, &reordered_records);
        // The reordered window has honest-looking digests for ITS order,
        // but the sequence numbers disagree with the anchor's range only
        // if reordered after digesting; walk-verify catches the stored
        // digest vs recomputed mismatch for the swapped middle entries.
        let anchor_two = export_tenant_anchor(
            &namespace_key,
            Some(tenant),
            None,
            &entries,
            1_700_000_000_000,
        )
        .expect("exports");
        let reordered_then_swapped = vec![
            TenantChainEntry {
                chain_sequence: 1,
                stored_digest: reordered_entries[0].stored_digest,
                record: &reordered_records[0],
            },
            TenantChainEntry {
                chain_sequence: 2,
                stored_digest: entries[1].stored_digest,
                record: &reordered_records[1],
            },
            TenantChainEntry {
                chain_sequence: 3,
                stored_digest: entries[2].stored_digest,
                record: &reordered_records[2],
            },
        ];
        assert!(matches!(
            verify_against_anchor(
                &namespace_key,
                Some(tenant),
                None,
                &reordered_then_swapped,
                &anchor_two,
            ),
            Err(ChainError::DigestMismatch { sequence: 2 })
        ));

        // An empty set verifies trivially (no records, no chain).
        assert!(verify_tenant_chain(&namespace_key, &[]).is_ok());
    }

    #[test]
    fn a_non_monotonic_sequence_set_is_refused() {
        let tenant = TenantId::new(sample_uuid(2));
        let namespace_key = test_namespace_key().chain_key_for_namespace(Some(tenant));
        let records = vec![sample_record(1, Some(tenant)), sample_record(2, Some(tenant))];
        let entries = chained_entries(&namespace_key, &records);
        let swapped = vec![
            TenantChainEntry {
                chain_sequence: 2,
                stored_digest: entries[0].stored_digest,
                record: &entries[0].record,
            },
            TenantChainEntry {
                chain_sequence: 1,
                stored_digest: entries[1].stored_digest,
                record: &entries[1].record,
            },
        ];
        assert!(matches!(
            verify_tenant_chain(&namespace_key, &swapped),
            Err(ChainError::NonMonotonicSequence { .. })
        ));
    }

    #[test]
    fn tenant_namespaces_are_independent() {
        let tenant_a = TenantId::new(sample_uuid(2));
        let tenant_b = TenantId::new(sample_uuid(9));
        let namespace_key = test_namespace_key();
        let key_a = namespace_key.chain_key_for_namespace(Some(tenant_a));
        let key_b = namespace_key.chain_key_for_namespace(Some(tenant_b));
        let key_platform = namespace_key.chain_key_for_namespace(None);
        let record = sample_record(1, Some(tenant_a));
        let head_a = chain_step(&key_a, &genesis_head(&key_a), &record);
        let head_b = chain_step(&key_b, &genesis_head(&key_b), &record);
        let head_platform = chain_step(&key_platform, &genesis_head(&key_platform), &record);
        // The same record has different digests in different namespaces —
        // a digest cannot be replayed across tenants.
        assert_ne!(head_a, head_b);
        assert_ne!(head_a, head_platform);
        // A chain verified under the wrong namespace fails.
        let records = vec![sample_record(1, Some(tenant_a)), sample_record(2, Some(tenant_a))];
        let entries_a = chained_entries(&key_a, &records);
        assert!(matches!(
            verify_tenant_chain(&key_b, &entries_a),
            Err(ChainError::DigestMismatch { .. })
        ));
    }

    #[test]
    fn the_anchor_round_trips_through_verification() {
        let tenant = TenantId::new(sample_uuid(2));
        let namespace_key = test_namespace_key().chain_key_for_namespace(Some(tenant));
        let records = vec![sample_record(1, Some(tenant)), sample_record(2, Some(tenant))];
        let entries = chained_entries(&namespace_key, &records);
        let anchor = export_tenant_anchor(
            &namespace_key,
            Some(tenant),
            None,
            &entries,
            1_700_000_000_500,
        )
        .expect("exports");
        assert_eq!(anchor.chain_version, AUDIT_CHAIN_FORMAT_VERSION);
        assert_eq!(anchor.algorithm, AUDIT_CHAIN_ALGORITHM);
        assert_eq!(anchor.record_count, 2);
        assert_eq!(anchor.first_sequence, 1);
        assert_eq!(anchor.last_sequence, 2);
        assert!(anchor.continues_from_head.is_none());
        verify_against_anchor(&namespace_key, Some(tenant), None, &entries, &anchor)
            .expect("verifies");
        // The anchor text carries the honest wording.
        assert!(anchor.audit_text().contains("DEPLOYMENT REQUIREMENT"));
        assert!(anchor.audit_text().contains("never a release signature"));

        // A continuation window seeds from the previous head and carries it
        // in the anchor.
        let window_two_records = vec![
            sample_record(4, Some(tenant)),
            sample_record(5, Some(tenant)),
        ];
        let window_two = chained_entries(&namespace_key, &window_two_records)
            .into_iter()
            .enumerate()
            .map(|(i, mut entry)| {
                entry.chain_sequence = 3 + i as i64;
                entry
            })
            .collect::<Vec<_>>();
        let previous_head = verify_tenant_chain(&namespace_key, &entries).expect("verifies");
        let anchor_two = export_tenant_anchor(
            &namespace_key,
            Some(tenant),
            Some(previous_head),
            &window_two,
            1_700_000_001_000,
        )
        .expect("exports");
        assert_eq!(
            anchor_two.continues_from_head.as_deref(),
            Some(hex_encode_digest(&previous_head).as_str())
        );
        verify_against_anchor(
            &namespace_key,
            Some(tenant),
            Some(previous_head),
            &window_two,
            &anchor_two,
        )
        .expect("verifies");
        // The continuation gate: the same records without the seed are
        // refused — the window claims to continue a specific history.
        assert!(matches!(
            verify_against_anchor(&namespace_key, Some(tenant), None, &window_two, &anchor_two),
            Err(ChainError::AnchorContinuationMismatch { .. })
        ));
    }

    #[test]
    fn malformed_keys_and_digests_fail_closed() {
        assert!(matches!(
            ChainKey::from_bytes(&[0u8; 8]),
            Err(ChainError::KeyTooShort { length: 8, required: 32 })
        ));
        assert!(matches!(
            parse_digest_hex("not-a-digest"),
            Err(ChainError::DigestHexMalformed { .. })
        ));
        // A digest of the wrong length is malformed even when hex-clean.
        assert!(matches!(
            parse_digest_hex("abcd"),
            Err(ChainError::DigestHexMalformed { length: 4 })
        ));
    }

    #[test]
    fn the_canonical_encoding_covers_every_s10_1_field() {
        // The chain must commit to EVERY field the record carries: two
        // records differing in ANY §10.1 field produce different digests.
        let tenant = TenantId::new(sample_uuid(2));
        let namespace_key = test_namespace_key().chain_key_for_namespace(Some(tenant));
        let base = sample_record(1, Some(tenant));
        let base_digest = chain_step(&namespace_key, &genesis_head(&namespace_key), &base);

        // actor
        let mut variant = base.clone();
        variant.actor = ActorIdentity::new("operator-2".to_string()).expect("valid actor");
        assert_ne!(chain_step(&namespace_key, &genesis_head(&namespace_key), &variant), base_digest);
        // action
        let mut variant = base.clone();
        variant.action = AuditAction::new("asset.update".to_string()).expect("valid action");
        assert_ne!(chain_step(&namespace_key, &genesis_head(&namespace_key), &variant), base_digest);
        // object type and id
        let mut variant = base.clone();
        variant.object = AuditObject::new("site".to_string(), "asset-1".to_string())
            .expect("valid object");
        assert_ne!(chain_step(&namespace_key, &genesis_head(&namespace_key), &variant), base_digest);
        // before_ref (absent -> present)
        let mut variant = base.clone();
        variant.before_ref = Some(RecordReference::new("asset-1:v1".to_string()).expect("valid ref"));
        assert_ne!(chain_step(&namespace_key, &genesis_head(&namespace_key), &variant), base_digest);
        // decision (absent -> present)
        let mut variant = base.clone();
        variant.decision = Some(
            crate::record::AuditDecision::new(
                DecisionOutcome::Denied,
                "policy-ref-1".to_string(),
            )
            .expect("valid decision"),
        );
        assert_ne!(chain_step(&namespace_key, &genesis_head(&namespace_key), &variant), base_digest);
        // evidence
        let mut variant = base.clone();
        variant.evidence =
            Some(EvidenceReference::new("evidence-1".to_string()).expect("valid evidence ref"));
        assert_ne!(chain_step(&namespace_key, &genesis_head(&namespace_key), &variant), base_digest);
        // event_time
        let mut variant = base.clone();
        variant.event_time = EventTime::from_unix_millis(1_500).expect("valid event time");
        assert_ne!(chain_step(&namespace_key, &genesis_head(&namespace_key), &variant), base_digest);
        // ingest_time
        let mut variant = base.clone();
        variant.ingest_time = IngestTime::from_unix_millis(2_500).expect("valid ingest time");
        assert_ne!(chain_step(&namespace_key, &genesis_head(&namespace_key), &variant), base_digest);
        // correlation_id
        let mut variant = base.clone();
        variant.correlation_id = sample_uuid(7);
        assert_ne!(chain_step(&namespace_key, &genesis_head(&namespace_key), &variant), base_digest);
        // tenant namespace
        let mut variant = base.clone();
        variant.tenant_id = Some(TenantId::new(sample_uuid(5)));
        assert_ne!(chain_step(&namespace_key, &genesis_head(&namespace_key), &variant), base_digest);
        // audit_id
        let mut variant = base.clone();
        variant.audit_id = sample_uuid(8);
        assert_ne!(chain_step(&namespace_key, &genesis_head(&namespace_key), &variant), base_digest);
        // corrects (absent -> present)
        let variant = base
            .clone()
            .link_correction(sample_uuid(9), "wrong after-ref".to_string())
            .expect("valid correction");
        assert_ne!(chain_step(&namespace_key, &genesis_head(&namespace_key), &variant), base_digest);
    }
}
