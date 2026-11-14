// P6.08 — Edge telemetry ingestion read path (report §18.1, §10.2–§10.4;
// ledger P6.08).
//
// This module is the READ PATH between the P6.12 adapter world and the
// P6.05 runner world. It wires the flow the ledger names:
//
// ```text
// EmsAdapter.read (native reading, P6.12 port)
//   → AntiCorruptionLayer.map_reading (P6.12 ACL: units, signs, quality,
//     event/ingest time — the ONLY translation, cited not reinvented)
//   → EdgeTelemetrySample (canonical, P6.05 ports)
//   → EmsSource.poll output → runner buffer → UplinkChannel forward
// ```
//
// [`AdapterIngestSource`] implements the edge [`crate::ports::EmsSource`]
// port over a composed [`EmsAdapter`], so the runner needs no change in
// shape: the composition root hands it a `Box<dyn EmsSource>` as before
// (P6.10 will compose the real OpenEMS adapter exactly this way; P6.07's
// mTLS uplink composes at the OTHER port — `UplinkChannel` — and this
// module deliberately does not touch it).
//
// The read-path rules, each enforced here:
//
// 1. **Quality is never upgraded or derived.** The ACL maps or refuses
//    (P6.12); this module adds NOTHING on top: a mapped sample's quality
//    passes through untouched, and an ACL rejection means the reading
//    produces no sample at all — there is no default quality, no inferred
//    `GOOD`, anywhere on this path (P0.08 non-silencing).
// 2. **event_time comes from the source; ingest_time is stamped at the
//    edge.** The two are the distinct P0.09 contract types and are never
//    conflated: the ACL maps the native timestamp to `event_time`, and
//    this module stamps `ingest_time` from the edge [`Clock`] at the poll
//    that received the reading. The sample constructor enforces
//    `ingest >= event` (P0.09) — a source timestamp from the future is a
//    rejection, never a silent clock "fix".
// 3. **Per-poll error accounting.** A per-point failure (read error, ACL
//    rejection) rejects that WHOLE reading (the P2.11 step-3 no-partial-
//    record rule at the edge) and is COUNTED in [`IngestStats`] — the
//    reduction of output is accounted, never silent. A poll where EVERY
//    point failed, or where the adapter is unreachable, is a hard poll
//    failure (`Err(SourcePollFailed)`), so the runner audits it
//    (`EDGE_SOURCE_POLL_FAILED`) — an unconfigured or dead source degrades
//    LOUDLY, never passes silently (the P6.05 fail-closed discipline).
//    This is the recorded per-poll policy: one broken channel must not
//    starve ingestion of the healthy ones, but zero output with failures
//    is never reported as a clean empty poll.
// 4. **Edge-side deduplication reuses the P4.24 discipline (cited, not
//    reinvented).** Identity = (source point identity, event_time) — the
//    edge analogue of the P4.24 idempotency key. An exact re-read (same
//    identity, same content) is an idempotent skip, counted
//    ([`IngestStats::duplicates_skipped`]); the same identity with
//    different content is a conflicting duplicate, refused deterministically
//    and counted separately (the P4.24 fixture-(b) rule: same key,
//    conflicting payload → rejected, never accepted). The window is
//    bounded ([`EdgeDedupWindow::new`]); a duplicate arriving OUTSIDE the
//    window is NOT caught at the edge — the platform's P4.24-style
//    idempotent telemetry ingest consumes it (§15.4 at-least-once; the
//    P2.09/P2.10 duplicate-classification discipline). Honest record: the
//    edge window is an optimization against poll-cadence re-reads, not the
//    system's idempotency boundary.
//
// Sync trait composition: the [`EmsAdapter`] port is sync (P6.12 house
// rule); the runner is async; this module is the sync bridge in between —
// it owns no runtime shape of its own.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;

use flexcore_ports::clock::Clock;

use crate::adapters::acl::{AntiCorruptionLayer, PointMapping};
use crate::adapters::port::{AdapterError, EmsAdapter, PointKey};
use crate::ports::{EdgeError, EdgeTelemetrySample, EmsSource};

// --- Edge-side deduplication (the P4.24 discipline, cited) -------------------

/// The outcome of admitting one sample into the edge dedup window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmitOutcome {
    /// First sighting: the sample is new (forward it).
    New,
    /// Exact replay of an already-admitted sample: same identity, same
    /// content — the idempotent skip (the P4.24 fixture-(a) rule:
    /// identical replay deduplicates to the original record).
    ExactDuplicate,
    /// Same identity but different content — the conflicting duplicate
    /// (the P4.24 fixture-(b) rule: same key, conflicting payload is
    /// REFUSED, deterministically, never accepted).
    ConflictingDuplicate,
}

/// The edge dedup identity: the sample's source identity (which already
/// encodes the adapter + point) plus its `event_time`. This is the edge
/// analogue of the P4.24 idempotency-key rule; like P4.24, the identity is
/// derived from immutable identity fields only — never from a mutable
/// value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DedupKey {
    source_id: String,
    event_millis: i128,
}

/// A bounded, order-evicted deduplication window over canonical samples.
///
/// Bounded BY CONTRACT (the edge is a constrained host, §18.3): the window
/// holds at most `capacity` identities, oldest evicted first. A duplicate
/// arriving after its identity was evicted is NOT caught here — downstream
/// idempotency (the platform's P4.24-style ingest, §15.4) consumes it.
/// The window is an optimization against poll-cadence re-reads, never the
/// system's idempotency boundary.
#[derive(Debug)]
pub struct EdgeDedupWindow {
    capacity: usize,
    order: VecDeque<DedupKey>,
    cache: HashMap<DedupKey, EdgeTelemetrySample>,
}

impl EdgeDedupWindow {
    /// Builds the window (capacity must be non-zero — a zero-capacity
    /// window is a construction-time programmer error, same rule as the
    /// buffer capacities).
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "dedup window capacity must be non-zero");
        Self {
            capacity,
            order: VecDeque::new(),
            cache: HashMap::new(),
        }
    }

    /// Admits one sample: classifies it as [`AdmitOutcome::New`],
    /// [`AdmitOutcome::ExactDuplicate`], or
    /// [`AdmitOutcome::ConflictingDuplicate`]. Only `New` samples are
    /// admitted into the window (a duplicate does not refresh its
    /// position — the window tracks first sightings in arrival order).
    pub fn admit(&mut self, sample: &EdgeTelemetrySample) -> AdmitOutcome {
        let key = DedupKey {
            source_id: sample.source_id().to_string(),
            event_millis: sample.event_time().unix_millis(),
        };
        match self.cache.get(&key) {
            Some(seen) if seen == sample => return AdmitOutcome::ExactDuplicate,
            Some(_) => return AdmitOutcome::ConflictingDuplicate,
            None => {}
        }
        if self.cache.len() >= self.capacity {
            // Evict the oldest identity (FIFO, bounded).
            if let Some(oldest) = self.order.pop_front() {
                self.cache.remove(&oldest);
            }
        }
        self.order.push_back(key.clone());
        self.cache.insert(key, sample.clone());
        AdmitOutcome::New
    }

    /// How many identities the window currently holds.
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Whether the window holds no identities.
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

// --- Per-poll error accounting ------------------------------------------------

/// The cumulative, queryable counters of one [`AdapterIngestSource`]
/// (visible to health, P6.17). Every failure mode the read path can hit is
/// a named counter — nothing is swallowed, and "no errors" is never
/// inferred from silence (the P6.17 rule: health is never inferred from the
/// absence of errors).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IngestStats {
    /// Polls attempted.
    pub polls: u64,
    /// Samples emitted (post-ACL, post-dedup).
    pub samples_emitted: u64,
    /// Whole readings rejected because the adapter read failed.
    pub read_failures: u64,
    /// Whole readings rejected by the anti-corruption layer (unmapped
    /// point, undeclared/unknown quality, non-UTC timestamp, non-finite
    /// value, …).
    pub acl_rejections: u64,
    /// Exact re-reads skipped idempotently (the P4.24 fixture-(a) rule).
    pub duplicates_skipped: u64,
    /// Same identity, different content — refused (the P4.24 fixture-(b)
    /// rule).
    pub conflicting_duplicates_skipped: u64,
    /// Polls that failed HARD (adapter unreachable, or every point failed)
    /// and were reported as `Err(SourcePollFailed)` to the runner.
    pub hard_poll_failures: u64,
    /// The most recent failure reason (human-safe; diagnostics only).
    pub last_failure_reason: Option<String>,
}

// --- Construction errors ------------------------------------------------------

/// Construction errors of [`AdapterIngestSource`] (fail-fast composition).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestConfigError {
    /// The mapped-point set was empty: a read path with no mapped points
    /// would silently poll nothing forever — refuse at composition.
    EmptyPointSet,
    /// The dedup window capacity was zero.
    ZeroDedupCapacity,
}

impl std::fmt::Display for IngestConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IngestConfigError::EmptyPointSet => {
                write!(f, "the read path has no mapped points; a read path that polls nothing must be a composition error, not a silent no-op")
            }
            IngestConfigError::ZeroDedupCapacity => {
                write!(f, "the dedup window capacity must be non-zero")
            }
        }
    }
}

impl std::error::Error for IngestConfigError {}

// --- The read path ------------------------------------------------------------

/// The P6.08 read path: one composed [`EmsAdapter`] + [`AntiCorruptionLayer`]
/// behind the edge [`EmsSource`] port. This is what replaces
/// `FailClosedEmsSource` when a read path exists (the P6.10 OpenEMS adapter
/// composes exactly here; the `SimulatedEmsSource` test fixture composes
/// exactly here in tests).
pub struct AdapterIngestSource {
    adapter: Box<dyn EmsAdapter>,
    acl: AntiCorruptionLayer,
    /// The mapped points, in declared mapping order (the poll visits them
    /// in this order; the order is the composition's declared order, never
    /// a discovery guess).
    points: Vec<PointKey>,
    clock: Arc<dyn Clock + Send + Sync>,
    dedup: EdgeDedupWindow,
    stats: IngestStats,
}

impl AdapterIngestSource {
    /// Composes the read path. `mapped_points` must be the SAME point set
    /// the ACL's mapping table declares (a point missing from the ACL is
    /// refused per-reading with `AclError::PointUnmapped` and counted —
    /// the composition should not create that divergence, and the ACL
    /// error surfaces it loudly if it happens).
    pub fn new(
        adapter: Box<dyn EmsAdapter>,
        acl: AntiCorruptionLayer,
        mapped_points: Vec<PointMapping>,
        clock: Arc<dyn Clock + Send + Sync>,
        dedup_capacity: usize,
    ) -> Result<Self, IngestConfigError> {
        if mapped_points.is_empty() {
            return Err(IngestConfigError::EmptyPointSet);
        }
        if dedup_capacity == 0 {
            return Err(IngestConfigError::ZeroDedupCapacity);
        }
        Ok(Self {
            adapter,
            acl,
            points: mapped_points.into_iter().map(|m| m.point).collect(),
            clock,
            dedup: EdgeDedupWindow::new(dedup_capacity),
            stats: IngestStats::default(),
        })
    }

    /// The cumulative counters (health visibility, P6.17).
    pub fn stats(&self) -> &IngestStats {
        &self.stats
    }

    fn record_failure(&mut self, reason: String) {
        self.stats.last_failure_reason = Some(reason);
    }
}

impl EmsSource for AdapterIngestSource {
    fn source_name(&self) -> &str {
        // The adapter id names the read path honestly (audit/logging).
        self.adapter.id()
    }

    /// One poll of the read path (the port contract: at most `max_samples`
    /// samples; failure is `Err`, never a silent reduction — see the module
    /// header for the recorded per-poll accounting policy).
    fn poll(&mut self, max_samples: usize) -> Result<Vec<EdgeTelemetrySample>, EdgeError> {
        self.stats.polls += 1;

        // ingest_time: stamped at the edge, at the poll that receives the
        // readings (P0.09 — distinct from the source-declared event_time;
        // one stamp per poll because the poll IS the receipt event).
        let ingest_time = self.clock.now_ingest_time();

        let mut emitted: Vec<EdgeTelemetrySample> = Vec::new();
        let mut read_failures: usize = 0;
        let mut acl_rejections: usize = 0;
        let mut first_failure: Option<String> = None;

        for point in &self.points {
            if emitted.len() >= max_samples {
                break;
            }
            match self.adapter.read(point) {
                Err(AdapterError::NotConnected { reason }) => {
                    // The adapter has no usable connection: the WHOLE
                    // source is unusable this poll. Fail closed — no
                    // partial output from a dead source, loudly.
                    let reason = format!(
                        "adapter '{}' is not connected: {reason}",
                        self.adapter.id()
                    );
                    self.record_failure(reason.clone());
                    self.stats.hard_poll_failures += 1;
                    self.stats.read_failures += 1;
                    return Err(EdgeError::SourcePollFailed { reason });
                }
                Err(other) => {
                    read_failures += 1;
                    let reason = format!("read of point '{point}' failed: {other}");
                    if first_failure.is_none() {
                        first_failure = Some(reason.clone());
                    }
                    self.record_failure(reason);
                    continue;
                }
                Ok(reading) => match self.acl.map_reading(&reading, ingest_time) {
                    Err(error) => {
                        // The ACL rejected the WHOLE reading (no default
                        // quality, no partial mapping). Count it; nothing
                        // is emitted for this point (P2.11 step-3 rule at
                        // the edge).
                        acl_rejections += 1;
                        let reason =
                            format!("ACL rejected the reading of point '{point}': {error}");
                        if first_failure.is_none() {
                            first_failure = Some(reason.clone());
                        }
                        self.record_failure(reason);
                        continue;
                    }
                    Ok(sample) => {
                        // Edge-side dedup (P4.24 discipline, cited): exact
                        // re-read = idempotent skip; same identity with
                        // different content = conflicting duplicate,
                        // refused. Both are counted, neither is forwarded.
                        match self.dedup.admit(&sample) {
                            AdmitOutcome::ExactDuplicate => {
                                self.stats.duplicates_skipped += 1;
                            }
                            AdmitOutcome::ConflictingDuplicate => {
                                self.stats.conflicting_duplicates_skipped += 1;
                                let reason = format!(
                                    "conflicting duplicate at identity ({}, event {}): \
                                     same identity, different content — refused",
                                    sample.source_id(),
                                    sample.event_time().unix_millis(),
                                );
                                if first_failure.is_none() {
                                    first_failure = Some(reason.clone());
                                }
                                self.record_failure(reason);
                            }
                            AdmitOutcome::New => emitted.push(sample),
                        }
                    }
                },
            }
        }

        self.stats.read_failures += read_failures as u64;
        self.stats.acl_rejections += acl_rejections as u64;

        // Hard-failure rule: every point failed → the poll is `Err`, so
        // the runner audits `EDGE_SOURCE_POLL_FAILED`. Zero output with
        // failures is NEVER reported as a clean empty poll.
        if emitted.is_empty() && read_failures + acl_rejections == self.points.len() {
            let reason = first_failure.unwrap_or_else(|| {
                format!(
                    "every mapped point of adapter '{}' failed this poll",
                    self.adapter.id()
                )
            });
            self.stats.hard_poll_failures += 1;
            return Err(EdgeError::SourcePollFailed { reason });
        }

        self.stats.samples_emitted += emitted.len() as u64;
        Ok(emitted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::acl::{NativeSign, NativeUnit, PointMapping};
    use crate::adapters::simulated::{
        SimulatedEmsSource, SimulatedFixture, SimulatedPointFixture,
    };
    use flexcore_contracts::quality::TelemetryQuality;
    use flexcore_contracts::time_model::{EventTime, IngestTime};
    use flexcore_contracts::units::{ActivePowerMW, ReactiveSignConvention};
    use flexcore_ports::fakes::FixedClock;
    use flexcore_ports::telemetry_source::{QuantityValue, TelemetryQuantity};

    use crate::ports::EdgeTelemetrySampleParts;

    const BASE_MILLIS: i64 = 1_700_000_000_000;
    const EDGE_INGEST_MILLIS: i64 = BASE_MILLIS + 5_000;

    fn fixture() -> SimulatedFixture {
        SimulatedFixture {
            points: vec![
                SimulatedPointFixture {
                    point: PointKey::new("ess0", "ActivePower"),
                    native_unit: NativeUnit::Kw,
                    values_per_tick: vec![1_500.0, 1_600.0],
                    quality_per_tick: vec!["GOOD".to_string(), "GOOD".to_string()],
                },
                SimulatedPointFixture {
                    point: PointKey::new("ess0", "Soc"),
                    native_unit: NativeUnit::SocPercent,
                    values_per_tick: vec![87.0, 86.0],
                    quality_per_tick: vec!["GOOD".to_string(), "OUT_OF_RANGE".to_string()],
                },
            ],
            start_millis: BASE_MILLIS,
            step_millis: 1_000,
            reactive_source_convention: ReactiveSignConvention::InjectionPositive,
            cadence_millis: 5_000,
        }
    }

    fn acl() -> AntiCorruptionLayer {
        AntiCorruptionLayer::new(
            vec![
                PointMapping {
                    point: PointKey::new("ess0", "ActivePower"),
                    asset_key: "ess-01".to_string(),
                    quantity: TelemetryQuantity::ActivePower,
                    native_unit: NativeUnit::Kw,
                    native_sign: Some(NativeSign::ExportPositive),
                    reactive_source_convention: None,
                },
                PointMapping {
                    point: PointKey::new("ess0", "Soc"),
                    asset_key: "ess-01".to_string(),
                    quantity: TelemetryQuantity::Soc,
                    native_unit: NativeUnit::SocPercent,
                    native_sign: None,
                    reactive_source_convention: None,
                },
            ],
            "edge:pilot-site-A:simulated-test",
            vec![],
        )
        .expect("valid mapping table")
    }

    fn source() -> AdapterIngestSource {
        AdapterIngestSource::new(
            Box::new(SimulatedEmsSource::new(fixture())),
            acl(),
            acl_test_mappings(),
            Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS)),
            64,
        )
        .expect("valid composition")
    }

    fn composed(
        sim: SimulatedEmsSource,
    ) -> AdapterIngestSource {
        AdapterIngestSource::new(
            Box::new(sim),
            acl(),
            acl_test_mappings(),
            Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS)),
            64,
        )
        .expect("valid composition")
    }

    fn acl_test_mappings() -> Vec<PointMapping> {
        vec![
            PointMapping {
                point: PointKey::new("ess0", "ActivePower"),
                asset_key: "ess-01".to_string(),
                quantity: TelemetryQuantity::ActivePower,
                native_unit: NativeUnit::Kw,
                native_sign: Some(NativeSign::ExportPositive),
                reactive_source_convention: None,
            },
            PointMapping {
                point: PointKey::new("ess0", "Soc"),
                asset_key: "ess-01".to_string(),
                quantity: TelemetryQuantity::Soc,
                native_unit: NativeUnit::SocPercent,
                native_sign: None,
                reactive_source_convention: None,
            },
        ]
    }

    #[test]
    fn read_path_maps_units_times_and_quality_canonically() {
        let mut source = source();
        let samples = source.poll(10).expect("poll succeeds");
        assert_eq!(samples.len(), 2, "both mapped points emit");

        // Active power: 1 500 kW export-positive → 1.5 MW (canonical unit).
        let power = &samples[0];
        assert_eq!(power.asset_key(), "ess-01");
        assert_eq!(power.quantity(), TelemetryQuantity::ActivePower);
        match power.value() {
            QuantityValue::Power(mw) => assert_eq!(mw.value(), 1.5),
            other => panic!("expected a power value, got {other:?}"),
        }
        assert_eq!(power.quality(), TelemetryQuality::Good);
        assert_eq!(power.event_time().unix_millis(), BASE_MILLIS as i128);
        // ingest_time is the EDGE stamp — present, distinct, and later than
        // the event (P0.09: two distinct types, never conflated).
        assert_eq!(power.ingest_time().unix_millis(), EDGE_INGEST_MILLIS as i128);
        assert!(power.ingest_time().unix_millis() > power.event_time().unix_millis());
        assert_eq!(power.source_sequence(), 1);
        // The source identity carries the edge prefix — never `flexcore:`
        // (P2.04 D-T6, enforced by the sample constructor).
        assert_eq!(
            power.source_id(),
            "edge:pilot-site-A:simulated-test/ess0/ActivePower"
        );

        // SOC: 87 % → fraction 0.87 (canonical fraction, never percent).
        let soc = &samples[1];
        match soc.value() {
            QuantityValue::Fraction(fraction) => {
                assert!((fraction.value() - 0.87).abs() < 1e-12)
            }
            other => panic!("expected a fraction value, got {other:?}"),
        }

        let stats = source.stats();
        assert_eq!(stats.polls, 1);
        assert_eq!(stats.samples_emitted, 2);
        assert_eq!(stats.read_failures, 0);
        assert_eq!(stats.acl_rejections, 0);
        assert_eq!(stats.duplicates_skipped, 0);
        assert_eq!(stats.hard_poll_failures, 0);
    }

    #[test]
    fn scenario_advances_event_time_between_ticks_and_quality_passes_through() {
        let sim = SimulatedEmsSource::new(fixture());
        let control = sim.control();
        let mut source = composed(sim);
        // Tick 0: GOOD soc at event BASE.
        let tick0 = source.poll(10).expect("tick 0");
        assert_eq!(tick0.len(), 2);
        let soc0 = tick0
            .iter()
            .find(|s| s.quantity() == TelemetryQuantity::Soc)
            .expect("soc sample emitted");
        assert_eq!(soc0.quality(), TelemetryQuality::Good);
        assert_eq!(soc0.event_time().unix_millis(), BASE_MILLIS as i128);

        // Advance the deterministic scenario clock: tick 1 has a NEW
        // event_time (fresh samples, not deduplicated) and the fixture's
        // OUT_OF_RANGE code for the soc point.
        control.advance_tick();
        let tick1 = source.poll(10).expect("tick 1");
        assert_eq!(tick1.len(), 2, "a new event_time is not deduplicated");
        let soc1 = tick1
            .iter()
            .find(|s| s.quantity() == TelemetryQuantity::Soc)
            .expect("soc sample emitted");
        assert_eq!(
            soc1.event_time().unix_millis(),
            (BASE_MILLIS + 1_000) as i128,
            "event_time comes from the source, per tick"
        );
        // The fixture declared OUT_OF_RANGE at this tick: the sample
        // arrives SUSPECT with the original code retained — never upgraded
        // to GOOD, never silently converted into a normal reading (P0.08;
        // ledger P6.08 step 3).
        assert_eq!(soc1.quality(), TelemetryQuality::Suspect);
        assert_eq!(soc1.source_quality_code(), Some("OUT_OF_RANGE"));
        // The power point is still GOOD: quality is per-value, exactly one
        // state per value (§10.4).
        let power1 = tick1
            .iter()
            .find(|s| s.quantity() == TelemetryQuantity::ActivePower)
            .expect("power sample emitted");
        assert_eq!(power1.quality(), TelemetryQuality::Good);
    }

    #[test]
    fn same_event_time_reread_is_an_idempotent_counted_skip() {
        let mut source = source();
        let first = source.poll(10).expect("first poll");
        assert_eq!(first.len(), 2);
        // Re-read at the SAME scenario tick: identical event_time + content
        // → idempotent skip (P4.24 fixture-(a) discipline, cited).
        let second = source.poll(10).expect("second poll");
        assert!(
            second.is_empty(),
            "re-read samples are deduplicated, not re-emitted"
        );
        let stats = source.stats();
        assert_eq!(stats.duplicates_skipped, 2);
        assert_eq!(stats.samples_emitted, 2, "emitted count does not grow");
        assert_eq!(stats.conflicting_duplicates_skipped, 0);
    }

    #[test]
    fn conflicting_duplicate_same_identity_different_content_is_refused() {
        // The P4.24 fixture-(b) rule at the edge: same identity (source_id
        // + event_time), different payload → refused deterministically.
        let mut window = EdgeDedupWindow::new(8);
        let original = sample_at(BASE_MILLIS, 1, 1.5);
        assert_eq!(window.admit(&original), AdmitOutcome::New);
        // Exact replay: idempotent skip.
        assert_eq!(window.admit(&original), AdmitOutcome::ExactDuplicate);
        // Same identity, different value: conflicting duplicate.
        let conflicting = sample_at(BASE_MILLIS, 1, 9.9);
        assert_eq!(
            window.admit(&conflicting),
            AdmitOutcome::ConflictingDuplicate
        );
        // The window remains bounded: duplicates do not grow it.
        assert_eq!(window.len(), 1, "duplicates do not grow the window");
    }

    /// A canonical sample with a distinct `event_time` per `seq` and a
    /// selectable value, at the shared test identity.
    fn sample_at(event_millis: i64, sequence: u64, mw: f64) -> EdgeTelemetrySample {
        EdgeTelemetrySample::new(EdgeTelemetrySampleParts {
            asset_key: "ess-01".to_string(),
            quantity: TelemetryQuantity::ActivePower,
            value: QuantityValue::Power(ActivePowerMW::new(mw).expect("finite")),
            quality: TelemetryQuality::Good,
            source_quality_code: None,
            event_time: EventTime::from_unix_millis(event_millis).expect("valid"),
            ingest_time: IngestTime::from_unix_millis(EDGE_INGEST_MILLIS).expect("valid"),
            source_sequence: sequence,
            source_id: "edge:pilot-site-A:simulated-test/ess0/ActivePower".to_string(),
        })
        .expect("valid sample")
    }

    #[test]
    fn dedup_window_evicts_oldest_and_stays_bounded() {
        let mut window = EdgeDedupWindow::new(2);
        assert_eq!(window.admit(&sample_at(BASE_MILLIS, 1, 1.0)), AdmitOutcome::New);
        assert_eq!(
            window.admit(&sample_at(BASE_MILLIS + 1_000, 2, 1.0)),
            AdmitOutcome::New
        );
        assert_eq!(window.len(), 2);
        // The third admission evicts the oldest identity (bounded window).
        assert_eq!(
            window.admit(&sample_at(BASE_MILLIS + 2_000, 3, 1.0)),
            AdmitOutcome::New
        );
        assert_eq!(window.len(), 2);
        // Sequence 1 was evicted: re-admitting it counts as New — the
        // recorded honesty of a bounded window (out-of-window duplicates
        // are the platform's P4.24-style ingest's job, cited).
        assert_eq!(window.admit(&sample_at(BASE_MILLIS, 1, 1.0)), AdmitOutcome::New);
    }

    #[test]
    fn acl_failure_is_counted_and_a_healthy_point_still_flows() {
        // A point whose quality code the ACL cannot map: the reading is
        // rejected WHOLE and counted; the healthy sibling point still
        // flows (the recorded per-poll accounting policy).
        let mut fixture = fixture();
        fixture.points[0].quality_per_tick = vec!["WEIRD_VENDOR_FLAG".to_string()];
        let mut source = AdapterIngestSource::new(
            Box::new(SimulatedEmsSource::new(fixture)),
            acl(),
            acl_test_mappings(),
            Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS)),
            64,
        )
        .expect("valid composition");
        let samples = source.poll(10).expect("poll succeeds with accounted failures");
        assert_eq!(samples.len(), 1, "only the mappable point emits");
        let stats = source.stats();
        assert_eq!(stats.acl_rejections, 1);
        assert_eq!(stats.read_failures, 0);
        assert!(
            stats
                .last_failure_reason
                .as_deref()
                .unwrap_or_default()
                .contains("WEIRD_VENDOR_FLAG"),
            "the failure reason names the cause"
        );
    }

    #[test]
    fn unreachable_adapter_and_all_points_failing_are_hard_poll_failures() {
        // NotConnected → immediate hard failure, no partial output. The
        // unreachable state is set through the fixture control handle while
        // the adapter is composed inside the read path.
        let sim = SimulatedEmsSource::new(fixture());
        let control = sim.control();
        control.set_unreachable(Some("scenario: EMS link down".to_string()));
        let mut dead = composed(sim);
        let error = dead.poll(10).expect_err("unreachable source fails hard");
        assert!(matches!(error, EdgeError::SourcePollFailed { .. }));
        assert_eq!(dead.stats().hard_poll_failures, 1);
        assert_eq!(dead.stats().read_failures, 1);

        // Every point failing with per-point errors → hard failure too
        // (zero output with failures is never a clean empty poll).
        let mut failing = fixture();
        for point in &mut failing.points {
            point.quality_per_tick = vec!["WEIRD_VENDOR_FLAG".to_string()];
        }
        let mut all_bad = composed(SimulatedEmsSource::new(failing));
        let error = all_bad.poll(10).expect_err("all-points-failed poll");
        assert!(matches!(error, EdgeError::SourcePollFailed { .. }));
        assert_eq!(all_bad.stats().acl_rejections, 2);
        assert_eq!(all_bad.stats().hard_poll_failures, 1);
    }

    #[test]
    fn composition_refuses_an_empty_point_set_and_zero_dedup() {
        let empty = AdapterIngestSource::new(
            Box::new(SimulatedEmsSource::new(fixture())),
            acl(),
            Vec::new(),
            Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS)),
            64,
        );
        assert!(
            matches!(empty, Err(IngestConfigError::EmptyPointSet)),
            "an empty point set is a composition error: {empty:?}"
        );
        let zero = AdapterIngestSource::new(
            Box::new(SimulatedEmsSource::new(fixture())),
            acl(),
            acl_test_mappings(),
            Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS)),
            0,
        );
        assert!(
            matches!(zero, Err(IngestConfigError::ZeroDedupCapacity)),
            "a zero dedup capacity is a composition error: {zero:?}"
        );
    }
}
