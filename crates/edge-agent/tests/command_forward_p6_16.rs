// P6.16 — Integration tests: forwarding only to allowlisted adapters;
//         fallback on failure.
//
// EVERY TEST BELOW IS UNRUN (the loop bans in
// `ledger/phase-06-edge-protocol/ENTRY_NOTE.md` forbid building and running
// tests; the human build/test pass owns execution). The failure-mode cases
// the ledger names, each landing in its specified §14.1 terminal/FALLBACK
// state with an audit record:
//
// - adapter NACK → FAILED, then FALLBACK per the configured per-asset policy;
// - adapter timeout → FAILED, then FALLBACK;
// - no acknowledgement → FAILED, then FALLBACK, with the P4/P5 honesty
//   wording (acknowledgement is NOT physical delivery);
// - tracking deviation → FAILED, then FALLBACK (on-target tracking and
//   completion reach TRACKING/COMPLETED);
// - command expiry mid-flight → EXPIRED, with FALLBACK ONLY where the
//   per-asset policy configures the expiry trigger;
// - no command reaches a non-allowlisted adapter (the forwarder port is
//   NEVER invoked for it — asserted against the call recorder);
// - fallback is per the configured policy per asset; an unconfigured asset
//   defaults to REQUIRE_OPERATOR_INTERVENTION with NO FALLBACK transition;
// - a missing operator-review field refuses the config load;
// - the PRESERVE_RESERVE_SOC variant stops new commands for the asset;
// - the production fail-closed stub fails every forward named;
// - the full gated pipeline (P6.14 gate → P6.15 verifier → P6.16 engine)
//   flows one valid command end to end in the test env.

use std::sync::{Arc, Mutex};

use flexcore_ports::clock::Clock;
use flexcore_ports::fakes::FixedClock;

use flexcore_edge_agent::command_forward::{
    ACKNOWLEDGEMENT_IS_NOT_DELIVERY, FailClosedCommandForwarder, FallbackPolicyConfig,
    FallbackVariant, ForwardError, ForwardOutcome, ForwardRequest, ForwardingRefusal,
    CommandForwarder, CommandForwardingEngine, SiteCommandAllowlist, TrackingObservation,
};
use flexcore_edge_agent::command_path::{
    CommandPathGateInputs, CommandPathRuntime, CommandReceiver, EdgeCommand, EnablementKey,
    GatedCommandPipeline, SignedEnablementRecord,
};
use flexcore_edge_agent::command_verify::{
    sign_command, AssetCommandScope, CommandAuthorization, CommandSigningKey, CommandVerifier,
    FileSequenceCheckpoint, IncomingCommand, VerifiedCommand,
};
use flexcore_edge_agent::policy::{
    AdapterPolicy, OperatingWindow, RateLimit, SitePolicyConfig, SitePolicyEngine,
};
use flexcore_edge_agent::ports::{AuditSink, InMemoryAuditSink};

const BASE_MILLIS: i64 = 1_700_000_000_000;
const VALID_UNTIL: i64 = BASE_MILLIS + 600_000;

// --- Fixtures ---------------------------------------------------------------------

/// A std-only unique scratch directory per call (the buffer_p6_09 pattern).
fn scratch_dir(label: &str) -> std::path::PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "flexcore-edge-p616-it-{}-{}-{}",
        std::process::id(),
        label,
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir creates");
    dir
}

/// The scripted forwarder test double: records EVERY request it receives
/// (the never-invoked-for-non-allowlisted assertion reads this) and answers
/// per its configured behaviour.
struct ScriptedForwarder {
    calls: Mutex<Vec<ForwardRequest>>,
    behaviour: Box<dyn Fn(&ForwardRequest) -> Result<ForwardOutcome, ForwardError> + Send + Sync>,
}

impl ScriptedForwarder {
    fn new(
        behaviour: Box<
            dyn Fn(&ForwardRequest) -> Result<ForwardOutcome, ForwardError> + Send + Sync,
        >,
    ) -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            behaviour,
        })
    }

    fn call_count(&self) -> usize {
        self.calls.lock().expect("poisoned call mutex").len()
    }
}

impl CommandForwarder for ScriptedForwarder {
    fn forward(&self, request: &ForwardRequest) -> Result<ForwardOutcome, ForwardError> {
        self.calls
            .lock()
            .expect("poisoned call mutex")
            .push(request.clone());
        (self.behaviour)(request)
    }
}

fn always_ack() -> Arc<ScriptedForwarder> {
    ScriptedForwarder::new(Box::new(|_request| Ok(ForwardOutcome::Acknowledged)))
}

fn always_nack(reason: &'static str) -> Arc<ScriptedForwarder> {
    ScriptedForwarder::new(Box::new(move |_request| {
        Ok(ForwardOutcome::Nack {
            reason: reason.to_string(),
        })
    }))
}

/// The permissive P6.17 policy engine (rate 100/60 s, all-day window, the
/// adapter enabled).
fn permissive_policy() -> SitePolicyEngine {
    SitePolicyEngine::new(SitePolicyConfig {
        format_version: 1,
        adapters: vec![AdapterPolicy {
            adapter_id: "simulated-test".to_string(),
            enabled: true,
            rate_limit: RateLimit {
                max_commands: 100,
                window_millis: 60_000,
            },
            operating_window: OperatingWindow {
                start_millis_of_day: 0,
                end_millis_of_day: 86_400_000,
            },
        }],
    })
    .expect("valid policy")
}

/// The fallback policy used by most tests: per-asset entries, EVERY one
/// carrying its (NOT-SIGNED, recorded) operator review — required to load.
fn fallback_config() -> FallbackPolicyConfig {
    let mut text = String::from("flexcore-edge-fallback-policy-v1\n");
    // ess-01: HOLD, all failure triggers except expiry.
    text.push_str(
        "asset = ess-01\nvariant = HOLD_LAST_ACCEPTED_SETPOINT\n\
         bounded_interval_millis = 300000\n\
         triggers = nack,timeout,no_ack,tracking_deviation\n\
         reviewed_by = A. Operator (site operator)\nreviewed_on = 2026-11-13\n",
    );
    // ess-02: PRESERVE_RESERVE_SOC + stop accepting (the stop-half engine).
    text.push_str(
        "asset = ess-02\nvariant = \
         PRESERVE_RESERVE_SOC_CONSTRAINTS_AND_STOP_ACCEPTING_NEW_COMMANDS\n\
         triggers = nack\nreviewed_by = B. Reviewer (equipment operator)\n\
         reviewed_on = 2026-11-12\n",
    );
    // ess-03: REQUIRE_OPERATOR_INTERVENTION as the CONFIGURED variant.
    text.push_str(
        "asset = ess-03\nvariant = REQUIRE_OPERATOR_INTERVENTION\ntriggers = nack,timeout\n\
         reviewed_by = C. Operator (site operator)\nreviewed_on = 2026-11-11\n",
    );
    // ess-04: expiry IS a configured trigger.
    text.push_str(
        "asset = ess-04\nvariant = RETURN_TO_LOCAL_EMS_AUTONOMOUS\ntriggers = expiry,nack\n\
         reviewed_by = D. Operator (site operator)\nreviewed_on = 2026-11-10\n",
    );
    // ess-05: expiry is NOT configured (trigger-not-configured outcome).
    text.push_str(
        "asset = ess-05\nvariant = RETURN_TO_LOCAL_EMS_AUTONOMOUS\ntriggers = nack\n\
         reviewed_by = E. Operator (site operator)\nreviewed_on = 2026-11-09\n",
    );
    FallbackPolicyConfig::parse(&text).expect("the fixture fallback policy parses")
}

/// A verified command (as the P6.15 verifier emits) for the given asset.
fn verified(command_id: &str, asset_key: &str, target_adapter: &str, sequence: u64) -> VerifiedCommand {
    VerifiedCommand {
        command_id: command_id.to_string(),
        asset_key: asset_key.to_string(),
        target_adapter: target_adapter.to_string(),
        quantity: "active_power".to_string(),
        value: 1.5,
        sequence,
        verified_at_unix_millis: BASE_MILLIS + 1_000,
        valid_until_unix_millis: VALID_UNTIL,
        scope_note: "edge verification passed in the TEST ENVIRONMENT ONLY".to_string(),
    }
}

fn engine_with(
    forwarder: Arc<dyn CommandForwarder>,
    allowlist: SiteCommandAllowlist,
    fallback: FallbackPolicyConfig,
    audit: Arc<dyn AuditSink>,
    clock: Arc<dyn Clock + Send + Sync>,
) -> CommandForwardingEngine {
    CommandForwardingEngine::new(
        forwarder,
        Arc::new(permissive_policy()),
        allowlist,
        fallback,
        audit,
        clock,
    )
    .expect("the engine composes")
}

fn default_engine(
    forwarder: Arc<dyn CommandForwarder>,
    audit: Arc<dyn AuditSink>,
) -> CommandForwardingEngine {
    engine_with(
        forwarder,
        SiteCommandAllowlist::new(vec!["simulated-test".to_string()]),
        fallback_config(),
        audit,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000)),
    )
}

/// The audit detail lines of a sink.
fn details(audit: &InMemoryAuditSink) -> Vec<String> {
    audit
        .events()
        .iter()
        .map(|event| event.detail.clone())
        .collect()
}

// --- 1. NACK / timeout / no-ack: FAILED then FALLBACK per policy --------------------

#[test]
fn an_adapter_nack_lands_in_failed_then_fallback_per_the_configured_policy() {
    let audit = Arc::new(InMemoryAuditSink::new());
    let forwarder = always_nack("the simulated adapter refused the setpoint");
    let engine = default_engine(forwarder, Arc::clone(&audit) as Arc<dyn AuditSink>);

    let record = engine
        .forward_verified(verified("cmd-1", "ess-01", "simulated-test", 1))
        .expect("the forward itself is not refused; it FAILS at the adapter");

    // §14.1: the NACK (the frozen ISSUED → FAILED trigger) plus the
    // configured HOLD fallback → the terminal state is FALLBACK.
    assert_eq!(record.lifecycle_state, "FALLBACK");
    assert_eq!(record.asset_key, "ess-01");
    assert!(record.detail.contains("NACK"));
    assert!(record.detail.contains("300000"), "the bounded interval is in the record");
    assert_eq!(
        engine.lifecycle_state_of("cmd-1").as_deref(),
        Some("FALLBACK")
    );

    let lines = details(&audit);
    // The FAILED transition is audited...
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("EDGE_COMMAND_LIFECYCLE") && line.contains("state=FAILED")),
        "the FAILED transition is audited: {lines:?}"
    );
    // ...and the FALLBACK transition carries the variant, the bounded
    // interval, and the operator-review record (NOT SIGNED — human gate
    // input).
    let fallback_line = lines
        .iter()
        .find(|line| line.starts_with("EDGE_COMMAND_FALLBACK"))
        .expect("the fallback activation is audited");
    assert!(fallback_line.contains("variant=HOLD_LAST_ACCEPTED_SETPOINT"));
    assert!(fallback_line.contains("reviewed_by='A. Operator (site operator)'"));
    assert!(fallback_line.contains("NOT SIGNED"));
}

#[test]
fn an_adapter_timeout_lands_in_failed_then_fallback() {
    let audit = Arc::new(InMemoryAuditSink::new());
    let forwarder = ScriptedForwarder::new(Box::new(|_request| {
        Err(ForwardError::Timeout {
            reason: "no answer within the adapter window".to_string(),
        })
    }));
    let engine = default_engine(forwarder, Arc::clone(&audit) as Arc<dyn AuditSink>);
    let record = engine
        .forward_verified(verified("cmd-1", "ess-01", "simulated-test", 1))
        .expect("recorded");
    assert_eq!(record.lifecycle_state, "FALLBACK");
    assert!(record.detail.contains("timed out"));
    let lines = details(&audit);
    assert!(lines
        .iter()
        .any(|line| line.starts_with("EDGE_COMMAND_LIFECYCLE") && line.contains("state=FAILED")));
    assert!(lines
        .iter()
        .any(|line| line.starts_with("EDGE_COMMAND_FALLBACK")
            && line.contains("trigger=timeout")));
}

#[test]
fn no_acknowledgement_lands_in_failed_then_fallback_with_the_honesty_note() {
    let audit = Arc::new(InMemoryAuditSink::new());
    let forwarder = ScriptedForwarder::new(Box::new(|_request| {
        Err(ForwardError::NoAcknowledgement {
            reason: "the acknowledgement window elapsed".to_string(),
        })
    }));
    let engine = default_engine(forwarder, Arc::clone(&audit) as Arc<dyn AuditSink>);
    let record = engine
        .forward_verified(verified("cmd-1", "ess-01", "simulated-test", 1))
        .expect("recorded");
    assert_eq!(record.lifecycle_state, "FALLBACK");
    // The P4/P5 honesty wording travels on the failure record: a missing
    // ack is NOT delivery, and the command never left ISSUED honestly.
    assert!(record.detail.contains(ACKNOWLEDGEMENT_IS_NOT_DELIVERY));
    let lines = details(&audit);
    assert!(lines
        .iter()
        .any(|line| line.starts_with("EDGE_COMMAND_LIFECYCLE")
            && line.contains("state=FAILED")
            && line.contains(ACKNOWLEDGEMENT_IS_NOT_DELIVERY)));
}

// --- 2. Tracking: on-target, deviation, completion ----------------------------------

#[test]
fn tracking_deviation_lands_in_failed_then_fallback() {
    let audit = Arc::new(InMemoryAuditSink::new());
    let engine = default_engine(always_ack(), Arc::clone(&audit) as Arc<dyn AuditSink>);

    // Forward: ISSUED → ACKNOWLEDGED (the ack is NOT delivery).
    let record = engine
        .forward_verified(verified("cmd-1", "ess-01", "simulated-test", 1))
        .expect("recorded");
    assert_eq!(record.lifecycle_state, "ACKNOWLEDGED");
    assert!(record.detail.contains(ACKNOWLEDGEMENT_IS_NOT_DELIVERY));

    // On-target observation: TRACKING.
    let record = engine
        .observe_tracking(
            "cmd-1",
            TrackingObservation::OnTarget {
                detail: "the adapter reports the setpoint tracking".to_string(),
            },
        )
        .expect("tracked");
    assert_eq!(record.lifecycle_state, "TRACKING");

    // Deviation: FAILED, then FALLBACK per the tracking_deviation trigger.
    let record = engine
        .observe_tracking(
            "cmd-1",
            TrackingObservation::Deviated {
                detail: "the response deviates beyond tolerance".to_string(),
            },
        )
        .expect("recorded");
    assert_eq!(record.lifecycle_state, "FALLBACK");
    assert_eq!(
        engine.lifecycle_state_of("cmd-1").as_deref(),
        Some("FALLBACK")
    );
    let lines = details(&audit);
    assert!(lines
        .iter()
        .any(|line| line.starts_with("EDGE_COMMAND_LIFECYCLE") && line.contains("state=TRACKING")));
    assert!(lines
        .iter()
        .any(|line| line.starts_with("EDGE_COMMAND_FALLBACK")
            && line.contains("trigger=tracking_deviation")));
}

#[test]
fn an_on_target_command_can_complete() {
    let audit = Arc::new(InMemoryAuditSink::new());
    let engine = default_engine(always_ack(), Arc::clone(&audit) as Arc<dyn AuditSink>);
    engine
        .forward_verified(verified("cmd-1", "ess-01", "simulated-test", 1))
        .expect("forwarded");
    engine
        .observe_tracking(
            "cmd-1",
            TrackingObservation::OnTarget {
                detail: "tracking".to_string(),
            },
        )
        .expect("tracked");
    let record = engine
        .observe_tracking(
            "cmd-1",
            TrackingObservation::Completed {
                detail: "the command objective completed".to_string(),
            },
        )
        .expect("completed");
    assert_eq!(record.lifecycle_state, "COMPLETED");
    assert_eq!(
        engine.lifecycle_state_of("cmd-1").as_deref(),
        Some("COMPLETED")
    );
    assert!(details(&audit).iter().any(|line| line
        .starts_with("EDGE_COMMAND_LIFECYCLE")
        && line.contains("state=COMPLETED")));
}

// --- 3. Expiry mid-flight: EXPIRED, FALLBACK only where configured -------------------

#[test]
fn expiry_mid_flight_lands_in_expired_and_fallback_only_where_configured() {
    // Asset ess-04: the expiry trigger IS configured → EXPIRED then FALLBACK.
    let audit = Arc::new(InMemoryAuditSink::new());
    // The advanceable test clock starts inside the validity window.
    let clock = Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000));
    let engine = engine_with(
        always_ack(),
        SiteCommandAllowlist::new(vec!["simulated-test".to_string()]),
        fallback_config(),
        Arc::clone(&audit) as Arc<dyn AuditSink>,
        clock.clone() as Arc<dyn Clock + Send + Sync>,
    );
    engine
        .forward_verified(verified("cmd-4", "ess-04", "simulated-test", 1))
        .expect("forwarded");

    // Before the validity bound: no premature expiry (fail-closed).
    let error = engine
        .expire_mid_flight("cmd-4")
        .expect_err("no premature expiry");
    assert!(matches!(error, ForwardingRefusal::NotYetExpired { .. }));

    // The clock advances past valid_until; the composition calls expiry.
    clock.advance_millis(VALID_UNTIL - (BASE_MILLIS + 1_000) + 1_000);
    let record = engine.expire_mid_flight("cmd-4").expect("expires");
    assert_eq!(record.lifecycle_state, "FALLBACK");
    let lines = details(&audit);
    assert!(lines
        .iter()
        .any(|line| line.starts_with("EDGE_COMMAND_LIFECYCLE") && line.contains("state=EXPIRED")));
    assert!(lines
        .iter()
        .any(|line| line.starts_with("EDGE_COMMAND_FALLBACK") && line.contains("trigger=expiry")));

    // Asset ess-05: the expiry trigger is NOT configured → EXPIRED, NO
    // FALLBACK transition (an operator decides).
    let audit_b = Arc::new(InMemoryAuditSink::new());
    let clock_b = Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000));
    let engine_b = engine_with(
        always_ack(),
        SiteCommandAllowlist::new(vec!["simulated-test".to_string()]),
        fallback_config(),
        Arc::clone(&audit_b) as Arc<dyn AuditSink>,
        clock_b.clone() as Arc<dyn Clock + Send + Sync>,
    );
    engine_b
        .forward_verified(verified("cmd-5", "ess-05", "simulated-test", 1))
        .expect("forwarded");
    clock_b.advance_millis(VALID_UNTIL - (BASE_MILLIS + 1_000) + 1_000);
    let record = engine_b.expire_mid_flight("cmd-5").expect("expires");
    assert_eq!(record.lifecycle_state, "EXPIRED");
    assert_eq!(
        engine_b.lifecycle_state_of("cmd-5").as_deref(),
        Some("EXPIRED"),
        "EXPIRED → FALLBACK fires ONLY where the policy configures it"
    );
    let lines = details(&audit_b);
    assert!(lines
        .iter()
        .any(|line| line.starts_with("EDGE_COMMAND_FALLBACK_TRIGGER_NOT_CONFIGURED")));
    assert!(!lines
        .iter()
        .any(|line| line.starts_with("EDGE_COMMAND_FALLBACK:")));
}

// --- 4. The allowlist: the forwarder is NEVER invoked for a stranger -----------------

#[test]
fn no_command_reaches_a_non_allowlisted_adapter() {
    let audit = Arc::new(InMemoryAuditSink::new());
    let forwarder = always_ack();
    // The forward-side allowlist deliberately EXCLUDES openems-rest.
    let engine = engine_with(
        forwarder.clone() as Arc<dyn CommandForwarder>,
        SiteCommandAllowlist::new(vec!["simulated-test".to_string()]),
        fallback_config(),
        Arc::clone(&audit) as Arc<dyn AuditSink>,
    );
    let error = engine
        .forward_verified(verified("cmd-1", "ess-01", "openems-rest", 1))
        .expect_err("a non-allowlisted adapter is refused");
    assert!(matches!(
        error,
        ForwardingRefusal::NotAllowlisted { ref adapter_id } if adapter_id == "openems-rest"
    ));
    // THE structural assertion: the forwarder port was NEVER invoked.
    assert_eq!(
        forwarder.call_count(),
        0,
        "the forwarder must never be invoked for a non-allowlisted adapter"
    );
    // The separate allowlist re-check has its OWN audit event, and the
    // refusal is audited with the allowlist code.
    let lines = details(&audit);
    assert!(lines
        .iter()
        .any(|line| line.starts_with("EDGE_COMMAND_FORWARD_ALLOWLIST_CHECKED")
            && line.contains("allowlisted=false")));
    assert!(lines
        .iter()
        .any(|line| line.starts_with("EDGE_COMMAND_FORWARD_REJECTED")
            && line.contains("code=allowlist")));
}

// --- 5. The fallback configuration: operator review REQUIRED (not signed) ------------

#[test]
fn a_missing_operator_review_field_refuses_the_config_load() {
    // Missing reviewed_by → REFUSED.
    let missing_by = "flexcore-edge-fallback-policy-v1\nasset = ess-01\nvariant = \
         RETURN_TO_LOCAL_EMS_AUTONOMOUS\ntriggers = nack\nreviewed_on = 2026-11-13\n";
    assert!(FallbackPolicyConfig::parse(missing_by).is_err());
    // Missing reviewed_on → REFUSED.
    let missing_on = "flexcore-edge-fallback-policy-v1\nasset = ess-01\nvariant = \
         RETURN_TO_LOCAL_EMS_AUTONOMOUS\ntriggers = nack\nreviewed_by = A. Operator\n";
    assert!(FallbackPolicyConfig::parse(missing_on).is_err());
    // Both missing → REFUSED.
    let missing_both = "flexcore-edge-fallback-policy-v1\nasset = ess-01\nvariant = \
         RETURN_TO_LOCAL_EMS_AUTONOMOUS\ntriggers = nack\n";
    assert!(FallbackPolicyConfig::parse(missing_both).is_err());
    // An EMPTY reviewed_by is as good as missing → REFUSED.
    let empty_by = "flexcore-edge-fallback-policy-v1\nasset = ess-01\nvariant = \
         RETURN_TO_LOCAL_EMS_AUTONOMOUS\ntriggers = nack\nreviewed_by =    \n\
         reviewed_on = 2026-11-13\n";
    assert!(FallbackPolicyConfig::parse(empty_by).is_err());

    // Other strict-parse refusals.
    let unknown_variant = "flexcore-edge-fallback-policy-v1\nasset = ess-01\nvariant = \
         SET_POWER_TO_ZERO\ntriggers = nack\nreviewed_by = A\nreviewed_on = 2026-11-13\n";
    assert!(FallbackPolicyConfig::parse(unknown_variant).is_err(),
        "the universal set-power-to-zero fallback is STRUCTURALLY ABSENT from the option set");
    let hold_without_interval = "flexcore-edge-fallback-policy-v1\nasset = ess-01\nvariant = \
         HOLD_LAST_ACCEPTED_SETPOINT\ntriggers = nack\nreviewed_by = A\nreviewed_on = 2026-11-13\n";
    assert!(FallbackPolicyConfig::parse(hold_without_interval).is_err(),
        "an unbounded hold is a load error (§14.4)");
    let duplicate_asset = "flexcore-edge-fallback-policy-v1\nasset = ess-01\nvariant = \
         RETURN_TO_LOCAL_EMS_AUTONOMOUS\ntriggers = nack\nreviewed_by = A\n\
         reviewed_on = 2026-11-13\nasset = ess-01\nvariant = RETURN_TO_LOCAL_EMS_AUTONOMOUS\n\
         triggers = nack\nreviewed_by = A\nreviewed_on = 2026-11-13\n";
    assert!(FallbackPolicyConfig::parse(duplicate_asset).is_err());
    let unknown_trigger = "flexcore-edge-fallback-policy-v1\nasset = ess-01\nvariant = \
         RETURN_TO_LOCAL_EMS_AUTONOMOUS\ntriggers = explode\nreviewed_by = A\n\
         reviewed_on = 2026-11-13\n";
    assert!(FallbackPolicyConfig::parse(unknown_trigger).is_err());
    let unknown_field = "flexcore-edge-fallback-policy-v1\nasset = ess-01\nextra = sneaky\n";
    assert!(FallbackPolicyConfig::parse(unknown_field).is_err());
    let wrong_version = "flexcore-edge-fallback-policy-v0\nasset = ess-01\n";
    assert!(FallbackPolicyConfig::parse(wrong_version).is_err());

    // The valid fixture loads and every entry carries its review.
    let config = fallback_config();
    assert_eq!(config.entries.len(), 5);
    for entry in &config.entries {
        assert!(!entry.review.reviewed_by.is_empty());
        assert!(!entry.review.reviewed_on.is_empty());
    }
    // The variant names are exactly the closed §14.4 option set.
    assert_eq!(
        FallbackVariant::HoldLastAcceptedSetpoint {
            bounded_interval_millis: 1
        }
        .name(),
        "HOLD_LAST_ACCEPTED_SETPOINT"
    );
    assert_eq!(
        FallbackVariant::ReturnToLocalEmsAutonomous.name(),
        "RETURN_TO_LOCAL_EMS_AUTONOMOUS"
    );
    assert_eq!(
        FallbackVariant::PreserveReserveSocConstraintsAndStopAcceptingNewCommands.name(),
        "PRESERVE_RESERVE_SOC_CONSTRAINTS_AND_STOP_ACCEPTING_NEW_COMMANDS"
    );
    assert_eq!(
        FallbackVariant::RequireOperatorIntervention.name(),
        "REQUIRE_OPERATOR_INTERVENTION"
    );
}

// --- 6. Per-asset policy: PRESERVE stops the asset; unconfigured → intervention ------

#[test]
fn preserve_reserve_soc_stops_accepting_new_commands_for_the_asset() {
    let audit = Arc::new(InMemoryAuditSink::new());
    let forwarder = always_nack("refused");
    let engine = default_engine(forwarder.clone(), Arc::clone(&audit) as Arc<dyn AuditSink>);

    // First command: NACK → FAILED → the PRESERVE fallback fires.
    let record = engine
        .forward_verified(verified("cmd-1", "ess-02", "simulated-test", 1))
        .expect("recorded");
    assert_eq!(record.lifecycle_state, "FALLBACK");
    assert_eq!(engine.stopped_assets(), vec!["ess-02".to_string()]);

    // A SECOND command for the same asset is refused BEFORE the forwarder:
    // no new commands are accepted until operator action.
    let error = engine
        .forward_verified(verified("cmd-2", "ess-02", "simulated-test", 2))
        .expect_err("the stop-half of the PRESERVE variant is enforced");
    assert!(matches!(error, ForwardingRefusal::FallbackStopActive { .. }));
    assert_eq!(
        forwarder.call_count(),
        1,
        "the stopped asset's second command never reached the forwarder"
    );
    assert!(details(&audit)
        .iter()
        .any(|line| line.starts_with("EDGE_COMMAND_FORWARD_REJECTED")
            && line.contains("code=fallback_stop")));

    // A DIFFERENT asset is unaffected (the policy is PER ASSET, never
    // fleet-wide).
    let record = engine
        .forward_verified(verified("cmd-3", "ess-01", "simulated-test", 1))
        .expect("ess-01 is not stopped");
    assert_eq!(record.lifecycle_state, "FALLBACK");
}

#[test]
fn an_unconfigured_asset_defaults_to_operator_intervention_without_a_fallback_transition() {
    let audit = Arc::new(InMemoryAuditSink::new());
    let engine = default_engine(always_nack("refused"), Arc::clone(&audit) as Arc<dyn AuditSink>);
    // ess-99 has NO fallback entry: the explicit DEFAULT is
    // REQUIRE_OPERATOR_INTERVENTION with NO automatic setpoint change and
    // NO FALLBACK transition (the P4.17 default-entry precedent).
    let record = engine
        .forward_verified(verified("cmd-1", "ess-99", "simulated-test", 1))
        .expect("recorded");
    assert_eq!(record.lifecycle_state, "FAILED", "no FALLBACK transition");
    assert_eq!(
        engine.lifecycle_state_of("cmd-1").as_deref(),
        Some("FAILED")
    );
    assert!(details(&audit)
        .iter()
        .any(|line| line.starts_with("EDGE_COMMAND_FALLBACK_INTERVENTION_REQUIRED")));
}

#[test]
fn the_fail_closed_production_stub_fails_every_forward_named() {
    let audit = Arc::new(InMemoryAuditSink::new());
    // PRODUCTION composition: the fail-closed stub — no adapter write path
    // is qualified, so every forward fails named and the per-asset
    // fallback applies.
    let engine = default_engine(
        Arc::new(FailClosedCommandForwarder),
        Arc::clone(&audit) as Arc<dyn AuditSink>,
    );
    let record = engine
        .forward_verified(verified("cmd-1", "ess-01", "simulated-test", 1))
        .expect("recorded");
    // Transport failure → FAILED (timeout-class) → the configured HOLD
    // fallback for ess-01 fires.
    assert_eq!(record.lifecycle_state, "FALLBACK");
    assert!(record.detail.contains("transport"));
    assert!(details(&audit)
        .iter()
        .any(|line| line.starts_with("EDGE_COMMAND_LIFECYCLE") && line.contains("state=FAILED")));
}

// --- 7. The full gated pipeline: gate → verifier → engine ----------------------------

#[test]
fn the_full_pipeline_flows_gate_then_verifier_then_engine_in_the_test_env() {
    let gate_audit = Arc::new(InMemoryAuditSink::new());
    let verify_audit = Arc::new(InMemoryAuditSink::new());
    let forward_audit = Arc::new(InMemoryAuditSink::new());
    let dir = scratch_dir("pipeline");

    // The P6.14 receiver (all four conditions; the UNCHANGED module).
    let key = EnablementKey::new(
        "lab-enablement-key-material".to_string(),
        "deployment:lab-key-1".to_string(),
    )
    .expect("valid gate key");
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
        d6_record_reference: Some(
            "ledger/phase-06-edge-protocol/03_d6_boundary_review.md".to_string(),
        ),
    };
    let runtime = CommandPathRuntime::evaluate(&inputs, &key, BASE_MILLIS);
    let receiver = CommandReceiver::new(
        runtime,
        Arc::new(permissive_policy()),
        Arc::clone(&gate_audit) as Arc<dyn AuditSink>,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000)),
    );

    // The P6.15 verifier.
    let signing_key = CommandSigningKey::new(
        "lab-command-signing-key-material".to_string(),
        "deployment:lab-command-key-1".to_string(),
    )
    .expect("valid signing key");
    let authorization = CommandAuthorization {
        site_id: "pilot-site-A".to_string(),
        valid_until_unix_millis: BASE_MILLIS + 86_400_000,
        adapter_allowlist: vec!["simulated-test".to_string()],
        asset_scopes: vec![AssetCommandScope {
            asset_key: "ess-01".to_string(),
            allowed_quantities: vec!["active_power".to_string()],
            min_value: -5.0,
            max_value: 5.0,
        }],
    };
    let checkpoint = FileSequenceCheckpoint::open(&dir.join("seq.ckpt")).expect("checkpoint opens");
    let verifier = CommandVerifier::new(
        signing_key.clone(),
        authorization,
        Box::new(checkpoint),
        Arc::clone(&verify_audit) as Arc<dyn AuditSink>,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000)),
    )
    .expect("verifier composes");

    // The P6.16 engine with a scripted ACK forwarder.
    let engine = default_engine(always_ack(), Arc::clone(&forward_audit) as Arc<dyn AuditSink>);

    let mut pipeline = GatedCommandPipeline::new(receiver, verifier, engine);

    // A valid, signed, in-window, in-scope command flows end to end.
    let edge = EdgeCommand::new(
        "cmd-1".to_string(),
        "ess-01".to_string(),
        "ISSUED".to_string(),
        1,
    )
    .expect("valid edge command");
    let unsigned = IncomingCommand::new(
        edge,
        "simulated-test".to_string(),
        String::new(),
        "edge-outbound-session-1".to_string(),
        "active_power".to_string(),
        1.5,
        BASE_MILLIS,
        VALID_UNTIL,
    )
    .expect("valid incoming");
    let signature = sign_command(&signing_key, &unsigned);
    let command = IncomingCommand::new(
        unsigned.edge_command,
        "simulated-test".to_string(),
        signature,
        "edge-outbound-session-1".to_string(),
        "active_power".to_string(),
        1.5,
        BASE_MILLIS,
        VALID_UNTIL,
    )
    .expect("valid incoming");
    let outcome = pipeline
        .receive_verify_and_forward(&command, flexcore_contracts::time_model::ClockQuality::Synchronized)
        .expect("the full chain admits the test-env command");
    assert_eq!(outcome.lifecycle_state, "ACKNOWLEDGED");
    assert!(outcome.detail.contains(ACKNOWLEDGEMENT_IS_NOT_DELIVERY));

    // All three stages audited their own events.
    assert!(details(&gate_audit)
        .iter()
        .any(|line| line.starts_with("EDGE_COMMAND_ACCEPTED")));
    assert!(details(&verify_audit)
        .iter()
        .any(|line| line.starts_with("EDGE_COMMAND_VERIFIED")));
    assert!(details(&forward_audit)
        .iter()
        .any(|line| line.starts_with("EDGE_COMMAND_LIFECYCLE")
            && line.contains("state=ACKNOWLEDGED")));

    // A FORGED command is stopped at the verification stage with its
    // distinct reason (and never forwarded).
    let forged_edge = EdgeCommand::new(
        "cmd-2".to_string(),
        "ess-01".to_string(),
        "ISSUED".to_string(),
        2,
    )
    .expect("valid edge command");
    let forged = IncomingCommand::new(
        forged_edge,
        "simulated-test".to_string(),
        "edge-sig:deployment:lab-command-key-1:cmd-2:fnv1a64:deadbeefdeadbeef".to_string(),
        "edge-outbound-session-1".to_string(),
        "active_power".to_string(),
        2.5,
        BASE_MILLIS,
        VALID_UNTIL,
    )
    .expect("valid incoming");
    let error = pipeline
        .receive_verify_and_forward(&forged, flexcore_contracts::time_model::ClockQuality::Synchronized)
        .expect_err("a forged command is refused");
    assert_eq!(error.stage(), "verification");
    assert_eq!(error.code(), "signature");
}
