// P5.31 — the alarm panels (the §20.1 item 1 "key alarms" resolution):
// the console consumes the derived-on-read GET /api/v1/alarms resource
// (routes_alarms.rs) and renders the alarm-model statement VERBATIM — the
// model-source citation travels with every panel, never paraphrased.
//
// Honesty rules implemented here:
// - alarms are OPERATOR-ATTENTION records derived from existing platform
//   facts, NOT protection signals (the statement in the response says so;
//   this component renders it, never paraphrases it);
// - the empty state is honest and explicit: no alarms are currently
//   DERIVABLE — nothing is fabricated when the derivation returns none;
// - a failed alarms read is an honest error block, never an empty list and
//   never a fabricated panel (the workspaces' other reads are unaffected);
// - the source inventory (consulted / unavailable / absent sources) is
//   rendered so an operator can tell "no alarms" from "alarms could not be
//   derived";
// - every alarm row carries its severity, code, scope, event-time
//   freshness, and the evidence link (source_reference) to the underlying
//   record.
import type { JSX } from 'react';
import type { AlarmDto, AlarmPageEnvelope } from '../api/alarms';
import { FreshnessLine } from './ValueType';

/** The model-source citation line (rendered under every alarm panel). */
function ModelCitation({ envelope }: { envelope: AlarmPageEnvelope }): JSX.Element {
  return (
    <p>
      Alarm model: <code>{envelope.model_version}</code> (
      <code>flexcore-contracts::alarm</code>, ledger P5.31) — derived
      records, not §15.3 events (the frozen event vocabulary is not
      extended). Statement (verbatim):{' '}
      <code data-testid="alarm-model-statement">{envelope.derivation_statement}</code>
    </p>
  );
}

/** The consulted/unavailable/absent source inventory (no invented alarms). */
function SourceInventory({ envelope }: { envelope: AlarmPageEnvelope }): JSX.Element {
  const s = envelope.sources;
  return (
    <div data-testid="alarm-source-inventory">
      <p>Alarm sources consulted for this derivation:</p>
      <ul>
        <li>
          telemetry current states: <code>{s.telemetry_current_state}</code>{' '}
          ({s.telemetry_assets_consulted} asset(s) consulted)
        </li>
        <li>dispatch-command records consulted: {s.dispatch_commands_consulted}</li>
        <li>feasibility-check evidence records consulted: {s.feasibility_checks_consulted}</li>
        {s.skipped_defective_source_records > 0 && (
          <li>
            defective source records skipped and COUNTED (never derived with
            an invented timestamp): {s.skipped_defective_source_records}
          </li>
        )}
      </ul>
      {s.absent_sources.length > 0 && (
        <>
          <p>Sources that do not exist in this phase (no alarms can come from them):</p>
          <ul>
            {s.absent_sources.map((source) => (
              <li key={source}>{source}</li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
}

/**
 * The full alarm panel (the Overview "Key alarms" replacement): the real
 * derived alarm list with its honest empty state and the model-source
 * citation.
 */
export function AlarmsPanel({
  alarms,
  alarmsError,
  readAt,
}: {
  alarms: AlarmPageEnvelope | null;
  alarmsError: string | null;
  readAt: string;
}): JSX.Element {
  if (alarmsError !== null) {
    return (
      <div role="note" data-testid="alarms-error">
        <p>
          The alarms read failed. The console shows no alarm rather than an
          empty list that would look like "no alarms" (a failed derivation is
          not a clean derivation).
        </p>
        <p>
          API error (verbatim): <code>{alarmsError}</code>
        </p>
      </div>
    );
  }
  if (alarms === null) {
    return (
      <div role="note" data-testid="alarms-unavailable">
        <p>The alarms read has not completed yet; no alarm is displayed.</p>
      </div>
    );
  }
  return (
    <div data-testid="alarms-panel">
      <ModelCitation envelope={alarms} />
      {alarms.data.length === 0 ? (
        <div role="note" data-testid="alarms-empty">
          <p>
            <strong>No alarms are currently derivable.</strong> The
            derivation from the platform's current facts (telemetry quality
            states, dispatch outcomes, feasibility verdicts) returned no
            qualifying condition. This is a derivation result, not a promise
            that everything is healthy — no alarm is fabricated, and a
            failed derivation would have been shown as an error above, never
            as this empty state.
          </p>
        </div>
      ) : (
        <AlarmTable alarms={alarms.data} />
      )}
      <SourceInventory envelope={alarms} />
      <p>
        <FreshnessLine readAt={readAt} /> Derived at{' '}
        <code>{alarms.derived_at}</code> — alarms are derived on read (no
        persistence exists in this phase): the list is exactly what the
        platform's current facts yield, and no cleared or acknowledged alarm
        can be served.
      </p>
    </div>
  );
}

/** The per-asset alarm list (the Assets workspace per-asset panel). */
export function AssetAlarmsPanel({
  assetId,
  alarms,
  alarmsError,
}: {
  assetId: string;
  alarms: AlarmPageEnvelope | null;
  alarmsError: string | null;
}): JSX.Element {
  if (alarmsError !== null) {
    return (
      <div role="note" data-testid="asset-alarms-error">
        <p>The alarms read failed for this asset's scope:</p>
        <p>
          API error (verbatim): <code>{alarmsError}</code>
        </p>
      </div>
    );
  }
  if (alarms === null) {
    return (
      <div role="note" data-testid={`asset-alarms-unavailable-${assetId}`}>
        <p>The alarms read has not completed yet; no alarm is displayed.</p>
      </div>
    );
  }
  const assetAlarms = alarms.data.filter((alarm) => alarm.asset_id === assetId);
  return (
    <div data-testid={`asset-alarms-${assetId}`}>
      <ModelCitation envelope={alarms} />
      {assetAlarms.length === 0 ? (
        <p>
          No alarms are currently derivable for this asset from the
          platform's current facts. Nothing is fabricated; consult the
          derivation inventory on the Overview workspace for which sources
          were consulted.
        </p>
      ) : (
        <AlarmTable alarms={assetAlarms} />
      )}
    </div>
  );
}

/** The severity text treatment (text-borne, not colour alone — §20.2). */
function SeverityBadge({ alarm }: { alarm: AlarmDto }): JSX.Element {
  return (
    <code data-testid={`alarm-severity-${alarm.code}`}>
      {alarm.severity === 'CRITICAL'
        ? '[CRITICAL]'
        : alarm.severity === 'WARNING'
          ? '[WARNING]'
          : '[INFO]'}
    </code>
  );
}

function AlarmTable({ alarms }: { alarms: AlarmDto[] }): JSX.Element {
  return (
    <table>
      <caption>
        Currently derivable alarms (derived on read from the platform's
        current facts; operator-attention records, never protection
        signals). Ordered by severity rank then alarm id.
      </caption>
      <thead>
        <tr>
          <th scope="col">Severity</th>
          <th scope="col">Code</th>
          <th scope="col">Message</th>
          <th scope="col">Scope</th>
          <th scope="col">Raised (event time)</th>
          <th scope="col">State</th>
          <th scope="col">Evidence (source reference)</th>
        </tr>
      </thead>
      <tbody>
        {alarms.map((alarm) => (
          <tr key={alarm.alarm_id}>
            <td>
              <SeverityBadge alarm={alarm} />
            </td>
            <td>
              <code>{alarm.code}</code>
            </td>
            <td>{alarm.message}</td>
            <td>
              {alarm.site_id ? `site ${alarm.site_id}` : 'tenant-wide'}
              {alarm.asset_id ? ` / asset ${alarm.asset_id}` : ''}
            </td>
            <td>
              <FreshnessLine eventTime={alarm.raised_at} />
            </td>
            <td>
              <code>{alarm.state}</code>
            </td>
            <td>
              <code>
                {alarm.source_kind}: {alarm.source_reference}
              </code>
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
