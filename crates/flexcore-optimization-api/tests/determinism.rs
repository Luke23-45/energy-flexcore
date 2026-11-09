// P3.19 — Deterministic-replay tests (ledger P3.19 step 4; report §19.2
// layer 5).
//
// Same inputs + same pinned versions + same config → identical schedules:
// the replay harness (`flexcore_optimization_api::determinism`) runs over
// the P3.17 reference fixtures through the full Tier 1 port path with the
// deterministic scripted backend double (byte-identical mode, tolerance
// 0.0), and the offline replay identity of the P3.18 conic fixtures is
// checked (identity of the assembled conic model + config digest + declared
// versions). The tolerance mode is exercised with STATED tolerances.
//
// VERIFY-BUILD: the real-solver replay runs — the Tier 1 worker adapter with
// the pinned binding and the full Tier 2 replay suite — are P3.26's
// scenario-regression job (decision notes 17_optimizer_port.md §6 item 6 and
// 18_clarabel_tier2.md §7 item 8); the harness is written so P3.26 calls
// `ReplayHarness::verify_replay` unchanged. The two `verify_build_*` tests
// here exercise the real Tier 2 solve path once the coordinator's root
// manifest line (`clarabel = "=0.11.1"`) lands.

mod common;

use common::{outcome, scripted};
use flexcore_optimization_api::backend::{BackendOutcome, BackendStatus};
use flexcore_optimization_api::clarabel_provider::{
    assemble_conic_model, ClarabelTier2Provider, CLARABEL_SOLVER_VERSION_PIN,
};
use flexcore_optimization_api::config::{
    config_contract, ProviderTier, SolverConfig, DETERMINISM_THREAD_COUNT_DEFAULT,
};
use flexcore_optimization_api::determinism::{
    compare_replay_records, problem_ir_digest, ReplayHarness, ReplayIdentity,
    COMPARISON_SCOPE_NOTE, DEFAULT_REPLAY_TOLERANCE, DETERMINISM_POLICY,
};
use flexcore_optimization_api::fixture;
use flexcore_optimization_api::port::Optimizer;
use flexcore_optimization_api::problem::OptimizationProblem;
use flexcore_optimization_api::result::SolveStatus;
use std::time::Duration;

/// The deterministic optimal outcome for a fixture (hand-computed optimum;
/// the scripted double returns EXACTLY this on every call, so two replays
/// through the provider path must produce identical records except for the
/// measured runtime, which the comparison excludes — stated in
/// COMPARISON_SCOPE_NOTE).
fn optimal_outcome(objective_value: f64, column_values: Vec<f64>) -> BackendOutcome {
    let mut base = outcome(
        BackendStatus::Optimal,
        "Optimal",
        Some(objective_value),
        Some(column_values),
    );
    base.max_primal_violation = Some(0.0);
    base.iteration_count = Some(3);
    base
}

/// The Tier 1 reference fixtures with their hand-computed optima.
fn tier1_fixture_cases() -> Vec<(OptimizationProblem, f64, Vec<f64>)> {
    vec![
        (
            fixture::reference_lp(),
            fixture::reference_lp_optimum().objective_value,
            fixture::reference_lp_optimum().column_values,
        ),
        (
            fixture::reference_qp(),
            fixture::reference_qp_optimum().objective_value,
            fixture::reference_qp_optimum().column_values,
        ),
        (
            fixture::reference_milp(),
            fixture::reference_milp_optimum().objective_value,
            fixture::reference_milp_optimum().column_values,
        ),
    ]
}

/// The byte-identical replay over every P3.17 reference fixture through the
/// full port path: same problem IR + same pinned versions + same config →
/// identical records (tolerance 0.0), with the replay identity recorded and
/// the thread-count recording explicit (default 1).
#[test]
fn byte_identical_replay_over_the_reference_fixtures() {
    let harness = ReplayHarness::byte_identical();
    assert_eq!(harness.tolerance(), 0.0);

    for (problem, objective, values) in tier1_fixture_cases() {
        let outcomes = vec![
            Ok(optimal_outcome(objective, values.clone())),
            Ok(optimal_outcome(objective, values.clone())),
        ];
        let provider = flexcore_optimization_api::highs_provider::HighsTier1Provider::new(
            scripted(outcomes),
        );
        let verification = harness
            .verify_replay(&provider, &problem, &SolverConfig::default())
            .expect("replay solves");

        assert!(
            verification.comparison.identical,
            "replay of {} must be byte-identical; differences: {:?}",
            problem.name, verification.comparison.differences
        );
        assert!(verification.comparison.differences.is_empty());
        assert_eq!(verification.comparison.scope_note, COMPARISON_SCOPE_NOTE);

        // The replay identity is stable and records every required part. The
        // expected identity is built from the SAME declarations the harness
        // reads off the provider's capability surface.
        let capabilities = provider.capabilities();
        let identity = ReplayIdentity::new(
            &problem,
            &SolverConfig::default(),
            &capabilities.solver_version,
            &capabilities.provider_version,
        );
        assert_eq!(verification.identity, identity);
        assert_eq!(identity.thread_count, DETERMINISM_THREAD_COUNT_DEFAULT);
        assert!(identity.config_digest.starts_with("fnv1a64:"));
        assert!(identity.config_canonical_json.contains("\"thread_count\":1"));
        assert_eq!(
            identity.solver_version,
            flexcore_optimization_api::highs_provider::HIGHS_SOLVER_VERSION_PIN
        );
    }
}

/// The replay identity changes when ANY identity part changes: config,
/// pinned solver version, provider version, or the problem IR.
#[test]
fn replay_identity_is_discriminating() {
    let problem = fixture::reference_lp();
    let config = SolverConfig::default();
    let baseline = ReplayIdentity::new(&problem, &config, "1.0.0", "0.1.0");

    let mut thread_count_config = SolverConfig::default();
    thread_count_config.thread_count = 4;
    assert_ne!(
        baseline.identity_digest(),
        ReplayIdentity::new(&problem, &thread_count_config, "1.0.0", "0.1.0").identity_digest()
    );

    let mut tolerance_config = SolverConfig::default();
    tolerance_config.primal_feasibility_tolerance = 1e-6;
    assert_ne!(
        baseline.identity_digest(),
        ReplayIdentity::new(&problem, &tolerance_config, "1.0.0", "0.1.0").identity_digest()
    );

    assert_ne!(
        baseline.identity_digest(),
        ReplayIdentity::new(&problem, &config, "9.9.9", "0.1.0").identity_digest()
    );
    assert_ne!(
        baseline.identity_digest(),
        ReplayIdentity::new(&problem, &config, "1.0.0", "9.9.9").identity_digest()
    );
    assert_ne!(
        baseline.identity_digest(),
        ReplayIdentity::new(&fixture::reference_qp(), &config, "1.0.0", "0.1.0")
            .identity_digest()
    );

    // A recorded digest verifies against its own config and no other.
    assert!(thread_count_config.matches_digest(&thread_count_config.config_digest()));
    assert!(!thread_count_config.matches_digest(&baseline.config_digest));
}

/// Tolerance-identical comparison with the tolerance STATED: values within
/// the stated tolerance compare identical; beyond it, the comparison is a
/// recorded determinism finding with the differing field named.
#[test]
fn tolerance_identical_comparison_states_its_tolerance() {
    let problem = fixture::reference_lp();
    let config = SolverConfig::default();

    let backend_outcome = optimal_outcome(
        fixture::reference_lp_optimum().objective_value,
        fixture::reference_lp_optimum().column_values.clone(),
    );
    let provider = flexcore_optimization_api::highs_provider::HighsTier1Provider::new(scripted(
        vec![Ok(backend_outcome.clone()), Ok(backend_outcome)],
    ));
    let verification = ReplayHarness::with_tolerance(DEFAULT_REPLAY_TOLERANCE)
        .verify_replay(&provider, &problem, &config)
        .expect("replay solves");
    assert!(verification.comparison.identical);
    assert_eq!(verification.comparison.tolerance, DEFAULT_REPLAY_TOLERANCE);

    // Two records differing beyond tolerance: the finding is recorded, with
    // the differing field named — never smoothed over. (A fresh provider,
    // since the replay above consumed the scripted outcomes.)
    let provider = flexcore_optimization_api::highs_provider::HighsTier1Provider::new(scripted(
        vec![
            Ok(optimal_outcome(8.0, vec![4.0, 0.0])),
            Ok(optimal_outcome(8.0, vec![4.0, 0.0])),
        ],
    ));
    let baseline = provider.solve(&problem, &config).expect("first solve");
    let mut replay = baseline.clone();
    replay.objective_value = baseline.objective_value.map(|v| v + 1e-3);
    let comparison = compare_replay_records(&baseline, &replay, DEFAULT_REPLAY_TOLERANCE);
    assert!(!comparison.identical);
    assert!(
        comparison
            .differences
            .iter()
            .any(|d| d.contains("objective_value")),
        "the objective difference must be reported: {:?}",
        comparison.differences
    );

    // The stated tolerance is what distinguishes the modes: a 5e-10 drift is
    // a finding at tolerance 0.0 and absorbed at the stated tolerance.
    let baseline = provider.solve(&problem, &config).expect("second solve");
    let mut near = baseline.clone();
    near.objective_value = baseline.objective_value.map(|v| v + 5e-10);
    let strict = compare_replay_records(&baseline, &near, 0.0);
    assert!(!strict.identical);
    let tolerant = compare_replay_records(&baseline, &near, DEFAULT_REPLAY_TOLERANCE);
    assert!(tolerant.identical, "the stated tolerance absorbs 5e-10");
}

/// The runtime is a measured field: two solves whose runtime differs are
/// still identical when everything else matches (the exclusion is stated in
/// the comparison scope, per the ledger's "with the tolerance stated"
/// discipline).
#[test]
fn measured_runtime_is_excluded_and_the_exclusion_is_stated() {
    let problem = fixture::reference_lp();
    let config = SolverConfig::default();
    let provider = flexcore_optimization_api::highs_provider::HighsTier1Provider::new(scripted(
        vec![
            Ok(optimal_outcome(8.0, vec![4.0, 0.0])),
            Ok(optimal_outcome(8.0, vec![4.0, 0.0])),
        ],
    ));
    // Real solves produce (slightly) different runtimes; the comparison must
    // still be identical.
    let verification = ReplayHarness::byte_identical()
        .verify_replay(&provider, &problem, &config)
        .expect("replay solves");
    assert!(verification.comparison.identical);
    assert!(verification.comparison.scope_note.contains("EXCLUDED"));
}

/// Offline replay identity of the P3.18 conic fixtures through the port-side
/// assembly path: the assembled conic model is identical across replays, the
/// config digest is stable, and the declared versions (pinned Tier 2 solver
/// + provider) are the recorded ones.
#[test]
fn conic_fixture_replay_identity_assembled_model_config_and_versions() {
    for problem in [
        flexcore_optimization_api::conic_fixture::inverter_cone_socp(),
        flexcore_optimization_api::conic_fixture::distflow_branch_socp(),
    ] {
        let config = SolverConfig::default();

        // The assembled model is identical across replays.
        let model_first = assemble_conic_model(&problem).expect("fixture assembles");
        let model_second = assemble_conic_model(&problem).expect("fixture assembles");
        assert_eq!(model_first, model_second);

        // The replay identity is stable and declares the pinned versions.
        let identity_first =
            ReplayIdentity::new(&problem, &config, CLARABEL_SOLVER_VERSION_PIN, "0.1.0");
        let identity_second =
            ReplayIdentity::new(&problem, &config, CLARABEL_SOLVER_VERSION_PIN, "0.1.0");
        assert_eq!(identity_first, identity_second);
        assert_eq!(identity_first.solver_version, "0.11.1");
        assert_eq!(identity_first.thread_count, 1);
        assert_eq!(
            problem_ir_digest(&problem),
            identity_first.problem_ir_digest
        );
    }
}

/// VERIFY-BUILD (18_clarabel_tier2.md §7 item 8; P3.26 lands the full
/// real-solver replay suite): the inverter-cone fixture solved TWICE through
/// the real Tier 2 provider replays identically at the stated tolerance
/// (DEFAULT_REPLAY_TOLERANCE = 1e-9). Runs once the coordinator's root
/// manifest line (`clarabel = "=0.11.1"`) lands.
#[test]
fn verify_build_real_socp_solve_replays_identically() {
    let provider = ClarabelTier2Provider::new(Default::default());
    let problem = flexcore_optimization_api::conic_fixture::inverter_cone_socp();
    let verification = ReplayHarness::with_tolerance(DEFAULT_REPLAY_TOLERANCE)
        .verify_replay(&provider, &problem, &SolverConfig::default())
        .expect("real replay solves");
    assert!(
        verification.comparison.identical,
        "the real conic solve must replay identically at the stated tolerance \
         {:e}; differences: {:?}",
        DEFAULT_REPLAY_TOLERANCE,
        verification.comparison.differences
    );
}

/// VERIFY-BUILD: the timeout-status construction for the Tier 2 path. The
/// solver's time-limit-hit outcome (native `MaxTime`; VERIFY-BUILD on the
/// exact API signal at build time) must map onto the closed §12.6 status
/// `timeout` with the verbatim native status recorded. The time limit is set
/// to 1 ns so a real solve cannot complete within it on any realistic
/// machine; should the pinned solver nevertheless finish, the test still
/// asserts the outcome stayed inside the closed vocabulary.
#[test]
fn verify_build_time_limited_socp_solve_maps_into_the_closed_vocabulary() {
    let provider = ClarabelTier2Provider::new(Default::default());
    let problem = flexcore_optimization_api::conic_fixture::inverter_cone_socp();
    let mut config = SolverConfig::default();
    config.time_limit = Some(Duration::from_nanos(1));
    config.validate().expect("1 ns limit is a non-zero limit");

    let result = provider
        .solve(&problem, &config)
        .expect("a time-limited solve maps in-band");
    assert!(
        matches!(
            result.status,
            SolveStatus::Timeout | SolveStatus::Optimal | SolveStatus::NumericalFailure
        ),
        "the limited outcome stays in the closed vocabulary, got {:?}",
        result.status
    );
    // The verbatim native status is recorded on every mapping.
    let mapping_warning = result
        .warnings
        .iter()
        .find(|w| w.contains("mapped to §12.6 status"))
        .expect("verbatim native status recorded");
    if mapping_warning.contains("MaxTime") {
        assert_eq!(
            result.status,
            SolveStatus::Timeout,
            "a time-limit-hit outcome maps to timeout"
        );
    }
    result
        .validate_complete()
        .expect("the limited record is complete");
}

/// The policy statement and the per-provider configuration contract are
/// present and cover the ledger's requirements.
#[test]
fn determinism_policy_and_config_contracts_are_stated() {
    assert!(DETERMINISM_POLICY.contains("byte-identical"));
    assert!(DETERMINISM_POLICY.contains("tolerance"));
    // The seed/config position: no random seed exists; the config digest
    // stands in for "seed/config".
    assert!(DETERMINISM_POLICY.contains("seed"));
    // Parallel-MIP determinism is not assumed (RESEARCH.md §6 task 1).
    assert!(DETERMINISM_POLICY.contains("NOT assumed"));

    // Per-provider configuration completion (P3.19 step 1): Tier 1 covers
    // the MIP gap (MILP class), Tier 2 does not (continuous conic).
    let tier1 = config_contract(ProviderTier::Tier1);
    let tier2 = config_contract(ProviderTier::Tier2);
    assert!(tier1.consumes_mip_relative_gap);
    assert!(!tier2.consumes_mip_relative_gap);
    for contract in [tier1, tier2] {
        assert!(contract.consumes_time_limit);
        assert!(contract.consumes_feasibility_tolerance);
        assert!(contract.consumes_optimality_tolerance);
        assert!(contract.consumes_scaling);
        assert!(contract.thread_count_fixed_for_determinism);
        assert!(!contract.notes.is_empty());
    }
}

/// The harness is written for P3.26: it accepts any `Optimizer` behind a
/// trait object (the port composition shape) — compile-checked here.
#[test]
fn harness_accepts_any_optimizer_trait_object() {
    let provider = flexcore_optimization_api::highs_provider::HighsTier1Provider::new(
        scripted(vec![
            Ok(optimal_outcome(8.0, vec![4.0, 0.0])),
            Ok(optimal_outcome(8.0, vec![4.0, 0.0])),
        ]),
    );
    let optimizer: Box<dyn Optimizer> = Box::new(provider);
    let verification = ReplayHarness::byte_identical()
        .verify_replay(
            optimizer.as_ref(),
            &fixture::reference_lp(),
            &SolverConfig::default(),
        )
        .expect("replay solves");
    assert!(verification.comparison.identical);
    assert_eq!(
        verification.identity.problem_ir_digest,
        problem_ir_digest(&fixture::reference_lp())
    );
}
