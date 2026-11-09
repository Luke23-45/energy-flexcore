// P3.17 — Dependency-direction test (ledger P3.17 acceptance: "scheduling
// code calls only the port, never HiGHS directly").
//
// Two checks:
// 1. **Source scan** — the port-surface modules (port, problem, result,
//    config, backend) never name the Tier 1 solver or its binding, nor the
//    Tier 2 conic solver (P3.18). The provider modules and the decision
//    notes are the only places allowed to. (Same pattern as the
//    flexcore-ports vendor-type scan, P1.07.)
// 2. **Manifest scan** — the crate manifest contains no solver binding
//    beyond the recorded Tier 2 native conic solver, no transport, and no
//    async runtime dependency: the allowed set is flexcore-contracts +
//    serde/serde_json/time (core-crate policy) plus the P3.18 `clarabel`
//    native Rust crate (decision note 18_clarabel_tier2.md §3: a native
//    conic-solver implementation invoked in-process by the Tier 2 provider —
//    not a transport, not a binding over a worker process).
//
// P3.18 change (recorded in 18_clarabel_tier2.md §3): the manifest set is
// extended by exactly the Tier 2 conic-solver crate; no other dependency is
// admitted.

use std::path::Path;

/// The solver identifiers, assembled from halves so this test module itself
/// never contains a literal hit for the needles it scans with.
fn solver_needles() -> Vec<String> {
    vec![
        format!("{}{}", "hi", "ghs"),    // the solver name and its C API prefix
        format!("{}{}", "hi", "ghspy"),  // the Python binding name
        format!("{}{}", "cla", "rabel"), // the Tier 2 conic solver (P3.18)
    ]
}

/// The port-surface modules whose sources must be solver-neutral.
fn port_surface_sources() -> Vec<(&'static str, String)> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    vec![
        (
            "src/port.rs",
            std::fs::read_to_string(Path::new(manifest_dir).join("src/port.rs"))
                .expect("port.rs readable"),
        ),
        (
            "src/problem.rs",
            std::fs::read_to_string(Path::new(manifest_dir).join("src/problem.rs"))
                .expect("problem.rs readable"),
        ),
        (
            "src/result.rs",
            std::fs::read_to_string(Path::new(manifest_dir).join("src/result.rs"))
                .expect("result.rs readable"),
        ),
        (
            "src/config.rs",
            std::fs::read_to_string(Path::new(manifest_dir).join("src/config.rs"))
                .expect("config.rs readable"),
        ),
        (
            "src/backend.rs",
            std::fs::read_to_string(Path::new(manifest_dir).join("src/backend.rs"))
                .expect("backend.rs readable"),
        ),
    ]
}

/// No port-surface source names the solver: scheduling code that consumes
/// only the port can never reference a solver-specific type.
#[test]
fn port_surface_never_names_the_solver() {
    let needles = solver_needles();
    for (name, source) in port_surface_sources() {
        let lowered = source.to_lowercase();
        for needle in &needles {
            assert!(
                !lowered.contains(needle.as_str()),
                "port-surface source {name} must not name the Tier 1 solver or its \
                 binding (found {needle}); providers own every concrete solver"
            );
        }
    }
}

/// The manifest carries no solver binding, transport, or runtime dependency
/// beyond the recorded Tier 2 native conic solver: Tier 1 routes through the
/// Python worker behind the in-crate `Tier1Backend` seam (decision note
/// D-P3.17-1), and Tier 2 (P3.18) links the native Rust crate `clarabel`
/// in-process (decision note D-P3.18-1) — no transport client and no async
/// runtime appears in the dependency list.
#[test]
fn manifest_has_no_solver_transport_or_runtime_dependency() {
    let manifest = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
    )
    .expect("Cargo.toml readable");
    // Split off the [dependencies] section body.
    let deps_body = manifest
        .split("[dependencies]")
        .nth(1)
        .expect("[dependencies] section present");
    let deps_body = deps_body.split('[').next().expect("section terminates");

    // The core-crate policy set plus exactly the Tier 2 conic solver
    // (assembled from halves like the scan needles above).
    let mut allowed = vec![
        "flexcore-contracts".to_string(),
        "serde".to_string(),
        "serde_json".to_string(),
        "time".to_string(),
        format!("{}{}", "cla", "rabel"),
    ];
    let mut found = Vec::new();
    for line in deps_body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let name = line.split('=').next().unwrap_or("").trim();
        if !name.is_empty() {
            found.push(name.to_string());
        }
    }
    found.sort();
    allowed.sort();
    assert_eq!(
        found, allowed,
        "the dependency list is exactly the allowed core-crate set plus the \
         recorded Tier 2 conic solver; any other solver binding, transport \
         client, or async runtime here would break the recorded provider-path \
         decisions"
    );
}

/// The provider exists and is reachable, but only behind the port: a
/// consumer that imports just the port types can construct and solve against
/// any `Optimizer` implementation without naming the provider's solver.
#[test]
fn port_only_consumer_compiles_against_any_provider() {
    use flexcore_optimization_api::config::SolverConfig;
    use flexcore_optimization_api::port::{Optimizer, SolverCapabilities};
    use flexcore_optimization_api::problem::{OptimizationProblem, ProblemClass};
    use flexcore_optimization_api::result::{OptimizationResult, SolveStatus};

    // A minimal port-only consumer: it depends on the port types only. Any
    // provider — the Tier 1 one, the future Tier 2 one (P3.18), or a test
    // double — plugs in through this trait object.
    struct PortOnlyConsumer {
        optimizer: Box<dyn Optimizer>,
    }
    impl PortOnlyConsumer {
        fn supported_classes(&self) -> Vec<ProblemClass> {
            let SolverCapabilities { problem_classes, .. } = self.optimizer.capabilities();
            problem_classes
        }
        fn solve(&self, problem: &OptimizationProblem) -> OptimizationResult {
            self.optimizer
                .solve(problem, &SolverConfig::default())
                .expect("in-band outcomes only")
        }
    }

    // The consumer type itself never names a solver; the assertion below
    // compiles the whole shape (and would fail if the port stopped being
    // object-safe or leaked solver types into the consumed surface).
    fn builds_with_any_provider(optimizer: Box<dyn Optimizer>) -> PortOnlyConsumer {
        PortOnlyConsumer { optimizer }
    }

    // Type-level check only — no solver name appears in this test beyond the
    // scan-needle halving above.
    let _ = builds_with_any_provider as fn(Box<dyn Optimizer>) -> PortOnlyConsumer;
    let _ = SolveStatus::Unsupported;
}
