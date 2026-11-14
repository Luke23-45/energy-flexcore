// P5.03 — Sites workspace tests (render + data-mapping + honesty labelling
// on a mocked client).
//
// UNRUN — written, never executed (Phase 5 loop bans; decision record
// 02_04_workspaces_a.md). Renders via react-dom/server (existing
// react-dom dependency; no new dependency, no DOM environment).
import { describe, expect, it } from 'vitest';
import { renderToString } from 'react-dom/server';
import type { ApiClient } from '../api/client';
import type { FeasibilityCheckDto, SiteDto } from '../api/types';
import { SitesView, latestCheckForSite, loadSites, type SitesData } from './Sites';

const SITE: SiteDto = {
  id: 'site-1',
  portfolio_id: 'portfolio-1',
  tenant_id: 'tenant-1',
  name: 'Synthetic Site 1',
  location: 'lab (synthetic)',
  timezone: 'UTC',
  point_of_common_coupling: 'pcc:feeder-a:bus-1',
  grid_model_reference: 'grid-model:three-bus-synthetic',
  edge_identity: 'edge:sim-1',
  created_at: '2026-11-11T09:00:00Z',
};

const CHECK: FeasibilityCheckDto = {
  check_id: 'check-1',
  tenant_id: 'tenant-1',
  command_id: undefined,
  portfolio_id: 'portfolio-1',
  site_id: 'site-1',
  asset_id: 'asset-1',
  schedule_id: 'sch-1',
  schedule_revision: 1,
  model_id: 'grid-three-bus',
  model_version: 'v7',
  model_content_hash: 'fnv1a64:0123456789abcdef',
  outcome: 'PASSED',
  failing_constraint: undefined,
  reason: undefined,
  margins: '{"worst_margin_fraction":0.12}',
  coverage: undefined,
  actor: 'svc:validator',
  correlation_id: 'corr-1',
  checked_at_unix_millis: 1794391200000,
  no_execute_endpoint_statement: 'read-only',
};

function sitesData(overrides: Partial<SitesData> = {}): SitesData {
  return {
    rows: [
      {
        site: SITE,
        view: {
          id: 'site-1',
          portfolio_id: 'portfolio-1',
          tenant_id: 'tenant-1',
          name: 'Synthetic Site 1',
          location: 'lab (synthetic)',
          timezone: 'UTC',
          point_of_common_coupling: 'pcc:feeder-a:bus-1',
          grid_model_reference: 'grid-model:three-bus-synthetic',
          edge_connectivity: { status: 'simulator', edge_identity: 'edge:sim-1', live: false },
          overview: { asset_count: 3 },
          created_at: '2026-11-11T09:00:00Z',
        },
        viewError: null,
        latestCheck: CHECK,
      },
    ],
    checkCount: 1,
    ...overrides,
  };
}

describe('latestCheckForSite (data mapping)', () => {
  it('picks the latest check by completion time, per site', () => {
    const older = { ...CHECK, check_id: 'check-0', checked_at_unix_millis: 1 };
    const otherSite = { ...CHECK, check_id: 'check-2', site_id: 'site-2' };
    expect(latestCheckForSite('site-1', [older, CHECK, otherSite])?.check_id).toBe('check-1');
    expect(latestCheckForSite('site-3', [CHECK])).toBeNull();
  });
});

describe('loadSites (mocked fetch mapping)', () => {
  it('reads the tenant-scoped site registry, views, and check records', async () => {
    const calls: string[] = [];
    const client: ApiClient = {
      async get<T>(path: string): Promise<T> {
        calls.push(path);
        const responses: Record<string, unknown> = {
          '/api/v1/sites': { data: [SITE] },
          '/api/v1/sites/site-1/view': { data: sitesData().rows[0]?.view },
          '/api/v1/feasibility-checks': { data: [CHECK] },
        };
        const body = responses[path];
        if (body === undefined) throw new Error(`no fixture for ${path}`);
        return body as T;
      },
      async request<T>(path: string): Promise<T> {
        return this.get<T>(path);
      },
    };
    const data = await loadSites(client);
    expect(data.rows[0]?.view?.edge_connectivity.status).toBe('simulator');
    expect(data.rows[0]?.latestCheck?.model_version).toBe('v7');
    expect(calls).toContain('/api/v1/sites');
    expect(calls).toContain('/api/v1/sites/site-1/view');
    expect(calls).toContain('/api/v1/feasibility-checks');
  });
});

describe('SitesView (render honesty)', () => {
  it('renders the PCC, the verbatim edge report (live: false), and the grid-model version + hash', () => {
    const html = renderToString(
      <SitesView state={{ status: 'ready', data: sitesData(), readAt: '2026-11-11T10:00:00Z' }} />,
    );
    expect(html).toContain('pcc:feeder-a:bus-1');
    expect(html).toContain('edge-connectivity');
    // The P2.17 contract rendered verbatim — a simulated edge is never live:
    expect(html).toContain('simulator');
    expect(html).toContain('false');
    expect(html).toContain('not a field device connection');
    // Grid model version + hash from the check evidence (P4.03):
    expect(html).toContain('grid-model-binding');
    expect(html).toContain('v7');
    expect(html).toContain('fnv1a64:0123456789abcdef');
    expect(html).toContain('checked at');
    // Registry reference rendered verbatim:
    expect(html).toContain('grid-model:three-bus-synthetic');
  });

  it('renders the honest EMS and grid-model gaps when no contract/data exists', () => {
    const data = sitesData({
      rows: [{ site: SITE, view: null, viewError: null, latestCheck: null }],
    });
    const html = renderToString(
      <SitesView state={{ status: 'ready', data, readAt: '2026-11-11T10:00:00Z' }} />,
    );
    expect(html).toContain('ems-gap');
    expect(html).toContain('No Phase 1–4 contract supplies EMS state');
    expect(html).toContain('grid-model-gap');
    expect(html).toContain('it is NOT invented here');
    // The model hash must NOT be invented when no check exists:
    expect(html).not.toContain('fnv1a64:0123456789abcdef');
  });

  it('renders a failed site view as an honest error (role visibility path)', () => {
    const data = sitesData({
      rows: [{ site: SITE, view: null, viewError: 'forbidden: no matching rule', latestCheck: null }],
    });
    const html = renderToString(
      <SitesView state={{ status: 'ready', data, readAt: 'x' }} />,
    );
    expect(html).toContain('site-view-error');
    expect(html).toContain('forbidden: no matching rule');
  });

  it('renders honest loading, error, and empty states', () => {
    expect(renderToString(<SitesView state={{ status: 'loading' }} />)).toContain('Loading the sites');
    const error = renderToString(
      <SitesView state={{ status: 'error', error: 'not_found: no such resource' }} />,
    );
    expect(error).toContain('not_found: no such resource');
    const empty = renderToString(
      <SitesView state={{ status: 'ready', data: { rows: [], checkCount: 0 }, readAt: 'x' }} />,
    );
    expect(empty).toContain('returned no sites');
  });
});
