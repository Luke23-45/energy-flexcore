// P7.25 — the plugin-inventory read DTO mirrors + typed fetch wrapper
// (`GET /api/v1/plugins`, apps/control-plane routes_plugins.rs /
// dto_plugins.rs).
//
// SHAPE-MIRROR NOTE (the views.ts / edgeStatus.ts discipline): these types
// mirror dto_plugins.rs field-for-field using the WIRE names (snake_case).
// The authoritative shapes are pinned by the OpenAPI schema (the endpoint
// is registered in openapi.rs) and the generated-client drift test lands
// with the first install; the hand-written mirrors here are its stand-in.
//
// HONESTY CONTRACT (the P6.25 edgeStatus.ts contract, applied to plugins):
// `lifecycle_state: "NOT_REPORTED"` and `health: null` are HONEST GAPS —
// "no host reporter is composed", never "ready", "healthy", or "stopped".
// The closed vocabularies below are the BUILT system's
// (`crates/flexcore-kernel/src/lifecycle.rs::LifecycleState::name` and
// `apps/plugin-host/src/health.rs::{HealthState,ReadinessState}`), rendered
// VERBATIM everywhere — the console never prettifies a health or lifecycle
// value into a healthy-looking synonym.
//
// THE DISPLAY-ONLY GUARANTEE (P7.25 step 3, recorded; the P6.25
// command-path rule): this module issues GET calls ONLY — there is NO
// endpoint on the server that activates, deactivates, upgrades, or
// switches anything (verified against routes_plugins.rs / openapi.rs), and
// none is called or invented here. The console's profile-switch affordance
// records a LOCAL request event (workspaces/pluginsWorkspaceData.ts, the
// denialLog pattern) and states it takes effect ONLY through the reviewed
// profile-file edit + re-composition procedure (P7.16).
import { api, type ApiClient } from './client';

/** The §5.6 plugin lifecycle states (the kernel's closed enum, machine names). */
export const LIFECYCLE_STATES = [
  'DISCOVERED',
  'VERIFIED',
  'COMPATIBLE',
  'CONFIGURED',
  'STARTED',
  'READY',
  'DRAINING',
  'STOPPED',
  'REJECTED',
  'FAILED',
] as const;

export type LifecycleState = (typeof LIFECYCLE_STATES)[number];

/**
 * The explicit gap: no host reporter is composed, so NO lifecycle state is
 * known. Named, never silently mapped onto a plausible state (the P6.25
 * NOT_REPORTED pattern).
 */
export const LIFECYCLE_STATE_NOT_REPORTED = 'NOT_REPORTED';

/** The P7.20 readiness vocabulary (closed; rendered verbatim). */
export const READINESS_STATES = ['UNOBSERVED', 'PROBE_FAILED', 'READY'] as const;

export type ReadinessState = (typeof READINESS_STATES)[number];

/**
 * The P7.20 health vocabulary shape: `HEALTHY` | `DEGRADED_<reason>` |
 * `FAILING_<reason>` with the closed reason tokens (health.rs::HealthReason).
 * Rendered verbatim — never enumerated away.
 */
export const HEALTH_REASON_TOKENS = [
  'UNVERIFIED',
  'UNRESPONSIVE',
  'MEMORY_LIMIT',
  'FUEL_EXHAUSTED',
  'DEADLINE',
  'OUTPUT_LIMIT',
  'CALL_RATE',
  'ERROR_RATE',
  'BINDINGS',
] as const;

/**
 * Whether a health state blocks dependent actions (only `FAILING_*` does —
 * health.rs::HealthState::blocks_dependent_actions).
 */
export function healthBlocksDependentActions(healthState: string): boolean {
  return healthState.startsWith('FAILING_');
}

/** One declared configuration property (the config SUMMARY — no values). */
export interface ConfigPropertyDto {
  name: string;
  /** `string` | `number` | `integer` | `boolean` | `array` | `object`. */
  property_type: string;
  description: string;
}

/** The health snapshot of one plugin (dto_plugins.rs PluginHealthDto). */
export interface PluginHealthDto {
  health_state: string;
  readiness: string;
  note: string;
}

/** One plugin of the recorded inventory (dto_plugins.rs PluginRecordDto). */
export interface PluginRecordDto {
  plugin_id: string;
  version: string;
  /** `builtin-rust` | `wasm-component` | `grpc-worker` — verbatim. */
  class: string;
  provider_of: string[];
  consumes: string[];
  permissions: string[];
  license: string;
  description: string;
  /** The config SUMMARY: names, types, descriptions — never values. */
  config_properties: ConfigPropertyDto[];
  config_required: string[];
  /** Where this record is recorded from (the shipped file, verbatim). */
  record_note: string;
  /** `NOT_REPORTED` when no host reporter is composed — the gap. */
  lifecycle_state: string;
  /** `null` = not reported — the gap, never "healthy". */
  health: PluginHealthDto | null;
}

/** One composition profile of the recorded inventory (ProfileRecordDto). */
export interface ProfileRecordDto {
  profile_id: string;
  version: string;
  /** Always `simulation-shadow` from the shipped loader (the mode lock). */
  control_authority: string;
  permission_ceiling: string[];
  active_selections: string[];
  /** `COMPOSES` | `DEFERRED_SHELL` — verbatim. */
  composition_state: string;
  state_note: string;
}

/** The plugin-inventory read (dto_plugins.rs PluginInventoryDto). */
export interface PluginInventoryDto {
  plugins: PluginRecordDto[];
  profiles: ProfileRecordDto[];
  /** The honest surface note (verbatim). */
  inventory_note: string;
}

/**
 * Fetches the plugin inventory (the tenant is server-derived from claims —
 * §16.2; NO tenant parameter exists or is sent). A forbidden role → 403
 * (rendered as the honest error, never hidden). READ-ONLY: this module
 * issues GET calls only (the display-only guarantee above).
 */
export function fetchPluginsInventory(
  client: ApiClient = api,
): Promise<PluginInventoryDto> {
  return client
    .get<{ data: PluginInventoryDto }>('/api/v1/plugins')
    .then((envelope) => envelope.data);
}
