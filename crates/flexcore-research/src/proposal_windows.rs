// P8.34 — Proposal validity windows and stale-proposal rejection.
//         Ledger `ledger/phase-08-advanced-coordination/ledger_phase_8.md`
//         §P8.34; report trace §14.3 (valid-from/valid-until on every
//         command), §17.5 invariant 5 (no stale or expired command), §13.4;
//         decision record:
//         `ledger/phase-08-advanced-coordination/34_proposal_windows.md`.
//
// WHAT THIS IS: the completion of the P8.05 validity-window seam. The
// harness (`validation.rs`) already enforces the window and snapshot
// freshness at VALIDATION time (stage 4 `validity_window` against the
// injected clock, stage 5 `snapshot_freshness`; the P8.34 seam recorded in
// its header). This module completes what that seam RESERVED, additively:
//
// 1. THE RECORDED WINDOW RULE (stated plainly — the exactly-at-expiry
//    rule): a proposal is current iff `valid_from_ms <= now <
//    valid_until_ms` on the INJECTED clock. Exactly-at-open
//    (`now == valid_from_ms`) is ACCEPTED (the window opens inclusive);
//    exactly-at-expiry (`now == valid_until_ms`) is REJECTED (the window
//    closes exclusive). This is the kernel `PluginRequestEnvelope` window
//    semantics (§14.3 field 15 by analogy) and the harness stage-4 rule —
//    one rule everywhere, pinned by known-answer tests on both sides.
//
// 2. THE WINDOW IS GRANTED BY THE HOST FROM THE INJECTED CLOCK, NEVER
//    READ FROM THE PROPOSAL'S OWN CLOCK, AND ITS DURATION IS CAPPED
//    (the ledger's recorded risk: "a window that is set from the
//    proposal's own clock with no bound"). [`grant_validity_window`]
//    takes the clock port ([`crate::validation::HarnessClock`] — the
//    crate's ONE clock vocabulary) and refuses a non-positive or
//    over-cap duration with named errors; `SystemTime::now()` appears
//    nowhere in this module or its tests.
//
// 3. GATE-AT-TIME-OF-USE. A proposal is checked at proposal time, at
//    validation time (harness stages 4–5), AND at the point any
//    downstream consumer touches it ([`gate_at_time_of_use`]) — the
//    ledger's other recorded risk: "validity checked at proposal time but
//    not at gate time". The re-check returns the SAME named refusals the
//    harness returns ([`crate::validation::Refusal`]: PROPOSAL_NOT_YET_VALID /
//    PROPOSAL_EXPIRED / SNAPSHOT_SUPERSEDED / SNAPSHOT_UNKNOWN) — a stale
//    proposal is rejected by the same gate vocabulary as an infeasible
//    one, BEFORE any downstream consumption (never passed to feasibility
//    or any consumer as if fresh).
//
// 4. RESTART-SURVIVING SNAPSHOT SUPERSESSION (the kernel revocation
//    checkpoint pattern, mirrored by citation — this crate does not
//    depend on flexcore-kernel): the snapshot-supersession register is
//    APPEND-ONLY (records are added, never removed or rewritten; there is
//    no un-supersede) and has a FILE-BACKED implementation
//    ([`FileSnapshotSupersessionRegister`]) in the `buffer_file.rs`
//    framing discipline of `crates/edge-agent/src/buffer_file.rs` and the
//    kernel `FileRevocationRegister` (P7.38), REIMPLEMENTED here because
//    the kernel is not a dependency:
//
//    ```text
//    header:  b"FLEXCORE-RESEARCH-SNAP-REG" + format version byte (0x01)
//    record:  [u32 LE payload_len][u32 LE crc32(payload)][payload]
//    payload: [u16 LE old_len][old snapshot id]
//             [u16 LE new_len][new snapshot id]
//             [u16 LE actor_len][actor]
//             [i64 LE superseded_at_millis]
//    ```
//
//    Recovery is truncate-and-audit, never silent (the FIRST malformed
//    frame ends the replay, the file is truncated at the last good
//    record, the loss is COUNTED via
//    [`FileSnapshotSupersessionRegister::corrupt_tail_truncated`]); a
//    wrong magic/version is a hard open error; the append is flushed and
//    `sync_data`d BEFORE the in-memory state moves (durability before
//    state). A restart therefore replays exactly the supersessions that
//    were durable — a snapshot superseded before the restart stays
//    superseded after it, so a stale proposal cannot look fresh across a
//    restart.
//
// 5. REPLAY/SUPERSEDE HANDLING. Supersessions form the freshness
//    computation ([`SnapshotFreshnessSource`](crate::validation::
//    SnapshotFreshnessSource) implementation
//    [`CheckpointedSnapshotSource`]): a snapshot with a supersession
//    record is `Superseded { by_snapshot_id }` (the replacing id travels,
//    harness stage-5 shape); a KNOWN snapshot without one is `Current`;
//    an UNKNOWN id is `Unknown` (unknown is never fresh — the P4.11
//    rule the harness mirrors). Supersession chains are supported (A→B
//    then B→C): freshness is computed from the DIRECT record, so a
//    proposal against A is stale with `by = B` — the consumer re-checks
//    and sees the chain through repeated gate calls.
//
// 6. MISSING-WINDOW REFUSAL. The proposal type carries the window
//    structurally (non-optional `validity`), so a missing window can only
//    arrive at the WIRE — [`window_from_wire_parts`] is the wire-contract
//    extraction point: either field absent is the named refusal
//    [`WindowError::WindowMissing`], never a defaulted window. The wire
//    contract itself (`python/workers/contracts/worker_api.proto`) does
//    not yet carry the window fields — that is a RECORDED Phase 7
//    follow-up (see `34_proposal_windows.md` §5), never fixed by
//    inventing a parallel contract (ENTRY_NOTE boundary 3).
//
// HARNESS INTEGRATION (additive; the versioned-change discipline): this
// module changes NOTHING in `validation.rs` — not `STAGE_ORDER`, not
// `HARNESS_VERSION`. The harness stages 4–5 run unchanged; what lands
// here is (a) the host-side window grantor, (b) the freshness SOURCE
// implementation the composition root injects into the harness, and (c)
// the gate-at-time-of-use re-check. Had the stage list needed to change,
// that would be a NEW `HARNESS_VERSION` (the P8.06 frozen-version
// discipline applied to the harness) — it did not, and
// `34_proposal_windows.md` records that determination explicitly.
//
// Determinism: register iteration is `BTreeMap`-ordered; file framing is
// fixed-layout binary; canonical forms are fixed-format strings; no
// `unwrap`/`expect` on untrusted input anywhere in this module.
//
// Control status: research proposals are DATA (ADR-010). Nothing here
// authorizes, issues, approves, or dispatches anything; a current window
// is a FRESHNESS fact, never an authority. No safety, certification, or
// field-authority claim is made.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::validation::{
    HarnessClock, Refusal, ResearchProposal, ResearchValidityWindow, SnapshotFreshness,
    SnapshotFreshnessSource,
};

// --- Honesty statements ----------------------------------------------------------

/// The recorded window rule (pinned by tests so what IS and IS NOT claimed
/// stays reviewable).
pub const WINDOW_RULE: &str = "a research proposal is current iff valid_from_ms <= now < \
     valid_until_ms on the INJECTED clock. Exactly-at-open (now == valid_from_ms) is \
     ACCEPTED (the window opens inclusive); exactly-at-expiry (now == valid_until_ms) is \
     REJECTED (the window closes exclusive). The window is granted by the HOST from the \
     injected clock — never read from the proposal's own clock — its duration is capped, \
     and freshness is re-checked at the point of use, not only at proposal time.";

/// The kernel checkpoint alignment seam (recorded — do not drift
/// silently): the file register mirrors the restart-surviving checkpoint
/// pattern of `crates/flexcore-kernel/src/revocation.rs::
/// FileRevocationRegister` (append-only framing, CRC-32 integrity,
/// truncate-and-audit recovery, durability before state) BY CITATION;
/// kernel unchanged; no flexcore-kernel dependency.
pub const KERNEL_CHECKPOINT_ALIGNMENT_SEAM: &str =
    "mirror of crates/flexcore-kernel/src/revocation.rs::FileRevocationRegister \
     checkpoint discipline (append-only frames, crc32, truncate-and-audit, durability \
     before state); kernel unchanged; no flexcore-kernel dependency";

/// The RECORDED DEFAULT maximum window duration: one hour, in millis. A
/// research proposal feeds a rolling decision cycle; a longer window
/// would let a proposal computed against old conditions look current for
/// a whole operating day. This is a RECORDED DEFAULT (RECOMMENDATION) —
/// re-litigable only through a new recorded decision, never by a caller
/// passing a bigger number (the cap is enforced at grant time).
pub const MAX_VALIDITY_WINDOW_MS: i64 = 60 * 60 * 1_000;

// --- Errors --------------------------------------------------------------------------

/// Every window/supersession refusal is a DISTINCT named error
/// ([`WindowError::name`]) — never a warning, never a silent acceptance,
/// never a defaulted window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowError {
    /// A wire contract delivered no window (either field absent) — the
    /// missing-window refusal. A defaulted window is never constructed.
    WindowMissing { field: &'static str },
    /// The window is empty or inverted (`valid_from >= valid_until`).
    WindowInverted { valid_from_ms: i64, valid_until_ms: i64 },
    /// The requested window duration is not positive.
    WindowDurationNonPositive { duration_ms: i64 },
    /// The requested window duration exceeds the recorded cap
    /// ([`MAX_VALIDITY_WINDOW_MS`]) — the no-unbounded-window refusal.
    WindowExceedsCap { duration_ms: i64, cap_ms: i64 },
    /// A supersession record is structurally invalid (empty snapshot id,
    /// self-supersession).
    SupersessionInvalid { reason: String },
    /// The supersession actor is empty — an unattributed supersession is
    /// refused, never recorded.
    SupersessionActorRequired,
    /// This snapshot already has a supersession record (the register is
    /// append-only; exactly one record per superseded id).
    SnapshotAlreadySuperseded { snapshot_id: String },
    /// The register store failed (I/O, poisoned lock).
    RegisterUnavailable { reason: String },
    /// The register file exists but is not a FlexCore research snapshot
    /// register (wrong magic/version) — refused, never guessed at.
    RegisterFileForeign { reason: String },
}

impl WindowError {
    /// The stable machine-readable error name (the named-error table key;
    /// pinned by tests).
    pub fn name(&self) -> &'static str {
        match self {
            WindowError::WindowMissing { .. } => "PROPOSAL_WINDOW_MISSING",
            WindowError::WindowInverted { .. } => "PROPOSAL_WINDOW_INVERTED",
            WindowError::WindowDurationNonPositive { .. } => "PROPOSAL_WINDOW_DURATION_NON_POSITIVE",
            WindowError::WindowExceedsCap { .. } => "PROPOSAL_WINDOW_EXCEEDS_CAP",
            WindowError::SupersessionInvalid { .. } => "SNAPSHOT_SUPERSESSION_INVALID",
            WindowError::SupersessionActorRequired => "SNAPSHOT_SUPERSESSION_ACTOR_REQUIRED",
            WindowError::SnapshotAlreadySuperseded { .. } => "SNAPSHOT_ALREADY_SUPERSEDED",
            WindowError::RegisterUnavailable { .. } => "WINDOW_REGISTER_UNAVAILABLE",
            WindowError::RegisterFileForeign { .. } => "WINDOW_REGISTER_FILE_FOREIGN",
        }
    }
}

impl fmt::Display for WindowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WindowError::WindowMissing { field } => write!(
                f,
                "the proposal carries no validity window ('{field}' is absent on the wire); \
                 a missing window is refused, never defaulted (ledger P8.34)"
            ),
            WindowError::WindowInverted { valid_from_ms, valid_until_ms } => write!(
                f,
                "the validity window is empty or inverted (valid_from {valid_from_ms} >= \
                 valid_until {valid_until_ms}); refused"
            ),
            WindowError::WindowDurationNonPositive { duration_ms } => write!(
                f,
                "the requested window duration {duration_ms} ms is not positive; a window \
                 must open strictly before it closes"
            ),
            WindowError::WindowExceedsCap { duration_ms, cap_ms } => write!(
                f,
                "the requested window duration {duration_ms} ms exceeds the recorded cap \
                 {cap_ms} ms — a window with no bound is how a stale proposal looks \
                 current (ledger P8.34 risk); refused"
            ),
            WindowError::SupersessionInvalid { reason } => {
                write!(f, "the snapshot supersession is invalid: {reason}")
            }
            WindowError::SupersessionActorRequired => write!(
                f,
                "a snapshot supersession requires a non-empty actor (unattributed \
                 supersessions are never recorded)"
            ),
            WindowError::SnapshotAlreadySuperseded { snapshot_id } => write!(
                f,
                "snapshot '{snapshot_id}' is already superseded (the register is \
                 append-only and keeps exactly one record per superseded id)"
            ),
            WindowError::RegisterUnavailable { reason } => {
                write!(f, "the snapshot-supersession register is unavailable: {reason}")
            }
            WindowError::RegisterFileForeign { reason } => write!(
                f,
                "the register file is not a FlexCore research snapshot register (refused, \
                 never guessed): {reason}"
            ),
        }
    }
}

impl std::error::Error for WindowError {}

// --- The host-side window grantor -----------------------------------------------------

/// Grants a validity window from the INJECTED clock: `[now, now +
/// duration)`. The clock is injected, the duration is caller-declared but
/// CAPPED ([`MAX_VALIDITY_WINDOW_MS`]) — there is no path to an unbounded
/// or self-clocked window. Exactly-at-open is inside (inclusive open);
/// exactly-at-expiry is outside (exclusive close) — the recorded rule in
/// [`WINDOW_RULE`].
pub fn grant_validity_window(
    clock: &dyn HarnessClock,
    duration_ms: i64,
) -> Result<ResearchValidityWindow, WindowError> {
    if duration_ms <= 0 {
        return Err(WindowError::WindowDurationNonPositive { duration_ms });
    }
    if duration_ms > MAX_VALIDITY_WINDOW_MS {
        return Err(WindowError::WindowExceedsCap {
            duration_ms,
            cap_ms: MAX_VALIDITY_WINDOW_MS,
        });
    }
    let now = clock.now_millis();
    Ok(ResearchValidityWindow {
        valid_from_ms: now,
        valid_until_ms: now + duration_ms,
    })
}

/// The wire-contract extraction point: builds the window from the
/// optionally-present wire fields. EITHER field absent is
/// [`WindowError::WindowMissing`] (named with the absent field) — a
/// missing window is refused, never defaulted; an inverted window is
/// [`WindowError::WindowInverted`]. The construction rules are the same
/// ones `ResearchProposal::new` enforces (the harness stage-1 shape check
/// re-verifies).
pub fn window_from_wire_parts(
    valid_from_ms: Option<i64>,
    valid_until_ms: Option<i64>,
) -> Result<ResearchValidityWindow, WindowError> {
    let valid_from_ms =
        valid_from_ms.ok_or(WindowError::WindowMissing { field: "valid_from_ms" })?;
    let valid_until_ms =
        valid_until_ms.ok_or(WindowError::WindowMissing { field: "valid_until_ms" })?;
    if valid_from_ms >= valid_until_ms {
        return Err(WindowError::WindowInverted { valid_from_ms, valid_until_ms });
    }
    Ok(ResearchValidityWindow { valid_from_ms, valid_until_ms })
}

// --- The snapshot-supersession record ---------------------------------------------------

/// One append-only supersession record: snapshot `superseded_snapshot_id`
/// was replaced by `by_snapshot_id` (the ledger: "a proposal against a
/// superseded snapshot is rejected"). The actor and timestamp are
/// REQUIRED (an unattributed supersession is refused, never recorded);
/// the timestamp comes from the injected clock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotSupersessionRecord {
    /// The snapshot that was replaced.
    pub superseded_snapshot_id: String,
    /// The snapshot that replaced it.
    pub by_snapshot_id: String,
    /// Who recorded the supersession (REQUIRED non-empty).
    pub actor: String,
    /// When (injected-clock millis).
    pub superseded_at_millis: i64,
}

impl SnapshotSupersessionRecord {
    /// The register's canonical key: the SUPERSEDED id (one record per
    /// superseded snapshot; BTreeMap key, deterministic).
    pub fn record_key(&self) -> &str {
        &self.superseded_snapshot_id
    }
}

// --- The store port and the memory register ----------------------------------------------

/// The append-only snapshot-supersession store port (crate-local; the
/// kernel `RevocationStore` pattern mirrored). APPEND-ONLY by contract:
/// there is no removal, update, or override operation on this trait.
pub trait SnapshotSupersessionStore {
    /// Appends one supersession. A duplicate (superseded id already
    /// recorded) is [`WindowError::SnapshotAlreadySuperseded`] — never a
    /// silent rewrite.
    fn append(&self, record: &SnapshotSupersessionRecord) -> Result<(), WindowError>;
    /// All records, in append order (the audit/replay view).
    fn load_all(&self) -> Result<Vec<SnapshotSupersessionRecord>, WindowError>;
}

/// The in-memory append-only supersession register (deterministic
/// collector for tests and in-process compositions). Keyed by the
/// superseded id in a `BTreeMap` (deterministic iteration); the
/// append-order vector preserves the audit order.
#[derive(Debug, Default)]
pub struct MemorySnapshotSupersessionRegister {
    by_superseded_id: Mutex<BTreeMap<String, SnapshotSupersessionRecord>>,
    known_ids: Mutex<BTreeSet<String>>,
    append_order: Mutex<Vec<SnapshotSupersessionRecord>>,
}

impl MemorySnapshotSupersessionRegister {
    pub fn new() -> MemorySnapshotSupersessionRegister {
        MemorySnapshotSupersessionRegister {
            by_superseded_id: Mutex::new(BTreeMap::new()),
            known_ids: Mutex::new(BTreeSet::new()),
            append_order: Mutex::new(Vec::new()),
        }
    }

    /// Declares a snapshot KNOWN (the host registers each snapshot it
    /// publishes). A known-and-not-superseded snapshot is `Current`;
    /// an id never declared is `Unknown` (unknown is never fresh).
    pub fn register_current(&self, snapshot_id: &str) -> Result<(), WindowError> {
        if snapshot_id.trim().is_empty() {
            return Err(WindowError::SupersessionInvalid {
                reason: "a snapshot id must be non-empty".to_string(),
            });
        }
        self.known_ids
            .lock()
            .map_err(|_| WindowError::RegisterUnavailable {
                reason: "the known-id lock is poisoned".to_string(),
            })?
            .insert(snapshot_id.to_string());
        Ok(())
    }

    /// Appends one supersession after structural validation (fail-closed:
    /// empty ids, self-supersession, empty actor, and duplicates are all
    /// refused, never normalized). The target snapshot becomes known.
    pub fn supersede(
        &self,
        superseded_snapshot_id: &str,
        by_snapshot_id: &str,
        actor: &str,
        clock: &dyn HarnessClock,
    ) -> Result<(), WindowError> {
        if superseded_snapshot_id.trim().is_empty() || by_snapshot_id.trim().is_empty() {
            return Err(WindowError::SupersessionInvalid {
                reason: "both the superseded and the replacing snapshot ids must be \
                         non-empty"
                    .to_string(),
            });
        }
        if superseded_snapshot_id == by_snapshot_id {
            return Err(WindowError::SupersessionInvalid {
                reason: "a snapshot cannot supersede itself".to_string(),
            });
        }
        if actor.trim().is_empty() {
            return Err(WindowError::SupersessionActorRequired);
        }
        let record = SnapshotSupersessionRecord {
            superseded_snapshot_id: superseded_snapshot_id.to_string(),
            by_snapshot_id: by_snapshot_id.to_string(),
            actor: actor.to_string(),
            superseded_at_millis: clock.now_millis(),
        };
        self.append(&record)?;
        Ok(())
    }

    /// The records in append order (test/composition inspection).
    pub fn records(&self) -> Vec<SnapshotSupersessionRecord> {
        match self.append_order.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => Vec::new(),
        }
    }
}

impl SnapshotSupersessionStore for MemorySnapshotSupersessionRegister {
    fn append(&self, record: &SnapshotSupersessionRecord) -> Result<(), WindowError> {
        let key = record.record_key().to_string();
        {
            let mut by_id = self
                .by_superseded_id
                .lock()
                .map_err(|_| WindowError::RegisterUnavailable {
                    reason: "the supersession map lock is poisoned".to_string(),
                })?;
            if by_id.contains_key(&key) {
                return Err(WindowError::SnapshotAlreadySuperseded { snapshot_id: key });
            }
            by_id.insert(key, record.clone());
        }
        {
            let mut known = self
                .known_ids
                .lock()
                .map_err(|_| WindowError::RegisterUnavailable {
                    reason: "the known-id lock is poisoned".to_string(),
                })?;
            known.insert(record.superseded_snapshot_id.clone());
            known.insert(record.by_snapshot_id.clone());
        }
        let mut order = self
            .append_order
            .lock()
            .map_err(|_| WindowError::RegisterUnavailable {
                reason: "the append-order lock is poisoned".to_string(),
            })?;
        order.push(record.clone());
        Ok(())
    }

    fn load_all(&self) -> Result<Vec<SnapshotSupersessionRecord>, WindowError> {
        let order = self
            .append_order
            .lock()
            .map_err(|_| WindowError::RegisterUnavailable {
                reason: "the append-order lock is poisoned".to_string(),
            })?;
        Ok(order.clone())
    }
}

/// The freshness computation over a supersession register — the
/// [`SnapshotFreshnessSource`](crate::validation::SnapshotFreshnessSource)
/// the composition root injects into the P8.05 harness (stage 5) and into
/// [`gate_at_time_of_use`]. Semantics (the harness shape, unchanged):
///
/// - a snapshot WITH a supersession record → `Superseded { by_snapshot_id }`
///   (the replacing id travels on the refusal);
/// - a KNOWN snapshot without one → `Current`;
/// - an id never registered → `Unknown` (unknown is never fresh — the
///   P4.11 rule; fail-closed).
#[derive(Debug, Default)]
pub struct CheckpointedSnapshotSource<S: SnapshotSupersessionStore> {
    register: S,
}

impl<S: SnapshotSupersessionStore> CheckpointedSnapshotSource<S> {
    pub fn new(register: S) -> CheckpointedSnapshotSource<S> {
        CheckpointedSnapshotSource { register }
    }

    /// The underlying store (the composition root uses the register's own
    /// API for registration and supersession).
    pub fn register(&self) -> &S {
        &self.register
    }
}

impl<S: SnapshotSupersessionStore> CheckpointedSnapshotSource<S> {
    /// The freshness classification for `snapshot_id` (the shared logic of
    /// the trait impl; needs the known-id view, which the memory register
    /// provides through its records: an id is KNOWN iff it appears as the
    /// superseded OR the replacing id of some record, or was declared via
    /// `register_current`).
    fn classify(&self, snapshot_id: &str, known: &BTreeSet<String>) -> SnapshotFreshness {
        // Load once; the store is append-only so a linear scan is exact.
        let records = match self.register.load_all() {
            Ok(records) => records,
            // Fail-closed: a freshness source that cannot read its
            // register never reports "Current" (the P6.06 unestablishable-
            // status rule).
            Err(_) => return SnapshotFreshness::Unknown,
        };
        for record in &records {
            if record.superseded_snapshot_id == snapshot_id {
                return SnapshotFreshness::Superseded {
                    by_snapshot_id: record.by_snapshot_id.clone(),
                };
            }
        }
        if known.contains(snapshot_id) {
            SnapshotFreshness::Current
        } else {
            SnapshotFreshness::Unknown
        }
    }
}

impl SnapshotFreshnessSource for CheckpointedSnapshotSource<MemorySnapshotSupersessionRegister> {
    fn freshness(&self, snapshot_id: &str) -> SnapshotFreshness {
        let known = match self.register.known_ids.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => BTreeSet::new(),
        };
        self.classify(snapshot_id, &known)
    }
}

// --- The file-backed register (restart-surviving checkpoint) ------------------------------

/// Hard sanity cap on one register record payload (rejects absurd lengths
/// from a corrupt header instead of attempting an absurd allocation — the
/// buffer_file.rs discipline).
const MAX_REGISTER_RECORD_BYTES: u32 = 16 * 1024;

/// The file-backed [`SnapshotSupersessionStore`] — the RESTART-SURVIVING
/// checkpoint, in the `buffer_file.rs` framing discipline (REIMPLEMENTED —
/// this crate depends on no flexcore crate; see the module header for the
/// exact format):
///
/// - **Recovery (truncate-and-audit, never silent).** On open the file is
///   replayed record by record; the FIRST malformed frame ends the replay
///   and the file is TRUNCATED at the last good record, with the loss
///   COUNTED ([`FileSnapshotSupersessionRegister::corrupt_tail_truncated`]).
///   A wrong magic/version is a hard open error.
/// - **Append-only.** Records are only ever added; the file grows.
/// - **Durability before state.** `append` flushes and `sync_data`s
///   BEFORE the in-memory maps move.
pub struct FileSnapshotSupersessionRegister {
    path: PathBuf,
    file: Mutex<std::fs::File>,
    by_superseded_id: Mutex<BTreeMap<String, SnapshotSupersessionRecord>>,
    known_ids: Mutex<BTreeSet<String>>,
    append_order: Mutex<Vec<SnapshotSupersessionRecord>>,
    corrupt_tail_truncated: std::sync::atomic::AtomicU64,
    records_written: std::sync::atomic::AtomicU64,
}

impl FileSnapshotSupersessionRegister {
    /// The journal magic header (26-byte label) + format version.
    fn magic() -> Vec<u8> {
        let mut magic = b"FLEXCORE-RESEARCH-SNAP-REG".to_vec();
        magic.push(0x01);
        magic
    }

    /// Opens (or creates) the register file and replays any existing
    /// records. A corrupt tail is truncated and counted, never silent; a
    /// wrong magic/version is a hard error.
    pub fn open(path: &Path) -> Result<Self, WindowError> {
        if path.as_os_str().is_empty() {
            return Err(WindowError::RegisterUnavailable {
                reason: "the register path is empty".to_string(),
            });
        }
        let magic = Self::magic();
        let (file, replayed) = match std::fs::OpenOptions::new().read(true).write(true).open(path)
        {
            Ok(mut existing) => {
                let mut contents = Vec::new();
                existing.read_to_end(&mut contents).map_err(|e| {
                    WindowError::RegisterUnavailable {
                        reason: format!("reading the existing register failed: {e}"),
                    }
                })?;
                if contents.is_empty() {
                    // A zero-length file (touch/new) is initialized, not refused.
                    (existing, Vec::new())
                } else {
                    if contents.len() < magic.len() || contents[..magic.len()] != magic[..] {
                        return Err(WindowError::RegisterFileForeign {
                            reason: format!(
                                "the file at {} does not carry the FlexCore research \
                                 snapshot register magic",
                                path.display()
                            ),
                        });
                    }
                    let mut records = Vec::new();
                    let mut offset = magic.len();
                    let mut corrupt_tail_truncated: u64 = 0;
                    while offset + 8 <= contents.len() {
                        let len = match contents
                            .get(offset..offset + 4)
                            .and_then(|b| <[u8; 4]>::try_from(b).ok())
                            .map(u32::from_le_bytes)
                        {
                            Some(len) => len,
                            None => {
                                corrupt_tail_truncated += 1;
                                break;
                            }
                        };
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
                            .map(u32::from_le_bytes)
                        {
                            Some(crc) => crc,
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
                        if crc32(payload) != crc_bytes {
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
                        // COUNTED, never silent.
                        existing.set_len(offset as u64).map_err(|e| {
                            WindowError::RegisterUnavailable {
                                reason: format!(
                                    "truncating the corrupt register tail failed: {e}"
                                ),
                            }
                        })?;
                        existing.sync_all().map_err(|e| {
                            WindowError::RegisterUnavailable {
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
                            WindowError::RegisterUnavailable {
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
                    .map_err(|e| WindowError::RegisterUnavailable {
                        reason: format!("creating the register file failed: {e}"),
                    })?;
                created
                    .write_all(&magic)
                    .and_then(|_| created.flush())
                    .and_then(|_| created.sync_all())
                    .map_err(|e| WindowError::RegisterUnavailable {
                        reason: format!("writing the register header failed: {e}"),
                    })?;
                (created, Vec::new())
            }
        };
        let mut by_superseded_id = BTreeMap::new();
        let mut known_ids = BTreeSet::new();
        for record in &replayed {
            by_superseded_id.insert(record.record_key().to_string(), record.clone());
            known_ids.insert(record.superseded_snapshot_id.clone());
            known_ids.insert(record.by_snapshot_id.clone());
        }
        Ok(Self {
            path: path.to_path_buf(),
            file: Mutex::new(file),
            by_superseded_id: Mutex::new(by_superseded_id),
            known_ids: Mutex::new(known_ids),
            append_order: Mutex::new(replayed),
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

    /// Declares a snapshot KNOWN (host-side; survives only in the current
    /// process — the supersession records are what survive restarts; a
    /// re-registration after a restart is the composition root's
    /// obligation and is idempotent).
    pub fn register_current(&self, snapshot_id: &str) -> Result<(), WindowError> {
        if snapshot_id.trim().is_empty() {
            return Err(WindowError::SupersessionInvalid {
                reason: "a snapshot id must be non-empty".to_string(),
            });
        }
        self.known_ids
            .lock()
            .map_err(|_| WindowError::RegisterUnavailable {
                reason: "the known-id lock is poisoned".to_string(),
            })?
            .insert(snapshot_id.to_string());
        Ok(())
    }

    /// Appends one supersession with structural validation (the same
    /// fail-closed rules as the memory register).
    pub fn supersede(
        &self,
        superseded_snapshot_id: &str,
        by_snapshot_id: &str,
        actor: &str,
        clock: &dyn HarnessClock,
    ) -> Result<(), WindowError> {
        if superseded_snapshot_id.trim().is_empty() || by_snapshot_id.trim().is_empty() {
            return Err(WindowError::SupersessionInvalid {
                reason: "both the superseded and the replacing snapshot ids must be \
                         non-empty"
                    .to_string(),
            });
        }
        if superseded_snapshot_id == by_snapshot_id {
            return Err(WindowError::SupersessionInvalid {
                reason: "a snapshot cannot supersede itself".to_string(),
            });
        }
        if actor.trim().is_empty() {
            return Err(WindowError::SupersessionActorRequired);
        }
        let record = SnapshotSupersessionRecord {
            superseded_snapshot_id: superseded_snapshot_id.to_string(),
            by_snapshot_id: by_snapshot_id.to_string(),
            actor: actor.to_string(),
            superseded_at_millis: clock.now_millis(),
        };
        self.append(&record)
    }

    fn encode_payload(record: &SnapshotSupersessionRecord) -> Vec<u8> {
        let mut payload = Vec::new();
        for field in [
            record.superseded_snapshot_id.as_str(),
            record.by_snapshot_id.as_str(),
            record.actor.as_str(),
        ] {
            payload.extend_from_slice(&(field.len() as u16).to_le_bytes());
            payload.extend_from_slice(field.as_bytes());
        }
        payload.extend_from_slice(&record.superseded_at_millis.to_le_bytes());
        payload
    }
}

fn decode_register_payload(payload: &[u8]) -> Option<SnapshotSupersessionRecord> {
    let mut offset = 0usize;
    let mut fields: Vec<String> = Vec::with_capacity(3);
    for _ in 0..3 {
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
    let superseded_at_millis = i64::from_le_bytes(ts_bytes);
    Some(SnapshotSupersessionRecord {
        superseded_snapshot_id: fields[0].clone(),
        by_snapshot_id: fields[1].clone(),
        actor: fields[2].clone(),
        superseded_at_millis,
    })
}

/// The CRC-32 (IEEE, reflected) used by the register records — the same
/// checksum family as the P7.36 sequence checkpoint and the P7.38
/// revocation register (an integrity checksum for corruption detection,
/// not a security primitive).
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

impl SnapshotSupersessionStore for FileSnapshotSupersessionRegister {
    fn append(&self, record: &SnapshotSupersessionRecord) -> Result<(), WindowError> {
        let key = record.record_key().to_string();
        {
            let by_id = self
                .by_superseded_id
                .lock()
                .map_err(|_| WindowError::RegisterUnavailable {
                    reason: "the supersession map lock is poisoned".to_string(),
                })?;
            if by_id.contains_key(&key) {
                return Err(WindowError::SnapshotAlreadySuperseded { snapshot_id: key });
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
            let mut file = self.file.lock().map_err(|_| WindowError::RegisterUnavailable {
                reason: "the register file lock is poisoned".to_string(),
            })?;
            file.seek(SeekFrom::End(0))
                .and_then(|_| file.write_all(&frame))
                .and_then(|_| file.flush())
                .and_then(|_| file.sync_data())
                .map_err(|e| WindowError::RegisterUnavailable {
                    reason: format!("appending the register record failed: {e}"),
                })?;
        }
        // Durability BEFORE state: the in-memory maps move only after the
        // record is on disk.
        {
            let mut by_id = self
                .by_superseded_id
                .lock()
                .map_err(|_| WindowError::RegisterUnavailable {
                    reason: "the supersession map lock is poisoned".to_string(),
                })?;
            by_id.insert(key, record.clone());
            drop(by_id);
            let mut known = self
                .known_ids
                .lock()
                .map_err(|_| WindowError::RegisterUnavailable {
                    reason: "the known-id lock is poisoned".to_string(),
                })?;
            known.insert(record.superseded_snapshot_id.clone());
            known.insert(record.by_snapshot_id.clone());
        }
        let mut order = self
            .append_order
            .lock()
            .map_err(|_| WindowError::RegisterUnavailable {
                reason: "the append-order lock is poisoned".to_string(),
            })?;
        order.push(record.clone());
        self.records_written
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    fn load_all(&self) -> Result<Vec<SnapshotSupersessionRecord>, WindowError> {
        let order = self
            .append_order
            .lock()
            .map_err(|_| WindowError::RegisterUnavailable {
                reason: "the append-order lock is poisoned".to_string(),
            })?;
        Ok(order.clone())
    }
}

impl SnapshotFreshnessSource for CheckpointedSnapshotSource<FileSnapshotSupersessionRegister> {
    fn freshness(&self, snapshot_id: &str) -> SnapshotFreshness {
        let known = match self.register.known_ids.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => BTreeSet::new(),
        };
        self.classify(snapshot_id, &known)
    }
}

// --- The gate-at-time-of-use check -----------------------------------------------------

/// The GATE-AT-TIME-OF-USE check (the ledger's step 2, at the consumption
/// point): re-runs the window and snapshot-freshness rule against the
/// injected clock BEFORE any downstream consumption — before feasibility
/// is consulted again, before a replay, before a superseding decision, or
/// before any consumer reads the proposal as if fresh. Returns the SAME
/// named refusals the harness returns (the
/// [`crate::validation::Refusal`] vocabulary), so a stale proposal is
/// rejected by the same gate class as an infeasible one:
///
/// - `now < valid_from` → [`Refusal::ProposalNotYetValid`];
/// - `now >= valid_until` → [`Refusal::ProposalExpired`] (exactly-at-expiry
///   is REJECTED — the recorded rule);
/// - superseded snapshot → [`Refusal::SnapshotSuperseded`] (the replacing
///   id travels);
/// - unknown snapshot → [`Refusal::SnapshotUnknown`] (unknown is never
///   fresh).
///
/// This is a RE-CHECK, never a substitute for the harness: the harness
/// (stages 4–5) remains the gate that produces a
/// [`crate::validation::ValidatedProposal`]; this function is the
/// consumption-point discipline the ledger's risk rows require. It is
/// pure and side-effect-free (auditing the consumption decision is the
/// composition root's obligation — this module takes no audit port so the
/// check can run anywhere, including inside a replay).
pub fn gate_at_time_of_use(
    proposal: &ResearchProposal,
    clock: &dyn HarnessClock,
    snapshots: &dyn SnapshotFreshnessSource,
) -> Result<(), Refusal> {
    let now = clock.now_millis();
    if now < proposal.validity.valid_from_ms {
        return Err(Refusal::ProposalNotYetValid {
            valid_from_ms: proposal.validity.valid_from_ms,
        });
    }
    if now >= proposal.validity.valid_until_ms {
        return Err(Refusal::ProposalExpired {
            valid_until_ms: proposal.validity.valid_until_ms,
        });
    }
    match snapshots.freshness(&proposal.provenance.input_snapshot_id) {
        SnapshotFreshness::Current => Ok(()),
        SnapshotFreshness::Superseded { by_snapshot_id } => {
            Err(Refusal::SnapshotSuperseded { by_snapshot_id })
        }
        SnapshotFreshness::Unknown => Err(Refusal::SnapshotUnknown),
    }
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is
// hand-computed from the rules above — window boundaries, frame sizes,
// refusal names — or a literal copied from this module's own constants.
// Nothing is fabricated.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// A fixed injected clock: the test supplies the instant; `now` never
    /// comes from a wall clock (the injected-clock discipline).
    struct FixedClock {
        now: i64,
    }

    impl HarnessClock for FixedClock {
        fn now_millis(&self) -> i64 {
            self.now
        }
    }

    // --- constants and the recorded rule ------------------------------------

    #[test]
    fn window_rule_states_the_exactly_at_expiry_rule() {
        assert!(WINDOW_RULE.contains("valid_from_ms <= now <"));
        assert!(WINDOW_RULE.contains("exactly-at-expiry"));
        assert!(WINDOW_RULE.contains("REJECTED"));
        assert!(WINDOW_RULE.contains("never read from the proposal's own clock"));
        assert!(KERNEL_CHECKPOINT_ALIGNMENT_SEAM.contains("FileRevocationRegister"));
        assert!(KERNEL_CHECKPOINT_ALIGNMENT_SEAM.contains("no flexcore-kernel dependency"));
    }

    #[test]
    fn the_window_cap_is_the_recorded_default() {
        // One hour, hand-computed: 60 * 60 * 1_000 = 3_600_000 ms.
        assert_eq!(MAX_VALIDITY_WINDOW_MS, 3_600_000);
    }

    #[test]
    fn every_window_error_has_its_known_answer_name() {
        let cases: Vec<(WindowError, &'static str)> = vec![
            (
                WindowError::WindowMissing { field: "valid_from_ms" },
                "PROPOSAL_WINDOW_MISSING",
            ),
            (
                WindowError::WindowInverted { valid_from_ms: 2, valid_until_ms: 1 },
                "PROPOSAL_WINDOW_INVERTED",
            ),
            (
                WindowError::WindowDurationNonPositive { duration_ms: 0 },
                "PROPOSAL_WINDOW_DURATION_NON_POSITIVE",
            ),
            (
                WindowError::WindowExceedsCap { duration_ms: 9, cap_ms: 1 },
                "PROPOSAL_WINDOW_EXCEEDS_CAP",
            ),
            (
                WindowError::SupersessionInvalid { reason: "r".to_string() },
                "SNAPSHOT_SUPERSESSION_INVALID",
            ),
            (WindowError::SupersessionActorRequired, "SNAPSHOT_SUPERSESSION_ACTOR_REQUIRED"),
            (
                WindowError::SnapshotAlreadySuperseded { snapshot_id: "s".to_string() },
                "SNAPSHOT_ALREADY_SUPERSEDED",
            ),
            (
                WindowError::RegisterUnavailable { reason: "r".to_string() },
                "WINDOW_REGISTER_UNAVAILABLE",
            ),
            (
                WindowError::RegisterFileForeign { reason: "r".to_string() },
                "WINDOW_REGISTER_FILE_FOREIGN",
            ),
        ];
        for (error, name) in cases {
            assert_eq!(error.name(), name);
            assert!(!error.to_string().is_empty());
        }
    }

    // --- the host-side grantor: injected clock, capped duration -------------

    #[test]
    fn the_grantor_uses_the_injected_clock_and_caps_the_duration() {
        // Hand-computed: clock at 1_000, duration 500 → [1_000, 1_500).
        let clock = FixedClock { now: 1_000 };
        let window = grant_validity_window(&clock, 500).expect("grants");
        assert_eq!(window.valid_from_ms, 1_000);
        assert_eq!(window.valid_until_ms, 1_500);

        // Non-positive durations are refused.
        for duration in [0, -1] {
            let err = grant_validity_window(&clock, duration)
                .expect_err("non-positive duration refused");
            assert_eq!(err.name(), "PROPOSAL_WINDOW_DURATION_NON_POSITIVE");
        }
        // Over-cap durations are refused (the no-unbounded-window rule).
        let err = grant_validity_window(&clock, MAX_VALIDITY_WINDOW_MS + 1)
            .expect_err("over-cap duration refused");
        assert_eq!(
            err,
            WindowError::WindowExceedsCap {
                duration_ms: MAX_VALIDITY_WINDOW_MS + 1,
                cap_ms: MAX_VALIDITY_WINDOW_MS,
            }
        );
        // Exactly the cap is granted: [1_000, 1_000 + 3_600_000).
        let window = grant_validity_window(&clock, MAX_VALIDITY_WINDOW_MS).expect("grants");
        assert_eq!(window.valid_until_ms, 3_601_000);
    }

    // --- the wire extraction: missing and inverted windows refused ----------

    #[test]
    fn missing_window_is_refused_with_the_absent_field_named() {
        let err = window_from_wire_parts(None, Some(2_000))
            .expect_err("missing valid_from refused");
        assert_eq!(
            err,
            WindowError::WindowMissing { field: "valid_from_ms" }
        );
        let err = window_from_wire_parts(Some(1_000), None)
            .expect_err("missing valid_until refused");
        assert_eq!(err, WindowError::WindowMissing { field: "valid_until_ms" });
        let err = window_from_wire_parts(None, None).expect_err("both missing refused");
        // The FIRST absent field wins, in fixed order.
        assert_eq!(err, WindowError::WindowMissing { field: "valid_from_ms" });
    }

    #[test]
    fn inverted_wire_window_is_refused_and_a_valid_one_builds() {
        let err = window_from_wire_parts(Some(2_000), Some(1_000))
            .expect_err("inverted window refused");
        assert_eq!(err.name(), "PROPOSAL_WINDOW_INVERTED");
        // An EMPTY window (from == until) is also inverted.
        let err = window_from_wire_parts(Some(2_000), Some(2_000))
            .expect_err("empty window refused");
        assert_eq!(err.name(), "PROPOSAL_WINDOW_INVERTED");
        let window = window_from_wire_parts(Some(1_000), Some(2_000)).expect("valid");
        assert_eq!(window.valid_from_ms, 1_000);
        assert_eq!(window.valid_until_ms, 2_000);
    }

    // --- the freshness source: superseded / current / unknown ---------------

    #[test]
    fn freshness_classifies_superseded_current_and_unknown() {
        let register = MemorySnapshotSupersessionRegister::new();
        let clock = FixedClock { now: 10_000 };
        register.register_current("snap-a").expect("registers");
        register.register_current("snap-b").expect("registers");
        // A→B: A becomes Superseded { by B }; B stays Current.
        register.supersede("snap-a", "snap-b", "host", &clock).expect("supersedes");
        let source = CheckpointedSnapshotSource::new(register);
        assert_eq!(
            source.freshness("snap-a"),
            SnapshotFreshness::Superseded { by_snapshot_id: "snap-b".to_string() }
        );
        assert_eq!(source.freshness("snap-b"), SnapshotFreshness::Current);
        // Unknown is never fresh (the P4.11 rule).
        assert_eq!(source.freshness("never-registered"), SnapshotFreshness::Unknown);
        // A supersession chain A→B then B→C: freshness is the DIRECT
        // record (A is stale with by = B; the consumer re-checks B and
        // sees the chain). The source still exposes the register, so the
        // host supersedes through it (supersede takes &self).
        source
            .register()
            .supersede("snap-b", "snap-c", "host", &clock)
            .expect("supersedes");
        assert_eq!(
            source.freshness("snap-a"),
            SnapshotFreshness::Superseded { by_snapshot_id: "snap-b".to_string() }
        );
        assert_eq!(
            source.freshness("snap-b"),
            SnapshotFreshness::Superseded { by_snapshot_id: "snap-c".to_string() }
        );
        assert_eq!(source.freshness("snap-c"), SnapshotFreshness::Current);
    }

    #[test]
    fn supersession_validation_refuses_empty_self_and_duplicate() {
        let register = MemorySnapshotSupersessionRegister::new();
        let clock = FixedClock { now: 11_000 };
        // Empty ids refused.
        assert_eq!(
            register.supersede("", "snap-b", "host", &clock).expect_err("empty old refused").name(),
            "SNAPSHOT_SUPERSESSION_INVALID"
        );
        assert_eq!(
            register.supersede("snap-a", "  ", "host", &clock).expect_err("empty new refused").name(),
            "SNAPSHOT_SUPERSESSION_INVALID"
        );
        // Self-supersession refused.
        assert_eq!(
            register
                .supersede("snap-a", "snap-a", "host", &clock)
                .expect_err("self-supersession refused")
                .name(),
            "SNAPSHOT_SUPERSESSION_INVALID"
        );
        // Empty actor refused.
        assert_eq!(
            register.supersede("snap-a", "snap-b", "  ", &clock).expect_err("no actor refused").name(),
            "SNAPSHOT_SUPERSESSION_ACTOR_REQUIRED"
        );
        // A duplicate supersession is refused (append-only, one per key).
        register.supersede("snap-a", "snap-b", "host", &clock).expect("first");
        assert_eq!(
            register
                .supersede("snap-a", "snap-c", "host", &clock)
                .expect_err("duplicate refused")
                .name(),
            "SNAPSHOT_ALREADY_SUPERSEDED"
        );
        // The FIRST record stands (never rewritten).
        assert_eq!(register.records().len(), 1);
        assert_eq!(register.records()[0].by_snapshot_id, "snap-b");
        assert_eq!(register.records()[0].superseded_at_millis, 11_000);
    }

    // --- the file register: restart-surviving checkpoint ---------------------

    #[test]
    fn file_register_round_trips_and_survives_a_reopen() {
        let path = std::env::temp_dir().join(format!(
            "flexcore-snap-reg-test-roundtrip-{}.bin",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        {
            let register = FileSnapshotSupersessionRegister::open(&path).expect("opens");
            let clock = FixedClock { now: 20_000 };
            register.register_current("snap-a").expect("registers");
            register.supersede("snap-a", "snap-b", "host", &clock).expect("supersedes");
            assert_eq!(register.records_written(), 1);
        }
        // REOPEN: the supersession survived the restart.
        {
            let register = FileSnapshotSupersessionRegister::open(&path).expect("reopens");
            assert_eq!(register.records().len(), 1);
            assert_eq!(register.records()[0].superseded_snapshot_id, "snap-a");
            assert_eq!(register.records()[0].by_snapshot_id, "snap-b");
            assert_eq!(register.records()[0].actor, "host");
            assert_eq!(register.records()[0].superseded_at_millis, 20_000);
            let source = CheckpointedSnapshotSource::new(register);
            assert_eq!(
                source.freshness("snap-a"),
                SnapshotFreshness::Superseded { by_snapshot_id: "snap-b".to_string() }
            );
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn file_register_refuses_a_foreign_file_and_counts_a_corrupt_tail() {
        // A file with the WRONG magic is a hard open error.
        let foreign = std::env::temp_dir().join(format!(
            "flexcore-snap-reg-test-foreign-{}.bin",
            std::process::id()
        ));
        std::fs::write(&foreign, b"NOT-A-FLEXCORE-REGISTER-FILE").expect("writes");
        let err = FileSnapshotSupersessionRegister::open(&foreign)
            .expect_err("foreign file refused");
        assert_eq!(err.name(), "WINDOW_REGISTER_FILE_FOREIGN");
        let _ = std::fs::remove_file(&foreign);

        // A corrupt tail is truncated and COUNTED, and the good records
        // survive.
        let path = std::env::temp_dir().join(format!(
            "flexcore-snap-reg-test-corrupt-{}.bin",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        {
            let register = FileSnapshotSupersessionRegister::open(&path).expect("opens");
            let clock = FixedClock { now: 21_000 };
            register.supersede("snap-a", "snap-b", "host", &clock).expect("supersedes");
            assert_eq!(register.records_written(), 1);
        }
        {
            // Corrupt the tail: append garbage after the good frame.
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .expect("opens for append");
            std::io::Write::write_all(&mut file, b"\x99\x99\x99\x99garbage")
                .expect("appends garbage");
        }
        {
            let register = FileSnapshotSupersessionRegister::open(&path).expect("reopens");
            assert_eq!(
                register.corrupt_tail_truncated(),
                1,
                "the corrupt tail is COUNTED, never silent"
            );
            assert_eq!(register.records().len(), 1, "the good record survives");
        }
        let _ = std::fs::remove_file(&path);
    }

    // --- the gate-at-time-of-use check: known-answer boundaries ---------------

    /// A proposal computed against window [1_000, 2_000) and snapshot
    /// "snap-a" (the reference fixture; mirrors the harness test's
    /// reference proposal shape, with the provenance fields the harness
    /// requires).
    fn reference_proposal() -> ResearchProposal {
        crate::validation::ResearchProposal::new(
            "prop-window-1",
            "plugin/research-admm",
            "0.1.0",
            1,
            crate::validation::ResearchResultKind::ProposedSchedule,
            serde_json::json!({ "setpoints": [1.5] }),
            crate::validation::ResearchProvenance {
                worker_version: "research-worker/0.1.0".to_string(),
                model_version: "grid-model/2026.11".to_string(),
                solver_version: "highspy 1.15.1".to_string(),
                input_snapshot_id: "snap-a".to_string(),
                termination_status: "converged".to_string(),
                diagnostics: Vec::new(),
                seed: Some(7),
                seed_supported: true,
                deterministic: true,
                model_artifact_ref: "artifact://models/gnn-warmstart.v1".to_string(),
            },
            ResearchValidityWindow { valid_from_ms: 1_000, valid_until_ms: 2_000 },
        )
        .expect("the reference proposal is shape-valid")
    }

    /// A freshness source answering one freshness for every id.
    struct FixedSnapshots {
        fresh: SnapshotFreshness,
    }

    impl SnapshotFreshnessSource for FixedSnapshots {
        fn freshness(&self, _snapshot_id: &str) -> SnapshotFreshness {
            self.fresh.clone()
        }
    }

    #[test]
    fn gate_at_time_of_use_boundary_known_answers() {
        // Window [1_000, 2_000), snapshot Current — the window edges:
        // exactly-at-open ACCEPTED, exactly-at-expiry REJECTED (the
        // recorded rule, stated in WINDOW_RULE).
        let cases: [(i64, &'static str); 4] = [
            (1_000, "accepted"),            // exactly-at-open: ACCEPTED
            (1_999, "accepted"),            // last valid millis
            (2_000, "PROPOSAL_EXPIRED"),    // exactly-at-expiry: REJECTED
            (999, "PROPOSAL_NOT_YET_VALID"), // before the window opens
        ];
        for (now, expectation) in cases {
            let clock = FixedClock { now };
            let snapshots = FixedSnapshots { fresh: SnapshotFreshness::Current };
            let outcome = gate_at_time_of_use(&reference_proposal(), &clock, &snapshots);
            if expectation == "accepted" {
                assert!(outcome.is_ok(), "now={now} must be accepted, got {outcome:?}");
            } else {
                let refusal = outcome.expect_err("outside the window is refused");
                assert_eq!(refusal.name(), expectation, "now={now}");
            }
        }
    }

    #[test]
    fn gate_at_time_of_use_refuses_stale_before_any_downstream_step() {
        // The gate runs FIRST in any consumption path: a stale proposal is
        // refused with the harness's own refusal vocabulary.
        let clock = FixedClock { now: 5_000 }; // past the window
        let snapshots = FixedSnapshots { fresh: SnapshotFreshness::Current };
        let refusal = gate_at_time_of_use(&reference_proposal(), &clock, &snapshots)
            .expect_err("an expired proposal is refused at the point of use");
        assert_eq!(refusal.name(), "PROPOSAL_EXPIRED");
        assert_eq!(
            refusal,
            Refusal::ProposalExpired { valid_until_ms: 2_000 }
        );

        // A superseded snapshot at the point of use: the replacing id
        // travels (the harness stage-5 shape).
        let clock = FixedClock { now: 1_500 };
        let snapshots = FixedSnapshots {
            fresh: SnapshotFreshness::Superseded {
                by_snapshot_id: "snap-b".to_string(),
            },
        };
        let refusal = gate_at_time_of_use(&reference_proposal(), &clock, &snapshots)
            .expect_err("a superseded snapshot is refused");
        assert_eq!(
            refusal,
            Refusal::SnapshotSuperseded { by_snapshot_id: "snap-b".to_string() }
        );

        // Unknown is never fresh.
        let snapshots = FixedSnapshots { fresh: SnapshotFreshness::Unknown };
        let refusal = gate_at_time_of_use(&reference_proposal(), &clock, &snapshots)
            .expect_err("an unknown snapshot is refused");
        assert_eq!(refusal.name(), "SNAPSHOT_UNKNOWN");
    }

    #[test]
    fn gate_at_time_of_use_matches_the_harness_stage_decision() {
        // CONSISTENCY PIN: for the same clock and snapshot source, the
        // consumption-point check and the harness's stage-4/5 decision
        // AGREE — the one-rule-everywhere property. A full harness pass
        // implies the consumption check passes; a consumption refusal
        // class matches the harness refusal class.
        let gate_spy = GateSpy;
        let register = MemorySnapshotSupersessionRegister::new();
        register.register_current("snap-a").expect("registers");
        let source = CheckpointedSnapshotSource::new(register);
        let clock = FixedClock { now: 1_500 };
        let harness = crate::validation::ValidationHarness::new(&gate_spy, &source, &clock);
        let report = harness
            .validate(&reference_proposal())
            .expect("the harness accepts the fresh proposal");
        assert!(report.is_validated());
        gate_at_time_of_use(&reference_proposal(), &clock, &source)
            .expect("the consumption check agrees on the full pass");

        // And on the stale side: both refuse, with the SAME refusal value.
        let clock = FixedClock { now: 2_000 }; // exactly-at-expiry
        let harness = crate::validation::ValidationHarness::new(&gate_spy, &source, &clock);
        let harness_refusal = harness
            .validate(&reference_proposal())
            .expect_err("the harness refuses exactly-at-expiry");
        let gate_refusal = gate_at_time_of_use(&reference_proposal(), &clock, &source)
            .expect_err("the consumption check refuses exactly-at-expiry");
        assert_eq!(harness_refusal, gate_refusal);
        assert_eq!(harness_refusal.name(), "PROPOSAL_EXPIRED");
    }

    /// The passing gate spy (mirrors the harness test fixture; the gate is
    /// not under test here — it only needs to say Passed).
    struct GateSpy;

    impl crate::validation::IndependentFeasibilityGate for GateSpy {
        fn revalidate(
            &self,
            _proposal: &ResearchProposal,
        ) -> Result<crate::validation::IndependentVerdict, crate::validation::GateSeamError> {
            Ok(crate::validation::IndependentVerdict::Passed {
                attempt_id: "AT-W".to_string(),
                model_id: "ieee13".to_string(),
                model_content_hash: "hash-w".to_string(),
                margins: "no violations".to_string(),
                coverage: "declared scope only".to_string(),
            })
        }
    }

    #[test]
    fn the_stage_list_is_unchanged_by_this_module() {
        // The additive-integration record: P8.34 changes NOTHING in the
        // harness's fixed stage order — the stage list and version are the
        // harness's own, unchanged (a change would be a NEW harness
        // version, never a silent edit).
        assert_eq!(
            crate::validation::STAGE_ORDER,
            [
                "envelope_shape",
                "result_vocabulary",
                "provenance_completeness",
                "validity_window",
                "snapshot_freshness",
                "physical_revalidation",
            ]
        );
        assert_eq!(
            crate::validation::HARNESS_VERSION,
            "flexcore-research/validation.v1"
        );
    }

    /// Counts gate calls (the harness test's pattern, reused to pin that
    /// the consumption check does not touch the gate at all — it is a
    /// pure freshness/window re-check).
    #[test]
    fn the_consumption_check_never_consumes_a_gate_attempt() {
        let calls = Cell::new(0usize);
        let gate = CountingGate { calls: &calls };
        let register = MemorySnapshotSupersessionRegister::new();
        register.register_current("snap-a").expect("registers");
        let source = CheckpointedSnapshotSource::new(register);
        let clock = FixedClock { now: 1_500 };
        gate_at_time_of_use(&reference_proposal(), &clock, &source)
            .expect("the consumption check passes without the gate");
        assert_eq!(calls.get(), 0, "the consumption check never calls the gate");
        drop(gate);
    }

    struct CountingGate<'a> {
        calls: &'a Cell<usize>,
    }

    impl crate::validation::IndependentFeasibilityGate for CountingGate<'_> {
        fn revalidate(
            &self,
            _proposal: &ResearchProposal,
        ) -> Result<crate::validation::IndependentVerdict, crate::validation::GateSeamError> {
            self.calls.set(self.calls.get() + 1);
            Ok(crate::validation::IndependentVerdict::Passed {
                attempt_id: "AT-C".to_string(),
                model_id: "ieee13".to_string(),
                model_content_hash: "hash-c".to_string(),
                margins: "no violations".to_string(),
                coverage: "declared scope only".to_string(),
            })
        }
    }
}
