// P13.18 — the Administration management section (wave 2; WRITTEN-UNRUN —
// never executed; the human gate runs the console tests after the first
// install).
//
// COMPOSITION (the P7.25/P8.27 precedent, applied exactly): this section
// renders BELOW the P5.07 AdministrationWorkspace on the same route
// (App.tsx, the additive marked edit); `Administration.tsx` itself is NOT
// modified. The P5.07 workspace keeps its own honesty record untouched.
//
// THE PRIME RULE (plan risk 6, structural): every management action in
// the catalog NAMES the REST endpoint it would call; every named endpoint
// is marked specified-not-implemented; NO control that would call any
// endpoint is rendered — the catalog renders TEXT rows and status pills
// only. There is no fetch, no button-with-handler, no form anywhere in
// this module (the tests pin it by source scan). Displaying the DESIGN of
// a management surface without fabricating its write path is the entire
// point of this section.
import type { JSX } from 'react';
import {
  ADMIN_MANAGEMENT_NOTE,
  DISPLAY_NEVER_SECRET_NOTE,
  MANAGEMENT_ACTIONS,
  MANAGEMENT_SURFACES,
  PHASE13_ABSENT_GRANTS_NOTES,
  PHASE13_GRANT_TABLE,
  PHASE13_MIRROR_NOTE,
  PHASE13_ROLES,
  SPECIFIED_NOT_IMPLEMENTED,
  type ManagementAction,
} from './adminManagement';

function surfaceTitle(surface: ManagementAction['surface']): string {
  switch (surface) {
    case 'users':
      return 'Users (the SCIM-fed view — the write path is the IdP, never this console)';
    case 'roles':
      return 'Roles and grants (the record 05 model, displayed)';
    case 'api-keys':
      return 'API keys (the record 08 model: identifiers and fingerprints, never secrets)';
    case 'webhooks':
      return 'Webhook endpoints (the record 21 model: references and fingerprints, never secrets)';
    case 'sessions':
      return 'Sessions (the record 42 design: revocation denies at the resolver)';
    case 'policy':
      return 'Policy and retention (no runtime editing exists by design)';
    case 'tenant-data':
      return 'Tenant data (the record 41 lifecycle design)';
  }
}

/**
 * The management catalog for one surface: TEXT rows only — the endpoint
 * (or its absence), the status, and the consumed-model note. No control,
 * no handler, no form (structural).
 */
function SurfaceCatalog({ surface }: { surface: ManagementAction['surface'] }): JSX.Element {
  const rows = MANAGEMENT_ACTIONS.filter((action) => action.surface === surface);
  return (
    <div data-testid={`admin-mgmt-surface-${surface}`}>
      <h4>{surfaceTitle(surface)}</h4>
      <table>
        <caption>
          The {surface} management actions: each names the REST endpoint it
          would call (report §20.1 item 12); {SPECIFIED_NOT_IMPLEMENTED}{' '}
          means that endpoint does not exist and NO control calling it is
          rendered.
        </caption>
        <thead>
          <tr>
            <th scope="col">Action</th>
            <th scope="col">Endpoint it would call</th>
            <th scope="col">Status</th>
            <th scope="col">Consumed model</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((action) => (
            <tr key={action.id} data-testid={`admin-mgmt-action-${action.id}`}>
              <td>{action.label}</td>
              <td>
                {action.endpoint === null ? (
                  <span data-testid={`${action.id}-no-endpoint`}>
                    none (no endpoint is required or permitted for this
                    action)
                  </span>
                ) : (
                  <code>{action.endpoint}</code>
                )}
              </td>
              <td data-testid={`${action.id}-status`}>
                {action.endpointStatus === 'specified-not-implemented'
                  ? SPECIFIED_NOT_IMPLEMENTED
                  : 'display-only'}
              </td>
              <td>{action.note}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/**
 * The P13.18 management section: the role model, the grant table, the
 * management catalog, and the display-never-secret rule. ADDITIVE below
 * the P5.07 workspace; no existing file is modified.
 */
export function AdminManagementSection(): JSX.Element {
  return (
    <section aria-label="Administration management (P13.18 design)" data-testid="admin-management-section">
      <h3>Administration management (P13.18 design catalog)</h3>
      <p className="honesty-banner" role="note" data-testid="admin-mgmt-note">
        {ADMIN_MANAGEMENT_NOTE}
      </p>

      <h4>The five-role vocabulary (record 05 — the P13.05 additive extension included)</h4>
      <p data-testid="admin-mgmt-roles">
        {PHASE13_ROLES.join(', ')} — the closed role vocabulary
        (flexcore-authz policy.rs Role::id spellings, mirrored;{' '}
        {PHASE13_MIRROR_NOTE})
      </p>
      <table data-testid="admin-mgmt-grant-table">
        <caption>
          The recorded Phase 13 grant set (record 05 §3 — the phase13_policies
          mirror; deny-by-default with explicit grants, no role bypasses the
          tenant match).
        </caption>
        <thead>
          <tr>
            <th scope="col">Action</th>
            <th scope="col">Roles</th>
            <th scope="col">Scope kinds</th>
            <th scope="col">Rationale</th>
          </tr>
        </thead>
        <tbody>
          {PHASE13_GRANT_TABLE.map((row) => (
            <tr key={row.actionId}>
              <td>
                <code>{row.actionId}</code>
              </td>
              <td>{row.roles.join(', ')}</td>
              <td>{row.scopeKinds.join(', ')}</td>
              <td>{row.rationale}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <ul data-testid="admin-mgmt-absent-grants">
        {PHASE13_ABSENT_GRANTS_NOTES.map((note) => (
          <li key={note}>{note}</li>
        ))}
      </ul>

      <p data-testid="admin-mgmt-display-never-secret">{DISPLAY_NEVER_SECRET_NOTE}</p>

      {MANAGEMENT_SURFACES.map((surface) => (
        <SurfaceCatalog key={surface} surface={surface} />
      ))}
    </section>
  );
}
