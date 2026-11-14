// P5.01 — Phase 2 telemetry client (routes_phase2.rs read routes).
// SHAPE-MIRROR: see types.ts (drift note). All telemetry on this surface
// is simulator-derived and carries `simulated: true` (P2.12); the console
// MUST keep that flag visible wherever the value is shown (§20.2; P5.02/
// P5.03 own the value-type treatments).
import { api, type ApiClient } from './client';
import type {
  CurrentStateDto,
  DataEnvelope,
  PageEnvelope,
  TelemetryRecordDto,
} from './types';

export interface TelemetryQuery {
  assetId: string;
  /** Inclusive range start, RFC 3339 (required server-side — 422 when absent). */
  from: string;
  /** Inclusive range end, RFC 3339 (required server-side — 422 when absent). */
  until: string;
  /** Comma-separated canonical quantity ids (absent = all). */
  quantities?: string;
  /** One canonical quality state (the frozen six). */
  quality?: string;
  cursor?: string;
  limit?: number;
}

export function queryTelemetry(
  query: TelemetryQuery,
  client: ApiClient = api,
): Promise<PageEnvelope<TelemetryRecordDto>> {
  const params = new URLSearchParams();
  // Wire param names mirror routes_phase2.rs TelemetryQueryParams verbatim.
  params.set('asset', query.assetId);
  params.set('from', query.from);
  params.set('until', query.until);
  if (query.quantities !== undefined) params.set('quantities', query.quantities);
  if (query.quality !== undefined) params.set('quality', query.quality);
  if (query.cursor !== undefined) params.set('cursor', query.cursor);
  if (query.limit !== undefined) params.set('limit', String(query.limit));
  return client.get(`/api/v1/telemetry/query?${params.toString()}`);
}

export interface TelemetryHistoryQuery {
  assetId: string;
  quantities: string;
  from: string;
  until: string;
  cursor?: string;
  limit?: number;
}

export function queryTelemetryHistory(
  query: TelemetryHistoryQuery,
  client: ApiClient = api,
): Promise<PageEnvelope<unknown>> {
  const params = new URLSearchParams();
  params.set('asset', query.assetId);
  params.set('quantities', query.quantities);
  params.set('from', query.from);
  params.set('until', query.until);
  if (query.cursor !== undefined) params.set('cursor', query.cursor);
  if (query.limit !== undefined) params.set('limit', String(query.limit));
  return client.get(`/api/v1/telemetry/history?${params.toString()}`);
}

export function getAssetCurrentState(
  assetId: string,
  client: ApiClient = api,
): Promise<DataEnvelope<CurrentStateDto>> {
  return client.get(`/api/v1/assets/${encodeURIComponent(assetId)}/state`);
}
