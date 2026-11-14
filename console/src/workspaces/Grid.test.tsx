// P5.04 — Grid workspace tests (render + data-mapping + honesty labelling
// on a mocked client).
//
// UNRUN — written, never executed (Phase 5 loop bans; decision record
// 02_04_workspaces_a.md). Renders via react-dom/server (existing
// react-dom dependency; no new dependency, no DOM environment).
//
// The two ledger fixture cases are covered:
// 1. a PASSED record with a BINDING thermal constraint (LIMIT_VIOLATED
//    with a named transformer/branch constraint) — the failing constraint
//    and margins render, never as a pass;
// 2. a PASSED record with a coverage limitation — qualified
//    coverage-LIMITED with the coverage statement verbatim; uncovered
//    parts are labelled uncovered, never shown as passing.
import { describe, expect, it } from 'vitest';
import { renderToString } from 'react-dom/server';
import type { ApiClient } from '../api/client';
import type { FeasibilityCheckDto } from '../api/types';
import { GridView, loadGrid } from './Grid';
import {
  countOutcomes,
  groupBySite,
  outcomeLabel,
  toGridCheckRow,
  type GridCheckRow,
} from './gridData';

function check(overrides: Partial<FeasibilityCheckDto> = {}): FeasibilityCheckDto {
  return {
    check_id: 'check-1',
    tenant_id: 'tenant-1',
    command_id: 'cmd-1',
    portfolio_id: 'portfolio-1',
    site_id: 'site-1',
    asset_id: 'asset-1',
    schedule_id: 'sch-1',
    schedule_revision: 4,
    model_id: 'grid-three-bus',
    model_version: 'v7',
    model_content_hash: 'fnv1a64:0123456789abcdef',
    outcome: 'PASSED',
    failing_constraint: undefined,
    reason: undefined,
    margins: '{"worst_margin_fraction":0.12,"binding_constraint":"branch bus-1 bus-2"}',
    coverage: undefined,
    actor: 'svc:validator',
    correlation_id: 'corr-1',
    checked_at_unix_millis: 1794391200000,
    no_execute_endpoint_statement: 'read-only',
    ...overrides,
  };
}

function toRow(check: FeasibilityCheckDto): GridCheckRow {
  return toGridCheckRow(check);
}

describe('gridData mappers', () => {
  it('keeps absent record fields as null (never defaulted)', () => {
    const row = toRow(check({ model_version: undefined, command_id: undefined }));
    expect(row.model_version).toBeNull();
    expect(row.command_id).toBeNull();
  });

  it('qualifies a PASSED outcome that carries a coverage statement', () => {
    expect(
      outcomeLabel({ outcome: 'PASSED', coverage: 'feeder-a covered; bus-3 UNCOVERED' }),
    ).toContain('coverage-limited');
    expect(outcomeLabel({ outcome: 'PASSED', coverage: undefined })).toBe('PASSED');
    expect(outcomeLabel({ outcome: 'LIMIT_VIOLATED', coverage: 'partial' })).toBe('LIMIT_VIOLATED');
  });

  it('groups by site and counts outcomes, never reading unknown codes as passes', () => {
    const rows = [
      toRow(check({ check_id: 'a', site_id: 'site-1', outcome: 'PASSED' })),
      toRow(check({ check_id: 'b', site_id: 'site-1', outcome: 'LIMIT_VIOLATED' })),
      toRow(check({ check_id: 'c', site_id: 'site-2', outcome: 'FAIL_CLOSED' })),
      toRow(check({ check_id: 'd', site_id: 'site-2', outcome: 'SOMETHING_ELSE' })),
    ];
    expect(groupBySite(rows).map((group) => group.site_id)).toEqual(['site-1', 'site-2']);
    expect(groupBySite(rows)[0]?.rows).toHaveLength(2);
    const counts = countOutcomes(rows);
    expect(counts).toEqual({ PASSED: 1, LIMIT_VIOLATED: 1, FAIL_CLOSED: 1, other: 1 });
  });
});

describe('loadGrid (mocked fetch mapping)', () => {
  it('reads the tenant-scoped feasibility-check listing', async () => {
    const calls: string[] = [];
    const client: ApiClient = {
      async get<T>(path: string): Promise<T> {
        calls.push(path);
        if (path === '/api/v1/feasibility-checks') return { data: [check()] } as T;
        throw new Error(`no fixture for ${path}`);
      },
      async request<T>(path: string): Promise<T> {
        return this.get<T>(path);
      },
    };
    const data = await loadGrid(client);
    expect(data.rows).toHaveLength(1);
    expect(data.rows[0]?.model_content_hash).toBe('fnv1a64:0123456789abcdef');
    expect(calls).toEqual(['/api/v1/feasibility-checks']);
  });
});

describe('GridView (render honesty)', () => {
  it('fixture 1: a transformer-limit violation shows the failing constraint and margins — never a pass', () => {
    const data = {
      rows: [
        toRow(
          check({
            outcome: 'LIMIT_VIOLATED',
            failing_constraint: 'transformer T1 loading 104% > operational limit 100%',
            margins: '{"worst_margin_fraction":-0.04,"violated":"transformer T1"}',
            coverage: undefined,
          }),
        ),
      ],
      consultedCount: 1,
    };
    const html = renderToString(
      <GridView state={{ status: 'ready', data, readAt: '2026-11-11T10:00:00Z' }} />,
    );
    expect(html).toContain('LIMIT_VIOLATED');
    expect(html).toContain('transformer T1 loading 104% &gt; operational limit 100%');
    expect(html).toContain('margins-block');
    expect(html).toContain('&quot;worst_margin_fraction&quot;:-0.04');
    expect(html).toContain('the candidate exceeded a declared network limit');
  });

  it('fixture 2: a PASSED record with a coverage limitation is qualified, uncovered labelled uncovered', () => {
    const data = {
      rows: [
        toRow(
          check({
            outcome: 'PASSED',
            coverage: 'feeder-a (bus-1..bus-2) covered; bus-3 and T2 are UNCOVERED',
          }),
        ),
      ],
      consultedCount: 1,
    };
    const html = renderToString(
      <GridView state={{ status: 'ready', data, readAt: 'x' }} />,
    );
    expect(html).toContain('coverage-limited (uncovered parts are NOT validated)');
    expect(html).toContain('coverage-block');
    expect(html).toContain('feeder-a (bus-1..bus-2) covered; bus-3 and T2 are UNCOVERED');
    expect(html).toContain('UNCOVERED');
    expect(html).toContain('never shown as passing');
  });

  it('model version + hash + timestamp render on EVERY outcome (§20.2), or explicit absence', () => {
    const data = {
      rows: [
        toRow(check()),
        toRow(check({ check_id: 'check-2', model_id: undefined, model_version: undefined, model_content_hash: undefined })),
      ],
      consultedCount: 2,
    };
    const html = renderToString(
      <GridView state={{ status: 'ready', data, readAt: 'x' }} />,
    );
    expect(html).toContain('v7');
    expect(html).toContain('fnv1a64:0123456789abcdef');
    expect(html).toContain('checked at 2026-11-11T10:00:00.000Z');
    // The record without model identity states the absence explicitly:
    expect((html.match(/not present in the record/g) ?? []).length).toBeGreaterThanOrEqual(3);
  });

  it('a FAIL_CLOSED record renders the reason and the not-inferred rule', () => {
    const data = {
      rows: [toRow(check({ outcome: 'FAIL_CLOSED', reason: 'model stale: source change after last verification' }))],
      consultedCount: 1,
    };
    const html = renderToString(
      <GridView state={{ status: 'ready', data, readAt: 'x' }} />,
    );
    expect(html).toContain('fail-closed-reason');
    expect(html).toContain('model stale: source change after last verification');
    expect(html).toContain('feasibility is NOT inferred');
  });

  it('renders the boundaries and model-health gaps (no invented topology, no invented health)', () => {
    const data = { rows: [toRow(check())], consultedCount: 1 };
    const html = renderToString(
      <GridView state={{ status: 'ready', data, readAt: 'x' }} />,
    );
    expect(html).toContain('boundaries-gap');
    expect(html).toContain('never a drawn topology');
    expect(html).toContain('model-health-gap');
    expect(html).toContain('draft/active/retired');
  });

  it('renders honest empty, loading, and error states', () => {
    const empty = renderToString(
      <GridView state={{ status: 'ready', data: { rows: [], consultedCount: 0 }, readAt: 'x' }} />,
    );
    expect(empty).toContain('No feasibility-check record exists');
    expect(renderToString(<GridView state={{ status: 'loading' }} />)).toContain(
      'Loading the grid feasibility records',
    );
    const error = renderToString(
      <GridView state={{ status: 'error', error: 'forbidden: no matching rule' }} />,
    );
    expect(error).toContain('forbidden: no matching rule');
  });
});
