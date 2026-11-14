# P8.15 — The MARL evaluation-harness test suite: pure-logic parts that
#         RUN WITHOUT any framework installed.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. WRITTEN, NEVER RUN. pytest was never installed in this loop; no test
#    below has ever executed. Expected values are HAND-COMPUTED
#    derivations with the arithmetic stated in each test (never observed
#    outputs). Run instruction:
#    `PYTHONPATH=apps/worker/python/marl_worker python -m pytest
#    apps/worker/python/marl_worker/tests -v`; the in-module self-tests
#    run WITHOUT pytest:
#    `python apps/worker/python/marl_worker/evaluation_harness.py`.
# 2. What this suite pins (ledger P8.15 step 3): the safety-filter
#    statistics (activation rate 1/5 = 0.2, infeasibility rate 1/5 = 0.2,
#    refusal rate 3/5 = 0.6 over the constructed five-entry log), the
#    outcome of EVERY infeasibility listed per occurrence (with the
#    fallback facts honestly NOT_RECORDED_IN_LOG_SCHEMA when the P8.14
#    log schema does not carry them), the per-case §19.4 metrics
#    (frequency 2/4 = 0.5, mean return 36.0/4 = 9.0), the
#    distribution-shift deltas (0.0 / -1.0), the per-metric win/tie/loss
#    verdicts under injected pre-registered tolerances (including the
#    boundary-is-a-tie case at exactly the tolerance), and the named
#    refusals (EVAL_LOG_EMPTY,
#    VIOLATION_FREQUENCY_NOT_COMPUTABLE_FROM_NOT_SIMULATED,
#    BASELINE_UNKNOWN, THRESHOLD_NOT_PRE_REGISTERED, CASE_FAMILY_UNKNOWN,
#    RETURN_UNIT_INCONSISTENT).
# 3. NOTHING here proves any safety property. The suite pins metric
#    DEFINITIONS over constructed inputs; zero observed violations from a
#    finite test set is a TEST OUTCOME, not a universal safety guarantee
#    (§13.3, X-4). A not_simulated log slice is refused, never silently
#    converted into a zero-violation frequency. The statistics are
#    reporting-only outputs (ADR-010) — never a dispatch input.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import os
import sys

import pytest

sys.path.insert(
    0,
    os.path.join(os.path.dirname(os.path.abspath(__file__)), os.pardir),
)

import shadow_log as sl  # noqa: E402 — path set above
from evaluation_harness import (  # noqa: E402 — path set above
    CASE_CONSEQUENCE_SCHEMA,
    EVALUATION_HARNESS_SCHEMA,
    EvaluationHarnessError,
    NOT_RECORDED_IN_LOG_SCHEMA,
    build_case_consequence,
    case_statistics,
    compare_case_to_baselines,
    distribution_shift_robustness,
    filter_statistics,
)


def _entry_kwargs(sequence: int, outcome: str, **overrides) -> dict:
    base = dict(
        sequence=sequence,
        request_id="req-%04d" % sequence,
        input_snapshot_id="snapshot-RECORD",
        worker_mode="filter_probe",
        policy_version="RECORD-AT-FIRST-RUN",
        training_seed=0,
        environment_version="flexcore-marl-env/0.1.0",
        action_kind="setpoint_delta",
        proposed_permission="schedule.propose",
        safety_filter_outcome=outcome,
        per_entry_outcomes=[
            {"asset_id": "ess-1", "entry_outcome": "within_limits"}
        ],
        filter_activations=[],
        infeasibility=None,
    )
    base.update(overrides)
    return base


def _entry(sequence: int, outcome: str, **overrides) -> dict:
    return sl.build_log_entry(**_entry_kwargs(sequence, outcome, **overrides))


def _recorded(entry: dict, *, violated: bool, kind, ret: float) -> dict:
    consequence = build_case_consequence(
        constraint_violation=violated,
        violation_kind=kind,
        return_value=ret,
        return_unit="declared-cost-unit",
    )
    return sl.build_log_entry(
        sequence=entry["sequence"],
        request_id=entry["request_id"],
        input_snapshot_id=entry["input_snapshot_id"],
        worker_mode=entry["worker_mode"],
        policy_version=entry["policy_version"],
        training_seed=entry["training_seed"],
        environment_version=entry["environment_version"],
        action_kind=entry["action_kind"],
        proposed_permission=entry["proposed_permission"],
        safety_filter_outcome=entry["safety_filter_outcome"],
        per_entry_outcomes=entry["per_entry_outcomes"],
        filter_activations=entry["filter_activations"],
        infeasibility=entry["infeasibility"],
        consequence_status="recorded_by_harness",
        simulated_consequence=consequence,
    )


def _five_entry_log() -> list:
    # HAND CHECK: outcomes passed / clamped / refused-permission /
    # refused-nonfinite / infeasible -> activation count 1, infeasibility
    # count 1, refusal count 3, total 5.
    return [
        _entry(1, "passed_within_limits"),
        _entry(
            2,
            "filtered_clamped_to_limits",
            filter_activations=[{"asset_id": "ess-1"}],
        ),
        _entry(3, "refused_permission_ceiling"),
        _entry(4, "refused_nonfinite_action"),
        _entry(
            5,
            "infeasible_no_safe_projection",
            infeasibility={
                "outcome": "infeasible_no_safe_projection",
                "entries": ["ess-1"],
                "recorded": True,
            },
        ),
    ]


class TestFilterStatistics:
    def test_hand_computed_rates(self) -> None:
        stats = filter_statistics(_five_entry_log())
        assert stats["total_actions"] == 5
        assert stats["counts"]["filter_activation_count"] == 1
        assert stats["counts"]["infeasibility_count"] == 1
        assert stats["counts"]["refusal_count"] == 3
        assert stats["activation_rate"] == 0.2  # 1/5
        assert stats["infeasibility_rate"] == 0.2  # 1/5
        assert stats["refusal_rate"] == 0.6  # 3/5

    def test_outcome_of_every_infeasibility(self) -> None:
        stats = filter_statistics(_five_entry_log())
        rows = stats["outcome_of_every_infeasibility"]
        assert len(rows) == 1
        assert rows[0]["sequence"] == 5
        assert rows[0]["outcome"] == "infeasible_no_safe_projection"
        assert rows[0]["assets"] == ["ess-1"]
        assert rows[0]["recorded"] is True
        # The P8.14 log schema carries no fallback facts on the
        # infeasibility object — the harness records the absence
        # honestly instead of fabricating a default.
        assert rows[0]["fallback_attempted"] == NOT_RECORDED_IN_LOG_SCHEMA
        assert rows[0]["fallback_viable"] == NOT_RECORDED_IN_LOG_SCHEMA

    def test_fallback_facts_carried_when_recorded(self) -> None:
        log = [
            _entry(
                1,
                "infeasible_no_safe_projection",
                infeasibility={
                    "outcome": "infeasible_no_safe_projection",
                    "entries": ["ess-2"],
                    "recorded": True,
                    "fallback_attempted": True,
                    "fallback_viable": False,
                },
            )
        ]
        stats = filter_statistics(log)
        row = stats["outcome_of_every_infeasibility"][0]
        assert row["fallback_attempted"] is True
        assert row["fallback_viable"] is False
        assert stats["infeasibility_rate"] == 1.0  # 1/1

    def test_reporting_only_statement_and_schema(self) -> None:
        stats = filter_statistics(_five_entry_log())
        assert stats["schema"] == EVALUATION_HARNESS_SCHEMA
        assert stats["usage"].startswith("evaluation reporting only")

    def test_empty_log_refused(self) -> None:
        with pytest.raises(EvaluationHarnessError, match="EVAL_LOG_EMPTY"):
            filter_statistics([])

    def test_infeasibility_outcome_vocabulary_enforced(self) -> None:
        log = [
            _entry(
                1,
                "infeasible_no_safe_projection",
                infeasibility={"outcome": "infeasible", "entries": []},
            )
        ]
        with pytest.raises(
            EvaluationHarnessError, match="CONSEQUENCE_SHAPE_INVALID"
        ):
            filter_statistics(log)

    def test_no_dispatch_shaped_callable_in_module(self) -> None:
        import evaluation_harness as eh

        forbidden = ("dispatch", "issue", "authorize", "approve", "command")
        offenders = [
            name
            for name, obj in vars(eh).items()
            if callable(obj)
            and not name.startswith("_")
            and any(token in name.lower() for token in forbidden)
        ]
        assert offenders == []


class TestCaseConsequence:
    def test_canonical_record(self) -> None:
        record = build_case_consequence(
            constraint_violation=False,
            violation_kind=None,
            return_value=8.0,
            return_unit="declared-cost-unit",
        )
        assert record["consequence_schema"] == CASE_CONSEQUENCE_SCHEMA
        assert record["constraint_violation"] is False
        assert record["violation_kind"] is None
        assert record["return_value"] == 8.0
        assert record["return_unit"] == "declared-cost-unit"
        assert record["source"].startswith("recorded_by_harness")

    def test_unnamed_violation_refused(self) -> None:
        with pytest.raises(
            EvaluationHarnessError, match="CONSEQUENCE_SHAPE_INVALID"
        ):
            build_case_consequence(
                constraint_violation=True,
                violation_kind=None,
                return_value=1.0,
                return_unit="declared-cost-unit",
            )

    def test_kind_without_violation_refused(self) -> None:
        with pytest.raises(
            EvaluationHarnessError, match="CONSEQUENCE_SHAPE_INVALID"
        ):
            build_case_consequence(
                constraint_violation=False,
                violation_kind="soc_bound",
                return_value=1.0,
                return_unit="declared-cost-unit",
            )

    def test_nonfinite_return_refused(self) -> None:
        with pytest.raises(
            EvaluationHarnessError, match="CONSEQUENCE_SHAPE_INVALID"
        ):
            build_case_consequence(
                constraint_violation=False,
                violation_kind=None,
                return_value=float("inf"),
                return_unit="declared-cost-unit",
            )

    def test_non_bool_violation_refused(self) -> None:
        with pytest.raises(
            EvaluationHarnessError, match="CONSEQUENCE_SHAPE_INVALID"
        ):
            build_case_consequence(
                constraint_violation=1,  # int, not bool
                violation_kind=None,
                return_value=1.0,
                return_unit="declared-cost-unit",
            )

    def test_empty_unit_refused(self) -> None:
        with pytest.raises(
            EvaluationHarnessError, match="CONSEQUENCE_SHAPE_INVALID"
        ):
            build_case_consequence(
                constraint_violation=False,
                violation_kind=None,
                return_value=1.0,
                return_unit="  ",
            )


class TestCaseStatistics:
    def _case_log(self) -> list:
        # HAND CHECK: violations [True, False, False, True] ->
        # frequency 2/4 = 0.5; returns 10.0+12.0+8.0+6.0 = 36.0 ->
        # mean 36.0/4 = 9.0; kinds {"soc_bound": 1, "power_limit": 1}.
        return [
            _recorded(
                _entry(1, "passed_within_limits"),
                violated=True,
                kind="soc_bound",
                ret=10.0,
            ),
            _recorded(
                _entry(2, "filtered_clamped_to_limits"),
                violated=False,
                kind=None,
                ret=12.0,
            ),
            _recorded(
                _entry(3, "passed_within_limits"),
                violated=False,
                kind=None,
                ret=8.0,
            ),
            _recorded(
                _entry(4, "passed_within_limits"),
                violated=True,
                kind="power_limit",
                ret=6.0,
            ),
        ]

    def test_hand_computed_frequency_and_return(self) -> None:
        stats = case_statistics(self._case_log())
        assert stats["total_decisions"] == 4
        assert stats["constraint_violation_count"] == 2
        assert stats["violation_frequency"] == 0.5  # 2/4
        assert stats["violation_kinds"] == {"soc_bound": 1, "power_limit": 1}
        assert stats["mean_return"] == 9.0  # 36.0/4
        assert stats["return_unit"] == "declared-cost-unit"

    def test_per_decision_rows_carry_sequences(self) -> None:
        stats = case_statistics(self._case_log())
        assert [row["sequence"] for row in stats["per_decision"]] == [1, 2, 3, 4]

    def test_not_simulated_slice_refused(self) -> None:
        # An EMPTY consequence cannot become a frequency of zero — the
        # §13.3 honesty rule enforced at the statistic's front door.
        with pytest.raises(
            EvaluationHarnessError,
            match="VIOLATION_FREQUENCY_NOT_COMPUTABLE_FROM_NOT_SIMULATED",
        ):
            case_statistics([_entry(1, "passed_within_limits")])

    def test_mixed_return_units_refused(self) -> None:
        log = self._case_log()
        other_unit = build_case_consequence(
            constraint_violation=False,
            violation_kind=None,
            return_value=1.0,
            return_unit="other-unit",
        )
        log[3] = sl.build_log_entry(
            sequence=4,
            request_id=log[3]["request_id"],
            input_snapshot_id=log[3]["input_snapshot_id"],
            worker_mode=log[3]["worker_mode"],
            policy_version=log[3]["policy_version"],
            training_seed=log[3]["training_seed"],
            environment_version=log[3]["environment_version"],
            action_kind=log[3]["action_kind"],
            proposed_permission=log[3]["proposed_permission"],
            safety_filter_outcome=log[3]["safety_filter_outcome"],
            per_entry_outcomes=log[3]["per_entry_outcomes"],
            filter_activations=log[3]["filter_activations"],
            infeasibility=log[3]["infeasibility"],
            consequence_status="recorded_by_harness",
            simulated_consequence=other_unit,
        )
        with pytest.raises(
            EvaluationHarnessError, match="RETURN_UNIT_INCONSISTENT"
        ):
            case_statistics(log)

    def test_hand_rolled_consequence_dict_refused(self) -> None:
        # A consequence dict that did not go through the real builder
        # (wrong schema tag) is refused — records are shaped, not assumed.
        log = [_entry(1, "passed_within_limits")]
        forged = sl.build_log_entry(
            **dict(
                _entry_kwargs(1, "passed_within_limits"),
                consequence_status="recorded_by_harness",
                simulated_consequence={"constraint_violation": False},
            )
        )
        log = [forged]
        with pytest.raises(
            EvaluationHarnessError, match="CONSEQUENCE_SHAPE_INVALID"
        ):
            case_statistics(log)

    def test_empty_case_slice_refused(self) -> None:
        with pytest.raises(EvaluationHarnessError, match="EVAL_LOG_EMPTY"):
            case_statistics([])


class TestDistributionShift:
    def _reference(self) -> dict:
        return case_statistics(
            [
                _recorded(
                    _entry(1, "passed_within_limits"),
                    violated=True,
                    kind="soc_bound",
                    ret=10.0,
                ),
                _recorded(
                    _entry(2, "passed_within_limits"),
                    violated=False,
                    kind=None,
                    ret=12.0,
                ),
                _recorded(
                    _entry(3, "passed_within_limits"),
                    violated=False,
                    kind=None,
                    ret=8.0,
                ),
                _recorded(
                    _entry(4, "passed_within_limits"),
                    violated=True,
                    kind="power_limit",
                    ret=6.0,
                ),
            ]
        )

    def _shifted(self) -> dict:
        # HAND CHECK: violations [True, True, False, False] -> 2/4 = 0.5;
        # returns 6.0+6.0+10.0+10.0 = 32.0 -> mean 8.0.
        return case_statistics(
            [
                _recorded(
                    _entry(1, "passed_within_limits"),
                    violated=True,
                    kind="soc_bound",
                    ret=6.0,
                ),
                _recorded(
                    _entry(2, "passed_within_limits"),
                    violated=True,
                    kind="soc_bound",
                    ret=6.0,
                ),
                _recorded(
                    _entry(3, "passed_within_limits"),
                    violated=False,
                    kind=None,
                    ret=10.0,
                ),
                _recorded(
                    _entry(4, "passed_within_limits"),
                    violated=False,
                    kind=None,
                    ret=10.0,
                ),
            ]
        )

    def test_hand_computed_deltas(self) -> None:
        robustness = distribution_shift_robustness(self._reference(), self._shifted())
        # frequency delta 0.5 - 0.5 = 0.0; return delta 8.0 - 9.0 = -1.0.
        assert robustness["violation_frequency_delta"] == 0.0
        assert robustness["mean_return_delta"] == -1.0
        assert robustness["return_unit"] == "declared-cost-unit"

    def test_unit_mismatch_refused(self) -> None:
        reference = self._reference()
        shifted = self._shifted()
        shifted["return_unit"] = "other-unit"
        with pytest.raises(
            EvaluationHarnessError, match="RETURN_UNIT_INCONSISTENT"
        ):
            distribution_shift_robustness(reference, shifted)

    def test_non_stats_object_refused(self) -> None:
        with pytest.raises(EvaluationHarnessError, match="STATS_OBJECT_INVALID"):
            distribution_shift_robustness({"mean_return": 1.0}, self._shifted())


class TestBaselineComparison:
    def _policy(self) -> dict:
        # HAND CHECK: 0 violations -> frequency 0.0; returns 10+12+8+6 =
        # 36.0 -> mean 9.0.
        return case_statistics(
            [
                _recorded(
                    _entry(1, "passed_within_limits"),
                    violated=False,
                    kind=None,
                    ret=10.0,
                ),
                _recorded(
                    _entry(2, "passed_within_limits"),
                    violated=False,
                    kind=None,
                    ret=12.0,
                ),
                _recorded(
                    _entry(3, "passed_within_limits"),
                    violated=False,
                    kind=None,
                    ret=8.0,
                ),
                _recorded(
                    _entry(4, "passed_within_limits"),
                    violated=False,
                    kind=None,
                    ret=6.0,
                ),
            ]
        )

    def _highs(self) -> dict:
        # HAND CHECK: violations [True, F, F, F] -> 1/4 = 0.25; returns
        # all 8.0 -> mean 8.0.
        return case_statistics(
            [
                _recorded(
                    _entry(1, "passed_within_limits"),
                    violated=True,
                    kind="power_limit",
                    ret=8.0,
                ),
                _recorded(
                    _entry(2, "passed_within_limits"),
                    violated=False,
                    kind=None,
                    ret=8.0,
                ),
                _recorded(
                    _entry(3, "passed_within_limits"),
                    violated=False,
                    kind=None,
                    ret=8.0,
                ),
                _recorded(
                    _entry(4, "passed_within_limits"),
                    violated=False,
                    kind=None,
                    ret=8.0,
                ),
            ]
        )

    def _rule_based(self) -> dict:
        # HAND CHECK: 0 violations -> frequency 0.0; returns all 8.5 ->
        # mean 8.5.
        return case_statistics(
            [
                _recorded(
                    _entry(1, "passed_within_limits"),
                    violated=False,
                    kind=None,
                    ret=8.5,
                ),
                _recorded(
                    _entry(2, "passed_within_limits"),
                    violated=False,
                    kind=None,
                    ret=8.5,
                ),
                _recorded(
                    _entry(3, "passed_within_limits"),
                    violated=False,
                    kind=None,
                    ret=8.5,
                ),
                _recorded(
                    _entry(4, "passed_within_limits"),
                    violated=False,
                    kind=None,
                    ret=8.5,
                ),
            ]
        )

    def _tolerances(self) -> dict:
        return {
            "violation_frequency_absolute_tolerance": 0.05,
            "return_absolute_tolerance": 0.5,
        }

    def test_hand_computed_verdicts(self) -> None:
        rows = compare_case_to_baselines(
            case_id="RECORD-AT-FIRST-RUN",
            case_family="held_out_regime",
            policy_stats=self._policy(),
            baseline_stats_by_id={
                "baseline/highs-tier1-lp": self._highs(),
                "baseline/rule-based-priority-list": self._rule_based(),
            },
            pre_registered_tolerances=self._tolerances(),
        )
        assert len(rows) == 2
        # vs HiGHS: frequency delta 0.0 - 0.25 = -0.25 < -0.05 -> win;
        # return delta 9.0 - 8.0 = 1.0 > 0.5 -> win.
        assert rows[0]["baseline_id"] == "baseline/highs-tier1-lp"
        assert rows[0]["violation_frequency_delta"] == -0.25
        assert rows[0]["violation_frequency_verdict"] == "win"
        assert rows[0]["mean_return_delta"] == 1.0
        assert rows[0]["return_verdict"] == "win"
        # vs rule-based: frequency delta 0.0 -> tie; return delta
        # 9.0 - 8.5 = 0.5 -> |0.5| <= 0.5 -> tie (the boundary IS the tie).
        assert rows[1]["baseline_id"] == "baseline/rule-based-priority-list"
        assert rows[1]["violation_frequency_verdict"] == "tie"
        assert rows[1]["mean_return_delta"] == 0.5
        assert rows[1]["return_verdict"] == "tie"
        # NO merged headline exists.
        assert all(row["merged_headline"] is None for row in rows)

    def test_loss_verdict_reported(self) -> None:
        # HAND CHECK: a policy WORSE than the rule-based baseline on
        # return (policy mean 7.0 < 8.5 - 0.5): delta -1.5 < -0.5 -> loss.
        worse_policy = case_statistics(
            [
                _recorded(
                    _entry(1, "passed_within_limits"),
                    violated=False,
                    kind=None,
                    ret=7.0,
                ),
                _recorded(
                    _entry(2, "passed_within_limits"),
                    violated=False,
                    kind=None,
                    ret=7.0,
                ),
                _recorded(
                    _entry(3, "passed_within_limits"),
                    violated=False,
                    kind=None,
                    ret=7.0,
                ),
                _recorded(
                    _entry(4, "passed_within_limits"),
                    violated=False,
                    kind=None,
                    ret=7.0,
                ),
            ]
        )
        rows = compare_case_to_baselines(
            case_id="c",
            case_family="device_outage",
            policy_stats=worse_policy,
            baseline_stats_by_id={
                "baseline/rule-based-priority-list": self._rule_based()
            },
            pre_registered_tolerances=self._tolerances(),
        )
        assert rows[0]["mean_return_delta"] == -1.5
        assert rows[0]["return_verdict"] == "loss"

    def test_unknown_baseline_refused(self) -> None:
        with pytest.raises(EvaluationHarnessError, match="BASELINE_UNKNOWN"):
            compare_case_to_baselines(
                case_id="c",
                case_family="held_out_regime",
                policy_stats=self._policy(),
                baseline_stats_by_id={"baseline/pi-undeclared": self._highs()},
                pre_registered_tolerances=self._tolerances(),
            )

    def test_missing_tolerances_refused(self) -> None:
        with pytest.raises(
            EvaluationHarnessError, match="THRESHOLD_NOT_PRE_REGISTERED"
        ):
            compare_case_to_baselines(
                case_id="c",
                case_family="held_out_regime",
                policy_stats=self._policy(),
                baseline_stats_by_id={
                    "baseline/highs-tier1-lp": self._highs()
                },
                pre_registered_tolerances={},
            )

    def test_negative_tolerance_refused(self) -> None:
        with pytest.raises(
            EvaluationHarnessError, match="THRESHOLD_NOT_PRE_REGISTERED"
        ):
            compare_case_to_baselines(
                case_id="c",
                case_family="held_out_regime",
                policy_stats=self._policy(),
                baseline_stats_by_id={
                    "baseline/highs-tier1-lp": self._highs()
                },
                pre_registered_tolerances={
                    "violation_frequency_absolute_tolerance": -0.1,
                    "return_absolute_tolerance": 0.5,
                },
            )

    def test_unknown_family_refused(self) -> None:
        with pytest.raises(EvaluationHarnessError, match="CASE_FAMILY_UNKNOWN"):
            compare_case_to_baselines(
                case_id="c",
                case_family="training_regime",
                policy_stats=self._policy(),
                baseline_stats_by_id={
                    "baseline/highs-tier1-lp": self._highs()
                },
                pre_registered_tolerances=self._tolerances(),
            )

    def test_unit_mismatch_refused(self) -> None:
        highs = self._highs()
        highs["return_unit"] = "other-unit"
        with pytest.raises(
            EvaluationHarnessError, match="RETURN_UNIT_INCONSISTENT"
        ):
            compare_case_to_baselines(
                case_id="c",
                case_family="state_estimate_uncertainty",
                policy_stats=self._policy(),
                baseline_stats_by_id={"baseline/highs-tier1-lp": highs},
                pre_registered_tolerances=self._tolerances(),
            )

    def test_empty_baseline_set_refused(self) -> None:
        with pytest.raises(
            EvaluationHarnessError, match="BASELINE_STATS_INVALID"
        ):
            compare_case_to_baselines(
                case_id="c",
                case_family="held_out_topology",
                policy_stats=self._policy(),
                baseline_stats_by_id={},
                pre_registered_tolerances=self._tolerances(),
            )
