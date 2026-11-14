<!-- P10.29 — Pilot handover pack / operations runbook (ADDITIVE doc). HONEST
     LABEL: this runbook describes the DESIGNED day-1 operating procedures
     for a v1.0.0 pilot — it does NOT describe a deployed site, because no
     site, deployment, operator, or handover exists. Every operational
     value (names, contacts, windows, rota, thresholds, site IDs, mode
     selections) is a PLACEHOLDER/EMPTY cell owned by a human gate; every
     procedure is cited from its owning Phase 9/Phase 10 record, never
     restated as active. No walkthrough has been held; the handover
     receipt is UNSIGNED; the handover acceptance checklist is EMPTY. -->
# FlexCore v1.0.0 — Pilot Handover Operations Runbook (DESIGN — describes designed procedures, NOT a deployed site)

| Field | Value |
|---|---|
| Subtask | P10.29 (`ledger/phase-10-v1-release-hardening/ledger_phase_10.md` §P10.29); decision record `ledger/phase-10-v1-release-hardening/29_pilot_handover_pack.md` |
| Document status | **DESIGN RUNBOOK — describes the procedures the handover pack WOULD contain for a v1.0.0 pilot.** No site, no deployment, no pilot operator, and no handover exists; v1.0.0 IS NOT RELEASED (P10.28 prepared-NOT-EXECUTED). Every operational value is a `<PLACEHOLDER>` or an EMPTY cell filled only by its owning human gate. |
| Control status | The pilot ceiling is operator-approved control AT MOST — and it is NOT QUALIFIED (P10.15, VERDICT: NOT QUALIFIED); simulation and read-only/shadow are the unqualified defaults. Bounded automation does not exist in v1.0.0 and is post-1.0 (P10.30 deferral record). FlexCore is a supervisory orchestration layer — no protection-grade, hard-real-time, or sub-cycle control. |
| Verification status | **WRITTEN, NEVER EXECUTED.** The pack walkthrough with pilot operators (ledger step 3) has NOT been held; the qualification-register check (ledger step 4) has NOT run; the handover receipt (ledger step 5) is UNSIGNED. |
| Sources (all cited, none restated as active) | `docs/operations/deployment-upgrade-model.md` (P10.24); `docs/operations/edge-agent-operations.md`; `ledger/phase-09-pilot-qualification/21_operator_training.md` (P9.21); `22_market_operational_procedures.md` (P9.22); `23_incident_response.md` (P9.23); `24_backup_dr_rehearsal.md` (P9.24); `25_observability_alerting.md` (P9.25); `26_control_scope_document.md` (P9.26); `ledger/phase-10-v1-release-hardening/22_site_harness.md` (P10.22); `11_backup_restore_rc.md` (P10.11); `23_operator_workflow_stability.md` (P10.23); `15_operator_mode_qualification.md` (P10.15); `30_post_release_deferral.md` (P10.30) |
| Units | MW, MVAr, MVA, MWh; `P > 0` = export at the point of connection; SOC a fraction in [0,1] at the API boundary |

## 1. What this runbook is, and is not

This runbook is the STRUCTURE of the pilot handover pack's operating
core: what the pilot operators do on day 1, which procedures govern
which situation, and where each procedure's authoritative text lives.
It is assembled BY CITATION — the owning records (all WRITTEN-UNRUN,
every execution cell EMPTY) keep their authority; nothing here re-
specifies, activates, or closes any of them.

It is **not** a description of a deployed site. There is no site, no
customer, no pilot operator, no on-call rota, and no released v1.0.0.
The ledger's named failure mode for P10.29 — "a runbook that describes
the pilot build, not the release" — is guarded here by version
discipline: every procedure reference names its OWNING record and the
version-discipline rule of §7 requires each procedure re-verified
against the released v1.0.0 before any handover; a pack that skips that
re-verification is invalid, not implicit.

## 2. The pack contents (ledger step 1) — each item with its status

| # | Pack item | Owning record / document (cited) | Status at this date |
|---|---|---|---|
| H-1 | Deployment guide (v1.0.0) | `docs/operations/deployment-upgrade-model.md` (P10.24) | DESIGN; every site value `<PLACEHOLDER>`; review-deploy NOT DONE |
| H-2 | Release notes (v1.0.0, with known limitations) | P10.26 (`26_release_notes.md` — concurrent sibling, expected path) | not written at this date |
| H-3 | Operations manual (P9.22 procedures, updated to v1.0.0) | `ledger/phase-09-pilot-qualification/22_market_operational_procedures.md` | conditional DRAFTS; no market exists (D1 OPEN); every activation-condition cell EMPTY |
| H-4 | Incident-response procedure | `ledger/phase-09-pilot-qualification/23_incident_response.md` (§3 of this runbook) | procedure written; tabletop UNHELD; rota UNSIGNED; contacts/deadlines `<EMPTY>` |
| H-5 | Backup/restore and DR procedure | `ledger/phase-09-pilot-qualification/24_backup_dr_rehearsal.md` + `ledger/phase-10-v1-release-hardening/11_backup_restore_rc.md` (§4 of this runbook) | plans written; nothing ever backed up or restored; RPO/RTO `<PLACEHOLDER>` |
| H-6 | Alert catalogue (P9.25) | `ledger/phase-09-pilot-qualification/25_observability_alerting.md` (§5 of this runbook) | verification TEMPLATE; no alert infrastructure; thresholds `<PLACEHOLDER per X-8>`; recipients `<EMPTY>` |
| H-7 | Control-scope document | `ledger/phase-09-pilot-qualification/26_control_scope_document.md` | prepared, NOT APPROVED; all signature cells `<EMPTY — UNSIGNED>`; no site promoted |
| H-8 | On-call rota | P9.23 §8.1 (owned there) | ALL CELLS EMPTY (UNSIGNED) |
| H-9 | Escalation contacts | P9.23 §5 per-class escalation paths (owned there) | customer contacts `<EMPTY>` — filled only from the customer contract |
| H-10 | Site integration test harness | `ledger/phase-10-v1-release-hardening/22_site_harness.md` + `harness/site-integration/` (§6 of this runbook) | manifest + entry procedure only; never built, never run |
| H-11 | Operator qualification register | `ledger/phase-09-pilot-qualification/21_operator_training.md` §5 (§2 of this runbook) | EMPTY — no trainee exists |
| H-12 | Handover receipt + walkthrough record | `ledger/phase-10-v1-release-hardening/29_pilot_handover_pack.md` §4 | EMPTY / UNSIGNED |

**The pack is not assemblable as a final v1.0.0 pack until P10.28
exists** (the ledger makes P10.28 a dependency of P10.29): a pack for a
release that is not released is a design pack, and this document is
honestly that — a design pack structure.

## 3. Day-1 operations (the runbook's operating core — every value a placeholder)

Day 1 of a pilot under the v1.0.0 ceiling would proceed through the
following sequence. Each step cites its authoritative procedure; the
"filled by" column names the human gate that owns the real value. No
step is executable today.

| Step | Day-1 activity | Authoritative procedure (cited) | Filled by (currently EMPTY) |
|---|---|---|---|
| D-1 | Confirm the deployed mode per site BEFORE any operator action: read the site's mode record; the mode is a deployment/per-site setting no plugin can raise (§21.4) | `26_control_scope_document.md` §5 C-1..C-11; `deployment-upgrade-model.md` §4 | signed control-scope doc + configured mode: `<PLACEHOLDER>` |
| D-2 | Confirm operator qualification BEFORE any approval attempt: every principal holding `command.approve` appears in the qualification register (P9.21 §5) — the rule is procedural until implemented (P9.08 §7 honest gap) | `21_operator_training.md` §5–§6 | register: `<EMPTY — no trainee exists>` |
| D-3 | Confirm the on-call rota version in force and that the customer-side contacts were acknowledged in writing per version (P9.23 §8.2) | `23_incident_response.md` §8 | rota version + contacts: `<EMPTY — UNSIGNED>` |
| D-4 | Confirm the alert path is verified (P9.25 §5 V-1..V-4 for at least the CRITICAL classes) before relying on any alert leg | `25_observability_alerting.md` §5 | verification record: `<EMPTY — UNRUN>` |
| D-5 | Confirm telemetry freshness discipline: the six quality states and what each forbids; no schedule/command built on STALE/SUSPECT/MISSING/ESTIMATED/BAD data | §10.3–§10.4 via `21_operator_training.md` C3; §17.5 invariant 9 | site freshness limits: `<PLACEHOLDER>` |
| D-6 | Confirm the command path state: v1.0.0 ships the command path DISABLED by default (config + every shipped profile); enabling requires the P6.14 four-condition gate | `deployment-upgrade-model.md` §4; `15_operator_mode_qualification.md` §3 R-8 | enablement record: `<EMPTY>` |
| D-7 | Walk the shadow comparison posture: the pilot's default operating evidence is the shadow/read-only surface until the qualification chain (P10.15 §9) is filled | `15_operator_mode_qualification.md` §9 | promotion decisions: `<EMPTY>` (P9.26 §6.3 table) |
| D-8 | Confirm the fallback posture per asset: the DEFAULT entry for an asset with no named policy is require-operator-intervention — never a universal zero-setpoint | `23_incident_response.md` §5 I-6; P9.20 §4 register | per-asset fallback register: `<PLACEHOLDER>` |

Day-1 operator procedures proper — creating/validating a command,
reading rejections, alarm handling (read surface; acknowledgement routes
OUTSIDE FlexCore until the P9.08 gap G3 closes), escalation to the
approver — are the P9.21 §3.2 modules (O1–O5, A1–A5) taught in training
and exercised in the P9.21 §4 assessment (S-1..S-4); this runbook does
not re-teach them, it binds the day-1 flow to them. The §14.2 state
definitions (APPROVED ≠ issued; acknowledgement ≠ delivery) are used
verbatim as trained — never paraphrased into blur.

## 4. Shadow-mode operator procedures (P9 records, by citation)

The pilot's qualified-on-paper ceiling for day 1 is read-only/shadow
operation (P10.15 §9: the release ships — if it ships at all — with
simulation / read-only (shadow) authority only until the qualification
chain is filled). The shadow procedures and their owning records:

| Situation | Owning record (cited) | What the operator uses | Status |
|---|---|---|---|
| Market/product operational situations (the P9.22 six situations) | `22_market_operational_procedures.md` §4–§9 | the conditional DRAFT procedure for the situation — ACTIVATION CONDITION cell must be FILLED by the P9.11 capture before any use | drafts; no market named (D1 OPEN); no walkthrough ever held |
| Telemetry quality interpretation | §10.4 six states via `21_operator_training.md` C3 | the trained state table; console quality distinction | trained procedure; no operator exists |
| Command-lifecycle reading (proposed → … → completed; REJECTED with named failing check) | §14.1–§14.2 via `21_operator_training.md` C4; `23_operator_workflow_stability.md` | the §20.2 principle that every displayed state matches the journal (P10.23's check) | check record EMPTY (no console build) |
| Alarm handling | `25_observability_alerting.md` §2.3 (`/api/v1/alarms` — derived-on-read, READ-ONLY) | the seven alarm classes (`TELEMETRY_LOSS`, `TELEMETRY_STALENESS`, `DISPATCH_FAILURE`, `DISPATCH_EXPIRY`, `DISPATCH_FALLBACK`, `FEASIBILITY_LIMIT_VIOLATED`, `FEASIBILITY_FAIL_CLOSED`); acknowledgement routes OUTSIDE FlexCore (P9.08 gap G3) | no acknowledgement path exists — recorded gap, carried |
| Rejection reading | P4.11 discipline via `21_operator_training.md` O3 | every rejection names its failing check | trained procedure |
| Emergency/rollback actions (command cancellation; site reversion to shadow; local-EMS fallback posture) | `21_operator_training.md` E1; `23_incident_response.md` §5 containment C-3 | §14.1 CANCELLED transition (actor + reason mandatory, P4.15); §21.4 operational mode reversion — confirmed in the site's mode record | EXERCISED in training only when training runs (S-4); never run |

**Nothing in this section authorizes a write to a device.** In shadow
mode there are no device writes by definition (§21.4 mode row); the
procedures above are observation, proposal, validation, and escalation
procedures.

## 5. Escalation (per `23_incident_response.md`) — the paths, not the contacts

Escalation follows P9.23 §5's per-class paths verbatim in structure:
first responder = pilot operator (`Role::Operator` — propose/validate/
read, NO approve) → approver/escalation authority (`Role::TenantAdmin`,
site/asset-scope approval only) → security owner → customer contacts;
site-technician actions live in the customer's OT identity system
(P9.08 gap G4); break-glass uses P9.08 §6's dual-authorization
procedure only — never a bypass. The severity scheme is P9.23 §4
(SEV-1..SEV-4, assigned with evidence, upgrade path always open).

The six incident classes (I-1 forged/replay, I-2 unauthorized action,
I-3 cross-tenant, I-4 edge compromise, I-5 telemetry integrity, I-6
central outage) keep P9.23 §5 as their authoritative table — including
its containment invariants: evidence preservation (§6) precedes
remediation; containment never disables the `PolicySet`, never adds a
bypass, never widens control scope. **Every customer contact and
notification-deadline cell in that table is `<EMPTY>`** — filled only
from the customer contract by the human gate, never from this runbook.

| Escalation value | Status |
|---|---|
| On-call rota (all roles) | `<EMPTY — UNSIGNED, P9.23 §8.1>` |
| Customer primary / escalation contacts | `<EMPTY — no customer exists>` |
| Contractual notification deadlines per class | `<EMPTY — contract-cited fill only>` |
| Break-glass dual authorizers | `<EMPTY>` |
| Tabletop held (corrects the procedure before any live use) | `<EMPTY — UNHELD>` |

## 6. Backup/DR entry points (per `24_backup_dr_rehearsal.md` + `11_backup_restore_rc.md`)

The authoritative backup/restore and DR procedure is P9.24's rehearsal
plan, re-run against the release candidate by P10.11's RC re-run rule
("RE-RUN, NOT INHERIT"). The operator-facing entry points for the
runbook:

- **Backup scope and stores:** the REAL stores cited by P9.24 §4
  (PostgreSQL, NATS JetStream streams, durable command/audit records,
  edge journals) with their append-only/evidence properties — cited
  there, not restated here.
- **Restore discipline:** restore into an ISOLATED environment;
  post-restore verification of command/audit completeness, outbox/inbox
  idempotency (no command re-issued by replay), schema-version match,
  scoped JetStream restore authorization (`11_backup_restore_rc.md`).
- **Rollback/DR on upgrade failure:** the edge verify-before-stage
  register rules and the no-downgrade-migration rule (UP-only runtime
  migrator) — `deployment-upgrade-model.md` §3.2–§3.3, cited, not
  re-specified.
- **Central-outage rule (verbatim intent, §21.2):** a central outage
  must not disable site-local protective controls or force an unsafe
  device state; no new commands while disconnected; recovery requires a
  NEW command through the full chain.
- **RPO/RTO:** `<PLACEHOLDER — §21.2 forbids setting them without
  customer and service requirements; P10.11 run cells all EMPTY>`.

| Backup/DR value | Status |
|---|---|
| Backup configuration exists | **NO — nothing has ever been backed up** |
| Restore performed/measured | **NO — never performed** |
| Edge-outage re-test E-1..E-6 | `<EMPTY — UNRUN>` |
| Measured RPO / RTO | `<PLACEHOLDER>` |

## 7. Site harness entry (per `22_site_harness.md`) and version discipline

The site integration test harness (`harness/site-integration/` —
manifest + entry procedure + EMPTY run-record template) is the pack's
acceptance instrument at the site: before handover, the site runs the
harness entry procedure against the RELEASED version. Rules carried
from the owning record:

1. The harness has NEVER been packaged-built or run; "runnable" is
   verified by construction only — the first human execution by a
   non-author fills its run record (`22_site_harness.md`).
2. Version-mismatch detection is part of the entry procedure: the
   harness refuses a mismatched version rather than degrading.
3. **Version discipline for the whole pack (ledger step 2):** every
   procedure in the pack must refer to the RELEASED version (v1.0.0)
   and the frozen grid-model/adapter versions — never the pilot build.
   The check is: every pack item (H-1..H-11) re-verified against the
   released artifacts after P10.28 exists, with the re-verification
   recorded in the decision record `29_pilot_handover_pack.md` §3.
   A pack item still referring to a superseded version invalidates the
   handover.

| Harness item | Status |
|---|---|
| Harness packaged-built | `<EMPTY — never built>` |
| Clean-environment run by a non-author | `<EMPTY — UNRUN>` |
| Version-mismatch detection verified | `<EMPTY — UNRUN>` |

## 8. What this runbook does NOT establish

1. It does not describe a deployed site — no site, deployment, pilot,
   operator, or handover exists; every operational value is a
   `<PLACEHOLDER>` or an EMPTY cell owned by a named human gate.
2. It does not activate any P9.22 market procedure (D1 OPEN; activation
   conditions EMPTY) and never presents a draft as active.
3. It does not grant field write authority: the pilot ceiling is
   operator-approved AT MOST, and that mode is NOT QUALIFIED (P10.15
   §9); day 1 operates in shadow/read-only posture until the chain is
   filled; bounded automation does not exist in v1.0.0 (P10.30 deferral
   record).
4. It does not fill any P9.23 contact, deadline, or rota cell — those
   are customer-contract cells owned by P9.23's human gates.
5. It does not replace any owning record: every section binds to its
   cited record; where this summary and the owning record differ, THE
   OWNING RECORD WINS and the divergence is a finding to raise.
6. The walkthrough with pilot operators (ledger step 3), the
   qualification-register check against the released version (ledger
   step 4), and the signed handover receipt (ledger step 5) have NOT
   happened — they are the open acceptance items, not exceptions claimed
   as satisfied.

## 9. Reviewer-verification procedure

1. Confirm the honest labels: the header and §1/§8 carry
   DESIGN-not-deployed; no sentence describes an operating site.
2. Confirm every pack item in §2 cites its owning record by path and
   that no status cell claims an executed or signed state.
3. Confirm §4–§7 bind to the owning records (P9.22, P9.23, P9.24/
   P10.11, P9.25, P9.26, P10.22) without re-specifying their content,
   and that the P9.23 contacts/deadlines/rota remain `<EMPTY>` here.
4. Confirm no bounded-automation capability is described, assumed, or
   scheduled anywhere; the mode ceilings of §1/§8 hold.
5. Confirm §7's version-discipline rule names the released-version
   check as a precondition, not a done state.
6. Confirm no real hostname, IP, person, contact, threshold, or date
   appears anywhere — every operational value is a `<PLACEHOLDER>` or
   an explicitly EMPTY cell.
7. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the runbook is written; the walkthrough, qualification
   check, and handover have not happened.
