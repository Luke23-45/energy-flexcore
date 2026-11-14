# ENTRY NOTE — Phase 9: Pilot Qualification

Date: 2026-11-14. Coordinator: implementation-coordinator. Loop rules in
force: nothing installed, nothing built, no tests run, no CI run — complete
code + test files written only. Ticks are "implemented, awaiting human
build/test". Decision records per batch in this directory.

## Gate exception (same pattern as Phases 1–8)

Stages 1–8 exit gates are **NOT HELD** (recorded per phase: v0.4.0
position in `ledger/phase-06-edge-protocol/30_retrospective_release_gate.md`,
v0.5.0 in `ledger/phase-07-plugin-sdk/30_retrospective_release_gate.md`,
Stage 8 in `ledger/phase-08-advanced-coordination/30_retrospective_exit_gate.md`).
Phase 9 starts on the recorded exceptions; P9.01 records this honestly.

## Phase-9-specific boundaries

1. **A pilot cannot be executed by agents.** Site selection, commissioning,
   HIL runs, field acceptance tests, drills, operator training, acceptance
   reviews, and authority engagements are HUMAN/ORGANIZATION gate inputs.
   For every such subtask the deliverable is the PLAN / RECORD / TEMPLATE
   with EMPTY execution cells, exact filling commands, and honest
   UNRUN/UNSIGNED/NOT-SCHEDULED labels. Never fabricate a site, a customer,
   a date, a measurement, or a sign-off.
2. **D1–D6 remain agent-undecided.** Where a subtask says a decision
   "closes" (e.g. P9.06 "D6 closure"), the agent records the REVIEW, the
   recommendation, and DEFAULT/RECOMMENDATION labels; the closure itself is
   a human decision recorded as OPEN until signed. D1 (market/program),
   D3 (unbalanced), D5 (customer integration), D6 (OT/IT boundary) —
   same discipline as `18_21_audit_offline_isolation.md` and prior phases.
3. **The pilot is operator-approved control AT MOST (v1.0.0 ladder).**
   Bounded automation is post-1.0. Nothing in Phase 9 grants field write
   authority; the control-scope document (P9.26) is a design with the
   operator-approval gate recorded, not an approval.
4. **No safety/certification claims; no protection-grade claims** (AGENTS.md
   §3). FlexCore is supervisory orchestration only.
5. **The edge-security block (P9.31–P9.36) is real code + tests** extending
   `crates/edge-agent` (and only where the ledger dictates), mirroring the
   Phase 6 discipline (P4.28/P6.15 four-check pattern, CRC-32 framing,
   named errors, injected clock). Tests WRITTEN-UNRUN.
6. **Every execution-type subtask (P9.14 lab test, P9.15 HIL, P9.17 field
   acceptance, P9.19 dry run, P9.20 drills, P9.24 DR rehearsal, P9.36 live
   drill, P9.37 conformance execution, P9.38 shadow replay, P9.39 perf/load,
   P9.41 regression re-run) = TEMPLATE + procedure; the run is the human
   gate.** Deliverables state plainly what the finite tests would and would
   not establish.
7. Component locations per PLAN.md §3 and the Phase 6/7/8 seam handovers;
   new crates require additive workspace-member + check_dependency_rule.py
   updates; contract gaps are Phase 7 follow-ups, never parallel contracts.

## Wave plan (max 2 subagents; host caps concurrency at 2; disjoint file ownership)

P9.01 (entry) blocks P9.02–P9.30 → wave 1 pairs it with the independent
edge-security block. P9.30 (exit-gate record) is the coordinator close.

| Wave | Subtasks | Files owned | Notes |
|---|---|---|---|
| 0 (coordinator) | — | this file, HANDOVER.md | Gate exception + boundaries |
| 1 | P9.01, P9.02, P9.03 ∥ P9.31, P9.32 | records ∥ edge-agent code | P9.01 entry freeze; edge rejection + audit proof |
| 2 | P9.04–P9.06 ∥ P9.33, P9.34 | records (D5 review; D6 review/closure-record) ∥ edge enrollment + signed update code | |
| 3 | P9.07–P9.09 ∥ P9.35, P9.36 | security records ∥ invariant map + drill template | |
| 4 | P9.10–P9.12 ∥ P9.37, P9.38 | data/market/authority records ∥ conformance + replay templates | |
| 5 | P9.13–P9.15 ∥ P9.39, P9.40 | commissioning/HIL/field-acceptance plans ∥ perf/load template + traceability map | |
| 6 | P9.16–P9.21 ∥ P9.41 + P9.22 | remaining pilot-activity plans ∥ regression template + market procedures | balanced at dispatch |
| 7 | P9.23–P9.25 ∥ P9.26–P9.27 | incident/DR/observability ∥ control-scope + limitations register | |
| 8 | P9.28, P9.29 | acceptance review + approval pack records | |
| 9 (coordinator) | P9.30 | retrospective + exit-gate record + CHECKLIST + HANDOVER | All P9.01–P9.41 |
