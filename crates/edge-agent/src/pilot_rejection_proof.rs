// P9.31 — The pilot edge rejection PROOF surface: the attack-case runner
//         that proves — by injection, not by tabletop — that a forged,
//         replayed, expired, or out-of-sequence command is rejected at the
//         edge BEFORE it can reach any adapter (report §17.4 forged/replayed
//         command, §18.2 the edge validates every command, §14.3 signature /
//         sequence / expiry / idempotency; ledger P9.31).
//
// THIS MODULE PROVES THE EXISTING PHASE 6 CONTROLS BY EXERCISE; it does not
// re-implement them. Every rejection in the proof is PRODUCED BY the P6.15
// four-check verifier (`crate::command_verify::CommandVerifier::verify`,
// `crates/edge-agent/src/command_verify.rs`) over the P6.14 gate receipt
// (`crate::command_path::CommandReceipt`) — the named rejection codes are
// the six DISTINCT codes of `crate::command_verify::CommandVerificationRejection`
// (`signature` / `expiry` / `sequence` / `duplicate` / `allowlist` / `scope`).
// The evaluation order inside the verifier is FIXED and ends with the
// sequence check LAST (the P4.28 `SequenceGuard` rule: a refused sequence
// never advances the window) — that discipline is P9.31 acceptance
// criterion (a) and is pinned by the tests, never re-implemented here.
//
// THE ONE ADDITIVE CONTROL (ledger P9.31 step 4: "a command with a
// conflicting payload under an already-used idempotency key is rejected, not
// double-executed"): §14.3 names an idempotency key per command. The Phase 6
// verifier's replay defence is SEQUENCE-based (check 3); the command id
// itself is bound into the signature (a conflicting payload under a used id
// cannot pass check 1 unless re-signed by the real key) but the verifier has
// NO command-id memory — a legitimately re-signed command re-using an
// already-accepted command id with a DIFFERENT payload would pass all four
// checks as a fresh sequence. That gap is recorded honestly in the decision
// record (`31_edge_rejection_proof.md`) as a Phase 7 follow-up. THIS module
// adds the edge-side stand-in: the [`FileCommandIdBinding`] journal — a
// restart-surviving command-id → canonical-binding memory, in the exact
// framing discipline of `FileSequenceCheckpoint` and `FileAuditBuffer`
// (magic header + version, length-prefixed CRC-32-checked records,
// truncate-and-audit recovery, durability BEFORE state advance). A command
// whose id is already bound to a DIFFERENT canonical payload is refused with
// the named [`PilotRejectionProofError::ConflictingIdempotencyPayload`]
// before the verifier runs (so no sequence advance is ever wasted on a
// refused command) and audited. The guard NEVER substitutes for the
// verifier: every guard-passing command still runs the full four checks.
//
// HONESTY BOUNDS (AGENTS.md rule 1, §13.3 discipline):
// - The proof is a TEST SUITE — a finite test set. Zero observed
//   rejections-missed in the suite is a TEST OUTCOME, not a guarantee that
//   no injection outside the suite can pass. What the suite establishes and
//   what it does NOT are itemised in `31_edge_rejection_proof.md`.
// - The signature underneath is still the P6.15 keyed-checksum scheme
//   (`edge-sig:...:fnv1a64:<digest>`) — tamper-EVIDENCE against casual
//   tampering, NOT a cryptographic guarantee. Replacing it under reviewed
//   key management remains the recorded deployment requirement (P6.15
//   module header; unchanged here).
// - The idempotency binding journal is an EDGE-LOCAL stand-in; the
//   platform-side §14.3 idempotency-key registry (authoritative, cross-edge)
//   is the Phase 7 follow-up.
//
// NOTHING REACHES AN ADAPTER FROM HERE: this module holds no forwarder, no
// queue, and no write path. A rejection is a return value plus an audit
// record; an accepted attack is the fail-loud [`AttackOutcome::ErroneouslyAccepted`]
// (never suppressed, never coerced into a pass). The structural fact the
// proof rests on is that the ONLY route from a gate-admitted command to an
// adapter is a `VerifiedCommand` through the P6.16 engine's separate
// allowlist/policy re-checks (`crate::command_forward::CommandForwardingEngine::forward_verified`)
// — and a rejected verification produces no `VerifiedCommand` at all.

use std::collections::BTreeMap;
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use flexcore_contracts::time_model::ClockQuality;
use flexcore_ports::clock::Clock;

use crate::command_path::CommandReceipt;
use crate::command_verify::{
    canonical_command_binding, IncomingCommand, VerifiedCommand,
};
use crate::ports::{AuditSink, EdgeAuditEvent, EdgeAuditKind};

// --- Constants -----------------------------------------------------------------

/// Hard sanity cap on one binding-journal record payload (rejects absurd
/// lengths from a corrupt header instead of attempting an absurd allocation).
const MAX_BINDING_RECORD_BYTES: u32 = 64 * 1024;
/// Default bound on the number of DISTINCT command ids the binding journal
/// tracks. A full guard refuses NEW bindings fail-closed (never drop-oldest:
/// dropping a binding would re-open that id for a conflicting payload) — the
/// deployment sizes this above its command volume and rotates journals.
pub const DEFAULT_ID_BINDING_CAPACITY: usize = 100_000;

// --- The closed attack-case vocabulary -------------------------------------------

/// The P9.31 attack cases (ledger steps 1–4, plus the recorded subtle
/// variants). CLOSED set: extending it is a reviewed change. Each case maps
/// to the named rejection code(s) the EXISTING Phase 6 controls must
/// produce — the mapping is asserted by the proof tests, not enforced here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PilotAttackCase {
    /// A forged command: valid shape, invalid or foreign signature (ledger
    /// step 2). Covers the forged-but-otherwise-valid payload variant: every
    /// other field fully valid, only the signature wrong.
    ForgedSignature,
    /// A replayed captured command: same id, same signature, resent after it
    /// was already accepted (ledger step 1) — including the replay-after-
    /// restart variant across the `FileSequenceCheckpoint` and the binding
    /// journal.
    ReplayedCaptured,
    /// An expired command: valid-until elapsed against the INJECTED edge
    /// clock (never wall-clock time; ledger step 3, §17.4 clock-drift threat
    /// handled by the P6.31 clock-quality seam).
    ExpiredWindow,
    /// An out-of-sequence command: sequence number behind the last accepted
    /// one for the asset (ledger step 3) — gap of 1 and gap of N alike,
    /// checked LAST per the recorded four-check discipline.
    OutOfSequence,
    /// A conflicting payload under an already-used idempotency key
    /// (ledger step 4): the command id is already bound to a different
    /// canonical payload. The additive [`FileCommandIdBinding`] guard refuses
    /// it BEFORE the verifier runs; the platform-side registry is the
    /// recorded Phase 7 follow-up.
    ConflictingIdempotencyPayload,
}

impl PilotAttackCase {
    /// The stable attack-case code (audit text, greppable).
    pub fn code(self) -> &'static str {
        match self {
            PilotAttackCase::ForgedSignature => "forged_signature",
            PilotAttackCase::ReplayedCaptured => "replayed_captured",
            PilotAttackCase::ExpiredWindow => "expired_window",
            PilotAttackCase::OutOfSequence => "out_of_sequence",
            PilotAttackCase::ConflictingIdempotencyPayload => "conflicting_idempotency_payload",
        }
    }

    /// The stable audit event name for a REJECTED attempt of this case.
    pub fn rejected_event_name(self) -> &'static str {
        "EDGE_PILOT_ATTACK_REJECTED"
    }

    /// The named rejection code(s) the EXISTING controls are expected to
    /// produce for this case (the mapping the proof tests assert). The
    /// conflicting-payload case is refused by the ADDITIVE binding guard
    /// (its own code), not by the four-check verifier.
    pub fn expected_rejection_codes(self) -> &'static [&'static str] {
        match self {
            PilotAttackCase::ForgedSignature => &["signature"],
            PilotAttackCase::ReplayedCaptured => &["duplicate"],
            PilotAttackCase::ExpiredWindow => &["expiry"],
            PilotAttackCase::OutOfSequence => &["sequence"],
            PilotAttackCase::ConflictingIdempotencyPayload => &["idempotency_conflict"],
        }
    }
}

// --- Errors -----------------------------------------------------------------------

/// Named errors of the proof surface. Every failure is named; nothing
/// implies a silent pass. No error ever contains a key value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PilotRejectionProofError {
    /// The attack input was unusable (empty identity, empty binding).
    InvalidAttackInput { reason: String },
    /// The binding journal could not be opened/loaded; the prover refuses
    /// to run attacks without idempotency memory (fail-closed — an attack
    /// against a guard-less edge is not evidence).
    BindingJournalUnavailable { reason: String },
    /// The binding journal is full: a NEW command id cannot be bound
    /// fail-closed. Never drop-oldest (dropping a binding re-opens that id
    /// for a conflicting payload).
    BindingGuardFull { capacity: usize },
    /// The binding journal I/O failed (the bind was NOT persisted).
    BindingJournalIo { reason: String },
    /// The file exists but is not a FlexCore edge command-id binding journal
    /// (wrong magic or version) — refused, never guessed at.
    NotABindingJournal { reason: String },
    /// THE ATTACK-SURFACE REFUSAL (ledger step 4): the command id is already
    /// bound to a DIFFERENT canonical payload. This is a REJECTION of the
    /// command, surfaced as the named error the prover records.
    ConflictingIdempotencyPayload {
        command_id: String,
        /// The audit reference of the refusal (the `command-denial:`-style
        /// reference the audit line carries).
        audit_reference: String,
    },
}

impl std::fmt::Display for PilotRejectionProofError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PilotRejectionProofError::InvalidAttackInput { reason } => {
                write!(f, "the attack input is invalid: {reason}")
            }
            PilotRejectionProofError::BindingJournalUnavailable { reason } => write!(
                f,
                "the command-id binding journal is unavailable; attacks are refused fail-closed: \
                 {reason}"
            ),
            PilotRejectionProofError::BindingGuardFull { capacity } => write!(
                f,
                "the command-id binding guard is at capacity {capacity}; a new binding is \
                 refused fail-closed (never drop-oldest — a dropped binding re-opens the id)"
            ),
            PilotRejectionProofError::BindingJournalIo { reason } => {
                write!(f, "the command-id binding journal I/O failed: {reason}")
            }
            PilotRejectionProofError::NotABindingJournal { reason } => write!(
                f,
                "the file is not a FlexCore edge command-id binding journal (refused, never \
                 guessed): {reason}"
            ),
            PilotRejectionProofError::ConflictingIdempotencyPayload {
                command_id,
                audit_reference,
            } => write!(
                f,
                "command '{command_id}' re-uses an already-bound idempotency id with a \
                 CONFLICTING payload — refused, not double-executed (§14.3; ledger P9.31 \
                 step 4); audit reference {audit_reference}"
            ),
        }
    }
}

impl std::error::Error for PilotRejectionProofError {}

// --- The command-id binding journal (the additive §14.3 edge stand-in) --------------

/// The file-backed command-id → canonical-binding journal. Same framing
/// discipline as `FileSequenceCheckpoint` (`command_verify.rs`) and
/// `FileAuditBuffer` (`audit_buffer.rs`):
///
/// ```text
/// header:  b"FLEXCORE-EDGE-COMMAND-ID-BINDING" + format version byte (0x01)
/// record:  [u32 LE payload_len][u32 LE crc32(payload)][payload]
/// payload: [u16 LE id_len][id bytes][u32 LE binding_len][binding bytes]
/// ```
///
/// **Recovery (truncate-and-audit, never silent).** On open the file is
/// replayed record by record; the FIRST malformed frame ends the replay and
/// the file is TRUNCATED at the last good record, with the loss counted
/// ([`FileCommandIdBinding::corrupt_tail_truncated`]) and logged loudly. A
/// wrong magic/version is a hard open error. The last record per id wins
/// (the store is append-only; replay keeps the newest binding).
///
/// **Durability BEFORE state.** A binding record is flushed and
/// `sync_data`-ed before the in-memory map moves — a crash cannot lose a
/// binding the caller was told was recorded, so a conflicting payload cannot
/// slip through a crash window.
pub struct FileCommandIdBinding {
    path: PathBuf,
    file: Mutex<std::fs::File>,
    bindings: Mutex<BTreeMap<String, String>>,
    capacity: usize,
    corrupt_tail_truncated: std::sync::atomic::AtomicU64,
    records_written: std::sync::atomic::AtomicU64,
}

impl FileCommandIdBinding {
    /// The journal magic header (31-byte label) + format version.
    fn magic() -> Vec<u8> {
        let mut magic = b"FLEXCORE-EDGE-COMMAND-ID-BINDING".to_vec();
        magic.push(0x01);
        magic
    }

    /// Opens (or creates) the binding journal at `path` and replays any
    /// existing bindings, bounded by `capacity` DISTINCT ids.
    pub fn open(path: &Path, capacity: usize) -> Result<Self, PilotRejectionProofError> {
        if capacity == 0 {
            return Err(PilotRejectionProofError::BindingJournalUnavailable {
                reason: "the binding capacity must be non-zero".to_string(),
            });
        }
        if path.as_os_str().is_empty() {
            return Err(PilotRejectionProofError::InvalidAttackInput {
                reason: "the binding journal path is empty".to_string(),
            });
        }
        let magic = Self::magic();
        let (file, replayed) = match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
        {
            Ok(mut existing) => {
                use std::io::Read;
                let mut contents = Vec::new();
                existing.read_to_end(&mut contents).map_err(|e| {
                    PilotRejectionProofError::BindingJournalIo {
                        reason: format!("reading the existing binding journal failed: {e}"),
                    }
                })?;
                if contents.is_empty() {
                    // A zero-length file (touch/new) is initialized, not refused.
                    (existing, BTreeMap::new())
                } else {
                    if contents.len() < magic.len() || contents[..magic.len()] != magic[..] {
                        return Err(PilotRejectionProofError::NotABindingJournal {
                            reason: format!(
                                "the file at {} does not carry the FlexCore edge command-id \
                                 binding journal magic",
                                path.display()
                            ),
                        });
                    }
                    let mut bindings = BTreeMap::new();
                    let mut offset = magic.len();
                    let mut corrupt_tail_truncated: u64 = 0;
                    while offset + 8 <= contents.len() {
                        let len = u32::from_le_bytes(
                            contents[offset..offset + 4].try_into().expect("4 bytes"),
                        );
                        if len > MAX_BINDING_RECORD_BYTES
                            || offset + 8 + len as usize > contents.len()
                        {
                            corrupt_tail_truncated += 1;
                            break;
                        }
                        let payload = &contents[offset + 8..offset + 8 + len as usize];
                        let stored_crc = u32::from_le_bytes(
                            contents[offset + 4..offset + 8].try_into().expect("4 bytes"),
                        );
                        if crc32(payload) != stored_crc {
                            corrupt_tail_truncated += 1;
                            break;
                        }
                        match decode_binding_payload(payload) {
                            Some((command_id, binding)) => {
                                bindings.insert(command_id, binding);
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
                        tracing::warn!(
                            truncated_records = corrupt_tail_truncated,
                            path = %path.display(),
                            "command-id binding journal: corrupt tail truncated; the affected \
                             ids' bindings reverted to their last good record"
                        );
                        existing.set_len(offset as u64).map_err(|e| {
                            PilotRejectionProofError::BindingJournalIo {
                                reason: format!(
                                    "truncating the corrupt binding-journal tail failed: {e}"
                                ),
                            }
                        })?;
                    }
                    (existing, bindings)
                }
            }
            Err(_) => {
                let mut created = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .map_err(|e| PilotRejectionProofError::BindingJournalIo {
                        reason: format!("creating the binding journal file failed: {e}"),
                    })?;
                created
                    .write_all(&magic)
                    .and_then(|_| created.flush())
                    .map_err(|e| PilotRejectionProofError::BindingJournalIo {
                        reason: format!("writing the binding journal header failed: {e}"),
                    })?;
                (created, BTreeMap::new())
            }
        };
        Ok(Self {
            path: path.to_path_buf(),
            file: Mutex::new(file),
            bindings: Mutex::new(replayed),
            capacity,
            corrupt_tail_truncated: std::sync::atomic::AtomicU64::new(0),
            records_written: std::sync::atomic::AtomicU64::new(0),
        })
    }

    /// The journal path (diagnostics).
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// How many corrupt tail records were truncated at open (the audited
    /// loss counter — the buffer_file discipline).
    pub fn corrupt_tail_truncated(&self) -> u64 {
        self.corrupt_tail_truncated
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// How many records this instance has written (diagnostics).
    pub fn records_written(&self) -> u64 {
        self.records_written
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// The number of DISTINCT bound command ids.
    pub fn len(&self) -> usize {
        self.bindings
            .lock()
            .expect("poisoned binding map mutex")
            .len()
    }

    /// Whether no id is bound.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The canonical payload an id is bound to (diagnostics/proof reading).
    pub fn binding_of(&self, command_id: &str) -> Option<String> {
        self.bindings
            .lock()
            .expect("poisoned binding map mutex")
            .get(command_id)
            .cloned()
    }

    /// Binds `command_id` to its canonical payload string. A REBIND with a
    /// different payload is the named
    /// [`PilotRejectionProofError::ConflictingIdempotencyPayload`] — the
    /// command is refused, nothing is overwritten in place. A rebind with
    /// the SAME payload (a byte-identical replay arriving at the guard) is
    /// accepted by the guard itself — the four-check verifier is what
    /// rejects it as a duplicate sequence. The record is durable BEFORE the
    /// in-memory map moves.
    pub fn bind(
        &self,
        command_id: &str,
        canonical_binding: &str,
        at_unix_millis: i64,
    ) -> Result<(), PilotRejectionProofError> {
        if command_id.trim().is_empty() || canonical_binding.trim().is_empty() {
            return Err(PilotRejectionProofError::InvalidAttackInput {
                reason: "the command id and the canonical binding are mandatory".to_string(),
            });
        }
        let mut bindings = self
            .bindings
            .lock()
            .expect("poisoned binding map mutex");
        if let Some(existing) = bindings.get(command_id) {
            if existing != canonical_binding {
                let audit_reference = format!(
                    "command-id-binding-conflict:{command_id}@{at_unix_millis}/\
                     idempotency_conflict"
                );
                tracing::warn!(
                    command_id,
                    "COMMAND-ID BINDING CONFLICT: a command re-uses an already-bound \
                     idempotency id with a DIFFERENT canonical payload — refused, not \
                     double-executed (§14.3; ledger P9.31 step 4)"
                );
                return Err(PilotRejectionProofError::ConflictingIdempotencyPayload {
                    command_id: command_id.to_string(),
                    audit_reference,
                });
            }
            // Same payload: the guard lets it through to the verifier (which
            // owns the replay verdict). Nothing new to persist.
            return Ok(());
        }
        if bindings.len() >= self.capacity {
            return Err(PilotRejectionProofError::BindingGuardFull {
                capacity: self.capacity,
            });
        }
        let payload = encode_binding_payload(command_id, canonical_binding);
        let record_len = payload.len() as u32;
        let crc = crc32(&payload);
        let mut record = Vec::with_capacity(8 + payload.len());
        record.extend_from_slice(&record_len.to_le_bytes());
        record.extend_from_slice(&crc.to_le_bytes());
        record.extend_from_slice(&payload);
        {
            let mut file = self.file.lock().expect("poisoned binding file mutex");
            file.seek(SeekFrom::End(0))
                .and_then(|_| file.write_all(&record))
                .and_then(|_| file.flush())
                .and_then(|_| file.sync_data())
                .map_err(|e| PilotRejectionProofError::BindingJournalIo {
                    reason: format!("appending the binding record failed: {e}"),
                })?;
        }
        bindings.insert(command_id.to_string(), canonical_binding.to_string());
        self.records_written
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

fn encode_binding_payload(command_id: &str, binding: &str) -> Vec<u8> {
    let mut payload = Vec::with_capacity(2 + command_id.len() + 4 + binding.len());
    payload.extend_from_slice(&(command_id.len() as u16).to_le_bytes());
    payload.extend_from_slice(command_id.as_bytes());
    payload.extend_from_slice(&(binding.len() as u32).to_le_bytes());
    payload.extend_from_slice(binding.as_bytes());
    payload
}

fn decode_binding_payload(payload: &[u8]) -> Option<(String, String)> {
    if payload.len() < 2 {
        return None;
    }
    let id_len = u16::from_le_bytes(payload[0..2].try_into().expect("2 bytes")) as usize;
    if payload.len() < 2 + id_len + 4 {
        return None;
    }
    let command_id = std::str::from_utf8(&payload[2..2 + id_len]).ok()?.to_string();
    let rest = &payload[2 + id_len..];
    let binding_len =
        u32::from_le_bytes(rest[0..4].try_into().expect("4 bytes")) as usize;
    if rest.len() < 4 + binding_len {
        return None;
    }
    let binding = std::str::from_utf8(&rest[4..4 + binding_len]).ok()?.to_string();
    Some((command_id, binding))
}

/// CRC-32 (IEEE, reflected) — the same checksum family as the checkpoint and
/// the audit journal (an integrity checksum for corruption detection, not a
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

// --- The idempotency binding digest --------------------------------------------------

/// The canonical binding of an incoming command: the P6.15
/// `canonical_command_binding` string (command id, asset, sequence, adapter,
/// quantity, exact value bit pattern, transport identity) — every field that
/// defines the command, in stable order. Reused by citation; never forked.
pub fn idempotency_binding(command: &IncomingCommand) -> String {
    canonical_command_binding(command)
}

// --- The attack attempt, outcome, and record -------------------------------------------

/// One recorded attack attempt (the attempt → outcome → audit-reference
/// triple the ledger deliverable names).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttackAttempt {
    /// The attack case.
    pub case: PilotAttackCase,
    /// The command id presented by the attack.
    pub command_id: String,
    /// The asset key the attack targeted.
    pub asset_key: String,
    /// The adapter the attack targeted.
    pub target_adapter: String,
    /// The sequence the attack presented.
    pub presented_sequence: u64,
    /// When the attempt was injected (the INJECTED clock — never wall time).
    pub attempted_at_unix_millis: i64,
}

impl AttackAttempt {
    /// The attempt summary line (audit text prefix).
    pub fn summary(&self) -> String {
        format!(
            "attack={} command={} asset={} adapter={} seq={}",
            self.case.code(),
            self.command_id,
            self.asset_key,
            self.target_adapter,
            self.presented_sequence,
        )
    }
}

/// The outcome of one attack attempt. An acceptance is a FAIL-LOUD outcome,
/// never coerced into a pass and never suppressed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttackOutcome {
    /// The EXISTING four-check verifier rejected the command with its named
    /// DISTINCT reason code (`signature`/`expiry`/`sequence`/`duplicate`/
    /// `allowlist`/`scope`) — produced by
    /// `crate::command_verify::CommandVerifier::verify`, not re-implemented.
    RejectedAtVerification {
        rejection_code: &'static str,
        rejection_detail: String,
    },
    /// The ADDITIVE idempotency binding guard refused the command (a
    /// conflicting payload under an already-used id) — the P9.31 step-4
    /// control.
    RejectedByIdempotencyGuard { audit_reference: String },
    /// THE FAIL-LOUD OUTCOME: the attack PASSED all controls. The prover
    /// records it, audits it, and returns it — the composition's job is to
    /// treat this as an alarm, and the proof suite treats it as a failure.
    ErroneouslyAccepted { command_id: String, sequence: u64 },
}

impl AttackOutcome {
    /// Whether the attempt was rejected (by either control).
    pub fn was_rejected(&self) -> bool {
        matches!(
            self,
            AttackOutcome::RejectedAtVerification { .. }
                | AttackOutcome::RejectedByIdempotencyGuard { .. }
        )
    }

    /// The named rejection code, if rejected (a verifier code or the
    /// additive `idempotency_conflict` code).
    pub fn rejection_code(&self) -> Option<&'static str> {
        match self {
            AttackOutcome::RejectedAtVerification { rejection_code, .. } => Some(rejection_code),
            AttackOutcome::RejectedByIdempotencyGuard { .. } => Some("idempotency_conflict"),
            AttackOutcome::ErroneouslyAccepted { .. } => None,
        }
    }
}

/// One attack record: attempt → outcome → audit reference (the ledger
/// deliverable's per-case shape).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttackRecord {
    pub attempt: AttackAttempt,
    pub outcome: AttackOutcome,
    /// The audit reference the rejection (or fail-loud acceptance) was
    /// recorded under — the greppable join between this record and the
    /// audit sink.
    pub audit_reference: String,
}

impl AttackRecord {
    /// Whether this record is the SUCCESS branch of the proof (rejected with
    /// an expected code).
    pub fn rejected_with_expected_code(&self) -> bool {
        self.outcome.was_rejected()
            && self
                .attempt
                .case
                .expected_rejection_codes()
                .contains(&self.outcome.rejection_code().unwrap_or(""))
    }

    /// The full audit text (the record an operator reads).
    pub fn audit_text(&self) -> String {
        let outcome_text = match &self.outcome {
            AttackOutcome::RejectedAtVerification {
                rejection_code,
                rejection_detail,
            } => format!("REJECTED code={rejection_code} ({rejection_detail})"),
            AttackOutcome::RejectedByIdempotencyGuard { audit_reference } => {
                format!("REJECTED code=idempotency_conflict ({audit_reference})")
            }
            AttackOutcome::ErroneouslyAccepted { command_id, sequence } => format!(
                "ACCEPTED — ALARM: the attack was not rejected (command={command_id}, \
                 sequence={sequence})"
            ),
        };
        format!("{} :: {}", self.attempt.summary(), outcome_text)
    }
}

// --- The prover -------------------------------------------------------------------------

/// The P9.31 attack runner over a composed P6.15 verifier. Holds NO queue
/// and NO forwarder: a rejected command is retained by nothing; an accepted
/// command is bound (idempotency) and reported — never forwarded by this
/// surface.
pub struct PilotRejectionProver {
    bindings: Arc<FileCommandIdBinding>,
    audit: Arc<dyn AuditSink>,
    clock: Arc<dyn Clock + Send + Sync>,
}

impl PilotRejectionProver {
    /// Composes the prover. Fail-closed on an unusable binding journal: the
    /// prover refuses to run attacks without idempotency memory (an attack
    /// against a guard-less edge is not evidence).
    pub fn new(
        bindings: Arc<FileCommandIdBinding>,
        audit: Arc<dyn AuditSink>,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Result<Self, PilotRejectionProofError> {
        if bindings.capacity == 0 {
            return Err(PilotRejectionProofError::BindingJournalUnavailable {
                reason: "the binding journal was built with zero capacity".to_string(),
            });
        }
        let prover = Self {
            bindings,
            audit,
            clock,
        };
        prover.audit_event(
            "EDGE_PILOT_REJECTION_PROVER_COMPOSED",
            format!(
                "pilot rejection prover composed (binding journal='{}', bound ids={}, \
                 attacks run against the composed P6.15 verifier; test-env only)",
                prover.bindings.path().display(),
                prover.bindings.len(),
            ),
        );
        Ok(prover)
    }

    /// Runs ONE attack attempt against the composed verifier. The command
    /// must already carry a P6.14 gate receipt (the verifier is only ever
    /// fed gate-admitted commands in composition — the proof does not
    /// bypass the gate; the gate's own refusal codes are the P6.14 surface,
    /// not this one).
    ///
    /// Order of controls (recorded):
    /// 1. the ADDITIVE idempotency binding guard (a conflicting payload
    ///    under an already-used id is refused BEFORE the verifier so no
    ///    sequence advance is ever spent on a refused command);
    /// 2. the EXISTING four-check verifier — its named rejection or its
    ///    `VerifiedCommand`.
    /// On a guard pass the command is bound (durable before memory) and the
    /// verifier verdict is recorded. An acceptance is the fail-loud
    /// [`AttackOutcome::ErroneouslyAccepted`].
    pub fn run_attack(
        &mut self,
        verifier: &mut crate::command_verify::CommandVerifier,
        receipt: &CommandReceipt,
        command: &IncomingCommand,
        case: PilotAttackCase,
        clock_quality: ClockQuality,
    ) -> Result<AttackRecord, PilotRejectionProofError> {
        let now = self.now_unix_millis();
        let command_id = command.edge_command.command_id.clone();
        if command_id.trim().is_empty() {
            return Err(PilotRejectionProofError::InvalidAttackInput {
                reason: "the attack command carries an empty command id".to_string(),
            });
        }
        let attempt = AttackAttempt {
            case,
            command_id: command_id.clone(),
            asset_key: command.edge_command.asset_key.clone(),
            target_adapter: command.target_adapter.clone(),
            presented_sequence: command.edge_command.source_sequence,
            attempted_at_unix_millis: now,
        };

        // 1. The ADDITIVE idempotency binding guard (ledger step 4).
        let binding = idempotency_binding(command);
        if let Err(error) = self.bindings.bind(&command_id, &binding, now) {
            match error {
                // THE P9.31 step-4 rejection: a conflicting payload under an
                // already-used id. Audited; the verifier is never reached.
                PilotRejectionProofError::ConflictingIdempotencyPayload {
                    ref audit_reference,
                    ..
                } => {
                    self.audit_event(
                        "EDGE_COMMAND_ID_BINDING_CONFLICT",
                        format!(
                            "{} :: REJECTED code=idempotency_conflict (conflicting payload under \
                             an already-used idempotency id; not double-executed)",
                            attempt.summary()
                        ),
                    );
                    return Ok(AttackRecord {
                        attempt,
                        outcome: AttackOutcome::RejectedByIdempotencyGuard {
                            audit_reference: audit_reference.clone(),
                        },
                        audit_reference: audit_reference.clone(),
                    });
                }
                // Capacity/IO/availability failures are NOT rejections: they
                // are named errors — the attack is NOT evidence either way.
                other => return Err(other),
            }
        }

        // 2. The EXISTING four-check verifier (cited, exercised, never
        //    re-implemented). Its rejection codes ARE the proof.
        match verifier.verify(receipt, command, clock_quality) {
            Err(rejection) => {
                let audit_reference = format!(
                    "pilot-attack:{}/{command_id}@{now}/{}",
                    case.code(),
                    rejection.code(),
                );
                self.audit_event(
                    case.rejected_event_name(),
                    format!(
                        "{} :: REJECTED code={} :: ref {audit_reference} :: {}",
                        attempt.summary(),
                        rejection.code(),
                        rejection.detail(),
                    ),
                );
                Ok(AttackRecord {
                    attempt,
                    outcome: AttackOutcome::RejectedAtVerification {
                        rejection_code: rejection.code(),
                        rejection_detail: rejection.detail(),
                    },
                    audit_reference,
                })
            }
            Ok(verified) => {
                let audit_reference = format!(
                    "pilot-attack:{}/{command_id}@{now}/ACCEPTED-ALARM",
                    case.code(),
                );
                self.audit_event(
                    "EDGE_PILOT_ATTACK_ACCEPTED",
                    format!(
                        "{} :: ACCEPTED — ALARM: the attack was not rejected :: ref \
                         {audit_reference} (the composition MUST treat this as an incident; \
                         the proof suite treats it as a failure)",
                        attempt.summary(),
                    ),
                );
                Ok(AttackRecord {
                    attempt,
                    outcome: AttackOutcome::ErroneouslyAccepted {
                        command_id: verified.command_id,
                        sequence: verified.sequence,
                    },
                    audit_reference,
                })
            }
        }
    }

    /// Binds a LEGITIMATE accepted command (the composition calls this after
    /// a normal, non-attack verification passes, so the idempotency memory
    /// covers the real command flow too). Exposed separately from
    /// [`PilotRejectionProver::run_attack`] so the proof surface never
    /// conflates the real flow with the attack flow.
    pub fn bind_accepted(
        &self,
        verified: &VerifiedCommand,
        command: &IncomingCommand,
    ) -> Result<(), PilotRejectionProofError> {
        let now = self.now_unix_millis();
        let binding = idempotency_binding(command);
        let command_id = &verified.command_id;
        match self.bindings.bind(command_id, &binding, now) {
            Ok(()) => {
                self.audit_event(
                    "EDGE_COMMAND_ID_BOUND",
                    format!(
                        "command={command_id} bound to its canonical payload (idempotency \
                         memory, §14.3 edge stand-in)"
                    ),
                );
                Ok(())
            }
            Err(PilotRejectionProofError::ConflictingIdempotencyPayload {
                ref audit_reference,
                ..
            }) => {
                self.audit_event(
                    "EDGE_COMMAND_ID_BINDING_CONFLICT",
                    format!(
                        "command={command_id} :: REJECTED code=idempotency_conflict :: ref \
                         {audit_reference} (conflicting payload under an already-used \
                         idempotency id)"
                    ),
                );
                Err(PilotRejectionProofError::ConflictingIdempotencyPayload {
                    command_id: command_id.clone(),
                    audit_reference: audit_reference.clone(),
                })
            }
            Err(other) => Err(other),
        }
    }

    /// The binding journal (proof/test reading: the restart-survival tests
    /// reopen it directly).
    pub fn bindings(&self) -> &Arc<FileCommandIdBinding> {
        &self.bindings
    }

    fn now_unix_millis(&self) -> i64 {
        let stamp = self.clock.now_ingest_time();
        i64::try_from(stamp.unix_millis()).unwrap_or(i64::MAX)
    }

    /// Records one prover audit event through the edge `AuditSink` port
    /// (same audit-kind limitation as the P6.14/P6.15 modules: `EdgeAuditKind`
    /// is a closed vocabulary this wave does not own, so the dedicated event
    /// name LEADS the detail string on kind `EDGE_STARTUP`).
    fn audit_event(&self, name: &'static str, detail: String) {
        let event = EdgeAuditEvent {
            recorded_at: self.clock.now_ingest_time(),
            kind: EdgeAuditKind::Startup,
            detail: format!("{name}: {detail}"),
        };
        if let Err(error) = self.audit.record(event) {
            tracing::error!(%error, audit_event = name, "pilot-rejection-prover audit write FAILED");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_attack_case_codes_and_expected_rejections_are_pairwise_consistent() {
        let cases = [
            PilotAttackCase::ForgedSignature,
            PilotAttackCase::ReplayedCaptured,
            PilotAttackCase::ExpiredWindow,
            PilotAttackCase::OutOfSequence,
            PilotAttackCase::ConflictingIdempotencyPayload,
        ];
        let mut codes: Vec<&str> = cases.iter().map(|c| c.code()).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), 5, "every attack case has a DISTINCT code: {codes:?}");
        for case in cases {
            assert!(
                !case.expected_rejection_codes().is_empty(),
                "case {} maps to at least one expected rejection code",
                case.code()
            );
        }
    }

    #[test]
    fn the_binding_journal_round_trips_and_survives_reopen() {
        let dir = std::env::temp_dir().join(format!(
            "flexcore-edge-p931-unit-{}-round-trip",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("scratch dir creates");
        let path = dir.join("id-bindings.journal");
        {
            let journal = FileCommandIdBinding::open(&path, 100).expect("opens");
            assert_eq!(journal.corrupt_tail_truncated(), 0);
            journal
                .bind("cmd-1", "edge-cmd:cmd-1|asset:ess-01|seq:1", 1_000)
                .expect("binds");
            journal
                .bind("cmd-2", "edge-cmd:cmd-2|asset:ess-01|seq:2", 1_000)
                .expect("binds");
        }
        let reopened = FileCommandIdBinding::open(&path, 100).expect("reopens");
        assert_eq!(reopened.len(), 2);
        assert_eq!(
            reopened.binding_of("cmd-1"),
            Some("edge-cmd:cmd-1|asset:ess-01|seq:1".to_string())
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_conflicting_rebind_is_refused_and_nothing_is_overwritten() {
        let dir = std::env::temp_dir().join(format!(
            "flexcore-edge-p931-unit-{}-conflict",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("scratch dir creates");
        let path = dir.join("id-bindings.journal");
        let journal = FileCommandIdBinding::open(&path, 100).expect("opens");
        journal
            .bind("cmd-1", "edge-cmd:cmd-1|asset:ess-01|seq:1", 1_000)
            .expect("binds");
        let error = journal
            .bind("cmd-1", "edge-cmd:cmd-1|asset:ess-01|seq:2", 1_000)
            .expect_err("a conflicting rebind is refused");
        assert!(matches!(
            error,
            PilotRejectionProofError::ConflictingIdempotencyPayload { .. }
        ));
        assert_eq!(
            journal.binding_of("cmd-1"),
            Some("edge-cmd:cmd-1|asset:ess-01|seq:1".to_string()),
            "nothing was overwritten in place"
        );
        // A same-payload rebind passes the guard (the verifier owns the
        // replay verdict).
        journal
            .bind("cmd-1", "edge-cmd:cmd-1|asset:ess-01|seq:1", 1_000)
            .expect("a same-payload rebind passes the guard");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_foreign_file_is_refused_not_guessed() {
        let dir = std::env::temp_dir().join(format!(
            "flexcore-edge-p931-unit-{}-foreign",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("scratch dir creates");
        let path = dir.join("not-a-binding-journal");
        std::fs::write(&path, b"something else entirely").expect("writes");
        assert!(matches!(
            FileCommandIdBinding::open(&path, 100),
            Err(PilotRejectionProofError::NotABindingJournal { .. })
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
