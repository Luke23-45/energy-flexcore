// P5.27 — the console's control-authority MODE gate (§21.4, §17.5), the
// enforcement-layer counterpart of the P5.13 mode INDICATOR and the P5.26
// capability gate.
//
// WHAT THIS MODULE IS:
// - The mode provider: it derives the deployment's control-authority mode
//   (Simulation / Read-only/shadow / Operator-approved / Bounded automation,
//   §21.4) from the console's DEPLOYMENT CONFIGURATION SURFACE — the
//   recorded constant below plus the optional per-site override map. A mode
//   is a deployment and per-site setting (§21.4); it is never taken from
//   user input, a URL parameter, a request field, or a plugin (a plugin can
//   never select or raise the mode — §21.4, §5.7).
// - The mode gate: a pure evaluation that classifies a mirrored action id
//   (read vs mutating) and allows/denies it PER MODE, with the denial REASON
//   rendered — never a bare allow/deny and never a silently hidden control.
// - The intersection combiner: a console action proceeds only when the
//   CAPABILITY check (P5.26 useCan/capabilityMatrix) AND the MODE check both
//   allow. The two gates compose; neither substitutes for the other.
//
// WHAT THIS MODULE IS NOT — the enforcement-honesty record (rendered as
// MODE_ENFORCEMENT_NOTE, never paraphrased):
// - The console mode gate is DEFENCE-IN-DEPTH UI, NOT the enforcement point.
//   The API enforces the control-authority boundary independently: the
//   P4.16 guard (SimulatorDispatchAdapter) hard-refuses every non-simulator
//   target kind, and the console's own REST surface is read-only (§15.2 —
//   the control plane registers no generic execute endpoint, so the console
//   has NO non-simulator issue surface at all). A forged console that shows
//   a button still meets the server's deny. This mirrors the P5.26 criteria
//   wording ("the API enforces it too — the console hiding a button is not
//   the enforcement") and the P5.27 criteria wording ("the API rejects such
//   attempts independently (P4.16 guard)"; "a mode flag in the console that
//   the API ignores" is the recorded risk, mitigated server-side).
// - The mode VALUE has no server source in this phase: a source scan of
//   apps/control-plane at authoring time found NO mode-reading endpoint
//   (no control-authority configuration surface is served), and none was
//   invented. The mode therefore comes from the console's deployment
//   configuration constant below, and the SERVER remains authoritative —
//   the console constant is the defence-in-depth mirror of a deployment
//   setting, never the setting itself.
import { canAction, CAPABILITY_MATRIX, type CapabilityCheck, type ScopeKindId } from './capabilityMatrix';
import { CONSOLE_CONTROL_AUTHORITY_MODE } from '../components/honesty';
import { getSession } from '../state/tenantContext';

// --- The closed §21.4 mode vocabulary -----------------------------------------

/**
 * The four §21.4 control-authority modes, VERBATIM (the vocabulary the
 * ledger and the P5.02/P5.13 records use). Closed — an unknown mode name is
 * a configuration defect, never silently coerced.
 */
export const CONTROL_AUTHORITY_MODES = [
  'Simulation',
  'Read-only/shadow',
  'Operator-approved',
  'Bounded automation',
] as const;

export type ControlAuthorityMode = (typeof CONTROL_AUTHORITY_MODES)[number];

/**
 * The modes the 0.x line EXERCISES (ledger P5.02/P5.27: in this phase the
 * configured mode is Simulation or read-only/shadow only; the other two
 * names are the configuration vocabulary, listed but not exercised).
 */
export const EXERCISED_MODES: readonly ControlAuthorityMode[] = [
  'Simulation',
  'Read-only/shadow',
];

// --- The deployment configuration surface --------------------------------------

/**
 * The deployment's configured control-authority mode — the console's ONLY
 * mode surface in this phase. The control plane serves no mode-reading
 * endpoint (verified by source scan of apps/control-plane at authoring
 * time; none invented), so the mode is deployment configuration: this
 * constant, changed per deployment, never per user, session, URL, request,
 * or plugin. MUST equal the P5.13 display constant
 * (`CONSOLE_CONTROL_AUTHORITY_MODE` in components/honesty.tsx) — the
 * equality is pinned by modeControl.test.tsx; a mismatch is a defect.
 */
export const DEPLOYED_CONTROL_AUTHORITY_MODE: ControlAuthorityMode = 'Simulation';

/**
 * The per-site mode overrides of the deployment (§21.4: mode is a
 * deployment AND per-site setting; a mixed-mode deployment configures
 * differing modes per site). `null` = the deployment is uniform (every
 * site uses `DEPLOYED_CONTROL_AUTHORITY_MODE`). In this phase the recorded
 * configuration is uniform Simulation — the map is null and the mixed-mode
 * DISPLAY path (effectiveMode / MIXED_MODE_NOTE) exists and is tested, so
 * a mixed deployment needs only this constant filled, never new code.
 */
export const DEPLOYMENT_SITE_MODES: Readonly<Record<string, ControlAuthorityMode>> | null = null;

/**
 * The effective mode for a site: the per-site override when the deployment
 * configures one, else the deployment-wide mode. An override naming a value
 * outside the closed vocabulary is a configuration defect — it denies
 * (fail-closed) rather than coercing, and the reason names the defect.
 */
export function effectiveMode(siteId?: string): ControlAuthorityMode | null {
  if (siteId === undefined || DEPLOYMENT_SITE_MODES === null) {
    return DEPLOYED_CONTROL_AUTHORITY_MODE;
  }
  const override = DEPLOYMENT_SITE_MODES[siteId];
  if (override === undefined) {
    return DEPLOYED_CONTROL_AUTHORITY_MODE;
  }
  if (!(CONTROL_AUTHORITY_MODES as readonly string[]).includes(override)) {
    return null; // fail-closed: the caller renders the configuration defect
  }
  return override;
}

/** The mixed-mode display note (§21.4 mixed-mode deployments; P5.27 step 3). */
export const MIXED_MODE_NOTE =
  'The control-authority mode is a deployment AND per-site setting (§21.4): ' +
  'a mixed-mode deployment configures differing modes per site, and the ' +
  'console displays the mode per site where they differ. This deployment ' +
  'configures a UNIFORM mode (no per-site override map is filled): the ' +
  'mode shown applies to every site. A mixed configuration fills ' +
  'DEPLOYMENT_SITE_MODES — the per-site resolution is implemented and ' +
  'tested; no mode is invented for a site that has none.';

// --- The action classification --------------------------------------------------

/**
 * Whether a mirrored action id is MUTATING (creates, updates, deletes,
 * proposes, validates, or approves) versus READ. The classification is
 * DERIVED from the P5.26 capability mirror (security/capabilityMatrix.ts):
 * an id the mirror carries that does not end in `.read` is mutating. The
 * mirror is the console's recorded action surface — the enum is
 * authoritative (the mirror's own drift note applies here too).
 */
export function isMutatingAction(actionId: string): boolean {
  return actionId.length > 0 && !actionId.endsWith('.read');
}

// --- The mode gate --------------------------------------------------------------

export interface ModeGateCheck {
  allowed: boolean;
  /** WHY (the mode rule + the honest enforcement boundary; rendered). */
  reason: string;
  /** The mode the check evaluated under (resolved per site). */
  mode: ControlAuthorityMode | null;
  /** Whether the resolved mode is one the 0.x console exercises. */
  exercised: boolean;
}

/**
 * The pure mode gate for one action id. Rules (ledger P5.27 steps 1–2, the
 * §21.4 mode semantics from the phase CONTEXT):
 *
 * - An action id the capability mirror does not carry DENIES (deny-by-
 *   default, P1.11 — the same rule the capability gate applies; the API
 *   would deny it with UnknownAction regardless).
 * - READ actions are permitted in every mode (a mode constrains control
 *   authority — issuing/acting — never reading evidence).
 * - MUTATING actions under **Simulation**: allowed, with the SIMULATED
 *   qualification — every such action acts on the simulator only (the
 *   console has NO non-simulator issue surface at all: the §15.2 control-
 *   plane surface is read-only and the P4.16 guard refuses every
 *   non-simulator target), and every result is labelled simulated.
 * - MUTATING actions under **Read-only/shadow**: DENIED with the
 *   explanation (shadow computes and compares proposed actions against
 *   measurements WITHOUT issuing commands — §21.4; the API rejects such
 *   attempts independently, P4.16 guard).
 * - MUTATING actions under **Operator-approved** / **Bounded automation**:
 *   DENIED as configured-but-not-exercised — the names are the §21.4
 *   configuration vocabulary only; the entire 0.x line is simulation-only;
 *   no mode promotion is in scope for this phase (promotion requires the
 *   §22.3 release gates plus site-specific acceptance — recorded, not
 *   available).
 * - A resolved mode outside the closed vocabulary (a configuration
 *   defect): DENY everything mutating, with the defect named (fail-closed).
 */
export function gateActionForMode(
  actionId: string,
  options: { siteId?: string; modeOverride?: ControlAuthorityMode } = {},
): ModeGateCheck {
  const mode = options.modeOverride ?? effectiveMode(options.siteId);
  if (mode === null || !(CONTROL_AUTHORITY_MODES as readonly string[]).includes(mode)) {
    return {
      allowed: false,
      reason:
        `the deployment's control-authority mode configuration is defective ` +
        `(resolved value '${String(mode)}' is outside the closed §21.4 mode ` +
        `vocabulary ${CONTROL_AUTHORITY_MODES.join(' / ')}) — every mutating ` +
        `action is denied fail-closed until the configuration is corrected`,
      mode,
      exercised: false,
    };
  }
  const exercised = (EXERCISED_MODES as readonly string[]).includes(mode);
  const modeClause = `Control-authority mode: ${mode} (deployment configuration; never user input, never plugin-selected — §21.4, §5.7).`;
  // Deny-by-default on the mirrored action surface (P1.11): an id the
  // capability mirror does not carry is unknown to the console — the gate
  // denies and says so (the API would deny it with UnknownAction anyway).
  if (!(actionId in CAPABILITY_MATRIX)) {
    return {
      allowed: false,
      reason:
        `${modeClause} action '${actionId}' is not on the mirrored action ` +
        `surface — deny-by-default (an unknown action denies, P1.11); it ` +
        `may be a §5.7-excluded name or a drift, and the API would deny it ` +
        `with UnknownAction regardless`,
      mode,
      exercised,
    };
  }
  if (!isMutatingAction(actionId)) {
    return {
      allowed: true,
      reason: `${modeClause} Reading evidence is permitted in every mode; the read is tenant-scoped and API-enforced (P5.26).`,
      mode,
      exercised,
    };
  }
  if (mode === 'Simulation') {
    return {
      allowed: true,
      reason:
        `${modeClause} The action is available ONLY against the simulator: the ` +
        `console has no non-simulator issue surface at all (the §15.2 ` +
        `control-plane surface is read-only — no generic execute endpoint), ` +
        `and the P4.16 guard (SimulatorDispatchAdapter) hard-refuses every ` +
        `non-simulator target, so an out-of-simulator issue is impossible ` +
        `here and API-rejected independently (P5.27 step 2; the console ` +
        `disable is defence-in-depth UI, not the enforcement). Every result ` +
        `of the action is labelled SIMULATED.`,
      mode,
      exercised,
    };
  }
  if (mode === 'Read-only/shadow') {
    return {
      allowed: false,
      reason:
        `${modeClause} In Read-only/shadow mode proposed actions are computed ` +
        `and compared against measurements WITHOUT issuing commands (§21.4), ` +
        `so every mutating console action is DISABLED with this explanation — ` +
        `not silently hidden. The API rejects such attempts independently ` +
        `(the P4.16 guard refuses every non-simulator target; the console ` +
        `gate is defence-in-depth UI, NOT the enforcement — P5.26/P5.27 ` +
        `criteria wording).`,
      mode,
      exercised,
    };
  }
  // Operator-approved / Bounded automation: the configuration vocabulary,
  // not exercised by the 0.x console.
  return {
    allowed: false,
    reason:
      `${modeClause} This mode is part of the §21.4 configuration vocabulary ` +
      `ONLY: the entire 0.x line is simulation-only and the v0.3.0 console ` +
      `does not exercise this mode (configured-but-not-exercised, displayed ` +
      `honestly). Mutating actions are denied. No mode promotion is in scope ` +
      `for this phase — promotion requires the §22.3 release gates plus ` +
      `site-specific acceptance, and none is recorded.`,
    mode,
    exercised,
  };
}

// --- The intersection with the capability gate ----------------------------------

/**
 * The intersection combiner: an action proceeds only when BOTH the
 * capability check (P5.26) and the mode check (P5.27) allow. BOTH denial
 * reasons render (a denial is never a bare allow/deny), and an allow
 * carries both qualifications (the capability mirror's API-enforcement
 * note and the mode's SIMULATED / boundary note).
 */
export function combineChecks(
  capability: CapabilityCheck,
  mode: ModeGateCheck,
): { allowed: boolean; reason: string } {
  const allowed = capability.allowed && mode.allowed;
  const reason = `${capability.reason} ${mode.reason}`;
  return { allowed, reason };
}

/**
 * The combined gate hook: capability (P5.26 useCan semantics — the
 * claims-derived session against the grant mirror) AND mode (P5.27) must
 * BOTH allow. Reads the session store on render exactly as useCan does
 * (the same recorded non-reactivity limitation applies).
 */
export function useCanInMode(
  actionId: string,
  scopeKind?: ScopeKindId,
  siteId?: string,
): { allowed: boolean; reason: string; mode: ControlAuthorityMode | null; exercised: boolean } {
  const session = getSession();
  const capability = canAction(actionId, session ? session.role : null, scopeKind);
  const modeCheck = gateActionForMode(actionId, { siteId });
  return {
    ...combineChecks(capability, modeCheck),
    mode: modeCheck.mode,
    exercised: modeCheck.exercised,
  };
}

// --- The honesty notes (rendered verbatim, never paraphrased) -------------------

/** The enforcement-boundary record (module header, rendered where the gate
 * is displayed). */
export const MODE_ENFORCEMENT_NOTE =
  'Control-authority mode enforcement (P5.27): the mode comes from this ' +
  'console\'s DEPLOYMENT configuration (no mode-serving API exists on the ' +
  'control plane in this phase — none was invented), and the SERVER remains ' +
  'the enforcement point: the P4.16 guard refuses every non-simulator ' +
  'target and the console\'s REST surface is read-only (§15.2). This ' +
  'console gate is DEFENCE-IN-DEPTH UI — disabling an action here is not ' +
  'the enforcement, and the API rejects attempts independently (P5.26 ' +
  'invariant-7 discipline applied to the UI; P5.27 step 2). The P4.32 §2 ' +
  'boundary applies verbatim: every issued command is issued to the ' +
  'SIMULATOR and nothing else; acknowledgement is NOT physical delivery; ' +
  'operator-approved field control is v1.0.0, not now.';

/**
 * Why the two Operator-approved / Bounded-automation modes render as
 * configured-but-not-exercised (rendered with the mode display).
 */
export const NON_EXERCISED_MODES_NOTE =
  'Operator-approved and Bounded automation are the §21.4 configuration ' +
  'vocabulary, not modes this console exercises: the entire 0.x line is ' +
  'simulation-only, the v0.3.0 release ladder grants no field control ' +
  'before v1.0.0, and mode promotion requires the §22.3 release gates plus ' +
  'site-specific acceptance — no promotion path exists in this phase ' +
  '(P5.27 step 4). The modes are listed so the configuration vocabulary is ' +
  'visible; listing one is never a claim that the console can operate in it.';
