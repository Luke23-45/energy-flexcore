// P1.07 — The `TenantContextSource` port: the documented integration point
//         for the P1.06 claims boundary (P0.16 rule 5).
//
// RECORDED GAP (task P1.07/P1.08 report; see the crate README): the domain
// `AuthenticatedClaims` type (flexcore-domain, P1.06) has private fields
// and **no public constructor anywhere** — by deliberate design, so no
// request field, plugin, or adapter can fabricate a `TenantContext`. The
// intended producer is the API middleware, which must construct the claims
// from a *verified* token; that constructor is a deliberate, reviewed
// decision scheduled as a later subtask (tenant-context integration,
// ledger P1.11 scope; the flexcore-authz crate records the same boundary
// in its test module).
//
// Consequences today, stated plainly:
//
// - **No implementation of this port can exist yet**: an implementation
//   would have to build a `TenantContext`, and the only constructor is
//   `TenantContext::from_authenticated_claims(AuthenticatedClaims)`, whose
//   argument is unconstructible outside flexcore-domain's own module.
// - The application use cases therefore receive `TenantContext` values as
//   parameters from the composition root; they never call this port, and
//   no Stage 1 code path depends on it existing.
// - The port is the seam the future middleware implements: once the domain
//   exposes the claims constructor for verified tokens, the middleware
//   implements `tenant_context()` over the authenticated request and every
//   downstream layer keeps compiling unchanged.
//
// This is honest scaffolding, not a stub: the trait is complete, compiles,
// and documents the exact obligation; there is simply no implementable
// body yet, and pretending otherwise would be the real hazard.

use flexcore_domain::tenant_context::TenantContext;

/// Errors returned by a [`TenantContextSource`] implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TenantContextSourceError {
    /// The current request/call carries no verified authentication — a
    /// tenant context must never be fabricated from untrusted input
    /// (P0.16 rule 5).
    NotAuthenticated,
}

impl std::fmt::Display for TenantContextSourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TenantContextSourceError::NotAuthenticated => {
                write!(
                    f,
                    "no verified authentication is present; a tenant context is never \
                     fabricated from untrusted input"
                )
            }
        }
    }
}

impl std::error::Error for TenantContextSourceError {}

/// The tenant-context source: the port the API middleware implements to
/// hand the composition root a claims-derived `TenantContext` (P0.16 rule
/// 5). See the module header for the recorded gap and the activation path.
pub trait TenantContextSource {
    /// Returns the tenant context for the current request/call, derived
    /// from verified authentication only.
    fn tenant_context(&self) -> Result<TenantContext, TenantContextSourceError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal inline implementation proving the trait shape is
    /// implementable as a signature. It can only ever return
    /// `NotAuthenticated` — which is precisely the recorded gap: no code
    /// outside flexcore-domain can produce a `TenantContext` today.
    struct NoAuthenticationSource;

    impl TenantContextSource for NoAuthenticationSource {
        fn tenant_context(&self) -> Result<TenantContext, TenantContextSourceError> {
            Err(TenantContextSourceError::NotAuthenticated)
        }
    }

    #[test]
    fn until_the_domain_exposes_the_claims_constructor_only_not_authenticated_exists() {
        let source: &dyn TenantContextSource = &NoAuthenticationSource;
        assert_eq!(
            source.tenant_context(),
            Err(TenantContextSourceError::NotAuthenticated)
        );
    }
}
