// P5.26 — the client-side capability mirror of the server authorization
// surface (§17.2–§17.3, §16.2).
//
// WHAT THIS IS (and is not):
// - A hand-written MIRROR of the recorded server grants, used ONLY for the
//   console's UX gating (hide/disable an action + explain the denial —
//   never show a broken button). THE ENFORCEMENT IS THE API (P4.29
//   invariant-7 discipline applied to the UI): the console hiding a button
//   is not the enforcement, and this module never claims to be.
// - THE AUTHORITATIVE SOURCE IS THE SERVER ENUM: `crates/flexcore-authz/
//   src/action.rs` (the closed `Action` enum, P1.11 deny-by-default) plus
//   the recorded grant sets — `phase1..phase5_default_policies` in
//   `apps/control-plane/src/auth.rs`, `phase4_command_policies` in
//   `crates/flexcore-authz/src/phase4_policies.rs`, and the P13.05/P13.08
//   additive set in `crates/flexcore-authz/src/phase13_policies.rs`.
//   This mirror CAN DRIFT until hand-updated (no generated drift test
//   exists in the console scaffold — the same recorded limitation as the
//   Administration workspace's action-surface mirror,
//   07_31_market_plugins_admin_alarms.md §2.3). The drift note is rendered
//   wherever a denial is explained.
// - THIS IS THE SINGLE AUTHORITATIVE ROLE/ACTION MIRROR IN THE CONSOLE
//   (FRV1-R3-003): `console/src/workspaces/adminManagement.ts` derives its
//   role vocabulary from this module (re-export) and its phase-13 grant
//   table must agree with the phase-13 entries below — the pinned tests on
//   both sides assert the agreement. There is exactly one place to update
//   the role vocabulary.
//
// MIRROR DISCIPLINE:
// - Action ids are the `Action::id()` spellings, VERBATIM (lowercase dotted).
// - Role ids are the `Role::id()` spellings, VERBATIM.
// - The matrix records the UNION of the recorded grant sets (a role can act
//   when ANY recorded grant set allows the action). Where the sets disagree
//   (command.read / feasibility.read: the REST read surface grants all four
//   roles at tenant scope, the gate policy set grants three), the union is
//   recorded with the drift note on the entry — never silently narrowed.
// - Scope kinds are recorded per entry; `command.approve` is granted ONLY at
//   site/asset scope (§17.3: resource/site/asset-scoped approval; NO
//   tenant-level approval grant exists in any recorded set — Phase 4 for
//   tenant-admin, Phase 13 for site-approver alike).
// - COMPOSITION STATUS IS RECORDED PER ENTRY (FRV1-R3-003): the shipped
//   control-plane root composes only `phase5_default_policies`
//   (apps/control-plane/src/main.rs — the FRV1-R2-009 note deliberately
//   does NOT compose the phase13/phase16/phase4-command sets there, because
//   no REST route evaluates them). The phase-13 entries below mirror the
//   RECORDED grant set (phase13_policies.rs), and their drift notes state
//   that composition status — the mirror never asserts an enforcement the
//   shipped root does not perform.
// - Deliberately ABSENT (asserted by the tests): `dispatch.authorize`,
//   `dispatch.issue` (§5.7 — they do not exist on the Action surface and
//   must never enter it), `command.issue`, `command.auto_approve`,
//   `alarm.write`, `alarm.acknowledge`, `audit.event.write/delete/export`,
//   `live.stream.read` (the recorded interim gate evaluates
//   `telemetry.read` — auth.rs P5.25 drift note), and the `market.*`
//   family (recorded below as deliberately unmirrored — the phase16 set
//   has no console consumer yet).

/** The closed five-role vocabulary (policy.rs Role::id, verbatim; the
 * P13.05 additive extension `site-approver` included — FRV1-R3-003). */
export const ROLES = [
  'platform-admin',
  'tenant-admin',
  'operator',
  'viewer',
  'site-approver',
] as const;

export type RoleId = (typeof ROLES)[number];

/** The recorded scope kinds (scope.rs ScopeKind::id, verbatim). */
export const SCOPE_KINDS = ['tenant', 'site', 'asset'] as const;

export type ScopeKindId = (typeof SCOPE_KINDS)[number];

export interface CapabilityEntry {
  /** The roles the recorded grants allow (union of the grant sets). */
  roles: readonly RoleId[];
  /** The scope kinds the recorded grants carry. */
  scopeKinds: readonly ScopeKindId[];
  /** Which recorded grant set(s) the entry mirrors. */
  source: string;
  /** The recorded union/drift note, rendered with a denial explanation. */
  driftNote?: string;
}

/**
 * The action → role matrix, mirroring the cumulative recorded grant sets
 * (phase1 → phase5 default policies + the phase4 gate policies). Every id
 * is verbatim from action.rs. THIS ENUM IS AUTHORITATIVE — the mirror is
 * the display-side convenience that can drift.
 */
export const CAPABILITY_MATRIX: Readonly<Record<string, CapabilityEntry>> = {
  // --- Phase 1 (phase1_default_policies) ---
  'tenant.create': {
    roles: ['platform-admin'],
    scopeKinds: ['tenant'],
    source: 'phase1_default_policies',
  },
  'tenant.read': {
    roles: ['platform-admin'],
    scopeKinds: ['tenant'],
    source: 'phase1_default_policies',
  },
  'portfolio.create': {
    roles: ['platform-admin', 'tenant-admin'],
    scopeKinds: ['tenant'],
    source: 'phase1_default_policies',
  },
  'site.create': {
    roles: ['platform-admin', 'tenant-admin'],
    scopeKinds: ['tenant'],
    source: 'phase1_default_policies',
  },
  'asset.create': {
    roles: ['platform-admin', 'tenant-admin'],
    scopeKinds: ['tenant'],
    source: 'phase1_default_policies',
  },
  'portfolio.read': {
    roles: ['platform-admin', 'tenant-admin', 'operator', 'viewer'],
    scopeKinds: ['tenant'],
    source: 'phase1_default_policies',
  },
  'site.read': {
    roles: ['platform-admin', 'tenant-admin', 'operator', 'viewer'],
    scopeKinds: ['tenant', 'site'],
    source: 'phase1_default_policies',
  },
  'asset.read': {
    roles: ['platform-admin', 'tenant-admin', 'operator', 'viewer'],
    scopeKinds: ['tenant', 'asset'],
    source: 'phase1_default_policies',
  },

  // --- Phase 2 (phase2_default_policies) ---
  'tenant.delete': {
    roles: ['platform-admin'],
    scopeKinds: ['tenant'],
    source: 'phase2_default_policies',
  },
  'portfolio.delete': {
    roles: ['platform-admin', 'tenant-admin'],
    scopeKinds: ['tenant'],
    source: 'phase2_default_policies',
  },
  'site.delete': {
    roles: ['platform-admin', 'tenant-admin'],
    scopeKinds: ['tenant'],
    source: 'phase2_default_policies',
  },
  'asset.delete': {
    roles: ['platform-admin', 'tenant-admin'],
    scopeKinds: ['tenant'],
    source: 'phase2_default_policies',
  },
  'asset.update': {
    roles: ['platform-admin', 'tenant-admin'],
    scopeKinds: ['tenant'],
    source: 'phase2_default_policies',
  },
  'asset.deactivate': {
    roles: ['platform-admin', 'tenant-admin'],
    scopeKinds: ['tenant'],
    source: 'phase2_default_policies',
  },
  'telemetry.read': {
    roles: ['platform-admin', 'tenant-admin', 'operator', 'viewer'],
    scopeKinds: ['tenant'],
    source: 'phase2_default_policies (also the recorded interim live-stream gate — auth.rs P5.25 drift note)',
    driftNote:
      'The ledger names a live.stream.read action, but the closed Action enum has no such variant yet; the server evaluates telemetry.read as the interim deny-by-default live-stream gate. This mirror records that fact — it does NOT invent a live.stream.read entry.',
  },
  'quarantine.read': {
    roles: ['platform-admin', 'tenant-admin'],
    scopeKinds: ['tenant'],
    source: 'phase2_default_policies',
  },

  // --- Phase 3 (phase3_default_policies) ---
  'flexibility.envelope.create': {
    roles: ['platform-admin', 'tenant-admin'],
    scopeKinds: ['tenant'],
    source: 'phase3_default_policies',
  },
  'flexibility.envelope.read': {
    roles: ['platform-admin', 'tenant-admin', 'operator', 'viewer'],
    scopeKinds: ['tenant'],
    source: 'phase3_default_policies',
  },
  'schedule.propose': {
    roles: ['platform-admin', 'tenant-admin', 'operator'],
    scopeKinds: ['tenant'],
    source: 'phase3_default_policies',
  },
  'schedule.read': {
    roles: ['platform-admin', 'tenant-admin', 'operator', 'viewer'],
    scopeKinds: ['tenant'],
    source: 'phase3_default_policies',
  },
  'optimization_run.create': {
    roles: ['platform-admin', 'tenant-admin'],
    scopeKinds: ['tenant'],
    source: 'phase3_default_policies',
  },
  'optimization_run.read': {
    roles: ['platform-admin', 'tenant-admin', 'operator', 'viewer'],
    scopeKinds: ['tenant'],
    source: 'phase3_default_policies',
  },

  // --- Phase 4: the gate actions (phase4_command_policies, flexcore-authz) ---
  'command.create': {
    roles: ['operator', 'tenant-admin'],
    scopeKinds: ['tenant', 'site', 'asset'],
    source: 'phase4_command_policies (flexcore-authz/phase4_policies.rs)',
  },
  'command.validate': {
    roles: ['operator', 'tenant-admin'],
    scopeKinds: ['tenant', 'site', 'asset'],
    source: 'phase4_command_policies (flexcore-authz/phase4_policies.rs)',
  },
  // Separation of duties by role (§17.3/P4.13 recorded default + the
  // P13.05 additive extension, FRV1-R3-003): the approving authority is
  // NOT the proposing Operator role; approvals are resource/site/asset-
  // scoped — NO tenant-level approval grant exists in any recorded set.
  // The union of two recorded grant sets: phase4_command_policies grants
  // tenant-admin at site/asset scope; phase13_policies adds SiteApprover
  // at the same scope kinds (phase13_policies.rs never widens existing
  // grants, it only adds approvers).
  'command.approve': {
    roles: ['tenant-admin', 'site-approver'],
    scopeKinds: ['site', 'asset'],
    source:
      'phase4_command_policies (flexcore-authz/phase4_policies.rs, tenant-admin) UNION phase13_policies (flexcore-authz/phase13_policies.rs, site-approver)',
    driftNote:
      'command.approve is granted ONLY to tenant-admin (Phase 4) and site-approver (P13.05), each at site/asset scope; no operator, viewer, or platform-admin grant, and no tenant-level grant, exists in any recorded set. COMPOSITION STATUS (FRV1-R3-003): neither gate set is composed in the shipped control-plane root (main.rs FRV1-R2-009 note — no REST route evaluates command.approve in 0.x; the command gates run via sim-runner), so this entry mirrors the recorded design grants, not a served REST enforcement.',
  },
  // --- Phase 13 (phase13_policies, P13.08) — the apikey.* admin surface.
  // Recorded grant set, deliberately NOT composed in the shipped
  // control-plane root (no apikey.* route exists in 0.x — main.rs
  // FRV1-R2-009 note); the Administration workspace mirrors the same set
  // (adminManagement.ts PHASE13_GRANT_TABLE, kept in sync by pinned test).
  'apikey.create': {
    roles: ['platform-admin', 'tenant-admin'],
    scopeKinds: ['tenant'],
    source: 'phase13_policies (flexcore-authz/phase13_policies.rs, P13.08)',
    driftNote:
      'apikey.create is a RECORDED grant (design-level), deliberately NOT composed in the shipped control-plane root: no apikey.* REST route exists in 0.x (main.rs FRV1-R2-009 note). The API neither grants nor denies it today — the action surface has no route to evaluate.',
  },
  'apikey.rotate': {
    roles: ['platform-admin', 'tenant-admin'],
    scopeKinds: ['tenant'],
    source: 'phase13_policies (flexcore-authz/phase13_policies.rs, P13.08)',
    driftNote:
      'apikey.rotate is a RECORDED grant (design-level), deliberately NOT composed in the shipped control-plane root: no apikey.* REST route exists in 0.x (main.rs FRV1-R2-009 note). The API neither grants nor denies it today — the action surface has no route to evaluate.',
  },
  'apikey.revoke': {
    roles: ['platform-admin', 'tenant-admin'],
    scopeKinds: ['tenant'],
    source: 'phase13_policies (flexcore-authz/phase13_policies.rs, P13.08)',
    driftNote:
      'apikey.revoke is a RECORDED grant (design-level), deliberately NOT composed in the shipped control-plane root: no apikey.* REST route exists in 0.x (main.rs FRV1-R2-009 note). The API neither grants nor denies it today — the action surface has no route to evaluate.',
  },
  'apikey.read': {
    roles: ['platform-admin', 'tenant-admin'],
    scopeKinds: ['tenant'],
    source: 'phase13_policies (flexcore-authz/phase13_policies.rs, P13.08)',
    driftNote:
      'apikey.read is a RECORDED grant (design-level), deliberately NOT composed in the shipped control-plane root: no apikey.* REST route exists in 0.x (main.rs FRV1-R2-009 note). The API neither grants nor denies it today — the action surface has no route to evaluate.',
  },
  // Union of two recorded grant sets: the control-plane REST read surface
  // (phase4_default_policies) grants all four roles at tenant scope; the
  // gate policy set (phase4_command_policies) grants three (no
  // platform-admin). The union is recorded; the drift is stated, never
  // silently narrowed.
  'command.read': {
    roles: ['platform-admin', 'tenant-admin', 'operator', 'viewer'],
    scopeKinds: ['tenant', 'site', 'asset'],
    source:
      'phase4_default_policies (REST read surface, all four roles) UNION phase4_command_policies (gate set, three roles)',
    driftNote:
      'The two recorded grant sets disagree on platform-admin: the REST read surface grants command.read to all four roles at tenant scope; the flexcore-authz gate set grants it to operator/tenant-admin/viewer. The mirror records the union.',
  },
  'feasibility.read': {
    roles: ['platform-admin', 'tenant-admin', 'operator', 'viewer'],
    scopeKinds: ['tenant', 'site', 'asset'],
    source:
      'phase4_default_policies (REST read surface, all four roles) UNION phase4_command_policies (gate set, three roles)',
    driftNote:
      'Same union note as command.read: the REST read surface grants all four roles; the flexcore-authz gate set grants three.',
  },
  'audit.event.read': {
    roles: ['platform-admin', 'tenant-admin', 'operator', 'viewer'],
    scopeKinds: ['tenant'],
    source: 'phase4_default_policies (P4.36)',
  },

  // --- Phase 5 (phase5_default_policies, P5.31) ---
  'alarm.read': {
    roles: ['platform-admin', 'tenant-admin', 'operator', 'viewer'],
    scopeKinds: ['tenant'],
    source: 'phase5_default_policies (P5.31)',
  },
};

/**
 * The action ids that DO NOT exist on the closed server surface and must
 * never appear in this mirror (the §5.7 plugin capability names, the deny
 * tests in action.rs, and the append-only/no-execute absences). The tests
 * pin their absence.
 */
export const FORBIDDEN_ACTION_IDS = [
  'dispatch.authorize',
  'dispatch.issue',
  'command.issue',
  'command.auto_approve',
  'alarm.write',
  'alarm.acknowledge',
  'audit.event.write',
  'audit.event.delete',
  'audit.event.export',
] as const;

/**
 * The recorded server grant sets that are DELIBERATELY UNMIRRORED here
 * (FRV1-R3-003): `phase16_market_policies` (the `market.*` family,
 * crates/flexcore-authz/src/phase16_market_policies.rs) is a recorded
 * design-level set with no console consumer in 0.x — no workspace gates on
 * a market.* action yet, so no entry exists and none is invented. When a
 * console surface first gates on market.*, add the entries here from the
 * Rust source. The phase4_command_policies / phase13 sets ARE mirrored
 * above (the command gates run via sim-runner; the Administration
 * workspace mirrors phase13). Deliberately-unmirrored is NOT absent-by-
 * prohibition: unlike FORBIDDEN_ACTION_IDS, these ids exist on the server
 * Action surface.
 */
export const DELIBERATELY_UNMIRRORED_SETS = [
  {
    setId: 'phase16_market_policies',
    actions: 'market.*',
    reason:
      'recorded design-level grant set (crates/flexcore-authz/src/phase16_market_policies.rs); no console consumer in 0.x — deliberately not mirrored, not composed in the shipped control-plane root (apps/control-plane/src/main.rs FRV1-R2-009 note)',
  },
] as const;

/**
 * THE ENUM IS AUTHORITATIVE — the mirror drift note rendered wherever a
 * capability denial is explained (§17.2: the API enforces; this matrix is
 * the console's UX convenience).
 */
export const CAPABILITY_MIRROR_NOTE =
  'RBAC note: this capability check mirrors the server authorization surface ' +
  '(crates/flexcore-authz action.rs + the recorded phase1–5 grant sets and ' +
  'the phase13 additive set) for console UX only. THE ENUM IS AUTHORITATIVE; ' +
  'the mirror can drift until hand-updated. Enforcement is ALWAYS the API: a ' +
  'request the server denies is denied regardless of what this console ' +
  'displayed, and a denial is audited SERVER-side (§16.2; P4.28 ' +
  'TenantDenialAuditRecord / SecurityAuditSink, ' +
  'crates/flexcore-application/src/dispatch_security.rs) — never by the ' +
  'console.';

export interface CapabilityCheck {
  allowed: boolean;
  /** WHY (the denial explanation; never a bare allow/deny, §10.1). */
  reason: string;
  /** The action id checked (verbatim mirror id). */
  actionId: string;
  /** The role the check ran for (null = no authenticated session). */
  role: string | null;
}

/**
 * The pure capability evaluation behind useCan. Deny-by-default exactly
 * like the server: an unknown action id denies (UnknownAction, P1.11), a
 * missing session denies (a context is never fabricated, P0.16 rule 5),
 * a role outside the recorded grants denies with the recorded scope note.
 */
export function canAction(
  actionId: string,
  role: string | null,
  scopeKind?: ScopeKindId,
): CapabilityCheck {
  const entry = CAPABILITY_MATRIX[actionId];
  if (!entry) {
    return {
      allowed: false,
      reason: `action '${actionId}' is not on the mirrored action surface — deny-by-default (an unknown action denies, P1.11); it may be a §5.7-excluded name or a drift, and the API would deny it with UnknownAction regardless`,
      actionId,
      role,
    };
  }
  if (role === null) {
    return {
      allowed: false,
      reason:
        `no authenticated session — the role cannot be derived from claims (§16.2: context comes only from authenticated claims, never fabricated). ` +
        CAPABILITY_MIRROR_NOTE,
      actionId,
      role,
    };
  }
  if (!(ROLES as readonly string[]).includes(role)) {
    return {
      allowed: false,
      reason: `role '${role}' is not on the recorded role mirror (the closed five-role vocabulary: ${ROLES.join(', ')}) — this mirror knows no such role and denies by default. The API decides INDEPENDENTLY of this mirror (the server enum + grant sets are authoritative; a role the mirror does not know may or may not hold a server-side grant, and the API — not this console — is the enforcement, §17.2).`,
      actionId,
      role,
    };
  }
  if (!entry.roles.includes(role as RoleId)) {
    const recorded = entry.driftNote
      ? ` Recorded grants: ${entry.roles.join(', ')} (${entry.source}). ${entry.driftNote}`
      : ` Recorded grants: ${entry.roles.join(', ')} (${entry.source}).`;
    return {
      allowed: false,
      reason: `role '${role}' holds no recorded grant for '${actionId}'.${recorded} ${CAPABILITY_MIRROR_NOTE}`,
      actionId,
      role,
    };
  }
  if (scopeKind !== undefined && !entry.scopeKinds.includes(scopeKind)) {
    return {
      allowed: false,
      reason: `role '${role}' holds '${actionId}' only at scope kind(s) ${entry.scopeKinds.join(', ')} — not at '${scopeKind}' (recorded grants: ${entry.source}). ${CAPABILITY_MIRROR_NOTE}`,
      actionId,
      role,
    };
  }
  const allowNote = entry.driftNote ? ` ${entry.driftNote}` : '';
  return {
    allowed: true,
    reason: `role '${role}' holds a recorded grant for '${actionId}' (scope kind(s) ${entry.scopeKinds.join(', ')}; ${entry.source}). The API enforces this independently — this console check is UX only.${allowNote}`,
    actionId,
    role,
  };
}
