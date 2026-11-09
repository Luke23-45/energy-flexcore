// P4.02 — The `GridModelProvider` port (report §9.1, §12.4, §12.5).
//
// ```text
// GridModelProvider
//   capabilities() -> GridProviderCapabilities      (scope declaration included)
//   run_power_flow(model, injections) -> PowerFlowResult
//   evaluate_dispatch(model, injections, limits) -> FeasibilityVerdict
// ```
//
// The Phase 4 feasibility gate depends on this contract, never on a grid
// provider directly (report §5.2, §12.4; ledger P4.02 acceptance:
// "feasibility code calls only the port"). Every adapter **declares** its
// model scope (a mandatory [`crate::scope::ScopeDeclaration`]) and refuses
// everything outside it; a provider never approximates.
//
// Solver-class discipline (AGENTS.md rule — absolute): this port runs an
// **AC power-flow evaluation of specified injections and controls**. It is
// NOT an AC-OPF optimizer, and nothing here optimizes (§12.5: "An AC
// power-flow check is a feasibility evaluation of specified
// injections/controls. It is not itself an AC-OPF optimization."). The
// feasibility gate never solves an optimization problem.
//
// Fail-closed discipline (§12.5; §17.5 invariant 9): non-convergence is a
// RESULT (reported with diagnostics, [`PowerFlowStatus::DidNotConverge`]),
// never silently retried and never treated as feasible; model-coverage
// limitations are carried on every result; a provider that cannot identify
// its pinned provider version cannot publish a result at all.
//
// Canonical units and sign (report §10.2; flexcore-contracts is the single
// home of the frozen rules): active power in MW with `P > 0` = export at
// the point of connection; reactive power in MVAr with the sign convention
// DECLARED per grid model and device profile (never inferred); voltage in
// kV at the nominal base and per-unit in the solver representation; current
// in A or per-unit on an explicit base. Injection candidates carry the
// frozen contract types [`ActivePowerMW`] and [`ReactivePowerMVAr`].
//
// This module contains **no provider-specific type**. The
// dependency-direction test (`tests/dependency_direction.rs`) enforces that
// the port-surface modules never name the concrete provider.

use crate::scope::{EquipmentKind, ScopeDeclaration, ScopeError};
use flexcore_contracts::units::{ActivePowerMW, ReactivePowerMVAr, ReactiveSignConvention};
use serde::{Deserialize, Serialize};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Versioned network model (provider-neutral IR)
// ---------------------------------------------------------------------------

/// Errors in a versioned network model or candidate set. These are caller
/// input errors — never provider outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    /// An identification field is empty.
    EmptyField { structure: &'static str, field: &'static str },
    /// Two elements share an identifier.
    DuplicateId { structure: &'static str, id: String },
    /// A numeric parameter is NaN/infinite or outside its required range
    /// (e.g. a non-positive rating).
    InvalidNumber { structure: &'static str, field: &'static str },
    /// A referenced bus does not exist in the model.
    UnknownBus { structure: &'static str, bus_id: String },
    /// The model has no slack source, or more than one.
    InvalidSlack { detail: String },
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelError::EmptyField { structure, field } => {
                write!(f, "{structure} field '{field}' is empty")
            }
            ModelError::DuplicateId { structure, id } => {
                write!(f, "duplicate {structure} id '{id}'")
            }
            ModelError::InvalidNumber { structure, field } => {
                write!(f, "{structure} field '{field}' is not a valid positive finite number")
            }
            ModelError::UnknownBus { structure, bus_id } => {
                write!(f, "{structure} references unknown bus '{bus_id}'")
            }
            ModelError::InvalidSlack { detail } => write!(f, "invalid slack configuration: {detail}"),
        }
    }
}

impl std::error::Error for ModelError {}

/// Reference to one immutable, versioned network model (P4.02 step 1: "load
/// a versioned network model"). The model identity travels onto every
/// result, so a verdict always names the model it was evaluated against.
/// Model versioning, content hashing, and health tracking are P4.03's
/// deliverable; this port consumes the versioned reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridModelRef {
    /// Stable model identifier (e.g. the site/feeder model id).
    pub model_id: String,
    /// Model version (immutable versions; new version on change, P4.03).
    pub version: String,
    /// Content hash of the exact model artefact, as assigned by the P4.03
    /// versioning layer. Carried verbatim; never recomputed here.
    pub content_hash: String,
}

impl GridModelRef {
    /// Validates that all three identity fields are non-empty.
    pub fn validate(&self) -> Result<(), ModelError> {
        for field in ["model_id", "version", "content_hash"] {
            let value = match field {
                "model_id" => &self.model_id,
                "version" => &self.version,
                _ => &self.content_hash,
            };
            if value.trim().is_empty() {
                return Err(ModelError::EmptyField { structure: "GridModelRef", field });
            }
        }
        Ok(())
    }
}

/// A bus of the network model. Voltage base: `nominal_kv` is the bus
/// nominal voltage in kV (report §10.2 — kV at the nominal base; per-unit
/// values are relative to this base in the solver representation).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BusSpec {
    /// Bus identifier, unique in the model.
    pub bus_id: String,
    /// Nominal voltage in kV (strictly positive).
    pub nominal_kv: f64,
    /// Whether this bus hosts the slack / external-grid source.
    pub is_slack: bool,
}

/// A distribution line / cable (branch). Parameters are positive-sequence,
/// physical units, as declared by the model artefact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BranchSpec {
    /// Branch identifier, unique in the model.
    pub branch_id: String,
    /// From-bus identifier (must exist).
    pub from_bus: String,
    /// To-bus identifier (must exist).
    pub to_bus: String,
    /// Positive-sequence resistance in ohm (>= 0).
    pub r_ohm: f64,
    /// Positive-sequence reactance in ohm (> 0 for a real branch).
    pub x_ohm: f64,
    /// Rated current in A (strictly positive) — the thermal-rating base
    /// for the branch loading fraction.
    pub rated_current_a: f64,
}

/// A two-winding transformer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransformerSpec {
    /// Transformer identifier, unique in the model.
    pub transformer_id: String,
    /// High-voltage bus identifier (must exist).
    pub hv_bus: String,
    /// Low-voltage bus identifier (must exist).
    pub lv_bus: String,
    /// Rated apparent power in MVA (strictly positive) — the rating base
    /// for the transformer loading fraction.
    pub rated_mva: f64,
    /// Short-circuit voltage `vk` in percent (strictly positive).
    pub vk_percent: f64,
    /// Real part of the short-circuit voltage `vk_r` in percent (>= 0).
    pub vk_r_percent: f64,
}

/// The external grid / slack source at one slack bus.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExternalGridSpec {
    /// The slack bus identifier (must exist and be marked slack).
    pub bus_id: String,
    /// Slack-bus voltage magnitude in per-unit of the bus nominal voltage.
    pub vm_pu: f64,
    /// Slack-bus voltage angle in degrees.
    pub va_degree: f64,
}

/// The provider-neutral, versioned network model (ledger P4.02 step 1).
///
/// This is the IR the feasibility gate assembles from the P4.03
/// versioned-model store; adapters translate it into their provider's
/// model via [`crate::translation`]. Validation is structural only
/// (identifiers, references, positive parameters); electrical plausibility
/// is the model artefact's and P4.03 health check's business.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VersionedNetworkModel {
    /// Versioned model identity, carried onto every result.
    pub model_ref: GridModelRef,
    /// System base apparent power in MVA (strictly positive) — the base
    /// for per-unit power and current representations.
    pub base_mva: f64,
    /// Buses.
    pub buses: Vec<BusSpec>,
    /// Branches (lines/cables).
    pub branches: Vec<BranchSpec>,
    /// Two-winding transformers.
    pub transformers: Vec<TransformerSpec>,
    /// External grid / slack sources (exactly one, at a slack bus).
    pub external_grids: Vec<ExternalGridSpec>,
}

impl VersionedNetworkModel {
    /// Structural validation: unique identifiers, existing bus references,
    /// positive parameters, exactly one slack source at a slack bus.
    pub fn validate(&self) -> Result<(), ModelError> {
        self.model_ref.validate()?;
        if !self.base_mva.is_finite() || self.base_mva <= 0.0 {
            return Err(ModelError::InvalidNumber {
                structure: "VersionedNetworkModel",
                field: "base_mva",
            });
        }
        if self.buses.is_empty() {
            return Err(ModelError::EmptyField {
                structure: "VersionedNetworkModel",
                field: "buses",
            });
        }
        let mut bus_ids = Vec::with_capacity(self.buses.len());
        for bus in &self.buses {
            if bus.bus_id.trim().is_empty() {
                return Err(ModelError::EmptyField { structure: "BusSpec", field: "bus_id" });
            }
            if !bus.nominal_kv.is_finite() || bus.nominal_kv <= 0.0 {
                return Err(ModelError::InvalidNumber {
                    structure: "BusSpec",
                    field: "nominal_kv",
                });
            }
            if bus_ids.contains(&bus.bus_id) {
                return Err(ModelError::DuplicateId {
                    structure: "bus",
                    id: bus.bus_id.clone(),
                });
            }
            bus_ids.push(bus.bus_id.clone());
        }
        let has_bus = |id: &str| bus_ids.iter().any(|b| b == id);

        let mut seen_branches = Vec::new();
        for branch in &self.branches {
            if branch.branch_id.trim().is_empty() {
                return Err(ModelError::EmptyField {
                    structure: "BranchSpec",
                    field: "branch_id",
                });
            }
            if seen_branches.contains(&branch.branch_id) {
                return Err(ModelError::DuplicateId {
                    structure: "branch",
                    id: branch.branch_id.clone(),
                });
            }
            seen_branches.push(branch.branch_id.clone());
            if !has_bus(&branch.from_bus) {
                return Err(ModelError::UnknownBus {
                    structure: "BranchSpec",
                    bus_id: branch.from_bus.clone(),
                });
            }
            if !has_bus(&branch.to_bus) {
                return Err(ModelError::UnknownBus {
                    structure: "BranchSpec",
                    bus_id: branch.to_bus.clone(),
                });
            }
            if !branch.r_ohm.is_finite() || branch.r_ohm < 0.0 {
                return Err(ModelError::InvalidNumber {
                    structure: "BranchSpec",
                    field: "r_ohm",
                });
            }
            if !branch.x_ohm.is_finite() || branch.x_ohm <= 0.0 {
                return Err(ModelError::InvalidNumber {
                    structure: "BranchSpec",
                    field: "x_ohm",
                });
            }
            if !branch.rated_current_a.is_finite() || branch.rated_current_a <= 0.0 {
                return Err(ModelError::InvalidNumber {
                    structure: "BranchSpec",
                    field: "rated_current_a",
                });
            }
        }

        let mut seen_trafos = Vec::new();
        for trafo in &self.transformers {
            if trafo.transformer_id.trim().is_empty() {
                return Err(ModelError::EmptyField {
                    structure: "TransformerSpec",
                    field: "transformer_id",
                });
            }
            if seen_trafos.contains(&trafo.transformer_id) {
                return Err(ModelError::DuplicateId {
                    structure: "transformer",
                    id: trafo.transformer_id.clone(),
                });
            }
            seen_trafos.push(trafo.transformer_id.clone());
            if !has_bus(&trafo.hv_bus) {
                return Err(ModelError::UnknownBus {
                    structure: "TransformerSpec",
                    bus_id: trafo.hv_bus.clone(),
                });
            }
            if !has_bus(&trafo.lv_bus) {
                return Err(ModelError::UnknownBus {
                    structure: "TransformerSpec",
                    bus_id: trafo.lv_bus.clone(),
                });
            }
            if !trafo.rated_mva.is_finite() || trafo.rated_mva <= 0.0 {
                return Err(ModelError::InvalidNumber {
                    structure: "TransformerSpec",
                    field: "rated_mva",
                });
            }
            if !trafo.vk_percent.is_finite() || trafo.vk_percent <= 0.0 {
                return Err(ModelError::InvalidNumber {
                    structure: "TransformerSpec",
                    field: "vk_percent",
                });
            }
            if !trafo.vk_r_percent.is_finite() || trafo.vk_r_percent < 0.0 {
                return Err(ModelError::InvalidNumber {
                    structure: "TransformerSpec",
                    field: "vk_r_percent",
                });
            }
        }

        if self.external_grids.len() != 1 {
            return Err(ModelError::InvalidSlack {
                detail: format!(
                    "exactly one external grid is required, found {}",
                    self.external_grids.len()
                ),
            });
        }
        let grid = &self.external_grids[0];
        if !has_bus(&grid.bus_id) {
            return Err(ModelError::UnknownBus {
                structure: "ExternalGridSpec",
                bus_id: grid.bus_id.clone(),
            });
        }
        let bus_is_slack = self
            .buses
            .iter()
            .find(|b| b.bus_id == grid.bus_id)
            .is_some_and(|b| b.is_slack);
        if !bus_is_slack {
            return Err(ModelError::InvalidSlack {
                detail: format!(
                    "external grid bus '{}' is not marked as the slack bus",
                    grid.bus_id
                ),
            });
        }
        if !grid.vm_pu.is_finite() || grid.vm_pu <= 0.0 {
            return Err(ModelError::InvalidNumber {
                structure: "ExternalGridSpec",
                field: "vm_pu",
            });
        }
        if !grid.va_degree.is_finite() {
            return Err(ModelError::InvalidNumber {
                structure: "ExternalGridSpec",
                field: "va_degree",
            });
        }
        Ok(())
    }

    /// The set of equipment kinds present in this model (for the adapter
    /// scope gate).
    pub fn equipment_kinds(&self) -> Vec<EquipmentKind> {
        let mut kinds = Vec::new();
        if !self.branches.is_empty() {
            kinds.push(EquipmentKind::Line);
        }
        if !self.transformers.is_empty() {
            kinds.push(EquipmentKind::TwoWindingTransformer);
        }
        if !self.external_grids.is_empty() {
            kinds.push(EquipmentKind::ExternalGrid);
        }
        kinds
    }

    /// The bus with the given id, if present.
    pub fn bus(&self, bus_id: &str) -> Option<&BusSpec> {
        self.buses.iter().find(|b| b.bus_id == bus_id)
    }
}

// ---------------------------------------------------------------------------
// Candidate injections
// ---------------------------------------------------------------------------

/// One candidate injection at one bus, in canonical units and sign
/// (report §10.2; ledger P4.02 step 2: "apply candidate injections/
/// controls").
///
/// - `active_power`: MW, `P > 0` = **export** at the point of connection
///   (frozen rule; [`ActivePowerMW`]).
/// - `reactive_power`: MVAr, sign interpreted ONLY under
///   `reactive_sign_convention` — the convention declared by the governing
///   grid model and device profile. The declared convention travels WITH
///   the candidate; it is never inferred from the sign alone (§10.2;
///   foundations §6.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateInjection {
    /// Candidate injection identifier (unique within one request).
    pub injection_id: String,
    /// The bus the injection is applied at (must exist in the model).
    pub bus_id: String,
    /// Active power in MW (`P > 0` = export at the PCC).
    pub active_power: ActivePowerMW,
    /// Reactive power in MVAr (sign per the declared convention).
    pub reactive_power: ReactivePowerMVAr,
    /// The reactive-power sign convention declared by the governing grid
    /// model and device profile (required — there is no default, §10.2).
    pub reactive_sign_convention: ReactiveSignConvention,
    /// Start of the schedule interval this injection belongs to, when the
    /// candidate is time-indexed (§10.3). `None` = a snapshot candidate.
    #[serde(default)]
    pub interval_start: Option<time::OffsetDateTime>,
}

impl CandidateInjection {
    /// Structural validation: non-empty identifier, referenced bus exists.
    /// Unit and sign validity are guaranteed by the frozen contract types.
    pub fn validate(&self, model: &VersionedNetworkModel) -> Result<(), ModelError> {
        if self.injection_id.trim().is_empty() {
            return Err(ModelError::EmptyField {
                structure: "CandidateInjection",
                field: "injection_id",
            });
        }
        if model.bus(&self.bus_id).is_none() {
            return Err(ModelError::UnknownBus {
                structure: "CandidateInjection",
                bus_id: self.bus_id.clone(),
            });
        }
        Ok(())
    }
}

/// One applied-injection record: exactly which bus received which
/// candidate, in canonical units (ledger P4.05 step 3 consumes this record;
/// the port records it so every run is reproducible from the record).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppliedInjectionRecord {
    /// The candidate injection identifier.
    pub injection_id: String,
    /// The bus the injection was applied at.
    pub bus_id: String,
    /// Active power in MW, canonical sign (`> 0` = export).
    pub active_power_mw: f64,
    /// Reactive power in MVAr, canonical (declared-convention) sign.
    pub reactive_power_mvar: f64,
    /// The declared reactive sign convention applied for this candidate.
    pub reactive_sign_convention: ReactiveSignConvention,
    /// The interval start, when the candidate was time-indexed.
    #[serde(default)]
    pub interval_start: Option<time::OffsetDateTime>,
}

// ---------------------------------------------------------------------------
// Network limits and the limit verdict (§12.5 steps 5, 7)
// ---------------------------------------------------------------------------

/// Network limits for one dispatch evaluation (§12.5 step 5: "verify
/// voltage, current/thermal, transformer, generator, and relevant equipment
/// limits"). Limit configuration sources are a P4.06 decision; this type
/// carries the configured values only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NetworkLimits {
    /// Lower voltage limit in per-unit of each bus nominal voltage.
    pub voltage_min_pu: f64,
    /// Upper voltage limit in per-unit of each bus nominal voltage.
    pub voltage_max_pu: f64,
    /// Maximum branch loading as a fraction of rated current, in (0, 1].
    pub max_branch_loading_fraction: f64,
    /// Maximum transformer loading as a fraction of rated MVA, in (0, 1].
    pub max_transformer_loading_fraction: f64,
}

impl NetworkLimits {
    /// Validates the limit configuration: ordered voltage band inside
    /// (0, ∞), loading fractions in (0, 1].
    pub fn validate(&self) -> Result<(), ModelError> {
        for (field, value) in [
            ("voltage_min_pu", self.voltage_min_pu),
            ("voltage_max_pu", self.voltage_max_pu),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(ModelError::InvalidNumber {
                    structure: "NetworkLimits",
                    field,
                });
            }
        }
        if self.voltage_min_pu >= self.voltage_max_pu {
            return Err(ModelError::InvalidNumber {
                structure: "NetworkLimits",
                field: "voltage_min_pu",
            });
        }
        for field in ["max_branch_loading_fraction", "max_transformer_loading_fraction"] {
            let value = match field {
                "max_branch_loading_fraction" => self.max_branch_loading_fraction,
                _ => self.max_transformer_loading_fraction,
            };
            if !value.is_finite() || value <= 0.0 || value > 1.0 {
                return Err(ModelError::InvalidNumber {
                    structure: "NetworkLimits",
                    field,
                });
            }
        }
        Ok(())
    }
}

/// The kind of one limit violation (§12.5 step 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitViolationKind {
    /// Bus voltage below the configured lower band.
    VoltageUnder,
    /// Bus voltage above the configured upper band.
    VoltageOver,
    /// Branch loading above the configured thermal limit.
    BranchOverload,
    /// Transformer loading above the configured limit.
    TransformerOverload,
}

/// One limit violation with its value, its limit, and the element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LimitViolation {
    /// The violation kind.
    pub kind: LimitViolationKind,
    /// The violated element (bus / branch / transformer id).
    pub element_id: String,
    /// The evaluated value (pu for voltage; fraction for loadings).
    pub value: f64,
    /// The configured limit that was violated.
    pub limit: f64,
    /// Recorded detail (which limit configuration, which side).
    pub detail: String,
}

/// The network-limit verdict for one candidate (§12.5 steps 5 and 7).
///
/// This verdict is the LIMIT CHECK ONLY — the reject / constrain / return
/// policy of §12.5 step 8 is a P4.07 decision and lives in the gate, not
/// here. Fail-closed: a non-converged or errored run is
/// [`FeasibilityVerdict::Inconclusive`] — never a pass, never a rejection
/// that implies the network state was evaluated (§17.5 invariant 9).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "verdict")]
pub enum FeasibilityVerdict {
    /// The converged evaluation found no limit violation.
    WithinLimits,
    /// The converged evaluation found the listed violations.
    Violations { violations: Vec<LimitViolation> },
    /// The evaluation produced no verdict (non-convergence, provider
    /// error, incomplete result). Fail-closed: never interpreted as a pass.
    Inconclusive { reason: String },
}

/// Evaluates the configured limits against one power-flow result
/// (§12.5 steps 5 and 7). Provider-independent by construction: the SAME
/// result yields the SAME verdict regardless of which adapter produced it
/// (the P4.02 contract test proves this through two adapters).
pub fn evaluate_limits(
    result: &PowerFlowResult,
    limits: &NetworkLimits,
) -> FeasibilityVerdict {
    if let Err(error) = limits.validate() {
        return FeasibilityVerdict::Inconclusive {
            reason: format!("invalid limit configuration: {error}"),
        };
    }
    match result.status {
        PowerFlowStatus::DidNotConverge => {
            return FeasibilityVerdict::Inconclusive {
                reason: "power flow did not converge; no limit verdict is possible \
                         (non-convergence is a result, never a pass — §12.5 step 6)"
                    .to_string(),
            };
        }
        PowerFlowStatus::ProviderReportedError => {
            return FeasibilityVerdict::Inconclusive {
                reason: "the provider reported an evaluation error; no limit verdict \
                         is possible (fail-closed, §17.5 invariant 9)"
                    .to_string(),
            };
        }
        PowerFlowStatus::Converged => {}
    }
    if result.validate_complete().is_err() {
        return FeasibilityVerdict::Inconclusive {
            reason: "the power-flow result is incomplete (provider bug); \
                     no limit verdict is published (fail-closed)"
                .to_string(),
        };
    }

    let mut violations = Vec::new();
    for voltage in &result.bus_voltages {
        if voltage.vm_pu < limits.voltage_min_pu {
            violations.push(LimitViolation {
                kind: LimitViolationKind::VoltageUnder,
                element_id: voltage.bus_id.clone(),
                value: voltage.vm_pu,
                limit: limits.voltage_min_pu,
                detail: "bus voltage below the configured lower band".to_string(),
            });
        }
        if voltage.vm_pu > limits.voltage_max_pu {
            violations.push(LimitViolation {
                kind: LimitViolationKind::VoltageOver,
                element_id: voltage.bus_id.clone(),
                value: voltage.vm_pu,
                limit: limits.voltage_max_pu,
                detail: "bus voltage above the configured upper band".to_string(),
            });
        }
    }
    for branch in &result.branch_loadings {
        if branch.loading_fraction > limits.max_branch_loading_fraction {
            violations.push(LimitViolation {
                kind: LimitViolationKind::BranchOverload,
                element_id: branch.branch_id.clone(),
                value: branch.loading_fraction,
                limit: limits.max_branch_loading_fraction,
                detail: "branch loading above the configured thermal limit".to_string(),
            });
        }
    }
    for trafo in &result.transformer_loadings {
        if trafo.loading_fraction > limits.max_transformer_loading_fraction {
            violations.push(LimitViolation {
                kind: LimitViolationKind::TransformerOverload,
                element_id: trafo.transformer_id.clone(),
                value: trafo.loading_fraction,
                limit: limits.max_transformer_loading_fraction,
                detail: "transformer loading above the configured limit".to_string(),
            });
        }
    }

    if violations.is_empty() {
        FeasibilityVerdict::WithinLimits
    } else {
        FeasibilityVerdict::Violations { violations }
    }
}

// ---------------------------------------------------------------------------
// Power-flow results
// ---------------------------------------------------------------------------

/// Power-flow outcome status — a closed vocabulary (§12.5 steps 4 and 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerFlowStatus {
    /// The power flow converged; bus/branch/transformer results are the
    /// evaluated network state.
    Converged,
    /// The power flow did NOT converge. This is a RESULT, reported with
    /// diagnostics — never silently retried with altered settings, and
    /// never treated as feasible (§12.5 step 6; ledger P4.05 step 3).
    DidNotConverge,
    /// The provider ran but reported an evaluation error on the given
    /// model/candidate. Fail-closed.
    ProviderReportedError,
}

impl PowerFlowStatus {
    /// The exact canonical status string.
    pub fn as_str(self) -> &'static str {
        match self {
            PowerFlowStatus::Converged => "converged",
            PowerFlowStatus::DidNotConverge => "did_not_converge",
            PowerFlowStatus::ProviderReportedError => "provider_reported_error",
        }
    }
}

/// A diagnostic block: available with its payload, or explicitly not
/// available with a recorded reason. A `NotAvailable` with an empty reason
/// is incomplete and fails [`PowerFlowResult::validate_complete`] —
/// silence is never legal (the §12.6 evidence-discipline pattern, ledger
/// phase 3 `21_result_contract.md`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Diagnostic<T> {
    /// The diagnostic is available.
    Available(T),
    /// The diagnostic is not available for this run; the reason is recorded.
    NotAvailable { reason: String },
}

impl<T> Diagnostic<T> {
    /// `true` when [`Diagnostic::Available`].
    pub fn is_available(&self) -> bool {
        matches!(self, Diagnostic::Available(_))
    }

    /// The recorded not-available reason, when not available.
    pub fn not_available_reason(&self) -> Option<&str> {
        match self {
            Diagnostic::Available(_) => None,
            Diagnostic::NotAvailable { reason } => Some(reason.as_str()),
        }
    }
}

/// Bus voltage in BOTH canonical representations (report §10.2): kV at the
/// nominal base and per-unit in the solver representation. The per-unit
/// base is the bus nominal voltage (`nominal_kv` of the bus spec); the
/// translation layer (`crate::translation`) guarantees
/// `vm_kv == vm_pu * nominal_kv` within float tolerance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BusVoltage {
    /// Bus identifier.
    pub bus_id: String,
    /// Voltage magnitude in per-unit of the bus nominal voltage.
    pub vm_pu: f64,
    /// Voltage magnitude in kV at the nominal base.
    pub vm_kv: f64,
}

/// Branch current and loading. Current is carried in A; the loading is a
/// fraction of the branch's rated current (the explicit base declared in
/// the model spec — report §10.2: "current is A or per-unit on an explicit
/// base"). Loading percent is a display conversion and never persisted as
/// canonical.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BranchLoading {
    /// Branch identifier.
    pub branch_id: String,
    /// Current at the from-bus end in A.
    pub current_from_a: f64,
    /// Current at the to-bus end in A.
    pub current_to_a: f64,
    /// Loading as a fraction of `rated_current_a` (the greater of the two
    /// ends, per the provider's loading definition).
    pub loading_fraction: f64,
}

/// Transformer loading as a fraction of the transformer's rated MVA (the
/// explicit rating base declared in the model spec), plus the side
/// currents in A.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransformerLoading {
    /// Transformer identifier.
    pub transformer_id: String,
    /// Loading as a fraction of `rated_mva`.
    pub loading_fraction: f64,
    /// High-voltage side current in A.
    pub hv_current_a: f64,
    /// Low-voltage side current in A.
    pub lv_current_a: f64,
}

/// Residuals and effort of one power-flow run (§12.5 step 7: "compute
/// margin/diagnostics"; §9.2 evidence discipline). Every field is optional
/// WITH a recorded reason when the provider does not expose it — see
/// [`crate::adapter`] for the recorded per-provider availability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PowerFlowResiduals {
    /// Solver iteration count, when reported.
    pub iterations: Option<u64>,
    /// Maximum bus-power mismatch in MW, when reported.
    pub max_power_mismatch_mw: Option<f64>,
    /// Maximum voltage-magnitude update in per-unit, when reported.
    pub max_voltage_update_pu: Option<f64>,
    /// Verbatim provider status text, for audit.
    pub provider_status_text: String,
}

/// One model-coverage limitation carried on every result (§12.5 step 6:
/// "report non-convergence and model coverage limitations explicitly").
/// Sourced from the adapter's declared scope — never invented per run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageLimitation {
    /// Short stable label (from the scope's `not_modelled` entry).
    pub label: String,
    /// The recorded limitation statement.
    pub statement: String,
}

/// The complete power-flow result record (ledger P4.02 step 1: bus
/// voltages, branch currents/loadings, transformer loadings, convergence
/// status, residuals, model-coverage limitations).
///
/// Provenance is mandatory: provider identification (including the PINNED
/// provider version — P4.01: the pin is recorded in EVERY feasibility
/// result path) and the versioned model reference travel on every result.
/// [`PowerFlowResult::validate_complete`] enforces completeness so P4.05's
/// run-record wiring builds on a contract that cannot be satisfied by a
/// half-filled record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PowerFlowResult {
    /// Convergence status (closed vocabulary).
    pub status: PowerFlowStatus,
    /// The versioned model this run evaluated.
    pub model_ref: GridModelRef,
    /// Provider identifier.
    pub provider_id: String,
    /// Pinned provider version — a REQUIRED field (P4.01: the provider pin
    /// is recorded on every feasibility result path).
    pub provider_version: String,
    /// Bus voltages (both canonical representations).
    pub bus_voltages: Vec<BusVoltage>,
    /// Branch currents and loadings.
    pub branch_loadings: Vec<BranchLoading>,
    /// Transformer loadings.
    pub transformer_loadings: Vec<TransformerLoading>,
    /// The injections applied for this run, recorded per element.
    pub applied_injections: Vec<AppliedInjectionRecord>,
    /// Residuals and effort (available, or not available WITH a reason).
    pub residuals: Diagnostic<PowerFlowResiduals>,
    /// Model-coverage limitations (mandatory, non-empty — sourced from the
    /// adapter's declared scope, §12.5 step 6).
    pub coverage_limitations: Vec<CoverageLimitation>,
    /// Warnings emitted during translation, evaluation, or mapping.
    pub warnings: Vec<String>,
    /// Wall-clock runtime of the evaluation.
    pub runtime: Duration,
}

/// Completeness violations of the power-flow result contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultCompletenessError {
    /// Provider identification missing or empty.
    EmptyIdentification { field: &'static str },
    /// The model reference is missing or empty.
    InvalidModelRef,
    /// Coverage limitations missing — §12.5 step 6 makes them mandatory on
    /// every result, including converged ones.
    MissingCoverageLimitations,
    /// A converged result must carry bus voltages.
    MissingBusVoltages,
    /// A converged result must carry the residuals diagnostic as
    /// available.
    MissingResiduals,
    /// A diagnostic is `NotAvailable` with an empty reason.
    EmptyNotAvailableReason { diagnostic: &'static str },
    /// A non-finite value in a numeric field (field names the location).
    NonFiniteValue { field: String },
}

impl std::fmt::Display for ResultCompletenessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResultCompletenessError::EmptyIdentification { field } => {
                write!(f, "result identification field '{field}' is empty")
            }
            ResultCompletenessError::InvalidModelRef => {
                write!(f, "result carries an invalid model reference")
            }
            ResultCompletenessError::MissingCoverageLimitations => {
                write!(f, "result carries no model-coverage limitations (§12.5 step 6)")
            }
            ResultCompletenessError::MissingBusVoltages => {
                write!(f, "a converged result must carry bus voltages")
            }
            ResultCompletenessError::MissingResiduals => {
                write!(f, "a converged result must carry the residuals diagnostic")
            }
            ResultCompletenessError::EmptyNotAvailableReason { diagnostic } => {
                write!(
                    f,
                    "diagnostic '{diagnostic}' is not available but no reason is recorded"
                )
            }
            ResultCompletenessError::NonFiniteValue { field } => {
                write!(f, "numeric field '{field}' is not finite")
            }
        }
    }
}

impl std::error::Error for ResultCompletenessError {}

fn require_finite(values: &[(String, f64)]) -> Result<(), ResultCompletenessError> {
    for (field, value) in values {
        if !value.is_finite() {
            return Err(ResultCompletenessError::NonFiniteValue {
                field: field.clone(),
            });
        }
    }
    Ok(())
}

impl PowerFlowResult {
    /// Validates result completeness: provider identification (including
    /// the required pinned provider version), model reference, mandatory
    /// coverage limitations, per-status requirements, and no silent
    /// not-available diagnostics.
    pub fn validate_complete(&self) -> Result<(), ResultCompletenessError> {
        for (field, value) in [
            ("provider_id", self.provider_id.as_str()),
            ("provider_version", self.provider_version.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(ResultCompletenessError::EmptyIdentification { field });
            }
        }
        if self.model_ref.validate().is_err() {
            return Err(ResultCompletenessError::InvalidModelRef);
        }
        if self.coverage_limitations.is_empty() {
            return Err(ResultCompletenessError::MissingCoverageLimitations);
        }
        if let Some(reason) = self.residuals.not_available_reason() {
            if reason.trim().is_empty() {
                return Err(ResultCompletenessError::EmptyNotAvailableReason {
                    diagnostic: "residuals",
                });
            }
        }

        let mut finite_checks: Vec<(String, f64)> = Vec::new();
        for voltage in &self.bus_voltages {
            finite_checks.push((format!("bus_voltage.{}.vm_pu", voltage.bus_id), voltage.vm_pu));
            finite_checks.push((format!("bus_voltage.{}.vm_kv", voltage.bus_id), voltage.vm_kv));
        }
        for branch in &self.branch_loadings {
            finite_checks.push((
                format!("branch_loading.{}.current_from_a", branch.branch_id),
                branch.current_from_a,
            ));
            finite_checks.push((
                format!("branch_loading.{}.current_to_a", branch.branch_id),
                branch.current_to_a,
            ));
            finite_checks.push((
                format!("branch_loading.{}.loading_fraction", branch.branch_id),
                branch.loading_fraction,
            ));
        }
        for trafo in &self.transformer_loadings {
            finite_checks.push((
                format!("transformer_loading.{}.loading_fraction", trafo.transformer_id),
                trafo.loading_fraction,
            ));
            finite_checks.push((
                format!("transformer_loading.{}.hv_current_a", trafo.transformer_id),
                trafo.hv_current_a,
            ));
            finite_checks.push((
                format!("transformer_loading.{}.lv_current_a", trafo.transformer_id),
                trafo.lv_current_a,
            ));
        }
        require_finite(&finite_checks)?;

        if self.status == PowerFlowStatus::Converged {
            if self.bus_voltages.is_empty() {
                return Err(ResultCompletenessError::MissingBusVoltages);
            }
            if !self.residuals.is_available() {
                return Err(ResultCompletenessError::MissingResiduals);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Provider capabilities and the port trait
// ---------------------------------------------------------------------------

/// The full provider capability declaration (§9.1 pattern for
/// `GridModelProvider`): provider identity, the PINNED provider version
/// (P4.01), and the mandatory scope declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridProviderCapabilities {
    /// Provider identifier.
    pub provider_id: String,
    /// Pinned provider version (REQUIRED — P4.01: recorded on every
    /// result; the pin discipline of decision note §2).
    pub provider_version: String,
    /// The mandatory declared model scope (ledger P4.02 step 3). There is
    /// no capability statement without a scope.
    pub scope: ScopeDeclaration,
}

/// Errors the port itself reports (as opposed to power-flow outcomes,
/// which are returned in-band as a [`PowerFlowResult`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GridPortError {
    /// The network model failed structural validation. Caller input error.
    MalformedModel(ModelError),
    /// A candidate injection failed validation. Caller input error.
    InvalidCandidate(ModelError),
    /// The request exceeds the adapter's declared scope; the adapter
    /// refuses instead of approximating (ledger P4.02 step 3).
    ScopeExceeded(ScopeError),
    /// The backend seam failed at the transport/contract level before or
    /// during a run (worker unreachable, or a payload violating its
    /// contract). Infrastructure failure — not a power-flow outcome.
    BackendUnavailable(String),
    /// The backend returned a payload violating the worker contract (e.g.
    /// a reported provider version disagreeing with the pin).
    BackendContractViolation(String),
    /// Provider-internal guard: a provider produced a result failing
    /// completeness validation. A provider bug, not a power-flow outcome.
    IncompleteResult(ResultCompletenessError),
}

impl std::fmt::Display for GridPortError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GridPortError::MalformedModel(error) => write!(f, "malformed network model: {error}"),
            GridPortError::InvalidCandidate(error) => {
                write!(f, "invalid candidate injection: {error}")
            }
            GridPortError::ScopeExceeded(error) => write!(f, "request exceeds declared scope: {error}"),
            GridPortError::BackendUnavailable(message) => {
                write!(f, "grid backend unavailable: {message}")
            }
            GridPortError::BackendContractViolation(message) => {
                write!(f, "grid backend contract violation: {message}")
            }
            GridPortError::IncompleteResult(error) => {
                write!(f, "provider produced an incomplete result (provider bug): {error}")
            }
        }
    }
}

impl std::error::Error for GridPortError {}

/// The `GridModelProvider` port (report §9.1): the contract the Phase 4
/// feasibility gate depends on and grid-model providers implement.
///
/// Implementations must be object-safe (`Arc<dyn GridModelProvider>` in
/// composition roots) and sync — no port names an async runtime (the
/// flexcore-ports rule, applied as in flexcore-optimization-api).
///
/// Power-flow outcomes — including non-convergence and provider-reported
/// errors — are returned in-band as a [`PowerFlowResult`];
/// [`GridPortError`] is reserved for malformed input, scope refusals, and
/// backend infrastructure/contract failures.
pub trait GridModelProvider: Send + Sync {
    /// Declares what this provider supports: identity, pinned version, and
    /// the mandatory scope declaration (ledger P4.02 step 3).
    fn capabilities(&self) -> GridProviderCapabilities;

    /// Applies the candidate injections to the versioned network model and
    /// runs the provider's power-flow evaluation (§9.1 `run_power_flow`;
    /// ledger P4.02 step 1). An AC power-flow evaluation of specified
    /// injections/controls — NOT an AC-OPF optimization.
    fn run_power_flow(
        &self,
        model: &VersionedNetworkModel,
        injections: &[CandidateInjection],
    ) -> Result<PowerFlowResult, GridPortError>;

    /// Convenience §9.1-shaped dispatch evaluation:
    /// `validate_dispatch(model, injections, limits) -> verdict`. The
    /// default implementation is `run_power_flow` followed by the
    /// port-level [`evaluate_limits`] — providers override only with good
    /// cause, because the verdict MUST be a pure function of the result
    /// and the limits (the P4.02 contract test pins this).
    fn evaluate_dispatch(
        &self,
        model: &VersionedNetworkModel,
        injections: &[CandidateInjection],
        limits: &NetworkLimits,
    ) -> Result<FeasibilityVerdict, GridPortError> {
        let result = self.run_power_flow(model, injections)?;
        Ok(evaluate_limits(&result, limits))
    }
}

/// Object-safety and pure-function guarantees, checked in-crate.
#[cfg(test)]
mod tests {
    use super::*;

    /// The port is held as a trait object in composition roots; object
    /// safety is part of the contract.
    #[test]
    fn grid_model_provider_trait_is_object_safe() {
        fn takes_dyn(_: &dyn GridModelProvider) {}
        let _ = takes_dyn;
    }

    #[test]
    fn power_flow_status_vocabulary_is_closed_and_stable() {
        let all = [
            (PowerFlowStatus::Converged, "converged"),
            (PowerFlowStatus::DidNotConverge, "did_not_converge"),
            (
                PowerFlowStatus::ProviderReportedError,
                "provider_reported_error",
            ),
        ];
        for (status, text) in all {
            assert_eq!(status.as_str(), text);
            let json = serde_json::to_string(&status).expect("serializes");
            assert_eq!(json, format!("\"{text}\""));
            let back: PowerFlowStatus = serde_json::from_str(&json).expect("deserializes");
            assert_eq!(back, status);
        }
        // Exhaustiveness proves there is no fourth status to sneak past the
        // fail-closed rules.
        let exhaustive = match PowerFlowStatus::Converged {
            PowerFlowStatus::Converged
            | PowerFlowStatus::DidNotConverge
            | PowerFlowStatus::ProviderReportedError => "closed",
        };
        assert_eq!(exhaustive, "closed");
    }

    #[test]
    fn evaluate_limits_is_fail_closed_on_non_convergence() {
        let result = PowerFlowResult {
            status: PowerFlowStatus::DidNotConverge,
            model_ref: GridModelRef {
                model_id: "m".to_string(),
                version: "1".to_string(),
                content_hash: "h".to_string(),
            },
            provider_id: "p".to_string(),
            provider_version: "1.0.0".to_string(),
            bus_voltages: Vec::new(),
            branch_loadings: Vec::new(),
            transformer_loadings: Vec::new(),
            applied_injections: Vec::new(),
            residuals: Diagnostic::NotAvailable {
                reason: "no converged solution; no residuals exist".to_string(),
            },
            coverage_limitations: vec![CoverageLimitation {
                label: "test".to_string(),
                statement: "test limitation".to_string(),
            }],
            warnings: Vec::new(),
            runtime: Duration::from_millis(1),
        };
        let limits = NetworkLimits {
            voltage_min_pu: 0.9,
            voltage_max_pu: 1.1,
            max_branch_loading_fraction: 1.0,
            max_transformer_loading_fraction: 1.0,
        };
        let verdict = evaluate_limits(&result, &limits);
        match verdict {
            FeasibilityVerdict::Inconclusive { reason } => {
                assert!(reason.contains("did not converge"));
            }
            other => panic!("non-convergence must be inconclusive, got {other:?}"),
        }
    }

    #[test]
    fn evaluate_limits_reports_voltage_and_loading_violations() {
        let result = PowerFlowResult {
            status: PowerFlowStatus::Converged,
            model_ref: GridModelRef {
                model_id: "m".to_string(),
                version: "1".to_string(),
                content_hash: "h".to_string(),
            },
            provider_id: "p".to_string(),
            provider_version: "1.0.0".to_string(),
            bus_voltages: vec![
                BusVoltage {
                    bus_id: "bus_a".to_string(),
                    vm_pu: 1.04,
                    vm_kv: 20.8,
                },
                BusVoltage {
                    bus_id: "bus_b".to_string(),
                    vm_pu: 0.88,
                    vm_kv: 17.6,
                },
            ],
            branch_loadings: vec![BranchLoading {
                branch_id: "line_1".to_string(),
                current_from_a: 500.0,
                current_to_a: 495.0,
                loading_fraction: 1.05,
            }],
            transformer_loadings: Vec::new(),
            applied_injections: Vec::new(),
            residuals: Diagnostic::Available(PowerFlowResiduals {
                iterations: Some(4),
                max_power_mismatch_mw: Some(1.0e-8),
                max_voltage_update_pu: Some(1.0e-9),
                provider_status_text: "test converged".to_string(),
            }),
            coverage_limitations: vec![CoverageLimitation {
                label: "test".to_string(),
                statement: "test limitation".to_string(),
            }],
            warnings: Vec::new(),
            runtime: Duration::from_millis(1),
        };
        let limits = NetworkLimits {
            voltage_min_pu: 0.9,
            voltage_max_pu: 1.1,
            max_branch_loading_fraction: 1.0,
            max_transformer_loading_fraction: 1.0,
        };
        match evaluate_limits(&result, &limits) {
            FeasibilityVerdict::Violations { violations } => {
                assert_eq!(violations.len(), 2);
                assert!(violations.iter().any(|v| v.element_id == "bus_b"
                    && v.kind == LimitViolationKind::VoltageUnder));
                assert!(violations.iter().any(|v| v.element_id == "line_1"
                    && v.kind == LimitViolationKind::BranchOverload));
            }
            other => panic!("expected violations, got {other:?}"),
        }
    }
}
