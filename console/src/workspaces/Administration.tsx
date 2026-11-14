// P5.07 — the Administration workspace (report §20.1 item 12): built
// HONESTLY — it shows what exists (the P2.12 tenant read, the mirrored
// closed action vocabulary with its drift note, the P4.30 retention
// configuration SHAPE with unset defaults, the D4 OPEN status line) and
// states what does not exist as an explicit gap statement (no production
// identity integration, no deployment-configuration read API for retention
// values, no real edge enrollment). No fabricated administrative panel
// anywhere (ledger P5.07).
import type { JSX } from 'react';
import type { ApiClient } from '../api/client';
import type { TenantDto } from '../api/types';
import { fetchSiteView, fetchSites, type SiteViewDto } from '../api/views';
import { listTenants } from '../api/registry';
import { FreshnessLine } from '../components/ValueType';
import {
  ErrorBlock,
  HonestEmpty,
  LoadingLine,
  RoleScopeLine,
  errorText,
  useWorkspaceData,
  type LoadState,
} from '../components/workspaceData';
import {
  AUTHZ_ACTION_SURFACE,
  AUTHZ_EXCLUDED_ACTIONS,
  AUTHZ_ROLES,
  D4_STATEMENT,
  EDGE_ENROLLMENT_STATEMENT,
  IDENTITY_STATEMENT,
  RETENTION_CLASSES,
  RETENTION_STATEMENT,
  TENANT_READ_STATEMENT,
  edgeEnrollmentRows,
  type EdgeEnrollmentRow,
} from './adminData';

export interface AdministrationData {
  tenant: TenantDto | null;
  tenantError: string | null;
  enrollmentRows: EdgeEnrollmentRow[];
}

/**
 * The tenant-scoped read: the caller's own tenant (P2.12 read-only grants)
 * plus the per-site edge-enrollment records. A failed tenant read is kept
 * as an honest error, never an invented tenant.
 */
export async function loadAdministration(client: ApiClient): Promise<AdministrationData> {
  const [tenantResult, sites] = await Promise.all([
    listTenants(client)
      .then((page) => ({ tenant: (page.data[0] ?? null) as TenantDto | null, tenantError: null as string | null }))
      .catch((error: unknown) => ({ tenant: null as TenantDto | null, tenantError: errorText(error) })),
    fetchSites(client),
  ]);
  const enrollmentRows = await Promise.all(
    sites.map(async (site): Promise<EdgeEnrollmentRow> => {
      try {
        const view: SiteViewDto = await fetchSiteView(site.id, client);
        return { site, view, viewError: null };
      } catch (error: unknown) {
        return { site, view: null, viewError: errorText(error) };
      }
    }),
  );
  return {
    tenant: tenantResult.tenant,
    tenantError: tenantResult.tenantError,
    enrollmentRows: edgeEnrollmentRows(sites, enrollmentRows.map((r) => r.view), enrollmentRows.map((r) => r.viewError)),
  };
}

/** The workspace shell the router renders (P5.01 registry pattern; the
 * export name is unchanged from the P5.01 stub). */
export function AdministrationWorkspace(): JSX.Element {
  const state = useWorkspaceData(loadAdministration);
  return <AdministrationView state={state} />;
}

export function AdministrationView({ state }: { state: LoadState<AdministrationData> }): JSX.Element {
  return (
    <section aria-label="Administration">
      <h2>Administration</h2>
      <p className="honesty-banner" role="note" data-testid="admin-honesty-banner">
        This workspace is built HONESTLY (ledger P5.07): it shows what
        exists (tenant record, action vocabulary mirror, retention
        configuration shape, D4 status) and states what does not exist — no
        fabricated identity integration, retention value, or edge
        enrollment appears anywhere on it.
      </p>
      <RoleScopeLine />
      {state.status === 'loading' && <LoadingLine what="the administration data" />}
      {state.status === 'error' && <ErrorBlock what="The administration data" error={state.error} />}
      {state.status === 'ready' && <AdministrationReady data={state.data} readAt={state.readAt} />}
    </section>
  );
}

function AdministrationReady({ data, readAt }: { data: AdministrationData; readAt: string }): JSX.Element {
  return (
    <>
      <h3>Identity integration</h3>
      <HonestEmpty
        what="Production identity integration"
        reason={IDENTITY_STATEMENT}
        testId="identity-gap"
      />

      <h3>Tenants (P2.12 grants, read-only)</h3>
      {data.tenantError !== null ? (
        <div role="note" data-testid="tenant-read-error">
          <p>The tenant read failed; no tenant is shown and none is invented.</p>
          <p>
            API error (verbatim): <code>{data.tenantError}</code>
          </p>
        </div>
      ) : data.tenant === null ? (
        <HonestEmpty
          what="Tenant record"
          reason="The tenant-scoped read returned no tenant record. No tenant is fabricated."
        />
      ) : (
        <table>
          <caption>
            The caller's own tenant (GET /api/v1/tenants lists exactly this —
            no platform-wide enumeration exists). Read at {readAt}.
          </caption>
          <thead>
            <tr>
              <th scope="col">Name</th>
              <th scope="col">Identity-provider reference</th>
              <th scope="col">Policy profile</th>
              <th scope="col">Created</th>
            </tr>
          </thead>
          <tbody>
            <tr key={data.tenant.id}>
              <td>{data.tenant.name}</td>
              <td>
                <code>{data.tenant.identity_provider_reference}</code>
              </td>
              <td>
                <code>{data.tenant.policy_profile}</code>
              </td>
              <td>{data.tenant.created_at}</td>
            </tr>
          </tbody>
        </table>
      )}
      <p>{TENANT_READ_STATEMENT}</p>

      <h3>Roles and the action vocabulary (display mirror with drift note)</h3>
      <div data-testid="authz-mirror">
        <p>
          Roles (flexcore-authz policy.rs, mirrored): {AUTHZ_ROLES.join(', ')}.
          Every grant is deny-by-default with an explicit rule reference; no
          role bypasses the tenant match (breadth comes from rule grants,
          never from a bypass).
        </p>
        <table>
          <caption>
            The closed action vocabulary (flexcore-authz action.rs
            Action::id()), mirrored as of P5.31. THE ENUM IS AUTHORITATIVE:
            this is a display MIRROR — a drift means this table must be
            updated by hand; no generated drift test exists for the console
            mirror (recorded limitation; the server-side enum is pinned by
            its own round-trip tests). `dispatch.authorize` and
            `dispatch.issue` do not exist on the surface (§5.7) and never
            may.
          </caption>
          <thead>
            <tr>
              <th scope="col">Phase</th>
              <th scope="col">Actions (canonical dotted ids)</th>
            </tr>
          </thead>
          <tbody>
            {AUTHZ_ACTION_SURFACE.map((group) => (
              <tr key={group.phase}>
                <td>{group.phase}</td>
                <td>
                  {group.actions.map((action) => (
                    <code key={action} style={{ marginRight: '0.5rem' }}>
                      {action}
                    </code>
                  ))}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        <p>
          Deliberately ABSENT (§5.7 — these must never enter the surface):
          {AUTHZ_EXCLUDED_ACTIONS.map((action) => (
            <code key={action} style={{ marginRight: '0.5rem' }}>
              {action}
            </code>
          ))}
        </p>
      </div>

      <h3>Data retention (configured per-class SHAPE — §16.3)</h3>
      <div data-testid="retention-classes">
        <p>{RETENTION_STATEMENT}</p>
        <table>
          <caption>
            The seven §16.3 retention classes (RetentionPolicy field names,
            mirrored verbatim from flexcore-storage/retention.rs). Each is
            UNSET by default in code: "no decision has been supplied by the
            deployment configuration" — and no read API serves deployment
            values in this phase, so no value is shown.
          </caption>
          <thead>
            <tr>
              <th scope="col">Data class (configuration field)</th>
              <th scope="col">Configured value</th>
            </tr>
          </thead>
          <tbody>
            {RETENTION_CLASSES.map((cls) => (
              <tr key={cls}>
                <td>
                  <code>{cls}</code>
                </td>
                <td>
                  <code>UNSET (not served by any API in this phase)</code>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <h3>Edge enrollment (D5 OPEN — lab state only)</h3>
      <p>{EDGE_ENROLLMENT_STATEMENT}</p>
      {data.enrollmentRows.length === 0 ? (
        <HonestEmpty
          what="Edge enrollment"
          reason="No site was returned by the tenant-scoped read, so no edge-identity record exists to show. No enrollment is fabricated."
        />
      ) : (
        <ul>
          {data.enrollmentRows.map((row) => (
            <li key={row.site.id} data-testid={`edge-enrollment-${row.site.id}`}>
              {row.viewError !== null ? (
                <>
                  {row.site.name}: the site view could not be read —{' '}
                  <code>{row.viewError}</code> (shown, never dropped; no
                  enrollment state is inferred from the failure).
                </>
              ) : (
                <>
                  {row.site.name}: registry edge identity{' '}
                  <code>{row.site.edge_identity}</code>, connectivity{' '}
                  <code>{row.view?.edge_connectivity.status ?? 'not_connected'}</code>, live{' '}
                  <code>{String(row.view?.edge_connectivity.live ?? false)}</code> (P2.17
                  record, verbatim — the simulation/lab state, never a real
                  enrollment).
                </>
              )}
            </li>
          ))}
        </ul>
      )}

      {/* P5.10: the edge-enrollment list's freshness anchor (§20.2) — the
          lab-state records above are as of this console read. */}
      <p>
        <FreshnessLine readAt={readAt} />
      </p>

      <h3>License audit status (D4)</h3>
      <div role="note" data-testid="d4-status">
        <p>{D4_STATEMENT}</p>
      </div>
    </>
  );
}
