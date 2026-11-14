# P8.20 — The rerun test, as a pure stdlib comparator: identity check,
#         output comparison against the PRE-REGISTERED tolerance, and the
#         rerun-verdict assembly. Over INJECTED records only.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. WHAT THIS MODULE IS. Ledger P8.20 steps 3–4: "Implement the rerun
#    test: a second engineer re-runs a selected result from the record
#    alone on a clean environment and confirms reproduction within the
#    tolerance recorded for that module BEFORE the rerun ... Record the
#    rerun outcome; any result that does not reproduce is marked as such
#    and excluded from the benchmark report's claims." This module is the
#    comparator the second engineer's rerun record is checked with: it
#    (a) verifies the rerun is OF THE SAME RESULT (identity fields),
#    (b) compares the primary and rerun outputs within the PRE-REGISTERED
#    tolerance injected by the caller, and (c) assembles the verdict with
#    the exclusion marking for non-reproducing results. It performs NO
#    I/O and runs nothing — the re-running is a human/lab act; this code
#    only judges the records.
# 2. WRITTEN, NEVER RUN; NO RERUN HAS HAPPENED. The Phase 8 loop bans
#    mean this comparator has never executed and NO rerun-test record
#    exists anywhere (the rerun tables in `20_reproducibility.md` are
#    EMPTY templates). The self-tests carry HAND-COMPUTED arithmetic with
#    the arithmetic STATED in each test (8.0 vs 8.0 -> deviation 0.0;
#    8.25 - 8.0 = 0.25 <= 0.25 -> reproduced; 8.3 - 8.0 = 0.3 > 0.25 ->
#    not reproduced). They pin the COMPARISON RULES; they are not a
#    rerun outcome and not evidence about any case.
# 3. THE TOLERANCE IS INJECTED, NEVER DERIVED (X-8). This module never
#    computes, stores, defaults, or suggests a tolerance. A comparison
#    without the caller-injected pre-registered tolerance is refused
#    (TOLERANCE_NOT_PRE_REGISTERED — mirroring the P8.09/P8.15 harness
#    discipline); a "distribution" rerun (training) is refused here with
#    a pointer to its owner, P8.38 — comparing single numbers against a
#    distribution locally would be exactly the class confusion the ledger
#    names ("a training rerun compared with an inference tolerance and
#    declared a failure — or vice versa").
# 4. IDENTITY BEFORE COMPARISON. A rerun that does not match the primary
#    record on the identity fields (same benchmark-set version, same code
#    version, same seeds, same config digest, same input snapshots and
#    artifact digests, same dependency versions) is NOT a rerun of that
#    result: the outcome is INVALID_RERUN_IDENTITY (a procedural
#    failure), never a reproduction verdict. Environment-version
#    differences are named findings (ENVIRONMENT_VERSION_DIFFERS), never
#    absorbed — the ledger's risk "same seed, different data" and its
#    version-sensitivity risk both live here.
# 5. EXACTNESS IS A DECLARED CLASS, NOT AN ASSUMPTION. An "exact" record
#    (the deterministic modules: the ADMM provider — seed_supported =
#    false, byte-equality discipline; HiGHS at thread_count 1 with a
#    fixed caller seed; Clarabel; the rule-based baseline) is compared at
#    zero deviation. A "within_tolerance" record (ML inference reruns,
#    with GPU nondeterminism DISCLOSED in the record, P8.38) is compared
#    at the pre-registered value. Which class applies is recorded on the
#    record (record_schema.v1 tolerance_class), never inferred here.
# 6. REPORTING ONLY — NEVER A DISPATCH INPUT (ADR-010). The namespace
#    contains NO dispatch/issue/authorize/approve/command-shaped callable
#    (pinned by a self-test); a rerun verdict is reproducibility
#    bookkeeping, nothing else.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import json
import math

REPORTING_ONLY_STATEMENT = (
    "P8.20 reproducibility tooling — reporting/bookkeeping only; the "
    "output never influences dispatch and carries no dispatch-shaped "
    "result (ADR-010; simulation-only, AGENTS.md rule 5)."
)

# The identity fields: a rerun must match the primary record on ALL of
# them (compared on their canonical JSON form, sort_keys=True, so dict
# ordering never changes the answer). benchmark_set_version is compared
# through the same loop — for dynamic records neither side carries one
# (record_schema.v1, NT-10), so two absent values are equal, while a
# rerun that adds or changes one is a named mismatch.
RERUN_IDENTITY_FIELDS = (
    "record_schema_version",
    "module_id",
    "module_version",
    "code_version",
    "config_digest",
    "seeds",
    "input_snapshot_ids",
    "input_artifact_digests",
    "benchmark_set_version",
)


def _canonical(value: object) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def _flatten_identity(record: dict) -> dict:
    """Extract the identity fields from a record-shaped dict.

    ``config_digest`` is read from record['config']['config_digest'],
    ``seeds`` from record['determinism']['seeds'] (None when the module
    supports no seed — which is itself part of the identity), and the
    benchmark-set version from the top level (None when absent, i.e.
    dynamic records).
    """
    return {
        "record_schema_version": record.get("record_schema_version"),
        "module_id": record.get("module_id"),
        "module_version": record.get("module_version"),
        "code_version": record.get("code_version"),
        "config_digest": (record.get("config") or {}).get("config_digest")
        if isinstance(record.get("config"), dict)
        else None,
        "seeds": (record.get("determinism") or {}).get("seeds")
        if isinstance(record.get("determinism"), dict)
        else None,
        "input_snapshot_ids": (record.get("inputs") or {}).get("input_snapshot_ids")
        if isinstance(record.get("inputs"), dict)
        else None,
        "input_artifact_digests": (record.get("inputs") or {}).get(
            "input_artifact_digests"
        )
        if isinstance(record.get("inputs"), dict)
        else None,
        "benchmark_set_version": record.get("benchmark_set_version"),
    }


def identity_findings(primary: dict, rerun: dict) -> list:
    """Compare the identity fields of the primary and rerun records.

    Returns a list of findings, each a named string:
      - ``RERUN_IDENTITY_MISMATCH:<field>`` — both records carry the
        field and it differs;
      - ``RERUN_IDENTITY_FIELD_MISSING:<field>`` — the rerun record does
        not carry a field the primary carries;
      - ``ENVIRONMENT_VERSION_DIFFERS:<component>:<primary> vs <rerun>``
        — a dependency version differs between the records (recorded as
        a finding, never absorbed: a rerun on different dependency
        versions did not honor the record it claims to re-run).

    An EMPTY list means the rerun is an identity-true rerun.
    """
    findings: list = []
    flat_primary = _flatten_identity(primary)
    flat_rerun = _flatten_identity(rerun)
    for field in RERUN_IDENTITY_FIELDS:
        p_value = flat_primary.get(field)
        r_value = flat_rerun.get(field)
        if r_value is None and p_value is not None:
            findings.append(f"RERUN_IDENTITY_FIELD_MISSING:{field}")
        elif _canonical(p_value) != _canonical(r_value):
            findings.append(f"RERUN_IDENTITY_MISMATCH:{field}")

    # Dependency versions: same keys, same values — differences are
    # named findings.
    p_versions = primary.get("component_versions") or {}
    r_versions = rerun.get("component_versions") or {}
    for key in sorted(set(p_versions) | set(r_versions)):
        p_value = p_versions.get(key)
        r_value = r_versions.get(key)
        if p_value != r_value:
            findings.append(
                f"ENVIRONMENT_VERSION_DIFFERS:{key}:{p_value!r} vs {r_value!r}"
            )
    return findings


def compare_outputs(
    primary_outputs: dict, rerun_outputs: dict, tolerance: object
) -> dict:
    """Compare primary vs rerun outputs within the injected tolerance.

    ``tolerance`` is the PRE-REGISTERED tolerance, injected by the caller
    from the P8.38 pre-registration record — never computed here:
      - ``{"class": "exact", "pre_registration_ref": ...}`` — zero
        deviation required;
      - ``{"class": "within_tolerance", "value": <number >= 0>,
        "pre_registration_ref": ...}`` — every output within the value;
      - ``{"class": "distribution", ...}`` — REFUSED here with a pointer
        to P8.38 (the owner of the distribution-comparison procedure).

    Output values must be finite numbers or strings; strings are
    compared for exact equality in every class (a status label is not a
    quantity). Returns a result dict; a refusal dict carries "refusal".
    """
    if tolerance is None:
        return {
            "refusal": "TOLERANCE_NOT_PRE_REGISTERED",
            "detail": (
                "no pre-registered tolerance was injected — a comparison "
                "without one is exactly the X-8 failure (fitted after the "
                "fact); P8.38 owns the per-module pre-registration"
            ),
            "usage": REPORTING_ONLY_STATEMENT,
        }
    if not isinstance(tolerance, dict):
        return {
            "refusal": "TOLERANCE_MALFORMED",
            "detail": f"expected a tolerance dict, got {type(tolerance).__name__}",
            "usage": REPORTING_ONLY_STATEMENT,
        }
    tolerance_class = tolerance.get("class")
    ref = tolerance.get("pre_registration_ref")
    if tolerance_class not in ("exact", "within_tolerance", "distribution"):
        return {
            "refusal": "TOLERANCE_CLASS_INVALID",
            "detail": f"{tolerance_class!r} is not a declared tolerance class",
            "usage": REPORTING_ONLY_STATEMENT,
        }
    if not isinstance(ref, str) or not ref:
        return {
            "refusal": "TOLERANCE_REF_MISSING",
            "detail": "the pre-registration reference must travel with the "
            "tolerance (the value alone proves nothing about when it was set)",
            "usage": REPORTING_ONLY_STATEMENT,
        }
    if tolerance_class == "distribution":
        return {
            "refusal": "DISTRIBUTION_RERUN_PROCEDURE_OWNED_BY_P8_38",
            "detail": (
                "a training rerun is compared against a distribution by the "
                "P8.38 procedure — comparing single numbers against a "
                "distribution here would be the exact class confusion the "
                "ledger names as P8.38's risk"
            ),
            "usage": REPORTING_ONLY_STATEMENT,
        }
    if not isinstance(primary_outputs, dict) or not isinstance(rerun_outputs, dict):
        return {
            "refusal": "OUTPUTS_NOT_A_DICT",
            "detail": "both primary and rerun outputs must be dicts",
            "usage": REPORTING_ONLY_STATEMENT,
        }

    p_keys = set(primary_outputs)
    r_keys = set(rerun_outputs)
    if p_keys != r_keys:
        return {
            "refusal": "OUTPUT_KEYS_DIFFER",
            "detail": (
                f"primary-only keys {sorted(p_keys - r_keys)}, rerun-only keys "
                f"{sorted(r_keys - p_keys)} — comparing different quantities "
                "is not a comparison"
            ),
            "usage": REPORTING_ONLY_STATEMENT,
        }
    if not p_keys:
        return {
            "refusal": "OUTPUTS_EMPTY",
            "detail": "an empty output set cannot become a verdict of zero "
            "deviation (an empty denominator cannot become a rate of zero)",
            "usage": REPORTING_ONLY_STATEMENT,
        }

    if tolerance_class == "within_tolerance":
        value = tolerance.get("value")
        if isinstance(value, bool) or not isinstance(value, (int, float)):
            return {
                "refusal": "TOLERANCE_VALUE_INVALID",
                "detail": f"tolerance value {value!r} is not a number",
                "usage": REPORTING_ONLY_STATEMENT,
            }
        if not math.isfinite(value) or value < 0:
            return {
                "refusal": "TOLERANCE_VALUE_INVALID",
                "detail": f"tolerance value {value!r} must be finite and >= 0",
                "usage": REPORTING_ONLY_STATEMENT,
            }

    deviations: dict = {}
    max_deviation = 0.0
    reproduced = True
    for name in sorted(p_keys):
        p_value = primary_outputs[name]
        r_value = rerun_outputs[name]
        if isinstance(p_value, str) or isinstance(r_value, str):
            if not isinstance(p_value, str) or not isinstance(r_value, str):
                return {
                    "refusal": "OUTPUT_VALUE_TYPE_MISMATCH",
                    "detail": f"output {name!r}: a string is never compared "
                    "against a number",
                    "usage": REPORTING_ONLY_STATEMENT,
                }
            deviation = 0.0 if p_value == r_value else None
            deviations[name] = "equal" if p_value == r_value else "STRING_MISMATCH"
            if p_value != r_value:
                reproduced = False
            continue
        for value in (p_value, r_value):
            if (
                isinstance(value, bool)
                or not isinstance(value, (int, float))
                or not math.isfinite(value)
            ):
                return {
                    "refusal": "NON_FINITE_OR_NON_NUMERIC_OUTPUT",
                    "detail": f"output {name!r} carries {value!r} — a "
                    "non-finite or non-numeric output is recorded as the "
                    "run's failure, never compared as a quantity",
                    "usage": REPORTING_ONLY_STATEMENT,
                }
        deviation = abs(float(p_value) - float(r_value))
        deviations[name] = deviation
        if deviation > max_deviation:
            max_deviation = deviation
        if tolerance_class == "exact":
            if deviation != 0.0:
                reproduced = False
        else:  # within_tolerance
            if deviation > float(tolerance["value"]):
                reproduced = False

    return {
        "verdict": "REPRODUCED" if reproduced else "NOT_REPRODUCED",
        "tolerance_class": tolerance_class,
        "pre_registration_ref": ref,
        "deviations": deviations,
        "max_deviation": max_deviation,
        "usage": REPORTING_ONLY_STATEMENT,
    }


def rerun_verdict(
    primary: dict, rerun: dict, pre_registered_tolerance: object
) -> dict:
    """Assemble the full rerun-test verdict (ledger P8.20 step 4).

    Order of judgment (fail-closed):
      1. identity findings -> INVALID_RERUN_IDENTITY (a procedural
         failure: the rerun is not of the recorded result);
      2. a "distribution"-class primary -> BLOCKED_DISTRIBUTION (P8.38
         owns the procedure);
      3. otherwise the output comparison under the injected
         pre-registered tolerance, with the exclusion marking on a
         non-reproducing result.

    The exclusion marking is the ledger step-4 deliverable: a
    non-reproducing result is MARKED and EXCLUDED from the benchmark
    report's claims (P8.26 consumes the marking; it is a standing
    exclusion, not a footnote).
    """
    findings = identity_findings(primary, rerun)
    if findings:
        return {
            "outcome": "INVALID_RERUN_IDENTITY",
            "findings": findings,
            "detail": (
                "the rerun record does not identify as the recorded result "
                "(same benchmark-set version, code version, seeds, config, "
                "inputs, dependency versions) — no reproduction verdict is "
                "possible; fix the rerun procedure, not the finding"
            ),
            "usage": REPORTING_ONLY_STATEMENT,
        }
    tolerance_class = primary.get("tolerance_class")
    if tolerance_class == "distribution":
        return {
            "outcome": "BLOCKED_DISTRIBUTION",
            "findings": [],
            "detail": (
                "a distribution-class (training) rerun is compared by the "
                "P8.38 procedure, never by a local single-number comparison"
            ),
            "usage": REPORTING_ONLY_STATEMENT,
        }
    comparison = compare_outputs(
        primary.get("outputs_comparable") or {},
        rerun.get("outputs_comparable") or {},
        pre_registered_tolerance,
    )
    if "refusal" in comparison:
        return {
            "outcome": "BLOCKED_" + comparison["refusal"],
            "findings": [],
            "detail": comparison["detail"],
            "usage": REPORTING_ONLY_STATEMENT,
        }
    if comparison["verdict"] == "REPRODUCED":
        outcome = (
            "REPRODUCED_EXACT"
            if tolerance_class == "exact"
            else "REPRODUCED_WITHIN_PRE_REGISTERED_TOLERANCE"
        )
        exclusion = None
    else:
        outcome = "NOT_REPRODUCED_MARKED_FOR_EXCLUSION"
        exclusion = (
            "MARKED NON-REPRODUCING within the pre-registered tolerance "
            f"({comparison['pre_registration_ref']}) — EXCLUDED from the "
            "benchmark report's claims (P8.20 ledger step 4; P8.26 must "
            "not cite this result)"
        )
    return {
        "outcome": outcome,
        "findings": [],
        "comparison": comparison,
        "exclusion_statement": exclusion,
        "usage": REPORTING_ONLY_STATEMENT,
    }


# ── Hand-computed self-tests ──────────────────────────────────────────────


def _exact_record() -> dict:
    """A HAND-CONSTRUCTED exact-class record body carrying the fields the
    identity check reads. ``outputs_comparable`` is the injected dict of
    numeric/string outputs EXTRACTED FROM THE STORED ARTIFACTS at rerun
    time — the schema record itself carries only artifact references and
    digests (record_schema.v1); the comparable numbers are never stored
    in the record, so the comparison is always against the stored,
    digest-covered artifact content."""
    return {
        "record_schema_version": "flexcore-research/reproducibility-record.v1",
        "module_id": "baseline/highs-tier1-lp",
        "module_version": "flexcore-research/baselines.v1",
        "code_version": "flexcore-baseline-freeze-2026-11-14",
        "benchmark_set_version": "flexcore-research-benchmark-set-v1",
        "config": {"config_digest": "fnv1a64:" + "0" * 16},
        "determinism": {
            "seed_supported": True,
            "seeds": {"site-north-bess-1": 20260601},
        },
        "inputs": {
            "input_snapshot_ids": ["snapshot-lib02-v1"],
            "input_artifact_digests": {"scenario-v2.json": "sha256:" + "c" * 64},
        },
        "component_versions": {"highs": "1.15.1"},
        "tolerance_class": "exact",
        "outputs_comparable": {"objective": 8.0, "feasible_interval_count": 96.0},
    }


def run_self_tests() -> list:
    """Run every self-test; return one record per test.

    Every expectation is HAND-COMPUTED with the arithmetic STATED. These
    pin the COMPARISON RULES; they are not a rerun outcome, not a seed
    result, and not evidence about any case.
    """
    results: list = []

    def check(name: str, condition: bool, detail: str) -> None:
        results.append(
            {
                "test": name,
                "passed": bool(condition),
                "detail": detail,
                "usage": REPORTING_ONLY_STATEMENT,
            }
        )

    exact_tolerance = {
        "class": "exact",
        "pre_registration_ref": "P8.38/baseline-highs-exact",
    }

    # (1) Exact class, identical outputs: 8.0 vs 8.0 -> deviation 0.0
    #     (8.0 - 8.0 = 0.0), 96 vs 96 -> 0.0 — REPRODUCED.
    primary = _exact_record()
    rerun = _exact_record()
    result = compare_outputs(
        primary["outputs_comparable"], rerun["outputs_comparable"], exact_tolerance
    )
    check(
        "exact_identical_outputs_reproduced",
        result.get("verdict") == "REPRODUCED" and result["max_deviation"] == 0.0,
        "stated arithmetic: 8.0 - 8.0 = 0.0 and 96 - 96 = 0; max deviation "
        "0.0 under the exact class",
    )

    # (2) Exact class, ANY deviation fails: 8.0 vs 8.0000001 -> deviation
    #     1e-07 > 0 — NOT_REPRODUCED (exact means zero, not "small").
    rerun = _exact_record()
    rerun["outputs_comparable"] = {"objective": 8.0000001, "feasible_interval_count": 96.0}
    result = compare_outputs(
        primary["outputs_comparable"], rerun["outputs_comparable"], exact_tolerance
    )
    check(
        "exact_any_deviation_fails",
        result.get("verdict") == "NOT_REPRODUCED",
        "stated arithmetic: |8.0000001 - 8.0| = 1e-07 > 0 — the exact class "
        "requires zero deviation; 'almost the same' is not exact",
    )

    # (3) Within tolerance, boundary-inclusive: 8.0 vs 8.25 with
    #     tolerance 0.25 -> deviation 0.25 (8.25 - 8.0 = 0.25);
    #     0.25 <= 0.25 -> REPRODUCED (the boundary is inclusive).
    within_tolerance = {
        "class": "within_tolerance",
        "value": 0.25,
        "pre_registration_ref": "P8.38/baseline-highs-inference",
    }
    result = compare_outputs(
        {"objective": 8.0}, {"objective": 8.25}, within_tolerance
    )
    check(
        "within_tolerance_boundary_inclusive",
        result.get("verdict") == "REPRODUCED" and result["max_deviation"] == 0.25,
        "stated arithmetic: 8.25 - 8.0 = 0.25 <= 0.25 — inclusive at the "
        "pre-registered boundary",
    )

    # (4) Within tolerance, beyond the boundary: 8.0 vs 8.3 with
    #     tolerance 0.25 -> deviation = |8.3 - 8.0| (approximately 0.3;
    #     8.3 is not exactly representable in binary floating point, so
    #     the assertion is against the DECISION, not against a decimal
    #     literal): deviation > 0.25 -> NOT_REPRODUCED.
    result = compare_outputs(
        {"objective": 8.0}, {"objective": 8.3}, within_tolerance
    )
    check(
        "within_tolerance_beyond_boundary_fails",
        result.get("verdict") == "NOT_REPRODUCED" and result["max_deviation"] > 0.25,
        "stated arithmetic: |8.3 - 8.0| is approximately 0.3 in binary "
        "floating point and strictly greater than 0.25",
    )
    primary_pair = _exact_record()
    rerun_pair = _exact_record()
    rerun_pair["outputs_comparable"] = {
        "objective": 8.3,
        "feasible_interval_count": 96.0,
    }
    verdict = rerun_verdict(primary_pair, rerun_pair, within_tolerance)
    check(
        "non_reproducing_marked_for_exclusion",
        verdict["outcome"] == "NOT_REPRODUCED_MARKED_FOR_EXCLUSION"
        and verdict["exclusion_statement"] is not None
        and "EXCLUDED" in verdict["exclusion_statement"],
        "ledger step 4 in code: a non-reproducing result carries the "
        "exclusion marking P8.26 must honour",
    )

    # (5) Missing tolerance -> refused, never defaulted (X-8).
    result = compare_outputs({"objective": 8.0}, {"objective": 8.0}, None)
    check(
        "tolerance_not_pre_registered_refused",
        result.get("refusal") == "TOLERANCE_NOT_PRE_REGISTERED",
        "a comparison without an injected pre-registered tolerance is the "
        "X-8 failure — refused, never fitted after the fact",
    )

    # (6) Different output key sets -> refused (comparing different
    #     quantities is not a comparison).
    result = compare_outputs(
        {"objective": 8.0}, {"objective": 8.0, "runtime": 1.0}, exact_tolerance
    )
    check(
        "output_keys_differ_refused",
        result.get("refusal") == "OUTPUT_KEYS_DIFFER",
        "primary has {objective}, rerun has {objective, runtime} — the "
        "extra key is a refusal, never ignored",
    )

    # (7) Empty outputs -> refused (an empty denominator cannot become a
    #     verdict of zero deviation).
    result = compare_outputs({}, {}, exact_tolerance)
    check(
        "empty_outputs_refused",
        result.get("refusal") == "OUTPUTS_EMPTY",
        "zero outputs cannot produce 'zero deviation' — the same "
        "empty-denominator discipline as the P8.15 harness rates",
    )

    # (8) A distribution-class comparison is refused here with the P8.38
    #     pointer (the ledger's class-confusion risk).
    result = compare_outputs(
        {"objective": 8.0},
        {"objective": 8.0},
        {"class": "distribution", "pre_registration_ref": "P8.38/training"},
    )
    check(
        "distribution_blocked_to_p838",
        result.get("refusal") == "DISTRIBUTION_RERUN_PROCEDURE_OWNED_BY_P8_38",
        "single numbers vs a distribution is P8.38's procedure — comparing "
        "them locally would be the named class confusion",
    )

    # (9) Identity: a rerun record citing benchmark-set v2 against a v1
    #     primary is RERUN_IDENTITY_MISMATCH:benchmark_set_version — the
    #     exact 'cited version differs from the artifact' finding the
    #     frozen artifact names.
    mismatched = _exact_record()
    mismatched["benchmark_set_version"] = "flexcore-research-benchmark-set-v2"
    findings = identity_findings(primary_pair, mismatched)
    check(
        "identity_benchmark_version_mismatch_named",
        findings == ["RERUN_IDENTITY_MISMATCH:benchmark_set_version"],
        f"hand-checked: only the benchmark-set version differs; got {findings}",
    )

    # (10) Identity: a differing dependency version is a named finding,
    #      never absorbed ("same seed, different data" generalizes to
    #      "same record, different environment").
    differing_env = _exact_record()
    differing_env["component_versions"] = {"highs": "1.16.0"}
    findings = identity_findings(primary_pair, differing_env)
    check(
        "environment_version_differs_named",
        findings
        == ["ENVIRONMENT_VERSION_DIFFERS:highs:'1.15.1' vs '1.16.0'"],
        f"hand-checked: the version difference is the finding; got {findings}",
    )

    # (11) Identity: a rerun missing a field the primary carries is a
    #      named RERUN_IDENTITY_FIELD_MISSING, never silently equal.
    truncated = _exact_record()
    del truncated["code_version"]
    truncated_flat = identity_findings(primary_pair, truncated)
    check(
        "identity_field_missing_named",
        "RERUN_IDENTITY_FIELD_MISSING:code_version" in truncated_flat,
        f"hand-checked; got {truncated_flat}",
    )

    # (12) Verdict assembly on identity failure -> INVALID_RERUN_IDENTITY
    #      (no reproduction verdict is possible; fix the procedure).
    verdict = rerun_verdict(primary_pair, mismatched, exact_tolerance)
    check(
        "identity_failure_blocks_verdict",
        verdict["outcome"] == "INVALID_RERUN_IDENTITY"
        and verdict["findings"]
        == ["RERUN_IDENTITY_MISMATCH:benchmark_set_version"],
        "fail-closed order: identity before comparison",
    )

    # (13) A distribution-class primary routes to BLOCKED_DISTRIBUTION
    #      even when identity holds.
    distribution_primary = _exact_record()
    distribution_primary["tolerance_class"] = "distribution"
    distribution_primary["module_id"] = "marl-policy"
    distribution_rerun = _exact_record()
    distribution_rerun["tolerance_class"] = "distribution"
    distribution_rerun["module_id"] = "marl-policy"
    verdict = rerun_verdict(distribution_primary, distribution_rerun, None)
    check(
        "distribution_primary_blocked",
        verdict["outcome"] == "BLOCKED_DISTRIBUTION",
        "the primary record's declared class routes the rerun to P8.38's "
        "procedure, regardless of the tolerance argument",
    )

    # (14) String outputs compare for exact equality in every class (a
    #      status label is not a quantity): equal strings reproduce,
    #      differing strings do not.
    result = compare_outputs(
        {"status": "COMPLETED"}, {"status": "COMPLETED"}, within_tolerance
    )
    result_bad = compare_outputs(
        {"status": "COMPLETED"}, {"status": "FAILED"}, within_tolerance
    )
    check(
        "string_outputs_exact_compared",
        result.get("verdict") == "REPRODUCED"
        and result_bad.get("verdict") == "NOT_REPRODUCED",
        "stated: 'COMPLETED' == 'COMPLETED' -> equal; 'COMPLETED' != "
        "'FAILED' -> string mismatch, never compared as a number",
    )

    # (15) A string-vs-number output pair is a refusal, never a
    #      comparison.
    result = compare_outputs(
        {"objective": 8.0}, {"objective": "eight"}, exact_tolerance
    )
    check(
        "string_number_mismatch_refused",
        result.get("refusal") == "OUTPUT_VALUE_TYPE_MISMATCH",
        "8.0 vs 'eight' is a type mismatch — refused, never coerced",
    )

    # (16) Namespace pin: no dispatch-shaped callable (reporting-only,
    #      ADR-010).
    forbidden = ("dispatch", "issue", "authorize", "approve", "command")
    namespace = [name for name in globals() if not name.startswith("_")]
    check(
        "no_dispatch_shaped_callable",
        not any(word in name.lower() for name in namespace for word in forbidden),
        f"module namespace {sorted(namespace)} contains no dispatch/issue/"
        "authorize/approve/command-shaped name",
    )

    return results


if __name__ == "__main__":  # pragma: no cover — human-gate entrypoint
    _records = run_self_tests()
    _failed = [r for r in _records if not r["passed"]]
    for _r in _records:
        print(f"{'PASS' if _r['passed'] else 'FAIL'} — {_r['test']}: {_r['detail']}")
    print(f"{len(_records) - len(_failed)}/{len(_records)} self-tests passed")
    raise SystemExit(1 if _failed else 0)
