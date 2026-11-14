// P5.01 — the minimal claims-derived tenant/session context store.
//
// §16.2 / P5.26 pattern (which this module deliberately anticipates but
// does NOT implement): the console's tenant context comes from the
// AUTHENTICATED session's claims — never from a URL parameter, never from
// a request body field, never from user-selectable state. There is no
// "select a tenant" affordance in this store; switching tenants (where
// supported) re-derives the full context from a fresh authenticated
// session (P5.32 owns the switch guard).
//
// Session shape: Phase 1 ships only the dev-token identity resolver
// (dev-auth feature; auth.rs) whose token carries tenant/subject/role —
// the parsed claims below mirror exactly those fields. Production OIDC
// integration is a later-phase matter (recorded Phase 1 status); this
// store changes shape with it, not with any console invention.

export interface SessionClaims {
  /** The authenticated tenant (canonical UUID) — derived from claims. */
  tenantId: string;
  /** The authenticated subject (an identity reference, not personal data). */
  subject: string;
  /** The role id: platform-admin | tenant-admin | operator | viewer. */
  role: string;
}

interface TenantContextState {
  claims: SessionClaims | null;
}

let state: TenantContextState = { claims: null };
const listeners = new Set<() => void>();

/** The current claims-derived tenant context (null = no session). */
export function getSession(): SessionClaims | null {
  return state.claims;
}

/**
 * Sets the session from verified claims ONLY. Callers are the
 * authentication layer (P5.26's login flow / the dev-session bootstrap);
 * the store never parses a tenant out of a request field, URL, or free
 * text — a caller that has no verified claims must NOT call this.
 */
export function setSession(claims: SessionClaims): void {
  state = { claims: { ...claims } };
  notify();
}

/** Clears the session (logout / expired authentication). */
export function clearSession(): void {
  state = { claims: null };
  notify();
}

/** Subscribes to session changes; returns the unsubscribe function. */
export function subscribeToSession(listener: () => void): () => void {
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
