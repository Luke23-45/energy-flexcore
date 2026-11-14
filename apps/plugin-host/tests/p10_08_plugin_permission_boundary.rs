// P10.08 — Plugin permission-boundary tests (release-gate slice, plugin
//         host): the never-granted set refused everywhere, the capability
//         broker's default-deny, the enforcement-at-the-door unforgeable
//         verdict, the ADR-010 dispatch-bypass-unbuildable type-system
//         proof exercised, and the limits/fuel/deadline refusal codes
//         (report §22.3 gate 7; §5.4–§5.7; §9.3; §13.4; §17.5 invariant 7;
//         ledger
//         `ledger/phase-10-v1-release-hardening/ledger_phase_10.md` P10.08;
//         record:
//         `ledger/phase-10-v1-release-hardening/08_plugin_boundary_tests.md`).
//
// WHAT THIS FILE PROVES — over the REAL code paths (`apps/plugin-host/src/`:
// `broker.rs` P7.11, `enforcement.rs` P7.12, `limits.rs` P7.10, `host.rs`
// P7.10), by exercise:
//
// 1. THE NEVER-GRANTED SET (`dispatch.authorize`, `dispatch.issue`,
//    `secret.read`, `network.any` — the §5.7 never-granted rows) is refused
//    at EVERY boundary it can reach: the broker's grant path
//    (`PERMISSION_NEVER_GRANTABLE`), the broker's check path (a crafted
//    call — also audited as a denial), the enforcement door
//    (`PERMISSION_NEVER_GRANTABLE` BEFORE any component code runs), and a
//    profile ceiling carrying one (refused as misconfiguration). The kernel
//    manifest gate's per-CLASS refusals (`PERMISSION_NEVER_GRANTED` for
//    builtin-rust / wasm-component / grpc-worker) are pinned in the kernel
//    slice of this suite
//    (`crates/flexcore-kernel/tests/p10_08_plugin_lifecycle_boundary.rs`
//    shares the P10.08 record; the manifest gate lives in the kernel).
// 2. THE BROKER DEFAULT-DENY: with no grant, every capability call is
//    `CAPABILITY_NOT_GRANTED` and audited (`PLUGIN_CAPABILITY_DENIED`);
//    revocation takes effect at the NEXT call (no cached authority —
//    dropping the grant IS the revocation); expiry is checked per call
//    (`GRANT_EXPIRED` at `now >= expires_at`, inclusive hand-computed
//    boundary); every scope outside the grant's envelope is
//    `SCOPE_OUTSIDE_GRANT` (a `..` path escape, a sibling prefix, another
//    tenant, a wrong send-only target, a cross-kind scope — each a named
//    denial, never a coercion).
// 3. ENFORCEMENT AT THE DOOR: a verdict is UNFORGEABLE and FROZEN — the
//    only issuer is `EnforcementGate::evaluate` (private fields, no public
//    constructor: a struct literal outside the crate is a compile error),
//    the granted set cannot be widened after issuance, broker revocation
//    tightens a running instance at the next call while the frozen verdict
//    copy stands, and `instantiate` REQUIRES a verdict (the default build
//    refuses with `BINDINGS_NOT_GENERATED` — no build exists in which raw
//    manifest data reaches a binding). Audit-or-refuse in both directions.
// 4. THE ADR-010 TYPE-SYSTEM PROOF, EXERCISED (report §13.4, §17.5
//    invariant 7; ADR-010): no permission id — allowed, never-granted, or
//    crafted — can produce authority that dispatches. The strongest form is
//    TYPE-LEVEL: `CapabilityBroker::request_validation_forward` returns
//    `()` — a validation VERDICT cannot flow back through a signature that
//    has no verdict in it — and this suite binds that fact as a type
//    assertion (`let (): () = ...`). The maximum a plugin can hold is a
//    proposal or a request-forward; `dispatch.authorize` / `dispatch.issue`
//    do not exist in ANY grantable vocabulary, and a verdict-shaped id
//    (`dispatch.validate.verdict`) does not exist at all
//    (`PERMISSION_UNKNOWN`).
// 5. LIMITS / FUEL / DEADLINE REFUSAL CODES (§5.4 Class B): the declared
//    bounds are re-validated against the kernel's closed ranges at the door
//    (`LIMITS_INVALID`, per field), there is NO default fuel budget (an
//    unmetered instance is not constructible), the epoch deadline rounds
//    UP (a sub-tick deadline never grants a full extra tick), and the
//    enforcement-contract codes (`DEADLINE_EXCEEDED`, `FUEL_EXHAUSTED`,
//    `MEMORY_LIMIT_EXCEEDED`, `OUTPUT_LIMIT_EXCEEDED`, `CALL_RATE_EXCEEDED`)
//    are the named refusals the host applies.
//
// STATUS: WRITTEN-UNRUN (ENTRY_NOTE boundary 3; no cargo command was
// executed in this loop; zero observed failures is NOT claimed — the human
// build/test gate owns:
// `cargo test -p flexcore-plugin-host --test p10_08_plugin_permission_boundary`).
// Expected strings are the modules' own constants. The P7.11/P7.12/P7.13
// suites are cross-referenced (this file is the release-gate slice, not a
// replacement); every plugin class is held to the SAME rigor (the ledger's
// recorded risk: a built-in plugin tested less rigorously because it is
// "trusted" — the class split is a manifest/kernel concern, and the broker
// applies the identical rules to every holder).

use std::path::PathBuf;
use std::sync::Arc;

use flexcore_plugin_host::broker::{
    BrokerAuditKind, BrokerError, CapabilityBroker, CapabilityScope, GrantDerivationRequest,
    GrantRequest, InMemoryBrokerAuditSink, NEVER_GRANTED_PERMISSIONS, PROTECTED_VALIDATOR_TARGET,
};
use flexcore_plugin_host::enforcement::{
    EnforcementAuditKind, EnforcementError, EnforcementGate, EnforcementRequest,
    EnforcementVerdict, InMemoryEnforcementAuditSink, InstantiationInput, ProfileCeiling,
    ProfileGrant, ENFORCEABLE_PERMISSIONS,
};
use flexcore_plugin_host::host::{ComponentArtifact, HostError, WorldId};
use flexcore_plugin_host::limits::{
    epoch_ticks_for, DeclaredBounds, ResourceLimits, DEFAULT_MAX_CALLS_PER_MINUTE,
    DEFAULT_MAX_CONCURRENT_CALLS, DEFAULT_MAX_OUTPUT_MB, EPOCH_TICK_MILLIS,
};

const NOW: u64 = 1_700_000_000_000;

// --- Helpers -----------------------------------------------------------------

fn bounds() -> DeclaredBounds {
    DeclaredBounds {
        max_memory_mb: 256,
        timeout_ms: 5_000,
        max_output_mb: None,
        max_calls_per_minute: None,
    }
}

fn request(plugin_id: &str, permissions: &[&str], world: WorldId) -> EnforcementRequest {
    EnforcementRequest {
        plugin_id: plugin_id.to_string(),
        manifest_id: plugin_id.to_string(),
        manifest_version: "1.0.0".to_string(),
        world,
        entry_symbol: match world {
            WorldId::OptimizerPlugin => "flexcore:optimizer/optimizer".to_string(),
            WorldId::GridModelProviderPlugin => "flexcore:grid/grid-model-provider".to_string(),
            WorldId::TelemetrySourcePlugin => "flexcore:telemetry/telemetry-source".to_string(),
        },
        manifest_permissions: permissions.iter().map(|p| (*p).to_string()).collect(),
        declared_resource_fields: vec!["max-memory-mb".to_string(), "timeout-ms".to_string()],
        bounds: bounds(),
        fuel_budget: 100_000_000,
    }
}

fn broker() -> CapabilityBroker {
    CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()))
}

fn broker_with_sink() -> (CapabilityBroker, Arc<InMemoryBrokerAuditSink>) {
    let sink = Arc::new(InMemoryBrokerAuditSink::new());
    let broker = CapabilityBroker::new(Box::new(sink.clone()));
    (broker, sink)
}

fn gate() -> EnforcementGate {
    EnforcementGate::compose(broker(), Box::new(InMemoryEnforcementAuditSink::new()))
}

fn gate_with_sink() -> (EnforcementGate, Arc<InMemoryEnforcementAuditSink>) {
    let sink = Arc::new(InMemoryEnforcementAuditSink::new());
    let gate = EnforcementGate::compose(broker(), Box::new(sink.clone()));
    (gate, sink)
}

fn data_scope(tenant: &str) -> CapabilityScope {
    CapabilityScope::DataScope { tenant_id: tenant.to_string(), site_id: None, asset_id: None }
}

fn grant_request(capability: &str, scope: CapabilityScope) -> GrantRequest {
    GrantRequest { capability: capability.to_string(), scope, expires_at_unix_ms: None }
}

fn ceiling_entry(capability: &str, scope: CapabilityScope) -> ProfileGrant {
    ProfileGrant { capability: capability.to_string(), scope, expires_at_unix_ms: None }
}

fn scratch(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "flexcore-plugin-host-p10-08-{}-{}",
        std::process::id(),
        label
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir creates");
    dir
}

/// An executable-shaped artifact file (the bytes are UNTRUSTED INPUT —
/// nothing parses them in the default build; the loader caps the size).
fn artifact_file(label: &str) -> (PathBuf, ComponentArtifact) {
    let dir = scratch(label);
    let path = dir.join("component.wasm");
    std::fs::write(&path, b"fake-component-bytes-not-parsed-by-the-default-build")
        .expect("writes the fake artifact");
    let artifact = ComponentArtifact::load(&path).expect("loads");
    (dir, artifact)
}

// =================================================================================
// 1. The never-granted set is refused at EVERY boundary.
// =================================================================================

#[test]
fn every_never_granted_permission_is_refused_at_the_broker_grant_path() {
    // §5.7 never-granted rows: not grantable through ANY manifest, profile,
    // or broker call. The broker re-refuses even though the kernel manifest
    // gate would have rejected the manifest first (the recorded P7.11
    // re-refusal discipline).
    for capability in NEVER_GRANTED_PERMISSIONS {
        let broker = broker();
        let error = broker
            .grant(
                "plugin.hostile",
                "plugin.hostile",
                "1.0.0",
                &grant_request(capability, data_scope("tenant-1")),
                NOW,
            )
            .expect_err("never granted at grant time");
        assert_eq!(error.name(), "PERMISSION_NEVER_GRANTABLE", "capability {capability}");
        assert!(error.to_string().contains("never-granted set"));
    }
}

#[test]
fn every_never_granted_permission_is_refused_at_the_broker_check_path_and_audited() {
    // A CHECK against a never-granted capability can only be a crafted
    // call — refused and audited as a denial, always.
    for capability in NEVER_GRANTED_PERMISSIONS {
        let (broker, sink) = broker_with_sink();
        let error = broker
            .check("plugin.hostile", capability, &data_scope("tenant-1"), NOW)
            .expect_err("never granted at check time");
        assert_eq!(error.name(), "PERMISSION_NEVER_GRANTABLE", "capability {capability}");
        let denials: Vec<_> = sink
            .events()
            .iter()
            .filter(|e| e.kind == BrokerAuditKind::CapabilityDenied)
            .collect();
        assert_eq!(denials.len(), 1, "the crafted call is audited for {capability}");
        assert_eq!(denials[0].plugin_id, "plugin.hostile");
        assert_eq!(denials[0].capability, capability);
    }
}

#[test]
fn every_never_granted_permission_is_refused_at_the_enforcement_door_for_every_world() {
    // The door refuses BEFORE any component code runs — for EVERY recorded
    // world (the plugin classes compose through these worlds; the broker
    // applies identical rules to every holder, so no class is trusted
    // differently at this boundary).
    let worlds = [
        WorldId::OptimizerPlugin,
        WorldId::GridModelProviderPlugin,
        WorldId::TelemetrySourcePlugin,
    ];
    let (gate, sink) = gate_with_sink();
    let mut refused_cells = 0usize;
    for world in worlds {
        for capability in NEVER_GRANTED_PERMISSIONS {
            let request = request("plugin.hostile", &[capability], world);
            let error = gate
                .evaluate(
                    &InstantiationInput { request: &request, ceiling: &ProfileCeiling::empty() },
                    NOW,
                )
                .expect_err("never granted at the door");
            assert_eq!(
                error.name(),
                "PERMISSION_NEVER_GRANTABLE",
                "capability {capability} world {}", world.name()
            );
            refused_cells += 1;
        }
    }
    assert_eq!(refused_cells, 12, "4 never-granted ids x 3 worlds — every cell refused");
    // Every door refusal is audited.
    let refusals: Vec<_> = sink
        .events()
        .iter()
        .filter(|e| e.kind == EnforcementAuditKind::InstantiationRefused)
        .collect();
    assert_eq!(refusals.len(), 12, "every refusal is audited (zero silent refusals)");
}

#[test]
fn a_profile_ceiling_carrying_a_never_granted_id_is_refused_as_misconfiguration() {
    // Even the DEPLOYMENT's own profile cannot smuggle a never-granted
    // ceiling entry past the door.
    let (gate, _sink) = gate_with_sink();
    for capability in NEVER_GRANTED_PERMISSIONS {
        let request = request("plugin.ok", &["telemetry.read"], WorldId::TelemetrySourcePlugin);
        let ceiling = ProfileCeiling::new(vec![
            ceiling_entry("telemetry.read", data_scope("tenant-1")),
            ceiling_entry(capability, data_scope("tenant-1")),
        ]);
        let error = gate
            .evaluate(&InstantiationInput { request: &request, ceiling: &ceiling }, NOW)
            .expect_err("a never-granted ceiling entry is a misconfiguration");
        assert_eq!(error.name(), "PERMISSION_NEVER_GRANTABLE");
    }
}

#[test]
fn the_never_granted_set_is_the_closed_four_id_vocabulary() {
    // The mirror discipline: the broker's never-granted set is exactly the
    // four §5.7 ids — and NONE of them is also grantable (the allowed and
    // never-granted vocabularies are disjoint).
    assert_eq!(NEVER_GRANTED_PERMISSIONS.len(), 4);
    assert!(NEVER_GRANTED_PERMISSIONS.contains(&"dispatch.authorize"));
    assert!(NEVER_GRANTED_PERMISSIONS.contains(&"dispatch.issue"));
    assert!(NEVER_GRANTED_PERMISSIONS.contains(&"secret.read"));
    assert!(NEVER_GRANTED_PERMISSIONS.contains(&"network.any"));
    for granted in ENFORCEABLE_PERMISSIONS {
        assert!(
            !NEVER_GRANTED_PERMISSIONS.contains(&granted),
            "the vocabularies are disjoint: {granted}"
        );
    }
}

// =================================================================================
// 2. The capability broker default-deny.
// =================================================================================

#[test]
fn the_broker_default_denies_every_capability_with_no_grant_and_audits_it() {
    for capability in ENFORCEABLE_PERMISSIONS {
        let (broker, sink) = broker_with_sink();
        let error = broker
            .check("plugin.alpha", capability, &data_scope("tenant-1"), NOW)
            .expect_err("defaults deny");
        assert_eq!(error.name(), "CAPABILITY_NOT_GRANTED");
        assert!(error.to_string().contains("defaults deny"));
        assert!(
            sink.events()
                .iter()
                .any(|e| e.kind == BrokerAuditKind::CapabilityDenied
                    && e.plugin_id == "plugin.alpha"
                    && e.capability == capability),
            "the default-deny is audited for {capability}"
        );
    }
    // An unknown id is not even in the vocabulary.
    let (broker, _sink) = broker_with_sink();
    let error = broker
        .check("plugin.alpha", "device.protocolspeak", &data_scope("tenant-1"), NOW)
        .expect_err("unknown id");
    assert_eq!(error.name(), "PERMISSION_UNKNOWN");
}

#[test]
fn revocation_takes_effect_at_the_next_call_no_cached_authority_exists() {
    let (broker, sink) = broker_with_sink();
    let receipt = broker
        .grant(
            "plugin.alpha",
            "plugin.alpha",
            "1.0.0",
            &grant_request("telemetry.read", CapabilityScope::ReadOnlyPathPrefix {
                prefix: "/data/telemetry".to_string(),
            }),
            NOW,
        )
        .expect("grants");
    assert_eq!(
        sink.events().iter().filter(|e| e.kind == BrokerAuditKind::GrantIssued).count(),
        1
    );
    // The use is honoured under the grant.
    broker
        .check(
            "plugin.alpha",
            "telemetry.read",
            &CapabilityScope::ReadOnlyPathPrefix { prefix: "/data/telemetry/site-a".to_string() },
            NOW,
        )
        .expect("the granted use passes");
    // Revocation: the grant is DROPPED; the very next call is denied
    // (nothing between calls holds authority).
    broker.revoke(receipt.grant.grant_id(), NOW + 1).expect("revokes");
    let error = broker
        .check(
            "plugin.alpha",
            "telemetry.read",
            &CapabilityScope::ReadOnlyPathPrefix { prefix: "/data/telemetry/site-a".to_string() },
            NOW + 2,
        )
        .expect_err("revoked at the next call");
    assert_eq!(error.name(), "CAPABILITY_NOT_GRANTED");
    assert_eq!(
        sink.events().iter().filter(|e| e.kind == BrokerAuditKind::GrantRevoked).count(),
        1
    );
    // Revoking an unknown grant id is an error, not a silent success.
    let error = broker.revoke("grant/nothing/telemetry.read/999", NOW).expect_err("unknown");
    assert_eq!(error.name(), "GRANT_UNKNOWN");
}

#[test]
fn grant_expiry_boundary_is_hand_computed_exclusive() {
    let broker = broker();
    let expires_at = NOW + 60_000;
    let receipt = broker
        .grant(
            "plugin.alpha",
            "plugin.alpha",
            "1.0.0",
            &GrantRequest {
                capability: "asset.read".to_string(),
                scope: data_scope("tenant-1"),
                expires_at_unix_ms: Some(expires_at),
            },
            NOW,
        )
        .expect("grants");
    // Before the expiry: honoured.
    broker
        .check("plugin.alpha", "asset.read", &data_scope("tenant-1"), expires_at - 1)
        .expect("one millis before expiry the grant is live");
    // AT the expiry instant: expired (exclusive boundary — use at
    // `now >= expires_at` is GRANT_EXPIRED).
    let error = broker
        .check("plugin.alpha", "asset.read", &data_scope("tenant-1"), expires_at)
        .expect_err("at the expiry instant the grant is expired");
    assert!(matches!(error, BrokerError::GrantExpired { .. }));
    assert!(error.to_string().contains("checked per call"));
    let _ = receipt; // the grant id is in the broker's records
}

#[test]
fn every_scope_outside_the_grant_envelope_is_a_named_denial() {
    let broker = broker();
    // A read-path grant.
    broker
        .grant(
            "plugin.alpha",
            "plugin.alpha",
            "1.0.0",
            &grant_request(
                "telemetry.read",
                CapabilityScope::ReadOnlyPathPrefix { prefix: "/data/telemetry".to_string() },
            ),
            NOW,
        )
        .expect("grants");
    // (a) The `..` escape: refused even though the string starts inside.
    let error = broker
        .check(
            "plugin.alpha",
            "telemetry.read",
            &CapabilityScope::ReadOnlyPathPrefix {
                prefix: "/data/telemetry/../secret".to_string(),
            },
            NOW,
        )
        .expect_err("a path escape is refused");
    assert_eq!(error.name(), "SCOPE_OUTSIDE_GRANT");
    // (b) The sibling prefix: a grant of /data/telemetry does NOT cover
    // /data/telemetry-evil (the grant prefix is normalised to a segment
    // boundary — fail-closed).
    let error = broker
        .check(
            "plugin.alpha",
            "telemetry.read",
            &CapabilityScope::ReadOnlyPathPrefix { prefix: "/data/telemetry-evil".to_string() },
            NOW,
        )
        .expect_err("a sibling prefix is outside the envelope");
    assert_eq!(error.name(), "SCOPE_OUTSIDE_GRANT");
    // (c) A send-only grant covers EXACTLY its target.
    broker
        .grant(
            "plugin.alpha",
            "plugin.alpha",
            "1.0.0",
            &grant_request(
                "forecast.publish",
                CapabilityScope::SendOnlyTarget { target: "forecast-queue".to_string() },
            ),
            NOW,
        )
        .expect("grants");
    let error = broker
        .check(
            "plugin.alpha",
            "forecast.publish",
            &CapabilityScope::SendOnlyTarget { target: "other-queue".to_string() },
            NOW,
        )
        .expect_err("a different target is outside the envelope");
    assert_eq!(error.name(), "SCOPE_OUTSIDE_GRANT");
    // (d) A data-scope grant covers its tenant (and narrower), never
    // another tenant, and never a cross-KIND request.
    broker
        .grant(
            "plugin.alpha",
            "plugin.alpha",
            "1.0.0",
            &grant_request("asset.read", data_scope("tenant-1")),
            NOW,
        )
        .expect("grants");
    broker
        .check("plugin.alpha", "asset.read", &data_scope("tenant-1"), NOW)
        .expect("the same tenant is covered");
    let error = broker
        .check("plugin.alpha", "asset.read", &data_scope("tenant-2"), NOW)
        .expect_err("another tenant is refused");
    assert_eq!(error.name(), "SCOPE_OUTSIDE_GRANT");
    let error = broker
        .check(
            "plugin.alpha",
            "asset.read",
            &CapabilityScope::ReadOnlyPathPrefix { prefix: "/data/telemetry".to_string() },
            NOW,
        )
        .expect_err("a cross-kind request is a denial, never a coercion");
    assert_eq!(error.name(), "SCOPE_OUTSIDE_GRANT");
}

#[test]
fn the_validate_request_capability_can_only_ever_target_the_protected_validator() {
    // §P7.11 step 4: a grant of `dispatch.validate.request` naming any
    // other target is refused AT GRANT TIME — a plugin can never hold a
    // grant whose envelope would let it fish for verdicts elsewhere.
    let broker = broker();
    let error = broker
        .grant(
            "plugin.alpha",
            "plugin.alpha",
            "1.0.0",
            &grant_request(
                "dispatch.validate.request",
                CapabilityScope::SendOnlyTarget { target: "evil-verdict-collector".to_string() },
            ),
            NOW,
        )
        .expect_err("only the protected validator target");
    assert_eq!(error.name(), "SCOPE_INVALID");
    // The correct target grants.
    broker
        .grant(
            "plugin.alpha",
            "plugin.alpha",
            "1.0.0",
            &grant_request(
                "dispatch.validate.request",
                CapabilityScope::SendOnlyTarget { target: PROTECTED_VALIDATOR_TARGET.to_string() },
            ),
            NOW,
        )
        .expect("the protected target grants");
}

// =================================================================================
// 3. Enforcement at the door: unforgeable, frozen verdicts; instantiate
//    requires a verdict; audit-or-refuse both directions.
// =================================================================================

#[test]
fn the_verdict_is_frozen_at_instantiation_and_revocation_tightens_only_the_live_authority() {
    let (gate, sink) = gate_with_sink();
    let request = request(
        "plugin.alpha",
        &["telemetry.read", "asset.read"],
        WorldId::TelemetrySourcePlugin,
    );
    let ceiling = ProfileCeiling::new(vec![
        ceiling_entry("telemetry.read", data_scope("tenant-1")),
        ceiling_entry("asset.read", data_scope("tenant-1")),
    ]);
    let verdict = gate
        .evaluate(&InstantiationInput { request: &request, ceiling: &ceiling }, NOW)
        .expect("the door allows the in-profile request");
    assert_eq!(verdict.granted_capabilities(), vec!["telemetry.read", "asset.read"]);
    assert_eq!(
        sink.events()
            .iter()
            .filter(|e| e.kind == EnforcementAuditKind::CapabilitySetFrozen)
            .count(),
        1,
        "the freeze is audited"
    );
    // The frozen copy stands even after the broker's live grants are
    // revoked: the verdict cannot be widened OR shrunk after issuance —
    // the LIVE authority (per-call broker checks) is what tightens.
    let grant_id = verdict.grants()[0].grant_id().to_string();
    gate.broker().revoke(&grant_id, NOW + 1).expect("revokes");
    let error = gate
        .broker()
        .check("plugin.alpha", "telemetry.read", &data_scope("tenant-1"), NOW + 2)
        .expect_err("the live authority is revoked");
    assert_eq!(error.name(), "CAPABILITY_NOT_GRANTED");
    assert_eq!(
        verdict.granted_capabilities(),
        vec!["telemetry.read", "asset.read"],
        "the frozen set stands (frozen at instantiation; only re-instantiation via \
         drain-before-replace changes it)"
    );
    // A mutated REQUEST object does not expand the issued verdict: the
    // verdict owns cloned copies, not references — `Clone` copies freeze
    // further, so mutating even a fresh clone of the request cannot reach
    // the issued authority.
    let mut widened = request.clone();
    widened.manifest_permissions.push("dispatch.authorize".to_string());
    assert!(widened
        .manifest_permissions
        .contains(&"dispatch.authorize".to_string()));
    assert_eq!(
        verdict.granted_capabilities(),
        vec!["telemetry.read", "asset.read"],
        "the issued verdict is immutable by construction"
    );
}

#[test]
fn instantiate_requires_a_verdict_and_the_default_build_refuses_at_the_binding_seam() {
    // The ONLY public constructor path to a binding takes a verdict; the
    // default build's binding seam is fail-closed (BINDINGS_NOT_GENERATED):
    // no build exists in which raw manifest data reaches a binding, and the
    // verdict is required in BOTH builds.
    let gate = gate();
    let request = request(
        "plugin.alpha",
        &["telemetry.read"],
        WorldId::TelemetrySourcePlugin,
    );
    let ceiling = ProfileCeiling::new(vec![ceiling_entry("telemetry.read", data_scope("tenant-1"))]);
    let verdict = gate
        .evaluate(&InstantiationInput { request: &request, ceiling: &ceiling }, NOW)
        .expect("the door allows");
    let (_dir, artifact) = artifact_file("instantiate");
    let error = flexcore_plugin_host::enforcement::instantiate(&verdict, &artifact)
        .expect_err("the default build has no generated bindings");
    match &error {
        EnforcementError::HostRefusal { error } => {
            assert_eq!(error.name(), "BINDINGS_NOT_GENERATED");
        }
        other => panic!("expected HostRefusal, got {other:?}"),
    }
}

#[test]
fn a_broken_audit_sink_refuses_the_door_decision_in_both_directions() {
    // Audit-or-refuse: an ALLOW that cannot be audited is rolled back (the
    // just-issued grants are REVOKED — unaudited authority is not
    // authority), and a REFUSAL that cannot be audited surfaces as
    // AUDIT_UNAVAILABLE.
    #[derive(Debug, Default)]
    struct FailingSink;
    impl flexcore_plugin_host::enforcement::EnforcementAuditSink for FailingSink {
        fn record(
            &self,
            _event: flexcore_plugin_host::enforcement::EnforcementAuditEvent,
        ) -> Result<(), EnforcementError> {
            Err(EnforcementError::AuditUnavailable {
                reason: "the test sink always fails".to_string(),
            })
        }
    }
    let gate = EnforcementGate::compose(broker(), Box::new(FailingSink));
    // The allow direction: refused with AUDIT_UNAVAILABLE, and the broker
    // holds NOTHING afterwards (the rollback).
    let request = request("plugin.alpha", &["telemetry.read"], WorldId::TelemetrySourcePlugin);
    let ceiling = ProfileCeiling::new(vec![ceiling_entry("telemetry.read", data_scope("tenant-1"))]);
    let error = gate
        .evaluate(&InstantiationInput { request: &request, ceiling: &ceiling }, NOW)
        .expect_err("an unauditable allow is not authority");
    assert_eq!(error.name(), "AUDIT_UNAVAILABLE");
    let error = gate
        .broker()
        .check("plugin.alpha", "telemetry.read", &data_scope("tenant-1"), NOW)
        .expect_err("the rolled-back grant is gone");
    assert_eq!(error.name(), "CAPABILITY_NOT_GRANTED");
    // The refuse direction: the same AUDIT_UNAVAILABLE replaces the refusal.
    let request = request("plugin.hostile", &["dispatch.authorize"], WorldId::OptimizerPlugin);
    let error = gate
        .evaluate(
            &InstantiationInput { request: &request, ceiling: &ProfileCeiling::empty() },
            NOW,
        )
        .expect_err("an unauditable refusal surfaces as audit-unavailable");
    assert_eq!(error.name(), "AUDIT_UNAVAILABLE");
}

#[test]
fn an_over_broad_request_is_refused_never_trimmed_to_fit() {
    // The intersection rule: a manifest permission with no ceiling entry
    // REFUSES the whole request (PERMISSION_ABOVE_PROFILE) — the gate never
    // silently drops the over-broad row and grants the rest.
    let (gate, sink) = gate_with_sink();
    let request = request(
        "plugin.alpha",
        &["telemetry.read", "forecast.publish"],
        WorldId::TelemetrySourcePlugin,
    );
    let ceiling = ProfileCeiling::new(vec![ceiling_entry("telemetry.read", data_scope("tenant-1"))]);
    let error = gate
        .evaluate(&InstantiationInput { request: &request, ceiling: &ceiling }, NOW)
        .expect_err("over-broad requests are refused");
    assert_eq!(error.name(), "PERMISSION_ABOVE_PROFILE");
    assert!(error.to_string().contains("never trimmed"));
    // And the EMPTY ceiling (the default posture of a deployment that
    // composed no profile) makes every declared permission over-broad.
    let request = request("plugin.alpha", &["telemetry.read"], WorldId::TelemetrySourcePlugin);
    let error = gate
        .evaluate(&InstantiationInput { request: &request, ceiling: &ProfileCeiling::empty() }, NOW)
        .expect_err("an empty ceiling permits nothing");
    assert_eq!(error.name(), "PERMISSION_ABOVE_PROFILE");
    let _ = sink;
}

// =================================================================================
// 4. The ADR-010 type-system proof, exercised.
// =================================================================================

#[test]
fn the_validation_forward_returns_the_unit_type_a_verdict_cannot_flow_back() {
    // THE TYPE-LEVEL PROOF (ADR-010 / §13.4, made concrete): the broker's
    // request path returns `()` — a validation verdict cannot flow back
    // through a signature that has no verdict in it. This assertion binds
    // the return type: the Ok payload IS the unit type.
    let broker = broker();
    broker
        .grant(
            "plugin.alpha",
            "plugin.alpha",
            "1.0.0",
            &grant_request(
                "dispatch.validate.request",
                CapabilityScope::SendOnlyTarget { target: PROTECTED_VALIDATOR_TARGET.to_string() },
            ),
            NOW,
        )
        .expect("grants");
    let unit: () = broker
        .request_validation_forward("plugin.alpha", "forward-1", NOW)
        .expect("the forward is dispatched");
    let _: () = unit; // the type assertion, spelled out
    // The forward is audited; the audit detail states the boundary verbatim.
    let (broker, sink) = broker_with_sink();
    broker
        .grant(
            "plugin.alpha",
            "plugin.alpha",
            "1.0.0",
            &grant_request(
                "dispatch.validate.request",
                CapabilityScope::SendOnlyTarget { target: PROTECTED_VALIDATOR_TARGET.to_string() },
            ),
            NOW,
        )
        .expect("grants");
    broker
        .request_validation_forward("plugin.alpha", "forward-2", NOW)
        .expect("dispatches");
    let events = sink.events();
    assert!(events.iter().any(|e| e.kind == BrokerAuditKind::ValidationForwardRequested
        && e.detail.contains("NEVER returns a verdict")));
}

#[test]
fn no_permission_id_conveys_dispatch_authority_unbuildable_not_discouraged() {
    // The ADR-010 proof, exhaustive over the vocabularies: NO permission id
    // — allowed, never-granted, or crafted — can produce a verdict that
    // conveys dispatch authority. The allowed set yields proposals only;
    // everything else refuses.
    let gate = gate();
    let allowed = ENFORCEABLE_PERMISSIONS;
    // dispatch.validate.request cannot carry a send-only candidate target
    // (it must target exactly the protected validator), so the full-ceiling
    // derivation refuses on THAT row — which is itself the fence working.
    // Evaluate the remaining rows individually with a candidate-queue
    // ceiling (the widest a proposal can get).
    for capability in allowed {
        if *capability == "dispatch.validate.request" {
            continue;
        }
        let request = request("plugin.alpha", &[capability], WorldId::OptimizerPlugin);
        let single = ProfileCeiling::new(vec![ceiling_entry(
            capability,
            CapabilityScope::SendOnlyTarget { target: "candidate-queue".to_string() },
        )]);
        let verdict = gate
            .evaluate(&InstantiationInput { request: &request, ceiling: &single }, NOW)
            .unwrap_or_else(|e| panic!("allowed capability {capability} must evaluate: {e}"));
        // The granted set is a SUBSET of the allowed vocabulary and names
        // NO authorization/issue capability: the maximum a plugin can hold
        // is a proposal or a request-forward.
        for grant in verdict.grants() {
            assert!(allowed.contains(&grant.capability()), "no grant outside the closed vocabulary");
            assert_ne!(grant.capability(), "dispatch.authorize");
            assert_ne!(grant.capability(), "dispatch.issue");
        }
    }
    // The crafted set: every protected-kernel id plus the verdict-authority
    // spelling refuses outright.
    let gate = gate();
    for capability in [
        "dispatch.authorize",
        "dispatch.issue",
        "dispatch.validate.verdict",
        "secret.read",
        "network.any",
        "device.protocolspeak",
    ] {
        let request = request("plugin.hostile", &[capability], WorldId::OptimizerPlugin);
        let error = gate
            .evaluate(
                &InstantiationInput { request: &request, ceiling: &ProfileCeiling::empty() },
                NOW,
            )
            .expect_err("protected-kernel authority is unbuildable");
        let expected = if capability == "dispatch.validate.verdict"
            || capability == "device.protocolspeak"
        {
            "PERMISSION_UNKNOWN"
        } else {
            "PERMISSION_NEVER_GRANTABLE"
        };
        assert_eq!(error.name(), expected, "capability {capability}");
    }
}

#[test]
fn a_derivation_batch_is_atomic_and_the_over_broad_declaration_is_refused() {
    // Grants honour the manifest's DECLARED bounds only: a request for a
    // capability the manifest did not declare is over-broad and refused;
    // a batch never half-applies.
    let broker = broker();
    let derivation = GrantDerivationRequest {
        plugin_id: "plugin.alpha".to_string(),
        manifest_id: "plugin.alpha".to_string(),
        manifest_version: "1.0.0".to_string(),
        manifest_permissions: vec!["telemetry.read".to_string()],
        requested_grants: vec![
            grant_request("telemetry.read", data_scope("tenant-1")),
            grant_request("asset.read", data_scope("tenant-1")), // NOT declared
        ],
    };
    let error = broker.derive_grants(&derivation, NOW).expect_err("over-broad");
    assert_eq!(error.name(), "PERMISSION_NOT_DECLARED");
    // Atomicity: NOTHING was granted.
    let error = broker
        .check("plugin.alpha", "telemetry.read", &data_scope("tenant-1"), NOW)
        .expect_err("the batch never half-applies");
    assert_eq!(error.name(), "CAPABILITY_NOT_GRANTED");
    // A duplicate capability in one batch is refused too.
    let derivation = GrantDerivationRequest {
        plugin_id: "plugin.alpha".to_string(),
        manifest_id: "plugin.alpha".to_string(),
        manifest_version: "1.0.0".to_string(),
        manifest_permissions: vec!["telemetry.read".to_string()],
        requested_grants: vec![
            grant_request("telemetry.read", data_scope("tenant-1")),
            grant_request("telemetry.read", data_scope("tenant-2")),
        ],
    };
    let error = broker.derive_grants(&derivation, NOW).expect_err("duplicate");
    assert_eq!(error.name(), "DUPLICATE_GRANT_REQUEST");
}

// =================================================================================
// 5. Limits / fuel / deadline refusal codes.
// =================================================================================

#[test]
fn there_is_no_default_fuel_budget_an_unmetered_instance_is_not_constructible() {
    let error = ResourceLimits::from_declared_bounds(&bounds(), 0)
        .expect_err("fuel is mandatory — there is no default budget");
    match &error {
        HostError::LimitsInvalid { field, .. } => assert_eq!(field, "fuel-budget"),
        other => panic!("expected LimitsInvalid, got {other:?}"),
    }
    assert_eq!(error.name(), "LIMITS_INVALID");
}

#[test]
fn every_declared_bound_is_revalidated_against_the_kernel_closed_ranges() {
    // Memory below/above the closed range 1..=65_536.
    for bad in [0u64, 65_537] {
        let bounds = DeclaredBounds { max_memory_mb: bad, ..bounds() };
        let error = ResourceLimits::from_declared_bounds(&bounds, 1_000_000)
            .expect_err("outside the closed range");
        match &error {
            HostError::LimitsInvalid { field, .. } => assert_eq!(field, "max-memory-mb"),
            other => panic!("expected LimitsInvalid, got {other:?}"),
        }
    }
    // Timeout outside 1..=3_600_000 (zero = the caller's bug).
    let bounds = DeclaredBounds { timeout_ms: 0, ..bounds() };
    let error = ResourceLimits::from_declared_bounds(&bounds, 1_000_000).expect_err("zero timeout");
    match &error {
        HostError::LimitsInvalid { field, .. } => assert_eq!(field, "timeout-ms"),
        other => panic!("expected LimitsInvalid, got {other:?}"),
    }
    // Output outside 1..=4_096 (when declared).
    let bounds = DeclaredBounds { max_output_mb: Some(4_097), ..bounds() };
    let error = ResourceLimits::from_declared_bounds(&bounds, 1_000_000).expect_err("over range");
    match &error {
        HostError::LimitsInvalid { field, .. } => assert_eq!(field, "max-output-mb"),
        other => panic!("expected LimitsInvalid, got {other:?}"),
    }
    // Call rate outside 1..=1_000_000 (when declared).
    let bounds = DeclaredBounds { max_calls_per_minute: Some(0), ..bounds() };
    let error = ResourceLimits::from_declared_bounds(&bounds, 1_000_000).expect_err("zero rate");
    match &error {
        HostError::LimitsInvalid { field, .. } => assert_eq!(field, "max-calls-per-minute"),
        other => panic!("expected LimitsInvalid, got {other:?}"),
    }
    // The valid bounds construct with the DEFAULT caps for the omitted
    // fields (host-side caps, never silent permissiveness — the DEFAULT
    // decisions are recorded).
    let limits = ResourceLimits::from_declared_bounds(&bounds(), 42_000_000).expect("valid");
    assert_eq!(limits.max_output_bytes(), DEFAULT_MAX_OUTPUT_MB * 1024 * 1024);
    assert_eq!(limits.max_calls_per_minute(), DEFAULT_MAX_CALLS_PER_MINUTE);
    assert_eq!(limits.max_concurrent_calls(), DEFAULT_MAX_CONCURRENT_CALLS);
    assert_eq!(limits.fuel_budget(), 42_000_000);
    // The page conversion is exact: 1 MiB = 16 wasm pages (hand-computed).
    let exact = DeclaredBounds { max_memory_mb: 1, ..bounds() };
    let limits = ResourceLimits::from_declared_bounds(&exact, 1_000_000).expect("valid");
    assert_eq!(limits.max_memory_pages(), 16);
}

#[test]
fn the_epoch_deadline_rounds_up_and_never_grants_a_full_extra_tick() {
    // Hand-computed boundaries (EPOCH_TICK_MILLIS = 10):
    //   1 ms → 1 tick; 10 ms → 1 tick; 11 ms → 2 ticks; 3_600_000 ms →
    //   360_000 ticks; 0 ms (a caller bug) still yields 1 tick (total).
    assert_eq!(EPOCH_TICK_MILLIS, 10);
    assert_eq!(epoch_ticks_for(0), 1);
    assert_eq!(epoch_ticks_for(1), 1);
    assert_eq!(epoch_ticks_for(10), 1);
    assert_eq!(epoch_ticks_for(11), 2);
    assert_eq!(epoch_ticks_for(3_600_000), 360_000);
    // The derived deadline equals the declared timeout (one source of truth).
    let limits = ResourceLimits::from_declared_bounds(&bounds(), 1_000_000).expect("valid");
    assert_eq!(limits.wall_clock_deadline_ms(), 5_000);
    assert_eq!(limits.epoch_ticks(), 500);
}

#[test]
fn the_door_refuses_resource_bound_lies_and_vocabulary_drift_before_any_code_runs() {
    let (gate, sink) = gate_with_sink();
    // A declared bound outside the closed range is refused at the door.
    let request = EnforcementRequest {
        bounds: DeclaredBounds { max_memory_mb: 0, ..bounds() },
        ..request("plugin.alpha", &["telemetry.read"], WorldId::TelemetrySourcePlugin)
    };
    let ceiling = ProfileCeiling::new(vec![ceiling_entry("telemetry.read", data_scope("tenant-1"))]);
    let error = gate
        .evaluate(&InstantiationInput { request: &request, ceiling: &ceiling }, NOW)
        .expect_err("the resource-bound lie is refused at the door");
    assert_eq!(error.name(), "LIMITS_INVALID");
    // Vocabulary drift: the ledger's `timeoutMs` spelling is NOT silently
    // interpreted — PERMISSION_VOCABULARY_DRIFT, never a default.
    let request = EnforcementRequest {
        declared_resource_fields: vec![
            "max-memory-mb".to_string(),
            "timeoutMs".to_string(), // the unalignable spelling
        ],
        ..request("plugin.alpha", &["telemetry.read"], WorldId::TelemetrySourcePlugin)
    };
    let error = gate
        .evaluate(&InstantiationInput { request: &request, ceiling: &ceiling }, NOW)
        .expect_err("an unalignable field is refused");
    assert_eq!(error.name(), "PERMISSION_VOCABULARY_DRIFT");
    assert!(error.to_string().contains("never given a default interpretation"));
    // A fuel/concurrency field the kernel schema does not carry: drift too.
    let request = EnforcementRequest {
        declared_resource_fields: vec![
            "max-memory-mb".to_string(),
            "timeout-ms".to_string(),
            "fuel-budget".to_string(),
        ],
        ..request("plugin.alpha", &["telemetry.read"], WorldId::TelemetrySourcePlugin)
    };
    let error = gate
        .evaluate(&InstantiationInput { request: &request, ceiling: &ceiling }, NOW)
        .expect_err("a fuel field in the manifest resources object is drift");
    assert_eq!(error.name(), "PERMISSION_VOCABULARY_DRIFT");
    // A missing fuel budget at the door: LIMITS_INVALID (the mandatory
    // deployment-configured budget).
    let request = EnforcementRequest {
        fuel_budget: 0,
        ..request("plugin.alpha", &["telemetry.read"], WorldId::TelemetrySourcePlugin)
    };
    let error = gate
        .evaluate(&InstantiationInput { request: &request, ceiling: &ceiling }, NOW)
        .expect_err("no default fuel budget at the door");
    assert_eq!(error.name(), "LIMITS_INVALID");
    // Every door refusal audited.
    assert!(sink
        .events()
        .iter()
        .filter(|e| e.kind == EnforcementAuditKind::InstantiationRefused)
        .count()
        >= 4);
}

#[test]
fn the_enforcement_contract_refusal_codes_are_named_and_distinct() {
    // The host's enforcement-contract codes (a limit exceeded => named
    // error + the instance torn down, the host process survives):
    let codes = [
        HostError::MemoryLimitExceeded { limit_pages: 4096, requested_pages: 4097 }.name(),
        HostError::DeadlineExceeded { deadline_ms: 5_000 }.name(),
        HostError::FuelExhausted.name(),
        HostError::OutputLimitExceeded { limit_bytes: 1024 }.name(),
        HostError::CallRateExceeded { limit_per_minute: 60 }.name(),
        HostError::LimitsInvalid { field: "f".to_string(), reason: "r".to_string() }.name(),
    ];
    let mut sorted = codes.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), codes.len(), "the limit refusal codes are DISTINCT: {sorted:?}");
    assert_eq!(sorted, vec![
        "CALL_RATE_EXCEEDED",
        "DEADLINE_EXCEEDED",
        "FUEL_EXHAUSTED",
        "LIMITS_INVALID",
        "MEMORY_LIMIT_EXCEEDED",
        "OUTPUT_LIMIT_EXCEEDED",
    ]);
}

#[test]
fn a_verdict_cannot_be_forged_outside_the_crate_the_only_issuer_is_evaluate() {
    // The unforgeable-verdict property: `EnforcementVerdict` has no public
    // constructor and private fields — the ONLY way to obtain one is
    // `EnforcementGate::evaluate`. This test documents the property by
    // obtaining the verdict legitimately and verifying its derivation trail;
    // the compile-level half (a struct literal over private fields is a
    // compile error) is enforced by the compiler, not by a test.
    let gate = gate();
    let request = request("plugin.alpha", &["telemetry.read"], WorldId::TelemetrySourcePlugin);
    let ceiling = ProfileCeiling::new(vec![ceiling_entry("telemetry.read", data_scope("tenant-1"))]);
    let verdict = gate
        .evaluate(&InstantiationInput { request: &request, ceiling: &ceiling }, NOW)
        .expect("issued by evaluate");
    // The derivation trail is carried on the verdict (manifest id/version
    // — who granted what, recorded).
    assert_eq!(verdict.manifest_id(), "plugin.alpha");
    assert_eq!(verdict.manifest_version(), "1.0.0");
    assert_eq!(verdict.plugin_id(), "plugin.alpha");
    assert_eq!(verdict.evaluated_at_unix_ms(), NOW);
    assert_eq!(verdict.world(), WorldId::TelemetrySourcePlugin);
    let _unused: fn(&EnforcementVerdict) -> &str = EnforcementVerdict::plugin_id;
}
