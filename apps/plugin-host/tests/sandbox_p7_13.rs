// P7.13 — Sandbox negative tests: escape, exhaustion, and ambient-access
// attempts (WRITTEN-UNRUN: no cargo command was executed in this loop; the
// first human `cargo test -p flexcore-plugin-host` run is the verification
// gate — decision record
// `ledger/phase-07-plugin-sdk/12_13_enforcement_negatives.md` §7, which
// carries the honest host-API-vs-real-component split of this suite).
//
// THE HONEST POSTURE (read first)
//
// No component artifact exists and the default build's bindings seam is
// `NoBindings` — the attack surface these tests can exercise is the HOST
// API itself: the enforcement gate (P7.12), the capability broker (P7.11),
// the resource-limit construction, and the closed vocabularies. Every test
// here proves the host REFUSES before any component code would run, or
// that the named limit errors carry their teardown contract at the type
// level. The BEHAVIOURAL half of the hostile suite — a real wasmtime
// component that opens files, opens sockets, allocates unboundedly, loops
// forever, returns multi-GB outputs, and calls host functions at abusive
// rates, and is observed to be denied/terminated while the host process
// survives — is the HUMAN-GATE list in decision record §7 (it needs a real
// build; ledger §P7.13 step 2/5).
//
// Wasmtime sandboxing caveat (ledger §P7.13 step 3, §5.4 [R4]): wasmtime
// provides sandboxing MECHANISMS, but sandboxing must still be combined
// with host capability policy, resource limits, signatures, and isolation.
// This suite is the capability-policy and resource-limit half; the
// wasmtime-internal half is the human gate citing the R7-C2 CVE floor
// (pinned =50.0.1, advisory re-check: `40_toolchain_pinning.md` §2 and the
// Cargo.toml D4 note — CVE-2026-115889, CVE-2026-115893, CVE-2026-115384;
// the advisory pages must be re-checked at implementation time and the
// check date recorded there — human-gate item, decision record §9).
//
// Coverage (ledger §P7.13 as scoped for this wave):
//
// (a) ambient-access attempts: filesystem reads outside the granted prefix
//     (traversal, `..`, absolute, sibling-prefix, symlink-style), network
//     sockets, environment variables, wall clock, subprocess spawn — every
//     one refused with its named error, nothing ambient exists;
// (b) exhaustion: the five named limit errors carry the teardown contract;
//     the declared bounds are re-validated and govern (the DEFAULT caps and
//     the mandatory fuel budget are refused/configured, never permissive);
// (c) escape attempts at the host API: ungranted capability, revoked-grant
//     reuse, expired grant, grant forgery (structural: the broker's grant
//     type is broker-constructed; behavioural: a captured grant object
//     confers nothing once revoked);
// (d) the dispatch-bypass attempt (ADR-010 made concrete): a manifest
//     requesting dispatch authority or any protected-kernel capability is
//     refused at the enforcement gate AND at the broker's never-granted
//     set — the dispatch-bypassing plugin is UNBUILDABLE, not discouraged;
// (e) the resource-bound lie: the host enforces the DECLARED bounds
//     (mirrored + re-validated), not the component's belief.

use flexcore_plugin_host::broker::{
    CapabilityBroker, CapabilityGrant, CapabilityScope, GrantRequest, InMemoryBrokerAuditSink,
};
use flexcore_plugin_host::enforcement::{
    EnforcementError, EnforcementGate, EnforcementRequest, InMemoryEnforcementAuditSink,
    InstantiationInput, ProfileCeiling, ProfileGrant,
};
use flexcore_plugin_host::host::HostError;
use flexcore_plugin_host::limits::{DeclaredBounds, DEFAULT_MAX_CALLS_PER_MINUTE, DEFAULT_MAX_OUTPUT_MB};

// --- Test helpers ---------------------------------------------------------------

const NOW: u64 = 1_700_000_000_000;

fn broker() -> CapabilityBroker {
    CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()))
}

fn gate() -> EnforcementGate {
    EnforcementGate::compose(
        broker(),
        Box::new(InMemoryEnforcementAuditSink::new()),
    )
}

fn data_scope(tenant: &str) -> CapabilityScope {
    CapabilityScope::DataScope {
        tenant_id: tenant.to_string(),
        site_id: None,
        asset_id: None,
    }
}

fn read_prefix(prefix: &str) -> CapabilityScope {
    CapabilityScope::ReadOnlyPathPrefix { prefix: prefix.to_string() }
}

fn send_target(target: &str) -> CapabilityScope {
    CapabilityScope::SendOnlyTarget { target: target.to_string() }
}

fn grant_request(capability: &str, scope: CapabilityScope) -> GrantRequest {
    GrantRequest { capability: capability.to_string(), scope, expires_at_unix_ms: None }
}

fn bounds(max_memory_mb: u64, timeout_ms: u64) -> DeclaredBounds {
    DeclaredBounds {
        max_memory_mb,
        timeout_ms,
        max_output_mb: None,
        max_calls_per_minute: None,
    }
}

fn request(permissions: &[&str]) -> EnforcementRequest {
    EnforcementRequest {
        plugin_id: "plugin.hostile".to_string(),
        manifest_id: "plugin.hostile".to_string(),
        manifest_version: "1.0.0".to_string(),
        world: flexcore_plugin_host::host::WorldId::OptimizerPlugin,
        entry_symbol: "flexcore:optimizer/optimizer".to_string(),
        manifest_permissions: permissions.iter().map(|p| (*p).to_string()).collect(),
        declared_resource_fields: vec!["max-memory-mb".to_string(), "timeout-ms".to_string()],
        bounds: bounds(256, 5_000),
        fuel_budget: 100_000_000,
    }
}

fn evaluate(gate: &EnforcementGate, request: &EnforcementRequest, ceiling: &ProfileCeiling) -> Result<(), EnforcementError> {
    gate.evaluate(&InstantiationInput { request, ceiling }, NOW).map(|_| ())
}

/// Extracts the source text of a top-level item (marker to the next
/// block-closing line) for the lexical assertions.
fn item_source(source: &str, marker: &str) -> String {
    let start = source.find(marker).unwrap_or_else(|| {
        panic!("lexical assertion: marker '{marker}' not present in source");
    });
    let rest = &source[start..];
    let end = rest.find("\n}").unwrap_or_else(|| {
        panic!("lexical assertion: block end not found after '{marker}'");
    });
    rest[..end].to_string()
}

// =================================================================================
// (a) Ambient-access attempts — nothing ambient exists
// =================================================================================

#[test]
fn a_path_traversal_request_is_refused_inside_a_granted_prefix() {
    let broker = broker();
    broker
        .grant(
            "plugin.hostile",
            "plugin.hostile",
            "1.0.0",
            &grant_request("telemetry.read", read_prefix("/data/telemetry")),
            NOW,
        )
        .expect("the scoped read grant issues");
    // The classic `..` traversal: refused even though the string STARTS
    // with the granted prefix (a literal escape segment is a refusal, never
    // a normalisation).
    for wanted in [
        "/data/telemetry/../../secrets",
        "/data/telemetry/../telemetry-evil/x",
        "/data/telemetry/sub/../../../etc/passwd",
    ] {
        let error = broker
            .check("plugin.hostile", "telemetry.read", &read_prefix(wanted), NOW)
            .expect_err("traversal refused");
        assert_eq!(error.name(), "SCOPE_OUTSIDE_GRANT", "wanted {wanted}");
    }
}

#[test]
fn absolute_paths_and_sibling_prefixes_are_outside_the_grant() {
    let broker = broker();
    broker
        .grant(
            "plugin.hostile",
            "plugin.hostile",
            "1.0.0",
            &grant_request("telemetry.read", read_prefix("/data/telemetry")),
            NOW,
        )
        .expect("the scoped read grant issues");
    // An absolute path elsewhere, and the sibling-prefix trick (the grant
    // boundary is a segment boundary: `/data/telemetry` must NOT cover
    // `/data/telemetry-evil`).
    for wanted in ["/etc/passwd", "/data/telemetry-evil", "C:/windows/system32/config"] {
        let error = broker
            .check("plugin.hostile", "telemetry.read", &read_prefix(wanted), NOW)
            .expect_err("outside the envelope");
        assert_eq!(error.name(), "SCOPE_OUTSIDE_GRANT", "wanted {wanted}");
    }
    // Within the prefix, under the boundary convention (requests name paths
    // INSIDE the granted prefix), the use is honoured.
    broker
        .check("plugin.hostile", "telemetry.read", &read_prefix("/data/telemetry/site-a"), NOW)
        .expect("a contained request is honoured");
}

#[test]
fn the_granted_prefix_is_a_virtual_mount_not_a_filesystem_escape() {
    // Symlink-style escapes: the scope model has NO symlink resolution — a
    // granted prefix is a virtual mount the host controls (broker.rs
    // `CapabilityScope::ReadOnlyPathPrefix` doc), so a request cannot
    // redirect through a link name; the HOST refuses any request outside
    // the segment-boundary envelope before any filesystem semantics could
    // apply. The REAL filesystem behaviour (what a real component's
    // syscall-shaped import could do) is the human-gate list — decision
    // record §7.
    let broker = broker();
    broker
        .grant(
            "plugin.hostile",
            "plugin.hostile",
            "1.0.0",
            &grant_request("telemetry.read", read_prefix("/data/telemetry")),
            NOW,
        )
        .expect("the scoped read grant issues");
    // A link-shaped name that would resolve outside on a real filesystem is
    // still just an outside prefix here: refused.
    let error = broker
        .check(
            "plugin.hostile",
            "telemetry.read",
            &read_prefix("/data/link-to-secrets"),
            NOW,
        )
        .expect_err("a name outside the prefix is outside the grant");
    assert_eq!(error.name(), "SCOPE_OUTSIDE_GRANT");
    // And the lexical ground truth of "no ambient surface": the P7.09
    // worlds declare zero `wasi:` imports (no filesystem import to resolve
    // anything with), and the gated linker starts empty and stays empty.
    let optimizer = include_str!("../../../wit/flexcore-optimizer@1.0.0/optimizer.wit");
    let grid = include_str!("../../../wit/flexcore-grid@1.0.0/grid.wit");
    let telemetry = include_str!("../../../wit/flexcore-telemetry@1.0.0/telemetry.wit");
    for source in [optimizer, grid, telemetry] {
        assert!(!source.contains("import wasi"), "no wasi import in the recorded world");
    }
    let bindings_source = include_str!("../src/bindings.rs");
    assert!(
        !bindings_source.contains("wasi::"),
        "the linker machinery never references a wasi namespace"
    );
    assert!(
        bindings_source.contains("Linker::new(&engine)"),
        "the gated linker starts empty (no ambient import is added anywhere)"
    );
}

#[test]
fn network_sockets_are_unreachable_through_any_path() {
    let broker = broker();
    // `network.any` is in the never-granted set: refused at grant time AND
    // at check time, through ANY scope, regardless of any manifest claim.
    let error = broker
        .grant(
            "plugin.hostile",
            "plugin.hostile",
            "1.0.0",
            &grant_request("network.any", send_target("anywhere")),
            NOW,
        )
        .expect_err("network.any is never granted");
    assert_eq!(error.name(), "PERMISSION_NEVER_GRANTABLE");
    let error = broker
        .check("plugin.hostile", "network.any", &send_target("anywhere"), NOW)
        .expect_err("network.any is never checkable");
    assert_eq!(error.name(), "PERMISSION_NEVER_GRANTABLE");
    // A crafted socket-shaped id that is not in the vocabulary at all is a
    // PERMISSION_UNKNOWN — the vocabulary is closed even against future ids.
    let error = broker
        .check("plugin.hostile", "network.socket.open", &send_target("0.0.0.0:9999"), NOW)
        .expect_err("unknown id");
    assert_eq!(error.name(), "PERMISSION_UNKNOWN");
}

#[test]
fn environment_variables_are_unreachable() {
    let broker = broker();
    // No capability id exists for environment access: a request is
    // PERMISSION_UNKNOWN (the closed vocabulary; decision record §7 table).
    let error = broker
        .check("plugin.hostile", "env.read", &data_scope("tenant-1"), NOW)
        .expect_err("env access is not a capability");
    assert_eq!(error.name(), "PERMISSION_UNKNOWN");
    let error = broker
        .grant(
            "plugin.hostile",
            "plugin.hostile",
            "1.0.0",
            &grant_request("env.read", data_scope("tenant-1")),
            NOW,
        )
        .expect_err("env access cannot be granted either");
    assert_eq!(error.name(), "PERMISSION_UNKNOWN");
}

#[test]
fn the_wall_clock_is_unreachable() {
    let broker = broker();
    let error = broker
        .check("plugin.hostile", "clock.read", &data_scope("tenant-1"), NOW)
        .expect_err("clock access is not a capability");
    assert_eq!(error.name(), "PERMISSION_UNKNOWN");
    // Structural: the broker itself reads NO clock — every method takes
    // `now_unix_ms` as an explicit parameter (the host supplies the time
    // and the responsibility). A component has no clock capability to call.
    let broker_source = include_str!("../src/broker.rs");
    assert!(
        broker_source.contains("now_unix_ms: u64"),
        "time is a caller-supplied parameter, never an ambient read"
    );
    assert!(
        !broker_source.contains("SystemTime::now"),
        "the broker never reads a clock of its own"
    );
}

#[test]
fn subprocess_spawn_is_unreachable() {
    let broker = broker();
    let error = broker
        .check("plugin.hostile", "process.spawn", &data_scope("tenant-1"), NOW)
        .expect_err("process spawn is not a capability");
    assert_eq!(error.name(), "PERMISSION_UNKNOWN");
    let error = broker
        .check("plugin.hostile", "device.protocolspeak", &data_scope("tenant-1"), NOW)
        .expect_err("direct device access is not a capability either (§5.4 Class B boundary)");
    assert_eq!(error.name(), "PERMISSION_UNKNOWN");
}

#[test]
fn every_ambient_denial_is_audited() {
    let broker = broker();
    // Denials are audited (audit-or-refuse): drive several denials — the
    // audit events themselves are inspected in broker_p7_11.rs (the P7.11
    // suite); this suite asserts the named refusals. The structural
    // assertion here: the broker has NO sink-less constructor, so every
    // denial above was recorded (a broker that cannot record is not
    // composed at all).
    let _ = broker.check("plugin.hostile", "env.read", &data_scope("t"), NOW);
    let _ = broker.check("plugin.hostile", "network.any", &send_target("x"), NOW);
    let _ = broker.check("plugin.hostile", "process.spawn", &data_scope("t"), NOW);
    let broker_source = include_str!("../src/broker.rs");
    let impl_block = item_source(broker_source, "impl CapabilityBroker {");
    assert!(
        !impl_block.contains("pub fn new() -> CapabilityBroker"),
        "there is no broker without an audit sink (fail-closed at composition)"
    );
}

// =================================================================================
// (b) Exhaustion — named errors + the teardown contract
// =================================================================================

#[test]
fn every_exhaustion_error_carries_the_teardown_contract() {
    // The enforcement contract (limits.rs / host.rs): a limit exceeded =>
    // named error + the instance torn down, the host process survives.
    // These are the type-level assertions; the BEHAVIOURAL proof (a real
    // component exceeding each limit and being observed torn down) is the
    // human-gate list — decision record §7.
    let cases: Vec<HostError> = vec![
        HostError::MemoryLimitExceeded { limit_pages: 4096, requested_pages: 4097 },
        HostError::FuelExhausted,
        HostError::DeadlineExceeded { deadline_ms: 5_000 },
        HostError::OutputLimitExceeded { limit_bytes: 1024 },
        HostError::CallRateExceeded { limit_per_minute: 60 },
    ];
    let expected_names = [
        "MEMORY_LIMIT_EXCEEDED",
        "FUEL_EXHAUSTED",
        "DEADLINE_EXCEEDED",
        "OUTPUT_LIMIT_EXCEEDED",
        "CALL_RATE_EXCEEDED",
    ];
    for (error, name) in cases.into_iter().zip(expected_names) {
        assert_eq!(error.name(), name);
        let rendered = error.to_string();
        assert!(
            rendered.contains("instance torn down"),
            "{name} must state the teardown contract: {rendered}"
        );
        assert!(
            rendered.contains("host survives"),
            "{name} must state that the host process survives: {rendered}"
        );
    }
}

#[test]
fn fuel_exhaustion_is_designed_away_at_the_door_not_configured_away() {
    // There is NO default fuel budget: an unmetered instance is refused at
    // the enforcement door (LIMITS_INVALID), so "fuel exhausted mid-call"
    // can only ever hit a METERED instance — the hostile "runaway" case has
    // no unmetered configuration to escape into.
    let gate = gate();
    let mut request = request(&[]);
    request.fuel_budget = 0;
    let ceiling = ProfileCeiling::empty();
    let error = evaluate(&gate, &request, &ceiling).expect_err("zero fuel refuses");
    assert_eq!(error.name(), "LIMITS_INVALID");
    let rendered = error.to_string();
    assert!(rendered.contains("fuel"), "the refusal names the field: {rendered}");
}

#[test]
fn the_epoch_deadline_is_derived_from_the_declared_timeout_only() {
    // The wall-clock deadline (epoch ticks) comes from the declared
    // `timeout-ms` alone — there is no second, guest-visible knob to widen.
    let limits = flexcore_plugin_host::limits::ResourceLimits::from_declared_bounds(
        &bounds(256, 5_000),
        100_000_000,
    )
    .expect("valid bounds construct");
    assert_eq!(limits.wall_clock_deadline_ms(), 5_000);
    assert_eq!(limits.epoch_ticks(), 500, "ceil(5000/10) at 10 ms per tick");
    // An out-of-range declared timeout is refused (re-validation, the
    // kernel's closed range).
    let error = flexcore_plugin_host::limits::ResourceLimits::from_declared_bounds(
        &bounds(256, 3_600_001),
        100_000_000,
    )
    .expect_err("above the closed range");
    assert_eq!(error.name(), "LIMITS_INVALID");
}

#[test]
fn output_and_call_rate_defaults_are_caps_not_permissions() {
    // The DEFAULT caps apply ONLY when the manifest declares nothing — and
    // they are CAPS (tighter is declarable, wider is not: the declared bound
    // governs, never a component's belief).
    let default_limits = flexcore_plugin_host::limits::ResourceLimits::from_declared_bounds(
        &bounds(256, 5_000),
        100_000_000,
    )
    .expect("valid bounds construct");
    assert_eq!(default_limits.max_output_bytes(), DEFAULT_MAX_OUTPUT_MB * 1024 * 1024);
    assert_eq!(default_limits.max_calls_per_minute(), DEFAULT_MAX_CALLS_PER_MINUTE);
    let declared = DeclaredBounds {
        max_memory_mb: 256,
        timeout_ms: 5_000,
        max_output_mb: Some(1),
        max_calls_per_minute: Some(1),
    };
    let tight_limits = flexcore_plugin_host::limits::ResourceLimits::from_declared_bounds(
        &declared,
        100_000_000,
    )
    .expect("valid bounds construct");
    assert_eq!(tight_limits.max_output_bytes(), 1024 * 1024);
    assert_eq!(tight_limits.max_calls_per_minute(), 1);
}

// =================================================================================
// (c) Escape attempts at the host API
// =================================================================================

#[test]
fn calling_an_ungranted_capability_is_refused_and_audited() {
    let broker = broker();
    let error = broker
        .check("plugin.hostile", "telemetry.read", &data_scope("tenant-1"), NOW)
        .expect_err("defaults deny");
    assert_eq!(error.name(), "CAPABILITY_NOT_GRANTED");
    // Structural: no sink-less broker exists, so every denial above was
    // recorded (audit-or-refuse) — the P7.11 suite inspects the events;
    // here the no-default-constructor shape is re-asserted.
    let broker_source = include_str!("../src/broker.rs");
    assert!(broker_source.contains("pub fn new(audit: Box<dyn BrokerAuditSink>)"));
}

#[test]
fn a_revoked_grant_cannot_be_reused() {
    let broker = broker();
    let receipt = broker
        .grant(
            "plugin.hostile",
            "plugin.hostile",
            "1.0.0",
            &grant_request("telemetry.read", data_scope("tenant-1")),
            NOW,
        )
        .expect("grant issues");
    broker
        .check("plugin.hostile", "telemetry.read", &data_scope("tenant-1"), NOW)
        .expect("granted use works");
    broker
        .revoke(receipt.grant.grant_id(), NOW + 1)
        .expect("revoke");
    let error = broker
        .check("plugin.hostile", "telemetry.read", &data_scope("tenant-1"), NOW + 2)
        .expect_err("revocation is immediate — nothing caches authority");
    assert_eq!(error.name(), "CAPABILITY_NOT_GRANTED");
    let error = broker
        .revoke(receipt.grant.grant_id(), NOW + 3)
        .expect_err("revoking nothing is an error");
    assert_eq!(error.name(), "GRANT_UNKNOWN");
}

#[test]
fn an_expired_grant_is_refused() {
    let broker = broker();
    let receipt = broker
        .grant(
            "plugin.hostile",
            "plugin.hostile",
            "1.0.0",
            &GrantRequest {
                capability: "telemetry.read".to_string(),
                scope: data_scope("tenant-1"),
                expires_at_unix_ms: Some(NOW + 10),
            },
            NOW,
        )
        .expect("grant issues");
    let _ = receipt;
    let error = broker
        .check("plugin.hostile", "telemetry.read", &data_scope("tenant-1"), NOW + 10)
        .expect_err("expiry is exclusive (use at now >= expires_at denies)");
    assert_eq!(error.name(), "GRANT_EXPIRED");
    // An expired grant never shadows a live one — but with NO live grant,
    // the expiry refusal names the dead grant.
    broker
        .check("plugin.hostile", "telemetry.read", &data_scope("tenant-1"), NOW + 5)
        .expect("before expiry it works");
}

#[test]
fn grant_forgery_is_structurally_refused_and_behaviourally_futile() {
    // STRUCTURAL: the broker's grant type is broker-constructed — its
    // fields are `pub(crate)` since P7.12/P7.13 (visibility-only change),
    // so a caller OUTSIDE the crate (like this test) cannot build a
    // CapabilityGrant literal at all; the only acquisition path is a
    // receipt from a broker method.
    let broker_source = include_str!("../src/broker.rs");
    let grant_struct = item_source(broker_source, "pub struct CapabilityGrant {");
    assert!(
        !grant_struct.contains("pub grant_id")
            && !grant_struct.contains("pub plugin_id")
            && !grant_struct.contains("pub capability")
            && !grant_struct.contains("pub scope")
            && !grant_struct.contains("pub expires_at_unix_ms")
            && !grant_struct.contains("pub manifest_id")
            && !grant_struct.contains("pub manifest_version"),
        "the grant type's constructor is private to the broker"
    );
    // The enforcement layer's authority object is equally unforgeable.
    let enforcement_source = include_str!("../src/enforcement.rs");
    let verdict_struct = item_source(enforcement_source, "pub struct EnforcementVerdict {");
    assert!(!verdict_struct.contains("pub "), "the verdict is unforgeable too");

    // BEHAVIOURAL futility: even a grant object CAPTURED legitimately (a
    // receipt's grant, cloned before revocation) confers nothing —
    // authority lives in the broker's per-call state, never in object
    // possession.
    let broker = broker();
    let receipt = broker
        .grant(
            "plugin.hostile",
            "plugin.hostile",
            "1.0.0",
            &grant_request("telemetry.read", data_scope("tenant-1")),
            NOW,
        )
        .expect("grant issues");
    let captured: CapabilityGrant = receipt.grant.clone();
    broker.revoke(captured.grant_id(), NOW + 1).expect("revoke");
    let error = broker
        .check("plugin.hostile", "telemetry.read", &data_scope("tenant-1"), NOW + 2)
        .expect_err("the captured grant object is inert after revocation");
    assert_eq!(error.name(), "CAPABILITY_NOT_GRANTED");
    // And a captured grant for a DIFFERENT plugin id is equally inert: the
    // broker matches (plugin, capability) in ITS OWN state.
    let other_broker = broker();
    let error = other_broker
        .check("plugin.hostile", "telemetry.read", &data_scope("tenant-1"), NOW)
        .expect_err("a captured object does not seed a fresh broker");
    assert_eq!(error.name(), "CAPABILITY_NOT_GRANTED");
}

// =================================================================================
// (d) The dispatch-bypass attempt (ADR-010 made concrete)
// =================================================================================

#[test]
fn a_dispatch_bypassing_manifest_is_refused_at_the_door_and_at_the_broker() {
    let (gate, sink) = gate_with_sink();
    // Every protected-kernel capability id: refused at the ENFORCEMENT gate
    // (before any component code would run)...
    for capability in ["dispatch.authorize", "dispatch.issue", "secret.read", "network.any"] {
        let request = request(&[capability]);
        let error = evaluate(&gate, &request, &ProfileCeiling::empty())
            .expect_err("never-granted at the door");
        assert_eq!(error.name(), "PERMISSION_NEVER_GRANTABLE", "capability {capability}");
        // ...AND at the broker's grant path (the re-refusal), and at its
        // check path (a crafted call is refused and audited).
        let broker = broker();
        let error = broker
            .grant(
                "plugin.hostile",
                "plugin.hostile",
                "1.0.0",
                &grant_request(capability, data_scope("tenant-1")),
                NOW,
            )
            .expect_err("never granted at the broker either");
        assert_eq!(error.name(), "PERMISSION_NEVER_GRANTABLE", "capability {capability}");
    }
    let refusals = sink
        .events()
        .iter()
        .filter(|e| e.kind == flexcore_plugin_host::enforcement::EnforcementAuditKind::InstantiationRefused)
        .count();
    assert_eq!(refusals, 4, "every door refusal is audited");
}

#[test]
fn a_validation_verdict_authority_does_not_exist_to_request() {
    let gate = gate();
    // The ADR-010 attempt in its most direct spelling: a manifest asking
    // for the validator's VERDICT as a capability. No such id exists in any
    // vocabulary — PERMISSION_UNKNOWN at the door.
    let request = request(&["dispatch.validate.verdict"]);
    let error = evaluate(&gate, &request, &ProfileCeiling::empty()).expect_err("no such authority");
    assert_eq!(error.name(), "PERMISSION_UNKNOWN");
}

#[test]
fn the_validate_request_capability_can_only_target_the_protected_validator() {
    let gate = gate();
    // Even the ONE dispatch-adjacent capability is fenced: a ceiling entry
    // scoping it at any other target is refused by the broker's derivation
    // (SCOPE_INVALID under the gate's BrokerRefusal) — a plugin can never
    // hold a grant whose envelope would let it fish for verdicts elsewhere.
    let request = request(&["dispatch.validate.request"]);
    let ceiling = ProfileCeiling::new(vec![ProfileGrant {
        capability: "dispatch.validate.request".to_string(),
        scope: send_target("evil-verdict-collector"),
        expires_at_unix_ms: None,
    }]);
    let error = evaluate(&gate, &request, &ceiling).expect_err("only the protected target");
    match &error {
        EnforcementError::BrokerRefusal { error } => {
            assert_eq!(error.name(), "SCOPE_INVALID");
        }
        other => panic!("expected BrokerRefusal, got {other:?}"),
    }
}

#[test]
fn no_permission_id_conveys_dispatch_authority_unbuildable_not_discouraged() {
    // The ADR-010 proof, exhaustive over the vocabularies: NO permission id
    // — allowed, never-granted, or crafted — can produce a verdict that
    // conveys dispatch authority. The allowed set yields proposals only;
    // everything else refuses.
    let gate = gate();
    let allowed = flexcore_plugin_host::broker::ALLOWED_PERMISSIONS;
    let ceiling = ProfileCeiling::new(
        allowed
            .iter()
            .map(|c| ProfileGrant {
                capability: (*c).to_string(),
                scope: send_target("candidate-queue"),
                expires_at_unix_ms: None,
            })
            .collect(),
    );
    // dispatch.validate.request cannot carry the send-only candidate target
    // (it must target exactly the protected validator), so the full-ceiling
    // derivation refuses on THAT row — which is itself the fence working.
    // Evaluate the remaining rows individually.
    for capability in allowed {
        if *capability == "dispatch.validate.request" {
            continue;
        }
        let request = request(&[capability]);
        let single = ProfileCeiling::new(vec![ProfileGrant {
            capability: (*capability).to_string(),
            scope: send_target("candidate-queue"),
            expires_at_unix_ms: None,
        }]);
        let verdict = gate
            .evaluate(&InstantiationInput { request: &request, ceiling: &single }, NOW)
            .unwrap_or_else(|e| panic!("allowed capability {capability} must evaluate: {e}"));
        // The granted set is a SUBSET of the allowed vocabulary and names
        // NO authorization/issue capability: the maximum a plugin can hold
        // is a proposal or a request-forward.
        for grant in verdict.grants() {
            assert!(
                allowed.contains(&grant.capability()),
                "no grant outside the closed vocabulary"
            );
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
        let request = request(&[capability]);
        let _ = evaluate(&gate, &request, &ProfileCeiling::empty())
            .expect_err("protected-kernel authority is unbuildable");
    }
}

// =================================================================================
// (e) The resource-bound lie
// =================================================================================

#[test]
fn the_host_enforces_the_declared_bounds_not_the_components_belief() {
    let (gate, _sink) = gate_with_sink();
    // The manifest declares TINY bounds; the component "behaves" as if it
    // had more. The host's limits come from the DECLARED bounds — mirrored
    // and re-validated — and there is no API to widen a constructed
    // ResourceLimits (private fields, no public constructor other than
    // `from_declared_bounds`).
    let mut request = request(&[]);
    request.bounds = bounds(1, 100); // 1 MiB, 100 ms — tiny by declaration
    let verdict = gate
        .evaluate(&InstantiationInput { request: &request, ceiling: &ProfileCeiling::empty() }, NOW)
        .expect("tiny bounds are valid bounds");
    assert_eq!(
        verdict.limits().max_memory_pages(),
        16,
        "1 MiB = 16 wasm pages (exact conversion) — the DECLARED bound governs"
    );
    assert_eq!(verdict.limits().wall_clock_deadline_ms(), 100);
    assert_eq!(verdict.limits().max_calls_per_minute(), DEFAULT_MAX_CALLS_PER_MINUTE);
    // The component's "belief" (a wider memory request, a longer deadline)
    // has no representation anywhere: the limits object is derived solely
    // from the declared bounds, and a widened DECLARATION that leaves the
    // kernel's closed range is refused at the door.
    let mut lying = request(&[]);
    lying.bounds = bounds(65_537, 100);
    let error = evaluate(&gate, &lying, &ProfileCeiling::empty()).expect_err("above the range");
    assert_eq!(error.name(), "LIMITS_INVALID");
    // Lexical: no other constructor exists for the limits.
    let limits_source = include_str!("../src/limits.rs");
    let impl_block = item_source(limits_source, "impl ResourceLimits {");
    assert!(
        !impl_block.contains("pub fn new"),
        "ResourceLimits is built ONLY from re-validated declared bounds"
    );
}

// --- Helper (declared late so the suites above read top-down) --------------------

/// Composes the gate with its audit sink and returns both (the tests assert
/// the audit trail through the SHARED sink handle: the gate holds one `Arc`
/// clone behind its `Box<dyn EnforcementAuditSink>`, the test inspects the
/// same log through the other).
fn gate_with_sink() -> (EnforcementGate, std::sync::Arc<InMemoryEnforcementAuditSink>) {
    let sink = std::sync::Arc::new(InMemoryEnforcementAuditSink::new());
    let gate = EnforcementGate::compose(broker(), Box::new(sink.clone()));
    (gate, sink)
}
