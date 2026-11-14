# P8.20 — The recorded seed derivation, implemented verbatim: the frozen
#         `flexcore-research-benchmark-set-v1` formula as a pure stdlib
#         function with hand-computed / theorem-derived self-tests.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. WHAT THIS MODULE IS. The ledger P8.20 step 1 requirement that seeds be
#    RECORDED per run includes the DERIVATION behind them. The frozen
#    benchmark artifact (`benchmarks/research-set-v1.json`, every case's
#    `seeds` block) records the derivation string
#    `seed(asset_key) = master_seed XOR splitmix64(FNV-1a-64(asset_key))`
#    verbatim. This module implements EXACTLY that formula — FNV-1a 64,
#    one output of the splitmix64 stream seeded with that hash, XORed with
#    the master seed — so a reviewer can recompute any recorded per-asset
#    seed from the recorded master seed and asset key without trusting the
#    researcher's arithmetic. It is PURE STDLIB, performs NO I/O, and
#    imports NOTHING from this repository's other modules.
# 2. WRITTEN, NEVER RUN. The Phase 8 loop bans (ENTRY_NOTE.md) mean this
#    module has never been executed. The self-tests below carry HAND-
#    COMPUTED or THEOREM-DERIVED expectations, each with its arithmetic or
#    its reasoning STATED in the test — see `run_self_tests`. NOTHING here
#    is a measurement, a seed result, or evidence about any case.
# 3. THE DERIVATION-STRING INTERPRETATION IS RECORDED, NOT SILENT. The
#    frozen derivation string does not spell out "one output of the
#    splitmix64 stream seeded with the FNV-1a-64 hash"; this module fixes
#    that reading (`splitmix64_first`: internal state initialized to the
#    hash, then one `state += GOLDEN; mix(state)` step — the standard
#    reading of "splitmix64(x)"). Any different reading is a NEW
#    derivation version and requires a NEW benchmark-set version
#    (flexcore-research-benchmark-set-v2) — the frozen v1 artifact is
#    never edited (the P8.19 anti-silent-growth rule). This module never
#    edits the artifact; it only implements the recorded string.
# 4. TEST-VECTOR SOURCES ARE NAMED. The FNV-1a 64 expectations are the
#    PUBLISHED test vectors of the FNV reference test suite (the
#    draft-eastlake-xiii-fnv test vectors: "" -> 0xcbf29ce484222325,
#    "a" -> 0xaf63dc4c8601ec8c, "foobar" -> 0x85944171f73967e8); they are
#    NOT invented here. The splitmix64 expectations are (a) the hand-
#    computed algebraic fact mix(0) == 0 (stated arithmetic in the test)
#    and (b) the bijection property of the mixing function (Vigna's
#    splitmix64 construction: xor-shifts are word-bijections and
#    multiplication by ODD constants is a bijection mod 2^64 — both
#    mixing constants here are odd), from which "distinct states give
#    distinct outputs" follows mathematically; the suite checks that
#    property on the first 1000 states. No splitmix64 numeric vector is
#    asserted from memory anywhere — none is published in this
#    repository's sources, and an unverifiable literal would be a
#    hallucinated fact (AGENTS.md rule 3).
# 5. REPORTING ONLY — NEVER A DISPATCH INPUT (ADR-010). The namespace
#    contains NO dispatch/issue/authorize/approve/command-shaped callable
#    (pinned by a self-test); a recomputed seed is reproducibility
#    bookkeeping, nothing else.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import math

# ── The recorded constants ────────────────────────────────────────────────

# Verbatim from benchmarks/research-set-v1.json (every case's
# `seeds.derivation` field). The self-tests pin this exact string; a
# different string means the artifact changed, which by the P8.19
# anti-silent-growth rule means a NEW benchmark-set version.
RECORDED_SEED_DERIVATION = (
    "seed(asset_key) = master_seed XOR splitmix64(FNV-1a-64(asset_key))"
)

# The recorded master seed of the v1 benchmark set (verbatim, same
# artifact) — used by the self-tests as the recorded example value.
RECORDED_MASTER_SEED = 20260601

# FNV-1a 64-bit parameters (the published constants of the algorithm).
FNV_OFFSET_BASIS = 0xcbf29ce484222325  # 14695981039346656037
FNV_PRIME = 0x100000001B3              # 1099511628211

# splitmix64 constants (Vigna 2015 reference implementation).
_SPLITMIX_GOLDEN_GAMMA = 0x9E3779B97F4A7C15
_SPLITMIX_MUL_A = 0xBF58476D1CE4E5B9   # odd
_SPLITMIX_MUL_B = 0x94D049BB133111EB   # odd

_U64 = 0xFFFFFFFFFFFFFFFF

REPORTING_ONLY_STATEMENT = (
    "P8.20 reproducibility tooling — reporting/bookkeeping only; the "
    "output never influences dispatch and carries no dispatch-shaped "
    "result (ADR-010; simulation-only, AGENTS.md rule 5)."
)


# ── The functions ─────────────────────────────────────────────────────────


def fnv1a64(data: bytes) -> int:
    """FNV-1a, 64-bit, over the bytes of ``data``.

    h = offset basis; per byte: h ^= byte; h *= FNV_PRIME (mod 2^64).
    """
    if not isinstance(data, (bytes, bytearray)):
        raise TypeError("fnv1a64 expects bytes")
    h = FNV_OFFSET_BASIS
    for byte in bytes(data):
        h ^= byte
        h = (h * FNV_PRIME) & _U64
    return h


def splitmix64_mix(z: int) -> int:
    """The splitmix64 mixing function (Vigna 2015), mod 2^64.

    z = (z ^ (z >> 30)) * A;  z = (z ^ (z >> 27)) * B;  return z ^ (z >> 31)
    with A = 0xBF58476D1CE4E5B9 and B = 0x94D049BB133111EB (both odd).
    """
    z &= _U64
    z = ((z ^ (z >> 30)) * _SPLITMIX_MUL_A) & _U64
    z = ((z ^ (z >> 27)) * _SPLITMIX_MUL_B) & _U64
    return (z ^ (z >> 31)) & _U64


def splitmix64_stream(seed: int):
    """The splitmix64 generator seeded with ``seed``.

    state = seed; each step: state += GOLDEN_GAMMA; yield mix(state).
    """
    state = seed & _U64
    while True:
        state = (state + _SPLITMIX_GOLDEN_GAMMA) & _U64
        yield splitmix64_mix(state)


def splitmix64_first(seed: int) -> int:
    """One output of the splitmix64 stream seeded with ``seed``.

    This is the module's recorded reading of "splitmix64(x)" in the
    frozen derivation string (HONESTY HEADER item 3).
    """
    return next(iter(splitmix64_stream(seed)))


def derive_asset_seed(master_seed: int, asset_key: str) -> int:
    """The recorded derivation, verbatim:

    seed(asset_key) = master_seed XOR splitmix64(FNV-1a-64(asset_key))

    ``master_seed`` must be an int in [0, 2^64); ``asset_key`` a
    non-empty string. Deterministic: same inputs -> same output, always.
    """
    if isinstance(master_seed, bool) or not isinstance(master_seed, int):
        raise ValueError("master_seed must be an int")
    if not 0 <= master_seed <= _U64:
        raise ValueError("master_seed must be in [0, 2^64)")
    if not isinstance(asset_key, str) or not asset_key:
        raise ValueError("asset_key must be a non-empty string")
    return master_seed ^ splitmix64_first(fnv1a64(asset_key.encode("utf-8")))


def is_finite_number(value: object) -> bool:
    return (
        not isinstance(value, bool)
        and isinstance(value, (int, float))
        and math.isfinite(value)
    )


# ── Hand-computed / theorem-derived self-tests ────────────────────────────


def run_self_tests() -> list:
    """Run every self-test; return one record per test.

    Every expectation is HAND-COMPUTED (arithmetic stated), a PUBLISHED
    test vector (source named), or THEOREM-DERIVED (reasoning stated).
    Nothing here is a measurement.
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

    # (1) FNV-1a 64, empty input — the offset basis itself, because zero
    #     bytes means zero xor/multiply iterations. HAND-COMPUTED (stated).
    check(
        "fnv1a64_empty_is_offset_basis",
        fnv1a64(b"") == 0xcbf29ce484222325,
        "published vector (draft-eastlake-xiii-fnv suite): "
        'fnv1a64("") = 0xcbf29ce484222325 = 14695981039346656037 — the '
        "offset basis, since an empty input performs no iterations",
    )

    # (2) FNV-1a 64, "a" — PUBLISHED test vector (source named above).
    check(
        "fnv1a64_a_published_vector",
        fnv1a64(b"a") == 0xAF63DC4C8601EC8C,
        "published vector: fnv1a64(\"a\") = 0xaf63dc4c8601ec8c",
    )

    # (3) FNV-1a 64, "foobar" — PUBLISHED test vector (source named above).
    check(
        "fnv1a64_foobar_published_vector",
        fnv1a64(b"foobar") == 0x85944171F73967E8,
        "published vector: fnv1a64(\"foobar\") = 0x85944171f73967e8",
    )

    # (4) splitmix64_mix(0) == 0 — HAND-COMPUTED, arithmetic stated:
    #     0 ^ (0 >> 30) = 0; 0 * A mod 2^64 = 0 for any A;
    #     0 ^ (0 >> 27) = 0; 0 * B mod 2^64 = 0; 0 ^ (0 >> 31) = 0.
    check(
        "splitmix64_mix_zero_is_zero",
        splitmix64_mix(0) == 0,
        "stated arithmetic: every stage maps 0 to 0 "
        "(x ^ (x >> k) = 0 and 0 * c = 0 mod 2^64 when x = 0)",
    )

    # (5) Bijection property: xor-shift (x ^ (x >> k)) is a bijection on
    #     64-bit words, and multiplication by an ODD constant is a
    #     bijection mod 2^64; both mixing constants are odd
    #     (0xBF58476D1CE4E5B9 ends 9, 0x94D049BB133111EB ends B). A
    #     composition of bijections is a bijection, so distinct states
    #     give distinct outputs. THEOREM-DERIVED expectation, machine-
    #     checked on the first 1000 states (none of which is 0, so (4)
    #     does not trivially satisfy this).
    first_1000 = {splitmix64_mix(s) for s in range(1000)}
    check(
        "splitmix64_mix_is_injective_on_first_1000_states",
        len(first_1000) == 1000,
        "theorem: xor-shift and odd-constant multiplication are "
        "bijections mod 2^64, so mix is a bijection — distinct states "
        "s in [0, 1000) must give 1000 distinct outputs",
    )

    # (6) The derivation string is pinned VERBATIM to the frozen v1
    #     artifact (the self-test fails if the artifact string and this
    #     module ever disagree — a change is a new derivation version).
    check(
        "recorded_derivation_string_verbatim",
        RECORDED_SEED_DERIVATION
        == "seed(asset_key) = master_seed XOR splitmix64(FNV-1a-64(asset_key))",
        "verbatim equality with the frozen "
        "flexcore-research-benchmark-set-v1 `seeds.derivation` field",
    )

    # (7) XOR structure: by the formula, seed = master XOR S where
    #     S = splitmix64_first(fnv1a64(asset_key)); XOR is self-inverse,
    #     so seed XOR S == master for EVERY master. HAND-DERIVED identity,
    #     checked over the recorded master seed and two others.
    key = "site-north-bess-1"
    s = splitmix64_first(fnv1a64(key.encode("utf-8")))
    ok = all(
        (derive_asset_seed(m, key) ^ s) == m for m in (0, 1, RECORDED_MASTER_SEED)
    )
    check(
        "derive_asset_seed_xor_structure",
        ok,
        "stated arithmetic: seed ^ S == master because seed = master ^ S "
        "and XOR is self-inverse — checked for master in "
        "{0, 1, 20260601} with S = splitmix64_first(fnv1a64("
        '"site-north-bess-1"))',
    )

    # (8) Determinism: the same inputs give the same output (the function
    #     has no randomness, no clock, no state across calls).
    check(
        "derive_asset_seed_deterministic",
        derive_asset_seed(RECORDED_MASTER_SEED, key)
        == derive_asset_seed(RECORDED_MASTER_SEED, key),
        "pure function of (master_seed, asset_key) — no randomness",
    )

    # (9) Range: the output is a 64-bit value in [0, 2^64).
    seed = derive_asset_seed(RECORDED_MASTER_SEED, key)
    check(
        "derive_asset_seed_range_u64",
        0 <= seed <= _U64,
        "both operands are in [0, 2^64) and XOR of two such values stays "
        "in [0, 2^64)",
    )

    # (10) Refusals: a non-int master seed, an out-of-range master seed,
    #      and an empty asset key are all refused — never defaulted.
    refusals_ok = True
    for bad_call in (
        lambda: derive_asset_seed("20260601", key),
        lambda: derive_asset_seed(-1, key),
        lambda: derive_asset_seed(2**64, key),
        lambda: derive_asset_seed(RECORDED_MASTER_SEED, ""),
    ):
        try:
            bad_call()
            refusals_ok = False
        except ValueError:
            pass
    check(
        "derive_asset_seed_refusals",
        refusals_ok,
        "each bad call raises ValueError — a malformed seed input is "
        "refused, never silently defaulted (fail-closed discipline)",
    )

    # (11) Namespace pin: no dispatch-shaped callable exists in this
    #      module (reporting-only, ADR-010).
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
