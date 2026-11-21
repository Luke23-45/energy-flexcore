# Ledger — F-10 Live forecasting ingestion + evaluated baselines

**Owning code:** `apps/worker (rust ingestion + python evaluation)`
**Phase record:** P15.01-P15.06/P15.16
**Source:** docs/releases/final-implementation-before-publication.md §5 (feature F-10); acceptance criteria cited per subtask.

| Subtask | Description | Status |
|---|---|---|
| FIM-F10.01 | Config-driven vendor weather/price HTTP ingestion adapter (no vendor named; P15-E-1/E-2 stay OPEN): provenance-carrying records, staleness window, fail-closed without a token fixed (written, awaiting human build/test) — wave 7 |
| FIM-F10.02 | Persistence-baseline fallback wiring: envelopes and rolling horizon consume a real forecast and fall back past max-age (ADR-010: forecasts are INPUTS, never commands) fixed (written, awaiting human build/test) — wave 7 |
| FIM-F10.03 | Backtesting harness (pure Python): MAE / RMSE / skill vs the persistence baseline; no skill claimed without a run fixed (written, awaiting human build/test) — wave 7 |
| FIM-F10.04 | Forecast artifact versioning + digest chain + replayability (P15.06 shape) fixed (written, awaiting human build/test) — wave 7 |
| FIM-F10.05 | Pinned tests: provenance, staleness fallback, metric computation on fixtures, artifact digest chain fixed (written, awaiting human build/test) — wave 7 |
| FIM-F10.06 | Honesty header: vendor feeds and evaluation runs are HUMAN GATES (token registration; P15.05 results stay EMPTY until a run) fixed (written, awaiting human build/test) — wave 7 |

---

**Status:** IMPLEMENTED (written) — 6/6 subtasks fixed (written, awaiting human build/test) — wave 7. Ticks are applied only as
**fixed (written, awaiting human build/test)** with the wave number.