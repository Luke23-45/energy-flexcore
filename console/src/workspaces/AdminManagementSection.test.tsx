// P13.18 — the management section rendering tests (WRITTEN-UNRUN; the
// human gate runs `npm test` after the first install). Follows the
// renderToString + structural-scan conventions of the P5.26/P5.32 tests.
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { renderToString } from 'react-dom/server';
import { AdminManagementSection } from './AdminManagementSection';
import { ADMIN_MANAGEMENT_NOTE } from './adminManagement';
import { App } from '../App';

describe('AdminManagementSection rendering', () => {
  const html = renderToString(<AdminManagementSection />);

  it('renders the section with its honesty note verbatim', () => {
    expect(html).toContain('admin-management-section');
    expect(html).toContain('admin-mgmt-note');
    expect(html).toContain(ADMIN_MANAGEMENT_NOTE.slice(0, 80));
  });

  it('renders the five-role vocabulary including the P13.05 site-approver', () => {
    expect(html).toContain('admin-mgmt-roles');
    expect(html).toContain('site-approver');
    expect(html).toContain('platform-admin');
    expect(html).toContain('THE ENUM IS AUTHORITATIVE');
  });

  it('renders the grant-table mirror with the record 05 semantics', () => {
    expect(html).toContain('admin-mgmt-grant-table');
    expect(html).toContain('command.approve');
    expect(html).toContain('apikey.rotate');
    expect(html).toContain('site-approver');
    expect(html).toContain('admin-mgmt-absent-grants');
    expect(html).toContain('never-granted permission set');
  });

  it('renders the display-never-secret rule', () => {
    expect(html).toContain('admin-mgmt-display-never-secret');
    expect(html).toContain('FINGERPRINT');
  });

  it('renders every surface catalog with its specified-not-implemented statuses', () => {
    for (const surface of ['users', 'roles', 'api-keys', 'webhooks', 'sessions', 'policy', 'tenant-data']) {
      expect(html).toContain(`admin-mgmt-surface-${surface}`);
    }
    expect(html).toContain('specified-not-implemented');
    expect(html).toContain('admin-mgmt-action-apikeys.create');
    expect(html).toContain('admin-mgmt-action-sessions.revoke');
    expect(html).toContain('admin-mgmt-action-tenant.export');
  });

  it('renders endpoint paths as text (<code>), never as links or buttons', () => {
    expect(html).toContain('<code>GET /api/v1/api-keys</code>');
    // No anchor tags point at the management endpoints (no navigation to
    // nonexistent paths), and no submit/button controls exist at all.
    expect(html).not.toContain('href="/api/v1/');
    expect(html).not.toContain('<button');
    expect(html).not.toContain('type="submit"');
  });
});

describe('composition (the P7.25 precedent, structural source scan)', () => {
  const APP_SOURCE = readFileSync(
    join(dirname(fileURLToPath(import.meta.url)), '..', 'App.tsx'),
    'utf-8',
  );

  it('App.tsx composes the section BELOW the unchanged P5.07 workspace on the administration route', () => {
    expect(APP_SOURCE).toContain('<AdministrationWorkspace />');
    expect(APP_SOURCE).toContain('<AdminManagementSection />');
    const administration = APP_SOURCE.indexOf('<AdministrationWorkspace />');
    const management = APP_SOURCE.indexOf('<AdminManagementSection />');
    expect(management).toBeGreaterThan(administration);
    // The route entry points at the composed wrapper, not at either piece alone.
    expect(APP_SOURCE).toContain('administration: AdministrationWithManagement');
  });

  it('Administration.tsx (the P5.07 file) was NOT modified by this wave', () => {
    const admin = readFileSync(
      join(dirname(fileURLToPath(import.meta.url)), 'Administration.tsx'),
      'utf-8',
    );
    expect(admin).not.toContain('P13.18');
    expect(admin).not.toContain('AdminManagementSection');
  });

  it('the section module performs no fetch and renders no control with a handler', () => {
    const source = readFileSync(
      join(dirname(fileURLToPath(import.meta.url)), 'AdminManagementSection.tsx'),
      'utf-8',
    );
    const code = source
      .split('\n')
      .filter((line) => !line.trim().startsWith('//'))
      .join('\n');
    expect(code).not.toMatch(/\bfetch\s*\(/);
    expect(code).not.toMatch(/XMLHttpRequest|sendBeacon/);
    expect(code).not.toMatch(/onClick|onSubmit|onChange/);
  });
});

describe('the App shell composition (the P13.17 banner, structural)', () => {
  it('the session banner renders in the shell next to the tenant banner', () => {
    const html = renderToString(<App />);
    expect(html).toContain('tenant-banner');
    expect(html).toContain('session-banner');
  });
});
