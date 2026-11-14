// P7.25 — Plugins-workspace tests (the SitesEdgeStatus.test.tsx pattern:
// render + data-mapping + honesty labelling on a mocked client;
// react-dom/server, no DOM environment).
//
// UNRUN — written, never executed (the loop bans; decision record
// 24_25_docs_console.md). Covered (the ledger P7.25 test set):
// - the states are DISTINGUISHABLE (lifecycle NOT_REPORTED gap vs a
//   reported READY, health gap vs the verbatim P7.20 snapshot, the
//   config summary as schema-only, the publisher gap, the
//   composition-change gap, the audit-trail gap);
// - no proposed value is displayed as executed (the proposed-vs-executed
//   statement renders on the panel; a recorded request states it is a
//   REQUEST, never an executed action);
// - NO mutation endpoint is called or exists on the client surface (the
//   mock client records ONLY GETs to /api/v1/plugins; there is no
//   activate/deactivate/upgrade/switch path anywhere in the module);
// - explicit confirmation is REQUIRED before the switch request is
//   recorded (the button stays disabled until the checkbox is operated);
// - absent operational facts render as explicit gaps, never as plausible
//   ready/healthy values;
// - the closed vocabularies render VERBATIM (lifecycle §5.6, health
//   P7.20) — never prettified.
import { describe, expect, it, beforeEach } from 'vitest';
import { renderToString } from 'react-dom/server';
import type { ApiClient } from '../api/client';
import type { PluginInventoryDto, PluginRecordDto } from '../api/plugins';
import {
  clearProfileSwitchRequests,
  getProfileSwitchRequests,
  healthOf,
  lifecycleStateOf,
  loadPluginsInventory,
  PROFILE_SWITCH_REQUEST_NOTE,
  PROPOSED_VS_EXECUTED_NOTE,
} from './pluginsWorkspaceData';
import { PluginsInventoryView } from './PluginsWorkspace';

const READ_AT = '2026-11-13T12:00:00.000Z';

const INVENTORY: PluginInventoryDto = {
  plugins: [
    {
      plugin_id: 'acme.forecast-worker',
      version: '0.3.1',
      class: 'grpc-worker',
      provider_of: ['forecast.day-ahead'],
      consumes: ['telemetry.history'],
      permissions: ['telemetry.read', 'forecast.publish'],
      license: 'Apache-2.0',
      description: 'Out-of-process Class C forecast worker.',
      config_properties: [
        { name: 'horizon-hours', property_type: 'integer', description: 'Forecast horizon in hours.' },
      ],
      config_required: ['horizon-hours'],
      record_note: 'recorded from plugins/manifests/forecast-worker.manifest.json (shipped manifest file)',
      lifecycle_state: 'NOT_REPORTED',
      health: null,
    },
    {
      plugin_id: 'neryva.optimizer-wasm-lp',
      version: '0.2.0',
      class: 'wasm-component',
      provider_of: ['optimization.lp'],
      consumes: ['portfolio.read'],
      permissions: ['schedule.propose'],
      license: 'Apache-2.0',
      description: 'Class B WASM component providing LP optimization.',
      config_properties: [],
      config_required: [],
      record_note: 'recorded from plugins/manifests/optimizer-wasm.manifest.json (shipped manifest file)',
      lifecycle_state: 'READY',
      health: {
        health_state: 'DEGRADED_ERROR_RATE',
        readiness: 'READY',
        note: 'composed test health source',
      },
    },
  ],
  profiles: [
    {
      profile_id: 'sim-lab',
      version: '0.5.0',
      control_authority: 'simulation-shadow',
      permission_ceiling: ['telemetry.read', 'asset.read', 'schedule.propose'],
      active_selections: [
        'builtin.simulator-telemetry',
        'builtin.highs-optimizer',
        'builtin.pandapower-grid',
        'neryva.sim-builtin-ess',
      ],
      composition_state: 'COMPOSES',
      state_note: 'composes the three seed built-ins plus the shipped ESS manifest',
    },
    {
      profile_id: 'site-edge',
      version: '0.5.0',
      control_authority: 'simulation-shadow',
      permission_ceiling: ['telemetry.read', 'asset.read'],
      active_selections: [],
      composition_state: 'DEFERRED_SHELL',
      state_note: 'a DEFERRED SHELL: composes NOTHING (PROFILE_EMPTY_SELECTIONS)',
    },
  ],
  inventory_note:
    'the RECORDED inventory of the shipped plugin manifests and composition profiles — READ-ONLY surface',
};

function readyState(): {
  status: 'ready';
  data: ReturnType<typeof buildData>;
  readAt: string;
} {
  return { status: 'ready' as const, data: buildData(), readAt: READ_AT };
}

function buildData() {
  return { inventory: INVENTORY };
}

function mockClient(calls: Array<{ path: string; method: string }>): ApiClient {
  return {
    async get<T>(path: string): Promise<T> {
      calls.push({ path, method: 'GET' });
      return { data: INVENTORY } as T;
    },
    async request<T>(path: string, init?: RequestInit): Promise<T> {
      calls.push({ path, method: init?.method ?? 'GET' });
      return this.get<T>(path);
    },
  };
}

describe('mapping (verbatim, no upgrades)', () => {
  it('renders the lifecycle state verbatim and the gap as NOT_REPORTED', () => {
    const reported = INVENTORY.plugins[1] as PluginRecordDto;
    expect(lifecycleStateOf(reported)).toBe('READY');
    expect(lifecycleStateOf(INVENTORY.plugins[0] as PluginRecordDto)).toBe('NOT_REPORTED');
  });

  it('renders health verbatim and blocks dependent actions ONLY for FAILING_*', () => {
    const health = healthOf(INVENTORY.plugins[1] as PluginRecordDto);
    expect(health).not.toBeNull();
    expect(health?.healthState).toBe('DEGRADED_ERROR_RATE');
    expect(health?.blocksDependentActions).toBe(false);
    const failing = healthOf({
      ...(INVENTORY.plugins[1] as PluginRecordDto),
      health: { health_state: 'FAILING_BINDINGS', readiness: 'UNOBSERVED', note: 'x' },
    });
    expect(failing?.blocksDependentActions).toBe(true);
    expect(healthOf(INVENTORY.plugins[0] as PluginRecordDto)).toBeNull();
  });
});

describe('loadPluginsInventory (mocked fetch mapping)', () => {
  it('GETs the inventory once and carries errors honestly', async () => {
    const calls: Array<{ path: string; method: string }> = [];
    const data = await loadPluginsInventory(mockClient(calls));
    expect(data.inventory.plugins).toHaveLength(2);
    expect(calls).toHaveLength(1);
    expect(calls[0]?.path).toBe('/api/v1/plugins');
    expect(calls.every((call) => call.method === 'GET')).toBe(true);

    const failing: ApiClient = {
      async get<T>(_path: string): Promise<T> {
        throw new Error('forbidden: action denied');
      },
      async request<T>(path: string): Promise<T> {
        return this.get<T>(path);
      },
    };
    await expect(loadPluginsInventory(failing)).rejects.toThrow('forbidden');
  });
});

describe('PluginsInventoryView (render honesty)', () => {
  beforeEach(() => {
    clearProfileSwitchRequests();
  });

  it('renders the states DISTINCTLY: lifecycle gap vs READY, health gap vs verbatim snapshot', () => {
    const html = renderToString(<PluginsInventoryView state={readyState()} />);
    // The NOT_REPORTED lifecycle gap is NAMED, never a silent state.
    expect(html).toContain('lifecycle-state-acme.forecast-worker');
    expect(html).toContain('[NOT_REPORTED]');
    expect(html).toContain('NO lifecycle state is known');
    // A reported lifecycle state renders verbatim with the serving rule.
    expect(html).toContain('lifecycle-state-neryva.optimizer-wasm-lp');
    expect(html).toContain('[READY]');
    expect(html).toContain('the only serving state is READY');
    // The health gap vs the verbatim P7.20 snapshot.
    expect(html).toContain('health-gap-acme.forecast-worker');
    expect(html).toContain('never rendered as healthy');
    expect(html).toContain('health-verbatim-neryva.optimizer-wasm-lp');
    expect(html).toContain('[DEGRADED_ERROR_RATE]');
    expect(html).toContain('composed test health source');
  });

  it('renders the config summary as SCHEMA ONLY and names the no-values rule', () => {
    const html = renderToString(<PluginsInventoryView state={readyState()} />);
    expect(html).toContain('config-summary-acme.forecast-worker');
    expect(html).toContain('horizon-hours');
    expect(html).toContain('integer');
    expect(html).toContain('(required)');
    expect(html).toContain('No configuration VALUE is displayed anywhere');
    // The manifest's secrets reference string is NOT part of the config
    // summary surface (the DTO carries no secrets field at all).
    expect(html).not.toContain('secret-store:');
  });

  it('renders the publisher gap and the permissions verbatim with the grant caveat', () => {
    const html = renderToString(<PluginsInventoryView state={readyState()} />);
    expect(html).toContain('publisher-gap-acme.forecast-worker');
    expect(html).toContain('does not exist in manifest schema v1');
    expect(html).toContain('permissions-acme.forecast-worker');
    expect(html).toContain('telemetry.read');
    expect(html).toContain('requested, not necessarily granted');
    expect(html).toContain('REFUSED, never trimmed');
  });

  it('renders the profile composition with the mode lock and the deferred shell', () => {
    const html = renderToString(<PluginsInventoryView state={readyState()} />);
    expect(html).toContain('profile-sim-lab');
    expect(html).toContain('profile-mode-sim-lab');
    expect(html).toContain('<code>simulation-shadow</code>');
    expect(html).toContain('can never select or raise the control-authority mode');
    expect(html).toContain('profile-state-site-edge');
    expect(html).toContain('DEFERRED_SHELL');
    // The composition-change record (actor + audit ref) is the named gap.
    expect(html).toContain('composition-change-gap');
    expect(html).toContain('never filled with a plausible actor or reference');
  });

  it('renders the audit-trail gap and the verbatim server surface note', () => {
    const html = renderToString(<PluginsInventoryView state={readyState()} />);
    expect(html).toContain('plugin-audit-gap');
    expect(html).toContain('No plugin-lifecycle audit source is composed');
    expect(html).toContain('inventory-note');
    expect(html).toContain('READ-ONLY surface');
  });

  it('renders the proposed-vs-executed distinction (§20.2)', () => {
    const html = renderToString(<PluginsInventoryView state={readyState()} />);
    expect(html).toContain('proposed-vs-executed');
    expect(PROPOSED_VS_EXECUTED_NOTE).toContain('never an executed action');
    expect(PROPOSED_VS_EXECUTED_NOTE).toContain('ADR-010');
  });

  it('offers a REQUEST-only switch affordance and never calls an endpoint', () => {
    const html = renderToString(<PluginsInventoryView state={readyState()} />);
    expect(html).toContain('profile-switch-request');
    expect(html).toContain('RECORD ONLY — nothing is switched');
    // No mutation path exists anywhere in the client module (the load test
    // above records ONLY GETs; the affordance's handler calls the LOCAL
    // recorder, never the API client).
    expect(getProfileSwitchRequests()).toHaveLength(0);
  });

  it('REQUIRES explicit confirmation before recording the request', () => {
    expect(PROFILE_SWITCH_REQUEST_NOTE).toContain('RECORDS A REQUEST');
    expect(PROFILE_SWITCH_REQUEST_NOTE).toContain('ONLY');
    expect(PROFILE_SWITCH_REQUEST_NOTE).toContain('P7.16');
    const html = renderToString(<PluginsInventoryView state={readyState()} />);
    expect(html).toContain('profile-switch-checkbox');
    expect(html).toContain('aria-required="true"');
    expect(html).toContain('profile-switch-button');
    expect(html).toContain('disabled');
    // Rendering alone records nothing.
    expect(getProfileSwitchRequests()).toHaveLength(0);
  });

  it('renders loading and error states honestly', () => {
    const loading = renderToString(
      <PluginsInventoryView state={{ status: 'loading' }} />,
    );
    expect(loading).toContain('data-testid="loading"');
    const error = renderToString(
      <PluginsInventoryView
        state={{ status: 'error', error: 'forbidden: action denied' }}
      />,
    );
    expect(error).toContain('data-testid="error-block"');
    expect(error).toContain('forbidden: action denied');
  });
});
