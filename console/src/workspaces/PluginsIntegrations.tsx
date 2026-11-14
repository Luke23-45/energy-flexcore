// P5.07 — the Plugins & Integrations workspace (report §20.1 item 10, §5,
// §5.7): built HONESTLY. It shows the integrations that DO exist (the two
// platform-internal simulator adapter integrations of P5.14/P5.15, plus
// the API-backed per-site edge-connectivity report) and states what does
// NOT exist as an explicit gap statement: no plugin kernel, no plugin SDK
// (Stage 7, v0.5.0), no installed plugin, no plugin registry API. The §5.7
// plugin-authority rules are rendered as a read-only statement. No plugin
// is ever shown as installed, able to run, or able to bypass the platform
// invariants (ledger P5.07; 29_safety_invariant_matrix.md residual gap 3).
import type { JSX } from 'react';
import type { ApiClient } from '../api/client';
import { fetchSiteView, fetchSites, type SiteViewDto } from '../api/views';
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
  PLUGIN_AUTHORITY_STATEMENT,
  PLUGIN_KERNEL_STATEMENT,
  edgeConnectivityRows,
  edgeStatusOf,
  registeredIntegrations,
  type EdgeConnectivityRow,
} from './pluginsData';

export interface PluginsIntegrationsData {
  rows: EdgeConnectivityRow[];
}

/**
 * The tenant-scoped read: the per-site edge-connectivity report (the
 * API-backed record of the simulator integration state). A failed view
 * read is kept as an honest per-row error, never dropped.
 */
export async function loadPluginsIntegrations(client: ApiClient): Promise<PluginsIntegrationsData> {
  const sites = await fetchSites(client);
  const rows = await Promise.all(
    sites.map(async (site): Promise<EdgeConnectivityRow> => {
      try {
        const view: SiteViewDto = await fetchSiteView(site.id, client);
        return { site, view, viewError: null };
      } catch (error: unknown) {
        return { site, view: null, viewError: errorText(error) };
      }
    }),
  );
  // The rows are already aligned per site; edgeConnectivityRows is the
  // pure re-mapping used by the tests (same output, no hidden I/O).
  return { rows: edgeConnectivityRows(sites, rows.map((r) => r.view), rows.map((r) => r.viewError)) };
}

/** The workspace shell the router renders (P5.01 registry pattern; the
 * export name is unchanged from the P5.01 stub). */
export function PluginsIntegrationsWorkspace(): JSX.Element {
  const state = useWorkspaceData(loadPluginsIntegrations);
  return <PluginsIntegrationsView state={state} />;
}

export function PluginsIntegrationsView({
  state,
}: {
  state: LoadState<PluginsIntegrationsData>;
}): JSX.Element {
  return (
    <section aria-label="Plugins & Integrations">
      <h2>Plugins &amp; Integrations</h2>
      <p className="honesty-banner" role="note" data-testid="plugins-honesty-banner">
        This workspace is built HONESTLY (ledger P5.07): it shows only
        integrations that actually exist and states explicitly that no
        plugin system exists yet — no plugin is ever shown as installed.
      </p>
      <RoleScopeLine />
      {state.status === 'loading' && <LoadingLine what="the integrations data" />}
      {state.status === 'error' && <ErrorBlock what="The site registry" error={state.error} />}
      {state.status === 'ready' && (
        <PluginsIntegrationsReady data={state.data} readAt={state.readAt} />
      )}
    </section>
  );
}

function PluginsIntegrationsReady({
  data,
  readAt,
}: {
  data: PluginsIntegrationsData;
  readAt: string;
}): JSX.Element {
  const integrations = registeredIntegrations();
  return (
    <>
      <h3>Installed plugins</h3>
      <HonestEmpty
        what="Installed plugins"
        reason={PLUGIN_KERNEL_STATEMENT}
        testId="plugins-gap"
      />

      <h3>Plugin authority rules (§5.7, read-only statement)</h3>
      <div role="note" data-testid="plugin-authority-rules">
        <p>{PLUGIN_AUTHORITY_STATEMENT}</p>
        <p>
          These rules are stated as the contract that WILL bind the Stage 7
          plugin host; they are not a claim that any plugin exists, and no
          capability or permission matrix is fabricated for plugins that do
          not exist (the ledger's manifest-comparison check has no manifest
          to compare — the empty state above is the honest comparison
          result).
        </p>
      </div>

      <h3>Registered integrations (platform-internal, not plugins)</h3>
      <table>
        <caption>
          The integrations that DO exist in this phase. Read at {readAt}.
          Each is a platform-INTERNAL simulator adapter (P5.14/P5.15,
          decision record 14_15_adapters.md) wired through the platform's
          adapter interfaces — none is a plugin, none is a registered
          third-party extension, and no plugin manifest system exists to
          register anything.
        </caption>
        <thead>
          <tr>
            <th scope="col">Integration</th>
            <th scope="col">Role (what it does)</th>
            <th scope="col">Origin (subtask / record)</th>
            <th scope="col">Plugin?</th>
            <th scope="col">Scope statement</th>
          </tr>
        </thead>
        <tbody>
          {integrations.map((integration) => (
            <tr key={integration.name}>
              <td>{integration.name}</td>
              <td>{integration.role}</td>
              <td>{integration.originSubtask}</td>
              <td>
                <code>no — platform-internal</code>
              </td>
              <td>{integration.scopeStatement}</td>
            </tr>
          ))}
        </tbody>
      </table>

      <h3>Site edge connectivity (the API-backed integration state)</h3>
      {data.rows.length === 0 ? (
        <HonestEmpty
          what="Site edge connectivity"
          reason="No site was returned by the tenant-scoped read, so no edge-connectivity record exists to show. No site and no connection is fabricated."
        />
      ) : (
        <ul>
          {data.rows.map((row) => {
            const status = edgeStatusOf(row);
            return (
              <li key={row.site.id} data-testid={`edge-connectivity-${row.site.id}`}>
                {row.viewError !== null ? (
                  <>
                    {row.site.name}: the site view could not be read —{' '}
                    <code>{row.viewError}</code> (shown, never dropped; no
                    connectivity state is inferred from the failure).
                  </>
                ) : (
                  <>
                    {row.site.name}: edge status <code>{status.status}</code>,
                    live <code>{String(status.live)}</code> (P2.17 record,
                    verbatim; never upgraded — {status.statement})
                  </>
                )}
              </li>
            );
          })}
        </ul>
      )}
      {/* P5.10: the edge-connectivity list's freshness anchor (§20.2) — the
          P2.17 records shown above are the state as of this console read. */}
      <p>
        <FreshnessLine readAt={readAt} />
      </p>
    </>
  );
}
