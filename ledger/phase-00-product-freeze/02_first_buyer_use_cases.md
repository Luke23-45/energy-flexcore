# P0.02 — First buyer and first use cases

| Field | Value |
|---|---|
| Subtask | P0.02 — Freeze the first buyer and first use cases |
| Report trace | §0, §1.3, §1.4, §1.5, §2.1, §2.2, §11.4, §23 (Stages 2–5), §25.2 (D1, D2) |
| Source | `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` |
| Status | Draft for Stage 0 review — first-buyer candidate is a RECOMMENDATION, not a decision |
| Decisions | D1 (target jurisdiction and market) and D2 (first portfolio and service) remain OPEN (§25.2). This document decides neither; it records a recommended candidate and the questions D1/D2 must answer. |

---

## 1. First buyer — RECOMMENDATION pending D2, not a decision

**Recommended candidate: VPP / DER aggregator operator** (§1.3 primary-audience table, row 1: "Need to understand fleet capacity, available flexibility, operating limits, and dispatch performance across many sites").

This is a **recommendation only, pending decision D2 ("First portfolio and service", §25.2 — determines asset mix, baseline method, and acceptance criteria; Phase 0; owner: product)**. It is not decided, approved, or committed. Rationale for the recommendation:

- §0 names organizations that aggregate or operate portfolios of DERs as the target for the first commercially credible product, with VPP and DER aggregators listed first.
- §1.4's initial operating scenario is written from the aggregator's position: a VPP aggregator coordinating a portfolio of several simulated or connected sites.
- The v0.1.0 capability set (§2.1: portfolio views, telemetry, registry, replay) directly serves the aggregator's fleet-visibility need in the §1.3 table.

Constraint while D1 is open: the buyer choice must not imply a market adapter, a jurisdiction, or a market programme. §25.2 defers target jurisdiction and market to D1 (Phase 0; product + legal); §2.3 states direct ISO/RTO market participation is a region-specific integration, not a universal core function. If product instead selects another §1.3 audience at the D2 gate, the disposition matrix in §2 below is re-checked against that selection.

## 2. First-generation use cases — disposition matrix (§1.5, all eight marked)

Per report §2.1, v0.1.0 delivers canonical asset/site models, simulator integration, telemetry ingestion, asset registry, portfolio views, and scenario replay — simulation only. **Portfolio visibility is therefore the first-generation use case v0.1.0 must demonstrably serve** (mandatory for v0.1.0). None of the other seven first-generation use cases is dropped; each is staged to a later §23 stage / §2.1 release. No first-generation use case is out of scope for the 0.x line as a whole; "out of scope" would apply only to work the report excludes, and none of the eight falls in that category.

| # | §1.5 first-generation use case | Disposition | Where it lands |
|---|---|---|---|
| 1 | Portfolio visibility: asset status, available capacity, state of charge, availability, alarms, telemetry quality, and site connectivity | **In scope for v0.1.0** (mandatory) | §2.1 v0.1.0 row: telemetry ingestion, asset registry, portfolio views, scenario replay; §23 Stage 2 exit gate: synthetic portfolio telemetry ingested, normalized, queried, and replayed deterministically |
| 2 | Flexibility estimation: what each asset and portfolio can safely offer over a defined interval and horizon | Staged to a later stage | §23 Stage 3 (flexibility envelope calculation); §2.1 v0.2.0 (flexibility envelopes) |
| 3 | Day-ahead and intraday scheduling | Staged to a later stage | §23 Stage 3 (LP/MILP/QP scheduler); §2.1 v0.2.0 (deterministic rolling-horizon scheduling) |
| 4 | Peak management: storage and flexible loads against a site or feeder import/export limit | Staged to a later stage | Builds on §23 Stage 3 scheduling and Stage 4 network feasibility; within §2.1 v0.2.0–v0.3.0 capability growth |
| 5 | Demand response: event/program signal, flexibility estimate, prepared response, delivered-performance tracking | Staged to a later stage | Builds on §23 Stage 3–4; §2.1 v0.3.0 (command lifecycle, approvals, audit). Baseline/measurement rules are region/product-specific (§11.4) and require D1 input where applicable (see §5) |
| 6 | Renewable and storage coordination: PV curtailment, battery charging/discharging, controllable loads | Staged to a later stage | Builds on §23 Stage 3 capability models and scheduling; §2.1 v0.2.0 |
| 7 | Dispatch assurance: capability, validity, topology, network, policy, and authorization gates before issue | Staged to a later stage | §23 Stage 4 (grid feasibility and dispatch assurance); §2.1 v0.3.0 (Dispatch Assurance) |
| 8 | Operational reporting: schedule-versus-actual, availability, rejected commands, delivery performance, audit history | Staged to a later stage | §23 Stage 4 (command lifecycle and audit) and Stage 5 (operator audit and reports); §2.1 v0.3.0 audit trail |

Review note: per CONTEXT.md (open question 3), the §23 Stage 2 exit gate concerns telemetry ingestion, not use-case mandating; portfolio visibility is one use case within the v0.1.0 delivery scope named by §2.1, and it is the use case that scope exists to serve. The disposition matrix is reviewed at the Stage 0 exit gate (ledger P0.02 verification method).

Scope-creep rule: no Phase 0–2 artefact, demo, or plan may claim all eight first-generation use cases for v0.1.0. Only row 1 is claimed for v0.1.0.

## 3. Deferred "later use cases" (§1.5, all eight, no delivery date promised)

The following §1.5 "later use cases, subject to validation" are **deferred with no delivery date promised**. Their eventual landing places (report research tracks, §23 Stage 8, §2.1 v1.x/v2.x) are recorded in the report; this document promises no date for any of them:

1. wholesale market bidding and settlement;
2. ancillary-service portfolio qualification and delivery tracking;
3. reserve forecasting and capacity accreditation;
4. multi-aggregator or multi-DSO coordination;
5. distributed optimization among sites that do not share private internal data;
6. GNN-assisted OPF warm starts and contingency ranking;
7. safe MARL for selected fleet coordination problems; and
8. dynamic stability / inverter-dominated grid research.

Deferred means deferred: naming a stage or release for any item above would contradict the report's "subject to validation" framing (§1.5) and the per-feature qualification posture of §2.1 (v1.x/v2.x rows).

## 4. Initial operating scenario (§1.4, stated concretely)

Per §1.4, the initial design supports a **VPP aggregator coordinating a portfolio composed of several simulated or connected sites**. Concretely, the first portfolio (simulated; §2.1 v0.1.0 is simulation-only) consists of multiple simulated sites, each containing one or more of:

- battery energy storage systems (BESS);
- photovoltaic (PV) inverters;
- controllable commercial or industrial loads;
- EV supply equipment / charging stations (EVSE);
- meters and site controllers; and
- a site-level EMS such as OpenEMS.

Over this portfolio the platform (§1.4): estimates each resource's available power and energy flexibility, aggregates the site capabilities, creates a schedule, validates the schedule against known network and asset constraints, and manages a controlled dispatch workflow. For v0.1.0 specifically, only the registry/telemetry/visibility/replay portion of this workflow is delivered (§2.1); the flexibility, scheduling, validation, and dispatch portions arrive in v0.2.0–v0.3.0 (§2.1) and §23 Stages 3–5. The §24 vertical slice (see `24_vertical_slice_map.md`, P0.24) spans v0.1.0–v0.3.0 and is not all promised in the first public release.

The site EMS integration named in §1.4 ("a site-level EMS such as OpenEMS") matches §2.3, where OpenEMS is the priority integration for lab/demo and selected pilot sites; v0.1.0 ships simulation adapters only (§2.3).

## 5. First-portfolio definition sheet — candidate inputs for D2 (not decided)

D2 is open (§25.2). The elements below are the candidate definition sheet P0.04 requires (asset mix, site count, service demonstrated, acceptance criteria); every value is a **recommendation pending D2**, and the sheet is completed and confirmed at the D2 gate (P0.04).

| Sheet element | Candidate value (recommendation pending D2) | Report basis |
|---|---|---|
| Number of simulated sites | Several, recommended as 3 — the report fixes no number ("several simulated or connected sites", §1.4); the exact count is a D2 output | §1.4; §25.2 (D2) |
| Asset mix per site | BESS, PV inverter, flexible C&I load, EVSE, site meter (mix may vary per site; each site contains one or more of the §1.4 asset kinds) | §1.4; §2.2 |
| Asset support level | See table below — each row cites §2.2 verbatim | §2.2 |
| Service demonstrated | Portfolio visibility over the simulated portfolio, with deterministic scenario replay (the §2.1 v0.1.0 capability set) | §2.1 |
| Acceptance criteria | To be written at the D2 gate before Phase 2 builds the registry (ledger P0.04 risk note); not pre-filled here | §25.2; ledger P0.04 |
| Baseline method | **OPEN** — see below | §11.4; §25.2 |

Asset support levels, citing §2.2 (which states: "Support accrues across the 0.x line; this table describes the target initial capability, not the v0.1.0 contents"):

| Asset type | Initial support (§2.2) | Later extension (§2.2) |
|---|---|---|
| BESS | Full simulated model; telemetry and capability model; schedule/dispatch proposal | Vendor-specific write adapters, degradation-aware dispatch, reserve qualification |
| PV inverter | Simulated generation and curtailment capability; telemetry and setpoint model | Vendor/region-specific smart inverter control profiles |
| Flexible C&I load | Aggregated availability, baseline, ramp, duration, recovery constraints | Device-specific HVAC/process/thermal models |
| EVSE / EV fleet | Simulated charging flexibility; OCPP adapter planned but not delivered in v0.1.0 | OCPP 2.0.1/2.1 profile support according to fleet/vendor capabilities and conformance testing |
| Site meter | Telemetry, quality, PCC measurements | Vendor-specific metering and settlement interfaces |

(§2.2 also lists wind, thermal storage/heat pumps, and backup generation — generic interfaces or not a first-class 0.x target — none of which is recommended for the first portfolio.)

### Baseline-method question — OPEN

Per §11.4: "For DR and market reporting, baseline and measurement rules must be region/product-specific." The baseline method for the first portfolio is therefore **recorded as OPEN**, not silently assumed:

- For the v0.1.0 service (portfolio visibility and replay), no DR/market baseline applies; flexible-load baseline demand is a model attribute (§11.4), not a market-settlement baseline.
- For any later DR or market reporting use case (§1.5 rows 5, and the deferred wholesale/ancillary list), the baseline and measurement rules require **D1 input** (target jurisdiction and market, §25.2) — they cannot be chosen generically. This dependency is handed to P0.03/P0.04; nothing here selects a baseline method.
