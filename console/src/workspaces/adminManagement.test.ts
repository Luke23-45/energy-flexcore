// P13.18 — the management-catalog data tests (WRITTEN-UNRUN; the human
// gate runs `npm test` after the first install). The tests pin the plan
// risk-6 discipline: every named endpoint is specified-not-implemented,
// no never-granted name appears anywhere, and the consumed models are the
// record 05/08/21/42 shapes (mirrors, never redefinitions).
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { CAPABILITY_MATRIX, ROLES } from '../security/capabilityMatrix';
import {
  ADMIN_MANAGEMENT_NOTE,
  DISPLAY_NEVER_SECRET_NOTE,
  MANAGEMENT_ACTIONS,
  MANAGEMENT_SURFACES,
  PHASE13_ABSENT_GRANTS_NOTES,
  PHASE13_GRANT_TABLE,
  PHASE13_ROLES,
  SPECIFIED_NOT_IMPLEMENTED,
  unspecifiedEndpointActions,
} from './adminManagement';

describe('the role model mirror (record 05, consumed by citation)', () => {
  it('carries the closed five-role vocabulary (the P13.05 extension included)', () => {
    expect(PHASE13_ROLES).toEqual([
      'platform-admin',
      'tenant-admin',
      'operator',
      'viewer',
      'site-approver',
    ]);
  });

  it('the role vocabulary is DERIVED from the single authoritative mirror, not redeclared (FRV1-R3-003)', () => {
    // PHASE13_ROLES re-exports capabilityMatrix ROLES — a drift in either
    // place would break this identity, so the two mirrors cannot disagree
    // on the role vocabulary.
    expect(PHASE13_ROLES).toBe(ROLES);
    expect([...ROLES]).toEqual([...PHASE13_ROLES]);
  });

  it('the phase-13 grant table agrees with the capability-matrix phase-13 entries (one recorded grant set, two mirrors)', () => {
    for (const row of PHASE13_GRANT_TABLE) {
      const entry = CAPABILITY_MATRIX[row.actionId];
      expect(entry, `${row.actionId} must exist in CAPABILITY_MATRIX`).toBeDefined();
      // The admin table mirrors the PHASE-13 SET ALONE; the matrix entry
      // is the union with any earlier recorded grant, so every phase-13
      // role must be present there, and the scope kinds must match.
      for (const role of row.roles) {
        expect(entry!.roles, `${row.actionId}: ${role} must be mirrored`).toContain(role);
      }
      expect([...entry!.scopeKinds]).toEqual([...row.scopeKinds]);
    }
  });

  it('mirrors the record 05 §3 grant table exactly', () => {
    expect(PHASE13_GRANT_TABLE.map((row) => row.actionId)).toEqual([
      'command.approve',
      'apikey.create',
      'apikey.rotate',
      'apikey.revoke',
      'apikey.read',
    ]);
    const approve = PHASE13_GRANT_TABLE[0];
    expect(approve.roles).toEqual(['site-approver']);
    expect(approve.scopeKinds).toEqual(['site', 'asset']);
    for (const row of PHASE13_GRANT_TABLE.slice(1)) {
      expect(row.roles).toEqual(['platform-admin', 'tenant-admin']);
      expect(row.scopeKinds).toEqual(['tenant']);
    }
  });

  it('records the deliberate absences including the never-granted set', () => {
    const joined = PHASE13_ABSENT_GRANTS_NOTES.join(' ');
    expect(joined).toContain('SiteApprover holds NO grant');
    expect(joined).toContain('NO apikey.* grant');
    expect(joined).toContain('No tenant-level command.approve grant');
    expect(joined).toContain('dispatch.authorize');
    expect(joined).toContain('network.any');
  });
});

describe('the management-action catalog (plan risk 6, pinned)', () => {
  it('covers exactly the seven recorded surfaces', () => {
    expect(MANAGEMENT_SURFACES).toEqual([
      'users',
      'roles',
      'api-keys',
      'webhooks',
      'sessions',
      'policy',
      'tenant-data',
    ]);
    for (const surface of MANAGEMENT_SURFACES) {
      expect(
        MANAGEMENT_ACTIONS.some((action) => action.surface === surface),
        `the ${surface} surface has at least one catalog row`,
      ).toBe(true);
    }
  });

  it('EVERY action with a named endpoint is specified-not-implemented (no served endpoint is claimed)', () => {
    const served = MANAGEMENT_ACTIONS.filter((action) => action.endpointStatus !== 'specified-not-implemented');
    for (const action of served) {
      expect(action.endpoint, `${action.id} with status ${action.endpointStatus} names no endpoint`).toBeNull();
    }
    // And the converse set is non-empty (the catalog names endpoints).
    expect(unspecifiedEndpointActions().length).toBeGreaterThan(0);
  });

  it('the named endpoints are the recorded design shapes (spot-pinned)', () => {
    const byId = new Map(MANAGEMENT_ACTIONS.map((action) => [action.id, action]));
    expect(byId.get('apikeys.list')?.endpoint).toBe('GET /api/v1/api-keys');
    expect(byId.get('apikeys.rotate')?.endpoint).toBe('POST /api/v1/api-keys/{keyId}/rotate');
    expect(byId.get('webhooks.list')?.endpoint).toBe('GET /api/v1/webhooks/endpoints');
    expect(byId.get('sessions.list-active')?.endpoint).toBe('GET /api/v1/sessions');
    expect(byId.get('tenant.export')?.endpoint).toBe('GET /api/v1/tenants/{tenantId}/export');
    // The SCIM write path is NOT a console endpoint: deactivation names none.
    expect(byId.get('users.deactivate')?.endpoint).toBeNull();
    expect(byId.get('users.deactivate')?.note).toContain('SCIM push');
  });

  it('no never-granted action id appears in the catalog', () => {
    const neverGranted = ['dispatch.authorize', 'dispatch.issue', 'secret.read', 'network.any'];
    for (const action of MANAGEMENT_ACTIONS) {
      if (action.id !== 'roles.view-grants') {
        // The only permitted appearance is the roles display-only note
        // about the structural exclusion — never as an action id.
        expect(neverGranted).not.toContain(action.id);
      }
    }
  });

  it('users deactivation is recorded as the IdP write path, never a console action', () => {
    const deactivate = MANAGEMENT_ACTIONS.find((action) => action.id === 'users.deactivate');
    expect(deactivate?.endpoint).toBeNull();
    expect(deactivate?.note).toContain('NO console write path');
    expect(deactivate?.note).toContain('resolver');
  });
});

describe('the display-never-secret rule (record 08 §4)', () => {
  it('states fingerprints-never-secrets and the one-time reveal', () => {
    expect(DISPLAY_NEVER_SECRET_NOTE).toContain('FINGERPRINT');
    expect(DISPLAY_NEVER_SECRET_NOTE).toContain('exactly once');
    expect(DISPLAY_NEVER_SECRET_NOTE).toContain('never stored retrievable');
  });

  it('the secret-displaying actions name the one-time response reveal', () => {
    const create = MANAGEMENT_ACTIONS.find((action) => action.id === 'apikeys.create');
    expect(create?.note).toContain('exactly ONCE');
    const webhookCreate = MANAGEMENT_ACTIONS.find((action) => action.id === 'webhooks.create');
    expect(webhookCreate?.note).toContain('displayed ONCE');
    const webhookList = MANAGEMENT_ACTIONS.find((action) => action.id === 'webhooks.list');
    expect(webhookList?.note).toContain('FINGERPRINTS');
    expect(webhookList?.note).toContain('never in the database');
  });
});

describe('the honesty notes', () => {
  it('the section note states the no-fabrication discipline and that nothing was executed', () => {
    expect(ADMIN_MANAGEMENT_NOTE).toContain('specified-not-implemented');
    expect(ADMIN_MANAGEMENT_NOTE).toContain('No management action has ever been executed');
    expect(ADMIN_MANAGEMENT_NOTE).toContain('fingerprints, never secrets');
  });

  it('the constant renders verbatim as the specified-not-implemented status text', () => {
    expect(SPECIFIED_NOT_IMPLEMENTED).toBe('specified-not-implemented');
  });
});

describe('structural honesty (source scan — the P5.26 precedent)', () => {
  const SOURCE = readFileSync(
    join(dirname(fileURLToPath(import.meta.url)), 'adminManagement.ts'),
    'utf-8',
  );
  const CODE = SOURCE.split('\n')
    .filter((line) => !line.trim().startsWith('//'))
    .join('\n');

  it('the catalog module performs no fetch (it is pure data)', () => {
    expect(CODE).not.toMatch(/\bfetch\s*\(/);
    expect(CODE).not.toMatch(/XMLHttpRequest|sendBeacon/);
  });
});
