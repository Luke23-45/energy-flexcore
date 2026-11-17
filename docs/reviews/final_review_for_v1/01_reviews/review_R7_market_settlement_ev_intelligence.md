# Final Review R7 — Market/Settlement, EV Charging, Forecasting, Asset Intelligence (Iteration 1)

**Reviewer:** Agent A (wave 4)
**Date:** 2026-11-17
**Status:** REVIEWED — static read only (no build/test executed)
**Finding totals:** P0: 1 · P1: 5 · P2: 4

## 1. Scope reviewed

All counts are `wc -l` over the in-scope files (test code included in the LOC; test-only
`unwrap()`/`expect()` usage was excluded from defect analysis).

### Market/settlement (phase-16)

| File | LOC | Depth |
|---|---|---|
| `crates/flexcore-application/src/fixed_decimal.rs` | 559 | Deep read (full) |
| `crates/flexcore-application/src/settlement_models.rs` | 830 | Deep read (full) |
| `crates/flexcore-application/src/settlement_reconciliation.rs` | 579 | Deep read (full) |
| `crates/flexcore-application/src/payment_lifecycle.rs` | 984 | Deep read (full) |
| `crates/flexcore-application/src/revenue_share_billing.rs` | 1,619 | Deep read (headers + all arithmetic/state-machine sections: helpers 313–538, owner statements 640–847, reconciliation 849–975) |
| `crates/flexcore-application/src/bid_composition.rs` | 628 | Deep read (types, grant validation, composed-record shapes) |
| `crates/flexcore-application/src/baseline_engine.rs` | ~700 | Deep read (CBL selection, SDA application, compute path 250–700) |
| `crates/flexcore-application/src/market_price_records.rs` | 595 | Structural skim + targeted reads (segment labels, `records_from_publication`) |
| `crates/flexcore-application/src/settlement_meter_data.rs` | 570 | Structural skim (P16.10 record layer) |
| `crates/flexcore-application/src/capacity_market_records.rs` | 998 | Structural skim + LR-075 conflict record read (322–360) |
| `crates/flexcore-application/src/uk_lfm_records.rs` | ~950 | Structural skim (P483 refusal posture, MoneyAmount usage) |
| `crates/flexcore-application/src/performance_evidence.rs` | ~630 | Targeted read (mileage sign rule 340–375) |
| `crates/flexcore-domain/src/market_program_model.rs` | 1,250 | Structural skim + contract/enrollment/transition sections |
| `crates/flexcore-domain/src/market_registration_records.rs` | 1,131 | Structural skim |
| `crates/flexcore-events/src/market_events_v2.rs` | 460 | Deep read (full) |
| `crates/flexcore-authz/src/phase16_market_policies.rs` | 331 | Deep read (full) |
| `crates/flexcore-authz/src/action.rs` (market actions) | — | Targeted read (lines 180–252) |
| `crates/flexcore-simulator/src/market/paper_loop.rs` | 723 | Deep read (`run_paper_cycle` 299–470 + header) |
| `crates/flexcore-simulator/src/market/settlement_fixtures.rs` | 303 | Targeted read (render_cents/render_tenths, statement generator) |
| `crates/flexcore-simulator/src/market/price_feed.rs` | 474 | Structural skim |

### EV charging (phase-17)

| File | LOC | Depth |
|---|---|---|
| `crates/flexcore-domain/src/ev_soc_arbitration.rs` | 754 | Deep read (full) |
| `crates/flexcore-domain/src/ev_charging_composer.rs` | 1,230 | Deep read (rules, conversion, min-rate, reconciliation 1–880) |
| `crates/flexcore-domain/src/ev_proposal_records.rs` | 799 | Structural skim + header rules |
| `crates/flexcore-domain/src/ev_departure_constraints.rs` | 699 | Structural skim |
| `crates/flexcore-domain/src/ev_override_policy.rs` | 551 | Structural skim + header rules |
| `crates/flexcore-domain/src/ev_depot_load.rs` | 733 | Structural skim |
| `crates/flexcore-domain/src/ev_v2g_readiness.rs` | 667 | Deep read (header + gate/verdict logic 1–260, 415–520) |
| `crates/flexcore-domain/src/evse_registry.rs` | 773 | Structural skim |
| `crates/flexcore-domain/src/ev_fleet_model.rs` | 1,456 | Targeted read (V2xConfig gate 390–423, fleet validation 896–1030) |
| `crates/flexcore-application/src/ev_proposal_ingest.rs` | 410 | Deep read (header + API) |
| `crates/flexcore-application/src/ev_meter_values_ingest.rs` | 563 | Targeted read (SoC cadence/freshness config) |
| `apps/worker/src/ev_ingest.rs` | — | Structural skim (wiring verification only) |

### Forecasting / asset intelligence (phase-15)

| File | LOC | Depth |
|---|---|---|
| `crates/flexcore-application/src/forecast_integration.rs` | 681 | Deep read (header + source selection 240–370) |
| `crates/flexcore-application/src/fleet_health.rs` | 961 | Deep read (header + `evaluate_deviation` 267–395) |
| `crates/flexcore-application/src/forecast_baselines.rs` | 1,061 | Targeted read (persistence + seasonal-naive 456–501, no-skill statements) |
| `crates/flexcore-application/src/forecast_evaluation.rs` | 707 | Structural skim + metric/skill-score API |
| `crates/flexcore-application/src/forecast_pv.rs` | 512 | Structural skim + UNTRAINED labeling greps |
| `crates/flexcore-application/src/forecast_architecture.rs` | 1,014 | Structural skim (no-skill register) |
| `crates/flexcore-application/src/forecast_fixture.rs` | 2,022 | Structural skim (synthetic-provenance labeling) |
| `crates/flexcore-domain/src/bess_degradation.rs` | 1,172 | Deep read (header + full API surface + provenance rules) |
| `crates/flexcore-domain/src/bess_soh.rs` | 507 | Structural skim + deferral statements |
| `crates/flexcore-domain/src/bess_warranty.rs` | 1,084 | Deep read (terms validation, retention resolution, headroom 190–730) |
| `crates/flexcore-domain/src/warranty_evidence.rs` | 832 | Deep read (claim boundary, digest chain, generation 1–330) |
| `crates/flexcore-application/src/scheduling_objective.rs` (degradation-cost path) | — | Targeted read (lines 114–160, 1054–1060, 1383–1386) |
| `crates/flexcore-application/src/calibrated_model_records.rs` | ~600 | Structural skim (P15.14 declared-parameter home) |

## 2. Method

Static reads only (Read/Grep/`wc`); no build, no test, no code execution other than `wc -l`/`grep`/`sed` text census. The concrete checks:

1. **Fixed-decimal API call-site sweep.** Grepped every use of `FixedDecimal`/`MoneyAmount` and each public API member (`parse`, `from_parts`, `mantissa`, `scale`, `is_zero`, `canonical_string`, `rescaled_to`, `checked_sub`, `partial_cmp`, `value`, `currency_code`) across all 11 consuming files (`market_price_records`, `payment_lifecycle`, `performance_evidence`, `revenue_share_billing`, `settlement_meter_data`, `settlement_models`, `settlement_reconciliation`, `uk_lfm_records`, `tests/market_integration_p16`, `simulator/market/paper_loop`, `fixed_decimal` itself). Arity and semantics matched at every call site. Result: **consistent** (one arithmetic hazard inside `fixed_decimal`'s only consumer-negation path — FRV1-R7-001).
2. **money-arithmetic correctness review.** `percent_of_amount` (exact mantissa multiply, scale+2, overflow/scale-checked), `sums_exactly` (chained checked-sub zero-remainder proof), `Tolerance::Within` inclusive-boundary comparison, currency-per-chain and cross-currency refusals — all traced end to end. Result: correct except FRV1-R7-001 (unchecked i128 negation) and FRV1-R7-004 (SDA zero-reference case).
3. **market_events_v2 payload cross-checks.** No emitter exists (grep: `MarketEventV2::` constructed only in `market_events_v2.rs` itself) — **UNWIRED-RECORDED** per the module header ("no producer is wired"). Payload vocabulary verified against the named homes: `MarketStructureDeclaration`/`MarketBidGrant` exist in `bid_composition.rs`; `segment_label` spellings `"day-ahead"`/`"real-time"` match `MarketSegment::as_str` (market_price_records.rs:94–95); `source_format_label` `"pjm-manual-29"` matches `SettlementFormat::as_str` (settlement_models.rs:88); `resolution_kind_label` `"upheld"` matches `ResolutionKind::Upheld.as_str` (dispute_workflow.rs:430); `challenge_kind` derivation exists (`challenge_kind_from_reason_code`, dispute_workflow.rs:217). Result: **consistent**.
4. **phase16 authz actions cross-check.** `Action::{MarketPriceRead, MarketBidCompose, MarketGrantIssue, MarketGrantRevoke, MarketGrantRead, MarketSettlementRead}` defined with ids `market.price.read`, `market.bid.compose`, `market.grant.*`, `market.settlement.read` (action.rs:186–211, 252); policy grants match the recorded role table; never-granted four unparseable. No runtime enforcer consumes the phase-16 set outside the authz crate — **cross-reference FRV1-R2-009** (three recorded grant sets composed nowhere); not re-counted as an R7 finding.
5. **Wiring-status table** (grep for construction/consumption of every engine across `crates/` and `apps/`, excluding the owning module and tests):

| Module cluster | Status | Evidence |
|---|---|---|
| Price records / settlement extractors / reconciliation / bid composition | **WIRED (simulation only)** | consumed by `flexcore-simulator/src/market/paper_loop.rs` + `price_feed.rs` + `settlement_fixtures.rs` |
| Domain market program/contract/enrollment/stacking stores | **UNWIRED-RECORDED** | zero consumers in adapters/apps; overlaps FRV1-R5-002 (postgres `market_records_store` family also consumer-less) — cross-referenced, not duplicated |
| `revenue_share_billing` (config, disaggregation, owner statements) | **UNWIRED** | no consumer outside own module/lib.rs; ownership honesty statements recorded ("no payment record exists", "never rendered, printed, sent") |
| `payment_lifecycle` (`PaymentLifecycleStore`) | **UNWIRED-RECORDED** | no consumer; paper loop carries only a payment-state *reference string*; the no-payment honesty statement records the empty-store posture |
| `baseline_engine` (CBL/SDA) | **UNWIRED-RECORDED** | header records "no store"; no consumer |
| `fleet_health` | **UNWIRED-RECORDED** | header records "no I/O, no store, no scheduler wiring"; lib.rs export only; no grid-api/storage/console consumer |
| `forecast_integration` / `forecast_baselines` / `forecast_evaluation` / `forecast_pv` / `forecast_architecture` / `forecast_fixture` | **UNWIRED-RECORDED** | headers record record-layer/control status; no consumer outside lib.rs |
| `ev_charging_composer` | **WIRED** | `adapters/protocol-ocpp/src/charging.rs`, `flexcore-simulator/src/adapters/sim_ocpp.rs` |
| `evse_registry` | **WIRED** | `adapters/protocol-ocpp` (csms, provisioning, conformance) |
| `ev_proposal_records` → `ev_proposal_ingest` | **WIRED** | `apps/worker/src/ev_ingest.rs` |
| `ev_meter_values_ingest` | **WIRED** | `apps/worker/src/ev_ingest.rs` |
| `ev_soc_arbitration` | **UNWIRED-UNRECORDED** | zero consumers; not consumed by its natural sibling `ev_meter_values_ingest`; FRV1-R7-006 |
| `ev_departure_constraints` / `ev_depot_load` / `ev_override_policy` / `ev_v2g_readiness` | **UNWIRED (record-layer posture)** | domain-only; phase-17 wave-2 record layers; no composition-root consumer, consistent with the program's written-unrun posture |
| `bess_degradation` | **WIRED (domain-internal)** | consumed by `bess_warranty`, `warranty_evidence` |
| `scheduling_objective` degradation-cost input (P15.08) | **WIRED (application)** | consumes `DegradationCostDeclaration` (flexcore-optimization-api); linear/piecewise-linear validation |
| `bess_warranty` / `warranty_evidence` | **UNWIRED (library)** | pure domain functions; no app/adapter consumer |
| `market_events_v2` | **UNWIRED-RECORDED** | module header records "no producer is wired" |
| `phase16_market_policies` | **UNWIRED-RECORDED** | cross-ref FRV1-R2-009 |

6. **Non-test `unwrap()`/`expect()` sweep.** `#[cfg(test)]` boundary located per file; all `unwrap()`/`expect()` occurrences checked against it. Result: clean everywhere except four *guarded* spots in non-test code (FRV1-R7-010) and one `expect` inside a peek-guarded loop (ev_charging_composer.rs:848, same class). No swallowed errors found (every `map_err` carries a named variant).
7. **State-machine completeness.** Payment lifecycle (closed 3-state, frozen table, PAID terminal — recorded design), owner statement (DRAFT→RECONCILED, closed), contract/enrollment/program transitions (supersede pattern), dispatch untouched everywhere. Result: complete per recorded designs except FRV1-R7-003 (transition-id uniqueness).

## 3. Checks performed, all consistent

1. `fixed_decimal.rs` defines no `From<f64>`/`TryFrom<f64>`; every construction path is `parse`/`from_parts`; verified by reading the full module and grepping for float conversions in all consumers.
2. Scale handling: `checked_sub` aligns to the common larger scale; `rescaled_to` refuses downscale (named refusal); equality/ordering compare values across scales; `canonical_string` preserves published scale. All consumers depend only on these verified behaviors.
3. Currency discipline: `MoneyAmount::checked_sub` refuses mixed currencies; `reconcile_bli_statement` refuses cross-currency lines at run level; `StatementReconciliationOutcome::run` refuses mixed currencies across both sides; payment chains carry one currency end-to-end. All verified.
4. `PjmBliLineItem` field usage in the reconciliation matches the model exactly (`line_ref`, `amount`, `charge_type`, `statement_ref`); `ExpectedLine`/`LineOutcome`/`DisputeQueueEntry` round-trip consistently between `settlement_models` and `settlement_reconciliation`.
5. CSV/XML extractor `line_ref` construction (`{statement_ref}#{row_index}`) is consistent between `extract_pjm_m29_bli_csv` (settlement_models.rs:255), `simulated_pjm_csv_statement` (settlement_fixtures.rs:77), and the paper loop's expected-set builder (paper_loop.rs:396) — the loop's reconciliation identities line up by construction.
6. Interval-length validation: CAISO SQMD = 300,000 ms, EBGL ISP = 900,000 ms; inverted intervals refused; re-binning never performed. Verified in both extractors and their tests.
7. Sign conventions: EBGL imbalance direction derived under the declared sign convention; zero imbalance carried; PJM statement amounts carried verbatim with sign; baseline engine's consumption-magnitude deviation from the P>0-export convention is explicitly documented in `DayProfile` (baseline_engine.rs:~428); performance-evidence mileage refuses negative magnitude rather than sign-flipping (performance_evidence.rs:357–363).
8. EV composer: precedence ranks (0=external constraints … 3=TxProfile) match `STACK_PRECEDENCE_STATEMENT`; the A↔W conversion formula is recorded verbatim in every decision record; min-rate to-zero pairing enforced with named decisions; unit-mismatch reconciliation refuses instead of converting.
9. EV proposal path: `InboundProposalMessageName::maps_to()` kinds/sources match `EvProposalKind`/`EvProposalSource` vocabulary; proposal records are structurally PROPOSED; no command constructor exists in the ingest module.
10. V2G gate: `ev_v2g_readiness` consumes `V2xConfig::is_enabled()` and has no enable path of its own; `V2xConfig::disabled()` is the default (ev_fleet_model.rs:390); the five-condition enable procedure refuses incomplete evidence. PnC is not implemented anywhere; `adapters/protocol-ocpp/src/scope.rs:45` records "no ISO 15118 Plug&Charge PKI implementation" — PHASE-GATED, no default-on. Clean.
11. ADR-010 in the intelligence cluster: `fleet_health` health scores/issues/KPIs are records with no dispatch-reach function; `bess_degradation` computes no degradation model or cost; degradation costs enter the optimizer only as `DegradationCostDeclaration` linear/piecewise-linear data with non-convex data refused (scheduling_objective.rs:154–156, 1383–1386) and out-of-scope chemistries refused (P15-E-3).
12. Warranty honesty: `compute_headroom` returns `NotCreditable` (never a guess) for UNKNOWN provenance, stale SoH, or no SoH; `RetentionResolution::beyond_schedule_coverage` records the no-extrapolation limitation; the evidence pack carries `CLAIM_SUPPORT_STATEMENT` inside every pack (supports a human claim, never asserts one).
13. Forecast honesty: `NO_SKILL_STATEMENT` (forecast_baselines.rs:69), UNTRAINED-first-class labeling in `forecast_pv` (lines 63, 309–355), no-skill register in `forecast_architecture` (§4), empty evaluation cells recorded in `forecast_evaluation`. `forecast_integration` selects sources with explicit fallback reasons (NoPublishedForecast / ClockBeforeComputation / ForecastSuperseded / MaxAgeExceeded) and never silently consumes a stale forecast.
14. `worse_with_staleness` never upgrades: BAD survives staleness, STALE input stays STALE, SUSPECT survives freshness (verified against the frozen six-state vocabulary; the `staleness` helper is consumed from `quality_rules`, not redefined).
15. `persistence_forecast` (last-value carry) and `seasonal_naive_forecast` (declared wrap rule, index bounds proven) are deterministic and sign/unit-preserving; no skill claim anywhere.
16. No non-test `unwrap()` in any in-scope domain file (`ev_proposal_records`, `ev_v2g_readiness`, `evse_registry`, `ev_override_policy`, `ev_departure_constraints`, `ev_depot_load`, `market_program_model`, `market_registration_records`); `ev_fleet_model`'s single `expect` (line 1025) is guarded by the `NoVehiclesDeclared` refusal at line 906.
17. `PaymentLifecycle` chain validation ordering: lifecycle-exists → state match → frozen edge → basis → chain link → currency, all before the append; PAID terminal enforced twice (table + store).

## 4. Findings

### FRV1-R7-001 [P0] [phase-16] Unchecked i128 negation in reconciliation tolerance can panic (debug) or silently accept a catastrophic mismatch (release)

- Status: OPEN
- Location: `crates/flexcore-application/src/settlement_reconciliation.rs:339–344` (with `crates/flexcore-application/src/fixed_decimal.rs:203–211`, `286–298`)
- Issue: The `Tolerance::Within` path negates the deviation mantissa **unchecked**: `FixedDecimal::from_parts(-deviation.value().mantissa(), ...)` (line 343). `from_parts` performs no magnitude bound check, and `checked_sub` can legitimately produce `i128::MIN` from two purely `parse`-able operands — e.g. `parse("-1").checked_sub(parse("170141183460469231731687303715884105727"))` succeeds with mantissa `i128::MIN` (the subtraction is exactly representable, so no named refusal fires). Negating `i128::MIN` then (a) panics in debug builds — contradicting the module's recorded invariant that the reconciliation "surfaces mismatches as records, never as panics" (fixed_decimal.rs:302–307) — or (b) in release, wraps back to `i128::MIN`, so the "magnitude" remains negative; `magnitude.partial_cmp(&bound)` then yields `Less`, which the code maps to `Some(_) => true` (line 356) — the largest possible deviation is declared **within tolerance** and the line is matched. This is precisely the "reconciliation tolerance logic that can silently accept large discrepancies" class: a 39-digit adversarial or corrupt statement amount passes reconciliation without a dispute-queue entry.
- Recommendation: Replace the parts-negation with an explicit checked operation in `fixed_decimal` (e.g. a `checked_neg`/`unsigned_abs` helper returning `FixedDecimalError::MantissaOverflow` on `i128::MIN`) and map it to `ReconciliationError::DecimalInvalid` at settlement_reconciliation.rs:342. Minimal, local; the existing tests continue to pass.

### FRV1-R7-002 [P1] [phase-17] SoC arbitration contradicts its own recorded policy statement on the stale-OCPP fallback

- Status: OPEN
- Location: `crates/flexcore-domain/src/ev_soc_arbitration.rs:21` (header rule 3), `:94–98` (`OCPP_SOC_AUTHORITATIVE_WHILE_PLUGGED_STATEMENT`), vs `:337–358` (`arbitrate_soc`) and `:590–606` (pinning test)
- Issue: The module header says "Telematics are the fallback when plugged and the OCPP SoC is **stale or absent**" (line 21) and the pinned statement constant says "when stale or absent the fresh telematics value is the fallback (and an aged one carries STALE)" (lines 97–98). The implementation does the opposite: `arbitrate_soc` returns the OCPP observation whenever one is *present*, regardless of staleness, and never falls back to telematics (lines 338–358); the inline comment at lines 360–362 re-records the implemented rule ("a stale-but-present OCPP value still wins"), and the test `a_stale_but_present_ocpp_value_still_outranks_fresh_telematics` (lines 590–606) pins it. Both the audit-displayed statement text and the behavior cannot be right. Since the constant is the text a human would read in records/exports, one of the two is wrong in the audit trail; this is also the recorded-precedence case most likely to be cited in a dispute.
- Recommendation: Decide once (human owner, record `23_p17_08_soc_arbitration.md`) whether stale-OCPP-wins or stale-OCPP-falls-back, then either fix `arbitrate_soc` to honor the header (fall back to a *fresh* telematics value when the OCPP value is stale, carrying both values on the outcome) or correct rule 3 in the header and the text of `OCPP_SOC_AUTHORITATIVE_WHILE_PLUGGED_STATEMENT` to the implemented rule. The behavior and its test must end up byte-consistent with the statement.

### FRV1-R7-003 [P1] [phase-16] Payment-lifecycle chain accepts duplicate transition ids and mutates chain storage before all checks pass

- Status: OPEN
- Location: `crates/flexcore-application/src/payment_lifecycle.rs:573–632` (`apply_transition`), `:606–613`, `:623–631`
- Issue: Two gaps in the chain-integrity mechanism the module advertises ("the store refuses a transition whose chain link does not match the chain's actual head", lines 68–72). (1) `transition_id` uniqueness is never checked — `PaymentTransitionRecord::new` (302–340) and `apply_transition` both accept a record whose id duplicates an earlier chain entry, so a chain can hold two transitions with the same id and a later `previous_transition_id` reference is ambiguous; the "verifiable linear record" claim is weakened. (2) `self.chains.entry(key.clone()).or_default()` (line 606) runs **before** the chain-link and currency checks; if the currency check at 615–622 refuses, an empty `Vec` remains in `chains` for the key, so `chain_of` subsequently returns `Some(&[])` for a lifecycle that never had a successful transition — an observable inconsistency versus the pre-call `None`.
- Recommendation: Enforce per-chain `transition_id` uniqueness in `apply_transition` (a new named error variant, e.g. `DuplicateTransitionId`), and defer the `chains` insertion (or remove the inserted empty entry on refusal) so a refused transition leaves no observable state change.

### FRV1-R7-004 [P1] [phase-16] Baseline engine: SDA with a zero baseline window sum silently records a no-op adjustment instead of a named gap

- Status: OPEN
- Location: `crates/flexcore-application/src/baseline_engine.rs:640–655` (SDA ratio computation)
- Issue: When the configuration declares an SDA, the raw ratio is computed as `if baseline_window_sum > 0.0 { ... } else { 0.0 }`. For a selected-day set whose baseline window sums to exactly 0.0 (all-zero intervals — possible for a low-usage eligible-day set), the engine returns `Computed` with `sda_applied: Some(SdaApplication { raw_ratio: 0.0, applied_ratio: 0.0, ... })`, i.e. an *applied* adjustment record that actually adjusted nothing, while the day-of actuals may be non-zero. Given the module's own discipline ("the engine's outcome: a computed profile OR a recorded gap row — never a silent approximation (LR-073)", lines ~598–600) and that `MissingSdaWindow` is already a named gap, this undefined-ratio case should not silently pass as a zero adjustment.
- Recommendation: Add a named gap reason (e.g. `SdaReferenceWindowZero`) returned when `baseline_window_sum <= 0.0`, so the anomaly surfaces in the gap record rather than as a raw=0.0 "applied" adjustment.

### FRV1-R7-005 [P1] [phase-16] CAISO's 20% SDA bound is enforced as a universal jurisdiction bound for every program

- Status: OPEN
- Location: `crates/flexcore-application/src/baseline_engine.rs:625–633` (`compute_baseline` SDA pre-check), `:67–69` (`CAISO_SDA_MAX_PERCENT`)
- Issue: `compute_baseline` refuses **any** program whose declared SDA cap exceeds `CAISO_SDA_MAX_PERCENT` (20.0), and the gap record names the CAISO constant as `jurisdiction_bound_percent` — even for a non-CAISO pack whose jurisdiction may permit a larger symmetric cap. The in-code comment (lines ~617–624) acknowledges the conflation ("the jurisdiction's bound is declared BY the pack, and the CAISO constant is the recorded sourced bound"), but the implementation makes the documented "program-configurable" engine unreachable for any pack declaring >20%, with an error message that misattributes the bound's jurisdiction. The supported set (`CBL_10_IN_10`, `CBL_5_IN_10`, `DAY_MATCHING`) is jurisdiction-agnostic elsewhere.
- Recommendation: Carry the jurisdiction bound as per-pack declared data (a field on the jurisdiction record pack, with `CAISO_SDA_MAX_PERCENT` as the CAISO pack's declared value) and refuse against the pack's own bound; keep the CAISO constant as the CAISO pack's sourced default.

### FRV1-R7-006 [P1] [phase-17] `arbitrate_soc` has zero consumers — the wired MeterValues ingest path never arbitrates SoC

- Status: OPEN
- Location: `crates/flexcore-domain/src/ev_soc_arbitration.rs:296–401` (the only public entry point); wiring grep: no consumer outside the module and its tests
- Issue: The P17.08 dual-source arbitration — the piece that decides which SoC is the participation input — is never called anywhere in the workspace. Its natural sibling, `crates/flexcore-application/src/ev_meter_values_ingest.rs` (P17.07), is wired into `apps/worker/src/ev_ingest.rs`, declares per-quantity SoC cadence and freshness limits (lines 134–186), and computes staleness for MeterValues — but never invokes `arbitrate_soc`, and no other module does either. Unlike every sibling module cluster in this review, whose unwired status is recorded in the module header ("no scheduler wiring", "no producer is wired", "no store"), the P17.08 header and the decision record record no wiring deferral. Status: **UNWIRED-UNRECORDED**.
- Recommendation: Either wire the arbitration into the MeterValues/telematics ingestion path in the worker (the declared plug-state and freshness limits already flow there), or add an explicit recorded-seam statement (the `RAINFLOW_EXTRACTION_SEAM` pattern from `bess_degradation.rs:80`) so the non-consumption is a recorded decision, not an oversight.

### FRV1-R7-007 [P2] [phase-16] `EmptyField` is used as the error variant for non-empty semantic failures across the settlement cluster

- Status: OPEN
- Location: `crates/flexcore-application/src/payment_lifecycle.rs:321–325, 453–457` (`recorded_at_unix_ms < 0` → `EmptyField { field: "recorded_at_unix_ms" }`); `crates/flexcore-application/src/revenue_share_billing.rs:439–443, 591–595` (same pattern); `crates/flexcore-application/src/bid_composition.rs:310–322` (invalid grant state → `EmptyField { field: "state" }`, inverted validity window → `EmptyField { field: "valid_until_unix_ms" }`); `crates/flexcore-application/src/baseline_engine.rs:437–440` (non-finite interval values → `EmptyField { field: "interval_values_mw" }`); `crates/flexcore-domain/src/market_program_model.rs` (contract `recorded_at_unix_ms < 0` → `EmptyField`); also `revenue_share_billing.rs:343–350` (`percent_of_amount` bounds refusal carries `asset_id: String::new()` — a placeholder identity in a named error)
- Issue: The field in question is not empty; it is negative, out-of-vocabulary, non-finite, or inverted. The machine-readable name (`*_EMPTY_FIELD`) and the prose ("must be non-empty") misdescribe the failure, which degrades the error catalogue's value for operator triage and audit (the P3.04 style the modules otherwise follow).
- Recommendation: Introduce the two missing named variants (`NegativeTimestamp`/`InvalidEnumValue`/`NonFiniteValue` as appropriate) or reuse existing ones (`FieldTooLong`-style siblings already exist), and give `ShareOutOfBounds` a real asset-id parameter at the `percent_of_amount` call sites.

### FRV1-R7-008 [P2] [phase-16] Dead `SettlementParseError::CurrencyInvalid` variant and a duplicated `render_tenths` helper

- Status: OPEN
- Location: `crates/flexcore-application/src/settlement_models.rs:110` (variant), `:127` (name), `:151–153` (Display), constructed only in the coverage test at `:820`; currency refusals in real paths ride `DecimalInvalid` via `MoneyAmount::parse` (`:248–253`, `:364–369`); `render_tenths` defined twice: `crates/flexcore-simulator/src/market/settlement_fixtures.rs:65–69` and `crates/flexcore-simulator/src/market/price_feed.rs:137`
- Issue: `CurrencyInvalid` is unreachable in non-test code (the money-record rule surfaces all currency-shape failures as `DecimalInvalid`), so its prose path is dead and the coverage test exercises a variant no caller can hit. The duplicated tenths renderer violates the crate's single-home discipline for record recipes; the two copies are currently byte-identical but can drift independently.
- Recommendation: Either construct `CurrencyInvalid` at the two `MoneyAmount::parse` call sites (re-mapping `FixedDecimalError::InvalidCurrencyCode`) or remove the variant; keep one `render_tenths` (settlement_fixtures is the statement-recipe home) and re-export it for `price_feed`.

### FRV1-R7-009 [P2] [phase-17] SoC-observation quality refusals reuse the `PlugStateQualityUnusable` error variant

- Status: OPEN
- Location: `crates/flexcore-domain/src/ev_soc_arbitration.rs:209–218` (`SocObservation::measured_quality`), raised for SoC observations at `:339` and `:364`; variant prose at `:163–167`
- Issue: When an OCPP or telematics **SoC** observation carries `MISSING`/`ESTIMATED`, the error returned is `SocArbitrationError::PlugStateQualityUnusable` with the Display text "the plug-state observation carries quality …". The offending record is an SoC observation, not a plug-state observation; the audit trail names the wrong input.
- Recommendation: Add a `SocQualityUnusable { source: SocSource, quality: TelemetryQuality }` variant (or parameterize the existing one with the input kind) and use it from `measured_quality`.

### FRV1-R7-010 [P2] [phase-15/16/17] Guarded `expect()`s in non-test code where the crate style is a named refusal or an invariant note

- Status: OPEN
- Location: `crates/flexcore-application/src/payment_lifecycle.rs:629` (`expect("checked above")` re-lookup after `get`); `crates/flexcore-application/src/forecast_integration.rs:296` (`expect("checked above")` behind a match that guarantees `published` is `Some`); `crates/flexcore-domain/src/ev_charging_composer.rs:848` (`expect("peeked")` behind a peek); `crates/flexcore-application/src/revenue_share_billing.rs:340–341, 456–457, 469–473, 476` (`expect` on `from_parts` constants and `MoneyAmount::new` with a fixed synthetic currency)
- Issue: All four spots are provably infallible as written (each is guarded by an immediately preceding check or by construction), so this is not a correctness defect — but the codebase's stated convention is named refusals and no panics in non-test code, and these `expect`s rely on reviewer-traced guards rather than types. They are the only `expect`s outside `#[cfg(test)]` in the entire reviewed area.
- Recommendation: Prefer restructuring the re-lookups to carry the value out of the guard scope (entry-API or `if let`), and for the constant constructions either hoist them to `OnceLock`-style constants built with `unwrap` at definition with a documented invariant comment, or accept and record the pattern once in `.agent/WRITING.md` so future modules do not multiply it.

## 5. Release-readiness verdict for this area

The area is structurally strong: the fixed-decimal money layer is disciplined and its API is used consistently at every call site (no float paths, no currency mixing, exact percent arithmetic, overflow-checked everywhere except the one negation in FRV1-R7-001); the settlement parsers, reconciliation engine, bid composer, CBL/SDA baseline engine, EV composer, V2G gate, BESS provenance chain, and forecast honesty statements all implement their recorded rules faithfully, and the simulator paper loop genuinely wires the market engines end to end in a records-only posture. Before release, FRV1-R7-001 must be fixed (it is the only place where money math can panic or silently accept a discrepancy), and FRV1-R7-002/003 should be resolved because they sit on audit-facing invariants (a self-contradicting recorded SoC policy statement and an ambiguous payment chain identity). FRV1-R7-004/005 are honest-behavior gaps in the baseline engine that should be closed or explicitly recorded as v0.x limitations, and FRV1-R7-006 needs either a wire-up or a recorded seam. The remaining items are polish. The widespread unwired status of the record-layer stores and forecast/health modules is the program's recorded written-unrun posture (cross-referenced to FRV1-R5-002 and FRV1-R2-009) and is not itself treated as a defect.
