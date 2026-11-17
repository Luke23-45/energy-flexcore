# P12.08 — Game-day runbook template + quarterly schedule (human-gated)

| Field | Value |
|---|---|
| Subtask | P12.08 (game-day runbook template + proposed quarterly cadence; every game day human-gated) |
| Wave / agent | Phase 12 wave 1, Agent B |
| Status | **WRITTEN-UNRUN.** The template and the cadence proposal are written; no game day has been scheduled, staffed, prepared, executed, or reviewed anywhere in this program. Every template field is `<EMPTY>` and stays `<EMPTY>` until a scheduled game day fills it from a real run. |
| Sources | [S-82] (Gremlin — game-day practice: pre-registered hypotheses, bounded blast radius, named roles, abort criteria, evidence capture; no claim about FlexCore); the P12.07 experiment inventory (record 21) |
| Files touched | `docs/phases-11-20/phase-12-backup-dr-resilience/runbooks/game_day_template.md` (new) |

## 1. What was delivered

1. **The template** (`runbooks/game_day_template.md` §1): nine sections —
   human-gate statement, hypotheses under test (consumed verbatim from the
   P12.07 experiment scripts' pre-registrations, never re-typed or edited
   at the game day), blast-radius table, named roles (director / operator
   / observer / evidence keeper / optional witness — people, never
   teams-of-convenience), X-8 registered thresholds, abort criteria
   (copied verbatim from the experiment scripts; the director may ADD
   criteria, never remove one), a during-run timeline, evidence capture,
   and the post-run review whose zero-findings row is a TEST OUTCOME
   under §13.3 semantics.
2. **The quarterly cadence** (template §2): a four-session PROPOSAL —
   Q1 C-WORKER-1 + C-POOL-1 (lowest blast radius), Q2 C-NATS-1, Q3 C-PG-1
   (gated on a P12.03 restore drill having run at least once), Q4 a
   findings-chosen repeat plus the DR restore-into-isolation rehearsal
   (P12.11). Five cadence rules: human-gated end to end; never two
   datastore faults in one session; no re-run with an open finding; the
   template is the record (an unfilled template is a fabrication defect,
   the P9.24 §8 rule inherited); no live-traffic game days in the 0.x
   posture.

## 2. The human gate (why no cell can be filled by an agent)

Every game-day input is a human-gate input per conventions §5: the
approving operator, the named people in each role, the adopted schedule,
the registered thresholds, the run timeline, and the findings. This
subtask ships the SCAFFOLDING for those inputs — an agent filling any of
them would be fabricating evidence. The approval signature cell is
explicitly a human signature, recorded UNSIGNED.

## 3. Honest boundaries

- [S-82] is practice context, not a conformance source — no claim that
  adopting the template makes anything "resilient".
- The cadence is a RECOMMENDATION; adoption is an operator decision
  recorded by the operator.
- The template consumes the P12.07 experiments by their pre-registration
  blocks; if an experiment script's abort criteria change, the game-day
  copy is re-taken from the script (one source of truth — the script),
  never edited independently.
- ADR-010: nothing here adds dispatch authority; game days run
  simulation-only data in isolated labs.

## 4. Handoff notes

1. **Wave 2 / P12.13**: the template's §6 timeline and §7 evidence
   sections are the shape the backup/restore + DR evidence templates
   should mirror for EMPTY-cell discipline.
2. **Operator**: adopt or amend the cadence; the Q3 gate (restore drill
   before C-PG-1) is the one ordering constraint this program imposes on
   the schedule.
3. **Reconciliation (DONE at close)**: Agent A's P12.03 restore-drill
   record landed as `records/03_p12_03_restore_drill_runbook.md`; the Q4
   DR-rehearsal row cites A's template and EMPTY-cell discipline instead
   of duplicating it (sibling-citation pattern, plan §2.1).
