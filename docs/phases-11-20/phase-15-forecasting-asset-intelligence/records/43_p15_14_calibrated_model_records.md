# P15.14 — Per-asset calibrated model records: declared thermal/efficiency model RECORDS with provenance, never fitted claims

| Field | Value |
|---|---|
| Subtask | P15.14 (thermal/efficiency model RECORDS per asset — declared parameters with provenance, never fitted claims — plus regression test fixtures written complete and UNRUN; consumes the simulator profiles' recorded absence, superseded as recorded parameters) |
| Wave / agent | Phase 15 wave 2, Agent A |
| Status | WRITTEN-UNRUN: the module and its tests are written, never compiled, never executed. No parameter was fitted (no fitting procedure exists anywhere in the module); no model record was populated for any real asset; every test value is hand-computed over CONSTRUCTED inputs. |
| Sources | [S-168] (the digital-twin spectrum — the "calibrated per-asset model" end this record type prepares for, explicitly at the lite end: records, not a live twin); [S-53] (asset-performance-management context) |
| Files touched | `crates/flexcore-application/src/calibrated_model_records.rs` (NEW); `crates/flexcore-application/src/lib.rs` (additive module line + pub use block). `crates/flexcore-simulator/src/profiles.rs` NOT touched (read-only citation — see §2). |

## 1. What landed (deliverable class: declared-parameter records + pure checkability functions)

| Item | Content |
|---|---|
| `DeclaredParameter` | A named value in a unit-carrying field name (`tau_thermal_hours`, `temp_rise_per_mw_c`, …), with provenance (the P3.02 Measured/Configured/Inferred/Unknown vocabulary — unknown narrows, never widens) and a RECORDED source; no bare float travels unnamed |
| `DeclaredThermalModel` | Per-asset first-order lumped RC equivalent: thermal time constant (hours, strictly positive), steady-state temperature rise per MW (°C/MW), ambient reference (°C); the pure first-order step evaluation (`temperature_after`) is textbook arithmetic over the declared parameters — checkable by hand, exactly as valid as the declaration behind it |
| `DeclaredEfficiencyCurve` / `DeclaredConstantEfficiency` | Piecewise-linear efficiency over STRICTLY INCREASING power-fraction breakpoints (each efficiency in (0, 1]); linear interpolation INSIDE the declared curve, a NAMED REFUSAL outside it (never extrapolation — a claim the declaration never made); a constant efficiency (the Phase 2 simulator's `sqrt(round_trip_efficiency)` split) is declared as data, not as a one-point curve |
| Honesty constants | `DECLARED_NOT_FITTED_STATEMENT`, `SIMULATOR_PROFILE_SUPERSESSION_STATEMENT`, `MODEL_RECORD_IS_NOT_A_COMMAND_STATEMENT` (ADR-010) |

## 2. The supersession, recorded (not an edit)

The Phase 2 simulator's behaviour profiles record their own absence — `crates/flexcore-simulator/src/profiles.rs`: "no electrochemical, temperature, degradation, or state-of-health model; constant efficiency". That recorded absence is SUPERSEDED AS RECORDED PARAMETERS by this module: the platform now has a home where a per-asset thermal/efficiency model is DECLARED as data with provenance. The simulator profile file is NOT edited and stays synthetic; the supersession is a record-level fact (`SIMULATOR_PROFILE_SUPERSESSION_STATEMENT`), pinned by a test that asserts the statement's text.

## 3. The regression fixtures (written complete and UNRUN)

The `#[cfg(test)]` module is the fixture set: the textbook first-order response (30 − 10/e ≈ 26.3212 °C after one tau at 1 MW over a 20 °C ambient with a 10 °C/MW rise — hand-computed), the linear-interpolation known answers (0.92 at the 0.25 fraction of the declared (0, 0.90)/(0.5, 0.94)/(1.0, 0.92) curve), the below-curve/above-curve refusals, and the full validation-refusal matrix. Every expectation is hand-computed from the rule sets over constructed inputs; execution is the human gate.

## 4. Honest boundaries

1. "Calibrated" is a record TYPE, not a claim: no calibration run exists, no runtime data exists, and a future calibration lands as a NEW record version naming its evidence (X-8) — never an in-place relabel.
2. No dispatch reach (ADR-010 extension, conventions §6.3); a model record is analytics/operator-view data.
3. Units travel in field names (hours, °C, °C/MW, fractions); canonical units and sign conventions are never normalized here.
4. Every test is WRITTEN-UNRUN; the permitted-tool outputs are quoted in the close-out record (48).
