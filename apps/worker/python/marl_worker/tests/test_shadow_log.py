# P8.14 — The MARL shadow-action-logger test suite: pure-logic parts that
#         RUN WITHOUT any framework installed.
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
#    `python apps/worker/python/marl_worker/shadow_log.py`.
# 2. What this suite pins (ledger P8.14 step 1): every log entry carries a
#    MANDATORY, closed-vocabulary safety_filter_outcome (the ledger's
#    "actions but not filter outcomes" failure mode is unrepresentable),
#    the fabricated-consequence pair is unbuildable (a
#    simulated_consequence cannot ride on a not_simulated entry), the
#    sequence order is strictly increasing from 1 (a gap would hide an
#    action), the closed field whitelist contains no
#    authorization/command-shaped key, and the summary arithmetic is the
#    hand-computed counts the P8.15 filter statistics consume. NOTHING
#    here proves any safety property: the log is a RECORD, never a
#    dispatch surface, and the simulated-consequence field is honestly
#    EMPTY (`not_simulated`) in this loop.
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


class TestCanonicalEntry:
    def test_fixed_statements(self) -> None:
        entry = sl.build_log_entry(**_entry_kwargs(1, "passed_within_limits"))
        assert entry["log_schema"] == "flexcore-marl-shadow-log/1"
        assert entry["lifecycle_state"] == "PROPOSED"
        assert entry["proposal_only"] is True
        assert entry["authority"].startswith("none —")
        assert entry["consequence_status"] == "not_simulated"
        assert entry["simulated_consequence"] is None
        assert entry["consequence_note"] == sl.NOT_SIMULATED_NOTE

    def test_closed_field_set(self) -> None:
        entry = sl.build_log_entry(**_entry_kwargs(1, "passed_within_limits"))
        assert set(entry) == set(sl.LOG_ENTRY_FIELDS)


class TestSequenceRules:
    def test_zero_sequence_refused(self) -> None:
        with pytest.raises(sl.ShadowLogError):
            sl.build_log_entry(**_entry_kwargs(0, "passed_within_limits"))

    def test_negative_sequence_refused(self) -> None:
        with pytest.raises(sl.ShadowLogError):
            sl.build_log_entry(**_entry_kwargs(-2, "passed_within_limits"))

    def test_bool_sequence_refused(self) -> None:
        with pytest.raises(sl.ShadowLogError):
            sl.build_log_entry(**_entry_kwargs(True, "passed_within_limits"))

    def test_sequence_gap_refused_hides_nothing(self) -> None:
        log = [
            sl.build_log_entry(**_entry_kwargs(1, "passed_within_limits")),
            sl.build_log_entry(**_entry_kwargs(3, "passed_within_limits")),
        ]
        with pytest.raises(sl.ShadowLogError):
            sl.validate_shadow_log(log)

    def test_duplicate_sequence_refused(self) -> None:
        log = [
            sl.build_log_entry(**_entry_kwargs(1, "passed_within_limits")),
            sl.build_log_entry(**_entry_kwargs(1, "passed_within_limits")),
        ]
        with pytest.raises(sl.ShadowLogError):
            sl.validate_shadow_log(log)

    def test_ordered_log_validates(self) -> None:
        log = [
            sl.build_log_entry(**_entry_kwargs(1, "passed_within_limits")),
            sl.build_log_entry(
                **_entry_kwargs(2, "filtered_clamped_to_limits",
                                filter_activations=[{"asset_id": "ess-1"}])
            ),
        ]
        sl.validate_shadow_log(log)

    def test_non_list_log_refused(self) -> None:
        with pytest.raises(sl.ShadowLogError):
            sl.validate_shadow_log({"sequence": 1})

    def test_wrong_schema_tag_refused(self) -> None:
        entry = sl.build_log_entry(**_entry_kwargs(1, "passed_within_limits"))
        entry["log_schema"] = "flexcore-marl-shadow-log/0"
        with pytest.raises(sl.ShadowLogError):
            sl.validate_shadow_log([entry])


class TestMandatoryFilterOutcome:
    def test_outcome_outside_vocabulary_refused(self) -> None:
        with pytest.raises(sl.ShadowLogError):
            sl.build_log_entry(**_entry_kwargs(1, "everything_fine"))

    def test_outcome_missing_refused(self) -> None:
        kwargs = _entry_kwargs(1, "passed_within_limits")
        kwargs["safety_filter_outcome"] = None
        with pytest.raises(sl.ShadowLogError):
            sl.build_log_entry(**kwargs)

    def test_every_closed_outcome_buildable(self) -> None:
        for outcome in (
            "passed_within_limits",
            "filtered_clamped_to_limits",
            "refused_permission_ceiling",
            "refused_nonfinite_action",
            "infeasible_no_safe_projection",
        ):
            entry = sl.build_log_entry(**_entry_kwargs(1, outcome))
            assert entry["safety_filter_outcome"] == outcome

    def test_log_with_outcome_outside_vocabulary_refused(self) -> None:
        entry = sl.build_log_entry(**_entry_kwargs(1, "passed_within_limits"))
        entry["safety_filter_outcome"] = "mostly_fine"
        with pytest.raises(sl.ShadowLogError):
            sl.validate_shadow_log([entry])


class TestConsequenceHonesty:
    def test_fabricated_consequence_refused(self) -> None:
        with pytest.raises(sl.ShadowLogError):
            sl.build_log_entry(
                **_entry_kwargs(
                    1, "passed_within_limits",
                    simulated_consequence={"violation": False},
                )
            )

    def test_recorded_status_requires_consequence(self) -> None:
        with pytest.raises(sl.ShadowLogError):
            sl.build_log_entry(
                **_entry_kwargs(1, "passed_within_limits",
                                consequence_status="recorded_by_harness")
            )

    def test_recorded_status_with_consequence_builds(self) -> None:
        entry = sl.build_log_entry(
            **_entry_kwargs(
                1, "passed_within_limits",
                consequence_status="recorded_by_harness",
                simulated_consequence={"consequence_ref": "harness/RECORD"},
            )
        )
        assert entry["consequence_status"] == "recorded_by_harness"

    def test_unknown_consequence_status_refused(self) -> None:
        with pytest.raises(sl.ShadowLogError):
            sl.build_log_entry(
                **_entry_kwargs(1, "passed_within_limits",
                                consequence_status="simulated_by_vibes")
            )


class TestFieldValidation:
    def test_empty_identity_refused(self) -> None:
        for field in ("request_id", "input_snapshot_id", "worker_mode",
                      "policy_version", "environment_version", "action_kind",
                      "proposed_permission"):
            kwargs = _entry_kwargs(1, "passed_within_limits")
            kwargs[field] = "  "
            with pytest.raises(sl.ShadowLogError):
                sl.build_log_entry(**kwargs)

    def test_bool_seed_refused(self) -> None:
        kwargs = _entry_kwargs(1, "passed_within_limits")
        kwargs["training_seed"] = True
        with pytest.raises(sl.ShadowLogError):
            sl.build_log_entry(**kwargs)

    def test_string_seed_refused(self) -> None:
        kwargs = _entry_kwargs(1, "passed_within_limits")
        kwargs["training_seed"] = "0"
        with pytest.raises(sl.ShadowLogError):
            sl.build_log_entry(**kwargs)

    def test_infeasibility_must_be_object_or_none(self) -> None:
        kwargs = _entry_kwargs(1, "infeasible_no_safe_projection")
        kwargs["infeasibility"] = "infeasible"
        with pytest.raises(sl.ShadowLogError):
            sl.build_log_entry(**kwargs)


class TestSummaryArithmetic:
    def test_hand_computed_counts(self) -> None:
        # HAND CHECK: entries 1..4 with outcomes passed / clamped /
        # refused-permission / infeasible -> total 4; by_outcome 1 each;
        # activation_count 1 (only entry 2 carries an activation);
        # infeasibility_count 1; refusal_count 2; consequences all
        # not_simulated (4).
        log = [
            sl.build_log_entry(**_entry_kwargs(1, "passed_within_limits")),
            sl.build_log_entry(
                **_entry_kwargs(2, "filtered_clamped_to_limits",
                                filter_activations=[
                                    {"asset_id": "ess-1", "p_filtered_mw": 3.5}
                                ])
            ),
            sl.build_log_entry(**_entry_kwargs(3, "refused_permission_ceiling")),
            sl.build_log_entry(
                **_entry_kwargs(
                    4, "infeasible_no_safe_projection",
                    infeasibility={"outcome": "infeasible_no_safe_projection",
                                   "recorded": True},
                )
            ),
        ]
        summary = sl.log_summary(log)
        assert summary["total_actions"] == 4
        assert summary["by_outcome"] == {
            "passed_within_limits": 1,
            "filtered_clamped_to_limits": 1,
            "refused_permission_ceiling": 1,
            "infeasible_no_safe_projection": 1,
        }
        assert summary["filter_activation_count"] == 1
        assert summary["infeasibility_count"] == 1
        assert summary["refusal_count"] == 2
        assert summary["consequence_status_counts"] == {
            "not_simulated": 4,
            "recorded_by_harness": 0,
        }

    def test_summary_over_empty_log(self) -> None:
        summary = sl.log_summary([])
        assert summary["total_actions"] == 0
        assert summary["by_outcome"] == {}
        assert summary["refusal_count"] == 0

    def test_summary_refuses_gapped_log(self) -> None:
        log = [
            sl.build_log_entry(**_entry_kwargs(1, "passed_within_limits")),
            sl.build_log_entry(**_entry_kwargs(3, "passed_within_limits")),
        ]
        with pytest.raises(sl.ShadowLogError):
            sl.log_summary(log)


class TestClosedFieldWhitelist:
    def test_no_authority_shaped_key_in_schema(self) -> None:
        assert not (
            sl.LOG_ENTRY_FIELDS
            & {"authorization", "authorization_token", "command", "command_id",
               "dispatch", "dispatch_command", "issued_command", "approval",
               "grant"}
        )

    def test_consequence_statuses_closed(self) -> None:
        assert sl.CONSEQUENCE_STATUSES == frozenset(
            {"not_simulated", "recorded_by_harness"}
        )


class TestSelfTestRunner:
    def test_in_module_self_tests_all_pass(self) -> None:
        # The hand-computed self-tests are runnable WITHOUT pytest
        # (`python apps/worker/python/marl_worker/shadow_log.py`); NEVER
        # RUN in the writing loop — the human gate runs it first.
        records = sl.run_self_tests()
        assert records, "the self-test runner returned no records"
        failures = [r for r in records if not r["passed"]]
        assert failures == [], failures
