# Ledger — F-11 Measured BESS degradation and SoH

**Owning code:** `crates/flexcore-domain`
**Phase record:** P15.07-P15.11
**Source:** docs/releases/final-implementation-before-publication.md §5 (feature F-11); acceptance criteria cited per subtask.

| Subtask | Description | Status |
|---|---|---|
| FIM-F11.01 | Rainflow counting (ASTM E1049 three-point method) over SOC series into DOD x mean-SOC histograms, replacing the RECORDED SEAM fixed (written, awaiting human build/test) — wave 5 |
| FIM-F11.02 | Chemistry-parameterized cost curves (LFP/NMC data tables from recorded sources only; P15-E-3 stays OPEN) feeding C_degradation fixed (written, awaiting human build/test) — wave 5 |
| FIM-F11.03 | SoH estimator with provenance + staleness bound; weakest-provenance merge preserved; never overrides BMS SoH without recording disagreement fixed (written, awaiting human build/test) — wave 5 |
| FIM-F11.04 | Warranty headroom computed from measured counters fixed (written, awaiting human build/test) — wave 5 |
| FIM-F11.05 | Pinned tests: pinned rainflow histogram, cost-curve vs zero placeholder divergence, headroom monotonicity, merge policy fixed (written, awaiting human build/test) — wave 5 |
| FIM-F11.06 | Honesty header: cost-curve parameters are secondary-source values pending P15-E-3; no measured fleet exists fixed (written, awaiting human build/test) — wave 5 |

---

**Status:** IMPLEMENTED (written) — 6/6 subtasks fixed (written, awaiting human build/test) — wave 5. Ticks are applied only as
**fixed (written, awaiting human build/test)** with the wave number.