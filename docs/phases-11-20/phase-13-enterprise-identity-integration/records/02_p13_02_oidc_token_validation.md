# P13.02 — OIDC token-validation middleware (the production identity path; LR-047 composition)

| Field | Value |
|---|---|
| Subtask | P13.02 — token-validation middleware replacing `DenyAllResolver` as the PRODUCTION identity path (dev-auth stays non-default and feature-gated); claims → `VerifiedClaims` → `AuthenticatedClaims::from_verified` mapping; JWKS caching, clock-skew/leeway policy, `iss`/`aud`/`exp` validation rules |
| Wave / agent | Phase 13 wave 1, Agent A |
| Status | WRITTEN-UNRUN throughout: the middleware module and its tests are written; nothing has been compiled (the jsonwebtoken pin is a recorded compile ASSUMPTION), no token has ever been validated, no JWKS document has been fetched, no discovery document has been read. No IdP exists (P13-E-1 OPEN — the DEFAULT is implemented per conventions §4, dependence recorded). |
| Sources | [S-77] (enterprise SSO + Keycloak, gap analysis §11); LR-047 (`ledger/phase-09-pilot-qualification/27_limitations_residual_risk.md` §3.8); record 01 (the identity architecture); OpenID Connect Core 1.0 / RFC 7519 (JWT) / RFC 7517 (JWK) cited as primary standards; jsonwebtoken crate page (crates.io/crates/jsonwebtoken, fetched 2026-11-14) for the pin |
| Files touched | `apps/control-plane/src/oidc.rs` (NEW), `apps/control-plane/src/lib.rs` (additive module lines, marked), `apps/control-plane/Cargo.toml` (additive pin + comment), `apps/control-plane/src/auth.rs` (additive: `parse_role` gains the `site-approver` arm — shared with the OIDC group→role mapping) |

## 1. What was implemented (written, never run)

| Piece | Where | Honest status |
|---|---|---|
| `OidcResolver` — an `IdentityResolver` implementation: bearer parse → header decode (`kid`, `alg` allow-list check BEFORE any key material) → JWKS key lookup (cache → refresh-on-unknown-kid → port) → crate-validated signature/`iss`/`aud`/`exp`/`nbf` → subject presence → tenant mapping (fail-closed) → role mapping (closed, fail-closed) → `VerifiedClaims` → `AuthenticatedClaims::from_verified` → `TenantContext` → `Caller` | `oidc.rs` | Implemented code; UNRUN. `DenyAllResolver` remains the SHIPPED default; composition is a deployment act (`main.rs` untouched — no IdP exists to point at) |
| `JwksSource` port + `JwksCache` (issuer → (document, fetched-at); TTL default 15 min REGISTERED RECOMMENDATION; refresh-on-unknown-`kid`, ONE bounded attempt per miss; expired cache never "verifies with the stale key anyway" — refresh failure is fail-closed) | `oidc.rs` | The cache mechanics are implemented and unit-tested with static documents; THE NETWORK HALF IS UNRUN BY DESIGN — the port has no HTTP client in this repo, implementations are deployment artifacts |
| `TenantClaimMapper` / `StaticTenantMapper` — issuer→tenant binding table (tenant data per record 01 §4); the direct `flexcore_tenant_id` claim is honored ONLY from the verified issuer (a token cannot name its own tenant); unmapped tenant fails closed | `oidc.rs` | Implemented; the DB-backed connection store is a future composition-root artifact (none exists) |
| `map_role_claims` — token groups → the closed `Role` vocabulary via the SAME `parse_role` the dev path uses; unknown groups ignored; NO mappable role rejects (never defaults to Viewer) | `oidc.rs` + the additive `auth.rs::parse_role` arm | Implemented |
| Validation policy as code: RS256-only DEFAULT allow-list; `iss` exact match; `aud` membership; `exp` required, leeway DEFAULT 60 s; `nbf` validated when present (same leeway) — all REGISTERED RECOMMENDATIONS (X-8) | `oidc.rs` | Implemented as `OidcConfig` (DEFAULT 60 s leeway, 15 min cache TTL, RS256-only) |
| Tests: audience-shape matching, closed role mapping, tenant mapper (binding / unknown issuer / forged cross-issuer tenant claim / empty table), JWKS cache (kid found, unknown kid fail-closed after ONE refresh, fresh reuse, expired refresh), resolver rejection shapes (missing/malformed bearer, `alg=none`) | `oidc.rs` `#[cfg(test)]` | Written complete; UNRUN (conventions §1 item 3 — execution is the human gate) |

## 2. The claims mapping (the P1.06 attestation point)

The domain crate's contract (`crates/flexcore-domain/src/tenant_context.rs`)
is preserved exactly: there is still exactly ONE construction path,
`AuthenticatedClaims::from_verified(VerifiedClaims { tenant_id, subject })`,
and the resolver is the reviewed call site that attests the token was
verified. The mapping:

| Token fact | Target | Rule |
|---|---|---|
| `sub` (required) | `VerifiedClaims.subject` | non-empty; absence rejects |
| verified issuer ↔ tenant binding (tenant data) or `flexcore_tenant_id` from the VERIFIED issuer | `VerifiedClaims.tenant_id` | unmapped ⇒ REJECT (no default tenant, P0.16 rule 5) |
| token `groups` (via closed `parse_role` ids) | `Caller.role` | no mappable role ⇒ REJECT |
| everything else in the token | ignored | only the claims above participate in identity |

Nothing about the mapping was changed in the domain crate —
`tenant_context.rs` is byte-identical after this subtask.

## 3. Dev-auth stays exactly where it was

`DevAuthResolver` remains compiled ONLY behind the non-default `dev-auth`
feature; `DenyAllResolver` remains the default when the feature is off;
CI still fails if `dev-auth` appears in default features. The production
path does not reuse, wrap, or weaken the dev path — the only shared
surface is `parse_role`'s closed id strings, deliberately, so dev and
production map role ids through ONE vocabulary.

## 4. Dependency pin (the single new dependency)

| Element | Value |
|---|---|
| Crate | `jsonwebtoken` |
| Pin | `11.1.0` — verified to exist on crates.io on 2026-11-14 (published 2026-09-16). A `12.0.0` exists (published 2026-11-13, one day before this edit) and was deliberately NOT taken: a same-week major-line jump is not a reviewed pin |
| Where | `apps/control-plane/Cargo.toml` (a composition crate — the only place infrastructure dependencies are allowed; no core-crate manifest was touched and `tools/check_dependency_rule.py` is unaffected) |
| What it is used for | `decode_header`, `Validation` (`set_issuer`/`set_audience`/`leeway`/`validate_exp`/`validate_nbf`/`algorithms`), `DecodingKey::from_jwk`, `JwkSet::find` |
| What it is NOT used for | JWKS fetching (the port keeps network behavior out), token issuance (P13.07 designs issuance for machine clients; nothing issues tokens in this repo) |

## 5. ASSUMPTION / UNRUN register

1. **ASSUMPTION (compile)** — the pinned `jsonwebtoken = "11.1.0"`
   compiles with the workspace (MSRV 1.88 per the crate page) and exposes
   the API surface §4 lists. Verified ONLY at the first human build.
2. **UNRUN** — every runtime behavior: no token validated, no JWKS
   fetched, no discovery read, no cache refresh against a live IdP, no
   tenant mapping against real connections. Evidence cells EMPTY.
3. **UNRUN (by design)** — the `JwksSource` port's real implementation
   (HTTPS fetch): network behavior is a runtime act, deliberately outside
   this repo. The lab spec (P13.20) is where it lands.
4. **REGISTERED, not tuned** — 60 s leeway, 15 min cache TTL, RS256-only
   allow-list, one bounded refresh per unknown kid. None is a tested
   optimum; all are pre-registered recommendations pending the operator
   sign-off X-8 requires.
5. **RECORDED LIMITATION** — the crate's `Validation` applies `exp`/`nbf`
   checks against the SYSTEM clock at decode time; the injected clock in
   this module governs JWKS CACHE AGE only. A deployment needing a
   test-controlled token clock owns that at the verifier seam, not here.

## 6. Handoff notes

1. **Wave 2 (P13.16/P13.17)**: session management and console login
   consume THIS resolver as the identity path — compose it via the
   state's `identity` field, never by editing `auth.rs`'s default rule.
2. **P13.19 (wave 2 tests)**: the `iss`/`aud`/`exp`/leeway matrix and the
   never-defaulted tenant/role rejections are the integration-test
   targets; the unit tests here cover the pure mechanics.
3. **Human gates**: (a) first build — the pin's compile ASSUMPTION and
   the module's tests; (b) first lab run (P13.20) — a real IdP, a real
   JWKS rotation, the refresh-on-unknown-kid path against live behavior.
