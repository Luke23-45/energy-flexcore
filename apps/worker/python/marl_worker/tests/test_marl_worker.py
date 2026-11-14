# P8.14 — The MARL simulation/shadow worker test suite: the boundary tests,
#         the closed P8.04 schema, the fail-closed composition edge, and
#         the framework honesty gate — all runnable WITHOUT any framework.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. WRITTEN, NEVER RUN. pytest was never installed in this loop; no test
#    below has ever executed. Expected values are HAND-COMPUTED
#    derivations stated in the tests (never observed outputs). Run
#    instruction: `PYTHONPATH=apps/worker/python/marl_worker python -m
#    pytest apps/worker/python/marl_worker/tests -v`; the in-module
#    self-tests run WITHOUT pytest:
#    `python apps/worker/python/marl_worker/marl_worker.py`.
# 2. What this suite pins (ledger P8.14 acceptance criteria — the parts
#    provable without any framework):
#    - THE BOUNDARY TEST (step 4): an attempt to route a MARL action to
#      dispatch fails BY CONSTRUCTION — no dispatch/issue/authorize/
#      approve/command-shaped callable exists in the module namespace
#      (introspection), the module's AST imports stdlib + the two sibling
#      pure modules only (no transport, no dispatch client), a
#      dispatch-shaped permission is refused with a named outcome and NO
#      proposal, and a dispatch-shaped key anywhere in a result tree
#      refuses emission (assert_proposal_only).
#    - The shadow logger records EVERY action WITH its filter outcome
#      (mandatory, closed vocabulary) — the "actions but not filter
#      outcomes" failure mode is unrepresentable.
#    - The infeasible-filter outcome: fallback attempted, provably not
#      viable, RECORDED — never a silent unfiltered pass-through.
#    - The closed P8.04 MARL payload (result_kind, policy_version,
#      training_seed, environment_version, safety_filter_outcome) and the
#      envelope's fixed posture (proposal_only, authority none, mode lock,
#      PROHIBITED-not-deferred verbatim).
#    - The honesty gate: shadow_inference and run_training raise the NAMED
#      DECISION-POINT ImportError with nothing installed — no policy
#      output, no training result, EMPTY cells (skipped loudly on a
#      machine where ray IS installed — there, the guarded-path tests
#      belong to the human build/test pass).
#    - The fail-closed composition edge: malformed stdin → exit 2, nothing
#      on stdout.
# 3. NOTHING here proves anything about any policy's behaviour, and NO
#    reward curve, convergence statistic, win-rate, or violation
#    frequency exists anywhere in this suite (ENTRY_NOTE boundary 4).
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import ast
import inspect
import io
import json
import os
import sys

import pytest

sys.path.insert(
    0,
    os.path.join(os.path.dirname(os.path.abspath(__file__)), os.pardir),
)

import marl_worker as worker  # noqa: E402 — path set above
import safety_filter as sf  # noqa: E402 — path set above
import shadow_log as sl  # noqa: E402 — path set above


def _limits(p_min: float, p_max: float, ramp: float) -> dict:
    return {"p_min_mw": p_min, "p_max_mw": p_max, "ramp_mw_per_interval": ramp}


def _valid_request(**overrides) -> dict:
    """The canonical valid filter_probe request (CONSTRUCTED test inputs,
    never measurements)."""
    request = {
        "protocol": worker.PROTOCOL_ID,
        "mode": "filter_probe",
        "policy_context": {
            # training_seed 0 = the documented no-training-provenance
            # convention, declared by the CALLER (the worker never
            # invents a seed).
            "policy_version": "RECORD-AT-FIRST-RUN",
            "training_seed": 0,
            "environment_version": "flexcore-marl-env/0.1.0",
        },
        "action": {
            "action_kind": "setpoint_delta",
            "proposed_permission": "schedule.propose",
            "assets": [
                {
                    "asset_id": "ess-1",
                    "p_current_mw": 2.0,
                    "p_proposed_mw": 7.0,
                    "soc_fraction": 0.5,
                    "limits": _limits(0.0, 5.0, 1.5),
                }
            ],
        },
        "provenance": {
            "input_snapshot_id": "snapshot-RECORD",
            "request_id": "req-0001",
        },
    }
    request.update(overrides)
    return request


def _shadow_request() -> dict:
    """The canonical valid shadow_inference request (the artifact ref
    names a file that does not exist and can never exist here)."""
    request = _valid_request(mode="shadow_inference")
    del request["action"]
    request["policy_manifest"] = {
        "policy_version": "RECORD-AT-FIRST-RUN",
        "policy_artifact_ref": "models/marl/never-trained-policy",
        "training_data_ref": "object-store:training-data/RECORD-REF",
        "observation_scope": "balanced-radial-feeder/RECORD-CLASS",
    }
    return request


class TestFilterProbePath:
    def test_clamped_action_completes_with_filtered_setpoint(self) -> None:
        # HAND CHECK: p_current 2.0, ramp 1.5, limits [0, 5] -> interval
        # [0.5, 3.5]; proposed 7.0 clamps to 3.5 (the ramp binds).
        envelope = worker.run_filter_probe(_valid_request())
        assert envelope["ok"] is True
        assert envelope["provenance"]["termination_status"] == "completed"
        assert envelope["filter_outcome"] == "filtered_clamped_to_limits"
        setpoints = envelope["outputs"]["proposal"]["setpoints"]
        assert setpoints[0]["p_filtered_mw"] == 3.5
        assert setpoints[0]["p_proposed_mw"] == 7.0

    def test_payload_matches_closed_p804_schema(self) -> None:
        envelope = worker.run_filter_probe(_valid_request())
        payload = envelope["outputs"]["result_payload"]
        assert set(payload) == set(worker.MARL_RESULT_FIELDS)
        worker.enforce_marl_result_schema(payload)
        assert payload["result_kind"] == "candidate_setpoint_vector"
        assert payload["safety_filter_outcome"] == "filtered_clamped_to_limits"
        assert payload["policy_version"] == "RECORD-AT-FIRST-RUN"
        assert payload["training_seed"] == 0
        assert payload["environment_version"] == "flexcore-marl-env/0.1.0"

    def test_shadow_log_records_action_outcome_and_consequence(self) -> None:
        envelope = worker.run_filter_probe(_valid_request())
        log = envelope["outputs"]["shadow_log"]
        sl.validate_shadow_log(log)
        entry = log[0]
        assert entry["safety_filter_outcome"] == "filtered_clamped_to_limits"
        assert len(entry["filter_activations"]) == 1
        assert entry["consequence_status"] == "not_simulated"
        assert entry["simulated_consequence"] is None
        assert entry["lifecycle_state"] == "PROPOSED"
        assert entry["proposal_only"] is True
        assert envelope["outputs"]["shadow_log_summary"]["total_actions"] == 1
        assert envelope["outputs"]["shadow_log_summary"][
            "filter_activation_count"
        ] == 1

    def test_permission_refusal_is_in_band_with_no_proposal(self) -> None:
        request = _valid_request()
        request["action"]["proposed_permission"] = "dispatch.issue"
        envelope = worker.run_filter_probe(request)
        assert envelope["ok"] is False
        assert envelope["provenance"]["termination_status"] == "failed"
        assert envelope["filter_outcome"] == "refused_permission_ceiling"
        assert envelope["outputs"]["proposal"] is None
        assert envelope["outputs"]["result_payload"] is None
        entry = envelope["outputs"]["shadow_log"][0]
        assert entry["safety_filter_outcome"] == "refused_permission_ceiling"
        assert envelope["outputs"]["shadow_log_summary"]["refusal_count"] == 1

    def test_infeasible_filter_recorded_never_silent(self) -> None:
        # HAND CHECK: p_current 3.0 outside [1.0, 2.0], ramp 0.5 ->
        # interval [max(1.0, 2.5), min(2.0, 3.5)] = [2.5, 2.0] EMPTY;
        # fallback (hold 3.0) provably outside [1.0, 2.0] -> not viable.
        request = _valid_request()
        request["action"]["assets"][0] = {
            "asset_id": "ess-1",
            "p_current_mw": 3.0,
            "p_proposed_mw": 3.0,
            "soc_fraction": None,
            "limits": _limits(1.0, 2.0, 0.5),
        }
        envelope = worker.run_filter_probe(request)
        assert envelope["ok"] is False
        assert envelope["filter_outcome"] == "infeasible_no_safe_projection"
        assert envelope["outputs"]["proposal"] is None
        report = envelope["outputs"]["filter_report"]
        assert report["fallback"]["attempted"] is True
        assert report["fallback"]["viable"] is False
        entry = envelope["outputs"]["shadow_log"][0]
        assert entry["infeasibility"] is not None
        assert entry["infeasibility"]["recorded"] is True
        assert envelope["outputs"]["shadow_log_summary"][
            "infeasibility_count"
        ] == 1

    def test_nonfinite_proposal_refused_in_band(self) -> None:
        request = _valid_request()
        request["action"]["assets"][0]["p_proposed_mw"] = float("nan")
        envelope = worker.run_filter_probe(request)
        assert envelope["ok"] is False
        assert envelope["filter_outcome"] == "refused_nonfinite_action"
        assert envelope["outputs"]["proposal"] is None

    def test_clean_hold_action_passes(self) -> None:
        request = _valid_request()
        request["action"]["action_kind"] = "hold_current_setpoint"
        request["action"]["assets"][0]["p_proposed_mw"] = 2.0
        envelope = worker.run_filter_probe(request)
        assert envelope["ok"] is True
        assert envelope["filter_outcome"] == "passed_within_limits"
        assert envelope["outputs"]["filter_report"]["filter_activations"] == []


class TestEnvelopePosture:
    def test_fixed_posture(self) -> None:
        envelope = worker.run_filter_probe(_valid_request())
        assert envelope["protocol"] == worker.PROTOCOL_ID
        assert envelope["kind"] == "marl_shadow_result"
        assert envelope["proposal_only"] is True
        assert envelope["authority"] == "none"
        assert envelope["control_authority_mode_locked"] == "simulation-shadow"
        assert worker.MODE_LOCKED == "simulation-shadow"
        assert envelope["payload_type"] == "flexcore.research.marl_shadow.v1"

    def test_prohibited_not_deferred_statement_verbatim(self) -> None:
        envelope = worker.run_filter_probe(_valid_request())
        assert any(
            "PROHIBITED, not deferred" in statement
            for statement in envelope["boundary_statements"]
        )
        assert any(
            "no code path to the dispatch gateway" in statement
            for statement in envelope["boundary_statements"]
        )

    def test_finite_test_set_statement_verbatim(self) -> None:
        envelope = worker.run_filter_probe(_valid_request())
        assert any(
            "not a universal safety guarantee" in statement
            for statement in envelope["boundary_statements"]
        )

    def test_cbf_not_yet_derived_statement_verbatim(self) -> None:
        envelope = worker.run_filter_probe(_valid_request())
        assert any(
            "NOT YET DERIVED" in statement and "TRAINING IS BLOCKED" in statement
            for statement in envelope["boundary_statements"]
        )

    def test_training_status_is_empty(self) -> None:
        envelope = worker.run_filter_probe(_valid_request())
        assert envelope["policy_scope"]["training_status"].startswith("EMPTY")

    def test_mode_lock_not_request_selectable(self) -> None:
        request = _valid_request()
        request["control_authority_mode"] = "operator-approved"
        envelope = worker.run_filter_probe(request)
        assert envelope["control_authority_mode_locked"] == "simulation-shadow"

    def test_envelope_passes_proposal_only_scan(self) -> None:
        for request in (_valid_request(),):
            envelope = worker.run_filter_probe(request)
            worker.assert_proposal_only(envelope)  # must not raise

    def test_provenance_names_verbatim_92(self) -> None:
        envelope = worker.run_filter_probe(_valid_request())
        provenance = envelope["provenance"]
        for field in ("worker_version", "model_version", "solver_version",
                      "input_snapshot_id", "request_id", "termination_status",
                      "diagnostics", "execution_ms"):
            assert field in provenance
        assert provenance["worker_version"] == worker.WORKER_VERSION
        assert provenance["input_snapshot_id"] == "snapshot-RECORD"
        assert provenance["request_id"] == "req-0001"
        assert provenance["execution_ms"] == 0


class TestProposalOnlyBoundary:
    def test_nested_dispatch_key_refuses_emission(self) -> None:
        deep = {"outputs": {"shadow_log": [{"sequence": 1, "dispatch": {"p": 1.0}}]}}
        with pytest.raises(RuntimeError):
            worker.assert_proposal_only(deep)

    def test_nested_issued_command_refuses_emission(self) -> None:
        deep = {"outputs": {"proposal": {"setpoints": [{"issued": True}]}}}
        with pytest.raises(RuntimeError):
            worker.assert_proposal_only(deep)

    def test_clean_tree_passes(self) -> None:
        worker.assert_proposal_only(
            {"outputs": {"shadow_log": [{"sequence": 1}]}}
        )

    def test_forbidden_keys_include_dispatch_shapes(self) -> None:
        assert worker.FORBIDDEN_RESULT_KEYS >= {
            "dispatch", "dispatchable", "command", "command_id", "issued",
            "issue", "authorization", "authorization_token", "approval",
        }


class TestClosedResultSchema:
    CANONICAL = {
        "result_kind": "candidate_setpoint_vector",
        "policy_version": "RECORD-AT-FIRST-RUN",
        "training_seed": 0,
        "environment_version": "flexcore-marl-env/0.1.0",
        "safety_filter_outcome": "filtered_clamped_to_limits",
    }

    def test_canonical_passes(self) -> None:
        worker.enforce_marl_result_schema(dict(self.CANONICAL))

    def test_unknown_key_refused(self) -> None:
        smuggled = dict(self.CANONICAL)
        smuggled["dispatch_vector"] = [1.0]
        with pytest.raises(worker.WorkerRequestError):
            worker.enforce_marl_result_schema(smuggled)

    def test_outcome_outside_vocabulary_refused(self) -> None:
        wrong = dict(self.CANONICAL)
        wrong["safety_filter_outcome"] = "mostly_fine"
        with pytest.raises(worker.WorkerRequestError):
            worker.enforce_marl_result_schema(wrong)

    def test_missing_filter_outcome_refused(self) -> None:
        wrong = dict(self.CANONICAL)
        del wrong["safety_filter_outcome"]
        with pytest.raises(worker.WorkerRequestError):
            worker.enforce_marl_result_schema(wrong)

    def test_bool_seed_refused(self) -> None:
        wrong = dict(self.CANONICAL)
        wrong["training_seed"] = True
        with pytest.raises(worker.WorkerRequestError):
            worker.enforce_marl_result_schema(wrong)

    def test_result_kind_outside_vocabulary_refused(self) -> None:
        wrong = dict(self.CANONICAL)
        wrong["result_kind"] = "dispatch_schedule"
        with pytest.raises(worker.WorkerRequestError):
            worker.enforce_marl_result_schema(wrong)

    def test_empty_policy_version_refused(self) -> None:
        wrong = dict(self.CANONICAL)
        wrong["policy_version"] = ""
        with pytest.raises(worker.WorkerRequestError):
            worker.enforce_marl_result_schema(wrong)


class TestRequestRefusals:
    def test_wrong_protocol_refused(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(_valid_request(protocol="flexcore-marl-worker/0"))

    def test_unknown_mode_refused(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(_valid_request(mode="live_dispatch"))

    def test_caller_injected_action_in_shadow_mode_refused(self) -> None:
        # A caller-injected action would BYPASS the policy: refused —
        # the P8.14 boundary, not a convenience.
        request = _shadow_request()
        request["action"] = _valid_request()["action"]
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(request)

    def test_missing_action_in_filter_probe_refused(self) -> None:
        request = _valid_request()
        del request["action"]
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(request)

    def test_missing_policy_context_refused(self) -> None:
        request = _valid_request()
        del request["policy_context"]
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(request)

    def test_missing_environment_version_refused(self) -> None:
        request = _valid_request()
        del request["policy_context"]["environment_version"]
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(request)

    def test_bool_seed_refused(self) -> None:
        request = _valid_request()
        request["policy_context"]["training_seed"] = True
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(request)

    def test_inverted_limits_normalized_to_refusal(self) -> None:
        request = _valid_request()
        request["action"]["assets"][0]["limits"] = _limits(5.0, 1.0, 1.0)
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(request)

    def test_missing_provenance_refused(self) -> None:
        request = _valid_request()
        del request["provenance"]
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(request)


class TestPolicyArtifactLoudFailure:
    def test_missing_file_raises_named_error(self) -> None:
        with pytest.raises(worker.PolicyArtifactError):
            worker.check_policy_artifact_path("models/marl/never-trained-policy")

    def test_empty_ref_raises_named_error(self) -> None:
        with pytest.raises(worker.PolicyArtifactError):
            worker.check_policy_artifact_path("")

    def test_object_store_ref_passes_pure_check(self) -> None:
        # The object-store resolution is the P8.20 activation seam
        # (VERIFY-BUILD item); the pure check passes it through.
        assert (
            worker.check_policy_artifact_path("object-store:policies/RECORD-REF")
            == "object-store:policies/RECORD-REF"
        )


class TestFrameworkHonestyGate:
    @pytest.fixture(autouse=True)
    def _skip_if_framework_installed(self) -> None:
        try:
            import ray  # noqa: F401,PLC0415 — probe only
        except ImportError:
            return  # expected everywhere in this repository
        pytest.skip(
            "ray IS installed on this machine — the guarded-path honesty "
            "tests belong to the human build/test pass after the "
            "DECISION-POINT is resolved (P8.03 section 2)"
        )

    def test_shadow_inference_raises_named_requirement(self) -> None:
        with pytest.raises(ImportError) as excinfo:
            worker.run_shadow_inference(_shadow_request())
        message = str(excinfo.value)
        assert worker.RAY_RLLIB_DEPLOYMENT_REQUIREMENT in message
        assert "DECISION-POINT" in message

    def test_training_raises_named_requirement(self) -> None:
        with pytest.raises(ImportError) as excinfo:
            worker.run_training({})
        message = str(excinfo.value)
        assert worker.RAY_RLLIB_DEPLOYMENT_REQUIREMENT in message

    def test_import_error_names_gymnasium_after_ray(
        self, monkeypatch: pytest.MonkeyPatch
    ) -> None:
        # With ray monkeypatched to "exist", the NEXT gate (gymnasium)
        # still refuses with ITS named requirement — the gates are
        # ordered and each names its own deployment requirement.
        import builtins
        import types

        fake_ray = types.ModuleType("ray")
        fake_ray.__version__ = "not-a-real-install"
        original_import = builtins.__import__

        def fake_import(name, *args, **kwargs):
            if name == "ray":
                return fake_ray
            return original_import(name, *args, **kwargs)

        monkeypatch.setattr(builtins, "__import__", fake_import)
        with pytest.raises(ImportError) as excinfo:
            worker.run_shadow_inference(_shadow_request())
        assert worker.GYMNASIUM_DEPLOYMENT_REQUIREMENT in str(excinfo.value)


class TestDispatchBoundary:
    """Ledger P8.14 step 4: attempt to route a MARL action to dispatch and
    confirm there is NO code path that allows it (verified by inspection
    + test)."""

    DISPATCH_TOKENS = ("dispatch", "issue", "authorize", "approve", "command")

    def test_no_dispatch_shaped_callable_in_module_namespace(self) -> None:
        offending = [
            name
            for name, value in vars(worker).items()
            if callable(value)
            and not name.startswith("_")
            and any(token in name.lower() for token in self.DISPATCH_TOKENS)
        ]
        assert offending == [], offending

    def test_no_dispatch_shaped_callable_in_sibling_modules(self) -> None:
        for module in (sf, sl):
            offending = [
                name
                for name, value in vars(module).items()
                if callable(value)
                and not name.startswith("_")
                and any(token in name.lower() for token in self.DISPATCH_TOKENS)
            ]
            assert offending == [], (module.__name__, offending)

    def test_module_ast_imports_stdlib_and_siblings_only(self) -> None:
        source = inspect.getsource(worker)
        tree = ast.parse(source)
        allowed_roots = {
            "__future__", "json", "os", "sys", "ast", "inspect",
            "safety_filter", "shadow_log",
        }
        top_level_roots: list = []
        for node in tree.body:
            if isinstance(node, ast.Import):
                top_level_roots.extend(
                    alias.name.split(".")[0] for alias in node.names
                )
            elif isinstance(node, ast.ImportFrom) and node.module:
                top_level_roots.append(node.module.split(".")[0])
        assert top_level_roots, "no top-level imports found (parse failed?)"
        assert all(root in allowed_roots for root in top_level_roots)

    def test_module_ast_guarded_imports_are_framework_gates_only(self) -> None:
        source = inspect.getsource(worker)
        tree = ast.parse(source)
        guarded_roots: list = []
        for node in ast.walk(tree):
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
                for inner in ast.walk(node):
                    if isinstance(inner, ast.Import):
                        guarded_roots.extend(
                            alias.name.split(".")[0] for alias in inner.names
                        )
                    elif isinstance(inner, ast.ImportFrom) and inner.module:
                        guarded_roots.append(inner.module.split(".")[0])
        assert set(guarded_roots) <= {"ray", "gymnasium"}

    def test_dispatch_permission_refusal_emits_no_proposal(self) -> None:
        # "Attempt to route a MARL action to dispatch": the closest expressible
        # attempt is a dispatch-shaped permission on the action — refused with
        # a named outcome, zero entries projected, NO proposal in the envelope.
        request = _valid_request()
        request["action"]["proposed_permission"] = "dispatch.issue"
        envelope = worker.run_filter_probe(request)
        assert envelope["outputs"]["proposal"] is None
        assert envelope["filter_outcome"] == "refused_permission_ceiling"
        worker.assert_proposal_only(envelope)

    def test_every_closed_vocabulary_lacks_dispatch_shape(self) -> None:
        vocabularies = (
            sf.FILTER_OUTCOMES,
            sf.ENTRY_OUTCOMES,
            sf.ACTION_KINDS,
            worker.RESULT_KINDS,
            worker.MARL_RESULT_FIELDS,
            sl.LOG_ENTRY_FIELDS,
        )
        for vocabulary in vocabularies:
            assert not any(
                token in str(name).lower()
                for name in vocabulary
                for token in ("dispatch", "issue", "authoriz", "command")
            )

    def test_run_worker_mode_switch_only(self) -> None:
        # run_worker dispatches on the WORKER MODE only — there is no
        # dispatch-target parameter and no function that takes one.
        assert worker.run_worker(_valid_request())["mode"] == "filter_probe"


class TestFailClosedMain:
    def _run_main(self, stdin_text: str):
        original_stdin, original_stdout = sys.stdin, sys.stdout
        sys.stdin = io.StringIO(stdin_text)
        captured = io.StringIO()
        sys.stdout = captured
        try:
            code = worker.main()
        finally:
            sys.stdin, sys.stdout = original_stdin, original_stdout
        return code, captured.getvalue()

    def test_malformed_json_exit_2_nothing_on_stdout(self) -> None:
        code, out = self._run_main("this is not json")
        assert code == 2
        assert out == ""

    def test_wrong_protocol_exit_2_nothing_on_stdout(self) -> None:
        request = _valid_request()
        request["protocol"] = "flexcore-marl-worker/0"
        code, out = self._run_main(json.dumps(request))
        assert code == 2
        assert out == ""

    def test_refused_request_exit_2_nothing_on_stdout(self) -> None:
        # A caller-injected action in shadow mode is a REFUSAL (exit 2),
        # not an in-band result.
        request = _shadow_request()
        request["action"] = _valid_request()["action"]
        code, out = self._run_main(json.dumps(request))
        assert code == 2
        assert out == ""

    def test_accepted_probe_emits_exactly_one_document(self) -> None:
        code, out = self._run_main(json.dumps(_valid_request()))
        assert code == 0
        documents = [line for line in out.splitlines() if line.strip()]
        assert len(documents) == 1
        envelope = json.loads(documents[0])
        assert envelope["protocol"] == worker.PROTOCOL_ID
        assert envelope["ok"] is True
        worker.assert_proposal_only(envelope)

    def test_refused_action_emits_in_band_document_exit_0(self) -> None:
        request = _valid_request()
        request["action"]["proposed_permission"] = "dispatch.issue"
        code, out = self._run_main(json.dumps(request))
        assert code == 0
        documents = [line for line in out.splitlines() if line.strip()]
        assert len(documents) == 1
        envelope = json.loads(documents[0])
        assert envelope["ok"] is False
        assert envelope["filter_outcome"] == "refused_permission_ceiling"


class TestConstants:
    def test_protocol_and_version(self) -> None:
        assert worker.PROTOCOL_ID == "flexcore-marl-worker/1"
        assert worker.WORKER_VERSION == "flexcore-marl-worker/0.1.0"

    def test_worker_modes_closed(self) -> None:
        assert worker.WORKER_MODES == frozenset({"filter_probe", "shadow_inference"})

    def test_payload_type_is_p804_marl(self) -> None:
        assert worker.PAYLOAD_TYPE == "flexcore.research.marl_shadow.v1"

    def test_marl_result_fields_match_p804(self) -> None:
        assert worker.MARL_RESULT_FIELDS == frozenset(
            {
                "result_kind",
                "policy_version",
                "training_seed",
                "environment_version",
                "safety_filter_outcome",
            }
        )

    def test_pin_status_is_decision_point_open(self) -> None:
        assert "DECISION-POINT OPEN" in worker.MARL_PIN_STATUS
        assert "UNPINNED-REVERIFY" in worker.MARL_PIN_STATUS

    def test_unverified_note_states_never_executed(self) -> None:
        assert "never executed" in worker.UNVERIFIED_NOTE


class TestSelfTestRunner:
    def test_in_module_self_tests_all_pass(self) -> None:
        # The hand-computed self-tests are runnable WITHOUT pytest
        # (`python apps/worker/python/marl_worker/marl_worker.py`); NEVER
        # RUN in the writing loop — the human gate runs it first.
        records = worker.run_self_tests()
        assert records, "the self-test runner returned no records"
        failures = [r for r in records if not r["passed"]]
        assert failures == [], failures
