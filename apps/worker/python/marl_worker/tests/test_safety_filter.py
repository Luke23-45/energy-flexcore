# P8.13/P8.14 — The MARL safety-filter test suite: pure-logic parts that
#               RUN WITHOUT any framework installed, plus the
#               permission-ceiling profile pin.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. WRITTEN, NEVER RUN. pytest was never installed in this loop; no test
#    below has ever executed. The tests target stdlib-only pure functions
#    (the feasible-interval projection, the named outcomes, the
#    infeasible-filter fallback property, the closed vocabularies) and are
#    expected to pass at the first human run. Every expected value is a
#    HAND-COMPUTED derivation stated in the test (never an observed
#    output).
# 2. Run instruction (the recorded procedure):
#    `PYTHONPATH=apps/worker/python/marl_worker python -m pytest
#    apps/worker/python/marl_worker/tests -v` from the repository root.
#    The in-module hand-computed self-tests run WITHOUT pytest:
#    `python apps/worker/python/marl_worker/safety_filter.py`.
# 3. What this suite pins (ledger P8.13 steps 2–3 as code): the
#    permission-ceiling gate (a non-projectable refusal, never a clamp),
#    the P/ramp projection arithmetic (hand-computed MW figures), the
#    EMPTY-interval infeasible-filter outcome with the PROVABLY dead
#    fallback, the non-finite refusal, the malformed-input error class
#    (errors refuse inputs; outcomes classify actions — never conflated),
#    and the closed outcome vocabularies (the value set of the P8.04
#    `safety_filter_outcome` result key). NOTHING here proves any safety
#    property of any policy: this filter is a STATIC limit/permission
#    projection, NOT the CBF layer (13_marl_design.md §3 — not yet
#    derived, training blocked), and no safety claim exists anywhere.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import json
import os
import sys

import pytest

sys.path.insert(
    0,
    os.path.join(os.path.dirname(os.path.abspath(__file__)), os.pardir),
)

import safety_filter as sf  # noqa: E402 — path set above

REPO_ROOT = os.path.abspath(
    os.path.join(
        os.path.dirname(os.path.abspath(__file__)),
        os.pardir, os.pardir, os.pardir, os.pardir, os.pardir,
    )
)


def _limits(p_min: float, p_max: float, ramp: float) -> dict:
    return {"p_min_mw": p_min, "p_max_mw": p_max, "ramp_mw_per_interval": ramp}


def _action(entries: list, permission: str = "schedule.propose",
            kind: str = "setpoint_delta") -> dict:
    return {"action_kind": kind, "proposed_permission": permission, "assets": entries}


def _entry(asset_id: str, p_current: float, p_proposed: float, limits: dict,
           soc: float | None = None) -> dict:
    return {
        "asset_id": asset_id,
        "p_current_mw": p_current,
        "p_proposed_mw": p_proposed,
        "soc_fraction": soc,
        "limits": limits,
    }


BASE = _limits(0.0, 5.0, 1.5)


class TestPermissionCeilingProfilePin:
    def test_ceiling_matches_sim_lab_profile_json(self) -> None:
        """The closed PERMISSION_CEILING constant equals the
        `permission-ceiling` list in plugins/profiles/sim-lab.profile.json
        (the profile file is the DEFINITION OF RECORD; the two MUST move
        together — this test is the pin). The profile is JSON-with-comments
        per the repo's documented commented-out pattern; this helper strips
        FULL-LINE `//` comments before parsing (the recorded limitation:
        a trailing comment would break this helper loudly, which is the
        safe direction)."""
        path = os.path.join(REPO_ROOT, "plugins", "profiles", "sim-lab.profile.json")
        with open(path, "r", encoding="utf-8") as handle:
            raw = handle.read()
        stripped = "\n".join(
            line for line in raw.splitlines()
            if not line.strip().startswith("//")
        )
        profile = json.loads(stripped)
        assert set(profile["permission-ceiling"]) == set(sf.PERMISSION_CEILING)
        assert sf.PERMISSION_CEILING == frozenset(
            {"telemetry.read", "asset.read", "schedule.propose"}
        )

    def test_action_permission_is_schedule_propose(self) -> None:
        assert sf.ACTION_PROPOSAL_PERMISSION == "schedule.propose"

    def test_permission_outside_ceiling_refused_wholesale(self) -> None:
        # HAND CHECK: "dispatch.issue" is not in the ceiling -> the named
        # refusal, ZERO entries projected (no clamp turns a forbidden
        # permission into an allowed one).
        report = sf.filter_action(
            _action([_entry("ess-1", 2.0, 2.5, BASE)], permission="dispatch.issue")
        )
        assert report["action_outcome"] == "refused_permission_ceiling"
        assert report["entries"] == []
        assert report["filter_activations"] == []

    def test_permission_refusal_is_in_refusing_set(self) -> None:
        assert "refused_permission_ceiling" in sf.REFUSING_OUTCOMES


class TestProjectionArithmetic:
    def test_inside_interval_within_limits(self) -> None:
        # HAND CHECK: interval = [max(0.0, 2.0-1.5), min(5.0, 2.0+1.5)]
        # = [0.5, 3.5]; proposed 2.5 inside -> within_limits, filtered 2.5.
        entry = sf.filter_entry(_entry("ess-1", 2.0, 2.5, BASE, soc=0.5))
        assert entry["entry_outcome"] == "within_limits"
        assert entry["p_filtered_mw"] == 2.5
        assert entry["allowed_min_mw"] == 0.5
        assert entry["allowed_max_mw"] == 3.5

    def test_above_interval_clamped_to_ramp_bound(self) -> None:
        # HAND CHECK: proposed 7.0 > hi 3.5 = min(5.0, 2.0+1.5) — the RAMP
        # binds before the P bound -> clamped to 3.5.
        entry = sf.filter_entry(_entry("ess-1", 2.0, 7.0, BASE))
        assert entry["entry_outcome"] == "clamped_to_limit"
        assert entry["p_filtered_mw"] == 3.5

    def test_below_interval_clamped_to_ramp_floor(self) -> None:
        # HAND CHECK: proposed -1.0 < lo 0.5 = max(0.0, 2.0-1.5) -> 0.5.
        entry = sf.filter_entry(_entry("ess-1", 2.0, -1.0, BASE))
        assert entry["entry_outcome"] == "clamped_to_limit"
        assert entry["p_filtered_mw"] == 0.5

    def test_p_bound_binds_when_ramp_would_allow_more(self) -> None:
        # HAND CHECK: hi = min(5.0, 4.0+1.5) = 5.0 (the P bound binds);
        # proposed 5.4 clamps to 5.0.
        entry = sf.filter_entry(_entry("ess-1", 4.0, 5.4, BASE))
        assert entry["entry_outcome"] == "clamped_to_limit"
        assert entry["p_filtered_mw"] == 5.0
        assert entry["allowed_max_mw"] == 5.0

    def test_hold_current_inside_interval_passes(self) -> None:
        # HAND CHECK: hold 2.0 inside [0.5, 3.5] -> within, no activation.
        report = sf.filter_action(
            _action([_entry("ess-1", 2.0, 2.0, BASE)], kind="hold_current_setpoint")
        )
        assert report["action_outcome"] == "passed_within_limits"
        assert report["filter_activations"] == []

    def test_activation_recorded_with_before_and_after(self) -> None:
        report = sf.filter_action(_action([_entry("ess-1", 2.0, 7.0, BASE)]))
        assert report["action_outcome"] == "filtered_clamped_to_limits"
        assert report["filter_activations"][0]["p_proposed_mw"] == 7.0
        assert report["filter_activations"][0]["p_filtered_mw"] == 3.5


class TestInfeasibleFilterOutcome:
    def test_empty_interval_named_infeasible(self) -> None:
        # HAND CHECK: p_current 3.0 outside [1.0, 2.0], ramp 0.5:
        #   interval = [max(1.0, 2.5), min(2.0, 3.5)] = [2.5, 2.0] EMPTY.
        report = sf.filter_action(
            _action([_entry("ess-1", 3.0, 3.0, _limits(1.0, 2.0, 0.5))])
        )
        assert report["action_outcome"] == "infeasible_no_safe_projection"
        entry = report["entries"][0]
        assert entry["allowed_min_mw"] == 2.5
        assert entry["allowed_max_mw"] == 2.0
        assert entry["p_filtered_mw"] is None

    def test_fallback_attempted_and_provably_not_viable(self) -> None:
        # The P8.13 §4 fallback (hold-current, re-filtered) is attempted
        # and RECORDED — and is provably infeasible when the primary
        # interval is empty (module-header item 4 derivation): hold-current
        # 3.0 outside [1.0, 2.0].
        report = sf.filter_action(
            _action([_entry("ess-1", 3.0, 3.0, _limits(1.0, 2.0, 0.5))])
        )
        assert report["fallback"]["attempted"] is True
        assert report["fallback"]["viable"] is False
        assert report["infeasibility"]["recorded"] is True
        assert report["infeasibility"]["outcome"] == "infeasible_no_safe_projection"

    def test_fallback_property_second_construction(self) -> None:
        # HAND CHECK: p_current -1.0 below p_min 0.0, ramp 0.2:
        #   interval = [max(0.0, -1.2), min(4.0, -0.8)] = [0.0, -0.8] EMPTY;
        #   hold-current -1.0 < p_min 0.0 -> fallback infeasible.
        report = sf.filter_action(
            _action([_entry("pv-1", -1.0, 1.0, _limits(0.0, 4.0, 0.2))])
        )
        assert report["action_outcome"] == "infeasible_no_safe_projection"
        assert report["fallback"]["viable"] is False

    def test_state_within_bounds_never_infeasible(self) -> None:
        # HAND CHECK: p_current inside [p_min, p_max] => p_current is
        # inside the interval (ramp >= 0), so the interval can NEVER be
        # empty — the infeasible outcome requires an out-of-bounds state.
        # Constructive check: 2.0 in [0.0, 5.0] -> interval [0.5, 3.5].
        lo, hi = sf.feasible_interval(2.0, BASE)
        assert lo <= 2.0 <= hi


class TestNonfiniteProposal:
    def test_nan_proposal_refused_named(self) -> None:
        # HAND CHECK: json.loads accepts NaN, so a NaN proposal can arrive;
        # refused with p_filtered_mw None — no 'nearest safe value' to a NaN.
        report = sf.filter_action(
            _action([_entry("ess-1", 2.0, float("nan"), BASE)])
        )
        assert report["action_outcome"] == "refused_nonfinite_action"
        assert report["entries"][0]["p_filtered_mw"] is None

    def test_infinite_proposal_refused_named(self) -> None:
        report = sf.filter_action(
            _action([_entry("ess-1", 2.0, float("inf"), BASE)])
        )
        assert report["action_outcome"] == "refused_nonfinite_action"


class TestMalformedInputsAreErrorsNotOutcomes:
    def test_inverted_limits_refused(self) -> None:
        with pytest.raises(sf.FilterInputError):
            sf.validate_limits(_limits(5.0, 1.0, 1.0), "test")

    def test_negative_ramp_refused(self) -> None:
        with pytest.raises(sf.FilterInputError):
            sf.validate_limits(_limits(0.0, 5.0, -1.0), "test")

    def test_nonfinite_limit_refused(self) -> None:
        with pytest.raises(sf.FilterInputError):
            sf.validate_limits(_limits(0.0, float("nan"), 1.0), "test")

    def test_soc_above_one_refused(self) -> None:
        with pytest.raises(sf.FilterInputError):
            sf.validate_soc_fraction(1.5, "test")

    def test_soc_negative_refused(self) -> None:
        with pytest.raises(sf.FilterInputError):
            sf.validate_soc_fraction(-0.1, "test")

    def test_soc_boundaries_accepted(self) -> None:
        assert sf.validate_soc_fraction(0.0, "test") == 0.0
        assert sf.validate_soc_fraction(1.0, "test") == 1.0

    def test_soc_none_accepted(self) -> None:
        assert sf.validate_soc_fraction(None, "test") is None

    def test_nonfinite_current_state_refused(self) -> None:
        # The CURRENT state cannot be non-finite: malformed input, refused
        # before any projection (a non-finite PROPOSAL is an outcome; the
        # two are never conflated).
        with pytest.raises(sf.FilterInputError):
            sf.validate_action(
                _action([_entry("ess-1", float("nan"), 1.0, BASE)])
            )

    def test_device_command_action_kind_refused(self) -> None:
        # A connect/disconnect is a device-command shape: outside the
        # closed action-kind set (P8.14 boundary).
        with pytest.raises(sf.FilterInputError):
            sf.validate_action(
                _action([_entry("ess-1", 2.0, 2.0, BASE)], kind="connect_device")
            )

    def test_empty_asset_list_refused(self) -> None:
        with pytest.raises(sf.FilterInputError):
            sf.validate_action(_action([]))

    def test_missing_limits_refused(self) -> None:
        with pytest.raises(sf.FilterInputError):
            sf.validate_action(
                [{"asset_id": "a", "p_current_mw": 1.0, "p_proposed_mw": 1.0,
                  "soc_fraction": None, "limits": None}]
            )

    def test_filter_input_error_normalizes_in_worker(self) -> None:
        # The worker normalizes FilterInputError into WorkerRequestError
        # (one fail-closed refusal type at the composition edge).
        import marl_worker

        request = {
            "protocol": marl_worker.PROTOCOL_ID,
            "mode": "filter_probe",
            "policy_context": {"policy_version": "p", "training_seed": 0,
                               "environment_version": "env"},
            "action": _action([_entry("a", 1.0, 1.0, _limits(5.0, 1.0, 1.0))]),
            "provenance": {"input_snapshot_id": "s", "request_id": "r"},
        }
        with pytest.raises(marl_worker.WorkerRequestError):
            marl_worker.validate_request(request)


class TestAggregateMapping:
    def test_mixed_clamped_and_within(self) -> None:
        # HAND CHECK: ess-1 clamps 7.0 -> 3.5; ess-2 proposed 1.2 inside
        # [max(0.0, -0.5), min(5.0, 2.5)] = [0.0, 2.5] -> within. Aggregate
        # = filtered_clamped_to_limits, exactly ONE activation.
        report = sf.filter_action(
            _action([_entry("ess-1", 2.0, 7.0, BASE),
                     _entry("ess-2", 1.0, 1.2, BASE)])
        )
        assert report["action_outcome"] == "filtered_clamped_to_limits"
        assert len(report["filter_activations"]) == 1
        assert report["filter_activations"][0]["asset_id"] == "ess-1"

    def test_every_refusing_probe_names_a_closed_outcome(self) -> None:
        for report in (
            sf.filter_action(_action([_entry("a", 3.0, 3.0, _limits(1.0, 2.0, 0.5))])),
            sf.filter_action(_action([_entry("a", 2.0, 2.5, BASE)],
                                     permission="dispatch.issue")),
            sf.filter_action(_action([_entry("a", 2.0, float("inf"), BASE)])),
        ):
            assert report["action_outcome"] in sf.FILTER_OUTCOMES
            assert report["action_outcome"] in sf.REFUSING_OUTCOMES


class TestClosedVocabularies:
    def test_filter_outcomes_exact_set(self) -> None:
        assert sf.FILTER_OUTCOMES == frozenset(
            {
                "passed_within_limits",
                "filtered_clamped_to_limits",
                "refused_permission_ceiling",
                "refused_nonfinite_action",
                "infeasible_no_safe_projection",
            }
        )

    def test_entry_outcomes_closed(self) -> None:
        assert sf.ENTRY_OUTCOMES == frozenset(
            {
                "within_limits",
                "clamped_to_limit",
                "refused_nonfinite_action",
                "infeasible_no_safe_projection",
            }
        )

    def test_refusing_outcomes_subset(self) -> None:
        assert sf.REFUSING_OUTCOMES <= sf.FILTER_OUTCOMES

    def test_action_kinds_closed_no_device_command_shape(self) -> None:
        assert sf.ACTION_KINDS == frozenset(
            {"setpoint_delta", "hold_current_setpoint"}
        )
        assert sf.FALLBACK_ACTION_KIND == "hold_current_setpoint"

    def test_report_schema_tag(self) -> None:
        assert sf.FILTER_REPORT_SCHEMA == "flexcore-marl-safety-filter/1"
        report = sf.filter_action(_action([_entry("a", 1.0, 1.0, BASE)]))
        assert report["report_schema"] == sf.FILTER_REPORT_SCHEMA
        assert report["permission_ceiling"] == sorted(sf.PERMISSION_CEILING)


class TestSelfTestRunner:
    def test_in_module_self_tests_all_pass(self) -> None:
        # The hand-computed self-tests are runnable WITHOUT pytest
        # (`python apps/worker/python/marl_worker/safety_filter.py`); this
        # mirror asserts every record passes. NEVER RUN in the writing
        # loop — the human gate runs it first.
        records = sf.run_self_tests()
        assert records, "the self-test runner returned no records"
        failures = [r for r in records if not r["passed"]]
        assert failures == [], failures
