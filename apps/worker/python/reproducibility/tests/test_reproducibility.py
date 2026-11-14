# P8.20 — The reproducibility-tooling test suite: the pytest mirror of the
#         in-module hand-computed self-tests (`seed_derivation.py`,
#         `record_schema.py`, `rerun_check.py` — each `run_self_tests`).
#         Pure logic only — NO framework, NO I/O, NO execution of any run.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. WRITTEN, NEVER RUN. pytest was never installed in this loop; no test
#    below has ever executed. Every expected value is HAND-COMPUTED or
#    THEOREM-DERIVED over constructed inputs, with the arithmetic or
#    reasoning stated in the in-module test detail strings — this mirror
#    asserts exactly the same expectations. This is TEST ARITHMETIC
#    pinning the schema and the comparison rules: it is NOT a rerun
#    outcome, NOT a seed result, and NOT evidence about any case. NO
#    reproducibility record exists anywhere and NO rerun has ever
#    happened (ENTRY_NOTE boundary 4).
# 2. Run instruction (the recorded procedure, from the repository root):
#    `PYTHONPATH=apps/worker/python/reproducibility python -m pytest
#    apps/worker/python/reproducibility/tests -v`. The in-module suites
#    run WITHOUT pytest:
#    `python apps/worker/python/reproducibility/<module>.py`.
# 3. What this suite pins (ledger P8.20, the parts provable without any
#    run): the recorded seed derivation against published FNV-1a 64
#    vectors and theorem-derived splitmix64 properties; the record
#    schema's refusals (missing fields, placeholders, closed
#    vocabularies, seed honesty, NT-10 dynamic separation, digest and
#    artifact-reference formats, the P8.38 tolerance reference); and the
#    rerun comparator's rules (identity before comparison, the injected
#    pre-registered tolerance with boundary inclusivity, the exclusion
#    marking for non-reproducing results, the P8.38 distribution-class
#    routing).
# ══════════════════════════════════════════════════════════════════════════

import seed_derivation
import record_schema
import rerun_check


def _all_passed(records: list) -> bool:
    return all(r["passed"] for r in records)


# ── seed_derivation.py ────────────────────────────────────────────────────


def test_seed_derivation_self_tests():
    records = seed_derivation.run_self_tests()
    assert records and _all_passed(records)


def test_fnv1a64_published_vectors():
    # Published FNV-1a 64 test vectors (draft-eastlake-xiii-fnv suite):
    # "" -> the offset basis (zero iterations); "a" and "foobar" ->
    # the published values. HAND-CITED, not invented.
    assert seed_derivation.fnv1a64(b"") == 0xcbf29ce484222325
    assert seed_derivation.fnv1a64(b"a") == 0xaf63dc4c8601ec8c
    assert seed_derivation.fnv1a64(b"foobar") == 0x85944171f73967e8


def test_splitmix64_mix_zero_is_zero():
    # Stated arithmetic: 0 ^ (0 >> k) = 0 and 0 * c = 0 mod 2^64.
    assert seed_derivation.splitmix64_mix(0) == 0


def test_derive_asset_seed_xor_structure():
    # seed = master XOR S implies seed XOR S == master (XOR self-inverse).
    key = "site-north-bess-1"
    s = seed_derivation.splitmix64_first(
        seed_derivation.fnv1a64(key.encode("utf-8"))
    )
    for master in (0, 1, 20260601):
        assert seed_derivation.derive_asset_seed(master, key) ^ s == master


def test_derive_asset_seed_deterministic_and_ranged():
    a = seed_derivation.derive_asset_seed(20260601, "site-north-bess-1")
    b = seed_derivation.derive_asset_seed(20260601, "site-north-bess-1")
    assert a == b
    assert 0 <= a <= 0xFFFFFFFFFFFFFFFF


def test_derive_asset_seed_refusals():
    for bad_call in (
        lambda: seed_derivation.derive_asset_seed("20260601", "k"),
        lambda: seed_derivation.derive_asset_seed(-1, "k"),
        lambda: seed_derivation.derive_asset_seed(2**64, "k"),
        lambda: seed_derivation.derive_asset_seed(20260601, ""),
    ):
        try:
            bad_call()
        except ValueError:
            pass
        else:
            raise AssertionError("expected ValueError")


def test_recorded_derivation_string_verbatim():
    assert seed_derivation.RECORDED_SEED_DERIVATION == (
        "seed(asset_key) = master_seed XOR splitmix64(FNV-1a-64(asset_key))"
    )


# ── record_schema.py ──────────────────────────────────────────────────────


def test_record_schema_self_tests():
    records = record_schema.run_self_tests()
    assert records and _all_passed(records)


def test_minimal_valid_record_zero_findings():
    assert record_schema.validate_record(record_schema._minimal_valid_record()) == []


def test_missing_field_named():
    record = record_schema._minimal_valid_record()
    del record["code_version"]
    errors = record_schema.validate_record(record)
    assert sum(1 for e in errors if e.startswith("FIELD_MISSING: code_version")) == 1


def test_placeholder_refused():
    record = record_schema._minimal_valid_record()
    record["code_version"] = "RECORD-AT-FIRST-RUN"
    assert any(
        e.startswith("PLACEHOLDER_PRESENT: code_version")
        for e in record_schema.validate_record(record)
    )


def test_schema_version_unknown_refused():
    record = record_schema._minimal_valid_record()
    record["record_schema_version"] = "flexcore-research/reproducibility-record.v2"
    assert any(
        e.startswith("SCHEMA_VERSION_UNKNOWN") for e in record_schema.validate_record(record)
    )


def test_seed_policy_conflict_refused():
    record = record_schema._minimal_valid_record()
    record["determinism"] = {
        "seed_supported": False,
        "seeds": {"asset": 1},
        "nondeterminism_sources": [],
    }
    assert any(
        e.startswith("SEED_POLICY_CONFLICT") for e in record_schema.validate_record(record)
    )


def test_dynamic_record_benchmark_set_forbidden():
    record = record_schema._minimal_valid_record()
    record["module_id"] = "dynamic-controller"
    record["component_versions"] = {"andes": "UNPINNED-REVERIFY"}
    record["benchmark_set_version"] = "flexcore-research-benchmark-set-v1"
    assert any(
        e.startswith("DYNAMIC_RECORD_BENCHMARK_SET_FORBIDDEN")
        for e in record_schema.validate_record(record)
    )


def test_tolerance_ref_required():
    record = record_schema._minimal_valid_record()
    record["module_id"] = "marl-policy"
    record["component_versions"] = {"rl_framework": "x"}
    record["benchmark_set_version"] = "flexcore-research-benchmark-set-v1"
    record["determinism"] = {
        "seed_supported": True,
        "seeds": {"t": 1},
        "seed_derivation": record_schema.RECORDED_SEED_DERIVATION,
        "nondeterminism_sources": ["gpu_nondeterminism"],
    }
    record["tolerance_class"] = "within_tolerance"
    assert any(
        e.startswith("TOLERANCE_REF_REQUIRED") for e in record_schema.validate_record(record)
    )


def test_digest_format_invalid_refused():
    record = record_schema._minimal_valid_record()
    record["config"]["config_digest"] = "deadbeef"
    assert any(
        e.startswith("DIGEST_FORMAT_INVALID") for e in record_schema.validate_record(record)
    )


def test_non_dict_record_refused():
    assert record_schema.validate_record([1, 2, 3]) == [
        "RECORD_NOT_A_DICT: expected a record object, got list"
    ]


# ── rerun_check.py ────────────────────────────────────────────────────────

_EXACT_TOLERANCE = {
    "class": "exact",
    "pre_registration_ref": "P8.38/baseline-highs-exact",
}
_WITHIN_TOLERANCE = {
    "class": "within_tolerance",
    "value": 0.25,
    "pre_registration_ref": "P8.38/baseline-highs-inference",
}


def test_rerun_check_self_tests():
    records = rerun_check.run_self_tests()
    assert records and _all_passed(records)


def test_exact_identical_outputs_reproduced():
    primary = rerun_check._exact_record()
    result = rerun_check.compare_outputs(
        primary["outputs_comparable"], primary["outputs_comparable"], _EXACT_TOLERANCE
    )
    assert result["verdict"] == "REPRODUCED" and result["max_deviation"] == 0.0


def test_exact_any_deviation_fails():
    primary = rerun_check._exact_record()
    rerun = rerun_check._exact_record()
    rerun["outputs_comparable"] = {
        "objective": 8.0000001,
        "feasible_interval_count": 96.0,
    }
    result = rerun_check.compare_outputs(
        primary["outputs_comparable"], rerun["outputs_comparable"], _EXACT_TOLERANCE
    )
    assert result["verdict"] == "NOT_REPRODUCED"


def test_within_tolerance_boundary_inclusive():
    # Stated arithmetic: 8.25 - 8.0 = 0.25 <= 0.25 (boundary inclusive).
    result = rerun_check.compare_outputs(
        {"objective": 8.0}, {"objective": 8.25}, _WITHIN_TOLERANCE
    )
    assert result["verdict"] == "REPRODUCED" and result["max_deviation"] == 0.25


def test_within_tolerance_beyond_boundary_fails():
    # |8.3 - 8.0| ~ 0.3 > 0.25 (binary floating point; asserted against
    # the decision, not a decimal literal).
    result = rerun_check.compare_outputs(
        {"objective": 8.0}, {"objective": 8.3}, _WITHIN_TOLERANCE
    )
    assert result["verdict"] == "NOT_REPRODUCED" and result["max_deviation"] > 0.25


def test_tolerance_not_pre_registered_refused():
    result = rerun_check.compare_outputs({"objective": 8.0}, {"objective": 8.0}, None)
    assert result["refusal"] == "TOLERANCE_NOT_PRE_REGISTERED"


def test_output_keys_differ_refused():
    result = rerun_check.compare_outputs(
        {"objective": 8.0},
        {"objective": 8.0, "runtime": 1.0},
        _EXACT_TOLERANCE,
    )
    assert result["refusal"] == "OUTPUT_KEYS_DIFFER"


def test_empty_outputs_refused():
    result = rerun_check.compare_outputs({}, {}, _EXACT_TOLERANCE)
    assert result["refusal"] == "OUTPUTS_EMPTY"


def test_distribution_blocked_to_p838():
    result = rerun_check.compare_outputs(
        {"objective": 8.0},
        {"objective": 8.0},
        {"class": "distribution", "pre_registration_ref": "P8.38/training"},
    )
    assert result["refusal"] == "DISTRIBUTION_RERUN_PROCEDURE_OWNED_BY_P8_38"


def test_identity_benchmark_version_mismatch_named():
    primary = rerun_check._exact_record()
    rerun = rerun_check._exact_record()
    rerun["benchmark_set_version"] = "flexcore-research-benchmark-set-v2"
    assert rerun_check.identity_findings(primary, rerun) == [
        "RERUN_IDENTITY_MISMATCH:benchmark_set_version"
    ]


def test_environment_version_differs_named():
    primary = rerun_check._exact_record()
    rerun = rerun_check._exact_record()
    rerun["component_versions"] = {"highs": "1.16.0"}
    assert rerun_check.identity_findings(primary, rerun) == [
        "ENVIRONMENT_VERSION_DIFFERS:highs:'1.15.1' vs '1.16.0'"
    ]


def test_identity_failure_blocks_verdict():
    primary = rerun_check._exact_record()
    rerun = rerun_check._exact_record()
    rerun["benchmark_set_version"] = "flexcore-research-benchmark-set-v2"
    verdict = rerun_check.rerun_verdict(primary, rerun, _EXACT_TOLERANCE)
    assert verdict["outcome"] == "INVALID_RERUN_IDENTITY"


def test_non_reproducing_marked_for_exclusion():
    primary = rerun_check._exact_record()
    rerun = rerun_check._exact_record()
    rerun["outputs_comparable"] = {
        "objective": 8.3,
        "feasible_interval_count": 96.0,
    }
    verdict = rerun_check.rerun_verdict(primary, rerun, _WITHIN_TOLERANCE)
    assert verdict["outcome"] == "NOT_REPRODUCED_MARKED_FOR_EXCLUSION"
    assert verdict["exclusion_statement"] is not None
    assert "EXCLUDED" in verdict["exclusion_statement"]


def test_reproduced_exact_verdict_outcome():
    primary = rerun_check._exact_record()
    rerun = rerun_check._exact_record()
    verdict = rerun_check.rerun_verdict(primary, rerun, _EXACT_TOLERANCE)
    assert verdict["outcome"] == "REPRODUCED_EXACT"


def test_string_outputs_exact_compared():
    assert rerun_check.compare_outputs(
        {"status": "COMPLETED"}, {"status": "COMPLETED"}, _WITHIN_TOLERANCE
    )["verdict"] == "REPRODUCED"
    assert rerun_check.compare_outputs(
        {"status": "COMPLETED"}, {"status": "FAILED"}, _WITHIN_TOLERANCE
    )["verdict"] == "NOT_REPRODUCED"


def test_string_number_mismatch_refused():
    result = rerun_check.compare_outputs(
        {"objective": 8.0}, {"objective": "eight"}, _EXACT_TOLERANCE
    )
    assert result["refusal"] == "OUTPUT_VALUE_TYPE_MISMATCH"
