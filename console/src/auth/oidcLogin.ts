// P13.17 — the real-login flow DESIGN (redirect/callback against the IdP;
// auth hardening, wave 2; WRITTEN-UNRUN — no login has ever been
// performed, no IdP exists, every runtime act is a human gate on the
// P13.20 lab environment).
//
// THE FLOW (the recorded design; consumes the P13.02 OIDC design — record
// 02 — and the P13.16 session semantics — record 42; both consumed BY
// CITATION, never redefined):
//
// 1. The console navigates the browser to the control plane's LOGIN
//    ENTRY (`GET /api/v1/auth/login?return_to=<console path>`) —
//    SPECIFIED-NOT-IMPLEMENTED (no such route exists; nothing here
//    performs the navigation against a live server).
// 2. The control plane (the confidential client per the record 01/02
//    architecture — the client secret never reaches the browser) builds
//    the OIDC authorize request (PKCE + `state` + `nonce` are the SERVER
//    side's construction; the console does not build, sign, or hold any
//    of it) and redirects to the per-tenant IdP connection.
// 3. The IdP authenticates the principal (the P13-E-3 DEFAULT:
//    IdP-delegated MFA) and redirects to the control plane's CALLBACK
//    (`/api/v1/auth/callback`) — also SPECIFIED-NOT-IMPLEMENTED.
// 4. The control plane validates the response, exchanges the code
//    server-side, maps claims through the P13.02 resolver vocabulary
//    (`iss`/`aud`/`exp`/groups → role — ONE closed vocabulary, record
//    02 §2), and CREATES THE SESSION (the P13.16
//    `SessionEnforcingResolver::create_session` semantics).
// 5. The control plane redirects the browser back to the console landing
//    path (the validated `return_to`; `buildLoginUrl` below refuses
//    anything that is not a same-origin path — the open-redirect guard
//    is console-side design; the SERVER re-validates independently, and
//    neither trusts the other's check).
// 6. The console reads the session facts via
//    `GET /api/v1/auth/session` — SPECIFIED-NOT-IMPLEMENTED — and the
//    authentication layer sets BOTH stores (claims → tenantContext,
//    lifecycle → auth/session). Until that endpoint exists, no store is
//    written and the console renders the honest no-session state.
//
// HONESTY: this module performs NO fetch and NO navigation (structurally:
// the tests scan the sources; the P5.26 precedent). The three endpoint
// constants below are the recorded design surface a deployment
// implements; until then the flow exists as design and tests only.

/** The login-entry path (SPECIFIED-NOT-IMPLEMENTED — no route exists). */
export const LOGIN_ENTRY_PATH = '/api/v1/auth/login';

/** The session-lifecycle read path (SPECIFIED-NOT-IMPLEMENTED — no route
 * exists; this is what would feed `setSessionLifecycle`). */
export const SESSION_READ_PATH = '/api/v1/auth/session';

/** The IdP-callback path, hosted by the CONTROL PLANE (not the console —
 * SPECIFIED-NOT-IMPLEMENTED; listed for the flow record's completeness). */
export const AUTH_CALLBACK_PATH = '/api/v1/auth/callback';

/**
 * The specified-not-implemented statement (rendered by the session banner
 * and quoted in the decision record).
 */
export const LOGIN_FLOW_NOT_IMPLEMENTED_NOTE =
  'Real login against the IdP is DESIGNED but NOT implemented: the login ' +
  'entry, IdP callback, and session-read endpoints of the P13.17 flow do ' +
  'not exist on the control plane yet (specified-not-implemented, plan ' +
  'risk 6). No login has ever been performed. The console neither builds ' +
  'authorize URLs nor holds client secrets — the confidential-client ' +
  'exchange is server-side by design (records 01/02).';

/**
 * Builds the console-side login-entry URL from a `return_to` landing
 * path. The guard is strict: ONLY same-origin absolute paths (leading
 * `/`, no `//` protocol-relative form, no scheme, no backslash form) are
 * accepted; everything else is a named rejection — an open-redirect
 * parameter is never rendered. The server re-validates independently;
 * this console-side guard is the display-layer half of the discipline.
 */
export function buildLoginUrl(baseUrl: string, returnTo: string): string {
  const guard = validateReturnTo(returnTo);
  if (!guard.ok) {
    throw new Error(guard.reason);
  }
  const trimmedBase = baseUrl.endsWith('/') ? baseUrl.slice(0, -1) : baseUrl;
  return `${trimmedBase}${LOGIN_ENTRY_PATH}?return_to=${encodeURIComponent(guard.path)}`;
}

export type ReturnToValidation =
  | { ok: true; path: string }
  | { ok: false; reason: string };

/**
 * The strict `return_to` validator (pure; exported for tests).
 * Accepted: a path starting with exactly one `/`, containing no scheme,
 * no `//`, no `\`, and no control characters. Rejected with a named
 * reason otherwise.
 */
export function validateReturnTo(returnTo: string): ReturnToValidation {
  if (returnTo.length === 0) {
    return { ok: false, reason: 'return_to must not be empty' };
  }
  if (!returnTo.startsWith('/')) {
    return {
      ok: false,
      reason: 'return_to must be a same-origin absolute path (leading /)',
    };
  }
  if (returnTo.startsWith('//')) {
    return {
      ok: false,
      reason: 'return_to must not be a protocol-relative URL (//)',
    };
  }
  if (returnTo.includes('\\')) {
    return { ok: false, reason: 'return_to must not contain a backslash' };
  }
  if (returnTo.includes('//')) {
    return {
      ok: false,
      reason: 'return_to must not contain an embedded //',
    };
  }
  if (/:/.test(returnTo)) {
    return {
      ok: false,
      reason: 'return_to must not contain a scheme (a colon)',
    };
  }
  // eslint-disable-next-line no-control-regex
  if (/[\u0000-\u001f\u007f]/.test(returnTo)) {
    return {
      ok: false,
      reason: 'return_to must not contain control characters',
    };
  }
  return { ok: true, path: returnTo };
}

/** The console-landing query contract (step 5–6 of the flow): the control
 * plane redirects back either bare (success; the console then performs
 * the session read) or with `login_error=<reason>` for the recorded
 * failure reasons. */
export type LoginLanding =
  | { status: 'success' }
  | { status: 'error'; reason: string }
  | { status: 'neutral' };

/**
 * The pure landing parser: reads ONLY the recorded `login_error` query
 * parameter (strict; unknown query content is ignored — the console
 * never derives state from unrecorded parameters). `search` is the raw
 * query string (with or without the leading `?`).
 */
export function parseLoginLanding(search: string): LoginLanding {
  const query = search.startsWith('?') ? search.slice(1) : search;
  if (query.length === 0) {
    return { status: 'success' };
  }
  for (const pair of query.split('&')) {
    const [name, value] = pair.split('=');
    if (name === 'login_error') {
      return {
        status: 'error',
        reason: decodeURIComponent(value ?? ''),
      };
    }
  }
  return { status: 'success' };
}
