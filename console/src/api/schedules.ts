// P5.05 — the Schedules-workspace API client (routes_phase3.rs: the
// /api/v1/schedules and /api/v1/optimization-runs resources, READ-ONLY on
// the console surface). SHAPE-MIRROR: see types.ts (drift note).
//
// Why a second module next to flexibility.ts (recorded, not duplication for
// its own sake): flexibility.ts (P5.01) mirrors only the bare
// cursor/limit params. The Schedules workspace needs the FULL Phase3ListParams
// the server actually accepts (routes_phase3.rs: portfolio_id, site_id,
// status, schedule_id — a schedule_id filter returns the FULL revision chain
// including SUPERSEDED revisions, without pagination), and the workspace
// contract (P5.05 step 4: superseded revisions retained and retrievable)
// depends on that chain filter. The URL and wire param names mirror
// routes_phase3.rs verbatim; a server rename breaks this module on purpose
// (contract drift fails visibly).
//
// No write path exists here. The POST resources (schedule propose,
// optimization-run register) are application/operator-tooling surfaces, not
// console surfaces in this phase (§15.2; the console consumes evidence).
import { api, type ApiClient } from './client';
import type {
  OptimizationRunDto,
  PageEnvelope,
  ScheduleRevisionDto,
} from './types';

/**
 * The full Phase 3 listing params (routes_phase3.rs Phase3ListParams,
 * verbatim wire names). A malformed filter UUID is 422; the schedule_id
 * filter returns the FULL chain (all revisions, superseded included)
 * without pagination.
 */
export interface ScheduleChainListParams {
  portfolio_id?: string;
  /** The schedule-chain identity: returns the FULL revision chain. */
  schedule_id?: string;
  cursor?: string;
  limit?: number;
}

/** Optimization-run listing params (routes_phase3.rs Phase3ListParams). */
export interface OptimizationRunListParams {
  portfolio_id?: string;
  site_id?: string;
  /** One closed §12.6 status value (unknown value is 422). */
  status?: string;
  cursor?: string;
  limit?: number;
}

function buildQuery(params: Record<string, string | number | undefined>): string {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined) search.set(key, String(value));
  }
  const query = search.toString();
  return query ? `?${query}` : '';
}

/** GET /api/v1/schedules (routes_phase3.rs list_schedules). */
export function listScheduleRevisions(
  params?: ScheduleChainListParams,
  client: ApiClient = api,
): Promise<PageEnvelope<ScheduleRevisionDto>> {
  const query = buildQuery({
    portfolio_id: params?.portfolio_id,
    schedule_id: params?.schedule_id,
    cursor: params?.cursor,
    limit: params?.limit,
  });
  return client.get(`/api/v1/schedules${query}`);
}

/** GET /api/v1/optimization-runs (routes_phase3.rs list_optimization_runs). */
export function listOptimizationRunRecords(
  params?: OptimizationRunListParams,
  client: ApiClient = api,
): Promise<PageEnvelope<OptimizationRunDto>> {
  const query = buildQuery({
    portfolio_id: params?.portfolio_id,
    site_id: params?.site_id,
    status: params?.status,
    cursor: params?.cursor,
    limit: params?.limit,
  });
  return client.get(`/api/v1/optimization-runs${query}`);
}
