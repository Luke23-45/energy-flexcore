// P5.27 — the additive shared mode-gate display component (§21.4, §17.5).
//
// ADDITIVE by design: the P5.13 `ControlAuthorityModeIndicator`
// (components/honesty.tsx) stays the persistent per-workspace mode
// INDICATOR and is not modified; this component renders the P5.27
// ENFORCEMENT layer beside it where a workspace needs the gate statement —
// the deployed mode, the defence-in-depth boundary (the server remains the
// enforcement point), the per-site (mixed-mode) rule, and the honest
// configured-but-not-exercised status of the two promoted §21.4 modes.
// Distinct markers + text, never colour alone (P5.12 discipline).
import type { JSX } from 'react';
import {
  CONTROL_AUTHORITY_MODES,
  DEPLOYED_CONTROL_AUTHORITY_MODE,
  EXERCISED_MODES,
  MIXED_MODE_NOTE,
  MODE_ENFORCEMENT_NOTE,
  NON_EXERCISED_MODES_NOTE,
} from '../security/modeControl';

/** One mode chip in the vocabulary line: exercised vs vocabulary-only. */
function ModeChip({ mode }: { mode: string }): JSX.Element {
  const exercised = (EXERCISED_MODES as readonly string[]).includes(mode);
  return (
    <code data-testid={`mode-chip-${mode}`} data-exercised={exercised}>
      {mode}
      {exercised ? '' : ' (configured-but-not-exercised)'}
    </code>
  );
}

/**
 * The mode-gate notice: WHAT mode this deployment is in, WHAT the gate
 * does, and the honest boundary of what it can enforce. Rendered beside
 * the P5.13 indicator; it never replaces it.
 */
export function ModeGateNotice(): JSX.Element {
  return (
    <div className="mode-gate" data-testid="control-authority-mode-gate" role="note">
      <p>
        <strong>
          [Control-authority mode enforcement: {DEPLOYED_CONTROL_AUTHORITY_MODE}]
        </strong>{' '}
        — mutating console actions are gated by the mode AND the capability
        (P5.26 ∩ P5.27): an action proceeds only when BOTH allow, and both
        denial reasons render. Reading evidence is permitted in every mode.
        In the deployed {DEPLOYED_CONTROL_AUTHORITY_MODE} mode, a mutating
        action is available ONLY against the simulator (never a
        non-simulator target — the P4.16 guard refuses every one, and the
        console has no non-simulator issue surface at all), and under
        Read-only/shadow every mutating action is DISABLED with its
        explanation — not silently hidden.
      </p>
      <p>
        The §21.4 mode vocabulary (closed):{' '}
        {CONTROL_AUTHORITY_MODES.map((mode) => (
          <span key={mode}>
            <ModeChip mode={mode} />{' '}
          </span>
        ))}
      </p>
      <p>{MODE_ENFORCEMENT_NOTE}</p>
      <p>{MIXED_MODE_NOTE}</p>
      <p>{NON_EXERCISED_MODES_NOTE}</p>
    </div>
  );
}
