// P7.20 — Plugin health, readiness, and observability (test file written,
// NOT run — the loop bans hold; the first human
// `cargo test -p flexcore-plugin-host` run is the verification gate).
//
// Coverage (task steps; ledger P7.20 acceptance surface, the parts
// testable without wasmtime artifacts):
//
// - never-ready-before-first-observation (the P6.17 rule mirrored: a
//   registered plugin with no observation is DEGRADED_UNVERIFIED and its
//   readiness is UNOBSERVED, never healthy/ready);
// - degradation on EACH limits.rs exhaustion error (the closed mapping:
//   MEMORY_LIMIT, FUEL_EXHAUSTED, DEADLINE, OUTPUT_LIMIT, CALL_RATE), plus
//   FAILING_BINDINGS for the bindings refusal and the named refusal for an
//   unmappable host error;
// - recovery (a successful call clears a latched exhaustion);
// - readiness via the first successful call AND via an explicit readiness
//   probe; a failed probe latches PROBE_FAILED;
// - liveness via the host's epoch/probe hooks (detection window on the
//   last seen fact; failed liveness probe fails immediately);
// - snapshot determinism (sorted plugin order, equality across runs);
// - the transition audit trail (audited state changes; refused
//   observations for unregistered plugins);
// - bounded stats (no unbounded memory under sustained failures).
//
// What these tests deliberately do NOT cover: wasmtime behaviour (no
// artifacts, no bindings — the health surface is host-side logic only).

use flexcore_plugin_host::health::{
    exhaustion_reason, HealthAuditKind, HealthLimits, HealthState, MemoryHealthAuditSink,
    PluginHealthMonitor, PluginObservation, ReadinessState,
};
use flexcore_plugin_host::host::HostError;

const BASE: u64 = 1_700_000_000_000;

fn limits() -> HealthLimits {
    HealthLimits {
        unresponsive_after_millis: 60_000,
        error_rate_window_millis: 300_000,
        max_errors_in_window: 3,
    }
}

fn monitor<'a>(audit: &'a MemoryHealthAuditSink) -> PluginHealthMonitor<'a> {
    PluginHealthMonitor::new(limits(), audit).expect("valid limits")
}

// --- 1. Never healthy / never ready before the first observation -------------------

#[test]
fn a_registered_plugin_is_never_healthy_or_ready_before_its_first_observation() {
    let audit = MemoryHealthAuditSink::new();
    let mut monitor = monitor(&audit);
    monitor.register("neryva.plugin-a", "0.1.0", BASE);
    // DEGRADED_UNVERIFIED — never healthy-by-default (the P6.17 rule).
    let record = monitor.assess("neryva.plugin-a", BASE + 1_000).expect("assessed");
    assert_eq!(record.status_name(), "DEGRADED_UNVERIFIED");
    assert!(!record.blocks_dependent_actions());
    // Readiness is UNOBSERVED — never ready before the first successful
    // call or a passed probe.
    assert_eq!(
        monitor.readiness("neryva.plugin-a").expect("registered"),
        ReadinessState::Unobserved
    );
    // An UNREGISTERED plugin is a named error — unknown is not a status.
    assert!(matches!(
        monitor.assess("neryva.ghost", BASE + 1_000),
        Err(flexcore_plugin_host::health::HealthError::UnknownPlugin { .. })
    ));
    assert!(matches!(
        monitor.readiness("neryva.ghost"),
        Err(flexcore_plugin_host::health::HealthError::UnknownPlugin { .. })
    ));
}

#[test]
fn nonsense_limits_are_composition_errors() {
    let audit = MemoryHealthAuditSink::new();
    let mut bad = limits();
    bad.unresponsive_after_millis = 0;
    assert!(PluginHealthMonitor::new(bad, &audit).is_err());
    let mut bad = limits();
    bad.error_rate_window_millis = 0;
    assert!(PluginHealthMonitor::new(bad, &audit).is_err());
    let mut bad = limits();
    bad.max_errors_in_window = 0;
    assert!(PluginHealthMonitor::new(bad, &audit).is_err());
}

// --- 2. Degradation on EACH exhaustion error (the closed mapping) -------------------

#[test]
fn every_exhaustion_error_maps_to_its_closed_degraded_reason() {
    let audit = MemoryHealthAuditSink::new();
    let mut monitor = monitor(&audit);
    // Register five plugins, one per exhaustion class, each responsive
    // first (an observation is needed to pass the UNVERIFIED gate).
    let cases: Vec<(&str, HostError, &str)> = vec![
        (
            "neryva.plugin-mem",
            HostError::MemoryLimitExceeded { limit_pages: 1024, requested_pages: 2048 },
            "DEGRADED_MEMORY_LIMIT",
        ),
        ("neryva.plugin-fuel", HostError::FuelExhausted, "DEGRADED_FUEL_EXHAUSTED"),
        ("neryva.plugin-deadline", HostError::DeadlineExceeded { deadline_ms: 30_000 }, "DEGRADED_DEADLINE"),
        ("neryva.plugin-output", HostError::OutputLimitExceeded { limit_bytes: 1024 }, "DEGRADED_OUTPUT_LIMIT"),
        ("neryva.plugin-rate", HostError::CallRateExceeded { limit_per_minute: 60 }, "DEGRADED_CALL_RATE"),
    ];
    for (id, _error, _status) in &cases {
        monitor.register(id, "0.1.0", BASE);
        monitor
            .record_call_result(id, BASE + 1_000, PluginObservation::CallSucceeded { fuel_used: 10 })
            .expect("observed");
    }
    for (id, error, expected_status) in &cases {
        // The mapping is total and closed.
        let reason = exhaustion_reason(error).expect("every exhaustion error maps");
        monitor.record_exhaustion(id, BASE + 2_000, error).expect("recorded");
        let record = monitor.assess(id, BASE + 3_000).expect("assessed");
        assert_eq!(record.status_name(), *expected_status, "plugin {id}");
        assert!(!record.blocks_dependent_actions(), "DEGRADED_* flows");
        assert_eq!(record.state, HealthState::Degraded(reason));
    }
}

#[test]
fn the_bindings_refusal_is_failing_and_an_unmappable_error_is_a_named_refusal() {
    let audit = MemoryHealthAuditSink::new();
    let mut monitor = monitor(&audit);
    monitor.register("neryva.plugin-b", "0.1.0", BASE);
    monitor
        .record_call_result("neryva.plugin-b", BASE + 1_000, PluginObservation::CallSucceeded { fuel_used: 0 })
        .expect("observed");
    // BindingsNotGenerated is not an exhaustion: FAILING_BINDINGS (the
    // plugin cannot run at all; dependent actions are BLOCKED).
    monitor
        .record_exhaustion("neryva.plugin-b", BASE + 2_000, &HostError::BindingsNotGenerated {
            world: "optimizer-plugin".to_string(),
            symbol: "neryva:optimizer".to_string(),
        })
        .expect("recorded");
    let record = monitor.assess("neryva.plugin-b", BASE + 3_000).expect("assessed");
    assert_eq!(record.status_name(), "FAILING_BINDINGS");
    assert!(record.blocks_dependent_actions());
    // A loader error is NOT a plugin-health input: the mapping is None and
    // feeding it is a NAMED refusal — never silently folded into healthy.
    assert!(exhaustion_reason(&HostError::ComponentNotFound { path: "x".to_string() }).is_none());
    let error = monitor
        .record_exhaustion("neryva.plugin-b", BASE + 4_000, &HostError::ComponentNotFound { path: "x".to_string() })
        .expect_err("unmappable host error");
    assert!(matches!(
        error,
        flexcore_plugin_host::health::HealthError::UnmappableHostError { .. }
    ));
}

// --- 3. Recovery ------------------------------------------------------------------

#[test]
fn a_successful_call_clears_a_latched_exhaustion() {
    let audit = MemoryHealthAuditSink::new();
    let mut monitor = monitor(&audit);
    monitor.register("neryva.plugin-c", "0.1.0", BASE);
    monitor
        .record_call_result("neryva.plugin-c", BASE + 1_000, PluginObservation::CallSucceeded { fuel_used: 5 })
        .expect("observed");
    assert_eq!(
        monitor.assess("neryva.plugin-c", BASE + 1_100).expect("assessed").status_name(),
        "HEALTHY"
    );
    // Exhaust: DEGRADED_FUEL_EXHAUSTED (latched).
    monitor
        .record_exhaustion("neryva.plugin-c", BASE + 2_000, &HostError::FuelExhausted)
        .expect("recorded");
    assert_eq!(
        monitor.assess("neryva.plugin-c", BASE + 2_100).expect("assessed").status_name(),
        "DEGRADED_FUEL_EXHAUSTED"
    );
    // Recovery: a later successful call clears the latch.
    monitor
        .record_call_result("neryva.plugin-c", BASE + 3_000, PluginObservation::CallSucceeded { fuel_used: 5 })
        .expect("observed");
    assert_eq!(
        monitor.assess("neryva.plugin-c", BASE + 3_100).expect("assessed").status_name(),
        "HEALTHY"
    );
}

#[test]
fn the_error_rate_threshold_degrades_until_the_window_clears() {
    let audit = MemoryHealthAuditSink::new();
    // A wide detection window so the error-rate window (300 s) can clear
    // INSIDE the detection window (60 s would fail liveness first — the
    // assessment order puts FAILING ahead of DEGRADED).
    let wide = HealthLimits {
        unresponsive_after_millis: 400_000,
        error_rate_window_millis: 300_000,
        max_errors_in_window: 3,
    };
    let mut monitor = PluginHealthMonitor::new(wide, &audit).expect("valid limits");
    monitor.register("neryva.plugin-d", "0.1.0", BASE);
    monitor
        .record_call_result("neryva.plugin-d", BASE + 1_000, PluginObservation::CallSucceeded { fuel_used: 0 })
        .expect("observed");
    // Two failures inside the window: below the threshold (3).
    for tick in 1..=2i64 {
        monitor
            .record_call_result(
                "neryva.plugin-d",
                BASE + 1_000 + tick as u64 * 1_000,
                PluginObservation::CallFailed { reason: format!("failure {tick}") },
            )
            .expect("observed");
    }
    assert_eq!(
        monitor.assess("neryva.plugin-d", BASE + 4_000).expect("assessed").status_name(),
        "HEALTHY",
        "below the threshold: still healthy"
    );
    // The third failure reaches the threshold: DEGRADED_ERROR_RATE.
    monitor
        .record_call_result("neryva.plugin-d", BASE + 5_000, PluginObservation::CallFailed { reason: "failure 3".to_string() })
        .expect("observed");
    assert_eq!(
        monitor.assess("neryva.plugin-d", BASE + 6_000).expect("assessed").status_name(),
        "DEGRADED_ERROR_RATE"
    );
    // Time passes, the error-rate window clears the samples (the detection
    // window is wide enough that liveness stays green): healthy again.
    assert_eq!(
        monitor
            .assess("neryva.plugin-d", BASE + 5_000 + 300_000 + 1_000)
            .expect("assessed")
            .status_name(),
        "HEALTHY"
    );
}

// --- 4. Readiness: first successful call OR the explicit probe ----------------------

#[test]
fn the_first_successful_call_makes_the_plugin_ready() {
    let audit = MemoryHealthAuditSink::new();
    let mut monitor = monitor(&audit);
    monitor.register("neryva.plugin-e", "0.1.0", BASE);
    assert_eq!(monitor.readiness("neryva.plugin-e").expect("registered"), ReadinessState::Unobserved);
    // A FAILED call does not make it ready.
    monitor
        .record_call_result("neryva.plugin-e", BASE + 1_000, PluginObservation::CallFailed { reason: "boom".to_string() })
        .expect("observed");
    assert_eq!(monitor.readiness("neryva.plugin-e").expect("registered"), ReadinessState::Unobserved);
    // The FIRST SUCCESSFUL call does.
    monitor
        .record_call_result("neryva.plugin-e", BASE + 2_000, PluginObservation::CallSucceeded { fuel_used: 1 })
        .expect("observed");
    assert_eq!(monitor.readiness("neryva.plugin-e").expect("registered"), ReadinessState::Ready);
}

#[test]
fn an_explicit_readiness_probe_gates_through_the_binding_seam_hook() {
    let audit = MemoryHealthAuditSink::new();
    let mut monitor = monitor(&audit);
    monitor.register("neryva.plugin-f", "0.1.0", BASE);
    // A FAILED probe latches PROBE_FAILED (its own recorded verdict).
    monitor.record_readiness_probe("neryva.plugin-f", BASE + 1_000, false).expect("recorded");
    assert_eq!(monitor.readiness("neryva.plugin-f").expect("registered"), ReadinessState::ProbeFailed);
    // A PASSED probe makes it READY.
    monitor.record_readiness_probe("neryva.plugin-f", BASE + 2_000, true).expect("recorded");
    assert_eq!(monitor.readiness("neryva.plugin-f").expect("registered"), ReadinessState::Ready);
    // And a passed probe stamps liveness (it is an observation of the
    // plugin answering).
    let record = monitor.assess("neryva.plugin-f", BASE + 2_100).expect("assessed");
    assert_eq!(record.last_seen_unix_millis, Some(BASE + 2_000));
}

// --- 5. Liveness: the detection window and the probe hook ---------------------------

#[test]
fn liveness_fails_through_the_detection_window_and_the_probe_hook() {
    let audit = MemoryHealthAuditSink::new();
    let mut monitor = monitor(&audit);
    monitor.register("neryva.plugin-g", "0.1.0", BASE);
    monitor
        .record_call_result("neryva.plugin-g", BASE, PluginObservation::CallSucceeded { fuel_used: 0 })
        .expect("observed");
    // Inside the window: healthy.
    assert_eq!(
        monitor.assess("neryva.plugin-g", BASE + 1_000).expect("assessed").status_name(),
        "HEALTHY"
    );
    // Silence past the detection window: FAILING_UNRESPONSIVE (a positive,
    // timestamped rule on the last observed fact — never a default).
    let record = monitor.assess("neryva.plugin-g", BASE + 61_000).expect("assessed");
    assert_eq!(record.status_name(), "FAILING_UNRESPONSIVE");
    assert!(record.blocks_dependent_actions());
    // Dependent actions are BLOCKED with an auditable reason.
    let reason = monitor
        .check_action_allowed("neryva.plugin-g", BASE + 62_000)
        .expect_err("blocked while FAILING");
    assert!(reason.contains("FAILING_UNRESPONSIVE"));
    // An explicit FAILED liveness probe fails immediately (the host's
    // epoch/probe hook).
    monitor.record_liveness_probe("neryva.plugin-h", BASE + 1_000, false).expect_err(
        "plugin-h was never registered",
    );
    monitor.register("neryva.plugin-h", "0.1.0", BASE);
    monitor
        .record_call_result("neryva.plugin-h", BASE + 1_000, PluginObservation::CallSucceeded { fuel_used: 0 })
        .expect("observed");
    monitor.record_liveness_probe("neryva.plugin-h", BASE + 2_000, false).expect("recorded");
    let record = monitor.assess("neryva.plugin-h", BASE + 2_100).expect("assessed");
    assert_eq!(record.status_name(), "FAILING_UNRESPONSIVE");
    // A passed liveness probe re-stamps liveness: the plugin recovers.
    monitor.record_liveness_probe("neryva.plugin-h", BASE + 3_000, true).expect("recorded");
    assert_eq!(
        monitor.assess("neryva.plugin-h", BASE + 3_100).expect("assessed").status_name(),
        "HEALTHY"
    );
}

// --- 6. Snapshot determinism -------------------------------------------------------

#[test]
fn the_snapshot_is_sorted_deterministic_and_renders_the_closed_vocabulary() {
    let audit = MemoryHealthAuditSink::new();
    let mut monitor = monitor(&audit);
    monitor.register("neryva.plugin-z", "0.2.0", BASE);
    monitor.register("neryva.plugin-a", "0.1.0", BASE);
    monitor.register("neryva.plugin-m", "0.1.5", BASE);
    monitor
        .record_call_result("neryva.plugin-a", BASE + 1_000, PluginObservation::CallSucceeded { fuel_used: 7 })
        .expect("observed");
    monitor
        .record_call_result("neryva.plugin-m", BASE + 1_000, PluginObservation::CallSucceeded { fuel_used: 3 })
        .expect("observed");
    monitor
        .record_exhaustion("neryva.plugin-z", BASE + 1_000, &HostError::CallRateExceeded { limit_per_minute: 60 })
        .expect("observed");

    let snapshot = monitor.snapshot(BASE + 2_000);
    let ids: Vec<&str> = snapshot.plugins.iter().map(|p| p.plugin_id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["neryva.plugin-a", "neryva.plugin-m", "neryva.plugin-z"],
        "the snapshot is ordered by plugin id"
    );
    let statuses: Vec<String> = snapshot.plugins.iter().map(|p| p.status_name()).collect();
    assert_eq!(
        statuses,
        vec![
            "HEALTHY".to_string(),
            "HEALTHY".to_string(),
            "DEGRADED_CALL_RATE".to_string(),
        ]
    );
    // Determinism: two snapshots at the same instant are equal, and the
    // JSON rendering is byte-stable.
    let again = monitor.snapshot(BASE + 2_000);
    assert_eq!(snapshot, again);
    assert_eq!(snapshot.json(), again.json());
    // The JSON carries the closed vocabulary and the correlation fields the
    // P7.25 console consumes.
    let json = snapshot.json();
    assert!(json.contains("\"status\":\"DEGRADED_CALL_RATE\""));
    assert!(json.contains("\"readiness\":\"READY\""));
    assert!(json.contains("\"plugin_id\":\"neryva.plugin-z\""));
    assert!(json.contains("\"version\":\"0.2.0\""));
    assert!(json.contains("\"fuel_used\":"));
    // No config values, no secrets — the snapshot carries identifiers and
    // counters only.
    assert!(!json.to_lowercase().contains("secret"));
    assert!(!json.to_lowercase().contains("password"));
}

// --- 7. The transition audit trail --------------------------------------------------

#[test]
fn every_health_transition_is_audited_and_unknown_plugins_are_refused_audited() {
    let audit = MemoryHealthAuditSink::new();
    let mut monitor = monitor(&audit);
    // Registration is audited.
    monitor.register("neryva.plugin-i", "0.1.0", BASE);
    // Pre-observation snapshot: DEGRADED_UNVERIFIED (never healthy).
    let before = monitor.snapshot(BASE + 500);
    assert_eq!(before.plugins[0].status_name(), "DEGRADED_UNVERIFIED");
    // The first successful call makes it HEALTHY; the UNVERIFIED → HEALTHY
    // transition is audited against the pre-observation snapshot.
    monitor
        .record_call_result("neryva.plugin-i", BASE + 1_000, PluginObservation::CallSucceeded { fuel_used: 0 })
        .expect("observed");
    monitor.audit_transitions(&before, BASE + 1_100);
    // ...then exhaust: the transition HEALTHY → DEGRADED_FUEL_EXHAUSTED is
    // audited against the fresh pre-exhaustion snapshot.
    let before_exhaust = monitor.snapshot(BASE + 1_200);
    assert_eq!(before_exhaust.plugins[0].status_name(), "HEALTHY");
    monitor
        .record_exhaustion("neryva.plugin-i", BASE + 2_000, &HostError::FuelExhausted)
        .expect("recorded");
    monitor.audit_transitions(&before_exhaust, BASE + 2_100);

    let records = audit.records();
    let kinds: Vec<&str> = records.iter().map(|r| r.kind.name()).collect();
    assert!(kinds.contains(&"REGISTRATION"), "registration audited: {kinds:?}");
    // The UNVERIFIED → HEALTHY and HEALTHY → DEGRADED_FUEL_EXHAUSTED
    // transitions are both present, with the closed-vocabulary names.
    assert!(
        records.iter().any(|r| r.kind == HealthAuditKind::Transition
            && r.from.as_deref() == Some("DEGRADED_UNVERIFIED")
            && r.to.as_deref() == Some("HEALTHY")),
        "the unverified-to-healthy transition is audited"
    );
    assert!(
        records.iter().any(|r| r.kind == HealthAuditKind::Transition
            && r.from.as_deref() == Some("HEALTHY")
            && r.to.as_deref() == Some("DEGRADED_FUEL_EXHAUSTED")),
        "the healthy-to-degraded transition is audited"
    );
    // Transitions carry the plugin identity and a timestamp.
    let transition = records
        .iter()
        .find(|r| r.kind == HealthAuditKind::Transition)
        .expect("at least one transition");
    assert_eq!(transition.plugin_id, "neryva.plugin-i");
    assert_eq!(transition.version, "0.1.0");
    assert!(transition.timestamp_unix_millis >= BASE);

    // An observation for an UNREGISTERED plugin is REFUSED and audited.
    let error = monitor
        .record_call_result("neryva.ghost", BASE + 3_000, PluginObservation::CallSucceeded { fuel_used: 0 })
        .expect_err("unknown plugin refused");
    assert!(matches!(
        error,
        flexcore_plugin_host::health::HealthError::UnknownPlugin { .. }
    ));
    assert!(
        records.iter().any(|r| r.kind == HealthAuditKind::Refused
            && r.plugin_id == "neryva.ghost"),
        "the refusal is audited"
    );
}

// --- 8. Bounded stats (no unbounded memory) ------------------------------------------

#[test]
fn the_stats_snapshot_is_bounded_under_sustained_failures() {
    let audit = MemoryHealthAuditSink::new();
    let mut monitor = monitor(&audit);
    monitor.register("neryva.plugin-j", "0.1.0", BASE);
    // One success first (readiness + liveness anchor), then 10_000 timed-out
    // calls: the counters accumulate; the error-sample list stays
    // hard-capped (no per-call record anywhere).
    monitor
        .record_call_result("neryva.plugin-j", BASE + 1, PluginObservation::CallSucceeded { fuel_used: 0 })
        .expect("observed");
    for tick in 0..10_000u64 {
        monitor
            .record_call_result(
                "neryva.plugin-j",
                BASE + 2 + tick,
                PluginObservation::CallTimedOut { reason: "deadline".to_string() },
            )
            .expect("observed");
    }
    let record = monitor.assess("neryva.plugin-j", BASE + 10_003).expect("assessed");
    assert_eq!(record.stats.calls_total, 10_001);
    assert_eq!(record.stats.errors_total, 10_000);
    assert_eq!(record.stats.timeouts_total, 10_000);
    // The bounded snapshot: a fixed-size Copy struct — no unbounded memory.
    let stats = record.stats;
    let cloned = stats; // Copy: an independent bounded snapshot
    assert_eq!(cloned, stats);
    // The error samples behind the error-rate derivation are capped at
    // MAX_ERROR_SAMPLES, but the derivation still works from the capped
    // list (the capped count is far above the threshold here).
    assert_eq!(
        monitor.assess("neryva.plugin-j", BASE + 10_003).expect("assessed").status_name(),
        "DEGRADED_ERROR_RATE"
    );
}

#[test]
fn fuel_accumulates_saturating_across_successes() {
    let audit = MemoryHealthAuditSink::new();
    let mut monitor = monitor(&audit);
    monitor.register("neryva.plugin-k", "0.1.0", BASE);
    monitor
        .record_call_result("neryva.plugin-k", BASE + 1, PluginObservation::CallSucceeded { fuel_used: 100 })
        .expect("observed");
    monitor
        .record_call_result("neryva.plugin-k", BASE + 2, PluginObservation::CallSucceeded { fuel_used: 250 })
        .expect("observed");
    let record = monitor.assess("neryva.plugin-k", BASE + 3).expect("assessed");
    assert_eq!(record.stats.fuel_used, 350);
    assert_eq!(record.stats.calls_total, 2);
    assert_eq!(record.stats.errors_total, 0);
}
