// P5.26 — step-up approval tests (§17.3; the UI flow + the honesty record).
// UNRUN — never executed in the Phase 5 loop (no install/build/test;
// decision record: 13_26_honesty_security.md). The human executes these
// via `npm test` after install.
//
// WHAT IS UNDER TEST (matching the module's honesty record): the UI flow
// only — (a) the exact typed confirmation of the action target, and (b)
// the local re-validation of the dev-session token against the session
// claims. REAL step-up authentication (server-verified re-auth,
// WebAuthn/MFA) is NOT implemented and these tests assert the HONESTY
// RECORD that says so — they never assert a server round-trip, because
// none exists and none was invented.
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { afterEach, describe, expect, it } from 'vitest';
import { renderToString } from 'react-dom/server';
import {
  STEP_UP_IMPLEMENTATION_NOTE,
  StepUpApproval,
  evaluateStepUp,
  parseDevSessionToken,
  recordStepUpDenial,
} from './stepUpApproval';
import { clearConsoleDenials, getConsoleDenials } from '../security/denialLog';
import type { SessionClaims } from '../state/tenantContext';

const CLAIMS: SessionClaims = {
  tenantId: '00000000-0000-0000-0000-000000000001',
  subject: 'user:operator-1',
  role: 'operator',
};
const TOKEN = `dev tenant=${CLAIMS.tenantId} subject=${CLAIMS.subject} role=${CLAIMS.role}`;
const COMMAND_ID = 'cmd-00000000-0000-0000-0000-00000000abc1';

afterEach(() => {
  clearConsoleDenials();
});

describe('parseDevSessionToken (strict, no lenient parsing)', () => {
  it('parses the auth.rs dev-session format (with and without the Bearer prefix)', () => {
    expect(parseDevSessionToken(TOKEN)).toEqual(CLAIMS);
    expect(parseDevSessionToken(`Bearer ${TOKEN}`)).toEqual(CLAIMS);
  });

  it('returns null for anything that is not a dev-session token (never guesses)', () => {
    expect(parseDevSessionToken('')).toBeNull();
    expect(parseDevSessionToken('Bearer eyJhbGciOiJIUzI1NiJ9.x.y')).toBeNull();
    expect(parseDevSessionToken('dev tenant=only')).toBeNull();
    expect(parseDevSessionToken('dev tenant=a subject=b')).toBeNull(); // no role
  });
});

describe('evaluateStepUp (the pure §17.3 evaluation)', () => {
  const base = {
    confirmTargetId: COMMAND_ID,
    claims: CLAIMS as SessionClaims | null,
    tokenValue: TOKEN,
  };

  it('the typed confirmation must match the target EXACTLY (trimmed)', () => {
    expect(evaluateStepUp({ ...base, typedValue: COMMAND_ID }).typedOk).toBe(true);
    expect(evaluateStepUp({ ...base, typedValue: ` ${COMMAND_ID} ` }).typedOk).toBe(true);
    expect(evaluateStepUp({ ...base, typedValue: 'cmd-something-else' }).typedOk).toBe(false);
    const nearMiss = evaluateStepUp({ ...base, typedValue: COMMAND_ID.slice(0, -1) });
    expect(nearMiss.typedOk).toBe(false);
    expect(nearMiss.satisfied).toBe(false);
  });

  it('a missing session can NEVER satisfy step-up (a context is never fabricated, P0.16 rule 5)', () => {
    const check = evaluateStepUp({
      confirmTargetId: COMMAND_ID,
      typedValue: COMMAND_ID,
      claims: null,
      tokenValue: TOKEN,
    });
    expect(check.reauthOk).toBe(false);
    expect(check.satisfied).toBe(false);
    expect(check.reauthReason).toContain('no authenticated session');
  });

  it('an unparsable re-presented credential fails with the expected format', () => {
    const check = evaluateStepUp({ ...base, tokenValue: 'Bearer not-a-dev-token' });
    expect(check.reauthOk).toBe(false);
    expect(check.reauthReason).toContain('dev tenant=<uuid>');
  });

  it('a credential that mismatches the session on ANY claim fails, naming the mismatch', () => {
    const otherTenant = evaluateStepUp({
      ...base,
      tokenValue: `dev tenant=00000000-0000-0000-0000-000000000099 subject=${CLAIMS.subject} role=${CLAIMS.role}`,
    });
    expect(otherTenant.reauthOk).toBe(false);
    expect(otherTenant.reauthReason).toContain('tenant');
    const otherRole = evaluateStepUp({
      ...base,
      tokenValue: `dev tenant=${CLAIMS.tenantId} subject=${CLAIMS.subject} role=viewer`,
    });
    expect(otherRole.reauthOk).toBe(false);
    expect(otherRole.reauthReason).toContain('role');
  });

  it('a matching credential re-validates, but satisfied ALSO requires the typed confirmation', () => {
    const revalidatedOnly = evaluateStepUp({ ...base, typedValue: 'wrong' });
    expect(revalidatedOnly.reauthOk).toBe(true);
    expect(revalidatedOnly.satisfied).toBe(false);
    const both = evaluateStepUp({ ...base, typedValue: COMMAND_ID });
    expect(both.satisfied).toBe(true);
    expect(both.reauthReason).toContain('re-validated locally');
  });
});

describe('recordStepUpDenial (LOCAL record only — the audit boundary)', () => {
  it('writes to the local denial log with the surface, role, and reason — nothing else', () => {
    recordStepUpDenial('Dispatch ApprovalFlow (re-authentication)', 'operator', 'mismatch');
    const events = getConsoleDenials();
    expect(events).toHaveLength(1);
    expect(events[0].actionId).toBe('step-up-approval (UI flow)');
    expect(events[0].role).toBe('operator');
    expect(events[0].surface).toBe('Dispatch ApprovalFlow (re-authentication)');
    expect(events[0].reason).toBe('mismatch');
  });

  it('never sends the denial anywhere (no fetch in the module code)', () => {
    const source = readFileSync(join(dirname(fileURLToPath(import.meta.url)), 'stepUpApproval.tsx'), 'utf-8');
    const code = source
      .split('\n')
      .filter((line) => !line.trim().startsWith('//'))
      .join('\n');
    expect(code).not.toMatch(/\bfetch\s*\(/);
    expect(code).not.toMatch(/XMLHttpRequest|sendBeacon/);
  });
});

describe('StepUpApproval rendering (the honest panel)', () => {
  const pending = evaluateStepUp({
    confirmTargetId: COMMAND_ID,
    typedValue: '',
    claims: CLAIMS,
    tokenValue: '',
  });
  const satisfied = evaluateStepUp({
    confirmTargetId: COMMAND_ID,
    typedValue: COMMAND_ID,
    claims: CLAIMS,
    tokenValue: TOKEN,
  });

  it('renders the implementation note verbatim (the honesty record is ON the panel)', () => {
    const html = renderToString(
      <StepUpApproval
        actionLabel="approval — passing the §14.1 authorization gate"
        confirmTargetId={COMMAND_ID}
        typedValue=""
        tokenValue=""
        claims={CLAIMS}
        onTypedChange={() => undefined}
        onTokenChange={() => undefined}
        check={pending}
      />,
    );
    // renderToString separates adjacent text nodes with comment markers,
    // so the static prefix, the actionLabel expression, and the '(§17.3)'
    // suffix are asserted separately (never as one literal across the
    // expression boundary).
    expect(html).toContain('Step-up approval for:');
    expect(html).toContain('approval — passing the §14.1 authorization gate');
    expect(html).toContain('(§17.3)');
    expect(html).toContain(COMMAND_ID);
  });

  it('the honesty record states what is NOT implemented (WebAuthn/MFA; no invented endpoint)', () => {
    expect(STEP_UP_IMPLEMENTATION_NOTE).toContain('UI flow only');
    expect(STEP_UP_IMPLEMENTATION_NOTE).toContain('WebAuthn/MFA');
    expect(STEP_UP_IMPLEMENTATION_NOTE).toContain('LATER-STAGE DEPLOYMENT REQUIREMENT');
    expect(STEP_UP_IMPLEMENTATION_NOTE).toContain('no step-up endpoint was invented');
    expect(STEP_UP_IMPLEMENTATION_NOTE).toContain('P4.13');
    expect(STEP_UP_IMPLEMENTATION_NOTE).toContain('TenantDenialAuditRecord');
  });

  it('pending shows both requirement statuses and the pending statement', () => {
    const html = renderToString(
      <StepUpApproval
        actionLabel="approval"
        confirmTargetId={COMMAND_ID}
        typedValue=""
        tokenValue=""
        claims={CLAIMS}
        onTypedChange={() => undefined}
        onTokenChange={() => undefined}
        check={pending}
      />,
    );
    expect(html).toContain('step-up-pending');
    expect(html).toContain('[not confirmed]');
    expect(html).toContain('[not re-validated]');
  });

  it('satisfied states it is a LOCAL UI EVENT, never a server-verified step-up', () => {
    const html = renderToString(
      <StepUpApproval
        actionLabel="approval"
        confirmTargetId={COMMAND_ID}
        typedValue={COMMAND_ID}
        tokenValue={TOKEN}
        claims={CLAIMS}
        onTypedChange={() => undefined}
        onTokenChange={() => undefined}
        check={satisfied}
      />,
    );
    expect(html).toContain('step-up-satisfied');
    expect(html).toContain('LOCAL UI EVENT');
    expect(html).toContain('NOT a server-verified step-up');
    expect(html).toContain('this panel never grants authority');
  });

  it('with no session, the explicit no-session alert renders', () => {
    const html = renderToString(
      <StepUpApproval
        actionLabel="approval"
        confirmTargetId={COMMAND_ID}
        typedValue=""
        tokenValue=""
        claims={null}
        onTypedChange={() => undefined}
        onTokenChange={() => undefined}
        check={evaluateStepUp({
          confirmTargetId: COMMAND_ID,
          typedValue: '',
          claims: null,
          tokenValue: '',
        })}
      />,
    );
    expect(html).toContain('step-up-no-session');
    expect(html).toContain('No authenticated session');
  });
});

describe('composition with the P5.10–P5.12 wave (structural, source scan)', () => {
  const DISPATCH = readFileSync(
    join(dirname(fileURLToPath(import.meta.url)), '..', 'workspaces', 'Dispatch.tsx'),
    'utf-8',
  );

  it('the Dispatch approval flow composes BOTH waves: ConfirmHighImpact (confirmation) + StepUpApproval (step-up)', () => {
    expect(DISPATCH).toContain('<ConfirmHighImpact');
    expect(DISPATCH).toContain('<StepUpApproval');
    // The step-up panel renders INSIDE the confirmation (the layering the
    // decision record describes) — the StepUpApproval JSX sits within the
    // ConfirmHighImpact children region of the ApprovalFlow body.
    const confirmation = DISPATCH.indexOf('<ConfirmHighImpact');
    const stepUp = DISPATCH.indexOf('<StepUpApproval');
    const childrenEnd = DISPATCH.indexOf('</ConfirmHighImpact>');
    expect(stepUp).toBeGreaterThan(confirmation);
    expect(stepUp).toBeLessThan(childrenEnd);
  });
});
