// P6.05 — The edge-local hexagonal ports (report §18.1, §6.2/§8.2; ledger
// P6.05).
//
// Four ports define everything the edge supervision loop needs; every
// concrete adapter arrives in a later wave and plugs in here:
//
// | Port | Meaning | Planned adapter | Status this wave |
// |---|---|---|---|
// | [`EmsSource`] | Poll the site EMS for canonical telemetry samples | P6.08 read path over the P6.12 `EmsAdapter` + ACL; P6.10 wires OpenEMS | in-memory test fake + [`FailClosedEmsSource`] |
// | [`UplinkChannel`] | Forward samples OUT to the central platform (edge-initiated, §3.3) | P6.07 outbound mTLS channel | [`FailClosedUplink`] |
// | [`LocalBuffer`] | Offline-first store-and-forward spine | P6.09 durable disk buffer | [`InMemoryLocalBuffer`] (bounded, order-preserving) |
// | [`AuditSink`] | Edge-local audit records (degraded-mode transitions, forward results) | P6.18 audit buffer/offline behaviour | [`InMemoryAuditSink`] |
//
// Port discipline (house rules, same as `flexcore-ports`):
//
// - **Sync traits.** The loop is async (tokio), but the ports are
//   deliberately synchronous: the in-memory implementations are
//   non-blocking, and a blocking adapter (disk buffer, HTTP read, mTLS
//   send) bridges itself — the adapter owns its runtime shape, the port
//   stays runtime-free. This mirrors the `TelemetrySource` sync-port rule
//   recorded in `flexcore-ports` (P2.11).
// - **Fail-closed, never silent.** No port method may swallow a failure:
//   every error is a named [`EdgeError`] variant, and the un-configured
//   stubs ([`FailClosedEmsSource`], [`FailClosedUplink`]) return errors on
//   every call — an unconfigured edge degrades LOUDLY, never passes
//   silently.
// - **Canonical samples only.** [`EdgeTelemetrySample`] carries the frozen
//   vocabulary (`TelemetryQuantity`/`QuantityValue` from `flexcore-ports`,
//   `TelemetryQuality` from `flexcore-contracts`, P0.07/P0.08/P0.09
//   timestamp types). The asset key is the edge-local EMS key — the
//   key → registry-`AssetId` mapping exists only at registration on the
//   control-plane side (the P2.25 step-2 pattern, cited not reinvented), so
//   no domain ID type appears here and this crate needs no
//   `flexcore-domain` dependency.
// - **No `flexcore:` source namespace.** A measured edge source must never
//   declare the reserved platform namespace (P2.04 D-T6), and a `MISSING`
//   quality state is a platform-derived gap marker the edge cannot emit
//   (P2.04 §3 value-field totality; edge sources are measured sources) —
//   both are construction-time rejections.

use std::collections::VecDeque;

use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{EventTime, IngestTime};
use flexcore_ports::telemetry_source::{QuantityValue, TelemetryQuantity};

// --- Errors -----------------------------------------------------------------

/// Errors the edge ports and runner report. Every failure is named; nothing
/// is swallowed, and no error implies a silent pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EdgeError {
    /// The source produced no usable reading for a polled point (the EMS
    /// side failed; the runner stays in degraded-safe behaviour).
    SourcePollFailed { reason: String },
    /// The uplink forward failed (channel down); the runner buffers instead.
    UplinkForwardFailed { reason: String },
    /// The buffer rejected an append (for example at capacity after the
    /// backpressure window was missed); the caller must audit, never drop
    /// silently.
    BufferAppendFailed { reason: String },
    /// The audit sink failed; the audit event was NOT recorded (the caller
    /// logs the failure, never suppresses it).
    AuditWriteFailed { reason: String },
    // P6.06 — enrollment / identity failures (additive; the enrolment flow
    // and the credential lifecycle report through the same named-error
    // discipline as the four runner ports — nothing is swallowed).
    /// The enrollment request was REJECTED (by the control plane or by the
    /// fail-closed local seam): single-use token already consumed, duplicate
    /// identity, binding outside the approved tenant/site scope, or no
    /// credential issuer composed. The edge does NOT proceed without an
    /// identity — it has no channel to bring up.
    EnrollmentFailed { reason: String },
    /// The credential store failed (read, write, clear, or a permissions
    /// check); the edge does NOT run with an identity it cannot load or
    /// protect — the caller degrades loudly, never silently.
    CredentialStoreFailed { reason: String },
    /// The revocation check could not be performed (P6.06: OCSP/CRL
    /// checking is a DEPLOYMENT REQUIREMENT — no checker is composed by
    /// default). Fail closed: a revocation status that cannot be
    /// established is never reported as "not revoked".
    RevocationCheckFailed { reason: String },
}

impl std::fmt::Display for EdgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdgeError::SourcePollFailed { reason } => write!(f, "EMS source poll failed: {reason}"),
            EdgeError::UplinkForwardFailed { reason } => {
                write!(f, "uplink forward failed: {reason}")
            }
            EdgeError::BufferAppendFailed { reason } => {
                write!(f, "local buffer append failed: {reason}")
            }
            EdgeError::AuditWriteFailed { reason } => write!(f, "audit sink write failed: {reason}"),
            EdgeError::EnrollmentFailed { reason } => write!(f, "enrollment failed: {reason}"),
            EdgeError::CredentialStoreFailed { reason } => {
                write!(f, "credential store failed: {reason}")
            }
            EdgeError::RevocationCheckFailed { reason } => {
                write!(f, "revocation check failed: {reason}")
            }
        }
    }
}

impl std::error::Error for EdgeError {}

// --- The canonical edge sample ----------------------------------------------

/// Construction errors of [`EdgeTelemetrySample`]. Every variant rejects the
/// WHOLE sample — there is no partial sample (the P2.11 step-3 rule applied
/// at the edge boundary).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EdgeSampleError {
    /// The edge-local asset key was empty.
    EmptyAssetKey,
    /// `source_id` was empty; source identity is mandatory (P2.04 §8).
    EmptySourceId,
    /// The source ID used the reserved `flexcore:` platform namespace —
    /// measured edge sources must not (P2.04 D-T6).
    ReservedSourceNamespace,
    /// A `MISSING` quality state was supplied. `MISSING` is a
    /// platform-derived gap marker (P2.04 D-T6); the edge agent is a
    /// measured source and can never emit it — the gap detector on the
    /// control-plane side owns gap markers.
    MissingQualityIsPlatformDerived,
    /// The value's kind does not match the quantity (P2.04 §3–§4 pairing
    /// rule, reused from the frozen vocabulary).
    ValueQuantityMismatch { quantity: TelemetryQuantity },
    /// `ingest_time` precedes `event_time` — a value cannot be received
    /// before it occurred (P0.09).
    TimeOrdering,
}

impl std::fmt::Display for EdgeSampleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdgeSampleError::EmptyAssetKey => write!(f, "the edge-local asset key is mandatory"),
            EdgeSampleError::EmptySourceId => write!(f, "source_id is mandatory and must be non-empty"),
            EdgeSampleError::ReservedSourceNamespace => write!(
                f,
                "the 'flexcore:' source namespace is reserved for platform-derived records; measured edge sources must not use it"
            ),
            EdgeSampleError::MissingQualityIsPlatformDerived => write!(
                f,
                "quality MISSING is a platform-derived gap marker; an edge measured source cannot emit it"
            ),
            EdgeSampleError::ValueQuantityMismatch { quantity } => {
                write!(f, "the value kind does not match the quantity {quantity:?}")
            }
            EdgeSampleError::TimeOrdering => write!(f, "ingest_time must not precede event_time"),
        }
    }
}

impl std::error::Error for EdgeSampleError {}

/// One canonical telemetry sample at the edge boundary: one quantity of one
/// edge-local asset at one `event_time` (the P2.04 D-T2 one-record shape,
/// carried edge-side with the asset KEY instead of a registry ID — see the
/// module header). Construction is a [`Result`]: a sample missing anything
/// canonical is rejected, never partially built.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeTelemetrySample {
    asset_key: String,
    quantity: TelemetryQuantity,
    value: QuantityValue,
    quality: TelemetryQuality,
    source_quality_code: Option<String>,
    event_time: EventTime,
    ingest_time: IngestTime,
    source_sequence: u64,
    source_id: String,
}

/// The validated construction inputs of [`EdgeTelemetrySample`].
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeTelemetrySampleParts {
    /// The edge-local EMS asset key (stable key the site EMS exposes). NOT a
    /// registry `AssetId`; the key → ID mapping lives control-plane side.
    pub asset_key: String,
    /// Which quantity (frozen vocabulary, `flexcore-ports`).
    pub quantity: TelemetryQuantity,
    /// The canonical value (canonical unit newtypes, `flexcore-contracts`).
    pub value: QuantityValue,
    /// Exactly one of the six canonical quality states (P0.08). Never
    /// `MISSING` from an edge source.
    pub quality: TelemetryQuality,
    /// The original source quality code retained as diagnostic metadata
    /// (P0.08 §3). Never a seventh canonical state.
    pub source_quality_code: Option<String>,
    /// When the measurement occurred (P0.09).
    pub event_time: EventTime,
    /// When the edge received it (P0.09). Distinct type from `event_time`.
    pub ingest_time: IngestTime,
    /// Source-assigned ordering identifier, monotonically increasing per
    /// (asset, source) (P0.09 §1).
    pub source_sequence: u64,
    /// Source identity; the edge adapter id + point, never the reserved
    /// `flexcore:` namespace (P2.04 D-T6).
    pub source_id: String,
}

impl EdgeTelemetrySample {
    /// Validates and builds the sample (fail-closed: see
    /// [`EdgeSampleError`]).
    pub fn new(parts: EdgeTelemetrySampleParts) -> Result<Self, EdgeSampleError> {
        if parts.asset_key.trim().is_empty() {
            return Err(EdgeSampleError::EmptyAssetKey);
        }
        if parts.source_id.trim().is_empty() {
            return Err(EdgeSampleError::EmptySourceId);
        }
        if parts.source_id.starts_with("flexcore:") {
            return Err(EdgeSampleError::ReservedSourceNamespace);
        }
        if parts.quality == TelemetryQuality::Missing {
            return Err(EdgeSampleError::MissingQualityIsPlatformDerived);
        }
        if !parts.quantity.accepts(&parts.value) {
            return Err(EdgeSampleError::ValueQuantityMismatch {
                quantity: parts.quantity,
            });
        }
        // P0.09: ingest must not precede event. The two timestamp types are
        // distinct (no conversion), so the comparison goes through the UTC
        // millisecond values both types expose.
        if parts.ingest_time.unix_millis() < parts.event_time.unix_millis() {
            return Err(EdgeSampleError::TimeOrdering);
        }
        Ok(Self {
            asset_key: parts.asset_key,
            quantity: parts.quantity,
            value: parts.value,
            quality: parts.quality,
            source_quality_code: parts.source_quality_code,
            event_time: parts.event_time,
            ingest_time: parts.ingest_time,
            source_sequence: parts.source_sequence,
            source_id: parts.source_id,
        })
    }

    /// The edge-local asset key.
    pub fn asset_key(&self) -> &str {
        &self.asset_key
    }
    /// The quantity (frozen vocabulary).
    pub fn quantity(&self) -> TelemetryQuantity {
        self.quantity
    }
    /// The canonical value.
    pub fn value(&self) -> &QuantityValue {
        &self.value
    }
    /// The canonical quality state (exactly one of six).
    pub fn quality(&self) -> TelemetryQuality {
        self.quality
    }
    /// The original source quality code, retained as diagnostic metadata.
    pub fn source_quality_code(&self) -> Option<&str> {
        self.source_quality_code.as_deref()
    }
    /// `event_time` (P0.09).
    pub fn event_time(&self) -> EventTime {
        self.event_time
    }
    /// `ingest_time` (P0.09).
    pub fn ingest_time(&self) -> IngestTime {
        self.ingest_time
    }
    /// Source sequence (P0.09 §1).
    pub fn source_sequence(&self) -> u64 {
        self.source_sequence
    }
    /// Source identity.
    pub fn source_id(&self) -> &str {
        &self.source_id
    }
}

// --- The four ports ---------------------------------------------------------

/// The edge-local EMS source port: poll the site EMS for canonical samples
/// (through the P6.12 adapter + anti-corruption layer in later waves).
///
/// Pull-based by contract: nothing is ingested unless the runner polls, and
/// each poll yields at most `max_samples` samples (the runner's
/// backpressure window). A poll failure is `Err` — never an `Ok` carrying
/// silently reduced output (P2.11 step-3 discipline at the edge).
pub trait EmsSource: Send {
    /// The source identity (audit and logging only).
    fn source_name(&self) -> &str;
    /// Polls up to `max_samples` canonical samples.
    fn poll(&mut self, max_samples: usize) -> Result<Vec<EdgeTelemetrySample>, EdgeError>;
}

/// The outbound-only uplink port: forward samples to the central platform
/// (§3.3: the edge dials out; nothing here listens or receives). There is
/// deliberately NO receive method — the command path does not exist in this
/// wave, and P6.14 implements the disabled-by-default receiver behind its
/// written gate.
pub trait UplinkChannel: Send {
    /// Whether the channel is currently usable (advisory; `forward` is the
    /// authoritative outcome).
    fn is_connected(&self) -> bool;
    /// Forwards a batch of samples. At-least-once delivery semantics apply
    /// end-to-end (the P6.09 buffer + control-plane deduplication own the
    /// idempotency, the §15.4 discipline); a failed batch is returned as
    /// `Err` and the runner re-buffers it IN ORDER.
    fn forward(&mut self, samples: &[EdgeTelemetrySample]) -> Result<(), EdgeError>;
}

/// The offline-first local buffer port: bounded, order-preserving
/// store-and-forward (the degraded-mode spine; P6.09 owns the durable disk
/// implementation).
pub trait LocalBuffer: Send {
    /// Current sample count.
    fn len(&self) -> usize;
    /// Total capacity (the §21.4-style bound; never unbounded).
    fn capacity(&self) -> usize;
    /// Appends one sample at the BACK. `Err(BufferAppendFailed)` on a hard
    /// failure (e.g. a journal I/O error) — the runner applies backpressure
    /// BEFORE polling, so a capacity hit is an audited anomaly, never a
    /// routine path. (P6.09 records the durable buffer's overflow policy
    /// explicitly: audited drop-of-OLDEST with a hard counter and a health
    /// flag — see `buffer_file`; the in-memory test buffer keeps the
    /// fail-closed full-is-error behaviour.)
    fn append(&mut self, sample: EdgeTelemetrySample) -> Result<(), EdgeError>;
    /// Removes up to `max` samples from the FRONT (oldest first).
    fn drain(&mut self, max: usize) -> Vec<EdgeTelemetrySample>;
    /// Puts a failed batch back at the FRONT, preserving order (the
    /// re-buffering path of a failed forward). The batch must be exactly
    /// what `drain` returned.
    fn prepend(&mut self, samples: Vec<EdgeTelemetrySample>);
    /// The P6.09 commit point: `samples` — previously returned by
    /// [`LocalBuffer::drain`] and now confirmed forwarded — are COMMITTED.
    /// Default: no-op (the in-memory buffer is volatile anyway). The
    /// durable disk buffer (`buffer_file::FileLocalBuffer`) uses this to
    /// compact its journal: between `drain` and `commit_forwarded` the
    /// records remain on disk, so a crash in that window re-delivers them
    /// after restart — at-least-once delivery (§15.4); downstream
    /// duplicates are consumed by the platform's P4.24-style idempotent
    /// telemetry ingest (cited, not reinvented).
    fn commit_forwarded(&mut self, _samples: &[EdgeTelemetrySample]) {}
}

/// Whether the buffer is empty (defaulted over `len`).
pub trait LocalBufferExt: LocalBuffer {
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T: LocalBuffer> LocalBufferExt for T {}

/// The kinds of edge-local audit events (P6.18 extends the vocabulary for
/// the offline audit buffer; these are the P6.05 runner transitions).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeAuditKind {
    /// The runner started (configuration summary, flag states).
    Startup,
    /// The uplink failed; the runner entered degraded (buffering) mode.
    DegradedBufferingEntered,
    /// The uplink recovered; the runner left degraded mode.
    DegradedBufferingExited,
    /// A batch was forwarded to the uplink.
    ForwardBatch,
    /// A poll of the EMS source failed.
    SourcePollFailed,
    /// The buffer reached capacity; polling paused (backpressure).
    BufferFull,
    // P6.06 — identity/enrollment lifecycle events (additive; §17.3: every
    // cross-boundary identity event is audited).
    /// The edge enrolled successfully: a device credential record was
    /// issued and stored (no secret material in the audit detail).
    EnrollmentCompleted,
    /// An enrollment attempt was REJECTED (duplicate identity, wrong
    /// tenant/site binding, revoked identity, consumed token, no issuer).
    EnrollmentRejected,
    /// The device credential enters the configured rotation window
    /// (P6.06 step 3: expiry is visible at least one rotation window
    /// before expiry — the operator-visible health surface).
    CredentialExpiryWarning,
    /// The device credential has EXPIRED; the channel must not be brought
    /// up on it (P6.07 composes against a valid identity only).
    CredentialExpired,
    /// A revocation check could not establish a status (OCSP/CRL is a
    /// deployment requirement; the un-checked state is audited, never
    /// reported as "not revoked").
    RevocationUnchecked,
}

impl EdgeAuditKind {
    /// Stable audit name (§15.3-style SCREAMING_SNAKE for log/audit lines).
    pub fn name(self) -> &'static str {
        match self {
            EdgeAuditKind::Startup => "EDGE_STARTUP",
            EdgeAuditKind::DegradedBufferingEntered => "EDGE_DEGRADED_BUFFERING_ENTERED",
            EdgeAuditKind::DegradedBufferingExited => "EDGE_DEGRADED_BUFFERING_EXITED",
            EdgeAuditKind::ForwardBatch => "EDGE_FORWARD_BATCH",
            EdgeAuditKind::SourcePollFailed => "EDGE_SOURCE_POLL_FAILED",
            EdgeAuditKind::BufferFull => "EDGE_BUFFER_FULL",
            EdgeAuditKind::EnrollmentCompleted => "EDGE_ENROLLMENT_COMPLETED",
            EdgeAuditKind::EnrollmentRejected => "EDGE_ENROLLMENT_REJECTED",
            EdgeAuditKind::CredentialExpiryWarning => "EDGE_CREDENTIAL_EXPIRY_WARNING",
            EdgeAuditKind::CredentialExpired => "EDGE_CREDENTIAL_EXPIRED",
            EdgeAuditKind::RevocationUnchecked => "EDGE_REVOCATION_UNCHECKED",
        }
    }
}

/// One edge-local audit record (P6.18 owns the durable/offline behaviour).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeAuditEvent {
    /// When the edge recorded the event (the `IngestTime` contracts type:
    /// this is the moment the EDGE received/observed the fact, P0.09).
    pub recorded_at: IngestTime,
    /// The event kind.
    pub kind: EdgeAuditKind,
    /// Human-readable detail (no secrets: the runner never places the
    /// control-plane endpoint or credentials here).
    pub detail: String,
}

/// The edge-local audit sink port.
pub trait AuditSink: Send + Sync {
    /// Records one audit event. A failure is `Err` — the caller logs the
    /// failure; audit is never silently skipped (the P4.28/P4.30
    /// audit-or-refuse discipline, cited not reinvented).
    fn record(&self, event: EdgeAuditEvent) -> Result<(), EdgeError>;
}

// --- In-memory implementations (tests + this-wave composition) --------------

/// A bounded, order-preserving in-memory [`LocalBuffer`]. P6.09 replaces
/// this with the durable disk buffer; the trait contract (bounds, order,
/// fail-closed append) is fixed here so that swap is mechanical.
#[derive(Debug)]
pub struct InMemoryLocalBuffer {
    capacity: usize,
    queue: VecDeque<EdgeTelemetrySample>,
}

impl InMemoryLocalBuffer {
    /// Builds a buffer with the given capacity (must be non-zero; enforced
    /// by `assert` — a zero-capacity buffer is a construction-time
    /// programmer error, not a runtime condition).
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "buffer capacity must be non-zero");
        Self {
            capacity,
            queue: VecDeque::new(),
        }
    }
}

impl LocalBuffer for InMemoryLocalBuffer {
    fn len(&self) -> usize {
        self.queue.len()
    }

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn append(&mut self, sample: EdgeTelemetrySample) -> Result<(), EdgeError> {
        if self.queue.len() >= self.capacity {
            return Err(EdgeError::BufferAppendFailed {
                reason: format!("buffer at capacity {}/{}", self.queue.len(), self.capacity),
            });
        }
        self.queue.push_back(sample);
        Ok(())
    }

    fn drain(&mut self, max: usize) -> Vec<EdgeTelemetrySample> {
        self.queue.drain(..max.min(self.queue.len())).collect()
    }

    fn prepend(&mut self, samples: Vec<EdgeTelemetrySample>) {
        // Front-insert in batch order so the queue keeps the original
        // global order (oldest first).
        for sample in samples.into_iter().rev() {
            self.queue.push_front(sample);
        }
    }
}

/// A [`Vec`]-backed in-memory [`AuditSink`]: records are retrievable for
/// assertions and for the honest startup dump. P6.18 replaces it with the
/// offline audit buffer.
#[derive(Debug, Default)]
pub struct InMemoryAuditSink {
    records: std::sync::Mutex<Vec<EdgeAuditEvent>>,
}

impl InMemoryAuditSink {
    pub fn new() -> Self {
        Self::default()
    }

    /// A snapshot of the recorded events, in record order.
    pub fn events(&self) -> Vec<EdgeAuditEvent> {
        self.records
            .lock()
            .expect("poisoned audit mutex")
            .clone()
    }
}

impl AuditSink for InMemoryAuditSink {
    fn record(&self, event: EdgeAuditEvent) -> Result<(), EdgeError> {
        self.records
            .lock()
            .expect("poisoned audit mutex")
            .push(event);
        Ok(())
    }
}

// --- Fail-closed stub adapters ----------------------------------------------

/// The un-configured [`EmsSource`]: every poll fails with a named error.
/// This is what the composition root wires when no real EMS read path is
/// composed yet (P6.10 wires OpenEMS) — an unconfigured edge degrades
/// LOUDLY at startup, never passes silently (no no-op stub anywhere).
#[derive(Debug, Default)]
pub struct FailClosedEmsSource;

impl EmsSource for FailClosedEmsSource {
    fn source_name(&self) -> &str {
        "fail-closed-ems-source"
    }

    fn poll(&mut self, _max_samples: usize) -> Result<Vec<EdgeTelemetrySample>, EdgeError> {
        Err(EdgeError::SourcePollFailed {
            reason: "no EMS read path is composed yet (P6.08/P6.10 wire the real source); \
                     the edge agent refuses to fabricate telemetry"
                .to_string(),
        })
    }
}

/// The un-configured [`UplinkChannel`]: every forward fails with a named
/// error. The composition root wires this until P6.07 delivers the outbound
/// mTLS channel — the edge runs degraded (buffering) the whole time, which
/// is the honest offline-first behaviour, never a silent pass-through.
#[derive(Debug, Default)]
pub struct FailClosedUplink;

impl UplinkChannel for FailClosedUplink {
    fn is_connected(&self) -> bool {
        false
    }

    fn forward(&mut self, _samples: &[EdgeTelemetrySample]) -> Result<(), EdgeError> {
        Err(EdgeError::UplinkForwardFailed {
            reason: "no outbound channel is composed yet (P6.07 wires the mTLS uplink); \
                     the edge agent stays in degraded buffering mode"
                .to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::units::ActivePowerMW;

    fn sample(sequence: u64) -> EdgeTelemetrySample {
        EdgeTelemetrySample::new(EdgeTelemetrySampleParts {
            asset_key: "ess-01".to_string(),
            quantity: TelemetryQuantity::ActivePower,
            value: QuantityValue::Power(ActivePowerMW::new(1.5).expect("finite")),
            quality: TelemetryQuality::Good,
            source_quality_code: None,
            event_time: EventTime::from_unix_millis(1_000).expect("valid"),
            ingest_time: IngestTime::from_unix_millis(2_000).expect("valid"),
            source_sequence: sequence,
            source_id: "edge:pilot-site-A:openems-rest:ess-01:active_power".to_string(),
        })
        .expect("valid sample")
    }

    #[test]
    fn in_memory_buffer_is_bounded_and_order_preserving() {
        let mut buffer = InMemoryLocalBuffer::new(3);
        for s in 0..3u64 {
            buffer.append(sample(s)).expect("fits");
        }
        assert_eq!(buffer.len(), 3);
        // Full buffer fails closed.
        assert!(matches!(
            buffer.append(sample(9)),
            Err(EdgeError::BufferAppendFailed { .. })
        ));
        // Drain preserves order (oldest first).
        let drained = buffer.drain(2);
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].source_sequence(), 0);
        assert_eq!(drained[1].source_sequence(), 1);
        // Prepend restores at the front, in batch order.
        buffer.prepend(drained);
        assert_eq!(buffer.len(), 3);
        let first = buffer.drain(1);
        assert_eq!(first[0].source_sequence(), 0);
    }

    #[test]
    fn sample_rejects_reserved_namespace_missing_quality_and_bad_ordering() {
        // Reserved namespace (P2.04 D-T6).
        let parts = EdgeTelemetrySampleParts {
            asset_key: "ess-01".to_string(),
            quantity: TelemetryQuantity::ActivePower,
            value: QuantityValue::Power(ActivePowerMW::new(1.0).expect("finite")),
            quality: TelemetryQuality::Good,
            source_quality_code: None,
            event_time: EventTime::from_unix_millis(1_000).expect("valid"),
            ingest_time: IngestTime::from_unix_millis(2_000).expect("valid"),
            source_sequence: 1,
            source_id: "flexcore:some-platform-record".to_string(),
        };
        assert_eq!(
            EdgeTelemetrySample::new(parts),
            Err(EdgeSampleError::ReservedSourceNamespace)
        );
        // MISSING is platform-derived; the edge cannot emit it.
        let parts = EdgeTelemetrySampleParts {
            quality: TelemetryQuality::Missing,
            source_id: "edge:pilot-site-A:openems-rest:ess-01:active_power".to_string(),
            ..same_parts()
        };
        assert_eq!(
            EdgeTelemetrySample::new(parts),
            Err(EdgeSampleError::MissingQualityIsPlatformDerived)
        );
        // ingest before event is rejected (P0.09).
        let parts = EdgeTelemetrySampleParts {
            event_time: EventTime::from_unix_millis(3_000).expect("valid"),
            ingest_time: IngestTime::from_unix_millis(2_000).expect("valid"),
            ..same_parts()
        };
        assert_eq!(EdgeTelemetrySample::new(parts), Err(EdgeSampleError::TimeOrdering));
    }

    fn same_parts() -> EdgeTelemetrySampleParts {
        EdgeTelemetrySampleParts {
            asset_key: "ess-01".to_string(),
            quantity: TelemetryQuantity::ActivePower,
            value: QuantityValue::Power(ActivePowerMW::new(1.0).expect("finite")),
            quality: TelemetryQuality::Good,
            source_quality_code: None,
            event_time: EventTime::from_unix_millis(1_000).expect("valid"),
            ingest_time: IngestTime::from_unix_millis(2_000).expect("valid"),
            source_sequence: 1,
            source_id: "edge:pilot-site-A:openems-rest:ess-01:active_power".to_string(),
        }
    }

    #[test]
    fn fail_closed_stubs_return_named_errors() {
        let mut source = FailClosedEmsSource;
        assert!(matches!(
            source.poll(10),
            Err(EdgeError::SourcePollFailed { .. })
        ));
        let mut uplink = FailClosedUplink;
        assert!(!uplink.is_connected());
        assert!(matches!(
            uplink.forward(&[]),
            Err(EdgeError::UplinkForwardFailed { .. })
        ));
    }

    #[test]
    fn audit_sink_records_in_order() {
        let sink = InMemoryAuditSink::new();
        sink.record(EdgeAuditEvent {
            recorded_at: IngestTime::from_unix_millis(1_000).expect("valid"),
            kind: EdgeAuditKind::Startup,
            detail: "test".to_string(),
        })
        .expect("records");
        sink.record(EdgeAuditEvent {
            recorded_at: IngestTime::from_unix_millis(2_000).expect("valid"),
            kind: EdgeAuditKind::DegradedBufferingEntered,
            detail: "test 2".to_string(),
        })
        .expect("records");
        let events = sink.events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, EdgeAuditKind::Startup);
        assert_eq!(events[1].kind.name(), "EDGE_DEGRADED_BUFFERING_ENTERED");
    }
}
