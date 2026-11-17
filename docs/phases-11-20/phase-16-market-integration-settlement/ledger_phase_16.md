# Phase 16 — Market Integration, Settlement and Revenue — Ledger

Date: 2026-11-14. Status: **IMPLEMENTED — awaiting human build/test** — 24 of 24 subtasks ticked. Phase close: HANDOVER.md, CHECKLIST.md, records/79_phase_16_close_and_consistency_audit.md. No market connected, no settlement number; the human-gate queue is in CHECKLIST.md.

**Gate.** No implementation of this phase starts before (a) human review of
this roadmap and (b) the first human build of the Phase 0–10 workspace is
green (program conventions §3; program README, hard status boundary 1).
Every run, scan, sign-off, threshold, and date produced during
implementation is a HUMAN GATE INPUT recorded UNRUN / EMPTY / UNSIGNED.

**Tick convention (carried verbatim from Phases 0–10).** A checkbox is
ticked only as "implemented, awaiting human build/test" with a
decision-record citation — `- [x] ... (decision record:
\`records/NN_slug.md\`)` — never as "tested", "verified", "passed", or
"works". Tick flips use the exact-match count==1 method or Write/Edit;
decision records are created at implementation time under `records/`.

## Subtask register

- [x] **P16.01 — Market-records store design.** Schema for programs, (decision record: `records/01_p16_01_market_records_store.md`)
      contracts, enrollments, market events, deliveries, and settlements,
      closing the LR-074/190 gap at design level; retention rows and
      audit-chain linkage (every financial record inherits the P11 chain
      treatment). Evidence: the record cites the schema, retention rows, and
      chain linkage; no store exists and none is claimed running.
- [x] **P16.02 — Program/enrollment domain model.** Program, contract, (decision record: `records/02_p16_02_program_enrollment_domain_model.md`)
      enrollment, qualification-evidence, and per-asset participation-window
      types with lifecycle states (the GE Program Manager analog [S-41]).
      Evidence: the record cites the types and states; no workflow has run.
- [x] **P16.03 — Registration data management.** Per-ISO registration (decision record: `records/03_p16_03_registration_data_management.md`)
      records: PJM per-DER + aggregation with EDC 45-day review tracking
      [S-90]; ERCOT PUCT+ERCOT dual registration [S-100]; ISO-NE CAMS/
      eMarket asset lifecycle [S-103]; attestations as signed records.
      Evidence: the record cites the per-ISO record types; no registration
      is filed or exists.
- [x] **P16.04 — Baseline engine.** CBL 10-in-10 / 5-in-10 / day-matching (decision record: `records/04_p16_04_baseline_engine.md`)
      with weather/day-type filters, capped symmetric SDA (CAISO ≤20%
      [S-30]); program-configurable — FlexCore never chooses the method
      (LR-073); determinism + replay tests written-unrun. Evidence: the
      record cites the configurations and written-unrun tests; the engine
      has not executed.
- [x] **P16.05 — M&V event reports.** Event performance vs the program's (decision record: `records/05_p16_05_mv_event_reports.md`)
      baseline, the same-day feedback pattern [S-43], evidence assembly from
      interval data; every number is a settlement-adjacent financial record.
      Evidence: the record cites the report model; no number is claimed.
- [x] **P16.06 — Bid composition service.** Asset envelopes → (decision record: `records/06_p16_06_bid_composition_service.md`)
      aggregation-level bids per market structure (PJM single aggregation
      node [S-90]; CAISO PDR two-ID load-shift [S-96]; ISO-NE DAM Asset tab
      [S-103]); the envelope remains the physical truth layer;
      operator-approved mode + explicit per-market grant required. Evidence:
      the record cites the bid models and grant rule; no bid is submitted —
      none can be.
- [x] **P16.07 — Price-signal ingestion.** DA/RT price records; ENTSO-E (decision record: `records/21_p16_07_price_signal_ingestion.md`)
      Transparency Platform adapter consuming P15.04 [S-111]; DataMiner2-
      class ISO price-publication seams [S-94]; prices are inputs, never
      commands (ADR-010). Evidence: the record cites the record types and
      seams; no price feed is connected.
- [x] **P16.08 — Telemetry export pipelines (design).** ERCOT 2-second ICCP (decision record: `records/22_p16_08_telemetry_export_pipelines.md`)
      with net-consumer normalization + weighted ramp-rate aggregation
      [S-100][S-101]; NYISO 6-second SCADA + SOC [S-105][S-106]; PJM ≥10 MW
      telemetry posture [S-90]; adapter seams + cadence budgets;
      certification is a human gate. Evidence: the record cites the
      pipelines and budgets; no export pipeline transmits.
- [x] **P16.09 — Settlement ingestion.** PJM Manual 29 CSV/XML parser specs (decision record: `records/23_p16_09_settlement_ingestion.md`)
      + MSRS/BLI reconciliation workflow [S-93]; CAISO 5-minute SQMD
      [S-97]; EBGL Art. 52–55 15-minute ISP imbalance statements
      [S-107][S-108][S-109]; statement→expected reconciliation with dispute
      queue. Evidence: the record cites the parser specs and workflow; no
      real statement has been parsed — none exists here.
- [x] **P16.10 — Settlement-quality meter data handling.** ANSI C12.19/ (decision record: `records/24_p16_10_settlement_meter_data.md`)
      C12.20 alignment (class 0.2/0.5 expectations) [S-31][S-33];
      meter-device-resource mapping; dual streams (settlement-interval
      meter data + seconds-level telemetry) kept distinct. Evidence: the
      record cites the mapping and dual-stream model; no meter is certified.
- [x] **P16.11 — Performance evidence capture.** Signal-following (decision record: `records/25_p16_11_performance_evidence.md`)
      telemetry, response timestamps vs instruction, mileage/RMRTS tracking,
      SOC traces, precision-score inputs (post-Oct-2025 consolidated score;
      RegUp/RegDn post-Phase-2 as reported — date UNVERIFIED, marked)
      [S-94][S-95]; German FCR 30 s / aFRR 5 min evidence classes [S-119].
      Evidence: the record cites the evidence classes with the UNVERIFIED
      date marked; no score is claimed.
- [x] **P16.12 — Dispute workflow.** Line-item challenge tracking, (decision record: `records/26_p16_12_dispute_workflow.md`)
      evidence-bundle assembly (chain-linked), resolution records feeding
      from the P16.09 reconciliation queue. Evidence: the record cites the
      workflow states; no dispute exists.
- [x] **P16.13 — Revenue-share billing basics.** Contract-configurable (decision record: `records/41_p16_13_revenue_share_billing.md`)
      splits, pooled-revenue disaggregation per asset, owner statements,
      invoicing records, reconciliation back to ISO/DSO statements [S-118];
      NOT a retail CIS (gap analysis §8). Evidence: the record cites the
      statement model; no invoice, statement, or figure is issued.
- [x] **P16.14 — Payment lifecycle transparency.** Forecast → earned → paid (decision record: `records/42_p16_14_payment_lifecycle.md`)
      states per asset/program (the Voltus pattern [S-47]); statuses as a
      closed vocabulary with chain-linked transitions. Evidence: the record
      cites the closed state vocabulary; no payment state has been entered.
- [x] **P16.15 — Program event management.** OpenADR event lifecycle (decision record: `records/43_p16_15_program_event_management.md`)
      linkage to the command lifecycle (event → PROPOSED commands per
      asset), opt-in/out handling, report delivery for M&V [S-116][S-117].
      Evidence: the record cites the linkage and opt-in/out rules; an
      external event never passes PROPOSED without the three gates.
- [x] **P16.16 — Capacity-market participation records.** RPM (PJM Feb 2027 (decision record: `records/44_p16_16_capacity_market_records.md`)
      proposed) / FCA19 (Feb 2026) / DECR qualification-evidence checklists;
      the LR-075 date conflict resolved by record with sourced dates,
      re-verified at use [S-26][S-103]. Evidence: the record cites the
      checklists and sourced dates; no qualification is claimed or possible.
- [x] **P16.17 — UK/EU LFM integration design.** Piclo registration/ (decision record: `records/45_p16_17_uk_eu_lfm_records.md`)
      tender/utilization/settlement flow designs, availability + utilization
      payment records, P483 dual-participation stacking records
      [S-36][S-114][S-115]. Evidence: the record cites the flow designs; no
      tender is placed and no payment record exists.
- [x] **P16.18 — Market jurisdiction register.** Per-tenant market (decision record: `records/46_p16_18_market_jurisdiction_register.md`)
      enablement matrix (none enabled by default), regulatory-calendar data
      (FERC 2222 timelines [S-26]), ERCOT ADER pilot posture [S-100]; dates
      as sourced data with the re-verify rule. Evidence: the record cites
      the matrix with zero markets enabled; every date carries a source.
- [x] **P16.19 — Market adapter permission model.** Bidding/dispatch-relay (decision record: `records/61_p16_19_market_adapter_permission_model.md`)
      requires operator-approved mode + explicit per-market grants;
      never-granted review (the set stays byte-identical); command-relay-
      only rule consumed from P14. Evidence: the record cites the grant
      model; the never-granted set is quoted byte-identical.
- [x] **P16.20 — Market simulator (paper-trading).** Simulated DA/RT prices (decision record: `records/62_p16_20_market_simulator.md`)
      and synthetic settlement statements so the full loop is testable with
      zero market connectivity; feeds replay/benchmark. Evidence: the record
      cites the design; it produces records only, never a connection.
- [x] **P16.21 — Market integration test suite (written-unrun).** Baseline (decision record: `records/63_p16_21_market_integration_test_suite.md`)
      determinism, settlement-parser fixtures (PJM-M29-shaped, CAISO-shaped,
      EBGL-shaped SYNTHETIC), payment-lifecycle state machine, opt-in/out
      negative tests. Evidence: the record cites the test files; execution
      is a human gate and no test has run.
- [x] **P16.22 — Additive event-vocabulary v2 + authz actions registration.** (decision record: `records/64_p16_22_market_v2_vocabulary_and_market_actions.md`)
      market.* actions registered per the authz pattern; new market events
      versioned separately from the frozen 26 — recorded as P16-E-2.
      Evidence: the record cites the proposed v2 event and action lists;
      the frozen 26 stay byte-identical.
- [x] **P16.23 — Decision records.** P16-E-1 (first market target — DEFAULT: (decision record: `records/65_p16_23_decision_records_e1_e2_e3.md`)
      paper-trading + ONE jurisdiction record pack, CAISO or ISO-NE given
      live/newest timelines; live connections explicitly out of this
      program), P16-E-2 (event vocabulary v2), P16-E-3 (settlement engine
      build-vs-buy). Evidence: the records compile options, defaults, and
      closing evidence; agents close none.
- [x] **P16.24 — Close + consistency audit.** HANDOVER, CHECKLIST, close (decision record: `records/79_phase_16_close_and_consistency_audit.md`)
      record; cross-check pipe counts, [S-n] resolution, and the honesty
      statements (no market connected, no settlement number) across every
      phase-16 artifact. Evidence: the close record cites the audit results;
      nothing closes without human review.

## Decision points (all OPEN — human-owned)

| ID | Question | DEFAULT (label) | Closer |
|---|---|---|---|
| P16-E-1 | First market target / jurisdiction record pack | Paper-trading + ONE jurisdiction record pack (CAISO or ISO-NE given live/newest timelines); live connections out of this program | human owner |
| P16-E-2 | Market events + market.* authz actions vs the frozen surfaces | Additive v2 event vocabulary versioned separately from the frozen 26; registered market.* actions | human owner |
| P16-E-3 | Settlement engine build vs buy | Build-by-spec with synthetic fixtures; revisit buy at first live jurisdiction | human owner |
