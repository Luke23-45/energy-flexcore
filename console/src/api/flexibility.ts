// P5.01 — Phase 3 client (routes_phase3.rs: flexibility envelopes,
// schedules, optimization runs — the §15.2 resources of the Phase 3 line).
// SHAPE-MIRROR: see types.ts (drift note).
//
// READ-ONLY in this scaffold: the POST resources (envelope create, schedule
// propose, optimization-run create) exist on the server but the console's
// write flows are the Schedules workspace's job (P5.05), including the
// §20.2 rule that objective selection is choosing among the DECLARED
// objectives — never free-text. This module ships the reads only.
import { api, type ApiClient } from './client';
import type {
  EnvelopeRecordDto,
  OptimizationRunDto,
  PageEnvelope,
  ScheduleRevisionDto,
} from './types';

export interface Phase3ListParams {
  /** Opaque keyset cursor from the previous page (malformed = 422). */
  cursor?: string;
  limit?: number;
}

function pageQuery(params?: Phase3ListParams): string {
  if (!params) return '';
  const search = new URLSearchParams();
  if (params.cursor !== undefined) search.set('cursor', params.cursor);
  if (params.limit !== undefined) search.set('limit', String(params.limit));
  const query = search.toString();
  return query ? `?${query}` : '';
}

export function listFlexibilityEnvelopes(
  params?: Phase3ListParams,
  client: ApiClient = api,
): Promise<PageEnvelope<EnvelopeRecordDto>> {
  return client.get(`/api/v1/flexibility/envelopes${pageQuery(params)}`);
}

export function listSchedules(
  params?: Phase3ListParams,
  client: ApiClient = api,
): Promise<PageEnvelope<ScheduleRevisionDto>> {
  return client.get(`/api/v1/schedules${pageQuery(params)}`);
}

export function listOptimizationRuns(
  params?: Phase3ListParams,
  client: ApiClient = api,
): Promise<PageEnvelope<OptimizationRunDto>> {
  return client.get(`/api/v1/optimization-runs${pageQuery(params)}`);
}
