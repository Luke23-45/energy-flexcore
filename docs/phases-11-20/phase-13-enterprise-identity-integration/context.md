# Phase 13 — Enterprise Identity, Administration and Integration Platform — Context

Date: 2026-11-14. Status: **NOT STARTED** — 0/22 subtasks (see
`ledger_phase_13.md`). This is a planning artifact: no IdP is deployed, no
identity provider is configured, no notification is sent, no webhook is
delivered. Citations `[S-n]` resolve in `../00_GAP_ANALYSIS.md` §11; the
rules in `../01_PROGRAM_CONVENTIONS.md` bind everything below.

## 1. Why this phase exists

The commercial research (Track C of the gap analysis) found capabilities no
enterprise VPP platform sells without: enterprise SSO and SCIM sit in the
Tier 2 enterprise-readiness baseline [S-77], and security attestations —
SOC 2 Type II minimum — are table stakes enterprise tenants demand
[S-45][S-44] (gap analysis §3 rows 8 and 10). The gap analysis assigns the
SSO/SCIM/MFA/webhook/API-versioning cluster to this phase (§5 Tier 2) and
the attestation itself to Phase 19 — Phase 13 builds the identity, audit
export, and privacy foundation that attestation will audit.

The internal inventory (Track A) found the concrete gaps:

| Gap | Register / evidence |
|---|---|
| No IdP integration — identity enforcement is future composition | LR-047 (`ledger/phase-09-pilot-qualification/27_limitations_residual_risk.md` §3.8) |
| No step-up/MFA enforcement mechanism in code | LR-048 (same register §3.8) |
| Approver shares TenantAdmin with administrative rights; finer roles need a reviewed change | LR-049 (same register §3.8) |
| No jurisdiction, no residency constraint, no privacy regime, no retention basis (D1 OPEN) | LR-062 (`ledger/phase-09-pilot-qualification/10_data_residency_retention.md` §13) |
| "sso" / "saml" / "scim" / "webhook" / "gdpr" / "pii": zero keyword hits | gap analysis §5 Tier 2 |

The engineering research (Track D) supplies the Tier 2 practices this phase
designs: machine identity via OAuth2 client-credentials JWT [S-76][S-77];
Stripe-grade outbound webhooks — HMAC signing, tolerance windows, retries,
dead-lettering, secret rolling [S-83]; API versioning with Deprecation and
Sunset headers (RFC 8594) [S-84]; and GDPR erasure engineering — data
inventory, DSAR, erasure including backups and derived stores [S-86][S-87].

## 2. What exists today (verified paths; all WRITTEN-UNRUN)

| Seam | Path | What Phase 13 builds on |
|---|---|---|
| Identity resolution | `apps/control-plane/src/auth.rs` | `IdentityResolver` trait; `DenyAllResolver` is the default (denies every request); `DevAuthResolver` compiles only behind the non-default `dev-auth` feature. LR-047 names this "future composition" — P13.02 composes it |
| Deny-by-default authz | `crates/flexcore-authz/src/` (`policy.rs`, `action.rs`, `scope.rs`, `phase4_policies.rs`) | allow-only `PolicySet`, typed `TenantScope`/`SiteScope`/`AssetScope`, closed `Action` vocabulary with a rejecting parse, the Phase 4 `command.*` grant set — a strength to build on, not a replacement |
| Authenticated claims | `crates/flexcore-domain/src/tenant_context.rs` | `AuthenticatedClaims` with the single public constructor `AuthenticatedClaims::from_verified` over `VerifiedClaims`; `TenantContext` derives only from authenticated claims — the mapping target for OIDC/SCIM claims |
| UI-only step-up | `console/src/components/stepUpApproval.tsx` | P5.26 typed confirmation plus fresh re-validation on the DEV-SESSION path; the honesty record states NO server-verified step-up exists and none was invented — the object P13.04 replaces with an enforced protocol |
| Administration workspace | `console/src/workspaces/adminData.ts` (+ `Administration.tsx`) | P5.07 read-only data mapping (report §20.1 item 12) — the scaffold P13.18 completes |
| Human approval gate | `crates/flexcore-application/src/command_gates.rs` | P4.13 authorization gate: FEASIBLE → APPROVED requires a recorded human/policy authorization (`record_command_approval`, `ApprovalScope` Tenant/Site/Asset); no auto-approve path — the seam P13.06 extends |
| Command lifecycle | `crates/flexcore-domain/src/command_state.rs` | the frozen 14-state / 27-edge transition set (report §14.1) — extended by P13.06 only from outside, never edited |
| Audit records | `crates/flexcore-audit/src/` (`record.rs`, `sink.rs`) | append-only audit records and sinks — the source P13.12's signed export packs read |
| Retention policy types | `crates/flexcore-storage/src/retention.rs` | P1.10 `RetentionPolicy`: the seven §16.3 data classes, defaults UNSET everywhere, no universal retention period in code — the schema P13.13's per-table retention matrix populates per jurisdiction |
| OpenAPI surface | `apps/control-plane/src/openapi.rs` | utoipa generation serving `/api/v1/openapi.json` — the artifact P13.10's versioning and deprecation policy governs |
| Frozen event vocabulary | `crates/flexcore-events/src/` | exactly 26 events (report §15.3) with mandatory idempotency keys — the closed source set P13.21's notification mapping draws from |
| Postgres RLS | `crates/adapters/postgres/` (RLS ENABLE + FORCE, migration 0008) | tenant isolation P13.15's tenant-lifecycle export/reconciliation operates under |
| Secrets posture | Phase 11 plan P11.16 (forward seam) | Vault DEFAULT recommendation / Compose file-per-host floor — where webhook signing secrets and IdP client secrets must land |

## 3. Constraints

1. **The program bans (conventions §1) apply in full.** No IdP image is
   pulled, no OIDC broker is stood up, no notification provider account is
   created, no SCIM client is exercised; every artifact is a design record,
   API specification, config draft, or test file written complete and left
   for the human build/test gate (P13.19, P13.20).
2. **The never-granted permission set stays byte-identical** (conventions
   §6.4): `dispatch.authorize`, `dispatch.issue`, `secret.read`,
   `network.any` are never granted to any principal — human, service
   account, API key, or machine identity. Adapters reach dispatch only
   through the command lifecycle.
3. **The 14-state / 27-edge lifecycle is frozen.** The multi-step approval
   workflow (P13.06) extends the P4.13 authorization gate from outside —
   sequential approvals, delegation, quorum — without adding states or
   edges. Validation, feasibility, and authorization remain three distinct
   gates; acknowledgement is not physical delivery (report §14.1).
4. **Step-up must never deadlock the approval gate.** The enforced step-up
   protocol (P13.04) gates APPROVED and high-impact console actions; its
   failure modes are recorded denials, not wedged commands — a failed
   challenge leaves the command FEASIBLE and unapproved, exactly as the
   current no-step-up world does.
5. **Notifications are never a dispatch path.** The notification pipeline
   (P13.11) is alarm/audit-driven output over a closed registered
   vocabulary (P13.21); no command is issued, altered, or advanced through
   a notification, and no channel provider gains control authority
   (P13-E-2 selects vendors, not powers).
6. **The dependency rule holds** (conventions §6.1): new crates named in
   the plan (identity, webhook, notification adapters) are proposals that
   become workspace members only at implementation, with real code, clear
   ownership, and ports-first dependency direction; adapters depend on
   ports, never sideways on each other.
7. **Honesty rules (conventions §7):** no capability claim about unrun
   code; external claims cite `[S-n]` or are marked UNVERIFIED/ASSUMPTION;
   LR-062's D1 stays OPEN — P13.14 captures per-tenant residency and legal
   basis as data, and legal sign-off is a human gate, never an agent act.

## 4. Glossary (as the terms are used in this phase)

| Term | Meaning here |
|---|---|
| OIDC | OpenID Connect — identity layer over OAuth 2.0; the DEFAULT first federation protocol (P13-E-1) [S-77] |
| SAML | Security Assertion Markup Language — the legacy enterprise federation protocol; supported via an OIDC broker, not natively (P13.01) [S-77] |
| SCIM | System for Cross-domain Identity Management — the user/group provisioning protocol whose 2.0 schema P13.03 designs against |
| Step-up authentication | A fresh, server-verified re-authentication (with MFA) required before a high-impact act; the enforced form replaces the UI-only P5.26 flow (LR-048) |
| Client-credentials flow | OAuth 2.0 grant where a machine principal obtains a JWT access token directly — the machine-identity mechanism (P13.07) [S-76][S-77] |
| HMAC webhook signing | Signature over `timestamp.rawbody` with a per-endpoint secret; receivers reject signatures outside a tolerance window; exhausted deliveries dead-letter (P13.09) [S-83] |
| DLQ (dead-letter table) | The bounded store for deliveries that exhausted retries; its depth is a metric, never a silent loss (P13.09) |
| Deprecation / Sunset headers | HTTP response headers announcing an API version's deprecation date and planned retirement; Sunset is RFC 8594 (P13.10) [S-84] |
| DSAR | Data Subject Access Request — the GDPR access/export duty P13.13 implements as a designed job, not a promise [S-86] |
| Cryptographic erasure | Destroying the key material that decrypts derived or cached personal data where rows cannot be reached — the ML-cache erasure strategy (P13.13) [S-87] |
| Retention matrix | Per-table retention assignments over the P1.10 seven data classes, populated per jurisdiction — never a universal default in code (LR-062 direction) |
| Delegation | Time-bounded re-assignment of an approver's approval duty during absence (P13.06) |
| Service account | A named non-human principal with typed scopes and its own audit identity — never a shared admin login (P13.07/P13.08) |
