// P10.04 — Authorization and tenant-isolation test suite (research-mirror
//         slice): the §16.2/§17.4 cross-tenant refusals over the
//         research-worker tenant-scope mirror
//         (`crates/flexcore-research/src/tenant_isolation.rs`, P8.31;
//         ledger P10.04; record
//         `ledger/phase-10-v1-release-hardening/04_authz_tenant_tests.md`).
//
// WHAT THIS FILE PROVES (the research-side tenant-scope enforcement, the
// mirror of the kernel's P7.39 `TenantScope` — the kernel-side refusals are
// proven in `crates/flexcore-kernel/tests/p10_04_tenant_scope_refusals.rs`):
//
// 1. THE CROSS-TENANT READ MATRIX, EXHAUSTIVE OVER THE WORKER FAMILIES.
//    Every one of the six closed worker families (admm_dmpc, gnn, marl,
//    andes, helics, portfolio_analytics) attempting to read EVERY
//    wrong-scope combination the narrowing predicate defines — other
//    tenant (tenant-wide and site-narrowed), same tenant other site,
//    un-narrowed request under a site-narrowed stamp — is refused with
//    `TENANT_SCOPE_MISMATCH` (the kernel-aligned stable name) and audited.
// 2. THE ECHO-MISMATCH / FORGERY REFUSALS. A worker echoing a wider or
//    different scope than the host-stamped one is
//    `TENANT_SCOPE_ECHO_MISMATCH`; a worker cannot widen its own scope.
//    (The stamp-binding digest is recomputed from private fields at every
//    enforcement step, so an externally forged stamp is unconstructible —
//    the compile-level half of the forgery proof, same discipline as the
//    domain `TenantContext`.)
// 3. FAIL-CLOSED. Unknown records, empty tenants, empty record ids and
//    unknown data kinds are named refusals — never "treat as in-scope".
// 4. CROSS-TENANT MODEL SERVING WITHOUT A RECORDED APPROVAL IS REFUSED,
//    and an in-tenant serving is admitted without any approval (the
//    approval is required ONLY when the serving scope escapes the owner
//    scope).
// 5. AUDIT: every refusal AND every admission is audited with the stable
//    audit kind names (§16.2 "Audit cross-tenant access denials").
//
// STATUS: WRITTEN-UNRUN (ENTRY_NOTE boundary 3; no cargo command was
// executed; the human build/test gate owns:
// `cargo test -p flexcore-research --test p10_04_research_tenant_isolation`).
// Every expected string below is copied from the module's own constants —
// nothing is fabricated.

use flexcore_research::tenant_isolation::{
    MemoryTenantIsolationAuditSink, ModelArtifactRegistration, ModelServingApproval,
    ModelServingRegister, ResearchDataKind, ResearchDataRecord, ResearchWorkerFamily,
    ResearchWorkerScope, ScopedResearchCatalog, TenantIsolationAuditKind, WorkerScopeStamp,
    KERNEL_TENANT_SCOPE_ALIGNMENT_SEAM, RESEARCH_WORKER_SCOPING_RULE, TENANT_ISOLATION_ERROR_NAMES,
    WORKER_FAMILIES,
};
use flexcore_research::validation::HarnessClock;

// --- Fixtures -----------------------------------------------------------------------

const TENANT_A: &str = "0192a5e0-8a01";
const TENANT_B: &str = "0192a5e0-8a02";
const SITE_1: &str = "0192a5e0-8a03";
const SITE_2: &str = "0192a5e0-8a04";

/// A fixed injected clock (the crate has ONE clock vocabulary —
/// `validation::HarnessClock`; no wall-clock read exists in these tests).
struct FixedClock {
    now: i64,
}

impl HarnessClock for FixedClock {
    fn now_millis(&self) -> i64 {
        self.now
    }
}

fn scope(tenant: &str, site: Option<&str>) -> ResearchWorkerScope {
    ResearchWorkerScope::from_authenticated_context(tenant, site).expect("valid scope fixture")
}

/// Host-stamps a worker of the given family with the given scope (the ONLY
/// stamping path — the host stamps from the authenticated context; the
/// worker receives the stamp and may echo it, never widen it).
fn stamp(worker_id: &str, family: ResearchWorkerFamily, tenant: &str, site: Option<&str>) -> WorkerScopeStamp {
    WorkerScopeStamp::stamp_by_host(worker_id, family, scope(tenant, site)).expect("valid stamp")
}

/// A catalog with records placed so every wrong-scope matrix case requests
/// a record whose catalog key EXISTS (the enforcement checks existence
/// before coverage — an unknown record would refuse with a different named
/// error, so the coverage refusals below must hit existing keys):
/// tenant A: site-1 telemetry, site-2 telemetry, tenant-wide training data;
/// tenant B: site-1 telemetry and tenant-wide telemetry (the out-of-scope
/// controls).
fn catalog() -> ScopedResearchCatalog {
    let mut c = ScopedResearchCatalog::new();
    c.register(ResearchDataRecord {
        kind: ResearchDataKind::Telemetry,
        tenant_id: TENANT_A.to_string(),
        site_id: Some(SITE_1.to_string()),
        record_id: "telemetry-site-1".to_string(),
        descriptor: "telemetry history for site 1".to_string(),
    })
    .expect("register site-1 telemetry");
    c.register(ResearchDataRecord {
        kind: ResearchDataKind::Telemetry,
        tenant_id: TENANT_A.to_string(),
        site_id: Some(SITE_2.to_string()),
        record_id: "telemetry-site-2".to_string(),
        descriptor: "telemetry history for site 2 (the cross-site control)".to_string(),
    })
    .expect("register site-2 telemetry");
    c.register(ResearchDataRecord {
        kind: ResearchDataKind::TrainingData,
        tenant_id: TENANT_A.to_string(),
        site_id: None,
        record_id: "training-tenant-wide".to_string(),
        descriptor: "tenant-wide training dataset".to_string(),
    })
    .expect("register tenant-wide training data");
    c.register(ResearchDataRecord {
        kind: ResearchDataKind::Telemetry,
        tenant_id: TENANT_B.to_string(),
        site_id: Some(SITE_1.to_string()),
        record_id: "telemetry-tenant-b-site-1".to_string(),
        descriptor: "tenant B telemetry — the cross-tenant control".to_string(),
    })
    .expect("register tenant-B site telemetry");
    c.register(ResearchDataRecord {
        kind: ResearchDataKind::Telemetry,
        tenant_id: TENANT_B.to_string(),
        site_id: None,
        record_id: "telemetry-tenant-b-wide".to_string(),
        descriptor: "tenant B tenant-wide telemetry — the cross-tenant control".to_string(),
    })
    .expect("register tenant-B tenant-wide telemetry");
    c
}

// --- 1. The cross-tenant read matrix, exhaustive over the worker families ------------

#[test]
fn every_worker_family_is_refused_every_wrong_scope_read() {
    // For EVERY family, a tenant-A/site-1-stamped worker is refused every
    // wrong-scope combination the narrowing predicate defines. The refusal
    // name is the kernel-aligned `TENANT_SCOPE_MISMATCH`.
    let c = catalog();
    for family in WORKER_FAMILIES {
        let family = ResearchWorkerFamily::parse(family).expect("closed family set");
        let worker = stamp("worker-matrix", family, TENANT_A, Some(SITE_1));
        // Other tenant, tenant-wide data (the record exists at that key).
        let err = c
            .enforce_scoped_read(&worker, ResearchDataKind::Telemetry, TENANT_B, None, "telemetry-tenant-b-wide")
            .expect_err("cross-tenant tenant-wide read refused");
        assert_eq!(err.name(), "TENANT_SCOPE_MISMATCH", "family {family:?}");
        // Other tenant, site-narrowed data (existing key).
        let err = c
            .enforce_scoped_read(&worker, ResearchDataKind::Telemetry, TENANT_B, Some(SITE_1), "telemetry-tenant-b-site-1")
            .expect_err("cross-tenant site read refused");
        assert_eq!(err.name(), "TENANT_SCOPE_MISMATCH", "family {family:?}");
        // Same tenant, OTHER site (the narrowed envelope covers only the
        // same site; the record exists at the site-2 key).
        let err = c
            .enforce_scoped_read(&worker, ResearchDataKind::Telemetry, TENANT_A, Some(SITE_2), "telemetry-site-2")
            .expect_err("cross-site read refused");
        assert_eq!(err.name(), "TENANT_SCOPE_MISMATCH", "family {family:?}");
        // Un-narrowed request under a site-narrowed envelope (the widening
        // attempt; the tenant-wide record exists).
        let err = c
            .enforce_scoped_read(&worker, ResearchDataKind::TrainingData, TENANT_A, None, "training-tenant-wide")
            .expect_err("tenant-wide read under a site-narrowed stamp refused");
        assert_eq!(err.name(), "TENANT_SCOPE_MISMATCH", "family {family:?}");
        // Narrowing is preserved: the equal-site read and the site request
        // inside a tenant-wide envelope pass.
        c.enforce_scoped_read(&worker, ResearchDataKind::Telemetry, TENANT_A, Some(SITE_1), "telemetry-site-1")
            .expect("the equal-site read passes");
        let wide_worker = stamp("worker-wide", family, TENANT_A, None);
        c.enforce_scoped_read(&wide_worker, ResearchDataKind::Telemetry, TENANT_A, Some(SITE_1), "telemetry-site-1")
            .expect("a site request inside a tenant-wide envelope passes");
    }
}

#[test]
fn the_closed_family_set_is_exactly_the_recorded_six() {
    // The scoping rule is per request and the family set is CLOSED: a
    // family outside the six is not a worker at all.
    assert_eq!(WORKER_FAMILIES, ["admm_dmpc", "gnn", "marl", "andes", "helics", "portfolio_analytics"]);
    for family in WORKER_FAMILIES {
        assert_eq!(ResearchWorkerFamily::parse(family).map(|f| f.as_str()), Some(family));
    }
    for intruder in ["generic", "superworker", "", "ADMM_DMPC"] {
        assert_eq!(ResearchWorkerFamily::parse(intruder), None, "{intruder:?}");
    }
    assert!(RESEARCH_WORKER_SCOPING_RULE.contains("training data, model artifacts, and inference \
             inputs are all scoped"));
}

// --- 2. Echo-mismatch / forgery refusals ---------------------------------------------

#[test]
fn a_worker_echoing_a_wider_or_different_scope_is_refused() {
    let stamped = stamp("worker-echo", ResearchWorkerFamily::Gnn, TENANT_A, Some(SITE_1));
    // The exact echo is honoured.
    stamped.verify_worker_echo(&scope(TENANT_A, Some(SITE_1))).expect("exact echo honoured");
    // Wider (drops the site narrowing): the cannot-widen refusal.
    let err = stamped
        .verify_worker_echo(&scope(TENANT_A, None))
        .expect_err("widened echo refused");
    assert_eq!(err.name(), "TENANT_SCOPE_ECHO_MISMATCH");
    // Different site.
    assert_eq!(
        stamped
            .verify_worker_echo(&scope(TENANT_A, Some(SITE_2)))
            .expect_err("cross-site echo refused")
            .name(),
        "TENANT_SCOPE_ECHO_MISMATCH"
    );
    // Different tenant — and the error names BOTH scope renderings (no
    // payload data, only scope renderings).
    let err = stamped
        .verify_worker_echo(&scope(TENANT_B, Some(SITE_1)))
        .expect_err("cross-tenant echo refused");
    assert_eq!(err.name(), "TENANT_SCOPE_ECHO_MISMATCH");
    let text = err.to_string();
    assert!(text.contains("tenant:0192a5e0-8a01|site:0192a5e0-8a03"));
    assert!(text.contains("tenant:0192a5e0-8a02|site:0192a5e0-8a03"));
}

#[test]
fn a_scope_cannot_be_built_from_empty_or_absent_tenant_data() {
    // Host-stamped construction only: an empty/blank tenant, an empty site
    // string, or an oversized id is a named refusal — there is no way to
    // assemble a scope from request data (the compile-level half of the
    // forgery proof: the fields are private, so a forged scope struct is a
    // compile error, and the constructor refuses the malformed inputs).
    assert_eq!(
        ResearchWorkerScope::from_authenticated_context("", None)
            .expect_err("no tenant")
            .name(),
        "RESEARCH_SCOPE_INVALID"
    );
    assert_eq!(
        ResearchWorkerScope::from_authenticated_context("   ", None)
            .expect_err("blank tenant")
            .name(),
        "RESEARCH_SCOPE_INVALID"
    );
    assert_eq!(
        ResearchWorkerScope::from_authenticated_context(TENANT_A, Some(""))
            .expect_err("empty site string")
            .name(),
        "RESEARCH_SCOPE_INVALID"
    );
    assert_eq!(
        WorkerScopeStamp::stamp_by_host("", ResearchWorkerFamily::Marl, scope(TENANT_A, None))
            .expect_err("empty worker id")
            .name(),
        "RESEARCH_WORKER_STAMP_INVALID"
    );
}

// --- 3. Fail-closed: unknown / malformed requests are named refusals ------------------

#[test]
fn unknown_records_and_malformed_reads_are_fail_closed_never_in_scope() {
    let c = catalog();
    let worker = stamp("worker-fc", ResearchWorkerFamily::Andes, TENANT_A, None);
    // Unknown record: refused (never "treat as in-scope").
    let err = c
        .enforce_scoped_read(&worker, ResearchDataKind::Telemetry, TENANT_A, None, "no-such-record")
        .expect_err("unknown record refused");
    assert_eq!(err.name(), "RESEARCH_DATA_RECORD_UNKNOWN");
    // Empty tenant in the READ request: refused before any data question.
    let err = c
        .enforce_scoped_read(&worker, ResearchDataKind::Telemetry, "", None, "telemetry-site-1")
        .expect_err("unnamed tenant refused");
    assert_eq!(err.name(), "RESEARCH_SCOPE_INVALID");
    // Empty site string: refused (use None for tenant-wide).
    let err = c
        .enforce_scoped_read(&worker, ResearchDataKind::Telemetry, TENANT_A, Some(""), "telemetry-site-1")
        .expect_err("empty site string refused");
    assert_eq!(err.name(), "RESEARCH_SCOPE_INVALID");
    // Empty record id: the catalog key never matches → unknown, refused.
    let err = c
        .enforce_scoped_read(&worker, ResearchDataKind::Telemetry, TENANT_A, None, "")
        .expect_err("empty record id refused");
    assert_eq!(err.name(), "RESEARCH_DATA_RECORD_UNKNOWN");
    // The data-kind vocabulary is closed: unknown kinds never guessed.
    for kind in ["telemetry", "topology", "training_data", "model_artifact", "scenario"] {
        assert!(ResearchDataKind::parse(kind).is_some(), "{kind}");
    }
    for intruder in ["secrets", "credentials", "", "TELEMETRY"] {
        assert_eq!(ResearchDataKind::parse(intruder), None, "{intruder:?}");
    }
}

// --- 4. Cross-tenant model serving without a recorded approval is refused --------------

#[test]
fn unapproved_cross_tenant_model_serving_is_refused_and_approved_serving_is_admitted() {
    let mut register = ModelServingRegister::new();
    let clock = FixedClock { now: 1_700_000_000_000 };
    let audit = MemoryTenantIsolationAuditSink::new();
    // The GNN model trained on tenant A's data (owner scope tenant A).
    register
        .register_artifact(ModelArtifactRegistration {
            artifact_ref: "artifact://models/gnn-warmstart.v1".to_string(),
            owner_family: ResearchWorkerFamily::Gnn,
            owner_scope: scope(TENANT_A, None),
            training_data_ref: "dataset://tenant-a/topologies".to_string(),
        })
        .expect("register artifact");
    // A tenant-A worker may serve WITHIN tenant A with no approval.
    let worker = stamp("worker-serve", ResearchWorkerFamily::Gnn, TENANT_A, None);
    register
        .enforce_model_serving(&worker, "artifact://models/gnn-warmstart.v1", &scope(TENANT_A, Some(SITE_1)))
        .expect("in-tenant serving needs no approval");
    // Serving to tenant B WITHOUT a recorded approval: refused.
    let err = register
        .enforce_model_serving(&worker, "artifact://models/gnn-warmstart.v1", &scope(TENANT_B, None))
        .expect_err("unapproved cross-tenant serving refused");
    assert_eq!(err.name(), "RESEARCH_MODEL_SERVING_NOT_APPROVED");
    // With the recorded approval (reason + actor required), serving is
    // admitted. An unattributed approval is refused, never recorded.
    let approval = ModelServingApproval {
        artifact_ref: "artifact://models/gnn-warmstart.v1".to_string(),
        serving_scope: scope(TENANT_B, None),
        reason: String::new(),
        actor: "security-review".to_string(),
        approved_at_millis: 0,
    };
    let err = register
        .record_approval(approval, &clock, &audit)
        .expect_err("an approval without a reason is refused");
    assert_eq!(err.name(), "RESEARCH_APPROVAL_REASON_REQUIRED");
    let approval = ModelServingApproval {
        artifact_ref: "artifact://models/gnn-warmstart.v1".to_string(),
        serving_scope: scope(TENANT_B, None),
        reason: "cross-tenant pilot evaluation, decision record ref".to_string(),
        actor: String::new(),
        approved_at_millis: 0,
    };
    let err = register
        .record_approval(approval, &clock, &audit)
        .expect_err("an approval without an actor is refused");
    assert_eq!(err.name(), "RESEARCH_APPROVAL_ACTOR_REQUIRED");
    let approval = ModelServingApproval {
        artifact_ref: "artifact://models/gnn-warmstart.v1".to_string(),
        serving_scope: scope(TENANT_B, None),
        reason: "cross-tenant pilot evaluation, decision record ref".to_string(),
        actor: "security-review".to_string(),
        approved_at_millis: 0,
    };
    register
        .record_approval(approval, &clock, &audit)
        .expect("the attributed approval is recorded");
    register
        .enforce_model_serving(&worker, "artifact://models/gnn-warmstart.v1", &scope(TENANT_B, None))
        .expect("serving with the recorded approval passes");
    // An approval for an UNKNOWN artifact is refused (fail-closed).
    let bogus = ModelServingApproval {
        artifact_ref: "artifact://models/does-not-exist".to_string(),
        serving_scope: scope(TENANT_B, None),
        reason: "r".to_string(),
        actor: "a".to_string(),
        approved_at_millis: 0,
    };
    let err = register
        .record_approval(bogus, &clock, &audit)
        .expect_err("unknown artifact approval refused");
    assert_eq!(err.name(), "RESEARCH_MODEL_ARTIFACT_UNKNOWN");
}

// --- 5. Audit: every refusal AND admission is recorded -------------------------------

#[test]
fn every_scoped_read_outcome_is_audited_with_the_stable_kinds() {
    let c = catalog();
    let clock = FixedClock { now: 1_700_000_001_000 };
    let audit = MemoryTenantIsolationAuditSink::new();
    let worker = stamp("worker-audit", ResearchWorkerFamily::Helics, TENANT_A, Some(SITE_1));
    // One admission ...
    c.scoped_read_audited(
        &worker,
        ResearchDataKind::Telemetry,
        TENANT_A,
        Some(SITE_1),
        "telemetry-site-1",
        &clock,
        &audit,
    )
    .expect("admitted");
    // ... and two refusals (cross-tenant; unknown record).
    assert!(c
        .scoped_read_audited(
            &worker,
            ResearchDataKind::Telemetry,
            TENANT_B,
            Some(SITE_1),
            "telemetry-tenant-b-site-1",
            &clock,
            &audit,
        )
        .is_err());
    assert!(c
        .scoped_read_audited(
            &worker,
            ResearchDataKind::Telemetry,
            TENANT_A,
            Some(SITE_1),
            "no-such-record",
            &clock,
            &audit,
        )
        .is_err());
    let records = audit.records();
    assert_eq!(records.len(), 3, "every outcome is audited");
    assert_eq!(records[0].kind, TenantIsolationAuditKind::ScopeReadAdmitted);
    assert_eq!(records[0].kind.name(), "RESEARCH_SCOPE_READ_ADMITTED");
    assert_eq!(records[1].kind, TenantIsolationAuditKind::ScopeReadRefused);
    assert_eq!(records[1].kind.name(), "RESEARCH_SCOPE_READ_REFUSED");
    assert!(
        records[1].detail.contains("TENANT_SCOPE_MISMATCH"),
        "the refusal class names the cross-tenant mismatch: {}",
        records[1].detail
    );
    assert_eq!(records[2].kind, TenantIsolationAuditKind::ScopeReadRefused);
    assert!(records[2].detail.contains("RESEARCH_DATA_RECORD_UNKNOWN"));
    // Timestamps come ONLY from the injected clock.
    for record in records.iter() {
        assert_eq!(record.timestamp_millis, 1_700_000_001_000);
    }
}

// --- 6. Alignment and error-name tables ----------------------------------------------

#[test]
fn the_kernel_alignment_seam_and_error_name_table_are_recorded() {
    // The mirror is recorded (no flexcore-kernel dependency; if the kernel
    // shape changes, the mirror update is deliberate).
    assert!(KERNEL_TENANT_SCOPE_ALIGNMENT_SEAM.contains("tenant_scope.rs::TenantScope"));
    // The error-name table is exactly the recorded 15 names (the module's
    // own test-side consistency set).
    assert_eq!(TENANT_ISOLATION_ERROR_NAMES.len(), 15);
    assert!(TENANT_ISOLATION_ERROR_NAMES.contains(&"TENANT_SCOPE_MISMATCH"));
    assert!(TENANT_ISOLATION_ERROR_NAMES.contains(&"TENANT_SCOPE_ECHO_MISMATCH"));
    assert!(TENANT_ISOLATION_ERROR_NAMES.contains(&"RESEARCH_MODEL_SERVING_NOT_APPROVED"));
}
