// P5.05/P5.06/P5.09 — shared honesty primitives for the data workspaces
// (§20.2 as applied by ledger P5.05–P5.09; the workspace-wide treatment
// pass and its legend are P5.10–P5.12 and will restyle, not remove, these
// contracts).
//
// P5.13 — this module is THE SINGLE SOURCE of simulation labelling for the
// console: every simulated issue, acknowledgement, and tracking result
// renders the SIMULATED label through the constants/components below, and
// acknowledgement is NEVER shown as physical delivery. The workspaces
// (Dispatch, AuditReports, SimulationLab) import these constants — they do
// not keep their own copies (the P5.13 unification rule; enforced by
// honestyStructural.test.ts). The §21.4 control-authority-mode indicator
// also lives here so every workspace renders the SAME indicator.
//
// Rules the primitives encode structurally:
// - Empty/error/loading are explicit states — a workspace never renders a
//   blank region that could be read as "no problems".
// - Freshness is displayed next to the data it qualifies (§20.2): the
//   fetched-at instant of THIS console read; per-record event/ingest times
//   come from the records themselves.
// - Closed vocabularies render through the vocabulary module; a value that
//   is not in the closed set is rendered VERBATIM with an explicit
//   unknown-state marker — never silently mapped to a plausible state.
// - Recorded contract statements (no-execute, simulation-only, append-only)
//   are rendered VERBATIM from the payload — the console never paraphrases
//   them (P5.06 consumption contract).
import type { JSX } from 'react';
import { ApiError } from '../api/client';
import {
  COMMAND_STATE_DEFINITIONS,
  COMMAND_STATE_MARKERS,
  COMMAND_STATES,
  QUALITY_STATES,
  describeAge,
  type CommandState,
} from './stateVocabulary';

/** The explicit loading state (§20.2: honest in-flight state). */
export function LoadingState({ label }: { label: string }): JSX.Element {
  return (
    <p role="status" data-testid="loading-state">
      Loading {label}… (the workspace renders no values until the data
      arrives; nothing here is simulated or placeholder-filled).
    </p>
  );
}

/** The explicit error state — carries the API's own code, never invented text. */
export function ErrorState({ error }: { error: unknown }): JSX.Element {
  if (error instanceof ApiError) {
    return (
      <p role="alert" data-testid="error-state">
        Data unavailable — the API returned {error.status} ({error.code}):
        {error.message ? ` ${error.message}` : ' (no message supplied by the server).'}{' '}
        No values are displayed for the failed request; the console does not
        substitute cached, demo, or fabricated data.
      </p>
    );
  }
  return (
    <p role="alert" data-testid="error-state">
      Data unavailable — the request failed without an API error envelope. No
      values are displayed for the failed request; the console does not
      substitute cached, demo, or fabricated data.
    </p>
  );
}

/** The explicit empty state — the explanation is REQUIRED (forces honesty). */
export function EmptyState({
  title,
  explanation,
}: {
  title: string;
  explanation: string;
}): JSX.Element {
  return (
    <div role="status" data-testid="empty-state">
      <h4>{title}</h4>
      <p>{explanation}</p>
      <p>
        This is an empty state, not a data view: nothing on this panel is
        demo, simulated, or estimated data.
      </p>
    </div>
  );
}

/**
 * The §20.2 freshness stamp for a loaded data set: the instant THIS console
 * read the data, with its age. The workspace shows it next to the data; the
 * records' own event/ingest times are displayed per record (P0.09).
 */
export function FreshnessStamp({
  fetchedAtIso,
}: {
  fetchedAtIso: string;
}): JSX.Element {
  return (
    <p className="freshness" data-testid="freshness-stamp">
      Data fetched at {fetchedAtIso} ({describeAge(fetchedAtIso)}). Freshness
      is the time of this console read (§20.2); each record also shows its
      own event/ingest timestamps where the API carries them.
    </p>
  );
}

/**
 * Renders a recorded contract statement VERBATIM (the no-execute /
 * simulation-only / append-only statements travel IN the payloads; the
 * console renders them, never paraphrases them — P5.06).
 */
export function VerbatimStatement({
  label,
  statement,
}: {
  label: string;
  statement: string;
}): JSX.Element {
  return (
    <blockquote data-testid="verbatim-statement" data-label={label}>
      <strong>{label} (recorded statement, verbatim):</strong>
      <br />
      {statement}
    </blockquote>
  );
}

/**
 * The 14-state lifecycle badge: distinct marker + the EXACT P0.11 spelling.
 * An unknown state string renders verbatim with an explicit unknown marker —
 * it is never mapped onto a plausible state (fail-honest display rule).
 */
export function CommandStateBadge({ state }: { state: string }): JSX.Element {
  if (!(COMMAND_STATES as readonly string[]).includes(state)) {
    return (
      <span data-testid="command-state-unknown">
        [UNKNOWN STATE — displayed verbatim: {state}] (outside the frozen 14-state
        §14.1 vocabulary; the console does not guess)
      </span>
    );
  }
  // P5.13 unification: for the simulator-side states (ISSUED through
  // FALLBACK) the badge carries the CENTRAL SIMULATED qualification —
  // SIMULATED_STATE_MEANINGS, the single source — instead of re-stating the
  // vocabulary definition, so a fetched and a LIVE-updated badge (the
  // P5.25 stream can carry DISPATCH_* events) are labelled identically and
  // the wording exists exactly once (enforced by honestyStructural.test.ts).
  // States before ISSUED keep their recorded definition meaning.
  const simulated = SIMULATED_STATE_MEANINGS[state];
  const definition =
    simulated === undefined
      ? COMMAND_STATE_DEFINITIONS.find((entry) => entry.state === state)
      : undefined;
  return (
    <span data-testid="command-state-badge" data-state={state}>
      {COMMAND_STATE_MARKERS[state as CommandState]} <strong>{state}</strong>
      {definition ? ` — ${definition.meaning}` : ''}
      {simulated !== undefined ? (
        <span role="note" data-testid="command-state-simulated">
          {' '}[{simulated}]
        </span>
      ) : null}
    </span>
  );
}

/**
 * The six-state telemetry-quality badge (§10.4). Unknown values render
 * verbatim with an unknown marker — never mapped.
 */
export function QualityBadge({ quality }: { quality: string }): JSX.Element {
  if (!(QUALITY_STATES as readonly string[]).includes(quality)) {
    return (
      <span data-testid="quality-unknown">
        [UNKNOWN QUALITY — displayed verbatim: {quality}]
      </span>
    );
  }
  return (
    <span data-testid="quality-badge" data-quality={quality}>
      quality: <strong>{quality}</strong>
    </span>
  );
}

// --- P5.12 — demo-data honesty -----------------------------------------------

/**
 * The demo-data honesty statement (P5.12 step 3; CONTEXT.md open question
 * 6 default position): NO demo/dev/synthetic data exists anywhere on this
 * console — every displayed value comes from a tenant-scoped API read of
 * platform records, and fixtures exist ONLY inside test files. This flag
 * is DEFINED FOR FUTURE USE: if a deployment or test harness ever injects
 * demo data into a rendered surface, that surface MUST render
 * <DemoDataBadge /> (or this statement verbatim) next to every demo value,
 * and the value must never be presented as a live measurement. The
 * designPrinciples test pins that no workspace currently renders the badge
 * (no demo data exists) and that the badge is present and correct here.
 */
export const CONSOLE_DEMO_DATA_STATEMENT =
  'DEMO DATA — not a live measurement: everything in this region is ' +
  'demo/synthetic data injected outside the platform APIs (P5.12 §20.2 ' +
  'demo honesty). No demo value is a live, measured, or platform-recorded ' +
  'value.';

/** The demo-data badge for future use (see CONSOLE_DEMO_DATA_STATEMENT). */
export function DemoDataBadge(): JSX.Element {
  return (
    <span className="demo-data" data-testid="demo-data-badge" role="note">
      [{CONSOLE_DEMO_DATA_STATEMENT}]
    </span>
  );
}

// --- P5.13 — the central simulation-honesty contract (§20.2, §21.4, §14.1) ---

/**
 * The Phase 4 handover's simulation-only boundary (P4.32 §2, "Simulation-only
 * boundary"), rendered VERBATIM wherever issue/acknowledgement/tracking data
 * is displayed. Source: `ledger/phase-04-grid-feasibility-dispatch/
 * 32_handover_phase5.md` §2 (REQUIRED handover section — §23 risk row). The
 * console renders this wording, never paraphrases it.
 */
export const SIMULATION_ONLY_BOUNDARY_STATEMENT =
  'Every issued command in this platform is issued to the SIMULATOR and ' +
  'nothing else. SimulatorDispatchAdapter hard-refuses all four ' +
  'non-simulator target kinds (P4.16 guard, re-verified in ' +
  '29_safety_invariant_matrix.md invariant 6). There is no device write ' +
  'path — a structural source scan is pinned in ' +
  'crates/flexcore-application/tests/dispatch_simulator.rs. Acknowledgement ' +
  'is NOT physical delivery; the console (P5.13) must label every simulated ' +
  'issue, acknowledgement, and tracking result as simulated and must never ' +
  'present an acknowledgement as delivery. Operator-approved field control ' +
  'is v1.0.0, not now; real protocol adapters are Stage 6.';

/**
 * The acknowledgement wording (§14.1), from the SAME handover §2 sentence —
 * the exact qualification every surface uses when an ACKNOWLEDGED state or
 * a DispatchAcknowledged record is displayed.
 */
export const ACKNOWLEDGEMENT_NOT_DELIVERY_STATEMENT =
  'Acknowledgement is NOT physical delivery.';

/**
 * The unified per-state SIMULATED qualification (P5.13 wording). This map is
 * THE single source: the Dispatch workspace's `simulatedMeaning` delegates
 * here (its exported strings are byte-identical to the pre-unification
 * strings so the existing vocabulary tests hold). Every state past APPROVED
 * is simulator-side in the 0.x line; acknowledgement is application-layer
 * acceptance, NEVER physical delivery.
 */
export const SIMULATED_STATE_MEANINGS: Readonly<Record<string, string>> = {
  ISSUED:
    'SIMULATED: this command was issued to the SIMULATOR (there is no device write path in the 0.x line).',
  ACKNOWLEDGED:
    'SIMULATED: this acknowledgement is APPLICATION-LAYER ACCEPTANCE by the simulator — it is NOT physical delivery (§14.1).',
  TRACKING:
    "SIMULATED: tracking observes the simulated run's measured response from telemetry; measured values come only from telemetry, never from the command.",
  COMPLETED:
    'SIMULATED: completion means the acceptance criterion was satisfied on the simulated run (one asset command, never a portfolio).',
  FAILED:
    'SIMULATED: the failure trigger fired on the simulated path (NACK/timeout/transport/no-response/tracking trigger).',
  FALLBACK:
    'SIMULATED: the asset/site fallback policy activated on the simulated path.',
};

/**
 * The console-level control-authority mode (§21.4). In the 0.x simulation
 * line the configured mode is Simulation or read-only/shadow ONLY (P5.02);
 * this constant is the console's recorded configuration value — NOT read
 * from user input, NOT from a URL parameter, and NOT from any API (no mode
 * surface exists in this phase; P5.27 reads the mode from deployment +
 * per-site configuration and owns the enforcement). The other §21.4 mode
 * names (read-only/shadow, operator-approved, bounded automation) are
 * configuration vocabulary only; none is claimed by this console.
 */
export const CONSOLE_CONTROL_AUTHORITY_MODE = 'Simulation';

/** Why the mode is a constant in this phase (rendered under the indicator). */
export const CONSOLE_MODE_SOURCE_NOTE =
  'Mode shown from the console\'s recorded configuration constant (the 0.x ' +
  'line is simulation-only, §21.4; no mode-serving API exists in this ' +
  'phase — P5.27 reads the mode from deployment + per-site configuration ' +
  'and enforces it). This value is never taken from user input.';

/**
 * The persistent §21.4 mode indicator (P5.13 step 1): every workspace that
 * displays issue/acknowledgement/tracking data renders this component.
 * Distinct marker + text (not colour alone, P5.12 discipline).
 */
export function ControlAuthorityModeIndicator(): JSX.Element {
  return (
    <p
      className="control-authority-mode"
      role="note"
      data-testid="control-authority-mode-indicator"
      data-mode={CONSOLE_CONTROL_AUTHORITY_MODE}
    >
      <strong>[Control-authority mode: {CONSOLE_CONTROL_AUTHORITY_MODE}]</strong>{' '}
      {CONSOLE_MODE_SOURCE_NOTE}
    </p>
  );
}

/**
 * The SIMULATED label badge (P5.13): the one label every simulated issue,
 * acknowledgement, and tracking result renders. `detail` names WHAT was
 * simulated (e.g. "issue", "acknowledgement", "tracking result"). Used by
 * workspaces that display simulated results OUTSIDE the 14-state badge
 * (run records, decision-chain stages) — the lifecycle badge carries its
 * own per-state qualification from the state vocabulary.
 */
export function SimulatedBadge({ detail }: { detail: string }): JSX.Element {
  return (
    <span data-testid="simulated-badge" role="note">
      <strong>[SIMULATED]</strong> {detail}
    </span>
  );
}

/**
 * The decision-chain stage note for the Audit & Reports workspace (P5.13):
 * the issue / acknowledgement / tracking stages of the §24-step-9 chain are
 * simulator-side in the 0.x line. Rendered once under the chain view; the
 * per-stage entries inherit this qualification (unified wording, not a
 * per-surface copy).
 */
export const SIMULATED_CHAIN_STAGES_NOTE =
  'SIMULATED: the issue, acknowledgement, and tracking stages of this ' +
  'chain are simulator-side in the 0.x line (the P4.16 guard refuses every ' +
  'non-simulator target). The acknowledgement stage is application-layer ' +
  'acceptance by the simulator — acknowledgement is NOT physical delivery ' +
  '(§14.1). No tracking verdict is computed by the console (the P4.16 ' +
  'acceptance-criterion record has no console read endpoint).';

/**
 * The live-stream honesty note (P5.13): the live stream (P5.25) can carry
 * dispatch-lifecycle events (live/types.ts classifies every DISPATCH_* tag
 * as a dispatch event), so a live update may advance a displayed command's
 * ISSUED/ACKNOWLEDGED/TRACKING badge. The label travels IN the badge
 * component (stateVocabulary markers + meanings), so any live-updated
 * state rendered through CommandStateBadge stays labelled — workspaces
 * must never render a live-updated lifecycle state through a different
 * path. Ordering/dedup for live events is P5.33 (not implemented here).
 */
export const LIVE_DISPATCH_LABEL_NOTE =
  'Live dispatch events (DISPATCH_ISSUED, DISPATCH_ACKNOWLEDGED, ' +
  'DISPATCH_TRACKING_UPDATED, …) update the same lifecycle badge: the ' +
  'SIMULATED qualification is carried by the badge component itself, so a ' +
  'live-updated ISSUED/ACKNOWLEDGED/TRACKING state renders labelled exactly ' +
  'as a fetched state does. Workspaces never render live lifecycle state ' +
  'outside this badge.';
