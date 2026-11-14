// P6.17 — Site-local policy enforcement and adapter health monitoring
//         (ledger P6.17; test file written, NOT run — the loop bans in
//         `ledger/phase-06-edge-protocol/ENTRY_NOTE.md` hold; the human
//         build/test pass runs these).
//
// Coverage (each outcome reject + audited, never silently proceeded):
//
// 1. **Adapter process/device failure** → FAILING_UNRESPONSIVE within the
//    detection window (an explicit failed poll fails immediately; a silent
//    death fails when `now - last_seen` crosses the window — a positive,
//    timestamped rule, never "absence of errors").
// 2. **Stale data** → DEGRADED_STALE_DATA against the freshness limit.
// 3. **Policy violation attempt** (disabled adapter) → rejected + audited
//    through the P6.14 receiver seam + health visibly degraded
//    (DEGRADED_POLICY_BLOCK).
// 4. **Rate-limit exceeded** → rejected + audited; the refusal consumes no
//    budget; after the window the budget returns.
// 5. **Health never reports healthy before the first observation** →
//    DEGRADED_UNVERIFIED (the forbidden-inference rule).
// 6. **A FAILING adapter blocks dependent actions** — ingest from it and
//    command targeting, each with an auditable reason.
// 7. **Versioned policy configuration** parses strictly (unknown fields /
//    wrong version / wrapped windows are named errors); the engine
//    implements the command path's `CommandPolicyGate` seam (P6.14 ↔
//    P6.17, no cycle).
// 8. **Certificate expiry** (the P6.06 credential-store input) is a health
//    input with the rotation-window and expired thresholds; the
//    HealthSnapshot renders the closed vocabulary for P6.25.
//
// In-process only; no network.

use std::sync::Arc;

use flexcore_edge_agent::command_path::{
    CommandPathGateInputs, CommandPathRuntime, CommandReceiver, CommandRefusal, EdgeCommand,
    EnablementKey, SignedEnablementRecord, CommandPolicyGate,
};
use flexcore_edge_agent::health::{
    AdapterHealthMonitor, AdapterObservation, BlockedAction, HealthLimits, HealthReason,
    HealthState,
};
use flexcore_edge_agent::policy::{
    AdapterPolicy, OperatingWindow, PolicyError, PolicyViolation, RateLimit, SitePolicyConfig,
    SitePolicyEngine, MILLIS_PER_DAY, SITE_POLICY_FORMAT, SITE_POLICY_FORMAT_VERSION,
};
use flexcore_edge_agent::ports::{AuditSink, InMemoryAuditSink};
use flexcore_ports::clock::Clock;
use flexcore_ports::fakes::FixedClock;

const BASE_MILLIS: i64 = 1_700_000_000_000;

fn limits() -> HealthLimits {
    HealthLimits {
        freshness_limit_millis: 30_000,
        unresponsive_after_millis: 60_000,
        error_rate_window_millis: 300_000,
        max_errors_in_window: 3,
        credential_rotation_window_millis: 7 * 86_400_000,
    }
}

fn policy_config(enabled: bool, max_commands: u32) -> SitePolicyConfig {
    SitePolicyConfig {
        format_version: SITE_POLICY_FORMAT_VERSION,
        adapters: vec![AdapterPolicy {
            adapter_id: "simulated-test".to_string(),
            enabled,
            rate_limit: RateLimit {
                max_commands,
                window_millis: 60_000,
            },
            operating_window: OperatingWindow {
                start_millis_of_day: 0,
                end_millis_of_day: MILLIS_PER_DAY,
            },
        }],
    }
}

/// Builds the P6.14 receiver over the given policy engine (the P6.17
/// re-check seam wired end to end).
fn gated_receiver(
    policy: SitePolicyEngine,
    audit: Arc<InMemoryAuditSink>,
) -> CommandReceiver {
    let key = EnablementKey::new(
        "lab-enablement-key-material".to_string(),
        "deployment:lab-key-1".to_string(),
    )
    .expect("valid key");
    let record = SignedEnablementRecord {
        site_id: "pilot-site-A".to_string(),
        approver: "J. Doe (security lead)".to_string(),
        adapter_scope: vec!["simulated-test".to_string()],
        enablement_token: "lab-token-123".to_string(),
        issued_at_unix_millis: BASE_MILLIS,
        valid_until_unix_millis: BASE_MILLIS + 3_600_000,
        key_reference: "deployment:lab-key-1".to_string(),
        digest: None,
    }
    .sign(&key)
    .expect("the record signs");
    let inputs = CommandPathGateInputs {
        site_id: "pilot-site-A".to_string(),
        config_flag_enabled: true,
        signed_record: Some(record),
        environment_token: Some("lab-token-123".to_string()),
        command_allowlist: vec!["simulated-test".to_string()],
        d6_record_reference: Some("d6:record".to_string()),
    };
    let runtime = CommandPathRuntime::evaluate(&inputs, &key, BASE_MILLIS);
    CommandReceiver::new(
        runtime,
        Arc::new(policy) as Arc<dyn CommandPolicyGate>,
        audit as Arc<dyn AuditSink>,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000)),
    )
}

fn issued_command() -> EdgeCommand {
    EdgeCommand::new(
        "cmd-1".to_string(),
        "ess-01".to_string(),
        "ISSUED".to_string(),
        1,
    )
    .expect("valid command")
}

#[test]
fn adapter_failure_lands_failing_unresponsive_within_the_detection_window() {
    let mut monitor = AdapterHealthMonitor::new(limits()).expect("valid limits");
    // Observed responsive at BASE (event 1 s old).
    monitor.observe(
        "simulated-test",
        BASE_MILLIS,
        AdapterObservation::Responsive { event_age_millis: 1_000 },
    );
    // Inside the window: healthy.
    assert_eq!(
        monitor
            .assess("simulated-test", BASE_MILLIS + 59_000)
            .expect("assessed")
            .status_name(),
        "HEALTHY"
    );
    // The device/process dies with NO further observation: at the 60 s
    // detection window the last-seen rule fires → FAILING_UNRESPONSIVE.
    let record = monitor
        .assess("simulated-test", BASE_MILLIS + 60_001)
        .expect("assessed");
    assert_eq!(record.status_name(), "FAILING_UNRESPONSIVE");
    assert!(record.blocks_dependent_actions());

    // An explicit failed poll lands the same state IMMEDIATELY (and the
    // reason travels to the detail, human-safe).
    monitor.observe(
        "simulated-test",
        BASE_MILLIS + 61_000,
        AdapterObservation::PollFailed {
            reason: "the responsive check timed out".to_string(),
        },
    );
    assert_eq!(
        monitor
            .assess("simulated-test", BASE_MILLIS + 61_500)
            .expect("assessed")
            .status_name(),
        "FAILING_UNRESPONSIVE"
    );
}

#[test]
fn adapter_returning_stale_data_is_degraded_stale_data() {
    let mut monitor = AdapterHealthMonitor::new(limits()).expect("valid limits");
    monitor.observe(
        "simulated-test",
        BASE_MILLIS,
        AdapterObservation::Responsive { event_age_millis: 1_000 },
    );
    // The adapter keeps ANSWERING but its data ages past the freshness
    // limit (46 s old at BASE+45_000 vs the 30 s limit) → degraded, not
    // failing (it is up; its data is not trustworthy as fresh).
    let record = monitor
        .assess("simulated-test", BASE_MILLIS + 45_000)
        .expect("assessed");
    assert_eq!(record.status_name(), "DEGRADED_STALE_DATA");
    assert_eq!(record.last_event_age_millis, Some(46_000));
    assert!(!record.blocks_dependent_actions());
    // A fresh observation recovers the state.
    monitor.observe(
        "simulated-test",
        BASE_MILLIS + 46_000,
        AdapterObservation::Responsive { event_age_millis: 500 },
    );
    assert_eq!(
        monitor
            .assess("simulated-test", BASE_MILLIS + 46_500)
            .expect("assessed")
            .status_name(),
        "HEALTHY"
    );
}

#[test]
fn a_policy_violation_is_rejected_audited_and_visibly_degrades_health() {
    // The adapter is DISABLED by the site policy: the receiver refuses,
    // the refusal is audited, and the composition notes the policy block
    // on the health monitor (the wiring the decision doc §7 itemises).
    let policy = SitePolicyEngine::new(policy_config(false, 100)).expect("valid policy");
    let audit = Arc::new(InMemoryAuditSink::new());
    let mut rx = gated_receiver(policy, Arc::clone(&audit));
    let mut monitor = AdapterHealthMonitor::new(limits()).expect("valid limits");
    monitor.observe(
        "simulated-test",
        BASE_MILLIS,
        AdapterObservation::Responsive { event_age_millis: 100 },
    );

    let error = rx
        .receive(&issued_command(), "simulated-test")
        .expect_err("a disabled adapter is refused by the independent gate");
    assert_eq!(error.code(), "policy_refused");
    let details: Vec<String> = audit.events().iter().map(|e| e.detail.clone()).collect();
    assert!(
        details
            .iter()
            .any(|d| d.starts_with("EDGE_COMMAND_REJECTED") && d.contains("policy_adapter_disabled")),
        "the policy refusal is audited: {details:?}"
    );

    // The composition notes the block; health visibly degrades.
    monitor.note_policy_block("simulated-test");
    let record = monitor
        .assess("simulated-test", BASE_MILLIS + 1_000)
        .expect("assessed");
    assert_eq!(record.status_name(), "DEGRADED_POLICY_BLOCK");
    // And the block is clearable after the policy window rolls over.
    monitor.clear_policy_block("simulated-test");
    assert_eq!(
        monitor
            .assess("simulated-test", BASE_MILLIS + 1_000)
            .expect("assessed")
            .status_name(),
        "HEALTHY"
    );
}

#[test]
fn a_rate_limit_exceeded_is_rejected_and_audited_and_consumes_no_budget() {
    let policy = SitePolicyEngine::new(policy_config(true, 1)).expect("valid policy");
    let audit = Arc::new(InMemoryAuditSink::new());
    let mut rx = gated_receiver(policy, Arc::clone(&audit));

    // First command admitted.
    let first = rx.receive(&issued_command(), "simulated-test");
    assert!(first.is_ok(), "the first command is admitted");
    // Second within the window: refused by the rate limit, audited.
    let error = rx
        .receive(&issued_command(), "simulated-test")
        .expect_err("the rate limit refuses the second command inside the window");
    assert_eq!(error.code(), "policy_refused");
    let details: Vec<String> = audit.events().iter().map(|e| e.detail.clone()).collect();
    assert!(
        details
            .iter()
            .any(|d| d.starts_with("EDGE_COMMAND_REJECTED") && d.contains("policy_rate_limit_exceeded")),
        "the rate-limit refusal is audited: {details:?}"
    );
    assert_eq!(
        details
            .iter()
            .filter(|d| d.starts_with("EDGE_COMMAND_ACCEPTED"))
            .count(),
        1,
        "the refusal consumed no budget (still one acceptance)"
    );

    // The engine-level view: the admitted count stays at the accepted
    // commands, and the violation is the named variant.
    let engine = SitePolicyEngine::new(policy_config(true, 1)).expect("valid policy");
    let now = BASE_MILLIS;
    assert!(engine.admit_command("simulated-test", now).is_ok());
    assert!(matches!(
        engine.admit_command("simulated-test", now + 1),
        Err(PolicyViolation::RateLimitExceeded { .. })
    ));
    // After the window: budget returns.
    assert!(engine.admit_command("simulated-test", now + 60_001).is_ok());
}

#[test]
fn health_never_reports_healthy_before_the_first_observation() {
    let mut monitor = AdapterHealthMonitor::new(limits()).expect("valid limits");
    // Registered (the composition fed the credential input) but never
    // observed: DEGRADED_UNVERIFIED — "unknown = healthy" is forbidden.
    monitor.set_credential_expiry("simulated-test", Some(BASE_MILLIS + 30 * 86_400_000));
    let record = monitor
        .assess("simulated-test", BASE_MILLIS)
        .expect("assessed");
    assert_eq!(record.status_name(), "DEGRADED_UNVERIFIED");
    assert_eq!(record.state, HealthState::Degraded(HealthReason::Unverified));
    // The snapshot carries the same honesty.
    let snapshot = monitor.snapshot(BASE_MILLIS);
    assert_eq!(
        snapshot.adapters[0].status_name(),
        "DEGRADED_UNVERIFIED"
    );
    // An adapter the monitor has never seen at all is a named error, not
    // a status.
    assert!(monitor.assess("ghost", BASE_MILLIS).is_err());
}

#[test]
fn a_failing_adapter_blocks_ingest_and_command_targeting() {
    let mut monitor = AdapterHealthMonitor::new(limits()).expect("valid limits");
    monitor.observe(
        "simulated-test",
        BASE_MILLIS,
        AdapterObservation::Responsive { event_age_millis: 100 },
    );
    // Healthy: both dependent actions allowed.
    assert!(monitor
        .check_action_allowed("simulated-test", BlockedAction::Ingest, BASE_MILLIS + 1_000)
        .is_ok());
    assert!(monitor
        .check_action_allowed("simulated-test", BlockedAction::Command, BASE_MILLIS + 1_000)
        .is_ok());
    // FAILING (device unreachable): both blocked, each with an auditable
    // reason naming the action and the status.
    monitor.observe(
        "simulated-test",
        BASE_MILLIS + 2_000,
        AdapterObservation::PollFailed { reason: "device unreachable".to_string() },
    );
    for action in [BlockedAction::Ingest, BlockedAction::Command] {
        let reason = monitor
            .check_action_allowed("simulated-test", action, BASE_MILLIS + 3_000)
            .expect_err("blocked while FAILING");
        assert!(reason.contains(action.as_str()), "the reason names the action: {reason}");
        assert!(reason.contains("FAILING_UNRESPONSIVE"));
    }
}

#[test]
fn the_versioned_policy_document_parses_strictly_and_the_seam_is_implemented() {
    // Strict parse: the versioned document loads.
    let text = format!(
        "{SITE_POLICY_FORMAT}\nadapter = simulated-test\nenabled = true\nrate_limit = 2/60000\n\
         operating_window = 0800-1800\n"
    );
    let config = SitePolicyConfig::parse(&text).expect("parses");
    assert_eq!(config.format_version, SITE_POLICY_FORMAT_VERSION);
    let engine = SitePolicyEngine::new(config).expect("loads");
    assert_eq!(engine.adapter_ids(), vec!["simulated-test"]);
    assert!(engine.is_adapter_enabled("simulated-test"));

    // Unknown fields are named errors (never silently accepted).
    let smuggled = format!("{SITE_POLICY_FORMAT}\nextra = sneaky\n");
    assert!(matches!(
        SitePolicyConfig::parse(&smuggled),
        Err(PolicyError::Malformed { .. })
    ));
    // A wrong format version is a named error.
    let wrong = "flexcore-site-policy-v0\nadapter = a\n".to_string();
    assert!(matches!(
        SitePolicyConfig::parse(&wrong),
        Err(PolicyError::Malformed { line: 1, .. })
    ));
    // A wrapped operating window is a validation error.
    let mut wrapped = SitePolicyConfig::parse(&text).expect("parses");
    wrapped.adapters[0].operating_window = OperatingWindow {
        start_millis_of_day: 20 * 3_600_000,
        end_millis_of_day: 4 * 3_600_000,
    };
    assert!(matches!(
        SitePolicyConfig::validate(&wrapped),
        Err(PolicyError::InvalidPolicy { .. })
    ));

    // The seam: the engine implements the command path's CommandPolicyGate
    // (the receiver consults it as the independent P6.17 gate).
    let gate: &dyn CommandPolicyGate = &engine;
    // 09:00 UTC on the epoch day is inside 0800-1800.
    let nine_am = BASE_MILLIS - (BASE_MILLIS % MILLIS_PER_DAY) + 9 * 3_600_000;
    assert!(gate.check_command("simulated-test", nine_am).is_ok());
    assert!(gate.check_command("simulated-test", nine_am + 1).is_ok());
    let refusal = gate
        .check_command("simulated-test", nine_am + 2)
        .expect_err("rate limit exceeded");
    assert_eq!(refusal.code, "policy_rate_limit_exceeded");
    // Outside the operating window: refused with the window refusal.
    let three_am = BASE_MILLIS - (BASE_MILLIS % MILLIS_PER_DAY) + 3 * 3_600_000;
    let engine = SitePolicyEngine::new(SitePolicyConfig::parse(&text).expect("parses"))
        .expect("loads");
    assert!(matches!(
        engine.admit_command("simulated-test", three_am),
        Err(PolicyViolation::OutsideOperatingWindow { .. })
    ));
    // Unknown adapter: refused, never defaulted.
    assert!(matches!(
        engine.admit_command("ghost", nine_am),
        Err(PolicyViolation::UnknownAdapter { .. })
    ));
}

#[test]
fn credential_expiry_is_a_health_input_and_the_snapshot_is_console_ready() {
    let mut monitor = AdapterHealthMonitor::new(limits()).expect("valid limits");
    monitor.observe(
        "aaa-adapter",
        BASE_MILLIS,
        AdapterObservation::Responsive { event_age_millis: 100 },
    );
    monitor.observe(
        "bbb-adapter",
        BASE_MILLIS,
        AdapterObservation::Responsive { event_age_millis: 100 },
    );
    // bbb's credential expires 3 days out (inside the 7-day rotation
    // window) → DEGRADED; aaa stays HEALTHY.
    monitor.set_credential_expiry("bbb-adapter", Some(BASE_MILLIS + 3 * 86_400_000));
    let snapshot = monitor.snapshot(BASE_MILLIS + 1_000);
    let names: Vec<String> = snapshot.adapters.iter().map(|r| r.status_name()).collect();
    assert_eq!(
        names,
        vec!["HEALTHY".to_string(), "DEGRADED_CREDENTIAL_EXPIRY".to_string()],
        "the snapshot is ordered by adapter id with the closed vocabulary"
    );
    // Expired credential → FAILING (dependent actions blocked).
    monitor.set_credential_expiry("bbb-adapter", Some(BASE_MILLIS - 1));
    let record = monitor
        .assess("bbb-adapter", BASE_MILLIS + 1_000)
        .expect("assessed");
    assert_eq!(record.status_name(), "FAILING_CREDENTIAL_EXPIRY");
    assert!(monitor
        .check_action_allowed("bbb-adapter", BlockedAction::Command, BASE_MILLIS + 1_000)
        .is_err());

    // The console-facing rendering (P6.25's seam): flat JSON with the
    // closed status names and the last-seen timestamps.
    let rendered = snapshot.json();
    assert!(rendered.contains("\"adapter_id\":\"aaa-adapter\""));
    assert!(rendered.contains("\"status\":\"HEALTHY\""));
    assert!(rendered.contains("\"status\":\"DEGRADED_CREDENTIAL_EXPIRY\""));
    assert!(rendered.contains("\"last_seen_unix_millis\":1700000000000"));
    // Detail strings are escaped (a quote in an adapter id cannot break
    // the rendering).
    monitor.observe(
        "weird\"id",
        BASE_MILLIS,
        AdapterObservation::Responsive { event_age_millis: 100 },
    );
    let escaped = monitor.snapshot(BASE_MILLIS + 1_000).json();
    assert!(escaped.contains("weird\\\"id"), "quotes are escaped: {escaped}");
}

#[test]
fn nonsense_policy_and_limits_are_named_composition_errors() {
    // Zero rate limit, empty adapter set, and zero health limits are all
    // load-time errors — a nonsense policy never silently enforces
    // nothing.
    assert!(SitePolicyEngine::new(SitePolicyConfig {
        format_version: 1,
        adapters: Vec::new(),
    })
    .is_err());
    let mut zero_limit = policy_config(true, 1);
    zero_limit.adapters[0].rate_limit.max_commands = 0;
    assert!(SitePolicyEngine::new(zero_limit).is_err());
    let mut zero_freshness = limits();
    zero_freshness.freshness_limit_millis = 0;
    assert!(AdapterHealthMonitor::new(zero_freshness).is_err());
}
