# Iteration 07 — Independent re-review + patch pass (2026-11-21)

## Re-review verdict

- **Re-reviewer:** independent subagent who did not build or patch the
  ledger. It read the design doc (§0–§17) and foundations.md in full
  against all 76 subtask files plus the master docs, re-deriving
  findings from scratch (coverage table, 28 traces opened at the cited
  design sections, grep recounts, dependency-graph check).
- **Verdict: 4 findings — 0 High, 1 Medium, 3 Low. NOT zero findings.
  FINALIZED stays suspended.**
- The re-reviewer verified the iteration-06 headline patches all
  present. One design section was uncovered for one wave (M-1: §13.5
  for Wave 4 prototype outputs); no other design section uncovered.

## Findings and patch log

### M-1 (Medium) — §13.5 has no Wave 4 owner: prototype results shown to homeowners carry no trustworthy-result disclosure requirement

Design §13.5 is cross-cutting ("A successful calculation is not just
an `OK` status. The response must state: …" — 7 items), and
MASTER_LEDGER.md §7 item 5 requires "Every user-facing result meets
the §13.5 trustworthy-result disclosure". But no Wave 4 subtask cited
§13.5 at all (grep for `13\.5` across `wave-*/` returned only W1.10,
W2.02, W2.06, W5.05). The Phase 0 prototype produces calculation
results shown to real homeowners in usability tests (W4.06), yet
W4.13 step 2 verified only two §13.5-adjacent honesty conditions, and
W4.04 step 2 recorded per-run provenance without requiring the full
§13.5 disclosure. Verified by the patcher against design §13.5
(lines 799–811: all 7 items), W4.04, W4.05, W4.06 and W4.13 before
patching.

Patched (W4.04 is the owner — it produces the results; W4.13 checks
them at the gate):

- `wave-04-f17-phase-0/04_prototype-pv-only-pv-battery-real-datasets.md`
  (W4.04): objective now requires trustworthy-result disclosure per
  §13.5 on every result shown to a homeowner; design trace and Inputs
  add §13.5; new step 6 applies §13.5 to every prototype result shown
  to a homeowner (including in the W4.06 usability tests), stating all
  7 items in full; deliverable adds the §13.5 disclosure; new
  acceptance item requires the §13.5 disclosure on every prototype
  result shown to a homeowner. No item is marked N/A: all 7 apply to
  prototype results (the prototype runs a REopt API v3 proof-of-concept
  adapter, so solver status applies; financial conclusions are drawn,
  so the unsupported-conclusion flag applies).
- `wave-04-f17-phase-0/13_phase-0-exit-gate-review-f17.md` (W4.13):
  step 2 now also verifies that every prototype result shown to a
  homeowner carries the §13.5 disclosure (W4.04). No new checkbox:
  the disclosure requirement is evidenced by W4.04's own acceptance
  item and checked here at the gate.

### L-1 (Low) — W2.03 names W1.17 as a test oracle for qualification transitions that W1.17 explicitly defers

`wave-02-f16-phase-2/03_flexibility-discovery-lifecycle.md` step 4
recorded the lifecycle as the SINGLE test oracle "for the
qualification state-transition tests in W1.17/W2.08", but W1.17 step 1
defers exactly those items ("flexibility qualification state
transitions and expiry/revocation (defined in W2.03)") to the Wave 2
test record (W2.08 exit gate), and W1.17 contains no
qualification-transition tests. Verified by the patcher against W1.17
step 1, W2.03 step 4 and W2.08 step 5 (which verifies exactly those
deferred items against W2.03) before patching. The patcher also
checked the other W1.17 mentions (W1.04 step 4, W1.14, W1.15, W1.18
step 2, W2.08): all are legitimate — authorization matrix, RLS,
BuildingSync tests and the exit-gate package are genuinely in W1.17
Phase 1 scope. Only W2.03 step 4 repeats the error.

Patched (`wave-02-f16-phase-2/03_flexibility-discovery-lifecycle.md`
step 4): the oracle location is now W2.08 only, with the W1.17
deferral cited ("W1.17 defers qualification state transitions to the
Wave 2 test record — see W1.17 step 1"). W2.03 acceptance 6 does not
repeat the error (it names no subtask) and is unchanged.

### L-2 (Low) — G5 interconnection data is named in W4.01 but gathered by no subtask

Design §16 item 5 (G5): the launch jurisdiction supplies
"authoritative tariff, export-credit, installed-cost, incentive and
interconnection data". W4.01 step 3 and acceptance 3 name the
interconnection data sources, but W4.02 steps 1–4 gathered only tariff
schedules, installed-cost sources and incentives, with a named-gaps
list for unsourced tariff/cost items — interconnection data appeared
nowhere in W4.02 or any other Wave 4 file (grep for "interconnection"
in `wave-04-*` returned only W4.01). The gap matters because the
decision report's next-steps (W5.05 step 6) and the enrollment handoff
(W6.05 step 1, "interconnection rules") later consume interconnection
rules/process data. Verified by the patcher against design §16 item 5,
W4.01 and W4.02 before patching.

Patched (`wave-04-f17-phase-0/02_gather-authoritative-tariff-installed-cost-sources.md`):
new step 4 gathers the interconnection data (interconnection rules and
process) from authoritative utility/regulator sources, with source and
effective date per item, naming W5.05 and W6.05 as the consumers; old
step 4 (named gaps) becomes step 5 and now covers tariff, cost,
incentive and interconnection items; heading and objective now include
incentive and interconnection sources; deliverable is now a tariff,
installed-cost, incentive and interconnection source pack; new
acceptance item requires an authoritative source and effective date for
interconnection rules/process data; the named-gaps acceptance item now
covers interconnection items too. No new dependency: W4.01 was already
W4.02's only dependency. (Export-credit data was not flagged and is
not added as a separate gather step: it rides with the tariff
schedules gathered in step 1.)

### L-3 (Low) — The §5.1↔§5.5 state mapping is defined normatively in two subtasks, with drift risk

W1.08 step 5 and W2.03 step 5 stated the identical five-pair mapping
(FlexibilityReview↔ENGINEERING_REVIEW, CandidateQualified↔QUALIFIED,
CandidateRejected↔REJECTED, OnboardingDraft↔ONBOARDING_DRAFT,
HumanApproval+CoreAssetCreated↔APPROVED_AND_ONBOARDED), the same
partial-scope caveat (DISCOVERED/EVIDENCE_INCOMPLETE before
FlexibilityReview; EXPIRED/SUSPENDED with no §5.1 counterpart) and the
same governs-clause, and both checked it in acceptance (W1.08
acceptance 5, W2.03 acceptance 5). Same defect class as iteration-06
M-1 (§11.3 controls defined twice), lower risk because the two
wordings were currently identical and no acceptance forbade the
duplication. Verified by the patcher by diffing W1.08 step 5 against
W2.03 step 5 before patching.

Patched (W2.03 owns the mapping — it is the §5.5 lifecycle owner;
W1.08 cross-references it):

- `wave-01-f16-phase-1/08_assessment-lifecycle-state-machine.md`
  (W1.08): step 5 no longer restates the mapping — it states the
  mapping is defined once, in W2.03 step 5, and is not restated here;
  acceptance 5 now checks that W2.03 step 5 is referenced as the
  single definition and that no second copy is recorded in this
  subtask. No Dependencies change: W2.03 is a later-wave test-oracle /
  definition owner, i.e. a forward cross-reference under the README
  step 3 / MASTER_LEDGER §4 rule, not a prerequisite (W1.08 step 4
  already forward-referenced W2.03 the same way).
- `wave-02-f16-phase-2/03_flexibility-discovery-lifecycle.md`
  (W2.03): step 5 now states it is the single definition of the
  §5.5-to-§5.1 state mapping and that W1.08 step 5 references it and
  does not restate it; deliverable names the single mapping;
  acceptance 5 now checks the mapping "as the single definition (W1.08
  step 5 references it; no second copy)".

## Master-doc updates (counts only — no subtask added, removed, merged or renamed)

- `FINALIZATION.md`: status line, "what finalized means", verified
  totals (340 → 342 unchecked boxes), recount block (now "after the
  iteration-07 patch pass"), review history (iteration-07 re-review +
  patch-pass entries), re-run paragraph, Known limitations review-history
  bullet (2 independent reviews + 5 re-reviews), checkbox granularity
  (340 → 342 boxes), Scope and file count (6 → 7 review files,
  85 → 86 `.md` files total).
- `MASTER_LEDGER.md`: banner (iteration-07 re-review entry) and §7
  item 6 (iteration-07 entry).
- `README.md`: review-history paragraph (iteration-07 entry).

## Counts after the iteration-07 patch pass

No subtasks added, removed, merged or renamed. Total stays
**76 subtasks** (10/20/8/5/13/12/8). Two new acceptance boxes (W4.04
§13.5 disclosure item, W4.02 interconnection item): **342 unchecked
boxes**, 0 checked.

## Verification (actual output, 2026-11-21, run from the ledger directory)

```
$ grep -r -c "Acceptance criteria" wave-*/ | awk -F: '{s+=$2} END {print s}'
76
$ grep -r -h "^# W" wave-*/ | wc -l
76
$ grep -r -F -h "  - [ ]" wave-*/ | wc -l
342
$ grep -r -F "  - [x]" wave-*/ | wc -l
0
```

Note: the unquoted `grep -r -h "  - [ ]"` form returned 0 in this
session (shell/bracket handling in this shell); the `-F` fixed-string
form above is the authoritative count, cross-checked per file (W4.04
now has 5 boxes, W4.02 now has 5).

Dependency-graph check (`/tmp/ledger_iter07_graph.py`, re-created for
this iteration — the iteration-03 script had been wiped from /tmp;
same method): 76 defined subtasks, 0 undefined refs anywhere,
0 orphan Dependencies refs, 0 cycles, exactly the 12 same-wave
later-number dependencies listed in MASTER_LEDGER §4 (W0.08→W0.09,
W0.08→W0.10, W1.10→W1.20, W1.18→W1.19, W1.18→W1.20, W4.04→W4.07,
W4.04→W4.08, W4.04→W4.09, W4.05→W4.08, W5.10→W5.11, W5.10→W5.12,
W6.05→W6.06). No Dependencies lines changed in this pass.

Repo checks (from `~/workspace/energy-flexcore`):

```
$ python3 tools/verify_docs.py
report: docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC
$ python3 tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid
```

`git status --short` shows only `?? docs/roadmap/` — nothing under
`ledger/`, `docs/foundations/`, or the design doc touched; nothing
committed or pushed.

## Rebuttals

None. Every finding was patched as instructed after checking the cited
file and line.

## Remaining gaps (honest)

1. Design-doc claims (BuildingSync v2.7.0, ASHRAE 211, IPMVP 2022,
   PVWatts V8, REopt API v3) were not re-verified against primary
   sources in this pass either; FINALIZATION.md still defers that to
   implementation time.
2. The FINALIZED stamp stays suspended until an independent re-review
   of the patched ledger returns zero findings.
