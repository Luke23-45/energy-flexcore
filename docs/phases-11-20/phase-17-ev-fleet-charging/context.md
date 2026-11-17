# Phase 17 — EV Fleet and Charging Integration — Context

Date: 2026-11-14. Status: **NOT STARTED** (0/18 — see `ledger_phase_17.md`).
This directory is PLANNING ONLY: no code, crates, or manifests are created by
this program's planning step (program conventions §3). Citations `[S-n]`
resolve in `../00_GAP_ANALYSIS.md` §11; the rules in
`../01_PROGRAM_CONVENTIONS.md` bind everything below.

## 1. Why this phase exists

EV is already a first-class asset class in the domain model — the P3.11 fleet
model (`crates/flexcore-domain/src/ev_fleet_model.rs`) carries EVSE ratings,
connection windows, hard departure energy, opt-out, and a gated bidirectional
capability — yet the workspace contains no protocol adapter for any charging
station: no OCPP stack, no CSMS role, no charging-profile path anywhere. The
EV build-list (gap analysis §7, Track F1) is what closes that gap:

| Why it matters | Sources |
|---|---|
| OCPP CSMS is the foundational integration surface: the server role charge points connect to (WebSocket OCPP-J framing, functional blocks, security profiles) | [S-121][S-123][S-125] |
| Smart-charging profile composition (purposes, stack levels, composite schedules) is the actual dispatch path into chargers | [S-124][S-141] |
| Dual-source state of charge (charger MeterValues vs OEM telematics) needs explicit staleness rules; Tesla 500 ms telemetry is the outlier, telematics go stale when the vehicle sleeps | [S-137][S-138] |
| Depot load management runs under site import/transformer limits with simultaneity-factor control across chargers | [S-141][S-140] |
| ISO 15118 Plug&Charge certificate management is phase-gated: PKI operations are the last commercial barrier | [S-128][S-130] |
| V2G hooks: OCPP 2.1 bidirectional/DER blocks as design records; the North-American AC path (IEEE 2030.5 V2G-AC / SAE J3072 / UL 1741 SC) is a SEPARATE future integration, not this phase | [S-4][S-131][S-144] |
| AFIR/Eichrecht data hooks (signed MeterValues) keep the EU metrology surface addressable without claiming compliance | [S-135][S-136] |
| OCPI/OICP roaming is explicitly out of scope for a depot-fleet VPP | [S-134] |

**D5 interplay (said once, exactly).** The `ledger/MASTER_LEDGER.md` row D5
("First protocol and asset mix") is OPEN, with the note that P6.01 must name
the OCPP *edition* as well as the version. Phase 17's OCPP work is the
CANDIDATE D5 would close — it builds the adapter design, the message
mappings, and the evidence D5 needs. It is NOT a closure of D5; D-decisions
are human-closed.

## 2. What exists today (the real seams)

All paths below exist in the Phase 0–10 workspace, which is WRITTEN-UNRUN —
written modules with records, not verified behavior. One honesty note up
front: V2G capability exists in the DOMAIN model (the five-condition gate),
not in the simulator profile — the EVSE simulator profile is charging-only
("no V2G in Phase 2" is recorded in its module header).

| Seam | Path | What it gives Phase 17 |
|---|---|---|
| EV fleet capability model | `crates/flexcore-domain/src/ev_fleet_model.rs` | The P3.11 pure-rules model: EVSE rated power + phase, per-vehicle connection windows, the HARD departure-energy policy, min/max charging power, ramp, priority, start-inclusive opt-out, site transformer limit, and the five-condition V2X gate (`V2xConfig`: EV, EVSE, protocol, interconnection agreement, market rules — each evidenced). Charging schedules map INTO this model, never around it. It already carries `OCPP_SCOPE_STATEMENT`: OCPP 2.1 is cited for capability scope only, never device conformance |
| EVSE simulator profile | `crates/flexcore-simulator/src/profiles.rs` | The synthetic charging-only profile: declared connection windows (midnight wrap allowed), one aggregate EV, constant efficiency, explicitly no V2G. P17.15 extends it — V2G behavior must be ADDED there, honestly labeled synthetic |
| Scenario EVSE parameters | `crates/flexcore-simulator/src/scenario.rs` | `EvseParams` (connection windows, charging power, initial SOC, SOC target) — the scenario surface the simulated charge points extend |
| Adapter port pattern | `crates/edge-agent/src/adapters/port.rs` | The protocol-shaped-not-vendor-shaped `EmsAdapter`: native readings, REQUIRED capability declarations (quality mapping, cadence, clock quality), honest health, deliberately no write path — and the recorded note "one adapter per D5 selection … D5 is OPEN" |
| Anti-corruption layer | `crates/edge-agent/src/adapters/acl.rs` | Native → canonical mapping in one reviewable place; unknown units, undeclared quality codes, and non-UTC timestamps are named errors, never defaults |
| Adapter registry | `crates/edge-agent/src/adapters/registry.rs` | The allowlist with named refusals for RESERVED ids — the pattern the CSMS connection registration follows |
| Port traits | `crates/flexcore-ports/src/` | `telemetry_source` (with `QualityMappingDeclaration`), `telemetry_ingest`, `telemetry_read`, `repository`, `event_outbox`; the sync-trait and no-vendor-types rules the CSMS port must keep |
| Ingestion pipeline | `apps/worker/src/ingestion.rs` | The 7-stage receive → validate → normalize → quality-assign → dedup → store → publish pipeline with quarantine, hold-or-lower quality refinement, and outbox events — MeterValues and SoC ingestion lands here |
| Quality vocabulary | `crates/flexcore-contracts/src/quality.rs` | The frozen six-state `TelemetryQuality` (GOOD, SUSPECT, BAD, MISSING, ESTIMATED, STALE) with the non-silencing rule — exactly one state per value, no seventh state |
| Command lifecycle | `crates/flexcore-domain/src/command_state.rs`, `command_lifecycle.rs` | The 14-state lifecycle with validation, feasibility, and authorization as distinct gates — the ONLY path a charging schedule travels to a charger |
| Scope-declaration pattern | `crates/flexcore-grid-api/src/scope.rs` | Declared-never-inferred scope with a mandatory non-empty `not_modelled` list — the CSMS adapter's scope declaration mirrors this |
| Kernel permission model | `crates/flexcore-kernel/src/manifest.rs` | The never-granted set (`dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any`) — byte-identical before and after this phase |
| OCPP/15118 research | `ledger/phase-06-edge-protocol/RESEARCH.md` §3 | OCPP version facts: 2.0.1 released 2020 (approved as IEC 63584:2024); 2.1 released January 2025, backward compatible, adds bidirectional power transfer with ISO 15118-20, a DER control block, battery swapping, and new payment options; 2.0.1 is NOT backward compatible with 1.6; "Edition 2" publication details are SECONDARY and carry an open verification task |
| Master ledger D5 row | `ledger/MASTER_LEDGER.md` | D5 OPEN — "First protocol and asset mix", "P6.01 (must name OCPP *edition* as well as version); P9.04" |

## 3. Constraints (critical)

1. **Command-relay-only.** Charging schedules that curtail or set charge
   rates are COMMANDS. A `SetChargingProfile` payload exists only downstream
   of the 14-state lifecycle with its three distinct gates (validation,
   feasibility, authorization); acknowledgement is not physical delivery.
   User-override precedence (driver manual starts over VPP curtailment) is a
   recorded dispatch POLICY resolved before issuance (P17.10) — never an
   adapter behavior. The never-granted set stays byte-identical; adapters get
   dispatch reach only through the command lifecycle (conventions §6.4).
2. **The CSMS is the server role.** Charge points connect to FlexCore, not
   the reverse. Where the CSMS adapter lives is an edge/cloud boundary
   decision (P17-E-2): control-plane-hosted vs edge-agent-hosted. The
   recorded DEFAULT is edge-agent-hosted for depot sites — offline tolerance
   matters and the edge agent's store-and-forward buffer is reused. The
   default is implemented only because P17.02 explicitly says so; agents
   never close the E-decision (conventions §4).
3. **Certification is a human-gated lab process.** OCA/DEKRA-class OCPP
   conformance, ISO 15118 qualification, and any metrology approval happen in
   labs with fees and signed results. Test suites are written-unrun;
   conformance is never claimed (conventions §7.3).
4. **Telematics integrations are vendor APIs with their own terms.** Adapter
   seams are specified; no vendor is promised and no terms of service are
   accepted by any planning document (P17-E-3; terms acceptance is a human
   gate input, conventions §5).
5. **Safety: charging control never overrides vehicle/BMS protection
   limits.** Physical charging limits (IEC 61851 charging modes [S-142]) are
   device-level truths the platform must respect, never command around.
   FlexCore remains supervisory — no inverter inner-loop control, no
   sub-cycle control, no protection-grade anything (foundations §0.3).
6. **New crates are additive workspace members** keeping the dependency rule
   green after every batch (conventions §6.1–6.2); the Phase 0–10 base is
   WRITTEN-UNRUN, so the first human build remains the program gate (program
   README, hard status boundary 1).

## 4. Glossary

| Term | Meaning as used in this phase |
|---|---|
| CSMS | Charging Station Management System — the server role OCPP defines; charge points connect TO the CSMS, and FlexCore operates this role [S-121] |
| Charge point / EVSE | The charging station and its individually addressable outlets; smart-charging payloads address per-EVSE ids (evseId 0 = the whole station) [S-124] |
| OCPP-J | OCPP over WebSocket with JSON framing — the CALL / CALLRESULT / CALLERROR message types and the WebSocket subprotocol negotiation [S-121] |
| Functional blocks A–P | OCPP's top-level feature grouping (Security, Provisioning, Authorization, Transactions, …, Plug&Charge, …); 2.1 adds bidirectional-power and DER-control blocks — exact letters cited from secondary sources, UNVERIFIED until checked against the OCA text [S-125] |
| Security profiles 1–3 | OCPP transport security levels: 1 unsecured transport, 2 TLS with HTTP basic auth, 3 TLS with client certificates; Profile 3 is the onboarding target [S-126] |
| Charging profile purposes | The four purposes: ChargingStationExternalConstraints, ChargingStationMaxProfile, TxDefaultProfile, TxProfile — each with defined composition precedence [S-124] |
| Stack level | The numeric precedence inside one purpose; higher stack levels override lower ones in the composite [S-124] |
| Composite schedule | The charge point's merged schedule across all installed profiles over a period; `GetCompositeSchedule` returns it — the composer's reconciliation surface [S-124] |
| minChargingRate | The minimum rate a charging profile permits; a schedule step below it must be 0 (off) — an unfulfillable small rate is not a valid alternative [S-124] |
| Plug&Charge (PnC) | ISO 15118 authorization using the vehicle's contract certificate, so charging is authenticated without driver interaction [S-128][S-130] |
| V2G Root CA | The trust anchor of the ISO 15118 PKI hierarchy that vehicle-OEM and charging-operator certificates chain to [S-128] |
| Contract certificate | The per-vehicle certificate presented for PnC; its installation and renewal run over OCPP certificate messages (2.1 flows — block letters UNVERIFIED) [S-125] |
| MeterValuesSign | OCPP 2.1's signed meter values — the data hook that keeps Eichrecht/AFIR-caliber metrological integrity addressable [S-135][S-136] |
| plug-state | Whether a vehicle is physically connected to an EVSE — the participation ground truth: a disconnected vehicle dispatches nothing, whatever telematics say [S-139][S-140] |
| Simultaneity factor | The fraction of connected chargers assumed to charge at full rate together; depot load management shapes per-charger limits so the site import stays under its limit [S-140][S-141] |
| Departure-time constraint | Per-vehicle "energy required by departure time" — already a HARD constraint in the domain model; feeds the composer from vehicle calendars and telematics |

## 5. Boundaries — what this phase does NOT do

1. **No OCPI/OICP roaming** — recorded with revisit conditions in P17.17
   (public CPO network operation is the trigger) [S-134].
2. **No PnC/PKI implementation** — P17.12 is a design record, gated on PKI
   operations maturity (P14.03 interplay: no CA is stood up anywhere in this
   program).
3. **No North-American V2G-AC integration** — IEEE 2030.5 V2G-AC / SAE J3072
   / UL 1741 SC is recorded (P17.13) as a SEPARATE future integration
   [S-4][S-131][S-144].
4. **No conformance, certification, metrology, or safety claim of any kind.**
5. **No field write authority change** — the release ladder, ADR-010, the
   never-granted set, and the simulation/shadow ceiling are untouched.
6. **No telematics vendor promised and D5 not closed** — P17-E-3 stays OPEN
   with the vendor-agnostic-port default; D5 stays human-closed.
