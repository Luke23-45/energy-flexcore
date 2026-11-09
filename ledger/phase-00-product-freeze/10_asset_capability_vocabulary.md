# P0.10 — Asset and capability vocabulary

| Field | Value |
|---|---|
| Subtask | P0.10 — Freeze the asset and capability vocabulary |
| Report trace | §10.1 (canonical entities, normative source), §11.1–§11.6 (capability models), §2.2 (initial asset support), §4 (Asset Registry ownership) |
| Source | `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md`; foundations specification §6.3 governs on conflict (cross-checked here) |
| Status | Draft for Stage 0 review |
| Consumers | Phase 2 registry schema review checks every field against this vocabulary (ledger P0.10); capability implementation is Phase 3 |

---

**This document is vocabulary, not a database schema.** Schema design is Phase 1/2 work; no table, column, or storage decision is made here. No field may be invented that is not listed in §10.1 (ledger P0.10 risk note).

## 1. Canonical entities (§10.1)

All fourteen §10.1 canonical entities, with their key fields:

| Entity | Key fields (§10.1) |
|---|---|
| Tenant | `tenant_id`, name, identity provider reference, policy profile |
| Portfolio | `portfolio_id`, tenant, operator, market region, aggregation rules |
| Site | `site_id`, portfolio, location, timezone, PCC, grid model reference, edge identity |
| Asset | `asset_id`, site, type, vendor/model, rated power/energy, phase, bus reference, capability profile |
| AssetState | SOC, active/reactive power, availability, operating mode, health, timestamp, quality |
| CapabilityEnvelope | time-indexed P/Q limits, energy limits, ramp, duration, availability, uncertainty |
| GridModel | model/version, buses, branches, transformers, switches, phases, base values, limits |
| ForecastBundle | horizon, resolution, variable, values, quantiles/scenarios, provider/version |
| Schedule | portfolio, horizon, interval, target at PCC, resource allocation, objective, version |
| DispatchCommand | command ID, asset/site, target, validity, sequence, source, validation and authorization references |
| DispatchExecution | issued/acknowledged/measured values, timestamps, deviations, failure code |
| MarketCommitment | region, product, delivery point, interval, quantity, baseline, status, adapter reference |
| PluginInstance | plugin ID/version, profile, config ref, capabilities, health, lifecycle state |
| AuditRecord | actor, action, object, before/after references, decision, evidence, timestamp, correlation ID |

Per §4 (bounded contexts), the Asset Registry module owns asset identity, type, topology reference, capabilities, constraints, and firmware/vendor metadata, and does not own raw telemetry transport.

## 2. Initial asset support (§2.2, reproduced)

Support accrues across the 0.x line; this table describes the target initial capability, not the v0.1.0 contents (§2.2 preamble).

| Asset type | Initial support | Later extension |
|---|---|---|
| BESS | Full simulated model; telemetry and capability model; schedule/dispatch proposal | Vendor-specific write adapters, degradation-aware dispatch, reserve qualification |
| PV inverter | Simulated generation and curtailment capability; telemetry and setpoint model | Vendor/region-specific smart inverter control profiles |
| Flexible C&I load | Aggregated availability, baseline, ramp, duration, recovery constraints | Device-specific HVAC/process/thermal models |
| EVSE / EV fleet | Simulated charging flexibility; OCPP adapter planned but not delivered in v0.1.0 | OCPP 2.0.1/2.1 profile support according to fleet/vendor capabilities and conformance testing |
| Site meter | Telemetry, quality, PCC measurements | Vendor-specific metering and settlement interfaces |
| Wind generation | Generic DER capability interface; later model | Wind-specific forecasts and controls |
| Thermal storage / heat pumps | Generic flexible-load envelope initially | Dedicated state and comfort/process constraints |
| Backup generation | **Not a first-class 0.x target** | Add only with emissions, fuel, permit, and operating policy support |

## 3. Generic capability rule (§11.1)

Capability is a **time-indexed feasible operating set** — the report states the model explicitly as X(i,t) = {(P, Q, E, u) : device and operating constraints hold} — and **is not a single `max_mw` field**.

The feasible set may depend on:

- active and reactive power;
- energy/SOC;
- device mode;
- temperature and health;
- ramp rate;
- minimum up/down time;
- duration and recovery constraints;
- site network constraints;
- forecast conditions; and
- operator/customer commitments.

## 4. Constraint provenance rule (§11.1)

Each asset adapter must state which constraints are **measured**, **configured**, **inferred**, or **unknown**. An adapter that does not declare constraint provenance is not conformant with the Phase 0 vocabulary.

## 5. Asset-class model vocabularies

Each subsection below is **vocabulary only — implementation is Phase 3**. Definitions are recorded so that Phase 3 implements frozen terms rather than inventing them.

### 5.1 BESS model (§11.2)

- Discharge power `P_dis` ≥ 0 and charge power `P_ch` ≥ 0; net export `P_net = P_dis − P_ch` (units and sign per P0.07).
- Discrete energy balance: `E(t+1) = E(t) + η_ch · P_ch(t) · Δt − P_dis(t)/η_dis · Δt`.
- Bounds: `E_min ≤ E(t) ≤ E_max`; `0 ≤ P_ch ≤ P_ch_max`; `0 ≤ P_dis ≤ P_dis_max`.
- If simultaneous charge/discharge must be prohibited, use a valid mixed-integer mode formulation or a justified convex relaxation plus post-solution checks; a model with binary charge/discharge variables is never labelled a continuous SOCP (§11.2).
- Later additions where data is available: SOC-dependent power limits, reserve margins, temperature, degradation cost, BMS constraints.

### 5.2 Inverter P–Q envelope (§11.3)

- Apparent-power constraint `P² + Q² ≤ S_max²` is necessary but not complete: real envelopes are device/standard-profile-specific limits `P_min(m,V) ≤ P ≤ P_max(m,V)` and `Q_min(P,V,m) ≤ Q ≤ Q_max(P,V,m)`, where `m` is the operating mode and `V` may matter for volt-var/watt-var capability.
- Charging and discharging must both be represented; a power-factor inequality that assumes `P ≥ 0` must not be applied to a bidirectional asset.
- IEEE 1547-2018 defines interconnection/interoperability performance requirements (Clause 5 covers reactive-power and voltage/power-control capabilities); the applicable capability depends on category, performance requirements, utility profile, and equipment. It is not a universal internal BMS cell-temperature specification, and IEEE 1547.1 conformance testing is a separate responsibility [R18].

### 5.3 Flexible load model (§11.4)

A flexible load is more than a negative power injection. It may need:

- baseline demand;
- reducible/increasable power range;
- response time;
- maximum event duration;
- minimum recovery time;
- rebound energy;
- comfort/process constraints; and
- availability windows.

For DR and market reporting, baseline and measurement rules must be region/product-specific (§11.4).

### 5.4 EV fleet model (§11.5)

The fleet flexibility model should include:

- EVSE rated power and phase;
- vehicle connection windows;
- required departure energy/SOC;
- minimum charging power and ramp;
- user override and opt-out;
- charging priority;
- site transformer limit; and
- bidirectional capability only when the EV, EVSE, protocol, interconnection agreement, and market rules support it.

OCPP 2.1 includes expanded smart-charging and V2X-related capabilities, but device and market support must be verified [R19].

### 5.5 Portfolio aggregation guard (§11.6)

The portfolio flexibility envelope is a constrained aggregation, not the sum of nameplate MW: `P_portfolio(t) = Σ_i P_net(i,t) − P_loss/adjustment(t)`, subject to each asset's time-coupled constraints and site/network coupling. Summing independent maxima can overstate deliverable flexibility under shared weather, site, communication, or policy exposure; the aggregator accounts for uncertainty and correlation and returns a central estimate with a confidence/availability descriptor where the model supports it. Recorded here because treating nameplate MW as flexibility is a named failure mode of this subtask (ledger P0.10 risk note).

## 6. Foundations §6.3 cross-check (feeds P0.26)

| This freeze | Foundations §6.3 (Asset capability representation) | Outcome |
|---|---|---|
| Time-indexed feasible operating set; not a single `max_mw` (§3) | "Asset models must describe more than nameplate MW" (operating mode, envelopes, ramp, availability, duration/energy budget, state variables, efficiency/losses, temperature/process limits, communication state, provenance) | Consistent |
| §2 asset support table (§2.2) | §6.3 asset-class minimum model elements (BESS, solar PV, wind, EV fleet, flexible C&I load, site/microgrid) | Consistent; the report's §2.2 table is release-scoping, foundations §6.3 is model content — no conflict |
| BESS vocabulary (§5.1) | §6.3 BESS row (charge/discharge limits, E capacity, SOC/E state, SOC bounds, efficiency, ramp, apparent power, Q envelope, availability, auxiliary load, degradation model) | Consistent |
| Inverter envelope (§5.2) | Foundations §7.3 (P² + Q² ≤ S_max²; device/mode-specific polygons; no P ≥ 0 power-factor assumption on bidirectional inverters) | Consistent |

Outcome: zero unresolved conflicts.

## 7. Acceptance criteria coverage (ledger P0.10)

- [x] All 14 §10.1 entities are listed with key fields (§1).
- [x] The §2.2 table is reproduced with "initial support" and "later extension" columns intact; backup generation is marked not a first-class 0.x target (§2).
- [x] The "not a single max_mw" rule (§3) and the measured/configured/inferred/unknown rule (§4) are stated.
- [x] BESS, inverter P–Q, flexible-load, and EV-fleet model sections are marked "vocabulary only — implementation is Phase 3" (§5).
