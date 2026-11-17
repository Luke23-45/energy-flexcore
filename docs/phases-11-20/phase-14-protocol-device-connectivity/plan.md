# Phase 14 — Protocol and Device Connectivity Expansion — Plan

Date: 2026-11-14. Status: **NOT STARTED** (0/24). Planning baseline only.
Ledger: `ledger_phase_14.md` (all boxes unticked). Conventions:
`../01_PROGRAM_CONVENTIONS.md` (binding). Context: `context.md` (seams and
constraints).

## 1. Scope statement

Phase 14 closes the two connectivity table-stakes rows (gap analysis §3 #1
and #11) and the standards obligations (§4): an IEEE 2030.5 AggregatorServer
[S-1][S-3], an OpenADR 3.0 VTN [S-10], a SunSpec Modbus client [S-16], DNP3
per IEEE 1815.2-2025 [S-14][S-15], design records for IEC 60870-5-104 and
IEC 61850-7-420 (EU surface), MQTT 5 + Sparkplug B edge transport
[S-39][S-40], CIM/CGMES + 61968-5 import [S-17][S-18], an IEEE 1547-2018
settings-profile repository [S-19][S-20], and the DOE consumption interface
[S-158][S-159] — all on the adapter architecture P14.01 defines, all bound by
the constraints in `context.md` §3: read/monitor + command-relay only, the
never-granted set byte-identical, a scope declaration + capability manifest
per adapter, certification forever a human-gated lab process, and every new
crate an additive workspace member keeping the dependency rule green.

Two deliverable depths exist in this phase, stated up front so no record
overclaims. **Design-and-model depth** (most subtasks): model types, schema
and mapping records, profile specifications, and written-unrun test suites —
the same WRITTEN-UNRUN class as the entire Phase 0–10 base. **Record-only
depth** (P14.03, P14.06, P14.09, P14.10, P14.16's ADR extension, P14.19,
P14.22): assessment and decision records where the honest deliverable is the
analysis, because implementation without a named counterparty (a DR program,
a utility SCADA integration, an EU DSO) would produce protocol code with
nothing to talk to. Which protocols move past design depth first is exactly
what P14-E-1 decides.

## 2. Subtask outline

| ID | Title | Summary | Key seams / sources |
|---|---|---|---|
| P14.01 | Protocol adapter architecture and crate layout | The phase's architecture record: proposed `crates/adapters/protocol-*` members (20305, openadr, modbus, dnp3, 104, 61850, mqtt, cim) plus the common adapter-ports crate (name/placement decided here; proposal: `crates/flexcore-protocol-ports` alongside `flexcore-ports`); per-adapter capability manifests; additive `tools/check_dependency_rule.py` updates; the simulator-first testing policy (every adapter testable with zero hardware) | `crates/edge-agent/src/adapters/port.rs`, `registry.rs`; `crates/flexcore-ports/src/`; `tools/check_dependency_rule.py` |
| P14.02 | IEEE 2030.5 AggregatorServer | Server design + model types for the CSIP resource tree — FunctionSetAssignments, DERProgram / DefaultDERControl / DERControlList, ResponseList, PricingList, LogEventList — with resource addressing and query semantics; DNS-SD discovery notes; mTLS server skeleton [S-1][S-3] | [S-1][S-3][S-5]; proposed `crates/adapters/protocol-20305/` |
| P14.03 | IEEE 2030.5 security | CA-chain validation, SFDI/LFDI handling, certificate lifecycle (issuance, rotation, revocation), trust-anchor management; explicitly a design + operations record — no CA service is stood up, real PKI ties to Phase 13 (secrets/Vault) and Phase 18 (device identity) | [S-7][S-8][S-9]; P14.02 server skeleton |
| P14.04 | IEEE 2030.5 conformance test suite (written-unrun) | CSIP use-case matrix incl. DERControl scheduling, response, and log-event flows; primacy / event-prioritization behavior tests [S-156]; tests written complete and UNRUN — execution and any lab correlation are human gates | [S-3][S-5][S-156]; P14.02 types; `context.md` §3 constraint 5 |
| P14.05 | OpenADR 3.0 VTN | REST/JSON event / report / registration endpoint design, VEN management, polling + push delivery, and the mapping from OpenADR events to command-lifecycle PROPOSED states — an external event is never an issued command | [S-10]; proposed `crates/adapters/protocol-openadr/`; `crates/flexcore-domain/src/command_state.rs` |
| P14.06 | OpenADR 2.0b compatibility assessment record | XML/SOAP legacy profile assessed as a record (payload transport, security differences, VTN/VEN interop surface); expected verdict DEFER — registered as P14-E-2 with DEFAULT "3.0-first, 2.0b on program demand" [S-11] | [S-11]; decision record |
| P14.07 | SunSpec Modbus client | Client design over Modbus TCP/RTU for the SunSpec information models (101/103 inverters, 201/203 meters, 80x battery models), gateway polling budgets, and a register-map registry keyed per device model; unit conversion at the adapter per `units.rs` (MW/MVAr, declared reactive sign) | [S-16]; `crates/flexcore-contracts/src/units.rs`; proposed `crates/adapters/protocol-modbus/` |
| P14.08 | DNP3 support | IEEE 1815.2-2025 DER profile data objects; OUTSTATION role first (representing the fleet to utility SCADA), master role assessed as a record; secure-transport posture note (no claims) | [S-14][S-15]; proposed `crates/adapters/protocol-dnp3/` |
| P14.09 | IEC 60870-5-104 client design | EU DSO telecontrol surface as a design record: ASDU/IOA modeling, interrogation and countermeasures for the no-native-encryption reality — compensating controls note (network-layer protection, IEC 62351 assessment) [Track B] | Track B (CESI via the [S-14] analogue); proposed `crates/adapters/protocol-104/` |
| P14.10 | IEC 61850-7-420 MMS client design | DSO-boundary design record: MMS client only; SCL/ICD ingestion; logical-node mapping for DER control (DRCT, ZRCT, DRCC — node semantics verified against the standard text at implementation, UNVERIFIED until then); explicit statement: GOOSE/SV out of scope — protection-grade (foundations §0.3) | [S-12][S-13]; proposed `crates/adapters/protocol-61850/` |
| P14.11 | MQTT 5 + Sparkplug B edge transport | Edge-agent uplink option: Sparkplug B BIRTH/DEATH state management, per-device credentials, topic namespace design; the NATS-at-the-edge evidence noted [S-40] without productizing it | [S-39][S-40]; proposed `crates/adapters/protocol-mqtt/`; `crates/edge-agent/src/` uplink seam |
| P14.12 | CIM/CGMES network-model import | RDF/XML profile parsing design, class mapping (PowerElectronicsConnection, EnergyConsumer, and the DER-relevant classes named at implementation), IEC 61968-5 DER group payloads, and mapping into grid-api models with `not_modelled` scope honesty — every unmapped CIM class is EXPLICITLY_UNMAPPED, never guessed | [S-17][S-18]; `crates/flexcore-grid-api/src/scope.rs`; `crates/edge-agent/src/adapters/openems.rs` classification pattern; proposed `crates/adapters/protocol-cim/` |
| P14.13 | IEEE 1547-2018 settings-profile repository | Function catalog (volt-var, volt-watt, freq-watt, constant-PF, ramp rates, enter/exit service, ride-through categories), jurisdiction default tables (MA TSRG as the first [S-19]), per-device settings state; a document store, not a claim that any device accepts any function | [S-19][S-20]; `crates/flexcore-domain/src/inverter_pq_model.rs` |
| P14.14 | Inverter settings-management workflow | Certified baseline (utility-owned) vs aggregator schedules (DERControl-shaped); primacy/precedence rules; utility-override hook (1547-2018 Clause 10); a settings change is a lifecycle command with the approval gate — never a direct adapter write | [S-20][S-156]; `crates/flexcore-domain/src/command_state.rs`; P14.13 repository |
| P14.15 | DOE consumption interface | Per-connection-point / per-group import-export envelopes ingested as optimizer constraint data with issue timestamps, expiry, and update-cadence handling (EDGE Workstream 3 finding [S-158]); lapse behavior (revert to the static limit per SAPN [S-159]); breach / curtailment telemetry | [S-158][S-159][S-160]; `crates/flexcore-domain/src/`, `crates/flexcore-simulator/src/profiles.rs` |
| P14.16 | Adapter permission model | Per-adapter capability manifests in kernel profiles, the never-granted review (review only — the set stays byte-identical), the command-relay-only rule enforced by kernel profiles, and the ADR-010 extension record stating adapters hold no dispatch authority | `crates/flexcore-kernel/src/manifest.rs`, `profile.rs`; `docs/architecture/adr/ADR-010.md` |
| P14.17 | Simulator integration | `flexcore-simulator` profiles extended so every adapter is testable against simulated devices with no hardware: a simulated 2030.5 client, a simulated OpenADR VEN, a simulated Modbus device; the OCPP counterpart is named for Phase 17 and built then | `crates/flexcore-simulator/src/profiles.rs`, `scenario.rs`; `crates/edge-agent/src/adapters/simulated.rs` |
| P14.18 | Edge-agent adapter seam extension | Policy for real protocol bridging at the edge: `generic-rest` becomes spec'd or is explicitly removed (the RESERVED placeholder is decided, one way or the other); edge-to-cloud protocol mapping; bandwidth budgets for fleet uplinks | `crates/edge-agent/src/adapters/registry.rs`, `mod.rs` |
| P14.19 | Adapter conformance evidence plan | Certification paths recorded as human-gated procedures — QualityLogic/CSIP test labs, OpenADR Alliance certification, DNP Users Group testing; the evidence each lab needs and what FlexCore can prepare in-repo; zero conformance claims | [S-5][S-10][S-14]; decision record; conventions §7.3 |
| P14.20 | Backpressure and rate limiting per protocol | Polling budgets per gateway, fleet-scale connection limits, bounded queues per adapter, per-adapter metrics landed in the P11 taxonomy (trace_id correlation consumed, never redefined) | [S-85]; P11 metric/log taxonomy; P14.07 polling budgets |
| P14.21 | Protocol observability | Per-adapter metrics/traces/logs taxonomy, connection-state alarms, message-rate budgets bound to the P11 alert-rule shape | P11 taxonomy; `apps/control-plane/src/metrics.rs` |
| P14.22 | Cross-protocol priority decision record | P14-E-1 recorded: DEFAULT recommendation IEEE 2030.5 + OpenADR 3.0 first (DR program surfaces), DNP3 next (utility SCADA), 104/61850 on EU demand; the record names what evidence would close it | decision record; conventions §4 |
| P14.23 | Operator documentation per adapter | Deployment, security, and operations documentation per protocol adapter (enrollment, credentials, budgets, failure modes) — written docs, no runbooks claimed exercised | docs tree; P14.03, P14.19 outputs |
| P14.24 | Decision records and phase close | P14-E-1 (priority), P14-E-2 (OpenADR 2.0b), P14-E-3 (61850/104 EU timing) recorded OPEN with DEFAULTs; consistency audit across the phase's records | phase `records/` (created at implementation time) |

## 3. Implementation wave plan (2 agents)

| Wave | Agent A | Agent B |
|---|---|---|
| 1 | P14.01–P14.04 (adapter architecture + IEEE 2030.5) | P14.05–P14.08 (OpenADR + field protocols) |
| 2 | P14.09–P14.12 (EU surface + CIM import) | P14.13–P14.16 (settings / DOE / permissions) |
| 3 | P14.17–P14.21 (simulator / edge / observability) | P14.22–P14.24 (docs + close) |

Disjoint ownership:

- **Wave 1, Agent A** owns the P14.01 architecture record, the common
  adapter-ports crate, and the proposed `crates/adapters/protocol-20305/`.
- **Wave 1, Agent B** owns the proposed `crates/adapters/protocol-openadr/`,
  `protocol-modbus/`, and `protocol-dnp3/` crates and the 2.0b assessment
  record. Agent B consumes the P14.01 layout record (A lands P14.01 first in
  the batch; B re-reads and records the dependence).
- **Wave 2, Agent A** owns the proposed `protocol-104/`, `protocol-61850/`,
  `protocol-mqtt/`, and `protocol-cim/` crates.
- **Wave 2, Agent B** owns the settings repository and workflow modules, the
  DOE interface modules, and the kernel profile additions in
  `crates/flexcore-kernel/src/` — with the never-granted set untouched.
- **Wave 3, Agent A** owns `crates/flexcore-simulator/src/` (profiles,
  scenario), the `crates/edge-agent/src/adapters/` seam changes, and the
  per-adapter metrics/observability modules.
- **Wave 3, Agent B** owns the evidence plan, the priority record, operator
  docs, and the close records — documentation and records only, no file
  overlap with Agent A.
- Shared files across waves: the workspace manifest (member additions) and
  `tools/check_dependency_rule.py` (additive checker entries). Edits there
  are single-line additive entries landed within each agent's own batch, with
  a re-read before edit and the dependence recorded. No other shared files
  exist.

## 4. Decision points (registered OPEN — agents never close E-decisions)

### P14-E-1 — Cross-protocol implementation priority

- **Question:** which protocol adapters get implementation effort first,
  given that all land as design + models + written-unrun tests in Phase 14?
- **Options:** (a) IEEE 2030.5 + OpenADR 3.0 first (DR program surfaces),
  DNP3 next (utility SCADA), 104/61850 on EU demand; (b) DNP3 first (utility
  SCADA before DR programs); (c) equal-depth implementation of all.
- **DEFAULT (RECOMMENDATION):** (a) — DR programs are the nearest commercial
  surface for the table-stakes rows, and 2030.5/OpenADR certification
  infrastructure [S-5][S-10] is the most documented path.
- **What closes it:** the human owner naming the first DR program and the
  first utility SCADA integration (if any). Program-level priorities are
  human E-decisions (conventions §4).

### P14-E-2 — OpenADR 2.0b compatibility

- **Question:** does the platform serve the legacy 2.0b XML/SOAP profile
  alongside 3.0?
- **Options:** (a) 3.0-only; (b) a 2.0b bridge adapter on demand; (c) a full
  2.0b VTN alongside 3.0.
- **DEFAULT (RECOMMENDATION):** "3.0-first, 2.0b on program demand" —
  effectively (a) now, (b) when a named program requires it. P14.06 records
  the assessment either way.
- **What closes it:** a named DR program or market requiring 2.0b VEN/VTN
  interop [S-11].

### P14-E-3 — IEC 61850-7-420 and IEC 60870-5-104 EU timing

- **Question:** when do the EU DSO surfaces move from design records to
  implementation?
- **Options:** (a) design-only in Phase 14, implement on the first named EU
  integration; (b) implement both in a dedicated later phase; (c) implement
  104 only (telecontrol precedes 61850).
- **DEFAULT (RECOMMENDATION):** (a) — design records exist so the EU surface
  is scopeable, but no EU integration is named, and protocol stacks without
  a counterparty rot.
- **What closes it:** the human owner naming the first EU DSO integration
  and its protocol requirements.

## 5. Risks

| # | Risk | Treatment |
|---|---|---|
| 1 | Protocol stacks are large: full 2030.5, OpenADR, DNP3, and CIM stacks are years of engineering — the subtask scope here is design + models + written-unrun tests, not full conformance | Every subtask states its deliverable class (design record, model types, test files, profile specs); no subtask claims a complete stack; depth is gated by P14-E-1 |
| 2 | Certification is human-gated and cannot be prepared by agents alone: labs, fees, signed test results | P14.19 records the paths and the evidence each lab needs; every document carries conventions §7.3 honesty; no conformance language anywhere |
| 3 | CA-chain operations (P14.03) need real PKI — issuance, revocation, HSM-class key custody are operational capabilities, not code | P14.03 is a design + operations record; the CA itself ties to Phase 13 (secrets surface) and Phase 18 (device identity); nothing is stood up in this phase |
| 4 | The 0–10 base is WRITTEN-UNRUN: adapters are written against a workspace that has never compiled, and the first build may surface recorded defects | Same treatment as every phase (program README boundary 1); records describe written code, never observed behavior; the first human build gates implementation start |
| 5 | Standard-detail drift: DNS-SD service names, exact register semantics, 61850 logical-node attributes are cited from secondary sources and may differ in the standard text | UNVERIFIED labels at each such point (the openems.rs UNVERIFIED-ASSUMPTION pattern); verification against the purchased standard text or a lab Edge is an implementation-time human gate |

## 6. Acceptance criteria (honest)

1. Every subtask has a decision record under `records/` (created at
   implementation time) citing the files touched and the [S-n] sources for
   every external claim.
2. The deliverables are model/schema designs, written-unrun test files,
   simulator profile specifications, and decision/assessment records —
   **no protocol is ever connected, no device is ever reached, no message is
   ever exchanged** in this program.
3. Every adapter (designed or implemented) carries a scope declaration with a
   mandatory non-empty `not_modelled` list and a capability manifest; the
   never-granted set in `crates/flexcore-kernel/src/manifest.rs` is
   byte-identical before and after.
4. `python -I tools/check_dependency_rule.py` stays green after every batch;
   `python tools/verify_docs.py` and `python tools/validate_agent.py` report
   no failures; every new crate is an additive workspace member.
5. P14-E-1, P14-E-2, and P14-E-3 remain OPEN with DEFAULTs recorded; no
   certification, conformance, or safety claim appears anywhere.

## 7. Execution notes for implementers

1. Decision records go under
   `phase-14-protocol-device-connectivity/records/`, created at
   implementation time (conventions §2.4), numbered in batch order.
2. New crates follow conventions §6.2 (real code, clear ownership, stable
   dependency direction) — a crate is added only in the batch that gives it
   code; the P14.01 layout record names them as proposals, never as
   scaffolding.
3. Adapter scope declarations mirror `crates/flexcore-grid-api/src/scope.rs`
   exactly: no Default impl, mandatory non-empty `not_modelled`,
   fail-closed `assert_supports`; the `not_modelled` list is the reviewed
   limitation record, and an empty one is a construction error.
4. External events (DERControl schedules, OpenADR events, 1547 settings
   changes) enter the command lifecycle at PROPOSED and pass all three gates
   like any other candidate — an adapter that constructs an APPROVED or
   ISSUED state from external input is a defect, not a shortcut.
5. Every batch ends with the three permitted checks quoted verbatim in the
   batch's decision record (conventions §1.3, §6.1) — no "verified" without
   output (AGENTS.md §3 rule 7).
6. Standard-detail honesty (the openems.rs UNVERIFIED-ASSUMPTION pattern,
   carried forward): every protocol detail cited from a secondary source —
   DNS-SD service names, exact SunSpec register semantics, 61850
   logical-node attributes, CIM class attribute lists — carries an
   UNVERIFIED label naming what the implementation-time check against the
   standard text or a lab device must confirm; a mismatch is a code or
   record edit, never a silent reinterpretation.
7. Simulator-first order within waves: P14.17's simulated counterparts land
   in the same program as the adapters they exercise (wave 3), so any
   subtask that adds adapter behavior names its simulated counterpart and
   defers hardware claims to the human lab gates entirely.
