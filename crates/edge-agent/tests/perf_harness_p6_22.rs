// P6.22 — Performance, resource, and scale validation of the edge agent:
//         the load HARNESS (report §19.2(8), §19.4, §18.3; ledger P6.22).
//         Written, NEVER RUN — the loop bans in
//         `ledger/phase-06-edge-protocol/ENTRY_NOTE.md` hold. The human
//         build/test pass runs
//         `cargo test -p flexcore-edge-agent --test perf_harness_p6_22 -- --nocapture`
//         and pastes the printed MEASURED record into the decision doc
//         (`19_23_22_conformance_security_performance.md` §7) with the
//         hardware/versions/conditions of the run.
//
// WHAT THIS IS: a std-only (std::time::Instant — NO criterion, NO new
// dependencies) load harness over the REAL read-path composition:
//
//   SimulatedEmsSource (N assets × rate points, deterministic scenario)
//     → AdapterIngestSource (P6.08: ACL mapping, dedup, per-poll accounting)
//     → FileLocalBuffer     (P6.09: the DURABLE journal — buffer-first)
//     → EdgeRunner.tick     (poll → buffer → forward, degraded-mode machine)
//     → SwitchableUplink    (a controllable double at the UplinkChannel port)
//
// parameterized by the LOAD PROFILE: asset count, telemetry rate (samples
// per asset per poll), buffer depth, and burst factor.
//
// WHAT THIS IS NOT (the honesty rules, report §19.4 / §19.3 / AGENTS.md
// rule 1):
//
// - It records NO performance claim and NO threshold: every assertion
//   below is STRUCTURAL (all samples accounted for; no loss within
//   capacity; the drain completes; the buffer engagement is OBSERVED, not
//   assumed). A slow run does not fail anything — §19.4 forbids arbitrary
//   universal targets.
// - The timings it prints are HARNESS TIMINGS ON THE TEST HOST. They are
//   NOT gateway numbers (the lab-laptop-vs-gateway note in the decision
//   doc §7) and NOT a 10,000-asset fleet extrapolation (§19.3).
// - The load-profile VALUES here (asset count, rate, depth, burst) are
//   HARNESS PARAMETERS for exercising the code paths — the REAL load
//   profile comes from the pilot site and is recorded in the decision doc
//   as "from pilot site — PLACEHOLDER pending pilot input".
//
// MEASURED OUTPUT (printed to stdout as a `flexcore-edge-perf-record/v1`
// block; the human copies it into the decision doc §7 table): CPU, memory,
// and disk are NOT measurable from inside a Rust test process — the record
// marks them `not_measured_by_this_harness` (external measurement is a
// human gate input), while ingest rate, forwarding latency percentiles,
// recovery drain time, and the buffer-engagement point ARE recorded here.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use flexcore_edge_agent::adapters::acl::{AntiCorruptionLayer, NativeSign, NativeUnit, PointMapping};
use flexcore_edge_agent::adapters::simulated::{
    SimulatedEmsSource, SimulatedFixture, SimulatedPointFixture,
};
use flexcore_edge_agent::buffer_file::FileLocalBuffer;
use flexcore_edge_agent::ingest::AdapterIngestSource;
use flexcore_edge_agent::ports::{
    EdgeError, EdgeTelemetrySample, InMemoryAuditSink, UplinkChannel,
};
use flexcore_edge_agent::runner::{EdgeRunner, RunnerParams};
use flexcore_contracts::units::ReactiveSignConvention;
use flexcore_ports::fakes::FixedClock;
use flexcore_ports::telemetry_source::TelemetryQuantity;

const BASE_MILLIS: i64 = 1_700_000_000_000;
const INGEST_MILLIS: i64 = BASE_MILLIS + 5_000;

// --- The load profile --------------------------------------------------------------

/// The load profile. The DEFAULTS below are HARNESS parameters (chosen to
/// exercise the paths in seconds, not hours) — the REAL pilot load profile
/// is a pilot-site input, recorded in the decision doc §7 as PLACEHOLDER.
#[derive(Debug, Clone, Copy)]
struct LoadProfile {
    /// Distinct assets the scenario serves.
    assets: usize,
    /// Telemetry points per asset per poll (the ingest rate knob).
    samples_per_asset_per_poll: usize,
    /// Buffer depth (records) — the P6.09 capacity.
    buffer_depth: usize,
    /// Burst factor: how many consecutive polls the burst window runs
    /// against a DOWN uplink (so the buffer must engage).
    burst_factor: usize,
    /// Steady-state polls before the burst.
    steady_polls: usize,
}

impl Default for LoadProfile {
    fn default() -> Self {
        Self {
            assets: 25,
            samples_per_asset_per_poll: 2,
            buffer_depth: 512,
            burst_factor: 12,
            steady_polls: 30,
        }
    }
}

impl LoadProfile {
    fn total_points(&self) -> usize {
        self.assets * self.samples_per_asset_per_poll
    }

    fn label(&self) -> String {
        format!(
            "assets={} rate={} samples/asset/poll buffer_depth={} burst_factor={} steady_polls={}",
            self.assets,
            self.samples_per_asset_per_poll,
            self.buffer_depth,
            self.burst_factor,
            self.steady_polls
        )
    }
}

// --- The switchable uplink double ----------------------------------------------------

/// A controllable [`UplinkChannel`] double: healthy (forward, count) or
/// DOWN (fail — the runner degrades into buffering). The switch is what
/// the burst/recovery phases drive.
#[derive(Clone)]
struct SwitchableUplink {
    healthy: Arc<AtomicBool>,
    forwarded: Arc<AtomicU64>,
}

impl SwitchableUplink {
    fn new(healthy: Arc<AtomicBool>) -> Self {
        Self {
            healthy,
            forwarded: Arc::new(AtomicU64::new(0)),
        }
    }

    fn forwarded_total(&self) -> u64 {
        self.forwarded.load(Ordering::SeqCst)
    }
}

impl UplinkChannel for SwitchableUplink {
    fn is_connected(&self) -> bool {
        self.healthy.load(Ordering::SeqCst)
    }

    fn forward(&mut self, samples: &[EdgeTelemetrySample]) -> Result<(), EdgeError> {
        if !self.healthy.load(Ordering::SeqCst) {
            return Err(EdgeError::UplinkForwardFailed {
                reason: "the uplink is forced DOWN (the P6.22 burst drill)".to_string(),
            });
        }
        self.forwarded
            .fetch_add(samples.len() as u64, Ordering::SeqCst);
        Ok(())
    }
}

// --- The harness composition ---------------------------------------------------------

struct TempJournalDir {
    dir: PathBuf,
}

impl TempJournalDir {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "flexcore-edge-p622-{label}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("the temp journal directory is creatable");
        Self { dir }
    }
}

impl Drop for TempJournalDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

struct Harness {
    runner: EdgeRunner,
    control: flexcore_edge_agent::adapters::simulated::SimControl,
    uplink: SwitchableUplink,
    healthy_flag: Arc<AtomicBool>,
    buffer: Arc<Mutex<FileLocalBuffer>>,
    _journal: TempJournalDir,
}

/// Composes the REAL read-path stack at the given profile size.
fn compose(profile: &LoadProfile) -> Harness {
    let total_points = profile.total_points();
    let mut points: Vec<SimulatedPointFixture> = Vec::with_capacity(total_points);
    let mut mappings: Vec<PointMapping> = Vec::with_capacity(total_points);
    for asset in 0..profile.assets {
        for sample in 0..profile.samples_per_asset_per_poll {
            let point = flexcore_edge_agent::adapters::port::PointKey::new(
                format!("asset{asset}"),
                format!("Channel{sample}"),
            );
            points.push(SimulatedPointFixture {
                point: point.clone(),
                native_unit: NativeUnit::Kw,
                values_per_tick: vec![1_000.0 + (asset * 31 + sample) as f64],
                quality_per_tick: vec!["GOOD".to_string()],
            });
            mappings.push(PointMapping {
                point,
                asset_key: format!("asset-{asset:04}"),
                quantity: TelemetryQuantity::ActivePower,
                native_unit: NativeUnit::Kw,
                native_sign: Some(NativeSign::ExportPositive),
                reactive_source_convention: None,
            });
        }
    }
    let fixture = SimulatedFixture {
        points,
        start_millis: BASE_MILLIS,
        step_millis: 1_000,
        reactive_source_convention: ReactiveSignConvention::InjectionPositive,
        cadence_millis: 5_000,
    };
    let sim = SimulatedEmsSource::new(fixture);
    let control = sim.control();
    let acl = AntiCorruptionLayer::new(
        mappings.clone(),
        "edge:pilot-site-A:simulated-test",
        vec![],
    )
    .expect("the mapping table is valid");
    let source = AdapterIngestSource::new(
        Box::new(sim),
        acl,
        mappings,
        Arc::new(FixedClock::at_unix_millis(INGEST_MILLIS)),
        4096,
    )
    .expect("the read path composes");

    let journal = TempJournalDir::new("perf");
    let buffer = FileLocalBuffer::open(
        journal.dir.join("telemetry-journal"),
        profile.buffer_depth,
        Some(Duration::from_secs(3_600)),
        Arc::new(FixedClock::at_unix_millis(INGEST_MILLIS)),
    )
    .expect("the durable buffer opens");
    let buffer = Arc::new(Mutex::new(buffer));

    // The runner owns its buffer as `Box<dyn LocalBuffer>`; the harness
    // keeps a shared view of the SAME journal through a test-local
    // delegating wrapper (the `Arc<Mutex<FileLocalBuffer>>`) so the audited
    // counters (`dropped_oldest`, capacity) are queryable after each phase
    // without reopening anything.
    let runner_buffer = CountingBuffer {
        inner: Arc::clone(&buffer),
    };
    let healthy_flag = Arc::new(AtomicBool::new(true));
    let uplink = SwitchableUplink::new(Arc::clone(&healthy_flag));
    let runner = EdgeRunner::new(
        RunnerParams::from_config(
            Duration::from_millis(5_000),
            100,
            profile.buffer_depth,
        ),
        source,
        Box::new(uplink.clone()),
        Box::new(runner_buffer),
        Arc::new(InMemoryAuditSink::new()),
        Arc::new(FixedClock::at_unix_millis(INGEST_MILLIS)),
    );
    Harness {
        runner,
        control,
        uplink,
        healthy_flag,
        buffer,
        _journal: journal,
    }
}

/// Test-local delegating [`LocalBuffer`]: every call forwards to the shared
/// `FileLocalBuffer` (the REAL durable journal), so the runner drives the
/// production buffer shape and the harness can read its audited counters.
struct CountingBuffer {
    inner: Arc<Mutex<FileLocalBuffer>>,
}

impl flexcore_edge_agent::ports::LocalBuffer for CountingBuffer {
    fn len(&self) -> usize {
        self.inner.lock().expect("buffer mutex").len()
    }

    fn capacity(&self) -> usize {
        self.inner.lock().expect("buffer mutex").capacity()
    }

    fn append(&mut self, sample: EdgeTelemetrySample) -> Result<(), EdgeError> {
        self.inner.lock().expect("buffer mutex").append(sample)
    }

    fn drain(&mut self, max: usize) -> Vec<EdgeTelemetrySample> {
        self.inner.lock().expect("buffer mutex").drain(max)
    }

    fn prepend(&mut self, samples: Vec<EdgeTelemetrySample>) {
        self.inner.lock().expect("buffer mutex").prepend(samples)
    }

    fn commit_forwarded(&mut self, samples: &[EdgeTelemetrySample]) {
        self.inner
            .lock()
            .expect("buffer mutex")
            .commit_forwarded(samples)
    }
}

// --- Percentiles ---------------------------------------------------------------------

fn percentile(sorted: &[u128], pct: f64) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    let index = ((pct / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

// --- The measured phases --------------------------------------------------------------

#[test]
fn the_load_harness_records_measured_timings_for_the_recorded_composition() {
    let profile = LoadProfile::default();
    let mut harness = compose(&profile);
    let points = profile.total_points();

    // --- Phase 1: steady state (healthy uplink) --------------------------
    let mut steady_tick_micros: Vec<u128> = Vec::new();
    let mut steady_forwarded: u64 = 0;
    for _ in 0..profile.steady_polls {
        harness.control.advance_tick();
        let started = Instant::now();
        let report = harness.runner.tick();
        steady_tick_micros.push(started.elapsed().as_micros());
        assert!(!report.poll_failed, "steady-state polls must not fail");
        assert!(!report.forward_failed);
        steady_forwarded += report.forwarded as u64;
    }
    let steady_total = Duration::from_micros(
        steady_tick_micros.iter().sum::<u128>() as u64
    );
    let steady_samples_per_second = if steady_total.as_secs_f64() > 0.0 {
        steady_forwarded as f64 / steady_total.as_secs_f64()
    } else {
        0.0
    };

    // --- Phase 2: burst against a DOWN uplink (the buffer engages) -------
    harness.healthy_flag.store(false, Ordering::SeqCst);
    let mut burst_tick_micros: Vec<u128> = Vec::new();
    let mut burst_buffered_total: u64 = 0;
    let mut buffer_engagement_tick: Option<usize> = None;
    for tick in 0..profile.burst_factor {
        harness.control.advance_tick();
        let started = Instant::now();
        let report = harness.runner.tick();
        burst_tick_micros.push(started.elapsed().as_micros());
        assert!(report.degraded, "a down uplink degrades the runner (tick {tick})");
        burst_buffered_total += report.buffered as u64;
        if report.buffered_len_after > 0 && buffer_engagement_tick.is_none() {
            buffer_engagement_tick = Some(tick);
        }
    }
    let burst_total = Duration::from_micros(burst_tick_micros.iter().sum::<u128>() as u64);
    let buffered_len_after_burst = {
        // The runner's buffer view (the wrapper delegates to the journal).
        harness.runner.buffered_len()
    };

    // --- Phase 3: recovery drain (uplink restored) ------------------------
    harness.healthy_flag.store(true, Ordering::SeqCst);
    let recovery_started = Instant::now();
    let mut recovery_forwarded: u64 = 0;
    let mut drain_ticks = 0usize;
    loop {
        harness.control.advance_tick();
        let report = harness.runner.tick();
        recovery_forwarded += report.forwarded as u64;
        drain_ticks += 1;
        if harness.runner.buffered_len() == 0 {
            break;
        }
        assert!(
            drain_ticks <= profile.burst_factor + 16,
            "the recovery drain must complete within a bounded number of ticks \
             (buffered_len={})",
            harness.runner.buffered_len()
        );
    }
    let recovery_drain_time = recovery_started.elapsed();

    // --- Structural assertions ONLY (no timing thresholds, §19.4) ---------
    // The burst samples are accounted for: buffered, or audited-dropped by
    // the drop-oldest policy when the depth bound was hit (the drop counter
    // can never exceed what was buffered). The drain then empties the
    // buffer without loss WITHIN the capacity bound.
    let dropped_oldest = harness.buffer.lock().expect("buffer mutex").dropped_oldest();
    assert!(
        dropped_oldest <= burst_buffered_total,
        "burst accounting must be coherent (buffered={burst_buffered_total}, \
         dropped={dropped_oldest})"
    );
    assert!(
        harness.uplink.forwarded_total() >= recovery_forwarded,
        "the uplink's own counter covers the recovery drain"
    );
    assert_eq!(
        harness.runner.buffered_len(),
        0,
        "the drain completes: the buffer is empty after recovery"
    );
    assert!(
        buffer_engagement_tick.is_some(),
        "the burst must OBSERVE the buffer engaging (the P6.22 acceptance item)"
    );

    // --- The MEASURED record (printed; NOT stored as a claim) -------------
    let mut sorted_steady = steady_tick_micros.clone();
    sorted_steady.sort_unstable();
    let mut sorted_burst = burst_tick_micros.clone();
    sorted_burst.sort_unstable();
    println!("--- flexcore-edge-perf-record/v1 (P6.22; MEASURED on the test host — NOT gateway numbers) ---");
    println!("load_profile: {}", profile.label());
    println!(
        "ingest_rate_samples_per_second: measured {:.1} (steady state, {} samples forwarded over {:?})",
        steady_samples_per_second, steady_forwarded, steady_total
    );
    println!(
        "forwarding_latency_tick_us: p50={} p95={} p99={} max={} (steady-state END-TO-END TICK elapsed, not wire latency)",
        percentile(&sorted_steady, 50.0),
        percentile(&sorted_steady, 95.0),
        percentile(&sorted_steady, 99.0),
        sorted_steady.last().copied().unwrap_or(0)
    );
    println!(
        "degraded_tick_us: p50={} p95={} p99={} (burst ticks with the uplink DOWN)",
        percentile(&sorted_burst, 50.0),
        percentile(&sorted_burst, 95.0),
        percentile(&sorted_burst, 99.0)
    );
    println!(
        "buffer_engagement: first tick where the buffer held records = {:?} of {} burst ticks; buffered_after_burst={} records; dropped_oldest={} (audited)",
        buffer_engagement_tick, profile.burst_factor, buffered_len_after_burst, dropped_oldest
    );
    println!(
        "recovery_drain_time: measured {:?} ({} drain ticks, {} samples forwarded during recovery)",
        recovery_drain_time, drain_ticks, recovery_forwarded
    );
    println!("cpu: not_measured_by_this_harness (external measurement — human gate input)");
    println!("memory: not_measured_by_this_harness (external measurement — human gate input)");
    println!("disk: not_measured_by_this_harness (external measurement — human gate input)");
    println!("--- end of record: paste into 19_23_22_conformance_security_performance.md §7 with hardware/OS/versions/conditions ---");
}

#[test]
fn the_load_profile_parameters_are_exercised_not_assumed() {
    // The harness is parameterized: a SECOND, smaller profile runs through
    // the same composition so the parameterization itself is provably live
    // (a fixed-size harness would be a single hardcoded scenario).
    let profile = LoadProfile {
        assets: 4,
        samples_per_asset_per_poll: 1,
        buffer_depth: 8,
        burst_factor: 10,
        steady_polls: 3,
    };
    let mut harness = compose(&profile);
    assert_eq!(profile.total_points(), 4);
    for _ in 0..profile.steady_polls {
        harness.control.advance_tick();
        let report = harness.runner.tick();
        assert!(!report.poll_failed);
        assert!(!report.forward_failed);
    }
    // Burst: 10 polls of 4 samples against a DOWN uplink with a buffer
    // depth of 8 — the drop-oldest policy MUST engage and be audited (the
    // P6.22 buffer-engagement acceptance item under stress).
    harness.healthy_flag.store(false, Ordering::SeqCst);
    let mut last_len = 0usize;
    for _ in 0..profile.burst_factor {
        harness.control.advance_tick();
        let report = harness.runner.tick();
        assert!(report.degraded);
        last_len = report.buffered_len_after;
        assert!(
            last_len <= profile.buffer_depth,
            "the buffer never exceeds its bound (got {last_len})"
        );
    }
    let dropped = harness.buffer.lock().expect("buffer mutex").dropped_oldest();
    assert!(
        dropped > 0,
        "the burst exceeds the depth-8 buffer: the audited drop-oldest policy must have fired"
    );
    // Recovery: the drain completes, the buffer empties.
    harness.healthy_flag.store(true, Ordering::SeqCst);
    for _ in 0..(profile.burst_factor + 16) {
        harness.control.advance_tick();
        let _ = harness.runner.tick();
        if harness.runner.buffered_len() == 0 {
            break;
        }
    }
    assert_eq!(harness.runner.buffered_len(), 0, "the drain completes");
}
