# P15.02 — Load forecasting service design: statistical baselines first (persistence, seasonal-naive, degree-day regression), frozen registry, explicit no-skill claims, guarded-deps Python worker skeleton

| Field | Value |
|---|---|
| Subtask | P15.02 (statistical baselines first: persistence, seasonal-naive, degree-day regression as deterministic registry-frozen references; EXPLICIT no-skill claims until evaluation runs (P15.05); Python worker skeleton per the guarded-deps pattern; the P3.10 `SYNTHETIC_BASELINE_DISCLAIMER` superseded only by evaluated baselines) |
| Wave / agent | Phase 15 wave 1, Agent A |
| Status | WRITTEN-UNRUN: rule sets, frozen registry, and the worker skeleton are written, never compiled or executed; no baseline output exists; no skill is claimed anywhere (conventions §1) |
| Sources | [S-41][S-50][S-51] (forecasting as commercial table stakes, gap analysis §3 row 3); seams: `crates/flexcore-research/src/baselines.rs` (P8.06 freeze discipline), `crates/flexcore-domain/src/flexible_load_model.rs` (P3.10 disclaimer), `apps/worker/python/gnn_worker/` (guarded-deps pattern), `benchmarks/forecast-eval-set-v1.json` (P15.05) |
| Files touched | `crates/flexcore-application/src/forecast_baselines.rs` (NEW); `apps/worker/python/forecast_worker/` (NEW: `README.md`, `forecast_baseline_worker.py`, `tests/test_forecast_baseline_worker.py`); `crates/flexcore-application/src/lib.rs` (additive) |

## 1. What landed (deliverable class: design + deterministic rule sets)

| Item | Content |
|---|---|
| Persistence (`persistence.v1`) | `forecast(t) = last observed value`, carried forward unchanged for every horizon step; sign- and unit-preserving; the REFERENCE baseline (skill = beating this on a frozen eval set); empty history and non-finite values are named refusals, never zero-fills |
| Seasonal-naive (`seasonal-naive.v1`) | `forecast(t) = value exactly one declared season earlier`; longer horizons wrap within the LAST season in the history (declared wrap rule); the season length is a DECLARED input, never inferred; short histories are named refusals |
| Degree-day regression (`degree-day.v1`) | OLS of load on (intercept, HDD, CDD) via the normal equations, solved by deterministic Gaussian elimination with first-max pivoting and a frozen singularity epsilon (refuses, never pseudo-inverts); degree days are DECLARED inputs (never an inferred base temperature); load-only; ≥ 3 samples required; prediction linear in the declared forecast degree days |
| Frozen registry | `forecast_baseline_registry()` — three records in fixed order, each with version, closed artifact-kind and horizon declarations, and written assumptions/limits (P8.06 step 4); `FrozenForecastBaseline::freeze` computes the FNV-1a 64 digest over the canonical description (test-grade, NOT a security primitive); a change is `bump_version()` (v1→v2, unparseable versions get a suffix), never a silent edit; `confirm_reproduction` names a digest deviation instead of absorbing it |
| Honesty constants | `NO_SKILL_STATEMENT` (no baseline claims skill/accuracy/fitness for any market or settlement purpose), `BASELINE_SUPERSESSION_RULE` (the P3.10 disclaimer is superseded only by EVALUATED baselines via a NEW registry version that names the eval-set version and the justifying evaluation record — never a silent relabel), `P3_10_SYNTHETIC_BASELINE_DISCLAIMER_MIRROR` (pinned VERBATIM-equal to the domain constant by a test — the domain is the single home) |

## 2. The two-language mirror (recorded)

The Python worker (`apps/worker/python/forecast_worker/`) mirrors the three rule sets for the out-of-process execution seam; the Rust module is the SINGLE HOME. The mirror is pinned by identical hand-computed test vectors: persistence `[-0.4,-0.5,-0.6]→[-0.6,-0.6,-0.6]×3`; seasonal-naive wrap `[10,20,30,40] s2 h4 → [30,40,30,40]`; the orthogonal degree-day construction resolving exactly to intercept 1.0, HDD coef 2.0, CDD coef 3.0 (hand-verified normal equations: AᵀA = [[5,1,1],[1,1,0],[1,0,1]], Aᵀy = [10,3,4]). A mirror divergence is a test failure, never a silent drift.

## 3. The worker skeleton (guarded-deps pattern)

`forecast_baseline_worker.py` follows the gnn_worker pattern verbatim: honesty header; guarded `import numpy` INSIDE a function raising the NAMED `NUMPY_DEPLOYMENT_REQUIREMENT` (the frozen rule sets are pure stdlib and never call it); pure request validation BEFORE any computation; `permissions_declared` validated against the producer vocabulary (`telemetry.read` + `forecast.publish`) with the never-granted set checked FIRST; ONE JSON request on stdin (`flexcore-forecast-worker/1`) → ONE result envelope on stdout; malformed input → NOTHING on stdout, reason on stderr, exit 2; in-band rule refusals are `termination_status: "failed"` envelopes (a refusal is a result, never a crash); `assert_proposal_only` scans the whole result tree for authorization/command-shaped keys before emission; mode-locked to simulation/shadow; every envelope carries the baseline version, the UNTRAINED registry state, the frozen eval-set reference (`forecast-eval-set-v1`), the no-skill note, and the P3.10 supersession statement. `tests/test_forecast_baseline_worker.py` mirrors the in-module self-tests including the subprocess transport contract.

## 4. Honest boundaries

1. No baseline output exists anywhere: the rule sets are arithmetic definitions, not produced forecasts.
2. No skill, accuracy, or fitness claim exists in any file touched by this subtask; the standing claim is no-skill until an X-8-gated run fills a cell (P15.05).
3. The P3.31 synthetic fixture remains the labeled synthetic fallback source (P15.16 consumes it as such); the P3.10 disclaimer is NOT edited by this subtask.
4. Every test (Rust and Python) is WRITTEN-UNRUN; execution is the human gate.
5. The degree-day baseline needs weather inputs that do not exist yet (P15.03, P15-E-1 OPEN) — its forecast-degree-day arguments are caller-declared, never invented here.
