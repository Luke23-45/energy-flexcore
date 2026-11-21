# FIM-F10.05 — pytest suite for the forecast evaluation harness (WRITTEN-
#              UNRUN: no pytest run was executed in this loop; the human
#              build/test gate owns the run. Every expectation below is
#              hand-computed arithmetic over CONSTRUCTED inputs, a
#              published SHA-256 test vector, or a float spelling
#              confirmed with `python -I` — the same mirror discipline as
#              test_forecast_baseline_worker.py / test_forecast_
#              publication.py. Nothing is fabricated.)
#
# HONESTY (FIM-F10.06): computing a metric over supplied series is
# mathematics, not a claim. Vendor feeds and evaluation runs are HUMAN
# GATES; the P15.05 result table stays EMPTY until a real backtesting run
# completes; no skill is claimed anywhere; P15-E-1/P15-E-2 stay OPEN; the
# synthetic fixture stays the LABELED fallback source (P15.16).
#
# Run instruction (human gate):
#   pytest apps/worker/python/forecast_worker/tests/

import importlib.util
from pathlib import Path

WORKER_DIR = Path(__file__).resolve().parent.parent

_spec = importlib.util.spec_from_file_location("forecast_evaluation", WORKER_DIR / "evaluation.py")
evaluation = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(evaluation)


# ---------------------------------------------------------------------------
# The published SHA-256 vectors (FIPS 180-4; the same pins the Rust chain
# carries)
# ---------------------------------------------------------------------------


def test_sha256_matches_published_vectors():
    assert evaluation.sha256_hex(b"") == (
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    )
    assert evaluation.sha256_hex(b"abc") == (
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    )


# ---------------------------------------------------------------------------
# FIM-F10.03 — the metric vocabulary (mirrors the Rust single home)
# ---------------------------------------------------------------------------


def test_metrics_match_the_hand_computed_values():
    observed = [(1000, 4.0), (2000, 4.0), (3000, 6.0), (4000, 6.0)]
    forecast = [(1000, 3.0), (2000, 4.0), (3000, 6.0), (4000, 6.0)]
    reference = [(1000, 3.0), (2000, 3.0), (3000, 5.0), (4000, 5.0)]
    result = evaluation.evaluate_against_persistence(observed, forecast, reference)
    # errors = [-1, 0, 0, 0] -> MAE 0.25; RMSE sqrt(1/4) = 0.5;
    # reference errors = [1, 1, 1, 1] -> MAE_ref 1.0; skill = 1 - 0.25/1.0 = 0.75.
    assert result["aligned_points"] == 4
    assert result["mae_mw"] == 0.25
    assert result["rmse_mw"] == 0.5
    assert result["reference_mae_mw"] == 1.0
    assert result["skill_score_vs_persistence"] == 0.75


def test_a_worse_model_scores_negative_skill():
    assert evaluation.skill_score_vs_persistence(2.0, 1.0) == -1.0


def test_zero_persistence_reference_is_a_named_refusal():
    import pytest

    with pytest.raises(evaluation.MetricRefusalError):
        evaluation.skill_score_vs_persistence(0.5, 0.0)


def test_length_mismatch_and_non_finite_inputs_are_named_refusals():
    import pytest

    with pytest.raises(evaluation.MetricRefusalError):
        evaluation.pointwise_errors([1.0, 2.0], [1.0])
    with pytest.raises(evaluation.MetricRefusalError):
        evaluation.pointwise_errors([float("nan")], [1.0])
    with pytest.raises(evaluation.MetricRefusalError):
        evaluation.mean_absolute_error([])


def test_the_evaluation_dict_carries_the_no_skill_honesty_fields():
    observed = [(1000, 4.0)]
    forecast = [(1000, 3.0)]
    reference = [(1000, 3.0)]
    result = evaluation.evaluate_against_persistence(observed, forecast, reference)
    assert "NO-SKILL-UNTIL-EVALUATED" in result["no_skill_claim"]
    assert result["p15_05_result_cells"] == "EMPTY (no evaluation run exists)"


# ---------------------------------------------------------------------------
# FIM-F10.03 — alignment: the exact timestamp join, gap refusal
# ---------------------------------------------------------------------------


def test_gap_in_observed_is_refused_never_interpolated():
    import pytest

    with pytest.raises(evaluation.AlignmentError):
        evaluation.align_series(
            [(1000, 4.0), (3000, 6.0)],
            [(1000, 3.0), (2000, 4.0), (3000, 6.0)],
        )


def test_duplicate_and_unsorted_series_are_refused():
    import pytest

    with pytest.raises(evaluation.AlignmentError):
        evaluation.align_series([(1000, 4.0), (1000, 5.0)], [(1000, 3.0)])
    with pytest.raises(evaluation.AlignmentError):
        evaluation.align_series([(2000, 4.0), (1000, 4.0)], [(1000, 3.0), (2000, 4.0)])


def test_exact_join_returns_shared_timestamps_in_order():
    aligned = evaluation.align_series(
        [(1000, 4.0), (2000, 4.0), (3000, 6.0)],
        [(1000, 3.0), (2000, 4.0), (3000, 6.0)],
    )
    assert aligned == [(1000, 3.0, 4.0), (2000, 4.0, 4.0), (3000, 6.0, 6.0)]


def test_persistence_reference_mirror_carries_the_last_value():
    assert evaluation.persistence_reference([-0.4, -0.5, -0.6], 3) == [-0.6, -0.6, -0.6]
    import pytest

    with pytest.raises(evaluation.MetricRefusalError):
        evaluation.persistence_reference([], 1)


# ---------------------------------------------------------------------------
# FIM-F10.04 — the artifact digest chain (the Rust chain's mirror)
# ---------------------------------------------------------------------------


def test_render_values_canonical_is_the_pinned_cross_language_format():
    assert evaluation.render_values_canonical([-0.6, -0.6]) == "-0.600000,-0.600000"
    assert evaluation.render_values_canonical([8.0, 1.0]) == "8.000000,1.000000"
    assert evaluation.render_values_canonical([42.5]) == "42.500000"


def _chain_records():
    payload_1 = evaluation.render_values_canonical([-0.6, -0.6])
    payload_2 = evaluation.render_values_canonical([8.0, 1.0])
    payload_3 = evaluation.render_values_canonical([42.5])
    record_1 = evaluation.build_artifact_record(
        artifact_ref="artifact:load:v1",
        artifact_kind="load",
        version="v1",
        produced_at_millis=1000,
        source_ref="vendor://fixture/feed",
        payload_canonical=payload_1,
        previous_head=evaluation.ARTIFACT_GENESIS_HEAD,
    )
    record_2 = evaluation.build_artifact_record(
        artifact_ref="artifact:pv_generation:v2",
        artifact_kind="pv_generation",
        version="v2",
        produced_at_millis=2000,
        source_ref="vendor://fixture/feed",
        payload_canonical=payload_2,
        previous_head=record_1["digest"],
    )
    record_3 = evaluation.build_artifact_record(
        artifact_ref="artifact:price:v3",
        artifact_kind="price",
        version="v3",
        produced_at_millis=3000,
        source_ref="vendor://fixture/feed",
        payload_canonical=payload_3,
        previous_head=record_2["digest"],
    )
    return [record_1, record_2, record_3]


def test_chain_digests_match_the_pinned_vectors():
    records = _chain_records()
    assert records[0]["payload_digest"] == (
        "668507de8a729fdcc4e208a8f1642fb64aa89dd62136abaa38e910986a4d17ff"
    )
    assert records[0]["digest"] == (
        "5fd7adb79b9dbe196a448878c97a504bcf55c15652cc623d6b6c983f34d4cf13"
    )
    assert records[1]["digest"] == (
        "dc5ffe60523f532c8ae44908cc6cf4b9675df54fa3e5bd513bc20977ccef9e86"
    )
    assert records[2]["digest"] == (
        "9dae0cd1f9f41fffbda4200587aa32472c3d4bc1e500dbee353e4eecca63d127"
    )


def test_chain_verification_walks_the_whole_chain():
    records = _chain_records()
    assert evaluation.verify_artifact_chain(records) == records[2]["digest"]


def test_a_tampered_payload_is_named_never_absorbed():
    import pytest

    records = _chain_records()
    forged = dict(records[0])
    forged["payload_canonical"] = evaluation.render_values_canonical([-0.7, -0.6])
    with pytest.raises(evaluation.ArtifactChainError):
        evaluation.verify_artifact_record(forged, evaluation.ARTIFACT_GENESIS_HEAD)


def test_a_stale_head_proof_is_refused():
    import pytest

    records = _chain_records()
    forged = dict(records[1])
    forged["previous_head"] = evaluation.ARTIFACT_GENESIS_HEAD
    with pytest.raises(evaluation.ArtifactChainError):
        evaluation.verify_artifact_record(forged, records[0]["digest"])


def test_closed_vocabulary_and_whitespace_rules():
    import pytest

    with pytest.raises(evaluation.ArtifactChainError):
        evaluation.build_artifact_record(
            artifact_ref="artifact:temperature:v1",
            artifact_kind="temperature",
            version="v1",
            produced_at_millis=1000,
            source_ref="vendor://fixture/feed",
            payload_canonical="1.000000",
            previous_head=evaluation.ARTIFACT_GENESIS_HEAD,
        )
    with pytest.raises(evaluation.ArtifactChainError):
        evaluation.build_artifact_record(
            artifact_ref="artifact:load:v1b",
            artifact_kind="load",
            version="v 1",
            produced_at_millis=1000,
            source_ref="vendor://fixture/feed",
            payload_canonical="1.000000",
            previous_head=evaluation.ARTIFACT_GENESIS_HEAD,
        )


def test_replay_requires_artifact_plus_inputs_identity():
    records = _chain_records()
    record_1 = records[0]
    replayed = evaluation.replay_artifact(
        record_1,
        {
            "artifact_ref": "artifact:load:v1",
            "artifact_kind": "load",
            "version": "v1",
            "produced_at_millis": 1000,
            "source_ref": "vendor://fixture/feed",
            "values": [-0.6, -0.6],
        },
    )
    assert replayed["digest"] == record_1["digest"]
    import pytest

    with pytest.raises(evaluation.ArtifactChainError):
        evaluation.replay_artifact(
            record_1,
            {
                "artifact_ref": "artifact:load:v1",
                "artifact_kind": "load",
                "version": "v1",
                "produced_at_millis": 1001,
                "source_ref": "vendor://fixture/feed",
                "values": [-0.6, -0.6],
            },
        )


# ---------------------------------------------------------------------------
# FIM-F10.02 — the fallback selection boundaries (the Rust composition's
# mirror; the P2.08 equality convention)
# ---------------------------------------------------------------------------


def test_age_at_max_age_is_fresh():
    decision = evaluation.select_ingested_forecast(
        {"values": [612.5, 610.0], "fetched_at_millis": 0},
        [-0.5, -0.6],
        2,
        evaluation.DEFAULT_MAX_AGE_MILLIS,
        evaluation.DEFAULT_MAX_AGE_MILLIS,
    )
    assert decision["decision"] == "live_ingested_forecast"
    assert decision["values"] == [612.5, 610.0]


def test_max_age_plus_one_falls_back_to_persistence():
    decision = evaluation.select_ingested_forecast(
        {"values": [612.5], "fetched_at_millis": 0},
        [-0.5, -0.6],
        2,
        evaluation.DEFAULT_MAX_AGE_MILLIS,
        evaluation.DEFAULT_MAX_AGE_MILLIS + 1,
    )
    assert decision["decision"] == "persistence_baseline_fallback"
    assert decision["reason"] == "max_age_exceeded"
    assert decision["values"] == [-0.6, -0.6]


def test_no_ingested_forecast_and_clock_before_fetch_fall_back():
    decision = evaluation.select_ingested_forecast(None, [-0.5, -0.6], 2, 3_600_000, 1_000)
    assert decision["decision"] == "persistence_baseline_fallback"
    assert decision["reason"] == "no_ingested_forecast"
    decision = evaluation.select_ingested_forecast(
        {"values": [612.5], "fetched_at_millis": 10_000},
        [-0.5, -0.6],
        2,
        3_600_000,
        5_000,
    )
    assert decision["reason"] == "clock_before_fetch"


def test_empty_history_escalates_to_the_labeled_synthetic_fixture():
    decision = evaluation.select_ingested_forecast(None, [], 2, 3_600_000, 1_000)
    assert decision["decision"] == "synthetic_fixture_fallback"
    assert decision["fixture_id"] == "synthetic-forecast-fixture"
    assert decision["provenance_label"] == "synthetic"


def test_fallback_outputs_carry_the_no_skill_posture():
    decision = evaluation.select_ingested_forecast(
        {"values": [612.5], "fetched_at_millis": 0},
        [-0.5, -0.6],
        2,
        evaluation.DEFAULT_MAX_AGE_MILLIS,
        evaluation.DEFAULT_MAX_AGE_MILLIS + 1,
    )
    assert "never claims skill" in decision["no_skill_claim"]


# ---------------------------------------------------------------------------
# The in-module self-tests (the same cases, run as one pytest case)
# ---------------------------------------------------------------------------


def test_the_in_module_self_tests_pass():
    results = evaluation.run_self_tests()
    failures = [r for r in results if not r["passed"]]
    assert failures == [], "failing self-tests: %s" % failures
