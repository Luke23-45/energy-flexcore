<!-- P9.08 — Identity, access, and approval design. DESIGN RECORD only: roles mapped to the EXISTING flexcore-authz vocabulary by citation, approval chains per §14, least-privilege table, break-glass and qualification DESIGN. NO role is assigned to any person: every role-assignment/IdP/step-up execution cell is EMPTY. No negative test was executed. -->
# P9.08 — Identity, Access, and Approval Design (Design Record — no person, IdP, or test exists)

Date: 2026-11-14. Subtask: P9.08 (`ledger_phase_9.md` §P9.08).
Owner role: Security (with the customer IT/IdP owner). Report trace:
§23 Stage 9; §17.3; §20.2; §14 (authorization distinct from validation and
feasibility); §21.4 (operator-approved control). Inputs: P9.01 scope
(deployment `<EMPTY>`); customer identity provider details — **do not
exist**; P9.07 findings on identity controls (TEMPLATE —
`07_deployment_security_assessment.md` §10 C1–C3).

## 1. Honest status — the plain statement

**No pilot identity exists, no IdP is integrated, no role is assigned to
any person, and no negative test has been executed.** There are no pilot
users (no people exist in the design), no customer IdP details, and no
deployed authorization configuration to test. This record delivers the
DESIGN: the pilot role set mapped to the EXISTING `flexcore-authz`
vocabulary BY CITATION (§3), the approval chains that make operator-
approved control (§21.4) enforceable rather than customary (§4), the
least-privilege mapping (§5), the break-glass procedure DESIGN (§6), the
operator-qualification rule linked to P9.21 (§7), the IdP/step-up design
cells (§8), and the negative-test matrix the human execution must fill
(§9). Every role-assignment, IdP, and test-result cell is EMPTY. The
design does not modify any code; where the existing vocabulary lacks
something the pilot needs, that is a NAMED GAP with an owner and follow-up,
never silently assumed (§3.3).

## 2. Design basis — what already exists (cited, not re-designed)

| Existing surface | Where | What it provides |
|---|---|---|
| Closed `Role` enum: `PlatformAdmin`, `TenantAdmin`, `Operator`, `Viewer` (flat — no hierarchy, no implication; every (role, action, scope-kind) triple needs its own grant) | `crates/flexcore-authz/src/policy.rs` (P1.11) | The ONLY roles the authorization evaluator knows; a pilot role not mapped to one of these denies everything |
| Closed `Action` vocabulary (31 actions, Phase 1–5, incl. `command.create/validate/approve/read`, `audit.event.read`, `alarm.read`) | `crates/flexcore-authz/src/action.rs` | Unknown action ids deny with `DenyReason::UnknownAction` — deny-by-default for anything off the surface |
| Typed scopes `Tenant/Site/Asset` + tenant match (no role bypasses, not even `PlatformAdmin`) | `crates/flexcore-authz/src/scope.rs`; `policy.rs` `decide` steps 2–4 | Cross-tenant scope denies `ScopeDenied`; scope kinds do not inherit |
| Deny-by-default `PolicySet` + decision-carries-reason (`AuthorizationDecision` with reason + `EvidenceRef`, never a bare bool) | `policy.rs`; `decision.rs` | The authorization-evidence primitive §14.3 requires (§4.3 below) |
| Recorded Phase 4 command grant set (`command.create`/`validate` Operator+TenantAdmin; `command.approve` **TenantAdmin ONLY at site/asset scope**; `command.read` + TenantAdmin+Operator+Viewer) | `crates/flexcore-authz/src/phase4_policies.rs` (P4.13) | The separation-of-duties default this design adopts (§4.1) |
| Application-layer resolver seam: `IdentityResolver` with a `DenyAllResolver` fail-closed default (and a dev-only resolver) | `apps/control-plane/src/auth.rs` | The composition point where a real IdP resolver lands (§8); role resolution happens in the application layer from REAL authentication, never from a request field (P0.16 rule 5; `policy.rs` header) |
| Claims-only tenant context | `flexcore-domain` `TenantContext::from_authenticated_claims` (the only constructor) | An untrusted request field cannot become an authorization context — a compile-level fact, not a runtime check |

## 3. The pilot role matrix (DESIGN — mapped to the existing vocabulary)

### 3.1 Role mapping

Role-assignment cells (who) are EMPTY — no person exists. "Assigned
actions" are the grants the EXISTING policy builders give each mapped
`Role` (cited), not new grants invented here.

| Pilot role (design) | flexcore-authz `Role` (citation) | Assigned actions (from the recorded default grant sets) | Assignment (person/IdP claim) |
|---|---|---|---|
| Pilot operator (day-to-day operations; proposes commands, never approves) | `Role::Operator` (`policy.rs`) | `command.create`, `command.validate` at tenant/site/asset; `command.read`; operational reads per the phase 1–5 default sets (`apps/control-plane/src/auth.rs` `phase1…phase5_default_policies`); **NO `command.approve`** (P4.13 recorded default) | `<EMPTY>` |
| Approver (dispatch approval authority) | `Role::TenantAdmin` (`policy.rs`) | `command.approve` at **site and asset scope only** — no tenant-level approval grant exists in the recorded default (`phase4_policies.rs`); plus create/validate/read | `<EMPTY>` |
| Read-only observer | `Role::Viewer` (`policy.rs`) | `command.read`, `feasibility.read`, `alarm.read`, `audit.event.read` and other read grants in the recorded default sets | `<EMPTY>` |
| Tenant administrator (config, registry lifecycle — NOT a dispatch approver by design) | `Role::TenantAdmin` (same mapped role as Approver — see §3.2) | Registry create/delete/update actions, `quarantine.read`, and the read sets, per the recorded default sets | `<EMPTY>` |
| Platform administrator (platform registry only) | `Role::PlatformAdmin` (`policy.rs`) | Platform-level registry actions per the recorded defaults; **gets no tenant-match bypass** (`policy.rs` test `platform_admin_gets_no_tenant_match_bypass`) | `<EMPTY>` |
| Site technician (physical/maintenance access to OT equipment) | **NOT on the authorization surface** — a named GAP (§3.3, G4); the technician acts in the CUSTOMER's OT identity system, not in FlexCore | — | `<EMPTY>` |

### 3.2 Two design rules recorded explicitly

1. **Approver and tenant administrator share `TenantAdmin` deliberately**:
   the existing vocabulary has exactly one tenant-scoped administrative
   role, and the ledger's separation-of-duties requirement (step 4) is
   enforced by the P4.13 gate (§4.1), not by splitting roles in this
   record. If the customer requires the approver to hold FEWER rights
   than `TenantAdmin` (no delete rights, say), that is a named condition
   on the customer review (§8 row R-C), NOT a new role invented here —
   adding a `Role` variant is a reviewed code change outside this wave.
2. **Exactly one role set per user** (ledger acceptance (a)): the IdP
   claim mapping (§8) must map every pilot user to exactly ONE of the
   mapped `Role` values per the deployed policy set; no composite or
   "pilot-admin" super-role exists, and no shared or generic account may
   hold approval rights (ledger acceptance (d)) — audit attribution is
   the reason (§17.3).

### 3.3 Named gaps in the existing vocabulary (follow-ups, never silent)

| # | Gap | Why it matters at the pilot | Disposition |
|---|---|---|---|
| G1 | No step-up/MFA concept exists anywhere in the authz code (no action requires a second factor; `Action` has no step-up variants) | Ledger step 3 requires step-up for high-impact actions (§17.3) | DESIGN: step-up is enforced by the customer IdP + operator console (§20.2), NOT by the `PolicySet`; follow-up: console/IdP integration contract. Owner: control plane + customer IT |
| G2 | No enrollment-approval action on the `Action` surface: the P9.33 ceremony `Approve` gate is a platform human/authority act, not an authz action | Approving an edge enrollment must be identity-attributed | Follow-up: platform-side ceremony API (P9.33 §8 item 2) defines the approving-principal contract. Owner: control plane |
| G3 | The alarm surface is READ-only (`alarm.read` exists; `alarm.write`/`alarm.acknowledge` parse to `None` — `action.rs` tests) | An operator cannot acknowledge alarms through the authz surface | Follow-up: if pilot operations require acknowledgement, it is a closed-enum extension with explicit grants (the recorded P5.31 extension rule) — a reviewed change, never a request-field bypass. Owner: control plane |
| G4 | No "site technician" role and no customer-OT identity federation | §17.3 requires scoping by tenant/site/asset for EVERY actor; technician access lives in the customer's system today | Follow-up: customer identity boundary documented in the integration design (P9.04); FlexCore grants nothing to technicians. Owner: customer IT/OT |
| G5 | `command.issue` / `dispatch.authorize` / `dispatch.issue` parse to NOTHING and must never enter the surface (§5.7; `action.rs` tests) | ISSUED is the protected kernel's signed, sequenced step (P4.16); no human role issues | Permanent exclusion — recorded, not a gap to fix |
| G6 | No break-glass action or elevation mechanism exists in code | §6 needs one procedurally, not in code | §6 records the DESIGN: break-glass uses EXISTING grants exercised under an audited procedure, never a new bypass action |

## 4. Approval chains (§14 lifecycle; §21.4 operator-approved control)

### 4.1 The dispatch approval chain

| Chain element | Design (cited) |
|---|---|
| Gates are distinct | PROPOSED → VALIDATED → FEASIBLE → APPROVED → ISSUED → … (§14; AGENTS.md §4). Validation (P4.11 `command.validate`), feasibility (§14.1), and authorization (P4.13 `command.approve`) are THREE distinct gates — no action or grant collapses them |
| Authorization is a necessary condition, never sufficient | A `command.approve` policy match answers only "may this role approve AT THIS SCOPE"; the P4.13 gate additionally enforces separation of duties, scope coverage of the command's asset, and a recorded expiry (`phase4_policies.rs` header) |
| Separation of duties | The proposer (`command.create`, Operator) is NOT the approver (`command.approve`, TenantAdmin-only at site/asset — the P4.13 recorded default; §17.4). The ledger's step-4 rule: proposer ≠ approver UNLESS the customer explicitly accepts a named exception IN WRITING — exception cell `<EMPTY>`; a named exception lists the person, the actions, the expiry, and is filed with the P9.08 evidence |
| The operator-approval ceiling | The pilot is operator-approved control AT MOST (v1.0.0 ladder; `ENTRY_NOTE.md` boundary 3; §21.4): bounded automation is post-1.0 and no pilot configuration may enable it; the control-authority mode is a deployment/per-site setting no plugin can select (§5.7) |
| What approval is NOT | APPROVED ≠ issued; ISSUED ≠ delivered: acknowledgement is not physical delivery (§14.1; AGENTS.md §4); tracking requires ack + telemetry comparison (§3.4/P9.06 U3). No auto-approve action exists and none may be added (`action.rs`) |

### 4.2 The enrollment approval chain (edge identity)

Edge enrollment approval is a SEPARATE chain from dispatch: the P9.33
ceremony's `Approve` transition is a PLATFORM human/authority gate with
no self-approval path (`CEREMONY_SELF_APPROVAL_REFUSED`), a fresh
`PlatformAuthorityTicket` reference is the only re-enrollment basis, and
a compromised identity can never vouch for the new one
(`CEREMONY_COMPROMISED_IDENTITY_VOUCHING_REFUSED`; `enrollment_ceremony.rs`,
P9.33 §2/§4). The approving principal's identity contract is gap G2.

### 4.3 Authorization evidence recorded per command (§14.3)

| Required element (§14.3) | Where it comes from (cited) |
|---|---|
| Authorization decision reference | `EvidenceRef` — every `AuthorizationDecision` carries reason + evidence (the policy rule reference, e.g. `policy:phase4:tenant-admin:command.approve:site`); callers must log/audit the full decision before acting (`decision.rs`; `policy.rs` header) |
| Approved scope | The typed `Scope` the decision was evaluated against (`scope.rs`) |
| Limits | The command's own validated limits (P4.11 creation gate) — the approval grants authority to the command, not a capability |
| Expiry | The recorded expiry the P4.13 gate enforces (`phase4_policies.rs` header); the edge independently enforces the command validity window (`command_verify.rs` check 2) |

Per-command evidence RETRIEVAL for the pilot (how an auditor reads these
records back) is the audit read surface (`audit.event.read`, P4.36); its
deployment configuration is a reviewer cell in P9.25/P9.07, not this record.

## 5. Least-privilege mapping (design rule per role)

| Role | Least-privilege rule the design fixes | What it must never hold |
|---|---|---|
| Pilot operator | Propose/validate/read only; approves nothing | `command.approve`, registry deletes, `quarantine.read` |
| Approver | Approve ONLY at site/asset scope for their site's assets (the P4.13 default already excludes tenant-level approval); read everything needed to judge a command | `command.create` is permitted but a proposer-approver conflict on the SAME command is refused by the gate; registry deletes |
| Read-only observer | Read only; no mutation action anywhere | Every create/delete/approve action |
| Tenant administrator | Config/registry within the tenant; tenant match enforced (no cross-tenant grant, no bypass — `policy.rs`) | Cross-tenant anything; approval beyond site/asset scope |
| Platform administrator | Platform registry only; breadth from explicit grants, never a bypass | Cross-tenant scope (denied like everyone else — tested) |
| Everyone | Role from authenticated claims only (P0.16 rule 5); an unknown action id denies regardless of role | Any grant not present as an explicit `PolicySet` rule |

## 6. Break-glass procedure (DESIGN — nothing deployed, nobody named)

The design principle: break-glass uses EXISTING grants under an audited,
time-boxed procedure — it never adds a bypass action, never disables the
`PolicySet`, and never widens scope beyond what a grant already covers
(gap G6 is deliberate).

| Step | Design requirement |
|---|---|
| 1. Trigger | A documented trigger set (e.g. IdP outage blocking an emergency approval) — the trigger list is a customer review cell: `<EMPTY>` |
| 2. Dual authorization | TWO named humans authorize the elevation before it is applied; the request, the authorizers, the reason, and the expiry are written BEFORE the grant is exercised. Names: `<EMPTY>` |
| 3. Elevation mechanics | Assign the EXISTING `TenantAdmin` role (or the specific needed grant) to the named emergency principal via the normal application-layer role-assignment path — never via a request field, never via a plugin, never by editing policy code |
| 4. Time box | The elevation carries a hard expiry recorded with the grant; expiry is checked at the next review — automatic revocation wiring does not exist (honest gap; the expiry is procedural until a reviewed change adds it) |
| 5. Audit | Every action taken under elevation is audited (the normal `AuditSink` path); a post-incident review reconciles the audit trail against the break-glass record within the window the customer requires: `<EMPTY>` |
| 6. Retirement | The elevation is removed and the record signed off; the reconciliation result is filed with the P9.08 evidence |

## 7. Operator qualification (the P9.21 link)

Design rule: **an approval right is bound to qualification, not to the
role alone.** The pilot configuration holds no approval right for any
person who has not passed the P9.21 practical exercise (approve a valid
command, reject an infeasible one with its reason, recognise a
stale-telemetry state, execute the fallback/rollback procedure — P9.21
step 3). Mechanism (design): the role-assignment step checks the
qualification register before granting `command.approve` to any
principal; the register itself is P9.21's deliverable (per-person,
per-role, material version recorded) and is EMPTY. This record fixes the
RULE; P9.21 owns the register; the enforcement wiring is a deployment
composition item (honest gap: no code today reads a qualification
register — the rule is procedural until implemented).

## 8. IdP integration, MFA/step-up, and fallback (cells EMPTY; recommendations labelled)

| Item | Design position | Customer/deployment cell |
|---|---|---|
| Identity provider | OIDC/OAuth2 per §17.3, integrated at the `IdentityResolver` seam (`apps/control-plane/src/auth.rs`); the resolver constructs the claims-derived context — never a request field | IdP name/product: `<EMPTY>` |
| Claim → role mapping | The resolver maps IdP claims onto exactly one mapped `Role` (§3.2); the mapping table is deployment configuration, reviewed with the customer IT/IdP owner | Mapping: `<EMPTY>` |
| Session/credential lifetime | Short-lived tokens per §17.3; concrete lifetimes are deployment decisions linked to P9.09's rotation inventory | Lifetimes: `<EMPTY>` |
| MFA | Required for all pilot users per §17.3, enforced at the IdP | Customer IdP capability: `<EMPTY>` |
| Step-up (ledger step 3) | RECOMMENDATION: step-up required for `command.approve` (the approval gate) and tenant/registry delete actions; the console presents the step-up challenge (§20.2) — enforcement lives in the IdP + console, NOT the `PolicySet` (gap G1) | Which actions the customer REQUIRES step-up for: `<EMPTY>`; written exceptions: `<EMPTY>` |
| IdP-unavailable fallback | RECOMMENDATION: fail CLOSED — the `DenyAllResolver` default posture (`auth.rs`): no cached offline approval, no local bypass; emergency needs go through the §6 break-glass procedure | Customer acceptance of fail-closed: `<EMPTY>` |

## 9. Negative-test matrix (TEMPLATE — execution is the human gate; UNRUN)

Ledger step 6 and acceptance (c): each role attempts an action above its
permission and is denied, with the denial audited. The evaluator-level
behaviour is pinned by the crate's own tests (`policy.rs`: every deny
branch; `action.rs`: off-surface ids deny); the DEPLOYMENT-level tests
below are human executions against the pilot configuration — none has run:

| # | Role under test | Attempted above-permission action | Expected decision (cited reason) | Executed? | Audit record inspected? |
|---|---|---|---|---|---|
| N1 | `Operator` | `command.approve` | Deny `NoMatchingRule` (no Operator approve grant exists — `phase4_policies.rs`) | `<EMPTY — UNRUN>` | `<EMPTY>` |
| N2 | `Viewer` | `command.create` | Deny `NoMatchingRule` | `<EMPTY — UNRUN>` | `<EMPTY>` |
| N3 | any role, cross-tenant scope | in-scope action against another tenant's asset | Deny `ScopeDenied` (no role bypasses — `policy.rs`) | `<EMPTY — UNRUN>` | `<EMPTY>` |
| N4 | any role | off-surface action id (e.g. `command.issue`, `alarm.write`) | Deny `UnknownAction` (`action.rs`) | `<EMPTY — UNRUN>` | `<EMPTY>` |
| N5 | proposer == approver on the same command | approve own proposed command | Refused by the P4.13 gate's separation-of-duties check | `<EMPTY — UNRUN>` | `<EMPTY>` |
| N6 | unqualified principal | `command.approve` with qualification register absent | Refused per §7 rule | `<EMPTY — UNRUN>` | `<EMPTY>` |
| N7 | IdP unavailable | any authenticated action | Fail-closed (no fallback authorization) | `<EMPTY — UNRUN>` | `<EMPTY>` |

## 10. Reviewer-verification procedure (human)

1. Confirm §1's plain statement and that EVERY role-assignment, IdP,
   step-up, exception, and test-result cell is `<EMPTY>`/UNRUN.
2. Confirm §3 maps each pilot role to a REAL `flexcore-authz` `Role`
   variant and every claimed grant to a recorded default set
   (`phase4_policies.rs`, `auth.rs` builders) — no invented action id,
   no invented role, no grant the builders do not make.
3. Confirm the six gaps G1–G6 are recorded as named follow-ups with
   owners, not silently assumed solved.
4. Confirm §4 never describes an auto-approve path, a collapsed gate, or
   a bounded-automation capability, and that `command.approve` is
   TenantAdmin-only at site/asset scope.
5. Confirm §6 break-glass adds no bypass action and names nobody; §7's
   qualification rule defers the register to P9.21.
6. Confirm the ledger tick wording is "implemented, awaiting human
   build/test".

## 11. Filling commands (for the human executor)

- With the customer IT/IdP owner: fill §8 (IdP, claim→role mapping,
  MFA/step-up requirements, written exceptions, fallback acceptance);
  record any named proposer==approver exception with person, actions,
  and expiry (§4.1).
- Assign people (§3.1 assignment cells) AFTER P9.21 qualification
  exists (§7 rule); confirm one role set per user and no shared
  approval-capable account (ledger acceptance (a)/(d)).
- Execute the §9 negative tests in the pilot environment; inspect the
  audit records for each denial; file the extracts as the P9.08
  evidence named in `CHECKLIST.md` ("Identity and access enforced").
- Raise the named follow-ups G1–G3/G5–G6 into the Phase 7 contract
  list; G4 into the P9.04 integration design.

## 12. Open risks carried to P9.27 (this subtask's contribution)

| Risk | Basis |
|---|---|
| No IdP integration exists (only `DenyAllResolver`/dev resolver in code) — identity enforcement at the pilot is entirely future deployment composition | §2, §8; P9.07 §10 C1 |
| No step-up/MFA enforcement mechanism in code (gap G1) — a step-up "defined in a document but not enforced in the console" is the ledger's named failure mode | §3.3 G1; §8 |
| Approver shares the `TenantAdmin` role with administrative rights; a finer-grained approver role requires a reviewed code change, not this record | §3.2 rule 1 |
| Qualification-before-approval enforcement is procedural (no code reads a qualification register) until implemented | §7 |
| Break-glass expiry/revocation is procedural (no automatic revocation wiring) | §6 step 4 |
| No pilot user, IdP, mapping, or negative test exists — every acceptance criterion of the ledger subtask is UNMET and not claimed | §1, §9 |
| Alarm acknowledgement is impossible on the current authz surface; if pilot operations need it, it is a reviewed closed-enum extension | §3.3 G3 |

Status: **WRITTEN-UNRUN** — a design record cannot be "run"; the IdP
integration, role assignments, step-up configuration, and negative-test
execution are the human gates and have not happened and are not
scheduled. No identity was created; no pilot exists and none is claimed.
