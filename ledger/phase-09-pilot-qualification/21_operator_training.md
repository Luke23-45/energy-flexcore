<!-- P9.21 — Operator training and role qualification (decision record). DESIGN ONLY: the curriculum outline, assessment design, and register shapes are written; NO trainee exists (no person is assigned to any role — P9.08 §1), no training was delivered, no assessment was taken, no qualification was granted, and the qualification register is EMPTY. The qualification-before-approval rule is PROCEDURAL until implemented (08_...md §7: no code reads a qualification register). Training delivery and assessment are human gates (ENTRY_NOTE.md boundary 1). -->
# P9.21 — Operator Training and Role Qualification (Design — register EMPTY, no trainee exists)

Date: 2026-11-14. Subtask: P9.21 (`ledger_phase_9.md` §P9.21). Owner
role: Product / operations (with the customer operator lead). Report
trace: §23 Stage 9; §20 (operator application design); §17.3. Inputs
per the ledger: P9.08 role matrix (DESIGN record —
`08_identity_access_approval.md`; no person holds any role), P9.17
shadow operation (UNCHECKED), the pilot operator console (§20.1
workspaces — not deployed). None of the inputs is satisfied; this
record exists so the training the human gate runs is designed before
any person is put in front of the pilot console.

## 1. Honest status — the plain statement

**No trainee exists, no training was delivered, no practical assessment
was taken or passed, and the qualification register is EMPTY.** There
are no pilot users at all: `08_identity_access_approval.md` §1 records
that "no role is assigned to any person" and §3.1 leaves every
assignment cell `<EMPTY>`. Every attendance, assessment, and
qualification cell in §4–§5 is deliberately EMPTY (`(EMPTY — UNRUN)` /
UNSIGNED). What this record delivers is the DESIGN: the per-role
curriculum outline (§3), the practical-assessment design (§4), the
register shapes (§5), and the honest statement of where the
qualification-before-approval rule is enforced today — procedurally
nowhere in code (§6).

## 2. Design basis — the P9.08 roles and the authorization vocabulary (cited)

The roles to train are the P9.08 design roles, mapped BY CITATION to
the existing `flexcore-authz` closed vocabulary
(`08_identity_access_approval.md` §3; `crates/flexcore-authz/src/policy.rs`):

| Pilot role (P9.08 design) | flexcore-authz `Role` | Approval rights | Training consequence |
|---|---|---|---|
| Pilot operator | `Operator` | **NO `command.approve`** (the recorded P4.13 default; proposes + validates + reads only) | Trains proposal/validation workflow; explicitly trained that approval is not theirs to attempt (the negative test N1 in P9.08 §9) |
| Approver | `TenantAdmin` | `command.approve` at **site/asset scope ONLY** — no tenant-level approval grant exists in the recorded default (`phase4_policies.rs`) | Trains the approval gate in depth; proposer ≠ approver (P9.08 §4.1) is trained as a rule, and the code gate refuses the conflict regardless |
| Read-only observer | `Viewer` | Read only | Trains observation/dashboards only |
| Tenant administrator | `TenantAdmin` (shared role, P9.08 §3.2 rule 1) | Config/registry — **NOT a dispatch approver by design** unless assigned the approver function | Trains registry/config lifecycle |
| Site technician | NOT on the authorization surface (P9.08 gap G4) | None — acts in the customer's OT identity system | Out of FlexCore training scope; customer OT responsibility |

The command-state semantics the curriculum teaches are the report §14.1
gate rules verbatim: "Validation, feasibility, and authorization are
distinct. Network feasibility is not operator authorization. An
acknowledgement confirms receipt/acceptance, not physical delivery."

## 3. Curriculum outline per role

Modules are outlines, not delivered material — every material-version
cell is EMPTY until written and versioned (ledger step 2 requires the
material version recorded per session).

### 3.1 All roles (core module)

| Module | Content | Source |
|---|---|---|
| C1. System orientation | What FlexCore is and is not: supervisory orchestration; no protection, primary frequency response, synthetic inertia, hard real-time (AGENTS.md §3; §0.3, §14.5) | Report |
| C2. Control-authority modes | Simulation / read-only+shadow / operator-approved / bounded automation; mode is a deployment and per-site setting no plugin can select; the pilot ceiling (§7 below) | §21.4; AGENTS.md §4 |
| C3. Telemetry quality and freshness | The six canonical states (`GOOD` `SUSPECT` `BAD` `MISSING` `ESTIMATED` `STALE`) and what each permits an operator to conclude; freshness limits per use case | §10.3–§10.4; P9.18 outputs |
| C4. The command lifecycle | Every §14.1 state and what each MEANS operationally: distinguishing proposed/feasible/approved/issued/acknowledged/measured states (§20.2); APPROVED ≠ issued; acknowledgement ≠ delivery | §14.1–§14.2; P9.19 outputs |
| C5. Audit and evidence | What the journal records per transition; how to read an authorization decision's evidence (scope, limits, expiry — P9.08 §4.3) | §14.3; §10.1 |

### 3.2 Role-specific modules

| Role | Modules |
|---|---|
| Pilot operator | O1. Console workspaces (§20.1) for proposal and monitoring; O2. Creating/validating a command from a schedule; O3. Reading rejections — every rejection names its failing check (P4.11 discipline); O4. Alarm handling (read surface — the authz gap G3 means NO acknowledgement action exists; the procedure routes acknowledgement outside FlexCore until G3 closes); O5. Escalation to the approver |
| Approver | A1. The approval gate's four conditions in code (allow decision at recorded scope; scope covers the asset; proposer ≠ approver; expiry in the future — `record_command_approval`, P4.13); A2. Reading validation + feasibility evidence BEFORE approving; A3. Scope discipline: site/asset-scope approval only; A4. Rejecting with a stated reason (ledger step 3); A5. Expiry and supersession: when an approval lapses |
| Read-only observer | V1. Console read surfaces; V2. Reading health and degraded states (§18.3 operator-visible health) |
| Tenant administrator | T1. Registry/config lifecycle; T2. Why tenant-admin is not an approver function by design (P9.08 §3.2); T3. The IdP-unavailable fail-closed posture and the break-glass procedure DESIGN (P9.08 §6 — used only under its dual-authorization procedure) |
| Everyone | E1. Emergency and rollback actions: command cancellation (APPROVED/ISSUED → CANCELLED, actor + reason mandatory — P4.15), reversion of a site to shadow mode (the P9.17 reversion rule), and the local-EMS/fallback posture (P9.20 §4 register) — EXERCISED, not just read (ledger acceptance c) |

### 3.3 Training material rules

- Training happens on the PILOT system with PILOT data (ledger risk:
  "training on a demo system that differs from the pilot build") — the
  material and the environment versions are recorded per session.
- The §14.2 state definitions table is taught verbatim; no paraphrase
  that blurs acknowledgement vs delivery is acceptable.
- The operator reference (ledger step 5) — the P9.22 market/product
  procedures and the P9.23 incident procedure in the form the operator
  actually uses — is assembled and versioned when those records exist;
  until then the cell stays EMPTY.

## 4. Practical assessment design (per ledger step 3; results EMPTY)

Each operator and approver is assessed individually on the pilot
system. The four ledger-required exercises, plus the passing rule:

| # | Exercise | Pass rule | Assessor | Result per person |
|---|---|---|---|---|
| S-1 | Approve a valid command | The command reaches APPROVED with a complete `AuthorizationDecisionRecord`; the approver correctly reads the validation/feasibility evidence first; approver ≠ proposer | Named assessor (not the trainee) | (EMPTY — UNRUN) |
| S-2 | Reject an infeasible command WITH its stated reason | The rejection is recorded with the named failing check; the trainee's stated reason matches the journal evidence | Named assessor | (EMPTY — UNRUN) |
| S-3 | Recognise a stale-telemetry state | The trainee identifies the `STALE`/`SUSPECT` state from the console and states what it forbids (no schedule/command built on it — §17.5 invariant 9 posture) | Named assessor | (EMPTY — UNRUN) |
| S-4 | Execute the fallback/rollback procedure | The trainee executes cancellation/reversion per E1 from the written procedure; the exercised steps match the P9.20 §4 register for the asset | Named assessor + site operator | (EMPTY — UNRUN) |

An exercise is PASSED only when the evidence (audit extract or console
record) supports it — a trainer's impression is not evidence. A failed
exercise is retried after recorded remediation; it is never waived
(ledger acceptance (a) admits no waiver path for approval rights).

## 5. The qualification register (TEMPLATE — EMPTY; no trainees exist)

One row per person per role, filled only by the human gate. An
unqualified person holds NO approval right in the pilot configuration
(ledger step 4; P9.08 §7).

| Person (name) | Role(s) | Curriculum modules completed (material version) | S-1 | S-2 | S-3 | S-4 | Qualified (date) | Assessor |
|---|---|---|---|---|---|---|---|---|
| (EMPTY — no trainee exists) | (EMPTY) | (EMPTY) | (EMPTY — UNRUN) | (EMPTY — UNRUN) | (EMPTY — UNRUN) | (EMPTY — UNRUN) | (EMPTY) | (EMPTY) |

## 6. The qualification-before-approval enforcement point — stated honestly

**The rule is PROCEDURAL until implemented.** Verbatim from the P9.08
design record (§7): "an approval right is bound to qualification, not
to the role alone … the role-assignment step checks the qualification
register before granting `command.approve` to any principal; the
register itself is P9.21's deliverable … **the enforcement wiring is a
deployment composition item (honest gap: no code today reads a
qualification register — the rule is procedural until implemented)**."
The same record's negative test N6 ("unqualified principal: approve
with qualification register absent → refused per §7 rule") is recorded
UNRUN.

Consequences, stated plainly:

1. Until the wiring exists, enforcement is: the human role-assignment
   step at the IdP/application layer is performed ONLY after §5 shows a
   passed assessment for that person and role, verified by a second
   person.
2. The code-level backstop that DOES exist today: `command.approve` is
   `TenantAdmin`-only at site/asset scope, separation of duties is
   enforced in the P4.13 gate, and every authorization decision is
   audited with its evidence (P9.08 §4.3). These bound the damage of a
   procedural failure; they do not implement the qualification check.
3. The gap is carried to P9.27 (§8 R-2) and to the deployment
   composition workstream — it is not silently closed by this record.

## 7. The ceiling statement (verbatim)

Verbatim, the Phase 9 ledger scope note (`ledger_phase_9.md`): "This
phase qualifies **one named customer deployment at named sites** for
operator-approved control. It does not certify FlexCore, any adapter,
any inverter, or any site. It does not grant bounded automation:
bounded automation is post-1.0, per deployment, site, and asset class,
after separate qualification (§21.4, §25)."

Verbatim, `ENTRY_NOTE.md` boundary 3: "**The pilot is operator-approved
control AT MOST (v1.0.0 ladder).** Bounded automation is post-1.0.
Nothing in Phase 9 grants field write authority."

Operative meaning for training: operators are qualified to APPROVE
within the v1.0.0 operator-approved mode — nothing in the curriculum,
the assessment, or the register qualifies anyone (or any configuration)
for bounded automation, and no module teaches or assumes an
automation-envelope capability. If the customer requests automation
during the pilot, the answer is the ceiling statement, not a training
module.

## 8. Open risks carried to P9.27

| # | Risk | Basis |
|---|---|---|
| R-1 | Training on a system that differs from the pilot build — mitigated by recording the environment/material versions per session, but the discipline is human-enforced | §3.3 |
| R-2 | Qualification-before-approval is procedural: no code reads a qualification register; a mis-assigned approval right is caught only by human review and the audit trail | §6 (P9.08 §7, gap) |
| R-3 | An operator who can approve but cannot recognise a stale or estimated value (the ledger's named risk) — mitigated by S-3 being a hard requirement, but assessment quality depends on the named assessor | §4 |
| R-4 | Qualification recorded for a role the person does not hold — the register's role column must be cross-checked against the P9.08 role matrix and the live IdP mapping at fill time; the cross-check is a second-person step | §5; P9.08 §3 |
| R-5 | Alarm acknowledgement is impossible on the current authz surface (P9.08 gap G3): the operator procedure must route acknowledgement outside FlexCore until the closed-enum extension is reviewed and added — trained as such in O4, and operationally awkward until then | §3.2 O4 |
| R-6 | No trainee, training session, assessment, or qualification exists — every ledger acceptance criterion is UNMET and not claimed | §1, §4, §5 |

## 9. Reviewer-verification procedure and filling commands (human gate)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) PRECONDITIONS (ledger): P9.08 roles assigned to named people
#     (AFTER qualification per its §7 rule — assign provisionally for
#     training, confirm at its end); P9.17 shadow operating so training
#     uses live pilot data. Neither holds today.

# (c) WRITE + VERSION the §3 material per role; record the version per
#     session (ledger step 2).
# (d) DELIVER training on the pilot system; record attendance + material
#     version (§3.3).
# (e) RUN the §4 assessments S-1..S-4 per person; file the audit/console
#     evidence per pass; a fail is remediated and retried, never waived.
# (f) FILL the §5 register; a second person cross-checks it against the
#     P9.08 role matrix and the live configuration (ledger verification
#     method); assemble the §3.3 operator reference (P9.22/P9.23 forms).
# (g) FILE the register as the P9.21 evidence named in CHECKLIST.md
#     ("Operators qualified").
```

Reviewer checks: (1) every §4/§5 cell EMPTY/UNRUN in the committed
template; (2) §6 quotes the P9.08 gap verbatim and does not claim code
enforcement; (3) §7 carries both verbatim ceiling quotes unaltered; (4)
no module or exercise trains or implies a bounded-automation capability.

## 10. Verification status (honest)

| Item | Status |
|---|---|
| `python tools/verify_docs.py` | RUN in the writing loop (output quoted in the completion report) |
| `python tools/validate_agent.py` | RUN in the writing loop (output quoted in the completion report) |
| `python -I tools/check_dependency_rule.py` | RUN in the writing loop (no dependency edge added) |
| Training delivery / assessments / register (§4–§5) | **NOT RUN — banned in this loop; human-gate input; no trainee exists; all cells EMPTY** |

Status: **WRITTEN-UNRUN** — a design record cannot be "run"; training,
assessment, and qualification are the human gates and have not happened
and are not scheduled. No person was qualified; no pilot exists and
none is claimed.
