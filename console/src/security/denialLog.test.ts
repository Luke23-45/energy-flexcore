// P5.26 — the local denial-log tests (§16.2 audit boundary).
// UNRUN — never executed in the Phase 5 loop (no install/build/test;
// decision record: 13_26_honesty_security.md). The human executes these
// via `npm test` after install.
//
// THE AUDIT BOUNDARY IS THE POINT: these tests pin that the console
// denial log is a LOCAL, bounded, in-memory UI-event record — never a
// server submission (no such endpoint exists on the §15.2 surface, none
// invented) and never a substitute for the P4.28 server-side audit
// (TenantDenialAuditRecord + SecurityAuditSink). The no-secrets rule is
// pinned structurally: the record shape carries action id, role, reason,
// surface, timestamp — nothing else.
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, describe, expect, it } from 'vitest';
import {
  DENIAL_LOG_CAPACITY,
  clearConsoleDenials,
  getConsoleDenials,
  recordConsoleDenial,
} from './denialLog';

afterEach(() => {
  clearConsoleDenials();
});

describe('the local denial UI-event log (P5.26 step 4)', () => {
  it('records a denial with a UTC timestamp and returns a copy', () => {
    recordConsoleDenial({
      actionId: 'command.approve',
      role: 'operator',
      reason: 'holds no recorded grant',
      surface: 'Dispatch ApprovalFlow',
    });
    const events = getConsoleDenials();
    expect(events).toHaveLength(1);
    expect(events[0].actionId).toBe('command.approve');
    expect(events[0].role).toBe('operator');
    expect(events[0].atIso).toMatch(/^\d{4}-\d{2}-\d{2}T/);
    // A copy: mutating the returned array does not touch the log.
    events.pop();
    expect(getConsoleDenials()).toHaveLength(1);
  });

  it('is bounded by the recorded capacity (a ring buffer, not an audit trail)', () => {
    for (let i = 0; i < DENIAL_LOG_CAPACITY + 10; i += 1) {
      recordConsoleDenial({ actionId: `a${i}`, role: 'viewer', reason: 'r', surface: 's' });
    }
    expect(getConsoleDenials()).toHaveLength(DENIAL_LOG_CAPACITY);
    expect(getConsoleDenials()[0].actionId).toBe(`a${10}`);
  });

  it('clears on logout/diagnostics', () => {
    recordConsoleDenial({ actionId: 'a', role: 'viewer', reason: 'r', surface: 's' });
    clearConsoleDenials();
    expect(getConsoleDenials()).toEqual([]);
  });
});

describe('the audit boundary + no-secrets rule (structural, method: source scan)', () => {
  // METHOD: the module source with line comments stripped (the header
  // documentation legitimately NAMES the boundary — "never a token" — so
  // the scan runs over the CODE, not the prose). Recorded limitation: a
  // regex over code, not a tsc-level scan; the human's first `npm test`
  // run is the gate.
  const SOURCE = readFileSync(
    join(dirname(fileURLToPath(import.meta.url)), 'denialLog.ts'),
    'utf-8',
  );
  const CODE = SOURCE.split('\n')
    .filter((line) => !line.trim().startsWith('//'))
    .join('\n');

  it('the module NEVER sends the denial anywhere (no fetch/XHR/Request call in code)', () => {
    expect(CODE).not.toMatch(/\bfetch\s*\(/);
    expect(CODE).not.toMatch(/XMLHttpRequest/);
    expect(CODE).not.toMatch(/navigator\.sendBeacon/);
    expect(CODE).not.toMatch(/new\s+Request\s*\(/);
  });

  it('the module header cites the SERVER-side audit it does not duplicate (P4.28, §16.2)', () => {
    expect(SOURCE).toContain('TenantDenialAuditRecord');
    expect(SOURCE).toContain('SecurityAuditSink');
    expect(SOURCE).toContain('never a substitute for the server audit trail');
  });

  it('the record shape carries no token/credential field (no secrets rule)', () => {
    expect(CODE).not.toMatch(/token|credential|secret|password/i);
  });
});
