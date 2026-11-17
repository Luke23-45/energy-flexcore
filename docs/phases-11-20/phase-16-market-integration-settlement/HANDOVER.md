# HANDOVER — Phase 16: Market Integration, Settlement and Revenue

Status: **implementation complete (expected 24/24), everything WRITTEN-UNRUN.**
The ledger's P16.01–P16.12 ticks are applied (by the orchestrator, citing
records 01–06 and 21–26); the P16.13–P16.24 ticks are the ORCHESTRATOR's
action, applied after this close (program conventions §2 — no agent ever
ticks a checkbox), expected to cite records 41–46 (wave 2, Agent A), 47 (A's
close-out), 61–65 (wave 2, Agent B), 66 (B's close-out), and the close record
79. The phase gate is **NOT MET** as a runtime claim: no market is connected,
no registration is filed, no price feed is connected, no statement is
received, no bid is submitted (none can be), no instruction is issued, no
payment moves, no settlement number exists, no test has run, no migration is
applied, no export transmits — the first human build of the Phase 0–10
workspace is the verification gate for every statement below.

## What was implemented (all WRITTEN, never run)

| Area | Subtasks | Records | Delivered (written) |
|---|---|---|---|
| Market records + domain (wave 1, Agent A) | P16.01–P16.06 | 01, 02, 03, 04, 05, 06 | migration 0022 (seven tables, expand-only, append-only grants + RLS), the `MarketRecordsStore` contract + postgres adapter (decimal-text money, supersede model, audit-chain linkage); the program/contract/enrollment domain model with closed lifecycle tables + stacking-rule refusal; the per-ISO registration record vocabulary (PJM per-DER + EDC 45-day journal, ERCOT dual legs, ISO-NE CAMS/eMarket, signed attestations); the program-configurable baseline engine (CBL 10-in-10 / 5-in-10 / day-matching, capped symmetric SDA ≤20% [S-30], LR-073 gap rows); the M&V event-report assembler (same-day feedback class, §10.4 quality states); the bid composition service (three market structures, envelope-truth provenance, operator-approved mode + `MarketBidGrant`) |
| Ingestion + evidence (wave 1, Agent B) | P16.07–P16.12 | 21, 22, 23, 24, 25, 26 | the exact fixed-decimal money type + DA/RT price record layer over the P15.04 seam (two-copies discipline) with the UNWIRED `IsoPricePublication` seam; the per-market telemetry export DESIGNS (ERCOT 2 s ICCP, NYISO 6 s SCADA+SOC, PJM ≥10 MW) with the UNWIRED `TelemetryExportSink`; the settlement parser SPECIFICATIONS (PJM M29 CSV/XML, CAISO 5-minute SQMD, EBGL 15-minute ISP — declared v1 fixture grammars) + the statement→expected reconciliation engine with the dispute queue; the settlement-quality meter-data dual-stream model with the fail-closed boundary gate; the performance-evidence record classes (no score claimed; UNVERIFIED dates marked); the dispute workflow (frozen transitions, chain-linked bundles, resolution records) |
| Billing / lifecycle / events / records / register (wave 2, Agent A) | P16.13–P16.18 | 41, 42, 43, 44, 45, 46, 47 | revenue-share billing basics (exact integer-mantissa disaggregation, shares sum to exactly 100%, DRAFT/RECONCILED statements); the payment lifecycle (closed FORECASTED/EARNED/PAID, two frozen edges, exact chain linkage, one currency per chain); program event management (external events enter at PROPOSED only, fail-closed opt-in/out); capacity-market records (LR-075 resolved BY RECORD, both dates carried; no QUALIFIED state); UK/EU LFM flow records (declared stages only; P483 one-variant stacking rule); the market jurisdiction register AS DATA (ten rows, ZERO enablements, ENABLED_BY_D1 the only opt-in with decision ref + audit link, date conflicts carried on both sides) |
| Permissions / simulator / tests / vocabulary / decisions / close (wave 2, Agent B) | P16.19–P16.24 | 61, 62, 63, 64, 65, 66, 79 | six `market.*` authz actions registered additively (records/grant-administration only; NO `market.dispatch.*` exists; the never-granted set byte-identical, review verdict recorded); the paper-trading market simulator (simulated feed ON the unwired price seam, synthetic statements ON the P16.09 grammars, in-memory export sink ON the unwired export seam, the deterministic paper loop signal→bid→dispatch→evidence→statement→reconciliation→payment-state-reference — RECORDS ONLY); the 32-test written-unrun market integration suite (baseline determinism, synthetic parser fixtures, dispute negatives, payment-lifecycle state machine over Agent A's landed module, enablement opt-in/out, paper-loop integration); the ADDITIVE market event vocabulary v2 (8 proposed events, own tag + subject scheme; the frozen 26 byte-identical, pinned by test); the consolidated E-decision register (P16-E-1/E-2/E-3 verified OPEN with DEFAULTs, human closers); this close set |

## Blockers carried (human gate inputs)

1. **The first human build** (`cargo build --workspace` → clippy →
   `cargo test --workspace`, plus the migration-0022 apply/down cycle):
   every Rust artifact in this phase is written against the WRITTEN-UNRUN
   0–10 base. Failures at that gate are recorded and fixed as code edits —
   never silently.
2. **No market is connected and none can be within this program**: the
   deliverable is jurisdiction-specific records, adapters-by-design, and
   the paper-trading simulator (ADR-011; conventions §7.3). Any wording
   implying connectivity, registration, qualification, or settlement
   participation is a compliance failure (risk R-P16-4).
3. **The three E-decisions are OPEN** (P16-E-1, P16-E-2, P16-E-3 — record
   65 compiles the register; every closer is the human owner). P16-E-1's
   DEFAULT is implemented exactly as far as the owner's naming allows:
   the paper-trading half; no jurisdiction record pack exists.
4. **No settlement number, revenue figure, invoice, or payment exists
   anywhere** (plan §8 criterion 4): every numeric fixture is
   integer-derived decimal text with no market meaning; the payment
   lifecycle entered NO state; the register carries ZERO enablements.
5. **Every test is WRITTEN-UNRUN** (wave 1: the baseline/bid suites; the
   inline suites across all wave-1/2 modules; wave 2: the 32-test
   market integration suite): execution is the human gate.
6. **Dates are recorded DATA with the re-verify-at-use rule** (risk
   R-P16-1): the CAISO/ISO-NE/NYISO/PJM/ERCOT timelines and the 45-day
   EDC window are sourced, never stable facts; the LR-075 conflict is
   resolved BY RECORD with both dates carried.
7. **The frozen surfaces are byte-identical** (pinned by tests): the
   26-event vocabulary (`events.rs` untouched; the v2 vocabulary is a NEW
   file), the never-granted permission set (quoted verbatim in records 61
   §2), the 14-state/27-edge command set, and the P11 audit chain
   (consumed, linked per record, never redefined).

## Expected orchestrator citations (ledger ticks, applied AFTER this close)

| Ledger row | Expected citation (decision record) |
|---|---|
| P16.13 | `records/41_p16_13_revenue_share_billing.md` |
| P16.14 | `records/42_p16_14_payment_lifecycle.md` |
| P16.15 | `records/43_p16_15_program_event_management.md` |
| P16.16 | `records/44_p16_16_capacity_market_records.md` |
| P16.17 | `records/45_p16_17_uk_eu_lfm_records.md` |
| P16.18 | `records/46_p16_18_market_jurisdiction_register.md` |
| P16.19 | `records/61_p16_19_market_adapter_permission_model.md` |
| P16.20 | `records/62_p16_20_market_simulator.md` |
| P16.21 | `records/63_p16_21_market_integration_test_suite.md` |
| P16.22 | `records/64_p16_22_market_v2_vocabulary_and_market_actions.md` |
| P16.23 | `records/65_p16_23_decision_records_e1_e2_e3.md` |
| P16.24 | `records/79_phase_16_close_and_consistency_audit.md` (this close) |

(The wave-2 record numbering landed as 41–47 for Agent A — the plan's
anticipated "records 41–59" band compressed to one record per subtask plus
the close-out, the same shape wave 1 used.)
