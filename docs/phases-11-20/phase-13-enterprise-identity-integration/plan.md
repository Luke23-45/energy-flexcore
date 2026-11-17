# Phase 13 — Enterprise Identity, Administration and Integration Platform — Plan

Date: 2026-11-14. Status: **NOT STARTED**. Planning artifact: nothing is
deployed, configured, sent, or delivered by this plan. Rules:
`../01_PROGRAM_CONVENTIONS.md`; sources: `../00_GAP_ANALYSIS.md` §11
(`[S-n]`); background: `context.md`.

## 1. Scope

Phase 13 designs and specifies FlexCore's enterprise identity and
integration layer, closing the LR-047/048/049 identity gaps and the LR-062
privacy-regime direction honestly: an identity architecture decision
(OIDC-first, SAML via broker, Keycloak DEFAULT), OIDC token validation that
composes the LR-047 "future composition", SCIM 2.0 provisioning, enforced
server-side step-up (closing LR-048), a refined role model (LR-049
direction), a multi-step approval workflow that extends the P4.13 gate
without touching the frozen 27-edge set, machine identity and API keys,
Stripe-grade outbound webhooks, API versioning and deprecation policy, a
notification pipeline, audit export, GDPR/privacy engineering and a
jurisdiction registry, tenant lifecycle operations, session management,
console hardening, admin workspace completion, a written-unrun test suite,
and a lab environment specification. Nothing in this phase deploys an IdP,
validates a real token, sends a notification, or delivers a webhook — every
such act is a human gate on the lab environment (P13.20).

Out of scope: SOC 2 / certification evidence collection (Phase 19 consumes
P13's audit export and privacy records); market-adapter identity
consumption (Phase 16 consumes P13.07); edge device identity (Phase 18);
Vault deployment (the P11.16 posture is consumed, not re-decided);
customer-facing experience (recorded out-of-scope decision, gap analysis
§8).

## 2. Subtask outline (22)

| ID | Subtask | Output (all WRITTEN-UNRUN unless stated) | Citations / seams |
|---|---|---|---|
| P13.01 | Identity architecture decision record | OIDC-first with SAML supported via an OIDC broker; Keycloak DEFAULT recommendation vs a commercial IdP; per-tenant SSO connections (each tenant's IdP connection is tenant data, not platform config); recorded as P13-E-1 | [S-77]; LR-047 |
| P13.02 | OIDC integration design | token validation middleware design replacing `DenyAllResolver` as the PRODUCTION identity path (dev-auth stays non-default and feature-gated); claims → `VerifiedClaims` → `AuthenticatedClaims::from_verified` mapping; JWKS caching, clock-skew/leeway policy, `iss`/`aud`/`exp` validation rules | `apps/control-plane/src/auth.rs`; `crates/flexcore-domain/src/tenant_context.rs` |
| P13.03 | SCIM 2.0 provisioning design | user lifecycle (create/update/deactivate) design, group→role mapping rules onto the `PolicySet` role vocabulary, deprovisioning SLA and its audit evidence | [S-77]; `crates/flexcore-authz/src/policy.rs` |
| P13.04 | MFA/step-up enforcement | server-side step-up challenge design gating the APPROVED gate (P4.13) and high-impact console actions; replaces the UI-only `stepUpApproval.tsx` flow with an enforced protocol; closes LR-048 | LR-048; `console/src/components/stepUpApproval.tsx`; `crates/flexcore-application/src/command_gates.rs` |
| P13.05 | Role model refinement | portfolio/site-scoped permissions (ABAC-lite) layered on the typed scope vocabulary — new scoped actions extend `Action` additively; finer-grained approver roles separate administrative rights from approval duty; closes the LR-049 direction | LR-049; `crates/flexcore-authz/src/scope.rs`; `phase4_policies.rs` |
| P13.06 | Multi-step approval workflow engine | sequential approvals, delegation, absence policy, quorum for high-impact commands — extends the P4.13 authorization gate from outside WITHOUT touching the frozen 14-state/27-edge transition set; approval records stay `record_command_approval`-shaped (§2.2) | `crates/flexcore-application/src/command_gates.rs`; `crates/flexcore-domain/src/command_state.rs` |
| P13.07 | Machine identity | OAuth2 client-credentials JWT issuance/validation design for external integrations — grid-operator systems and the Phase 16 market adapters; per-client registration, scope subsets, revocation | [S-76][S-77]; gap analysis §9 (P16 consumes P13 identity) |
| P13.08 | API-key issuance/rotation service | legacy/simple tier: per-tenant API keys with rotation and last-used evidence, scopes a strict subset of the typed vocabulary, HMAC webhook secrets bound per key; storage follows the P11.16 secrets posture | P11.16 (forward seam); `crates/flexcore-authz/src/action.rs` |
| P13.09 | Outbound webhook service | Stripe-grade signing: HMAC-SHA256 over `timestamp.rawbody`, 5-minute tolerance, exponential-backoff retries with bookkeeping in Postgres, dead-letter table plus depth metric, secret rolling with overlap | [S-83] |
| P13.10 | API versioning + deprecation policy | `/api/v1/` confirmed as the current version; `Deprecation` + `Sunset` headers design; 6–12 month deprecation windows; OpenAPI lint and breaking-change detection plan over the generated spec | [S-84][S-85]; `apps/control-plane/src/openapi.rs` |
| P13.11 | Notification pipeline | email/SMS/webhook channels, template registry, per-tenant preferences, event→notification mapping — alarm/audit-driven only; channel providers are P13-E-2; no channel is ever a dispatch path | [S-83]; P13-E-2 |
| P13.12 | Audit export service | signed export packs over `flexcore-audit` records, SIEM-friendly JSONL format, date-range plus tenant scoping, integrity statement inheriting the Phase 11 audit-chain upgrade | `crates/flexcore-audit/src/`; Phase 11 audit chain (forward seam) |
| P13.13 | GDPR/privacy engineering | data inventory across the workspace, per-table retention matrix over the P1.10 seven data classes (per jurisdiction — no universal default in code), DSAR export job design, erasure strategy including backups (documented re-materialization caveat), event streams (bounded retention plus erasure events), ML caches (cryptographic erasure) | [S-86][S-87][S-88]; `crates/flexcore-storage/src/retention.rs` |
| P13.14 | Jurisdiction registry | per-tenant residency and legal-basis capture design closing the LR-062 direction; retention basis becomes jurisdiction-qualified data; legal sign-off = human gate | LR-062; `ledger/phase-09-pilot-qualification/10_data_residency_retention.md` §13 |
| P13.15 | Tenant lifecycle operations | onboarding checklist, offboarding/decommission procedure, tenant data export and reconciliation design under RLS | `crates/adapters/postgres/` RLS posture [S-70] |
| P13.16 | Session management | timeout policy, concurrent-session policy, active-session list design, revocation semantics — a revoked session denies at the resolver, not at the UI | `apps/control-plane/src/auth.rs` |
| P13.17 | Console auth hardening | real login flows against the IdP (redirect/callback design), step-up UX wired to the P13.04 enforced protocol, session banner with expiry display | `console/src/components/stepUpApproval.tsx`; P13.02/P13.04 |
| P13.18 | Admin workspace completion | users/roles/API-keys/webhooks/policy management UI designs on the Administration scaffold — no write-path fabrication; every management action names the REST endpoint it would call, specified per report §20.1 item 12 | `console/src/workspaces/adminData.ts`; report §20.1 |
| P13.19 | Identity test suite (written-unrun) | authz matrix extension tests (new scoped actions/roles), SCIM conformance scenarios, step-up negative tests (expired/replayed/failed challenges), webhook signature/tolerance/replay tests, idempotent-handler tests for delivery bookkeeping | conventions §1 item 3 |
| P13.20 | IdP integration lab environment spec | Keycloak compose profile specification (written, not run; no image pulls claimed), realm/client/claims configuration as checked-in config drafts | [S-77]; P13-E-1 DEFAULT |
| P13.21 | Notification/audit interplay | closed, registered vocabulary of which lifecycle transitions and alarms emit notifications vs audit-only; the mapping is registered, versioned data, not scattered logic | `crates/flexcore-events/src/` frozen 26-event vocabulary; P13.11 |
| P13.22 | Decision records + close | P13-E-1 (IdP), P13-E-2 (notification channel providers), P13-E-3 (MFA mechanism) register entries, per-batch decision records, cross-subtask consistency audit | conventions §2, §4 |

### 2.1 Ordering and in-wave dependencies

| Dependency | Direction | Note |
|---|---|---|
| P13.01 → P13.02, P13.20 | wave 1 (A), internal | the OIDC design and the lab spec execute the P13-E-1 DEFAULT (Keycloak); if P13-E-1 is still OPEN, they implement the DEFAULT and record the dependence (conventions §4) |
| P13.02 → P13.17 | wave 1 (A) → wave 2 | console login consumes the P13.02 middleware design |
| P13.04 → P13.17 | wave 1 (A) → wave 2 | the step-up UX consumes the P13.04 enforced protocol |
| P13.05 → P13.18 | wave 1 (A) → wave 2 | the admin role/policy UIs render the P13.05 role model |
| P13.08 → P13.09 | wave 1 (A) → wave 1 (B) | the webhook secret FORMAT is defined once in P13.08 and consumed by the P13.09 signer — never redefined (gap analysis §9 rule) |
| P13.09 → P13.18 | wave 1 (B) → wave 2 | the webhook management UI renders the P13.09 endpoint/secret model |
| P13.11 → P13.21 | wave 1 (B) → wave 2 | the interplay vocabulary registers against the P13.11 mapping table |
| P13.14 → P13.13 | wave 1 (B), internal | the jurisdiction registry defines the key; the retention matrix fills per-jurisdiction values against it |
| P13.12, P13.16, P13.19 | wave 2 | the test suite asserts the wave-1 designs; export and session design are self-contained |

### 2.2 The approval-gate extension rule (P13.06 detail)

| Rule | Meaning |
|---|---|
| No new states, no new edges | the workflow engine operates BETWEEN FEASIBLE and APPROVED: multiple `record_command_approval`-shaped records accumulate against one command; the transition fires when the policy (sequence/quorum) is satisfied |
| Distinct gates preserved | validation, feasibility, and authorization remain three distinct gates; a multi-step approval is still exactly one authorization gate with multiple human inputs |
| Step-up composition | each human input may require a P13.04 step-up challenge; a failed challenge is a recorded denial, never a partial approval |
| Delegation is scoped | a delegated approval carries the delegator's scope subset, is time-bounded, and is itself audited |

### 2.3 The webhook delivery contract (P13.09 detail)

| Element | Contract |
|---|---|
| Signature | `HMAC-SHA256(secret, timestamp + "." + rawbody)` — signed over the RAW request body, never a re-serialized form [S-83] |
| Tolerance | receivers reject deliveries whose timestamp is older than 5 minutes — the replay window [S-83] |
| Retries | exponential backoff with a bounded attempt count; every attempt is booked in Postgres before it is made, under a delivery idempotency key |
| Dead-letter | deliveries that exhaust retries land in a dead-letter table; its depth is a metric with a registered alert threshold; nothing is silently dropped |
| Secret rolling | the new secret is issued with an overlap window during which both secrets verify; the old secret is revoked only after the overlap closes |
| Handler idempotency | receivers may observe duplicates; handlers must be idempotent — asserted by the P13.19 idempotent-handler tests |

## 3. Wave plan (2 agents)

| Wave | Agent A | Agent B | Note |
|---|---|---|---|
| 1 | P13.01–P13.08 (identity core) | P13.09–P13.14 (integration + privacy) | parallel; disjoint ownership — Agent A owns the `auth.rs`/authz seams, Agent B owns webhook/notification/privacy artifacts; the P13.08→P13.09 secret-format seam is defined once and cited |
| 2 | P13.15–P13.21 (console + ops) | P13.22 (close) | the close consumes both wave-1 outputs; the consistency audit runs last |

## 4. Decision points (E-decisions — agents never close them)

| ID | Question | Options | DEFAULT / RECOMMENDATION | Closer |
|---|---|---|---|---|
| P13-E-1 | Identity provider: Keycloak DEFAULT vs a commercial IdP (Okta/Entra/Auth0 class)? | self-hosted Keycloak / commercial IdP / both (Keycloak lab, commercial per-tenant) | DEFAULT (RECOMMENDATION): OIDC-first with Keycloak as the reference IdP and lab target [S-77]; commercial IdPs remain supported as per-tenant SSO connections; budget is the human owner's call | human owner |
| P13-E-2 | Notification channel providers (email/SMS vendor selection)? | self-hosted SMTP only / commercial email+SMS providers / email-only first | DEFAULT (RECOMMENDATION): email-first with provider-abstracted ports (adapters depend on ports); SMS and webhook channels designed but vendor selection deferred; no channel is a dispatch path | human owner |
| P13-E-3 | MFA mechanism for the enforced step-up (P13.04)? | TOTP / WebAuthn / passkeys / IdP-delegated MFA | DEFAULT (RECOMMENDATION): IdP-delegated MFA as primary (the IdP owns the authenticator), TOTP as the documented fallback; WebAuthn/passkeys noted as the forward path | human owner |

## 5. Cross-phase seams

| Direction | Seam | Discipline |
|---|---|---|
| Consumes from Phase 11 | audit-chain upgrade (LR-203 closure) — P13.12 exports inherit it; the P11.16 secrets posture — webhook/IdP secrets land there | defined once in the earlier phase, consumed here — never redefined (gap analysis §9) |
| Provides to Phase 16 | machine identity (P13.07) for market adapters; the API versioning/deprecation policy (P13.10) for the market API surface | Phase 16 consumes the records, not new copies |
| Provides to Phase 19 | audit export packs, privacy records, jurisdiction registry — the evidence the SOC 2 track audits | collected by Phase 19, never re-derived |
| Provides to Phase 18 | service-account patterns stay distinct from device identity; edge enrollment is NOT re-decided here | edge identity remains Phase 18's |
| ADR-010 boundary | every Phase 13 capability (identity, notification, webhook) adds no dispatch authority | the never-granted permission set stays byte-identical |

## 6. Risks

| # | Risk | Handling |
|---|---|---|
| 1 | Identity is cross-cutting: `apps/control-plane/src/auth.rs` is a shared seam every route touches | composition-root ownership rules: Agent A owns the identity path in wave 1; wave-2 console work consumes the design, never edits the middleware; the dependency rule stays the arbiter |
| 2 | Webhook signing secrets and IdP client secrets need storage that does not exist yet | the P11.16 secrets posture (Vault DEFAULT, Compose file-per-host floor) is the landing zone; P13.08 designs formats, never invents storage |
| 3 | Step-up enforcement could deadlock the approval gate (an unreachable APPROVED) | P13.04/P13.06 failure semantics: a failed or missing challenge is a recorded denial leaving the command FEASIBLE; negative tests in P13.19 assert no wedge |
| 4 | SCIM needs an IdP to test against — none is deployed | SCIM work is design plus written-unrun conformance scenarios (P13.03, P13.19), lab-gated on P13.20; no conformance claim |
| 5 | LR-062 is D1-OPEN: a privacy regime without a jurisdiction is a placeholder | P13.14 captures residency/legal-basis as data with legal sign-off as a human gate; retention values stay per-jurisdiction RECOMMENDATIONS, never universal defaults (§16.3 rule inherited) |
| 6 | Admin UI write paths could fabricate capabilities the API does not have | P13.18 names the REST endpoint per report §20.1 for every management action; where an endpoint does not exist, the UI design records it as specified-not-implemented |
| 7 | Notifications creeping toward control | P13.21's closed vocabulary and the never-a-dispatch-path rule (context §3 item 5); channel adapters get scope subsets, never command reach |

## 7. Deliverables layout and verification

- This planning step produces exactly three files: `context.md`, `plan.md`,
  `ledger_phase_13.md` (conventions §8). The `records/` subdirectory is
  created at implementation time; implementation-time artifacts (design
  records, API specifications, config drafts, test files) land under the
  phase directory as the subtasks name them, never as new workspace crates
  during planning.
- New crates named in the plan (identity/webhook/notification adapters) are
  proposals: they become workspace members only during implementation, with
  real code, clear ownership, and a stable dependency direction, with the
  additive `check_dependency_rule.py` update the conventions require.
- Permitted verification only (conventions §1): `python
  tools/verify_docs.py`, `python tools/validate_agent.py`, `python -I
  tools/check_dependency_rule.py`; the doc checks run after any
  documentation change and a task is not complete while either reports a
  failure.

## 8. Acceptance criteria (honest)

1. All 22 subtasks have complete written artifacts; ledger ticks read
   "implemented, awaiting human build/test" with a decision-record citation —
   never "tested", "verified", "passed", or "works" (conventions §2).
2. No IdP is deployed, no OIDC token is validated, no SCIM object is
   provisioned, no notification is sent, no webhook is delivered, no MFA
   enrollment exists — every evidence cell is EMPTY/UNRUN; the lab
   environment (P13.20) is a written specification, not a running stack.
3. P13-E-1..E-3 remain OPEN with their DEFAULTs recorded, never closed by
   an agent; LR-047/048 close as designed (implemented, awaiting human
   build/test) and the LR-062 direction closes as data capture with legal
   sign-off still a human gate.
4. The 14-state/27-edge transition set and the never-granted permission set
   are byte-identical after Phase 13; `python -I
   tools/check_dependency_rule.py` stays green after every implementation
   batch.
5. Every capability claim traces to written code or design records; test
   files are written complete and left unrun (conventions §1 item 3); the
   first human build of the Phase 0–10 workspace still precedes
   implementation (`../README.md` hard boundary 1).
