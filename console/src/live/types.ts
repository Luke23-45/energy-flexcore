// P5.25 — client-side mirror of the frozen §15.3 event vocabulary and the
// SSE envelope the control plane streams (routes_live.rs LiveEventEnvelope).
//
// DRIFT NOTE (read before editing): these TS types MIRROR, field-for-field,
// the frozen payloads in crates/flexcore-events/src/events.rs and the
// envelope in apps/control-plane/src/routes_live.rs. The wire names are
// snake_case and the event tags are the frozen SCREAMING_SNAKE_CASE
// spellings (§15.3). The event vocabulary is CLOSED (26 events; no
// DispatchValidated/DispatchFeasible exists and none may be invented —
// events.rs). If a payload changes upstream, this mirror MUST change in
// the same review; the SSE server decodes only the frozen vocabulary, so
// an unknown tag arriving here means the server changed first — treat it
// as a contract defect, not a new feature.
//
// Delivery honesty (the contract every consumer accepts by using this
// module): the stream is at-least-once and UNORDERED (§15.4). Events may
// be duplicated or reordered; consumers MUST NOT treat arrival order as
// occurrence order and MUST re-sync from the REST read APIs after a gap or
// a RESYNC_REQUIRED notice — never silently gap-fill. Ordering/dedup is
// P5.33 and is deliberately NOT implemented here.

/** The frozen six-state telemetry quality vocabulary (§10.4). */
export type TelemetryQuality =
  | 'GOOD'
  | 'SUSPECT'
  | 'BAD'
  | 'MISSING'
  | 'ESTIMATED'
  | 'STALE';

/** `TelemetryReceived` (events.rs TelemetryReceivedPayload). */
export interface TelemetryReceivedPayload {
  correlation_id: string;
  tenant_id: string;
  site_id: string;
  asset_id: string;
  event_time: string;
}

/** `TelemetryQualityChanged` (P2.16 shape: previous AND new quality). */
export interface TelemetryQualityChangedPayload {
  correlation_id: string;
  tenant_id: string;
  site_id: string;
  asset_id: string;
  quality: TelemetryQuality;
  previous_quality?: TelemetryQuality;
  event_time: string;
}

/** `AssetAvailabilityChanged` (site/asset scope). */
export interface AssetAvailabilityChangedPayload {
  correlation_id: string;
  tenant_id: string;
  site_id: string;
  asset_id: string;
  event_time: string;
}

/** `ScheduleProposed` / `ScheduleValidated` (scheduling). */
export interface SchedulePayload {
  correlation_id: string;
  tenant_id: string;
  portfolio_id: string;
  site_id: string;
  asset_id: string;
  event_time: string;
}

/** `FlexibilityEnvelopeUpdated` (flexibility). */
export interface FlexibilityEnvelopeUpdatedPayload {
  correlation_id: string;
  tenant_id: string;
  portfolio_id: string;
  site_id: string;
  asset_id: string;
  event_time: string;
}

/** Any dispatch-lifecycle payload (events.rs dispatch_payload! macro):
 * every dispatch event carries the §14.2 command id. */
export interface DispatchLifecyclePayload {
  correlation_id: string;
  tenant_id: string;
  portfolio_id: string;
  site_id: string;
  asset_id: string;
  command_id: string;
  event_time: string;
}

/** The 26 frozen §15.3 wire tags (SCREAMING_SNAKE spelling of the frozen
 * PascalCase names; the closed list — events.rs FROZEN_EVENT_NAMES). */
export type FrozenEventTag =
  | 'ASSET_REGISTERED'
  | 'ASSET_CAPABILITY_UPDATED'
  | 'TELEMETRY_RECEIVED'
  | 'TELEMETRY_QUALITY_CHANGED'
  | 'ASSET_AVAILABILITY_CHANGED'
  | 'FLEXIBILITY_ENVELOPE_UPDATED'
  | 'FORECAST_PUBLISHED'
  | 'MARKET_SIGNAL_RECEIVED'
  | 'SCHEDULE_PROPOSED'
  | 'SCHEDULE_VALIDATED'
  | 'FEASIBILITY_CHECK_COMPLETED'
  | 'DISPATCH_PROPOSED'
  | 'DISPATCH_APPROVED'
  | 'DISPATCH_REJECTED'
  | 'DISPATCH_ISSUED'
  | 'DISPATCH_ACKNOWLEDGED'
  | 'DISPATCH_TRACKING_UPDATED'
  | 'DISPATCH_COMPLETED'
  | 'DISPATCH_FAILED'
  | 'DISPATCH_EXPIRED'
  | 'DISPATCH_CANCELLED'
  | 'DISPATCH_SUPERSEDED'
  | 'DISPATCH_FALLBACK_ACTIVATED'
  | 'PLUGIN_ACTIVATED'
  | 'PLUGIN_FAILED'
  | 'OPERATOR_ACTION_RECORDED';

/** The SSE data envelope (routes_live.rs LiveEventEnvelope). */
export interface LiveEventEnvelope {
  event_type: FrozenEventTag;
  /** The frozen §15.3 PascalCase name. */
  event_name: string;
  subject: string;
  /** §21.3: the SAME id the audit record carries. */
  correlation_id: string;
  tenant_id: string;
  portfolio_id?: string;
  site_id?: string;
  asset_id?: string;
  /** Dispatch-lifecycle events only. */
  command_id?: string;
  plugin_id?: string;
  /** Event time (§10.3), RFC 3339 UTC. */
  event_time: string;
  /** Server delivery time (the live-vs-stale freshness anchor, P5.10). */
  delivered_at: string;
  /** Always "at-least-once". */
  delivery: string;
  /** The duplicate/reorder honesty statement. */
  ordering_statement: string;
}

/** The RESYNC_REQUIRED body (routes_live.rs LiveResyncNotice). */
export interface LiveResyncNotice {
  notice: string;
  skipped_events: number;
  statement: string;
}

/** What a workspace consumer does with one live event (§15.1: telemetry,
 * schedule state, dispatch lifecycle are the report's live areas). */
export type LiveEventKind = 'telemetry' | 'schedule' | 'dispatch' | 'other';

/** Classifies a frozen wire tag into the §15.1 live-update area. */
export function classifyEventTag(tag: FrozenEventTag): LiveEventKind {
  if (tag === 'TELEMETRY_RECEIVED' || tag === 'TELEMETRY_QUALITY_CHANGED') {
    return 'telemetry';
  }
  if (
    tag === 'SCHEDULE_PROPOSED' ||
    tag === 'SCHEDULE_VALIDATED' ||
    tag === 'FLEXIBILITY_ENVELOPE_UPDATED' ||
    tag === 'FEASIBILITY_CHECK_COMPLETED'
  ) {
    return 'schedule';
  }
  if (tag.startsWith('DISPATCH_')) {
    return 'dispatch';
  }
  return 'other';
}
