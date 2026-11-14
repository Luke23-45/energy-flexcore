// P6.25 — the per-edge status read DTO mirrors + typed fetch wrapper
// (`GET /api/v1/edge/status`, apps/control-plane routes_edge.rs /
// dto_edge.rs).
//
// SHAPE-MIRROR NOTE (the views.ts discipline): these types mirror
// dto_edge.rs field-for-field using the WIRE names (snake_case). The
// authoritative shapes are pinned by the OpenAPI schema (the endpoint is
// registered in openapi.rs) and the generated-client drift test lands
// with the first install; the hand-written mirrors here are its stand-in.
//
// HONESTY CONTRACT (recorded in
// ledger/phase-06-edge-protocol/25_31_32_console_clock_decommission.md §1):
// every optional field of the server DTO is an HONEST GAP when absent —
// `null` means "not reported", never "healthy", "connected", or "fresh".
// The console renders the gaps and the verbatim notes; it never upgrades
// a gap into a plausible value. This surface is POLLING (a plain GET; no
// live SSE requirement on this resource — the recorded decision).
//
// THE COMMAND-PATH GUARANTEE (P6.25 step 3): the console can NEVER enable
// the command path. There is NO endpoint on the server surface that could
// enable it (verified against routes_edge.rs / openapi.rs), and none is
// called or invented here. The only affordance is recording a REQUEST for
// the signed per-site configuration change (a local console-log event, the
// security/denialLog pattern), which states it takes effect ONLY through
// the P6.14 procedure.
import { api, type ApiClient } from './client';

/** The closed command-path state vocabulary (routes_edge.rs consts). */
export const COMMAND_PATH_STATES = [
  'DISABLED',
  'TEST_ENV_ENABLED',
  'NOT_REPORTED',
] as const;

export type CommandPathState = (typeof COMMAND_PATH_STATES)[number];

/** The connectivity report (dto_edge.rs EdgeConnectivityReportDto). */
export interface EdgeConnectivityReportDto {
  /** `null` = never seen / no reporter composed — the gap, never "now". */
  last_seen_unix_millis: number | null;
  /** Where this connectivity fact comes from (verbatim). */
  source_note: string;
}

/** The data-freshness report (dto_edge.rs DataFreshnessDto). */
export interface DataFreshnessDto {
  last_event_unix_millis: number | null;
  /** The freshness basis note (verbatim). */
  note: string;
}

/** The P6.14 command-path state (dto_edge.rs CommandPathStateDto). */
export interface CommandPathStateDto {
  state: string;
  /** When a TEST_ENV_ENABLED enablement expires (unix millis). */
  enablement_valid_until_unix_millis: number | null;
  /** The verbatim never-enable statement. */
  note: string;
}

/** The per-edge status record (dto_edge.rs EdgeStatusDto). */
export interface EdgeStatusDto {
  device_id: string;
  site_id: string;
  enrollment_seq: number;
  /** Closed registry vocabulary, rendered verbatim (ACTIVE/REVOKED/EXPIRED). */
  enrollment_status: string;
  identity_expires_at_unix_millis: number | null;
  certificate_expiry_unix_millis: number | null;
  connectivity: EdgeConnectivityReportDto;
  /**
   * The P6.17 HealthSnapshot JSON, VERBATIM (the closed HEALTHY /
   * DEGRADED_* / FAILING_* vocabulary as the edge rendered it). Rendered
   * as-is — never prettified into healthy-looking synonyms.
   */
  adapter_health_snapshot_json: string | null;
  buffer_depth: number | null;
  buffer_capacity: number | null;
  data_freshness: DataFreshnessDto;
  command_path: CommandPathStateDto;
  status_note: string;
}

/**
 * Fetches the per-edge status record for one device (the tenant is
 * server-derived from claims — §16.2; NO tenant parameter exists or is
 * sent). Unknown identity → 404; a forbidden role → 403 (rendered as the
 * honest error, never hidden).
 */
export function fetchEdgeStatus(
  deviceId: string,
  client: ApiClient = api,
): Promise<EdgeStatusDto> {
  return client
    .get<{ data: EdgeStatusDto }>(
      `/api/v1/edge/status?device_id=${encodeURIComponent(deviceId)}`,
    )
    .then((envelope) => envelope.data);
}

/**
 * The certificate-expiry treatment (the P5 text-borne treatment pattern,
 * ValueType.tsx): literal word + pinned testid, colour-independent.
 * | Treatment      | When |
 * |----------------|------|
 * | expired        | expires_at <= now |
 * | rotation-due   | expires_at inside the 7-day rotation window (the P6.06/P6.17 window) |
 * | valid          | beyond the rotation window |
 * | not-reported   | the record carries no expiry — the gap, never "valid forever" |
 */
export type CertificateExpiryTreatment =
  | 'expired'
  | 'rotation-due'
  | 'valid'
  | 'not-reported';

/** The rotation window the health surface uses (P6.06/P6.17: 7 days). */
export const CERTIFICATE_ROTATION_WINDOW_MS = 7 * 24 * 60 * 60 * 1000;

export function certificateExpiryTreatment(
  expiresAtUnixMillis: number | null,
  nowMillis: number,
): CertificateExpiryTreatment {
  if (expiresAtUnixMillis === null || !Number.isFinite(expiresAtUnixMillis)) {
    return 'not-reported';
  }
  const remaining = expiresAtUnixMillis - nowMillis;
  if (remaining <= 0) return 'expired';
  if (remaining <= CERTIFICATE_ROTATION_WINDOW_MS) return 'rotation-due';
  return 'valid';
}

/**
 * Whether the command-path state display must carry the proposed-vs-
 * executed distinction (P6.25 step 3): a TEST_ENV_ENABLED path is still
 * TEST-ENV ONLY — a proposed setpoint is never an executed command.
 */
export function commandPathIsEnabled(state: string): boolean {
  return state === 'TEST_ENV_ENABLED';
}
