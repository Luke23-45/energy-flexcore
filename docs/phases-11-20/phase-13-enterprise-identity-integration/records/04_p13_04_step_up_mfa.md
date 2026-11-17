# P13.04 — MFA/step-up enforcement (the server-side challenge protocol; LR-048 closure)

| Field | Value |
|---|---|
| Subtask | P13.04 — server-side step-up challenge gating the APPROVED gate (P4.13) and high-impact console actions; replaces the UI-only `console/src/components/stepUpApproval.tsx` flow with an enforced protocol; failure modes are recorded denials, never a deadlocked approval gate; registered as **P13-E-3** |
| Wave / agent | Phase 13 wave 1, Agent A |
| Status | WRITTEN-UNRUN + DESIGN: the challenge protocol is implemented code (`apps/control-plane/src/stepup.rs`) that has never been compiled or exercised; no challenge has ever been issued or verified; no MFA factor is enrolled anywhere; no IdP exists (P13-E-3 OPEN). Every operational evidence cell is EMPTY/UNRUN. |
| Sources | LR-048 (`ledger/phase-09-pilot-qualification/27_limitations_residual_risk.md` §3.8: "No step-up/MFA enforcement mechanism in code (gap G1)"); `console/src/components/stepUpApproval.tsx` (the UI-only flow this protocol replaces — its honesty record is the baseline); `crates/flexcore-application/src/command_gates.rs` (the P4.13 gate this composes with, from outside); plan §2.2/§6 risk 3 |
| Files touched | `apps/control-plane/src/stepup.rs` (NEW), `apps/control-plane/src/lib.rs` (additive module line, marked), `apps/control-plane/src/state.rs` (additive optional `step_up` field + `with_step_up` builder, fail-closed default) |

## 1. What LR-048 said and what closes it

LR-048 records gap G1: no step-up/MFA enforcement mechanism exists in
code. The P5.26 component (`stepUpApproval.tsx`) records honestly that
its satisfied step-up is "a LOCAL UI event with a local timestamp — NOT
a server-verified step-up"; no step-up endpoint existed and none was
invented. This subtask closes LR-048 **as designed and
implemented-written-unrun**: the enforced protocol exists as code (§3);
the console wiring is P13.17's (wave 2). It does NOT claim an MFA
enrollment, a verification run, or a working IdP integration.

## 2. The enforced protocol (design summary; the code is the detail)

| Phase | Mechanism | Enforced property |
|---|---|---|
| ISSUE | `StepUpService::issue(subject, tenant, purpose)` → a challenge bound to the (subject, tenant, purpose) triple | a challenge grants NOTHING; it is a single-use, time-bounded (DEFAULT TTL 5 min — REGISTERED RECOMMENDATION) request for fresh authentication |
| VERIFY | the principal presents FRESH IdP authentication evidence; the injected `StepUpVerifier` judges it | binding enforcement BEFORE the verifier runs: wrong subject / tenant / purpose / expired ⇒ named `StepUpDenial`; the challenge is consumed at verify entry — a REPLAY denies (single-use) |
| CONSUME | a verified challenge yields single-use `StepUpEvidence` bound to the same triple, expiring with the challenge window | the evidence reference is what a gated operation records (e.g. the approval-workflow engine's `step_up_evidence`) — the evidence never outlives its challenge |

The closed purpose vocabulary today: `CommandApproval { command_id }`
(the APPROVED-gate input — an evidence token for command A can never
approve command B) and `ConsoleAction { action_id }` (the registered
high-impact console actions; P13.17 wires them). Extension = a new
variant (the closed-vocabulary rule), never a free-form string.

## 3. Gating the APPROVED gate — from outside, with the no-wedge rule

The composition with the P4.13 gate (record 06 §4 carries the engine
detail):

1. A step of the multi-step policy may declare `requires_step_up: true`.
2. The composition root verifies the challenge at the P13.04 boundary
   and passes the evidence REFERENCE with the approval input.
3. The engine refuses a step whose required evidence is absent (a named
   denial); the P4.13 gate itself is untouched and still runs all of its
   own checks on top.

**The no-wedge guarantee (plan risk 3), structural:** a FAILED, MISSING,
EXPIRED, or REPLAYED challenge is a `StepUpDenial` — a recorded denial
of the attempt. The module holds NO reference to the command lifecycle:
it cannot move a command, and a failed challenge leaves the command
FEASIBLE and unapproved, exactly as in the no-step-up world. There is no
state anywhere in the protocol that a command can get stuck behind.

## 4. The UI-only flow's fate

`stepUpApproval.tsx` is wave-2 territory (not edited in wave 1). The
P13.17 design will re-wire the console's step-up UX onto this protocol;
until that lands, the UI-only flow remains what its honesty record says
it is, and the enforced protocol remains unconsumed-but-present. The
console file itself is NOT edited in this wave.

## 5. P13-E-3 — the MFA mechanism (registered OPEN)

| Element | Content |
|---|---|
| Question | Which MFA mechanism does the enforced step-up consume? |
| Options | TOTP / WebAuthn / passkeys / IdP-delegated MFA |
| DEFAULT / RECOMMENDATION | **IdP-delegated MFA as primary** (the IdP owns the authenticator; FlexCore consumes the IdP's authentication result through the `StepUpVerifier` port), TOTP as the documented fallback (its own reviewed secret storage per the P11.16 posture), WebAuthn/passkeys noted as the forward path |
| Closer | human owner |
| Status | **OPEN — never closed by this record or any agent.** The code implements the DEFAULT's SEAM only (the verifier port + the fail-closed `DenyAllStepUpVerifier`); no verifier backend exists, no TOTP implementation was built, no factor was enrolled |

## 6. What is in state.rs (the fail-closed composition seam)

`AppState` gains an optional `step_up: Option<Arc<StepUpService>>`
(default `None`) and a `with_step_up` builder. `None` means every
step-up request fails closed (a recorded denial) — the same discipline
as the `edge_enrollment` seam. NO composition exists in this repo (no
IdP is deployed); `main.rs` is untouched.

## 7. Tests (written, never run)

`stepup.rs` `#[cfg(test)]`: issue→verify yields triple-bound
single-use evidence; replay denies; expiry is a named denial at the
exclusive window close; missing challenge denies; subject/tenant/purpose
binding enforced (including command-A-vs-command-B); rejected evidence
consumes the challenge (a fresh challenge is the retry path); the
shipped default verifier denies everything; the denial set renders
readable audit text; empty subject never issues. All UNRUN — execution
is the human gate; P13.19 adds the expired/replayed/failed negative
matrix at integration level.

## 8. ASSUMPTION / UNRUN register

1. **UNRUN** — no challenge issued, no evidence verified, no MFA factor
   enrolled, no IdP interaction, no TTL measured. Evidence cells EMPTY.
2. **ASSUMPTION** — the 5-minute TTL is a pre-registered recommendation
   (X-8), not a tested optimum.
3. **ASSUMPTION** — the IdP-delegated verification description is the
   P13-E-3 DEFAULT's design intent; the actual verifier behavior is
   whatever the deployment composes, and nothing about it is claimed.
4. **OPEN** — P13-E-3 (§5). The shipped default denies; until the human
   owner composes a verifier and signs off, NO step-up can ever succeed
   anywhere — which is the honest state for a platform with no IdP.
5. **RECORDED LIMITATION** — the in-memory challenge store is for tests
   and single-process deployments; the durable store (Postgres under
   RLS) is a composition-root artifact named in the module docs, not
   built (no migration was added).

## 9. Handoff notes

1. **P13.17 (wave 2)**: wire the console step-up UX to the protocol; the
   UI-only `stepUpApproval.tsx` honesty record is then superseded BY THE
   CONSUMER, not by this record.
2. **P13.19 (wave 2)**: the expired/replayed/failed negative matrix.
3. **Agent B**: no notification/webhook surface consumes step-up; the
   protocol's audit output is denial/evidence records like everything
   else.
