// P5.07 — Plugins & Integrations workspace data (§20.1 item 10): pure
// mapping functions and the honesty statements (no I/O in the pure layer;
// the loader reads the tenant-scoped APIs).
//
// What exists and what does not (the recorded inventory, decision record
// 07_31_market_plugins_admin_alarms.md):
// - EXISTS (as platform-INTERNAL simulator adapters, per-run recorded —
//   NOT plugins, with no plugin manifest or registry system): the asset
//   simulator integration (P5.14, ledger decision 14_15_adapters.md) and
//   the pandapower grid adapter integration (P5.15). Both run INSIDE the
//   platform through the adapter interfaces; neither is a third-party
//   extension.
// - EXISTS (API-backed): the site-level edge-connectivity report (P2.17
//   site view): `simulator` / `not_connected` with live:false — the
//   honest simulation/lab state, D5 OPEN.
// - DOES NOT EXIST: the plugin kernel and plugin SDK (Stage 7, v0.5.0 per
//   the release ladder — 29_safety_invariant_matrix.md residual gap 3
//   records this as a proven-structural state: no plugin execution path
//   exists, so nothing can be bypassed); any installed/registered plugin;
//   any plugin capability/permission/health/configuration record; any
//   plugin registry API (the §15.2 surface has no /plugins resource).
//   All are stated as gaps — no plugin is ever shown as installed.
import type { SiteDto } from '../api/types';
import type { SiteViewDto } from '../api/views';

/** The plugin-kernel gap statement (the release ladder is the authority). */
export const PLUGIN_KERNEL_STATEMENT =
  'No plugin kernel exists and no plugin SDK exists. The plugin SDK and ' +
  'the WASM host are Stage 7 (v0.5.0) per the release ladder; the Phase 4 ' +
  'safety-invariant matrix (29_safety_invariant_matrix.md, residual gap 3) ' +
  'records the honest consequence: no plugin execution path exists in this ' +
  'slice, so "plugins cannot bypass the platform invariants" holds ' +
  'structurally — because there is no plugin to run. No plugin is shown ' +
  'as installed, and no plugin capability, permission, version, health, or ' +
  'configuration is displayed, because no such record exists anywhere.';

/** The §5.7 plugin-authority rules, as a read-only statement. */
export const PLUGIN_AUTHORITY_STATEMENT =
  'Plugin authority rules (report §5.7, read-only statement — there is ' +
  'no plugin to apply them to yet): a plugin can NEVER select or raise ' +
  'the control-authority mode; never bypass authorization, tenant ' +
  'isolation, audit, the command lifecycle, or final dispatch validation; ' +
  'third-party extensions use WASM/WIT or out-of-process gRPC workers, ' +
  'never arbitrary Rust dylib loading; and the §5.7 capability names ' +
  '`dispatch.authorize` and `dispatch.issue` do not exist on the closed ' +
  'authorization action vocabulary at all (flexcore-authz action.rs — ' +
  'they parse to nothing, so they can only ever be denied).';

/** The integrations that DO exist — platform-internal, honestly scoped. */
export interface RegisteredIntegration {
  name: string;
  role: string;
  originSubtask: string;
  isPlugin: false;
  scopeStatement: string;
}

export function registeredIntegrations(): RegisteredIntegration[] {
  return [
    {
      name: 'Asset simulator integration',
      role: 'the telemetry and acknowledgement source for the Phase 5 scenario runs (BESS, PV, flexible load, EVSE)',
      originSubtask: 'P5.14 (decision record 14_15_adapters.md)',
      isPlugin: false,
      scopeStatement:
        'A platform-INTERNAL simulator adapter wired through the platform adapter interfaces — not a plugin, not a registered extension, and displayed nowhere as one.',
    },
    {
      name: 'Grid simulator adapter (pandapower) integration',
      role: 'the feasibility-check execution path for the scenario runs (Phase 4 pipeline, §12.5)',
      originSubtask: 'P5.15 (decision record 14_15_adapters.md)',
      isPlugin: false,
      scopeStatement:
        'A platform-INTERNAL adapter: the scenario path calls the platform pipeline, not a reimplementation — not a plugin, not a registered extension.',
    },
  ];
}

export interface EdgeConnectivityRow {
  site: SiteDto;
  view: SiteViewDto | null;
  viewError: string | null;
}

/** The edge/integration connectivity rows: one per site, from the site
 * views (the API-backed record of the simulator integration state). */
export function edgeConnectivityRows(
  sites: SiteDto[],
  views: Array<SiteViewDto | null>,
  viewErrors: Array<string | null>,
): EdgeConnectivityRow[] {
  return sites.map((site, index) => ({
    site,
    view: views[index] ?? null,
    viewError: viewErrors[index] ?? null,
  }));
}

/** The honest edge-connectivity reading of one site view: the API's
 * contract is `simulator` | `not_connected` with live:false in Phase 2
 * (D5 OPEN — no real edge agent is enrolled in this phase); the console
 * renders the recorded values verbatim and never upgrades them. */
export function edgeStatusOf(row: EdgeConnectivityRow): {
  status: string;
  live: boolean;
  statement: string;
} {
  const status = row.view?.edge_connectivity.status ?? 'not_connected';
  const live = row.view?.edge_connectivity.live ?? false;
  return {
    status,
    live,
    statement:
      'Edge connectivity (P2.17 contract, verbatim record): the status is ' +
      '`simulator` when the site carries an edge identity — the only ' +
      'Phase 2 provider set is the simulator — and `not_connected` ' +
      'otherwise; `live` is always false in this phase. No real edge ' +
      'agent is enrolled in 0.x (first site integration is v0.4.0, D5 ' +
      'OPEN); the lab state is shown, never upgraded to a live connection.',
  };
}
