# Ledger — Phase 0: Product and Domain Freeze

| Field | Value |
|---|---|
| Phase | Phase 0 — Report Stage 0: Product and domain freeze |
| Report trace | §23 Stage 0; §25.2 D1/D2/D3; §22.1 ADR-001…ADR-012; §0–§2; §10 (units/sign); §14 (command state machine); §17 (threat model); §21.4 (control authority); §24 (vertical slice, steps 1–2 only) |
| Status | Not started — planning ledger only; no code, no decisions recorded |
| Simulation status | Entire 0.x line is simulation-only. Nothing in this phase authorises field control, device writes, or any control-authority mode above Simulation (§21.4, §0) |
| Authority | Report v1.1 is the product baseline; `docs/foundations/` v1.0 governs where the two conflict (AGENTS.md §2). This ledger adds no product promises |
| Spelling | "license" (US spelling); "acknowledgement" (noun) — per `.agent/WRITING.md` |

## How to use this ledger

- Every subtask has an ID (`P0.01`…), a report trace (`§`), inputs, steps, a concrete deliverable, testable acceptance criteria, an owner role, dependencies, a verification method, and risks.
- No subtask is "done" until its acceptance criteria are evidenced in the deliverable it names. Verbal agreement is not evidence.
- Decisions D1–D6 remain OPEN (`.agent/DECISIONS.md`). This phase closes D1, D2 and D3 only if the named owners decide them in a recorded venue; otherwise it records an owner, a decision venue, and a target date for each, which is itself the §23 Stage 0 exit gate. A recommended default used for simulation work must never be written up as decided, approved, or committed.
- Cross-references: Phase 1 consumes P0 outputs listed in `CONTEXT.md`. Do not duplicate Phase 0 subtasks in later phases; later phases reference `P0.xx` IDs.

## Subtask format

Each subtask uses checkboxes (`- [ ]`) for its acceptance criteria so progress is auditable. "Owner role" is a role, not a named person, until the project assigns names in P0.28.

---

## P0.01 — Freeze product scope and non-goals in a single signed-off scope note

- **Objective:** Produce one authoritative scope statement for v0.1.0 that every later phase can test scope creep against.
- **Report trace:** §0, §0.3, §1.2, §1.3, §1.6, §2.1
- **Inputs / prerequisites:** Report v1.1 §0–§2; foundations specification §6 (mandatory conventions).
- **Steps:**
  1. Extract the §0.3 "deliberately not promised" list verbatim into the scope note; do not paraphrase it into weaker language.
  2. Extract the §1.3 primary audience (4 roles), secondary audience, and "not the initial target" list.
  3. Extract the §1.6 responsibility boundary: site EMS, inverter firmware, BMS, EVSE controller, protection relays and certified local control remain responsible for device safety and fast local control.
  4. State explicitly that FlexCore is an enterprise web application backed by a central control plane, with optional site edge connectors and a simulation/research environment — not a mobile app, not an ADMS/SCADA replacement, not a protective relay controller, not an ISO/RTO market platform, not a hard-real-time controller.
  5. Circulate the note to product, power systems, platform engineering, security/OT, and legal reviewers (§Document control, "Intended reviewers").
- **Deliverable / output:** `ledger/phase-00-product-freeze/` scope note (or a docs note referenced from it) titled "FlexCore v0.1.0 scope and non-goals", versioned, with reviewer sign-off lines.
- **Acceptance criteria / exit check:**
  - [x] The note contains the full §0.3 list with no item dropped or softened. — implemented, awaiting human build/test
  - [x] The note states the four primary audiences and the five "not the initial target" groups from §1.3. — implemented, awaiting human build/test
  - [x] The note states the §1.6 responsibility boundary in full. — implemented, awaiting human build/test
  - [ ] At least one reviewer from each of the five intended-reviewer groups has signed off or recorded an objection. — NOT MET: reviewer sign-off is a human action; sign-off lines left blank in 01_scope_and_non_goals.md (blocker in HANDOVER.md)
- **Owner role:** Product
- **Dependencies:** None (first subtask).
- **Verification method:** Reviewer checklist against §0.3, §1.3, §1.6 line by line; `tools/verify_docs.py` passes if the note lives in the report tree.
- **Risks / failure modes:** Scope note paraphrased into marketing language; a non-goal (e.g. "certified protocol conformance") re-enters later via a plugin or demo claim. Mitigation: the note is the scope-creep test in every later phase gate.

## P0.02 — Freeze the first buyer and first use cases

- **Objective:** Name the first buyer persona and the first-generation use cases v0.1.0 must demonstrably serve, so Phase 2 and the §24 vertical slice have a concrete target.
- **Report trace:** §0, §1.3, §1.4, §1.5, §25.2 (D2)
- **Inputs / prerequisites:** P0.01; D2 open gate (first portfolio and service).
- **Steps:**
  1. Select one primary audience from §1.3 as the first buyer (default candidate: VPP/DER aggregator operator — a recommendation, not a decision, until product confirms).
  2. Select the subset of §1.5 first-generation use cases in scope for v0.1.0: portfolio visibility is mandatory; flexibility estimation, scheduling, peak management, demand response, renewable/storage coordination, dispatch assurance, and operational reporting are staged across §23 Stages 2–4 and must not all be claimed for v0.1.0.
  3. Write the §1.4 initial operating scenario in concrete terms: several simulated sites, each with one or more of BESS, PV, controllable load, EVSE, meters, site EMS (OpenEMS).
  4. Record what is explicitly deferred to "later use cases" (§1.5 second list: wholesale market bidding, ancillary-service qualification, reserve forecasting, multi-aggregator coordination, DMPC, GNN/MARL, dynamic stability).
- **Deliverable / output:** "First buyer and first use cases" section in the Phase 0 freeze record, naming the buyer, the in-scope use cases, and the deferred list.
- **Acceptance criteria / exit check:**
  - [x] Exactly one first buyer is named, or the decision is explicitly deferred to D2 with owner, venue, and date (P0.28). — implemented, awaiting human build/test
  - [x] Each §1.5 first-generation use case is marked in-scope for v0.1.0, staged to a later stage, or out of scope — none left unmarked. — implemented, awaiting human build/test
  - [x] All eight "later use cases" are listed as deferred, with no delivery date promised. — implemented, awaiting human build/test
- **Owner role:** Product
- **Dependencies:** P0.01
- **Verification method:** Traceability matrix: every §1.5 use case appears exactly once with a disposition.
- **Risks / failure modes:** Claiming all eight first-generation use cases for v0.1.0 (scope blow-out); picking a buyer that implies a market adapter before D1 closes. Mitigation: disposition matrix is reviewed at the Stage 0 exit gate.

## P0.03 — Close or schedule decision D1 (target jurisdiction and market)

- **Objective:** Resolve D1, or record a binding owner, decision venue, and target date for it, per the §23 Stage 0 exit gate.
- **Report trace:** §25.2 (D1), §2.3, §22.3, §23 Stage 0
- **Inputs / prerequisites:** P0.01, P0.02; FERC Order 2222 research in `RESEARCH.md` (R1 in report §27).
- **Steps:**
  1. Record why D1 is deferred: it drives interconnection, market adapter, data residency, and liability (§25.2).
  2. Record the report's standing position: FERC Order 2222 is a US regulatory framework mediated through individual RTO/ISO tariffs, not itself a protocol and not a universal market API (§2.3, §22.1 ADR-011). No market adapter may be built on a generic "Order 2222" mapping.
  3. If jurisdiction is decided: name it, name the RTO/ISO or market programme (if any), name the interconnection/data-residency/liability consequences, and file an ADR.
  4. If not decided: name the owner (product + legal), the decision venue, and the target date; record the recommended default for simulation work only (no jurisdiction assumed; market integration stays a region-specific adapter, §25).
- **Deliverable / output:** D1 decision record (decided with ADR, or scheduled with owner/venue/date) in `.agent/DECISIONS.md` and the Phase 0 freeze record.
- **Acceptance criteria / exit check:**
  - [x] D1 is either decided (ADR filed, consequences listed) or scheduled (owner + venue + date recorded). — implemented, awaiting human build/test
  - [x] No Phase 0/1/2 artefact names a jurisdiction or market as decided unless D1 is decided. — implemented, awaiting human build/test
  - [x] The record states that Order 2222 is not a protocol and does not create a universal market API. — implemented, awaiting human build/test
- **Owner role:** Product + legal
- **Dependencies:** P0.01, P0.02
- **Verification method:** Inspect `.agent/DECISIONS.md` D1 row; grep later-phase artefacts for jurisdiction claims.
- **Risks / failure modes:** A demo or adapter quietly assumes a US/ISO-NE market; data-residency or liability consequences discovered after build. Mitigation: D1 status is a Stage 0 exit-gate item (see `CHECKLIST.md`).

## P0.04 — Close or schedule decision D2 (first portfolio and service)

- **Objective:** Resolve D2, or record a binding owner, decision venue, and target date for it.
- **Report trace:** §25.2 (D2), §1.4, §1.5, §2.2, §23 Stage 0
- **Inputs / prerequisites:** P0.02; asset-mix table §2.2.
- **Steps:**
  1. Record why D2 is deferred: it determines asset mix, baseline method, and acceptance criteria (§25.2).
  2. Define the first portfolio in concrete terms: number of simulated sites, asset mix per §2.2 (BESS, PV, flexible load, EVSE, site meter), and the service it demonstrates (e.g. portfolio visibility + flexibility estimation).
  3. Define the baseline method question explicitly (per §11.4, DR/market baseline rules are region/product-specific) and mark it as requiring D1 input where applicable.
  4. If decided, file an ADR; if not, record owner (product), venue, and target date.
- **Deliverable / output:** D2 decision record; first-portfolio definition sheet (sites, assets, service, acceptance criteria).
- **Acceptance criteria / exit check:**
  - [x] D2 is either decided (ADR filed) or scheduled (owner + venue + date). — implemented, awaiting human build/test
  - [x] The first-portfolio sheet names asset mix, site count, and service, and cites §2.2 for each asset's support level. — implemented, awaiting human build/test
  - [x] The baseline-method question is recorded as open or decided — not silently assumed. — implemented, awaiting human build/test
- **Owner role:** Product
- **Dependencies:** P0.02, P0.03 (baseline method may depend on D1)
- **Verification method:** Traceability: portfolio sheet items map to §2.2 rows; D2 row in `.agent/DECISIONS.md` updated.
- **Risks / failure modes:** Building a portfolio whose baseline method is later invalidated by the chosen market/product rules; asset mix chosen without a service to demonstrate. Mitigation: acceptance criteria are written before Phase 2 builds the registry.

## P0.05 — Close or schedule decision D3 (network class and model fidelity)

- **Objective:** Resolve D3, or record a binding owner, decision venue, and target date for it.
- **Report trace:** §25.2 (D3), §12.4, §12.5, §19.3, §23 Stage 0
- **Inputs / prerequisites:** P0.02; grid-model provider table §12.4.
- **Steps:**
  1. Record why D3 is deferred: balanced radial, unbalanced three-phase, and meshed transmission need different models and solvers (§25.2).
  2. Record the recommended default for 0.x simulation work only: balanced radial distribution feeders; pandapower as primary power-flow provider; unbalanced three-phase enters from v0.4.0; meshed transmission out of scope (§25.2, `.agent/DECISIONS.md`).
  3. State explicitly that the default is a recommendation, not a decision, and list the revisit trigger (a pilot requiring unbalanced or meshed modelling).
  4. If decided, file an ADR naming network class, model fidelity, and the primary provider; if not, record owner (power systems), venue, and target date.
  5. Record the consequence for later phases: Phase 2 grid-model reference field must carry model class and version (P2 subtasks reference this); Phase 4 feasibility work depends on this decision.
- **Deliverable / output:** D3 decision record; network-class statement for the 0.x line.
- **Acceptance criteria / exit check:**
  - [x] D3 is either decided (ADR filed) or scheduled (owner + venue + date). — implemented, awaiting human build/test
  - [x] The network-class statement names the 0.x default and its revisit trigger, and labels the default as a recommendation. — implemented, awaiting human build/test
  - [x] Meshed transmission is recorded as out of scope for 0.x unless D3 decides otherwise. — implemented, awaiting human build/test
- **Owner role:** Power systems
- **Dependencies:** P0.01
- **Verification method:** D3 row in `.agent/DECISIONS.md`; network-class statement cross-checked against §12.4 and §25.2.
- **Risks / failure modes:** Assuming unbalanced three-phase support in v0.1.0 (it enters at v0.4.0); choosing a solver before the network class is known. Mitigation: Phase 4 grid-provider selection is gated on D3.

## P0.06 — Record D4, D5, D6 as scheduled gates with owners (not Phase 0 closures)

- **Objective:** Ensure the three gates that are NOT Phase 0 closures (D4 before v0.1.0, D5 before v0.4.0, D6 before any read-only pilot) each have an owner, venue, and date, so nothing is silently assumed.
- **Report trace:** §25.2 (D4/D5/D6), §22.1, §22.3, §23 Stage 0
- **Inputs / prerequisites:** P0.01–P0.05.
- **Steps:**
  1. D4 (license and dependency audit): owner legal + engineering; gate "before v0.1.0". Record that the core license is an open decision, Apache-2.0 is the recommendation, and the repo currently has no license file (`.agent/PROVENANCE.md`).
  2. D5 (first protocol and asset mix): owner product + integration; gate "before v0.4.0". Record that v0.1.0 ships simulation adapters only (§2.3).
  3. D6 (cyber and OT boundary review): owner security; gate "before any read-only pilot". Record that it is site-specific and not derivable from the architecture alone (§25.2).
  4. For each, record the consequence of deciding late (e.g. D4 late blocks public release; D6 late blocks any pilot promotion beyond simulation).
- **Deliverable / output:** D4/D5/D6 scheduled-gate records in `.agent/DECISIONS.md` and the Phase 0 freeze record.
- **Acceptance criteria / exit check:**
  - [x] Each of D4, D5, D6 has an owner, a decision venue, and a target date recorded. — implemented, awaiting human build/test
  - [x] Each record states its gate (before v0.1.0 / before v0.4.0 / before any read-only pilot). — implemented, awaiting human build/test
  - [x] No Phase 0 artefact describes D4/D5/D6 as decided. — implemented, awaiting human build/test
- **Owner role:** Product (coordinator); legal + engineering (D4); product + integration (D5); security (D6)
- **Dependencies:** P0.01
- **Verification method:** Inspect `.agent/DECISIONS.md` D4–D6 rows.
- **Risks / failure modes:** D4 assumed decided because Apache-2.0 is "recommended"; D5 assumed because OpenEMS is "priority" (§2.3) — priority is not a decision. Mitigation: gate dates are reviewed at every later phase gate.

## P0.07 — Freeze canonical units and sign conventions

- **Objective:** Produce the single canonical units/sign contract that every adapter, API, and UI in later phases must enforce at the boundary.
- **Report trace:** §10.2, §0 (revision note), §11.2, §11.3; foundations §6
- **Inputs / prerequisites:** Report §10.2; foundations specification §6 (governs on conflict).
- **Steps:**
  1. Tabulate canonical internal units: active power MW; reactive power MVAr; apparent power MVA; energy MWh; voltage kV at nominal base (per-unit in solver representation, base declared explicitly); current A or per-unit on an explicit base; frequency Hz; SOC fraction in [0,1] at the API boundary.
  2. State the active-power sign rule: P > 0 means export/injection into the grid at the point of connection; P < 0 means import/consumption.
  3. State the reactive-power rule: no universal sign; each grid model and device profile declares whether Q > 0 means injection (generator convention) or consumption (load convention); each adapter records the mapping it applies; platform contracts carry the declared convention explicitly.
  4. State the BESS net-power relation from §11.2: P_net = P_dis − P_ch, with P_dis, P_ch ≥ 0.
  5. State the adapter rule: adapters convert vendor/protocol units into canonical units and record both the unit and the sign-mapping profile; persisted values and API contracts use canonical units; the UI may display kW/kWh and other scaled units but must not persist them as canonical.
  6. State the SOC rule: percentages exist only as display or vendor-source values and are converted at the adapter.
  7. Cross-check every line against foundations §6; record any conflict and resolve in favour of the foundations document, with a deviation ADR if the report is to be followed instead. Record the known SOC representation divergence explicitly (see acceptance): foundations §6.2 permits "fraction in [0,1] or percent, but one representation is selected per API and stated explicitly", while report §10.2 mandates fraction [0,1] at the API boundary. The report rule is the stricter selection that foundations §6.2 permits — a compatible refinement, not a conflict — and the freeze records it as such in P0.26.
- **Deliverable / output:** "Canonical units and sign conventions" contract table (one page, normative), referenced by Phase 1 (API contracts) and Phase 2 (telemetry model).
- **Acceptance criteria / exit check:**
  - [x] All seven canonical units are listed with no kW/kWh in any contract row. — implemented, awaiting human build/test
  - [x] P > 0 = export is stated with the point-of-connection qualifier. — implemented, awaiting human build/test
  - [x] The reactive-power rule states per-model/per-profile declaration and per-adapter mapping; no universal Q sign is assumed anywhere. — implemented, awaiting human build/test
  - [x] SOC [0,1] at the API boundary is stated; percentage handling is adapter-only. — implemented, awaiting human build/test
  - [x] Foundations §6 cross-check is recorded, with zero unresolved conflicts. — implemented, awaiting human build/test
  - [x] The SOC representation divergence is recorded in the P0.26 table: foundations §6.2 permits fraction or percent per API; the report's fraction-only rule is recorded as the stricter selection permitted by §6.2, not as a conflict. — implemented, awaiting human build/test
- **Owner role:** Power systems + platform engineering
- **Dependencies:** P0.01
- **Verification method:** Line-by-line comparison against §10.2 and foundations §6; later phases cite this table by ID (no re-definition allowed).
- **Risks / failure modes:** A single adapter assuming a universal Q sign; kW leaking into a persisted contract; SOC percent stored as canonical. Mitigation: Phase 1/2 acceptance criteria require a units test per adapter (see P1/P2 ledgers).

## P0.08 — Freeze the telemetry quality vocabulary (six states, exactly one per value)

- **Objective:** Freeze the six-state quality vocabulary and the source-state mapping rule before any telemetry code exists.
- **Report trace:** §10.4, §3.4, §10.1 (AssetState), §15.3 (`TelemetryQualityChanged`)
- **Inputs / prerequisites:** P0.07.
- **Steps:**
  1. Tabulate the six states with the §10.4 meanings: GOOD (valid, fresh, within range, trusted source); SUSPECT (parsed but a plausibility/range/consistency check failed); BAD (source reports invalid, or a hard validation rule failed); MISSING (no value for the expected interval); ESTIMATED (derived/interpolated/substituted/model-generated, not measured); STALE (last known value whose age exceeds the use-case freshness limit).
  2. State the exactly-one rule: every telemetry value carries exactly one canonical quality state.
  3. State the mapping rule: richer source states (SUBSTITUTED, OUT_OF_RANGE, UNSYNCHRONIZED, COMMUNICATION_LOSS, UNRELIABLE) map to one of the six at the adapter boundary, with the original source code retained as diagnostic metadata.
  4. State the non-silencing rule: a quality flag must not be silently converted into a normal reading.
  5. State the per-use-case acceptability rule: every use case defines which quality states are acceptable (to be specified per use case in later phases; Phase 2 defines the defaults for current-state and historical views).
  6. State the foundations §6.4 estimation rule and carry it forward: estimation/interpolation is explicit, tagged, bounded by policy, and disallowed for safety-critical authorization unless approved. The disallowance has no carrier in Phase 2 (telemetry only) — it is recorded here and handed to Phase 4, where command authorization first exists, as a constraint on any authorization decision that would rely on ESTIMATED values.
- **Deliverable / output:** Telemetry-quality vocabulary table in the Phase 0 freeze record.
- **Acceptance criteria / exit check:**
  - [x] Exactly six states, each with the §10.4 meaning reproduced. — implemented, awaiting human build/test
  - [x] The exactly-one rule and the source-mapping rule are stated. — implemented, awaiting human build/test
  - [x] The non-silencing rule is stated. — implemented, awaiting human build/test
  - [x] The foundations §6.4 estimation rule is stated, including that estimation is disallowed for safety-critical authorization unless approved, with the Phase 4 carry-forward recorded. — implemented, awaiting human build/test
  - [x] No seventh state is introduced anywhere in Phase 0 artefacts. — implemented, awaiting human build/test
- **Owner role:** Power systems + platform engineering
- **Dependencies:** P0.07
- **Verification method:** Comparison against §10.4; grep Phase 0 artefacts for non-canonical quality names.
- **Risks / failure modes:** A simulator emitting a seventh state; ESTIMATED values displayed as measured. Mitigation: Phase 2 adapter contract tests enforce the six-state enum (P2 ledger).

## P0.09 — Freeze the time model

- **Objective:** Freeze the time-field vocabulary and UTC rule before any timestamped record is designed.
- **Report trace:** §10.3, §3.4, §14.3 (valid-from/valid-until)
- **Inputs / prerequisites:** P0.07.
- **Steps:**
  1. Define the five time fields: `event_time` (when the measurement/event occurred); `ingest_time` (when FlexCore received it); `valid_from`/`valid_until` (validity window where applicable); source sequence or version; timezone/UTC offset where relevant; clock-quality/synchronisation status if available.
  2. State the UTC rule: UTC internally; original site timezone retained for operator display and market calendar calculations.
  3. State the DST rule: do not assume a local calendar day always equals 24 hours.
  4. State the clock-quality rule: clock drift is a threat-model item (§17.4) and a data-quality input; synchronisation status is recorded where available.
- **Deliverable / output:** Time-model definition in the Phase 0 freeze record.
- **Acceptance criteria / exit check:**
  - [x] All time fields from §10.3 are defined; `event_time` and `ingest_time` are distinguished in every later schema (Phase 1/2 reference this). — implemented, awaiting human build/test
  - [x] UTC-internal rule and DST rule are stated. — implemented, awaiting human build/test
  - [x] Clock-quality/synchronisation status is a recorded field, not an assumption. — implemented, awaiting human build/test
- **Owner role:** Platform engineering
- **Dependencies:** P0.07
- **Verification method:** Comparison against §10.3; Phase 1/2 schema reviews cite this definition.
- **Risks / failure modes:** Local time stored as canonical; DST day treated as 24 h in a schedule horizon. Mitigation: Phase 2 telemetry tests include a DST-boundary case.

## P0.10 — Freeze the asset and capability vocabulary

- **Objective:** Freeze the asset types, capability-model vocabulary, and "measured / configured / inferred / unknown" discipline for v0.1.0.
- **Report trace:** §10.1, §11.1–§11.6, §2.2, §4 (Asset Registry)
- **Inputs / prerequisites:** P0.02, P0.04, P0.07.
- **Steps:**
  1. List the §10.1 canonical entities with key fields (Tenant, Portfolio, Site, Asset, AssetState, CapabilityEnvelope, GridModel, ForecastBundle, Schedule, DispatchCommand, DispatchExecution, MarketCommitment, PluginInstance, AuditRecord) — as vocabulary, not as a database schema (schema is Phase 1/2 work).
  2. List the §2.2 initial asset support table: BESS, PV inverter, flexible C&I load, EVSE/EV fleet, site meter, wind (generic interface), thermal storage/heat pumps (generic envelope), backup generation (not a first-class 0.x target).
  3. State the §11.1 generic capability rule: capability is a time-indexed feasible operating set, not a single `max_mw` field; it may depend on P/Q, energy/SOC, mode, temperature/health, ramp, minimum up/down time, duration/recovery, site network constraints, forecast conditions, operator/customer commitments.
  4. State the provenance rule (§11.1): each asset adapter must state which constraints are measured, configured, inferred, or unknown.
  5. Record the BESS (§11.2), inverter P–Q envelope (§11.3), flexible load (§11.4), and EV fleet (§11.5) model vocabularies as definitions to be implemented in Phase 3 — not as Phase 0 implementations.
- **Deliverable / output:** Asset and capability vocabulary in the Phase 0 freeze record, with the §2.2 support table reproduced.
- **Acceptance criteria / exit check:**
  - [x] All 14 §10.1 entities are listed with key fields. — implemented, awaiting human build/test
  - [x] The §2.2 table is reproduced with "initial support" and "later extension" columns intact; backup generation is marked not a first-class 0.x target. — implemented, awaiting human build/test
  - [x] The "not a single max_mw" rule and the measured/configured/inferred/unknown rule are stated. — implemented, awaiting human build/test
  - [x] BESS/P–Q/load/EV model sections are marked "vocabulary only — implementation is Phase 3". — implemented, awaiting human build/test
- **Owner role:** Power systems
- **Dependencies:** P0.02, P0.04
- **Verification method:** Comparison against §10.1, §11.1–§11.6, §2.2.
- **Risks / failure modes:** Treating nameplate MW as flexibility (explicitly forbidden by §11.6 aggregation rule); inventing a capability field not in §10.1. Mitigation: Phase 2 registry schema review checks every field against this vocabulary.

## P0.11 — Freeze the command state machine (states, transitions, invariants)

- **Objective:** Freeze the protected command lifecycle exactly as §14 defines it, before any dispatch code exists.
- **Report trace:** §14.1–§14.4, §0.2, §5.7, §17.5
- **Inputs / prerequisites:** P0.07, P0.09.
- **Steps:**
  1. Reproduce the §14.1 state diagram as a transition table: PROPOSED → VALIDATED → FEASIBLE → APPROVED → ISSUED → ACKNOWLEDGED → TRACKING → COMPLETED, plus REJECTED, FAILED, EXPIRED, CANCELLED, SUPERSEDED, FALLBACK, with every §14.1 transition listed (including VALIDATED → SUPERSEDED, FEASIBLE → SUPERSEDED, APPROVED → EXPIRED/SUPERSEDED/CANCELLED, ISSUED → EXPIRED/SUPERSEDED/CANCELLED, ACKNOWLEDGED → EXPIRED/SUPERSEDED, TRACKING → EXPIRED/SUPERSEDED, FAILED/EXPIRED → FALLBACK).
  2. Reproduce the §14.2 state definitions verbatim in substance.
  3. State the three-gate distinction: validation, feasibility, and authorization are distinct gates; network feasibility is not operator authorization; acknowledgement confirms receipt/acceptance, not physical delivery.
  4. State the §14.3 command invariants: unique command ID; tenant/portfolio/site/asset IDs; source subsystem and plugin/model version; source schedule ID and revision; reason/action and actor/service identity; target P/Q or supported control parameter; units and sign convention; valid-from/valid-until; duration and ramp; monotonic sequence/version per asset or control channel; idempotency key; grid-model and telemetry-snapshot references; validation evidence reference; authorization decision reference (scope, limits, expiry); signature or authenticated transport identity; supersedes/superseded-by reference.
  5. State the rejection rules: reject when stale, duplicated with conflicting payloads, out of sequence, outside capability, or missing required evidence; partial fleet success is per-asset; a portfolio-level acknowledgement is not inferred from the first responding device.
  6. State the §14.4 fallback rule: no universal "set power to zero" fallback; fallback policy is per asset/site, reviewed with the equipment/site operator.
  7. State the §14.5 boundary: dispatch is not hard real-time; no primary-frequency, synthetic-inertia, relay, inverter inner-loop, or sub-cycle control claims.
- **Deliverable / output:** Command-lifecycle freeze document: transition table + state definitions + invariants + rejection rules + fallback rule.
- **Acceptance criteria / exit check:**
  - [x] All 14 states appear (8 forward + 6 terminal/fallback states). — implemented, awaiting human build/test
  - [x] Every transition in the §14.1 diagram appears in the table; no transition is added that the diagram does not show. — implemented, awaiting human build/test
  - [x] SUPERSEDED and CANCELLED are distinguished (per `.agent/WRITING.md`). — implemented, awaiting human build/test
  - [x] All §14.3 invariant fields are listed; none is dropped. — implemented, awaiting human build/test
  - [x] The three-gate distinction and the acknowledgement ≠ physical delivery rule are stated. — implemented, awaiting human build/test
  - [x] The no-universal-fallback rule is stated. — implemented, awaiting human build/test
- **Owner role:** Power systems + platform engineering
- **Dependencies:** P0.07, P0.09
- **Verification method:** Transition-table diff against §14.1 diagram (mechanical check: each diagram edge appears once); invariant checklist against §14.3.
- **Risks / failure modes:** Adding a convenient shortcut transition (e.g. PROPOSED → ISSUED) in a later phase; merging SUPERSEDED into CANCELLED; treating acknowledgement as delivery. Mitigation: Phase 4 implements this table as a tested state machine; the table is the test oracle.

## P0.12 — Freeze the grid-model contract

- **Objective:** Freeze what a grid model is, what it must declare, and what it must never be assumed to prove.
- **Report trace:** §10.1 (GridModel), §12.4, §12.5, §19.3; §25.2 D3
- **Inputs / prerequisites:** P0.05, P0.10.
- **Steps:**
  1. Define the GridModel entity: model/version, buses, branches, transformers, switches, phases, base values, limits (§10.1).
  2. State the versioning rule: feasibility checks apply candidate injections to a versioned network model (§12.5 step 3); every decision record carries the grid-model version (§3.4 / §21.3 context — cite §12.5 and §12.6).
  3. State the AC power-flow rule: AC power flow evaluates a network state; it is not AC-OPF (§12.5, §7.4 solver rule).
  4. State the SOCP-relaxation rule: a branch-flow SOCP relaxation may be useful for radial feeders but its solution may be inexact; validate relaxation conditions and verify with an independent AC power-flow engine; do not assume exactness for reverse power flow, voltage upper bounds, or arbitrary network constraints (§12.5).
  5. State the failure rule: if the validator fails or the model is stale/incomplete, the system must not infer feasibility; it rejects or holds the command per documented policy (§12.5).
  6. Record the §12.4 provider table (pandapower, GridLAB-D, OpenDSS, PyPSA, ANDES, HELICS) with each tool's recommended role and boundary — as a provider menu, not a selection (selection is Phase 4, gated on D3).
  7. Record the foundations §7.7 feasibility-check extras that the report §12.5 list does not name, as checks to carry into Phase 4 (cross-reference, not a second list here): tap/phase-shifter limits, emergency vs normal ratings, topology/model revision, measurement freshness, and operating-state consistency. Phase 4 owns the checks; this step only prevents them from being silently dropped because §12.5 does not list them.
- **Deliverable / output:** Grid-model contract in the Phase 0 freeze record.
- **Acceptance criteria / exit check:**
  - [x] GridModel fields from §10.1 are listed in full. — implemented, awaiting human build/test
  - [x] The versioned-model rule and the "stale/incomplete model is not a pass" rule are stated. — implemented, awaiting human build/test
  - [x] AC power flow ≠ AC-OPF is stated. — implemented, awaiting human build/test
  - [x] The SOCP-relaxation exactness warning is stated. — implemented, awaiting human build/test
  - [x] The foundations §7.7 extras (tap/phase-shifter limits, emergency/normal ratings, topology/model revision, measurement freshness, operating-state consistency) are recorded as Phase 4 carry-forwards. — implemented, awaiting human build/test
  - [x] No provider is selected as "the" grid provider in Phase 0. — implemented, awaiting human build/test
- **Owner role:** Power systems
- **Dependencies:** P0.05, P0.10
- **Verification method:** Comparison against §10.1, §12.4, §12.5.
- **Risks / failure modes:** Treating a power-flow run as an optimisation result; assuming SOCP exactness. Mitigation: Phase 4 validation pipeline tests encode these rules.

## P0.13 — Freeze the solver-class discipline

- **Objective:** Freeze the solver vocabulary so no later document or implementation conflates solver classes.
- **Report trace:** §7.4 (solver selection rule), §12.3, §12.6; AGENTS.md §3 rule 4
- **Inputs / prerequisites:** P0.12.
- **Steps:**
  1. Tabulate the classes: LP/QP/MIP (HiGHS — not SOCP); convex conic incl. SOCP (Clarabel — not MISOCP); CVXPY as a modelling layer whose capabilities depend on the selected solver; IPOPT local nonlinear (no global-optimality guarantee); AC power flow vs AC-OPF (different problems).
  2. State the Tier 1–4 framing from §12.3 (LP/QP/MILP; continuous SOCP; MISOCP/MINLP only when discrete decisions materially require it and a suitable solver is available; nonlinear AC-OPF as advanced research).
  3. State the provider-contract rule (§7.4): the solver provider contract must declare problem classes, supported constraint types, scaling, tolerances, warm-start support, licensing, determinism, and diagnostic outputs.
  4. State the mixed-integer conic rule: a mixed-integer conic problem needs a solver that actually supports that class, or a different linearized/MILP formulation.
  5. State the BESS binary-variable rule (§11.2): do not label a model with binary charge/discharge variables as a continuous SOCP.
- **Deliverable / output:** Solver-class discipline table in the Phase 0 freeze record.
- **Acceptance criteria / exit check:**
  - [x] HiGHS is described as LP/QP/MIP only; Clarabel as convex conic, not MISOCP; IPOPT as local only — in every Phase 0 artefact. — implemented, awaiting human build/test
  - [x] The four tiers from §12.3 are reproduced. — implemented, awaiting human build/test
  - [x] The provider-contract declaration list from §7.4 is reproduced. — implemented, awaiting human build/test
- **Owner role:** Power systems
- **Dependencies:** P0.12
- **Verification method:** Grep Phase 0 artefacts for "SOCP" claims attached to HiGHS, "MISOCP" attached to Clarabel, and "global" attached to IPOPT — zero hits required.
- **Risks / failure modes:** A later schedule or paper claiming "HiGHS SOCP" or "IPOPT global optimum". Mitigation: this table is the check used in Phase 3 (scheduler) reviews.

## P0.14 — Freeze the event vocabulary (facts, not commands)

- **Objective:** Freeze the initial domain-event vocabulary and the events-vs-commands distinction before any messaging design.
- **Report trace:** §15.3, §15.4, §6.3, §6.4 (outbox/inbox)
- **Inputs / prerequisites:** P0.08, P0.11.
- **Steps:**
  1. List the §15.3 initial events: `AssetRegistered`, `AssetCapabilityUpdated`, `TelemetryReceived`, `TelemetryQualityChanged`, `AssetAvailabilityChanged`, `FlexibilityEnvelopeUpdated`, `ForecastPublished`, `MarketSignalReceived`, `ScheduleProposed`, `ScheduleValidated`, `FeasibilityCheckCompleted`, `DispatchProposed`, `DispatchApproved`, `DispatchRejected`, `DispatchIssued`, `DispatchAcknowledged`, `DispatchTrackingUpdated`, `DispatchCompleted`, `DispatchFailed`, `DispatchExpired`, `DispatchCancelled`, `DispatchSuperseded`, `DispatchFallbackActivated`, `PluginActivated`, `PluginFailed`, `OperatorActionRecorded`.
  2. State the rule: events describe facts that occurred, not imperative commands; commands and events are separate types.
  3. State the delivery rule (§15.4, §6.3): NATS Core for ephemeral notifications where loss is acceptable; JetStream for durable work/events; JetStream is at-least-once, so consumers must be idempotent — do not promise exactly-once side effects from broker acknowledgements or deduplication options alone.
  4. State the outbox rule (§6.3, §15.5): state change + outbox record in one PostgreSQL transaction; publisher publishes to NATS and marks delivered after broker acknowledgement; consumers keep idempotency keys/inbox records.
- **Deliverable / output:** Event-vocabulary table in the Phase 0 freeze record.
- **Acceptance criteria / exit check:**
  - [x] All 26 §15.3 events are listed, spelled exactly as in the report. — implemented, awaiting human build/test
  - [x] The events-are-facts rule and the at-least-once/idempotency rule are stated. — implemented, awaiting human build/test
  - [x] The transactional-outbox four-step sequence from §6.3 is reproduced. — implemented, awaiting human build/test
  - [x] No event name is also used as a command name anywhere in Phase 0. — implemented, awaiting human build/test
- **Owner role:** Platform engineering
- **Dependencies:** P0.08, P0.11
- **Verification method:** Comparison against §15.3; count check (26 events).
- **Risks / failure modes:** Treating an event as a command (double-execution); assuming exactly-once delivery. Mitigation: Phase 1 outbox/consumer tests are built from this vocabulary.

## P0.15 — Freeze the API resource surface (candidate list, no implementation)

- **Objective:** Freeze the candidate REST resource list and API styles as the contract Phase 1 implements.
- **Report trace:** §15.1, §15.2, §10.5, §21.4 (SemVer for public contracts)
- **Inputs / prerequisites:** P0.10, P0.11, P0.14.
- **Steps:**
  1. List the §15.2 candidate resources: `/api/v1/tenants`, `/portfolios`, `/sites`, `/assets`, `/grid-models`, `/telemetry/query`, `/flexibility/envelopes`, `/forecasts`, `/schedules`, `/optimization-runs`, `/feasibility-checks`, `/dispatch-commands`, `/market-commitments`, `/plugins`, `/audit-events`, `/simulation-runs`.
  2. State the API styles (§15.1): REST/JSON externally; gRPC/Protobuf internally; NATS subjects/JetStream for durable events; WebSocket/SSE for live UI; WIT for sandboxed WASM calls.
  3. State the versioning rule: versioned REST base path (e.g. `/api/v1`); publish OpenAPI; Protobuf for cross-process contracts with Buf lint/breaking checks (§10.5); public contracts follow Semantic Versioning; the 0.x line may break contracts with a documented migration note; v1.0.0 freezes the public API and plugin contracts for the 1.x line (§21.4).
  4. State the prohibition: do not expose a generic "execute arbitrary command" endpoint (§15.2).
  5. State the schema rule: do not expose raw database schemas as public API contracts (§10.5).
- **Deliverable / output:** API-surface freeze note in the Phase 0 freeze record.
- **Acceptance criteria / exit check:**
  - [x] All 16 §15.2 resources are listed under `/api/v1`. — implemented, awaiting human build/test
  - [x] The five API styles from §15.1 are stated with their purposes. — implemented, awaiting human build/test
  - [x] The no-arbitrary-command-endpoint prohibition is stated. — implemented, awaiting human build/test
  - [x] The SemVer/migration-note rule for 0.x vs 1.x is stated. — implemented, awaiting human build/test
- **Owner role:** Platform engineering
- **Dependencies:** P0.10, P0.11, P0.14
- **Verification method:** Comparison against §15.1–§15.2, §10.5, §21.4.
- **Risks / failure modes:** An "execute" shortcut endpoint added for demo convenience; DB schema leaked as the API. Mitigation: Phase 1 API review checks every route against this list.

## P0.16 — Freeze multi-tenancy and storage principles

- **Objective:** Freeze tenancy and storage principles so Phase 1 migrations cannot bake in a cross-tenant hole.
- **Report trace:** §16.1–§16.3, §17.4 (cross-tenant leakage), §6.5
- **Inputs / prerequisites:** P0.10, P0.15.
- **Steps:**
  1. State the storage assignments (§16.1): PostgreSQL is the transactional source of truth (tenants, users/roles references, sites, assets, grid metadata, schedules, commands, approvals, plugins, outbox, audit metadata); PostgreSQL partitioned telemetry tables for initial telemetry storage and time-window queries; TimescaleDB optional, never mandatory for the open baseline; S3-compatible object storage for large artifacts; NATS JetStream is messaging, not the authoritative business database; Redis only if measurements justify it — not in v0.1.0.
  2. State the tenancy rules (§16.2): `tenant_id` is part of every tenant-owned record and query scope; application authorization plus PostgreSQL Row-Level Security (RLS) as defence in depth; the application runtime role must not own tenant tables or bypass RLS; use `FORCE ROW LEVEL SECURITY` where appropriate and test all service roles; derive tenant context from authenticated claims/authorized context, never from untrusted request-body fields; audit cross-tenant access denials; edge identities scoped to one tenant and approved site/asset set.
  3. State the RLS caveat (§16.2, [R25]): table owners typically bypass RLS policies unless configured otherwise — do not rely on a policy while running the application as the table owner.
  4. State the retention rule (§16.3): retention defined per data class (raw high-frequency telemetry, normalized telemetry, aggregated intervals, command/audit evidence, market records, simulation artifacts, model/forecast outputs); configurable by customer contract, regional market rules, and privacy/security requirements; no universal retention period in code.
  5. State the storage-strategy rule (§6.5): event sourcing selectively (append-oriented history for command lifecycle, approvals, plugin/config/security actions); current-state tables for assets, schedules, configuration, portfolio state; time-series tables for telemetry; object storage for large artifacts; never a single append-only log as the only source of truth for high-volume telemetry.
- **Deliverable / output:** Tenancy and storage principles note in the Phase 0 freeze record.
- **Acceptance criteria / exit check:**
  - [x] All six §16.1 storage rows are reproduced with roles intact. — implemented, awaiting human build/test
  - [x] All seven §16.2 tenancy rules are stated, including the RLS table-owner caveat. — implemented, awaiting human build/test
  - [x] The per-data-class retention rule is stated with no universal retention value chosen. — implemented, awaiting human build/test
  - [x] The §6.5 selective-event-sourcing rule is stated. — implemented, awaiting human build/test
- **Owner role:** Platform engineering + security
- **Dependencies:** P0.10, P0.15
- **Verification method:** Comparison against §16.1–§16.3, §6.5; Phase 1 migration review checks `tenant_id` presence per table.
- **Risks / failure modes:** Application running as table owner, silently bypassing RLS; tenant_id taken from the request body; a universal retention period hard-coded. Mitigation: Phase 1 includes an RLS test per service role (P1 ledger).

## P0.17 — Freeze the threat model and security zones

- **Objective:** Freeze the §17 threat model and zone model as the security baseline for all later phases.
- **Report trace:** §17.1–§17.5, §3.3, §18.2
- **Inputs / prerequisites:** P0.01, P0.16; NIST SP 800-82 research in `RESEARCH.md`.
- **Steps:**
  1. State the security-model reference (§17.1): NIST SP 800-82 Rev. 3 is the current final reference at the report cut-off; Rev. 4 is an initial public draft (21 September 2026, comments due 30 November 2026) and must be treated as a draft, not a final standard.
  2. Reproduce the five security zones (§17.2): enterprise IT; central application; customer integration/DMZ; OT/site control; field/device. State the flow rule: only documented flows between zones; no direct database, NATS, or arbitrary plugin network access from a field-device network.
  3. Reproduce the §17.3 required-controls list (OIDC/OAuth2, RBAC + scoped authorization, MFA/step-up, mTLS, short-lived credentials + rotation, signed commands + sequence + expiry + replay protection, plugin signature/provenance + SBOM, WASM sandbox limits + capabilities, segmentation + allowlisted paths, encrypted transport/storage, secrets-manager integration with no secrets in plugin config or source control, audit logging, secure boot/update for edge where supported, backup/restore tests + DR, vulnerability management + dependency scanning, incident response + customer notification).
  4. Reproduce the §17.4 threat table (10 threats with controls): forged/replayed command; compromised plugin; malicious/incorrect telemetry; cross-tenant leakage; OT lateral movement; broker duplication/replay; unauthorized operator action; plugin supply-chain compromise; clock drift; central outage.
  5. Reproduce the §17.5 safety invariants (10 items, protected platform code, not bypassable by plugins) — including "a missing/uncertain/stale model is not interpreted as a passing feasibility check" and "site-local safety and protection remain independent of FlexCore".
  6. State the scope limit: these are system requirements, not a certification claim.
- **Deliverable / output:** Threat-model and security-baseline document in the Phase 0 freeze record.
- **Acceptance criteria / exit check:**
  - [x] Rev. 3 final / Rev. 4 draft status is stated correctly with dates. — implemented, awaiting human build/test
  - [x] All five zones and the inter-zone flow rule are stated. — implemented, awaiting human build/test
  - [x] All §17.3 controls are listed; none is dropped. — implemented, awaiting human build/test
  - [x] All 10 §17.4 threats and all 10 §17.5 invariants are reproduced. — implemented, awaiting human build/test
  - [x] The "not a certification claim" qualifier is present. — implemented, awaiting human build/test
- **Owner role:** Security + platform engineering
- **Dependencies:** P0.01, P0.16
- **Verification method:** Comparison against §17.1–§17.5; NIST status cross-checked in `RESEARCH.md`.
- **Risks / failure modes:** Treating the Rev. 4 draft as final; a zone diagram that permits cloud-to-device access; an invariant treated as a plugin-configurable option. Mitigation: Phase 1 security tests are derived from the §17.4 table (P1 ledger).

## P0.18 — Freeze control-authority modes and the simulation-only rule

- **Objective:** Freeze the four control-authority modes and the rule that mode is a deployment/site setting, never a plugin choice.
- **Report trace:** §21.4, §0, §2.1, §22.3, §14.5
- **Inputs / prerequisites:** P0.01, P0.11, P0.17.
- **Steps:**
  1. Tabulate the four modes (§21.4): Simulation (no authority, no writes; v0.1.0, development, demonstration, benchmarking); Read-only / shadow (no authority, computes and compares proposed actions; early pilots and commissioning); Operator-approved (each command requires an authorized human approval; v1.0.0 default); Bounded automation (pre-approved policy envelope with limits, expiry, automatic fallback; post-1.0, per deployment, site, and asset class after qualification).
  2. State the mode rule: control-authority mode is a deployment and per-site setting, not something a plugin can select; a deployment may run mixed modes by site or asset class; promotion to a higher-authority mode requires the §22.3 release gates plus site-specific acceptance.
  3. State the release-ladder rule (§2.1, §0): the entire 0.x line is simulation-only; field write authority begins at v1.0.0 with operator approval; bounded automation is post-1.0 per deployment/site/asset class; a version number does not imply regulatory certification or operational qualification.
  4. State the terminology rule (`.agent/WRITING.md`): use "operator-approved control", not "manual control" or "human-in-the-loop"; use "read-only / shadow mode", not "passive mode".
- **Deliverable / output:** Control-authority freeze note in the Phase 0 freeze record.
- **Acceptance criteria / exit check:**
  - [x] All four modes are tabulated with authority, writes, and default use. — implemented, awaiting human build/test
  - [x] The mode-is-a-deployment-setting rule and the mixed-modes rule are stated. — implemented, awaiting human build/test
  - [x] The 0.x simulation-only rule and the v1.0.0 operator-approved default are stated. — implemented, awaiting human build/test
  - [x] No Phase 0 artefact promises field control in any 0.x release. — implemented, awaiting human build/test
- **Owner role:** Product + security
- **Dependencies:** P0.01, P0.11, P0.17
- **Verification method:** Comparison against §21.4, §2.1; grep Phase 0 artefacts for field-control promises in 0.x — zero hits required.
- **Risks / failure modes:** A demo or roadmap slide implying 0.x dispatch to devices; a plugin claiming to raise authority. Mitigation: this note is cited in every phase gate; Phase 4 dispatch work is simulation-only by construction.

## P0.19 — Freeze the plugin model and protected kernel

- **Objective:** Freeze the plugin architecture, execution classes, and the non-replaceable protected kernel before any plugin code is written.
- **Report trace:** §5.1–§5.8, §9.3, §22.1 ADR-005/ADR-006/ADR-010
- **Inputs / prerequisites:** P0.11, P0.17, P0.18.
- **Steps:**
  1. State the design rule (§5.1): everything that can safely vary is a plugin; anything that defines safety, authority, tenant isolation, audit integrity, or command lifecycle is a protected platform invariant.
  2. State the capability-seam rule (§5.2): contract → provider → consumer; e.g. scheduling depends on an `Optimizer` contract, not on HiGHS or Clarabel directly.
  3. List the §5.3 plugin categories (asset adapter, site integration, telemetry source, grid-model provider, optimization provider, forecast provider, market/program adapter, protocol adapter, research algorithm, analytics/report, UI extension) with permitted responsibilities.
  4. State the three execution classes (§5.4): Class A trusted built-in Rust modules; Class B sandboxed WASM/WIT components (WASI 0.2 / Component Model baseline; WASI 0.3 is not a release blocker at the cut-off — see `RESEARCH.md`); Class C out-of-process gRPC workers.
  5. State the rejection (§5.4): no arbitrary Rust dynamic-library (`.so`/`.dll`) plugin loading in the 1.x line — Rust's native ABI has no stability guarantee.
  6. State the manifest and lifecycle rules (§5.5, §5.6): validated versioned manifest; dependency resolution detects missing capabilities, version incompatibility, cycles, conflicting providers before activation; no hot-reload of safety/dispatch providers in the 1.x line; activation/upgrade must not occur mid-dispatch-transaction (drain first).
  7. State the authority model (§5.7): plugins may propose; they must not grant themselves authority; `dispatch.authorize` never granted to general plugins; `dispatch.issue` owned by the protected dispatch service under a signed authorization token; `secret.read` never granted directly; `network.any` prohibited by default.
  8. State the composition profiles (§5.8): `sim-lab`, `vpp-operator`, `site-edge`, `research-gnn`, `research-marl`; v0.1.0 ships only static built-in providers and the `sim-lab` profile; WASM host is deferred to v0.5.0.
- **Deliverable / output:** Plugin-model freeze note in the Phase 0 freeze record.
- **Acceptance criteria / exit check:**
  - [x] The protected-kernel rule names safety, authority, tenant isolation, audit integrity, and command lifecycle as non-replaceable. — implemented, awaiting human build/test
  - [x] All three execution classes and the dylib rejection are stated. — implemented, awaiting human build/test
  - [x] The §5.7 permission table is reproduced, including the never-granted rows. — implemented, awaiting human build/test
  - [x] v0.1.0 is stated as static built-in providers + `sim-lab` profile only. — implemented, awaiting human build/test
- **Owner role:** Platform engineering
- **Dependencies:** P0.11, P0.17, P0.18
- **Verification method:** Comparison against §5.1–§5.8.
- **Risks / failure modes:** A plugin designed to call device protocols directly; a dylib loader introduced for convenience. Mitigation: Phase 7 (plugin SDK) tests against this note; Phase 1 kernel boundaries reference it.

## P0.20 — Freeze the deployment and build form

- **Objective:** Freeze the deployment modes and build/distribution form so Phase 1 Compose work has a target.
- **Report trace:** §21.1–§21.4, §1.2, §8.1
- **Inputs / prerequisites:** P0.15, P0.18, P0.19.
- **Steps:**
  1. Tabulate the §21.1 hosting modes: local developer (Compose); public demo/lab (single-node/container, synthetic assets, no physical dispatch); customer private cloud (Kubernetes or customer-managed platform); customer on-premises; hybrid.
  2. State the sequencing rule: start with Docker Compose for developer reproducibility; add Helm/Kubernetes after core contracts and service lifecycle stabilize; Kubernetes is a deployment option, not a prerequisite for modular design.
  3. State the availability/recovery capabilities (§21.2): PostgreSQL backup + tested restore; NATS stream replication/retention; durable command/audit records; edge offline buffering; idempotent replay/recovery; health/readiness probes; rolling upgrades + rollback; schema migrations with compatibility windows; documented degraded modes. State the outage rule: a central outage must not disable site-local protective controls or force an unsafe device state.
  4. State the observability correlation rule (§21.3): correlate every workflow with tenant/site/portfolio/asset IDs, schedule/command IDs, plugin/solver/model versions, telemetry snapshot and grid-model version, trace ID and actor identity, validation/authorization evidence; measure ingestion delay, stale telemetry, queue depth, plugin health, solver duration/status, command delivery, acknowledgement, tracking deviation; never put secrets or unnecessary personal/customer data in logs.
  5. Reproduce the §21.4 build/distribution table (console static bundle; control plane Linux x86-64/ARM64 OCI image + native binary; WASM host from v0.5.0; Python workers pinned by lockfile; edge agent container/systemd; CLI cross-platform + containerized simulators) and the release rule (checksums, SBOM, support matrix per release; Helm chart from v1.0.0).
- **Deliverable / output:** Deployment and build-form freeze note in the Phase 0 freeze record.
- **Acceptance criteria / exit check:**
  - [x] All five §21.1 modes are listed with intended use. — implemented, awaiting human build/test
  - [x] The Compose-first / Helm-later rule is stated. — implemented, awaiting human build/test
  - [x] All §21.2 recovery capabilities and the central-outage rule are stated. — implemented, awaiting human build/test
  - [x] The §21.3 correlation list and the no-secrets-in-logs rule are stated. — implemented, awaiting human build/test
  - [x] The §21.4 distribution table is reproduced; no mobile app or single-desktop build is promised. — implemented, awaiting human build/test
- **Owner role:** Platform engineering
- **Dependencies:** P0.15, P0.18, P0.19
- **Verification method:** Comparison against §21.1–§21.4.
- **Risks / failure modes:** Kubernetes treated as a Phase 1 prerequisite; RPO/RTO values invented without customer requirements (§21.2 forbids setting them without requirements). Mitigation: Phase 1 Compose scope is checked against this note.

## P0.21 — Author ADRs ADR-001…ADR-012

- **Objective:** Write the initial ADR set (§22.1) with context, decision, alternatives, consequences, and revisit conditions for each.
- **Report trace:** §22.1, §25, `.agent/DECISIONS.md`; ADR-authoring skill in `.agent/skills/` if present
- **Inputs / prerequisites:** P0.01–P0.20 (each ADR draws on the freeze it records).
- **Steps:**
  1. For each ADR-001…ADR-012, write: context, decision, alternatives considered, consequences, revisit conditions. ADR list: 001 product is VPP/DER orchestration, not ADMS/SCADA; 002 Rust modular monolith control plane; 003 site edge as separate deployment/security boundary; 004 Python isolated to scientific/numerical/research workers; 005 plugin model with capability seams and profiles, protected safety kernel non-replaceable; 006 WASM/WIT or out-of-process workers for third-party plugins, no arbitrary Rust dylib loading; 007 PostgreSQL source of truth, NATS JetStream durable event/work transport; 008 Protobuf/gRPC internal, REST/OpenAPI external; 009 deterministic optimizer + independent feasibility validation as first operational baseline; 010 ML/RL outputs are proposals only, cannot directly dispatch; 011 market adapters region-specific, FERC Order 2222 is not a protocol; 012 no hard-real-time or protection claims.
  2. Mark each ADR's status: proposed (pending the §23 Stage 0 exit-gate review) — not "accepted" until reviewers agree.
  3. Record the ADR location convention (`docs/architecture/adr/` per §8.1 target tree) and numbering rule (next ADR is ADR-013).
  4. Cross-link each ADR to the freeze subtask(s) that supply its evidence (e.g. ADR-011 → P0.03; ADR-009 → P0.12, P0.13).
- **Deliverable / output:** Twelve ADR files (or one ADR set document with twelve sections), each with the five required parts.
- **Acceptance criteria / exit check:**
  - [x] All twelve ADRs exist, each with context, decision, alternatives, consequences, and revisit conditions. — implemented, awaiting human build/test
  - [x] No ADR claims a decision that D1–D6 leave open (e.g. ADR-011 must not name a jurisdiction). — implemented, awaiting human build/test
  - [x] Each ADR cites the report section(s) it records. — implemented, awaiting human build/test
  - [x] ADR status is "proposed" pending Stage 0 review — none marked "accepted" before the gate. — implemented, awaiting human build/test
- **Owner role:** Principal engineering (author); product, power systems, security/OT, legal (reviewers)
- **Dependencies:** P0.01–P0.20
- **Verification method:** Checklist per ADR against §22.1; `tools/verify_docs.py` if ADRs live in the report tree.
- **Risks / failure modes:** An ADR written as a decision for something D1–D6 defers; alternatives omitted, making the ADR unreviewable. Mitigation: the Stage 0 review reads alternatives explicitly.

## P0.22 — Freeze the operator-console scope (workspaces only, no UI build)

- **Objective:** Freeze the twelve console workspaces as the UI scope, with the display rules that prevent a proposed value being shown as executed.
- **Report trace:** §20.1–§20.2, §1.2, §10.4
- **Inputs / prerequisites:** P0.08, P0.11, P0.15.
- **Steps:**
  1. List the twelve §20.1 workspaces: Overview; Portfolios; Sites; Assets; Grid; Schedules; Dispatch; Market & Programs; Simulation Lab; Plugins & Integrations; Audit & Reports; Administration.
  2. State the §20.2 display rules: show operational state and data freshness prominently; distinguish measured, estimated, stale, and simulated values visually; never display a proposed setpoint as if it were an executed command; keep candidate, feasible, approved, issued, acknowledged, and measured states visually distinct; explain each rejected command with the failing constraint and evidence; show model version and timestamp for grid-feasibility outcomes; require explicit confirmation for high-impact actions; keep emergency actions and normal scheduling visually separate; provide role-specific screens.
  3. Mark the console as Phase 5 work; Phase 0 freezes scope only.
- **Deliverable / output:** Console-scope note in the Phase 0 freeze record.
- **Acceptance criteria / exit check:**
  - [x] All twelve workspaces are listed. — implemented, awaiting human build/test
  - [x] All nine §20.2 display rules are stated. — implemented, awaiting human build/test
  - [x] The note states the console is not built in Phase 0–2. — implemented, awaiting human build/test
- **Owner role:** Product
- **Dependencies:** P0.08, P0.11, P0.15
- **Verification method:** Comparison against §20.1–§20.2.
- **Risks / failure modes:** A proposed setpoint rendered as executed in a demo (the exact error §20.2 forbids). Mitigation: Phase 5 UI review checks each rule.

## P0.23 — Freeze the simulation, testing, and benchmark strategy (as a plan, not a suite)

- **Objective:** Freeze the test-layer list, simulation tracks, benchmark cases, and the no-arbitrary-targets rule, so later phases have a test strategy to implement.
- **Report trace:** §19.1–§19.4, §12.4, §24 step 10
- **Inputs / prerequisites:** P0.05, P0.12, P0.13.
- **Steps:**
  1. Tabulate the six §19.1 simulation tracks (portfolio scheduling; distribution quasi-static; transmission/planning; dynamic stability; multi-domain co-simulation; hardware-in-the-loop) with tools and the question each answers; state the tool-boundary rules (do not use a steady-state power-flow tool as evidence for transient frequency stability; HELICS coordinates federates, it does not replace their physics).
  2. List the ten §19.2 test layers: unit; property; contract; integration; deterministic scenario; fault injection; security; performance/load; shadow-mode evaluation; HIL/commissioning.
  3. List the §19.3 benchmark cases: IEEE 13/34/123-bus distribution feeders (where licensing and model availability are confirmed); IEEE 39/118-bus transmission systems for selected studies; synthetic BESS/PV/load/EVSE portfolios; multiple feeders/sites through a common substation constraint; stressed scenarios (PV ramps, forecast error, communication loss, asset dropout, reverse power flow, topology change). State the scale rule: do not claim the first distribution-feeder tests establish capability for a 10,000-bus utility network.
  4. Reproduce the §19.4 evaluation-metrics table (scheduling, asset model, grid feasibility, flexibility, dispatch, ADMM, GNN, MARL, platform).
  5. State the threshold rule (§19.4): do not set arbitrary universal targets such as "zero violations in 1,000 cases proves safety" or "ADMM converges in 30 iterations"; set acceptance thresholds after defining the model, service, benchmark, and operational risk.
- **Deliverable / output:** Simulation/testing/benchmark strategy note in the Phase 0 freeze record.
- **Acceptance criteria / exit check:**
  - [x] All six tracks, ten test layers, and benchmark cases are listed. — implemented, awaiting human build/test
  - [x] The two tool-boundary rules and the 10,000-bus scale rule are stated. — implemented, awaiting human build/test
  - [x] The metrics table is reproduced. — implemented, awaiting human build/test
  - [x] The no-arbitrary-targets rule is stated. — implemented, awaiting human build/test
- **Owner role:** Power systems + platform engineering
- **Dependencies:** P0.05, P0.12, P0.13
- **Verification method:** Comparison against §19.1–§19.4.
- **Risks / failure modes:** A benchmark result quoted as a safety proof; HIL treated as a Phase 0–2 activity. Mitigation: thresholds are set per benchmark in later phases, never in Phase 0.

## P0.24 — Define the §24 vertical slice and map its steps to stages

- **Objective:** Map the ten §24 vertical-slice steps to the §23 stages so Phase 0–2 scope is unambiguous about what the slice does NOT yet deliver.
- **Report trace:** §24, §23 Stages 0–4, §2.1
- **Inputs / prerequisites:** P0.02, P0.10–P0.15, P0.19.
- **Steps:**
  1. List the ten §24 steps: (1) create tenant/portfolio/site/grid model/four simulated assets (BESS, PV, flexible load, EVSE); (2) generate timestamped telemetry with quality flags; (3) calculate flexibility over a 24-hour horizon; (4) optimize a portfolio schedule with an LP/MILP/QP-appropriate solver; (5) validate against asset capability and a feeder power-flow model; (6) create a PROPOSED command and demonstrate validation/rejection/approval transitions; (7) send only to a simulator adapter; (8) simulate acknowledgement, tracking, partial failure, timeout; (9) display the decision/audit trail in the console; (10) replay the same scenario with the same versions and verify reproducibility.
  2. Map steps to stages: steps 1–2 land in Stage 2 (Phase 2); steps 3–4 in Stage 3; steps 5–6 in Stage 4; step 7 in Stages 4/6 (simulator adapter); step 8 in Stage 4; step 9 in Stage 5; step 10 in Stages 2/5.
  3. State the report's own caveat: the slice is the target for the 0.x line; its steps land across v0.1.0–v0.3.0; they are not all promised in the first public release (§24, final paragraph).
  4. Record which slice steps Phase 0 enables (none implemented — Phase 0 freezes the vocabulary the slice uses) and which Phase 1 enables (the foundation the slice runs on).
- **Deliverable / output:** Vertical-slice map in the Phase 0 freeze record.
- **Acceptance criteria / exit check:**
  - [x] All ten steps are listed and each is mapped to at least one §23 stage. — implemented, awaiting human build/test
  - [x] The "not all in v0.1.0" caveat is stated. — implemented, awaiting human build/test
  - [x] No step is claimed as delivered by Phase 0. — implemented, awaiting human build/test
- **Owner role:** Principal engineering
- **Dependencies:** P0.02, P0.10–P0.15, P0.19
- **Verification method:** Comparison against §24 and §23.
- **Risks / failure modes:** Presenting the slice as a v0.1.0 commitment. Mitigation: the map is re-checked at the Stage 1 and Stage 2 gates.

## P0.25 — Run the research-cut-off check and record post-cut-off changes

- **Objective:** Verify that the report's research cut-off (7 November 2026) still holds for Phase 0 sources, and record anything that changed after it.
- **Report trace:** §27 (cut-off 7 November 2026); AGENTS.md §7
- **Inputs / prerequisites:** `RESEARCH.md` (this phase).
- **Steps:**
  1. List every Phase 0 external claim and its primary source (FERC Order 2222, IEEE 1547-2018, NIST SP 800-82, WASI/Component Model status).
  2. For each, record the check date and whether the source changed after the 7 November 2026 cut-off. Known post-cut-off-sensitive items: ISO-NE Order 2222 effectiveness (1 November 2026 — before the cut-off, but the ISO-NE readiness page was last updated 4 November 2026); NIST SP 800-82 Rev. 4 comment period (open until 30 November 2026 — after the cut-off, still a draft); Rust 1.100 stable (planned 12 November 2026 — after the cut-off, not yet stable on 7 November 2026).
  3. If a source changed after the cut-off in a way that affects a freeze decision, record it as an open verification task and, if needed, propose a cut-off update per AGENTS.md §7 (update the cut-off and say what changed) — do not silently move the cut-off.
  4. Confirm the report's [R1]–[R29] citations used by Phase 0 still resolve (FERC fact sheet, IEEE 1547 page, NIST SP 800-82 pages).
- **Deliverable / output:** Cut-off check note in `RESEARCH.md` (§"Cut-off check").
- **Acceptance criteria / exit check:**
  - [x] Every Phase 0 external claim has a source + check date in `RESEARCH.md`. — implemented, awaiting human build/test
  - [x] Post-cut-off changes (if any) are listed with their effect on Phase 0 decisions, or "none affecting Phase 0" is recorded with evidence. — implemented, awaiting human build/test
  - [x] Rust 1.100 is recorded as not-yet-stable on 7 November 2026 (planned 12 November 2026). — implemented, awaiting human build/test
  - [x] No Phase 0 claim relies on a source that could not be reached without saying so. — implemented, awaiting human build/test
- **Owner role:** Principal engineering
- **Dependencies:** P0.03, P0.05, P0.13, P0.17 (sources those subtasks cite)
- **Verification method:** Re-open each cited URL; record HTTP/date outcome.
- **Risks / failure modes:** A frozen decision resting on a superseded standard (e.g. IEEE P1547 revision completing); a draft (NIST Rev. 4) cited as final. Mitigation: cut-off check repeats at each phase gate.

## P0.26 — Cross-check the freeze against the foundations specification

- **Objective:** Mechanically cross-check every frozen convention against `docs/foundations/` v1.0 §6 and record conflicts or the absence of conflicts.
- **Report trace:** AGENTS.md §2 (source-of-truth order); report "Relationship to the foundations specification"
- **Inputs / prerequisites:** P0.07–P0.13, P0.16–P0.18; foundations specification PDF/DOCX.
- **Steps:**
  1. Extract the mandatory conventions from foundations §6 (units, signs, quality states, command lifecycle, control authority, or as enumerated there).
  2. Compare each against the Phase 0 freeze (P0.07 units/sign; P0.08 quality; P0.11 command machine; P0.18 control authority).
  3. For each conflict: record it, resolve in favour of the foundations document, and file a deviation ADR only if the report is to be followed instead.
  4. For each area where the report is silent, record "report silent — no additional commitment" (per the report's own relationship statement).
  5. Record the two known representation divergences as compatible refinements in the table (neither is a conflict, and neither may be left implicit): (a) SOC representation — foundations §6.2 permits fraction in [0,1] or percent with one representation selected per API and stated explicitly, while report §10.2 mandates fraction [0,1] at the API boundary (P0.07 records the fraction-only rule as the stricter permitted selection); (b) CANCELLED/SUPERSEDED — foundations §9.1 merges them into one state-definition row, while report §14.2 distinguishes them and P0.11 follows the report (the distinction is a refinement the foundations row does not forbid).
- **Deliverable / output:** Foundations cross-check table (convention | report § | foundations § | match/conflict | resolution).
- **Acceptance criteria / exit check:**
  - [x] Every foundations §6 mandatory convention appears in the table. — implemented, awaiting human build/test
  - [x] Zero unresolved conflicts remain; each conflict has a resolution recorded. — implemented, awaiting human build/test
  - [x] The two known representation divergences (SOC per §6.2 vs §10.2; CANCELLED/SUPERSEDED per §9.1 vs §14.2) appear in the table as compatible refinements with their resolutions. — implemented, awaiting human build/test
  - [x] Silent areas are listed as silent, not filled in by assumption. — implemented, awaiting human build/test
- **Owner role:** Principal engineering
- **Dependencies:** P0.07–P0.13, P0.16–P0.18
- **Verification method:** Independent re-read of foundations §6 by a second reviewer; table reviewed at the Stage 0 gate.
- **Risks / failure modes:** The digest (`.agent/CONTEXT.md`) treated as authoritative over the foundations document — it is not (AGENTS.md §2 rank 3 vs rank 1). Mitigation: the check reads the foundations document itself, not the digest.

## P0.27 — Write the Phase 0 handover pack for Phase 1

- **Objective:** Assemble the exact inputs Phase 1 consumes, so Phase 1 starts from frozen contracts, not from re-reading the whole report.
- **Report trace:** §23 Stage 0 → Stage 1; `ledger/phase-01-rust-foundation/CONTEXT.md` ("prior-phase outputs consumed")
- **Inputs / prerequisites:** P0.01–P0.26.
- **Steps:**
  1. List the handover artefacts: scope/non-goals note (P0.01); first buyer/use cases (P0.02); D1/D2/D3 records (P0.03–P0.05); D4–D6 gate records (P0.06); units/sign contract (P0.07); quality vocabulary (P0.08); time model (P0.09); asset/capability vocabulary (P0.10); command state machine (P0.11); grid-model contract (P0.12); solver discipline (P0.13); event vocabulary (P0.14); API surface (P0.15); tenancy/storage principles (P0.16); threat model (P0.17); control-authority note (P0.18); plugin model (P0.19); deployment/build form (P0.20); ADRs 001–012 (P0.21); console scope (P0.22); test strategy (P0.23); vertical-slice map (P0.24); foundations cross-check (P0.26).
  2. For each artefact, record location, version, and status (frozen / scheduled-decision / open question).
  3. List open questions Phase 1 must not answer by itself (any D1–D6 item; any choice the freeze marks as a later-phase decision).
  4. State the Phase 1 entry condition: the Stage 0 exit gate (P0.28) has passed.
- **Deliverable / output:** Handover pack index (one page) at the end of this ledger's freeze record.
- **Acceptance criteria / exit check:**
  - [x] Every P0.01–P0.26 deliverable appears in the index with location + status. — implemented, awaiting human build/test
  - [x] Open questions are listed, with the gate that owns each. — implemented, awaiting human build/test
  - [x] The Phase 1 entry condition is stated. — implemented, awaiting human build/test
- **Owner role:** Principal engineering
- **Dependencies:** P0.01–P0.26
- **Verification method:** Phase 1 lead confirms each handover artefact is readable and versioned before Phase 1 starts.
- **Risks / failure modes:** Phase 1 starting from the report instead of the freeze (re-litigating decisions); an artefact missing from the index. Mitigation: the index is the Stage 0 gate checklist item (see `CHECKLIST.md`).

## P0.28 — Assign owners, venues, and dates for every deferred decision

- **Objective:** Satisfy the §23 Stage 0 exit-gate requirement that every §25.2 gate has an owner, a decision venue, and a date.
- **Report trace:** §23 Stage 0 (exit gate), §25.2, `.agent/DECISIONS.md`
- **Inputs / prerequisites:** P0.03–P0.06.
- **Steps:**
  1. Build the gate table: D1 (Phase 0; product + legal), D2 (Phase 0; product), D3 (Phase 0; power systems), D4 (before v0.1.0; legal + engineering), D5 (before v0.4.0; product + integration), D6 (before any read-only pilot; security).
  2. For each gate, fill in a named owner (person, not just role), a decision venue (meeting, review, or document approval), and a target date. Relative ordering only: D1–D3 dates precede the Stage 1 start; D4 precedes the v0.1.0 release; D5 precedes Stage 6; D6 precedes any read-only pilot. No fake calendar dates are asserted in this ledger — dates are assigned in the gate meeting and recorded in `.agent/DECISIONS.md`.
  3. Record the escalation rule: a gate whose date passes undecided blocks the dependent work (D1–D3 block Stage 1 start; D4 blocks v0.1.0; D5 blocks Stage 6; D6 blocks pilot promotion).
- **Deliverable / output:** Gate table in `.agent/DECISIONS.md` (updated D1–D6 rows) and the Phase 0 freeze record.
- **Acceptance criteria / exit check:**
  - [ ] All six gates have a named owner, a venue, and a target date. — NOT MET: named persons and dates are assigned at the Stage 0 gate meeting (ledger forbids inventing them); skeleton in 03_decision_records.md §7 (blocker in HANDOVER.md)
  - [x] The escalation rule is stated per gate. — implemented, awaiting human build/test
  - [ ] No gate is recorded as "TBD" at the Stage 0 exit gate. — NOT MET: date placeholders remain until the gate meeting assigns them (blocker in HANDOVER.md)
- **Owner role:** Product (coordinator)
- **Dependencies:** P0.03–P0.06
- **Verification method:** Inspect `.agent/DECISIONS.md`; gate table reviewed at the exit-gate meeting.
- **Risks / failure modes:** A gate with a role but no named person; a date that passes silently. Mitigation: the gate table is re-read at every later phase gate.

## P0.29 — Hold the Stage 0 exit-gate review

- **Objective:** Run the formal §23 Stage 0 exit gate and record its outcome.
- **Report trace:** §23 Stage 0 (exit gate), §22.3
- **Inputs / prerequisites:** P0.01–P0.28; `CHECKLIST.md` for this phase.
- **Steps:**
  1. Convene engineering, product, and power-systems reviewers (the §23 Stage 0 gate names these three groups).
  2. Walk `CHECKLIST.md` item by item; each item needs evidence, not assertion.
  3. Confirm agreement on the first workflow and the protected invariants (§23: "engineering, product, and power-systems reviewers agree on the first workflow and protected invariants").
  4. Confirm every §25.2 gate has an owner, venue, and date (P0.28).
  5. Record the outcome: pass, pass with conditions (conditions listed with owners/dates), or fail (blocking items listed). Record dissent explicitly — a dissenting reviewer is recorded, not overruled silently.
  6. If the gate passes, authorise Phase 1 to start; if not, list the blocking subtasks and re-convene.
- **Deliverable / output:** Exit-gate review record (date, attendees, checklist outcome per item, decision, conditions/dissent).
- **Acceptance criteria / exit check:**
  - [ ] All three reviewer groups were present (or sent a recorded delegate). — NOT MET: the review has not been held; see 29_exit_gate_review.md (blocker in HANDOVER.md)
  - [ ] Every `CHECKLIST.md` item has an evidence reference or a recorded exception. — NOT MET: the review has not been held; see 29_exit_gate_review.md (blocker in HANDOVER.md)
  - [ ] Agreement on the first workflow and protected invariants is recorded explicitly. — NOT MET: requires the three human reviewer groups; see 29_exit_gate_review.md (blocker in HANDOVER.md)
  - [ ] The outcome (pass / pass with conditions / fail) is recorded with conditions or blocking items. — NOT MET: outcome is recorded as NOT HELD; see 29_exit_gate_review.md (blocker in HANDOVER.md)
- **Owner role:** Principal engineering (chair); product, power systems (reviewers)
- **Dependencies:** P0.01–P0.28
- **Verification method:** The review record itself; `CHECKLIST.md` fully evidenced.
- **Risks / failure modes:** Gate passed on verbal agreement with no evidence; a reviewer group absent. Mitigation: the record requires evidence references per item.

## P0.30 — Record what Phase 0 explicitly did NOT build (anti-scope audit)

- **Objective:** Close Phase 0 with an explicit negative list, so no later phase mistakes a freeze document for an implementation.
- **Report trace:** §23 Stage 0; AGENTS.md §1 ("documentation only… do not scaffold crates 'to be ready'")
- **Inputs / prerequisites:** P0.01–P0.29.
- **Steps:**
  1. List what was not built: no Rust workspace, no crates, no API, no database, no migrations, no NATS streams, no telemetry ingestion, no registry, no simulator, no console, no plugin host, no solver integration.
  2. List what was not decided: D1–D6 where still open; no jurisdiction, market, protocol, or license decision beyond what P0.03–P0.06 record.
  3. List what was not claimed: no safety/certification claim, no protection-grade claim, no conformance claim for any protocol, no solver-capability claim beyond §7.4, no performance or scale claim.
  4. Verify the repository state: no code files added by Phase 0 work; `docs/foundations/*` untouched; the report untouched; nothing committed or pushed by this ledger work (per task instruction).
  5. File the negative list in the Phase 0 freeze record and reference it from the Phase 1 entry checklist.
- **Deliverable / output:** "What Phase 0 did not build / decide / claim" list in the freeze record.
- **Acceptance criteria / exit check:**
  - [x] The negative list covers code, decisions, and claims, each with at least the items in steps 1–3. — implemented, awaiting human build/test
  - [ ] Repository state verified: no new code files, `docs/foundations/*` untouched, report untouched, nothing committed/pushed. — NOT MET AS WRITTEN: flexcore-domain code contracts were created per the coordinator loop rules; recorded deviation in 30_anti_scope_audit.md §1
  - [x] The list is referenced from the Phase 1 handover pack (P0.27). — implemented, awaiting human build/test
- **Owner role:** Principal engineering
- **Dependencies:** P0.01–P0.29
- **Verification method:** `git status` (clean except the ledger files themselves); file listing of `ledger/phase-00-product-freeze/`.
- **Risks / failure modes:** A freeze document mistaken for an implementation in a status report ("telemetry done" when only the vocabulary is frozen). Mitigation: the negative list is the first page of the handover pack.

---

## Traceability summary

| Report section | Subtasks |
|---|---|
| §0–§2 (product, scope, releases) | P0.01, P0.02, P0.18, P0.24 |
| §10 (units, time, quality) | P0.07, P0.08, P0.09 |
| §11 (capability models) | P0.10 |
| §12 (scheduling/validation, solvers) | P0.12, P0.13 |
| §14 (command lifecycle) | P0.11 |
| §15 (APIs, events, messaging) | P0.14, P0.15 |
| §16 (storage, tenancy) | P0.16 |
| §17 (security, threat model) | P0.17 |
| §19 (simulation/testing) | P0.23 |
| §20 (operator console) | P0.22 |
| §21 (deployment, control authority) | P0.18, P0.20 |
| §22.1 (ADRs) | P0.21 |
| §23 Stage 0 (exit gate) | P0.28, P0.29, P0.30 |
| §24 (vertical slice) | P0.24 |
| §25.2 (D1–D6) | P0.03–P0.06, P0.28 |
| §27 (research cut-off) | P0.25 |
| Foundations v1.0 §6 | P0.26 |

Subtask count: **30** (P0.01–P0.30).
