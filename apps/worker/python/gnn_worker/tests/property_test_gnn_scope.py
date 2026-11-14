# P8.37 — Hand-written PROPERTY tests for the GNN research worker family
#               (scope enforcement, graph/feature validation, the
#               held-out overlap check — gnn_inference_worker,
#               gnn_evaluation).
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER — read before trusting anything below
#
# 1. PROPERTY-BASED, STDLIB-ONLY, NO FRAMEWORK. No property-testing
#    library is imported or installed (the Phase 8 loop bans; P8.37
#    requires hand-written generators). Every generator is a plain
#    function over `random.Random(<fixed seed>)` — stdlib, deterministic
#    for a fixed seed. EACH property states its ORACLE, its SEED, and
#    its DETERMINISM contract (the module constant PROPERTY_SEED; a
#    fresh seed for the P8.37 verification method is a one-constant
#    change — the properties must hold for ANY seed).
# 2. WRITTEN, NEVER RUN. pytest was never installed in this loop; no
#    test below has ever executed. The tested surfaces are the
#    stdlib-only pure-logic core of the GNN worker (the framework path
#    is UNREACHABLE: torch/PyG DECISION-POINT OPEN, P8.10 §6 — no
#    inference output exists, and none is produced here).
# 3. Run instruction (the recorded procedure):
#    `PYTHONPATH=apps/worker/python/gnn_worker python -m pytest
#    apps/worker/python/gnn_worker/tests -v` from the repository root.
# 4. WHAT THE PROPERTIES PIN (ledger P8.37 step 2, as code):
#    - P-SCOPE:   the topology-scope check flags EVERY out-of-scope
#                 topology for arbitrary scopes and arbitrary
#                 trained lists — exact string equality only; NO
#                 near-miss (prefix, suffix, case, whitespace variant)
#                 ever passes (this is the research-worker scope-
#                 enforcement surface in this directory; the platform's
#                 TENANT isolation is P8.31's crates-side surface, owned
#                 elsewhere — recorded in 37_research_tests.md §6, not
#                 tested here);
#    - P-GRAPH:   the graph validator refuses EVERY malformed graph
#                 (self-loops, out-of-range endpoints, wrong arity,
#                 bad node counts) and preserves every VALID graph's
#                 edge multiset (parallel edges allowed — real
#                 feeders);
#    - P-FEATURE: the feature validator refuses ANY shape or range
#                 violation (row width, non-finite, SOC outside [0,1],
#                 status outside {0,1}, voltage <= 0) and accepts every
#                 conforming row;
#    - P-OVERLAP: the held-out overlap check reports leakage IF AND
#                 ONLY IF the trained and held-out sets intersect
#                 (exact string equality).
#    These pin the pure validation logic over simulation inputs. The
#    warm-start honesty statement carries: a warm start affects the
#    SPEED of an exact solve, never its optimality — nothing here
#    proves any generalization property of any model (X-7); no model
#    exists.
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

import gnn_inference_worker as gw  # noqa: E402 — path set above
import gnn_evaluation as ge  # noqa: E402

# The ONE seed every generator derives from (determinism contract, as in
# the MARL property suite).
PROPERTY_SEED = 20261114


def _rng() -> random.Random:
    return random.Random(PROPERTY_SEED)


def _topology_id(rng: random.Random) -> str:
    """An arbitrary topology-ID-shaped string (the manifest instance-ID
    scheme: family id + 4-digit sequence, plus arbitrary junk ids)."""
    if rng.random() < 0.7:
        return "fam-%s-%04d" % (
            rng.choice(["train-radial-small", "holdout-radial-large", "other"]),
            rng.randrange(10000),
        )
    return "".join(
        rng.choice(string.ascii_letters + string.digits + "-_./ ")
        for _ in range(rng.randrange(1, 20))
    )


# ---------------------------------------------------------------------------
# P-SCOPE — scope enforcement for arbitrary scopes and trained lists
# ---------------------------------------------------------------------------


class TestTopologyScopeProperty:
    def test_membership_is_exact_equality_only(self) -> None:
        """P-SCOPE. Oracle: exact string membership — for arbitrary
        (topology_id, trained list, declared scope) triples, in_scope is
        True IF AND ONLY IF topology_id is an element of the trained
        list. Generator: seeded random ids and random trained lists
        (subsets of a generated id pool)."""
        rng = _rng()
        for _ in range(300):
            pool = [_topology_id(rng) for _ in range(rng.randrange(1, 8))]
            trained = [t for t in pool if rng.random() < 0.5]
            topology_id = rng.choice(pool + [_topology_id(rng)])
            verdict = gw.evaluate_topology_scope(
                topology_id, trained, "balanced-radial-feeder/RECORD-CLASS"
            )
            assert verdict["in_scope"] == (topology_id in trained)

    def test_near_miss_never_passes(self) -> None:
        """P-SCOPE (the near-miss rule — 'a near-miss must flag, not
        pass'). Oracle: exact equality — for a trained list containing
        id X, a mutated variant (prefix, suffix, case flip, whitespace,
        unicode look-alike substitution) is NEVER in scope unless it
        equals X exactly. Generator: seeded mutations of seeded ids."""
        rng = _rng()
        mutations = (
            lambda s: s + "-1",
            lambda s: "x" + s,
            lambda s: s.upper(),
            lambda s: s.lower(),
            lambda s: s + " ",
            lambda s: " " + s,
            lambda s: s.replace("a", "ɑ"),  # unicode look-alike
            lambda s: s[: max(1, len(s) - 1)],
        )
        for _ in range(200):
            base = _topology_id(rng).strip() or "fam-x-0000"
            trained = [base]
            variant = rng.choice(mutations)(base)
            if variant == base:
                continue
            verdict = gw.evaluate_topology_scope(
                variant, trained, "balanced-radial-feeder/RECORD-CLASS"
            )
            assert verdict["in_scope"] is False

    def test_empty_trained_list_flags_everything(self) -> None:
        """P-SCOPE (the P8.35 manifest's current state, as a property):
        with an EMPTY trained list, EVERY topology is out-of-scope —
        the fail-closed consequence recorded in the training-data
        manifest. Oracle: in_scope is False for arbitrary ids."""
        rng = _rng()
        for _ in range(50):
            verdict = gw.evaluate_topology_scope(
                _topology_id(rng), [], "balanced-radial-feeder/RECORD-CLASS"
            )
            assert verdict["in_scope"] is False


# ---------------------------------------------------------------------------
# P-GRAPH — the graph validator over arbitrary graph shapes
# ---------------------------------------------------------------------------


def _valid_graph(rng: random.Random) -> tuple:
    """An arbitrary VALID graph: node_count >= 1, edges with endpoints
    in [0, node_count), no self-loops, parallel edges allowed."""
    node_count = rng.randrange(1, 30)
    edges = []
    for _ in range(rng.randrange(0, 40)):
        src = rng.randrange(node_count)
        dst = rng.randrange(node_count)
        if src == dst:
            dst = (dst + 1) % node_count
        edges.append([src, dst])
    return node_count, edges


class TestGraphValidationProperty:
    def test_valid_graphs_accepted_and_preserved(self) -> None:
        """P-GRAPH (accept side). Oracle: the normalized output equals
        the input edge multiset (order-preserving, parallel edges kept
        verbatim — real feeders have parallel lines). Generator: seeded
        valid graphs."""
        rng = _rng()
        for _ in range(200):
            node_count, edges = _valid_graph(rng)
            normalized = gw.validate_graph(node_count, edges)
            assert normalized == [list(e) for e in edges]

    def test_self_loop_always_refused(self) -> None:
        """P-GRAPH. Oracle: the refusal — ANY self-loop (src == dst, at
        any position, on any node) is refused, for arbitrary valid
        surrounding edge sets. Generator: seeded valid graph + one
        injected self-loop."""
        rng = _rng()
        for _ in range(100):
            node_count, edges = _valid_graph(rng)
            node = rng.randrange(node_count)
            position = rng.randrange(len(edges) + 1)
            edges.insert(position, [node, node])
            with pytest.raises(gw.WorkerRequestError):
                gw.validate_graph(node_count, edges)

    def test_out_of_range_endpoint_always_refused(self) -> None:
        """P-GRAPH. Oracle: the refusal — an endpoint outside
        [0, node_count) is refused for arbitrary placements (negative
        and >= node_count, src or dst side)."""
        rng = _rng()
        for _ in range(100):
            node_count, edges = _valid_graph(rng)
            edge = [
                rng.randrange(node_count),
                rng.randrange(node_count),
            ]
            side = rng.randrange(2)
            bad = rng.choice([-1, node_count, node_count + 7])
            edge[side] = bad
            position = rng.randrange(len(edges) + 1)
            edges.insert(position, edge)
            with pytest.raises(gw.WorkerRequestError):
                gw.validate_graph(node_count, edges)

    def test_malformed_structures_always_refused(self) -> None:
        """P-GRAPH (shape level). Oracle: the refusal — wrong arity, non-
        integer endpoints, bool endpoints (bool is an int subtype and
        must be refused explicitly), non-list edge sets, zero/negative
        and bool node counts: ALL refused for arbitrary combinations."""
        rng = _rng()
        with pytest.raises(gw.WorkerRequestError):
            gw.validate_graph(0, [])
        with pytest.raises(gw.WorkerRequestError):
            gw.validate_graph(True, [])
        with pytest.raises(gw.WorkerRequestError):
            gw.validate_graph(3, None)
        for _ in range(100):
            node_count = rng.randrange(1, 10)
            malformed = rng.choice(
                [
                    [[0]],                     # wrong arity (1)
                    [[0, 1, 2]],               # wrong arity (3)
                    [["0", 1]],                # string endpoint
                    [[0.5, 1]],                # float endpoint
                    [[True, 1]],               # bool endpoint (int subtype)
                    ["0-1"],                   # not a pair at all
                ]
            )
            with pytest.raises(gw.WorkerRequestError):
                gw.validate_graph(node_count, [malformed])


# ---------------------------------------------------------------------------
# P-FEATURE — the feature validator over arbitrary rows
# ---------------------------------------------------------------------------


class TestFeatureValidationProperty:
    def _declarations(self, rng: random.Random) -> list:
        kinds = [
            {"name": "p_export_mw", "kind": "power", "unit": "MW"},
            {"name": "q_export_mvar", "kind": "power", "unit": "MVAr"},
            {"name": "soc_fraction", "kind": "soc", "unit": None},
            {"name": "v_pu", "kind": "voltage", "unit": None},
            {"name": "status", "kind": "status", "unit": None},
        ]
        return rng.sample(kinds, rng.randrange(1, len(kinds) + 1))

    def _valid_value(self, rng: random.Random, kind: str) -> float:
        if kind == "soc":
            return rng.choice([0.0, 1.0, rng.uniform(0.0, 1.0)])
        if kind == "status":
            return rng.choice([0.0, 1.0])
        if kind == "voltage":
            return rng.uniform(0.01, 2.0)
        return rng.uniform(-10.0, 10.0)  # power: any sign (P > 0 = export,
        # a caller-side declaration carried by the NAME, not enforced here)

    def test_conforming_rows_always_accepted(self) -> None:
        """P-FEATURE (accept side). Oracle: the validator returns None
        (accepts) for arbitrary conforming feature matrices. Generator:
        seeded declarations + seeded conforming rows."""
        rng = _rng()
        for _ in range(200):
            declarations = self._declarations(rng)
            node_count = rng.randrange(1, 15)
            features = [
                [self._valid_value(rng, d["kind"]) for d in declarations]
                for _ in range(node_count)
            ]
            gw.validate_features(features, node_count, declarations)  # no raise

    def test_range_violations_always_refused(self) -> None:
        """P-FEATURE. Oracle: the refusal — SOC outside [0,1], status
        outside {0,1}, voltage <= 0, non-finite values, and non-numeric
        cells are ALWAYS refused, at arbitrary (row, column) positions.
        Generator: seeded conforming matrix + one injected violation."""
        rng = _rng()
        for _ in range(200):
            declarations = self._declarations(rng)
            node_count = rng.randrange(1, 10)
            features = [
                [self._valid_value(rng, d["kind"]) for d in declarations]
                for _ in range(node_count)
            ]
            row = rng.randrange(node_count)
            col = rng.randrange(len(declarations))
            kind = declarations[col]["kind"]
            violation = {
                "soc": rng.choice([-0.1, 1.0000001, 2.0, -1.0]),
                "status": rng.choice([-1.0, 0.5, 2.0, 1.5]),
                "voltage": rng.choice([0.0, -0.5, -1e-9]),
                "power": rng.choice([float("nan"), float("inf")]),
            }[kind]
            if rng.random() < 0.15:
                violation = rng.choice(
                    [float("nan"), float("inf"), "not-a-number", True]
                )
            features[row][col] = violation
            with pytest.raises(gw.WorkerRequestError):
                gw.validate_features(features, node_count, declarations)

    def test_shape_violations_always_refused(self) -> None:
        """P-FEATURE (shape level). Oracle: the refusal — a row count
        different from node_count, a row width different from the
        declaration count, and non-list rows are ALWAYS refused for
        arbitrary shapes."""
        rng = _rng()
        for _ in range(150):
            declarations = self._declarations(rng)
            node_count = rng.randrange(1, 10)
            conforming = [
                [self._valid_value(rng, d["kind"]) for d in declarations]
                for _ in range(node_count)
            ]
            mutation = rng.choice(["too_few_rows", "too_many_rows", "narrow_row",
                                   "wide_row", "non_list_row"])
            if mutation == "too_few_rows" and node_count == 1:
                # A single-row matrix cannot lose a row; fall back to a
                # violation that is well-defined at any node count.
                mutation = "non_list_row"
            features = [list(r) for r in conforming]
            if mutation == "too_few_rows":
                features = features[:-1]
            elif mutation == "too_many_rows":
                features.append([self._valid_value(rng, declarations[0]["kind"])])
            elif mutation == "narrow_row":
                features[rng.randrange(node_count)] = features[0][:-1]
            elif mutation == "wide_row":
                features[rng.randrange(node_count)] = features[0] + [0.0]
            else:
                features[rng.randrange(node_count)] = "not-a-row"
            with pytest.raises(gw.WorkerRequestError):
                gw.validate_features(features, node_count, declarations)


# ---------------------------------------------------------------------------
# P-OVERLAP — the held-out overlap check (the ledger's verification
# method, in code)
# ---------------------------------------------------------------------------


class TestHeldoutOverlapProperty:
    def test_leakage_iff_sets_intersect(self) -> None:
        """P-OVERLAP. Oracle: set intersection — `check_heldout_overlap`
        reports leakage IF AND ONLY IF the trained and held-out lists
        share at least one id (exact string equality), and the reported
        overlap equals the sorted intersection. Generator: seeded id
        pools and seeded subset choices."""
        rng = _rng()
        for _ in range(200):
            pool = [
                "fam-%s-%04d" % (rng.choice(["train", "holdout"]), rng.randrange(50))
                for _ in range(rng.randrange(1, 12))
            ]
            trained = [t for t in pool if rng.random() < 0.5]
            heldout = [t for t in pool if rng.random() < 0.5]
            result = ge.check_heldout_overlap(trained, heldout)
            expected = sorted(set(trained) & set(heldout))
            assert result["overlap"] == expected
            assert result["leakage"] is bool(expected)
            if expected:
                assert result["finding"].startswith("LEAKAGE")
            else:
                assert "no overlap" in result["finding"]

    def test_disjoint_families_of_the_p835_manifest_never_leak(self) -> None:
        """P-OVERLAP (manifest seam). Oracle: set intersection — the
        P8.35 manifest's training and held-out FAMILY ids are disjoint
        by construction, so the family-level check reports no leakage
        (the instance-level check is the reviewer's, run once instances
        exist). Deterministic: the family ids are the manifest's
        constants, not generated."""
        trained = ["fam-train-radial-small", "fam-train-radial-medium"]
        heldout = [
            "fam-holdout-radial-large",
            "fam-holdout-radial-deep-lateral",
            "fam-holdout-radial-open-ring",
        ]
        result = ge.check_heldout_overlap(trained, heldout)
        assert result["leakage"] is False
        assert result["overlap"] == []
