// P5.01 — minimal hand-rolled hash router.
//
// RECORDED DECISION (decision record 01_25_scaffold_live.md, DEFAULT): the
// report (§7.5) recommends "TanStack Router and Query or the recorded
// equivalent", but RESEARCH.md verified NO version for TanStack Router or
// React Router. Per the no-hallucinated-versions rule, the scaffold adds
// NO unpinned routing dependency: a ~60-line hash router covers the twelve
// §20.1 workspace routes. Revisit trigger: wave 2a/2b when a workspace
// needs nested routes, deep links with parameters, or scroll restoration —
// at that point TanStack Router (or the recorded equivalent) is verified
// against a primary source, pinned, and swapped in, and this module is
// deleted. Hash routing (not history routing) keeps the static-bundle
// deployment form (§21.4) serverless-config-free.
import { useEffect, useState } from 'react';
import { WORKSPACES, type WorkspaceId } from './workspaces/registry';

export const DEFAULT_ROUTE: WorkspaceId = 'overview';

/** Extracts the workspace id from `location.hash` (`#/grid` -> "grid"). */
export function parseRoute(hash: string): WorkspaceId {
  const raw = hash.replace(/^#\/?/, '').split(/[?#]/)[0];
  const workspace = WORKSPACES.find((w) => w.id === raw);
  return workspace ? workspace.id : DEFAULT_ROUTE;
}

/** Canonical hash for a workspace id. */
export function routeHash(id: WorkspaceId): string {
  return `#/${id}`;
}

/**
 * Subscribes to hash changes; returns the initial route and an
 * unsubscribe. Framework-free so the router stays testable without a DOM.
 */
export function subscribeToRoute(
  onChange: (route: WorkspaceId) => void,
): () => void {
  const listener = () => onChange(parseRoute(window.location.hash));
  window.addEventListener('hashchange', listener);
  return () => window.removeEventListener('hashchange', listener);
}

/** React binding: the current workspace id, updated on hashchange. */
export function useHashRoute(): WorkspaceId {
  const [route, setRoute] = useState<WorkspaceId>(() =>
    typeof window === 'undefined' ? DEFAULT_ROUTE : parseRoute(window.location.hash),
  );
  useEffect(() => subscribeToRoute(setRoute), []);
  return route;
}

/** Navigates to a workspace (hash routing: assign the hash). */
export function navigate(id: WorkspaceId): void {
  window.location.hash = routeHash(id);
}
