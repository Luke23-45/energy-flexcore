# Phase 14 — Protocol and Device Connectivity Expansion — Context

Date: 2026-11-14. Status: **NOT STARTED** (0/24 — see `ledger_phase_14.md`).
This directory is PLANNING ONLY: no code, crates, or manifests are created by
this program's planning step (program conventions §3). Citations `[S-n]`
resolve in `../00_GAP_ANALYSIS.md` §11; the rules in
`../01_PROGRAM_CONVENTIONS.md` bind everything below.

## 1. Why this phase exists

Two rows of the commercial table-stakes analysis (gap analysis §3) are the
business case, and the standards-obligations table (gap analysis §4) is the
compliance case:

| # | Table-stakes capability | Evidence | FlexCore today |
|---|---|---|---|
| 1 | Multi-OEM, multi-asset-class connectivity + enrollment | Uplight "40+ OEMs, 10+ protocols" [S-43]; Enode "100+ brands, 1,000+ models" [S-45]; EnergyHub partner ecosystem [S-42] | OpenEMS REST adapter only (UNVERIFIED A1–A8); generic-rest reserved, never implemented |
| 11 | Interoperability standards adherence (OpenADR / IEEE 2030.5) | GE Gateway "certified to IEEE 2030.5" [S-41]; OpenADR Alliance DER framing [S-10] | Zero protocol stacks; only OpenEMS REST + HELICS co-sim |

Every credible enterprise VPP platform ships protocol connectivity that
FlexCore entirely lacks. The standards obligations (gap analysis §4) say the
same thing as program rules and regulation rather than competition:

| Obligation | What support means | Sources |
|---|---|---|
| IEEE 2030.5 server (CSIP) — the CA Rule 21 / HI 14H default aggregator interface | AggregatorServer resource tree (FunctionSetAssignments, DERProgram/DefaultDERControl/DERControlList, Response, Pricing, LogEvent), mTLS with CA-chain validation, certificate lifecycle | [S-1][S-3][S-5]; security [S-7][S-8][S-9] |
| OpenADR VTN — 3.0 REST/JSON current; 2.0b legacy XML | Event/report/registration endpoints, polling + push, VEN management, certification path | [S-10][S-11] |
| SunSpec Modbus client | Information models for inverters/meters/batteries over Modbus TCP/RTU | [S-16] |
| DNP3 / IEEE 1815.2-2025 DER profile | Master/outstation roles, DER data objects, secure transport | [S-14][S-15] |
| IEC 60870-5-104 client (EU DSO telecontrol) | 104 client plus compensating security (no native encryption) | Track B (CESI via the [S-14] analogue) |
| IEC 61850-7-420 MMS client | DER data-model awareness, SCL/ICD ingestion at the DSO boundary | [S-12][S-13] |
| CIM/CGMES + IEC 61968-5 DER groups | Network-model import; DER group management payloads | [S-17][S-18] |
| IEEE 1547-2018 settings profiles | Function catalog, jurisdiction default tables, read/write via 2030.5/Modbus; aggregator respects utility override rights (Clause 10) | [S-19][S-20] |
| MQTT 5 + Sparkplug B edge transport | Birth/death state management for DER fleets; OpenFMB validates NATS at the grid edge | [S-39][S-40] |
| PKI operations (2030.5 CA hierarchy) | Trust anchors, issuance, revocation, renewal | [S-8][S-9] |

## 2. What exists today (the real seams)

All paths below exist in the Phase 0–10 workspace, which is WRITTEN-UNRUN —
written modules with records, not verified behavior. These are the patterns
the new adapters must follow, not reinvent.

| Seam | Path | What it gives Phase 14 |
|---|---|---|
| EmsAdapter port | `crates/edge-agent/src/adapters/port.rs` | The protocol-shaped-not-vendor-shaped port: points, native readings, declared capabilities (sign convention, quality mapping, cadence, clock quality), honest health — and deliberately NO write path ("absence is structural") |
| Anti-corruption layer | `crates/edge-agent/src/adapters/acl.rs` | EMS-native → canonical mapping in one reviewable place; the translation-at-boundary pattern every new adapter repeats |
| Adapter registry | `crates/edge-agent/src/adapters/registry.rs` | The allowlist: `openems-rest` AVAILABLE, `generic-rest` a RESERVED placeholder (never implemented, never constructible), `simulated-test` TEST-ONLY; RESERVED ids get a named refusal |
| OpenEMS adapter | `crates/edge-agent/src/adapters/openems.rs` | The UNVERIFIED-ASSUMPTION register (gap-analysis shorthand A1–A8), the versioned mapping profile, and the MAPPED / EXPLICITLY_UNMAPPED / UNMAPPED / AMBIGUOUS classification — never mapped by guessing |
| Simulated adapter | `crates/edge-agent/src/adapters/simulated.rs` | The TEST-ONLY scenario fixture — the simulator-first testing policy in existing code |
| Scope declaration | `crates/flexcore-grid-api/src/scope.rs` | `ScopeDeclaration`: declared never inferred, mandatory non-empty `not_modelled`, `assert_supports` refuses out-of-scope requests — THE model each protocol adapter's scope declaration mirrors |
| Kernel permission model | `crates/flexcore-kernel/src/manifest.rs` | `NEVER_GRANTED_PERMISSIONS` (`dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any`) rejected at the manifest gate — the permission model P14.16 extends without touching |
| Port traits | `crates/flexcore-ports/src/` | `telemetry_source` (with `QualityMappingDeclaration`), `repository`, `telemetry_ingest`, `tenant_context_source` — the hexagonal port pattern new protocol ports follow |
| Domain capability models | `crates/flexcore-domain/src/inverter_pq_model.rs`, `bess_model.rs`, `ev_fleet_model.rs` | The pure-rules envelopes (P-Q circle, BESS net power, EV fleet) that protocol readings and controls must map INTO — adapters never define parallel models |
| Simulator profiles | `crates/flexcore-simulator/src/profiles.rs` | Synthetic per-tick profiles for the four exit-gate asset types plus the site meter — the base P14.17 extends |
| Unit contract | `crates/flexcore-contracts/src/units.rs` | Canonical MW/MVAr/MVA/MWh, P > 0 export, reactive sign declared per profile (never inferred), SOC fraction — adapter boundary conversions |
| ADR-010 / ADR-011 | `docs/architecture/adr/ADR-010.md`, `ADR-011.md` | ADR-010: outputs enter only as proposals through the protected gates (extended here to protocol adapters); ADR-011: adapter selection is region/standard-specific, never a universal mapping |

## 3. Constraints (critical)

1. **Adapters are READ/monitor + command-relay only.** Commands reach devices
   only through the 14-state command lifecycle with all three gates
   (validation, feasibility, authorization) distinct. No adapter may hold
   dispatch authority — this is ADR-010 extended: a protocol adapter, like an
   ML output, enters the platform as a transport, never as an authorizer.
   External control objects (2030.5 DERControl, OpenADR events) enter as
   PROPOSED lifecycle commands, never as issued setpoints.
2. **The never-granted set is byte-identical.** `dispatch.authorize`,
   `dispatch.issue`, `secret.read`, `network.any` stay exactly as frozen in
   `crates/flexcore-kernel/src/manifest.rs` (conventions §6.4); protocol
   adapters receive dispatch reach only through the command lifecycle.
3. **Solver-class discipline is unaffected.** Nothing in this phase adds a
   solver problem class; DOE envelopes (P14.15) enter the optimizer as
   constraint data only.
4. **Every adapter gets a scope declaration and a capability manifest**
   mirroring the grid-api pattern (`crates/flexcore-grid-api/src/scope.rs`):
   declared never inferred, mandatory non-empty `not_modelled`, fail-closed
   refusal outside scope — plus the kernel capability-manifest discipline.
5. **Certification is a human-gated lab process.** CSIP, OpenADR, DNP3
   conformance is certified by test labs and alliances; plans prepare test
   suites and evidence, never claim conformance (conventions §7.3).
6. **New adapter crates are additive workspace members** passing
   `python -I tools/check_dependency_rule.py` after every batch (conventions
   §6.1–6.2); the Phase 0–10 base is WRITTEN-UNRUN, so the first human build
   remains the program gate (program README, boundary 1).

## 4. Glossary

| Term | Meaning as used in this phase |
|---|---|
| VTN / VEN | Virtual Top Node / Virtual End Node — the OpenADR server and client roles; the VTN publishes events and collects reports, VENs poll or receive push [S-10] |
| AggregatorServer | The IEEE 2030.5 server role an aggregator operates; DER devices connect to it as TLS clients [S-1][S-5] |
| CSIP resource tree | The IEEE 2030.5 resource hierarchy CSIP requires of an aggregator: FunctionSetAssignments, DERProgram (DefaultDERControl, DERControlList), Response, Pricing, LogEvent [S-3] |
| DERControl | An IEEE 2030.5 control resource carrying setpoints, timing, and targets for one or more DERs — the shape external DR schedules arrive in |
| Function set assignment | The 2030.5 mechanism binding function-set instances (control, reading, pricing) to a device or group [S-1] |
| SunSpec information model | SunSpec's standardized Modbus register blocks (e.g., inverter models 101/103, meter 201/203, battery 80x models) [S-16] |
| Outstation vs master | DNP3 roles: the outstation responds to the master (control-center) polling; a VPP presenting its fleet to utility SCADA acts as an outstation [S-14] |
| MMS / GOOSE / SV | Three IEC 61850 communication profiles: MMS (client/server reporting and control), GOOSE (fast peer-to-peer events), SV (sampled values). GOOSE and SV are protection-grade — out of scope per foundations §0.3 |
| CIM profile | A subset of IEC Common Information Model classes selected for one exchange purpose; CGMES is the ENTSO-E grid-model-exchange profile [S-17] |
| SCL / ICD | IEC 61850 System Configuration language; an ICD (IED Capability Description) file describes one device's data model and is ingested, not handwritten |
| Sparkplug BIRTH/DEATH | Sparkplug state management over MQTT: a BIRTH message declares the full metric set on (re)connection; a DEATH declares disconnection [S-39][S-40] |
| DOE | Dynamic operating envelope — an operator-issued import/export limit at a connection point, with issue time, expiry, and update cadence [S-158][S-159] |
| Primacy / event prioritization | The CSIP rule set for resolving conflicting controls from multiple authorities (utility vs aggregator), including the utility-override path; production practice documented by SunSpec [S-156] |

## 5. Boundaries — what this phase does NOT do

1. **No conformance, certification, or compliance claim of any kind** —
   QualityLogic/CSIP, OpenADR Alliance, DNP Users Group certification is a
   human-gated lab process; P14.19 prepares evidence only (conventions §7.3).
2. **No GOOSE or SV anything** — protection-grade per foundations §0.3;
   P14.10 is MMS client design only.
3. **No EV charging protocols** — OCPP and ISO 15118 are Phase 17 (P14.17
   names the simulator seam OCPP will land on, and builds nothing for it).
4. **No field write authority change** — the release ladder, ADR-010, the
   never-granted set, and the simulation/shadow ceiling are untouched.
5. **No market or tariff adapters** — ADR-011 boundary; Phase 16 scope.
6. **No PKI service deployment** — CA operations design (P14.03) ties to the
   Phase 13 secrets surface and Phase 18 device identity; no CA is stood up
   in this phase.
