// P7.34 — Plugin-host trust boundary: mTLS, service identity, UNTRUSTED
// HOST (test file written, NOT run — the loop bans hold; the first human
// `cargo test -p flexcore-plugin-host` run is the verification gate).
//
// THE TRUST POSTURE, carried by this test file per the task (the
// authoritative table is `flexcore_plugin_host::transport_trust::
// TRUST_POSTURE`, asserted as documentation by
// `the_trust_posture_table_is_asserted_as_documentation`): the plugin host
// executes untrusted code and is NOT in the TCB for authorization
// decisions — the kernel/enforcement are policy, the PLATFORM is the
// authority. A compromised host can deny service and expose its own
// inputs; it cannot grant itself capabilities (the never-granted set is
// refused at grant and check time), cannot raise the control-authority
// mode, cannot forge authoritative audit (the P7.33 boundary statement —
// the platform's append-only audit store is the authoritative record),
// and every result it returns is untrusted input the platform validates
// (schema, §9.2 provenance, tenant scope) before the protected path sees
// it.
//
// Coverage (ledger §P7.34 steps 1–3, the parts implementable without a
// TLS stack or PKI — the transport itself is the RECORDED composition
// seam, see `TRANSPORT_WIRING_SEAM`):
//
// - identity-missing → fail-closed construction (`TRUST_IDENTITY_MISSING`);
// - identity-incomplete (no certificate reference / no key reference) →
//   fail-closed (`TRUST_IDENTITY_INCOMPLETE`);
// - CA-reference-missing → fail-closed (`TRUST_CA_PIN_MISSING`);
// - plaintext / scheme-less platform endpoint refused
//   (`TRUST_ENDPOINT_REFUSED`);
// - the rotation/expiry/revocation posture is REQUIRED configuration
//   (rotation owner, expiry monitor, revocation checking — each refusal
//   named);
// - the honest composition assembles references-only trust material
//   (no key material anywhere in the assembled type);
// - the trust-posture table and the compromised-host bound are asserted
//   as documentation;
// - the never-granted set restated as the compromised-host bound —
//   BEHAVIORALLY, through the broker: a compromised host calling
//   `grant`/`check` for a never-granted capability is refused, the
//   refusal is audited, and the audit lands in the P7.33 chain.
//
// What these tests deliberately do NOT cover: an actual TLS handshake,
// certificate rotation, or session revocation at the wire level — no TLS
// stack exists in this crate (recorded seam); the ledger's compromised-
// host result-validation suite (step 3's forged/oversized/wrong-tenant
// results at the PLATFORM) is the control-plane's gate over the same
// seam, recorded in the decision record §5.

use std::sync::Arc;
use std::time::Duration;

use flexcore_plugin_host::audit_chain::{first_broken_link, ChainedAuditSink};
use flexcore_plugin_host::broker::{
    BrokerAuditEvent, BrokerAuditKind, BrokerAuditSink, BrokerError, CapabilityBroker,
    CapabilityScope, GrantRequest,
};
use flexcore_plugin_host::transport_trust::{
    assemble_trust_material, never_granted_bound, HostIdentityStore, HostServiceIdentity,
    MtlsTrustConfig, RevocationChecking, TrustError, TlsMinimum, COMPROMISED_HOST_BOUND,
    TRUST_POSTURE, TRANSPORT_WIRING_SEAM,
};

const NOW: u64 = 1_700_000_000_000;

// --- Fixtures -----------------------------------------------------------------------

/// The honest identity record (references only — no material exists).
fn honest_identity() -> HostServiceIdentity {
    HostServiceIdentity {
        host_instance_id: "plugin-host-i-42".to_string(),
        deployment_id: "deployment-sim-lab-1".to_string(),
        certificate_reference: Some("deploy://certs/plugin-host-i-42.pem".to_string()),
        private_key_reference: Some("keyref:plugin-host-i-42".to_string()),
    }
}

/// The honest channel configuration (every required field present).
fn honest_config() -> MtlsTrustConfig {
    MtlsTrustConfig {
        platform_endpoint: "wss://platform.example:8443".to_string(),
        pinned_server_name: "platform.example".to_string(),
        pinned_platform_ca_reference: Some("deploy://ca/platform-root.pem".to_string()),
        tls_minimum: TlsMinimum::DEFAULT,
        session_credential_ttl: Duration::from_secs(3600),
        rotation_owner: "platform-security-ops".to_string(),
        expiry_monitor_enabled: true,
        revocation_checking: RevocationChecking::DeploymentEndpoint {
            reference: "deploy://revocation/checker".to_string(),
        },
    }
}

/// An identity store yielding a fixed record (`None` = nothing stored).
struct ScriptedIdentityStore(Option<HostServiceIdentity>);

impl HostIdentityStore for ScriptedIdentityStore {
    fn load_identity(&self) -> Result<Option<HostServiceIdentity>, TrustError> {
        Ok(self.0.clone())
    }
}

// --- 1. Identity-missing → fail-closed construction ---------------------------------

#[test]
fn an_identity_absent_store_refuses_construction() {
    let store = ScriptedIdentityStore(None);
    let error = assemble_trust_material(&store, &honest_config())
        .expect_err("no identity — no trust boundary (fail-closed)");
    assert_eq!(error.name(), "TRUST_IDENTITY_MISSING");
    assert!(
        error.to_string().contains("fail closed"),
        "the refusal states the fail-closed rule: {error}"
    );
}

// --- 2. Identity-incomplete → fail-closed construction -------------------------------

#[test]
fn an_identity_without_a_certificate_reference_refuses_construction() {
    let mut identity = honest_identity();
    identity.certificate_reference = None;
    let store = ScriptedIdentityStore(Some(identity));
    let error = assemble_trust_material(&store, &honest_config())
        .expect_err("no certificate reference — no presentable identity");
    assert_eq!(error.name(), "TRUST_IDENTITY_INCOMPLETE");
}

#[test]
fn an_identity_without_a_key_reference_refuses_construction() {
    let mut identity = honest_identity();
    identity.private_key_reference = Some("   ".to_string());
    let store = ScriptedIdentityStore(Some(identity));
    let error = assemble_trust_material(&store, &honest_config())
        .expect_err("a blank key reference is no key reference");
    assert_eq!(error.name(), "TRUST_IDENTITY_INCOMPLETE");
    let error = assemble_trust_material(&store, &honest_config()).expect_err("again");
    assert!(
        error.to_string().contains("private_key_reference"),
        "the refusal names the missing field: {error}"
    );
}

// --- 3. CA-reference-missing → fail-closed construction ------------------------------

#[test]
fn a_missing_platform_ca_pin_refuses_construction() {
    let store = ScriptedIdentityStore(Some(honest_identity()));
    let mut config = honest_config();
    config.pinned_platform_ca_reference = None;
    let error =
        assemble_trust_material(&store, &config).expect_err("no pinned CA — no verified platform");
    assert_eq!(error.name(), "TRUST_CA_PIN_MISSING");
    assert!(
        error.to_string().contains("ambient system store"),
        "the refusal states the no-system-store-fallback rule: {error}"
    );
}

// --- 4. Plaintext / scheme-less endpoints are refused --------------------------------

#[test]
fn plaintext_and_scheme_less_endpoints_are_refused() {
    let store = ScriptedIdentityStore(Some(honest_identity()));
    for bad_endpoint in [
        "ws://platform.example:8443",
        "http://platform.example",
        "platform.example:8443",
    ] {
        let mut config = honest_config();
        config.platform_endpoint = bad_endpoint.to_string();
        let error = assemble_trust_material(&store, &config)
            .expect_err("plaintext and scheme-less endpoints are refused");
        assert_eq!(error.name(), "TRUST_ENDPOINT_REFUSED", "endpoint: {bad_endpoint}");
    }
}

// --- 5. The rotation / expiry / revocation posture is REQUIRED configuration ---------

#[test]
fn the_rotation_expiry_revocation_posture_is_required_configuration() {
    let store = ScriptedIdentityStore(Some(honest_identity()));

    // No rotation owner recorded — the identity lifecycle is unowned.
    let mut config = honest_config();
    config.rotation_owner = "  ".to_string();
    assert_eq!(
        assemble_trust_material(&store, &config)
            .expect_err("rotation ownership is recorded, not invented")
            .name(),
        "TRUST_ROTATION_OWNER_MISSING"
    );

    // Expiry monitoring disabled — the host cannot see its own expiry.
    let mut config = honest_config();
    config.expiry_monitor_enabled = false;
    assert_eq!(
        assemble_trust_material(&store, &config)
            .expect_err("expiry monitoring must be enabled")
            .name(),
        "TRUST_EXPIRY_MONITOR_DISABLED"
    );

    // Revocation checking unconfigured — a revoked identity would be
    // undetectable; fail-closed.
    let mut config = honest_config();
    config.revocation_checking = RevocationChecking::Unconfigured;
    assert_eq!(
        assemble_trust_material(&store, &config)
            .expect_err("revocation checking must be configured")
            .name(),
        "TRUST_REVOCATION_UNCONFIGURED"
    );

    // Short-lived credentials: a zero TTL is invalid.
    let mut config = honest_config();
    config.session_credential_ttl = Duration::ZERO;
    assert_eq!(
        assemble_trust_material(&store, &config)
            .expect_err("credentials must be short-lived")
            .name(),
        "TRUST_CREDENTIAL_TTL_INVALID"
    );
}

// --- 6. The honest composition assembles references-only material --------------------

#[test]
fn the_honest_composition_assembles_references_only_trust_material() {
    let store = ScriptedIdentityStore(Some(honest_identity()));
    let material =
        assemble_trust_material(&store, &honest_config()).expect("the honest pair composes");
    assert_eq!(material.host_instance_id, "plugin-host-i-42");
    assert_eq!(material.deployment_id, "deployment-sim-lab-1");
    assert_eq!(
        material.pinned_platform_ca_reference,
        "deploy://ca/platform-root.pem"
    );
    assert_eq!(material.pinned_server_name, "platform.example");
    assert_eq!(material.tls_minimum, TlsMinimum::Tls13);
    assert_eq!(
        material.revocation_endpoint_reference,
        "deploy://revocation/checker"
    );
    // References only: the Debug rendering carries no key material because
    // NONE exists in the type (identity material is a deployment
    // requirement — no PKI implemented, the enrollment.rs honesty pattern).
    let rendered = format!("{material:?}");
    assert!(
        !rendered.to_lowercase().contains("private key material"),
        "no key material can appear in any rendering"
    );
}

// --- 7. The trust-posture table is asserted as documentation -------------------------

/// THE TRUST POSTURE, asserted row by row: every row's "decides" column
/// names a component OUTSIDE the host; the "compromised host" column is
/// the residual capability (deny service, expose own inputs — never more).
#[test]
fn the_trust_posture_table_is_asserted_as_documentation() {
    assert_eq!(TRUST_POSTURE.len(), 6, "six boundary layers are mapped");
    let decides: Vec<&str> = TRUST_POSTURE.iter().map(|row| row.0).collect();
    for layer in [
        "capability grants",
        "authorization authority",
        "authoritative audit",
        "control-authority mode",
        "session authentication",
        "result validation",
    ] {
        assert!(
            decides.contains(&layer),
            "the posture table must map the '{layer}' layer"
        );
    }
    // The kernel/enforcement-vs-platform split is stated.
    assert!(TRUST_POSTURE[1].1.contains("the platform"));
    assert!(TRUST_POSTURE[1].1.contains("distinct kernel gates"));
    // The audit row cites the P7.33 boundary.
    assert!(TRUST_POSTURE[2].2.contains("P7.33 boundary statement"));
    // The session row carries the revocation posture.
    assert!(TRUST_POSTURE[4].2.contains("drained not completed on trust"));
    // The result row carries the untrusted-input rule.
    assert!(TRUST_POSTURE[5].1.contains("UNTRUSTED INPUT"));
}

#[test]
fn the_compromised_host_bound_is_asserted_as_documentation() {
    for required_phrase in [
        "DENY SERVICE",
        "EXPOSE ITS OWN INPUTS",
        "never-granted set",
        "dispatch.authorize",
        "dispatch.issue",
        "secret.read",
        "network.any",
        "untrusted input",
        "no authority from having been proposed",
    ] {
        assert!(
            COMPROMISED_HOST_BOUND.contains(required_phrase),
            "the compromised-host bound must state '{required_phrase}'"
        );
    }
    // The transport wiring seam is recorded with its dependency list and
    // its honesty label (rustls NOT added in this wave).
    for required_phrase in [
        "RECORDED SEAM",
        "rustls",
        "rustls-pemfile",
        "ring",
        "UNPINNED-REVERIFY",
        "P7.20",
        "no TLS handshake",
    ] {
        assert!(
            TRANSPORT_WIRING_SEAM.contains(required_phrase),
            "the transport seam must record '{required_phrase}'"
        );
    }
}

// --- 8. The never-granted set, restated BEHAVIORALLY as the compromised-host bound ----

/// A shared handle to the chained audit sink (`ChainedAuditSink` implements
/// `BrokerAuditSink` on `&self`; the shared wrapper lets the test inspect
/// the chain while the broker holds the boxed port).
#[derive(Clone)]
struct SharedChained(Arc<ChainedAuditSink>);

impl BrokerAuditSink for SharedChained {
    fn record(&self, event: BrokerAuditEvent) -> Result<(), BrokerError> {
        self.0.record(event)
    }
}

#[test]
fn the_never_granted_set_restated_as_the_compromised_host_bound() {
    // The trust boundary cites ONE set — the broker's, not a copy.
    assert_eq!(
        never_granted_bound().as_slice(),
        &["dispatch.authorize", "dispatch.issue", "secret.read", "network.any"],
        "the compromised-host authorization bound is the broker's never-granted set"
    );

    // BEHAVIORAL assertion through the broker: a compromised host asking
    // the broker to grant it dispatch authority is refused, and the
    // refusal path is audited into the P7.33 chain.
    let chained = Arc::new(
        ChainedAuditSink::new("host-session-p7-34", Box::new(
            flexcore_plugin_host::broker::InMemoryBrokerAuditSink::new(),
        ))
        .expect("a non-empty session id composes"),
    );
    let broker = CapabilityBroker::new(Box::new(SharedChained(Arc::clone(&chained))));

    // 8a. The self-grant attempt: refused as never-grantable (the broker
    // re-refuses the never-granted set at grant time, regardless of what
    // the "manifest" declares). Shipped broker behavior: the grant-time
    // refusal happens BEFORE the audit write, so it produces no record —
    // the authority never existed to audit.
    let self_grant = broker.grant(
        "neryva.host-itself",
        "neryva.host-itself",
        "0.1.0",
        &GrantRequest {
            capability: "dispatch.authorize".to_string(),
            scope: CapabilityScope::DataScope {
                tenant_id: "t-1".to_string(),
                site_id: None,
                asset_id: None,
            },
            expires_at_unix_ms: None,
        },
        NOW,
    );
    let error = self_grant.expect_err("a compromised host cannot grant itself authority");
    assert_eq!(error.name(), "PERMISSION_NEVER_GRANTABLE");

    // 8b. ...and the direct check path refuses too (no grant exists, and
    // a never-granted capability cannot be checked into existence) — and
    // THIS refusal is audited (the broker audits check outcomes in both
    // directions).
    let direct_check = broker.check(
        "neryva.host-itself",
        "dispatch.authorize",
        &CapabilityScope::DataScope {
            tenant_id: "t-1".to_string(),
            site_id: None,
            asset_id: None,
        },
        NOW,
    );
    let error = direct_check.expect_err("the check path refuses the never-granted id");
    assert_eq!(error.name(), "PERMISSION_NEVER_GRANTABLE");

    // 8c. The default-deny bound for a GRANTABLE capability the host was
    // never granted (no manifest declared it here) — also audited.
    let ungranted = broker.check(
        "neryva.host-itself",
        "schedule.propose",
        &CapabilityScope::SendOnlyTarget {
            target: "scheduler-inbox".to_string(),
        },
        NOW,
    );
    assert_eq!(
        ungranted.expect_err("defaults deny").name(),
        "CAPABILITY_NOT_GRANTED"
    );

    // 8d. Every CHECK refusal was audited — and, through the chained
    // writer, is tamper-evident (the P7.33 evidence layer covers exactly
    // these host-issued denial records).
    let records = chained.chain_records();
    assert_eq!(
        records.len(),
        2,
        "the two refused check calls produced two audited denials"
    );
    assert!(
        records
            .iter()
            .all(|record| record.kind_name == BrokerAuditKind::CapabilityDenied.name()),
        "both records are capability denials"
    );
    assert!(
        records[0].detail.contains("PERMISSION_NEVER_GRANTABLE"),
        "the first denial names the never-granted refusal: {}",
        records[0].detail
    );
    assert!(
        records[1].detail.contains("CAPABILITY_NOT_GRANTED"),
        "the second denial names the default-deny: {}",
        records[1].detail
    );
    // The denial records themselves chain cleanly — the compromised host
    // cannot quietly edit its own denial trail either.
    assert!(
        first_broken_link(&records).is_none(),
        "the denial chain is intact"
    );
}
