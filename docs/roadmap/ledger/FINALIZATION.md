# Finalization Record — FlexCore Roadmap Ledger (F-16 / F-17)

- **Status:** FINALIZED (plan only) — 2026-11-21; restored by the iteration-11 independent re-review (zero findings) after the suspension by the iteration-03 adversarial audit
- **Ledger version:** v1.2 (finalized after independent re-review), 2026-11-21
- **Scope:** 7 waves (Wave 0–6), 76 subtasks, 79 ledger files + 11
  review files under `docs/roadmap/ledger/`
- **What "finalized" means:** the ledger as a *plan* is written,
  independently reviewed (iteration 01: 20 findings, all patched),
  re-reviewed (iteration 02: zero findings remaining), then
  adversarially audited (iteration 03: 45 findings, all patched),
  re-reviewed (iteration 04: 12 findings, all patched), re-reviewed
  (iteration 05: 7 findings, all patched) and re-reviewed
  (iteration 06: 6 findings, all patched) and re-reviewed
  (iteration 07: 4 findings, all patched) and re-reviewed
  (iteration 08: 3 findings, all patched) and re-reviewed
  (iteration 09: 1 finding, patched) and re-reviewed
  (iteration 10: 1 finding, patched) and re-reviewed
  (iteration 11: zero findings).
  The FINALIZED stamp from iterations 01–02 was SUSPENDED by the
  iteration-03 adversarial audit and is RESTORED by the iteration-11
  independent re-review, which returned zero findings.
  "Finalized" describes the plan only — all 76 subtasks are unchecked
  and no implementation work is done. Do not read this record as a
  build sign-off, a review sign-off, or an authorization to build.

## Verified totals (re-counted by the drafting agent, then re-counted in the iteration-02 re-review — not trusted from the task list)

| Wave | Subtasks | Wave | Subtasks |
|---|---|---|---|
| 0 Gate 0 | 10 | 4 F-17 Phase 0 | 13 |
| 1 F-16 Phase 1 | 20 | 5 F-17 Phase 1 | 12 |
| 2 F-16 Phase 2 | 8 | 6 F-17 later phases | 8 |
| 3 F-16 Phase 3 | 5 | **Total** | **76** |

Counting method: "Acceptance criteria" lines across the wave files
(10/20/8/5/13/12/8 = 76); subtask heading IDs are unique and contiguous
1..N within every wave (76 unique IDs); 353 unchecked boxes and 0
checked boxes across the wave files; 0 orphan subtask cross-references
in Dependencies lines (every `W<w>.<nn>` mentioned is a defined
heading); file numbers match subtask numbers (01 = W<w>.01). File
count: 79 ledger files (MASTER_LEDGER.md, README.md, FINALIZATION.md,
76 subtask files) + 11 review files under `_iterations/` (iteration 01,
iteration 02, iteration 03, iteration 04, iteration 05, iteration 06,
iteration 07, iteration 08, iteration 09, iteration 10 and iteration
11 findings) = 90 `.md` files total in the ledger tree.

Actual recount (21 Nov 2026, run from the ledger directory, after the
iteration-11 zero-findings re-review):

```
$ grep -r -c "Acceptance criteria" wave-*/ | awk -F: '{s+=$2} END {print s}'
76
$ grep -r -h "^# W" wave-*/ | wc -l
76
$ grep -r -h "  - [ ]" wave-*/ | wc -l
353
$ grep -r "  - [x]" wave-*/ | wc -l
0
```

## Review performed

- **Drafting pass** (2026-11-21): read the v1 ledger
  (`ledger/MASTER_LEDGER.md`, `ledger/README.md`,
  `ledger/FINALIZATION.md`, `ledger/phase-00-product-freeze/
  ledger_phase_0.md`), `docs/roadmap/foundations.md`, and the design
  doc in full (§0–§17) before writing anything; wrote the 70 subtask
  files in the specified structure with no additions, drops, merges or
  renumbering; verified the counts above with grep.
- **Independent review — iteration 01** (2026-11-21, reviewer
  subagent who did not build the ledger): read the draft against the
  design doc and foundations.md; verified counts, structure,
  dependency direction, gate mapping, ADR mapping, control-authority
  wording, solver-class discipline and certification language.
  Verdict: **20 findings — 0 Critical, 2 High, 14 Medium, 4 Low.
  NOT finalized.** Recorded in
  `_iterations/iteration-01/findings.md`.
- **Patch pass** (2026-11-21, same session): all 20 findings patched;
  two new subtasks added continuing their wave sequences (W0.09,
  W5.11 — existing IDs never renumbered). The per-finding patch log
  with file references is appended to
  `_iterations/iteration-01/findings.md`.
- **Re-review — iteration 02** (2026-11-21, author's fresh re-read
  with a Python dependency-graph check and targeted greps of every
  patched line): every iteration-01 fix verified at the patched line;
  all structural checks pass (72 acceptance lines, 72 unique
  contiguous headings, 305 unchecked / 0 checked boxes, 0 undefined
  refs, 0 dependency cycles, 3 "certified" hits all negations).
  Verdict: **0 new findings.** Recorded in
  `_iterations/iteration-02/findings.md`. The ledger is FINALIZED as
  a plan on this basis.
- **Adversarial audit — iteration 03** (2026-11-21, independent
  auditor subagent who did not build the ledger): read the design doc
  (§0–§17) and foundations.md in full against all 72 subtask files
  plus the master docs, section by section, checking coverage,
  invented work, acceptance testability, duplicates, wrong-scope
  items and trace errors (≥20 traces opened at the cited design
  sections). Verdict: **45 findings — 3 High, 17 Medium, 25 Low.
  FINALIZED suspended.** Recorded in
  `_iterations/iteration-03/findings.md` (audit report; the full
  finding text was delivered to the parent agent and is summarized
  there with the patch log).
- **Patch pass — iteration 03** (2026-11-21, same session): all 45
  findings patched or rebutted with evidence; four new subtasks added
  continuing their wave sequences (W0.10, W1.19, W1.20, W5.12 —
  existing IDs never renumbered); W5.08 rewritten as a §11.3
  control-coverage verification matrix; W5.10 renamed and restated
  from "Phase 1 exit-gate review" to "Phase 1 scope review" (design
  §14 defines no F-17 Phase 1 exit gate); counts re-verified by grep
  (76 acceptance lines, 76 unique contiguous headings, 336 unchecked
  / 0 checked boxes). The per-finding patch log is in
  `_iterations/iteration-03/findings.md`. An independent re-review of
  the patched ledger is still required before the FINALIZED stamp is
  restored.
- **Re-review — iteration 04** (2026-11-21, independent re-auditor
  subagent who did not build or patch the ledger): read the design doc
  (§0–§17) and foundations.md in full against all 76 subtask files
  plus the master docs, re-deriving findings from scratch (coverage
  table, ≥20 traces opened at the cited design sections, grep
  recounts, dependency-graph check: 76 defined, 0 orphans, 0 cycles,
  exactly the 12 same-wave later-number dependencies). Verdict:
  **12 findings — 1 High, 4 Medium, 7 Low. FINALIZED stays
  suspended.** The headline iteration-03 patches all verified present;
  the findings were a same-class invented exit gate (F-16 Phase 3),
  one wrong G7 cross-reference, two stale deliverable lines from the
  iteration-03 deletions, and smaller trace/coverage gaps. Recorded in
  `_iterations/iteration-04/findings.md` (audit report + patch log).
- **Patch pass — iteration 04** (2026-11-21, same session): all 12
  findings patched (1 partial rebuttal — see the iteration-04 log);
  W3.05 renamed and restated from "Phase 3 exit-gate review" to
  "Phase 3 scope review" (design §14 defines no F-16 Phase 3 exit
  gate); counts re-verified by grep (76 acceptance lines, 76 unique
  contiguous headings, 339 unchecked / 0 checked boxes — +3 boxes from
  the new W1.02, W5.01 and W3.05 acceptance items). The per-finding
  patch log is in `_iterations/iteration-04/findings.md`. An
  independent re-review of the patched ledger is still required before
  the FINALIZED stamp is restored.
- **Re-review — iteration 05** (2026-11-21, independent re-auditor
  subagent who did not build or patch the ledger): read the design doc
  (§0–§17) and foundations.md in full against all 76 subtask files
  plus the master docs, re-deriving findings from scratch (coverage
  table, 30 traces opened at the cited design sections, grep recounts,
  dependency-graph check: 76 defined, 0 orphans, 0 cycles, exactly the
  12 same-wave later-number dependencies). Verdict: **7 findings — 0
  High, 1 Medium, 6 Low. FINALIZED stays suspended.** No design
  section uncovered; the iteration-04 headline patches all verified
  present. Recorded in `_iterations/iteration-05/findings.md` (audit
  report + patch log).
- **Patch pass — iteration 05** (2026-11-21, same session): all 7
  findings patched (0 rebuttals); no subtasks added, removed, merged
  or renamed; counts re-verified by grep (76 acceptance lines, 76
  unique contiguous headings, 339 unchecked / 0 checked boxes —
  unchanged). The per-finding patch log is in
  `_iterations/iteration-05/findings.md`. An independent re-review of
  the patched ledger is still required before the FINALIZED stamp is
  restored.
- **Re-review — iteration 06** (2026-11-21, independent re-auditor
  subagent who did not build or patch the ledger): read the design doc
  (§0–§17) and foundations.md in full against all 76 subtask files
  plus the master docs, re-deriving findings from scratch (coverage
  table, traces opened at the cited design sections, grep recounts,
  dependency-graph check: 76 defined, 0 orphans, 0 cycles, exactly the
  12 same-wave later-number dependencies). Verdict: **6 findings — 0
  High, 1 Medium, 5 Low. FINALIZED stays suspended.** No design
  section uncovered; the iteration-05 headline patches all verified
  present. Recorded in `_iterations/iteration-06/findings.md` (audit
  report + patch log).
- **Patch pass — iteration 06** (2026-11-21, same session): all 6
  findings patched (0 rebuttals); no subtasks added, removed, merged
  or renamed; counts re-verified by grep (76 acceptance lines, 76
  unique contiguous headings, 340 unchecked / 0 checked boxes — +1 box
  from the new W5.06 acceptance item). The per-finding patch log is in
  `_iterations/iteration-06/findings.md`. An independent re-review of
  the patched ledger is still required before the FINALIZED stamp is
  restored.
- **Re-review — iteration 07** (2026-11-21, independent re-auditor
  subagent who did not build or patch the ledger): read the design doc
  (§0–§17) and foundations.md in full against all 76 subtask files
  plus the master docs, re-deriving findings from scratch (coverage
  table, 28 traces opened at the cited design sections, grep
  recounts, dependency-graph check: 76 defined, 0 orphans, 0 cycles,
  exactly the 12 same-wave later-number dependencies). Verdict:
  **4 findings — 0 High, 1 Medium, 3 Low. FINALIZED stays
  suspended.** No design section uncovered except the §13.5 gap for
  Wave 4 prototype outputs (M-1); the iteration-06 headline patches
  all verified present. Recorded in
  `_iterations/iteration-07/findings.md` (audit report + patch log).
- **Patch pass — iteration 07** (2026-11-21, same session): all 4
  findings patched (0 rebuttals); no subtasks added, removed, merged
  or renamed; counts re-verified by grep (76 acceptance lines, 76
  unique contiguous headings, 342 unchecked / 0 checked boxes — +2
  boxes from the new W4.04 §13.5 acceptance item and the new W4.02
  interconnection acceptance item). The per-finding patch log is in
  `_iterations/iteration-07/findings.md`. An independent re-review of
  the patched ledger is still required before the FINALIZED stamp is
  restored.
- **Re-review — iteration 08** (2026-11-21, independent re-auditor
  subagent who did not build or patch the ledger): read the design doc
  (§0–§17) and foundations.md in full against all 76 subtask files
  plus the master docs, re-deriving findings from scratch (coverage
  table, 35 traces opened at the cited design sections, grep
  recounts, dependency-graph check: 76 defined, 0 orphans, 0 cycles,
  exactly the 12 same-wave later-number dependencies). Verdict:
  **3 findings — 0 High, 1 Medium, 2 Low. FINALIZED stays
  suspended.** No design section uncovered except the §13.5 gap for
  Wave 3 M&V results (M-1); the iteration-07 headline patches all
  verified present. Recorded in
  `_iterations/iteration-08/findings.md` (audit report + patch log).
- **Patch pass — iteration 08** (2026-11-21, same session): all 3
  findings patched (0 rebuttals); no subtasks added, removed, merged
  or renamed; counts re-verified by grep (76 acceptance lines, 76
  unique contiguous headings, 353 unchecked / 0 checked boxes — +11
  boxes: the new W3.02 §13.5 acceptance item and one new owners/status
  acceptance item in each of the 10 ADR-owning subtasks that lacked
  one). The per-finding patch log is in
  `_iterations/iteration-08/findings.md`. An independent re-review of
  the patched ledger is still required before the FINALIZED stamp is
  restored.
- **Re-review — iteration 09** (2026-11-21, independent re-auditor
  subagent who did not build or patch the ledger): read the design doc
  (§0–§17) and foundations.md in full against all 76 subtask files
  plus the master docs, re-deriving findings from scratch (coverage
  table, 28 traces opened at the cited design sections, grep
  recounts, dependency-graph check: 76 defined, 0 orphans, 0 cycles,
  exactly the 12 same-wave later-number dependencies). Verdict:
  **1 finding — 0 High, 0 Medium, 1 Low. FINALIZED stays
  suspended.** No design section uncovered; the iteration-08 headline
  patches all verified present. Recorded in
  `_iterations/iteration-09/findings.md` (audit report + patch log).
- **Patch pass — iteration 09** (2026-11-21, same session): the 1
  finding patched (0 rebuttals); no subtasks added, removed, merged
  or renamed; counts re-verified by grep (76 acceptance lines, 76
  unique contiguous headings, 353 unchecked / 0 checked boxes —
  unchanged). The per-finding patch log is in
  `_iterations/iteration-09/findings.md`. An independent re-review of
  the patched ledger is still required before the FINALIZED stamp is
  restored.
- **Re-review — iteration 10** (2026-11-21, independent re-auditor
  subagent who did not build or patch the ledger): read the design doc
  (§0–§17) and foundations.md in full against all 76 subtask files
  plus the master docs, re-deriving findings from scratch (coverage
  table, 37 traces opened at the cited design sections, grep
  recounts, dependency-graph check: 76 defined, 0 orphans, 0 cycles,
  exactly the 12 same-wave later-number dependencies). Verdict:
  **1 finding — 0 High, 0 Medium, 1 Low. FINALIZED stays
  suspended.** No design section uncovered; the iteration-09 headline
  patch verified present except for the residue it left in W5.10's
  step/acceptance lines. Recorded in
  `_iterations/iteration-10/findings.md` (audit report + patch log).
- **Patch pass — iteration 10** (2026-11-21, same session): the 1
  finding patched (0 rebuttals); no subtasks added, removed, merged
  or renamed; counts re-verified by grep (76 acceptance lines, 76
  unique contiguous headings, 353 unchecked / 0 checked boxes —
  unchanged). The per-finding patch log is in
  `_iterations/iteration-10/findings.md`.
- **Re-review — iteration 11** (2026-11-21, independent re-auditor
  subagent who did not build or patch the ledger): read the design doc
  (§0–§17) and foundations.md in full against all 76 subtask files
  plus the master docs, re-deriving findings from scratch (coverage
  table, 50 traces opened at the cited design sections, grep
  recounts, dependency-graph check: 76 defined, 0 orphans, 0 cycles,
  exactly the 12 same-wave later-number dependencies, plus a scripted
  scan of all 18 subtask-ID ranges and an Inputs-vs-Dependencies
  audit). Verdict: **ZERO findings — 0 High, 0 Medium, 0 Low.**
  No design section uncovered; the iteration-10 headline patch
  verified present; three candidates considered and cleared (W6.06
  raw-address exclusion, W0.08 verification-method line, W1.19
  §1.3 item 4 condition). The FINALIZED stamp is RESTORED on the
  basis of this review. Recorded in
  `_iterations/iteration-11/findings.md` (audit report).

## Sign-off checklist (each item checked with evidence, not asserted)

> RESTORED 2026-11-21 by the iteration-11 independent re-review
> (zero findings): every item below was checked `[x]` at v1.0
> (72 subtasks), reset to `[ ]` by the iteration-03 audit, and is
> re-checked `[x]` here against the patched ledger (76 subtasks,
> 353 unchecked boxes). "Finalized" describes the plan only.

- [x] Subtask total re-counted by an independent reviewer and by the
  iteration-02 re-review: 76, per-wave counts match MASTER_LEDGER.md
  §1 and README.md (10/20/8/5/13/12/8). — Re-counted again by the
  iteration-11 independent re-review: 76 (zero findings).
- [x] Every subtask has an ID, workstream, objective, design trace,
  inputs, steps, deliverable, testable acceptance criteria, owner
  role, dependencies, verification method, risks. — Spot-read in
  iteration 01 and iteration 02; format verified across all 76 files;
  verified again in iteration 11 (every file contains all ten
  structural fields).
- [x] Wave exit gates match design §14 (Gate 0, F-16 P1–P2, F-17 P0;
  F-16 Phase 3 and F-17 Phase 1 have NO §14 exit gate — W3.05 and
  W5.10 are scope reviews; later phases have none either — W6.08 is a
  close-out). — Verified in iteration 01; entry-gate fixes (F-05)
  verified in iteration 02; W5.10 restated in iteration 03; W3.05
  restated in iteration 04; re-verified in iteration 11 (exactly
  three `**Exit gate:**` lines in the design doc, zero findings).
- [x] §16 gates G1–G9 all OPEN, mapped to owning subtasks, none
  silently decided. — MASTER_LEDGER.md §5; verified in iteration 01
  (F-05, F-06, F-15 fixes applied); G7/G8 ownership extended in
  iteration 03 (W5.12 final G7, W0.10 F-16 G8); re-verified in
  iteration 11 (all 9 OPEN with owners, zero findings).
- [x] §15 ADRs (12) all PROPOSED, mapped to owning subtasks, none
  recorded as decided. — MASTER_LEDGER.md §6; verified in
  iteration 01; owners + status recorded in all 12 owning subtasks
  in iteration 08; re-verified in iteration 11 (12 owners/status
  lines, zero findings).
- [x] Control authority stated per wave: simulation-only until v1.0.0
  operator-approved control; F-16 qualification is not dispatch
  authority; F-17 never receives dispatch permission. No subtask
  grants field dispatch early. — Verified in iteration 01 (F-07,
  F-14, F-20 fixes applied); W3.03 restated with a cited basis in
  iteration 03; re-verified in iteration 11 (zero findings).
- [x] No safety/certification/protection-grade claims: the only 3
  "certified" hits in wave files are negations ("no
  BuildingSync-certified claim … without granted status"); solver
  classes stated correctly throughout (HiGHS LP/QP/MIP, Clarabel
  conic not MISOCP, AC-PF ≠ AC-OPF, IPOPT local). — Grep-checked
  21 Nov 2026 in iterations 01–03; re-checked in iteration 11
  (zero findings).
- [x] Design snapshot is 21 Nov 2026; no design-doc claim was
  re-verified against a primary source in this draft. — TRUE by
  construction; re-verify at implementation time.
- [x] Repo checks pass (outputs below); `git status` shows only
  `docs/roadmap/` (which also holds the two pre-existing uncommitted
  roadmap files, `foundations.md` and the design doc); nothing under
  `ledger/`, `docs/foundations/` or the design doc was touched;
  nothing committed or pushed. — Re-verified 2026-11-21 after the
  iteration-11 review: both checks exit 0, `git status` shows only
  `?? docs/roadmap/`.

## Verification (actual output, 2026-11-21, run from repo root)

```
$ python3 tools/verify_docs.py
report: docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC   (exit 0)
$ python3 tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid   (exit 0)
$ git status --short
?? docs/roadmap/
```

Re-run after the iteration-03 patch pass (same commands, same output);
the ledger recount at that point was 76 acceptance lines, 76 headings,
336 unchecked / 0 checked boxes, 0 orphan refs, 0 dependency cycles
(see `_iterations/iteration-03/findings.md`). Re-run again after the
iteration-04 patch pass (same commands, same output); the recount then
was 76 acceptance lines, 76 headings, 339 unchecked / 0 checked boxes
(+3 boxes from the new W1.02, W5.01 and W3.05 acceptance items), 0
orphan refs, 0 dependency cycles (see
`_iterations/iteration-04/findings.md`). Re-run again after the
iteration-05 patch pass (same commands, same output); the recount then
was unchanged (76 acceptance lines, 76 headings, 339 unchecked / 0
checked boxes, 0 orphan refs, 0 dependency cycles — no subtasks added,
removed or renamed; see `_iterations/iteration-05/findings.md`). Re-run
again after the iteration-06 patch pass (same commands, same output);
the recount then was 76 acceptance lines, 76 headings, 340 unchecked /
0 checked boxes (+1 box from the new W5.06 acceptance item), 0 orphan
refs, 0 dependency cycles (see
`_iterations/iteration-06/findings.md`). Re-run again after the
iteration-07 patch pass (same commands, same output); the recount then
was 76 acceptance lines, 76 headings, 342 unchecked / 0 checked boxes
(+2 boxes from the new W4.04 §13.5 acceptance item and the new W4.02
interconnection acceptance item), 0 orphan refs, 0 dependency cycles
(see `_iterations/iteration-07/findings.md`). Re-run again after the
iteration-08 patch pass (same commands, same output); the recount then
was 76 acceptance lines, 76 headings, 353 unchecked / 0 checked boxes
(+11 boxes: the new W3.02 §13.5 acceptance item and one new
owners/status acceptance item in each of the 10 ADR-owning subtasks
that lacked one), 0 orphan refs, 0 dependency cycles (see
`_iterations/iteration-08/findings.md`). Re-run again after the
iteration-09 patch pass (same commands, same output); the recount then
was unchanged (76 acceptance lines, 76 headings, 353 unchecked / 0
checked boxes, 0 orphan refs, 0 dependency cycles — no subtasks added,
removed or renamed; see `_iterations/iteration-09/findings.md`). Re-run
again after the iteration-10 patch pass (same commands, same output);
the recount then was unchanged (76 acceptance lines, 76 headings, 353
unchecked / 0 checked boxes, 0 orphan refs, 0 dependency cycles — no
subtasks added, removed or renamed; see
`_iterations/iteration-10/findings.md`). Re-run again after the
iteration-11 zero-findings re-review (same commands, same output);
the recount then was unchanged (76 acceptance lines, 76 headings, 353
unchecked / 0 checked boxes, 0 orphan refs, 0 dependency cycles — no
subtasks added, removed or renamed; see
`_iterations/iteration-11/findings.md`).

Note: `python` is not on PATH in this VM; `python3` was used. The two
repo checks validate the report tree and the agent harness; they do not
validate this ledger's content. `git status` shows a single untracked
directory, `docs/roadmap/`, because the two pre-existing uncommitted
roadmap files (`foundations.md`,
`flexcore_extensions_principal_engineering_design.md`) already lived
there before this ledger was written.

## Known limitations (do not mistake these for gaps)

- **Review history.** This ledger was independently reviewed once
  (iteration 01: 20 findings, all patched), re-reviewed once
  (iteration 02: zero findings remaining), then adversarially audited
  (iteration 03: 45 findings, all patched), re-reviewed
  (iteration 04: 12 findings, all patched), re-reviewed again
  (iteration 05: 7 findings, all patched) and re-reviewed again
  (iteration 06: 6 findings, all patched) and re-reviewed again
  (iteration 07: 4 findings, all patched) and re-reviewed again
  (iteration 08: 3 findings, all patched) and re-reviewed again
  (iteration 09: 1 finding, patched) and re-reviewed again
  (iteration 10: 1 finding, patched) and re-reviewed again
  (iteration 11: zero findings), and the FINALIZED stamp was
  RESTORED on the basis of that zero-findings re-review. The v1
  ledger's finalization rested on 10 review iterations plus a final
  independent review; this ledger has had 2 independent reviews +
  9 re-reviews. The reviews are recorded
  in `_iterations/` with per-finding file references.
- **Design-doc claims are not re-verified.** Standard editions
  (BuildingSync v2.7.0, ASHRAE Standard 211 2018 reaffirmed 2023 +
  Addendum a, IPMVP Core Concepts 2022), API versions (PVWatts V8,
  REopt API v3), and provider terms are taken from the design doc as
  stated. Re-verify each against its primary source at implementation
  time; do not pin from memory.
- **Checkbox granularity is one checkbox per acceptance criterion**
  (353 boxes across 76 subtasks, ~4 per subtask), matching the P0/P1/P2
  style of the v1 ledger. Implementers should evidence each criterion
  separately when ticking a subtask box.
- **The design doc's own open gates bound this ledger.** §16 gates
  G1–G9, the AGENTS.md §1.2 defect, and the D1/D2/D4 positions are
  recorded as open with owning subtasks; this ledger does not close
  any of them.

## Remaining open items (what honestly prevents calling this "complete")

These are decisions and work, not ledger defects. The ledger is a
finalized plan (as a plan only); the *project* is not started.

0. **Independent re-review of the patched ledger** — CLOSED
   2026-11-21: the iteration-11 independent re-review returned zero
   findings and the FINALIZED stamp was restored on that basis.
1. **Wave 0, Gate 0** — AGENTS.md reconciliation (§1.2), existing
   gates, D4, D1/D2, G1, G4. Each needs an owner, decision venue, and
   date (or the decision itself) before F-16/F-17 work can honestly
   proceed; both features stay outside the v1.0.0 critical path.
2. **§16 gates G2, G3, G5–G9** — first facility archetype, first asset
   integration, F-17 launch jurisdiction, roof imagery budget, REopt
   API vs local, retention/consent, license — all OPEN with owning
   subtasks (W1.02, W1.03, W4.01, W4.03, W4.10 interim + W5.12 final, W4.11, W0.03).
3. **All 12 §15 ADRs** — PROPOSED, none approved. Each needs its named
   owners to decide it.
4. **Licensed standards** — ASHRAE Standard 211 + Addendum a and
   IPMVP Core Concepts 2022 must be obtained as licensed copies with
   qualified/practitioner review before any normative workflow
   (W1.16, W3.02).
5. **F-17 research-candidate status** — F-17 stays a candidate until
   the Phase 0 prototype demonstrates real demand (W4.13 exit gate).
