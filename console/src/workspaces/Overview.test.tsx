// P5.02 — Overview workspace tests (render + data-mapping + honesty
// labelling on a mocked client).
//
// UNRUN — written, never executed (the Phase 5 loop bans: no npm install,
// no vitest run; decision record 02_04_workspaces_a.md). The human executes
// `npm test` after the first install. Rendering uses react-dom/server's
// renderToString (part of the existing react-dom dependency — NO new
// dependency is added); no DOM environment is required.
import { describe, expect, it } from 'vitest';
import { renderToString } from 'react-dom/server';
import type { ApiClient } from '../api/client';
import type { AlarmPageEnvelope } from '../api/alarms';
import type { CurrentStateDto, DispatchCommandDto, ScheduleRevisionDto } from '../api/types';
import {
  ALARM_CONTRACT_TEXT,
  aggregateActivePower,
  selectActiveSchedules,
  summarizeDeliveryStatus,
  summarizeFleetAvailability,
} from './overviewData';
import { OverviewView, loadOverview, type OverviewData } from './Overview';

// The P5.31 alarm fixture: the shape the derived-on-read GET /api/v1/alarms
// resource serves (dto_alarms.rs AlarmPageEnvelope; see api/alarms.ts).
const ALARMS_FIXTURE: AlarmPageEnvelope = {
  data: [
    {
      alarm_id: 'flexcore:alarm:DISPATCH_FAILURE:cmd-1',
      tenant_id: 't',
      site_id: 's',
      asset_id: 'a',
      code: 'DISPATCH_FAILURE',
      severity: 'CRITICAL',
      state: 'ACTIVE',
      message: 'dispatch command cmd-1 is in the FAILED state (the simulated lifecycle, §14.2)',
      raised_at: '2026-11-11T09:00:00Z',
      raised_at_unix_millis: 1765453200000,
      derived_at: '2026-11-11T10:00:00Z',
      source_kind: 'DISPATCH_COMMAND',
      source_reference: 'cmd-1:check:c',
      derivation_rule: 'p5_31/dispatch_outcome_alarm',
    },
  ],
  sources: {
    telemetry_current_state: 'available',
    telemetry_assets_consulted: 1,
    dispatch_commands_consulted: 1,
    feasibility_checks_consulted: 0,
    skipped_defective_source_records: 0,
    absent_sources: [
      'market signals and market adapters: no market integration exists in 0.x (D1/D2 OPEN)',
      'edge/adapter health: P6.17 (Stage 6)',
      'plugin health: no plugin kernel exists (the plugin SDK is Stage 7, v0.5.0)',
    ],
  },
  derived_at: '2026-11-11T10:00:00Z',
  derivation_statement:
    'Alarms are OPERATOR-ATTENTION records derived deterministically from existing platform facts (telemetry quality states, dispatch outcomes, feasibility verdicts). They are NOT protection signals and carry no protection-grade or safety semantics: an alarm never triggers, inhibits, or supervises any physical action. The frozen §15.3 event vocabulary is not extended — no alarm event exists; alarms are derived records (derived on read in the 0.x simulation line), never invented by the console.',
  model_version: 'ALARM-MODEL-0.1.0 (P5.31)',
  next_cursor: null,
};

function quantity(
  overrides: Partial<CurrentStateDto['quantities'][number]> & { quantity: string },
): CurrentStateDto['quantities'][number] {
  return {
    value_num: undefined,
    value_bool: undefined,
    value_text: undefined,
    unit: 'MW',
    quality: 'GOOD',
    q_sign_convention: undefined,
    event_time: '2026-11-11T10:00:00Z',
    ingest_time: '2026-11-11T10:00:01Z',
    age_ms: 1000,
    simulated: true,
    ...overrides,
  };
}

function assetState(
  assetId: string,
  quantities: CurrentStateDto['quantities'],
): CurrentStateDto {
  return {
    asset_id: assetId,
    generated_at: '2026-11-11T10:00:02Z',
    quantities,
    simulated: true,
  };
}

const READY_STATE: CurrentStateDto = assetState('asset-1', [
  quantity({ quantity: 'active_power', value_num: 1.5 }),
  quantity({ quantity: 'availability', value_bool: true, unit: '1' }),
]);

function overviewData(overrides: Partial<OverviewData> = {}): OverviewData {
  return {
    assets: [{ id: 'asset-1' }] as OverviewData['assets'],
    assetStates: [{ asset_id: 'asset-1', state: READY_STATE, error: null }],
    schedules: [],
    commands: [],
    alarms: ALARMS_FIXTURE,
    alarmsError: null,
    ...overrides,
  };
}

describe('aggregateActivePower (data mapping)', () => {
  it('sums usable (GOOD/SUSPECT) active_power contributors', () => {
    const result = aggregateActivePower([
      assetState('a', [quantity({ quantity: 'active_power', value_num: 1.5 })]),
      assetState('b', [quantity({ quantity: 'active_power', value_num: -2.25, quality: 'SUSPECT' })]),
    ]);
    expect(result.total_mw).toBeCloseTo(-0.75);
    expect(result.contributor_count).toBe(2);
    expect(result.excluded).toHaveLength(0);
    expect(result.all_inputs_simulated).toBe(true);
    expect(result.freshness_event_time).toBe('2026-11-11T10:00:00Z');
  });

  it('excludes BAD/MISSING/STALE contributors and never coerces them', () => {
    const result = aggregateActivePower([
      assetState('a', [quantity({ quantity: 'active_power', value_num: 9, quality: 'BAD' })]),
      assetState('b', [quantity({ quantity: 'active_power', quality: 'MISSING' })]),
      assetState('c', [quantity({ quantity: 'active_power', value_num: 7, quality: 'STALE' })]),
    ]);
    // No usable contributor: the aggregate is null, never 0.
    expect(result.total_mw).toBeNull();
    expect(result.excluded.map((entry) => entry.asset_id)).toEqual(['a', 'b', 'c']);
    expect(result.excluded[0]?.reason).toContain('never coerced');
    expect(result.excluded[1]?.reason).toContain('never read as zero');
  });

  it('reports which inputs are simulated from the records themselves', () => {
    const result = aggregateActivePower([
      assetState('a', [quantity({ quantity: 'active_power', value_num: 1, simulated: false })]),
    ]);
    expect(result.all_inputs_simulated).toBe(false);
  });
});

describe('summarizeFleetAvailability (the mirrored P2.16 rule)', () => {
  it('derives available/unavailable/unknown per the P2.16 branches', () => {
    const summary = summarizeFleetAvailability([
      assetState('a', [quantity({ quantity: 'availability', value_bool: true, unit: '1' })]),
      assetState('b', [quantity({ quantity: 'availability', value_bool: false, unit: '1' })]),
      assetState('c', [quantity({ quantity: 'availability', quality: 'BAD', unit: '1' })]),
      assetState('d', []), // no availability record at all
    ]);
    expect(summary.available).toEqual(['a']);
    expect(summary.unavailable).toEqual(['b']);
    expect(summary.unknown).toEqual(['c', 'd']);
    expect(summary.no_new_signal).toEqual([]);
  });

  it('maps MISSING and STALE signals to no-new-signal (never an invented state)', () => {
    const summary = summarizeFleetAvailability([
      assetState('a', [quantity({ quantity: 'availability', quality: 'STALE', unit: '1' })]),
      assetState('b', [quantity({ quantity: 'availability', quality: 'MISSING', unit: '1' })]),
    ]);
    expect(summary.no_new_signal).toEqual(['a', 'b']);
    expect(summary.available).toEqual([]);
  });
});

describe('selectActiveSchedules (ID + revision)', () => {
  const schedule: ScheduleRevisionDto = {
    schedule_id: 'sch-1',
    revision: 3,
    tenant_id: 't',
    portfolio_id: 'p',
    site_id: 's',
    horizon_start_millis: 0,
    horizon_end_millis: 1,
    roll_trigger: 'initial_solve',
    state: 'active',
    candidate_id: 'c',
    candidate_status: 'candidate',
    candidate_status_statement: 'statement',
    validation_scope: 'asset_time_coupled',
    validation_scope_statement: 'statement',
    schedule_payload: {},
  };

  it('keeps only active revisions (superseded are not shown as active)', () => {
    const rows = selectActiveSchedules([schedule, { ...schedule, revision: 2, state: 'superseded' }]);
    expect(rows).toHaveLength(1);
    expect(rows[0]).toMatchObject({ schedule_id: 'sch-1', revision: 3 });
  });
});

describe('summarizeDeliveryStatus (simulation-honest delivery)', () => {
  const command: DispatchCommandDto = {
    command_id: 'cmd-1',
    tenant_id: 't',
    portfolio_id: 'p',
    site_id: 's',
    asset_id: 'a',
    schedule_id: 'sch',
    schedule_revision: 1,
    valid_until_millis: 0,
    state: 'ISSUED',
    command_payload: {},
    no_execute_endpoint_statement: 'read-only statement',
    simulation_only_statement: 'SIMULATION ONLY statement',
  };

  it('counts per §14.2 state and flags the simulator-qualified states', () => {
    const summary = summarizeDeliveryStatus([
      command,
      { ...command, command_id: 'cmd-2', state: 'ACKNOWLEDGED' },
      { ...command, command_id: 'cmd-3', state: 'ACKNOWLEDGED' },
      { ...command, command_id: 'cmd-4', state: 'COMPLETED' },
    ]);
    expect(summary.counts).toEqual({ ISSUED: 1, ACKNOWLEDGED: 2, COMPLETED: 1 });
    expect(summary.issued_to_simulator).toBe(1);
    expect(summary.acknowledged_not_delivery).toBe(2);
    expect(summary.total).toBe(4);
  });
});

function fakeClient(responses: Record<string, unknown>): { client: ApiClient; calls: string[] } {
  const calls: string[] = [];
  const client: ApiClient = {
    async get<T>(path: string): Promise<T> {
      calls.push(path);
      const body = responses[path];
      if (body === undefined) throw new Error(`no fixture for ${path}`);
      return body as T;
    },
    async request<T>(path: string): Promise<T> {
      return this.get<T>(path);
    },
  };
  return { client, calls };
}

describe('loadOverview (mocked fetch mapping)', () => {
  it('reads the tenant-scoped §15.2 resources and per-asset states', async () => {
    const assetEnvelope = {
      data: [{ id: 'asset-1', name: 'Asset 1', tenant_id: 't' }],
    };
    const { client, calls } = fakeClient({
      '/api/v1/assets': { data: assetEnvelope.data },
      '/api/v1/schedules': { data: [] },
      '/api/v1/dispatch-commands': { data: [] },
      '/api/v1/assets/asset-1/state': { data: READY_STATE },
      '/api/v1/alarms': ALARMS_FIXTURE,
    });
    const data = await loadOverview(client);
    expect(data.assets).toHaveLength(1);
    expect(data.assetStates[0]?.state).toEqual(READY_STATE);
    expect(data.alarms?.data[0]?.code).toBe('DISPATCH_FAILURE');
    expect(calls).toContain('/api/v1/assets');
    expect(calls).toContain('/api/v1/schedules');
    expect(calls).toContain('/api/v1/dispatch-commands');
    expect(calls).toContain('/api/v1/assets/asset-1/state');
    expect(calls).toContain('/api/v1/alarms');
    // §16.2: the console never sends a tenant selector of any kind.
    for (const call of calls) {
      expect(call).not.toContain('tenant');
    }
  });

  it('keeps a failed alarms read as an honest per-panel error (never an empty list)', async () => {
    const { client } = fakeClient({
      '/api/v1/assets': { data: [{ id: 'asset-1' }] },
      '/api/v1/schedules': { data: [] },
      '/api/v1/dispatch-commands': { data: [] },
      '/api/v1/assets/asset-1/state': { data: READY_STATE },
      // no fixture for /api/v1/alarms -> the fetch throws
    });
    const data = await loadOverview(client);
    expect(data.alarms).toBeNull();
    expect(data.alarmsError).toContain('no fixture for');
  });

  it('keeps a failed per-asset state read as an honest error entry', async () => {
    const { client } = fakeClient({
      '/api/v1/assets': { data: [{ id: 'asset-1' }] },
      '/api/v1/schedules': { data: [] },
      '/api/v1/dispatch-commands': { data: [] },
      // no fixture for the state read -> the fetch throws
    });
    const data = await loadOverview(client);
    expect(data.assetStates[0]?.state).toBeNull();
    expect(data.assetStates[0]?.error).toContain('no fixture for');
  });
});

describe('OverviewView (render honesty)', () => {
  it('renders all five §20.1 item 1 elements with freshness and mode', () => {
    const data = overviewData({
      schedules: [
        {
          schedule_id: 'sch-1',
          revision: 1,
          tenant_id: 't',
          portfolio_id: 'p',
          site_id: 's',
          horizon_start_millis: 0,
          horizon_end_millis: 1,
          roll_trigger: 'initial_solve',
          state: 'active',
          candidate_id: 'c',
          candidate_status: 'candidate',
          candidate_status_statement: 'statement',
          validation_scope: 'asset_time_coupled',
          validation_scope_statement: 'statement',
          schedule_payload: {},
        },
      ],
      commands: [
        {
          command_id: 'cmd-1',
          tenant_id: 't',
          portfolio_id: 'p',
          site_id: 's',
          asset_id: 'a',
          schedule_id: 'sch-1',
          schedule_revision: 1,
          valid_until_millis: 0,
          state: 'ACKNOWLEDGED',
          command_payload: {},
          no_execute_endpoint_statement: 'read-only',
          simulation_only_statement: 'SIMULATION ONLY (0.x line)',
        },
      ],
    });
    const html = renderToString(
      <OverviewView state={{ status: 'ready', data, readAt: '2026-11-11T10:00:00Z' }} />,
    );
    // The five §20.1 item 1 elements are present:
    expect(html).toContain('Fleet availability');
    expect(html).toContain('Current aggregate power');
    expect(html).toContain('Active schedules');
    expect(html).toContain('Key alarms');
    expect(html).toContain('Delivery status');
    // §20.2 freshness on the values:
    expect(html).toContain('event time 2026-11-11T10:00:00Z');
    expect(html).toContain('read at 2026-11-11T10:00:00Z');
    // The simulated type treatment is part of the component:
    expect(html).toContain('value-type-simulated');
    expect(html).toContain('[simulated]');
    // Sign convention labelled (MW, P > 0 export):
    expect(html).toContain('P &gt; 0 means export at the point of connection');
    // Control-authority mode shown prominently:
    expect(html).toContain('mode-line');
    expect(html).toContain('SIMULATION / READ-ONLY (SHADOW)');
    // Delivery status qualifies acknowledgement (never physical delivery):
    expect(html).toContain('acknowledgement is NOT physical delivery');
    expect(html).toContain('SIMULATION ONLY (0.x line)');
    // The alarm panel is the real P5.31 derived list (the gap is resolved):
    expect(html).toContain('alarms-panel');
    expect(html).toContain('DISPATCH_FAILURE');
    expect(html).toContain('[CRITICAL]');
    // The model-source citation travels verbatim under the panel:
    expect(html).toContain('ALARM-MODEL-0.1.0 (P5.31)');
    expect(html).toContain('NOT protection signals');
    expect(html).toContain('alarm-contract-note');
    expect(html).toContain(ALARM_CONTRACT_TEXT);
    // The consulted/absent source inventory is rendered (no invented sources):
    expect(html).toContain('edge/adapter health: P6.17 (Stage 6)');
  });

  it('renders the honest alarm empty state when nothing is derivable', () => {
    const data = overviewData({
      alarms: { ...ALARMS_FIXTURE, data: [] },
    });
    const html = renderToString(
      <OverviewView state={{ status: 'ready', data, readAt: '2026-11-11T10:00:00Z' }} />,
    );
    expect(html).toContain('No alarms are currently derivable');
    // An empty derivation is not presented as "everything is healthy".
    expect(html).toContain('not a promise');
  });

  it('renders the honest alarm error block when the derivation cannot be read', () => {
    const data = overviewData({ alarms: null, alarmsError: 'forbidden: no matching rule' });
    const html = renderToString(
      <OverviewView state={{ status: 'ready', data, readAt: 'x' }} />,
    );
    expect(html).toContain('alarms-error');
    expect(html).toContain('forbidden: no matching rule');
    expect(html).toContain('a failed derivation is not a clean derivation');
  });

  it('shows the aggregate as missing (never 0 MW) when nothing is usable', () => {
    const data = overviewData({
      assetStates: [
        {
          asset_id: 'asset-1',
          state: assetState('asset-1', [quantity({ quantity: 'active_power', quality: 'BAD' })]),
          error: null,
        },
      ],
    });
    const html = renderToString(
      <OverviewView state={{ status: 'ready', data, readAt: '2026-11-11T10:00:00Z' }} />,
    );
    expect(html).toContain('NOT shown as 0 MW');
    expect(html).not.toContain('value-type-simulated"><strong'); // no aggregate row rendered
  });

  it('lists excluded assets and failed state reads instead of dropping them', () => {
    const data = overviewData({
      assetStates: [
        { asset_id: 'asset-1', state: READY_STATE, error: null },
        { asset_id: 'asset-2', state: null, error: 'forbidden: no matching rule' },
      ],
    });
    const html = renderToString(
      <OverviewView state={{ status: 'ready', data, readAt: '2026-11-11T10:00:00Z' }} />,
    );
    expect(html).toContain('state-read-failures');
    expect(html).toContain('forbidden: no matching rule');
    expect(html).toContain('never silently dropped');
  });

  it('renders honest loading and error states', () => {
    const loading = renderToString(<OverviewView state={{ status: 'loading' }} />);
    expect(loading).toContain('Loading the overview');
    const error = renderToString(
      <OverviewView state={{ status: 'error', error: 'unauthenticated: missing token' }} />,
    );
    expect(error).toContain('unauthenticated: missing token');
    expect(error).toContain('shows no data rather than');
  });
});
