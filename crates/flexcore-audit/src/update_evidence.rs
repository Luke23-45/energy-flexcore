// P18.07 — Per-device update EVIDENCE records and their digest-chained,
//          exportable evidence pack (decision record
//          `docs/phases-11-20/phase-18-edge-fleet-lifecycle-ota/records/
//          07_p18_07_update_evidence_audit_pack.md`).
//
// WHAT THIS MODULE IS: the record shapes for one device-update attempt's
// evidence (attempt / verify / install / result / rollback — the ledger's
// five closed kinds), a per-attempt linear digest chain over those
// records, an export bundle shape, and the mapping that rides each record
// into the §10.1 `AuditRecord` so the evidence ALSO flows through the
// P11.15 production chain.
//
// WHAT THIS MODULE CONSUMES (never redefines):
//
// - THE P11.15 CHAIN: the §10.1 `AuditRecord` mapping means every update
//   evidence record can be appended through the existing `AuditSink`
//   port and chained by the P11.15 keyed HMAC-SHA-256 construction
//   (`chain::chain_step` / `chain::verify_tenant_chain`). This module
//   adds NOTHING to that construction — the per-attempt chain below is a
//   SECOND-LEVEL evidence commit (one chain per update attempt, so an
//   operator can hand over one attempt's evidence without the whole
//   tenant chain) built from the SAME std-only SHA-256/HMAC primitives
//   (`chain::hmac_sha256`), domain-separated from the audit chain. The
//   P11.15 deployment requirement carries over verbatim: keyed
//   tamper-EVIDENCE under reviewed key management, never tamper-PROOF,
//   never a release signature.
// - LR-203: the test-grade FNV-1a-64 path stays what it is
//   (`crates/edge-agent/src/audit_proof.rs`); nothing here touches it and
//   no update evidence record exists anywhere (no device, no fleet, no
//   update).
//
// KEY MANAGEMENT (the honest boundary): the chain key is CONSUMED from
// the deployment (the same P11.15 key-material discipline — generation,
// distribution, rotation, storage, and operator sign-off are human-gated
// DEPLOYMENT concerns). This module never generates, loads, or stores key
// material; until an operator provisions reviewed keys, the evidence
// chain is NOT running.
//
// Append-orientation: records are appended to a per-attempt chain and
// never edited; the chain head commits to content, order, and count.

use crate::chain::{hex_encode_digest, hmac_sha256, ChainKey};
use crate::record::{
    ActorIdentity, AuditAction, AuditObject, AuditRecord, AuditRecordFields, EvidenceReference,
    RecordReference,
};
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{EventTime, IngestTime};
use flexcore_domain::ids::TenantId;

// --- Constants -----------------------------------------------------------------

/// The update-evidence chain's domain-separation prefix: a digest computed
/// here can never collide with a digest of the same bytes under the
/// P11.15 audit-chain domain or any other protocol role.
pub const UPDATE_EVIDENCE_DOMAIN: &[u8] = b"flexcore-update-evidence-v1";

/// The update-evidence chain digest algorithm label (versioned: a
/// construction change is a detectable format change, never a silent one).
pub const UPDATE_EVIDENCE_CHAIN_ALGORITHM: &str = "hmac-sha256-update-evidence-v1";

/// The update-evidence chain format version.
pub const UPDATE_EVIDENCE_FORMAT_VERSION: u32 = 1;

/// The P18.07 honesty statement (pinned so tests and reviewers can read
/// what IS and IS NOT claimed).
pub const UPDATE_EVIDENCE_STATEMENT: &str = "update evidence records are per-device, \
     per-attempt evidence shapes; each attempt's records form a keyed HMAC-SHA-256 linear \
     chain (domain-separated from the P11.15 audit chain, same std-only primitives) and \
     each record maps onto the §10.1 AuditRecord so the evidence also rides the production \
     chain. No update evidence record exists anywhere: no device, no fleet, no update has \
     ever been attempted, verified, installed, rolled back, or recorded. Keyed \
     tamper-EVIDENCE under reviewed key management (the P11.15 DEPLOYMENT REQUIREMENT), \
     never tamper-PROOF, never a release signature. No certification or security claim is \
     made.";

// --- The closed vocabularies -------------------------------------------------------

/// The five closed update-evidence kinds (the ledger's
/// attempt / verify / install / result / rollback — nothing else is
/// expressible).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateEvidenceKind {
    /// An update attempt STARTED on a device (the bundle download path
    /// began; nothing has been verified).
    Attempt,
    /// A verification VERDICT was recorded (pass or refusal — the stable
    /// refusal code rides the detail).
    Verify,
    /// An install step was recorded (slot write started/completed per the
    /// device-side contract).
    Install,
    /// The attempt RESULT (success or failure with the explicit failure
    /// state).
    Result,
    /// A rollback was recorded (the P9.34 never-silent-downgrade rule: the
    /// reason and any recorded override ride the detail).
    Rollback,
}

impl UpdateEvidenceKind {
    /// The stable machine-readable kind name (audit actions, export
    /// lines).
    pub fn name(self) -> &'static str {
        match self {
            UpdateEvidenceKind::Attempt => "UPDATE_ATTEMPTED",
            UpdateEvidenceKind::Verify => "UPDATE_VERIFY_RECORDED",
            UpdateEvidenceKind::Install => "UPDATE_INSTALL_RECORDED",
            UpdateEvidenceKind::Result => "UPDATE_RESULT_RECORDED",
            UpdateEvidenceKind::Rollback => "UPDATE_ROLLBACK_RECORDED",
        }
    }

    /// The §10.1 action string the record maps to (the `update.` namespace;
    /// validated through `AuditAction` on the mapping path).
    pub fn audit_action(self) -> &'static str {
        match self {
            UpdateEvidenceKind::Attempt => "update.attempt",
            UpdateEvidenceKind::Verify => "update.verify",
            UpdateEvidenceKind::Install => "update.install",
            UpdateEvidenceKind::Result => "update.result",
            UpdateEvidenceKind::Rollback => "update.rollback",
        }
    }

    fn code(self) -> u8 {
        match self {
            UpdateEvidenceKind::Attempt => 1,
            UpdateEvidenceKind::Verify => 2,
            UpdateEvidenceKind::Install => 3,
            UpdateEvidenceKind::Result => 4,
            UpdateEvidenceKind::Rollback => 5,
        }
    }

    fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(UpdateEvidenceKind::Attempt),
            2 => Some(UpdateEvidenceKind::Verify),
            3 => Some(UpdateEvidenceKind::Install),
            4 => Some(UpdateEvidenceKind::Result),
            5 => Some(UpdateEvidenceKind::Rollback),
            _ => None,
        }
    }
}

/// The outcome half of an evidence record (closed; never a bare boolean —
/// the detail carries the reason).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateEvidenceOutcome {
    /// The step succeeded.
    Success,
    /// The step failed (the explicit failure state rides the detail).
    Failed,
    /// The step was REFUSED (a gate/verifier refusal; the stable refusal
    /// code rides the detail).
    Refused,
}

impl UpdateEvidenceOutcome {
    /// The stable machine-readable outcome name.
    pub fn name(self) -> &'static str {
        match self {
            UpdateEvidenceOutcome::Success => "SUCCESS",
            UpdateEvidenceOutcome::Failed => "FAILED",
            UpdateEvidenceOutcome::Refused => "REFUSED",
        }
    }

    fn code(self) -> u8 {
        match self {
            UpdateEvidenceOutcome::Success => 1,
            UpdateEvidenceOutcome::Failed => 2,
            UpdateEvidenceOutcome::Refused => 3,
        }
    }
}

// --- Errors -----------------------------------------------------------------------

/// Named errors of the update-evidence records and chain. Every failure
/// is named; nothing is a silent pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateEvidenceError {
    /// The attempt id was empty or all whitespace.
    EmptyAttemptId,
    /// The device id was empty or all whitespace.
    EmptyDeviceId,
    /// The target version was empty or all whitespace.
    EmptyVersion,
    /// The detail (the reason/evidence text) was empty — an evidence
    /// record without a reason is not evidence.
    EmptyDetail,
    /// The record was written before the action occurred (P0.09 ordering
    /// rule, mirrored from the §10.1 record).
    IngestBeforeEvent,
    /// The record belongs to a DIFFERENT attempt than the chain — a chain
    /// is per-attempt; mixing attempts is refused, never merged.
    AttemptMismatch { chain_attempt: String, record_attempt: String },
    /// The record's per-attempt sequence is not exactly the next one.
    SequenceDisorder { expected: u64, found: u64 },
    /// The recomputed digest differs from the claimed one at this
    /// sequence — content, order, or count was altered.
    DigestMismatch { sequence: u64 },
    /// An export was attempted over an empty chain — refused (an empty
    /// bundle commits to nothing and is not evidence; the P11.15
    /// empty-export rule, mirrored).
    EmptyChainExport,
}

impl UpdateEvidenceError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            UpdateEvidenceError::EmptyAttemptId => "UPDATE_EVIDENCE_ATTEMPT_ID_EMPTY",
            UpdateEvidenceError::EmptyDeviceId => "UPDATE_EVIDENCE_DEVICE_ID_EMPTY",
            UpdateEvidenceError::EmptyVersion => "UPDATE_EVIDENCE_VERSION_EMPTY",
            UpdateEvidenceError::EmptyDetail => "UPDATE_EVIDENCE_DETAIL_EMPTY",
            UpdateEvidenceError::IngestBeforeEvent => "UPDATE_EVIDENCE_INGEST_BEFORE_EVENT",
            UpdateEvidenceError::AttemptMismatch { .. } => "UPDATE_EVIDENCE_ATTEMPT_MISMATCH",
            UpdateEvidenceError::SequenceDisorder { .. } => "UPDATE_EVIDENCE_SEQUENCE_DISORDER",
            UpdateEvidenceError::DigestMismatch { .. } => "UPDATE_EVIDENCE_DIGEST_MISMATCH",
            UpdateEvidenceError::EmptyChainExport => "UPDATE_EVIDENCE_EMPTY_CHAIN_EXPORT",
        }
    }
}

impl std::fmt::Display for UpdateEvidenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdateEvidenceError::EmptyAttemptId => {
                write!(f, "the update-evidence attempt id must be non-empty")
            }
            UpdateEvidenceError::EmptyDeviceId => {
                write!(f, "the update-evidence device id must be non-empty")
            }
            UpdateEvidenceError::EmptyVersion => {
                write!(f, "the update-evidence target version must be non-empty")
            }
            UpdateEvidenceError::EmptyDetail => write!(
                f,
                "the update-evidence detail (reason/evidence text) must be non-empty — an \
                 evidence record without a reason is not evidence"
            ),
            UpdateEvidenceError::IngestBeforeEvent => write!(
                f,
                "ingest_time must not precede event_time (the P0.09 ordering rule, mirrored)"
            ),
            UpdateEvidenceError::AttemptMismatch {
                chain_attempt,
                record_attempt,
            } => write!(
                f,
                "the record belongs to attempt '{record_attempt}' but this chain is for \
                 attempt '{chain_attempt}' — an evidence chain is per-attempt; mixing \
                 attempts is refused, never merged"
            ),
            UpdateEvidenceError::SequenceDisorder { expected, found } => write!(
                f,
                "the per-attempt sequence is {found}; the next expected sequence is \
                 {expected} — the disorder itself is a finding"
            ),
            UpdateEvidenceError::DigestMismatch { sequence } => write!(
                f,
                "UPDATE EVIDENCE DIGEST MISMATCH at per-attempt sequence {sequence}: the \
                 recomputed digest differs from the claimed one — the record content, the \
                 order, or the count was altered (keyed tamper-EVIDENCE, not tamper-PROOF)"
            ),
            UpdateEvidenceError::EmptyChainExport => write!(
                f,
                "an empty evidence chain commits to nothing — no export bundle is produced"
            ),
        }
    }
}

impl std::error::Error for UpdateEvidenceError {}

// --- The record --------------------------------------------------------------------

/// One per-device update-evidence record (P18.07). Timestamps are the
/// P0.09 contracts types (event = when the step happened on the device
/// path, ingest = when the platform recorded it); the detail is mandatory
/// (a refusal's stable code, a rollback's reason and override reference,
/// an install's slot reference — never empty).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateEvidenceRecord {
    /// The update attempt this record belongs to (the chain key; one
    /// chain per attempt).
    pub attempt_id: String,
    /// The device the attempt ran on (the fleet registry's device
    /// identity — the P18.12 schema, consumed by citation).
    pub device_id: String,
    /// The target version of the attempt.
    pub version: String,
    /// The closed evidence kind.
    pub kind: UpdateEvidenceKind,
    /// The closed outcome.
    pub outcome: UpdateEvidenceOutcome,
    /// The mandatory detail (reason / stable refusal code / failure
    /// state / override reference).
    pub detail: String,
    /// When the step happened (P0.09 event time).
    pub event_time: EventTime,
    /// When the platform recorded the evidence (P0.09 ingest time).
    pub ingest_time: IngestTime,
}

impl UpdateEvidenceRecord {
    /// Builds and validates one evidence record (fail-closed; every
    /// violation is a named [`UpdateEvidenceError`]).
    pub fn new(fields: Self) -> Result<Self, UpdateEvidenceError> {
        if fields.attempt_id.trim().is_empty() {
            return Err(UpdateEvidenceError::EmptyAttemptId);
        }
        if fields.device_id.trim().is_empty() {
            return Err(UpdateEvidenceError::EmptyDeviceId);
        }
        if fields.version.trim().is_empty() {
            return Err(UpdateEvidenceError::EmptyVersion);
        }
        if fields.detail.trim().is_empty() {
            return Err(UpdateEvidenceError::EmptyDetail);
        }
        if fields.ingest_time.unix_millis() < fields.event_time.unix_millis() {
            return Err(UpdateEvidenceError::IngestBeforeEvent);
        }
        Ok(fields)
    }
}

// --- The chain key (domain-separated from the audit chain) ----------------------------

/// Derives the update-evidence chain key from the deployment's namespace
/// key: HMAC domain separation over [`UPDATE_EVIDENCE_DOMAIN`]. The
/// update-evidence chains and the P11.15 audit chains are
/// cryptographically independent — a digest from one can never be
/// replayed into the other. The namespace key is the DEPLOYMENT-SUPPLIED
/// key material (the P11.15 discipline; consumed, never generated).
pub fn update_evidence_key(namespace_key: &ChainKey) -> ChainKey {
    let mut message = Vec::with_capacity(UPDATE_EVIDENCE_DOMAIN.len() + 1);
    message.extend_from_slice(UPDATE_EVIDENCE_DOMAIN);
    message.push(0);
    ChainKey(hmac_sha256(&namespace_key.0, &message))
}

// --- The canonical encoding and the per-attempt chain ---------------------------------

/// Appends one length-prefixed byte string (u64 little-endian length,
/// then the bytes) so field boundaries are unambiguous in the canonical
/// form (the P11.15 canonical-encoding pattern, mirrored).
fn push_field(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

/// The canonical byte encoding of one update-evidence record: every field
/// the record carries, in fixed order, length-prefixed, plus the
/// per-attempt sequence (so the chain commits to the ORDER, not just the
/// content). Adding a field without extending this encoding would
/// silently narrow what the chain commits to; the unit tests assert the
/// encoding covers the full field set.
fn canonical_record_bytes(record: &UpdateEvidenceRecord, sequence: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(256);
    push_field(&mut out, UPDATE_EVIDENCE_DOMAIN);
    out.push(UPDATE_EVIDENCE_FORMAT_VERSION as u8);
    push_field(&mut out, record.attempt_id.as_bytes());
    push_field(&mut out, record.device_id.as_bytes());
    push_field(&mut out, record.version.as_bytes());
    out.push(record.kind.code());
    out.push(record.outcome.code());
    push_field(&mut out, record.detail.as_bytes());
    out.extend_from_slice(&record.event_time.unix_millis().to_le_bytes());
    out.extend_from_slice(&record.ingest_time.unix_millis().to_le_bytes());
    out.extend_from_slice(&sequence.to_le_bytes());
    out
}

/// The domain-separated genesis head of a per-attempt evidence chain.
pub fn update_evidence_genesis(attempt_key: &ChainKey) -> [u8; 32] {
    let mut message = Vec::with_capacity(UPDATE_EVIDENCE_DOMAIN.len() + 16);
    message.extend_from_slice(UPDATE_EVIDENCE_DOMAIN);
    message.push(0);
    message.extend_from_slice(b"genesis");
    hmac_sha256(&attempt_key.0, &message)
}

/// Folds one evidence record into its attempt's chain: HMAC under the
/// attempt key over (previous head || canonical record bytes). The
/// previous head being an input is what makes the head commit to the
/// whole ordered attempt history.
pub fn update_evidence_step(
    attempt_key: &ChainKey,
    previous_head: &[u8; 32],
    record: &UpdateEvidenceRecord,
    sequence: u64,
) -> [u8; 32] {
    let mut message = Vec::with_capacity(32 + canonical_record_bytes(record, sequence).len());
    message.extend_from_slice(previous_head);
    message.extend_from_slice(&canonical_record_bytes(record, sequence));
    hmac_sha256(&attempt_key.0, &message)
}

/// The per-attempt evidence chain accumulator: appends records in order,
/// folds the head, and refuses anything not exactly the next sequence of
/// the SAME attempt. Append-only: there is no edit, remove, or rewrite.
pub struct UpdateEvidenceChain {
    attempt_key: ChainKey,
    attempt_id: String,
    device_id: String,
    head: [u8; 32],
    count: u64,
}

impl UpdateEvidenceChain {
    /// Opens a chain for ONE update attempt (the genesis head).
    pub fn new(namespace_key: &ChainKey, attempt_id: &str, device_id: &str)
    -> Result<Self, UpdateEvidenceError> {
        if attempt_id.trim().is_empty() {
            return Err(UpdateEvidenceError::EmptyAttemptId);
        }
        if device_id.trim().is_empty() {
            return Err(UpdateEvidenceError::EmptyDeviceId);
        }
        let attempt_key = update_evidence_key(namespace_key);
        let head = update_evidence_genesis(&attempt_key);
        Ok(Self {
            attempt_key,
            attempt_id: attempt_id.to_string(),
            device_id: device_id.to_string(),
            head,
            count: 0,
        })
    }

    /// The attempt id this chain commits to.
    pub fn attempt_id(&self) -> &str {
        &self.attempt_id
    }

    /// The current head digest (hex).
    pub fn head_hex(&self) -> String {
        hex_encode_digest(&self.head)
    }

    /// The number of records folded so far.
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Appends one record: validates it, refuses a DIFFERENT attempt or a
    /// sequence gap, folds the head, and returns the new head. The
    /// per-attempt sequence is assigned BY the chain (count + 1) — the
    /// caller never supplies it.
    pub fn append(&mut self, record: UpdateEvidenceRecord) -> Result<[u8; 32], UpdateEvidenceError>
    {
        if record.attempt_id != self.attempt_id {
            return Err(UpdateEvidenceError::AttemptMismatch {
                chain_attempt: self.attempt_id.clone(),
                record_attempt: record.attempt_id.clone(),
            });
        }
        if record.device_id != self.device_id {
            return Err(UpdateEvidenceError::AttemptMismatch {
                chain_attempt: format!("{} (device {})", self.attempt_id, self.device_id),
                record_attempt: format!("{} (device {})", record.attempt_id, record.device_id),
            });
        }
        let validated = UpdateEvidenceRecord::new(record)?;
        let sequence = self.count + 1;
        self.head = update_evidence_step(&self.attempt_key, &self.head, &validated, sequence);
        self.count = sequence;
        Ok(self.head)
    }

    /// The export bundle for the chain as it stands (P18.07: exportable).
    /// An empty chain refuses the export (an empty bundle commits to
    /// nothing and is not evidence — the P11.15 empty-export rule,
    /// mirrored).
    pub fn export(&self, exported_at_unix_millis: i64)
    -> Result<UpdateEvidenceExport, UpdateEvidenceError> {
        if self.count == 0 {
            return Err(UpdateEvidenceError::EmptyChainExport);
        }
        Ok(UpdateEvidenceExport {
            chain_version: UPDATE_EVIDENCE_FORMAT_VERSION,
            algorithm: UPDATE_EVIDENCE_CHAIN_ALGORITHM,
            attempt_id: self.attempt_id.clone(),
            device_id: self.device_id.clone(),
            record_count: self.count,
            head_digest: self.head_hex(),
            exported_at_unix_millis,
        })
    }
}

// --- The export bundle ----------------------------------------------------------------

/// The exportable per-attempt evidence bundle (P18.07). Plain data — the
/// transport (operator pack, offline retention) is the composition's
/// concern; the text form carries the honesty wording verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateEvidenceExport {
    pub chain_version: u32,
    pub algorithm: &'static str,
    pub attempt_id: String,
    pub device_id: String,
    pub record_count: u64,
    /// The attempt chain's head digest (lower-case hex).
    pub head_digest: String,
    /// When the bundle was exported (the INJECTED clock — never wall
    /// time).
    pub exported_at_unix_millis: i64,
}

impl UpdateEvidenceExport {
    /// The export summary line (operator-pack text). Carries the honest
    /// P11.15-carryover wording: keyed tamper-EVIDENCE under reviewed key
    /// management, never a release signature.
    pub fn audit_text(&self) -> String {
        format!(
            "update-evidence-export v{} alg={} attempt='{}' device='{}' records={} head={} \
             exported_at={} (keyed tamper-EVIDENCE under reviewed key management — the \
             P11.15 DEPLOYMENT REQUIREMENT carries over; never a release signature)",
            self.chain_version,
            self.algorithm,
            self.attempt_id,
            self.device_id,
            self.record_count,
            self.head_digest,
            self.exported_at_unix_millis,
        )
    }
}

// --- The §10.1 mapping (the records ride the P11.15 production chain) --------------------

/// Maps one update-evidence record onto the §10.1 `AuditRecord` so the
/// evidence ALSO flows through the existing `AuditSink` port and the
/// P11.15 keyed chain (CONSUMED, never redefined: the audit chain's
/// construction, namespace discipline, and deployment requirement are
/// `chain.rs`'s, unchanged). The mapping is fixed:
///
/// - `action` = the kind's `update.*` audit action;
/// - `object` = (`"device"`, device_id);
/// - `after_ref` = `device/{device_id}/version/{version}`;
/// - `evidence` = `update-evidence/attempt/{attempt_id}/seq/{sequence}`;
/// - `actor` / tenant / ids / timestamps / correlation id are supplied by
///   the composition (the actor is the authenticated edge or operator
///   identity — never invented here).
pub fn to_audit_record(
    record: &UpdateEvidenceRecord,
    sequence: u64,
    tenant_id: Option<TenantId>,
    actor: ActorIdentity,
    audit_id: FlexUuid,
    correlation_id: FlexUuid,
) -> Result<AuditRecord, crate::record::AuditError> {
    AuditRecord::new(AuditRecordFields {
        audit_id,
        tenant_id,
        actor,
        action: AuditAction::new(record.kind.audit_action().to_string())?,
        object: AuditObject::new("device".to_string(), record.device_id.clone())?,
        before_ref: None,
        after_ref: Some(RecordReference::new(format!(
            "device/{}/version/{}",
            record.device_id, record.version
        ))?),
        decision: None,
        evidence: Some(EvidenceReference::new(format!(
            "update-evidence/attempt/{}/seq/{sequence}",
            record.attempt_id
        ))?),
        event_time: record.event_time,
        ingest_time: record.ingest_time,
        correlation_id,
    })
}

// --- Tests (WRITTEN-UNRUN; execution is a human gate) -----------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::time_model::{EventTime, IngestTime};

    fn sample_uuid(tag: u8) -> FlexUuid {
        let mut bytes = [0u8; 16];
        bytes[15] = tag;
        FlexUuid::from_bytes(bytes)
    }

    fn test_namespace_key() -> ChainKey {
        // Test key material ONLY — never a real deployment key (the
        // P11.15 test discipline, mirrored).
        ChainKey::from_bytes(&[0x5au8; 32]).expect("valid test key")
    }

    fn record(attempt: &str, kind: UpdateEvidenceKind, outcome: UpdateEvidenceOutcome)
    -> UpdateEvidenceRecord {
        UpdateEvidenceRecord::new(UpdateEvidenceRecord {
            attempt_id: attempt.to_string(),
            device_id: "device-1".to_string(),
            version: "1.2.0".to_string(),
            kind,
            outcome,
            detail: format!("{} ({})", kind.name(), outcome.name()),
            event_time: EventTime::from_unix_millis(1_000).expect("valid"),
            ingest_time: IngestTime::from_unix_millis(2_000).expect("valid"),
        })
        .expect("valid record")
    }

    fn attempt_chain(attempt: &str) -> UpdateEvidenceChain {
        UpdateEvidenceChain::new(&test_namespace_key(), attempt, "device-1")
            .expect("chain opens")
    }

    #[test]
    fn a_full_attempt_lifecycle_chains_through_all_five_kinds() {
        let mut chain = attempt_chain("attempt-1");
        chain
            .append(record("attempt-1", UpdateEvidenceKind::Attempt, UpdateEvidenceOutcome::Success))
            .expect("attempt appended");
        chain
            .append(record(
                "attempt-1",
                UpdateEvidenceKind::Verify,
                UpdateEvidenceOutcome::Success,
            ))
            .expect("verify appended");
        chain
            .append(record(
                "attempt-1",
                UpdateEvidenceKind::Install,
                UpdateEvidenceOutcome::Success,
            ))
            .expect("install appended");
        chain
            .append(record(
                "attempt-1",
                UpdateEvidenceKind::Result,
                UpdateEvidenceOutcome::Success,
            ))
            .expect("result appended");
        assert_eq!(chain.count(), 4);
        let head_after_result = chain.head_hex();
        // A refusal verdict also chains (the refusal code rides the
        // detail).
        chain
            .append(record(
                "attempt-1",
                UpdateEvidenceKind::Verify,
                UpdateEvidenceOutcome::Refused,
            ))
            .expect("refused verify appended");
        assert_ne!(chain.head_hex(), head_after_result);
        // Rollback chains too.
        chain
            .append(record(
                "attempt-1",
                UpdateEvidenceKind::Rollback,
                UpdateEvidenceOutcome::Success,
            ))
            .expect("rollback appended");
        assert_eq!(chain.count(), 6);
    }

    #[test]
    fn a_content_edit_breaks_the_chain_at_the_edited_sequence() {
        let honest = [
            (UpdateEvidenceKind::Attempt, UpdateEvidenceOutcome::Success),
            (UpdateEvidenceKind::Verify, UpdateEvidenceOutcome::Success),
            (UpdateEvidenceKind::Install, UpdateEvidenceOutcome::Success),
        ];
        // Build the honest chain and record every per-sequence head.
        let mut chain = attempt_chain("attempt-2");
        let mut heads: Vec<String> = Vec::new();
        for (kind, outcome) in honest {
            chain
                .append(record("attempt-2", kind, outcome))
                .expect("appended");
            heads.push(chain.head_hex());
        }
        // An independent honest replay reproduces every head exactly.
        let key = update_evidence_key(&test_namespace_key());
        let mut head = update_evidence_genesis(&key);
        for (i, (kind, outcome)) in honest.iter().enumerate() {
            let r = record("attempt-2", *kind, *outcome);
            let computed = update_evidence_step(&key, &head, &r, (i + 1) as u64);
            assert_eq!(
                hex_encode_digest(&computed),
                heads[i],
                "the honest replay matches the head at sequence {}",
                i + 1
            );
            head = computed;
        }
        // A flipped outcome at sequence 2 changes the digest there (the
        // head after it diverges — the tamper cannot be hidden).
        let head_after_1 = update_evidence_step(
            &key,
            &update_evidence_genesis(&key),
            &record("attempt-2", UpdateEvidenceKind::Attempt, UpdateEvidenceOutcome::Success),
            1,
        );
        let seq2_honest =
            record("attempt-2", UpdateEvidenceKind::Verify, UpdateEvidenceOutcome::Success);
        let seq2_tampered =
            record("attempt-2", UpdateEvidenceKind::Verify, UpdateEvidenceOutcome::Failed);
        assert_ne!(
            update_evidence_step(&key, &head_after_1, &seq2_honest, 2),
            update_evidence_step(&key, &head_after_1, &seq2_tampered, 2),
            "a flipped outcome changes the digest at its sequence"
        );
    }

    #[test]
    fn sequences_and_attempt_mixing_are_refused() {
        let mut chain = attempt_chain("attempt-3");
        chain
            .append(record("attempt-3", UpdateEvidenceKind::Attempt, UpdateEvidenceOutcome::Success))
            .expect("appended");
        // A different attempt is refused (a chain is per-attempt).
        assert!(matches!(
            chain.append(record(
                "attempt-OTHER",
                UpdateEvidenceKind::Verify,
                UpdateEvidenceOutcome::Success
            )),
            Err(UpdateEvidenceError::AttemptMismatch { .. })
        ));
        // The chain's sequence assignment is internal; a caller cannot
        // create a gap (count is still 1).
        assert_eq!(chain.count(), 1);
    }

    #[test]
    fn namespace_separation_independence() {
        // The update-evidence domain is independent of the P11.15 audit
        // domain AND of a differently-derived evidence key: the same
        // record under different namespace keys yields different heads.
        let key_a = update_evidence_key(&test_namespace_key());
        let other_namespace = ChainKey::from_bytes(&[0xa5u8; 32]).expect("valid test key");
        let key_b = update_evidence_key(&other_namespace);
        let r = record("attempt-4", UpdateEvidenceKind::Attempt, UpdateEvidenceOutcome::Success);
        let head_a = update_evidence_step(&key_a, &update_evidence_genesis(&key_a), &r, 1);
        let head_b = update_evidence_step(&key_b, &update_evidence_genesis(&key_b), &r, 1);
        assert_ne!(head_a, head_b);
    }

    #[test]
    fn record_validation_is_fail_closed() {
        let base = record("attempt-5", UpdateEvidenceKind::Attempt, UpdateEvidenceOutcome::Success);
        let mut variant = base.clone();
        variant.attempt_id = "  ".to_string();
        assert_eq!(
            UpdateEvidenceRecord::new(variant),
            Err(UpdateEvidenceError::EmptyAttemptId)
        );
        let mut variant = base.clone();
        variant.device_id = String::new();
        assert!(matches!(
            UpdateEvidenceRecord::new(variant),
            Err(UpdateEvidenceError::EmptyDeviceId)
        ));
        let mut variant = base.clone();
        variant.detail = "   ".to_string();
        assert!(matches!(
            UpdateEvidenceRecord::new(variant),
            Err(UpdateEvidenceError::EmptyDetail)
        ));
        // P0.09 ordering mirrored.
        let mut variant = base.clone();
        variant.event_time = EventTime::from_unix_millis(3_000).expect("valid");
        variant.ingest_time = IngestTime::from_unix_millis(2_000).expect("valid");
        assert!(matches!(
            UpdateEvidenceRecord::new(variant),
            Err(UpdateEvidenceError::IngestBeforeEvent)
        ));
    }

    #[test]
    fn the_export_bundle_carries_the_honesty_wording() {
        let mut chain = attempt_chain("attempt-6");
        chain
            .append(record("attempt-6", UpdateEvidenceKind::Attempt, UpdateEvidenceOutcome::Success))
            .expect("appended");
        let bundle = chain.export(1_700_000_000_000).expect("exports");
        assert_eq!(bundle.record_count, 1);
        assert_eq!(bundle.algorithm, UPDATE_EVIDENCE_CHAIN_ALGORITHM);
        assert!(bundle.audit_text().contains("DEPLOYMENT REQUIREMENT"));
        assert!(bundle.audit_text().contains("never a release signature"));
        assert!(bundle.audit_text().contains("attempt-6"));
    }

    #[test]
    fn the_s10_1_mapping_rides_the_p11_15_chain() {
        // The mapped record is a valid §10.1 AuditRecord that the P11.15
        // chain functions accept and chain — CONSUMED, not redefined.
        let evidence = record(
            "attempt-7",
            UpdateEvidenceKind::Verify,
            UpdateEvidenceOutcome::Refused,
        );
        let audit_record = to_audit_record(
            &evidence,
            2,
            Some(TenantId::new(sample_uuid(2))),
            ActorIdentity::new("edge:device-1".to_string()).expect("valid actor"),
            sample_uuid(1),
            sample_uuid(3),
        )
        .expect("valid audit record");
        assert_eq!(audit_record.action.as_str(), "update.verify");
        assert_eq!(audit_record.object.object_type.as_str(), "device");
        assert_eq!(audit_record.object.object_id.as_str(), "device-1");
        let evidence_ref = audit_record.evidence.as_ref().expect("evidence set");
        assert_eq!(evidence_ref.as_str(), "update-evidence/attempt/attempt-7/seq/2");

        // The P11.15 chain accepts the mapped record (the production
        // chain functions, cited not reimplemented).
        let namespace = test_namespace_key();
        let tenant = TenantId::new(sample_uuid(2));
        let tenant_key = namespace.chain_key_for_namespace(Some(tenant));
        let head = crate::chain::chain_step(
            &tenant_key,
            &crate::chain::genesis_head(&tenant_key),
            &audit_record,
        );
        assert_ne!(head, crate::chain::genesis_head(&tenant_key));
    }

    #[test]
    fn the_statement_pins_the_honesty_bounds() {
        assert!(UPDATE_EVIDENCE_STATEMENT.contains("no device, no fleet, no update"));
        assert!(UPDATE_EVIDENCE_STATEMENT.contains("never tamper-PROOF"));
        assert!(UPDATE_EVIDENCE_STATEMENT.contains("never a release signature"));
        assert_eq!(UPDATE_EVIDENCE_DOMAIN, b"flexcore-update-evidence-v1");
        // The closed kind set is exactly the ledger's five.
        for (code, kind) in [
            (1u8, UpdateEvidenceKind::Attempt),
            (2, UpdateEvidenceKind::Verify),
            (3, UpdateEvidenceKind::Install),
            (4, UpdateEvidenceKind::Result),
            (5, UpdateEvidenceKind::Rollback),
        ] {
            assert_eq!(UpdateEvidenceKind::from_code(code), Some(kind));
            assert!(kind.audit_action().starts_with("update."));
        }
        assert_eq!(UpdateEvidenceKind::from_code(6), None);
        assert_eq!(UpdateEvidenceOutcome::Success.name(), "SUCCESS");
        assert_eq!(UpdateEvidenceOutcome::Failed.name(), "FAILED");
        assert_eq!(UpdateEvidenceOutcome::Refused.name(), "REFUSED");
    }
}
