// P5.07 — Market & Programs workspace tests (render + gap-statement
// assertions on a mocked fetch).
//
// UNRUN — written, never executed (the Phase 5 loop bans: no npm install,
// no vitest run; decision record 07_31_market_plugins_admin_alarms.md).
// Rendering uses react-dom/server's renderToString (existing dependency,
// no DOM environment); the loader is tested at the pure-loader level with
// a fake ApiClient, pinning the §15.2 wire paths and the no-tenant-selector
// rule (§16.2).
import { describe, expect, it } from 'vitest';
import { renderToString } from 'react-dom/server';
import type { ApiClient } from '../api/client';
import type { PortfolioDto } from '../api/types';
import type { PortfolioViewDto } from '../api/views';
import {
  COMMITMENTS_STATEMENT,
  DELIVERY_EVIDENCE_STATEMENT,
  MARKET_ADAPTER_STATEMENT,
  MARKET_REGION_STATEMENT,
  PROGRAM_SIGNALS_STATEMENT,
  marketRegionRows,
  syntheticObjectiveSummary,
} from './marketData';
import { MarketProgramsView, loadMarketPrograms, type MarketProgramsData } from './MarketPrograms';

const PORTFOLIO: PortfolioDto = {
  id: 'p-1',
  tenant_id: 't',
  name: 'Portfolio One (synthetic)',
  operator: 'operator-a',
  market_region: 'SYNTHETIC_REGION_A',
  aggregation_rules: 'fixture aggregation rules',
  created_at: '2026-11-11T09:00:00Z',
};

const VIEW: PortfolioViewDto = {
  id: 'p-1',
  tenant_id: 't',
  name: 'Portfolio One (synthetic)',
  operator: 'operator-a',
  market_region: 'SYNTHETIC_REGION_A',
  aggregation_rules: 'fixture aggregation rules',
  composition: { site_count: 1, asset_count: 2 },
  commitments: {
    populated: false,
    note: 'commitments are a later-phase feature; Phase 2 stores no commitments',
  },
  created_at: '2026-11-11T09:00:00Z',
};

function data(overrides: Partial<MarketProgramsData> = {}): MarketProgramsData {
  return {
    rows: marketRegionRows(
      [PORTFOLIO],
      [VIEW as PortfolioViewDto],
      [null],
    ),
    ...overrides,
  };
}

describe('the market honesty statements (the gap inventory)', () => {
  it('states every absent market element as an explicit gap, never as data', () => {
    // No market adapter (the ledger's required wording):
    expect(MARKET_ADAPTER_STATEMENT).toContain('No market adapter is configured');
    expect(MARKET_ADAPTER_STATEMENT).toContain('none exists in the 0.x simulation');
    // No program signal (the §15.3 event has no producer):
    expect(PROGRAM_SIGNALS_STATEMENT).toContain('No program signal exists');
    expect(PROGRAM_SIGNALS_STATEMENT).toContain('MarketSignalReceived');
    expect(PROGRAM_SIGNALS_STATEMENT).toContain('D1/D2 OPEN');
    // Commitments stay the PLACEHOLDER (never upgraded):
    expect(COMMITMENTS_STATEMENT).toContain('PLACEHOLDER');
    // No market delivery evidence exists:
    expect(DELIVERY_EVIDENCE_STATEMENT).toContain('no market delivery to evidence');
    // The market region is the synthetic registry field (D1 OPEN):
    expect(MARKET_REGION_STATEMENT).toContain('synthetic');
    expect(MARKET_REGION_STATEMENT).toContain('D1, OPEN');
  });

  it('summarizes the single declared synthetic objective honestly', () => {
    const summary = syntheticObjectiveSummary();
    expect(summary.name).toBe('SYNTHETIC_ENERGY_COST_DEVIATION_V1');
    expect(summary.provenance).toBe('SYNTHETIC');
    expect(summary.termNames).toEqual(['energy', 'degradation', 'deviation', 'curtailment', 'service']);
    expect(summary.notAMarketProduct).toContain('not a market product');
    expect(summary.notAMarketProduct).toContain('D1/D2 OPEN');
  });

  it('keeps the rows aligned with their view errors (never dropped)', () => {
    const rows = marketRegionRows([PORTFOLIO], [null], ['forbidden: denied']);
    expect(rows[0]?.view).toBeNull();
    expect(rows[0]?.viewError).toBe('forbidden: denied');
  });
});

describe('loadMarketPrograms (mocked fetch mapping)', () => {
  it('reads the tenant-scoped registry and views; a failed view stays an error', async () => {
    const calls: string[] = [];
    const client: ApiClient = {
      async get<T>(path: string): Promise<T> {
        calls.push(path);
        if (path === '/api/v1/portfolios') return { data: [PORTFOLIO] } as T;
        if (path === '/api/v1/portfolios/p-1/view') return { data: VIEW } as T;
        throw new Error(`no fixture for ${path}`);
      },
      async request<T>(path: string): Promise<T> {
        return this.get<T>(path);
      },
    };
    const result = await loadMarketPrograms(client);
    expect(result.rows[0]?.view?.commitments.populated).toBe(false);
    expect(calls).toContain('/api/v1/portfolios');
    expect(calls).toContain('/api/v1/portfolios/p-1/view');
    for (const call of calls) {
      expect(call).not.toContain('tenant');
    }
  });

  it('keeps a failed view read as a per-row error (never an empty dataset)', async () => {
    const client: ApiClient = {
      async get<T>(path: string): Promise<T> {
        if (path === '/api/v1/portfolios') return { data: [PORTFOLIO] } as T;
        throw new Error('no fixture for the view');
      },
      async request<T>(path: string): Promise<T> {
        return this.get<T>(path);
      },
    };
    const result = await loadMarketPrograms(client);
    expect(result.rows[0]?.view).toBeNull();
    expect(result.rows[0]?.viewError).toContain('no fixture for the view');
  });
});

describe('MarketProgramsView (render honesty)', () => {
  it('renders the market region, the commitments PLACEHOLDER verbatim, and the synthetic objective', () => {
    const html = renderToString(
      <MarketProgramsView state={{ status: 'ready', data: data(), readAt: '2026-11-11T10:00:00Z' }} />,
    );
    expect(html).toContain('SYNTHETIC_REGION_A');
    expect(html).toContain('commitments are a later-phase feature; Phase 2 stores no commitments');
    expect(html).toContain('SYNTHETIC_ENERGY_COST_DEVIATION_V1');
    expect(html).toContain('not a market product');
    expect(html).toContain('read at 2026-11-11T10:00:00Z');
  });

  it('renders all four gap states as explicit empty statements', () => {
    const html = renderToString(
      <MarketProgramsView state={{ status: 'ready', data: data(), readAt: 'x' }} />,
    );
    expect(html).toContain('program-signals-gap');
    expect(html).toContain('market-adapter-gap');
    expect(html).toContain('delivery-evidence-gap');
    expect(html).toContain('No market adapter is configured');
    expect(html).toContain('No program signal exists');
  });

  it('renders the honest loading and error states', () => {
    expect(renderToString(<MarketProgramsView state={{ status: 'loading' }} />)).toContain(
      'Loading the market & programs data',
    );
    const error = renderToString(
      <MarketProgramsView state={{ status: 'error', error: 'unauthenticated: missing token' }} />,
    );
    expect(error).toContain('unauthenticated: missing token');
    expect(error).toContain('shows no data rather than');
  });
});
