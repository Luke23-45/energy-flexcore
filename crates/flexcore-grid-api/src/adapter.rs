// P4.02 — The Rust-side provider for the pandapower path (ledger P4.02
// step 2; report §9.1, §12.4, §7.2).
//
// THIS MODULE NAMES THE PROVIDER — it is adapter-side code, not port
// surface. The feasibility gate consumes only `crate::port`; the
// dependency-direction test (`tests/dependency_direction.rs`) enforces
// that the port-surface modules never name the provider.
//
// Recorded decisions (decision note
// `ledger/phase-04-grid-feasibility-dispatch/01_02_grid_provider.md`):
//
// - **D-P4.02-1 — provider path.** The AC power-flow evaluation executes
//   in the Python numerical worker (§7.2) via the pinned provider version,
//   behind the transport-neutral [`crate::backend::GridModelBackend`] seam
//   (the D-P3.17-1 pattern). The worker adapter lands in a later wave;
//   this module is the Rust-side provider logic that composes scope
//   gating → translation → backend call → result mapping → completeness
//   gate, and is fully tested against backend doubles.
// - **D-P4.01-1 — provider selection (a RECOMMENDATION, not a decision).**
//   The provider is the recommended default (report §25.2, DECISIONS.md).
//   D3 (network class and model fidelity) stays OPEN — stated verbatim:
//   "Network class and model fidelity | Balanced radial distribution,
//   unbalanced three-phase, and meshed transmission require different
//   models and solvers | Phase 0; power systems". This provider does not
//   decide D3.
// - **D-P4.01-2 — provider boundary (§12.4).** The provider is the
//   balanced AC power-flow path with SOME three-phase/asymmetric
//   calculations; it is NOT a universal validator. Model and solver
//   limitations are verified per case; the declared scope below is the
//   machine-checked boundary record. The provider's OPF capability is not
//   the feasibility gate and is not wired in (§12.5; RESEARCH.md §7).
// - **D-P4.01-3 — provider-addition rule.** A second provider (OpenDSS,
//   GridLAB-D, PyPSA) is added ONLY when a named test case cannot be
//   expressed or validated with the primary provider; the case is
//   recorded. No familiarity-based additions.
//
// Simulation-only discipline (0.x line): this crate evaluates network
// states in simulation. Nothing here touches a device, issues a command,
// or authorises field control; a within-limits verdict means the candidate
// satisfied the declared model and limits within stated tolerances — not
// that it is safe in the field, certified, or approved (§12.5; ledger
// CONTEXT.md §5).
//
// Per-provider residual availability (recorded, feeds the result contract):
// the iteration count is recorded when the worker exposes it (VERIFY-BUILD
// item 3 in `backend.rs`); bus-power-mismatch residuals are
// `NotAvailable` with a recorded reason until a documented source in the
// pinned provider is verified (VERIFY-BUILD item 4). Silence is never
// published.

use crate::backend::{
    BackendBranchResult, BackendBusResult, BackendError, BackendInjection, BackendPowerFlowOutcome,
    BackendRunOptions, BackendTransformerResult, GridModelBackend,
};
use crate::port::{
    AppliedInjectionRecord, CandidateInjection, CoverageLimitation, Diagnostic, GridModelProvider,
    GridPortError, GridProviderCapabilities, PowerFlowResult, PowerFlowResiduals, PowerFlowStatus,
    VersionedNetworkModel,
};
use crate::scope::{EquipmentKind, PhaseScope, ScopeDeclaration, TopologyScope};
use crate::translation::{
    branch_loading_from_backend, bus_voltage_from_backend, candidate_to_backend,
    transformer_loading_from_backend, TranslationError,
};
use std::sync::Arc;
use std::time::Instant;

/// The pinned provider version (decision note §2; RESEARCH.md §1:
/// verified 2026-11-07, PyPI release files; released 7 October 2026;
/// license BSD-3-Clause — D4 audit input, the formal audit is OPEN).
///
/// Pin discipline (P4.01): the pin is recorded in the worker lockfile
/// note (`Cargo.toml` coordinator note) and carried as a REQUIRED field on
/// every result. The provider cross-checks the worker's reported installed
/// version against this pin and refuses a mismatch — a result whose
/// provider version disagrees with the recorded pin is a contract
/// violation, never a warning to publish anyway.
pub const PROVIDER_PINNED_VERSION: &str = "3.5.6";

/// The provider identifier used in every result and capability statement.
pub const PROVIDER_ID: &str = "pandapower-worker";

/// The module-level decision references, surfaced for tests and records.
pub const PROVIDER_BOUNDARY_STATEMENT: &str =
    "Balanced AC power flow with some three-phase/asymmetric calculations; \
     NOT a universal validator (report §12.4). Verify model and solver \
     limitations for each case. The provider's OPF capability is not the \
     feasibility gate.";

pub const D3_STATUS_STATEMENT: &str =
    "D3 (network class and model fidelity) is OPEN. The balanced-radial \
     default and this provider are a RECOMMENDATION (report §25.2), never \
     a decision.";

pub const PROVIDER_ADDITION_RULE: &str =
    "A second provider (OpenDSS, GridLAB-D, PyPSA) is added ONLY when a \
     named test case cannot be expressed or validated with the primary \
     provider; the case is recorded. No familiarity-based additions \
     (D-P4.01-3).";

/// The declared scope of the v0.3.0 balanced adapter.
///
/// Modelled: balanced (positive-sequence) AC power flow, radial and meshed
/// topologies, lines, two-winding transformers, load elements, static
/// generators, external grid.
///
/// NOT modelled (the reviewed limitation record; surfaced verbatim on
/// every result as model-coverage limitations, §12.5 step 6):
/// unbalanced three-phase asymmetry (enters from v0.4.0 per the
/// recommended default — D3 OPEN), protection coordination, dynamic and
/// transient behaviour, sub-cycle control, optimal power flow (the gate
/// never optimizes).
///
/// VERIFY-BUILD (worker-adapter wave): confirm against the pinned
/// provider's documentation that the declared modelled set matches what
/// the worker actually exercises (element types materialised, slack
/// handling), and that no declared-not-modelled capability is silently
/// exercised through a provider default.
pub fn balanced_worker_scope() -> ScopeDeclaration {
    ScopeDeclaration::new(
        PROVIDER_ID.to_string(),
        PhaseScope::Balanced,
        TopologyScope::RadialAndMeshed,
        vec![
            EquipmentKind::Line,
            EquipmentKind::TwoWindingTransformer,
            EquipmentKind::LoadElement,
            EquipmentKind::StaticGenerator,
            EquipmentKind::ExternalGrid,
        ],
        vec![
            crate::scope::NotModelled::new(
                "unbalanced_three_phase".to_string(),
                "unbalanced three-phase asymmetry is not modelled by the v0.3.0 \
                 balanced adapter; unbalanced three-phase modelling enters the \
                 track from v0.4.0 per the recommended default (D3 OPEN)"
                    .to_string(),
            )
            .expect("valid limitation"),
            crate::scope::NotModelled::new(
                "protection".to_string(),
                "protection coordination is not modelled; FlexCore is a supervisory \
                 orchestration layer and makes no protection-grade claims"
                    .to_string(),
            )
            .expect("valid limitation"),
            crate::scope::NotModelled::new(
                "dynamics_transient".to_string(),
                "dynamic and transient behaviour is not modelled; a steady-state \
                 power-flow result is not evidence for transient stability"
                    .to_string(),
            )
            .expect("valid limitation"),
            crate::scope::NotModelled::new(
                "sub_cycle_control".to_string(),
                "sub-cycle control is not modelled; FlexCore performs no sub-cycle \
                 control"
                    .to_string(),
            )
            .expect("valid limitation"),
            crate::scope::NotModelled::new(
                "ac_opf".to_string(),
                "optimal power flow is not modelled; the feasibility gate runs an AC \
                 power-flow evaluation of specified injections and controls and \
                 never optimizes (§12.5)"
                    .to_string(),
            )
            .expect("valid limitation"),
        ],
        "runs an AC power-flow evaluation of specified injections and controls; \
         it is NOT an AC-OPF optimizer (§12.5)"
            .to_string(),
        vec![
            format!("provider pin: {PROVIDER_PINNED_VERSION} (RESEARCH.md §1, 2026-11-07)"),
            PROVIDER_BOUNDARY_STATEMENT.to_string(),
            D3_STATUS_STATEMENT.to_string(),
        ],
    )
    .expect("the recorded balanced scope is a valid scope declaration")
}

/// The Rust-side provider for the pandapower path (the §9.1
/// `GridModelProvider` implementation for Phase 4).
///
/// Constructed with a backend seam handle and a declared scope — there is
/// no way to build this provider without a scope (an undeclared-scope
/// adapter is a construction error, ledger P4.02 step 3). The constructor
/// additionally rejects a scope that does not declare the balanced phase:
/// this provider implements exactly the balanced path in v0.3.0; an
/// unbalanced declaration belongs to a future adapter (D3 OPEN).
pub struct PandapowerGridProvider {
    backend: Arc<dyn GridModelBackend>,
    scope: ScopeDeclaration,
}

impl PandapowerGridProvider {
    /// Constructs the provider over the given backend seam with the given
    /// declared scope. Fails when the scope is not a balanced-phase
    /// scope (see the type docs).
    pub fn new(
        backend: Arc<dyn GridModelBackend>,
        scope: ScopeDeclaration,
    ) -> Result<Self, GridPortError> {
        if !scope.covers_phase(PhaseScope::Balanced) {
            return Err(GridPortError::ScopeExceeded(
                crate::scope::ScopeError::ScopeExceeded {
                    requested: "this provider implements the balanced path only"
                        .to_string(),
                    declared: format!("{} phase scope", scope.phase.name()),
                },
            ));
        }
        Ok(Self { backend, scope })
    }

    /// The worker's identity, for records.
    pub fn backend_description(&self) -> crate::backend::BackendDescription {
        self.backend.describe()
    }

    /// Maps the raw worker outcome onto the port result (scope-gated,
    /// translated, provenance-complete). Internal; public for tests of
    /// the mapping table.
    fn map_outcome(
        &self,
        model: &VersionedNetworkModel,
        applied: &[AppliedInjectionRecord],
        outcome: BackendPowerFlowOutcome,
        runtime: std::time::Duration,
        warnings: &mut Vec<String>,
    ) -> Result<PowerFlowResult, GridPortError> {
        // Pin cross-check (decision note §2): the worker's reported
        // installed version must agree with the recorded pin.
        if let Some(reported) = &outcome.provider_version_reported {
            if reported != PROVIDER_PINNED_VERSION {
                return Err(GridPortError::BackendContractViolation(format!(
                    "worker reported provider version '{reported}' but the recorded \
                     pin is '{PROVIDER_PINNED_VERSION}' (P4.01 pin discipline)"
                )));
            }
        } else {
            return Err(GridPortError::BackendContractViolation(
                "worker did not report its installed provider version; the pin \
                 cross-check is mandatory (decision note §2)"
                    .to_string(),
            ));
        }

        let status = if outcome.converged {
            PowerFlowStatus::Converged
        } else if outcome.reported_error.is_some() {
            PowerFlowStatus::ProviderReportedError
        } else {
            PowerFlowStatus::DidNotConverge
        };

        // Row-by-row translation; every model element must have exactly
        // one matching result row when the run converged (worker contract).
        let mut bus_voltages = Vec::with_capacity(model.buses.len());
        let mut branch_loadings = Vec::with_capacity(model.branches.len());
        let mut transformer_loadings = Vec::with_capacity(model.transformers.len());

        if status == PowerFlowStatus::Converged {
            for bus in &model.buses {
                let rows: Vec<&BackendBusResult> = outcome
                    .bus_results
                    .iter()
                    .filter(|r| r.bus_id == bus.bus_id)
                    .collect();
                if rows.len() != 1 {
                    return Err(GridPortError::BackendContractViolation(format!(
                        "expected exactly one bus result row for '{}', found {}",
                        bus.bus_id,
                        rows.len()
                    )));
                }
                let voltage = bus_voltage_from_backend(bus, rows[0])
                    .map_err(translation_error)?;
                // kV/per-unit consistency is a translation invariant; a
                // violation here is a provider bug, caught at the gate.
                bus_voltages.push(voltage);
            }
            for branch in &model.branches {
                let rows: Vec<&BackendBranchResult> = outcome
                    .branch_results
                    .iter()
                    .filter(|r| r.branch_id == branch.branch_id)
                    .collect();
                if rows.len() != 1 {
                    return Err(GridPortError::BackendContractViolation(format!(
                        "expected exactly one branch result row for '{}', found {}",
                        branch.branch_id,
                        rows.len()
                    )));
                }
                let loading = branch_loading_from_backend(branch, rows[0])
                    .map_err(translation_error)?;
                // Cross-check the worker's loading fraction against the
                // declared rating base and the converted currents; a
                // divergence beyond the recorded tolerance is a warning,
                // not a silent pass (the worker's loading definition is
                // authoritative, the cross-check documents it).
                let computed = (loading.current_from_a.max(loading.current_to_a))
                    / branch.rated_current_a;
                if (computed - loading.loading_fraction).abs() > 1.0e-4 * computed.max(1.0) {
                    warnings.push(format!(
                        "branch '{}' loading fraction {:.6} diverges from \
                         max(I)/rating {:.6} beyond tolerance; the worker's \
                         loading definition is authoritative",
                        branch.branch_id, loading.loading_fraction, computed
                    ));
                }
                branch_loadings.push(loading);
            }
            for transformer in &model.transformers {
                let rows: Vec<&BackendTransformerResult> = outcome
                    .transformer_results
                    .iter()
                    .filter(|r| r.transformer_id == transformer.transformer_id)
                    .collect();
                if rows.len() != 1 {
                    return Err(GridPortError::BackendContractViolation(format!(
                        "expected exactly one transformer result row for '{}', found {}",
                        transformer.transformer_id,
                        rows.len()
                    )));
                }
                let loading = transformer_loading_from_backend(transformer, rows[0])
                    .map_err(translation_error)?;
                transformer_loadings.push(loading);
            }
        }

        // Residuals per the recorded per-provider availability (module
        // header): iterations when reported; mismatch residuals not
        // available with a recorded reason (VERIFY-BUILD item 4).
        let residuals = if status == PowerFlowStatus::Converged {
            Diagnostic::Available(PowerFlowResiduals {
                iterations: outcome.iterations,
                max_power_mismatch_mw: None,
                max_voltage_update_pu: None,
                provider_status_text: outcome.worker_status_text.clone(),
            })
        } else {
            Diagnostic::NotAvailable {
                reason: match status {
                    PowerFlowStatus::DidNotConverge => {
                        "the power flow did not converge; no converged residuals exist"
                            .to_string()
                    }
                    _ => "the provider reported an evaluation error; no residuals exist"
                        .to_string(),
                },
            }
        };

        let result = PowerFlowResult {
            status,
            model_ref: model.model_ref.clone(),
            provider_id: PROVIDER_ID.to_string(),
            provider_version: PROVIDER_PINNED_VERSION.to_string(),
            bus_voltages,
            branch_loadings,
            transformer_loadings,
            applied_injections: applied.to_vec(),
            residuals,
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
            warnings: {
                warnings.extend(outcome.warnings.clone());
                if let Some(error) = &outcome.reported_error {
                    warnings.push(format!("provider reported: {error}"));
                }
                warnings.clone()
            },
            runtime,
        };
        // The completeness gate (provider-internal guard): a provider
        // never publishes an incomplete result.
        result
            .validate_complete()
            .map_err(GridPortError::IncompleteResult)?;
        Ok(result)
    }
}

fn translation_error(error: TranslationError) -> GridPortError {
    GridPortError::BackendContractViolation(format!("translation failed: {error}"))
}

impl GridModelProvider for PandapowerGridProvider {
    fn capabilities(&self) -> GridProviderCapabilities {
        GridProviderCapabilities {
            provider_id: PROVIDER_ID.to_string(),
            provider_version: PROVIDER_PINNED_VERSION.to_string(),
            scope: self.scope.clone(),
        }
    }

    fn run_power_flow(
        &self,
        model: &VersionedNetworkModel,
        injections: &[CandidateInjection],
    ) -> Result<PowerFlowResult, GridPortError> {
        // 1. Structural validation of the caller's inputs.
        model.validate().map_err(GridPortError::MalformedModel)?;
        for injection in injections {
            injection
                .validate(model)
                .map_err(GridPortError::InvalidCandidate)?;
        }
        let mut duplicate_ids = Vec::new();
        for (index, injection) in injections.iter().enumerate() {
            if injections[index + 1..]
                .iter()
                .any(|other| other.injection_id == injection.injection_id)
            {
                duplicate_ids.push(injection.injection_id.clone());
            }
        }
        if !duplicate_ids.is_empty() {
            return Err(GridPortError::InvalidCandidate(
                crate::port::ModelError::DuplicateId {
                    structure: "candidate injection",
                    id: duplicate_ids.remove(0),
                },
            ));
        }

        // 2. Scope gate (ledger P4.02 step 3): the model's topology and
        // equipment must lie inside the declared scope, or the adapter
        // refuses. Radial/meshed classification of the model: a cycle in
        // the branch graph at equal voltage level means meshed; this
        // check is conservative and the declared RadialAndMeshed scope of
        // this adapter covers both — the gate below is still exercised so
        // the mechanism is pinned for future narrower scopes.
        model
            .equipment_kinds()
            .iter()
            .try_for_each(|kind| {
                self.scope
                    .assert_supports(PhaseScope::Balanced, true, true, &[*kind])
            })
            .map_err(GridPortError::ScopeExceeded)?;

        // 3. Translation: canonical candidates -> worker injections
        // (owned, tested code — never an assumption).
        let mut backend_injections: Vec<BackendInjection> = Vec::with_capacity(injections.len());
        let mut applied: Vec<AppliedInjectionRecord> = Vec::with_capacity(injections.len());
        for injection in injections {
            let mapped = candidate_to_backend(injection).map_err(translation_error)?;
            applied.push(AppliedInjectionRecord {
                injection_id: injection.injection_id.clone(),
                bus_id: injection.bus_id.clone(),
                active_power_mw: injection.active_power.value(),
                reactive_power_mvar: injection.reactive_power.value(),
                reactive_sign_convention: injection.reactive_sign_convention,
                interval_start: injection.interval_start,
            });
            backend_injections.push(mapped);
        }

        // 4. Backend call (transport-neutral seam).
        let options = BackendRunOptions {
            evaluation_label: format!(
                "grid-feasibility:{}:{}",
                model.model_ref.model_id, model.model_ref.version
            ),
            report_provider_version: true,
        };
        let started = Instant::now();
        let outcome = self
            .backend
            .run_power_flow(model, &backend_injections, &options)
            .map_err(|error| match error {
                BackendError::Unavailable(message) => GridPortError::BackendUnavailable(message),
                BackendError::ContractViolation(message) => {
                    GridPortError::BackendContractViolation(message)
                }
            })?;
        let runtime = started.elapsed();

        // 5. Mapping, provenance, completeness gate.
        let mut warnings = Vec::new();
        self.map_outcome(model, &applied, outcome, runtime, &mut warnings)
    }
}

/// Re-exported for the contract tests: the shared §9.1-shaped dispatch
/// evaluation lives on the port trait (`GridModelProvider::
/// evaluate_dispatch`) and must not be reimplemented adapter-side — the
/// verdict is a pure function of the result and the limits.
pub const DISPATCH_EVALUATION_STATEMENT: &str =
    "The limit verdict is computed at the port level (port::evaluate_limits) \
     as a pure function of the PowerFlowResult and the NetworkLimits; the \
     same result yields the same verdict regardless of which adapter \
     produced it.";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::BackendDescription;
    use crate::port::GridModelRef;
    use flexcore_contracts::units::{ActivePowerMW, ReactivePowerMVAr, ReactiveSignConvention};

    fn model_ref() -> GridModelRef {
        GridModelRef {
            model_id: "feeder-a".to_string(),
            version: "1".to_string(),
            content_hash: "hash-1".to_string(),
        }
    }

    fn injection(id: &str, bus: &str, p: f64, q: f64) -> CandidateInjection {
        CandidateInjection {
            injection_id: id.to_string(),
            bus_id: bus.to_string(),
            active_power: ActivePowerMW::new(p).expect("valid"),
            reactive_power: ReactivePowerMVAr::new(q).expect("valid"),
            reactive_sign_convention: ReactiveSignConvention::InjectionPositive,
            interval_start: None,
        }
    }

    fn outcome(converged: bool) -> BackendPowerFlowOutcome {
        BackendPowerFlowOutcome {
            converged,
            reported_error: None,
            provider_version_reported: Some(PROVIDER_PINNED_VERSION.to_string()),
            bus_results: Vec::new(),
            branch_results: Vec::new(),
            transformer_results: Vec::new(),
            iterations: Some(3),
            worker_status_text: "worker test status".to_string(),
            warnings: Vec::new(),
        }
    }

    fn scope() -> ScopeDeclaration {
        balanced_worker_scope()
    }

    fn provider(
        outcome: BackendPowerFlowOutcome,
    ) -> PandapowerGridProvider {
        struct Scripted(BackendPowerFlowOutcome);
        impl GridModelBackend for Scripted {
            fn run_power_flow(
                &self,
                _model: &VersionedNetworkModel,
                _injections: &[BackendInjection],
                _options: &BackendRunOptions,
            ) -> Result<BackendPowerFlowOutcome, BackendError> {
                Ok(self.0.clone())
            }
            fn describe(&self) -> BackendDescription {
                BackendDescription {
                    name: "scripted-test".to_string(),
                    version: "0".to_string(),
                    notes: Vec::new(),
                }
            }
        }
        PandapowerGridProvider::new(Arc::new(Scripted(outcome)), scope())
            .expect("balanced scope accepted")
    }

    #[test]
    fn provider_refuses_an_unbalanced_scope_declaration() {
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
            fn describe(&self) -> BackendDescription {
                BackendDescription {
                    name: "never".to_string(),
                    version: "0".to_string(),
                    notes: Vec::new(),
                }
            }
        }
        let unbalanced = ScopeDeclaration::new(
            "future.adapter".to_string(),
            PhaseScope::Unbalanced,
            TopologyScope::RadialAndMeshed,
            vec![EquipmentKind::Line],
            vec![
                crate::scope::NotModelled::new(
                    "balanced",
                    "this future adapter declares only the unbalanced path"
                        .to_string(),
                )
                .expect("valid"),
            ],
            "runs an unbalanced three-phase power-flow evaluation of specified \
             injections and controls; it is NOT an AC-OPF optimizer"
                .to_string(),
            Vec::new(),
        )
        .expect("valid scope");
        let error =
            PandapowerGridProvider::new(Arc::new(NeverBackend), unbalanced)
                .expect_err("an unbalanced scope must be refused");
        assert!(matches!(error, GridPortError::ScopeExceeded(_)));
    }

    #[test]
    fn provider_refuses_a_version_mismatch_from_the_worker() {
        let mut mismatched = outcome(true);
        mismatched.provider_version_reported = Some("3.5.5".to_string());
        let model = crate::tests_support::three_bus_model(model_ref());
        let error = provider(mismatched)
            .run_power_flow(&model, &[injection("i1", "bus_1", 1.0, 0.0)])
            .expect_err("a pin mismatch is a contract violation");
        assert!(matches!(
            error,
            GridPortError::BackendContractViolation(_)
        ));
    }

    #[test]
    fn provider_records_the_pin_on_every_result() {
        let model = crate::tests_support::three_bus_model(model_ref());
        let result = provider(outcome(true))
            .run_power_flow(&model, &[injection("i1", "bus_1", 1.0, 0.0)])
            .expect("converged run maps");
        assert_eq!(result.provider_id, PROVIDER_ID);
        assert_eq!(result.provider_version, PROVIDER_PINNED_VERSION);
        assert_eq!(result.status, PowerFlowStatus::Converged);
        // Coverage limitations present and sourced from the declared scope.
        assert_eq!(
            result.coverage_limitations.len(),
            scope().not_modelled.len()
        );
        result
            .validate_complete()
            .expect("the provider never publishes an incomplete result");
    }

    #[test]
    fn non_convergence_is_an_in_band_result_with_diagnostics() {
        let model = crate::tests_support::three_bus_model(model_ref());
        let result = provider(outcome(false))
            .run_power_flow(&model, &[injection("i1", "bus_1", 1.0, 0.0)])
            .expect("non-convergence is a result, not an error");
        assert_eq!(result.status, PowerFlowStatus::DidNotConverge);
        match &result.residuals {
            Diagnostic::NotAvailable { reason } => {
                assert!(reason.contains("did not converge"));
            }
            other => panic!("residuals must be not-available, got {other:?}"),
        }
        result
            .validate_complete()
            .expect("a non-converged result is complete");
    }
}
