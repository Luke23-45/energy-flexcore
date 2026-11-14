// P5.22 — Simulation Lab workspace tests (render + visibility assertions
// on mocked run records).
//
// UNRUN — written, never executed (the Phase 5 loop bans: no npm install,
// no vitest run; decision record 21_24_faults_visibility.md). Rendering
// uses react-dom/server's renderToString (existing dependency, no DOM
// environment), the loader is tested at the pure-loader level with a fake
// ApiClient, pinning the display-contract path and the no-tenant-selector
// rule (§16.2).
//
// What the tests pin (the P5.22 acceptance core — the reviewer test):
//   1. a fault-injected run ALWAYS carries the FAULT-INJECTED tag and the
//      per-fault injected/caused/response columns with the evidence refs;
//   2. the platform's response renders verbatim (reject/hold/fallback/
//      failed/per_asset/not_observed_in_this_run) — never re-interpreted
//      into a pass;
//   3. a failed run renders its failure journal (per-command and per-grid
//      check rows) — a failure is never a silent blank screen;
//   4. the alarm mapping is honest over the closed P5.31 vocabulary:
//      failed → DISPATCH_FAILURE, fallback → DISPATCH_FALLBACK, and every
//      other response maps to NO alarm with the no-invented-alarm note;
//   5. the clean re-run affordance states the exact command AND the gap
//      (no console trigger API yet) — a documented action, not a button;
//   6. a clean run states the absence of a fault journal honestly;
//   7. every run carries the SIMULATED label with its scenario id + version;
//   8. the run-record serving gap renders when the read fails (no
//      fabricated runs), and the loader never sends a tenant parameter.
//
// P5.08 — ADDITIVE extensions (the P5.22 assertions above are unchanged):
//   9. the scenario library renders the lib-01…08 mirror with pins/seeds
//      and the §19.3 honesty notes (synthetic, IEEE exclusion, scale limit);
//  10. the replay section renders the recorded-inputs rule, the per-record
//      replay commands, and the serving gap; ReplayComparisonView renders
//      the input-hash check, per-leg diffs, and the verdict verbatim;
//  11. the benchmark section renders the template status: EMPTY cells,
//      the not_measured reasons verbatim, the binding rules;
//  12. the result comparison renders §12.6 candidates through the imported
//      P3.23 comparability rule (comparable/incomparable/empty/error);
//  13. the run-record consumption renders the additive fields (statements,
//      timings, consumption decisions) when they exist and nothing when
//      they do not;
//  14. the P5.27 mode-gate notice renders beside the mode indicator
//      (defence-in-depth boundary + the configured-but-not-exercised
//      vocabulary), and the workspace's own actions are mode-safe.
import { describe, expect, it } from 'vitest';
import { renderToString } from 'react-dom/server';
import type { ApiClient } from '../api/client';
import { SCENARIO_RUNS_PATH, type ScenarioRunRecordSummaryDto } from '../api/scenarioRuns';
import type { OptimizationRunDto } from '../api/types';
import {
  BENCHMARK_NOT_MEASURED,
  BENCHMARK_TEMPLATE_STATUS,
  CLEAN_RERUN_STATEMENT,
  COMPARISON_REUSE_NOTE,
  NO_FAULT_JOURNAL_STATEMENT,
  REPLAY_RECORDED_INPUTS_STATEMENT,
  REPLAY_SERVING_GAP,
  RUN_RECORD_API_GAP,
  SCENARIO_LIBRARY_HONESTY_NOTE,
  SCENARIO_LIBRARY_MIRROR,
  SCENARIO_LIBRARY_MIRROR_NOTE,
  SIMULATED_RESULTS_STATEMENT,
  failureRows,
  faultAlarmMapping,
  isFaultInjected,
  replayRunCommand,
  replayVerdictMarker,
  runExtras,
} from './simulationLabData';
import {
  ResultComparisonSection,
  ReplayComparisonView,
  SimulationLabView,
  loadSimulationLab,
  type SimulationLabData,
} from './SimulationLab';

/** A fault journal entry fixture (the wire shape of FaultJournalEntryDto). */
function entry(overrides: Partial<{
  fault_id: string;
  kind: string;
  injection_point: string;
  at_unix_millis: number;
  observed_effect: string;
  platform_response: string;
  evidence_reference: string;
  pre_registered_reference: string;
}> = {}) {
  return {
    fault_id: 'p2-drop-site-north-bess-1-1780279200000',
    kind: 'unavailable_asset',
    injection_point:
      'leg=telemetry_source, target=site-north-bess-1, window [1780279200000, 1780279800000) ms scenario-clock',
    at_unix_millis: 1780279200000,
    observed_effect:
      'no frames exist for the window: the run record carries the availability change at 1780279200000 ms',
    platform_response: 'not_observed_in_this_run',
    evidence_reference: 'availability_change:site-north-bess-1@1780279200000',
    pre_registered_reference:
      'P4.31 F8 (unavailable asset ⇒ FAILED + fallback) + P2.15 drop + P3.24 roll',
    ...overrides,
  };
}

/** A fault-injected run fixture. */
function injectedRun(overrides: Partial<ScenarioRunRecordSummaryDto> = {}): ScenarioRunRecordSummaryDto {
  return {
    scenario_id: 'synthetic-portfolio-exit-gate',
    scenario_version: 'v2',
    master_seed: 20260601,
    code_version: 'test-code',
    simulator_source_id: 'sim://synthetic-portfolio-exit-gate',
    records_ingested: 8640,
    commands: [
      {
        command_id: 'cmd-1',
        asset_key: 'site-north-bess-1',
        final_state: 'FAILED',
        issue_outcome: 'failed',
        acceptance_reference: null,
        tracking_verdict: null,
      },
    ],
    grid_checks: [
      {
        attempt_id: 'attempt-1',
        backend: 'scripted',
        provider_id_version: 'pandapower@3.5.6',
        verdict: 'passed',
        reason: null,
      },
    ],
    fault_journal: {
      entries: [
        entry({
          platform_response: 'failed',
          observed_effect:
            "the command's issue outcome is 'failed' (a delivery in the window has no acceptance — the transport-timeout mapping, D-P4.16-4)",
          evidence_reference: 'command:cmd-1:issue_outcome',
        }),
        entry({
          fault_id: 'p2-delay-site-south-bess-1-1780282800000',
          kind: 'stale_telemetry',
          platform_response: 'reject',
          observed_effect: '240 ingested sample(s) carried the STALE quality state (quality_counts)',
          evidence_reference: 'quality_counts:STALE',
        }),
        entry({
          fault_id: 'SF-SOLVER-1',
          kind: 'solver_timeout',
          platform_response: 'cross_referenced_to_phase_4_suite',
          observed_effect:
            'not injected at the scenario level in this run — the pre-registered platform outcomes live in the P4.31 suite',
          evidence_reference: 'P4.31 F9a (hold, no transition) + F9b',
        }),
      ],
      observed_count: 2,
      cross_referenced_count: 1,
      statement:
        'The fault journal records INJECTED SIMULATOR faults only: every entry is scenario configuration ... A fault whose effect this run did not observe is recorded not_observed_in_this_run, never passed.',
    },
    ...overrides,
  };
}

/** A clean (uninjected) run fixture. */
function cleanRun(): ScenarioRunRecordSummaryDto {
  return {
    scenario_id: 'synthetic-portfolio-exit-gate',
    scenario_version: 'v1',
    master_seed: 20260601,
    code_version: 'clean-code',
    simulator_source_id: 'sim://synthetic-portfolio-exit-gate',
    records_ingested: 8640,
    commands: [
      {
        command_id: 'cmd-2',
        asset_key: 'site-north-pv-1',
        final_state: 'COMPLETED',
        issue_outcome: 'accepted',
        acceptance_reference: 'sim-accept:cmd-2/1@1780275660000',
        tracking_verdict: 'completed',
      },
    ],
    grid_checks: [
      {
        attempt_id: 'attempt-2',
        backend: 'scripted',
        provider_id_version: 'pandapower@3.5.6',
        verdict: 'passed',
        reason: null,
      },
    ],
  };
}

function data(overrides: Partial<SimulationLabData> = {}): SimulationLabData {
  return {
    runs: [injectedRun(), cleanRun()],
    runsError: null,
    alarms: null,
    alarmsError: null,
    comparison: [],
    comparisonError: null,
    ...overrides,
  };
}

/** A §12.6 optimization-run fixture (the result-comparison input). */
function optimizationRun(overrides: Partial<OptimizationRunDto> = {}): OptimizationRunDto {
  return {
    candidate_id: 'cand-1',
    status: 'FEASIBLE',
    objective_value: 12.5,
    objective_units: 'EUR',
    solver_name: 'HiGHS',
    solver_version: '1.15.1',
    provider_id: 'highspy',
    provider_version: '1.15.1',
    input_snapshot_id: 'snap-1',
    ...overrides,
  } as OptimizationRunDto;
}

/** A run record carrying the additive P5.23/P5.18 fields (cast: the
 * summary mirror tolerates additive wire fields). */
function runWithExtras(): ScenarioRunRecordSummaryDto {
  return {
    ...cleanRun(),
    statements: [
      'Every command in a scenario run is issued to the SIMULATOR and nothing else; an acknowledgement is application-layer acceptance, never physical delivery; the entire run is synthetic simulation telemetry on the scenario clock (0.x line).',
    ],
    timings: {
      record_schema: 'flexcore-scenario-run-timings/v1',
      subtask: 'P5.23',
      code_version: 'clean-code',
      scenario_id: 'synthetic-portfolio-exit-gate',
      scenario_version: 'v1',
      master_seed: 20260601,
      legs: [
        {
          leg: 'dispatch_slice',
          duration_seconds: 0.25,
          method: 'std::time::Instant around the leg (a single sequential measurement)',
        },
      ],
      statement: 'Per-leg wall-clock durations of ONE sequential scenario-run composition ...',
    },
    consumption_decisions: [
      {
        command_id: 'cmd-2',
        outcome_code: 'no_violation',
        decision_ref: 'D-P4.07-1',
        action: 'no_consumption',
        route: 'none_required',
        reason: 'the pipeline outcome routed with no violated constraint',
        violated_constraints: [],
        problem_inputs: [],
        check_id: 'attempt-2',
        model_id_version: 'feeder-a:1',
        roll_trigger_note: 'the P4.37 open item travels per decision',
        statements: ['the closed-loop wiring statement (verbatim)'],
      },
    ],
  } as unknown as ScenarioRunRecordSummaryDto;
}

/** A ReplayComparisonRecord fixture (the P5.19 output shape). */
function comparisonRecord(overrides: Record<string, unknown> = {}): Parameters<typeof ReplayComparisonView>[0]['record'] {
  return {
    record_id: 'lib-02-small-portfolio',
    record_version: 'v1',
    comparison_kind: 'double_run',
    scenario_id: 'synthetic-portfolio-exit-gate',
    scenario_version: 'v2',
    master_seed: 20260601,
    code_version: 'proof-code',
    input_hash: 'a1b2c3d4e5f60718a1b2c3d4e5f60718',
    input_hash_match: true,
    legs: [
      {
        leg: 'input_hash',
        verdict: 'pass',
        diff_count: 0,
        diffs: [],
        detail: 'the recomputed hash matches the recorded hash',
      },
      {
        leg: 'telemetry',
        verdict: 'pass',
        diff_count: 0,
        diffs: [],
        detail: 'the frozen P2.24 tuple; byte-exact (1440)',
      },
      {
        leg: 'run_record',
        verdict: 'identical',
        diff_count: 0,
        diffs: [],
        detail: 'the ScenarioRunRecord JSON, structural diff',
      },
    ],
    verdict: 'identical',
    verdict_statement: 'all legs matched within the pre-registered tolerance',
    tolerance_rule: 'telemetry: byte_identical, run record: byte_identical (pre-registered)',
    ...overrides,
  } as Parameters<typeof ReplayComparisonView>[0]['record'];
}

describe('the run-record derivation helpers (pure, tested before render)', () => {
  it('tags a run fault-injected exactly when the journal carries entries', () => {
    expect(isFaultInjected(injectedRun())).toBe(true);
    expect(isFaultInjected(cleanRun())).toBe(false);
    const emptyJournal = injectedRun({
      fault_journal: { entries: [], observed_count: 0, cross_referenced_count: 0, statement: 'x' },
    });
    expect(isFaultInjected(emptyJournal)).toBe(false);
  });

  it('derives the failure rows of a failed run (never a blank screen)', () => {
    const rows = failureRows(injectedRun());
    expect(rows.length).toBe(1);
    expect(rows[0].label).toContain('cmd-1');
    expect(rows[0].detail).toContain('FAILED');
    expect(failureRows(cleanRun())).toHaveLength(0);
  });

  it('maps fault responses onto the CLOSED P5.31 alarm vocabulary honestly', () => {
    const failed = faultAlarmMapping(entry({ platform_response: 'failed' }));
    expect(failed.alarmCode).toBe('DISPATCH_FAILURE');
    const fallback = faultAlarmMapping(entry({ platform_response: 'fallback' }));
    expect(fallback.alarmCode).toBe('DISPATCH_FALLBACK');
    // Every other response maps to NO alarm — none invented.
    for (const response of [
      'reject',
      'hold',
      'per_asset',
      'not_observed_in_this_run',
      'cross_referenced_to_phase_4_suite',
    ]) {
      const mapped = faultAlarmMapping(entry({ platform_response: response }));
      expect(mapped.alarmCode).toBeNull();
      expect(mapped.note).toContain('no alarm code exists');
      expect(mapped.note).toContain(response);
    }
  });
});

describe('loadSimulationLab (mocked fetch, the display-contract path)', () => {
  it('reads the run records, the alarms, and the §12.6 records without a tenant parameter', async () => {
    const paths: string[] = [];
    const client: ApiClient = {
      async get<T>(path: string): Promise<T> {
        paths.push(path);
        if (path === SCENARIO_RUNS_PATH) return { data: [injectedRun()] } as T;
        if (path === '/api/v1/alarms') return { data: [], sources: {}, derived_at: '', derivation_statement: '', model_version: '', next_cursor: null } as T;
        if (path.startsWith('/api/v1/optimization-runs')) return { data: [optimizationRun()] } as T;
        throw new Error(`no fixture for ${path}`);
      },
      async request<T>(path: string): Promise<T> {
        return this.get<T>(path);
      },
    };
    const result = await loadSimulationLab(client);
    expect(result.runs).toHaveLength(1);
    expect(result.runsError).toBeNull();
    expect(result.comparison).toHaveLength(1);
    expect(result.comparisonError).toBeNull();
    expect(paths).toContain(SCENARIO_RUNS_PATH);
    expect(paths).toContain('/api/v1/alarms');
    expect(paths.some((path) => path.startsWith('/api/v1/optimization-runs'))).toBe(true);
    for (const path of paths) {
      expect(path).not.toContain('tenant');
    }
  });

  it('keeps a failed run-record read as the verbatim error + the gap', async () => {
    const client: ApiClient = {
      async get<T>(path: string): Promise<T> {
        if (path === '/api/v1/alarms') return { data: [] } as T;
        if (path.startsWith('/api/v1/optimization-runs')) return { data: [] } as T;
        throw new Error('not_found: no such route');
      },
      async request<T>(path: string): Promise<T> {
        return this.get<T>(path);
      },
    };
    const result = await loadSimulationLab(client);
    expect(result.runs).toBeNull();
    expect(result.runsError).toContain('not_found: no such route');
  });

  it('keeps a failed §12.6 read as its own verbatim error (one failing surface never blanks the others)', async () => {
    const client: ApiClient = {
      async get<T>(path: string): Promise<T> {
        if (path === SCENARIO_RUNS_PATH) return { data: [cleanRun()] } as T;
        if (path === '/api/v1/alarms') return { data: [] } as T;
        throw new Error('forbidden: role does not permit this read');
      },
      async request<T>(path: string): Promise<T> {
        return this.get<T>(path);
      },
    };
    const result = await loadSimulationLab(client);
    expect(result.runs).toHaveLength(1);
    expect(result.comparison).toBeNull();
    expect(result.comparisonError).toContain('forbidden: role does not permit this read');
  });
});

describe('SimulationLabView (render honesty — the reviewer test)', () => {
  it('renders the injected/caused/response columns with evidence, verbatim', () => {
    const html = renderToString(
      <SimulationLabView state={{ status: 'ready', data: data(), readAt: '2026-11-11T10:00:00Z' }} />,
    );
    // The injected tag is structural (P5.22 step 1: an operator can always
    // tell a run is fault-injected).
    expect(html).toContain('[FAULT-INJECTED]');
    // Per-fault columns: what was injected, what it caused, the response.
    expect(html).toContain('p2-drop-site-north-bess-1-1780279200000');
    expect(html).toContain('unavailable_asset');
    expect(html).toContain('leg=telemetry_source');
    expect(html).toContain('availability_change:site-north-bess-1@1780279200000');
    // The response renders VERBATIM (never re-interpreted into a pass).
    expect(html).toContain('not_observed_in_this_run');
    expect(html).toContain('cross_referenced_to_phase_4_suite');
    // The reused pre-registration travels with the row.
    expect(html).toContain('P4.31 F8');
    // The journal statement renders verbatim.
    expect(html).toContain('fault-journal-statement');
    expect(html).toContain('never passed');
  });

  it('renders the failure journal of a failed run (never a blank screen)', () => {
    const html = renderToString(
      <SimulationLabView state={{ status: 'ready', data: data(), readAt: 'x' }} />,
    );
    expect(html).toContain('Failure journal of this run');
    expect(html).toContain('cmd-1');
    expect(html).toContain('final state FAILED');
  });

  it('renders the honest alarm mapping per fault (none invented)', () => {
    const html = renderToString(
      <SimulationLabView state={{ status: 'ready', data: data(), readAt: 'x' }} />,
    );
    expect(html).toContain('DISPATCH_FAILURE');
    expect(html).toContain('no alarm code exists');
    expect(html).toContain('no alarm is invented');
  });

  it('states the clean re-run command AND the gap for an injected run', () => {
    const html = renderToString(
      <SimulationLabView state={{ status: 'ready', data: data(), readAt: 'x' }} />,
    );
    expect(html).toContain('Clear injections and re-run clean');
    expect(html).toContain('flexcore-sim-runner scenario-run');
    expect(html).toContain(CLEAN_RERUN_STATEMENT);
    expect(html).toContain('the console-side trigger API is not served yet');
  });

  it('labels a clean run as clean and simulated (never injected-looking)', () => {
    const html = renderToString(
      <SimulationLabView state={{ status: 'ready', data: data(), readAt: 'x' }} />,
    );
    expect(html).toContain(NO_FAULT_JOURNAL_STATEMENT);
    expect(html).toContain('no-fault-journal');
    // Every run labels itself simulated with its scenario id + version.
    expect(html).toContain('sim://synthetic-portfolio-exit-gate');
    expect(html).toContain('synthetic-portfolio-exit-gate:v2');
    expect(html).toContain(SIMULATED_RESULTS_STATEMENT);
  });

  it('renders the run-record serving gap when the read fails (no fabricated runs)', () => {
    const html = renderToString(
      <SimulationLabView
        state={{
          status: 'ready',
          data: { runs: null, runsError: 'not_found: no such route', alarms: null, alarmsError: null, comparison: null, comparisonError: null },
          readAt: 'x',
        }}
      />,
    );
    expect(html).toContain(RUN_RECORD_API_GAP);
    expect(html).toContain('not_found: no such route');
    expect(html).not.toContain('cmd-1');
  });

  it('renders the honest empty, loading, and error states', () => {
    const empty = renderToString(
      <SimulationLabView
        state={{
          status: 'ready',
          data: { runs: [], runsError: null, alarms: null, alarmsError: null, comparison: [], comparisonError: null },
          readAt: 'x',
        }}
      />,
    );
    expect(empty).toContain('runs-empty');
    expect(empty).toContain('No run is fabricated');
    const loading = renderToString(<SimulationLabView state={{ status: 'loading' }} />);
    expect(loading).toContain('Loading the scenario-run records and alarms');
    const error = renderToString(
      <SimulationLabView state={{ status: 'error', error: 'unauthenticated: missing token' }} />,
    );
    expect(error).toContain('unauthenticated: missing token');
  });
});

// --- P5.08 — the full §20.1 item 9 workspace (additive) ------------------------

describe('P5.08 helpers (pure, tested before render)', () => {
  it('mirrors the 8-record library with its pins, seeds, and declared runtime classes', () => {
    expect(SCENARIO_LIBRARY_MIRROR).toHaveLength(8);
    expect(SCENARIO_LIBRARY_MIRROR.map((entry) => entry.recordId)).toEqual([
      'lib-01-minimal-single-asset',
      'lib-02-small-portfolio',
      'lib-03-medium-multi-site-meshed',
      'lib-04-stress-worker-timeout',
      'lib-05-stress-nonconvergence',
      'lib-06-stress-pv-ramp',
      'lib-07-stress-asset-dropout',
      'lib-08-stress-reverse-power-flow',
    ]);
    for (const entry of SCENARIO_LIBRARY_MIRROR) {
      expect(entry.recordVersion).toBe('v1');
      expect(entry.masterSeed).toBe(20260601);
      expect(entry.runtimeClass).toContain('not measured');
      expect(entry.objective).toBe('SYNTHETIC_ENERGY_COST_DEVIATION_V1');
    }
    const worker = SCENARIO_LIBRARY_MIRROR.find((entry) => entry.recordId === 'lib-04-stress-worker-timeout');
    expect(worker?.backend).toBe('worker_process');
    expect(worker?.backendNote).toContain('UNVERIFIED pandapower API');
  });

  it('derives the replay-run commands in both comparison kinds', () => {
    expect(replayRunCommand('lib-01-minimal-single-asset.json')).toBe(
      'flexcore-sim-runner replay-run --record scenarios/library/lib-01-minimal-single-asset.json --double-run',
    );
    expect(replayRunCommand('lib-01-minimal-single-asset.json', 'run-record.json')).toBe(
      'flexcore-sim-runner replay-run --record scenarios/library/lib-01-minimal-single-asset.json --baseline run-record.json',
    );
  });

  it('marks the replay verdicts distinctly and never maps an unknown verdict', () => {
    expect(replayVerdictMarker('identical')).toBe('[REPRODUCIBLE: IDENTICAL]');
    expect(replayVerdictMarker('within_tolerance')).toBe('[REPRODUCIBLE: WITHIN TOLERANCE]');
    expect(replayVerdictMarker('mismatch')).toBe('[NOT REPRODUCIBLE: MISMATCH]');
    expect(replayVerdictMarker('something_else')).toContain('UNKNOWN VERDICT');
    expect(replayVerdictMarker('something_else')).toContain('something_else');
  });

  it('extracts the additive run-record fields (render-what-exists)', () => {
    const plain = runExtras(cleanRun());
    expect(plain.timings).toBeUndefined();
    expect(plain.consumption_decisions).toBeUndefined();
    const extras = runExtras(runWithExtras());
    expect(extras.timings?.legs).toHaveLength(1);
    expect(extras.timings?.legs[0].leg).toBe('dispatch_slice');
    expect(extras.consumption_decisions?.[0].action).toBe('no_consumption');
  });
});

describe('P5.08 render honesty (the full §20.1 item 9 contract)', () => {
  it('renders the scenario library with pins/seeds and the §19.3 honesty notes', () => {
    const html = renderToString(<SimulationLabView state={{ status: 'ready', data: data(), readAt: 'x' }} />);
    expect(html).toContain('scenario-library');
    expect(html).toContain('lib-01-minimal-single-asset');
    expect(html).toContain('lib-08-stress-reverse-power-flow');
    expect(html).toContain('library-minimal-single-asset:v1');
    expect(html).toContain('20260601');
    expect(html).toContain('DECLARED, never measured');
    expect(html).toContain(SCENARIO_LIBRARY_MIRROR_NOTE.slice(0, 60));
    expect(html).toContain(SCENARIO_LIBRARY_HONESTY_NOTE.slice(0, 60));
    expect(html).toContain('10,000-bus utility');
    expect(html).toContain('included: false');
  });

  it('renders the replay section: recorded-inputs rule, per-record commands, serving gap', () => {
    const html = renderToString(<SimulationLabView state={{ status: 'ready', data: data(), readAt: 'x' }} />);
    expect(html).toContain('replay-section');
    expect(html).toContain(REPLAY_RECORDED_INPUTS_STATEMENT.slice(0, 60));
    expect(html).toContain('flexcore-sim-runner replay-run --record scenarios/library/lib-02-small-portfolio.json --double-run');
    expect(html).toContain(REPLAY_SERVING_GAP.slice(0, 60));
    expect(html).toContain('never a fabricated verdict');
  });

  it('renders the benchmark template status: EMPTY cells, not_measured reasons, binding rules', () => {
    const html = renderToString(<SimulationLabView state={{ status: 'ready', data: data(), readAt: 'x' }} />);
    expect(html).toContain('benchmark-section');
    expect(html).toContain(BENCHMARK_TEMPLATE_STATUS.slice(0, 60));
    // Every Table A/B measured cell renders the honest gap (never a zero).
    const emptyCells = html.split('benchmark-cell-empty').length - 1;
    expect(emptyCells).toBeGreaterThan(0);
    expect(html).toContain('(EMPTY — honest gap, never a zero)');
    for (const entry of BENCHMARK_NOT_MEASURED) {
      expect(html).toContain(entry.reason.slice(0, 50));
    }
    expect(html).toContain('worker leg UNVERIFIED');
    expect(html).toContain('never targets');
    expect(html).toContain('NOT an AC-OPF');
  });

  it('renders the run-record consumption: statements, timings, consumption decisions', () => {
    const html = renderToString(
      <SimulationLabView
        state={{ status: 'ready', data: data({ runs: [runWithExtras()] }), readAt: 'x' }}
      />,
    );
    expect(html).toContain('run-record-consumption');
    expect(html).toContain('issued to the SIMULATOR and nothing else');
    expect(html).toContain('flexcore-scenario-run-timings/v1');
    expect(html).toContain('dispatch_slice');
    expect(html).toContain('run-timings');
    expect(html).toContain('D-P4.07-1');
    expect(html).toContain('no_consumption');
    expect(html).toContain('consumption-cmd-2');
  });

  it('renders no consumption block when the record carries none (no placeholder)', () => {
    const html = renderToString(
      <SimulationLabView state={{ status: 'ready', data: data({ runs: [cleanRun()] }), readAt: 'x' }} />,
    );
    expect(html).not.toContain('run-timings');
    expect(html).not.toContain('run-consumption-decisions');
    expect(html).not.toContain('run-statements');
  });
});

describe('P5.08 result comparison (the imported P3.23 rule)', () => {
  it('labels same-input candidates COMPARABLE and renders per-candidate metrics', () => {
    const html = renderToString(
      <ResultComparisonSection
        comparison={[
          optimizationRun({ candidate_id: 'cand-1' }),
          optimizationRun({ candidate_id: 'cand-2', objective_value: 13.75 }),
        ]}
        comparisonError={null}
      />,
    );
    expect(html).toContain('comparison-verdict');
    expect(html).toContain('COMPARABLE');
    expect(html).toContain('snap-1');
    expect(html).toContain('12.5 EUR');
    expect(html).toContain('HiGHS 1.15.1');
    expect(html).toContain('SIMULATED / solver-produced');
  });

  it('labels cross-input candidates INCOMPARABLE (never presented as equivalent)', () => {
    const html = renderToString(
      <ResultComparisonSection
        comparison={[
          optimizationRun({ candidate_id: 'cand-1', input_snapshot_id: 'snap-1' }),
          optimizationRun({ candidate_id: 'cand-2', input_snapshot_id: 'snap-2' }),
        ]}
        comparisonError={null}
      />,
    );
    expect(html).toContain('INCOMPARABLE');
    expect(html).toContain('do not share one input snapshot');
  });

  it('renders the honest empty and the verbatim read error', () => {
    const empty = renderToString(<ResultComparisonSection comparison={[]} comparisonError={null} />);
    expect(empty).toContain('comparison-empty');
    expect(empty).toContain('none is fabricated');
    const error = renderToString(
      <ResultComparisonSection comparison={null} comparisonError="forbidden: role does not permit this read" />,
    );
    expect(error).toContain('comparison-read-gap');
    expect(error).toContain('forbidden: role does not permit this read');
    expect(error).not.toContain('cand-1');
  });

  it('carries the reuse note (one rule, no copy)', () => {
    const html = renderToString(
      <ResultComparisonSection comparison={[optimizationRun()]} comparisonError={null} />,
    );
    expect(html).toContain(COMPARISON_REUSE_NOTE.slice(0, 60));
    expect(html).toContain('assessComparison');
  });
});

describe('P5.08 ReplayComparisonView (the reproducibility verdict display)', () => {
  it('renders the input-hash match, the per-leg table, and the verdict verbatim', () => {
    const html = renderToString(<ReplayComparisonView record={comparisonRecord()} />);
    expect(html).toContain('replay-comparison');
    expect(html).toContain('a1b2c3d4e5f60718a1b2c3d4e5f60718');
    expect(html).toContain('MATCHES the recorded hash');
    expect(html).toContain('replay-leg-telemetry');
    expect(html).toContain('replay-leg-run_record');
    expect(html).toContain('[REPRODUCIBLE: IDENTICAL]');
    expect(html).toContain('byte_identical');
    expect(html).toContain('double_run');
  });

  it('renders an input-hash MISMATCH as the tamper finding it is', () => {
    const html = renderToString(
      <ReplayComparisonView
        record={comparisonRecord({
          input_hash_match: false,
          verdict: 'mismatch',
          verdict_statement: 'the input hash leg failed',
        })}
      />,
    );
    expect(html).toContain('DOES NOT MATCH the recorded hash');
    expect(html).toContain('tamper finding');
    expect(html).toContain('[NOT REPRODUCIBLE: MISMATCH]');
  });

  it('renders unknown vocabulary values verbatim with the unknown marker', () => {
    const html = renderToString(
      <ReplayComparisonView
        record={comparisonRecord({
          comparison_kind: 'triple_run',
          verdict: 'unclear',
          legs: [
            {
              leg: 'input_hash',
              verdict: 'inconclusive',
              diff_count: 1,
              diffs: ['path x: reference a vs candidate b'],
              detail: 'unknown vocabulary fixture',
            },
          ],
        })}
      />,
    );
    expect(html).toContain('triple_run');
    expect(html).toContain('outside the closed comparison-kind vocabulary');
    expect(html).toContain('outside the closed verdict vocabulary');
    expect(html).toContain('outside the closed leg-verdict vocabulary');
    expect(html).toContain('inconclusive');
  });
});

describe('P5.08/P5.27 mode interaction on the workspace', () => {
  it('renders the mode-gate notice beside the mode indicator (the enforcement layer)', () => {
    const html = renderToString(<SimulationLabView state={{ status: 'ready', data: data(), readAt: 'x' }} />);
    expect(html).toContain('control-authority-mode-indicator');
    expect(html).toContain('control-authority-mode-gate');
    expect(html).toContain('DEFENCE-IN-DEPTH UI');
    expect(html).toContain('Operator-approved');
    expect(html).toContain('configured-but-not-exercised');
    expect(html).toContain('deployment AND per-site setting');
  });

  it('keeps the workspace action-free of ungated mutations (reads and documented CLI commands only)', () => {
    const html = renderToString(<SimulationLabView state={{ status: 'ready', data: data(), readAt: 'x' }} />);
    // The only "actions" on this workspace are tenant-scoped reads and the
    // documented CLI commands — no button issues anything, and the
    // Simulation-mode gate statement is present.
    expect(html).toContain('ONLY against the simulator');
  });
});
