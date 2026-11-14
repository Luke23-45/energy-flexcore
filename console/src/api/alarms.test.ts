// P5.31 — the alarms API client tests: the wire-path pinning and the
// closed-vocabulary mirrors for the derived-on-read GET /api/v1/alarms
// resource (routes_alarms.rs; §20.1 item 1).
//
// UNRUN — written, never executed (the Phase 5 loop bans: no npm install,
// no vitest run; decision record 07_31_market_plugins_admin_alarms.md).
import { describe, expect, it } from 'vitest';
import {
  ALARM_CODES,
  ALARM_SEVERITIES,
  listAlarms,
  type AlarmPageEnvelope,
} from './alarms';
import type { ApiClient } from './client';

describe('the closed alarm vocabularies (mirrors of flexcore-contracts::alarm)', () => {
  it('pins the severity set exactly', () => {
    expect(ALARM_SEVERITIES).toEqual(['INFO', 'WARNING', 'CRITICAL']);
  });

  it('pins the code set exactly — one code per EXISTING source, no invented source', () => {
    expect(ALARM_CODES).toEqual([
      'TELEMETRY_LOSS',
      'TELEMETRY_STALENESS',
      'FEASIBILITY_LIMIT_VIOLATED',
      'FEASIBILITY_FAIL_CLOSED',
      'DISPATCH_FAILURE',
      'DISPATCH_EXPIRY',
      'DISPATCH_FALLBACK',
    ]);
    // The sources that do not exist must never enter the vocabulary.
    expect(ALARM_CODES).not.toContain('MARKET_SIGNAL_LOST');
    expect(ALARM_CODES).not.toContain('EDGE_ADAPTER_DOWN');
    expect(ALARM_CODES).not.toContain('PLUGIN_FAILED');
  });
});

function capturingClient() {
  const calls: string[] = [];
  const client: ApiClient = {
    async get<T>(path: string): Promise<T> {
      calls.push(path);
      return Promise.resolve({
        data: [],
        sources: {
          telemetry_current_state: 'available',
          telemetry_assets_consulted: 0,
          dispatch_commands_consulted: 0,
          feasibility_checks_consulted: 0,
          skipped_defective_source_records: 0,
          absent_sources: [],
        },
        derived_at: '2026-11-11T10:00:00Z',
        derivation_statement: 'statement',
        model_version: 'ALARM-MODEL-0.1.0 (P5.31)',
        next_cursor: null,
      } as AlarmPageEnvelope as T);
    },
    async request<T>(path: string): Promise<T> {
      return this.get<T>(path);
    },
  };
  return { client, calls };
}

describe('listAlarms (wire-path pinning)', () => {
  it('hits GET /api/v1/alarms with snake_case filters and no cursor by default', async () => {
    const { client, calls } = capturingClient();
    await listAlarms(
      {
        severity: 'WARNING',
        code: 'DISPATCH_FAILURE',
        assetId: 'asset-1',
        fromEventUnixMillis: 100,
        toEventUnixMillis: 200,
        activeOnly: true,
        limit: 50,
      },
      client,
    );
    expect(calls).toEqual([
      '/api/v1/alarms?severity=WARNING&code=DISPATCH_FAILURE&asset_id=asset-1&from_event_unix_millis=100&to_event_unix_millis=200&active_only=true&limit=50',
    ]);
  });

  it('sends the bare path when no filter is given', async () => {
    const { client, calls } = capturingClient();
    await listAlarms(undefined, client);
    expect(calls).toEqual(['/api/v1/alarms']);
  });

  it('never sends a tenant selector of any kind (§16.2)', async () => {
    const { client, calls } = capturingClient();
    await listAlarms({ severity: 'INFO', assetId: 'a' }, client);
    for (const call of calls) {
      expect(call).not.toContain('tenant');
    }
  });
});
