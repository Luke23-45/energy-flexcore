// P4.02 — The grid-model backend seam: the transport-neutral, provider-path
// contract between the Rust provider and the Python numerical worker
// (decision note
// `ledger/phase-04-grid-feasibility-dispatch/01_02_grid_provider.md`,
// D-P4.02-1 — the D-P3.17-1 pattern).
//
// Decision summary (full rationale in the decision note):
// - The AC power-flow evaluation executes in the **Python numerical
//   worker** (§7.2: Python is for power-system simulation adapters and
//   numerical work; it is never the command authority) via the pinned
//   provider version recorded in the decision note (pandapower 3.5.6,
//   RESEARCH.md §1, verified 2026-11-07). This module deliberately
//   contains **no** binding, no transport, and no provider-library type in
//   Rust: the worker's payload vocabulary below is worker-contract data,
//   not a Rust binding.
// - The worker adapter implementing [`GridModelBackend`] lands in a later
//   wave (the P3.19 precedent): an in-process bridge for tests, and the
//   §15.1 Protobuf/gRPC path for deployment once the Phase 7 protobuf
//   contracts exist. The trait is transport-neutral so the adapter choice
//   does not ripple into the port.
// - The provider (Rust side) owns scope gating, translation
//   (`crate::translation`), result construction, provenance, and the
//   completeness gate. The worker computes the physics and reports its
//   installed provider version; the Rust side pins and cross-checks it
//   (decision note §2).
//
// Payload vocabulary (VERIFY-BUILD — all items confirmed at worker-adapter
// build time against the pinned provider's documentation and installed
// version; see decision note §6):
// 1. Result-table fields: per-bus voltage magnitude in per-unit
//    (`res_bus.vm_pu`); per-branch currents in kA and loading in percent
//    (`res_line.i_from_ka`, `res_line.i_to_ka`, `res_line.loading_percent`);
//    per-transformer loading in percent and side currents in kA
//    (`res_trafo.loading_percent`, `res_trafo.i_hv_ka`, `res_trafo.i_lv_ka`).
//    The exact column names and semantics must be re-verified against the
//    pinned version's docs at adapter build time.
// 2. Convergence flag location and semantics (`net["converged"]` in the
//    pinned version) — including the provider's behaviour for failed runs:
//    result tables may be absent or NaN on non-convergence; the adapter
//    maps that to an in-band non-converged outcome, never to an error to
//    retry away.
// 3. Iteration count source (`net["_ppc"]["iterations"]` or the version's
//    equivalent) — VERIFY before claiming it is recorded.
// 4. Residual availability: the pinned provider does not expose a
//    bus-power-mismatch residual for converged runs by default; until a
//    documented source is verified, residuals beyond the iteration count
//    are `NotAvailable { reason }` on the port result (contract-legal).
// 5. Element sign conventions on the worker side: a static-generator
//    element's `p_mw` is positive for generation (injection into the
//    network); a load element's `p_mw` is positive for consumption. The
//    translation layer (`crate::translation`) owns the mapping from
//    FlexCore's canonical `P > 0` = export rule, in both directions.
// 6. The provider's three-phase/asymmetric calculation surface is OUT OF
//    SCOPE for the v0.3.0 balanced adapter (declared scope; unbalanced
//    enters from v0.4.0 per the recommended default — D3 OPEN).

use serde::{Deserialize, Serialize};

/// One bus result of the worker's power-flow run (worker-contract shape).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackendBusResult {
    /// Bus identifier (same identifier space as the model IR).
    pub bus_id: String,
    /// Voltage magnitude in per-unit of the bus nominal voltage.
    pub vm_pu: f64,
}

/// One branch result of the worker's power-flow run (worker-contract
/// shape; kA and percent are the worker's reporting units — the
/// translation layer converts to canonical A and fraction).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackendBranchResult {
    /// Branch identifier.
    pub branch_id: String,
    /// From-end current in kA.
    pub i_from_ka: f64,
    /// To-end current in kA.
    pub i_to_ka: f64,
    /// Loading in percent of the branch rating.
    pub loading_percent: f64,
}

/// One transformer result of the worker's power-flow run (worker-contract
/// shape).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackendTransformerResult {
    /// Transformer identifier.
    pub transformer_id: String,
    /// Loading in percent of the transformer rating.
    pub loading_percent: f64,
    /// High-voltage side current in kA.
    pub i_hv_ka: f64,
    /// Low-voltage side current in kA.
    pub i_lv_ka: f64,
}

/// One injection in the worker's element model, after the translation
/// layer mapped the canonical candidate onto it (VERIFY-BUILD item 5:
/// positive `p_injection_mw` / `q_injection_mvar` means injection into the
/// network at the bus — the worker adapter materialises this as the
/// provider's generation-type element; negative values become
/// consumption-type elements).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackendInjection {
    /// Candidate injection identifier (carried for the applied-record).
    pub injection_id: String,
    /// Bus identifier.
    pub bus_id: String,
    /// Active-power injection in MW (positive = into the network).
    pub p_injection_mw: f64,
    /// Reactive-power injection in MVAr (positive = into the network).
    pub q_injection_mvar: f64,
    /// Interval start carried for provenance, when the candidate was
    /// time-indexed (serialized as RFC 3339 in the worker payload).
    #[serde(default)]
    pub interval_start: Option<time::OffsetDateTime>,
}

/// Options for one power-flow run, translated from the evaluation request.
/// The worker maps each field onto the pinned provider's call parameters
/// (VERIFY-BUILD at adapter build time).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackendRunOptions {
    /// Evaluation label (for worker logs and audit).
    pub evaluation_label: String,
    /// Whether the worker must report its installed provider version in
    /// the outcome (always `true` in production paths — the pin
    /// cross-check is mandatory, decision note §2).
    pub report_provider_version: bool,
}

/// The raw worker outcome for one power-flow run, mapped by the provider
/// onto the port result. Vector lengths are contracts: one result row per
/// model element, matched by identifier — a missing or duplicate row is a
/// contract failure ([`BackendError::ContractViolation`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackendPowerFlowOutcome {
    /// Whether the power flow converged. Non-convergence is an in-band
    /// outcome (a result), never a backend error to retry away (§12.5).
    pub converged: bool,
    /// Whether the worker reports an evaluation error on the given
    /// model/candidates (in-band; fail-closed on the port side).
    pub reported_error: Option<String>,
    /// The worker's installed provider version, when
    /// [`BackendRunOptions::report_provider_version`] was set. The
    /// provider cross-checks this against the pin (decision note §2).
    pub provider_version_reported: Option<String>,
    /// Per-bus voltage results (one row per model bus).
    pub bus_results: Vec<BackendBusResult>,
    /// Per-branch results (one row per model branch; empty when the model
    /// has no branches).
    pub branch_results: Vec<BackendBranchResult>,
    /// Per-transformer results (one row per model transformer; empty when
    /// the model has no transformers).
    pub transformer_results: Vec<BackendTransformerResult>,
    /// Iteration count, when reported (VERIFY-BUILD item 3).
    pub iterations: Option<u64>,
    /// Verbatim worker status text (audit trail for every mapping).
    pub worker_status_text: String,
    /// Warnings the worker itself raised.
    pub warnings: Vec<String>,
}

/// Backend identity for records and diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendDescription {
    /// Backend name (e.g. the worker adapter's name).
    pub name: String,
    /// Backend version.
    pub version: String,
    /// Recorded notes (transport, pinned provider identity as recorded
    /// upstream, etc.).
    pub notes: Vec<String>,
}

/// Backend seam failure (transport or contract level).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    /// The backend is unreachable or failed before a run could be
    /// attempted.
    Unavailable(String),
    /// The backend returned a payload violating its contract (e.g. a
    /// missing result row, a duplicate, a non-finite value).
    ContractViolation(String),
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendError::Unavailable(message) => write!(f, "backend unavailable: {message}"),
            BackendError::ContractViolation(message) => {
                write!(f, "backend contract violation: {message}")
            }
        }
    }
}

impl std::error::Error for BackendError {}

/// The grid-model backend seam (decision note D-P4.02-1): the
/// transport-neutral contract the Python numerical worker adapter
/// implements in a later wave.
///
/// The seam consumes the provider-neutral model IR (`VersionedNetworkModel`
/// from [`crate::port`]) — the worker adapter translates it into the
/// provider's model inside the worker — and the already-translated
/// injections (`BackendInjection` from this module, produced by
/// `crate::translation`). The outcome comes back in the worker's reporting
/// units (per-unit, kA, percent); the translation layer converts to
/// canonical units on the Rust side.
///
/// Sync by the same rule as the `GridModelProvider` port: no async runtime
/// named here; the adapter decides how to bridge any real transport.
pub trait GridModelBackend: Send + Sync {
    /// Runs the power-flow evaluation for the given model, injections, and
    /// options.
    fn run_power_flow(
        &self,
        model: &crate::port::VersionedNetworkModel,
        injections: &[BackendInjection],
        options: &BackendRunOptions,
    ) -> Result<BackendPowerFlowOutcome, BackendError>;

    /// Backend identity for records and diagnostics.
    fn describe(&self) -> BackendDescription;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_trait_is_object_safe() {
        fn takes_dyn(_: &dyn GridModelBackend) {}
        let _ = takes_dyn;
    }

    #[test]
    fn backend_outcome_serializes_with_stable_names() {
        let outcome = BackendPowerFlowOutcome {
            converged: false,
            reported_error: None,
            provider_version_reported: Some("3.5.6".to_string()),
            bus_results: Vec::new(),
            branch_results: Vec::new(),
            transformer_results: Vec::new(),
            iterations: None,
            worker_status_text: "test".to_string(),
            warnings: Vec::new(),
        };
        let json = serde_json::to_string(&outcome).expect("serializes");
        assert!(json.contains("\"converged\":false"));
        assert!(json.contains("\"provider_version_reported\":\"3.5.6\""));
        let back: BackendPowerFlowOutcome = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(back, outcome);
    }

    #[test]
    fn backend_injection_round_trips_through_json() {
        let injection = BackendInjection {
            injection_id: "inj-1".to_string(),
            bus_id: "bus_a".to_string(),
            p_injection_mw: 2.5,
            q_injection_mvar: -1.25,
            interval_start: None,
        };
        let json = serde_json::to_string(&injection).expect("serializes");
        let back: BackendInjection = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(back, injection);
    }
}
