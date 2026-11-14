<!-- P9.23 — Incident response and customer notification procedure (PROCEDURE + TEMPLATES). The procedure is WRITTEN; every execution cell is EMPTY: no tabletop has been held (UNHELD), no on-call rota exists (UNSIGNED), no customer contact or notification deadline is filled (PLACEHOLDER — customer-specific per contract), no incident has occurred or been handled. No severity implies any response time that is not an EMPTY cell. This record is the P9.36-recorded missing prerequisite: the "P9.23 does not exist" dependency is closed by this record being written; the "P9.23 procedure + tabletop record SIGNED" dependency stays OPEN until the human tabletop. -->
# P9.23 — Incident Response and Customer Notification Procedure (Procedure — tabletop UNHELD, rota UNSIGNED, deadlines PLACEHOLDER)

Date: 2026-11-14. Subtask: P9.23 (`ledger_phase_9.md` §P9.23).
Owner role: Security (with product/operations and the customer).
Report trace: §23 Stage 9; §17.3 (incident response and customer
notification process); §17.4.
Inputs: P9.07 security assessment (TEMPLATE —
`07_deployment_security_assessment.md`; this record does not read its
findings for content, only its §17.4 threat vocabulary); P9.01 scope
(TEMPLATE, UNSIGNED); customer escalation contacts and contractual
notification deadlines — **which do not exist**; P9.08 identity design
(`08_identity_access_approval.md` — the roles and break-glass design
cited below).

## 1. Honest status — the plain statement

**No incident has occurred, no incident class has been exercised, no
tabletop has been held, no on-call rota exists, and no customer contact or
notification deadline has been recorded** — there is no customer, no site,
no deployment, and no contract. Every deadline, contact, rota, and
exercise cell in this record is `<EMPTY>`/PLACEHOLDER/UNHELD. What this
record delivers is the PROCEDURE the human gates complete: the severity
scheme (§4), the six ledger incident classes with detection source, first
responder, escalation path, and containment (§5 — the contact/deadline
cells are PLACEHOLDERS filled from the customer contract, never invented),
the evidence-preservation rule (§6), the communication-template skeletons
(§7), the on-call rota template with its update procedure (§8), and the
P9.36 linkage closure (§3).

The ledger's acceptance criteria for P9.23 are NOT met and are NOT
claimed: (a) every class has detection/escalation/containment, but the
customer contact and notification deadline cells are EMPTY; (b) the
revert-to-shadow/local-control containment action is written as the
mandatory step, unexercised; (c) the tabletop has NOT been held with the
customer (there is no customer) and no corrections exist; (d) the on-call
rota does not exist.

## 2. Design basis — what the procedure rests on (cited, read 2026-11-14)

| Surface | Where | What it provides |
|---|---|---|
| §17.4 threat vocabulary | report §17.4; P9.07 assessment | The incident classes in §5 name the threats the §17.4 table already enumerates (forged/replayed command, unauthorized operator action, cross-tenant access, edge compromise, telemetry integrity, central outage) — no new threat is invented here |
| Pilot roles | `08_identity_access_approval.md` §3 (mapped to `crates/flexcore-authz/src/policy.rs` :: `Role::{PlatformAdmin, TenantAdmin, Operator, Viewer}`) | The escalation-path actors: first responder = pilot operator (`Role::Operator` — propose/validate/read, NO approve); approver/escalation authority = `Role::TenantAdmin` (approver at site/asset scope per `phase4_policies.rs`); read-only observer = `Role::Viewer`; platform admin for platform-registry actions; NO site-technician role in FlexCore (P9.08 gap G4 — technician acts in the customer's OT identity system) |
| Break-glass | `08_identity_access_approval.md` §6 | Emergency elevation uses EXISTING grants under the audited, time-boxed, dual-authorization procedure — never a bypass action, never a `PolicySet` disable; invoked only where an incident class names it, with authorizer names `<EMPTY>` |
| IdP-unavailable fallback | `08_identity_access_approval.md` §8 (fail-closed `DenyAllResolver` posture, `apps/control-plane/src/auth.rs`) | An IdP outage is fail-closed, not an incident workaround: emergency authorization goes through §6 break-glass, never through a cached or local bypass |
| Edge rejection + audit evidence | `crates/edge-agent/src/command_verify.rs` (§18.2: the edge validates every command); `crates/edge-agent/src/pilot_rejection_proof.rs` :: `PilotAttackCase` (closed inject vocabulary: `forged_signature`, `replayed_captured`, `expired_window`, `out_of_sequence`, `conflicting_idempotency_payload`); greppable audit lines `EDGE_PILOT_ATTACK_REJECTED`, `EDGE_COMMAND_REJECTED`, `EDGE_COMMAND_ID_BINDING_CONFLICT` | The detection sources and evidence of incident classes I-1/I-4 (§5) |
| Audit tamper-evidence | `crates/edge-agent/src/audit_proof.rs` :: `compute_chain_anchor`, `verify_against_anchor`, `recompute_chain_head` (FNV-1a-64 chain — tamper-EVIDENCE, not cryptographic proof); central `audit_records` append-only (migration 0008: trigger + no-DELETE grants, corrections are NEW rows) | The evidence-preservation mechanics of §6 |
| Edge audit buffer | `crates/edge-agent/src/audit_buffer.rs` (P6.18: bounded journal, disk-first append, in-order at-least-once upload, audited drop-of-oldest with `dropped_oldest` counter and `has_overflowed` latch) | Evidence captured during a central outage (I-6) survives at the edge within the bounded buffer — the loss window is a recorded property, not zero by assumption |
| Mode reversion | §21.4 (control-authority mode is a deployment/per-site setting; no code path "reverts" itself — `36_forged_command_drill.md` §3 leg 5) | Containment step C-3: the revert-to-shadow/local-control action is an OPERATIONAL action executed by people, confirmed in the site's mode record and audited |

## 3. The P9.36 linkage — the missing-prerequisite closure, stated explicitly

`36_forged_command_drill.md` §1 records, verbatim in substance: "P9.23
incident-response + customer-notification procedure + tabletop record —
**NOT YET WRITTEN** … the drill CANNOT be scheduled until P9.23's
procedure exists and its tabletop (its step 4, a forged/replayed-command
tabletop with the customer) has run — the ledger itself names both as
inputs. Until then this drill stays UNSCHEDULABLE, not merely UNRUN."

**Closure statement (explicit):**

1. The **"procedure does not exist"** half of that dependency is CLOSED by
   this record: the procedure (§4–§8) now exists as of 2026-11-14.
2. The **"tabletop record exists and is signed"** half is NOT closed: the
   §9.1 tabletop is UNHELD. Until it is held with the customer and its
   corrections are applied to this procedure, **P9.36 remains
   UNSCHEDULABLE** per its own recorded scheduling rule ("P9.36 is
   executable only after ALL of: P9.23 procedure + tabletop signed; P9.31
   matrix run against the pilot edge build; the site in shadow per P9.17;
   P9.25 alert path verified").
3. **Ordering rule (tabletop-then-live):** the §9.1 tabletop — this
   procedure's step 4 in the ledger, a forged/replayed-command attempt
   walked with the customer — is held BEFORE any live drill. The live P9.36
   drill executes THIS procedure as corrected by the tabletop, never the
   pre-tabletop text. A live drill run against a tabletop-unsigned version
   of this procedure is a protocol violation, and its record cannot close
   P9.36.
4. This record therefore feeds P9.36 schedulability in exactly one
   direction: it removes the "not written" blocker and defines the
   notification-deadline cell the drill reads ("the deadline is READ from
   the P9.23 procedure, never invented there" — `36_...md` §2 step 0).
   While §5 I-1's deadline cell is `<EMPTY>`, the drill's deadline clock
   has nothing to read and cannot start.

## 4. Severity classification scheme (DESIGN — assignment exercised at the tabletop)

Four levels, assigned by the first responder at declaration and re-assigned
on new information. The scheme classifies IMPACT and URGENCY; it does NOT
fix response times — every time bound is a customer-contract cell
(`<EMPTY>`), filled in §5 per class.

| Severity | Meaning at the pilot | Examples (from §5 classes) | Initial response expectation |
|---|---|---|---|
| SEV-1 | Evidence that a control-integrity attack or compromise is in progress or succeeded — containment cannot wait for the next business step | I-1 with a command that reached an adapter/device (rule: zero-adapter-reach holds → not SEV-1 by itself); I-4 with confirmed edge compromise | Immediate containment (C-1–C-3); escalate in parallel, not sequentially |
| SEV-2 | Attack attempt contained by design (rejected at the edge, nothing reached a device), or an integrity failure with bounded, counted loss | I-1 rejected attempts; I-5 telemetry integrity failure; I-3 cross-tenant attempt denied and audited | Contain per class; notify per the class deadline |
| SEV-3 | Degraded operation with no integrity implication | I-2 unauthorized action denied; I-6 central outage with site-local fallback active (P9.20 D-1 behaviour) | Restore service; record; notify per class deadline |
| SEV-4 | Observation with no service or integrity impact, recorded for pattern analysis | Repeated rejected attempts at a rate suggesting probing | Record and monitor; batch review |

Assignment rules: (i) the severity is recorded WITH its evidence reference
(audit line, alert, ticket) at declaration — a severity without evidence is
not a declaration; (ii) an upgrade path SEV-4→SEV-1 exists at any time and
the §7 notification clock starts at the SEV-1 declaration, not the original
observation; (iii) downgrades require the approver-role person's record in
the incident log. No severity number carries an automatic customer deadline
— the deadlines are the contract's (§5).

## 5. Incident classes (ledger step 1–2) — contacts and deadlines are PLACEHOLDERS

For every class: Detection source (cited), First responder, Escalation
path, Customer contact + notification deadline (**`<EMPTY>` — filled only
from the customer contract/escalation sheet by the human gate**), and
Immediate containment. Class I-1 is the P9.36 drill class.

| # | Class | Detection source (cited) | First responder | Escalation path | Customer contact / notification deadline | Immediate containment |
|---|---|---|---|---|---|---|
| I-1 | Suspected command forgery/replay | Edge rejection audit lines (`EDGE_COMMAND_REJECTED` with rejection code, `EDGE_PILOT_ATTACK_REJECTED`, `EDGE_COMMAND_ID_BINDING_CONFLICT` — `command_verify.rs`, `pilot_rejection_proof.rs`, `audit_buffer.rs`); alert leg per P9.25 (infrastructure-PENDING until P9.25 runs) | Pilot operator (`Role::Operator`) | Operator → approver (`Role::TenantAdmin`) → security owner → customer contact `<EMPTY>`; break-glass (P9.08 §6) if approval authority is unavailable | `<EMPTY>` | C-1 preserve evidence (§6) → C-2 stop accepting external commands at the affected site (site-policy admission closes — `crates/edge-agent/src/policy.rs`) → C-3 **revert the site to shadow or local control** (§21.4 operational action, confirmed in the site's mode record) → C-4 hand the attack-case set to the P9.36 drill vocabulary |
| I-2 | Unauthorized operator action | Authorization denial audit records (`audit_records`, migration 0008; deny decisions with `DenyReason` + `EvidenceRef` — `crates/flexcore-authz`); negative-test surface (P9.08 §9) | Pilot operator | Operator → approver → security owner; credential revocation per P9.09's measured revocation procedure (`<EMPTY>`) | `<EMPTY>` | C-1 preserve evidence → C-2 revoke/suspend the implicated credential per P9.09 → C-3 if the action touched control state, revert affected site(s) to shadow per §21.4 and confirm in the mode record |
| I-3 | Cross-tenant access attempt | Isolation denials audited (`ScopeDenied` carries actor/object/timestamp in the audit-record shape — P6.18/P6.21; platform-side denials land in `audit_records`) | Pilot operator | Operator → approver → security owner; RLS/`tenant_isolation` posture verified against the running roles per `10_data_residency_retention.md` §9.A (the owner-bypass warning [R25] stands) | `<EMPTY>` | C-1 preserve evidence → C-2 suspend the implicated principal (customer IdP + P9.09 revocation) → C-3 verify no cross-tenant row was returned (query logs) — a confirmed cross-tenant READ upgrades to SEV-1 |
| I-4 | Edge compromise (physical or identity) | Health vocabulary (`crates/edge-agent/src/health.rs`: `FAILING_*` blocks ingest/commands with audited reason), enrollment register (a revoked identity can never authorize its successor — `enrollment_ceremony.rs`), missing/odd audit chain anchor on verification | Pilot operator with the site technician (CUSTOMER side — P9.08 gap G4) | Operator → approver → security owner → customer OT owner; re-enrollment only via the P9.33 ceremony (`Approve` is a platform human/authority gate; no self-approval; `CEREMONY_COMPROMISED_IDENTITY_VOUCHING_REFUSED`) | `<EMPTY>` | C-1 preserve evidence (edge-local journals BEFORE any re-image — see §6 edge rule) → C-2 revoke the edge identity per P9.09 → C-3 the site reverts to LOCAL control (no central supervision at a compromised edge) → C-4 re-enroll per P9.33 |
| I-5 | Telemetry integrity failure (malicious or incorrect telemetry) | Quality vocabulary (§10.4 six states; exactly one per value); stale/missing handling (P9.18 verification; §17.5 invariant 9 — a missing/uncertain/stale model is not a passing feasibility check); `telemetry_quarantine` (migration 0010) | Pilot operator | Operator → approver → security owner (if malicious) or platform engineering (if incorrect); corrections are NEW rows (`corrected_by`, migration 0012 — never edits) | `<EMPTY>` | C-1 preserve evidence → C-2 quarantine/flag the source per the quality policy → C-3 any schedule/command built on the affected feed is held or fails validation — it never issues with a note |
| I-6 | Central outage during an active dispatch | Platform probes (`/health`, `/ready` — `apps/control-plane/src/probes.rs`) and the edge's audited degraded-mode entry/exit (`DegradedBufferingEntered`/`DegradedBufferingExited` — `crates/edge-agent/src/runner.rs`) | Pilot operator (customer-side observes site-local behaviour) | Operator → approver → platform engineering → customer contact `<EMPTY>`; recovery per P9.24's DR procedure | `<EMPTY>` | C-1 confirm site-local fallback per the P9.20 register (the DEFAULT entry for an asset with no named policy is require-operator-intervention — `fallback_policy.rs`; never a universal zero-setpoint) → C-2 no new commands while degraded (recovery requires a NEW command through the full chain — `RECOVERY_RULE_STATEMENT`) → C-3 on recovery, verify the edge buffers drained in order with counted losses (§6) |

Containment rules that hold for EVERY class: containment NEVER precedes
the §6 evidence-preservation step except where preserving evidence would
require leaving the attack path open longer (the responder records which
rule was applied and why); containment NEVER disables the `PolicySet`,
never adds a bypass action, and never widens control scope (P9.08 §6
invariants); a revert-to-shadow action that reverts the WRONG site is the
ledger's named failure mode — the record names the site ID and matches it
to the incident's site binding.

## 6. Evidence preservation (ledger step 3) — before any remediation

| Rule | Content (cited mechanics) |
|---|---|
| Order of operations | Evidence is preserved BEFORE remediation deletes or mutates it (ledger rule; P9.36 step 6 consumes this). The preserved set per incident: audit records (central + edge), telemetry snapshots around the window, command records and their transitions, the site's mode record |
| Central audit evidence | `audit_records` is append-only by design (migration 0008: UPDATE/DELETE refused by trigger, no DELETE grant to ANY role; corrections are new rows with `corrected_audit_id`) — preservation = export + integrity anchor, not "protect from edits" |
| Edge audit evidence | The edge journal is bounded with audited drop-of-OLDEST overflow (`FileAuditBuffer::dropped_oldest`, `has_overflowed` latch — `audit_buffer.rs`): preservation means EXPORT PROMPTLY — the buffer is a loss-window, not an archive. Verify the exported chain against an anchor (`audit_proof.rs` :: `compute_chain_anchor` pre-incident, `verify_against_anchor` post-incident, `recompute_chain_head` for the raw chain) |
| Edge-local journals on a compromised edge (I-4) | Copy the journals (`FLEXCORE-EDGE-TELEMETRY-JOURNAL`, `FLEXCORE-EDGE-AUDIT-JOURNAL` — CRC-32 framed, `buffer_file.rs`/`audit_buffer.rs`) BEFORE re-imaging or re-enrollment; the CRC framing and the strict-decode rule make a copied journal verifiable offline |
| No remediation deletion | No step of this procedure deletes audit evidence; deletion of ANY record follows `10_data_residency_retention.md` §10.B (human-gated, reviewed migration or access-withdrawal — never a runtime delete, which does not exist) |
| Preservation record | Per incident: what was preserved, by whom, when, against which anchor — `<EMPTY>` until an incident occurs |

## 7. Communication templates (skeletons — the fields are the content)

Skeletons only; the bracketed fields are EMPTY until the customer contract
and rota exist. No template pre-writes an incident narrative.

**7.1 Initial notification (SEV-1/SEV-2, per the class deadline):**

```text
Subject: [FlexCore pilot] Incident <ID> — <class I-n> — SEV-m — initial notice
Incident ID / declared at (UTC): <EMPTY>
Class + one-line factual statement (what was OBSERVED, no speculation): <EMPTY>
Sites/assets affected (named IDs): <EMPTY>
Containment status (action taken, mode record reference): <EMPTY>
Evidence preserved (anchor references): <EMPTY>
Next update at (UTC): <EMPTY>
On-call contact for questions: <EMPTY — rota §8>
```

**7.2 Update notice (on any severity change or containment step):** same
fields, plus `Changed since last notice: <EMPTY>` and
`Severity re-assessment: <EMPTY>`.

**7.3 All-clear / close-out:** same fields, plus `Containment verified how
(mode record, anchor verification): <EMPTY>`, `Corrective actions with
owners/dates: <EMPTY>`, `Procedure corrections raised (§9.1): <EMPTY>`.

Template rules: notifications state OBSERVED FACTS with evidence
references — never speculation about cause before the review, never a
minimizing paraphrase, never a safety/certification assurance (AGENTS.md
§3). A notification that cannot fill the "sites affected" or "containment
status" fields factually is sent with those fields marked
`<under determination>` and the reason — never left silently blank and
never filled with guesses.

## 8. On-call rota (ledger step 5) — EMPTY; update procedure written

### 8.1 Rota — ALL CELLS EMPTY (UNSIGNED)

| Field | Value |
|---|---|
| Pilot period covered | `<EMPTY>` |
| First responder (pilot operator) — name, contact, window | `<EMPTY>` |
| Approver / escalation authority — name, contact | `<EMPTY>` |
| Security owner — name, contact | `<EMPTY>` |
| Platform engineering on-call — name, contact | `<EMPTY>` |
| Customer primary contact (SEV-1/2 notification) — name, contact, channel | `<EMPTY>` |
| Customer escalation contact — name, contact | `<EMPTY>` |
| Customer OT owner (I-4) — name, contact | `<EMPTY>` |
| Break-glass dual authorizers (P9.08 §6) — names | `<EMPTY>` |
| Backup per role (absence cover) | `<EMPTY>` |
| Rota version / effective date / signed by | `<EMPTY — UNSIGNED>` |

### 8.2 Rota update procedure

1. Any role change (person, contact, window) is recorded as a new rota
   version — never an in-place edit of a signed version.
2. The customer acknowledges the customer-side contacts in writing per
   version (their own notification path depends on it).
3. The active version is filed with the P9.23 evidence and referenced by
   every incident record ("rota version used").
4. The ledger's verification-method check — "verify the rota contacts by a
   test call or message where the customer permits it" — runs per version,
   with the result recorded; permission cell `<EMPTY>`.
5. A rota older than the pilot period with no successor version is an OPEN
   finding at the P9.28 review (criterion: "the on-call rota is current").

## 9. Tabletop and exercise records — UNHELD (human gate)

### 9.1 Tabletop (this subtask's own exercise — ledger step 4)

The tabletop walks at least one incident class — I-1 (suspected
forged/replayed command) — with the customer, executing the §5 I-1 row and
the §6 preservation rule on paper, and exercising at least one containment
decision (the ledger's verification method: "a containment decision
actually exercised"). **Status: `<EMPTY — UNHELD>`**; attendees,
corrections, and the corrected-procedure version all `<EMPTY>`.

Tabletop-then-live ordering (§3): corrections from this tabletop are
applied to this procedure BEFORE the P9.36 live drill is scheduled. The
P9.36 record reads the corrected procedure version.

### 9.2 The P9.36 live drill record

NOT here — P9.36 owns its own record (`36_forged_command_drill.md` §5,
EMPTY). This record only supplies: the procedure version to execute, the
I-1 deadline cell, the escalation path, and the containment steps. The
drill's outcome feeds corrections BACK into this procedure (P9.36
acceptance (e) — "procedure corrections are applied").

## 10. Reviewer-verification procedure (human)

1. Confirm §1's plain statement and that every contact/deadline/rota/
   exercise cell is `<EMPTY>`/PLACEHOLDER/UNHELD/UNSIGNED — no filled
   contact, no invented deadline, no fabricated attendee.
2. Confirm every notification deadline in §5 is explicitly a
   customer-contract cell, and that no severity level implies a response
   time (§4).
3. Trace §2's citations (`grep -n "<symbol>" <file>`): the `Role` variants,
   the `PilotAttackCase` set, the `EDGE_*` event names, the audit_proof
   functions, migration 0008's append-only properties, the drop-of-oldest
   counters, `DegradedBufferingEntered`, and the `DEFAULT_ENTRY_STATEMENT`
   fallback rule.
4. Confirm §3's closure statement: the procedure-existence half of the
   P9.36 dependency is closed, the tabletop half is not, and the
   tabletop-then-live ordering is intact.
5. Confirm containment invariants: no step disables the `PolicySet`, adds
   a bypass, widens control scope, or promises automated containment.
6. Confirm the templates (§7) carry no pre-written narrative and no
   assurance language.
7. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the procedure is written; no tabletop, rota, or incident
   response has happened.

## 11. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) With the customer: fill §5's contact + notification-deadline cells
#     ONLY from the customer contract / escalation sheet (cited by clause);
#     a deadline with no contract citation cannot be entered.
# (c) Fill §8.1 (rota) with named people; sign it; run the §8.2 per-version
#     contact test where the customer permits; record the version.
# (d) HOLD the §9.1 tabletop (I-1) with the customer; exercise a containment
#     decision; apply corrections; bump the procedure version; file the
#     tabletop record as P9.23 evidence named in CHECKLIST.md
#     ("Incident response ready").
# (e) Only then: schedule P9.36 per its §1 rule (all four preconditions);
#     the drill reads the corrected procedure version and the I-1 deadline.
# (f) On any real incident: follow this procedure AS WRITTEN at its then-
#     current version; file the incident record with the §6 preservation
#     evidence and the §7 notifications actually sent.
```

## 12. Open risks carried to P9.27 (this subtask's contribution)

| # | Risk | Basis |
|---|---|---|
| R-P9.23-1 | No customer contact, contract, or notification deadline exists — every §5 deadline cell is EMPTY, so no incident class currently has a measurable notification path | §1, §5; ledger P9.23 risk row ("a notification deadline in the contract that the procedure does not meet") |
| R-P9.23-2 | The alert leg of I-1/I-6 detection depends on P9.25-verified alerting (not yet built) — until then, detection rests on human inspection of audit lines, which does not bound detection time | §5 I-1/I-6; `36_forged_command_drill.md` R-P9.36-2 |
| R-P9.23-3 | The tabletop is UNHELD — the procedure is unexercised; P9.36 stays UNSCHEDULABLE and a schedule slip here directly slips P9.36 and P9.28 | §3, §9.1 |
| R-P9.23-4 | The edge audit buffer is a bounded loss window (audited drop-of-oldest), not an archive — evidence not exported promptly can be counted-lost; whether that window is acceptable is a customer/authority decision | §6; `audit_buffer.rs`; P9.20 R-6 |
| R-P9.23-5 | An incident class with no detection source would only be found by the customer — the I-2/I-3 detection sources are audit-record inspection, which bounds nothing until P9.25 wires alerts | §5 I-2/I-3; ledger risk row |
| R-P9.23-6 | Evidence destroyed by a well-meaning remediation — mitigated by the §6 order-of-operations rule, enforced by people at incident time (no code enforces it) | §6; ledger risk row |
| R-P9.23-7 | Break-glass expiry/revocation is procedural (no automatic revocation wiring — P9.08 §6 step 4); an incident-time elevation could outlive its box without the review catching it | §2; P9.08 §6 |
| R-P9.23-8 | The FNV-1a-64 audit chain and keyed-checksum command signature are tamper-EVIDENCE-grade, not cryptographic proof — incident evidence integrity rests on the recorded deployment replacements (P9.36 §6 bound, inherited here) | §2, §6; P9.36 §9 |

Status: **WRITTEN-UNRUN** — a procedure cannot be "run" by writing it; the
tabletop, the rota, the contact/deadline fills, and any real incident
response are the human gates and have not happened and are not scheduled.
No incident occurred; none was handled; none is claimed.
