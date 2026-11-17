// P13.17 — the step-up UX state machine wired to the P13.04 ENFORCED
// protocol (auth hardening, wave 2; WRITTEN-UNRUN).
//
// CONSUMES RECORD 04 BY CITATION — the protocol's shapes below are the
// console-facing VIEWS of the server-side `stepup.rs` design, never a
// second definition:
// - the closed purpose vocabulary (`CommandApproval { command_id }` /
//   `ConsoleAction { action_id }`) mirrors `StepUpPurpose` exactly;
// - the closed denial vocabulary mirrors `StepUpDenial` exactly (each
//   variant maps to display text; the mapping is a VIEW, not a protocol);
// - single-use, time-bounded, triple-bound semantics are the SERVER's —
//   the console can only display the verdicts the server produced.
//
// THE UI-ONLY FLOW'S FATE (record 04 §4, honored): the P5.26
// `stepUpApproval.tsx` panel remains in place (it is NOT edited here —
// the P7.25 additive precedent); its honesty record ("LOCAL UI EVENT —
// NOT a server-verified step-up") stays true for THAT component. This
// module is the ENFORCED-protocol surface: where a composition wires a
// gated action through it, the satisfied state means the SERVER verified
// a challenge — it is never produced by local evaluation. The two flows
// coexist until the composition replaces the UI-only path; neither is
// described as the other.
//
// THE ENDPOINTS (SPECIFIED-NOT-IMPLEMENTED — plan risk 6 discipline): the
// challenge/verify REST surface this flow would drive (the record 04 §2
// ISSUE/VERIFY phases over HTTP) does not exist — no route, no handler.
// This module performs NO fetch and NO navigation (structurally; the
// tests pin it). The transitions below are driven by RESULTS the
// composition supplies; until a deployment supplies them, the machine
// simply never leaves `idle`.

/** The challenge/verify endpoint paths (SPECIFIED-NOT-IMPLEMENTED — the
 * recorded design surface; no fetch exists against them). */
export const STEP_UP_CHALLENGE_PATH = '/api/v1/auth/step-up/challenge';
export const STEP_UP_VERIFY_PATH = '/api/v1/auth/step-up/verify';

/** The closed purpose vocabulary — the console view of record 04's
 * `StepUpPurpose` (consumed by citation, never redefined). */
export type StepUpPurposeView =
  | { kind: 'command-approval'; commandId: string }
  | { kind: 'console-action'; actionId: string };

/** The purpose's recorded text (mirrors `StepUpPurpose::recorded_text`). */
export function purposeRecordedText(purpose: StepUpPurposeView): string {
  return purpose.kind === 'command-approval'
    ? `command-approval:${purpose.commandId}`
    : `console-action:${purpose.actionId}`;
}

/**
 * The closed denial vocabulary — the console view of record 04's
 * `StepUpDenial` variants, each with its display text. A denial the
 * console does not recognize renders as the explicit unknown-denial state
 * (never silently treated as a success — fail-closed display).
 */
export const STEP_UP_DENIAL_KINDS = [
  'NoChallenge',
  'Expired',
  'AlreadyUsed',
  'SubjectMismatch',
  'PurposeMismatch',
  'TenantMismatch',
  'EvidenceRejected',
  'VerifierNotComposed',
  'StoreError',
] as const;

export type StepUpDenialKind = (typeof STEP_UP_DENIAL_KINDS)[number];

const DENIAL_DISPLAY: Record<StepUpDenialKind, string> = {
  NoChallenge:
    'No step-up challenge exists for this subject and purpose — request a fresh challenge.',
  Expired:
    'The step-up challenge expired before verification — request a fresh challenge.',
  AlreadyUsed:
    'The step-up challenge was already consumed (single-use) — a replay never verifies; request a fresh challenge.',
  SubjectMismatch:
    'The presented evidence belongs to another subject — the challenge is bound to its issuing principal.',
  PurposeMismatch:
    'The challenge is bound to another purpose — evidence for one purpose never verifies another (an approval evidence token can never approve a different command).',
  TenantMismatch:
    'The challenge is bound to another tenant — the challenge does not cross tenants.',
  EvidenceRejected:
    'The authentication evidence was rejected by the verifier — re-authenticate freshly at the IdP.',
  VerifierNotComposed:
    'No step-up verifier is composed on this deployment (the fail-closed default) — no step-up can succeed until the operator composes and signs off a verifier.',
  StoreError:
    'The step-up challenge store failed (fail-closed) — the attempt is denied; retry after the operator resolves the store failure.',
};

/** The display text for a denial kind (pure; exported for tests). */
export function stepUpDenialDisplay(kind: StepUpDenialKind): string {
  return DENIAL_DISPLAY[kind];
}

/** An unrecognized denial from the server (forward-compatibility guard:
 * a NEW server denial variant renders as this, never as a success). */
export const UNKNOWN_DENIAL_DISPLAY =
  'The server returned a denial kind this console does not recognize — the attempt is DENIED (fail-closed display); the server-side record is authoritative.';

// --- The flow state machine --------------------------------------------------------

export type StepUpStage = 'idle' | 'challenge-issued' | 'verifying' | 'verified' | 'denied';

export interface StepUpFlowState {
  readonly stage: StepUpStage;
  readonly purpose: StepUpPurposeView | null;
  /** The challenge reference the server issued (opaque to the console). */
  readonly challengeRef: string | null;
  /** The single-use evidence reference of a VERIFIED challenge (what the
   * gated operation would present; the evidence never outlives its
   * challenge window — record 04 §2). */
  readonly evidenceRef: string | null;
  /** The evidence expiry (UTC millis; display-only — the server enforces). */
  readonly evidenceExpiresAtUnixMillis: number | null;
  readonly denialKind: StepUpDenialKind | 'Unknown' | null;
  readonly denialDetail: string | null;
}

/** The initial state: no challenge, no evidence, nothing granted. */
export function initialStepUpFlow(): StepUpFlowState {
  return {
    stage: 'idle',
    purpose: null,
    challengeRef: null,
    evidenceRef: null,
    evidenceExpiresAtUnixMillis: null,
    denialKind: null,
    denialDetail: null,
  };
}

/** ISSUE (the record 04 §2 phase): a challenge reference arrived from the
 * server. A challenge grants NOTHING — the stage only records that fresh
 * authentication is now expected. */
export function challengeIssued(
  previous: StepUpFlowState,
  purpose: StepUpPurposeView,
  challengeRef: string,
): StepUpFlowState {
  return {
    stage: 'challenge-issued',
    purpose,
    challengeRef,
    evidenceRef: null,
    evidenceExpiresAtUnixMillis: null,
    denialKind: null,
    denialDetail: null,
  };
}

/** VERIFY entry (display stage only — the verdict is always the server's). */
export function verificationPending(previous: StepUpFlowState): StepUpFlowState {
  if (previous.stage !== 'challenge-issued') {
    return deniedFromInvalidTransition(previous);
  }
  return { ...previous, stage: 'verifying' };
}

export type StepUpVerifyOutcome =
  | {
      ok: true;
      evidenceRef: string;
      evidenceExpiresAtUnixMillis: number;
    }
  | { ok: false; denialKind: string; detail?: string };

/** CONSUME (the record 04 §2 phase): the server's verdict arrives. Only a
 * server `ok` with an evidence reference produces the `verified` stage —
 * the console cannot synthesize one. A challenge-issued state that
 * receives a second verify outcome after a verified one is displayed as
 * the replay denial (`AlreadyUsed`) — the single-use rule, mirrored for
 * display. */
export function verificationResolved(
  previous: StepUpFlowState,
  outcome: StepUpVerifyOutcome,
): StepUpFlowState {
  if (previous.stage === 'verified') {
    return {
      ...previous,
      stage: 'denied',
      denialKind: 'AlreadyUsed',
      denialDetail:
        'the flow already holds verified evidence for this challenge; a second verdict is a replay (single-use, record 04)',
    };
  }
  if (previous.stage !== 'verifying') {
    return deniedFromInvalidTransition(previous);
  }
  if (outcome.ok) {
    return {
      ...previous,
      stage: 'verified',
      evidenceRef: outcome.evidenceRef,
      evidenceExpiresAtUnixMillis: outcome.evidenceExpiresAtUnixMillis,
      denialKind: null,
      denialDetail: null,
    };
  }
  const known = (STEP_UP_DENIAL_KINDS as readonly string[]).includes(outcome.denialKind);
  return {
    ...previous,
    stage: 'denied',
    denialKind: known ? (outcome.denialKind as StepUpDenialKind) : 'Unknown',
    denialDetail: outcome.detail ?? null,
  };
}

function deniedFromInvalidTransition(previous: StepUpFlowState): StepUpFlowState {
  return {
    ...initialStepUpFlow(),
    stage: 'denied',
    denialKind: 'Unknown',
    denialDetail: `the flow received a verdict while in the '${previous.stage}' stage — the attempt is DENIED (fail-closed display)`,
  };
}

/**
 * The honest satisfied statement (the enforced-protocol counterpart of
 * the P5.26 panel's note — the two are NEVER interchangeable): a verified
 * state here means the SERVER verified a fresh, single-use, purpose-bound
 * challenge. It never means a local UI event, and this module contains no
 * code path that could produce the verified stage without a server `ok`.
 */
export const STEP_UP_ENFORCED_NOTE =
  'Step-up (enforced protocol, record 04): satisfaction means the SERVER ' +
  'verified a fresh, single-use, purpose-bound challenge (the P13-E-3 ' +
  'DEFAULT is IdP-delegated MFA). This is NOT the P5.26 local UI event, ' +
  'and the console never grants authority — the gated operation presents ' +
  'the evidence reference to the server, which enforces the binding ' +
  'independently. A failed, expired, replayed, or missing challenge is a ' +
  'recorded denial that leaves the gated operation unchanged.';
