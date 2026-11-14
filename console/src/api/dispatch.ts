// P5.01 — Phase 4 client (routes_phase4.rs: dispatch commands,
// feasibility checks, audit events — the §15.2 resources of the Phase 4
// line, READ-ONLY). SHAPE-MIRROR: see types.ts (drift note).
//
// The §14.1 gate transitions (validate/approve/…) are application-layer
// gates, NOT REST (routes_phase4.rs header); the console's Dispatch
// workspace (P5.06) consumes the read surface and the recorded gate-safety
// statements (`no_execute_endpoint_statement`, `simulation_only_statement`
// travel IN the payloads — the console renders them, never paraphrases
// them). No write path exists here, and none may be invented.
import { api, type ApiClient } from './client';
import type {
  AuditEventDto,
  DataEnvelope,
  DispatchCommandDetailDto,
  DispatchCommandDto,
  FeasibilityCheckDto,
  PageEnvelope,
} from './types';

export interface Phase4ListParams {
  /** Opaque keyset cursor from the previous page (malformed = 422). */
  cursor?: string;
  limit?: number;
}

/** Dispatch-command list filters (routes_phase4.rs Phase4ListParams). */
export interface DispatchCommandListParams extends Phase4ListParams {
  /** A §14.2 command state (closed vocabulary; an unknown value is 422). */
  state?: string;
}

export interface FeasibilityCheckListParams extends Phase4ListParams {
  commandId?: string;
  scheduleId?: string;
}

export function listDispatchCommands(
  params?: DispatchCommandListParams,
  client: ApiClient = api,
): Promise<PageEnvelope<DispatchCommandDto>> {
  const search = new URLSearchParams();
  if (params?.state !== undefined) search.set('state', params.state);
  if (params?.cursor !== undefined) search.set('cursor', params.cursor);
  if (params?.limit !== undefined) search.set('limit', String(params.limit));
  const query = search.toString();
  return client.get(`/api/v1/dispatch-commands${query ? `?${query}` : ''}`);
}

export function getDispatchCommand(
  commandId: string,
  client: ApiClient = api,
): Promise<DataEnvelope<DispatchCommandDetailDto>> {
  return client.get(`/api/v1/dispatch-commands/${encodeURIComponent(commandId)}`);
}

export function listFeasibilityChecks(
  params?: FeasibilityCheckListParams,
  client: ApiClient = api,
): Promise<PageEnvelope<FeasibilityCheckDto>> {
  const search = new URLSearchParams();
  if (params?.commandId !== undefined) search.set('command_id', params.commandId);
  if (params?.scheduleId !== undefined) search.set('schedule_id', params.scheduleId);
  if (params?.cursor !== undefined) search.set('cursor', params.cursor);
  if (params?.limit !== undefined) search.set('limit', String(params.limit));
  const query = search.toString();
  return client.get(`/api/v1/feasibility-checks${query ? `?${query}` : ''}`);
}

export function getFeasibilityCheck(
  checkId: string,
  client: ApiClient = api,
): Promise<DataEnvelope<FeasibilityCheckDto>> {
  return client.get(`/api/v1/feasibility-checks/${encodeURIComponent(checkId)}`);
}

export interface AuditEventListParams extends Phase4ListParams {
  action?: string;
  actor?: string;
  objectType?: string;
  objectId?: string;
  fromEventUnixMillis?: number;
  toEventUnixMillis?: number;
}

export function listAuditEvents(
  params?: AuditEventListParams,
  client: ApiClient = api,
): Promise<PageEnvelope<AuditEventDto>> {
  const search = new URLSearchParams();
  if (params?.action !== undefined) search.set('action', params.action);
  if (params?.actor !== undefined) search.set('actor', params.actor);
  if (params?.objectType !== undefined) search.set('object_type', params.objectType);
  if (params?.objectId !== undefined) search.set('object_id', params.objectId);
  if (params?.fromEventUnixMillis !== undefined) {
    search.set('from_event_unix_millis', String(params.fromEventUnixMillis));
  }
  if (params?.toEventUnixMillis !== undefined) {
    search.set('to_event_unix_millis', String(params.toEventUnixMillis));
  }
  if (params?.cursor !== undefined) search.set('cursor', params.cursor);
  if (params?.limit !== undefined) search.set('limit', String(params.limit));
  const query = search.toString();
  return client.get(`/api/v1/audit-events${query ? `?${query}` : ''}`);
}

export function getAuditEvent(
  auditId: string,
  client: ApiClient = api,
): Promise<DataEnvelope<AuditEventDto>> {
  return client.get(`/api/v1/audit-events/${encodeURIComponent(auditId)}`);
}
