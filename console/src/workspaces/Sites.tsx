// P5.03 — the Sites workspace (report §20.1 item 3), on the Phase 2
// registry contracts (P2.17 site views, dto.rs) with the grid-model
// binding read from the Phase 4 feasibility-check records.
//
// Elements per §20.1 item 3 / ledger P5.03 step 2:
// - site overview (the P2.17 view's asset_count), PCC (rendered verbatim),
// - grid model: the registry carries a grid_model_reference STRING only —
//   the P4.03 version + content hash are NOT registry fields. They are
//   shown from the latest feasibility-check record for the site
//   (FeasibilityCheckDto.model_version / model_content_hash /
//   checked_at_unix_millis, dto_phase4.rs); when no check exists for the
//   site the gap is stated, never filled with a plausible version,
// - edge connectivity: the P2.17 report is HONEST BY CONTRACT — status is
//   `simulator` | `not_connected` and `live` is always false in Phase 2;
//   the console renders those fields verbatim and never upgrades a
//   simulated edge into a live one (the ledger risk row),
// - local EMS status: NO Phase 1–4 contract supplies EMS state, so the
//   panel renders the honest gap — an invented "EMS connected" line would
//   violate the no-hallucinated-facts rule.
import type { JSX } from 'react';
import type { ApiClient } from '../api/client';
import type { FeasibilityCheckDto, SiteDto } from '../api/types';
import { listFeasibilityChecks } from '../api/dispatch';
import { fetchSiteView, fetchSites, type SiteViewDto } from '../api/views';
import {
  ErrorBlock,
  HonestEmpty,
  LoadingLine,
  RoleScopeLine,
  errorText,
  useWorkspaceData,
  type LoadState,
} from '../components/workspaceData';
import { FreshnessLine } from '../components/ValueType';
import { formatUnixMillis } from '../components/format';

export interface SiteRow {
  site: SiteDto;
  view: SiteViewDto | null;
  viewError: string | null;
  /** Latest feasibility check for this site (the grid-model binding). */
  latestCheck: FeasibilityCheckDto | null;
}

export interface SitesData {
  rows: SiteRow[];
  /** Whether the check page consulted was empty (drives the grid note). */
  checkCount: number;
}

/**
 * The latest feasibility-check record for one site by completion time.
 * Recorded limitation (decision record 02_04_workspaces_a.md): the
 * feasibility-check list has NO site filter on the API, so the console
 * filters the first returned page client-side — a large backlog could
 * scroll a site's latest check off the page, and the display states the
 * consulted page size rather than claiming completeness.
 */
export function latestCheckForSite(
  siteId: string,
  checks: FeasibilityCheckDto[],
): FeasibilityCheckDto | null {
  let latest: FeasibilityCheckDto | null = null;
  for (const check of checks) {
    if (check.site_id !== siteId) continue;
    if (latest === null || check.checked_at_unix_millis > latest.checked_at_unix_millis) {
      latest = check;
    }
  }
  return latest;
}

export async function loadSites(client: ApiClient): Promise<SitesData> {
  const [sites, checksPage] = await Promise.all([
    fetchSites(client),
    listFeasibilityChecks(undefined, client),
  ]);
  const rows = await Promise.all(
    sites.map(async (site) => {
      try {
        const view = await fetchSiteView(site.id, client);
        return {
          site,
          view,
          viewError: null,
          latestCheck: latestCheckForSite(site.id, checksPage.data),
        };
      } catch (error: unknown) {
        return {
          site,
          view: null,
          viewError: errorText(error),
          latestCheck: latestCheckForSite(site.id, checksPage.data),
        };
      }
    }),
  );
  return { rows, checkCount: checksPage.data.length };
}

export function SitesWorkspace(): JSX.Element {
  const state = useWorkspaceData(loadSites);
  return <SitesView state={state} />;
}

export function SitesView({ state }: { state: LoadState<SitesData> }): JSX.Element {
  return (
    <section aria-label="Sites">
      <h2>Sites</h2>
      <RoleScopeLine />
      {state.status === 'loading' && <LoadingLine what="the sites" />}
      {state.status === 'error' && <ErrorBlock what="The site registry" error={state.error} />}
      {state.status === 'ready' && <SitesReady data={state.data} readAt={state.readAt} />}
    </section>
  );
}

function SitesReady({ data, readAt }: { data: SitesData; readAt: string }): JSX.Element {
  if (data.rows.length === 0) {
    return (
      <HonestEmpty
        what="Sites"
        reason="The tenant-scoped site read returned no sites. No site row is fabricated."
      />
    );
  }
  return (
    <>
      {data.rows.map((row) => (
        <SiteSection key={row.site.id} row={row} readAt={readAt} />
      ))}
      <p>
        Feasibility-check records consulted for the grid-model bindings:{' '}
        {data.checkCount} (the first returned page; the API exposes no site
        filter on this listing — recorded limitation).
      </p>
    </>
  );
}

function SiteSection({ row, readAt }: { row: SiteRow; readAt: string }): JSX.Element {
  const { site, view, viewError, latestCheck } = row;
  return (
    <article data-testid={`site-${site.id}`}>
      <h3>
        {site.name} <small>({site.id})</small>
      </h3>
      <dl>
        <dt>Location / timezone</dt>
        <dd>
          {site.location} ({site.timezone})
        </dd>
        <dt>Portfolio</dt>
        <dd>{site.portfolio_id}</dd>
        <dt>Point of common coupling (registry, verbatim)</dt>
        <dd>
          <code>{site.point_of_common_coupling}</code>
        </dd>
        <dt>Created</dt>
        <dd>{site.created_at}</dd>
      </dl>

      {viewError !== null ? (
        <div role="note" data-testid="site-view-error">
          <p>
            The P2.17 site view could not be read for this site; its overview
            and edge connectivity are NOT shown rather than guessed:
          </p>
          <p>
            API error (verbatim): <code>{viewError}</code>
          </p>
        </div>
      ) : (
        view && (
          <>
            <h4>Site overview (P2.17 view)</h4>
            <p>{view.overview.asset_count} asset(s) registered at this site.</p>
            <h4>Edge connectivity (P2.17, rendered verbatim)</h4>
            <p data-testid="edge-connectivity">
              Status: <code>{view.edge_connectivity.status}</code>; live:{' '}
              <code>{String(view.edge_connectivity.live)}</code>; edge identity:{' '}
              <code>{view.edge_connectivity.edge_identity}</code>. The API
              never presents a simulated edge as live (P2.17 contract): in
              this phase a connected edge means the SIMULATOR edge identity,
              not a field device connection.
            </p>
          </>
        )
      )}

      <h4>Grid model (P4.03 version + hash)</h4>
      <p>
        Registry grid-model reference (verbatim):{' '}
        <code>{site.grid_model_reference}</code>
      </p>
      {latestCheck === null ? (
        <HonestEmpty
          what="Grid-model version + content hash"
          reason="No Phase 4 feasibility-check record exists for this site in the consulted page, and the registry contract carries a reference string only — no version/hash. The binding is shown from check evidence when it exists; it is NOT invented here (P4.03: every piece of feasibility evidence references THE hash)."
          testId="grid-model-gap"
        />
      ) : (
        <p data-testid="grid-model-binding">
          Latest check for this site: model <code>{latestCheck.model_id ?? 'unrecorded'}</code>,
          version <code>{latestCheck.model_version ?? 'not present in the record'}</code>, content
          hash <code>{latestCheck.model_content_hash ?? 'not present in the record'}</code>,
          checked at {formatUnixMillis(latestCheck.checked_at_unix_millis)}.
        </p>
      )}

      <h4>Local EMS status</h4>
      <HonestEmpty
        what="Local EMS status"
        reason="No Phase 1–4 contract supplies EMS state (no EMS entity, event, or API exists in the implemented phases). Displaying nothing rather than an invented EMS status; a first site integration (OpenEMS) is a v0.4.0 matter."
        testId="ems-gap"
      />

      <p>
        <FreshnessLine readAt={readAt} />
      </p>
    </article>
  );
}
