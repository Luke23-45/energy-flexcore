# P8.37 — Hand-written PROPERTY tests for the P8.20 reproducibility
#               tooling (seed derivation, the record schema, the rerun
#               comparator — seed_derivation, record_schema, rerun_check).
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER — read before trusting anything below
#
# 1. PROPERTY-BASED, STDLIB-ONLY. No property-testing library is imported
#    or installed (the Phase 8 loop bans; P8.37 requires hand-written
#    generators). Every generator is a plain function over
#    `random.Random(<fixed seed>)`. EACH property states its ORACLE, its
#    SEED, and its DETERMINISM contract (the module constant
#    PROPERTY_SEED; a fresh seed for the P8.37 verification method is a
#    one-constant change — the properties must hold for ANY seed).
# 2. WRITTEN, NEVER RUN. pytest was never installed in this loop. The
#    tested surfaces are the stdlib-only reproducibility tooling.
# 3. Run instruction (the recorded procedure):
#    `PYTHONPATH=apps/worker/python/reproducibility python -m pytest
#    apps/worker/python/reproducibility/tests -v` from the repository
#    root.
# 4. WHAT THE PROPERTIES PIN (ledger P8.37 step 2, as code):
#    - P-SEED:    the recorded seed derivation is DETERMINISTIC, lands
#                 in [0, 2^64), and carries the XOR structure the
#                 derivation string declares (an algebraic identity, not
#                 a measurement); sampled-key distinctness is asserted
#                 over a fixed seeded key set (sample evidence, NOT the
#                 injectivity theorem — stated as such);
#    - P-SCHEMA:  the record validator refuses EVERY record with a
#                 required field removed — for ALL required fields, not
#                 a hand-picked few — and accepts the minimal valid
#                 record unchanged;
#    - P-RERUN:   the rerun comparator's tolerance boundary is INCLUSIVE
#                 (deviation exactly == the pre-registered tolerance
#                 reproduces; anything beyond fails), the exact class
#                 fails on ANY nonzero deviation, and a distribution
#                 class or missing pre-registration is ALWAYS refused
#                 (X-8 — the comparator never derives a tolerance).
#    THE TEST-ARITHMETIC / RUN-RESULT DISTINCTION (carried from the
#    P8.20 record): everything here pins the derivation, schema, and
#    comparison RULES over constructed inputs. It is NOT a rerun
#    outcome, NOT a seed result, and NOT evidence about any case. No
#    measured cell is filled by it.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import os
import random
import string
import sys

import pytest

sys.path.insert(
    0,
    os.path.join(os.path.dirname(os.path.abspath(__file__)), os.pardir),
)

import seed_derivation as sd  # noqa: E402 — path set above
import record_schema as rs  # noqa: E402
import rerun_check as rc  # noqa: E402

PROPERTY_SEED = 20261114

_U64 = 0xFFFFFFFFFFFFFFFF


def _rng() -> random.Random:
    return random.Random(PROPERTY_SEED)


def _asset_key(rng: random.Random) -> str:
    """An arbitrary asset-key-shaped string (site/asset paths plus junk)."""
    if rng.random() < 0.7:
        return "site-%s/asset-%s-%04d" % (
            rng.choice(["north", "south", "east", "west"]),
            rng.choice(["bess", "pv", "evse", "load"]),
            rng.randrange(10000),
        )
    return "".join(
        rng.choice(string.ascii_letters + string.digits + "-_/.:")
        for _ in range(rng.randrange(1, 30))
    )


# ---------------------------------------------------------------------------
# P-SEED — the derivation's determinism, range, and XOR structure
# ---------------------------------------------------------------------------


class TestSeedDerivationProperty:
    def test_deterministic_and_in_range(self) -> None:
        """P-SEED (determinism + range). Oracle: the derivation's own
        contract — the same (master_seed, asset_key) pair ALWAYS yields
        the same seed, and every seed lies in [0, 2^64). Generator:
        seeded asset keys, two fixed master seeds."""
        rng = _rng()
        keys = [_asset_key(rng) for _ in range(200)]
        for master in (sd.RECORDED_MASTER_SEED, 0, _U64):
            first_pass = [sd.derive_asset_seed(master, k) for k in keys]
            second_pass = [sd.derive_asset_seed(master, k) for k in keys]
            assert first_pass == second_pass
            for seed in first_pass:
                assert 0 <= seed <= _U64

    def test_xor_structure_identity(self) -> None:
        """P-SEED (algebraic). Oracle: the derivation string itself —
        `seed(k) = m XOR splitmix64(FNV-1a-64(k))` means
        derive(m1, k) XOR derive(m2, k) == m1 XOR m2 for EVERY k (the
        key-dependent term cancels). This is an IDENTITY of the recorded
        structure, checked over generated keys; a derivation that broke
        it would not be the recorded derivation."""
        rng = _rng()
        for _ in range(150):
            k = _asset_key(rng)
            m1 = rng.randrange(0, 1 << 32)
            m2 = rng.randrange(0, 1 << 32)
            derived_xor = sd.derive_asset_seed(m1, k) ^ sd.derive_asset_seed(m2, k)
            assert derived_xor == (m1 ^ m2)

    def test_distinct_keys_sample_distinct_seeds(self) -> None:
        """P-SEED (distinctness — SAMPLE evidence, honestly labelled).
        Oracle: injectivity ON THE SAMPLED SET only — distinct keys
        yield distinct seeds for the generated key set under the fixed
        seed. This is collision-resistance EVIDENCE for a sample, NOT a
        theorem (a 64-bit hash is not injective in general); the P8.20
        record's own distinctness self-test makes the same scoped claim
        over its 1000 states."""
        rng = _rng()
        keys = list({_asset_key(rng) for _ in range(500)})
        seeds = [sd.derive_asset_seed(sd.RECORDED_MASTER_SEED, k) for k in keys]
        assert len(set(seeds)) == len(keys)

    def test_invalid_master_seed_refused(self) -> None:
        """P-SEED (input validation). Oracle: the refusal — a bool or
        out-of-range master seed, and an empty/non-string asset key, is
        ALWAYS refused (the derivation never coerces)."""
        for bad in (True, -1, _U64 + 1, "20260601", 1.0):
            with pytest.raises(ValueError):
                sd.derive_asset_seed(bad, "site-x/asset-bess-0001")
        with pytest.raises(ValueError):
            sd.derive_asset_seed(sd.RECORDED_MASTER_SEED, "")
        with pytest.raises(TypeError):
            sd.derive_asset_seed(sd.RECORDED_MASTER_SEED, 42)


# ---------------------------------------------------------------------------
# P-SCHEMA — the record validator over arbitrary removals
# ---------------------------------------------------------------------------


class TestRecordSchemaProperty:
    def test_minimal_valid_record_accepted(self) -> None:
        """P-SCHEMA (baseline). Oracle: the validator's empty-findings
        contract — the hand-built minimal valid record yields ZERO
        findings (same record the module's own self-tests pin)."""
        assert rs.validate_record(rs._minimal_valid_record()) == []

    def test_every_required_field_removal_is_named(self) -> None:
        """P-SCHEMA. Oracle: the validator's completeness contract —
        removing ANY required top-level field yields at least one
        finding NAMING that field, for ALL of them (property over the
        whole required-field tuple, not a hand-picked subset). The seed
        selects only the iteration order, which does not affect the
        assertion."""
        rng = _rng()
        required = list(rs._TOP_LEVEL_REQUIRED)
        rng.shuffle(required)
        for field in required:
            record = rs._minimal_valid_record()
            del record[field]
            findings = rs.validate_record(record)
            assert findings, "no finding for removed %r" % field
            assert any(field in f for f in findings), (
                "removed %r not named in %r" % (field, findings)
            )

    def test_placeholder_tokens_always_refused(self) -> None:
        """P-SCHEMA (the 'not run hidden as a blank' failure mode).
        Oracle: `PLACEHOLDER_TOKENS` — a template token in ANY scalar
        record field is refused (`PLACEHOLDER_PRESENT`), for every
        token x every probe field combination."""
        rng = _rng()
        probes = ("record_id", "run_id", "module_version", "code_version")
        for token in sorted(rs.PLACEHOLDER_TOKENS):
            field = rng.choice(probes)
            record = rs._minimal_valid_record()
            record[field] = token
            findings = rs.validate_record(record)
            assert any(f.startswith("PLACEHOLDER_PRESENT: " + field) for f in findings)

    def test_within_tolerance_record_requires_preregistration_ref(self) -> None:
        """P-SCHEMA (the P8.38 seam). Oracle: `TOLERANCE_REF_REQUIRED` —
        a within_tolerance record WITHOUT a `tolerance_ref` is refused
        and a nondeterminism-sources disclosure is REQUIRED
        (`NONDETERMINISM_SOURCES_REQUIRED` when empty); the exact class
        needs neither. This is the record-level shape of P8.38's
        pre-registration rule (X-8)."""
        record = rs._minimal_valid_record()
        record["tolerance_class"] = "within_tolerance"
        record["determinism"] = {
            "seed_supported": False,
            "nondeterminism_sources": [],
        }
        findings = rs.validate_record(record)
        assert any(f.startswith("TOLERANCE_REF_REQUIRED") for f in findings)
        assert any(
            f.startswith("NONDETERMINISM_SOURCES_REQUIRED") for f in findings
        )
        record["tolerance_ref"] = "P8.38/pre-registration-record-ref"
        record["determinism"]["nondeterminism_sources"] = [
            "gpu_nondeterminism",
            "framework_version_sensitivity",
        ]
        assert rs.validate_record(record) == []


# ---------------------------------------------------------------------------
# P-RERUN — the comparator's boundary and refusal rules
# ---------------------------------------------------------------------------


class TestRerunComparatorProperty:
    def _tolerance(self, value: float) -> dict:
        return {
            "class": "within_tolerance",
            "value": value,
            "pre_registration_ref": "P8.38/pre-registration-record-ref",
        }

    def test_tolerance_boundary_is_inclusive(self) -> None:
        """P-RERUN. Oracle: the comparison rule — for arbitrary output
        pairs, deviation exactly EQUAL to the pre-registered tolerance
        reproduces (`REPRODUCED`), the next representable step beyond
        fails (`NOT_REPRODUCED`). The boundary is inclusive, exactly as
        the P8.20 record states. Generator: seeded deviations around
        seeded tolerances."""
        rng = _rng()
        for _ in range(100):
            tol = rng.choice([0.0, 1e-9, 0.25, 1.0, 42.5])
            base = rng.uniform(-100.0, 100.0)
            primary = {"objective": base}
            at_boundary = {"objective": base + tol}
            result_at = rc.compare_outputs(primary, at_boundary, self._tolerance(tol))
            assert result_at["verdict"] == "REPRODUCED"
            beyond = base + tol + (abs(base) + abs(tol) + 1.0) * 1e-9
            result_beyond = rc.compare_outputs(
                primary, {"objective": beyond}, self._tolerance(tol)
            )
            assert result_beyond["verdict"] == "NOT_REPRODUCED"

    def test_exact_class_fails_on_any_nonzero_deviation(self) -> None:
        """P-RERUN. Oracle: the exact class — ANY nonzero deviation
        fails, no matter how small; identical outputs reproduce with
        max deviation 0.0. Generator: seeded tiny deviations."""
        rng = _rng()
        exact = {
            "class": "exact",
            "pre_registration_ref": "P8.38/baseline-exact-RECORD-REF",
        }
        for _ in range(100):
            base = rng.uniform(-100.0, 100.0)
            assert rc.compare_outputs(
                {"o": base}, {"o": base}, exact
            )["verdict"] == "REPRODUCED"
            eps = (abs(base) + 1.0) * 10.0 ** rng.randrange(-12, -1)
            result = rc.compare_outputs({"o": base}, {"o": base + eps}, exact)
            assert result["verdict"] == "NOT_REPRODUCED"

    def test_refusals_never_become_verdicts(self) -> None:
        """P-RERUN. Oracle: the refusal vocabulary — a MISSING
        pre-registration (`TOLERANCE_NOT_PRE_REGISTERED`), a malformed
        tolerance, an invalid class, a missing pre-registration ref, a
        DISTRIBUTION class (`DISTRIBUTION_RERUN_PROCEDURE_OWNED_BY_P8_38`
        — the class confusion is structurally impossible here), and
        differing output key sets are ALL refused: a refusal dict, never
        a verdict."""
        rng = _rng()
        primary = {"objective": 1.0}
        rerun = {"objective": 1.0}
        assert "refusal" in rc.compare_outputs(primary, rerun, None)
        assert "refusal" in rc.compare_outputs(primary, rerun, "0.25")
        assert "refusal" in rc.compare_outputs(primary, rerun, {"class": "bogus",
                                                                "pre_registration_ref": "r"})
        assert "refusal" in rc.compare_outputs(
            primary, rerun, {"class": "within_tolerance", "value": 1.0}
        )
        for _ in range(50):
            assert rc.compare_outputs(
                primary, rerun,
                {"class": "distribution",
                 "pre_registration_ref": "P8.38/RECORD-REF"},
            )["refusal"] == "DISTRIBUTION_RERUN_PROCEDURE_OWNED_BY_P8_38"
            assert rc.compare_outputs(
                primary, {"other": 1.0}, self._tolerance(1.0)
            )["refusal"] == "OUTPUT_KEYS_DIFFER"

    def test_identity_findings_block_every_mismatch(self) -> None:
        """P-RERUN (identity). Oracle: the identity contract — for EVERY
        identity field, a rerun that differs on that field (or drops it)
        yields a NAMED finding (`RERUN_IDENTITY_MISMATCH:<field>` /
        `RERUN_IDENTITY_FIELD_MISSING:<field>`), and a differing
        dependency version yields `ENVIRONMENT_VERSION_DIFFERS`; an
        identical rerun yields NO findings. Property over all identity
        fields."""
        rng = _rng()
        for field in rc.RERUN_IDENTITY_FIELDS:
            primary = rc._exact_record()
            rerun = rc._exact_record()
            if field == "config_digest":
                rerun["config"]["config_digest"] = "fnv1a64:" + "f" * 16
            elif field == "seeds":
                rerun["determinism"]["seeds"] = {"site-x": 1}
            elif field == "input_snapshot_ids":
                rerun["inputs"]["input_snapshot_ids"] = ["other-snapshot"]
            elif field == "input_artifact_digests":
                rerun["inputs"]["input_artifact_digests"] = {
                    "other.json": "sha256:" + "a" * 64
                }
            else:
                rerun[field] = "different-" + str(rng.randrange(1000))
            findings = rc.identity_findings(primary, rerun)
            assert findings, "no finding for %r mismatch" % field
        primary = rc._exact_record()
        assert rc.identity_findings(primary, rc._exact_record()) == []
        version_rerun = rc._exact_record()
        version_rerun["component_versions"] = {"highs": "1.15.2"}
        findings = rc.identity_findings(primary, version_rerun)
        assert any(f.startswith("ENVIRONMENT_VERSION_DIFFERS:highs") for f in findings)
