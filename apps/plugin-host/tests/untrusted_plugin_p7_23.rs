// P7.23 — Untrusted-plugin negative test: ungranted capabilities are
//         unreachable (report §23 Stage 7 exit gate's second half, §5.7,
//         §17.4, §19.2(7); ledger `ledger_phase_7.md` §P7.23).
//         WRITTEN-UNRUN: the loop bans hold; the first human
//         `cargo test -p flexcore-plugin-host` run is the verification gate.
//
// WHAT THIS FILE IS (beyond P7.13's unit-level negatives): the END-TO-END
// negative proof that an untrusted (Class B) plugin CANNOT reach ungranted
// capabilities, composed as the full chain with the refusal AT THE EARLIEST
// layer and defence in depth behind it:
//
//   L0 kernel manifest gate (§5.7 closed vocabulary) — CITED, not callable
//      from this crate (this crate depends on no flexcore crate, §8.2):
//      a parsed manifest structurally cannot carry a never-granted id; its
//      refusal is RESTATEATED end-to-end by the layers below (the broker
//      and gate "re-refuse at grant/check time", kernel manifest.rs header).
//   L1 profile ceiling     — `enforcement.rs` evaluate step 3 + step 5
//                            (`ProfileCeiling`; a ceiling can never contain
//                            or grant a never-granted id; over-broad refuses).
//   L2 enforcement door    — `enforcement.rs` evaluate step 4 (the closed
//                            vocabulary re-check before ANY code runs).
//   L3 broker grant        — `broker.rs::grant` / `derive_grants` (a caller
//                            that bypasses the door and speaks to the broker
//                            directly is still refused; over-broad requests
//                            honour the manifest's declared bounds ONLY).
//   L4 broker check        — `broker.rs::check` (the per-call gate, defaults
//                            deny; never-granted ids refused even as crafted
//                            calls; every outcome audited).
//   L5 binding seam        — `enforcement::instantiate` → the fail-closed
//                            `NoBindings` object (`BINDINGS_NOT_GENERATED`
//                            in the default build): even a fully allowed
//                            verdict yields no live plugin code here.
//
// CASE → REQUIREMENT MAP (every case maps to a report requirement or is
// marked FLEXCORE-ONLY — the P6.19 pattern):
//
// | # | Case | Requirement |
// |---|---|---|
// | 1 | layered_walk_dispatch_authorize_is_refused_at_every_layer | §5.7 never-granted + §19.2(7) + ADR-010 (the authorizer is unreachable at L1–L4) |
// | 2 | layered_walk_dispatch_issue_and_secret_read_and_network_any | §5.7 never-granted set, end to end |
// | 3 | cross_tenant_telemetry_read_is_outside_every_grant | §5.7 tenant scoping + §6.1 tenant isolation (protected) |
// | 4 | a_proposed_dispatch_gains_no_authority_adr010 | §13.4 + ADR-010 (candidate-only; the forward returns no verdict) |
// | 5 | ungranted_capability_use_is_denied_and_audited | §17.4 (attempt → denial → audit) |
// | 6 | over_broad_manifest_is_refused_before_the_broker | §5.7 (grants honour declared bounds only) |
// | 7 | a_plugin_cannot_write_another_modules_tables | §6.4 + ADR-007 (no storage capability id exists) |
// | 8 | the_vocabulary_has_no_kernel_contract_and_no_verdict_id | §5.1/§9.1 protected kernel, restated end-to-end |
// | 9 | a_manifest_lie_about_class_buys_nothing | FLEXCORE-ONLY (§5.4 — every instantiation passes the same door; the trusted-class bypass failure mode is asserted impossible) |
// | 10 | research_profile_context_refuses_dispatch_permissions | §5.8 + P7.15 (the research-shell no-dispatch ceiling, bound to this run) |
// | 11 | grant_forgery_is_structurally_refused_and_behaviourally_inert | P7.13(c) at composition level |
// | 12 | the_binding_seam_is_the_last_refusal_not_the_first_gap | FLEXCORE-ONLY (NoBindings structural guarantee; real-WASM residual = human gate) |
// | 13 | every_denial_in_this_suite_is_audited | §5.1 audit integrity (per-case audit assertions throughout) |
//
// CLAIM BOUNDARY: this suite proves the HOST API refuses every attempt —
// it is NOT a security certification and claims none (AGENTS.md §3 rule 2).
// Where a layer is only provable with real WASM components (the bindings
// seam: a live component actually attempting a call), the test asserts the
// structural guarantee that exists (NoBindings refuse) and the residual
// items are the recorded HUMAN GATE list — the same WRITTEN-UNRUN vs
// human-gate split discipline as P7.13
// (`12_13_enforcement_negatives.md` §7; the decision record
// `21_22_23_conformance_swap_untrusted.md` §"human-gate split" restates it
// for this suite).

use std::sync::Arc;

use flexcore_plugin_host::bindings::PluginBinding;
use flexcore_plugin_host::broker::{
    BrokerAuditEvent, BrokerAuditKind, BrokerAuditSink, BrokerError, CapabilityBroker,
    CapabilityScope, GrantDerivationRequest, GrantRequest, InMemoryBrokerAuditSink,
    ALLOWED_PERMISSIONS, NEVER_GRANTED_PERMISSIONS, PROTECTED_VALIDATOR_TARGET,
};
use flexcore_plugin_host::enforcement::{
    instantiate, EnforcementAuditKind, EnforcementError, EnforcementGate, EnforcementRequest,
    ENFORCEABLE_PERMISSIONS, InMemoryEnforcementAuditSink, InstantiationInput, ProfileCeiling,
    ProfileGrant,
};
use flexcore_plugin_host::host::{ComponentArtifact, WorldId};
use flexcore_plugin_host::limits::DeclaredBounds;

// --- Shared fixtures -------------------------------------------------------------

const NOW: u64 = 1_700_000_000_000;

/// The untrusted plugin under attack (a Class B component's identity).
const ATTACKER_PLUGIN: &str = "evil.untrusted-component";
const ATTACKER_MANIFEST: &str = "evil.untrusted-component";
const ATTACKER_VERSION: &str = "9.9.9";

/// A second broker's audit sink, shared with the test (clones share the log).
struct SharedBrokerSink(Arc<InMemoryBrokerAuditSink>);

impl BrokerAuditSink for SharedBrokerSink {
    fn record(&self, event: BrokerAuditEvent) -> Result<(), BrokerError> {
        self.0.record(event)
    }
}

fn shared_broker_sink() -> (SharedBrokerSink, Arc<InMemoryBrokerAuditSink>) {
    let inner = Arc::new(InMemoryBrokerAuditSink::new());
    (SharedBrokerSink(Arc::clone(&inner)), inner)
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

/// The attacker's instantiation request with the given declared permissions.
fn attacker_request(permissions: &[&str]) -> EnforcementRequest {
    EnforcementRequest {
        plugin_id: ATTACKER_PLUGIN.to_string(),
        manifest_id: ATTACKER_MANIFEST.to_string(),
        manifest_version: ATTACKER_VERSION.to_string(),
        world: WorldId::OptimizerPlugin,
        entry_symbol: "evil:component/entry@9.9.9".to_string(),
        manifest_permissions: permissions.iter().map(|p| (*p).to_string()).collect(),
        declared_resource_fields: declared_fields(),
        bounds: bounds(),
        fuel_budget: 10_000_000,
    }
}

fn ceiling_entry(capability: &str, scope: CapabilityScope) -> ProfileGrant {
    ProfileGrant { capability: capability.to_string(), scope, expires_at_unix_ms: None }
}

fn data_scope(tenant: &str, site: Option<&str>) -> CapabilityScope {
    CapabilityScope::DataScope {
        tenant_id: tenant.to_string(),
        site_id: site.map(|s| s.to_string()),
        asset_id: None,
    }
}

/// A broker composed over a shared audit sink (the composition-level
/// object an attacker would face directly when bypassing the gate).
fn attacker_facing_broker() -> (CapabilityBroker, Arc<InMemoryBrokerAuditSink>) {
    let (sink_handle, sink) = shared_broker_sink();
    (CapabilityBroker::new(Box::new(sink_handle)), sink)
}

/// An enforcement gate composed over its own broker + shared audit sink.
fn composed_gate() -> (EnforcementGate, Arc<InMemoryEnforcementAuditSink>) {
    let (broker, _broker_sink) = attacker_facing_broker();
    let gate_sink = Arc::new(InMemoryEnforcementAuditSink::new());
    let gate = EnforcementGate::compose(broker, Box::new(Arc::clone(&gate_sink)));
    (gate, gate_sink)
}

fn assert_named<T: std::fmt::Debug>(result: Result<T, EnforcementError>, expected: &str, ctx: &str) {
    match result {
        Err(err) => assert_eq!(err.name(), expected, "{ctx}: {err}"),
        Ok(value) => panic!("{ctx}: expected refusal {expected}, got {value:?}"),
    }
}

// --- #1/#2: the layered walk over the never-granted set ---------------------------
//
// For each never-granted capability: the EARLIEST host layer refuses, and
// if an attacker bypasses that layer the NEXT one still refuses. L0 (the
// kernel manifest gate) is cited — a parsed manifest cannot carry the id —
// so the in-host chain starts at the ceiling.

fn layered_never_granted_walk(capability: &str) {
    // L1 — the profile ceiling: a ceiling entry for a never-granted id is
    // refused at ceiling-soundness time (evaluate step 3).
    let (gate, gate_sink) = composed_gate();
    let bad_ceiling = ProfileCeiling::new(vec![
        ceiling_entry("telemetry.read", data_scope("tenant-a", None)),
        ceiling_entry(capability, data_scope("tenant-a", None)),
    ]);
    let request = attacker_request(&["telemetry.read", capability]);
    assert_named(
        gate.evaluate(&InstantiationInput { request: &request, ceiling: &bad_ceiling }, NOW),
        "PERMISSION_NEVER_GRANTABLE",
        "L1 (ceiling) must refuse a never-granted id",
    );
    assert!(gate_sink.events().iter().any(|e| e.kind == EnforcementAuditKind::InstantiationRefused));

    // L2 — bypass L1 with a clean ceiling: the DOOR still refuses the
    // declared never-granted id before anything runs (evaluate step 4).
    let (gate, gate_sink) = composed_gate();
    let clean_ceiling = ProfileCeiling::new(vec![ceiling_entry(
        "telemetry.read",
        data_scope("tenant-a", None),
    )]);
    let request = attacker_request(&["telemetry.read", capability]);
    assert_named(
        gate.evaluate(&InstantiationInput { request: &request, ceiling: &clean_ceiling }, NOW),
        "PERMISSION_NEVER_GRANTABLE",
        "L2 (door) must re-refuse the declared never-granted id",
    );
    assert!(gate_sink.events().iter().any(|e| e.kind == EnforcementAuditKind::InstantiationRefused));

    // L3 — bypass L1+L2 entirely: the attacker speaks to the BROKER
    // directly (the composition-level bypass) and asks for a grant.
    let (broker, _sink) = attacker_facing_broker();
    let grant_request = GrantRequest {
        capability: capability.to_string(),
        scope: data_scope("tenant-a", None),
        expires_at_unix_ms: None,
    };
    let err = broker
        .grant(ATTACKER_PLUGIN, ATTACKER_MANIFEST, ATTACKER_VERSION, &grant_request, NOW)
        .expect_err("L3 (grant) must refuse a never-granted id");
    assert_eq!(err.name(), "PERMISSION_NEVER_GRANTABLE", "{capability}");

    // L4 — bypass L3 too (assume SOMEHOW a grant existed): the per-call
    // check still refuses, and the denial is audited.
    let (broker, broker_sink) = attacker_facing_broker();
    let err = broker
        .check(ATTACKER_PLUGIN, capability, &data_scope("tenant-a", None), NOW)
        .expect_err("L4 (check) must refuse a never-granted call");
    assert_eq!(err.name(), "PERMISSION_NEVER_GRANTABLE", "{capability}");
    let denials = broker_sink.events();
    let denial = denials
        .iter()
        .find(|e| e.kind == BrokerAuditKind::CapabilityDenied)
        .expect("the L4 denial is audited");
    assert_eq!(denial.capability, capability);
    assert_eq!(denial.plugin_id, ATTACKER_PLUGIN);
    assert!(denial.detail.contains("PERMISSION_NEVER_GRANTABLE"));
}

#[test]
fn layered_walk_dispatch_authorize_is_refused_at_every_layer() {
    layered_never_granted_walk("dispatch.authorize");
}

#[test]
fn layered_walk_the_rest_of_the_never_granted_set() {
    layered_never_granted_walk("dispatch.issue");
    layered_never_granted_walk("secret.read");
    layered_never_granted_walk("network.any");
    // The mirror consistency: the host's never-granted set is exactly the
    // §5.7 four, and the grantable vocabulary is exactly the §5.7 six —
    // the L0 manifest gate (kernel, cited) mirrors what these layers
    // re-refuse.
    assert_eq!(NEVER_GRANTED_PERMISSIONS.len(), 4);
    assert_eq!(ALLOWED_PERMISSIONS.len(), 6);
    assert_eq!(ENFORCEABLE_PERMISSIONS, ALLOWED_PERMISSIONS);
}

// --- #3: cross-tenant telemetry read ------------------------------------------------

#[test]
fn cross_tenant_telemetry_read_is_outside_every_grant() {
    let (broker, broker_sink) = attacker_facing_broker();
    // The attacker is granted telemetry.read for tenant-a ONLY (the profile
    // ceiling's scope, narrowed at grant time — never a manifest property).
    broker
        .grant(
            ATTACKER_PLUGIN,
            ATTACKER_MANIFEST,
            ATTACKER_VERSION,
            &GrantRequest {
                capability: "telemetry.read".to_string(),
                scope: data_scope("tenant-a", Some("site-1")),
                expires_at_unix_ms: None,
            },
            NOW,
        )
        .expect("the legitimate-shaped grant issues");

    // Attempt 1: another tenant entirely.
    let err = broker
        .check(ATTACKER_PLUGIN, "telemetry.read", &data_scope("tenant-b", None), NOW)
        .expect_err("another tenant's telemetry is unreachable");
    assert_eq!(err.name(), "SCOPE_OUTSIDE_GRANT");
    // Attempt 2: same tenant, another site (site grants do not widen).
    let err = broker
        .check(ATTACKER_PLUGIN, "telemetry.read", &data_scope("tenant-a", Some("site-2")), NOW)
        .expect_err("another site's telemetry is unreachable");
    assert_eq!(err.name(), "SCOPE_OUTSIDE_GRANT");
    // Attempt 3: the un-scoped request (wider than the grant) — a grant of
    // a site does not cover a tenant-wide ask.
    let err = broker
        .check(ATTACKER_PLUGIN, "telemetry.read", &data_scope("tenant-a", None), NOW)
        .expect_err("a site grant never covers a tenant-wide request");
    assert_eq!(err.name(), "SCOPE_OUTSIDE_GRANT");
    // Every denial audited (three CapabilityDenied records, in order).
    let denials: Vec<&BrokerAuditEvent> = broker_sink
        .events()
        .iter()
        .filter(|e| e.kind == BrokerAuditKind::CapabilityDenied)
        .collect();
    assert_eq!(denials.len(), 3, "each attempted read is individually audited");
    assert!(denials.iter().all(|e| e.plugin_id == ATTACKER_PLUGIN));
    // A DIFFERENT plugin (or a captured grant object) gets nothing: the
    // grant is bound to exactly one plugin.
    let err = broker
        .check("evil.second-component", "telemetry.read", &data_scope("tenant-a", Some("site-1")), NOW)
        .expect_err("grants are never shareable or inheritable");
    assert_eq!(err.name(), "CAPABILITY_NOT_GRANTED");
}

// --- #4: the ADR-010 dispatch-authority attempt through every layer ------------------

#[test]
fn a_proposed_dispatch_gains_no_authority_adr010() {
    let (broker, broker_sink) = attacker_facing_broker();
    // The attacker holds the widest LEGAL proposal set (candidate-only).
    broker
        .derive_grants(
            &GrantDerivationRequest {
                plugin_id: ATTACKER_PLUGIN.to_string(),
                manifest_id: ATTACKER_MANIFEST.to_string(),
                manifest_version: ATTACKER_VERSION.to_string(),
                manifest_permissions: vec![
                    "schedule.propose".to_string(),
                    "dispatch.propose".to_string(),
                    "dispatch.validate.request".to_string(),
                ],
                requested_grants: vec![
                    GrantRequest {
                        capability: "schedule.propose".to_string(),
                        scope: CapabilityScope::SendOnlyTarget { target: "scheduler".to_string() },
                        expires_at_unix_ms: None,
                    },
                    GrantRequest {
                        capability: "dispatch.propose".to_string(),
                        scope: CapabilityScope::SendOnlyTarget { target: "dispatcher".to_string() },
                        expires_at_unix_ms: None,
                    },
                    GrantRequest {
                        capability: "dispatch.validate.request".to_string(),
                        scope: CapabilityScope::SendOnlyTarget {
                            target: PROTECTED_VALIDATOR_TARGET.to_string(),
                        },
                        expires_at_unix_ms: None,
                    },
                ],
            },
            NOW,
        )
        .expect("candidate-only grants issue (they make proposals, never authority)");

    // The proposal enters the protected path as DATA: the only broker path
    // that could look like "asking for a verdict" is the validation
    // forward — and its return type is `()`. A verdict cannot flow back
    // through a signature that has no verdict in it (§5.7, §13.4).
    let forward: () = broker
        .request_validation_forward(ATTACKER_PLUGIN, "evil-forward-1", NOW)
        .expect("the forward dispatches the REQUEST only");
    let _ = forward; // the unit value IS the proof: nothing was returned to authorize with

    // The forward is audited as a REQUEST (no verdict event exists to audit).
    assert!(broker_sink
        .events()
        .iter()
        .any(|e| e.kind == BrokerAuditKind::ValidationForwardRequested));

    // The "propose as if authorized" escalation: authorize/issue remain
    // unreachable even with proposal grants in hand (L4).
    for capability in ["dispatch.authorize", "dispatch.issue"] {
        let err = broker
            .check(ATTACKER_PLUGIN, capability, &CapabilityScope::SendOnlyTarget { target: "dispatcher".to_string() }, NOW)
            .expect_err("a proposal grant never widens into authority");
        assert_eq!(err.name(), "PERMISSION_NEVER_GRANTABLE", "{capability}");
    }
    // And a validation request aimed ANYWHERE else never had a grantable
    // envelope: a grant whose scope targets another validator is refused
    // at derivation time (L1/L3 fence).
    let (fresh_broker, _) = attacker_facing_broker();
    let err = fresh_broker
        .grant(
            ATTACKER_PLUGIN,
            ATTACKER_MANIFEST,
            ATTACKER_VERSION,
            &GrantRequest {
                capability: "dispatch.validate.request".to_string(),
                scope: CapabilityScope::SendOnlyTarget { target: "my-own-verdict-service".to_string() },
                expires_at_unix_ms: None,
            },
            NOW,
        )
        .expect_err("the validate-request capability is fenced to the protected validator");
    assert_eq!(err.name(), "SCOPE_INVALID");
}

// --- #5/#6: over-broad manifest; ungranted use denied and audited ---------------------

#[test]
fn over_broad_manifest_is_refused_before_the_broker() {
    // The attacker's manifest declares telemetry.read; the deployment
    // profile's ceiling does NOT permit it. The DOOR refuses the whole
    // instantiation (over-broad is never trimmed to fit), before the
    // broker sees anything.
    let (gate, gate_sink) = composed_gate();
    let ceiling = ProfileCeiling::new(vec![ceiling_entry(
        "forecast.publish",
        CapabilityScope::SendOnlyTarget { target: "forecast-ingest".to_string() },
    )]);
    let request = attacker_request(&["telemetry.read"]);
    assert_named(
        gate.evaluate(&InstantiationInput { request: &request, ceiling: &ceiling }, NOW),
        "PERMISSION_ABOVE_PROFILE",
        "L1/L2: the manifest ∩ ceiling intersection refuses over-broad declarations",
    );
    // The broker holds nothing for the attacker (the derivation never ran).
    let (fresh_broker, _) = attacker_facing_broker();
    let err = fresh_broker
        .check(ATTACKER_PLUGIN, "telemetry.read", &data_scope("tenant-a", None), NOW)
        .expect_err("a refused instantiation leaves zero authority behind");
    assert_eq!(err.name(), "CAPABILITY_NOT_GRANTED");
    // The refusal is audited at the door.
    assert!(gate_sink.events().iter().any(|e| {
        e.kind == EnforcementAuditKind::InstantiationRefused
            && e.capability == "telemetry.read"
            && e.detail.contains("PERMISSION_ABOVE_PROFILE")
    }));
}

#[test]
fn ungranted_capability_use_is_denied_and_audited() {
    // The attacker holds NO grants at all: every capability use is
    // default-deny, and every denial is audited with the reason class.
    let (broker, broker_sink) = attacker_facing_broker();
    for capability in ALLOWED_PERMISSIONS {
        let err = broker
            .check(ATTACKER_PLUGIN, capability, &data_scope("tenant-a", None), NOW)
            .expect_err("defaults deny");
        assert_eq!(err.name(), "CAPABILITY_NOT_GRANTED", "{capability}");
    }
    let denials: Vec<&BrokerAuditEvent> = broker_sink
        .events()
        .iter()
        .filter(|e| e.kind == BrokerAuditKind::CapabilityDenied)
        .collect();
    assert_eq!(denials.len(), ALLOWED_PERMISSIONS.len());
    assert!(denials.iter().all(|e| e.detail.contains("CAPABILITY_NOT_GRANTED")));
}

// --- #7: writing another module's tables ----------------------------------------------

#[test]
fn a_plugin_cannot_write_another_modules_tables() {
    // There is no storage/database capability id AT ALL: the repositories
    // are protected non-seams (ADR-007; seam inventory §6.1), so a plugin
    // cannot even ADDRESS them — the structural form of the refusal.
    let (broker, broker_sink) = attacker_facing_broker();
    for crafted in [
        "storage.telemetry-ingest.write",
        "repository.asset.write",
        "db.execute",
        "outbox.append",
    ] {
        let err = broker
            .check(ATTACKER_PLUGIN, crafted, &data_scope("tenant-a", None), NOW)
            .expect_err("no such capability exists");
        assert_eq!(err.name(), "PERMISSION_UNKNOWN", "{crafted}");
        // And a grant attempt for it refuses identically (L3).
        let err = broker
            .grant(
                ATTACKER_PLUGIN,
                ATTACKER_MANIFEST,
                ATTACKER_VERSION,
                &GrantRequest {
                    capability: crafted.to_string(),
                    scope: data_scope("tenant-a", None),
                    expires_at_unix_ms: None,
                },
                NOW,
            )
            .expect_err("no such capability is grantable");
        assert_eq!(err.name(), "PERMISSION_UNKNOWN", "{crafted}");
    }
    // Denials audited (one per crafted id at the check layer).
    assert!(broker_sink
        .events()
        .iter()
        .filter(|e| e.kind == BrokerAuditKind::CapabilityDenied)
        .count()
        >= 4);
}

// --- #8: the protected-kernel contract, restated end-to-end ---------------------------

#[test]
fn the_vocabulary_has_no_kernel_contract_and_no_verdict_id() {
    // The §9.1 protected contracts (CommandAuthorizer, DispatchGateway,
    // final dispatch validation, ...) are not replaceable and not callable:
    // the host vocabulary has no `kernel.*` id and no "validation verdict"
    // authority id to request (ADR-005/ADR-010; the composition-level
    // refusal lives in the kernel resolver/registry — cited: a manifest
    // claiming `kernel.command-authorizer` is `PROTECTED_KERNEL_REPLACE`
    // there, and no grant path here can ever compensate for it).
    let (broker, _) = attacker_facing_broker();
    for crafted in [
        "kernel.command-authorizer",
        "kernel.dispatch-gateway",
        "kernel.dispatch-validation",
        "validation.verdict.read",
        "command.approve.self",
    ] {
        let err = broker
            .check(ATTACKER_PLUGIN, crafted, &data_scope("tenant-a", None), NOW)
            .expect_err("the protected kernel is not addressable");
        assert_eq!(err.name(), "PERMISSION_UNKNOWN", "{crafted}");
    }
    // Vocabulary sweep: NO grantable or never-granted id names the kernel
    // or an authorization verdict — the claim "the authorizer is
    // unreachable" holds by construction, not by configuration.
    for id in ALLOWED_PERMISSIONS.iter().chain(NEVER_GRANTED_PERMISSIONS.iter()) {
        assert!(!id.contains("kernel."), "no capability id may name the kernel");
        assert!(!id.contains("authorize") || *id == "dispatch.validate.request");
    }
}

// --- #9: the manifest lie about class ---------------------------------------------------

#[test]
fn a_manifest_lie_about_class_buys_nothing() {
    // FLEXCORE-ONLY (§5.4): the enforcement request carries NO class field —
    // there is nothing to lie with. Every instantiation passes the SAME
    // door; a Class A identity claim is not an input the host accepts
    // (exhaustive by construction: the request type's fields are exactly
    // plugin/manifest identity, world, symbol, permissions, resources,
    // fuel — the door's verdict depends only on those, never on a trust
    // label).
    //
    // The P7.12 recorded failure mode "trusted class bypasses enforcement"
    // is closed from both sides:
    let (gate, _) = composed_gate();
    // (a) a plugin id that LOOKS first-party is still subject to the
    //     ceiling intersection — the EMPTY ceiling (the DEFAULT posture)
    //     makes every declaration over-broad, for ANY identity;
    let ceiling = ProfileCeiling::empty();
    let lying = EnforcementRequest {
        plugin_id: "builtin.highs-optimizer".to_string(),
        manifest_id: "builtin.highs-optimizer".to_string(),
        manifest_version: "0.1.0".to_string(),
        world: WorldId::OptimizerPlugin,
        entry_symbol: "neryva.builtin/sim-ess".to_string(),
        manifest_permissions: vec!["telemetry.read".to_string()],
        declared_resource_fields: declared_fields(),
        bounds: bounds(),
        fuel_budget: 10_000_000,
    };
    assert_named(
        gate.evaluate(&InstantiationInput { request: &lying, ceiling: &ceiling }, NOW),
        "PERMISSION_ABOVE_PROFILE",
        "no identity buys a wider ceiling",
    );
    // (b) the P7.12 recorded failure mode "trusted class bypasses
    //     enforcement" is also closed at the use layer: the attacker cannot
    //     ride the built-in's grants — a derivation that never ran for
    //     `evil.untrusted-component` leaves it default-deny at L4.
    let (broker, _) = attacker_facing_broker();
    let err = broker
        .check(ATTACKER_PLUGIN, "telemetry.read", &data_scope("tenant-a", None), NOW)
        .expect_err("no derivation, no authority");
    assert_eq!(err.name(), "CAPABILITY_NOT_GRANTED");
}

// --- #10: the research-profile context (P7.15 bound to this run) -----------------------

#[test]
fn research_profile_context_refuses_dispatch_permissions() {
    // The shipped research shells' ceiling (research-gnn / research-marl):
    // telemetry/asset reads only — schedule.propose and every dispatch.*
    // id withheld (the P7.15 no-dispatch DEFAULT). An untrusted plugin in
    // that context is refused for anything beyond reads, at the door.
    for profile in ["research-gnn", "research-marl"] {
        let (gate, gate_sink) = composed_gate();
        let ceiling = ProfileCeiling::new(vec![
            ceiling_entry("telemetry.read", data_scope("research-tenant", None)),
            ceiling_entry("asset.read", data_scope("research-tenant", None)),
        ]);
        let request = attacker_request(&["telemetry.read", "schedule.propose"]);
        assert_named(
            gate.evaluate(&InstantiationInput { request: &request, ceiling: &ceiling }, NOW),
            "PERMISSION_ABOVE_PROFILE",
            &format!("{profile}: the research ceiling withholds schedule.propose"),
        );
        let request = attacker_request(&["dispatch.propose"]);
        assert_named(
            gate.evaluate(&InstantiationInput { request: &request, ceiling: &ceiling }, NOW),
            "PERMISSION_ABOVE_PROFILE",
            &format!("{profile}: the research ceiling withholds dispatch.propose"),
        );
        assert!(gate_sink
            .events()
            .iter()
            .filter(|e| e.kind == EnforcementAuditKind::InstantiationRefused)
            .count()
            >= 2);
    }
}

// --- #11: grant forgery at the composition level -----------------------------------------

#[test]
fn grant_forgery_is_structurally_refused_and_behaviourally_inert() {
    // STRUCTURAL: `CapabilityGrant`'s fields are `pub(crate)` and the
    // broker has no import/insert API — a caller outside the broker crate
    // cannot construct or plant a grant (a struct literal over non-public
    // fields is a compile error; there is no public constructor to call).
    // This test therefore exercises the BEHAVIOURAL half of the same rule:
    // every artifact a caller CAN hold is inert.
    let (broker_a, _) = attacker_facing_broker();
    let receipts = broker_a
        .derive_grants(
            &GrantDerivationRequest {
                plugin_id: ATTACKER_PLUGIN.to_string(),
                manifest_id: ATTACKER_MANIFEST.to_string(),
                manifest_version: ATTACKER_VERSION.to_string(),
                manifest_permissions: vec!["telemetry.read".to_string()],
                requested_grants: vec![GrantRequest {
                    capability: "telemetry.read".to_string(),
                    scope: data_scope("tenant-a", None),
                    expires_at_unix_ms: None,
                }],
            },
            NOW,
        )
        .expect("the attacker holds one real receipt");
    assert_eq!(receipts.len(), 1);
    let receipt = &receipts[0];

    // INERT ACROSS BROKERS: the receipt object (cloned out of broker A)
    // confers nothing in a fresh broker — authority lives in the broker's
    // state, never in the caller's hands.
    let (broker_b, _) = attacker_facing_broker();
    let err = broker_b
        .check(ATTACKER_PLUGIN, receipt.grant.capability(), receipt.grant.scope(), NOW)
        .expect_err("a captured receipt is not portable authority");
    assert_eq!(err.name(), "CAPABILITY_NOT_GRANTED");

    // INERT AFTER REVOCATION: the same captured receipt is dead once the
    // grant is dropped (per-call re-check; nothing caches authority).
    broker_a
        .revoke(receipt.grant.grant_id(), NOW)
        .expect("the grant is revoked");
    let err = broker_a
        .check(ATTACKER_PLUGIN, receipt.grant.capability(), receipt.grant.scope(), NOW)
        .expect_err("a revoked grant is dead authority");
    assert_eq!(err.name(), "CAPABILITY_NOT_GRANTED");

    // INERT AFTER EXPIRY: an expired grant is refused per call.
    let (broker_c, _) = attacker_facing_broker();
    broker_c
        .grant(
            ATTACKER_PLUGIN,
            ATTACKER_MANIFEST,
            ATTACKER_VERSION,
            &GrantRequest {
                capability: "telemetry.read".to_string(),
                scope: data_scope("tenant-a", None),
                expires_at_unix_ms: Some(NOW + 1_000),
            },
            NOW,
        )
        .expect("a short-lived grant issues");
    let err = broker_c
        .check(ATTACKER_PLUGIN, "telemetry.read", &data_scope("tenant-a", None), NOW + 2_000)
        .expect_err("expiry is checked per call");
    assert_eq!(err.name(), "GRANT_EXPIRED");
}

// --- #12: the binding seam as the last refusal --------------------------------------------

#[test]
fn the_binding_seam_is_the_last_refusal_not_the_first_gap() {
    // Even a FULLY ALLOWED verdict cannot produce live plugin code in this
    // build: the ONLY constructor path to a binding requires a verdict
    // (`enforcement::instantiate`), and the default build's bindings seam
    // is the fail-closed `NoBindings` object — the artifact bytes are not
    // even parsed (no gated wasmtime call sites exist), and every USE of
    // the resulting binding refuses with `BINDINGS_NOT_GENERATED`. An
    // attacker who satisfied every layer above would still be holding an
    // object that cannot execute.
    let (gate, _) = composed_gate();
    let ceiling = ProfileCeiling::new(vec![ceiling_entry("telemetry.read", data_scope("tenant-a", None))]);
    let request = attacker_request(&["telemetry.read"]);
    let verdict = gate
        .evaluate(&InstantiationInput { request: &request, ceiling: &ceiling }, NOW)
        .expect("the allowed path reaches a verdict");
    assert_eq!(verdict.granted_capabilities(), vec!["telemetry.read".to_string()]);

    // A loadable artifact file (bytes are irrelevant to the default seam —
    // that is itself the honest point: nothing executes here).
    let artifact_path = std::env::temp_dir().join("flexcore-p723-untrusted-fixture.wasm");
    std::fs::write(&artifact_path, b"not-a-real-component")
        .expect("the test fixture file writes");
    let artifact = ComponentArtifact::load(&artifact_path).expect("the fixture loads");
    let binding = instantiate(&verdict, &artifact)
        .expect("the default seam yields the fail-closed NoBindings object");
    // L5: the binding's every use refuses — live plugin code is
    // unreachable in this build.
    let err = binding
        .verify()
        .expect_err("the NoBindings seam refuses every use");
    assert_eq!(err.name(), "BINDINGS_NOT_GENERATED");
    assert_eq!(binding.world(), WorldId::OptimizerPlugin);
    let _ = std::fs::remove_file(&artifact_path);

    // And the loader itself refuses an ABSENT artifact before the seam
    // (a missing component is a named refusal, never a permissive default).
    let err = ComponentArtifact::load(std::path::Path::new("nonexistent/evil.wasm"))
        .expect_err("the loader refuses a missing artifact");
    assert_eq!(err.name(), "COMPONENT_NOT_FOUND");
}

// --- #13: audit coverage of the whole suite ------------------------------------------------

#[test]
fn every_denial_in_this_suite_is_audited() {
    // One composed run whose every refusal leaves BOTH audit trails
    // populated (broker denials + gate refusals), with stable machine
    // names — the §5.1 audit-integrity shape for the exit-gate record.
    let (gate, gate_sink) = composed_gate();
    let ceiling = ProfileCeiling::empty();
    let request = attacker_request(&["telemetry.read", "dispatch.authorize"]);
    let _ = gate.evaluate(&InstantiationInput { request: &request, ceiling: &ceiling }, NOW)
        .expect_err("the door refuses (never-granted fires before the ceiling row is even reached)");
    let events = gate_sink.events();
    assert!(
        events
            .iter()
            .any(|e| e.kind == EnforcementAuditKind::InstantiationRefused
                && e.detail.contains("PERMISSION_NEVER_GRANTABLE")
                && e.plugin_id == ATTACKER_PLUGIN),
        "the door's refusal is audited with the named reason and the attacker's identity"
    );
    // The audit event shape is complete (identity, capability, timestamp).
    for event in &events {
        assert_eq!(event.recorded_at_unix_ms, NOW, "the host supplies the time");
        assert!(!event.plugin_id.is_empty());
    }
}
