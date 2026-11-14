// P5.25 — the React hook binding for the live stream (§15.1).
//
// What a workspace gets: the connection status (to render "live updates
// paused — data may be stale" with the last delivery time — never a
// silent continue), the needsResync flag (set on ANY gap: a disconnect, a
// server RESYNC_REQUIRED, or a reconnect; the caller re-syncs from the
// REST read APIs and then acknowledges), and the raw envelopes (typed
// mirrors, see types.ts). StrictMode-correct: the effect's cleanup closes
// the stream, so double-mounting cannot leak a connection.
//
// P5.33 (additive): every envelope now also passes through the
// ordering/dedup gate (live/ordering.ts — per-entity last-write-wins by
// event_time, dedup by identity, contract-defect and tenant-mismatch
// drops, all counted). `lastEvent` stays the RAW last delivery (the P5.25
// contract, unchanged); `lastAppliedEvent` is the last envelope that
// PASSED the gate (the one a workspace may apply to displayed state), and
// `ordering` is the cumulative observable diagnostics. On ANY gap
// (RESYNC_REQUIRED notice or a paused-stale status) the ordering ledger is
// reset (markResync) — the re-fetch from the REST read APIs is the new
// baseline; acknowledgeResync is where a consumer states the re-sync is
// done. The gate is display-side only; the server's fail-closed tenant
// filter and the platform's outbox/consumer idempotency (P4.24) remain the
// enforcement.
import { useEffect, useRef, useState } from 'react';
import { devSessionToken } from '../api/client';
import { getSession } from '../state/tenantContext';
import { connectLiveStream, type LiveStreamHandle, type LiveStreamStatus } from './sseClient';
import {
  applyOrderedEvent,
  createOrderingState,
  markResync,
  orderingDiagnostics,
  type OrderingDiagnostics,
} from './ordering';
import type { LiveEventEnvelope, LiveResyncNotice } from './types';

export interface LiveSession {
  status: LiveStreamStatus;
  /** Server delivery time of the last event (the freshness anchor), RFC
   * 3339; null before the first delivery. */
  lastDeliveredAt: string | null;
  /**
   * True when the session has a GAP the consumer has not re-synced yet:
   * after a disconnect, a RESYNC_REQUIRED notice, or a reconnect. The
   * consumer re-syncs from the REST read APIs and then calls
   * acknowledgeResync(). Never set by an event that merely arrives late —
   * ordering/dedup is P5.33.
   */
  needsResync: boolean;
  /** The last RESYNC_REQUIRED notice (skipped count etc.), if any. */
  lastResyncNotice: LiveResyncNotice | null;
  /** The last envelope received (kept so small views can react directly). */
  lastEvent: LiveEventEnvelope | null;
  /**
   * P5.33: the last envelope that PASSED the ordering/dedup gate — the
   * only envelope a workspace may apply to displayed state. Null before
   * the first applied event; a duplicate/stale/defective delivery never
   * lands here (it lands in the diagnostics instead).
   */
  lastAppliedEvent: LiveEventEnvelope | null;
  /** P5.33: the cumulative ordering/dedup diagnostics (observable). */
  ordering: OrderingDiagnostics;
  acknowledgeResync(): void;
}

export function useLiveEvents(url = '/api/v1/live'): LiveSession {
  const [status, setStatus] = useState<LiveStreamStatus>('connecting');
  const [lastDeliveredAt, setLastDeliveredAt] = useState<string | null>(null);
  const [lastResyncNotice, setLastResyncNotice] = useState<LiveResyncNotice | null>(null);
  const [lastEvent, setLastEvent] = useState<LiveEventEnvelope | null>(null);
  const [lastAppliedEvent, setLastAppliedEvent] = useState<LiveEventEnvelope | null>(null);
  const [ordering, setOrdering] = useState<OrderingDiagnostics>(() =>
    orderingDiagnostics(createOrderingState()),
  );
  const orderingRef = useRef(createOrderingState());
  const needsResyncRef = useRef(false);
  const [needsResync, setNeedsResync] = useState(false);

  useEffect(() => {
    let handle: LiveStreamHandle | null = null;
    let disposed = false;
    handle = connectLiveStream({
      url,
      // The stream REQUIRES the Authorization bearer (the fetch-based SSE
      // decision exists precisely for this header — see sseClient.ts). The
      // token comes from the SAME claims-derived session store the REST
      // client uses (state/tenantContext.ts + api/client.ts devSessionToken)
      // — never from a URL query string, which would leak the credential
      // into logs/history (P5.25 step 5). Until P5.26 this is the recorded
      // dev-auth path only; the swap to the real session is here, in this
      // one provider, not scattered through the client.
      tokenProvider: () => {
        const session = getSession();
        return session ? devSessionToken(session) : null;
      },
      onStatusChange: (next) => {
        if (disposed) return;
        setStatus(next);
      },
      onEnvelope: (envelope) => {
        if (disposed) return;
        setLastEvent(envelope);
        setLastDeliveredAt(envelope.delivered_at);
        // P5.33: the gate decides whether this delivery may touch
        // displayed state. Defence-in-depth tenant check against the
        // claims-derived session (the server filter is the enforcement).
        const decision = applyOrderedEvent(
          orderingRef.current,
          envelope,
          getSession()?.tenantId ?? null,
        );
        if (
          decision.outcome === 'applied-first' ||
          decision.outcome === 'applied-newer' ||
          decision.outcome === 'applied-equal-time' ||
          decision.outcome === 'applied-unordered-fact'
        ) {
          setLastAppliedEvent(envelope);
        }
        setOrdering(orderingDiagnostics(orderingRef.current));
      },
      onResyncNotice: (notice) => {
        if (disposed) return;
        // P5.33: a RESYNC_REQUIRED (or any gap) resets the ordering
        // ledger — the re-fetch is the new baseline. Counters survive.
        markResync(orderingRef.current);
        setOrdering(orderingDiagnostics(orderingRef.current));
        needsResyncRef.current = true;
        setNeedsResync(true);
        setLastResyncNotice(notice);
      },
    });
    return () => {
      disposed = true;
      handle?.close();
    };
  }, [url]);

  // A status drop to paused-stale is itself a gap: the hook owns the flag
  // for that case too (the consumer never has to watch the status). The
  // ordering ledger resets with it (P5.33 — the reconnect's re-fetch is
  // the baseline; a pre-gap redelivery must not be trusted against
  // pre-gap anchors).
  useEffect(() => {
    if (status === 'paused-stale') {
      markResync(orderingRef.current);
      setOrdering(orderingDiagnostics(orderingRef.current));
      needsResyncRef.current = true;
      setNeedsResync(true);
    }
  }, [status]);

  return {
    status,
    lastDeliveredAt,
    needsResync,
    lastResyncNotice,
    lastEvent,
    lastAppliedEvent,
    ordering,
    acknowledgeResync(): void {
      needsResyncRef.current = false;
      setNeedsResync(false);
    },
  };
}
