// P13.18 — the Administration workspace completion DESIGN (the users/
// roles/API-keys/webhooks/policy management surfaces; wave 2;
// WRITTEN-UNRUN).
//
// THE PRIME RULE (plan risk 6, applied verbatim): NO write-path
// fabrication. Every management action below NAMES the REST endpoint it
// would call (report §20.1 item 12's Administration surface), and where
// that endpoint does not exist the action is recorded
// SPECIFIED-NOT-IMPLEMENTED — the UI renders the catalog with the status
// column and never renders a control that would call a nonexistent
// endpoint. NOTHING here performs a fetch (structurally; the tests pin
// it). No management action has ever been executed.
//
// CONSUMED MODELS (by citation — never redefined):
// - record 05 §3 (the Phase 13 grant set: SiteApprover's site/asset-scoped
//   `command.approve`; the `apikey.*` tenant-scoped admin surface) and the
//   closed five-role vocabulary it extended;
//
// SINGLE AUTHORITATIVE MIRROR (FRV1-R3-003): the role vocabulary is
// DERIVED, not redeclared — PHASE13_ROLES below re-exports
// `ROLES` from `../security/capabilityMatrix` (the console's one
// authoritative role/action mirror). The PHASE13_GRANT_TABLE mirrors the
// same recorded grant set (phase13_policies.rs) that capabilityMatrix
// carries as its phase-13 entries; the pinned tests on both sides assert
// the two mirrors agree. Update the vocabulary in ONE place only.
// - record 08 §2/§4 (the API-key model: key id `fkid_…`, digest-at-rest,
//   scope subsets, last-used evidence; the display rule: fingerprints —
//   the first 8 characters after the prefix — NEVER the secret; a secret
//   is displayed ONCE, in the create/rotate RESPONSE of the endpoint that
//   does not exist yet);
// - record 21 §2/§5 (the webhook endpoint model: references +
//   fingerprints per the record 08 §4 rule, active flag, the DLQ depth
//   metric; the management ENDPOINT does not exist — record 21 §5);
// - record 42 §4 (the active-session list design surface; the management
//   endpoint is specified-not-implemented);
// - record 41 §2/§5 (the onboarding checklist and its
//   specified-not-implemented export endpoint);
// - record 03 §3 (the SCIM group→role mapping as TENANT DATA — the users/
//   roles surface is a VIEW of provisioning state, never its write path;
//   the write path is the IdP's SCIM push, record 03 §2).

/** The specified-not-implemented status (rendered verbatim per action). */
export const SPECIFIED_NOT_IMPLEMENTED = 'specified-not-implemented';

/** The display-never-secret rule (record 08 §4, quoted as the UI rule). */
export const DISPLAY_NEVER_SECRET_NOTE =
  'Secrets are never displayed after their one-time reveal: management ' +
  'surfaces show the identifier and a FINGERPRINT (the first 8 characters ' +
  'after the prefix, record 08 §4) — never the secret material. A secret ' +
  'is displayed exactly once, in the create/rotate response, and is never ' +
  'stored retrievable, never logged, never echoed in a GET. A management ' +
  'surface that can re-display a secret would be a fabrication this ' +
  'console does not contain.';

/** The drift note for the phase-13 role/action mirrors (the same recorded
 * limitation as the P5.07/P5.26 mirrors: THE ENUM IS AUTHORITATIVE). */
export const PHASE13_MIRROR_NOTE =
  'This section mirrors the Phase 13 additions to the closed server ' +
  'vocabularies (records 05/08; crates/flexcore-authz action.rs and ' +
  'phase13_policies.rs). THE ENUM IS AUTHORITATIVE; the mirror can drift ' +
  'until hand-updated (no generated drift test exists for the console ' +
  'mirror — the same recorded limitation as the P5.07 action-surface ' +
  'mirror). Enforcement is ALWAYS the API.';

import { ROLES } from '../security/capabilityMatrix';

/** The closed five-role vocabulary — DERIVED from the single
 * authoritative role mirror (`ROLES` in ../security/capabilityMatrix,
 * FRV1-R3-003): policy.rs Role::id spellings, verbatim, with the P13.05
 * additive extension `site-approver` included. Do NOT redeclare role ids
 * here; the vocabulary changes in exactly one place. */
export const PHASE13_ROLES: readonly string[] = ROLES;

/** The P13.05 grant-set mirror (record 05 §3, verbatim semantics). This
 * table mirrors the PHASE-13 SET ALONE (what Phase 13 ADDS); the union of
 * all recorded sets for the same actions lives in capabilityMatrix.ts
 * (e.g. its `command.approve` entry also carries the Phase 4 tenant-admin
 * grant). The pinned tests assert the two mirrors never disagree on what
 * Phase 13 grants. */
export interface Phase13GrantRow {
  actionId: string;
  roles: readonly string[];
  scopeKinds: readonly string[];
  rationale: string;
}

export const PHASE13_GRANT_TABLE: readonly Phase13GrantRow[] = [
  {
    actionId: 'command.approve',
    roles: ['site-approver'],
    scopeKinds: ['site', 'asset'],
    rationale:
      'The LR-049 separation: approval duty at resource scope, zero administrative rights; NO tenant-level grant exists for anyone.',
  },
  {
    actionId: 'apikey.create',
    roles: ['platform-admin', 'tenant-admin'],
    scopeKinds: ['tenant'],
    rationale: 'API keys are tenant-scoped credentials administered by tenant/platform administrators (record 08).',
  },
  {
    actionId: 'apikey.rotate',
    roles: ['platform-admin', 'tenant-admin'],
    scopeKinds: ['tenant'],
    rationale: 'Rotation issues a new secret for the same key id (24 h overlap, a registered recommendation).',
  },
  {
    actionId: 'apikey.revoke',
    roles: ['platform-admin', 'tenant-admin'],
    scopeKinds: ['tenant'],
    rationale: 'Revocation is immediate — the resolver consults the key table on every use.',
  },
  {
    actionId: 'apikey.read',
    roles: ['platform-admin', 'tenant-admin'],
    scopeKinds: ['tenant'],
    rationale: 'Read = the key id, scopes, state, and last-used evidence — never the secret material.',
  },
];

/** The deliberately absent grants (record 05 §3; the tests pin them). */
export const PHASE13_ABSENT_GRANTS_NOTES: readonly string[] = [
  'SiteApprover holds NO grant for any registry write, delete, API-key, or command create/validate action.',
  'Operator, Viewer, and SiteApprover hold NO apikey.* grant.',
  'No tenant-level command.approve grant exists for any role.',
  'The never-granted permission set (dispatch.authorize, dispatch.issue, secret.read, network.any) is structurally unreachable — none of the four names exists on the Action surface.',
];

// --- The management-action catalog -------------------------------------------------

export type EndpointStatus =
  /** The action needs no endpoint: it renders code-mirrored or
   * design-recorded data that already exists in the repo (the display IS
   * the deliverable; nothing is called). */
  | 'display-only'
  /** The endpoint the action names does NOT exist on the control plane —
   * the action is the recorded shape of a future implementation, never a
   * fabricated call. */
  | 'specified-not-implemented';

export interface ManagementAction {
  /** The stable action id (the display catalog's key). */
  id: string;
  /** The surface the action belongs to (the §20.1 item 12 groups). */
  surface: 'users' | 'roles' | 'api-keys' | 'webhooks' | 'sessions' | 'policy' | 'tenant-data';
  label: string;
  /** The REST endpoint the action would call (method + path), or null
   * when the action is a pure view of server-derived state. */
  endpoint: string | null;
  /** Whether the endpoint exists TODAY. Every row with an endpoint is
   * 'specified-not-implemented' — no management endpoint exists on the
   * control plane (the registry/telemetry read surface is the only
   * served API); the catalog records the shape each future endpoint
   * would take, never a fabricated call. Rows without an endpoint are
   * 'display-only' (code mirrors and design records) or explicitly
   * absent by design. */
  endpointStatus: EndpointStatus;
  /** The consumed record / model note (rendered with the action). */
  note: string;
}

export const MANAGEMENT_ACTIONS: readonly ManagementAction[] = [
  // --- Users (the SCIM-fed view; the WRITE path is the IdP's SCIM push) ---
  {
    id: 'users.list',
    surface: 'users',
    label: 'List the tenant\'s provisioned users',
    endpoint: 'GET /api/v1/identity/users',
    endpointStatus: 'specified-not-implemented',
    note: 'The users surface is a VIEW of SCIM-provisioned tenant data (record 03 §2); no user list endpoint exists. The user records shown would carry SCIM id, userName, active, and mapped groups.',
  },
  {
    id: 'users.deactivate',
    surface: 'users',
    label: 'Deactivate a user',
    endpoint: null,
    endpointStatus: 'specified-not-implemented',
    note: 'NO console write path: deactivation is the IdP\'s SCIM push (record 03 §2 — `active=false` or DELETE); the console cannot and must not perform provisioning writes. Enforcement lands at the resolver within the recorded deprovisioning window (record 03 §4).',
  },
  {
    id: 'users.role-mapping',
    surface: 'users',
    label: 'View the group→role mapping table',
    endpoint: 'GET /api/v1/identity/role-mappings',
    endpointStatus: 'specified-not-implemented',
    note: 'The mapping is TENANT DATA (record 03 §3) — one group maps to at most one role id of the closed vocabulary; an unmapped group contributes nothing; a user in no mapped group rejects at login (never defaults to a role).',
  },
  // --- Roles (the record 05 model, displayed) ---
  {
    id: 'roles.view-grants',
    surface: 'roles',
    label: 'View the recorded grant table',
    endpoint: null,
    endpointStatus: 'display-only',
    note: 'The grant table is CODE (flexcore-authz phase13_policies.rs + the phase default sets) — the table renders the PHASE13_GRANT_TABLE mirror above; no runtime policy editing exists or is designed (record 05 §4: no rule-expression language, no attribute predicates beyond the typed scope).',
  },
  // --- API keys (record 08 §2) ---
  {
    id: 'apikeys.list',
    surface: 'api-keys',
    label: 'List the tenant\'s API keys',
    endpoint: 'GET /api/v1/api-keys',
    endpointStatus: 'specified-not-implemented',
    note: 'The list would render key id (fkid_…), fingerprint, scopes (a strict subset of the closed Action vocabulary), state, and last-used evidence (last_used_at, last_used_subject_path, last_used_result, use counter) — NEVER the secret.',
  },
  {
    id: 'apikeys.create',
    surface: 'api-keys',
    label: 'Create an API key',
    endpoint: 'POST /api/v1/api-keys',
    endpointStatus: 'specified-not-implemented',
    note: 'The create RESPONSE displays the secret exactly ONCE (fk_…, record 08 §2); the stored row carries the SHA-256 digest. Requires apikey.create at tenant scope (PlatformAdmin/TenantAdmin).',
  },
  {
    id: 'apikeys.rotate',
    surface: 'api-keys',
    label: 'Rotate an API key',
    endpoint: 'POST /api/v1/api-keys/{keyId}/rotate',
    endpointStatus: 'specified-not-implemented',
    note: 'Rotation issues a new secret (shown once in the response); the old digest stays valid through the overlap window (24 h, a registered recommendation) and is revoked when it closes (record 08 §2).',
  },
  {
    id: 'apikeys.revoke',
    surface: 'api-keys',
    label: 'Revoke an API key',
    endpoint: 'DELETE /api/v1/api-keys/{keyId}',
    endpointStatus: 'specified-not-implemented',
    note: 'Revocation is immediate — state → revoked; there is no "works until its next refresh" window (record 08 §2).',
  },
  // --- Webhooks (record 21 §2/§5) ---
  {
    id: 'webhooks.list',
    surface: 'webhooks',
    label: 'List the tenant\'s webhook endpoints',
    endpoint: 'GET /api/v1/webhooks/endpoints',
    endpointStatus: 'specified-not-implemented',
    note: 'The list would render the endpoint id/URL, active flag, the CURRENT and PREVIOUS secret FINGERPRINTS (references + fingerprints per record 08 §4 — the material lives in the P11.16 posture, never in the database), and the last delivery bookkeeping facts (record 21 §2).',
  },
  {
    id: 'webhooks.create',
    surface: 'webhooks',
    label: 'Register a webhook endpoint',
    endpoint: 'POST /api/v1/webhooks/endpoints',
    endpointStatus: 'specified-not-implemented',
    note: 'Creation would bind a signing secret (the record 08 §4 format, generated server-side, displayed ONCE) and store a fingerprint + provider reference — never the material (record 21 §2).',
  },
  {
    id: 'webhooks.rotate-secret',
    surface: 'webhooks',
    label: 'Rotate a webhook signing secret',
    endpoint: 'POST /api/v1/webhooks/endpoints/{endpointId}/rotate-secret',
    endpointStatus: 'specified-not-implemented',
    note: 'Rotation follows the rolling-overlap semantics (current + previous both verify during the overlap; the old secret is revoked when it closes — record 21 §1).',
  },
  {
    id: 'webhooks.revoke',
    surface: 'webhooks',
    label: 'Disable a webhook endpoint',
    endpoint: 'DELETE /api/v1/webhooks/endpoints/{endpointId}',
    endpointStatus: 'specified-not-implemented',
    note: 'Disabling stops delivery; the dead-letter table is NOT auto-purged — its depth is a metric (the P13.09 gauge), never a silent loss (record 21 §1).',
  },
  // --- Sessions (record 42 §4) ---
  {
    id: 'sessions.list-active',
    surface: 'sessions',
    label: 'List a principal\'s active sessions',
    endpoint: 'GET /api/v1/sessions',
    endpointStatus: 'specified-not-implemented',
    note: 'The active-session list design (record 42 §4): sessions that would authenticate right now, ordered by creation, with created-at / last-seen-at / state. The resolver-side list method exists as a library seam; nothing serves it over REST.',
  },
  {
    id: 'sessions.revoke',
    surface: 'sessions',
    label: 'Revoke a session',
    endpoint: 'DELETE /api/v1/sessions/{sessionId}',
    endpointStatus: 'specified-not-implemented',
    note: 'Revocation denies at the RESOLVER from the next request on — no UI cooperation is involved (record 42 §1). Idempotent; an unknown session reference is already in the end state.',
  },
  // --- Policy / retention (the P5.07 honesty carries forward) ---
  {
    id: 'policy.retention-values',
    surface: 'policy',
    label: 'View configured retention values',
    endpoint: null,
    endpointStatus: 'display-only',
    note: 'NO read API serves deployment retention values (the P5.07 statement stands) — the P13.13 per-jurisdiction matrix is DESIGN data with every value cell EMPTY; no value is displayed and none is invented here.',
  },
  {
    id: 'policy.runtime-editing',
    surface: 'policy',
    label: 'Edit the policy set at runtime',
    endpoint: null,
    endpointStatus: 'specified-not-implemented',
    note: 'DOES NOT EXIST BY DESIGN (record 05 §4): no rule-expression language, no runtime policy editing, no override flag, no wildcard. Grants change through recorded code amendments, reviewed like every closed-enum extension.',
  },
  // --- Tenant data (record 41) ---
  {
    id: 'tenant.export',
    surface: 'tenant-data',
    label: 'Export the tenant\'s data (the offboarding evidence pack)',
    endpoint: 'GET /api/v1/tenants/{tenantId}/export',
    endpointStatus: 'specified-not-implemented',
    note: 'The P13.15 §4 export-and-reconcile design: a read-only job over the existing tenant-scoped ports, reusing the record 24 pack format. The endpoint does not exist; the export has never been produced.',
  },
];

/** The actions whose `endpoint` is non-null AND specified-not-implemented
 * (the tests pin: every named endpoint is marked not-implemented — no
 * action claims a served endpoint). */
export function unspecifiedEndpointActions(): ManagementAction[] {
  return MANAGEMENT_ACTIONS.filter(
    (action) => action.endpoint !== null && action.endpointStatus === 'specified-not-implemented',
  );
}

/** The surfaces the catalog covers (the §20.1 item 12 Administration
 * groups this section completes). */
export const MANAGEMENT_SURFACES: readonly ManagementAction['surface'][] = [
  'users',
  'roles',
  'api-keys',
  'webhooks',
  'sessions',
  'policy',
  'tenant-data',
];

/** The section's honesty banner (rendered by AdminManagementSection). */
export const ADMIN_MANAGEMENT_NOTE =
  'Administration management (P13.18): this section is a DESIGN catalog ' +
  'on the P5.07 scaffold. Every management action names the REST endpoint ' +
  'it would call (report §20.1 item 12); where that endpoint does not ' +
  'exist the action is marked specified-not-implemented and NO control ' +
  'that would call it is rendered. No management action has ever been ' +
  'executed. The role/secret models displayed are the record 05/08/21/42 ' +
  'designs, consumed by citation — fingerprints, never secrets.';
