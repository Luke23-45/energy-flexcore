// P5.05 — the Schedules workspace (report §20.1 item 6; §12.1, §12.6; the
// P3.21–P3.23 records). Replaces the P5.01 stub (the stub's honest-empty
// discipline carries over: empty/error/loading are explicit; no demo data).
//
// What is shown (ledger P5.05):
// - Schedule revisions per chain: ID + revision, horizon, roll trigger
//   (closed P3.24 vocabulary), stored state (`active` | `superseded`),
//   supersedes/superseded-by links, the §12.6 candidate id, the verbatim
//   candidate-status and validation-scope statements. SUPERSEDED revisions
//   are retained and displayed — never hidden (P5.05 step 4).
// - Optimization-run (§12.6) detail: solver-outcome status (closed
//   vocabulary), problem class, solver + version, provider + version,
//   objective value with units, input-snapshot reference, grid-model
//   version + site reference — all as recorded.
// - Objective selection display: the DECLARED objective set (exactly one
//   entry, SYNTHETIC_ENERGY_COST_DEVIATION_V1, provenance SYNTHETIC —
//   D1/D2 OPEN, no market objective exists). Selection means choosing among
//   this declared set; free-text objective entry does not exist here.
// - Alternatives/scenario comparison: candidate-side metrics from the §12.6
//   records, with the P3.23 shared-input rule ENFORCED in the component —
//   candidates on different input snapshots are labelled INCOMPARABLE.
//   Honest gap: no API endpoint serves the P3.23 ScheduleComparisonRecord
//   or the Tier 1 vs Tier 2 cross-check record; comparability here is
//   derived client-side from input_snapshot_id equality only (recorded in
//   05_09_workspaces_b.md).
import { useEffect, useState } from 'react';
import type { JSX } from 'react';
import { api, type ApiClient } from '../api/client';
import { listOptimizationRunRecords, listScheduleRevisions } from '../api/schedules';
import type { OptimizationRunDto, ScheduleRevisionDto } from '../api/types';
import {
  EmptyState,
  ErrorState,
  FreshnessStamp,
  LoadingState,
  VerbatimStatement,
} from '../components/honesty';
import { RoleScopeLine } from '../components/workspaceData';
import {
  DECLARED_OBJECTIVES,
  ROLL_TRIGGERS,
  SOLVER_OUTCOMES,
  formatUtcMillis,
} from '../components/stateVocabulary';

// --- Data loading (pure, exported for tests) ---------------------------------

export interface SchedulesData {
  schedules: ScheduleRevisionDto[];
  runs: OptimizationRunDto[];
  fetchedAtIso: string;
  /** Whether each listing stopped at its first page (keyset cursor present). */
  schedulesTruncated: boolean;
  runsTruncated: boolean;
}

/** Loads the first page of schedule revisions + optimization-run records. */
export async function loadSchedulesData(client: ApiClient): Promise<SchedulesData> {
  const [schedulesPage, runsPage] = await Promise.all([
    listScheduleRevisions({ limit: 200 }, client),
    listOptimizationRunRecords({ limit: 200 }, client),
  ]);
  return {
    schedules: schedulesPage.data,
    runs: runsPage.data,
    fetchedAtIso: new Date().toISOString(),
    schedulesTruncated: schedulesPage.next_cursor !== null && schedulesPage.next_cursor !== undefined,
    runsTruncated: runsPage.next_cursor !== null && runsPage.next_cursor !== undefined,
  };
}

export interface ScheduleChain {
  scheduleId: string;
  /** Revisions ascending (revision 1 first; superseded ones included). */
  revisions: ScheduleRevisionDto[];
}

/** Groups revisions into chains, ordered ascending by revision number. */
export function groupRevisionsByChain(schedules: ScheduleRevisionDto[]): ScheduleChain[] {
  const byId = new Map<string, ScheduleRevisionDto[]>();
  for (const revision of schedules) {
    const existing = byId.get(revision.schedule_id);
    if (existing) {
      existing.push(revision);
    } else {
      byId.set(revision.schedule_id, [revision]);
    }
  }
  const chains: ScheduleChain[] = [];
  for (const [scheduleId, revisions] of byId) {
    revisions.sort((a, b) => a.revision - b.revision);
    chains.push({ scheduleId, revisions });
  }
  chains.sort((a, b) => a.scheduleId.localeCompare(b.scheduleId));
  return chains;
}

export interface ExtractedObjective {
  objectiveName: string;
  configFields: Array<[string, string]>;
}

/**
 * Defensively reads the objective configuration out of the persisted §12.6
 * record JSON (the InputSnapshotRecord carries `objective_name` and
 * `objective_config_record`). Returns null when the persisted JSON on this
 * surface does not carry them — the console then says so instead of
 * fabricating a configuration.
 */
export function extractObjectiveFromRecord(record: unknown): ExtractedObjective | null {
  if (record === null || typeof record !== 'object') return null;
  const container = record as Record<string, unknown>;
  const snapshot =
    container.input_snapshot !== null && typeof container.input_snapshot === 'object'
      ? (container.input_snapshot as Record<string, unknown>)
      : container;
  const name =
    typeof snapshot.objective_name === 'string' ? snapshot.objective_name : null;
  const rawFields = snapshot.objective_config_record;
  const configFields: Array<[string, string]> = [];
  if (Array.isArray(rawFields)) {
    for (const entry of rawFields) {
      if (Array.isArray(entry) && entry.length >= 2
        && typeof entry[0] === 'string' && typeof entry[1] === 'string') {
        configFields.push([entry[0], entry[1]]);
      }
    }
  }
  if (name === null && configFields.length === 0) return null;
  return { objectiveName: name ?? '(no objective_name field in the record JSON)', configFields };
}

export interface CandidateComparisonRow {
  candidateId: string;
  found: true;
  status: string;
  objectiveValue: number | null;
  objectiveUnits: string | null;
  solver: string;
  inputSnapshotId: string;
}

export interface ComparisonAssessment {
  rows: CandidateComparisonRow[];
  /**
   * The P3.23 shared-input verdict: comparable iff at least two candidates
   * were found AND every input snapshot reference is identical and
   * non-empty. Cross-input comparisons are INCOMPARABLE — the label is
   * enforced here, not left to the reader.
   */
  comparable: boolean;
  sharedInputSnapshotId: string | null;
  missing: string[];
}

/** Assesses candidate comparability from the §12.6 records (input snapshots). */
export function assessComparison(
  runs: OptimizationRunDto[],
  candidateIds: readonly string[],
): ComparisonAssessment {
  const byId = new Map(runs.map((run) => [run.candidate_id, run]));
  const rows: CandidateComparisonRow[] = [];
  const missing: string[] = [];
  for (const candidateId of candidateIds) {
    const run = byId.get(candidateId);
    if (!run) {
      missing.push(candidateId);
      continue;
    }
    rows.push({
      candidateId: run.candidate_id,
      found: true,
      status: run.status,
      objectiveValue: run.objective_value ?? null,
      objectiveUnits: run.objective_units ?? null,
      solver: `${run.solver_name} ${run.solver_version} (provider ${run.provider_id} ${run.provider_version})`,
      inputSnapshotId: run.input_snapshot_id,
    });
  }
  const snapshots = rows.map((row) => row.inputSnapshotId);
  const shared =
    rows.length >= 2 && snapshots.every((id) => id.length > 0 && id === snapshots[0])
      ? snapshots[0]
      : null;
  return { rows, comparable: shared !== null, sharedInputSnapshotId: shared, missing };
}

// --- Presentational components -----------------------------------------------

function RevisionRow({ revision }: { revision: ScheduleRevisionDto }): JSX.Element {
  const superseded = revision.state === 'superseded';
  const knownTrigger = (ROLL_TRIGGERS as readonly string[]).includes(revision.roll_trigger);
  return (
    <li data-testid="schedule-revision" data-state={revision.state}>
      <strong>
        {revision.schedule_id} revision {revision.revision}
      </strong>{' '}
      — state:{' '}
      {superseded
        ? 'SUPERSEDED (retained for audit; retrievable, never deleted)'
        : `active (${revision.state})`}{' '}
      | roll trigger:{' '}
      {knownTrigger
        ? revision.roll_trigger
        : `${revision.roll_trigger} [outside the closed roll-trigger vocabulary — displayed verbatim]`}{' '}
      | horizon: {formatUtcMillis(revision.horizon_start_millis)} →{' '}
      {formatUtcMillis(revision.horizon_end_millis)} (start-inclusive →
      end-exclusive) | candidate: {revision.candidate_id}
      {revision.supersedes_revision !== null && revision.supersedes_revision !== undefined ? (
        <> | supersedes revision {revision.supersedes_revision} ({revision.supersedes_candidate_id})</>
      ) : null}
      {revision.superseded_by_revision !== null && revision.superseded_by_revision !== undefined ? (
        <> | superseded by revision {revision.superseded_by_revision} ({revision.superseded_by_candidate_id})</>
      ) : null}
      <VerbatimStatement
        label="candidate status (P3.21)"
        statement={revision.candidate_status_statement}
      />
      <VerbatimStatement
        label="validation scope (ScheduleValidated caveat)"
        statement={revision.validation_scope_statement}
      />
    </li>
  );
}

export function RevisionChainList({ chains }: { chains: ScheduleChain[] }): JSX.Element {
  return (
    <div data-testid="revision-chains">
      {chains.map((chain) => (
        <section key={chain.scheduleId}>
          <h4>
            Chain {chain.scheduleId} — {chain.revisions.length} revision(s), superseded
            retained
          </h4>
          <ul>
            {chain.revisions.map((revision) => (
              <RevisionRow key={`${revision.schedule_id}:${revision.revision}`} revision={revision} />
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
}

export function DeclaredObjectivePanel(): JSX.Element {
  const declared = DECLARED_OBJECTIVES[0];
  return (
    <div data-testid="declared-objective-panel">
      <h4>Declared objective set (P3.14 — objective selection picks from THIS set)</h4>
      <p>
        The only declared objective is{' '}
        <strong data-testid="declared-objective-name">{declared.name}</strong> with
        provenance <strong data-testid="declared-objective-provenance">{declared.provenance}</strong>{' '}
        (schema version {declared.schemaVersion}). Provenance note, verbatim from the
        record fields: {declared.notAMarketProduct}.
      </p>
      <p>
        Objective selection on this console means choosing among the declared set
        above. Free-text objective entry that bypasses the declared set does not
        exist (P5.05 step 2). D1 (target jurisdiction/market) and D2 (first
        portfolio/service) are OPEN: there is no market objective to declare, and
        this synthetic objective is not a market product.
      </p>
      <table data-testid="declared-objective-terms">
        <caption>
          The declared objective's terms with their weight units and sign
          conventions, verbatim from the P3.14 record (read this table as the
          closed definition of what the objective can mean — never re-derived).
        </caption>
        <thead>
          <tr>
            <th scope="col">Term</th>
            <th scope="col">Weight unit</th>
            <th scope="col">Sign convention</th>
          </tr>
        </thead>
        <tbody>
          {declared.terms.map((term) => (
            <tr key={term.name}>
              <td>{term.name}</td>
              <td>{term.weightUnit}</td>
              <td>{term.signConvention}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export function RunDetailPanel({ run }: { run: OptimizationRunDto }): JSX.Element {
  const knownStatus = (SOLVER_OUTCOMES as readonly string[]).includes(run.status);
  const extracted = extractObjectiveFromRecord(run.record);
  return (
    <div data-testid="run-detail" data-candidate={run.candidate_id}>
      <h4>
        §12.6 record {run.candidate_id} — solver outcome:{' '}
        {knownStatus
          ? run.status
          : `${run.status} [outside the closed §12.6 vocabulary — displayed verbatim]`}
      </h4>
      <ul>
        <li>Problem class: {run.problem_class}</li>
        <li>
          Solver: {run.solver_name} {run.solver_version} (recorded fact, not a
          capability claim) | Provider: {run.provider_id} {run.provider_version}
        </li>
        <li>
          Objective value:{' '}
          {run.objective_value !== null && run.objective_value !== undefined
            ? `${run.objective_value} ${run.objective_units ?? '(units not recorded)'}`
            : 'not recorded for this outcome'}
        </li>
        <li>Input-snapshot reference: {run.input_snapshot_id}</li>
        <li>
          Grid model: version {run.grid_model_version} (site {run.grid_model_site_id},
          reference {run.grid_model_site_reference} — never detached from its site)
        </li>
        <li>ScheduleProposed idempotency key: {run.schedule_proposed_key}</li>
      </ul>
      {/* P5.11 (rejection explanations, applied across Schedules as well as
          Dispatch): a non-optimal solver outcome is the closest thing to a
          rejection on the Phase 3 surface. The §12.6 record DTO carries NO
          failing-check/evidence field, so the console names that absence
          instead of inventing a reason — the recorded
          candidate_status_statement below is the only explanation the
          surface serves, shown verbatim. */}
      {run.status !== 'optimal' && run.status !== 'feasible' ? (
        <p role="note" data-testid="solver-outcome-explanation">
          Rejection-style outcome (<strong>{run.status}</strong>): the §12.6
          record served by GET /api/v1/optimization-runs carries no
          failing-check or evidence-reference field on this surface — none is
          invented. The recorded candidate-status statement (verbatim, below)
          is the explanation this surface serves; a full rejection-evidence
          pack is the Phase 4 feasibility/rejection surface (P4.27, the
          Dispatch workspace).
        </p>
      ) : null}
      <VerbatimStatement label="candidate status (P3.21)" statement={run.candidate_status_statement} />
      <h5>Objective configuration in the persisted record</h5>
      {extracted === null ? (
        <p data-testid="objective-not-in-record">
          The persisted §12.6 record JSON on this surface does not carry the
          objective-name/config fields; the objective in effect is the declared set
          shown below — the console does not infer a configuration that the record
          does not contain.
        </p>
      ) : (
        <div data-testid="objective-in-record">
          <p>
            objective_name in record: <strong>{extracted.objectiveName}</strong>
          </p>
          {extracted.configFields.length > 0 ? (
            <ul>
              {extracted.configFields.map(([field, value]) => (
                <li key={field}>
                  {field} = {value}
                </li>
              ))}
            </ul>
          ) : null}
        </div>
      )}
      <DeclaredObjectivePanel />
    </div>
  );
}

export function ComparisonPanel({ assessment }: { assessment: ComparisonAssessment }): JSX.Element {
  return (
    <div data-testid="comparison-panel">
      <h4>Candidate comparison (P3.23 shared-input rule enforced here)</h4>
      <p data-testid="comparison-verdict">
        {assessment.comparable
          ? `COMPARABLE — all compared candidates cite the SAME input snapshot (${assessment.sharedInputSnapshotId}); only the formulation/solver tier varies.`
          : 'INCOMPARABLE — the compared candidates do not share one input snapshot (or fewer than two candidates were found). A cross-input comparison is labelled incomparable and is NOT presented as equivalent (P3.23).'}
      </p>
      <table>
        <caption>
          Per-candidate rows from the loaded §12.6 records. When the verdict
          above is INCOMPARABLE, the rows are shown side by side for
          inspection only — they are NOT presented as equivalent (P3.23).
        </caption>
        <thead>
          <tr>
            <th scope="col">Candidate</th>
            <th scope="col">§12.6 status</th>
            <th scope="col">Objective value</th>
            <th scope="col">Solver (recorded fact)</th>
            <th scope="col">Input snapshot</th>
          </tr>
        </thead>
        <tbody>
          {assessment.rows.map((row) => (
            <tr key={row.candidateId}>
              <td>{row.candidateId}</td>
              <td>{row.status}</td>
              <td>
                {row.objectiveValue !== null
                  ? `${row.objectiveValue} ${row.objectiveUnits ?? '(units not recorded)'}`
                  : 'not recorded'}
              </td>
              <td>{row.solver}</td>
              <td>{row.inputSnapshotId}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {assessment.missing.length > 0 ? (
        <p data-testid="comparison-missing">
          Candidate id(s) not present in the loaded records: {assessment.missing.join(', ')} —
          no row is fabricated for them.
        </p>
      ) : null}
      <p>
        Recorded gap: the P3.23 <em>ScheduleComparisonRecord</em> and the P3.18
        Tier 1 vs Tier 2 cross-check record have no read endpoint on this API
        surface. Comparability here is derived client-side from
        <code> input_snapshot_id</code> equality only; per-candidate §19.4 metrics
        beyond objective value/status/solver (schedule deviation, feasible interval
        count, runtime) are shown only when the persisted record JSON carries them —
        this console does not recompute them. See 05_09_workspaces_b.md.
      </p>
    </div>
  );
}

// --- The workspace container ---------------------------------------------------

interface SchedulesState {
  status: 'loading' | 'loaded' | 'error';
  data?: SchedulesData | undefined;
  error?: unknown;
  selectedCandidateId?: string | undefined;
  comparisonIds: [string, string];
}

export function SchedulesWorkspace({
  client,
}: {
  client?: ApiClient | undefined;
}): JSX.Element {
  const [state, setState] = useState<SchedulesState>({ status: 'loading', comparisonIds: ['', ''] });

  useEffect(() => {
    let cancelled = false;
    async function run(): Promise<void> {
      try {
        const data = await loadSchedulesData(client ?? api);
        if (!cancelled) setState((prev) => ({ ...prev, status: 'loaded', data }));
      } catch (error) {
        if (!cancelled) setState((prev) => ({ ...prev, status: 'error', error }));
      }
    }
    void run();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const runById = new Map(
    (state.data?.runs ?? []).map((run) => [run.candidate_id, run] as const),
  );
  const selected =
    state.selectedCandidateId !== undefined ? runById.get(state.selectedCandidateId) : undefined;

  return (
    <section aria-label="Schedules">
      <h2>Schedules</h2>
      <p>
        Day-ahead/intraday planning view over the Phase 3 records (§20.1 item 6).
        Every schedule shown is a CANDIDATE (Phase 3 never produces a feasibility,
        authorization, or dispatch verdict). Superseded revisions are retained and
        displayed (§14.2).
      </p>
      {/* P5.12 (extends the P5.03 role-line pattern to wave-2b workspaces):
          the claims-derived role line, so every workspace states the session
          role the visibility is enforced against. */}
      <RoleScopeLine />
      {state.status === 'loading' ? <LoadingState label="schedules and optimization-run records" /> : null}
      {state.status === 'error' ? <ErrorState error={state.error} /> : null}
      {state.status === 'loaded' && state.data ? (
        <>
          <FreshnessStamp fetchedAtIso={state.data.fetchedAtIso} />
          {state.data.schedulesTruncated || state.data.runsTruncated ? (
            <p role="note">
              One or both listings stopped at the first page (keyset cursor present
              server-side); this view shows that page only and invents nothing for
              the remainder.
            </p>
          ) : null}
          {state.data.schedules.length === 0 ? (
            <EmptyState
              title="No schedule revisions"
              explanation="GET /api/v1/schedules returned no records for this tenant. Phase 3 schedule revisions appear here once proposed; the console does not fabricate planning data."
            />
          ) : (
            <RevisionChainList chains={groupRevisionsByChain(state.data.schedules)} />
          )}
          <h3>Optimization-run records (§12.6) and objective display</h3>
          {state.data.runs.length === 0 ? (
            <EmptyState
              title="No optimization-run records"
              explanation="GET /api/v1/optimization-runs returned no records for this tenant. Records appear once a solve is registered; no solve result is invented here."
            />
          ) : (
            <>
              <label>
                Candidate record to inspect:{' '}
                <select
                  data-testid="candidate-select"
                  value={state.selectedCandidateId ?? ''}
                  onChange={(event) =>
                    setState((prev) => ({
                      ...prev,
                      selectedCandidateId: event.target.value === '' ? undefined : event.target.value,
                    }))
                  }
                >
                  <option value="">— select a candidate —</option>
                  {state.data.runs.map((run) => (
                    <option key={run.candidate_id} value={run.candidate_id}>
                      {run.candidate_id} ({run.status})
                    </option>
                  ))}
                </select>
              </label>
              {selected ? <RunDetailPanel run={selected} /> : null}
              <h3>Scenario comparison (P3.23)</h3>
              <label>
                Candidate A:{' '}
                <select
                  data-testid="comparison-a"
                  value={state.comparisonIds[0]}
                  onChange={(event) =>
                    setState((prev) => ({
                      ...prev,
                      comparisonIds: [event.target.value, prev.comparisonIds[1]],
                    }))
                  }
                >
                  <option value="">— none —</option>
                  {state.data.runs.map((run) => (
                    <option key={run.candidate_id} value={run.candidate_id}>
                      {run.candidate_id}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                Candidate B:{' '}
                <select
                  data-testid="comparison-b"
                  value={state.comparisonIds[1]}
                  onChange={(event) =>
                    setState((prev) => ({
                      ...prev,
                      comparisonIds: [prev.comparisonIds[0], event.target.value],
                    }))
                  }
                >
                  <option value="">— none —</option>
                  {state.data.runs.map((run) => (
                    <option key={run.candidate_id} value={run.candidate_id}>
                      {run.candidate_id}
                    </option>
                  ))}
                </select>
              </label>
              {state.comparisonIds[0] !== '' && state.comparisonIds[1] !== '' ? (
                <ComparisonPanel
                  assessment={assessComparison(state.data.runs, state.comparisonIds)}
                />
              ) : (
                <p>
                  Select two candidates to compare. Comparisons across different
                  input snapshots are labelled INCOMPARABLE by the component — the
                  rule is enforced, not left to the reader (P3.23).
                </p>
              )}
            </>
          )}
        </>
      ) : null}
    </section>
  );
}
