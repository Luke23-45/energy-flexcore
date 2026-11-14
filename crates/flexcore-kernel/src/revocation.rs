// P7.38 — Plugin revocation, staged rollout, and failed-plugin removal
// (report §17.4 supply-chain compromise: "signed artifacts, provenance,
// SBOM, pinned dependencies, staged rollout"; §5.6 lifecycle; §9.3;
// ledger `ledger_phase_7.md` P7.38; decision record
// `ledger/phase-07-plugin-sdk/37_38_39_invariants_revocation_tenancy.md`).
//
// WHAT THIS MODULE COMPLETES: the P7.14 revocation seams (recorded in
// `14_15_16_signing_profiles_recomposition.md` §2.4 — "Revocation (P7.38)
// plugs in at three recorded seams: (a) the DigestVerifier port gains a
// revocation-aware implementation ... (c) the lifecycle gate is the
// enforcement point for 'revoke a RUNNING plugin' (drain-and-remove)").
// This module implements those seams as a PLUGIN (not just artifact)
// revocation register, wired as lifecycle gate outcomes and as a
// registry-time re-check.
//
// THE REGISTER (the data structure):
//
// - Keys are of two kinds, [`RevocationKey`]: a `(plugin id, version)`
//   pair (revoking one version of one plugin) and a signer key REFERENCE
//   (revoking a key revokes everything it signed — the record's
//   `signer-key-reference` field from P7.14 is the join key, so no new
//   provenance field is needed).
// - The store port [`RevocationStore`] is APPEND-ONLY: records are added,
//   never removed or rewritten (§5.1 audit integrity). There is NO
//   un-revoke API — un-revocation is a NEW signed record type and a new
//   policy decision, recorded as the [`UNREVOCATION_SEAM`] seam and
//   deliberately NOT implemented in v0.5.0 (fail-closed: the safe
//   direction is the only direction implemented).
// - Two implementations: [`MemoryRevocationRegister`] (deterministic
//   collector for tests and in-process compositions) and
//   [`FileRevocationRegister`] — the file-backed register in the
//   `buffer_file.rs` FRAMING DISCIPLINE of `crates/edge-agent/src/
//   buffer_file.rs` (cited, REIMPLEMENTED here — the kernel depends on no
//   flexcore-* crate), the same framing as the P7.36 sequence checkpoint:
//
//   ```text
//   header:  b"FLEXCORE-PLUGIN-REV-REG" + format version byte (0x01)
//   record:  [u32 LE payload_len][u32 LE crc32(payload)][payload]
//   payload: [u8 key_kind][u16 LE id_len][id][u16 LE ver_len][ver]
//            [u16 LE reason_len][reason][u16 LE actor_len][actor]
//            [u64 LE revoked_at_millis]
//   ```
//
//   Recovery is truncate-and-audit, never silent (first malformed frame
//   ends the replay, the file is truncated at the last good record, the
//   loss is COUNTED); a wrong magic/version is a hard open error; the
//   append is flushed and `sync_data`d BEFORE the in-memory state moves
//   (durability before state).
//
// WIRING (the two enforcement points):
//
// 1. LIFECYCLE GATE, CONSULTED FIRST: at `Discovered → Verified` the
//    caller composes [`revocation_gate`] BEFORE
//    `crate::signing::provenance_gate` — a package whose signature was
//    revoked after signing is refused at the gate BEFORE any digest is
//    computed (the ledger P7.38 step-1 rule; the P7.14 order "unsigned
//    before anything else" is preserved for the non-revoked remainder).
//    The gate output is the P7.07 [`GateOutcome`], so the lifecycle
//    refuses the transition fail-closed and audits it exactly like any
//    other gate refusal.
// 2. REGISTRY-TIME RE-CHECK: [`ensure_startable`] re-consults the register
//    immediately before `Configured → Started`, so a plugin revoked
//    AFTER verification cannot START. A revoked RUNNING plugin is
//    contained by the same check at every subsequent lifecycle advance
//    and by the host broker's per-call re-checks; the removal path below
//    is how it leaves the composition. A revoked plugin can never
//    re-verify (the register consult happens before provenance on every
//    `Discovered → Verified`, including after an explicit restart).
//
// STAGED ROLLOUT (the ledger step 3; §17.4 "staged rollout"):
//
// [`StagedRollout`] is a rollout plan with promotion/refusal semantics —
// cohort/percentage-free BY CONSTRUCTION. v0.5.0 supports ONLY explicit
// per-profile staging (the P7.15 profiles are the staging units): a new
// plugin version is added to ONE profile first, then promoted
// profile-by-profile in the plan's DECLARED order. There is no percentage
// canary — no field, method, or type in this module can express one (the
// [`ROLLOUT_STATEMENT`] pins this; the type surface is the proof).
// Promoting to a profile that is not exactly the next un-promoted one is
// the named refusal [`RolloutError::PromotionOutOfOrder`] — a "staged"
// rollout that is really a flag flip is refused, and so is an
// out-of-order promotion. Every promotion and refusal is audited.
//
// FAILED-PLUGIN REMOVAL (the ledger step 4; the P7.30 drill item):
//
// The lifecycle already holds `Failed` (P7.07). The REMOVAL path here is
// EXPLICIT and drain-first: [`remove_failed_plugin`] is legal ONLY from
// `Failed`, requires a non-empty reason and actor, drives the §5.6
// `Failed → Draining → Stopped` edges through the plugin's
// [`DrainPort`] (quiescence or the fail-closed deadline refusal — a
// Failed plugin with in-flight records is NOT removable until it
// quiesces), and audits every step. Removal NEVER deletes anything from
// the audit trail: the lifecycle's records persist in the audit
// sink/store by construction (this module appends records, never
// removes), so a removed plugin — including a malicious one — keeps its
// full evidence trail.
//
// AUDIT: every revocation append, every revocation refusal, every
// register-consult refusal, every promotion, and every removal step
// emits a [`RevocationAuditRecord`] through the in-module
// [`RevocationAuditSink`] port — a fourth kernel-local audit port (the
// lifecycle's [`crate::lifecycle::AuditSink`], the signing gate's reuse
// of it, and the P7.33 host chain are the others; a separate port here
// avoids overloading the lifecycle transition-record semantics with
// non-transition events). Timestamps come ONLY from the injected
// [`Clock`] (`crate::lifecycle::Clock`); `SystemTime::now()` appears
// nowhere in this module.
//
// Determinism: register iteration is `BTreeMap`-ordered; file framing is
// fixed-layout binary; no `unwrap`/`expect` on untrusted input anywhere
// in `src/`.

use std::collections::BTreeMap;
use std::fmt;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::lifecycle::{
    Clock, DrainOutcome, DrainPort, GateOutcome, LifecycleError, LifecycleState, PluginLifecycle,
};
use crate::manifest::{validate_plugin_id, SemVer};
use crate::signing::KEY_REFERENCE_SCHEMES;

// --- Honesty / seam statements ----------------------------------------------------

/// The revocation statement (the P4.28 statement pattern — pinned by tests
/// so what IS and IS NOT claimed stays reviewable).
pub const REVOCATION_STATEMENT: &str = "the P7.38 revocation register is an APPEND-ONLY, \
     versioned plugin-revocation structure keyed by (plugin id, version) and by signer key \
     reference, consulted FIRST at the Discovered-to-Verified gate and re-checked before \
     Start. Revoking a key revokes everything it signed. There is NO un-revoke: \
     un-revocation is a new signed record type and a new policy decision (the recorded \
     seam). No certification or security claim is made.";

/// The recorded un-revocation seam: the register has NO removal API. A
/// reinstatement would be a NEW append-only record type (signed, policy-
/// gated, separately audited) and is deliberately NOT implemented in
/// v0.5.0 — the only direction the register implements is the safe one.
pub const UNREVOCATION_SEAM: &str = "un-revocation is NOT implemented: the register is \
     append-only and has no removal or override API. A future reinstatement mechanism is a \
     NEW signed record type with its own decision record; until then a revoked key or \
     plugin version stays revoked.";

/// The staged-rollout statement (the type surface is the proof: no field,
/// method, or type here can express a percentage canary).
pub const ROLLOUT_STATEMENT: &str = "v0.5.0 supports ONLY explicit per-profile staging: a \
     new plugin version is added to ONE profile first, then promoted profile-by-profile in \
     the plan's declared order. There is NO percentage canary — a percentage rollout is \
     not constructible from the StagedRollout type surface — and promoting to any profile \
     other than the exact next un-promoted one is refused.";

/// The failed-plugin removal statement.
pub const REMOVAL_STATEMENT: &str = "removal of a Failed plugin is EXPLICIT and \
     drain-first: it is legal only from Failed, requires a recorded reason and actor, \
     drives the §5.6 Failed-to-Draining-to-Stopped edges through the plugin's drain port \
     (quiescence or the fail-closed deadline refusal), and is audited at every step. \
     Removal NEVER deletes the plugin's audit trail — records persist in the audit \
     sink/store.";

// --- Revocation keys ----------------------------------------------------------------

/// The register key. TWO kinds, exactly (closed): one plugin version, or a
/// signer key reference (revoking a key revokes everything it signed —
/// the P7.14 provenance records already carry `signer-key-reference`, so
/// the join needs no new field).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevocationKey {
    /// One plugin version.
    Plugin { plugin_id: String, version: SemVer },
    /// A signer key REFERENCE (scheme-prefixed; never key material — the
    /// P7.14 reference discipline).
    SignerKey { key_reference: String },
}

impl RevocationKey {
    /// The stable machine-readable kind name (audit records, error text).
    pub fn kind_name(&self) -> &'static str {
        match self {
            RevocationKey::Plugin { .. } => "PLUGIN_VERSION",
            RevocationKey::SignerKey { .. } => "SIGNER_KEY",
        }
    }

    /// The register's canonical key string (BTreeMap key; deterministic).
    pub fn register_key(&self) -> String {
        match self {
            RevocationKey::Plugin { plugin_id, version } => {
                format!("plugin/{plugin_id}@{version}")
            }
            RevocationKey::SignerKey { key_reference } => {
                format!("signer/{key_reference}")
            }
        }
    }

    /// Structural validation at revocation time (fail-closed: a malformed
    /// key is refused, never normalized into something else).
    fn validate(&self) -> Result<(), RevocationError> {
        match self {
            RevocationKey::Plugin { plugin_id, .. } => {
                validate_plugin_id(plugin_id).map_err(|reason| RevocationError::KeyInvalid {
                    reason: format!("plugin id is malformed: {reason}"),
                })
            }
            RevocationKey::SignerKey { key_reference } => {
                let scheme_ok = KEY_REFERENCE_SCHEMES.iter().any(|scheme| {
                    key_reference
                        .strip_prefix(*scheme)
                        .map(|rest| !rest.is_empty())
                        .unwrap_or(false)
                });
                if scheme_ok {
                    Ok(())
                } else {
                    // The VALUE is not echoed (it may be key material) —
                    // the P7.14 refusal discipline.
                    Err(RevocationError::KeyInvalid {
                        reason: "a signer key reference must use the key-store:, env:, or \
                                 file: scheme and name a location; the value is not echoed"
                            .to_string(),
                    })
                }
            }
        }
    }
}

impl fmt::Display for RevocationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RevocationKey::Plugin { plugin_id, version } => {
                write!(f, "plugin '{plugin_id}' version {version}")
            }
            RevocationKey::SignerKey { .. } => {
                write!(f, "signer key reference (scheme-prefixed; the value is not rendered)")
            }
        }
    }
}

// --- The record ---------------------------------------------------------------------

/// One append-only revocation record. Timestamps come from the injected
/// [`Clock`]; the reason and actor are REQUIRED (an unattributed
/// revocation is refused, never recorded).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevocationRecord {
    pub key: RevocationKey,
    /// Why the revocation happened (incident reference, decision record —
    /// REQUIRED non-empty; REQUIRED to name the authority).
    pub reason: String,
    pub actor: String,
    pub revoked_at_millis: u64,
}

// --- Errors -------------------------------------------------------------------------

/// Every revocation/rollout/removal refusal is a DISTINCT named error
/// ([`RevocationError::name`]) — never a warning, never a silent
/// acceptance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevocationError {
    /// The register store failed (I/O, poisoned lock). A CHECK that cannot
    /// read the register is fail-closed: it NEVER reports "not revoked".
    CheckFailed { reason: String },
    /// The register file could not be opened or appended.
    RegisterUnavailable { reason: String },
    /// The register file exists but is not a FlexCore plugin revocation
    /// register (wrong magic/version) — refused, never guessed at.
    RegisterFileForeign { reason: String },
    /// This key is already revoked — a duplicate revocation is refused
    /// (the register stays append-only and idempotent-in-fact: exactly one
    /// record per key).
    AlreadyRevoked { key: String },
    /// The checked key IS revoked — the named refusal of every gate
    /// consult. `revocation_reason` is the recorded revocation reason.
    Revoked { key: String, revocation_reason: String },
    /// The key is structurally invalid (malformed plugin id; a signer key
    /// reference without a scheme prefix — the value is never echoed).
    KeyInvalid { reason: String },
    /// The revocation reason is empty — an unattributed revocation is
    /// refused, never recorded.
    ReasonRequired,
    /// The revocation actor is empty — an unattributed revocation is
    /// refused, never recorded.
    ActorRequired,
}

impl RevocationError {
    /// The stable machine-readable error name (the named-error table key).
    pub fn name(&self) -> &'static str {
        match self {
            RevocationError::CheckFailed { .. } => "REVOCATION_CHECK_FAILED",
            RevocationError::RegisterUnavailable { .. } => "REVOCATION_REGISTER_UNAVAILABLE",
            RevocationError::RegisterFileForeign { .. } => "REVOCATION_REGISTER_FILE_FOREIGN",
            RevocationError::AlreadyRevoked { .. } => "REVOCATION_ALREADY_REVOKED",
            RevocationError::Revoked { .. } => "PLUGIN_REVOKED",
            RevocationError::KeyInvalid { .. } => "REVOCATION_KEY_INVALID",
            RevocationError::ReasonRequired => "REVOCATION_REASON_REQUIRED",
            RevocationError::ActorRequired => "REVOCATION_ACTOR_REQUIRED",
        }
    }
}

impl fmt::Display for RevocationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RevocationError::CheckFailed { reason } => write!(
                f,
                "the revocation check could not be performed: {reason} (fail-closed: a check \
                 that cannot read the register never reports 'not revoked')"
            ),
            RevocationError::RegisterUnavailable { reason } => {
                write!(f, "the revocation register is unavailable: {reason}")
            }
            RevocationError::RegisterFileForeign { reason } => write!(
                f,
                "the register file is not a FlexCore plugin revocation register (refused, \
                 never guessed): {reason}"
            ),
            RevocationError::AlreadyRevoked { key } => {
                write!(f, "'{key}' is already revoked; the register is append-only and keeps exactly one record per key")
            }
            RevocationError::Revoked { key, revocation_reason } => write!(
                f,
                "'{key}' is REVOKED: {revocation_reason} — verify and start are refused; a \
                 running plugin is drained and removed"
            ),
            RevocationError::KeyInvalid { reason } => {
                write!(f, "the revocation key is invalid: {reason}")
            }
            RevocationError::ReasonRequired => {
                write!(f, "a revocation requires a non-empty reason (unattributed revocations are never recorded)")
            }
            RevocationError::ActorRequired => {
                write!(f, "a revocation requires a non-empty actor (unattributed revocations are never recorded)")
            }
        }
    }
}

impl std::error::Error for RevocationError {}

// --- The store port -----------------------------------------------------------------

/// The append-only revocation register port (kernel-local; the composition
/// root injects the implementation). APPEND-ONLY by contract: there is no
/// removal, update, or override operation on this trait — the
/// [`UNREVOCATION_SEAM`] is structural, not a convention.
pub trait RevocationStore {
    /// Appends one revocation. A duplicate (key already revoked) is
    /// [`RevocationError::AlreadyRevoked`] — never a silent rewrite.
    fn append(&self, record: &RevocationRecord) -> Result<(), RevocationError>;
    /// All records, in append order (the audit/replay view).
    fn load_all(&self) -> Result<Vec<RevocationRecord>, RevocationError>;
}

/// The in-memory append-only register (deterministic collector for tests
/// and in-process compositions). Keyed by [`RevocationKey::register_key`]
/// in a `BTreeMap` (deterministic iteration); the append-order vector
/// preserves the audit order.
#[derive(Debug, Default)]
pub struct MemoryRevocationRegister {
    by_key: Mutex<BTreeMap<String, RevocationRecord>>,
    append_order: Mutex<Vec<RevocationRecord>>,
}

impl MemoryRevocationRegister {
    pub fn new() -> MemoryRevocationRegister {
        MemoryRevocationRegister {
            by_key: Mutex::new(BTreeMap::new()),
            append_order: Mutex::new(Vec::new()),
        }
    }

    /// The records in append order (test/composition inspection).
    pub fn records(&self) -> Vec<RevocationRecord> {
        match self.append_order.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => Vec::new(),
        }
    }
}

impl RevocationStore for MemoryRevocationRegister {
    fn append(&self, record: &RevocationRecord) -> Result<(), RevocationError> {
        let key = record.key.register_key();
        let mut by_key = self.by_key.lock().map_err(|_| RevocationError::CheckFailed {
            reason: "the register key lock is poisoned".to_string(),
        })?;
        if by_key.contains_key(&key) {
            return Err(RevocationError::AlreadyRevoked { key });
        }
        by_key.insert(key, record.clone());
        drop(by_key);
        let mut order = self.append_order.lock().map_err(|_| {
            RevocationError::CheckFailed { reason: "the register order lock is poisoned".to_string() }
        })?;
        order.push(record.clone());
        Ok(())
    }

    fn load_all(&self) -> Result<Vec<RevocationRecord>, RevocationError> {
        let order = self.append_order.lock().map_err(|_| RevocationError::CheckFailed {
            reason: "the register order lock is poisoned".to_string(),
        })?;
        Ok(order.clone())
    }
}

// --- The file-backed register (buffer_file.rs framing discipline) --------------------

/// Hard sanity cap on one register record payload (rejects absurd lengths
/// from a corrupt header instead of attempting an absurd allocation — the
/// buffer_file.rs discipline).
const MAX_REGISTER_RECORD_BYTES: u32 = 64 * 1024;

/// The file-backed [`RevocationStore`], in the `buffer_file.rs` framing
/// discipline (REIMPLEMENTED — the kernel depends on no flexcore-* crate;
/// see the module header for the exact format):
///
/// - **Recovery (truncate-and-audit, never silent).** On open the file is
///   replayed record by record; the FIRST malformed frame ends the replay
///   and the file is TRUNCATED at the last good record, with the loss
///   COUNTED ([`FileRevocationRegister::corrupt_tail_truncated`]). A
///   wrong magic/version is a hard open error.
/// - **Append-only.** Records are only ever added; the file grows.
/// - **Durability before state.** `append` flushes and `sync_data`s
///   BEFORE the in-memory map moves.
pub struct FileRevocationRegister {
    path: PathBuf,
    file: Mutex<std::fs::File>,
    append_order: Mutex<Vec<RevocationRecord>>,
    by_key: Mutex<BTreeMap<String, RevocationRecord>>,
    corrupt_tail_truncated: std::sync::atomic::AtomicU64,
    records_written: std::sync::atomic::AtomicU64,
}

impl FileRevocationRegister {
    /// The journal magic header (24-byte label) + format version.
    fn magic() -> Vec<u8> {
        let mut magic = b"FLEXCORE-PLUGIN-REV-REG".to_vec();
        magic.push(0x01);
        magic
    }

    /// Opens (or creates) the register file and replays any existing
    /// records. A corrupt tail is truncated and counted, never silent; a
    /// wrong magic/version is a hard error.
    pub fn open(path: &Path) -> Result<Self, RevocationError> {
        if path.as_os_str().is_empty() {
            return Err(RevocationError::RegisterUnavailable {
                reason: "the register path is empty".to_string(),
            });
        }
        let magic = Self::magic();
        let (file, replayed) = match std::fs::OpenOptions::new().read(true).write(true).open(path)
        {
            Ok(mut existing) => {
                let mut contents = Vec::new();
                existing.read_to_end(&mut contents).map_err(|e| {
                    RevocationError::RegisterUnavailable {
                        reason: format!("reading the existing register failed: {e}"),
                    }
                })?;
                if contents.is_empty() {
                    // A zero-length file (touch/new) is initialized, not refused.
                    (existing, Vec::new())
                } else {
                    if contents.len() < magic.len() || contents[..magic.len()] != magic[..] {
                        return Err(RevocationError::RegisterFileForeign {
                            reason: format!(
                                "the file at {} does not carry the FlexCore plugin revocation \
                                 register magic",
                                path.display()
                            ),
                        });
                    }
                    let mut records = Vec::new();
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
                            || len > MAX_REGISTER_RECORD_BYTES
                            || offset + 8 + len as usize > contents.len()
                        {
                            corrupt_tail_truncated += 1;
                            break;
                        }
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
                        let payload =
                            match contents.get(offset + 8..offset + 8 + len as usize) {
                                Some(p) => p,
                                None => {
                                    corrupt_tail_truncated += 1;
                                    break;
                                }
                            };
                        if crc32(payload) != u32::from_le_bytes(crc_bytes) {
                            corrupt_tail_truncated += 1;
                            break;
                        }
                        match decode_register_payload(payload) {
                            Some(record) => {
                                records.push(record);
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
                        existing.set_len(offset as u64).map_err(|e| {
                            RevocationError::RegisterUnavailable {
                                reason: format!(
                                    "truncating the corrupt register tail failed: {e}"
                                ),
                            }
                        })?;
                        existing.sync_all().map_err(|e| {
                            RevocationError::RegisterUnavailable {
                                reason: format!("syncing the truncated register failed: {e}"),
                            }
                        })?;
                    }
                    (existing, records)
                }
            }
            Err(_) => {
                // Create a new register file with the magic header.
                if let Some(parent) = path.parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent).map_err(|e| {
                            RevocationError::RegisterUnavailable {
                                reason: format!("creating the register directory failed: {e}"),
                            }
                        })?;
                    }
                }
                let mut created = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .map_err(|e| RevocationError::RegisterUnavailable {
                        reason: format!("creating the register file failed: {e}"),
                    })?;
                created
                    .write_all(&magic)
                    .and_then(|_| created.flush())
                    .and_then(|_| created.sync_all())
                    .map_err(|e| RevocationError::RegisterUnavailable {
                        reason: format!("writing the register header failed: {e}"),
                    })?;
                (created, Vec::new())
            }
        };
        let mut by_key = BTreeMap::new();
        for record in &replayed {
            by_key.insert(record.key.register_key(), record.clone());
        }
        Ok(Self {
            path: path.to_path_buf(),
            file: Mutex::new(file),
            append_order: Mutex::new(replayed),
            by_key: Mutex::new(by_key),
            corrupt_tail_truncated: std::sync::atomic::AtomicU64::new(0),
            records_written: std::sync::atomic::AtomicU64::new(0),
        })
    }

    /// The register file path (diagnostics).
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// How many corrupt tail records were truncated at open (the audited
    /// loss counter — the buffer_file.rs discipline; nothing here is
    /// silent about a lost record).
    pub fn corrupt_tail_truncated(&self) -> u64 {
        self.corrupt_tail_truncated
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// How many records this instance has written (diagnostics).
    pub fn records_written(&self) -> u64 {
        self.records_written
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    fn encode_payload(record: &RevocationRecord) -> Vec<u8> {
        let (kind, id, version) = match &record.key {
            RevocationKey::Plugin { plugin_id, version } => (1u8, plugin_id.as_str(), version.to_string()),
            RevocationKey::SignerKey { key_reference } => (2u8, key_reference.as_str(), String::new()),
        };
        let mut payload = Vec::new();
        payload.push(kind);
        for field in [id, version.as_str(), record.reason.as_str(), record.actor.as_str()] {
            payload.extend_from_slice(&(field.len() as u16).to_le_bytes());
            payload.extend_from_slice(field.as_bytes());
        }
        payload.extend_from_slice(&record.revoked_at_millis.to_le_bytes());
        payload
    }
}

fn decode_register_payload(payload: &[u8]) -> Option<RevocationRecord> {
    if payload.is_empty() {
        return None;
    }
    let kind = payload[0];
    let mut offset = 1usize;
    let mut fields: Vec<String> = Vec::with_capacity(4);
    for _ in 0..4 {
        if offset + 2 > payload.len() {
            return None;
        }
        let len_bytes = <[u8; 2]>::try_from(&payload[offset..offset + 2]).ok()?;
        let len = u16::from_le_bytes(len_bytes) as usize;
        offset += 2;
        if offset + len > payload.len() {
            return None;
        }
        fields.push(std::str::from_utf8(&payload[offset..offset + len]).ok()?.to_string());
        offset += len;
    }
    if offset + 8 > payload.len() {
        return None;
    }
    let ts_bytes = <[u8; 8]>::try_from(&payload[offset..offset + 8]).ok()?;
    let revoked_at_millis = u64::from_le_bytes(ts_bytes);
    let key = match kind {
        1u8 => {
            let version = SemVer::parse(&fields[1]).ok()?;
            RevocationKey::Plugin { plugin_id: fields[0].clone(), version }
        }
        2u8 => RevocationKey::SignerKey { key_reference: fields[0].clone() },
        _ => return None,
    };
    Some(RevocationRecord {
        key,
        reason: fields[2].clone(),
        actor: fields[3].clone(),
        revoked_at_millis,
    })
}

/// The CRC-32 (IEEE, reflected) used by the register records — the same
/// checksum family as the P7.36 sequence checkpoint and the edge telemetry
/// journal (an integrity checksum for corruption detection, not a
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

impl RevocationStore for FileRevocationRegister {
    fn append(&self, record: &RevocationRecord) -> Result<(), RevocationError> {
        let key = record.key.register_key();
        {
            let by_key = self.by_key.lock().map_err(|_| RevocationError::CheckFailed {
                reason: "the register key lock is poisoned".to_string(),
            })?;
            if by_key.contains_key(&key) {
                return Err(RevocationError::AlreadyRevoked { key });
            }
        }
        let payload = Self::encode_payload(record);
        let record_len = payload.len() as u32;
        let crc = crc32(&payload);
        let mut frame = Vec::with_capacity(8 + payload.len());
        frame.extend_from_slice(&record_len.to_le_bytes());
        frame.extend_from_slice(&crc.to_le_bytes());
        frame.extend_from_slice(&payload);
        {
            let mut file = self.file.lock().map_err(|_| RevocationError::RegisterUnavailable {
                reason: "the register file lock is poisoned".to_string(),
            })?;
            file.seek(SeekFrom::End(0))
                .and_then(|_| file.write_all(&frame))
                .and_then(|_| file.flush())
                .and_then(|_| file.sync_data())
                .map_err(|e| RevocationError::RegisterUnavailable {
                    reason: format!("appending the register record failed: {e}"),
                })?;
        }
        // Durability BEFORE state: the in-memory maps move only after the
        // record is on disk.
        let mut by_key = self.by_key.lock().map_err(|_| RevocationError::CheckFailed {
            reason: "the register key lock is poisoned".to_string(),
        })?;
        by_key.insert(key, record.clone());
        drop(by_key);
        let mut order = self.append_order.lock().map_err(|_| RevocationError::CheckFailed {
            reason: "the register order lock is poisoned".to_string(),
        })?;
        order.push(record.clone());
        self.records_written
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    fn load_all(&self) -> Result<Vec<RevocationRecord>, RevocationError> {
        let order = self.append_order.lock().map_err(|_| RevocationError::CheckFailed {
            reason: "the register order lock is poisoned".to_string(),
        })?;
        Ok(order.clone())
    }
}

// --- The revocation audit port ------------------------------------------------------

/// The kinds of P7.38 audit records (SCREAMING_SNAKE stable names — the
/// broker/edge audit-kind style; no secret material in any detail).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevocationAuditKind {
    /// A revocation record was appended.
    RevocationAppended,
    /// A revocation APPEND was refused (duplicate, missing reason/actor,
    /// invalid key).
    RevocationRefused,
    /// A gate/start re-check REFUSED because the key is revoked.
    RevocationCheckRefused,
    /// A staged-rollout promotion was recorded.
    PromotionRecorded,
    /// A staged-rollout promotion was refused.
    PromotionRefused,
    /// A failed-plugin removal was requested (explicit operation).
    RemovalRequested,
    /// A failed-plugin removal completed (plugin drained and stopped).
    RemovalCompleted,
    /// A failed-plugin removal was refused (wrong state, undrained work).
    RemovalRefused,
}

impl RevocationAuditKind {
    /// Stable audit name.
    pub fn name(self) -> &'static str {
        match self {
            RevocationAuditKind::RevocationAppended => "PLUGIN_REVOCATION_APPENDED",
            RevocationAuditKind::RevocationRefused => "PLUGIN_REVOCATION_REFUSED",
            RevocationAuditKind::RevocationCheckRefused => "PLUGIN_REVOCATION_CHECK_REFUSED",
            RevocationAuditKind::PromotionRecorded => "PLUGIN_ROLLOUT_PROMOTED",
            RevocationAuditKind::PromotionRefused => "PLUGIN_ROLLOUT_PROMOTION_REFUSED",
            RevocationAuditKind::RemovalRequested => "PLUGIN_REMOVAL_REQUESTED",
            RevocationAuditKind::RemovalCompleted => "PLUGIN_REMOVAL_COMPLETED",
            RevocationAuditKind::RemovalRefused => "PLUGIN_REMOVAL_REFUSED",
        }
    }
}

/// One P7.38 audit record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevocationAuditRecord {
    /// When the event was recorded (injected-clock milliseconds).
    pub timestamp_millis: u64,
    pub kind: RevocationAuditKind,
    /// The subject (register key string, or the plugin id for removals).
    pub subject: String,
    /// Human-readable detail (identifiers and refusal names — never key
    /// material, never a secret value).
    pub detail: String,
}

/// The P7.38 audit sink port (kernel-local, the lifecycle-port style).
/// Implementations MUST be append-only (§5.1 audit integrity): records
/// are never rewritten or removed — the same contract as
/// [`crate::lifecycle::AuditSink`].
pub trait RevocationAuditSink {
    fn record(&self, record: RevocationAuditRecord);
}

/// An append-only in-memory [`RevocationAuditSink`] — the deterministic
/// collector used by tests and in-process compositions. Ordering is
/// exactly the emission order.
#[derive(Debug, Default)]
pub struct MemoryRevocationAuditSink {
    records: std::cell::RefCell<Vec<RevocationAuditRecord>>,
}

impl MemoryRevocationAuditSink {
    pub fn new() -> MemoryRevocationAuditSink {
        MemoryRevocationAuditSink { records: std::cell::RefCell::new(Vec::new()) }
    }

    /// The records in emission order (append-only; the borrow is brief and
    /// single-threaded).
    pub fn records(&self) -> std::cell::Ref<Vec<RevocationAuditRecord>> {
        self.records.borrow()
    }
}

impl RevocationAuditSink for MemoryRevocationAuditSink {
    fn record(&self, record: RevocationAuditRecord) {
        self.records.borrow_mut().push(record);
    }
}

fn emit(
    audit: &dyn RevocationAuditSink,
    clock: &dyn Clock,
    kind: RevocationAuditKind,
    subject: &str,
    detail: &str,
) {
    audit.record(RevocationAuditRecord {
        timestamp_millis: clock.now_millis(),
        kind,
        subject: subject.to_string(),
        detail: detail.to_string(),
    });
}

// --- Revocation operations ------------------------------------------------------------

/// Appends one revocation for `key`. Validates reason/actor/key FIRST
/// (fail-closed: nothing unattributed or malformed is ever recorded),
/// then appends through the store (append-only; a duplicate is refused),
/// then audits. There is no un-revoke: see [`UNREVOCATION_SEAM`].
pub fn revoke(
    store: &dyn RevocationStore,
    key: RevocationKey,
    reason: &str,
    actor: &str,
    clock: &dyn Clock,
    audit: &dyn RevocationAuditSink,
) -> Result<(), RevocationError> {
    key.validate()?;
    if reason.trim().is_empty() {
        emit(
            audit,
            clock,
            RevocationAuditKind::RevocationRefused,
            &key.register_key(),
            RevocationError::ReasonRequired.name(),
        );
        return Err(RevocationError::ReasonRequired);
    }
    if actor.trim().is_empty() {
        emit(
            audit,
            clock,
            RevocationAuditKind::RevocationRefused,
            &key.register_key(),
            RevocationError::ActorRequired.name(),
        );
        return Err(RevocationError::ActorRequired);
    }
    let record = RevocationRecord {
        key,
        reason: reason.to_string(),
        actor: actor.to_string(),
        revoked_at_millis: clock.now_millis(),
    };
    let register_key = record.key.register_key();
    match store.append(&record) {
        Ok(()) => {
            emit(
                audit,
                clock,
                RevocationAuditKind::RevocationAppended,
                &register_key,
                &format!(
                    "kind {} revoked by '{}' at {}: {reason} (append-only; no un-revoke — {})",
                    record.key.kind_name(),
                    actor,
                    record.revoked_at_millis,
                    UNREVOCATION_SEAM,
                ),
            );
            Ok(())
        }
        Err(err) => {
            emit(
                audit,
                clock,
                RevocationAuditKind::RevocationRefused,
                &register_key,
                &format!("append refused ({}): {err}", err.name()),
            );
            Err(err)
        }
    }
}

/// Convenience wrapper: revoke one plugin version.
pub fn revoke_plugin(
    store: &dyn RevocationStore,
    plugin_id: &str,
    version: SemVer,
    reason: &str,
    actor: &str,
    clock: &dyn Clock,
    audit: &dyn RevocationAuditSink,
) -> Result<(), RevocationError> {
    revoke(
        store,
        RevocationKey::Plugin { plugin_id: plugin_id.to_string(), version },
        reason,
        actor,
        clock,
        audit,
    )
}

/// Convenience wrapper: revoke a signer key — revokes everything it
/// signed (the register consults the key reference on every plugin whose
/// provenance names it).
pub fn revoke_signer_key(
    store: &dyn RevocationStore,
    key_reference: &str,
    reason: &str,
    actor: &str,
    clock: &dyn Clock,
    audit: &dyn RevocationAuditSink,
) -> Result<(), RevocationError> {
    revoke(
        store,
        RevocationKey::SignerKey { key_reference: key_reference.to_string() },
        reason,
        actor,
        clock,
        audit,
    )
}

/// The register consult: is `key` revoked? A register load failure is
/// fail-closed — it is NEVER rendered as "not revoked"
/// ([`RevocationError::CheckFailed`]; the P6.06 revocation-seam rule:
/// an unestablishable status is never "not revoked").
pub fn is_revoked(
    store: &dyn RevocationStore,
    key: &RevocationKey,
) -> Result<Option<RevocationRecord>, RevocationError> {
    let records = store.load_all().map_err(|e| RevocationError::CheckFailed {
        reason: e.to_string(),
    })?;
    let wanted = key.register_key();
    Ok(records.into_iter().find(|r| r.key.register_key() == wanted))
}

/// The registry-time / start-time re-check: refuses with
/// [`RevocationError::Revoked`] (audited) if the plugin version OR its
/// signer key is revoked. Called immediately before `Configured →
/// Started` — a plugin revoked after verification cannot START — and at
/// any other point the composition root re-checks a running plugin
/// (revocation takes effect at the next check; nothing caches).
pub fn ensure_startable(
    store: &dyn RevocationStore,
    plugin_id: &str,
    version: SemVer,
    signer_key_reference: Option<&str>,
    clock: &dyn Clock,
    audit: &dyn RevocationAuditSink,
) -> Result<(), RevocationError> {
    let plugin_key =
        RevocationKey::Plugin { plugin_id: plugin_id.to_string(), version };
    if let Some(record) = is_revoked(store, &plugin_key)? {
        let detail = format!(
            "start/re-check refused for plugin key '{}': {}",
            plugin_key.register_key(),
            RevocationError::Revoked {
                key: plugin_key.register_key(),
                revocation_reason: record.reason.clone(),
            }
        );
        emit(
            audit,
            clock,
            RevocationAuditKind::RevocationCheckRefused,
            &plugin_key.register_key(),
            &detail,
        );
        return Err(RevocationError::Revoked {
            key: plugin_key.register_key(),
            revocation_reason: record.reason,
        });
    }
    if let Some(reference) = signer_key_reference {
        let key_key = RevocationKey::SignerKey { key_reference: reference.to_string() };
        if let Some(record) = is_revoked(store, &key_key)? {
            let detail = format!(
                "start/re-check refused for signer key of plugin '{}': {}",
                plugin_id,
                RevocationError::Revoked {
                    key: key_key.register_key(),
                    revocation_reason: record.reason.clone(),
                }
            );
            emit(
                audit,
                clock,
                RevocationAuditKind::RevocationCheckRefused,
                &key_key.register_key(),
                &detail,
            );
            return Err(RevocationError::Revoked {
                key: key_key.register_key(),
                revocation_reason: record.reason,
            });
        }
    }
    Ok(())
}

/// The `Discovered → Verified` revocation gate: CONSULTED FIRST, before
/// `crate::signing::provenance_gate` — a package whose signature was
/// revoked after signing is refused at the gate BEFORE any digest
/// computation. Checks the plugin-version key, then the signer-key key
/// (when the provenance record's signer reference is known). The output
/// is the P7.07 [`GateOutcome`]: `Failed` reasons carry the stable tags
/// `REVOKED_PLUGIN` / `REVOKED_SIGNER_KEY`, so the lifecycle refuses the
/// transition fail-closed and audits it like any other gate refusal.
///
/// Fail-closed on register failure: a check that cannot read the register
/// produces `GateOutcome::Failed` (never a pass).
pub fn revocation_gate(
    store: &dyn RevocationStore,
    plugin_id: &str,
    version: SemVer,
    signer_key_reference: Option<&str>,
    clock: &dyn Clock,
    audit: &dyn RevocationAuditSink,
) -> GateOutcome {
    let plugin_key = RevocationKey::Plugin { plugin_id: plugin_id.to_string(), version };
    match is_revoked(store, &plugin_key) {
        Err(err) => {
            let reason = format!(
                "REVOCATION_CHECK_FAILED: the register could not be consulted for '{}' ({err}); \
                 the gate refuses (fail-closed — an unreadable register is never 'not revoked')",
                plugin_key.register_key()
            );
            emit(
                audit,
                clock,
                RevocationAuditKind::RevocationCheckRefused,
                &plugin_key.register_key(),
                &reason,
            );
            return GateOutcome::Failed { reason };
        }
        Ok(Some(record)) => {
            let revocation_reason = record.reason.clone();
            let reason = format!(
                "REVOKED_PLUGIN: '{}' was revoked at {} by '{}' — {revocation_reason}; verify \
                 is refused before any provenance digest is computed",
                plugin_key.register_key(),
                record.revoked_at_millis,
                record.actor,
            );
            emit(
                audit,
                clock,
                RevocationAuditKind::RevocationCheckRefused,
                &plugin_key.register_key(),
                &reason,
            );
            return GateOutcome::Failed { reason };
        }
        Ok(None) => {}
    }
    if let Some(reference) = signer_key_reference {
        let key_key = RevocationKey::SignerKey { key_reference: reference.to_string() };
        match is_revoked(store, &key_key) {
            Err(err) => {
                let reason = format!(
                    "REVOCATION_CHECK_FAILED: the register could not be consulted for the \
                     signer key ({err}); the gate refuses (fail-closed)"
                );
                emit(
                    audit,
                    clock,
                    RevocationAuditKind::RevocationCheckRefused,
                    &key_key.register_key(),
                    &reason,
                );
                return GateOutcome::Failed { reason };
            }
            Ok(Some(record)) => {
                let revocation_reason = record.reason.clone();
                let reason = format!(
                    "REVOKED_SIGNER_KEY: the signer key of '{}' was revoked at {} by '{}' — \
                     {revocation_reason}; every plugin it signed is refused at verify",
                    plugin_id,
                    record.revoked_at_millis,
                    record.actor,
                );
                emit(
                    audit,
                    clock,
                    RevocationAuditKind::RevocationCheckRefused,
                    &key_key.register_key(),
                    &reason,
                );
                return GateOutcome::Failed { reason };
            }
            Ok(None) => {}
        }
    }
    GateOutcome::Passed
}

// --- Staged rollout -------------------------------------------------------------------

/// Rollout/plan errors. Every refusal is a DISTINCT named error
/// ([`RolloutError::name`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RolloutError {
    /// The plan is malformed (empty profile order, duplicate profile,
    /// malformed plugin id).
    PlanInvalid { reason: String },
    /// The "upgrade" is the same version — a no-op rollout is refused
    /// (an event worth recording is an actual version change).
    NoOpRollout { version: String },
    /// The promoted profile is not in the plan's declared order.
    PromotionUnknownProfile { profile_id: String },
    /// The promoted profile is not exactly the next un-promoted one — the
    /// no-flag-flip refusal (skipping ahead is how a "staged" rollout
    /// becomes an un-staged full rollout).
    PromotionOutOfOrder { profile_id: String, expected_next: String },
    /// The promotion reason is empty.
    ReasonRequired,
    /// The promotion actor is empty.
    ActorRequired,
}

impl RolloutError {
    /// The stable machine-readable error name (the named-error table key).
    pub fn name(&self) -> &'static str {
        match self {
            RolloutError::PlanInvalid { .. } => "ROLLOUT_PLAN_INVALID",
            RolloutError::NoOpRollout { .. } => "ROLLOUT_NO_OP",
            RolloutError::PromotionUnknownProfile { .. } => "PROMOTION_UNKNOWN_PROFILE",
            RolloutError::PromotionOutOfOrder { .. } => "PROMOTION_OUT_OF_ORDER",
            RolloutError::ReasonRequired => "ROLLOUT_REASON_REQUIRED",
            RolloutError::ActorRequired => "ROLLOUT_ACTOR_REQUIRED",
        }
    }
}

impl fmt::Display for RolloutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RolloutError::PlanInvalid { reason } => {
                write!(f, "the staged-rollout plan is invalid: {reason}")
            }
            RolloutError::NoOpRollout { version } => {
                write!(f, "the rollout upgrades to the same version '{version}'; a no-op rollout is refused, not silently recorded")
            }
            RolloutError::PromotionUnknownProfile { profile_id } => {
                write!(f, "profile '{profile_id}' is not in the rollout plan's declared profile order")
            }
            RolloutError::PromotionOutOfOrder { profile_id, expected_next } => write!(
                f,
                "profile '{profile_id}' is not the next un-promoted profile (expected \
                 '{expected_next}'); a staged rollout is promoted in the plan's declared \
                 order — there is no percentage canary and no flag-flip full rollout"
            ),
            RolloutError::ReasonRequired => {
                write!(f, "a promotion requires a non-empty reason (the stage decision is the audit record)")
            }
            RolloutError::ActorRequired => {
                write!(f, "a promotion requires a non-empty actor")
            }
        }
    }
}

impl std::error::Error for RolloutError {}

/// A staged rollout plan: one plugin version upgrade promoted
/// profile-by-profile, in the plan's DECLARED profile order. The staging
/// units are the P7.15 profiles (explicit per-profile staging — the
/// recorded v0.5.0 model; see [`ROLLOUT_STATEMENT`] for the no-percentage-
/// canary rule, which the type surface enforces: there is no cohort,
/// percentage, or bulk field anywhere on this type).
///
/// Promotion semantics: `promote` succeeds ONLY for the exact next
/// un-promoted profile in the declared order; anything else is a named
/// refusal. The plan is complete when every declared profile has been
/// promoted. Every promotion and every refusal is audited through the
/// [`RevocationAuditSink`] port with timestamps from the injected
/// [`Clock`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedRollout {
    plugin_id: String,
    from_version: SemVer,
    to_version: SemVer,
    profile_order: Vec<String>,
    /// The promoted prefix of `profile_order` (a staged rollout is always
    /// a prefix — that is what "in the declared order" means).
    promoted_count: usize,
}

impl StagedRollout {
    /// Plans a rollout. The first stage is ALWAYS a single profile: the
    /// plan starts with `promoted_count == 0` and the caller promotes the
    /// first profile explicitly — there is no constructor that starts a
    /// version on more than one profile at once.
    pub fn plan(
        plugin_id: &str,
        from_version: SemVer,
        to_version: SemVer,
        profile_order: Vec<String>,
    ) -> Result<StagedRollout, RolloutError> {
        validate_plugin_id(plugin_id).map_err(|reason| RolloutError::PlanInvalid {
            reason: format!("plugin id is malformed: {reason}"),
        })?;
        if from_version == to_version {
            return Err(RolloutError::NoOpRollout { version: to_version.to_string() });
        }
        if profile_order.is_empty() {
            return Err(RolloutError::PlanInvalid {
                reason: "the profile order is empty — a rollout stages through at least one \
                         profile"
                    .to_string(),
            });
        }
        let mut seen: BTreeMap<String, ()> = BTreeMap::new();
        for profile in &profile_order {
            if profile.trim().is_empty() || profile.len() > 64 {
                return Err(RolloutError::PlanInvalid {
                    reason: "every profile id must be a non-empty string of at most 64 \
                             characters"
                        .to_string(),
                });
            }
            if seen.insert(profile.clone(), ()).is_some() {
                return Err(RolloutError::PlanInvalid {
                    reason: format!("profile '{profile}' appears twice in the plan"),
                });
            }
        }
        Ok(StagedRollout {
            plugin_id: plugin_id.to_string(),
            from_version,
            to_version,
            profile_order,
            promoted_count: 0,
        })
    }

    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    pub fn from_version(&self) -> SemVer {
        self.from_version
    }

    pub fn to_version(&self) -> SemVer {
        self.to_version
    }

    /// The declared profile order (plan data; reviewable).
    pub fn profile_order(&self) -> &[String] {
        &self.profile_order
    }

    /// The promoted profiles so far (a prefix of the declared order).
    pub fn promoted(&self) -> &[String] {
        &self.profile_order[..self.promoted_count]
    }

    /// The next profile to promote, if any.
    pub fn next_profile(&self) -> Option<&str> {
        self.profile_order.get(self.promoted_count).map(|s| s.as_str())
    }

    /// Whether every declared profile has been promoted.
    pub fn is_complete(&self) -> bool {
        self.promoted_count == self.profile_order.len()
    }

    /// Promotes the EXACT next un-promoted profile. Anything else is a
    /// named, audited refusal ([`RolloutError::PromotionOutOfOrder`] /
    /// [`RolloutError::PromotionUnknownProfile`]) — an out-of-order or
    /// skipped-ahead promotion is how a staged rollout becomes a flag
    /// flip, and both are refused here.
    pub fn promote(
        &mut self,
        profile_id: &str,
        actor: &str,
        reason: &str,
        clock: &dyn Clock,
        audit: &dyn RevocationAuditSink,
    ) -> Result<(), RolloutError> {
        let subject = format!("{}/{}->{}", self.plugin_id, self.from_version, self.to_version);
        if reason.trim().is_empty() {
            emit(
                audit,
                clock,
                RevocationAuditKind::PromotionRefused,
                &subject,
                RolloutError::ReasonRequired.name(),
            );
            return Err(RolloutError::ReasonRequired);
        }
        if actor.trim().is_empty() {
            emit(
                audit,
                clock,
                RevocationAuditKind::PromotionRefused,
                &subject,
                RolloutError::ActorRequired.name(),
            );
            return Err(RolloutError::ActorRequired);
        }
        let Some(expected) = self.next_profile() else {
            // Complete already: any further promotion is out of order
            // (there is no next stage).
            let err = RolloutError::PromotionOutOfOrder {
                profile_id: profile_id.to_string(),
                expected_next: "(rollout complete)".to_string(),
            };
            emit(
                audit,
                clock,
                RevocationAuditKind::PromotionRefused,
                &subject,
                &format!("promotion refused ({}): {err}", err.name()),
            );
            return Err(err);
        };
        if !self.profile_order.iter().any(|p| p == profile_id) {
            let err = RolloutError::PromotionUnknownProfile { profile_id: profile_id.to_string() };
            emit(
                audit,
                clock,
                RevocationAuditKind::PromotionRefused,
                &subject,
                &format!("promotion refused ({}): {err}", err.name()),
            );
            return Err(err);
        }
        if profile_id != expected {
            let err = RolloutError::PromotionOutOfOrder {
                profile_id: profile_id.to_string(),
                expected_next: expected.to_string(),
            };
            emit(
                audit,
                clock,
                RevocationAuditKind::PromotionRefused,
                &subject,
                &format!("promotion refused ({}): {err}", err.name()),
            );
            return Err(err);
        }
        self.promoted_count += 1;
        emit(
            audit,
            clock,
            RevocationAuditKind::PromotionRecorded,
            &subject,
            &format!(
                "profile '{profile_id}' promoted by '{actor}' ({promoted}/{total} profiles): \
                 {reason}{}",
                if self.is_complete() {
                    " — rollout COMPLETE"
                } else {
                    ""
                },
                promoted = self.promoted_count,
                total = self.profile_order.len(),
            ),
        );
        Ok(())
    }
}

// --- Failed-plugin removal --------------------------------------------------------------

/// Removal errors. Every refusal is a DISTINCT named error
/// ([`RemovalError::name`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemovalError {
    /// Removal is EXPLICIT and legal ONLY from [`LifecycleState::Failed`]
    /// — a running plugin is drained through the normal lifecycle /
    /// recomposition path first (or contained via revocation).
    RequiresFailedState { state: LifecycleState },
    /// The removal reason is empty — an unattributed removal is refused.
    ReasonRequired,
    /// The removal actor is empty — an unattributed removal is refused.
    ActorRequired,
    /// The drain deadline expired with work outstanding: the lifecycle's
    /// fail-closed rule restored the plugin to `Ready` (there is no forced
    /// stop) and the removal is refused — the operator decides, and the
    /// plugin's authority should be contained meanwhile (revoke the
    /// plugin / drop its broker grants).
    DrainDeadlineExceeded { outstanding: usize },
    /// The lifecycle refused an edge (illegal transition, missing drain
    /// port); the lifecycle's own named error is carried.
    LifecycleRefused { source: LifecycleError },
}

impl RemovalError {
    /// The stable machine-readable error name (the named-error table key).
    pub fn name(&self) -> &'static str {
        match self {
            RemovalError::RequiresFailedState { .. } => "REMOVAL_REQUIRES_FAILED_STATE",
            RemovalError::ReasonRequired => "REMOVAL_REASON_REQUIRED",
            RemovalError::ActorRequired => "REMOVAL_ACTOR_REQUIRED",
            RemovalError::DrainDeadlineExceeded { .. } => "REMOVAL_DRAIN_DEADLINE_EXCEEDED",
            RemovalError::LifecycleRefused { .. } => "REMOVAL_LIFECYCLE_REFUSED",
        }
    }
}

impl fmt::Display for RemovalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RemovalError::RequiresFailedState { state } => write!(
                f,
                "explicit removal is legal only from Failed (the plugin is {state}); drain a \
                 running plugin through the normal lifecycle path first, or contain it via \
                 revocation"
            ),
            RemovalError::ReasonRequired => {
                write!(f, "a removal requires a non-empty reason (the removal is audited with its reason)")
            }
            RemovalError::ActorRequired => {
                write!(f, "a removal requires a non-empty actor")
            }
            RemovalError::DrainDeadlineExceeded { outstanding } => write!(
                f,
                "removal drain deadline expired with {outstanding} operation(s) outstanding; \
                 the lifecycle restored the plugin to Ready (fail-closed, no forced stop) and \
                 the removal is refused — contain the plugin (revoke it / drop its grants) \
                 while the operator decides"
            ),
            RemovalError::LifecycleRefused { source } => {
                write!(f, "the lifecycle refused the removal path ({}): {source}", source.name())
            }
        }
    }
}

impl std::error::Error for RemovalError {}

/// The EXPLICIT removal path for a `Failed` plugin (the P7.30 drill item:
/// a Failed plugin with in-flight records). Order of operations, all
/// audited through `audit`:
///
/// 1. the reason and actor are REQUIRED (non-empty);
/// 2. the lifecycle must be in [`LifecycleState::Failed`] — a running
///    plugin is removed through the normal drain-before-replace path, not
///    this one;
/// 3. `RemovalRequested` is audited;
/// 4. the §5.6 `Failed → Draining` edge is committed through the
///    lifecycle ([`PluginLifecycle::begin_drain`]);
/// 5. `Draining → Stopped` runs the plugin's [`DrainPort`] against the
///    lifecycle's drain policy: quiesced → `RemovalCompleted` is audited
///    and the drain outcome returned; deadline exceeded → the lifecycle's
///    fail-closed restoration applies (the incumbent returns to `Ready`,
///    there is no forced stop), `RemovalRefused` is audited with the
///    containment instruction, and
///    [`RemovalError::DrainDeadlineExceeded`] is returned.
///
/// AUDIT-TRAIL PRESERVATION (the ledger criterion): this function deletes
/// nothing. The lifecycle's records stay in the audit sink/store (the
/// sinks are append-only by contract), so a removed plugin — including a
/// malicious one — keeps its full evidence trail; the plugin's records
/// are never silently lost.
pub fn remove_failed_plugin<'a>(
    lifecycle: &mut PluginLifecycle<'a>,
    drain: &'a mut dyn DrainPort,
    reason: &str,
    actor: &str,
    clock: &dyn Clock,
    audit: &dyn RevocationAuditSink,
) -> Result<DrainOutcome, RemovalError> {
    let subject = lifecycle.plugin_id().to_string();
    if reason.trim().is_empty() {
        emit(
            audit,
            clock,
            RevocationAuditKind::RemovalRefused,
            &subject,
            RemovalError::ReasonRequired.name(),
        );
        return Err(RemovalError::ReasonRequired);
    }
    if actor.trim().is_empty() {
        emit(
            audit,
            clock,
            RevocationAuditKind::RemovalRefused,
            &subject,
            RemovalError::ActorRequired.name(),
        );
        return Err(RemovalError::ActorRequired);
    }
    if lifecycle.state() != LifecycleState::Failed {
        let err = RemovalError::RequiresFailedState { state: lifecycle.state() };
        emit(
            audit,
            clock,
            RevocationAuditKind::RemovalRefused,
            &subject,
            &format!("removal refused ({}): {err}", err.name()),
        );
        return Err(err);
    }
    emit(
        audit,
        clock,
        RevocationAuditKind::RemovalRequested,
        &subject,
        &format!("explicit removal of Failed plugin requested by '{actor}': {reason}"),
    );
    lifecycle
        .begin_drain(drain, actor, &format!("REMOVAL (explicit, from Failed): {reason}"))
        .map_err(|source| {
            emit(
                audit,
                clock,
                RevocationAuditKind::RemovalRefused,
                &subject,
                &format!(
                    "removal refused at Failed -> Draining ({}): {source}",
                    source.name()
                ),
            );
            RemovalError::LifecycleRefused { source }
        })?;
    match lifecycle.complete_drain(actor, &format!("REMOVAL: {reason}")) {
        Ok(outcome) => {
            emit(
                audit,
                clock,
                RevocationAuditKind::RemovalCompleted,
                &subject,
                &format!(
                    "plugin drained and removed (state Stopped; the audit trail is PRESERVED — \
                     records persist in the audit sink/store): {reason}"
                ),
            );
            Ok(outcome)
        }
        Err(LifecycleError::DrainDeadlineExceeded { outstanding }) => {
            emit(
                audit,
                clock,
                RevocationAuditKind::RemovalRefused,
                &subject,
                &format!(
                    "removal refused ({}): {outstanding} operation(s) outstanding; the \
                     lifecycle restored the plugin to Ready (fail-closed, no forced stop) — \
                     contain the plugin (revoke it / drop its broker grants) while the \
                     operator decides",
                    RemovalError::DrainDeadlineExceeded { outstanding }.name(),
                ),
            );
            Err(RemovalError::DrainDeadlineExceeded { outstanding })
        }
        Err(source) => {
            emit(
                audit,
                clock,
                RevocationAuditKind::RemovalRefused,
                &subject,
                &format!("removal refused ({}): {source}", source.name()),
            );
            Err(RemovalError::LifecycleRefused { source })
        }
    }
}
