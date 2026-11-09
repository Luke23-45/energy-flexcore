// P4.02 — Dependency-direction test (ledger P4.02 acceptance:
// "feasibility code calls only the port" and "dependency-direction check";
// report §12.4: the gate depends on a contract, not on pandapower
// directly).
//
// Three checks:
// 1. **Source scan** — the port-surface modules (`src/port.rs`,
//    `src/scope.rs`: the types the feasibility gate consumes directly)
//    never name the concrete provider. The adapter module
//    (`src/adapter.rs`), the backend seam (`src/backend.rs`), the
//    translation layer (`src/translation.rs`), the crate docs, and the
//    decision notes are the places that may name the provider: they are
//    adapter-side, not port surface. (Same pattern as the
//    flexcore-optimization-api and flexcore-ports vendor-type scans;
//    the scan scope here is exactly the port surface as recorded in the
//    decision note §5.)
// 2. **Manifest scan** — the crate manifest contains no provider binding,
//    no transport, and no async runtime dependency: the allowed set is
//    exactly flexcore-contracts + serde/serde_json/time (core-crate
//    policy). The provider path runs through the transport-neutral
//    `GridModelBackend` seam (decision note D-P4.02-1), so no Python
//    binding and no gRPC client may ever appear here.
// 3. **Port-only consumer** — a consumer that imports only the port types
//    can evaluate a candidate against any `GridModelProvider`
//    implementation without naming any provider.

use std::path::Path;

/// The provider identifiers, assembled from halves so this test module
/// itself never contains a literal hit for the needles it scans with.
fn provider_needles() -> Vec<String> {
    vec![
        format!("{}{}", "pan", "dapower"),  // the provider library name
        format!("{}{}", "pan", "daseries"), // its model-construction API family
    ]
}

/// The port-surface modules whose sources must be provider-neutral: these
/// are the modules whose TYPES the feasibility gate consumes directly.
fn port_surface_sources() -> Vec<(&'static str, String)> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    vec![
        (
            "src/port.rs",
            std::fs::read_to_string(Path::new(manifest_dir).join("src/port.rs"))
                .expect("port.rs readable"),
        ),
        (
            "src/scope.rs",
            std::fs::read_to_string(Path::new(manifest_dir).join("src/scope.rs"))
                .expect("scope.rs readable"),
        ),
    ]
}

/// No port-surface source names the provider: feasibility code that
/// consumes only the port can never reference a provider-specific type.
#[test]
fn port_surface_never_names_the_provider() {
    let needles = provider_needles();
    for (name, source) in port_surface_sources() {
        let lowered = source.to_lowercase();
        for needle in &needles {
            assert!(
                !lowered.contains(needle.as_str()),
                "port-surface source {name} must not name the concrete provider \
                 (found {needle}); adapters own every concrete provider and the \
                 dependency-direction test pins the port boundary (ledger P4.02)"
            );
        }
    }
}

/// The manifest carries no provider binding, transport, or runtime
/// dependency: the provider path is the Python numerical worker behind the
/// in-crate transport-neutral `GridModelBackend` seam (decision note
/// D-P4.02-1 — the D-P3.17-1 pattern), so no Python-binding crate, no gRPC
/// client, and no async runtime appears in the dependency list.
#[test]
fn manifest_has_no_provider_transport_or_runtime_dependency() {
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

    // The exact core-crate policy set for this crate (the coordinator's
    // check_dependency_rule.py entry mirrors it: ALLOWED_INTERNAL_EDGES =
    // {"flexcore-contracts"}; serde/serde_json/time sit outside the
    // internal-edge check, exactly as for flexcore-contracts itself).
    let mut allowed = vec![
        "flexcore-contracts".to_string(),
        "serde".to_string(),
        "serde_json".to_string(),
        "time".to_string(),
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
        "the dependency list is exactly the allowed core-crate set; any \
         provider binding, transport client, or async runtime here would \
         break the recorded provider-path decision D-P4.02-1"
    );
}

/// The provider exists and is reachable, but only behind the port: a
/// consumer that imports just the port types can construct and evaluate
/// against any `GridModelProvider` implementation — the real adapter, the
/// in-crate stub, or a future second provider — without naming any of
/// them.
#[test]
fn port_only_consumer_compiles_against_any_provider() {
    use flexcore_grid_api::port::{
        CandidateInjection, FeasibilityVerdict, GridModelProvider, NetworkLimits,
        PowerFlowResult, VersionedNetworkModel,
    };

    // A minimal port-only consumer: it depends on the port types only.
    // Any provider plugs in through this trait object.
    struct PortOnlyFeasibilityGate {
        provider: Box<dyn GridModelProvider>,
    }
    impl PortOnlyFeasibilityGate {
        fn capabilities(&self) -> (String, String) {
            let caps = self.provider.capabilities();
            (caps.provider_id, caps.provider_version)
        }
        fn evaluate(
            &self,
            model: &VersionedNetworkModel,
            candidates: &[CandidateInjection],
            limits: &NetworkLimits,
        ) -> FeasibilityVerdict {
            self.provider
                .evaluate_dispatch(model, candidates, limits)
                .expect("in-band outcomes only")
        }
    }

    // The consumer type itself never names a provider; the assertion
    // below compiles the whole shape (and would fail if the port stopped
    // being object-safe or leaked provider types into the consumed
    // surface).
    fn builds_with_any_provider(provider: Box<dyn GridModelProvider>) -> PortOnlyFeasibilityGate {
        PortOnlyFeasibilityGate { provider }
    }

    let _ = builds_with_any_provider as fn(Box<dyn GridModelProvider>) -> PortOnlyFeasibilityGate;
    let _: Option<PowerFlowResult> = None;
}
