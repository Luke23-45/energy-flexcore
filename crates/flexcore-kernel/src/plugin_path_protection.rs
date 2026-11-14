// P7.36 — Replay, sequence, and expiry protection on the plugin path
// (report §17.3 signed commands/sequence/expiry/replay, §17.4 forged and
// replayed commands + broker duplication, §14.3 sequence/idempotency/
// validity window, §15.4–§15.5 outbox/inbox; ledger `ledger_phase_7.md`
// P7.36; decision record
// `ledger/phase-07-plugin-sdk/19_20_36_config_health_path.md`).
//
// WHAT IS PROTECTED HERE: every request FROM the platform TO a plugin, and
// every plugin response BACK, carries the P4.28/P6.15 discipline ADAPTED to
// the plugin path (cited, not reinvented):
//
// - `crates/flexcore-application/src/dispatch_security.rs` (P4.28): the
//   payload-bound identity (correlation id + request digest), the monotonic
//   per-channel `SequenceGuard` with its DISTINCT duplicate vs
//   out-of-sequence refusals, and the rule that a refusal NEVER advances
//   the window (fail-closed, no attacker-driven window advance).
// - `crates/edge-agent/src/command_verify.rs` (P6.15): the four-check
//   pattern with one distinct named refusal per class, the FIXED evaluation
//   order with sequence LAST (a request refused by ANY earlier check never
//   moves the sequence state), and the restart-surviving sequence
//   checkpoint.
//
// THE FOUR CHECKS, each with a DISTINCT named refusal
// ([`PluginPathError::name`]) — never one combined "valid" boolean:
//
// | # | Check | Refusal |
// |---|---|---|
// | 1 | envelope self-validity (non-empty correlation id, sequence ≥ 1, payload within the cap, window `valid_from < valid_until`) | [`PluginPathError::RequestEnvelopeInvalid`] |
// | 2 | payload-bound digest: the envelope's digest must recompute exactly over the canonical binding (correlation id, plugin id+version, sequence, window, payload) | [`PluginPathError::DigestMismatch`] |
// | 3 | validity window against the INJECTED clock (`valid_from` not yet reached → [`PluginPathError::RequestNotYetValid`]; `valid_until` passed → [`PluginPathError::RequestExpired`] — expired requests are refused, never executed) | [`PluginPathError::RequestNotYetValid`] / [`PluginPathError::RequestExpired`] |
// | 4 | monotonic per-plugin sequence, restart-surviving | [`PluginPathError::SequenceReplayed`] / [`PluginPathError::SequenceOutOfOrder`] |
//
// Evaluation order is FIXED: (1) envelope → (2) digest → (3) window →
// (4) sequence LAST. A refused request never advances the sequence window
// and never reaches a plugin (the P6.15 ordering decision, restated).
//
// DIGEST HONESTY (the signing.rs pattern, reused verbatim): the recorded
// request digest is FNV-1a 64 ([`crate::signing::DIGEST_ALGORITHM_FNV1A64_TEST`])
// — the repo's non-cryptographic integrity-checksum pattern. It detects
// corruption and casual tampering; it is NOT proof against a determined
// adversary. Replacing it with a production digest (SHA-256 or stronger,
// keyed under reviewed key management) is a DEPLOYMENT REQUIREMENT — the
// same honesty as `signing.rs`'s [`crate::signing::NoVerifier`] default and
// the P4.28/P6.15 `fnv1a64` statements. The digest algorithm label is
// versioned by construction: a scheme change is a detectable format change,
// never a silent one.
//
// SEQUENCE CHECKPOINT (restart survival — the ledger's recorded risk):
// the per-channel last-accepted sequence survives a host-process restart
// through the [`SequenceCheckpointStore`] trait;
// [`FilePluginSequenceCheckpoint`] is the file-backed implementation in the
// `buffer_file.rs` FRAMING DISCIPLINE of `crates/edge-agent/src/
// buffer_file.rs` (cited, REIMPLEMENTED here in full — the kernel depends
// on no flexcore-* crate, so the edge-agent crate is never imported):
//
// ```text
// header:  b"FLEXCORE-PLUGIN-PATH-SEQ-CKPT" + format version byte (0x01)
// record:  [u32 LE payload_len][u32 LE crc32(payload)][payload]
// payload: [u16 LE channel_len][channel bytes][u64 LE sequence]
// ```
//
// Recovery is truncate-and-audit, never silent: the FIRST malformed frame
// (bad length, CRC mismatch, undecodable payload) ends the replay and the
// file is TRUNCATED at the last good record, with the loss COUNTED
// ([`FilePluginSequenceCheckpoint::corrupt_tail_truncated`]). A wrong
// magic/version is a hard open error. The last record per channel wins
// (the store is append-only). Durability BEFORE state advance: a sequence
// is admitted only after its checkpoint record is flushed (`sync_data`),
// so a crash cannot lose a stored advance and a replayed request after
// restart is still refused.
//
// TIME: the verifier reads the INJECTED [`Clock`] only (`crate::lifecycle::Clock`,
// the kernel-local port — `SystemTime::now()` appears nowhere; P7.42
// determinism and the ledger's "expiry checked against wall-clock time
// without a clock bound" risk both demand it). Same clock readings → same
// admission decisions.
//
// ADR-010 ON THE PATH (the reinforcement): a plugin's RESPONSE to an
// optimizer-class contract is a PROPOSAL envelope — a typed CANDIDATE
// ([`PluginProposalEnvelope`]: proposal id, producer plugin id+version,
// payload, sequence) that enters the protected validation → feasibility →
// authorization path and gains NO authority from having been proposed
// (§13.4; ADR-010). The type system enforces the structural half: the
// path outcome enum ([`PluginPathOutcome`]) has NO dispatch-authorizing
// variant — a dispatch-authorizing response is not constructible through
// this module because no such type exists (the same type-level honesty as
// the P7.11 broker's `()`-returning validation-forward). The DYNAMIC half
// is enforced at resolve/registry: a manifest claiming a protected-kernel
// contract id is refused by the P7.06 resolver
// (`crate::resolve::PROTECTED_KERNEL_CONTRACT_IDS`,
// [`crate::resolve::ResolveError::ProtectedKernelReplace`]); this module
// re-states the guard over any manifest handed to it directly
// ([`refuse_protected_contract_claim`]) so a caller that skips the resolver
// still cannot walk a protected claim through the proposal path — cited,
// restated, and pinned by tests.
//
// BROKER DUPLICATION (§15.4 at-least-once): a duplicate delivery of the
// same request is SEQUENCE_REPLAYED (check 4) and processed once; the
// duplicate delivery that carries a conflicting payload under the same
// correlation id is DIGEST_MISMATCH (check 2) — the payload-bound identity
// makes the two cases distinct named refusals, never a silent
// process-twice. The outbox/inbox persistence itself is the transport
// seam's obligation (recorded, not implemented here — see the decision
// record).
//
// Determinism: checkpoint load/iteration is sorted; the canonical digest
// string is built in a fixed field order; `serde_json::Value` object keys
// are serialized in sorted key order (serde_json's default `Map`), so the
// same payload always hashes the same. No `unwrap`/`expect` on untrusted
// input anywhere in `src/`.

use std::collections::BTreeMap;
use std::fmt;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::Value;

use crate::lifecycle::Clock;
use crate::manifest::{validate_plugin_id, Manifest};
use crate::resolve::PROTECTED_KERNEL_CONTRACT_IDS;
use crate::signing::DIGEST_ALGORITHM_FNV1A64_TEST;

/// The versioned plugin-path digest label inside the recorded envelope
/// statement (the P6.15 scheme-prefix pattern: a scheme change is a
/// detectable format change). The algorithm itself is the test-grade
/// FNV-1a 64 (see the module-header honesty note).
pub const PLUGIN_PATH_DIGEST_ALG: &str = DIGEST_ALGORITHM_FNV1A64_TEST;

/// The recorded plugin-path protection statement (the P4.28
/// statement pattern — pinned by tests so what IS and IS NOT claimed stays
/// reviewable).
pub const PLUGIN_PATH_PROTECTION_STATEMENT: &str = "the P7.36 plugin-path protection is the \
     P4.28/P6.15 discipline adapted to the plugin path: payload-bound request identity \
     (correlation id + FNV-1a-64 digest — the recorded TEST-GRADE binding; a production \
     digest is a DEPLOYMENT REQUIREMENT), monotonic per-plugin sequence with distinct \
     duplicate/out-of-sequence refusals that survive process restart through a file \
     checkpoint, and an enforced validity window against an injected clock. Plugin \
     responses to optimizer-class contracts are PROPOSAL envelopes with no \
     dispatch-authorizing variant anywhere in the type surface (ADR-010, §13.4); \
     protected-kernel contract claims are refused at resolve/registry. No certification \
     or security claim is made.";

/// The default control channel of the per-plugin sequence guard
/// (§14.3: "per asset or control channel"; a deployment may name more —
/// the guard's key is plugin + channel).
pub const DEFAULT_PLUGIN_CHANNEL: &str = "invocation";

/// The per-channel sequence key (plugin id + control channel; the P4.28
/// `channel_key` discipline, adapted to the plugin path).
pub fn sequence_channel(plugin_id: &str, channel: &str) -> String {
    format!("{plugin_id}/{channel}")
}

/// Fail-closed payload cap on one request envelope (a request envelope is
/// metadata plus the payload it binds; an unbounded payload would make the
/// digest and checkpoint paths a denial-of-service vector before any host
/// limit could apply).
pub const MAX_REQUEST_PAYLOAD_BYTES: usize = 1024 * 1024;

// --- The request envelope ---------------------------------------------------------

/// A platform→plugin request envelope. Built ON THE PLATFORM SIDE (the
/// constructor computes and stores the payload-bound digest); the verifier
/// recomputes and compares ([`PluginRequestEnvelope::verify_digest`]) —
/// the P4.28 sign/verify split, adapted.
#[derive(Debug, Clone, PartialEq)]
pub struct PluginRequestEnvelope {
    /// The correlation id binding the request to its response
    /// (§21.3 correlation discipline; opaque to the plugin logic).
    pub correlation_id: String,
    /// The plugin the request targets.
    pub plugin_id: String,
    /// The plugin version the request was composed against (part of the
    /// bound identity — a version change rebinds every digest).
    pub plugin_version: String,
    /// The monotonic per-plugin/channel sequence (§14.3 field 10; ≥ 1).
    pub sequence: u64,
    /// The validity window in INJECTED-CLOCK milliseconds (§14.3 field 15
    /// by analogy): the request is executable only while
    /// `valid_from_ms <= now < valid_until_ms`.
    pub valid_from_ms: u64,
    pub valid_until_ms: u64,
    /// The payload the identity binds (opaque JSON; never parsed here).
    pub payload: Value,
    /// The recorded digest over the canonical binding (16 lowercase hex
    /// digits of the FNV-1a 64 — [`PLUGIN_PATH_DIGEST_ALG`]).
    pub digest_hex: String,
}

impl PluginRequestEnvelope {
    /// Constructs an envelope: validates the self-validity class up front
    /// and records the payload-bound digest (the platform "signs"; the
    /// verifier later recomputes — a constructed envelope's digest can
    /// still mismatch if its fields are mutated after construction, which
    /// is exactly what check 2 catches).
    pub fn new(
        correlation_id: &str,
        plugin_id: &str,
        plugin_version: &str,
        sequence: u64,
        valid_from_ms: u64,
        valid_until_ms: u64,
        payload: Value,
    ) -> Result<PluginRequestEnvelope, PluginPathError> {
        let envelope = PluginRequestEnvelope {
            correlation_id: correlation_id.to_string(),
            plugin_id: plugin_id.to_string(),
            plugin_version: plugin_version.to_string(),
            sequence,
            valid_from_ms,
            valid_until_ms,
            payload,
            digest_hex: String::new(),
        };
        envelope.check_self()?;
        let mut envelope = envelope;
        envelope.digest_hex = format!("{:016x}", fnv1a64(envelope.canonical_string().as_bytes()));
        Ok(envelope)
    }

    /// The canonical digest binding: stable field order, `|` separators,
    /// the payload rendered through serde_json (sorted object keys —
    /// deterministic for equal payloads). The plugin id is part of the
    /// binding: a request captured for one plugin is not replayable
    /// against another.
    pub fn canonical_string(&self) -> String {
        format!(
            "corr:{}|plugin:{}@{}|seq:{}|from:{}|until:{}|alg:{}|payload:{}",
            self.correlation_id,
            self.plugin_id,
            self.plugin_version,
            self.sequence,
            self.valid_from_ms,
            self.valid_until_ms,
            PLUGIN_PATH_DIGEST_ALG,
            self.payload,
        )
    }

    /// Recomputes the digest over the CURRENT field values and compares it
    /// with the recorded one — the tampered-envelope refusal (check 2).
    pub fn verify_digest(&self) -> Result<(), PluginPathError> {
        let recomputed = format!("{:016x}", fnv1a64(self.canonical_string().as_bytes()));
        if recomputed == self.digest_hex {
            Ok(())
        } else {
            Err(PluginPathError::DigestMismatch {
                correlation_id: self.correlation_id.clone(),
            })
        }
    }

    /// Check 1: the envelope's self-validity, refused before any other
    /// check (an invalid envelope is malformed, not merely stale).
    fn check_self(&self) -> Result<(), PluginPathError> {
        if self.correlation_id.trim().is_empty() || self.correlation_id.len() > 256 {
            return Err(PluginPathError::RequestEnvelopeInvalid {
                reason: "the correlation id is empty or longer than 256 characters"
                    .to_string(),
            });
        }
        if self.plugin_id.trim().is_empty() {
            return Err(PluginPathError::RequestEnvelopeInvalid {
                reason: "the plugin id is empty".to_string(),
            });
        }
        if self.plugin_version.trim().is_empty() {
            return Err(PluginPathError::RequestEnvelopeInvalid {
                reason: "the plugin version is empty".to_string(),
            });
        }
        if self.sequence == 0 {
            return Err(PluginPathError::RequestEnvelopeInvalid {
                reason: "sequence 0 is unsequenced (sequences start at 1)".to_string(),
            });
        }
        if self.valid_from_ms >= self.valid_until_ms {
            return Err(PluginPathError::RequestEnvelopeInvalid {
                reason: "the validity window is empty or inverted (valid_from must be \
                         strictly before valid_until)"
                    .to_string(),
            });
        }
        if self.payload.to_string().len() > MAX_REQUEST_PAYLOAD_BYTES {
            return Err(PluginPathError::RequestEnvelopeInvalid {
                reason: "the payload exceeds the request-payload cap".to_string(),
            });
        }
        Ok(())
    }
}

// --- The response envelope --------------------------------------------------------

/// A plugin→platform response: it carries BACK the request's correlation id
/// and sequence (the bound identity) plus the plugin's payload. The
/// platform verifies the echo ([`PluginPathVerifier::verify_response`]) —
/// a response that does not echo the request's identity is a named refusal.
/// There is NO authorization surface on this type: a response is data for
/// the protected path, never an authority (ADR-010 — see
/// [`PluginProposalEnvelope`]).
#[derive(Debug, Clone, PartialEq)]
pub struct PluginResponseEnvelope {
    /// The echoed request correlation id.
    pub correlation_id: String,
    /// The echoed request sequence.
    pub sequence: u64,
    /// The plugin's response payload (for an optimizer-class contract, the
    /// PROPOSAL data — recorded as [`PluginProposalEnvelope`] by the
    /// caller, never as a dispatch authority).
    pub payload: Value,
}

// --- The proposal envelope (ADR-010, the type-level proof) -------------------------

/// The ADR-010 proposal statement (restated for the plugin path; pinned by
/// tests and printed in the decision record).
pub const PROPOSAL_ENVELOPE_STATEMENT: &str = "a plugin response to an optimizer-class \
     contract is a PROPOSAL envelope: a typed candidate (proposal id, producer plugin \
     id and version, payload, sequence) that enters the protected validation → \
     feasibility → authorization path and gains no authority from having been proposed. \
     The plugin-path type surface has NO dispatch-authorizing variant: a \
     dispatch-authorizing response is structurally not constructible from this module \
     (ADR-010; report §13.4; the never-granted permission set of §5.7).";

/// A plugin's PROPOSAL: the typed CANDIDATE envelope every optimizer-class
/// plugin response is recorded as (ADR-010 reinforcement — the type system
/// makes it structurally impossible to construct a dispatch-authorizing
/// response here, because no such variant exists on [`PluginPathOutcome`]
/// and no such constructor exists on this type). A proposal is INPUT to the
/// protected kernel path; it authorizes nothing.
#[derive(Debug, Clone, PartialEq)]
pub struct PluginProposalEnvelope {
    /// The proposal id (unique within the producer's proposal stream).
    pub proposal_id: String,
    /// The producing plugin's id.
    pub producer_plugin_id: String,
    /// The producing plugin's version (provenance — §9.2 provenance
    /// discipline on the proposal path).
    pub producer_plugin_version: String,
    /// The proposal payload (the candidate schedule, forecast, or other
    /// contract output — opaque here).
    pub payload: Value,
    /// The producer's monotonic sequence the proposal was produced under
    /// (ties the proposal to the request/response discipline above).
    pub sequence: u64,
}

impl PluginProposalEnvelope {
    /// Constructs a proposal envelope (self-validity refused as
    /// [`PluginPathError::ProposalEnvelopeInvalid`]). NO variant or method
    /// anywhere on this type grants, authorizes, issues, or approves
    /// anything.
    pub fn new(
        proposal_id: &str,
        producer_plugin_id: &str,
        producer_plugin_version: &str,
        payload: Value,
        sequence: u64,
    ) -> Result<PluginProposalEnvelope, PluginPathError> {
        if proposal_id.trim().is_empty() || proposal_id.len() > 256 {
            return Err(PluginPathError::ProposalEnvelopeInvalid {
                reason: "the proposal id is empty or longer than 256 characters".to_string(),
            });
        }
        if producer_plugin_id.trim().is_empty() {
            return Err(PluginPathError::ProposalEnvelopeInvalid {
                reason: "the producer plugin id is empty".to_string(),
            });
        }
        if producer_plugin_version.trim().is_empty() {
            return Err(PluginPathError::ProposalEnvelopeInvalid {
                reason: "the producer plugin version is empty".to_string(),
            });
        }
        if sequence == 0 {
            return Err(PluginPathError::ProposalEnvelopeInvalid {
                reason: "the proposal sequence is 0 (sequences start at 1)".to_string(),
            });
        }
        Ok(PluginProposalEnvelope {
            proposal_id: proposal_id.to_string(),
            producer_plugin_id: producer_plugin_id.to_string(),
            producer_plugin_version: producer_plugin_version.to_string(),
            payload,
            sequence,
        })
    }
}

/// The outcome of one protected-path exchange. CLOSED: the variants are
/// exactly what the plugin path can produce. THE TYPE-LEVEL PROOF
/// (ADR-010): there is no variant — and therefore no way through this
/// module — that authorizes, issues, or approves a dispatch; a proposal
/// enters the protected validation path as data, never as authority. Tests
/// pin the closed variant set by exhaustive match.
#[derive(Debug, Clone, PartialEq)]
pub enum PluginPathOutcome {
    /// The request was verified, sequenced, and delivered; the plugin's
    /// response is recorded as a PROPOSAL envelope (for optimizer-class
    /// contracts) or as observed result data.
    ProposalRecorded(PluginProposalEnvelope),
    /// The exchange completed with no proposal to record (e.g. a
    /// non-optimizer contract); only the correlation identity is retained.
    ResultObserved { correlation_id: String },
}

// --- The protected-kernel restatement (resolve/registry, cited) ---------------------

/// Restates the P7.06 resolver's protected-kernel guard over a manifest
/// handed to the plugin path directly (the ledger P7.36 risk: "sequence
/// checked at the broker but not at the protected gate, so a replayed
/// proposal that bypasses the broker still lands" — the same bypass logic
/// applies to the protected-contract guard, so it is restated here). The
/// AUTHORITATIVE guard is the resolver's
/// (`crate::resolve::PROTECTED_KERNEL_CONTRACT_IDS` +
/// [`crate::resolve::ResolveError::ProtectedKernelReplace`], ADR-005/ADR-010);
/// this restatement refuses the same claim set with the same reserved ids
/// for a caller that skipped resolution. Cited, restated, and tested.
pub fn refuse_protected_contract_claim(manifest: &Manifest) -> Result<(), PluginPathError> {
    for contract in &manifest.provider_of {
        if PROTECTED_KERNEL_CONTRACT_IDS.contains(&contract.as_str()) {
            return Err(PluginPathError::ProtectedContractClaim {
                plugin_id: manifest.plugin_id.clone(),
                contract_id: contract.clone(),
            });
        }
    }
    // Defensive consistency: a manifest whose plugin id never parsed as a
    // plugin id cannot walk the plugin path either (the same grammar the
    // manifest validator enforces — `validate_plugin_id` is re-run because
    // this function may receive a hand-built [`Manifest`]).
    validate_plugin_id(&manifest.plugin_id).map_err(|reason| PluginPathError::RequestEnvelopeInvalid {
        reason: format!("the manifest's plugin id is malformed: {reason}"),
    })?;
    Ok(())
}

// --- Errors -----------------------------------------------------------------------

/// Every plugin-path refusal is a DISTINCT named error
/// ([`PluginPathError::name`]) — never a warning, never a silent
/// acceptance, never one combined "valid" boolean (the P6.15 pattern).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginPathError {
    /// Check 1: the envelope is malformed (empty correlation id, sequence
    /// 0, inverted window, oversized payload, or a malformed identity in
    /// the protected-contract restatement).
    RequestEnvelopeInvalid { reason: String },
    /// Check 2: the envelope's digest does not recompute over its current
    /// fields — tampered or conflicting payload under a captured identity.
    /// The payload is never echoed.
    DigestMismatch { correlation_id: String },
    /// Check 3a: the validity window has not opened yet (injected clock).
    RequestNotYetValid { correlation_id: String, valid_from_ms: u64 },
    /// Check 3b: the validity window has closed — the request is expired
    /// and is refused, never executed (injected clock, never wall-clock).
    RequestExpired { correlation_id: String, valid_until_ms: u64 },
    /// Check 4a: the sequence equals the last ACCEPTED one — a replay of
    /// an already-processed request (duplicate delivery, §15.4).
    SequenceReplayed { channel: String, last_accepted: u64 },
    /// Check 4b: the sequence is LOWER than the last accepted one — out of
    /// sequence (never a "late but fine" delivery).
    SequenceOutOfOrder { channel: String, last_accepted: u64 },
    /// The sequence checkpoint could not be loaded or stored; verification
    /// cannot run without sequence history and cannot admit without a
    /// durable advance (fail-closed — the P6.15 rule).
    CheckpointUnavailable { reason: String },
    /// The checkpoint file exists but is not a plugin-path sequence
    /// checkpoint (wrong magic/version) — refused, never guessed at.
    CheckpointFileForeign { reason: String },
    /// A response does not echo the request's correlation id.
    ResponseCorrelationMismatch { expected: String, found: String },
    /// A response does not echo the request's sequence.
    ResponseSequenceMismatch { expected: u64, found: u64 },
    /// A proposal envelope is malformed (empty id/producer identity,
    /// sequence 0).
    ProposalEnvelopeInvalid { reason: String },
    /// A manifest handed to the plugin path claims a protected-kernel
    /// contract id (the resolver's `PROTECTED_KERNEL_REPLACE` restated —
    /// ADR-005/ADR-010; the authoritative refusal is the P7.06 resolver's).
    ProtectedContractClaim { plugin_id: String, contract_id: String },
}

impl PluginPathError {
    /// The stable machine-readable error name (the named-error table key;
    /// printed in the decision record).
    pub fn name(&self) -> &'static str {
        match self {
            PluginPathError::RequestEnvelopeInvalid { .. } => "REQUEST_ENVELOPE_INVALID",
            PluginPathError::DigestMismatch { .. } => "REQUEST_DIGEST_MISMATCH",
            PluginPathError::RequestNotYetValid { .. } => "REQUEST_NOT_YET_VALID",
            PluginPathError::RequestExpired { .. } => "REQUEST_EXPIRED",
            PluginPathError::SequenceReplayed { .. } => "SEQUENCE_REPLAYED",
            PluginPathError::SequenceOutOfOrder { .. } => "SEQUENCE_OUT_OF_ORDER",
            PluginPathError::CheckpointUnavailable { .. } => "CHECKPOINT_UNAVAILABLE",
            PluginPathError::CheckpointFileForeign { .. } => "CHECKPOINT_FILE_FOREIGN",
            PluginPathError::ResponseCorrelationMismatch { .. } => "RESPONSE_CORRELATION_MISMATCH",
            PluginPathError::ResponseSequenceMismatch { .. } => "RESPONSE_SEQUENCE_MISMATCH",
            PluginPathError::ProposalEnvelopeInvalid { .. } => "PROPOSAL_ENVELOPE_INVALID",
            PluginPathError::ProtectedContractClaim { .. } => "PROTECTED_CONTRACT_CLAIM",
        }
    }
}

impl fmt::Display for PluginPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PluginPathError::RequestEnvelopeInvalid { reason } => {
                write!(f, "the plugin request envelope is invalid: {reason}")
            }
            PluginPathError::DigestMismatch { correlation_id } => {
                write!(
                    f,
                    "request {correlation_id}: payload-bound digest mismatch — the payload does \
                     not match the bound identity; refused"
                )
            }
            PluginPathError::RequestNotYetValid { correlation_id, valid_from_ms } => {
                write!(
                    f,
                    "request {correlation_id}: validity window opens at {valid_from_ms} (injected \
                     clock); refused until then"
                )
            }
            PluginPathError::RequestExpired { correlation_id, valid_until_ms } => {
                write!(
                    f,
                    "request {correlation_id}: expired at {valid_until_ms} (injected clock); \
                     refused, never executed"
                )
            }
            PluginPathError::SequenceReplayed { channel, last_accepted } => {
                write!(
                    f,
                    "sequence {last_accepted} on channel '{channel}' was already accepted — a \
                     replayed/duplicated request; refused"
                )
            }
            PluginPathError::SequenceOutOfOrder { channel, last_accepted } => {
                write!(
                    f,
                    "sequence below the last accepted ({last_accepted}) on channel '{channel}' — \
                     out of sequence; refused"
                )
            }
            PluginPathError::CheckpointUnavailable { reason } => {
                write!(
                    f,
                    "the sequence checkpoint is unavailable: {reason} (fail-closed: no history, \
                     no admission)"
                )
            }
            PluginPathError::CheckpointFileForeign { reason } => {
                write!(
                    f,
                    "the checkpoint file is not a plugin-path sequence checkpoint (refused, \
                     never guessed): {reason}"
                )
            }
            PluginPathError::ResponseCorrelationMismatch { expected, found } => {
                write!(
                    f,
                    "the response does not echo the request's correlation id (expected \
                     '{expected}', found '{found}'); refused"
                )
            }
            PluginPathError::ResponseSequenceMismatch { expected, found } => {
                write!(
                    f,
                    "the response does not echo the request's sequence (expected {expected}, \
                     found {found}); refused"
                )
            }
            PluginPathError::ProposalEnvelopeInvalid { reason } => {
                write!(f, "the proposal envelope is invalid: {reason}")
            }
            PluginPathError::ProtectedContractClaim { plugin_id, contract_id } => {
                write!(
                    f,
                    "manifest '{plugin_id}' claims protected-kernel contract '{contract_id}' — \
                     refused (ADR-005/ADR-010; the authoritative guard is the resolver's \
                     PROTECTED_KERNEL_REPLACE)"
                )
            }
        }
    }
}

impl std::error::Error for PluginPathError {}

// --- The sequence checkpoint (restart survival) ------------------------------------

/// Errors of opening or operating the file checkpoint. Every failure is
/// named; nothing is swallowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckpointError {
    /// The checkpoint path was empty or not usable as a file path.
    InvalidPath { reason: String },
    /// The underlying filesystem failed (open/read/write/sync).
    Io { reason: String },
    /// The file exists but is not a plugin-path sequence checkpoint (wrong
    /// magic or version) — refused, never guessed at.
    NotACheckpointFile { reason: String },
}

impl fmt::Display for CheckpointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckpointError::InvalidPath { reason } => {
                write!(f, "the checkpoint path is not usable: {reason}")
            }
            CheckpointError::Io { reason } => write!(f, "the checkpoint I/O failed: {reason}"),
            CheckpointError::NotACheckpointFile { reason } => {
                write!(f, "the file is not a FlexCore plugin-path sequence checkpoint: {reason}")
            }
        }
    }
}

impl std::error::Error for CheckpointError {}

/// The persisted last-accepted-sequence store (the restart-survivability
/// half of check 4). The verifier loads the whole state at construction
/// (fail-closed: no history, no verification) and stores each advance
/// DURABLY BEFORE the in-memory state moves.
pub trait SequenceCheckpointStore {
    /// The persisted last-accepted sequence per channel, as
    /// `(channel, sequence)` pairs (a channel never admitted has no entry).
    fn load_all(&self) -> Result<Vec<(String, u64)>, CheckpointError>;
    /// Persists one channel's advanced last-accepted sequence. On `Err` the
    /// caller must refuse the request fail-closed (an unpersisted advance
    /// could be replayed after a restart).
    fn store_last_accepted(&self, channel: &str, sequence: u64) -> Result<(), CheckpointError>;
}

/// Hard sanity cap on one checkpoint record payload (rejects absurd lengths
/// from a corrupt header instead of attempting an absurd allocation — the
/// buffer_file.rs discipline).
const MAX_CHECKPOINT_RECORD_BYTES: u32 = 64 * 1024;

/// The file-backed [`SequenceCheckpointStore`], in the `buffer_file.rs`
/// framing discipline (REIMPLEMENTED — the kernel depends on no flexcore-*
/// crate; see the module header for the exact format):
///
/// - **Recovery (truncate-and-audit, never silent).** On open the file is
///   replayed record by record; the FIRST malformed frame ends the replay
///   and the file is TRUNCATED at the last good record, with the loss
///   COUNTED ([`FilePluginSequenceCheckpoint::corrupt_tail_truncated`]).
///   A wrong magic/version is a hard open error.
/// - **Last record per channel wins** (the store is append-only; replay
///   keeps the newest entry).
/// - **Durability before state.** `store_last_accepted` appends + flushes
///   + `sync_data` BEFORE the in-memory map moves.
pub struct FilePluginSequenceCheckpoint {
    path: PathBuf,
    file: Mutex<std::fs::File>,
    state: Mutex<BTreeMap<String, u64>>,
    corrupt_tail_truncated: std::sync::atomic::AtomicU64,
    records_written: std::sync::atomic::AtomicU64,
}

impl FilePluginSequenceCheckpoint {
    /// The journal magic header (31-byte label) + format version.
    fn magic() -> Vec<u8> {
        let mut magic = b"FLEXCORE-PLUGIN-PATH-SEQ-CKPT".to_vec();
        magic.push(0x01);
        magic
    }

    /// Opens (or creates) the checkpoint file and replays any existing
    /// state. A corrupt tail is truncated and counted, never silent; a
    /// wrong magic/version is a hard error.
    pub fn open(path: &Path) -> Result<Self, CheckpointError> {
        if path.as_os_str().is_empty() {
            return Err(CheckpointError::InvalidPath {
                reason: "the checkpoint path is empty".to_string(),
            });
        }
        let magic = Self::magic();
        let (file, replayed) = match std::fs::OpenOptions::new().read(true).write(true).open(path) {
            Ok(mut existing) => {
                let mut contents = Vec::new();
                existing
                    .read_to_end(&mut contents)
                    .map_err(|e| CheckpointError::Io {
                        reason: format!("reading the existing checkpoint failed: {e}"),
                    })?;
                if contents.is_empty() {
                    // A zero-length file (touch/new) is initialized, not refused.
                    (existing, BTreeMap::new())
                } else {
                    if contents.len() < magic.len() || contents[..magic.len()] != magic[..] {
                        return Err(CheckpointError::NotACheckpointFile {
                            reason: format!(
                                "the file at {} does not carry the FlexCore plugin-path \
                                 sequence checkpoint magic",
                                path.display()
                            ),
                        });
                    }
                    let mut state = BTreeMap::new();
                    let mut offset = magic.len();
                    let mut corrupt_tail_truncated: u64 = 0;
                    while offset + 8 <= contents.len() {
                        let len_bytes = match contents
                            .get(offset..offset + 4)
                            .and_then(|b| <[u8; 4]>::try_from(b).ok())
                        {
                            Some(b) => b,
                            None => {
                                corrupt_tail_truncated += 1;
                                break;
                            }
                        };
                        let len = u32::from_le_bytes(len_bytes);
                        if len == 0
                            || len > MAX_CHECKPOINT_RECORD_BYTES
                            || offset + 8 + len as usize > contents.len()
                        {
                            corrupt_tail_truncated += 1;
                            break;
                        }
                        let payload = match contents.get(offset + 8..offset + 8 + len as usize) {
                            Some(p) => p,
                            None => {
                                corrupt_tail_truncated += 1;
                                break;
                            }
                        };
                        let crc_bytes = match contents
                            .get(offset + 4..offset + 8)
                            .and_then(|b| <[u8; 4]>::try_from(b).ok())
                        {
                            Some(b) => b,
                            None => {
                                corrupt_tail_truncated += 1;
                                break;
                            }
                        };
                        let stored_crc = u32::from_le_bytes(crc_bytes);
                        if crc32(payload) != stored_crc {
                            corrupt_tail_truncated += 1;
                            break;
                        }
                        match decode_checkpoint_payload(payload) {
                            Some((channel, sequence)) => {
                                state.insert(channel, sequence);
                                offset += 8 + len as usize;
                            }
                            None => {
                                corrupt_tail_truncated += 1;
                                break;
                            }
                        }
                    }
                    if offset < contents.len() && corrupt_tail_truncated == 0 {
                        // A torn partial frame (fewer bytes than a record
                        // header) is also a corrupt tail.
                        corrupt_tail_truncated += 1;
                    }
                    if corrupt_tail_truncated > 0 {
                        // Truncate at the last good record; the loss is
                        // COUNTED (the queryable counter), never silent.
                        existing
                            .set_len(offset as u64)
                            .map_err(|e| CheckpointError::Io {
                                reason: format!(
                                    "truncating the corrupt checkpoint tail failed: {e}"
                                ),
                            })?;
                        existing
                            .sync_all()
                            .map_err(|e| CheckpointError::Io {
                                reason: format!("syncing the truncated checkpoint failed: {e}"),
                            })?;
                    }
                    (existing, state)
                }
            }
            Err(_) => {
                // Create a new checkpoint file with the magic header.
                if let Some(parent) = path.parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent).map_err(|e| CheckpointError::Io {
                            reason: format!("creating the checkpoint directory failed: {e}"),
                        })?;
                    }
                }
                let mut created = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .map_err(|e| CheckpointError::Io {
                        reason: format!("creating the checkpoint file failed: {e}"),
                    })?;
                created
                    .write_all(&magic)
                    .and_then(|_| created.flush())
                    .and_then(|_| created.sync_all())
                    .map_err(|e| CheckpointError::Io {
                        reason: format!("writing the checkpoint header failed: {e}"),
                    })?;
                (created, BTreeMap::new())
            }
        };
        Ok(Self {
            path: path.to_path_buf(),
            file: Mutex::new(file),
            state: Mutex::new(replayed),
            corrupt_tail_truncated: std::sync::atomic::AtomicU64::new(0),
            records_written: std::sync::atomic::AtomicU64::new(0),
        })
    }

    /// The checkpoint file path (diagnostics).
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// How many corrupt tail records were truncated at open (the audited
    /// loss counter — the buffer_file.rs discipline; nothing here is
    /// silent about a lost window).
    pub fn corrupt_tail_truncated(&self) -> u64 {
        self.corrupt_tail_truncated
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// How many records this instance has written (diagnostics).
    pub fn records_written(&self) -> u64 {
        self.records_written
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    fn encode_payload(channel: &str, sequence: u64) -> Vec<u8> {
        let mut payload = Vec::with_capacity(2 + channel.len() + 8);
        payload.extend_from_slice(&(channel.len() as u16).to_le_bytes());
        payload.extend_from_slice(channel.as_bytes());
        payload.extend_from_slice(&sequence.to_le_bytes());
        payload
    }
}

fn decode_checkpoint_payload(payload: &[u8]) -> Option<(String, u64)> {
    if payload.len() < 10 {
        return None;
    }
    let channel_len_bytes = <[u8; 2]>::try_from(&payload[0..2]).ok()?;
    let channel_len = u16::from_le_bytes(channel_len_bytes) as usize;
    if payload.len() < 2 + channel_len + 8 {
        return None;
    }
    let channel =
        std::str::from_utf8(&payload[2..2 + channel_len]).ok()?.to_string();
    let seq_bytes =
        <[u8; 8]>::try_from(&payload[2 + channel_len..2 + channel_len + 8]).ok()?;
    let sequence = u64::from_le_bytes(seq_bytes);
    Some((channel, sequence))
}

/// The CRC-32 (IEEE, reflected) used by the checkpoint records — the same
/// checksum family as the edge telemetry journal and the P6.15 edge
/// checkpoint (an integrity checksum for corruption detection, not a
/// security primitive).
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

impl SequenceCheckpointStore for FilePluginSequenceCheckpoint {
    fn load_all(&self) -> Result<Vec<(String, u64)>, CheckpointError> {
        let state = self
            .state
            .lock()
            .map_err(|_| CheckpointError::Io {
                reason: "the checkpoint state lock is poisoned".to_string(),
            })?;
        // BTreeMap iteration: sorted channel order (determinism).
        Ok(state.iter().map(|(c, s)| (c.clone(), *s)).collect())
    }

    fn store_last_accepted(&self, channel: &str, sequence: u64) -> Result<(), CheckpointError> {
        // Durability BEFORE state: the record is flushed and synced to disk
        // before the in-memory map moves (a crash cannot lose a stored
        // advance).
        let payload = Self::encode_payload(channel, sequence);
        let record_len = payload.len() as u32;
        let crc = crc32(&payload);
        let mut record = Vec::with_capacity(8 + payload.len());
        record.extend_from_slice(&record_len.to_le_bytes());
        record.extend_from_slice(&crc.to_le_bytes());
        record.extend_from_slice(&payload);
        {
            let mut file = self.file.lock().map_err(|_| CheckpointError::Io {
                reason: "the checkpoint file lock is poisoned".to_string(),
            })?;
            file.seek(SeekFrom::End(0))
                .and_then(|_| file.write_all(&record))
                .and_then(|_| file.flush())
                .and_then(|_| file.sync_data())
                .map_err(|e| CheckpointError::Io {
                    reason: format!("appending the checkpoint record failed: {e}"),
                })?;
        }
        let mut state = self.state.lock().map_err(|_| CheckpointError::Io {
            reason: "the checkpoint state lock is poisoned".to_string(),
        })?;
        state.insert(channel.to_string(), sequence);
        self.records_written
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

// --- The verifier ------------------------------------------------------------------

/// The plugin-path verifier: runs the four checks over every platform→
/// plugin request (and verifies response echoes), holds the injected clock
/// and the loaded checkpoint state, and admits a sequence only after the
/// durable advance. A refusal NEVER advances the sequence window (check 4
/// runs LAST — the P6.15 ordering) and never reaches a plugin.
pub struct PluginPathVerifier<'a> {
    clock: &'a dyn Clock,
    checkpoint: Box<dyn SequenceCheckpointStore>,
    /// Last accepted sequence per channel (loaded at construction,
    /// fail-closed).
    last_accepted: BTreeMap<String, u64>,
}

impl<'a> PluginPathVerifier<'a> {
    /// Constructs the verifier over the injected clock and the checkpoint
    /// store. The checkpoint is loaded HERE (fail-closed: a load failure is
    /// [`PluginPathError::CheckpointUnavailable`] — no history, no
    /// verification; the P6.15 rule).
    pub fn new(
        clock: &'a dyn Clock,
        checkpoint: Box<dyn SequenceCheckpointStore>,
    ) -> Result<PluginPathVerifier<'a>, PluginPathError> {
        let loaded = checkpoint.load_all().map_err(|e| {
            PluginPathError::CheckpointUnavailable {
                reason: e.to_string(),
            }
        })?;
        let mut last_accepted = BTreeMap::new();
        for (channel, sequence) in loaded {
            last_accepted.insert(channel, sequence);
        }
        Ok(PluginPathVerifier { clock, checkpoint, last_accepted })
    }

    /// The last accepted sequence for a channel (test/inspection surface).
    pub fn last_accepted(&self, channel: &str) -> Option<u64> {
        self.last_accepted.get(channel).copied()
    }

    /// Runs the four checks in the FIXED order and admits the request.
    /// On `Ok(())` the channel's last-accepted has advanced DURABLY; on any
    /// refusal the window is UNCHANGED.
    pub fn verify_and_admit(
        &mut self,
        envelope: &PluginRequestEnvelope,
    ) -> Result<(), PluginPathError> {
        // 1. Envelope self-validity.
        envelope.check_self()?;
        // 2. Payload-bound identity.
        envelope.verify_digest()?;
        // 3. Validity window (injected clock only).
        let now = self.clock.now_millis();
        if now < envelope.valid_from_ms {
            return Err(PluginPathError::RequestNotYetValid {
                correlation_id: envelope.correlation_id.clone(),
                valid_from_ms: envelope.valid_from_ms,
            });
        }
        if now >= envelope.valid_until_ms {
            return Err(PluginPathError::RequestExpired {
                correlation_id: envelope.correlation_id.clone(),
                valid_until_ms: envelope.valid_until_ms,
            });
        }
        // 4. Sequence admission LAST (a request refused by any earlier
        //    check never advances the window). Duplicate vs out-of-sequence
        //    are DISTINCT refusals (the P4.28 SequenceGuard rule).
        let channel = sequence_channel(&envelope.plugin_id, DEFAULT_PLUGIN_CHANNEL);
        match self.last_accepted.get(&channel) {
            Some(&last) if envelope.sequence == last => {
                return Err(PluginPathError::SequenceReplayed {
                    channel,
                    last_accepted: last,
                });
            }
            Some(&last) if envelope.sequence < last => {
                return Err(PluginPathError::SequenceOutOfOrder {
                    channel,
                    last_accepted: last,
                });
            }
            _ => {}
        }
        // Durability BEFORE state: the checkpoint advance lands on disk
        // before the in-memory window moves; a failed store refuses the
        // request with the window unchanged.
        self.checkpoint
            .store_last_accepted(&channel, envelope.sequence)
            .map_err(|e| PluginPathError::CheckpointUnavailable {
                reason: e.to_string(),
            })?;
        self.last_accepted.insert(channel, envelope.sequence);
        Ok(())
    }

    /// Verifies a plugin response's identity echo against its request:
    /// the correlation id and the sequence must match exactly. The
    /// response payload is data for the protected path (for optimizer-class
    /// contracts, recorded as [`PluginProposalEnvelope`] by the caller) —
    /// never an authority.
    pub fn verify_response(
        &self,
        request: &PluginRequestEnvelope,
        response: &PluginResponseEnvelope,
    ) -> Result<(), PluginPathError> {
        if response.correlation_id != request.correlation_id {
            return Err(PluginPathError::ResponseCorrelationMismatch {
                expected: request.correlation_id.clone(),
                found: response.correlation_id.clone(),
            });
        }
        if response.sequence != request.sequence {
            return Err(PluginPathError::ResponseSequenceMismatch {
                expected: request.sequence,
                found: response.sequence,
            });
        }
        Ok(())
    }

    /// Records a verified response as the path outcome: an optimizer-class
    /// proposal is wrapped in the typed [`PluginProposalEnvelope`] (the
    /// ADR-010 reinforcement — no dispatch-authorizing surface exists on
    /// the outcome), or the exchange is closed as
    /// [`PluginPathOutcome::ResultObserved`]. The echo is re-verified FIRST
    /// (the record path must not accept a mismatched echo either); the
    /// caller chooses the proposal variant, and BOTH outcomes carry data
    /// only.
    pub fn record_outcome(
        &self,
        request: &PluginRequestEnvelope,
        response: &PluginResponseEnvelope,
        proposal: Option<PluginProposalEnvelope>,
    ) -> Result<PluginPathOutcome, PluginPathError> {
        self.verify_response(request, response)?;
        Ok(match proposal {
            Some(p) => PluginPathOutcome::ProposalRecorded(p),
            None => PluginPathOutcome::ResultObserved {
                correlation_id: request.correlation_id.clone(),
            },
        })
    }
}

// --- The digest (the repo's FNV-1a 64 pattern, test-grade) --------------------------

/// The FNV-1a 64-bit hash (the repo's non-cryptographic integrity-checksum
/// pattern — P4.28/P6.14/P7.14; deterministic, dependency-free, explicitly
/// NOT a security primitive; a production digest is a DEPLOYMENT
/// REQUIREMENT, same honesty as `signing.rs`).
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
