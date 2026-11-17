# Phase 14 — Protocol and Device Connectivity Expansion — Ledger

Date: 2026-11-14. Status: **IMPLEMENTED — awaiting human build/test** — 24 of 24 subtasks ticked. Phase close: HANDOVER.md, CHECKLIST.md, records/104_p14_24_phase_close.md. No protocol connected, no device reached, no message exchanged; the human-gate queue is in CHECKLIST.md.

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

- [x] **P14.01 — Protocol adapter architecture and crate layout.** Proposed (decision record: `records/01_p14_01_adapter_architecture.md`)
      `crates/adapters/protocol-*` members, the common adapter-ports crate,
      per-adapter capability manifests, additive `check_dependency_rule.py`
      updates, the simulator-first testing policy. Evidence: the record
      cites all five outputs; crates stay proposals until a batch adds real
      code.
- [x] **P14.02 — IEEE 2030.5 AggregatorServer.** Server design and model (decision record: `records/02_p14_02_aggregator_server.md`)
      types for the CSIP resource tree (FunctionSetAssignments, DERProgram/
      DefaultDERControl/DERControlList, ResponseList, PricingList,
      LogEventList), DNS-SD notes, mTLS server skeleton [S-1][S-3].
      Evidence: the record cites the models and skeleton; no server runs.
- [x] **P14.03 — IEEE 2030.5 security.** CA-chain validation, SFDI/LFDI (decision record: `records/03_p14_03_security_design.md`)
      handling, certificate lifecycle (issuance/rotation/revocation), and
      trust-anchor management as design + operations record [S-7][S-8][S-9].
      Evidence: the record cites the design, trust-anchor plan, and Phase
      13/18 PKI ties; no CA exists, no certificate is issued.
- [x] **P14.04 — IEEE 2030.5 conformance test suite (written-unrun).** CSIP (decision record: `records/04_p14_04_conformance_test_suite.md`)
      use-case matrix incl. DERControl scheduling, response, and log-event
      flows plus primacy/event-prioritization tests [S-156]. Evidence: the
      record cites the matrix and the written-unrun test files; execution
      is a human gate and no conformance is claimed.
- [x] **P14.05 — OpenADR 3.0 VTN.** REST/JSON event/report/registration (decision record: `records/21_p14_05_openadr_vtn.md`)
      endpoint design, VEN management, polling + push, and the
      event-to-lifecycle mapping into PROPOSED states [S-10]. Evidence: the
      record cites the endpoint design, VEN model, and mapping rule; an
      external event never passes PROPOSED without the three gates.
- [x] **P14.06 — OpenADR 2.0b compatibility assessment record.** The legacy (decision record: `records/22_p14_06_openadr_2_0b_assessment.md`)
      XML/SOAP profile assessed (transport, security, interop surface),
      verdict registered as P14-E-2, DEFAULT "3.0-first, 2.0b on program
      demand" [S-11]. Evidence: the record cites the assessment and the
      E-decision entry; no 2.0b code exists.
- [x] **P14.07 — SunSpec Modbus client.** Client design for the SunSpec (decision record: `records/23_p14_07_sunspec_modbus_client.md`)
      information models (101/103 inverters, 201/203 meters, 80x battery),
      gateway polling budgets, and a register-map registry per device model
      [S-16]. Evidence: the record cites the mapping, budgets, and
      registry; unit conversion follows `units.rs` at the adapter.
- [x] **P14.08 — DNP3 support.** IEEE 1815.2-2025 DER profile data objects; (decision record: `records/24_p14_08_dnp3_der_profile.md`)
      outstation role first (fleet presented to utility SCADA), master role
      assessment, secure-transport posture note [S-14][S-15]. Evidence: the
      record cites the data-object mapping, role decision, and posture
      note; no SCADA connection exists.
- [x] **P14.09 — IEC 60870-5-104 client design.** EU DSO telecontrol design (decision record: `records/41_p14_09_iec_60870_5_104_client_design.md`)
      record: ASDU/IOA modeling and the compensating-controls note for the
      no-native-encryption reality (network-layer protection, 62351
      assessment) [Track B]. Evidence: the record cites the design and the
      controls note; no 104 connection exists.
- [x] **P14.10 — IEC 61850-7-420 MMS client design.** DSO-boundary design: (decision record: `records/42_p14_10_iec_61850_7_420_mms_client_design.md`)
      MMS client only, SCL/ICD ingestion, logical-node mapping (DRCT, ZRCT,
      DRCC — UNVERIFIED until checked against the standard text), the
      explicit GOOSE/SV out-of-scope statement (protection-grade,
      foundations §0.3) [S-12][S-13]. Evidence: the record cites all four.
- [x] **P14.11 — MQTT 5 + Sparkplug B edge transport.** Edge-agent uplink (decision record: `records/43_p14_11_mqtt_sparkplug_b_uplink.md`)
      option with BIRTH/DEATH state management, per-device credentials, and
      topic namespace design [S-39][S-40]. Evidence: the record cites the
      namespace design, state model, and credential scheme; no broker is
      stood up and no message is published.
- [x] **P14.12 — CIM/CGMES network-model import.** RDF/XML parsing design, (decision record: `records/44_p14_12_cim_cgmes_import.md`)
      class mapping (PowerElectronicsConnection, EnergyConsumer, DER
      classes named at implementation), 61968-5 DER group payloads, grid-api
      mapping with `not_modelled` honesty [S-17][S-18]. Evidence: the
      record cites the mapping and the explicitly-unmapped register.
- [x] **P14.13 — IEEE 1547-2018 settings-profile repository.** Function (decision record: `records/61_p14_13_settings_profile_repository.md`)
      catalog (volt-var, volt-watt, freq-watt, constant-PF, ramp rates,
      enter/exit service, ride-through categories), jurisdiction tables
      (MA TSRG first [S-19]), per-device settings state [S-20]. Evidence:
      the record cites catalog, tables, state model; no device writes.
- [x] **P14.14 — Inverter settings-management workflow.** Certified baseline (decision record: `records/62_p14_14_settings_management_workflow.md`)
      (utility-owned) vs aggregator schedules (DERControl-shaped),
      primacy rules, the utility-override hook (1547-2018 Clause 10),
      settings change = lifecycle command with approval gate [S-20][S-156].
      Evidence: the record cites precedence, hook, gate; nothing bypasses
      the lifecycle.
- [x] **P14.15 — DOE consumption interface.** Per-connection-point/group (decision record: `records/63_p14_15_doe_consumption_interface.md`)
      import-export envelopes as optimizer constraint data: issue
      timestamps, expiry, update cadence [S-158]; lapse reverts to the
      static limit per SAPN [S-159]; breach telemetry. Evidence: the record
      cites the schema, lapse rule, and telemetry wiring.
- [x] **P14.16 — Adapter permission model.** Per-adapter capability (decision record: `records/64_p14_16_adapter_permission_model.md`)
      manifests in kernel profiles, the never-granted review (the set stays
      byte-identical), the command-relay-only rule, the ADR-010 extension
      record. Evidence: the record cites all four; no adapter holds
      dispatch authority.
- [x] **P14.17 — Simulator integration.** `flexcore-simulator` profiles (decision record: `records/81_p14_17_simulated_counterparts.md`)
      extended so every adapter is testable against simulated devices with
      no hardware: simulated 2030.5 client, OpenADR VEN, Modbus device;
      OCPP named for Phase 17. Evidence: the record cites the profile
      additions; simulated data is never evidence about real devices.
- [x] **P14.18 — Edge-agent adapter seam extension.** Policy for real (decision record: `records/82_p14_18_edge_agent_seam_extension.md`)
      protocol bridging at the edge: `generic-rest` becomes spec'd or is
      explicitly removed, edge-to-cloud protocol mapping, bandwidth
      budgets. Evidence: the record cites the RESERVED-placeholder decision,
      mapping, budgets; the registry stays an allowlist.
- [x] **P14.19 — Adapter conformance evidence plan.** Certification paths (decision record: `records/101_p14_19_conformance_evidence_plan.md`)
      (QualityLogic/CSIP labs, OpenADR Alliance certification, DNP Users
      Group testing) as human-gated procedures with evidence lists; zero
      conformance claims [S-5][S-10][S-14]. Evidence: the record cites the
      paths; all outcomes are human lab results.
- [x] **P14.20 — Backpressure and rate limiting per protocol.** Polling (decision record: `records/83_p14_20_backpressure_rate_limiting.md`)
      budgets, fleet-scale connection limits, bounded queues per adapter,
      per-adapter metrics landed in the P11 taxonomy (trace_id correlation
      consumed, never redefined). Evidence: the record cites budgets,
      limits, and the metric families with cardinality decisions.
- [x] **P14.21 — Protocol observability.** Per-adapter metrics/traces/logs (decision record: `records/84_p14_21_protocol_observability.md`)
      taxonomy, connection-state alarms, and message-rate budgets bound to
      the P11 alert-rule shape. Evidence: the record cites the taxonomy,
      alarms, and budget bindings; nothing claims a live scrape or fired
      alarm.
- [x] **P14.22 — Cross-protocol priority decision record.** P14-E-1 recorded (decision record: `records/102_p14_22_priority_decision_record.md`)
      OPEN: DEFAULT recommendation IEEE 2030.5 + OpenADR 3.0 first (DR
      program surfaces), DNP3 next (utility SCADA), 104/61850 on EU demand.
      Evidence: the record cites the options, DEFAULT, and what closes it;
      no agent closes the decision.
- [x] **P14.23 — Operator documentation per adapter.** Deployment, security, (decision record: `records/103_p14_23_operator_documentation.md`)
      and operations documentation per adapter (enrollment, credentials,
      budgets, failure modes). Evidence: the record cites the doc paths per
      adapter; documentation is written, never exercised.
- [x] **P14.24 — Decision records and phase close.** P14-E-1 (priority), (decision record: `records/104_p14_24_phase_close.md`)
      P14-E-2 (OpenADR 2.0b), and P14-E-3 (61850/104 EU timing) recorded
      OPEN with DEFAULTs, plus a consistency audit across the phase's
      records. Evidence: the record cites the three E-decision entries and
      the consistency-audit result; no E-decision is closed by an agent.

## Decision register (all OPEN — agents never close E-decisions)

| ID | Question | DEFAULT (RECOMMENDATION) | What closes it |
|---|---|---|---|
| P14-E-1 | Cross-protocol implementation priority | IEEE 2030.5 + OpenADR 3.0 first; DNP3 next; 104/61850 on EU demand | Human owner names the first DR program and first utility SCADA integration |
| P14-E-2 | OpenADR 2.0b compatibility | 3.0-first; 2.0b only on named program demand | A named DR program or market requiring 2.0b interop [S-11] |
| P14-E-3 | IEC 61850-7-420 / IEC 104 EU implementation timing | Design-only in Phase 14; implement on the first named EU integration | Human owner names the first EU DSO integration and its protocol requirements |
