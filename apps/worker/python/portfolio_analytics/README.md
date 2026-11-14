# P8.22 — Portfolio analytics research (`apps/worker/python/portfolio_analytics/`)

Status: **WRITTEN, NEVER EXECUTED — pure stdlib, DERIVED REPORTING ONLY,
ALL MEASURED CELLS EMPTY.** The Phase 8 loop bans install/build/run:
this module was never executed, no pytest ran, and no figure in any
record here was computed by running it. The hand-computed self-test
expectations pin the analytic DEFINITIONS (stated arithmetic, e.g.
2.0 MW × 0.5 h = 1.0 MWh) and are NOT measurements
(`ledger/phase-08-advanced-coordination/22_portfolio_analytics.md` holds
the EMPTY measured cells and the filling procedure).

## What this is — and what it is not

Pure-stdlib analytics over INJECTED raw scenario data (ledger P8.22
step 1): available vs delivered flexibility (MW → MWh via the declared
interval hours), shortfall (MWh) with over-delivery carried SEPARABLY
(never netted into the shortfall), the delivery ratio, uncertainty-band
calibration coverage, and the comparison against the deterministic
baseline's accounting (ledger step 3, tolerance pre-registered, every
beyond-tolerance difference REPORTED as a discrepancy).

**The boundary (the subtask's stated risk, enforced in code):** these
outputs are DERIVED REPORTING, never a dispatch input. There is no
scoring, ranking, or scheduling function in the module — the "flexibility
score used to rank dispatch candidates" failure mode is structurally
absent — and `assert_reporting_only` refuses any document carrying a
dispatch-shaped key (schedule, setpoint, dispatch_rank,
flexibility_score, authorization, command, …) at ANY depth. Every
derived record carries the fixed reporting-only statement (ADR-010).

## Units, signs, and quality states (§10.2 — canonical)

- Power MW, energy MWh; SOC a fraction in [0, 1] wherever present
  (outside is a refusal, never a clamp).
- `P > 0` = export at the point of connection: the sign context is a
  DECLARED `direction` field (`export` | `import`) on every event; a
  negative available/delivered magnitude is refused so a sign is never
  silently encoded in a magnitude.
- Telemetry quality states (GOOD/SUSPECT/BAD/MISSING/ESTIMATED/STALE)
  are CARRIED on event records and summarized in the aggregate's
  `quality_state_mix` — a ratio built over STALE/ESTIMATED values is
  visible as such.

## Files

| File | Content |
|---|---|
| `analytics.py` | The pure analytics core: `event_energy` / `aggregate_delivery` (flexibility MW/MWh, shortfall, ratio), `uncertainty_calibration` (declared-band coverage, scope statement carried), `compare_to_baseline_accounting` (pre-registered tolerance; discrepancies reported, never absorbed), `assert_reporting_only` (the dispatch-boundary scan); named refusals (`EVENT_INVALID`, `CALIBRATION_INVALID`, `BASELINE_COMPARE_INVALID`, `TOLERANCE_NOT_PRE_REGISTERED`, `REPORTING_ONLY_VIOLATION`, `DIVISOR_ZERO`); hand-computed in-module self-tests (`run_self_tests()`) |
| `tests/test_portfolio_analytics.py` | Pytest mirror of the same arithmetic and refusals (WRITTEN, NEVER RUN) |

## Honest limitations

- No run has ever happened: the calibration coverage, delivery ratios,
  and discrepancy figures are DEFINITIONS pinned by constructed-input
  tests, never results.
- Calibration is assessed on the TESTED REGIMES ONLY — the scope
  statement travels on every calibration record and no generalization
  beyond the tested regimes is claimed anywhere.
- The baseline-accounting comparison needs the frozen baseline results
  artifact, which does not exist until the first human/lab run (P8.06 §3
  recorded this); the comparison cells are EMPTY until then.
- The ledger's "reporting plugins behind the SDK (Phase 7)" is the
  recorded activation seam: this module is the pure analytics core a
  Phase 7 SDK reporting plugin would wrap; no plugin contract is
  invented here (the gap is recorded in
  `22_portfolio_analytics.md` §8, never fixed by a parallel contract).

## How the human build/test pass runs it

```sh
python apps/worker/python/portfolio_analytics/analytics.py   # in-module self-tests
PYTHONPATH=apps/worker/python/portfolio_analytics python -m pytest \
  apps/worker/python/portfolio_analytics/tests -v           # pytest mirror
```
