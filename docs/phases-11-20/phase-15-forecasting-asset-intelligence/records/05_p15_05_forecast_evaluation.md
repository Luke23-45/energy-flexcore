# P15.05 — Forecast evaluation framework (the X-8 extension): metrics, backtesting protocol, frozen eval set, structural emptiness of every result cell

| Field | Value |
|---|---|
| Subtask | P15.05 (X-8 pre-registration extended to forecasting: metrics MAE / RMSE / skill score vs persistence, backtesting protocol, frozen eval sets mirroring `benchmarks/research-set-v1.json`; a change is a NEW version; every result cell EMPTY; no skill claimed anywhere) |
| Wave / agent | Phase 15 wave 1, Agent A |
| Status | WRITTEN-UNRUN: the framework and the frozen eval set are written, never run; NO evaluation run exists, no metric has been computed over real data, every result cell is EMPTY, and no skill is claimed anywhere (conventions §1, §5; the P8.19 run-status discipline) |
| Sources | [S-41] (forecasting table stakes — the evaluation framework is what makes any future skill claim honest); seams: `benchmarks/research-set-v1.json` (P8.19 shape, mirrored), `crates/flexcore-research/src/baselines.rs` (P8.06 freeze discipline), `docs/phases-11-20/01_PROGRAM_CONVENTIONS.md` §5 (X-8) |
| Files touched | `crates/flexcore-application/src/forecast_evaluation.rs` (NEW); `benchmarks/forecast-eval-set-v1.json` (NEW); `crates/flexcore-application/src/lib.rs` (additive) |

## 1. What landed

| Item | Content |
|---|---|
| Metric vocabulary | `ForecastMetric` (closed): `mae_mw`, `rmse_mw` (MW), `skill_score_vs_persistence` (unitless, `1 − MAE_model / MAE_persistence`, defined only for a strictly positive reference — a non-positive reference is a named refusal, never a division by zero); the metric functions (`pointwise_errors`, `mean_absolute_error`, `root_mean_squared_error`, `skill_score_vs_persistence`) are pure, fixed-accumulation-order arithmetic, tested against HAND-COMPUTED values over constructed inputs — computing a metric over supplied data is mathematics, not a claim |
| Backtesting protocol | `BacktestWindow` with the frozen split rule: a test window that overlaps or precedes its train window is refused (`TestOverlapsTrain`) — a forecaster is scored only on data it did not see; windows are DECLARED in the frozen artifact, never chosen after seeing outputs |
| Pre-registration (X-8) | `RegisteredThreshold`: metric, bound, direction (`LowerIsBetter` / `HigherIsBetter`), the eval-set VERSION it is registered under (thresholds do not silently transfer between set versions), and the registered date. Thresholds are registered BEFORE any run that could fill them; the v1 artifact registers the PLACEHOLDER-FREEZE DEFAULT 0.0 for every metric — 0.0 means "no threshold registered yet" (unachievable by construction for MAE/RMSE, a perfect-tie for skill) so nothing can be silently met; a real bound enters as a NEW set version, never an in-place edit |
| Structural emptiness | `EvaluationRunGate` has NO public constructor (a zero-sized private field; nothing in the program can mint one) — so `EvaluationRunRecord` (eval-set version + model ref + window id + metric + observed value + REQUIRED gate authorization ref) is UNCONSTRUCTABLE in-program. The refusal is a type, not prose. The constructor is minted by the human/lab run gate in the composition/lab runtime (VERIFY-BUILD item recorded here) |
| Frozen eval set | `benchmarks/forecast-eval-set-v1.json` mirroring the P8.19 shape: normative field order, anti-silent-growth rule (a change is `forecast-eval-set-v2` frozen as a NEW file), run_status NOT_RUN everywhere, `observed_value: null` in every cell, one backtest window over the P3.31 SYNTHETIC fixture (day 2 of the fixture horizon as the test window), the closed metric vocabulary with registered bounds, the designed-UNTRAINED models listed as not-evaluable, and the permission/authority note (no dispatch reach; never-granted set byte-identical) |

## 2. The honesty mechanism (why the cells cannot silently fill)

1. The frozen artifact's cells are `null` and its run statuses are `NOT_RUN`; a filled run never edits the artifact — it lands in an evaluation RECORD citing this version id and a gate authorization (the P8.19 citation rule, mirrored: a row without the version id or the gate authorization is invalid and must not be entered).
2. In-program code cannot record a result at all: the record type requires an `EvaluationRunGate` and the gate has no constructor. Any future code that would fabricate a filled record cannot compile.
3. Thresholds and observed values live STRUCTURALLY SEPARATE: the threshold is a frozen artifact field; the observed value exists only in the gated record type. There is no field where both appear.

## 3. What is NOT claimed

- No skill claim exists for any model — statistical or designed-ML; the standing default is "no skill until proven otherwise against the persistence baseline" (P15.02's `NO_SKILL_STATEMENT`).
- No evaluation result, score, or skill claim may construct or influence a dispatch command (ADR-010 extension — recorded in the artifact's `adr_010_note`).
- No real-data window exists: the platform has never run; the only window is over the labeled SYNTHETIC fixture, and the artifact says so explicitly.

## 4. Honest boundaries

1. The hand-computed test values (MAE 2.0, RMSE √(14/3), skill ±0.5) are arithmetic over constructed inputs — they demonstrate the metric definitions, not model quality.
2. The placeholder-freeze threshold convention (0.0 = not registered) is itself a pre-registered convention, recorded in the artifact's `registered_bound_note` fields; replacing it is a NEW set version.
3. `EvaluationRunGate`'s constructor at the human gate is a VERIFY-BUILD item: the first lab run must add it in the lab runtime with the gate-authorization record, never in this program's code.
4. Every test is WRITTEN-UNRUN.
