// P8.06 — The strong deterministic baseline registry (the comparison every
//         research module must beat). Ledger `ledger/phase-08-advanced-
//         coordination/ledger_phase_8.md` §P8.06; report trace §23 Stage 8
//         (exit gate), §13.1–§13.3, §19.3–§19.4, §12.3; decision record:
//         `ledger/phase-08-advanced-coordination/05_06_harness_baselines.md`.
//
// WHAT THIS IS: the version-stamped, FROZEN registry of deterministic
// baselines every Phase 8 research module is compared against — defined and
// frozen BEFORE any research result is compared to them (no moving the
// baseline after seeing research numbers, ledger P8.06 step 3/risk).
//
// The three tiers (P8.06 step 1; ADR-009 — the deterministic optimizer +
// independent feasibility validation IS the baseline):
//
// - **Tier 1 (HiGHS)** — LP/QP/MIP on the linearized problem, via the
//   EXISTING `crates/flexcore-optimization-api` Tier 1 provider seam.
//   SOLVER-CLASS DISCIPLINE VERBATIM (AGENTS.md rule 4; §7.4): HiGHS is
//   LP/QP/MIP, **not SOCP**.
// - **Tier 2 (Clarabel)** — convex conic where applicable, via the
//   EXISTING Tier 2 provider seam. DISCIPLINE VERBATIM: Clarabel is convex
//   conic, **not MISOCP**; no integer variables (the P3.18 discipline).
// - **Rule-based (priority-list dispatch)** — a SIMPLE, fully-specified,
//   deterministic dispatch heuristic implemented IN-CRATE below (no
//   solver, no optimality claim, no randomness). Canonical units MW/MWh;
//   `P > 0` = export at the point of connection; SOC a fraction in
//   [0, 1] (§10.2).
//
// COMPOSITION SEAM (recorded): the solver tiers are consumed through the
// EXISTING `Optimizer` port of `crates/flexcore-optimization-api/src/
// port.rs` (`capabilities()` → `SolverCapabilities`, `solve(problem,
// config)` → `OptimizationResult` with the closed six-status §12.6
// vocabulary) — mirrored here by citation as [`BaselineOptimizer`] because
// this crate keeps its dependency set minimal (see `Cargo.toml`); the
// composition root implements the port over the real providers. The frozen
// solver versions below are the workspace pins (root `Cargo.toml`:
// `clarabel = "=0.11.1"`; `crates/flexcore-optimization-api/Cargo.toml`
// header: highspy 1.15.1 via the Python Tier 1 worker — RESEARCH.md §2,
// checked 7 November 2026). They are RECORDED facts about the pin, not a
// claim that any build ran.
//
// FROZEN-VERSION DISCIPLINE (P8.06 step 3): a baseline change is a NEW
// baseline version, never a silent edit. Every record carries its version;
// freezing computes a digest over the record's canonical description
// (FNV-1a 64 — the repo's test-grade checksum pattern, P4.28/P6.14/P7.14;
// explicitly NOT a security primitive); re-running must reproduce the
// frozen digest or the deviation is NAMED, never absorbed.
//
// SEED POLICY (P8.06 step 2): the heuristic is seed-INDEPENDENT (it
// contains no randomness — recorded as [`SeedPolicy::None`]); the solver
// tiers run under the pinned thread-count-1 configuration (the P3.26 pin
// block; `SolverConfig::thread_count` default 1) with a caller-supplied
// seed recorded per run through the worker contract's
// `DeterminismMetadata` (P7.42) — same seed + same pinned version → same
// result, and any non-determinism is an EXCLUDED run, not an averaged one.
//
// BENCHMARK ENTRY POINT (P8.06 step 2): every baseline runs on the §19.3
// benchmark cases through the existing benchmark infrastructure —
// `apps/sim-runner` (the benchmark/replay path, P5.18/P5.19) over
// `scenarios/library/` (lib-01…lib-08, INDEX.md) — with pinned versions,
// seeds, and tolerances recorded per run (the P5.19 pre-registered
// tolerance discipline). The §19.3 research-case extensions are P8.19's
// artifact; the registry cites the library version, not a case list it
// does not own.
//
// Control status: research comparison infrastructure. Nothing here
// dispatches, authorizes, or claims optimality beyond the declared problem
// class. Determinism: sorted iteration, fixed field order, no wall clock
// anywhere in the heuristic.

use serde::Serialize;

/// The registry version: the version of the BASELINE SET as a whole
/// (adding a baseline is a registry version bump; editing one is a record
/// version bump — never silent).
pub const BASELINE_REGISTRY_VERSION: &str = "flexcore-research/baselines.v1";

/// The in-crate heuristic baseline's own version (a rule change is a NEW
/// version of the heuristic — the frozen-version discipline applied to the
/// rule set itself).
pub const PRIORITY_LIST_BASELINE_VERSION: &str = "priority-list.v1";

/// The recorded baseline statement (the repo's statement pattern — pinned
/// by tests).
pub const BASELINE_STATEMENT: &str = "the baseline registry is the frozen, version-stamped \
     comparison set every Phase 8 research module must beat: Tier 1 HiGHS LP/QP/MIP on the \
     linearized problem (NOT SOCP), Tier 2 Clarabel convex conic where applicable (NOT \
     MISOCP; no integer variables), and a fully-specified deterministic priority-list \
     dispatch heuristic (no solver, no optimality claim). A baseline change is a NEW \
     baseline version, never a silent edit; results are frozen (versioned artifact, \
     digest-stamped) BEFORE research numbers exist. No safety, certification, or \
     optimality claim beyond the declared problem class is made.";

// --- Solver-class discipline (VERBATIM — AGENTS.md rule 4; §7.4) -------------

/// The Tier 1 solver-class label (HiGHS).
pub const SOLVER_CLASS_LABEL_HIGHS: &str =
    "HiGHS — LP/QP/MIP (Tier 1); not SOCP (AGENTS.md rule 4; report §7.4)";
/// The Tier 2 solver-class label (Clarabel).
pub const SOLVER_CLASS_LABEL_CLARABEL: &str =
    "Clarabel.rs — convex conic (Tier 2); not MISOCP; no integer variables (P3.18 discipline)";
/// The rule-based baseline's class label (no solver at all).
pub const SOLVER_CLASS_LABEL_PRIORITY_LIST: &str =
    "rule-based deterministic dispatch — no solver; no optimality claim";

/// The frozen Tier 1 solver version: the workspace's recorded highspy pin
/// (`crates/flexcore-optimization-api/Cargo.toml` header; RESEARCH.md §2,
/// checked 2026-11-07).
pub const FROZEN_SOLVER_VERSION_HIGHS: &str = "1.15.1";
/// The frozen Tier 2 solver version: the workspace's exact pin
/// (root `Cargo.toml`: `clarabel = "=0.11.1"`).
pub const FROZEN_SOLVER_VERSION_CLARABEL: &str = "0.11.1";

/// The exact benchmark entry point every baseline record cites (the
/// existing infrastructure; the §19.3 case set + P8.19 extensions).
pub const BENCHMARK_ENTRY_POINT: &str =
    "apps/sim-runner (apps/sim-runner/src/benchmark.rs; P5.18/P5.19 replay path) over \
     scenarios/library (lib-01…lib-08, INDEX.md; §19.3 base set; research extensions are \
     the P8.19 versioned artifact)";

// ---------------------------------------------------------------------------
// The registry records
// ---------------------------------------------------------------------------

/// The kind of baseline (the tier identity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BaselineKind {
    /// Tier 1 LP/QP/MIP on the linearized problem (HiGHS provider seam).
    Tier1LpQpHighs,
    /// Tier 2 convex conic where applicable (Clarabel provider seam).
    Tier2ConicClarabel,
    /// The in-crate rule-based deterministic dispatch heuristic.
    RuleBasedPriorityList,
}

/// The seed policy of a baseline (recorded per run; P8.06 step 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SeedPolicy {
    /// No randomness in the baseline: the seed is irrelevant and a run is
    /// deterministic by construction (the in-crate heuristic).
    None,
    /// A fixed caller-supplied seed, recorded per run through the worker
    /// contract's `DeterminismMetadata` (P7.42); the solver runs pinned at
    /// thread count 1 (the P3.26 determinism pin).
    Fixed,
}

impl SeedPolicy {
    /// The recorded text form (audit/record field).
    pub fn as_record(self) -> &'static str {
        match self {
            SeedPolicy::None => "none (no randomness in the baseline)",
            SeedPolicy::Fixed => "fixed caller-supplied seed via DeterminismMetadata \
                                  (P7.42); thread_count pinned 1 (P3.26)",
        }
    }
}

/// One baseline definition: the frozen identity a research module is
/// compared against. Fields are the P8.06 step-1/step-4 requirements:
/// frozen version, seed policy, the exact benchmark entry point, and the
/// solver-class label — plus the assumptions/limits that make a "win"
/// against a weak baseline VISIBLE (P8.06 step 4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BaselineRecord {
    /// The stable baseline id (e.g. `baseline/highs-tier1-lp`).
    pub baseline_id: &'static str,
    /// The baseline version (a change is a NEW version, never a silent
    /// edit). Starts at `v1`.
    pub version: String,
    /// The tier identity.
    pub kind: BaselineKind,
    /// The solver-class label (VERBATIM discipline — see the constants).
    pub solver_class_label: &'static str,
    /// The frozen solver version (`"n/a — no solver"` for the heuristic).
    pub frozen_solver_version: &'static str,
    /// The seed policy (recorded form travels on every comparison).
    pub seed_policy: SeedPolicy,
    /// The exact benchmark entry point ([`BENCHMARK_ENTRY_POINT`]).
    pub benchmark_entry_point: &'static str,
    /// The per-baseline provider seam (the `Optimizer` port declaration
    /// the composition root implements).
    pub provider_seam: &'static str,
    /// Assumptions and limits (P8.06 step 4 — what the baseline does NOT
    /// do, so a "win" against it is honestly visible).
    pub assumptions_and_limits: Vec<&'static str>,
}

impl BaselineRecord {
    /// The canonical digest input: stable field order, `|` separators —
    /// the deterministic string the frozen digest is computed over.
    pub fn canonical_string(&self) -> String {
        format!(
            "id:{}|version:{}|kind:{:?}|label:{}|solver:{}|seed:{}|entry:{}|seam:{}|assumptions:{}",
            self.baseline_id,
            self.version,
            self.kind,
            self.solver_class_label,
            self.frozen_solver_version,
            self.seed_policy.as_record(),
            self.benchmark_entry_point,
            self.provider_seam,
            self.assumptions_and_limits.join(";"),
        )
    }

    /// The next version of this record (`v1` → `v2`, …): the ONLY legal
    /// way a baseline changes. A bump is a NEW record; the old frozen
    /// artifact stays frozen.
    pub fn bump_version(&self) -> BaselineRecord {
        let mut bumped = self.clone();
        bumped.version = next_version(&self.version);
        bumped
    }
}

/// `v<N>` → `v<N+1>` (unparseable versions get a `v2` suffix appended —
/// never silently reused).
fn next_version(version: &str) -> String {
    if let Some(rest) = version.strip_prefix('v') {
        if let Ok(n) = rest.parse::<u64>() {
            return format!("v{}", n + 1);
        }
    }
    format!("{version}.v2")
}

/// The baseline id of the Tier 1 HiGHS record.
pub const BASELINE_ID_HIGHS_TIER1: &str = "baseline/highs-tier1-lp";
/// The baseline id of the Tier 2 Clarabel record.
pub const BASELINE_ID_CLARABEL_TIER2: &str = "baseline/clarabel-tier2-conic";
/// The baseline id of the rule-based record.
pub const BASELINE_ID_PRIORITY_LIST: &str = "baseline/rule-based-priority-list";

/// The full frozen registry (P8.06 step 1 + step 4): the baseline set every
/// research module is compared against. Order is fixed (registry order is
/// part of the frozen artifact).
pub fn baseline_registry() -> Vec<BaselineRecord> {
    vec![
        BaselineRecord {
            baseline_id: BASELINE_ID_HIGHS_TIER1,
            version: "v1".to_string(),
            kind: BaselineKind::Tier1LpQpHighs,
            solver_class_label: SOLVER_CLASS_LABEL_HIGHS,
            frozen_solver_version: FROZEN_SOLVER_VERSION_HIGHS,
            seed_policy: SeedPolicy::Fixed,
            benchmark_entry_point: BENCHMARK_ENTRY_POINT,
            provider_seam: "crates/flexcore-optimization-api Optimizer port (Tier 1 provider \
                            seam; §12.6 result contract)",
            assumptions_and_limits: vec![
                "solves the LINEARIZED problem only: LP/QP/MIP classes per ProblemClass \
                 (lp / convex_qp / milp); declares SOCP-class problems unsupported (never \
                 approximates)",
                "deterministic under the pinned configuration: thread_count = 1 (P3.26), \
                 pinned solver version, recorded config digest (§12.6 DeterminismRecord)",
                "no integer-variable claim beyond MIP: not a conic solver",
                "no global-optimality claim for mixed-integer solves terminated within a \
                 gap (the §12.6 'feasible' status, not 'optimal')",
            ],
        },
        BaselineRecord {
            baseline_id: BASELINE_ID_CLARABEL_TIER2,
            version: "v1".to_string(),
            kind: BaselineKind::Tier2ConicClarabel,
            solver_class_label: SOLVER_CLASS_LABEL_CLARABEL,
            frozen_solver_version: FROZEN_SOLVER_VERSION_CLARABEL,
            seed_policy: SeedPolicy::None,
            benchmark_entry_point: BENCHMARK_ENTRY_POINT,
            provider_seam: "crates/flexcore-optimization-api Optimizer port (Tier 2 provider \
                            seam; §12.6 result contract)",
            assumptions_and_limits: vec![
                "convex conic problems only (LP/QP/SOCP/SDP/exp/power cones per the \
                 provider declaration); NO integer variables — a mixed-integer conic \
                 problem (MISOCP) is NOT solvable here and must be declared unsupported",
                "applicable only where the problem is genuinely conic (e.g. a relaxed \
                 network formulation); the linearized Tier 1 comparison remains the \
                 primary baseline",
                "deterministic under the pinned configuration; no seed input exists for \
                 the algorithm (SeedPolicy::None recorded honestly)",
            ],
        },
        BaselineRecord {
            baseline_id: BASELINE_ID_PRIORITY_LIST,
            version: PRIORITY_LIST_BASELINE_VERSION.to_string(),
            kind: BaselineKind::RuleBasedPriorityList,
            solver_class_label: SOLVER_CLASS_LABEL_PRIORITY_LIST,
            frozen_solver_version: "n/a — no solver (in-crate deterministic heuristic)",
            seed_policy: SeedPolicy::None,
            benchmark_entry_point: BENCHMARK_ENTRY_POINT,
            provider_seam: "in-crate (flexcore-research::baselines::priority_list_dispatch); \
                            no provider seam",
            assumptions_and_limits: vec![
                "fully-specified rule set (see priority_list_dispatch): assets in \
                 (priority, asset_id) order, committed between p_min and p_max, no \
                 overshoot (remaining < p_min skips the asset), optional per-asset energy \
                 budget = energy_capacity_mwh * soc_fraction (SOC a fraction in [0,1])",
                "no feasibility knowledge: the dispatch is a CAPABILITY-level rule set — \
                 it does not see network constraints; a rule-based dispatch can still be \
                 refused by the §12.5 feasibility gate",
                "under-supply is an honest outcome (the deficit is recorded per interval), \
                 never silently filled",
                "the strength of this baseline is exactly the written rule set; a research \
                 module that beats it beats THOSE rules, not 'a heuristic'",
            ],
        },
    ]
}

/// The Phase 8 research modules the baselines serve (P8.06 step 1 mapping).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchModule {
    /// The ADMM/DMPC provider (P8.07–P8.09).
    AdmmDmpc,
    /// The GNN warm-start/screening experiment (P8.10–P8.12).
    GnnWarmStart,
    /// The safe MARL simulation/shadow worker (P8.13–P8.15).
    Marl,
}

/// The per-module baseline mapping (P8.06 step 1): every research module
/// gets at least one strong deterministic baseline AND the rule-based
/// baseline — the ledger's minimum. Order fixed; ids resolve against
/// [`baseline_registry`].
pub fn baselines_for_module(module: ResearchModule) -> Vec<&'static str> {
    match module {
        ResearchModule::AdmmDmpc => vec![
            BASELINE_ID_HIGHS_TIER1,
            BASELINE_ID_CLARABEL_TIER2,
            BASELINE_ID_PRIORITY_LIST,
        ],
        // The GNN warms the solver; its comparison is against the COLD-START
        // solver (the same Tier 1 seam, cold) and the non-GNN heuristic warm
        // start is the rule-based record (P8.10/P8.12 design owns the
        // experiment framing — the registry supplies the frozen artifacts).
        ResearchModule::GnnWarmStart => vec![BASELINE_ID_HIGHS_TIER1, BASELINE_ID_PRIORITY_LIST],
        // MARL compares against deterministic optimization AND the
        // rule-based baseline (§13.3).
        ResearchModule::Marl => vec![BASELINE_ID_HIGHS_TIER1, BASELINE_ID_PRIORITY_LIST],
    }
}

// ---------------------------------------------------------------------------
// Freezing (P8.06 step 3)
// ---------------------------------------------------------------------------

/// The freezing errors. Every failure is named; a digest mismatch is a
/// NAMED deviation, never absorbed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FreezeError {
    /// The record's assumptions/limits list is empty — a baseline without
    /// written assumptions cannot be frozen (P8.06 step 4).
    AssumptionsMissing { baseline_id: String },
    /// The observed digest does not match the frozen one.
    DigestMismatch { expected: String, observed: String },
}

impl std::fmt::Display for FreezeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FreezeError::AssumptionsMissing { baseline_id } => write!(
                f,
                "baseline {baseline_id} carries no assumptions/limits record; it cannot be \
                 frozen until its assumptions are written down (P8.06 step 4)"
            ),
            FreezeError::DigestMismatch { expected, observed } => write!(
                f,
                "the re-run digest {observed} does not match the frozen digest {expected}; \
                 the deviation is named, never absorbed"
            ),
        }
    }
}

impl std::error::Error for FreezeError {}

/// A frozen baseline: the record plus the digest over its canonical
/// description (and, for the executable heuristic, over its outputs — see
/// [`FrozenBaseline::with_results`]). Frozen BEFORE any research result is
/// compared to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FrozenBaseline {
    /// The frozen record.
    pub record: BaselineRecord,
    /// The digest over the record's canonical description (FNV-1a 64, the
    /// repo's test-grade checksum — NOT a security primitive).
    pub definition_digest: String,
    /// The digest over the frozen RESULTS (the run outputs on the benchmark
    /// set), when frozen with results.
    pub results_digest: Option<String>,
    /// The registry version the freeze happened under (provenance).
    pub registry_version: String,
}

impl FrozenBaseline {
    /// Freezes a record's DEFINITION (assumptions must be present — a
    /// baseline without written assumptions cannot be frozen, P8.06
    /// step 4).
    pub fn freeze(record: &BaselineRecord) -> Result<FrozenBaseline, FreezeError> {
        if record.assumptions_and_limits.is_empty() {
            return Err(FreezeError::AssumptionsMissing {
                baseline_id: record.baseline_id.to_string(),
            });
        }
        Ok(FrozenBaseline {
            definition_digest: format!("{:016x}", fnv1a64(record.canonical_string().as_bytes())),
            record: record.clone(),
            results_digest: None,
            registry_version: BASELINE_REGISTRY_VERSION,
        })
    }

    /// Attaches the frozen RESULTS digest (P8.06 step 3: the versioned
    /// results artifact). The caller computes the digest over the run
    /// outputs with [`results_digest_for`].
    pub fn with_results(mut self, results_digest: String) -> FrozenBaseline {
        self.results_digest = Some(results_digest);
        self
    }

    /// The reproduction check (P8.06 verification method: re-run one case
    /// from the frozen artifact's inputs and confirm the recorded result
    /// reproduces). A mismatch is a NAMED error, never absorbed.
    pub fn confirm_reproduction(&self, observed_digest: &str) -> Result<(), FreezeError> {
        let expected = self.results_digest.as_deref().unwrap_or(&self.definition_digest);
        if expected == observed_digest {
            Ok(())
        } else {
            Err(FreezeError::DigestMismatch {
                expected: expected.to_string(),
                observed: observed_digest.to_string(),
            })
        }
    }
}

/// Computes the results digest over a fixed canonical rendering of run
/// outputs (one string per result line, in order). Deterministic: the
/// caller passes lines already in fixed order.
pub fn results_digest_for(result_lines: &[String]) -> String {
    format!("{:016x}", fnv1a64(result_lines.join("\n").as_bytes()))
}

// ---------------------------------------------------------------------------
// The in-crate deterministic dispatch heuristic (the rule-based baseline)
// ---------------------------------------------------------------------------

/// The numeric epsilon of the dispatch rules (MW). Below this, a remaining
/// demand or an energy budget counts as exhausted. Fixed, part of the
/// frozen rule set.
pub const PRIORITY_LIST_EPSILON_MW: f64 = 1e-9;

/// One interval's demand: the MW the DER fleet must INJECT (export at the
/// point of connection, `P > 0` convention) to serve the interval's net
/// load. Positive values only; a negative value is a input error (the
/// rule set does not absorb loads, it serves them).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct IntervalDemand {
    /// The net demand to serve this interval, MW (≥ 0).
    pub net_demand_mw: f64,
}

/// One dispatchable asset of the rule-based baseline (the capability-level
/// facts the rules may use — nothing more).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DispatchAsset {
    /// The asset id (tie-break key; see the rule set).
    pub asset_id: String,
    /// The dispatch priority (LOWER runs FIRST — priority 1 is the first
    /// asset committed).
    pub priority: u32,
    /// Minimum output when committed, MW (≥ 0; export convention).
    pub p_min_mw: f64,
    /// Maximum output, MW (≥ p_min).
    pub p_max_mw: f64,
    /// The storable energy capacity, MWh, when the asset is energy-limited
    /// (`None` = not energy-limited).
    pub energy_capacity_mwh: Option<f64>,
    /// The state of charge as a fraction in [0, 1] (§10.2), when the asset
    /// is energy-limited. `None` with a capacity means fully charged (1.0).
    pub soc_fraction: Option<f64>,
}

/// The rule-based baseline's input: a horizon of intervals and the asset
/// list. Everything is validated before any rule runs (fail-closed, named
/// errors).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PriorityListInput {
    /// The interval duration, hours (> 0; MWh = MW × hours).
    pub interval_hours: f64,
    /// The intervals, in horizon order (an empty horizon is legal and
    /// dispatches nothing).
    pub intervals: Vec<IntervalDemand>,
    /// The assets (any order; the rule set sorts).
    pub assets: Vec<DispatchAsset>,
}

/// The named input errors of the rule set (fail-closed; every rejection
/// names its field — the §14.2 rejection discipline). `PartialEq` only:
/// the variants carry `f64` payloads, and `Eq` is not derivable over
/// floats (coordinator consistency pass, post-P8.08 review finding).
#[derive(Debug, Clone, PartialEq)]
pub enum DispatchInputError {
    /// The interval duration is not a finite positive number of hours.
    NonPositiveIntervalHours { value: f64 },
    /// An interval demand is not finite.
    NonFiniteDemand { index: usize },
    /// An interval demand is negative (the rules serve loads, not absorb
    /// them; a negative demand is a formulation error).
    NegativeDemand { index: usize, value: f64 },
    /// An asset id is empty.
    EmptyAssetId { index: usize },
    /// A power field is not finite (`field` names which).
    NonFinitePower { index: usize, asset_id: String, field: &'static str },
    /// A power field is negative.
    NegativePower { index: usize, asset_id: String, field: &'static str },
    /// The asset's p_min exceeds its p_max.
    PMinAbovePMax { index: usize, asset_id: String, p_min_mw: f64, p_max_mw: f64 },
    /// The energy capacity is not finite or negative.
    InvalidEnergyCapacity { index: usize, asset_id: String, value: f64 },
    /// The SOC fraction is outside [0, 1] (§10.2: a fraction at the API
    /// boundary; out-of-range is a formulation error, never clipped).
    SocOutOfRange { index: usize, asset_id: String, value: f64 },
}

impl std::fmt::Display for DispatchInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DispatchInputError::NonPositiveIntervalHours { value } => {
                write!(f, "interval_hours must be a finite positive number of hours; got {value}")
            }
            DispatchInputError::NonFiniteDemand { index } => {
                write!(f, "interval {index}: demand is not finite")
            }
            DispatchInputError::NegativeDemand { index, value } => {
                write!(f, "interval {index}: demand {value} MW is negative; the rules serve \
                           loads, never absorb them")
            }
            DispatchInputError::EmptyAssetId { index } => {
                write!(f, "asset {index}: asset_id is empty")
            }
            DispatchInputError::NonFinitePower { index, asset_id, field } => {
                write!(f, "asset {index} ({asset_id}): field '{field}' is not finite")
            }
            DispatchInputError::NegativePower { index, asset_id, field } => {
                write!(f, "asset {index} ({asset_id}): field '{field}' is negative (P > 0 is \
                           export at the point of connection; the rules dispatch export)")
            }
            DispatchInputError::PMinAbovePMax { index, asset_id, p_min_mw, p_max_mw } => {
                write!(f, "asset {index} ({asset_id}): p_min {p_min_mw} MW exceeds p_max \
                           {p_max_mw} MW")
            }
            DispatchInputError::InvalidEnergyCapacity { index, asset_id, value } => {
                write!(f, "asset {index} ({asset_id}): energy_capacity_mwh {value} is not a \
                           finite non-negative quantity")
            }
            DispatchInputError::SocOutOfRange { index, asset_id, value } => {
                write!(f, "asset {index} ({asset_id}): soc_fraction {value} is outside [0,1] \
                           (§10.2); out-of-range is a formulation error, never clipped")
            }
        }
    }
}

impl std::error::Error for DispatchInputError {}

/// One asset's dispatch in one interval.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AssetIntervalDispatch {
    /// The asset id.
    pub asset_id: String,
    /// The dispatched power, MW (export convention; 0 = not committed).
    pub p_mw: f64,
}

/// One interval's dispatch result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IntervalDispatch {
    /// The interval index (horizon order).
    pub index: usize,
    /// The demand the interval had to serve, MW.
    pub net_demand_mw: f64,
    /// The dispatched assets, in the rule set's FIXED order.
    pub dispatched: Vec<AssetIntervalDispatch>,
    /// The total supplied, MW.
    pub supplied_mw: f64,
    /// The deficit (demand − supplied, ≥ 0), MW. An honest outcome,
    /// never silently filled.
    pub deficit_mw: f64,
}

/// The full dispatch result of the rule-based baseline.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PriorityListDispatch {
    /// The per-interval results, in horizon order.
    pub intervals: Vec<IntervalDispatch>,
    /// The total supplied energy over the horizon, MWh.
    pub total_supplied_mwh: f64,
    /// The total deficit energy over the horizon, MWh.
    pub total_deficit_mwh: f64,
    /// The heuristic version that produced this result (the frozen-version
    /// discipline: the result names its rule set).
    pub baseline_version: String,
}

/// THE RULE SET (fully specified, deterministic — the frozen
/// `priority-list.v1` rules):
///
/// 1. **Validate** the input (fail-closed, named errors): every power
///    field finite and ≥ 0 (export convention), p_min ≤ p_max, demands
///    finite and ≥ 0, interval_hours > 0, SOC fractions in [0, 1],
///    capacities finite and ≥ 0.
/// 2. **Order** assets by (priority ASC, asset_id ASC) — a fixed
///    lexicographic tie-break makes equal priorities deterministic.
/// 3. **Per interval, in horizon order:** walk the ordered assets;
///    for each asset with remaining demand > ε:
///    a. the energy-limited cap is `remaining_energy_mwh / interval_hours`
///       (floored at 0), where `remaining_energy_mwh` starts at
///       `energy_capacity_mwh × soc_fraction` and decreases by the energy
///       dispatched in PRIOR intervals;
///    b. the candidate is `min(remaining_demand, p_max, energy_cap)`;
///    c. if the candidate is below `p_min`, the asset is SKIPPED (the
///       rules never overshoot to reach a minimum, and never dispatch
///       below a declared minimum);
///    d. otherwise the asset commits at exactly the candidate; the
///       remaining demand and the asset's energy budget decrease.
/// 4. **Record** the supplied total and the deficit per interval (deficit
///    = demand − supplied, ≥ 0) — an under-supplied interval is an honest
///    outcome, never silently filled.
///
/// Determinism: fixed ordering, fixed rule order, no randomness, no wall
/// clock, no float accumulation beyond the documented arithmetic — the
/// same input always produces the same result.
pub fn priority_list_dispatch(
    input: &PriorityListInput,
) -> Result<PriorityListDispatch, DispatchInputError> {
    validate_input(input)?;

    // Rule 2 — the fixed order (sorted by priority, then id).
    let mut ordered: Vec<&DispatchAsset> = input.assets.iter().collect();
    ordered.sort_by(|a, b| a.priority.cmp(&b.priority).then(a.asset_id.cmp(&b.asset_id)));

    // The cumulative energy used per asset (indexed by position in
    // `ordered`; the input's assets are untouched).
    let mut energy_used_mwh: Vec<f64> = vec![0.0; ordered.len()];

    let mut interval_results: Vec<IntervalDispatch> = Vec::with_capacity(input.intervals.len());
    for (index, interval) in input.intervals.iter().enumerate() {
        let mut remaining = interval.net_demand_mw;
        let mut dispatched: Vec<AssetIntervalDispatch> = Vec::with_capacity(ordered.len());
        for (position, asset) in ordered.iter().enumerate() {
            if remaining <= PRIORITY_LIST_EPSILON_MW {
                break; // Rule 3: the interval is served.
            }
            // Rule 3a — the energy-limited cap, when the asset has one.
            let energy_cap_mw = match asset.energy_capacity_mwh {
                Some(capacity) => {
                    let soc = asset.soc_fraction.unwrap_or(1.0);
                    let available_mwh = (capacity * soc - energy_used_mwh[position]).max(0.0);
                    available_mwh / input.interval_hours
                }
                None => f64::INFINITY,
            };
            // Rule 3b — the candidate.
            let candidate = remaining.min(asset.p_max_mw).min(energy_cap_mw);
            // Rule 3c — below the declared minimum: skip, never overshoot.
            if candidate < asset.p_min_mw - PRIORITY_LIST_EPSILON_MW {
                continue;
            }
            // Rule 3d — commit at exactly the candidate.
            let p = candidate;
            dispatched.push(AssetIntervalDispatch {
                asset_id: asset.asset_id.clone(),
                p_mw: p,
            });
            energy_used_mwh[position] += p * input.interval_hours;
            remaining -= p;
        }
        let supplied: f64 = dispatched.iter().map(|d| d.p_mw).sum();
        let deficit = (interval.net_demand_mw - supplied).max(0.0);
        interval_results.push(IntervalDispatch {
            index,
            net_demand_mw: interval.net_demand_mw,
            dispatched,
            supplied_mw: supplied,
            deficit_mw: deficit,
        });
    }

    let total_supplied_mwh: f64 = interval_results
        .iter()
        .map(|i| i.supplied_mw * input.interval_hours)
        .sum();
    let total_deficit_mwh: f64 = interval_results
        .iter()
        .map(|i| i.deficit_mw * input.interval_hours)
        .sum();

    Ok(PriorityListDispatch {
        intervals: interval_results,
        total_supplied_mwh,
        total_deficit_mwh,
        baseline_version: PRIORITY_LIST_BASELINE_VERSION,
    })
}

/// Rule 1 — input validation (fail-closed, named errors, first failure
/// wins).
fn validate_input(input: &PriorityListInput) -> Result<(), DispatchInputError> {
    if !input.interval_hours.is_finite() || input.interval_hours <= 0.0 {
        return Err(DispatchInputError::NonPositiveIntervalHours {
            value: input.interval_hours,
        });
    }
    for (index, interval) in input.intervals.iter().enumerate() {
        if !interval.net_demand_mw.is_finite() {
            return Err(DispatchInputError::NonFiniteDemand { index });
        }
        if interval.net_demand_mw < 0.0 {
            return Err(DispatchInputError::NegativeDemand {
                index,
                value: interval.net_demand_mw,
            });
        }
    }
    for (index, asset) in input.assets.iter().enumerate() {
        if asset.asset_id.trim().is_empty() {
            return Err(DispatchInputError::EmptyAssetId { index });
        }
        for (field, value) in [
            ("p_min_mw", asset.p_min_mw),
            ("p_max_mw", asset.p_max_mw),
        ] {
            if !value.is_finite() {
                return Err(DispatchInputError::NonFinitePower {
                    index,
                    asset_id: asset.asset_id.clone(),
                    field,
                });
            }
            if value < 0.0 {
                return Err(DispatchInputError::NegativePower {
                    index,
                    asset_id: asset.asset_id.clone(),
                    field,
                });
            }
        }
        if asset.p_min_mw > asset.p_max_mw {
            return Err(DispatchInputError::PMinAbovePMax {
                index,
                asset_id: asset.asset_id.clone(),
                p_min_mw: asset.p_min_mw,
                p_max_mw: asset.p_max_mw,
            });
        }
        if let Some(capacity) = asset.energy_capacity_mwh {
            if !capacity.is_finite() || capacity < 0.0 {
                return Err(DispatchInputError::InvalidEnergyCapacity {
                    index,
                    asset_id: asset.asset_id.clone(),
                    value: capacity,
                });
            }
        }
        if let Some(soc) = asset.soc_fraction {
            if !soc.is_finite() || !(0.0..=1.0).contains(&soc) {
                return Err(DispatchInputError::SocOutOfRange {
                    index,
                    asset_id: asset.asset_id.clone(),
                    value: soc,
                });
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The composition seam to the platform providers (mirrored by citation)
// ---------------------------------------------------------------------------

/// The `Optimizer` composition seam, MIRRORED by citation (recorded — do
/// not drift silently): the real contract is
/// `crates/flexcore-optimization-api/src/port.rs::Optimizer`
/// (`capabilities()` → `SolverCapabilities`, `solve(problem, config)` →
/// `OptimizationResult` with the closed six-status §12.6 vocabulary). This
/// crate keeps its dependency set minimal (see `Cargo.toml`); the
/// composition root implements THIS port over the real providers, and the
/// mirror is pinned by tests here. The kernel/host mirror discipline
/// (P7.10/11 §6) applies: a change on either side is a deliberate,
/// recorded seam update.
pub const OPTIMIZER_PORT_ALIGNMENT_SEAM: &str =
    "mirror of crates/flexcore-optimization-api/src/port.rs::Optimizer \
     (capabilities() -> SolverCapabilities; solve(problem, config) -> OptimizationResult, \
     closed six-status §12.6 vocabulary); composition root implements the port over the \
     Tier 1 HiGHS and Tier 2 Clarabel providers";

/// The baseline solver seam the composition root implements over the
/// EXISTING providers (the same object-safe, sync shape as the real
/// `Optimizer` port — see [`OPTIMIZER_PORT_ALIGNMENT_SEAM`]).
pub trait BaselineOptimizer: Send + Sync {
    /// Declares what the provider supports (mirrors
    /// `Optimizer::capabilities`); the registry's record must AGREE with
    /// the declaration (the class gate is the provider's obligation, not
    /// the registry's).
    fn capabilities_declaration(&self) -> String;
}

// ---------------------------------------------------------------------------
// The digest (the repo's FNV-1a 64 pattern, test-grade)
// ---------------------------------------------------------------------------

/// The FNV-1a 64-bit hash (the repo's non-cryptographic integrity-checksum
/// pattern — P4.28/P6.14/P7.14; deterministic, dependency-free, explicitly
/// NOT a security primitive; same honesty note as the kernel and the
/// command-gate digest usage).
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value is either
// hand-computed from the rule set below, a published FNV-1a 64 test vector,
// or a literal copied from this module's constants — nothing is fabricated).
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- the digest function: published FNV-1a 64 test vectors -------------

    #[test]
    fn fnv1a64_matches_published_test_vectors() {
        // The offset basis (the empty input is defined as the basis).
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        // Published FNV-1a 64 test vectors (draft-eastlake-fnv test set and
        // the reference implementation's published values for "a" and
        // "foobar") — these pin the digest used for FREEZING.
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
    }

    // --- the registry (known answers) ---------------------------------------

    #[test]
    fn registry_constants_are_the_recorded_strings() {
        assert_eq!(BASELINE_REGISTRY_VERSION, "flexcore-research/baselines.v1");
        assert_eq!(PRIORITY_LIST_BASELINE_VERSION, "priority-list.v1");
        assert_eq!(FROZEN_SOLVER_VERSION_HIGHS, "1.15.1");
        assert_eq!(FROZEN_SOLVER_VERSION_CLARABEL, "0.11.1");
        assert!(BASELINE_STATEMENT.contains("NOT SOCP"));
        assert!(BASELINE_STATEMENT.contains("NOT MISOCP"));
        assert!(BASELINE_STATEMENT.contains("a NEW baseline version, never a silent edit"));
    }

    #[test]
    fn registry_has_exactly_three_frozen_records() {
        let registry = baseline_registry();
        assert_eq!(registry.len(), 3);
        assert_eq!(registry[0].baseline_id, "baseline/highs-tier1-lp");
        assert_eq!(registry[1].baseline_id, "baseline/clarabel-tier2-conic");
        assert_eq!(registry[2].baseline_id, "baseline/rule-based-priority-list");
        assert_eq!(registry[0].version, "v1");
        assert_eq!(registry[1].version, "v1");
        assert_eq!(registry[2].version, "priority-list.v1");
        // The solver-class discipline VERBATIM (AGENTS.md rule 4): the
        // labels carry the negations, so no summary can drop them.
        assert!(registry[0].solver_class_label.contains("not SOCP"));
        assert!(registry[1].solver_class_label.contains("not MISOCP"));
        assert!(registry[2].solver_class_label.contains("no optimality claim"));
        // The heuristic names no solver (honesty: it wraps none).
        assert!(registry[2].frozen_solver_version.starts_with("n/a"));
        // Every record carries written assumptions (P8.06 step 4) and the
        // exact benchmark entry point.
        for record in &registry {
            assert!(!record.assumptions_and_limits.is_empty());
            assert_eq!(record.benchmark_entry_point, BENCHMARK_ENTRY_POINT);
        }
        assert!(BENCHMARK_ENTRY_POINT.contains("apps/sim-runner"));
        assert!(BENCHMARK_ENTRY_POINT.contains("scenarios/library"));
    }

    #[test]
    fn per_module_baseline_mapping_is_the_recorded_minimum() {
        assert_eq!(
            baselines_for_module(ResearchModule::AdmmDmpc),
            vec![
                "baseline/highs-tier1-lp",
                "baseline/clarabel-tier2-conic",
                "baseline/rule-based-priority-list",
            ]
        );
        assert_eq!(
            baselines_for_module(ResearchModule::GnnWarmStart),
            vec![
                "baseline/highs-tier1-lp",
                "baseline/rule-based-priority-list",
            ]
        );
        assert_eq!(
            baselines_for_module(ResearchModule::Marl),
            vec![
                "baseline/highs-tier1-lp",
                "baseline/rule-based-priority-list",
            ]
        );
    }

    // --- the canonical string and the freeze (known answers) ---------------

    /// A minimal record built IN THE TEST (not from the registry): short
    /// fields so the canonical string and its digest are exact known
    /// answers, hand-checkable character by character.
    fn minimal_record() -> BaselineRecord {
        BaselineRecord {
            baseline_id: "t/b1",
            version: "v1".to_string(),
            kind: BaselineKind::Tier1LpQpHighs,
            solver_class_label: "L",
            frozen_solver_version: "1.0",
            seed_policy: SeedPolicy::None,
            benchmark_entry_point: "e",
            provider_seam: "s",
            assumptions_and_limits: vec!["a1"],
        }
    }

    #[test]
    fn canonical_string_is_an_exact_known_answer() {
        // Hand-assembled from the recorded field order and `|`
        // separators: id, version, kind (Debug), label, solver, seed
        // record, entry, seam, assumptions joined with ";".
        assert_eq!(
            minimal_record().canonical_string(),
            "id:t/b1|version:v1|kind:Tier1LpQpHighs|label:L|solver:1.0|\
             seed:none (no randomness in the baseline)|entry:e|seam:s|\
             assumptions:a1"
        );
    }

    #[test]
    fn freeze_digest_is_an_exact_known_answer() {
        // FNV-1a 64 over the canonical string above (computed with an
        // independent implementation validated against the published
        // vectors in fnv1a64_matches_published_test_vectors).
        let frozen = FrozenBaseline::freeze(&minimal_record()).expect("assumptions present");
        assert_eq!(frozen.definition_digest, "d8fc16af0b376496");
        assert_eq!(frozen.registry_version, BASELINE_REGISTRY_VERSION);
        assert!(frozen.results_digest.is_none(), "no results until frozen WITH results");
    }

    #[test]
    fn freeze_is_deterministic_and_sensitive_to_content() {
        let a = FrozenBaseline::freeze(&minimal_record()).unwrap();
        let b = FrozenBaseline::freeze(&minimal_record()).unwrap();
        assert_eq!(a, b, "the same record freezes to the same digest");
        let mut changed = minimal_record();
        changed.assumptions_and_limits = vec!["a2"];
        let c = FrozenBaseline::freeze(&changed).unwrap();
        assert_ne!(a.definition_digest, c.definition_digest);
    }

    #[test]
    fn freeze_refuses_a_record_without_written_assumptions() {
        let mut naked = minimal_record();
        naked.assumptions_and_limits = Vec::new();
        let error = FrozenBaseline::freeze(&naked).expect_err("no assumptions, no freeze");
        assert_eq!(
            error,
            FreezeError::AssumptionsMissing {
                baseline_id: "t/b1".to_string()
            }
        );
    }

    #[test]
    fn version_bumps_are_the_only_legal_change() {
        let bumped = minimal_record().bump_version();
        assert_eq!(bumped.version, "v2");
        let mut nine = minimal_record();
        nine.version = "v9".to_string();
        assert_eq!(nine.bump_version().version, "v10");
        let mut weird = minimal_record();
        weird.version = "weird".to_string();
        // Unparseable versions get a suffix, never a silent reuse.
        assert_eq!(weird.bump_version().version, "weird.v2");
    }

    // --- the results digest and the reproduction check ----------------------

    #[test]
    fn results_digest_is_an_exact_known_answer() {
        // FNV-1a 64 over "alpha\nbeta" (validated implementation).
        assert_eq!(
            results_digest_for(&["alpha".to_string(), "beta".to_string()]),
            "a15677518e2469e9"
        );
        // The empty result set digests the empty string (the offset basis).
        assert_eq!(results_digest_for(&[]), "cbf29ce484222325");
    }

    #[test]
    fn reproduction_confirm_is_exact_and_mismatch_is_named() {
        let frozen = FrozenBaseline::freeze(&minimal_record())
            .unwrap()
            .with_results("a15677518e2469e9".to_string());
        frozen
            .confirm_reproduction("a15677518e2469e9")
            .expect("the recorded digest reproduces");
        let error = frozen
            .confirm_reproduction("0000000000000000")
            .expect_err("a deviation is named, never absorbed");
        assert_eq!(
            error,
            FreezeError::DigestMismatch {
                expected: "a15677518e2469e9".to_string(),
                observed: "0000000000000000".to_string(),
            }
        );
        // Without results, the reproduction check runs against the
        // definition digest.
        let definition_only = FrozenBaseline::freeze(&minimal_record()).unwrap();
        definition_only
            .confirm_reproduction(&definition_only.definition_digest)
            .expect("the definition digest reproduces");
    }

    // --- the priority-list rule set: hand-computed known answers -----------

    /// Convenience asset constructor (no energy limit).
    fn asset(id: &str, priority: u32, p_min: f64, p_max: f64) -> DispatchAsset {
        DispatchAsset {
            asset_id: id.to_string(),
            priority,
            p_min_mw: p_min,
            p_max_mw: p_max,
            energy_capacity_mwh: None,
            soc_fraction: None,
        }
    }

    fn battery(id: &str, priority: u32, p_min: f64, p_max: f64, cap_mwh: f64, soc: f64) -> DispatchAsset {
        DispatchAsset {
            asset_id: id.to_string(),
            priority,
            p_min_mw: p_min,
            p_max_mw: p_max,
            energy_capacity_mwh: Some(cap_mwh),
            soc_fraction: Some(soc),
        }
    }

    fn demand(mw: f64) -> IntervalDemand {
        IntervalDemand { net_demand_mw: mw }
    }

    #[test]
    fn dispatch_commits_in_priority_order_and_fills_the_remainder() {
        // Demand 10 MW, 1 h intervals. A1 (priority 1) caps at 6 MW, A2
        // (priority 2) caps at 5 MW: A1 commits 6.0, A2 covers the 4.0
        // remainder. Hand-computed.
        let input = PriorityListInput {
            interval_hours: 1.0,
            intervals: vec![demand(10.0)],
            assets: vec![asset("a1", 1, 0.0, 6.0), asset("a2", 2, 0.0, 5.0)],
        };
        let result = priority_list_dispatch(&input).expect("valid input");
        let interval = &result.intervals[0];
        assert_eq!(interval.dispatched.len(), 2);
        assert_eq!(interval.dispatched[0].asset_id, "a1");
        assert_eq!(interval.dispatched[0].p_mw, 6.0);
        assert_eq!(interval.dispatched[1].asset_id, "a2");
        assert_eq!(interval.dispatched[1].p_mw, 4.0);
        assert_eq!(interval.supplied_mw, 10.0);
        assert_eq!(interval.deficit_mw, 0.0);
        assert_eq!(result.total_supplied_mwh, 10.0);
        assert_eq!(result.total_deficit_mwh, 0.0);
        assert_eq!(result.baseline_version, "priority-list.v1");
    }

    #[test]
    fn dispatch_orders_by_priority_then_asset_id() {
        // Given in scrambled order: z and a share priority 1, m has
        // priority 0. Order: m, a, z (priority ASC, then id ASC). Demand
        // 5 MW with 4 MW caps: m commits 4.0, a covers 1.0, z never runs.
        let input = PriorityListInput {
            interval_hours: 1.0,
            intervals: vec![demand(5.0)],
            assets: vec![
                asset("z", 1, 0.0, 4.0),
                asset("a", 1, 0.0, 4.0),
                asset("m", 0, 0.0, 4.0),
            ],
        };
        let result = priority_list_dispatch(&input).expect("valid input");
        let dispatched: Vec<&str> = result.intervals[0]
            .dispatched
            .iter()
            .map(|d| d.asset_id.as_str())
            .collect();
        assert_eq!(dispatched, vec!["m", "a"]);
        assert_eq!(result.intervals[0].dispatched[0].p_mw, 4.0);
        assert_eq!(result.intervals[0].dispatched[1].p_mw, 1.0);
    }

    #[test]
    fn energy_limited_asset_respects_its_budget_and_skips_below_p_min() {
        // Battery: 4.0 MWh capacity, SOC 0.5 -> 2.0 MWh available; p_min
        // 0.5 MW, p_max 5.0 MW, priority 1. Peaker: p_min 0, p_max 5,
        // priority 2. Two 3 MW, 1 h intervals. Hand-computed:
        // interval 0: battery cap = 2.0/1.0 = 2.0 MW -> commits 2.0
        //   (energy used 2.0 MWh); peaker covers the 1.0 remainder.
        // interval 1: available = 2.0 - 2.0 = 0 -> cap 0.0 MW; candidate
        //   0.0 < p_min 0.5 -> SKIPPED (never overshoot); peaker commits 3.0.
        let input = PriorityListInput {
            interval_hours: 1.0,
            intervals: vec![demand(3.0), demand(3.0)],
            assets: vec![
                battery("bat", 1, 0.5, 5.0, 4.0, 0.5),
                asset("pk", 2, 0.0, 5.0),
            ],
        };
        let result = priority_list_dispatch(&input).expect("valid input");
        let i0 = &result.intervals[0];
        assert_eq!(i0.dispatched.len(), 2);
        assert_eq!(i0.dispatched[0].asset_id, "bat");
        assert_eq!(i0.dispatched[0].p_mw, 2.0);
        assert_eq!(i0.dispatched[1].asset_id, "pk");
        assert_eq!(i0.dispatched[1].p_mw, 1.0);
        let i1 = &result.intervals[1];
        assert_eq!(i1.dispatched.len(), 1, "the battery is skipped, not dispatched");
        assert_eq!(i1.dispatched[0].asset_id, "pk");
        assert_eq!(i1.dispatched[0].p_mw, 3.0);
        assert_eq!(result.total_supplied_mwh, 6.0);
        assert_eq!(result.total_deficit_mwh, 0.0);
    }

    #[test]
    fn multi_interval_energy_depletes_across_the_horizon() {
        // 0.5 h intervals, battery 3.0 MWh full, p_max 6.0, p_min 0,
        // priority 1; generator p_max 4.0, priority 2. Demands [4, 4] MW.
        // Hand-computed: interval 0 cap = 3.0/0.5 = 6.0 MW -> battery
        // commits 4.0 (2.0 MWh used); interval 1 available 1.0 MWh ->
        // cap 2.0 MW -> battery commits 2.0, generator covers 2.0.
        let input = PriorityListInput {
            interval_hours: 0.5,
            intervals: vec![demand(4.0), demand(4.0)],
            assets: vec![
                battery("bat", 1, 0.0, 6.0, 3.0, 1.0),
                asset("g", 2, 0.0, 4.0),
            ],
        };
        let result = priority_list_dispatch(&input).expect("valid input");
        let i0 = &result.intervals[0];
        assert_eq!(i0.dispatched.len(), 1);
        assert_eq!(i0.dispatched[0].p_mw, 4.0);
        let i1 = &result.intervals[1];
        assert_eq!(i1.dispatched.len(), 2);
        assert_eq!(i1.dispatched[0].asset_id, "bat");
        assert_eq!(i1.dispatched[0].p_mw, 2.0);
        assert_eq!(i1.dispatched[1].asset_id, "g");
        assert_eq!(i1.dispatched[1].p_mw, 2.0);
        // (4.0 MW + 4.0 MW) x 0.5 h = 4.0 MWh.
        assert_eq!(result.total_supplied_mwh, 4.0);
        assert_eq!(result.total_deficit_mwh, 0.0);
    }

    #[test]
    fn an_exhausted_battery_with_p_min_zero_commits_an_explicit_zero() {
        // PINNED RULE SEMANTICS (documented, deliberate): with p_min 0 the
        // rule 3c skip does not fire at a 0.0 candidate, so an exhausted
        // energy-limited asset appears with an explicit 0.0 commit entry.
        // Two 1 MW, 1 h intervals; battery 1.0 MWh full, p_min 0, p_max 5.
        // interval 0: commits 1.0 (battery emptied); interval 1: cap 0.0,
        // candidate 0.0, p_min 0.0 -> commits an explicit 0.0; deficit 1.0.
        let input = PriorityListInput {
            interval_hours: 1.0,
            intervals: vec![demand(1.0), demand(1.0)],
            assets: vec![battery("bat", 1, 0.0, 5.0, 1.0, 1.0)],
        };
        let result = priority_list_dispatch(&input).expect("valid input");
        assert_eq!(result.intervals[0].dispatched[0].p_mw, 1.0);
        let i1 = &result.intervals[1];
        assert_eq!(i1.dispatched.len(), 1);
        assert_eq!(i1.dispatched[0].asset_id, "bat");
        assert_eq!(i1.dispatched[0].p_mw, 0.0);
        assert_eq!(i1.supplied_mw, 0.0);
        assert_eq!(i1.deficit_mw, 1.0);
        assert_eq!(result.total_supplied_mwh, 1.0);
        assert_eq!(result.total_deficit_mwh, 1.0);
    }

    #[test]
    fn under_supply_is_an_honest_recorded_deficit() {
        // Demand 10 MW, one 4 MW asset: supplied 4.0, deficit 6.0 — never
        // silently filled.
        let input = PriorityListInput {
            interval_hours: 1.0,
            intervals: vec![demand(10.0)],
            assets: vec![asset("only", 1, 0.0, 4.0)],
        };
        let result = priority_list_dispatch(&input).expect("valid input");
        assert_eq!(result.intervals[0].supplied_mw, 4.0);
        assert_eq!(result.intervals[0].deficit_mw, 6.0);
        assert_eq!(result.total_deficit_mwh, 6.0);
    }

    #[test]
    fn the_rules_never_overshoot_to_reach_a_minimum() {
        // Demand 1 MW; the only asset declares p_min 2.0: committing would
        // overshoot, so it is skipped and the interval records the deficit.
        let input = PriorityListInput {
            interval_hours: 1.0,
            intervals: vec![demand(1.0)],
            assets: vec![asset("big-min", 1, 2.0, 5.0)],
        };
        let result = priority_list_dispatch(&input).expect("valid input");
        assert!(result.intervals[0].dispatched.is_empty());
        assert_eq!(result.intervals[0].supplied_mw, 0.0);
        assert_eq!(result.intervals[0].deficit_mw, 1.0);
    }

    #[test]
    fn an_empty_horizon_dispatches_nothing() {
        let input = PriorityListInput {
            interval_hours: 1.0,
            intervals: Vec::new(),
            assets: vec![asset("a", 1, 0.0, 5.0)],
        };
        let result = priority_list_dispatch(&input).expect("an empty horizon is legal");
        assert!(result.intervals.is_empty());
        assert_eq!(result.total_supplied_mwh, 0.0);
        assert_eq!(result.total_deficit_mwh, 0.0);
    }

    // --- rule 1: input validation (fail-closed, named, first wins) ---------

    #[test]
    fn invalid_inputs_are_refused_with_named_errors() {
        let base = |intervals: Vec<IntervalDemand>, assets: Vec<DispatchAsset>| PriorityListInput {
            interval_hours: 1.0,
            intervals,
            assets,
        };
        // Non-positive interval duration.
        let mut input = base(vec![demand(1.0)], vec![asset("a", 1, 0.0, 1.0)]);
        input.interval_hours = 0.0;
        assert_eq!(
            priority_list_dispatch(&input).unwrap_err(),
            DispatchInputError::NonPositiveIntervalHours { value: 0.0 }
        );
        // Negative demand (the rules serve loads, never absorb them).
        let input = base(vec![demand(1.0), demand(-2.0)], vec![]);
        assert_eq!(
            priority_list_dispatch(&input).unwrap_err(),
            DispatchInputError::NegativeDemand { index: 1, value: -2.0 }
        );
        // Empty asset id.
        let input = base(vec![], vec![asset("", 1, 0.0, 1.0)]);
        assert_eq!(
            priority_list_dispatch(&input).unwrap_err(),
            DispatchInputError::EmptyAssetId { index: 0 }
        );
        // p_min above p_max.
        let input = base(vec![], vec![asset("a", 1, 3.0, 2.0)]);
        assert_eq!(
            priority_list_dispatch(&input).unwrap_err(),
            DispatchInputError::PMinAbovePMax {
                index: 0,
                asset_id: "a".to_string(),
                p_min_mw: 3.0,
                p_max_mw: 2.0,
            }
        );
        // Negative energy capacity.
        let input = base(vec![], vec![battery("bat", 1, 0.0, 1.0, -1.0, 0.5)]);
        assert_eq!(
            priority_list_dispatch(&input).unwrap_err(),
            DispatchInputError::InvalidEnergyCapacity {
                index: 0,
                asset_id: "bat".to_string(),
                value: -1.0,
            }
        );
        // SOC outside [0, 1] (§10.2: a fraction, never clipped).
        let input = base(vec![], vec![battery("bat", 1, 0.0, 1.0, 1.0, 1.5)]);
        assert_eq!(
            priority_list_dispatch(&input).unwrap_err(),
            DispatchInputError::SocOutOfRange {
                index: 0,
                asset_id: "bat".to_string(),
                value: 1.5,
            }
        );
    }

    #[test]
    fn non_finite_inputs_are_refused_not_absorbed() {
        let mut input = PriorityListInput {
            interval_hours: 1.0,
            intervals: vec![demand(f64::NAN)],
            assets: vec![asset("a", 1, 0.0, 1.0)],
        };
        assert_eq!(
            priority_list_dispatch(&input).unwrap_err(),
            DispatchInputError::NonFiniteDemand { index: 0 }
        );
        input.intervals.clear();
        input.assets = vec![asset("a", 1, f64::INFINITY, 1.0)];
        assert_eq!(
            priority_list_dispatch(&input).unwrap_err(),
            DispatchInputError::NonFinitePower {
                index: 0,
                asset_id: "a".to_string(),
                field: "p_min_mw",
            }
        );
        input.assets = vec![asset("a", 1, 0.0, f64::NEG_INFINITY)];
        assert_eq!(
            priority_list_dispatch(&input).unwrap_err(),
            DispatchInputError::NonFinitePower {
                index: 0,
                asset_id: "a".to_string(),
                field: "p_max_mw",
            }
        );
    }

    #[test]
    fn the_first_validation_failure_wins_in_recorded_order() {
        // Interval-hours validity is checked before demands and assets:
        // with BOTH an invalid duration and a negative demand, the named
        // error is the duration one.
        let input = PriorityListInput {
            interval_hours: -1.0,
            intervals: vec![demand(-5.0)],
            assets: vec![asset("", 1, -1.0, -2.0)],
        };
        assert_eq!(
            priority_list_dispatch(&input).unwrap_err(),
            DispatchInputError::NonPositiveIntervalHours { value: -1.0 }
        );
    }

    #[test]
    fn every_input_error_renders_non_empty_prose() {
        let errors = [
            DispatchInputError::NonPositiveIntervalHours { value: 0.0 },
            DispatchInputError::NonFiniteDemand { index: 0 },
            DispatchInputError::NegativeDemand { index: 0, value: -1.0 },
            DispatchInputError::EmptyAssetId { index: 0 },
            DispatchInputError::NonFinitePower {
                index: 0,
                asset_id: "a".to_string(),
                field: "p_min_mw",
            },
            DispatchInputError::NegativePower {
                index: 0,
                asset_id: "a".to_string(),
                field: "p_max_mw",
            },
            DispatchInputError::PMinAbovePMax {
                index: 0,
                asset_id: "a".to_string(),
                p_min_mw: 2.0,
                p_max_mw: 1.0,
            },
            DispatchInputError::InvalidEnergyCapacity {
                index: 0,
                asset_id: "a".to_string(),
                value: -1.0,
            },
            DispatchInputError::SocOutOfRange {
                index: 0,
                asset_id: "a".to_string(),
                value: 2.0,
            },
        ];
        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }

    // --- the composition seam ----------------------------------------------

    #[test]
    fn optimizer_port_seam_is_recorded_verbatim() {
        assert!(OPTIMIZER_PORT_ALIGNMENT_SEAM.contains("flexcore-optimization-api/src/port.rs"));
        assert!(OPTIMIZER_PORT_ALIGNMENT_SEAM.contains("Optimizer"));
        assert!(OPTIMIZER_PORT_ALIGNMENT_SEAM.contains("six-status"));
    }

    #[test]
    fn seed_policies_record_their_honest_form() {
        assert_eq!(
            SeedPolicy::None.as_record(),
            "none (no randomness in the baseline)"
        );
        assert!(SeedPolicy::Fixed.as_record().contains("P7.42"));
        assert!(SeedPolicy::Fixed.as_record().contains("thread_count pinned 1"));
    }
}
