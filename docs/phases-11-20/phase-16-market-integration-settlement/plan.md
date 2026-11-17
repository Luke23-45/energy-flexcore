# Phase 16 — Market Integration, Settlement and Revenue — Plan

Date: 2026-11-14. Status: **NOT STARTED** (0/24). Planning baseline only.
Ledger: `ledger_phase_16.md` (all boxes unticked). Conventions:
`../01_PROGRAM_CONVENTIONS.md` (binding). Context: `context.md` (seams and
constraints).

## 1. Scope statement

Phase 16 closes the three commercial table-stakes rows (gap analysis §3 #5,
#6, #7) and the market/standards obligations (gap analysis §4, §6): the
market-records store design that closes the LR-074/190 gap at design level,
the program/enrollment domain model, per-ISO registration records [S-90]
[S-100][S-103], a program-configurable baseline engine (CBL 10-in-10 / 5-in-10
/ day-matching, capped symmetric SDA ≤20% [S-30]) that narrows LR-073 by
keeping the method the program's, M&V event reports, bid composition from
asset envelopes, price-signal ingestion (ENTSO-E TP consuming P15.04,
DataMiner2-class ISO seams [S-94]), per-market telemetry export pipeline
designs (ERCOT 2-second ICCP [S-100][S-101], NYISO 6-second [S-105][S-106],
PJM ≥10 MW posture [S-90]), settlement ingestion by spec (PJM Manual 29
CSV/XML + MSRS/BLI [S-93], CAISO 5-minute SQMD [S-97], EBGL 15-minute ISP
[S-107][S-108][S-109]), settlement-quality meter handling (ANSI C12.19/C12.20
[S-31][S-33]), performance-evidence capture [S-94][S-95][S-119], disputes,
revenue-share billing and the payment lifecycle [S-47][S-118], OpenADR event
management [S-116][S-117], capacity-market records [S-26][S-103], UK/EU LFM
design [S-36][S-114][S-115], the jurisdiction register [S-100], the market
permission model, and a paper-trading market simulator.

**ADR-011 restated for this phase (binding):** every deliverable above is
jurisdiction-specific RECORDS, adapters-by-design, and a paper-trading
simulator. Phase 16 does NOT build a generic tariff engine, a generic "Order
2222 adapter", or a universal market API (`docs/architecture/adr/ADR-011.md`).
Market structure knowledge (a two-ID CAISO load-shift, a single PJM
aggregation node, a 15-minute ISP) lives in per-jurisdiction record packs and
configurable data, never in a claimed-universal core.

Three deliverable depths exist, stated up front so no record overclaims.
**Design-and-model depth** (most subtasks): schemas, domain types, parser
specifications, pipeline designs, and written-unrun test suites — the same
WRITTEN-UNRUN class as the Phase 0–10 base. **Record-only depth** (P16.16,
P16.18, P16.19, P16.22, P16.23, P16.24): decision registers, checklists, and
records where the honest deliverable is the analysis, because implementation
without a named jurisdiction (D1, OPEN) and without a market counterparty
would produce market code with nothing to talk to. **Simulator depth**
(P16.20): executable-by-design simulator logic against the frozen seams, run
only under the standing loop bans (nothing in this program has ever been
built or run).

The expected artifact inventory, so the acceptance criteria in §8 are
checkable: schema and migration-design records (P16.01); domain model types
and lifecycle states (P16.02, P16.06, P16.14); per-jurisdiction record-pack
specifications (P16.03, P16.08, P16.16, P16.17); engine and parser
specifications with synthetic fixtures (P16.04, P16.09, P16.21); workflow
records (P16.05, P16.12, P16.15); a register as data (P16.18); permission
and decision records (P16.19, P16.22, P16.23); and the phase close set
(P16.24). Implementation-time `records/` files are created per batch under
the phase directory, never during planning (conventions §8).

## 2. Subtask outline

| ID | Title | Summary | Key seams / sources |
|---|---|---|---|
| P16.01 | Market-records store design | Schema for programs, contracts, enrollments, market events, deliveries, settlements (closes the LR-074/190 gap at design level); retention rows and audit-chain linkage (every financial record inherits the P11 chain treatment) | `ledger/phase-09-pilot-qualification/22_market_operational_procedures.md` §5 (store DOES NOT EXIST); migration pattern `crates/adapters/postgres/migrations/`; `context.md` §3.6 |
| P16.02 | Program/enrollment domain model | Program, contract, enrollment, qualification-evidence, and per-asset participation-window types with lifecycle states (the GE Program Manager analog [S-41]) | [S-41][S-43][S-48]; `crates/flexcore-domain/src/` model patterns |
| P16.03 | Registration data management | Per-ISO registration records: PJM per-DER + aggregation registration with EDC 45-day review tracking [S-90]; ERCOT PUCT+ERCOT dual registration [S-100]; ISO-NE CAMS/eMarket asset lifecycle [S-103]; attestations modeled as signed records | [S-90][S-100][S-102][S-103]; P16.02 types |
| P16.04 | Baseline engine | CBL 10-in-10 / 5-in-10 / day-matching with weather/day-type filters and capped symmetric SDA (CAISO ≤20% [S-30]); program-configurable — FlexCore never chooses the method (LR-073); determinism (injected Clock, BTreeMap ordering) + replay tests written-unrun | [S-30][S-96]; `crates/flexcore-domain/src/flexible_load_model.rs` (`baseline_method` field); LR-073 |
| P16.05 | M&V event reports | Event performance vs the program's baseline, the same-day feedback pattern [S-43], evidence assembly from interval data with §10.4 quality states; every number is a financial record (constraint 6) | [S-32][S-43][S-59]; P16.04 baselines; telemetry quality vocabulary |
| P16.06 | Bid composition service | Asset envelopes → aggregation-level bids per market structure: PJM single aggregation node [S-90]; CAISO PDR two-ID load-shift bidding [S-96]; ISO-NE DAM Asset tab lifecycle [S-103]; the envelope remains the physical truth layer; operator-approved mode + explicit per-market grant required before any bid is composed | [S-90][S-96][S-103][S-120]; `crates/flexcore-domain/src/` envelopes; `context.md` §3.2 |
| P16.07 | Price-signal ingestion | Day-ahead/real-time price records; ENTSO-E Transparency Platform adapter (consumes P15.04's output, never redefines it) [S-111]; ISO price-publication seams (DataMiner2-class [S-94]); prices are inputs, never commands (ADR-010) | [S-94][S-111][S-112]; P15.04 seam; `crates/flexcore-events/src/subject.rs` |
| P16.08 | Telemetry export pipelines (design) | Per-market pipeline designs: ERCOT 2-second ICCP with net-consumer normalization and weighted ramp-rate aggregation [S-100][S-101]; NYISO 6-second SCADA + SOC [S-105][S-106]; PJM ≥10 MW telemetry posture [S-90]; adapter seams + cadence budgets; certification is a human-gated lab process | [S-90][S-100][S-101][S-104][S-105][S-106]; P14 adapter architecture; conventions §7.3 |
| P16.09 | Settlement ingestion | PJM Manual 29 CSV/XML parser specifications + MSRS/BLI reconciliation workflow [S-93]; CAISO 5-minute SQMD settlement handling [S-97]; EBGL Art. 52–55 15-minute ISP imbalance statements [S-107][S-108][S-109]; statement→expected reconciliation with dispute queue (P16.12) | [S-93][S-97][S-107][S-108][S-109]; P16.01 settlement tables |
| P16.10 | Settlement-quality meter data handling | ANSI C12.19/C12.20 alignment (class 0.2/0.5 expectations) [S-31][S-33]; meter-device-resource mapping; dual streams — settlement-interval meter data and seconds-level telemetry kept distinct, never silently merged | [S-31][S-33][S-59][S-104]; `crates/flexcore-contracts/src/` telemetry types |
| P16.11 | Performance evidence capture | Signal-following telemetry, response timestamps vs instruction, mileage/RMRTS tracking, SOC traces, precision-score inputs (post-Oct-2025 consolidated score as reported; RegUp/RegDn post-Phase-2 — date UNVERIFIED, marked) [S-94][S-95]; German FCR 30 s / aFRR 5 min evidence classes [S-119] | [S-94][S-95][S-119][S-120]; P16.05 reports; gap analysis §11 UNVERIFIED list |
| P16.12 | Dispute workflow | Line-item challenge tracking, evidence-bundle assembly (chain-linked), resolution records; disputes reference the reconciliation queue from P16.09 | P16.09 queue; P16.01 audit-chain linkage; P11 chain |
| P16.13 | Revenue-share billing basics | Contract-configurable splits, pooled-revenue disaggregation per asset, owner statements, invoicing records, reconciliation back to ISO/DSO statements [S-118]; NOT a retail CIS (gap analysis §8) | [S-118][S-47]; P16.01 tables; P16.02 contracts |
| P16.14 | Payment lifecycle transparency | Forecast → earned → paid states per asset/program (the Voltus pattern [S-47]); statuses as a closed vocabulary; every state transition chain-linked | [S-47][S-118]; P16.13; closed-vocabulary kernel pattern |
| P16.15 | Program event management | OpenADR event lifecycle linkage to the command lifecycle — an external event becomes PROPOSED commands per asset, never an issued command; opt-in/out handling; report delivery for M&V [S-116][S-117] | [S-116][S-117]; `crates/flexcore-domain/src/command_state.rs`; P14.05 OpenADR mapping |
| P16.16 | Capacity-market participation records | RPM (PJM Feb 2027 proposed) / FCA19 (Feb 2026) / DECR qualification-evidence checklists; the LR-075 PJM date conflict resolved by record with sourced dates, re-verified at use [S-26][S-103] | [S-26][S-103]; `ledger/phase-09-pilot-qualification/11_market_program_requirements.md` §7; LR-075 |
| P16.17 | UK/EU LFM integration design | Piclo registration/tender/utilization/settlement flow designs, availability + utilization payment records, P483 dual-participation stacking records [S-36][S-114][S-115] | [S-36][S-37][S-114][S-115]; P16.02/P16.13 models |
| P16.18 | Market jurisdiction register | Per-tenant market enablement matrix (none enabled by default); regulatory-calendar data (FERC 2222 timelines [S-26]); ERCOT ADER pilot posture [S-100]; dates stored as sourced data with the re-verify rule | [S-26][S-27][S-100][S-103]; P13 tenant model; gap analysis §4 timeline row |
| P16.19 | Market adapter permission model | Bidding/dispatch-relay requires operator-approved mode + explicit per-market grants; never-granted review (the set stays byte-identical); command-relay-only rule consumed from P14 | [S-90][S-100]; `crates/flexcore-authz/src/action.rs`; conventions §6.4; P14.16 |
| P16.20 | Market simulator (paper-trading) | Simulated DA/RT prices and synthetic settlement statements so the full loop — signal → bid → dispatch → evidence → statement → payment state — is testable with zero market connectivity; feeds replay/benchmark | `crates/flexcore-simulator/`; P16.07/P16.09/P16.14 outputs; `context.md` §3.4 |
| P16.21 | Market integration test suite (written-unrun) | Baseline-engine determinism, settlement-parser fixtures (PJM-M29-shaped, CAISO-shaped, EBGL-shaped SYNTHETIC — no real statement exists here), payment-lifecycle state machine, opt-in/out negative tests; execution is a human gate | [S-30][S-93][S-97][S-107]; P16.04/P16.09/P16.14; conventions §1.3 |
| P16.22 | Additive event-vocabulary v2 + authz actions registration | market.* actions registered per the authz pattern; new market events versioned separately from the frozen 26 — recorded as P16-E-2 | `crates/flexcore-events/src/events.rs` (frozen 26); `crates/flexcore-authz/src/action.rs`; conventions §4 |
| P16.23 | Decision records | P16-E-1 (first market target — DEFAULT: paper-trading + ONE jurisdiction record pack, CAISO or ISO-NE given live/newest timelines; live connections explicitly out of this program); P16-E-2 (event vocabulary v2); P16-E-3 (settlement engine build-vs-buy); agents record options and defaults, close none | decision records; conventions §4; `context.md` §3 |
| P16.24 | Close + consistency audit | HANDOVER, CHECKLIST, close record; cross-check pipe counts, [S-n] resolution, honesty statements (no market connected, no settlement number) across every phase-16 artifact | conventions §2.5; this plan §8 |

## 3. Blocking order and shared surfaces

| Producer (blocks) | Consumers (what they take, never redefine) |
|---|---|
| P16.01 store schema + chain linkage | P16.05 evidence tables, P16.09 settlement tables, P16.12 dispute records, P16.13/16.14 billing and payment rows |
| P16.02 program/enrollment types | P16.03 registration records, P16.06 bid scoping, P16.13 contract splits, P16.15 opt-in/out, P16.17 LFM flows |
| P16.04 baseline configurations | P16.05 event reports, P16.21 determinism tests |
| P16.06 bid models + grant rule | P16.19 permission model, P16.20 simulator bid leg |
| P16.07 price records | P16.11 signal-following evidence, P16.20 simulated prices |
| P16.09 reconciliation queue | P16.12 dispute queue, P16.13 statement reconciliation, P16.21 parser fixtures |
| P16.14 payment states | P16.21 state-machine tests, P16.24 audit |
| P16.22 vocabulary v2 + market.* actions | P16.23 (the P16-E-2 record compiles it) |

Two shared surfaces are consumed, never redefined: the P11 audit chain (every
financial record links to it) and the P15.04 forecast seam (price ingestion
reads it). The proposed market-records migrations continue the existing
numbering additively; new crates named in designs remain proposals until an
implementation batch adds real code and the additive
`tools/check_dependency_rule.py` update (conventions §6.2).

## 4. Deliberate non-goals (this phase does not)

- No retail billing or CIS replacement — revenue-share statements only
  (gap analysis §8, recorded with revisit conditions).
- No live market connection, registration filing, operator contact, or
  qualification claim — the paper-trading simulator and records are the
  deliverable (context constraint 4; conventions §7.3).
- No generic tariff engine or universal "Order 2222 adapter" — jurisdiction
  specificity is the whole point (ADR-011; §1 restatement).
- No autonomous or unsupervised bidding — operator-approved mode plus
  explicit per-market grants; the never-granted set stays byte-identical.
- No new solver classes — bid composition consumes envelope data and existing
  LP/QP results; no MISOCP/MINLP creep (conventions §6.5).
- No money arithmetic hidden in floats — settlement amounts are explicitly
  labeled decimal records carrying a currency code as data (the contracts
  crate has no currency unit today; a typed money unit is proposed only if
  the first jurisdiction record pack requires one).

## 5. Wave plan

Two agents, per the standing loop discipline.

| Wave | Agent A | Agent B | Shared note |
|---|---|---|---|
| 1 | P16.01–P16.06 (records store, domain model, registration, baselines, M&V, bids) | P16.07–P16.12 (prices, telemetry export, settlement, meter data, evidence, disputes) | Disjoint seams: market-records/domain vs ingestion/evidence; P16.01's settlement tables are the one interface, defined in wave 1 by Agent A and consumed by Agent B's reconciliation design |
| 2 | P16.13–P16.18 (revenue share, payment lifecycle, event management, capacity, LFM, jurisdiction register) | P16.19–P16.24 (permissions, simulator, tests, vocabulary/authz decision, decision records, close) | P16.19 grants consume the P16.06 bid model and P14's relay rule; P16.22's action/event lists are compiled by Agent B and reviewed in P16.23 |

Dependency rules inherited (gap analysis §9): Phase 16 implementation starts
only after P11–P15 records exist; trace_id, audit chain, permission sets, and
P15.04 are consumed, never redefined. Each implemented batch gets a decision
record under `records/` and keeps the dependency-rule checker green
(conventions §2, §6.1).

### 5.1 Batch discipline during implementation

- Every implemented batch closes with a decision record under `records/`
  citing the subtask IDs, the seams touched, and the honest status
  ("implemented, awaiting human build/test") — conventions §2.4.
- Thresholds or budgets named in designs (cadence budgets, retention
  periods, split rules) are registered BEFORE any run that could fill
  them, and observed values stay structurally separate (X-8 carryover;
  conventions §5).
- Phase close follows conventions §2.5: HANDOVER, CHECKLIST, and the
  close/exit-gate record are added under this directory only at
  implementation completion (P16.24) — never during planning.
- New crates named in designs (a market-records adapter, a settlement
  worker) become workspace members only in the batch that adds their real
  code, with the additive `tools/check_dependency_rule.py` update that
  requires (conventions §6.2); the dependency rule stays green after every
  batch.

## 6. Decision points (E-decisions — agents never close them)

| ID | Question | Options | DEFAULT / RECOMMENDATION | What would close it | Closer |
|---|---|---|---|---|---|
| P16-E-1 | First market target: which jurisdiction record pack, if any, does the platform prepare first? | (a) CAISO record pack (only RTO/ISO fully live, Nov 2024 [S-97]); (b) ISO-NE (effective Nov 2026 [S-103]); (c) PJM (Feb 2027/28 [S-26][S-90]); (d) ERCOT ADER-pilot posture [S-100]; (e) none — paper-trading only | **DEFAULT: paper-trading + ONE jurisdiction record pack — CAISO or ISO-NE given the live/newest timelines**; live market connections are explicitly out of this program regardless | The owner naming the jurisdiction (the D1-class decision) after reviewing the P16.18 register and the sourced timelines | human owner |
| P16-E-2 | How do market events and market.* authz actions enter the frozen surfaces? | (a) additive v2 event vocabulary versioned separately from the frozen 26 + registered market.* actions; (b) extend the 26 in place (rejected — vocabulary is frozen); (c) out-of-band side channel (rejected — breaks event/idempotency discipline) | **DEFAULT: additive v2 vocabulary + registered market.* actions** (P16.22) | The E-decision record with the proposed event list and action list reviewed against the versioning discipline | human owner |
| P16-E-3 | Settlement engine: build or buy? | (a) build parsers/reconciliation by spec with synthetic fixtures; (b) license a settlement vendor engine; (c) defer until E-1 closes | **DEFAULT: build-by-spec with synthetic fixtures now; revisit buy when a live jurisdiction exists** | A budget/vendor decision (program-level, owner-owned per conventions §4) | human owner |

## 7. Risks

| # | Risk | Basis / mitigation |
|---|---|---|
| R-P16-1 | Market rules and timelines change mid-program (the gap-analysis and Phase 9 rows already differ on NYISO/PJM dates) | Timelines are recorded as DATA with sources [S-26][S-100][S-103] and a re-verify-at-use rule; no date is baked into logic |
| R-P16-2 | Settlement formats are deep — PJM Manual 29, CAISO, and EBGL statements encode tariff detail no parser spec fully captures | Parsers are specified from public manuals [S-93][S-97][S-107] and validated only against SYNTHETIC fixtures (P16.21); no format-conformance claim is made; statement→expected reconciliation flags every mismatch |
| R-P16-3 | Financial records demand the P11 audit chain — baselines, M&V numbers, statements, and payment states are settlement-adjacent financial data | The same append-only/audit-chain treatment as commands (context constraint 6); P11's chain and production-crypto requirement are consumed, not redefined |
| R-P16-4 | No live market claims, ever — any wording implying connectivity, registration, qualification, or settlement participation is a compliance failure | ADR-011 + §0.3 non-promises + conventions §7.3; paper-trading simulator only (context constraint 4); every phase-16 record carries the not-connected statement |
| R-P16-5 | The 0–10 base is WRITTEN-UNRUN — first build may surface recorded defects (e.g., F-P10.07-5) that shift every seam cited here | Program-level standing risk (program README boundary 1); planning proceeds, implementation cannot precede a green human build |
| R-P16-6 | Baseline-method drift — a jurisdiction's program rules differ from the modeled CBL/SDA families, tempting a FlexCore-chosen method | LR-073: the engine is configuration, the method is the program's; an unsupported method is a recorded gap row, never a silent approximation |
| R-P16-7 | Multi-market enrollment and dual-participation stacking (P483 [S-115]) create double-counting hazards across programs | Stacking rules are recorded per jurisdiction; the enrollment model refuses a combination that violates a recorded stacking rule |

## 8. Acceptance criteria (honest)

1. Schemas, domain models, parser specifications, pipeline designs, and test
   files exist **written-unrun**; no subtask claims a run, a test pass, or a
   verified behavior.
2. The market simulator produces **simulated prices and synthetic settlement
   statements only** — records, never connectivity.
3. **No market is connected, no registration is filed, no qualification or
   conformance is claimed** (ADR-011; conventions §7.3).
4. **No settlement number, revenue figure, invoice, or payment is claimed
   anywhere** — the payment lifecycle enters states only in the simulator
   under the loop bans.
5. The ADR-011 boundary is **restated and honored**: jurisdiction-specific
   records and adapters-by-design; no generic tariff/Order-2222 engine.
6. The frozen 26-event vocabulary stays byte-identical; the never-granted
   permission set stays byte-identical; market events/actions exist only as
   the P16-E-2 proposal.
7. `python tools/verify_docs.py` and `python tools/validate_agent.py` are
   green after every documentation change in this phase (program conventions;
   results quoted, never asserted).
