// P1.23 — Security test suite (tenant-context slice): the tenant-context
//         boundary, derived from the §17.4 threat table (ledger P1.23;
//         report §19.2 test layer 7, §17.4 row 4 "Cross-tenant leakage",
//         §16.2; P0.16 rule 5).
//
// What is under test here is the STRUCTURAL rule: `TenantContext` is
// derived only from authenticated claims — never from a request body
// field, a raw tenant string, or a bare tenant ID — and
// `AuthenticatedClaims::from_verified(VerifiedClaims)` is the single
// external construction path.
//
// Compile-fact comment (the negative test; the compiler is the oracle):
// every attempted bypass below is a **compile error**, not a runtime
// check that can be forgotten. If any line were uncommented, this test
// file — and therefore the crate — would stop compiling:
//
//   use flexcore_contracts::ids::FlexUuid;
//   use flexcore_domain::ids::TenantId;
//
//   // 1. Raw string tenant — no `From<String>` impl exists:
//   let ctx = TenantContext::from("acme".to_string());
//   // error[E0277]: the trait bound `TenantContext: From<String>` is
//   // not satisfied
//
//   // 2. Bare tenant ID — no `From<TenantId>` impl exists:
//   let ctx = TenantContext::from(TenantId::new(FlexUuid::NIL));
//   // error[E0277]: the trait bound `TenantContext: From<TenantId>` is
//   // not satisfied
//
//   // 3. Struct literal — `tenant_id`/`subject` are private to
//   // flexcore-domain:
//   let ctx = TenantContext { tenant_id: TenantId::new(FlexUuid::NIL), subject: "x".to_string() };
//   // error[E0451]: fields `tenant_id` and `subject` of struct
//   // `TenantContext` are private
//
//   // 4. Fabricated authenticated claims — no public constructor besides
//   // `from_verified`, private fields:
//   let c = AuthenticatedClaims { tenant_id: TenantId::new(FlexUuid::NIL), subject: "x".to_string() };
//   // error[E0451]: fields ... are private
//
//   // 5. Defaulted context — no `Default` impl exists:
//   let ctx = TenantContext::default();
//   // error[E0277]: the trait bound `TenantContext: Default` is not
//   // satisfied
//
//   // 6. Skipping the claims step — no `From<VerifiedClaims> for
//   // TenantContext` shortcut exists; the path is exactly two steps:
//   let ctx = TenantContext::from(verified);
//   // error[E0277]: the trait bound `TenantContext: From<VerifiedClaims>`
//   // is not satisfied
//
// Runtime note (recorded deliberately, mirroring the in-crate tests):
// trait-impl *absence* is not observable at runtime on stable Rust, so
// "no From<String>" cannot be asserted by an executing test. What the
// tests below pin executably is the positive path and the round-trip of
// the verified values.

use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::ids::TenantId;
use flexcore_domain::tenant_context::{
    AuthenticatedClaims, TenantContext, VerifiedClaims,
};

const UUID_TENANT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a0b";

fn tenant_id() -> TenantId {
    TenantId::new(FlexUuid::parse(UUID_TENANT).expect("valid test uuid"))
}

fn verified() -> VerifiedClaims {
    VerifiedClaims {
        tenant_id: tenant_id(),
        subject: "user:operator-7".to_string(),
    }
}

/// Positive path: from_verified -> from_authenticated_claims is the only
/// public path, and it is reachable from OUTSIDE the domain crate (this
/// integration test is outside it) — which is precisely the API-middleware
/// path (P1.17).
#[test]
fn from_verified_is_the_only_public_path_and_reachable_from_outside_the_domain() {
    let claims = AuthenticatedClaims::from_verified(verified());
    let ctx = TenantContext::from_authenticated_claims(claims);
    assert_eq!(ctx.tenant_id(), tenant_id());
    assert_eq!(ctx.subject(), "user:operator-7");
}

/// The context carries exactly what the verified claims carried — tenant
/// and subject round-trip verbatim, nothing more, nothing less.
#[test]
fn context_carries_the_verified_tenant_and_subject_verbatim() {
    for (tenant_uuid, subject) in [
        ("0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a01", "user:operator-7"),
        ("0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a02", "service:edge-agent-3"),
        ("0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a03", "user:admin@acme.example"),
    ] {
        let claims = VerifiedClaims {
            tenant_id: TenantId::new(FlexUuid::parse(tenant_uuid).expect("valid uuid")),
            subject: subject.to_string(),
        };
        let ctx =
            TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(claims));
        assert_eq!(ctx.tenant_id().uuid(), FlexUuid::parse(tenant_uuid).unwrap());
        assert_eq!(ctx.subject(), subject);
    }
}

/// Two different verified tenants (or subjects) produce two different,
/// non-interchangeable contexts — the value a downstream evaluator matches
/// scopes against (P1.11) is the exact tenant the verification produced.
#[test]
fn different_verified_claims_produce_different_contexts() {
    let other = VerifiedClaims {
        tenant_id: TenantId::new(
            FlexUuid::parse("0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a0c").expect("valid uuid"),
        ),
        subject: "user:operator-7".to_string(),
    };
    let ctx_a =
        TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(verified()));
    let ctx_other =
        TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(other));
    assert_ne!(ctx_a, ctx_other);
    assert_ne!(ctx_a.tenant_id(), ctx_other.tenant_id());

    // Same tenant, different subject: still distinct (audit correlation
    // carries the principal, P0.16 rule 5).
    let different_subject = VerifiedClaims {
        tenant_id: tenant_id(),
        subject: "user:someone-else".to_string(),
    };
    let ctx_subject =
        TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
            different_subject,
        ));
    assert_ne!(ctx_a, ctx_subject);
    assert_eq!(ctx_a.tenant_id(), ctx_subject.tenant_id());
}

/// Determinism: the same verified claims always produce the same context
/// (equality holds across repeated constructions).
#[test]
fn the_same_verified_claims_produce_the_same_context() {
    let ctx_1 =
        TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(verified()));
    let ctx_2 =
        TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(verified()));
    assert_eq!(ctx_1, ctx_2);
}

// --- VerifiedClaims misuse documentation -------------------------------------
//
// `VerifiedClaims` has PUBLIC fields BY DESIGN (P1.06): type-level privacy
// cannot prove that a token was actually verified — only the authentication
// integration can. The enforceable controls are:
//
// 1. exactly ONE named, greppable construction path into
//    `AuthenticatedClaims` (`from_verified`);
// 2. a reviewed call site in the API middleware (ledger P1.17) that attests
//    the underlying OIDC token was validated before building
//    `VerifiedClaims`;
// 3. the audit trail recording the subject on every operation.
//
// A caller that forges `VerifiedClaims` in production code is therefore
// writing reviewed authentication code, not bypassing a check — every call
// site is greppable as `AuthenticatedClaims::from_verified`. What forgery
// can NOT do is reach a `TenantContext` from a request body, a raw tenant
// string, or a bare tenant ID: those paths are compile errors (see the
// compile-fact comment in this file's header). This is a recorded,
// deliberate boundary — not an oversight to "fix" by making the fields
// private.
//
// The executable pins for this boundary are the tests above: the only
// public path works, carries the values verbatim, and distinguishes
// tenants and subjects exactly.
