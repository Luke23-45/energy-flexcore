# P15.19 — Decision records: P15-E-1, P15-E-2, P15-E-3 verified registered OPEN with DEFAULTs — the consolidated register record (no E-decision is closed)

| Field | Value |
|---|---|
| Subtask | P15.19 (decision records: P15-E-1 weather-data build-vs-buy, P15-E-2 price-forecast vendor seam, P15-E-3 degradation-model chemistry coverage — each verified registered OPEN with its DEFAULT; the consolidated statement of what closes each; agents never close E-decisions, conventions §4) |
| Wave / agent | Phase 15 wave 2, Agent B (P15.19–P15.20) |
| Status | REGISTER VERIFICATION ONLY — this record closes nothing. All three E-decisions are OPEN at close time; every closer named below is the human owner. No vendor account, API token, budget, jurisdiction entry, fleet review, or per-chemistry parameter set exists (UNSIGNED / EMPTY / NOT-STARTED). |
| Sources | The owning wave-1 records: `records/03_p15_03_pv_weather_seam.md` §3 (P15-E-1), `records/04_p15_04_price_ingestion_entsoe.md` §3 (P15-E-2), `records/21_p15_07_bess_degradation_data_model.md` §4 (P15-E-3); `plan.md` §4 (the full E-decision text); `ledger_phase_15.md` §E-decision register; gap analysis §11 ([S-41][S-50][S-51] forecasting table stakes; [S-111] ENTSO-E; [S-112] Amperon-class vendor-forecast analogy; [S-145][S-146][S-147] degradation model sources); program conventions §4 (agents never close) and §5 (human-gate inputs) |
| Files touched | This record only. `plan.md`, `ledger_phase_15.md`, `context.md`, and every other file are untouched by this subtask. |

## 1. The register verification (what was checked, what was found)

Each E-decision was verified in ALL THREE places the conventions require it to
live — the plan's decision section, the ledger's decision register, and the
owning subtask's decision record — as OPEN with its DEFAULT (RECOMMENDATION)
and its named closer:

| ID | `plan.md` §4 | `ledger_phase_15.md` register | Owning decision record | Verdict |
|---|---|---|---|---|
| P15-E-1 (weather/irradiance data: build vs buy) | OPEN entry with options (a)/(b)/(c), DEFAULT (a) | OPEN row, DEFAULT "Buy from a commercial vendor, ingested as data with provenance" | `records/03_p15_03_pv_weather_seam.md` §3 (registered at wave 1); mirrored as `P15_E_1_STATUS` in `crates/flexcore-application/src/forecast_pv.rs` (pinned OPEN by test) | VERIFIED — registered, OPEN, no agent closed it |
| P15-E-2 (price-forecast vendor seam) | OPEN entry with options (a)/(b)/(c), DEFAULT (a) | OPEN row, DEFAULT "Amperon-class vendor forecasts ingested as data [S-112]" | `records/04_p15_04_price_ingestion_entsoe.md` §3 (registered at wave 1); the `VendorForecast` kind already exists in `crates/flexcore-ports/src/price_signal.rs` so the seam exists without any vendor integration | VERIFIED — registered, OPEN, no agent closed it |
| P15-E-3 (degradation-model chemistry coverage) | OPEN entry with options (a)/(b)/(c), DEFAULT (a) | OPEN row, DEFAULT "LFP and NMC first per BLAST-Lite [S-147]" | `records/21_p15_07_bess_degradation_data_model.md` §4 (registered at wave 1); the DEFAULT is implemented only as recorded there (`DegradationChemistry::Lfp` / `Nmc` in scope; out-of-scope chemistries are `OutOfScope` recorded limitations) | VERIFIED — registered, OPEN, no agent closed it |

**No register entry was missing** — each E-decision was already registered by
its owning wave-1 record exactly as `plan.md` §4 expected — so no new register
entry was created beyond this consolidated record. Nothing in this phase
closes any E-decision: the DEFAULTs that WERE applied (the weather port, the
`VendorForecast` seam kind, the LFP/NMC scope enums) were applied only because
the subtasks explicitly implement the recommended default, with the dependence
recorded in each owning record (conventions §4).

## 2. What closes each E-decision (the consolidated closer statement)

The closers were named in `plan.md` §4 and in each owning record; this
section restates them in one place so the human-gate queue (CHECKLIST) can
cite a single section. **In every case the closer is the human owner; no
agent, plugin, or automation can close any of the three.**

| ID | What closes it | The human-gate inputs (all EMPTY/UNSIGNED/NOT-STARTED at close) | What the closure unblocks |
|---|---|---|---|
| P15-E-1 | The human owner's **budget and vendor selection** for weather/irradiance data | The vendor account/API key — a HUMAN GATE input, UNSIGNED until a human registers it; the budget line for the bought service; the vendor configuration naming (vendor names live in deployment configuration, never in the port — record 03 §1 vendor-neutrality rule) | An actual `WeatherDataProvider` implementation behind `crates/flexcore-ports/src/weather_data.rs`; the first real weather sample ingested as DATA with provenance; the degree-day baseline's (P15.02) real weather arguments |
| P15-E-2 | The human owner's **vendor selection and market-jurisdiction register entry** (program README hard status boundary 4; conventions §4) | The vendor-forecast account (UNSIGNED); the jurisdiction in which prices will be consumed (a register entry, not an agent act); the ENTSO-E Transparency API token — UNSIGNED until a human registers it (record 04 §2 token gate) | Wiring the `A44Transport` seam to a real HTTP client; ingesting the first `PriceSeries::Actual` or `::VendorForecast` data; Phase 16's consumption of whatever lands (nothing in this phase bids) |
| P15-E-3 | The human owner's **fleet-composition review**, plus per-chemistry parameters as human-gate inputs | The fleet composition (which chemistries the actual fleet contains); a reviewed parameter set per chemistry (LFP/NMC first per the DEFAULT) declared WITH provenance — ASSUMPTION-grade until reviewed; the out-of-scope chemistries confirmed as recorded limitations | Activating any `DegradationCostCurve` in the optimizer (until P15-E-3 closes, every curve value is ASSUMPTION-grade declared data — record 22 §5 item 1; `require_in_scope()` refuses out-of-scope chemistry use) |

## 3. The closure discipline (what closure may and may NOT mean)

1. Closing P15-E-1 or P15-E-2 authorizes INGESTION of bought data as DATA
   with provenance — never a bid, never a dispatch input (ADR-010 extension
   and ADR-011 unchanged; the price port's trait surface has no
   bid/offer/order vocabulary — record 04 §2).
2. Closing P15-E-3 activates cost-curve DATA for the reviewed chemistries
   only; the solver-class ceiling is unchanged by closure: curves remain
   linear/piecewise-linear optimizer INPUT data (conventions §6.5), and the
   P15.08 guard tests prove the MISOCP refusal and tier guards stay
   regardless of what data closes in.
3. A closure is a RECORDED human decision (an update to the register rows in
   `plan.md` §4 and `ledger_phase_15.md`, with the decision's evidence), not
   a code edit and not a silent relabel; the in-code status constants
   (`P15_E_1_STATUS`) are updated in the same recorded change or not at all.
4. §13.3 semantics apply to anything the closure eventually enables: when a
   vendor feed, a token-gated ingestion, or a parameterized cost curve is
   first exercised, the outcome is a test/lab observation at the tested time
   under the registered configuration — never a guarantee, and (rung-3
   ceiling, Phase 12 record 51 §2) no number of green runs converts it into
   one.

## 4. Honest boundaries

1. This record is a register VERIFICATION and consolidation; it introduces
   no new external claim and cites only the sources its three E-decisions
   already cite ([S-41][S-50][S-51], [S-111], [S-112], [S-145]–[S-147]).
2. No vendor, product, jurisdiction, or parameter VALUE is named as a fact:
   Solcast/DNV/Amperon-class naming remains a RECOMMENDATION label (records
   03 §4 item 3, 04 §1), never a register source.
3. The verification is an on-disk inspection of `plan.md`,
   `ledger_phase_15.md`, and the three owning records performed 2026-11-14 —
   a read, not a tool run; it claims nothing about runtime behavior.
4. No checkbox was ticked by this agent anywhere (conventions §2; the
   P15.19/P15.20 ticks are the orchestrator's post-close action).
