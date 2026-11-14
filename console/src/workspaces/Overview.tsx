// P5.02 — the Overview workspace (report §20.1 item 1): fleet availability,
// current aggregate power, active schedules, key alarms, delivery status.
//
// §20.2 honesty implemented here:
// - every value carries its freshness (event time + age, or the console's
//   read-at time for list records that carry no served timestamp);
// - value types are visually distinct and TEXT-borne (ValueTypeBadge):
//   every Phase 2 telemetry value is typed `simulated` from its own
//   provenance flag — the treatment is part of the component, not an
//   optional prop (the ledger risk row's mitigation);
// - the aggregate never shows a missing fleet as 0 MW and lists every
//   excluded asset with its reason;
// - the alarm panel consumes the P5.31 derived-on-read contract (GET
//   /api/v1/alarms; components/alarmsPanel.tsx): the real derived alarm
//   list, an honest empty state when nothing is derivable, an honest error
//   block when the derivation cannot be read — never a fabricated alarm;
// - delivery status reuses the Phase 4 handover wording: every issue is to
//   the SIMULATOR; acknowledgement is application-layer acceptance and is
//   NEVER displayed as physical delivery (32_handover_phase5.md §2; the
//   API's simulation_only_statement is rendered verbatim, not paraphrased).
//
// Tenant scoping (§16.2): all reads go through the P5.01 api clients, which
// send credentials only — no tenant selector exists anywhere on the wire.
import type { JSX } from 'react';
import type { ApiClient } from '../api/client';
import type {
  AssetDto,
  CurrentStateDto,
  DispatchCommandDto,
  ScheduleRevisionDto,
} from '../api/types';
import { listAssets } from '../api/registry';
import { listSchedules } from '../api/flexibility';
import { listDispatchCommands } from '../api/dispatch';
import { getAssetCurrentState } from '../api/telemetry';
import { listAlarms, type AlarmPageEnvelope } from '../api/alarms';
import {
  ErrorBlock,
  HonestEmpty,
  LoadingLine,
  RoleScopeLine,
  errorText,
  useWorkspaceData,
  type LoadState,
} from '../components/workspaceData';
import { FreshnessLine, QualityStateBadge, ValueTypeBadge, valueTypeFor } from '../components/ValueType';
import { formatUnixMillis } from '../components/format';
import { AlarmsPanel } from '../components/alarmsPanel';
import {
  ALARM_CONTRACT_TEXT,
  CONTROL_AUTHORITY_MODE_TEXT,
  SIGN_CONVENTION_LABEL,
  aggregateActivePower,
  selectActiveSchedules,
  summarizeDeliveryStatus,
  summarizeFleetAvailability,
} from './overviewData';

export interface OverviewData {
  assets: AssetDto[];
  /** Per-asset current state, or the honest read error (never dropped). */
  assetStates: Array<{ asset_id: string; state: CurrentStateDto | null; error: string | null }>;
  schedules: ScheduleRevisionDto[];
  commands: DispatchCommandDto[];
  /** The derived-on-read alarm envelope, or null when the read failed. */
  alarms: AlarmPageEnvelope | null;
  /** The honest alarms-read error (never swallowed into an empty list). */
  alarmsError: string | null;
}

/**
 * The Overview read: tenant-scoped list reads plus one current-state read
 * per asset. Recorded limitation (decision record 02_04_workspaces_a.md):
 * no fleet-level aggregate/current-state endpoint exists on the §15.2
 * surface, so the console composes per-asset reads; a failed per-asset
 * read is rendered as an exclusion with its error, never as a zero.
 */
export async function loadOverview(client: ApiClient): Promise<OverviewData> {
  const [assetsPage, schedulesPage, commandsPage] = await Promise.all([
    listAssets(undefined, client),
    listSchedules(undefined, client),
    listDispatchCommands(undefined, client),
  ]);
  const assetStates = await Promise.all(
    assetsPage.data.map(async (asset) => {
      try {
        const envelope = await getAssetCurrentState(asset.id, client);
        return { asset_id: asset.id, state: envelope.data, error: null };
      } catch (error: unknown) {
        return { asset_id: asset.id, state: null, error: errorText(error) };
      }
    }),
  );
  // The P5.31 alarm derivation: a failed alarms read is an honest
  // per-panel error (never swallowed into an empty list that would read as
  // "no alarms"), while the workspace's other elements stay served.
  const alarms = await listAlarms(undefined, client).then(
    (envelope) => ({ alarms: envelope as AlarmPageEnvelope | null, alarmsError: null as string | null }),
    (error: unknown) => ({ alarms: null as AlarmPageEnvelope | null, alarmsError: errorText(error) }),
  );
  return {
    assets: assetsPage.data,
    assetStates,
    schedules: schedulesPage.data,
    commands: commandsPage.data,
    ...alarms,
  };
}

/** The workspace shell the router renders (P5.01 registry pattern). */
export function OverviewWorkspace(): JSX.Element {
  const state = useWorkspaceData(loadOverview);
  return <OverviewView state={state} />;
}

export function OverviewView({ state }: { state: LoadState<OverviewData> }): JSX.Element {
  return (
    <section aria-label="Overview">
      <h2>Overview</h2>
      <p className="mode-banner" role="note" data-testid="mode-line">
        {CONTROL_AUTHORITY_MODE_TEXT}
      </p>
      <RoleScopeLine />
      {state.status === 'loading' && <LoadingLine what="the overview" />}
      {state.status === 'error' && <ErrorBlock what="The overview data" error={state.error} />}
      {state.status === 'ready' && <OverviewReady data={state.data} readAt={state.readAt} />}
    </section>
  );
}

function OverviewReady({ data, readAt }: { data: OverviewData; readAt: string }): JSX.Element {
  const usableStates = data.assetStates
    .map((entry) => entry.state)
    .filter((state): state is CurrentStateDto => state !== null);
  const availability = summarizeFleetAvailability(usableStates);
  const power = aggregateActivePower(usableStates);
  const activeSchedules = selectActiveSchedules(data.schedules);
  const delivery = summarizeDeliveryStatus(data.commands);
  const failedStateReads = data.assetStates.filter((entry) => entry.error !== null);
  const simulationStatement = data.commands[0]?.simulation_only_statement ?? null;

  return (
    <>
      <h3>Fleet availability</h3>
      {usableStates.length === 0 ? (
        <HonestEmpty
          what="Fleet availability"
          reason="No asset current state could be read (no assets returned, or every per-asset state read failed). Nothing is displayed rather than an invented availability figure."
        />
      ) : (
        <>
          <ul>
            <li>available: {availability.available.length}</li>
            <li>unavailable: {availability.unavailable.length}</li>
            <li>unknown (no trustworthy signal): {availability.unknown.length}</li>
            <li>no new signal (MISSING/STALE): {availability.no_new_signal.length}</li>
          </ul>
          <p>{availability.derivation}</p>
          <p>
            <FreshnessLine eventTime={availability.freshness_event_time} readAt={readAt} />
          </p>
        </>
      )}
      {failedStateReads.length > 0 && (
        <div role="note" data-testid="state-read-failures">
          <p>
            {failedStateReads.length} per-asset state read(s) failed and are
            excluded from the figures above (never silently dropped):
          </p>
          <ul>
            {failedStateReads.map((entry) => (
              <li key={entry.asset_id}>
                {entry.asset_id}: <code>{entry.error}</code>
              </li>
            ))}
          </ul>
        </div>
      )}

      <h3>Current aggregate power</h3>
      {power.total_mw === null ? (
        <HonestEmpty
          what="Current aggregate power"
          reason="No usable contributor exists (no asset reports active_power with GOOD/SUSPECT quality and a numeric value). The aggregate is NOT shown as 0 MW: a missing aggregate is not a zero aggregate (P2.18 discipline)."
        />
      ) : (
        <>
          <p data-testid="aggregate-power">
            <ValueTypeBadge type="simulated" />{' '}
            <strong>
              {power.total_mw} MW
            </strong>{' '}
            across {power.contributor_count} asset(s). {SIGN_CONVENTION_LABEL}
          </p>
          <p>
            <FreshnessLine eventTime={power.freshness_event_time} readAt={readAt} />
          </p>
          <p>
            Composed by the console from per-asset telemetry reads (no
            fleet-aggregate endpoint exists on the §15.2 surface — recorded
            limitation); every input carries <code>simulated: true</code>, so
            the aggregate is typed simulated. Excluded assets (shown, never
            silently dropped):
          </p>
          {power.excluded.length === 0 ? (
            <p>none — every consulted asset contributed.</p>
          ) : (
            <ul>
              {power.excluded.map((entry) => (
                <li key={entry.asset_id}>
                  {entry.asset_id}: {entry.reason}
                </li>
              ))}
            </ul>
          )}
        </>
      )}

      <h3>Active schedules</h3>
      {activeSchedules.length === 0 ? (
        <HonestEmpty
          what="Active schedules"
          reason="No schedule revision in the active state was returned by the tenant-scoped read. Superseded revisions are retained server-side and are not fabricated here."
        />
      ) : (
        <>
          <table>
            <caption>
              Active schedule revisions (ID + revision; §20.1 item 1). Read
              at {readAt} — the schedule record carries no served freshness
              timestamp; the persisted horizon bounds are shown per row.
            </caption>
            <thead>
              <tr>
                <th scope="col">Schedule ID</th>
                <th scope="col">Revision</th>
                <th scope="col">Site</th>
                <th scope="col">Horizon (start — end, UTC)</th>
                <th scope="col">Roll trigger</th>
              </tr>
            </thead>
            <tbody>
              {activeSchedules.map((schedule) => (
                <tr key={`${schedule.schedule_id}#${schedule.revision}`}>
                  <td>{schedule.schedule_id}</td>
                  <td>{schedule.revision}</td>
                  <td>{schedule.site_id}</td>
                  <td>
                    {formatUnixMillis(schedule.horizon_start_millis)} —{' '}
                    {formatUnixMillis(schedule.horizon_end_millis)}
                  </td>
                  <td>{schedule.roll_trigger}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}

      <h3>Key alarms</h3>
      <AlarmsPanel alarms={data.alarms} alarmsError={data.alarmsError} readAt={readAt} />
      {/* The resolved contract citation (P5.31) stays under the panel. */}
      <p data-testid="alarm-contract-note">{ALARM_CONTRACT_TEXT}</p>

      <h3>Delivery status</h3>
      {delivery.total === 0 ? (
        <HonestEmpty
          what="Delivery status"
          reason="No dispatch command was returned by the tenant-scoped read — there is nothing to qualify, and no delivery state is invented."
        />
      ) : (
        <div data-testid="delivery-status">
          <p>
            Dispatch commands by persisted §14.2 state ({delivery.total}{' '}
            total):
          </p>
          <ul>
            {Object.entries(delivery.counts).map(([stateName, count]) => (
              <li key={stateName}>
                {stateName}: {count}
                {stateName === 'ISSUED'
                  ? ' — issued to the SIMULATOR: a simulated issue, not field delivery (32_handover_phase5.md §2)'
                  : ''}
                {stateName === 'ACKNOWLEDGED'
                  ? ' — acknowledgement is application-layer acceptance by the simulator; acknowledgement is NOT physical delivery (§14.1)'
                  : ''}
              </li>
            ))}
          </ul>
          {simulationStatement && (
            <p>
              API simulation-only statement (verbatim): <code>{simulationStatement}</code>
            </p>
          )}
          <p>
            <FreshnessLine readAt={readAt} />
          </p>
        </div>
      )}

      <h3>Fleet telemetry quality (per-value states)</h3>
      {usableStates.length === 0 ? (
        <HonestEmpty
          what="Per-value telemetry quality"
          reason="No current state available to display quality for."
        />
      ) : (
        <QualitySample assetStates={usableStates} />
      )}
    </>
  );
}

/**
 * One sample row per asset's active_power reading, showing the frozen
 * quality state and the resolved value type side by side (the §20.2
 * treatments are visible together, not tribal knowledge — P5.10 renders
 * the console-wide legend).
 */
function QualitySample({ assetStates }: { assetStates: CurrentStateDto[] }): JSX.Element {
  const rows = assetStates.flatMap((state) => {
    const quantity = state.quantities.find((q) => q.quantity === 'active_power');
    if (!quantity) return [];
    return [{ state, quantity }];
  });
  if (rows.length === 0) {
    return (
      <p>
        No active_power readings available to display quality treatments for.
      </p>
    );
  }
  return (
    <table>
      <caption>
        Sample of the frozen six-state telemetry quality (§10.4) and the §20.2
        value types, per asset's latest active_power reading.
      </caption>
      <thead>
        <tr>
          <th scope="col">Asset</th>
          <th scope="col">Value</th>
          <th scope="col">Type (§20.2)</th>
          <th scope="col">Quality (§10.4)</th>
          <th scope="col">Freshness</th>
        </tr>
      </thead>
      <tbody>
        {rows.map(({ state, quantity }) => (
          <tr key={state.asset_id}>
            <td>{state.asset_id}</td>
            <td>
              {quantity.value_num === undefined ? '(no numeric value)' : `${quantity.value_num} ${quantity.unit}`}
            </td>
            <td>
              <ValueTypeBadge type={valueTypeFor(quantity.quality, quantity.simulated)} />
            </td>
            <td>
              <QualityStateBadge quality={quantity.quality} />
            </td>
            <td>
              <FreshnessLine eventTime={quantity.event_time} ageMs={quantity.age_ms} />
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
