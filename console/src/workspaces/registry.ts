// P5.01 — the §20.1 workspace registry: the twelve console areas, in
// report order. Each entry has ONE component. All twelve workspaces are
// IMPLEMENTED (P5.02–P5.09 and the P6/P7/P8/P13 additive extensions — see
// README.md for the shipped surface): every entry renders a real
// component with tenant-scoped API reads and honest empty/error/loading
// states — no fabricated data, no remaining "NOT IMPLEMENTED YET"
// placeholders. (./WorkspacePlaceholder.tsx is retained as the recorded
// pattern for any future stub, but has no consumers.)
//
// This registry is the single source of truth for the router, the
// navigation, and the tests that pin the §20.1 coverage (registry.test.ts
// asserts EXACTLY these twelve, in report order).

export interface WorkspaceDefinition {
  /** Route id (also the hash path segment). */
  id: string;
  /** The §20.1 item number this workspace maps to. */
  reportItem: number;
  /** Display title (the §20.1 item name). */
  title: string;
  /** The Phase 5 subtask(s) that build this workspace. */
  owningSubtasks: string;
}

export type WorkspaceId = string;

export const WORKSPACES: readonly WorkspaceDefinition[] = [
  { id: 'overview', reportItem: 1, title: 'Overview', owningSubtasks: 'P5.02 (alarm gap: P5.31)' },
  { id: 'portfolios', reportItem: 2, title: 'Portfolios', owningSubtasks: 'P5.03' },
  { id: 'sites', reportItem: 3, title: 'Sites', owningSubtasks: 'P5.03' },
  { id: 'assets', reportItem: 4, title: 'Assets', owningSubtasks: 'P5.03' },
  { id: 'grid', reportItem: 5, title: 'Grid', owningSubtasks: 'P5.04' },
  { id: 'schedules', reportItem: 6, title: 'Schedules', owningSubtasks: 'P5.05' },
  { id: 'dispatch', reportItem: 7, title: 'Dispatch', owningSubtasks: 'P5.06' },
  { id: 'market-programs', reportItem: 8, title: 'Market & Programs', owningSubtasks: 'P5.07' },
  { id: 'simulation-lab', reportItem: 9, title: 'Simulation Lab', owningSubtasks: 'P5.08' },
  { id: 'plugins-integrations', reportItem: 10, title: 'Plugins & Integrations', owningSubtasks: 'P5.07' },
  { id: 'audit-reports', reportItem: 11, title: 'Audit & Reports', owningSubtasks: 'P5.09' },
  { id: 'administration', reportItem: 12, title: 'Administration', owningSubtasks: 'P5.07' },
] as const;
