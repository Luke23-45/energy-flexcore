// P5.07 — Administration workspace tests (render + gap-statement
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
import type { SiteDto, TenantDto } from '../api/types';
import type { SiteViewDto } from '../api/views';
import {
  AUTHZ_ACTION_SURFACE,
  AUTHZ_EXCLUDED_ACTIONS,
  AUTHZ_ROLES,
  D4_STATEMENT,
  EDGE_ENROLLMENT_STATEMENT,
  IDENTITY_STATEMENT,
  RETENTION_CLASSES,
  RETENTION_STATEMENT,
  TENANT_READ_STATEMENT,
  edgeEnrollmentRows,
} from './adminData';
import { AdministrationView, loadAdministration, type AdministrationData } from './Administration';

const TENANT: TenantDto = {
  id: 't-1',
  name: 'Tenant One (synthetic)',
  identity_provider_reference: 'idp://dev',
  policy_profile: 'default',
  created_at: '2026-11-11T09:00:00Z',
};

const SITE: SiteDto = {
  id: 'site-1',
  portfolio_id: 'p-1',
  tenant_id: 't-1',
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
  tenant_id: 't-1',
  name: 'Site One (synthetic)',
  location: 'lab',
  timezone: 'UTC',
  point_of_common_coupling: 'pcc-1',
  grid_model_reference: 'grid-a',
  edge_connectivity: { status: 'simulator', edge_identity: 'sim:edge-1', live: false },
  overview: { asset_count: 1 },
  created_at: '2026-11-11T09:00:00Z',
};

function data(overrides: Partial<AdministrationData> = {}): AdministrationData {
  return {
    tenant: TENANT,
    tenantError: null,
    enrollmentRows: edgeEnrollmentRows(
      [SITE],
      [SITE_VIEW],
      [null],
    ),
    ...overrides,
  };
}

describe('the administration honesty statements (the gap inventory)', () => {
  it('states the identity gap (dev-auth only, real OIDC is P5.26)', () => {
    expect(IDENTITY_STATEMENT).toContain('NO production identity provider');
    expect(IDENTITY_STATEMENT).toContain('dev-auth');
    expect(IDENTITY_STATEMENT).toContain('P5.26');
    expect(IDENTITY_STATEMENT).toContain('§16.2');
  });

  it('mirrors the closed action vocabulary with the drift note and the §5.7 exclusions', () => {
    // The P5.31 addition is on the mirror:
    const allActions = AUTHZ_ACTION_SURFACE.flatMap((group) => group.actions);
    expect(allActions).toContain('alarm.read');
    expect(allActions).toContain('audit.event.read');
    // The §5.7 exclusions must never appear in the mirrored surface:
    for (const excluded of AUTHZ_EXCLUDED_ACTIONS) {
      expect(allActions).not.toContain(excluded);
    }
    // The four roles, verbatim:
    expect(AUTHZ_ROLES).toEqual(['platform-admin', 'tenant-admin', 'operator', 'viewer']);
  });

  it('shows retention as the configuration shape (seven classes, no values)', () => {
    expect(RETENTION_CLASSES).toEqual([
      'raw_high_frequency_telemetry',
      'normalized_telemetry',
      'aggregated_intervals',
      'command_audit_evidence',
      'market_records',
      'simulation_artifacts',
      'model_forecast_outputs',
    ]);
    expect(RETENTION_STATEMENT).toContain('never as values');
    expect(RETENTION_STATEMENT).toContain('No universal retention period is chosen in code');
    expect(RETENTION_STATEMENT).toContain('NO read API serves a deployment');
  });

  it('states D4 OPEN and the honest edge-enrollment state (D5 OPEN)', () => {
    expect(D4_STATEMENT).toContain('D4 (license/dependency audit) is OPEN');
    expect(D4_STATEMENT).toContain('audit INPUTS');
    expect(EDGE_ENROLLMENT_STATEMENT).toContain('NO real edge agent is enrolled');
    expect(EDGE_ENROLLMENT_STATEMENT).toContain('D5 OPEN');
    expect(TENANT_READ_STATEMENT).toContain('READ-ONLY');
    expect(TENANT_READ_STATEMENT).toContain("caller's OWN tenant only");
  });
});

describe('loadAdministration (mocked fetch mapping)', () => {
  it('reads the tenant, sites, and site views; no tenant selector on the wire', async () => {
    const calls: string[] = [];
    const client: ApiClient = {
      async get<T>(path: string): Promise<T> {
        calls.push(path);
        if (path === '/api/v1/tenants') return { data: [TENANT] } as T;
        if (path === '/api/v1/sites') return { data: [SITE] } as T;
        if (path === '/api/v1/sites/site-1/view') return { data: SITE_VIEW } as T;
        throw new Error(`no fixture for ${path}`);
      },
      async request<T>(path: string): Promise<T> {
        return this.get<T>(path);
      },
    };
    const result = await loadAdministration(client);
    expect(result.tenant?.name).toBe('Tenant One (synthetic)');
    expect(result.enrollmentRows[0]?.view?.edge_connectivity.live).toBe(false);
    expect(calls).toContain('/api/v1/tenants');
    expect(calls).toContain('/api/v1/sites');
    for (const call of calls) {
      expect(call).not.toContain('tenant');
    }
  });

  it('keeps a failed tenant read as an honest error (never an invented tenant)', async () => {
    const client: ApiClient = {
      async get<T>(path: string): Promise<T> {
        if (path === '/api/v1/sites') return { data: [] } as T;
        throw new Error('no fixture for the tenant read');
      },
      async request<T>(path: string): Promise<T> {
        return this.get<T>(path);
      },
    };
    const result = await loadAdministration(client);
    expect(result.tenant).toBeNull();
    expect(result.tenantError).toContain('no fixture for the tenant read');
  });
});

describe('AdministrationView (render honesty)', () => {
  it('renders the tenant record, the action mirror, the retention shape, and the enrollment state', () => {
    const html = renderToString(
      <AdministrationView state={{ status: 'ready', data: data(), readAt: '2026-11-11T10:00:00Z' }} />,
    );
    expect(html).toContain('Tenant One (synthetic)');
    expect(html).toContain('authz-mirror');
    expect(html).toContain('alarm.read');
    expect(html).toContain('THE ENUM IS AUTHORITATIVE');
    expect(html).toContain('retention-classes');
    expect(html).toContain('UNSET (not served by any API in this phase)');
    expect(html).toContain('edge-enrollment-site-1');
    expect(html).toContain('<code>false</code>');
    expect(html).toContain('d4-status');
    expect(html).toContain('identity-gap');
  });

  it('renders the honest loading and error states', () => {
    expect(renderToString(<AdministrationView state={{ status: 'loading' }} />)).toContain(
      'Loading the administration data',
    );
    const error = renderToString(
      <AdministrationView state={{ status: 'error', error: 'unauthenticated: missing token' }} />,
    );
    expect(error).toContain('unauthenticated: missing token');
    expect(error).toContain('shows no data rather than');
  });
});
