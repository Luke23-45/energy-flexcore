# P13.18 — Admin workspace completion (the users/roles/API-keys/webhooks/policy management DESIGN catalog on the Administration scaffold)

| Field | Value |
|---|---|
| Subtask | P13.18 — users/roles/API-keys/webhooks/policy management UI designs on the Administration scaffold — NO write-path fabrication; every management action names the REST endpoint it would call, specified per report §20.1 item 12; where an endpoint does not exist, record specified-not-implemented (plan risk 6). Renders the record-05 role model and the record-08/21 endpoint/secret model (references + fingerprints, display-never-secret). |
| Wave / agent | Phase 13 wave 2, Agent A |
| Status | WRITTEN-UNRUN: the catalog and section are written and never executed; **no management action has ever been performed, no management endpoint has ever been called, no key/endpoint/session/policy has ever been displayed from live data**. Every operational evidence cell is EMPTY/UNRUN. |
| Sources | report §20.1 item 12 (the Administration surface — cited, never edited); record 05 §3 (the Phase 13 grant set + the five-role vocabulary); record 08 §2/§4 (the API-key model; the display rule: fingerprints — the first 8 characters after the prefix — never the secret; the one-time reveal); record 21 §2/§5 (the webhook endpoint model: references + fingerprints; no management endpoint exists); record 42 §4 (the active-session list design); record 41 §2/§5 (the lifecycle checklist + the specified-not-implemented export endpoint); record 03 §2/§3 (the SCIM write path is the IdP's push — the console never provisions); plan risk 6; the console conventions (`Administration.tsx` / `adminData.ts` honesty-record pattern, the P7.25 additive-composition precedent) |
| Files touched | `console/src/workspaces/adminManagement.ts` (NEW), `console/src/workspaces/AdminManagementSection.tsx` (NEW), `console/src/workspaces/adminManagement.test.ts` (NEW), `console/src/workspaces/AdminManagementSection.test.tsx` (NEW), `console/src/App.tsx` (ADDITIVE composition-root edit, marked P13.18: the `AdministrationWithManagement` wrapper renders the P5.07 workspace UNCHANGED with the new section below it — the P7.25 PluginsWorkspace precedent, no existing line modified) |

## 1. The prime rule, applied structurally

**No write-path fabrication.** The section is a TEXT catalog: every
management action names the REST endpoint it would call, every named
endpoint carries the `specified-not-implemented` status, and the section
renders no control that would call anything — no `<button>`, no `onClick`
/ `onSubmit` / `onChange` handler, no `<a href>` to any `/api/v1/` path,
no `fetch` (each pinned by a test: a rendering assertion and a source
scan). Where an action needs no endpoint (`roles.view-grants`,
`policy.retention-values`), it is `display-only` with `endpoint: null`.
No management action has ever been executed.

## 2. What was implemented (written, never run)

| Piece | Where | Honest status |
|---|---|---|
| The five-role vocabulary mirror (`PHASE13_ROLES`, the record-05 extension `site-approver` included) + the record-05 §3 grant-table mirror (`PHASE13_GRANT_TABLE`) + the deliberate-absences notes (incl. the never-granted set, structurally unreachable) + the drift note (THE ENUM IS AUTHORITATIVE — the same recorded limitation as the P5.07/P5.26 mirrors) | `adminManagement.ts` | Written-unrun; mirrors can drift until hand-updated |
| The management-action catalog: 18 actions over 7 surfaces (users / roles / api-keys / webhooks / sessions / policy / tenant-data), each `{id, surface, label, endpoint, endpointStatus, note}`; 13 actions name endpoints — ALL `specified-not-implemented`; 2 actions are `display-only` with no endpoint; the users write path (deactivation) names NO endpoint because it is the IdP's SCIM push (record 03) | same | Written-unrun |
| The display-never-secret rule rendered verbatim (`DISPLAY_NEVER_SECRET_NOTE`): identifiers + fingerprints, never the secret; the one-time reveal belongs to the create/rotate RESPONSE of the (nonexistent) endpoints; never stored retrievable, never logged, never echoed in a GET | same | Written-unrun |
| The section component: the role model, grant table, absent-grants list, the secret rule, and per-surface catalog tables | `AdminManagementSection.tsx` | Written-unrun |
| Composition: `AdministrationWithManagement` renders `<AdministrationWorkspace />` (UNCHANGED) then `<AdminManagementSection />`; the `administration` route entry points at the wrapper | `App.tsx` (additive, marked) | Written-unrun |
| Tests: the role/grant mirror pinned against record 05 §3; every named endpoint pinned specified-not-implemented (the converse set non-empty); the never-granted names absent from the catalog's action ids; the SCIM write-path assertion; the secret-rule assertions; rendering tests; the App composition structural scan; the `Administration.tsx` NOT-modified structural scan | the two test files | Written complete; UNRUN (conventions §1 item 3) |

## 3. The endpoint catalog (the specified-not-implemented register)

Every endpoint below is the RECORDED SHAPE of a future implementation —
no route, handler, or DTO exists, and none was fabricated:

| Action(s) | Endpoint(s) | Consumed model |
|---|---|---|
| users.list | `GET /api/v1/identity/users` | record 03 §2 (the SCIM-fed view) |
| users.role-mapping | `GET /api/v1/identity/role-mappings` | record 03 §3 (the single mapping vocabulary, tenant data) |
| users.deactivate | **none** — the IdP's SCIM push is the write path; the console never provisions | record 03 §2/§4 |
| roles.view-grants | none (`display-only` — the grant table is code) | record 05 §3 |
| apikeys.list / create / rotate / revoke | `GET /api/v1/api-keys`, `POST /api/v1/api-keys`, `POST /api/v1/api-keys/{keyId}/rotate`, `DELETE /api/v1/api-keys/{keyId}` | record 08 §2 (digest-at-rest, one-time reveal, 24 h overlap, immediate revocation) |
| webhooks.list / create / rotate-secret / revoke | `GET /api/v1/webhooks/endpoints`, `POST /api/v1/webhooks/endpoints`, `POST /api/v1/webhooks/endpoints/{endpointId}/rotate-secret`, `DELETE /api/v1/webhooks/endpoints/{endpointId}` | record 21 §2 (references + fingerprints, active flag, rolling overlap; §5 recorded no management endpoint) |
| sessions.list-active / revoke | `GET /api/v1/sessions`, `DELETE /api/v1/sessions/{sessionId}` | record 42 §4 (the list design surface; revocation denies at the resolver) |
| policy.retention-values | none (`display-only` — no read API serves values; the P5.07 statement stands; every P13.13 matrix value cell EMPTY) | records 25/26 |
| policy.runtime-editing | **none, BY DESIGN** — no runtime policy editing exists or is designed (record 05 §4) | record 05 §4 |
| tenant.export | `GET /api/v1/tenants/{tenantId}/export` | record 41 §4/§5 |

The endpoints above are the P13.18 naming of the §20.1 item 12 surface;
their implementation is a later, reviewed batch's work. This record does
not promise them.

## 4. What renders TODAY vs. what the catalog designs

| Renders today (from existing served data) | Designed only (catalog rows) |
|---|---|
| The role/grant mirrors (code-mirrored text) | The users/api-keys/webhooks/sessions lists |
| The absent-grants and never-granted notes | Every create/rotate/revoke management act |
| The retention SHAPE with UNSET values (P5.07, unchanged) | Retention values (none exist; none invented) |
| The display-never-secret rule | Any secret display beyond the designed one-time reveal |

The P5.07 workspace above this section is untouched — its honesty
statements (IDENTITY_STATEMENT, TENANT_READ_STATEMENT, RETENTION_STATEMENT,
D4, edge enrollment) render exactly as before.

## 5. ASSUMPTION / UNRUN register

1. **UNRUN** — every catalog, section, test, and composition. No
   management endpoint exists, no management action was executed, no
   live key/webhook/session was ever displayed. Evidence cells EMPTY.
2. **ASSUMPTION** — the endpoint SHAPES in §3 are this record's naming of
   the future surface; a later implementation may adjust paths (the
   catalog is the display contract to update, not a protocol).
3. **RECORDED LIMITATION** — the console mirrors (roles, grants) can
   drift from the server enums until hand-updated (no generated drift
   test exists for the console mirror — the same recorded limitation as
   the P5.07 action-surface mirror; the server-side enum is pinned by
   its own round-trip tests).
4. **EMPTY** — every operational cell: no key id, no fingerprint, no
   session row, no webhook endpoint, no DLQ depth reading, no retention
   value has ever been rendered from live data.

## 6. Handoff notes

1. **P13.19 (record 45)**: no console test is duplicated there — the
   server-side authz-matrix tests assert the real grants; the mirrors
   here stay display-side.
2. **The human build**: the first console install runs the four new test
   files (with P13.17's); the App-composition scans break if a later
   wave reorders the Administration composition — update the scan, never
   silently.
3. **P13.22 (close)**: the §3 catalog is the consistency-audit input for
   the specified-not-implemented register across records 41/42/43/44 —
   the same endpoints are named in each owning record; no second
   register was created.
