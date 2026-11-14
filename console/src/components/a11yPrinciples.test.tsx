// P5.12 — the accessibility / role-screen / demo-honesty design-principle
//        tests (§20.2, §21.4, §17.3; ledger P5.12).
//
// UNRUN — written, never executed (the Phase 5 loop bans: no npm install,
// no vitest run; decision record 10_12_design_pass.md). The human executes
// `npm test` after the first install.
//
// HONEST LIMITATION (stated here because it bounds everything below): these
// are structural / DOM-attribute assertions over react-dom/server
// renderToString output — they can verify that landmarks, labels, roles,
// and text-borne treatments EXIST, but they cannot run a real screen
// reader, cannot exercise keyboard focus behaviour (no DOM, no effects),
// and cannot replace the §21.4 browser-matrix or keyboard-only walkthrough.
// Real AT verification and the keyboard-only approval workflow are the
// human's gate (recorded in 10_12_design_pass.md and the P5.12 ledger row).
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { renderToString } from 'react-dom/server';
import { App } from '../App';
import { DemoDataBadge, CONSOLE_DEMO_DATA_STATEMENT } from './honesty';
import { ConfirmHighImpact } from './ConfirmHighImpact';
import { COMMAND_STATE_MARKERS, COMMAND_STATES, QUALITY_STATES } from './stateVocabulary';

// --- P5.12 step 2: keyboard reachability + semantic landmarks ----------------

describe('P5.12 — the shell is semantically landmarked with a skip link', () => {
  const html = renderToString(<App />);

  it('renders a skip link as the way past the navigation', () => {
    expect(html).toContain('skip-link');
    expect(html).toContain('Skip to workspace content');
    expect(html).toContain('href="#workspace-main"');
  });

  it('the skip-link target exists (main landmark with the matching id)', () => {
    expect(html).toContain('<main id="workspace-main"');
    // tabIndex={-1} lets the target receive focus programmatically.
    expect(html).toMatch(/<main[^>]*tabindex="-1"/);
  });

  it('the workspace navigation is a labelled landmark with the current page marked', () => {
    expect(html).toContain('<nav aria-label="Workspaces">');
    expect(html).toContain('aria-current="page"');
  });

  it('the persistent state regions are announced (role attributes, not colour)', () => {
    expect(html).toContain('role="note"'); // the control-authority banner
    expect(html).toContain('role="status"'); // the tenant indicator / live status
    expect(html).toContain('value-type-legend'); // the P5.10 legend element
  });

  it('header and main landmarks are present (focus order follows DOM order: skip link → header → nav → main)', () => {
    const skip = html.indexOf('skip-link');
    const header = html.indexOf('<header');
    const nav = html.indexOf('<nav');
    const main = html.indexOf('<main');
    expect(skip).toBeGreaterThan(-1);
    expect(skip).toBeLessThan(header);
    expect(header).toBeLessThan(nav);
    expect(nav).toBeLessThan(main);
  });
});

// --- P5.12 step 2: states/value types not conveyed by colour alone ----------

describe('P5.12 — colour-independence: every state treatment carries its meaning in text', () => {
  it('every lifecycle marker contains the state keyword (glyph/shape + word, never colour alone)', () => {
    for (const state of COMMAND_STATES) {
      const marker = COMMAND_STATE_MARKERS[state];
      expect(marker.toLowerCase()).toContain(state.toLowerCase());
    }
  });

  it('terminal states carry distinct symbols in their markers (shape, not colour)', () => {
    expect(COMMAND_STATE_MARKERS.REJECTED).toContain('✕');
    expect(COMMAND_STATE_MARKERS.FAILED).toContain('✕');
    expect(COMMAND_STATE_MARKERS.EXPIRED).toContain('⏱');
    expect(COMMAND_STATE_MARKERS.CANCELLED).toContain('⌫');
    expect(COMMAND_STATE_MARKERS.SUPERSEDED).toContain('⇄');
    expect(COMMAND_STATE_MARKERS.FALLBACK).toContain('⇅');
  });

  it('the six quality states exist as a closed text vocabulary (the badge spells the code)', () => {
    expect(QUALITY_STATES).toEqual([
      'GOOD',
      'SUSPECT',
      'BAD',
      'MISSING',
      'ESTIMATED',
      'STALE',
    ]);
  });
});

// --- P5.12 step 2: labelled interactive elements ------------------------------

describe('P5.12 — interactive elements are labelled', () => {
  it('the shared confirmation control is bound to its visible label (htmlFor/id)', () => {
    const html = renderToString(
      <ConfirmHighImpact
        actionId="approval"
        subject="command cmd-9"
        summary="sum"
        confirmed={false}
        onConfirm={() => undefined}
      />,
    );
    expect(html).toMatch(/<label for="confirm-high-impact-approval">/);
    expect(html).toMatch(/<input[^>]*id="confirm-high-impact-approval"/);
    expect(html).toContain('I confirm this high-impact action');
  });

  it('the workspaces label their interactive controls in source (structural check)', () => {
    // The containers' inputs/selects sit inside <label> elements; the
    // audit's structural form asserts the pattern is present per module.
    const labelledModules = [
      ['../workspaces/Dispatch.tsx', 'Filter by lifecycle state'],
      ['../workspaces/Schedules.tsx', 'Candidate record to inspect'],
      ['../workspaces/AuditReports.tsx', 'Command id (object_id)'],
    ] as const;
    for (const [path, marker] of labelledModules) {
      const source = readFileSync(new URL(path, import.meta.url), 'utf8');
      expect(source).toContain(marker);
    }
  });
});

// --- P5.12 step 1: role-specific screens --------------------------------------

describe('P5.12 — the role model is the closed authz vocabulary, displayed per workspace', () => {
  it('the console does not invent roles: the vocabulary is the four authz roles', () => {
    // Mirrors crates/flexcore-authz/src/policy.rs (adminData.ts AUTHZ_ROLES).
    // The subtask brief's "auditor" maps onto the audit-read actions a role
    // holds — there is NO auditor role in the vocabulary, and none is added.
    const source = readFileSync(new URL('../workspaces/adminData.ts', import.meta.url), 'utf8');
    expect(source).toContain("'platform-admin'");
    expect(source).toContain("'tenant-admin'");
    expect(source).toContain("'operator'");
    expect(source).toContain("'viewer'");
    expect(source).not.toContain("'auditor'");
  });

  it('every workspace renders the claims-derived RoleScopeLine (structural audit)', () => {
    const workspaces = [
      'Overview',
      'Portfolios',
      'Sites',
      'Assets',
      'Grid',
      'Schedules',
      'Dispatch',
      'MarketPrograms',
      'SimulationLab',
      'PluginsIntegrations',
      'AuditReports',
      'Administration',
    ];
    const missing: string[] = [];
    for (const name of workspaces) {
      const source = readFileSync(new URL(`../workspaces/${name}.tsx`, import.meta.url), 'utf8');
      if (!source.includes('RoleScopeLine')) missing.push(name);
    }
    expect(missing).toEqual([]);
  });
});

// --- P5.12 step 3: demo-data honesty -------------------------------------------

describe('P5.12 — demo-data honesty: no demo data exists; the badge is defined for future use', () => {
  it('the badge renders the demo statement as text (role="note", not colour)', () => {
    const html = renderToString(<DemoDataBadge />);
    expect(html).toContain('demo-data-badge');
    expect(html).toContain('DEMO DATA');
    expect(html).toContain('not a live measurement');
    expect(html).toContain('role="note"');
  });

  it('the statement commits to the rule: a demo value is never a live/measured/platform-recorded value', () => {
    expect(CONSOLE_DEMO_DATA_STATEMENT).toContain(
      'No demo value is a live, measured, or platform-recorded value',
    );
  });
});
