// P5.02 — Overview workspace data mapping (pure functions, no I/O).
//
// Honesty rules implemented here (each tested in Overview.test.tsx):
// - The aggregate power is computed ONLY from usable-quality contributors
//   (GOOD/SUSPECT — the tracking-quality discipline of the Phase 3/4
//   pipelines: BAD/MISSING/STALE are never coerced into a number). Assets
//   that cannot contribute are LISTED as excluded, never silently dropped.
//   With zero usable contributors the aggregate is `null` — rendered as
//   "no usable contributor" — NEVER as 0 MW (a missing aggregate is not a
//   zero aggregate, the P2.18 missing-value discipline).
// - Every value the platform serves in Phase 2 carries simulated: true
//   (P2.11/P2.12); the mapping keeps that flag and the workspace renders
//   the simulated type treatment as PART OF THE COMPONENT (the P5.02 risk
//   mitigation — the simulated label is not an optional prop).
// - Fleet availability derives each asset's state with the mirrored P2.16
//   rule (components/availability.tsx); the derivation statement travels
//   with the summary.
// - The alarm contract is DEFINED by P5.31 (ALARM-MODEL-0.1.0,
//   flexcore-contracts::alarm): the workspace consumes the derived-on-read
//   GET /api/v1/alarms resource through components/alarmsPanel.tsx — no
//   alarm is invented here, and the §15.3 event vocabulary was not
//   extended (no alarm event exists).
// - Delivery status counts the persisted §14.2 command states and flags
//   the two states the simulation-only boundary qualifies: ISSUED (issued
//   to the SIMULATOR — a simulated issue, not field delivery) and
//   ACKNOWLEDGED (application-layer acceptance by the simulator;
//   acknowledgement is NOT physical delivery — 32_handover_phase5.md §2).
import type { CurrentStateDto, DispatchCommandDto, ScheduleRevisionDto, TelemetryQuality } from '../api/types';
import { deriveAvailability, type AvailabilityState } from '../components/availability';

/** The sign-convention label (report §10.2; rendered next to every MW). */
export const SIGN_CONVENTION_LABEL =
  'P > 0 means export at the point of connection (report §10.2 canonical sign).';

/** The only quality states a numeric aggregation may consume. */
export function isUsableQuality(quality: TelemetryQuality): boolean {
  return quality === 'GOOD' || quality === 'SUSPECT';
}

export interface ExcludedAsset {
  asset_id: string;
  reason: string;
}

export interface AggregatePowerResult {
  /** MW sum over usable contributors; null when NO contributor is usable. */
  total_mw: number | null;
  contributor_count: number;
  /** Assets excluded from the aggregate, with the reason — never silent. */
  excluded: ExcludedAsset[];
  /** Newest contributing event time (the aggregate's freshness anchor). */
  freshness_event_time: string | null;
  /** True when every contributor carries the simulated provenance flag. */
  all_inputs_simulated: boolean;
}

/**
 * Sums the latest `active_power` (MW) across the fleet's current states.
 * Usable = quality GOOD/SUSPECT and a numeric value present.
 */
export function aggregateActivePower(states: CurrentStateDto[]): AggregatePowerResult {
  const excluded: ExcludedAsset[] = [];
  let total = 0;
  let contributorCount = 0;
  let allSimulated = true;
  let freshness: string | null = null;
  for (const state of states) {
    const quantity = state.quantities.find((q) => q.quantity === 'active_power');
    if (!quantity) {
      excluded.push({ asset_id: state.asset_id, reason: 'active_power not reported by this asset' });
      continue;
    }
    if (!isUsableQuality(quantity.quality)) {
      excluded.push({
        asset_id: state.asset_id,
        reason: `active_power quality ${quantity.quality} is not usable for aggregation (BAD/MISSING/STALE are never coerced)`,
      });
      continue;
    }
    if (quantity.value_num === undefined) {
      excluded.push({
        asset_id: state.asset_id,
        reason: 'active_power carries no numeric value (a missing value is never read as zero, P2.18)',
      });
      continue;
    }
    total += quantity.value_num;
    contributorCount += 1;
    if (!quantity.simulated) allSimulated = false;
    if (freshness === null || quantity.event_time > freshness) {
      freshness = quantity.event_time;
    }
  }
  return {
    total_mw: contributorCount > 0 ? total : null,
    contributor_count: contributorCount,
    excluded,
    freshness_event_time: freshness,
    all_inputs_simulated: allSimulated,
  };
}

export interface FleetAvailabilitySummary {
  available: string[];
  unavailable: string[];
  unknown: string[];
  no_new_signal: string[];
  /** Newest availability event time consulted (freshness anchor). */
  freshness_event_time: string | null;
  derivation: string;
}

/**
 * Derives each asset's availability state (the mirrored P2.16 rule) from
 * its current state's `availability` quantity and buckets the asset ids.
 */
export function summarizeFleetAvailability(states: CurrentStateDto[]): FleetAvailabilitySummary {
  const summary: FleetAvailabilitySummary = {
    available: [],
    unavailable: [],
    unknown: [],
    no_new_signal: [],
    freshness_event_time: null,
    derivation:
      'Derived per the P2.16 availability rule from each asset\'s latest `availability` telemetry quantity ' +
      '(components/availability.tsx documents the mirrored rule and its recorded display limitation).',
  };
  for (const state of states) {
    const availability = state.quantities.find((q) => q.quantity === 'availability');
    const derived: AvailabilityState = availability
      ? deriveAvailability(availability.quality, availability.value_bool)
      : deriveAvailability(null, undefined);
    if (availability) {
      const eventTime = availability.event_time;
      if (
        summary.freshness_event_time === null ||
        eventTime > summary.freshness_event_time
      ) {
        summary.freshness_event_time = eventTime;
      }
    }
    summary[derived].push(state.asset_id);
  }
  return summary;
}

export interface ActiveScheduleRow {
  schedule_id: string;
  revision: number;
  portfolio_id: string;
  site_id: string;
  horizon_start_millis: number;
  horizon_end_millis: number;
  roll_trigger: string;
}

/**
 * The active schedules (ID + revision): only revisions whose stored state
 * is `active` (dto_phase3.rs: `active` | `superseded`; a superseded
 * revision is retained server-side and simply does not appear here).
 */
export function selectActiveSchedules(schedules: ScheduleRevisionDto[]): ActiveScheduleRow[] {
  return schedules
    .filter((schedule) => schedule.state === 'active')
    .map((schedule) => ({
      schedule_id: schedule.schedule_id,
      revision: schedule.revision,
      portfolio_id: schedule.portfolio_id,
      site_id: schedule.site_id,
      horizon_start_millis: schedule.horizon_start_millis,
      horizon_end_millis: schedule.horizon_end_millis,
      roll_trigger: schedule.roll_trigger,
    }));
}

export interface DeliveryStatusSummary {
  /** Count per persisted §14.2 state (closed vocabulary, server-side). */
  counts: Record<string, number>;
  total: number;
  /** Count of commands currently in the ISSUED state (simulator target). */
  issued_to_simulator: number;
  /** Count of commands currently ACKNOWLEDGED (application-layer only). */
  acknowledged_not_delivery: number;
}

/**
 * Summarizes the persisted dispatch commands by their current §14.2 state.
 * The ISSUED and ACKNOWLEDGED qualifications rendered next to these counts
 * reuse the Phase 4 handover wording verbatim (32_handover_phase5.md §2);
 * the counts themselves are plain persisted facts.
 */
export function summarizeDeliveryStatus(commands: DispatchCommandDto[]): DeliveryStatusSummary {
  const counts: Record<string, number> = {};
  for (const command of commands) {
    counts[command.state] = (counts[command.state] ?? 0) + 1;
  }
  return {
    counts,
    total: commands.length,
    issued_to_simulator: counts['ISSUED'] ?? 0,
    acknowledged_not_delivery: counts['ACKNOWLEDGED'] ?? 0,
  };
}

/**
 * The resolved alarm-contract wording (P5.31): the gap P5.02 recorded is
 * DEFINED — the minimal alarm model lives in `flexcore-contracts::alarm`
 * (ALARM-MODEL-0.1.0), the derivation rules in
 * `flexcore-domain::alarm_rules`, and the console consumes the
 * derived-on-read GET /api/v1/alarms resource. The §15.3 freeze was NOT
 * extended: no alarm event exists; alarms are derived records. The
 * response's derivation_statement is rendered verbatim by the alarm
 * panels (components/alarmsPanel.tsx) — this constant is the citation the
 * panels keep under them.
 */
export const ALARM_CONTRACT_TEXT =
  'Alarm contract: DEFINED by P5.31 (ALARM-MODEL-0.1.0, ' +
  'flexcore-contracts::alarm). Alarms are operator-attention records ' +
  'derived on read from existing platform facts (telemetry quality ' +
  'states, dispatch outcomes, feasibility verdicts) — never protection ' +
  'signals, never invented by the console, and never §15.3 events (the ' +
  'frozen event vocabulary is not extended). The panel renders the real ' +
  'derived alarm list; when no alarm is currently derivable it says so ' +
  'explicitly instead of showing a fabricated one.';

/**
 * The control-authority mode line (P5.02 step 3). The 0.x deployment modes
 * are Simulation and read-only/shadow only (§21.4); reading the mode from
 * deployment configuration lands with P5.13/P5.27, and until then the
 * boundary is stated textually — never a mode value the console did not
 * read from configuration.
 */
export const CONTROL_AUTHORITY_MODE_TEXT =
  'Control-authority mode: SIMULATION / READ-ONLY (SHADOW) only in this ' +
  'phase (§21.4). The 0.x line is simulation-only: every dispatch is issued ' +
  'to the simulator and nothing else; there is no device write path. The ' +
  'mode indicator read from deployment configuration (per site where modes ' +
  'differ) lands with P5.13/P5.27; this line states the phase boundary ' +
  'instead of displaying a configuration value the console cannot yet read.';
