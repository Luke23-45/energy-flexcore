# P3.30 — Phase 3 handover to Phase 4 (§23 Stage 3)

Date: 2026-11-07. Status: **all 31 subtasks implemented (P3.01–P3.31);
nothing built, nothing run** — verification is the human's. Gate: Stage 3
NOT held (`30` record below); the demonstration + sign-off are gate inputs.

## 1. What Phase 4 consumes (exact inputs)

| Input | Contract + version home | Source |
|---|---|---|
| Schedule identity + revision chain | `crates/flexcore-storage/src/schedule_revision.rs`; `is_revision_superseded` surface; migrations 0013/0014 | P3.22 |
| Envelope version + staleness gates | `crates/flexcore-application/src/{flexibility_envelope,envelope_staleness}.rs`; envelope store + migration 0015 | P3.12/13 |
| §12.6 optimization result records | `crates/flexcore-optimization-api/src/result.rs` + `crates/flexcore-application/src/optimization_result_record.rs`; `InputSnapshotRecord`, `GridModelTraceability` | P3.21 |
| Comparison records (Tier 1 vs Tier 2) | `crates/flexcore-optimization-api/src/conic_fixture.rs` (`Tier1Tier2ComparisonRecord`); `crates/flexcore-application/src/schedule_comparison.rs` | P3.18/23 |
| Solver/provider pins | HiGHS 1.15.1 via highspy (D-P3.17-1, Python worker); Clarabel.rs =0.11.1 (native); thread_count 1 default | P3.17/19 |
| Grid-model provider boundary | `crates/flexcore-grid-api` (P4.01/02 already landed IN Phase 4's wave 1) | — |
| Synthetic forecast fixture | `scenarios/synthetic-portfolio/forecast-fixture-v1.json` (+ error variant); version `v1` / `v1-error-pv-ramp`, horizon 24 h, cadence 60 s, `provenance: synthetic` at fixture + value level; generator `crates/flexcore-application/src/forecast_fixture.rs` (P3.31 — listed here per its ledger criterion) | P3.31 |
| Rolling-horizon triggers + frozen-past semantics | `crates/flexcore-application/src/rolling_horizon.rs`; six pinned trigger strings | P3.24 |

## 2. Recorded limitations Phase 4 must NOT treat as guarantees

1. **Degradation is a configurable linear placeholder, default zero** — not a
   measured law (`DegradationMetadata.is_placeholder` on every output).
2. **The objective is SYNTHETIC** (`SYNTHETIC_ENERGY_COST_DEVIATION_V1`) and
   commitments are synthetic test commitments — D1/D2 OPEN; no market claim.
3. **SOCP exactness is NOT assumed** — Tier 2 results are candidates whose
   residuals are diagnostic only; Phase 4's independent AC power flow is the
   check (P3.18 caveat; P4.09 enforces the residual-record gate).
4. **Tier 3/4 do not exist** — `unsupported` with the decision reference
   (P3.20 guard); MISOCP/MINLP needs reformulate-or-Stage-8.
5. **P_aux unknown** is omitted-and-recorded, not zero (BESS energy drain is
   otherwise understated); registry gaps G1–G18 remain unknown-narrowed.
6. **Baseline method** for loads is `scenario-declared-baseline-profile` —
   synthetic, not a DR/market baseline.
7. **No measured performance figures exist** — the P3.27 template is
   pre-registered with EMPTY cells; the human's benchmark run creates the
   v0.2.0 baseline (CI records are triage context, not baselines).
8. **Everything is unbuilt/unrun** — first `cargo build`/`test` is the real
   proof; VERIFY-BUILD comments carry the flagged spots per crate.

## 3. Exit-gate evidence (§23 Stage 3)

- `25_26_tests_scenarios.md`: three pre-registered scenarios (a/b/c) with
  pinned versions/seeds/tolerances — expectations written BEFORE any run;
  UNRUN.
- `27_benchmark_report.md`: measurement template, measured cells EMPTY.
- `26_exit_gate_review.md` equivalent: **Stage 3 review NOT HELD** — human
  reviewers + the scenario runs + the benchmark record are the gate inputs
  (same pattern as Stages 0–2; recorded in CHECKLIST.md).
- Verification tools (P3.30 step 3): `python tools/verify_docs.py` → OK;
  `python tools/validate_agent.py` → OK (recorded at close).

## 4. Open items carried to Phase 4

- Human runs: `cargo test --workspace`, the P3.26 scenarios, the P3.27
  benchmark, coverage (`cargo llvm-cov`), the dependency/secret scans
  (P3.29 F3).
- Tier 1 worker adapter (Python/highspy side of `Tier1Backend`) — lands with
  Phase 4's dispatch/shadow work; G5 real-solve determinism re-verified then.
- Coverage gaps G1–G6 (static map in `25_26_tests_scenarios.md`).
- PLAN.md open questions (interval length confirmation, dominance threshold,
  deviation coefficient) remain configuration, not decisions.
