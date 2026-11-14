// P6.15 — Command verification at the edge (test-env only): signature,
//         expiry with the recorded clock-quality check, monotonic sequence
//         with a persisted restart checkpoint, and allowlist + scope
//         (report §18.1, §17.3, §17.4, §14.3, §17.5 rules 5–6; ledger
//         P6.15).
//
// This module is the FOUR-CHECK VERIFIER run on every command that passed
// the P6.14 gate ([`crate::command_path::CommandReceiver`]) BEFORE any
// forwarding. Nothing here forwards, stores, or queues a command: a
// verification failure is a return value plus an audit record, and the
// command is dropped (retained by nothing).
//
// THE FOUR CHECKS, each with a DISTINCT named rejection reason — never one
// combined "valid" boolean (the ledger names that as the risk: a single
// boolean hides which check failed):
//
// | # | Check | Rejection reason (code) |
// |---|---|---|
// | 1 | Payload-bound signature against the central platform's key reference — unsigned, malformed, wrong signer, or tampered payload all refuse | [`CommandVerificationRejection::Signature`] (`signature`) |
// | 2 | `valid_from`/`valid_until` against the edge clock WITH the recorded clock-quality check — a degraded/unsynchronized edge clock FAILS CLOSED (§17.4, §17.5 rule 5); covers the command window AND the authorization reference's validity | [`CommandVerificationRejection::Expiry`] (`expiry`) |
// | 3 | Monotonic sequence per asset/control channel — a sequence equal to the last accepted is a DUPLICATE, a lower one is out of sequence; state survives an edge restart through the persisted checkpoint | [`CommandVerificationRejection::Duplicate`] (`duplicate`) / [`CommandVerificationRejection::Sequence`] (`sequence`) |
// | 4 | Target adapter on the site allowlist + command within the approved scope/limits/expiry of its authorization reference (§17.5 rule 6) | [`CommandVerificationRejection::Allowlist`] (`allowlist`) / [`CommandVerificationRejection::Scope`] (`scope`) |
//
// Evaluation order is FIXED: (1) signature → (2) expiry/clock → (4a)
// allowlist → (4b) scope → (3) sequence LAST. The sequence check is placed
// last so a command refused by ANY earlier check never advances the
// sequence window (the P4.28 `SequenceGuard` rule: a refused sequence never
// moves the state — fail-closed, no attacker-driven window advance). The
// ledger numbers sequence as check 3; the numbering here is a deliberate,
// recorded ordering decision, and every reason is still distinct.
//
// SIGNATURE HONESTY (AGENTS.md rule 1, the P4.28 pattern reused by
// citation): the signature is the payload-bound
// `edge-sig:<key_reference>:<command_id>:fnv1a64:<digest>` reference over
// the canonical command binding — the same NON-cryptographic keyed-checksum
// shape as the P4.28 `sim-sig:` reference and the P6.14 enablement digest.
// It detects corruption and casual tampering; it is NOT proof against a
// determined adversary. Replacing it with a real signature (HMAC-SHA-256 or
// an asymmetric scheme) under reviewed key management is a DEPLOYMENT
// REQUIREMENT; the versioned scheme prefix makes that replacement a
// detectable format change, never a silent one.
//
// KEY MATERIAL IS A DEPLOYMENT REQUIREMENT: the FlexCore code base
// generates, stores, ships, and distributes NO key material (§17.3). The
// deployment provisions the central platform's command-signing key
// out-of-band; the composition root loads it into [`CommandSigningKey`].
// The key never renders (`Debug`/`Display` redact) and never enters an
// audit line.
//
// CLOCK QUALITY IS A SUPPLIED INPUT (the P6.31 seam): the ledger's P6.31
// (edge time synchronization and clock-quality monitoring) owns the
// clock-quality status work that this verifier depends on. This module
// takes the recorded [`ClockQuality`] as a parameter — it does NOT detect
// drift, query NTP, or implement P6.31. The rule implemented here is the
// §17.5 rule 5 fail-closed one: any quality other than `Synchronized`
// (including `Unknown` — never silently assumed good, P0.09 §4) fails the
// expiry check.
//
// SEQUENCE CHECKPOINT PERSISTENCE: the per-channel last-accepted sequence
// survives an edge restart through the [`SequenceCheckpointStore`] trait;
// [`FileSequenceCheckpoint`] is the file-backed implementation, in the same
// framing discipline as `buffer_file.rs` (magic header + version,
// length-prefixed CRC-32-checked records, truncate-and-audit recovery,
// durability BEFORE state advance). Without the checkpoint, an edge restart
// would reset the window and allow replays of already-accepted commands —
// exactly the ledger's recorded risk.
//
// TEST-ENV ONLY: this whole surface exists for the test-environment command
// path behind the P6.14 gate. Verification is not delivery; acknowledgement
// is not physical delivery (§14.1). Nothing here is a field-write path.

use std::collections::HashMap;
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use flexcore_contracts::time_model::ClockQuality;
use flexcore_ports::clock::Clock;

use crate::command_path::{CommandReceipt, CommandRejectionAuditRecord};
use crate::ports::{AuditSink, EdgeAuditEvent, EdgeAuditKind};

// --- Constants -----------------------------------------------------------------

/// The payload-bound command signature scheme prefix (the P4.28
/// `sim-sig:` pattern, adapted to the edge). Versioned by construction: a
/// scheme change is a format change, detectable and refuse-able.
pub const COMMAND_SIGNATURE_PREFIX: &str = "edge-sig";
/// The digest algorithm label inside the signature reference. `fnv1a64` is
/// the repo's existing non-cryptographic hash pattern — an integrity
/// checksum, NOT a security primitive (see the module-header honesty note).
pub const COMMAND_DIGEST_ALG: &str = "fnv1a64";
/// The default control-channel name of the per-asset sequence guard
/// (§14.3: "per asset or control channel"; the channel key is asset +
/// channel, mirroring the P4.28 `channel_key` discipline).
pub const DEFAULT_COMMAND_CHANNEL: &str = "command";
/// Hard sanity cap on one checkpoint record payload (rejects absurd lengths
/// from a corrupt header instead of attempting an absurd allocation).
const MAX_CHECKPOINT_RECORD_BYTES: u32 = 64 * 1024;

/// The per-channel sequence key (asset + control channel; the P4.28
/// `channel_key` discipline, adapted to the edge-local asset key).
pub fn sequence_channel(asset_key: &str, channel: &str) -> String {
    format!("{asset_key}/{channel}")
}

// --- Errors ----------------------------------------------------------------------

/// Named errors of the verifier's construction and inputs. Every failure is
/// named; nothing implies a silent pass. No error ever contains a key VALUE.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandVerifyError {
    /// The signing key value or reference was empty.
    InvalidSigningKey { reason: String },
    /// The incoming command shape was invalid (empty identity, non-finite
    /// value, inverted validity window).
    InvalidCommand { reason: String },
    /// The authorization reference was incomplete or self-contradictory.
    InvalidAuthorization { reason: String },
    /// The sequence checkpoint could not be loaded/stored; verification
    /// cannot run without sequence history (fail-closed).
    CheckpointUnavailable { reason: String },
}

impl std::fmt::Display for CommandVerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandVerifyError::InvalidSigningKey { reason } => {
                write!(f, "the command signing key is invalid: {reason}")
            }
            CommandVerifyError::InvalidCommand { reason } => {
                write!(f, "the incoming command is invalid: {reason}")
            }
            CommandVerifyError::InvalidAuthorization { reason } => {
                write!(f, "the command authorization reference is invalid: {reason}")
            }
            CommandVerifyError::CheckpointUnavailable { reason } => {
                write!(f, "the sequence checkpoint is unavailable: {reason}")
            }
        }
    }
}

impl std::error::Error for CommandVerifyError {}

// --- The signing key -------------------------------------------------------------

/// The central platform's command-signing key (the P6.14 [`crate::command_path::EnablementKey`]
/// pattern, reused for the command surface). THE KEY MATERIAL IS A
/// DEPLOYMENT REQUIREMENT: the deployment provisions it out-of-band (secret
/// manager / protected volume, §17.3) and the composition root loads it.
/// It never renders and never enters a log or audit line.
pub struct CommandSigningKey {
    value: String,
    reference: String,
}

impl CommandSigningKey {
    /// Wraps the key value and its reference label. The reference is the
    /// NON-secret label carried inside signature references (which key
    /// signed this command) and checked against the loaded key BEFORE the
    /// digest is compared — a command signed under a different key
    /// reference is refused as a wrong signer.
    pub fn new(value: String, reference: String) -> Result<Self, CommandVerifyError> {
        if value.trim().is_empty() {
            return Err(CommandVerifyError::InvalidSigningKey {
                reason: "the signing key value is empty".to_string(),
            });
        }
        if reference.trim().is_empty() {
            return Err(CommandVerifyError::InvalidSigningKey {
                reason: "the signing key reference is empty".to_string(),
            });
        }
        Ok(Self { value, reference })
    }

    /// The one named accessor for the key value (the digest computation
    /// only). Never passed to a log/audit formatter.
    pub fn expose(&self) -> &str {
        &self.value
    }

    /// The non-secret key reference label.
    pub fn reference(&self) -> &str {
        &self.reference
    }
}

impl std::fmt::Debug for CommandSigningKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CommandSigningKey(<redacted>)")
    }
}

impl std::fmt::Display for CommandSigningKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted>")
    }
}

/// The HMAC-STYLE keyed digest (key mixed first, explicit
/// domain-separation byte, then the body) — the same shape as the P6.14
/// `keyed_digest` and the P4.28 payload-bound digest. A keyed CHECKSUM, NOT
/// a cryptographic MAC (see the module-header honesty note).
fn keyed_digest(key: &CommandSigningKey, body: &str) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut state = FNV_OFFSET;
    for byte in key.expose().as_bytes() {
        state ^= u64::from(*byte);
        state = state.wrapping_mul(FNV_PRIME);
    }
    state ^= 0xff;
    state = state.wrapping_mul(FNV_PRIME);
    for byte in body.as_bytes() {
        state ^= u64::from(*byte);
        state = state.wrapping_mul(FNV_PRIME);
    }
    state
}

// --- The incoming command ----------------------------------------------------------

/// The edge-side shape of a gate-admitted command, extended with everything
/// the four checks need: the target adapter, the presented signature
/// reference, the authenticated transport identity (§14.3 field 15 — the
/// recorded stand-in until real transport security exists), the requested
/// quantity/value, and the validity window. The inner
/// [`crate::command_path::EdgeCommand`] carries the frozen-lifecycle state
/// name and the source sequence.
#[derive(Debug, Clone, PartialEq)]
pub struct IncomingCommand {
    /// The P6.14 command shape (identity, lifecycle state, sequence).
    pub edge_command: crate::command_path::EdgeCommand,
    /// The target adapter id (the allowlist key).
    pub target_adapter: String,
    /// The presented payload-bound signature reference (empty = unsigned —
    /// refused at verification with the `signature` reason, never at
    /// construction, so the unsigned case reaches the verifier and its
    /// audit).
    pub signature_reference: String,
    /// The authenticated transport identity of the channel the command
    /// arrived on (bound into the signature).
    pub transport_identity: String,
    /// The requested quantity label (compared against the authorization
    /// scope's allowed quantities).
    pub quantity: String,
    /// The requested value in canonical units (bound into the signature by
    /// its exact bit pattern; compared against the authorization scope's
    /// approved limits).
    pub value: f64,
    /// The command validity window (§14.3 expiry): valid
    /// `valid_from <= now < valid_until`.
    pub valid_from_unix_millis: i64,
    pub valid_until_unix_millis: i64,
}

impl IncomingCommand {
    /// Validates and builds the incoming command (fail-closed on empty
    /// identity, non-finite value, or an inverted window). The signature
    /// reference is deliberately NOT required non-empty here: an unsigned
    /// command must reach the verifier to be REFUSED and AUDITED as
    /// unsigned, not vanish at demarshalling.
    pub fn new(
        edge_command: crate::command_path::EdgeCommand,
        target_adapter: String,
        signature_reference: String,
        transport_identity: String,
        quantity: String,
        value: f64,
        valid_from_unix_millis: i64,
        valid_until_unix_millis: i64,
    ) -> Result<Self, CommandVerifyError> {
        if target_adapter.trim().is_empty() {
            return Err(CommandVerifyError::InvalidCommand {
                reason: "target_adapter is empty".to_string(),
            });
        }
        if transport_identity.trim().is_empty() {
            return Err(CommandVerifyError::InvalidCommand {
                reason: "transport_identity is empty (the authenticated transport identity is \
                         mandatory, §14.3 field 15)"
                    .to_string(),
            });
        }
        if quantity.trim().is_empty() {
            return Err(CommandVerifyError::InvalidCommand {
                reason: "quantity is empty".to_string(),
            });
        }
        if !value.is_finite() {
            return Err(CommandVerifyError::InvalidCommand {
                reason: "value is not finite".to_string(),
            });
        }
        if valid_until_unix_millis < valid_from_unix_millis {
            return Err(CommandVerifyError::InvalidCommand {
                reason: "valid_until is before valid_from".to_string(),
            });
        }
        Ok(Self {
            edge_command,
            target_adapter,
            signature_reference,
            transport_identity,
            quantity,
            value,
            valid_from_unix_millis,
            valid_until_unix_millis,
        })
    }
}

/// The canonical command binding the signature covers: every field that
/// defines the command, in stable order, `|`-separated; the value renders
/// as its 16-hex-digit bit pattern so the encoding is total and exact (the
/// P4.28 `DispatchPayloadBinding::canonical_string` discipline).
pub fn canonical_command_binding(command: &IncomingCommand) -> String {
    format!(
        "edge-cmd:{}|asset:{}|seq:{}|adapter:{}|qty:{}|val:{:016x}|via:{}",
        command.edge_command.command_id,
        command.edge_command.asset_key,
        command.edge_command.source_sequence,
        command.target_adapter,
        command.quantity,
        command.value.to_bits(),
        command.transport_identity,
    )
}

/// Computes the payload-bound signature reference for a command under
/// `key` (the central platform's signing side; the edge only verifies).
pub fn sign_command(key: &CommandSigningKey, command: &IncomingCommand) -> String {
    let digest = keyed_digest(key, &canonical_command_binding(command));
    format!(
        "{}:{}:{}:{}:{:016x}",
        COMMAND_SIGNATURE_PREFIX,
        key.reference(),
        command.edge_command.command_id,
        COMMAND_DIGEST_ALG,
        digest
    )
}

// --- The authorization reference (§17.5 rule 6) --------------------------------------

/// The approved scope for ONE asset inside the authorization reference: the
/// quantities the command may request and the approved value limits
/// (canonical units). There are no defaults: an asset without a scope is
/// refused, a quantity outside the scope is refused, a value outside the
/// limits is refused.
#[derive(Debug, Clone, PartialEq)]
pub struct AssetCommandScope {
    pub asset_key: String,
    pub allowed_quantities: Vec<String>,
    pub min_value: f64,
    pub max_value: f64,
}

/// The command authorization reference (§17.5 rule 6): which adapters may
/// receive commands, what each asset is allowed to be commanded to, and how
/// long the authorization itself is valid. The edge checks the command
/// against THIS reference at verification (check 4b) — being on an
/// allowlist alone is not authorization.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandAuthorization {
    /// The site the authorization binds to (informational at the edge; the
    /// site binding is enforced by the P6.14 gate's record check).
    pub site_id: String,
    /// The authorization's own validity — an expired authorization refuses
    /// every command (with the `expiry` reason; the detail names the
    /// authorization, not the command window).
    pub valid_until_unix_millis: i64,
    /// The site command allowlist (adapter ids) — the verification-side
    /// pre-check (check 4a). The forwarder re-checks the allowlist
    /// separately before every forward (P6.16, its own audit event).
    pub adapter_allowlist: Vec<String>,
    /// One scope per commanded asset.
    pub asset_scopes: Vec<AssetCommandScope>,
}

impl CommandAuthorization {
    /// Validates the reference (fail-closed at verifier construction).
    pub fn validate(&self) -> Result<(), CommandVerifyError> {
        let mut problems: Vec<String> = Vec::new();
        if self.site_id.trim().is_empty() {
            problems.push("site_id is empty".to_string());
        }
        if self.valid_until_unix_millis <= 0 {
            problems.push("valid_until_unix_millis is not positive".to_string());
        }
        if self
            .adapter_allowlist
            .iter()
            .all(|id| id.trim().is_empty())
        {
            problems.push("the adapter allowlist is empty".to_string());
        }
        if self.asset_scopes.is_empty() {
            problems.push("there are no asset scopes (an authorization that scopes nothing \
                           authorizes nothing)"
                .to_string());
        }
        let mut seen: Vec<&str> = Vec::new();
        for scope in &self.asset_scopes {
            if scope.asset_key.trim().is_empty() {
                problems.push("an asset scope has an empty asset_key".to_string());
                continue;
            }
            if seen.contains(&scope.asset_key.as_str()) {
                problems.push(format!("asset '{}' is scoped twice", scope.asset_key));
            }
            seen.push(&scope.asset_key);
            if scope.allowed_quantities.is_empty() {
                problems.push(format!(
                    "asset '{}' has no allowed quantities",
                    scope.asset_key
                ));
            }
            for quantity in &scope.allowed_quantities {
                if quantity.trim().is_empty() {
                    problems.push(format!(
                        "asset '{}' has an empty quantity label",
                        scope.asset_key
                    ));
                }
            }
            if !scope.min_value.is_finite() || !scope.max_value.is_finite() {
                problems.push(format!(
                    "asset '{}' has non-finite value limits",
                    scope.asset_key
                ));
            }
            if scope.min_value > scope.max_value {
                problems.push(format!(
                    "asset '{}' has min_value above max_value",
                    scope.asset_key
                ));
            }
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(CommandVerifyError::InvalidAuthorization {
                reason: problems.join("; "),
            })
        }
    }
}

// --- Rejections (the DISTINCT named reasons) -------------------------------------------

/// Why the verifier refused a command. Every variant is a DISTINCT named
/// reason with its own stable audit code — never one combined "valid"
/// boolean (the ledger's recorded risk). Every rejection is audited with
/// its reason via the `AuditSink` port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandVerificationRejection {
    /// Check 1: unsigned, malformed, wrong-signer, or tampered payload.
    Signature { detail: String },
    /// Check 2: outside the validity window, the authorization reference is
    /// expired, or the edge clock quality is degraded/unsynchronized/unknown
    /// (fail-closed, §17.5 rule 5).
    Expiry { detail: String },
    /// Check 3: the sequence is below the last accepted (out of sequence)
    /// or unsequenced (zero).
    Sequence { detail: String },
    /// Check 3: the sequence equals the last accepted — a replay of an
    /// already-accepted command.
    Duplicate { detail: String },
    /// Check 4a: the target adapter is not on the site command allowlist.
    Allowlist { adapter_id: String },
    /// Check 4b: the command is outside the approved scope/limits of its
    /// authorization reference.
    Scope { detail: String },
}

impl CommandVerificationRejection {
    /// The stable rejection code (audit text). The six codes are pairwise
    /// distinct — pinned by a test.
    pub fn code(&self) -> &'static str {
        match self {
            CommandVerificationRejection::Signature { .. } => "signature",
            CommandVerificationRejection::Expiry { .. } => "expiry",
            CommandVerificationRejection::Sequence { .. } => "sequence",
            CommandVerificationRejection::Duplicate { .. } => "duplicate",
            CommandVerificationRejection::Allowlist { .. } => "allowlist",
            CommandVerificationRejection::Scope { .. } => "scope",
        }
    }

    /// The human-safe detail behind the rejection (audit text).
    pub fn detail(&self) -> String {
        match self {
            CommandVerificationRejection::Signature { detail } => detail.clone(),
            CommandVerificationRejection::Expiry { detail } => detail.clone(),
            CommandVerificationRejection::Sequence { detail } => detail.clone(),
            CommandVerificationRejection::Duplicate { detail } => detail.clone(),
            CommandVerificationRejection::Allowlist { adapter_id } => format!(
                "adapter '{adapter_id}' is not on the site command allowlist (verification-side \
                 pre-check)"
            ),
            CommandVerificationRejection::Scope { detail } => detail.clone(),
        }
    }
}

impl std::fmt::Display for CommandVerificationRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code(), self.detail())
    }
}

// --- The sequence checkpoint (restart-survivable sequence state) -------------------------

/// Errors of the sequence checkpoint store. Every failure is named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckpointError {
    /// The checkpoint path was empty or not usable.
    InvalidPath { reason: String },
    /// The underlying filesystem failed.
    Io { reason: String },
    /// The file exists but is not a FlexCore edge command sequence
    /// checkpoint (wrong magic or version) — refused, never guessed at.
    NotACheckpointFile { reason: String },
}

impl std::fmt::Display for CheckpointError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckpointError::InvalidPath { reason } => {
                write!(f, "the checkpoint path is not usable: {reason}")
            }
            CheckpointError::Io { reason } => write!(f, "checkpoint I/O failed: {reason}"),
            CheckpointError::NotACheckpointFile { reason } => write!(
                f,
                "the file is not a FlexCore edge command sequence checkpoint (refused, never \
                 guessed): {reason}"
            ),
        }
    }
}

impl std::error::Error for CheckpointError {}

/// The persisted last-accepted-sequence store (the restart-survivability
/// half of verification check 3). Implemented by
/// [`FileSequenceCheckpoint`] (durable) — a test may supply any other
/// implementation. The verifier loads the whole state at construction
/// (fail-closed: no history, no verification) and stores each advance
/// DURABLY BEFORE the in-memory state moves.
pub trait SequenceCheckpointStore: Send {
    /// The persisted last-accepted sequence per channel, as
    /// `(channel, sequence)` pairs (a channel never admitted has no entry).
    fn load_all(&self) -> Result<Vec<(String, u64)>, CheckpointError>;
    /// Persists one channel's advanced last-accepted sequence. On `Err` the
    /// caller must refuse the command fail-closed (an unpersisted advance
    /// could be replayed after a restart).
    fn store_last_accepted(&self, channel: &str, sequence: u64) -> Result<(), CheckpointError>;
}

/// The file-backed [`SequenceCheckpointStore`], in the same framing
/// discipline as `buffer_file.rs`:
///
/// ```text
/// header:  b"FLEXCORE-EDGE-COMMAND-SEQ-CKPT" + format version byte (0x01)
/// record:  [u32 LE payload_len][u32 LE crc32(payload)][payload]
/// payload: [u16 LE channel_len][channel bytes][u64 LE sequence]
/// ```
///
/// **Recovery (truncate-and-audit, never silent).** On open the file is
/// replayed record by record; the FIRST malformed frame ends the replay and
/// the file is TRUNCATED at the last good record, with the loss counted
/// ([`FileSequenceCheckpoint::corrupt_tail_truncated`]) and logged loudly.
/// A wrong magic/version is a hard open error. The last record per channel
/// wins (the store is append-only; replay keeps the newest entry).
pub struct FileSequenceCheckpoint {
    path: PathBuf,
    file: Mutex<std::fs::File>,
    state: Mutex<HashMap<String, u64>>,
    corrupt_tail_truncated: std::sync::atomic::AtomicU64,
    records_written: std::sync::atomic::AtomicU64,
}

impl FileSequenceCheckpoint {
    /// The journal magic header (30-byte label) + format version.
    fn magic() -> Vec<u8> {
        let mut magic = b"FLEXCORE-EDGE-COMMAND-SEQ-CKPT".to_vec();
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
                // Replay the existing file.
                use std::io::Read;
                let mut contents = Vec::new();
                existing
                    .read_to_end(&mut contents)
                    .map_err(|e| CheckpointError::Io {
                        reason: format!("reading the existing checkpoint failed: {e}"),
                    })?;
                if contents.is_empty() {
                    // A zero-length file (touch/new) is initialized, not refused.
                    (existing, HashMap::new())
                } else {
                    if contents.len() < magic.len() || contents[..magic.len()] != magic[..] {
                        return Err(CheckpointError::NotACheckpointFile {
                            reason: format!(
                                "the file at {} does not carry the FlexCore edge command \
                                 sequence checkpoint magic",
                                path.display()
                            ),
                        });
                    }
                    let mut state = HashMap::new();
                    let mut offset = magic.len();
                    let mut corrupt_tail_truncated: u64 = 0;
                    while offset + 8 <= contents.len() {
                        let len = u32::from_le_bytes(
                            contents[offset..offset + 4].try_into().expect("4 bytes"),
                        );
                        if len > MAX_CHECKPOINT_RECORD_BYTES
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
                        // COUNTED and logged, never silent.
                        tracing::warn!(
                            truncated_records = corrupt_tail_truncated,
                            path = %path.display(),
                            "command sequence checkpoint: corrupt tail truncated; the affected \
                             channels' window reverted to their last good record"
                        );
                        existing
                            .set_len(offset as u64)
                            .map_err(|e| CheckpointError::Io {
                                reason: format!("truncating the corrupt checkpoint tail failed: {e}"),
                            })?;
                    }
                    (existing, state)
                }
            }
            Err(_) => {
                // Create a new checkpoint file with the magic header.
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
                    .map_err(|e| CheckpointError::Io {
                        reason: format!("writing the checkpoint header failed: {e}"),
                    })?;
                (created, HashMap::new())
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
    let channel_len = u16::from_le_bytes(payload[0..2].try_into().expect("2 bytes")) as usize;
    if payload.len() < 2 + channel_len + 8 {
        return None;
    }
    let channel = std::str::from_utf8(&payload[2..2 + channel_len]).ok()?.to_string();
    let sequence = u64::from_le_bytes(
        payload[2 + channel_len..2 + channel_len + 8]
            .try_into()
            .expect("8 bytes"),
    );
    Some((channel, sequence))
}

/// The CRC-32 (IEEE, reflected) used by the checkpoint records — the same
/// checksum family as the telemetry journal (an integrity checksum for
/// corruption detection, not a security primitive).
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

impl SequenceCheckpointStore for FileSequenceCheckpoint {
    fn load_all(&self) -> Result<Vec<(String, u64)>, CheckpointError> {
        let state = self.state.lock().expect("poisoned checkpoint state mutex");
        Ok(state
            .iter()
            .map(|(channel, sequence)| (channel.clone(), *sequence))
            .collect())
    }

    fn store_last_accepted(&self, channel: &str, sequence: u64) -> Result<(), CheckpointError> {
        // Durability BEFORE state: the record is flushed to disk before the
        // in-memory map moves (a crash cannot lose a stored advance).
        let payload = Self::encode_payload(channel, sequence);
        let record_len = payload.len() as u32;
        let crc = crc32(&payload);
        let mut record = Vec::with_capacity(8 + payload.len());
        record.extend_from_slice(&record_len.to_le_bytes());
        record.extend_from_slice(&crc.to_le_bytes());
        record.extend_from_slice(&payload);
        {
            let mut file = self.file.lock().expect("poisoned checkpoint file mutex");
            file.seek(SeekFrom::End(0))
                .and_then(|_| file.write_all(&record))
                .and_then(|_| file.flush())
                .and_then(|_| file.sync_data())
                .map_err(|e| CheckpointError::Io {
                    reason: format!("appending the checkpoint record failed: {e}"),
                })?;
        }
        self.state
            .lock()
            .expect("poisoned checkpoint state mutex")
            .insert(channel.to_string(), sequence);
        self.records_written
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

// --- The verified command ----------------------------------------------------------------

/// A command that passed ALL FOUR checks. This is NOT a delivery and not an
/// acknowledgement: it is the input to the P6.16 forwarder, which runs its
/// own separate allowlist + policy re-check before anything moves (§14.1:
/// acknowledgement is not physical delivery).
#[derive(Debug, Clone, PartialEq)]
pub struct VerifiedCommand {
    pub command_id: String,
    pub asset_key: String,
    pub target_adapter: String,
    pub quantity: String,
    pub value: f64,
    pub sequence: u64,
    pub verified_at_unix_millis: i64,
    /// The command's own validity bound (§14.3), carried so the P6.16
    /// engine can expire a command mid-flight honestly (no premature
    /// expiry — the engine refuses to expire before this instant).
    pub valid_until_unix_millis: i64,
    /// The recorded scope note (what verification does NOT mean).
    pub scope_note: String,
}

// --- The verifier ---------------------------------------------------------------------------

/// The four-check verifier. Composed ONCE per edge with the deployment's
/// signing key, the site's authorization reference, and the persisted
/// sequence checkpoint; [`CommandVerifier::verify`] is called on every
/// gate-admitted command BEFORE any forwarding. Construction is fail-closed:
/// an invalid key, an invalid authorization reference, or an unloadable
/// checkpoint means NO verification (and therefore no commands) can run.
pub struct CommandVerifier {
    key: CommandSigningKey,
    authorization: CommandAuthorization,
    checkpoint: Box<dyn SequenceCheckpointStore>,
    audit: Arc<dyn AuditSink>,
    clock: Arc<dyn Clock + Send + Sync>,
    /// Per-channel last-accepted sequence (memory mirror of the checkpoint;
    /// advanced only after a durable store succeeded).
    last_accepted: HashMap<String, u64>,
}

impl CommandVerifier {
    /// Composes the verifier. Loads the sequence history from the checkpoint
    /// (fail-closed: a load failure is a construction error). Audits the
    /// composition (`EDGE_COMMAND_VERIFIER_COMPOSED`) with identifiers only —
    /// never key material.
    pub fn new(
        key: CommandSigningKey,
        authorization: CommandAuthorization,
        checkpoint: Box<dyn SequenceCheckpointStore>,
        audit: Arc<dyn AuditSink>,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Result<Self, CommandVerifyError> {
        if key.expose().trim().is_empty() || key.reference().trim().is_empty() {
            return Err(CommandVerifyError::InvalidSigningKey {
                reason: "the signing key value or reference is empty".to_string(),
            });
        }
        authorization.validate()?;
        let history = checkpoint.load_all().map_err(|error| {
            CommandVerifyError::CheckpointUnavailable {
                reason: error.to_string(),
            }
        })?;
        let last_accepted: HashMap<String, u64> = history.into_iter().collect();
        let verifier = Self {
            key,
            authorization,
            checkpoint,
            audit,
            clock,
            last_accepted,
        };
        verifier.audit_event(
            "EDGE_COMMAND_VERIFIER_COMPOSED",
            format!(
                "command verifier composed (key_reference='{}', site='{}', scoped assets=[{}], \
                 checkpoint channels={}, test-env only)",
                verifier.key.reference(),
                verifier.authorization.site_id,
                verifier
                    .authorization
                    .asset_scopes
                    .iter()
                    .map(|s| s.asset_key.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
                verifier.last_accepted.len(),
            ),
        );
        Ok(verifier)
    }

    /// The signing key reference (non-secret; diagnostics).
    pub fn key_reference(&self) -> &str {
        self.key.reference()
    }

    /// The authorization reference (read-only access for composition/tests).
    pub fn authorization(&self) -> &CommandAuthorization {
        &self.authorization
    }

    /// Runs the four checks on ONE gate-admitted command, in the fixed
    /// order documented in the module header. On success the sequence
    /// advance is persisted durably BEFORE the in-memory state moves, and a
    /// [`VerifiedCommand`] is returned. On any failure the command is
    /// dropped (nothing retains it), the rejection is audited with its
    /// DISTINCT reason, and the sequence window is UNCHANGED.
    ///
    /// `clock_quality` is the recorded edge clock-quality status supplied by
    /// the composition (P6.31 owns producing it): any quality other than
    /// `Synchronized` fails the expiry check CLOSED.
    pub fn verify(
        &mut self,
        receipt: &CommandReceipt,
        command: &IncomingCommand,
        clock_quality: ClockQuality,
    ) -> Result<VerifiedCommand, CommandVerificationRejection> {
        let now = self.now_unix_millis();

        // 0. The receipt must be a live gate admission (the P6.14 enablement
        //    is time-bounded; an expired enablement refuses fail-closed even
        //    if the receiver raced the clock).
        if now >= receipt.enablement_valid_until_unix_millis {
            let rejection = CommandVerificationRejection::Expiry {
                detail: format!(
                    "the test-env enablement admission expired at {} (the verification cannot \
                     run outside the enablement window)",
                    receipt.enablement_valid_until_unix_millis
                ),
            };
            self.audit_rejection(command, &rejection, now);
            return Err(rejection);
        }

        // 1. SIGNATURE — unsigned, malformed, wrong signer, or tampered.
        if let Err(detail) = self.check_signature(command) {
            let rejection = CommandVerificationRejection::Signature { detail };
            self.audit_rejection(command, &rejection, now);
            return Err(rejection);
        }

        // 2. EXPIRY — clock quality first (fail-closed), then the windows.
        if clock_quality != ClockQuality::Synchronized {
            let rejection = CommandVerificationRejection::Expiry {
                detail: format!(
                    "edge clock quality is {:?}; expiry checks are not trustworthy on a \
                     degraded/unsynchronized/unknown clock and FAIL CLOSED (§17.4, §17.5 rule 5; \
                     P6.31 owns the clock-quality status)",
                    clock_quality
                ),
            };
            self.audit_rejection(command, &rejection, now);
            return Err(rejection);
        }
        if now < command.valid_from_unix_millis {
            let rejection = CommandVerificationRejection::Expiry {
                detail: format!(
                    "the command is not yet valid (valid_from={}, now={})",
                    command.valid_from_unix_millis, now
                ),
            };
            self.audit_rejection(command, &rejection, now);
            return Err(rejection);
        }
        if now >= command.valid_until_unix_millis {
            let rejection = CommandVerificationRejection::Expiry {
                detail: format!(
                    "the command expired at {} (now={}; §14.3, §17.5 rule 5)",
                    command.valid_until_unix_millis, now
                ),
            };
            self.audit_rejection(command, &rejection, now);
            return Err(rejection);
        }
        if now >= self.authorization.valid_until_unix_millis {
            let rejection = CommandVerificationRejection::Expiry {
                detail: format!(
                    "the authorization reference expired at {} (now={})",
                    self.authorization.valid_until_unix_millis, now
                ),
            };
            self.audit_rejection(command, &rejection, now);
            return Err(rejection);
        }

        // 4a. ALLOWLIST — the verification-side pre-check (the P6.16
        //     forwarder re-checks separately, with its own audit event).
        if !self
            .authorization
            .adapter_allowlist
            .iter()
            .any(|id| id == &command.target_adapter)
        {
            let rejection = CommandVerificationRejection::Allowlist {
                adapter_id: command.target_adapter.clone(),
            };
            self.audit_rejection(command, &rejection, now);
            return Err(rejection);
        }

        // 4b. SCOPE — the command must sit inside the approved scope/limits
        //     of the authorization reference (§17.5 rule 6).
        if let Err(detail) = self.check_scope(command) {
            let rejection = CommandVerificationRejection::Scope { detail };
            self.audit_rejection(command, &rejection, now);
            return Err(rejection);
        }

        // 3. SEQUENCE — evaluated LAST so a command refused by any earlier
        //    check never advances the window (the P4.28 SequenceGuard rule).
        //    The verdict is computed first; the commit (durable store, then
        //    memory) happens only here, after every other check passed.
        let channel = sequence_channel(
            &command.edge_command.asset_key,
            DEFAULT_COMMAND_CHANNEL,
        );
        let sequence = command.edge_command.source_sequence;
        if sequence == 0 {
            let rejection = CommandVerificationRejection::Sequence {
                detail: format!(
                    "command '{}' carries sequence 0 (unsequenced) on channel '{channel}'",
                    command.edge_command.command_id
                ),
            };
            self.audit_rejection(command, &rejection, now);
            return Err(rejection);
        }
        match self.last_accepted.get(&channel) {
            Some(&last) if sequence == last => {
                let rejection = CommandVerificationRejection::Duplicate {
                    detail: format!(
                        "command '{}' sequence {sequence} equals the last accepted sequence on \
                         channel '{channel}' — a replay of an already-accepted command",
                        command.edge_command.command_id
                    ),
                };
                self.audit_rejection(command, &rejection, now);
                return Err(rejection);
            }
            Some(&last) if sequence < last => {
                let rejection = CommandVerificationRejection::Sequence {
                    detail: format!(
                        "command '{}' sequence {sequence} is below the last accepted sequence \
                         {last} on channel '{channel}' — out of sequence",
                        command.edge_command.command_id
                    ),
                };
                self.audit_rejection(command, &rejection, now);
                return Err(rejection);
            }
            _ => {}
        }
        if let Err(error) = self.checkpoint.store_last_accepted(&channel, sequence) {
            let rejection = CommandVerificationRejection::Sequence {
                detail: format!(
                    "the sequence checkpoint persistence failed for channel '{channel}'; the \
                     command is refused fail-closed (an unpersisted advance could be replayed \
                     after a restart): {error}"
                ),
            };
            self.audit_rejection(command, &rejection, now);
            return Err(rejection);
        }
        self.last_accepted.insert(channel.clone(), sequence);

        // All four checks passed (test env only).
        let verified = VerifiedCommand {
            command_id: command.edge_command.command_id.clone(),
            asset_key: command.edge_command.asset_key.clone(),
            target_adapter: command.target_adapter.clone(),
            quantity: command.quantity.clone(),
            value: command.value,
            sequence,
            verified_at_unix_millis: now,
            valid_until_unix_millis: command.valid_until_unix_millis,
            scope_note: "edge verification passed (signature, expiry with clock-quality, sequence \
                         with persisted checkpoint, allowlist + scope) in the TEST ENVIRONMENT \
                         ONLY; verification is not delivery and never a physical write — \
                         acknowledgement is not physical delivery (§14.1)"
                .to_string(),
        };
        self.audit_event(
            "EDGE_COMMAND_VERIFIED",
            format!(
                "command={} asset={} adapter={} sequence={} verified in the test env (all four \
                 checks; approval='{}'); forwarding is the separately-gated P6.16 surface",
                verified.command_id,
                verified.asset_key,
                verified.target_adapter,
                verified.sequence,
                receipt.approver,
            ),
        );
        Ok(verified)
    }

    /// Check 1: the presented reference must parse as
    /// `edge-sig:<key_reference>:<command_id>:fnv1a64:<digest>`, name THIS
    /// key and THIS command, and carry the digest of the canonical binding.
    fn check_signature(&self, command: &IncomingCommand) -> Result<(), String> {
        let presented = command.signature_reference.trim();
        let command_id = &command.edge_command.command_id;
        if presented.is_empty() {
            return Err(format!(
                "command '{command_id}' carries no signature — unsigned commands are refused"
            ));
        }
        let parts: Vec<&str> = presented.split(':').collect();
        if parts.len() != 5
            || parts[0] != COMMAND_SIGNATURE_PREFIX
            || parts[3] != COMMAND_DIGEST_ALG
        {
            return Err(format!(
                "malformed signature reference (expected \
                 {COMMAND_SIGNATURE_PREFIX}:<key_reference>:<command_id>:{COMMAND_DIGEST_ALG}:\
                 <digest>)"
            ));
        }
        if parts[1] != self.key.reference() {
            return Err(format!(
                "wrong signer: signature key reference '{}' is not this edge's verifying key \
                 reference '{}'",
                parts[1],
                self.key.reference()
            ));
        }
        if parts[2] != command_id.as_str() {
            return Err(format!(
                "signature is bound to command '{}' but arrived for command '{command_id}' — \
                 payload binding mismatch",
                parts[2]
            ));
        }
        let expected = format!(
            "{:016x}",
            keyed_digest(&self.key, &canonical_command_binding(command))
        );
        if parts[4] != expected {
            return Err(format!(
                "payload integrity check failed (presented digest {}, expected {expected}) — the \
                 payload does not match what was signed",
                parts[4]
            ));
        }
        Ok(())
    }

    /// Check 4b: the asset must have a scope; the quantity must be allowed;
    /// the value must sit inside the approved limits.
    fn check_scope(&self, command: &IncomingCommand) -> Result<(), String> {
        let asset_key = &command.edge_command.asset_key;
        let Some(scope) = self
            .authorization
            .asset_scopes
            .iter()
            .find(|scope| &scope.asset_key == asset_key)
        else {
            return Err(format!(
                "asset '{asset_key}' has no scope in the authorization reference (unknown assets \
                 are refused, never defaulted)"
            ));
        };
        if !scope.allowed_quantities.contains(&command.quantity) {
            return Err(format!(
                "quantity '{}' is not within the approved scope for asset '{asset_key}' (allowed: \
                 {})",
                command.quantity,
                scope.allowed_quantities.join(",")
            ));
        }
        if command.value < scope.min_value || command.value > scope.max_value {
            return Err(format!(
                "value {} is outside the approved limits [{}, {}] for asset '{asset_key}' \
                 quantity '{}'",
                command.value, scope.min_value, scope.max_value, command.quantity
            ));
        }
        Ok(())
    }

    fn now_unix_millis(&self) -> i64 {
        let stamp = self.clock.now_ingest_time();
        i64::try_from(stamp.unix_millis()).unwrap_or(i64::MAX)
    }

    /// Records one verifier audit event through the edge `AuditSink` port
    /// (same audit-kind limitation as the P6.14 module: `EdgeAuditKind` is a
    /// closed vocabulary this wave does not own, so the dedicated event name
    /// LEADS the detail string on kind `EDGE_STARTUP`).
    fn audit_event(&self, name: &'static str, detail: String) {
        let event = EdgeAuditEvent {
            recorded_at: self.clock.now_ingest_time(),
            kind: EdgeAuditKind::Startup,
            detail: format!("{name}: {detail}"),
        };
        if let Err(error) = self.audit.record(event) {
            tracing::error!(%error, audit_event = name, "command-verifier audit write FAILED");
        }
    }

    /// Builds and records the P4-pattern rejection record (cited, reused —
    /// the same `CommandRejectionAuditRecord` shape the P6.14 receiver
    /// emits, with the verifier's distinct reason code).
    fn audit_rejection(
        &self,
        command: &IncomingCommand,
        rejection: &CommandVerificationRejection,
        now_millis: i64,
    ) {
        let record = CommandRejectionAuditRecord {
            command_id: command.edge_command.command_id.clone(),
            asset_key: command.edge_command.asset_key.clone(),
            target_adapter: command.target_adapter.clone(),
            refusal_code: rejection.code().to_string(),
            at_unix_millis: now_millis,
            refusal_reference: format!(
                "command-denial:{}/{}@{}/{}",
                command.edge_command.command_id,
                command.target_adapter,
                now_millis,
                rejection.code()
            ),
        };
        self.audit_event(
            "EDGE_COMMAND_VERIFICATION_REJECTED",
            format!("{} ({})", record.audit_text(), rejection.detail()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_path::EdgeCommand;
    use crate::ports::InMemoryAuditSink;
    use flexcore_ports::fakes::FixedClock;

    const BASE_MILLIS: i64 = 1_700_000_000_000;

    fn key() -> CommandSigningKey {
        CommandSigningKey::new(
            "lab-command-signing-key-material".to_string(),
            "deployment:lab-command-key-1".to_string(),
        )
        .expect("valid key")
    }

    fn authorization() -> CommandAuthorization {
        CommandAuthorization {
            site_id: "pilot-site-A".to_string(),
            valid_until_unix_millis: BASE_MILLIS + 86_400_000,
            adapter_allowlist: vec!["simulated-test".to_string()],
            asset_scopes: vec![AssetCommandScope {
                asset_key: "ess-01".to_string(),
                allowed_quantities: vec!["active_power".to_string()],
                min_value: -5.0,
                max_value: 5.0,
            }],
        }
    }

    fn incoming(signature: String, sequence: u64) -> IncomingCommand {
        let edge = EdgeCommand::new(
            "cmd-1".to_string(),
            "ess-01".to_string(),
            "ISSUED".to_string(),
            sequence,
        )
        .expect("valid command");
        IncomingCommand::new(
            edge,
            "simulated-test".to_string(),
            signature,
            "edge-session-1".to_string(),
            "active_power".to_string(),
            1.5,
            BASE_MILLIS,
            BASE_MILLIS + 3_600_000,
        )
        .expect("valid incoming")
    }

    fn receipt() -> CommandReceipt {
        CommandReceipt {
            command_id: "cmd-1".to_string(),
            target_adapter: "simulated-test".to_string(),
            admitted_at_unix_millis: BASE_MILLIS,
            approver: "J. Doe (security lead)".to_string(),
            enablement_valid_until_unix_millis: BASE_MILLIS + 3_600_000,
            scope_note: "gate admission only (test)".to_string(),
        }
    }

    fn verifier(scratch: &Path) -> CommandVerifier {
        let checkpoint =
            FileSequenceCheckpoint::open(&scratch.join("seq.ckpt")).expect("checkpoint opens");
        CommandVerifier::new(
            key(),
            authorization(),
            Box::new(checkpoint),
            Arc::new(InMemoryAuditSink::new()),
            Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000)),
        )
        .expect("verifier composes")
    }

    fn scratch_dir(label: &str) -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "flexcore-edge-p615-unit-{}-{}-{}",
            std::process::id(),
            label,
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).expect("scratch dir creates");
        dir
    }

    #[test]
    fn the_six_rejection_codes_are_pairwise_distinct() {
        let rejections = [
            CommandVerificationRejection::Signature {
                detail: "d".to_string(),
            },
            CommandVerificationRejection::Expiry {
                detail: "d".to_string(),
            },
            CommandVerificationRejection::Sequence {
                detail: "d".to_string(),
            },
            CommandVerificationRejection::Duplicate {
                detail: "d".to_string(),
            },
            CommandVerificationRejection::Allowlist {
                adapter_id: "a".to_string(),
            },
            CommandVerificationRejection::Scope {
                detail: "d".to_string(),
            },
        ];
        let mut codes: Vec<&str> = rejections.iter().map(|r| r.code()).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), 6, "every rejection reason is DISTINCT: {codes:?}");
    }

    #[test]
    fn the_signature_round_trips_and_refuses_tampering() {
        let key = key();
        let command = incoming(String::new(), 1);
        let signed = sign_command(&key, &command);
        let command = incoming(signed, 1);
        // A verifier with the same key accepts the signature (checked in the
        // full pipeline tests; here only the digest mechanics).
        let binding = canonical_command_binding(&command);
        assert!(binding.starts_with("edge-cmd:cmd-1|asset:ess-01|seq:1|"));
        // The key redacts.
        assert_eq!(format!("{:?}", key), "CommandSigningKey(<redacted>)");
        assert_eq!(key.to_string(), "<redacted>");
        // Tampering with any bound field changes the digest.
        let mut tampered = command.clone();
        tampered.value = 2.5;
        assert_ne!(
            canonical_command_binding(&tampered),
            canonical_command_binding(&command)
        );
    }

    #[test]
    fn the_checkpoint_file_round_trips_and_survives_reopen() {
        let dir = scratch_dir("round-trip");
        let path = dir.join("seq.ckpt");
        {
            let checkpoint = FileSequenceCheckpoint::open(&path).expect("opens");
            assert_eq!(checkpoint.corrupt_tail_truncated(), 0);
            checkpoint
                .store_last_accepted("ess-01/command", 7)
                .expect("stores");
            checkpoint
                .store_last_accepted("ess-02/command", 3)
                .expect("stores");
        }
        let reopened = FileSequenceCheckpoint::open(&path).expect("reopens");
        let mut state = reopened.load_all().expect("loads");
        state.sort();
        assert_eq!(
            state,
            vec![
                ("ess-01/command".to_string(), 7u64),
                ("ess-02/command".to_string(), 3u64)
            ]
        );
    }

    #[test]
    fn a_corrupt_checkpoint_tail_is_truncated_and_counted() {
        let dir = scratch_dir("corrupt-tail");
        let path = dir.join("seq.ckpt");
        {
            let checkpoint = FileSequenceCheckpoint::open(&path).expect("opens");
            checkpoint
                .store_last_accepted("ess-01/command", 9)
                .expect("stores");
        }
        // Append a torn/bad frame (a torn write): a record header claiming
        // an absurd length — the reopen must truncate and count.
        use std::io::Write as _;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("appends");
        let mut garbage = 0xFFFF_FFF0u32.to_le_bytes().to_vec();
        garbage.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        file.write_all(&garbage).expect("garbage");
        drop(file);
        let reopened = FileSequenceCheckpoint::open(&path).expect("reopens");
        assert_eq!(reopened.corrupt_tail_truncated(), 1, "the loss is counted");
        let state = reopened.load_all().expect("loads");
        assert_eq!(
            state,
            vec![("ess-01/command".to_string(), 9u64)],
            "the last good record survives"
        );
    }

    #[test]
    fn a_wrong_magic_file_is_refused_not_guessed() {
        let dir = scratch_dir("wrong-magic");
        let path = dir.join("not-a-checkpoint");
        std::fs::write(&path, b"something else entirely").expect("writes");
        assert!(matches!(
            FileSequenceCheckpoint::open(&path),
            Err(CheckpointError::NotACheckpointFile { .. })
        ));
    }
}
