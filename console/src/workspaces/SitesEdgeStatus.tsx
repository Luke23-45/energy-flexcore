// P6.25 — the Sites-edge-status panel (the Sites workspace extension; the
// `Sites.tsx` component patterns, NEW files over rewrites — see
// ledger/phase-06-edge-protocol/25_31_32_console_clock_decommission.md §1).
//
// WHAT AN OPERATOR CAN DETERMINE FROM THIS PANEL WITHOUT READING LOGS
// (the ledger P6.25 acceptance criterion):
// - edge connectivity (last-seen + the verbatim source note — a missing
//   report renders the gap, never a plausible "connected"; the risk row's
//   "green indicator that hides a stale feed" is structurally impossible
//   here because freshness is displayed PROMINENTLY on the same panel),
// - certificate expiry with the P5 rotation-window treatment
//   (api/edgeStatus.ts certificateExpiryTreatment: expired / rotation-due
//   / valid / not-reported — text-borne, colour-independent),
// - adapter health as the P6.17 closed vocabulary VERBATIM (the edge's
//   HealthSnapshot JSON is rendered as-is: HEALTHY / DEGRADED_<reason> /
//   FAILING_<reason> — never prettified into healthy-looking synonyms),
// - last-seen timestamps, buffer depth, and data freshness (the freshness
//   line reuses components/ValueType — no new visual language; the value
//   types measured/estimated/stale/simulated are decided by the shared
//   valueTypeFor treatment record in the persistent shell legend).
//
// THE COMMAND-PATH GUARANTEE (P6.25 step 3, recorded): the display shows
// disabled / test-env-enabled WITH expiry so an operator can NEVER mistake
// a proposed setpoint for an executed command, and this console NEVER
// enables the command path: there is no endpoint call that could (the
// server has no such endpoint — routes_edge.rs / openapi.rs), and the only
// affordance is "request signed per-site configuration change", which
// records a LOCAL request event (sitesEdgeStatus.ts, the denialLog
// pattern) and states it takes effect ONLY through the P6.14 procedure.
//
// CONFIRMATION (P6.25 step 4): the request affordance requires an explicit
// confirmation control following the ConfirmHighImpact pattern exactly
// (labelled required checkbox inside a <label>, aria-required, inline with
// the parameters, keyboard-operable, not colour- or hover-dependent). The
// recorded HIGH_IMPACT_ACTIONS list (components/ConfirmHighImpact.tsx) is
// a closed Phase-5 vocabulary this wave does not own, so the confirmation
// is implemented HERE following that pattern instead of passing an
// unlisted actionId into the shared component (which would render its
// unknown-action alert — recorded in the decision document).
import { useState } from 'react';
import type { JSX } from 'react';
import { HonestEmpty } from '../components/workspaceData';
import { FreshnessLine } from '../components/ValueType';
import { formatUnixMillis, NOT_PRESENT } from '../components/format';
import {
  COMMAND_PATH_REQUEST_NOTE,
  recordCommandPathEnablementRequest,
  type EdgeStatusData,
  type EdgeStatusRow,
  type EdgeStatusSlot,
} from './sitesEdgeStatus';

const EXPIRY_TREATMENT_LABEL: Record<string, string> = {
  expired: 'EXPIRED — the credential must not run; renewal is a decommission/re-enroll decision',
  'rotation-due':
    'ROTATION DUE — inside the 7-day rotation window (P6.06/P6.17); renewal must start now',
  valid: 'VALID — beyond the rotation window',
  'not-reported': 'NOT REPORTED — the record carries no expiry; never read as "valid forever"',
};

export function SitesEdgeStatusSection({ data }: { data: EdgeStatusData }): JSX.Element {
  return (
    <section aria-label="Edge status (per site)" data-testid="sites-edge-status">
      <h3>Edge status (per edge identity)</h3>
      <p>
        A POLLING read of <code>GET /api/v1/edge/status</code> per edge
        identity (no live stream on this resource — recorded decision).
        Absent operational facts render as explicit gaps, never as plausible
        healthy values. Read at {formatUnixMillis(data.readAtUnixMillis)}.
      </p>
      {data.slots.map((slot) => (
        <EdgeStatusArticle key={slot.deviceId} slot={slot} readAtUnixMillis={data.readAtUnixMillis} />
      ))}
    </section>
  );
}

function EdgeStatusArticle({
  slot,
  readAtUnixMillis,
}: {
  slot: EdgeStatusSlot;
  readAtUnixMillis: number;
}): JSX.Element {
  return (
    <article data-testid={`edge-status-${slot.deviceId}`}>
      <h4>
        Edge identity <code>{slot.deviceId}</code>
      </h4>
      {slot.error !== null ? (
        <div role="alert" data-testid="edge-status-error">
          <p>
            The edge status could not be read for this identity. The panel
            shows no status rather than stale or invented data.
          </p>
          <p>
            API error (verbatim): <code>{slot.error}</code>
          </p>
        </div>
      ) : slot.row ? (
        <EdgeStatusDetails
          row={slot.row}
          deviceId={slot.deviceId}
          readAtUnixMillis={readAtUnixMillis}
        />
      ) : (
        <HonestEmpty
          what="Edge status"
          reason="No status row was produced for this identity (no data and no error) — nothing is fabricated."
        />
      )}
    </article>
  );
}

function EdgeStatusDetails({
  row,
  deviceId,
  readAtUnixMillis,
}: {
  row: EdgeStatusRow;
  deviceId: string;
  readAtUnixMillis: number;
}): JSX.Element {
  const { status, certificateTreatment, commandPathEnabled } = row;
  return (
    <>
      <dl>
        <dt>Identity term (registry, verbatim)</dt>
        <dd>
          status <code>{status.enrollment_status}</code>, term{' '}
          <code>{String(status.enrollment_seq)}</code>, identity expiry{' '}
          {status.identity_expires_at_unix_millis === null
            ? NOT_PRESENT
            : formatUnixMillis(status.identity_expires_at_unix_millis)}
        </dd>

        <dt>Edge connectivity (last-seen)</dt>
        <dd data-testid="edge-connectivity-last-seen">
          {status.connectivity.last_seen_unix_millis === null
            ? `last seen: ${NOT_PRESENT} — ${status.connectivity.source_note}`
            : `last seen ${formatUnixMillis(status.connectivity.last_seen_unix_millis)} — ${status.connectivity.source_note}`}
        </dd>

        <dt>Certificate expiry (rotation-window treatment)</dt>
        <dd data-testid={`certificate-expiry-${certificateTreatment}`}>
          <strong>[{certificateTreatment}]</strong>{' '}
          {EXPIRY_TREATMENT_LABEL[certificateTreatment]} — recorded expiry:{' '}
          {status.certificate_expiry_unix_millis === null
            ? NOT_PRESENT
            : formatUnixMillis(status.certificate_expiry_unix_millis)}
        </dd>

        <dt>Buffer depth (P6.09)</dt>
        <dd data-testid="edge-buffer-depth">
          {status.buffer_depth === null || status.buffer_capacity === null
            ? `not reported (the gap is named, never assumed empty): depth ${String(status.buffer_depth)} / capacity ${String(status.buffer_capacity)}`
            : `${status.buffer_depth} record(s) buffered of capacity ${status.buffer_capacity}`}
        </dd>

        <dt>Adapter health (P6.17 closed vocabulary, verbatim)</dt>
        <dd>
          {status.adapter_health_snapshot_json === null ? (
            <HonestEmpty
              what="Adapter health"
              reason="No adapter-health snapshot was reported for this edge — the gap is named, never rendered as healthy."
              testId="adapter-health-gap"
            />
          ) : (
            <pre data-testid="adapter-health-verbatim">
              <code>{status.adapter_health_snapshot_json}</code>
            </pre>
          )}
        </dd>
      </dl>

      <h4>Data freshness (§20.2 — prominent)</h4>
      <p data-testid="edge-data-freshness">
        {status.data_freshness.last_event_unix_millis === null
          ? status.data_freshness.note
          : ''}
        <FreshnessLine
          eventTime={
            status.data_freshness.last_event_unix_millis === null
              ? null
              : formatUnixMillis(status.data_freshness.last_event_unix_millis)
          }
          readAt={formatUnixMillis(readAtUnixMillis)}
        />
      </p>
      <p>
        The measured / estimated / stale / simulated treatments of the values
        behind this freshness line are decided ONCE by the shared treatment
        record (components/ValueType <code>valueTypeFor</code> + the
        persistent-shell legend) — this panel renders no new visual
        language and never upgrades a stale or simulated value into a
        measured one.
      </p>

      <h4>Command-path state (P6.14 gate)</h4>
      <CommandPathStateBlock
        deviceId={deviceId}
        state={status.command_path.state}
        enabled={commandPathEnabled}
        enablementValidUntil={status.command_path.enablement_valid_until_unix_millis}
        note={status.command_path.note}
      />
    </>
  );
}

function CommandPathStateBlock({
  deviceId,
  state,
  enabled,
  enablementValidUntil,
  note,
}: {
  deviceId: string;
  state: string;
  enabled: boolean;
  enablementValidUntil: number | null;
  note: string;
}): JSX.Element {
  const expiry =
    enablementValidUntil === null
      ? 'no enablement expiry recorded'
      : `enablement expires ${formatUnixMillis(enablementValidUntil)}`;
  return (
    <div data-testid={`command-path-${state}`}>
      <p data-testid="command-path-state">
        <strong>
          {enabled
            ? `[COMMAND PATH: ${state} — TEST ENV ONLY; ${expiry}]`
            : `[COMMAND PATH: ${state}]`}
        </strong>
      </p>
      <p>{note}</p>
      <p data-testid="proposed-vs-executed">
        A PROPOSED setpoint is never an executed command on this console: an
        enabled test-env path admits commands in the TEST ENVIRONMENT ONLY,
        and acknowledgement is NOT physical delivery (§14.1).
      </p>
      {enabled || state === 'NOT_REPORTED' ? (
        <CommandPathRequestAffordance deviceId={deviceId} reportedState={state} />
      ) : (
        <p data-testid="no-request-affordance">
          The command path is DISABLED for this edge; there is no enable
          affordance on this console (the server has no enable endpoint
          either). Enablement requires the signed per-site configuration
          change procedure (P6.14), which starts OFF this console.
        </p>
      )}
    </div>
  );
}

function CommandPathRequestAffordance({
  deviceId,
  reportedState,
}: {
  deviceId: string;
  reportedState: string;
}): JSX.Element {
  const [confirmed, setConfirmed] = useState(false);
  const [recorded, setRecorded] = useState(false);
  return (
    <div data-testid="command-path-request">
      <p data-testid="command-path-request-note">{COMMAND_PATH_REQUEST_NOTE}</p>
      {!recorded ? (
        <>
          <p>
            <label htmlFor={`command-path-request-confirm-${deviceId}`}>
              <input
                type="checkbox"
                id={`command-path-request-confirm-${deviceId}`}
                data-testid="command-path-request-checkbox"
                aria-required="true"
                checked={false}
                onChange={() => setConfirmed(true)}
              />{' '}
              I have read the statement above and want to RECORD A REQUEST
              for the signed per-site configuration change for edge{' '}
              <code>{deviceId}</code>.
            </label>
          </p>
          <p>
            <button
              type="button"
              data-testid="command-path-request-button"
              disabled={!confirmed}
              onClick={() => {
                recordCommandPathEnablementRequest({
                  deviceId,
                  reportedState,
                  reason:
                    'operator requested the signed per-site configuration change (recorded locally; takes effect ONLY through the P6.14 procedure)',
                });
                setRecorded(true);
              }}
            >
              Record request (local console event only)
            </button>
          </p>
        </>
      ) : (
        <p data-testid="command-path-request-recorded" role="status">
          REQUEST RECORDED LOCALLY at {new Date().toISOString()} for edge{' '}
          <code>{deviceId}</code>. Nothing was enabled: no endpoint call was
          made, and enablement takes effect ONLY through the P6.14
          procedure.
        </p>
      )}
    </div>
  );
}
