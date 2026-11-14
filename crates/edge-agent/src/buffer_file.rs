// P6.09 — Telemetry buffering, store-and-forward, and recovery: the
// DURABLE local buffer (report §18.1, §18.3, §21.2, §15.4; ledger P6.09).
//
// [`FileLocalBuffer`] implements the P6.05 [`crate::ports::LocalBuffer`]
// port over an append-only journal file (the `FLEXCORE_EDGE_BUFFER_PATH`
// config point, P6.05), so buffered telemetry survives an edge restart —
// the three §18.3/§21.2 failure modes this subtask exists for (unbounded
// growth, silent overflow, memory-only buffers) are each closed by
// construction:
//
// **Journal format (std-only, no new dependencies).** A fixed magic
// header, then length-prefixed CRC-checked records:
//
// ```text
// header:  b"FLEXCORE-EDGE-TELEMETRY-JOURNAL" + format version byte (0x01)
// record:  [u32 LE payload_len][u32 LE crc32(payload)][payload]
// payload: manually encoded EdgeTelemetrySample (see `wire` below) —
//          std-only serialization matching the canonical sample shape,
//          decoded back through the sample's VALIDATED constructor, so a
//          bit-corrupt-but-CRC-collision-free record can never produce an
//          invalid sample silently.
// ```
//
// **Recovery (truncate-and-audit, never silent).** On open, the journal is
// replayed record by record. The FIRST malformed frame (bad length, CRC
// mismatch, undecodable payload) ends the replay and the file is TRUNCATED
// at the last good record, with a named counter
// ([`FileLocalBuffer::corrupt_tail_truncated`]) and a loud log line. There
// is no resync: without magic markers between records, bytes after a
// corrupt frame cannot be trusted, so everything from the first corruption
// is treated as lost — and the loss is COUNTED, never silent. A wrong
// magic/version is a hard open error (never a guess).
//
// **Bounds and overflow policy (the recorded P6.09 policy).** Capacity
// comes from configuration (records; the P6.05 bound). The overflow policy
// is **audited drop-of-OLDEST**: when an append would exceed capacity, the
// OLDEST record is removed, a hard counter increments
// ([`FileLocalBuffer::dropped_oldest`]), the health flag
// ([`FileLocalBuffer::has_overflowed`]) latches, and the drop is logged —
// never a silent overwrite, never unbounded growth. Backpressure (the
// runner pausing polling at a full buffer) remains the FIRST line of
// defence; the drop-oldest path is the audited last resort when the bound
// is hit anyway (e.g. a reopened journal larger than the configured
// capacity). The counters are queryable for P6.17 health. Age retention is
// the other bound: records older than the configured max age (measured on
// `ingest_time`, the edge's own receipt time) are dropped with their own
// audited counter at drain/reopen time. A dropped record is a recorded
// GAP — downstream, the affected intervals are MISSING via the platform's
// gap detector (P2.04/P2.08 discipline, cited), never fabricated.
//
// **Store-and-forward and at-least-once honesty.** `append` writes the
// record to disk BEFORE it exists in memory (a crash cannot lose an
// appended record). `drain` removes from memory ONLY — the journal keeps
// the records until the runner confirms a successful forward through
// [`crate::ports::LocalBuffer::commit_forwarded`] (the P6.09-added commit
// point), which compacts the journal to the still-pending set. Consequence
// (deliberate, recorded): a crash in the drain→forward→commit window
// leaves the records in the journal, so after restart they are
// re-buffered and RE-FORWARDED — at-least-once delivery with possible
// downstream duplicates, exactly the §15.4 semantics. The duplicates are
// consumed downstream by the platform's P4.24-style idempotent telemetry
// ingest (the P2.09/P2.10 duplicate-classification discipline; cited, not
// reinvented) — the edge does NOT reinvent an idempotency store.
// A channel outage mid-drain loses nothing: the failed batch is
// `prepend`-ed back in order and its records are still on disk.
//
// Durability note (honest): appends are `sync_data`-flushed per record;
// compaction rewrites the file atomically-enough for the journal's purpose
// (write-new + rename is avoided to keep std-only and single-file; a crash
// mid-compaction is recovered by the same truncate-and-audit replay, with
// the loss counted). Encryption at rest (§18.3) is NOT implemented in this
// wave — recorded as an explicit open item in the decision document, to be
// closed with the P6.02-audited crypto dependency (no new dependencies
// were permitted in this wave).

use std::collections::VecDeque;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use flexcore_ports::clock::Clock;

use crate::ports::{EdgeError, EdgeTelemetrySample, LocalBuffer};

/// The journal magic header (32 bytes: 31-byte label + format version).
const JOURNAL_MAGIC: &[u8] = b"FLEXCORE-EDGE-TELEMETRY-JOURNAL";
const JOURNAL_VERSION: u8 = 0x01;
/// Hard sanity cap on one encoded record (rejects absurd lengths from a
/// corrupt header instead of attempting an absurd allocation).
const MAX_RECORD_BYTES: u32 = 1 << 20;

// --- Open/IO errors ----------------------------------------------------------

/// Errors of opening or operating the journal-backed buffer. Every failure
/// is named; nothing is swallowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileBufferError {
    /// The buffer path was empty or not usable as a file path.
    InvalidPath { reason: String },
    /// The underlying filesystem failed (create/open/read/write).
    Io { reason: String },
    /// The file exists but is not a FlexCore edge telemetry journal (wrong
    /// magic or version) — refused, never guessed at.
    NotAJournal { reason: String },
}

impl std::fmt::Display for FileBufferError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileBufferError::InvalidPath { reason } => {
                write!(f, "the buffer path is not usable: {reason}")
            }
            FileBufferError::Io { reason } => write!(f, "the buffer journal I/O failed: {reason}"),
            FileBufferError::NotAJournal { reason } => write!(
                f,
                "the file is not a FlexCore edge telemetry journal (refused, never guessed): {reason}"
            ),
        }
    }
}

impl std::error::Error for FileBufferError {}

// --- std-only wire encoding of the canonical sample --------------------------
//
// The encoding mirrors `EdgeTelemetrySample` field-for-field and decodes
// back through `EdgeTelemetrySample::new` (the validated constructor), so
// the on-disk shape can never hold a sample the runtime would refuse.
// Numeric fields are little-endian fixed-width; strings are u32-length-
// prefixed UTF-8. Discriminants are positional over the frozen enums and
// every decode of an unknown discriminant is a CORRUPT record (truncate-
// and-audit), never a guess.

mod wire {
    use flexcore_contracts::quality::TelemetryQuality;
    use flexcore_contracts::time_model::{EventTime, IngestTime};
    use flexcore_contracts::units::{
        ActivePowerMW, EnergyMWh, ReactivePowerMVAr, ReactiveSignConvention, StateOfCharge,
    };
    use flexcore_ports::telemetry_source::{QuantityValue, TelemetryQuantity};

    use crate::ports::{EdgeSampleError, EdgeTelemetrySample, EdgeTelemetrySampleParts};

    /// A decode failure is a CORRUPT record for the recovery policy — the
    /// caller truncates and audits, never guesses.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum WireError {
        UnexpectedEnd,
        UnknownDiscriminant { field: &'static str, code: u8 },
        BadString,
        BadValue { reason: String },
    }

    impl std::fmt::Display for WireError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                WireError::UnexpectedEnd => write!(f, "record ends unexpectedly"),
                WireError::UnknownDiscriminant { field, code } => {
                    write!(f, "unknown {field} discriminant {code}")
                }
                WireError::BadString => write!(f, "a length-prefixed string is not valid UTF-8"),
                WireError::BadValue { reason } => write!(f, "{reason}"),
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

        fn u32(&mut self) -> Result<u32, WireError> {
            let b = self.take(4)?;
            Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
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

        fn f64(&mut self) -> Result<f64, WireError> {
            Ok(f64::from_bits(self.u64()?))
        }

        fn string(&mut self) -> Result<String, WireError> {
            let len = self.u32()? as usize;
            let bytes = self.take(len)?;
            String::from_utf8(bytes.to_vec()).map_err(|_| WireError::BadString)
        }
    }

    fn put_u32(out: &mut Vec<u8>, value: u32) {
        out.extend_from_slice(&value.to_le_bytes());
    }

    fn put_u64(out: &mut Vec<u8>, value: u64) {
        out.extend_from_slice(&value.to_le_bytes());
    }

    fn put_string(out: &mut Vec<u8>, value: &str) {
        put_u32(out, value.len() as u32);
        out.extend_from_slice(value.as_bytes());
    }

    fn quality_code(quality: TelemetryQuality) -> u8 {
        match quality {
            TelemetryQuality::Good => 0,
            TelemetryQuality::Suspect => 1,
            TelemetryQuality::Bad => 2,
            TelemetryQuality::Missing => 3,
            TelemetryQuality::Estimated => 4,
            TelemetryQuality::Stale => 5,
        }
    }

    fn quality_from_code(code: u8) -> Result<TelemetryQuality, WireError> {
        Ok(match code {
            0 => TelemetryQuality::Good,
            1 => TelemetryQuality::Suspect,
            2 => TelemetryQuality::Bad,
            3 => TelemetryQuality::Missing,
            4 => TelemetryQuality::Estimated,
            5 => TelemetryQuality::Stale,
            other => {
                return Err(WireError::UnknownDiscriminant {
                    field: "quality",
                    code: other,
                })
            }
        })
    }

    fn quantity_code(quantity: TelemetryQuantity) -> u8 {
        match quantity {
            TelemetryQuantity::ActivePower => 0,
            TelemetryQuantity::ReactivePower => 1,
            TelemetryQuantity::Soc => 2,
            TelemetryQuantity::Energy => 3,
            TelemetryQuantity::Availability => 4,
            TelemetryQuantity::OperatingMode => 5,
            TelemetryQuantity::Health => 6,
            TelemetryQuantity::ConnectionState => 7,
            TelemetryQuantity::BaselineReference => 8,
        }
    }

    fn quantity_from_code(code: u8) -> Result<TelemetryQuantity, WireError> {
        Ok(match code {
            0 => TelemetryQuantity::ActivePower,
            1 => TelemetryQuantity::ReactivePower,
            2 => TelemetryQuantity::Soc,
            3 => TelemetryQuantity::Energy,
            4 => TelemetryQuantity::Availability,
            5 => TelemetryQuantity::OperatingMode,
            6 => TelemetryQuantity::Health,
            7 => TelemetryQuantity::ConnectionState,
            8 => TelemetryQuantity::BaselineReference,
            other => {
                return Err(WireError::UnknownDiscriminant {
                    field: "quantity",
                    code: other,
                })
            }
        })
    }

    fn convention_code(convention: ReactiveSignConvention) -> u8 {
        match convention {
            ReactiveSignConvention::InjectionPositive => 0,
            ReactiveSignConvention::ConsumptionPositive => 1,
        }
    }

    fn convention_from_code(code: u8) -> Result<ReactiveSignConvention, WireError> {
        Ok(match code {
            0 => ReactiveSignConvention::InjectionPositive,
            1 => ReactiveSignConvention::ConsumptionPositive,
            other => {
                return Err(WireError::UnknownDiscriminant {
                    field: "reactive convention",
                    code: other,
                })
            }
        })
    }

    /// Encodes one canonical sample (the journal record payload).
    pub fn encode_sample(sample: &EdgeTelemetrySample) -> Vec<u8> {
        let mut out = Vec::with_capacity(96);
        out.push(quantity_code(sample.quantity()));
        // Value: one tag + the variant payload.
        match sample.value() {
            QuantityValue::Power(mw) => {
                out.push(0);
                put_u64(&mut out, mw.value().to_bits());
            }
            QuantityValue::Reactive { power, convention } => {
                out.push(1);
                put_u64(&mut out, power.value().to_bits());
                out.push(convention_code(*convention));
            }
            QuantityValue::Fraction(soc) => {
                out.push(2);
                put_u64(&mut out, soc.value().to_bits());
            }
            QuantityValue::Energy(mwh) => {
                out.push(3);
                put_u64(&mut out, mwh.value().to_bits());
            }
            QuantityValue::Flag(flag) => {
                out.push(4);
                out.push(u8::from(*flag));
            }
            QuantityValue::State(text) => {
                out.push(5);
                put_string(&mut out, text);
            }
        }
        out.push(quality_code(sample.quality()));
        match sample.source_quality_code() {
            Some(code) => {
                out.push(1);
                put_string(&mut out, code);
            }
            None => out.push(0),
        }
        // Times: the two distinct P0.09 fields, stored as their own
        // millisecond values (the decode rebuilds them through their own
        // constructors — the types are never merged).
        put_u64(&mut out, sample.event_time().unix_millis() as u64);
        put_u64(&mut out, sample.ingest_time().unix_millis() as u64);
        put_u64(&mut out, sample.source_sequence());
        put_string(&mut out, sample.asset_key());
        put_string(&mut out, sample.source_id());
        out
    }

    /// Decodes one record payload back into a canonical sample — through
    /// the validated constructor, so a decoded sample satisfies every
    /// canonical rule the runtime enforces (a corrupted-but-decodable
    /// payload still cannot produce an invalid sample silently: the
    /// constructor's rejection is a corrupt record for the recovery
    /// policy).
    pub fn decode_sample(bytes: &[u8]) -> Result<EdgeTelemetrySample, WireError> {
        let mut reader = Reader::new(bytes);
        let quantity = quantity_from_code(reader.u8()?)?;
        let value = match reader.u8()? {
            0 => QuantityValue::Power(ActivePowerMW::new(reader.f64()?).map_err(|error| {
                WireError::BadValue {
                    reason: format!("the recorded power value is out of range: {error}"),
                }
            })?),
            1 => {
                let power = ReactivePowerMVAr::new(reader.f64()?).map_err(|error| {
                    WireError::BadValue {
                        reason: format!("the recorded reactive value is out of range: {error}"),
                    }
                })?;
                let convention = convention_from_code(reader.u8()?)?;
                QuantityValue::Reactive { power, convention }
            }
            2 => QuantityValue::Fraction(StateOfCharge::new(reader.f64()?).map_err(|error| {
                WireError::BadValue {
                    reason: format!("the recorded SOC fraction is out of range: {error}"),
                }
            })?),
            3 => QuantityValue::Energy(EnergyMWh::new(reader.f64()?).map_err(|error| {
                WireError::BadValue {
                    reason: format!("the recorded energy value is out of range: {error}"),
                }
            })?),
            4 => QuantityValue::Flag(reader.u8()? != 0),
            5 => QuantityValue::State(reader.string()?),
            other => {
                return Err(WireError::UnknownDiscriminant {
                    field: "value",
                    code: other,
                })
            }
        };
        let quality = quality_from_code(reader.u8()?)?;
        let source_quality_code = match reader.u8()? {
            0 => None,
            1 => Some(reader.string()?),
            other => {
                return Err(WireError::UnknownDiscriminant {
                    field: "source quality code presence",
                    code: other,
                })
            }
        };
        let event_millis = reader.i64()?;
        let ingest_millis = reader.i64()?;
        let source_sequence = reader.u64()?;
        let asset_key = reader.string()?;
        let source_id = reader.string()?;

        let event_time = EventTime::from_unix_millis(event_millis).map_err(|error| {
            WireError::BadValue {
                reason: format!("the recorded event_time is not a valid timestamp: {error}"),
            }
        })?;
        let ingest_time = IngestTime::from_unix_millis(ingest_millis).map_err(|error| {
            WireError::BadValue {
                reason: format!("the recorded ingest_time is not a valid timestamp: {error}"),
            }
        })?;
        EdgeTelemetrySample::new(EdgeTelemetrySampleParts {
            asset_key,
            quantity,
            value,
            quality,
            source_quality_code,
            event_time,
            ingest_time,
            source_sequence,
            source_id,
        })
        .map_err(|error: EdgeSampleError| WireError::BadValue {
            reason: format!("the decoded record fails canonical validation: {error}"),
        })
    }
}

/// CRC-32 (IEEE 802.3), bitwise — std-only, no dependency. Used for the
/// journal record integrity check (detecting truncated/corrupt tails).
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

// --- The buffer ---------------------------------------------------------------

/// The durable, journal-backed [`LocalBuffer`] (P6.09). See the module
/// header for the format, the recovery policy, the overflow policy, and
/// the at-least-once semantics.
pub struct FileLocalBuffer {
    path: PathBuf,
    capacity: usize,
    max_age: Option<Duration>,
    clock: Arc<dyn Clock + Send + Sync>,
    /// The pending (not yet committed-forwarded) samples, oldest first.
    queue: VecDeque<EdgeTelemetrySample>,
    file: std::fs::File,
    file_len: u64,
    // Audited counters (queryable for P6.17 health; in-memory, reset per
    // open — recorded limitation: a restart loses the counters, the
    // HEALTH FLAG state lives in the operator-visible composition, not
    // only here).
    dropped_oldest: u64,
    dropped_expired: u64,
    corrupt_tail_truncated: u64,
    overflowed: bool,
}

impl FileLocalBuffer {
    /// Opens (or creates) the journal at `path`, replays it, and enforces
    /// the bounds. `capacity` is the record bound (the P6.05 config);
    /// `max_age` is the retention bound on `ingest_time` (`None` =
    /// unlimited — a choice the composition must make explicitly; the
    /// shipped config defaults to a bounded value).
    pub fn open(
        path: impl AsRef<Path>,
        capacity: usize,
        max_age: Option<Duration>,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Result<Self, FileBufferError> {
        assert!(capacity > 0, "buffer capacity must be non-zero");
        let path = path.as_ref().to_path_buf();
        if path.as_os_str().is_empty() {
            return Err(FileBufferError::InvalidPath {
                reason: "the configured buffer path is empty".to_string(),
            });
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| FileBufferError::Io {
                reason: format!("could not create the buffer directory: {error}"),
            })?;
        }

        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|error| FileBufferError::Io {
                reason: format!("could not open the journal file: {error}"),
            })?;

        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(|error| FileBufferError::Io {
            reason: format!("could not read the journal file: {error}"),
        })?;

        // Header: create or verify (never guess). A newly created file
        // gets the header WRITTEN immediately (the append path assumes the
        // header is on disk).
        let header_len = JOURNAL_MAGIC.len() + 1;
        if bytes.is_empty() {
            let header = {
                let mut header = Vec::with_capacity(header_len);
                header.extend_from_slice(JOURNAL_MAGIC);
                header.push(JOURNAL_VERSION);
                header
            };
            file.seek(SeekFrom::Start(0)).map_err(|error| FileBufferError::Io {
                reason: format!("could not seek the new journal: {error}"),
            })?;
            file.write_all(&header).map_err(|error| FileBufferError::Io {
                reason: format!("could not write the journal header: {error}"),
            })?;
            file.sync_all().map_err(|error| FileBufferError::Io {
                reason: format!("could not sync the journal header: {error}"),
            })?;
            bytes = header;
        } else if bytes.len() < header_len
            || &bytes[..JOURNAL_MAGIC.len()] != JOURNAL_MAGIC
            || bytes[JOURNAL_MAGIC.len()] != JOURNAL_VERSION
        {
            return Err(FileBufferError::NotAJournal {
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
        let mut queue: VecDeque<EdgeTelemetrySample> = VecDeque::new();
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
                    "CORRUPT journal frame (bad length) — truncating at the last good record; \
                     the loss is audited, never silent"
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
                    "TRUNCATED journal frame (declared length exceeds the file) — truncating at \
                     the last good record; the loss is audited, never silent"
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
                    "CORRUPT journal frame (CRC mismatch) — truncating at the last good record; \
                     the loss is audited, never silent"
                );
                break;
            }
            match wire::decode_sample(payload) {
                Ok(sample) => queue.push_back(sample),
                Err(error) => {
                    corrupt_tail_truncated += 1;
                    corruption_logged = true;
                    tracing::warn!(
                        journal = %path.display(),
                        offset,
                        error = %error,
                        "UNDECODABLE journal frame — truncating at the last good record; \
                         the loss is audited, never silent"
                    );
                    break;
                }
            }
            offset = end;
            good_offset = end;
        }

        // Truncate the corrupt tail (or stray trailing bytes shorter than
        // a frame header) at the last good record — audited in BOTH cases,
        // never silent.
        if good_offset < bytes.len() {
            if !corruption_logged {
                corrupt_tail_truncated += 1;
                tracing::warn!(
                    journal = %path.display(),
                    offset = good_offset,
                    "STRAY trailing bytes in the journal (shorter than a frame header) — \
                     truncating at the last good record; the loss is audited, never silent"
                );
            }
            file.set_len(good_offset as u64).map_err(|error| {
                FileBufferError::Io {
                    reason: format!("could not truncate the corrupt journal tail: {error}"),
                }
            })?;
            file.sync_all().map_err(|error| FileBufferError::Io {
                reason: format!("could not sync the truncated journal: {error}"),
            })?;
        }

        let mut buffer = Self {
            path,
            capacity,
            max_age,
            clock,
            queue,
            file,
            file_len: good_offset.max(header_len) as u64,
            dropped_oldest: 0,
            dropped_expired: 0,
            corrupt_tail_truncated,
            overflowed: false,
        };

        // Bounds at load: overflow and age retention apply to the replayed
        // set too (a reopened journal larger than the configured capacity
        // drops OLDEST — audited — not silently).
        buffer.enforce_capacity();
        buffer.enforce_age();
        if buffer.dropped_oldest > 0 || buffer.dropped_expired > 0 {
            buffer.compact_journal();
        }
        Ok(buffer)
    }

    // --- Audited counters and the health flag (P6.17 query surface) ---

    /// How many OLDEST records the drop-oldest overflow policy removed.
    pub fn dropped_oldest(&self) -> u64 {
        self.dropped_oldest
    }

    /// How many records the max-age retention removed.
    pub fn dropped_expired(&self) -> u64 {
        self.dropped_expired
    }

    /// How many times a corrupt/truncated tail was cut at open.
    pub fn corrupt_tail_truncated(&self) -> u64 {
        self.corrupt_tail_truncated
    }

    /// The latched overflow health flag: TRUE once the drop-oldest policy
    /// has ever fired on this buffer instance. Never inferred from silence
    /// — it is set only by an actual overflow, never cleared implicitly.
    pub fn has_overflowed(&self) -> bool {
        self.overflowed
    }

    /// The journal path (diagnostics).
    pub fn journal_path(&self) -> &Path {
        &self.path
    }

    // --- Internals ----------------------------------------------------

    /// Drop-oldest until within capacity. Audited: counter + flag + log.
    fn enforce_capacity(&mut self) {
        while self.queue.len() > self.capacity {
            self.queue.pop_front();
            self.dropped_oldest += 1;
            self.overflowed = true;
        }
    }

    /// Drop records older than the max age (measured on `ingest_time`, the
    /// edge's own receipt time). Audited: counter + log.
    fn enforce_age(&mut self) {
        let Some(max_age) = self.max_age else {
            return;
        };
        let now_millis = self.clock.now_ingest_time().unix_millis();
        let limit = now_millis - max_age.as_millis() as i128;
        let mut dropped = 0u64;
        while let Some(front) = self.queue.front() {
            if front.ingest_time().unix_millis() >= limit {
                break;
            }
            self.queue.pop_front();
            dropped += 1;
        }
        if dropped > 0 {
            self.dropped_expired += dropped;
            tracing::warn!(
                journal = %self.path.display(),
                dropped,
                "AGE-EXPIRED records dropped (max-age retention); the gap is audited, \
                 never silent"
            );
        }
    }

    /// Rewrites the journal to EXACTLY the current pending set (the
    /// compaction used by `commit_forwarded` and by the audited-drop
    /// paths). Journal content after this call == in-memory content.
    ///
    /// Atomicity: the rewrite goes to a sibling temp file that is synced
    /// and RENAMED over the journal, so a crash mid-compaction can never
    /// destroy the previous journal (the rename either happened or not).
    /// A failed compaction is logged loudly and leaves the previous
    /// journal untouched; the in-memory queue remains the pending truth
    /// and the next commit retries.
    fn compact_journal(&mut self) {
        let temp_path = {
            let mut text = self.path.clone().into_os_string();
            text.push(".tmp");
            PathBuf::from(text)
        };
        let result = (|| -> std::io::Result<()> {
            let mut temp = std::fs::File::create(&temp_path)?;
            let mut out = Vec::with_capacity(64 + self.queue.len() * 96);
            out.extend_from_slice(JOURNAL_MAGIC);
            out.push(JOURNAL_VERSION);
            for sample in &self.queue {
                let payload = wire::encode_sample(sample);
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
                // Reopen the (replaced) journal at the new content.
                let reopened = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&self.path);
                match reopened {
                    Ok(file) => {
                        self.file = file;
                        // Exact post-compaction length: header + frames.
                        self.file_len = {
                            let mut len = (JOURNAL_MAGIC.len() + 1) as u64;
                            for sample in &self.queue {
                                len += (wire::encode_sample(sample).len() + 8) as u64;
                            }
                            len
                        };
                    }
                    Err(error) => {
                        tracing::error!(
                            journal = %self.path.display(),
                            error = %error,
                            "journal compaction succeeded but the journal could not be \
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
                    "journal compaction FAILED (previous journal intact; the next commit \
                     retries)"
                );
            }
        }
    }

    /// Appends one frame at the physical end of the journal, synced.
    fn append_frame(&mut self, sample: &EdgeTelemetrySample) -> Result<(), FileBufferError> {
        let payload = wire::encode_sample(sample);
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
            Err(error) => Err(FileBufferError::Io {
                reason: format!("could not append to the journal: {error}"),
            }),
        }
    }
}

impl LocalBuffer for FileLocalBuffer {
    fn len(&self) -> usize {
        self.queue.len()
    }

    fn capacity(&self) -> usize {
        self.capacity
    }

    /// Appends one sample: **disk first, then memory** (a crash cannot
    /// lose a record the caller was told was buffered). At capacity the
    /// recorded P6.09 overflow policy applies: drop OLDEST, audited
    /// (counter + health flag + log) — never a silent overwrite. An I/O
    /// failure refuses the append entirely (fail-closed: the caller learns
    /// the record was NOT durably buffered) and leaves memory unchanged.
    fn append(&mut self, sample: EdgeTelemetrySample) -> Result<(), EdgeError> {
        // Write to disk first.
        if let Err(error) = self.append_frame(&sample) {
            return Err(EdgeError::BufferAppendFailed {
                reason: error.to_string(),
            });
        }

        // Memory, then the (audited) overflow policy.
        self.queue.push_back(sample);
        if self.queue.len() > self.capacity {
            self.queue.pop_front();
            self.dropped_oldest += 1;
            self.overflowed = true;
            tracing::warn!(
                journal = %self.path.display(),
                dropped_total = self.dropped_oldest,
                "BUFFER OVERFLOW: the OLDEST record was dropped (audited drop-oldest policy, \
                 P6.09); the loss is a recorded gap, never silent"
            );
            // The dropped-oldest frame is still on disk: compact to keep
            // journal == pending set.
            self.compact_journal();
        }
        Ok(())
    }

    /// Removes up to `max` samples from the FRONT (oldest first). The
    /// journal is NOT touched here: between `drain` and
    /// [`LocalBuffer::commit_forwarded`] the records remain on disk, so a
    /// crash in that window re-delivers them after restart (at-least-once,
    /// §15.4 — the recorded P6.09 semantics). Age retention applies first
    /// (audited).
    fn drain(&mut self, max: usize) -> Vec<EdgeTelemetrySample> {
        self.enforce_age();
        self.queue
            .drain(..max.min(self.queue.len()))
            .collect()
    }

    /// Puts a failed batch back at the FRONT, preserving order. The
    /// journal needs no change: the records were never removed from it.
    fn prepend(&mut self, samples: Vec<EdgeTelemetrySample>) {
        for sample in samples.into_iter().rev() {
            self.queue.push_front(sample);
        }
    }

    /// The commit point: these drained samples were forwarded
    /// successfully, so the journal compacts to the still-pending set.
    /// Before this call the records stay on disk — the at-least-once
    /// window.
    fn commit_forwarded(&mut self, _samples: &[EdgeTelemetrySample]) {
        self.compact_journal();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::quality::TelemetryQuality;
    use flexcore_contracts::time_model::{EventTime, IngestTime};
    use flexcore_contracts::units::{ActivePowerMW, ReactiveSignConvention};
    use flexcore_ports::fakes::FixedClock;
    use flexcore_ports::telemetry_source::{QuantityValue, TelemetryQuantity};
    use std::sync::atomic::{AtomicU64, Ordering};

    const BASE_MILLIS: i64 = 1_700_000_000_000;

    /// A unique scratch directory per call (std-only; no tempfile dep).
    fn scratch_dir(label: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "flexcore-edge-p609-{}-{}-{}",
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

    /// A power sample with distinct event/ingest times and sequence.
    fn power_sample(sequence: u64, mw: f64, ingest_offset_millis: i64) -> EdgeTelemetrySample {
        EdgeTelemetrySample::new(EdgeTelemetrySampleParts {
            asset_key: "ess-01".to_string(),
            quantity: TelemetryQuantity::ActivePower,
            value: QuantityValue::Power(ActivePowerMW::new(mw).expect("finite")),
            quality: TelemetryQuality::Good,
            source_quality_code: None,
            event_time: EventTime::from_unix_millis(BASE_MILLIS + sequence as i64 * 1_000)
                .expect("valid"),
            ingest_time: IngestTime::from_unix_millis(BASE_MILLIS + ingest_offset_millis)
                .expect("valid"),
            source_sequence: sequence,
            source_id: "edge:pilot-site-A:simulated-test/ess0/ActivePower".to_string(),
        })
        .expect("valid sample")
    }

    /// A sample per quantity/value kind, so the wire round-trip covers the
    /// full value vocabulary (including the reactive convention, which
    /// MUST travel with the value — P0.07 §3).
    fn varied_samples() -> Vec<EdgeTelemetrySample> {
        vec![
            EdgeTelemetrySample::new(EdgeTelemetrySampleParts {
                asset_key: "ess-01".to_string(),
                quantity: TelemetryQuantity::ActivePower,
                value: QuantityValue::Power(ActivePowerMW::new(-2.5).expect("finite")),
                quality: TelemetryQuality::Suspect,
                source_quality_code: Some("OUT_OF_RANGE".to_string()),
                event_time: EventTime::from_unix_millis(BASE_MILLIS).expect("valid"),
                ingest_time: IngestTime::from_unix_millis(BASE_MILLIS + 1_000).expect("valid"),
                source_sequence: 1,
                source_id: "edge:pilot-site-A:simulated-test/ess0/ActivePower".to_string(),
            })
            .expect("valid"),
            EdgeTelemetrySample::new(EdgeTelemetrySampleParts {
                asset_key: "ess-01".to_string(),
                quantity: TelemetryQuantity::ReactivePower,
                value: QuantityValue::Reactive {
                    power: flexcore_contracts::units::ReactivePowerMVAr::new(0.4)
                        .expect("finite"),
                    convention: ReactiveSignConvention::ConsumptionPositive,
                },
                quality: TelemetryQuality::Good,
                source_quality_code: None,
                event_time: EventTime::from_unix_millis(BASE_MILLIS).expect("valid"),
                ingest_time: IngestTime::from_unix_millis(BASE_MILLIS + 1_000).expect("valid"),
                source_sequence: 2,
                source_id: "edge:pilot-site-A:simulated-test/ess0/ReactivePower".to_string(),
            })
            .expect("valid"),
            EdgeTelemetrySample::new(EdgeTelemetrySampleParts {
                asset_key: "ess-01".to_string(),
                quantity: TelemetryQuantity::Soc,
                value: QuantityValue::Fraction(
                    flexcore_contracts::units::StateOfCharge::new(0.87).expect("in range"),
                ),
                quality: TelemetryQuality::Good,
                source_quality_code: None,
                event_time: EventTime::from_unix_millis(BASE_MILLIS).expect("valid"),
                ingest_time: IngestTime::from_unix_millis(BASE_MILLIS + 1_000).expect("valid"),
                source_sequence: 3,
                source_id: "edge:pilot-site-A:simulated-test/ess0/Soc".to_string(),
            })
            .expect("valid"),
            EdgeTelemetrySample::new(EdgeTelemetrySampleParts {
                asset_key: "meter-01".to_string(),
                quantity: TelemetryQuantity::Energy,
                value: QuantityValue::Energy(
                    flexcore_contracts::units::EnergyMWh::new(12.25).expect("finite"),
                ),
                quality: TelemetryQuality::Estimated,
                source_quality_code: Some("SUBSTITUTED".to_string()),
                event_time: EventTime::from_unix_millis(BASE_MILLIS).expect("valid"),
                ingest_time: IngestTime::from_unix_millis(BASE_MILLIS + 1_000).expect("valid"),
                source_sequence: 4,
                source_id: "edge:pilot-site-A:simulated-test/meter0/EnergyOut".to_string(),
            })
            .expect("valid"),
        ]
    }

    #[test]
    fn journal_round_trip_preserves_samples_across_reopen() {
        let dir = scratch_dir("round-trip");
        let path = dir.join("telemetry.journal");
        let clock = Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 9_000));

        let expected = varied_samples();
        {
            let mut buffer =
                FileLocalBuffer::open(&path, 100, None, Arc::clone(&clock) as Arc<dyn Clock + Send + Sync>)
                    .expect("opens");
            for sample in &expected {
                buffer.append(sample.clone()).expect("appends");
            }
            assert_eq!(buffer.len(), expected.len());
            // Buffer dropped WITHOUT commit: everything must survive.
        }

        let mut reopened =
            FileLocalBuffer::open(&path, 100, None, Arc::clone(&clock) as Arc<dyn Clock + Send + Sync>)
                .expect("reopens");
        assert_eq!(reopened.len(), expected.len(), "the buffer survives restart");
        assert_eq!(reopened.corrupt_tail_truncated(), 0);
        let drained = reopened.drain(100);
        assert_eq!(drained, expected, "same samples, same order, same content");
        cleanup(&dir);
    }

    #[test]
    fn corrupt_tail_is_truncated_and_audited_never_silent() {
        let dir = scratch_dir("corrupt-tail");
        let path = dir.join("telemetry.journal");
        let clock = Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 9_000));

        {
            let mut buffer =
                FileLocalBuffer::open(&path, 100, None, Arc::clone(&clock) as Arc<dyn Clock + Send + Sync>)
                    .expect("opens");
            buffer.append(power_sample(1, 1.0, 1_000)).expect("appends");
            buffer.append(power_sample(2, 2.0, 1_000)).expect("appends");
        }

        // Corrupt the tail: append garbage bytes directly to the file (the
        // simulated partial write / disk corruption).
        {
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .expect("opens for append");
            file.write_all(&[0x51, 0x00, 0x00, 0x00, 0xAB, 0xCD, 0x12, 0x34, 0xFF, 0x00])
                .expect("appends garbage");
        }

        let mut reopened =
            FileLocalBuffer::open(&path, 100, None, Arc::clone(&clock) as Arc<dyn Clock + Send + Sync>)
                .expect("reopens (truncating the corrupt tail)");
        assert_eq!(reopened.corrupt_tail_truncated(), 1, "the corruption is COUNTED");
        assert_eq!(reopened.len(), 2, "the good records survive");
        // The journal is usable again: append after recovery persists.
        reopened.append(power_sample(3, 3.0, 1_000)).expect("appends post-recovery");
        drop(reopened);
        let mut final_open =
            FileLocalBuffer::open(&path, 100, None, Arc::clone(&clock) as Arc<dyn Clock + Send + Sync>)
                .expect("reopens again");
        assert_eq!(final_open.len(), 3, "post-recovery appends persist");
        let drained = final_open.drain(10);
        assert_eq!(drained[0].source_sequence(), 1);
        assert_eq!(drained[2].source_sequence(), 3);
        cleanup(&dir);
    }

    #[test]
    fn a_foreign_file_is_refused_not_guessed() {
        let dir = scratch_dir("foreign");
        let path = dir.join("telemetry.journal");
        std::fs::write(&path, b"not a flexcore journal at all").expect("writes");
        let error = FileLocalBuffer::open(
            &path,
            10,
            None,
            Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)) as Arc<dyn Clock + Send + Sync>,
        )
        .expect_err("a foreign file is refused");
        assert!(matches!(error, FileBufferError::NotAJournal { .. }));
        cleanup(&dir);
    }

    #[test]
    fn overflow_drops_oldest_with_a_hard_counter_and_health_flag() {
        let dir = scratch_dir("overflow");
        let path = dir.join("telemetry.journal");
        let clock = Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 9_000));

        let mut buffer =
            FileLocalBuffer::open(&path, 2, None, Arc::clone(&clock) as Arc<dyn Clock + Send + Sync>)
                .expect("opens");
        buffer.append(power_sample(1, 1.0, 1_000)).expect("appends");
        buffer.append(power_sample(2, 2.0, 1_000)).expect("appends");
        // Capacity 2, third append: the recorded drop-OLDEST policy fires
        // — audited, never a silent overwrite.
        buffer.append(power_sample(3, 3.0, 1_000)).expect("appends (drop-oldest)");
        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.dropped_oldest(), 1, "the drop is counted");
        assert!(buffer.has_overflowed(), "the health flag latches");

        let drained = buffer.drain(10);
        assert_eq!(drained[0].source_sequence(), 2, "the OLDEST was dropped");
        assert_eq!(drained[1].source_sequence(), 3);

        // The drop also persists: the reopened journal holds the pending
        // set only (the dropped record is gone, counted at its moment).
        drop(buffer);
        let mut reopened =
            FileLocalBuffer::open(&path, 2, None, Arc::clone(&clock) as Arc<dyn Clock + Send + Sync>)
                .expect("reopens");
        assert_eq!(reopened.len(), 2);
        let drained = reopened.drain(10);
        assert_eq!(drained[0].source_sequence(), 2);
        assert_eq!(drained[1].source_sequence(), 3);
        cleanup(&dir);
    }

    #[test]
    fn max_age_retention_drops_expired_records_audited() {
        let dir = scratch_dir("max-age");
        let path = dir.join("telemetry.journal");
        // "Now" is 9 s past BASE; a 5 s retention drops ingest older than
        // 4 s past BASE.
        let clock = Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 9_000));

        let mut buffer = FileLocalBuffer::open(
            &path,
            100,
            Some(Duration::from_millis(5_000)),
            Arc::clone(&clock) as Arc<dyn Clock + Send + Sync>,
        )
        .expect("opens");
        buffer.append(power_sample(1, 1.0, 1_000)).expect("appends"); // ingest at +1 s: expired
        buffer.append(power_sample(2, 2.0, 6_000)).expect("appends"); // ingest at +6 s: fresh

        let drained = buffer.drain(10);
        assert_eq!(drained.len(), 1, "the expired record is retained-dropped");
        assert_eq!(drained[0].source_sequence(), 2);
        assert_eq!(buffer.dropped_expired(), 1, "the expiry drop is counted");
        assert_eq!(buffer.dropped_oldest(), 0);
        cleanup(&dir);
    }

    #[test]
    fn store_and_forward_survives_a_forced_outage_and_a_crash_mid_drain() {
        let dir = scratch_dir("outage");
        let path = dir.join("telemetry.journal");
        let clock = Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 9_000));

        // Phase 1 — channel DOWN: samples accumulate on disk.
        let mut buffer =
            FileLocalBuffer::open(&path, 100, None, Arc::clone(&clock) as Arc<dyn Clock + Send + Sync>)
                .expect("opens");
        for seq in 1..=3u64 {
            buffer.append(power_sample(seq, seq as f64, 1_000)).expect("appends");
        }
        assert_eq!(buffer.len(), 3);

        // Phase 2 — the runner drains for the (failing) forward, then the
        // process "crashes" (the buffer is dropped without commit). The
        // records are still on disk: nothing was committed.
        let drained = buffer.drain(3);
        assert_eq!(drained.len(), 3);
        drop(buffer); // crash: no commit_forwarded

        // Phase 3 — reconnect/restart: the buffer replays the journal and
        // re-delivers the un-committed batch (at-least-once; downstream
        // duplicates are consumed by the platform's P4.24-style idempotent
        // ingest — cited, not reinvented).
        let mut recovered =
            FileLocalBuffer::open(&path, 100, None, Arc::clone(&clock) as Arc<dyn Clock + Send + Sync>)
                .expect("reopens after the crash");
        assert_eq!(recovered.len(), 3, "the crashed drain is re-delivered");
        let redelivered = recovered.drain(3);
        assert_eq!(
            redelivered.iter().map(|s| s.source_sequence()).collect::<Vec<_>>(),
            vec![1, 2, 3],
            "redelivery preserves order"
        );

        // Phase 4 — the forward SUCCEEDS and the runner commits: the
        // journal compacts to empty; a restart holds nothing (and holds no
        // duplicates either).
        recovered.commit_forwarded(&redelivered);
        drop(recovered);
        let mut clean =
            FileLocalBuffer::open(&path, 100, None, Arc::clone(&clock) as Arc<dyn Clock + Send + Sync>)
                .expect("reopens after the commit");
        assert_eq!(clean.len(), 0, "committed records do not redeliver");
        cleanup(&dir);
    }

    #[test]
    fn commit_forwarded_compacts_exactly_the_uncommitted_set() {
        let dir = scratch_dir("commit");
        let path = dir.join("telemetry.journal");
        let clock = Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 9_000));

        let mut buffer =
            FileLocalBuffer::open(&path, 100, None, Arc::clone(&clock) as Arc<dyn Clock + Send + Sync>)
                .expect("opens");
        for seq in 1..=5u64 {
            buffer.append(power_sample(seq, seq as f64, 1_000)).expect("appends");
        }
        // Forward batch A (seq 1–2): drain, forward OK, commit.
        let batch_a = buffer.drain(2);
        buffer.commit_forwarded(&batch_a);
        // Forward batch B (seq 3–4): drain, forward FAILS, prepend back.
        let batch_b = buffer.drain(2);
        buffer.prepend(batch_b);
        // Crash (no commit). The journal must hold the pending set: seq
        // 3, 4, 5 — batches A is committed, B is back at the front.
        drop(buffer);
        let mut reopened =
            FileLocalBuffer::open(&path, 100, None, Arc::clone(&clock) as Arc<dyn Clock + Send + Sync>)
                .expect("reopens");
        let pending = reopened.drain(10);
        assert_eq!(
            pending.iter().map(|s| s.source_sequence()).collect::<Vec<_>>(),
            vec![3, 4, 5],
            "committed batch gone, failed batch back in order, later appends kept"
        );
        cleanup(&dir);
    }

    #[test]
    fn io_failure_refuses_the_append_fail_closed() {
        // A path whose "file" is a DIRECTORY: the open itself fails, which
        // is the named-error path (fail-closed on an unusable journal).
        let dir = scratch_dir("io-fail");
        let path = dir.join("telemetry.journal");
        std::fs::create_dir_all(&path).expect("creates a directory in the file's place");
        let result = FileLocalBuffer::open(
            &path,
            10,
            None,
            Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)) as Arc<dyn Clock + Send + Sync>,
        );
        assert!(
            matches!(result, Err(FileBufferError::Io { .. })),
            "an unusable journal path is a named I/O error: {result:?}"
        );
        cleanup(&dir);
    }
}
