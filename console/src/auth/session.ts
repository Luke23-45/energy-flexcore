// P13.17 — the console session-lifecycle store (auth hardening, wave 2;
// WRITTEN-UNRUN — never executed; the console loop bans install/build/test
// and the human gate runs these).
//
// WHAT THIS STORE IS: the SESSION-LIFECYCLE facts the P13.16 design
// attaches to an authenticated session — the session reference
// (`X-FlexCore-Session`) and the expiry instant. It deliberately does NOT
// duplicate the claims: tenant/subject/role stay in
// `state/tenantContext.ts` (the P5.01 store, untouched). The two stores
// are set together by the authentication layer (the login flow,
// `oidcLogin.ts` design); nothing else may set either (the P5.26
// structural discipline: the session changes only through the
// authentication layer, never through a workspace or a user selection).
//
// THE ENFORCEMENT BOUNDARY (P13.16, structural): this store is DISPLAY
// data. A revoked or expired session denies at the RESOLVER on the next
// request; the console can neither extend, revive, nor assert a session —
// it renders the expiry and the denials it observes, nothing more.
//
// Until a deployment composes the login flow, NOTHING sets this store:
// the honest default is "no lifecycle facts" and the banner renders that
// state verbatim. The lifecycle read endpoint it would consume
// (`GET /api/v1/auth/session`) is SPECIFIED-NOT-IMPLEMENTED (the P13.17
// decision record, plan risk 6 discipline) — no fetch exists in this
// module, and none may be added without the endpoint existing.

/**
 * The session-lifecycle facts of ONE authenticated session (display data
 * only — enforcement is the resolver, P13.16).
 */
export interface ConsoleSessionLifecycle {
  /** The opaque session reference (the `X-FlexCore-Session` value). */
  sessionId: string;
  /** The resolver-side expiry instant (UTC millis; the P13.16 absolute
   * deadline — the idle timeout can end the session EARLIER, which this
   * display cannot predict and never claims to). */
  expiresAtUnixMillis: number;
}

interface SessionLifecycleState {
  lifecycle: ConsoleSessionLifecycle | null;
}

let state: SessionLifecycleState = { lifecycle: null };
const listeners = new Set<() => void>();

/** The current session-lifecycle facts (null = none established). */
export function getSessionLifecycle(): ConsoleSessionLifecycle | null {
  return state.lifecycle;
}

/**
 * Sets the lifecycle facts from the authentication layer ONLY (the login
 * flow's session-read step). Callers that have not received the facts
 * from the server's session-lifecycle response must NOT call this —
 * a lifecycle the console invents would be a fabricated credential
 * display (the P0.16 rule-5 discipline, applied to display data).
 */
export function setSessionLifecycle(lifecycle: ConsoleSessionLifecycle): void {
  state = { lifecycle: { ...lifecycle } };
  notify();
}

/** Clears the lifecycle facts (logout landing / expired-session landing). */
export function clearSessionLifecycle(): void {
  state = { lifecycle: null };
  notify();
}

/** Subscribes to lifecycle changes; returns the unsubscribe function. */
export function subscribeToSessionLifecycle(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function notify(): void {
  for (const listener of listeners) {
    listener();
  }
}

// --- The expiry display (pure; the banner consumes it) ---------------------------

/** The expiring-soon threshold. REGISTERED RECOMMENDATION (X-8): a session
 * within 5 minutes of its expiry renders the expiring-soon cue — a display
 * threshold, never an enforcement act. */
export const EXPIRING_SOON_WINDOW_MILLIS = 5 * 60 * 1000;

export type SessionExpiryState = 'active' | 'expiring-soon' | 'expired';

export interface SessionExpiryDisplay {
  state: SessionExpiryState;
  /** The rendered line (text-borne, not colour alone — the P5.12 rule). */
  text: string;
}

/**
 * The pure expiry display: remaining time bucketed into active /
 * expiring-soon / expired. `nowMillis` is injected (tests pin the
 * boundaries; the component passes `Date.now()` for display only — a
 * stale client clock can misrender the countdown but can never extend a
 * session, because the resolver, not this display, decides).
 */
export function formatSessionExpiry(
  expiresAtUnixMillis: number,
  nowMillis: number,
): SessionExpiryDisplay {
  const remaining = expiresAtUnixMillis - nowMillis;
  if (remaining <= 0) {
    return {
      state: 'expired',
      text: 'Session expired — the next request will be denied at the resolver; re-authentication is required.',
    };
  }
  const minutes = Math.floor(remaining / 60000);
  const seconds = Math.floor((remaining % 60000) / 1000);
  if (remaining <= EXPIRING_SOON_WINDOW_MILLIS) {
    return {
      state: 'expiring-soon',
      text: `Session expires in ${minutes}m ${String(seconds).padStart(2, '0')}s — re-authenticate to continue (the resolver enforces the deadline regardless of this display).`,
    };
  }
  return {
    state: 'active',
    text: `Session expires in ${minutes}m ${String(seconds).padStart(2, '0')}s.`,
  };
}
