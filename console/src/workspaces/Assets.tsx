// P5.03 — the Assets workspace (report §20.1 item 4), on the Phase 2
// registry contracts (P2.17 asset views + P2.18 current states, dto.rs).
//
// Elements per §20.1 item 4 / ledger P5.03 step 3:
// - asset state and metadata (registry lifecycle + stored maintenance/
//   outage status — a stored registry field per dto.rs, not a live
//   maintenance system),
// - capability: the registry carries the capability_profile REFERENCE; the
//   constraint data itself is the Phase 3 contract (P3.01–P3.04) and is
//   not served by a Phase 1–4 read API — the reference is rendered
//   verbatim and the boundary stated,
// - telemetry latest values WITH the frozen quality state per value
//   (GOOD/SUSPECT/BAD/MISSING/ESTIMATED/STALE — exactly one per value,
//   §10.4) plus freshness (event time + age) and the simulated provenance
//   flag (P2.11: every Phase 2 value is simulator-derived),
// - availability state: derived per the mirrored P2.16 rule
//   (components/availability.tsx) from the asset's `availability` quantity,
// - limits: the stored rated magnitudes (the P2.17 contract note is
//   rendered: these are NOT runtime capability envelopes),
// - alarms: the P5.31 derived-on-read contract (GET /api/v1/alarms,
//   components/alarmsPanel.tsx) — the real per-asset alarm list with its
//   honest empty state; no alarm semantics are invented here either.
//
// Role-specific visibility (P5.03 step 4): same discipline as Portfolios —
// the claims-derived role line plus API-enforced visibility; tested with
// two roles in Assets.test.tsx.
import type { JSX } from 'react';
import type { ApiClient } from '../api/client';
import type { AssetDto, CurrentStateDto } from '../api/types';
import { getAssetCurrentState } from '../api/telemetry';
import {
  fetchAssetView,
  fetchAssets,
  type AssetViewDto,
} from '../api/views';
import { AvailabilityLine, deriveAvailability } from '../components/availability';
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
import { listAlarms, type AlarmPageEnvelope } from '../api/alarms';
import { AssetAlarmsPanel } from '../components/alarmsPanel';

export interface AssetRow {
  asset: AssetDto;
  view: AssetViewDto | null;
  viewError: string | null;
  state: CurrentStateDto | null;
  stateError: string | null;
}

export interface AssetsData {
  rows: AssetRow[];
  /** The derived-on-read alarm envelope (P5.31), or null on read failure. */
  alarms: AlarmPageEnvelope | null;
  /** The honest alarms-read error (never swallowed into an empty list). */
  alarmsError: string | null;
}

export async function loadAssets(client: ApiClient): Promise<AssetsData> {
  const assets = await fetchAssets(client);
  const rows = await Promise.all(
    assets.map(async (asset) => {
      const [viewResult, stateResult] = await Promise.all([
        fetchAssetView(asset.id, client)
          .then((view) => ({ view, viewError: null }))
          .catch((error: unknown) => ({
            view: null,
            viewError: errorText(error),
          })),
        getAssetCurrentState(asset.id, client)
          .then((envelope) => ({ state: envelope.data, stateError: null }))
          .catch((error: unknown) => ({
            state: null,
            stateError: errorText(error),
          })),
      ]);
      return { asset, ...viewResult, ...stateResult };
    }),
  );
  // The P5.31 alarm derivation (one tenant-scoped read; a failed alarms
  // read is a per-panel error, never a silent empty list).
  const alarms = await listAlarms(undefined, client).then(
    (envelope) => ({ alarms: envelope as AlarmPageEnvelope | null, alarmsError: null as string | null }),
    (error: unknown) => ({ alarms: null as AlarmPageEnvelope | null, alarmsError: errorText(error) }),
  );
  return { rows, ...alarms };
}

export function AssetsWorkspace(): JSX.Element {
  const state = useWorkspaceData(loadAssets);
  return <AssetsView state={state} />;
}

export function AssetsView({ state }: { state: LoadState<AssetsData> }): JSX.Element {
  return (
    <section aria-label="Assets">
      <h2>Assets</h2>
      <RoleScopeLine />
      {state.status === 'loading' && <LoadingLine what="the assets" />}
      {state.status === 'error' && <ErrorBlock what="The asset registry" error={state.error} />}
      {state.status === 'ready' && <AssetsReady data={state.data} />}
    </section>
  );
}

function AssetsReady({ data }: { data: AssetsData }): JSX.Element {
  if (data.rows.length === 0) {
    return (
      <HonestEmpty
        what="Assets"
        reason="The tenant-scoped asset read returned no assets. No asset row is fabricated."
      />
    );
  }
  return (
    <>
      {data.rows.map((row) => (
        <AssetSection
          key={row.asset.id}
          row={row}
          alarms={data.alarms}
          alarmsError={data.alarmsError}
        />
      ))}
    </>
  );
}

function AssetSection({
  row,
  alarms,
  alarmsError,
}: {
  row: AssetRow;
  alarms: AlarmPageEnvelope | null;
  alarmsError: string | null;
}): JSX.Element {
  const { asset, view, viewError, state, stateError } = row;
  const availabilityQuantity = state?.quantities.find((q) => q.quantity === 'availability');
  const availabilityState = availabilityQuantity
    ? deriveAvailability(availabilityQuantity.quality, availabilityQuantity.value_bool)
    : deriveAvailability(null, undefined);
  return (
    <article data-testid={`asset-${asset.id}`}>
      <h3>
        {asset.name} <small>({asset.id})</small>
      </h3>
      <dl>
        <dt>Asset type</dt>
        <dd>
          <code>{asset.asset_type}</code>
        </dd>
        <dt>Vendor model</dt>
        <dd>{asset.vendor_model}</dd>
        <dt>Phase / bus reference</dt>
        <dd>
          {asset.phase} @ <code>{asset.bus_reference}</code>
        </dd>
        <dt>Capability profile</dt>
        <dd>
          <code>{asset.capability_profile}</code> — the registry reference
          only; the constraint data is the Phase 3 contract (P3.01–P3.04) and
          is not served by a Phase 1–4 read API, so it is not rendered here.
        </dd>
        <dt>Lifecycle status (registry)</dt>
        <dd>
          <code>{asset.lifecycle_status}</code>
          {asset.deactivation_reason ? ` — reason (verbatim): ${asset.deactivation_reason}` : ''}
        </dd>
        <dt>Rated limits (stored registry magnitudes)</dt>
        <dd>
          {asset.rated_power_mw} MW
          {asset.rated_energy_mwh !== undefined
            ? ` / ${asset.rated_energy_mwh} MWh`
            : ''} — NOT runtime capability envelopes (P2.17 contract note);
          envelopes are the Phase 3 surface.
        </dd>
        <dt>Maintenance / outage status (stored registry field)</dt>
        <dd>
          <code>{view?.maintenance_outage_status ?? asset.lifecycle_status}</code>{' '}
          — the stored registry lifecycle (P2.17), not a live maintenance
          system's state.
        </dd>
        <dt>Created</dt>
        <dd>{asset.created_at}</dd>
      </dl>

      {viewError !== null && (
        <div role="note" data-testid="asset-view-error">
          <p>
            The P2.17 asset view could not be read for this asset:
          </p>
          <p>
            API error (verbatim): <code>{viewError}</code>
          </p>
        </div>
      )}

      <AvailabilityLine
        state={availabilityState}
        quality={availabilityQuantity?.quality ?? null}
        value={availabilityQuantity?.value_bool}
        eventTime={availabilityQuantity?.event_time ?? null}
        ageMs={availabilityQuantity?.age_ms ?? null}
      />

      <h4>Telemetry latest values (quality per value, §10.4)</h4>
      {stateError !== null ? (
        <div role="note" data-testid="asset-state-error">
          <p>The current state could not be read for this asset:</p>
          <p>
            API error (verbatim): <code>{stateError}</code>
          </p>
        </div>
      ) : state === null || state.quantities.length === 0 ? (
        <HonestEmpty
          what="Telemetry latest values"
          reason="No current state was returned for this asset (P2.18). No value is fabricated; a missing value is never shown as zero."
        />
      ) : (
        <table>
          <caption>
            Latest record per quantity (latest by event_time, P2.18), each
            with its frozen quality state, §20.2 value type, freshness, and
            the simulated provenance flag from the record itself.
          </caption>
          <thead>
            <tr>
              <th scope="col">Quantity</th>
              <th scope="col">Value</th>
              <th scope="col">Type (§20.2)</th>
              <th scope="col">Quality (§10.4)</th>
              <th scope="col">Freshness</th>
            </tr>
          </thead>
          <tbody>
            {state.quantities.map((q) => (
              <tr key={q.quantity}>
                <td>
                  {q.quantity}
                  {q.q_sign_convention ? ` (declared Q sign: ${q.q_sign_convention})` : ''}
                </td>
                <td>
                  {q.value_num !== undefined
                    ? `${q.value_num} ${q.unit}`
                    : q.value_bool !== undefined
                      ? `${String(q.value_bool)} (${q.unit})`
                      : q.value_text !== undefined
                        ? `${q.value_text} (${q.unit})`
                        : `(no value present — quality ${q.quality}; never read as zero, P2.18)`}
                </td>
                <td>
                  <ValueTypeBadge type={valueTypeFor(q.quality, q.simulated)} />
                </td>
                <td>
                  <QualityStateBadge quality={q.quality} />
                </td>
                <td>
                  <FreshnessLine eventTime={q.event_time} ageMs={q.age_ms} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      <h4>Alarms</h4>
      {/* P5.31: the real derived-on-read alarm list for this asset (the
          per-asset filter of the tenant-scoped GET /api/v1/alarms read),
          with the honest empty state and the model-source citation. */}
      <AssetAlarmsPanel assetId={asset.id} alarms={alarms} alarmsError={alarmsError} />
    </article>
  );
}
