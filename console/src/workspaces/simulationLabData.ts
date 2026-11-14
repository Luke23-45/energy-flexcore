// P5.22 — the Simulation Lab data module: the honest statements, the
// fault→alarm mapping (over the CLOSED P5.31 vocabulary), and the
// derivation helpers the workspace renders. Pure functions + constants —
// no fetch, no state.
//
// Honesty rules encoded here:
// - every run record is SIMULATED and labelled with its scenario id +
//   version (§20.2; the label is part of the rendering, never an optional
//   prop);
// - a fault-injected run is ALWAYS tagged FAULT-INJECTED (an operator can
//   tell an injected run from a clean one at a glance — P5.22 step 1);
// - the fault→alarm mapping uses ONLY the closed ALARM-MODEL-0.1.0
//   vocabulary (crates/flexcore-contracts/src/alarm.rs): a response with
//   no matching alarm code maps to NO alarm and says so — no alarm is
//   invented (P5.31);
// - the "clear injections and re-run clean" affordance states the exact
//   command AND the gap (the console trigger API is not served in this
//   phase) — a documented affordance, never a fake button.
import type { FaultJournalEntryDto, ScenarioRunRecordSummaryDto } from '../api/scenarioRuns';

/** The simulated-results statement (rendered on the workspace, verbatim). */
export const SIMULATED_RESULTS_STATEMENT =
  'Every result on this workspace is SIMULATED (the entire 0.x line, §21.4): ' +
  'each run carries the scenario id and version it came from, and no value ' +
  'here is a field measurement. Acknowledgements are application-layer ' +
  'acceptance by the simulator, never physical delivery.';

/**
 * The run-record serving gap (rendered whenever the run read fails,
 * verbatim): the run-record read API does not exist yet in this phase.
 */
export const RUN_RECORD_API_GAP =
  'No control-plane route serves scenario-run records yet in this phase: ' +
  'the ScenarioRunRecord is produced by the sim-runner CLI as a JSON ' +
  'artefact (flexcore-sim-runner scenario-run), and the run-history ' +
  'serving contract is a pending surface (P5.08/P5.30). The console shows ' +
  'this gap rather than fabricating runs.';

/** The clean re-run affordance (P5.22 step 3): the exact command and the gap. */
export function cleanReRunCommand(run: ScenarioRunRecordSummaryDto): string {
  return (
    `flexcore-sim-runner scenario-run --scenario <the recorded scenario path> ` +
    `--seed ${run.master_seed} --code-version <your code version>`
  );
}

export const CLEAN_RERUN_STATEMENT =
  '"Clear injections and re-run clean" replays the SAME scenario from its ' +
  'RECORDED inputs (P5.18) with the fault plan disabled, for comparison. ' +
  'In this phase the action is available at the CLI with the command above ' +
  '— the console-side trigger API is not served yet (the run-record ' +
  'surface is pending, P5.08/P5.30) — so this workspace documents the ' +
  'action instead of rendering a button that could not do anything. The ' +
  'clean re-run MUST use the recorded inputs; any deviation is flagged, ' +
  'never silently substituted (P5.19).';

/** The fault-journal statement source (rendered verbatim from the record;
 * this constant is the fallback when a record predates the journal). */
export const NO_FAULT_JOURNAL_STATEMENT =
  'This run carries no fault journal: its scenario configured no fault ' +
  '(the scenario_faults v3 slot absent and the P2.15 fault plan disabled), ' +
  'so nothing was injected — this is the record\'s own absence, not a ' +
  'hidden failure.';

/** The alarm-code mapping (the closed P5.31 vocabulary only). */
export const FAULT_ALARM_NOTE =
  'ALARM-MODEL-0.1.0 (P5.31) has a closed 7-code vocabulary derived from ' +
  'platform facts. A fault response maps to an alarm code ONLY where the ' +
  'vocabulary has one for that outcome; every other response maps to NO ' +
  'alarm and the workspace says so — no alarm is invented.';

/**
 * Maps a fault journal entry's platform response onto the closed P5.31
 * alarm-code vocabulary (or honestly to no alarm). `failed` is the
 * simulated-lifecycle FAILED command state → `DISPATCH_FAILURE`;
 * `fallback` → `DISPATCH_FALLBACK`. Every other response has no alarm code
 * in the closed vocabulary.
 */
export function faultAlarmMapping(entry: FaultJournalEntryDto): {
  alarmCode: string | null;
  note: string;
} {
  if (entry.platform_response === 'failed') {
    return {
      alarmCode: 'DISPATCH_FAILURE',
      note: 'the command outcome the response records is the FAILED state — the P5.31 derivation rule surfaces DISPATCH_FAILURE from the same fact',
    };
  }
  if (entry.platform_response === 'fallback') {
    return {
      alarmCode: 'DISPATCH_FALLBACK',
      note: 'the fallback activation the response records is the same fact the P5.31 derivation rule surfaces as DISPATCH_FALLBACK',
    };
  }
  return {
    alarmCode: null,
    note:
      `no alarm code exists in the closed ALARM-MODEL-0.1.0 vocabulary for a ` +
      `'${entry.platform_response}' response — no alarm is invented (the ` +
      `console does not extend the vocabulary)`,
  };
}

/** Whether a run is fault-injected (a journal with at least one entry). */
export function isFaultInjected(run: ScenarioRunRecordSummaryDto): boolean {
  return (run.fault_journal?.entries.length ?? 0) > 0;
}

/**
 * The failure rows of a run: the commands whose final state or issue
 * outcome names a failure/rejection, and the grid checks that did not
 * pass. A failed run renders these — never a blank screen (P5.22).
 */
export function failureRows(run: ScenarioRunRecordSummaryDto): {
  label: string;
  detail: string;
}[] {
  const rows: { label: string; detail: string }[] = [];
  const failingStates = ['FAILED', 'REJECTED', 'EXPIRED', 'CANCELLED', 'SUPERSEDED', 'FALLBACK'];
  for (const command of run.commands) {
    const failing =
      failingStates.includes(command.final_state) ||
      command.issue_outcome.startsWith('rejected') ||
      command.issue_outcome.startsWith('failed') ||
      command.issue_outcome === 'nack' ||
      command.issue_outcome.startsWith('denied');
    if (failing) {
      rows.push({
        label: `command ${command.command_id} (asset ${command.asset_key})`,
        detail: `final state ${command.final_state}, issue outcome '${command.issue_outcome}'${
          command.tracking_verdict ? `, tracking ${command.tracking_verdict}` : ''
        }`,
      });
    }
  }
  for (const check of run.grid_checks) {
    if (check.verdict !== 'passed') {
      rows.push({
        label: `grid check via ${check.backend} (${check.provider_id_version})`,
        detail: `verdict ${check.verdict}${check.reason ? ` — ${check.reason}` : ''}`,
      });
    }
  }
  return rows;
}

/** Whether the run has any failure row (the workspace renders the failure
 * journal block when true). */
export function hasFailures(run: ScenarioRunRecordSummaryDto): boolean {
  return failureRows(run).length > 0;
}

// ===========================================================================
// P5.08 — the FULL Simulation Lab workspace (§20.1 item 9): scenarios,
// replay, benchmark runs, and result comparison. Everything below is
// additive to the P5.22 fault-visibility layer above (extended, never
// rewritten), and carries the same honesty rules:
// - the library index, the replay comparison shapes, and the benchmark
//   template status are RECORDED REPO ARTEFACTS mirrored here with a drift
//   note — they are never presented as API reads and never fabricated;
// - the run-record READ API and the comparison-serving API do not exist
//   yet in this phase (P5.22's gap, kept) — the console renders the gaps
//   and never invents a run, a comparison, or a measured figure;
// - every result is SIMULATED and labelled with its scenario id + version.
// ===========================================================================

// --- The scenario library (§19.3 cases; P5.20) -------------------------------

/** One mirrored library record (the fields the workspace displays). */
export interface ScenarioLibraryEntry {
  recordId: string;
  recordVersion: string;
  purpose: string;
  scaleClass: string;
  /** The DECLARED runtime class — never a measured figure (P5.23 owns those). */
  runtimeClass: string;
  backend: string;
  backendNote?: string;
  /** The pinned telemetry scenario (id + version), verbatim from the record. */
  scenarioPin: string;
  masterSeed: number;
  gridFixture: string;
  objective: string;
}

/**
 * The scenario-library mirror (lib-01…08). Source: `scenarios/library/
 * INDEX.md` + the `lib-*.json` ScenarioRecord v3 files (record_id,
 * record_version, scale_class, runtime_class, backend.selection,
 * inputs.scenario, seeds.master_seed, inputs.grid_model.fixture,
 * inputs.objective_config.objective). DRIFT NOTE: this is a hand-written
 * mirror of repo artefacts (the same discipline as the capability mirror,
 * security/capabilityMatrix.ts) — it can drift until hand-updated; the
 * records themselves are authoritative. No API serves library records in
 * this phase; none was invented.
 */
export const SCENARIO_LIBRARY_MIRROR: readonly ScenarioLibraryEntry[] = [
  {
    recordId: 'lib-01-minimal-single-asset',
    recordVersion: 'v1',
    purpose:
      'The minimal replayable case: one tenant, one site, ONE BESS; record round-trip, fail-loud pin verification, replay identity at the smallest scale',
    scaleClass: 'minimal',
    runtimeClass: 'seconds-class on the scripted backend (96 ticks × 1 asset + one dispatch slice) — not measured',
    backend: 'scripted',
    scenarioPin: 'library-minimal-single-asset:v1',
    masterSeed: 20260601,
    gridFixture: 'three_bus_radial',
    objective: 'SYNTHETIC_ENERGY_COST_DEVIATION_V1',
  },
  {
    recordId: 'lib-02-small-portfolio',
    recordVersion: 'v1',
    purpose:
      'The full synthetic-portfolio fixture (2 sites, BESS+PV+load+EVSE+meter per site, 1-min cadence); the replay-identity workhorse and negative-control base',
    scaleClass: 'small',
    runtimeClass: 'seconds-class on the scripted backend (1440 ticks × 10 assets + one dispatch slice) — not measured',
    backend: 'scripted',
    scenarioPin: 'synthetic-portfolio-exit-gate:v2',
    masterSeed: 20260601,
    gridFixture: 'three_bus_radial',
    objective: 'SYNTHETIC_ENERGY_COST_DEVIATION_V1',
  },
  {
    recordId: 'lib-03-medium-multi-site-meshed',
    recordVersion: 'v1',
    purpose:
      'Two sites against the MESHED two-feeder fixture (common substation, balanced only, TopologyScope::RadialAndMeshed per P4.34)',
    scaleClass: 'medium',
    runtimeClass: 'seconds-class on the scripted backend — not measured',
    backend: 'scripted',
    scenarioPin: 'synthetic-portfolio-exit-gate:v2',
    masterSeed: 20260601,
    gridFixture: 'meshed_two_feeder',
    objective: 'SYNTHETIC_ENERGY_COST_DEVIATION_V1',
  },
  {
    recordId: 'lib-04-stress-worker-timeout',
    recordVersion: 'v1',
    purpose:
      "Stress: the worker_process backend's bounded-timeout fail-closed path (a hung worker is killed; never a pass, never a retry)",
    scaleClass: 'stress',
    runtimeClass: 'bounded by the pinned worker timeout (30 s default) — not measured',
    backend: 'worker_process',
    backendNote: 'UNVERIFIED pandapower API — the leg was written, never executed',
    scenarioPin: 'synthetic-portfolio-exit-gate:v2',
    masterSeed: 20260601,
    gridFixture: 'three_bus_radial',
    objective: 'SYNTHETIC_ENERGY_COST_DEVIATION_V1',
  },
  {
    recordId: 'lib-05-stress-nonconvergence',
    recordVersion: 'v1',
    purpose:
      'Stress: a non-converged power flow routes FAIL-CLOSED (retry NONE), never a pass; convergence is the §19.4 grid-feasibility metric',
    scaleClass: 'stress',
    runtimeClass: 'bounded by the pinned worker timeout — not measured',
    backend: 'worker_process',
    backendNote: 'UNVERIFIED pandapower API — the leg was written, never executed',
    scenarioPin: 'synthetic-portfolio-exit-gate:v2',
    masterSeed: 20260601,
    gridFixture: 'three_bus_radial',
    objective: 'SYNTHETIC_ENERGY_COST_DEVIATION_V1',
  },
  {
    recordId: 'lib-06-stress-pv-ramp',
    recordVersion: 'v1',
    purpose:
      'Stress: PV ramp / forecast error (P3.26 scenario (b) + the v1-error-pv-ramp fixture); the stressor is the forecast, the telemetry stays all-GOOD',
    scaleClass: 'stress',
    runtimeClass: 'seconds-class on the scripted backend (identical volume to lib-02) — not measured',
    backend: 'scripted',
    scenarioPin: 'synthetic-portfolio-phase3-b-pv-ramp:v1',
    masterSeed: 20260601,
    gridFixture: 'three_bus_radial',
    objective: 'SYNTHETIC_ENERGY_COST_DEVIATION_V1',
  },
  {
    recordId: 'lib-07-stress-asset-dropout',
    recordVersion: 'v1',
    purpose:
      'Stress: asset dropout mid-horizon (P3.26 scenario (c)); the drop window is also the communication-loss analogue; the REAL P3.24 roll fires via the roll pin',
    scaleClass: 'stress',
    runtimeClass: 'seconds-class on the scripted backend — not measured',
    backend: 'scripted',
    scenarioPin: 'synthetic-portfolio-phase3-c-dropout:v1',
    masterSeed: 20260601,
    gridFixture: 'three_bus_radial',
    objective: 'SYNTHETIC_ENERGY_COST_DEVIATION_V1',
  },
  {
    recordId: 'lib-08-stress-reverse-power-flow',
    recordVersion: 'v1',
    purpose:
      'Stress: reverse power flow (+1.5 MW export dispatch target); the scripted-backend limitation is recorded honestly; the violation expectation is pre-registered for the worker leg',
    scaleClass: 'stress',
    runtimeClass: 'seconds-class on the scripted backend — not measured',
    backend: 'scripted',
    scenarioPin: 'library-minimal-single-asset:v1',
    masterSeed: 20260601,
    gridFixture: 'three_bus_radial',
    objective: 'SYNTHETIC_ENERGY_COST_DEVIATION_V1',
  },
];

/** The library mirror's drift + source note (rendered under the table). */
export const SCENARIO_LIBRARY_MIRROR_NOTE =
  'Source: scenarios/library/INDEX.md + the lib-*.json ScenarioRecord v3 ' +
  'files (the records are authoritative; this hand-written mirror can ' +
  'drift until hand-updated — the same recorded mirror discipline as the ' +
  'capability mirror). The runtime classes are DECLARED classes, never ' +
  'measured figures (measured runtimes are the P5.23 benchmark\'s ' +
  'obligation). No API serves the library in this phase; none was ' +
  'invented — the seeds, pins, and versions above are copied from the ' +
  'records, and a run history appears only under "Scenario runs" when ' +
  'run records exist.';

/** The library honesty note (rendered verbatim; §19.3 discipline). */
export const SCENARIO_LIBRARY_HONESTY_NOTE =
  'Every library record is SYNTHETIC (no IEEE feeder model is used or ' +
  'claimed anywhere: every record\'s per-case IEEE record reads ' +
  'included: false — no IEEE 13/34/123-bus distribution or 39/118-bus ' +
  'transmission model is available and neither licensing nor model ' +
  'availability is confirmed). Scale limit (§19.3, mandatory): the first ' +
  'feeder tests do NOT establish capability for a 10,000-bus utility ' +
  'network. Expected outcomes are pre-registered and UNRUN; when the ' +
  'scenarios are first run, a mismatch is a FINDING — never a reason to ' +
  'edit the expectations.';

// --- Replay (P5.18–P5.19): the recorded-inputs rule and the comparison -------

/** The replay rule (P5.18 step 2), rendered verbatim on the replay section. */
export const REPLAY_RECORDED_INPUTS_STATEMENT =
  'Replay re-executes a scenario from its RECORDED inputs ONLY (P5.18): ' +
  'any input that cannot be reproduced from the record is an ERROR, never ' +
  'a silent default — a replay that used new inputs would be exactly the ' +
  'failure mode this rule exists to prevent. The seed is a recorded input; ' +
  'no seed override exists on the replay path. Any deviation is flagged ' +
  '(P5.19), never silently substituted.';

/**
 * The comparison-serving gap (rendered on the replay section, verbatim):
 * no control-plane route serves ReplayComparisonRecords either.
 */
export const REPLAY_SERVING_GAP =
  'The reproducibility verdict (ReplayComparisonRecord: input-hash match, ' +
  'per-leg comparisons with named diffs, verdict identical / ' +
  'within_tolerance / mismatch) is produced by the sim-runner CLI ' +
  '(flexcore-sim-runner replay-run) as a JSON artefact. No control-plane ' +
  'route serves comparison records yet in this phase, so this workspace ' +
  'renders the replay COMMANDS and the comparison DISPLAY COMPONENT, and ' +
  'shows the gap instead of a fabricated verdict (the same display-' +
  'contract discipline as the run-record read). When a serving surface ' +
  'exists (P5.30), the component renders the record verbatim.';

/** The replay-run command shapes (the recorded CLI contract). */
export function replayRunCommand(recordFile: string, baselinePath?: string): string {
  return baselinePath === undefined
    ? `flexcore-sim-runner replay-run --record scenarios/library/${recordFile} --double-run`
    : `flexcore-sim-runner replay-run --record scenarios/library/${recordFile} --baseline ${baselinePath}`;
}

/** The closed comparison-kind vocabulary (replay_record.rs). */
export const REPLAY_COMPARISON_KINDS = ['double_run', 'baseline'] as const;

/** The closed reproducibility-verdict vocabulary (replay_record.rs). */
export const REPLAY_VERDICTS = ['identical', 'within_tolerance', 'mismatch'] as const;

/** The closed per-leg verdict vocabulary (replay_record.rs LegOutcome). */
export const REPLAY_LEG_VERDICTS = ['pass', 'within_tolerance', 'fail', 'not_compared'] as const;

/**
 * The verdict marker (distinct text, never colour alone — P5.12). An
 * unknown verdict renders verbatim with an explicit unknown marker — the
 * fail-honest display rule.
 */
export function replayVerdictMarker(verdict: string): string {
  if (verdict === 'identical') return '[REPRODUCIBLE: IDENTICAL]';
  if (verdict === 'within_tolerance') return '[REPRODUCIBLE: WITHIN TOLERANCE]';
  if (verdict === 'mismatch') return '[NOT REPRODUCIBLE: MISMATCH]';
  return `[UNKNOWN VERDICT — displayed verbatim: ${verdict}]`;
}

/** One compared leg's outcome (replay_record.rs LegOutcome, wire names). */
export interface ReplayLegOutcomeDto {
  leg: string;
  verdict: string;
  diff_count: number;
  diffs: string[];
  detail: string;
}

/**
 * The ReplayComparisonRecord wire mirror (apps/sim-runner/src/
 * replay_record.rs ReplayComparisonRecord, serde field-for-field in the
 * WIRE names; drift note: the Rust struct is authoritative — a payload
 * mismatch fails visibly instead of silently).
 */
export interface ReplayComparisonRecordDto {
  record_id: string;
  record_version: string;
  comparison_kind: string;
  scenario_id: string;
  scenario_version: string;
  master_seed: number;
  code_version: string;
  input_hash: string;
  input_hash_match: boolean;
  legs: ReplayLegOutcomeDto[];
  verdict: string;
  verdict_statement: string;
  tolerance_rule: string;
}

// --- The run-record consumption (the additive record fields, P5.23/P5.18) ----

/** One measured leg duration (metrics.rs LegTiming, wire names). */
export interface LegTimingDto {
  leg: string;
  duration_seconds: number;
  method: string;
}

/**
 * The per-leg timings block wire mirror (apps/sim-runner/src/metrics.rs
 * ScenarioRunTimings, wire names; additive on the run record — present
 * only when the run measured legs; an untimed leg is absent, never zero).
 */
export interface ScenarioRunTimingsDto {
  record_schema: string;
  subtask: string;
  code_version: string;
  scenario_id: string;
  scenario_version: string;
  master_seed: number;
  legs: LegTimingDto[];
  statement: string;
}

/**
 * One recorded P4.37 closed-loop consumption decision (scenario_run.rs
 * ConsumptionDecisionSummary, wire names).
 */
export interface ConsumptionDecisionSummaryDto {
  command_id: string;
  outcome_code: string;
  decision_ref: string;
  action: string;
  route?: string | null;
  reason: string;
  violated_constraints: string[];
  problem_inputs: string[];
  check_id: string;
  model_id_version: string;
  roll_trigger_note: string;
  statements: string[];
}

/**
 * The ADDITIVE run-record fields the workspace renders beyond the P5.22
 * summary mirror (api/scenarioRuns.ts, which this module does not modify):
 * `timings` (P5.23) and `consumption_decisions` (P5.18) travel on the wire
 * when present and are omitted otherwise. The summary mirror tolerates
 * additive fields; this interface names them so the render is typed and
 * renders ONLY what exists (never a placeholder for an absent block).
 */
export interface ScenarioRunRecordExtrasDto {
  timings?: ScenarioRunTimingsDto;
  consumption_decisions?: ConsumptionDecisionSummaryDto[];
}

/** Reads the additive fields off a run record (render-what-exists). */
export function runExtras(run: ScenarioRunRecordSummaryDto): ScenarioRunRecordExtrasDto {
  const extras = run as ScenarioRunRecordSummaryDto & ScenarioRunRecordExtrasDto;
  return {
    timings: extras.timings,
    consumption_decisions: extras.consumption_decisions,
  };
}

// --- Benchmark runs (§19.3–§19.4; the P5.23 template status) -----------------

/** The benchmark template status (rendered verbatim on the section). */
export const BENCHMARK_TEMPLATE_STATUS =
  'The Phase 5 benchmark report (ledger/phase-05-operator-console-simulator/' +
  '23_benchmark_report.md) is a PRE-REGISTERED TEMPLATE: every measured ' +
  'cell below is EMPTY, no measured figure exists for the Phase 5 library ' +
  'cases yet, and none is claimed. The human\'s single recorded run fills ' +
  'the template\'s §7 tables FROM the produced records only. An empty cell ' +
  'is an honest gap — a written zero would be a false claim; a ' +
  'not-measurable field is recorded not_measured WITH its reason.';

/** The binding rules of the template (rendered verbatim; §0 rules 1–6). */
export const BENCHMARK_BINDING_RULES_NOTE =
  'Binding rules (23_benchmark_report.md §0, restated): fill a cell only ' +
  'from the produced run record; the figures are the v0.3.0 BASELINE, ' +
  'never targets (report §19.4 verbatim: no arbitrary universal targets); ' +
  'raw numbers only — no scaling claim from any column; CI runs are not ' +
  'baseline figures; conditions travel with every figure; every published ' +
  'figure is labelled SIMULATED with its scenario id + version. The grid ' +
  'leg is an AC power-flow evaluation, NOT an AC-OPF solve; HiGHS is ' +
  'LP/QP/MIP — never SOCP.';

/** The Table A/B case rows of the template (§7) and their honest status. */
export const BENCHMARK_TABLE_CASES: readonly { recordId: string; note?: string }[] = [
  { recordId: 'lib-01-minimal-single-asset' },
  { recordId: 'lib-02-small-portfolio' },
  { recordId: 'lib-03-medium-multi-site-meshed' },
  { recordId: 'lib-06-stress-pv-ramp' },
  { recordId: 'lib-07-stress-asset-dropout' },
  { recordId: 'lib-08-stress-reverse-power-flow' },
];

/** The lib-04/lib-05 exclusion note (rendered with the case table). */
export const BENCHMARK_WORKER_LEG_NOTE =
  'lib-04-stress-worker-timeout and lib-05-stress-nonconvergence run the ' +
  'worker_process backend: their benchmark rows belong to the P4.35 ' +
  'feasibility record once that leg executes — until then their cells ' +
  'stay EMPTY with the reason "worker leg UNVERIFIED — see ' +
  'scenarios/library/INDEX.md" (pandapower 3.5.6 was never installed in ' +
  'the authoring loop).';

/** The Table B leg columns (the P5.23 timings surface, metrics.rs legs). */
export const BENCHMARK_TIMING_LEGS = [
  'telemetry_ingest_pump',
  'dispatch_slice',
  'horizon_sweep',
  'fault_probes',
  'record_build',
] as const;

/** The Table C not_measured reasons (rendered verbatim per column group). */
export const BENCHMARK_NOT_MEASURED: readonly { area: string; reason: string }[] = [
  { area: 'scheduling (solver runtime, objective value, feasible intervals, schedule deviation)', reason: 'not_measured (no real solve is wired into the composition — the P3.27 G5 rule carries forward)' },
  { area: 'dispatch (acknowledgement latency, tracking error)', reason: 'not_measured (no latency timer exists on the issue path in this composition — template §3.3)' },
  { area: 'flexibility (available vs delivered MW/MWh, uncertainty calibration, shortfall)', reason: 'not_measured (no flexibility envelope delivery path runs in the 0.3.0 composition — template §3.4)' },
  { area: 'ADMM / GNN / MARL rows', reason: 'out of scope for 0.3.0 — no ADMM/GNN/MARL module exists in any phase; the rows state the exclusion honestly (nothing measured, nothing claimed)' },
  { area: 'recovery time after restart', reason: 'NOT measured — P5.24\'s template owns the pre-registered manual procedure' },
];

/** The Table C count-columns note (grid/asset/dispatch counts). */
export const BENCHMARK_COUNT_COLUMNS_NOTE =
  'The grid / asset-model / dispatch-count cells fill FROM the produced ' +
  'records\' verdicts and summaries only; a case whose backend is ' +
  'scripted records its counts from the FIXTURE outcome and names the ' +
  'backend — a fixture count is a composition fact, never a physics ' +
  'result. Until records exist the cells stay EMPTY (an honest gap, ' +
  'never a zero).';

// --- Result comparison (§12.6 outcomes; the P3.23 comparability rule) --------

/**
 * The comparison-reuse note (rendered on the result-comparison section):
 * the comparability rule is the Schedules workspace's P3.23 rule, REUSED
 * by import — `assessComparison` lives in Schedules.tsx and is imported
 * here, so there is exactly ONE rule and no mirror to drift.
 */
export const COMPARISON_REUSE_NOTE =
  'Comparability rule REUSED, not re-implemented: the Simulation Lab ' +
  'imports assessComparison from the Schedules workspace (P5.05), which ' +
  'enforces the P3.23 shared-input rule — candidates are COMPARABLE only ' +
  'when at least two were found AND every input snapshot reference is ' +
  'identical; a cross-input comparison is labelled INCOMPARABLE and is ' +
  'never presented as equivalent. The rule lives in ONE place ' +
  '(Schedules.tsx); this workspace renders its verdicts, it does not ' +
  'copy the rule.';

/** The comparison-scope note (what the Simulation Lab compares). */
export const COMPARISON_SCOPE_NOTE =
  'This section compares the §12.6 optimization-run records the ' +
  'tenant-scoped read returns (schedule alternatives; Tier comparisons ' +
  'where records exist), with the per-candidate §19.4 metrics the record ' +
  'carries (objective value with units, solver + version, status, input ' +
  'snapshot). A scenario-run comparison (same scenario, replay legs) is ' +
  'the REPLAY section\'s concern and uses the record\'s own ' +
  'pre-registered tolerance — the two comparisons are never conflated.';

