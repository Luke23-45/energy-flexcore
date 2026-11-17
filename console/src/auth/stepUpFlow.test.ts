// P13.17 — the enforced-protocol step-up flow tests (WRITTEN-UNRUN; the
// human gate runs `npm test` after the first install). The machine
// consumes record 04's protocol BY CITATION: the denial vocabulary and
// purpose vocabulary asserted here mirror `apps/control-plane/src/
// stepup.rs` exactly; a divergence is a defect in this view, never a
// second protocol definition.
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import {
  STEP_UP_CHALLENGE_PATH,
  STEP_UP_DENIAL_KINDS,
  STEP_UP_ENFORCED_NOTE,
  STEP_UP_VERIFY_PATH,
  challengeIssued,
  initialStepUpFlow,
  purposeRecordedText,
  stepUpDenialDisplay,
  verificationPending,
  verificationResolved,
} from './stepUpFlow';

const COMMAND_PURPOSE = { kind: 'command-approval' as const, commandId: 'cmd-1' };
const ACTION_PURPOSE = { kind: 'console-action' as const, actionId: 'apikey.rotate' };

describe('the consumed record-04 vocabularies (mirrors, never redefinitions)', () => {
  it('the denial kinds mirror StepUpDenial exactly (nine variants)', () => {
    expect(STEP_UP_DENIAL_KINDS).toEqual([
      'NoChallenge',
      'Expired',
      'AlreadyUsed',
      'SubjectMismatch',
      'PurposeMismatch',
      'TenantMismatch',
      'EvidenceRejected',
      'VerifierNotComposed',
      'StoreError',
    ]);
  });

  it('every denial kind has display text (none empty, none success-shaped)', () => {
    for (const kind of STEP_UP_DENIAL_KINDS) {
      const text = stepUpDenialDisplay(kind);
      expect(text.length).toBeGreaterThan(0);
      expect(text.toLowerCase()).not.toContain('succeed');
    }
  });

  it('the replay denial names single-use; the not-composed denial states the fail-closed default', () => {
    expect(stepUpDenialDisplay('AlreadyUsed')).toContain('single-use');
    expect(stepUpDenialDisplay('VerifierNotComposed')).toContain('fail-closed');
    expect(stepUpDenialDisplay('PurposeMismatch')).toContain(
      'evidence for one purpose never verifies another',
    );
  });

  it('the purpose recorded text mirrors StepUpPurpose::recorded_text', () => {
    expect(purposeRecordedText(COMMAND_PURPOSE)).toBe('command-approval:cmd-1');
    expect(purposeRecordedText(ACTION_PURPOSE)).toBe('console-action:apikey.rotate');
  });
});

describe('the flow state machine (server-driven transitions only)', () => {
  it('starts idle with nothing granted', () => {
    const state = initialStepUpFlow();
    expect(state.stage).toBe('idle');
    expect(state.evidenceRef).toBeNull();
    expect(state.denialKind).toBeNull();
  });

  it('ISSUE: a challenge reference moves to challenge-issued and grants nothing', () => {
    const state = challengeIssued(initialStepUpFlow(), COMMAND_PURPOSE, 'chal-1');
    expect(state.stage).toBe('challenge-issued');
    expect(state.challengeRef).toBe('chal-1');
    expect(state.evidenceRef).toBeNull();
  });

  it('VERIFY: only a challenge-issued state may enter verifying (a verdict against idle fails closed)', () => {
    expect(verificationPending(initialStepUpFlow()).stage).toBe('denied');
    const ok = verificationPending(challengeIssued(initialStepUpFlow(), COMMAND_PURPOSE, 'chal-2'));
    expect(ok.stage).toBe('verifying');
  });

  it('CONSUME: only a server ok produces the verified stage (the console cannot synthesize evidence)', () => {
    const verifying = verificationPending(
      challengeIssued(initialStepUpFlow(), COMMAND_PURPOSE, 'chal-3'),
    );
    const verified = verificationResolved(verifying, {
      ok: true,
      evidenceRef: 'ev-1',
      evidenceExpiresAtUnixMillis: 9_999,
    });
    expect(verified.stage).toBe('verified');
    expect(verified.evidenceRef).toBe('ev-1');
    expect(verified.evidenceExpiresAtUnixMillis).toBe(9_999);
  });

  it('a server denial resolves to the named denial with its display text', () => {
    const verifying = verificationPending(
      challengeIssued(initialStepUpFlow(), COMMAND_PURPOSE, 'chal-4'),
    );
    const denied = verificationResolved(verifying, { ok: false, denialKind: 'Expired' });
    expect(denied.stage).toBe('denied');
    expect(denial_text(denied)).toContain('expired');
  });

  it('an UNRECOGNIZED server denial kind renders fail-closed (never as a success)', () => {
    const verifying = verificationPending(
      challengeIssued(initialStepUpFlow(), COMMAND_PURPOSE, 'chal-5'),
    );
    const denied = verificationResolved(verifying, {
      ok: false,
      denialKind: 'SomeFutureDenial',
    });
    expect(denied.stage).toBe('denied');
    expect(denied.denialKind).toBe('Unknown');
  });

  it('a second verdict after a verified state is displayed as the replay denial (single-use, mirrored)', () => {
    const verifying = verificationPending(
      challengeIssued(initialStepUpFlow(), COMMAND_PURPOSE, 'chal-6'),
    );
    const verified = verificationResolved(verifying, {
      ok: true,
      evidenceRef: 'ev-2',
      evidenceExpiresAtUnixMillis: 9_999,
    });
    const replay = verificationResolved(verified, { ok: false, denialKind: 'Expired' });
    expect(replay.stage).toBe('denied');
    expect(replay.denialKind).toBe('AlreadyUsed');
    expect(replay.denialDetail).toContain('single-use');
  });

  it('a verdict against a non-verifying state denies fail-closed with the stage named', () => {
    const denied = verificationResolved(initialStepUpFlow(), {
      ok: true,
      evidenceRef: 'ev-3',
      evidenceExpiresAtUnixMillis: 1,
    });
    expect(denied.stage).toBe('denied');
    expect(denied.evidenceRef).toBeNull();
    expect(denied.denialDetail).toContain("'idle'");
  });
});

function denial_text(state: { denialDetail: string | null; denialKind: string | null }): string {
  return `${state.denialKind ?? ''} ${state.denialDetail ?? ''}`;
}

describe('the enforced-note boundary (never interchangeable with the P5.26 local event)', () => {
  it('states the server-verified semantics and the recorded-denial no-wedge rule', () => {
    expect(STEP_UP_ENFORCED_NOTE).toContain('SERVER');
    expect(STEP_UP_ENFORCED_NOTE).toContain('single-use, purpose-bound');
    expect(STEP_UP_ENFORCED_NOTE).toContain('NOT the P5.26 local UI event');
    expect(STEP_UP_ENFORCED_NOTE).toContain('recorded denial');
    expect(STEP_UP_ENFORCED_NOTE).toContain('P13-E-3');
  });
});

describe('structural honesty (source scan — the P5.26 precedent)', () => {
  const SOURCE = readFileSync(
    join(dirname(fileURLToPath(import.meta.url)), 'stepUpFlow.ts'),
    'utf-8',
  );
  const CODE = SOURCE.split('\n')
    .filter((line) => !line.trim().startsWith('//'))
    .join('\n');

  it('performs no fetch against the specified-not-implemented endpoints (the verdict is always supplied)', () => {
    expect(CODE).not.toMatch(/\bfetch\s*\(/);
    expect(CODE).not.toMatch(/XMLHttpRequest|sendBeacon/);
    expect(CODE).not.toMatch(/window\.location|location\.assign|location\.replace/);
    // The endpoint constants exist as the recorded design surface only.
    expect(STEP_UP_CHALLENGE_PATH).toBe('/api/v1/auth/step-up/challenge');
    expect(STEP_UP_VERIFY_PATH).toBe('/api/v1/auth/step-up/verify');
  });

  it('contains no code path that produces verified without a server ok (the ok-shaped literal only appears in the outcome branch)', () => {
    // The machine constructs `stage: 'verified'` exclusively inside the
    // `outcome.ok` branch of verificationResolved; a structural scan
    // counts the literal and pins it to one occurrence site.
    const occurrences = CODE.split("stage: 'verified'").length - 1;
    expect(occurrences).toBe(1);
    expect(CODE).toContain('if (outcome.ok)');
  });
});
