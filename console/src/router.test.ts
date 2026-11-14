// P5.01 — router tests. UNRUN — human executes via `npm test` after
// install (the Phase 5 loop bans installs/builds/test runs; decision
// record: ledger/phase-05-operator-console-simulator/01_25_scaffold_live.md).
import { describe, expect, it } from 'vitest';
import { parseRoute, routeHash } from './router';
import { WORKSPACES } from './workspaces/registry';

describe('parseRoute (hand-rolled hash router)', () => {
  it('maps every registered workspace route to its id', () => {
    for (const workspace of WORKSPACES) {
      expect(parseRoute(`#/${workspace.id}`)).toBe(workspace.id);
    }
  });

  it('accepts the bare-hash and no-hash forms', () => {
    expect(parseRoute('#/overview')).toBe('overview');
    expect(parseRoute('')).toBe('overview'); // default route
    expect(parseRoute('#')).toBe('overview');
    expect(parseRoute('#/')).toBe('overview');
  });

  it('defaults unknown routes to overview (never a broken route)', () => {
    expect(parseRoute('#/not-a-workspace')).toBe('overview');
    expect(parseRoute('#/grid/extra/segments')).toBe('grid');
  });

  it('ignores query fragments after the route segment', () => {
    expect(parseRoute('#/grid?asset=1')).toBe('grid');
  });
});

describe('routeHash', () => {
  it('round-trips with parseRoute for every workspace', () => {
    for (const workspace of WORKSPACES) {
      expect(parseRoute(routeHash(workspace.id))).toBe(workspace.id);
    }
  });
});
