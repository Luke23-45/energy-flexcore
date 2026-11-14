// P7.11 — Capability-broker tests (WRITTEN-UNRUN: no cargo command was
// executed in this loop; the first human `cargo test -p flexcore-plugin-host`
// run is the verification gate — decision record
// `ledger/phase-07-plugin-sdk/10_11_plugin_host_broker.md` §8).
//
// Coverage (ledger §P7.11 acceptance surface):
//
// - default-deny: EVERY capability class refused without a grant; the
//   never-granted set refused even with a crafted request; unknown ids
//   refused;
// - scoped grant honoured within scope and refused outside it (path-prefix
//   boundary and `..` escape, expired grant, wrong scope kind, wrong
//   tenant/site narrowing);
// - revocation takes effect immediately (per-call re-check, no cached
//   authority);
// - audit records for grant / use / denial / revocation / validation
//   forward;
// - manifest-permission → grant derivation honours the manifest's declared
//   bounds ONLY (over-broad request → named error; never-granted refused at
//   derivation; atomic batch);
// - the `dispatch.validate.request` boundary: only the protected-validator
//   target is grantable, and the forward returns no verdict.

use flexcore_plugin_host::broker::{
    BrokerAuditEvent, BrokerAuditKind, BrokerAuditSink, BrokerError, CapabilityBroker,
    CapabilityScope, GrantDerivationRequest, GrantRequest, InMemoryBrokerAuditSink,
    ALLOWED_PERMISSIONS, NEVER_GRANTED_PERMISSIONS, PROTECTED_VALIDATOR_TARGET,
};

/// A shared sink handle: the broker owns one end (a `Box<dyn BrokerAuditSink>`
/// over a clone), the test inspects the same event log through the other.
/// (The P7.11 draft moved the sink INTO the broker with `Box::new(sink)` and
/// read it afterwards — a use-after-move found during the P7.12/P7.13 review;
/// this wrapper is the minimal compile fix, recorded in the decision record
/// `12_13_enforcement_negatives.md` §10. No broker logic changed.)
#[derive(Clone, Debug, Default)]
struct SharedSink(std::sync::Arc<InMemoryBrokerAuditSink>);

impl SharedSink {
    fn new() -> SharedSink {
        SharedSink(std::sync::Arc::new(InMemoryBrokerAuditSink::new()))
    }

    /// A snapshot of the recorded events (delegated through the shared arc).
    fn events(&self) -> Vec<BrokerAuditEvent> {
        self.0.events()
    }
}

impl BrokerAuditSink for SharedSink {
    fn record(&self, event: BrokerAuditEvent) -> Result<(), BrokerError> {
        self.0.record(event)
    }
}
use flexcore_plugin_host::host::HostError;

// --- Test helpers ---------------------------------------------------------------

const NOW: u64 = 1_700_000_000_000;

fn data_scope(tenant: &str) -> CapabilityScope {
    CapabilityScope::DataScope {
        tenant_id: tenant.to_string(),
        site_id: None,
        asset_id: None,
    }
}

fn grant_request(capability: &str, scope: CapabilityScope) -> GrantRequest {
    GrantRequest { capability: capability.to_string(), scope, expires_at_unix_ms: None }
}

/// An audit sink that ALWAYS fails (the audit-or-refuse counter-case).
#[derive(Debug, Default)]
struct FailingAuditSink;

impl BrokerAuditSink for FailingAuditSink {
    fn record(&self, _event: BrokerAuditEvent) -> Result<(), BrokerError> {
        Err(BrokerError::AuditUnavailable {
            reason: "the test sink always fails".to_string(),
        })
    }
}

// --- Default deny ----------------------------------------------------------------

#[test]
fn every_capability_class_is_refused_without_a_grant() {
    let broker = CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()));
    for capability in ALLOWED_PERMISSIONS {
        let error = broker
            .check("plugin.a", capability, &data_scope("tenant-1"), NOW)
            .expect_err("defaults deny");
        assert_eq!(error.name(), "CAPABILITY_NOT_GRANTED", "capability {capability}");
    }
}

#[test]
fn the_never_granted_set_is_refused_even_without_any_grant_mechanics() {
    let broker = CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()));
    for capability in NEVER_GRANTED_PERMISSIONS {
        let error = broker
            .check("plugin.a", capability, &data_scope("tenant-1"), NOW)
            .expect_err("never granted");
        assert_eq!(error.name(), "PERMISSION_NEVER_GRANTABLE", "capability {capability}");
    }
}

#[test]
fn an_unknown_capability_id_is_refused() {
    let broker = CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()));
    let error = broker
        .check("plugin.a", "device.protocolspeak", &data_scope("tenant-1"), NOW)
        .expect_err("unknown id");
    assert_eq!(error.name(), "PERMISSION_UNKNOWN");
}

#[test]
fn a_grant_of_a_never_granted_capability_is_refused() {
    // The broker re-refuses at grant time (kernel manifest.rs header: the
    // manifest gate rejects first; the broker re-refuses regardless of what
    // any crafted manifest or profile claims).
    let broker = CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()));
    for capability in NEVER_GRANTED_PERMISSIONS {
        let error = broker
            .grant("plugin.a", "plugin.a", "1.0.0", &grant_request(capability, data_scope("tenant-1")), NOW)
            .expect_err("never grantable");
        assert_eq!(error.name(), "PERMISSION_NEVER_GRANTABLE", "capability {capability}");
    }
    // And the grant list holds nothing afterwards (nothing half-issued).
    for capability in ALLOWED_PERMISSIONS {
        let error = broker
            .check("plugin.a", capability, &data_scope("tenant-1"), NOW)
            .expect_err("still denied");
        assert_eq!(error.name(), "CAPABILITY_NOT_GRANTED", "capability {capability}");
    }
}

// --- Scoped grant: honoured within scope, refused outside ------------------------

#[test]
fn a_scoped_read_grant_is_honoured_within_its_prefix() {
    let broker = CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()));
    broker
        .grant(
            "plugin.telemetry",
            "plugin.telemetry",
            "1.0.0",
            &grant_request(
                "telemetry.read",
                CapabilityScope::ReadOnlyPathPrefix { prefix: "/data/telemetry".to_string() },
            ),
            NOW,
        )
        .expect("grant issues");
    broker
        .check(
            "plugin.telemetry",
            "telemetry.read",
            &CapabilityScope::ReadOnlyPathPrefix { prefix: "/data/telemetry/day.csv".to_string() },
            NOW,
        )
        .expect("inside the grant envelope");
}

#[test]
fn a_path_prefix_escape_is_refused() {
    let broker = CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()));
    broker
        .grant(
            "plugin.telemetry",
            "plugin.telemetry",
            "1.0.0",
            &grant_request(
                "telemetry.read",
                CapabilityScope::ReadOnlyPathPrefix { prefix: "/data/telemetry".to_string() },
            ),
            NOW,
        )
        .expect("grant issues");
    // Sibling-prefix trick: starts with the granted string, but is a
    // different segment (the grant boundary is a segment boundary).
    let sibling = broker.check(
        "plugin.telemetry",
        "telemetry.read",
        &CapabilityScope::ReadOnlyPathPrefix { prefix: "/data/telemetry-evil/x".to_string() },
        NOW,
    );
    assert_eq!(sibling.expect_err("sibling prefix denied").name(), "SCOPE_OUTSIDE_GRANT");
    // Literal `..` escape attempt: refused even though it starts with the
    // granted prefix.
    let dotdot = broker.check(
        "plugin.telemetry",
        "telemetry.read",
        &CapabilityScope::ReadOnlyPathPrefix { prefix: "/data/telemetry/../../etc".to_string() },
        NOW,
    );
    assert_eq!(dotdot.expect_err("escape denied").name(), "SCOPE_OUTSIDE_GRANT");
    // A wholly different tree.
    let other = broker.check(
        "plugin.telemetry",
        "telemetry.read",
        &CapabilityScope::ReadOnlyPathPrefix { prefix: "/data/other/x".to_string() },
        NOW,
    );
    assert_eq!(other.expect_err("other tree denied").name(), "SCOPE_OUTSIDE_GRANT");
}

#[test]
fn a_grant_of_an_escaping_prefix_is_refused_at_grant_time() {
    let broker = CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()));
    let error = broker
        .grant(
            "plugin.telemetry",
            "plugin.telemetry",
            "1.0.0",
            &grant_request(
                "telemetry.read",
                CapabilityScope::ReadOnlyPathPrefix { prefix: "/data/../secrets".to_string() },
            ),
            NOW,
        )
        .expect_err("escape-at-construction refused");
    assert_eq!(error.name(), "SCOPE_INVALID");
}

#[test]
fn an_expired_grant_is_refused_and_a_live_one_is_honoured() {
    let broker = CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()));
    broker
        .grant(
            "plugin.telemetry",
            "plugin.telemetry",
            "1.0.0",
            &GrantRequest {
                capability: "telemetry.read".to_string(),
                scope: data_scope("tenant-1"),
                expires_at_unix_ms: Some(NOW + 100),
            },
            NOW,
        )
        .expect("grant issues");
    broker
        .check("plugin.telemetry", "telemetry.read", &data_scope("tenant-1"), NOW + 99)
        .expect("one tick before expiry");
    let expired = broker
        .check("plugin.telemetry", "telemetry.read", &data_scope("tenant-1"), NOW + 100)
        .expect_err("expiry is exclusive");
    match &expired {
        BrokerError::GrantExpired { grant_id, expired_at_unix_ms } => {
            assert_eq!(*expired_at_unix_ms, NOW + 100);
            assert!(grant_id.starts_with("grant/plugin.telemetry/telemetry.read/"));
        }
        other => panic!("expected GrantExpired, got {other:?}"),
    }
}

#[test]
fn a_request_of_a_different_scope_kind_is_refused() {
    let broker = CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()));
    broker
        .grant(
            "plugin.telemetry",
            "plugin.telemetry",
            "1.0.0",
            &grant_request(
                "telemetry.read",
                CapabilityScope::ReadOnlyPathPrefix { prefix: "/data/telemetry".to_string() },
            ),
            NOW,
        )
        .expect("grant issues");
    // A send-only request can never fit inside a read-prefix grant.
    let cross = broker.check(
        "plugin.telemetry",
        "telemetry.read",
        &CapabilityScope::SendOnlyTarget { target: "some-queue".to_string() },
        NOW,
    );
    assert_eq!(cross.expect_err("cross-kind denied").name(), "SCOPE_OUTSIDE_GRANT");
}

#[test]
fn data_scope_narrowing_is_enforced_per_dimension() {
    let broker = CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()));
    // Grant: tenant-wide, no site/asset narrowing.
    broker
        .grant(
            "plugin.a",
            "plugin.a",
            "1.0.0",
            &grant_request("asset.read", data_scope("tenant-1")),
            NOW,
        )
        .expect("grant issues");
    // A site request inside the tenant is covered.
    broker
        .check(
            "plugin.a",
            "asset.read",
            &CapabilityScope::DataScope {
                tenant_id: "tenant-1".to_string(),
                site_id: Some("site-9".to_string()),
                asset_id: Some("asset-3".to_string()),
            },
            NOW,
        )
        .expect("tenant-wide grant covers the narrower request");
    // Another tenant is outside.
    let other_tenant = broker
        .check(
            "plugin.a",
            "asset.read",
            &CapabilityScope::DataScope {
                tenant_id: "tenant-2".to_string(),
                site_id: Some("site-9".to_string()),
                asset_id: None,
            },
            NOW,
        )
        .expect_err("other tenant denied");
    assert_eq!(other_tenant.name(), "SCOPE_OUTSIDE_GRANT");
    // And the reverse: a SITE-scoped grant does not cover a tenant-wide
    // request (no widening through the broker).
    let narrow_broker = CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()));
    narrow_broker
        .grant(
            "plugin.b",
            "plugin.b",
            "1.0.0",
            &grant_request(
                "telemetry.read",
                CapabilityScope::DataScope {
                    tenant_id: "tenant-1".to_string(),
                    site_id: Some("site-1".to_string()),
                    asset_id: None,
                },
            ),
            NOW,
        )
        .expect("grant issues");
    let widened = narrow_broker
        .check("plugin.b", "telemetry.read", &data_scope("tenant-1"), NOW)
        .expect_err("widening denied");
    assert_eq!(widened.name(), "SCOPE_OUTSIDE_GRANT");
}

// --- Revocation ------------------------------------------------------------------

#[test]
fn revocation_takes_effect_immediately() {
    let broker = CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()));
    let receipt = broker
        .grant(
            "plugin.a",
            "plugin.a",
            "1.0.0",
            &grant_request("forecast.publish", data_scope("tenant-1")),
            NOW,
        )
        .expect("grant issues");
    broker
        .check("plugin.a", "forecast.publish", &data_scope("tenant-1"), NOW)
        .expect("granted use works");
    broker.revoke(receipt.grant.grant_id(), NOW + 1).expect("revoke");
    let after = broker
        .check("plugin.a", "forecast.publish", &data_scope("tenant-1"), NOW + 2)
        .expect_err("revocation is immediate — nothing caches authority");
    assert_eq!(after.name(), "CAPABILITY_NOT_GRANTED");
    // Revoking the same (now unknown) id again is an error, not a silent
    // success.
    let again = broker.revoke(receipt.grant.grant_id(), NOW + 3).expect_err("nothing to revoke");
    assert_eq!(again.name(), "GRANT_UNKNOWN");
}

// --- Audit records ----------------------------------------------------------------

#[test]
fn grants_uses_denials_and_revocations_are_audited() {
    let sink = SharedSink::new();
    let broker = CapabilityBroker::new(Box::new(sink.clone()));
    let receipt = broker
        .grant(
            "plugin.a",
            "plugin.a",
            "1.0.0",
            &grant_request("schedule.propose", data_scope("tenant-1")),
            NOW,
        )
        .expect("grant issues");
    broker
        .check("plugin.a", "schedule.propose", &data_scope("tenant-1"), NOW)
        .expect("use honoured");
    let _ = broker
        .check("plugin.a", "dispatch.propose", &data_scope("tenant-1"), NOW)
        .expect_err("denied (no grant)");
    broker.revoke(receipt.grant.grant_id(), NOW).expect("revoke");

    let events = sink.events();
    let count = |kind: BrokerAuditKind| events.iter().filter(|e| e.kind == kind).count();
    assert_eq!(count(BrokerAuditKind::GrantIssued), 1);
    assert_eq!(count(BrokerAuditKind::CapabilityUse), 1);
    // Denials: the no-grant refusal for dispatch.propose.
    assert_eq!(count(BrokerAuditKind::CapabilityDenied), 1);
    assert_eq!(count(BrokerAuditKind::GrantRevoked), 1);
    // Every denial line names the plugin, the capability, and the scope
    // (ledger §P7.11 step 5).
    let denied = events
        .iter()
        .find(|e| e.kind == BrokerAuditKind::CapabilityDenied)
        .expect("a denial was recorded");
    assert_eq!(denied.plugin_id, "plugin.a");
    assert_eq!(denied.capability, "dispatch.propose");
    assert!(denied.detail.contains("tenant=tenant-1"));
}

#[test]
fn a_broken_audit_sink_stops_the_broker_in_both_directions() {
    // Audit-or-refuse: a USE past a broken sink fails the use; a GRANT past
    // a broken sink fails the grant. The broker never proceeds silently.
    let broker = CapabilityBroker::new(Box::new(FailingAuditSink));
    let granted = broker.grant(
        "plugin.a",
        "plugin.a",
        "1.0.0",
        &grant_request("schedule.propose", data_scope("tenant-1")),
        NOW,
    );
    assert_eq!(granted.expect_err("audit-or-refuse").name(), "AUDIT_UNAVAILABLE");
    let checked = broker.check("plugin.a", "schedule.propose", &data_scope("tenant-1"), NOW);
    assert_eq!(checked.expect_err("audit-or-refuse").name(), "AUDIT_UNAVAILABLE");
}

// --- Manifest-permission → grant derivation --------------------------------------

fn derivation(
    manifest_permissions: Vec<&str>,
    requested: Vec<GrantRequest>,
) -> GrantDerivationRequest {
    GrantDerivationRequest {
        plugin_id: "plugin.a".to_string(),
        manifest_id: "plugin.a".to_string(),
        manifest_version: "1.2.3".to_string(),
        manifest_permissions: manifest_permissions.iter().map(|s| s.to_string()).collect(),
        requested_grants: requested,
    }
}

#[test]
fn derivation_honours_the_manifest_declared_bounds_only() {
    let broker = CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()));
    let receipts = broker
        .derive_grants(
            &derivation(
                vec!["telemetry.read", "schedule.propose"],
                vec![
                    grant_request("telemetry.read", data_scope("tenant-1")),
                    grant_request("schedule.propose", data_scope("tenant-1")),
                ],
            ),
            NOW,
        )
        .expect("both requests are within the manifest's declared permissions");
    assert_eq!(receipts.len(), 2);
    for receipt in &receipts {
        assert_eq!(receipt.grant.manifest_id(), "plugin.a");
        assert_eq!(receipt.grant.manifest_version(), "1.2.3");
    }
}

#[test]
fn an_over_broad_derivation_request_is_a_named_error() {
    // `asset.read` is grantable in principle, but THIS manifest did not
    // declare it — refused, never trimmed to fit.
    let broker = CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()));
    let error = broker
        .derive_grants(
            &derivation(
                vec!["telemetry.read"],
                vec![grant_request("asset.read", data_scope("tenant-1"))],
            ),
            NOW,
        )
        .expect_err("over-broad");
    match &error {
        BrokerError::PermissionNotDeclared { plugin_id, capability } => {
            assert_eq!(plugin_id, "plugin.a");
            assert_eq!(capability, "asset.read");
        }
        other => panic!("expected PermissionNotDeclared, got {other:?}"),
    }
}

#[test]
fn a_crafted_manifest_cannot_derive_a_never_granted_capability() {
    // Ledger §P7.11 verification method: attempt to grant
    // `dispatch.authorize` via a crafted manifest and confirm the broker
    // refuses.
    let broker = CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()));
    let error = broker
        .derive_grants(
            &derivation(
                vec!["dispatch.authorize"], // a crafted manifest that "declares" it
                vec![grant_request("dispatch.authorize", data_scope("tenant-1"))],
            ),
            NOW,
        )
        .expect_err("never grantable");
    assert_eq!(error.name(), "PERMISSION_NEVER_GRANTABLE");
    // Nothing was granted (the batch is atomic).
    let after = broker.check("plugin.a", "dispatch.authorize", &data_scope("tenant-1"), NOW);
    assert_eq!(after.expect_err("nothing granted").name(), "PERMISSION_NEVER_GRANTABLE");
}

#[test]
fn a_duplicate_derivation_request_is_a_named_error_and_the_batch_is_atomic() {
    let broker = CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()));
    let error = broker
        .derive_grants(
            &derivation(
                vec!["telemetry.read", "asset.read"],
                vec![
                    grant_request("telemetry.read", data_scope("tenant-1")),
                    grant_request("asset.read", data_scope("tenant-1")),
                    grant_request("telemetry.read", data_scope("tenant-2")),
                ],
            ),
            NOW,
        )
        .expect_err("duplicate");
    assert_eq!(error.name(), "DUPLICATE_GRANT_REQUEST");
    // Atomicity: phase 1 validates ALL requests before phase 2 issues ANY
    // grant — nothing half-applied.
    for capability in ["telemetry.read", "asset.read"] {
        let after = broker.check("plugin.a", capability, &data_scope("tenant-1"), NOW);
        assert_eq!(after.expect_err("nothing granted").name(), "CAPABILITY_NOT_GRANTED");
    }
}

#[test]
fn an_already_expired_derivation_request_is_a_named_error() {
    let broker = CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()));
    let error = broker
        .derive_grants(
            &derivation(
                vec!["telemetry.read"],
                vec![GrantRequest {
                    capability: "telemetry.read".to_string(),
                    scope: data_scope("tenant-1"),
                    expires_at_unix_ms: Some(NOW - 1),
                }],
            ),
            NOW,
        )
        .expect_err("expiry in the past");
    assert_eq!(error.name(), "GRANT_EXPIRY_INVALID");
}

// --- The dispatch.validate.request boundary --------------------------------------

#[test]
fn the_validation_forward_grants_only_the_protected_validator_target() {
    let broker = CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()));
    // A grant to ANY other target for this capability is refused at grant
    // time — the plugin can never hold a grant whose envelope would let it
    // fish for verdicts elsewhere.
    let error = broker
        .grant(
            "plugin.a",
            "plugin.a",
            "1.0.0",
            &grant_request(
                "dispatch.validate.request",
                CapabilityScope::SendOnlyTarget { target: "my-own-validator".to_string() },
            ),
            NOW,
        )
        .expect_err("only the protected validator is grantable");
    assert_eq!(error.name(), "SCOPE_INVALID");
    // The correct grant works, and the forward returns NO verdict — the
    // return type carries nothing (the boundary, ledger §P7.11 step 4).
    broker
        .grant(
            "plugin.a",
            "plugin.a",
            "1.0.0",
            &grant_request(
                "dispatch.validate.request",
                CapabilityScope::SendOnlyTarget { target: PROTECTED_VALIDATOR_TARGET.to_string() },
            ),
            NOW,
        )
        .expect("the protected-validator grant issues");
    broker
        .request_validation_forward("plugin.a", "val-req-1", NOW)
        .expect("the forward is dispatched");
}

#[test]
fn a_validation_forward_without_the_grant_is_denied_and_audited() {
    let sink = SharedSink::new();
    let broker = CapabilityBroker::new(Box::new(sink.clone()));
    let error = broker
        .request_validation_forward("plugin.a", "val-req-2", NOW)
        .expect_err("defaults deny");
    assert_eq!(error.name(), "CAPABILITY_NOT_GRANTED");
    let denied = sink
        .events()
        .iter()
        .find(|e| e.kind == BrokerAuditKind::CapabilityDenied)
        .expect("the denial was audited");
    assert_eq!(denied.capability, "dispatch.validate.request");
    assert_eq!(denied.plugin_id, "plugin.a");
}

#[test]
fn the_forwarded_request_returns_no_verdict_object() {
    // Type-level honesty: the forward's `Ok` payload is `()` — there is no
    // verdict type in the signature for a plugin to mistake for
    // authorization (§5.7; the escalation itself is P7.12's test suite).
    let sink = InMemoryBrokerAuditSink::new();
    let broker = CapabilityBroker::new(Box::new(sink));
    broker
        .grant(
            "plugin.a",
            "plugin.a",
            "1.0.0",
            &grant_request(
                "dispatch.validate.request",
                CapabilityScope::SendOnlyTarget { target: PROTECTED_VALIDATOR_TARGET.to_string() },
            ),
            NOW,
        )
        .expect("grant issues");
    let outcome: () = broker
        .request_validation_forward("plugin.a", "val-req-3", NOW)
        .expect("forward dispatched");
    let _ = outcome; // literally nothing
}

// --- Host-side posture cross-check ------------------------------------------------

#[test]
fn host_error_names_stay_stable_for_the_matrix() {
    // The P7.12 enforcement matrix names code locations by error name; the
    // host-side names are pinned here so a rename is a deliberate, reviewed
    // change.
    assert_eq!(HostError::FuelExhausted.name(), "FUEL_EXHAUSTED");
    assert_eq!(
        HostError::DeadlineExceeded { deadline_ms: 1 }.name(),
        "DEADLINE_EXCEEDED"
    );
    assert_eq!(
        HostError::MemoryLimitExceeded { limit_pages: 1, requested_pages: 2 }.name(),
        "MEMORY_LIMIT_EXCEEDED"
    );
    assert_eq!(HostError::OutputLimitExceeded { limit_bytes: 1 }.name(), "OUTPUT_LIMIT_EXCEEDED");
    assert_eq!(
        HostError::CallRateExceeded { limit_per_minute: 1 }.name(),
        "CALL_RATE_EXCEEDED"
    );
}
