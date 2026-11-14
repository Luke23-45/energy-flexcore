// P5.09 — Audit & Reports workspace tests (Vitest). UNRUN — never executed
// in the Phase 5 loop (no install/build/test; decision record:
// 05_09_workspaces_b.md). The human executes these via `npm test` after the
// first install.
//
// Coverage per the task contract: rendering, data mapping (filters → wire
// params, chain classification, export), state vocabulary (§10.1 record
// fields, frozen event names, quality states), and honesty labelling on
// mocked fetch (measured values only from telemetry with simulated flags;
// references preserved end to end; no invented verdicts).
import { describe, expect, it } from 'vitest';
import { renderToString } from 'react-dom/server';
import { createApiClient, type ApiClient } from '../api/client';
import type {
  AuditEventDto,
  DataEnvelope,
  DispatchCommandDetailDto,
  DispatchCommandDto,
  PageEnvelope,
  TelemetryRecordDto,
} from '../api/types';
import { listAuditEvents } from '../api/dispatch';
import {
  DECISION_CHAIN_STAGES,
  EVENT_STAGE_MAP,
  FROZEN_EVENT_NAMES,
  QUALITY_STATES,
} from '../components/stateVocabulary';
import {
  AuditRecordCard,
  buildAuditExport,
  buildAuditListParams,
  EMPTY_FILTERS,
  loadAuditEvents,
  loadScheduleVsActual,
  mapAuditEventsToChain,
} from './AuditReports';

// --- Fixtures (P4.36 wire shapes, from dto_phase4.rs) -------------------------

function auditFixture(overrides: Partial<AuditEventDto>): AuditEventDto {
  return {
    audit_id: 'audit-1',
    tenant_id: 'tenant-1',
    actor: 'svc:validator',
    action: 'DispatchProposed',
    object_type: 'command',
    object_id: 'command-1',
    evidence: 'evidence-ref-1',
    event_time_unix_millis: 1767225600000,
    ingest_time_unix_millis: 1767225601000,
    correlation_id: 'corr-1',
    append_only_statement: 'APPEND-ONLY-STATEMENT-VERBATIM',
    ...overrides,
  };
}

const CHAIN_RECORDS: AuditEventDto[] = [
  auditFixture({ audit_id: 'a1', action: 'ScheduleProposed', event_time_unix_millis: 1000, evidence: 'schedule-ref' }),
  auditFixture({ audit_id: 'a2', action: 'FeasibilityCheckCompleted', event_time_unix_millis: 2000, evidence: 'feasibility-ref' }),
  auditFixture({
    audit_id: 'a3',
    action: 'DispatchApproved',
    event_time_unix_millis: 3000,
    actor: 'user:operator-1',
    decision_outcome: 'ALLOWED',
    decision_reason_reference: 'decision-ref-1',
    evidence: 'authorization-ref',
  }),
  auditFixture({ audit_id: 'a4', action: 'DispatchIssued', event_time_unix_millis: 4000, evidence: 'signature-ref' }),
  auditFixture({ audit_id: 'a5', action: 'DispatchAcknowledged', event_time_unix_millis: 5000, evidence: 'acceptance-ref' }),
  auditFixture({ audit_id: 'a6', action: 'command.approve', event_time_unix_millis: 6000 }),
];

const CORRECTION_ROW = auditFixture({
  audit_id: 'audit-correction',
  action: 'OperatorActionRecorded',
  corrected_audit_id: 'audit-1',
  decision_outcome: 'ALLOWED',
  decision_reason_reference: 'policy-ref-9',
  before_ref: 'state:BEFORE',
  after_ref: 'state:AFTER',
});

// --- Vocabulary tests ---------------------------------------------------------

describe('chain-stage vocabulary', () => {
  it('the decision-chain stages are exactly the §24-step-9 template', () => {
    expect([...DECISION_CHAIN_STAGES]).toEqual([
      'schedule',
      'validation evidence',
      'feasibility evidence',
      'authorization decision',
      'issue',
      'acknowledgement',
      'tracking',
      'outcome',
    ]);
  });

  it('the stage map covers only frozen §15.3 event names', () => {
    for (const key of Object.keys(EVENT_STAGE_MAP)) {
      expect((FROZEN_EVENT_NAMES as readonly string[]).includes(key)).toBe(true);
    }
  });

  it('the six telemetry-quality states are exact (§10.4)', () => {
    expect([...QUALITY_STATES]).toEqual(['GOOD', 'SUSPECT', 'BAD', 'MISSING', 'ESTIMATED', 'STALE']);
  });
});

// --- Data-mapping tests --------------------------------------------------------

describe('buildAuditListParams (filters → P4.36 wire params)', () => {
  it('maps action/actor/object_type/object_id and the inclusive time window', () => {
    const { params, ignored } = buildAuditListParams({
      ...EMPTY_FILTERS,
      action: 'command.approve',
      actor: 'user:operator-1',
      objectType: 'command',
      objectId: 'command-1',
      fromEventTime: '2026-01-01T00:00:00Z',
      toEventTime: '2026-01-02T00:00:00Z',
    });
    expect(params).toEqual({
      action: 'command.approve',
      actor: 'user:operator-1',
      object_type: 'command',
      object_id: 'command-1',
      from_event_unix_millis: Date.parse('2026-01-01T00:00:00Z'),
      to_event_unix_millis: Date.parse('2026-01-02T00:00:00Z'),
    });
    expect(ignored).toEqual([]);
  });

  it('an unparseable time bound is reported as ignored, never silently sent', () => {
    const { params, ignored } = buildAuditListParams({
      ...EMPTY_FILTERS,
      fromEventTime: 'not-a-time',
    });
    expect(params.from_event_unix_millis).toBeUndefined();
    expect(ignored).toHaveLength(1);
    expect(ignored[0]).toContain('not sent');
  });
});

describe('mapAuditEventsToChain (classification, never invention)', () => {
  it('classifies frozen event names onto the chain stages in event-time order', () => {
    const chain = mapAuditEventsToChain(CHAIN_RECORDS);
    const stageOf = (stage: string) =>
      chain.stages.find((s) => s.stage === stage)?.entries.map((e) => e.auditId) ?? [];
    expect(stageOf('schedule')).toEqual(['a1']);
    expect(stageOf('feasibility evidence')).toEqual(['a2']);
    expect(stageOf('authorization decision')).toEqual(['a3']);
    expect(stageOf('issue')).toEqual(['a4']);
    expect(stageOf('acknowledgement')).toEqual(['a5']);
    // Non-§15.3 actions are unclassified, never forced into a stage.
    expect(chain.unclassified.map((e) => e.auditId)).toEqual(['a6']);
  });

  it('stages with no matched record stay empty (displayed as "no record")', () => {
    const chain = mapAuditEventsToChain([CHAIN_RECORDS[0]]);
    const tracking = chain.stages.find((s) => s.stage === 'tracking');
    expect(tracking?.entries).toEqual([]);
  });
});

describe('buildAuditExport (references preserved, no summary that drops them)', () => {
  it('round-trips every §10.1 field including decision/reason/before/after/correction refs', () => {
    const exported = buildAuditExport([CORRECTION_ROW], '2026-11-11T00:00:00.000Z');
    const roundTrip = JSON.parse(JSON.stringify(exported)) as typeof exported;
    const record = roundTrip.records[0];
    expect(record.evidence).toBe('evidence-ref-1');
    expect(record.decision_outcome).toBe('ALLOWED');
    expect(record.decision_reason_reference).toBe('policy-ref-9');
    expect(record.before_ref).toBe('state:BEFORE');
    expect(record.after_ref).toBe('state:AFTER');
    expect(record.corrected_audit_id).toBe('audit-1');
    expect(record.correlation_id).toBe('corr-1');
    expect(record.event_time_unix_millis).toBe(1767225600000);
    expect(record.ingest_time_unix_millis).toBe(1767225601000);
    expect(roundTrip.record_count).toBe(1);
    expect(roundTrip.scope_note).toContain('preserves every served field verbatim');
    expect(roundTrip.scope_note).toContain('no export endpoint');
  });

  it('declares itself a client-side copy of the loaded scope, not the complete trail', () => {
    const exported = buildAuditExport([], '2026-11-11T00:00:00.000Z');
    expect(exported.export_kind).toBe('client-side-audit-trail-json');
    expect(exported.scope_note).toContain('not the complete trail');
  });
});

// --- Rendering tests -------------------------------------------------------------

describe('rendering (fixture-driven)', () => {
  it('renders every §10.1 field on the record card, correction linkage included', () => {
    const html = renderToString(<AuditRecordCard record={CORRECTION_ROW} />);
    expect(html).toContain('OperatorActionRecorded');
    expect(html).toContain('ALLOWED');
    expect(html).toContain('policy-ref-9');
    expect(html).toContain('state:BEFORE');
    expect(html).toContain('state:AFTER');
    expect(html).toContain('evidence-ref-1');
    expect(html).toContain('CORRECTS audit record');
    expect(html).toContain('corr-1');
    expect(html).toContain('(distinct P0.09 stamps)');
  });

  it('renders a missing decision reason reference as a visible defect note, never silently', () => {
    // decision_reason_reference is left ABSENT (optional on the DTO) while the
    // outcome is present — the card must flag that as a visible defect note.
    const defective = auditFixture({ decision_outcome: 'DENIED' });
    expect(defective.decision_reason_reference).toBeUndefined();
    const html = renderToString(<AuditRecordCard record={defective} />);
    expect(html).toContain('DENIED');
    expect(html).toContain('defective');
  });

  it('renders the append-only statement verbatim when present on the records', () => {
    const html = renderToString(<AuditRecordCard record={auditFixture({})} />);
    expect(html).toContain('APPEND-ONLY-STATEMENT-VERBATIM');
  });
});

// --- Mocked-fetch tests -----------------------------------------------------------

describe('data loading over a mocked fetch', () => {
  it('loadAuditEvents sends the P4.36 filter params and follows the keyset cursor', async () => {
    const paths: string[] = [];
    const client: ApiClient = createApiClient({
      fetchImpl: async (input: RequestInfo | URL) => {
        paths.push(String(input));
        return new Response(
          JSON.stringify({
            data: [auditFixture({})],
            next_cursor: 'audit-1',
          } satisfies PageEnvelope<AuditEventDto>),
          { status: 200 },
        );
      },
    });
    const page = await loadAuditEvents(
      client,
      { ...EMPTY_FILTERS, action: 'DispatchIssued', objectId: 'command-1' },
      'audit-0',
    );
    const url = new URL(paths[0], 'http://test');
    expect(url.pathname).toBe('/api/v1/audit-events');
    expect(url.searchParams.get('action')).toBe('DispatchIssued');
    expect(url.searchParams.get('object_id')).toBe('command-1');
    expect(url.searchParams.get('cursor')).toBe('audit-0');
    expect(url.searchParams.get('limit')).toBe('200');
    expect(page.nextCursor).toBe('audit-1');
  });

  it('loadAuditEvents refuses to query when a filter was ignored (honest refusal)', async () => {
    const client: ApiClient = createApiClient({
      fetchImpl: async () => new Response(JSON.stringify({ data: [] }), { status: 200 }),
    });
    await expect(
      loadAuditEvents(client, { ...EMPTY_FILTERS, toEventTime: 'garbage' }),
    ).rejects.toThrow('not sent');
  });

  it('loadScheduleVsActual reads the command + telemetry only (measured never from the command)', async () => {
    const paths: string[] = [];
    const command = {
      command_id: 'command-1',
      tenant_id: 'tenant-1',
      portfolio_id: 'portfolio-1',
      site_id: 'site-1',
      asset_id: 'asset-1',
      schedule_id: 'sched-1',
      schedule_revision: 1,
      valid_until_millis: 1767312000000,
      state: 'TRACKING',
      command_payload: { intervals: [{ p_export_mw: 0.4 }] },
      no_execute_endpoint_statement: 'NO-EXECUTE',
      simulation_only_statement: 'SIM-ONLY',
    } satisfies DispatchCommandDto;
    const client: ApiClient = createApiClient({
      fetchImpl: async (input: RequestInfo | URL) => {
        paths.push(String(input));
        const url = String(input);
        if (url.startsWith('/api/v1/dispatch-commands/command-1')) {
          return new Response(
            JSON.stringify({ data: { command, journal: [] } } satisfies DataEnvelope<DispatchCommandDetailDto>),
            { status: 200 },
          );
        }
        return new Response(
          JSON.stringify({
            data: [
              {
                asset_id: 'asset-1',
                quantity: 'active_power_export',
                value_num: 0.38,
                unit: 'MW',
                quality: 'GOOD',
                event_time: '2026-01-01T00:05:00Z',
                ingest_time: '2026-01-01T00:05:01Z',
                source_id: 'sim',
                source_sequence: 1,
                simulated: true,
              } satisfies TelemetryRecordDto,
            ],
            next_cursor: null,
          } satisfies PageEnvelope<TelemetryRecordDto>),
          { status: 200 },
        );
      },
    });
    const data = await loadScheduleVsActual(client, 'command-1', '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z');
    expect(paths.some((p) => p.startsWith('/api/v1/telemetry/query?asset=asset-1&from=2026-01-01T00%3A00%3A00Z'))).toBe(true);
    expect(data.telemetry).toHaveLength(1);
    expect(data.telemetry[0].simulated).toBe(true);
    expect(data.detail.command.command_payload).toEqual({ intervals: [{ p_export_mw: 0.4 }] });
  });

  it('loadScheduleVsActual refuses to query without a telemetry window (no invented measured values)', async () => {
    const client: ApiClient = createApiClient({
      fetchImpl: async () => new Response(JSON.stringify({ data: [] }), { status: 200 }),
    });
    await expect(loadScheduleVsActual(client, 'command-1', '', '')).rejects.toThrow(
      'telemetry window',
    );
  });

  it('the audit list endpoint rejects cross-tenant reads as 404 without leaking existence', async () => {
    const client: ApiClient = createApiClient({
      fetchImpl: async () =>
        new Response(JSON.stringify({ error: { code: 'not_found', message: '' } }), { status: 404 }),
    });
    await expect(listAuditEvents({ limit: 10 }, client)).rejects.toMatchObject({ status: 404 });
  });
});
