// P8.32/P8.33 — integration tests for the model-artifact signing load gate
// and the research-worker identity/secret modules: the ledger's negative
// suites. Ledger `ledger/phase-08-advanced-coordination/ledger_phase_8.md`
// §P8.32/§P8.33; decision records:
// `ledger/phase-08-advanced-coordination/32_model_artifact_signing.md` and
// `ledger/phase-08-advanced-coordination/33_identity_mtls_secrets.md`.
//
// P8.32 verification method (the ledger's "tamper with a signed artifact
// and attempt load; confirm refusal"): a TAMPERED signed artifact, an
// UNSIGNED artifact, and a REVOKED artifact are each refused at the
// composed load gate (P8.31 serving register FIRST, then revocation, then
// provenance verification), and every refusal is audited.
//
// P8.33 verification method (the ledger's "a worker presenting a revoked
// identity is refused; a worker config containing a literal secret value
// is rejected at load and the attempt audited; no secret appears in worker
// logs"): the admission door refuses a revoked identity; the config scan
// refuses the literal-secret config; the refused VALUE never appears in
// any error text or audit detail.
//
// Honest status: WRITTEN-UNRUN — no cargo command was executed in this
// loop (the subtask's loop bans forbid build/test); the human build/test
// gate owns the run. Every expected value is hand-computed from the
// recorded rules — nothing is fabricated. The one numeric digest
// expectation is DEFINITIONAL: FNV-1a 64 over ZERO bytes equals the
// offset basis 0xcbf29ce484222325 (no bytes are processed).
//
// Control status: research data-plane integrity only (ADR-010). Nothing
// here authorizes, issues, approves, or dispatches anything.

use std::time::Duration;

use flexcore_research::artifact_signing::{
    parse_model_provenance_value, verify_model_artifact_at_load, MemoryModelArtifactAuditSink,
    ModelArtifactAuditKind, ModelArtifactRevocationRegister, ModelArtifactSigningError,
    MODEL_ARTIFACT_SIGNING_ERROR_NAMES, MODEL_PROVENANCE_SCHEMA_ID,
    NoArtifactVerifier, UnsafeTestOnlyArtifactDigest,
};
use flexcore_research::tenant_isolation::{
    ModelArtifactRegistration, ModelServingRegister, ResearchWorkerFamily, ResearchWorkerScope,
    WorkerScopeStamp,
};
use flexcore_research::validation::HarnessClock;
use flexcore_research::worker_identity::{
    parse_secret_declarations, scan_worker_config_audited, verify_worker_admission,
    MemoryWorkerIdentityAuditSink, ResearchWorkerIdentity, WorkerIdentityAuditKind,
    WorkerIdentityStore, WorkerIdentityRevocationRegister, WorkerMtlsTransportConfig,
    WorkerRevocationChecking, WorkerTlsMinimum, WORKER_IDENTITY_ERROR_NAMES,
};

/// A fixed injected clock (each module also defines one in its in-module
/// suite; this is the integration-suite's own).
struct FixedClock {
    now: i64,
}

impl HarnessClock for FixedClock {
    fn now_millis(&self) -> i64 {
        self.now
    }
}

const ARTIFACT_REF: &str = "artifact://models/gnn-warmstart.v1";

/// The FNV-1a 64 digest of EMPTY bytes is DEFINITIONALLY the offset basis
/// (no bytes are processed) — the hand-computable pinned digest used by
/// the load-gate tests. The tamper test mutates the ARTIFACT bytes (the
/// record still declares the empty-bytes digest), which is exactly the
/// ledger's tamper case.
const EMPTY_BYTES_FNV1A64: &str = "cbf29ce484222325";

/// The P8.31 registration: the artifact is owned by tenant-a (wide), the
/// training-data reference carried (the P8.35 record is referenced, not
/// copied).
fn serving_register() -> ModelServingRegister {
    let mut register = ModelServingRegister::new();
    register
        .register_artifact(ModelArtifactRegistration {
            artifact_ref: ARTIFACT_REF.to_string(),
            owner_family: ResearchWorkerFamily::Gnn,
            owner_scope: ResearchWorkerScope::from_authenticated_context("tenant-a", None)
                .expect("valid scope"),
            training_data_ref: "training-data://tenant-a/ds-1".to_string(),
        })
        .expect("the artifact registers");
    register
}

/// A worker stamped (by the host) for tenant-a, wide.
fn worker_stamp(worker_id: &str) -> WorkerScopeStamp {
    WorkerScopeStamp::stamp_by_host(
        worker_id,
        ResearchWorkerFamily::Gnn,
        ResearchWorkerScope::from_authenticated_context("tenant-a", None).expect("valid scope"),
    )
    .expect("the host stamps")
}

/// A provenance record bound to ARTIFACT_REF declaring the empty-bytes
/// digest under the test-grade algorithm.
fn provenance() -> flexcore_research::artifact_signing::ModelArtifactProvenance {
    let json = serde_json::json!({
        "provenance-schema": MODEL_PROVENANCE_SCHEMA_ID,
        "artifact-ref": ARTIFACT_REF,
        "digest-algorithm": "fnv1a64-test",
        "digest": EMPTY_BYTES_FNV1A64,
        "signer-key-reference": "key-store://release/model-signing",
        "signed-at-millis": 1_000u64,
        "worker-family": "gnn",
        "training-data-ref": "training-data://tenant-a/ds-1",
        "training-config-ref": "config://experiments/gnn-warmstart.v1",
        "framework-version-ref": "repro://runs/run-1/framework-versions"
    });
    parse_model_provenance_value(&json).expect("the fixture record parses")
}

// ---------------------------------------------------------------------------
// P8.32 — the load-gate negative suite (the ledger's verification method)
// ---------------------------------------------------------------------------

#[test]
fn tampered_signed_artifact_is_refused_at_load_and_audited() {
    let register = serving_register();
    let stamp = worker_stamp("worker/gnn-load-1");
    let serving_scope =
        ResearchWorkerScope::from_authenticated_context("tenant-a", None).expect("valid scope");
    let revocations = ModelArtifactRevocationRegister::new();
    let record = provenance();
    let clock = FixedClock { now: 20_000 };
    let audit = MemoryModelArtifactAuditSink::new();

    // The happy verification first: the exact empty bytes verify (the
    // digest matches DEFINITIONALLY).
    let acceptance = verify_model_artifact_at_load(
        &stamp,
        &register,
        &serving_scope,
        &revocations,
        Some(&record),
        ARTIFACT_REF,
        b"",
        &UnsafeTestOnlyArtifactDigest,
        &clock,
        &audit,
    )
    .expect("the untampered artifact loads");
    assert_eq!(acceptance.artifact_ref, ARTIFACT_REF);
    // The LINKED provenance travels with the acceptance — there is no
    // accepted load without it.
    assert_eq!(acceptance.training_data_ref, "training-data://tenant-a/ds-1");
    assert_eq!(acceptance.training_config_ref, "config://experiments/gnn-warmstart.v1");
    assert_eq!(acceptance.framework_version_ref, "repro://runs/run-1/framework-versions");

    // THE TAMPER: one byte changed — the record still declares the
    // empty-bytes digest, the artifact bytes no longer match. REFUSED.
    let err = verify_model_artifact_at_load(
        &stamp,
        &register,
        &serving_scope,
        &revocations,
        Some(&record),
        ARTIFACT_REF,
        b"tampered",
        &UnsafeTestOnlyArtifactDigest,
        &clock,
        &audit,
    )
    .expect_err("a tampered artifact is refused, never trusted");
    assert_eq!(err.name(), "MODEL_ARTIFACT_VERIFICATION_REFUSED");
    assert!(format!("{err}").contains("MODEL_DIGEST_MISMATCH"));

    // The audit trail: acceptance, then refusal — every outcome audited.
    let records = audit.records();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].kind, ModelArtifactAuditKind::LoadAccepted);
    assert_eq!(records[1].kind, ModelArtifactAuditKind::LoadRefused);
    assert_eq!(records[1].timestamp_millis, 20_000);
}

#[test]
fn unsigned_artifact_is_refused_at_load_and_audited() {
    let register = serving_register();
    let stamp = worker_stamp("worker/gnn-load-2");
    let serving_scope =
        ResearchWorkerScope::from_authenticated_context("tenant-a", None).expect("valid scope");
    let revocations = ModelArtifactRevocationRegister::new();
    let clock = FixedClock { now: 21_000 };
    let audit = MemoryModelArtifactAuditSink::new();
    // NO provenance record supplied: the unsigned case. Refused before any
    // use — never "treat as trusted".
    let err = verify_model_artifact_at_load(
        &stamp,
        &register,
        &serving_scope,
        &revocations,
        None,
        ARTIFACT_REF,
        b"",
        &UnsafeTestOnlyArtifactDigest,
        &clock,
        &audit,
    )
    .expect_err("an unsigned artifact is refused");
    assert_eq!(err.name(), "MODEL_ARTIFACT_UNSIGNED");
    let records = audit.records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, ModelArtifactAuditKind::LoadRefused);
    assert!(records[0].detail.contains("MODEL_ARTIFACT_UNSIGNED"));
}

#[test]
fn revoked_artifact_is_refused_even_with_valid_provenance() {
    let register = serving_register();
    let stamp = worker_stamp("worker/gnn-load-3");
    let serving_scope =
        ResearchWorkerScope::from_authenticated_context("tenant-a", None).expect("valid scope");
    let mut revocations = ModelArtifactRevocationRegister::new();
    let record = provenance();
    let clock = FixedClock { now: 22_000 };
    let audit = MemoryModelArtifactAuditSink::new();
    // The host revokes the artifact (reason + actor REQUIRED).
    revocations
        .revoke(ARTIFACT_REF, "tampered copy found in object storage", "security", &clock, &audit)
        .expect("the revocation records");
    let err = verify_model_artifact_at_load(
        &stamp,
        &mut register,
        &serving_scope,
        &revocations,
        Some(&record),
        ARTIFACT_REF,
        b"",
        &UnsafeTestOnlyArtifactDigest,
        &clock,
        &audit,
    )
    .expect_err("a revoked artifact is refused even with valid provenance");
    assert_eq!(err.name(), "MODEL_ARTIFACT_REVOKED");
    // Audit order: revocation recorded, then the load refusal.
    let records = audit.records();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].kind, ModelArtifactAuditKind::RevocationRecorded);
    assert_eq!(records[1].kind, ModelArtifactAuditKind::LoadRefused);
}

#[test]
fn cross_tenant_load_is_refused_by_the_p831_gate_before_signing() {
    // The composition rule pinned: the P8.31 gate runs FIRST. A tenant-b
    // worker requesting the tenant-a-owned artifact is refused with the
    // P8.31 name (unapproved cross-tenant serving) — the signing gate is
    // never even reached.
    let register = serving_register();
    let stamp_b = WorkerScopeStamp::stamp_by_host(
        "worker/gnn-load-4",
        ResearchWorkerFamily::Gnn,
        ResearchWorkerScope::from_authenticated_context("tenant-b", None).expect("valid scope"),
    )
    .expect("the host stamps");
    let serving_scope_b =
        ResearchWorkerScope::from_authenticated_context("tenant-b", None).expect("valid scope");
    let revocations = ModelArtifactRevocationRegister::new();
    let record = provenance();
    let clock = FixedClock { now: 23_000 };
    let audit = MemoryModelArtifactAuditSink::new();
    let err = verify_model_artifact_at_load(
        &stamp_b,
        &register,
        &serving_scope_b,
        &revocations,
        Some(&record),
        ARTIFACT_REF,
        b"",
        &UnsafeTestOnlyArtifactDigest,
        &clock,
        &audit,
    )
    .expect_err("cross-tenant load refused at the P8.31 gate");
    // The load error delegates its name to the inner P8.31 refusal, so
    // audit queries span both gates.
    assert_eq!(err.name(), "RESEARCH_MODEL_SERVING_NOT_APPROVED");
    assert_eq!(
        err,
        flexcore_research::artifact_signing::ModelArtifactLoadError::TenantIsolation(
            flexcore_research::tenant_isolation::TenantIsolationError::ModelServingNotApproved {
                artifact_ref: ARTIFACT_REF.to_string(),
                owner: "tenant:tenant-a|site:none".to_string(),
                requested: "tenant:tenant-b|site:none".to_string(),
            }
        )
    );
    assert_eq!(audit.records()[0].kind, ModelArtifactAuditKind::LoadRefused);
}

#[test]
fn the_default_verifier_refuses_everything_fail_closed() {
    // The fail-closed posture pinned end to end: even a perfectly formed
    // record over matching bytes does NOT load under the default — the
    // absence of signing infrastructure can never look like success.
    let register = serving_register();
    let stamp = worker_stamp("worker/gnn-load-5");
    let serving_scope =
        ResearchWorkerScope::from_authenticated_context("tenant-a", None).expect("valid scope");
    let revocations = ModelArtifactRevocationRegister::new();
    let record = provenance();
    let clock = FixedClock { now: 24_000 };
    let audit = MemoryModelArtifactAuditSink::new();
    let err = verify_model_artifact_at_load(
        &stamp,
        &register,
        &serving_scope,
        &revocations,
        Some(&record),
        ARTIFACT_REF,
        b"",
        &NoArtifactVerifier,
        &clock,
        &audit,
    )
    .expect_err("the default verifier refuses everything");
    assert_eq!(err.name(), "MODEL_ARTIFACT_VERIFICATION_REFUSED");
    assert!(format!("{err}").contains("MODEL_VERIFIER_REFUSED"));
}

#[test]
fn the_model_signing_error_name_table_is_closed_and_distinct() {
    let from_table: std::collections::BTreeSet<&'static str> =
        MODEL_ARTIFACT_SIGNING_ERROR_NAMES.into_iter().collect();
    assert_eq!(from_table.len(), MODEL_ARTIFACT_SIGNING_ERROR_NAMES.len());
    // The two refusal enums round-trip through their name() tables (the
    // in-module suites pin the exact table==enum equality).
    let _ = ModelArtifactSigningError::RecordNotAnObject.name();
}

// ---------------------------------------------------------------------------
// P8.33 — the identity/secret negative suite (the ledger's verification
// method)
// ---------------------------------------------------------------------------

fn worker_identity(worker_id: &str) -> ResearchWorkerIdentity {
    ResearchWorkerIdentity {
        worker_id: worker_id.to_string(),
        family: ResearchWorkerFamily::Marl,
        deployment_id: "deployment-lab-1".to_string(),
        certificate_reference: Some("file:///etc/flexcore/workers/worker.crt".to_string()),
        private_key_reference: Some("file:///etc/flexcore/workers/worker.key".to_string()),
    }
}

struct FixedIdentityStore {
    identity: Option<ResearchWorkerIdentity>,
}

impl WorkerIdentityStore for FixedIdentityStore {
    fn load_identity(
        &self,
    ) -> Result<Option<ResearchWorkerIdentity>, flexcore_research::worker_identity::WorkerIdentityError>
    {
        Ok(self.identity.clone())
    }
}

fn transport_config() -> WorkerMtlsTransportConfig {
    WorkerMtlsTransportConfig {
        platform_endpoint: "https://controlplane.internal:8443".to_string(),
        pinned_server_name: "controlplane.internal".to_string(),
        pinned_platform_ca_reference: Some(
            "file:///etc/flexcore/workers/platform-ca.pem".to_string(),
        ),
        tls_minimum: WorkerTlsMinimum::Tls13,
        session_credential_ttl: Duration::from_secs(300),
        rotation_owner: "platform-security".to_string(),
        expiry_monitor_enabled: true,
        revocation_checking: WorkerRevocationChecking::DeploymentEndpoint {
            reference: "file:///etc/flexcore/workers/revocation-endpoint".to_string(),
        },
    }
}

#[test]
fn a_revoked_worker_identity_is_refused_at_admission() {
    let store = FixedIdentityStore {
        identity: Some(worker_identity("worker/marl-it-9")),
    };
    let mut register = WorkerIdentityRevocationRegister::new();
    let clock = FixedClock { now: 30_000 };
    let audit = MemoryWorkerIdentityAuditSink::new();
    register
        .revoke("worker/marl-it-9", "instance compromised; key rotated", "platform-security", &clock, &audit)
        .expect("the revocation records");
    let err = verify_worker_admission(&store, &transport_config(), &register, &clock, &audit)
        .expect_err("a worker presenting a revoked identity is refused");
    assert_eq!(err.name(), "WORKER_IDENTITY_REVOKED");
    let records = audit.records();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].kind, WorkerIdentityAuditKind::RevocationRecorded);
    assert_eq!(records[1].kind, WorkerIdentityAuditKind::AdmissionRefused);
}

#[test]
fn a_worker_config_with_a_literal_secret_is_rejected_and_audited_without_echoing_the_value() {
    let clock = FixedClock { now: 31_000 };
    let audit = MemoryWorkerIdentityAuditSink::new();
    // The literal-secret config (the ledger's step-3 hostile case): the
    // credential sits as a VALUE under a credential-shaped key.
    let config = serde_json::json!({
        "worker": "marl",
        "db_password": "hunter2-the-literal-secret",
        "secrets": [
            { "name": "dataset-access", "reference": "secret-store://research/dataset-credentials" }
        ]
    });
    let err = scan_worker_config_audited(&config, &clock, &audit)
        .expect_err("a literal-secret config is rejected at load");
    assert_eq!(err.name(), "WORKER_CONFIG_SECRET_KEY_REFUSED");
    // The attempt is audited, and NO secret appears in the logs: the audit
    // detail and the error text carry the KEY NAME, never the VALUE.
    let records = audit.records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].kind, WorkerIdentityAuditKind::SecretConfigRefused);
    assert!(records[0].detail.contains("db_password"));
    assert!(!records[0].detail.contains("hunter2"));
    assert!(!format!("{err}").contains("hunter2"));
}

#[test]
fn a_schemeless_credential_reference_is_refused_as_likely_inline_secret() {
    // The sanctioned declaration path is the OTHER half of the rule: a
    // scheme-less reference under the closed {name, reference} key set is
    // treated as inline secret material and refused — the VALUE never
    // echoes back.
    let inline = serde_json::json!([
        { "name": "simulator-access", "reference": "plain-super-secret-value" }
    ]);
    let err = parse_secret_declarations(&inline).expect_err("inline secret refused");
    assert_eq!(err.name(), "WORKER_SECRET_REFERENCE_INVALID");
    assert!(!format!("{err}").contains("plain-super-secret-value"));
    // The sanctioned form parses: references only, host-resolved.
    let declared = serde_json::json!([
        { "name": "simulator-access", "reference": "secret-store://research/simulator-credentials" }
    ]);
    let parsed = parse_secret_declarations(&declared).expect("the sanctioned form parses");
    assert_eq!(parsed[0].reference, "secret-store://research/simulator-credentials");
}

#[test]
fn the_worker_identity_error_name_table_is_closed_and_distinct() {
    let from_table: std::collections::BTreeSet<&'static str> =
        WORKER_IDENTITY_ERROR_NAMES.into_iter().collect();
    assert_eq!(from_table.len(), WORKER_IDENTITY_ERROR_NAMES.len());
    assert!(from_table.contains("WORKER_IDENTITY_REVOKED"));
    assert!(from_table.contains("WORKER_CONFIG_SECRET_KEY_REFUSED"));
}
