// P5.05 — Schedules workspace tests (Vitest). UNRUN — never executed in the
// Phase 5 loop (no install/build/test; decision record: 05_09_workspaces_b.md).
// The human executes these via `npm test` after the first install.
//
// Coverage per the task contract: rendering (react-dom/server renderToString
// of the presentational components over fixtures), data mapping (chain
// grouping, objective extraction, comparison assessment), state vocabulary
// (§12.6 outcomes, roll triggers, revision states), and honesty labelling
// (superseded retained, incomparable label, candidate statements verbatim,
// mocked-fetch URL shapes).
import { describe, expect, it } from 'vitest';
import { renderToString } from 'react-dom/server';
import { createApiClient, type ApiClient } from '../api/client';
import { listScheduleRevisions } from '../api/schedules';
import type { OptimizationRunDto, PageEnvelope, ScheduleRevisionDto } from '../api/types';
import {
  ROLL_TRIGGERS,
  SOLVER_OUTCOMES,
} from '../components/stateVocabulary';
import {
  assessComparison,
  ComparisonPanel,
  DeclaredObjectivePanel,
  extractObjectiveFromRecord,
  groupRevisionsByChain,
  loadSchedulesData,
  RevisionChainList,
  RunDetailPanel,
} from './Schedules';

// --- Fixtures (Phase 3 wire shapes, snake_case, from dto_phase3.rs) ----------

function revisionFixture(overrides: Partial<ScheduleRevisionDto>): ScheduleRevisionDto {
  return {
    schedule_id: 'sched-chain-1',
    revision: 1,
    tenant_id: 'tenant-1',
    portfolio_id: 'portfolio-1',
    site_id: 'site-1',
    horizon_start_millis: 1767225600000,
    horizon_end_millis: 1767312000000,
    roll_trigger: 'initial_solve',
    state: 'active',
    candidate_id: 'candidate-1',
    candidate_status: 'candidate',
    candidate_status_statement: 'CANDIDATE-STATUS-STATEMENT-VERBATIM',
    validation_scope: 'asset_time_coupled',
    validation_scope_statement: 'VALIDATION-SCOPE-STATEMENT-VERBATIM',
    schedule_payload: { intervals: [] },
    ...overrides,
  };
}

function runFixture(overrides: Partial<OptimizationRunDto>): OptimizationRunDto {
  return {
    candidate_id: '01990000-0000-7000-8000-00000000000a',
    tenant_id: 'tenant',
    portfolio_id: 'portfolio',
    site_id: 'site',
    status: 'optimal',
    problem_class: 'LP',
    solver_name: 'HiGHS',
    solver_version: '1.15.1',
    provider_id: 'highspy',
    provider_version: '1.15.1',
    objective_value: 42.5,
    objective_units: 'currency-units',
    input_snapshot_id: 'snapshot-A',
    grid_model_version: 'grid-model-v1',
    grid_model_site_id: 'site',
    grid_model_site_reference: 'pandapower-synthetic-feeder-v1',
    schedule_proposed_key: 'schedule_proposed:portfolio:site:candidate',
    candidate_status: 'candidate',
    candidate_status_statement: 'CANDIDATE-STATUS-STATEMENT-VERBATIM',
    record: { status: 'optimal' },
    ...overrides,
  };
}

const CHAIN = [
  revisionFixture({
    revision: 1,
    state: 'superseded',
    roll_trigger: 'initial_solve',
    superseded_by_revision: 2,
    superseded_by_candidate_id: 'candidate-2',
  }),
  revisionFixture({
    revision: 2,
    state: 'active',
    roll_trigger: 'forecast_update',
    supersedes_revision: 1,
    supersedes_candidate_id: 'candidate-1',
  }),
];

// --- Vocabulary tests ---------------------------------------------------------

describe('closed vocabularies (display strings must match the server)', () => {
  it('the §12.6 solver-outcome vocabulary is exactly the six recorded values', () => {
    expect([...SOLVER_OUTCOMES]).toEqual([
      'optimal',
      'feasible',
      'infeasible',
      'timeout',
      'numerical_failure',
      'unsupported',
    ]);
  });

  it('the roll-trigger vocabulary is exactly the closed P3.24 set + initial_solve', () => {
    expect([...ROLL_TRIGGERS]).toEqual([
      'initial_solve',
      'horizon_advance',
      'envelope_invalidation',
      'asset_opt_out_dropout',
      'forecast_update',
      'operator_request',
    ]);
  });
});

// --- Data-mapping tests --------------------------------------------------------

describe('groupRevisionsByChain', () => {
  it('groups by schedule id and sorts revisions ascending, superseded included', () => {
    const chains = groupRevisionsByChain([...CHAIN].reverse());
    expect(chains).toHaveLength(1);
    expect(chains[0].scheduleId).toBe(CHAIN[0].schedule_id);
    expect(chains[0].revisions.map((r) => r.revision)).toEqual([1, 2]);
    expect(chains[0].revisions[0].state).toBe('superseded');
  });
});

describe('extractObjectiveFromRecord', () => {
  it('reads objective_name + config fields from the InputSnapshotRecord shape', () => {
    const extracted = extractObjectiveFromRecord({
      input_snapshot: {
        objective_name: 'SYNTHETIC_ENERGY_COST_DEVIATION_V1',
        objective_config_record: [
          ['objective.name', 'SYNTHETIC_ENERGY_COST_DEVIATION_V1'],
          ['objective.provenance', 'SYNTHETIC'],
        ],
      },
    });
    expect(extracted).not.toBeNull();
    expect(extracted?.objectiveName).toBe('SYNTHETIC_ENERGY_COST_DEVIATION_V1');
    expect(extracted?.configFields).toEqual([
      ['objective.name', 'SYNTHETIC_ENERGY_COST_DEVIATION_V1'],
      ['objective.provenance', 'SYNTHETIC'],
    ]);
  });

  it('returns null when the record carries no objective fields (no fabrication)', () => {
    expect(extractObjectiveFromRecord({ status: 'optimal' })).toBeNull();
    expect(extractObjectiveFromRecord(null)).toBeNull();
  });
});

describe('assessComparison (the P3.23 shared-input rule enforced in code)', () => {
  it('candidates on the SAME input snapshot are comparable', () => {
    const a = runFixture({ candidate_id: 'cand-A', input_snapshot_id: 'snap-1' });
    const b = runFixture({ candidate_id: 'cand-B', input_snapshot_id: 'snap-1' });
    const assessment = assessComparison([a, b], ['cand-A', 'cand-B']);
    expect(assessment.comparable).toBe(true);
    expect(assessment.sharedInputSnapshotId).toBe('snap-1');
  });

  it('candidates on DIFFERENT input snapshots are labelled INCOMPARABLE', () => {
    const a = runFixture({ candidate_id: 'cand-A', input_snapshot_id: 'snap-1' });
    const b = runFixture({ candidate_id: 'cand-B', input_snapshot_id: 'snap-2' });
    const assessment = assessComparison([a, b], ['cand-A', 'cand-B']);
    expect(assessment.comparable).toBe(false);
    expect(assessment.sharedInputSnapshotId).toBeNull();
  });

  it('missing candidates are reported, never fabricated', () => {
    const a = runFixture({ candidate_id: 'cand-A' });
    const assessment = assessComparison([a], ['cand-A', 'cand-ghost']);
    expect(assessment.rows).toHaveLength(1);
    expect(assessment.missing).toEqual(['cand-ghost']);
    expect(assessment.comparable).toBe(false);
  });
});

// --- Rendering tests (react-dom/server over fixtures) --------------------------

describe('rendering (fixture-driven)', () => {
  it('renders the revision chain with SUPERSEDED retained and the verbatim statements', () => {
    const html = renderToString(<RevisionChainList chains={groupRevisionsByChain(CHAIN)} />);
    expect(html).toContain('SUPERSEDED (retained for audit; retrievable, never deleted)');
    expect(html).toContain('CANDIDATE-STATUS-STATEMENT-VERBATIM');
    expect(html).toContain('VALIDATION-SCOPE-STATEMENT-VERBATIM');
    expect(html).toContain('supersedes revision 1');
    expect(html).toContain('superseded by revision 2');
  });

  it('renders the declared objective as SYNTHETIC_ENERGY_COST_DEVIATION_V1 / SYNTHETIC with the D1/D2 note', () => {
    const html = renderToString(<DeclaredObjectivePanel />);
    expect(html).toContain('SYNTHETIC_ENERGY_COST_DEVIATION_V1');
    expect(html).toContain('SYNTHETIC');
    expect(html).toContain('D1');
    expect(html).toContain('D2');
    expect(html).toContain('not a market product');
    expect(html).toContain('Free-text objective entry that bypasses the declared set does not');
    // all five declared terms, in §12.1 order
    for (const term of ['energy', 'degradation', 'deviation', 'curtailment', 'service']) {
      expect(html).toContain(term);
    }
  });

  it('renders run detail with solver, objective value + units, and the no-fabrication objective note', () => {
    const run = runFixture({ record: { status: 'optimal' } });
    const html = renderToString(<RunDetailPanel run={run} />);
    expect(html).toContain('HiGHS');
    expect(html).toContain('1.15.1');
    expect(html).toContain('42.5');
    expect(html).toContain('currency-units');
    expect(html).toContain('objective-not-in-record');
    expect(html).toContain('recorded fact, not a');
  });

  it('renders the INCOMPARABLE verdict for cross-input candidates', () => {
    const a = runFixture({ candidate_id: 'cand-A', input_snapshot_id: 'snap-1' });
    const b = runFixture({ candidate_id: 'cand-B', input_snapshot_id: 'snap-2' });
    const html = renderToString(<ComparisonPanel assessment={assessComparison([a, b], ['cand-A', 'cand-B'])} />);
    expect(html).toContain('INCOMPARABLE');
    expect(html).toContain('snap-1');
    expect(html).toContain('snap-2');
  });

  it('renders the COMPARABLE verdict with the shared snapshot id', () => {
    const a = runFixture({ candidate_id: 'cand-A', input_snapshot_id: 'snap-1' });
    const b = runFixture({ candidate_id: 'cand-B', input_snapshot_id: 'snap-1' });
    const html = renderToString(<ComparisonPanel assessment={assessComparison([a, b], ['cand-A', 'cand-B'])} />);
    expect(html).toContain('COMPARABLE');
    expect(html).toContain('SAME input snapshot (snap-1)');
  });
});

// --- Mocked-fetch tests ---------------------------------------------------------

describe('data loading over a mocked fetch', () => {
  it('loadSchedulesData reads both listings and records the truncation flags', async () => {
    const requests: string[] = [];
    const client: ApiClient = createApiClient({
      fetchImpl: async (input: RequestInfo | URL) => {
        const url = String(input);
        requests.push(url);
        if (url.startsWith('/api/v1/schedules')) {
          return new Response(
            JSON.stringify({ data: CHAIN, next_cursor: 'cursor-opaque' } satisfies PageEnvelope<ScheduleRevisionDto>),
            { status: 200 },
          );
        }
        return new Response(
          JSON.stringify({ data: [runFixture({})], next_cursor: null } satisfies PageEnvelope<OptimizationRunDto>),
          { status: 200 },
        );
      },
    });
    const data = await loadSchedulesData(client);
    expect(requests.some((url) => url.startsWith('/api/v1/schedules?limit=200'))).toBe(true);
    expect(requests.some((url) => url.startsWith('/api/v1/optimization-runs?limit=200'))).toBe(true);
    expect(data.schedules).toHaveLength(2);
    expect(data.runs).toHaveLength(1);
    expect(data.schedulesTruncated).toBe(true);
    expect(data.runsTruncated).toBe(false);
  });

  it('the chain filter reaches the wire as schedule_id (the full-chain filter)', async () => {
    const paths: string[] = [];
    const client: ApiClient = createApiClient({
      fetchImpl: async (input: RequestInfo | URL) => {
        paths.push(String(input));
        return new Response(JSON.stringify({ data: [] }), { status: 200 });
      },
    });
    await listScheduleRevisions(
      { schedule_id: '01990000-0000-7000-8000-000000000001', limit: 50 },
      client,
    );
    const url = new URL(paths[0], 'http://test');
    expect(url.pathname).toBe('/api/v1/schedules');
    expect(url.searchParams.get('schedule_id')).toBe('01990000-0000-7000-8000-000000000001');
    expect(url.searchParams.get('limit')).toBe('50');
  });

  it('an API error surfaces as the honest error state (no fabricated data path)', async () => {
    const client: ApiClient = createApiClient({
      fetchImpl: async () =>
        new Response(JSON.stringify({ error: { code: 'forbidden', message: 'no' } }), { status: 403 }),
    });
    await expect(loadSchedulesData(client)).rejects.toMatchObject({
      status: 403,
      code: 'forbidden',
    });
  });
});
