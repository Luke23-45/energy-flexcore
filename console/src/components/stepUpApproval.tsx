// P5.26 — the step-up approval UI for high-impact actions (§17.3, §20.2;
// ledger P5.26 step 3 — the approval flow from P5.06 is the implementation).
//
// MERGE POINT with the concurrent P5.10–P5.12 wave (recorded in
// 13_26_honesty_security.md): that wave implements a `ConfirmHighImpact`
// confirmation component for the §20.2/P5.11 explicit-confirmation step.
// This file is deliberately SEPARATE (`stepUpApproval.tsx`) and covers the
// §17.3 STEP-UP requirements layered on top of confirmation: the typed
// confirmation of the action target AND a fresh re-authentication step.
// When both waves' components are reviewed together, `ConfirmHighImpact`
// (confirmation) and `StepUpApproval` (step-up) compose at the Dispatch
// approval flow — or merge into one component — WITHOUT either wave having
// edited the other's file.
//
// HONESTY RECORD (P5.26 step 3, what is implemented vs. what is not):
// - IMPLEMENTED: the UI flow — (a) an explicit typed confirmation of the
//   action's target id (a command id for the Dispatch approval), and
//   (b) a fresh re-validation of the session credential on the DEV-SESSION
//   path (the only identity path shipped in Phase 1, auth.rs feature-gated):
//   the operator re-presents the dev-session token and it must parse to
//   claims matching the current authenticated session EXACTLY (tenant,
//   subject, role).
// - NOT IMPLEMENTED (a later-stage deployment requirement, §17.3): real
//   step-up authentication — server-verified re-authentication, WebAuthn/
//   MFA, or any server round-trip. NO step-up validation endpoint exists on
//   the §15.2 surface and NONE WAS INVENTED: the P4.13 authorization-gate
//   records are the server-side gate, and the P4.28 denial audit
//   (`TenantDenialAuditRecord` / `SecurityAuditSink`) is the server-side
//   denial record. A failed step-up attempt is recorded LOCALLY only
//   (security/denialLog.ts).
// - A satisfied step-up is a LOCAL UI event with a local timestamp. It is
//   not a server-verified step-up and must never be described as one.
import { useState } from 'react';
import type { JSX } from 'react';
import type { SessionClaims } from '../state/tenantContext';
import { recordConsoleDenial } from '../security/denialLog';

/** The rendered honesty record (verbatim constant; see the module header). */
export const STEP_UP_IMPLEMENTATION_NOTE =
  'Step-up approval (§17.3): this phase implements the UI flow only — an ' +
  'explicit typed confirmation of the action target and a fresh ' +
  're-validation of the session credential on the dev-session path. REAL ' +
  'step-up authentication (server-verified re-authentication, WebAuthn/MFA) ' +
  'is a LATER-STAGE DEPLOYMENT REQUIREMENT and is NOT implemented in 0.x; ' +
  'no step-up endpoint was invented. The server-side gates remain the P4.13 ' +
  'authorization-gate records; denials are audited server-side (§16.2, ' +
  'P4.28 TenantDenialAuditRecord) — a failed step-up attempt here is ' +
  'recorded locally only.';

/**
 * Parses a dev-session token (the auth.rs dev-auth format:
 * `dev tenant=<uuid> subject=<subject> role=<role>`; the same format
 * api/client.ts renders). Returns null for anything else — no guessing,
 * no lenient parsing (the dev resolver itself is strict).
 */
export function parseDevSessionToken(token: string): SessionClaims | null {
  const rest = token.startsWith('Bearer ') ? token.slice('Bearer '.length) : token;
  const trimmed = rest.trim();
  if (!trimmed.startsWith('dev ')) return null;
  let tenantId: string | null = null;
  let subject: string | null = null;
  let role: string | null = null;
  for (const part of trimmed.split(/\s+/)) {
    if (part.startsWith('tenant=')) tenantId = part.slice('tenant='.length);
    else if (part.startsWith('subject=')) subject = part.slice('subject='.length);
    else if (part.startsWith('role=')) role = part.slice('role='.length);
  }
  if (tenantId === null || subject === null || role === null) return null;
  return { tenantId, subject, role };
}

export interface StepUpCheck {
  /** The typed confirmation matches the action target id exactly. */
  typedOk: boolean;
  /** The re-presented token parsed to claims matching the session. */
  reauthOk: boolean;
  /** WHY the re-authentication step failed (or why it is satisfied). */
  reauthReason: string;
  /** Both requirements are satisfied (the local UI event). */
  satisfied: boolean;
}

/**
 * The pure step-up evaluation (exported for tests and for the parent's
 * onChange handlers). The typed confirmation is EXACT (trimmed); the token
 * re-validation requires tenant + subject + role to match the session.
 * A missing session can never satisfy step-up (a context is never
 * fabricated, P0.16 rule 5).
 */
export function evaluateStepUp(options: {
  confirmTargetId: string;
  typedValue: string;
  claims: SessionClaims | null;
  tokenValue: string;
}): StepUpCheck {
  const typedOk = options.typedValue.trim() === options.confirmTargetId;
  const parsed = parseDevSessionToken(options.tokenValue);
  if (options.claims === null) {
    return {
      typedOk,
      reauthOk: false,
      reauthReason:
        'no authenticated session to re-validate against — step-up cannot be satisfied (P0.16 rule 5)',
      satisfied: false,
    };
  }
  if (parsed === null) {
    return {
      typedOk,
      reauthOk: false,
      reauthReason:
        'the re-presented credential did not parse as a dev-session token (expected format: dev tenant=<uuid> subject=<subject> role=<role>)',
      satisfied: false,
    };
  }
  const mismatches: string[] = [];
  if (parsed.tenantId !== options.claims.tenantId) mismatches.push('tenant');
  if (parsed.subject !== options.claims.subject) mismatches.push('subject');
  if (parsed.role !== options.claims.role) mismatches.push('role');
  if (mismatches.length > 0) {
    return {
      typedOk,
      reauthOk: false,
      reauthReason: `the re-presented credential does not match the authenticated session (${mismatches.join(', ')})`,
      satisfied: false,
    };
  }
  return {
    typedOk,
    reauthOk: true,
    reauthReason:
      'the re-presented credential matches the authenticated session (tenant, subject, role) — re-validated locally',
    satisfied: typedOk,
  };
}

/** Records a failed step-up attempt LOCALLY (never to a server — see the
 * module header for the audit boundary). The role comes from the caller's
 * claims-derived session (never user input). */
export function recordStepUpDenial(
  surface: string,
  role: string,
  reason: string,
): void {
  recordConsoleDenial({
    actionId: 'step-up-approval (UI flow)',
    role,
    reason,
    surface,
  });
}

/**
 * The controlled step-up panel. The parent (the Dispatch approval flow)
 * owns the input values and re-renders with them; this component renders
 * the two requirements, their statuses, and — when both are satisfied —
 * the honest satisfied statement.
 */
export function StepUpApproval({
  actionLabel,
  confirmTargetId,
  typedValue,
  tokenValue,
  claims,
  onTypedChange,
  onTokenChange,
  check,
}: {
  actionLabel: string;
  confirmTargetId: string;
  typedValue: string;
  tokenValue: string;
  claims: SessionClaims | null;
  onTypedChange: (value: string) => void;
  onTokenChange: (value: string) => void;
  check: StepUpCheck;
}): JSX.Element {
  return (
    <div data-testid="step-up-approval">
      <h5>Step-up approval for: {actionLabel} (§17.3)</h5>
      <p role="note">{STEP_UP_IMPLEMENTATION_NOTE}</p>
      <ol>
        <li>
          <label>
            Type the target id ({confirmTargetId}) to confirm the action target
            exactly:{' '}
            <input
              type="text"
              data-testid="step-up-typed-id"
              value={typedValue}
              onChange={(event) => onTypedChange(event.target.value)}
            />
          </label>
          <p data-testid="step-up-confirmation-status">
            {check.typedOk
              ? '[confirmed] the typed id matches the action target.'
              : '[not confirmed] the typed id does not (yet) match the action target.'}
          </p>
        </li>
        <li>
          <label>
            Fresh re-authentication — re-present your session credential for
            re-validation (dev-session path:{' '}
            <code>dev tenant=&lt;uuid&gt; subject=&lt;subject&gt; role=&lt;role&gt;</code>
            ):{' '}
            <input
              type="text"
              data-testid="step-up-token-input"
              value={tokenValue}
              onChange={(event) => onTokenChange(event.target.value)}
            />
          </label>
          <p data-testid="step-up-reauth-status">
            {check.reauthOk
              ? `[re-validated] ${check.reauthReason}`
              : `[not re-validated] ${check.reauthReason}`}
          </p>
        </li>
      </ol>
      {check.satisfied ? (
        <div data-testid="step-up-satisfied" role="status">
          <strong>
            Step-up satisfied (LOCAL UI EVENT with a local timestamp — NOT a
            server-verified step-up):
          </strong>{' '}
          the typed confirmation matches the target and the session credential
          re-validated against the authenticated claims. The authorization
          decision itself remains the server-side gate (the P4.13
          authorization-gate records); this panel never grants authority.
        </div>
      ) : (
        <p data-testid="step-up-pending" role="status">
          Step-up pending: both requirements above must be satisfied before the
          high-impact action resolves.
        </p>
      )}
      {claims === null ? (
        <p role="alert" data-testid="step-up-no-session">
          No authenticated session: step-up cannot be satisfied without claims
          to re-validate against (§16.2).
        </p>
      ) : null}
    </div>
  );
}
