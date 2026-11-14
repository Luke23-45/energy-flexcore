// P7.25 — the Plugins & Integrations workspace EXTENSION's data layer (the
// `sitesEdgeStatus.ts` pattern: the loading/mapping module of a Phase-wave
// workspace panel — new files over rewrites, the P5.07 workspace itself is
// NOT modified; the two sections render in sequence on the same route).
//
// What this layer loads: the plugin-inventory read (`GET /api/v1/plugins`,
// api/plugins.ts) — the RECORDED shipped inventory plus whatever the
// deployment's composed source reports. Every runtime fact the server does
// not report renders as the named gap (`NOT_REPORTED` / `null` health),
// never a plausible "ready"/"healthy" value.
//
// THE DISPLAY-ONLY GUARANTEE (P7.25 step 3, recorded; the P6.25
// command-path rule): the console NEVER executes a composition change —
// the server has no activate/deactivate/upgrade/profile-switch endpoint
// (verified against routes_plugins.rs / openapi.rs), and the only
// affordance is "request a profile switch", which records a LOCAL request
// event (the denialLog pattern) and states it takes effect ONLY through
// the reviewed profile-file edit + re-composition procedure (P7.16, whose
// own SwitchConfirmation gate is the enforcement, not this console).
//
// NO SECRET VALUE (§5.7): the config summary carries property NAMES and
// TYPES only; no configuration value — and no secret value or secret
// location beyond the recorded manifest reference strings — is served by
// the endpoint or displayed here.
import {
  fetchPluginsInventory,
  LIFECYCLE_STATE_NOT_REPORTED,
  healthBlocksDependentActions,
  type PluginInventoryDto,
  type PluginRecordDto,
  type ProfileRecordDto,
} from '../api/plugins';
import type { ApiClient } from '../api/client';

/** The mapped load result of the panel's data read. */
export interface PluginsInventoryData {
  inventory: PluginInventoryDto;
}

/**
 * Loads the plugin inventory through the tenant-scoped API client. A
 * failed read fails the panel honestly (the whole-load error path — the
 * inventory is one resource; per-slot errors do not apply here).
 */
export async function loadPluginsInventory(client: ApiClient): Promise<PluginsInventoryData> {
  const inventory = await fetchPluginsInventory(client);
  return { inventory };
}

/**
 * The lifecycle-state reading of one plugin record: the reported state
 * VERBATIM, or the named gap. Nothing is normalised, prettified, or
 * upgraded.
 */
export function lifecycleStateOf(plugin: PluginRecordDto): string {
  return plugin.lifecycle_state === LIFECYCLE_STATE_NOT_REPORTED
    ? LIFECYCLE_STATE_NOT_REPORTED
    : plugin.lifecycle_state;
}

/**
 * The health reading of one plugin record: the snapshot VERBATIM (the
 * P7.20 closed vocabulary as the server rendered it) or `null` (the gap —
 * never a fabricated "HEALTHY").
 */
export function healthOf(plugin: PluginRecordDto): PluginHealthVerbatim | null {
  if (plugin.health === null) return null;
  return {
    healthState: plugin.health.health_state,
    readiness: plugin.health.readiness,
    note: plugin.health.note,
    blocksDependentActions: healthBlocksDependentActions(plugin.health.health_state),
  };
}

export interface PluginHealthVerbatim {
  healthState: string;
  readiness: string;
  note: string;
  blocksDependentActions: boolean;
}

/** The profile-composition reading of one profile record (verbatim). */
export function profileCompositionOf(profile: ProfileRecordDto): ProfileComposition {
  return {
    profileId: profile.profile_id,
    version: profile.version,
    controlAuthority: profile.control_authority,
    ceiling: profile.permission_ceiling,
    activeSelections: profile.active_selections,
    compositionState: profile.composition_state,
    stateNote: profile.state_note,
  };
}

export interface ProfileComposition {
  profileId: string;
  version: string;
  controlAuthority: string;
  ceiling: string[];
  activeSelections: string[];
  compositionState: string;
  stateNote: string;
}

/**
 * The verbatim statement the panel renders next to the switch affordance
 * (the recorded P7.25 rule: the console can never execute a composition
 * change).
 */
export const PROFILE_SWITCH_REQUEST_NOTE =
  'This action RECORDS A REQUEST for a profile switch only. It does NOT ' +
  'switch anything: the console has no endpoint that could change a ' +
  'composition, and a switch takes effect ONLY through the reviewed ' +
  'profile-file edit + re-composition procedure (P7.16), whose own ' +
  'switch-confirmation gate is the enforcement. A profile can never ' +
  'change the control-authority mode.';

/**
 * Records ONE profile-switch REQUEST as a local console event (the
 * security/denialLog pattern: local diagnostics only, never sent to any
 * server — no such endpoint exists and none was invented).
 */
export interface ProfileSwitchRequestEvent {
  requestedProfileId: string;
  /** The profile the deployment reported as active-composing at request time, if any. */
  reportedCompositionState: string;
  reason: string;
  atIso: string;
}

const REQUEST_LOG_CAPACITY = 100;
let requestLog: ProfileSwitchRequestEvent[] = [];

export function recordProfileSwitchRequest(event: {
  requestedProfileId: string;
  reportedCompositionState: string;
  reason: string;
}): void {
  requestLog = [
    ...requestLog,
    { ...event, atIso: new Date().toISOString() },
  ].slice(-REQUEST_LOG_CAPACITY);
}

export function getProfileSwitchRequests(): ProfileSwitchRequestEvent[] {
  return [...requestLog];
}

export function clearProfileSwitchRequests(): void {
  requestLog = [];
}

/**
 * The §20.2 proposed-vs-executed statement, rendered on the panels where
 * plugin facts and requests are displayed (recorded P7.25 step 4): a
 * plugin's output is a CANDIDATE (ADR-010) and a console request is a
 * REQUEST — neither is ever displayed as executed.
 */
export const PROPOSED_VS_EXECUTED_NOTE =
  'A plugin output is a PROPOSAL (candidate) that still passes the ' +
  'protected validation → feasibility → authorization path — it gains no ' +
  'authority from having been proposed (ADR-010, §13.4). A request ' +
  'recorded on this console is a REQUEST, never an executed action. ' +
  'Candidate, validated, feasible, approved, issued, and acknowledged ' +
  'remain distinct states everywhere (§20.2).';
