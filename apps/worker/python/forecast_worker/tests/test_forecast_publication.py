# P15.17 — pytest suite for the publication-record seam (WRITTEN-UNRUN:
#          no pytest run was executed in this loop; the human build/test
#          gate owns the run. Every expectation below is hand-computed or
#          a literal copied from the worker module's own constants — the
#          same mirror discipline as test_forecast_baseline_worker.py.
#          Nothing is fabricated.)
#
# Covers the P15.17 completion additions only (mode
# `publish_forecast_record`); the baseline-mode suite is
# test_forecast_baseline_worker.py, unchanged.
#
# Run instruction (human gate):
#   pytest apps/worker/python/forecast_worker/tests/

import json
import subprocess
import sys
from pathlib import Path

WORKER = Path(__file__).resolve().parent.parent / "forecast_baseline_worker.py"

import importlib.util  # noqa: E402 — the worker path is computed above

_spec = importlib.util.spec_from_file_location("forecast_baseline_worker", WORKER)
worker = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(worker)


# ---------------------------------------------------------------------------
# The FNV-1a 64 mirror (published test vectors — the same pins the Rust
# helper carries)
# ---------------------------------------------------------------------------


def test_fnv1a64_matches_published_vectors():
    assert worker.fnv1a64(b"") == 0xCBF29CE484222325
    assert worker.fnv1a64(b"a") == 0xAF63DC4C8601EC8C
    assert worker.fnv1a64(b"foobar") == 0x85944171F73967E8


# ---------------------------------------------------------------------------
# The publication request path
# ---------------------------------------------------------------------------


def _valid_request(**overrides):
    request = {
        "protocol": worker.PROTOCOL_ID,
        "mode": worker.MODE_PUBLISH,
        "baseline": worker.BASELINE_PERSISTENCE,
        "permissions_declared": ["telemetry.read", "forecast.publish"],
        "scope": {"scope_kind": "site", "scope_id": "site-north"},
        "horizon": {"steps": 3, "interval_millis": 900000},
        "history": [-0.4, -0.5, -0.6],
        "season_length": None,
        "degree_day": None,
        "publication": {
            "forecast_version": "v1",
            "artifact_kind": "load",
            "input_data_ref": "fixture-forecast-input-record",
            "producing_model_ref": "forecast-baseline/persistence@v1",
            "eval_set_ref": "forecast-eval-set-v1",
        },
        "provenance": {
            "input_snapshot_id": "snapshot-RECORD",
            "request_id": "req-0001",
        },
    }
    request.update(overrides)
    return request


def test_publication_completes_with_the_required_provenance_links():
    result = worker.run_publication(_valid_request())
    assert result["ok"] is True
    assert result["kind"] == "forecast_publication_record"
    assert result["payload_type"] == "flexcore.forecast.publication_record.v1"
    assert result["proposal_only"] is True
    assert result["authority"] == "none"
    provenance = result["forecast"]["outputs"]["publication"]["provenance_record"]
    assert provenance["schema_id"] == "flexcore-forecast-artifact-provenance-v1"
    assert provenance["digest_algorithm"] == "fnv1a64-test"
    assert provenance["input_data_ref"] == "fixture-forecast-input-record"
    assert provenance["producing_model_ref"] == "forecast-baseline/persistence@v1"
    assert provenance["eval_set_ref"] == "forecast-eval-set-v1"
    assert provenance["signer_key_reference"] is None
    assert "UNSIGNED" in provenance["signing_status"]
    # The persistence rule set ran under the record: last value carried
    # forward (the hand-computed mirror vector).
    assert result["forecast"]["outputs"]["values"] == [-0.6, -0.6, -0.6]


def test_publication_digest_is_deterministic_and_content_sensitive():
    first = worker.run_publication(_valid_request())["forecast"]["outputs"][
        "publication"
    ]["provenance_record"]["digest"]
    second = worker.run_publication(_valid_request())["forecast"]["outputs"][
        "publication"
    ]["provenance_record"]["digest"]
    assert first == second, "the same request reproduces the same digest"
    changed = worker.run_publication(_valid_request(forecast_version="v2"))[
        "forecast"
    ]["outputs"]["publication"]["provenance_record"]["digest"]
    assert changed != first, "a content change changes the digest"


def test_publication_envelope_passes_proposal_only():
    result = worker.run_publication(_valid_request())
    worker.assert_proposal_only(result)  # must not raise


def test_missing_provenance_link_is_refused_before_computation():
    request = _valid_request()
    del request["publication"]["producing_model_ref"]
    try:
        worker.validate_request(request)
    except worker.WorkerRequestError as error:
        assert "producing_model_ref" in str(error)
    else:
        raise AssertionError("a missing provenance link must be refused")


def test_unknown_artifact_kind_is_refused():
    request = _valid_request()
    request["publication"]["artifact_kind"] = "temperature"
    try:
        worker.validate_request(request)
    except worker.WorkerRequestError as error:
        assert "artifact_kind" in str(error)
    else:
        raise AssertionError("an unknown artifact kind must be refused")


def test_whitespace_forecast_version_is_refused():
    request = _valid_request()
    request["publication"]["forecast_version"] = "v1 v2"
    try:
        worker.validate_request(request)
    except worker.WorkerRequestError as error:
        assert "whitespace-free" in str(error)
    else:
        raise AssertionError("a whitespace version must be refused")


def test_statements_are_the_recorded_strings():
    assert worker.PROVENANCE_SCHEMA_ID_MIRROR == (
        "flexcore-forecast-artifact-provenance-v1"
    )
    assert worker.DIGEST_ALGORITHM_FNV1A64_TEST == "fnv1a64-test"
    assert "NOT a security primitive" in worker.TEST_GRADE_DIGEST_STATEMENT
    assert "never a command" in worker.PUBLICATION_BOUNDARY_STATEMENT


# ---------------------------------------------------------------------------
# The transport contract (one JSON in -> one envelope out; malformed ->
# NOTHING on stdout, exit 2)
# ---------------------------------------------------------------------------


def _run_worker(payload: str):
    return subprocess.run(
        [sys.executable, str(WORKER)],
        input=payload,
        capture_output=True,
        text=True,
    )


def test_transport_contract_publication_mode():
    done = _run_worker(json.dumps(_valid_request()))
    assert done.returncode == 0, done.stderr
    envelope = json.loads(done.stdout)
    assert envelope["kind"] == "forecast_publication_record"
    assert envelope["ok"] is True


def test_transport_contract_malformed_publication_is_silent():
    request = _valid_request()
    del request["publication"]
    refused = _run_worker(json.dumps(request))
    assert refused.returncode == 2
    assert refused.stdout == "", "a refused request emits NOTHING on stdout"
    assert "publication" in refused.stderr

    malformed = _run_worker("{not json")
    assert malformed.returncode == 2
    assert malformed.stdout == "", "a malformed request emits NOTHING on stdout"
