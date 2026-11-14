// P5.26 — the tenant-scoping structural tests (§16.2: tenant context comes
// from authenticated claims, never from user input).
// UNRUN — never executed in the Phase 5 loop (no install/build/test;
// decision record: 13_26_honesty_security.md). The human executes these
// via `npm test` after install.
//
// METHOD (stated per the task contract — grep-based STRUCTURAL assertions,
// acceptable and recorded): the test reads the console's own non-test
// sources with node:fs and asserts, structurally, that
//
//   1. no source persists anything to browser storage (localStorage /
//      sessionStorage / document.cookie) — the token-storage honesty rule
//      (the session lives in the in-memory module store only);
//   2. no workspace, api, live, or security module ever WRITES the session
//      (setSession is reserved to the authentication layer, which does not
//      exist in the console scaffold yet — the store is set only by the
//      Phase 1 auth bootstrap when it lands);
//   3. no workspace builds an authenticated request itself or imports the
//      token renderer (devSessionToken stays inside api/client.ts and the
//      live hook) — so no workspace can attach a credential or a tenant
//      selector to a request;
//   4. no workspace source contains a tenant query/field assignment
//      (`tenant=`) — the api client layer's recorded discipline (client.ts
//      header: "the client sends credentials and NOTHING else that could
//      select a tenant") extends to every display surface.
//
// WHAT A STRUCTURAL SCAN CANNOT PROVE (recorded honestly): it proves the
// wiring exists in source, not runtime behaviour. The runtime cross-tenant
// denial proof is SERVER-side (the API tests: cross-tenant reads are 404,
// audited per §16.2/P4.28) — the ledger's exit check says exactly that,
// and the console cannot prove the server's behaviour from the client.
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

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
 * construct — e.g. "never localStorage" — so the scan runs over code). */
function codeOf(file: string): string {
  return readFileSync(file, 'utf-8')
    .split('\n')
    .filter((line) => !line.trim().startsWith('//'))
    .join('\n');
}

const files = sourceFiles(SRC_DIR);

describe('no persisted token anywhere (token-storage honesty, P5.26)', () => {
  it('no source touches localStorage / sessionStorage / document.cookie', () => {
    const violations: string[] = [];
    for (const file of files) {
      const code = codeOf(file);
      if (/localStorage|sessionStorage|document\.cookie/.test(code)) {
        violations.push(relative(file));
      }
    }
    expect(violations).toEqual([]);
  });
});

describe('the session is written by the authentication layer ONLY (§16.2)', () => {
  it('no module outside state/tenantContext.ts calls setSession', () => {
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

describe('no workspace builds a request or a credential (§16.2 discipline)', () => {
  it('devSessionToken is imported by nothing outside api/client.ts and live/useLiveEvents.ts', () => {
    const ALLOWED = ['api/client.ts', 'live/useLiveEvents.ts'];
    const violations: string[] = [];
    for (const file of files) {
      if (ALLOWED.includes(relative(file))) continue;
      if (/devSessionToken/.test(codeOf(file))) {
        violations.push(relative(file));
      }
    }
    expect(violations).toEqual([]);
  });

  it('no workspace contains a tenant query/field assignment ("tenant=")', () => {
    const violations: string[] = [];
    for (const file of files) {
      if (!relative(file).startsWith('workspaces/')) continue;
      if (/tenant\s*=\s*[^=]/.test(codeOf(file))) {
        violations.push(relative(file));
      }
    }
    expect(violations).toEqual([]);
  });
});
