// P8.27 — The `/api/v1/research/status` research-status read DTOs (report
//         §20.1 item 9 Simulation Lab surfacing, §20.2; ledger
//         `ledger_phase_8.md` §P8.27). ADDITIVE — no existing DTO is
//         modified.
//
// HONESTY CONTRACT (the P7.25 dto_plugins.rs pattern, applied to research):
// every measured/runtime fact this surface does not have a composed source
// for is an HONEST GAP — `evaluation_status: TEMPLATE_EMPTY` and
// `proposals: []` mean "no research run exists", never "ready",
// "converged", or "passed". Phase 8 has ZERO measured results: every
// evaluation record is a TEMPLATE with EMPTY cells (ledger ENTRY_NOTE
// boundary 4), the frozen baseline RESULTS artifact does not exist until
// the first run, and the torch/PyG and RL-framework DECISION-POINTs are
// OPEN. The console renders the gaps and the verbatim notes; it never
// upgrades a gap into a plausible result.
//
// PROPOSALS-ONLY GUARANTEE (ADR-010, §13.4; §20.2): every research output
// this surface could ever carry is a PROPOSAL — a candidate that enters
// the protected validation → feasibility → authorization path and gains
// no authority from having been proposed. There is NO mutation endpoint
// on this surface: no promote, issue, authorize, fast-track, or approve
// route exists (verified against routes_research.rs / openapi.rs), and a
// proposal can never select or raise the control-authority mode (the
// mode is a deployment and per-site setting; the research profiles' mode
// is locked to `simulation-shadow`, §5.8/§21.4).

use serde::Serialize;
use utoipa::ToSchema;

/// One research module of the recorded status (P8.27). Every field is
/// RECORDED metadata from the phase's decision records the composed
/// [`crate::routes_research::ResearchStatusSource`] serves — never a run
/// observation unless a deployment composes a source that has actually
/// run something.
#[derive(Debug, Serialize, ToSchema)]
pub struct ResearchModuleDto {
    /// The module id (the closed P8.20 reproducibility vocabulary,
    /// verbatim: `admm-dmpc` | `gnn-warm-start` | `gnn-screening` |
    /// `marl-policy` | `dynamic-controller` | the three frozen
    /// `baseline/*` registry ids).
    pub module_id: String,
    /// The module's recorded frozen version stamp(s), VERBATIM where the
    /// source record freezes one (e.g. `flexcore-research/admm-formulation.v1`
    /// + `flexcore-research/admm-operations.v1`); `NOT_RECORDED` where no
    /// frozen stamp exists yet — the gap, never an invented version.
    pub module_version: String,
    /// The module's role, verbatim from the frozen P8.02 question list
    /// (M1–M4): what the module is, never what it achieved.
    pub role: String,
    /// The evaluation-record status (closed vocabulary, verbatim):
    /// `TEMPLATE_EMPTY` — the evaluation record is a TEMPLATE whose
    /// measured cells are EMPTY and whose runs are human/lab gate inputs.
    /// `NO_RECORD` — the source record does not exist yet. No status in
    /// this vocabulary means "results exist".
    pub evaluation_status: String,
    /// The decision record that owns the module's evaluation (the ledger
    /// path, verbatim) — the provenance of this status row.
    pub evaluation_record: String,
    /// The recorded OPEN decision points / blockers that keep the module's
    /// evaluation unfilled (verbatim notes: DECISION-POINTs, unsigned
    /// derivations, missing manifests). EMPTY list = no recorded blocker
    /// beyond the not-yet-run state — never a claim that results exist.
    pub open_blockers: Vec<String>,
    /// The provenance triple every future result row must carry, recorded
    /// as a note (the benchmark-set version id for quasi-static modules —
    /// `flexcore-research-benchmark-set-v1` — the P8.05 validation-outcome
    /// rule, and the P8.20 record rule). Dynamic modules carry NO
    /// benchmark-set version (the X-5 boundary; their rows cite the
    /// dynamic case reference instead) — the note says which.
    pub provenance_note: String,
}

/// One research proposal (P8.27 step 1) as a composed source would serve
/// it — with the provenance triple the ledger requires. The SHIPPED
/// default serves ZERO proposals (no research run exists); a deployment
/// composes a real source via [`crate::state::AppState::with_research_status`].
#[derive(Debug, Serialize, ToSchema)]
pub struct ResearchProposalDto {
    /// The proposal id (the source's own id; the shipped default serves
    /// none, so none is fabricated here).
    pub proposal_id: String,
    /// The module that produced it (the closed P8.20 vocabulary, verbatim).
    pub module_id: String,
    /// The module/model version stamp of the producing run.
    pub module_version: String,
    /// The input snapshot id the run consumed (the §9.2 provenance base).
    pub input_snapshot_id: String,
    /// The P8.05 independent-validation outcome for this proposal
    /// (verbatim from the composed source; a proposal without a recorded
    /// validation outcome renders `VALIDATION_OUTCOME_MISSING` — the
    /// refusal, never a pass).
    pub validation_outcome: String,
    /// The lifecycle position, VERBATIM and always proposal-side:
    /// `PROPOSED` — a candidate that entered the protected path. There is
    /// no "issued"/"executed" research state on this surface (§20.2: a
    /// proposal is never displayed as executed; ADR-010).
    pub proposal_status: String,
    /// The verbatim provenance note from the composed source (its basis,
    /// benchmark-set version citation, reproducibility-record reference).
    pub provenance_note: String,
}

/// The shadow-comparison summary for one research module vs the
/// deterministic baseline (P8.27 step 3's ADMM residual/termination and
/// MARL filter statistics ride on these rows). The SHIPPED default serves
/// the NO_RUN state for every module.
#[derive(Debug, Serialize, ToSchema)]
pub struct ShadowComparisonDto {
    /// The module id the comparison would be about (closed vocabulary).
    pub module_id: String,
    /// The deterministic baseline it would compare against (the frozen
    /// P8.06 registry ids, verbatim).
    pub baseline_id: String,
    /// The comparison status (closed vocabulary, verbatim):
    /// `NO_RUN` — the comparison does not exist: no shadow comparison has
    /// run, the frozen baseline RESULTS artifact does not exist, and every
    /// comparison cell is EMPTY. `NO_RUN` is the honest answer, never a
    /// zero-delta "no difference" claim.
    pub comparison_status: String,
    /// The module-specific statistics slot, verbatim from the composed
    /// source: for `admm-dmpc` the residual/termination summary; for
    /// `marl-policy` the safety-filter statistics (activation /
    /// infeasibility / refusal rates and the outcome of EVERY
    /// infeasibility). `null` = not reported — the gap, never a computed
    /// rate over an empty log (an empty denominator cannot become a rate
    /// of zero; the P8.15 `EVAL_LOG_EMPTY` refusal is the harness-side
    /// twin of this rule).
    pub statistics_note: Option<String>,
    /// The verbatim note from the composed source (why the comparison
    /// does not exist yet, or its basis if one is ever composed).
    pub comparison_note: String,
}

/// The research-status read (P8.27). The shape mirrors the composed
/// [`crate::routes_research::ResearchStatusSnapshot`] — the RECORDED
/// Phase 8 research status plus whatever the deployment's composed source
/// reports; absent run facts render as the explicit gaps named above.
#[derive(Debug, Serialize, ToSchema)]
pub struct ResearchStatusDto {
    /// The recorded research-module status rows (sorted by module id).
    pub modules: Vec<ResearchModuleDto>,
    /// The recorded proposals. ZERO from the shipped default — no research
    /// run exists, so no proposal row is fabricated.
    pub proposals: Vec<ResearchProposalDto>,
    /// The shadow-comparison rows vs the frozen deterministic baselines
    /// (sorted by module id).
    pub shadow_comparisons: Vec<ShadowComparisonDto>,
    /// The control-authority mode the research surface operates under,
    /// verbatim: `simulation-shadow` — the mode lock. A profile, plugin,
    /// or research proposal can never select or raise the mode (§5.8/
    /// §21.4); the mode itself remains a deployment and per-site setting.
    pub mode_lock: String,
    /// The honest surface note (what this status is and is not — verbatim
    /// in every response).
    pub status_note: String,
}
