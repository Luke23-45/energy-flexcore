// P8.27 — The research-status read (`GET /api/v1/research/status`): the
//         operator console's research surfacing seam (report §20.1 item 9
//         Simulation Lab, §20.2; ledger `ledger_phase_8.md` §P8.27).
//         ADDITIVE — no existing route is modified; the path is not one of
//         the 16 frozen §15.2 candidate resources and touches no dispatch
//         state (the recorded P4.36/P5.31/P6.06/P7.25 additive-path rule).
//
// AUTH (the recorded-drift pattern, the `authorize_live_stream` P5.25,
// `get_edge_status` P6.25 and `get_plugin_inventory` P7.25 precedent): the
// ledger's action id for this surface would be a new closed-enum variant
// (`research.status.read`), but the `Action` vocabulary is a CLOSED enum
// owned by `crates/flexcore-authz` (outside this wave's file ownership).
// The route therefore evaluates `telemetry.read` — already tenant-scoped
// and granted to ALL roles by the phase policy sets — as the interim
// deny-by-default gate; the single swap point is this call site. The
// caller's tenant is derived from the verified claims (§16.2); the
// request carries NO tenant field.
//
// DATA SOURCE (the honest seam, exactly the P6.25 `EdgeStatusSource` /
// P7.25 `PluginInventorySource` pattern): the control plane holds NO
// research run — Phase 8's evaluations are TEMPLATES with EMPTY cells,
// no shadow comparison has run, and no proposal exists. The surface is
// therefore served by the [`ResearchStatusSource`] seam: the shipped
// default [`RecordedResearchStatus`] serves the RECORDED Phase 8 research
// status (the module registry, the recorded OPEN blockers, the mode lock
// — from the phase decision records, recorded verbatim 2026-11-14), and
// every RUN fact (proposals, shadow comparisons, filter statistics,
// residual/termination summaries) renders as the explicit gap — never a
// plausible result. A deployment composes a real source (its
// evaluation/shadow forwarder) via [`AppState::with_research_status`].
//
// READ-ONLY SURFACE: there is NO endpoint here (or anywhere on the
// research surface) that promotes, issues, authorizes, fast-tracks, or
// approves a research proposal (ledger P8.27 step 5). Promotion toward
// the dispatch path, where a future phase ever builds it, passes ONLY
// through the protected validation → feasibility → authorization gates
// and, for any field use, per-feature qualification (ADR-010; P8.29) —
// none of which starts on this console.

use std::fmt;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use flexcore_authz::action::Action;
use flexcore_domain::tenant_context::TenantContext;

use crate::auth::{authorize_tenant_scoped, CallerExtractor};
use crate::dto::DataEnvelope;
use crate::dto_research::{
    ResearchModuleDto, ResearchProposalDto, ResearchStatusDto, ShadowComparisonDto,
};
use crate::error::ApiError;
use crate::state::AppState;

// --- The closed gap/state vocabulary ---------------------------------------------

/// The evaluation-record status of a module whose evaluation is a TEMPLATE
/// with EMPTY measured cells (the ENTRY_NOTE boundary-4 state; the P8.09/
/// P8.12/P8.15/P8.17 records verbatim). No vocabulary value ever means
/// "results exist".
pub const RESEARCH_EVALUATION_TEMPLATE_EMPTY: &str = "TEMPLATE_EMPTY";

/// The evaluation-record status of a module whose source record does not
/// exist yet (the gap, named).
pub const RESEARCH_EVALUATION_NO_RECORD: &str = "NO_RECORD";

/// The module_version gap rendered where no frozen version stamp exists
/// (never an invented version).
pub const RESEARCH_VERSION_NOT_RECORDED: &str = "NOT_RECORDED";

/// The shadow-comparison status: the comparison does not exist — no shadow
/// comparison has run and the frozen P8.06 baseline RESULTS artifact does
/// not exist until the first run. `NO_RUN` is never a zero-delta claim.
pub const SHADOW_COMPARISON_NO_RUN: &str = "NO_RUN";

/// The proposal lifecycle position — the ONLY proposal-side state this
/// surface carries (§20.2: a proposal is never displayed as executed).
pub const PROPOSAL_STATUS_PROPOSED: &str = "PROPOSED";

/// The validation-outcome refusal rendered for a proposal whose P8.05
/// outcome was not recorded (the refusal, never a pass).
pub const VALIDATION_OUTCOME_MISSING: &str = "VALIDATION_OUTCOME_MISSING";

/// The mode lock, verbatim: research operates under `simulation-shadow`
/// and no proposal/profile/plugin can select or raise the mode (§5.8/
/// §21.4).
pub const RESEARCH_MODE_LOCK: &str = "simulation-shadow";

/// The honest surface note (verbatim in every response).
pub const RESEARCH_STATUS_NOTE: &str = "the RECORDED Phase 8 research status (the \
    module registry, recorded OPEN blockers, and the mode lock — from the phase \
    decision records, recorded 2026-11-14), NOT a live research view: the control \
    plane holds NO research run, every evaluation record is a TEMPLATE with EMPTY \
    measured cells, no shadow comparison has run, and no proposal exists. Every \
    research output is a PROPOSAL (candidate) that gains no authority from having \
    been proposed (ADR-010); a promotion toward the dispatch path requires the \
    protected validation -> feasibility -> authorization gates and, for any field \
    use, per-feature qualification (P8.29). This surface is READ-ONLY: no promote, \
    issue, authorize, fast-track, or approve endpoint exists, and the console's \
    promotion-request affordance records a LOCAL request event only.";

// --- The source-level records ------------------------------------------------------

/// One research module of the status snapshot a composed source serves
/// (the recorded metadata; every field is a record fact, never a run
/// fact).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchModule {
    pub module_id: String,
    pub module_version: String,
    pub role: String,
    pub evaluation_status: String,
    pub evaluation_record: String,
    pub open_blockers: Vec<String>,
    pub provenance_note: String,
}

/// One research proposal as a composed source reports it (runtime facts;
/// every field required — a proposal without its provenance triple is a
/// composed-source defect, not something this route papers over).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchProposal {
    pub proposal_id: String,
    pub module_id: String,
    pub module_version: String,
    pub input_snapshot_id: String,
    pub validation_outcome: String,
    pub provenance_note: String,
}

/// One shadow-comparison row as a composed source reports it. The
/// `statistics_note` is the module-specific slot (ADMM residual/
/// termination; MARL filter statistics) — `None` = not reported, the gap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowComparison {
    pub module_id: String,
    pub baseline_id: String,
    pub comparison_status: String,
    pub statistics_note: Option<String>,
    pub comparison_note: String,
}

/// The status snapshot a composed source serves.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResearchStatusSnapshot {
    pub modules: Vec<ResearchModule>,
    pub proposals: Vec<ResearchProposal>,
    pub shadow_comparisons: Vec<ShadowComparison>,
}

/// Errors of the status-source seam (named; mapped to infrastructure).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResearchStatusError {
    /// The composed source failed (reason opaque/human-safe).
    Backend { reason: String },
}

impl fmt::Display for ResearchStatusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResearchStatusError::Backend { reason } => {
                write!(f, "the research-status source failed: {reason}")
            }
        }
    }
}

impl std::error::Error for ResearchStatusError {}

/// The research-status source seam (P8.27): the deployment composes its
/// evaluation/shadow forwarder (whatever carries the run facts); the
/// shipped default serves the RECORDED Phase 8 status with every run fact
/// an explicit gap (fail-closed honesty, never a fake result).
pub trait ResearchStatusSource: Send + Sync {
    /// The status snapshot. The `ctx` is the caller's server-derived
    /// tenant context (for sources that partition their records); the
    /// shipped default serves the phase-recorded set regardless of tenant
    /// (research status is per phase/deployment, not tenant-partitioned
    /// business data — the honest scoping statement is in the surface
    /// note).
    fn status(&self, ctx: &TenantContext) -> Result<ResearchStatusSnapshot, ResearchStatusError>;
}

/// The shipped default: the RECORDED Phase 8 research status, recorded
/// verbatim from the phase decision records (read 2026-11-14), with NO
/// run fact composed — zero proposals, every shadow comparison `NO_RUN`,
/// every evaluation record `TEMPLATE_EMPTY`. This is an honest record of
/// what the PHASE RECORDS state, never a claim about what RAN.
#[derive(Debug, Default, Clone, Copy)]
pub struct RecordedResearchStatus;

/// Builds a recorded module row (no run facts — the gaps).
fn recorded_module(
    module_id: &str,
    module_version: &str,
    role: &str,
    evaluation_status: &str,
    evaluation_record: &str,
    open_blockers: &[&str],
    provenance_note: &str,
) -> ResearchModule {
    ResearchModule {
        module_id: module_id.to_string(),
        module_version: module_version.to_string(),
        role: role.to_string(),
        evaluation_status: evaluation_status.to_string(),
        evaluation_record: evaluation_record.to_string(),
        open_blockers: open_blockers.iter().map(|s| (*s).to_string()).collect(),
        provenance_note: provenance_note.to_string(),
    }
}

/// Builds a recorded NO_RUN shadow-comparison row.
fn recorded_no_run_comparison(module_id: &str, baseline_id: &str, note: &str) -> ShadowComparison {
    ShadowComparison {
        module_id: module_id.to_string(),
        baseline_id: baseline_id.to_string(),
        comparison_status: SHADOW_COMPARISON_NO_RUN.to_string(),
        statistics_note: None,
        comparison_note: note.to_string(),
    }
}

impl ResearchStatusSource for RecordedResearchStatus {
    fn status(&self, _ctx: &TenantContext) -> Result<ResearchStatusSnapshot, ResearchStatusError> {
        // The rows below are the phase decision records' contents,
        // recorded verbatim (read 2026-11-14). A drifted record and a
        // drifted row are a review failure, not a runtime hazard: the
        // ledger's verification procedures re-check the records.
        let modules = vec![
            recorded_module(
                "admm-dmpc",
                "flexcore-research/admm-formulation.v1 + flexcore-research/admm-operations.v1",
                "M1 (P8.02 §4): ADMM/DMPC coordination with explicit convexity \
                 assumptions, residuals, termination, infeasibility handling, and \
                 centralized fallback — proposals only, reported as heuristic on \
                 nonconvex/mixed-integer instances (X-3).",
                RESEARCH_EVALUATION_TEMPLATE_EMPTY,
                "ledger/phase-08-advanced-coordination/09_admm_evaluation.md",
                &[
                    "the P8.06 baseline RESULTS artifact does not exist until the \
                     first human/lab run (BASELINE_NOT_FROZEN for the comparison \
                     columns)",
                    "no P8.38 tolerance pre-registration exists yet — the \
                     Win/Tie/Loss verdict column is refused (X-8)",
                ],
                "quasi-static: every result row cites flexcore-research-benchmark-set-v1, \
                 the P8.05 validation outcome, and a schema-valid P8.20 record (P8.23 §2)",
            ),
            recorded_module(
                "gnn-warm-start",
                "flexcore-gnn-evaluation/0.1.0 (evaluation harness); module version \
                 NOT_RECORDED until the design freeze resolves the framework pair",
                "M2 role (a) (P8.02 §4): candidate AC-OPF warm-start generation — the \
                 GNN warms the solver, never replaces it; the headline metric is \
                 end-to-end feasible solve time, never inference latency.",
                RESEARCH_EVALUATION_TEMPLATE_EMPTY,
                "ledger/phase-08-advanced-coordination/12_gnn_evaluation.md",
                &[
                    "the torch/PyG framework pair is a DECISION-POINT (P8.10 §6; \
                     D4 OPEN) — no pin exists and none is invented",
                    "the P8.35 hold-out manifest does not exist yet (the \
                     generalization claim cites held-out rows only, X-7)",
                    "the scope-extension follow-up is unresolved (12_gnn_evaluation.md \
                     §7 gap 1) — held-out runs produce the out-of-scope flag, not numbers",
                ],
                "quasi-static: every result row cites flexcore-research-benchmark-set-v1, \
                 the P8.05 validation outcome, and a schema-valid P8.20 record (P8.23 §2)",
            ),
            recorded_module(
                "gnn-screening",
                "flexcore-gnn-evaluation/0.1.0 (evaluation harness); module version \
                 NOT_RECORDED until the design freeze resolves the framework pair",
                "M2 role (b) (P8.02 §4): active-constraint/contingency screening — a \
                 warm-start win is NOT a screening win; the roles are never conflated \
                 into one headline number.",
                RESEARCH_EVALUATION_TEMPLATE_EMPTY,
                "ledger/phase-08-advanced-coordination/12_gnn_evaluation.md",
                &[
                    "the torch/PyG framework pair is a DECISION-POINT (P8.10 §6; \
                     D4 OPEN) — no pin exists and none is invented",
                    "the P8.35 hold-out manifest does not exist yet",
                ],
                "quasi-static: every result row cites flexcore-research-benchmark-set-v1, \
                 the P8.05 validation outcome, and a schema-valid P8.20 record (P8.23 §2)",
            ),
            recorded_module(
                "marl-policy",
                "flexcore-marl-shadow-log/1 (shadow log) + flexcore-marl-eval/1 \
                 (evaluation harness); module version NOT_RECORDED until the \
                 framework DECISION-POINT resolves",
                "M3 (P8.02 §4): safe MARL (CTDE + derived safety filter) in \
                 simulation/shadow ONLY — a live device command from a MARL worker \
                 is PROHIBITED, not deferred (X-2).",
                RESEARCH_EVALUATION_TEMPLATE_EMPTY,
                "ledger/phase-08-advanced-coordination/15_marl_evaluation.md",
                &[
                    "the RL framework is a DECISION-POINT (P8.13 §5; D4 OPEN) — no \
                     pin exists and none is invented",
                    "the CBF derivation is unsigned and training is BLOCKED until \
                     the sign-off exists (P8.13 §3)",
                ],
                "quasi-static: every result row cites flexcore-research-benchmark-set-v1, \
                 the P8.05 validation outcome, and a schema-valid P8.20 record (P8.23 §2)",
            ),
            recorded_module(
                "dynamic-controller",
                "flexcore-andes-worker/1 (protocol tag); module version NOT_RECORDED",
                "M4 (P8.02 §4): research controllers under disturbances in the \
                 declared ANDES model — evidence about the DECLARED MODEL only, \
                 never about the grid (X-5; ADR-012).",
                RESEARCH_EVALUATION_TEMPLATE_EMPTY,
                "ledger/phase-08-advanced-coordination/17_dynamic_evaluation.md",
                &[
                    "ANDES is never installed and its API is UNVERIFIED in this loop \
                     (P8.16; the pin procedure is the recorded one)",
                    "the dynamic cases are NOT in the benchmark set (NT-10) — rows \
                     cite the dynamic case reference, never a benchmark-set version",
                ],
                "dynamic track: rows cite inputs.dynamic_case_ref (model/case \
                 reference), NOT a benchmark-set version (X-5; P8.20 §2)",
            ),
            recorded_module(
                "baseline/highs-tier1-lp",
                "flexcore-research/baselines.v1 (registry; HiGHS pinned 1.15.1)",
                "the frozen Tier-1 deterministic baseline (P8.06 §3): centralized \
                 LP/MILP/QP (HiGHS — LP/QP/MIP, never SOCP).",
                RESEARCH_EVALUATION_NO_RECORD,
                "ledger/phase-08-advanced-coordination/05_06_harness_baselines.md",
                &[
                    "the registry freezes DEFINITIONS; the frozen RESULTS artifact \
                     does not exist until the first human/lab run",
                ],
                "the baseline columns of any comparison record refuse with \
                 BASELINE_NOT_FROZEN until the results artifact exists (P8.23 §3)",
            ),
            recorded_module(
                "baseline/clarabel-tier2-conic",
                "flexcore-research/baselines.v1 (registry; Clarabel pinned 0.11.1)",
                "the frozen Tier-2 deterministic baseline (P8.06 §3): convex conic \
                 (Clarabel — never MISOCP).",
                RESEARCH_EVALUATION_NO_RECORD,
                "ledger/phase-08-advanced-coordination/05_06_harness_baselines.md",
                &[
                    "the registry freezes DEFINITIONS; the frozen RESULTS artifact \
                     does not exist until the first human/lab run",
                ],
                "the baseline columns of any comparison record refuse with \
                 BASELINE_NOT_FROZEN until the results artifact exists (P8.23 §3)",
            ),
            recorded_module(
                "baseline/rule-based-priority-list",
                "flexcore-research/baselines.v1 (registry; in-crate heuristic, no \
                 external solver)",
                "the frozen rule-based deterministic baseline (P8.06 §3).",
                RESEARCH_EVALUATION_NO_RECORD,
                "ledger/phase-08-advanced-coordination/05_06_harness_baselines.md",
                &[
                    "the registry freezes DEFINITIONS; the frozen RESULTS artifact \
                     does not exist until the first human/lab run",
                ],
                "the baseline columns of any comparison record refuse with \
                 BASELINE_NOT_FROZEN until the results artifact exists (P8.23 §3)",
            ),
        ];
        // NO proposals exist (no research run has ever happened): the
        // shipped default serves the EMPTY set — never a plausible row.
        let proposals: Vec<ResearchProposal> = Vec::new();
        // The shadow-comparison rows: the comparison does not exist for any
        // module. The MARL and ADMM rows name the statistics slots (P8.27
        // step 3) with the statistics_note as the gap (None — an empty
        // denominator is never a rate of zero).
        let shadow_comparisons = vec![
            recorded_no_run_comparison(
                "admm-dmpc",
                "baseline/highs-tier1-lp",
                "the ADMM residual/termination summary (r vs eps_pri, s vs eps_dual, \
                 iteration count) rides here when a comparison ever exists; today \
                 NO_RUN — no comparison cell is filled and none is fabricated",
            ),
            recorded_no_run_comparison(
                "marl-policy",
                "baseline/highs-tier1-lp",
                "the MARL safety-filter statistics (activation / infeasibility / \
                 refusal rates and the outcome of EVERY infeasibility) ride here \
                 when a shadow log ever exists; today NO_RUN — the shadow log is \
                 EMPTY and an empty denominator is never a rate of zero \
                 (EVAL_LOG_EMPTY, P8.15 §5)",
            ),
        ];
        Ok(ResearchStatusSnapshot {
            modules,
            proposals,
            shadow_comparisons,
        })
    }
}

// --- The route -----------------------------------------------------------------

/// Builds the research-status router (merged into the stage-1 router).
/// GET only — a narrow READ surface; no mutation endpoint exists and none
/// may be added here (a promotion toward the dispatch path, where a
/// future phase ever builds it, passes ONLY through the protected gates —
/// P8.27 step 5).
pub fn research_router() -> Router<AppState> {
    Router::new().route("/api/v1/research/status", get(get_research_status))
}

/// Composes the DTO from the snapshot: recorded metadata flows verbatim;
/// a proposal whose validation outcome is missing renders the refusal
/// (never a pass); the mode lock is always the recorded constant.
fn compose_proposal_dto(proposal: &ResearchProposal) -> ResearchProposalDto {
    ResearchProposalDto {
        proposal_id: proposal.proposal_id.clone(),
        module_id: proposal.module_id.clone(),
        module_version: proposal.module_version.clone(),
        input_snapshot_id: proposal.input_snapshot_id.clone(),
        validation_outcome: if proposal.validation_outcome.is_empty() {
            VALIDATION_OUTCOME_MISSING.to_string()
        } else {
            proposal.validation_outcome.clone()
        },
        // The lifecycle position is ALWAYS proposal-side on this surface:
        // a composed source reporting anything else is a defect this
        // route refuses to relay (§20.2 — a proposal is never displayed
        // as executed; ADR-010).
        proposal_status: PROPOSAL_STATUS_PROPOSED.to_string(),
        provenance_note: proposal.provenance_note.clone(),
    }
}

/// The research-status read (P8.27). Auth: every authenticated role,
/// tenant-scoped (the recorded-drift interim gate above; the swap point
/// for the closed-enum `research.status.read` action). The status is the
/// phase's recorded research set — research records are per
/// phase/deployment, not tenant-partitioned business data; the statement
/// is in the verbatim surface note.
#[utoipa::path(
    get,
    path = "/api/v1/research/status",
    responses(
        (status = 200, description = "The RECORDED Phase 8 research status (module registry, recorded OPEN blockers, mode lock) plus whatever the composed source reports; absent run facts render as explicit gaps (TEMPLATE_EMPTY / NO_RUN / zero proposals)", body = DataEnvelope<ResearchStatusDto>),
        (status = 401, description = "Unauthenticated caller"),
        (status = 403, description = "The role is not granted the interim research-status-read action"),
        (status = 500, description = "The composed research-status source failed (infrastructure — honest, never a fake success)"),
    )
)]
async fn get_research_status(
    State(state): State<AppState>,
    caller: CallerExtractor,
) -> Result<Json<DataEnvelope<ResearchStatusDto>>, ApiError> {
    // AUTH: the recorded-drift interim gate (deny-by-default; the swap
    // point for the closed-enum `research.status.read` action).
    authorize_tenant_scoped(&state.policies, &caller.0, Action::TelemetryRead)?;
    let ctx = caller.0.ctx.clone();
    let snapshot = state
        .research_status
        .status(&ctx)
        .map_err(|error| {
            tracing::error!(
                %error,
                "the research-status source FAILED — the status is not served as a fake success"
            );
            ApiError::infrastructure()
        })?;
    let mut modules: Vec<ResearchModuleDto> = snapshot
        .modules
        .iter()
        .map(|m| ResearchModuleDto {
            module_id: m.module_id.clone(),
            module_version: m.module_version.clone(),
            role: m.role.clone(),
            evaluation_status: m.evaluation_status.clone(),
            evaluation_record: m.evaluation_record.clone(),
            open_blockers: m.open_blockers.clone(),
            provenance_note: m.provenance_note.clone(),
        })
        .collect();
    // Sorted by module id (deterministic output; the source may already
    // sort, the route does not rely on it).
    modules.sort_by(|a, b| a.module_id.cmp(&b.module_id));
    let proposals: Vec<ResearchProposalDto> =
        snapshot.proposals.iter().map(compose_proposal_dto).collect();
    let mut comparisons: Vec<ShadowComparisonDto> = snapshot
        .shadow_comparisons
        .iter()
        .map(|c| ShadowComparisonDto {
            module_id: c.module_id.clone(),
            baseline_id: c.baseline_id.clone(),
            comparison_status: c.comparison_status.clone(),
            statistics_note: c.statistics_note.clone(),
            comparison_note: c.comparison_note.clone(),
        })
        .collect();
    comparisons.sort_by(|a, b| a.module_id.cmp(&b.module_id));
    Ok(Json(DataEnvelope {
        data: ResearchStatusDto {
            modules,
            proposals,
            shadow_comparisons: comparisons,
            mode_lock: RESEARCH_MODE_LOCK.to_string(),
            status_note: RESEARCH_STATUS_NOTE.to_string(),
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_domain::ids::TenantId;
    use flexcore_domain::tenant_context::{AuthenticatedClaims, VerifiedClaims};

    fn tenant_ctx() -> TenantContext {
        TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
            VerifiedClaims {
                tenant_id: TenantId::new(FlexUuid::from_bytes(1u128.to_be_bytes())),
                subject: "p8.27-unit-test".to_string(),
            },
        ))
    }

    #[test]
    fn the_recorded_status_serves_the_closed_module_vocabulary_with_no_run_facts() {
        let snapshot = RecordedResearchStatus
            .status(&tenant_ctx())
            .expect("the recorded default never fails");
        // The closed P8.20 module vocabulary, verbatim ids.
        let ids: Vec<&str> = snapshot
            .modules
            .iter()
            .map(|m| m.module_id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec![
                "admm-dmpc",
                "baseline/clarabel-tier2-conic",
                "baseline/highs-tier1-lp",
                "baseline/rule-based-priority-list",
                "dynamic-controller",
                "gnn-screening",
                "gnn-warm-start",
                "marl-policy",
            ]
        );
        // Every module's evaluation status is an honest gap: TEMPLATE_EMPTY
        // for the research modules, NO_RECORD for the frozen baselines.
        // No status in this vocabulary means "results exist".
        for module in &snapshot.modules {
            assert!(
                module.evaluation_status == RESEARCH_EVALUATION_TEMPLATE_EMPTY
                    || module.evaluation_status == RESEARCH_EVALUATION_NO_RECORD,
                "{} must not fabricate an evaluation status",
                module.module_id
            );
        }
        // NO proposals exist — the gap, never a plausible row.
        assert!(
            snapshot.proposals.is_empty(),
            "no research run exists, so no proposal row may be fabricated"
        );
        // Every shadow comparison is NO_RUN with NO statistics — an empty
        // denominator is never a rate of zero.
        assert_eq!(snapshot.shadow_comparisons.len(), 2);
        for comparison in &snapshot.shadow_comparisons {
            assert_eq!(comparison.comparison_status, SHADOW_COMPARISON_NO_RUN);
            assert!(comparison.statistics_note.is_none());
        }
        // The recorded OPEN blockers are carried verbatim per module.
        let marl = snapshot
            .modules
            .iter()
            .find(|m| m.module_id == "marl-policy")
            .expect("the MARL row");
        assert_eq!(marl.open_blockers.len(), 2);
        assert!(marl.open_blockers[0].contains("DECISION-POINT"));
        assert!(marl.open_blockers[1].contains("CBF derivation is unsigned"));
    }

    #[test]
    fn the_dynamic_row_carries_no_benchmark_set_version_and_the_quasi_static_rows_do() {
        let snapshot = RecordedResearchStatus
            .status(&tenant_ctx())
            .expect("the recorded default never fails");
        let dynamic = snapshot
            .modules
            .iter()
            .find(|m| m.module_id == "dynamic-controller")
            .expect("the dynamic row");
        assert!(
            !dynamic.provenance_note.contains("flexcore-research-benchmark-set-v1"),
            "the dynamic track cites the dynamic case reference, never a benchmark-set version (X-5)"
        );
        assert!(dynamic.provenance_note.contains("dynamic case reference"));
        let admm = snapshot
            .modules
            .iter()
            .find(|m| m.module_id == "admm-dmpc")
            .expect("the ADMM row");
        assert!(admm.provenance_note.contains("flexcore-research-benchmark-set-v1"));
    }

    #[test]
    fn proposals_render_as_proposed_with_the_validation_refusal_never_a_pass() {
        let proposal = ResearchProposal {
            proposal_id: "p-1".to_string(),
            module_id: "admm-dmpc".to_string(),
            module_version: "flexcore-research/admm-formulation.v1".to_string(),
            input_snapshot_id: "snap-1".to_string(),
            validation_outcome: String::new(),
            provenance_note: "basis note".to_string(),
        };
        let dto = compose_proposal_dto(&proposal);
        // The lifecycle position is ALWAYS proposal-side on this surface —
        // never "issued"/"executed" (§20.2; ADR-010).
        assert_eq!(dto.proposal_status, "PROPOSED");
        // A missing validation outcome renders the REFUSAL, never a pass.
        assert_eq!(dto.validation_outcome, "VALIDATION_OUTCOME_MISSING");
        let recorded = ResearchProposal {
            validation_outcome: "passed".to_string(),
            ..proposal
        };
        let dto = compose_proposal_dto(&recorded);
        assert_eq!(dto.validation_outcome, "passed");
    }

    #[test]
    fn the_surface_note_names_the_read_only_promotion_and_gap_rules() {
        assert!(RESEARCH_STATUS_NOTE.contains("TEMPLATE"));
        assert!(RESEARCH_STATUS_NOTE.contains("READ-ONLY"));
        assert!(RESEARCH_STATUS_NOTE.contains("no promote, issue, authorize, fast-track, or approve endpoint"));
        assert!(RESEARCH_STATUS_NOTE.contains("protected validation -> feasibility -> authorization gates"));
        assert!(RESEARCH_STATUS_NOTE.contains("PROPOSAL"));
        assert_eq!(RESEARCH_MODE_LOCK, "simulation-shadow");
    }
}
