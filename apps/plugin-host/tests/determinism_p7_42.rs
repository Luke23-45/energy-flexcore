// P7.42 — Host-side plugin-path determinism (ledger P7.42; report §24 step
// 10 replay reproducibility, §19.2(5)). WRITTEN, NEVER RUN — the loop bans
// hold; the human build/test pass executes these
// (`cargo test -p flexcore-plugin-host --test determinism_p7_42`).
//
// WHAT IS PROVEN HERE (the host-side half of P7.42 — the kernel-side half is
// `crates/flexcore-kernel/tests/determinism_p7_42.rs`; the FULL §24-step-10
// replay through a LIVE host+worker is the recorded human/deployment
// follow-up in
// `ledger/phase-07-plugin-sdk/41_42_43_hardening_determinism_regression.md`
// §4, where the live-run-only nondeterminism sources — fuel-limit behaviour,
// Wasmtime-version sensitivity, worker timing — are listed and excluded from
// every equivalence claim below):
//
// 1. **Enforcement verdicts are pure functions of their inputs**: the same
//    `InstantiationInput` evaluated on two INDEPENDENTLY composed gates
//    (fresh brokers, fresh audit sinks) at the same `now` yields identical
//    verdict objects — same frozen grants (with the broker's deterministic
//    grant ids), same limits, same identity fields, same evaluated-at.
// 2. **Broker grant derivation is a pure function of its inputs**: the same
//    `GrantDerivationRequest` on fresh brokers yields identical grant
//    receipts (the `grant/<plugin>/<capability>/<seq>` ids are issued
//    deterministically in request order), and the same capability check
//    yields an identical `UseReceipt`; refusals are identical named errors.
// 3. **Health snapshots are deterministic for identical event sequences**:
//    two monitors fed the same observations at the same injected timestamps
//    produce equal `HealthSnapshot`s and byte-identical JSON renderings.
// 4. **Limits conversions are exact and pure**: pages-per-MiB is an exact
//    integer conversion, epoch ticks round up deterministically, the
//    host-side defaults are applied deterministically, and the same
//    `(DeclaredBounds, fuel)` always yields an equal `ResourceLimits`.
//
// WALL-CLOCK NOTE: the broker, the enforcement gate, and the health monitor
// read NO clock of their own — every timestamp is an explicit `now`/`now_..`
// parameter supplied by the caller (asserted structurally by the
// construction below: there is no parameterless time-taking call to make).
// The inputs here are FIXED constants, so identical inputs → identical
// outputs is asserted exactly, with no tolerance anywhere.

use std::sync::Arc;

use flexcore_plugin_host::broker::{
    BrokerAuditSink, CapabilityBroker, CapabilityScope, GrantDerivationRequest, GrantRequest,
    InMemoryBrokerAuditSink,
};
use flexcore_plugin_host::enforcement::{
    EnforcementAuditSink, EnforcementGate, EnforcementRequest, InMemoryEnforcementAuditSink,
    InstantiationInput, ProfileCeiling, ProfileGrant,
};
use flexcore_plugin_host::health::{
    HealthLimits, HealthState, MemoryHealthAuditSink, PluginHealthMonitor, PluginObservation,
    ReadinessState,
};
use flexcore_plugin_host::host::{HostError, WorldId};
use flexcore_plugin_host::limits::{
    epoch_ticks_for, DeclaredBounds, EPOCH_TICK_MILLIS, DEFAULT_MAX_CALLS_PER_MINUTE,
    DEFAULT_MAX_CONCURRENT_CALLS, DEFAULT_MAX_OUTPUT_MB, WASM_PAGES_PER_MIB,
};

const NOW: u64 = 1_760_000_000_000;
const PLUGIN_ID: &str = "neryva.det-optimizer";
const MANIFEST_ID: &str = "neryva.det-optimizer";
const MANIFEST_VERSION: &str = "0.2.0";
const FUEL_BUDGET: u64 = 10_000_000;

// --- Shared fixtures -------------------------------------------------------------

fn declared_bounds() -> DeclaredBounds {
    DeclaredBounds {
        max_memory_mb: 128,
        timeout_ms: 30_000,
        max_output_mb: Some(8),
        max_calls_per_minute: Some(120),
    }
}

fn enforcement_request() -> EnforcementRequest {
    EnforcementRequest {
        plugin_id: PLUGIN_ID.to_string(),
        manifest_id: MANIFEST_ID.to_string(),
        manifest_version: MANIFEST_VERSION.to_string(),
        world: WorldId::OptimizerPlugin,
        entry_symbol: "neryva:optimizer/lp-solve@0.5.0".to_string(),
        manifest_permissions: vec![
            "telemetry.read".to_string(),
            "schedule.propose".to_string(),
        ],
        declared_resource_fields: vec![
            "max-memory-mb".to_string(),
            "timeout-ms".to_string(),
            "max-output-mb".to_string(),
            "max-calls-per-minute".to_string(),
        ],
        bounds: declared_bounds(),
        fuel_budget: FUEL_BUDGET,
    }
}

fn profile_ceiling() -> ProfileCeiling {
    ProfileCeiling::new(vec![
        ProfileGrant {
            capability: "telemetry.read".to_string(),
            scope: CapabilityScope::ReadOnlyPathPrefix {
                prefix: "/data/telemetry".to_string(),
            },
            expires_at_unix_ms: Some(NOW + 3_600_000),
        },
        ProfileGrant {
            capability: "schedule.propose".to_string(),
            scope: CapabilityScope::DataScope {
                tenant_id: "tenant-det".to_string(),
                site_id: Some("site-a".to_string()),
                asset_id: None,
            },
            expires_at_unix_ms: None,
        },
    ])
}

// --- 1. Enforcement verdicts are pure functions ----------------------------------

#[test]
fn enforcement_verdicts_are_pure_functions_of_their_inputs() {
    let gate_a = EnforcementGate::compose(
        CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new())),
        Box::new(InMemoryEnforcementAuditSink::new()),
    );
    let gate_b = EnforcementGate::compose(
        CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new())),
        Box::new(InMemoryEnforcementAuditSink::new()),
    );

    let input = InstantiationInput {
        request: &enforcement_request(),
        ceiling: &profile_ceiling(),
    };

    let verdict_a = gate_a
        .evaluate(&input, NOW)
        .expect("the reference instantiation is allowed");
    let verdict_b = gate_b
        .evaluate(&input, NOW)
        .expect("the identical instantiation on a fresh gate is allowed");

    // Identical verdict OBJECTS (PartialEq): same identity, same frozen
    // grants (including the deterministic broker-issued grant ids), same
    // limits, same evaluated-at.
    assert_eq!(verdict_a, verdict_b, "the verdict is not a pure function of its inputs");
    assert_eq!(verdict_a.granted_capabilities(), verdict_b.granted_capabilities());
    assert_eq!(verdict_a.describe_grants(), verdict_b.describe_grants());
    assert_eq!(verdict_a.limits(), verdict_b.limits());
    assert_eq!(verdict_a.evaluated_at_unix_ms(), NOW);

    // The verdict is stable across a re-evaluation on the SAME gate too
    // (the grant sequence numbers advance — ids may gap, but the capability
    // set, scopes, and limits stay identical).
    let verdict_a2 = gate_a
        .evaluate(&input, NOW)
        .expect("the re-evaluation is allowed");
    assert_eq!(
        verdict_a.granted_capabilities(),
        verdict_a2.granted_capabilities(),
        "the frozen capability set changed across identical evaluations"
    );
    assert_eq!(verdict_a.limits(), verdict_a2.limits());
}

#[test]
fn enforcement_refusals_are_identical_named_errors_for_identical_inputs() {
    // An over-broad manifest (a permission the ceiling does not permit) is
    // refused with the same named error on every fresh gate.
    let mut over_broad = enforcement_request();
    over_broad.manifest_permissions.push("asset.read".to_string());
    over_broad.declared_resource_fields.push("max-calls-per-minute".to_string());

    let build = || {
        EnforcementGate::compose(
            CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new())),
            Box::new(InMemoryEnforcementAuditSink::new()),
        )
    };
    let input = InstantiationInput {
        request: &over_broad,
        ceiling: &profile_ceiling(),
    };
    let err_a = build().evaluate(&input, NOW).expect_err("over-broad is refused");
    let err_b = build().evaluate(&input, NOW).expect_err("over-broad is refused again");
    assert_eq!(err_a, err_b, "the refusal is not deterministic");
    assert_eq!(err_a.name(), "PERMISSION_ABOVE_PROFILE");

    // A zero fuel budget is refused at the limits re-validation, identically.
    let mut unmetered = enforcement_request();
    unmetered.fuel_budget = 0;
    let input = InstantiationInput {
        request: &unmetered,
        ceiling: &profile_ceiling(),
    };
    let err_a = build().evaluate(&input, NOW).expect_err("unmetered is refused");
    let err_b = build().evaluate(&input, NOW).expect_err("unmetered is refused again");
    assert_eq!(err_a, err_b);
    assert_eq!(err_a.name(), "LIMITS_INVALID");

    // Vocabulary drift (`timeoutMs` — the recorded seam) is refused
    // identically, never given a default interpretation.
    let mut drifted = enforcement_request();
    drifted.declared_resource_fields.push("timeoutMs".to_string());
    let input = InstantiationInput {
        request: &drifted,
        ceiling: &profile_ceiling(),
    };
    let err = build().evaluate(&input, NOW).expect_err("drift is refused");
    assert_eq!(err.name(), "PERMISSION_VOCABULARY_DRIFT");
}

// --- 2. Broker grant derivation and checks are pure functions --------------------

fn derivation_request() -> GrantDerivationRequest {
    GrantDerivationRequest {
        plugin_id: PLUGIN_ID.to_string(),
        manifest_id: MANIFEST_ID.to_string(),
        manifest_version: MANIFEST_VERSION.to_string(),
        manifest_permissions: vec![
            "telemetry.read".to_string(),
            "schedule.propose".to_string(),
        ],
        requested_grants: vec![
            GrantRequest {
                capability: "telemetry.read".to_string(),
                scope: CapabilityScope::ReadOnlyPathPrefix {
                    prefix: "/data/telemetry/ess-01".to_string(),
                },
                expires_at_unix_ms: Some(NOW + 60_000),
            },
            GrantRequest {
                capability: "schedule.propose".to_string(),
                scope: CapabilityScope::DataScope {
                    tenant_id: "tenant-det".to_string(),
                    site_id: Some("site-a".to_string()),
                    asset_id: Some("ess-01".to_string()),
                },
                expires_at_unix_ms: None,
            },
        ],
    }
}

#[test]
fn broker_grant_derivation_is_a_pure_function_of_its_inputs() {
    let broker_a = CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()));
    let broker_b = CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()));

    let receipts_a = broker_a
        .derive_grants(&derivation_request(), NOW)
        .expect("the reference derivation is granted");
    let receipts_b = broker_b
        .derive_grants(&derivation_request(), NOW)
        .expect("the identical derivation on a fresh broker is granted");

    assert_eq!(
        receipts_a, receipts_b,
        "the grant derivation is not a pure function of its inputs \
         (the grant ids are deterministic: grant/<plugin>/<capability>/<seq>)"
    );
    // The deterministic id shape, pinned.
    assert!(receipts_a[0].grant.grant_id().starts_with(&format!("grant/{PLUGIN_ID}/telemetry.read/")));
    assert!(receipts_a[1].grant.grant_id().starts_with(&format!("grant/{PLUGIN_ID}/schedule.propose/")));
}

#[test]
fn broker_check_verdicts_are_pure_functions_of_their_inputs() {
    let broker = CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()));
    broker
        .derive_grants(&derivation_request(), NOW)
        .expect("the derivation is granted");

    // The same granted check → an identical UseReceipt, every time.
    let scope = CapabilityScope::ReadOnlyPathPrefix {
        prefix: "/data/telemetry/ess-01/2026".to_string(),
    };
    let first = broker
        .check(PLUGIN_ID, "telemetry.read", &scope, NOW)
        .expect("the granted check is honoured");
    let second = broker
        .check(PLUGIN_ID, "telemetry.read", &scope, NOW)
        .expect("the identical check is honoured again");
    assert_eq!(first, second, "the honoured check is not a pure function of its inputs");
    assert_eq!(first.grant_id, second.grant_id);
    assert_eq!(first.scope, scope);

    // The same denied check → an identical named error, every time.
    let outside = CapabilityScope::ReadOnlyPathPrefix {
        prefix: "/data/telemetry-evil".to_string(),
    };
    let denial_a = broker
        .check(PLUGIN_ID, "telemetry.read", &outside, NOW)
        .expect_err("the outside-scope check is denied");
    let denial_b = broker
        .check(PLUGIN_ID, "telemetry.read", &outside, NOW)
        .expect_err("the identical denial is denied again");
    assert_eq!(denial_a, denial_b, "the denial is not deterministic");
    assert_eq!(denial_a.name(), "SCOPE_OUTSIDE_GRANT");

    // Expiry is a pure function of (grant, now): the same `now` below the
    // expiry honours, the same `now` at/after the expiry refuses.
    let at_expiry = NOW + 60_000;
    let expired = broker
        .check(PLUGIN_ID, "telemetry.read", &scope, at_expiry)
        .expect_err("at the expiry instant the grant is dead");
    assert_eq!(expired.name(), "GRANT_EXPIRED");
    let still_live = broker
        .check(PLUGIN_ID, "telemetry.read", &scope, at_expiry - 1)
        .expect("one millisecond before the expiry the grant is live");
    assert_eq!(still_live, second, "the pre-expiry check must match the reference receipt");
}

// --- 3. Health snapshots are deterministic for identical event sequences ---------

fn health_limits() -> HealthLimits {
    HealthLimits {
        unresponsive_after_millis: 5_000,
        error_rate_window_millis: 10_000,
        max_errors_in_window: 3,
    }
}

/// The identical event sequence for one probe plugin: register → success →
/// failure → timeout → exhaustion → readiness probe → liveness probe, all at
/// fixed injected timestamps.
fn feed_identical_event_sequence(monitor: &mut PluginHealthMonitor<'_>) {
    monitor.register(PLUGIN_ID, MANIFEST_VERSION, NOW);
    monitor
        .record_call_result(
            PLUGIN_ID,
            NOW + 100,
            PluginObservation::CallSucceeded { fuel_used: 1_000 },
        )
        .expect("the track exists");
    monitor
        .record_call_result(
            PLUGIN_ID,
            NOW + 200,
            PluginObservation::CallFailed {
                reason: "the plugin returned an error".to_string(),
            },
        )
        .expect("the track exists");
    monitor
        .record_call_result(
            PLUGIN_ID,
            NOW + 300,
            PluginObservation::CallTimedOut {
                reason: "the call hit its deadline".to_string(),
            },
        )
        .expect("the track exists");
    monitor
        .record_exhaustion(PLUGIN_ID, NOW + 400, &HostError::FuelExhausted)
        .expect("the exhaustion maps into the closed vocabulary");
    monitor
        .record_readiness_probe(PLUGIN_ID, NOW + 500, true)
        .expect("the track exists");
    monitor
        .record_liveness_probe(PLUGIN_ID, NOW + 600, true)
        .expect("the track exists");
}

#[test]
fn health_snapshots_are_deterministic_for_identical_event_sequences() {
    let sink_a = MemoryHealthAuditSink::new();
    let sink_b = MemoryHealthAuditSink::new();
    let mut monitor_a =
        PluginHealthMonitor::new(health_limits(), &sink_a).expect("the limits are valid");
    let mut monitor_b =
        PluginHealthMonitor::new(health_limits(), &sink_b).expect("the limits are valid");

    feed_identical_event_sequence(&mut monitor_a);
    feed_identical_event_sequence(&mut monitor_b);

    let snapshot_a = monitor_a.snapshot(NOW + 1_000);
    let snapshot_b = monitor_b.snapshot(NOW + 1_000);
    assert_eq!(
        snapshot_a, snapshot_b,
        "identical event sequences must produce identical health snapshots"
    );
    // The JSON rendering is byte-identical too (the P7.25 console seam).
    assert_eq!(snapshot_a.json(), snapshot_b.json());

    // The same snapshot call at the same reading is stable within one
    // monitor (the assessment is a pure function of the track facts + now).
    let snapshot_again = monitor_a.snapshot(NOW + 1_000);
    assert_eq!(snapshot_a, snapshot_again);

    // The recorded statuses are the closed-vocabulary ones the sequence
    // determines (exhaustion latched → DEGRADED_FUEL_EXHAUSTED; readiness
    // READY via the passed readiness probe; liveness alive at NOW+600 keeps
    // the plugin responsive inside the detection window).
    let record = &snapshot_a.plugins[0];
    assert_eq!(record.plugin_id, PLUGIN_ID);
    assert_eq!(record.status_name(), "DEGRADED_FUEL_EXHAUSTED");
    assert_eq!(record.readiness, ReadinessState::Ready);
    assert_eq!(record.state, HealthState::Degraded(flexcore_plugin_host::health::HealthReason::FuelExhausted));

    // And the audit trails are identical as well (sorted emission order).
    assert_eq!(sink_a.records(), sink_b.records());
}

#[test]
fn health_snapshots_are_deterministic_across_registration_orders() {
    // The snapshot is ordered by plugin id (BTreeMap), independent of the
    // registration order — the same two plugins registered in opposite
    // orders produce byte-identical JSON.
    let sink_a = MemoryHealthAuditSink::new();
    let sink_b = MemoryHealthAuditSink::new();
    let mut monitor_a =
        PluginHealthMonitor::new(health_limits(), &sink_a).expect("the limits are valid");
    let mut monitor_b =
        PluginHealthMonitor::new(health_limits(), &sink_b).expect("the limits are valid");

    let other = "neryva.det-grid-provider";
    monitor_a.register(PLUGIN_ID, MANIFEST_VERSION, NOW);
    monitor_a.register(other, MANIFEST_VERSION, NOW);
    monitor_a
        .record_call_result(PLUGIN_ID, NOW + 100, PluginObservation::CallSucceeded { fuel_used: 10 })
        .expect("the track exists");
    monitor_a
        .record_call_result(other, NOW + 100, PluginObservation::CallSucceeded { fuel_used: 20 })
        .expect("the track exists");

    monitor_b.register(other, MANIFEST_VERSION, NOW);
    monitor_b.register(PLUGIN_ID, MANIFEST_VERSION, NOW);
    monitor_b
        .record_call_result(other, NOW + 100, PluginObservation::CallSucceeded { fuel_used: 20 })
        .expect("the track exists");
    monitor_b
        .record_call_result(PLUGIN_ID, NOW + 100, PluginObservation::CallSucceeded { fuel_used: 10 })
        .expect("the track exists");

    let snapshot_a = monitor_a.snapshot(NOW + 200);
    let snapshot_b = monitor_b.snapshot(NOW + 200);
    assert_eq!(snapshot_a, snapshot_b, "snapshot order depends on registration order");
    assert_eq!(snapshot_a.plugins[0].plugin_id, other, "the snapshot is not plugin-id sorted");
    assert_eq!(snapshot_a.json(), snapshot_b.json());
}

// --- 4. Limits conversions are exact and pure -------------------------------------

#[test]
fn limits_conversions_are_exact_and_pure() {
    // Pages per MiB: the exact integer conversion (no rounding anywhere).
    assert_eq!(WASM_PAGES_PER_MIB, 16);
    let limits_a =
        flexcore_plugin_host::limits::ResourceLimits::from_declared_bounds(&declared_bounds(), FUEL_BUDGET)
            .expect("the declared bounds are inside the closed ranges");
    let limits_b =
        flexcore_plugin_host::limits::ResourceLimits::from_declared_bounds(&declared_bounds(), FUEL_BUDGET)
            .expect("the same bounds derive the same limits");
    assert_eq!(limits_a, limits_b, "limits derivation is not a pure function");
    assert_eq!(
        limits_a.max_memory_pages(),
        128 * WASM_PAGES_PER_MIB,
        "the MiB→page conversion must be exact (128 MiB = 2048 pages)"
    );
    assert_eq!(limits_a.wall_clock_deadline_ms(), 30_000);
    assert_eq!(
        limits_a.max_output_bytes(),
        8u64 * 1024 * 1024,
        "the MiB→byte conversion must be exact"
    );
    assert_eq!(limits_a.max_calls_per_minute(), 120);
    assert_eq!(limits_a.max_concurrent_calls(), DEFAULT_MAX_CONCURRENT_CALLS);
    assert_eq!(limits_a.fuel_budget(), FUEL_BUDGET);
    assert_eq!(limits_a.epoch_ticks(), 30_000 / EPOCH_TICK_MILLIS, "30 000 ms = 3 000 ticks at 10 ms/tick");

    // The DEFAULTS are deterministic: omitting the optional fields applies
    // exactly the recorded default caps, every time.
    let sparse = DeclaredBounds {
        max_memory_mb: 1,
        timeout_ms: 1,
        max_output_mb: None,
        max_calls_per_minute: None,
    };
    let sparse_limits = flexcore_plugin_host::limits::ResourceLimits::from_declared_bounds(&sparse, 1)
        .expect("the minimal bounds are inside the closed ranges");
    assert_eq!(
        sparse_limits.max_output_bytes(),
        DEFAULT_MAX_OUTPUT_MB * 1024 * 1024,
        "the default output cap must be exactly the recorded DEFAULT_MAX_OUTPUT_MB"
    );
    assert_eq!(
        sparse_limits.max_calls_per_minute(),
        DEFAULT_MAX_CALLS_PER_MINUTE,
        "the default call-rate cap must be exactly the recorded DEFAULT_MAX_CALLS_PER_MINUTE"
    );

    // Epoch ticks round UP (fail-closed), deterministically.
    assert_eq!(epoch_ticks_for(1), 1);
    assert_eq!(epoch_ticks_for(10), 1);
    assert_eq!(epoch_ticks_for(11), 2);
    assert_eq!(epoch_ticks_for(3_600_000), 360_000);

    // The zero-fuel refusal is deterministic (there is no default budget —
    // an unmetered instance is not constructible).
    let err_a = flexcore_plugin_host::limits::ResourceLimits::from_declared_bounds(&declared_bounds(), 0)
        .expect_err("zero fuel is refused");
    let err_b = flexcore_plugin_host::limits::ResourceLimits::from_declared_bounds(&declared_bounds(), 0)
        .expect_err("zero fuel is refused again");
    assert_eq!(err_a, err_b);
    assert_eq!(err_a.name(), "LIMITS_INVALID");
}

// --- 5. The enforcement audit trail is deterministic for identical inputs ---------

#[test]
fn enforcement_audit_trails_are_identical_for_identical_inputs() {
    let sink_a = Arc::new(InMemoryEnforcementAuditSink::new());
    let sink_b = Arc::new(InMemoryEnforcementAuditSink::new());
    // Shared-sink composition (the recorded pattern): each gate owns one Arc
    // clone, the test inspects the other end of each log.
    let gate_a = EnforcementGate::compose(
        CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new())),
        Box::new(sink_a.clone()),
    );
    let gate_b = EnforcementGate::compose(
        CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new())),
        Box::new(sink_b.clone()),
    );

    let input = InstantiationInput {
        request: &enforcement_request(),
        ceiling: &profile_ceiling(),
    };
    let _verdict_a = gate_a.evaluate(&input, NOW).expect("the reference is allowed");
    let _verdict_b = gate_b.evaluate(&input, NOW).expect("the identical input is allowed");

    // Identical inputs on independently composed gates produce identical
    // audit trails (same events, same order, same details).
    let events_a = sink_a.events();
    let events_b = sink_b.events();
    assert!(!events_a.is_empty(), "the allow decision was audited");
    assert_eq!(
        events_a, events_b,
        "the audit trail is not a pure function of the evaluation input"
    );
    // Every recorded timestamp is the caller-supplied `now` — the gate read
    // no clock of its own.
    assert!(events_a.iter().all(|e| e.recorded_at_unix_ms == NOW));
}
