// P5.33 — ordering/dedup gate tests (§15.4 at-least-once; the P4.24
// discipline reused: one effective application per identity, nothing
// silently dropped — every discard is counted).
// UNRUN — never executed in the Phase 5 loop (no install/build/test;
// decision record: 32_33_tenant_live_ordering.md). The human executes
// these via `npm test` after install.
import { describe, expect, it } from 'vitest';
import {
  ORDERING_RULES,
  SEEN_LIMIT,
  applyOrderedEvent,
  createOrderingState,
  identityOf,
  markResync,
  orderingDiagnostics,
  ruleForTag,
  seedAnchor,
} from './ordering';
import { FROZEN_EVENT_NAMES } from '../components/stateVocabulary';
import type { LiveEventEnvelope } from './types';

/**
 * The frozen §15.3 PascalCase names (stateVocabulary.ts, verbatim) rendered
 * as their SCREAMING_SNAKE wire tags — the same deterministic conversion
 * events.rs uses for the wire (`"TelemetryReceived"` -> `TELEMETRY_RECEIVED`).
 * The coverage test below cross-checks BOTH vocabularies against the rule
 * table: if either drifts, this test cannot hold.
 */
const FROZEN_EVENT_TAGS: readonly string[] = FROZEN_EVENT_NAMES.map((name) =>
  name.replace(/([a-z0-9])([A-Z])/g, '$1_$2').toUpperCase(),
);

const TENANT = '00000000-0000-0000-0000-000000000001';

let seq = 0;
function envelope(overrides: Partial<LiveEventEnvelope> = {}): LiveEventEnvelope {
  seq += 1;
  return {
    event_type: 'TELEMETRY_RECEIVED',
    event_name: 'TelemetryReceived',
    subject: `flexcore.v1.telemetry.telemetry_received#${seq}`,
    correlation_id: `00000000-0000-0000-0000-${String(seq).padStart(12, '0')}`,
    tenant_id: TENANT,
    site_id: 'site-1',
    asset_id: 'asset-1',
    event_time: '2026-11-12T10:00:00Z',
    delivered_at: '2026-11-12T10:00:01Z',
    delivery: 'at-least-once',
    ordering_statement: 'events may be duplicated or reordered',
    ...overrides,
  };
}

describe('deduplication by event identity (§15.4; P4.24 discipline)', () => {
  it('a redelivered event is discarded as a duplicate and counted — no second application', () => {
    const state = createOrderingState();
    const event = envelope();
    const first = applyOrderedEvent(state, event);
    expect(first.outcome).toBe('applied-first');
    // The SAME envelope (same identity), redelivered — not a new event.
    const redelivery = applyOrderedEvent(state, { ...event, delivered_at: '2026-11-12T10:00:02Z' });
    expect(redelivery.outcome).toBe('discarded-duplicate');
    const diagnostics = orderingDiagnostics(state);
    expect(diagnostics.applied).toBe(1);
    expect(diagnostics.duplicatesDiscarded).toBe(1);
    expect(diagnostics.lastDiscard?.outcome).toBe('discarded-duplicate');
  });

  it('distinct events with distinct correlation ids are NOT duplicates even at the same event_time', () => {
    const state = createOrderingState();
    const first = applyOrderedEvent(state, envelope({ event_time: '2026-11-12T10:00:00Z' }));
    const second = applyOrderedEvent(state, envelope({ event_time: '2026-11-12T10:00:00Z' }));
    expect(first.outcome).toBe('applied-first');
    expect(second.outcome).toBe('applied-equal-time');
    expect(orderingDiagnostics(state).applied).toBe(2);
  });

  it('identity is the (event_type, correlation_id, event_time) tuple mirrored from the envelope', () => {
    const base = envelope();
    expect(identityOf(base)).toBe(`${base.event_type}|${base.correlation_id}|${base.event_time}`);
    expect(identityOf(envelope({ correlation_id: 'other' }))).not.toBe(identityOf(base));
  });

  it('the dedup memory is bounded and evicts the oldest identity (recorded, not hidden)', () => {
    const state = createOrderingState();
    // The oldest event is the FIRST of the window; the rest of the window
    // is strictly newer, so the entity anchor ends past the oldest event.
    const oldest = envelope({ event_time: '2026-11-12T10:00:00Z' });
    applyOrderedEvent(state, oldest);
    for (let i = 0; i < SEEN_LIMIT; i += 1) {
      applyOrderedEvent(state, envelope({ event_time: '2026-11-12T11:00:00Z' }));
    }
    expect(state.seen.size).toBe(SEEN_LIMIT);
    // The oldest identity was evicted: its redelivery passes the dedup gate
    // (bounded memory is the recorded tradeoff) and is then discarded by the
    // entity anchor — the display is still guarded against re-application.
    const redelivery = applyOrderedEvent(state, { ...oldest, delivered_at: '2026-11-12T23:59:00Z' });
    expect(redelivery.outcome).toBe('discarded-stale');
  });
});

describe('ordering: out-of-order events never overwrite fresher displayed state', () => {
  it('a stale dispatch state arriving after a fresher one is discarded and counted', () => {
    const state = createOrderingState();
    const fresher = applyOrderedEvent(
      state,
      envelope({
        event_type: 'DISPATCH_ACKNOWLEDGED',
        event_name: 'DispatchAcknowledged',
        command_id: 'cmd-1',
        event_time: '2026-11-12T10:05:00Z',
      }),
    );
    expect(fresher.outcome).toBe('applied-first');
    const stale = applyOrderedEvent(
      state,
      envelope({
        event_type: 'DISPATCH_ISSUED',
        event_name: 'DispatchIssued',
        command_id: 'cmd-1',
        event_time: '2026-11-12T10:00:00Z',
      }),
    );
    expect(stale.outcome).toBe('discarded-stale');
    expect(stale.entityKey).toBe('command/cmd-1');
    const diagnostics = orderingDiagnostics(state);
    expect(diagnostics.staleDiscarded).toBe(1);
    expect(diagnostics.applied).toBe(1);
  });

  it('a strictly newer event applies (fresher wins) for telemetry and schedule entities', () => {
    const state = createOrderingState();
    expect(
      applyOrderedEvent(state, envelope({ event_time: '2026-11-12T10:00:00Z' })).outcome,
    ).toBe('applied-first');
    expect(
      applyOrderedEvent(state, envelope({ event_time: '2026-11-12T10:01:00Z' })).outcome,
    ).toBe('applied-newer');
    expect(
      applyOrderedEvent(
        state,
        envelope({
          event_type: 'SCHEDULE_PROPOSED',
          event_name: 'ScheduleProposed',
          portfolio_id: 'p-1',
          site_id: 'site-1',
          asset_id: 'asset-1',
          event_time: '2026-11-12T10:02:00Z',
        }),
      ).outcome,
    ).toBe('applied-newer');
    expect(orderingDiagnostics(state).applied).toBe(3);
  });

  it('entities are independent: a stale event for another command still applies', () => {
    const state = createOrderingState();
    applyOrderedEvent(
      state,
      envelope({ event_type: 'DISPATCH_ISSUED', event_name: 'DispatchIssued', command_id: 'cmd-1', event_time: '2026-11-12T10:05:00Z' }),
    );
    const other = applyOrderedEvent(
      state,
      envelope({ event_type: 'DISPATCH_ISSUED', event_name: 'DispatchIssued', command_id: 'cmd-2', event_time: '2026-11-12T10:00:00Z' }),
    );
    expect(other.outcome).toBe('applied-first');
  });

  it('the ordering rule is stated per event type and covers the CLOSED 26-tag vocabulary exactly once each', () => {
    const covered = new Set(ORDERING_RULES.flatMap((rule) => rule.tags));
    expect(covered.size).toBe(FROZEN_EVENT_TAGS.length);
    for (const tag of FROZEN_EVENT_TAGS) {
      expect(() => ruleForTag(tag)).not.toThrow();
      expect(ORDERING_RULES.filter((rule) => rule.tags.includes(tag)).length).toBe(1);
      expect(ruleForTag(tag).statement.length).toBeGreaterThan(0);
    }
  });
});

describe('contract defects are never applied (fail closed)', () => {
  it('a dispatch envelope without command_id is a contract defect', () => {
    const state = createOrderingState();
    const decision = applyOrderedEvent(
      state,
      envelope({ event_type: 'DISPATCH_ISSUED', event_name: 'DispatchIssued', command_id: undefined }),
    );
    expect(decision.outcome).toBe('discarded-contract-defect');
    expect(orderingDiagnostics(state).contractDefects).toBe(1);
  });

  it('an unparsable event_time is a contract defect', () => {
    const state = createOrderingState();
    const decision = applyOrderedEvent(state, envelope({ event_time: 'not-a-time' }));
    expect(decision.outcome).toBe('discarded-contract-defect');
    expect(orderingDiagnostics(state).contractDefects).toBe(1);
  });

  it('a tag outside the frozen vocabulary is a contract defect (the vocabulary is closed)', () => {
    const state = createOrderingState();
    const decision = applyOrderedEvent(
      state,
      envelope({ event_type: 'NOT_A_REAL_EVENT' as LiveEventEnvelope['event_type'] }),
    );
    expect(decision.outcome).toBe('discarded-contract-defect');
  });
});

describe('dedup-only streams (outside the three §15.1 live areas)', () => {
  it('a standalone fact applies on first sight; its redelivery is still a duplicate', () => {
    const state = createOrderingState();
    const event = envelope({ event_type: 'ASSET_REGISTERED', event_name: 'AssetRegistered' });
    const first = applyOrderedEvent(state, event);
    expect(first.outcome).toBe('applied-unordered-fact');
    expect(first.entityKey).toBeNull();
    const again = applyOrderedEvent(state, event);
    expect(again.outcome).toBe('discarded-duplicate');
  });
});

describe('defence-in-depth tenant drop (NOT the enforcement, §16.2)', () => {
  it('an envelope from another tenant is dropped and counted when the session tenant is supplied', () => {
    const state = createOrderingState();
    const decision = applyOrderedEvent(state, envelope({ tenant_id: '00000000-0000-0000-0000-000000000009' }), TENANT);
    expect(decision.outcome).toBe('discarded-tenant-mismatch');
    expect(orderingDiagnostics(state).tenantMismatchesDropped).toBe(1);
  });

  it('without a session tenant the gate does not drop (the server filter is the enforcement)', () => {
    const state = createOrderingState();
    const decision = applyOrderedEvent(state, envelope({ tenant_id: 'someone-else' }));
    expect(decision.outcome).toBe('applied-first');
  });
});

describe('RESYNC_REQUIRED handling (§15.4; P5.25/P5.33)', () => {
  it('markResync clears anchors and identities; the discard counters survive (no laundered history)', () => {
    const state = createOrderingState();
    applyOrderedEvent(state, envelope({ event_time: '2026-11-12T10:05:00Z' }));
    applyOrderedEvent(state, envelope({ event_time: '2026-11-12T09:00:00Z' })); // stale
    const before = orderingDiagnostics(state);
    expect(before.staleDiscarded).toBe(1);
    markResync(state);
    const after = orderingDiagnostics(state);
    expect(after.resyncs).toBe(1);
    expect(after.staleDiscarded).toBe(1);
    expect(after.applied).toBe(1);
    expect(state.seen.size).toBe(0);
    expect(state.anchors.size).toBe(0);
    // The previously-stale event re-applies after the reset: the anchor is
    // gone until the consumer re-seeds from the re-fetched records — the
    // recorded reset window, covered by the REST re-sync being the authority.
    expect(applyOrderedEvent(state, envelope({ event_time: '2026-11-12T09:00:00Z' })).outcome).toBe('applied-first');
  });

  it('seedAnchor re-seeds the baseline so a pre-gap redelivery cannot overwrite re-fetched state', () => {
    const state = createOrderingState();
    markResync(state);
    seedAnchor(state, 'site-1/asset-1', '2026-11-12T12:00:00Z');
    const preGap = applyOrderedEvent(state, envelope({ event_time: '2026-11-12T10:00:00Z' }));
    expect(preGap.outcome).toBe('discarded-stale');
    const newer = applyOrderedEvent(state, envelope({ event_time: '2026-11-12T12:01:00Z' }));
    expect(newer.outcome).toBe('applied-newer');
  });

  it('seedAnchor ignores an unparsable time and never lowers an anchor', () => {
    const state = createOrderingState();
    seedAnchor(state, 'e', '2026-11-12T12:00:00Z');
    seedAnchor(state, 'e', 'not-a-time');
    seedAnchor(state, 'e', '2026-11-12T11:00:00Z');
    expect(state.anchors.get('e')).toBe(Date.parse('2026-11-12T12:00:00Z'));
  });
});
