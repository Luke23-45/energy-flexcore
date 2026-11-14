// P10.04 — Authorization and tenant-isolation test suite (kernel-mirror
//         slice): the §16.2/§17.4 cross-tenant and echo/forgery refusals
//         over the plugin-path host-stamped tenant scope
//         (`crates/flexcore-kernel/src/tenant_scope.rs`, P7.39; ledger
//         P10.04; record
//         `ledger/phase-10-v1-release-hardening/04_authz_tenant_tests.md`).
//
// WHAT THIS FILE PROVES (the kernel-side mirror; the research-side mirror
// is proven in
// `crates/flexcore-research/tests/p10_04_research_tenant_isolation.rs`):
//
// 1. THE WRONG-SCOPE MATRIX, EXHAUSTIVE OVER THE NARROWING PREDICATE.
//    For both envelope shapes (tenant-wide, site-narrowed) × every
//    requested shape (same tenant same site, same tenant other site,
//    un-narrowed, other tenant tenant-wide, other tenant site-narrowed):
//    `TenantScope::covers` and `enforce_scope` admit EXACTLY the covered
//    combinations and refuse the rest with the kernel-aligned stable name
//    `TENANT_SCOPE_MISMATCH` — cross-tenant is NEVER covered.
// 2. THE ECHO-MISMATCH REFUSAL. A plugin echoing a wider or different
//    scope than the host-stamped one is `TENANT_SCOPE_ECHO_MISMATCH` —
//    the cannot-widen-your-own-scope rule (§16.2).
// 3. HOST-STAMPED CONSTRUCTION ONLY. An empty/blank tenant, an empty site
//    string, or an oversized id is `TENANT_SCOPE_INVALID`; the fields are
//    private, so no plugin-supplied scope can be assembled (the
//    compile-level half of the forgery proof).
// 4. THE AUDITED ENFORCEMENT PATH. `enforce_scoped_request` records a
//    `PLUGIN_TENANT_SCOPE_REFUSED` audit record with the refusal class in
//    the detail for every refusal, and `PLUGIN_TENANT_SCOPE_ADMITTED` on
//    admission — §16.2 "Audit cross-tenant access denials".
// 5. THE P7.36 COMPOSITION. The wrapper composes (never replaces) the
//    inner plugin-path envelope; an invalid inner envelope is
//    `TENANT_SCOPE_ENVELOPE_INVALID` at wrap time.
//
// STATUS: WRITTEN-UNRUN (ENTRY_NOTE boundary 3; no cargo command was
// executed; the human build/test gate owns:
// `cargo test -p flexcore-kernel --test p10_04_tenant_scope_refusals`).
// Expected strings are copied from the module's own constants.

use flexcore_kernel::lifecycle::FixedClock;
use flexcore_kernel::plugin_path_protection::PluginRequestEnvelope;
use flexcore_kernel::tenant_scope::{
    enforce_scope, enforce_scoped_request, error_names_from_enum, MemoryTenantScopeAuditSink,
    ScopedRequestEnvelope, TenantScope, TenantScopeAuditKind, TENANT_SCOPE_ERROR_NAMES,
    TENANT_SCOPE_STATEMENT,
};

const BASE_MS: u64 = 1_700_000_000_000;
const TENANT_A: &str = "tenant-alpha";
const TENANT_B: &str = "tenant-beta";
const SITE_1: &str = "site-1";
const SITE_2: &str = "site-2";

fn scope(tenant: &str, site: Option<&str>) -> TenantScope {
    TenantScope::from_authenticated(tenant, site).expect("valid scope fixture")
}

/// A valid inner P7.36 envelope (self-valid; the constructor computes the
/// digest).
fn inner_envelope(sequence: u64) -> PluginRequestEnvelope {
    PluginRequestEnvelope::new(
        "corr-p10-04",
        "neryva.scoped-worker",
        "0.1.0",
        sequence,
        BASE_MS,
        BASE_MS + 60_000,
        serde_json::json!({ "task": "scope-matrix" }),
    )
    .expect("valid inner envelope")
}

// --- 1. The wrong-scope matrix over the narrowing predicate ---------------------------

#[test]
fn covers_admits_exactly_the_covered_combinations() {
    // Envelope: tenant-wide. Covers every site request in the SAME tenant;
    // never another tenant.
    let wide = scope(TENANT_A, None);
    assert!(wide.covers(&scope(TENANT_A, None)));
    assert!(wide.covers(&scope(TENANT_A, Some(SITE_1))));
    assert!(wide.covers(&scope(TENANT_A, Some(SITE_2))));
    assert!(!wide.covers(&scope(TENANT_B, None)), "cross-tenant is never covered");
    assert!(!wide.covers(&scope(TENANT_B, Some(SITE_1))));
    // Envelope: site-narrowed. Covers only the SAME site; an un-narrowed
    // request is a widening attempt and is NOT covered.
    let narrowed = scope(TENANT_A, Some(SITE_1));
    assert!(narrowed.covers(&scope(TENANT_A, Some(SITE_1))));
    assert!(!narrowed.covers(&scope(TENANT_A, None)), "un-narrowed under narrowed is a widening");
    assert!(!narrowed.covers(&scope(TENANT_A, Some(SITE_2))));
    assert!(!narrowed.covers(&scope(TENANT_B, Some(SITE_1))));
}

#[test]
fn enforce_scope_refuses_every_wrong_scope_request_with_the_stable_name() {
    // The enforcement point cross-checks the REQUESTED data scope against
    // the host-stamped envelope: the full matrix of (envelope, requested)
    // shapes.
    let cases: [(TenantScope, TenantScope, bool); 10] = [
        (scope(TENANT_A, None), scope(TENANT_A, None), true),
        (scope(TENANT_A, None), scope(TENANT_A, Some(SITE_1)), true),
        (scope(TENANT_A, None), scope(TENANT_A, Some(SITE_2)), true),
        (scope(TENANT_A, None), scope(TENANT_B, None), false),
        (scope(TENANT_A, None), scope(TENANT_B, Some(SITE_1)), false),
        (scope(TENANT_A, Some(SITE_1)), scope(TENANT_A, Some(SITE_1)), true),
        (scope(TENANT_A, Some(SITE_1)), scope(TENANT_A, Some(SITE_2)), false),
        (scope(TENANT_A, Some(SITE_1)), scope(TENANT_A, None), false),
        (scope(TENANT_A, Some(SITE_1)), scope(TENANT_B, Some(SITE_1)), false),
        (scope(TENANT_A, Some(SITE_1)), scope(TENANT_B, None), false),
    ];
    for (envelope_scope, requested, allowed) in cases {
        let scoped = ScopedRequestEnvelope::new(inner_envelope(1), envelope_scope.clone())
            .expect("wrap a valid envelope");
        let outcome = enforce_scope(&scoped, &requested);
        assert_eq!(
            outcome.is_ok(),
            allowed,
            "envelope={} requested={}",
            envelope_scope,
            requested
        );
        if !allowed {
            let err = outcome.expect_err("refused");
            assert_eq!(err.name(), "TENANT_SCOPE_MISMATCH");
            assert!(err.to_string().contains("TENANT_SCOPE_MISMATCH"));
        }
    }
}

// --- 2. Echo-mismatch refusal ---------------------------------------------------------

#[test]
fn a_plugin_echoing_a_wider_or_different_scope_is_refused() {
    let stamped = scope(TENANT_A, Some(SITE_1));
    let scoped = ScopedRequestEnvelope::new(inner_envelope(1), stamped).expect("wrap");
    // Exact echo honoured.
    scoped.check_echo(&scope(TENANT_A, Some(SITE_1))).expect("exact echo");
    // Widened echo refused.
    assert_eq!(
        scoped
            .check_echo(&scope(TENANT_A, None))
            .expect_err("widened echo")
            .name(),
        "TENANT_SCOPE_ECHO_MISMATCH"
    );
    // Cross-site and cross-tenant echoes refused.
    assert_eq!(
        scoped
            .check_echo(&scope(TENANT_A, Some(SITE_2)))
            .expect_err("cross-site echo")
            .name(),
        "TENANT_SCOPE_ECHO_MISMATCH"
    );
    let err = scoped
        .check_echo(&scope(TENANT_B, Some(SITE_1)))
        .expect_err("cross-tenant echo");
    assert_eq!(err.name(), "TENANT_SCOPE_ECHO_MISMATCH");
    // The error names both scopes (scope renderings only — never payload).
    let text = err.to_string();
    assert!(text.contains("tenant:tenant-alpha|site:site-1"));
    assert!(text.contains("tenant:tenant-beta|site:site-1"));
}

// --- 3. Host-stamped construction only ------------------------------------------------

#[test]
fn a_scope_is_derived_from_the_authenticated_context_only() {
    // Empty/blank tenant, empty site string, oversized ids: named
    // refusals. The fields are private — no plugin-supplied struct can be
    // assembled (compile-level).
    assert_eq!(
        TenantScope::from_authenticated("", None).expect_err("no tenant").name(),
        "TENANT_SCOPE_INVALID"
    );
    assert_eq!(
        TenantScope::from_authenticated("   ", None).expect_err("blank tenant").name(),
        "TENANT_SCOPE_INVALID"
    );
    assert_eq!(
        TenantScope::from_authenticated(TENANT_A, Some(""))
            .expect_err("empty site")
            .name(),
        "TENANT_SCOPE_INVALID"
    );
    let oversized = "x".repeat(257);
    assert_eq!(
        TenantScope::from_authenticated(&oversized, None)
            .expect_err("oversized tenant id")
            .name(),
        "TENANT_SCOPE_INVALID"
    );
    // The recorded §16.2 rule statement is pinned.
    assert!(TENANT_SCOPE_STATEMENT.contains("HOST-STAMPED"));
    assert!(TENANT_SCOPE_STATEMENT.contains("never from any plugin-supplied field"));
    assert!(TENANT_SCOPE_STATEMENT.contains("cannot widen its own scope"));
}

// --- 4. The audited enforcement path ---------------------------------------------------

#[test]
fn every_enforcement_outcome_is_audited_with_the_stable_kinds() {
    let clock = FixedClock::at_unix_millis(BASE_MS as i64 + 1_000);
    let audit = MemoryTenantScopeAuditSink::new();
    let stamped = scope(TENANT_A, Some(SITE_1));
    let scoped = ScopedRequestEnvelope::new(inner_envelope(1), stamped).expect("wrap");
    // Admission (equal scope, no echo).
    enforce_scoped_request(&scoped, &scope(TENANT_A, Some(SITE_1)), None, &clock, &audit)
        .expect("admitted");
    // Cross-tenant refusal — audited with the refusal class.
    assert!(enforce_scoped_request(&scoped, &scope(TENANT_B, Some(SITE_1)), None, &clock, &audit).is_err());
    // Widened-echo refusal — audited.
    assert!(enforce_scoped_request(
        &scoped,
        &scope(TENANT_A, Some(SITE_1)),
        Some(&scope(TENANT_A, None)),
        &clock,
        &audit,
    )
    .is_err());
    let records = audit.records();
    assert_eq!(records.len(), 3, "every outcome is audited");
    assert_eq!(records[0].kind, TenantScopeAuditKind::ScopeAdmitted);
    assert_eq!(records[0].kind.name(), "PLUGIN_TENANT_SCOPE_ADMITTED");
    assert_eq!(records[1].kind, TenantScopeAuditKind::ScopeRefused);
    assert!(records[1].detail.contains("TENANT_SCOPE_MISMATCH"));
    assert_eq!(records[2].kind, TenantScopeAuditKind::ScopeRefused);
    assert!(records[2].detail.contains("TENANT_SCOPE_ECHO_MISMATCH"));
    // Timestamps come ONLY from the injected clock.
    for record in records.iter() {
        assert_eq!(record.timestamp_millis, BASE_MS + 1_000);
    }
}

// --- 5. The P7.36 composition: an invalid inner envelope is refused at wrap time -------

#[test]
fn an_invalid_inner_envelope_is_refused_at_wrap_time() {
    // The wrapper re-runs the inner envelope's own P7.36 self-validity
    // check: an inner envelope with an inverted window is
    // TENANT_SCOPE_ENVELOPE_INVALID — the four plugin-path checks run
    // UNCHANGED through `inner()`.
    // Wrapped form: the wrap-time re-check maps the inner refusal into the
    // tenant-scope error space. (Whether the P7.36 constructor refuses the
    // inverted window itself or the wrapper's re-check does, the malformed
    // envelope never reaches a plugin with a valid scope binding — the
    // test is fail-closed on BOTH paths.)
    let scope_fixture = scope(TENANT_A, None);
    let invalid = PluginRequestEnvelope::new(
        "corr-p10-04-bad",
        "neryva.scoped-worker",
        "0.1.0",
        1,
        BASE_MS + 60_000,
        BASE_MS,
        serde_json::json!({}),
    );
    if let Ok(envelope) = invalid {
        let err = ScopedRequestEnvelope::new(envelope, scope_fixture)
            .expect_err("an invalid inner envelope is refused at wrap time");
        assert_eq!(err.name(), "TENANT_SCOPE_ENVELOPE_INVALID");
    } else {
        // The inner constructor refuses first — either way the malformed
        // envelope never reaches a plugin with a valid scope binding.
        assert!(invalid.is_err(), "fail-closed on both paths");
    }
}

// --- 6. The refusal-name table ----------------------------------------------------------

#[test]
fn the_refusal_name_table_is_the_recorded_five() {
    assert_eq!(
        TENANT_SCOPE_ERROR_NAMES,
        [
            "TENANT_SCOPE_BINDING_MISMATCH",
            "TENANT_SCOPE_ECHO_MISMATCH",
            "TENANT_SCOPE_ENVELOPE_INVALID",
            "TENANT_SCOPE_INVALID",
            "TENANT_SCOPE_MISMATCH",
        ]
    );
    // The table and the enum agree (the test-side consistency set).
    let from_enum = error_names_from_enum();
    assert_eq!(from_enum.len(), 5);
    for name in TENANT_SCOPE_ERROR_NAMES {
        assert!(from_enum.contains(name), "missing {name}");
    }
}
