# Decision record — P5.08 Simulation Lab workspace, full §20.1 item 9 (§20.1 item 9, §19.1–§19.4, §24 step 10)

**Status:** implemented, awaiting human build/test. **Nothing was built,
installed, run, or executed** — no cargo command, no npm install, no vitest
run (loop bans; AGENTS.md working agreement). Every test below is written
and pre-registered UNRUN; the human build/test pass is the gate.

## 1. What was built

| Artefact | File | Change |
|---|---|---|
| Lab data module (EXTENDED) | `console/src/workspaces/simulationLabData.ts` | ADDITIVE to the P5.22 fault-visibility module (nothing above the P5.08 marker changed): the `SCENARIO_LIBRARY_MIRROR` (lib-01…08 with record version, purpose, scale class, DECLARED runtime class, backend + worker-leg note, pinned scenario id:version, master seed, grid fixture, declared objective), the replay statements (`REPLAY_RECORDED_INPUTS_STATEMENT`, `REPLAY_SERVING_GAP`), `replayRunCommand` (double-run / baseline forms), the closed verdict vocabularies (`REPLAY_VERDICTS`, `REPLAY_LEG_VERDICTS`, `REPLAY_COMPARISON_KINDS`) + `replayVerdictMarker` (distinct text, unknown rendered verbatim), the `ReplayComparisonRecordDto` / `ScenarioRunTimingsDto` / `ConsumptionDecisionSummaryDto` wire mirrors (drift-noted), `runExtras` (render-what-exists for the additive record fields), and the benchmark template constants (`BENCHMARK_TEMPLATE_STATUS`, `BENCHMARK_BINDING_RULES_NOTE`, Table A/B cases, the five timing legs, the verbatim `not_measured` reasons, the count-columns note). |
| Lab workspace (EXTENDED) | `console/src/workspaces/SimulationLab.tsx` | ADDITIVE sections after the unchanged P5.22 run/fault layer: `ScenarioLibrarySection` (the recorded mirror table + §19.3 honesty notes), `ReplaySection` (the recorded-inputs rule, the per-record replay commands, the serving gap), `ReplayComparisonView` (exported; input-hash match/mismatch as a tamper finding, per-leg table with named diffs, verdict + verdict statement + tolerance rule, unknown vocabulary rendered verbatim), `BenchmarkSection` (Tables A/B with every measured cell EMPTY — "an honest gap, never a zero" — plus the verbatim not_measured list and binding rules), `ResultComparisonSection` + `ComparisonTable` (the §12.6 records through the IMPORTED P3.23 rule), and `RunRecordConsumption` (the record's verbatim statements, the P5.23 timings block, the P4.37 consumption decisions — each rendered only when the record carries it). The loader gains the third read (`GET /api/v1/optimization-runs`, the existing schedules client) with its own kept error. Export name unchanged. |
| Lab tests (EXTENDED) | `console/src/workspaces/SimulationLab.test.tsx` | ADDITIVE: the P5.22 assertions unchanged; 13 new tests (UNRUN) covering the library mirror, the replay commands/markers/`runExtras`, the four new sections' render honesty, the comparison verdicts (comparable/incomparable/empty/error), `ReplayComparisonView` (match, mismatch-as-tamper-finding, unknown-vocabulary verbatim), the run-record consumption (present/absent), and the P5.27 mode-gate notice interaction. The two fixture helpers gain the new loader fields. |
| Mode gate notice (NEW, shared) | `console/src/components/modeGateNotice.tsx` | The additive P5.27 display beside the P5.13 indicator (see `27_mode_enforcement.md`). |

No other file changed: `console/src/api/scenarioRuns.ts` (the P5.22 run-record
client) was NOT modified — the additive record fields are read through the
typed `runExtras` cast, because the wire mirror already tolerates additive
fields. No serving API was invented anywhere.

## 2. Coverage of the four §20.1 item 9 elements

| Element (task scope) | Where implemented | Honest status rendered |
|---|---|---|
| Scenarios (the scenarios/library INDEX, lib-01…08, with pins / runtime class) | `ScenarioLibrarySection` over `SCENARIO_LIBRARY_MIRROR` | The mirror is labelled as a mirror of `scenarios/library/INDEX.md` + the lib-*.json records (records authoritative; mirror can drift — the capability-mirror discipline). Runtime classes render as DECLARED, never measured. Run history appears only under "Scenario runs" from real records. |
| Replay (replay-run records: input-hash match, per-leg diffs, ReplayComparisonRecord display) | `ReplaySection` + `ReplayComparisonView` | The recorded-inputs rule renders verbatim (P5.18: any input not reproducible from the record is an ERROR; no seed override). The replay commands render per record (double-run / `--baseline`). The comparison display renders the verdict, the input-hash check (a mismatch IS the tamper finding), every named diff, and the pre-registered tolerance rule. THE GAP: no control-plane route serves ReplayComparisonRecords (`REPLAY_SERVING_GAP`, rendered) — the component is exported for the P5.30 serving surface; no verdict is fabricated. |
| Benchmark runs (the 23_benchmark_report.md template status) | `BenchmarkSection` | The template status renders verbatim: every measured cell EMPTY ("an honest gap, never a zero"), the lib-04/lib-05 worker-leg note, the five Table-C not_measured reasons verbatim, the count-columns rule, and the binding rules (baseline never targets; no arbitrary universal targets §19.4; raw numbers only; conditions travel; every figure SIMULATED-labelled; the grid leg is NOT an AC-OPF). No figure exists; none is claimed; none invented. |
| Result comparison (schedule alternatives / §12.6 outcomes + Tier comparisons) | `ResultComparisonSection` + `ComparisonTable` | The P3.23 comparability rule is REUSED BY IMPORT (`assessComparison` from Schedules.tsx — one rule, no copy; the reuse note renders). Comparable requires the shared input snapshot; cross-input comparisons render INCOMPARABLE. Per-candidate metrics (objective value with units, solver + version, status, input snapshot) render from the real read; empty and error states are honest. |

## 3. The run-record consumption (additive fields)

`ScenarioRunRecord` (apps/sim-runner/src/scenario_run.rs) carries additive
fields the P5.22 summary mirror did not name: `timings`
(flexcore-scenario-run-timings/v1 — per-leg wall clock, MEASUREMENT clock,
never the scenario-clock STATE), `consumption_decisions` (the frozen
D-P4.07-1 routing summaries), and `statements`. The workspace renders each
ONLY when the record carries it (`runExtras`), never a placeholder: the
timings table renders the record's statement verbatim and per-leg
durations with their method; the consumption decisions render outcome
code, P4.37 action, route, reason, violated constraints, problem inputs,
check id, model id:version, the roll-trigger note, and the verbatim
statements; the record's own statements render verbatim (the console never
paraphrases the simulation-only boundary).

## 4. Decisions (all DEFAULT unless noted)

| ID | Decision | Label |
|---|---|---|
| D-P5.08-1 | The library index is rendered as a labelled REPO-ARTEFACT mirror, not an API read: no route serves ScenarioRecords in this phase and none was invented; the drift note names the records authoritative (the same mirror discipline as capabilityMatrix.ts). | DEFAULT |
| D-P5.08-2 | The replay comparison display is a component awaiting a serving surface: `ReplayComparisonView` is exported and tested against fixtures, the workspace renders the gap instead of a fabricated verdict (the P5.22 display-contract discipline, D-P5.22-3, extended to comparisons). | DEFAULT |
| D-P5.08-3 | The benchmark section mirrors the TEMPLATE STATUS (empty cells + not_measured reasons + binding rules), not measured figures: P5.23's measured run is the human's gate; the console renders no number that no record produced. | DEFAULT |
| D-P5.08-4 | The comparability rule is imported from Schedules.tsx (one rule, no copy, no drift surface) rather than duplicated into a shared component; the reuse note renders so a reviewer can trace the rule to its single home. | DEFAULT |
| D-P5.08-5 | The additive run-record fields are consumed through a local typed cast (`runExtras`) instead of editing the P5.22 client module (file-ownership boundary respected; the wire mirror's additive-tolerance note is the recorded basis). | DEFAULT |
| D-P5.08-6 | The HELICS composed-vs-executed distinction is carried through the record's own statements path: a federation record rendered through the statements/consumption surfaces shows `execution_mode` only as the record states it; the console never upgrades a `composed` record to executed (P5.16 step 4 wording travels with the record). | DEFAULT |

## 5. Honest limitations

1. **Everything is UNRUN.** No vitest run, no build, no type-check has
   executed. The 13 new tests (and the 8 pre-existing ones) are
   pre-registered, not passing.
2. **No run-record or comparison serving API exists** (the P5.22 gap,
   kept and extended to comparisons). In a real deployment the runs read
   404s and the workspace renders `RUN_RECORD_API_GAP`; the §12.6
   optimization-run read is the only one of the three reads that hits an
   existing control-plane route (routes_phase3.rs).
3. **The library mirror can drift** from `scenarios/library/` until
   hand-updated; the mirror note says so on the workspace.
4. **Benchmark cells stay EMPTY** until the human's recorded run produces
   records; the console renders no placeholder figure.
5. **The replay commands reference repo-relative record paths** as the CLI
   convention records them; a deployed console cannot execute them (no
   trigger API — the same documented-affordance-not-fake-button rule as
   the P5.22 clean re-run).
6. **Pre-existing inconsistency found, not owned here:** in
   `console/src/workspaces/Dispatch.test.tsx` (lines ~262/267) the
   approval-flow render tests pass `claims={null}` and expect the
   `confirm-high-impact-checkbox`, but the P5.26 capability gate (landed
   in wave 5b) renders the capability-denial block when no session exists.
   The tests are UNRUN so the contradiction has never executed; the
   human's build/test pass will surface it. Not fixed here — the file is
   outside this subtask's ownership; recorded so it is not mistaken for a
   P5.08/P5.27 regression.
