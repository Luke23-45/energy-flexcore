// P4.02 — Shared integration-test doubles and fixtures for the
// flexcore-grid-api contract tests.
//
// `ScriptedBackend` is the in-process backend double (the P3.19 pattern):
// it returns a scripted worker outcome and counts the calls, so the
// contract tests can prove the port flow and the pin cross-check without
// the later-wave worker adapter.
//
// `StubSecondProvider` is a second, fully independent
// `GridModelProvider` implementation (no backend seam, no adapter.rs
// code path) used to prove the port is real: the same candidate through
// the port interface produces the same verdict regardless of which
// adapter is behind it.

use flexcore_grid_api::backend::{
    BackendError, BackendInjection, BackendPowerFlowOutcome, BackendRunOptions, GridModelBackend,
};
use flexcore_grid_api::port::{
    AppliedInjectionRecord, CandidateInjection, CoverageLimitation, Diagnostic, GridModelRef,
    GridModelProvider, GridPortError, GridProviderCapabilities, NetworkLimits, PowerFlowResult,
    PowerFlowResiduals, PowerFlowStatus, VersionedNetworkModel,
};
use flexcore_grid_api::scope::{NotModelled, PhaseScope, ScopeDeclaration, TopologyScope};
use flexcore_contracts::units::{ActivePowerMW, ReactivePowerMVAr, ReactiveSignConvention};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// The pinned provider version the tests assert (kept here so the tests
/// fail loudly if the pin ever moves without the tests being reviewed).
pub const EXPECTED_PIN: &str = "3.5.6";

/// A scripted backend double: returns the scripted outcome, counts calls.
pub struct ScriptedBackend {
    pub outcome: BackendPowerFlowOutcome,
    pub calls: AtomicUsize,
}

impl ScriptedBackend {
    pub fn new(outcome: BackendPowerFlowOutcome) -> Self {
        Self {
            outcome,
            calls: AtomicUsize::new(0),
        }
    }

    pub fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl GridModelBackend for ScriptedBackend {
    fn run_power_flow(
        &self,
        _model: &VersionedNetworkModel,
        _injections: &[BackendInjection],
        _options: &BackendRunOptions,
    ) -> Result<BackendPowerFlowOutcome, BackendError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.outcome.clone())
    }

    fn describe(&self) -> flexcore_grid_api::backend::BackendDescription {
        flexcore_grid_api::backend::BackendDescription {
            name: "scripted-test-backend".to_string(),
            version: "0".to_string(),
            notes: vec!["in-process double for the contract tests".to_string()],
        }
    }
}

/// The reviewed scope of the stub second adapter: balanced, radial-only,
/// a deliberately DIFFERENT declaration from the pandapower-path scope —
/// proving the verdict contract holds across different declared scopes.
pub fn stub_scope() -> ScopeDeclaration {
    ScopeDeclaration::new(
        "stub-feeder-evaluator".to_string(),
        PhaseScope::Balanced,
        TopologyScope::RadialOnly,
        vec![
            flexcore_grid_api::scope::EquipmentKind::Line,
            flexcore_grid_api::scope::EquipmentKind::ExternalGrid,
        ],
        vec![
            NotModelled::new(
                "transformers".to_string(),
                "the stub evaluates line-only feeders; transformers are not modelled"
                    .to_string(),
            )
            .expect("valid limitation"),
            NotModelled::new(
                "ac_opf".to_string(),
                "the stub runs an AC power-flow evaluation; it is NOT an AC-OPF optimizer"
                    .to_string(),
            )
            .expect("valid limitation"),
        ],
        "runs an AC power-flow evaluation of specified injections and controls; \
         it is NOT an AC-OPF optimizer"
            .to_string(),
        vec!["in-crate stub proving the port is real (P4.02 contract test)".to_string()],
    )
    .expect("the stub scope is valid")
}

/// A second, fully independent provider implementation: it maps its own
/// worker-shaped outcome onto the port result WITHOUT the backend seam,
/// without `adapter.rs`, and without the pandapower-path mapping code —
/// only the port types and the port-level translation units are shared.
pub struct StubSecondProvider {
    pub outcome: BackendPowerFlowOutcome,
    pub scope: ScopeDeclaration,
}

impl GridModelProvider for StubSecondProvider {
    fn capabilities(&self) -> GridProviderCapabilities {
        GridProviderCapabilities {
            provider_id: "stub-feeder-evaluator".to_string(),
            // A different provider version on purpose: the verdict must
            // not depend on it.
            provider_version: "9.9.9-stub".to_string(),
            scope: self.scope.clone(),
        }
    }

    fn run_power_flow(
        &self,
        model: &VersionedNetworkModel,
        injections: &[CandidateInjection],
    ) -> Result<PowerFlowResult, GridPortError> {
        model
            .validate()
            .map_err(flexcore_grid_api::port::GridPortError::MalformedModel)?;
        for injection in injections {
            injection
                .validate(model)
                .map_err(flexcore_grid_api::port::GridPortError::InvalidCandidate)?;
        }

        let status = if self.outcome.converged {
            PowerFlowStatus::Converged
        } else {
            PowerFlowStatus::DidNotConverge
        };

        // The stub's own mapping (independent of the pandapower-path
        // adapter), using the shared port-level translation units only.
        let mut bus_voltages = Vec::new();
        let mut branch_loadings = Vec::new();
        if status == PowerFlowStatus::Converged {
            for bus in &model.buses {
                let row = self
                    .outcome
                    .bus_results
                    .iter()
                    .find(|r| r.bus_id == bus.bus_id)
                    .expect("stub outcome covers every bus");
                bus_voltages.push(
                    flexcore_grid_api::translation::bus_voltage_from_backend(bus, row)
                        .expect("stub outcome values are valid"),
                );
            }
            for branch in &model.branches {
                if let Some(row) = self
                    .outcome
                    .branch_results
                    .iter()
                    .find(|r| r.branch_id == branch.branch_id)
                {
                    branch_loadings.push(
                        flexcore_grid_api::translation::branch_loading_from_backend(branch, row)
                            .expect("stub outcome values are valid"),
                    );
                }
            }
        }

        let result = PowerFlowResult {
            status,
            model_ref: model.model_ref.clone(),
            provider_id: "stub-feeder-evaluator".to_string(),
            provider_version: "9.9.9-stub".to_string(),
            bus_voltages,
            branch_loadings,
            transformer_loadings: Vec::new(),
            applied_injections: injections
                .iter()
                .map(|injection| AppliedInjectionRecord {
                    injection_id: injection.injection_id.clone(),
                    bus_id: injection.bus_id.clone(),
                    active_power_mw: injection.active_power.value(),
                    reactive_power_mvar: injection.reactive_power.value(),
                    reactive_sign_convention: injection.reactive_sign_convention,
                    interval_start: injection.interval_start,
                })
                .collect(),
            residuals: if status == PowerFlowStatus::Converged {
                Diagnostic::Available(PowerFlowResiduals {
                    iterations: self.outcome.iterations,
                    max_power_mismatch_mw: None,
                    max_voltage_update_pu: None,
                    provider_status_text: self.outcome.worker_status_text.clone(),
                })
            } else {
                Diagnostic::NotAvailable {
                    reason: "the stub did not converge; no converged residuals exist"
                        .to_string(),
                }
            },
            coverage_limitations: self
                .scope
                .coverage_limitation_statements()
                .into_iter()
                .map(|statement| {
                    let (label, rest) = statement
                        .split_once(':')
                        .unwrap_or(("", statement.as_str()));
                    CoverageLimitation {
                        label: label.trim().to_string(),
                        statement: rest.trim().to_string(),
                    }
                })
                .collect(),
            warnings: Vec::new(),
            runtime: Duration::from_millis(1),
        };
        result
            .validate_complete()
            .map_err(flexcore_grid_api::port::GridPortError::IncompleteResult)?;
        Ok(result)
    }
}

/// A validated candidate injection fixture (export 2.5 MW, inject 0.5
/// MVAr under the declared injection-positive convention).
pub fn export_candidate(bus_id: &str) -> CandidateInjection {
    CandidateInjection {
        injection_id: "cand-001".to_string(),
        bus_id: bus_id.to_string(),
        active_power: ActivePowerMW::new(2.5).expect("valid"),
        reactive_power: ReactivePowerMVAr::new(0.5).expect("valid"),
        reactive_sign_convention: ReactiveSignConvention::InjectionPositive,
        interval_start: None,
    }
}

/// A second candidate: import 1.25 MW, consume 0.25 MVAr under the
/// declared consumption-positive convention.
pub fn import_candidate(bus_id: &str) -> CandidateInjection {
    CandidateInjection {
        injection_id: "cand-002".to_string(),
        bus_id: bus_id.to_string(),
        active_power: ActivePowerMW::new(-1.25).expect("valid"),
        reactive_power: ReactivePowerMVAr::new(0.25).expect("valid"),
        reactive_sign_convention: ReactiveSignConvention::ConsumptionPositive,
        interval_start: None,
    }
}

/// The limit configuration used by the contract tests.
pub fn test_limits() -> NetworkLimits {
    NetworkLimits {
        voltage_min_pu: 0.9,
        voltage_max_pu: 1.1,
        max_branch_loading_fraction: 0.9,
        max_transformer_loading_fraction: 0.9,
    }
}

/// The versioned model reference fixture.
pub fn model_ref() -> GridModelRef {
    GridModelRef {
        model_id: "feeder-contract-test".to_string(),
        version: "1".to_string(),
        content_hash: "hash-contract-test".to_string(),
    }
}
