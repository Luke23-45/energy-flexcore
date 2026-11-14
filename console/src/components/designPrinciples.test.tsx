// P5.10 — the cross-workspace freshness/value-type design-principle audit
//        (§20.2; ledger P5.10 steps 2–4).
//
// UNRUN — written, never executed (the Phase 5 loop bans: no npm install,
// no vitest run; decision record 10_12_design_pass.md). The human executes
// `npm test` after the first install.
//
// What this file asserts, honestly:
// 1. A STRUCTURAL audit (grep-style, over the source of every workspace
//    module): every one of the twelve workspace modules renders the §20.2
//    freshness treatment through the shared components (FreshnessLine /
//    FreshnessStamp) or an explicit "Read at {readAt}" caption, and renders
//    the claims-derived RoleScopeLine (the P5.12 extension). This is the
//    "checklist run across every workspace" of P5.10 step 3, implemented as
//    source assertions because rendering every workspace container needs the
//    effect/DOM environment the human's test run provides.
// 2. The FUNCTIONAL core: `valueTypeFor` maps a fixture value of each type
//    to the correct treatment, and no unknown state ever types as measured
//    (fail-open typing is forbidden — the P5.02 risk row).
// 3. The legend (P5.10 step 2) is a UI element that renders ALL five value
//    types + the quality vocabulary + the freshness rule.
// 4. No demo data exists: no workspace module references the demo badge —
//    the badge exists (honesty.tsx) for future use only (P5.12 step 3).
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { renderToString } from 'react-dom/server';
import { ValueTypeBadge, ValueTypeLegend, TREATMENT_RECORD_NOTE, valueTypeFor } from './ValueType';
import { CONSOLE_DEMO_DATA_STATEMENT } from './honesty';

const WORKSPACE_IDS = [
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
] as const;

function workspaceSource(name: string): string {
  return readFileSync(new URL(`../workspaces/${name}.tsx`, import.meta.url), 'utf8');
}

describe('P5.10 structural audit — every workspace renders the §20.2 treatments', () => {
  it('every workspace module renders a freshness treatment (shared component or explicit read-at)', () => {
    const missing: string[] = [];
    for (const name of WORKSPACE_IDS) {
      const source = workspaceSource(name);
      const hasFreshness =
        source.includes('FreshnessLine') ||
        source.includes('FreshnessStamp') ||
        source.includes('{readAt}');
      if (!hasFreshness) missing.push(name);
    }
    expect(missing).toEqual([]);
  });

  it('every workspace module renders the shared claims-derived RoleScopeLine (P5.12 role screens)', () => {
    const missing: string[] = [];
    for (const name of WORKSPACE_IDS) {
      if (!workspaceSource(name).includes('RoleScopeLine')) missing.push(name);
    }
    expect(missing).toEqual([]);
  });

  it('no workspace module renders demo data (the demo badge exists for future use only, P5.12)', () => {
    const offenders: string[] = [];
    for (const name of WORKSPACE_IDS) {
      if (workspaceSource(name).includes('DemoDataBadge')) offenders.push(name);
    }
    expect(offenders).toEqual([]);
    expect(CONSOLE_DEMO_DATA_STATEMENT).toContain('DEMO DATA');
  });

  it('the treatment record is stated (the design note a later contributor cannot silently change)', () => {
    expect(TREATMENT_RECORD_NOTE).toContain('pinned by the');
    expect(TREATMENT_RECORD_NOTE).toContain('design-principles structural test');
  });
});

describe('P5.10 functional core — one value of each type resolves to the correct treatment', () => {
  it('maps quality+provenance onto the four §20.2 types (and the untyped fail-safe)', () => {
    // measured: GOOD quality, not simulated (never served on the Phase 2
    // surface, but the resolution must still be correct for it).
    expect(valueTypeFor('GOOD', false)).toBe('measured');
    expect(valueTypeFor('SUSPECT', false)).toBe('measured');
    // simulated: the simulated provenance flag (every Phase 2 value).
    expect(valueTypeFor('GOOD', true)).toBe('simulated');
    expect(valueTypeFor('SUSPECT', true)).toBe('simulated');
    // estimated / stale: the quality state describes the value itself.
    expect(valueTypeFor('ESTIMATED', true)).toBe('estimated');
    expect(valueTypeFor('STALE', true)).toBe('stale');
    // BAD/MISSING still type from provenance (the value exists as a record).
    expect(valueTypeFor('BAD', true)).toBe('simulated');
    expect(valueTypeFor('MISSING', true)).toBe('simulated');
    // fail-closed: an unknown quality NEVER types as measured.
    expect(valueTypeFor('SOMETHING_ELSE', false)).toBe('untyped');
    expect(valueTypeFor('SOMETHING_ELSE', true)).not.toBe('measured');
  });

  it('renders the literal badge for each type (text-borne treatments, not colour)', () => {
    const html = renderToString(
      <div>
        <ValueTypeBadge type="measured" />
        <ValueTypeBadge type="estimated" />
        <ValueTypeBadge type="stale" />
        <ValueTypeBadge type="simulated" />
        <ValueTypeBadge type="untyped" />
      </div>,
    );
    expect(html).toContain('[measured]');
    expect(html).toContain('[estimated]');
    expect(html).toContain('[stale]');
    expect(html).toContain('[simulated]');
    expect(html).toContain('untyped');
    for (const type of ['measured', 'estimated', 'stale', 'simulated', 'untyped']) {
      expect(html).toContain(`value-type-${type}`);
    }
  });
});

describe('P5.10 legend — the treatments are a UI element, not tribal knowledge', () => {
  it('renders all value types, the quality vocabulary, and the freshness rule', () => {
    const html = renderToString(<ValueTypeLegend />);
    expect(html).toContain('value-type-legend');
    expect(html).toContain('[measured]');
    expect(html).toContain('[estimated]');
    expect(html).toContain('[stale]');
    expect(html).toContain('[simulated]');
    expect(html).toContain('GOOD|SUSPECT|BAD|MISSING|ESTIMATED|STALE');
    expect(html).toContain('Freshness');
    expect(html).toContain('not present in the record');
  });
});
