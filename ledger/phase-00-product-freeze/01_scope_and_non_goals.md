# P0.01 — FlexCore v0.1.0 scope and non-goals

| Field | Value |
|---|---|
| Subtask | P0.01 — Freeze product scope and non-goals in a single signed-off scope note |
| Report trace | §0, §0.1, §0.3, §1.2, §1.3, §1.6, §2.1, §2.3, §25; Document control ("Intended reviewers") |
| Source | `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` (product baseline; foundations specification governs on conflict per AGENTS.md §2) |
| Status | Draft for Stage 0 review — no reviewer has signed yet |
| Decisions | D1–D6 remain OPEN (§25.2); this note decides none of them |

---

## 1. What FlexCore is

Per report §0 and §25: **FlexCore is an enterprise web application backed by a central control plane, with optional site-level edge connectors and a simulation/research environment.** It is an open-source, modular Virtual Power Plant (VPP) orchestration and distribution-aware Distributed Energy Resource (DER) coordination platform (§0, §1.1).

Per §1.2 it is a multi-surface enterprise software product: browser-based Operator Console, Rust control plane, Linux site edge agent, isolated Python numerical/research workers, plugin SDK, Simulation Lab, and an external REST/OpenAPI API. The platform is not a mobile app and not a single desktop binary (§1.2).

Explicitly (§0, §25): FlexCore is

- **not a mobile app** (and not a single desktop binary, §1.2);
- **not an ADMS/SCADA replacement**;
- **not a protective relay controller**;
- **not an ISO/RTO market platform**; and
- **not a hard-real-time controller** (§14.5; §0.3).

FlexCore is not certified, is not a safety case, and is not permission for live grid control. The entire 0.x line is simulation-only; field write authority begins at v1.0.0 with operator approval (§0, §2.1). A version number does not imply regulatory certification or operational qualification (§2.1).

## 2. What is deliberately not promised (§0.3 — verbatim, no paraphrase)

> The first public release and the v1.0.0 pilot baseline do not promise:
>
> - hard real-time or protection-grade control;
> - primary frequency response or synthetic inertia control;
> - autonomous grid switching or protection-system operation;
> - universal AC-OPF global optimality;
> - universal market qualification or FERC Order 2222 compliance;
> - certified OpenADR, IEEE 2030.5, IEC 61850, SunSpec, or OCPP conformance merely by implementing an adapter;
> - safe field dispatch without site-specific integration, commissioning, cyber review, and hardware-in-the-loop validation; or
> - that GNN or MARL outputs are safe by construction.
>
> These are separate qualification and research tracks.

This list is the scope-creep test for every later phase gate: no later document, demo, or plugin claim may reintroduce any item above as a FlexCore capability.

## 3. Audiences (§0 and §1.3)

### 3.1 Primary audiences for the first product generation

§0 names four target audiences:

> 1. VPP and DER aggregators;
> 2. commercial and industrial (C&I) energy portfolio operators;
> 3. microgrid and multi-site energy operators; and
> 4. energy service companies and system integrators.

§1.3 ("Primary audience for the first product generation") enumerates the same set as five table roles:

| Role / organization (§1.3) | Problem FlexCore addresses (§1.3) |
|---|---|
| VPP / DER aggregator operator | Need to understand fleet capacity, available flexibility, operating limits, and dispatch performance across many sites |
| C&I portfolio operator | Need to coordinate batteries, PV, flexible loads, and EV charging across multiple facilities |
| Microgrid operator | Need coordinated scheduling of local generation, storage, and controllable loads while maintaining site constraints |
| Energy service company (ESCO) | Need a reusable orchestration layer across customer sites and equipment vendors |
| Energy systems integrator | Need extensible interfaces and a common platform to connect customer assets, telemetry, and optimization components |

Count note for reviewers: §0 groups ESCOs and system integrators into one audience (four audiences); §1.3 lists them as two table rows (five roles). The content is the same set; the count difference is presentational. The ledger's "four primary audiences" matches §0's enumeration.

§0 additionally states: utilities and distribution system operators (DSOs) are an important later audience, but FlexCore should not initially claim to be a fully qualified utility DERMS or a replacement for an ADMS. Direct ISO/RTO market participation is a region-specific integration, not a universal core function.

### 3.2 Secondary audience (§1.3, verbatim)

> - DSO / utility planning and DER integration teams;
> - grid engineers and power-system researchers;
> - market operations and energy trading teams;
> - DER device and EMS vendors;
> - universities and open-source contributors; and
> - regional market-interface implementers.

### 3.3 Not the initial target (§1.3, verbatim — five groups)

> - residential consumers managing one home;
> - individual solar installers looking only for proposal software;
> - ISOs/RTOs seeking a replacement for their market or control-room platforms;
> - TSOs seeking a replacement for EMS/SCADA or transmission control; and
> - operators seeking a certified protection, relay, or inverter inner-loop controller.

## 4. Responsibility boundary (§1.6 — in full, verbatim)

> FlexCore is supervisory orchestration software. The site EMS, inverter firmware, BMS, EVSE controller, protection relays, and certified local control systems remain responsible for device-specific safety and fast local control.
>
> FlexCore may request an operating point. It must not assume the device can or will execute it. The edge/site controller and the equipment must enforce local limits independently.
>
> The platform does not replace:
>
> - protective relays or inverter protection;
> - BMS cell-level protection;
> - local droop, grid-forming, or inner inverter control;
> - site PLC / DCS / SCADA safety interlocks;
> - utility ADMS/EMS/SCADA; or
> - market operator systems.

## 5. Release ladder (§2.1 — reproduced with the control-status column intact)

| Release | Product capability | Control status |
|---|---|---|
| **v0.1.0 — FlexCore Lab (first public release)** | Canonical asset/site models, simulator integration, telemetry ingestion, asset registry, portfolio views, scenario replay | Simulation only; no device writes |
| **v0.2.0 — Flexibility & Scheduling** | BESS/PV/load/EVSE models, flexibility envelopes, deterministic rolling-horizon scheduling, schedule comparisons | Simulation only |
| **v0.3.0 — Dispatch Assurance** | Resource validation, network feasibility checks, command lifecycle, operator approvals, audit trail | Simulation and shadow mode |
| **v0.4.0 — Site Integration** | First site adapter through OpenEMS; selected telemetry/protocol adapters; outbound edge connection | Read-only or test-lab control by default |
| **v0.5.0 — Plugin SDK** | Versioned plugin contracts, plugin manifest, trusted built-in modules, constrained WASM plugin host, examples | Simulation/shadow; no plugin can bypass dispatch gates |
| **v1.0.0 — Pilot-ready FlexCore** | Stable operator workflow, reproducible benchmark, deployment/upgrade model, security review, site integration test harness | Operator-approved control is the default; production dispatch only after customer-specific acceptance and commissioning |
| **v1.x — Advanced coordination** | DMPC/ADMM, expanded market adapters, topology-aware optimization research, portfolio analytics | Per-feature qualification; bounded automation may be granted per deployment |
| **v2.x — Advanced grid intelligence** | GNN acceleration, safe MARL research, dynamic stability research, broader utility/DSO integrations | Shadow/simulation first; no automatic field authority by default |

Version labels are product releases, not regulatory certifications (§2.1). The 0.x line is simulation-only throughout; the control-authority mode enabled at a site is governed by §21.4 and the release gates in §22.3. Protocol posture: v0.1.0 ships simulation adapters only, and implementing an adapter is not a conformance claim (§2.3).

## 6. Scope statement for sign-off

For v0.1.0, FlexCore delivers exactly what the §2.1 release ladder names for that release: canonical asset/site models, simulator integration, telemetry ingestion, asset registry, portfolio views, and scenario replay — simulation only, no device writes. Anything beyond that scope in a v0.1.0 claim, demo, or plan is scope creep and must be rejected by reference to this note, the §0.3 non-promises (§2 above), and the §1.6 responsibility boundary (§4 above).

## 7. Reviewer sign-off (Document control, "Intended reviewers")

No reviewer has signed yet. Sign-off (or a recorded objection) from at least one reviewer in each group below is a P0.01 acceptance criterion.

| Reviewer group | Name | Sign-off / objection | Date |
|---|---|---|---|
| Product | ______________________ | ☐ Signed off  ☐ Objection recorded: ______________________ | ____________ |
| Power systems | ______________________ | ☐ Signed off  ☐ Objection recorded: ______________________ | ____________ |
| Platform engineering | ______________________ | ☐ Signed off  ☐ Objection recorded: ______________________ | ____________ |
| Security/OT | ______________________ | ☐ Signed off  ☐ Objection recorded: ______________________ | ____________ |
| Legal | ______________________ | ☐ Signed off  ☐ Objection recorded: ______________________ | ____________ |
