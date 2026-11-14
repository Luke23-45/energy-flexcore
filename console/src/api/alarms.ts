// P5.31 — the alarm client (routes_alarms.rs: GET /api/v1/alarms — the
// derived-on-read alarm resource; §20.1 item 1). SHAPE-MIRROR: see types.ts
// (drift note) — these types mirror dto_alarms.rs field-for-field in the
// WIRE names, so a payload mismatch fails visibly instead of silently.
//
// Honesty contract carried by every response (the console renders, never
// paraphrases):
// - alarms are OPERATOR-ATTENTION records derived on read from existing
//   platform facts (telemetry quality states, dispatch outcomes,
//   feasibility verdicts) — NOT protection signals;
// - the derivation_statement is the alarm-model statement, verbatim;
// - the sources block names which alarm sources were consulted, which were
//   unavailable, and which DO NOT EXIST (no invented alarms);
// - there is no alarm write path anywhere (no acknowledge endpoint) and
//   the frozen §15.3 event vocabulary is not extended.
import { api, type ApiClient } from './client';

/** The closed severity vocabulary (flexcore-contracts::alarm::AlarmSeverity). */
export const ALARM_SEVERITIES = ['INFO', 'WARNING', 'CRITICAL'] as const;
export type AlarmSeverity = (typeof ALARM_SEVERITIES)[number];

/** The closed code vocabulary (AlarmCode — one code per EXISTING source). */
export const ALARM_CODES = [
  'TELEMETRY_LOSS',
  'TELEMETRY_STALENESS',
  'FEASIBILITY_LIMIT_VIOLATED',
  'FEASIBILITY_FAIL_CLOSED',
  'DISPATCH_FAILURE',
  'DISPATCH_EXPIRY',
  'DISPATCH_FALLBACK',
] as const;
export type AlarmCode = (typeof ALARM_CODES)[number];

/** The alarm lifecycle state (ACTIVE only on the derived-on-read path). */
export type AlarmStateVocabulary = 'ACTIVE' | 'ACKNOWLEDGED' | 'CLEARED';

/** The kind of platform record an alarm was derived from. */
export type AlarmSourceKind =
  | 'TELEMETRY_CURRENT_STATE'
  | 'DISPATCH_COMMAND'
  | 'FEASIBILITY_CHECK';

/** One derived alarm record (dto_alarms.rs AlarmDto, wire names). */
export interface AlarmDto {
  /** Deterministic key: `flexcore:alarm:<CODE>:<scope>`. */
  alarm_id: string;
  tenant_id: string;
  site_id?: string;
  asset_id?: string;
  code: AlarmCode;
  severity: AlarmSeverity;
  /** ACTIVE on the derived-on-read path. */
  state: AlarmStateVocabulary;
  /** The deterministic operator-facing message. */
  message: string;
  /** The source observation's event time (RFC 3339), never ingest time. */
  raised_at: string;
  raised_at_unix_millis: number;
  /** Always absent on the derived-on-read path (no persistence). */
  cleared_at?: string;
  /** The derivation instant (the read) — distinct from raised_at. */
  derived_at: string;
  source_kind: AlarmSourceKind;
  /** The reference to the exact source record (the evidence link). */
  source_reference: string;
  derivation_rule: string;
}

/** Which sources were consulted and which do not exist (dto_alarms.rs). */
export interface AlarmSourcesDto {
  telemetry_current_state: string;
  telemetry_assets_consulted: number;
  dispatch_commands_consulted: number;
  feasibility_checks_consulted: number;
  skipped_defective_source_records: number;
  /** The sources that DO NOT exist in this phase, named explicitly. */
  absent_sources: string[];
}

/** The response envelope (dto_alarms.rs AlarmPageEnvelope). */
export interface AlarmPageEnvelope {
  data: AlarmDto[];
  sources: AlarmSourcesDto;
  derived_at: string;
  /** The alarm-model statement, verbatim. */
  derivation_statement: string;
  model_version: string;
  /** Always null: a single bounded derivation, no cursor (by contract). */
  next_cursor: string | null;
}

export interface AlarmListParams {
  severity?: AlarmSeverity;
  code?: AlarmCode;
  assetId?: string;
  siteId?: string;
  fromEventUnixMillis?: number;
  toEventUnixMillis?: number;
  /** Only `true` (or unset) is valid: active_only=false is a 422 by
   * contract (derived-on-read — no alarm history exists). */
  activeOnly?: true;
  limit?: number;
}

/** Lists the tenant's currently derivable alarms (GET /api/v1/alarms). */
export function listAlarms(
  params?: AlarmListParams,
  client: ApiClient = api,
): Promise<AlarmPageEnvelope> {
  const search = new URLSearchParams();
  if (params?.severity !== undefined) search.set('severity', params.severity);
  if (params?.code !== undefined) search.set('code', params.code);
  if (params?.assetId !== undefined) search.set('asset_id', params.assetId);
  if (params?.siteId !== undefined) search.set('site_id', params.siteId);
  if (params?.fromEventUnixMillis !== undefined) {
    search.set('from_event_unix_millis', String(params.fromEventUnixMillis));
  }
  if (params?.toEventUnixMillis !== undefined) {
    search.set('to_event_unix_millis', String(params.toEventUnixMillis));
  }
  if (params?.activeOnly !== undefined) search.set('active_only', 'true');
  if (params?.limit !== undefined) search.set('limit', String(params.limit));
  const query = search.toString();
  return client.get(`/api/v1/alarms${query ? `?${query}` : ''}`);
}
