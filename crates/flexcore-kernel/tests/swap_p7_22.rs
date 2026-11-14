// P7.22 — Swap test: replace one optimizer and one grid provider without
//         touching application/domain code (report §23 Stage 7 exit gate's
//         first half, §5.2, §12.3–§12.4; ledger `ledger_phase_7.md` §P7.22).
//         WRITTEN-UNRUN: the loop bans hold; the human build/test pass
//         executes `cargo test -p flexcore-kernel --test swap_p7_22`.
//
// HONEST STATUS — the P7.28 exit-gate input: THIS TEST IS THE CORE PROOF
// WRITTEN-UNRUN. The §23 Stage 7 exit gate requires a WITNESSED run (the
// P7.28/P7.29 records: an independent witness re-runs the swap from the
// record). Nothing here has been executed; the witnessed run is a HUMAN
// GATE input, and the tick line reads "implemented, awaiting human
// build/test" — never "tested". The full §23 Stage 7 scenario re-run (the
// §24 slice with recorded timings and a `git diff` over application/domain
// code) is executed at that witnessed run; this test proves the SEAM
// PROPERTY the gate hinges on.
//
// WHAT THE TEST PROVES (the seam property, per ledger P7.22):
//
//   With the P7.08 registry and the P7.07 lifecycle composed, swapping the
//   optimizer provider (HighsTier1Provider → ClarabelTier2Provider) and a
//   second grid-model provider (PandapowerGridProvider → an OpenDSS-class
//   adapter) requires ZERO edits to application/domain code: the swap is
//   performed ONLY through `ProviderRegistry::swap_provider` + the
//   lifecycle's drain-before-replace, and the application consumer —
//   defined ONCE, before any swap — keeps working unchanged.
//
// Assertions:
//   (a) the new provider answers through the SAME port trait (`dyn
//       Solver` / `dyn GridPort`); the consumer's signature is untouched;
//   (b) the independent feasibility validation (ADR-009 baseline:
//       deterministic optimizer + independent feasibility validation, per
//       `ledger/phase-07-plugin-sdk/PLAN.md` §"ADR-009") still runs on the
//       swapped optimizer's output — in THIS file as the test-local mirror
//       `independent_feasibility_validation` that consumes the port-trait
//       output of BOTH the incumbent and the swapped provider; the REAL
//       application seam (`flexcore_application::command_gates::
//       run_validation_gate` / `run_feasibility_gate` over the
//       `CommandFeasibilityRunner`/`GridModelProvider` chain) is NOT
//       callable from this test crate — that is a recorded SEAM GAP with
//       the minimal exposure named in the decision record §"seam gaps"
//       (no source file was edited to bridge it);
//   (c) solver-class discipline intact: HiGHS is LP/QP/MIP and NOT SOCP;
//       Clarabel is convex conic (continuous only) and NOT MISOCP
//       (AGENTS.md §3 rule 4; seam inventory §4) — the labels are carried
//       through the swap by the port trait itself, and the swap is only
//       exercised where the formulation fits the new solver's class (the
//       MILP formulation is refused by the Class B double — a Clarabel
//       swap on a MILP formulation would be the class-mismatch failure
//       mode the ledger names, so it is asserted impossible here);
//   (d) the audit trail shows the version change AND the drain ordering:
//       refusal (swap while active) → Ready→Draining → Draining→Stopped →
//       VERSION_CHANGE, in emission order.
//
// The swap goes through the lifecycle — the blocked-then-allowed path is
// asserted directly: a swap while the incumbent is Active (Ready) is
// REFUSED (`REPLACEMENT_BLOCKED_UNDRAINED` via `LIFECYCLE_REFUSED`), the
// same refusal holds mid-drain, and only a fully drained (Stopped)
// incumbent can be replaced. There is no direct registry mutation while
// Active because the ONLY replacement API is `swap_provider`.
//
// PRE-DECLARED EQUIVALENCE CRITERIA (ledger P7.22 step 2 — declared BEFORE
// either swap, in the fixture constants below):
//   1. the same feasibility verdict for the LP fixture problem;
//   2. the same schedule objective within tolerance 0 (basis: the fixture
//      LP has integral data and a unique exact optimum — the tolerance is
//      exact equality on the fixture, not a loose band);
//   3. the same §9.2 result completeness (provenance AND diagnostics
//      present in the result record).
//   Timing is compared as a MEASUREMENT at the witnessed run, never as an
//   equivalence criterion (the ledger's recorded rule).
//
// Provider honesty (the P7.08 record's in-test rule): the kernel crate
// depends on no `flexcore-*` crate, so the REAL `HighsTier1Provider`,
// `ClarabelTier2Provider`, and `PandapowerGridProvider` cannot be
// constructed inside this test file. The doubles below are named after the
// real providers via the registry's seed constants and carry the seam
// inventory's §4 solver-class declarations; the REAL registration is a
// composition-root line through the same `FnFactory` adapter, and the
// real-provider witnessed run is the P7.28 human gate (see the decision
// record §"seam gaps" for the exact Cargo.toml exposure that enables it).

use std::sync::Arc;

use flexcore_kernel::lifecycle::{
    DrainOutcome, DrainPort, DrainPolicy, FixedClock, GateOutcome, LifecycleAuditKind,
    LifecycleState, MemoryAuditSink, PluginLifecycle,
};
use flexcore_kernel::manifest::{parse_manifest_value, Manifest};
use flexcore_kernel::registry::{
    FnFactory, ProviderFactory, ProviderRegistry, RegistryError, seed,
};
use flexcore_kernel::resolve::{resolve, CompositionPlan, ExplicitSelections};

// --- Local port traits (the seams application code depends on) ----------------------
//
// These mirror the SHAPE of the real port seams (`flexcore-optimization-api
// ::port::Optimizer`, `flexcore-grid-api::port::GridModelProvider`) at the
// triviality level the P7.08 tests established. The consumer functions
// below depend ONLY on these traits — that dependency shape is the thing
// the swap must not break.

/// The optimizer seam (`Optimizer`, seam inventory §3.1) — test projection.
trait Solver {
    /// The implementation name (seam-inventory name for first-party providers).
    fn provider_name(&self) -> &'static str;
    /// The §7.4 solver-class declaration (AGENTS.md rule 4: never conflated).
    fn solver_class(&self) -> &'static str;
    /// Whether the declared class supports `problem_class` (§9.1
    /// `supported_problem_classes` maps to this in the real port).
    fn supports(&self, problem_class: &str) -> bool;
    /// Solve one fixture problem; `None` = the class does not support it.
    fn solve(&self, problem_class: &str) -> Option<Candidate>;
}

/// One optimizer result record (the §9.2 completeness surface the
/// equivalence criteria check: provenance AND diagnostics, always).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Candidate {
    objective: i64,
    provenance: &'static str,
    diagnostics: &'static str,
}

/// The grid-model seam (`GridModelProvider`, seam inventory §3.2) — test
/// projection. The trait doc pins what the real port pins: an AC
/// power-flow evaluation, NOT an AC-OPF optimization.
trait GridPort {
    fn provider_name(&self) -> &'static str;
    /// The declared analysis class (fidelity label; D3 governs changes).
    fn analysis_class(&self) -> &'static str;
    /// Evaluate one dispatch against the model.
    fn evaluate_dispatch(&self, injections: &str) -> Result<GridVerdict, &'static str>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GridVerdict {
    verdict: &'static str,
    provenance: &'static str,
}

// --- The §7.4 solver-class declarations (seam inventory §4, mirrored) ---------------

const HIGHS_CLASS_DECLARATION: &str = "LP/QP/MIP (MILP) — Tier 1; not SOCP, no conic capability claimed";
const CLARABEL_CLASS_DECLARATION: &str =
    "convex conic (LP/QP/SOCP) — Tier 2, continuous only; not MISOCP, no integer variables";
const GRID_ANALYSIS_CLASS: &str = "balanced AC power-flow evaluation — NOT an AC-OPF optimization";

const PROBLEM_LP: &str = "linear-program";
const PROBLEM_MILP: &str = "mixed-integer-program";

// --- Local doubles named after the real providers ------------------------------------

/// Local stand-in for the REAL `HighsTier1Provider` (LP/QP/MIP tier 1).
struct HighsTier1Like;

impl Solver for HighsTier1Like {
    fn provider_name(&self) -> &'static str {
        seed::PROVIDER_HIGHS_TIER1
    }
    fn solver_class(&self) -> &'static str {
        HIGHS_CLASS_DECLARATION
    }
    fn supports(&self, problem_class: &str) -> bool {
        matches!(problem_class, PROBLEM_LP | PROBLEM_MILP)
    }
    fn solve(&self, problem_class: &str) -> Option<Candidate> {
        if !self.supports(problem_class) {
            return None;
        }
        Some(Candidate {
            objective: -42,
            provenance: "provider=builtin.highs-optimizer model=solver-pin-1.15.1 termination=optimal",
            diagnostics: "fixture LP: integral data, unique optimum",
        })
    }
}

/// Local stand-in for the REAL `ClarabelTier2Provider` (convex conic
/// tier 2 — continuous only, never MISOCP).
struct ClarabelTier2Like;

impl Solver for ClarabelTier2Like {
    fn provider_name(&self) -> &'static str {
        seed::PROVIDER_CLARABEL_TIER2
    }
    fn solver_class(&self) -> &'static str {
        CLARABEL_CLASS_DECLARATION
    }
    fn supports(&self, problem_class: &str) -> bool {
        // Continuous convex problems only: LP fits; MILP does NOT
        // (no integer variables — the class-mismatch case the ledger names).
        problem_class == PROBLEM_LP
    }
    fn solve(&self, problem_class: &str) -> Option<Candidate> {
        if !self.supports(problem_class) {
            return None;
        }
        Some(Candidate {
            objective: -42,
            provenance: "provider=builtin.highs-optimizer model=solver-pin-0.11.1 termination=solved",
            diagnostics: "fixture LP: conic solve, no integer variables",
        })
    }
}

/// Local stand-in for the REAL `PandapowerGridProvider`.
struct PandapowerGridLike;

impl GridPort for PandapowerGridLike {
    fn provider_name(&self) -> &'static str {
        seed::PROVIDER_PANDAPOWER_GRID
    }
    fn analysis_class(&self) -> &'static str {
        GRID_ANALYSIS_CLASS
    }
    fn evaluate_dispatch(&self, _injections: &str) -> Result<GridVerdict, &'static str> {
        Ok(GridVerdict {
            verdict: "within-limits",
            provenance: "provider=builtin.pandapower-grid model=fixture-network-1",
        })
    }
}

/// Local stand-in for the planned OpenDSS-class adapter (the ledger's
/// grid-provider swap example; D3 fidelity is recorded at the swap).
struct OpenDssGridLike;

impl GridPort for OpenDssGridLike {
    fn provider_name(&self) -> &'static str {
        "OpenDssGridProviderLike (planned-adapter stand-in)"
    }
    fn analysis_class(&self) -> &'static str {
        GRID_ANALYSIS_CLASS
    }
    fn evaluate_dispatch(&self, _injections: &str) -> Result<GridVerdict, &'static str> {
        Ok(GridVerdict {
            verdict: "within-limits",
            provenance: "provider=opendss-like-adapter model=fixture-network-1",
        })
    }
}

// --- THE APPLICATION CONSUMER (defined ONCE — never touched by any swap) -------------
//
// These functions stand in for the application/domain consumers (the
// scheduling use case and the feasibility gate consumption). They depend
// only on the port traits. THE SWAP CHANGES NOTHING IN THIS SECTION — that
// is the property under test; a code change here would be the ledger's
// named failure mode ("a swap that actually edits the scheduling use case").

/// The scheduling use case (application-side consumer of `dyn Solver`).
fn scheduling_use_case(optimizer: &dyn Solver, problem_class: &str) -> String {
    match optimizer.solve(problem_class) {
        Some(candidate) => format!(
            "schedule from {} ({}): objective {}",
            optimizer.provider_name(),
            optimizer.solver_class(),
            candidate.objective
        ),
        None => format!(
            "no schedule: {} does not support {problem_class} ({})",
            optimizer.provider_name(),
            optimizer.solver_class()
        ),
    }
}

/// THE ADR-009 INDEPENDENT FEASIBILITY VALIDATION (test-local mirror).
///
/// The baseline (ADR-009): the optimizer is deterministic, and its output
/// is validated by an INDEPENDENT feasibility check — not by the optimizer
/// itself, and never skipped because "the solver said so". In the real
/// platform this is the `flexcore-application` gate chain
/// (`command_gates::run_validation_gate` / `run_feasibility_gate` over
/// `CommandFeasibilityRunner` → `GridModelProvider::evaluate_dispatch`);
/// here it is the same SHAPE over the local port traits: it consumes the
/// candidate a provider produced and reaches its own verdict through the
/// GRID port (the independent evaluator), never through the optimizer.
/// This function is defined ONCE and used for BOTH the incumbent and the
/// swapped provider — the swap must not touch it (that is assertion (b)).
fn independent_feasibility_validation(
    candidate: &Candidate,
    grid: &dyn GridPort,
    injections: &str,
) -> Result<&'static str, String> {
    // Completeness first (§9.2): a result without provenance and
    // diagnostics is not eligible for validation at all.
    if candidate.provenance.is_empty() || candidate.diagnostics.is_empty() {
        return Err("result incomplete: provenance and diagnostics are required (§9.2)".to_string());
    }
    // The independent check: the grid provider evaluates the candidate's
    // dispatch. The optimizer's own verdict is NOT consulted.
    match grid.evaluate_dispatch(injections) {
        Ok(grid_verdict) if grid_verdict.verdict == "within-limits" => Ok("feasible"),
        Ok(other) => Err(format!("network limit: {}", other.verdict)),
        Err(reason) => Err(format!("fail-closed: {reason}")),
    }
}

// --- Plan fixtures (through the real P7.05 validator + P7.06 resolver) ---------------

fn manifest_json(plugin_id: &str, version: &str, provides: &[&str]) -> serde_json::Value {
    serde_json::json!({
        "manifest-schema": "flexcore-plugin-manifest-v1",
        "plugin-id": plugin_id,
        "name": "Swap test plugin",
        "version": version,
        "api-version": ">=0.5,<1.0",
        "class": "builtin-rust",
        "entry-point": { "registered-type": "test/type" },
        "provider-of": provides,
        "consumes": [],
        "permissions": [],
        "configuration-schema": { "type": "object", "additionalProperties": false },
        "resources": { "max-memory-mb": 64, "timeout-ms": 1000 },
        "metadata": { "license": "Apache-2.0", "description": "swap test fixture" }
    })
}

fn manifest(plugin_id: &str, version: &str, provides: &[&str]) -> Manifest {
    parse_manifest_value(&manifest_json(plugin_id, version, provides))
        .expect("swap test fixture must be a valid manifest")
}

/// The seed composition: the optimizer seam and the grid-model seam, one
/// first-party provider each (the DEFAULT-labelled seed ids).
fn seed_plan() -> CompositionPlan {
    let optimizer = manifest(seed::PLUGIN_OPTIMIZER_DEFAULT, "0.1.0", &[seed::CONTRACT_OPTIMIZER]);
    let grid = manifest(seed::PLUGIN_GRID_MODEL_DEFAULT, "0.1.0", &[seed::CONTRACT_GRID_MODEL]);
    resolve(&[optimizer, grid], &ExplicitSelections::new()).expect("seed plan resolves")
}

fn highs_factory() -> Box<dyn ProviderFactory<dyn Solver>> {
    Box::new(FnFactory::new(seed::PROVIDER_HIGHS_TIER1, || {
        Ok(Arc::new(HighsTier1Like) as Arc<dyn Solver>)
    }))
}

fn clarabel_factory() -> Box<dyn ProviderFactory<dyn Solver>> {
    Box::new(FnFactory::new(seed::PROVIDER_CLARABEL_TIER2, || {
        Ok(Arc::new(ClarabelTier2Like) as Arc<dyn Solver>)
    }))
}

fn pandapower_factory() -> Box<dyn ProviderFactory<dyn GridPort>> {
    Box::new(FnFactory::new(seed::PROVIDER_PANDAPOWER_GRID, || {
        Ok(Arc::new(PandapowerGridLike) as Arc<dyn GridPort>)
    }))
}

fn opendss_factory() -> Box<dyn ProviderFactory<dyn GridPort>> {
    Box::new(FnFactory::new("OpenDssGridProviderLike (planned-adapter stand-in)", || {
        Ok(Arc::new(OpenDssGridLike) as Arc<dyn GridPort>)
    }))
}

// --- The drain port double -------------------------------------------------------------

/// A cooperative drain port: quiesces immediately (the fixture's incumbent
/// has outstanding work cancelled cleanly within the deadline).
struct CooperativeDrain {
    outstanding: usize,
}

impl DrainPort for CooperativeDrain {
    fn outstanding_work(&self) -> usize {
        self.outstanding
    }
    fn drain(&mut self, _deadline_millis: u64) -> DrainOutcome {
        let cancelled = self.outstanding;
        self.outstanding = 0;
        DrainOutcome::Quiesced { cancelled }
    }
}

// --- The pre-declared equivalence criteria (ledger P7.22 step 2) -----------------------

/// The pre-declared objective tolerance and its basis: the fixture LP has
/// integral data and a unique exact optimum, so the tolerance is exact
/// equality (0). Declared BEFORE either swap (module constants are the
/// declaration).
const OBJECTIVE_TOLERANCE: i64 = 0;
const OBJECTIVE_TOLERANCE_BASIS: &str =
    "fixture LP with integral data and a unique exact optimum; tolerance = exact equality";

fn assert_equivalence_criteria(optimizer: &dyn Solver, grid: &dyn GridPort) {
    // Criterion 1 + 2: same feasibility verdict and same objective within
    // the pre-declared tolerance, for the SAME (LP) fixture problem.
    let candidate = optimizer
        .solve(PROBLEM_LP)
        .expect("the fixture LP is supported by BOTH swap participants — this is the only swap the class discipline permits");
    let verdict = independent_feasibility_validation(&candidate, grid, "fixture-injections")
        .expect("the independent validation runs on the swapped output");
    assert_eq!(verdict, "feasible");
    assert!(
        (candidate.objective - -42).abs() <= OBJECTIVE_TOLERANCE,
        "objective within the pre-declared tolerance (basis: {OBJECTIVE_TOLERANCE_BASIS})"
    );
    // Criterion 3: §9.2 result completeness (provenance AND diagnostics).
    assert!(!candidate.provenance.is_empty() && !candidate.diagnostics.is_empty());
}

// --- The swap scenario -------------------------------------------------------------------

/// Drives one plugin's lifecycle from registration to Ready (Active).
fn drive_to_ready(lifecycle: &mut PluginLifecycle<'_>) {
    lifecycle.verify(GateOutcome::Passed, "swap-test", "provenance gate passes").expect("verified");
    lifecycle
        .mark_compatible(GateOutcome::Passed, "swap-test", "compatible")
        .expect("compatible");
    lifecycle
        .configure(GateOutcome::Passed, "swap-test", "configured")
        .expect("configured");
    lifecycle.start(GateOutcome::Passed, "swap-test", "plan resolved").expect("started");
    lifecycle.mark_ready(GateOutcome::Passed, "swap-test", "health passes").expect("ready");
    // The drain port is attached when the drain begins (§5.6), not here —
    // the machine's own rule.
}

#[test]
fn the_optimizer_swap_never_touches_application_or_domain_code() {
    let plan = seed_plan();
    let clock = FixedClock::new(10_000);
    let audit = MemoryAuditSink::new();

    // The composition: registry + lifecycles, registered from the plan.
    let mut registry: ProviderRegistry<dyn Solver> = ProviderRegistry::new(&plan);
    let mut factories = std::collections::BTreeMap::new();
    factories.insert(seed::CONTRACT_OPTIMIZER.to_string(), highs_factory());
    registry
        .register_from_plan(&plan, factories)
        .expect("the seed composition registers completely");

    // The drain port is declared BEFORE the lifecycle (it is borrowed for
    // the lifecycle's lifetime once the drain begins).
    let mut optimizer_drain = CooperativeDrain { outstanding: 2 };
    // The lifecycle for the optimizer plugin (identity = the plan's
    // registered provider of the contract).
    let mut optimizer_lifecycle = PluginLifecycle::register(
        seed::PLUGIN_OPTIMIZER_DEFAULT,
        "0.1.0",
        &clock,
        &audit,
        DrainPolicy::default(),
    );
    drive_to_ready(&mut optimizer_lifecycle);

    // BASELINE (ledger step 1): the incumbent answers; the application
    // consumer and the ADR-009 independent validation run unchanged.
    let incumbent = registry.construct(seed::CONTRACT_OPTIMIZER).expect("constructed");
    assert_eq!(incumbent.provider_name(), seed::PROVIDER_HIGHS_TIER1);
    let baseline_schedule = scheduling_use_case(incumbent.as_ref(), PROBLEM_LP);
    assert!(baseline_schedule.contains(seed::PROVIDER_HIGHS_TIER1));

    // (b) ADR-009: the INDEPENDENT validation runs on the incumbent's
    // output — the optimizer's own word is never the verdict.
    let grid: PandapowerGridLike = PandapowerGridLike;
    let incumbent_candidate = incumbent.solve(PROBLEM_LP).expect("LP supported");
    assert_eq!(
        independent_feasibility_validation(&incumbent_candidate, &grid, "fixture-injections")
            .expect("incumbent output validates"),
        "feasible"
    );

    // BLOCKED (the no-direct-mutation rule): a swap while the incumbent is
    // Active (Ready) is REFUSED with the named drain-before-replace error.
    let blocked = registry.swap_provider(
        seed::CONTRACT_OPTIMIZER,
        clarabel_factory(),
        &mut optimizer_lifecycle,
        "0.2.0",
        "swap-test",
        "attempted swap while the incumbent is Active",
    );
    match &blocked {
        Err(RegistryError::LifecycleRefused { source, .. }) => {
            assert_eq!(source.name(), "REPLACEMENT_BLOCKED_UNDRAINED");
        }
        other => panic!("expected LIFECYCLE_REFUSED (undrained), got {other:?}"),
    }
    // The refusal left the incumbent in place and Active.
    assert_eq!(optimizer_lifecycle.state(), LifecycleState::Ready);
    assert_eq!(
        registry.construct(seed::CONTRACT_OPTIMIZER).expect("constructed").provider_name(),
        seed::PROVIDER_HIGHS_TIER1
    );
    // The refusal is audited (require_fully_drained emits the record).
    assert!(audit.records().iter().any(|r| r.kind == LifecycleAuditKind::Refused));

    // DRAIN-BEFORE-REPLACE (§5.6): Ready → Draining → Stopped.
    optimizer_lifecycle
        .begin_drain(&mut optimizer_drain, "swap-test", "upgrade requested: optimizer swap")
        .expect("drain begins from Ready");
    assert_eq!(optimizer_lifecycle.state(), LifecycleState::Draining);
    // A swap mid-drain (Draining, not yet quiesced) is ALSO refused.
    let mid_drain = registry.swap_provider(
        seed::CONTRACT_OPTIMIZER,
        clarabel_factory(),
        &mut optimizer_lifecycle,
        "0.2.0",
        "swap-test",
        "attempted swap while draining",
    );
    assert!(matches!(mid_drain, Err(RegistryError::LifecycleRefused { .. })));
    let outcome = optimizer_lifecycle
        .complete_drain("swap-test", "outstanding work finished or cancelled")
        .expect("the drain quiesces");
    assert!(matches!(outcome, DrainOutcome::Quiesced { .. }));
    assert_eq!(optimizer_lifecycle.state(), LifecycleState::Stopped);

    // THE SWAP (ledger step 3): profile/registry change ONLY.
    registry
        .swap_provider(
            seed::CONTRACT_OPTIMIZER,
            clarabel_factory(),
            &mut optimizer_lifecycle,
            "0.2.0",
            "swap-test",
            "optimizer swap HighsTier1Provider -> ClarabelTier2Provider on the LP formulation",
        )
        .expect("the fully drained incumbent is replaceable");

    // (a) The new provider answers through the SAME port trait — the
    // consumer function was NOT recompiled, re-signatured, or re-called
    // with a different type: it still takes `&dyn Solver`.
    let swapped = registry.construct(seed::CONTRACT_OPTIMIZER).expect("constructed");
    assert_eq!(swapped.provider_name(), seed::PROVIDER_CLARABEL_TIER2);
    let swapped_schedule = scheduling_use_case(swapped.as_ref(), PROBLEM_LP);
    assert!(swapped_schedule.contains(seed::PROVIDER_CLARABEL_TIER2));
    assert_eq!(
        registry.registered_plugin_id(seed::CONTRACT_OPTIMIZER),
        Some(seed::PLUGIN_OPTIMIZER_DEFAULT),
        "the contract's plugin identity is stable across the swap"
    );

    // (b) ADR-009 preserved: the INDEPENDENT feasibility validation still
    // runs on the SWAPPED optimizer's output — same function, same verdict
    // path, the grid evaluator independent of the optimizer.
    let swapped_candidate = swapped.solve(PROBLEM_LP).expect("LP supported by the swapped solver");
    assert_eq!(
        independent_feasibility_validation(&swapped_candidate, &grid, "fixture-injections")
            .expect("swapped output validates"),
        "feasible"
    );

    // (c) Solver-class discipline intact through the swap: the labels ride
    // the port trait and are never conflated.
    assert_eq!(swapped.solver_class(), CLARABEL_CLASS_DECLARATION);
    assert!(swapped.solver_class().contains("not MISOCP"));
    assert_eq!(incumbent.solver_class(), HIGHS_CLASS_DECLARATION);
    assert!(incumbent.solver_class().contains("not SOCP"));
    // The class-mismatch failure mode is impossible: the Clarabel-class
    // provider does NOT support the MILP formulation, so a Clarabel swap
    // on a MILP problem yields NO schedule (the use case refuses) — the
    // swap under test is valid only on the LP/conic formulation (§7.4).
    assert!(!swapped.supports(PROBLEM_MILP));
    assert!(swapped.solve(PROBLEM_MILP).is_none());
    assert!(scheduling_use_case(swapped.as_ref(), PROBLEM_MILP).starts_with("no schedule"));
    // And the equivalence criteria hold for the swapped composition.
    assert_equivalence_criteria(swapped.as_ref(), &grid);

    // (d) The audit trail shows the version change AND the drain ordering:
    // refusal(s) → Ready→Draining → Draining→Stopped → VERSION_CHANGE.
    let records = audit.records();
    let kinds: Vec<LifecycleAuditKind> = records.iter().map(|r| r.kind).collect();
    let refused_idx = kinds
        .iter()
        .position(|k| *k == LifecycleAuditKind::Refused)
        .expect("the blocked swap is audited");
    let draining_idx = records
        .iter()
        .position(|r| {
            r.kind == LifecycleAuditKind::Transition
                && r.from == Some(LifecycleState::Ready)
                && r.to == Some(LifecycleState::Draining)
        })
        .expect("the drain-in record is audited");
    let stopped_idx = records
        .iter()
        .position(|r| {
            r.kind == LifecycleAuditKind::Transition
                && r.from == Some(LifecycleState::Draining)
                && r.to == Some(LifecycleState::Stopped)
        })
        .expect("the drain-out record is audited");
    let version_idx = kinds
        .iter()
        .position(|k| *k == LifecycleAuditKind::VersionChange)
        .expect("the version change is audited");
    assert!(refused_idx < draining_idx);
    assert!(draining_idx < stopped_idx);
    assert!(stopped_idx < version_idx, "the version change is recorded AFTER the drain");
    let version_record = &records[version_idx];
    assert_eq!(version_record.plugin_id, seed::PLUGIN_OPTIMIZER_DEFAULT);
    assert_eq!(version_record.version, "0.1.0", "the record carries the OLD version");
    assert!(version_record.reason.contains("0.2.0"), "the NEW version rides the reason");
    assert!(version_record.reason.contains("HighsTier1Provider -> ClarabelTier2Provider"));
}

#[test]
fn the_grid_provider_swap_follows_the_same_lifecycle_path() {
    let plan = seed_plan();
    let clock = FixedClock::new(20_000);
    let audit = MemoryAuditSink::new();

    let mut registry: ProviderRegistry<dyn GridPort> = ProviderRegistry::new(&plan);
    let mut factories = std::collections::BTreeMap::new();
    factories.insert(seed::CONTRACT_GRID_MODEL.to_string(), pandapower_factory());
    registry.register_from_plan(&plan, factories).expect("seed grid composition registers");

    // The drain port is declared BEFORE the lifecycle (borrow-order rule).
    let mut grid_drain = CooperativeDrain { outstanding: 1 };
    let mut grid_lifecycle = PluginLifecycle::register(
        seed::PLUGIN_GRID_MODEL_DEFAULT,
        "0.1.0",
        &clock,
        &audit,
        DrainPolicy::default(),
    );
    drive_to_ready(&mut grid_lifecycle);

    // Baseline: the pandapower-class provider evaluates; the analysis-class
    // declaration is the fidelity label (D3).
    let baseline = registry.construct(seed::CONTRACT_GRID_MODEL).expect("constructed");
    assert_eq!(baseline.provider_name(), seed::PROVIDER_PANDAPOWER_GRID);
    assert_eq!(baseline.analysis_class(), GRID_ANALYSIS_CLASS);

    // Blocked while Active, then drained, then swapped (the same
    // blocked-then-allowed path — no direct registry mutation while Active).
    let blocked = registry.swap_provider(
        seed::CONTRACT_GRID_MODEL,
        opendss_factory(),
        &mut grid_lifecycle,
        "0.2.0",
        "swap-test",
        "attempted grid-provider swap while Active",
    );
    assert!(matches!(blocked, Err(RegistryError::LifecycleRefused { .. })));
    grid_lifecycle
        .begin_drain(&mut grid_drain, "swap-test", "grid provider swap requested")
        .expect("drain begins");
    grid_lifecycle
        .complete_drain("swap-test", "outstanding evaluations finished")
        .expect("quiesced");
    registry
        .swap_provider(
            seed::CONTRACT_GRID_MODEL,
            opendss_factory(),
            &mut grid_lifecycle,
            "0.2.0",
            "swap-test",
            "grid-model provider swap PandapowerGridProvider -> OpenDSS-class adapter (same declared fidelity; D3 recorded)",
        )
        .expect("the drained incumbent is replaceable");

    // (a) The second provider answers through the SAME grid port trait.
    let swapped = registry.construct(seed::CONTRACT_GRID_MODEL).expect("constructed");
    assert!(swapped.provider_name().contains("OpenDss"));
    // (c/Fidelity) The declared analysis class is UNCHANGED — a fidelity
    // change would be a problem-definition change to record (D3), not a
    // silent property of the swap.
    assert_eq!(swapped.analysis_class(), GRID_ANALYSIS_CLASS);
    assert!(swapped.analysis_class().contains("NOT an AC-OPF"));
    // The ADR-009 independent validation consumes the swapped grid
    // provider through the same function (the optimizer-side of the
    // baseline was proven above; here the grid seam swap is proven).
    let candidate = Candidate {
        objective: -42,
        provenance: "provider=builtin.highs-optimizer model=solver-pin-1.15.1 termination=optimal",
        diagnostics: "fixture LP",
    };
    assert_eq!(
        independent_feasibility_validation(&candidate, swapped.as_ref(), "fixture-injections")
            .expect("the swapped grid provider validates the candidate"),
        "feasible"
    );
    // (d) The audit trail for the grid swap: refusal → drain → version
    // change, in order.
    let records = audit.records();
    let refused_idx = records
        .iter()
        .position(|r| r.kind == LifecycleAuditKind::Refused)
        .expect("the blocked grid swap is audited");
    let version_idx = records
        .iter()
        .position(|r| r.kind == LifecycleAuditKind::VersionChange)
        .expect("the grid version change is audited");
    assert!(refused_idx < version_idx);
    assert_eq!(records[version_idx].plugin_id, seed::PLUGIN_GRID_MODEL_DEFAULT);
}

#[test]
fn swap_refusals_are_named_and_ordered() {
    let plan = seed_plan();
    let clock = FixedClock::new(30_000);
    let audit = MemoryAuditSink::new();

    let mut registry: ProviderRegistry<dyn Solver> = ProviderRegistry::new(&plan);
    let mut factories = std::collections::BTreeMap::new();
    factories.insert(seed::CONTRACT_OPTIMIZER.to_string(), highs_factory());
    registry.register_from_plan(&plan, factories).expect("registers");

    let mut optimizer_lifecycle = PluginLifecycle::register(
        seed::PLUGIN_OPTIMIZER_DEFAULT,
        "0.1.0",
        &clock,
        &audit,
        DrainPolicy::default(),
    );
    let mut other_lifecycle = PluginLifecycle::register(
        seed::PLUGIN_GRID_MODEL_DEFAULT,
        "0.1.0",
        &clock,
        &audit,
        DrainPolicy::default(),
    );

    // Unknown contract: refused before the lifecycle is consulted.
    let err = registry
        .swap_provider(
            "optimization.nonexistent",
            clarabel_factory(),
            &mut optimizer_lifecycle,
            "0.2.0",
            "swap-test",
            "unknown contract",
        )
        .expect_err("unknown contracts are refused");
    assert_eq!(err.name(), "SWAP_CONTRACT_UNKNOWN");

    // Lifecycle identity mismatch: the handle must belong to the
    // registered provider of the contract.
    let err = registry
        .swap_provider(
            seed::CONTRACT_OPTIMIZER,
            clarabel_factory(),
            &mut other_lifecycle,
            "0.2.0",
            "swap-test",
            "wrong plugin's lifecycle handle",
        )
        .expect_err("a foreign lifecycle handle is refused");
    match &err {
        RegistryError::SwapLifecycleMismatch { registered_plugin_id, lifecycle_plugin_id, .. } => {
            assert_eq!(registered_plugin_id, seed::PLUGIN_OPTIMIZER_DEFAULT);
            assert_eq!(lifecycle_plugin_id, seed::PLUGIN_GRID_MODEL_DEFAULT);
        }
        other => panic!("expected SWAP_LIFECYCLE_MISMATCH, got {other:?}"),
    }

    // Protected kernel: a swap can never be used to place a factory under
    // a protected-kernel contract id (checked FIRST, before identity).
    let err = registry
        .swap_provider(
            flexcore_kernel::resolve::PROTECTED_KERNEL_CONTRACT_IDS[0],
            clarabel_factory(),
            &mut optimizer_lifecycle,
            "0.2.0",
            "swap-test",
            "protected contract",
        )
        .expect_err("the protected kernel has no swap path");
    assert_eq!(err.name(), "PROTECTED_KERNEL_CONTRACT_REFUSED");

    // The refused attempts never disturbed the registered provider.
    assert_eq!(
        registry.construct(seed::CONTRACT_OPTIMIZER).expect("constructed").provider_name(),
        seed::PROVIDER_HIGHS_TIER1
    );
    // And the two refused swap attempts on the optimizer lifecycle did not
    // advance its state (it was never drained; it stays at Discovered).
    assert_eq!(optimizer_lifecycle.state(), LifecycleState::Discovered);
}
