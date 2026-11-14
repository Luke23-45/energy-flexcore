// P5.03 — Portfolios workspace tests (render + data-mapping + honesty
// labelling on a mocked client).
//
// UNRUN — written, never executed (Phase 5 loop bans; decision record
// 02_04_workspaces_a.md). Renders via react-dom/server (part of the
// existing react-dom dependency; no new dependency, no DOM environment).
import { describe, expect, it } from 'vitest';
import { renderToString } from 'react-dom/server';
import type { ApiClient } from '../api/client';
import type { EnvelopeRecordDto, PortfolioDto } from '../api/types';
import {
  PortfoliosView,
  envelopeComputedAt,
  latestEnvelopeFor,
  loadPortfolios,
  type PortfoliosData,
} from './Portfolios';
import { setSession, clearSession } from '../state/tenantContext';

const PORTFOLIO: PortfolioDto = {
  id: 'portfolio-1',
  tenant_id: 'tenant-1',
  name: 'Synthetic Portfolio A',
  operator: 'op:demo',
  market_region: 'NONE (D1 open)',
  aggregation_rules: 'sum of site envelopes, bounded by PCC rating',
  created_at: '2026-11-11T09:00:00Z',
};

const ENVELOPE: EnvelopeRecordDto = {
  envelope_id: 'env-1',
  tenant_id: 'tenant-1',
  portfolio_id: 'portfolio-1',
  envelope_version: 'env-v3',
  horizon_start_millis: 1794391200000,
  horizon_end_millis: 1765486400000,
  interval_millis: 900000,
  interval_count: 96,
  covered_site_ids: ['site-1'],
  envelope_payload: { computed_at: '2026-11-11T09:30:00Z' },
  event_payload_gap_note: 'gap note',
};

function portfoliosData(overrides: Partial<PortfoliosData> = {}): PortfoliosData {
  return {
    rows: [
      {
        portfolio: PORTFOLIO,
        view: {
          id: 'portfolio-1',
          tenant_id: 'tenant-1',
          name: 'Synthetic Portfolio A',
          operator: 'op:demo',
          market_region: 'NONE (D1 open)',
          aggregation_rules: 'sum of site envelopes, bounded by PCC rating',
          composition: { site_count: 2, asset_count: 5 },
          commitments: { populated: false, note: 'commitments are a later-phase field (P2.17)' },
          created_at: '2026-11-11T09:00:00Z',
        },
        viewError: null,
        envelope: ENVELOPE,
      },
    ],
    envelopeCount: 1,
    ...overrides,
  };
}

describe('pure mappers', () => {
  it('envelopeComputedAt reads the payload timestamp only when present', () => {
    expect(envelopeComputedAt({ computed_at: '2026-11-11T09:30:00Z' })).toBe('2026-11-11T09:30:00Z');
    expect(envelopeComputedAt({})).toBeNull();
    expect(envelopeComputedAt({ computed_at: 42 })).toBeNull();
    expect(envelopeComputedAt(null)).toBeNull();
  });

  it('latestEnvelopeFor picks the record with the max horizon start', () => {
    const older = { ...ENVELOPE, envelope_id: 'env-0', horizon_start_millis: 1 };
    expect(latestEnvelopeFor('portfolio-1', [older, ENVELOPE])?.envelope_id).toBe('env-1');
    expect(latestEnvelopeFor('other-portfolio', [ENVELOPE])).toBeNull();
  });
});

describe('loadPortfolios (mocked fetch mapping)', () => {
  it('reads the tenant-scoped registry and envelope resources', async () => {
    const calls: string[] = [];
    const client: ApiClient = {
      async get<T>(path: string): Promise<T> {
        calls.push(path);
        const responses: Record<string, unknown> = {
          '/api/v1/portfolios': { data: [PORTFOLIO] },
          '/api/v1/portfolios/portfolio-1/view': {
            data: portfoliosData().rows[0]?.view,
          },
          '/api/v1/flexibility/envelopes': { data: [ENVELOPE] },
        };
        const body = responses[path];
        if (body === undefined) throw new Error(`no fixture for ${path}`);
        return body as T;
      },
      async request<T>(path: string): Promise<T> {
        return this.get<T>(path);
      },
    };
    const data = await loadPortfolios(client);
    expect(data.rows[0]?.view?.composition.asset_count).toBe(5);
    expect(data.rows[0]?.envelope?.envelope_version).toBe('env-v3');
    expect(calls).toContain('/api/v1/portfolios');
    expect(calls).toContain('/api/v1/portfolios/portfolio-1/view');
    expect(calls).toContain('/api/v1/flexibility/envelopes');
    // §16.2: no tenant selector on the wire.
    for (const call of calls) expect(call).not.toContain('tenant=');
  });
});

describe('PortfoliosView (render honesty)', () => {
  it('renders composition, verbatim aggregation rules, the commitments placeholder, and the envelope version + computation time', () => {
    const html = renderToString(
      <PortfoliosView
        state={{ status: 'ready', data: portfoliosData(), readAt: '2026-11-11T10:00:00Z' }}
      />,
    );
    expect(html).toContain('Synthetic Portfolio A');
    expect(html).toContain('2 site(s), 5 asset(s)');
    expect(html).toContain('sum of site envelopes, bounded by PCC rating');
    // Commitments placeholder rendered verbatim, never upgraded:
    expect(html).toContain('PLACEHOLDER — not populated in Phase 2');
    expect(html).toContain('commitments are a later-phase field (P2.17)');
    // Envelope version + computation time (P3.12):
    expect(html).toContain('env-v3');
    expect(html).toContain('Computation time (from the envelope record payload): 2026-11-11T09:30:00Z');
    expect(html).toContain('read at 2026-11-11T10:00:00Z');
  });

  it('states the computation-time gap instead of fabricating one', () => {
    const data = portfoliosData({
      rows: [
        {
          portfolio: PORTFOLIO,
          view: null,
          viewError: null,
          envelope: { ...ENVELOPE, envelope_payload: {} },
        },
      ],
    });
    const html = renderToString(
      <PortfoliosView state={{ status: 'ready', data, readAt: '2026-11-11T10:00:00Z' }} />,
    );
    expect(html).toContain('Computation time: not present in the envelope record payload');
  });

  it('renders a failed view read as an honest per-row error (role visibility, §20.2)', () => {
    const data = portfoliosData({
      rows: [{ portfolio: PORTFOLIO, view: null, viewError: 'forbidden: no matching rule', envelope: null }],
    });
    const html = renderToString(
      <PortfoliosView state={{ status: 'ready', data, readAt: '2026-11-11T10:00:00Z' }} />,
    );
    expect(html).toContain('portfolio-view-error');
    expect(html).toContain('forbidden: no matching rule');
    expect(html).toContain('NOT shown rather than guessed');
  });

  it('shows the claims-derived role line for two roles (P5.03 step 4)', () => {
    setSession({ tenantId: 'tenant-1', subject: 'user:operator', role: 'operator' });
    const operatorHtml = renderToString(
      <PortfoliosView state={{ status: 'ready', data: portfoliosData(), readAt: 'x' }} />,
    );
    expect(operatorHtml).toContain('Role: operator');
    setSession({ tenantId: 'tenant-1', subject: 'user:viewer', role: 'viewer' });
    const viewerHtml = renderToString(
      <PortfoliosView state={{ status: 'ready', data: portfoliosData(), readAt: 'x' }} />,
    );
    expect(viewerHtml).toContain('Role: viewer');
    clearSession();
  });

  it('renders honest loading, error, and empty states', () => {
    expect(renderToString(<PortfoliosView state={{ status: 'loading' }} />)).toContain(
      'Loading the portfolios',
    );
    const error = renderToString(
      <PortfoliosView state={{ status: 'error', error: 'unauthenticated: missing token' }} />,
    );
    expect(error).toContain('unauthenticated: missing token');
    const empty = renderToString(
      <PortfoliosView state={{ status: 'ready', data: { rows: [], envelopeCount: 0 }, readAt: 'x' }} />,
    );
    expect(empty).toContain('returned no portfolios');
  });
});
