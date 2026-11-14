// P5.01 — tenant-context store tests (the §16.2 claims-derived discipline).
// UNRUN — human executes via `npm test` after install (decision record:
// 01_25_scaffold_live.md; the Phase 5 loop bans installs/builds/test runs).
import { afterEach, describe, expect, it } from 'vitest';
import {
  clearSession,
  getSession,
  setSession,
  subscribeToSession,
  type SessionClaims,
} from './tenantContext';

const CLAIMS: SessionClaims = {
  tenantId: '00000000-0000-0000-0000-000000000001',
  subject: 'user:operator-1',
  role: 'operator',
};

afterEach(() => {
  clearSession();
});

describe('the claims-derived tenant context store', () => {
  it('starts with no session', () => {
    expect(getSession()).toBeNull();
  });

  it('stores the session exactly as the verified claims arrived (no mutation)', () => {
    setSession(CLAIMS);
    const session = getSession();
    expect(session).toEqual(CLAIMS);
    // A later mutation of the caller's object must not leak into the store.
    const mutable = { ...CLAIMS };
    setSession(mutable);
    mutable.role = 'platform-admin';
    expect(getSession()?.role).toBe('operator');
  });

  it('clears the session (logout / expired authentication)', () => {
    setSession(CLAIMS);
    clearSession();
    expect(getSession()).toBeNull();
  });

  it('notifies subscribers on set and clear, and unsubscribes cleanly', () => {
    const events: string[] = [];
    const unsubscribe = subscribeToSession(() => {
      events.push(getSession() ? 'session' : 'cleared');
    });
    setSession(CLAIMS);
    clearSession();
    unsubscribe();
    // After unsubscribing the listener no longer fires.
    setSession(CLAIMS);
    expect(events).toEqual(['session', 'cleared']);
  });

  it('overwrites the previous session wholesale (a fresh authenticated session re-derives the full context)', () => {
    setSession(CLAIMS);
    setSession({ ...CLAIMS, tenantId: '00000000-0000-0000-0000-000000000002', role: 'viewer' });
    expect(getSession()?.tenantId).toBe('00000000-0000-0000-0000-000000000002');
    expect(getSession()?.role).toBe('viewer');
  });
});
