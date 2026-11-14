// P5.15 — The worker-process backend: the REAL `GridModelBackend`
//         implementation over the Python numerical worker leg
//         (`apps/worker/python/grid_worker.py`).
//
// This module closes the D-P4.02-1 "later wave" deliverable recorded in
// `backend.rs`: the worker adapter that transports the model IR and the
// translated injections to the Python numerical worker and transports the
// worker's result tables back. The transport is deliberately the simplest
// one that needs no new dependency in this CORE crate (the dependency
// policy admits flexcore-contracts + serde/serde_json/time only — no
// tokio, no gRPC client, no Python binding): ONE JSON document on the
// worker process's stdin, ONE JSON document on its stdout, spawned with
// `std::process::Command` and waited on with a bounded timeout
// (`std::thread` + `std::sync::mpsc::recv_timeout`; no async runtime).
// A gRPC/Protobuf transport remains the deployment-stage upgrade path
// recorded in `backend.rs` — the payload shapes below are unchanged by
// it.
//
// ## The worker contract
//
// Request (one JSON object on stdin):
//
//     {
//       "protocol": "flexcore-grid-worker/1",
//       "request": "run_power_flow",
//       "model":       <VersionedNetworkModel, serde form>,
//       "injections":  [ <BackendInjection, serde form> ],
//       "options":     <BackendRunOptions, serde form>
//     }
//
// Response (one JSON object on stdout):
//
//     <BackendPowerFlowOutcome, serde form>
//
// Both halves reuse the crate's existing serde shapes (the worker contract
// of `backend.rs`) — there is no second vocabulary to drift.
//
// ## Fail-closed discipline (P4.08; §12.5 closing rule)
//
// EVERY failure of this backend is a `BackendError` — spawn failure,
// non-zero exit, timeout, empty or unparseable output, a payload that
// violates the contract (missing/duplicate/extra result rows are the
// ADAPTER's row-by-row check; here: non-finite numbers, wrong protocol
// tag, a missing required field) — and every `BackendError` maps through
// `GridPortError::BackendUnavailable`/`BackendContractViolation` to the
// P4.08 `ProviderError` fail-closed trigger. There is NO path from a
// worker failure to a within-limits verdict, and no retry (the recorded
// NO_RETRY_POLICY, D-P4.05-2).
//
// ## Honesty note (P5.15, recorded)
//
// The Python worker this backend drives was WRITTEN, never executed
// (nothing installed, nothing run — the phase's loop bans). Its pandapower
// 3.5.6 API usage follows the provider's documented API as recorded in the
// VERIFY-BUILD items of `backend.rs` and is **UNVERIFIED** until the
// human's build/test pass executes it against the pinned version. This
// backend therefore does not claim any pandapower behaviour: it transports
// the contract and validates the RESULT side (finiteness, protocol tag,
// JSON shape) — the physics-side claims stay with the (unexecuted) worker.
//
// ## Solver-class discipline (absolute; AGENTS.md rule 4)
//
// What crosses this seam is an **AC power-flow evaluation of specified
// injections and controls**. It is NOT an AC-OPF optimizer; the
// feasibility gate never optimizes (§12.5). AC power flow ≠ AC-OPF; the
// terms are never interchanged. Non-convergence is an in-band RESULT (the
// outcome's `converged: false`), never a backend error and never retried.

use std::io::Write;
use std::sync::mpsc;
use std::time::Duration;

use serde::Deserialize;
use serde::Serialize;

use crate::backend::{
    BackendError, BackendInjection, BackendPowerFlowOutcome, BackendRunOptions, BackendDescription,
    GridModelBackend,
};
use crate::port::VersionedNetworkModel;

/// The protocol tag the request carries and the response must echo (a
/// response without it is a contract violation, never a best-effort
/// parse).
pub const WORKER_PROTOCOL_ID: &str = "flexcore-grid-worker/1";

/// The one request kind this backend speaks.
pub const WORKER_REQUEST_RUN_POWER_FLOW: &str = "run_power_flow";

/// The wire request (one JSON document on the worker's stdin).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkerRequest<'a> {
    /// The protocol tag ([`WORKER_PROTOCOL_ID`]).
    pub protocol: &'a str,
    /// The request kind ([`WORKER_REQUEST_RUN_POWER_FLOW`]).
    pub request: &'a str,
    /// The provider-neutral model IR — the worker translates it into the
    /// pinned provider's model inside the worker process.
    pub model: &'a VersionedNetworkModel,
    /// The already-translated injections (crate::translation output).
    pub injections: &'a [BackendInjection],
    /// The run options (evaluation label; the mandatory version report).
    pub options: &'a BackendRunOptions,
}

/// The wire response envelope: the protocol tag plus the outcome. The
/// outcome itself reuses [`BackendPowerFlowOutcome`]'s serde form
/// verbatim — one vocabulary, no drift.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct WorkerResponse {
    /// The protocol tag; must equal [`WORKER_PROTOCOL_ID`].
    pub protocol: String,
    /// The worker outcome (the `backend.rs` contract shape).
    #[serde(flatten)]
    pub outcome: BackendPowerFlowOutcome,
}

/// The REAL `GridModelBackend` implementation over the Python numerical
/// worker process (`apps/worker/python/grid_worker.py` — the P5.15 leg).
///
/// The worker's installed provider version is cross-checked against the
/// recorded pin BY THE ADAPTER (the pin discipline lives in
/// `adapter.rs`); this backend only transports and validates the payload
/// contract.
pub struct WorkerProcessBackend {
    program: String,
    args: Vec<String>,
    timeout: Duration,
}

impl WorkerProcessBackend {
    /// Composes the backend: the interpreter program (e.g. `python`), the
    /// argument list (e.g. the worker entrypoint script path), and the
    /// bounded wait for the worker's answer.
    pub fn new(program: String, args: Vec<String>, timeout: Duration) -> Self {
        Self {
            program,
            args,
            timeout,
        }
    }

    /// Serializes the wire request (the exact document the worker reads).
    pub fn request_text(
        model: &VersionedNetworkModel,
        injections: &[BackendInjection],
        options: &BackendRunOptions,
    ) -> Result<String, BackendError> {
        let request = WorkerRequest {
            protocol: WORKER_PROTOCOL_ID,
            request: WORKER_REQUEST_RUN_POWER_FLOW,
            model,
            injections,
            options,
        };
        serde_json::to_string(&request)
            .map_err(|error| BackendError::ContractViolation(format!(
                "the worker request could not be serialized: {error}"
            )))
    }

    /// Parses and validates the worker's stdout document (the fail-closed
    /// payload checks: protocol tag, JSON shape, finiteness of every
    /// number). A violation is a `BackendError::ContractViolation` — the
    /// adapter maps it to the P4.08 `ProviderError` trigger.
    pub fn parse_response(text: &str) -> Result<BackendPowerFlowOutcome, BackendError> {
        let response: WorkerResponse = serde_json::from_str(text).map_err(|error| {
            BackendError::ContractViolation(format!(
                "the worker response is not a contract-shaped JSON document: {error}"
            ))
        })?;
        if response.protocol != WORKER_PROTOCOL_ID {
            return Err(BackendError::ContractViolation(format!(
                "the worker response carries protocol '{}' but this backend speaks '{}'",
                response.protocol, WORKER_PROTOCOL_ID
            )));
        }
        let outcome = response.outcome;
        // Finiteness: a NaN or infinite result value is a contract
        // violation, never a silently published number (the provider
        // result's completeness gate would refuse it downstream, but the
        // refuse point is HERE, at the transport boundary).
        let mut check = |name: &str, value: f64| -> Result<(), BackendError> {
            if value.is_finite() {
                Ok(())
            } else {
                Err(BackendError::ContractViolation(format!(
                    "the worker response's {name} is not finite ({value})"
                )))
            }
        };
        for bus in &outcome.bus_results {
            check("bus vm_pu", bus.vm_pu)?;
        }
        for branch in &outcome.branch_results {
            check("branch i_from_ka", branch.i_from_ka)?;
            check("branch i_to_ka", branch.i_to_ka)?;
            check("branch loading_percent", branch.loading_percent)?;
        }
        for transformer in &outcome.transformer_results {
            check("transformer loading_percent", transformer.loading_percent)?;
            check("transformer i_hv_ka", transformer.i_hv_ka)?;
            check("transformer i_lv_ka", transformer.i_lv_ka)?;
        }
        Ok(outcome)
    }
}

impl GridModelBackend for WorkerProcessBackend {
    fn run_power_flow(
        &self,
        model: &VersionedNetworkModel,
        injections: &[BackendInjection],
        options: &BackendRunOptions,
    ) -> Result<BackendPowerFlowOutcome, BackendError> {
        let request = Self::request_text(model, injections, options)?;
        let mut child = std::process::Command::new(&self.program)
            .args(&self.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|error| BackendError::Unavailable(format!(
                "the grid worker process could not be spawned (program '{}'): {error}",
                self.program
            )))?;

        // Write the request, close stdin (the worker reads ONE document).
        {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| BackendError::Unavailable("the worker's stdin was closed".to_string()))?;
            stdin
                .write_all(request.as_bytes())
                .map_err(|error| BackendError::Unavailable(format!(
                    "the worker request could not be written: {error}"
                )))?;
        }

        // Bounded wait: read the output on a helper thread and enforce the
        // timeout from this thread (std has no timed wait on a child; the
        // helper-thread + channel pattern keeps the wait bounded without
        // an async runtime). On timeout the worker is killed: a hung
        // worker is fail-closed (Unavailable), never a silent pass.
        let (sender, receiver) = mpsc::channel();
        let handle = child.stdout.take();
        let stderr_handle = child.stderr.take();
        let reader = std::thread::spawn(move || {
            use std::io::Read;
            let mut text = String::new();
            if let Some(mut stdout) = handle {
                let _ = stdout.read_to_string(&mut text);
            }
            let mut stderr_text = String::new();
            if let Some(mut stderr) = stderr_handle {
                let _ = stderr.read_to_string(&mut stderr_text);
            }
            let _ = sender.send((text, stderr_text));
        });

        let (text, stderr_text) = match receiver.recv_timeout(self.timeout) {
            Ok(pair) => pair,
            Err(_) => {
                let _ = child.kill();
                let _ = reader.join();
                return Err(BackendError::Unavailable(format!(
                    "the grid worker did not answer within {:?}; the worker was killed and \
                     the outcome is fail-closed (no retry — D-P4.05-2)",
                    self.timeout
                )));
            }
        };
        let status = child.wait().map_err(|error| BackendError::Unavailable(format!(
            "the worker process could not be reaped: {error}"
        )))?;
        if !status.success() && text.trim().is_empty() {
            return Err(BackendError::Unavailable(format!(
                "the grid worker exited with {} and produced no result document{}",
                status,
                if stderr_text.trim().is_empty() {
                    String::new()
                } else {
                    format!(" (worker stderr: {})", stderr_text.trim())
                }
            )));
        }
        if text.trim().is_empty() {
            return Err(BackendError::ContractViolation(
                "the grid worker produced no result document on stdout".to_string(),
            ));
        }
        Self::parse_response(&text)
    }

    fn describe(&self) -> BackendDescription {
        BackendDescription {
            name: "worker-process-backend".to_string(),
            version: WORKER_PROTOCOL_ID.to_string(),
            notes: vec![
                format!("program: {} {:?}", self.program, self.args),
                format!("timeout: {:?}", self.timeout),
                // The honesty note travels with the backend identity, so
                // no run record can lose it.
                "the Python worker leg was WRITTEN, never executed (P5.15 loop bans); its \
                 pandapower 3.5.6 API usage is per the documented API and UNVERIFIED until \
                 a human build/test pass runs it"
                    .to_string(),
                "the transport is one JSON request on stdin, one JSON response on stdout \
                 (std::process, bounded timeout, fail-closed on every failure)"
                    .to_string(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{BackendBusResult, BackendBranchResult, BackendTransformerResult};
    use crate::port::{BranchSpec, BusSpec, ExternalGridSpec, GridModelRef, TransformerSpec};

    fn model() -> VersionedNetworkModel {
        VersionedNetworkModel {
            model_ref: GridModelRef {
                model_id: "feeder-test".to_string(),
                version: "1".to_string(),
                content_hash: "hash-test".to_string(),
            },
            base_mva: 100.0,
            buses: vec![
                BusSpec {
                    bus_id: "bus_1".to_string(),
                    nominal_kv: 20.0,
                    is_slack: true,
                },
                BusSpec {
                    bus_id: "bus_2".to_string(),
                    nominal_kv: 0.4,
                    is_slack: false,
                },
            ],
            branches: vec![BranchSpec {
                branch_id: "line_1_2".to_string(),
                from_bus: "bus_1".to_string(),
                to_bus: "bus_2".to_string(),
                r_ohm: 0.21,
                x_ohm: 0.07,
                rated_current_a: 500.0,
            }],
            transformers: Vec::new(),
            external_grids: vec![ExternalGridSpec {
                bus_id: "bus_1".to_string(),
                vm_pu: 1.02,
                va_degree: 0.0,
            }],
        }
    }

    fn options() -> BackendRunOptions {
        BackendRunOptions {
            evaluation_label: "worker-backend-test".to_string(),
            report_provider_version: true,
        }
    }

    fn scripted_response_json() -> String {
        format!(
            r#"{{"protocol":"{WORKER_PROTOCOL_ID}","converged":true,"reported_error":null,
            "provider_version_reported":"3.5.6",
            "bus_results":[{{"bus_id":"bus_1","vm_pu":1.02}},{{"bus_id":"bus_2","vm_pu":1.01}}],
            "branch_results":[{{"branch_id":"line_1_2","i_from_ka":0.38,"i_to_ka":0.3795,"loading_percent":76.0}}],
            "transformer_results":[],
            "iterations":4,
            "worker_status_text":"converged in 4 iterations",
            "warnings":[]}}"#
        )
    }

    #[test]
    fn the_request_document_carries_the_contract_fields() {
        let text = WorkerProcessBackend::request_text(&model(), &[], &options())
            .expect("the request serializes");
        let value: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(value["protocol"], WORKER_PROTOCOL_ID);
        assert_eq!(value["request"], WORKER_REQUEST_RUN_POWER_FLOW);
        assert_eq!(value["model"]["model_ref"]["model_id"], "feeder-test");
        assert_eq!(value["model"]["buses"][0]["bus_id"], "bus_1");
        assert_eq!(value["options"]["evaluation_label"], "worker-backend-test");
        // The document is ONE line of JSON (the worker reads one document
        // from stdin to EOF).
        assert!(!text.contains('\n'));
    }

    #[test]
    fn a_contract_shaped_response_parses_into_the_outcome() {
        let outcome = WorkerProcessBackend::parse_response(&scripted_response_json())
            .expect("the scripted response is contract-shaped");
        assert!(outcome.converged);
        assert_eq!(
            outcome.provider_version_reported.as_deref(),
            Some("3.5.6")
        );
        assert_eq!(outcome.bus_results.len(), 2);
        assert_eq!(outcome.branch_results.len(), 1);
        assert_eq!(outcome.transformer_results.len(), 0);
        assert_eq!(outcome.iterations, Some(4));
        assert!(outcome.warnings.is_empty());
    }

    #[test]
    fn a_malformed_payload_is_a_contract_violation_never_a_best_effort_parse() {
        // Not JSON at all.
        let error = WorkerProcessBackend::parse_response("not json").unwrap_err();
        assert!(matches!(error, BackendError::ContractViolation(_)));
        // Valid JSON but the wrong protocol tag.
        let wrong_tag = scripted_response_json().replace("flexcore-grid-worker/1", "other/1");
        let error = WorkerProcessBackend::parse_response(&wrong_tag).unwrap_err();
        assert!(matches!(error, BackendError::ContractViolation(_)));
        // Missing required field (no converged flag).
        let missing = r#"{"protocol":"flexcore-grid-worker/1","bus_results":[]}"#;
        let error = WorkerProcessBackend::parse_response(missing).unwrap_err();
        assert!(matches!(error, BackendError::ContractViolation(_)));
    }

    #[test]
    fn a_non_finite_result_value_is_a_contract_violation() {
        // NaN cannot appear in strict JSON, but the parse path still
        // refuses non-finite values from any JSON number form (e.g. the
        // 1e999 overflow renders as inf in some parsers) — the check is
        // the fail-closed boundary, and a hand-written document proves it.
        let non_finite = format!(
            r#"{{"protocol":"{WORKER_PROTOCOL_ID}","converged":true,"reported_error":null,
            "provider_version_reported":"3.5.6",
            "bus_results":[{{"bus_id":"bus_1","vm_pu":1e999}}],
            "branch_results":[],"transformer_results":[],
            "iterations":null,"worker_status_text":"x","warnings":[]}}"#
        );
        let error = WorkerProcessBackend::parse_response(&non_finite).unwrap_err();
        assert!(matches!(error, BackendError::ContractViolation(_)));
    }

    #[test]
    fn the_outcome_shapes_round_trip_through_the_wire() {
        // The response payload reuses the crate's existing serde shapes —
        // a scripted document and a serialized outcome must agree.
        let outcome = BackendPowerFlowOutcome {
            converged: false,
            reported_error: Some("worker reported".to_string()),
            provider_version_reported: Some("3.5.6".to_string()),
            bus_results: vec![BackendBusResult {
                bus_id: "bus_1".to_string(),
                vm_pu: 0.5,
            }],
            branch_results: vec![BackendBranchResult {
                branch_id: "l".to_string(),
                i_from_ka: 1.0,
                i_to_ka: 1.0,
                loading_percent: 0.0,
            }],
            transformer_results: vec![BackendTransformerResult {
                transformer_id: "t".to_string(),
                loading_percent: 0.0,
                i_hv_ka: 0.0,
                i_lv_ka: 0.0,
            }],
            iterations: None,
            worker_status_text: "test".to_string(),
            warnings: vec!["w".to_string()],
        };
        let wire = serde_json::to_string(&outcome).expect("serializes");
        let parsed = WorkerProcessBackend::parse_response(&format!(
            r#"{{"protocol":"{WORKER_PROTOCOL_ID}",{}}}"#,
            &wire[1..wire.len() - 1]
        ))
        .expect("the envelope wraps the outcome verbatim");
        assert_eq!(parsed, outcome);
    }

    #[test]
    fn the_backend_description_carries_the_honesty_note() {
        let backend = WorkerProcessBackend::new(
            "python".to_string(),
            vec!["apps/worker/python/grid_worker.py".to_string()],
            Duration::from_secs(30),
        );
        let description = backend.describe();
        assert!(description
            .notes
            .iter()
            .any(|note| note.contains("UNVERIFIED")));
        assert!(description
            .notes
            .iter()
            .any(|note| note.contains("never executed")));
    }
}
