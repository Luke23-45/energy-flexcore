# P16.04 — Baseline engine: CBL 10-in-10 / 5-in-10 / day-matching with weather/day-type filters and capped symmetric SDA (CAISO ≤20% [S-30]); program-configurable (LR-073); determinism + replay tests written-unrun

| Field | Value |
|---|---|
| Subtask | P16.04 (Baseline engine — CBL 10-in-10 / 5-in-10 / day-matching with weather/day-type filters, capped symmetric SDA; program-configurable — FlexCore never chooses the method (LR-073); determinism + replay tests written-unrun) |
| Wave / agent | Phase 16 wave 1, Agent A |
| Status | WRITTEN-UNRUN: the engine and its test file are written, never compiled, never executed, never run. **No baseline has been computed for any real asset; every input the tests use is SYNTHETIC fixture data.** |
| Sources | [S-30] (CAISO BPM for Demand Response v10 — the 10-in-10 / 5-in-10 construction and the ≤20% symmetric SDA bound), [S-96] (CAISO demand response — DLA named, NOT implemented: an unsupported method is a gap row, per LR-073); `crates/flexcore-domain/src/flexible_load_model.rs` (`baseline_method` field — unchanged, see decision 3) |
| Files touched | `crates/flexcore-application/src/baseline_engine.rs` (NEW); `crates/flexcore-application/tests/baseline_engine.rs` (NEW, written-unrun); `crates/flexcore-application/src/lib.rs` (additive module line) |

## 1. What landed

| Item | Content |
|---|---|
| `ProgramBaselineConfig` | The program's declared configuration: method NAME (as the program wrote it), optional day-type filter (exact equality), optional weather-index filter (symmetric tolerance; days WITHOUT an index never match — a missing index never silently passes), optional `SdaConfig` (cap in (0, 100]), optional selection rule |
| Closed method set | `CBL_10_IN_10`, `CBL_5_IN_10`, `DAY_MATCHING` (`SUPPORTED_METHOD_NAMES`); the CBL methods apply the recorded ten-day recency window (`CBL_RECENCY_WINDOW_DAYS = 10` [S-30]) then the highest-N selection (N=10 / N=5) with EXPLICIT tie-breaking (total descending, then day-key ascending) |
| Gap rows | `BaselineGapRecord` with named reasons: `UnsupportedMethod` (LR-073 — recorded, never approximated), `InsufficientEligibleDays`, `IntervalCountMismatch`, `MissingSdaWindow` (an SDA is never silently skipped), `SdaCapExceedsJurisdictionBound` (above `CAISO_SDA_MAX_PERCENT = 20.0` [S-30]), `InvalidSdaWindow`, `ClockOutOfRange`. Persisting the gap row with its P11 audit-chain linkage is the caller's obligation |
| SDA | Capped SYMMETRIC same-day adjustment over the program-declared window: the raw ratio is RECORDED alongside the applied one (capped, never clipped into silence); the CAISO ≤20% bound [S-30] is enforced as a recorded sourced bound, re-verified at use |
| Determinism | Pure functions; time enters ONLY through the injected `flexcore_ports::clock::Clock`; day selection is BTreeMap-ordered by day key with explicit tie-breaking; no wall clock, no RNG, no hash-order dependence. Same inputs + same clock ⇒ byte-identical outcome (the replay property) |
| Tests | `tests/baseline_engine.rs`: 13 tests — unsupported method ⇒ gap; 10-in-10 with an event day excluded and an out-of-window day excluded ⇒ exact-gap assertion; 10-in-10 positive path; 5-in-10 highest-five with deterministic tie-break; day-matching day-type + weather filters; SDA capped symmetrically in BOTH directions with both ratios recorded; declared-SDA-without-window ⇒ gap; window outside the grid ⇒ gap; cap above the jurisdiction bound ⇒ gap; interval-count mismatch ⇒ gap; replay byte-identity; the closed vocabulary + LR-073 statement pins; the recency-window constant pin |

## 2. Recorded decisions

1. **LR-073 is the module's first invariant** (`FLEXCORE_NEVER_CHOOSES_STATEMENT`): the method is the program's declared configuration; the engine computes only the closed supported set; everything else is a RECORDED GAP ROW — never a silent approximation, never a FlexCore-chosen substitute.
2. **The CAISO SDA bound is SOURCED DATA** (`CAISO_SDA_MAX_PERCENT = 20.0`, cited [S-30], re-verify-at-use). The check applies to every program whose declared cap exceeds the bound — the bound is declared BY the jurisdiction pack, and the CAISO constant is the recorded sourced instance.
3. **The flexible-load model is NOT touched.** The Phase 3 restriction (`flexible_load_model::BaselineMethod` accepts only the scenario-declared synthetic method) is unchanged; the market baseline engine is a separate record-level engine for program-declared DR baselines and is never wired into the flexible-load model.
4. **DLA [S-96] is named and NOT implemented**: it falls outside the closed supported set and therefore produces a recorded gap row — the honest LR-073 disposition, not a missing feature to be silently approximated.
5. **ADR-010**: the engine produces inputs and records — never commands; no dispatch authority exists here.

## 3. Honest boundaries

1. No baseline has been computed for any real asset or program; the engine has never executed; the tests have never run (their first run is the human verification obligation).
2. The ten-day recency window and the highest-N construction are the [S-30] construction as cited; the exact fallback rules a real program may apply (day substitution, holiday handling beyond the declared day-type filter) are program configuration — a program needing them declares them through its record pack; this engine invents nothing.
3. The permitted-tool outputs are quoted in the close-out record (07).
