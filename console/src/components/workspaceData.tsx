// P5.02–P5.04 — the honest empty/error/loading states and the workspace
// data hook.
//
// §20.2 and the ledger non-negotiables: a workspace NEVER fills a missing
// datum with a plausible value, a placeholder number, or an invented
// status. What does not exist renders as an explicit empty state that says
// WHY it is empty (no contract / no data / API error) — the CONTEXT.md
// open-question-6 default position (empty states preferred; labelled demo
// data only if ever needed — no demo data exists in these workspaces, so
// no demo-mode flag is present anywhere: §21.4 labelling is moot without
// fixtures, and the decision record states this).
import { useEffect, useState } from 'react';
import type { JSX } from 'react';
import { ApiError, api, type ApiClient } from '../api/client';
import { getSession } from '../state/tenantContext';

/** Extracts the honest error text from a thrown value (never invented). */
export function errorText(error: unknown): string {
  if (error instanceof ApiError) {
    return `${error.code}: ${error.message}`;
  }
  if (error instanceof Error) {
    return `unreadable response: ${error.message}`;
  }
  return 'unreadable response';
}

/** The load state every workspace view renders from (tested directly). */
export type LoadState<T> =
  | { status: 'loading' }
  | { status: 'ready'; data: T; readAt: string }
  | { status: 'error'; error: string };

/**
 * Loads one workspace's data through the tenant-scoped API client. The
 * tenant context never travels in the request (§16.2): the server derives
 * it from claims; the console only carries the bearer credential.
 *
 * The loader runs once per mount; the hook does not poll (freshness is
 * shown per value via the §20.2 freshness line, and the live channel is
 * P5.25/P5.33's surface, not a re-fetch loop).
 */
export function useWorkspaceData<T>(load: (client: ApiClient) => Promise<T>): LoadState<T> {
  const [state, setState] = useState<LoadState<T>>({ status: 'loading' });
  useEffect(() => {
    let cancelled = false;
    setState({ status: 'loading' });
    load(api)
      .then((data) => {
        if (!cancelled) setState({ status: 'ready', data, readAt: new Date().toISOString() });
      })
      .catch((error: unknown) => {
        if (!cancelled) setState({ status: 'error', error: errorText(error) });
      });
    return () => {
      cancelled = true;
    };
    // The loader is a module-level pure function of the client; it takes
    // no reactive inputs, so the mount-only effect is intentional (no
    // exhaustive-deps suppression is used — the dependency array is
    // honestly empty).
  }, []);
  return state;
}

/** The honest loading line (no spinner-as-progress invented). */
export function LoadingLine({ what }: { what: string }): JSX.Element {
  return (
    <p role="status" data-testid="loading">
      Loading {what} from the control plane… nothing is displayed until the
      tenant-scoped read returns.
    </p>
  );
}

/** The honest error block: the API's own error text, verbatim. */
export function ErrorBlock({ what, error }: { what: string; error: string }): JSX.Element {
  return (
    <div role="alert" data-testid="error-block">
      <p>
        {what} could not be read. The console shows no data rather than stale
        or invented data.
      </p>
      <p>
        API error (verbatim): <code>{error}</code>
      </p>
      <p>
        If this is <code>forbidden</code>, your role does not permit this
        read — visibility is enforced by the API (§20.2/P5.26); hiding the
        section in the console is not the enforcement and this console does
        not pretend otherwise.
      </p>
    </div>
  );
}

export interface HonestEmptyProps {
  what: string;
  /** WHY nothing is shown (no contract / no records / not served yet). */
  reason: string;
  testId?: string;
}

/**
 * The honest empty state (§20.2; ledger P5.07 pattern applied to waves 2a):
 * names WHAT is absent and WHY — never a blank, never a fabricated row.
 */
export function HonestEmpty({ what, reason, testId = 'empty-state' }: HonestEmptyProps): JSX.Element {
  return (
    <div role="note" data-testid={testId}>
      <p>
        <strong>{what}</strong>
      </p>
      <p>{reason}</p>
    </div>
  );
}

/**
 * The role/scope line (§20.2 role-specific screens, P5.03 step 4): states
 * the claims-derived session identity and that visibility is API-enforced.
 * No session (dev bootstrap not wired until P5.26) is shown honestly too.
 */
export function RoleScopeLine(): JSX.Element {
  const session = getSession();
  if (!session) {
    return (
      <p className="role-scope" data-testid="role-scope" role="status">
        Role: no authenticated session (the dev-session bootstrap is wired at
        P5.26). Reads will fail honestly until authenticated; the role shown
        here always comes from authenticated claims, never from a URL or a
        selectable field (§16.2).
      </p>
    );
  }
  return (
    <p className="role-scope" data-testid="role-scope" role="status">
      Role: {session.role} (from authenticated claims; tenant {session.tenantId}).
      Visibility is enforced by the API per role and tenant (§20.2, P5.26) —
      this workspace renders only what the tenant-scoped reads return, and a
      forbidden read renders as an error, never as an empty dataset.
    </p>
  );
}
