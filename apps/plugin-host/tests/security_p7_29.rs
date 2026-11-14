// P7.29 — Security testing at the SDK boundary: escalation and containment
//         attacks on the host enforcement surface (report §19.2(7), §17.4,
//         §9.3; ledger `ledger_phase_7.md` §P7.29).
//         WRITTEN-UNRUN: the loop bans hold (no cargo command of any kind);
//         the first human `cargo test -p flexcore-plugin-host` run is the
//         verification gate. No pass is claimed anywhere.
//
// WHAT THIS FILE IS (and is not): the §19.2(7) attack layer EXTENDING — not
// duplicating — P7.12/P7.13 (unit-level enforcement and sandbox negatives,
// `tests/enforcement_p7_12.rs`, `tests/sandbox_p7_13.rs`) and P7.23 (the
// end-to-end layered untrusted-plugin proof, `tests/untrusted_plugin_p7_23.rs`).
// Each case below is an ATTACK SCENARIO COMPOSITION (attempt → refusal →
// audit → containment), not a re-statement of a unit refusal another suite
// already owns; the reuse-by-citation map in the dated report
// (`ledger/phase-07-plugin-sdk/29_security_test_report.md`) allocates every
// §19.2(7) category to exactly one owning suite.
//
// CASE → REQUIREMENT MAP (the P6.19 pattern):
//
// | # | Case | Requirement |
// |---|---|---|
// | 1 | an_expanded_capability_set_is_a_new_verdict_and_never_a_mutation_of_the_issued_one | §19.2(7) privilege escalation; §5.7 frozen-capability rule (P7.12 §4) at attack-chain level |
// | 2 | detected_escalation_is_contained_by_revoking_the_whole_frozen_set | §17.4 (attempt → containment); §19.2(7) "the operator must be able to contain and remove it" |
// | 3 | a_downgraded_manifest_identity_is_recorded_verbatim_and_an_unnamed_one_is_refused | §19.2(7) supply chain (manifest substitution traceability at the host seam); §9.3 |
// | 4 | the_escalation_attempt_leaves_a_complete_ordered_audited_trail | §5.1 audit integrity; §17.4 (every step of the attempt is auditable) |
//
// SPLIT DISCIPLINE (honest): this crate depends on no flexcore crate
// (§8.2 dependency rule, enforced by `tools/check_dependency_rule.py`), so
// the KERNEL-side attack chains of P7.29 — the P7.14 provenance-gate tamper
// and substitution chain, the profile version-constraint downgrade, the
// unprovenanced-plugin refusal, the lifecycle restart/drain-deadline abuse,
// and the forged-profile strict parse refusals — live in the companion file
// `crates/flexcore-kernel/tests/security_p7_29_kernel.rs`. This file holds
// only what the HOST API can prove. The host-compromise posture (compromised
// HOST vs compromised COMPONENT) is RECORDED, not tested: no in-repo seam can
// show it — it is the P7.34 trust-boundary wave's input (see the report §5).
//
// CLAIM BOUNDARY: these tests prove the HOST API refuses and audits every
// attempted escalation. They are NOT a security certification and claim none
// (AGENTS.md §3 rule 2). Real-WASM residuals (a live hostile component, the
// wasmtime CVE surface) stay the recorded human-gate items of P7.13 §7 and
// the R7-C2 CVE floor re-check (`12_13_enforcement_negatives.md` §9).

use std::sync::Arc;

use flexcore_plugin_host::bindings::PluginBinding;
use flexcore_plugin_host::broker::{
    BrokerAuditEvent, BrokerAuditKind, BrokerAuditSink, BrokerError, CapabilityBroker,
    CapabilityScope, InMemoryBrokerAuditSink,
};
use flexcore_plugin_host::enforcement::{
    instantiate, EnforcementAuditEvent, EnforcementAuditKind, EnforcementError, EnforcementGate,
    EnforcementRequest, InMemoryEnforcementAuditSink, InstantiationInput, ProfileCeiling,
    ProfileGrant,
};
use flexcore_plugin_host::host::{ComponentArtifact, WorldId};
use flexcore_plugin_host::limits::DeclaredBounds;

// --- Shared fixtures -------------------------------------------------------------

const NOW: u64 = 1_700_000_000_000;

/// The plugin under attack (a Class B component's identity). Escalation here
/// means: a plugin that was granted a legitimate narrow set tries to end up
/// running with a wider one.
const ESCALATING_PLUGIN: &str = "evil.escalating-component";
const ESCALATING_MANIFEST: &str = "evil.escalating-component";
/// The version the attacker's substituted manifest claims (downgrade shape;
/// the host records identity verbatim — see case 3).
const DOWNGRADED_VERSION: &str = "1.9.9";
const HONEST_VERSION: &str = "2.0.0";

/// A second broker's audit sink, shared with the test (clones share the log).
struct SharedBrokerSink(Arc<InMemoryBrokerAuditSink>);

impl BrokerAuditSink for SharedBrokerSink {
    fn record(&self, event: BrokerAuditEvent) -> Result<(), BrokerError> {
        self.0.record(event)
    }
}

fn bounds() -> DeclaredBounds {
    DeclaredBounds {
        max_memory_mb: 256,
        timeout_ms: 5_000,
        max_output_mb: None,
        max_calls_per_minute: None,
    }
}

fn declared_fields() -> Vec<String> {
    vec!["max-memory-mb".to_string(), "timeout-ms".to_string()]
}

fn escalation_request(permissions: &[&str], manifest_version: &str) -> EnforcementRequest {
    EnforcementRequest {
        plugin_id: ESCALATING_PLUGIN.to_string(),
        manifest_id: ESCALATING_MANIFEST.to_string(),
        manifest_version: manifest_version.to_string(),
        world: WorldId::OptimizerPlugin,
        entry_symbol: "evil:component/entry@2.0.0".to_string(),
        manifest_permissions: permissions.iter().map(|p| (*p).to_string()).collect(),
        declared_resource_fields: declared_fields(),
        bounds: bounds(),
        fuel_budget: 10_000_000,
    }
}

fn ceiling_entry(capability: &str, scope: CapabilityScope) -> ProfileGrant {
    ProfileGrant { capability: capability.to_string(), scope, expires_at_unix_ms: None }
}

fn data_scope(tenant: &str) -> CapabilityScope {
    CapabilityScope::DataScope { tenant_id: tenant.to_string(), site_id: None, asset_id: None }
}

fn send_target(target: &str) -> CapabilityScope {
    CapabilityScope::SendOnlyTarget { target: target.to_string() }
}

/// An enforcement gate composed over a broker whose audit sink is shared with
/// the test (so broker-side denials are assertable), plus the door's own sink.
fn composed_gate() -> (
    EnforcementGate,
    Arc<InMemoryEnforcementAuditSink>,
    Arc<InMemoryBrokerAuditSink>,
) {
    let inner = Arc::new(InMemoryBrokerAuditSink::new());
    let broker = CapabilityBroker::new(Box::new(SharedBrokerSink(Arc::clone(&inner))));
    let gate_sink = Arc::new(InMemoryEnforcementAuditSink::new());
    let gate = EnforcementGate::compose(broker, Box::new(Arc::clone(&gate_sink)));
    (gate, gate_sink, inner)
}

/// The three-ceiling escalation target: everything the widest legal Class B
/// manifest may declare.
fn widened_ceiling() -> ProfileCeiling {
    ProfileCeiling::new(vec![
        ceiling_entry("telemetry.read", data_scope("tenant-a")),
        ceiling_entry("asset.read", data_scope("tenant-a")),
        ceiling_entry("forecast.publish", send_target("forecast-ingest")),
    ])
}

// --- #1: capability-set expansion after instantiation -----------------------------

#[test]
fn an_expanded_capability_set_is_a_new_verdict_and_never_a_mutation_of_the_issued_one() {
    let (gate, gate_sink, _broker_sink) = composed_gate();

    // The honest instantiation: one narrow capability, scoped to one tenant.
    let honest = escalation_request(&["telemetry.read"], HONEST_VERSION);
    let narrow_ceiling =
        ProfileCeiling::new(vec![ceiling_entry("telemetry.read", data_scope("tenant-a"))]);
    let verdict = gate
        .evaluate(&InstantiationInput { request: &honest, ceiling: &narrow_ceiling }, NOW)
        .expect("the honest instantiation is allowed");
    let frozen = verdict.clone();

    // THE ESCALATION ATTEMPT: the same plugin re-presented with a mutated
    // manifest (three capabilities) against a widened ceiling. Re-evaluation
    // is a NEW decision — it must never reach back into the issued verdict.
    let widened_request = escalation_request(
        &["telemetry.read", "asset.read", "forecast.publish"],
        HONEST_VERSION,
    );
    let expanded = gate
        .evaluate(&InstantiationInput { request: &widened_request, ceiling: &widened_ceiling() }, NOW + 1)
        .expect("the widened request is a separate decision (the door cannot know it is an attack)");
    assert_eq!(
        expanded.granted_capabilities(),
        vec![
            "telemetry.read".to_string(),
            "asset.read".to_string(),
            "forecast.publish".to_string()
        ]
    );

    // THE FROZEN-VERDICT RESTATEMENT: the already-issued verdict is untouched
    // by the expansion — the running instance's authority record did not move.
    assert_eq!(frozen, verdict, "the issued verdict is immutable");
    assert_eq!(frozen.granted_capabilities(), vec!["telemetry.read".to_string()]);
    assert_eq!(frozen.granted_scope("telemetry.read"), Some(data_scope("tenant-a")));
    assert_ne!(frozen, expanded, "a new capability set is a new verdict, never a mutation");

    // Both decisions are audited, and each freeze record names the ONLY path
    // to a different capability set: re-instantiation via drain-before-replace.
    let events = gate_sink.events();
    assert_eq!(
        events.iter().filter(|e| e.kind == EnforcementAuditKind::InstantiationAllowed).count(),
        2,
        "both evaluations are audited allows"
    );
    let freezes: Vec<&EnforcementAuditEvent> = events
        .iter()
        .filter(|e| e.kind == EnforcementAuditKind::CapabilitySetFrozen)
        .collect();
    assert_eq!(freezes.len(), 2, "both freezes are audited");
    assert!(
        freezes.iter().all(|e| e.detail.contains("drain-before-replace")),
        "every freeze record names the only re-instantiation path"
    );
}

// --- #2: containment of a detected escalation --------------------------------------

#[test]
fn detected_escalation_is_contained_by_revoking_the_whole_frozen_set() {
    let (gate, _gate_sink, broker_sink) = composed_gate();

    // The plugin legitimately holds the widest legal set (three capabilities).
    let request = escalation_request(
        &["telemetry.read", "asset.read", "forecast.publish"],
        HONEST_VERSION,
    );
    let verdict = gate
        .evaluate(&InstantiationInput { request: &request, ceiling: &widened_ceiling() }, NOW)
        .expect("the wide-but-legal instantiation is allowed");
    assert_eq!(verdict.grants().len(), 3);

    // An expansion is DETECTED (case 1's audit trail is how). Containment:
    // the operator revokes EVERY grant the frozen verdict carries — the
    // authority is dropped, per grant, in one pass.
    let grant_ids: Vec<String> =
        verdict.grants().iter().map(|g| g.grant_id().to_string()).collect();
    assert_eq!(grant_ids.len(), 3);
    for grant_id in &grant_ids {
        gate.broker().revoke(grant_id, NOW + 1).expect("each revocation succeeds");
    }

    // Every formerly granted capability now denies at the per-call check —
    // there is no residual authority in any scope.
    for (capability, scope) in [
        ("telemetry.read", data_scope("tenant-a")),
        ("asset.read", data_scope("tenant-a")),
        ("forecast.publish", send_target("forecast-ingest")),
    ] {
        let err = gate
            .broker()
            .check(ESCALATING_PLUGIN, capability, &scope, NOW + 2)
            .expect_err("revoked authority is gone in every scope");
        assert_eq!(err.name(), "CAPABILITY_NOT_GRANTED", "{capability}");
    }

    // Each denial is individually audited (§17.4: attempt → denial → audit).
    let denials: Vec<&BrokerAuditEvent> = broker_sink
        .events()
        .iter()
        .filter(|e| e.kind == BrokerAuditKind::CapabilityDenied)
        .collect();
    assert_eq!(denials.len(), 3, "each post-revocation use attempt is audited");
    assert!(denials.iter().all(|e| e.plugin_id == ESCALATING_PLUGIN));

    // The frozen record still stands AFTER containment: it is the
    // instantiation-time evidence of what the instance HELD (audit trail),
    // never a live authority.
    assert_eq!(verdict.granted_capabilities().len(), 3);
}

// --- #3: manifest substitution traceability at the host seam -----------------------

#[test]
fn a_downgraded_manifest_identity_is_recorded_verbatim_and_an_unnamed_one_is_refused() {
    let (gate, _gate_sink, _broker_sink) = composed_gate();

    // SUPPLY-CHAIN SUBSTITUTION (host-side half): the artifact/manifest pair
    // presented to the door claims a LOWER version than the one that was
    // verified. The KERNEL refuses the downgrade itself (the profile
    // version-constraint + the P7.14 provenance manifest binding — the
    // companion kernel file's case; cited, not re-tested here: this crate
    // depends on no flexcore crate). What the HOST seam guarantees is
    // TRACEABILITY: whatever identity reaches the door is recorded verbatim
    // on the verdict and on every grant it derives — a substitution can
    // never blur the derivation trail.
    let downgraded = escalation_request(&["telemetry.read"], DOWNGRADED_VERSION);
    let ceiling = ProfileCeiling::new(vec![ceiling_entry("telemetry.read", data_scope("tenant-a"))]);
    let verdict = gate
        .evaluate(&InstantiationInput { request: &downgraded, ceiling: &ceiling }, NOW)
        .expect("the door evaluates the identity it is handed (kernel-side gates refuse the downgrade)");
    assert_eq!(verdict.manifest_version(), DOWNGRADED_VERSION, "the trail is verbatim, never normalized");
    assert_eq!(verdict.manifest_id(), ESCALATING_MANIFEST);
    for grant in verdict.grants() {
        assert_eq!(grant.manifest_id(), ESCALATING_MANIFEST);
        assert_eq!(grant.manifest_version(), DOWNGRADED_VERSION);
    }

    // And an UNNAMED identity is refused outright: the door does not
    // instantiate unnamed things (an attacker cannot hide the trail by
    // omitting it).
    let mut unnamed = escalation_request(&["telemetry.read"], HONEST_VERSION);
    unnamed.manifest_version = String::new();
    let err = gate
        .evaluate(&InstantiationInput { request: &unnamed, ceiling: &ceiling }, NOW)
        .expect_err("an empty manifest version is refused");
    assert_eq!(err.name(), "INSTANTIATION_REQUEST_INVALID");

    let mut unnamed_id = escalation_request(&["telemetry.read"], HONEST_VERSION);
    unnamed_id.manifest_id = String::new();
    let err = gate
        .evaluate(&InstantiationInput { request: &unnamed_id, ceiling: &ceiling }, NOW)
        .expect_err("an empty manifest id is refused");
    assert_eq!(err.name(), "INSTANTIATION_REQUEST_INVALID");
}

// --- #4: the whole escalation attempt is one ordered, auditable trail ---------------

#[test]
fn the_escalation_attempt_leaves_a_complete_ordered_audited_trail() {
    let (gate, gate_sink, broker_sink) = composed_gate();

    // Step 1 (NOW): the forged escalation — a ceiling entry for a
    // never-granted capability — is refused at the door, audited.
    let forged_ceiling = ProfileCeiling::new(vec![
        ceiling_entry("telemetry.read", data_scope("tenant-a")),
        ceiling_entry("dispatch.authorize", data_scope("tenant-a")),
    ]);
    let request = escalation_request(&["telemetry.read"], HONEST_VERSION);
    let err = gate
        .evaluate(&InstantiationInput { request: &request, ceiling: &forged_ceiling }, NOW)
        .expect_err("a ceiling can never grant a never-granted id");
    assert_eq!(err.name(), "PERMISSION_NEVER_GRANTABLE");

    // Step 2 (NOW+1): the honest narrow instantiation is allowed and frozen.
    let narrow_ceiling =
        ProfileCeiling::new(vec![ceiling_entry("telemetry.read", data_scope("tenant-a"))]);
    let verdict = gate
        .evaluate(&InstantiationInput { request: &request, ceiling: &narrow_ceiling }, NOW + 1)
        .expect("the honest path still works after the refused attempt");

    // Step 3 (NOW+2): containment rehearsal — the operator revokes; the next
    // use denies (broker-side audit stream).
    let grant_id = verdict.grants()[0].grant_id().to_string();
    gate.broker().revoke(&grant_id, NOW + 2).expect("revoke");
    let err = gate
        .broker()
        .check(ESCALATING_PLUGIN, "telemetry.read", &data_scope("tenant-a"), NOW + 3)
        .expect_err("revoked authority denies");
    assert_eq!(err.name(), "CAPABILITY_NOT_GRANTED");

    // THE TRAIL: door-side records exist in decision order — refused
    // (attempt) < allowed < frozen — with the injected timestamps, so an
    // auditor can reconstruct the whole attempt from the records alone.
    let events = gate_sink.events();
    let refused_at = events
        .iter()
        .find(|e| e.kind == EnforcementAuditKind::InstantiationRefused)
        .expect("the forged attempt is audited").recorded_at_unix_ms;
    let allowed_at = events
        .iter()
        .find(|e| e.kind == EnforcementAuditKind::InstantiationAllowed)
        .expect("the honest allow is audited").recorded_at_unix_ms;
    let frozen_at = events
        .iter()
        .find(|e| e.kind == EnforcementAuditKind::CapabilitySetFrozen)
        .expect("the freeze is audited").recorded_at_unix_ms;
    assert!(refused_at < allowed_at, "the refusal precedes the allow");
    assert!(allowed_at <= frozen_at, "the freeze follows its allow");

    // Broker-side: the post-revocation denial is on the broker stream with
    // the reason class in its detail.
    let denial = broker_sink
        .events()
        .iter()
        .find(|e| e.kind == BrokerAuditKind::CapabilityDenied)
        .expect("the post-revocation denial is audited broker-side");
    assert_eq!(denial.plugin_id, ESCALATING_PLUGIN);
    assert!(denial.detail.contains("CAPABILITY_NOT_GRANTED"));
}

// --- The last line, restated for the supply-chain scenario --------------------------
//
// The P7.23 suite owns the binding-seam case end-to-end (case 12 there). The
// one P7.29-specific restatement is recorded, not re-tested, to avoid drift:
// even a fully-allowed verdict whose artifact was SUBSTITUTED after
// verification cannot execute in this build — `enforcement::instantiate`
// hands the artifact to the fail-closed `NoBindings` seam and the artifact
// bytes are never parsed (`BINDINGS_NOT_GENERATED` on every use). The real
// component-execution residual stays the P7.13 §7 human-gate list.
//
// instantiate() IS exercised below — once, against a substituted artifact —
// so this file's chain ends at the seam rather than assuming it:
#[test]
fn a_substituted_artifact_reaches_no_execution_even_under_a_fully_allowed_verdict() {
    let (gate, _gate_sink, _broker_sink) = composed_gate();
    let request = escalation_request(&["telemetry.read"], HONEST_VERSION);
    let ceiling = ProfileCeiling::new(vec![ceiling_entry("telemetry.read", data_scope("tenant-a"))]);
    let verdict = gate
        .evaluate(&InstantiationInput { request: &request, ceiling: &ceiling }, NOW)
        .expect("the verdict is allowed");

    // The artifact file whose BYTES were swapped after verification (the
    // supply-chain tamper); in this build the bytes are irrelevant — that IS
    // the honest point: nothing parses, nothing executes.
    let artifact_path = std::env::temp_dir().join("flexcore-p729-substituted-fixture.wasm");
    std::fs::write(&artifact_path, b"substituted-after-verification-bytes")
        .expect("the test fixture file writes");
    let artifact = ComponentArtifact::load(&artifact_path).expect("the fixture loads");
    let binding = instantiate(&verdict, &artifact)
        .expect("the default seam yields the fail-closed NoBindings object");
    let err = binding
        .verify()
        .expect_err("the NoBindings seam refuses every use of the substituted artifact");
    assert_eq!(err.name(), "BINDINGS_NOT_GENERATED");
    assert_eq!(binding.world(), WorldId::OptimizerPlugin);
    let _ = std::fs::remove_file(&artifact_path);
}
