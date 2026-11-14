// P5.11 — the shared high-impact action confirmation (§20.2).
//
// THE HIGH-IMPACT ACTION LIST FOR THIS PHASE (ledger P5.11 step 3 — the
// list is recorded HERE, in one place, and rendered verbatim by the
// component so a reviewer can audit it):
//
// | Action                 | Operator-triggerable from this console?        |
// |------------------------|------------------------------------------------|
// | approval               | YES — the Dispatch authorization-gate flow     |
// |                        | (ApprovalFlow); confirmed through this         |
// |                        | component before any resolution                |
// | cancellation           | NO — no cancellation transition endpoint       |
// |                        | exists on the read-only P4.30 surface; the     |
// |                        | CANCELLED state is recorded by the platform,   |
// |                        | not entered from the console                   |
// | supersede              | NO — no supersede transition endpoint on the   |
// |                        | console surface; supersessions are platform-   |
// |                        | recorded (§14.2)                               |
// | fallback activation    | NO — fallback is a POLICY activation (P4.17),  |
// | (operator-triggered)   | never a console action; the console displays   |
// |                        | the FALLBACK state read-only                   |
//
// The one console-triggerable high-impact action therefore requires THIS
// confirmation step. The confirmation is EXPLICIT (a labelled, required
// checkbox the user must operate — keyboard-reachable, not colour- or
// hover-dependent, P5.12); the component renders the action name, what it
// affects, and the consequences BEFORE the confirmation control, so the
// confirmation is informed, not blind (§20.2).
//
// Deliberately NOT a modal: the confirmation is inline with the parameters
// it confirms (actor/scope/limits/expiry sit directly above it in the
// Dispatch flow), so the user confirms what they have just read. A typed
// re-entry confirmation is not required by §20.2 or ledger P5.11 step 3;
// the required checkbox is the recorded DEFAULT (decision record
// 10_12_design_pass.md).
import type { JSX } from 'react';

export interface HighImpactAction {
  /** The action id, as rendered (never paraphrased downstream). */
  id: string;
  /** What the action is, in §14.1 terms. */
  description: string;
  /** Whether this console can trigger it at all in this phase. */
  consoleTriggerable: boolean;
  /** Why not, when consoleTriggerable is false (rendered, never elided). */
  notTriggerableReason?: string;
}

/**
 * The recorded phase list. Additions/changes are vocabulary changes: they
 * edit THIS constant AND the pinned test (statePrinciples.test.tsx), never
 * a per-screen improvisation.
 */
export const HIGH_IMPACT_ACTIONS: readonly HighImpactAction[] = [
  {
    id: 'approval',
    description:
      'Approval = passing the §14.1 AUTHORIZATION gate (records actor, scope, limits, and expiry, P4.13)',
    consoleTriggerable: true,
  },
  {
    id: 'cancellation',
    description: 'Cancellation = the CANCELLED lifecycle state (§14.2)',
    consoleTriggerable: false,
    notTriggerableReason:
      'no cancellation transition endpoint exists on the read-only dispatch surface; the console does not invent one (P5.06 gap record)',
  },
  {
    id: 'supersede',
    description: 'Supersede = the SUPERSEDED lifecycle state (§14.2, distinct from CANCELLED)',
    consoleTriggerable: false,
    notTriggerableReason:
      'no supersede transition endpoint exists on the console surface; supersessions are platform-recorded',
  },
  {
    id: 'fallback-activation',
    description:
      'Fallback activation = the FALLBACK lifecycle state via the asset/site fallback policy (P4.17)',
    consoleTriggerable: false,
    notTriggerableReason:
      'fallback is a policy activation, never an operator-triggered console action in this phase',
  },
];

export interface ConfirmHighImpactProps {
  /** The high-impact action id (must be in HIGH_IMPACT_ACTIONS). */
  actionId: string;
  /** What specifically is being confirmed (the concrete instance). */
  subject: string;
  /** The parameters/consequences already displayed to the user. */
  summary: string;
  /** Whether the user has operated the confirmation control. */
  confirmed: boolean;
  /** Called when the user operates the confirmation control. */
  onConfirm: () => void;
  /** Rendered after confirmation (the resolution). */
  children?: JSX.Element;
}

/**
 * The shared explicit-confirmation step for high-impact actions (§20.2;
 * ledger P5.11 step 3). Keyboard-operable by construction: a native
 * checkbox inside a <label> (no colour-only affordance), with
 * aria-required and an explicit description binding.
 */
export function ConfirmHighImpact({
  actionId,
  subject,
  summary,
  confirmed,
  onConfirm,
  children,
}: ConfirmHighImpactProps): JSX.Element {
  const action = HIGH_IMPACT_ACTIONS.find((entry) => entry.id === actionId);
  if (!action) {
    return (
      <p role="alert" data-testid="confirm-high-impact-unknown-action">
        [UNKNOWN HIGH-IMPACT ACTION — displayed verbatim: {actionId}] The
        action is not in the recorded phase list (ConfirmHighImpact.
        HIGH_IMPACT_ACTIONS); no confirmation flow is offered, because an
        unlisted action must never gain a confirmation step by improvisation.
      </p>
    );
  }
  const inputId = `confirm-high-impact-${actionId}`;
  return (
    <div data-testid="confirm-high-impact">
      <p data-testid="confirm-high-impact-action">
        <strong>High-impact action: {action.id}</strong> — {action.description}
        {!action.consoleTriggerable && action.notTriggerableReason
          ? ` (NOT console-triggerable in this phase: ${action.notTriggerableReason})`
          : ''}
      </p>
      <p data-testid="confirm-high-impact-subject">Instance: {subject}</p>
      <p data-testid="confirm-high-impact-summary">{summary}</p>
      {!confirmed ? (
        <p>
          <label htmlFor={inputId}>
            <input
              type="checkbox"
              id={inputId}
              data-testid="confirm-high-impact-checkbox"
              aria-required="true"
              checked={false}
              onChange={onConfirm}
            />{' '}
            I confirm this high-impact action ({action.id}: {subject}) after
            reading the parameters above.
          </label>
        </p>
      ) : (
        children ?? <p data-testid="confirm-high-impact-done">Confirmed.</p>
      )}
    </div>
  );
}
