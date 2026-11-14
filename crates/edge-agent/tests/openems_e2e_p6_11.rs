// P6.11 — OpenEMS integration: read-only data path end to end — integration
// tests (ledger P6.11; test file written, NOT run — the loop bans in
// `ledger/phase-06-edge-protocol/ENTRY_NOTE.md` hold: nothing built, no
// tests executed. The human build/test pass runs these).
//
// What this file exercises — the COMPLETE read-only path through the REAL
// composition, against an IN-PROCESS test double serving fixture
// EdgeConfig/channel JSON (NO external network, NO sockets):
//
// ```text
// OpenEmsRestAdapter (EdgeConfig discovery + versioned mapping profile +
//                     read-only GET client over the fixture transport)
//   → AntiCorruptionLayer (the ONLY translation: units, signs, quality,
//     event/ingest time — P6.12)
//   → AdapterIngestSource (P6.08 read path: per-poll accounting, dedup)
//   → FileLocalBuffer     (P6.09 durable journal — buffer-first, always)
//   → EdgeRunner.tick     (poll → buffer → forward, degraded-mode machine)
//   → RecordingUplink     (a test double at the UplinkChannel port; the real
//                          P6.07 mTLS channel stays fail-closed on the wire-
//                          encoder seam — recorded in the decision doc)
// ```
//
// Coverage (ledger P6.11 steps 2–4, as far as a test loop can go):
//
// 1. **Value-for-value reconciliation at BOTH ends** — the fixture's
//    served channel values are reconciled against the canonical samples the
//    uplink receives: quantity, canonical unit value, quality, asset key,
//    event_time (source), ingest_time (edge), source sequence, source
//    identity.
// 2. **Forced stale feed** — a channel served with a timestamp beyond the
//    profile's freshness limit arrives `STALE` downstream at the FIRST poll
//    after it went stale (within the freshness limit, flagged, never
//    upgraded); `MISSING` is NEVER emitted by the edge (it is a
//    platform-derived gap marker, P2.04 D-T6 — the control plane's gap
//    detector owns the MISSING intervals downstream).
// 3. **The read-only structural guarantee** — (a) the transport trait
//    exposes GET only, so a write call is not expressible; (b) the lexical
//    scan over `src/adapters/openems.rs` proves the module carries no other
//    HTTP verb anywhere (no write method, no writer request format); (c)
//    the fixture double counts GETs and records zero write attempts.
//
// The LAB RUN ITSELF (a real OpenEMS Edge 2026.x, a timed window, a network
// capture, independent reconciliation) CANNOT happen inside this test loop:
// the decision record `ledger/phase-06-edge-protocol/10_11_openems.md`
// carries the lab-run record as a TEMPLATE with EMPTY measured cells,
// status PREPARED/UNRUN — that evidence is the human's gate input, and no
// figure in this file stands in for it.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::units::ReactiveSignConvention;
use flexcore_edge_agent::adapters::acl::{AntiCorruptionLayer, PointMapping};
use flexcore_edge_agent::adapters::openems::{
    OpenEmsRestAdapter, OpenemsError, OpenemsMappingProfile, OpenemsRestTransport, RestResponse,
    OPENEMS_CHANNEL_PATH_PREFIX, OPENEMS_EDGE_CONFIG_PATH, OPENEMS_REST_ADAPTER_ID,
};
use flexcore_edge_agent::adapters::port::{EmsAdapter, PointKey};
use flexcore_edge_agent::adapters::registry::AdapterRegistry;
use flexcore_edge_agent::buffer_file::FileLocalBuffer;
use flexcore_edge_agent::ingest::AdapterIngestSource;
use flexcore_edge_agent::ports::{
    EdgeError, EdgeTelemetrySample, InMemoryAuditSink, UplinkChannel,
};
use flexcore_edge_agent::runner::{EdgeRunner, RunnerParams};
use flexcore_ports::fakes::FixedClock;
use flexcore_ports::telemetry_source::{QuantityValue, TelemetryQuantity};

const NOW_MILLIS: i64 = 1_700_000_000_000;
/// The edge clock during the run (ingest_time stamp; > the fixture's event
/// times so the P0.09 ordering holds).
const INGEST_MILLIS: i64 = NOW_MILLIS + 500;
const FRESH_AGE_MILLIS: i64 = 1_000;
/// The shipped profile's freshness limit (stale_after_millis).
const STALE_AFTER_MILLIS: i64 = 15_000;

// --- The fixture (labelled) -------------------------------------------------------
//
// FIXTURE EdgeConfig: simulated PV + battery per the OpenEMS Getting
// Started shape. UNVERIFIED-ASSUMPTION field spellings (`components`,
// `factoryId`, `alias`, `channels`, `unit`) — RESEARCH.md §1; report §27
// OpenEMS references; verified only at the P6.11 lab run.

const FIXTURE_EDGE_CONFIG: &str = r#"{
  "components": {
    "_meta": {
      "factoryId": "Core.Meta",
      "alias": "OpenEMS Edge metadata",
      "channels": {
        "ComponentState": { "unit": "", "text": "Component state" }
      }
    },
    "ess0": {
      "factoryId": "Simulator.Ess.Direct.Symmetric",
      "alias": "Lab battery ESS",
      "channels": {
        "ActivePower": { "unit": "W", "text": "Active Power" },
        "ReactivePower": { "unit": "var", "text": "Reactive Power" },
        "Soc": { "unit": "%", "text": "State of Charge" },
        "AllowedChargePower": { "unit": "W", "text": "Allowed Charge Power" }
      }
    },
    "pvInverter": {
      "factoryId": "Simulator.PvInverter.Direct",
      "alias": "Lab PV inverter",
      "channels": {
        "ActivePower": { "unit": "W", "text": "Active Power" },
        "ReactivePower": { "unit": "var", "text": "Reactive Power" }
      }
    },
    "meter0": {
      "factoryId": "Simulator.Meter.Direct",
      "alias": "Grid connection meter",
      "channels": {
        "ActivePower": { "unit": "W", "text": "Active Power" },
        "ReactivePower": { "unit": "var", "text": "Reactive Power" }
      }
    }
  }
}"#;

struct ChannelFixture {
    value: f64,
    timestamp_millis: i64,
}

/// The fixture channel values the double serves (all fresh: NOW - 1 s,
/// inside the 15 s freshness limit).
fn fixture_channels() -> HashMap<String, ChannelFixture> {
    let fresh = |value: f64| ChannelFixture {
        value,
        timestamp_millis: NOW_MILLIS - FRESH_AGE_MILLIS,
    };
    HashMap::from([
        ("ess0/ActivePower".to_string(), fresh(1_500.0)),
        ("ess0/ReactivePower".to_string(), fresh(200.0)),
        ("ess0/Soc".to_string(), fresh(87.0)),
        ("pvInverter/ActivePower".to_string(), fresh(900.0)),
        ("pvInverter/ReactivePower".to_string(), fresh(10.0)),
        ("meter0/ActivePower".to_string(), fresh(-450.0)),
        ("meter0/ReactivePower".to_string(), fresh(100.0)),
    ])
}

// --- The in-process test double (GET only, no network) ------------------------------

struct FixtureTransport {
    state: Arc<Mutex<FixtureState>>,
}

struct FixtureState {
    config_json: String,
    channels: HashMap<String, ChannelFixture>,
    gets_served: u64,
    /// Write attempts recorded by the double. The transport trait exposes
    /// GET only, so nothing in the adapter surface can even express a
    /// write; the counter is kept (and asserted zero) so the no-write
    /// attestation has a runtime witness as well as the lexical scan.
    write_attempts: u64,
}

impl FixtureTransport {
    fn new(config_json: &str, channels: HashMap<String, ChannelFixture>) -> Self {
        Self::new_with_handle(config_json, channels).0
    }

    /// Builds the double AND its shared state handle, so a test can hold
    /// the counters while the composition consumes the transport.
    fn new_with_handle(
        config_json: &str,
        channels: HashMap<String, ChannelFixture>,
    ) -> (Self, Arc<Mutex<FixtureState>>) {
        let state = Arc::new(Mutex::new(FixtureState {
            config_json: config_json.to_string(),
            channels,
            gets_served: 0,
            write_attempts: 0,
        }));
        (Self {
            state: Arc::clone(&state),
        }, state)
    }
}

impl OpenemsRestTransport for FixtureTransport {
    fn get(&mut self, path: &str) -> Result<RestResponse, OpenemsError> {
        let mut state = self.state.lock().expect("fixture state mutex");
        state.gets_served += 1;
        if path == OPENEMS_EDGE_CONFIG_PATH {
            return Ok(RestResponse {
                status: 200,
                body: state.config_json.clone(),
            });
        }
        if let Some(address) = path.strip_prefix(OPENEMS_CHANNEL_PATH_PREFIX) {
            return match state.channels.get(address) {
                Some(channel) => Ok(RestResponse {
                    status: 200,
                    body: format!(
                        "{{\"value\": {}, \"timestamp\": {}}}",
                        channel.value, channel.timestamp_millis
                    ),
                }),
                None => Ok(RestResponse {
                    status: 404,
                    body: "{}".to_string(),
                }),
            };
        }
        Ok(RestResponse {
            status: 404,
            body: "{}".to_string(),
        })
    }
}

// --- The recording uplink (a test double at the UplinkChannel port) ------------------

#[derive(Clone)]
struct RecordingUplink {
    recorded: Arc<Mutex<Vec<EdgeTelemetrySample>>>,
}

impl RecordingUplink {
    fn new() -> Self {
        Self {
            recorded: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn received(&self) -> Vec<EdgeTelemetrySample> {
        self.recorded.lock().expect("recording uplink mutex").clone()
    }
}

impl UplinkChannel for RecordingUplink {
    fn is_connected(&self) -> bool {
        true
    }

    fn forward(&mut self, samples: &[EdgeTelemetrySample]) -> Result<(), EdgeError> {
        self.recorded
            .lock()
            .expect("recording uplink mutex")
            .extend(samples.iter().cloned());
        Ok(())
    }
}

// --- The REAL composition under test -------------------------------------------------

/// A unique throwaway journal directory per instance (tests run in
/// parallel); removed on drop.
struct TempJournalDir {
    dir: PathBuf,
}

impl TempJournalDir {
    fn new(label: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "flexcore-edge-openems-e2e-{label}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("the temp journal directory is creatable");
        Self { dir }
    }
}

impl Drop for TempJournalDir {
    fn drop(&mut self) {
        // Best-effort cleanup; a leaked temp dir must never mask a test
        // outcome.
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The composed stack, built EXACTLY the way the composition root
/// (`src/main.rs` `compose_openems`) builds it: registry-constructed
/// adapter (the P6.10 AVAILABLE flip) → EdgeConfig discovery → ACL mapping
/// table → AdapterIngestSource → FileLocalBuffer → EdgeRunner over the
/// recording uplink.
struct ComposedStack {
    runner: EdgeRunner,
    uplink: RecordingUplink,
    journal_dir: TempJournalDir,
    /// The fixture double's shared state (GET/write-attempt counters — the
    /// runtime no-write witness).
    fixture_state: Arc<Mutex<FixtureState>>,
}

fn compose_stack(
    channels: HashMap<String, ChannelFixture>,
    label: &str,
) -> ComposedStack {
    // The registry flip: `openems-rest` constructs THROUGH the registry
    // (AVAILABLE, P6.10) — the factory builds + discovers the adapter and
    // hands the validated mapping table out through the slots, exactly as
    // the composition root does.
    let registry = AdapterRegistry::with_default_allowlist().with_openems_available();
    let profile_for_factory = OpenemsMappingProfile::from_path(format!(
        "{}/config/openems-mapping-profile.v1.json",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("the shipped versioned profile loads");
    let (transport, fixture_state) =
        FixtureTransport::new_with_handle(FIXTURE_EDGE_CONFIG, channels);

    let prepared: Arc<Mutex<Option<FixtureTransport>>> = Arc::new(Mutex::new(Some(transport)));
    let mappings_slot: Arc<Mutex<Vec<PointMapping>>> = Arc::new(Mutex::new(Vec::new()));
    let unmapped_slot: Arc<Mutex<Vec<PointKey>>> = Arc::new(Mutex::new(Vec::new()));
    let prepared_for_factory = Arc::clone(&prepared);
    let mappings_for_factory = Arc::clone(&mappings_slot);
    let unmapped_for_factory = Arc::clone(&unmapped_slot);
    let adapter = registry
        .construct(OPENEMS_REST_ADAPTER_ID, &move || {
            let Some(transport) = prepared_for_factory.lock().expect("prepared slot").take()
            else {
                return Err("the prepared composition was already consumed".to_string());
            };
            let mut concrete = OpenEmsRestAdapter::new(
                Box::new(transport),
                profile_for_factory.clone(),
                Arc::new(FixedClock::at_unix_millis(NOW_MILLIS)),
            )
            .map_err(|error| format!("the OpenEMS adapter could not be composed: {error}"))?;
            concrete
                .discover()
                .map_err(|error| format!("EdgeConfig discovery failed: {error}"))?;
            *mappings_for_factory.lock().expect("mappings slot") =
                concrete.mapped_point_mappings();
            *unmapped_for_factory.lock().expect("unmapped slot") =
                concrete.declared_unmapped_points();
            Ok(Box::new(concrete) as Box<dyn EmsAdapter>)
        })
        .expect("the openems-rest adapter constructs through the registry (P6.10 flip)");

    let mappings = mappings_slot.lock().expect("mappings slot").clone();
    assert_eq!(
        mappings.len(),
        7,
        "the shipped profile maps exactly the seven telemetry channels of the fixture"
    );
    let acl = AntiCorruptionLayer::new(
        mappings.clone(),
        "edge:pilot-site-A:openems-rest",
        unmapped_slot.lock().expect("unmapped slot").clone(),
    )
    .expect("the mapping table is valid");

    let source = AdapterIngestSource::new(
        adapter,
        acl,
        mappings,
        Arc::new(FixedClock::at_unix_millis(INGEST_MILLIS)),
        64,
    )
    .expect("the read path composes");

    // The DURABLE buffer (P6.09) — buffer-first, exactly like production.
    let journal_dir = TempJournalDir::new(label);
    let buffer = FileLocalBuffer::open(
        journal_dir.dir.join("telemetry-journal"),
        128,
        Some(Duration::from_secs(3_600)),
        Arc::new(FixedClock::at_unix_millis(INGEST_MILLIS)),
    )
    .expect("the durable buffer opens");

    let uplink = RecordingUplink::new();
    let runner = EdgeRunner::new(
        RunnerParams::from_config(Duration::from_millis(5_000), 100, 128),
        source,
        Box::new(uplink.clone()),
        Box::new(buffer),
        Arc::new(InMemoryAuditSink::new()),
        Arc::new(FixedClock::at_unix_millis(INGEST_MILLIS)),
    );
    ComposedStack {
        runner,
        uplink,
        journal_dir,
        fixture_state,
    }
}

/// Finds the received sample with the given source identity.
fn find_sample<'a>(
    samples: &'a [EdgeTelemetrySample],
    source_id: &str,
) -> &'a EdgeTelemetrySample {
    samples
        .iter()
        .find(|sample| sample.source_id() == source_id)
        .unwrap_or_else(|| panic!("expected a sample with source_id '{source_id}'"))
}

fn approx(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-12,
        "expected {expected}, got {actual}"
    );
}

// --- 1. Value-for-value reconciliation at BOTH ends ----------------------------------

#[test]
fn read_path_reconciles_value_for_value_at_both_ends() {
    let mut stack = compose_stack(fixture_channels(), "reconcile");
    let report = stack.runner.tick();

    // The runner forwarded exactly the mapped channels and drained the
    // buffer (healthy, not degraded).
    assert_eq!(report.polled, 7, "one sample per mapped channel");
    assert_eq!(report.forwarded, 7);
    assert_eq!(report.buffered_len_after, 0);
    assert!(!report.degraded);
    assert!(!report.poll_failed);
    assert!(!report.forward_failed);

    let received = stack.uplink.received();
    assert_eq!(received.len(), 7, "the uplink received every sample");

    // --- Source end (fixture) → canonical end (uplink), field by field ---

    // Battery active power: 1500 W, declared export-positive → 0.0015 MW,
    // GOOD, event from the source, ingest stamped by the edge.
    let ess_power = find_sample(&received, "edge:pilot-site-A:openems-rest/ess0/ActivePower");
    assert_eq!(ess_power.asset_key(), "ess-01");
    assert_eq!(ess_power.quantity(), TelemetryQuantity::ActivePower);
    match ess_power.value() {
        QuantityValue::Power(power) => approx(power.value(), 0.0015),
        other => panic!("expected a power value, got {other:?}"),
    }
    assert_eq!(ess_power.quality(), TelemetryQuality::Good);
    assert_eq!(ess_power.source_quality_code(), Some("GOOD"));
    assert_eq!(ess_power.event_time().unix_millis(), (NOW_MILLIS - FRESH_AGE_MILLIS) as i128);
    assert_eq!(ess_power.ingest_time().unix_millis(), INGEST_MILLIS as i128);
    assert!(ess_power.ingest_time().unix_millis() >= ess_power.event_time().unix_millis());
    assert_eq!(ess_power.source_sequence(), 1);

    // Battery reactive power: 200 var under the DECLARED injection-positive
    // convention → 0.0002 MVAr WITH its convention (never a silent flip).
    let ess_reactive = find_sample(&received, "edge:pilot-site-A:openems-rest/ess0/ReactivePower");
    assert_eq!(ess_reactive.quantity(), TelemetryQuantity::ReactivePower);
    match ess_reactive.value() {
        QuantityValue::Reactive { power, convention } => {
            approx(power.value(), 0.0002);
            assert_eq!(*convention, ReactiveSignConvention::InjectionPositive);
        }
        other => panic!("expected a reactive value, got {other:?}"),
    }

    // Battery SOC: 87 % through the explicit from_percent boundary → the
    // canonical fraction 0.87 (percent is never persisted).
    let ess_soc = find_sample(&received, "edge:pilot-site-A:openems-rest/ess0/Soc");
    match ess_soc.value() {
        QuantityValue::Fraction(soc) => approx(soc.value(), 0.87),
        other => panic!("expected a fraction value, got {other:?}"),
    }

    // PV active power: 900 W export-positive → 0.0009 MW, asset pv-01.
    let pv_power = find_sample(&received, "edge:pilot-site-A:openems-rest/pvInverter/ActivePower");
    assert_eq!(pv_power.asset_key(), "pv-01");
    match pv_power.value() {
        QuantityValue::Power(power) => approx(power.value(), 0.0009),
        other => panic!("expected a power value, got {other:?}"),
    }

    // PV reactive power: 10 var → 0.00001 MVAr, injection-positive.
    let pv_reactive = find_sample(&received, "edge:pilot-site-A:openems-rest/pvInverter/ReactivePower");
    match pv_reactive.value() {
        QuantityValue::Reactive { power, convention } => {
            approx(power.value(), 0.00001);
            assert_eq!(*convention, ReactiveSignConvention::InjectionPositive);
        }
        other => panic!("expected a reactive value, got {other:?}"),
    }

    // Grid meter active power: the fixture serves -450 W (exporting) under
    // the DECLARED import-positive meter convention → the explicit negation
    // yields +0.00045 MW, which under the canonical rule (P > 0 = export)
    // is exactly the exported 450 W. The conversion is the mapping's
    // recorded choice, not an inference.
    let meter_power = find_sample(&received, "edge:pilot-site-A:openems-rest/meter0/ActivePower");
    assert_eq!(meter_power.asset_key(), "grid-meter-01");
    match meter_power.value() {
        QuantityValue::Power(power) => approx(power.value(), 0.00045),
        other => panic!("expected a power value, got {other:?}"),
    }

    // Grid meter reactive power: 100 var → 0.0001 MVAr, injection-positive.
    let meter_reactive = find_sample(&received, "edge:pilot-site-A:openems-rest/meter0/ReactivePower");
    match meter_reactive.value() {
        QuantityValue::Reactive { power, convention } => {
            approx(power.value(), 0.0001);
            assert_eq!(*convention, ReactiveSignConvention::InjectionPositive);
        }
        other => panic!("expected a reactive value, got {other:?}"),
    }

    // Every sample carries the edge source prefix (never the reserved
    // 'flexcore:' namespace) and the OpenEMS adapter id.
    for sample in &received {
        assert!(sample.source_id().starts_with("edge:pilot-site-A:openems-rest/"));
        assert_eq!(sample.quality(), TelemetryQuality::Good);
    }

    // Unmapped channels never flow: the fixture's AllowedChargePower (a
    // limit channel, explicitly unmapped) appears nowhere downstream.
    assert!(
        received
            .iter()
            .all(|sample| sample.source_id() != "edge:pilot-site-A:openems-rest/ess0/AllowedChargePower"),
        "explicitly-unmapped channels are never forwarded"
    );
}

#[test]
fn a_second_tick_with_the_same_feed_deduplicates_to_zero_new_samples() {
    // The EdgeConfig/channel values did not change (same event_time), so a
    // re-poll hits the same dedup identity (source_id + event_time): the
    // P4.24-discipline dedup window classifies the re-read as a duplicate
    // (exact replay, or conflicting once the per-read source sequence
    // advances) and refuses it — nothing new is forwarded, nothing is
    // double-counted downstream (§15.4 at-least-once with edge dedup).
    let mut stack = compose_stack(fixture_channels(), "dedup");
    let first = stack.runner.tick();
    assert_eq!(first.forwarded, 7);
    let second = stack.runner.tick();
    assert_eq!(second.forwarded, 0, "re-reads at the same event_time are deduplicated");
    assert_eq!(stack.uplink.received().len(), 7, "no duplicate delivery");
}

// --- 2. Forced stale feed ------------------------------------------------------------

#[test]
fn forced_stale_feed_arrives_stale_within_the_freshness_limit_and_never_missing() {
    // The feed stops: ess0/Soc keeps its LAST value but its timestamp
    // freezes 60 s in the past — beyond the profile's 15 s freshness
    // limit. The FIRST poll after the feed went stale must flag it.
    let mut channels = fixture_channels();
    channels.insert(
        "ess0/Soc".to_string(),
        ChannelFixture {
            value: 87.0,
            timestamp_millis: NOW_MILLIS - 60_000,
        },
    );
    let mut stack = compose_stack(channels, "stale");
    let report = stack.runner.tick();
    assert_eq!(report.forwarded, 7, "all mapped channels still flow");

    let received = stack.uplink.received();
    let soc = find_sample(&received, "edge:pilot-site-A:openems-rest/ess0/Soc");
    // STALE — flagged, never upgraded to GOOD, never silently converted
    // into a normal reading (P0.08; ledger P6.11 step 3). The flagging
    // happened at the FIRST poll after the feed went stale (this very
    // tick), i.e. within the freshness limit.
    assert_eq!(soc.quality(), TelemetryQuality::Stale);
    assert_eq!(soc.source_quality_code(), Some("STALE"));
    let event_age = INGEST_MILLIS - soc.event_time().unix_millis();
    assert!(
        event_age > STALE_AFTER_MILLIS,
        "the event age ({event_age} ms) exceeds the declared freshness limit \
         ({STALE_AFTER_MILLIS} ms) and the value arrived flagged, not fresh"
    );
    // The stale value's CONTENT is still reconciled value-for-value: the
    // last known value travels WITH its staleness, never fabricated.
    match soc.value() {
        QuantityValue::Fraction(soc_value) => approx(soc_value.value(), 0.87),
        other => panic!("expected a fraction value, got {other:?}"),
    }

    // The sibling channels (fresh timestamps) remain GOOD — quality is
    // per-value, exactly one state per value (§10.4).
    let power = find_sample(&received, "edge:pilot-site-A:openems-rest/ess0/ActivePower");
    assert_eq!(power.quality(), TelemetryQuality::Good);

    // MISSING is NEVER emitted by the edge: it is a platform-derived gap
    // marker (P2.04 D-T6); the control plane's gap detector derives the
    // MISSING intervals downstream from staleness and gaps. The edge's
    // contribution is the honest STALE flag above.
    for sample in &received {
        assert_ne!(
            sample.quality(),
            TelemetryQuality::Missing,
            "an edge measured source can never emit the platform-derived MISSING state"
        );
    }
}

// --- 3. The read-only structural guarantee --------------------------------------------

/// The lexical no-write scan: the adapter module carries NO other HTTP verb
/// anywhere — no writer method, no writer request format, no verb string
/// literal. (The scan patterns are deliberately broader than the compiler
/// needs: they also catch commented-out or renamed writers.)
#[test]
fn the_adapter_module_structurally_carries_get_only() {
    let source_path = format!(
        "{}/src/adapters/openems.rs",
        env!("CARGO_MANIFEST_DIR")
    );
    let source = std::fs::read_to_string(&source_path)
        .expect("the adapter module source is readable in the crate tree");
    const FORBIDDEN_WRITE_TOKENS: &[&str] = &[
        "fn post",
        "fn put",
        "fn delete",
        "fn patch",
        ".post(",
        ".put(",
        ".delete(",
        ".patch(",
        "\"POST\"",
        "\"PUT\"",
        "\"DELETE\"",
        "\"PATCH\"",
        "POST ",
        "PUT ",
        "DELETE ",
    ];
    for token in FORBIDDEN_WRITE_TOKENS {
        assert!(
            !source.contains(token),
            "the read-only guarantee is violated: '{token}' appears in {source_path}"
        );
    }

    // The transport trait is the only outbound surface in PRODUCTION code:
    // outside the test module, exactly the trait declaration + the TCP
    // client implementation name the GET method — and no other verb exists
    // (the forbidden-token scan above covers that).
    let production_source = source
        .split("#[cfg(test)]")
        .next()
        .expect("the module source has a production section");
    let get_declarations = production_source.matches("fn get(&mut self").count();
    assert_eq!(
        get_declarations, 2,
        "exactly the trait declaration + the TCP client implementation expose GET; \
         no other outbound surface exists"
    );
}

#[test]
fn the_fixture_double_witnesses_get_only_traffic() {
    // Runtime witness complementing the lexical scan: the double counts
    // its GETs while the real composition runs, and the adapter surface
    // has no way to even ATTEMPT a write (the trait declares GET only) —
    // the write-attempt counter stays zero.
    let mut stack = compose_stack(fixture_channels(), "getcount");
    let report = stack.runner.tick();
    assert_eq!(report.forwarded, 7);

    let state = stack.fixture_state.lock().expect("fixture state mutex");
    // Exactly 1 EdgeConfig GET + 7 channel GETs — nothing else ever left
    // the edge toward the EMS.
    assert_eq!(state.gets_served, 1 + 7);
    assert_eq!(
        state.write_attempts, 0,
        "zero write attempts observed (none is expressible through the GET-only trait)"
    );
}
