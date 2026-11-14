// P7.10 — Fail-closed host tests (WRITTEN-UNRUN: no cargo command was
// executed in this loop; the first human `cargo test -p flexcore-plugin-host`
// run is the verification gate — decision record
// `ledger/phase-07-plugin-sdk/10_11_plugin_host_broker.md` §8).
//
// Coverage (ledger §P7.10 acceptance surface, the parts testable without
// wasmtime artifacts):
//
// - missing component path → named `COMPONENT_NOT_FOUND`;
// - unconfigured inputs → named `COMPONENT_UNCONFIGURED` / `WORLD_UNKNOWN`;
// - the default build's fail-closed bindings seam → `BINDINGS_NOT_GENERATED`
//   on every use (reached through the P7.12 enforcement path since P7.12:
//   `enforcement::instantiate` over a gate-issued verdict — there is no
//   public raw-binding call anymore);
// - limits construction from manifest bounds (valid, defaults for the two
//   optional fields, out-of-range refusal, mandatory fuel budget);
// - epoch-deadline configuration (tick conversion, round-up, consistency
//   with the wall-clock deadline).
//
// What these tests deliberately do NOT cover: wasmtime behaviour (no
// artifacts, no bindings — the P7.13 hostile suite and the limit-behaviour
// record are the P7.13 owner's, over a real build).

use std::path::PathBuf;

use flexcore_plugin_host::bindings::{make_binding, NoBindings, PluginBinding};
use flexcore_plugin_host::broker::{CapabilityBroker, InMemoryBrokerAuditSink};
use flexcore_plugin_host::enforcement::{
    instantiate, EnforcementGate, EnforcementRequest, EnforcementVerdict,
    InMemoryEnforcementAuditSink, InstantiationInput, ProfileCeiling,
};
use flexcore_plugin_host::host::{ComponentArtifact, HostError, WorldId, MAX_COMPONENT_BYTES};
use flexcore_plugin_host::limits::{epoch_ticks_for, DeclaredBounds, EPOCH_TICK_MILLIS, ResourceLimits};

// --- Test helpers --------------------------------------------------------------

/// A unique temp path that does NOT exist (no write races between tests).
fn absent_path(label: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "plugin-host-p7-10-{}-{}.wasm",
        label,
        std::process::id()
    ));
    // Best-effort removal so the path is genuinely absent.
    let _ = std::fs::remove_file(&path);
    path
}

/// Writes a small (not-a-real-component) artifact and returns its path.
fn write_stub_artifact(label: &str) -> PathBuf {
    let path = absent_path(label);
    std::fs::write(&path, b"not a real component artifact").expect("test fixture write");
    path
}

// --- Missing component ---------------------------------------------------------

#[test]
fn missing_component_is_a_named_error() {
    let path = absent_path("missing");
    let error = ComponentArtifact::load(&path).expect_err("an absent path must fail");
    assert_eq!(error.name(), "COMPONENT_NOT_FOUND");
    match &error {
        HostError::ComponentNotFound { path: shown } => {
            assert_eq!(shown, &path.display().to_string());
        }
        other => panic!("expected ComponentNotFound, got {other:?}"),
    }
}

#[test]
fn empty_component_path_is_a_named_error() {
    // The unconfigured-empty case: no default component exists and none is
    // invented. The named refusal is the honest posture (not
    // ComponentUnconfigured, which is the binary root's no-env-var state —
    // the loader's job is only to refuse an unusable path).
    let error = ComponentArtifact::load(std::path::Path::new("")).expect_err("empty path must fail");
    assert_eq!(error.name(), "COMPONENT_NOT_FOUND");
}

#[test]
fn unconfigured_posture_is_a_named_error() {
    // The binary root's no-configuration state, at the type level: the
    // named error exists, is stable, and states the run-only-with-a-signed-
    // component posture.
    let error = HostError::ComponentUnconfigured {
        what: "no component path configured".to_string(),
    };
    assert_eq!(error.name(), "COMPONENT_UNCONFIGURED");
    let rendered = error.to_string();
    assert!(rendered.contains("runs only when a human supplies a signed component"));
}

#[test]
fn unknown_world_selector_is_a_named_error() {
    let error = WorldId::parse("definitely-not-a-world").expect_err("unknown world must fail");
    assert_eq!(error.name(), "WORLD_UNKNOWN");
    for known in ["optimizer-plugin", "grid-model-provider-plugin", "telemetry-source-plugin"] {
        assert!(WorldId::parse(known).is_ok(), "recorded world '{known}' must parse");
    }
}

// --- The fail-closed bindings seam ---------------------------------------------

/// An enforcement verdict from the P7.12 gate — the ONLY public constructor
/// path to a binding since P7.12 (`bind_component` is `pub(crate)`). This
/// helper is capability-less: these tests target the bindings seam and the
/// limits construction, not capabilities (the enforcement verdict itself is
/// `enforcement_p7_12.rs`'s subject).
fn verdict_for(world: WorldId, symbol: &str) -> EnforcementVerdict {
    let gate = EnforcementGate::compose(
        CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new())),
        Box::new(InMemoryEnforcementAuditSink::new()),
    );
    let request = EnforcementRequest {
        plugin_id: "test.plugin".to_string(),
        manifest_id: "test.plugin".to_string(),
        manifest_version: "1.0.0".to_string(),
        world,
        entry_symbol: symbol.to_string(),
        manifest_permissions: Vec::new(),
        declared_resource_fields: vec!["max-memory-mb".to_string(), "timeout-ms".to_string()],
        bounds: DeclaredBounds {
            max_memory_mb: 256,
            timeout_ms: 5_000,
            max_output_mb: None,
            max_calls_per_minute: None,
        },
        fuel_budget: 100_000_000,
    };
    let ceiling = ProfileCeiling::empty();
    gate.evaluate(
        &InstantiationInput { request: &request, ceiling: &ceiling },
        1_700_000_000_000,
    )
    .expect("a capability-less request passes the gate")
}

#[test]
fn no_bindings_refuses_every_use_with_the_named_error() {
    let stub = write_stub_artifact("no-bindings");
    let artifact = ComponentArtifact::load(&stub).expect("stub loads");
    let verdict = verdict_for(WorldId::OptimizerPlugin, "flexcore:optimizer/optimizer");
    let binding = instantiate(&verdict, &artifact)
        .expect("the default build hands back the fail-closed binding object");
    assert_eq!(binding.world(), WorldId::OptimizerPlugin);
    assert_eq!(binding.symbol(), "flexcore:optimizer/optimizer");
    let error = binding.verify().expect_err("the default build cannot instantiate");
    match &error {
        HostError::BindingsNotGenerated { world, symbol } => {
            assert_eq!(world, "optimizer-plugin");
            assert_eq!(symbol, "flexcore:optimizer/optimizer");
        }
        other => panic!("expected BindingsNotGenerated, got {other:?}"),
    }
    assert_eq!(error.name(), "BINDINGS_NOT_GENERATED");
    let _ = std::fs::remove_file(&stub);
}

#[test]
fn the_bindings_seam_entry_point_is_fail_closed_by_default() {
    // Direct seam call (no artifact needed): the seam is where the refusal
    // lives, independent of loading.
    let binding = make_binding(
        WorldId::TelemetrySourcePlugin,
        "flexcore:telemetry/telemetry-source".to_string(),
        std::path::Path::new("irrelevant-in-the-default-build"),
        &valid_limits(),
    )
    .expect("the default build returns the NoBindings object");
    let error = binding.verify().expect_err("fail-closed");
    assert_eq!(error.name(), "BINDINGS_NOT_GENERATED");
}

#[test]
fn no_bindings_is_constructible_and_refuses_directly() {
    let stub = NoBindings::new(WorldId::GridModelProviderPlugin, "flexcore:grid/grid-model-provider".to_string());
    let error = stub.verify().expect_err("the stub never verifies");
    assert_eq!(error.name(), "BINDINGS_NOT_GENERATED");
}

// --- Limits construction from manifest bounds -----------------------------------

fn valid_limits() -> ResourceLimits {
    ResourceLimits::from_declared_bounds(
        &DeclaredBounds {
            max_memory_mb: 256,
            timeout_ms: 5_000,
            max_output_mb: None,
            max_calls_per_minute: None,
        },
        100_000_000,
    )
    .expect("valid bounds construct")
}

#[test]
fn limits_from_manifest_bounds_derive_every_field() {
    let limits = ResourceLimits::from_declared_bounds(
        &DeclaredBounds {
            max_memory_mb: 256,
            timeout_ms: 5_000,
            max_output_mb: Some(2),
            max_calls_per_minute: Some(120),
        },
        250_000_000,
    )
    .expect("valid bounds construct");
    // 256 MiB = 4096 wasm pages (16 pages per MiB, exact).
    assert_eq!(limits.max_memory_pages(), 256 * 16);
    assert_eq!(limits.wall_clock_deadline_ms(), 5_000);
    assert_eq!(limits.epoch_ticks(), 500);
    assert_eq!(limits.max_output_bytes(), 2 * 1024 * 1024);
    assert_eq!(limits.max_calls_per_minute(), 120);
    assert_eq!(limits.fuel_budget(), 250_000_000);
}

#[test]
fn limits_default_the_optional_manifest_fields() {
    let limits = valid_limits();
    // DEFAULT decisions (record §3): 64 MiB output cap, 60 calls/minute.
    assert_eq!(limits.max_output_bytes(), 64 * 1024 * 1024);
    assert_eq!(limits.max_calls_per_minute(), 60);
    // DEFAULT concurrency cap: one call at a time.
    assert_eq!(limits.max_concurrent_calls(), 1);
}

#[test]
fn limits_refuse_out_of_range_manifest_bounds() {
    let cases = [
        DeclaredBounds { max_memory_mb: 0, timeout_ms: 1_000, max_output_mb: None, max_calls_per_minute: None },
        DeclaredBounds { max_memory_mb: 65_537, timeout_ms: 1_000, max_output_mb: None, max_calls_per_minute: None },
        DeclaredBounds { max_memory_mb: 256, timeout_ms: 0, max_output_mb: None, max_calls_per_minute: None },
        DeclaredBounds { max_memory_mb: 256, timeout_ms: 3_600_001, max_output_mb: None, max_calls_per_minute: None },
        DeclaredBounds { max_memory_mb: 256, timeout_ms: 1_000, max_output_mb: Some(4_097), max_calls_per_minute: None },
        DeclaredBounds { max_memory_mb: 256, timeout_ms: 1_000, max_output_mb: None, max_calls_per_minute: Some(1_000_001) },
    ];
    for bounds in cases {
        let error = ResourceLimits::from_declared_bounds(&bounds, 100_000_000)
            .expect_err("out-of-range bounds must refuse");
        assert_eq!(error.name(), "LIMITS_INVALID", "bounds {bounds:?}");
    }
}

#[test]
fn limits_refuse_a_missing_fuel_budget() {
    // There is NO default fuel budget: an unmetered instance is refused at
    // construction, not at first runaway (record §3).
    let error = ResourceLimits::from_declared_bounds(
        &DeclaredBounds {
            max_memory_mb: 256,
            timeout_ms: 5_000,
            max_output_mb: None,
            max_calls_per_minute: None,
        },
        0,
    )
    .expect_err("zero fuel must refuse");
    assert_eq!(error.name(), "LIMITS_INVALID");
    let rendered = error.to_string();
    assert!(rendered.contains("fuel"));
}

// --- Epoch deadline configuration ----------------------------------------------

#[test]
fn epoch_deadline_config_matches_the_wall_clock_deadline() {
    let limits = valid_limits();
    // Ticks = ceil(deadline_ms / tick_ms); the sub-tick remainder rounds UP
    // (fail-closed: a guest never gets a free extra tick).
    assert_eq!(EPOCH_TICK_MILLIS, 10);
    assert_eq!(limits.epoch_ticks(), epoch_ticks_for(limits.wall_clock_deadline_ms()));
    assert_eq!(limits.epoch_ticks(), 500);
}

#[test]
fn epoch_tick_conversion_rounds_up() {
    assert_eq!(epoch_ticks_for(1), 1);
    assert_eq!(epoch_ticks_for(9), 1);
    assert_eq!(epoch_ticks_for(10), 1);
    assert_eq!(epoch_ticks_for(11), 2);
    assert_eq!(epoch_ticks_for(0), 1, "a zero deadline is a caller bug (rejected by the closed range); the conversion stays total and tight");
}

// --- Loader cap (constant, documented) ------------------------------------------

#[test]
fn loader_cap_is_recorded_and_nonzero() {
    // The loader cap is a DEFAULT decision (record §3); the oversized path
    // itself is exercised only over a real 64 MiB+ artifact (deliberately
    // not written here — the P7.13 hostile suite owns oversized-input
    // behaviour over a real build).
    assert_eq!(MAX_COMPONENT_BYTES, 64 * 1024 * 1024);
}
