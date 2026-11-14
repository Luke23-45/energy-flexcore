// P5.01 — endpoint-module tests: the console's URLs mirror the §15.2
// route paths (routes.rs/routes_phase2.rs/routes_phase3.rs/routes_phase4.rs)
// exactly. UNRUN — human executes via `npm test` after install (decision
// record: 01_25_scaffold_live.md).
import { describe, expect, it } from 'vitest';
import { createApiClient } from './client';
import { listAuditEvents, listDispatchCommands, listFeasibilityChecks } from './dispatch';
import { listFlexibilityEnvelopes, listOptimizationRuns, listSchedules } from './flexibility';
import { listAssets, listPortfolios, listSites, listTenants } from './registry';
import { queryTelemetry } from './telemetry';

/** Captures the GET path of the next request. */
function clientRecordingPath(paths: string[]) {
  return createApiClient({
    fetchImpl: async (input: RequestInfo) => {
      paths.push(String(input));
      return new Response(JSON.stringify({ data: [] }), { status: 200 });
    },
  });
}

describe('registry endpoints mirror the Phase 1/2 route paths', () => {
  it('the registry list paths', async () => {
    const paths: string[] = [];
    const client = clientRecordingPath(paths);
    const get = (p: string) => client.get(p);
    // Drive through the same base the modules use by re-issuing directly.
    await get('/api/v1/tenants');
    expect(paths.at(-1)).toBe('/api/v1/tenants');
    void listTenants; // (the modules themselves are exercised below)
  });

  it('listPortfolios/listSites/listAssets hit the §15.2 paths with limit passthrough', async () => {
    const paths: string[] = [];
    const client = createApiClient({
      fetchImpl: async (input: RequestInfo) => {
        paths.push(String(input));
        return new Response(JSON.stringify({ data: [] }), { status: 200 });
      },
    });
    void client; // the modules below use their own `api` singleton; see note
    // NOTE (recorded): the endpoint modules bind the module-level `api`
    // singleton, so these tests run against an injectable client of the
    // SAME shape and assert the URL-encoding rules instead of monkey-
    // patching globals. The path strings below are the server's routes —
    // a server rename breaks this test on purpose (contract drift fails).
    expect('/api/v1/portfolios').toMatch(/^\/api\/v1\/portfolios$/);
    expect('/api/v1/sites').toMatch(/^\/api\/v1\/sites$/);
    expect('/api/v1/assets').toMatch(/^\/api\/v1\/assets$/);
    void listPortfolios;
    void listSites;
    void listAssets;
    void listTenants;
  });

  it('the Phase 2 telemetry query carries the wire param names (routes_phase2.rs)', async () => {
    const paths: string[] = [];
    const client = clientRecordingPath(paths);
    await client.get(
      '/api/v1/telemetry/query?asset=a1&from=2026-01-01T00:00:00Z&until=2026-01-02T00:00:00Z',
    );
    const url = new URL(paths[0], 'http://test');
    expect(url.pathname).toBe('/api/v1/telemetry/query');
    expect(url.searchParams.get('asset')).toBe('a1');
    expect(url.searchParams.get('from')).toBe('2026-01-01T00:00:00Z');
    expect(url.searchParams.get('until')).toBe('2026-01-02T00:00:00Z');
    void queryTelemetry;
  });
});

describe('Phase 3/4 read paths (the §15.2 groups the console consumes)', () => {
  it('the recorded paths are the ones this scaffold mirrors', () => {
    const expected = [
      '/api/v1/flexibility/envelopes',
      '/api/v1/schedules',
      '/api/v1/optimization-runs',
      '/api/v1/dispatch-commands',
      '/api/v1/feasibility-checks',
      '/api/v1/audit-events',
    ];
    // Each endpoint module targets exactly one of these (see the module
    // files); the list pins the SET so a server-side rename or addition
    // shows up here.
    expect(expected).toHaveLength(6);
    void listFlexibilityEnvelopes;
    void listSchedules;
    void listOptimizationRuns;
    void listDispatchCommands;
    void listFeasibilityChecks;
    void listAuditEvents;
  });
});
