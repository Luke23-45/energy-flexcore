// P5.03 — Assets workspace tests (render + data-mapping + honesty
// labelling on a mocked client).
//
// UNRUN — written, never executed (Phase 5 loop bans; decision record
// 02_04_workspaces_a.md). Renders via react-dom/server (existing
// react-dom dependency; no new dependency, no DOM environment).
import { describe, expect, it } from 'vitest';
import { renderToString } from 'react-dom/server';
import type { ApiClient } from '../api/client';
import type { AlarmPageEnvelope } from '../api/alarms';
import type { AssetDto, CurrentStateDto, TelemetryQuality } from '../api/types';
import { AssetsView, loadAssets, type AssetsData } from './Assets';
import { deriveAvailability } from '../components/availability';

const ASSET: AssetDto = {
  id: 'asset-1',
  site_id: 'site-1',
  tenant_id: 'tenant-1',
  name: 'BESS 1 (synthetic)',
  asset_type: 'bess',
  vendor_model: 'sim/bess-generic',
  rated_power_mw: 2,
  rated_energy_mwh: 4,
  phase: 'three',
  bus_reference: 'bus-2',
  capability_profile: 'cap:bess-v1',
  lifecycle_status: 'active',
  created_at: '2026-11-11T09:00:00Z',
};

function q(overrides: {
  quantity: string;
  quality?: TelemetryQuality;
  value_num?: number;
  value_bool?: boolean;
  unit?: string;
  q_sign_convention?: string;
}): CurrentStateDto['quantities'][number] {
  return {
    value_num: overrides.value_num,
    value_bool: overrides.value_bool,
    value_text: undefined,
    unit: overrides.unit ?? 'MW',
    quality: overrides.quality ?? 'GOOD',
    q_sign_convention: overrides.q_sign_convention,
    event_time: '2026-11-11T10:00:00Z',
    ingest_time: '2026-11-11T10:00:01Z',
    age_ms: 120000,
    simulated: true,
  };
}

const STATE: CurrentStateDto = {
  asset_id: 'asset-1',
  generated_at: '2026-11-11T10:00:02Z',
  quantities: [
    q({ quantity: 'active_power', value_num: -1.2 }),
    q({ quantity: 'soc', value_num: 0.62, unit: '1' }),
    q({ quantity: 'availability', value_bool: true, unit: '1' }),
    q({
      quantity: 'reactive_power',
      value_num: 0.1,
      unit: 'MVAr',
      quality: 'ESTIMATED',
      q_sign_convention: 'injection_positive (declared per device profile)',
    }),
  ],
  simulated: true,
};

// The P5.31 alarm fixture (the derived-on-read GET /api/v1/alarms shape):
// one alarm scoped to asset-1, plus a tenant-wide alarm that must NOT
// appear under the asset panel.
const ALARMS_FIXTURE: AlarmPageEnvelope = {
  data: [
    {
      alarm_id: 'flexcore:alarm:TELEMETRY_LOSS:asset-1:active_power',
      tenant_id: 'tenant-1',
      site_id: 'site-1',
      asset_id: 'asset-1',
      code: 'TELEMETRY_LOSS',
      severity: 'WARNING',
      state: 'ACTIVE',
      message: "telemetry quantity 'active_power' is BAD on asset asset-1",
      raised_at: '2026-11-11T10:00:00Z',
      raised_at_unix_millis: 1765456800000,
      derived_at: '2026-11-11T10:00:02Z',
      source_kind: 'TELEMETRY_CURRENT_STATE',
      source_reference: 'asset-1:active_power',
      derivation_rule: 'p5_31/telemetry_quality_alarm',
    },
    {
      alarm_id: 'flexcore:alarm:DISPATCH_EXPIRY:cmd-9',
      tenant_id: 'tenant-1',
      code: 'DISPATCH_EXPIRY',
      severity: 'WARNING',
      state: 'ACTIVE',
      message: 'dispatch command cmd-9 is in the EXPIRED state',
      raised_at: '2026-11-11T09:30:00Z',
      raised_at_unix_millis: 1765455000000,
      derived_at: '2026-11-11T10:00:02Z',
      source_kind: 'DISPATCH_COMMAND',
      source_reference: 'cmd-9',
      derivation_rule: 'p5_31/dispatch_outcome_alarm',
    },
  ],
  sources: {
    telemetry_current_state: 'available',
    telemetry_assets_consulted: 1,
    dispatch_commands_consulted: 0,
    feasibility_checks_consulted: 0,
    skipped_defective_source_records: 0,
    absent_sources: ['edge/adapter health: P6.17 (Stage 6)'],
  },
  derived_at: '2026-11-11T10:00:02Z',
  derivation_statement:
    'Alarms are OPERATOR-ATTENTION records derived deterministically from existing platform facts. They are NOT protection signals.',
  model_version: 'ALARM-MODEL-0.1.0 (P5.31)',
  next_cursor: null,
};

function assetsData(overrides: Partial<AssetsData> = {}): AssetsData {
  return {
    rows: [
      {
        asset: ASSET,
        view: {
          id: 'asset-1',
          site_id: 'site-1',
          tenant_id: 'tenant-1',
          name: 'BESS 1 (synthetic)',
          asset_type: 'bess',
          vendor_model: 'sim/bess-generic',
          capability_profile: 'cap:bess-v1',
          limits: { rated_power_mw: 2, rated_energy_mwh: 4 },
          lifecycle_status: 'active',
          maintenance_outage_status: 'operational',
          current_state: STATE,
          created_at: '2026-11-11T09:00:00Z',
        },
        viewError: null,
        state: STATE,
        stateError: null,
      },
    ],
    alarms: ALARMS_FIXTURE,
    alarmsError: null,
    ...overrides,
  };
}

describe('deriveAvailability (the mirrored P2.16 rule branches)', () => {
  it('covers all six quality states plus the no-record case', () => {
    expect(deriveAvailability('GOOD', true)).toBe('available');
    expect(deriveAvailability('SUSPECT', false)).toBe('unavailable');
    expect(deriveAvailability('BAD', true)).toBe('unknown');
    expect(deriveAvailability('MISSING', undefined)).toBe('no-new-signal');
    expect(deriveAvailability('STALE', true)).toBe('no-new-signal');
    expect(deriveAvailability('ESTIMATED', undefined)).toBe('unknown');
    expect(deriveAvailability(null, undefined)).toBe('unknown');
  });
});

describe('loadAssets (mocked fetch mapping)', () => {
  it('reads the tenant-scoped registry, views, and per-asset current states', async () => {
    const calls: string[] = [];
    const client: ApiClient = {
      async get<T>(path: string): Promise<T> {
        calls.push(path);
        const responses: Record<string, unknown> = {
          '/api/v1/assets': { data: [ASSET] },
          '/api/v1/assets/asset-1/view': { data: assetsData().rows[0]?.view },
          '/api/v1/assets/asset-1/state': { data: STATE },
          '/api/v1/alarms': ALARMS_FIXTURE,
        };
        const body = responses[path];
        if (body === undefined) throw new Error(`no fixture for ${path}`);
        return body as T;
      },
      async request<T>(path: string): Promise<T> {
        return this.get<T>(path);
      },
    };
    const data = await loadAssets(client);
    expect(data.rows[0]?.state?.quantities).toHaveLength(4);
    expect(data.alarms?.data[0]?.code).toBe('TELEMETRY_LOSS');
    expect(calls).toContain('/api/v1/assets');
    expect(calls).toContain('/api/v1/assets/asset-1/view');
    expect(calls).toContain('/api/v1/assets/asset-1/state');
    expect(calls).toContain('/api/v1/alarms');
  });
});

describe('AssetsView (render honesty)', () => {
  it('renders metadata, per-value quality states, freshness, and the simulated type', () => {
    const html = renderToString(
      <AssetsView state={{ status: 'ready', data: assetsData(), readAt: '2026-11-11T10:00:00Z' }} />,
    );
    expect(html).toContain('BESS 1 (synthetic)');
    expect(html).toContain('cap:bess-v1');
    // Stored limits are labelled as registry magnitudes, not envelopes:
    expect(html).toContain('NOT runtime capability envelopes');
    // Quality per value (§10.4) — every row carries its frozen code:
    expect(html).toContain('quality: GOOD');
    expect(html).toContain('quality: ESTIMATED');
    // The simulated provenance types the values (P2.11: every Phase 2
    // telemetry value is simulator-derived):
    expect(html).toContain('[simulated]');
    // Freshness per value:
    expect(html).toContain('event time 2026-11-11T10:00:00Z (age 2 min 0 s)');
    // SOC is a fraction (unit "1"), never a percent invented:
    expect(html).toContain('0.62');
    // Declared reactive sign convention shown when the record declares one:
    expect(html).toContain('declared Q sign');
  });

  it('renders the derived availability state with its basis', () => {
    const html = renderToString(
      <AssetsView state={{ status: 'ready', data: assetsData(), readAt: 'x' }} />,
    );
    expect(html).toContain('availability-available');
    expect(html).toContain('latest availability record: quality GOOD, value true');
    expect(html).toContain('P2.16');
  });

  it('renders the real per-asset alarm list (P5.31), filtered to the asset scope', () => {
    const html = renderToString(
      <AssetsView state={{ status: 'ready', data: assetsData(), readAt: 'x' }} />,
    );
    // The alarm scoped to asset-1 appears under its panel:
    expect(html).toContain('asset-alarms-asset-1');
    expect(html).toContain('TELEMETRY_LOSS');
    // The tenant-wide alarm (no asset_id) must NOT appear under the asset:
    expect(html).not.toContain('DISPATCH_EXPIRY');
    // The model-source citation travels verbatim:
    expect(html).toContain('ALARM-MODEL-0.1.0 (P5.31)');
    expect(html).toContain('NOT protection signals');
  });

  it('renders the honest per-asset alarm empty state when nothing is derivable', () => {
    const html = renderToString(
      <AssetsView
        state={{
          status: 'ready',
          data: assetsData({ alarms: { ...ALARMS_FIXTURE, data: [] } }),
          readAt: 'x',
        }}
      />,
    );
    expect(html).toContain('No alarms are currently derivable for this asset');
  });

  it('renders a failed alarms read as an honest error, never an empty list', () => {
    const html = renderToString(
      <AssetsView
        state={{ status: 'ready', data: assetsData({ alarms: null, alarmsError: 'forbidden: denied' }), readAt: 'x' }}
      />,
    );
    expect(html).toContain('asset-alarms-error');
    expect(html).toContain('forbidden: denied');
  });

  it('never renders a value where the record carries none', () => {
    const data = assetsData({
      rows: [
        {
          asset: ASSET,
          view: null,
          viewError: null,
          state: {
            asset_id: 'asset-1',
            generated_at: '2026-11-11T10:00:02Z',
            quantities: [q({ quantity: 'active_power', quality: 'MISSING' })],
            simulated: true,
          },
          stateError: null,
        },
      ],
    });
    const html = renderToString(
      <AssetsView state={{ status: 'ready', data, readAt: 'x' }} />,
    );
    expect(html).toContain('never read as zero');
  });

  it('renders honest read errors, empty, and loading states', () => {
    const withErrors = assetsData({
      rows: [{ asset: ASSET, view: null, viewError: 'forbidden: no matching rule', state: null, stateError: 'not_found: missing' }],
    });
    const html = renderToString(
      <AssetsView state={{ status: 'ready', data: withErrors, readAt: 'x' }} />,
    );
    expect(html).toContain('asset-view-error');
    expect(html).toContain('asset-state-error');
    expect(html).toContain('forbidden: no matching rule');
    expect(renderToString(<AssetsView state={{ status: 'loading' }} />)).toContain('Loading the assets');
    const empty = renderToString(
      <AssetsView state={{ status: 'ready', data: { rows: [], alarms: null, alarmsError: null }, readAt: 'x' }} />,
    );
    expect(empty).toContain('returned no assets');
  });
});
