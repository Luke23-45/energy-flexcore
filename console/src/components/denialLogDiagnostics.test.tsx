// P5.26 (FRV1-R3-011) — denial-log diagnostics tests (WRITTEN-UNRUN; the
// human gate runs `npm test` after the first install). Follows the
// P13.17 test conventions: renderToString, data-testid targeting,
// verbatim honesty-record assertions. The tests pin: the block renders
// the count and the last entry from the REAL local log (the same module
// the step-up flow writes), renders the explicit empty state when no
// denial was recorded, and never claims to be the server audit trail.
import { afterEach, describe, expect, it } from 'vitest';
import { renderToString } from 'react-dom/server';
import { DenialLogDiagnostics } from './denialLogDiagnostics';
import {
  clearConsoleDenials,
  recordConsoleDenial,
} from '../security/denialLog';

afterEach(() => {
  clearConsoleDenials();
});

describe('DenialLogDiagnostics (the FRV1-R3-011 display surface)', () => {
  it('with no recorded denials, renders the explicit empty state (never a fabricated entry)', () => {
    clearConsoleDenials();
    const html = renderToString(<DenialLogDiagnostics />);
    expect(html).toContain('denial-log-diagnostics');
    expect(html).toContain('denial-log-empty');
    expect(html).not.toContain('denial-log-last');
    expect(html).toContain('Denials recorded locally this session: <code>0</code>');
  });

  it('renders the count and the LAST entry of the local log (the same reason text the UI rendered)', () => {
    recordConsoleDenial({
      actionId: 'command.approve',
      role: 'operator',
      reason: "role 'operator' holds no recorded grant for 'command.approve'.",
      surface: 'Dispatch ApprovalFlow',
    });
    recordConsoleDenial({
      actionId: 'command.approve',
      role: 'operator',
      reason: 'typed confirmation does not match the action target (c-1)',
      surface: 'Dispatch ApprovalFlow (typed confirmation)',
    });
    const html = renderToString(<DenialLogDiagnostics />);
    expect(html).toContain('Denials recorded locally this session: <code>2</code>');
    expect(html).toContain('denial-log-last');
    // The LAST entry is displayed (the first is not the headline):
    expect(html).toContain('typed confirmation does not match the action target (c-1)');
    expect(html).toContain('Dispatch ApprovalFlow (typed confirmation)');
  });

  it('states the audit boundary honestly: local UI events, never the server audit trail', () => {
    const html = renderToString(<DenialLogDiagnostics />);
    expect(html).toContain('Local denial diagnostics (P5.26, FRV1-R3-011)');
    expect(html).toContain('in memory');
    expect(html).toContain('cleared on');
    expect(html).toContain('never sent to any server');
    expect(html).toContain('NEVER the audit trail');
    expect(html).toContain('P4.28');
  });
});
