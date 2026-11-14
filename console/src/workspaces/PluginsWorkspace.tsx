// P7.25 — the Plugins & Integrations workspace EXTENSION (the P6.25
// `SitesEdgeStatus.tsx` precedent: a Phase-wave-added workspace panel —
// NEW files over rewrites; the P5.07 `PluginsIntegrations.tsx` workspace is
// NOT modified, and the two sections render in sequence on the same
// `plugins-integrations` route via the App.tsx wiring).
//
// WHAT AN OPERATOR CAN DETERMINE FROM THIS PANEL WITHOUT READING LOGS
// (the ledger P7.25 acceptance criterion):
// - every plugin's ID, version, class, permissions (the §5.7 ids the
//   manifest requests), contracts provided/consumed, license, and the
//   config SUMMARY (property names and types — NEVER a value, §5.7);
// - the lifecycle state from the §5.6 vocabulary rendered VERBATIM
//   (DISCOVERED..FAILED — and `NOT_REPORTED` as the named gap: no plugin
//   host reporter is composed in 0.x, so no state is plausible-by-default);
// - the health snapshot from the P7.20 closed vocabulary rendered VERBATIM
//   (HEALTHY / DEGRADED_<reason> / FAILING_<reason> — never prettified
//   into healthy-looking synonyms; `null` health renders as the gap);
// - the active profile composition: which profile composes what, its
//   permission ceiling, and its control-authority posture (always
//   `simulation-shadow` — the mode lock);
// - the audit trail for plugin lifecycle events as the HONEST GAP (no
//   plugin-lifecycle audit source is composed into the control plane).
//
// THE COMMAND-PATH GUARANTEE (P7.25 step 3, recorded; the P6.25 rule): the
// console NEVER executes a composition change. There is NO endpoint on the
// server surface that could activate, deactivate, upgrade, or switch
// anything (verified against routes_plugins.rs / openapi.rs), and the only
// affordance is "request a profile switch", which records a LOCAL request
// event (pluginsWorkspaceData.ts, the denialLog pattern) and states it
// takes effect ONLY through the reviewed profile-file edit +
// re-composition procedure (P7.16).
//
// CONFIRMATION (P7.25 step 3): the request affordance requires an explicit
// confirmation control following the ConfirmHighImpact pattern exactly
// (labelled required checkbox inside a <label>, aria-required, inline with
// the parameters, keyboard-operable, not colour- or hover-dependent) — the
// SitesEdgeStatus pattern, for the same recorded reason (the recorded
// HIGH_IMPACT_ACTIONS list in components/ConfirmHighImpact.tsx is a closed
// Phase-5 vocabulary this wave does not own).
//
// §20.2 step 4: a plugin output is a PROPOSAL and a console request is a
// REQUEST — neither is ever displayed as executed (the
// PROPOSED_VS_EXECUTED_NOTE renders on this panel).
import { useState } from 'react';
import type { JSX } from 'react';
import { FreshnessLine } from '../components/ValueType';
import {
  ErrorBlock,
  HonestEmpty,
  LoadingLine,
  RoleScopeLine,
  useWorkspaceData,
  type LoadState,
} from '../components/workspaceData';
import type { PluginRecordDto, ProfileRecordDto } from '../api/plugins';
import {
  PROPOSED_VS_EXECUTED_NOTE,
  PROFILE_SWITCH_REQUEST_NOTE,
  getProfileSwitchRequests,
  healthOf,
  lifecycleStateOf,
  loadPluginsInventory,
  profileCompositionOf,
  recordProfileSwitchRequest,
  type PluginsInventoryData,
} from './pluginsWorkspaceData';

/**
 * The P7.25 extension section the App wiring renders BELOW the P5.07
 * workspace on the same route. The P5.07 gap statement above it recorded
 * the honest PHASE 5 state; this section is the Phase 7 addition: the
 * plugin SDK now exists as WRITTEN-NOT-BUILT code (kernel + host, awaiting
 * the human build/test pass), and this panel renders the RECORDED shipped
 * inventory the control plane serves — no running plugin host is composed,
 * so every runtime fact renders as the named gap.
 */
export function PluginsInventorySection(): JSX.Element {
  const state = useWorkspaceData(loadPluginsInventory);
  return <PluginsInventoryView state={state} />;
}

export function PluginsInventoryView({
  state,
}: {
  state: LoadState<PluginsInventoryData>;
}): JSX.Element {
  return (
    <section aria-label="Plugin inventory (Phase 7)" data-testid="plugins-inventory">
      <h2>Plugin inventory (Phase 7, P7.25)</h2>
      <p className="honesty-banner" role="note" data-testid="plugins-inventory-banner">
        This section EXTENDS the P5.07 workspace above (whose gap statement
        recorded the honest Phase 5 state). The plugin SDK now exists as
        WRITTEN-NOT-BUILT code (the kernel and host crates, awaiting the
        human build/test pass — no claim here is a tool-verified fact). The
        panel renders the RECORDED shipped inventory the control plane
        serves; the control plane holds NO running plugin host, so every
        lifecycle state renders <code>NOT_REPORTED</code> and health
        renders as an explicit gap unless a deployment composes a source.
        This surface is READ-ONLY: no activate, deactivate, upgrade, or
        profile-switch endpoint exists.
      </p>
      <RoleScopeLine />
      {state.status === 'loading' && <LoadingLine what="the plugin inventory" />}
      {state.status === 'error' && <ErrorBlock what="The plugin inventory" error={state.error} />}
      {state.status === 'ready' && <PluginsInventoryReady data={state.data} readAt={state.readAt} />}
    </section>
  );
}

function PluginsInventoryReady({
  data,
  readAt,
}: {
  data: PluginsInventoryData;
  readAt: string;
}): JSX.Element {
  const { inventory } = data;
  return (
    <>
      <p data-testid="proposed-vs-executed">{PROPOSED_VS_EXECUTED_NOTE}</p>

      <h3>Plugin inventory (the recorded shipped set)</h3>
      {inventory.plugins.length === 0 ? (
        <HonestEmpty
          what="Plugin inventory"
          reason="The inventory read returned no plugin records. No plugin row is fabricated."
          testId="plugins-empty"
        />
      ) : (
        inventory.plugins.map((plugin) => (
          <PluginArticle key={plugin.plugin_id} plugin={plugin} />
        ))
      )}

      <h3>Profile composition (the recorded shipped profiles)</h3>
      <ProfileCompositionTable profiles={inventory.profiles} />
      <p data-testid="composition-change-gap">
        No composition-change record (actor + audit reference) is served by
        this surface: the recorded inventory is the shipped files as
        reviewed, composition changes happen through the reviewed
        profile-file edit + re-composition procedure (P7.16), and no
        plugin-composition event vocabulary exists in the audit trail this
        console reads. The gap is named, never filled with a plausible
        actor or reference.
      </p>

      {inventory.profiles.length === 0 ? (
        <HonestEmpty
          what="Profile switch request"
          reason="No profile was returned by the inventory read, so there is nothing to request a switch for. No request is fabricated."
          testId="profile-switch-empty"
        />
      ) : (
        <ProfileSwitchRequestAffordance profiles={inventory.profiles} />
      )}

      <h3>Plugin lifecycle audit trail</h3>
      <HonestEmpty
        what="Plugin lifecycle audit records"
        reason="No plugin-lifecycle audit source is composed into the control plane: the kernel emits lifecycle audit records through its own audit port (P7.07) inside the host, and no forwarder exists in 0.x. The gap is named — no audit record is fabricated here."
        testId="plugin-audit-gap"
      />

      <p data-testid="inventory-note">
        <strong>Surface note (server, verbatim):</strong> {inventory.inventory_note}
      </p>
      <p>
        <FreshnessLine readAt={readAt} />
      </p>
    </>
  );
}

function PluginArticle({ plugin }: { plugin: PluginRecordDto }): JSX.Element {
  const lifecycle = lifecycleStateOf(plugin);
  const health = healthOf(plugin);
  return (
    <article data-testid={`plugin-${plugin.plugin_id}`}>
      <h4>
        <code>{plugin.plugin_id}</code> <small>({plugin.class})</small>
      </h4>
      <dl>
        <dt>Version (manifest, verbatim)</dt>
        <dd>{plugin.version}</dd>

        <dt>Contracts provided / consumed</dt>
        <dd>
          provides <code>{plugin.provider_of.join(', ') || 'nothing'}</code>, consumes{' '}
          <code>{plugin.consumes.join(', ') || 'nothing'}</code>
        </dd>

        <dt>Permissions requested (§5.7 ids, verbatim)</dt>
        <dd data-testid={`permissions-${plugin.plugin_id}`}>
          <code>{plugin.permissions.join(', ') || 'none'}</code> — requested, not
          necessarily granted: the profile ceiling and the capability broker
          decide the grant, and a permission above the ceiling is REFUSED,
          never trimmed.
        </dd>

        <dt>License (manifest, required)</dt>
        <dd>{plugin.license}</dd>

        <dt>Configuration summary (schema only — no values, §5.7)</dt>
        <dd data-testid={`config-summary-${plugin.plugin_id}`}>
          {plugin.config_properties.length === 0 ? (
            'no configuration properties declared'
          ) : (
            <>
              {plugin.config_properties.map((property) => (
                <div key={property.name}>
                  <code>{property.name}</code>: {property.property_type} —{' '}
                  {property.description}
                  {plugin.config_required.includes(property.name) ? ' (required)' : ''}
                </div>
              ))}
              No configuration VALUE is displayed anywhere on this panel: the
              surface serves the schema summary only, and secret references
              resolve to handles on the host side — a plugin never receives
              a value (§5.7).
            </>
          )}
        </dd>

        <dt>Publisher</dt>
        <dd data-testid={`publisher-gap-${plugin.plugin_id}`}>
          NOT RECORDED — a publisher field does not exist in manifest schema
          v1 (deferred to the P7.14 provenance record, where it belongs with
          signature verification). The gap is named, never guessed.
        </dd>

        <dt>Lifecycle state (§5.6 closed vocabulary, verbatim)</dt>
        <dd data-testid={`lifecycle-state-${plugin.plugin_id}`}>
          {lifecycle === 'NOT_REPORTED' ? (
            <>
              <strong>[{lifecycle}]</strong> — no plugin host reporter is
              composed, so NO lifecycle state is known. This is an explicit
              gap, never a silent READY or STOPPED.
            </>
          ) : (
            <>
              <strong>[{lifecycle}]</strong> — the §5.6 state as the composed
              source reported it; the only serving state is READY.
            </>
          )}
        </dd>

        <dt>Health (P7.20 closed vocabulary, verbatim)</dt>
        <dd>
          {health === null ? (
            <HonestEmpty
              what="Plugin health"
              reason="No plugin health reporter is composed — the gap is named, never rendered as healthy."
              testId={`health-gap-${plugin.plugin_id}`}
            />
          ) : (
            <p data-testid={`health-verbatim-${plugin.plugin_id}`}>
              <strong>[{health.healthState}]</strong> readiness{' '}
              <code>{health.readiness}</code>
              {health.blocksDependentActions
                ? ' — a FAILING_* state BLOCKS dependent actions (P7.20).'
                : ''}{' '}
              Basis note (verbatim): {health.note}
            </p>
          )}
        </dd>

        <dt>Record provenance</dt>
        <dd>{plugin.record_note}</dd>
      </dl>
    </article>
  );
}

function ProfileCompositionTable({ profiles }: { profiles: ProfileRecordDto[] }): JSX.Element {
  if (profiles.length === 0) {
    return (
      <HonestEmpty
        what="Profile composition"
        reason="The inventory read returned no profile records. No profile row is fabricated."
        testId="profiles-empty"
      />
    );
  }
  return (
    <table>
      <caption>
        The recorded composition profiles. The control-authority column is
        the profile's POSTURE — with the shipped loader it can only be
        <code> simulation-shadow</code> (the mode lock: a profile can never
        select or raise the control-authority mode, §21.4); the mode itself
        remains a deployment and per-site setting.
      </caption>
      <thead>
        <tr>
          <th scope="col">Profile</th>
          <th scope="col">Version</th>
          <th scope="col">Control-authority posture (verbatim)</th>
          <th scope="col">Permission ceiling (maximum, never a grant)</th>
          <th scope="col">Active selections</th>
          <th scope="col">Composition state (verbatim)</th>
          <th scope="col">State note (verbatim)</th>
        </tr>
      </thead>
      <tbody>
        {profiles.map((profile) => {
          const composition = profileCompositionOf(profile);
          return (
            <tr key={composition.profileId} data-testid={`profile-${composition.profileId}`}>
              <td>
                <code>{composition.profileId}</code>
              </td>
              <td>{composition.version}</td>
              <td data-testid={`profile-mode-${composition.profileId}`}>
                <code>{composition.controlAuthority}</code>
              </td>
              <td>
                <code>{composition.ceiling.join(', ') || 'none'}</code>
              </td>
              <td>
                <code>{composition.activeSelections.join(', ') || 'none'}</code>
              </td>
              <td data-testid={`profile-state-${composition.profileId}`}>
                {composition.compositionState}
              </td>
              <td>{composition.stateNote}</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

/**
 * The profile-switch REQUEST affordance (the CommandPathRequestAffordance
 * pattern, exactly): the request is RECORDED LOCALLY ONLY — nothing is
 * switched, no endpoint is called, and the recorded note states the
 * procedure the change actually takes effect through.
 */
function ProfileSwitchRequestAffordance({
  profiles,
}: {
  profiles: ProfileRecordDto[];
}): JSX.Element {
  const [selectedProfileId, setSelectedProfileId] = useState<string>(profiles[0]?.profile_id ?? '');
  const [confirmed, setConfirmed] = useState(false);
  const [recorded, setRecorded] = useState(false);
  const requested = profiles.find((profile) => profile.profile_id === selectedProfileId);
  return (
    <div data-testid="profile-switch-request">
      <h3>Request a profile switch (RECORD ONLY — nothing is switched)</h3>
      <p data-testid="profile-switch-request-note">{PROFILE_SWITCH_REQUEST_NOTE}</p>
      {!recorded ? (
        <>
          <p>
            <label htmlFor="profile-switch-select">Requested profile:</label>{' '}
            <select
              id="profile-switch-select"
              data-testid="profile-switch-select"
              value={selectedProfileId}
              onChange={(event) => {
                setSelectedProfileId(event.target.value);
                setConfirmed(false);
              }}
            >
              {profiles.map((profile) => (
                <option key={profile.profile_id} value={profile.profile_id}>
                  {profile.profile_id}
                </option>
              ))}
            </select>
          </p>
          <p>
            <label htmlFor="profile-switch-confirm">
              <input
                type="checkbox"
                id="profile-switch-confirm"
                data-testid="profile-switch-checkbox"
                aria-required="true"
                checked={false}
                onChange={() => setConfirmed(true)}
              />{' '}
              I have read the statement above and want to RECORD A REQUEST to
              switch the composition to <code>{selectedProfileId}</code>
              {requested ? ` (composition state: ${requested.composition_state})` : ''}.
            </label>
          </p>
          <p>
            <button
              type="button"
              data-testid="profile-switch-button"
              disabled={!confirmed}
              onClick={() => {
                recordProfileSwitchRequest({
                  requestedProfileId: selectedProfileId,
                  reportedCompositionState: requested?.composition_state ?? 'NOT_RECORDED',
                  reason:
                    'operator requested a profile switch (recorded locally; takes effect ONLY through the P7.16 reviewed profile-file edit + re-composition procedure)',
                });
                setRecorded(true);
              }}
            >
              Record request (local console event only)
            </button>
          </p>
        </>
      ) : (
        <p data-testid="profile-switch-request-recorded" role="status">
          REQUEST RECORDED LOCALLY at {new Date().toISOString()} for profile{' '}
          <code>{selectedProfileId}</code>. Nothing was switched: no endpoint
          call was made, and the change takes effect ONLY through the P7.16
          procedure. Requests recorded this session:{' '}
          {getProfileSwitchRequests().length}.
        </p>
      )}
    </div>
  );
}
