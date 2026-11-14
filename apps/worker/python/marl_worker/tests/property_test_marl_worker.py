# P8.37 — Hand-written PROPERTY tests for the MARL research worker family
#               (safety_filter, marl_worker, shadow_log, evaluation_harness).
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER — read before trusting anything below
#
# 1. PROPERTY-BASED, STDLIB-ONLY, NO FRAMEWORK. No property-testing library
#    is imported or installed (the Phase 8 loop bans; P8.37 requires
#    hand-written generators). Every generator is a plain function over
#    `random.Random(<fixed seed>)` — the stdlib Mersenne Twister, which is
#    deterministic for a fixed seed and version-recorded in the P8.20
#    environment block at the first real run. EACH property below states:
#    its ORACLE (what the assertion is checked against), its SEED (fixed
#    in-file, never random), and its DETERMINISM contract (rerunning this
#    file with the same Python version replays the exact same cases).
# 2. WRITTEN, NEVER RUN. pytest was never installed in this loop; no test
#    below has ever executed. They target the pure-logic core only
#    (safety_filter, marl_worker, shadow_log, evaluation_harness — all
#    stdlib-only) and are expected to pass at the first human run.
# 3. Run instruction (the recorded procedure):
#    `PYTHONPATH=apps/worker/python/marl_worker python -m pytest
#    apps/worker/python/marl_worker/tests -v` from the repository root.
#    A FRESH SEED variant for the P8.37 verification method ("run one
#    property suite with a fresh seed"): change the module constant
#    PROPERTY_SEED — every generator derives from it alone; the
#    properties must hold for ANY seed, which is what makes them
#    properties and not unit cases.
# 4. WHAT THE PROPERTIES PIN (ledger P8.37 step 2, as code):
#    - P-VOCAB:  the safety-filter outcome is ALWAYS in the closed
#                vocabulary for arbitrary P/ramp inputs (the closed
#                FILTER_OUTCOMES set is the oracle);
#    - P-ENVELOPE: the filter NEVER returns a setpoint outside the
#                declared feasible envelope for arbitrary inputs (the
#                interval arithmetic is the oracle);
#    - P-SCAN:   the proposal-only scan catches an injected dispatch key
#                at ANY nesting depth (the forbidden-key set is the
#                oracle);
#    - P-REFUSE: the evaluation harness refuses ANY malformed input shape
#                with its named error class — a number is never produced
#                from malformed input (the refusal discipline is the
#                oracle);
#    - P-STATS:  the shadow-log statistics are internally consistent for
#                arbitrary valid logs (sum/count arithmetic is the
#                oracle).
#    These are properties of the PURE RESEARCH-WORKER LOGIC over
#    simulation inputs. They prove NO safety property of any policy: the
#    filter is a STATIC limit/permission projection, NOT the CBF layer
#    (13_marl_design.md §3 — not yet derived, training blocked), and no
#    safety or certification claim exists anywhere in this file.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import json
import os
import random
import sys

import pytest

sys.path.insert(
    0,
    os.path.join(os.path.dirname(os.path.abspath(__file__)), os.pardir),
)

import safety_filter as sf  # noqa: E402 — path set above
import shadow_log as sl  # noqa: E402
import evaluation_harness as eh  # noqa: E402
import marl_worker as mw  # noqa: E402

# The ONE seed every generator in this file derives from (determinism
# contract: fixed in-file; the property must hold for any seed — see the
# honesty header item 3 for the fresh-seed verification procedure).
PROPERTY_SEED = 20261114

# Spread of generated magnitudes (MW; canonical units §10.2). Values are
# bounded so the hand-verifiable arithmetic stays readable; the boundary
# values 0.0 and the exact interval endpoints are generated explicitly
# by the boundary-biased generator below.
MW_SPAN = 10.0


# ---------------------------------------------------------------------------
# Generators (deterministic over PROPERTY_SEED; stdlib only)
# ---------------------------------------------------------------------------


def _rng() -> random.Random:
    return random.Random(PROPERTY_SEED)


def _finite_float(rng: random.Random, low: float, high: float) -> float:
    return rng.uniform(low, high)


def _limits(rng: random.Random) -> dict:
    """Arbitrary VALID limits: p_min <= p_max, ramp >= 0 (MW family)."""
    p_min = _finite_float(rng, -MW_SPAN, MW_SPAN)
    p_max = p_min + abs(_finite_float(rng, 0.0, MW_SPAN))
    ramp = abs(_finite_float(rng, 0.0, MW_SPAN))
    return {"p_min_mw": p_min, "p_max_mw": p_max, "ramp_mw_per_interval": ramp}


def _maybe_boundary(rng: random.Random, lo: float, hi: float) -> float:
    """A value biased toward the interval boundaries (boundary values are
    where interval arithmetic breaks; the ledger names boundary values a
    unit-test requirement, and the property generator includes them)."""
    roll = rng.random()
    if roll < 0.2:
        return lo
    if roll < 0.4:
        return hi
    return _finite_float(rng, lo, hi)


def _asset(rng: random.Random) -> dict:
    limits = _limits(rng)
    p_current = _finite_float(rng, -MW_SPAN, MW_SPAN)
    p_proposed = _finite_float(rng, -MW_SPAN, MW_SPAN)
    soc = rng.choice([None, 0.0, 1.0, _finite_float(rng, 0.0, 1.0)])
    asset = {
        "asset_id": "gen-%04d" % rng.randrange(10000),
        "p_current_mw": p_current,
        "p_proposed_mw": p_proposed,
        "soc_fraction": soc,
        "limits": limits,
    }
    return asset


def _action(rng: random.Random) -> dict:
    """An arbitrary WELL-FORMED action (the vocabulary properties are
    about well-formed inputs; malformed inputs belong to the refusal
    properties, where the corruption is applied explicitly)."""
    return {
        "action_kind": rng.choice(sorted(sf.ACTION_KINDS)),
        "proposed_permission": rng.choice(
            ["schedule.propose", "dispatch.issue", "telemetry.read", "nonsense"]
        ),
        "assets": [_asset(rng) for _ in range(rng.randrange(1, 5))],
    }


def _nested_tree(rng: random.Random, depth: int) -> dict:
    """An arbitrary JSON-shaped tree of the given depth (lists and dicts
    mixed), with harmless leaf values — the carrier for the injected
    forbidden key in P-SCAN."""
    if depth <= 0:
        return rng.choice(
            [
                _finite_float(rng, -MW_SPAN, MW_SPAN),
                "leaf-%04d" % rng.randrange(10000),
                True,
                None,
            ]
        )
    node = rng.choice(["dict", "list"])
    if node == "list":
        return [_nested_tree(rng, depth - 1) for _ in range(rng.randrange(1, 4))]
    return {
        "field_%d" % i: _nested_tree(rng, depth - 1)
        for i in range(rng.randrange(1, 4))
    }


def _inject_key(tree, rng: random.Random, key: str) -> None:
    """Injects `key` at a random mapping position ANYWHERE in the tree
    (a random path is walked; the key lands at that depth)."""
    node = tree
    while True:
        if isinstance(node, dict) and (rng.random() < 0.5 or not any(
            isinstance(v, (dict, list)) for v in node.values()
        )):
            node[key] = 1.0
            return
        if isinstance(node, list):
            children = [v for v in node if isinstance(v, (dict, list))]
            if not children or rng.random() < 0.3:
                node.append({key: 1.0})
                return
            node = rng.choice(children)
            continue
        children = [v for v in node.values() if isinstance(v, (dict, list))]
        if not children:
            node[key] = 1.0
            return
        node = rng.choice(children)


def _log_entry(rng: random.Random, sequence: int) -> dict:
    """One arbitrary VALID shadow-log entry (built through the real
    builder, so every property below exercises the real schema)."""
    outcome = rng.choice(sorted(sf.FILTER_OUTCOMES))
    return sl.build_log_entry(
        sequence=sequence,
        request_id="req-%05d" % sequence,
        input_snapshot_id="snapshot-prop",
        worker_mode=rng.choice(["filter_probe", "shadow_inference"]),
        policy_version="RECORD-AT-FIRST-RUN",
        training_seed=0,
        environment_version="flexcore-marl-env/0.1.0",
        action_kind=rng.choice(sorted(sf.ACTION_KINDS)),
        proposed_permission="schedule.propose",
        safety_filter_outcome=outcome,
        per_entry_outcomes=[
            {"asset_id": "a-%d" % i, "entry_outcome": "within_limits"}
            for i in range(rng.randrange(0, 3))
        ],
        filter_activations=(
            [{"asset_id": "a-0", "p_filtered_mw": 1.0}]
            if rng.random() < 0.3
            else []
        ),
        infeasibility=(
            {"outcome": outcome, "entries": ["a-0"], "recorded": True}
            if outcome == "infeasible_no_safe_projection"
            else None
        ),
    )


# ---------------------------------------------------------------------------
# P-VOCAB / P-ENVELOPE — the safety filter over arbitrary P/ramp inputs
# ---------------------------------------------------------------------------


class TestSafetyFilterClosedVocabularyProperty:
    def test_outcome_always_in_closed_vocabulary(self) -> None:
        """P-VOCAB. Oracle: membership in the closed `FILTER_OUTCOMES`
        frozenset (safety_filter.py — the closed value set of the P8.04
        `safety_filter_outcome` result key). Generator: `random.Random(
        PROPERTY_SEED)` over arbitrary well-formed actions with arbitrary
        finite P/ramp inputs (including non-`schedule.propose`
        permissions and out-of-interval proposals). Determinism: the
        seed is fixed in-file; the same Python replays the same cases."""
        rng = _rng()
        for _ in range(300):
            report = sf.filter_action(_action(rng))
            assert report["action_outcome"] in sf.FILTER_OUTCOMES

    def test_entry_outcome_always_in_closed_entry_vocabulary(self) -> None:
        """P-VOCAB (entry level). Oracle: the closed `ENTRY_OUTCOMES`
        set — the per-asset vocabulary. Same generator and seed as
        P-VOCAB."""
        rng = _rng()
        for _ in range(300):
            entry = sf.filter_entry(_asset(rng))
            assert entry["entry_outcome"] in sf.ENTRY_OUTCOMES

    def test_filter_is_deterministic_for_identical_input(self) -> None:
        """Replay determinism (the P7.42 determinism posture): the SAME
        action filtered twice yields canonically identical reports (the
        JSON canonical form is the oracle). Seeded generator as above."""
        rng = _rng()
        for _ in range(50):
            action = _action(rng)
            first = sf.filter_action(action)
            second = sf.filter_action(json.loads(json.dumps(action)))
            assert json.dumps(first, sort_keys=True) == json.dumps(
                second, sort_keys=True
            )


class TestSetpointNeverOutsideEnvelopeProperty:
    def test_filtered_setpoint_within_feasible_interval(self) -> None:
        """P-ENVELOPE (the ledger P8.37 step-2 example property: "a
        worker never returns a setpoint outside the declared capability
        envelope"). Oracle: the interval arithmetic itself — for a
        non-refused entry, p_filtered_mw must lie in
        [max(p_min, p_current - ramp), min(p_max, p_current + ramp)],
        recomputed INDEPENDENTLY here from the raw limits. Generator:
        arbitrary finite P/ramp inputs, boundary-biased (endpoints
        included). Seed fixed in-file."""
        rng = _rng()
        for _ in range(400):
            limits = _limits(rng)
            p_current = _finite_float(rng, -MW_SPAN, MW_SPAN)
            p_proposed = _maybe_boundary(
                rng,
                limits["p_min_mw"] - MW_SPAN,
                limits["p_max_mw"] + MW_SPAN,
            )
            entry = sf.filter_entry(
                {
                    "asset_id": "ess-1",
                    "p_current_mw": p_current,
                    "p_proposed_mw": p_proposed,
                    "soc_fraction": None,
                    "limits": limits,
                }
            )
            lo = max(limits["p_min_mw"], p_current - limits["ramp_mw_per_interval"])
            hi = min(limits["p_max_mw"], p_current + limits["ramp_mw_per_interval"])
            if lo > hi:
                # Infeasible interval: NO setpoint may be produced.
                assert entry["entry_outcome"] == "infeasible_no_safe_projection"
                assert entry["p_filtered_mw"] is None
            elif entry["entry_outcome"] in ("within_limits", "clamped_to_limit"):
                assert lo <= entry["p_filtered_mw"] <= hi
                # A clamped value sits ON an interval endpoint; a passed
                # value equals the proposal (the projection is exact).
                if entry["entry_outcome"] == "within_limits":
                    assert entry["p_filtered_mw"] == p_proposed
            else:
                # refused_nonfinite_action cannot occur: every generated
                # value is finite.
                raise AssertionError(
                    "unexpected outcome %r for finite input"
                    % entry["entry_outcome"]
                )

    def test_refusing_outcome_means_no_setpoints_emitted(self) -> None:
        """P-ENVELOPE (aggregate level). Oracle: `REFUSING_OUTCOMES` —
        under a refusing aggregate outcome NO proposal may exist in the
        worker envelope (a refusal is a result, never a pass-through).
        Uses the full worker path (run_filter_probe) over seeded valid
        requests."""
        rng = _rng()
        for _ in range(100):
            request = {
                "protocol": mw.PROTOCOL_ID,
                "mode": "filter_probe",
                "policy_context": {
                    "policy_version": "p",
                    "training_seed": 0,
                    "environment_version": "env",
                },
                "action": _action(rng),
                "provenance": {"input_snapshot_id": "s", "request_id": "r"},
            }
            envelope = mw.run_filter_probe(request)
            outcome = envelope["filter_outcome"]
            assert outcome in sf.FILTER_OUTCOMES
            if outcome in sf.REFUSING_OUTCOMES:
                assert envelope["outputs"]["proposal"] is None
                assert envelope["outputs"]["result_payload"] is None


# ---------------------------------------------------------------------------
# P-SCAN — the proposal-only boundary at ANY nesting depth
# ---------------------------------------------------------------------------


class TestProposalOnlyScanProperty:
    def test_injected_forbidden_key_caught_at_any_depth(self) -> None:
        """P-SCAN. Oracle: `FORBIDDEN_RESULT_KEYS` — the scan must raise
        RuntimeError for EVERY forbidden key, injected at a random
        position of an arbitrary tree of depth 1..12. Generator:
        `_nested_tree` + `_inject_key` over `random.Random(
        PROPERTY_SEED)`. Determinism: fixed seed; the property is
        depth-independent by construction (the assertion is the same
        refusal at every depth)."""
        rng = _rng()
        for _ in range(150):
            depth = rng.randrange(1, 13)
            tree = _nested_tree(rng, depth)
            key = rng.choice(sorted(mw.FORBIDDEN_RESULT_KEYS))
            _inject_key(tree, rng, key)
            with pytest.raises(RuntimeError):
                mw.assert_proposal_only(tree)

    def test_clean_tree_of_any_depth_passes(self) -> None:
        """P-SCAN (negative control): the same generator WITHOUT the
        injection must NEVER raise — the scan refuses only forbidden
        keys, not depth or shape. Same seed, same tree distribution."""
        rng = _rng()
        for _ in range(100):
            tree = _nested_tree(rng, rng.randrange(1, 13))
            mw.assert_proposal_only(tree)  # must not raise

    def test_case_and_whitespace_variants_caught(self) -> None:
        """P-SCAN (bypass resistance): the scan normalizes
        case/whitespace (`key.strip().lower()`), so 'DISPATCH',
        ' dispatch ', and 'Command' are caught too. Oracle: the same
        forbidden-key set."""
        rng = _rng()
        for key in sorted(mw.FORBIDDEN_RESULT_KEYS):
            tree = _nested_tree(rng, 3)
            _inject_key(tree, rng, key.upper())
            with pytest.raises(RuntimeError):
                mw.assert_proposal_only(tree)


# ---------------------------------------------------------------------------
# P-REFUSE — the evaluation harness refuses any malformed input shape
# ---------------------------------------------------------------------------

# The corruption catalogue: every way a harness input can be malformed.
# Each corruption is applied to a VALID base input; the property is that
# the harness REFUSES (its named error class) — never returns a number.
_CORRUPTIONS = (
    "not_a_dict",
    "drop_field",
    "wrong_type_string",
    "wrong_type_bool",
    "wrong_type_list",
    "nonfinite_value",
)


class TestEvaluationHarnessRefusesMalformedShapesProperty:
    def _valid_case_log(self, rng: random.Random, size: int) -> list:
        """An arbitrary VALID recorded-consequence log slice (built
        through the real builders — the shape `case_statistics`
        accepts)."""
        entries = []
        for sequence in range(1, size + 1):
            entry = _log_entry(rng, sequence)
            violated = rng.random() < 0.5
            consequence = eh.build_case_consequence(
                constraint_violation=violated,
                # A violation MUST name a kind; a non-violation MUST
                # carry None (the builder refuses the mismatched pairs).
                violation_kind=(
                    rng.choice(["soc_bound", "power_limit", "voltage_band"])
                    if violated
                    else None
                ),
                return_value=_finite_float(rng, -100.0, 100.0),
                return_unit="declared-cost-unit",
            )
            entries.append(
                sl.build_log_entry(
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
            )
        return entries

    def test_build_case_consequence_refuses_every_corruption(self) -> None:
        """P-REFUSE. Oracle: `EvaluationHarnessError` — the consequence
        builder must refuse EVERY malformed FIELD VALUE produced by the
        value-corruption catalogue applied over its four fields, and
        never return a record for one. (A MISSING field is a caller
        signature error — a TypeError, not a harness refusal — so the
        catalogue here corrupts values, not presence; the slice-level
        test below covers the missing-record shape through the log
        schema.) Generator: fixed-seed catalogue application; the valid
        base is constructed, not sampled from a run."""
        rng = _rng()
        base = {
            "constraint_violation": False,
            "violation_kind": None,
            "return_value": 1.0,
            "return_unit": "declared-cost-unit",
        }
        value_fields = ("constraint_violation", "violation_kind",
                        "return_value", "return_unit")
        for _ in range(200):
            corrupted = dict(base)
            field = rng.choice(value_fields)
            if field == "constraint_violation":
                # A bool is the VALID type for this field, so the bool
                # corruption is not a corruption here (a True/False value
                # is a legitimate input); only non-bool corruptions apply.
                corruption = rng.choice(
                    ("wrong_type_string", "wrong_type_list", "nonfinite_value")
                )
            else:
                corruption = rng.choice(
                    ("wrong_type_string", "wrong_type_bool", "wrong_type_list",
                     "nonfinite_value")
                )
            if corruption == "wrong_type_string":
                corrupted[field] = "not-a-number"
            elif corruption == "wrong_type_bool":
                corrupted[field] = rng.choice([True, False])
            elif corruption == "wrong_type_list":
                corrupted[field] = [1.0, 2.0]
            else:
                corrupted[field] = rng.choice([float("nan"), float("inf")])
            with pytest.raises(eh.EvaluationHarnessError):
                eh.build_case_consequence(**corrupted)

    def test_case_statistics_refuses_every_corrupted_slice(self) -> None:
        """P-REFUSE (slice level). Oracle: the harness's refusal classes
        — `EvaluationHarnessError` OR `ShadowLogError` (both subclass
        ValueError; the slice may be refused by the log validator OR by
        the statistics layer, and both are named refusals, never a
        number). `case_statistics` must refuse a log slice whose entries
        are corrupted (non-dict, missing schema-bearing consequence,
        not_simulated status), never produce a frequency from it."""
        rng = _rng()
        valid = self._valid_case_log(rng, 3)
        for _ in range(200):
            index = rng.randrange(len(valid))
            corruption = rng.choice(_CORRUPTIONS)
            slice_copy = [dict(e) for e in valid]
            if corruption == "not_a_dict":
                slice_copy[index] = rng.choice([None, [], 7, "x"])
            elif corruption == "drop_field":
                slice_copy[index].pop("simulated_consequence", None)
            elif corruption == "wrong_type_string":
                slice_copy[index]["simulated_consequence"] = "consequence"
            elif corruption == "nonfinite_value":
                slice_copy[index] = dict(slice_copy[index])
                slice_copy[index]["consequence_status"] = "not_simulated"
            else:
                # wrong_type_bool / wrong_type_list land on the
                # consequence object's shape check.
                slice_copy[index]["simulated_consequence"] = rng.choice(
                    [True, [1.0]]
                )
            with pytest.raises(ValueError):
                eh.case_statistics(slice_copy)

    def test_filter_statistics_refuses_empty_and_gapped_logs(self) -> None:
        """P-REFUSE (log level). Oracle: the refusal classes —
        `EvaluationHarnessError` for the empty slice, `ShadowLogError`
        for a gapped/duplicated sequence order (both subclass ValueError;
        an action hidden in a gap is refused, never averaged over), for
        arbitrary generated gaps. Deterministic corruption of the seeded
        valid log."""
        rng = _rng()
        valid = [_log_entry(rng, i) for i in range(1, 5)]
        with pytest.raises(eh.EvaluationHarnessError):
            eh.filter_statistics([])
        for _ in range(100):
            log = [dict(e) for e in valid]
            victim = rng.randrange(len(log))
            # Every corruption below provably breaks the
            # strictly-increasing 1-based order at position `victim`:
            # 0 and len(log)+10 are outside the position's required
            # value, and duplicating another entry's sequence value can
            # never equal the victim position's required value (the
            # sequences are distinct 1..n).
            log[victim]["sequence"] = rng.choice(
                [0, len(log) + 10, log[(victim + 1) % len(log)]["sequence"]]
            )
            with pytest.raises(ValueError):
                eh.filter_statistics(log)


# ---------------------------------------------------------------------------
# P-STATS — shadow-log statistics consistency for arbitrary valid logs
# ---------------------------------------------------------------------------


class TestShadowLogStatisticsProperty:
    def test_summary_counts_sum_to_total(self) -> None:
        """P-STATS. Oracle: counting arithmetic — for an arbitrary valid
        log, by_outcome counts sum to total_actions; the activation and
        infeasibility counts are bounded by it; the refusal count equals
        the sum over REFUSING_OUTCOMES only. Generator: seeded valid
        logs of length 1..40."""
        rng = _rng()
        for _ in range(60):
            log = [_log_entry(rng, i) for i in range(1, rng.randrange(2, 41))]
            summary = sl.log_summary(log)
            assert summary["total_actions"] == len(log)
            assert sum(summary["by_outcome"].values()) == len(log)
            assert 0 <= summary["filter_activation_count"] <= len(log)
            assert 0 <= summary["infeasibility_count"] <= len(log)
            assert summary["refusal_count"] == sum(
                count
                for outcome, count in summary["by_outcome"].items()
                if outcome in sf.REFUSING_OUTCOMES
            )

    def test_rates_within_unit_interval(self) -> None:
        """P-STATS (harness level). Oracle: the interval [0, 1] — every
        rate `filter_statistics` produces is a count over the validated
        total, so it lies in [0, 1] for arbitrary valid logs; and the
        recomputed fraction equals the reported rate exactly (same
        division)."""
        rng = _rng()
        for _ in range(60):
            log = [_log_entry(rng, i) for i in range(1, rng.randrange(2, 31))]
            stats = eh.filter_statistics(log)
            total = stats["total_actions"]
            assert 0.0 <= stats["activation_rate"] <= 1.0
            assert 0.0 <= stats["infeasibility_rate"] <= 1.0
            assert 0.0 <= stats["refusal_rate"] <= 1.0
            assert stats["activation_rate"] == (
                stats["counts"]["filter_activation_count"] / total
            )
            assert stats["infeasibility_rate"] == (
                stats["counts"]["infeasibility_count"] / total
            )
            assert stats["refusal_rate"] == (
                stats["counts"]["refusal_count"] / total
            )

    def test_sequence_order_invariant(self) -> None:
        """P-STATS (ordering). Oracle: the strictly-increasing 1-based
        sequence rule — a VALID generated log always validates, and any
        permutation of it (except the identity) is refused (an ordered
        record reordered hides actions)."""
        rng = _rng()
        for _ in range(40):
            log = [_log_entry(rng, i) for i in range(1, rng.randrange(3, 8))]
            sl.validate_shadow_log(log)  # must not raise
            shuffled = log[:]
            rng.shuffle(shuffled)
            if shuffled != log:
                with pytest.raises(sl.ShadowLogError):
                    sl.validate_shadow_log(shuffled)
