// P5.09 — the Audit & Reports workspace (report §20.1 item 11; §15.3, §21.3,
// §24 step 9; the P4.36 audit-events read API). Replaces the P5.01 stub.
//
// What is shown (ledger P5.09):
// - The complete audit trail via GET /api/v1/audit-events (+/{event_id}) with
//   the P4.36 filters (action, actor, object_type, object_id, inclusive time
//   window) and keyset cursor pagination. Every §10.1 field is displayed:
//   actor, action, object, decision outcome + reason reference (never a bare
//   allow/deny), evidence reference, before/after REFERENCES, the distinct
//   event vs ingest times, the correlation id, and the correction linkage
//   (corrected_audit_id — a correction is a NEW row; the original is never
//   edited).
// - The per-command decision chain (§24 step 9): the loaded records for one
//   object_id, ordered by event time, classified onto the decision-chain
//   stages ONLY where the action is a frozen §15.3 event name with an
//   unambiguous stage; anything else is shown verbatim and marked
//   unclassified — never forced into a stage.
// - Schedule vs actual: requested values from the persisted command payload;
//   measured values ONLY from the telemetry query (with the six-state
//   quality and the simulated flag shown). The console computes NO tracking
//   verdict: the P4.16 acceptance-criterion/tracking-error record has no
//   console read endpoint (recorded gap, 05_09_workspaces_b.md).
// - Dispatch failures/expiries/fallback activations, filtered from the
//   loaded records, with their evidence references.
// - Operator-action records (the frozen OperatorActionRecorded event class)
//   with actor and timestamp.
// - Export: a client-side JSON export of the CURRENTLY LOADED records that
//   preserves every field including references (tested). The server has no
//   export endpoint by design (the append-only statement travels in every
//   response and is rendered verbatim); the export states its scope honestly.
import { useEffect, useState } from 'react';
import type { JSX } from 'react';
import { api, type ApiClient } from '../api/client';
import {
  getAuditEvent,
  getDispatchCommand,
  listAuditEvents,
} from '../api/dispatch';
import { queryTelemetry } from '../api/telemetry';
import type {
  AuditEventDto,
  DataEnvelope,
  DispatchCommandDetailDto,
  PageEnvelope,
  TelemetryRecordDto,
} from '../api/types';
import {
  ControlAuthorityModeIndicator,
  EmptyState,
  ErrorState,
  FreshnessStamp,
  LoadingState,
  QualityBadge,
  SimulatedBadge,
  SIMULATED_CHAIN_STAGES_NOTE,
  VerbatimStatement,
} from '../components/honesty';
import {
  DECISION_CHAIN_STAGES,
  EVENT_STAGE_MAP,
  formatUtcMillis,
  type DecisionChainStage,
} from '../components/stateVocabulary';
import { useCan } from '../security/useCan';
import { RoleScopeLine } from '../components/workspaceData';

// --- Filter → params mapping (pure, exported for tests) ----------------------

export interface AuditFilters {
  action: string;
  actor: string;
  objectType: string;
  objectId: string;
  /** RFC 3339 / datetime-local strings; empty = unbounded. */
  fromEventTime: string;
  toEventTime: string;
}

export const EMPTY_FILTERS: AuditFilters = {
  action: '',
  actor: '',
  objectType: '',
  objectId: '',
  fromEventTime: '',
  toEventTime: '',
};

/**
 * Maps the UI filters onto the P4.36 wire params (routes_phase4.rs
 * AuditEventListParams). A time bound that does not parse is NOT sent —
 * the mapping returns it in `ignored` so the UI can say so honestly
 * instead of silently dropping or guessing.
 */
export function buildAuditListParams(filters: AuditFilters): {
  params: {
    action?: string;
    actor?: string;
    object_type?: string;
    object_id?: string;
    from_event_unix_millis?: number;
    to_event_unix_millis?: number;
  };
  ignored: string[];
} {
  const ignored: string[] = [];
  const params: ReturnType<typeof buildAuditListParams>['params'] = {};
  if (filters.action.trim() !== '') params.action = filters.action.trim();
  if (filters.actor.trim() !== '') params.actor = filters.actor.trim();
  if (filters.objectType.trim() !== '') params.object_type = filters.objectType.trim();
  if (filters.objectId.trim() !== '') params.object_id = filters.objectId.trim();
  if (filters.fromEventTime.trim() !== '') {
    const millis = Date.parse(filters.fromEventTime);
    if (Number.isFinite(millis)) params.from_event_unix_millis = millis;
    else ignored.push('from (unparseable time — not sent)');
  }
  if (filters.toEventTime.trim() !== '') {
    const millis = Date.parse(filters.toEventTime);
    if (Number.isFinite(millis)) params.to_event_unix_millis = millis;
    else ignored.push('to (unparseable time — not sent)');
  }
  return { params, ignored };
}

// --- Data loading (pure, exported for tests) ---------------------------------

export interface AuditPageData {
  records: AuditEventDto[];
  nextCursor: string | null;
  fetchedAtIso: string;
}

/** Loads one page of audit events for the given filters (optionally from a cursor). */
export async function loadAuditEvents(
  client: ApiClient,
  filters: AuditFilters,
  cursor?: string,
): Promise<AuditPageData> {
  const { params, ignored } = buildAuditListParams(filters);
  if (ignored.length > 0) {
    throw new Error(`filter(s) not sent: ${ignored.join('; ')}`);
  }
  // The wire object is built WITHOUT undefined values (exactOptionalPropertyTypes:
  // the console never assigns an explicit undefined into an optional prop). The
  // cast only restores the server DTO's optional-prop shape; every present value
  // is a defined string/number by construction.
  const wire = { ...params, limit: 200 } as Record<string, string | number>;
  if (cursor !== undefined) wire.cursor = cursor;
  const page: PageEnvelope<AuditEventDto> = await listAuditEvents(
    wire as unknown as Parameters<typeof listAuditEvents>[0],
    client,
  );
  return { records: page.data, nextCursor: page.next_cursor, fetchedAtIso: new Date().toISOString() };
}

export interface AuditDetailData {
  record: AuditEventDto;
  fetchedAtIso: string;
}

/** Loads one audit record with every reference preserved. */
export async function loadAuditEvent(client: ApiClient, auditId: string): Promise<AuditDetailData> {
  const envelope: DataEnvelope<AuditEventDto> = await getAuditEvent(auditId, client);
  return { record: envelope.data, fetchedAtIso: new Date().toISOString() };
}

export interface ScheduleVsActualData {
  detail: DispatchCommandDetailDto;
  telemetry: TelemetryRecordDto[];
  fetchedAtIso: string;
}

/**
 * Loads the command (requested values come from its persisted payload) and
 * the telemetry window the operator specified (measured values come ONLY
 * from telemetry — never from the command, §20.2).
 */
export async function loadScheduleVsActual(
  client: ApiClient,
  commandId: string,
  telemetryFrom: string,
  telemetryUntil: string,
): Promise<ScheduleVsActualData> {
  if (telemetryFrom.trim() === '' || telemetryUntil.trim() === '') {
    throw new Error('a telemetry window (from and until) is required before querying measured values');
  }
  const detailEnvelope: DataEnvelope<DispatchCommandDetailDto> = await getDispatchCommand(
    commandId,
    client,
  );
  const telemetryPage: PageEnvelope<TelemetryRecordDto> = await queryTelemetry(
    { assetId: detailEnvelope.data.command.asset_id, from: telemetryFrom, until: telemetryUntil },
    client,
  );
  return {
    detail: detailEnvelope.data,
    telemetry: telemetryPage.data,
    fetchedAtIso: new Date().toISOString(),
  };
}

// --- Chain mapping + export (pure, exported for tests) ------------------------

export interface ChainStageEntry {
  auditId: string;
  action: string;
  actor: string;
  eventTimeUnixMillis: number;
  evidence: string | null;
  decisionOutcome: string | null;
  decisionReasonReference: string | null;
}

export interface DecisionChain {
  stages: Array<{ stage: DecisionChainStage; entries: ChainStageEntry[] }>;
  unclassified: ChainStageEntry[];
}

/**
 * Classifies the loaded records onto the decision-chain stages. Only frozen
 * §15.3 event names with an unambiguous stage map; every other action is
 * listed as unclassified (displayed verbatim, never forced into a stage).
 */
export function mapAuditEventsToChain(records: AuditEventDto[]): DecisionChain {
  const toEntry = (record: AuditEventDto): ChainStageEntry => ({
    auditId: record.audit_id,
    action: record.action,
    actor: record.actor,
    eventTimeUnixMillis: record.event_time_unix_millis,
    evidence: record.evidence ?? null,
    decisionOutcome: record.decision_outcome ?? null,
    decisionReasonReference: record.decision_reason_reference ?? null,
  });
  const sorted = [...records].sort((a, b) => a.event_time_unix_millis - b.event_time_unix_millis);
  const stages = DECISION_CHAIN_STAGES.map((stage) => ({ stage, entries: [] as ChainStageEntry[] }));
  const unclassified: ChainStageEntry[] = [];
  for (const record of sorted) {
    const entry = toEntry(record);
    const stage = EVENT_STAGE_MAP[record.action as keyof typeof EVENT_STAGE_MAP];
    if (stage) {
      stages.find((s) => s.stage === stage)?.entries.push(entry);
    } else {
      unclassified.push(entry);
    }
  }
  return { stages, unclassified };
}

export interface AuditExport {
  export_kind: 'client-side-audit-trail-json';
  exported_at: string;
  scope_note: string;
  record_count: number;
  records: AuditEventDto[];
}

/**
 * Builds the audit-trail export: the SAME records verbatim — every §10.1
 * field including the decision/reason references, before/after references,
 * and correction linkage survives (tested). No summary that drops
 * references (P5.09 step 4).
 */
export function buildAuditExport(records: AuditEventDto[], exportedAtIso: string): AuditExport {
  return {
    export_kind: 'client-side-audit-trail-json',
    exported_at: exportedAtIso,
    scope_note:
      'Client-side export of the records currently loaded in the Audit & Reports workspace (the filters/pagination scope in effect at export time). The server exposes no export endpoint by design (append-only trail, P1.10/P4.36); this file preserves every served field verbatim, including all references. It is a copy of the loaded page(s), not the complete trail beyond that scope.',
    record_count: records.length,
    records,
  };
}

// --- Record display (shared by trail/chain/failures/operator actions) ---------

export function AuditRecordCard({ record }: { record: AuditEventDto }): JSX.Element {
  return (
    <li data-testid="audit-record" data-audit-id={record.audit_id}>
      <strong>{record.action}</strong> on {record.object_type} {record.object_id} | actor{' '}
      {record.actor}
      <br />
      audit_id {record.audit_id} | tenant {record.tenant_id ?? '(absent — the read port never serves platform-level rows)'}
      <br />
      event time {formatUtcMillis(record.event_time_unix_millis)} | ingest time{' '}
      {formatUtcMillis(record.ingest_time_unix_millis)} (distinct P0.09 stamps) | correlation id{' '}
      {record.correlation_id}
      <br />
      decision outcome:{' '}
      {record.decision_outcome ? (
        <>
          <strong>{record.decision_outcome}</strong> with reason reference{' '}
          <em>{record.decision_reason_reference ?? '(MISSING — the record would be defective: the decision and its reason reference are recorded together, P1.11)'}</em>
        </>
      ) : (
        'not recorded on this row'
      )}
      <br />
      evidence reference: {record.evidence ?? '(none on this row)'} | before ref:{' '}
      {record.before_ref ?? '(none)'} | after ref: {record.after_ref ?? '(none)'}
      <br />
      correction linkage:{' '}
      {record.corrected_audit_id ? (
        <>
          this row CORRECTS audit record <strong>{record.corrected_audit_id}</strong> (a
          correction is a NEW row; the original is never edited or removed, P1.10)
        </>
      ) : (
        'not a correction row'
      )}
    </li>
  );
}

// --- The workspace container ---------------------------------------------------

type LoadStatus = 'idle' | 'loading' | 'loaded' | 'error';

interface AuditState {
  filters: AuditFilters;
  trail: AuditPageData | null;
  trailStatus: LoadStatus;
  trailError?: unknown;
  accumulated: AuditEventDto[];
  selectedAuditId: string | undefined;
  detail: AuditDetailData | null;
  detailStatus: LoadStatus;
  detailError?: unknown;
  chainCommandId: string;
  chain: DecisionChain | null;
  svaCommandId: string;
  svaFrom: string;
  svaUntil: string;
  sva: ScheduleVsActualData | null;
  svaStatus: LoadStatus;
  svaError?: unknown;
}

export function AuditReportsWorkspace({
  client,
}: {
  client?: ApiClient | undefined;
}): JSX.Element {
  const apiClient = client ?? api;
  // P5.26 — the export affordance is capability-gated over the recorded
  // grant mirror (audit.event.read: every role at tenant scope). The gate is
  // UX only — the API enforces the read independently.
  const exportCapability = useCan('audit.event.read');
  const [state, setState] = useState<AuditState>({
    filters: EMPTY_FILTERS,
    trail: null,
    trailStatus: 'idle',
    accumulated: [],
    selectedAuditId: undefined,
    detail: null,
    detailStatus: 'idle',
    chainCommandId: '',
    chain: null,
    svaCommandId: '',
    svaFrom: '',
    svaUntil: '',
    sva: null,
    svaStatus: 'idle',
  });

  async function runQuery(append: boolean): Promise<void> {
    setState((prev) => ({ ...prev, trailStatus: 'loading', trailError: undefined }));
    try {
      const cursor = append ? (state.trail?.nextCursor ?? undefined) : undefined;
      const page = await loadAuditEvents(apiClient, state.filters, cursor);
      setState((prev) => ({
        ...prev,
        trail: page,
        trailStatus: 'loaded',
        accumulated: append ? [...prev.accumulated, ...page.records] : page.records,
      }));
    } catch (error) {
      setState((prev) => ({ ...prev, trailStatus: 'error', trailError: error }));
    }
  }

  useEffect(() => {
    if (state.selectedAuditId === undefined) return;
    let cancelled = false;
    setState((prev) => ({ ...prev, detailStatus: 'loading', detail: null }));
    async function run(): Promise<void> {
      try {
        const detail = await loadAuditEvent(apiClient, state.selectedAuditId as string);
        if (!cancelled) setState((prev) => ({ ...prev, detail, detailStatus: 'loaded' }));
      } catch (error) {
        if (!cancelled) setState((prev) => ({ ...prev, detailStatus: 'error', detailError: error }));
      }
    }
    void run();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state.selectedAuditId]);

  async function runScheduleVsActual(): Promise<void> {
    setState((prev) => ({ ...prev, svaStatus: 'loading', svaError: undefined }));
    try {
      const data = await loadScheduleVsActual(apiClient, state.svaCommandId, state.svaFrom, state.svaUntil);
      setState((prev) => ({ ...prev, sva: data, svaStatus: 'loaded' }));
    } catch (error) {
      setState((prev) => ({ ...prev, svaStatus: 'error', svaError: error }));
    }
  }

  function buildChainForCommand(): void {
    const commandId = state.chainCommandId.trim();
    const records = state.accumulated.filter((record) => record.object_id === commandId);
    setState((prev) => ({ ...prev, chain: mapAuditEventsToChain(records) }));
  }

  function downloadExport(): void {
    const exportData = buildAuditExport(state.accumulated, new Date().toISOString());
    const blob = new Blob([JSON.stringify(exportData, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = `flexcore-audit-export-${Date.now()}.json`;
    anchor.click();
    URL.revokeObjectURL(url);
  }

  const failureActions = ['DispatchFailed', 'DispatchExpired', 'DispatchFallbackActivated'];
  const failureRecords = state.accumulated.filter((record) => failureActions.includes(record.action));
  const operatorActionRecords = state.accumulated.filter((record) => record.action === 'OperatorActionRecorded');
  const hasAppendOnlyStatement = state.accumulated.length > 0 ? state.accumulated[0].append_only_statement : null;

  return (
    <section aria-label="Audit & Reports">
      <h2>Audit &amp; Reports</h2>
      {/* P5.13 step 1 — the persistent §21.4 control-authority-mode indicator
          (one component, shared by every workspace; see honesty.tsx). */}
      <ControlAuthorityModeIndicator />
      <p>
        The decision evidence and operator-action trail (§20.1 item 11, §24 step 9) over
        the P4.36 audit-events read API. The trail is append-only: corrections are NEW
        rows referencing the original; nothing is edited or removed.
      </p>
      {/* P5.12 (extends the P5.03 role-line pattern to wave-2b workspaces):
          the claims-derived role line, so every workspace states the session
          role the visibility is enforced against. */}
      <RoleScopeLine />
      <fieldset data-testid="audit-filters">
        <legend>Filters (P4.36 wire params; empty = unbounded)</legend>
        {(
          [
            ['action', 'Action'],
            ['actor', 'Actor'],
            ['objectType', 'Object type'],
            ['objectId', 'Object id'],
            ['fromEventTime', 'From (event time)'],
            ['toEventTime', 'To (event time)'],
          ] as const
        ).map(([key, label]) => (
          <label key={key}>
            {label}:{' '}
            <input
              data-testid={`filter-${key}`}
              value={state.filters[key]}
              onChange={(event) =>
                setState((prev) => ({ ...prev, filters: { ...prev.filters, [key]: event.target.value } }))
              }
            />
          </label>
        ))}
        <button
          type="button"
          data-testid="audit-query"
          onClick={() => void runQuery(false)}
        >
          Query trail
        </button>
      </fieldset>
      {state.trailStatus === 'loading' ? <LoadingState label="audit events" /> : null}
      {state.trailStatus === 'error' ? <ErrorState error={state.trailError} /> : null}
      {state.trailStatus === 'loaded' && state.trail ? (
        <>
          <FreshnessStamp fetchedAtIso={state.trail.fetchedAtIso} />
          <p>
            Loaded {state.trail.records.length} record(s) on this page;{' '}
            {state.accumulated.length} accumulated across followed pages.
          </p>
          {state.trail.nextCursor ? (
            <button type="button" data-testid="audit-next-page" onClick={() => void runQuery(true)}>
              Load next page (keyset cursor)
            </button>
          ) : (
            <p>No further page (next_cursor absent on the last page).</p>
          )}
          {hasAppendOnlyStatement ? (
            <VerbatimStatement label="append-only trail" statement={hasAppendOnlyStatement} />
          ) : null}
          {state.accumulated.length === 0 ? (
            <EmptyState
              title="No audit records"
              explanation="GET /api/v1/audit-events returned no records for these filters and this tenant. The console fabricates no audit row."
            />
          ) : (
            <ol data-testid="audit-trail">
              {state.accumulated.map((record) => (
                <li key={record.audit_id}>
                  <label>
                    <input
                      type="radio"
                      name="selected-audit"
                      checked={state.selectedAuditId === record.audit_id}
                      onChange={() =>
                        setState((prev) => ({ ...prev, selectedAuditId: record.audit_id }))
                      }
                    />{' '}
                    inspect
                  </label>
                  <AuditRecordCard record={record} />
                </li>
              ))}
            </ol>
          )}
          {exportCapability.allowed ? (
            <button type="button" data-testid="audit-export" onClick={downloadExport}>
              Export loaded records (JSON, references preserved)
            </button>
          ) : (
            <div role="alert" data-testid="export-capability-denial">
              <p>
                <strong>
                  Export not available: audit.event.read is denied for this
                  session.
                </strong>
              </p>
              <p>{exportCapability.reason}</p>
              <p role="note">
                The console hides the action with this explanation instead of
                rendering a broken button; the API enforces authorization
                independently (P4.29 invariant-7). No audit.event.export action
                exists on the closed surface by design (append-only trail) — the
                export is a CLIENT-SIDE copy of the loaded records, and a role
                that may not read the trail may not copy it either.
              </p>
            </div>
          )}
          <p>
            The export is CLIENT-SIDE over the currently loaded records only (the
            server has no export endpoint by design — see the append-only statement
            above). It preserves every served field verbatim; it is a copy of the
            loaded scope, never a summary that drops references.
          </p>

          <h3>Decision chain per command (§24 step 9)</h3>
          {/* P5.13 — the unified simulated qualification for the
              issue/acknowledgement/tracking stages (one constant from the
              honesty module, not a per-surface copy). */}
          <SimulatedBadge detail={SIMULATED_CHAIN_STAGES_NOTE} />
          <label>
            Command id (object_id):{' '}
            <input
              data-testid="chain-command-id"
              value={state.chainCommandId}
              onChange={(event) => setState((prev) => ({ ...prev, chainCommandId: event.target.value }))}
            />
          </label>
          <button type="button" data-testid="chain-build" onClick={buildChainForCommand}>
            Build chain from loaded records
          </button>
          {state.chain ? (
            <div data-testid="decision-chain">
              {state.chain.stages.map((stageBlock) => (
                <div key={stageBlock.stage}>
                  <h4>{stageBlock.stage}</h4>
                  {stageBlock.entries.length === 0 ? (
                    <p>
                      No loaded record classified to this stage (the audit action
                      vocabulary on this surface may differ — use the filters; nothing
                      is inferred into a stage).
                    </p>
                  ) : (
                    <ul>
                      {stageBlock.entries.map((entry) => (
                        <li key={entry.auditId}>
                          {entry.action} | actor {entry.actor} | at{' '}
                          {formatUtcMillis(entry.eventTimeUnixMillis)} | evidence{' '}
                          {entry.evidence ?? '(none)'} | decision {entry.decisionOutcome ?? '(none)'}
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              ))}
              <h4>Unclassified actions (displayed verbatim)</h4>
              {state.chain.unclassified.length === 0 ? (
                <p>None — every loaded record matched a frozen §15.3 event name.</p>
              ) : (
                <ul>
                  {state.chain.unclassified.map((entry) => (
                    <li key={entry.auditId}>
                      {entry.action} | actor {entry.actor} | at{' '}
                      {formatUtcMillis(entry.eventTimeUnixMillis)} (outside the stage map —
                      not forced into a chain stage)
                    </li>
                  ))}
                </ul>
              )}
            </div>
          ) : null}

          <h3>Dispatch failures, expiries, and fallback activations (P4.17)</h3>
          {failureRecords.length === 0 ? (
            <EmptyState
              title="No failure-class records in the loaded scope"
              explanation="No DispatchFailed / DispatchExpired / DispatchFallbackActivated record is present among the loaded pages. The console does not scan beyond the loaded scope and invents no failure."
            />
          ) : (
            <ul data-testid="failure-records">
              {failureRecords.map((record) => (
                <li key={record.audit_id}>
                  <AuditRecordCard record={record} />
                </li>
              ))}
            </ul>
          )}

          <h3>Operator-action records (§15.3 OperatorActionRecorded)</h3>
          {operatorActionRecords.length === 0 ? (
            <EmptyState
              title="No OperatorActionRecorded rows in the loaded scope"
              explanation="No record with the frozen OperatorActionRecorded event class is present among the loaded pages. Operator actions recorded under other discriminators appear in the trail view verbatim."
            />
          ) : (
            <ul data-testid="operator-actions">
              {operatorActionRecords.map((record) => (
                <li key={record.audit_id}>
                  actor {record.actor} | at {formatUtcMillis(record.event_time_unix_millis)} |
                  object {record.object_type} {record.object_id} | evidence{' '}
                  {record.evidence ?? '(none)'} | audit_id {record.audit_id}
                </li>
              ))}
            </ul>
          )}
        </>
      ) : null}
      {state.selectedAuditId !== undefined && state.detailStatus === 'loading' ? (
        <LoadingState label="the audit record detail" />
      ) : null}
      {state.detailStatus === 'error' ? <ErrorState error={state.detailError} /> : null}
      {state.detailStatus === 'loaded' && state.detail ? (
        <div data-testid="audit-detail">
          <h3>Audit record {state.detail.record.audit_id}</h3>
          <AuditRecordCard record={state.detail.record} />
        </div>
      ) : null}

      <h3>Schedule vs actual (requested vs measured; measured from telemetry ONLY)</h3>
      <label>
        Command id:{' '}
        <input
          data-testid="sva-command-id"
          value={state.svaCommandId}
          onChange={(event) => setState((prev) => ({ ...prev, svaCommandId: event.target.value }))}
        />
      </label>
      <label>
        Telemetry window from:{' '}
        <input
          data-testid="sva-from"
          value={state.svaFrom}
          placeholder="2026-01-01T00:00:00Z"
          onChange={(event) => setState((prev) => ({ ...prev, svaFrom: event.target.value }))}
        />
      </label>
      <label>
        until:{' '}
        <input
          data-testid="sva-until"
          value={state.svaUntil}
          placeholder="2026-01-02T00:00:00Z"
          onChange={(event) => setState((prev) => ({ ...prev, svaUntil: event.target.value }))}
        />
      </label>
      <button type="button" data-testid="sva-run" onClick={() => void runScheduleVsActual()}>
        Load requested vs measured
      </button>
      {state.svaStatus === 'loading' ? <LoadingState label="the command and telemetry window" /> : null}
      {state.svaStatus === 'error' ? <ErrorState error={state.svaError} /> : null}
      {state.svaStatus === 'loaded' && state.sva ? (
        <div data-testid="schedule-vs-actual">
          <FreshnessStamp fetchedAtIso={state.sva.fetchedAtIso} />
          <h4>Requested (from the persisted command payload — verbatim JSON)</h4>
          <pre data-testid="sva-requested">
            {JSON.stringify(state.sva.detail.command.command_payload, null, 2)}
          </pre>
          <h4>Measured (telemetry query only — quality + simulated flag shown per value)</h4>
          {state.sva.telemetry.length === 0 ? (
            <EmptyState
              title="No telemetry records in the window"
              explanation="GET /api/v1/telemetry/query returned no records for this asset and window. Measured values come only from telemetry; none is substituted from the command."
            />
          ) : (
            <ul data-testid="sva-measured">
              {state.sva.telemetry.map((record, index) => (
                <li key={index}>
                  {record.quantity} = {record.value_num ?? record.value_bool ?? record.value_text ?? '(no value)'}{' '}
                  {record.unit} | <QualityBadge quality={record.quality} /> | event{' '}
                  {record.event_time} |{' '}
                  {record.simulated ? 'SIMULATED (simulator-derived telemetry, P2.12)' : 'simulated flag absent on the record'}
                </li>
              ))}
            </ul>
          )}
          <p role="note">
            The console computes NO tracking verdict here: the P4.16
            acceptance-criterion / tracking-error record has no console read endpoint
            (recorded gap, 05_09_workspaces_b.md). This view juxtaposes the requested
            payload with telemetry-measured values only — measured values never come
            from the command itself (§20.2), and no agreement conclusion is drawn.
          </p>
        </div>
      ) : null}
    </section>
  );
}
