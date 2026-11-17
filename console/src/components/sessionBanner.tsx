// P13.17 — the session banner with expiry display (auth hardening, wave
// 2; WRITTEN-UNRUN — never executed; the human gate runs the console
// tests after the first install).
//
// WHAT THIS COMPONENT IS: the persistent display of the session's
// lifecycle facts (the P13.16 store, `auth/session.ts`) and of the
// real-login flow's status (`auth/oidcLogin.ts` design). It is DISPLAY
// ONLY — the load-bearing property it renders verbatim is P13.16's: a
// revoked or expired session DENIES AT THE RESOLVER; this banner can
// neither extend, revive, revoke, nor assert a session.
//
// WHAT IT IS NOT: a login implementation. The "Sign in" affordance is a
// DISABLED control with the specified-not-implemented note — the login
// entry / callback / session-read endpoints do not exist (plan risk 6),
// so no navigation to a nonexistent path is rendered, and no login has
// ever been performed. When a deployment implements the endpoints, the
// composition replaces the disabled control with the real one; the banner
// itself never fabricates the capability.
//
// Composition: rendered in the app shell (App.tsx, the additive P13.17
// marked edit) next to the P5.32 TenantBanner — the claims display stays
// in THAT component; this one adds the lifecycle facts only.
import { useEffect, useState } from 'react';
import type { JSX } from 'react';
import { getSession } from '../state/tenantContext';
import {
  clearSessionLifecycle,
  formatSessionExpiry,
  getSessionLifecycle,
  subscribeToSessionLifecycle,
} from '../auth/session';
import { LOGIN_FLOW_NOT_IMPLEMENTED_NOTE } from '../auth/oidcLogin';

/** The banner's honesty record (verbatim constant; rendered on it). */
export const SESSION_BANNER_NOTE =
  'Session display (P13.16/P13.17): the expiry line is DISPLAY data from ' +
  'the session-lifecycle store — enforcement is the RESOLVER (a revoked ' +
  'or expired session denies authentication on the next request ' +
  'regardless of this display). A stale client clock can misrender the ' +
  'countdown; it can never extend a session. The idle timeout can end a ' +
  'session earlier than the displayed deadline (the display shows the ' +
  'absolute deadline only — it cannot see idle activity on the server).';

/** The banner's poll cadence for the countdown. REGISTERED RECOMMENDATION
 * (X-8): 30 s — a display refresh, never a server poll contract. */
export const SESSION_BANNER_TICK_MILLIS = 30_000;

/**
 * The persistent session banner. Reads the claims store (the
 * authentication layer's) for the signed-in/out distinction and the
 * lifecycle store for the expiry display; subscribes to both the
 * lifecycle changes and the display tick.
 */
export function SessionBanner(): JSX.Element {
  const claims = getSession();
  const [lifecycle, setLifecycle] = useState(getSessionLifecycle());
  const [nowMillis, setNowMillis] = useState(() => Date.now());

  useEffect(() => {
    const unsubscribe = subscribeToSessionLifecycle(() => {
      setLifecycle(getSessionLifecycle());
    });
    return unsubscribe;
  }, []);

  useEffect(() => {
    const timer = window.setInterval(() => {
      setNowMillis(Date.now());
    }, SESSION_BANNER_TICK_MILLIS);
    return () => {
      window.clearInterval(timer);
    };
  }, []);

  return (
    <p className="session-banner" role="status" data-testid="session-banner">
      Session:{' '}
      {claims === null ? (
        <span data-testid="session-banner-no-session">
          no authenticated session.{' '}
          <button
            type="button"
            data-testid="session-banner-sign-in"
            disabled
            aria-disabled="true"
          >
            Sign in
          </button>{' '}
          <span data-testid="session-banner-login-note">
            (disabled — {LOGIN_FLOW_NOT_IMPLEMENTED_NOTE})
          </span>
        </span>
      ) : lifecycle === null ? (
        <span data-testid="session-banner-lifecycle-unknown">
          signed in as <strong>{claims.subject}</strong>, but the session
          lifecycle facts are unknown — no session-lifecycle endpoint serves
          them in this phase (specified-not-implemented), so no expiry is
          displayed and none is invented.
        </span>
      ) : (
        <span data-testid="session-banner-lifecycle">
          signed in as <strong>{claims.subject}</strong>.{' '}
          <span data-testid={`session-banner-expiry-${formatSessionExpiry(lifecycle.expiresAtUnixMillis, nowMillis).state}`}>
            {formatSessionExpiry(lifecycle.expiresAtUnixMillis, nowMillis).text}
          </span>{' '}
          <span data-testid="session-banner-session-ref">
            session reference <code>{lifecycle.sessionId}</code> (opaque;
            travels in the X-FlexCore-Session header).
          </span>{' '}
          <button
            type="button"
            data-testid="session-banner-clear-display"
            onClick={() => clearSessionLifecycle()}
          >
            Clear the local display
          </button>{' '}
          (clears the LOCAL lifecycle display only — real revocation is the
          server-side resolver act, P13.16; a cleared display is not a
          logout and must never be described as one).
        </span>
      )}{' '}
      {SESSION_BANNER_NOTE}
    </p>
  );
}
