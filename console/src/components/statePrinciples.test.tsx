// P5.11 — the state-distinction / feasibility-timestamp / high-impact
//        confirmation design-principle tests (§20.2, §14.1, §12.5; ledger
//        P5.11).
//
// UNRUN — written, never executed (the Phase 5 loop bans: no npm install,
// no vitest run; decision record 10_12_design_pass.md). The human executes
// `npm test` after the first install.
//
// Honest scope note: the reviewer test "a reviewer must be able to tell the
// state from the display alone, without reading a tooltip" is asserted
// structurally here — every lifecycle state has a UNIQUE literal text
// marker rendered INSIDE the badge (not a title/tooltip attribute), so the
// distinguishing information is in the display text itself. Colour is never
// part of the treatment (P5.12).
import { describe, expect, it } from 'vitest';
import { renderToString } from 'react-dom/server';
import {
  COMMAND_STATE_DEFINITIONS,
  COMMAND_STATE_MARKERS,
  COMMAND_STATES,
  formatUtcMillis,
} from './stateVocabulary';
import { CommandStateBadge } from './honesty';
import {
  ConfirmHighImpact,
  HIGH_IMPACT_ACTIONS,
} from './ConfirmHighImpact';
import { RejectionPanel } from '../workspaces/Dispatch';
import { GridView, type GridData } from '../workspaces/Grid';
import type { CommandTransitionDto, FeasibilityCheckDto } from '../api/types';

// --- P5.11 step 1: distinct lifecycle states, shared treatment tokens -------

describe('P5.11 — the 14 lifecycle states are distinct from the display alone', () => {
  it('every state has a marker, and no two states render identically', () => {
    const markers = COMMAND_STATES.map((state) => COMMAND_STATE_MARKERS[state]);
    expect(markers.every((marker) => marker.length > 0)).toBe(true);
    expect(new Set(markers).size).toBe(COMMAND_STATES.length);
  });

  it('each marker carries the state keyword in the display text (not a tooltip)', () => {
    for (const state of COMMAND_STATES) {
      const marker = COMMAND_STATE_MARKERS[state];
      expect(marker.toLowerCase()).toContain(state.toLowerCase());
      expect(marker.startsWith('[')).toBe(true);
    }
  });

  it('the ISSUED/ACKNOWLEDGED markers carry the simulated qualification in the text (P5.13 wording)', () => {
    expect(COMMAND_STATE_MARKERS.ISSUED).toContain('simulator');
    expect(COMMAND_STATE_MARKERS.ACKNOWLEDGED).toContain('simulated');
  });

  it('every state has a recorded meaning rendered under the badge (distinct semantics, not just distinct glyphs)', () => {
    expect(COMMAND_STATE_DEFINITIONS.length).toBe(COMMAND_STATES.length);
    for (const state of COMMAND_STATES) {
      const definition = COMMAND_STATE_DEFINITIONS.find((entry) => entry.state === state);
      expect(definition).toBeDefined();
    }
  });

  it('the badge renders marker + state + meaning (the reviewer reads the state off the page)', () => {
    const html = renderToString(<CommandStateBadge state="PROPOSED" />);
    expect(html).toContain('[proposed ·]');
    expect(html).toContain('PROPOSED');
    expect(html).toContain('never displayed as executed');
  });

  it('an out-of-vocabulary state renders verbatim with an unknown marker (never mapped)', () => {
    const html = renderToString(<CommandStateBadge state="MAYBE" />);
    expect(html).toContain('UNKNOWN STATE');
    expect(html).toContain('MAYBE');
  });
});

// --- P5.11 step 2: model version + timestamp on feasibility outcomes ---------

function checkFixture(overrides: Partial<FeasibilityCheckDto> = {}): FeasibilityCheckDto {
  return {
    check_id: 'check-1',
    tenant_id: 't',
    command_id: 'cmd-1',
    portfolio_id: 'p',
    site_id: 's',
    asset_id: 'a',
    schedule_id: 'sch',
    schedule_revision: 1,
    model_id: 'model-ieee13',
    model_version: 'v3',
    model_content_hash: 'abc123',
    outcome: 'LIMIT_VIOLATED',
    failing_constraint: 'transformer thermal limit',
    reason: undefined,
    margins: undefined,
    coverage: undefined,
    actor: 'gate',
    correlation_id: 'corr-1',
    checked_at_unix_millis: 1765453200000,
    no_execute_endpoint_statement: 'no execute endpoint exists (§15.2)',
    ...overrides,
  };
}

describe('P5.11 — model version + timestamp on every feasibility outcome', () => {
  it('the Grid workspace renders model version + content hash + timestamp per outcome row', () => {
    const data: GridData = { rows: [checkFixture()], consultedCount: 1 };
    const html = renderToString(
      <GridView state={{ status: 'ready', data, readAt: '2026-11-11T10:00:00Z' }} />,
    );
    expect(html).toContain('model-binding');
    expect(html).toContain('v3');
    expect(html).toContain('abc123');
    expect(html).toContain(formatUtcMillis(1765453200000));
    // P5.04 is verified, not re-implemented: the row states the §20.2 rule.
    expect(html).toContain('and timestamp travel with EVERY outcome');
  });

  it('the Dispatch rejection evidence ALSO carries the model version and the checked-at timestamp', () => {
    const transitions: CommandTransitionDto[] = [
      {
        seq: 1,
        from_state: 'FEASIBLE',
        to_state: 'REJECTED',
        actor: 'authorization-gate',
        at_unix_millis: 1765453200001,
        gate: 'authorization',
        reason: 'not authorized by policy',
        evidence_reference: 'authz-ref-1',
      },
    ];
    const html = renderToString(
      <RejectionPanel
        rejection={{
          gate: 'authorization',
          failingCheck: 'not authorized by policy',
          evidenceReference: 'authz-ref-1',
          matchingCheck: checkFixture({ outcome: 'LIMIT_VIOLATED' }),
        }}
      />,
    );
    expect(html).toContain('checked at');
    expect(html).toContain(formatUtcMillis(1765453200000));
    expect(html).toContain('v3');
    expect(html).toContain('transformer thermal limit');
    expect(html).toContain('authz-ref-1');
  });
});

// --- P5.11 step 3: the high-impact list + explicit confirmation --------------

describe('P5.11 — the recorded high-impact action list', () => {
  it('lists approval, cancellation, supersede, and fallback activation', () => {
    const ids = HIGH_IMPACT_ACTIONS.map((action) => action.id);
    expect(ids).toEqual(['approval', 'cancellation', 'supersede', 'fallback-activation']);
  });

  it('approval is the ONLY console-triggerable one; the others state why not', () => {
    for (const action of HIGH_IMPACT_ACTIONS) {
      if (action.id === 'approval') {
        expect(action.consoleTriggerable).toBe(true);
      } else {
        expect(action.consoleTriggerable).toBe(false);
        expect(action.notTriggerableReason).toBeTruthy();
      }
    }
  });
});

describe('P5.11 — the shared explicit-confirmation component', () => {
  it('renders the action, instance, and summary BEFORE the confirmation control', () => {
    const html = renderToString(
      <ConfirmHighImpact
        actionId="approval"
        subject="command cmd-9"
        summary="Approving passes the §14.1 AUTHORIZATION gate."
        confirmed={false}
        onConfirm={() => undefined}
      />,
    );
    const actionIndex = html.indexOf('High-impact action: approval');
    const checkboxIndex = html.indexOf('confirm-high-impact-checkbox');
    expect(actionIndex).toBeGreaterThan(-1);
    expect(checkboxIndex).toBeGreaterThan(actionIndex);
    expect(html).toContain('command cmd-9');
  });

  it('the confirmation control is an explicitly labelled required checkbox (keyboard-operable, not colour/hover)', () => {
    const html = renderToString(
      <ConfirmHighImpact
        actionId="approval"
        subject="command cmd-9"
        summary="summary"
        confirmed={false}
        onConfirm={() => undefined}
      />,
    );
    expect(html).toContain('type="checkbox"');
    expect(html).toContain('aria-required="true"');
    // The label is bound to the input by id (label htmlFor + input id).
    expect(html).toMatch(/<label for="confirm-high-impact-approval">/);
    expect(html).toMatch(/<input[^>]*id="confirm-high-impact-approval"/);
  });

  it('renders the children (the resolution) only after confirmation', () => {
    const unconfirmed = renderToString(
      <ConfirmHighImpact
        actionId="approval"
        subject="s"
        summary="sum"
        confirmed={false}
        onConfirm={() => undefined}
      >
        <p>RESOLUTION-SENTINEL</p>
      </ConfirmHighImpact>,
    );
    const confirmed = renderToString(
      <ConfirmHighImpact
        actionId="approval"
        subject="s"
        summary="sum"
        confirmed={true}
        onConfirm={() => undefined}
      >
        <p>RESOLUTION-SENTINEL</p>
      </ConfirmHighImpact>,
    );
    expect(unconfirmed).not.toContain('RESOLUTION-SENTINEL');
    expect(confirmed).toContain('RESOLUTION-SENTINEL');
  });

  it('an unlisted action id renders fail-honest and offers NO confirmation control', () => {
    const html = renderToString(
      <ConfirmHighImpact
        actionId="deploy-to-field"
        subject="s"
        summary="sum"
        confirmed={false}
        onConfirm={() => undefined}
      />,
    );
    expect(html).toContain('UNKNOWN HIGH-IMPACT ACTION');
    expect(html).toContain('deploy-to-field');
    expect(html).not.toContain('type="checkbox"');
  });
});
