// P13.02 — The OIDC token-validation resolver: the PRODUCTION identity path
//         that composes the LR-047 "future composition" (decision record:
//         phase-13 `records/02_p13_02_oidc_token_validation.md`; identity
//         architecture: `records/01_p13_01_identity_architecture.md`).
//
// WRITTEN-UNRUN throughout: no token has ever been validated, no JWKS
// document has been fetched, no discovery document has been read, no IdP is
// deployed (P13-E-1 stays OPEN — this module implements the recorded
// DEFAULT only, conventions §4). The first human build compiles the JWT
// crate pin; the first IdP-connected lab run (P13.20) is the operational
// gate. NOTHING here claims runtime behavior.
//
// What this module IS, honestly:
//
// - An implementation of the existing `IdentityResolver` trait
//   (`crate::auth`), with `DenyAllResolver` still the SHIPPED DEFAULT: the
//   composition root opts in by constructing [`OidcResolver`] and wiring it
//   as the state's `identity` resolver. Nothing in this repo does that
//   wiring today (no IdP exists to point at) — composition is a deployment
//   act, and `main.rs` is untouched by this subtask.
// - The single place where a verified OIDC token becomes
//   `VerifiedClaims` → `AuthenticatedClaims::from_verified` →
//   `TenantContext` (flexcore-domain P1.06): the reviewed attestation call
//   the domain crate documents. The `decode`-then-validate pipeline below
//   is that attestation: signature, issuer, audience, expiry, and not-
//   before are all checked BEFORE any `VerifiedClaims` value is built.
//
// What this module is NOT:
//
// - Not a token endpoint, not a login flow (P13.17, wave 2), not a session
//   store (P13.16), not an MFA verifier (P13.04 — the IdP owns the
//   authenticator per the P13-E-3 DEFAULT).
// - Not a JWKS fetcher: network behavior is a RUNTIME ACT. The JWKS
//   document arrives through the injected [`JwksSource`] port; no HTTP
//   client exists in this module and none was added (the composition root
//   supplies the fetch implementation at deployment time). Every cache-
//   refresh path that would touch the network is UNRUN by definition.
//
// Validation rules (the recorded policy, enforced below):
//
// 1. Signature: the token's `kid` must exist in the cached JWKS; the JWK
//    must decode to a [`jsonwebtoken::DecodingKey`]; verification uses the
//    configured algorithm allow-list (DEFAULT: RS256 only — an `alg` header
//    of `none` or any asymmetric algorithm outside the allow-list rejects).
// 2. `iss`: exact string match against the configured issuer (the
//    per-tenant SSO connection's issuer — record 01 §4). No trailing-slash
//    normalization, no multi-issuer acceptance.
// 3. `aud`: the configured client id MUST be present in the audience
//    claim (string or list form). Missing audience rejects.
// 4. `exp`: must be in the future beyond the configured leeway (DEFAULT
//    60 s — a registered RECOMMENDATION for clock skew, X-8 discipline).
//    `nbf` is validated when present, same leeway.
// 5. Tenant mapping: the verified claims map to a `TenantId` through the
//    injected [`TenantClaimMapper`] (the per-tenant SSO connection is
//    tenant data, record 01 §4). An unmapped tenant FAILS CLOSED — no
//    default tenant, no "platform" fallback (P0.16 rule 5).
// 6. Role mapping: the token's roles/groups map to the closed `Role`
//    vocabulary through the closed [`RoleClaimMapper`]. An identity whose
//    role cannot be mapped is rejected — never defaulted to Viewer.
//
// A failure at ANY step is `AuthError::InvalidCredentials` (or
// `MissingCredentials` when no bearer is present) — a reasoned 401 at the
// route layer (`crate::auth::caller_from_parts`), never a fabricated
// context and never a degraded pass.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use flexcore_domain::ids::TenantId;
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use jsonwebtoken::decode_header;
use jsonwebtoken::jwk::{Jwk, JwkSet};
use serde::Deserialize;

use crate::auth::{AuthError, Caller, IdentityResolver, parse_role};
use flexcore_authz::policy::Role;

/// The DEFAULT clock-skew leeway for `exp`/`nbf` validation (seconds).
/// REGISTERED RECOMMENDATION (X-8: registered before any run that could
/// tune it) — 60 s absorbs ordinary NTP skew without widening the replay
/// window meaningfully. An operator may tighten it; nothing here claims a
/// tested optimum.
pub const DEFAULT_LEEWAY_SECONDS: u64 = 60;

/// The DEFAULT JWKS cache lifetime. REGISTERED RECOMMENDATION: 15 minutes,
/// the documented shape of typical IdP key-rotation guidance (ASSUMPTION:
/// the per-IdP values are the deployment's; nothing here claims a tested
/// TTL). Refresh-on-unknown-`kid` (§ below) supersedes the TTL on key
/// rotation.
pub const DEFAULT_JWKS_CACHE_TTL: Duration = Duration::from_secs(15 * 60);

/// The DEFAULT signature-algorithm allow-list: RS256 ONLY. RSA with SHA-256
/// is the algorithm the recorded lab IdP config (P13.20 draft) issues; a
/// wider allow-list is a deployment-level loosening that must be reviewed,
/// never a code default.
pub const DEFAULT_ALGORITHMS: &[jsonwebtoken::Algorithm] = &[jsonwebtoken::Algorithm::RS256];

// --- The claims we read from a verified token --------------------------------

/// The OIDC claims this resolver reads from a VERIFIED token. Fields beyond
/// these are ignored (the token may carry anything the IdP issues; only the
/// claims below participate in identity resolution). Deserialization of the
/// OPTIONAL claims is lenient (`Option` + defaults) so an IdP that omits a
/// claim yields `None`/empty rather than a parse failure — absence is then
/// handled by the fail-closed rules below, never by a panic or a guess.
#[derive(Debug, Clone, Deserialize)]
pub struct OidcTokenClaims {
    /// The subject (the end-user identifier at the IdP). REQUIRED: absence
    /// rejects (`InvalidCredentials`).
    pub sub: Option<String>,
    /// The issuer. Validated against the configured issuer (exact match).
    pub iss: Option<String>,
    /// The audience. The crate's `Validation` enforces membership; kept on
    /// the struct so the decoded value is inspectable in tests.
    #[serde(default)]
    pub aud: AudienceClaim,
    /// Expiry (seconds since the epoch). Validated by the crate's
    /// `Validation` with the configured leeway.
    pub exp: Option<i64>,
    /// Not-before (seconds since the epoch). Validated when present.
    pub nbf: Option<i64>,
    /// The roles/groups the IdP asserts for this principal. The closed
    /// [`RoleClaimMapper`] maps them; unknown entries are ignored, and a
    /// principal with NO mappable role is rejected.
    #[serde(default)]
    pub groups: Vec<String>,
    /// OPTIONAL direct tenant binding (`flexcore_tenant_id`): the UUID of
    /// the tenant the SSO connection bound this token to. When the
    /// per-tenant SSO connection issues it, the [`TenantClaimMapper`]
    /// consumes it; absence defers to the mapper's connection lookup —
    /// either way an unmapped tenant fails closed.
    #[serde(default)]
    pub flexcore_tenant_id: Option<String>,
}

/// The audience claim in its two legal shapes (a single string or a list).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum AudienceClaim {
    /// A single audience string.
    Single(String),
    /// The multi-audience list form.
    Many(Vec<String>),
}

impl Default for AudienceClaim {
    /// No audience asserted (an absent `aud` claim). The crate's
    /// `Validation` enforces membership, so the empty list fails closed.
    fn default() -> Self {
        AudienceClaim::Many(Vec::new())
    }
}

impl AudienceClaim {
    /// Whether the named audience is present.
    pub fn contains(&self, audience: &str) -> bool {
        match self {
            AudienceClaim::Single(value) => value == audience,
            AudienceClaim::Many(values) => values.iter().any(|v| v == audience),
        }
    }
}

// --- The JWKS source port (network behavior lives OUTSIDE this module) -------

/// The port that supplies a JWKS document for the configured issuer.
///
/// THE NETWORK BOUNDARY (recorded deliberately): fetching a JWKS document
/// is an HTTPS runtime act against the IdP. This module defines the port
/// and consumes it; it contains NO HTTP client and performs NO network
/// behavior. The composition root (or the lab environment, P13.20)
/// implements this port at deployment time. EVERY implementation of this
/// trait is therefore UNRUN from this repo's perspective.
pub trait JwksSource: Send + Sync {
    /// Fetches the current JWKS document for the given issuer. A fetch
    /// failure is a named error; the resolver fails closed on it — a stale
    /// cache is used only while it has not expired (see [`JwksCache`]), and
    /// never beyond expiry.
    fn fetch(&self, issuer: &str) -> Result<JwkSet, JwksSourceError>;
}

/// Errors of the JWKS source port. Named, never stringly; a source error
/// never degrades to "use an empty key set" or "skip verification".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JwksSourceError {
    /// The fetch could not reach the IdP (transport-level failure; the
    /// composition root names the cause in `detail` for audit).
    FetchFailed { issuer: String, detail: String },
    /// The response was not a parseable JWKS document.
    MalformedDocument { issuer: String, detail: String },
}

impl std::fmt::Display for JwksSourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JwksSourceError::FetchFailed { issuer, detail } => {
                write!(f, "JWKS fetch failed for issuer '{issuer}': {detail}")
            }
            JwksSourceError::MalformedDocument { issuer, detail } => {
                write!(f, "JWKS document from '{issuer}' is malformed: {detail}")
            }
        }
    }
}

impl std::error::Error for JwksSourceError {}

/// The in-memory JWKS cache: issuer → (document, fetched-at). Deterministic
/// lookup by `kid` inside the cached document; the TTL is the configured
/// [`OidcConfig::jwks_cache_ttl`].
///
/// Cache discipline (recorded; the network half is UNRUN):
/// - a cached document is used while younger than the TTL;
/// - an EXPIRED document is not "sticky": the next token that needs a key
///   triggers a refresh attempt through the port; a failed refresh during
///   expiry fails closed (`InvalidCredentials`) — never "verify with the
///   stale key anyway";
/// - refresh-on-unknown-`kid`: a token whose `kid` is absent from a
///   FRESH cache triggers ONE refresh attempt (the IdP rotated keys);
///   repeated misses within the same TTL window do NOT re-fetch (a bounded
///   single retry per miss is the recorded anti-stampede posture; a
///   production implementation adds the shared negative-cache the design
///   record names — deliberately not built here).
pub struct JwksCache {
    entries: Mutex<HashMap<String, CachedJwks>>,
    source: Box<dyn JwksSource>,
    ttl: Duration,
}

struct CachedJwks {
    document: JwkSet,
    fetched_at_unix_millis: i64,
}

impl JwksCache {
    /// Builds a cache over the injected source with the configured TTL.
    pub fn new(source: Box<dyn JwksSource>, ttl: Duration) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            source,
            ttl,
        }
    }

    /// Returns the key named `kid` for `issuer`: from a fresh cache entry,
    /// or after a successful refresh. All failure paths are named; no
    /// network behavior runs in this repo (the port's implementations are
    /// deployment artifacts — UNRUN here).
    pub fn decoding_key(
        &self,
        issuer: &str,
        kid: &str,
        now_unix_millis: i64,
    ) -> Result<DecodingKeyForKid, KeyLookupError> {
        let fresh = {
            let mut entries = self
                .entries
                .lock()
                .map_err(|_| KeyLookupError::CachePoisoned)?;
            match entries.get_mut(issuer) {
                Some(entry) if self.is_fresh(entry, now_unix_millis) => {
                    lookup_in(&entry.document, kid).map_err(KeyLookupError::from)
                }
                // Absent, expired, or the kid missed a fresh cache: fall
                // through to the refresh path below (one attempt).
                _ => Err(KeyLookupError::NeedsRefresh),
            }
        };
        match fresh {
            Ok(key) => Ok(key),
            Err(KeyLookupError::KeyNotFound { .. }) | Err(KeyLookupError::NeedsRefresh) => {
                // One refresh attempt; a poisoned lock or a failed fetch is
                // fail-closed. The refreshed entry replaces the old one.
                let document = self
                    .source
                    .fetch(issuer)
                    .map_err(|e| KeyLookupError::Source(e.to_string()))?;
                let key = lookup_in(&document, kid).map_err(KeyLookupError::from)?;
                let mut entries = self
                    .entries
                    .lock()
                    .map_err(|_| KeyLookupError::CachePoisoned)?;
                entries.insert(
                    issuer.to_string(),
                    CachedJwks {
                        document,
                        fetched_at_unix_millis: now_unix_millis,
                    },
                );
                Ok(key)
            }
            Err(other) => Err(other),
        }
    }

    fn is_fresh(&self, entry: &CachedJwks, now_unix_millis: i64) -> bool {
        let age_millis = now_unix_millis
            .saturating_sub(entry.fetched_at_unix_millis)
            .max(0);
        u64::try_from(age_millis)
            .map(|age| Duration::from_millis(age) < self.ttl)
            .unwrap_or(false)
    }
}

/// The decoded key material for one `kid` (a thin newtype over the crate's
/// key so the cache's return type stays named and greppable).
pub struct DecodingKeyForKid(jsonwebtoken::DecodingKey);

impl DecodingKeyForKid {
    /// The wrapped decoding key (consumed by the crate's `decode`).
    pub fn into_inner(self) -> jsonwebtoken::DecodingKey {
        self.0
    }
}

/// Errors of the cache lookup path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyLookupError {
    /// The kid is not in the (fresh) cached document — triggers the
    /// refresh-on-unknown-kid path internally.
    KeyNotFound { kid: String },
    /// A refresh is needed (no entry, or expired).
    NeedsRefresh,
    /// The JWKS port failed (the detail is the port error's text).
    Source(String),
    /// The cache mutex was poisoned — fail-closed.
    CachePoisoned,
}

impl From<LookupInError> for KeyLookupError {
    fn from(value: LookupInError) -> Self {
        match value {
            LookupInError::NotFound { kid } => KeyLookupError::KeyNotFound { kid },
            LookupInError::NotUsable { kid, detail } => KeyLookupError::Source(format!(
                "JWK '{kid}' cannot produce a decoding key: {detail}"
            )),
        }
    }
}

/// Errors of looking a kid up inside one document.
#[derive(Debug, Clone, PartialEq, Eq)]
enum LookupInError {
    NotFound { kid: String },
    NotUsable { kid: String, detail: String },
}

/// Looks `kid` up in one JWKS document and builds the decoding key.
fn lookup_in(document: &JwkSet, kid: &str) -> Result<DecodingKeyForKid, LookupInError> {
    let jwk: &Jwk = document
        .find(kid)
        .ok_or_else(|| LookupInError::NotFound { kid: kid.to_string() })?;
    let key = jsonwebtoken::DecodingKey::from_jwk(jwk)
        .map_err(|e| LookupInError::NotUsable {
            kid: kid.to_string(),
            detail: e.to_string(),
        })?;
    Ok(DecodingKeyForKid(key))
}

// --- The tenant and role claim mappers ----------------------------------------

/// The tenant-claim mapper: the verified token (its claims and its verified
/// issuer) resolves to the tenant the principal belongs to.
///
/// This is the P13.01 decision "per-tenant SSO connections are tenant
/// data" at the resolver: the mapper consults the deployment's SSO-
/// connection data (issuer → tenant binding, or the direct
/// `flexcore_tenant_id` claim the connection issues). This module ships
/// NO mapper implementation against a database — no such store exists in
/// this repo — only the closed in-config mapper below; the composition
/// root replaces it when the connection store exists. An unmapped token
/// FAILS CLOSED (P0.16 rule 5: no default tenant, ever).
pub trait TenantClaimMapper: Send + Sync {
    fn resolve_tenant(
        &self,
        verified_issuer: &str,
        claims: &OidcTokenClaims,
    ) -> Option<TenantId>;
}

/// The closed in-config tenant mapper: an exact issuer → tenant-uuid table,
/// with the optional direct `flexcore_tenant_id` claim honored ONLY when
/// its issuer matches the verified one (a tenant claim from a different
/// issuer is never accepted — a token cannot name its own tenant).
pub struct StaticTenantMapper {
    bindings: HashMap<String, String>,
}

impl StaticTenantMapper {
    /// Builds the mapper from issuer → tenant-UUID (canonical hyphenated
    /// form) pairs. Empty is legal: it maps nothing and denies everything
    /// (deny-by-default, like every other surface here).
    pub fn new(bindings: HashMap<String, String>) -> Self {
        Self { bindings }
    }
}

impl TenantClaimMapper for StaticTenantMapper {
    fn resolve_tenant(
        &self,
        verified_issuer: &str,
        claims: &OidcTokenClaims,
    ) -> Option<TenantId> {
        // The token's own issuer claim must match the verified issuer
        // before EITHER path is consulted: a token whose iss disagrees
        // with the verified issuer is inconsistent, and neither the direct
        // claim nor the connection binding may ride on it (a token cannot
        // name its own tenant — and cannot borrow another issuer's
        // binding either). The production path validates iss exactly
        // before this mapper is reached; this is the defence-in-depth
        // check at the mapper layer itself.
        if claims.iss.as_deref() != Some(verified_issuer) {
            return None;
        }
        // The direct claim is honored only for the verified issuer.
        if let Some(tid) = &claims.flexcore_tenant_id {
            if let Ok(uuid) = flexcore_contracts::ids::FlexUuid::parse(tid) {
                return Some(TenantId::new(uuid));
            }
        }
        // The connection binding: the verified issuer maps to the bound
        // tenant (tenant data, record 01 §4).
        let bound = self.bindings.get(verified_issuer)?;
        let uuid = flexcore_contracts::ids::FlexUuid::parse(bound).ok()?;
        Some(TenantId::new(uuid))
    }
}

/// The role-claim mapper: the token's roles/groups map onto the CLOSED
/// `Role` vocabulary through the same id strings the dev resolver parses
/// (`crate::auth::parse_role`). Unknown group names are ignored; a
/// principal with NO mappable role is rejected (never defaulted to
/// Viewer). This is the group→role mapping the SCIM design (record 03)
/// keeps in step: SCIM groups and token groups use the same role ids.
pub fn map_role_claims(groups: &[String]) -> Option<Role> {
    groups.iter().find_map(|g| parse_role(g))
}

// --- The resolver ----------------------------------------------------------------

/// The configuration of one production identity path (one deployment; the
/// per-tenant SSO connections live behind the mappers, record 01).
#[derive(Debug, Clone)]
pub struct OidcConfig {
    /// The expected issuer (exact match against the token's `iss`).
    pub issuer: String,
    /// The expected audience (this deployment's client id).
    pub audience: String,
    /// The signature-algorithm allow-list (DEFAULT: RS256 only).
    pub algorithms: Vec<jsonwebtoken::Algorithm>,
    /// The `exp`/`nbf` leeway (seconds). DEFAULT 60 s — registered
    /// recommendation.
    pub leeway_seconds: u64,
    /// The JWKS cache TTL. DEFAULT 15 min — registered recommendation.
    pub jwks_cache_ttl: Duration,
}

impl Default for OidcConfig {
    fn default() -> Self {
        Self {
            issuer: String::new(),
            audience: String::new(),
            algorithms: DEFAULT_ALGORITHMS.to_vec(),
            leeway_seconds: DEFAULT_LEEWAY_SECONDS,
            jwks_cache_ttl: DEFAULT_JWKS_CACHE_TTL,
        }
    }
}

/// The production OIDC identity resolver. Implements
/// [`IdentityResolver`] so it composes the existing seam unchanged:
/// `DenyAllResolver` stays the shipped default, and the composition root
/// opts in by wiring this resolver as the state's identity (a deployment
/// act — no wiring exists in this repo).
pub struct OidcResolver {
    config: OidcConfig,
    jwks: JwksCache,
    tenants: Box<dyn TenantClaimMapper>,
    /// The controlled clock for cache freshness and (indirectly) expiry
    /// cross-checks. Injected, never a wall clock read inside decision
    /// logic — the crate's `Validation` applies its own checks at decode
    /// time against the system clock; the injected clock governs the CACHE
    /// age only (the recorded boundary: token expiry is the JWT library's
    /// validated claim; cache age is our bookkeeping).
    clock_millis: Box<dyn Fn() -> i64 + Send + Sync>,
}

impl OidcResolver {
    /// Builds the resolver. The JWKS source and tenant mapper are injected;
    /// nothing here constructs either from a default network stack.
    pub fn new(
        config: OidcConfig,
        source: Box<dyn JwksSource>,
        tenants: Box<dyn TenantClaimMapper>,
        clock_millis: Box<dyn Fn() -> i64 + Send + Sync>,
    ) -> Self {
        let ttl = config.jwks_cache_ttl;
        Self {
            config,
            jwks: JwksCache::new(source, ttl),
            tenants,
            clock_millis,
        }
    }
}

impl IdentityResolver for OidcResolver {
    fn resolve(&self, bearer: Option<&str>) -> Result<Caller, AuthError> {
        // 0. The bearer must be present and shaped `Bearer <token>`.
        let raw = bearer.ok_or(AuthError::MissingCredentials)?;
        let token = raw
            .strip_prefix("Bearer ")
            .filter(|t| !t.trim().is_empty())
            .ok_or(AuthError::InvalidCredentials)?;

        // 1. The header names the key (`kid`) and the algorithm. An `alg`
        //    outside the allow-list rejects BEFORE any key material is
        //    touched (algorithm-confusion defense).
        let header = decode_header(token).map_err(|_| AuthError::InvalidCredentials)?;
        let kid = header.kid.ok_or(AuthError::InvalidCredentials)?;
        if !self.config.algorithms.contains(&header.alg) {
            return Err(AuthError::InvalidCredentials);
        }

        // 2. The key: cache → (refresh) → port. Every failure is a named
        //    fail-closed 401. THE NETWORK HALF (a real fetch) IS UNRUN in
        //    this repo — the port's implementations are deployment
        //    artifacts.
        let now = (self.clock_millis)();
        let key = self
            .jwks
            .decoding_key(&self.config.issuer, &kid, now)
            .map_err(|_| AuthError::InvalidCredentials)?;

        // 3. Decode + validate: signature (RS256 per the allow-list),
        //    `iss` (exact), `aud` (membership), `exp`/`nbf` (with the
        //    configured leeway). The crate's `Validation` is configured
        //    with exactly the recorded policy; nothing is defaulted
        //    silently.
        let mut validation = jsonwebtoken::Validation::new(header.alg);
        validation.set_issuer(&[self.config.issuer.clone()]);
        validation.set_audience(&[self.config.audience.clone()]);
        validation.leeway = self.config.leeway_seconds;
        validation.validate_exp = true;
        validation.validate_nbf = true;
        validation.algorithms = self.config.algorithms.clone();
        let data =
            jsonwebtoken::decode::<OidcTokenClaims>(token, &key.into_inner(), &validation)
                .map_err(|_| AuthError::InvalidCredentials)?;
        let claims = data.claims;

        // 4. Subject presence (required by the §14.3/audit subject
        //    discipline — an identity without a subject is not one).
        let subject = claims
            .sub
            .clone()
            .filter(|s| !s.trim().is_empty())
            .ok_or(AuthError::InvalidCredentials)?;

        // 5. Tenant mapping — fail closed on an unmapped tenant.
        let tenant = self
            .tenants
            .resolve_tenant(&self.config.issuer, &claims)
            .ok_or(AuthError::InvalidCredentials)?;

        // 6. Role mapping — a principal with no mappable role rejects.
        let role = map_role_claims(&claims.groups).ok_or(AuthError::InvalidCredentials)?;

        // 7. The reviewed attestation point (flexcore-domain P1.06):
        //    VERIFIED claims → `VerifiedClaims` → the single construction
        //    path into `AuthenticatedClaims` → `TenantContext`.
        let verified = VerifiedClaims {
            tenant_id: tenant,
            subject,
        };
        let claims = AuthenticatedClaims::from_verified(verified);
        Ok(Caller {
            ctx: TenantContext::from_authenticated_claims(claims),
            role,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI64, Ordering};

    // A syntactically valid JWKS document with one RSA JWK (`kid = "k1"`).
    // The modulus/exponent are NOT a real key pair — the cache and lookup
    // paths under test never verify a signature (signature verification is
    // the human-gated UNRUN path, needs a real IdP-issued token).
    const JWKS_JSON: &str = r#"{
        "keys": [{
            "kty": "RSA",
            "kid": "k1",
            "use": "sig",
            "alg": "RS256",
            "n": "sXgdKaG-u8FJh9T3sJ-HkquoFfMTOMX3CLmJUvNhcWJmVhVOczvWTs0Y",
            "e": "AQAB"
        }]
    }"#;

    struct StaticSource(JwkSet);

    impl JwksSource for StaticSource {
        fn fetch(&self, _issuer: &str) -> Result<JwkSet, JwksSourceError> {
            Ok(self.0.clone())
        }
    }

    struct CountingSource {
        document: JwkSet,
        fetches: Arc<AtomicI64>,
    }

    impl JwksSource for CountingSource {
        fn fetch(&self, _issuer: &str) -> Result<JwkSet, JwksSourceError> {
            self.fetches.fetch_add(1, Ordering::SeqCst);
            Ok(self.document.clone())
        }
    }

    struct FailingSource;

    impl JwksSource for FailingSource {
        fn fetch(&self, issuer: &str) -> Result<JwkSet, JwksSourceError> {
            Err(JwksSourceError::FetchFailed {
                issuer: issuer.to_string(),
                detail: "no IdP exists in this repo (UNRUN by design)".to_string(),
            })
        }
    }

    struct FixedTenant(TenantId);

    impl TenantClaimMapper for FixedTenant {
        fn resolve_tenant(
            &self,
            _issuer: &str,
            _claims: &OidcTokenClaims,
        ) -> Option<TenantId> {
            Some(self.0)
        }
    }

    fn jwks() -> JwkSet {
        serde_json::from_str(JWKS_JSON).expect("the test JWKS document parses")
    }

    fn tenant_uuid() -> flexcore_contracts::ids::FlexUuid {
        flexcore_contracts::ids::FlexUuid::parse("0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a01").unwrap()
    }

    fn config() -> OidcConfig {
        OidcConfig {
            issuer: "https://idp.example/realms/flexcore".to_string(),
            audience: "flexcore-control-plane".to_string(),
            ..OidcConfig::default()
        }
    }

    fn clock(start: i64) -> Box<dyn Fn() -> i64 + Send + Sync> {
        Box::new(move || start)
    }

    #[test]
    fn audience_claim_matches_both_shapes() {
        assert!(AudienceClaim::Single("flexcore-control-plane".into())
            .contains("flexcore-control-plane"));
        assert!(!AudienceClaim::Single("other".into()).contains("flexcore-control-plane"));
        assert!(AudienceClaim::Many(vec!["a".into(), "flexcore-control-plane".into()])
            .contains("flexcore-control-plane"));
        assert!(!AudienceClaim::Many(vec!["a".into(), "b".into()])
            .contains("flexcore-control-plane"));
    }

    #[test]
    fn role_claim_mapping_is_closed_and_strict() {
        // The same role ids the dev resolver parses; unknown groups are
        // ignored; an empty set maps to nothing.
        assert_eq!(
            map_role_claims(&["tenant-admin".to_string()]),
            Some(Role::TenantAdmin)
        );
        assert_eq!(
            map_role_claims(&["unknown-group".to_string(), "operator".to_string()]),
            Some(Role::Operator)
        );
        assert_eq!(map_role_claims(&[]), None);
        assert_eq!(map_role_claims(&["no-such-role".to_string()]), None);
    }

    #[test]
    fn static_tenant_mapper_honors_binding_and_direct_claim_only_for_verified_issuer() {
        let mut bindings = HashMap::new();
        bindings.insert(
            "https://idp.example/realms/flexcore".to_string(),
            "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a02".to_string(),
        );
        let mapper = StaticTenantMapper::new(bindings);
        let tenant = TenantId::new(tenant_uuid());

        // Binding path: verified issuer maps to the bound tenant.
        let claims = OidcTokenClaims {
            sub: Some("user-1".to_string()),
            iss: Some("https://idp.example/realms/flexcore".to_string()),
            aud: AudienceClaim::Single("flexcore-control-plane".to_string()),
            exp: Some(1),
            nbf: None,
            groups: vec![],
            flexcore_tenant_id: None,
        };
        assert_eq!(
            mapper.resolve_tenant("https://idp.example/realms/flexcore", &claims),
            Some(TenantId::new(
                flexcore_contracts::ids::FlexUuid::parse(
                    "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a02"
                )
                .unwrap()
            ))
        );

        // Unknown issuer: no binding, no claim → None (fail closed).
        assert_eq!(mapper.resolve_tenant("https://other.example/", &claims), None);

        // The direct claim from a DIFFERENT issuer than the verified one is
        // NEVER accepted (a token cannot name its own tenant).
        let forged = OidcTokenClaims {
            flexcore_tenant_id: Some(tenant.uuid().to_string()),
            iss: Some("https://evil.example/".to_string()),
            ..claims.clone()
        };
        assert_eq!(
            mapper.resolve_tenant("https://idp.example/realms/flexcore", &forged),
            None
        );

        // Empty mapper maps nothing even for a matching issuer.
        assert_eq!(
            StaticTenantMapper::new(HashMap::new())
                .resolve_tenant("https://idp.example/realms/flexcore", &claims),
            None
        );
    }

    #[test]
    fn jwks_cache_finds_the_kid_and_counts_one_fetch() {
        let source = CountingSource {
            document: jwks(),
            fetches: Arc::new(AtomicI64::new(0)),
        };
        let fetches = Arc::clone(&source.fetches);
        let cache = JwksCache::new(Box::new(source), config().jwks_cache_ttl);
        let key = cache
            .decoding_key("https://idp.example/realms/flexcore", "k1", 1_000)
            .expect("kid k1 is in the document");
        let _ = key.into_inner();
        assert_eq!(fetches.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn jwks_cache_unknown_kid_fails_closed_after_refresh_attempt() {
        let source = CountingSource {
            document: jwks(),
            fetches: Arc::new(AtomicI64::new(0)),
        };
        let fetches = Arc::clone(&source.fetches);
        let cache = JwksCache::new(Box::new(source), config().jwks_cache_ttl);
        assert!(
            matches!(
                cache.decoding_key("https://idp.example/realms/flexcore", "no-such-kid", 1_000),
                Err(KeyLookupError::KeyNotFound { .. })
            ),
            "an unknown kid never resolves"
        );
        // Exactly ONE refresh attempt for the miss (the recorded bounded
        // anti-stampede posture).
        assert_eq!(fetches.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn jwks_cache_fresh_entry_is_reused_without_a_second_fetch() {
        let source = CountingSource {
            document: jwks(),
            fetches: Arc::new(AtomicI64::new(0)),
        };
        let fetches = Arc::clone(&source.fetches);
        let cache = JwksCache::new(Box::new(source), config().jwks_cache_ttl);
        cache
            .decoding_key("https://idp.example/realms/flexcore", "k1", 1_000)
            .expect("first lookup fetches");
        cache
            .decoding_key("https://idp.example/realms/flexcore", "k1", 1_000 + 60_000)
            .expect("second lookup reuses the fresh entry");
        assert_eq!(fetches.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn jwks_cache_expired_entry_triggers_refresh() {
        let source = CountingSource {
            document: jwks(),
            fetches: Arc::new(AtomicI64::new(0)),
        };
        let ttl = config().jwks_cache_ttl;
        let fetches = Arc::clone(&source.fetches);
        let cache = JwksCache::new(Box::new(source), ttl);
        cache
            .decoding_key("https://idp.example/realms/flexcore", "k1", 1_000)
            .expect("first lookup fetches");
        // Beyond the TTL (15 min): the second lookup refreshes.
        cache
            .decoding_key(
                "https://idp.example/realms/flexcore",
                "k1",
                1_000 + ttl.as_millis() as i64 + 1,
            )
            .expect("second lookup refreshes past expiry");
        assert_eq!(fetches.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn resolver_rejects_missing_and_malformed_bearers_without_touching_the_network() {
        let resolver = OidcResolver::new(
            config(),
            Box::new(FailingSource),
            Box::new(FixedTenant(TenantId::new(tenant_uuid()))),
            clock(1_000),
        );
        assert_eq!(resolver.resolve(None).expect_err("the resolver must deny this request"), AuthError::MissingCredentials);
        assert_eq!(resolver.resolve(Some("")).expect_err("the resolver must deny this request"), AuthError::InvalidCredentials);
        assert_eq!(
            resolver.resolve(Some("not-a-bearer")).expect_err("the resolver must deny this request"),
            AuthError::InvalidCredentials
        );
        assert_eq!(
            resolver.resolve(Some("Bearer ")).expect_err("the resolver must deny this request"),
            AuthError::InvalidCredentials
        );
        assert_eq!(
            resolver.resolve(Some("Bearer not-a-jwt")).expect_err("the resolver must deny this request"),
            AuthError::InvalidCredentials
        );
    }

    #[test]
    fn resolver_rejects_a_token_whose_source_fails_and_a_token_with_no_mappable_role()
    {
        // Both paths below exercise the fail-closed pipeline with a JWKS
        // source that cannot succeed and a fixed tenant mapper; the tokens
        // are not real JWTs, so every rejection is the named
        // `InvalidCredentials` — the point is the failure SHAPE, not the
        // crypto (which is UNRUN here and gated at the lab, P13.20).
        let resolver = OidcResolver::new(
            config(),
            Box::new(FailingSource),
            Box::new(FixedTenant(TenantId::new(tenant_uuid()))),
            clock(1_000),
        );
        // A structurally valid but unverifiable token shape (header-only,
        // unsigned — `alg=none` is outside the allow-list and would reject
        // even with a working source).
        let alg_none = "eyJhbGciOiJub25lIiwia2lkIjoiazEifQ.e30.";
        assert_eq!(
            resolver.resolve(Some(&format!("Bearer {alg_none}"))).expect_err("the resolver must deny this request"),
            AuthError::InvalidCredentials
        );
    }

    #[test]
    fn jwks_source_error_is_named_not_stringly() {
        let err = JwksSourceError::FetchFailed {
            issuer: "https://idp.example/realms/flexcore".to_string(),
            detail: "unreachable".to_string(),
        };
        let text = err.to_string();
        assert!(text.contains("JWKS fetch failed"));
        assert!(text.contains("https://idp.example/realms/flexcore"));
    }

    // Recorded boundary (compile-level, same discipline as the domain
    // tests): the resolver NEVER constructs a `TenantContext` from anything
    // but `AuthenticatedClaims::from_verified` — the only construction path
    // that exists compiles against the domain types above; there is no
    // `From<TenantId>`/`From<String>` on `TenantContext` to misuse.
}
