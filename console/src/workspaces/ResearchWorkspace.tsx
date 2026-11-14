// P8.27 — the Simulation Lab research-surfacing section EXTENSION (the
// P7.25 `PluginsWorkspace.tsx` precedent: a Phase-wave-added workspace
// panel — NEW files over rewrites; the P5.22 `SimulationLab.tsx` workspace
// is NOT modified, and the two sections render in sequence on the same
// `simulation-lab` route via the App.tsx wiring).
//
// WHAT AN OPERATOR CAN DETERMINE FROM THIS PANEL WITHOUT READING LOGS
// (the ledger P8.27 acceptance criterion):
// - every research module's id, role, recorded frozen version stamp(s)
//   (or `NOT_RECORDED` — the gap), evaluation-record status
//   (`TEMPLATE_EMPTY` / `NO_RECORD` — the closed vocabulary carries NO
//   "results exist" value), the owning decision record, the recorded OPEN
//   blockers, and the provenance rules its future result rows must carry;
// - the shadow-comparison rows vs the frozen deterministic baselines,
//   each rendered `NO_RUN` — the comparison does not exist, never a
//   zero-delta claim — with the module-specific statistics slot named
//   (ADMM residual/termination; MARL safety-filter statistics) and the
//   statistics explicitly ABSENT (an empty denominator is never a rate of
//   zero);
// - every proposal's provenance triple (module/model version, input
//   snapshot id, validation outcome) rendered per proposal — and today
//   the honest state is ZERO proposals (no research run exists; none is
//   fabricated);
// - the mode lock, verbatim: `simulation-shadow` — no proposal, profile,
//   or plugin can select or raise the control-authority mode (§5.8/§21.4).
//
// THE LABELLING RULE (P8.27 step 2, recorded; §20.2): every research
// output on this panel is labelled a PROPOSAL / simulation result and is
// structurally distinct from validated schedules and executed commands —
// those live in the Schedules and Dispatch workspaces and are never
// merged into this section; nothing on this panel can ever render as
// "executed" (the closed proposal vocabulary carries only `PROPOSED`).
//
// THE COMMAND-PATH GUARANTEE (P8.27 steps 4–5, recorded; the P6.25/P7.25
// rule): the console NEVER promotes a research proposal. There is NO
// endpoint on the server surface that could promote, issue, authorize,
// fast-track, or approve anything (verified against routes_research.rs /
// openapi.rs), and the only affordance is "record a promotion request",
// which requires an explicit confirmation control (the ConfirmHighImpact
// pattern: labelled required checkbox inside a <label>, aria-required,
// keyboard-operable, not colour- or hover-dependent), records a LOCAL
// request event (researchWorkspaceData.ts, the denialLog pattern), and
// states that promotion takes effect ONLY through the protected
// validation → feasibility → authorization gates and, for any field use,
// per-feature qualification (P8.29).
//
// THE NEGATIVE CHECK (P8.27 step 5): the UI walkthrough that attempts a
// gate-skipping promotion and confirms refusal + auditability is the
// human gate input — recorded UNRUN in
// ledger/phase-08-advanced-coordination/27_console_research.md (never
// claimed). The structural refusal is in the server surface: no such
// endpoint exists.
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
import type { ResearchModuleDto, ShadowComparisonDto } from '../api/research';
import {
  PROMOTION_REQUEST_NOTE,
  RESEARCH_PROPOSED_VS_EXECUTED_NOTE,
  getPromotionRequests,
  moduleVersionOf,
  proposalOf,
  recordPromotionRequest,
  shadowComparisonOf,
  loadResearchStatus,
  type ResearchStatusData,
} from './researchWorkspaceData';

/**
 * The P8.27 extension section the App wiring renders BELOW the P5.22
 * workspace on the same route. The P5.22 workspace records the
 * fault-injection visibility layer; this section is the Phase 8 addition:
 * the research modules exist as RECORDED Phase 8 status (evaluation
 * templates with EMPTY cells — awaiting the human/lab runs), and this
 * panel renders that recorded status — no research run exists, so every
 * run fact renders as the named gap.
 */
export function ResearchStatusSection(): JSX.Element {
  const state = useWorkspaceData(loadResearchStatus);
  return <ResearchStatusView state={state} />;
}

export function ResearchStatusView({
  state,
}: {
  state: LoadState<ResearchStatusData>;
}): JSX.Element {
  return (
    <section aria-label="Research status (Phase 8)" data-testid="research-status">
      <h2>Research status (Phase 8, P8.27)</h2>
      <p className="honesty-banner" role="note" data-testid="research-status-banner">
        This section EXTENDS the P5.22 Simulation Lab workspace above. The
        research modules exist as RECORDED Phase 8 status: every evaluation
        record is a TEMPLATE with EMPTY measured cells (the runs are
        human/lab gate inputs — no claim here is a tool-verified fact), no
        shadow comparison has run, and no proposal exists. The panel renders
        the RECORDED status the control plane serves; every run fact renders
        as an explicit gap. This surface is READ-ONLY: no promote, issue,
        authorize, fast-track, or approve endpoint exists, and nothing here
        is ever displayed as executed.
      </p>
      <RoleScopeLine />
      {state.status === 'loading' && <LoadingLine what="the research status" />}
      {state.status === 'error' && <ErrorBlock what="The research status" error={state.error} />}
      {state.status === 'ready' && <ResearchStatusReady data={state.data} readAt={state.readAt} />}
    </section>
  );
}

function ResearchStatusReady({
  data,
  readAt,
}: {
  data: ResearchStatusData;
  readAt: string;
}): JSX.Element {
  const { status } = data;
  return (
    <>
      <p data-testid="research-proposed-vs-executed">{RESEARCH_PROPOSED_VS_EXECUTED_NOTE}</p>

      <h3>Control-authority mode lock</h3>
      <p data-testid="research-mode-lock">
        Research operates under control-authority mode{' '}
        <strong>
          <code>{status.mode_lock}</code>
        </strong>{' '}
        — the mode LOCK: no proposal, profile, or plugin can select or raise
        the control-authority mode (§5.8/§21.4); the mode itself remains a
        deployment and per-site setting. Every output on this panel is a
        proposal under that mode, never a dispatch decision (ADR-010).
      </p>

      <h3>Research modules (the recorded Phase 8 set)</h3>
      {status.modules.length === 0 ? (
        <HonestEmpty
          what="Research module status"
          reason="The status read returned no module rows. No module row is fabricated."
          testId="research-modules-empty"
        />
      ) : (
        status.modules.map((module) => <ModuleArticle key={module.module_id} module={module} />)
      )}

      <h3>Shadow comparisons vs the deterministic baselines</h3>
      {status.shadow_comparisons.length === 0 ? (
        <HonestEmpty
          what="Shadow comparisons"
          reason="The status read returned no comparison rows. No comparison row is fabricated."
          testId="research-comparisons-empty"
        />
      ) : (
        <ShadowComparisonTable comparisons={status.shadow_comparisons} />
      )}

      <h3>Research proposals</h3>
      {status.proposals.length === 0 ? (
        <HonestEmpty
          what="Research proposals"
          reason="No research proposal exists: no research run has ever happened (every evaluation record is a TEMPLATE with EMPTY cells), so the proposal set is EMPTY — the gap, never a fabricated row."
          testId="research-proposals-empty"
        />
      ) : (
        status.proposals.map((proposal) => {
          const read = proposalOf(proposal);
          return (
            <article key={read.proposalId} data-testid={`research-proposal-${read.proposalId}`}>
              <h4>
                <code>{read.proposalId}</code>{' '}
                <strong>[PROPOSAL — {read.proposalStatus}; never executed]</strong>
              </h4>
              <dl>
                <dt>Module / model version</dt>
                <dd>{read.moduleVersion}</dd>
                <dt>Input snapshot id</dt>
                <dd>{read.inputSnapshotId}</dd>
                <dt>Validation outcome (P8.05, verbatim)</dt>
                <dd data-testid={`proposal-validation-${read.proposalId}`}>
                  {read.validationOutcome}
                  {read.validationOutcome === 'VALIDATION_OUTCOME_MISSING'
                    ? ' — the refusal: the P8.05 outcome was not recorded, and this proposal is not a pass.'
                    : ''}
                </dd>
                <dt>Provenance note (verbatim)</dt>
                <dd>{read.provenanceNote}</dd>
              </dl>
            </article>
          );
        })
      )}

      {status.proposals.length === 0 ? (
        <HonestEmpty
          what="Promotion request"
          reason="No proposal was returned by the status read, so there is nothing to record a promotion request for. No request is fabricated."
          testId="promotion-request-empty"
        />
      ) : (
        <PromotionRequestAffordance proposalCount={status.proposals.length} />
      )}

      <h3>Research promotion audit trail</h3>
      <HonestEmpty
        what="Research promotion audit records"
        reason="No research-promotion audit source is composed into the control plane: no promotion has ever been attempted or granted (there is no promotion endpoint), and no research-promotion event vocabulary exists in the audit trail this console reads. The gap is named — no audit record is fabricated here."
        testId="research-audit-gap"
      />

      <p data-testid="research-status-note">
        <strong>Surface note (server, verbatim):</strong> {status.status_note}
      </p>
      <p>
        <FreshnessLine readAt={readAt} />
      </p>
    </>
  );
}

function ModuleArticle({ module }: { module: ResearchModuleDto }): JSX.Element {
  return (
    <article data-testid={`research-module-${module.module_id}`}>
      <h4>
        <code>{module.module_id}</code>{' '}
        <small data-testid={`evaluation-status-${module.module_id}`}>
          [{module.evaluation_status}]
        </small>
      </h4>
      <dl>
        <dt>Role (the frozen P8.02 question, verbatim)</dt>
        <dd>{module.role}</dd>

        <dt>Module / model version (recorded stamps, verbatim)</dt>
        <dd data-testid={`module-version-${module.module_id}`}>
          {moduleVersionOf(module)}
          {module.module_version === 'NOT_RECORDED'
            ? ' — no frozen version stamp exists yet; the gap is named, never an invented version.'
            : ''}
        </dd>

        <dt>Evaluation record (the status source)</dt>
        <dd>
          <code>{module.evaluation_record}</code>
        </dd>

        <dt>Recorded OPEN blockers / decision points</dt>
        <dd data-testid={`blockers-${module.module_id}`}>
          {module.open_blockers.length === 0
            ? 'no recorded blocker beyond the not-yet-run state — this is NOT a claim that results exist'
            : module.open_blockers.map((blocker) => (
                <div key={blocker}>{blocker}</div>
              ))}
        </dd>

        <dt>Provenance rules for its result rows (recorded note)</dt>
        <dd>{module.provenance_note}</dd>
      </dl>
    </article>
  );
}

function ShadowComparisonTable({
  comparisons,
}: {
  comparisons: ShadowComparisonDto[];
}): JSX.Element {
  return (
    <table>
      <caption>
        The shadow-comparison rows vs the frozen deterministic baselines.
        Every row renders its recorded status VERBATIM — `NO_RUN` means the
        comparison DOES NOT EXIST (no shadow comparison has run; the frozen
        P8.06 baseline RESULTS artifact does not exist until the first run),
        never a zero-delta "no difference" claim. These are simulation/shadow
        comparisons (§21.4), never executed dispatch outcomes.
      </caption>
      <thead>
        <tr>
          <th scope="col">Module</th>
          <th scope="col">Baseline (frozen P8.06 id)</th>
          <th scope="col">Comparison status (verbatim)</th>
          <th scope="col">Module-specific statistics</th>
          <th scope="col">Note (verbatim)</th>
        </tr>
      </thead>
      <tbody>
        {comparisons.map((comparison) => {
          const read = shadowComparisonOf(comparison);
          return (
            <tr key={`${read.moduleId}-${read.baselineId}`} data-testid={`shadow-comparison-${read.moduleId}`}>
              <td>
                <code>{read.moduleId}</code>
              </td>
              <td>
                <code>{read.baselineId}</code>
              </td>
              <td data-testid={`comparison-status-${read.moduleId}`}>
                <strong>[{read.comparisonStatus}]</strong>
              </td>
              <td data-testid={`comparison-statistics-${read.moduleId}`}>
                {read.statisticsNote === null
                  ? 'NOT REPORTED — the gap: an empty denominator is never a rate of zero; no statistic is fabricated.'
                  : read.statisticsNote}
              </td>
              <td>{read.comparisonNote}</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}

/**
 * The promotion-REQUEST affordance (the ProfileSwitchRequestAffordance
 * pattern, exactly): the request is RECORDED LOCALLY ONLY — nothing is
 * promoted, no endpoint is called, and the recorded note states the gates
 * the change actually takes effect through. The explicit confirmation
 * follows the ConfirmHighImpact pattern (labelled required checkbox
 * inside a <label>, aria-required, keyboard-operable).
 */
function PromotionRequestAffordance({
  proposalCount,
}: {
  proposalCount: number;
}): JSX.Element {
  const [confirmed, setConfirmed] = useState(false);
  const [recorded, setRecorded] = useState(false);
  return (
    <div data-testid="promotion-request">
      <h3>Request a promotion review (RECORD ONLY — nothing is promoted)</h3>
      <p data-testid="promotion-request-note">{PROMOTION_REQUEST_NOTE}</p>
      {!recorded ? (
        <>
          <p>
            <label htmlFor="promotion-request-confirm">
              <input
                type="checkbox"
                id="promotion-request-confirm"
                data-testid="promotion-request-checkbox"
                aria-required="true"
                checked={false}
                onChange={() => setConfirmed(true)}
              />{' '}
              I have read the statement above and want to RECORD A REQUEST for
              a promotion review of a research proposal ({proposalCount}{' '}
              proposal row(s) served), knowing the request itself performs NO
              promotion and the gates stay protected.
            </label>
          </p>
          <p>
            <button
              type="button"
              data-testid="promotion-request-button"
              disabled={!confirmed}
              onClick={() => {
                recordPromotionRequest({
                  requestedProposalId: 'UNSPECIFIED (the request records a review request only)',
                  reportedModuleId: 'UNSPECIFIED',
                  reportedValidationOutcome: 'NOT_RECORDED',
                  reason:
                    'operator requested a promotion review (recorded locally; promotion takes effect ONLY through the protected validation -> feasibility -> authorization gates and, for any field use, per-feature qualification — P8.29)',
                });
                setRecorded(true);
              }}
            >
              Record request (local console event only)
            </button>
          </p>
        </>
      ) : (
        <p data-testid="promotion-request-recorded" role="status">
          REQUEST RECORDED LOCALLY at {new Date().toISOString()}. Nothing was
          promoted: no endpoint call was made, and any promotion takes effect
          ONLY through the protected validation → feasibility → authorization
          gates (and, for any field use, per-feature qualification — P8.29).
          Requests recorded this session: {getPromotionRequests().length}.
        </p>
      )}
    </div>
  );
}
