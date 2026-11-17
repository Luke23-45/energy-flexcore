# P13.07 — Machine identity (OAuth2 client-credentials JWT for external integrations)

| Field | Value |
|---|---|
| Subtask | P13.07 — OAuth2 client-credentials JWT issuance/validation design for external integrations (grid-operator systems, Phase 16 market adapters); per-client registration, scope subsets, revocation |
| Wave / agent | Phase 13 wave 1, Agent A |
| Status | DESIGN RECORD — no client is registered, no client secret exists, no token is issued, no token is validated, no revocation was exercised. Every operational evidence cell is EMPTY/UNRUN. Phase 16 consumes this record; nothing here touches market-adapter code. |
| Sources | [S-76] (SPIFFE, gap analysis §11 — the workload-identity direction FlexCore deliberately does NOT adopt in this phase, recorded); [S-77] (enterprise SSO + Keycloak — the IdP that can host the client registry); RFC 6749 §4.4 (client-credentials grant), RFC 7523 (JWT client authentication), RFC 9068 (JWT access tokens) cited as primary standards; gap analysis §9 (the P16 seam) |
| Files touched | none (design record only) |

## 1. Position and scope

Machine identity is the OAuth2 **client-credentials tier** of the
identity architecture (record 01 §2): a named non-human principal — a
grid-operator integration, a Phase 16 market adapter — authenticates
with its own credential and receives a short-lived JWT access token it
presents as the bearer to the same `IdentityResolver` surface humans
use. Distinct from:

- human SSO (records 01/02),
- the legacy/simple API-key tier (record 08),
- edge-device identity (Phase 18 — NOT re-decided here),
- SPIFFE/SPIRE mesh identity ([S-76] — recorded as the deferred
  multi-host direction; this phase standardizes OAuth2 client
  credentials because the integration partners named in the gap
  analysis speak OAuth2, not SPIFFE).

## 2. The client-credentials flow (design; the RFC-documented shape)

| Step | Design |
|---|---|
| Registration | a machine client is a registered principal: `client_id`, tenant binding (one client belongs to exactly one tenant — a client never crosses the tenant boundary), a scope subset (§3), a secret (or a private key for `private_key_jwt`, the RFC 7523 form — RECOMMENDED for market adapters), and its own audit identity. Registration is tenant-administered data; the P11.16 secrets posture is where the secret material lands |
| Token issuance | `POST /token` with `grant_type=client_credentials`: the server authenticates the client (secret or `private_key_jwt`), and issues a SHORT-LIVED JWT access token (RFC 9068 shape): `iss` = the FlexCore token endpoint's issuer, `sub` = the client id, `aud` = the control-plane API, `exp` TTL (REGISTERED RECOMMENDATION: 15 minutes — short-lived credentials per [S-75]'s posture; never a tested optimum), plus the client's scope subset |
| Validation | the SAME P13.02 validation pipeline (signature, exact `iss`, `aud` membership, `exp` leeway) with the tenant mapper resolving the tenant from the client registration — never from a token claim alone. A client-credentials token is indistinguishable in shape from a user token at the resolver; only the subject grammar (`integration:<client_id>`) differs |
| Scope enforcement | the token's scope subset is evaluated against the typed `Action` vocabulary exactly like every other principal (deny-by-default `PolicySet`); the scope list can only name actions that exist on the closed surface |

## 3. Scope subsets and the never-granted set

A client's scope subset is a strict subset of the typed `Action`
vocabulary (the same rule as record 08's API keys). The never-granted
permission set is **byte-identical**: `dispatch.authorize`,
`dispatch.issue`, `secret.read`, `network.any` — none is on the
`Action` surface, none is requestable or grantable by any machine
identity, and a market adapter reaches dispatch reach ONLY through the
command lifecycle (the P4.11–P4.13 gates; conventions §6.4). A grid-
operator integration reads telemetry and submits candidates; it never
authorizes or issues a command.

## 4. Per-client registration and revocation

| Element | Design |
|---|---|
| Per-client registration | one row per client, tenant-scoped: id, tenant, scope subset, credential class (secret / private key), state (`enabled`/`disabled`), rotation fields, audit fields. No shared "integration login" exists — each integration is a named principal with its own audit identity (the glossary's service-account rule) |
| Rotation | secret/key rotation with an overlap window (the record 08 pattern consumed by citation — same mechanics, different credential class) |
| Revocation | disabling a client rejects new token issuance immediately; issued tokens die at `exp` (short TTL bounds the exposure window — the reason the TTL is minutes, not hours). An emergency kill-switch sets state → `disabled`; the resolver refuses tokens whose client is disabled at validation time (the client-state check is part of tenant resolution, fail-closed) |
| Audit | registration, rotation, revocation, and every denied token request are audit-record facts (the append-only trail; the P11.15 keyed chain inherits when keyed) |

## 5. What Phase 16 consumes (and what it must not re-decide)

Phase 16's market adapters consume THIS record: the client-credentials
flow shape, the scope-subset rule, the revocation semantics, and the
subject grammar. They do not re-decide the identity architecture, do
not mint their own token format, and do not widen the scope surface
(any new market-surface action extends the closed `Action` enum
additively — the recorded closed-enum extension rule).

## 6. ASSUMPTION / UNRUN register

1. **ASSUMPTION** — the flow/protocol descriptions are the
   RFC 6749/RFC 7523/RFC 9068-documented behavior of those standards;
   none has been exercised against FlexCore. The 15-minute TTL is a
   REGISTERED RECOMMENDATION (X-8), not a tested value.
2. **UNRUN** — no client registered, no secret created, no token
   issued or validated, no revocation exercised. Evidence cells EMPTY.
3. **NOT BUILT (deliberate)** — the token endpoint, the client registry
   store, and the issuance code: this subtask is a design record; the
   issuance implementation is a later implementation wave with its own
   reviewed crates (the plan §7 rule — proposals become workspace
   members only with real code). The P13.02 resolver already accepts
   the resulting tokens' shape unchanged.
4. **OPEN dependence** — P13-E-1 (record 01) stays OPEN; the DEFAULT
   (Keycloak as the hosting IdP for the client registry) is the design
   basis, and a commercial IdP changes the mechanics, not the
   vocabulary.
5. **ADR-010 boundary** — nothing here grants dispatch authority to any
   machine principal; market adapters reach dispatch only through the
   command lifecycle.

## 7. Handoff notes

1. **Phase 16**: consume this record by citation; the API
   versioning/deprecation policy (P13.10) governs the market API
   surface the tokens authenticate to.
2. **P13.19 (wave 2)**: machine-identity negative tests (disabled
   client, expired token, out-of-scope request) are natural extensions
   of the resolver test matrix.
3. **Phase 18**: edge-device identity stays Phase 18's — an edge agent
   is NOT a machine client under this record.
