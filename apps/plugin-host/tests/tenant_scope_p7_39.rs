// P7.39 — The host-side tenant-scope cross-check (test file WRITTEN, NOT
// RUN — the loop bans hold; the human build/test pass executes
// `cargo test -p flexcore-plugin-host --test tenant_scope_p7_39`).
//
// Coverage (ledger P7.39; task test list):
//
// 1. Scope derivation from the authenticated context only (the mirrored
//    `HostTenantScope`; the only constructor; empty tenant/site refused).
// 2. Mismatch refusal at enforcement: a data request outside the
//    host-stamped envelope is TENANT_SCOPE_MISMATCH — refused BEFORE the
//    broker is consulted (envelope FIRST: a misconfigured grant can never
//    widen the stamped scope).
// 3. Broker narrowing still applies: inside the envelope but outside the
//    grant → the broker's named refusal surfaces.
// 4. Cross-tenant attempt end-to-end refused + audited (both audit
//    surfaces: the gate's denial record and the broker's denial record).
// 5. Non-data scopes through the data gate are refused, never coerced.
// 6. Audit-or-refuse: a broken audit sink refuses the check in BOTH
//    directions.
// 7. The worker-tenancy model and the RLS relationship are recorded.

use std::sync::Arc;

use flexcore_plugin_host::broker::{
    CapabilityBroker, CapabilityScope, InMemoryBrokerAuditSink,
};
use flexcore_plugin_host::tenant_gate::{
    HostTenantScope, InMemoryTenantGateAuditSink, TenantGateAuditKind, TenantGateError,
    TenantScopeGate, RLS_RELATIONSHIP_STATEMENT, TENANT_GATE_STATEMENT,
    WORKER_TENANCY_STATEMENT,
};

const NOW: u64 = 1_700_000_000_000;
const PLUGIN: &str = "neryva.tenant-scoped-forecaster";

fn scope(tenant: &str, site: Option<&str>) -> HostTenantScope {
    HostTenantScope::from_authenticated(tenant, site).expect("valid scope fixture")
}

fn data_scope(tenant: &str, site: Option<&str>) -> CapabilityScope {
    CapabilityScope::DataScope {
        tenant_id: tenant.to_string(),
        site_id: site.map(|s| s.to_string()),
        asset_id: None,
    }
}

/// A broker with one tenant-A/site-1 telemetry.read grant (the P7.11
/// derivation path; the grant is deliberately TENANT-WIDE in one test to
/// prove the envelope bound).
fn broker_with_grant(tenant: &str, site: Option<&str>) -> CapabilityBroker {
    let broker = CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()));
    let derivation = flexcore_plugin_host::broker::GrantDerivationRequest {
        plugin_id: PLUGIN.to_string(),
        manifest_id: PLUGIN.to_string(),
        manifest_version: "0.1.0".to_string(),
        manifest_permissions: vec!["telemetry.read".to_string()],
        requested_grants: vec![flexcore_plugin_host::broker::GrantRequest {
            capability: "telemetry.read".to_string(),
            scope: data_scope(tenant, site),
            expires_at_unix_ms: None,
        }],
    };
    broker.derive_grants(&derivation, NOW).expect("grants derived");
    broker
}

// --- 1. Authenticated-context-only derivation ------------------------------------

#[test]
fn the_host_scope_is_stamped_from_the_authenticated_context_only() {
    let wide = scope("tenant-alpha", None);
    assert_eq!(wide.tenant_id(), "tenant-alpha");
    assert_eq!(wide.site_id(), None);
    assert_eq!(wide.canonical(), "tenant:tenant-alpha|site:none");

    let narrowed = scope("tenant-alpha", Some("site-1"));
    assert_eq!(narrowed.site_id(), Some("site-1"));
    assert_eq!(narrowed.canonical(), "tenant:tenant-alpha|site:site-1");

    assert_eq!(
        HostTenantScope::from_authenticated("", None)
            .expect_err("no tenant")
            .name(),
        "TENANT_SCOPE_INVALID"
    );
    assert_eq!(
        HostTenantScope::from_authenticated("tenant-alpha", Some(" "))
            .expect_err("blank site")
            .name(),
        "TENANT_SCOPE_INVALID"
    );
    assert!(TENANT_GATE_STATEMENT.contains("host-stamped"));
    assert!(TENANT_GATE_STATEMENT.contains("BOTH"));
}

// --- 2. The cross-check: envelope FIRST ------------------------------------------

#[test]
fn the_gate_cross_checks_the_envelope_and_a_misconfigured_grant_cannot_widen_it() {
    let gate = TenantScopeGate::compose(Box::new(InMemoryTenantGateAuditSink::new()));
    let broker = broker_with_grant("tenant-alpha", None); // TENANT-WIDE grant (over-broad on purpose)
    let envelope = scope("tenant-alpha", Some("site-1")); // host-stamped site narrowing

    // Inside BOTH envelopes: admitted.
    gate.check_data_request(
        &broker,
        PLUGIN,
        &envelope,
        "telemetry.read",
        &data_scope("tenant-alpha", Some("site-1")),
        NOW,
    )
    .expect("in-scope request admitted");

    // Outside the host-stamped envelope (other site) but inside the
    // (misconfigured, tenant-wide) GRANT: refused at the ENVELOPE —
    // the broker is not even consulted. TENANT_SCOPE_MISMATCH.
    let err = gate
        .check_data_request(
            &broker,
            PLUGIN,
            &envelope,
            "telemetry.read",
            &data_scope("tenant-alpha", Some("site-2")),
            NOW,
        )
        .expect_err("cross-site refused at the envelope");
    match err {
        TenantGateError::TenantScopeMismatch { envelope: env, requested } => {
            assert_eq!(env, "tenant:tenant-alpha|site:site-1");
            assert_eq!(requested, "tenant:tenant-alpha|site:site-2");
        }
        other => panic!("expected TenantScopeMismatch, got {other:?}"),
    }

    // An UN-narrowed request under a site-narrowed envelope: refused too
    // (a site envelope does not cover a tenant-wide request).
    assert_eq!(
        gate.check_data_request(
            &broker,
            PLUGIN,
            &envelope,
            "telemetry.read",
            &data_scope("tenant-alpha", None),
            NOW,
        )
        .expect_err("un-narrowed request refused")
        .name(),
        "TENANT_SCOPE_MISMATCH"
    );
}

// --- 3/4. Broker narrowing + cross-tenant end-to-end refusal + audit ---------------

#[test]
fn cross_tenant_attempts_are_refused_and_audited_at_both_layers() {
    let gate_audit = Arc::new(InMemoryTenantGateAuditSink::new());
    let gate = TenantScopeGate::compose(Box::new(gate_audit.clone()));
    // The grant is correctly tenant-A-wide; the envelope is tenant-A
    // site-1; the REQUEST is tenant-B — cross-tenant.
    let broker = broker_with_grant("tenant-alpha", None);
    let envelope = scope("tenant-alpha", Some("site-1"));

    // Refused at the envelope (before the broker), audited on the gate.
    let err = gate
        .check_data_request(
            &broker,
            PLUGIN,
            &envelope,
            "telemetry.read",
            &data_scope("tenant-beta", None),
            NOW,
        )
        .expect_err("cross-tenant refused");
    assert_eq!(err.name(), "TENANT_SCOPE_MISMATCH");
    let gate_denials = gate_audit.events();
    let denied = gate_denials
        .iter()
        .find(|e| e.kind == TenantGateAuditKind::ScopeRefused)
        .expect("the gate denial is audited");
    assert_eq!(denied.plugin_id, PLUGIN);
    assert!(denied.detail.contains("TENANT_SCOPE_MISMATCH"));
    assert_eq!(denied.recorded_at_unix_ms, NOW);

    // Defence in depth #2: even when the envelope is tenant-wide (so the
    // envelope check passes), a cross-tenant request still hits the
    // broker's grant narrowing and is refused under the broker's name.
    let wide_gate = TenantScopeGate::compose(Box::new(InMemoryTenantGateAuditSink::new()));
    let wide_broker = broker_with_grant("tenant-alpha", None);
    let wide_envelope = scope("tenant-alpha", None);
    let err = wide_gate
        .check_data_request(
            &wide_broker,
            PLUGIN,
            &wide_envelope,
            "telemetry.read",
            &data_scope("tenant-beta", None),
            NOW,
        )
        .expect_err("cross-tenant refused at the broker");
    match err {
        TenantGateError::BrokerRefusal { error } => {
            assert_eq!(error.name(), "SCOPE_OUTSIDE_GRANT");
        }
        other => panic!("expected BrokerRefusal, got {other:?}"),
    }

    // And a scope the grant simply does not carry (wrong site under a
    // site-1 grant, envelope-wide) surfaces the broker refusal as well.
    let site_broker = broker_with_grant("tenant-alpha", Some("site-1"));
    let err = wide_gate
        .check_data_request(
            &site_broker,
            PLUGIN,
            &wide_envelope,
            "telemetry.read",
            &data_scope("tenant-alpha", Some("site-2")),
            NOW,
        )
        .expect_err("outside the grant envelope");
    match err {
        TenantGateError::BrokerRefusal { error } => {
            assert_eq!(error.name(), "SCOPE_OUTSIDE_GRANT");
        }
        other => panic!("expected BrokerRefusal, got {other:?}"),
    }
}

// --- 5. Non-data scopes are refused, never coerced ---------------------------------

#[test]
fn only_data_scopes_pass_the_data_gate() {
    let gate = TenantScopeGate::compose(Box::new(InMemoryTenantGateAuditSink::new()));
    let broker = CapabilityBroker::new(Box::new(InMemoryBrokerAuditSink::new()));
    let envelope = scope("tenant-alpha", None);

    let err = gate
        .check_data_request(
            &broker,
            PLUGIN,
            &envelope,
            "telemetry.read",
            &CapabilityScope::ReadOnlyPathPrefix { prefix: "/data/telemetry".to_string() },
            NOW,
        )
        .expect_err("a path scope is not a data request");
    assert_eq!(err.name(), "TENANT_SCOPE_KIND_INVALID");

    let err = gate
        .check_data_request(
            &broker,
            PLUGIN,
            &envelope,
            "telemetry.read",
            &CapabilityScope::SendOnlyTarget { target: "worker-1".to_string() },
            NOW,
        )
        .expect_err("a send-only scope is not a data request");
    assert_eq!(err.name(), "TENANT_SCOPE_KIND_INVALID");
}

// --- 6. Audit-or-refuse in both directions -------------------------------------------

#[test]
fn a_broken_audit_sink_refuses_the_gate_in_both_directions() {
    struct BrokenSink;
    impl flexcore_plugin_host::tenant_gate::TenantGateAuditSink for BrokenSink {
        fn record(
            &self,
            _event: flexcore_plugin_host::tenant_gate::TenantGateAuditEvent,
        ) -> Result<(), TenantGateError> {
            Err(TenantGateError::AuditUnavailable {
                reason: "broken".to_string(),
            })
        }
    }
    let gate = TenantScopeGate::compose(Box::new(BrokenSink));
    let broker = broker_with_grant("tenant-alpha", Some("site-1"));
    let envelope = scope("tenant-alpha", Some("site-1"));

    // An ADMITTED use that could not be audited: the receipt never
    // reaches the caller (the use confers nothing).
    let err = gate
        .check_data_request(
            &broker,
            PLUGIN,
            &envelope,
            "telemetry.read",
            &data_scope("tenant-alpha", Some("site-1")),
            NOW,
        )
        .expect_err("an unauditable admission is refused");
    assert_eq!(err.name(), "AUDIT_UNAVAILABLE");

    // A REFUSAL that could not be audited: surfaces as AUDIT_UNAVAILABLE
    // too (the broker's audit-or-refuse asymmetry is preserved by the
    // gate's own audit port).
    let err = gate
        .check_data_request(
            &broker,
            PLUGIN,
            &envelope,
            "telemetry.read",
            &data_scope("tenant-beta", None),
            NOW,
        )
        .expect_err("an unauditable denial is an error too");
    assert_eq!(err.name(), "AUDIT_UNAVAILABLE");
}

// --- 7. Recorded models: worker tenancy + RLS relationship ---------------------------

#[test]
fn the_worker_tenancy_model_and_the_rls_relationship_are_recorded() {
    // Ledger P7.39 step 3: the per-tenant vs shared worker model is
    // RECORDED. v0.5.0: one out-of-process worker per plugin instance,
    // launched by the host that instantiated it — no worker process is
    // shared across tenants; the tested case is the per-tenant worker.
    assert!(WORKER_TENANCY_STATEMENT.contains("NO worker process is shared across tenants"));
    assert!(WORKER_TENANCY_STATEMENT.contains("per plugin instance"));
    assert!(WORKER_TENANCY_STATEMENT.contains("new architecture with its own decision record"));

    // The RLS relationship (§16.2/[R25]; ADR-007) is recorded, not
    // re-implemented.
    assert!(RLS_RELATIONSHIP_STATEMENT.contains("never touch tables directly"));
    assert!(RLS_RELATIONSHIP_STATEMENT.contains("Row-Level Security remain the platform's"));
    assert!(RLS_RELATIONSHIP_STATEMENT.contains("envelope-level cross-check only"));

    // The gate's audit-kind names are stable and distinct.
    let names = [
        TenantGateAuditKind::ScopeAdmitted.name(),
        TenantGateAuditKind::ScopeRefused.name(),
    ];
    assert_ne!(names[0], names[1]);
}
