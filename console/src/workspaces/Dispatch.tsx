// P5.06 — the Dispatch workspace (report §20.1 item 7; §14.1–§14.2; the
// P4.30 read-only evidence surfaces). Replaces the P5.01 stub (the stub's
// honest-empty discipline carries over).
//
// The rules this file ENCODES STRUCTURALLY (they are tested, not styled):
//
// 1. The 14-state lifecycle is displayed with the P0.11 vocabulary EXACTLY
//    (components/stateVocabulary.ts COMMAND_STATES; an out-of-vocabulary
//    state renders verbatim with an unknown marker, never mapped).
// 2. Validation, feasibility, and authorization evidence is shown as THREE
//    DISTINCT evidence blocks (§14.1: the gates are distinct) — never merged
//    into one "gate result".
// 3. No approval is performed from the console. routes_phase4.rs exposes
//    READ-ONLY dispatch-command resources only — there is NO transition
//    endpoint the console could call, and the console never invents one
//    (§15.2: no generic execute endpoint; the gates are application-layer).
//    The approval flow therefore (a) assembles and displays the approval
//    parameters with explicit confirmation (P5.06 step 4 shape: actor,
//    scope, limits, expiry), and (b) on confirmation resolves to the
//    MISSING-ENDPOINT GAP result — the state stays read-only and the gap is
//    recorded, not papered over.
// 4. Every simulated issue/acknowledgement/tracking result is labelled
//    simulated (P5.13 wording) through the CENTRAL honesty module
//    (components/honesty.tsx — the single source; this workspace keeps no
//    copy of the wording), and acknowledgement is NEVER shown as physical
//    delivery — the ISSUED/ACKNOWLEDGED/TRACKING qualifications live in the
//    honesty module's SIMULATED_STATE_MEANINGS + the state vocabulary.
// 5. The payload's no-execute and simulation-only statements are rendered
//    VERBATIM (the console never paraphrases them), alongside the Phase 4
//    handover's simulation-only boundary statement (P4.32 §2) and the
//    persistent §21.4 control-authority-mode indicator (P5.13).
// 6. Emergency actions: §20.2 requires emergency and normal scheduling to
//    be visually separate, but NO emergency action is defined for this
//    phase — recorded as an OPEN ITEM (ledger P5.06 step 5), not a feature;
//    this console renders no emergency control and invents none.
// 7. P5.26 — the approval action is capability-gated (useCan over the
//    recorded server grant mirror: command.approve is tenant-admin at
//    site/asset scope only, §17.3); a denial renders the REASON instead of
//    a broken button. The API enforces independently — hiding is not the
//    enforcement. After explicit confirmation (ConfirmHighImpact, P5.11),
//    the flow requires the STEP-UP panel (components/stepUpApproval.tsx):
//    typed target confirmation + fresh dev-session re-validation. No
//    approval is performed from the console regardless (rule 3).
import { useEffect, useState } from 'react';
import type { JSX } from 'react';
import { api, type ApiClient } from '../api/client';
import {
  getDispatchCommand,
  listDispatchCommands,
  listFeasibilityChecks,
} from '../api/dispatch';
import type {
  CommandTransitionDto,
  DispatchCommandDto,
  DispatchCommandDetailDto,
  FeasibilityCheckDto,
} from '../api/types';
import {
  ACKNOWLEDGEMENT_NOT_DELIVERY_STATEMENT,
  ControlAuthorityModeIndicator,
  EmptyState,
  ErrorState,
  FreshnessStamp,
  LIVE_DISPATCH_LABEL_NOTE,
  LoadingState,
  SIMULATED_STATE_MEANINGS,
  SIMULATION_ONLY_BOUNDARY_STATEMENT,
  VerbatimStatement,
  CommandStateBadge,
} from '../components/honesty';
import { ConfirmHighImpact } from '../components/ConfirmHighImpact';
import { COMMAND_STATES, formatUtcMillis } from '../components/stateVocabulary';
import {
  evaluateStepUp,
  recordStepUpDenial,
  StepUpApproval,
} from '../components/stepUpApproval';
import { useCan, type CapabilityCheck } from '../security/useCan';
import { combineChecks, gateActionForMode } from '../security/modeControl';
import { RoleScopeLine } from '../components/workspaceData';
import { getSession, type SessionClaims } from '../state/tenantContext';

/** The mirrored action id of the authorization gate (§14.1/P4.13) — the
 * only approval action on the closed surface; the console gate is UX only. */
const APPROVAL_ACTION_ID = 'command.approve';

// --- Data loading (pure, exported for tests) ---------------------------------

export interface CommandListData {
  commands: DispatchCommandDto[];
  fetchedAtIso: string;
  truncated: boolean;
}

/** Loads the first page of dispatch commands (optionally one state filter). */
export async function loadCommandList(
  client: ApiClient,
  state?: string,
): Promise<CommandListData> {
  // Wire object built without undefined values (exactOptionalPropertyTypes —
  // no explicit undefined into an optional prop); the cast only restores the
  // server DTO's optional-prop shape.
  const wire = { limit: 200 } as Record<string, string | number>;
  if (state !== undefined) wire.state = state;
  const page = await listDispatchCommands(
    wire as unknown as Parameters<typeof listDispatchCommands>[0],
    client,
  );
  return {
    commands: page.data,
    fetchedAtIso: new Date().toISOString(),
    truncated: page.next_cursor !== null && page.next_cursor !== undefined,
  };
}

export interface CommandDetailData {
  detail: DispatchCommandDetailDto;
  checks: FeasibilityCheckDto[];
  fetchedAtIso: string;
}

/** Loads one command with its full journal and its feasibility-check records. */
export async function loadCommandDetail(
  client: ApiClient,
  commandId: string,
): Promise<CommandDetailData> {
  const detailEnvelope = await getDispatchCommand(commandId, client);
  const checksPage = await listFeasibilityChecks({ commandId, limit: 200 }, client);
  return {
    detail: detailEnvelope.data,
    checks: checksPage.data,
    fetchedAtIso: new Date().toISOString(),
  };
}

// --- Pure mappers -------------------------------------------------------------

export interface GateEvidence {
  gate: string;
  fromState: string;
  toState: string;
  actor: string;
  atUnixMillis: number;
  evidenceReference: string | null;
  reason: string | null;
}

/**
 * Extracts the THREE gate evidences from the journal as three DISTINCT
 * entries (§14.1). The journal carries `gate` exactly on the gate passes
 * and gate rejections; anything else is left out — the console never
 * merges the gates into a combined verdict.
 */
export function gateEvidence(journal: CommandTransitionDto[]): GateEvidence[] {
  const gates: GateEvidence[] = [];
  for (const entry of journal) {
    if (entry.gate === undefined) continue;
    gates.push({
      gate: entry.gate,
      fromState: entry.from_state,
      toState: entry.to_state,
      actor: entry.actor,
      atUnixMillis: entry.at_unix_millis,
      evidenceReference: entry.evidence_reference ?? null,
      reason: entry.reason ?? null,
    });
  }
  return gates;
}

export interface RejectionView {
  gate: string | null;
  failingCheck: string | null;
  evidenceReference: string | null;
  matchingCheck: FeasibilityCheckDto | null;
}

/**
 * Builds the rejection explanation for a REJECTED command: WHICH gate
 * rejected, the failing check (the journal reason), and the evidence
 * reference — plus, when a feasibility-check record exists for the command,
 * the check's own verdict fields (LIMIT_VIOLATED failing constraint /
 * FAIL_CLOSED reason). Rejection evidence stays gate-specific.
 */
export function rejectionView(
  commandState: string,
  journal: CommandTransitionDto[],
  checks: FeasibilityCheckDto[],
): RejectionView {
  if (commandState !== 'REJECTED') {
    return { gate: null, failingCheck: null, evidenceReference: null, matchingCheck: null };
  }
  const rejectionEntry = [...journal]
    .reverse()
    .find((entry) => entry.to_state === 'REJECTED');
  const gate = rejectionEntry?.gate ?? null;
  const matchingCheck =
    checks.find((check) => check.outcome !== 'PASSED') ?? null;
  return {
    gate,
    failingCheck: rejectionEntry?.reason ?? null,
    evidenceReference: rejectionEntry?.evidence_reference ?? null,
    matchingCheck,
  };
}

export interface ApprovalParameters {
  actor: string;
  scope: {
    tenantId: string;
    portfolioId: string;
    siteId: string;
    assetId: string;
    scheduleId: string;
    scheduleRevision: number;
  };
  limits: string;
  expiryUtc: string;
}

/**
 * Assembles the approval parameters the confirmation flow displays (P5.06
 * step 4: actor, scope, limits, expiry). The actor comes from the
 * claims-derived session (never free text typed into a spoofable field);
 * limits are read ONLY from the command payload when it carries a
 * `limits` field — otherwise the console says so instead of inventing
 * limit figures.
 */
export function approvalParameters(
  command: DispatchCommandDto,
  claims: SessionClaims | null,
): ApprovalParameters {
  let limits =
    'not displayed — the command payload on this surface does not carry a dedicated limits field; the console does not invent limit figures';
  if (command.command_payload !== null && typeof command.command_payload === 'object') {
    const payloadLimits = (command.command_payload as Record<string, unknown>).limits;
    if (payloadLimits !== undefined && payloadLimits !== null) {
      limits = `as recorded in the command payload: ${JSON.stringify(payloadLimits)}`;
    }
  }
  return {
    actor: claims ? `${claims.subject} (role: ${claims.role}, tenant: ${claims.tenantId})` : 'no authenticated session (P5.26 wires the real session)',
    scope: {
      tenantId: command.tenant_id,
      portfolioId: command.portfolio_id,
      siteId: command.site_id,
      assetId: command.asset_id,
      scheduleId: command.schedule_id,
      scheduleRevision: command.schedule_revision,
    },
    limits,
    expiryUtc: formatUtcMillis(command.valid_until_millis),
  };
}

export type ApprovalResolution =
  | { kind: 'gap'; detail: string }
  | { kind: 'not-approvable-state'; detail: string };

/**
 * Resolves what the console CAN do for an approval request. The §14.1
 * authorization gate is an application-layer gate; routes_phase4.rs exposes
 * NO transition endpoint, so the resolution is always the recorded gap (or
 * a not-in-this-state note for commands the gate could not apply to). The
 * console never invents a POST and never bypasses the gates.
 */
export function resolveApprovalAction(command: DispatchCommandDto): ApprovalResolution {
  if (command.state !== 'FEASIBLE') {
    return {
      kind: 'not-approvable-state',
      detail: `The authorization gate applies only from FEASIBLE (§14.1); this command is ${command.state}. The console shows the state read-only.`,
    };
  }
  return {
    kind: 'gap',
    detail:
      'MISSING-ENDPOINT GAP (recorded): routes_phase4.rs exposes READ-ONLY dispatch-command resources only — GET /api/v1/dispatch-commands (+/{command_id}). There is no transition endpoint for the authorization gate (or any gate) on this API surface, and the console does not invent one. The approval parameters above were displayed and confirmed, but NO approval was performed from the console; the command state stays read-only and the gate remains the application-layer authorization action. See 05_09_workspaces_b.md.',
  };
}

/**
 * The simulated-issue qualification (P5.13 wording) — DELEGATED to the
 * central honesty module (SIMULATED_STATE_MEANINGS is the single source;
 * this workspace keeps no copy, the P5.13 unification rule). Every state
 * past APPROVED is simulator-side in the 0.x line; acknowledgement is
 * application-layer acceptance, NEVER physical delivery.
 */
export function simulatedMeaning(state: string): string {
  return SIMULATED_STATE_MEANINGS[state] ?? '';
}

// --- Presentational components -----------------------------------------------

export function CommandListItem({ command }: { command: DispatchCommandDto }): JSX.Element {
  return (
    <li data-testid="command-list-item" data-state={command.state}>
      <CommandStateBadge state={command.state} />
      <br />
      <span>
        command {command.command_id} | asset {command.asset_id} | site {command.site_id} |
        schedule {command.schedule_id} rev {command.schedule_revision} | valid until{' '}
        {formatUtcMillis(command.valid_until_millis)} (end-exclusive)
      </span>
      {command.supersedes_command_id !== null && command.supersedes_command_id !== undefined ? (
        <span> | supersedes {command.supersedes_command_id}</span>
      ) : null}
      {command.superseded_by_command_id !== null && command.superseded_by_command_id !== undefined ? (
        <span> | superseded by {command.superseded_by_command_id}</span>
      ) : null}
    </li>
  );
}

export function GateEvidencePanel({ evidence }: { evidence: GateEvidence[] }): JSX.Element {
  const order = ['validation', 'feasibility', 'authorization'];
  const sorted = [...evidence].sort((a, b) => order.indexOf(a.gate) - order.indexOf(b.gate));
  return (
    <div data-testid="gate-evidence-panel">
      <h4>Gate evidence — three DISTINCT gates, shown separately (§14.1; never merged)</h4>
      {sorted.length === 0 ? (
        <p>No gate transition is recorded in the loaded journal yet.</p>
      ) : (
        <ol>
          {sorted.map((entry, index) => (
            <li key={`${entry.gate}:${index}`} data-testid={`gate-evidence-${entry.gate}`}>
              <strong>{entry.gate} gate</strong>: {entry.fromState} → {entry.to_state} | actor{' '}
              {entry.actor} | at {formatUtcMillis(entry.atUnixMillis)} | evidence:{' '}
              {entry.evidenceReference ?? '(no evidence reference recorded)'}
              {entry.reason ? ` | reason: ${entry.reason}` : ''}
            </li>
          ))}
        </ol>
      )}
      <p>
        Validation, feasibility, and authorization are DISTINCT gates: passing one
        never implies another, and their evidence references are never combined
        into a single verdict on this console.
      </p>
    </div>
  );
}

export function RejectionPanel({ rejection }: { rejection: RejectionView }): JSX.Element {
  if (rejection.gate === null && rejection.failingCheck === null && rejection.matchingCheck === null) {
    return null;
  }
  return (
    <div data-testid="rejection-panel">
      <h4>Rejection explanation (failing constraint + evidence, §20.2)</h4>
      <ul>
        <li>Rejecting gate: {rejection.gate ?? '(not recorded in the loaded journal)'}</li>
        <li>Failing check: {rejection.failingCheck ?? '(not recorded in the loaded journal)'}</li>
        <li>Evidence reference: {rejection.evidenceReference ?? '(not recorded)'}</li>
        {rejection.matchingCheck ? (
          <>
            <li>
              Feasibility-check record {rejection.matchingCheck.check_id}: outcome{' '}
              {rejection.matchingCheck.outcome}
              {rejection.matchingCheck.failing_constraint
                ? ` | failing constraint: ${rejection.matchingCheck.failing_constraint}`
                : ''}
              {rejection.matchingCheck.reason
                ? ` | fail-closed reason: ${rejection.matchingCheck.reason}`
                : ''}
            </li>
            <li>
              Check evidence links: model {rejection.matchingCheck.model_id ?? '(none)'} version{' '}
              {rejection.matchingCheck.model_version ?? '(none)'} | checked at{' '}
              {formatUtcMillis(rejection.matchingCheck.checked_at_unix_millis)} (§20.2 via P5.11:
              model version AND timestamp travel with every feasibility outcome, here on the
              rejection evidence too) | correlation id{' '}
              {rejection.matchingCheck.correlation_id}
            </li>
          </>
        ) : (
          <li>
            No feasibility-check record was returned for this command by
            GET /api/v1/feasibility-checks — none is invented.
          </li>
        )}
      </ul>
    </div>
  );
}

export function ApprovalFlow({
  command,
  claims,
  confirmed,
  onConfirm,
  capability,
  stepUpSatisfied,
}: {
  command: DispatchCommandDto;
  claims: SessionClaims | null;
  confirmed: boolean;
  onConfirm: () => void;
  /**
   * Optional capability-check override (tests / a precomputed check). When
   * omitted, the check is derived from the claims-derived session via
   * useCan('command.approve') against the recorded grant mirror. When the
   * check denies, the flow renders the REASON — never a broken button.
   */
  capability?: CapabilityCheck;
  /**
   * Optional step-up override (tests / a parent-driven flow): when `true`,
   * the step-up requirements are treated as satisfied and the flow resolves
   * directly. The app container does NOT pass it — the interactive step-up
   * panel applies in the console.
   */
  stepUpSatisfied?: boolean;
}): JSX.Element {
  // The capability gate runs on every render (rules of hooks): the role
  // comes from the claims-derived session store — never from user input.
  const hookCheck = useCan(APPROVAL_ACTION_ID);
  // P5.27 — the §21.4 control-authority MODE gate composes WITH the
  // capability gate (intersection: BOTH must allow; both denial reasons
  // render). The mode comes from the deployment configuration
  // (security/modeControl.ts); the API still enforces independently —
  // this composition is defence-in-depth UI, not the enforcement.
  const modeCheck = gateActionForMode(APPROVAL_ACTION_ID);
  const check = combineChecks(capability ?? hookCheck, modeCheck);
  const parameters = approvalParameters(command, claims);
  const resolution = resolveApprovalAction(command);
  const [stepUpTyped, setStepUpTyped] = useState('');
  const [stepUpToken, setStepUpToken] = useState('');
  const stepUp = evaluateStepUp({
    confirmTargetId: command.command_id,
    typedValue: stepUpTyped,
    claims,
    tokenValue: stepUpToken,
  });
  const stepUpEffective = stepUpSatisfied === true || stepUp.satisfied;
  const role = claims ? claims.role : '(no session)';
  function handleTyped(value: string): void {
    setStepUpTyped(value);
    if (value.trim() !== command.command_id) {
      recordStepUpDenial(
        'Dispatch ApprovalFlow (typed confirmation)',
        role,
        `typed confirmation does not match the action target (${command.command_id})`,
      );
    }
  }
  function handleToken(value: string): void {
    setStepUpToken(value);
    const attempt = evaluateStepUp({
      confirmTargetId: command.command_id,
      typedValue: stepUpTyped,
      claims,
      tokenValue: value,
    });
    if (!attempt.reauthOk) {
      recordStepUpDenial('Dispatch ApprovalFlow (re-authentication)', role, attempt.reauthReason);
    }
  }
  if (!check.allowed) {
    // P5.26 step 2: a denied action is hidden WITH its explanation — never a
    // broken button. The API enforces authorization independently (P4.29
    // invariant-7 discipline applied to the UI); hiding is NOT the
    // enforcement, and the denial is audited SERVER-side (§16.2, P4.28) —
    // recorded locally only (security/denialLog.ts).
    return (
      <div data-testid="approval-flow">
        <h4>Approval (authorization gate)</h4>
        <div data-testid="capability-denial" role="alert">
          <p>
            <strong>
              Action not available: {APPROVAL_ACTION_ID} is denied for this
              session.
            </strong>
          </p>
          <p>{check.reason}</p>
          <p role="note">
            The console hides the action with this explanation instead of
            rendering a broken button. The API enforces authorization
            independently on every action — hiding the button is NOT the
            enforcement (P4.29 invariant-7 discipline applied to the UI).
            Denials are audited SERVER-side (§16.2; P4.28
            TenantDenialAuditRecord / SecurityAuditSink); this denial is
            recorded locally only (security/denialLog.ts) and never sent
            anywhere (no such endpoint exists; none invented).
          </p>
        </div>
      </div>
    );
  }
  return (
    <div data-testid="approval-flow">
      <h4>Approval (authorization gate) — explicit confirmation + step-up required (§20.2, §17.3)</h4>
      <ul data-testid="approval-parameters">
        <li>Actor: {parameters.actor} (from the authenticated claims — never free text)</li>
        <li>
          Scope: tenant {parameters.scope.tenantId} | portfolio {parameters.scope.portfolioId} |
          site {parameters.scope.siteId} | asset {parameters.scope.assetId} | schedule{' '}
          {parameters.scope.scheduleId} rev {parameters.scope.scheduleRevision}
        </li>
        <li>Limits: {parameters.limits}</li>
        <li>Expiry: {parameters.expiryUtc} (the §14.3 valid-until bound)</li>
      </ul>
      {/* P5.11: the confirmation step is the SHARED high-impact confirmation
          component (components/ConfirmHighImpact.tsx) — the approval is the
          one console-triggerable action on the recorded phase list
          (HIGH_IMPACT_ACTIONS); cancellation/supersede/fallback are NOT
          console-triggerable and the list says why. P5.26 layers the STEP-UP
          requirements (components/stepUpApproval.tsx) between confirmation
          and resolution: typed target confirmation + fresh dev-session
          re-validation (honest record inside the panel). */}
      <ConfirmHighImpact
        actionId="approval"
        subject={`command ${command.command_id} (asset ${command.asset_id}, site ${command.site_id})`}
        summary="Approving passes the §14.1 AUTHORIZATION gate: it records actor, scope, limits, and expiry (P4.13). Read the parameters above before confirming."
        confirmed={confirmed}
        onConfirm={onConfirm}
      >
        {stepUpEffective ? (
          <div data-testid={`approval-resolution-${resolution.kind}`}>
            {resolution.kind === 'gap' ? (
              <p role="alert">{resolution.detail}</p>
            ) : (
              <p role="status">{resolution.detail}</p>
            )}
          </div>
        ) : (
          <StepUpApproval
            actionLabel="approval — passing the §14.1 authorization gate"
            confirmTargetId={command.command_id}
            typedValue={stepUpTyped}
            tokenValue={stepUpToken}
            claims={claims}
            onTypedChange={handleTyped}
            onTokenChange={handleToken}
            check={stepUp}
          />
        )}
      </ConfirmHighImpact>
    </div>
  );
}

export function CommandDetailPanel({
  data,
  claims,
}: {
  data: CommandDetailData;
  claims: SessionClaims | null;
}): JSX.Element {
  const { command, journal } = data.detail;
  const evidence = gateEvidence(journal);
  const rejection = rejectionView(command.state, journal, data.checks);
  const simulated = simulatedMeaning(command.state);
  const [confirmed, setConfirmed] = useState(false);
  return (
    <div data-testid="command-detail">
      <h3>Command {command.command_id}</h3>
      {/* P5.10: the detail view's freshness anchor (§20.2) — the instant this
          console read the command + journal + check records. */}
      <FreshnessStamp fetchedAtIso={data.fetchedAtIso} />
      <CommandStateBadge state={command.state} />
      {simulated !== '' ? (
        <p data-testid="simulated-meaning" role="note">
          {simulated}
        </p>
      ) : null}
      <VerbatimStatement label="no-execute-endpoint" statement={command.no_execute_endpoint_statement} />
      <VerbatimStatement label="simulation-only boundary" statement={command.simulation_only_statement} />
      <GateEvidencePanel evidence={evidence} />
      <RejectionPanel rejection={rejection} />
      {command.state === 'FEASIBLE' || command.state === 'APPROVED' ? (
        <ApprovalFlow
          command={command}
          claims={claims}
          confirmed={confirmed}
          onConfirm={() => setConfirmed(true)}
        />
      ) : (
        <p>
          The approval action applies to FEASIBLE commands (the authorization gate);
          this command is {command.state} and is shown read-only.
        </p>
      )}
      <h4>Transition journal (immutable, in order)</h4>
      {journal.length === 0 ? (
        <p>No transition is journalled yet (the command is PROPOSED; the initial state is not a journalled transition).</p>
      ) : (
        <ol data-testid="transition-journal">
          {journal.map((entry) => (
            <li key={entry.seq}>
              seq {entry.seq}: {entry.from_state} → {entry.to_state} | actor {entry.actor} | at{' '}
              {formatUtcMillis(entry.at_unix_millis)}
              {entry.gate ? ` | gate: ${entry.gate}` : ''}
              {entry.reason ? ` | reason: ${entry.reason}` : ''}
              {entry.evidence_reference ? ` | evidence: ${entry.evidence_reference}` : ''}
            </li>
          ))}
        </ol>
      )}
      <h4>Command payload (as persisted — verbatim)</h4>
      <pre data-testid="command-payload">{JSON.stringify(command.command_payload, null, 2)}</pre>
    </div>
  );
}

// --- The workspace container ---------------------------------------------------

interface DispatchState {
  list: CommandListData | null;
  listStatus: 'loading' | 'loaded' | 'error';
  listError?: unknown;
  stateFilter: string;
  selectedCommandId: string | undefined;
  detail: CommandDetailData | null;
  detailStatus: 'idle' | 'loading' | 'loaded' | 'error';
  detailError?: unknown;
}

const STATE_FILTER_OPTIONS: readonly string[] = COMMAND_STATES;

export function DispatchWorkspace({
  client,
}: {
  client?: ApiClient | undefined;
}): JSX.Element {
  const apiClient = client ?? api;
  const [state, setState] = useState<DispatchState>({
    list: null,
    listStatus: 'loading',
    stateFilter: '',
    selectedCommandId: undefined,
    detail: null,
    detailStatus: 'idle',
  });
  const claims = getSession();

  useEffect(() => {
    let cancelled = false;
    async function run(): Promise<void> {
      try {
        const list = await loadCommandList(apiClient, state.stateFilter === '' ? undefined : state.stateFilter);
        if (!cancelled) setState((prev) => ({ ...prev, list, listStatus: 'loaded' }));
      } catch (error) {
        if (!cancelled) setState((prev) => ({ ...prev, listStatus: 'error', listError: error }));
      }
    }
    void run();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state.stateFilter]);

  useEffect(() => {
    if (state.selectedCommandId === undefined) return;
    let cancelled = false;
    setState((prev) => ({ ...prev, detailStatus: 'loading', detail: null }));
    async function run(): Promise<void> {
      try {
        const detail = await loadCommandDetail(apiClient, state.selectedCommandId as string);
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
  }, [state.selectedCommandId]);

  return (
    <section aria-label="Dispatch">
      <h2>Dispatch</h2>
      {/* P5.13 step 1 — the persistent §21.4 control-authority-mode indicator
          (one component, shared by every workspace; see honesty.tsx). */}
      <ControlAuthorityModeIndicator />
      <p>
        Proposed commands, validation/feasibility/authorization evidence, and the
        issue/acknowledgement/tracking lifecycle (§20.1 item 7, §14.1). Every
        dispatch in the 0.x line targets the SIMULATOR; acknowledgement is
        application-layer acceptance, NEVER physical delivery.
      </p>
      {/* P5.13 — the shared acknowledgement qualification (§14.1), from the
          central honesty module (one constant, not a per-surface copy). */}
      <p role="note" data-testid="ack-not-delivery">
        {ACKNOWLEDGEMENT_NOT_DELIVERY_STATEMENT}
      </p>
      {/* P5.13 — the Phase 4 handover's simulation-only boundary (P4.32 §2),
          rendered VERBATIM (the console never paraphrases it). */}
      <VerbatimStatement
        label="simulation-only boundary (P4.32 handover §2)"
        statement={SIMULATION_ONLY_BOUNDARY_STATEMENT}
      />
      {/* P5.13 — the live-stream honesty note: a live dispatch event updates
          the same lifecycle badge, so live-updated ISSUED/ACKNOWLEDGED/
          TRACKING state stays labelled. */}
      <p role="note" data-testid="live-dispatch-label-note">
        {LIVE_DISPATCH_LABEL_NOTE}
      </p>
      {/* P5.12 (extends the P5.03 role-line pattern to wave-2b workspaces):
          the claims-derived role line, so every workspace states the session
          role the visibility is enforced against. */}
      <RoleScopeLine />
      <p role="note" data-testid="emergency-open-item">
        Emergency actions: §20.2 requires emergency and normal scheduling to be
        visually separate, but no emergency action is defined for this phase — this
        is recorded as an OPEN ITEM (ledger P5.06 step 5), not a feature. This
        console renders no emergency control and invents none.
      </p>
      <label>
        Filter by lifecycle state (the closed 14-state vocabulary):{' '}
        <select
          data-testid="state-filter"
          value={state.stateFilter}
          onChange={(event) =>
            setState((prev) => ({ ...prev, stateFilter: event.target.value }))
          }
        >
          <option value="">— all states —</option>
          {STATE_FILTER_OPTIONS.map((option) => (
            <option key={option} value={option}>
              {option}
            </option>
          ))}
        </select>
      </label>
      {state.listStatus === 'loading' ? <LoadingState label="dispatch commands" /> : null}
      {state.listStatus === 'error' ? <ErrorState error={state.listError} /> : null}
      {state.listStatus === 'loaded' && state.list ? (
        <>
          <FreshnessStamp fetchedAtIso={state.list.fetchedAtIso} />
          {state.list.truncated ? (
            <p role="note">
              The listing stopped at the first page (keyset cursor present
              server-side); this view shows that page only and invents nothing for
              the remainder.
            </p>
          ) : null}
          {state.list.commands.length === 0 ? (
            <EmptyState
              title="No dispatch commands"
              explanation="GET /api/v1/dispatch-commands returned no records for this tenant (for the selected state filter, if any). Commands appear here once proposed; the console fabricates no command."
            />
          ) : (
            <ul data-testid="command-list">
              {state.list.commands.map((command) => (
                <li key={command.command_id}>
                  <label>
                    <input
                      type="radio"
                      name="selected-command"
                      checked={state.selectedCommandId === command.command_id}
                      onChange={() =>
                        setState((prev) => ({ ...prev, selectedCommandId: command.command_id }))
                      }
                    />{' '}
                    inspect
                  </label>
                  <CommandListItem command={command} />
                </li>
              ))}
            </ul>
          )}
        </>
      ) : null}
      {state.selectedCommandId !== undefined && state.detailStatus === 'loading' ? (
        <LoadingState label="the command detail" />
      ) : null}
      {state.detailStatus === 'error' ? <ErrorState error={state.detailError} /> : null}
      {state.detailStatus === 'loaded' && state.detail ? (
        <CommandDetailPanel data={state.detail} claims={claims} />
      ) : null}
    </section>
  );
}
