// P5.01 — endpoint-module drift tests: every endpoint function below is
// INVOKED through an injected recording client and the produced URL is
// asserted against the pinned §15.2 route path (routes.rs / routes_phase2.rs
// / routes_phase3.rs / routes_phase4.rs / routes_alarms.rs). This gives the
// drift-detection property the module header claims, honestly:
// - a change to a path INSIDE an endpoint module fails (the module is
//   actually called — no `void` bindings, no literal-compared-to-literal);
// - a change to the pinned SERVER route fails (the expected paths here are
//   the server's routes.rs spellings, updated by hand when the server
//   changes — the same recorded hand-mirror limitation as the capability
//   matrix; the generated OpenAPI drift test (P5.01 step 4) replaces this
//   when the toolchain lands).
// Every api function covered here accepts `client: ApiClient = api`, so no
// module singleton is monkey-patched. UNRUN — human executes via `npm test`
// after install (decision record: 01_25_scaffold_live.md).
//
// FRV1-R3-003-adjacent honesty note (FRV1-R3-008): the previous version of
// this file asserted literal strings against their own regexes and `void`ed
// every module import — it could not fail on drift and claimed it could.
// This rewrite replaces it.
import { describe, expect, it } from 'vitest';
import { createApiClient, type ApiClient } from './client';
import {
  getAuditEvent,
  getDispatchCommand,
  getFeasibilityCheck,
  listAuditEvents,
  listDispatchCommands,
  listFeasibilityChecks,
} from './dispatch';
import { listFlexibilityEnvelopes, listOptimizationRuns, listSchedules } from './flexibility';
import {
  getAsset,
  getAssetView,
  getPortfolio,
  getPortfolioView,
  getSite,
  getSiteView,
  listAssets,
  listPortfolios,
  listSites,
  listTenants,
  getTenant,
} from './registry';
import { getAssetCurrentState, queryTelemetry, queryTelemetryHistory } from './telemetry';
import { listAlarms } from './alarms';

/** A client whose fetch records every requested URL and answers with a
 * minimal 200 JSON body (the functions only need the request to succeed). */
function recordingClient(paths: { requested: string[] }): ApiClient {
  return createApiClient({
    fetchImpl: async (input: RequestInfo) => {
      paths.requested.push(String(input));
      return new Response(JSON.stringify({ data: [] }), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      });
    },
  });
}

/** Splits a captured URL into path + searchParams (against a dummy origin;
 * the modules emit origin-less paths and the default client base is ''). */
function partsOf(url: string): { path: string; params: URLSearchParams } {
  const parsed = new URL(url, 'http://recorded.test');
  return { path: parsed.pathname, params: parsed.searchParams };
}

describe('registry endpoints (routes.rs / routes_phase2.rs — the module functions themselves)', () => {
  it('the list paths, called through the real modules', async () => {
    const recorded = { requested: [] as string[] };
    const client = recordingClient(recorded);
    await listTenants(client);
    await listPortfolios(undefined, client);
    await listSites(undefined, client);
    await listAssets(undefined, client);
    expect(recorded.requested).toEqual([
      '/api/v1/tenants',
      '/api/v1/portfolios',
      '/api/v1/sites',
      '/api/v1/assets',
    ]);
  });

  it('the detail paths encode the id and nothing else', async () => {
    const recorded = { requested: [] as string[] };
    const client = recordingClient(recorded);
    await getTenant('t-1', client);
    await getPortfolio('p/needs encoding', client);
    await getSite('s-1', client);
    await getAsset('a-1', client);
    expect(recorded.requested).toEqual([
      '/api/v1/tenants/t-1',
      '/api/v1/portfolios/p%2Fneeds%20encoding',
      '/api/v1/sites/s-1',
      '/api/v1/assets/a-1',
    ]);
  });

  it('the limit passthrough uses the wire param name `limit`', async () => {
    const recorded = { requested: [] as string[] };
    const client = recordingClient(recorded);
    await listPortfolios(5, client);
    const { path, params } = partsOf(recorded.requested[0]);
    expect(path).toBe('/api/v1/portfolios');
    expect(params.get('limit')).toBe('5');
  });

  it('the P2.19 view paths hang off the resource they view', async () => {
    const recorded = { requested: [] as string[] };
    const client = recordingClient(recorded);
    await getPortfolioView('p-1', client);
    await getSiteView('s-1', client);
    await getAssetView('a-1', client);
    expect(recorded.requested).toEqual([
      '/api/v1/portfolios/p-1/view',
      '/api/v1/sites/s-1/view',
      '/api/v1/assets/a-1/view',
    ]);
  });
});

describe('telemetry endpoints (routes_phase2.rs — the wire param names are pinned verbatim)', () => {
  it('queryTelemetry emits asset/from/until plus the optional wire params', async () => {
    const recorded = { requested: [] as string[] };
    const client = recordingClient(recorded);
    await queryTelemetry(
      {
        assetId: 'a-1',
        from: '2026-01-01T00:00:00Z',
        until: '2026-01-02T00:00:00Z',
        quantities: 'energy.active_import',
        quality: 'GOOD',
        cursor: 'cur-1',
        limit: 10,
      },
      client,
    );
    const { path, params } = partsOf(recorded.requested[0]);
    expect(path).toBe('/api/v1/telemetry/query');
    expect(params.get('asset')).toBe('a-1');
    expect(params.get('from')).toBe('2026-01-01T00:00:00Z');
    expect(params.get('until')).toBe('2026-01-02T00:00:00Z');
    expect(params.get('quantities')).toBe('energy.active_import');
    expect(params.get('quality')).toBe('GOOD');
    expect(params.get('cursor')).toBe('cur-1');
    expect(params.get('limit')).toBe('10');
  });

  it('queryTelemetryHistory hits /api/v1/telemetry/history with the pinned params', async () => {
    const recorded = { requested: [] as string[] };
    const client = recordingClient(recorded);
    await queryTelemetryHistory(
      {
        assetId: 'a-1',
        quantities: 'energy.active_import',
        from: '2026-01-01T00:00:00Z',
        until: '2026-01-02T00:00:00Z',
      },
      client,
    );
    const { path, params } = partsOf(recorded.requested[0]);
    expect(path).toBe('/api/v1/telemetry/history');
    expect(params.get('asset')).toBe('a-1');
    expect(params.get('quantities')).toBe('energy.active_import');
    expect(params.get('from')).toBe('2026-01-01T00:00:00Z');
    expect(params.get('until')).toBe('2026-01-02T00:00:00Z');
  });

  it('getAssetCurrentState hits the per-asset state path', async () => {
    const recorded = { requested: [] as string[] };
    const client = recordingClient(recorded);
    await getAssetCurrentState('a-1', client);
    expect(recorded.requested).toEqual(['/api/v1/assets/a-1/state']);
  });
});

describe('Phase 3 endpoints (routes_phase3.rs — called through the real modules)', () => {
  it('the flexibility/schedules/optimization-runs list paths', async () => {
    const recorded = { requested: [] as string[] };
    const client = recordingClient(recorded);
    await listFlexibilityEnvelopes(undefined, client);
    await listSchedules(undefined, client);
    await listOptimizationRuns(undefined, client);
    expect(recorded.requested).toEqual([
      '/api/v1/flexibility/envelopes',
      '/api/v1/schedules',
      '/api/v1/optimization-runs',
    ]);
  });

  it('the cursor/limit passthrough carries the wire param names', async () => {
    const recorded = { requested: [] as string[] };
    const client = recordingClient(recorded);
    await listSchedules({ cursor: 'cur-1', limit: 25 }, client);
    const { path, params } = partsOf(recorded.requested[0]);
    expect(path).toBe('/api/v1/schedules');
    expect(params.get('cursor')).toBe('cur-1');
    expect(params.get('limit')).toBe('25');
  });
});

describe('Phase 4 endpoints (routes_phase4.rs READ surface — called through the real modules)', () => {
  it('the dispatch-commands paths', async () => {
    const recorded = { requested: [] as string[] };
    const client = recordingClient(recorded);
    await listDispatchCommands({ state: 'ISSUED', limit: 10 }, client);
    await getDispatchCommand('c-1', client);
    const listed = partsOf(recorded.requested[0]);
    expect(listed.path).toBe('/api/v1/dispatch-commands');
    expect(listed.params.get('state')).toBe('ISSUED');
    expect(listed.params.get('limit')).toBe('10');
    expect(recorded.requested[1]).toBe('/api/v1/dispatch-commands/c-1');
  });

  it('the feasibility-checks paths carry the snake_case wire params', async () => {
    const recorded = { requested: [] as string[] };
    const client = recordingClient(recorded);
    await listFeasibilityChecks({ commandId: 'c-1', scheduleId: 'sch-1' }, client);
    await getFeasibilityCheck('f-1', client);
    const listed = partsOf(recorded.requested[0]);
    expect(listed.path).toBe('/api/v1/feasibility-checks');
    expect(listed.params.get('command_id')).toBe('c-1');
    expect(listed.params.get('schedule_id')).toBe('sch-1');
    expect(recorded.requested[1]).toBe('/api/v1/feasibility-checks/f-1');
  });

  it('the audit-events paths carry the snake_case wire params', async () => {
    const recorded = { requested: [] as string[] };
    const client = recordingClient(recorded);
    await listAuditEvents(
      { action: 'command.approve', objectType: 'command', fromEventUnixMillis: 42 },
      client,
    );
    await getAuditEvent('a-1', client);
    const listed = partsOf(recorded.requested[0]);
    expect(listed.path).toBe('/api/v1/audit-events');
    expect(listed.params.get('action')).toBe('command.approve');
    expect(listed.params.get('object_type')).toBe('command');
    expect(listed.params.get('from_event_unix_millis')).toBe('42');
    expect(recorded.requested[1]).toBe('/api/v1/audit-events/a-1');
  });
});

describe('Phase 5 endpoints (routes_alarms.rs — called through the real modules)', () => {
  it('listAlarms hits /api/v1/alarms with the wire param names', async () => {
    const recorded = { requested: [] as string[] };
    const client = recordingClient(recorded);
    await listAlarms(
      { severity: 'CRITICAL', assetId: 'a-1', activeOnly: true, limit: 5 },
      client,
    );
    const { path, params } = partsOf(recorded.requested[0]);
    expect(path).toBe('/api/v1/alarms');
    expect(params.get('severity')).toBe('CRITICAL');
    expect(params.get('asset_id')).toBe('a-1');
    expect(params.get('active_only')).toBe('true');
    expect(params.get('limit')).toBe('5');
  });
});
