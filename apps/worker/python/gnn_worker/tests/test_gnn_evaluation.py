# P8.12 — The GNN evaluation-protocol test suite: the pytest mirror of the
#         in-module hand-computed self-tests (`gnn_evaluation.py::
#         run_self_tests`). Pure logic only — NO framework, NO I/O.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. WRITTEN, NEVER RUN. pytest was never installed in this loop; no test
#    below has ever executed. Every expected value is HAND-COMPUTED
#    arithmetic over toy inputs, stated in the comments — the same
#    arithmetic the in-module self-tests pin. This is TEST ARITHMETIC
#    pinning the metric definitions: it is NOT a benchmark measurement,
#    NOT a seed result, and NOT evidence about any topology. NO accuracy,
#    latency, speedup, success-rate, ranking-quality, or miss-cost figure
#    from a real run exists anywhere in this suite (ENTRY_NOTE boundary 4;
#    the torch/PyG pair is a recorded DECISION-POINT, so no inference
#    output — and no end-to-end run — exists in this repository at all).
# 2. Run instruction (the recorded procedure):
#    `PYTHONPATH=apps/worker/python/gnn_worker python -m pytest
#    apps/worker/python/gnn_worker/tests -v` from the repository root.
#    The in-module suite runs WITHOUT pytest:
#    `python apps/worker/python/gnn_worker/gnn_evaluation.py`.
# 3. What this suite pins (ledger P8.12, the parts provable without any
#    framework or measured data): the end-to-end feasible-solve-time
#    definition (inference + solver + validation; the headline guard
#    refusing a missing validation component; NOT_A_SUCCESS for
#    infeasible/validation-failed runs), the speedup definition and its
#    non-positive-time refusals, the feasible-solver success rate, the
#    paired-by-topology speedups with failures recorded as failures and
#    unpaired successes listed (never silently dropped), the screening
#    metrics (precision@k, miss rate, recorded miss cost — the worker's
#    own deterministic ranking order), the held-out overlap/leakage check,
#    the leakage-control declarations, the contract-correct out-of-scope
#    flag on held-out topologies (the recorded Phase 7 follow-up), and the
#    carried VERBATIM statements (warm-start speed-never-optimality, the
#    headline rule, the UNCONFIRMED hold-out status, the X-8
#    no-thresholds rule, the DECISION-POINT status).
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import os
import sys

import pytest

sys.path.insert(
    0, os.path.join(os.path.dirname(__file__), "..")
)

import gnn_evaluation as ev  # noqa: E402
from gnn_evaluation import EvaluationError  # noqa: E402


@pytest.fixture()
def cold_run() -> dict:
    # solver 2.0 s + validation 0.5 s = 2.5 s end-to-end (no inference).
    return {
        "strategy": "cold_start",
        "topology_id": "t-cold",
        "solver_time_s": 2.0,
        "validation_time_s": 0.5,
        "feasible": True,
        "validation_passed": True,
    }


@pytest.fixture()
def gnn_run(cold_run: dict) -> dict:
    # inference 0.1 + solver 2.0 + validation 0.5 = 2.6 s end-to-end.
    return dict(cold_run, strategy="gnn_warm_start", topology_id="t-gnn",
                inference_time_s=0.1)


class TestEndToEndFeasibleSolveTime:
    def test_cold_run_is_solver_plus_validation(self, cold_run):
        # HAND-COMPUTED: 2.0 + 0.5 = 2.5.
        assert ev.end_to_end_feasible_time(cold_run) == 2.5

    def test_gnn_run_includes_inference_in_the_end_to_end_sum(self, gnn_run):
        # HAND-COMPUTED: 0.1 + 2.0 + 0.5 = 2.6 — the inference component
        # is included AND reported separately; the headline is the sum,
        # never the 0.1 s inference latency alone.
        assert ev.end_to_end_feasible_time(gnn_run) == 2.6

    def test_infeasible_run_has_no_end_to_end_time(self, gnn_run):
        with pytest.raises(EvaluationError, match="NOT_A_SUCCESS"):
            ev.end_to_end_feasible_time(dict(gnn_run, feasible=False))

    def test_solver_fixed_but_validation_failed_is_not_a_success(self, gnn_run):
        # "If the solver fixed it, that is the solver's success, not the
        # GNN's" — the ledger's named failure mode, refused here.
        with pytest.raises(EvaluationError, match="NOT_A_SUCCESS"):
            ev.end_to_end_feasible_time(dict(gnn_run, validation_passed=False))

    def test_missing_validation_component_is_refused(self, gnn_run):
        # The headline guard: without a recorded validation component the
        # table could not prove its headline is not inference latency.
        broken = dict(gnn_run)
        del broken["validation_time_s"]
        with pytest.raises(EvaluationError, match="must be a number"):
            ev.end_to_end_feasible_time(broken)

    def test_unknown_strategy_is_refused(self, gnn_run):
        with pytest.raises(EvaluationError, match="UNKNOWN_STRATEGY|not in the closed"):
            ev.end_to_end_feasible_time(dict(gnn_run, strategy="marl_policy"))

    def test_negative_component_is_refused(self, gnn_run):
        with pytest.raises(EvaluationError, match=">= 0"):
            ev.end_to_end_feasible_time(dict(gnn_run, solver_time_s=-1.0))


class TestFeasibleSolverSuccessRate:
    def test_two_of_four_is_half(self, cold_run, gnn_run):
        # HAND-COMPUTED: cold and gnn succeed; one infeasible GNN run and
        # one validation-failed cold run are failures → 2/4 = 0.5.
        runs = [
            cold_run,
            gnn_run,
            dict(gnn_run, topology_id="t-gnn2", feasible=False),
            dict(cold_run, topology_id="t-cold2", validation_passed=False),
        ]
        assert ev.feasible_success_rate(runs) == 0.5

    def test_empty_set_refused(self):
        with pytest.raises(EvaluationError, match="empty run set"):
            ev.feasible_success_rate([])


class TestSpeedup:
    def test_paired_ratio(self):
        # HAND-COMPUTED: 10.0 / 4.0 = 2.5; 4.0 / 10.0 = 0.4.
        assert ev.speedup(10.0, 4.0) == 2.5
        assert ev.speedup(4.0, 10.0) == 0.4

    def test_non_positive_times_refused(self):
        for baseline, treatment in ((0.0, 4.0), (10.0, 0.0), (-1.0, 4.0)):
            with pytest.raises(EvaluationError, match="finite and > 0"):
                ev.speedup(baseline, treatment)


class TestPairedSpeedups:
    def test_pairing_by_topology_with_failures_recorded(
        self, cold_run, gnn_run
    ):
        # HAND-SUMMED: cold t1 = 9.5 + 0.5 = 10.0; gnn t1 = 0.1 + 3.4 +
        # 0.5 = 4.0 → speedup 2.5. t2 is a GNN FAILURE (recorded as one,
        # never in the speedup table); t3 is a GNN success with no cold
        # pair (listed unpaired, never silently dropped).
        runs = [
            dict(cold_run, topology_id="t1", solver_time_s=9.5),
            dict(cold_run, topology_id="t2", solver_time_s=7.5),
            dict(gnn_run, topology_id="t1", solver_time_s=3.4),
            dict(gnn_run, topology_id="t2", feasible=False),
            dict(gnn_run, topology_id="t3", solver_time_s=1.4),
        ]
        paired = ev.paired_speedups(runs, "cold_start", "gnn_warm_start")
        assert paired["speedups"] == {"t1": 2.5}
        assert paired["treatment_failures"] == ["t2"]
        assert paired["unpaired_treatment_successes"] == ["t3"]

    def test_same_strategy_refused(self, cold_run):
        with pytest.raises(EvaluationError, match="against itself"):
            ev.paired_speedups([cold_run], "cold_start", "cold_start")

    def test_duplicate_topology_refused(self, cold_run):
        with pytest.raises(EvaluationError, match="duplicate"):
            ev.paired_speedups([cold_run, dict(cold_run)], "cold_start",
                               "gnn_warm_start")


SCORED = [
    {"candidate_id": "c1", "score": 0.9, "uncertainty": 0.1},
    {"candidate_id": "c2", "score": 0.8, "uncertainty": 0.2},
    {"candidate_id": "c3", "score": 0.7, "uncertainty": 0.3},
    {"candidate_id": "c4", "score": 0.6, "uncertainty": 0.2},
    {"candidate_id": "c5", "score": 0.5, "uncertainty": 0.4},
]


class TestScreeningMetrics:
    def test_hand_computed_confusion_at_depth_two(self):
        # True active {c1, c3}; top-2 screened in {c1, c2} → hits {c1},
        # false alarms {c2}, misses {c3}: precision@2 = 0.5,
        # miss rate = 0.5, miss cost = 7.0 (c3's RECORDED cost).
        metrics = ev.screening_metrics(SCORED, {"c1", "c3"}, 2,
                                       {"c3": 7.0, "c1": 3.0})
        assert metrics["ranking"] == ["c1", "c2", "c3", "c4", "c5"]
        assert metrics["screened_in"] == ["c1", "c2"]
        assert metrics["hits"] == ["c1"]
        assert metrics["false_alarms"] == ["c2"]
        assert metrics["misses"] == ["c3"]
        assert metrics["precision_at_screen_depth"] == 0.5
        assert metrics["miss_rate"] == 0.5
        assert metrics["miss_cost"] == 7.0

    def test_unrecorded_miss_cost_refused(self):
        # A missed constraint without a RECORDED cost is refused — never a
        # fabricated zero (the miss cost is a named P8.12 metric).
        with pytest.raises(EvaluationError, match="MISS_COST_UNRECORDED"):
            ev.screening_metrics(SCORED, {"c1", "c3"}, 2, {"c1": 3.0})

    def test_empty_true_active_refused(self):
        with pytest.raises(EvaluationError, match="EMPTY_TRUE_ACTIVE"):
            ev.screening_metrics(SCORED, set(), 2, {})

    def test_unknown_active_candidate_refused(self):
        with pytest.raises(EvaluationError, match="absent from the scored list"):
            ev.screening_metrics(SCORED, {"cX"}, 2, {"cX": 1.0})

    def test_ranking_uses_the_worker_deterministic_order(self):
        # A tie on score (c1/c2 both 0.8) breaks by candidate_id ascending
        # — the worker's own order, imported, never duplicated.
        tied = [
            {"candidate_id": "c2", "score": 0.8, "uncertainty": 0.1},
            {"candidate_id": "c1", "score": 0.8, "uncertainty": 0.1},
            {"candidate_id": "c3", "score": 0.5, "uncertainty": 0.1},
        ]
        metrics = ev.screening_metrics(tied, {"c1"}, 1, {"c1": 2.0})
        assert metrics["ranking"][0] == "c1"
        assert metrics["misses"] == []
        assert metrics["false_alarms"] == ["c2"]


class TestHeldoutAndLeakage:
    def test_overlap_is_leakage(self):
        overlap = ev.check_heldout_overlap(["t1", "t2"], ["t2", "t3"])
        assert overlap["overlap"] == ["t2"]
        assert overlap["leakage"] is True
        assert "LEAKAGE" in overlap["finding"]

    def test_disjoint_lists_pass(self):
        assert ev.check_heldout_overlap(["t1"], ["t2"])["leakage"] is False

    def test_complete_record_has_no_findings(self):
        complete = {
            "normalization_split": "training_only",
            "regime_seeds_disjoint_from_holdout": True,
            "dataset_version": "dataset-v1",
            "heldout_topology_overlap_checked": True,
        }
        assert ev.verify_leakage_controls(complete) == []

    def test_every_missing_control_is_a_finding(self):
        assert len(ev.verify_leakage_controls({})) == 4


class TestCarriedStatementsAndContractGaps:
    def test_heldout_topology_is_flagged_out_of_scope_contract_correctly(self):
        # The recorded Phase 7 follow-up (HONESTY HEADER item 7): held-out
        # ids are outside the trained list, so the worker as implemented
        # FLAGS them — the transfer experiment needs the recorded
        # scope-extension follow-up BEFORE any held-out run can produce
        # outputs. Pinned here so the gap cannot be silently bypassed.
        from gnn_inference_worker import evaluate_topology_scope

        scope = evaluate_topology_scope(
            "heldout-t6", ["train-t1", "train-t2"], "balanced-radial-feeder"
        )
        assert scope["in_scope"] is False

    def test_warm_start_statement_verbatim(self):
        assert ("affects SPEED of an exact solve, never its optimality"
                in ev.SOLVE_TIME_DISCIPLINE)

    def test_headline_rule_verbatim(self):
        assert "never model inference latency" in ev.HEADLINE_RULE

    def test_holdout_status_is_unconfirmed_recommendation(self):
        assert ev.HOLDOUT_STATUS.startswith("RECOMMENDATION — UNCONFIRMED")
        assert "lib-06" in ev.HOLDOUT_STATUS
        assert "P8.35" in ev.HOLDOUT_STATUS

    def test_no_threshold_is_set_here(self):
        assert "NOT SET" in ev.PRE_REGISTRATION_STATUS
        assert "never as a universal target" in ev.PRE_REGISTRATION_STATUS

    def test_measured_cells_status(self):
        assert "EMPTY until" in ev.MEASURED_CELLS_STATUS
        assert "NOT a" in ev.MEASURED_CELLS_STATUS

    def test_decision_point_carried(self):
        assert ev.GNN_PIN_STATUS.startswith("DECISION-POINT OPEN")


class TestInModuleSelfTestParity:
    def test_all_in_module_self_tests_pass(self):
        # The pytest mirror of `run_self_tests()` — one invocation, every
        # entry must be PASS (the same hand-computed arithmetic).
        results = ev.run_self_tests()
        assert results, "the in-module suite must be non-empty"
        failed = [entry for entry in results if entry[1] != "PASS"]
        assert failed == []
