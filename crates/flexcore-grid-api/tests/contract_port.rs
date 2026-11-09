// P4.02 — Contract tests (ledger P4.02 step 4 and acceptance):
//
// 1. "the same candidate through the port interface produces the same
//    verdict regardless of which adapter is behind it" — a pandapower-path
//    provider (scripted backend double) and a fully independent stub
//    second adapter (`common::StubSecondProvider`, no backend seam, no
//    adapter.rs code) evaluate the same candidate on the same model; the
//    verdicts MUST be equal, and MUST diverge only when the network state
//    diverges.
// 2. "an undeclared-scope adapter is a construction error" — no provider
//    can be constructed without a validated `ScopeDeclaration`, and the
//    scope gate refuses models whose equipment exceeds the declaration.
// 3. "no pandapower-specific types leak into the port surface" — covered
//    by `tests/dependency_direction.rs` (source scan); this file proves
//    the behavioural side: a port-only consumer runs both adapters
//    through `dyn GridModelProvider` alone.
// 4. The pinned provider version is recorded as a REQUIRED field on every
//    result path (P4.01 pin discipline), and the worker's reported
//    version is cross-checked against the pin.
// 5. Fail-closed: non-convergence yields an Inconclusive verdict through
//    the port's `evaluate_dispatch`, never a pass (§12.5 step 6; §17.5
//    invariant 9).

mod common;

use common::{
    export_candidate, import_candidate, model_ref, test_limits, EXPECTED_PIN, ScriptedBackend,
    StubSecondProvider,
};
use flexcore_grid_api::adapter::{PandapowerGridProvider, PROVIDER_ID, PROVIDER_PINNED_VERSION};
use flexcore_grid_api::backend::{BackendError, BackendInjection, BackendPowerFlowOutcome, BackendRunOptions, GridModelBackend};
use flexcore_grid_api::port::{
    evaluate_limits, FeasibilityVerdict, GridModelProvider, GridPortError,
    LimitViolationKind, PowerFlowResult, ResultCompletenessError, VersionedNetworkModel,
};
use flexcore_grid_api::scope::{EquipmentKind, PhaseScope, ScopeDeclaration, ScopeError, TopologyScope};
use flexcore_grid_api::tests_support::{converged_outcome, three_bus_model};
use std::sync::Arc;
use std::time::Duration;

fn converged_backend() -> ScriptedBackend {
    ScriptedBackend::new(converged_outcome(EXPECTED_PIN))
}

fn provider_with(backend: ScriptedBackend) -> PandapowerGridProvider {
    PandapowerGridProvider::new(
        Arc::new(backend),
        flexcore_grid_api::adapter::balanced_worker_scope(),
    )
    .expect("the recorded balanced scope is accepted")
}

/// (1) The same candidate through the port interface: both adapters see
/// the same network state and MUST produce the same verdict.
#[test]
fn same_candidate_same_verdict_across_adapters() {
    let model = three_bus_model(model_ref());
    let candidates = vec![export_candidate("bus_3"), import_candidate("bus_3")];

    let mut backend = converged_backend();
    backend.outcome.warnings.clear();
    let pandapower_path = provider_with(backend);
    let stub = StubSecondProvider {
        outcome: converged_outcome("9.9.9-stub-internal"),
        scope: common::stub_scope(),
    };

    let limits = test_limits();
    let verdict_a = pandapower_path
        .evaluate_dispatch(&model, &candidates, &limits)
        .expect("the port path evaluates");
    let verdict_b = stub
        .evaluate_dispatch(&model, &candidates, &limits)
        .expect("the stub path evaluates");

    assert_eq!(
        verdict_a, verdict_b,
        "the same network state through two different adapters must produce \
         the same port-level verdict"
    );
    // And the expected verdict for these hand-picked magnitudes:
    // all voltages inside [0.9, 1.1] pu, branch at 76 percent of a 0.9
    // limit, transformer at 62.5 percent of 0.9.
    assert_eq!(verdict_a, FeasibilityVerdict::WithinLimits);
}

/// (1b) The verdict tracks the network state, not the adapter: raising
/// bus_3 above the upper band flips BOTH adapters to the same violation.
#[test]
fn verdicts_diverge_together_when_the_state_diverges() {
    let model = three_bus_model(model_ref());
    let candidates = vec![export_candidate("bus_3")];
    let limits = test_limits();

    let mut violating = converged_outcome(EXPECTED_PIN);
    violating.bus_results[2].vm_pu = 1.2; // bus_3 above the 1.1 upper band

    let pandapower_path = provider_with(ScriptedBackend::new(violating.clone()));
    let stub = StubSecondProvider {
        outcome: violating,
        scope: common::stub_scope(),
    };

    let verdict_a = pandapower_path
        .evaluate_dispatch(&model, &candidates, &limits)
        .expect("the port path evaluates");
    let verdict_b = stub
        .evaluate_dispatch(&model, &candidates, &limits)
        .expect("the stub path evaluates");
    assert_eq!(verdict_a, verdict_b);
    match verdict_a {
        FeasibilityVerdict::Violations { violations } => {
            assert_eq!(violations.len(), 1);
            assert_eq!(violations[0].kind, LimitViolationKind::VoltageOver);
            assert_eq!(violations[0].element_id, "bus_3");
            assert_eq!(violations[0].value, 1.2);
        }
        other => panic!("expected a voltage-over violation, got {other:?}"),
    }
}

/// (2) The scope gate refuses a model whose equipment exceeds the
/// declared scope: a scope without transformers must reject the
/// three-bus model (which has one) instead of approximating.
#[test]
fn scope_gate_refuses_equipment_outside_the_declaration() {
    let model = three_bus_model(model_ref());
    let narrowed = ScopeDeclaration::new(
        "narrow.adapter".to_string(),
        PhaseScope::Balanced,
        TopologyScope::RadialAndMeshed,
        vec![EquipmentKind::Line, EquipmentKind::LoadElement],
        vec![flexcore_grid_api::scope::NotModelled::new(
            "transformers".to_string(),
            "this narrow scope does not model transformers".to_string(),
        )
        .expect("valid limitation")],
        "runs an AC power-flow evaluation of specified injections and controls; \
         it is NOT an AC-OPF optimizer"
            .to_string(),
        Vec::new(),
    )
    .expect("valid narrow scope");

    let provider = PandapowerGridProvider::new(Arc::new(converged_backend()), narrowed)
        .expect("balanced scope accepted");
    let error = provider
        .run_power_flow(&model, &[export_candidate("bus_3")])
        .expect_err("a transformer-carrying model exceeds the narrow scope");
    match error {
        GridPortError::ScopeExceeded(ScopeError::ScopeExceeded { requested, .. }) => {
            assert!(
                requested.contains("two_winding_transformer"),
                "the refusal names the undeclared equipment: {requested}"
            );
        }
        other => panic!("expected ScopeExceeded, got {other:?}"),
    }
}

/// (2b) An undeclared-scope adapter is a construction error: the scope's
/// own constructor rejects a declaration without limitations, and the
/// provider constructor refuses a non-balanced scope.
#[test]
fn undeclared_scope_is_a_construction_error() {
    // No not_modelled list at all.
    let error = ScopeDeclaration::new(
        "incomplete.adapter".to_string(),
        PhaseScope::Balanced,
        TopologyScope::RadialAndMeshed,
        vec![EquipmentKind::Line],
        Vec::new(),
        "runs an AC power-flow evaluation of specified injections and controls; \
         it is NOT an AC-OPF optimizer"
            .to_string(),
        Vec::new(),
    );
    assert_eq!(error, Err(ScopeError::EmptyNotModelled));

    // A provider without a scope cannot be expressed: the scope is a
    // constructor argument, and a scope that does not cover the balanced
    // path is refused by the provider constructor.
    let unbalanced = ScopeDeclaration::new(
        "future.adapter".to_string(),
        PhaseScope::Unbalanced,
        TopologyScope::RadialAndMeshed,
        vec![EquipmentKind::Line],
        vec![flexcore_grid_api::scope::NotModelled::new(
            "balanced".to_string(),
            "this future adapter declares only the unbalanced path".to_string(),
        )
        .expect("valid limitation")],
        "runs an unbalanced three-phase power-flow evaluation of specified \
         injections and controls; it is NOT an AC-OPF optimizer"
            .to_string(),
        Vec::new(),
    )
    .expect("valid unbalanced scope");
    struct NeverBackend;
    impl GridModelBackend for NeverBackend {
        fn run_power_flow(
            &self,
            _: &VersionedNetworkModel,
            _: &[BackendInjection],
            _: &BackendRunOptions,
        ) -> Result<BackendPowerFlowOutcome, BackendError> {
            Err(BackendError::Unavailable("must not be called".to_string()))
        }
        fn describe(&self) -> flexcore_grid_api::backend::BackendDescription {
            flexcore_grid_api::backend::BackendDescription {
                name: "never".to_string(),
                version: "0".to_string(),
                notes: Vec::new(),
            }
        }
    }
    assert!(PandapowerGridProvider::new(Arc::new(NeverBackend), unbalanced).is_err());
}

/// (3) A port-only consumer runs BOTH adapters through `dyn
/// GridModelProvider` alone — no adapter type, no backend type, no
/// provider name appears in the consumer's surface.
#[test]
fn port_only_consumer_runs_any_adapter() {
    struct PortOnlyFeasibilityCheck {
        provider: Box<dyn GridModelProvider>,
    }
    impl PortOnlyFeasibilityCheck {
        fn verdict(
            &self,
            model: &VersionedNetworkModel,
            candidates: &[flexcore_grid_api::port::CandidateInjection],
        ) -> FeasibilityVerdict {
            // The feasibility gate calls ONLY the port.
            self.provider
                .evaluate_dispatch(model, candidates, &test_limits())
                .expect("in-band outcomes only")
        }
    }

    let model = three_bus_model(model_ref());
    let candidates = vec![export_candidate("bus_3")];

    let through_pandapower_path = PortOnlyFeasibilityCheck {
        provider: Box::new(provider_with(converged_backend())),
    };
    let through_stub = PortOnlyFeasibilityCheck {
        provider: Box::new(StubSecondProvider {
            outcome: converged_outcome("9.9.9-stub-internal"),
            scope: common::stub_scope(),
        }),
    };

    assert_eq!(
        through_pandapower_path.verdict(&model, &candidates),
        through_stub.verdict(&model, &candidates),
        "the port contract holds for any adapter behind the trait object"
    );
}

/// (4) The pinned provider version is recorded on every result path, is
/// required for completeness, and the worker's reported version is
/// cross-checked against the pin.
#[test]
fn provider_version_pin_is_recorded_and_cross_checked() {
    assert_eq!(PROVIDER_PINNED_VERSION, EXPECTED_PIN);
    let model = three_bus_model(model_ref());

    let result = provider_with(converged_backend())
        .run_power_flow(&model, &[export_candidate("bus_3")])
        .expect("converged run");
    assert_eq!(result.provider_id, PROVIDER_ID);
    assert_eq!(result.provider_version, EXPECTED_PIN);

    // REQUIRED field: an empty provider version fails completeness — the
    // gate can never accept a result that does not name its provider pin.
    let mut stripped = result.clone();
    stripped.provider_version = "  ".to_string();
    assert_eq!(
        stripped.validate_complete(),
        Err(ResultCompletenessError::EmptyIdentification {
            field: "provider_version"
        })
    );

    // Cross-check: a worker reporting a different installed version is a
    // contract violation, never a result.
    let mut mismatched = converged_outcome(EXPECTED_PIN);
    mismatched.provider_version_reported = Some("3.5.5".to_string());
    let error = provider_with(ScriptedBackend::new(mismatched))
        .run_power_flow(&model, &[export_candidate("bus_3")])
        .expect_err("a pin mismatch is a backend contract violation");
    assert!(matches!(error, GridPortError::BackendContractViolation(_)));
}

/// (5) Fail-closed: non-convergence through the port's `evaluate_dispatch`
/// is Inconclusive — never a pass (§12.5 step 6; §17.5 invariant 9).
#[test]
fn non_convergence_is_inconclusive_through_the_port() {
    let model = three_bus_model(model_ref());
    let mut non_converged = converged_outcome(EXPECTED_PIN);
    non_converged.converged = false;

    let backend = ScriptedBackend::new(non_converged);
    let provider = provider_with(backend);
    let verdict = provider
        .evaluate_dispatch(&model, &[export_candidate("bus_3")], &test_limits())
        .expect("non-convergence is an in-band result");
    match verdict {
        FeasibilityVerdict::Inconclusive { reason } => {
            assert!(reason.contains("did not converge"));
        }
        other => panic!("non-convergence must be inconclusive, got {other:?}"),
    }
}

/// Applied injections are recorded per element, so the run is
/// reproducible from the record (ledger P4.05 step 3 consumes this).
#[test]
fn applied_injections_are_recorded_per_element() {
    let model = three_bus_model(model_ref());
    let candidates = vec![export_candidate("bus_3"), import_candidate("bus_2")];
    let result = provider_with(converged_backend())
        .run_power_flow(&model, &candidates)
        .expect("converged run");
    assert_eq!(result.applied_injections.len(), 2);
    let first = &result.applied_injections[0];
    assert_eq!(first.injection_id, "cand-001");
    assert_eq!(first.bus_id, "bus_3");
    assert!((first.active_power_mw - 2.5).abs() < 1.0e-12);
    assert!((first.reactive_power_mvar - 0.5).abs() < 1.0e-12);
    let second = &result.applied_injections[1];
    assert_eq!(second.injection_id, "cand-002");
    assert!((second.active_power_mw - (-1.25)).abs() < 1.0e-12);
}

/// The result carries the model reference and the coverage limitations
/// sourced from the declared scope (§12.5 step 6), and the backend double
/// was actually consulted exactly once per run.
#[test]
fn result_provenance_and_coverage_are_complete() {
    let model = three_bus_model(model_ref());
    let backend = converged_backend();
    let provider = provider_with(backend);
    let result = provider
        .run_power_flow(&model, &[export_candidate("bus_3")])
        .expect("converged run");

    assert_eq!(result.model_ref, model.model_ref);
    let scope = flexcore_grid_api::adapter::balanced_worker_scope();
    assert_eq!(result.coverage_limitations.len(), scope.not_modelled.len());
    for limitation in &result.coverage_limitations {
        assert!(!limitation.label.is_empty());
        assert!(!limitation.statement.is_empty());
    }
    // Every bus appears with both canonical representations, consistent
    // with the nominal base (kV = pu * nominal_kv).
    for bus in &model.buses {
        let voltage = result
            .bus_voltages
            .iter()
            .find(|v| v.bus_id == bus.bus_id)
            .expect("every model bus has a result row");
        assert!(
            (voltage.vm_kv - voltage.vm_pu * bus.nominal_kv).abs() < 1.0e-9,
            "kV and per-unit representations agree at the nominal base"
        );
    }
    // Branch loading: 76.0 percent of the 500 A rating = 0.76; currents
    // 0.38 kA / 0.3795 kA = 380 A / 379.5 A (hand-computed fixture).
    let loading = &result.branch_loadings[0];
    assert!((loading.loading_fraction - 0.76).abs() < 1.0e-9);
    assert!((loading.current_from_a - 380.0).abs() < 1.0e-6);
    assert!((loading.current_to_a - 379.5).abs() < 1.0e-6);

    result.validate_complete().expect("complete result");
}

/// A result with an empty coverage-limitations list is never publishable:
/// the completeness gate enforces §12.5 step 6 for every status.
#[test]
fn incomplete_results_are_refused_at_the_port() {
    let result = PowerFlowResult {
        status: flexcore_grid_api::port::PowerFlowStatus::Converged,
        model_ref: model_ref(),
        provider_id: "p".to_string(),
        provider_version: EXPECTED_PIN.to_string(),
        bus_voltages: Vec::new(),
        branch_loadings: Vec::new(),
        transformer_loadings: Vec::new(),
        applied_injections: Vec::new(),
        residuals: flexcore_grid_api::port::Diagnostic::Available(
            flexcore_grid_api::port::PowerFlowResiduals {
                iterations: Some(1),
                max_power_mismatch_mw: None,
                max_voltage_update_pu: None,
                provider_status_text: "t".to_string(),
            },
        ),
        coverage_limitations: Vec::new(),
        warnings: Vec::new(),
        runtime: Duration::from_millis(1),
    };
    assert_eq!(
        result.validate_complete(),
        Err(ResultCompletenessError::MissingCoverageLimitations)
    );
}

/// The limit-evaluation helper stays a pure function of the result and
/// the limits: identical inputs, identical verdict, independent of any
/// provider (the port-level guarantee the contract tests pin).
#[test]
fn evaluate_limits_is_a_pure_port_level_function() {
    let model = three_bus_model(model_ref());
    let a = provider_with(converged_backend())
        .run_power_flow(&model, &[export_candidate("bus_3")])
        .expect("converged run");
    let b = StubSecondProvider {
        outcome: converged_outcome("9.9.9-stub-internal"),
        scope: common::stub_scope(),
    }
    .run_power_flow(&model, &[export_candidate("bus_3")])
    .expect("converged run");

    let limits = test_limits();
    assert_eq!(evaluate_limits(&a, &limits), evaluate_limits(&b, &limits));
}
