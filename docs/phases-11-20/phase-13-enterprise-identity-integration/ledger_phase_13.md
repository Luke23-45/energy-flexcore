# Phase 13 Ledger — Enterprise Identity, Administration and Integration Platform

Date: 2026-11-14. Status: **IMPLEMENTED — awaiting human build/test** — 22/22 subtasks ticked. Phase close: HANDOVER.md, CHECKLIST.md, records/61_p13_22_phase_close.md. No IdP run, token validated, or delivery made; the human-gate queue is in CHECKLIST.md.

Gate: this ledger moves only after the human gate on this roadmap AND the
first human build of the Phase 0–10 workspace are green (`../README.md`
hard boundary 1; `../01_PROGRAM_CONVENTIONS.md` §3). Until then every line
stays unticked and nothing in the phase claims implementation, run, or
verification status.

Tick convention (carried verbatim from the conventions): a checkbox is
ticked ONLY as "implemented, awaiting human build/test" with a
decision-record citation — never as "tested", "verified", "passed", or
"works". Tick flips are applied by `python -I` heredoc with exact-match
count==1 assertions, or by the Write/Edit tools for whole files. Decision
records go under `phase-13-enterprise-identity-integration/records/`,
created at implementation time, not during planning.

## Subtask register

- [x] **P13.01 — Identity architecture decision record.** (decision record: `records/01_p13_01_identity_architecture.md`)
  Scope: OIDC-first with SAML supported via an OIDC broker; Keycloak DEFAULT recommendation vs a commercial IdP; per-tenant SSO connections as tenant data, not platform config; recorded as P13-E-1.
  Evidence: decision record citing [S-77] and LR-047; the E-decision stays OPEN.
- [x] **P13.02 — OIDC integration design.** (decision record: `records/02_p13_02_oidc_token_validation.md`)
  Scope: token validation middleware replacing `DenyAllResolver` as the PRODUCTION identity path (dev-auth stays non-default and feature-gated); claims → `VerifiedClaims` → `AuthenticatedClaims::from_verified` mapping; JWKS caching, clock-skew/leeway policy, `iss`/`aud`/`exp` validation rules.
  Evidence: design record tracing `apps/control-plane/src/auth.rs` and `crates/flexcore-domain/src/tenant_context.rs`; no token is validated.
- [x] **P13.03 — SCIM 2.0 provisioning design.** (decision record: `records/03_p13_03_scim_provisioning.md`)
  Scope: user lifecycle (create/update/deactivate), group→role mapping onto the `PolicySet` role vocabulary, deprovisioning SLA with audit evidence.
  Evidence: design record citing [S-77]; lab-gated on P13.20, no conformance claim.
- [x] **P13.04 — MFA/step-up enforcement.** (decision record: `records/04_p13_04_step_up_mfa.md`)
  Scope: server-side step-up challenge gating the APPROVED gate (P4.13) and high-impact console actions; replaces the UI-only `stepUpApproval.tsx` flow with an enforced protocol; failure modes are recorded denials, never a deadlocked approval gate.
  Evidence: design record citing LR-048, `console/src/components/stepUpApproval.tsx`, and `crates/flexcore-application/src/command_gates.rs`; closes LR-048 as designed.
- [x] **P13.05 — Role model refinement.** (decision record: `records/05_p13_05_role_model.md`)
  Scope: portfolio/site-scoped permissions (ABAC-lite) layered on the typed scope vocabulary; new scoped actions extend `Action` additively; finer approver roles separate administrative rights from approval duty.
  Evidence: design record citing LR-049 and `crates/flexcore-authz/src/scope.rs`; the never-granted permission set is untouched.
- [x] **P13.06 — Multi-step approval workflow engine.** (decision record: `records/06_p13_06_approval_workflow_engine.md`)
  Scope: sequential approvals, delegation, absence policy, quorum for high-impact commands — extends the P4.13 authorization gate from outside WITHOUT touching the frozen 14-state/27-edge transition set; approval records stay `record_command_approval`-shaped.
  Evidence: design record tracing `command_gates.rs` and `command_state.rs`; the transition table remains byte-identical.
- [x] **P13.07 — Machine identity.** (decision record: `records/07_p13_07_machine_identity.md`)
  Scope: OAuth2 client-credentials JWT issuance/validation design for external integrations — grid-operator systems and the Phase 16 market adapters; per-client registration, scope subsets, revocation.
  Evidence: design record citing [S-76][S-77] and the gap analysis §9 P16 seam.
- [x] **P13.08 — API-key issuance/rotation service.** (decision record: `records/08_p13_08_api_keys_webhook_secret_format.md`)
  Scope: legacy/simple tier — per-tenant API keys with rotation and last-used evidence, scopes a strict subset of the typed vocabulary, HMAC webhook secrets bound per key; storage follows the P11.16 secrets posture.
  Evidence: design record citing P11.16 (forward seam) and `crates/flexcore-authz/src/action.rs`; no key is issued.
- [x] **P13.09 — Outbound webhook service.** (decision record: `records/21_p13_09_webhook_service.md`)
  Scope: Stripe-grade signing — HMAC-SHA256 over `timestamp.rawbody`, 5-minute tolerance, exponential-backoff retries with Postgres bookkeeping, dead-letter table plus depth metric, secret rolling with overlap.
  Evidence: design record citing [S-83]; no delivery is performed.
- [x] **P13.10 — API versioning + deprecation policy.** (decision record: `records/22_p13_10_api_versioning_deprecation.md`)
  Scope: `/api/v1/` confirmed as the current version; `Deprecation` + `Sunset` headers design; 6–12 month deprecation windows; OpenAPI lint and breaking-change detection plan over the generated spec.
  Evidence: policy record citing [S-84][S-85] and `apps/control-plane/src/openapi.rs`.
- [x] **P13.11 — Notification pipeline.** (decision record: `records/23_p13_11_notification_pipeline.md`)
  Scope: email/SMS/webhook channels, template registry, per-tenant preferences, event→notification mapping — alarm/audit-driven only; channel providers are P13-E-2; no channel is ever a dispatch path.
  Evidence: design record; no channel is configured, no message is sent.
- [x] **P13.12 — Audit export service.** (decision record: `records/24_p13_12_audit_export.md`)
  Scope: signed export packs over `flexcore-audit` records, SIEM-friendly JSONL format, date-range plus tenant scoping, integrity statement inheriting the Phase 11 audit-chain upgrade.
  Evidence: design record tracing `crates/flexcore-audit/src/`; no export is produced.
- [x] **P13.13 — GDPR/privacy engineering.** (decision record: `records/25_p13_13_gdpr_privacy.md`)
  Scope: data inventory across the workspace, per-table retention matrix over the P1.10 seven data classes (per jurisdiction — no universal default in code), DSAR export job design, erasure strategy including backups (documented re-materialization caveat), event streams (bounded retention plus erasure events), ML caches (cryptographic erasure).
  Evidence: engineering record citing [S-86][S-87][S-88] and `crates/flexcore-storage/src/retention.rs`.
- [x] **P13.14 — Jurisdiction registry.** (decision record: `records/26_p13_14_jurisdiction_registry.md`)
  Scope: per-tenant residency and legal-basis capture design closing the LR-062 direction; retention basis becomes jurisdiction-qualified data; legal sign-off is a human gate.
  Evidence: design record citing LR-062 and `ledger/phase-09-pilot-qualification/10_data_residency_retention.md` §13; the sign-off cell stays EMPTY.
- [x] **P13.15 — Tenant lifecycle operations.** (decision record: `records/41_p13_15_tenant_lifecycle.md`)
  Scope: onboarding checklist, offboarding/decommission procedure, tenant data export and reconciliation design under RLS.
  Evidence: procedure record tracing the `crates/adapters/postgres/` RLS posture [S-70]; no tenant is operated on.
- [x] **P13.16 — Session management.** (decision record: `records/42_p13_16_session_management.md`)
  Scope: timeout policy, concurrent-session policy, active-session list design, revocation semantics — a revoked session denies at the resolver, not at the UI.
  Evidence: design record tracing `apps/control-plane/src/auth.rs`.
- [x] **P13.17 — Console auth hardening.** (decision record: `records/43_p13_17_console_auth_hardening.md`)
  Scope: real login flows against the IdP (redirect/callback design), step-up UX wired to the P13.04 enforced protocol, session banner with expiry display.
  Evidence: design record consuming the P13.02/P13.04 outputs; no login is performed.
- [x] **P13.18 — Admin workspace completion.** (decision record: `records/44_p13_18_admin_workspace_completion.md`)
  Scope: users/roles/API-keys/webhooks/policy management UI designs on the Administration scaffold — no write-path fabrication; every management action names the REST endpoint it would call, specified per report §20.1 item 12.
  Evidence: design record tracing `console/src/workspaces/adminData.ts`; endpoints are specified-not-implemented where absent.
- [x] **P13.19 — Identity test suite (written-unrun).** (decision record: `records/45_p13_19_identity_test_suite.md`)
  Scope: authz matrix extension tests (new scoped actions/roles), SCIM conformance scenarios, step-up negative tests (expired/replayed/failed challenges), webhook signature/tolerance/replay tests, idempotent-handler tests for delivery bookkeeping.
  Evidence: complete test files written-unrun per conventions §1 item 3; execution is a human gate.
- [x] **P13.20 — IdP integration lab environment spec.** (decision record: `records/46_p13_20_idp_lab_spec.md`)
  Scope: Keycloak compose profile specification (written, not run; no image pulls claimed), realm/client/claims configuration as checked-in config drafts.
  Evidence: spec citing [S-77] and the P13-E-1 DEFAULT; nothing is deployed.
- [x] **P13.21 — Notification/audit interplay.** (decision record: `records/47_p13_21_notification_audit_interplay.md`)
  Scope: closed, registered vocabulary of which lifecycle transitions and alarms emit notifications vs audit-only; the mapping is registered, versioned data, not scattered logic.
  Evidence: vocabulary record against the frozen 26-event set in `crates/flexcore-events/src/` and the P13.11 mapping table.
- [x] **P13.22 — Decision records + close.** (decision record: `records/61_p13_22_phase_close.md`)
  Scope: P13-E-1 (IdP), P13-E-2 (notification channel providers), P13-E-3 (MFA mechanism: TOTP/WebAuthn/passkeys) register entries, per-batch decision records, cross-subtask consistency audit.
  Evidence: register entries plus audit record; E-decisions stay OPEN, human-closed.

## Decision points (OPEN — agents never close them)

| ID | Question | Status | DEFAULT recorded |
|---|---|---|---|
| P13-E-1 | Identity provider: Keycloak DEFAULT vs a commercial IdP | OPEN | OIDC-first, Keycloak as reference IdP and lab target [S-77]; commercial IdPs supported as per-tenant SSO connections; budget = human owner |
| P13-E-2 | Notification channel providers (email/SMS vendors) | OPEN | email-first with provider-abstracted ports; SMS/webhook designed, vendor selection deferred; no channel is a dispatch path |
| P13-E-3 | MFA mechanism for enforced step-up (TOTP/WebAuthn/passkeys/IdP-delegated) | OPEN | IdP-delegated MFA primary, TOTP documented fallback; WebAuthn/passkeys noted as the forward path |

## Wave notes

| Wave | Subtasks | Note |
|---|---|---|
| 1 | P13.01–P13.08 (Agent A, identity core); P13.09–P13.14 (Agent B, integration + privacy) | parallel; disjoint ownership; the P13.08→P13.09 secret format is defined once and cited |
| 2 | P13.15–P13.21 (Agent A, console + ops); P13.22 (Agent B, close) | the close consumes both wave-1 outputs; the consistency audit runs last |

## Carried-gap closures this register tracks

| Gap | Register ID | Closing subtasks |
|---|---|---|
| No IdP integration | LR-047 | P13.01, P13.02, P13.20 |
| No step-up/MFA enforcement | LR-048 | P13.04, P13.17 |
| Approver shares TenantAdmin | LR-049 | P13.05 (direction; finer roles) |
| No jurisdiction/privacy regime/retention basis | LR-062 | P13.13, P13.14 (direction; legal sign-off human-gated) |
| No machine identity | gap analysis §5 Tier 2 [S-76][S-77] | P13.07 |
| No outbound webhooks | gap analysis §5 Tier 2 [S-83] | P13.09 |
| No API versioning/deprecation policy | gap analysis §5 Tier 2 [S-84] | P13.10 |

Status: **IMPLEMENTED (22/22).** Ticks were applied only through the
conventions-disciplined exact-match process; wave assignment is in
`plan.md` §3. The first human `cargo build --workspace` remains the program gate.
