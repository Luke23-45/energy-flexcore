// P8.27 — the Simulation Lab research-surfacing section's data layer (the
// `pluginsWorkspaceData.ts` pattern: the loading/mapping module of a
// Phase-wave workspace panel — new files over rewrites, the P5.22
// Simulation Lab workspace itself is NOT modified; the two sections
// render in sequence on the same `simulation-lab` route via the App.tsx
// wiring).
//
// What this layer loads: the research-status read
// (`GET /api/v1/research/status`, api/research.ts) — the RECORDED Phase 8
// research status plus whatever the deployment's composed source reports.
// Every run fact the server does not report renders as the named gap
// (`TEMPLATE_EMPTY` / `NO_RUN` / zero proposals / `NOT_RECORDED`), never
// a plausible result.
//
// THE DISPLAY-ONLY GUARANTEE (P8.27 step 5, recorded; the P6.25/P7.25
// command-path rule): the console NEVER promotes a research proposal —
// the server has no promote/issue/authorize/fast-track/approve endpoint
// (verified against routes_research.rs / openapi.rs), and the only
// affordance is "record a promotion request", which records a LOCAL
// request event (the denialLog pattern) and states that promotion takes
// effect ONLY through the protected validation → feasibility →
// authorization gates and, for any field use, per-feature qualification
// (P8.29). The console cannot issue, authorize, or fast-track a proposal
// (the ledger's step-5 negative check — the UI walkthrough record is the
// human gate input, recorded UNRUN in the decision record).
//
// PROPOSED-VS-EXECUTED (P8.27 step 2, recorded; §20.2): every research
// output displayed here is labelled a PROPOSAL / simulation result and is
// visually distinct from validated schedules and executed commands — the
// PROPOSED_VS_EXECUTED_NOTE renders on the panel.
import {
  fetchResearchStatus,
  PROPOSAL_STATUS_PROPOSED,
  RESEARCH_VERSION_NOT_RECORDED,
  SHADOW_COMPARISON_NO_RUN,
  VALIDATION_OUTCOME_MISSING,
  type ResearchModuleDto,
  type ResearchProposalDto,
  type ResearchStatusDto,
  type ShadowComparisonDto,
} from '../api/research';
import type { ApiClient } from '../api/client';

/** The mapped load result of the panel's data read. */
export interface ResearchStatusData {
  status: ResearchStatusDto;
}

/**
 * Loads the research status through the tenant-scoped API client. A
 * failed read fails the panel honestly (the whole-load error path — the
 * status is one resource; per-slot errors do not apply here).
 */
export async function loadResearchStatus(client: ApiClient): Promise<ResearchStatusData> {
  const status = await fetchResearchStatus(client);
  return { status };
}

/** The module-version reading of one module row: the recorded stamp(s)
 * VERBATIM, or the named gap. Nothing is normalised or invented. */
export function moduleVersionOf(module: ResearchModuleDto): string {
  return module.module_version === RESEARCH_VERSION_NOT_RECORDED
    ? RESEARCH_VERSION_NOT_RECORDED
    : module.module_version;
}

/** The comparison reading of one shadow-comparison row (verbatim). */
export function shadowComparisonOf(comparison: ShadowComparisonDto): ShadowComparison {
  return {
    moduleId: comparison.module_id,
    baselineId: comparison.baseline_id,
    comparisonStatus: comparison.comparison_status,
    statisticsNote: comparison.statistics_note,
    comparisonNote: comparison.comparison_note,
  };
}

export interface ShadowComparison {
  moduleId: string;
  baselineId: string;
  comparisonStatus: string;
  statisticsNote: string | null;
  comparisonNote: string;
}

/** The proposal reading of one proposal row (verbatim; the validation
 * refusal named where the outcome is missing). */
export function proposalOf(proposal: ResearchProposalDto): ResearchProposal {
  return {
    proposalId: proposal.proposal_id,
    moduleId: proposal.module_id,
    moduleVersion: proposal.module_version,
    inputSnapshotId: proposal.input_snapshot_id,
    validationOutcome:
      proposal.validation_outcome === ''
        ? VALIDATION_OUTCOME_MISSING
        : proposal.validation_outcome,
    proposalStatus:
      proposal.proposal_status === '' ? PROPOSAL_STATUS_PROPOSED : proposal.proposal_status,
    provenanceNote: proposal.provenance_note,
  };
}

export interface ResearchProposal {
  proposalId: string;
  moduleId: string;
  moduleVersion: string;
  inputSnapshotId: string;
  validationOutcome: string;
  proposalStatus: string;
  provenanceNote: string;
}

/**
 * The verbatim statement the panel renders next to the promotion-request
 * affordance (the recorded P8.27 rule: the console can never execute a
 * promotion; step 5's negative check is recorded in the decision record).
 */
export const PROMOTION_REQUEST_NOTE =
  'This action RECORDS A REQUEST for a promotion review only. It does NOT ' +
  'promote anything: the console has no endpoint that could promote, issue, ' +
  'authorize, or fast-track a research proposal, and a promotion toward the ' +
  'dispatch path takes effect ONLY through the protected validation -> ' +
  'feasibility -> authorization gates (ADR-010, §13.4) and, for any field ' +
  'use, per-feature qualification (P8.29). A research proposal can never ' +
  'change the control-authority mode.';

/**
 * The §20.2 proposed-vs-executed statement for research outputs
 * (the P7.25 note, extended per the P8.27 step-2 labelling rule): a
 * research output is a PROPOSAL / simulation result, never an executed
 * action, and is displayed distinctly from validated schedules and
 * executed commands.
 */
export const RESEARCH_PROPOSED_VS_EXECUTED_NOTE =
  'A research output is a PROPOSAL / simulation result that still passes ' +
  'the protected validation → feasibility → authorization path — it gains ' +
  'no authority from having been proposed (ADR-010, §13.4). Research rows ' +
  'are labelled as proposals and are never displayed as executed commands ' +
  '(§20.2); validated schedules and issued commands live in the Schedules ' +
  'and Dispatch workspaces and are never merged into this section.';

/**
 * Records ONE promotion REQUEST as a local console event (the
 * security/denialLog pattern: local diagnostics only, never sent to any
 * server — no such endpoint exists and none was invented).
 */
export interface PromotionRequestEvent {
  requestedProposalId: string;
  /** The module the proposal came from, as reported at request time. */
  reportedModuleId: string;
  /** The validation outcome the server reported for the proposal, if any. */
  reportedValidationOutcome: string;
  reason: string;
  atIso: string;
}

const REQUEST_LOG_CAPACITY = 100;
let requestLog: PromotionRequestEvent[] = [];

export function recordPromotionRequest(event: {
  requestedProposalId: string;
  reportedModuleId: string;
  reportedValidationOutcome: string;
  reason: string;
}): void {
  requestLog = [
    ...requestLog,
    { ...event, atIso: new Date().toISOString() },
  ].slice(-REQUEST_LOG_CAPACITY);
}

export function getPromotionRequests(): PromotionRequestEvent[] {
  return [...requestLog];
}

export function clearPromotionRequests(): void {
  requestLog = [];
}

/** Convenience re-export for the panel's NO_RUN rendering. */
export const SHADOW_COMPARISON_NO_RUN_STATUS = SHADOW_COMPARISON_NO_RUN;
