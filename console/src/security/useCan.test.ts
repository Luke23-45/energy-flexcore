// P5.26 — the useCan capability-gate tests (§17.2–§17.3, §20.2).
// UNRUN — never executed in the Phase 5 loop (no install/build/test;
// decision record: 13_26_honesty_security.md). The human executes these
// via `npm test` after install.
//
// METHOD NOTE: useCan reads the session store on call and uses no React
// hook primitives (no useState/useEffect/subscription), so the tests call
// it DIRECTLY — the same value it returns during a component render.
import { afterEach, describe, expect, it } from 'vitest';
import {
  clearSession,
  setSession,
  type SessionClaims,
} from '../state/tenantContext';
import { useCan, useCanWithSession } from './useCan';

const OPERATOR: SessionClaims = {
  tenantId: '00000000-0000-0000-0000-000000000001',
  subject: 'user:operator-1',
  role: 'operator',
};
const TENANT_ADMIN: SessionClaims = { ...OPERATOR, role: 'tenant-admin' };

afterEach(() => {
  clearSession();
});

describe('useCan — the role comes from the claims-derived session store', () => {
  it('with no session, every action denies (a context is never fabricated)', () => {
    const check = useCan('command.read');
    expect(check.allowed).toBe(false);
    expect(check.role).toBeNull();
    expect(check.reason).toContain('no authenticated session');
  });

  it('with an operator session, command.approve denies with the recorded reason (§17.3 separation of duties)', () => {
    setSession(OPERATOR);
    const check = useCan('command.approve');
    expect(check.allowed).toBe(false);
    expect(check.role).toBe('operator');
    expect(check.reason).toContain('holds no recorded grant for');
    expect(check.reason).toContain('THE ENUM IS AUTHORITATIVE');
  });

  it('with a tenant-admin session, command.approve allows (the API still enforces independently)', () => {
    setSession(TENANT_ADMIN);
    const check = useCan('command.approve', 'asset');
    expect(check.allowed).toBe(true);
    expect(check.reason).toContain('The API enforces this independently');
  });

  it('an unknown action denies for ANY session (deny-by-default)', () => {
    setSession(OPERATOR);
    const check = useCan('command.auto_approve');
    expect(check.allowed).toBe(false);
    expect(check.reason).toContain('deny-by-default');
  });
});

describe('useCanWithSession — the claims for the step-up comparison', () => {
  it('returns the session claims alongside the capability check', () => {
    setSession(OPERATOR);
    const result = useCanWithSession('command.read');
    expect(result.allowed).toBe(true);
    expect(result.subject).toBe('user:operator-1');
    expect(result.tenantId).toBe('00000000-0000-0000-0000-000000000001');
  });

  it('returns null claims fields with no session', () => {
    const result = useCanWithSession('command.read');
    expect(result.allowed).toBe(false);
    expect(result.subject).toBeNull();
    expect(result.tenantId).toBeNull();
  });
});
