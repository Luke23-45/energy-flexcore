// P5.32 — the tenant-context display and tenant-switch guard tests
// (§16.2, §20.2).
// UNRUN — never executed in the Phase 5 loop (no install/build/test;
// decision record: 32_33_tenant_live_ordering.md). The human executes
// these via `npm test` after install.
//
// METHOD (stated per the task contract): the rendered assertions use
// react-dom/server `renderToString` (SSR — the recorded P5.26 convention;
// assertions never span an expression boundary); the switch-guard is a
// grep-based STRUCTURAL scan over the console's own non-test sources with
// full-line comments stripped (prose may NAME a banned construct — e.g.
// "no tenant selector" — so the scan runs over code). The same recorded
// limitation as every structural test in this console applies: a
// structural scan proves the wiring exists in source, not runtime
// behaviour; the human's first `npm test` run is the gate.
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, describe, expect, it } from 'vitest';
import { renderToString } from 'react-dom/server';
import {
  NO_TENANT_SWITCH_NOTE,
  TENANT_MISMATCH_CUE,
  TENANT_MISMATCH_STATEMENT,
  TENANT_SOURCE_NOTE,
  SINGLE_TENANT_NOTE,
  TenantBanner,
  TenantMismatchBadge,
  checkTenantScope,
  tenantActionGuard,
} from './tenantBanner';
import { clearSession, setSession, type SessionClaims } from '../state/tenantContext';

const SRC_DIR = join(dirname(fileURLToPath(import.meta.url)), '..');

/** All non-test .ts/.tsx sources under src/ (the scan corpus). */
function sourceFiles(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      out.push(...sourceFiles(full));
    } else if (/\.(ts|tsx)$/.test(entry) && !/\.test\.(ts|tsx)$/.test(entry)) {
      out.push(full);
    }
  }
  return out;
}

const relative = (file: string): string =>
  file.slice(SRC_DIR.length + 1).replace(/\\/g, '/');

/** The source with full-line comments removed (prose may NAME a banned
 * construct, so the scan runs over code). */
function codeOf(file: string): string {
  return readFileSync(file, 'utf-8')
    .split('\n')
    .filter((line) => !line.trim().startsWith('//'))
    .join('\n');
}

const files = sourceFiles(SRC_DIR);

const CLAIMS: SessionClaims = {
  tenantId: '00000000-0000-0000-0000-000000000001',
  subject: 'user:operator-1',
  role: 'operator',
};

afterEach(() => {
  clearSession();
});

describe('the persistent tenant banner (§16.2, §20.2)', () => {
  it('with a session, renders the claims-derived tenant id and role', () => {
    setSession(CLAIMS);
    const html = renderToString(<TenantBanner />);
    expect(html).toContain('data-testid="tenant-banner"');
    expect(html).toContain(CLAIMS.tenantId);
    expect(html).toContain('(role: ');
    expect(html).toContain(CLAIMS.role);
  });

  it('without a session, renders the explicit no-session state (never a blank line)', () => {
    const html = renderToString(<TenantBanner />);
    expect(html).toContain('data-testid="tenant-banner-no-session"');
    expect(html).toContain('no session');
  });

  it('renders the source note, the switch-guard note, and the single-tenant note', () => {
    const html = renderToString(<TenantBanner />);
    expect(html).toContain('authenticated session');
    // The notes are rendered from the exported constants (single source).
    for (const note of [TENANT_SOURCE_NOTE, NO_TENANT_SWITCH_NOTE, SINGLE_TENANT_NOTE]) {
      const probe = note.slice(0, 40);
      expect(html).toContain(probe);
    }
  });

  it('is rendered by the app shell (persistent, every workspace)', () => {
    const app = codeOf(join(SRC_DIR, 'App.tsx'));
    expect(app).toContain('TenantBanner');
    expect(app).not.toContain('className="tenant-indicator"'); // the old inline copy is gone
  });
});

describe('the wrong-tenant cue (§16.2: visible cue + blocked actions)', () => {
  it('a record tenant matching the context matches', () => {
    expect(checkTenantScope(CLAIMS.tenantId, CLAIMS.tenantId).status).toBe('match');
  });

  it('a record tenant differing from the context is a MISMATCH with the blocking statement', () => {
    const check = checkTenantScope('00000000-0000-0000-0000-000000000009', CLAIMS.tenantId);
    expect(check.status).toBe('mismatch');
    expect(check.statement).toBe(TENANT_MISMATCH_STATEMENT);
  });

  it('a record without a tenant_id is unverifiable — stated honestly, never faked', () => {
    expect(checkTenantScope(undefined, CLAIMS.tenantId).status).toBe('unverifiable');
    expect(checkTenantScope(CLAIMS.tenantId, undefined).status).toBe('unverifiable');
  });

  it('the action guard BLOCKS on mismatch and never blocks silently', () => {
    const blocked = tenantActionGuard('00000000-0000-0000-0000-000000000009', CLAIMS.tenantId);
    expect(blocked.allowed).toBe(false);
    expect(blocked.reason).toContain('Blocked');
    expect(blocked.reason).toContain('tenant');
  });

  it('the action guard allows match, and allows unverifiable WITH the enforcement caveat', () => {
    expect(tenantActionGuard(CLAIMS.tenantId, CLAIMS.tenantId).allowed).toBe(true);
    const unverifiable = tenantActionGuard(undefined, CLAIMS.tenantId);
    expect(unverifiable.allowed).toBe(true);
    expect(unverifiable.reason).toContain('server-side tenant scoping');
  });

  it('the mismatch badge renders the visible cue (text-borne, not colour alone)', () => {
    const html = renderToString(<TenantMismatchBadge recordTenantId="other-tenant" />);
    expect(html).toContain('data-testid="tenant-mismatch-badge"');
    expect(html).toContain(TENANT_MISMATCH_CUE);
  });
});

describe('the tenant-switch guard (structural — §16.2; ledger P5.32 step 2)', () => {
  it('no source contains a tenant-switch affordance identifier', () => {
    const violations: string[] = [];
    for (const file of files) {
      if (/selectTenant|switchTenant|setTenant|changeTenant|chooseTenant|TenantSelector|tenant-switch/i.test(codeOf(file))) {
        violations.push(relative(file));
      }
    }
    expect(violations).toEqual([]);
  });

  it('no source reads a tenant from the URL (URL params can never select a tenant, §16.2)', () => {
    const violations: string[] = [];
    for (const file of files) {
      const code = codeOf(file);
      if (/URLSearchParams\s*\([^)]*tenant/i.test(code) || /location\.[a-z]+[^;\n]*tenant|tenant[^;\n]*location\.(search|hash)/i.test(code)) {
        violations.push(relative(file));
      }
    }
    expect(violations).toEqual([]);
  });

  it('no workspace carries a tenant query/field assignment (the switch guard\'s own lens)', () => {
    // Also pinned by the P5.26 scan (tenantScopingStructural.test.ts);
    // re-asserted here because the guard owns it: a tenant written from
    // user input would BE an unoffered switch.
    const violations: string[] = [];
    for (const file of files) {
      if (!relative(file).startsWith('workspaces/')) continue;
      if (/tenant\s*=\s*[^=]/.test(codeOf(file))) {
        violations.push(relative(file));
      }
    }
    expect(violations).toEqual([]);
  });

  it('the session is written by the authentication layer ONLY (re-asserted for the guard)', () => {
    const violations: string[] = [];
    for (const file of files) {
      if (relative(file) === 'state/tenantContext.ts') continue;
      if (/\bsetSession\s*\(/.test(codeOf(file))) {
        violations.push(relative(file));
      }
    }
    expect(violations).toEqual([]);
  });
});
