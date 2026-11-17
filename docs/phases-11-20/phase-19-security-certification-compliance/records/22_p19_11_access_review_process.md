<!-- P19.11 — Access-review process. HONEST RECORD: a PROCESS record with
     an EMPTY evidence template. No access review has ever run; the IdP
     DESIGN landed in phase 13 (P13.01) but the runtime IdP is
     UNDEPLOYED, so every mechanics cell is marked PENDING-P13-RUNTIME.
     The quarterly cadence is registered (X-8) BEFORE any review run.
     No attestation, certification, or audit outcome is claimed. -->
# P19.11 — Access-review process: quarterly cadence, IdP-dependent mechanics, EMPTY evidence template, findings path (revoke / accept-with-expiry)

| Field | Value |
|---|---|
| Subtask | P19.11 — Access-review process (`ledger_phase_19.md` §P19.11) |
| Wave / agent | Phase 19 wave 1, Agent B |
| Status | **PROCESS RECORD — WRITTEN, NEVER RUN.** No access review has been performed anywhere in this program; every evidence cell in the template (§5) is `<EMPTY>` and stays `<EMPTY>` until a real review fills it. The runtime IdP is UNDEPLOYED — every mechanics cell (§4) is marked PENDING-P13-RUNTIME. |
| Sources | P13.01 identity architecture (`docs/phases-11-20/phase-13-enterprise-identity-integration/records/01_p13_01_identity_architecture.md` — cited, never restated); P13.03 SCIM provisioning (`.../records/03_p13_03_scim_provisioning.md`); P13.05 role model (`.../records/05_p13_05_role_model.md`); P13.07 machine identity (`.../records/07_p13_07_machine_identity.md`); P13.15 tenant lifecycle (`.../records/41_p13_15_tenant_lifecycle.md`); P14.16/P16.19 grant records (adapter and market grants enter review scope); conventions §5 (human gates, X-8); [S-80] (access review as SOC 2 evidence practice — process framing only) |
| Not-a-certification | Per `ledger/phase-10-v1-release-hardening/25_security_consolidation.md` §7 (verbatim discipline, AGENTS.md §3 rules 2–3): FlexCore is not certified, not a safety case, and not permission for live grid control. This process document is not a certification, not an attestation, and not evidence that any review has operated. |

## 1. Honest status — the plain statement

**No access review has ever run; no review evidence exists.** This
record delivers the standing process a certification process would
consume [S-80]: the cadence (§3, registered before any run), the review
scope (§3), the mechanics (§4 — IdP-dependent, PENDING-P13-RUNTIME), the
EMPTY evidence record template (§5), and the findings path (§6). The
identity DESIGN landed in phase 13 and is cited at its record path; the
RUNTIME IdP is UNDEPLOYED — no IdP is reachable from any code path in
this repo, no tenant SSO connection exists, and `DenyAllResolver`
remains the shipped default resolver (P13.01 §3, carried verbatim in
substance). Nothing here deploys, configures, or connects an IdP.

## 2. Dependence structure (consumed, never re-decided)

| Input | Owning record | Status carried into this process |
|---|---|---|
| Identity architecture (OIDC-first; per-tenant SSO connections as tenant data) | P13.01 record (path in header) | DESIGN landed; **P13-E-1 OPEN** (human owner closes); runtime IdP **UNDEPLOYED** |
| Role vocabulary (what "who holds what access" means) | P13.05 record | WRITTEN-UNRUN; the review reads roles as defined there, never redefines them |
| SCIM provisioning (joiner/mover/leaver feed) | P13.03 record | WRITTEN-UNRUN; UNDEPLOYED — the automated principal feed the review would consume does not operate |
| Machine principals and API keys | P13.07 / P13.08 records | WRITTEN-UNRUN; in review scope as principal classes, not afterthoughts |
| Adapter and market grants | P14.16 (`crates/flexcore-kernel/src/adapter_permissions.rs` design) / P16.19 (`crates/flexcore-authz/src/phase16_market_policies.rs` RECOMMENDED-DEFAULT table) | WRITTEN-UNRUN; grant tables enter review scope (§3) |
| Audit evidence of review actions | P11.15 keyed chain / P13.12 audit export | WRITTEN-UNRUN; review decisions are auditable events once the chain is keyed and operating |

## 3. Cadence and scope — registered (X-8) BEFORE any run

| Item | Registered value | Status |
|---|---|---|
| Review cadence | **Quarterly** (a review covering all principals at least once per calendar quarter) | REGISTERED — before any run that could fill evidence cells (X-8; conventions §5) |
| Review trigger — ad hoc | Any of: a tenant offboarding (P13.15), a grant-table change (P14.16/P16.19), a role-model change (P13.05), a security-incident finding | REGISTERED |
| Scope — human principals | Every role assignment under the P13.05 vocabulary, per tenant, including SiteApprover separation (no market grant anywhere — P16.19 posture) | REGISTERED |
| Scope — machine principals | Every machine client (P13.07) and API key (P13.08), with owner and purpose recorded | REGISTERED |
| Scope — authorization grants | Adapter bindings (P14.16) and market grant records (P16.19), including the never-granted set REVIEW posture (reviewed, never altered by this process) | REGISTERED |
| Scope — exclusion | No dispatch authority is reviewable or grantable by this process: `dispatch.authorize`/`dispatch.issue` stay in the never-granted set, byte-identical | REGISTERED |

## 4. Mechanics — IdP-dependent, ALL cells PENDING-P13-RUNTIME

Every cell below describes what the quarterly review WILL do once the
IdP runtime exists; none of it operates today. The design citations are
P13.01 (SSO connections as tenant data; claims mapping resolved at the
P13.02 middleware) and P13.03 (SCIM as the joiner/mover/leaver feed).

| Step | What it does when the IdP is deployed | Status |
|---|---|---|
| M-1 Principal export | Export the principal list from the IdP per tenant SSO connection (via SCIM once deployed) — the export is the review's input of record, never a hand-typed list | PENDING-P13-RUNTIME (IdP UNDEPLOYED; SCIM UNDEPLOYED) |
| M-2 Grant reconciliation | Compare the export against granted roles (P13.05), machine clients (P13.07), API keys (P13.08), adapter bindings (P14.16), and market grants (P16.19) | PENDING-P13-RUNTIME |
| M-3 Tenant confirmation | Per-tenant admin confirmation of their tenant's access (the SSO connection is tenant data — P13.01) | PENDING-P13-RUNTIME |
| M-4 Findings classification | Each discrepancy classified: revoke (§6.1) or accept-with-expiry (§6.2) | REGISTERED (classification rule); instances PENDING-P13-RUNTIME |
| M-5 Evidence pack | Assemble the §5 template, filled from M-1–M-4 outputs; audited via the P11.15 chain once keyed and operating | PENDING-P13-RUNTIME |

## 5. Evidence record template — ALL CELLS EMPTY (human-gated inputs)

| Field | Value |
|---|---|
| Review period (quarter) | `<EMPTY>` |
| Reviewer (named person) | `<EMPTY>` |
| Approver (named person; separation from reviewer) | `<EMPTY>` |
| Principal export reference (IdP/SCIM export id + date) | `<EMPTY>` — export-based, never hand-typed |
| Human principals in scope (count per tenant) | `<EMPTY>` |
| Machine principals / API keys in scope (count) | `<EMPTY>` |
| Adapter bindings reviewed (count; reference to the binding set) | `<EMPTY>` |
| Market grant records reviewed (count) | `<EMPTY>` |
| Findings (each: principal, grant, discrepancy, classification) | `<EMPTY>` |
| Revocations executed (with audit-evidence references) | `<EMPTY>` |
| Acceptances with expiry (each: signed by whom, expires when — §6.2) | `<EMPTY>` |
| Next review due | `<EMPTY>` |

## 6. Findings path

### 6.1 Revoke

A finding classified as revoke is executed promptly and audited: the
revocation is an authorization-surface event recorded into the audit
chain (P11.15 design; P13.12 export makes it producer-consumable), the
session/credential is denied at the resolver (P13.16 posture: revocation
denies at the resolver). No revocation exists today — nothing to revoke
(no operating IdP, no grants issued).

### 6.2 Accept with expiry

A finding classified as accept is accepted ONLY with: a named human
acceptor, a signature, and a time-bounded expiry — **maximum accepted
expiry 90 days from acceptance** (REGISTERED before any run, X-8). An
expired acceptance is an OPEN finding in the next review, not an
entitlement. An acceptance without a signature or time bound is not an
acceptance (the P10.20 §7 discipline, carried). No acceptance exists
today.

## 7. Reviewer-verification procedure (human)

1. Confirm §5's cells are all `<EMPTY>` and §4's cells all say
   PENDING-P13-RUNTIME (or REGISTERED where a rule, not an instance, is
   recorded).
2. Confirm the quarterly cadence and the 90-day acceptance-expiry
   maximum are registered BEFORE any run (X-8).
3. Confirm the P13.01/P13-E-1 status is carried honestly: design landed,
   runtime IdP UNDEPLOYED, P13-E-1 still OPEN (human-owned).
4. Confirm no review, revocation, or acceptance is claimed.

## 8. Status

**WRITTEN / PROCESS-ONLY.** Cadence and scope registered; mechanics
honestly IdP-pending; evidence template EMPTY; findings path defined.
No access review has run and none is scheduled as run.
