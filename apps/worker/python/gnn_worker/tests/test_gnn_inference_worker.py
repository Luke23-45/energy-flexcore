# P8.11 — The GNN inference-worker test suite: pure-logic parts that RUN
#         WITHOUT torch/PyG installed, plus the named DECISION-POINT gate
#         tests.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. WRITTEN, NEVER RUN. pytest was never installed in this loop; no test
#    below has ever executed. The pure-logic tests target stdlib-only
#    worker functions (graph-shape checks, feature-shape/range checks,
#    topology-scope flagging, the closed P8.04 result schema, the
#    proposal-only boundary, the ranking order) and are expected to pass
#    at the first human run. The inference-path tests assert the HONESTY
#    GATE (the NAMED DECISION-POINT ImportError) and are guarded so they
#    skip loudly on a machine where torch actually IS installed — there,
#    the guarded-path tests belong to the human build/test pass after the
#    verified torch/PyG pair is pinned (P8.03 §4 DECISION-POINT; nothing
#    is selected or installed in this repository).
# 2. Run instruction (the recorded procedure):
#    `PYTHONPATH=apps/worker/python/gnn_worker python -m pytest
#    apps/worker/python/gnn_worker/tests -v` from the repository root.
#    The in-module hand-computed self-tests run WITHOUT pytest:
#    `python apps/worker/python/gnn_worker/gnn_inference_worker.py`.
# 3. What this suite pins (ledger P8.11 acceptance, the parts provable
#    without any framework): fail-closed request validation (refusals
#    before any framework import or artifact check), graph-shape and
#    feature-shape/range checks (SOC in [0,1], binary status, positive
#    pu voltage, canonical units), the out-of-scope topology FLAG
#    (in-band, outputs NULL — computed with NO framework installed), the
#    missing-model-artifact loud failure, the closed P8.04 result schema
#    (training_data_ref + topology_scope mandatory), the proposal-only
#    boundary in code, the deterministic ranking order with hand-computed
#    expectations, the fixed envelope posture (proposal_only, authority
#    none, mode lock, boundary statements incl. "a warm start affects
#    SPEED, never optimality"), the §9.2 provenance field names, and the
#    fail-closed composition edge (malformed stdin → exit 2, nothing on
#    stdout). NOTHING here proves anything about GNN behaviour, and NO
#    accuracy/latency/speedup figure exists anywhere in this suite.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import io
import json
import os
import sys

import pytest

sys.path.insert(
    0,
    os.path.join(os.path.dirname(os.path.abspath(__file__)), os.pardir),
)

import gnn_inference_worker as worker  # noqa: E402 — path set above


def _valid_request(**overrides) -> dict:
    """The canonical valid request (CONSTRUCTED test inputs, never
    measurements; the artifact ref names a file that does not exist and
    can never exist in this repository)."""
    request = {
        "protocol": worker.PROTOCOL_ID,
        "mode": "warm_start",
        "target_solver": "highs_lp_qp_mip",
        "model_manifest": {
            "model_version": "RECORD-AT-FIRST-RUN",
            "model_artifact_ref": "models/gnn/never-trained.pt",
            "training_data_ref": "object-store:training-data/RECORD-REF",
            "topology_scope": "balanced-radial-feeder/RECORD-CLASS",
            "trained_topology_ids": ["feeder-A", "feeder-B", "feeder-C"],
        },
        "topology": {
            "topology_id": "feeder-A",
            "node_count": 3,
            "edges": [[0, 1], [1, 2]],
        },
        "feature_declarations": [
            {"name": "p_export_mw", "kind": "power", "unit": "MW"},
            {"name": "soc_fraction", "kind": "soc", "unit": None},
        ],
        "features": [[1.5, 0.5], [0.0, 0.25], [-2.0, 1.0]],
        "provenance": {
            "input_snapshot_id": "snapshot-RECORD",
            "request_id": "req-0001",
        },
    }
    request.update(overrides)
    return request


def _valid_screening_request() -> dict:
    request = _valid_request(mode="screening")
    request["screening"] = {
        "candidates": [
            {"candidate_id": "constraint-1", "node_index": 1},
            {"candidate_id": "constraint-2", "node_index": 2},
        ]
    }
    return request


class TestGraphShapeChecks:
    def test_valid_three_node_graph_accepted(self) -> None:
        edges = worker.validate_graph(3, [[0, 1], [1, 2], [2, 0]])
        assert edges == [[0, 1], [1, 2], [2, 0]]

    def test_edge_endpoint_out_of_range_refused(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_graph(3, [[0, 3]])

    def test_negative_endpoint_refused(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_graph(3, [[-1, 2]])

    def test_self_loop_refused(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_graph(3, [[1, 1]])

    def test_non_integer_node_count_refused(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_graph("3", [])

    def test_zero_node_count_refused(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_graph(0, [])

    def test_parallel_edges_allowed(self) -> None:
        # Real feeders have parallel lines; duplicates are carried as given.
        edges = worker.validate_graph(2, [[0, 1], [0, 1]])
        assert edges == [[0, 1], [0, 1]]


class TestFeatureChecks:
    def _decls(self) -> list:
        return worker.validate_feature_declarations(
            [
                {"name": "p_export_mw", "kind": "power", "unit": "MW"},
                {"name": "soc_fraction", "kind": "soc", "unit": None},
            ]
        )

    def test_feature_row_count_mismatch_refused(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_features([[1.0, 0.5], [1.0, 0.5]], 3, self._decls())

    def test_feature_width_mismatch_refused(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_features(
                [[1.0], [1.0], [1.0]], 3, self._decls()
            )

    def test_non_finite_feature_refused(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_features(
                [[float("nan"), 0.5]], 1, self._decls()
            )

    def test_soc_boundary_values_accepted(self) -> None:
        decls = worker.validate_feature_declarations(
            [{"name": "s", "kind": "soc", "unit": None}]
        )
        worker.validate_features([[0.0], [1.0]], 2, decls)  # must not raise

    def test_soc_above_one_refused(self) -> None:
        decls = worker.validate_feature_declarations(
            [{"name": "s", "kind": "soc", "unit": None}]
        )
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_features([[1.5]], 1, decls)

    def test_soc_negative_refused(self) -> None:
        decls = worker.validate_feature_declarations(
            [{"name": "s", "kind": "soc", "unit": None}]
        )
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_features([[-0.1]], 1, decls)

    def test_status_non_binary_refused(self) -> None:
        decls = worker.validate_feature_declarations(
            [{"name": "st", "kind": "status", "unit": None}]
        )
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_features([[2.0]], 1, decls)

    def test_voltage_nonpositive_refused(self) -> None:
        decls = worker.validate_feature_declarations(
            [{"name": "v", "kind": "voltage", "unit": "pu"}]
        )
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_features([[0.0]], 1, decls)

    def test_feature_kind_outside_closed_set_refused(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_feature_declarations(
                [{"name": "x", "kind": "embedding", "unit": None}]
            )

    def test_non_canonical_power_unit_refused(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_feature_declarations(
                [{"name": "p", "kind": "power", "unit": "GW"}]
            )

    def test_voltage_kind_requires_pu_unit(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_feature_declarations(
                [{"name": "v", "kind": "voltage", "unit": "kV"}]
            )

    def test_soc_kind_forbids_unit(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_feature_declarations(
                [{"name": "s", "kind": "soc", "unit": "pu"}]
            )


class TestRequestValidation:
    def test_valid_warm_start_request_accepted(self) -> None:
        validated = worker.validate_request(_valid_request())
        assert validated["mode"] == "warm_start"
        assert validated["target_solver"] == "highs_lp_qp_mip"
        assert validated["topology"]["edges"] == [[0, 1], [1, 2]]
        assert validated["model_manifest"]["training_data_ref"].startswith(
            "object-store:"
        )

    def test_valid_screening_request_accepted(self) -> None:
        validated = worker.validate_request(_valid_screening_request())
        assert validated["mode"] == "screening"
        assert validated["screening"]["candidates"][0]["candidate_id"] == "constraint-1"

    def test_wrong_protocol_refused(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(_valid_request(protocol="flexcore-gnn-worker/2"))

    def test_unknown_mode_refused(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(_valid_request(mode="dispatch"))

    def test_unknown_target_solver_refused(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(_valid_request(target_solver="socp_solver"))

    def test_missing_manifest_refused(self) -> None:
        request = _valid_request()
        del request["model_manifest"]
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(request)

    def test_empty_training_data_ref_refused(self) -> None:
        request = _valid_request()
        request["model_manifest"]["training_data_ref"] = " "
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(request)

    def test_empty_trained_topology_list_refused(self) -> None:
        request = _valid_request()
        request["model_manifest"]["trained_topology_ids"] = []
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(request)

    def test_screening_mode_requires_candidates(self) -> None:
        request = _valid_screening_request()
        del request["screening"]
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(request)

    def test_duplicate_screening_candidate_refused(self) -> None:
        request = _valid_screening_request()
        request["screening"]["candidates"].append(
            {"candidate_id": "constraint-1", "node_index": 0}
        )
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(request)

    def test_screening_node_index_out_of_range_refused(self) -> None:
        request = _valid_screening_request()
        request["screening"]["candidates"][0]["node_index"] = 9
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(request)

    def test_timeout_over_recorded_max_refused(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(
                _valid_request(timeout_seconds=worker.MAX_INFERENCE_TIMEOUT_S + 1.0)
            )

    def test_timeout_nonpositive_refused(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(_valid_request(timeout_seconds=0.0))

    def test_timeout_within_max_accepted(self) -> None:
        validated = worker.validate_request(_valid_request(timeout_seconds=30.0))
        assert validated["timeout_seconds"] == 30.0

    def test_missing_provenance_refused(self) -> None:
        request = _valid_request()
        del request["provenance"]
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(request)

    def test_missing_input_snapshot_refused(self) -> None:
        request = _valid_request()
        request["provenance"] = {"request_id": "req-0002"}
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(request)


class TestProposalOnlyBoundary:
    def test_clean_envelope_passes(self) -> None:
        envelope = worker.build_result(
            validated=worker.validate_request(_valid_request()),
            termination_status="failed",
            outputs=None,
            diagnostics=["recorded reason"],
            torch_version=None,
            pyg_version=None,
            execution_ms=0,
        )
        worker.assert_proposal_only(envelope)  # must not raise

    def test_forbidden_key_refused_anywhere_in_tree(self) -> None:
        envelope = worker.build_result(
            validated=worker.validate_request(_valid_request()),
            termination_status="failed",
            outputs=None,
            diagnostics=[],
            torch_version=None,
            pyg_version=None,
            execution_ms=0,
        )
        smuggled = dict(envelope)
        smuggled["outputs"] = {"ranking": []}
        smuggled["outputs"]["authorization_token"] = "bearer-REDACTED"
        with pytest.raises(RuntimeError):
            worker.assert_proposal_only(smuggled)

    def test_nested_forbidden_key_refused(self) -> None:
        deep = {"outputs": {"ranking": [{"candidate_id": "c1", "command_id": "CMD-1"}]}}
        with pytest.raises(RuntimeError):
            worker.assert_proposal_only(deep)

    def test_list_payloads_scanned(self) -> None:
        smuggled = [{"ok": True}, {"dispatch": {"target": "device-1"}}]
        with pytest.raises(RuntimeError):
            worker.assert_proposal_only(smuggled)


class TestClosedResultSchema:
    def _canonical(self) -> dict:
        return {
            "result_kind": "candidate_setpoint_vector",
            "training_data_ref": "object-store:training-data/RECORD-REF",
            "topology_scope": "balanced-radial-feeder/RECORD-CLASS",
            "uncertainty": True,
        }

    def test_canonical_payload_passes(self) -> None:
        worker.enforce_gnn_result_schema(self._canonical())  # must not raise

    def test_unknown_key_refused(self) -> None:
        payload = self._canonical()
        payload["warm_start_vector"] = [1.0, 2.0, 3.0]
        with pytest.raises(worker.WorkerRequestError):
            worker.enforce_gnn_result_schema(payload)

    def test_missing_training_data_ref_refused(self) -> None:
        payload = self._canonical()
        del payload["training_data_ref"]
        with pytest.raises(worker.WorkerRequestError):
            worker.enforce_gnn_result_schema(payload)

    def test_missing_topology_scope_refused(self) -> None:
        payload = self._canonical()
        del payload["topology_scope"]
        with pytest.raises(worker.WorkerRequestError):
            worker.enforce_gnn_result_schema(payload)

    def test_result_kind_outside_vocabulary_refused(self) -> None:
        payload = self._canonical()
        payload["result_kind"] = "dispatch_schedule"
        with pytest.raises(worker.WorkerRequestError):
            worker.enforce_gnn_result_schema(payload)

    def test_result_kind_matches_mode_map(self) -> None:
        # The role/kind mapping stays inside the closed §13.4 vocabulary.
        assert worker.GNN_RESULT_KIND_BY_MODE["warm_start"] == "candidate_setpoint_vector"
        assert worker.GNN_RESULT_KIND_BY_MODE["screening"] == "ranking"
        for kind in worker.GNN_RESULT_KIND_BY_MODE.values():
            assert kind in worker.RESULT_KINDS


class TestTopologyScopeFlag:
    def test_in_scope_passes(self) -> None:
        result = worker.evaluate_topology_scope(
            "feeder-B", ["feeder-A", "feeder-B", "feeder-C"], "balanced-radial"
        )
        assert result["in_scope"] is True

    def test_out_of_scope_flagged_not_silently_answered(self) -> None:
        result = worker.evaluate_topology_scope(
            "feeder-UNSEEN", ["feeder-A", "feeder-B", "feeder-C"], "balanced-radial"
        )
        assert result["in_scope"] is False
        assert "NOT in the manifest" in result["reason"]

    def test_no_fuzzy_matching(self) -> None:
        # Exact membership only: a near-miss must flag, not pass.
        result = worker.evaluate_topology_scope(
            "feeder-A-v2", ["feeder-A"], "balanced-radial"
        )
        assert result["in_scope"] is False


class TestModelArtifactLoudFailure:
    def test_missing_model_file_fails_loudly(self) -> None:
        with pytest.raises(worker.ModelArtifactError) as excinfo:
            worker.check_model_artifact_path("models/gnn/never-trained.pt")
        assert "fails loudly" in str(excinfo.value)

    def test_empty_artifact_ref_fails_loudly(self) -> None:
        with pytest.raises(worker.ModelArtifactError):
            worker.check_model_artifact_path("")


class TestRankingPureFunction:
    def test_descending_order_hand_computed(self) -> None:
        ranking = worker.rank_candidates(
            [
                {"candidate_id": "c1", "score": 0.9, "uncertainty": 0.05},
                {"candidate_id": "c2", "score": 0.1, "uncertainty": 0.30},
                {"candidate_id": "c3", "score": 0.5, "uncertainty": 0.12},
            ]
        )
        assert [entry["candidate_id"] for entry in ranking] == ["c1", "c3", "c2"]

    def test_deterministic_tie_break(self) -> None:
        tie = worker.rank_candidates(
            [
                {"candidate_id": "t2", "score": 0.5, "uncertainty": 0.1},
                {"candidate_id": "t1", "score": 0.5, "uncertainty": 0.1},
            ]
        )
        assert [entry["candidate_id"] for entry in tie] == ["t1", "t2"]

    def test_negative_uncertainty_refused(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.rank_candidates(
                [{"candidate_id": "c1", "score": 0.9, "uncertainty": -0.1}]
            )

    def test_duplicate_candidate_id_refused(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.rank_candidates(
                [
                    {"candidate_id": "c1", "score": 0.9, "uncertainty": 0.1},
                    {"candidate_id": "c1", "score": 0.4, "uncertainty": 0.1},
                ]
            )

    def test_non_finite_score_refused(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.rank_candidates(
                [{"candidate_id": "c1", "score": float("inf"), "uncertainty": 0.1}]
            )

    def test_empty_candidate_list_refused(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.rank_candidates([])


class TestResultEnvelope:
    def test_fixed_posture(self) -> None:
        envelope = worker.build_result(
            validated=worker.validate_request(_valid_request()),
            termination_status="completed",
            outputs={"warm_start_vector": [0.0], "result_payload": {}},
            diagnostics=[],
            torch_version=None,
            pyg_version=None,
            execution_ms=0,
        )
        assert envelope["proposal_only"] is True
        assert envelope["authority"] == "none"
        assert envelope["control_authority_mode_locked"] == worker.MODE_LOCKED
        assert worker.MODE_LOCKED == "simulation-shadow"
        assert len(envelope["boundary_statements"]) >= 5
        assert envelope["ok"] is True

    def test_warm_start_speed_not_optimality_statement_present(self) -> None:
        # The no-optimality-claim statement travels verbatim in every
        # envelope: a warm start affects SPEED of an exact solve, never
        # its optimality, and the solver still solves.
        envelope = worker.build_result(
            validated=worker.validate_request(_valid_request()),
            termination_status="failed",
            outputs=None,
            diagnostics=[],
            torch_version=None,
            pyg_version=None,
            execution_ms=0,
        )
        statements = " ".join(envelope["boundary_statements"])
        assert "the solver still solves" in statements
        assert "never its optimality" in statements
        assert "HiGHS is LP/QP/MIP" in statements
        assert "DECISION-POINT" in statements

    def test_provenance_field_names_match_template(self) -> None:
        # §9.2 field names, verbatim from the Phase 7 worker template.
        envelope = worker.build_result(
            validated=worker.validate_request(_valid_request()),
            termination_status="failed",
            outputs=None,
            diagnostics=["inference failed in-band: recorded reason"],
            torch_version=None,
            pyg_version=None,
            execution_ms=5,
        )
        for field in (
            "worker_version",
            "model_version",
            "solver_version",
            "input_snapshot_id",
            "termination_status",
            "diagnostics",
            "execution_ms",
        ):
            assert field in envelope["provenance"]
        assert envelope["provenance"]["termination_status"] == "failed"
        assert envelope["ok"] is False
        assert envelope["provenance"]["execution_ms"] == 5

    def test_model_scope_carries_p811_provenance(self) -> None:
        validated = worker.validate_request(_valid_request())
        envelope = worker.build_result(
            validated=validated,
            termination_status="failed",
            outputs=None,
            diagnostics=[],
            torch_version=None,
            pyg_version=None,
            execution_ms=0,
        )
        # P8.11 step 3: model version, training-data reference, input
        # snapshot ID, topology scope — on every output.
        assert envelope["model_scope"]["model_version"] == validated["model_manifest"]["model_version"]
        assert envelope["model_scope"]["training_data_ref"] == validated["model_manifest"]["training_data_ref"]
        assert envelope["model_scope"]["topology_scope"] == validated["model_manifest"]["topology_scope"]
        assert envelope["provenance"]["input_snapshot_id"] == validated["input_snapshot_id"]
        assert envelope["model_scope"]["framework_pin_status"] == worker.GNN_PIN_STATUS

    def test_no_measured_figures_in_envelope(self) -> None:
        # ENTRY_NOTE boundary 4: no fabricated accuracy/latency/speedup.
        envelope = worker.build_result(
            validated=worker.validate_request(_valid_request()),
            termination_status="failed",
            outputs=None,
            diagnostics=[],
            torch_version=None,
            pyg_version=None,
            execution_ms=0,
        )
        text = json.dumps(envelope)
        for banned in ("accuracy", "speedup", "mae", "rmse", "hit_rate"):
            assert banned not in text.lower()


class TestInferencePath:
    def _torch_importable(self) -> bool:
        try:
            import torch  # noqa: F401,PLC0415 — probe only

            return True
        except ImportError:
            return False

    def test_out_of_scope_flagged_without_framework(self) -> None:
        # The scope gate runs BEFORE any framework import: the flagged
        # envelope is producible with NOTHING installed (this machine).
        request = _valid_request()
        request["topology"]["topology_id"] = "feeder-UNSEEN"
        result = worker.run_inference(request)
        assert result["ok"] is False
        assert result["provenance"]["termination_status"] == "failed"
        assert result["outputs"] is None
        assert result["topology_scope_check"]["in_scope"] is False
        assert result["topology_scope_check"]["topology_id"] == "feeder-UNSEEN"
        worker.assert_proposal_only(result)  # the flagged envelope is clean

    def test_missing_torch_raises_the_named_decision_point(self) -> None:
        if self._torch_importable():
            pytest.skip(
                "torch is installed on this machine; the named-requirement "
                "test only applies to environments without it (the run-"
                "path tests belong to the human build/test pass after the "
                "verified torch/PyG pair is pinned per P8.03 §4)"
            )
        with pytest.raises(ImportError) as excinfo:
            worker.run_inference(_valid_request())
        assert worker.TORCH_DEPLOYMENT_REQUIREMENT in str(excinfo.value)
        assert "DECISION-POINT" in str(excinfo.value)

    def test_module_imports_without_any_framework(self) -> None:
        # The honesty gate itself: the pure-logic core is importable with
        # no torch/PyG present (the deferred imports are the mechanism).
        if self._torch_importable():
            pytest.skip("torch installed; the gate is trivially satisfied")
        assert callable(worker.validate_request)
        assert callable(worker.build_result)
        assert callable(worker.assert_proposal_only)
        assert callable(worker.run_inference)

    def test_framework_pin_status_is_the_decision_point(self) -> None:
        assert "DECISION-POINT OPEN" in worker.GNN_PIN_STATUS
        assert "no torch/PyG pair is selected" in worker.GNN_PIN_STATUS


class TestFailClosedMain:
    def _run_main(self, stdin_text: str, capsys) -> int:
        monkeypatch_stdin = io.StringIO(stdin_text)
        saved_stdin = sys.stdin
        saved_stdout = sys.stdout
        sys.stdin = monkeypatch_stdin
        sys.stdout = io.StringIO()
        try:
            return worker.main()
        finally:
            captured_stdout = sys.stdout.getvalue()
            sys.stdin = saved_stdin
            sys.stdout = saved_stdout
            capsys.readouterr()  # drain pytest's own capture
            self._last_stdout = captured_stdout

    def test_malformed_json_exit_2_no_stdout(self, capsys) -> None:
        code = self._run_main("{not json", capsys)
        assert code == 2
        assert self._last_stdout == ""

    def test_wrong_protocol_exit_2_no_stdout(self, capsys) -> None:
        request = _valid_request()
        request["protocol"] = "something-else/1"
        code = self._run_main(json.dumps(request), capsys)
        assert code == 2
        assert self._last_stdout == ""

    def test_out_of_scope_yields_flagged_envelope_exit_0(self, capsys) -> None:
        # An out-of-scope topology is a RESULT (flagged in-band), not a
        # crash: exit 0 with the envelope on stdout.
        request = _valid_request()
        request["topology"]["topology_id"] = "feeder-UNSEEN"
        code = self._run_main(json.dumps(request), capsys)
        assert code == 0
        envelope = json.loads(self._last_stdout)
        assert envelope["provenance"]["termination_status"] == "failed"
        assert envelope["outputs"] is None
        assert envelope["topology_scope_check"]["in_scope"] is False

    def test_in_scope_without_torch_exit_2_no_stdout(self, capsys) -> None:
        try:
            import torch  # noqa: F401,PLC0415 — probe only

            pytest.skip(
                "torch installed; the named-requirement path is untestable here"
            )
        except ImportError:
            pass
        request = _valid_request()
        code = self._run_main(json.dumps(request), capsys)
        assert code == 2
        assert self._last_stdout == ""


class TestSelfTestsMirrored:
    def test_in_module_self_tests_entrypoint_present(self) -> None:
        # The pytest suite mirrors `run_self_tests()`; this pins that the
        # in-module entrypoint stays present for the no-pytest human gate.
        assert callable(worker.run_self_tests)
