// P5.26 (FRV1-R3-011) — the diagnostics display for the console-side
// denial UI-event log (WRITTEN-UNRUN — never executed; the human gate
// runs the console tests after the first install).
//
// WHAT THIS COMPONENT IS: the display surface the P5.26 denial-log module
// recorded as its purpose ("so the operator/console diagnostics can
// explain what the user saw") but which no component ever rendered
// (FRV1-R3-011). It renders the CURRENT LOCAL log — the entry count and
// the most recent entry — from `security/denialLog.ts` (the same
// in-memory ring buffer the step-up flow and the capability gates write).
//
// WHAT IT IS NOT (the audit boundary, rendered verbatim): these are LOCAL
// UI-event diagnostics only. They are in memory, cleared on reload, never
// sent to any server (no such endpoint exists on the §15.2 surface), and
// are NEVER a substitute for the server-side audit trail (§16.2; the
// P4.28 TenantDenialAuditRecord / SecurityAuditSink — the real audit).
//
// Composition: rendered on the Administration route, below the P13.18
// management catalog (the additive P7.25 panel precedent — App.tsx, the
// FRV1-R3-011 marked edit; Administration.tsx itself is not modified).
import type { JSX } from 'react';
import {
  DENIAL_LOG_CAPACITY,
  getConsoleDenials,
} from '../security/denialLog';

/** The diagnostics block's honesty record (rendered verbatim). */
export const DENIAL_LOG_DIAGNOSTICS_NOTE =
  'Local denial diagnostics (P5.26, FRV1-R3-011): these are the console\'s ' +
  'LOCAL UI-event records of capability denials and step-up failures — ' +
  'in memory, bounded (' + DENIAL_LOG_CAPACITY + ' entries), cleared on ' +
  'reload, and never sent to any server. They explain what THIS console ' +
  'rendered to the user; they are NEVER the audit trail. Denials are ' +
  'audited SERVER-side (§16.2; P4.28 TenantDenialAuditRecord / ' +
  'SecurityAuditSink) — this display is not that record and claims to be ' +
  'nothing more.';

/**
 * The denial-log diagnostics block: the entry count and the most recent
 * local entry (surface, action, role, the SAME reason text the UI
 * rendered, and the UI-event instant). Renders the explicit empty state
 * when nothing has been recorded this session — never a fabricated entry.
 */
export function DenialLogDiagnostics(): JSX.Element {
  const denials = getConsoleDenials();
  const last = denials.at(-1);
  return (
    <div className="denial-diagnostics" data-testid="denial-log-diagnostics" role="note">
      <h2>Local denial diagnostics</h2>
      <p>
        Denials recorded locally this session: <code>{denials.length}</code>{' '}
        (ring buffer of {DENIAL_LOG_CAPACITY}).
      </p>
      {last ? (
        <p data-testid="denial-log-last">
          Last denial: <code>{last.actionId}</code> as <code>{last.role}</code>{' '}
          at <code>{last.surface}</code> ({last.atIso}) — reason:{' '}
          {last.reason}
        </p>
      ) : (
        <p data-testid="denial-log-empty" role="status">
          No capability denial or step-up failure has been recorded in this
          session. (The log records UI events only; a session with no denials
          shows this empty state — no entry is ever invented.)
        </p>
      )}
      <p>{DENIAL_LOG_DIAGNOSTICS_NOTE}</p>
    </div>
  );
}
