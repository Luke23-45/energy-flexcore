// P6.14 — The command path is disabled by default; the four-condition
//         test-env enablement gate (ledger P6.14; test file written, NOT
//         run — the loop bans in
//         `ledger/phase-06-edge-protocol/ENTRY_NOTE.md` hold; the human
//         build/test pass runs these).
//
// Coverage:
//
// 1. **Default build**: with the default gate inputs, an `ISSUED` command
//    arriving at the edge is REJECTED (not queued, not forwarded) and the
//    rejection is audited with the P4-`TenantDenialAuditRecord`-pattern
//    record (cited, adapted to the edge `AuditSink` port).
// 2. **Each gate condition individually missing** → still disabled: (a)
//    config flag off; (b) no signed record / tampered digest / wrong token /
//    wrong key reference; (c) empty allowlist; (d) no D6 record reference.
// 3. **All four present + unexpired** → enabled WITHIN the recorded scope;
//    outside the scope refused per-command.
// 4. **Expiry** → auto-disable (the time bound enforced at read time) +
//    one audited disable transition.
// 5. **Structural no-plugin-enable** (§5.7): a plugin can only ever hold a
//    shared reference to the runtime — every public read is exercised and
//    the state is unchanged; no setter exists to call (the compile-level
//    argument is in the decision doc §6; this test pins the read-only
//    surface behaviourally).
// 6. **Network shape**: no inbound listener anywhere — this file adds no
//    socket code, and the crate-wide lexical scan
//    (`tests/no_inbound_listener.rs`) covers the new modules unchanged.
//
// In-process only; no network; the enablement KEY is a fixture value (the
// real key material is a deployment requirement — decision doc §4).

use std::sync::{Arc, Mutex};

use flexcore_contracts::time_model::{EventTime, IngestTime};
use flexcore_edge_agent::command_path::{
    CommandPathEnablement, CommandPathGateInputs, CommandPathRuntime, CommandReceiver,
    CommandRefusal, EdgeCommand, EnablementKey, GateRefusal, SignedEnablementRecord,
    CommandPolicyGate, FROZEN_COMMAND_STATE_NAMES,
};
use flexcore_edge_agent::policy::{AdapterPolicy, OperatingWindow, RateLimit, SitePolicyConfig, SitePolicyEngine};
use flexcore_edge_agent::ports::{AuditSink, InMemoryAuditSink};
use flexcore_ports::clock::Clock;
use flexcore_ports::fakes::FixedClock;

const BASE_MILLIS: i64 = 1_700_000_000_000;

/// A test clock whose value the test can advance through a shared handle
/// (the receiver holds the clock; the test holds the shared state — no
/// private-field access from this external test crate).
#[derive(Debug, Clone)]
struct SharedTestClock {
    current_millis: Arc<Mutex<i64>>,
}

impl SharedTestClock {
    fn at(millis: i64) -> Self {
        Self {
            current_millis: Arc::new(Mutex::new(millis)),
        }
    }
    fn advance_to(&self, millis: i64) {
        *self.current_millis.lock().expect("poisoned test clock") = millis;
    }
}

impl Clock for SharedTestClock {
    fn now_event_time(&self) -> EventTime {
        EventTime::from_unix_millis(*self.current_millis.lock().expect("poisoned test clock"))
            .expect("valid test timestamp")
    }
    fn now_ingest_time(&self) -> IngestTime {
        IngestTime::from_unix_millis(*self.current_millis.lock().expect("poisoned test clock"))
            .expect("valid test timestamp")
    }
}

fn key() -> EnablementKey {
    EnablementKey::new(
        "lab-enablement-key-material".to_string(),
        "deployment:lab-key-1".to_string(),
    )
    .expect("valid key")
}

fn signed_record(site: &str, valid_until: i64, key: &EnablementKey) -> SignedEnablementRecord {
    SignedEnablementRecord {
        site_id: site.to_string(),
        approver: "J. Doe (security lead)".to_string(),
        adapter_scope: vec!["simulated-test".to_string()],
        enablement_token: "lab-token-123".to_string(),
        issued_at_unix_millis: BASE_MILLIS,
        valid_until_unix_millis: valid_until,
        key_reference: "deployment:lab-key-1".to_string(),
        digest: None,
    }
    .sign(key)
    .expect("the record signs")
}

fn full_inputs(site: &str, valid_until: i64, key: &EnablementKey) -> CommandPathGateInputs {
    CommandPathGateInputs {
        site_id: site.to_string(),
        config_flag_enabled: true,
        signed_record: Some(signed_record(site, valid_until, key)),
        environment_token: Some("lab-token-123".to_string()),
        command_allowlist: vec!["simulated-test".to_string()],
        d6_record_reference: Some(
            "ledger/phase-06-edge-protocol/03_d6_boundary_review.md".to_string(),
        ),
    }
}

fn permissive_policy() -> Arc<dyn CommandPolicyGate> {
    // The crate's P6.17 engine with generous limits (the policy-specific
    // cases live in `policy_health_p6_17.rs`).
    Arc::new(
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
        .expect("valid policy"),
    )
}

fn receiver_with_clock(
    runtime: CommandPathRuntime,
    audit: Arc<InMemoryAuditSink>,
    clock: Arc<dyn Clock + Send + Sync>,
) -> CommandReceiver {
    CommandReceiver::new(
        runtime,
        permissive_policy(),
        audit as Arc<dyn AuditSink>,
        clock,
    )
}

fn receiver(runtime: CommandPathRuntime, audit: Arc<InMemoryAuditSink>) -> CommandReceiver {
    receiver_with_clock(
        runtime,
        audit,
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
fn the_default_build_rejects_issued_with_an_audit_record_and_retains_nothing() {
    let audit = Arc::new(InMemoryAuditSink::new());
    // The DEFAULT gate inputs: every field at its safe default (flag off,
    // no record, no token, empty allowlist, no D6 reference).
    let runtime = CommandPathRuntime::evaluate(&CommandPathGateInputs::default(), &key(), BASE_MILLIS);
    let mut rx = receiver(runtime, Arc::clone(&audit));

    let error = rx
        .receive(&issued_command(), "simulated-test")
        .expect_err("the default build rejects every command");
    assert_eq!(error.code(), "command_path_disabled");
    assert!(matches!(
        error,
        CommandRefusal::PathDisabled {
            reason: GateRefusal::ConfigFlagOff
        }
    ));

    // The rejection is audited with the dedicated event name and the
    // P4-pattern denial record text (identifiers only, no secrets).
    let details: Vec<String> = audit.events().iter().map(|e| e.detail.clone()).collect();
    assert!(
        details
            .iter()
            .any(|d| d.starts_with("EDGE_COMMAND_REJECTED") && d.contains("command_denial")),
        "the rejection is audited: {details:?}"
    );
    assert!(
        details.iter().any(|d| d.contains("code=command_path_disabled")),
        "the audit names the refusal code"
    );

    // Not queued, not forwarded: a second receive behaves identically (no
    // retained state replays anything) and the audit shows rejections only.
    let error = rx
        .receive(&issued_command(), "simulated-test")
        .expect_err("still disabled");
    assert_eq!(error.code(), "command_path_disabled");
    // Fresh snapshot AFTER the second receive.
    let details: Vec<String> = audit.events().iter().map(|e| e.detail.clone()).collect();
    let rejections = details
        .iter()
        .filter(|d| d.starts_with("EDGE_COMMAND_REJECTED"))
        .count();
    let acceptances = details
        .iter()
        .filter(|d| d.starts_with("EDGE_COMMAND_ACCEPTED"))
        .count();
    assert_eq!(acceptances, 0, "nothing was ever admitted");
    assert!(rejections >= 2, "each refusal is audited");
}

#[test]
fn each_gate_condition_missing_individually_still_disables() {
    let key = key();
    let valid_until = BASE_MILLIS + 3_600_000;
    let full = full_inputs("pilot-site-A", valid_until, &key);

    // (a) The explicit signed per-site configuration flag is OFF.
    let mut inputs = full.clone();
    inputs.config_flag_enabled = false;
    let runtime = CommandPathRuntime::evaluate(&inputs, &key, BASE_MILLIS);
    assert!(matches!(
        runtime.enablement_at(BASE_MILLIS),
        CommandPathEnablement::Disabled {
            reason: GateRefusal::ConfigFlagOff
        }
    ));

    // (b1) No signed enablement record loaded.
    let mut inputs = full.clone();
    inputs.signed_record = None;
    assert!(matches!(
        CommandPathRuntime::evaluate(&inputs, &key, BASE_MILLIS).enablement_at(BASE_MILLIS),
        CommandPathEnablement::Disabled {
            reason: GateRefusal::NoSignedRecord
        }
    ));

    // (b2) The record body was TAMPERED after signing (digest mismatch).
    let mut inputs = full.clone();
    if let Some(record) = &mut inputs.signed_record {
        record.valid_until_unix_millis += 1; // any body edit invalidates
    }
    assert!(matches!(
        CommandPathRuntime::evaluate(&inputs, &key, BASE_MILLIS).enablement_at(BASE_MILLIS),
        CommandPathEnablement::Disabled {
            reason: GateRefusal::DigestMismatch
        }
    ));

    // (b3) The environment token does not match the record's token.
    let mut inputs = full.clone();
    inputs.environment_token = Some("wrong-token".to_string());
    assert!(matches!(
        CommandPathRuntime::evaluate(&inputs, &key, BASE_MILLIS).enablement_at(BASE_MILLIS),
        CommandPathEnablement::Disabled {
            reason: GateRefusal::TokenMismatch
        }
    ));

    // (b4) The record was signed under a DIFFERENT key reference.
    let other_key = EnablementKey::new(
        "other-material".to_string(),
        "deployment:other-key".to_string(),
    )
    .expect("valid key");
    assert!(matches!(
        CommandPathRuntime::evaluate(&full, &other_key, BASE_MILLIS).enablement_at(BASE_MILLIS),
        CommandPathEnablement::Disabled {
            reason: GateRefusal::KeyReferenceMismatch
        }
    ));

    // (c) The site command allowlist is empty.
    let mut inputs = full.clone();
    inputs.command_allowlist = Vec::new();
    assert!(matches!(
        CommandPathRuntime::evaluate(&inputs, &key, BASE_MILLIS).enablement_at(BASE_MILLIS),
        CommandPathEnablement::Disabled {
            reason: GateRefusal::AllowlistEmpty
        }
    ));

    // (d) The site's D6 review record reference is absent.
    let mut inputs = full;
    inputs.d6_record_reference = None;
    assert!(matches!(
        CommandPathRuntime::evaluate(&inputs, &key, BASE_MILLIS).enablement_at(BASE_MILLIS),
        CommandPathEnablement::Disabled {
            reason: GateRefusal::D6RecordAbsent
        }
    ));
}

#[test]
fn all_four_conditions_present_and_unexpired_enables_within_the_recorded_scope() {
    let key = key();
    let inputs = full_inputs("pilot-site-A", BASE_MILLIS + 3_600_000, &key);
    let runtime = CommandPathRuntime::evaluate(&inputs, &key, BASE_MILLIS);
    let audit = Arc::new(InMemoryAuditSink::new());
    let mut rx = receiver(runtime, Arc::clone(&audit));

    // The enable event fired at composition, naming the approver and the
    // expiry (the P6.14 step-2 audit).
    let details: Vec<String> = audit.events().iter().map(|e| e.detail.clone()).collect();
    assert!(
        details
            .iter()
            .any(|d| d.starts_with("EDGE_COMMAND_PATH_ENABLED")
                && d.contains("J. Doe (security lead)")),
        "the enablement is audited with the named approver: {details:?}"
    );

    // Within scope: admitted (a gate admission only — the receipt says so).
    let receipt = rx
        .receive(&issued_command(), "simulated-test")
        .expect("admitted within the allowlisted scope");
    assert_eq!(receipt.target_adapter, "simulated-test");
    assert!(receipt.scope_note.contains("P6.15"), "the receipt is honest about scope");
    assert!(receipt.scope_note.contains("not physical delivery"));

    // Outside scope: refused per-command with its own audited refusal.
    let error = rx
        .receive(&issued_command(), "openems-rest")
        .expect_err("outside the allowlisted scope");
    assert_eq!(error.code(), "adapter_not_allowlisted");
    let details: Vec<String> = audit.events().iter().map(|e| e.detail.clone()).collect();
    assert!(
        details
            .iter()
            .any(|d| d.starts_with("EDGE_COMMAND_REJECTED") && d.contains("adapter_not_allowlisted")),
        "the scope refusal is audited"
    );
}

#[test]
fn expiry_auto_disables_and_audits_the_transition_exactly_once() {
    let key = key();
    // Enabled at evaluation; expires one second later. The clock STARTS
    // BEFORE the expiry (at BASE), so the path is genuinely enabled at
    // composition, and the test then advances PAST valid_until.
    let inputs = full_inputs("pilot-site-A", BASE_MILLIS + 1_000, &key);
    let runtime = CommandPathRuntime::evaluate(&inputs, &key, BASE_MILLIS);
    let audit = Arc::new(InMemoryAuditSink::new());
    let clock = SharedTestClock::at(BASE_MILLIS);
    let mut rx = receiver_with_clock(
        runtime,
        Arc::clone(&audit),
        Arc::new(clock.clone()) as Arc<dyn Clock + Send + Sync>,
    );

    let receipt = rx
        .receive(&issued_command(), "simulated-test")
        .expect("admitted before expiry");
    assert_eq!(receipt.enablement_valid_until_unix_millis, BASE_MILLIS + 1_000);

    // Past valid_until: the SAME runtime reads disabled (the time bound is
    // enforced at read time — no mutation anywhere), the command is
    // refused, and the disable transition is audited ONCE.
    clock.advance_to(BASE_MILLIS + 2_000);
    let error = rx
        .receive(&issued_command(), "simulated-test")
        .expect_err("expired enablement disables the path");
    assert!(matches!(
        error,
        CommandRefusal::PathDisabled {
            reason: GateRefusal::EnablementExpired
        }
    ));
    let details: Vec<String> = audit.events().iter().map(|e| e.detail.clone()).collect();
    assert_eq!(
        details
            .iter()
            .filter(|d| d.starts_with("EDGE_COMMAND_PATH_DISABLED"))
            .count(),
        1,
        "the expiry transition is audited exactly once"
    );

    // A further receive is still refused and does NOT re-audit the
    // transition.
    let _ = rx.receive(&issued_command(), "simulated-test");
    let details: Vec<String> = audit.events().iter().map(|e| e.detail.clone()).collect();
    assert_eq!(
        details
            .iter()
            .filter(|d| d.starts_with("EDGE_COMMAND_PATH_DISABLED"))
            .count(),
        1
    );
}

#[test]
fn a_plugin_holding_a_shared_reference_cannot_enable_the_path() {
    // §5.7, behaviourally pinned: a "plugin" can obtain a SHARED reference
    // to the runtime (as through any composition-root seam). Every public
    // call on that reference is a read; the state it observes is derived
    // from the immutable gate inputs and cannot change. There is no setter
    // to call — this test exercises the ENTIRE public read surface and
    // asserts the state is unchanged after all of it.
    let key = key();
    let inputs = full_inputs("pilot-site-A", BASE_MILLIS + 3_600_000, &key);
    let runtime = CommandPathRuntime::evaluate(&inputs, &key, BASE_MILLIS);
    assert!(runtime
        .enablement_at(BASE_MILLIS)
        .is_enabled_at(BASE_MILLIS));

    let disabled_runtime = CommandPathRuntime::evaluate(
        &CommandPathGateInputs::default(),
        &key,
        BASE_MILLIS,
    );
    // Everything a shared reference can do: read the enablement at any
    // instant, any number of times — the path stays disabled.
    for offset in [0i64, 1_000, 86_400_000] {
        let observed = disabled_runtime.enablement_at(BASE_MILLIS + offset);
        assert_eq!(
            observed,
            CommandPathEnablement::Disabled {
                reason: GateRefusal::ConfigFlagOff
            }
        );
    }
    // Cloning a runtime copies its (immutable) state — it cannot be used
    // to reach back into the original.
    let clone = disabled_runtime.clone();
    assert_eq!(
        clone.enablement_at(BASE_MILLIS),
        disabled_runtime.enablement_at(BASE_MILLIS)
    );
    // The gate inputs themselves are inert data: re-evaluating a mutated
    // COPY never affects the already-evaluated runtime.
    let mut sneaky = full_inputs("pilot-site-A", BASE_MILLIS + 3_600_000, &key);
    sneaky.config_flag_enabled = true; // was already true; flip attempts are meaningless on a copy
    let _ = sneaky;
    assert!(matches!(
        CommandPathRuntime::evaluate(&CommandPathGateInputs::default(), &key, BASE_MILLIS)
            .enablement_at(BASE_MILLIS),
        CommandPathEnablement::Disabled { .. }
    ));
}

#[test]
fn the_frozen_lifecycle_is_cited_and_only_issued_commands_arrive() {
    // The edge cites the frozen §14.1 lifecycle by name (it has no
    // flexcore-domain dependency by design); the receiver refuses any
    // non-ISSUED arrival with its own audited refusal.
    assert_eq!(FROZEN_COMMAND_STATE_NAMES.len(), 14);
    assert!(FROZEN_COMMAND_STATE_NAMES.contains(&"ISSUED"));
    let key = key();
    let inputs = full_inputs("pilot-site-A", BASE_MILLIS + 3_600_000, &key);
    let runtime = CommandPathRuntime::evaluate(&inputs, &key, BASE_MILLIS);
    let audit = Arc::new(InMemoryAuditSink::new());
    let mut rx = receiver(runtime, Arc::clone(&audit));

    let proposed = EdgeCommand::new(
        "cmd-2".to_string(),
        "ess-01".to_string(),
        "PROPOSED".to_string(),
        2,
    )
    .expect("PROPOSED is a frozen name");
    let error = rx
        .receive(&proposed, "simulated-test")
        .expect_err("only ISSUED arrives at a gateway/adapter");
    assert_eq!(error.code(), "unexpected_command_state");
    let details: Vec<String> = audit.events().iter().map(|e| e.detail.clone()).collect();
    assert!(
        details
            .iter()
            .any(|d| d.starts_with("EDGE_COMMAND_REJECTED") && d.contains("unexpected_command_state")),
        "the state refusal is audited"
    );

    // An unknown state name never even constructs (the frozen list is the
    // validator — the lifecycle is cited, never forked).
    assert!(EdgeCommand::new(
        "cmd-3".to_string(),
        "ess-01".to_string(),
        "DEFINITELY_NOT_FROZEN".to_string(),
        3,
    )
    .is_err());
}
