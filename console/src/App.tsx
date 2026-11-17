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
// P5.27 (FRV1-R3-010) — the additive mode-gate notice. ADDITIVE
// composition-root edit (the P7.25/P13.17 precedent): the notice renders
// in the shell header beside the P5.13 mode banner, so the recorded
// P5.27 gate statement (the deployed mode, the enforcement boundary, the
// mixed-mode rule, the configured-but-not-exercised modes) reaches a user
// on EVERY workspace instead of living only in tests. The component is
// complete and unchanged; the composition is the only missing step.
import { ModeGateNotice } from './components/modeGateNotice';
// P5.25 — the re-sync contract wiring (FRV1-R3-006): the shell owns the
// live session, so the shell provides the workspace resync channel — every
// workspace data hook keys a REST re-fetch on the resync token and
// acknowledges once its re-read lands. No workspace reads the stream
// directly; the gap contract lives in this one provider.
import { WorkspaceResyncContext } from './components/workspaceData';
// P13.17 — the session banner with expiry display (auth hardening, wave
// 2). ADDITIVE composition-root edit (the P7.25 precedent): the banner
// renders next to the P5.32 TenantBanner; no existing line is modified.
// DISPLAY only — enforcement is the resolver (P13.16); the login/
// session endpoints it names are specified-not-implemented.
import { SessionBanner } from './components/sessionBanner';
// P5.26 (FRV1-R3-011) — the denial-log diagnostics display. ADDITIVE
// composition-root edit (the P7.25 panel precedent): the block renders on
// the Administration route below the P13.18 management catalog, so the
// locally recorded capability denials / step-up failures finally have a
// visible surface. It displays LOCAL UI-event diagnostics only — the
// server audit trail (P4.28) is the real record, and the block says so.
import { DenialLogDiagnostics } from './components/denialLogDiagnostics';
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
// P6.25 (FRV1-R3-004) — the Sites-edge-status workspace extension (the
// P7.25 PluginsWorkspace precedent: a Phase-wave-added panel rendered
// BELOW the P5.03 workspace on the same route; Sites.tsx itself is not
// modified). The panel's own header records that the Sites workspace
// embeds it as a per-edge status panel — this composition makes that
// claim true.
import { SitesEdgeStatusPanel } from './workspaces/SitesEdgeStatus';
import { SchedulesWorkspace } from './workspaces/Schedules';
// P8.27 — the research-status workspace extension (the P7.25
// PluginsWorkspace precedent: a Phase-wave-added panel rendered BELOW the
// P5.22 workspace on the same route; SimulationLab.tsx itself is not
// modified).
import { ResearchStatusSection } from './workspaces/ResearchWorkspace';

const WORKSPACE_COMPONENTS: Record<string, () => JSX.Element> = {
  overview: OverviewWorkspace,
  portfolios: PortfoliosWorkspace,
  // P6.25 (FRV1-R3-004) — the Sites route composes the P5.03 workspace
  // (unchanged) with the per-edge status extension rendered below it
  // (see SitesWithEdgeStatus).
  sites: SitesWithEdgeStatus,
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
 * P6.25 (FRV1-R3-004) — the Sites route composes the P5.03 workspace
 * (unchanged) with the P6.25 per-edge status extension rendered below it.
 * ADDITIVE: no existing workspace component is modified. The panel is fed
 * by `loadSitesEdgeStatus` (the sites' edge identities, one status read
 * per identity) — the data path the recorded P6.25 header describes; the
 * server-side composition (the `GET /api/v1/edge/status` route with its
 * composed enrollment store) was fixed as FRV1-R2-002.
 */
function SitesWithEdgeStatus(): JSX.Element {
  return (
    <>
      <SitesWorkspace />
      <SitesEdgeStatusPanel />
    </>
  );
}

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
 * (plan risk 6). P5.26 (FRV1-R3-011): the local denial-log diagnostics
 * block renders below the catalog — the display surface the P5.26
 * denial-log module recorded as its purpose.
 */
function AdministrationWithManagement(): JSX.Element {
  return (
    <>
      <AdministrationWorkspace />
      <AdminManagementSection />
      <DenialLogDiagnostics />
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
        {/* FRV1-R9-002 (design wave 2) — ADDITIVE wrapper. Only the title and
            the live-status line sit in .console-titlebar, the frosted STICKY
            chrome; the banner stack below (mode banner, ModeGateNotice,
            TenantBanner, SessionBanner, ValueTypeLegend) scrolls away
            naturally, so a pinned header can never hold ~2.9k words of
            honesty prose. No existing text node or attribute is changed; the
            only DOM deltas are this div and the live-status line moving ABOVE
            the banner stack (the audit's amended order — the operator always
            sees title + connectivity, never pinned prose). */}
        <div className="console-titlebar">
          <h1>Neryva FlexCore — Operator Console</h1>
          <LiveStatusLine status={live.status} lastDeliveredAt={live.lastDeliveredAt} />
        </div>
        <p className="mode-banner" role="note">
          Control authority: SIMULATION / READ-ONLY (SHADOW) ONLY in this
          phase (§21.4). This console never issues field commands and never
          displays a simulated value as measured. Mode display is read from
          deployment configuration starting with P5.13/P5.27.
        </p>
        {/* P5.27 (FRV1-R3-010) — the additive mode-gate notice, rendered
            beside the mode banner above (the composition the component
            header describes). It never replaces the banner or the P5.13
            per-workspace indicator; it adds the P5.27 enforcement
            statement. */}
        <ModeGateNotice />
        {/* P5.32: the persistent tenant banner — claims-derived tenant +
            role, the source note, the no-switch guard statement, and the
            single-tenant note, on every workspace. */}
        <TenantBanner />
        {/* P13.17 — the additive session banner (expiry display + the
            honest no-session state; see the import note above). */}
        <SessionBanner />
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
      {/* P5.25 — the workspace resync provider: the mounted workspace's data
          hooks consume this and re-fetch from the REST read APIs whenever
          the live session flags a gap (resyncToken), acknowledging the
          re-sync once the fresh read lands. */}
      <WorkspaceResyncContext.Provider
        value={{ token: live.resyncToken, acknowledge: live.acknowledgeResync }}
      >
        <main id="workspace-main" tabIndex={-1}>
          <Workspace />
        </main>
      </WorkspaceResyncContext.Provider>
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
      Live updates paused — data may be stale ({lastUpdate}). Workspace data
      reads re-fetch from the REST APIs on reconnect (the P5.25 re-sync
      contract); operator-run query results are re-run by the operator — no
      gap is filled silently (P5.25).
    </p>
  );
}
