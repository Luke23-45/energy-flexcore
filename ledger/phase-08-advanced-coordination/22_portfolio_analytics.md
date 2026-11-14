# P8.22 — Portfolio analytics research (derived reporting, not dispatch) — code WRITTEN, measured cells EMPTY

Date: 2026-11-14. Subtask: P8.22 (`ledger_phase_8.md` §P8.22). Owner role:
research engineer. Report trace: §5.3 (analytics/report category); §19.4
(flexibility metrics); §10.2 (canonical units/signs). Inputs: P8.01
(`01_readiness_check.md`); P7.15 profiles (the analytics/report plugin
category, by citation); P8.06 (`05_06_harness_baselines.md` §3 — the
deterministic baseline whose accounting the analytics are validated
against); P8.02 (`02_research_scope_freeze.md` S-6 — reporting-only
scope, not a hypothesis-bearing module); ENTRY_NOTE.md boundary 4.

## 1. Status of this record — reporting-only code WRITTEN-NEVER-RUN; nothing measured, nothing fabricated

The analytics core
(`apps/worker/python/portfolio_analytics/analytics.py`, schema
`flexcore-portfolio-analytics/1`) is **pure stdlib over INJECTED raw
scenario data**, with its pytest mirror
(`tests/test_portfolio_analytics.py`) — all **WRITTEN, NEVER RUN** (the
Phase 8 loop bans: nothing installed, nothing built, nothing executed).
Every hand-computed self-test expectation is a derivation from
constructed inputs with the arithmetic STATED in the test (e.g. 2.0 MW ×
0.5 h = 1.0 MWh; the ratio 4.75/5.0 = 0.95) — **test-grade arithmetic
pinning the analytic definitions**, NOT a measurement and NOT evidence
about any scenario. **Every measured cell in this record is EMPTY**
(ENTRY_NOTE boundary 4): no delivery ratio, shortfall, calibration
coverage, or discrepancy figure from a real scenario exists anywhere.

P8.02 S-6 scope note, binding here: portfolio analytics carries NO
falsifiable research question — its exit check is validation of derived
analytics against the deterministic baseline's accounting within a
stated tolerance on the tested scenarios, with limits recorded. This
record delivers exactly that and no hypothesis.

## 2. The analytics, each with formula, inputs, and uncertainty treatment (ledger step 1)

| Analytic | Formula (stated) | Inputs (raw scenario data) | Units / sign / uncertainty treatment |
|---|---|---|---|
| Available vs delivered flexibility | `available_mwh = available_mw × interval_hours`; `delivered_mwh = delivered_mw × interval_hours` | per-event records: `event_id`, `interval_hours` (> 0, h), `available_mw`, `delivered_mw` (≥ 0, MW), `direction` (declared `export`/`import` — the `P > 0` = export sign context is DECLARED per record, never encoded in a magnitude), `quality_state` (exactly one §10.2 state, CARRIED), optional SOC fractions in [0, 1] | MW / MWh; §10.2 canonical; telemetry quality states carried and mixed into the aggregate's `quality_state_mix` so a ratio built over STALE/ESTIMATED values is visible as such |
| Shortfall | `shortfall_mwh = Σ max(0, available_mwh − delivered_mwh)` per event then summed; over-delivery is carried SEPARABLY (`over_delivery_mwh = max(0, delivered − available)`), never netted into the shortfall (netting would silently hide a real shortfall elsewhere in the portfolio) | same events | MWh; honest deficits — the P8.06 rule-based baseline's deficit discipline, mirrored |
| Delivery ratio | `delivered_total_mwh / available_total_mwh` | the aggregate over events | dimensionless; `DIVISOR_ZERO` refused when the available total is 0 (a ratio over an empty denominator is never silently 1.0) |
| Uncertainty calibration | `coverage = |{records with declared_band_low ≤ actual ≤ declared_band_high}| / |records|` (band boundary INCLUSIVE) | forecast/observation pairs: band low/high (low ≤ high, inverted refused), actual, declared `unit` (MW or MWh — never assumed), `regime` | dimensionless coverage of the DECLARED band; the scope statement "assessed on the TESTED REGIMES ONLY" travels on every record (§6, the ledger's recorded limit) |
| Baseline-accounting comparison | per quantity, `absolute_difference_mwh = |analytics − baseline|`, compared against the PRE-REGISTERED tolerance | the aggregate over the SAME scenario + the deterministic baseline's accounting totals | MWh; the tolerance is injected and must pre-date the comparison (`TOLERANCE_NOT_PRE_REGISTERED` otherwise — the X-8 discipline applied to tolerances, never fitted after the difference is seen) |

## 3. The reporting-only boundary, enforced in code (ledger step 2; the ledger's named risk)

The ledger's named risk — *an analytic that silently becomes a dispatch
input (e.g. a "flexibility score" used to rank dispatch candidates
without validation)* — is made structurally impossible in the module:

| # | Rule | Enforcement |
|---|---|---|
| 1 | No dispatch-shaped capability EXISTS | the module namespace contains NO callable whose name carries dispatch/issue/authorize/approve/command/rank/score/schedule (pinned by a self-test and mirrored in the pytest suite) — there is no ranking, scoring, or scheduling function to abuse |
| 2 | No dispatch-shaped key can ride on an output | `assert_reporting_only(document)` refuses any dispatch-shaped key (`schedule`, `setpoint`, `dispatch_rank`, `flexibility_score`, `authorization`, `command`, …) at ANY depth of an output document (`REPORTING_ONLY_VIOLATION`) |
| 3 | Every output states its own status | every derived record carries `usage: "derived reporting only — … it never enters the dispatch lifecycle …"` verbatim |
| 4 | The lifecycle is not touched | no command-lifecycle state, no authorization token, no gateway path exists in the module; the outputs are RECORDS |

This is DERIVED REPORTING, not dispatch (the subtask's stated boundary):
the analytics are computed over raw scenario data for reports; they are
never an input to scheduling, feasibility, authorization, or any
dispatch-lifecycle stage. The verification is by inspection (the
namespace pin) plus the boundary tests — both exercisable at the human
gate without any framework.

## 4. Validation against the deterministic baseline's accounting (ledger step 3) — method recorded, cells EMPTY

The ledger's verification method — *"Recompute one analytic from raw
scenario data independently and compare"* — is exercisable because every
function is pure over raw records and the per-event records travel
INSIDE the aggregate (`per_event`), so an independent reviewer recomputes
the totals by hand from the carried rows and compares. The recorded
procedure (EMPTY until the first run, which also needs the frozen P8.06
baseline results artifact — §8.2):

1. pick one scenario from the §19.3 library version cited by the run;
2. hand-recompute `available_total_mwh`, `delivered_total_mwh`,
   `shortfall_total_mwh` from the raw event records (not from the
   aggregate) and compare;
3. run `compare_to_baseline_accounting` against the deterministic
   baseline's accounting on the SAME scenario with the pre-registered
   tolerance (recorded BEFORE the comparison, P8.06 freeze time at the
   earliest);
4. record any beyond-tolerance difference AS A DISCREPANCY
   (`discrepancy_fields`) — the discrepancy is the ledger's step-3
   deliverable ("report discrepancies"), never absorbed, never silently
   reconciled.

**Validation record — ALL MEASURED CELLS EMPTY:**

| Scenario id (RECORD-AT-FIRST-RUN) | Benchmark-set version | Independent recompute result | Baseline accounting vs analytics (per-quantity differences) | Tolerance (pre-registered) | Discrepancies reported |
|---|---|---|---|---|---|
| EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |

Hand-computed pin (test arithmetic, NOT a measurement): the constructed
two-event aggregate (available 1.0 + 4.0 = 5.0 MWh; delivered 0.75 + 4.0
= 4.75 MWh; shortfall 0.25 MWh) compared against a baseline record of
the same figures shows differences of exactly 0.0 within the 1e-9 MWh
tolerance; a baseline 0.2 MWh off against a 0.1 MWh tolerance is
REPORTED as a discrepancy on both affected quantities. The boundary
difference (0.1 == tolerance) is WITHIN (`<=`), pinned in the tests.

## 5. Where the analytics live (recorded placement)

`apps/worker/python/portfolio_analytics/` — mirroring the wave-2b/3b
research-worker locations (`apps/worker/python/gnn_worker/`,
`apps/worker/python/andes_worker/`, `apps/worker/python/marl_worker/`).
The ledger's "implement as reporting plugins behind the SDK (Phase 7)"
is the recorded activation seam (§8.1): this module is the pure
analytics core a Phase 7 SDK reporting plugin would wrap; no plugin
manifest, WIT world, or SDK contract is invented here (X-10 — a
contract gap is a follow-up, never a parallel contract).

## 6. The analytics' limits (ledger step 4) — recorded now

- **Calibration is assessed on the tested regimes only** — the coverage
  figure is a fact about the injected records; the scope statement
  travels verbatim on every calibration output
  (`CALIBRATION_SCOPE_STATEMENT`) and no generalization beyond the
  tested regimes is claimed anywhere (the ledger's named limit).
- The shortfall/ratio analytics describe the injected scenarios' declared
  events; they are not a settlement system, not a billing calculation,
  and no market/settlement conformance is claimed (§5.3 names the
  category, this record implements the research derivations only).
- A ratio built over degraded-quality telemetry (STALE/ESTIMATED values)
  is visible through `quality_state_mix`, but the module performs NO
  imputation and NO quality correction — degraded values are carried,
  never silently repaired (§10.2).
- The analytics are reporting-only (§3): no KPI here can become a
  dispatch, scheduling, or authorization input — the boundary scan and
  namespace pin hold by construction and by test.

## 7. What the module computes vs what the human run supplies

| Supplied by | Content |
|---|---|
| `analytics.py` (written, never run) | the analytic definitions (§2 formulas) and their refusal discipline; `validate_flexibility_event` (units/signs/quality/SOC discipline), `event_energy`, `aggregate_delivery` (with per-event records for independent recompute), `uncertainty_calibration` (scope statement carried), `compare_to_baseline_accounting` (pre-registered tolerance; discrepancies reported), `assert_reporting_only` (the boundary scan); hand-computed in-module self-tests |
| The human/lab run | the scenario runs and the raw event/forecast records; the frozen P8.06 baseline accounting (does not exist until the first run, `05_06_harness_baselines.md` §3); the pre-registered tolerance; the fills of §4; the SDK-plugin activation (§8.1) |

## 8. Contract gaps → Phase 7 follow-ups (recorded, not invented)

1. **The Phase 7 SDK reporting-plugin surface** (ledger step 2:
   "reporting plugins behind the SDK"): the P7.15 analytics/report
   profile category is the citation, but no plugin manifest/world for a
   reporting plugin exists to bind; the pure core is the wrappable
   implementation and the binding is the recorded activation task — a
   Phase 7 seam, never a parallel contract (X-10).
2. **The P8.06 frozen baseline-accounting artifact does not exist** (§3
   of `05_06_harness_baselines.md` recorded this): the §4 comparison
   cells cannot be filled until the first human/lab run freezes the
   baseline accounting per scenario.
3. **Settlement/performance derivations beyond the flexibility metrics**
   (ledger step 1 mentions "any settlement/performance derivations"):
   the report's §5.3 category names the class, but no settlement rule
   set exists in this repository to derive against — implementing one
   would be inventing a contract. The delivered analytics are the §19.4
   flexibility-metric set; any settlement derivation is recorded as a
   scope question for the P8.30 retrospective, not silently added.

## 9. Verification method and status (the human gate)

```sh
# (a) the pure self-tests (no framework needed) — WRITTEN, NEVER RUN in
#     this loop; the human gate runs them first:
python apps/worker/python/portfolio_analytics/analytics.py

# (b) the pytest mirror:
PYTHONPATH=apps/worker/python/portfolio_analytics python -m pytest \
  apps/worker/python/portfolio_analytics/tests -v

# (c) the boundary inspection:
grep -rn "dispatch\|authorize\|issue\|approve\|command" \
  apps/worker/python/portfolio_analytics --include=*.py
#     expected: hits only in the refusal vocabularies/comments — no
#     dispatch-shaped callable or gateway path exists

# (d) the documented-tools gate:
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (e) FIRST RUN — the §4 fills (the independent recompute + the
#     baseline-accounting comparison), per the recorded procedure.
```

A green (a)–(d) earns only a one-line "human build/test gate: PASSED
(date)" appended here; failures are recorded verbatim and the ledger tick
reverts to `- [ ]`. The measured cells of §4 stay EMPTY until the (e)
runs fill them.

## 10. Acceptance criteria — honest status

| Ledger acceptance criterion | Status |
|---|---|
| Analytics reproduce the baseline accounting within stated tolerance on the tested scenarios | **UNRUN** — the comparison method and refusal discipline are recorded (§4); the baseline artifact does not exist yet (§8.2); the tolerance must be pre-registered, never fitted |
| No dispatch influence (verified by inspection) | ENFORCED IN CODE (§3 — the namespace pin, the depth scan, the usage statements, no lifecycle field) and WRITTEN as tests; inspection is the human gate's (d) |
| Limits recorded | DONE — §6 (calibration on tested regimes only, carried quality states, no settlement claim) |

## 11. Non-claims (restated)

No dispatch influence; no field authority; no settlement/billing
conformance; no calibration claim beyond the tested regimes; no safety
or certification claim of any kind; no delivery ratio, shortfall,
coverage, or discrepancy figure from a real scenario exists anywhere in
this record or its code — the self-tests' constructed arithmetic pins
the definitions and is not a measurement.
