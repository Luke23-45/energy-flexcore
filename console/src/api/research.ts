// P8.27 — the research-status read DTO mirrors + typed fetch wrapper
// (`GET /api/v1/research/status`, apps/control-plane routes_research.rs /
// dto_research.rs).
//
// SHAPE-MIRROR NOTE (the api/plugins.ts discipline): these types mirror
// dto_research.rs field-for-field using the WIRE names (snake_case). The
// authoritative shapes are pinned by the OpenAPI schema (the endpoint is
// registered in openapi.rs) and the generated-client drift test lands
// with the first install; the hand-written mirrors here are its stand-in.
//
// HONESTY CONTRACT (the P7.25 plugins.ts contract, applied to research):
// `evaluation_status: "TEMPLATE_EMPTY"` / `"NO_RECORD"`,
// `comparison_status: "NO_RUN"`, `module_version: "NOT_RECORDED"`, and an
// empty `proposals` array are HONEST GAPS — "no research run exists",
// never "converged", "passed", or "no difference". The closed
// vocabularies below are the Phase 8 records' own (the P8.20
// reproducibility module vocabulary and the P8.23 refusal vocabulary),
// rendered VERBATIM everywhere — the console never prettifies a gap into
// a plausible result.
//
// PROPOSALS-ONLY GUARANTEE (P8.27 step 2, recorded; ADR-010): every
// research output is a PROPOSAL — a candidate that still passes the
// protected validation → feasibility → authorization path — and is never
// displayed as executed. The mode is LOCKED to `simulation-shadow`: no
// proposal, profile, or plugin can select or raise the control-authority
// mode (§5.8/§21.4).
//
// THE DISPLAY-ONLY GUARANTEE (P8.27 step 5, recorded; the P7.25
// command-path rule): this module issues GET calls ONLY — there is NO
// endpoint on the server that promotes, issues, authorizes, fast-tracks,
// or approves a research proposal (verified against routes_research.rs /
// openapi.rs), and none is called or invented here. The console's
// promotion-request affordance records a LOCAL request event
// (workspaces/researchWorkspaceData.ts, the denialLog pattern) and states
// that promotion takes effect ONLY through the protected gates and, for
// any field use, per-feature qualification (P8.29).
import { api, type ApiClient } from './client';

/** The closed P8.20 reproducibility module vocabulary (research modules +
 * frozen P8.06 baseline registry ids), rendered verbatim. */
export const RESEARCH_MODULE_IDS = [
  'admm-dmpc',
  'gnn-warm-start',
  'gnn-screening',
  'marl-policy',
  'dynamic-controller',
  'baseline/highs-tier1-lp',
  'baseline/clarabel-tier2-conic',
  'baseline/rule-based-priority-list',
] as const;

export type ResearchModuleId = (typeof RESEARCH_MODULE_IDS)[number];

/**
 * The evaluation-record status vocabulary (closed): `TEMPLATE_EMPTY` —
 * the evaluation record is a TEMPLATE with EMPTY measured cells (the runs
 * are human/lab gate inputs); `NO_RECORD` — the source record does not
 * exist yet. NO value in this vocabulary means "results exist".
 */
export const RESEARCH_EVALUATION_STATUSES = ['TEMPLATE_EMPTY', 'NO_RECORD'] as const;

export type ResearchEvaluationStatus = (typeof RESEARCH_EVALUATION_STATUSES)[number];

/**
 * The module-version gap: no frozen version stamp exists yet (e.g. the
 * framework DECISION-POINT is open). Named, never an invented version.
 */
export const RESEARCH_VERSION_NOT_RECORDED = 'NOT_RECORDED';

/**
 * The shadow-comparison status (closed): the comparison does not exist —
 * no shadow comparison has run and the frozen P8.06 baseline RESULTS
 * artifact does not exist until the first run. Never a zero-delta claim.
 */
export const SHADOW_COMPARISON_NO_RUN = 'NO_RUN';

/**
 * The ONLY proposal lifecycle state this surface carries (§20.2: a
 * proposal is never displayed as executed; ADR-010).
 */
export const PROPOSAL_STATUS_PROPOSED = 'PROPOSED';

/**
 * The validation-outcome refusal rendered for a proposal whose P8.05
 * outcome was not recorded — the refusal, never a pass.
 */
export const VALIDATION_OUTCOME_MISSING = 'VALIDATION_OUTCOME_MISSING';

/** The mode lock, verbatim (research operates under simulation-shadow). */
export const RESEARCH_MODE_LOCK = 'simulation-shadow';

/** One research module of the recorded status (dto_research.rs ResearchModuleDto). */
export interface ResearchModuleDto {
  module_id: string;
  /** The recorded frozen version stamp(s), or `NOT_RECORDED` — the gap. */
  module_version: string;
  /** The module's role from the frozen P8.02 question list (M1–M4). */
  role: string;
  /** `TEMPLATE_EMPTY` | `NO_RECORD` — verbatim; never "results exist". */
  evaluation_status: string;
  /** The decision record that owns the module's evaluation (ledger path). */
  evaluation_record: string;
  /** The recorded OPEN blockers / decision points (verbatim notes). */
  open_blockers: string[];
  /** The provenance triple note (benchmark-set version / P8.05 / P8.20 rules). */
  provenance_note: string;
}

/** One research proposal as a composed source would serve it. The shipped
 * default serves ZERO proposals — no research run exists. */
export interface ResearchProposalDto {
  proposal_id: string;
  module_id: string;
  module_version: string;
  /** The input snapshot id the run consumed (the §9.2 provenance base). */
  input_snapshot_id: string;
  /** The P8.05 validation outcome, verbatim — or the refusal above. */
  validation_outcome: string;
  /** Always `PROPOSED` on this surface (never executed; ADR-010). */
  proposal_status: string;
  provenance_note: string;
}

/** One shadow-comparison row vs the deterministic baseline
 * (dto_research.rs ShadowComparisonDto). */
export interface ShadowComparisonDto {
  module_id: string;
  /** The frozen P8.06 baseline registry id, verbatim. */
  baseline_id: string;
  /** `NO_RUN` — verbatim; never a zero-delta claim. */
  comparison_status: string;
  /** The module-specific statistics slot (ADMM residual/termination;
   * MARL filter statistics). `null` = not reported — the gap, never a
   * rate over an empty denominator. */
  statistics_note: string | null;
  comparison_note: string;
}

/** The research-status read (dto_research.rs ResearchStatusDto). */
export interface ResearchStatusDto {
  modules: ResearchModuleDto[];
  proposals: ResearchProposalDto[];
  shadow_comparisons: ShadowComparisonDto[];
  /** The mode lock, verbatim: `simulation-shadow`. */
  mode_lock: string;
  /** The honest surface note (verbatim). */
  status_note: string;
}

/**
 * Fetches the research status (the tenant is server-derived from claims —
 * §16.2; NO tenant parameter exists or is sent). A forbidden role → 403
 * (rendered as the honest error, never hidden). READ-ONLY: this module
 * issues GET calls only (the display-only guarantee above).
 */
export function fetchResearchStatus(
  client: ApiClient = api,
): Promise<ResearchStatusDto> {
  return client
    .get<{ data: ResearchStatusDto }>('/api/v1/research/status')
    .then((envelope) => envelope.data);
}
