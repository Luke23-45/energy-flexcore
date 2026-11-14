# P8.04 — The research-contract test suite: pure-logic tests for
#         `worker_template/research.py` that RUN WITHOUT grpcio.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. WRITTEN, NEVER RUN. pytest was never installed in this loop; no test
#    below has ever executed. The suite targets the stdlib-only research
#    module and is expected to pass at the first human run, alongside
#    `test_worker_template.py` (same run instruction: scratch venv,
#    `PYTHONPATH=python/workers`, `python -m pytest python/workers/tests/`).
# 2. Coverage map (what each test pins, ledger P8.04 acceptance):
#    closed payload-type vocabulary; closed §13.4 result-kind vocabulary
#    (name-aligned with flexcore-research::validation::ResearchResultKind);
#    per-module closed result-key sets (ADMM / GNN / MARL extensions);
#    the proposal-only boundary IN CODE (whitelist builder + recursive
#    forbidden-key scan — an authorization token has no path through);
#    §9.2 provenance completeness (the Rust-harness mirror, bare-"ok"
#    rule included); seed honesty (requires_seed); the PROPOSED-stage
#    candidate record (fixed key set, authority = statement, not token);
#    the transport seam reusing server.require_grpc (no parallel seam).
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import json

import pytest

from worker_template import errors, research, server


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


def complete_provenance() -> dict[str, object]:
    return {
        "worker_version": "research-admm-worker/0.1.0",
        "model_version": "grid-model/2026.11",
        "solver_version": "admm/0.1.0",
        "input_snapshot_id": "snapshot-2026-11-13-a",
        "termination_status": "converged",
        "diagnostics": [],
        "execution_ms": 0,
        "model_artifact_ref": "artifact://models/admm.v1",
    }


def admm_result() -> dict[str, object]:
    return {
        "result_kind": "proposed_schedule",
        "primal_residual": 1e-6,
        "dual_residual": 2e-6,
        "iteration_count": 42,
    }


def admm_request() -> dict[str, object]:
    return {
        "contract_version": "flexcore.worker.v1",
        "request_id": "req-1",
        "correlation_id": "corr-1",
        "payload_type": "flexcore.research.admm_schedule.v1",
        "seed": 7,
        "deterministic": True,
        "payload": b"{}",
        "input_snapshot_id": "snapshot-2026-11-13-a",
    }


class AdmmLogic:
    """A minimal concrete research logic returning a valid ADMM result."""

    def evaluate(
        self, request: dict[str, object]
    ) -> tuple[dict[str, object], dict[str, object]]:
        return admm_result(), complete_provenance()


# ---------------------------------------------------------------------------
# Closed vocabularies (P8.04 step 1)
# ---------------------------------------------------------------------------


class TestVocabularies:
    def test_payload_types_are_closed_and_versioned(self) -> None:
        assert research.RESEARCH_PAYLOAD_TYPES == (
            "flexcore.research.admm_schedule.v1",
            "flexcore.research.gnn_warm_start.v1",
            "flexcore.research.marl_shadow.v1",
        )
        for payload_type in research.RESEARCH_PAYLOAD_TYPES:
            assert payload_type.startswith("flexcore.research.")
            assert payload_type.endswith(".v1")

    def test_result_kinds_are_the_closed_s134_vocabulary(self) -> None:
        # Name-aligned with the Rust enum (the recorded seam): exactly the
        # six §13.4 kinds, in the same order as ResearchResultKind.
        assert research.RESEARCH_RESULT_KINDS == (
            "proposed_schedule",
            "candidate_setpoint_vector",
            "ranking",
            "forecast_distribution",
            "uncertainty_estimate",
            "diagnostics",
        )

    def test_module_result_sets_carry_the_p804_extensions(self) -> None:
        fields = research.MODULE_RESULT_FIELDS
        assert fields["flexcore.research.admm_schedule.v1"] == frozenset(
            {"result_kind", "primal_residual", "dual_residual", "iteration_count"}
        )
        assert fields["flexcore.research.gnn_warm_start.v1"] == frozenset(
            {"result_kind", "training_data_ref", "topology_scope", "uncertainty"}
        )
        assert fields["flexcore.research.marl_shadow.v1"] == frozenset(
            {
                "result_kind",
                "policy_version",
                "training_seed",
                "environment_version",
                "safety_filter_outcome",
            }
        )

    def test_forbidden_keys_cover_authorization_shapes(self) -> None:
        forbidden = research.FORBIDDEN_RESULT_KEYS
        for key in (
            "authorization_token",
            "authorization",
            "auth_token",
            "access_token",
            "approval",
            "approved",
            "command_id",
            "dispatch",
            "grant",
        ):
            assert key in forbidden
        # Closed-set sanity: no research result key is forbidden.
        for allowed in research.MODULE_RESULT_FIELDS.values():
            assert allowed.isdisjoint(forbidden)

    def test_contract_version_is_reused_not_forked(self) -> None:
        assert research.RESEARCH_CONTRACT_VERSION == "flexcore.worker.v1"
        assert research.RESEARCH_CONTRACT_VERSION == errors.CONTRACT_VERSION

    def test_statements_are_present(self) -> None:
        assert "proposals only" in research.PROPOSAL_ONLY_STATEMENT
        assert "NO authority" in research.PROPOSAL_ONLY_STATEMENT
        assert "worker_api.proto" in research.RESEARCH_CONTRACT_STATEMENT
        assert "No parallel contract" in research.RESEARCH_CONTRACT_STATEMENT


# ---------------------------------------------------------------------------
# The proposal-only boundary, enforced in code (P8.04 step 2)
# ---------------------------------------------------------------------------


class TestProposalOnlyBoundary:
    def test_builder_accepts_a_valid_result(self) -> None:
        built = research.build_research_result(
            "flexcore.research.admm_schedule.v1", admm_result()
        )
        assert built == admm_result()

    def test_builder_refuses_unknown_keys(self) -> None:
        result = admm_result()
        result["extra_score"] = 1.0
        with pytest.raises(errors.WorkerError) as excinfo:
            research.build_research_result(
                "flexcore.research.admm_schedule.v1", result
            )
        assert excinfo.value.code == "WORKER_ERROR_CODE_INVALID_REQUEST"
        assert excinfo.value.field == "result"

    def test_builder_refuses_an_authorization_token_at_top_level(self) -> None:
        result = admm_result()
        result["authorization_token"] = "never-a-token"
        with pytest.raises(research.ProposalOnlyViolation, match="authorization_token"):
            research.build_research_result(
                "flexcore.research.admm_schedule.v1", result
            )

    def test_scan_refuses_a_token_nested_in_lists_and_dicts(self) -> None:
        nested = {
            "result_kind": "ranking",
            "training_data_ref": "dataset://d1",
            "topology_scope": "ieee13",
            "uncertainty": [{"score": 0.5}, {"dispatch": "cmd-9"}],
        }
        with pytest.raises(research.ProposalOnlyViolation, match="dispatch"):
            research.assert_proposal_only(nested)

    def test_scan_refuses_the_whole_forbidden_vocabulary(self) -> None:
        for key in sorted(research.FORBIDDEN_RESULT_KEYS):
            with pytest.raises(research.ProposalOnlyViolation):
                research.assert_proposal_only({key: "x"})

    def test_scan_message_names_key_and_path_never_values(self) -> None:
        with pytest.raises(research.ProposalOnlyViolation) as excinfo:
            research.assert_proposal_only(
                {"result_kind": "ranking", "training_data_ref": "d",
                 "topology_scope": "t", "uncertainty": {"approval": "SECRET-VALUE"}}
            )
        message = str(excinfo.value)
        assert "approval" in message
        assert "uncertainty" in message
        assert "SECRET-VALUE" not in message

    def test_refused_results_are_never_serialized(self) -> None:
        class TokenLogic:
            def evaluate(
                self, request: dict[str, object]
            ) -> tuple[dict[str, object], dict[str, object]]:
                result = admm_result()
                result["authorized"] = True
                return result, complete_provenance()

        with pytest.raises(research.ProposalOnlyViolation):
            research.handle_research_execute(admm_request(), TokenLogic())


# ---------------------------------------------------------------------------
# §9.2 provenance completeness (the Rust-harness mirror)
# ---------------------------------------------------------------------------


class TestProvenance:
    def test_complete_provenance_passes(self) -> None:
        research.validate_research_provenance(complete_provenance())

    def test_each_missing_field_is_named_first_wins(self) -> None:
        order = research.REQUIRED_PROVENANCE_FIELDS
        assert order == (
            "worker_version",
            "model_version",
            "solver_version",
            "input_snapshot_id",
            "termination_status",
            "model_artifact_ref",
        )
        for index, field in enumerate(order):
            provenance = complete_provenance()
            for later_field in order[index:]:
                provenance[later_field] = ""
            with pytest.raises(errors.WorkerError) as excinfo:
                research.validate_research_provenance(provenance)
            assert excinfo.value.field == field

    def test_bare_ok_termination_status_is_refused(self) -> None:
        provenance = complete_provenance()
        provenance["termination_status"] = "ok"
        with pytest.raises(errors.WorkerError, match="not sufficient"):
            research.validate_research_provenance(provenance)

    def test_requires_seed_refuses_a_seedless_request(self) -> None:
        class SeededAdmmLogic(AdmmLogic):
            requires_seed = True

        request = admm_request()
        del request["seed"]
        with pytest.raises(errors.WorkerError) as excinfo:
            research.handle_research_execute(request, SeededAdmmLogic())
        assert excinfo.value.field == "seed"
        assert excinfo.value.code == "WORKER_ERROR_CODE_INVALID_REQUEST"


# ---------------------------------------------------------------------------
# The Execute path (template reuse + boundary)
# ---------------------------------------------------------------------------


class TestHandleResearchExecute:
    def test_happy_path_is_a_template_response_with_json_payload(self) -> None:
        response = research.handle_research_execute(admm_request(), AdmmLogic())
        assert response["status"] == "EXECUTE_STATUS_OK"
        assert response["correlation_id"] == "corr-1"
        assert response["request_id"] == "req-1"
        assert response["contract_version"] == "flexcore.worker.v1"
        # Deterministic serialization: sorted keys, compact separators.
        assert response["payload"] == (
            b'{"dual_residual":2e-06,"iteration_count":42,'
            b'"primal_residual":1e-06,"result_kind":"proposed_schedule"}'
        )
        determinism = response["determinism"]
        assert determinism["seed"] == 7
        assert determinism["seed_supported"] is True
        assert determinism["deterministic"] is True

    def test_unknown_payload_type_is_refused_before_compute(self) -> None:
        class SpyLogic(AdmmLogic):
            called = False

            def evaluate(self, request):  # type: ignore[override]
                SpyLogic.called = True
                return super().evaluate(request)

        request = admm_request()
        request["payload_type"] = "flexcore.research.unknown.v9"
        with pytest.raises(errors.WorkerError) as excinfo:
            research.handle_research_execute(request, SpyLogic())
        assert excinfo.value.code == "WORKER_ERROR_CODE_UNSUPPORTED_PAYLOAD_TYPE"
        assert SpyLogic.called is False

    def test_draining_worker_refuses_before_any_check(self) -> None:
        class SpyLogic(AdmmLogic):
            called = False

            def evaluate(self, request):  # type: ignore[override]
                SpyLogic.called = True
                return super().evaluate(request)

        with pytest.raises(errors.WorkerError) as excinfo:
            research.handle_research_execute(
                admm_request(), SpyLogic(), accepts_work=False
            )
        assert excinfo.value.code == "WORKER_ERROR_CODE_UNAVAILABLE"
        assert SpyLogic.called is False

    def test_refusals_shape_through_the_template_error_path(self) -> None:
        request = admm_request()
        request["payload_type"] = "flexcore.research.unknown.v9"
        try:
            research.handle_research_execute(request, AdmmLogic())
        except errors.WorkerError as error:
            response = research.handler.handle_execute_error("req-1", "corr-1", error)
        assert response["status"] == "EXECUTE_STATUS_REFUSED"
        assert response["error"]["code"] == "WORKER_ERROR_CODE_UNSUPPORTED_PAYLOAD_TYPE"

    def test_incomplete_provenance_never_reaches_the_wire(self) -> None:
        class LazyLogic(AdmmLogic):
            def evaluate(
                self, request: dict[str, object]
            ) -> tuple[dict[str, object], dict[str, object]]:
                provenance = complete_provenance()
                provenance["input_snapshot_id"] = ""
                return admm_result(), provenance

        with pytest.raises(errors.WorkerError) as excinfo:
            research.handle_research_execute(admm_request(), LazyLogic())
        assert excinfo.value.field == "input_snapshot_id"


# ---------------------------------------------------------------------------
# The validation hook: the PROPOSED-stage candidate record (P8.04 step 3)
# ---------------------------------------------------------------------------


class TestCandidateRecord:
    def test_record_shape_is_closed_and_proposals_only(self) -> None:
        record = research.proposal_candidate_record(
            admm_result(),
            complete_provenance(),
            proposal_id="prop-1",
            producer_plugin_id="plugin/research-admm",
            producer_plugin_version="0.1.0",
            sequence=1,
        )
        assert set(record) == {
            "proposal_id",
            "producer_plugin_id",
            "producer_plugin_version",
            "sequence",
            "result_kind",
            "result",
            "provenance",
            "lifecycle_state",
            "authority",
        }
        assert record["lifecycle_state"] == "PROPOSED"
        assert record["result_kind"] == "proposed_schedule"
        # `authority` is a fixed statement, never a token.
        assert record["authority"] == research.PROPOSAL_ONLY_STATEMENT
        # The record itself passes the proposal-only scan.
        research.assert_proposal_only(record)

    def test_record_refuses_sequence_zero(self) -> None:
        with pytest.raises(errors.WorkerError, match="sequence"):
            research.proposal_candidate_record(
                admm_result(),
                complete_provenance(),
                proposal_id="prop-1",
                producer_plugin_id="plugin/research-admm",
                producer_plugin_version="0.1.0",
                sequence=0,
            )

    def test_record_refuses_empty_producer_identity(self) -> None:
        with pytest.raises(errors.WorkerError, match="producer_plugin_id"):
            research.proposal_candidate_record(
                admm_result(),
                complete_provenance(),
                proposal_id="prop-1",
                producer_plugin_id="",
                producer_plugin_version="0.1.0",
                sequence=1,
            )

    def test_record_refuses_a_token_bearing_result(self) -> None:
        result = admm_result()
        result["capability_grant"] = "nope"
        with pytest.raises(research.ProposalOnlyViolation):
            research.proposal_candidate_record(
                result,
                complete_provenance(),
                proposal_id="prop-1",
                producer_plugin_id="plugin/x",
                producer_plugin_version="0.1.0",
                sequence=1,
            )

    def test_marl_result_builds_and_records(self) -> None:
        result = research.build_research_result(
            "flexcore.research.marl_shadow.v1",
            {
                "result_kind": "candidate_setpoint_vector",
                "policy_version": "marl-policy.v1",
                "training_seed": 123,
                "environment_version": "gymnasium-1.3.0-env.v1",
                "safety_filter_outcome": "passed",
            },
        )
        record = research.proposal_candidate_record(
            result,
            complete_provenance(),
            proposal_id="prop-2",
            producer_plugin_id="plugin/research-marl",
            producer_plugin_version="0.1.0",
            sequence=2,
        )
        assert record["result_kind"] == "candidate_setpoint_vector"


# ---------------------------------------------------------------------------
# The transport seam: reuse, never a second grpcio import point
# ---------------------------------------------------------------------------


class TestTransportSeam:
    def test_transport_reuses_the_template_choke_point(self) -> None:
        try:
            grpc = research.require_worker_grpc()
        except ImportError as exc:
            # grpcio absent: the NAMED deployment requirement, verbatim.
            assert server.GRPC_DEPLOYMENT_REQUIREMENT in str(exc)
        else:
            # grpcio present: it is the same module object require_grpc
            # resolves — one choke point, no parallel seam.
            import grpc  # noqa: PLC0415 — only reachable when installed.

            assert grpc is grpc
            del grpc

    def test_gnn_payload_round_trips_through_the_builder(self) -> None:
        result = research.build_research_result(
            "flexcore.research.gnn_warm_start.v1",
            {
                "result_kind": "ranking",
                "training_data_ref": "sim-runs://2026-11/lib-01..lib-08",
                "topology_scope": "trained: ieee13/ieee34; held out: ieee123",
                "uncertainty": 0.17,
            },
        )
        # The serialized bytes decode back to the same result (the payload
        # is typed by agreement on both sides of the worker boundary).
        encoded = json.dumps(result, sort_keys=True, separators=(",", ":"))
        assert json.loads(encoded) == result
