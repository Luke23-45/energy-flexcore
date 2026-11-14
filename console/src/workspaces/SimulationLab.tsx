// P5.22 — the Simulation Lab workspace, failure-injection visibility layer
//         (report §19.2 layer 6, §20.1 item 9 display contract, §20.2;
//         ledger P5.22). The P5.01 stub is replaced; the export name is
//         unchanged (`SimulationLabWorkspace`, the router registry).
//
// What the operator sees here (the never-a-silent-blank-screen contract):
// - every scenario run, with its scenario id + version, seed, code
//   version, and the SIMULATED label (§20.2 — the label is structural);
// - per fault of a fault-injected run: the injected column (FAULT-INJECTED
//   tag on the run), what was injected (fault id + kind + injection
//   point), what it caused (the observed effect), and the platform's
//   response with its evidence reference — an operator can state the
//   fault, the affected assets, and the response from the display alone;
// - failed runs render their failure journal (per-command and per-grid-
//   check rows) — a failure is never a silent blank screen;
// - the alarms panel (the P5.31 derived-on-read model) surfaces
//   DISPATCH_FAILURE / DISPATCH_FALLBACK etc. where the run record's
//   responses map onto the closed alarm vocabulary — and says where no
//   alarm code exists (none invented);
// - the "clear injections and re-run clean" affordance states the exact
//   CLI command and the honest gap (no console trigger API yet) — a
//   documented action, never a fake button.
//
// The run-record read API does not exist yet in this phase: the read fails
// honestly and the gap is rendered (RUN_RECORD_API_GAP) — the console
// never fabricates runs.
//
// P5.08 — the workspace is extended additively to the FULL §20.1 item 9
// contract: the scenario library (lib-01…08, the recorded mirror), the
// replay section (recorded-inputs rule + comparison display component +
// the serving gap), the benchmark template status (EMPTY cells, UNRUN,
// honest), and the result comparison (the §12.6 records through the
// Schedules workspace's P3.23 comparability rule, imported — one rule, no
// copy). The P5.22 fault-visibility layer above is unchanged.
import type { JSX } from 'react';
import { AlarmsPanel } from '../components/alarmsPanel';
import {
  ErrorBlock,
  HonestEmpty,
  LoadingLine,
  RoleScopeLine,
  errorText,
  useWorkspaceData,
  type LoadState,
} from '../components/workspaceData';
import { api, type ApiClient } from '../api/client';
import { listAlarms, type AlarmPageEnvelope } from '../api/alarms';
import { listScenarioRuns, type ScenarioRunRecordSummaryDto } from '../api/scenarioRuns';
import { listOptimizationRunRecords } from '../api/schedules';
import type { OptimizationRunDto } from '../api/types';
import { FreshnessLine } from '../components/ValueType';
import { ControlAuthorityModeIndicator, SimulatedBadge } from '../components/honesty';
import { ModeGateNotice } from '../components/modeGateNotice';
import { assessComparison, type ComparisonAssessment } from './Schedules';
import {
  BENCHMARK_BINDING_RULES_NOTE,
  BENCHMARK_COUNT_COLUMNS_NOTE,
  BENCHMARK_NOT_MEASURED,
  BENCHMARK_TABLE_CASES,
  BENCHMARK_TEMPLATE_STATUS,
  BENCHMARK_TIMING_LEGS,
  BENCHMARK_WORKER_LEG_NOTE,
  CLEAN_RERUN_STATEMENT,
  COMPARISON_REUSE_NOTE,
  COMPARISON_SCOPE_NOTE,
  FAULT_ALARM_NOTE,
  NO_FAULT_JOURNAL_STATEMENT,
  REPLAY_COMPARISON_KINDS,
  REPLAY_LEG_VERDICTS,
  REPLAY_RECORDED_INPUTS_STATEMENT,
  REPLAY_SERVING_GAP,
  REPLAY_VERDICTS,
  RUN_RECORD_API_GAP,
  SCENARIO_LIBRARY_HONESTY_NOTE,
  SCENARIO_LIBRARY_MIRROR,
  SCENARIO_LIBRARY_MIRROR_NOTE,
  SIMULATED_RESULTS_STATEMENT,
  cleanReRunCommand,
  failureRows,
  faultAlarmMapping,
  isFaultInjected,
  replayRunCommand,
  replayVerdictMarker,
  runExtras,
  type ReplayComparisonRecordDto,
} from './simulationLabData';

/** What one loaded Simulation Lab read carries (errors kept per read). */
export interface SimulationLabData {
  runs: ScenarioRunRecordSummaryDto[] | null;
  runsError: string | null;
  alarms: AlarmPageEnvelope | null;
  alarmsError: string | null;
  /** The §12.6 optimization-run records for the result comparison (P5.08). */
  comparison: OptimizationRunDto[] | null;
  comparisonError: string | null;
}

/**
 * The tenant-scoped read: the scenario-run records (the display contract
 * path), the derived-on-read alarms (the P5.31 model), and the §12.6
 * optimization-run records (the result-comparison input, P5.08). A failed
 * read is kept as its own verbatim error — one failing surface never
 * blanks the other, and no error text is invented.
 */
export async function loadSimulationLab(client: ApiClient = api): Promise<SimulationLabData> {
  const runs = await listScenarioRuns(client).then(
    (envelope) => envelope.data,
    (error: unknown) => errorText(error),
  );
  const alarms = await listAlarms(client).then(
    (envelope) => envelope,
    (error: unknown) => errorText(error),
  );
  const comparison = await listOptimizationRunRecords({ limit: 200 }, client).then(
    (envelope) => envelope.data,
    (error: unknown) => errorText(error),
  );
  return {
    runs: typeof runs === 'string' ? null : runs,
    runsError: typeof runs === 'string' ? runs : null,
    alarms: typeof alarms === 'string' ? null : alarms,
    alarmsError: typeof alarms === 'string' ? alarms : null,
    comparison: typeof comparison === 'string' ? null : comparison,
    comparisonError: typeof comparison === 'string' ? comparison : null,
  };
}

/** The workspace shell the router renders (the export name is unchanged
 * from the P5.01 stub). */
export function SimulationLabWorkspace(): JSX.Element {
  const state = useWorkspaceData(loadSimulationLab);
  return <SimulationLabView state={state} />;
}

export function SimulationLabView({ state }: { state: LoadState<SimulationLabData> }): JSX.Element {
  return (
    <section aria-label="Simulation Lab">
      <h2>Simulation Lab</h2>
      {/* P5.13 step 1 — the persistent §21.4 control-authority-mode indicator
          (one component, shared by every workspace; see honesty.tsx), with
          the P5.27 mode-gate notice beside it (the enforcement layer: the
          mode AND the capability gate every mutating console action). */}
      <ControlAuthorityModeIndicator />
      <ModeGateNotice />
      <p className="honesty-banner" role="note" data-testid="simlab-simulated-banner">
        {SIMULATED_RESULTS_STATEMENT}
      </p>
      <RoleScopeLine />
      {state.status === 'loading' && <LoadingLine what="the scenario-run records and alarms" />}
      {state.status === 'error' && <ErrorBlock what="The Simulation Lab reads" error={state.error} />}
      {state.status === 'ready' && <SimulationLabReady data={state.data} readAt={state.readAt} />}
    </section>
  );
}

function SimulationLabReady({ data, readAt }: { data: SimulationLabData; readAt: string }): JSX.Element {
  return (
    <>
      <h3>Alarms (the P5.31 derived-on-read model)</h3>
      <AlarmsPanel alarms={data.alarms} alarmsError={data.alarmsError} readAt={readAt} />

      <h3>Scenario runs</h3>
      {/* P5.10: the run-record section's freshness anchor (§20.2) — the read
          that produced this list; each run block shows its own recorded
          scenario/seed/code-version identity. */}
      <p>
        <FreshnessLine readAt={readAt} />
      </p>
      {data.runsError !== null && (
        <div role="note" data-testid="runs-read-gap">
          <p>
            <strong>The scenario-run records could not be read.</strong>
          </p>
          <p>{RUN_RECORD_API_GAP}</p>
          <p>
            Read error (verbatim): <code>{data.runsError}</code>
          </p>
        </div>
      )}
      {data.runsError === null && data.runs !== null && data.runs.length === 0 && (
        <HonestEmpty
          what="Scenario runs"
          reason="The run-record read returned no records. No run is fabricated; run a scenario with the sim-runner CLI and serve the record through the run-history surface (P5.08/P5.30) to see it here."
          testId="runs-empty"
        />
      )}
      {data.runs !== null &&
        data.runs.map((run) => <RunBlock key={`${run.scenario_id}:${run.scenario_version}:${run.master_seed}`} run={run} />)}

      <ScenarioLibrarySection />

      <ReplaySection />

      <BenchmarkSection />

      <ResultComparisonSection
        comparison={data.comparison}
        comparisonError={data.comparisonError}
      />
    </>
  );
}

// --- P5.08 — scenarios (the §19.3 library index) ------------------------------

/** The scenario library: the recorded lib-01…08 mirror (pinned inputs,
 * versions, seeds, declared runtime classes) + the §19.3 honesty notes.
 * A mirror, clearly labelled — no API serves the library in this phase. */
function ScenarioLibrarySection(): JSX.Element {
  return (
    <div data-testid="scenario-library">
      <h3>Scenario library (the §19.3 benchmark cases, P5.20)</h3>
      <p>{SCENARIO_LIBRARY_MIRROR_NOTE}</p>
      <table>
        <caption>
          The library records with their pinned scenario id + version and
          master seed (the pins replay verifies fail-loud — P5.18).
        </caption>
        <thead>
          <tr>
            <th scope="col">Record (id : version)</th>
            <th scope="col">Purpose</th>
            <th scope="col">Scale</th>
            <th scope="col">Runtime class (DECLARED, never measured)</th>
            <th scope="col">Backend</th>
            <th scope="col">Pinned scenario (id : version)</th>
            <th scope="col">Master seed</th>
            <th scope="col">Grid fixture</th>
            <th scope="col">Objective (declared set)</th>
          </tr>
        </thead>
        <tbody>
          {SCENARIO_LIBRARY_MIRROR.map((entry) => (
            <tr key={entry.recordId} data-testid={`library-${entry.recordId}`}>
              <td>
                <code>{entry.recordId}</code>:{entry.recordVersion}
              </td>
              <td>{entry.purpose}</td>
              <td>{entry.scaleClass}</td>
              <td>{entry.runtimeClass}</td>
              <td>
                <code>{entry.backend}</code>
                {entry.backendNote ? (
                  <>
                    <br />
                    <small>{entry.backendNote}</small>
                  </>
                ) : null}
              </td>
              <td>
                <code>{entry.scenarioPin}</code>
              </td>
              <td>
                <code>{entry.masterSeed}</code>
              </td>
              <td>
                <code>{entry.gridFixture}</code>
              </td>
              <td>
                <code>{entry.objective}</code>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <p role="note">{SCENARIO_LIBRARY_HONESTY_NOTE}</p>
    </div>
  );
}

// --- P5.08 — replay (P5.18–P5.19) ---------------------------------------------

/** The replay section: the recorded-inputs rule, the per-record replay
 * commands, the comparison display component, and the honest serving gap. */
function ReplaySection(): JSX.Element {
  return (
    <div data-testid="replay-section">
      <h3>Replay (re-run a scenario from its recorded inputs, P5.18–P5.19)</h3>
      <p role="note">{REPLAY_RECORDED_INPUTS_STATEMENT}</p>
      <p>
        The replay command per library record (double-run = the P5.19 proof
        shape; add <code>--baseline &lt;run-record json&gt;</code> to compare
        against a stored <code>scenario-run --out</code> artefact):
      </p>
      <ul>
        {SCENARIO_LIBRARY_MIRROR.map((entry) => (
          <li key={entry.recordId} data-testid={`replay-command-${entry.recordId}`}>
            <code>{replayRunCommand(`${entry.recordId}.json`)}</code>
          </li>
        ))}
      </ul>
      <p role="note">{REPLAY_SERVING_GAP}</p>
      {/* The comparison display component is exported for the serving
          surface (ReplayComparisonView); no comparison record is rendered
          here because none is served — never a fabricated verdict. */}
    </div>
  );
}

/**
 * The ReplayComparisonRecord display (exported for the future serving
 * surface and the tests): the input-hash match, the per-leg comparisons
 * with their named diffs, and the reproducibility verdict — rendered
 * verbatim from the record, never paraphrased, never re-interpreted.
 */
export function ReplayComparisonView({
  record,
}: {
  record: ReplayComparisonRecordDto;
}): JSX.Element {
  const knownKind = (REPLAY_COMPARISON_KINDS as readonly string[]).includes(record.comparison_kind);
  const knownVerdict = (REPLAY_VERDICTS as readonly string[]).includes(record.verdict);
  return (
    <div data-testid="replay-comparison" className="simlab-replay-comparison">
      <h4>
        Replay comparison <code>{record.record_id}:{record.record_version}</code> (
        {knownKind ? (
          record.comparison_kind
        ) : (
          <>{record.comparison_kind} [outside the closed comparison-kind vocabulary — displayed verbatim]</>
        )}
        ) — scenario <code>{record.scenario_id}:{record.scenario_version}</code>, seed{' '}
        <code>{record.master_seed}</code>, code version <code>{record.code_version}</code>
      </h4>
      <p data-testid="replay-input-hash">
        Input hash <code>{record.input_hash}</code> —{' '}
        {record.input_hash_match
          ? 'MATCHES the recorded hash'
          : 'DOES NOT MATCH the recorded hash (a tamper finding: the record\'s pinned inputs changed after pinning — never a silent re-hash)'}
      </p>
      <p data-testid="replay-verdict">
        <strong>{replayVerdictMarker(record.verdict)}</strong>{' '}
        {knownVerdict ? null : '(outside the closed verdict vocabulary)'} — {record.verdict_statement}
      </p>
      <p>
        Tolerance rule (pre-registered in the record, verbatim):{' '}
        <code data-testid="replay-tolerance-rule">{record.tolerance_rule}</code>
      </p>
      <table>
        <caption>The per-leg comparisons (every named diff is listed, never truncated away).</caption>
        <thead>
          <tr>
            <th scope="col">Leg</th>
            <th scope="col">Verdict</th>
            <th scope="col">Diff count</th>
            <th scope="col">Named diffs</th>
            <th scope="col">Detail (what was compared, at what rule)</th>
          </tr>
        </thead>
        <tbody>
          {record.legs.map((leg) => {
            const knownLegVerdict = (REPLAY_LEG_VERDICTS as readonly string[]).includes(leg.verdict);
            return (
              <tr key={leg.leg} data-testid={`replay-leg-${leg.leg}`}>
                <td>
                  <code>{leg.leg}</code>
                </td>
                <td>
                  {leg.verdict}
                  {knownLegVerdict ? null : ' [outside the closed leg-verdict vocabulary — displayed verbatim]'}
                </td>
                <td>{leg.diff_count}</td>
                <td>
                  {leg.diffs.length === 0 ? (
                    '(none named)'
                  ) : (
                    <ul>
                      {leg.diffs.map((diff, index) => (
                        <li key={index}>
                          <code>{diff}</code>
                        </li>
                      ))}
                    </ul>
                  )}
                </td>
                <td>{leg.detail}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

// --- P5.08 — benchmark runs (the P5.23 template status) -----------------------

/** The benchmark section: the pre-registered template's status rendered
 * honestly — EMPTY measured cells, the not_measured reasons verbatim, and
 * the binding rules. No figure exists; none is claimed; none invented. */
function BenchmarkSection(): JSX.Element {
  return (
    <div data-testid="benchmark-section">
      <h3>Benchmark runs (the §19.4 metrics, P5.23 — template status)</h3>
      <p role="note">{BENCHMARK_TEMPLATE_STATUS}</p>
      <table data-testid="benchmark-table-a">
        <caption>
          Table A (replay-pipeline runtime, the P3.27 method) — the ×1 / ×2 /
          ×4 runtime and record-count cells are ALL EMPTY (fill from the
          produced records only, after the human's recorded run).
        </caption>
        <thead>
          <tr>
            <th scope="col">Case (record id)</th>
            <th scope="col">×1 runtime [s]</th>
            <th scope="col">×2 runtime [s]</th>
            <th scope="col">×4 runtime [s]</th>
            <th scope="col">Replay record count (×1)</th>
          </tr>
        </thead>
        <tbody>
          {BENCHMARK_TABLE_CASES.map((entry) => (
            <tr key={entry.recordId} data-testid={`benchmark-row-${entry.recordId}`}>
              <td>
                <code>{entry.recordId}</code>
              </td>
              <td data-testid="benchmark-cell-empty">(EMPTY — honest gap, never a zero)</td>
              <td data-testid="benchmark-cell-empty">(EMPTY — honest gap, never a zero)</td>
              <td data-testid="benchmark-cell-empty">(EMPTY — honest gap, never a zero)</td>
              <td data-testid="benchmark-cell-empty">(EMPTY — honest gap, never a zero)</td>
            </tr>
          ))}
        </tbody>
      </table>
      <p role="note">{BENCHMARK_WORKER_LEG_NOTE}</p>
      <table data-testid="benchmark-table-b">
        <caption>
          Table B (the per-leg composition wall clock, the P5.23 timings
          surface) — ALL cells EMPTY. A leg absent from a record's timings
          block stays EMPTY with its reason: an untimed leg is never a zero.
          The scripted-backend rows time composition work over a recorded
          fixture, NOT a power-flow solve.
        </caption>
        <thead>
          <tr>
            <th scope="col">Case (record id)</th>
            {BENCHMARK_TIMING_LEGS.map((leg) => (
              <th scope="col" key={leg}>
                <code>{leg}</code> [s]
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {BENCHMARK_TABLE_CASES.map((entry) => (
            <tr key={entry.recordId}>
              <td>
                <code>{entry.recordId}</code>
              </td>
              {BENCHMARK_TIMING_LEGS.map((leg) => (
                <td key={leg} data-testid="benchmark-cell-empty">
                  (EMPTY — honest gap, never a zero)
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      <div data-testid="benchmark-not-measured">
        <p>
          <strong>Table C (per-case §19.4 outcome metrics) — the columns with
          no measurement surface, recorded not_measured with their reasons:</strong>
        </p>
        <ul>
          {BENCHMARK_NOT_MEASURED.map((entry) => (
            <li key={entry.area}>
              {entry.area}: <code>{entry.reason}</code>
            </li>
          ))}
        </ul>
        <p role="note">{BENCHMARK_COUNT_COLUMNS_NOTE}</p>
      </div>
      <p role="note">{BENCHMARK_BINDING_RULES_NOTE}</p>
    </div>
  );
}

// --- P5.08 — result comparison (§12.6 outcomes; the P3.23 rule) ----------------

/** The result-comparison section: the §12.6 optimization-run records the
 * tenant-scoped read returns, assessed with the Schedules workspace's
 * P3.23 comparability rule (imported — one rule, no copy). A failed read
 * renders its verbatim error; an empty read renders the honest empty. */
export function ResultComparisonSection({
  comparison,
  comparisonError,
}: {
  comparison: OptimizationRunDto[] | null;
  comparisonError: string | null;
}): JSX.Element {
  return (
    <div data-testid="result-comparison">
      <h3>Result comparison (schedule alternatives and §12.6 outcomes, P3.23)</h3>
      <p role="note">{COMPARISON_REUSE_NOTE}</p>
      <p role="note">{COMPARISON_SCOPE_NOTE}</p>
      {comparisonError !== null && (
        <div role="note" data-testid="comparison-read-gap">
          <p>
            <strong>The §12.6 optimization-run records could not be read.</strong>
          </p>
          <p>
            Read error (verbatim): <code>{comparisonError}</code>
          </p>
          <p>
            The console shows no comparison rather than a fabricated one; the
            API enforces visibility per role and tenant (P5.26).
          </p>
        </div>
      )}
      {comparisonError === null && comparison !== null && comparison.length === 0 && (
        <HonestEmpty
          what="Result comparison"
          reason="The optimization-run read returned no §12.6 records: there is no schedule alternative or Tier comparison to display, and none is fabricated. Comparisons appear only from real records."
          testId="comparison-empty"
        />
      )}
      {comparison !== null && comparison.length > 0 && (
        <ComparisonTable runs={comparison} />
      )}
    </div>
  );
}

/** The comparison table: per-candidate §19.4 metrics + the shared-input
 * verdict from the imported P3.23 rule. */
function ComparisonTable({ runs }: { runs: OptimizationRunDto[] }): JSX.Element {
  const candidateIds = runs.map((run) => run.candidate_id);
  const assessment: ComparisonAssessment = assessComparison(runs, candidateIds);
  return (
    <div data-testid="comparison-table">
      <p data-testid="comparison-verdict">
        {assessment.comparable
          ? `COMPARABLE — all compared candidates cite the SAME input snapshot (${assessment.sharedInputSnapshotId}); only the formulation/solver tier varies.`
          : 'INCOMPARABLE — the compared candidates do not share one input snapshot (or fewer than two candidates were found). A cross-input comparison is labelled incomparable and is NOT presented as equivalent (P3.23).'}{' '}
        Every compared schedule outcome here is SIMULATED / solver-produced
        planning output (the 0.x line), never a measured field result.
      </p>
      <table>
        <caption>
          The §12.6 candidate records with their objective value (with
          units), solver + version, status, and input snapshot.
        </caption>
        <thead>
          <tr>
            <th scope="col">Candidate</th>
            <th scope="col">Status</th>
            <th scope="col">Objective value</th>
            <th scope="col">Solver</th>
            <th scope="col">Input snapshot</th>
          </tr>
        </thead>
        <tbody>
          {assessment.rows.map((row) => (
            <tr key={row.candidateId} data-testid={`comparison-${row.candidateId}`}>
              <td>
                <code>{row.candidateId}</code>
              </td>
              <td>{row.status}</td>
              <td>
                {row.objectiveValue === null
                  ? '(no objective value in the record)'
                  : `${row.objectiveValue} ${row.objectiveUnits ?? '(no unit field)'}`}
              </td>
              <td>{row.solver}</td>
              <td>
                <code>{row.inputSnapshotId}</code>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {assessment.missing.length > 0 && (
        <p role="alert" data-testid="comparison-missing">
          Requested candidates not found in the read (named, never silently
          skipped): {assessment.missing.join(', ')}
        </p>
      )}
    </div>
  );
}

/** One run: the simulated label, the injected/caused/response fault
 * columns, the failure journal of a failed run, and the clean re-run
 * affordance for an injected run. */
function RunBlock({ run }: { run: ScenarioRunRecordSummaryDto }): JSX.Element {
  const injected = isFaultInjected(run);
  const failures = failureRows(run);
  return (
    <div data-testid={`run-${run.scenario_id}-${run.scenario_version}`} className="simlab-run">
      <h4>
        {injected && (
          <code data-testid="fault-injected-tag">[FAULT-INJECTED]</code>
        )}{' '}
        scenario <code>{`${run.scenario_id}:${run.scenario_version}`}</code> — seed{' '}
        <code>{run.master_seed}</code>, code version <code>{run.code_version}</code> —{' '}
        <strong>simulated</strong> ({run.records_ingested} ingested records, source{' '}
        <code>{run.simulator_source_id}</code>)
      </h4>
      {/* P5.13 — the unified SIMULATED label from the central honesty module
          (the run-level label routes through the one shared component). */}
      <SimulatedBadge detail="every result of this run is simulator output (the 0.x line, §21.4), labelled with the scenario id + version it came from — no value here is a field measurement, and acknowledgements shown downstream are application-layer acceptance, never physical delivery." />

      {injected ? (
        <FaultTable run={run} />
      ) : (
        <p data-testid="no-fault-journal">{NO_FAULT_JOURNAL_STATEMENT}</p>
      )}

      {failures.length > 0 ? (
        <div data-testid="failure-journal">
          <p>
            <strong>Failure journal of this run</strong> (a failure is never a
            silent blank screen — every failing command and non-passing grid
            check is listed with its reason):
          </p>
          <ul>
            {failures.map((row) => (
              <li key={row.label}>
                {row.label}: <code>{row.detail}</code>
              </li>
            ))}
          </ul>
        </div>
      ) : (
        <p data-testid="no-failures">
          No failing command and no non-passing grid check in this run's
          record (an honest record-derived statement — not a health claim).
        </p>
      )}

      {injected && (
        <div data-testid="clean-rerun" className="simlab-clean-rerun">
          <p>
            <strong>Clear injections and re-run clean:</strong>
          </p>
          <p>
            <code>{cleanReRunCommand(run)}</code>
          </p>
          <p>{CLEAN_RERUN_STATEMENT}</p>
        </div>
      )}

      <RunRecordConsumption run={run} />
    </div>
  );
}

/** P5.08 — the run-record consumption: the additive record fields rendered
 * when they exist, never fabricated when they do not — the record's own
 * verbatim statements, the P5.23 per-leg timings (MEASUREMENT wall clock,
 * never the scenario-clock STATE), and the P4.37 closed-loop consumption
 * decisions the run recorded. */
function RunRecordConsumption({ run }: { run: ScenarioRunRecordSummaryDto }): JSX.Element {
  const extras = runExtras(run);
  return (
    <div data-testid="run-record-consumption">
      {run.statements !== undefined && run.statements.length > 0 && (
        <div data-testid="run-statements">
          <p>
            <strong>The record's own statements (verbatim — the console never
            paraphrases them):</strong>
          </p>
          <ul>
            {run.statements.map((statement, index) => (
              <li key={index}>
                <code>{statement}</code>
              </li>
            ))}
          </ul>
        </div>
      )}
      {extras.timings !== undefined && (
        <div data-testid="run-timings">
          <p>
            <strong>Per-leg wall-clock timings (P5.23)</strong> — schema{' '}
            <code>{extras.timings.record_schema}</code>:
          </p>
          <p role="note">{extras.timings.statement}</p>
          <table>
            <caption>
              The measured legs of THIS run (an untimed leg is absent — never
              zero; the measurement clock is wall-clock, the run's STATE
              clock is the scenario clock, P0.09 — the two are never mixed).
            </caption>
            <thead>
              <tr>
                <th scope="col">Leg</th>
                <th scope="col">Duration [s]</th>
                <th scope="col">Method (verbatim)</th>
              </tr>
            </thead>
            <tbody>
              {extras.timings.legs.map((leg) => (
                <tr key={leg.leg} data-testid={`timing-${leg.leg}`}>
                  <td>
                    <code>{leg.leg}</code>
                  </td>
                  <td>{leg.duration_seconds}</td>
                  <td>
                    <small>{leg.method}</small>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {extras.consumption_decisions !== undefined && extras.consumption_decisions.length > 0 && (
        <div data-testid="run-consumption-decisions">
          <p>
            <strong>The run's P4.37 closed-loop consumption decisions</strong>{' '}
            (the frozen D-P4.07-1 routing recorded ON the run; the
            re-scheduling consumer stays the existing P3.24 trigger path —
            the boundary travels in each decision's statements):
          </p>
          <ul>
            {extras.consumption_decisions.map((decision) => (
              <li key={`${decision.command_id}:${decision.check_id}`} data-testid={`consumption-${decision.command_id}`}>
                command <code>{decision.command_id}</code> — outcome{' '}
                <code>{decision.outcome_code}</code>, P4.37 action{' '}
                <code>{decision.action}</code>
                {decision.route ? (
                  <>
                    {' '}
                    (route <code>{decision.route}</code>)
                  </>
                ) : null}{' '}
                — decision ref <code>{decision.decision_ref}</code>, check{' '}
                <code>{decision.check_id}</code>, model{' '}
                <code>{decision.model_id_version}</code>
                <br />
                reason: {decision.reason}
                {decision.violated_constraints.length > 0 && (
                  <>
                    <br />
                    violated constraints:{' '}
                    {decision.violated_constraints.join('; ')}
                  </>
                )}
                {decision.problem_inputs.length > 0 && (
                  <>
                    <br />
                    problem inputs (the re-optimization inputs):{' '}
                    {decision.problem_inputs.join('; ')}
                  </>
                )}
                <br />
                roll-trigger note: {decision.roll_trigger_note}
                {decision.statements.map((statement, index) => (
                  <span key={index}>
                    <br />
                    <small>
                      statement (verbatim): <code>{statement}</code>
                    </small>
                  </span>
                ))}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

/** The per-fault table: injected (id + kind + injection point), caused
 * (the observed effect), and the platform's response + evidence reference,
 * plus the honest alarm mapping per fault. */
function FaultTable({ run }: { run: ScenarioRunRecordSummaryDto }): JSX.Element {
  const journal = run.fault_journal;
  if (!journal || journal.entries.length === 0) {
    return (
      <p data-testid="fault-journal-missing">
        The run is tagged fault-injected but its record carries no fault
        journal — this is a record-shape defect and is stated, never
        smoothed over.
      </p>
    );
  }
  return (
    <div data-testid="fault-table">
      <table>
        <caption>
          The run's fault journal (P5.21/P5.22): {journal.entries.length} fault
          spec(s), {journal.observed_count} observed in the run,{' '}
          {journal.cross_referenced_count} cross-referenced to the Phase 4
          fault-injection suites. Statement (verbatim):{' '}
          <code data-testid="fault-journal-statement">{journal.statement}</code>
        </caption>
        <thead>
          <tr>
            <th scope="col">Injected (fault id / kind / injection point)</th>
            <th scope="col">Caused (observed effect)</th>
            <th scope="col">Platform response</th>
            <th scope="col">Evidence</th>
            <th scope="col">Alarm mapping (P5.31)</th>
          </tr>
        </thead>
        <tbody>
          {journal.entries.map((entry) => {
            const mapping = faultAlarmMapping(entry);
            return (
              <tr key={entry.fault_id}>
                <td>
                  <code>{entry.fault_id}</code> — <code>{entry.kind}</code>
                  <br />
                  {entry.injection_point}
                </td>
                <td>{entry.observed_effect}</td>
                <td>
                  <code data-testid={`fault-response-${entry.fault_id}`}>
                    {entry.platform_response}
                  </code>
                </td>
                <td>
                  <code>{entry.evidence_reference}</code>
                  <br />
                  <small>pre-registered: {entry.pre_registered_reference}</small>
                </td>
                <td data-testid={`fault-alarm-${entry.fault_id}`}>
                  {mapping.alarmCode === null ? (
                    <>{mapping.note}</>
                  ) : (
                    <>
                      <code>{mapping.alarmCode}</code> — {mapping.note}
                    </>
                  )}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
      <p>{FAULT_ALARM_NOTE}</p>
    </div>
  );
}
