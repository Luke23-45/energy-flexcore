# Iteration 08 — Independent re-review + patch pass (2026-11-21)

## Re-review verdict

- **Re-reviewer:** independent subagent who did not build or patch the
  ledger. It read the design doc (§0–§17) and foundations.md in full
  against all 76 subtask files plus the master docs, re-deriving
  findings from scratch (coverage table, 35 traces opened at the cited
  design sections, grep recounts, dependency-graph check).
- **Verdict: 3 findings — 0 High, 1 Medium, 2 Low. NOT zero findings.
  FINALIZED stays suspended.**
- The re-reviewer verified the iteration-07 headline patches all
  present. One design section was uncovered for one wave (M-1: §13.5
  for Wave 3 M&V results); no other design section uncovered.

## Findings and patch log

### M-1 (Medium) — §13.5 has no Wave 3 owner: efficiency-savings M&V results carry no trustworthy-result disclosure requirement

Neither W3.01 nor W3.02 cited §13.5 at all (grep for `13\.5` across
`wave-*/` returned only W1.10, W2.02, W2.06, W4.04, W4.13 and W5.05),
and W3.05 did not check it either — yet Wave 3 produces
savings-verification results as a user-facing report carrying
financial savings claims (W3.01 step 1 requires "independent reviewer
and report status"; W3.02 step 1 determines savings "by comparing
measured baseline and reporting-period consumption/demand on a
consistent basis, with suitable adjustments"). Only one §13.5 item had
partial coverage (W3.01 step 1: "data exclusions and uncertainty" and
"M&V plan version and chosen method"). Same defect class as
iteration-07 M-1 (§13.5 missing for Wave 4 prototype outputs).
Verified by the patcher against design §13.5 (lines 799–811: all 7
items), W3.01, W3.02 and W3.05 before patching.

Patched (W3.02 is the owner — it produces the results; W3.05 checks
them at the scope review):

- `wave-03-f16-phase-3/02_ipmvp-based-efficiency-savings-mv-workflow.md`
  (W3.02): objective now requires trustworthy-result disclosure per
  §13.5 on every M&V result; design trace and Inputs add §13.5; new
  step 5 applies §13.5 to every efficiency-savings M&V result/report
  (all 7 items, with solver status recorded explicitly as N/A — no
  solver runs in M&V determination, savings are determined by comparing
  measured baseline and reporting-period consumption/demand with
  adjustments, not by a solver); deliverable adds the §13.5 disclosure;
  new acceptance item requires the §13.5 disclosure on every M&V
  result/report, with solver status recorded explicitly as N/A.
- `wave-03-f16-phase-3/05_phase-3-scope-review-f16-handover.md`
  (W3.05): step 1 now also verifies the efficiency-savings M&V results
  carry the §13.5 disclosure (W3.02). No new checkbox: the disclosure
  requirement is evidenced by W3.02's own acceptance item and checked
  here at the scope review.

### L-1 (Low) — MASTER_LEDGER claims every ADR-owning subtask records its ADR as proposed; only 2 of 12 did

MASTER_LEDGER.md §6 ("Each owning subtask records its ADR as
proposed, pending approval by its named owners") and §8 ("All 12 §15
ADRs: PROPOSED, none approved. Recorded as proposed in their owning
subtasks.") overclaimed: grep for the status wording returned only
W2.07 and W4.12. The other ten owning subtasks said only "Record
ADR-… as owned by this subtask" with no status word. No subtask
recorded any ADR as decided, so the substantive risk was nil; the
defect was the master-doc overclaim plus missing status recording.
Verified by the patcher against MASTER_LEDGER.md §6/§8, design §15
(all 12 PROPOSED in the design table, lines 880–897) and all 12 owning
subtasks before patching.

Patched (the reviewer's preferred option — make the MASTER claim true
rather than weakening it): each of the 10 subtasks now names its
owners (verbatim from the design §15 "Owner / gate" column) and
records "status (proposed, pending approval)", mirroring W2.07 step 3
/ W4.12 step 4, with a matching acceptance item mirroring W2.07
acceptance 3 ("Owners (…) and status are recorded."):

- W0.05 ADR-SHARED-01 — owners (architecture review)
- W1.05 ADR-F16-01 — owners (product, domain architecture)
- W1.07 ADR-DATA-01 — owners (data architecture, safety review)
- W1.09 ADR-F16-03 — owners (domain architecture)
- W1.13 ADR-F16-06 — owners (frontend/API owners)
- W1.15 ADR-F16-02 — owners (product, building-energy reviewer,
  license review)
- W3.02 ADR-F16-04 — owners (M&V practitioner, product) (step 4
  extended; acceptance item added alongside the M-1 §13.5 item)
- W4.07 ADR-F17-04 — owners (product, numerical validation)
- W4.08 ADR-F17-02 — owners (product, local energy-market expert)
- W4.09 ADR-F17-03 — owners (numerical engineering,
  license/operations review)

No MASTER_LEDGER.md §6/§8 wording change was needed: the claim is now
true as written. Verified after patching: grep for "Owners (.*) and
status are recorded" across `wave-*/` returns 12 hits (all 12
ADR-owning subtasks).

### L-2 (Low) — W1.01 step 3 misquotes the foundations.md §4 F-16 definition it claims to reproduce

W1.01 step 3 dropped the possessive ("facility's" → "facility") and
dropped "operational" before "constraints", in a sentence explicitly
presented as the foundations definition. Verified by the patcher
against foundations.md line 352 before patching.

Patched (`wave-01-f16-phase-1/01_f16-scope-statement-non-goals.md`
step 3): now verbatim — "a structured, evidence-backed representation
of a facility's energy resources, operational constraints, efficiency
opportunities and available flexibility". No other change: acceptance
1's "(asset and flexibility inventory for FlexCore)" is a paraphrase
summary, not a quotation, and is unchanged.

## Master-doc updates (counts only — no subtask added, removed, merged or renamed)

- `FINALIZATION.md`: status line, "what finalized means", verified
  totals (342 → 353 unchecked boxes), recount block (now "after the
  iteration-08 patch pass"), review history (iteration-08 re-review +
  patch-pass entries), re-run paragraph, Known limitations review-history
  bullet (2 independent reviews + 6 re-reviews), checkbox granularity
  (342 → 353 boxes), Scope and file count (7 → 8 review files,
  86 → 87 `.md` files total).
- `MASTER_LEDGER.md`: banner (iteration-08 re-review entry) and §7
  item 6 (iteration-08 entry).
- `README.md`: review-history paragraph (iteration-08 entry).

## Counts after the iteration-08 patch pass

No subtasks added, removed, merged or renamed. Total stays
**76 subtasks** (10/20/8/5/13/12/8). Eleven new acceptance boxes (the
W3.02 §13.5 item + one owners/status item in each of the 10
ADR-owning subtasks that lacked one): **353 unchecked boxes**,
0 checked.

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
$ grep -r -c "Owners (.*) and status are recorded" wave-*/ | awk -F: '{s+=$2} END {print s}'
12
```

Dependency-graph check (`/tmp/ledger_iter07_graph.py`, same method as
iterations 02–07): 76 defined subtasks, 0 undefined refs anywhere,
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

None. Every finding was patched as instructed after checking the cited
file and line.

## Remaining gaps (honest)

1. Design-doc claims (BuildingSync v2.7.0, ASHRAE 211, IPMVP 2022,
   PVWatts V8, REopt API v3) were not re-verified against primary
   sources in this pass either; FINALIZATION.md still defers that to
   implementation time.
2. The FINALIZED stamp stays suspended until an independent re-review
   of the patched ledger returns zero findings.
