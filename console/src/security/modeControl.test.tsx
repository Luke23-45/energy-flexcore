// P5.27 — the control-authority mode gate tests (§21.4, §17.5).
//
// UNRUN — written, never executed (the Phase 5 loop bans: no npm install,
// no vitest run; decision record: 27_mode_enforcement.md). The human
// executes these via `npm test` after install.
//
// METHOD NOTE: gateActionForMode / combineChecks / useCanInMode are pure
// functions of their inputs plus the claims-derived session store (no
// subscription, no effect), so the tests call them DIRECTLY — the same
// value they return during a component render (the useCan.test.ts method).
// One render test pins the ModeGateNotice display component end-to-end.
//
// What the tests pin (the P5.27 acceptance core):
//   1. the mode vocabulary is the closed §21.4 four-mode set; the deployed
//      mode is a member, is an exercised mode, and equals the P5.13 display
//      constant (one value, two surfaces — a mismatch is a defect);
//   2. Simulation mode: mutating actions ALLOW with the SIMULATED
//      qualification and the no-non-simulator-surface boundary; reads allow;
//   3. Read-only/shadow mode: EVERY mutating action DENIED with the
//      explanation (never a silent hide);
//   4. Operator-approved / Bounded automation: mutating actions DENIED as
//      configured-but-not-exercised; no promotion path claimed;
//   5. the mode-vs-capability INTERSECTION: an action proceeds only when
//      both gates allow; both denial reasons render; either denial alone
//      denies;
//   6. deny-by-default: an action id off the mirrored surface denies in
//      every mode;
//   7. the per-site resolution: uniform deployment uses the deployment
//      mode everywhere; an override map applies per site; a defective
//      override fails closed;
//   8. the enforcement-honesty statements render (defence-in-depth UI,
//      server authoritative; configured-but-not-exercised vocabulary).
import { afterEach, describe, expect, it } from 'vitest';
import { renderToString } from 'react-dom/server';
import { setSession, clearSession, type SessionClaims } from '../state/tenantContext';
import { CONSOLE_CONTROL_AUTHORITY_MODE } from '../components/honesty';
import {
  combineChecks,
  effectiveMode,
  gateActionForMode,
  isMutatingAction,
  MIXED_MODE_NOTE,
  MODE_ENFORCEMENT_NOTE,
  NON_EXERCISED_MODES_NOTE,
  useCanInMode,
  CONTROL_AUTHORITY_MODES,
  DEPLOYED_CONTROL_AUTHORITY_MODE,
  EXERCISED_MODES,
} from './modeControl';
import { canAction, CAPABILITY_MIRROR_NOTE } from './capabilityMatrix';
import { ModeGateNotice } from '../components/modeGateNotice';

const TENANT_ADMIN: SessionClaims = {
  tenantId: '00000000-0000-0000-0000-000000000001',
  subject: 'user:tenant-admin-1',
  role: 'tenant-admin',
};
const VIEWER: SessionClaims = { ...TENANT_ADMIN, role: 'viewer' };

afterEach(() => {
  clearSession();
});

describe('the mode configuration surface (§21.4 vocabulary, deployment-derived)', () => {
  it('carries the closed four-mode §21.4 vocabulary, verbatim', () => {
    expect([...CONTROL_AUTHORITY_MODES]).toEqual([
      'Simulation',
      'Read-only/shadow',
      'Operator-approved',
      'Bounded automation',
    ]);
  });

  it('deploys an exercised mode and agrees with the P5.13 display constant', () => {
    expect(
      (CONTROL_AUTHORITY_MODES as readonly string[]).includes(DEPLOYED_CONTROL_AUTHORITY_MODE),
    ).toBe(true);
    expect(EXERCISED_MODES).toContain(DEPLOYED_CONTROL_AUTHORITY_MODE);
    // One value, two surfaces (P5.13 indicator / P5.27 gate): a mismatch is
    // a defect the tests refuse to hide.
    expect(DEPLOYED_CONTROL_AUTHORITY_MODE).toBe(CONSOLE_CONTROL_AUTHORITY_MODE);
  });

  it('resolves a uniform deployment to the deployment mode for any site', () => {
    expect(effectiveMode()).toBe('Simulation');
    expect(effectiveMode('site-north')).toBe('Simulation');
  });

  it('fails closed on a defective mode name (never coerced)', () => {
    // The override map is exercised through the pure path with an explicit
    // mode (the DEPLOYMENT_SITE_MODES constant itself is uniform null in
    // this phase); the defective-name rule is pinned at the vocabulary
    // level via gateActionForMode's fail-closed branch.
    const defective = gateActionForMode('command.approve', {
      modeOverride: 'unlimited' as never,
    });
    expect(defective.allowed).toBe(false);
    expect(defective.reason).toContain('defective');
  });
});

describe('the mode gate per mode (P5.27 steps 1–2)', () => {
  it('Simulation: a mutating action is allowed ONLY against the simulator, labelled SIMULATED', () => {
    const check = gateActionForMode('command.approve');
    expect(check.allowed).toBe(true);
    expect(check.mode).toBe('Simulation');
    expect(check.exercised).toBe(true);
    expect(check.reason).toContain('ONLY against the simulator');
    expect(check.reason).toContain('SIMULATED');
    expect(check.reason).toContain('P4.16');
    expect(check.reason).toContain('no non-simulator issue surface');
  });

  it('Simulation: reads are allowed', () => {
    const check = gateActionForMode('command.read');
    expect(check.allowed).toBe(true);
  });

  it('Read-only/shadow: EVERY mutating action is denied WITH the explanation', () => {
    for (const actionId of ['command.approve', 'schedule.propose', 'asset.update', 'site.create']) {
      const check = gateActionForMode(actionId, { modeOverride: 'Read-only/shadow' });
      expect(check.allowed).toBe(false);
      expect(check.reason).toContain('Read-only/shadow');
      expect(check.reason).toContain('WITHOUT issuing commands');
      expect(check.reason).toContain('DISABLED with this explanation');
      expect(check.reason).toContain('NOT the enforcement');
    }
  });

  it('Read-only/shadow: reads are still allowed (a mode constrains control, not evidence)', () => {
    const check = gateActionForMode('command.read', { modeOverride: 'Read-only/shadow' });
    expect(check.allowed).toBe(true);
  });

  it('Operator-approved / Bounded automation: configured-but-not-exercised, mutating denied, no promotion claimed', () => {
    for (const mode of ['Operator-approved', 'Bounded automation'] as const) {
      const check = gateActionForMode('command.approve', { modeOverride: mode });
      expect(check.allowed).toBe(false);
      expect(check.exercised).toBe(false);
      expect(check.reason).toContain('configuration vocabulary');
      expect(check.reason).toContain('simulation-only');
      expect(check.reason).toContain('configured-but-not-exercised');
      expect(check.reason).toContain('No mode promotion is in scope');
      expect(check.reason).toContain('§22.3');
    }
  });

  it('deny-by-default: an action id off the mirrored surface denies in every mode', () => {
    for (const mode of ['Simulation', 'Read-only/shadow'] as const) {
      const check = gateActionForMode('dispatch.issue', { modeOverride: mode });
      expect(check.allowed).toBe(false);
      expect(check.reason).toContain('deny-by-default');
    }
  });

  it('classifies mutating vs read from the mirrored surface', () => {
    expect(isMutatingAction('command.approve')).toBe(true);
    expect(isMutatingAction('command.validate')).toBe(true);
    expect(isMutatingAction('schedule.propose')).toBe(true);
    expect(isMutatingAction('command.read')).toBe(false);
    expect(isMutatingAction('audit.event.read')).toBe(false);
  });
});

describe('the mode-vs-capability intersection (P5.27 wiring rule)', () => {
  it('allows only when BOTH gates allow (capability allow + Simulation allow)', () => {
    setSession(TENANT_ADMIN);
    const capability = canAction('command.approve', TENANT_ADMIN.role, 'asset');
    expect(capability.allowed).toBe(true);
    const combined = combineChecks(capability, gateActionForMode('command.approve'));
    expect(combined.allowed).toBe(true);
    expect(combined.reason).toContain('The API enforces this independently');
    expect(combined.reason).toContain('ONLY against the simulator');
  });

  it('denies when the mode denies, with BOTH reasons rendered (capability allow + shadow deny)', () => {
    setSession(TENANT_ADMIN);
    const capability = canAction('command.approve', TENANT_ADMIN.role, 'asset');
    const combined = combineChecks(
      capability,
      gateActionForMode('command.approve', { modeOverride: 'Read-only/shadow' }),
    );
    expect(combined.allowed).toBe(false);
    expect(combined.reason).toContain('holds a recorded grant');
    expect(combined.reason).toContain('WITHOUT issuing commands');
  });

  it('denies when the capability denies, with BOTH reasons rendered (viewer + Simulation)', () => {
    setSession(VIEWER);
    const capability = canAction('command.approve', VIEWER.role, 'asset');
    expect(capability.allowed).toBe(false);
    const combined = combineChecks(capability, gateActionForMode('command.approve'));
    expect(combined.allowed).toBe(false);
    expect(combined.reason).toContain('holds no recorded grant');
    expect(combined.reason).toContain('ONLY against the simulator');
  });

  it('denies with no session at all (a context is never fabricated, P0.16 rule 5)', () => {
    const combined = useCanInMode('command.approve', 'asset');
    expect(combined.allowed).toBe(false);
    expect(combined.reason).toContain('no authenticated session');
  });

  it('useCanInMode composes the session-derived capability with the mode gate', () => {
    setSession(TENANT_ADMIN);
    const allowed = useCanInMode('command.approve', 'asset');
    expect(allowed.allowed).toBe(true);
    expect(allowed.mode).toBe('Simulation');
    const shadow = useCanInMode('command.read');
    expect(shadow.allowed).toBe(true);
    expect(shadow.reason).toContain(CAPABILITY_MIRROR_NOTE.slice(0, 40));
  });
});

describe('the honesty statements render verbatim', () => {
  it('the enforcement note states defence-in-depth UI and the server boundary', () => {
    expect(MODE_ENFORCEMENT_NOTE).toContain('DEFENCE-IN-DEPTH UI');
    expect(MODE_ENFORCEMENT_NOTE).toContain('SERVER remains');
    expect(MODE_ENFORCEMENT_NOTE).toContain('P4.16');
    expect(MODE_ENFORCEMENT_NOTE).toContain('acknowledgement is NOT physical delivery');
    expect(MODE_ENFORCEMENT_NOTE).toContain('v1.0.0, not now');
  });

  it('the non-exercised note states the vocabulary-only status of the promoted modes', () => {
    expect(NON_EXERCISED_MODES_NOTE).toContain('configuration vocabulary');
    expect(NON_EXERCISED_MODES_NOTE).toContain('no promotion path exists in this phase');
  });

  it('the mixed-mode note states the per-site display rule', () => {
    expect(MIXED_MODE_NOTE).toContain('deployment AND per-site setting');
    expect(MIXED_MODE_NOTE).toContain('displays the mode per site where they differ');
  });

  it('ModeGateNotice renders the deployed mode, the boundary, and the vocabulary honestly', () => {
    const html = renderToString(<ModeGateNotice />);
    expect(html).toContain('control-authority-mode-gate');
    expect(html).toContain('Simulation');
    expect(html).toContain('DEFENCE-IN-DEPTH UI');
    expect(html).toContain('Operator-approved');
    expect(html).toContain('configured-but-not-exercised');
    expect(html).toContain('deployment AND per-site setting');
  });
});
