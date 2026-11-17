# Phase 16 — Market Integration, Settlement and Revenue — Context

Date: 2026-11-14. Status: **NOT STARTED** (0/24 — see `ledger_phase_16.md`).
This directory is PLANNING ONLY: no code, crates, or manifests are created by
this program's planning step (program conventions §3). Citations `[S-n]`
resolve in `../00_GAP_ANALYSIS.md` §11; the rules in
`../01_PROGRAM_CONVENTIONS.md` bind everything below.

## 1. Why this phase exists

Three rows of the commercial table-stakes analysis (gap analysis §3) are the
business case. All three are currently absent:

| # | Table-stakes capability | Evidence | FlexCore today |
|---|---|---|---|
| 5 | Program/contract/enrollment lifecycle management | GE Program Manager [S-41]; Uplight turnkey program management [S-43]; CPower curtailment plans [S-48] | Nothing: no program entity, no enrollment workflow |
| 6 | Market or utility-program participation layer | Voltus nine wholesale markets [S-47]; Next Kraftwerke day-ahead/intraday/control reserve [S-49]; GE Markets Orchestration [S-41] | `MarketSignalReceived` event has no producer; market workspace shows gap panels; bidding excluded by report §0.3 scope and ADR-011 forbids a *generic* tariff adapter |
| 7 | M&V / baselining / settlement-grade metering | Uplight same-day M&V [S-43]; GridBeyond "metering, verification, and settlement needed to prove delivery" [S-51]; NJ BPU straw [S-60]; RMI metering framework [S-59] | "revenue grade"/"measurement and verification": zero hits; settlement quality rules are EMPTY placeholders (P9.11) |

The commercial research adds two differentiators this phase owns: payment
lifecycle transparency to asset owners ("forecast to earned to paid", Voltus
[S-47]) and M&V as a productized module (almost nobody markets it — an honest
differentiation opportunity). The standards obligations (gap analysis §4) say
the same thing as regulation: FERC 2222 participation models [S-26][S-27],
ENTSO-E balancing (FCR/aFRR/mFRR; PICASSO/MARI) [S-28], M&V (IPMVP options
A–D [S-32]; CBL 10-in-10 + ≤20% SDA [S-30]; ANSI C12.19/C12.20 revenue-grade
data [S-31][S-33]), and the UK LFM (Piclo tenders, availability + utilization,
P483 dual participation) [S-36][S-37].

Recorded limitations this phase closes (or honestly narrows):

| LR | Meaning | Phase 16 disposition |
|---|---|---|
| LR-074 / LR-190 | No market-records store; no adapter / records store / commitments route — the §16.3 market-records class is unimplemented | Closed at DESIGN level (P16.01): schema specified, store written only at implementation |
| LR-073 | Baseline method chosen by FlexCore instead of the program — the program-baseline decision is OPEN | Baseline engine is program-configurable data (P16.04); FlexCore never picks the method |
| LR-075 | PJM capacity-market date conflict between sources — unresolved verification task | Resolved by record with sourced dates (P16.16), re-verified at use |

What the market research (gap analysis §6) says a platform must build, and
what each item is anchored to:

| Build item | Market evidence |
|---|---|
| Registration data management (per-DER + per-aggregation, EDC 45-day reviews, PUCT+ERCOT dual registration, CAMS/eMarket asset lifecycle) | PJM [S-90]; ERCOT [S-100]; ISO-NE [S-103] |
| Telemetry cadences | ERCOT 2-second ICCP [S-100][S-101]; NYISO 6-second SCADA + SOC [S-105][S-106]; PJM ≥10 MW aggregations [S-90]; CAISO 5-minute settlement [S-97] |
| Baselines | CBL 10-in-10 + capped symmetric SDA ≤20% [S-30]; DLA [S-96] |
| Settlement formats | PJM Manual 29 CSV/XML + MSRS line items [S-93]; CAISO 5-minute SQMD [S-97]; EBGL Art. 52–55 15-minute ISP imbalance [S-107][S-108][S-109] |
| Performance evidence | PJM precision scoring post-Oct-2025 consolidated score [S-94][S-95]; German FCR 30 s / aFRR 5 min full-response evidence [S-119]; SOC traces |
| Timelines (recorded as data, re-verified at use) | CAISO live Nov 2024; ISO-NE Nov 2026; NYISO end-2026; PJM Feb 2027/28; MISO 2027/29; SPP 2030 [S-26]; ERCOT ADER pilot [S-100][S-103] |

The timeline row is data, not schedule: the Phase 9 capture template
(`ledger/phase-09-pilot-qualification/11_market_program_requirements.md` §7)
carries slightly different NYISO/PJM rows as checked 2026-11-07 and requires
re-verification against primary sources on the day of use. Phase 16 inherits
that rule — dates are stored with sources, never treated as stable facts.

## 2. What exists today (the real seams)

All paths below exist in the Phase 0–10 workspace, which is WRITTEN-UNRUN —
written modules with records, not verified behavior. Every one was verified
for this context.

| Seam | Path | What it honestly contains |
|---|---|---|
| Market gap panel | `console/src/workspaces/marketData.ts` | Four explicit DOES-NOT-EXIST statements: market adapter, program signals, commitments, delivery evidence — no fabricated signal, no invented adapter status |
| Synthetic objective | `crates/flexcore-application/src/scheduling_objective.rs` | `SYNTHETIC_ENERGY_COST_DEVIATION_V1`, provenance SYNTHETIC, explicitly not a market product (D1/D2 OPEN) |
| Baseline field, no method | `crates/flexcore-domain/src/flexible_load_model.rs` | `baseline_method: BaselineMethod` on the flexible-load model; the METHOD itself is the program's (LR-073) |
| Frozen event vocabulary | `crates/flexcore-events/src/events.rs` | Exactly 26 frozen events (report §15.3); `MarketSignalReceivedPayload` exists with NO producer |
| Authz action registry | `crates/flexcore-authz/src/action.rs` | Deny-by-default closed registry; `dispatch.authorize`/`dispatch.issue` deliberately absent and never granted |
| GET-only dispatch routes | `apps/control-plane/src/routes_phase4.rs` | Phase 4 dispatch-command and feasibility routes, GET-only; no market-commitments route exists anywhere in the routed API |
| Canonical units | `crates/flexcore-contracts/src/units.rs` | MW/MVAr/MVA/MWh plus kV/A/Hz/SOC conventions; NO currency or price unit exists — money is not a typed unit in the contracts crate |
| Phase 9 market records | `ledger/phase-09-pilot-qualification/11_market_program_requirements.md`, `22_market_operational_procedures.md` | P9.11: capture TEMPLATE, operator question set Q1–Q12, every answer cell EMPTY; P9.22: procedures M-1–M-5 all `DRAFT — INACTIVE`; the market-records store is stated DOES NOT EXIST |

Dependency inputs from the phase architecture (gap analysis §9): Phase 16
consumes P13 identity (tenant/permission surfaces), P14 exports (adapter
architecture, command-relay-only rule), and P15 outputs (forecast records via
P15.04 for price-signal ingestion). Cross-phase seams are defined once in the
earlier phase and consumed, never redefined.

## 3. Constraints (critical)

1. **ADR-011 boundary — restated and honored.** `docs/architecture/adr/ADR-011.md`
   forbids a generic "FERC Order 2222" mapping, universal market API, or
   generic tariff/Order-2222 adapter: market integrations are always
   region-specific adapter plugins and records, selected only after a
   jurisdiction decision (D1, still OPEN). Phase 16 positions itself as
   **jurisdiction-specific records — NOT a generic engine** — and says so in
   its plan. Any design that would work "for any market" by hard-coding a
   false universalism is out of bounds.
2. **Control ceiling and permissions.** Bidding and dispatch-relay require
   operator-approved mode plus explicit per-market grants (conventions §6.4);
   the never-granted set (`dispatch.authorize`, `dispatch.issue`,
   `secret.read`, `network.any`) stays byte-identical; market adapters reach
   dispatch only through the command lifecycle (P14 command-relay-only rule).
3. **The 26-event vocabulary is FROZEN.** `crates/flexcore-events` carries
   exactly 26 events; market events enter only as an additive v2 vocabulary
   decision (P16-E-2) following the versioning discipline — never by editing
   the frozen list.
4. **No live market connection in this program.** The deliverable is a
   paper-trading simulator plus records. No registration is filed, no market
   is contacted, no settlement statement is received, no qualification is
   claimed (conventions §7.3; ADR-011 consequences).
5. **Billing here = revenue-share statements only.** The forecast → earned →
   paid lifecycle per asset/program follows the Voltus pattern [S-47]; retail
   billing / CIS replacement is explicitly out of scope (gap analysis §8,
   recorded with revisit conditions).
6. **M&V numbers are settlement-adjacent financial records.** Baselines,
   event-performance figures, settlement statements, and payment states get
   the same append-only audit/chain treatment as commands (P11 interplay:
   chain consumed, not redefined; LR-203 production-crypto requirement
   inherited).

## 4. Glossary (market terms used in this phase)

| Term | Meaning |
|---|---|
| DER Aggregation Resource | ISO-NE's registered aggregation of DERs participating per Order 2222 [S-103] |
| Scheduling Coordinator (SC) | Entity submitting schedules/bids and settlement data on participants' behalf (CAISO model) [S-96][S-97] |
| QSE | Qualified Scheduling Entity — the ERCOT-registered entity representing resources [S-100][S-102] |
| RERRA | Relevant electric retail regulatory authority whose concurrence DER aggregation requires [S-90] |
| CBL 10-in-10 / 5-in-10 | Customer Baseline Load from the highest (or all) 10 (or 5) of the last valid non-event days [S-30] |
| SDA | Same-Day Adjustment — symmetric day-of correction, capped (CAISO ≤20%) [S-30] |
| DLA | Day-Of Load Adjustment — CAISO's alternative day-of baseline adjustment [S-96] |
| SQMD | Settlement Quality Meter Data — CAISO's settlement-grade meter data at 5-minute resolution [S-97][S-98] |
| ICCP | Inter-Control-Center Communications Protocol (IEC 60870-6/TASE.2) — ERCOT ADER's 2-second telemetry transport [S-100] |
| SCED | Security-Constrained Economic Dispatch — ERCOT's 5-minute base-point process ADERs follow [S-100][S-101] |
| DAM / RTM | Day-Ahead Market / Real-Time Market |
| RPM / FCA | PJM Reliability Pricing Model capacity auctions / ISO-NE Forward Capacity Auction [S-26][S-103] |
| FCR / aFRR / mFRR | Frequency Containment Reserve / automatic and manual Frequency Restoration Reserve — ENTSO-E balancing products [S-28] |
| BSP | Balancing Service Provider — the prequalified ENTSO-E balancing-market entity [S-28] |
| Imbalance settlement | Settlement of deviations between scheduled and metered energy per EBGL [S-107] |
| ISP | Imbalance Settlement Period — 15 minutes per EBGL Art. 52–55 (from 60) [S-107][S-108][S-109] |
| Availability vs utilization payments | UK LFM structure: paid for offered availability and separately for delivered utilization [S-36][S-114] |
| MSRS / BLI | PJM Market Settlement and Resolution System statements and their Billing Line Items [S-93] |
| M&V | Measurement and Verification — quantifying delivered performance against baseline [S-32][S-43] |
| IPMVP option | International Performance Measurement and Verification Protocol options A–D [S-32] |
| Revenue-grade metering | Metering meeting ANSI C12.20 accuracy classes (0.2/0.5 expectations) and ANSI C12.19 data tables [S-31][S-33] |
