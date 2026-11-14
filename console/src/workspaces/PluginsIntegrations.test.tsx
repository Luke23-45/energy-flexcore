// P5.07 — Plugins & Integrations workspace tests (render + gap-statement
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
import type { SiteDto } from '../api/types';
import type { SiteViewDto } from '../api/views';
import {
  PLUGIN_AUTHORITY_STATEMENT,
  PLUGIN_KERNEL_STATEMENT,
  edgeConnectivityRows,
  edgeStatusOf,
  registeredIntegrations,
} from './pluginsData';
import {
  PluginsIntegrationsView,
  loadPluginsIntegrations,
  type PluginsIntegrationsData,
} from './PluginsIntegrations';

const SITE: SiteDto = {
  id: 'site-1',
  portfolio_id: 'p-1',
  tenant_id: 't',
  name: 'Site One (synthetic)',
  location: 'lab',
  timezone: 'UTC',
  point_of_common_coupling: 'pcc-1',
  grid_model_reference: 'grid-a',
  edge_identity: 'sim:edge-1',
  created_at: '2026-11-11T09:00:00Z',
};

const SITE_VIEW: SiteViewDto = {
  id: 'site-1',
  portfolio_id: 'p-1',
  tenant_id: 't',
  name: 'Site One (synthetic)',
  location: 'lab',
  timezone: 'UTC',
  point_of_common_coupling: 'pcc-1',
  grid_model_reference: 'grid-a',
  edge_connectivity: { status: 'simulator', edge_identity: 'sim:edge-1', live: false },
  overview: { asset_count: 2 },
  created_at: '2026-11-11T09:00:00Z',
};

function data(overrides: Partial<PluginsIntegrationsData> = {}): PluginsIntegrationsData {
  return {
    rows: edgeConnectivityRows(
      [SITE],
      [SITE_VIEW],
      [null],
    ),
    ...overrides,
  };
}

describe('the plugin honesty statements (the gap inventory)', () => {
  it('states the plugin-kernel absence explicitly (Stage 7, structural proof)', () => {
    expect(PLUGIN_KERNEL_STATEMENT).toContain('No plugin kernel exists');
    expect(PLUGIN_KERNEL_STATEMENT).toContain('Stage 7 (v0.5.0)');
    expect(PLUGIN_KERNEL_STATEMENT).toContain('29_safety_invariant_matrix.md');
    expect(PLUGIN_KERNEL_STATEMENT).toContain('no plugin execution path exists');
    expect(PLUGIN_KERNEL_STATEMENT).toContain('No plugin is shown as installed');
  });

  it('states the §5.7 authority rules without inventing plugin capabilities', () => {
    expect(PLUGIN_AUTHORITY_STATEMENT).toContain('§5.7');
    // The banned bypass list, verbatim coverage:
    expect(PLUGIN_AUTHORITY_STATEMENT).toContain('authorization');
    expect(PLUGIN_AUTHORITY_STATEMENT).toContain('tenant isolation');
    expect(PLUGIN_AUTHORITY_STATEMENT).toContain('audit');
    expect(PLUGIN_AUTHORITY_STATEMENT).toContain('command lifecycle');
    expect(PLUGIN_AUTHORITY_STATEMENT).toContain('final dispatch validation');
    // The §5.7 capability names do not exist on the action surface:
    expect(PLUGIN_AUTHORITY_STATEMENT).toContain('dispatch.authorize');
    expect(PLUGIN_AUTHORITY_STATEMENT).toContain('dispatch.issue');
    expect(PLUGIN_AUTHORITY_STATEMENT).toContain('never arbitrary Rust dylib loading');
  });

  it('lists exactly the integrations that exist, none marked as plugins', () => {
    const integrations = registeredIntegrations();
    expect(integrations).toHaveLength(2);
    expect(integrations.map((i) => i.name)).toEqual([
      'Asset simulator integration',
      'Grid simulator adapter (pandapower) integration',
    ]);
    for (const integration of integrations) {
      expect(integration.isPlugin).toBe(false);
      expect(integration.scopeStatement).toContain('not a plugin');
    }
  });

  it('reads the edge connectivity from the record verbatim, never upgraded', () => {
    const row = edgeConnectivityRows([SITE], [SITE_VIEW], [null])[0];
    const status = edgeStatusOf(row);
    expect(status.status).toBe('simulator');
    expect(status.live).toBe(false);
    expect(status.statement).toContain('live');
    expect(status.statement).toContain('D5');
    // A failed view degrades to the honest default, never to a connection:
    const failed = edgeConnectivityRows([SITE], [null], ['forbidden: denied'])[0];
    expect(edgeStatusOf(failed).status).toBe('not_connected');
    expect(edgeStatusOf(failed).live).toBe(false);
  });
});

describe('loadPluginsIntegrations (mocked fetch mapping)', () => {
  it('reads the tenant-scoped sites and views; a failed view stays an error', async () => {
    const calls: string[] = [];
    const client: ApiClient = {
      async get<T>(path: string): Promise<T> {
        calls.push(path);
        if (path === '/api/v1/sites') return { data: [SITE] } as T;
        if (path === '/api/v1/sites/site-1/view') return { data: SITE_VIEW } as T;
        throw new Error(`no fixture for ${path}`);
      },
      async request<T>(path: string): Promise<T> {
        return this.get<T>(path);
      },
    };
    const result = await loadPluginsIntegrations(client);
    expect(result.rows[0]?.view?.edge_connectivity.status).toBe('simulator');
    expect(result.rows[0]?.view?.edge_connectivity.live).toBe(false);
    expect(calls).toContain('/api/v1/sites');
    expect(calls).toContain('/api/v1/sites/site-1/view');
    for (const call of calls) {
      expect(call).not.toContain('tenant');
    }
  });
});

describe('PluginsIntegrationsView (render honesty)', () => {
  it('renders the plugin gap, the authority rules, and the two real integrations', () => {
    const html = renderToString(
      <PluginsIntegrationsView
        state={{ status: 'ready', data: data(), readAt: '2026-11-11T10:00:00Z' }}
      />,
    );
    expect(html).toContain('plugins-gap');
    expect(html).toContain('No plugin kernel exists');
    expect(html).toContain('plugin-authority-rules');
    expect(html).toContain('dispatch.authorize');
    expect(html).toContain('Asset simulator integration');
    expect(html).toContain('Grid simulator adapter (pandapower) integration');
    expect(html).toContain('no — platform-internal');
    // The per-site edge connectivity is rendered verbatim with live:false:
    expect(html).toContain('edge-connectivity-site-1');
    expect(html).toContain('<code>simulator</code>');
    expect(html).toContain('<code>false</code>');
    // No fabricated plugin row anywhere:
    expect(html).not.toContain('installed plugin:');
  });

  it('renders the honest loading and error states', () => {
    expect(
      renderToString(<PluginsIntegrationsView state={{ status: 'loading' }} />),
    ).toContain('Loading the integrations data');
    const error = renderToString(
      <PluginsIntegrationsView state={{ status: 'error', error: 'unauthenticated: missing token' }} />,
    );
    expect(error).toContain('unauthenticated: missing token');
    expect(error).toContain('shows no data rather than');
  });
});
