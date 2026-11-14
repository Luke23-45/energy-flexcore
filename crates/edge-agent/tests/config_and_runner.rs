// P6.05 — Edge agent config and runner tests (ledger P6.05; test file
// written, NOT run — the loop bans in
// `ledger/phase-06-edge-protocol/ENTRY_NOTE.md` hold: nothing built, no
// tests executed. The human build/test pass runs these).
//
// Coverage:
//
// 1. **Config parsing defaults** — the command-path flag defaults to FALSE
//    (`FLEXCORE_EDGE_COMMAND_PATH_ENABLED` unset ⇒ `false`); buffer path,
//    capacity, batch size, and cadence take their documented defaults;
//    out-of-range and malformed values are named errors.
// 2. **Fail-closed on missing config** — each required variable absent ⇒
//    `ConfigError::MissingEnv` naming the variable (never a default
//    site/tenant/endpoint/adapter).
// 3. **Runner start/stop with in-memory fakes** — the supervision loop
//    (async `run`) starts, ticks against scripted fakes, and stops on the
//    shutdown watch, retaining buffered samples.
// 4. **Fail-closed port behaviour** — the un-configured source/uplink stubs
//    error on every call; a fail-closed composition keeps buffering (never
//    fabricating, never silently passing).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{EventTime, IngestTime};
use flexcore_contracts::units::ActivePowerMW;
use flexcore_edge_agent::config::{ConfigError, EdgeConfig};
use flexcore_edge_agent::ports::{
    EdgeError, EdgeTelemetrySample, EdgeTelemetrySampleParts, FailClosedEmsSource,
    FailClosedUplink, InMemoryAuditSink, InMemoryLocalBuffer, LocalBuffer,
};
use flexcore_edge_agent::runner::{EdgeRunner, RunnerParams, TickReport};
use flexcore_ports::clock::Clock;
use flexcore_ports::fakes::FixedClock;
use flexcore_ports::telemetry_source::{QuantityValue, TelemetryQuantity};
use tokio::sync::watch;

/// The environment is process-global: every env-dependent test takes this
/// lock so the required-variable cases run serially.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn set_env(name: &str, value: &str) {
    // SAFETY (test-only, under ENV_LOCK): the edge configuration variables
    // are process-scoped test inputs; no other thread reads them while the
    // lock is held.
    unsafe { std::env::set_var(name, value) };
}

fn remove_env(name: &str) {
    unsafe { std::env::remove_var(name) };
}

const REQUIRED: [&str; 4] = [
    "FLEXCORE_EDGE_SITE_ID",
    "FLEXCORE_EDGE_TENANT_ID",
    "FLEXCORE_EDGE_CONTROL_PLANE_ENDPOINT",
    "FLEXCORE_EDGE_ADAPTER_ID",
];

fn with_required_env<F: FnOnce()>(test: F) {
    let _guard = ENV_LOCK.lock().expect("env lock");
    set_env("FLEXCORE_EDGE_SITE_ID", "pilot-site-A");
    set_env("FLEXCORE_EDGE_TENANT_ID", "tenant-demo");
    set_env("FLEXCORE_EDGE_CONTROL_PLANE_ENDPOINT", "wss://central.example/edge");
    set_env("FLEXCORE_EDGE_ADAPTER_ID", "openems-rest");
    // Clear the optionals so defaults are exercised.
    for optional in [
        "FLEXCORE_EDGE_BUFFER_PATH",
        "FLEXCORE_EDGE_COMMAND_PATH_ENABLED",
        "FLEXCORE_EDGE_POLL_INTERVAL_MS",
        "FLEXCORE_EDGE_BUFFER_CAPACITY",
        "FLEXCORE_EDGE_FORWARD_BATCH_SIZE",
        "FLEXCORE_EDGE_LOG_LEVEL",
    ] {
        remove_env(optional);
    }
    test();
    for name in REQUIRED {
        remove_env(name);
    }
}

// --- 1. Config parsing defaults ---------------------------------------------

#[test]
fn defaults_command_path_false_and_documented_defaults() {
    with_required_env(|| {
        let config = EdgeConfig::from_env().expect("required vars are set");
        // THE default this wave exists to prove: the command path is OFF.
        assert!(
            !config.command_path_enabled,
            "the command path MUST default to false (v0.4.0 read-only default, §21.4)"
        );
        assert_eq!(config.site_id, "pilot-site-A");
        assert_eq!(config.tenant_id, "tenant-demo");
        assert_eq!(config.adapter_id, "openems-rest");
        assert_eq!(config.buffer_path, std::path::PathBuf::from("edge-buffer"));
        assert_eq!(config.poll_interval, Duration::from_millis(5_000));
        assert_eq!(config.buffer_capacity, 10_000);
        assert_eq!(config.forward_batch_size, 100);
    });
}

#[test]
fn explicit_false_and_true_parse_but_defaults_hold_false() {
    with_required_env(|| {
        set_env("FLEXCORE_EDGE_COMMAND_PATH_ENABLED", "false");
        let config = EdgeConfig::from_env().expect("parses");
        assert!(!config.command_path_enabled);
        remove_env("FLEXCORE_EDGE_COMMAND_PATH_ENABLED");
        // `true` parses (recording an operator intent) — but per the config
        // contract nothing activates: no receiver exists in this wave.
        set_env("FLEXCORE_EDGE_COMMAND_PATH_ENABLED", "true");
        let config = EdgeConfig::from_env().expect("parses");
        assert!(config.command_path_enabled);
        remove_env("FLEXCORE_EDGE_COMMAND_PATH_ENABLED");
        // A value that is neither true nor false is a named error.
        set_env("FLEXCORE_EDGE_COMMAND_PATH_ENABLED", "yes-please");
        let error = EdgeConfig::from_env().expect_err("named error");
        assert!(matches!(error, ConfigError::InvalidEnv { .. }));
        remove_env("FLEXCORE_EDGE_COMMAND_PATH_ENABLED");
    });
}

#[test]
fn debug_rendering_redacts_the_control_plane_endpoint() {
    with_required_env(|| {
        let config = EdgeConfig::from_env().expect("parses");
        let rendered = format!("{config:?}");
        assert!(
            !rendered.contains("wss://central.example"),
            "the endpoint can embed credentials and must never render: {rendered}"
        );
        assert!(rendered.contains("<redacted>"));
    });
}

#[test]
fn out_of_range_values_are_named_errors() {
    with_required_env(|| {
        set_env("FLEXCORE_EDGE_BUFFER_CAPACITY", "0");
        let error = EdgeConfig::from_env().expect_err("zero capacity is invalid");
        assert!(matches!(error, ConfigError::InvalidEnv { name: "FLEXCORE_EDGE_BUFFER_CAPACITY", .. }));
        set_env("FLEXCORE_EDGE_BUFFER_CAPACITY", "2_000_000");
        let error = EdgeConfig::from_env().expect_err("over-capacity is invalid");
        assert!(matches!(error, ConfigError::InvalidEnv { .. }));
        set_env("FLEXCORE_EDGE_BUFFER_CAPACITY", "50000");
        set_env("FLEXCORE_EDGE_POLL_INTERVAL_MS", "0");
        let error = EdgeConfig::from_env().expect_err("zero cadence is invalid");
        assert!(matches!(error, ConfigError::InvalidEnv { name: "FLEXCORE_EDGE_POLL_INTERVAL_MS", .. }));
        remove_env("FLEXCORE_EDGE_POLL_INTERVAL_MS");
        remove_env("FLEXCORE_EDGE_BUFFER_CAPACITY");
    });
}

// --- 2. Fail-closed on missing config ----------------------------------------

#[test]
fn each_required_variable_missing_is_a_named_error() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    for missing in REQUIRED {
        for name in REQUIRED {
            if name == missing {
                remove_env(name);
            } else {
                set_env(name, "x-test-value");
            }
        }
        let error = EdgeConfig::from_env().expect_err("a required variable is missing");
        match &error {
            ConfigError::MissingEnv { name } => assert_eq!(*name, missing),
            other => panic!("expected MissingEnv for {missing}, got {other:?}"),
        }
        assert!(
            error.to_string().contains(missing),
            "the error names the missing variable"
        );
    }
    for name in REQUIRED {
        remove_env(name);
    }
}

#[test]
fn set_but_empty_required_variable_is_a_named_error() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    set_env("FLEXCORE_EDGE_SITE_ID", "   ");
    set_env("FLEXCORE_EDGE_TENANT_ID", "tenant-demo");
    set_env("FLEXCORE_EDGE_CONTROL_PLANE_ENDPOINT", "wss://central.example/edge");
    set_env("FLEXCORE_EDGE_ADAPTER_ID", "openems-rest");
    let error = EdgeConfig::from_env().expect_err("empty site id is invalid");
    assert!(matches!(
        error,
        ConfigError::InvalidEnv { name: "FLEXCORE_EDGE_SITE_ID", .. }
    ));
    for name in REQUIRED {
        remove_env(name);
    }
}

// --- 3. Runner start/stop with in-memory fakes --------------------------------

const BASE_MILLIS: i64 = 1_700_000_000_000;

fn sample(sequence: u64) -> EdgeTelemetrySample {
    EdgeTelemetrySample::new(EdgeTelemetrySampleParts {
        asset_key: "ess-01".to_string(),
        quantity: TelemetryQuantity::ActivePower,
        value: QuantityValue::Power(ActivePowerMW::new(3.0).expect("finite")),
        quality: TelemetryQuality::Good,
        source_quality_code: None,
        event_time: EventTime::from_unix_millis(BASE_MILLIS + 1_000).expect("valid"),
        ingest_time: IngestTime::from_unix_millis(BASE_MILLIS + 2_000).expect("valid"),
        source_sequence: sequence,
        source_id: "edge:pilot-site-A:openems-rest:ess-01:active_power".to_string(),
    })
    .expect("valid sample")
}

/// A scripted source: emits the queued sequences once, then reports empty.
struct OnceSource {
    queued: Mutex<Vec<u64>>,
}

impl OnceSource {
    fn new(sequences: Vec<u64>) -> Self {
        Self {
            queued: Mutex::new(sequences),
        }
    }
}

impl flexcore_edge_agent::ports::EmsSource for OnceSource {
    fn source_name(&self) -> &str {
        "once-fake"
    }

    fn poll(&mut self, max: usize) -> Result<Vec<EdgeTelemetrySample>, EdgeError> {
        let mut queue = self.queued.lock().expect("poisoned");
        if queue.is_empty() {
            return Ok(Vec::new());
        }
        let take = max.min(queue.len());
        Ok(queue.drain(..take).map(sample).collect())
    }
}

/// The in-memory buffer implements the [`LocalBuffer`] trait directly; the
/// trait-level behaviour (bounds, order, prepend) is exercised in the ports
/// unit tests, so the runner tests compose it through the trait object.
fn buffered(capacity: usize) -> Box<dyn LocalBuffer> {
    Box::new(InMemoryLocalBuffer::new(capacity))
}

#[tokio::test]
async fn runner_starts_ticks_and_stops_on_shutdown() {
    // Composition: scripted source (3 samples), healthy in-memory uplink,
    // bounded in-memory buffer, audit sink, fixed clock.
    let uplink = Arc::new(Mutex::new(Vec::new()));
    struct CollectingUplink {
        collected: Arc<Mutex<Vec<u64>>>,
    }
    impl flexcore_edge_agent::ports::UplinkChannel for CollectingUplink {
        fn is_connected(&self) -> bool {
            true
        }
        fn forward(&mut self, samples: &[EdgeTelemetrySample]) -> Result<(), EdgeError> {
            self.collected
                .lock()
                .expect("poisoned")
                .extend(samples.iter().map(|s| s.source_sequence()));
            Ok(())
        }
    }

    let audit = Arc::new(InMemoryAuditSink::new());
    let runner = EdgeRunner::new(
        RunnerParams::from_config(Duration::from_millis(20), 10, 100),
        Box::new(OnceSource::new(vec![1, 2, 3])),
        Box::new(CollectingUplink {
            collected: Arc::clone(&uplink),
        }),
        buffered(100),
        Arc::clone(&audit) as Arc<dyn flexcore_edge_agent::ports::AuditSink>,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)),
    );

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handle = tokio::spawn(runner.run(shutdown_rx));

    // Let a few ticks run (20 ms cadence), then stop.
    tokio::time::sleep(Duration::from_millis(80)).await;
    shutdown_tx.send(true).expect("the receiver is alive");
    let last: TickReport = handle.await.expect("the runner task finishes");

    // Stop happened on the watch flag. The final tick's report reflects the
    // last tick only (later ticks poll empty), so the cumulative assertion
    // goes on the collected uplink batches.
    assert_eq!(last.buffered_len_after, 0);
    assert!(!last.degraded);
    let collected = uplink.lock().expect("poisoned").clone();
    assert_eq!(collected, vec![1, 2, 3], "all scripted samples forwarded, in order");
}

#[tokio::test]
async fn runner_with_fail_closed_ports_keeps_buffering_and_audits() {
    // The honest this-wave composition: fail-closed source AND uplink.
    let audit = Arc::new(InMemoryAuditSink::new());
    let runner = EdgeRunner::new(
        RunnerParams::from_config(Duration::from_millis(20), 10, 100),
        Box::new(FailClosedEmsSource),
        Box::new(FailClosedUplink),
        buffered(100),
        Arc::clone(&audit) as Arc<dyn flexcore_edge_agent::ports::AuditSink>,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)),
    );
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handle = tokio::spawn(runner.run(shutdown_rx));
    tokio::time::sleep(Duration::from_millis(60)).await;
    shutdown_tx.send(true).expect("the receiver is alive");
    let last: TickReport = handle.await.expect("the runner task finishes");

    // Nothing fabricated, nothing forwarded, poll failures audited.
    assert_eq!(last.forwarded, 0);
    assert_eq!(last.polled, 0);
    assert!(last.poll_failed);
    assert!(last.degraded);
    let events = audit.events();
    assert!(
        events
            .iter()
            .any(|e| e.kind.name() == "EDGE_SOURCE_POLL_FAILED"),
        "poll failures are audited"
    );
}

// --- 4. Fail-closed port behaviour ---------------------------------------------

#[test]
fn fail_closed_uplink_errors_and_reports_disconnected() {
    let mut uplink = FailClosedUplink;
    assert!(!uplink.is_connected());
    let result = uplink.forward(&[sample(1)]);
    assert!(matches!(result, Err(EdgeError::UplinkForwardFailed { .. })));
}

#[test]
fn fail_closed_source_errors_and_names_the_missing_wave() {
    let mut source = FailClosedEmsSource;
    let result = source.poll(10);
    match result {
        Err(EdgeError::SourcePollFailed { reason }) => {
            assert!(reason.contains("P6.08/P6.10"), "the error explains itself: {reason}");
        }
        other => panic!("expected a fail-closed poll error, got {other:?}"),
    }
}

#[test]
fn clock_port_serves_both_timestamp_views() {
    // The audit timestamps come from the Clock port (P1.07): the two views
    // are distinct types from one instant (asserted through the fake).
    let clock = FixedClock::at_unix_millis(BASE_MILLIS);
    let event: EventTime = clock.now_event_time();
    let ingest: IngestTime = clock.now_ingest_time();
    assert_eq!(event.unix_millis(), BASE_MILLIS);
    assert_eq!(ingest.unix_millis(), BASE_MILLIS);
}
