// P13.17 — session banner tests (WRITTEN-UNRUN; the human gate runs
// `npm test` after the first install). Follows the P5.26/P5.32 test
// conventions: renderToString, verbatim honesty-record assertions,
// data-testid targeting.
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, describe, expect, it } from 'vitest';
import { renderToString } from 'react-dom/server';
import { SessionBanner, SESSION_BANNER_NOTE } from './sessionBanner';
import {
  clearSessionLifecycle,
  setSessionLifecycle,
} from '../auth/session';
import { LOGIN_FLOW_NOT_IMPLEMENTED_NOTE } from '../auth/oidcLogin';
import type { SessionClaims } from '../state/tenantContext';

afterEach(() => {
  clearSessionLifecycle();
});

const CLAIMS: SessionClaims = {
  tenantId: '00000000-0000-0000-0000-000000000001',
  subject: 'user:operator-1',
  role: 'operator',
};

// The component reads Date.now() at mount; the render tests that touch
// the expiry states seed the lifecycle far from "now" so the buckets are
// stable regardless of when the human runs the suite.
const FAR_FUTURE = Date.now() + 6 * 60 * 60 * 1000; // 6 h out: active
const NEAR_PAST = Date.now() - 1_000; // already past: expired

describe('SessionBanner rendering (the honest states)', () => {
  it('with no session, renders the no-session state and the DISABLED sign-in control', () => {
    clearSessionLifecycle();
    const html = renderToString(<SessionBanner />);
    expect(html).toContain('session-banner-no-session');
    expect(html).toContain('session-banner-sign-in');
    expect(html).toContain('disabled');
    // The disabled control carries the specified-not-implemented note —
    // never a navigation to a nonexistent endpoint.
    expect(html).toContain('disabled —');
    expect(html).toContain(LOGIN_FLOW_NOT_IMPLEMENTED_NOTE.slice(0, 60));
  });

  it('the specified-not-implemented note states that no login was ever performed', () => {
    expect(LOGIN_FLOW_NOT_IMPLEMENTED_NOTE).toContain('No login has ever been performed');
  });

  it('with claims but no lifecycle facts, renders the expiry-unknown state (nothing invented)', () => {
    const html = renderToString(<SessionBanner />);
    expect(html).toContain('session-banner-lifecycle-unknown');
    expect(html).toContain('lifecycle facts are unknown');
    expect(html).toContain('specified-not-implemented');
    expect(html).toContain('no expiry is displayed and none is invented');
  });

  it('with lifecycle facts, renders the signed-in subject and the expiry line (active bucket)', () => {
    setSessionLifecycle({ sessionId: 'sess-abc', expiresAtUnixMillis: FAR_FUTURE });
    const html = renderToString(<SessionBanner />);
    expect(html).toContain('session-banner-lifecycle');
    expect(html).toContain(CLAIMS.subject);
    expect(html).toContain('session-banner-expiry-active');
    expect(html).toContain('Session expires in');
    expect(html).toContain('sess-abc');
    expect(html).toContain('X-FlexCore-Session');
  });

  it('a past deadline renders the expired alert state naming the resolver', () => {
    setSessionLifecycle({ sessionId: 'sess-old', expiresAtUnixMillis: NEAR_PAST });
    const html = renderToString(<SessionBanner />);
    expect(html).toContain('session-banner-expiry-expired');
    expect(html).toContain('Session expired');
    expect(html).toContain('denied at the resolver');
  });

  it('the clear-display control is described honestly: a display clear, never a logout', () => {
    setSessionLifecycle({ sessionId: 'sess-xyz', expiresAtUnixMillis: FAR_FUTURE });
    const html = renderToString(<SessionBanner />);
    expect(html).toContain('session-banner-clear-display');
    expect(html).toContain('Clear the local display');
    expect(html).toContain('real revocation is the server-side resolver act');
    expect(html).toContain('a cleared display is not a logout');
  });

  it('renders the honesty record verbatim (the enforcement boundary is ON the banner)', () => {
    const html = renderToString(<SessionBanner />);
    expect(html).toContain(SESSION_BANNER_NOTE.slice(0, 80));
    expect(SESSION_BANNER_NOTE).toContain('enforcement is the RESOLVER');
    expect(SESSION_BANNER_NOTE).toContain('never extend a session');
    expect(SESSION_BANNER_NOTE).toContain('absolute deadline only');
  });
});

describe('structural honesty (source scan — the P5.26 precedent)', () => {
  it('the banner module performs no fetch (all data comes from the two local stores)', () => {
    // Source scan, not runtime interception: the banner must not grow a
    // network path silently; the session read belongs to the
    // authentication-layer composition, not to a display component.
    const source = readFileSync(
      join(dirname(fileURLToPath(import.meta.url)), 'sessionBanner.tsx'),
      'utf-8',
    );
    const code = source
      .split('\n')
      .filter((line) => !line.trim().startsWith('//'))
      .join('\n');
    expect(code).not.toMatch(/\bfetch\s*\(/);
    expect(code).not.toMatch(/XMLHttpRequest|sendBeacon/);
    expect(code).not.toMatch(/window\.location|location\.assign|location\.replace/);
  });
});
