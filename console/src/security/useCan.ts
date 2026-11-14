// P5.26 — the client-side capability gate hook (§17.2–§17.3, §20.2
// role-specific screens).
//
// `useCan(actionId)` derives the role from the CLAIMS-DERIVED session store
// (state/tenantContext.ts — never from user input, a URL parameter, or a
// request field) and evaluates it against the capability mirror
// (capabilityMatrix.ts) — deny-by-default, with the denial REASON rendered,
// never a bare allow/deny and never a broken button.
//
// What this hook is NOT (recorded honestly, P5.26 step 2):
// - NOT the enforcement. The API enforces authorization on every action
//   (P4.29 invariant-7 discipline applied to the UI); the console gate is
//   UX only. A forged console that shows the button still meets the
//   server's deny.
// - NOT reactive to session changes: the hook reads the store on render.
//   Session changes (setSession/clearSession) notify subscribers, but this
//   hook does not subscribe — a session swap re-renders the tree through
//   the app's own session handling. Recorded limitation, no invented
//   reactivity.
//
// Token storage honesty (P5.26, recorded): the session lives in the
// IN-MEMORY module store (state/tenantContext.ts) — never localStorage/
// sessionStorage. Tradeoff: an XSS in the console cannot read a PERSISTED
// token from storage (there is none to steal; the dev-session token is
// rebuilt from claims per request and lives only in closures), but a page
// refresh loses the session and re-authentication is required each load.
// Persistent, hardened token storage (BFF/httpOnly-cookie patterns) is a
// later-stage deployment requirement — NOT implemented in 0.x.
import { canAction, type CapabilityCheck, type ScopeKindId } from './capabilityMatrix';
import { getSession } from '../state/tenantContext';

/**
 * The client-side capability gate: `allowed` decides hide/disable; `reason`
 * is the denial explanation rendered where the action would have been.
 */
export function useCan(actionId: string, scopeKind?: ScopeKindId): CapabilityCheck {
  const session = getSession();
  return canAction(actionId, session ? session.role : null, scopeKind);
}

/**
 * Same evaluation with the full session (for flows that need the claims —
 * e.g. the step-up flow compares subject/tenant, not only the role).
 */
export function useCanWithSession(
  actionId: string,
  scopeKind?: ScopeKindId,
): CapabilityCheck & { subject: string | null; tenantId: string | null } {
  const session = getSession();
  const check = canAction(actionId, session ? session.role : null, scopeKind);
  return {
    ...check,
    subject: session ? session.subject : null,
    tenantId: session ? session.tenantId : null,
  };
}
