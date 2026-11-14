// P6.25 — the Sites-edge-status data layer (the Sites workspace extension's
// loading/mapping, the `Sites.tsx` data-module pattern: new files over
// rewrites — see
// ledger/phase-06-edge-protocol/25_31_32_console_clock_decommission.md §1).
//
// The Sites workspace embeds this extension as a per-edge status panel:
// edge connectivity (last-seen), certificate expiry (the P5 rotation-window
// treatment pattern), adapter health (the P6.17 closed vocabulary rendered
// VERBATIM), last-seen timestamps, buffer depth, data freshness PROMINENT
// with the measured/estimated/stale/simulated treatments reused from
// `components/honesty` + `stateVocabulary` (no new visual language), and
// the P6.14 command-path state. The console NEVER enables the command
// path — the only affordance records a request locally and states it takes
// effect ONLY through the P6.14 procedure (api/edgeStatus.ts header).
import { errorText } from '../components/workspaceData';
import {
  certificateExpiryTreatment,
  commandPathIsEnabled,
  fetchEdgeStatus,
  type CertificateExpiryTreatment,
  type EdgeStatusDto,
} from '../api/edgeStatus';
import type { ApiClient } from '../api/client';

/** One edge's status as the panel renders it (the mapped row). */
export interface EdgeStatusRow {
  status: EdgeStatusDto;
  /** The certificate-expiry treatment (P6.06/P6.17 rotation-window rules). */
  certificateTreatment: CertificateExpiryTreatment;
  /** Whether the command path reports TEST_ENV_ENABLED. */
  commandPathEnabled: boolean;
}

/** One per-edge slot: either a mapped row or the verbatim fetch error. */
export interface EdgeStatusSlot {
  deviceId: string;
  row: EdgeStatusRow | null;
  error: string | null;
}

/** The mapped load result of the panel's data read. */
export interface EdgeStatusData {
  slots: EdgeStatusSlot[];
  /** The instant the read was made (unix millis; the freshness anchor). */
  readAtUnixMillis: number;
}

/**
 * Loads the per-edge status records for the given device ids (the Sites
 * workspace passes the sites' edge identities). Every device id is fetched
 * independently; a failed fetch is carried as that slot's verbatim error
 * (the Sites.tsx `viewError` pattern) — never swallowed, never replaced by
 * a plausible row, and never failing the whole panel for one edge.
 */
export async function loadEdgeStatuses(
  deviceIds: readonly string[],
  client: ApiClient,
  nowMillis: number = Date.now(),
): Promise<EdgeStatusData> {
  const slots = await Promise.all(
    deviceIds.map(async (deviceId): Promise<EdgeStatusSlot> => {
      try {
        const status = await fetchEdgeStatus(deviceId, client);
        return { deviceId, row: mapEdgeStatus(status, nowMillis), error: null };
      } catch (error: unknown) {
        return { deviceId, row: null, error: errorText(error) };
      }
    }),
  );
  return { slots, readAtUnixMillis: nowMillis };
}

/** Pure mapping (exported for the tests): DTO → render row. */
export function mapEdgeStatus(status: EdgeStatusDto, nowMillis: number): EdgeStatusRow {
  return {
    status,
    certificateTreatment: certificateExpiryTreatment(
      status.certificate_expiry_unix_millis,
      nowMillis,
    ),
    commandPathEnabled: commandPathIsEnabled(status.command_path.state),
  };
}

/**
 * The verbatim statement the panel renders next to the request action (the
 * recorded P6.25 rule: the console can never enable the command path).
 */
export const COMMAND_PATH_REQUEST_NOTE =
  'This action RECORDS A REQUEST for the signed per-site configuration ' +
  'change only. It does NOT enable anything: the console has no endpoint ' +
  'that could enable the command path, and enablement takes effect ONLY ' +
  'through the P6.14 procedure (signed configuration + signed time-bounded ' +
  'enablement + allowlist + D6 record).';

/**
 * Records ONE command-path enablement REQUEST as a local console event
 * (the security/denialLog pattern: local diagnostics only, never sent to
 * any server — no such endpoint exists and none was invented).
 */
export interface CommandPathRequestEvent {
  deviceId: string;
  /** The state the edge reported at request time. */
  reportedState: string;
  reason: string;
  atIso: string;
}

const REQUEST_LOG_CAPACITY = 100;
let requestLog: CommandPathRequestEvent[] = [];

export function recordCommandPathEnablementRequest(event: {
  deviceId: string;
  reportedState: string;
  reason: string;
}): void {
  requestLog = [
    ...requestLog,
    { ...event, atIso: new Date().toISOString() },
  ].slice(-REQUEST_LOG_CAPACITY);
}

export function getCommandPathRequests(): CommandPathRequestEvent[] {
  return [...requestLog];
}

export function clearCommandPathRequests(): void {
  requestLog = [];
}
