# Iteration 03 — Adversarial audit + patch pass (2026-11-21)

## Audit verdict

- **Auditor:** independent subagent who did not build the ledger. It read
  the design doc (§0–§17) and `foundations.md` in full against all 72
  subtask files plus the master docs, section by section, checking
  coverage, invented work, acceptance testability, duplicates,
  wrong-scope items and trace errors (≥20 traces opened at the cited
  design sections).
- **Verdict: 45 findings — 3 High, 17 Medium, 25 Low. FINALIZED suspended.**
- **Coverage table and the 8-item "exactly what would have to change"
  list** were delivered with the audit report to the parent agent.

## Honest note on what the patcher could and could not see

The auditor's full 45-finding text (finding IDs A-F4–A-F8, A-F9–A-F21,
B-F3–B-F8, B-F9–B-F22, M1 with their cited file+line evidence) was
delivered as a handoff to the parent agent and is NOT in the transcript
the patcher received. The patch therefore proceeded from:

1. The parent agent's 8 itemized patch instructions, which quote the
   verified findings and the exact fixes required; and
2. The patcher's own file-by-file verification of every cited file and
   line before editing it, plus its own sweep for stragglers.

Findings verified by ID against the files before patching:

- **A-F3 (High, parent-verified):** design §5.1 and §5.5 define two
  different candidate lifecycles; W1.08 vs W2.03 each claim theirs as
  the test oracle with no mapping. Patched (item 1).
- **B-F1 (High, parent-verified):** W6.06 line 2 granted "unless
  explicitly requires" exceptions while its own acceptance and risks
  said the §12 field list is closed. Patched (item 2).
- **B-F2 (High, parent-verified):** W5.03 step 5 recorded the final G7
  decision "before the engine is productized" inside the subtask that
  IS the productization, and W4.06 was consumed but missing from
  Dependencies. Patched (item 3).
- **A-F1 (patcher-verified by grep):** design §13.2 bullet 4 absent
  from Waves 0–3. Patched (item 4: W1.07).
- **A-F2 (patcher-verified by grep):** "readiness report" zero hits in
  Waves 0–3. Patched (item 4: new W1.19).
- **B-F18 (patcher-verified by grep):** "simultaneously" zero hits in
  the ledger — the foundations.md rule "do not launch both
  simultaneously" was nowhere stated. Patched (item 4: W0.05, and
  MASTER_LEDGER §3 note).

The Low findings by ID (A-F9–A-F21, B-F9–B-F22, M1) were not
individually visible to the patcher. They were addressed through the
parent's items 4–7 (each item names the files and the fix) and the
patcher's own sweep (see "Patcher's own sweep" below). The independent
re-review — briefed with the same audit checks, re-deriving findings
from scratch — is the backstop for any Low the patcher could not see
by ID. No finding in the 8 items was found wrong on checking; **zero
findings were rebutted**.

## Patch log (by the parent agent's items 1–8)

### Item 1 — Reconcile the two candidate lifecycles (A-F3)

- `wave-01-f16-phase-1/08_facility-candidate-lifecycle.md` (W1.08):
  objective, step 4 and acceptance now state the §5.1↔§5.5 mapping
  explicitly — §5.5 DISCOVERED/EVIDENCE_INCOMPLETE precede §5.1
  FlexibilityReview; EXPIRED/SUSPENDED have no §5.1 counterpart;
  REVOKED maps to §5.1 REJECTED (as it did before).
- `wave-02-f16-phase-2/03_flexibility-discovery-lifecycle.md` (W2.03):
  step 6 and acceptance now state that the §5.5 lifecycle in this
  subtask is the SINGLE test oracle for qualification transitions, and
  the §5.1↔§5.5 mapping is given in the objective.
- Mapping is partial by design: the two lifecycles are not the same
  set of states, and the ledger now says so instead of pretending
  otherwise.

### Item 2 — Delete the W6.06 exception clauses (B-F1)

- `wave-06-f17-later-phases/06_optional-vpp-enrollment-handoff.md`
  (W6.06): step 2 rewritten — the §12 field list is closed; bills,
  interval data, raw addresses and occupancy data are excluded with no
  exception. Acceptance now states "closed list; no exceptions".
  Risks reworded to match. Objective notes the deletion and its reason.

### Item 3 — Fix G7 sequencing (B-F2)

- `wave-05-f17-phase-1/03_scenario-comparison-grid-pv-pv-battery.md`
  (W5.03): step 5 rewritten — the final G7 decision is deferred to
  W5.12; this subtask consumes only the W4.10 interim position. New
  step 6 carries the Tier B restriction into the engine. Dependencies
  now W5.02, W4.06, W4.07, W4.09, W4.10.
- `wave-04-f17-phase-0/04_prototype.md` (W4.04): Dependencies now
  W4.02, W4.07, W4.08, W4.09.
- `wave-04-f17-phase-0/05_benchmark-against-pvwatts-reopt-sam.md`
  (W4.05): Dependencies now W4.04, W4.08.
- New `wave-05-f17-phase-1/12_record-final-g7-decision.md` (W5.12):
  records the final G7 decision per design §10.4 step 6 (after the
  model is understood and demand validated). Dependencies W5.03,
  W4.06, W4.10; evidenced by W5.10.
- `wave-04-f17-phase-0/06_consuming-prototype-outputs-and-g7.md`
  (W4.06): objective notes it is now also consumed by W5.03.
- MASTER_LEDGER §5: G7 owner is now "W4.10 (interim position) +
  W5.12 (final decision)".

### Item 4 — Missing coverage added as new or extended subtasks

New subtasks (all unchecked, continuing their wave sequences; existing
IDs never renumbered):

- **W0.10** `wave-00-gate-0/10_decide-f16-data-retention-consent-g8.md` —
  F-16 G8 retention/consent decision (design §16 G8; the F-17 side is
  W4.11). Dependencies W0.06; evidenced by W0.08; consumed by W1.14.
- **W1.19** `wave-01-f16-phase-1/19_phase-1-readiness-report.md` —
  design §14 F-16 Phase 1 readiness report (data completeness, missing
  evidence, interval quality, boundaries). Dependencies W1.09, W1.10;
  evidenced by W1.18.
- **W1.20** `wave-01-f16-phase-1/20_f16-tariff-pack-design.md` — F-16
  tariff pack (design §1.3 item 4 + §6.1 tariff_data port), built only
  after the first jurisdiction is selected (G1). Dependencies W0.06,
  W1.11; worked before W1.10 in execution order.

Extended subtasks:

- **W1.07** `wave-01-f16-phase-1/07_provenance-quality-uncertainty-verification.md`:
  §4.3 source-kind enumeration (9 kinds) and verification-state
  enumeration (6 states) now required; §13.2 bullet 4 (keep model
  evaluation datasets and hand-calculated reference cases separate
  from live user data) added to step 4 and acceptance (A-F1).
- **W1.06** `wave-01-f16-phase-1/06_domain-model-records-invariants.md`:
  ALL §4.2 invariants now required (previously "at least one invariant
  each"); all 15 records' invariants spelled out; new step 5 defines
  the QualificationDecision fields (rule-set version, result, reason).
- **W1.10** `wave-01-f16-phase-1/10_energy-baseline-consumption-analysis.md`:
  Dependencies now include W1.20; tariff/billing window sourced from
  the W1.20 pack.
- **W1.14** `wave-01-f16-phase-1/14_persistence-object-storage-rls.md`:
  Dependencies now include W0.10; retention periods and deletion
  behaviour follow the W0.10 G8 record.
- **W2.02** `wave-02-f16-phase-2/02_measure-scenarios-economics.md` and
  **W2.06** `wave-02-f16-phase-2/06_qualification-approval-flow.md`:
  §13.5 disclosure steps and acceptances added for measure scenarios
  and qualification/approval outputs (W1.10 already covered §13.5 for
  baseline results only).
- **W5.02** `wave-05-f17-phase-1/02_property-bill-interval-import-tariff-roof-model.md`:
  new step 5 — design §8.2 step 1 address explanation + manual
  location fallback.
- **W5.05** `wave-05-f17-phase-1/05_immutable-calculation-report.md`:
  new step 6 — design §8.2 step 8 next-steps content as a required
  report item.
- **W0.05** `wave-00-gate-0/05_baseline-rules.md`: new step 6 —
  launch-sequencing rule: F-16 first; F-17 stays a research candidate
  until its Phase 0 prototype demonstrates real demand; do not launch
  both simultaneously (B-F18; foundations.md §8 + design §14).
- **W4.11** `wave-04-f17-phase-0/11_data-retention-consent-decisions.md`:
  "Owns G8" line now states it owns G8 for the F-17 launch
  jurisdiction; the F-16 side is W0.10.
- MASTER_LEDGER §5: G8 owner is now "W0.10 (F-16) + W4.11 (F-17)".

### Item 5 — Traces and gates

- **W5.11** `wave-05-f17-phase-1/11_section-8-2-user-journey-trace.md`:
  step-5 trace now points to W5.02 (PVSystemScenario/BatteryScenario
  record definitions), not W5.03; Inputs updated to W5.02 steps 1–5;
  stale "exit-gate review" references restated as the W5.10 scope
  review.
- **W5.07** `wave-05-f17-phase-1/07_api-surface.md`: step 1 now
  reproduces all 11 design §11.2 routes as method+path strings with
  purpose; acceptance requires method+path strings. (Routes verified
  against design §11.2: `POST /api/v1/home-plans`,
  `PUT /api/v1/home-plans/{id}/property`,
  `POST /api/v1/home-plans/{id}/usage-imports`,
  `POST /api/v1/home-plans/{id}/roof-assessments`,
  `GET /api/v1/tariff-options`,
  `POST /api/v1/home-plans/{id}/scenarios`,
  `POST /api/v1/home-plans/{id}/scenario-runs`,
  `GET /api/v1/home-plans/{id}/scenario-runs/{run_id}`,
  `POST /api/v1/home-plans/{id}/share-consents`,
  `DELETE /api/v1/home-plans/{id}/share-consents/{consent_id}`,
  `DELETE /api/v1/home-plans/{id}`.)
- **W0.09** `wave-00-gate-0/09_section-2-1-logical-architecture-cross-product.md`:
  step 4 and acceptance now claim only the W1.11 cross-reference; the
  W5.01 cross-reference is owned by W5.01 (step 5) in Wave 5.
- **W6.08** `wave-06-f17-later-phases/08_roadmap-ledger-close-out-handover.md`:
  spurious foundations §7 trace removed from the Design trace; step 1
  and acceptance now verify the five real exit gates plus the F-17
  Phase 1 scope review (design §14 defines no F-17 Phase 1 exit gate);
  counts updated 72 → 76.
- **W5.10** `wave-05-f17-phase-1/10_phase-1-scope-review-f17.md`
  (renamed from `10_phase-1-exit-gate-review-f17.md`): rewritten as a
  scope review. Design §14 defines NO F-17 Phase 1 exit gate — the
  file now says so in its objective, deliverable and acceptance, and
  records per-item verdicts plus Wave 6 start conditions instead of an
  exit-gate verdict. Dependencies now include W5.12; Inputs W5.01–W5.12.
  The old filename's only remaining references are in the historical
  iteration-01 findings log, which is a record of what was true then
  and was deliberately not edited.
- **W2.08** `wave-02-f16-phase-2/08_phase-2-exit-gate-review.md`: new
  step 6 and acceptance item evidence ADR-F16-05 as filed by W2.07
  (owners and proposed status).

### Item 6 — Duplication and inventions

- **W5.08** `wave-05-f17-phase-1/08_privacy-security-design.md`:
  rewritten (not merged) as a §11.3 control-coverage verification
  matrix — every §11.3 control traced to its defining subtask (W5.06
  or W5.07) and its verification, with no control defined twice and
  none uncovered; the no-path-to-FlexCore-dispatch boundary verified
  against ADR-F17-01. The rewrite was chosen over folding because the
  ledger's "never renumber existing IDs" convention makes a merge
  costlier than a rewrite with unique checkable content.
- **W3.03** `wave-03-f16-phase-3/03_bms-ems-edge-interval-integrations-pilot.md`:
  "read-only" and "simulation-only" constraints deleted; the surviving
  constraint — no dispatch authority or control write — is grounded in
  design §0 product boundary and §5.5.
- **W6.07** `wave-06-f17-later-phases/07_regional-pack-review-expiry-governance.md`:
  the "local-expert review before offer" rule is deleted — verified to
  have NO basis ("local energy-market expert" appears only as an
  ADR-F17-02 owner role in design §15). Expiry labelling and
  re-validation are re-grounded in foundations.md §7 item 3 (validity
  date per pack item) and the W4.02 source discipline; Dependencies now
  include W4.02.
- **W6.01** `wave-06-f17-later-phases/01_more-jurisdiction-packs.md`:
  the per-pack PVWatts/REopt/SAM benchmark step is deleted — W4.05's
  benchmark is Phase 0 prototype-specific, not a per-pack rule;
  Dependencies no longer include W4.05; "Phase 1 exit" input reference
  restated as the W5.10 scope review.

### Item 7 — Acceptance tightening

- **W4.04** `wave-04-f17-phase-0/04_prototype.md`: Tier B now lists all
  four design §9.1 prohibitions (precise battery size, hourly
  self-consumption, peak-shaving savings, optimized dispatch result);
  the duplicated phrase ("measured hourly self-consumption" AND
  "hourly self-consumption") is removed.
- **W5.09** `wave-05-f17-phase-1/09_f17-test-plan.md` and **W1.17**
  `wave-01-f16-phase-1/17_f16-test-plan.md`: an item that could not be
  run is now recorded as NOT RUN with reason and owner and does NOT
  count toward the exit check (it blocks the gate or is carried as an
  open item); previously the wording let an unrun test satisfy the
  check.
- **W4.09** `wave-04-f17-phase-0/09_roof-solar-resource-provider-seams-prototype.md`:
  new acceptance item — the D4 license/terms position for each
  provider (PVWatts, PVGIS, REopt) is recorded from W0.03.
- **W5.03**: Tier B restriction carried into the engine (new step 6);
  see item 3.

### Patcher's own sweep (Low findings by ID not visible — see note above)

- Stale "exit-gate" references to W5.10 found and fixed in W5.11
  (objective, risks) and W6.08 (inputs, step 1, deliverable,
  acceptance) after the W5.10 rename.
- "certified" hits re-checked: exactly 3 exact-word hits, all
  negations (W1.15); "certification" hits are also all negations
  (W1.01, W1.15, W5.01).
- No "72 subtasks" leftovers in wave files; no " working;" claims in
  wave files.

### Item 8 — Counts reconciled

- New total: **76 subtasks** (W0: 10, W1: 20, W2: 8, W3: 5, W4: 13,
  W5: 12, W6: 8). Four added (W0.10, W1.19, W1.20, W5.12); none
  removed or merged (W5.08 rewritten in place, W5.10 renamed in place).
- MASTER_LEDGER.md updated: banner (UNDER REVISION, 76, 79 files),
  wave map (W0/W1/W5 goals and ranges), dashboard (0/76), §3 Wave 5
  exit-gate cell restated as scope review + launch-sequencing note,
  Wave 6 entry gate restated as the W5.10 scope review, §4 same-wave
  later-number dependencies 4 → 12, §5 G7/G8 owners, §7 item 1,
  §8 review status.
- README.md updated: 72 → 76 (two places), review line, step 3
  later-number list (12), step 4 W5.10 scope-review note.
- FINALIZATION.md updated: status UNDER REVISION, version v1.1
  (patched), scope 76 subtasks / 79 ledger files + 3 review files,
  totals table, recount block, iteration-03 review record, sign-off
  checklist RESET to unchecked pending re-review, review-history and
  checkbox-granularity counts (336 boxes), remaining-open-items item 0
  (re-review pending).

## Verification (actual output, 2026-11-21, run from the ledger directory)

```
$ grep -r -c "Acceptance criteria" wave-*/ | awk -F: '{s+=$2} END {print s}'
76
$ grep -r -h "^# W" wave-*/ | wc -l
76
$ grep -r -h "  - [ ]" wave-*/ | wc -l
336
$ grep -r "  - [x]" wave-*/ | wc -l
0
```

Dependency-graph check (`/tmp/ledger_iter03_graph.py`, same method as
iteration 02): 76 defined subtasks, 0 orphan refs, exactly the 12
same-wave later-number dependencies listed in MASTER_LEDGER §4
(W0.08→W0.09, W0.08→W0.10, W1.10→W1.20, W1.18→W1.19, W1.18→W1.20,
W4.04→W4.07, W4.04→W4.08, W4.04→W4.09, W4.05→W4.08, W5.10→W5.11,
W5.10→W5.12, W6.05→W6.06), 0 cycles.

## Rebuttals

None. Every finding in the parent agent's 8 items was patched as
instructed after checking the cited file and line; no finding was found
wrong on checking.

## Remaining gaps (honest)

1. The Low findings by ID were not individually visible to the patcher
   (see the note at the top). They were addressed through items 4–7 and
   the patcher's own sweep; the independent re-review must re-derive
   findings from scratch.
2. Design-doc claims (BuildingSync v2.7.0, ASHRAE 211, IPMVP 2022,
   PVWatts V8, REopt API v3) were not re-verified against primary
   sources in this pass either; FINALIZATION.md still defers that to
   implementation time.
3. The FINALIZED stamp stays suspended until an independent re-review
   of the patched ledger returns zero findings.
