// P5.26 — the console-side denial UI-event log (§16.2, §21.3).
//
// AUDIT BOUNDARY (recorded honestly, P5.26 step 4): denials are audited
// SERVER-side (§16.2; the P4.28 `TenantDenialAuditRecord` + `SecurityAuditSink`
// port in crates/flexcore-application/src/dispatch_security.rs; cross-tenant
// reads are 404 by design and the §16.2 denial logging stays on the P4.28
// seam — Phase 4 handover §5). The console does NOT log denials to any
// server endpoint: no such endpoint exists on the §15.2 surface, and none is
// invented here. This module records the denial UI events LOCALLY (in
// memory) only, so the operator/console diagnostics can explain what the
// user saw — it is never a substitute for the server audit trail, and it
// carries no claims data beyond the role id already displayed on screen.
//
// No secrets rule (P5.26): entries carry the action id, the role id, the
// reason text, and a timestamp — never a token, never a subject id beyond
// what the workspace already renders, never a request credential.

export interface ConsoleDenialEvent {
  /** The mirrored action id that was denied. */
  actionId: string;
  /** The role the check ran for ('(no session)' when unauthenticated). */
  role: string;
  /** The capability check's denial reason (the SAME text the UI rendered). */
  reason: string;
  /** Where in the console the denial was rendered (workspace/flow label). */
  surface: string;
  /** The UI-event instant (the console's clock, NOT an audit event time —
   * the P0.09 UTC discipline belongs to server records). */
  atIso: string;
}

/** The local ring-buffer cap (diagnostics only; bounded by design). */
export const DENIAL_LOG_CAPACITY = 100;

let log: ConsoleDenialEvent[] = [];

/**
 * Records ONE denial UI event locally. Never sent to the server (no such
 * endpoint; the server audit is P4.28's, §16.2).
 */
export function recordConsoleDenial(event: Omit<ConsoleDenialEvent, 'atIso'>): void {
  log = [...log, { ...event, atIso: new Date().toISOString() }].slice(-DENIAL_LOG_CAPACITY);
}

/** The current local denial UI events (a copy; diagnostics/tests only). */
export function getConsoleDenials(): ConsoleDenialEvent[] {
  return [...log];
}

/** Clears the local log (logout/diagnostics). */
export function clearConsoleDenials(): void {
  log = [];
}
