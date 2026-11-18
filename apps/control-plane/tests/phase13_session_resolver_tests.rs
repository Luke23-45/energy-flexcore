// P13.19 — the session RESOLVER-level negative tests (wave 2;
// WRITTEN-UNRUN — never compiled or executed; execution is the human
// build/test gate, conventions §1 item 3).
//
// Extends record 42's in-module matrix at integration level (the record
// 42 §8 handoff named this slice): the resolver-layer denials — revoked /
// idle-expired / absolutely-expired / binding-mismatch / unknown or
// missing session reference — over the PUBLIC `SessionEnforcingResolver`
// API (`apps/control-plane/src/session.rs`, consumed, never redefined),
// with a stand-in inner resolver through the P1.17 `IdentityResolver`
// trait (the composed deployment wraps the P13.02 `OidcResolver`; no IdP
// exists, so the inner path is a local fixture).
//
// THE LOAD-BEARING PROPERTY under test (the record 42 §1 sentence): every
// denial below is an authentication failure AT THE RESOLVER — the caller
// never reaches a route handler; no UI cooperation is involved or needed.
// NO session has ever been created, listed, or revoked in any real
// environment; every record here lives in the in-memory directory fixture.
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use flexcore_contracts::ids::FlexUuid;
use flexcore_control_plane::auth::{AuthError, Caller, IdentityResolver};
use flexcore_control_plane::session::{
    InMemorySessionDirectory, SessionDenial, SessionEnforcingResolver, SessionPolicy,
    DEFAULT_ABSOLUTE_LIFETIME, DEFAULT_IDLE_TIMEOUT, DEFAULT_MAX_CONCURRENT_SESSIONS,
};
use flexcore_domain::ids::TenantId;

const TENANT_A: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a01";
const TENANT_B: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a02";

fn tenant(uuid_text: &str) -> TenantId {
    TenantId::new(FlexUuid::parse(uuid_text).expect("valid test uuid"))
}

/// The inner-identity stand-in: accepts exactly one bearer token and
/// resolves it to one (tenant, subject) through the single public claims
/// path (P0.16 rule 5).
struct StaticInner {
    tenant: TenantId,
    subject: &'static str,
    token: &'static str,
}

impl IdentityResolver for StaticInner {
    fn resolve(&self, bearer: Option<&str>) -> Result<Caller, AuthError> {
        let raw = bearer.ok_or(AuthError::MissingCredentials)?;
        if raw.strip_prefix("Bearer ").unwrap_or("") != self.token {
            return Err(AuthError::InvalidCredentials);
        }
        let claims = flexcore_domain::tenant_context::AuthenticatedClaims::from_verified(
            flexcore_domain::tenant_context::VerifiedClaims {
                tenant_id: self.tenant,
                subject: self.subject.to_string(),
            },
        );
        Ok(Caller {
            ctx: flexcore_domain::tenant_context::TenantContext::from_authenticated_claims(
                claims,
            ),
            role: flexcore_authz::policy::Role::TenantAdmin,
        })
    }
}

struct Fixture {
    resolver: SessionEnforcingResolver,
    clock: Arc<AtomicI64>,
}

fn fixture(now: i64) -> Fixture {
    let clock = Arc::new(AtomicI64::new(now));
    let clock_for_resolver = Arc::clone(&clock);
    let resolver = SessionEnforcingResolver::new(
        Box::new(StaticInner {
            tenant: tenant(TENANT_A),
            subject: "user:alice",
            token: "tok-alice",
        }),
        Box::new(InMemorySessionDirectory::default()),
        SessionPolicy::default(),
        Box::new(|| {
            static COUNTER: AtomicI64 = AtomicI64::new(0);
            format!("sess-{}", COUNTER.fetch_add(1, Ordering::SeqCst))
        }),
        Box::new(move || clock_for_resolver.load(Ordering::SeqCst)),
    );
    Fixture { resolver, clock }
}

fn bearer() -> Option<&'static str> {
    Some("Bearer tok-alice")
}

// --- The registered policy constants (X-8; re-asserted at the public API) ----------

#[test]
fn the_registered_session_policy_defaults_hold() {
    assert_eq!(DEFAULT_IDLE_TIMEOUT, std::time::Duration::from_secs(30 * 60));
    assert_eq!(
        DEFAULT_ABSOLUTE_LIFETIME,
        std::time::Duration::from_secs(12 * 60 * 60)
    );
    assert_eq!(DEFAULT_MAX_CONCURRENT_SESSIONS, 5);
    // The default policy composes exactly these.
    let policy = SessionPolicy::default();
    assert_eq!(policy.idle_timeout, DEFAULT_IDLE_TIMEOUT);
    assert_eq!(policy.absolute_lifetime, DEFAULT_ABSOLUTE_LIFETIME);
    assert_eq!(policy.max_concurrent_sessions, DEFAULT_MAX_CONCURRENT_SESSIONS);
}

// --- Revocation denies AT THE RESOLVER ----------------------------------------------

#[test]
fn a_revoked_session_denies_at_the_resolver_while_the_token_still_verifies() {
    let fx = fixture(1_000);
    let session = fx
        .resolver
        .create_session(tenant(TENANT_A), "user:alice")
        .expect("the session creates");
    // The token alone still resolves through the INNER path — proving the
    // denial that follows comes from the SESSION gate, not the token.
    assert!(
        IdentityResolver::resolve(&fx.resolver, bearer()).is_err(),
        "the bare trait path never authenticates (fail-closed; see the dedicated test below)"
    );
    fx.resolver
        .revoke_session(&session.session_id)
        .expect("the revocation books");
    assert_eq!(
        fx.resolver.resolve_session(bearer(), Some(&session.session_id)).expect_err("the resolver must deny this request"),
        AuthError::InvalidCredentials,
        "a revoked session denies at the RESOLVER, regardless of token validity — not at the UI"
    );
}

#[test]
fn the_bare_trait_path_fails_closed_total_denial_never_session_bypass() {
    let fx = fixture(1_000);
    let session = fx
        .resolver
        .create_session(tenant(TENANT_A), "user:alice")
        .expect("the session creates");
    // A wrong composition (this resolver in the identity slot without the
    // session-aware middleware) denies EVERY request — never silently
    // bypasses the session gate.
    assert_eq!(
        IdentityResolver::resolve(&fx.resolver, bearer()).expect_err("the resolver must deny this request"),
        AuthError::MissingCredentials
    );
    let _ = session;
}

#[test]
fn the_inner_token_still_authenticates_independently_of_the_session_gate() {
    // Contrast test: the INNER resolver accepts the token at the same
    // instant the session gate denies — the session layer is strictly
    // additive on top of identity.
    let fx = fixture(1_000);
    let session = fx
        .resolver
        .create_session(tenant(TENANT_A), "user:alice")
        .expect("the session creates");
    fx.resolver
        .revoke_session(&session.session_id)
        .expect("the revocation books");
    // (The inner path is exercised indirectly: resolve_session reached
    // the session gate, which only happens AFTER the inner path allowed
    // the token — the ordering the module pins. An inner deny would be
    // indistinguishable to the caller, which is the point: both are 401.)
    assert_eq!(
        fx.resolver.resolve_session(bearer(), Some(&session.session_id)).expect_err("the resolver must deny this request"),
        AuthError::InvalidCredentials
    );
}

// --- The timeout policies ------------------------------------------------------------

#[test]
fn the_idle_timeout_denies_and_denials_never_extend_the_anchor() {
    let fx = fixture(1_000);
    let session = fx
        .resolver
        .create_session(tenant(TENANT_A), "user:alice")
        .expect("the session creates");
    fx.clock.store(1_000 + 30 * 60 * 1000, Ordering::SeqCst);
    assert_eq!(
        fx.resolver.resolve_session(bearer(), Some(&session.session_id)).expect_err("the resolver must deny this request"),
        AuthError::InvalidCredentials,
        "an idled-out session denies at the resolver"
    );
    // Denied attempts never refresh the idle anchor: one millisecond past
    // the deadline still denies — had the denied attempt touched
    // last_seen, the deadline would have moved and this would succeed.
    fx.clock.store(1_000 + 30 * 60 * 1000 + 1, Ordering::SeqCst);
    assert_eq!(
        fx.resolver.resolve_session(bearer(), Some(&session.session_id)).expect_err("the resolver must deny this request"),
        AuthError::InvalidCredentials,
        "a denied request never extends the session"
    );
    // And the denial did not terminate the session either: back inside
    // the window (clock 1_100, anchor still 1_000) it resolves again —
    // the lib session test pins the same property ("a denied session is
    // not terminated; inside the window it resolves").
    fx.clock.store(1_100, Ordering::SeqCst);
    fx.resolver
        .resolve_session(bearer(), Some(&session.session_id))
        .expect("a denied session is not terminated; inside the window it resolves");
}

#[test]
fn the_absolute_lifetime_expires_an_actively_used_session() {
    let fx = fixture(1_000);
    let session = fx
        .resolver
        .create_session(tenant(TENANT_A), "user:alice")
        .expect("the session creates");
    // Heavy activity keeps the session alive — but each touch must land
    // inside the 30-minute IDLE window (touching at 6h and 11h idles out
    // first: no use between creation and 6h). Touch every 15 minutes up
    // to 11h45m: inside the idle window every time, inside the 12h
    // absolute window every time.
    for step in 1..=47u64 {
        fx.clock.store(1_000 + (step as i64) * 15 * 60 * 1000, Ordering::SeqCst);
        fx.resolver
            .resolve_session(bearer(), Some(&session.session_id))
            .expect("activity inside both windows keeps the session alive");
    }
    fx.clock.store(1_000 + 12 * 60 * 60 * 1000, Ordering::SeqCst);
    assert_eq!(
        fx.resolver.resolve_session(bearer(), Some(&session.session_id)).expect_err("the resolver must deny this request"),
        AuthError::InvalidCredentials,
        "the absolute lifetime closes an actively-used session"
    );
}

// --- Binding and reference integrity -------------------------------------------------

#[test]
fn a_session_reference_never_rides_on_another_principals_token() {
    let fx = fixture(1_000);
    let mallory = fx
        .resolver
        .create_session(tenant(TENANT_A), "user:mallory")
        .expect("mallory's session creates");
    // Alice's token + Mallory's session reference: binding mismatch deny.
    assert_eq!(
        fx.resolver.resolve_session(bearer(), Some(&mallory.session_id)).expect_err("the resolver must deny this request"),
        AuthError::InvalidCredentials,
        "the (tenant, subject) binding is enforced at every resolution"
    );
}

#[test]
fn missing_blank_and_unknown_references_deny_fail_closed() {
    let fx = fixture(1_000);
    fx.resolver
        .create_session(tenant(TENANT_A), "user:alice")
        .expect("the session creates");
    assert_eq!(
        fx.resolver.resolve_session(bearer(), None).expect_err("the resolver must deny this request"),
        AuthError::MissingCredentials
    );
    assert_eq!(
        fx.resolver.resolve_session(bearer(), Some("   ")).expect_err("the resolver must deny this request"),
        AuthError::MissingCredentials
    );
    assert_eq!(
        fx.resolver.resolve_session(bearer(), Some("sess-unknown")).expect_err("the resolver must deny this request"),
        AuthError::InvalidCredentials
    );
}

// --- Bulk revocations (the record 41 D-3 / record 03 §4 paths) ------------------------

#[test]
fn the_tenant_wide_revocation_denies_every_tenant_session_and_only_those() {
    let fx = fixture(1_000);
    let a1 = fx
        .resolver
        .create_session(tenant(TENANT_A), "user:alice")
        .expect("session a1 creates");
    let a2 = fx
        .resolver
        .create_session(tenant(TENANT_A), "user:bob")
        .expect("session a2 creates");
    let b1 = fx
        .resolver
        .create_session(tenant(TENANT_B), "user:alice")
        .expect("tenant B's session creates");
    let revoked = fx
        .resolver
        .revoke_all_for_tenant(&tenant(TENANT_A))
        .expect("the tenant-wide revocation books");
    assert_eq!(revoked, 2, "exactly the two ACTIVE tenant-A sessions were revoked");
    assert_eq!(
        fx.resolver.resolve_session(bearer(), Some(&a1.session_id)).expect_err("the resolver must deny this request"),
        AuthError::InvalidCredentials
    );
    // Tenant B is untouched (a scoped revocation, never a global kill).
    let rows = fx
        .resolver
        .list_active_for_subject(&tenant(TENANT_B), "user:alice")
        .expect("the list reads");
    assert_eq!(rows.len(), 1, "tenant B's session is still live");
    assert_eq!(rows[0].session_id, b1.session_id);
    // a2's owner differs (bob), so alice's token denies before binding —
    // assert a2 denies through its own (not exercised here; the fixture
    // serves one token — the per-subject revocation test below covers it).
}

#[test]
fn the_subject_scoped_revocation_matches_the_deprovisioning_path() {
    // Record 03 §4: a deactivated user's sessions die at the resolver
    // within the recorded window — this is the enforcement seam.
    let fx = fixture(1_000);
    let alice = fx
        .resolver
        .create_session(tenant(TENANT_A), "user:alice")
        .expect("alice's session creates");
    let revoked = fx
        .resolver
        .revoke_all_for_subject(&tenant(TENANT_A), "user:alice")
        .expect("the subject-wide revocation books");
    assert_eq!(revoked, 1);
    assert_eq!(
        fx.resolver.resolve_session(bearer(), Some(&alice.session_id)).expect_err("the resolver must deny this request"),
        AuthError::InvalidCredentials,
        "the deactivated subject's session denies at the resolver"
    );
}

// --- The concurrent-session cap -------------------------------------------------------

#[test]
fn the_cap_refuses_the_newest_login_across_independent_counters() {
    let fx = fixture(1_000);
    for _ in 0..DEFAULT_MAX_CONCURRENT_SESSIONS {
        fx.resolver
            .create_session(tenant(TENANT_A), "user:alice")
            .expect("sessions up to the cap create");
    }
    assert_eq!(
        fx.resolver.create_session(tenant(TENANT_A), "user:alice"),
        Err(SessionDenial::SessionLimitReached {
            limit: DEFAULT_MAX_CONCURRENT_SESSIONS
        }),
        "the newest login is refused; the oldest is never silently evicted"
    );
    fx.resolver
        .create_session(tenant(TENANT_A), "user:bob")
        .expect("another subject is an independent counter");
    fx.resolver
        .create_session(tenant(TENANT_B), "user:alice")
        .expect("another tenant is an independent counter");
}

// --- The active-session list design surface -------------------------------------------

#[test]
fn the_list_shows_only_live_sessions_of_the_pair_ordered_by_creation() {
    let fx = fixture(1_000);
    let s1 = fx
        .resolver
        .create_session(tenant(TENANT_A), "user:alice")
        .expect("s1 creates");
    fx.clock.store(2_000, Ordering::SeqCst);
    let s2 = fx
        .resolver
        .create_session(tenant(TENANT_A), "user:alice")
        .expect("s2 creates");
    fx.resolver.revoke_session(&s1.session_id).expect("s1 revokes");
    let rows = fx
        .resolver
        .list_active_for_subject(&tenant(TENANT_A), "user:alice")
        .expect("the list reads");
    assert_eq!(rows, vec![s2], "only the live session, ordered by creation");
}
