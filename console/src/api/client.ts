// P5.01 — the typed fetch client (the console's single HTTP surface).
import { getSession } from '../state/tenantContext';
//
// Contracts consumed (the §15.2 REST surface the console is allowed to
// read; there is NO execute-command endpoint on the server, §15.2, and the
// console adds none):
// - Phase 1/2 registry + telemetry (routes.rs / routes_phase2.rs)
// - Phase 3 flexibility/schedules/optimization-runs (routes_phase3.rs)
// - Phase 4 dispatch-commands / feasibility-checks / audit-events
//   (routes_phase4.rs, READ-only)
//
// Errors: the server's stable envelope {"error":{"code","message"}}
// (error.rs) maps onto ApiError verbatim — the console never invents an
// error taxonomy the API does not have.
//
// Tenant discipline (§16.2): the client sends credentials (Authorization)
// and NOTHING else that could select a tenant. The server derives the
// tenant from verified claims; a tenant parameter in any console request
// would be a spoofing surface, so none exists anywhere in this layer.
//
// Types: the DTO mirrors in types.ts carry drift-note comments; the
// authoritative shapes live in dto.rs / dto_phase3.rs / dto_phase4.rs and
// are pinned by the apps/control-plane contract tests. The generated
// client + OpenAPI drift test (P5.01 step 4) lands with the first install
// (it needs the toolchain; the hand-written mirrors here are its
// stand-in until then).

/** The stable API error (error.rs envelope, 1:1). */
export class ApiError extends Error {
  readonly status: number;
  /** The server's stable code (unauthenticated/forbidden/not_found/…). */
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
    this.code = code;
  }
}

/** The stable error envelope (error.rs). */
export interface ErrorEnvelope {
  error: { code: string; message: string };
}

export interface ApiClientConfig {
  /** Base URL; default '' (same origin — dev goes through the vite proxy). */
  baseUrl?: string;
  /** Bearer token provider (the authenticated session; may return null). */
  tokenProvider?: () => string | null | undefined;
  /** Injectable fetch for tests (default: global fetch). */
  fetchImpl?: typeof fetch;
}

export interface ApiClient {
  /** GETs a JSON document and unwraps it as T (no envelope assumption). */
  get<T>(path: string): Promise<T>;
  /** The raw request, exposed for the endpoint modules' specific shapes. */
  request<T>(path: string, init?: RequestInit): Promise<T>;
}

export function createApiClient(config: ApiClientConfig = {}): ApiClient {
  const baseUrl = config.baseUrl ?? '';
  const doFetch = config.fetchImpl ?? fetch.bind(globalThis);

  async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
    const headers = new Headers(init.headers);
    headers.set('accept', 'application/json');
    const token = config.tokenProvider?.();
    if (token) {
      headers.set('authorization', `Bearer ${token}`);
    }
    // Deliberately NO tenant header/field of any kind (§16.2): the server
    // derives tenancy from claims; the console cannot and must not select.
    const response = await doFetch(`${baseUrl}${path}`, { ...init, headers });
    if (!response.ok) {
      const envelope = (await safeParseJson(response)) as ErrorEnvelope | null;
      if (envelope?.error) {
        throw new ApiError(response.status, envelope.error.code, envelope.error.message);
      }
      // No parseable envelope: fail with the status, never with invented
      // server text (the opaque-infrastructure rule, error.rs).
      throw new ApiError(response.status, 'unparsed_error', response.statusText);
    }
    return (await response.json()) as T;
  }

  return {
    request,
    get<T>(path: string): Promise<T> {
      return request<T>(path, { method: 'GET' });
    },
  };
}

async function safeParseJson(response: Response): Promise<unknown> {
  try {
    return await response.json();
  } catch {
    return null;
  }
}

/** The default console client (session token from the state store). */
export const api = createApiClient({
  tokenProvider: () => {
    const session = getSession();
    return session ? devSessionToken(session) : null;
  },
});

/**
 * Renders the dev-session token. Phase 1 ships ONLY the dev-auth resolver
 * (auth.rs, feature-gated): token format
 * `Bearer dev tenant=<uuid> subject=<subject> role=<role>`. This is a
 * DEV-ONLY credential path (never a production credential, auth.rs); P5.26
 * replaces it with the real OIDC session. Recorded honestly: until P5.26,
 * the console authenticates only against a dev-auth control plane.
 */
export function devSessionToken(claims: {
  tenantId: string;
  subject: string;
  role: string;
}): string {
  return `dev tenant=${claims.tenantId} subject=${claims.subject} role=${claims.role}`;
}
