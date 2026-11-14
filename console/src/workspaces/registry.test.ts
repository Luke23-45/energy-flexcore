// P5.01 — the §20.1 coverage test: EXACTLY the twelve workspaces, in
// report order, all honest stubs. UNRUN — human executes via `npm test`
// after install (decision record: 01_25_scaffold_live.md).
import { describe, expect, it } from 'vitest';
import { WORKSPACES } from './registry';

const REPORT_ITEM_TITLES: Record<number, string> = {
  1: 'Overview',
  2: 'Portfolios',
  3: 'Sites',
  4: 'Assets',
  5: 'Grid',
  6: 'Schedules',
  7: 'Dispatch',
  8: 'Market & Programs',
  9: 'Simulation Lab',
  10: 'Plugins & Integrations',
  11: 'Audit & Reports',
  12: 'Administration',
};

describe('the §20.1 workspace registry', () => {
  it('registers exactly twelve workspaces', () => {
    expect(WORKSPACES).toHaveLength(12);
  });

  it('covers report items 1..12 in order, with the §20.1 titles', () => {
    expect(WORKSPACES.map((w) => w.reportItem)).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    for (const workspace of WORKSPACES) {
      expect(workspace.title).toBe(REPORT_ITEM_TITLES[workspace.reportItem]);
    }
  });

  it('uses unique route ids', () => {
    const ids = WORKSPACES.map((w) => w.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it('every workspace names its owning Phase 5 subtask (traceability)', () => {
    for (const workspace of WORKSPACES) {
      expect(workspace.owningSubtasks).toMatch(/P5\.\d+/);
    }
  });
});
