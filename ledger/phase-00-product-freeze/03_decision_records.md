# P0.03–P0.06 — Decision records and scheduled gates (D1–D6)

| Field | Value |
|---|---|
| Subtask IDs | P0.03 (D1), P0.04 (D2), P0.05 (D3), P0.06 (D4/D5/D6); gate-table skeleton feeds P0.28 |
| Report trace | §25.2 (deferred decisions D1–D6), §2.2 (asset support), §2.3 (protocol posture), §11.4 (baseline method), §12.4 (grid-model providers), §22.1 (ADR-011), §23 Stage 0 |
| Inputs | Report v1.1; `RESEARCH.md` (this phase, verified 7 November 2026); `.agent/DECISIONS.md`; `.agent/PROVENANCE.md`; `02_first_buyer_use_cases.md` (P0.02 deliverable) |
| Status | Draft for Stage 0 review |
| Date rule | No calendar dates are invented here. Every target date is a placeholder of the form "date: assigned at the Stage 0 gate meeting", per the ledger (P0.28 step 2: relative ordering only; dates are assigned in the gate meeting) |

## Status vocabulary used in this document

- **SCHEDULED** — the gate is open; a binding owner (role), a decision venue, and a target-date placeholder are recorded. This is the §23 Stage 0 exit-gate requirement for §25.2 decisions.
- **RECOMMENDATION** — an engineering default usable for simulation work only. It is not a decision; it must never be written up as decided, approved, or committed (`.agent/DECISIONS.md` rule; ledger "How to use").
- **OPEN** — the question itself, unresolved.

Nothing in this document closes D1–D6. Closing a gate requires the named owners to decide it in a recorded venue, after which `.agent/DECISIONS.md` is updated and an ADR is filed (P0.21 writes the ADR set; ADR-011 already carries the standing market-adapter position).

---

## 1. D1 — Target jurisdiction and market (P0.03)

### 1.1 Why D1 is deferred

D1 drives interconnection requirements, the market adapter, data residency, and liability (§25.2, D1 row). These four consequences cannot be evaluated without knowing a jurisdiction and, where applicable, an RTO/ISO or market programme. The report deliberately leaves this decision to Neryva and forbids silently assuming an answer (§25.2 preamble).

### 1.2 Standing position (not contingent on D1)

FERC Order 2222 is a US regulatory framework whose implementation is mediated through individual RTO/ISO tariffs and requirements. It is not itself a protocol and does not create a universal market API. Regional requirements must be handled by market-specific adapters and configuration (§2.3; §22.1 ADR-011: "Market adapters are region-specific; FERC Order 2222 is not a protocol"). This matches the primary-source finding in `RESEARCH.md` §1 (FERC Order 2222 Fact Sheet; report [R1]).

**Consequence, binding on all later phases:** no market adapter may be built on a generic "Order 2222" mapping. A market integration is always a region-specific adapter plugin (§2.3, ISO/RTO-specific APIs row), selected or configured only after D1 names a jurisdiction and market programme.

### 1.3 Live data point (research, not a decision input by itself)

ISO-NE implemented Order 2222 with an effectiveness date of **1 November 2026** — participants can begin to register DER aggregations (DERAs); minimum aggregation 100 kW (source: ISO New England participant-readiness page, https://www.iso-ne.com/participate/support/participant-readiness-outlook/ferc-order-no-2222, last updated 4 November 2026; recorded in `RESEARCH.md` §1). This confirms the report's §2.3 statement that implementation is per-ISO and staggered. It is a data point for the D1 owners, not a signal that ISO-NE is the assumed jurisdiction.

### 1.4 Recommended default for simulation work only, not a decision

No jurisdiction is assumed anywhere in the 0.x simulation line; market integration stays a region-specific adapter (§25.2, §2.3). Phase 0/1/2 artefacts may not name a jurisdiction or market as decided unless the D1 owners close the gate. Any Phase 1/2 work needing a "market signal" input uses simulated market signals, not an ISO/RTO integration.

### 1.5 Gate record

| Field | Value |
|---|---|
| Status | SCHEDULED (OPEN) |
| Owner | Product + legal (roles; named persons assigned at the gate meeting — see §6) |
| Venue | Stage 0 gate meeting, then a product + legal review with interconnection, data-residency, and liability consequences recorded |
| Target date | date: assigned at the Stage 0 gate meeting |
| On closing | File an ADR naming the jurisdiction, the RTO/ISO or market programme (if any), and the interconnection/data-residency/liability consequences; update `.agent/DECISIONS.md` D1 row |
| Escalation | D1 undecided after the gate date blocks the start of Stage 1 (§6 below) |

### 1.6 Acceptance-criteria coverage (P0.03)

- D1 is scheduled with owner + venue + date placeholder recorded. ✔ (§1.5)
- The record states that Order 2222 is not a protocol and does not create a universal market API. ✔ (§1.2)
- No Phase 0/1/2 artefact names a jurisdiction or market as decided unless D1 closes. ✔ (§1.4; verification by grep of later-phase artefacts is P0.03's verification method)

---

## 2. D2 — First portfolio and service (P0.04)

### 2.1 Why D2 is deferred

D2 determines asset mix, baseline method, and acceptance criteria (§25.2, D2 row). A portfolio built before the service it demonstrates is chosen risks an asset mix with no service, and acceptance criteria written after the build instead of before Phase 2 constructs the registry (P0.04 risks).

### 2.2 First-portfolio definition sheet

The concrete first-portfolio definition — first buyer, number of simulated sites, asset mix per §2.2, and the demonstrated service — is defined in the P0.02 deliverable **`02_first_buyer_use_cases.md`** and is not duplicated here. That document is the input this record relies on; it is expected to name the asset mix from the §2.2 initial-support table (BESS, PV inverter, flexible C&I load, EVSE/EV fleet, site meter — the first-class 0.x rows; wind and thermal storage as generic envelopes; backup generation not a first-class 0.x target), a site count, and the demonstrated service, citing §2.2 for each asset's support level.

### 2.3 Baseline-method question — OPEN

Per §11.4, for DR and market reporting, baseline and measurement rules must be region/product-specific. No baseline method is selected in Phase 0. The question is recorded as **OPEN**, and it requires D1 input where the eventual service turns out to be DR/market-facing: the applicable baseline and measurement rules follow from the jurisdiction and product programme chosen at D1. It must not be silently assumed — the answer travels with D2 when the gate closes.

### 2.4 Gate record

| Field | Value |
|---|---|
| Status | SCHEDULED (OPEN) |
| Owner | Product (role; named person assigned at the gate meeting — see §6) |
| Venue | Stage 0 gate meeting, then a product review over the first-portfolio definition sheet (`02_first_buyer_use_cases.md`) |
| Target date | date: assigned at the Stage 0 gate meeting |
| On closing | File an ADR referencing the first-portfolio sheet, the chosen baseline-method answer, and the resulting acceptance criteria; update `.agent/DECISIONS.md` D2 row |
| Escalation | D2 undecided after the gate date blocks the start of Stage 1 (§6 below) |

### 2.5 Acceptance-criteria coverage (P0.04)

- D2 is scheduled with owner + venue + date placeholder. ✔ (§2.4)
- The first-portfolio sheet (asset mix per §2.2, site count, service) is referenced by file name rather than duplicated. ✔ (§2.2)
- The baseline-method question is recorded as open, not silently assumed, with its D1 dependency stated. ✔ (§2.3)

---

## 3. D3 — Network class and model fidelity (P0.05)

### 3.1 Why D3 is deferred

Balanced radial distribution feeders, unbalanced three-phase distribution, and meshed transmission require different models and solvers (§25.2, D3 row). The §12.4 provider table (pandapower, GridLAB-D, OpenDSS, PyPSA, ANDES, HELICS — each with a recommended role and an explicit boundary) only becomes a selection decision once the network class is known; choosing a solver before the network class is known is a recorded failure mode of P0.05.

### 3.2 Recommended default for 0.x simulation work only — RECOMMENDATION, NOT A DECISION

The following is the report's recommended default (§25.2 closing paragraph; `.agent/DECISIONS.md` "Recommended defaults" table), usable for planning and 0.x simulation work only. It is a **recommendation, not a decision**; it is wrong the moment a customer, jurisdiction, or pilot says otherwise.

| Area | Recommended default for 0.x | Revisit trigger |
|---|---|---|
| Network class | Balanced radial distribution feeders | A pilot requires unbalanced or meshed modelling |
| Primary power-flow provider | pandapower (balanced AC power flow per §12.4) | Model fidelity or performance requires OpenDSS/GridLAB-D |
| Unbalanced three-phase | Enters the track from v0.4.0 | A pilot needs it earlier |
| Meshed transmission | Out of scope for 0.x unless D3 closes otherwise | A transmission-level use case is approved |

The revisit trigger for the network-class default is a pilot requiring unbalanced or meshed modelling. No Phase 0/0.x artefact may describe this default as decided, approved, or committed, and no artefact may assume unbalanced three-phase support in v0.1.0 — it enters from v0.4.0.

### 3.3 Consequence for later phases

- Phase 2 grid-model reference fields must carry the model class and the model version (P2 subtasks reference this; §12.5 requires feasibility checks against a versioned network model). A grid-model reference without class + version is a Phase 2 review failure.
- Phase 4 feasibility work depends on this decision: the grid-model provider selection in Phase 4 is gated on D3 (P0.05 verification and risk notes; §12.4 "choose one primary deterministic path").

### 3.4 Gate record

| Field | Value |
|---|---|
| Status | SCHEDULED (OPEN) |
| Owner | Power systems (role; named person assigned at the gate meeting — see §6) |
| Venue | Stage 0 gate meeting, then a power-systems review over §12.4 and §25.2 (network class, model fidelity, primary provider) |
| Target date | date: assigned at the Stage 0 gate meeting |
| On closing | File an ADR naming the network class, the model fidelity, and the primary provider; update `.agent/DECISIONS.md` D3 row |
| Escalation | D3 undecided after the gate date blocks the start of Stage 1 (§6 below) |

### 3.5 Acceptance-criteria coverage (P0.05)

- D3 is scheduled with owner + venue + date placeholder. ✔ (§3.4)
- The network-class statement names the 0.x default and its revisit trigger, and labels the default as a recommendation. ✔ (§3.2)
- Meshed transmission is recorded as out of scope for 0.x unless D3 closes otherwise. ✔ (§3.2 table)

---

## 4. D4 — License and dependency audit (P0.06, scheduled gate — not a Phase 0 closure)

| Field | Value |
|---|---|
| Status | SCHEDULED (OPEN) — gate outside Phase 0 |
| Decision | License of the core and a dependency audit for Apache-2.0 compatibility of protocol stacks, solvers, models, and datasets (§25.2 D4 row) |
| Gate | Before v0.1.0 (the first public release) |
| Owner | Legal + engineering (roles; named persons assigned at the gate meeting — see §6) |
| Venue | License and dependency review (legal + engineering), recorded as a review approval before the v0.1.0 release gate |
| Target date | date: assigned at the Stage 0 gate meeting (relative ordering: precedes the v0.1.0 release) |
| Current state | The core license is open; Apache-2.0 is the recommendation (§25.1 "Apache-2.0 core license", with its own revisit trigger: "License audit finds incompatible required dependencies"). The repository currently has **no license file**, and `.agent/PROVENANCE.md` ("License status of this setup") states that until the decision is made, the project files are internal project material and must not be distributed separately. This record was verified against that file. |
| Consequence of deciding late | D4 undecided blocks the v0.1.0 public release — an open-source release without a core license and a dependency audit cannot be published |
| Not-a-decision guard | Apache-2.0 is a recommendation; no Phase 0 artefact may describe the core license as chosen |

---

## 5. D5 — First protocol and asset mix (P0.06, scheduled gate — not a Phase 0 closure)

| Field | Value |
|---|---|
| Status | SCHEDULED (OPEN) — gate outside Phase 0 |
| Decision | Selects the pilot adapter and the conformance tests (§25.2 D5 row) — the first protocol integration and the asset mix it covers |
| Gate | Before v0.4.0 (first site integration, OpenEMS, protocol adapters — §2.1 release ladder) |
| Owner | Product + integration (roles; named persons assigned at the gate meeting — see §6) |
| Venue | Integration review with the selected pilot partner, recorded before the v0.4.0 gate |
| Target date | date: assigned at the Stage 0 gate meeting (relative ordering: precedes the v0.4.0 release / Stage 6 work) |
| Current state | v0.1.0 ships simulation adapters only (§2.3 protocol posture). OpenEMS is listed as "priority integration for lab/demo and selected pilot sites" (§2.3) — **priority is not a decision**; the pilot adapter is selected at D5, gated by the selected pilot partner and the conformance plan (§2.3 preamble). Implementing an adapter is not a conformance claim; each production adapter must pass the relevant conformance or interoperability suite before it is described as certified or qualified (§2.3) |
| Consequence of deciding late | D5 undecided blocks site integration (v0.4.0) — the first real protocol adapter cannot be built or tested without it |
| Not-a-decision guard | No Phase 0 artefact may describe OpenEMS (or any protocol) as the selected pilot adapter |

---

## 6. D6 — Cyber and OT boundary review (P0.06, scheduled gate — not a Phase 0 closure)

| Field | Value |
|---|---|
| Status | SCHEDULED (OPEN) — gate outside Phase 0 |
| Decision | Site-specific cyber and OT boundary review (§25.2 D6 row) |
| Gate | Before any read-only pilot (the earliest pilot mode above Simulation is Read-only / shadow, §21.4) |
| Owner | Security (role; named person assigned at the gate meeting — see §6) |
| Venue | Site-specific security/OT review with the site operator, recorded per site before pilot promotion |
| Target date | date: assigned at the Stage 0 gate meeting (relative ordering: precedes any read-only pilot) |
| Current state | The review is site-specific and not derivable from the architecture alone (§25.2 D6 row) — the §17 threat model and zone model are the platform-side inputs, but each site's OT boundary requires its own review |
| Consequence of deciding late | D6 undecided blocks any pilot promotion beyond simulation — no site may enter read-only/shadow mode without its boundary review |
| Not-a-decision guard | No Phase 0 artefact may describe any site's OT boundary as reviewed |

---

## 7. Gate-table skeleton (P0.28)

All six §25.2 gates, with owner **role** only. Named persons are assigned at the gate meeting — the blanks below are explicit and must not be filled from any Phase 0 artefact. Dates are placeholders; the ledger forbids inventing calendar dates (P0.28 step 2).

| Gate | Decision | Owner role | Named owner | Venue | Target date | Escalation rule |
|---|---|---|---|---|---|---|
| D1 | Target jurisdiction and market | Product + legal | ______ (assigned at the gate meeting) | Stage 0 gate meeting; then product + legal review | date: assigned at the Stage 0 gate meeting | D1 undecided blocks Stage 1 start |
| D2 | First portfolio and service | Product | ______ (assigned at the gate meeting) | Stage 0 gate meeting; then product review of `02_first_buyer_use_cases.md` | date: assigned at the Stage 0 gate meeting | D2 undecided blocks Stage 1 start |
| D3 | Network class and model fidelity | Power systems | ______ (assigned at the gate meeting) | Stage 0 gate meeting; then power-systems review (§12.4, §25.2) | date: assigned at the Stage 0 gate meeting | D3 undecided blocks Stage 1 start |
| D4 | License and dependency audit | Legal + engineering | ______ (assigned at the gate meeting) | License and dependency review, before the v0.1.0 release gate | date: assigned at the Stage 0 gate meeting (precedes v0.1.0) | D4 undecided blocks v0.1.0 |
| D5 | First protocol and asset mix | Product + integration | ______ (assigned at the gate meeting) | Integration review with the pilot partner, before the v0.4.0 gate | date: assigned at the Stage 0 gate meeting (precedes v0.4.0 / Stage 6) | D5 undecided blocks Stage 6 |
| D6 | Cyber and OT boundary review | Security | ______ (assigned at the gate meeting) | Site-specific security/OT review, before any read-only pilot | date: assigned at the Stage 0 gate meeting (precedes any read-only pilot) | D6 undecided blocks pilot promotion beyond simulation |

Notes:

- The escalation rule matches P0.28 step 3: a gate whose date passes undecided blocks the dependent work — D1–D3 block Stage 1 start; D4 blocks v0.1.0; D5 blocks Stage 6; D6 blocks pilot promotion.
- This table is the skeleton P0.28 consumes. P0.28's own exit check ("all six gates have a named owner, a venue, and a target date; no gate recorded as TBD at the Stage 0 exit gate") is satisfied by the gate meeting filling the blanks above and recording the outcome in `.agent/DECISIONS.md` — not by this document, which deliberately leaves them blank.
- After each gate closes, the corresponding row in `.agent/DECISIONS.md` is updated and an ADR is filed (P0.21). Until then, every gate remains OPEN, and the recommendations in this document remain recommendations only.
