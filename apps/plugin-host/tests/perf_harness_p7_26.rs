// P7.26 — Performance and resource validation of the plugin host and
//         workers: the MEASUREMENT HARNESS (report §19.2(8), §19.4;
//         ledger `ledger_phase_7.md` §P7.26). Written, NEVER RUN — the
//         loop bans hold; the first human
//         `cargo test -p flexcore-plugin-host --test perf_harness_p7_26 -- --nocapture`
//         run pastes the printed `flexcore-pluginhost-perf-record/v1`
//         block into `ledger/phase-07-plugin-sdk/26_27_35_ci_performance.md`
//         §4 with the hardware/OS/versions/conditions of the run.
//
// WHAT THIS IS: a std-only (std::time::Instant — NO criterion, NO new
// dependencies) harness over the REAL fail-closed host seams that are
// measurable WITHOUT a WASM component:
//
//   1. enforcement-gate evaluation cost  (`enforcement::EnforcementGate::evaluate`
//      — the P7.12 door: manifest ∩ ceiling + broker grant derivation + audit)
//   2. broker grant derivation           (`broker::CapabilityBroker::derive_grants`
//      — the P7.11 grant path, including its audit emission)
//   3. limits construction               (`limits::ResourceLimits::from_declared_bounds`
//      — the closed-range re-validation + exact page/epoch derivation)
//   4. health snapshot serialization     (`health::PluginHealthMonitor::snapshot`
//      + `HealthSnapshot::json` — the P7.25 console payload)
//
// plus STRUCTURAL instrumentation of the REAL measurements — component
// instantiation time, per-call overhead, memory per component, fuel per
// contract call, epoch-deadline behaviour under load — as RECORDED,
// SKIPPED-WHEN-NO-BINDINGS steps: under the DEFAULT build the bindings seam
// is `NoBindings` (fail-closed), so those steps print a `skipped` row naming
// the EXACT command that fills them after the human's bindings-enabled
// build. The harness deliberately does NOT write the
// `generated-bindings` code arm: the gated implementation references the
// human's wit-bindgen-generated modules, and enabling the feature without
// materialising them is a loud compile error (the crate's recorded design).
//
// WHAT THIS IS NOT (the honesty rules, report §19.4 / §19.3 / AGENTS.md
// rule 1):
//
// - It records NO performance claim and NO threshold: every assertion below
//   is STRUCTURAL (the seam answers; the conversions are exact; the
//   fail-closed refusal is the named one). A slow run does not fail
//   anything — §19.4 forbids arbitrary universal targets.
// - The timings it prints are HARNESS TIMINGS ON THE TEST HOST. They are
//   NOT pilot/target numbers (the lab-vs-target note in the decision doc
//   §4) and NOT a fleet extrapolation (§19.3).
// - The iteration counts are HARNESS PARAMETERS chosen to exercise the
//   paths in seconds; the REAL load profile (invocations per schedule run,
//   component and worker counts, payload sizes) is
//   "from pilot/research deployment — PLACEHOLDER", recorded EMPTY in the
//   decision doc §4.
//
// WORKER SIDE (reference, not duplication): the P7.18 supervisor tests
// (`apps/worker/tests/supervision_p7_18.rs`) already carry the
// injected-clock determinism for timeout/backpressure/recovery behaviour;
// this harness does NOT re-test them. Their measured (wall-clock) cells
// are recorded EMPTY in the decision doc §4 with the filling commands.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use flexcore_plugin_host::bindings::PluginBinding;
use flexcore_plugin_host::broker::{
    CapabilityBroker, CapabilityScope, GrantDerivationRequest, GrantRequest,
    InMemoryBrokerAuditSink,
};
use flexcore_plugin_host::enforcement::{
    instantiate, EnforcementGate, EnforcementRequest, InMemoryEnforcementAuditSink,
    InstantiationInput, ProfileCeiling, ProfileGrant,
};
use flexcore_plugin_host::health::{
    HealthLimits, MemoryHealthAuditSink, PluginHealthMonitor, PluginObservation,
};
use flexcore_plugin_host::host::{ComponentArtifact, HostError, WorldId};
use flexcore_plugin_host::limits::{epoch_ticks_for, DeclaredBounds, ResourceLimits};

// --- Harness parameters (NOT the pilot load profile — see the header) ---------

const GATE_ITERATIONS: usize = 1_000;
const BROKER_ITERATIONS: usize = 1_000;
const LIMITS_ITERATIONS: usize = 10_000;
const SNAPSHOT_ITERATIONS: usize = 2_000;
const SNAPSHOT_PLUGINS: usize = 8;
const NOW: u64 = 1_700_000_000_000;

// --- Fixtures ------------------------------------------------------------------

fn bounds() -> DeclaredBounds {
    DeclaredBounds {
        max_memory_mb: 256,
        timeout_ms: 5_000,
        max_output_mb: None,
        max_calls_per_minute: None,
    }
}

fn fuel_budget() -> u64 {
    100_000_000
}

fn enforcement_request(plugin_id: &str) -> EnforcementRequest {
    EnforcementRequest {
        plugin_id: plugin_id.to_string(),
        manifest_id: plugin_id.to_string(),
        manifest_version: "1.0.0".to_string(),
        world: WorldId::OptimizerPlugin,
        entry_symbol: "flexcore:optimizer/optimizer".to_string(),
        manifest_permissions: vec!["telemetry.read".to_string()],
        declared_resource_fields: vec![
            "max-memory-mb".to_string(),
            "timeout-ms".to_string(),
        ],
        bounds: bounds(),
        fuel_budget: fuel_budget(),
    }
}

fn telemetry_ceiling() -> ProfileCeiling {
    ProfileCeiling::new(vec![ProfileGrant {
        capability: "telemetry.read".to_string(),
        scope: CapabilityScope::DataScope {
            tenant_id: "tenant-a".to_string(),
            site_id: None,
            asset_id: None,
        },
        expires_at_unix_ms: None,
    }])
}

fn derivation_request(plugin_id: &str) -> GrantDerivationRequest {
    GrantDerivationRequest {
        plugin_id: plugin_id.to_string(),
        manifest_id: plugin_id.to_string(),
        manifest_version: "1.0.0".to_string(),
        manifest_permissions: vec!["telemetry.read".to_string()],
        requested_grants: vec![GrantRequest {
            capability: "telemetry.read".to_string(),
            scope: CapabilityScope::DataScope {
                tenant_id: "tenant-a".to_string(),
                site_id: None,
                asset_id: None,
            },
            expires_at_unix_ms: None,
        }],
    }
}

fn health_limits() -> HealthLimits {
    HealthLimits {
        unresponsive_after_millis: 60_000,
        error_rate_window_millis: 300_000,
        max_errors_in_window: 3,
    }
}

/// A throwaway component artifact file (bytes are irrelevant to the
/// fail-closed seam — the loader checks existence/size, never content).
struct TempArtifact {
    path: PathBuf,
}

impl TempArtifact {
    fn new(label: &str) -> TempArtifact {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        let unique = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "flexcore-pluginhost-p726-{label}-{}-{unique}.wasm",
            std::process::id()
        ));
        std::fs::write(&path, b"flexcore-p726-throwaway-artifact")
            .expect("the throwaway artifact file is writable");
        TempArtifact { path }
    }
}

impl Drop for TempArtifact {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

// --- Percentiles ----------------------------------------------------------------

fn percentile(sorted: &[u128], pct: f64) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    let index = ((pct / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

// --- The measured seams -----------------------------------------------------------

/// Phase 1 — the enforcement-gate evaluation cost (P7.12 door). The gate
/// and its sinks are recomposed per iteration (untimed) so the audit
/// history stays bounded and the timed section is the evaluation itself.
fn measure_gate_evaluation() -> Vec<u128> {
    let mut samples: Vec<u128> = Vec::with_capacity(GATE_ITERATIONS);
    for i in 0..GATE_ITERATIONS {
        let plugin_id = format!("neryva.perf-gate-{i}");
        let request = enforcement_request(&plugin_id);
        let ceiling = telemetry_ceiling();
        let gate = EnforcementGate::compose(
            CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new())),
            Box::new(InMemoryEnforcementAuditSink::new()),
        );
        let started = Instant::now();
        let verdict = gate
            .evaluate(&InstantiationInput { request: &request, ceiling: &ceiling }, NOW)
            .expect("the perf request is allow-listed and within the ceiling");
        samples.push(started.elapsed().as_nanos());
        // Structural: the verdict carries the frozen grant and the exact
        // page conversion (1 MiB = 16 wasm pages, `limits.rs`).
        assert_eq!(verdict.granted_capabilities(), vec!["telemetry.read".to_string()]);
        let limits = verdict.limits();
        assert_eq!(limits.max_memory_pages(), 256 * 16);
        assert_eq!(limits.wall_clock_deadline_ms(), 5_000);
        assert_eq!(limits.epoch_ticks(), epoch_ticks_for(5_000));
    }
    samples
}

/// Phase 2 — the broker grant derivation cost (P7.11), including its audit
/// emission. Fresh broker per iteration (untimed) for bounded state.
fn measure_broker_derivation() -> Vec<u128> {
    let mut samples: Vec<u128> = Vec::with_capacity(BROKER_ITERATIONS);
    for i in 0..BROKER_ITERATIONS {
        let plugin_id = format!("neryva.perf-broker-{i}");
        let request = derivation_request(&plugin_id);
        let broker = CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()));
        let started = Instant::now();
        let receipts = broker
            .derive_grants(&request, NOW)
            .expect("the perf derivation is within the declared permissions");
        samples.push(started.elapsed().as_nanos());
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].grant.capability(), "telemetry.read");
    }
    samples
}

/// Phase 3 — the limits construction cost (closed-range re-validation +
/// the exact page/epoch derivations).
fn measure_limits_construction() -> Vec<u128> {
    let mut samples: Vec<u128> = Vec::with_capacity(LIMITS_ITERATIONS);
    let bounds = bounds();
    for _ in 0..LIMITS_ITERATIONS {
        let started = Instant::now();
        let limits = ResourceLimits::from_declared_bounds(&bounds, fuel_budget())
            .expect("the perf bounds are inside the closed ranges");
        samples.push(started.elapsed().as_nanos());
        assert_eq!(limits.max_memory_pages(), 256 * 16);
        assert_eq!(limits.epoch_ticks(), epoch_ticks_for(5_000));
    }
    samples
}

/// Phase 4 — the health snapshot serialization cost (the P7.25 console
/// payload): `snapshot` + `json` over a monitor holding SNAPSHOT_PLUGINS
/// registered, observed plugins (setup untimed).
fn measure_health_snapshot_serialization() -> Vec<u128> {
    let audit = MemoryHealthAuditSink::new();
    let mut monitor =
        PluginHealthMonitor::new(health_limits(), &audit).expect("valid health limits");
    for i in 0..SNAPSHOT_PLUGINS {
        let plugin_id = format!("neryva.perf-health-{i}");
        monitor.register(&plugin_id, "1.0.0", NOW);
        monitor
            .record_call_result(&plugin_id, NOW + 1_000, PluginObservation::CallSucceeded { fuel_used: 10 })
            .expect("observed");
    }
    let mut samples: Vec<u128> = Vec::with_capacity(SNAPSHOT_ITERATIONS);
    for _ in 0..SNAPSHOT_ITERATIONS {
        let started = Instant::now();
        let snapshot = monitor.snapshot(NOW + 2_000);
        let json = snapshot.json();
        samples.push(started.elapsed().as_nanos());
        // Structural: the payload is the deterministic, sorted rendering —
        // every registered plugin appears.
        assert!(json.contains("\"plugins\":["));
        for i in 0..SNAPSHOT_PLUGINS {
            assert!(json.contains(&format!("neryva.perf-health-{i}")));
        }
    }
    samples
}

// --- The recorded, SKIPPED-WHEN-NO-BINDINGS real measurements ----------------------

/// The real-measurement rows the ledger P7.26 requires. Under the DEFAULT
/// build every component-side row is `skipped` (the bindings seam is
/// `NoBindings`, fail-closed) and names the EXACT command that fills it
/// post-first-build. The worker-side rows are `skipped` here BY DESIGN:
/// their determinism lives in the P7.18 supervisor suite (injected clock),
/// and their wall-clock measured cells need the lab run.
///
/// The `generated-bindings` arm of this function is DELIBERATELY NOT
/// WRITTEN: it belongs to the owner of the bindings-enabled build (the
/// human's wit-bindgen run per `wit/TOOLCHAIN.md`), and writing it against
/// module names that do not exist yet would either fabricate an API or
/// break the default build. Enabling the feature without this arm existing
/// is the crate's recorded loud compile error — fail-closed by design.
#[cfg(not(feature = "generated-bindings"))]
fn record_real_measurements(
    verdict: &flexcore_plugin_host::enforcement::EnforcementVerdict,
    artifact: &ComponentArtifact,
) -> Vec<(String, String, String)> {
    let mut rows: Vec<(String, String, String)> = Vec::new();

    // The one thing the DEFAULT build CAN measure structurally: the
    // fail-closed instantiation answers, and its binding refuses every use
    // with the named error. This proves the seam the real measurements
    // hang on EXISTS and is fail-closed — it measures no timing.
    let binding = instantiate(verdict, artifact)
        .expect("the default build hands back the fail-closed binding");
    let verify_result = binding.verify();
    match verify_result {
        Err(HostError::BindingsNotGenerated { world, symbol }) => {
            assert_eq!(world, "optimizer-plugin");
            assert_eq!(symbol, "flexcore:optimizer/optimizer");
            rows.push((
                "bindings_seam_state".to_string(),
                "fail-closed (NoBindings — BINDINGS_NOT_GENERATED on every use)".to_string(),
                "structural (measured here: the seam refuses, it does not run)".to_string(),
            ));
        }
        other => panic!("the default build must refuse at the bindings seam, got {other:?}"),
    }

    let fill_command = "cargo test -p flexcore-plugin-host --test perf_harness_p7_26 \
        --features generated-bindings -- --nocapture \
        (after the human's wit-bindgen run materialises the bindings modules \
        per wit/TOOLCHAIN.md, against a signed component artifact)";
    let lab_note = "lab/target run — the harness host is not the target machine";

    rows.push((
        "component_instantiation_time".to_string(),
        "SKIPPED — no bindings, no component artifact exists".to_string(),
        fill_command.to_string(),
    ));
    rows.push((
        "per_call_overhead".to_string(),
        "SKIPPED — no bindings, no component artifact exists".to_string(),
        fill_command.to_string(),
    ));
    rows.push((
        "memory_per_component".to_string(),
        "SKIPPED — not measurable from inside a Rust test process; needs external observation".to_string(),
        format!("{fill_command}; plus external memory observation on the target ({lab_note})"),
    ));
    rows.push((
        "fuel_consumption_per_contract_call".to_string(),
        "SKIPPED — no bindings; fuel accounting is the gated implementation's".to_string(),
        fill_command.to_string(),
    ));
    rows.push((
        "epoch_deadline_behaviour_under_load".to_string(),
        "SKIPPED — epoch interruption lives in the gated wasmtime implementation".to_string(),
        format!("{fill_command}; plus the limit-hit load drill (P7.13 §7 / P7.30)"),
    ));
    rows.push((
        "grpc_worker_call_latency_p95_p99".to_string(),
        "SKIPPED — no gRPC harness exists in this wave; not measurable here".to_string(),
        "the P7.28/P7.30 lab run against the example worker (python/workers/), \
         tool versions recorded per the P7.40 discipline"
            .to_string(),
    ));
    rows.push((
        "supervisor_timeout_behaviour".to_string(),
        "SKIPPED here BY DESIGN — determinism already proven on the injected clock".to_string(),
        "cargo test -p flexcore-worker --test supervision_p7_18 -- --nocapture \
         (behaviour); wall-clock measured cells: the P7.30 lab run"
            .to_string(),
    ));
    rows.push((
        "backpressure_engagement_point".to_string(),
        "SKIPPED — needs a saturated real worker pool".to_string(),
        "cargo test -p flexcore-worker --test supervision_p7_18 -- --nocapture \
         (deterministic engagement); queue-depth metrics under load: the P7.18 \
         fault-injection verification / P7.30 lab run"
            .to_string(),
    ));
    rows.push((
        "recovery_drain_time".to_string(),
        "SKIPPED — needs a real worker restart".to_string(),
        "the P7.30 failure-mode drill (lab; recovery time from the logs)"
            .to_string(),
    ));
    rows
}

// --- The harness test ----------------------------------------------------------------

#[test]
fn the_host_side_seam_costs_are_measured_and_the_real_measurements_are_recorded_skipped() {
    // The four measurable host-side seams (timed; structural assertions
    // inline — no timing thresholds, §19.4).
    let gate = measure_gate_evaluation();
    let broker = measure_broker_derivation();
    let limits = measure_limits_construction();
    let snapshot = measure_health_snapshot_serialization();

    // The recorded skip block, driven through the REAL instantiation seam.
    let artifact_file = TempArtifact::new("record");
    let artifact = ComponentArtifact::load(Path::new(&artifact_file.path))
        .expect("the throwaway artifact loads (existence/size checks only)");
    let request = enforcement_request("neryva.perf-record");
    let ceiling = telemetry_ceiling();
    let gate_composed = EnforcementGate::compose(
        CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new())),
        Box::new(InMemoryEnforcementAuditSink::new()),
    );
    let verdict = gate_composed
        .evaluate(&InstantiationInput { request: &request, ceiling: &ceiling }, NOW)
        .expect("the record request is allow-listed and within the ceiling");
    let rows = record_real_measurements(&verdict, &artifact);

    // --- Structural assertion on the RECORD ITSELF (the empty-cell
    // contract): every ledger row is present; every row is either measured
    // or skipped WITH a named reason AND a non-empty filling command.
    assert!(
        rows.len() >= 10,
        "the record must carry the bindings-seam state row plus every P7.26 row (got {})",
        rows.len()
    );
    for (name, status, fill) in &rows {
        assert!(!name.trim().is_empty() && !status.trim().is_empty() && !fill.trim().is_empty(),
            "every record row needs name/status/filling-command (row {name:?})");
    }

    // --- The MEASURED record (printed; NOT stored as a claim) -------------
    let mut sorted_gate = gate.clone();
    sorted_gate.sort_unstable();
    let mut sorted_broker = broker.clone();
    sorted_broker.sort_unstable();
    let mut sorted_limits = limits.clone();
    sorted_limits.sort_unstable();
    let mut sorted_snapshot = snapshot.clone();
    sorted_snapshot.sort_unstable();

    println!("--- flexcore-pluginhost-perf-record/v1 (P7.26; MEASURED on the test host — NOT pilot/target numbers) ---");
    println!("harness_parameters: gate_iterations={GATE_ITERATIONS} broker_iterations={BROKER_ITERATIONS} limits_iterations={LIMITS_ITERATIONS} snapshot_iterations={SNAPSHOT_ITERATIONS} snapshot_plugins={SNAPSHOT_PLUGINS}");
    println!(
        "enforcement_gate_evaluation_ns: p50={} p95={} p99={} max={} (P7.12 door: manifest ∩ ceiling + broker derivation + audit; fresh gate/sinks per iteration, evaluate timed)",
        percentile(&sorted_gate, 50.0),
        percentile(&sorted_gate, 95.0),
        percentile(&sorted_gate, 99.0),
        sorted_gate.last().copied().unwrap_or(0)
    );
    println!(
        "broker_grant_derivation_ns: p50={} p95={} p99={} max={} (P7.11 derive_grants incl. audit emission; fresh broker per iteration)",
        percentile(&sorted_broker, 50.0),
        percentile(&sorted_broker, 95.0),
        percentile(&sorted_broker, 99.0),
        sorted_broker.last().copied().unwrap_or(0)
    );
    println!(
        "limits_construction_ns: p50={} p95={} p99={} max={} (closed-range re-validation + exact page/epoch derivation)",
        percentile(&sorted_limits, 50.0),
        percentile(&sorted_limits, 95.0),
        percentile(&sorted_limits, 99.0),
        sorted_limits.last().copied().unwrap_or(0)
    );
    println!(
        "health_snapshot_serialization_ns: p50={} p95={} p99={} max={} (snapshot+json over {SNAPSHOT_PLUGINS} plugins; the P7.25 console payload)",
        percentile(&sorted_snapshot, 50.0),
        percentile(&sorted_snapshot, 95.0),
        percentile(&sorted_snapshot, 99.0),
        sorted_snapshot.last().copied().unwrap_or(0)
    );
    for (name, status, fill) in &rows {
        println!("{name}: {status} | fill: {fill}");
    }
    println!("load_profile: from pilot/research deployment — PLACEHOLDER (never assumed; §19.3)");
    println!("profiles_required: x86-64 AND ARM64 (two separate measured runs on the target hardware)");
    println!("--- end of record: paste into 26_27_35_ci_performance.md §4 with hardware/OS/versions/conditions ---");
}

#[test]
fn the_measured_seams_cannot_pass_by_being_absent() {
    // The harness's own honesty gate: if a seam ever returns an error or a
    // wrong shape, the harness FAILS LOUDLY rather than printing a record
    // with a silently missing row. This pins: the enforcement request shape,
    // the derivation shape, the exact conversions, and the snapshot shape.
    let request = enforcement_request("neryva.perf-shape");
    let ceiling = telemetry_ceiling();
    let gate = EnforcementGate::compose(
        CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new())),
        Box::new(InMemoryEnforcementAuditSink::new()),
    );
    let verdict = gate
        .evaluate(&InstantiationInput { request: &request, ceiling: &ceiling }, NOW)
        .expect("the shape request evaluates");
    assert_eq!(verdict.granted_capabilities(), vec!["telemetry.read".to_string()]);
    assert_eq!(verdict.limits().fuel_budget(), fuel_budget());

    let broker = CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()));
    let receipts = broker
        .derive_grants(&derivation_request("neryva.perf-shape"), NOW)
        .expect("the shape derivation derives");
    assert_eq!(receipts.len(), 1);

    let limits = ResourceLimits::from_declared_bounds(&bounds(), fuel_budget())
        .expect("the shape bounds construct");
    assert_eq!(limits.max_memory_pages(), 256 * 16);
    assert_eq!(limits.epoch_ticks(), epoch_ticks_for(5_000));

    let audit = MemoryHealthAuditSink::new();
    let mut monitor =
        PluginHealthMonitor::new(health_limits(), &audit).expect("valid health limits");
    monitor.register("neryva.perf-shape", "1.0.0", NOW);
    monitor
        .record_call_result("neryva.perf-shape", NOW + 1_000, PluginObservation::CallSucceeded { fuel_used: 1 })
        .expect("observed");
    assert!(monitor.snapshot(NOW + 2_000).json().contains("neryva.perf-shape"));
}
