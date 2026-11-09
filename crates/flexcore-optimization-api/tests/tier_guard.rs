// P3.20 — Tier 3/4 boundary guard tests (report §12.3, §13; ledger P3.20).
//
// Acceptance coverage here:
// - MISOCP (Tier 3) and nonlinear AC-OPF / MINLP (Tier 4) formulation
//   requests are rejected with `unsupported` AND the pointer to the
//   recorded decision ([`TIER_BOUNDARY_DECISION_REFERENCE`]).
// - No Tier 3/4 code path exists: the capability surfaces of BOTH providers
//   declare only Tier 1/2 classes (checked against the guard), and a
//   structural source scan asserts the crate exposes no Tier 3/4 entry
//   point.
// - The IPOPT honesty note: local nonlinear solve only, NO
//   global-optimality guarantee; not wired as a Phase 3 scheduler solver.

mod common;

use common::scripted;
use flexcore_optimization_api::fixture;
use flexcore_optimization_api::highs_provider::HighsTier1Provider;
use flexcore_optimization_api::clarabel_provider::ClarabelTier2Provider;
use flexcore_optimization_api::port::{Optimizer, SolverCapabilities};
use flexcore_optimization_api::problem::ProblemClass;
use flexcore_optimization_api::tier_guard::{
    check_capabilities_within_boundary, check_tier_boundary, guard_problem, IPOPT_LOCALITY_NOTE,
    REFORMULATION_RULE, TIER_BOUNDARY_DECISION_REFERENCE, TIER_BOUNDARY_POLICY,
};

/// MISOCP (Tier 3) and nonlinear (Tier 4 — the AC-OPF/MINLP boundary class)
/// requests are rejected with `unsupported` and the decision reference.
#[test]
fn tier3_and_tier4_requests_are_rejected_with_the_decision_reference() {
    for problem in [
        fixture::reference_misocp_declaration(),
        fixture::reference_nonlinear_declaration(),
    ] {
        let rejection = guard_problem(&problem).expect_err("Tier 3/4 is rejected");
        assert_eq!(rejection.outcome, "unsupported");
        assert_eq!(
            rejection.decision_reference, TIER_BOUNDARY_DECISION_REFERENCE,
            "the rejection carries the recorded decision pointer"
        );
        assert!(
            rejection
                .decision_reference
                .contains("19_20_determinism_tiers.md"),
            "the pointer names the decision note: {}",
            rejection.decision_reference
        );
        assert!(!rejection.reformulation_guidance.is_empty());
        assert!(
            rejection.requested_tier == 3 || rejection.requested_tier == 4,
            "rejected tier is 3 or 4, got {}",
            rejection.requested_tier
        );
    }
}

/// The guard is class-granular: every Tier 3/4 class is rejected, every
/// Tier 1/2 class passes.
#[test]
fn the_guard_is_exactly_the_tier_boundary() {
    let accepted = [
        ProblemClass::LinearProgram,
        ProblemClass::ConvexQuadraticProgram,
        ProblemClass::MixedIntegerLinearProgram,
        ProblemClass::ContinuousSecondOrderConeProgram,
    ];
    let rejected = [
        ProblemClass::MixedIntegerSecondOrderConeProgram,
        ProblemClass::NonlinearProgram,
    ];
    for class in accepted {
        check_tier_boundary(class).unwrap_or_else(|e| {
            panic!("Tier {} class {} must pass the guard: {e}", class.tier(), class.as_str())
        });
    }
    for class in rejected {
        let rejection = check_tier_boundary(class).expect_err("Tier 3/4 rejected");
        assert_eq!(rejection.requested_class, class);
        assert_eq!(rejection.requested_tier, class.tier());
    }
}

/// Per-class reformulation guidance: MISOCP points at the Tier 1 MILP /
/// Tier 2 SOCP / Stage-8 rule; nonlinear points at the linearised/convex
/// reformulation and the IPOPT locality note.
#[test]
fn reformulation_guidance_follows_the_recorded_rule() {
    let misocp = check_tier_boundary(ProblemClass::MixedIntegerSecondOrderConeProgram)
        .expect_err("rejected")
        .reformulation_guidance;
    assert!(misocp.contains("MILP"));
    assert!(misocp.contains("SOCP"));
    assert!(misocp.contains("Stage-8"));
    assert!(misocp.contains(REFORMULATION_RULE));

    let nonlinear = check_tier_boundary(ProblemClass::NonlinearProgram)
        .expect_err("rejected")
        .reformulation_guidance;
    assert!(nonlinear.contains("Stage-8"));
    assert!(
        nonlinear.contains("IPOPT"),
        "the nonlinear guidance carries the IPOPT honesty note"
    );

    // The policy statement names both tiers and the release boundary.
    assert!(TIER_BOUNDARY_POLICY.contains("MISOCP/MINLP"));
    assert!(TIER_BOUNDARY_POLICY.contains("NEITHER is built in v0.2.0"));
}

/// No Tier 3/4 code path exists: BOTH providers' capability surfaces declare
/// only Tier 1/2 classes, and both pass the capability-boundary check.
#[test]
fn no_provider_declares_a_tier34_class() {
    let tier1 = HighsTier1Provider::new(scripted(Vec::new()));
    let tier2 = ClarabelTier2Provider::new(Default::default());

    let capabilities: Vec<SolverCapabilities> = vec![tier1.capabilities(), tier2.capabilities()];
    for capabilities in &capabilities {
        check_capabilities_within_boundary(capabilities)
            .expect("no provider declares a Tier 3/4 class");
        for class in &capabilities.problem_classes {
            assert!(
                class.tier() <= 2,
                "provider {} declares class '{}' at tier {}",
                capabilities.provider_id,
                class.as_str(),
                class.tier()
            );
        }
    }
    // Exactly the built classes are declared anywhere.
    let mut declared: Vec<ProblemClass> = capabilities
        .into_iter()
        .flat_map(|c| c.problem_classes)
        .collect();
    declared.sort_by_key(|c| c.as_str());
    let expected = [
        ProblemClass::LinearProgram,
        ProblemClass::ConvexQuadraticProgram,
        ProblemClass::MixedIntegerLinearProgram,
        ProblemClass::ContinuousSecondOrderConeProgram,
    ];
    for class in expected {
        assert!(
            declared.contains(&class),
            "built class {} must be declared",
            class.as_str()
        );
    }
    assert!(!declared.contains(&ProblemClass::MixedIntegerSecondOrderConeProgram));
    assert!(!declared.contains(&ProblemClass::NonlinearProgram));
}

/// Structural test: the crate exposes NO Tier 3/4 entry point — a source
/// scan over `src/` asserting no function declaration implements (or is
/// named for) a MISOCP/MINLP/AC-OPF solver path. If a Tier 3/4 path is ever
/// added, this test fails and the recorded decision must be updated first
/// (the ledger's risk mitigation: "the guard test fails the build if a
/// Tier 3/4 path is added without updating this decision").
#[test]
fn structural_scan_no_tier34_entry_point_exists() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let src_dir = std::path::Path::new(manifest_dir).join("src");
    // Needles assembled from halves so this test file carries no complete
    // hit for itself beyond the fixture-driven guard tests above.
    let needles: Vec<String> = vec![
        format!("{}{}", "mi", "socp"),
        format!("{}{}", "min", "lp_solver"),
        format!("{}{}", "ac", "opf"),
    ];

    let entries = std::fs::read_dir(&src_dir).expect("src dir readable");
    // fixture.rs is the ONE recorded exclusion: it declares the MISOCP
    // formulation fixture (`reference_misocp_declaration`) solely so the
    // rejection tests can prove the formulation is REFUSED — it is an input
    // to the guard tests, not a solver path (asserted rejected in
    // tests/provider_status_mapping.rs and in this file above).
    let excluded_files = ["fixture.rs"];
    for entry in entries {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("<src>")
            .to_string();
        if excluded_files.contains(&file_name.as_str()) {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("source readable");
        for line in source.lines() {
            let trimmed = line.trim_start();
            let is_fn_declaration = trimmed.starts_with("fn ")
                || trimmed.starts_with("pub fn ")
                || trimmed.starts_with("pub(crate) fn ");
            if !is_fn_declaration {
                continue;
            }
            let lowered = line.to_lowercase();
            for needle in &needles {
                assert!(
                    !lowered.contains(needle.as_str()),
                    "{file_name} declares a function that looks like a Tier 3/4 \
                     solver path ({needle:?}); update the recorded decision \
                     {TIER_BOUNDARY_DECISION_REFERENCE} first"
                );
            }
        }
    }
}

/// The IPOPT honesty note (report §7.4; ledger P3.20 step 4): local solve
/// only, no global-optimality claim, not a Phase 3 scheduler solver, EPL
/// license recorded for the D4 audit.
#[test]
fn ipopt_is_described_honestly() {
    let lowered = IPOPT_LOCALITY_NOTE.to_lowercase();
    assert!(lowered.contains("local"), "local solve only");
    assert!(
        lowered.contains("no global-optimality guarantee"),
        "the no-global-optimality claim is explicit"
    );
    assert!(
        lowered.contains("not wired as a phase 3 scheduler solver"),
        "IPOPT is not a Phase 3 scheduler solver"
    );
    assert!(lowered.contains("stage 8"), "future use lands in Stage 8");
    assert!(
        lowered.contains("eclipse public license"),
        "the EPL license is noted for the D4 audit"
    );
    // Nothing in the note claims global optimality for any solver.
    assert!(!lowered.contains("globally optimal"));
    assert!(!lowered.contains("global optimality guarantee"));
}
