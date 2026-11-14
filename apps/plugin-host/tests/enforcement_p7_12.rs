// P7.12 — Permission-enforcement tests (WRITTEN-UNRUN: no cargo command was
// executed in this loop; the first human `cargo test -p flexcore-plugin-host`
// run is the verification gate — decision record
// `ledger/phase-07-plugin-sdk/12_13_enforcement_negatives.md`).
//
// Coverage (ledger §P7.12 as scoped for this wave; the decision record
// carries the honest host-API-vs-real-component split):
//
// - manifest permissions that match the profile ceiling and the broker's
//   scopes → the verdict constructs with the EXACT granted scope, and the
//   instantiation flows through the NoBindings fail-closed seam (the test
//   asserts the ENFORCEMENT VERDICT, not real execution — there are no
//   components and no bindings in this loop);
// - a never-granted permission → refusal BEFORE instantiation, audited,
//   and the broker still holds nothing;
// - an over-broad manifest (declares above the profile ceiling) → refusal,
//   never trimmed to fit; a ceiling entry the manifest does not declare →
//   not granted (the intersection, silently bounded);
// - vocabulary drift (`timeoutMs`, fuel/concurrency field names, duplicate
//   field) → the named `PERMISSION_VOCABULARY_DRIFT` refusal, never a
//   default interpretation;
// - the capability set is FROZEN at instantiation: a mutated manifest
//   object does not expand an already-issued verdict; the broker's
//   revocation tightens per-call authority while the frozen set stands;
// - every allow/refuse decision is audited (allow with the exact granted
//   scope, refuse with the named reason, plus the freeze record), and a
//   broken audit sink refuses the decision in BOTH directions;
// - the candidate-only rule (§13.4): proposal capabilities confer no
//   authority — the broker exposes no path from a proposal to an
//   authorization or an issued command;
// - NO raw-instantiation path exists (lexical/API-shape assertions over the
//   crate sources: `bind_component` is `pub(crate)`, the verdict is
//   unforgeable, the broker's grant type is broker-constructed).

use flexcore_plugin_host::bindings::PluginBinding;
use flexcore_plugin_host::broker::CapabilityScope;
use flexcore_plugin_host::enforcement::{
    instantiate, EnforcementAuditEvent, EnforcementAuditKind, EnforcementAuditSink,
    EnforcementError, EnforcementGate, EnforcementRequest, EnforcementVerdict,
    InMemoryEnforcementAuditSink, InstantiationInput, ProfileCeiling, ProfileGrant,
};
use flexcore_plugin_host::host::{ComponentArtifact, WorldId};
use flexcore_plugin_host::limits::DeclaredBounds;

// --- Test helpers ---------------------------------------------------------------

const NOW: u64 = 1_700_000_000_000;

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

fn request(plugin_id: &str, permissions: &[&str]) -> EnforcementRequest {
    EnforcementRequest {
        plugin_id: plugin_id.to_string(),
        manifest_id: plugin_id.to_string(),
        manifest_version: "1.0.0".to_string(),
        world: WorldId::OptimizerPlugin,
        entry_symbol: "flexcore:optimizer/optimizer".to_string(),
        manifest_permissions: permissions.iter().map(|p| (*p).to_string()).collect(),
        declared_resource_fields: declared_fields(),
        bounds: bounds(),
        fuel_budget: 100_000_000,
    }
}

fn ceiling_entry(capability: &str, scope: CapabilityScope) -> ProfileGrant {
    ProfileGrant {
        capability: capability.to_string(),
        scope,
        expires_at_unix_ms: None,
    }
}

fn data_scope(tenant: &str) -> CapabilityScope {
    CapabilityScope::DataScope {
        tenant_id: tenant.to_string(),
        site_id: None,
        asset_id: None,
    }
}

/// Composes the gate with its audit sink and returns both (the tests assert
/// the audit trail through the SHARED sink handle: the gate holds one `Arc`
/// clone behind its `Box<dyn EnforcementAuditSink>`, the test inspects the
/// same log through the other).
fn gate_with_audit() -> (EnforcementGate, std::sync::Arc<InMemoryEnforcementAuditSink>) {
    let sink = std::sync::Arc::new(InMemoryEnforcementAuditSink::new());
    let gate = EnforcementGate::compose(
        flexcore_plugin_host::broker::CapabilityBroker::new(Box::new(
            flexcore_plugin_host::broker::InMemoryBrokerAuditSink::new(),
        )),
        Box::new(sink.clone()),
    );
    (gate, sink)
}

/// An enforcement audit sink that ALWAYS fails (the audit-or-refuse
/// counter-case, both directions).
#[derive(Debug, Default)]
struct FailingEnforcementAuditSink;

impl EnforcementAuditSink for FailingEnforcementAuditSink {
    fn record(&self, _event: EnforcementAuditEvent) -> Result<(), EnforcementError> {
        Err(EnforcementError::AuditUnavailable {
            reason: "the test enforcement sink always fails".to_string(),
        })
    }
}

fn evaluate(gate: &EnforcementGate, request: &EnforcementRequest, ceiling: &ProfileCeiling) -> Result<EnforcementVerdict, EnforcementError> {
    gate.evaluate(&InstantiationInput { request, ceiling }, NOW)
}

/// Extracts the source text of a top-level item (from its marker to the
/// next line that closes the block) for the lexical/API-shape assertions.
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

// --- Matching permissions construct the verdict -----------------------------------

#[test]
fn matching_permissions_construct_a_verdict_with_the_exact_granted_scope() {
    let (gate, _sink) = gate_with_audit();
    let request = request("plugin.telemetry", &["telemetry.read", "schedule.propose"]);
    let ceiling = ProfileCeiling::new(vec![
        ceiling_entry("telemetry.read", data_scope("tenant-1")),
        ceiling_entry(
            "schedule.propose",
            CapabilityScope::SendOnlyTarget { target: "candidate-schedules".to_string() },
        ),
    ]);
    let verdict = evaluate(&gate, &request, &ceiling).expect("matching permissions construct");
    assert_eq!(verdict.plugin_id(), "plugin.telemetry");
    assert_eq!(verdict.manifest_id(), "plugin.telemetry");
    assert_eq!(verdict.manifest_version(), "1.0.0");
    assert_eq!(verdict.world(), WorldId::OptimizerPlugin);
    assert_eq!(verdict.entry_symbol(), "flexcore:optimizer/optimizer");
    assert_eq!(verdict.granted_capabilities(), vec!["telemetry.read", "schedule.propose"]);
    assert_eq!(
        verdict.granted_scope("telemetry.read"),
        Some(data_scope("tenant-1")),
        "the verdict carries the EXACT granted scope"
    );
    assert_eq!(
        verdict.granted_scope("schedule.propose"),
        Some(CapabilityScope::SendOnlyTarget { target: "candidate-schedules".to_string() })
    );
    assert_eq!(verdict.grants().len(), 2);
    assert_eq!(verdict.evaluated_at_unix_ms(), NOW);
    // The limits are the re-validated declared bounds (256 MiB = 4096 wasm
    // pages) — the host enforces the DECLARED bounds, not the component's
    // belief (§P7.13(e)).
    assert_eq!(verdict.limits().max_memory_pages(), 256 * 16);
}

#[test]
fn the_verdict_instantiates_through_the_no_bindings_fail_closed_seam() {
    // The test asserts the ENFORCEMENT VERDICT, not real execution: there
    // are no component artifacts and no generated bindings in this loop.
    // The instantiation path still proves its shape: the gate-issued
    // verdict reaches the bindings seam and the DEFAULT build fails closed
    // with the named `BINDINGS_NOT_GENERATED` error.
    let (gate, _sink) = gate_with_audit();
    let request = request("plugin.telemetry", &["telemetry.read"]);
    let ceiling = ProfileCeiling::new(vec![ceiling_entry("telemetry.read", data_scope("tenant-1"))]);
    let verdict = evaluate(&gate, &request, &ceiling).expect("verdict constructs");

    // A stub artifact (present, readable, not a real component) — the
    // binding object is produced without execution.
    let mut stub = std::env::temp_dir();
    stub.push(format!("enforcement-p7-12-{}.wasm", std::process::id()));
    let _ = std::fs::remove_file(&stub);
    std::fs::write(&stub, b"not a real component artifact").expect("test fixture write");
    let artifact = ComponentArtifact::load(&stub).expect("stub loads");

    let binding = instantiate(&verdict, &artifact).expect("the verdict is the only key needed");
    assert_eq!(binding.world(), WorldId::OptimizerPlugin);
    let error = binding.verify().expect_err("the default build cannot instantiate");
    assert_eq!(error.name(), "BINDINGS_NOT_GENERATED");
    let _ = std::fs::remove_file(&stub);
}

// --- Refusals at the door --------------------------------------------------------

#[test]
fn a_never_granted_permission_is_refused_before_instantiation() {
    let (gate, sink) = gate_with_audit();
    for permission in ["dispatch.authorize", "dispatch.issue", "secret.read", "network.any"] {
        let request = request("plugin.a", &[permission]);
        let ceiling = ProfileCeiling::empty();
        let error = evaluate(&gate, &request, &ceiling)
            .expect_err("never-granted ids are refused at the door");
        assert_eq!(error.name(), "PERMISSION_NEVER_GRANTABLE", "permission {permission}");
    }
    // The refusal is audited.
    let refusals = sink
        .events()
        .iter()
        .filter(|e| e.kind == EnforcementAuditKind::InstantiationRefused)
        .count();
    assert_eq!(refusals, 4, "every refusal is audited");
    // And the broker holds nothing afterwards: no grant was issued for the
    // refused plugin (fail-closed at the door, not half-applied).
    let error = gate
        .broker()
        .check("plugin.a", "telemetry.read", &data_scope("tenant-1"), NOW)
        .expect_err("no grant exists for the refused plugin");
    assert_eq!(error.name(), "CAPABILITY_NOT_GRANTED");
}

#[test]
fn an_over_broad_manifest_is_refused_never_trimmed_to_fit() {
    let (gate, sink) = gate_with_audit();
    // The manifest declares two permissions; the ceiling permits only one.
    // The request REFUSES (it is never silently reduced to the intersection
    // and instantiated with less than it asked for).
    let request = request("plugin.a", &["telemetry.read", "asset.read"]);
    let ceiling = ProfileCeiling::new(vec![ceiling_entry("telemetry.read", data_scope("tenant-1"))]);
    let error = evaluate(&gate, &request, &ceiling).expect_err("over-broad is refused");
    match &error {
        EnforcementError::PermissionAboveProfile { plugin_id, capability } => {
            assert_eq!(plugin_id, "plugin.a");
            assert_eq!(capability, "asset.read");
        }
        other => panic!("expected PermissionAboveProfile, got {other:?}"),
    }
    assert_eq!(error.name(), "PERMISSION_ABOVE_PROFILE");
    let refusals = sink
        .events()
        .iter()
        .filter(|e| e.kind == EnforcementAuditKind::InstantiationRefused)
        .count();
    assert_eq!(refusals, 1);
}

#[test]
fn a_ceiling_entry_the_manifest_does_not_declare_is_never_granted() {
    // The reverse over-broad direction: the ceiling may permit more than
    // the manifest declares, but the GRANTED set is the intersection — the
    // undeclared ceiling entry stays unused (bounded, not inherited).
    let (gate, _sink) = gate_with_audit();
    let request = request("plugin.a", &["telemetry.read"]);
    let ceiling = ProfileCeiling::new(vec![
        ceiling_entry("telemetry.read", data_scope("tenant-1")),
        ceiling_entry(
            "forecast.publish",
            CapabilityScope::SendOnlyTarget { target: "forecasts".to_string() },
        ),
    ]);
    let verdict = evaluate(&gate, &request, &ceiling).expect("declared permissions construct");
    assert_eq!(verdict.granted_capabilities(), vec!["telemetry.read"]);
    assert_eq!(verdict.granted_scope("forecast.publish"), None);
}

#[test]
fn an_unknown_permission_id_is_refused_at_the_door() {
    let (gate, _sink) = gate_with_audit();
    // The ADR-010 spelling: a manifest asking for a "validation verdict"
    // authority that does not exist in ANY vocabulary.
    let request = request("plugin.a", &["dispatch.validate.verdict"]);
    let ceiling = ProfileCeiling::empty();
    let error = evaluate(&gate, &request, &ceiling).expect_err("unknown ids refuse");
    assert_eq!(error.name(), "PERMISSION_UNKNOWN");
}

#[test]
fn a_misconfigured_ceiling_is_refused() {
    let (gate, _sink) = gate_with_audit();
    // A ceiling that names a never-granted capability is refused exactly
    // like a manifest that names one (the gate trusts no configuration).
    let request = request("plugin.a", &[]);
    let bad_ceiling =
        ProfileCeiling::new(vec![ceiling_entry("secret.read", data_scope("tenant-1"))]);
    let error = evaluate(&gate, &request, &bad_ceiling).expect_err("never-granted ceiling entry");
    assert_eq!(error.name(), "PERMISSION_NEVER_GRANTABLE");

    // Duplicate ceiling entries are a profile defect.
    let ceiling = ProfileCeiling::new(vec![
        ceiling_entry("telemetry.read", data_scope("tenant-1")),
        ceiling_entry("telemetry.read", data_scope("tenant-2")),
    ]);
    let error = evaluate(&gate, &request, &ceiling).expect_err("duplicate ceiling entry");
    assert_eq!(error.name(), "PROFILE_INVALID");
}

// --- Vocabulary drift ------------------------------------------------------------

#[test]
fn vocabulary_drift_is_a_named_refusal_never_a_default_interpretation() {
    let (gate, sink) = gate_with_audit();
    // The recorded alignment seam: the ledger's `resources.timeoutMs`
    // spelling vs the kernel schema's `timeout-ms`; the kernel
    // `ResourceBounds` carries NO fuel or concurrency field. Any
    // unalignable field name refuses the WHOLE request.
    let cases = [
        "timeoutMs",
        "fuel-ms",
        "fuel",
        "concurrency",
        "max-memory-MB",
    ];
    for field in cases {
        let mut request = request("plugin.a", &[]);
        request.declared_resource_fields = vec![
            "max-memory-mb".to_string(),
            field.to_string(),
        ];
        let ceiling = ProfileCeiling::empty();
        let error = evaluate(&gate, &request, &ceiling)
            .expect_err("unalignable field names refuse the request");
        assert_eq!(error.name(), "PERMISSION_VOCABULARY_DRIFT", "field {field}");
        let rendered = error.to_string();
        assert!(rendered.contains(field), "the drift names the offending field '{field}'");
    }
    // The refusal is audited with its named reason.
    let refusals = sink
        .events()
        .iter()
        .filter(|e| e.kind == EnforcementAuditKind::InstantiationRefused)
        .count();
    assert_eq!(refusals, cases.len());
}

#[test]
fn a_duplicate_declared_resource_field_is_drift_too() {
    let (gate, _sink) = gate_with_audit();
    let mut request = request("plugin.a", &[]);
    request.declared_resource_fields = vec![
        "max-memory-mb".to_string(),
        "timeout-ms".to_string(),
        "timeout-ms".to_string(),
    ];
    let ceiling = ProfileCeiling::empty();
    let error = evaluate(&gate, &request, &ceiling).expect_err("duplicate field shape");
    assert_eq!(error.name(), "PERMISSION_VOCABULARY_DRIFT");
}

// --- The frozen capability set ---------------------------------------------------

#[test]
fn the_capability_set_is_frozen_at_instantiation() {
    let (gate, _sink) = gate_with_audit();
    let request = request("plugin.telemetry", &["telemetry.read"]);
    let ceiling = ProfileCeiling::new(vec![ceiling_entry("telemetry.read", data_scope("tenant-1"))]);
    let verdict = evaluate(&gate, &request, &ceiling).expect("verdict constructs");
    let frozen = verdict.clone();

    // A "later manifest change": the SAME manifest object mutated to
    // declare more, plus a widened ceiling. Re-evaluation produces a NEW,
    // larger verdict — but the already-issued verdict is untouched.
    let mut mutated = request("plugin.telemetry", &["telemetry.read", "asset.read", "forecast.publish"]);
    mutated.bounds = DeclaredBounds {
        max_memory_mb: 65_536,
        timeout_ms: 3_600_000,
        max_output_mb: Some(4_096),
        max_calls_per_minute: Some(1_000_000),
    };
    let widened = ProfileCeiling::new(vec![
        ceiling_entry("telemetry.read", data_scope("tenant-1")),
        ceiling_entry("asset.read", data_scope("tenant-1")),
        ceiling_entry(
            "forecast.publish",
            CapabilityScope::SendOnlyTarget { target: "forecasts".to_string() },
        ),
    ]);
    let expanded = evaluate(&gate, &mutated, &widened).expect("the mutated request evaluates separately");
    assert_eq!(
        expanded.granted_capabilities(),
        vec!["telemetry.read", "asset.read", "forecast.publish"]
    );
    // The mutated manifest object did NOT expand the existing verdict.
    assert_eq!(frozen, verdict, "the issued verdict is immutable");
    assert_eq!(frozen.granted_capabilities(), vec!["telemetry.read"]);
    assert_eq!(frozen.limits().max_memory_pages(), 256 * 16, "its limits are frozen too");
    assert_ne!(frozen, expanded, "a new capability set requires a new verdict");
}

#[test]
fn broker_revocation_tightens_per_call_authority_but_the_frozen_set_stands() {
    let (gate, _sink) = gate_with_audit();
    let request = request("plugin.telemetry", &["telemetry.read"]);
    let ceiling = ProfileCeiling::new(vec![ceiling_entry("telemetry.read", data_scope("tenant-1"))]);
    let verdict = evaluate(&gate, &request, &ceiling).expect("verdict constructs");
    let grant_id = verdict.grants()[0].grant_id().to_string();

    // Revocation tightens the LIVE authority at the next call...
    gate.broker().revoke(&grant_id, NOW + 1).expect("revoke");
    let error = gate
        .broker()
        .check("plugin.telemetry", "telemetry.read", &data_scope("tenant-1"), NOW + 2)
        .expect_err("revoked authority is gone");
    assert_eq!(error.name(), "CAPABILITY_NOT_GRANTED");
    // ...but the frozen set is the instantiation-time record: it bounds
    // what the instance CAN hold; it is never widened by anything, and it
    // is not erased by tightening either.
    assert_eq!(verdict.granted_capabilities(), vec!["telemetry.read"]);
    assert_eq!(verdict.granted_scope("telemetry.read"), Some(data_scope("tenant-1")));
}

// --- Audit-or-refuse -------------------------------------------------------------

#[test]
fn allow_and_refuse_decisions_are_audited_with_scope_and_reason() {
    let (gate, sink) = gate_with_audit();
    let request = request("plugin.telemetry", &["telemetry.read"]);
    let ceiling = ProfileCeiling::new(vec![ceiling_entry("telemetry.read", data_scope("tenant-1"))]);
    let _verdict = evaluate(&gate, &request, &ceiling).expect("allow");

    let events = sink.events();
    let allowed = events
        .iter()
        .find(|e| e.kind == EnforcementAuditKind::InstantiationAllowed)
        .expect("the allow is audited");
    assert_eq!(allowed.plugin_id, "plugin.telemetry");
    assert!(
        allowed.detail.contains("telemetry.read=data:tenant=tenant-1"),
        "the allow carries the EXACT granted scope: {}",
        allowed.detail
    );
    let frozen = events
        .iter()
        .find(|e| e.kind == EnforcementAuditKind::CapabilitySetFrozen)
        .expect("the freeze is audited");
    assert!(
        frozen.detail.contains("drain-before-replace"),
        "the freeze record names the only re-instantiation path: {}",
        frozen.detail
    );

    // A refusal is audited with its named reason.
    let bad = request("plugin.a", &["dispatch.authorize"]);
    let _ = evaluate(&gate, &bad, &ProfileCeiling::empty()).expect_err("refuse");
    let refused = events
        .iter()
        .filter(|e| e.kind == EnforcementAuditKind::InstantiationRefused)
        .count();
    assert_eq!(refused, 1);
}

#[test]
fn a_broken_audit_sink_refuses_the_decision_in_both_directions() {
    // Allow direction: a verdict that could not be audited is NEVER issued.
    let gate = EnforcementGate::compose(
        flexcore_plugin_host::broker::CapabilityBroker::new(Box::new(
            flexcore_plugin_host::broker::InMemoryBrokerAuditSink::new(),
        )),
        Box::new(FailingEnforcementAuditSink),
    );
    let request = request("plugin.a", &["telemetry.read"]);
    let ceiling = ProfileCeiling::new(vec![ceiling_entry("telemetry.read", data_scope("tenant-1"))]);
    let error = evaluate(&gate, &request, &ceiling).expect_err("unaudited allow is no allow");
    assert_eq!(error.name(), "AUDIT_UNAVAILABLE");

    // Refuse direction: the refusal surfaces as the audit failure too (the
    // broker's audit-or-refuse pattern).
    let bad = request("plugin.a", &["dispatch.authorize"]);
    let error = evaluate(&gate, &bad, &ProfileCeiling::empty())
        .expect_err("unaudited refusal still refuses");
    assert_eq!(error.name(), "AUDIT_UNAVAILABLE");
}

// --- The candidate-only rule (§13.4) ---------------------------------------------

#[test]
fn a_proposed_schedule_gains_no_authority_from_being_proposed() {
    let (gate, _sink) = gate_with_audit();
    let request = request("plugin.research", &["schedule.propose", "dispatch.propose"]);
    let ceiling = ProfileCeiling::new(vec![
        ceiling_entry(
            "schedule.propose",
            CapabilityScope::SendOnlyTarget { target: "candidate-schedules".to_string() },
        ),
        ceiling_entry(
            "dispatch.propose",
            CapabilityScope::SendOnlyTarget { target: "candidate-dispatches".to_string() },
        ),
    ]);
    let verdict = evaluate(&gate, &request, &ceiling).expect("proposal capabilities are grantable");
    // The grants exist and are send-only candidates...
    assert_eq!(verdict.granted_capabilities(), vec!["schedule.propose", "dispatch.propose"]);
    // ...and the broker exposes NO path from a proposal to an authorization
    // or an issued command: the plugin holds no dispatch.authorize /
    // dispatch.issue capability (never-granted — a check against them is a
    // crafted call, refused and audited).
    for capability in ["dispatch.authorize", "dispatch.issue"] {
        let error = gate
            .broker()
            .check("plugin.research", capability, &data_scope("tenant-1"), NOW)
            .expect_err("no authorization path exists");
        assert_eq!(error.name(), "PERMISSION_NEVER_GRANTABLE", "capability {capability}");
    }
}

#[test]
fn the_validation_forward_returns_no_verdict_type_level() {
    // Lexical/API-shape assertion (ledger §P7.11 step 4, re-asserted here
    // as the candidate-only enforcement backing): the forward's signature
    // has no verdict in it — a proposal cannot harvest an authorization
    // through a return type that does not exist.
    let broker_source = include_str!("../src/broker.rs");
    assert!(
        broker_source.contains("pub fn request_validation_forward"),
        "the forward exists"
    );
    assert!(
        broker_source.contains("-> Result<(), BrokerError>"),
        "the forward returns () — no verdict can flow back through a signature with no verdict in it"
    );
    assert!(!broker_source.contains("pub fn authorize"), "no authorization function is exposed");
    assert!(!broker_source.contains("pub fn issue"), "no dispatch-issue function is exposed");
}

// --- No raw-instantiation path exists (lexical/API-shape) ------------------------

#[test]
fn no_raw_instantiation_path_exists() {
    // `bind_component` (the host's raw binding helper) was demoted to
    // `pub(crate)` in P7.12 (visibility-only change): the lexical state of
    // the source is the API shape.
    let host_source = include_str!("../src/host.rs");
    assert!(
        host_source.contains("pub(crate) fn bind_component"),
        "the raw binding helper is crate-internal"
    );
    assert!(
        !host_source.contains("pub fn bind_component"),
        "no public raw binding helper exists"
    );

    // The ONLY public constructor path takes the enforcement verdict.
    let enforcement_source = include_str!("../src/enforcement.rs");
    assert!(
        enforcement_source.contains("pub fn instantiate("),
        "the public instantiation function exists"
    );
    assert!(
        enforcement_source.contains("verdict: &EnforcementVerdict"),
        "it requires an enforcement verdict"
    );

    // The verdict is unforgeable: private fields, no public constructor
    // inside its impl block (the only way to obtain one is
    // `EnforcementGate::evaluate` — a struct literal over private fields is
    // a compile error in any external caller, including this test).
    let verdict_struct = item_source(enforcement_source, "pub struct EnforcementVerdict {");
    assert!(
        !verdict_struct.contains("pub "),
        "every EnforcementVerdict field is private (forgery is a compile error)"
    );
    let verdict_impl = item_source(enforcement_source, "impl EnforcementVerdict {");
    assert!(
        !verdict_impl.contains("pub fn new"),
        "no public constructor on the verdict"
    );
}

#[test]
fn the_brokers_grant_type_is_broker_constructed() {
    // Ledger §P7.13(c) structural refusal, asserted at the enforcement
    // layer too: a CapabilityGrant can only be constructed INSIDE the
    // broker (its fields are `pub(crate)` since P7.12/P7.13 — a
    // visibility-only change), so a caller outside this crate cannot forge
    // a grant object, and authority itself never leaves the broker's state
    // (per-call re-check).
    let broker_source = include_str!("../src/broker.rs");
    let grant_struct = item_source(broker_source, "pub struct CapabilityGrant {");
    assert!(
        !grant_struct.contains("pub grant_id")
            && !grant_struct.contains("pub plugin_id")
            && !grant_struct.contains("pub capability")
            && !grant_struct.contains("pub scope"),
        "the grant type's fields are not publicly constructible"
    );
    assert!(
        broker_source.contains("pub fn grant_id(&self)"),
        "read access goes through getters"
    );
}
