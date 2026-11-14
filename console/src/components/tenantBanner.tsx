// P5.32 — the persistent tenant-context display and the tenant-switch
// guard (§16.2, §20.2; ledger P5.32).
//
// The rules this module encodes:
//
// - The tenant indicator is populated from the AUTHENTICATED session's
//   claims ONLY (state/tenantContext.ts) — never from a URL parameter, a
//   request field, or user-selectable state (§16.2; the P5.26 structural
//   scans pin that no module writes the session and no workspace carries a
//   tenant selector). It renders in the persistent shell (App.tsx), so it
//   is visible on EVERY workspace, alongside the control-authority mode
//   indicator.
// - SINGLE-TENANT case (ledger P5.32 step 4): the indicator shows even
//   where the deployment is single-tenant — it is the PROOF that the
//   context is what the operator thinks it is, not a multi-tenant feature.
// - TENANT SWITCHING (ledger P5.32 step 2): this console does NOT offer
//   tenant switching — there is no tenant selector on the wire (verified
//   against P5.26/P5.07; the structural test in tenantBanner.test.tsx pins
//   that no source contains a switch/select-tenant affordance). The guard
//   is therefore the STRUCTURAL guarantee: the only path a session changes
//   by is setSession from the authentication layer, which re-derives the
//   FULL context (claims, role) wholesale — a partially re-derived context
//   cannot exist by construction (tenantContext.ts overwrites the whole
//   state object in one assignment). If a future console ever offers a
//   switch, it must go through a fresh authenticated session (explicit
//   confirmation + full re-validation) and a failed re-derivation must
//   leave the previous tenant's data visible with a stale-context warning
//   — never silently mixed data. That future switch UI does NOT exist
//   here, and none is invented.
// - WRONG-TENANT CUE: registry/command/alarm records carry their own
//   `tenant_id` (verified on the server DTOs: dto.rs PortfolioDto/SiteDto/
//   AssetDto, dto_phase3/dto_phase4 records, dto_alarms.rs AlarmRecord).
//   Where a displayed record carries a tenant_id, it is contrasted with
//   the context tenant: a mismatch renders the visible MISMATCH cue and
//   BLOCKS actions (tenantActionGuard below). The per-record badge is
//   exported for the record tables; the twelve workspaces were READ-ONLY
//   files in this pass, so wiring the badge into each table row is
//   recorded in the decision record (32_33_tenant_live_ordering.md) as the
//   defined integration point — the helper and its tests exist; the
//   server's fail-closed tenant scoping (404 on cross-tenant reads, P4.28
//   audit) remains the enforcement, as everywhere on this console.
import type { JSX } from 'react';
import { getSession } from '../state/tenantContext';

/**
 * Why the banner shows what it shows (rendered under the indicator — the
 * source is on screen, not tribal knowledge, §20.2).
 */
export const TENANT_SOURCE_NOTE =
  'Tenant and role are read from the authenticated session\'s claims only ' +
  '(§16.2) — never from a URL parameter, a request field, or a user ' +
  'selection. This console has NO tenant selector: the server derives ' +
  'tenancy from the authenticated claims on every request, and cross-tenant ' +
  'reads are denied there (404) and audited (P4.28) — the banner is the ' +
  'display of that context, not its enforcement.';

/** The single-tenant honesty note (ledger P5.32 step 4). */
export const SINGLE_TENANT_NOTE =
  'Even on a single-tenant deployment this indicator shows: it is the proof ' +
  'that the tenant context is what the operator thinks it is.';

/** The switch-guard statement (ledger P5.32 step 2; the structural test
 * pins that no switch affordance exists anywhere in the sources). */
export const NO_TENANT_SWITCH_NOTE =
  'Tenant switching is not offered on this console. A tenant context can ' +
  'only change with a fresh authenticated session, which re-derives the ' +
  'full context (claims, role, scopes) wholesale before any data reloads — ' +
  'a switch that fails to re-derive context would leave the previous ' +
  'tenant\'s data visible with a stale-context warning, never silently ' +
  'mixed data. No switch UI exists in this phase and none is invented.';

/** The visible wrong-tenant cue (ledger P5.32 step 1's "visible cue"). */
export const TENANT_MISMATCH_CUE = '[TENANT MISMATCH — actions blocked]';

export const TENANT_MISMATCH_STATEMENT =
  'This record\'s tenant_id differs from the authenticated session\'s ' +
  'tenant. The console blocks actions against it (tenantActionGuard) and ' +
  'the server denies the read/write independently (fail-closed tenant ' +
  'scoping, §16.2); a mismatch here means a display contract broke and ' +
  'must be treated as a defect, never click-through.';

export const TENANT_UNVERIFIABLE_STATEMENT =
  'This record carries no tenant_id to compare against the session tenant. ' +
  'No mismatch can be shown; the server-side tenant scoping remains the ' +
  'enforcement (§16.2).';

export type TenantScopeStatus = 'match' | 'mismatch' | 'unverifiable';

export interface TenantScopeCheck {
  status: TenantScopeStatus;
  /** The honest statement for the status (rendered where the check is used). */
  statement: string;
}

/**
 * Contrasts ONE displayed record's tenant_id with the context tenant.
 * A record without a tenant_id is `unverifiable` — the console states that
 * honestly rather than pretending the comparison happened.
 */
export function checkTenantScope(
  recordTenantId: string | null | undefined,
  contextTenantId: string | null | undefined,
): TenantScopeCheck {
  if (!contextTenantId || !recordTenantId) {
    return { status: 'unverifiable', statement: TENANT_UNVERIFIABLE_STATEMENT };
  }
  if (recordTenantId === contextTenantId) {
    return { status: 'match', statement: 'record tenant matches the session tenant.' };
  }
  return { status: 'mismatch', statement: TENANT_MISMATCH_STATEMENT };
}

export interface TenantActionGuardResult {
  /** Whether console-side actions (approval, export, …) may proceed. */
  allowed: boolean;
  /** The reason, rendered where an action would have been (the P5.26
   * denial-reason discipline — never a bare block). */
  reason: string;
}

/**
 * The action-blocking half of the wrong-tenant cue: on a MISMATCH actions
 * are blocked; on `unverifiable` the action is allowed but the reason
 * states the server-side enforcement (blocking every record that lacks a
 * tenant_id would fabricate a client-side authority the console does not
 * have — the API enforces tenant scoping on every request, §16.2/P5.26).
 */
export function tenantActionGuard(
  recordTenantId: string | null | undefined,
  contextTenantId: string | null | undefined,
): TenantActionGuardResult {
  const check = checkTenantScope(recordTenantId, contextTenantId);
  if (check.status === 'mismatch') {
    return { allowed: false, reason: `Blocked: ${check.statement}` };
  }
  return {
    allowed: true,
    reason:
      check.status === 'unverifiable'
        ? `Allowed with caveat: ${check.statement}`
        : check.statement,
  };
}

/**
 * The wrong-tenant MISMATCH badge (render next to any record whose
 * tenant_id is contrasted with the context tenant). Text-borne, not colour
 * alone (P5.12 discipline).
 */
export function TenantMismatchBadge({ recordTenantId }: { recordTenantId: string }): JSX.Element {
  return (
    <span data-testid="tenant-mismatch-badge" role="alert" data-record-tenant={recordTenantId}>
      <strong>{TENANT_MISMATCH_CUE}</strong> {TENANT_MISMATCH_STATEMENT}
    </span>
  );
}

/**
 * The persistent tenant indicator (P5.32 step 1): rendered in the app
 * shell, so it is visible on every workspace alongside the
 * control-authority mode indicator. Populated from the claims-derived
 * session store ONLY; a missing session renders the explicit no-session
 * state (never a blank line that could be read as "no tenant").
 */
export function TenantBanner(): JSX.Element {
  const session = getSession();
  return (
    <p className="tenant-indicator" role="status" data-testid="tenant-banner">
      Tenant:{' '}
      {session ? (
        <>
          <strong data-testid="tenant-banner-id">{session.tenantId}</strong>{' '}
          (role: <span data-testid="tenant-banner-role">{session.role}</span>)
        </>
      ) : (
        <span data-testid="tenant-banner-no-session">
          no session — the indicator is populated from authenticated claims
          only (§16.2; the authentication layer wires the session)
        </span>
      )}{' '}
      {TENANT_SOURCE_NOTE} {NO_TENANT_SWITCH_NOTE} {SINGLE_TENANT_NOTE}
    </p>
  );
}
