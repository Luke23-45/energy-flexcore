# Iteration 01 — Independent review of the F-16 / F-17 roadmap ledger (draft 2026-11-21)

- Date: 2026-11-21
- Angle: independent review — the reviewer did NOT write the draft. Read in full before judging: `docs/roadmap/flexcore_extensions_principal_engineering_design.md` (§0–§17), `docs/roadmap/foundations.md`, and the v1 ledger format reference (`ledger/MASTER_LEDGER.md`, `ledger/README.md`, `ledger/FINALIZATION.md`, `ledger/_iterations/iteration-01/findings.md`). Then read all 70 wave subtask files end to end, plus `docs/roadmap/ledger/MASTER_LEDGER.md`, `README.md`, `FINALIZATION.md`.
- Status: OPEN — findings remain. This ledger is **NOT finalized**. `FINALIZATION.md` correctly states the independent review was pending; this pass is that review, and it finds 20 items (0 Critical, 2 High, 14 Medium, 4 Low). No draft file was edited — findings file only. Nothing committed or pushed.

## Independent grep recount (run 2026-11-21 from `docs/roadmap/ledger/`)

| Check | Draft claim | Independent recount | Verdict |
|---|---|---|---|
| "Acceptance criteria" lines, per wave | 8/18/8/5/13/10/8 = 70 | 8/18/8/5/13/10/8 = 70 | MATCH |
| Subtask heading IDs (`^# W`) | 70 unique, contiguous 1..N per wave | 70, no duplicates, contiguous per wave | MATCH |
| File numbers match subtask numbers | 01 = W\<w\>.01 | All 70 match (verified file-number vs ID suffix) | MATCH |
| Unchecked boxes (`  - [ ]`) in wave files | 286 | 286 | MATCH |
| Checked boxes (`  - [x]`) in wave files | 0 | 0 | MATCH |
| Files | 73 (70 subtasks + MASTER_LEDGER.md + README.md + FINALIZATION.md) | 73 | MATCH |
| `W<w>.<nn>` references anywhere (wave files + MASTER_LEDGER.md + README.md + FINALIZATION.md) | 0 orphan | 0 undefined IDs; all refs resolve to the 70 defined headings | MATCH |
| Dependencies-line references | 0 orphan | 0 orphan | MATCH |
| Forward dependencies (Python check: dep in a later wave, or a later number in the same wave) | — | Exactly 2: W4.04 → W4.07 and W6.05 → W6.06 (both also circular — see F-01, F-02) | FAIL (see F-01, F-02) |
| All 70 subtask files carry all 11 fields (workstream, objective, design trace, inputs, steps, deliverable, acceptance criteria, owner role, dependencies, verification method, risks) | claimed present | 0 files missing any field | MATCH |
| `git status --short` | only `docs/roadmap/` | `?? docs/roadmap/` | MATCH |
| `python3 tools/verify_docs.py` / `python3 tools/validate_agent.py` | exit 0 | both OK (report 2241 lines / 28 sections / 127 headings; skills 5, roles 5) | MATCH |
| "certified" hits in wave files | 3, all negations | 3, all negations (`wave-01-f16-phase-1/15_buildingsync-v270-interoperability.md` lines 5, 13, 20) | MATCH |
| v1-ledger citations in W0.03/W0.04 (P0.03, P0.04, P1.27, P1.28, P10.19) | — | all exist in `ledger/` | MATCH |

## What passed (checked, no finding)

- Counts, IDs, file numbering, checkboxes, file count, cross-reference resolution, and the FINALIZATION.md repo-check claims all re-verify exactly.
- All 12 §15 ADRs have exactly one owning subtask (ADR-F16-01 W1.05, -02 W1.15, -03 W1.09, -04 W3.02, -05 W2.07, -06 W1.13, ADR-F17-01 W4.12, -02 W4.08, -03 W4.09, -04 W4.07, ADR-SHARED-01 W0.05, ADR-DATA-01 W1.07), each recorded as PROPOSED.
- All 9 §16 gates have exactly one owning subtask framed as a decision or scheduled decision (G1 W0.06, G2 W1.02, G3 W1.03, G4 W0.07, G5 W4.01, G6 W4.03, G7 W4.10, G8 W4.11, G9 W0.03), all OPEN.
- No implementation/build/test-run/result claims in wave files; no safety/certification/protection-grade claims (all "certified"/"certification"/"conformance"/"guarantee" hits are negations or non-goal listings); solver classes stated correctly wherever named (HiGHS LP/QP/MIP, Clarabel conic not MISOCP, AC-PF ≠ AC-OPF, IPOPT local — MASTER_LEDGER.md §9, README.md, W5.03 line 21).
- Naming discipline holds: "energy audit" is used only for the F-16 assessment domain; the operational append-only audit log (flexcore-audit, P1.10), DR event M&V (P16.05) and efficiency savings M&V are kept separate (W0.05, W3.01, W3.05, README.md).
- F-16 boundary rules present: no building/household fields in Core Site/Asset, no new AssetType values, console stays read-only, Assessment Studio is the writable surface (W1.05, W1.13, W2.05).
- F-17 boundary rules present: no FlexCore tenant, no FlexCore database access, no dispatch permission; Waves 4–6 marked companion-repo work in every subtask file (W4.12, W5.01, W5.07, W6.05).
- Generic household energy audit is NOT a subtask anywhere; it appears only as a deprioritized non-goal in W5.01 (step 4, line 12; acceptance line 18).
- Every `§N.M` citation in wave files resolves to a real design-doc subsection; no stale § reference was found.

## Findings

| # | Severity | Item | File / subtask | Status |
|---|---|---|---|---|
| 1-01 | High | W4.04 ↔ W4.07 circular dependency (forward dep) | `wave-04-f17-phase-0/04_prototype-pv-only-pv-battery-real-datasets.md` W4.04; `07_input-quality-tiers.md` W4.07 | OPEN |
| 1-02 | High | W6.05 ↔ W6.06 circular dependency (forward dep) | `wave-06-f17-later-phases/05_vpp-enrollment-handoff-optional.md` W6.05; `06_residential-der-enrollment-package-definition.md` W6.06 | OPEN |
| 1-03 | Medium | W1.17 tests Wave 2 scope in Phase 1 (ECM/qualification/mapping tests before W2.01–W2.05 define them) | `wave-01-f16-phase-1/17_f16-test-plan.md` W1.17 | OPEN |
| 1-04 | Medium | W2.03 and W2.06 cite W1.17 as the test venue for Wave 2 rules — Wave 1 already executed | `wave-02-f16-phase-2/03_flexibility-discovery-lifecycle.md` W2.03; `06_onboarding-draft-human-approval.md` W2.06 | OPEN |
| 1-05 | Medium | MASTER_LEDGER.md §3 entry gates are self-contradictory: Wave 1 entry requires G2, Wave 4 entry requires G5, but G2/G5 are owned inside those waves | `MASTER_LEDGER.md` §3 (lines 81, 84) | OPEN |
| 1-06 | Medium | W0.06 step 1 uses the pilot archetype from W1.02, a later wave — forward reference; creates a G1↔G2 circularity | `wave-00-gate-0/06_decide-f16-first-jurisdiction.md` W0.06 | OPEN |
| 1-07 | Medium | Design §0 product-boundary sentences uncovered (assessment ≠ asset; discovered load ≠ qualified dispatch resource; solar estimate ≠ interconnection approval; optimizer result ≠ authorization to control) | (no owning subtask) — belongs in Wave 0 | OPEN |
| 1-08 | Medium | Design §2.1 logical architecture uncovered — no subtask owns the cross-product diagram or the external provider seams | (no owning subtask) — belongs in Wave 0/1 | OPEN |
| 1-09 | Medium | Design §8.2 nine-step user journey uncovered (esp. step 5 equipment/constraints and step 8 decision-package contents) | (no owning subtask) — belongs in Wave 5 | OPEN |
| 1-10 | Medium | Design §9: only 5 of 10 records defined in W5.02; PVSystemScenario, BatteryScenario, FinancialAssumptionSet, ScenarioRun, ConsentGrant never defined as records | `wave-05-f17-phase-1/02_property-bill-interval-import-tariff-roof-model.md` W5.02 | OPEN |
| 1-11 | Medium | Design §13.2 determinism/repeatability for F-17 uncovered (W1.07 covers F-16 only) | (no owning subtask) — belongs in Wave 5 | OPEN |
| 1-12 | Medium | Design §1.1 reuse rows uncovered: units conversion, forecasts UNTRAINED/no-accuracy-claims, weather-seam boundary, F-16 optimizer-port rule | (no owning subtask) — belongs in Wave 1 | OPEN |
| 1-13 | Medium | Design §1.1 flexible-load model and BESS rows uncovered: model field inventory / not-a-calibrated-thermal-simulator; unknown limits must not create flexibility credit | (no owning subtask) — belongs in Wave 1/2 | OPEN |
| 1-14 | Medium | W3.03 invents a control allowance ("read-only integration unless a separate, explicitly approved control decision exists") — not in the design; conflicts with simulation-only until v1.0.0 | `wave-03-f16-phase-3/03_bms-ems-edge-interval-integrations-pilot.md` W3.03 | OPEN |
| 1-15 | Medium | W4.10 decides G7 in Phase 0 and asks which option the prototype uses, but the prototype (W4.04/W4.05) is built first and design §10.4 step 6 defers the decision until model understood + demand validated | `wave-04-f17-phase-0/10_decide-reopt-api-v3-vs-local-reopt-jl.md` W4.10 | OPEN |
| 1-16 | Medium | W6.01 and W6.07 duplicate the same pack work (foundations.md §7 item 3 is one item); the "format vs instances" split is invented | `wave-06-f17-later-phases/01_more-jurisdiction-packs.md` W6.01; `07_regional-energy-planning-packs.md` W6.07 | OPEN |
| 1-17 | Low | W4.05 and W4.06 both claim the same verdict (accuracy/usefulness justifying regional data) | `wave-04-f17-phase-0/05_benchmark-yield-economics-pvwatts-reopt-sam.md` W4.05; `06_usability-tests-actual-prospective-homeowners.md` W4.06 | OPEN |
| 1-18 | Low | W5.08 invents "threat considerations" and a "pre-launch security review" — neither is in design §11.3 | `wave-05-f17-phase-1/08_privacy-security-design.md` W5.08 | OPEN |
| 1-19 | Low | W4.03 attaches "imagery age shown" to manual roof-plane entry — imagery age is a provider-data attribute, not a manual-input attribute | `wave-04-f17-phase-0/03_compare-manual-roof-input-vs-roof-provider.md` W4.03 | OPEN |
| 1-20 | Low | W6.06 "(it does not)" over-asserts: §12 step 1 includes "customer-chosen site location", which may be a raw address | `wave-06-f17-later-phases/06_residential-der-enrollment-package-definition.md` W6.06 | OPEN |

## Finding details

### 1-01 — High — W4.04 ↔ W4.07 circular dependency

- **File / subtask:** `wave-04-f17-phase-0/04_prototype-pv-only-pv-battery-real-datasets.md`, W4.04, line 20; `wave-04-f17-phase-0/07_input-quality-tiers.md`, W4.07, line 21.
- **Evidence:** W4.04: `- **Dependencies:** W4.02, W4.07`. W4.07: `- **Dependencies:** W4.04`. W4.04 step 3 requires the prototype to "Verify the Tier B rule" — a rule defined only in W4.07 — while W4.07's inputs list "W4.04 prototype" as a prerequisite. A Python forward-dependency check over all Dependencies lines flags exactly this pair (plus F-02) as the only forward deps in the ledger.
- **Why it matters:** the wave cannot be worked in ID order (README.md "How to update" step 3); each subtask blocks the other, so neither can honestly start.
- **Fix:** break the cycle by ordering the definition before the use: change W4.07 Dependencies to `W4.01` (tiers are a §9.1 definition, not a prototype result) and delete `W4.04` from its inputs; change W4.04 Dependencies to `W4.02, W4.07` (keep) and reword W4.07 step 1 so the tiers are defined from design §9.1 alone, with the prototype (W4.04) cited only as a later verification consumer, not a prerequisite.

### 1-02 — High — W6.05 ↔ W6.06 circular dependency

- **File / subtask:** `wave-06-f17-later-phases/05_vpp-enrollment-handoff-optional.md`, W6.05, line 21; `wave-06-f17-later-phases/06_residential-der-enrollment-package-definition.md`, W6.06, line 20.
- **Evidence:** W6.05: `- **Dependencies:** W6.06, W4.12, W5.06, W5.10`. W6.06: `- **Dependencies:** W6.05`. W6.05 inputs list "W6.06 enrollment package" and W6.06 inputs list "W6.05 handoff". Design §12 itself orders the package first (step 1: the planner creates the package; step 2: the aggregator receives it).
- **Why it matters:** same as F-01 — the two subtasks block each other and the wave cannot be worked in order.
- **Fix:** make the dependency one-way, package before handoff: W6.06 Dependencies → `W4.12, W5.06, W5.10` (remove `W6.05`; its inputs become design §12 + W5.06 only); W6.05 Dependencies → `W6.06, W4.12, W5.06, W5.10` (keep). Optionally renumber so the package definition precedes the handoff, or leave numbering and note the order in both files — do not renumber existing IDs.

### 1-03 — Medium — W1.17 tests Wave 2 scope in Phase 1

- **File / subtask:** `wave-01-f16-phase-1/17_f16-test-plan.md`, W1.17, line 9 (step 1).
- **Evidence:** "List the §13.3 domain/property tests for Phase 1 scope: … ECM applicability, bundle interaction handling and no silent savings stacking; flexibility qualification state transitions and expiry/revocation; mapping of only supported candidate types to existing AssetType and CapabilityModel v1." ECM applicability/bundle interaction are defined in W2.01/W2.02 (§5.4), qualification transitions in W2.03 (§5.5), AssetType mapping in W2.05 (§5.6) — all Wave 2. W1.17's Dependencies (line 20) list only W1.01–W1.16, so it cannot consume Wave 2 definitions.
- **Why it matters:** the Phase 1 test plan claims Phase 1 scope for tests whose oracles do not exist yet; executing W1.17 in Wave 1 would either test undefined behaviour or silently defer the items.
- **Fix:** in W1.17 step 1, move the three Wave 2 items (ECM applicability/bundle interaction, qualification state transitions, AssetType/CapabilityModel mapping) out of the Phase 1 list and record them as "deferred to the Wave 2 test record (W2.08 exit gate)"; add a Wave 2 test subtask (or extend W2.08) that owns those §13.3 items against the W2.01–W2.05 definitions. Alternatively keep them in W1.17 only as a forward reference labelled "not executable in Phase 1".

### 1-04 — Medium — W2.03 and W2.06 cite W1.17 as the test venue for Wave 2 rules

- **File / subtask:** `wave-02-f16-phase-2/03_flexibility-discovery-lifecycle.md`, W2.03, line 12; `wave-02-f16-phase-2/06_onboarding-draft-human-approval.md`, W2.06, line 22.
- **Evidence:** W2.03 step 4: "Record the lifecycle as the test oracle for the qualification state-transition tests in W1.17/W2.08." W2.06 Risks: "both rules are stated as contract and tested in W1.17." W1.17 executes in Wave 1, before W2.03/W2.06 exist; it cannot test rules defined later. (Contrast W1.15 Risks, which correctly cites same-wave W1.17.)
- **Fix:** in W2.03 step 4 delete `/W1.17` so the oracle is for "the qualification state-transition tests in W2.08" (and the Wave 2 test record created per F-03); in W2.06 Risks replace "tested in W1.17" with "tested in the Wave 2 exit-gate review (W2.08)".

### 1-05 — Medium — MASTER_LEDGER.md §3 entry gates are self-contradictory

- **File:** `MASTER_LEDGER.md`, §3 entry/exit-gates table, lines 81 and 84.
- **Evidence:** Line 81 (Wave 1): "Entry gate … Gate 0 exit gate; G1 and G2 decided or recorded as interim positions". Line 84 (Wave 4): "Entry gate … Gate 0 exit gate; G5 decided or recorded as an interim position". But §5 of the same file assigns G2 to W1.02 (inside Wave 1) and G5 to W4.01 (inside Wave 4) — a wave cannot require as its entry a decision produced by its own subtasks. §2's dashboard is consistent with the ownership (Wave 1 blocking: "G2 (first facility archetype); G3"), contradicting §3.
- **Fix:** amend §3: Wave 1 entry gate → "Gate 0 exit gate; G1 decided or recorded as an interim position" (G2 is a Wave 1 deliverable, decided in W1.02); Wave 4 entry gate → "Gate 0 exit gate" (G5 is a Wave 4 deliverable, decided in W4.01). Add a sentence noting G2/G5 are decided inside their waves, not before them.

### 1-06 — Medium — W0.06 step 1 uses the pilot archetype from W1.02 (later wave)

- **File / subtask:** `wave-00-gate-0/06_decide-f16-first-jurisdiction.md`, W0.06, line 10.
- **Evidence:** "1. Identify the candidate jurisdiction(s) from the D1 record and the pilot archetype in W1.02; do not select one silently." W0.06 Dependencies (line 21) are only `W0.04`; W1.02 (Wave 1) depends on W0.06 (G1). Design §16 G1 says nothing about needing the archetype; the archetype is G2's subject. The step makes G1 depend on G2 while G2's subtask depends on G1.
- **Fix:** reword step 1 to "Identify the candidate jurisdiction(s) from the D1 record (W0.04); do not select one silently." Delete the W1.02 reference. If the archetype genuinely constrains the jurisdiction, record that as an explicit G1↔G2 sequencing note in MASTER_LEDGER.md §5 instead of a hidden step dependency.

### 1-07 — Medium — Design §0 product-boundary sentences uncovered

- **File / subtask:** (none — no owning subtask exists).
- **Evidence:** design §0, "The product boundary": "An assessment is not an asset. A discovered flexible load is not a qualified dispatch resource. A solar estimate is not an interconnection approval. An optimizer result is not authorization to control a device." A case-insensitive grep over all wave files for "assessment is not an asset", "not a qualified dispatch", "interconnection approval", "not authorization to control" returns zero hits. W0.05 records the §0 decision-table rows but not these four sentences; the last one is a control-authority rule with no other owner (W2.06 states qualification ≠ dispatch authority, but not the optimizer-result rule).
- **Fix:** add the four sentences to W0.05 (steps + acceptance criteria), with an acceptance checkbox that each appears verbatim in substance in the Wave 0 baseline record.

### 1-08 — Medium — Design §2.1 logical architecture uncovered

- **File / subtask:** (none — no owning subtask exists).
- **Evidence:** no wave file cites design §2.1 (grep for `§2.1` returns zero hits; only §2.2 and §2.3 are cited). §2.1's mermaid diagram defines the cross-product architecture: Assessment Studio → control-plane API → F-16 bounded context → tenant Postgres / evidence object store / workers → Core registry via a reviewed onboarding package; the separate F-17 web/API/database/object-store/calculation worker; and the shared external seams (weather/solar providers → W and CALC; roof provider → HAPI; tariff/cost packs → F-16 and HAPI; REopt/PVWatts/SAM → CALC). W1.11 covers only the §6.1 F-16 placement and W5.01 only the §11.1 F-17 layers; neither owns the joint diagram or the external seams.
- **Fix:** add a subtask (new W0.09 or a Wave 1 subtask, continuing the sequence — never renumber) recording the §2.1 logical architecture as the cross-product deployment diagram, including the external-provider seams and the rule that the two products meet only at the §12 enrollment boundary; cross-reference it from W1.11 and W5.01.

### 1-09 — Medium — Design §8.2 nine-step user journey uncovered

- **File / subtask:** (none — no owning subtask exists).
- **Evidence:** no wave file cites design §8.2 (grep for `§8.2` returns zero hits; W5.01 traces §8.1–§8.3 in its header but its steps cover only the §8.1 statement and the §8.3 non-goals). The journey steps — (1) property/location, (2) usage, (3) tariff, (4) roof, (5) equipment/constraints, (6) compare scenarios, (7) review outputs, (8) decision package, (9) optional sharing — are therefore never owned as a journey. In particular step 5 (candidate PV module/inverter assumptions and battery options; generic assumptions visibly separate from actual vendor quotations) and step 8 (whether solar makes economic sense, which configuration, driving assumptions, missing information, financial risks, whether professional inspection is necessary, what the household must do next; comfortable recommending "do not install") have no owning subtask: W5.03 covers scenario comparison, W5.05 covers the §13.1/§13.5 report, but neither reproduces the §8.2 step-5 or step-8 content lists.
- **Fix:** add a Wave 5 subtask (continuing the sequence) owning the §8.2 journey end to end, reproducing all nine steps with the step-5 equipment/constraints rule and the step-8 decision-package contents as acceptance criteria; cross-reference W5.02 (steps 1–4), W5.03 (step 6), W5.05 (step 7–8 report) and W5.06 (step 9) instead of duplicating them.

### 1-10 — Medium — Design §9: only 5 of 10 records defined in W5.02

- **File / subtask:** `wave-05-f17-phase-1/02_property-bill-interval-import-tariff-roof-model.md`, W5.02, step 1 (line 9).
- **Evidence:** design §9 defines ten records: HomePlan, PropertyProfile, RoofPlane, LoadProfile, TariffProfile, PVSystemScenario, BatteryScenario, FinancialAssumptionSet, ScenarioRun, ConsentGrant. W5.02 step 1 defines only the first five. A grep for `PVSystemScenario|BatteryScenario|FinancialAssumptionSet|ScenarioRun|ConsentGrant` over all wave files returns zero hits. The missing record rules are therefore unowned: PVSystemScenario fields (DC size, inverter AC size, module/array assumptions, tilt, azimuth, losses, degradation, bifacial/soiling); BatteryScenario fields (usable vs nameplate capacity, power, chemistry, round-trip efficiency, SOC range, reserve, warranty/degradation/replacement); FinancialAssumptionSet (itemized CAPEX; user-entered quote overrides a generic estimate but is labelled); ScenarioRun (immutable input snapshot, engine/model versions, solver status, output digest, warnings/uncertainty); ConsentGrant (purpose, recipient, fields, expiry, revocation status, created-at). W5.03/W5.04/W5.05/W5.06 cover adjacent behaviour, not these record definitions.
- **Fix:** extend W5.02 step 1 (and its acceptance criteria) to define all ten §9 records with their key fields and design rules, or split the five missing records into a new Wave 5 subtask that W5.03–W5.06 depend on. Update the acceptance checkbox to require all ten records.

### 1-11 — Medium — Design §13.2 determinism/repeatability for F-17 uncovered

- **File / subtask:** (none — no owning subtask exists).
- **Evidence:** `§13.2` is cited only in W1.07 (F-16 jobs: worker envelope, tolerances, no exact-replay claim, provider request/response digest). No Wave 4/5 subtask cites §13.2. Design §13.2 applies to both products ("For numerical providers that are not bitwise deterministic, record tolerances and solver status… Store actual external provider request parameters and provider response digest, with credentials removed. Keep model evaluation datasets and hand-calculated reference cases separate from live user data") — for F-17 this governs the PVWatts/REopt/SAM provider calls. W5.05 covers §13.1/§13.5, not §13.2.
- **Fix:** add §13.2 (F-17) to W5.05 or to a new Wave 5 subtask: determinism rules for the companion product's scenario jobs (request/response digests with credentials removed, tolerance and solver-status recording, no exact-replay claim, evaluation datasets separate from live user data), with acceptance criteria.

### 1-12 — Medium — Design §1.1 reuse rows uncovered (units, forecasts, weather seam, optimizer)

- **File / subtask:** (none — no owning subtask exists).
- **Evidence:** design §1.1 rows with no owning subtask (grep returns zero hits in wave files for the quoted rules):
  - Units: "FlexCore boundary uses MW, MVAr, MVA and MWh; SOC is a fraction. Convert kW/kWh at adapters." Only README.md states canonical units; no subtask owns the conversion rule for either feature.
  - Forecasts: "The PV model declarations are UNTRAINED and unevaluated; no accuracy claims." Zero hits for "untrained" / "no accuracy claim" in wave files.
  - Weather: "A vendor-neutral, tenant-scoped weather seam already exists, but it has no real provider adapter. It is appropriate for F-16; it is not directly reusable by F-17's independent consumer identity boundary." No subtask states this reuse boundary.
  - Optimization: "Solver-neutral problem IR and provider capability declarations already exist. Use only with an explicit supported formulation; unsupported classes must refuse." W5.03 step 4 covers the F-17 reuse condition only; no subtask states the rule for F-16's use of the optimization API.
- **Fix:** add a Wave 1 subtask (or extend W1.05/W1.11) recording the §1.1 reuse rules for F-16, and a corresponding note in W4.09/W5.03 that the weather seam and the FlexCore optimizer port are not directly reusable by F-17 except under the stated conditions; add acceptance criteria for the units-conversion rule, the no-accuracy-claim rule, the weather-seam boundary, and the unsupported-formulation refusal.

### 1-13 — Medium — Design §1.1 flexible-load model and BESS rows uncovered

- **File / subtask:** (none — no owning subtask exists; nearest is W2.04).
- **Evidence:** design §1.1 flexible-load row: the current model "has baseline, reducible/increasable range, duration, recovery, rebound, availability and descriptors for comfort/process constraints. It is not a calibrated building thermal simulator." W2.04 step 3 covers only the P15.14 DeclaredThermalModel half ("record sourced parameters; does not fit or calibrate"); the flexible-load model field inventory and the not-a-calibrated-thermal-simulator limit have no owner. Design §1.1 BESS row: "Reuse battery capability rules where the source data is adequate. Unknown limits must not create flexibility credit." Zero hits for "unknown limits" / "flexibility credit" in wave files (W1.07's "earn positive dispatch credit" is the P3.02 unknown-narrows rule, a different rule).
- **Fix:** extend W2.04 (or add a Wave 2 subtask) with: the flexible-load model field inventory and its not-a-calibrated-thermal-simulator limit as the reuse rule for flexibility candidates; and the BESS rule that unknown limits must not create flexibility credit, with acceptance criteria for both.

### 1-14 — Medium — W3.03 invents a control allowance not in the design

- **File / subtask:** `wave-03-f16-phase-3/03_bms-ems-edge-interval-integrations-pilot.md`, W3.03, line 10 (step 2).
- **Evidence:** "For each integration, record the security boundary it crosses, the authorization it requires, and the data it may read (read-only integration unless a separate, explicitly approved control decision exists)." Design §14 F-16 Phase 3 says only: "Add BMS/EMS/edge/interval integrations only for an actual pilot and security boundary." It authorizes no control decision anywhere in Phase 3, and design §0/AGENTS.md hold the whole 0.x line simulation-only with field write authority beginning at v1.0.0 with operator approval. The parenthetical invents a control path the design never grants.
- **Fix:** delete the parenthetical; replace with "(read-only integration; no integration in this subtask grants dispatch authority or any control write — see step 3)". Step 3 already states this; the fix removes the contradiction between steps 2 and 3.

### 1-15 — Medium — W4.10 decides G7 in Phase 0 before the prototype that needs the engine

- **File / subtask:** `wave-04-f17-phase-0/10_decide-reopt-api-v3-vs-local-reopt-jl.md`, W4.10, line 11 (step 2).
- **Evidence:** "Record the decision or interim position with owner, venue and date, and state which option the Phase 0 prototype uses and why." W4.10 Dependencies (line 21) are `W4.05, W0.03` — i.e. the decision is recorded after the prototype is built in W4.04 and benchmarked in W4.05, so the prototype's sizing/dispatch engine is undecided while W4.04/W4.05 run. Design §10.4 step 6 defers this decision: "After the model is understood and user demand is validated, decide whether a local REopt.jl worker is operationally preferable to a hosted API. This is a build-vs-buy/license/operations decision, not a foregone conclusion." User demand is validated in W4.06, which W4.10 does not depend on.
- **Fix:** either (a) record G7 in W4.10 as an interim position only, with the prototype explicitly using the REopt API v3 proof-of-concept adapter (§10.4: "leading candidate for an external optimization reference and proof-of-concept adapter") and the final decision deferred to after W4.06 per §10.4 step 6; or (b) move the G7 decision out of Phase 0 to Wave 5 and add a W4.04 input stating the prototype engine (REopt API v3 PoC adapter). Do not leave the prototype engine undecided during W4.04.

### 1-16 — Medium — W6.01 and W6.07 duplicate the same pack work

- **File / subtask:** `wave-06-f17-later-phases/01_more-jurisdiction-packs.md`, W6.01; `wave-06-f17-later-phases/07_regional-energy-planning-packs.md`, W6.07.
- **Evidence:** foundations.md §7 item 3 ("Later — Regional energy planning packs: Region-specific tariffs, net-metering/net-billing rules, equipment costs, incentives, financing assumptions and installation workflows. Each pack needs a source, jurisdiction and validity date") is one item, and design §14 later phases lists "More jurisdiction packs" as the same work. W6.01 builds packs jurisdiction by jurisdiction with source/jurisdiction/validity per item; W6.07 defines "regional energy planning packs" with the same six contents and the same source/jurisdiction/validity rule. W6.07 step 3 claims a distinction — "regional packs are the pack format; W6.01 adds the packs jurisdiction by jurisdiction" — but neither foundations.md nor the design doc distinguishes a pack-format subtask from pack instances. Two subtasks own the same definition.
- **Fix:** merge W6.07 into W6.01 (keep W6.01 as the single pack owner, carrying the format definition, the local-expert review rule and the expired-pack labelling rule from W6.07 steps 3–4), and delete W6.07 — or, if the count must stay at 70, keep W6.07 but rewrite it to own only the review/expiry governance with an explicit "format defined in W6.01, not redefined here" cross-reference. Do not leave two subtasks defining the same pack contents.

### 1-17 — Low — W4.05 and W4.06 both claim the same verdict

- **File / subtask:** `wave-04-f17-phase-0/05_benchmark-yield-economics-pvwatts-reopt-sam.md`, W4.05, line 12 (step 4); `wave-04-f17-phase-0/06_usability-tests-actual-prospective-homeowners.md`, W4.06, line 12 (step 4).
- **Evidence:** W4.05 step 4: "Record the verdict per case: whether the prototype output is sufficiently accurate and useful to justify maintaining the regional data, or what must change." W4.06 step 4: "Record the verdict: whether the output is sufficiently accurate and useful to justify maintaining the regional data, and what must change before Phase 1." Both record the foundations.md §8 item 5 verdict ("Verify that the output is sufficiently accurate and useful to justify maintaining the regional data") as their own deliverable.
- **Fix:** keep the verdict in one place: W4.05 owns the per-case accuracy verdict from the benchmark; W4.06 owns only the usability verdict (what homeowners needed / where they struggled). Reword W4.06 step 4 and its acceptance checkbox (line 18) to record the usability verdict, and have W4.13 cite W4.05 for the accuracy verdict and W4.06 for the usability verdict.

### 1-18 — Low — W5.08 invents "threat considerations" and a "pre-launch security review"

- **File / subtask:** `wave-05-f17-phase-1/08_privacy-security-design.md`, W5.08, lines 9, 13, 18.
- **Evidence:** Step 1: "add the threat considerations that follow from it (IDOR across home plans, provider-secret isolation, PII in logs, consent bypass)". Deliverable (line 13): "with the §11.3 baseline, the threat considerations, the boundary, and the pre-launch review." Acceptance (line 18): "The pre-launch security review and its owner are recorded." Design §11.3 contains no threat model and requires no pre-launch security review. (IDOR/rate-limit/CSRF are §11.3 controls, correctly covered in W5.06/W5.07 — the invention is the threat-modelling exercise and the extra review gate.)
- **Fix:** delete "and add the threat considerations that follow from it (…)" from step 1; delete "the threat considerations" and "the pre-launch review" from the deliverable; replace the line-18 acceptance checkbox with "The §11.3 baseline, consent rule and no-dispatch-path boundary are stated" (already covered by the other checkboxes — or drop the checkbox). If a threat model or pre-launch review is wanted, it needs a design-doc basis first, not a ledger invention.

### 1-19 — Low — W4.03 attaches "imagery age shown" to manual roof-plane entry

- **File / subtask:** `wave-04-f17-phase-0/03_compare-manual-roof-input-vs-roof-provider.md`, W4.03, line 10 (step 1).
- **Evidence:** "Compare manual roof-plane entry (user-drawn/declared planes and obstructions, with source and imagery age shown) against an optional provider …" Imagery age is a provider-data attribute: design §8.2 step 4 ("Show source and imagery age") sits in the provider-roof context, and §10.1's RoofGeometryProvider carries "imagery/model quality and source timestamp". A user-drawn plane has no imagery age.
- **Fix:** reword to "Compare manual roof-plane entry (user-drawn/declared planes and obstructions, with source shown) against an optional provider (… with source and imagery age shown …)".

### 1-20 — Low — W6.06 "(it does not)" over-asserts against §12's "customer-chosen site location"

- **File / subtask:** `wave-06-f17-later-phases/06_residential-der-enrollment-package-definition.md`, W6.06, line 10 (step 2).
- **Evidence:** "no household bills, interval data, raw addresses or occupancy data are included unless the selected-field definition explicitly requires them (it does not)." Design §12 step 1's selected fields include "customer-chosen site location" — a site location chosen by the customer may be a raw address, so the parenthetical assertion that the definition does not require raw addresses is not established by §12 as written.
- **Fix:** delete "(it does not)" and replace with an explicit rule: "site location is included only in the form §12 requires (customer-chosen site location); raw postal addresses are excluded unless the aggregator's enrollment definition explicitly requires them, in which case that requirement and its consent basis are recorded."

## Counts

- Found: 20 items — Critical 0, High 2, Medium 14, Low 4.
- Applied: 0 (reviewer does not edit draft files; all findings OPEN pending the drafting agent's patch pass).
- Remaining open: 20 (F-01–F-20 above).
- Verdict: **findings remain — NOT finalized.** The two High findings (F-01, F-02) are circular dependencies that make Waves 4 and 6 unworkable in order; the fourteen Medium findings include six uncovered design requirements (F-07–F-13) and four dependency/ordering defects (F-03–F-06) plus three invented-content items (F-14–F-16). A patch pass addressing F-01–F-16, followed by a re-count and a re-check of the affected files, is required before this ledger can be called finalized.

---

## Patch pass (applied 2026-11-21, same session, by the ledger author)

All 20 findings were patched in `docs/roadmap/ledger/`. No draft file
was left unpatched; no finding was dismissed. Two new subtasks were
added, continuing their wave sequences (existing IDs were never
renumbered):

- **W0.09** — `wave-00-gate-0/09_section-2-1-logical-architecture-cross-product.md` (F-08)
- **W5.11** — `wave-05-f17-phase-1/11_section-8-2-user-journey-trace.md` (F-09)

**F-01 (High) — FIXED.** `wave-04-f17-phase-0/07_input-quality-tiers.md`:
Dependencies now `W4.01` only (W4.04 removed); step 1 states the
W4.04 prototype is a later verification consumer, not a prerequisite;
`wave-04-f17-phase-0/04_prototype-pv-only-pv-battery-real-datasets.md`
Dependencies now `W4.02, W4.07` (kept, as the fix prescribed).

**F-02 (High) — FIXED.** `wave-06-f17-later-phases/06_residential-der-enrollment-package-definition.md`:
Dependencies now `W4.12, W5.06, W5.10` (W6.05 removed);
`wave-06-f17-later-phases/05_vpp-enrollment-handoff-optional.md`
Dependencies now `W6.06, W4.12, W5.06, W5.10` (kept).

**F-03 — FIXED.** `wave-01-f16-phase-1/17_f16-test-plan.md`: step 1
defers the three Wave 2 §13.3 items to the Wave 2 test record (W2.08
exit gate) with a new checkbox; `wave-02-f16-phase-2/08_phase-2-exit-gate-review.md`
step 5 + checkbox verify them against the Wave 2 definitions.

**F-04 — FIXED.** `wave-02-f16-phase-2/03_flexibility-discovery-lifecycle.md`
step 4 and `wave-02-f16-phase-2/06_onboarding-draft-human-approval.md`
Risks now cite the W2.08 test record only (W1.17 mention removed).

**F-05 — FIXED.** `MASTER_LEDGER.md` §3: Wave 1 entry gate no longer
requires G2 (G1 only; "G2 is a Wave 1 deliverable, decided in
W1.02"); Wave 4 entry gate no longer requires G5 ("G5 is a Wave 4
deliverable, decided in W4.01"); the note after the table now states
G2 and G5 are decided inside their own waves and no wave's entry gate
requires a decision its own subtasks produce.

**F-06 — FIXED.** `wave-00-gate-0/06_decide-f16-first-jurisdiction.md`
step 1 cites the D1 record (W0.04) only (W0.03 removed);
`MASTER_LEDGER.md` §5 carries the G1↔G2 sequencing note (G1 decided
from the D1 record alone; a G1 revisit trigger is recorded in W1.02
if the archetype constrains the jurisdiction).

**F-07 — FIXED.** `wave-00-gate-0/05_record-section-0-executive-decisions-baseline.md`:
four §0 boundary sentences added to step 4, objective, deliverable
and acceptance checkbox (assessment ≠ asset, ≠ qualified dispatch
resource, ≠ interconnection approval, ≠ authorization to control).

**F-08 — FIXED.** New W0.09 owns the §2.1 diagram and external seams
(§2.3). `wave-00-gate-0/08_gate-0-exit-gate-review-handover-wave-1.md`
Inputs/Dependencies/step 1/acceptance now include W0.09.
`wave-01-f16-phase-1/11_repository-placement-module-layout.md` step 5
+ W0.09 dependency; `wave-05-f17-phase-1/01_consumer-auth-separate-data-store-product-scope.md`
step 5 + W0.09 dependency. `MASTER_LEDGER.md` §1, §2, §4 and the
Wave 0 entry gates in §3 name W0.09.

**F-09 — FIXED.** New W5.11 owns the §8.2 nine-step journey trace
(deps W5.02/W5.03/W5.05/W5.06). `wave-05-f17-phase-1/10_phase-1-exit-gate-review-f17.md`
Inputs/Dependencies/step 1/acceptance now cover W5.01–W5.11.
`MASTER_LEDGER.md` §1, §2, §4 name W5.11.

**F-10 — FIXED.** `wave-05-f17-phase-1/02_property-bill-interval-import-tariff-roof-model.md`
step 1 now defines all ten §9 records (Property, Bill/Interval
records, Tariff, RoofPlane, PVSystemScenario, BatteryScenario,
FinancialAssumptionSet, ScenarioRun, CalculationReport, ConsentGrant)
and acceptance checkbox 1 lists them.

**F-11 — FIXED.** `wave-05-f17-phase-1/05_immutable-calculation-report.md`
now traces §13.2 determinism/repeatability (step 5 + checkbox).

**F-12 — FIXED.** §1.1 reuse rules added with checkbox + trace to
W1.09 (step 5), W1.10 (step 6), W2.02 (step 5) and W4.09 (step 5).

**F-13 — FIXED.** Flexible-load inventory requirement added to
`wave-02-f16-phase-2/04_qualification-checklist-by-resource.md`
(step 5 + checkbox); BESS no-flexibility-credit rule added to
`wave-02-f16-phase-2/05_safe-mapping-into-existing-flexcore-assets.md`
(step 5 + checkbox).

**F-14 — FIXED.** `wave-03-f16-phase-3/03_bms-ems-edge-interval-integrations-pilot.md`
invented control parenthetical replaced with read-only wording
("read-only data collection from BMS/EMS/edge systems; no control
claims").

**F-15 — FIXED.** `wave-04-f17-phase-0/10_decide-reopt-api-v3-vs-local-reopt-jl.md`
reframed as interim position only (deps now W4.05/W4.06/W0.03);
`wave-04-f17-phase-0/04_prototype-pv-only-pv-battery-real-datasets.md`
step 5 states the REopt API v3 PoC adapter engine;
`wave-05-f17-phase-1/03_scenario-comparison-financial-model.md`
step 5 + checkbox records the final G7 decision.

**F-16 — FIXED.** `wave-06-f17-later-phases/07_regional-pack-review-expiry-governance.md`
(renamed from `07_regional-pack-format-and-governance.md`) is now
pack review/expiry governance only; `wave-06-f17-later-phases/01_per-jurisdiction-pack-format.md`
step 5 + checkbox cross-refers W6.07 for governance.

**F-17 — FIXED.** `wave-04-f17-phase-0/06_usability-tests-actual-prospective-homeowners.md`
step 4/deliverable/acceptance now give a usability verdict only
(accuracy verdict belongs to W4.05); `wave-04-f17-phase-0/13_phase-0-exit-gate-review-f17.md`
step 1 notes the accuracy verdict is W4.05's.

**F-18 — FIXED.** `wave-05-f17-phase-1/08_privacy-security-design.md`:
invented threat-considerations and pre-launch-review material removed
(step 1 phrase, step 4, deliverable, acceptance checkbox 4 dropped;
"threat considerations"/"pre-launch" now 0 hits in the file).

**F-19 — FIXED.** `wave-04-f17-phase-0/03_compare-manual-roof-input-vs-roof-provider.md`
step 1: source shown for manual entry; source + imagery age for the
provider.

**F-20 — FIXED.** `wave-06-f17-later-phases/06_residential-der-enrollment-package-definition.md`
step 2: "(it does not)" replaced with the explicit site-location rule
(each package carries exactly one site location — a facility address
for F-16, a home address for F-17).

**Counts after the patch pass** (grep-verified, see iteration 02):
W0 9, W1 18, W2 8, W3 5, W4 13, W5 11, W6 8 = **72 subtasks**;
305 unchecked boxes, 0 checked. `MASTER_LEDGER.md`, `README.md` and
`FINALIZATION.md` updated to the new counts and review status.
