# HANDOVER — Phase 13: Enterprise Identity, Administration and Integration Platform

Status: **implementation complete (expected 22/22), everything
WRITTEN-UNRUN.** The ledger's P13.01–P13.14 ticks are applied (by the
orchestrator, citing records 01–08 and 21–26); P13.15–P13.22 carry the
wave-2 decision records (Agent A: records 41–47 and the wave-2 close-out
48; Agent B: the phase close record 61) and their ticks are the
ORCHESTRATOR's action, applied after this close (program conventions
§2.2 — no agent ever ticks a checkbox). The phase gate is **NOT MET** as
a runtime claim: no IdP is deployed, no OIDC token is validated, no SCIM
object is provisioned, no notification is sent, no webhook is delivered,
no MFA enrollment exists, no login is performed — the first human build
of the Phase 0–10 workspace and the P13.20 lab runbook are the
verification gates for every statement below.

## What was implemented (all WRITTEN, never run)

| Area | Subtasks | Records | Delivered (written) |
|---|---|---|---|
| Identity architecture + OIDC | P13.01, P13.02 | 01, 02 | the P13-E-1 registration (OIDC-first, SAML via broker, per-tenant SSO connections as tenant data); the `OidcResolver` middleware composing the `IdentityResolver` seam (JWKS cache port, `iss`/`aud`/`exp` validation policy, the single `AuthenticatedClaims::from_verified` construction path preserved); `DenyAllResolver` stays the shipped default |
| SCIM + step-up + roles + approval engine | P13.03, P13.04, P13.05, P13.06 | 03, 04, 05, 06 | SCIM lifecycle/group→role/deprovisioning-SLA design (no conformance claim); the server-side step-up protocol (`stepup.rs`, fail-closed default verifier, the no-wedge rule); `Role::SiteApprover` + the additive `apikey.*` actions (`phase13_policies.rs`); the multi-step approval workflow engine appended OUTSIDE the frozen 14-state/27-edge set (`record_command_approval`-shaped accumulation) |
| Machine identity + API keys + the secret FORMAT | P13.07, P13.08 | 07, 08 | OAuth2 client-credentials design for external integrations (Phase 16 consumes, never re-decides); the API-key tier (digest-at-rest, rotation overlap, last-used evidence); the webhook signing-secret FORMAT defined ONCE (`whsec_<43 unpadded base64url>`, 32 bytes) — every downstream artifact consumes it by citation |
| Webhooks + versioning + notifications + audit export | P13.09, P13.10, P13.11, P13.12 | 21, 22, 23, 24 | signed delivery service (HMAC-SHA-256 over `timestamp.rawbody`, 5-min tolerance, book-before-attempt, bounded backoff, DLQ + depth gauge, migration 0020 with references+fingerprints never material); the versioning/deprecation policy (v1 CURRENT, Sunset per RFC 8594, registered 180/365-day bounds, the generated-spec lint); the notification pipeline over receipt-only ports (never a dispatch path, P13-E-2 registered OPEN); signed audit export packs with the P11.15 chain-inheritance gap stated verbatim |
| Privacy + jurisdiction | P13.13, P13.14 | 25, 26 | the workspace data inventory (by citation + the Phase 13 additions); the per-jurisdiction retention matrix keyed on the registry (ALL value/basis cells EMPTY — no universal default anywhere); DSAR design; the backup RE-MATERIALIZATION caveat stated plainly; the jurisdiction registry shape (sign-off = human gate; D1 stays OPEN) |
| Tenant lifecycle + sessions + console + admin UI | P13.15, P13.16, P13.17, P13.18 | 41, 42, 43, 44 | the onboarding/offboarding procedures under RLS (revocation-first order; export reuses the record-24 pack format); session management denying at the RESOLVER (revocation, timeouts, cap — all REGISTERED RECOMMENDATIONS); console auth hardening (specified-not-implemented login endpoints; the enforced-protocol step-up machine; the session banner); the admin management catalog (18 actions, every named endpoint specified-not-implemented; display-never-secret) |
| Tests + lab spec + interplay + close | P13.19, P13.20, P13.21, P13.22 | 45, 46, 47, 48, 61 | five written-unrun Rust test files + six console test files (execution = the human gate); the Keycloak lab compose profile + realm draft (pinned `26.8.1`, no pull, runbook R-1..R-8); the 26-row disposition registry importing `FROZEN_EVENT_NAMES` (4 Notify / 22 AuditOnly, agreeing with the untouched P13.11 map); this HANDOVER + CHECKLIST + the close record |

## Blockers carried (human gate inputs)

1. **The first human build** (`cargo build --workspace` → clippy →
   `cargo test --workspace`): every Rust artifact in this phase is
   written against the WRITTEN-UNRUN 0–10 base. Recorded compile
   ASSUMPTIONS: the `jsonwebtoken = "11.1.0"` pin (record 02 §5), the
   wave-2 modules and five test files (record 45 §5), the exhaustive-
   `Role`-match audit (record 05 §7), and the hand-written SHA-256/
   HMAC-SHA-256 vector tests in `webhook_signing.rs` (record 21 §4 —
   build-blocking on mismatch). The first console install runs the six
   new vitest files (`npm test`).
2. **The P13.20 lab runbook (R-1..R-8) is the identity gate**: the first
   image pull, the first realm import, the first real token, the first
   JWKS rotation, the first SCIM push, the first login, the record 45 §3
   SCIM scenario cells — all UNRUN and all human-gated.
3. **Every threshold is a RECOMMENDATION**: the 60 s leeway / 15 min
   JWKS TTL (record 02), the 5-min step-up TTL (record 04), the 5-min
   deprovisioning window (record 03), the 24 h key-rotation overlap and
   90-day unused-key threshold (record 08), the webhook backoff/
   tolerance/DLQ constants (record 21 §3), the 180/365-day deprecation
   bounds (record 22), the 30 min / 12 h / cap-5 session policies
   (record 42) — X-8 pre-registered before any run that could tune them.
4. **The three E-decisions are OPEN** (P13-E-1 record 01 §1; P13-E-2
   record 23 §4; P13-E-3 record 04 §5) — agents never close them; the
   closers and the closing evidence are named in each register entry.

## Decisions still needed (the closers)

- P13-E-1 (identity provider): the human owner's budget/vendor sign-off
  on the Keycloak DEFAULT; commercial IdPs remain supported as per-tenant
  SSO connections either way.
- P13-E-2 (notification channel providers): the human owner's vendor
  selection (a provider account, its contract, its secrets under the
  P11.16 posture); email-first DEFAULT stands until then.
- P13-E-3 (MFA mechanism): the human owner's mechanism sign-off
  (IdP-delegated primary, TOTP fallback, WebAuthn/passkeys the forward
  path); the shipped default verifier denies everything until composed
  and signed off.
- D1 / LR-062: the jurisdiction decision + the per-tenant legal sign-offs
  (record 26's `SIGNED` gate) — legal sign-off is a human gate, never an
  agent act.

## Known risks / recorded deviations

See the close record `records/61_p13_22_phase_close.md` §3 for the full
audit list. Headlines: no IdP, no token, no SCIM object, no notification,
no webhook delivery, no MFA enrollment, no login exists anywhere (every
evidence cell EMPTY/UNRUN); the Postgres adapter implementations of the
webhook/session stores are recorded follow-ups (records 21 §5, 42 §5);
the audit-export chain-column gap is stated verbatim in every manifest
(record 24 §2); the backup re-materialization caveat is a REAL gap, not a
resolved risk (record 25 §4); one pipe-count inconsistency in record 43
(a literal `|` inside a table cell — Agent A's file, recorded not fixed)
and the ledger's stale trailing status line are recorded for the
orchestrator (close record §3.7).

Full close position: `records/61_p13_22_phase_close.md`.
