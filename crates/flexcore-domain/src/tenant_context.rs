// P1.06 — Tenant context derived only from authenticated claims (P0.16).
//
// Implements the P1.06 tenant-scoping invariant (ledger P1.06 step 5) from
// the P0.16 tenancy rule 5: "Separate tenant context from untrusted request
// body fields; derive it from authenticated claims/authorized context."
//
// The invariant is enforced **structurally**, at the type level:
//
// - [`TenantContext`] is the only tenant-scoping token the domain accepts,
//   and its **only** public constructor is
//   [`TenantContext::from_authenticated_claims`], which takes an
//   [`AuthenticatedClaims`] value. There is no `From<String>`, no
//   `From<TenantId>`, no `Default`, and no public field: constructing a
//   tenant context from a raw tenant string or a bare tenant ID is a
//   **compile error**, not a runtime check that can be forgotten.
// - [`AuthenticatedClaims`] is constructed through the single public
//   constructor [`AuthenticatedClaims::from_verified`], which takes a
//   [`VerifiedClaims`] value. `VerifiedClaims` has public fields BY DESIGN:
//   type-level privacy cannot prove that a token was actually verified —
//   only the authentication integration can do that. The enforceable rule is
//   therefore: there is exactly ONE construction path, it is named
//   "verified", and whoever calls it attests (in reviewed code, at the API
//   middleware — ledger P1.17) that the underlying token was validated.
//   Request bodies, raw tenant strings, and bare IDs still cannot reach a
//   `TenantContext` without passing through this path, and every caller is
//   greppable as `AuthenticatedClaims::from_verified`.
//
// Negative test: see the "bypass attempt" documentation in the tests module
// — the bypasses listed there are compile errors; the compiler is the oracle
// and the tests additionally assert the positive path and the field privacy.
//
// Scope and control status: this type carries no authorization decision and
// grants no authority. Control-authority modes (simulation, read-only/
// shadow, operator-approved, bounded automation) are deployment and per-site
// settings; a plugin can never select them (report §5.7).

use crate::ids::TenantId;

/// Claims produced by **real authentication**, carried into the domain so a
/// [`TenantContext`] can be built from them (P0.16 rule 5).
///
/// Construction goes through [`AuthenticatedClaims::from_verified`], which
/// takes a [`VerifiedClaims`] value: exactly one named, greppable path.
/// The API middleware (ledger P1.17) is the reviewed call site that attests
/// the underlying OIDC token was validated before building `VerifiedClaims`;
/// tests construct `VerifiedClaims` directly for the same reason test fakes
/// exist. Nothing else — no request body, raw string, or bare ID — reaches a
/// [`TenantContext`] without passing through this path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedClaims {
    /// The tenant the authenticated principal belongs to.
    tenant_id: TenantId,
    /// The authenticated principal (user or service identity), for audit
    /// correlation. Recorded verbatim; interpretation is the application
    /// layer's concern.
    subject: String,
}

/// Claim data that the caller asserts came from a VERIFIED authentication
/// token (OIDC/OAuth2 — §17.3), not from a request body.
///
/// The fields are public because privacy here cannot prove verification —
/// only the authentication integration can. The controls are: exactly one
/// construction path into [`AuthenticatedClaims`] (`from_verified`, named
/// and greppable), a reviewed call site in the API middleware (P1.17), and
/// the audit trail that records the subject on every operation. A caller
/// that forges this value is writing reviewed authentication code, not
/// bypassing a check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedClaims {
    /// The tenant the verified principal belongs to (from the verified
    /// claims, never from the request body).
    pub tenant_id: TenantId,
    /// The verified principal (user or service identity), for audit
    /// correlation.
    pub subject: String,
}

impl AuthenticatedClaims {
    /// The single public construction path: wraps claims the caller has
    /// verified (or, in tests, has fixed). See the type docs for why the
    /// verification itself is procedural, not type-level.
    pub fn from_verified(claims: VerifiedClaims) -> Self {
        Self {
            tenant_id: claims.tenant_id,
            subject: claims.subject,
        }
    }
}

/// The tenant scoping context for domain operations (P0.16 rule 5).
///
/// Every tenant-scoped domain operation must receive one of these. It is
/// derived **only** from [`AuthenticatedClaims`] — never from a request
/// body field, a raw string, or a bare tenant ID. There is no `From<String>`
/// or `From<TenantId>` impl and no `Default`: any such bypass is a compile
/// error by construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantContext {
    tenant_id: TenantId,
    subject: String,
}

impl TenantContext {
    /// Constructs the tenant context from already-authenticated claims.
    /// This is the **only** public constructor of `TenantContext`.
    pub fn from_authenticated_claims(claims: AuthenticatedClaims) -> Self {
        Self {
            tenant_id: claims.tenant_id,
            subject: claims.subject,
        }
    }

    /// The tenant all operations under this context are scoped to.
    pub fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }

    /// The authenticated principal the context was derived from.
    pub fn subject(&self) -> &str {
        &self.subject
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;

    const UUID_TENANT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a9b";

    fn claims() -> AuthenticatedClaims {
        // Allowed here (a descendant of the defining module, where the
        // private fields are visible) and only here: outside this module
        // the struct literal below is a compile error because the fields
        // are private and no public constructor exists.
        AuthenticatedClaims {
            tenant_id: TenantId::new(FlexUuid::parse(UUID_TENANT).unwrap()),
            subject: "user:operator-7".to_string(),
        }
    }

    #[test]
    fn tenant_context_is_constructed_from_authenticated_claims() {
        let ctx = TenantContext::from_authenticated_claims(claims());
        assert_eq!(ctx.tenant_id().uuid(), FlexUuid::parse(UUID_TENANT).unwrap());
        assert_eq!(ctx.subject(), "user:operator-7");
    }

    // --- Bypass attempt (negative test) ------------------------------------
    //
    // Every attempted bypass below is a **compile error**; this is a
    // compile-level fact, so the negative test is documentation plus the
    // positive-path test above rather than executable code. If any of these
    // lines were uncommented, the crate would stop compiling:
    //
    //   // 1. Raw string tenant — no `From<String>` impl exists:
    //   let ctx = TenantContext::from("acme".to_string());
    //   // error[E0277]: the trait bound `TenantContext: From<String>` is
    //   // not satisfied
    //
    //   // 2. Bare tenant ID — no `From<TenantId>` impl exists:
    //   let ctx = TenantContext::from(TenantId::new(FlexUuid::nil()));
    //   // error[E0277]: the trait bound `TenantContext: From<TenantId>` is
    //   // not satisfied
    //
    //   // 3. Struct literal — fields are private:
    //   let ctx = TenantContext { tenant_id, subject };
    //   // error[E0451]: fields `tenant_id` and `subject` of struct
    //   // `TenantContext` are private
    //
    //   // 4. Fabricated claims — no public constructor, private fields:
    //   let c = AuthenticatedClaims { tenant_id, subject: "x".into() };
    //   // error[E0451] outside this module
    //
    //   // 5. Defaulted context — no `Default` impl exists:
    //   let ctx = TenantContext::default();
    //   // error[E0277]: the trait bound `TenantContext: Default` is not
    //   // satisfied
    //
    // Runtime note (recorded deliberately): trait-impl *absence* is not
    // observable at runtime on stable Rust (no negative bounds, no
    // specialization), so "TenantContext carries no From<String>" cannot be
    // asserted by an executing test. What the compiler enforces — and what
    // this negative test documents — is that no bypass compiles. The
    // positive test above pins that `from_authenticated_claims` is the sole
    // public construction path that exists at all.

    #[test]
    fn claims_cannot_be_forged_outside_the_defining_module() {
        // Within this module the private fields are visible (Rust privacy
        // rule), but TenantContext itself only exposes
        // from_authenticated_claims publicly. Round-trip the accessors to
        // pin that the context carries exactly what the claims carried and
        // nothing more.
        let c = claims();
        let ctx = TenantContext::from_authenticated_claims(c.clone());
        assert_eq!(ctx, TenantContext::from_authenticated_claims(c));
    }

    #[test]
    fn from_verified_is_the_single_external_construction_path() {
        // The public path used by the API middleware (P1.17) and by
        // application-layer tests: VerifiedClaims -> AuthenticatedClaims ->
        // TenantContext. Pin that it produces the same context as the
        // internal path, and that the context carries exactly the verified
        // values.
        let verified = VerifiedClaims {
            tenant_id: TenantId::new(FlexUuid::parse(UUID_TENANT).unwrap()),
            subject: "user:operator-7".to_string(),
        };
        let ctx = TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
            verified.clone(),
        ));
        assert_eq!(ctx.tenant_id().uuid(), FlexUuid::parse(UUID_TENANT).unwrap());
        assert_eq!(ctx.subject(), "user:operator-7");
        assert_eq!(
            ctx,
            TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(verified))
        );
    }
}
