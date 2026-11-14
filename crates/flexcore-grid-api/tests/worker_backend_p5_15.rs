// P5.15 — The worker-process backend integration suite: the REAL
//         `GridModelBackend` over the Python numerical worker leg
//         (`apps/worker/python/grid_worker.py`), tested with SCRIPTED
//         fake-worker payloads — NEVER with an executed worker.
//
// WHAT IS TESTED:
//   1. A spawn failure (the worker program does not exist) is fail-closed:
//      `BackendError::Unavailable`, the P4.08 ProviderError/HOLD trigger —
//      never a best-effort pass.
//   2. A SCRIPTED fake-worker stdout document (the exact wire shape the
//      worker contract defines) drives the REAL provider pipeline:
//      parse → pin cross-check → scope gate → translation → result
//      mapping → completeness gate → port verdict.
//   3. A worker that answers nothing (unavailable) maps to the fail-closed
//      `GridPortError::BackendUnavailable`.
//   4. A worker whose payload violates the contract maps to the
//      fail-closed `GridPortError::BackendContractViolation`.
//   5. Row completeness at the adapter: a fake-worker document missing a
//      bus row is refused — one row per model element is a CONTRACT.
//
// WHAT IS DELIBERATELY NOT TESTED HERE (recorded honestly):
//   - The live timeout path (a hung worker killed after the bounded wait):
//     testing it needs a real process that hangs, which is a
//     platform/timing-dependent test; the timeout branch is implemented
//     (helper-thread + `recv_timeout`, worker killed, Unavailable) and
//     reviewed, not executed in this loop.
//   - ANY pandapower behaviour: the fake worker doubles speak the wire
//     contract only. The Python worker itself was WRITTEN, never executed
//     (pandapower 3.5.6 never installed); its API usage is UNVERIFIED
//     until the human build/test pass runs it. No test here proves
//     anything about pandapower, and no test here produces a power-flow
//     result that could be mistaken for a computed one — the scripted
//     documents are fixtures, named as such.

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{model_ref, test_limits, EXPECTED_PIN, ScriptedBackend};
use flexcore_grid_api::adapter::{
    balanced_worker_scope, PandapowerGridProvider, PROVIDER_ID, PROVIDER_PINNED_VERSION,
};
use flexcore_grid_api::backend::{
    BackendError, BackendInjection, BackendPowerFlowOutcome, BackendRunOptions, GridModelBackend,
};
use flexcore_grid_api::port::{
    FeasibilityVerdict, GridModelProvider, GridPortError, VersionedNetworkModel,
};
use flexcore_grid_api::tests_support::{converged_outcome, three_bus_model};
use flexcore_grid_api::worker_backend::{
    WorkerProcessBackend, WORKER_PROTOCOL_ID, WORKER_REQUEST_RUN_POWER_FLOW,
};
use flexcore_contracts::units::{ActivePowerMW, ReactivePowerMVAr, ReactiveSignConvention};

/// The SCRIPTED fake-worker stdout document: the exact `flexcore-grid-worker/1`
/// envelope whose payload is the `BackendPowerFlowOutcome` serde shape. This
/// is a FIXTURE (a hand-written document, never produced by an executed
/// worker); the provider pipeline downstream of the transport is REAL.
fn fake_worker_stdout() -> String {
    format!(
        r#"{{"protocol":"{WORKER_PROTOCOL_ID}","converged":true,"reported_error":null,
        "provider_version_reported":"{EXPECTED_PIN}",
        "bus_results":[{{"bus_id":"bus_1","vm_pu":1.02}},{{"bus_id":"bus_2","vm_pu":1.01}},{{"bus_id":"bus_3","vm_pu":1.005}}],
        "branch_results":[{{"branch_id":"line_2_3","i_from_ka":0.38,"i_to_ka":0.3795,"loading_percent":76.0}}],
        "transformer_results":[{{"transformer_id":"trafo_1_2","loading_percent":62.5,"i_hv_ka":0.03,"i_lv_ka":1.5}}],
        "iterations":4,
        "worker_status_text":"AC power flow (scripted fake-worker fixture — NOT an executed worker; NOT a computed result)",
        "warnings":[]}}"#
    )
}

/// A backend double that always fails with the given error (the stand-in
/// for "the worker process was unreachable / answered garbage").
struct FailingWorker {
    error: BackendError,
}

impl GridModelBackend for FailingWorker {
    fn run_power_flow(
        &self,
        _model: &VersionedNetworkModel,
        _injections: &[BackendInjection],
        _options: &BackendRunOptions,
    ) -> Result<BackendPowerFlowOutcome, BackendError> {
        Err(self.error.clone())
    }

    fn describe(&self) -> flexcore_grid_api::backend::BackendDescription {
        flexcore_grid_api::backend::BackendDescription {
            name: "failing-worker-double".to_string(),
            version: "0".to_string(),
            notes: vec!["fail-closed path double (never an executed worker)".to_string()],
        }
    }
}

fn export_candidate(bus_id: &str) -> flexcore_grid_api::port::CandidateInjection {
    flexcore_grid_api::port::CandidateInjection {
        injection_id: "cand-001".to_string(),
        bus_id: bus_id.to_string(),
        active_power: ActivePowerMW::new(2.5).expect("valid"),
        reactive_power: ReactivePowerMVAr::new(0.5).expect("valid"),
        reactive_sign_convention: ReactiveSignConvention::InjectionPositive,
        interval_start: None,
    }
}

// ---------------------------------------------------------------------------
// 1. Spawn failure ⇒ fail-closed Unavailable (the P4.08 ProviderError
//    trigger — never a pass).
// ---------------------------------------------------------------------------

#[test]
fn a_spawn_failure_is_fail_closed_unavailable() {
    let backend = WorkerProcessBackend::new(
        "definitely-not-a-real-worker-program-flexcore-p5-15".to_string(),
        vec!["apps/worker/python/grid_worker.py".to_string()],
        Duration::from_secs(10),
    );
    let model = three_bus_model(model_ref());
    let error = backend
        .run_power_flow(&model, &[], &BackendRunOptions {
            evaluation_label: "p5-15-spawn-failure".to_string(),
            report_provider_version: true,
        })
        .expect_err("a failed spawn is an error, never a fabricated outcome");
    match error {
        BackendError::Unavailable(message) => {
            assert!(
                message.contains("could not be spawned"),
                "the error names the transport failure: {message}"
            );
        }
        BackendError::ContractViolation(message) => {
            panic!("a spawn failure is Unavailable, not a contract violation: {message}")
        }
    }
}

// ---------------------------------------------------------------------------
// 2. The scripted fake-worker document drives the REAL provider pipeline.
// ---------------------------------------------------------------------------

#[test]
fn the_scripted_fake_worker_payload_drives_the_real_provider_pipeline() {
    // The transport boundary: parse + validate the fake worker document.
    let outcome = WorkerProcessBackend::parse_response(&fake_worker_stdout())
        .expect("the scripted fake-worker document is contract-shaped");
    assert!(outcome.converged);
    assert_eq!(
        outcome.provider_version_reported.as_deref(),
        Some(EXPECTED_PIN),
        "the fake worker reports the pinned provider version (the adapter \
         cross-checks it — a wrong version is refused downstream)"
    );

    // The REAL provider over the parsed fake-worker outcome.
    let provider = PandapowerGridProvider::new(
        Arc::new(ScriptedBackend::new(outcome)),
        balanced_worker_scope(),
    )
    .expect("the recorded balanced scope is accepted");

    let model = three_bus_model(model_ref());
    let candidates = vec![export_candidate("bus_3")];
    let verdict = provider
        .evaluate_dispatch(&model, &candidates, &test_limits())
        .expect("the scripted fake-worker payload evaluates through the real pipeline");
    assert_eq!(
        verdict,
        FeasibilityVerdict::WithinLimits,
        "the scripted fixture magnitudes are within the declared limits — and it is \
         the FIXTURE that makes them so, never a computed power flow"
    );

    // The run provenance names the real adapter identity.
    let capabilities = provider.capabilities();
    assert_eq!(capabilities.provider_id, PROVIDER_ID);
    assert_eq!(capabilities.provider_version, PROVIDER_PINNED_VERSION);
}

// ---------------------------------------------------------------------------
// 3. Worker unavailable ⇒ fail-closed port error.
// ---------------------------------------------------------------------------

#[test]
fn a_worker_unavailable_error_maps_to_the_fail_closed_port_error() {
    let provider = PandapowerGridProvider::new(
        Arc::new(FailingWorker {
            error: BackendError::Unavailable(
                "the grid worker did not answer within 30s; the worker was killed".to_string(),
            ),
        }),
        balanced_worker_scope(),
    )
    .expect("the recorded balanced scope is accepted");

    let model = three_bus_model(model_ref());
    let candidates = vec![export_candidate("bus_3")];
    let error = provider
        .evaluate_dispatch(&model, &candidates, &test_limits())
        .expect_err("an unavailable worker is an error, never a verdict");
    assert!(
        matches!(error, GridPortError::BackendUnavailable(_)),
        "the P4.08 ProviderError trigger is a fail-closed port error, got {error:?}"
    );
}

// ---------------------------------------------------------------------------
// 4. Worker contract violation ⇒ fail-closed port error.
// ---------------------------------------------------------------------------

#[test]
fn a_worker_contract_violation_maps_to_the_fail_closed_port_error() {
    let provider = PandapowerGridProvider::new(
        Arc::new(FailingWorker {
            error: BackendError::ContractViolation(
                "the worker response carries protocol 'other/1'".to_string(),
            ),
        }),
        balanced_worker_scope(),
    )
    .expect("the recorded balanced scope is accepted");

    let model = three_bus_model(model_ref());
    let candidates = vec![export_candidate("bus_3")];
    let error = provider
        .evaluate_dispatch(&model, &candidates, &test_limits())
        .expect_err("a contract-violating worker is an error, never a verdict");
    assert!(
        matches!(error, GridPortError::BackendContractViolation(_)),
        "a malformed worker payload fail-closes at the port boundary, got {error:?}"
    );
}

// ---------------------------------------------------------------------------
// 5. Row completeness at the adapter: a fake-worker document missing a bus
//    row is refused — one row per model element is a CONTRACT.
// ---------------------------------------------------------------------------

#[test]
fn a_fake_worker_document_missing_a_result_row_is_refused() {
    let mut incomplete = converged_outcome(EXPECTED_PIN);
    incomplete.bus_results.remove(2); // bus_3 missing
    let provider = PandapowerGridProvider::new(
        Arc::new(ScriptedBackend::new(incomplete)),
        balanced_worker_scope(),
    )
    .expect("the recorded balanced scope is accepted");

    let model = three_bus_model(model_ref());
    let candidates = vec![export_candidate("bus_3")];
    let error = provider
        .evaluate_dispatch(&model, &candidates, &test_limits())
        .expect_err("a result missing a model element's row is a contract failure");
    assert!(
        matches!(
            error,
            GridPortError::BackendContractViolation(_) | GridPortError::IncompleteResult(_)
        ),
        "the row-completeness contract fail-closes, got {error:?}"
    );
}

// ---------------------------------------------------------------------------
// 6. The backend identity carries the honesty note verbatim — a run record
//    built from the real backend can never lose it.
// ---------------------------------------------------------------------------

#[test]
fn the_worker_backend_identity_carries_the_unverified_note() {
    let backend = WorkerProcessBackend::new(
        "python".to_string(),
        vec!["apps/worker/python/grid_worker.py".to_string()],
        Duration::from_secs(30),
    );
    let description = backend.describe();
    assert_eq!(description.name, "worker-process-backend");
    assert_eq!(description.version, WORKER_PROTOCOL_ID);
    assert!(
        description
            .notes
            .iter()
            .any(|note| note.contains("UNVERIFIED") && note.contains("never executed")),
        "the UNVERIFIED-API honesty note travels with the backend identity: {:?}",
        description.notes
    );
}

// ---------------------------------------------------------------------------
// 7. The wire request the backend emits is exactly the document the Python
//    worker's stdin contract expects (protocol, request kind, model IR,
//    injections, options) — one line of JSON.
// ---------------------------------------------------------------------------

#[test]
fn the_wire_request_matches_the_python_worker_stdin_contract() {
    let model = three_bus_model(model_ref());
    let injections = vec![BackendInjection {
        injection_id: "cand-001".to_string(),
        bus_id: "bus_3".to_string(),
        p_injection_mw: 2.5,
        q_injection_mvar: 0.5,
        interval_start: None,
    }];
    let text = WorkerProcessBackend::request_text(
        &model,
        &injections,
        &BackendRunOptions {
            evaluation_label: "p5-15-wire-contract".to_string(),
            report_provider_version: true,
        },
    )
    .expect("the request serializes");
    let value: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
    assert_eq!(value["protocol"], WORKER_PROTOCOL_ID);
    assert_eq!(value["request"], WORKER_REQUEST_RUN_POWER_FLOW);
    assert_eq!(value["model"]["model_ref"]["model_id"], "feeder-contract-test");
    assert_eq!(value["injections"][0]["bus_id"], "bus_3");
    assert_eq!(value["injections"][0]["p_injection_mw"], 2.5);
    assert_eq!(value["options"]["report_provider_version"], true);
    // ONE document (the worker reads stdin to EOF): no embedded newlines.
    assert!(!text.contains('\n'));
}
