# Iteration 04 — Independent re-review + patch pass (2026-11-21)

## Re-review verdict

- **Re-reviewer:** independent subagent who did not build or patch the
  ledger. It read the design doc (§0–§17) and foundations.md in full
  against all 76 subtask files plus the master docs, re-deriving
  findings from scratch (coverage table, ≥20 traces opened at the
  cited design sections, grep recounts, dependency-graph check).
- **Verdict: 12 findings — 1 High, 4 Medium, 7 Low. NOT zero findings.
  FINALIZED stays suspended.**
- The re-reviewer verified the iteration-03 headline patches all
  present (lifecycle mapping, W6.06 closed list, G7 deferral, §13.2
  bullet 4, readiness report, tariff pack, F-16 G8, §8.2 steps 1/8,
  §13.5 for F-16, launch-sequencing rule, W5.10 scope review, W5.08
  matrix, W3.03 grounding, Tier B four prohibitions, NOT-RUN wording).
  Its coverage table finds NO uncovered design section.

## Findings and patch log

### H-1 (High) — Invented F-16 Phase 3 "exit gate"

Design §14 contains exactly three `**Exit gate:**` lines (F-16 Phase 1
line 834, F-16 Phase 2 line 844, F-17 Phase 0 line 861). The F-16
Phase 3 subsection (design lines 846–850) is three scope bullets only
— no exit gate. The ledger nevertheless presented one. Verified by the
patcher by grep before patching. Same defect class as W5.10 (fixed in
iteration 03 for F-17 Phase 1).

Patched:

- `wave-03-f16-phase-3/05_phase-3-scope-review-f16-handover.md`
  (renamed from `05_phase-3-exit-gate-review-f16-handover.md`):
  rewritten as a scope review — objective, deliverable and a new
  acceptance item all state that design §14 defines no F-16 Phase 3
  exit gate and that this review is not one; verification method
  restated as a scope checklist.
- `MASTER_LEDGER.md` §3 intro: the blanket "exit gate for each phase"
  claim restated — only phases that have one; F-16 Phase 3 and F-17
  Phase 1 have none; Wave 6 has none either (W6.08 is a close-out).
- `MASTER_LEDGER.md` §3 Wave 3 row: exit-gate cell restated as "No §14
  exit gate for Phase 3 — scope: … (W3.05 scope review)".
- `MASTER_LEDGER.md` §3 Wave 6 row: exit-gate cell restated as "No §14
  exit gate for later phases — scope per item: …".
- `MASTER_LEDGER.md` §7 item 1: exit-gate list is now (W0.08, W1.18,
  W2.08, W4.13); W3.05 and W5.10 scope reviews evidenced instead;
  W6.08 described as the close-out that verifies all of the above
  (it was wrongly listed as an exit-gate review subtask).
- `wave-06-f17-later-phases/08_roadmap-ledger-close-out-handover.md`
  (W6.08): Inputs now separate the exit-gate records (W0.08, W1.18,
  W2.08, W4.13) from the scope-review records (W3.05, W5.10); step 1,
  deliverable and acceptance 1 updated the same way.
- `README.md` step 4: the no-exit-gate note now covers F-16 Phase 3
  (W3.05) as well as F-17 Phase 1 (W5.10).
- `FINALIZATION.md` sign-off item 3: exit-gate list restated (Gate 0,
  F-16 P1–P2, F-17 P0; W3.05 and W5.10 are scope reviews; later phases
  have none — W6.08 is a close-out).

### M-1 (Medium) — Invented §5.5 EXPIRED/SUSPENDED handling in W1.08

`wave-01-f16-phase-1/08_assessment-lifecycle-state-machine.md` step 5
claimed "assessment-level expiry is handled by the Incomplete/
RevisionRequired loops". Design §5.1 defines no expiry concept;
Incomplete and RevisionRequired are missing/invalid-input and
assessor-revision correction loops. Verified by the patcher against
W1.08 and design §5.1 before patching.

Patched: the parenthetical is deleted. The "no §5.1 counterpart"
clause (correct) remains.

### M-2 (Medium) — W1.20 drops the pilot-portfolio condition

Design §1.3 item 4 (line 74): "Select the first F-16 pilot portfolio
and jurisdiction before building a tariff pack or a compliance-oriented
report." W1.20 step 1 mentioned the jurisdiction (G1) only. Verified
by the patcher against design line 74 before patching.

Patched (`wave-01-f16-phase-1/20_f16-tariff-pack-design.md`): objective,
step 1, acceptance 1, verification method and risks now require BOTH
the pilot portfolio (D2/W1.02 pilot facility) and the jurisdiction
(G1/W0.06); Inputs and Dependencies now include W0.04 and W1.02
(Dependencies now W0.04, W0.06, W1.02, W1.11).

### M-3 (Medium) — W4.04 misattributes the FINAL G7 decision to W4.10

`wave-04-f17-phase-0/04_prototype-pv-only-pv-battery-real-datasets.md`
step 5 said "the final G7 decision is recorded later in W4.10".
W4.10's own objective states it records "an interim position only"
and "the final decision is deferred"; the final decision is owned by
W5.12. Verified by the patcher against both files before patching.

Patched: step 5 now reads "the final G7 decision is recorded later in
W5.12 (W4.10 records an interim position only)".

### M-4 (Medium) — G1 revisit trigger specified in MASTER_LEDGER but absent from W1.02

`MASTER_LEDGER.md` sequencing note (§6): "If the archetype genuinely
constrains the jurisdiction, that is recorded as a G1 revisit trigger
in W1.02, not as a hidden dependency." Grep of W1.02 found zero
"revisit" hits. Verified by the patcher before patching.

Patched (`wave-01-f16-phase-1/02_first-customer-archetype-pilot-jurisdiction-facility.md`):
new step 5 records the G1 revisit trigger (an archetype that constrains
the jurisdiction is a revisit trigger for W0.06, not a silent change);
new acceptance item; deliverable now includes the G1 revisit trigger.

### L-1 (Low) — Stale "benchmark record" in W6.01

Iteration 03 deleted the per-pack PVWatts/REopt/SAM benchmark step
from W6.01, but its deliverable still ended "and a benchmark record"
and its verification method still said "benchmark records inspected".
No step produces a benchmark record. Verified by the patcher before
patching.

Patched (`wave-06-f17-later-phases/01_more-jurisdiction-packs.md`):
both lines cleaned; deliverable and verification method no longer
mention benchmark records.

### L-2 (Low) — Stale "local-expert review rule" in W6.07 deliverable

Iteration 03 deleted the local-expert-review-before-offer rule from
W6.07, but its deliverable still listed "local-expert review rule".
Verified by the patcher before patching.

Patched (`wave-06-f17-later-phases/07_regional-pack-review-expiry-governance.md`):
deliverable now lists only the expiry labelling rule, re-validation
triggers and the per-pack governance record.

### L-3 (Low) — FINALIZATION / MASTER open-items lists omit W5.12 as final G7 owner

PARTIAL REBUTTAL. The FINALIZATION limb is real: `FINALIZATION.md:220`
listed G7's owning subtasks as W4.10 only. Patched: now "W4.10 interim
+ W5.12 final". The MASTER_LEDGER limb was NOT found: `grep -n "W4.10"
MASTER_LEDGER.md` returns only §5 line 175, which already reads
"W4.10 (interim position) + W5.12 (final decision)". No W4.10-only G7
list exists in MASTER_LEDGER.md (its §8 open-items bullets name no
owner IDs for G7). The MASTER half of L-3 is therefore rebutted with
that evidence; no MASTER edit was needed for it.

### L-4 (Low) — MASTER_LEDGER §4 overstates Wave 2's consumption of W1.03

`MASTER_LEDGER.md` §4 said "Wave 2 consumes the W1.02 pilot
definition, the W1.03 asset-integration decision, and the W1.06
records." No W2 subtask lists W1.03 in Dependencies (W2.01: W1.02,
W1.06, W1.18; W2.03: W1.06, W1.08; W2.05: W2.03, W2.04). Only W3.03
(Dependencies: W1.03, W2.08) consumes it. Verified by the patcher by
grep before patching.

Patched: §4 now says Wave 2 consumes the W1.02 pilot definition and
the W1.06 records, and the W1.03 asset-integration decision is consumed
by Wave 3 (W3.03), not by Wave 2.

### L-5 (Low) — Dependencies lines omit inputs the subtask text consumes

Verified by the patcher against each cited file before patching. All
five fixed:

- **W2.02** (`wave-02-f16-phase-2/02_measure-scenarios-economics.md`):
  Inputs cited W1.06 and W1.07; Dependencies listed only W2.01.
  Dependencies now W2.01, W1.06, W1.07, W1.20; Inputs now also cite
  the W1.20 tariff pack (fixing the reverse mismatch: W1.20 step 4
  names W2.02 as a tariff-pack consumer, but W2.02 never listed W1.20).
- **W2.06** (`wave-02-f16-phase-2/06_onboarding-draft-human-approval.md`):
  Inputs cited W1.12 and W1.04 and the verification method checks the
  approval against the W1.04 authorization matrix; Dependencies listed
  only W2.03, W2.04, W2.05. Dependencies now add W1.04, W1.12.
- **W3.02** (`wave-03-f16-phase-3/02_ipmvp-based-efficiency-savings-mv-workflow.md`):
  Inputs cited "W0.03 D4 record (IPMVP license/terms)"; Dependencies
  listed only W3.01. Dependencies now W3.01, W0.03.
- **W6.03** (`wave-06-f17-later-phases/03_local-roof-imagery-providers-verified-installer-quotes.md`):
  step 4 requires "the D4 position for each from W0.03"; Dependencies
  omitted W0.03. Dependencies now add W0.03; Inputs now cite the W0.03
  D4 record.

### L-6 (Low) — Two small trace/coverage omissions

- **W0.05** (`wave-00-gate-0/05_record-section-0-executive-decisions-baseline.md`):
  Design trace omitted §2.3 although Inputs cite "§2.3 (no premature
  shared library)". Patched: trace line now reads "Design §0; §2.3;
  §15 ADR-SHARED-01; foundations.md final recommendation (launch
  sequencing)".
- **W5.01** (`wave-05-f17-phase-1/01_consumer-auth-separate-data-store-product-scope.md`):
  never restated design §11.1's closing prohibition (design line 673:
  "Do not build a generalized plugin system, installer CRM or
  microservice fleet for the first user test"). Verified by the patcher
  against design line 673 before patching. Patched: new step 6 states
  the prohibition, new acceptance item checks it, deliverable includes
  it.

### L-7 (Low) — W5.03 step/acceptance mismatch on the G7 trigger

`wave-05-f17-phase-1/03_scenario-comparison-grid-pv-pv-battery.md`
acceptance 5 requires "the W4.06 demand validation is named as the
deferred decision's trigger", but step 5 only said the decision is
recorded "after the model is understood and user demand is validated"
without naming W4.06. Verified by the patcher before patching.

Patched: step 5 now reads "…it is recorded only after the model is
understood and user demand is validated — the W4.06 demand validation
is the deferred decision's trigger — per design §10.4 step 6."

## Counts after the iteration-04 patch pass

No subtasks added, removed or merged (W3.05 renamed in place). Total
stays **76 subtasks** (10/20/8/5/13/12/8). Three new acceptance boxes
(W1.02 G1 revisit trigger, W5.01 §11.1 prohibition, W3.05 no-exit-gate
statement): 336 → **339 unchecked boxes**, 0 checked.

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
iterations 02–03): 76 defined subtasks, 0 orphan refs, exactly the 12
same-wave later-number dependencies listed in MASTER_LEDGER §4
(W0.08→W0.09, W0.08→W0.10, W1.10→W1.20, W1.18→W1.19, W1.18→W1.20,
W4.04→W4.07, W4.04→W4.08, W4.04→W4.09, W4.05→W4.08, W5.10→W5.11,
W5.10→W5.12, W6.05→W6.06), 0 cycles. The new Dependencies added in
this pass (W1.20→W0.04/W1.02, W2.02→W1.06/W1.07/W1.20,
W2.06→W1.04/W1.12, W3.02→W0.03, W6.03→W0.03) are all cross-wave or
earlier-numbered same-wave, so the later-number list is unchanged.

## Rebuttals

One partial rebuttal: L-3's MASTER_LEDGER limb (see L-3 above) — no
W4.10-only G7 owner list exists in MASTER_LEDGER.md; §5 line 175
already gives "W4.10 (interim position) + W5.12 (final decision)".
Every other finding was patched as instructed after checking the cited
file and line.

## Remaining gaps (honest)

1. Design-doc claims (BuildingSync v2.7.0, ASHRAE 211, IPMVP 2022,
   PVWatts V8, REopt API v3) were not re-verified against primary
   sources in this pass either; FINALIZATION.md still defers that to
   implementation time.
2. The FINALIZED stamp stays suspended until an independent re-review
   of the patched ledger returns zero findings.
