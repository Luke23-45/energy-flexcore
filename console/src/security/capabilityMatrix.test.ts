// P5.26 — capability-mirror tests (§17.2–§17.3, P1.11 deny-by-default).
// UNRUN — never executed in the Phase 5 loop (no install/build/test;
// decision record: 13_26_honesty_security.md). The human executes these
// via `npm test` after install.
//
// WHAT THESE TESTS PIN: the mirror's deny-by-default behaviour and the
// recorded facts that must never silently change (command.approve's
// separation of duties, the union-drift entries, the forbidden §5.7
// names). They do NOT pin the whole matrix against the Rust sources —
// that would need a generated drift test (a later-stage matter; the
// recorded limitation in capabilityMatrix.ts and the decision record
// §2.3 applies). THE ENUM IS AUTHORITATIVE: crates/flexcore-authz/src/
// action.rs + the recorded grant sets win over this mirror on any drift.
import { describe, expect, it } from 'vitest';
import {
  CAPABILITY_MATRIX,
  CAPABILITY_MIRROR_NOTE,
  FORBIDDEN_ACTION_IDS,
  ROLES,
  canAction,
} from './capabilityMatrix';

describe('the capability mirror discipline (P5.26 step 2)', () => {
  it('the four roles are the closed policy.rs vocabulary, verbatim', () => {
    expect([...ROLES]).toEqual(['platform-admin', 'tenant-admin', 'operator', 'viewer']);
  });

  it('command.approve is granted ONLY to tenant-admin at site/asset scope (recorded separation of duties)', () => {
    const entry = CAPABILITY_MATRIX['command.approve'];
    expect(entry).toBeDefined();
    expect([...entry!.roles]).toEqual(['tenant-admin']);
    expect([...entry!.scopeKinds]).toEqual(['site', 'asset']);
    expect(entry!.driftNote).toBeTruthy();
  });

  it('the union-drift entries are recorded with a drift note, never narrowed', () => {
    for (const actionId of ['command.read', 'feasibility.read']) {
      const entry = CAPABILITY_MATRIX[actionId];
      expect(entry!.roles).toContain('platform-admin');
      expect(entry!.driftNote).toBeTruthy();
      expect(entry!.source).toContain('UNION');
    }
  });

  it('the §5.7-excluded and append-only-absent names never appear in the mirror', () => {
    for (const forbidden of FORBIDDEN_ACTION_IDS) {
      expect(CAPABILITY_MATRIX[forbidden]).toBeUndefined();
    }
    // And the enumerated forbidden list itself pins the recorded absences.
    expect([...FORBIDDEN_ACTION_IDS]).toContain('dispatch.authorize');
    expect([...FORBIDDEN_ACTION_IDS]).toContain('dispatch.issue');
    expect([...FORBIDDEN_ACTION_IDS]).toContain('audit.event.write');
  });

  it('live.stream.read is NOT a mirror key (the interim gate is telemetry.read, recorded)', () => {
    expect(CAPABILITY_MATRIX['live.stream.read']).toBeUndefined();
    expect(CAPABILITY_MATRIX['telemetry.read']!.driftNote).toContain('telemetry.read');
    expect(CAPABILITY_MATRIX['telemetry.read']!.driftNote).toContain('no such variant');
  });

  it('the mirror note states the authority order (THE ENUM IS AUTHORITATIVE; the API enforces)', () => {
    expect(CAPABILITY_MIRROR_NOTE).toContain('THE ENUM IS AUTHORITATIVE');
    expect(CAPABILITY_MIRROR_NOTE).toContain('Enforcement is ALWAYS the API');
    expect(CAPABILITY_MIRROR_NOTE).toContain('P4.28');
  });
});

describe('canAction (deny-by-default, P1.11 discipline in the mirror)', () => {
  it('an unknown action id denies with the P1.11 reason (never invents a grant)', () => {
    const check = canAction('dispatch.issue', 'platform-admin');
    expect(check.allowed).toBe(false);
    expect(check.reason).toContain('deny-by-default');
    expect(check.reason).toContain('UnknownAction');
  });

  it('a missing session denies and never fabricates a context (P0.16 rule 5)', () => {
    const check = canAction('command.read', null);
    expect(check.allowed).toBe(false);
    expect(check.reason).toContain('no authenticated session');
    expect(check.reason).toContain('never fabricated');
  });

  it('a role outside the closed vocabulary denies', () => {
    const check = canAction('command.read', 'super-admin');
    expect(check.allowed).toBe(false);
    expect(check.reason).toContain('outside the closed four-role vocabulary');
  });

  it('a role without a recorded grant denies WITH the recorded grants and the mirror note', () => {
    const check = canAction('command.approve', 'operator');
    expect(check.allowed).toBe(false);
    expect(check.reason).toContain("holds no recorded grant for 'command.approve'");
    expect(check.reason).toContain('tenant-admin');
    expect(check.reason).toContain('THE ENUM IS AUTHORITATIVE');
  });

  it('the recorded grant allows (with the API-enforces caveat in the reason)', () => {
    const check = canAction('command.read', 'operator');
    expect(check.allowed).toBe(true);
    expect(check.reason).toContain('The API enforces this independently');
  });

  it('command.approve at tenant scope denies (no tenant-level approval grant)', () => {
    const check = canAction('command.approve', 'tenant-admin', 'tenant');
    expect(check.allowed).toBe(false);
    expect(check.reason).toContain("only at scope kind(s) site, asset");
  });

  it('command.approve at asset scope allows for tenant-admin (the recorded approval scope)', () => {
    const check = canAction('command.approve', 'tenant-admin', 'asset');
    expect(check.allowed).toBe(true);
  });
});
