# Iteration 09 — Independent re-review + patch pass (2026-11-21)

## Re-review verdict

- **Re-reviewer:** independent subagent who did not build or patch the
  ledger. It read the design doc (§0–§17) and foundations.md in full
  against all 76 subtask files plus the master docs, re-deriving
  findings from scratch (coverage table, 28 traces opened at the cited
  design sections, grep recounts, dependency-graph check).
- **Verdict: 1 finding — 0 High, 0 Medium, 1 Low. NOT zero findings.
  FINALIZED stays suspended.**
- The re-reviewer verified the iteration-08 headline patches all
  present. No design section uncovered; §13.5 coverage confirmed
  complete for every wave producing user-facing calculation results
  (Wave 6 produces no new calculation results — its outputs are
  governed by W5.05's report disclosure and W6.01/W6.07 validity
  discipline).

## Findings and patch log

### L-1 (Low) — Two exit/scope-review subtasks list their own output as their own input

- `wave-01-f16-phase-1/18_phase-1-exit-gate-review.md:7` (W1.18):
  Inputs listed "W1.01–W1.20 outputs" — the range includes W1.18
  itself.
- `wave-05-f17-phase-1/10_phase-1-scope-review-f17.md:7` (W5.10):
  Inputs listed "W5.01–W5.12 outputs" — the range includes W5.10
  itself.

A subtask cannot consume its own output as a prerequisite. The
ledger's own convention elsewhere excludes the review subtask from its
input range (W0.08: "W0.01–W0.07, W0.09 and W0.10"; W2.08:
"W2.01–W2.07"; W3.05: "W3.01–W3.04"; W4.13: "W4.01–W4.12"; W6.08:
"W6.01–W6.07"). The authoritative Dependencies lines were already
correct (W1.18 lists W1.01–W1.17, W1.19, W1.20; W5.10 lists
W5.01–W5.09, W5.11, W5.12, W4.13), so no cycle or execution error
resulted — the defect was confined to the Inputs prose line. Verified
by the patcher against both files and all five convention examples
before patching.

Patched (exactly as the reviewer specified; no checkbox-count or
dependency change):

- W1.18 Inputs now read: "W1.01–W1.17, W1.19 and W1.20 outputs;
  design §14 F-16 Phase 1."
- W5.10 Inputs now read: "W5.01–W5.09, W5.11 and W5.12 outputs;
  design §14 F-17 Phase 1; W4.13 Phase 0 exit."

## Master-doc updates (counts only — no subtask added, removed, merged or renamed)

- `FINALIZATION.md`: status line, "what finalized means", recount
  block (now "after the iteration-09 patch pass"), review history
  (iteration-09 re-review + patch-pass entries), re-run paragraph,
  Known limitations review-history bullet (2 independent reviews +
  7 re-reviews), Scope and file count (8 → 9 review files,
  87 → 88 `.md` files total). Totals unchanged (353 unchecked boxes).
- `MASTER_LEDGER.md`: banner (iteration-09 re-review entry) and §7
  item 6 (iteration-09 entry).
- `README.md`: review-history paragraph (iteration-09 entry).

## Counts after the iteration-09 patch pass

No subtasks added, removed, merged or renamed. Total stays
**76 subtasks** (10/20/8/5/13/12/8). No boxes added or removed:
**353 unchecked boxes**, 0 checked.

## Verification (actual output, 2026-11-21, run from the ledger directory)

```
$ grep -r -c "Acceptance criteria" wave-*/ | awk -F: '{s+=$2} END {print s}'
76
$ grep -r -h "^# W" wave-*/ | wc -l
76
$ grep -r -F -h "  - [ ]" wave-*/ | wc -l
353
$ grep -r -F "  - [x]" wave-*/ | wc -l
0
```

Dependency-graph check (`/tmp/ledger_iter07_graph.py`, same method as
iterations 02–08): 76 defined subtasks, 0 undefined refs anywhere,
0 orphan Dependencies refs, 0 cycles, exactly the 12 same-wave
later-number dependencies listed in MASTER_LEDGER §4 (W0.08→W0.09,
W0.08→W0.10, W1.10→W1.20, W1.18→W1.19, W1.18→W1.20, W4.04→W4.07,
W4.04→W4.08, W4.04→W4.09, W4.05→W4.08, W5.10→W5.11, W5.10→W5.12,
W6.05→W6.06). No Dependencies lines changed in this pass.

Repo checks (from `~/workspace/energy-flexcore`):

```
$ python3 tools/verify_docs.py
OK: headings, section references, fences, tables, references, TOC
$ python3 tools/validate_agent.py
OK: skills, roles, and README index are valid
```

Both exit 0. `git status --short` shows only `?? docs/roadmap/` —
nothing under `ledger/`, `docs/foundations/`, or the design doc
touched; nothing committed or pushed.

## Rebuttals

None. The single finding was patched as instructed after checking the
cited file and line.

## Remaining gaps (honest)

1. Design-doc claims (BuildingSync v2.7.0, ASHRAE 211, IPMVP 2022,
   PVWatts V8, REopt API v3) were not re-verified against primary
   sources in this pass either; FINALIZATION.md still defers that to
   implementation time.
2. The FINALIZED stamp stays suspended until an independent re-review
   of the patched ledger returns zero findings.
