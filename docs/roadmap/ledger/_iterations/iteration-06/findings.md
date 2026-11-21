# Iteration 06 — Independent re-review + patch pass (2026-11-21)

## Re-review verdict

- **Re-reviewer:** independent subagent who did not build or patch the
  ledger. It read the design doc (§0–§17) and foundations.md in full
  against all 76 subtask files plus the master docs, re-deriving
  findings from scratch (coverage table, traces opened at the cited
  design sections, grep recounts, dependency-graph check).
- **Verdict: 6 findings — 0 High, 1 Medium, 5 Low. NOT zero findings.
  FINALIZED stays suspended.**
- The re-reviewer verified the iteration-05 headline patches all
  present and found NO uncovered design section.

## Findings and patch log

### M-1 (Medium) — §11.3 API-authorization controls defined normatively in TWO subtasks

Four §11.3 controls (explicit owner authorization, IDOR protection,
rate limits, CSRF protection) were stated as normative rules in both
W5.06 step 1 and W5.07 step 2 (+ W5.07 acceptance 2), contradicting
W5.08 acceptance 3 ("No §11.3 control is defined as a normative rule
in more than one subtask"). That acceptance was unsatisfiable as
written, and W5.08 step 1 could not name a single defining subtask for
those four rows ("W5.06 or W5.07"). Verified by the patcher against
W5.06, W5.07 and W5.08 before patching.

Patched (W5.06 is now the single definer; W5.07 applies by reference):

- `wave-05-f17-phase-1/06_data-export-deletion-consent-controls.md`
  (W5.06): step 1 now states the four authorization controls are
  defined here and W5.07 applies them by reference without redefining
  them; new acceptance item checks the four controls are defined here.
- `wave-05-f17-phase-1/07_api-surface.md` (W5.07): step 2 now applies
  the §11.3 authorization controls defined in W5.06 to every route and
  states they are defined in W5.06, not restated here; acceptance 2
  now checks every route is subject to the W5.06-defined controls;
  deliverable, verification method and risks reworded to the same
  define-vs-apply distinction.
- `wave-05-f17-phase-1/08_privacy-security-design.md` (W5.08):
  objective and step 1 now name the single defining subtask per row
  (W5.06 for data controls and the four API-authorization controls;
  W5.07 for the route table and per-route application). Acceptance 3
  is now satisfiable.

### L-1 (Low) — W1.12 reproduces §6.2 route purposes only, no method+path strings

`wave-01-f16-phase-1/12_api-surface-events-async-work.md` step 1
reproduced the §6.2 routes table "with purpose per route" only, and
acceptance 1 required only that the 12 routes "appear with their
stated purpose". Same defect class iteration 03 fixed in W5.07 for the
11 §11.2 routes; W1.12 never got the same treatment. Verified by the
patcher against design §6.2 (12 exact method+path strings) before
patching.

Patched: step 1 now reproduces all 12 §6.2 routes as method+path
strings with purpose (verified against the design §6.2 table:
`POST /api/v1/facility-assessments`,
`GET /api/v1/facility-assessments/{id}`,
`POST /api/v1/facility-assessments/{id}/uploads`,
`POST /api/v1/facility-assessments/{id}/bills`,
`POST /api/v1/facility-assessments/{id}/meter-imports`,
`POST /api/v1/facility-assessments/{id}/buildingsync-imports`,
`POST /api/v1/facility-assessments/{id}/equipment-observations`,
`POST /api/v1/facility-assessments/{id}/analysis-runs`,
`GET /api/v1/facility-assessments/{id}/analysis-runs/{run_id}`,
`POST /api/v1/facility-assessments/{id}/flexibility-candidates/{candidate_id}/review`,
`POST /api/v1/facility-assessments/{id}/asset-onboarding-drafts`,
`POST /api/v1/asset-onboarding-drafts/{id}/approve`); acceptance 1
now requires method+path strings.

### L-2 (Low) — W2.06 paraphrases §13.5 item 3 instead of stating it or marking it N/A

Design §13.5 item 3: "which tariff, weather and equipment model
versions were used". W2.06 step 5 rendered it as "which model versions
were used" — neither stated nor recorded as N/A with a reason, unlike
the two items it does mark N/A. W1.10 step 5 and W2.02 step 6 state it
as "which tariff/weather/equipment model versions were used" and W5.05
step 3 states it in full. Verified by the patcher by grep before
patching.

Patched (`wave-02-f16-phase-2/06_onboarding-draft-human-approval.md`
step 5): now reads "which tariff/weather/equipment model versions were
used", matching W1.10/W2.02. The two N/A justifications (solver status,
financial conclusion) are unchanged.

### L-3 (Low) — W6.04 names W5.01 in step and verification, omits it from Dependencies

`wave-06-f17-later-phases/04_financing-incentive-support-reliable-data.md`
step 4 ("not financing underwriting or a loan marketplace (W5.01
non-goals)") and verification ("non-goal checked against W5.01")
consume W5.01, but Dependencies were only W4.02, W5.04, W5.10.
Verified by the patcher before patching; adding it creates no cycle
(W5.01 depends on W4.12, W4.13, W0.09 only).

Patched: Dependencies now W4.02, W5.01, W5.04, W5.10.

### L-4 (Low) — W5.08 verification method names W4.12, omits it from Dependencies

`wave-05-f17-phase-1/08_privacy-security-design.md` verification method
checks the boundary "against ADR-F17-01 (W4.12)", but Dependencies
were only W5.06, W5.07. Verified by the patcher before patching;
adding it creates no cycle (W4.12 depends on W0.05 only).

Patched: Dependencies now W5.06, W5.07, W4.12.

### L-5 (Low) — The Dependencies-completeness standard is breached by forward references in ~19 files

The re-reviewer's mechanical scan found ~19 files naming subtask IDs
in Inputs/steps/acceptance that do not appear in Dependencies. Almost
all are downstream/forward references (the named subtask consumes this
one, or is a later consumer/test-oracle), and adding them as
Dependencies would create cycles (e.g. W0.09→W1.11, W1.20→W1.10,
W5.03→W5.12, W6.01→W6.07, W6.06→W6.05). The correct fix is NOT to add
the dependencies — it is to document the forward-reference exception
class so the stated standard matches the ledger. L-3/L-4 above are the
only two upstream-consumed omissions in the set, scored separately.

Note: no "one documented exception" claim exists in the ledger itself
(patcher grepped MASTER_LEDGER.md, README.md and FINALIZATION.md —
zero hits); the phrase was in the re-review brief, not in a ledger
file. The ledger simply never stated the forward-reference rule at
all, which is what this fix corrects.

Patched:

- `README.md` step 3: Dependencies list prerequisites only (upstream
  inputs a subtask consumes). A subtask ID named in Inputs, steps or
  acceptance that is a downstream consumer, a test-oracle, or an
  affected subtask evidencing this subtask's output is a forward
  cross-reference, not a prerequisite, and must NOT be added to
  Dependencies — adding it would create a cycle (examples: W0.09
  consumed by W1.11, W1.20 consumed by W1.10, W5.03 consumed by W5.12,
  W6.01 consumed by W6.07). W4.07 step 1 is cited as the in-ledger
  precedent (the W4.04 prototype is a later verification consumer of
  the tier definitions, not a prerequisite for them).
- `MASTER_LEDGER.md` §4: same rule added after the same-wave
  later-number paragraph, with the full example list (W0.09→W1.11/
  W5.01, W1.17→W2.01/W2.02/W2.03/W2.05/W2.08, W1.20→W1.10/W2.02,
  W5.03→W5.12, W6.01→W6.07, W6.06→W6.05) and the W5.08↛W5.09 case
  named as an instance of the class (W5.09 tests the controls W5.08
  traces).

## Counts after the iteration-06 patch pass

No subtasks added, removed, merged or renamed. Total stays
**76 subtasks** (10/20/8/5/13/12/8). One new acceptance box (W5.06
four-controls item): **340 unchecked boxes**, 0 checked.

## Verification (actual output, 2026-11-21, run from the ledger directory)

```
$ grep -r -c "Acceptance criteria" wave-*/ | awk -F: '{s+=$2} END {print s}'
76
$ grep -r -h "^# W" wave-*/ | wc -l
76
$ grep -r -h "  - [ ]" wave-*/ | wc -l
340
$ grep -r "  - [x]" wave-*/ | wc -l
0
```

Dependency-graph check (`/tmp/ledger_iter03_graph.py`, same method as
iterations 02–05): 76 defined subtasks, 0 orphan refs, exactly the 12
same-wave later-number dependencies listed in MASTER_LEDGER §4
(W0.08→W0.09, W0.08→W0.10, W1.10→W1.20, W1.18→W1.19, W1.18→W1.20,
W4.04→W4.07, W4.04→W4.08, W4.04→W4.09, W4.05→W4.08, W5.10→W5.11,
W5.10→W5.12, W6.05→W6.06), 0 cycles. The Dependencies added in this
pass (W6.04→W5.01, W5.08→W4.12) are cross-wave and acyclic.

## Rebuttals

None. Every finding was patched as instructed after checking the cited
file and line. (L-5's "one documented exception claim" wording was
traced to the re-review brief rather than to any ledger file; the
underlying gap — the forward-reference rule was unstated — was real
and is now documented. That is a correction of the finding's premise,
not a rebuttal of the finding.)

## Remaining gaps (honest)

1. Design-doc claims (BuildingSync v2.7.0, ASHRAE 211, IPMVP 2022,
   PVWatts V8, REopt API v3) were not re-verified against primary
   sources in this pass either; FINALIZATION.md still defers that to
   implementation time.
2. The FINALIZED stamp stays suspended until an independent re-review
   of the patched ledger returns zero findings.
