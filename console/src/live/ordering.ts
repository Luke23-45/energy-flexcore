// P5.33 — client-side ordering and deduplication for the live stream
// (§15.4 at-least-once semantics, §15.1; ledger P5.33).
//
// The contract this module implements (stated per the ledger criteria):
//
// - The stream is AT-LEAST-ONCE and UNORDERED (§15.4; the envelope's
//   `delivery: "at-least-once"` and `ordering_statement` carry the honesty
//   note verbatim — routes_live.rs). Out-of-order or duplicate events must
//   NEVER overwrite fresher displayed state.
// - DEDUPLICATION by event identity: the identity fields mirrored here are
//   `event_type` + `correlation_id` + `event_time` — the correlation fields
//   the SSE envelope actually carries. DRIFT NOTE (read before editing):
//   the platform's durable dedup identity is the transactional-outbox
//   `IdempotencyKey` (crates/flexcore-events/src/outbox.rs, unique in the
//   outbox store — the P4.24 discipline), but `LiveEventEnvelope`
//   (apps/control-plane/src/routes_live.rs) carries NO idempotency-key
//   field, so the console cannot use it; it mirrors the envelope's
//   correlation fields instead. Every frozen §15.3 payload carries its own
//   `correlation_id` (events.rs), so (event_type, correlation_id) already
//   distinguishes distinct events; `event_time` is included as defence.
//   If the envelope ever gains the outbox idempotency key, `identityOf`
//   MUST switch to it in the same review — a console-private identity
//   derivation must never drift from the platform's.
// - ORDERING per (stream, entity), last-write-wins by the event's own
//   `event_time` (§10.3). The ledger's step-1 "per-stream sequence" is only
//   partially satisfiable on this wire: the frozen §15.3 payloads carry NO
//   monotonic sequence number and the envelope adds none, so the console
//   keeps the last-APPLIED event_time per entity (the honest equivalent of
//   the last-applied sequence) and discards events strictly OLDER than it.
//   A real source-sequence requires an envelope field the closed vocabulary
//   does not have — recorded as a gap, NOT invented. The rule PER EVENT
//   TYPE is in ORDERING_RULES below (and restated in the decision record
//   32_33_tenant_live_ordering.md).
// - The P4.24 discipline (rejection_duplicate.rs / dispatch_security.rs
//   SequenceGuard) is reused as a DISCIPLINE, not code: one effective
//   application per identity; a redelivered event produces no visible
//   change; nothing is silently dropped — every discard is COUNTED in the
//   observable diagnostics (ledger P5.33 step 2).
// - RESYNC_REQUIRED (the P5.25 stream sends it on lag): the consumer drops
//   local state and re-fetches from the REST read APIs. `markResync`
//   clears the identity set and the per-entity anchors (the re-fetch is
//   the new baseline); the discard counters are CUMULATIVE session
//   diagnostics and survive the reset. `seedAnchor` lets a consumer
//   re-seed an entity's last-applied time from the re-fetched record so a
//   late redelivery of a pre-gap event cannot overwrite the re-fetched
//   state.
//
// What a client-side guard CANNOT prove (recorded honestly): this module
// orders the console's DISPLAY only. The platform-side deduplication
// (broker duplicate window + consumer idempotency, outbox.rs) and the
// P4.24 SequenceGuard remain the enforcement; the REST re-sync is the
// authority whenever the two could disagree.

import type { LiveEventEnvelope } from './types';
import { classifyEventTag, type LiveEventKind } from './types';

/** Bounded dedup memory: the identity set holds at most this many ids and
 * evicts the OLDEST (insertion order) beyond it. A bound is mandatory —
 * an unbounded set on a long-lived stream is a leak. 4096 identities is
 * far above any duplicate window this platform's outbox retry policy can
 * produce; the bound and its eviction are recorded, not hidden. */
export const SEEN_LIMIT = 4096;

/** One ordering rule per event type, stated honestly. */
export interface OrderingRule {
  /** Which events the rule covers (frozen wire tags). */
  tags: readonly string[];
  /**
   * How the entity key is derived from the envelope (null = dedup-only:
   * the event is a standalone fact; the console merges no mutable state
   * from it, so there is nothing to order — the rule is stated, not
   * assumed). Dispatch events REQUIRE command_id; a dispatch envelope
   * without one is a CONTRACT DEFECT (the frozen dispatch payloads always
   * carry it — events.rs dispatch_payload!) and is never applied.
   */
  entityKey: (envelope: LiveEventEnvelope) => string | null;
  /** Why this rule — the honest statement rendered/tested alongside. */
  statement: string;
}

const scopeJoin = (...parts: Array<string | undefined>): string =>
  parts.map((part) => (part ? part : '-')).join('/');

/** The three §15.1 live areas order per entity; everything outside them
 * (the P5.25 classification, live/types.ts classifyEventTag) is
 * dedup-only. `classifyEventTag` decides the stream; the rule table
 * decides the entity. */
export const ORDERING_RULES: readonly OrderingRule[] = [
  {
    tags: ['TELEMETRY_RECEIVED', 'TELEMETRY_QUALITY_CHANGED'],
    entityKey: (envelope) => scopeJoin(envelope.site_id, envelope.asset_id),
    statement:
      'telemetry: last-write-wins per site/asset by event_time — a telemetry ' +
      'fact older than the one displayed for the same asset is discarded and ' +
      'counted; an equal-event_time DISTINCT fact applies (concurrent facts ' +
      'at one event time cannot be ordered by this wire; the tie rule and ' +
      'the REST re-sync are recorded, not hidden).',
  },
  {
    tags: [
      'SCHEDULE_PROPOSED',
      'SCHEDULE_VALIDATED',
      'FEASIBILITY_CHECK_COMPLETED',
      'FLEXIBILITY_ENVELOPE_UPDATED',
    ],
    entityKey: (envelope) =>
      scopeJoin(envelope.portfolio_id, envelope.site_id, envelope.asset_id),
    statement:
      'schedule state: last-write-wins per portfolio/site/asset by ' +
      'event_time. The key is deliberately coarse (a PROPOSED and a ' +
      'VALIDATED for the same scope share one anchor): the display shows the ' +
      'freshest fact ABOUT that scope, each applied event carries its own ' +
      'event_type, and the REST read APIs remain the authority — the ' +
      'coarseness is a recorded DEFAULT, not a claim that one event ' +
      'invalidates the other.',
  },
  {
    tags: [
      'DISPATCH_PROPOSED',
      'DISPATCH_APPROVED',
      'DISPATCH_REJECTED',
      'DISPATCH_ISSUED',
      'DISPATCH_ACKNOWLEDGED',
      'DISPATCH_TRACKING_UPDATED',
      'DISPATCH_COMPLETED',
      'DISPATCH_FAILED',
      'DISPATCH_EXPIRED',
      'DISPATCH_CANCELLED',
      'DISPATCH_SUPERSEDED',
      'DISPATCH_FALLBACK_ACTIVATED',
    ],
    entityKey: (envelope) => (envelope.command_id ? `command/${envelope.command_id}` : null),
    statement:
      'dispatch lifecycle: last-write-wins per §14.2 command by event_time — ' +
      'a stale lifecycle event arriving after a fresher one can never move ' +
      'the displayed state backwards. A dispatch envelope WITHOUT command_id ' +
      'is a contract defect (the frozen payloads always carry it) and is ' +
      'never applied.',
  },
  {
    tags: [
      'ASSET_REGISTERED',
      'ASSET_CAPABILITY_UPDATED',
      'ASSET_AVAILABILITY_CHANGED',
      'FORECAST_PUBLISHED',
      'MARKET_SIGNAL_RECEIVED',
      'PLUGIN_ACTIVATED',
      'PLUGIN_FAILED',
      'OPERATOR_ACTION_RECORDED',
    ],
    entityKey: () => null,
    statement:
      'outside the three §15.1 live areas (the P5.25 classification): ' +
      'dedup-only, every DISTINCT event applies — these are standalone ' +
      'facts (registry changes, forecasts, plugin/audit records); the ' +
      'console merges no mutable per-entity state from them, so there is ' +
      'nothing to order, and the rule is stated rather than improvised.',
  },
];

/** The rule for one envelope's tag (every frozen tag has exactly one). */
export function ruleForTag(tag: string): OrderingRule {
  const rule = ORDERING_RULES.find((entry) => entry.tags.includes(tag));
  if (!rule) {
    // The event vocabulary is CLOSED (events.rs FROZEN_EVENT_NAMES); an
    // unknown tag means the server changed first — it is a contract
    // defect, never a new feature (live/types.ts drift note).
    throw new Error(`unknown live event tag: ${tag}`);
  }
  return rule;
}

/** The dedup identity mirrored from the envelope's correlation fields (see
 * the module drift note re: the outbox IdempotencyKey). */
export function identityOf(envelope: LiveEventEnvelope): string {
  return [envelope.event_type, envelope.correlation_id, envelope.event_time].join('|');
}

export type OrderingOutcome =
  /** First event for this entity (or a dedup-only stream's distinct event). */
  | 'applied-first'
  /** Strictly newer event_time than the entity's anchor. */
  | 'applied-newer'
  /** Equal event_time, unseen identity (the recorded tie rule). */
  | 'applied-equal-time'
  /** Distinct event outside the three live areas (dedup-only stream). */
  | 'applied-unordered-fact'
  /** Identity already applied — no visible change, no duplicate side effect. */
  | 'discarded-duplicate'
  /** Strictly older than the entity's last-applied event_time. */
  | 'discarded-stale'
  /** Dispatch envelope without command_id, or an unparsable event_time. */
  | 'discarded-contract-defect'
  /** Defence-in-depth: envelope tenant differs from the session tenant
   * (the server filter is fail-closed; this client-side drop is a
   * recorded belt-and-braces check, NOT the enforcement, §16.2). */
  | 'discarded-tenant-mismatch';

export interface OrderingDecision {
  outcome: OrderingOutcome;
  /** The §15.1 stream (classifyEventTag), for diagnostics grouping. */
  stream: LiveEventKind;
  /** The entity key (null for dedup-only streams and defects). */
  entityKey: string | null;
}

export interface OrderingDiagnostics {
  /** Cumulative counters (they survive markResync — a reset must not
   * launder the discard history, ledger P5.33 step 2). */
  applied: number;
  duplicatesDiscarded: number;
  staleDiscarded: number;
  contractDefects: number;
  tenantMismatchesDropped: number;
  resyncs: number;
  /** The last discard, observable in diagnostics (what/why/when). */
  lastDiscard: {
    outcome: OrderingOutcome;
    identity: string;
    at: string;
  } | null;
}

export interface OrderingState {
  /** Bounded seen-identity set (insertion order = oldest first). */
  seen: Set<string>;
  /** entityKey -> last-applied event_time (unix ms). */
  anchors: Map<string, number>;
  diagnostics: OrderingDiagnostics;
}

export function createOrderingState(): OrderingState {
  return {
    seen: new Set(),
    anchors: new Map(),
    diagnostics: {
      applied: 0,
      duplicatesDiscarded: 0,
      staleDiscarded: 0,
      contractDefects: 0,
      tenantMismatchesDropped: 0,
      resyncs: 0,
      lastDiscard: null,
    },
  };
}

/** A read-only snapshot of the cumulative diagnostics (for the UI/tests). */
export function orderingDiagnostics(state: OrderingState): OrderingDiagnostics {
  return { ...state.diagnostics, lastDiscard: state.diagnostics.lastDiscard ? { ...state.diagnostics.lastDiscard } : null };
}

function discard(
  state: OrderingState,
  outcome: OrderingOutcome,
  identity: string,
  counter: 'duplicatesDiscarded' | 'staleDiscarded' | 'contractDefects' | 'tenantMismatchesDropped',
): OrderingDecision {
  state.diagnostics[counter] += 1;
  state.diagnostics.lastDiscard = { outcome, identity, at: new Date().toISOString() };
  return {
    outcome,
    stream: 'other',
    entityKey: null,
  };
}

/**
 * Runs ONE envelope through the ordering/dedup gate. Mutates `state`
 * (anchors, seen set, diagnostics) and returns the decision — the caller
 * applies the event to displayed state ONLY when the outcome is applied-*.
 */
export function applyOrderedEvent(
  state: OrderingState,
  envelope: LiveEventEnvelope,
  sessionTenantId?: string | null,
): OrderingDecision {
  const identity = identityOf(envelope);

  // 0. Defence-in-depth tenancy (NOT the enforcement — the server scopes
  // the stream per caller tenant, fail-closed, §16.2; this drop means the
  // contract broke somewhere and says so in the counters).
  if (sessionTenantId && envelope.tenant_id !== sessionTenantId) {
    const decision = discard(state, 'discarded-tenant-mismatch', identity, 'tenantMismatchesDropped');
    decision.stream = classifyEventTag(envelope.event_type);
    return decision;
  }

  // 1. Dedup: a redelivered event produces no visible change and no
  // duplicate side effect (§15.4 idempotent consumer; P4.24 discipline).
  if (state.seen.has(identity)) {
    const decision = discard(state, 'discarded-duplicate', identity, 'duplicatesDiscarded');
    decision.stream = classifyEventTag(envelope.event_type);
    return decision;
  }
  state.seen.add(identity);
  if (state.seen.size > SEEN_LIMIT) {
    const oldest = state.seen.values().next().value;
    if (oldest !== undefined) state.seen.delete(oldest);
  }

  // 2. Contract checks before any application.
  const eventTimeMs = Date.parse(envelope.event_time);
  if (!Number.isFinite(eventTimeMs)) {
    const decision = discard(state, 'discarded-contract-defect', identity, 'contractDefects');
    decision.stream = classifyEventTag(envelope.event_type);
    return decision;
  }
  let rule: OrderingRule;
  try {
    rule = ruleForTag(envelope.event_type);
  } catch {
    const decision = discard(state, 'discarded-contract-defect', identity, 'contractDefects');
    decision.stream = classifyEventTag(envelope.event_type);
    return decision;
  }
  const entityKey = rule.entityKey(envelope);
  const stream = classifyEventTag(envelope.event_type);
  if (entityKey === null) {
    // Dedup-only stream (or a dispatch envelope missing command_id).
    if (stream === 'dispatch') {
      const decision = discard(state, 'discarded-contract-defect', identity, 'contractDefects');
      decision.stream = stream;
      return decision;
    }
    state.diagnostics.applied += 1;
    return { outcome: 'applied-unordered-fact', stream, entityKey: null };
  }

  // 3. Ordering: per-entity last-write-wins by event_time.
  const anchor = state.anchors.get(entityKey);
  if (anchor !== undefined && eventTimeMs < anchor) {
    const decision = discard(state, 'discarded-stale', identity, 'staleDiscarded');
    decision.stream = stream;
    decision.entityKey = entityKey;
    return decision;
  }
  state.anchors.set(entityKey, eventTimeMs);
  state.diagnostics.applied += 1;
  return {
    outcome: anchor === undefined ? 'applied-first' : eventTimeMs === anchor ? 'applied-equal-time' : 'applied-newer',
    stream,
    entityKey,
  };
}

/**
 * RESYNC_REQUIRED / reconnect handling (§15.4; P5.25/P5.33): clears the
 * identity set and the per-entity anchors so the re-fetched REST records
 * are the new baseline. The counters are cumulative and survive. The
 * consumer MUST re-fetch (the needsResync contract, P5.25) and SHOULD
 * re-seed anchors from the fetched records via seedAnchor — between the
 * reset and the re-seed a pre-gap redelivery could apply against a missing
 * anchor; the REST re-fetch is the authority that window.
 */
export function markResync(state: OrderingState): void {
  state.seen.clear();
  state.anchors.clear();
  state.diagnostics.resyncs += 1;
}

/** Re-seeds one entity's last-applied event_time from a re-fetched record
 * (after markResync). Exported for consumers; no workspace wires it yet
 * (the workspaces were read-only in this pass) — recorded, like
 * DemoDataBadge, as a defined-for-use hook. */
export function seedAnchor(state: OrderingState, entityKey: string, eventTimeIso: string): void {
  const ms = Date.parse(eventTimeIso);
  if (!Number.isFinite(ms)) return;
  const anchor = state.anchors.get(entityKey);
  if (anchor === undefined || ms > anchor) state.anchors.set(entityKey, ms);
}
