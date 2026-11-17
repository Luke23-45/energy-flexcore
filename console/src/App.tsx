// P5.01 — the console app shell: workspace routing (hand-rolled hash
// router, see router.ts for the recorded decision), the §20.1 navigation,
// and the persistent chrome the later waves build on:
//
// - the control-authority banner: the 0.x platform is SIMULATION-ONLY
//   (§21.4). The real mode indicator (read from deployment configuration,
//   never a hard-coded constant) is P5.13/P5.27; until then the shell
//   states the phase boundary textually — it never displays a mode it did
//   not read from configuration.
// - the tenant indicator, populated from the claims-derived session store
//   ONLY (state/tenantContext.ts; never a URL parameter or request field,
//   §16.2). Rendered through the P5.32 TenantBanner component (persistent,
//   every workspace, with the switch-guard and source notes); the full
//   enforcement is the server's (P5.26).
// - the live-update status line (P5.25): connected / "live updates paused
//   — data may be stale" with the last-update time. Data workspaces
//   (waves 2+) must surface the same status next to the data they show.
import type { JSX } from 'react';
import { navigate, routeHash, useHashRoute } from './router';
import { useLiveEvents } from './live/useLiveEvents';
import { WORKSPACES } from './workspaces/registry';
import { ValueTypeLegend } from './components/ValueType';
import { TenantBanner } from './components/tenantBanner';
// P13.17 — the session banner with expiry display (auth hardening, wave
// 2). ADDITIVE composition-root edit (the P7.25 precedent): the banner
// renders next to the P5.32 TenantBanner; no existing line is modified.
// DISPLAY only — enforcement is the resolver (P13.16); the login/
// session endpoints it names are specified-not-implemented.
import { SessionBanner } from './components/sessionBanner';
import {
  AdministrationWorkspace,
} from './workspaces/Administration';
// P13.18 — the Administration management section (the management DESIGN
// catalog). ADDITIVE composition-root edit (the P7.25 PluginsWorkspace
// precedent: a wave-added panel rendered BELOW the P5.07 workspace on the
// same route; Administration.tsx itself is not modified). The section is
// a text catalog only — no control calls any endpoint (plan risk 6).
import { AdminManagementSection } from './workspaces/AdminManagementSection';
import { AssetsWorkspace } from './workspaces/Assets';
import { AuditReportsWorkspace } from './workspaces/AuditReports';
import { DispatchWorkspace } from './workspaces/Dispatch';
import { GridWorkspace } from './workspaces/Grid';
import { MarketProgramsWorkspace } from './workspaces/MarketPrograms';
import { OverviewWorkspace } from './workspaces/Overview';
import { PluginsIntegrationsWorkspace } from './workspaces/PluginsIntegrations';
// P7.25 — the plugin-inventory workspace extension (the P6.25
// SitesEdgeStatus precedent: a Phase-wave-added panel rendered BELOW the
// P5.07 workspace on the same route; PluginsIntegrations.tsx itself is not
// modified).
import { PluginsInventorySection } from './workspaces/PluginsWorkspace';
import { PortfoliosWorkspace } from './workspaces/Portfolios';
import { SimulationLabWorkspace } from './workspaces/SimulationLab';
import { SitesWorkspace } from './workspaces/Sites';
import { SchedulesWorkspace } from './workspaces/Schedules';
// P8.27 — the research-status workspace extension (the P7.25
// PluginsWorkspace precedent: a Phase-wave-added panel rendered BELOW the
// P5.22 workspace on the same route; SimulationLab.tsx itself is not
// modified).
import { ResearchStatusSection } from './workspaces/ResearchWorkspace';

const WORKSPACE_COMPONENTS: Record<string, () => JSX.Element> = {
  overview: OverviewWorkspace,
  portfolios: PortfoliosWorkspace,
  sites: SitesWorkspace,
  assets: AssetsWorkspace,
  grid: GridWorkspace,
  schedules: SchedulesWorkspace,
  dispatch: DispatchWorkspace,
  'market-programs': MarketProgramsWorkspace,
  'simulation-lab': SimulationLabWithResearch,
  'plugins-integrations': PluginsIntegrationsWithInventory,
  'audit-reports': AuditReportsWorkspace,
  // P13.18 — the Administration route composes the P5.07 workspace with
  // the additive management-DESIGN catalog (see AdministrationWithManagement).
  administration: AdministrationWithManagement,
};

/**
 * P7.25 — the Plugins & Integrations route composes the P5.07 workspace
 * (unchanged) with the Phase 7 plugin-inventory extension rendered below
 * it. ADDITIVE: no existing workspace component is modified.
 */
function PluginsIntegrationsWithInventory(): JSX.Element {
  return (
    <>
      <PluginsIntegrationsWorkspace />
      <PluginsInventorySection />
    </>
  );
}

/**
 * P8.27 — the Simulation Lab route composes the P5.22 workspace
 * (unchanged) with the Phase 8 research-status extension rendered below
 * it. ADDITIVE: no existing workspace component is modified. The research
 * section renders only recorded Phase 8 status — every research output is
 * a PROPOSAL, never an executed action, and the mode is locked to
 * simulation-shadow.
 */
function SimulationLabWithResearch(): JSX.Element {
  return (
    <>
      <SimulationLabWorkspace />
      <ResearchStatusSection />
    </>
  );
}

/**
 * P13.18 — the Administration route composes the P5.07 workspace
 * (unchanged) with the P13.18 management-DESIGN catalog rendered below
 * it. ADDITIVE: no existing workspace component is modified. The catalog
 * names the REST endpoint each management action would call and marks
 * every one specified-not-implemented — no write path is fabricated
 * (plan risk 6).
 */
function AdministrationWithManagement(): JSX.Element {
  return (
    <>
      <AdministrationWorkspace />
      <AdminManagementSection />
    </>
  );
}

export function App(): JSX.Element {
  const route = useHashRoute();
  const live = useLiveEvents();
  const Workspace = WORKSPACE_COMPONENTS[route] ?? OverviewWorkspace;

  return (
    <div className="console">
      {/* P5.12: the skip link is the FIRST focusable element — keyboard
          users reach the workspace content without tabbing the nav
          (keyboard-only navigation baseline, §20.2). */}
      <a className="skip-link" href="#workspace-main" data-testid="skip-link">
        Skip to workspace content
      </a>
      <header className="console-header">
        <h1>Neryva FlexCore — Operator Console</h1>
        <p className="mode-banner" role="note">
          Control authority: SIMULATION / READ-ONLY (SHADOW) ONLY in this
          phase (§21.4). This console never issues field commands and never
          displays a simulated value as measured. Mode display is read from
          deployment configuration starting with P5.13/P5.27.
        </p>
        {/* P5.32: the persistent tenant banner — claims-derived tenant +
            role, the source note, the no-switch guard statement, and the
            single-tenant note, on every workspace. */}
        <TenantBanner />
        {/* P13.17 — the additive session banner (expiry display + the
            honest no-session state; see the import note above). */}
        <SessionBanner />
        <LiveStatusLine status={live.status} lastDeliveredAt={live.lastDeliveredAt} />
        {/* P5.10: the §20.2 treatment legend is a persistent shell element,
            so every workspace carries it — the legend is on screen, not
            tribal knowledge. */}
        <ValueTypeLegend />
      </header>
      <nav aria-label="Workspaces">
        <ul>
          {WORKSPACES.map((workspace) => (
            <li key={workspace.id}>
              <a
                href={routeHash(workspace.id)}
                aria-current={route === workspace.id ? 'page' : undefined}
                onClick={(event) => {
                  event.preventDefault();
                  navigate(workspace.id);
                }}
              >
                {workspace.title}
              </a>
            </li>
          ))}
        </ul>
      </nav>
      <main id="workspace-main" tabIndex={-1}>
        <Workspace />
      </main>
    </div>
  );
}

interface LiveStatusLineProps {
  status: ReturnType<typeof useLiveEvents>['status'];
  lastDeliveredAt: string | null;
}

/**
 * The P5.25 disconnect contract, visible in the shell: a stream that is
 * not open is shown as paused with the last delivery time — never as
 * silently up-to-date.
 */
function LiveStatusLine({ status, lastDeliveredAt }: LiveStatusLineProps): JSX.Element {
  const lastUpdate = lastDeliveredAt ? `last update ${lastDeliveredAt}` : 'no update received';
  if (status === 'open') {
    return (
      <p className="live-status" role="status">
        Live updates: connected ({lastUpdate}).
      </p>
    );
  }
  if (status === 'connecting') {
    return (
      <p className="live-status" role="status">
        Live updates: connecting… ({lastUpdate}).
      </p>
    );
  }
  return (
    <p className="live-status" role="alert">
      Live updates paused — data may be stale ({lastUpdate}). The console
      re-syncs explicitly on reconnect; no gap is filled silently (P5.25).
    </p>
  );
}
