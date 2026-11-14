// P5.13 — the structural honesty guarantee tests (Vitest). UNRUN — never
// executed in the Phase 5 loop (no install/build/test; decision record:
// 13_26_honesty_security.md). The human executes these via `npm test`.
//
// METHOD (stated per the task contract — this is a grep-based STRUCTURAL
// assertion, acceptable and recorded): the test reads the console's own
// source files with node:fs and asserts, structurally, that
//
//   1. the per-state SIMULATED wording has EXACTLY ONE source (the central
//      honesty module) — no workspace keeps its own copy;
//   2. every workspace that displays issue/acknowledgement/tracking data
//      imports and renders the central honesty module's components (the
//      lifecycle badge, the mode indicator, the verbatim statements);
//   3. no UNQUALIFIED "dispatched to" / "executed" / "delivered" /
//      "applied to device" string remains anywhere in the display surface
//      (P5.13 step 3 text review, automated): every source line carrying a
//      banned phrase is either a wire identifier (delivered_at /
//      lastDeliveredAt) or carries a simulator/simulated/never/not
//      qualification on the SAME line — covering error/empty/toast strings
//      too, because the scan is line-level over ALL non-test sources;
//   4. the lifecycle badge carries the SIMULATED qualification itself, so
//      a LIVE-updated ISSUED/ACKNOWLEDGED/TRACKING badge (the P5.25 stream
//      can carry DISPATCH_* events) renders labelled exactly as a fetched
//      state — verified by rendering the badge for the simulator-side
//      states and by pinning classifyEventTag('DISPATCH_*') === 'dispatch';
//   5. the §21.4 control-authority-mode indicator is the ONE shared
//      component and is present in every workspace this subtask owns.
//
// What a structural scan CANNOT prove (recorded honestly): it proves the
// wiring exists in source, not runtime behaviour — the lifecycle-fixture
// display review (P5.13 step 4) and the rendered-string assertions below
// cover the runtime side, and the human's first `npm test` run is the gate.
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { renderToString } from 'react-dom/server';
import {
  ACKNOWLEDGEMENT_NOT_DELIVERY_STATEMENT,
  CommandStateBadge,
  ControlAuthorityModeIndicator,
  SIMULATED_STATE_MEANINGS,
  SIMULATION_ONLY_BOUNDARY_STATEMENT,
} from './honesty';
import { COMMAND_STATE_MARKERS, COMMAND_STATE_DEFINITIONS } from './stateVocabulary';
import { classifyEventTag } from '../live/types';

const SRC_DIR = join(dirname(fileURLToPath(import.meta.url)), '..');

/** All non-test .ts/.tsx sources under src/ (the scan corpus). */
function sourceFiles(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      out.push(...sourceFiles(full));
    } else if (/\.(ts|tsx)$/.test(entry) && !/\.test\.(ts|tsx)$/.test(entry)) {
      out.push(full);
    }
  }
  return out;
}

const read = (relative: string): string =>
  readFileSync(join(SRC_DIR, relative), 'utf-8');

describe('single source of simulation labelling (P5.13 unification)', () => {
  it('the central honesty module defines the per-state SIMULATED meanings', () => {
    expect(SIMULATED_STATE_MEANINGS['ISSUED']).toContain('SIMULATOR');
    expect(SIMULATED_STATE_MEANINGS['ACKNOWLEDGED']).toContain(
      'APPLICATION-LAYER ACCEPTANCE',
    );
    expect(SIMULATED_STATE_MEANINGS['ACKNOWLEDGED']).toContain('NOT physical delivery');
    for (const state of ['TRACKING', 'COMPLETED', 'FAILED', 'FALLBACK']) {
      expect(SIMULATED_STATE_MEANINGS[state]).toContain('SIMULATED');
    }
  });

  it('Dispatch delegates to the central map and keeps NO copy of the wording', () => {
    const dispatch = read('workspaces/Dispatch.tsx');
    expect(dispatch).toContain('SIMULATED_STATE_MEANINGS[state]');
    // The unification removed the per-surface copies (the exact strings now
    // live only in honesty.tsx — pinned by the assertions above).
    expect(dispatch).not.toContain("'SIMULATED: this acknowledgement");
    expect(dispatch).not.toContain("'SIMULATED: this command was issued");
  });

  it('the state-vocabulary meanings carry the same qualification (badge path)', () => {
    const issued = COMMAND_STATE_DEFINITIONS.find((d) => d.state === 'ISSUED');
    const ack = COMMAND_STATE_DEFINITIONS.find((d) => d.state === 'ACKNOWLEDGED');
    expect(issued?.meaning).toContain('SIMULATOR');
    expect(ack?.meaning).toContain('NOT physical delivery');
    // The badge markers themselves carry the label (live updates included).
    expect(COMMAND_STATE_MARKERS.ISSUED).toContain('issued to simulator');
    expect(COMMAND_STATE_MARKERS.ACKNOWLEDGED).toContain('(simulated)');
    expect(COMMAND_STATE_MARKERS.TRACKING).toContain('(simulated)');
  });
});

describe('the simulation-only boundary + acknowledgement wording (P4.32 §2, verbatim)', () => {
  it('renders the handover boundary wording verbatim (key sentences intact)', () => {
    expect(SIMULATION_ONLY_BOUNDARY_STATEMENT).toContain(
      'Every issued command in this platform is issued to the SIMULATOR and nothing else.',
    );
    expect(SIMULATION_ONLY_BOUNDARY_STATEMENT).toContain('There is no device write path');
    expect(SIMULATION_ONLY_BOUNDARY_STATEMENT).toContain('Acknowledgement is NOT physical delivery');
    expect(SIMULATION_ONLY_BOUNDARY_STATEMENT).toContain(
      'Operator-approved field control is v1.0.0, not now; real protocol adapters are Stage 6.',
    );
    expect(ACKNOWLEDGEMENT_NOT_DELIVERY_STATEMENT).toBe('Acknowledgement is NOT physical delivery.');
  });

  it('Dispatch renders the boundary statement and the payload statements verbatim', () => {
    const dispatch = read('workspaces/Dispatch.tsx');
    expect(dispatch).toContain('SIMULATION_ONLY_BOUNDARY_STATEMENT');
    expect(dispatch).toContain('label="simulation-only boundary (P4.32 handover §2)"');
    // The control-plane's simulation_only_statement (P4.30 DTO field) renders
    // verbatim through VerbatimStatement (pre-existing, pinned structurally).
    expect(dispatch).toContain('statement={command.simulation_only_statement}');
    expect(dispatch).toContain('statement={command.no_execute_endpoint_statement}');
  });
});

describe('workspaces route through the honesty module (structural wiring)', () => {
  const WORKSPACES = [
    'workspaces/Dispatch.tsx',
    'workspaces/AuditReports.tsx',
    'workspaces/SimulationLab.tsx',
  ] as const;

  for (const workspace of WORKSPACES) {
    it(`${workspace} imports the central honesty module`, () => {
      expect(read(workspace)).toContain("from '../components/honesty'");
    });
    it(`${workspace} renders the shared §21.4 mode indicator`, () => {
      expect(read(workspace)).toContain('<ControlAuthorityModeIndicator />');
    });
  }
});

describe('the §21.4 control-authority-mode indicator (one shared component)', () => {
  it('renders the Simulation mode with its recorded source note', () => {
    const html = renderToString(<ControlAuthorityModeIndicator />);
    expect(html).toContain('control-authority-mode-indicator');
    // The mode string is asserted via the data attribute and the static
    // prefix separately: renderToString separates adjacent text nodes with
    // comment markers, so a literal spanning the {CONSOLE_CONTROL_AUTHORITY_MODE}
    // expression boundary is not a stable substring of the SSR HTML.
    expect(html).toContain('data-mode="Simulation"');
    expect(html).toContain('Control-authority mode:');
    expect(html).toContain('Simulation');
    expect(html).toContain('never taken from user input');
    expect(html).toContain('P5.27');
  });
});

describe('the P5.13 text review (step 3, automated over ALL non-test sources)', () => {
  /**
   * METHOD: line-level scan of every non-test .ts/.tsx source. A line
   * carrying a banned phrase must be EITHER a wire identifier (the SSE
   * envelope's delivered_at / the hook's lastDeliveredAt — not display
   * strings) OR carry a simulator/simulated/never/not qualification on the
   * SAME line or the line immediately before/after — statement strings are
   * built across multiple source lines (concatenated literals, JSX text),
   * so the qualifier may sit on the continuation line (the scan covers
   * error, empty, and note strings too — they are lines like any other).
   * LIMITATION (recorded): this is a heuristic over source lines, not a
   * parse of the rendered DOM; the runtime wording is pinned separately by
   * the rendered assertions in this file and the workspace test suites.
   */
  const BANNED = /dispatched to|executed|delivered|applied to device/i;
  const IDENTIFIER = /delivered_at|lastDeliveredAt/;
  const QUALIFIER = /simulated|simulator|never|not |no |does not|NOT /i;

  it('no unqualified banned phrase remains in any display source', () => {
    const violations: string[] = [];
    for (const file of sourceFiles(SRC_DIR)) {
      const lines = readFileSync(file, 'utf-8').split('\n');
      const qualified = lines.map((line) => QUALIFIER.test(line));
      lines.forEach((line, index) => {
        if (!BANNED.test(line)) return;
        if (IDENTIFIER.test(line)) return;
        if (
          qualified[index] ||
          (index > 0 && qualified[index - 1]) ||
          (index + 1 < qualified.length && qualified[index + 1])
        ) {
          return;
        }
        violations.push(`${file}:${index + 1}: ${line.trim()}`);
      });
    }
    expect(violations).toEqual([]);
  });
});

describe('live-updated dispatch state stays labelled (P5.13 + P5.25)', () => {
  it('the live stream can carry dispatch lifecycle events', () => {
    expect(classifyEventTag('DISPATCH_ISSUED')).toBe('dispatch');
    expect(classifyEventTag('DISPATCH_ACKNOWLEDGED')).toBe('dispatch');
    expect(classifyEventTag('DISPATCH_TRACKING_UPDATED')).toBe('dispatch');
  });

  it('the badge renders the SIMULATED qualification for the simulator-side states', () => {
    for (const state of ['ISSUED', 'ACKNOWLEDGED', 'TRACKING']) {
      const html = renderToString(<CommandStateBadge state={state} />);
      expect(html).toContain('SIMULATED');
      expect(html).toContain(state);
    }
    const ack = renderToString(<CommandStateBadge state="ACKNOWLEDGED" />);
    expect(ack).toContain('NOT physical delivery');
    const issued = renderToString(<CommandStateBadge state="ISSUED" />);
    expect(issued).toContain('issued to the SIMULATOR');
    expect(issued).not.toContain('issued to device');
  });

  it('the live-label note is rendered where dispatch data is displayed', () => {
    expect(read('workspaces/Dispatch.tsx')).toContain('LIVE_DISPATCH_LABEL_NOTE');
  });
});
