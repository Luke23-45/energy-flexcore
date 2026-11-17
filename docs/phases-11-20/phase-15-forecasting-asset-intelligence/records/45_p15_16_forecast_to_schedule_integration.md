# P15.16 — Forecast-to-schedule integration: real forecasts with extended staleness rules, the persistence-baseline fallback, the fixture as the labeled synthetic fallback

| Field | Value |
|---|---|
| Subtask | P15.16 (envelopes and rolling-horizon scheduling consume real forecasts with staleness rules EXTENDED from `envelope_staleness.rs` — `ForecastPublished` trigger, horizon rollover, max-age bound; fallback to the persistence baseline on staleness; the fixture stays the labeled synthetic fallback source) |
| Wave / agent | Phase 15 wave 2, Agent A |
| Status | WRITTEN-UNRUN: the module and its tests are written, never compiled, never executed. No forecast was produced, no fallback decision was taken in operation, no scheduling pass consumed anything. Every test value is hand-computed over CONSTRUCTED inputs. |
| Sources | [S-41] (forecasting as table stakes — the consumption side); seams: `envelope_staleness.rs` (P3.13, composed — never restated), `forecast_baselines.rs` (P15.02 persistence rule set — the fallback source), `forecast_fixture.rs` (P3.31 — the labeled synthetic fallback), `rolling_horizon.rs` (P3.24 trigger vocabulary — reused) |
| Files touched | `crates/flexcore-application/src/forecast_integration.rs` (NEW); `crates/flexcore-application/src/lib.rs` (additive module line + pub use block). `envelope_staleness.rs` itself NOT edited (the extension COMPOSES its pure functions — the single staleness home is unchanged). |

## 1. What landed

| Item | Content |
|---|---|
| `select_forecast_source` | The pure source-order decision over caller-declared inputs: (1) `PublishedForecast` when a forecast is published, the clock is consistent, the P3.13 supersede trigger did not fire, and the forecast's OWN max-age bound holds; (2) `PersistenceBaselineFallback` (the P15.02 `persistence_forecast` rule set over the declared history) with the fallback reason CARRIED on the decision — a fallback is a recorded degradation, never a silent substitution; (3) `SyntheticFixtureFallback` when even the persistence baseline refuses (empty/non-finite history) — the decision carries the fixture id and the `synthetic` provenance label; this module never invents fixture values (the labeled accessor, P3.31, is the only value path) |
| Staleness composition | The P3.13 discipline is CONSUMED, not copied: `evaluate_envelope_trigger` evaluates the `ForecastPublished` supersede trigger over the envelope metadata; the forecast's own age is checked against `ForecastSourceConfig` (default = the P3.13 one-hour recorded default) with the P2.08 equality convention (age == bound is NOT stale); a clock read before the forecast's computation time is its own `ClockBeforeComputation` reason — never a silent fresh verdict |
| Roll-trigger mapping | `roll_trigger_for_fallback`: a forecast-side degradation maps to the P3.24 `forecast_update` re-solve trigger; a horizon rollover maps to `horizon_advance` (the stored strings are the P3.24 vocabulary, reused — never restated) |
| Honesty constants | `FORECAST_TO_SCHEDULE_STATEMENT` (stale-never-silently-consumed; the fixture stays the labeled synthetic fallback source), `STALENESS_REUSE_STATEMENT` (composition-not-restatement), `FALLBACK_IS_NOT_A_COMMAND_STATEMENT` (ADR-010) |

## 2. Recorded decisions

1. **The envelope's telemetry/availability/capability triggers do NOT invalidate the forecast source**: they invalidate the ENVELOPE (P3.13); the forecast-source decision maps only the supersede trigger, the forecast's own max-age, the horizon rollover, the no-forecast and clock-inconsistent cases. An exhaustive match guards future P3.13 reasons against silent collapse.
2. **Two max-age bounds, two roles**: the envelope's `StalenessConfig` governs the envelope evaluation passed through; `ForecastSourceConfig.max_age_millis` (same recorded one-hour default) governs the published forecast's own age — a published forecast has its own computation timestamp, which is not the envelope's.
3. **The debounce semantics are the P3.13 ones verbatim**: debouncing delays only the recomputation advice; it never re-validates a stale source.
4. **`envelope_staleness.rs` was not edited**: the extension is a new module composing the existing pure functions; the P3.13 single-home discipline is preserved.

## 3. Honest boundaries

1. No skill claim anywhere: a persistence-baseline fallback carries the P15.02 `NO_SKILL_STATEMENT` posture (mirrored by citation in the tests); the synthetic fixture fallback is labeled synthetic (the P3.31 four-place labeling rule unchanged).
2. No dispatch reach (ADR-010 extension, conventions §6.3): a source decision is an input to scheduling analytics.
3. Every test is WRITTEN-UNRUN; the permitted-tool outputs are quoted in the close-out record (48).
