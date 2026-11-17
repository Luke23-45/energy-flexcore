# P13.06 — Multi-step approval workflow engine (extends the P4.13 gate from OUTSIDE; the frozen 27-edge set untouched)

| Field | Value |
|---|---|
| Subtask | P13.06 — sequential approvals, delegation (scoped, time-bounded, audited), absence policy, quorum for high-impact commands; extends the P4.13 authorization gate from outside WITHOUT touching the frozen 14-state/27-edge transition set; approval records stay `record_command_approval`-shaped; implemented written-unrun with complete negative tests |
| Wave / agent | Phase 13 wave 1, Agent A |
| Status | WRITTEN-UNRUN: the engine and its test module are appended to `crates/flexcore-application/src/command_gates.rs`; nothing compiled, no workflow executed, no approval recorded anywhere. Every operational evidence cell is EMPTY/UNRUN. |
| Sources | plan §2.2 (the approval-gate extension rule) and §6 risk 3; `crates/flexcore-domain/src/command_state.rs` (the frozen set — consumed, never edited); `crates/flexcore-application/src/command_gates.rs` (the P4.13 gate — consumed via `record_command_approval`, never wrapped) |
| Files touched | `crates/flexcore-application/src/command_gates.rs` (ADDITIVE section appended after the existing tests: the engine, its types, and its own test module; the pre-existing 2 048 lines are untouched) |

## 1. The extension rule, enforced by construction

| Rule (plan §2.2) | How the code enforces it |
|---|---|
| No new states, no new edges | the engine contains NO `CommandLifecycle`, NO `CommandTransition`, and performs NO state change — its only effects are records in the injected `ApprovalAccumulationStore`. The 14 states and 27 edges of `flexcore_domain::command_state` are byte-identical after Phase 13 (the file was not opened for edit) |
| Distinct gates preserved | validation, feasibility, and authorization stay three distinct gates; the multi-step approval is ONE authorization gate with multiple human inputs — the engine decides only WHEN the composition root may call the existing `CommandGates::record_command_approval`, which remains the single operation that can produce APPROVED and which still runs ALL of its own checks (policy allow, scope coverage, separation of duties, expiry) on top |
| Step-up composition | a step may declare `requires_step_up: true`; the composition root verifies at the P13.04 boundary and passes the evidence reference; a step without required evidence is a named denial (`StepUpEvidenceMissing`) — a failed challenge is a recorded denial, never a partial approval |
| Delegation is scoped | a delegation carries a scope subset (checked to COVER the command), a time window (checked ACTIVE at record time; an expired delegation is a named denial, never a grace extension), and both identities travel in the accumulated record (delegator + delegate) — audited |

## 2. The engine surface (what was implemented)

| Piece | Behavior |
|---|---|
| `ApprovalWorkflowPolicy` | the deployment's policy: ordered `steps` (role + coverage + step-up requirement), optional `ApprovalQuorumRule` (N DISTINCT approvers from eligible roles), `AbsencePolicy` (DEFAULT: delegation DISABLED). Empty policy = the plain P4.13 gate (the engine is not engaged) |
| `AccumulatedApproval` | one human input, `record_command_approval`-shaped (subject, role, `ApprovalScope`, decided-at, step-up evidence, delegation reference, satisfied step) — many accumulate per command; none transitions anything |
| `ApprovalWorkflowEngine::record_step` | validates (non-empty subject; one approver one record; the first UNSATISFIED step's role must match — out-of-order inputs deny with `NoMatchingStep`; scope must COVER the command; required step-up evidence present; delegation policy/registration/activity/scope when presented) and accumulates; returns the verdict |
| `ApprovalWorkflowEngine::evaluate` | `Satisfied { satisfied_steps }` when every sequential step matched AND (when configured) the quorum of DISTINCT in-scope eligible approvers is met; otherwise `Pending { missing_steps, distinct_approvers, quorum_minimum }`. `Satisfied` is permission for the composition root to call the P4.13 gate — it makes approval HARDER, never easier |
| `register_delegation` | the audited registration act: non-empty distinct identities, window not in the past, non-empty window; re-validated for activity at record time |
| `ApprovalAccumulationStore` | the injected seam (append/read approvals, register/look-up delegations); the composition root implements it over persistence — none exists in this repo (no store was added) |

## 3. The negative-test matrix (written complete, UNRUN)

`mod approval_workflow_tests`: empty policy satisfied immediately; two
sequential steps accumulate and fire only when satisfied; out-of-order
and unqualified roles deny (`NoMatchingStep`); duplicate approver denies;
a scope that does not cover the command denies (`ScopeDoesNotCoverCommand`,
including an asset-scoped record over a different asset);
`requires_step_up` step without evidence denies and accepts with the
reference; a satisfied policy refuses further records
(`PolicyAlreadySatisfied`); delegation denied when the absence policy is
off, on self-delegation, with no registration, with a scope not covering
the command, and when expired (exclusive window close — no grace);
delegated records carry the delegator (both identities audited); quorum
counts distinct eligible in-scope approvers and ignores ineligible/out-
of-scope records; empty subject denies; delegation registration
validates window and identity. Every denial path asserts the command
world is unchanged — the engine cannot move a command.

## 4. Composition with the P4.13 gate and P13.04 (the sequence)

```text
policy steps accumulate (engine) ──Satisfied──▶ composition root calls
CommandGates::record_command_approval (the SAME single gate: policy
allow, scope coverage, separation of duties, expiry all still apply) ──▶
the FEASIBLE → APPROVED edge fires exactly as before, through the frozen
table. Any earlier denial anywhere leaves the command FEASIBLE.
```

## 5. ASSUMPTION / UNRUN register

1. **UNRUN** — nothing compiled; no workflow configured, executed, or
   audited. Evidence cells EMPTY.
2. **BYTE-IDENTICAL claims** — the pre-existing content of
   `command_gates.rs` (through the original tests module, 2 048 lines)
   is untouched (append-only edit); `command_state.rs` was not opened
   for edit at all. Both verified by the file-length check in the
   close-out record (3324 lines = 2048 + the appended block).
3. **RECORDED DEFAULT, reviewable** — `AbsencePolicy` DEFAULT is
   delegation DISABLED; an empty policy engages nothing. Multi-step
   policies are per-deployment data, never compiled defaults.
4. **RECORDED LIMITATION** — the coverage rule for a step is the
   existing `ApprovalScope::covers` superset rule (Tenant ⊇ site ⊇
   asset); the engine does not add a finer coverage predicate. The
   P13.19 tests extend the matrix (plan §2.1: wave 2).

## 6. Handoff notes

1. **P13.19 (wave 2)**: extends this matrix; the engine's tests here are
   the in-crate core (conventions §1 item 3 — execution is the human
   gate).
2. **Composition root**: wiring the engine is a deployment act — inject
   a durable `ApprovalAccumulationStore` and the per-class
   `ApprovalWorkflowPolicy`; nothing in this repo composes one.
3. **The frozen-set audit (P13.22)**: `command_state.rs` and the
   never-granted set are the two byte-identical invariants this wave
   claims; both are re-asserted in the close-out record.
