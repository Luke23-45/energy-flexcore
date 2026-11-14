// P6.18 — Local audit buffer and offline behaviour (report §18.1, §18.3,
//         §21.2, §17.3; ledger P6.18). P6.21 uses this module's record shape
//         for the audited isolation denials (actor, object, timestamp).
//
// [`FileAuditBuffer`] is the durable edge-local audit buffer: an
// append-oriented, BOUNDED journal of audit records that survives an edge
// restart and uploads IN ORDER on reconnect. It mirrors the P6.09
// mechanics of `buffer_file::FileLocalBuffer` (same framing discipline,
// same recovery policy, same overflow policy) — the mechanics are REUSED
// as patterns, not re-invented from a new design.
//
// **Journal format (std-only, no new dependencies).** A fixed magic
// header, then length-prefixed CRC-checked records — the exact framing
// discipline of the P6.09 telemetry journal:
//
// ```text
// header:  b"FLEXCORE-EDGE-AUDIT-JOURNAL" + format version byte (0x01)
// record:  [u32 LE payload_len][u32 LE crc32(payload)][payload]
// payload: manually encoded BufferedAuditRecord (see `wire` below) —
//          std-only serialization, decoded through strict validation, so
//          a bit-corrupt-but-CRC-collision-free record can never produce
//          a silently invalid audit record.
// ```
//
// **Closed kind vocabulary, INSIDE this module.** `EdgeAuditKind`
// (`crate::ports`) is a CLOSED vocabulary this wave does not own (the
// recorded limitation in the P6.14 decision doc), so this module carries
// its OWN closed enum — [`BufferedAuditKind`] with the enrollment /
// command / health / rejection kinds — and maps the runner's
// `EdgeAuditKind` events onto it when composed as the edge `AuditSink`.
// The stable, greppable event names (`EDGE_*`, the `enrollment.rs` naming
// style) are preserved: command-path and scope-denial events keep their
// dedicated name LEADING the detail string (the recorded
// `EdgeAuditKind`-limitation pattern), and the mapping never rewrites a
// detail.
//
// **Upload semantics — at-least-once, in-order only.** `append` writes the
// record to disk BEFORE it exists in memory (a crash cannot lose an
// appended audit record). `drain` removes from memory ONLY — the journal
// keeps the records until the upload driver confirms delivery through
// [`FileAuditBuffer::commit_uploaded`] (the mirror of the P6.09
// `commit_forwarded` commit point), which compacts the journal to the
// still-pending set. A crash in the drain→upload→commit window re-delivers
// the records after restart — at-least-once, the §15.4 semantics, same as
// telemetry. IN-ORDER is structural: `drain` always takes from the FRONT
// (the lowest sequence first), so no record is ever uploaded before its
// predecessors; and `commit_uploaded` refuses (fail-closed, nothing
// removed) a commit whose first record's sequence does not match the
// journal's front — an un-uploaded record is NEVER removed out of order
// and never overwritten in place. Monotonic sequences are assigned at
// append time and PERSIST in the journal, so ordering survives restarts
// (reconnect = the same global order, not a per-boot order).
//
// **Bounds and overflow policy (the recorded P6.09 pattern).** Capacity is
// a record bound given at open. The overflow policy is **audited
// drop-of-OLDEST**: when an append would exceed capacity the OLDEST
// record is removed, a hard counter increments
// ([`FileAuditBuffer::dropped_oldest`]), the health flag
// ([`FileAuditBuffer::has_overflowed`]) LATCHES, and the drop is logged —
// never a silent overwrite, never unbounded growth, and the loss is
// REPORTED (the counter and the flag are the explicit loss report an
// operator's health surface reads; P6.17 consumes them). There is
// deliberately NO age-based drop for audit records: audit evidence
// retention is a deployment `RetentionPolicy` decision (P0.16 §4), not a
// period in code — recorded as DEFAULT decision D-2 in the decision
// document.
//
// **Recovery (truncate-and-audit, never silent).** Identical to the P6.09
// discipline: the FIRST malformed frame (bad length, CRC mismatch,
// undecodable payload) ends the replay and the file is TRUNCATED at the
// last good record, with a named counter
// ([`FileAuditBuffer::corrupt_tail_truncated`]) and a loud log line. No
// resync, no guessing past corruption. A wrong magic/version is a hard
// open error.
//
// **Encryption at rest: NOT implemented.** Recorded honestly twice: it is
// an open item in the P6.09 decision document (`08_09_ingest_buffer.md`
// §6) and it is repeated here as a DEPLOYMENT REQUIREMENT — until closed
// with a P6.02-audited crypto dependency, the audit journal holds records
// in the clear on the edge host, and the deployment profile (P6.20 / D6)
// must treat the audit-journal path as in-scope for OS-level site
// protection (protected volume / filesystem permissions / full-disk
// encryption per the site's OT boundary review).
//
// **OFFLINE BEHAVIOUR STATEMENT (ledger P6.18 step 2 — recorded in the
// decision document and summarised here):**
//
// 1. The local site EMS remains authoritative for fast safety behaviour —
//    FlexCore is a supervisory orchestration layer (AGENTS.md rule 3) and
//    never performs protection-grade or sub-cycle control.
// 2. While disconnected, the edge issues NO new commands — the test-env
//    command path included: the P6.14 gate is read-time-checked
//    (`CommandPathRuntime::enablement_at`) and the channel that carries
//    response-direction frames is DOWN while disconnected, so nothing can
//    arrive, be admitted, or be queued for later issuance (a refused
//    command is retained nowhere — there is no queue field).
// 3. Telemetry buffering while offline follows the P6.09 design
//    (`FileLocalBuffer`): bounded, audited drop-oldest, in-order
//    store-and-forward.
// 4. A central outage does NOT disable site-local protective controls and
//    does NOT force any unsafe device state (§21.2): the edge agent has
//    NO field-write authority at all in v0.4.0 (read-only / test-lab by
//    default), and the site EMS's own protective behaviour is entirely
//    local to the site.
//
// Durability note (honest, same as P6.09): appends are `sync_data`-flushed
// per record; compaction rewrites the journal through a sibling temp file
// synced and RENAMED over it, so a crash mid-compaction cannot destroy the
// previous journal. The audited counters are in-memory per open (a restart
// loses the counts; the loss events themselves were audited at the moment
// they happened, and the flag state lives in the operator-visible
// composition) — persisting counters is a P6.17 candidate, recorded.

use std::collections::VecDeque;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use flexcore_contracts::time_model::IngestTime;
use flexcore_ports::clock::Clock;

use crate::ports::{AuditSink, EdgeAuditEvent, EdgeAuditKind, EdgeError};

/// The audit journal magic header (27 bytes: 26-byte label + version).
const JOURNAL_MAGIC: &[u8] = b"FLEXCORE-EDGE-AUDIT-JOURNAL";
const JOURNAL_VERSION: u8 = 0x01;
/// Hard sanity cap on one encoded record (rejects absurd lengths from a
/// corrupt header instead of attempting an absurd allocation). Audit
/// records are small; 64 KiB is generous.
const MAX_RECORD_BYTES: u32 = 1 << 16;

// --- Open/IO errors ----------------------------------------------------------

/// Errors of opening or operating the journal-backed audit buffer. Every
/// failure is named; nothing is swallowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileAuditBufferError {
    /// The buffer path was empty or not usable as a file path.
    InvalidPath { reason: String },
    /// The underlying filesystem failed (create/open/read/write).
    Io { reason: String },
    /// The file exists but is not a FlexCore edge audit journal (wrong
    /// magic or version) — refused, never guessed at.
    NotAJournal { reason: String },
}

impl std::fmt::Display for FileAuditBufferError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileAuditBufferError::InvalidPath { reason } => {
                write!(f, "the audit buffer path is not usable: {reason}")
            }
            FileAuditBufferError::Io { reason } => {
                write!(f, "the audit journal I/O failed: {reason}")
            }
            FileAuditBufferError::NotAJournal { reason } => write!(
                f,
                "the file is not a FlexCore edge audit journal (refused, never guessed): {reason}"
            ),
        }
    }
}

impl std::error::Error for FileAuditBufferError {}

/// Errors of the upload-commit path. The commit is the at-least-once
/// boundary; a failure here leaves every record ON DISK and PENDING —
/// the upload retries, nothing is lost or removed out of order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditUploadError {
    /// The journal rewrite failed; the previous journal is intact, the
    /// in-memory pending set is the truth, and the next commit retries.
    Io { reason: String },
    /// The commit was refused because the journal's front record does not
    /// match the first record being committed — committing would remove an
    /// UN-UPLOADED predecessor. Nothing was removed (fail-closed).
    OutOfOrderCommit {
        /// The sequence the caller's batch starts with.
        expected_front_sequence: u64,
        /// The sequence actually at the journal's front.
        found_front_sequence: Option<u64>,
    },
}

impl std::fmt::Display for AuditUploadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditUploadError::Io { reason } => {
                write!(f, "the audit journal compaction failed: {reason}")
            }
            AuditUploadError::OutOfOrderCommit {
                expected_front_sequence,
                found_front_sequence,
            } => write!(
                f,
                "out-of-order upload commit REFUSED (fail-closed): the batch starts at \
                 sequence {expected_front_sequence} but the journal front is \
                 {found_front_sequence:?}; no record was removed"
            ),
        }
    }
}

impl std::error::Error for AuditUploadError {}

// --- The closed audit kind vocabulary (THIS module owns it) -------------------

/// The CLOSED audit-event kind vocabulary of the durable audit buffer:
/// the enrollment / command / health / rejection kinds the edge generates.
/// This enum exists INSIDE this module because `EdgeAuditKind`
/// (`crate::ports`) is a closed vocabulary this wave does not own (the
/// recorded P6.14 limitation); extending THIS set is a reviewed change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferedAuditKind {
    // --- Enrollment lifecycle (§17.3 cross-boundary identity events) ---
    /// The edge enrolled successfully (mirrors `EdgeAuditKind::EnrollmentCompleted`).
    EnrollmentCompleted,
    /// An enrollment attempt was REJECTED.
    EnrollmentRejected,
    /// The credential entered the rotation window.
    CredentialExpiryWarning,
    /// The credential has EXPIRED.
    CredentialExpired,
    /// A revocation check could not establish a status.
    RevocationUnchecked,
    // --- Command path (test-env only; P6.14 gate events) ---
    /// The test-env command path was enabled (approver + expiry in detail).
    CommandPathEnabled,
    /// The command path is disabled (composition) or auto-disabled (expiry).
    CommandPathDisabled,
    /// A command was ADMITTED by the gate + policy (admission, not delivery).
    CommandAccepted,
    /// A command was REFUSED (the P4-pattern denial record in detail).
    CommandRejected,
    // --- Health / supervision (the P6.05 runner transitions) ---
    /// Startup / shutdown / configuration summary events.
    SupervisionStartup,
    /// The uplink failed; degraded (buffering) mode entered.
    DegradedBufferingEntered,
    /// The uplink recovered; degraded mode exited.
    DegradedBufferingExited,
    /// A telemetry batch was forwarded to the uplink.
    ForwardBatch,
    /// A poll of the EMS source failed.
    SourcePollFailed,
    /// The telemetry buffer reached capacity (backpressure).
    BufferFull,
    // --- Isolation / scope denials (P6.21) ---
    /// A cross-tenant / cross-site access attempt was DENIED at the edge
    /// (actor, object, timestamp in the record — never silently denied).
    ScopeDenied,
    // --- Buffer integrity events (this buffer's own losses, reported) ---
    /// The audit buffer overflowed and the OLDEST record was dropped
    /// (audited drop-of-oldest; the loss is REPORTED, never silent).
    AuditBufferOverflow,
    /// A corrupt/truncated journal tail was cut at open (counted loss).
    AuditBufferCorruptTail,
}

impl BufferedAuditKind {
    /// Stable audit name (`EDGE_*` SCREAMING_SNAKE, the `enrollment.rs`
    /// naming style) — the greppable audit identity in log/audit lines.
    pub fn name(self) -> &'static str {
        match self {
            BufferedAuditKind::EnrollmentCompleted => "EDGE_ENROLLMENT_COMPLETED",
            BufferedAuditKind::EnrollmentRejected => "EDGE_ENROLLMENT_REJECTED",
            BufferedAuditKind::CredentialExpiryWarning => "EDGE_CREDENTIAL_EXPIRY_WARNING",
            BufferedAuditKind::CredentialExpired => "EDGE_CREDENTIAL_EXPIRED",
            BufferedAuditKind::RevocationUnchecked => "EDGE_REVOCATION_UNCHECKED",
            BufferedAuditKind::CommandPathEnabled => "EDGE_COMMAND_PATH_ENABLED",
            BufferedAuditKind::CommandPathDisabled => "EDGE_COMMAND_PATH_DISABLED",
            BufferedAuditKind::CommandAccepted => "EDGE_COMMAND_ACCEPTED",
            BufferedAuditKind::CommandRejected => "EDGE_COMMAND_REJECTED",
            BufferedAuditKind::SupervisionStartup => "EDGE_STARTUP",
            BufferedAuditKind::DegradedBufferingEntered => "EDGE_DEGRADED_BUFFERING_ENTERED",
            BufferedAuditKind::DegradedBufferingExited => "EDGE_DEGRADED_BUFFERING_EXITED",
            BufferedAuditKind::ForwardBatch => "EDGE_FORWARD_BATCH",
            BufferedAuditKind::SourcePollFailed => "EDGE_SOURCE_POLL_FAILED",
            BufferedAuditKind::BufferFull => "EDGE_BUFFER_FULL",
            BufferedAuditKind::ScopeDenied => "EDGE_SCOPE_DENIED",
            BufferedAuditKind::AuditBufferOverflow => "EDGE_AUDIT_BUFFER_OVERFLOW",
            BufferedAuditKind::AuditBufferCorruptTail => "EDGE_AUDIT_BUFFER_CORRUPT_TAIL",
        }
    }

    /// The positional wire discriminant (frozen order; a decode of an
    /// unknown discriminant is a CORRUPT record — truncate-and-audit,
    /// never a guess).
    fn code(self) -> u8 {
        match self {
            BufferedAuditKind::EnrollmentCompleted => 0,
            BufferedAuditKind::EnrollmentRejected => 1,
            BufferedAuditKind::CredentialExpiryWarning => 2,
            BufferedAuditKind::CredentialExpired => 3,
            BufferedAuditKind::RevocationUnchecked => 4,
            BufferedAuditKind::CommandPathEnabled => 5,
            BufferedAuditKind::CommandPathDisabled => 6,
            BufferedAuditKind::CommandAccepted => 7,
            BufferedAuditKind::CommandRejected => 8,
            BufferedAuditKind::SupervisionStartup => 9,
            BufferedAuditKind::DegradedBufferingEntered => 10,
            BufferedAuditKind::DegradedBufferingExited => 11,
            BufferedAuditKind::ForwardBatch => 12,
            BufferedAuditKind::SourcePollFailed => 13,
            BufferedAuditKind::BufferFull => 14,
            BufferedAuditKind::ScopeDenied => 15,
            BufferedAuditKind::AuditBufferOverflow => 16,
            BufferedAuditKind::AuditBufferCorruptTail => 17,
        }
    }

    fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            0 => BufferedAuditKind::EnrollmentCompleted,
            1 => BufferedAuditKind::EnrollmentRejected,
            2 => BufferedAuditKind::CredentialExpiryWarning,
            3 => BufferedAuditKind::CredentialExpired,
            4 => BufferedAuditKind::RevocationUnchecked,
            5 => BufferedAuditKind::CommandPathEnabled,
            6 => BufferedAuditKind::CommandPathDisabled,
            7 => BufferedAuditKind::CommandAccepted,
            8 => BufferedAuditKind::CommandRejected,
            9 => BufferedAuditKind::SupervisionStartup,
            10 => BufferedAuditKind::DegradedBufferingEntered,
            11 => BufferedAuditKind::DegradedBufferingExited,
            12 => BufferedAuditKind::ForwardBatch,
            13 => BufferedAuditKind::SourcePollFailed,
            14 => BufferedAuditKind::BufferFull,
            15 => BufferedAuditKind::ScopeDenied,
            16 => BufferedAuditKind::AuditBufferOverflow,
            17 => BufferedAuditKind::AuditBufferCorruptTail,
            _ => return None,
        })
    }

    /// Maps the runner-port [`EdgeAuditKind`] onto this module's closed
    /// vocabulary (the composition path when the buffer is the edge's
    /// [`AuditSink`]). The detail string is never rewritten: command-path
    /// and scope-denial events keep their dedicated name leading the
    /// detail (the recorded `EdgeAuditKind` limitation — do not fix here;
    /// it stays recorded).
    pub fn map_edge_kind(kind: EdgeAuditKind) -> Self {
        match kind {
            EdgeAuditKind::Startup => BufferedAuditKind::SupervisionStartup,
            EdgeAuditKind::DegradedBufferingEntered => BufferedAuditKind::DegradedBufferingEntered,
            EdgeAuditKind::DegradedBufferingExited => BufferedAuditKind::DegradedBufferingExited,
            EdgeAuditKind::ForwardBatch => BufferedAuditKind::ForwardBatch,
            EdgeAuditKind::SourcePollFailed => BufferedAuditKind::SourcePollFailed,
            EdgeAuditKind::BufferFull => BufferedAuditKind::BufferFull,
            EdgeAuditKind::EnrollmentCompleted => BufferedAuditKind::EnrollmentCompleted,
            EdgeAuditKind::EnrollmentRejected => BufferedAuditKind::EnrollmentRejected,
            EdgeAuditKind::CredentialExpiryWarning => BufferedAuditKind::CredentialExpiryWarning,
            EdgeAuditKind::CredentialExpired => BufferedAuditKind::CredentialExpired,
            EdgeAuditKind::RevocationUnchecked => BufferedAuditKind::RevocationUnchecked,
        }
    }
}

// --- The record ----------------------------------------------------------------

/// One durable audit record (the journal payload and the upload unit).
/// `actor` and `object` make the P6.21 denial-audit requirement explicit
/// for every record: WHO acted, on WHAT — both plain, non-secret text
/// (no token value, no key material ever enters these fields).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferedAuditRecord {
    /// The monotonic upload sequence, assigned at append time and PERSISTED
    /// in the journal. Upload is in-order by this sequence across restarts.
    pub sequence: u64,
    /// When the edge recorded the event (the P0.09 `IngestTime` contracts
    /// type: the moment the EDGE observed the fact).
    pub recorded_at: IngestTime,
    /// The event kind (this module's closed vocabulary).
    pub kind: BufferedAuditKind,
    /// The acting principal (e.g. `edge:pilot-site-A`, `edge:enrollment:<device>`,
    /// an operator id). Never empty; never a secret.
    pub actor: String,
    /// The object the event concerns (e.g. the requested tenant/site/asset,
    /// the command id, the adapter id). Never empty; never a secret.
    pub object: String,
    /// Human-safe detail (the dedicated event name leads it where one
    /// exists; no secret material).
    pub detail: String,
}

// --- std-only wire encoding -----------------------------------------------------

mod wire {
    use super::{BufferedAuditKind, BufferedAuditRecord};
    use flexcore_contracts::time_model::IngestTime;

    /// A decode failure is a CORRUPT record for the recovery policy — the
    /// caller truncates and audits, never guesses.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum WireError {
        UnexpectedEnd,
        UnknownKind { code: u8 },
        BadString,
        BadTimestamp { reason: String },
    }

    impl std::fmt::Display for WireError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                WireError::UnexpectedEnd => write!(f, "record ends unexpectedly"),
                WireError::UnknownKind { code } => write!(f, "unknown audit-kind code {code}"),
                WireError::BadString => {
                    write!(f, "a length-prefixed string is not valid UTF-8")
                }
                WireError::BadTimestamp { reason } => write!(f, "{reason}"),
            }
        }
    }

    struct Reader<'a> {
        bytes: &'a [u8],
        pos: usize,
    }

    impl<'a> Reader<'a> {
        fn new(bytes: &'a [u8]) -> Self {
            Self { bytes, pos: 0 }
        }

        fn take(&mut self, n: usize) -> Result<&'a [u8], WireError> {
            if self.pos + n > self.bytes.len() {
                return Err(WireError::UnexpectedEnd);
            }
            let slice = &self.bytes[self.pos..self.pos + n];
            self.pos += n;
            Ok(slice)
        }

        fn u8(&mut self) -> Result<u8, WireError> {
            Ok(self.take(1)?[0])
        }

        fn u64(&mut self) -> Result<u64, WireError> {
            let b = self.take(8)?;
            let mut array = [0u8; 8];
            array.copy_from_slice(b);
            Ok(u64::from_le_bytes(array))
        }

        fn i64(&mut self) -> Result<i64, WireError> {
            Ok(self.u64()? as i64)
        }

        fn string(&mut self) -> Result<String, WireError> {
            let len = self.u64()? as usize;
            let bytes = self.take(len)?;
            String::from_utf8(bytes.to_vec()).map_err(|_| WireError::BadString)
        }
    }

    fn put_u64(out: &mut Vec<u8>, value: u64) {
        out.extend_from_slice(&value.to_le_bytes());
    }

    fn put_string(out: &mut Vec<u8>, value: &str) {
        put_u64(out, value.len() as u64);
        out.extend_from_slice(value.as_bytes());
    }

    /// Encodes one audit record (the journal record payload).
    pub fn encode_record(record: &BufferedAuditRecord) -> Vec<u8> {
        let mut out = Vec::with_capacity(64);
        put_u64(&mut out, record.sequence);
        // The timestamp travels as its own millisecond value and is
        // rebuilt through the contracts constructor (the type is never
        // bypassed; an out-of-range value is a CORRUPT record).
        put_u64(&mut out, record.recorded_at.unix_millis() as u64);
        out.push(record.kind.code());
        put_string(&mut out, &record.actor);
        put_string(&mut out, &record.object);
        put_string(&mut out, &record.detail);
        out
    }

    /// Decodes one record payload back into a [`BufferedAuditRecord`]
    /// (strict: an unknown kind code, an undecodable string, or an
    /// invalid timestamp is a CORRUPT record for the recovery policy).
    pub fn decode_record(bytes: &[u8]) -> Result<BufferedAuditRecord, WireError> {
        let mut reader = Reader::new(bytes);
        let sequence = reader.u64()?;
        let millis = reader.i64()?;
        let kind_code = reader.u8()?;
        let kind = BufferedAuditKind::from_code(kind_code).ok_or(WireError::UnknownKind {
            code: kind_code,
        })?;
        let actor = reader.string()?;
        let object = reader.string()?;
        let detail = reader.string()?;
        let recorded_at =
            IngestTime::from_unix_millis(millis).map_err(|error| WireError::BadTimestamp {
                reason: format!("the recorded timestamp is not a valid instant: {error}"),
            })?;
        Ok(BufferedAuditRecord {
            sequence,
            recorded_at,
            kind,
            actor,
            object,
            detail,
        })
    }
}

/// CRC-32 (IEEE 802.3), bitwise — std-only, no dependency (the same
/// framing discipline as the P6.09 telemetry journal).
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// One on-disk frame: length + CRC + payload.
fn frame_bytes(payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(payload.len() + 8);
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(&crc32(payload).to_le_bytes());
    frame.extend_from_slice(payload);
    frame
}

// --- The buffer -------------------------------------------------------------------

/// The mutable state behind the buffer. [`FileAuditBuffer`] wraps it in a
/// `Mutex` because the buffer composes as the edge [`AuditSink`] (whose
/// `record` takes `&self`) while the upload driver also calls `drain` /
/// `commit_uploaded` on shared references.
struct Inner {
    path: PathBuf,
    capacity: usize,
    clock: Arc<dyn Clock + Send + Sync>,
    /// The pending (not yet commit-uploaded) records, oldest (lowest
    /// sequence) first.
    queue: VecDeque<BufferedAuditRecord>,
    file: std::fs::File,
    file_len: u64,
    /// The next sequence to assign (persisted order: continues from the
    /// replayed journal's highest sequence).
    next_sequence: u64,
    // Audited counters (in-memory per open — recorded limitation).
    appended_total: u64,
    dropped_oldest: u64,
    corrupt_tail_truncated: u64,
    overflowed: bool,
}

impl Inner {
    /// Appends one frame at the physical end of the journal, synced
    /// (disk FIRST — a crash cannot lose a record the caller was told was
    /// buffered).
    fn append_frame(&mut self, record: &BufferedAuditRecord) -> Result<(), FileAuditBufferError> {
        let payload = wire::encode_record(record);
        let frame = frame_bytes(&payload);
        let write_result = (|| -> std::io::Result<()> {
            self.file.seek(SeekFrom::Start(self.file_len))?;
            self.file.write_all(&frame)?;
            self.file.sync_data()?;
            Ok(())
        })();
        match write_result {
            Ok(()) => {
                self.file_len += frame.len() as u64;
                Ok(())
            }
            Err(error) => Err(FileAuditBufferError::Io {
                reason: format!("could not append to the audit journal: {error}"),
            }),
        }
    }

    /// The shared append path (used by both `record_event` and the
    /// [`AuditSink`] composition): sequence assignment, disk-first append,
    /// then memory, then the audited drop-of-OLDEST overflow policy.
    fn append_record(&mut self, mut record: BufferedAuditRecord) -> Result<(), EdgeError> {
        record.sequence = self.next_sequence;
        self.next_sequence += 1;

        // Disk first (fail-closed: on an I/O failure the caller learns the
        // record was NOT durably buffered and memory is unchanged).
        if let Err(error) = self.append_frame(&record) {
            // The sequence was consumed for a record that never landed —
            // rewind it so the retry reuses it (no sequence gaps are
            // manufactured by an I/O failure).
            self.next_sequence -= 1;
            return Err(EdgeError::AuditWriteFailed {
                reason: error.to_string(),
            });
        }
        self.appended_total += 1;

        // Memory, then the audited overflow policy (the recorded P6.09
        // pattern: drop OLDEST, hard counter, latched flag, loud log).
        self.queue.push_back(record);
        if self.queue.len() > self.capacity {
            let dropped = self.queue.pop_front();
            self.dropped_oldest += 1;
            self.overflowed = true;
            tracing::warn!(
                journal = %self.path.display(),
                dropped_total = self.dropped_oldest,
                dropped_sequence = dropped.as_ref().map(|r| r.sequence),
                "AUDIT BUFFER OVERFLOW: the OLDEST audit record was dropped (audited \
                 drop-oldest policy, the P6.09 pattern); the loss is REPORTED — hard \
                 counter + latched health flag — never silent, never an in-place overwrite"
            );
            // The dropped-oldest frame is still on disk: compact so the
            // journal == the pending set (a compaction failure is logged
            // there and leaves the previous journal intact).
            self.compact_journal();
        }
        Ok(())
    }

    /// Rewrites the journal to EXACTLY the current pending set (the
    /// compaction used by `commit_uploaded` and by the overflow path).
    /// Temp-file + rename: a crash mid-compaction cannot destroy the
    /// previous journal; a failed compaction leaves everything pending.
    fn compact_journal(&mut self) {
        let temp_path = {
            let mut text = self.path.clone().into_os_string();
            text.push(".tmp");
            PathBuf::from(text)
        };
        let result = (|| -> std::io::Result<()> {
            let mut temp = std::fs::File::create(&temp_path)?;
            let mut out = Vec::with_capacity(64 + self.queue.len() * 64);
            out.extend_from_slice(JOURNAL_MAGIC);
            out.push(JOURNAL_VERSION);
            for record in &self.queue {
                let payload = wire::encode_record(record);
                out.extend_from_slice(&frame_bytes(&payload));
            }
            temp.write_all(&out)?;
            temp.sync_all()?;
            drop(temp);
            std::fs::rename(&temp_path, &self.path)?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                let reopened = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&self.path);
                match reopened {
                    Ok(file) => {
                        self.file = file;
                        self.file_len = {
                            let mut len = (JOURNAL_MAGIC.len() + 1) as u64;
                            for record in &self.queue {
                                len += (wire::encode_record(record).len() + 8) as u64;
                            }
                            len
                        };
                    }
                    Err(error) => {
                        tracing::error!(
                            journal = %self.path.display(),
                            error = %error,
                            "audit journal compaction succeeded but the journal could not be \
                             reopened for append; appends will fail closed until restart"
                        );
                    }
                }
            }
            Err(error) => {
                let _ = std::fs::remove_file(&temp_path);
                // Compaction failure is NOT a data event: the previous
                // journal is intact, the in-memory queue is the pending
                // truth, and the next commit retries.
                tracing::error!(
                    journal = %self.path.display(),
                    error = %error,
                    "audit journal compaction FAILED (previous journal intact; the next \
                     commit retries; all records remain pending on disk)"
                );
            }
        }
    }
}

/// The durable, journal-backed audit buffer (P6.18). See the module header
/// for the format, the upload semantics, the recovery and overflow
/// policies, and the encryption-at-rest deployment requirement.
pub struct FileAuditBuffer {
    inner: Mutex<Inner>,
}

impl FileAuditBuffer {
    /// Opens (or creates) the audit journal at `path`, replays it, and
    /// enforces the bound. `capacity` is the record bound (never
    /// unbounded). Sequences continue from the replayed journal's highest
    /// sequence, so upload ordering survives restarts.
    pub fn open(
        path: impl AsRef<Path>,
        capacity: usize,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Result<Self, FileAuditBufferError> {
        assert!(
            capacity > 0,
            "audit buffer capacity must be non-zero (bounded by construction)"
        );
        let path = path.as_ref().to_path_buf();
        if path.as_os_str().is_empty() {
            return Err(FileAuditBufferError::InvalidPath {
                reason: "the configured audit buffer path is empty".to_string(),
            });
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| FileAuditBufferError::Io {
                reason: format!("could not create the audit buffer directory: {error}"),
            })?;
        }

        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|error| FileAuditBufferError::Io {
                reason: format!("could not open the audit journal file: {error}"),
            })?;

        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(|error| {
            FileAuditBufferError::Io {
                reason: format!("could not read the audit journal file: {error}"),
            }
        })?;

        // Header: create or verify (never guess). A newly created file
        // gets the header WRITTEN immediately.
        let header_len = JOURNAL_MAGIC.len() + 1;
        if bytes.is_empty() {
            let header = {
                let mut header = Vec::with_capacity(header_len);
                header.extend_from_slice(JOURNAL_MAGIC);
                header.push(JOURNAL_VERSION);
                header
            };
            file.seek(SeekFrom::Start(0)).map_err(|error| {
                FileAuditBufferError::Io {
                    reason: format!("could not seek the new audit journal: {error}"),
                }
            })?;
            file.write_all(&header).map_err(|error| {
                FileAuditBufferError::Io {
                    reason: format!("could not write the audit journal header: {error}"),
                }
            })?;
            file.sync_all().map_err(|error| FileAuditBufferError::Io {
                reason: format!("could not sync the audit journal header: {error}"),
            })?;
            bytes = header;
        } else if bytes.len() < header_len
            || &bytes[..JOURNAL_MAGIC.len()] != JOURNAL_MAGIC
            || bytes[JOURNAL_MAGIC.len()] != JOURNAL_VERSION
        {
            return Err(FileAuditBufferError::NotAJournal {
                reason: format!(
                    "the file at {} does not carry the expected {}-byte header + version {}",
                    path.display(),
                    header_len,
                    JOURNAL_VERSION
                ),
            });
        }

        // Replay: decode frames until the FIRST corruption, then truncate
        // there (truncate-and-audit; never silent, never guessed past).
        let mut queue: VecDeque<BufferedAuditRecord> = VecDeque::new();
        let mut corrupt_tail_truncated: u64 = 0;
        let mut corruption_logged = false;
        let mut good_offset = header_len;
        let mut offset = header_len;
        while offset + 8 <= bytes.len() {
            let len = u32::from_le_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ]);
            let crc = u32::from_le_bytes([
                bytes[offset + 4],
                bytes[offset + 5],
                bytes[offset + 6],
                bytes[offset + 7],
            ]);
            if len == 0 || len > MAX_RECORD_BYTES {
                corrupt_tail_truncated += 1;
                corruption_logged = true;
                tracing::warn!(
                    journal = %path.display(),
                    offset,
                    "CORRUPT audit journal frame (bad length) — truncating at the last good \
                     record; the loss is audited, never silent"
                );
                break;
            }
            let end = offset + 8 + len as usize;
            if end > bytes.len() {
                corrupt_tail_truncated += 1;
                corruption_logged = true;
                tracing::warn!(
                    journal = %path.display(),
                    offset,
                    "TRUNCATED audit journal frame (declared length exceeds the file) — \
                     truncating at the last good record; the loss is audited, never silent"
                );
                break;
            }
            let payload = &bytes[offset + 8..end];
            if crc32(payload) != crc {
                corrupt_tail_truncated += 1;
                corruption_logged = true;
                tracing::warn!(
                    journal = %path.display(),
                    offset,
                    "CORRUPT audit journal frame (CRC mismatch) — truncating at the last good \
                     record; the loss is audited, never silent"
                );
                break;
            }
            match wire::decode_record(payload) {
                Ok(record) => queue.push_back(record),
                Err(error) => {
                    corrupt_tail_truncated += 1;
                    corruption_logged = true;
                    tracing::warn!(
                        journal = %path.display(),
                        offset,
                        error = %error,
                        "UNDECODABLE audit journal frame — truncating at the last good record; \
                         the loss is audited, never silent"
                    );
                    break;
                }
            }
            offset = end;
            good_offset = end;
        }

        if good_offset < bytes.len() {
            if !corruption_logged {
                corrupt_tail_truncated += 1;
                tracing::warn!(
                    journal = %path.display(),
                    offset = good_offset,
                    "STRAY trailing bytes in the audit journal (shorter than a frame header) — \
                     truncating at the last good record; the loss is audited, never silent"
                );
            }
            file.set_len(good_offset as u64).map_err(|error| {
                FileAuditBufferError::Io {
                    reason: format!("could not truncate the corrupt audit journal tail: {error}"),
                }
            })?;
            file.sync_all().map_err(|error| FileAuditBufferError::Io {
                reason: format!("could not sync the truncated audit journal: {error}"),
            })?;
        }

        // Persisted order: the next sequence continues from the replayed
        // journal's highest sequence (in-order upload across restarts).
        let highest_sequence = queue.back().map(|r| r.sequence).unwrap_or(0);
        let replayed = queue.len();
        // A replayed journal LARGER than the configured capacity drops
        // OLDEST — audited — not silently (the P6.09 pattern at load).
        let mut dropped_at_open: u64 = 0;
        while queue.len() > capacity {
            queue.pop_front();
            dropped_at_open += 1;
        }
        if dropped_at_open > 0 {
            tracing::warn!(
                journal = %path.display(),
                dropped = dropped_at_open,
                "AUDIT BUFFER OVERFLOW at open: the replayed journal exceeded the configured \
                 capacity; OLDEST records dropped (audited drop-oldest policy) — reported, \
                 never silent"
            );
        }

        let mut inner = Inner {
            path,
            capacity,
            clock,
            queue,
            file,
            file_len: good_offset.max(header_len) as u64,
            next_sequence: highest_sequence + 1,
            appended_total: replayed.saturating_sub(dropped_at_open as usize) as u64,
            dropped_oldest: dropped_at_open,
            corrupt_tail_truncated,
            overflowed: dropped_at_open > 0,
        };
        if dropped_at_open > 0 {
            inner.compact_journal();
        }

        Ok(Self {
            inner: Mutex::new(inner),
        })
    }

    // --- Audited counters and the health flag (the P6.17 query surface) ---

    /// The pending (not yet commit-uploaded) record count.
    pub fn pending_len(&self) -> usize {
        self.inner.lock().expect("poisoned audit buffer").queue.len()
    }

    /// The record bound (never unbounded).
    pub fn capacity(&self) -> usize {
        self.inner.lock().expect("poisoned audit buffer").capacity
    }

    /// How many OLDEST records the audited drop-of-oldest overflow policy
    /// removed (the explicit loss report).
    pub fn dropped_oldest(&self) -> u64 {
        self.inner.lock().expect("poisoned audit buffer").dropped_oldest
    }

    /// How many times a corrupt/truncated tail was cut at open.
    pub fn corrupt_tail_truncated(&self) -> u64 {
        self.inner
            .lock()
            .expect("poisoned audit buffer")
            .corrupt_tail_truncated
    }

    /// Total records appended through this buffer instance (including the
    /// replayed journal at open). Event-count reconciliation:
    /// `appended_total == committed_total + pending_len + dropped_oldest`
    /// within one open (per-open counters; a restart resets them — the
    /// recorded limitation).
    pub fn appended_total(&self) -> u64 {
        self.inner.lock().expect("poisoned audit buffer").appended_total
    }

    /// The latched overflow health flag: TRUE once the drop-oldest policy
    /// has ever fired on this buffer instance. Never inferred from silence
    /// — set only by an actual overflow, never cleared implicitly.
    pub fn has_overflowed(&self) -> bool {
        self.inner.lock().expect("poisoned audit buffer").overflowed
    }

    /// The journal path (diagnostics).
    pub fn journal_path(&self) -> PathBuf {
        self.inner.lock().expect("poisoned audit buffer").path.clone()
    }

    // --- Recording ----------------------------------------------------------

    /// Records one audit event with explicit actor/object (the P6.21
    /// denial-audit shape and the direct-caller path). The timestamp is
    /// stamped from the buffer's clock; the sequence is assigned here.
    /// A failure is a named `EdgeError` — audit is never silently skipped
    /// (the P4.28/P4.30 audit-or-refuse discipline, cited).
    pub fn record_event(
        &self,
        kind: BufferedAuditKind,
        actor: &str,
        object: &str,
        detail: String,
    ) -> Result<(), EdgeError> {
        if actor.trim().is_empty() || object.trim().is_empty() {
            // An audit record without actor or object is not an audit
            // record (the P6.21 requirement is actor+object+timestamp);
            // refuse rather than record a hole.
            return Err(EdgeError::AuditWriteFailed {
                reason: "audit record refused: actor and object are mandatory".to_string(),
            });
        }
        let recorded_at = {
            let inner = self.inner.lock().expect("poisoned audit buffer");
            inner.clock.now_ingest_time()
        };
        let record = BufferedAuditRecord {
            sequence: 0, // assigned by append_record
            recorded_at,
            kind,
            actor: actor.to_string(),
            object: object.to_string(),
            detail,
        };
        self.inner
            .lock()
            .expect("poisoned audit buffer")
            .append_record(record)
    }

    // --- Upload (at-least-once, in-order) ------------------------------------

    /// Removes up to `max` records from the FRONT (lowest sequence first)
    /// for upload. The journal is NOT touched here: between `drain` and
    /// [`FileAuditBuffer::commit_uploaded`] the records remain on disk, so
    /// a crash in that window re-delivers them after restart
    /// (at-least-once, the §15.4 semantics, same as the P6.09 buffer).
    ///
    /// IN-ORDER IS STRUCTURAL: records are always taken from the front, so
    /// no record is ever uploaded before its predecessors.
    pub fn drain(&self, max: usize) -> Vec<BufferedAuditRecord> {
        let mut inner = self.inner.lock().expect("poisoned audit buffer");
        inner.queue.drain(..max.min(inner.queue.len())).collect()
    }

    /// Puts a failed batch back at the FRONT, preserving order (the
    /// re-buffering path of a failed upload). The batch must be exactly
    /// what `drain` returned. The journal needs no change: the records
    /// were never removed from it.
    pub fn prepend(&self, records: Vec<BufferedAuditRecord>) {
        let mut inner = self.inner.lock().expect("poisoned audit buffer");
        for record in records.into_iter().rev() {
            inner.queue.push_front(record);
        }
    }

    /// The commit point: these drained records were CONFIRMED uploaded, so
    /// the journal compacts to the still-pending set. Before this call the
    /// records stay on disk — the at-least-once window.
    ///
    /// IN-ORDER COMMIT (fail-closed): the commit walks the caller's batch
    /// in order and removes ONLY while the journal's front matches the
    /// batch's next sequence. A mismatch (the batch does not start at the
    /// journal's front) is a named [`AuditUploadError::OutOfOrderCommit`]
    /// and removes NOTHING — an un-uploaded predecessor is never removed
    /// out of order, and no record is ever overwritten in place.
    pub fn commit_uploaded(
        &self,
        records: &[BufferedAuditRecord],
    ) -> Result<usize, AuditUploadError> {
        let mut inner = self.inner.lock().expect("poisoned audit buffer");
        if records.is_empty() {
            return Ok(0);
        }
        // Fail-closed front check: the batch must start at the journal's
        // front, else committing would remove an un-uploaded predecessor.
        match inner.queue.front() {
            Some(front) if front.sequence == records[0].sequence => {}
            Some(front) => {
                tracing::error!(
                    journal = %inner.path.display(),
                    batch_front = records[0].sequence,
                    journal_front = front.sequence,
                    "OUT-OF-ORDER audit upload commit REFUSED — no record was removed; \
                     an un-uploaded predecessor is never removed out of order"
                );
                return Err(AuditUploadError::OutOfOrderCommit {
                    expected_front_sequence: records[0].sequence,
                    found_front_sequence: Some(front.sequence),
                });
            }
            None => {
                return Err(AuditUploadError::OutOfOrderCommit {
                    expected_front_sequence: records[0].sequence,
                    found_front_sequence: None,
                });
            }
        }
        // Remove the matching PREFIX (in order only).
        let mut committed = 0usize;
        for record in records {
            match inner.queue.front() {
                Some(front) if front.sequence == record.sequence => {
                    inner.queue.pop_front();
                    committed += 1;
                }
                _ => break,
            }
        }
        inner.compact_journal();
        Ok(committed)
    }
}

// --- The AuditSink composition (P6.18 replaces the in-memory sink) -------------

impl AuditSink for FileAuditBuffer {
    /// Records one edge audit event durably (the composition path: the
    /// runner, the enrollment orchestrator, and the command receiver all
    /// record through this port). The [`EdgeAuditKind`] maps onto this
    /// module's closed vocabulary; the detail is never rewritten (the
    /// recorded `EdgeAuditKind` limitation: dedicated event names lead the
    /// detail string). Actor = the edge agent itself (runner-level events
    /// concern the agent); object = the event class — dedicated
    /// actor/object-bearing records use [`FileAuditBuffer::record_event`].
    fn record(&self, event: EdgeAuditEvent) -> Result<(), EdgeError> {
        let kind = BufferedAuditKind::map_edge_kind(event.kind);
        let record = BufferedAuditRecord {
            sequence: 0, // assigned by append_record
            recorded_at: event.recorded_at,
            kind,
            actor: "edge-agent".to_string(),
            object: kind.name().to_string(),
            detail: event.detail,
        };
        self.inner
            .lock()
            .expect("poisoned audit buffer")
            .append_record(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_ports::fakes::FixedClock;
    use std::sync::atomic::{AtomicU64, Ordering};

    const BASE_MILLIS: i64 = 1_700_000_000_000;

    /// A unique scratch directory per call (std-only; no tempfile dep).
    fn scratch_dir(label: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "flexcore-edge-p618-{}-{}-{}",
            std::process::id(),
            label,
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).expect("scratch dir creates");
        dir
    }

    fn cleanup(dir: &Path) {
        let _ = std::fs::remove_dir_all(dir);
    }

    fn clock() -> Arc<dyn Clock + Send + Sync> {
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS))
    }

    fn denial(buffer: &FileAuditBuffer, actor: &str, object: &str, detail: &str) {
        buffer
            .record_event(
                BufferedAuditKind::ScopeDenied,
                actor,
                object,
                detail.to_string(),
            )
            .expect("records");
    }

    #[test]
    fn records_round_trip_across_reopen_in_persisted_sequence_order() {
        let dir = scratch_dir("round-trip");
        let path = dir.join("audit.journal");
        {
            let buffer = FileAuditBuffer::open(&path, 100, clock()).expect("opens");
            denial(&buffer, "edge:site-A", "tenant:tenant-B", "EDGE_SCOPE_DENIED: telemetry request refused");
            denial(&buffer, "edge:site-A", "asset:ess-02", "EDGE_SCOPE_DENIED: asset enumeration refused");
        }

        let reopened = FileAuditBuffer::open(&path, 100, clock()).expect("reopens");
        assert_eq!(reopened.pending_len(), 2, "the audit buffer survives restart");
        assert_eq!(reopened.corrupt_tail_truncated(), 0);
        let drained = reopened.drain(10);
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].sequence, 1, "sequences persist across restarts");
        assert_eq!(drained[1].sequence, 2);
        assert_eq!(drained[0].kind, BufferedAuditKind::ScopeDenied);
        assert_eq!(drained[0].kind.name(), "EDGE_SCOPE_DENIED");
        assert_eq!(drained[0].actor, "edge:site-A");
        assert_eq!(drained[0].object, "tenant:tenant-B");
        assert_eq!(
            drained[0].recorded_at.unix_millis(),
            BASE_MILLIS as i128,
            "the recorded timestamp survives the round trip (never re-stamped)"
        );
        cleanup(&dir);
    }

    #[test]
    fn commit_uploaded_compacts_and_a_restart_holds_only_the_uncommitted() {
        let dir = scratch_dir("commit");
        let path = dir.join("audit.journal");
        let buffer = FileAuditBuffer::open(&path, 100, clock()).expect("opens");
        for n in 1..=4u64 {
            denial(&buffer, "edge:site-A", &format!("object-{n}"), "event");
        }
        // Upload batch A (seq 1–2): drain, upload OK, commit.
        let batch_a = buffer.drain(2);
        assert_eq!(batch_a.len(), 2);
        let committed = buffer.commit_uploaded(&batch_a).expect("commits");
        assert_eq!(committed, 2);
        // Upload batch B (seq 3–4): drain, upload FAILS, prepend back.
        let batch_b = buffer.drain(2);
        buffer.prepend(batch_b);

        // Restart: the journal holds the pending set only — batch A gone,
        // batch B back at the front, in order.
        drop(buffer);
        let reopened = FileAuditBuffer::open(&path, 100, clock()).expect("reopens");
        let pending = reopened.drain(10);
        assert_eq!(
            pending.iter().map(|r| r.sequence).collect::<Vec<_>>(),
            vec![3, 4],
            "committed records do not redeliver; the failed batch is retained in order"
        );
        cleanup(&dir);
    }

    #[test]
    fn an_out_of_order_commit_is_refused_and_removes_nothing() {
        let dir = scratch_dir("out-of-order");
        let path = dir.join("audit.journal");
        let buffer = FileAuditBuffer::open(&path, 100, clock()).expect("opens");
        for n in 1..=3u64 {
            denial(&buffer, "edge:site-A", &format!("object-{n}"), "event");
        }
        // A batch that does NOT start at the journal's front (a malicious
        // or buggy uploader skipping an un-uploaded predecessor).
        let all = buffer.drain(10);
        buffer.prepend(all);
        let bogus = vec![BufferedAuditRecord {
            sequence: 2,
            recorded_at: IngestTime::from_unix_millis(BASE_MILLIS).expect("valid"),
            kind: BufferedAuditKind::ScopeDenied,
            actor: "edge:site-A".to_string(),
            object: "object-2".to_string(),
            detail: "skipping seq 1".to_string(),
        }];
        let error = buffer.commit_uploaded(&bogus).expect_err("refused");
        assert!(matches!(
            error,
            AuditUploadError::OutOfOrderCommit { .. }
        ));
        // Nothing was removed: all three records are still pending, in order.
        assert_eq!(buffer.pending_len(), 3);
        let pending = buffer.drain(10);
        assert_eq!(
            pending.iter().map(|r| r.sequence).collect::<Vec<_>>(),
            vec![1, 2, 3],
            "an un-uploaded predecessor is never removed out of order"
        );
        cleanup(&dir);
    }

    #[test]
    fn overflow_drops_oldest_with_a_hard_counter_and_a_latched_flag() {
        let dir = scratch_dir("overflow");
        let path = dir.join("audit.journal");
        let buffer = FileAuditBuffer::open(&path, 2, clock()).expect("opens");
        for n in 1..=3u64 {
            denial(&buffer, "edge:site-A", &format!("object-{n}"), "event");
        }
        assert_eq!(buffer.pending_len(), 2);
        assert_eq!(buffer.dropped_oldest(), 1, "the drop is COUNTED");
        assert!(buffer.has_overflowed(), "the health flag LATCHES");
        // Reconciliation within one open: appended == pending + dropped.
        assert_eq!(
            buffer.appended_total(),
            (buffer.pending_len() + buffer.dropped_oldest() as usize) as u64,
            "the loss is explicitly reported by the counters"
        );
        let pending = buffer.drain(10);
        assert_eq!(
            pending.iter().map(|r| r.sequence).collect::<Vec<_>>(),
            vec![2, 3],
            "the OLDEST was dropped, order preserved — never a silent overwrite"
        );
        cleanup(&dir);
    }

    #[test]
    fn a_foreign_file_is_refused_not_guessed() {
        let dir = scratch_dir("foreign");
        let path = dir.join("audit.journal");
        std::fs::write(&path, b"not a flexcore audit journal").expect("writes");
        let error = FileAuditBuffer::open(&path, 10, clock()).expect_err("refused");
        assert!(matches!(error, FileAuditBufferError::NotAJournal { .. }));
        cleanup(&dir);
    }

    #[test]
    fn records_without_actor_or_object_are_refused() {
        let dir = scratch_dir("empty-actor");
        let path = dir.join("audit.journal");
        let buffer = FileAuditBuffer::open(&path, 10, clock()).expect("opens");
        let error = buffer
            .record_event(BufferedAuditKind::ScopeDenied, "  ", "object", "detail")
            .expect_err("an actorless audit record is refused");
        assert!(matches!(error, EdgeError::AuditWriteFailed { .. }));
        assert_eq!(buffer.pending_len(), 0, "nothing was recorded");
        cleanup(&dir);
    }

    #[test]
    fn the_audit_sink_composition_maps_edge_kinds_without_rewriting_details() {
        let dir = scratch_dir("sink");
        let path = dir.join("audit.journal");
        let buffer = FileAuditBuffer::open(&path, 100, clock()).expect("opens");
        let sink: &dyn AuditSink = &buffer;
        sink.record(EdgeAuditEvent {
            recorded_at: IngestTime::from_unix_millis(BASE_MILLIS).expect("valid"),
            kind: EdgeAuditKind::EnrollmentRejected,
            detail: "EDGE_ENROLLMENT_REJECTED: duplicate identity".to_string(),
        })
        .expect("records");
        sink.record(EdgeAuditEvent {
            recorded_at: IngestTime::from_unix_millis(BASE_MILLIS).expect("valid"),
            kind: EdgeAuditKind::DegradedBufferingEntered,
            detail: "uplink forward failed: channel down".to_string(),
        })
        .expect("records");
        let pending = buffer.drain(10);
        assert_eq!(pending[0].kind, BufferedAuditKind::EnrollmentRejected);
        assert_eq!(pending[1].kind, BufferedAuditKind::DegradedBufferingEntered);
        assert_eq!(
            pending[0].detail, "EDGE_ENROLLMENT_REJECTED: duplicate identity",
            "the detail is never rewritten (the dedicated name stays greppable)"
        );
        cleanup(&dir);
    }
}
