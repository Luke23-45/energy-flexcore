# Iteration 05 — Independent re-review + patch pass (2026-11-21)

## Re-review verdict

- **Re-reviewer:** independent subagent who did not build or patch the
  ledger. It read the design doc (§0–§17) and foundations.md in full
  against all 76 subtask files plus the master docs, re-deriving
  findings from scratch (coverage table, 30 traces opened at the cited
  design sections, grep recounts, dependency-graph check).
- **Verdict: 7 findings — 0 High, 1 Medium, 6 Low. NOT zero findings.
  FINALIZED stays suspended.**
- The re-reviewer verified the iteration-04 headline patches all
  present and found NO uncovered design section.

## Findings and patch log

### M-1 (Medium) — W2.06 step/acceptance mismatch on §13.5

`wave-02-f16-phase-2/06_onboarding-draft-human-approval.md` step 5
enumerated only five of the seven §13.5 disclosure items (omitting
solver status and whether any financial conclusion is unsupported),
while acceptance item 5 required the whole set. The three parallel
§13.5 subtasks (W1.10 step 5, W2.02 step 6, W5.05 step 3) all
enumerate all seven; W2.06 was the outlier. Verified by the patcher
against W2.06 and the §13.5 item list (MASTER_LEDGER §7.5) before
patching.

Patched: step 5 now enumerates all seven, with the two not-applicable
items recorded as N/A with reasons, not omitted silently — solver
status (no solver runs in qualification or approval: the decision is a
human approval of evidence, not a computed result) and whether any
financial conclusion is unsupported (these outputs publish no financial
conclusion — measure economics live in W2.02). Acceptance item 5 is
unchanged and is now satisfiable as written.

### L-1 (Low) — Dependencies omit subtasks named in the Inputs line (9 cases)

Same class as iteration-04 L-5. Each verified by the patcher against
the cited file before patching. All fixed by adding the missing ID to
Dependencies (except W5.08 — see below):

- **W1.14** (`wave-01-f16-phase-1/14_persistence-object-storage-rls.md`):
  Inputs cite W1.09 ingestion and W1.12 events; Dependencies now
  W1.06, W1.09, W1.11, W1.12, W0.10.
- **W2.05** (`wave-02-f16-phase-2/05_safe-mapping-into-existing-flexcore-assets.md`):
  Inputs cite W1.05 domain model; Dependencies now W2.03, W2.04, W1.05.
- **W2.08** (`wave-02-f16-phase-2/08_phase-2-exit-gate-review.md`):
  Inputs cite W1.18 Phase 1 exit; Dependencies now include W1.18
  (and W1.17 — see L-2).
- **W3.03** (`wave-03-f16-phase-3/03_bms-ems-edge-interval-integrations-pilot.md`):
  Inputs cite W0.02 existing security gates; Dependencies now W1.03,
  W2.08, W0.02.
- **W3.04** (`wave-03-f16-phase-3/04_expand-measure-catalogs-sector-support-real-evidence.md`):
  Inputs cite W2.02 scenarios; Dependencies now W2.01, W2.02, W2.08.
- **W3.05** (`wave-03-f16-phase-3/05_phase-3-scope-review-f16-handover.md`):
  Inputs cite W2.08 Phase 2 exit; Dependencies now W3.01–W3.04, W2.08.
- **W5.09** (`wave-05-f17-phase-1/09_f17-test-plan.md`): Inputs cite
  W4.05 benchmark record; Dependencies now W5.02–W5.08, W4.05.
- **W5.10** (`wave-05-f17-phase-1/10_phase-1-scope-review-f17.md`):
  Inputs cite W4.13 Phase 0 exit; Dependencies now W5.01–W5.09,
  W5.11, W5.12, W4.13.
- **W5.08** (`wave-05-f17-phase-1/08_privacy-security-design.md`):
  Inputs cited W5.09 test plan; adding it to Dependencies would create
  a cycle (W5.09 already depends on W5.08). Fixed as the reviewer
  directed: W5.09 removed from W5.08's Inputs. Step 1 still allows a
  control's verification to be a W5.09 test item or a design review —
  that is a forward verification reference, not a dependency.

### L-2 (Low) — Dependencies omit subtasks consumed in steps/acceptance (4 cases)

Each verified by the patcher against the cited file before patching:

- **W5.12** (`wave-05-f17-phase-1/12_record-final-g7-decision.md`):
  step 1 and acceptance item 1 cite the W4.05 benchmark record;
  Inputs and Dependencies both omitted it. Inputs now cite the W4.05
  benchmark record; Dependencies now W5.03, W4.05, W4.06, W4.10.
- **W2.06** (`wave-02-f16-phase-2/06_onboarding-draft-human-approval.md`):
  step 4 links the QualificationDecision record from W1.06; Inputs
  and Dependencies omitted it. Inputs now cite the W1.06
  QualificationDecision record; Dependencies now include W1.06.
- **W2.08**: step 5 verifies the Wave 2 §13.3 test record deferred
  from W1.17; Dependencies omitted W1.17. Inputs now cite the W1.17
  test plan; Dependencies now include W1.17.
- **W6.01** (`wave-06-f17-later-phases/01_more-jurisdiction-packs.md`):
  step 3 applies the W4.08 tariff-model rules; Inputs and Dependencies
  omitted W4.08. Inputs now cite the W4.08 tariff model; Dependencies
  now W4.01, W4.02, W4.08, W5.10.

### L-3 (Low) — W6.08 objective still implies every wave has an exit gate

`wave-06-f17-later-phases/08_roadmap-ledger-close-out-handover.md`
objective said "verify all waves exit gates are evidenced", while step 1
and acceptance 1 already correctly separate the four real exit gates
from the W3.05/W5.10 scope reviews. Verified by the patcher before
patching.

Patched: objective now reads "verify every wave's exit gate (where
design §14 defines one) and scope review is evidenced".

### L-4 (Low) — G7 interim/final wording unqualified in two places

- **W4.10** (`wave-04-f17-phase-0/10_decide-reopt-api-v3-vs-local-reopt-jl.md`):
  Owns line said "Owns §16 gate G7 (…)" with no interim qualifier,
  though the objective, steps and acceptance all say interim position
  only. Patched: Owns line now reads "Owns §16 gate G7 — interim
  position only (…; the final decision is W5.12)".
- **W5.03** (`wave-05-f17-phase-1/03_scenario-comparison-grid-pv-pv-battery.md`):
  Inputs said "W4.10 REopt decision". Patched: "W4.10 REopt interim
  position".

### L-5 (Low) — W4.09 trace omits §10.4, the source of its optimizer-port rule

`wave-04-f17-phase-0/09_roof-solar-resource-provider-seams-prototype.md`
step 5 states the optimizer port is reused only under "the W5.03
condition". That condition is design §10.4's closing paragraph verbatim
(design line 634: "The existing FlexCore `Optimizer` port should only
be reused if the chosen home-planning formulation can be faithfully
expressed in its problem IR and the selected provider declares support
for all needed features."), not §1.1 and not W5.03 (W5.03 merely
restates it). Verified by the patcher against design line 634 before
patching.

Patched: trace now cites §10.4 (optimizer-port reuse condition); step 5
and acceptance 6 now cite the §10.4 condition, noting W5.03 restates it.

### L-6 (Low) — §8.2 step-8 contents misattributed; real basis is foundations.md §3 item 5 (+ design §8.1)

- **W5.11** (`wave-05-f17-phase-1/11_section-8-2-user-journey-trace.md`):
  step 3 listed the seven-item decision-package contents as §8.2 step 8
  contents. Design §8.2 step 8 contains none of them — it says only to
  produce a downloadable report able to recommend "do not install" or
  "collect more data". The seven items are foundations.md §3 item 5
  ("A decision and implementation report") verbatim. Verified by the
  patcher against design §8.2 and foundations.md lines 277–290 before
  patching. Patched: trace now cites "Design §8.2; foundations.md §3
  item 5 (step-8 decision-package contents)"; step 3 now attributes the
  seven contents to foundations §3 item 5 and states what §8.2 step 8
  itself requires.
- **W5.05** (`wave-05-f17-phase-1/05_immutable-calculation-report.md`):
  step 6 attributed the next-steps content to "§8.2 step 8".
  "Site-specific quotation" traces to design §8.1 (line 506: "the next
  steps needed for a site-specific quotation or inspection"), and "what
  the household needs to do to proceed" to foundations.md §3 item 5.
  Verified by the patcher before patching. Patched: trace now cites
  §8.1 and foundations.md §3 item 5 alongside §8.2 step 8; Inputs cite
  both; step 6 and acceptance 6 now attribute the content correctly
  and state it is delivered in the §8.2 step 8 report.

## Counts after the iteration-05 patch pass

No subtasks added, removed, merged or renamed. Total stays
**76 subtasks** (10/20/8/5/13/12/8). No acceptance boxes added or
removed: **339 unchecked boxes**, 0 checked.

## Verification (actual output, 2026-11-21, run from the ledger directory)

```
$ grep -r -c "Acceptance criteria" wave-*/ | awk -F: '{s+=$2} END {print s}'
76
$ grep -r -h "^# W" wave-*/ | wc -l
76
$ grep -r -h "  - [ ]" wave-*/ | wc -l
339
$ grep -r "  - [x]" wave-*/ | wc -l
0
```

Dependency-graph check (`/tmp/ledger_iter03_graph.py`, same method as
iterations 02–04): 76 defined subtasks, 0 orphan refs, exactly the 12
same-wave later-number dependencies listed in MASTER_LEDGER §4
(W0.08→W0.09, W0.08→W0.10, W1.10→W1.20, W1.18→W1.19, W1.18→W1.20,
W4.04→W4.07, W4.04→W4.08, W4.04→W4.09, W4.05→W4.08, W5.10→W5.11,
W5.10→W5.12, W6.05→W6.06), 0 cycles. Every Dependencies ID added in
this pass is cross-wave or earlier-numbered same-wave, so the
later-number list is unchanged.

## Rebuttals

None. Every finding was patched as instructed after checking the cited
file and line.

## Notes (not scored as findings — design-owner observation)

The iteration-05 reviewer noted (not scored): design §5.5's own
lifecycle string reads `… ENGINEERING_REVIEW → QUALIFIED | REJECTED →
ONBOARDING_DRAFT → …`, which literally routes REJECTED into
ONBOARDING_DRAFT. W2.03 step 1 reproduces this faithfully; the
ambiguity is in the design doc, not a ledger invention. The design
owners should clarify whether REJECTED is terminal. The ledger cannot
fix this — the design doc is edited only by its owners.

## Remaining gaps (honest)

1. Design-doc claims (BuildingSync v2.7.0, ASHRAE 211, IPMVP 2022,
   PVWatts V8, REopt API v3) were not re-verified against primary
   sources in this pass either; FINALIZATION.md still defers that to
   implementation time.
2. The FINALIZED stamp stays suspended until an independent re-review
   of the patched ledger returns zero findings.
