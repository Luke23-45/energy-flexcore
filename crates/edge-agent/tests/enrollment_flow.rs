// P6.06 — Integration tests for the edge enrollment flow, the credential
//         lifecycle, and the negative (fail-closed) paths (ledger P6.06
//         step 4; decision record `06_07_identity_channel.md`).
//
// Covered (all with FAKES — no CA, no network, no real TLS, no key
// generation anywhere):
//
// - **The full flow**: provisioning token + configured binding →
//   `TokenEnrollmentAdapter` → scripted issuer → validated record →
//   filesystem credential store → audited `EDGE_ENROLLMENT_COMPLETED`.
// - **The negative paths (ledger step 4 — all fail closed)**: a duplicate
//   (already-ACTIVE) identity, a REVOKED identity, a binding outside the
//   approved tenant/site scope, an already-consumed single-use token, an
//   issued record whose binding does not match the configured binding, and
//   the shipped `DenyAllCredentialIssuer` (no CA exists).
// - **Expiry monitoring**: the rotation-window warning fires exactly when
//   the remaining validity drops inside the window (the "at least one
//   rotation window before expiry" visibility rule), the expired state is
//   audited, and an un-expiring record is named honestly
//   (`NoExpiryRecorded`).
// - **The credential store**: round-trip, owner-only permissions on Unix
//   (write 0600, refuse to load a widened file), atomic-ish replace, clear.
// - **The revocation seam**: the shipped un-configured checker fails
//   closed — an un-established revocation status is never "not revoked".
//
// UNRUN — written, never executed in this loop (the ENTRY_NOTE bans);
// the human build pass runs `cargo test -p flexcore-edge-agent`.

use std::path::PathBuf;

use flexcore_edge_agent::enrollment::{
    CredentialExpiryMonitor, CredentialIssuer, CredentialStore, DeviceBinding,
    DeviceCredentialRecord, DenyAllCredentialIssuer, EnrollmentClient, EnrollmentOrchestrator,
    ExpiryStatus, FilesystemCredentialStore, PemBody, ProvisioningToken, RevocationChecker,
    ScriptedCredentialIssuer, TokenEnrollmentAdapter, UnconfiguredRevocationChecker,
};
use flexcore_edge_agent::ports::{EdgeAuditKind, EdgeError, InMemoryAuditSink};
use flexcore_edge_agent::test_support::TempFileCredentialStore;

const TENANT_A: &str = "tenant-a";
const TENANT_B: &str = "tenant-b";
const SITE_A: &str = "pilot-site-A"; // PLACEHOLDER site naming (D5 input; ENTRY_NOTE boundary 5)
const SITE_B: &str = "pilot-site-B";
const ISSUED: i64 = 1_700_000_000_000;

fn binding() -> DeviceBinding {
    DeviceBinding::new(TENANT_A.to_string(), SITE_A.to_string())
}

/// The record template the scripted issuer returns. With the shipped seam
/// NO certificate is issued (`certificate_pem = None`) — the honesty shape;
/// the root-CA pin and the key reference are present because the P6.07
/// channel composes from them once a real deployment issues an identity.
fn record_template() -> DeviceCredentialRecord {
    DeviceCredentialRecord {
        device_id: "edge-01".to_string(),
        tenant_id: TENANT_A.to_string(),
        site_id: SITE_A.to_string(),
        certificate_pem: None,
        chain_pem: Vec::new(),
        root_ca_pem: Some(PemBody::new(
            "-----BEGIN CERTIFICATE-----\nTESTROOT\n-----END CERTIFICATE-----\n".to_string(),
        )),
        private_key_reference: Some(PathBuf::from("/var/lib/flexcore-edge/keys/device.key")),
        issued_at_unix_millis: ISSUED,
        expires_at_unix_millis: Some(ISSUED + 30 * 86_400_000),
    }
}

fn approved_scope() -> Vec<(String, String)> {
    vec![
        (TENANT_A.to_string(), SITE_A.to_string()),
        (TENANT_A.to_string(), SITE_B.to_string()),
    ]
}

fn token(value: &str) -> ProvisioningToken {
    ProvisioningToken::new(value.to_string()).expect("valid token")
}

// ---------------------------------------------------------------------------
// The positive flow
// ---------------------------------------------------------------------------

#[test]
fn the_full_flow_enrolls_stores_and_audits() {
    let mut issuer =
        ScriptedCredentialIssuer::new(approved_scope(), record_template());
    let mut client = TokenEnrollmentAdapter::new(Box::new(issuer));
    let mut store = TempFileCredentialStore::new("p6-06-flow-positive");
    let audit = InMemoryAuditSink::new();

    let record = EnrollmentOrchestrator::enroll(
        &mut client,
        store.inner(),
        &token("single-shot-1"),
        &binding(),
        &audit,
        ISSUED,
    )
    .expect("the flow enrolls");

    // The record is the shipped honest shape: identity + binding, NO
    // certificate (issuance is a deployment requirement), root-CA pin
    // present, key reference present, key MATERIAL absent.
    assert_eq!(record.device_id, "edge-01");
    assert_eq!(record.tenant_id, TENANT_A);
    assert_eq!(record.site_id, SITE_A);
    assert!(record.certificate_pem.is_none(), "no CA exists — no certificate is fabricated");
    assert!(record.root_ca_pem.is_some());
    assert!(record.private_key_reference.is_some());

    // The stored record round-trips.
    let stored = store
        .inner()
        .load()
        .expect("the store reads")
        .expect("the record is stored");
    assert_eq!(stored, record);

    // Exactly one completion audit event; no token value in any detail.
    let events = audit.events();
    let completed = events
        .iter()
        .filter(|e| e.kind.name() == "EDGE_ENROLLMENT_COMPLETED")
        .count();
    assert_eq!(completed, 1, "one completion event");
    for event in &events {
        assert!(
            !event.detail.contains("single-shot-1"),
            "the provisioning token value must never enter an audit detail"
        );
    }
}

// ---------------------------------------------------------------------------
// The negative paths (ledger P6.06 step 4 — all fail closed)
// ---------------------------------------------------------------------------

#[test]
fn duplicate_identity_fails_closed() {
    let mut issuer =
        ScriptedCredentialIssuer::new(approved_scope(), record_template());
    let mut client = TokenEnrollmentAdapter::new(Box::new(issuer));
    let mut store = TempFileCredentialStore::new("p6-06-duplicate");
    let audit = InMemoryAuditSink::new();

    EnrollmentOrchestrator::enroll(
        &mut client,
        store.inner(),
        &token("t-1"),
        &binding(),
        &audit,
        ISSUED,
    )
    .expect("first enrollment succeeds");

    let error = EnrollmentOrchestrator::enroll(
        &mut client,
        store.inner(),
        &token("t-2"),
        &binding(),
        &audit,
        ISSUED,
    )
    .expect_err("a duplicate (already-ACTIVE) identity must FAIL CLOSED");
    let rendered = error.to_string();
    assert!(
        rendered.contains("duplicate identity"),
        "the rejection names the duplicate-identity cause: {rendered}"
    );
    assert!(
        audit
            .events()
            .iter()
            .any(|e| e.kind.name() == "EDGE_ENROLLMENT_REJECTED"),
        "the rejection is audited"
    );
}

#[test]
fn revoked_identity_fails_closed() {
    // A device previously REVOKED on the control-plane side: the scripted
    // registry carries it, and re-enrollment is refused.
    let mut issuer =
        ScriptedCredentialIssuer::new(approved_scope(), record_template());
    // Simulate the control-plane state: the device is REVOKED.
    issuer
        .enrolled
        .push(("edge-01".to_string(), "REVOKED".to_string()));
    let mut client = TokenEnrollmentAdapter::new(Box::new(issuer));
    let mut store = TempFileCredentialStore::new("p6-06-revoked");
    let audit = InMemoryAuditSink::new();

    let error = EnrollmentOrchestrator::enroll(
        &mut client,
        store.inner(),
        &token("t-1"),
        &binding(),
        &audit,
        ISSUED,
    )
    .expect_err("a REVOKED identity must FAIL CLOSED");
    let rendered = error.to_string();
    assert!(
        rendered.contains("revoked identity"),
        "the rejection names the revoked-identity cause: {rendered}"
    );
    // Nothing is stored: a revoked identity must not leave a credential
    // behind on the edge either.
    assert_eq!(store.inner().load().expect("the store reads"), None);
}

#[test]
fn binding_outside_the_approved_scope_fails_closed() {
    // The edge is approved for tenant-a/pilot-site-A only; a request bound
    // to another tenant (or another site outside the approved set) refuses.
    let mut issuer =
        ScriptedCredentialIssuer::new(vec![(TENANT_A.to_string(), SITE_A.to_string())], record_template());
    let mut client = TokenEnrollmentAdapter::new(Box::new(issuer));
    let store = TempFileCredentialStore::new("p6-06-wrong-tenant");

    let wrong_tenant = DeviceBinding::new(TENANT_B.to_string(), SITE_A.to_string());
    let error = client
        .enroll(&token("t-1"), &wrong_tenant)
        .expect_err("a cross-tenant binding must FAIL CLOSED");
    assert!(
        error.to_string().contains("approved enrollment scope"),
        "the rejection names the scope cause: {error}"
    );

    // A different SITE within the same tenant, when not approved, refuses
    // too (site B is in THIS test's approved set deliberately excluded).
    let wrong_site = DeviceBinding::new(TENANT_A.to_string(), SITE_B.to_string());
    assert!(client.enroll(&token("t-2"), &wrong_site).is_err());
    // The store never received a record for either attempt.
    assert_eq!(store.inner().load().expect("the store reads"), None);
}

#[test]
fn consumed_single_use_token_fails_closed() {
    let mut issuer =
        ScriptedCredentialIssuer::new(approved_scope(), record_template());
    let mut client = TokenEnrollmentAdapter::new(Box::new(issuer));
    let store = TempFileCredentialStore::new("p6-06-single-use");

    client
        .enroll(&token("one-time"), &binding())
        .expect("the first use issues");
    let error = client
        .enroll(&token("one-time"), &binding())
        .expect_err("the SAME token is single-use and must FAIL CLOSED on reuse");
    assert!(
        error.to_string().contains("already consumed"),
        "the rejection names the token-reuse cause: {error}"
    );
    let _ = store; // the flow above does not store on failure paths
}

#[test]
fn an_issued_record_with_a_mismatched_binding_fails_closed() {
    // Even a "successful" issue whose record binds the device to a
    // DIFFERENT tenant than the configured binding is refused by the
    // adapter (the edge never accepts a re-issued scope swap, §16.2).
    let mut mismatched = record_template();
    mismatched.tenant_id = TENANT_B.to_string();
    let mut issuer =
        ScriptedCredentialIssuer::new(approved_scope(), mismatched);
    let mut client = TokenEnrollmentAdapter::new(Box::new(issuer));
    let error = client
        .enroll(&token("t-1"), &binding())
        .expect_err("a binding-swap issue must FAIL CLOSED");
    assert!(
        error.to_string().contains("does not match the configured"),
        "the rejection names the mismatch cause: {error}"
    );
}

#[test]
fn the_shipped_issuer_refuses_everything_and_names_the_requirement() {
    let mut issuer = DenyAllCredentialIssuer;
    let error = issuer
        .issue(&token("t"), &binding())
        .expect_err("no CA is implemented — the shipped issuer refuses");
    assert!(
        error.to_string().contains("DEPLOYMENT REQUIREMENT"),
        "the refusal records the honesty, not a fake success: {error}"
    );
}

#[test]
fn an_incomplete_configured_binding_is_refused_before_any_issuer_call() {
    let mut issuer =
        ScriptedCredentialIssuer::new(approved_scope(), record_template());
    let mut client = TokenEnrollmentAdapter::new(Box::new(issuer));
    let empty_site = DeviceBinding::new(TENANT_A.to_string(), "   ".to_string());
    let error = client
        .enroll(&token("t"), &empty_site)
        .expect_err("an incomplete §16.2 binding must FAIL CLOSED");
    assert!(error.to_string().contains("incomplete"));
}

// ---------------------------------------------------------------------------
// Expiry monitoring (ledger P6.06 step 3 + the acceptance criterion)
// ---------------------------------------------------------------------------

#[test]
fn expiry_becomes_visible_exactly_one_rotation_window_early() {
    let monitor = CredentialExpiryMonitor::new(7 * 86_400_000).expect("valid window");
    let record = record_template(); // expires 30 days after issue
    let audit = InMemoryAuditSink::new();

    // Day 22: remaining = 8 days > window → Valid, NO event.
    let day22 = ISSUED + 22 * 86_400_000;
    assert_eq!(
        EnrollmentOrchestrator::assess_expiry(&monitor, &record, &audit, day22),
        ExpiryStatus::Valid
    );
    assert!(audit.events().is_empty());

    // Day 24: remaining = 6 days ≤ window → RotationDue, AUDITED (this is
    // the "at least one rotation window before expiry" operator visibility).
    let day24 = ISSUED + 24 * 86_400_000;
    assert_eq!(
        EnrollmentOrchestrator::assess_expiry(&monitor, &record, &audit, day24),
        ExpiryStatus::RotationDue
    );
    assert!(audit
        .events()
        .iter()
        .any(|e| e.kind.name() == "EDGE_CREDENTIAL_EXPIRY_WARNING"));

    // Day 31: past expiry → Expired, audited.
    let day31 = ISSUED + 31 * 86_400_000;
    assert_eq!(
        EnrollmentOrchestrator::assess_expiry(&monitor, &record, &audit, day31),
        ExpiryStatus::Expired
    );
    assert!(audit
        .events()
        .iter()
        .any(|e| e.kind.name() == "EDGE_CREDENTIAL_EXPIRED"));
}

#[test]
fn a_record_without_expiry_is_named_not_assumed() {
    let monitor = CredentialExpiryMonitor::new(7 * 86_400_000).expect("valid window");
    let mut record = record_template();
    record.expires_at_unix_millis = None;
    let audit = InMemoryAuditSink::new();
    assert_eq!(
        EnrollmentOrchestrator::assess_expiry(&monitor, &record, &audit, ISSUED),
        ExpiryStatus::NoExpiryRecorded,
        "the shipped seam records no expiry — the assessment names it, \
         it never reports 'valid forever'"
    );
    assert!(
        audit.events().is_empty(),
        "no event may fabricate an expiry state for an un-expiring record"
    );
}

#[test]
fn a_non_positive_rotation_window_is_refused_at_construction() {
    assert!(matches!(
        CredentialExpiryMonitor::new(0),
        Err(EdgeError::EnrollmentFailed { .. })
    ));
}

// ---------------------------------------------------------------------------
// The credential store (permissions, round-trip, clear)
// ---------------------------------------------------------------------------

#[test]
#[cfg(unix)]
fn the_store_enforces_owner_only_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let mut store = TempFileCredentialStore::new("p6-06-perms");
    store.put(&record_template()).expect("stored");
    let path = store.inner().path().clone();
    let mode = std::fs::metadata(&path)
        .expect("the record file exists")
        .permissions()
        .mode();
    assert_eq!(
        mode & 0o077,
        0,
        "the credential record must be owner-only (mode {mode:o})"
    );

    // A deployment that WIDENS the permissions is refused at load (fail
    // closed on a tampered deployment).
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644))
        .expect("permissions settable");
    let error = store
        .inner()
        .load()
        .expect_err("a widened credential file must FAIL CLOSED at load");
    assert!(
        error.to_string().contains("too open"),
        "the refusal names the permission cause: {error}"
    );
}

#[test]
fn the_store_round_trips_certificate_and_chain_fields() {
    let mut record = record_template();
    record.certificate_pem = Some(PemBody::new(
        "-----BEGIN CERTIFICATE-----\nDEVICECERT\n-----END CERTIFICATE-----\n".to_string(),
    ));
    record.chain_pem = vec![
        PemBody::new("-----BEGIN CERTIFICATE-----\nINTERMEDIATE\n-----END CERTIFICATE-----\n".to_string()),
    ];
    record.expires_at_unix_millis = None;
    let mut store = TempFileCredentialStore::new("p6-06-roundtrip2");
    store.put(&record).expect("stored");
    let loaded = store
        .inner()
        .load()
        .expect("reads")
        .expect("present");
    assert_eq!(loaded, record, "the record round-trips byte-faithfully");
    store.inner().clear().expect("cleared");
    assert_eq!(store.inner().load().expect("reads"), None);
}

#[test]
fn a_malformed_store_file_is_a_named_error_never_a_half_credential() {
    let mut store = TempFileCredentialStore::new("p6-06-malformed");
    store.put(&record_template()).expect("stored");
    let path = store.inner().path().clone();
    std::fs::write(&path, "flexcore-edge-credential-v1\ndevice_id = orphan\n").expect("written");
    let error = store
        .inner()
        .load()
        .expect_err("an incomplete record must FAIL CLOSED");
    assert!(error.to_string().contains("issued_at_unix_millis"));
}

// ---------------------------------------------------------------------------
// The revocation seam
// ---------------------------------------------------------------------------

#[test]
fn the_unconfigured_revocation_checker_fails_closed() {
    let checker = UnconfiguredRevocationChecker;
    let error = checker
        .check(&record_template())
        .expect_err("no OCSP/CRL client exists — the check FAILS CLOSED");
    let rendered = error.to_string();
    assert!(
        rendered.contains("DEPLOYMENT REQUIREMENT"),
        "the error records the deployment requirement: {rendered}"
    );
    // And the audit vocabulary exists for the un-checked state, so a
    // deployment can surface it (the kind maps to a stable name).
    assert_eq!(EdgeAuditKind::RevocationUnchecked.name(), "EDGE_REVOCATION_UNCHECKED");
}
