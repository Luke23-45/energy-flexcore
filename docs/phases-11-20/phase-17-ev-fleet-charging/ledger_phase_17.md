# Phase 17 — EV Fleet and Charging Integration — Ledger

Date: 2026-11-14. Status: **IMPLEMENTED — awaiting human build/test** — 18 of 18 subtasks ticked. Phase close: HANDOVER.md, CHECKLIST.md, records/64_phase_17_close_and_consistency_audit.md. No charge point connected, no OCPP message exchanged with a real device; the human-gate queue is in CHECKLIST.md.

**Gate.** No implementation starts before (a) human review of this roadmap
and (b) the first human build of the Phase 0–10 workspace is green (program
conventions §3; program README, hard status boundary 1); every run, scan,
sign-off, and date is a HUMAN GATE INPUT recorded UNRUN / EMPTY / UNSIGNED.

**Tick convention (carried from Phases 0–10).** Ticked only as "implemented,
awaiting human build/test" with a decision-record citation (`- [x] ...
(decision record: \`records/NN_slug.md\`)`) — never as "tested" or
"verified"; records are created at implementation time under `records/`.

## Subtask register

- [x] **P17.01 — OCPP CSMS adapter architecture.** WebSocket server, OCPP-J (decision record: `records/01_p17_01_ocpp_csms_adapter_architecture.md`)
      JSON framing, subprotocol negotiation (`ocpp2.0.1` baseline; 1.6 =
      P17-E-1), one-outstanding-CALL discipline, both-direction initiation
      [S-121][S-123][S-125]. Evidence: framing cited; no charge point
      connects.
- [x] **P17.02 — CSMS hosting boundary decision + design.** (decision record: `records/02_p17_02_csms_hosting_boundary.md`)
      Edge-agent-hosted vs control-plane-hosted (P17-E-2); DEFAULT
      edge-agent-hosted for depot sites (offline tolerance,
      store-and-forward reuse) per conventions §4. Evidence: both designs
      and the E-decision cited; stays OPEN.
- [x] **P17.03 — OCPP security Profile 3 onboarding.** Per-device client (decision record: `records/03_p17_03_security_profile_3_onboarding.md`)
      certificates, certificate flows (SignCertificate / CertificateSigned /
      InstallCertificate), SecurityEventNotification ingestion, identity
      proofing at registration [S-126][S-127][S-133]. Evidence: flows and
      proofing cited; no certificate issued.
- [x] **P17.04 — Boot/provisioning/status/transaction mapping.** (decision record: `records/04_p17_04_boot_status_transaction_mapping.md`)
      BootNotification, StatusNotification, Heartbeat, TransactionEvent →
      domain EVSE registry extension; offline behavior recorded (connection
      loss is not inferred unavailability) [S-121][S-122]. Evidence: mapping
      and offline rules cited.
- [x] **P17.05 — Smart-charging schedule composer.** Dispatch targets/ (decision record: `records/05_p17_05_smart_charging_composer.md`)
      envelopes → SetChargingProfile payloads: purposes
      (ChargingStationExternalConstraints at evseId 0 with isGridCritical,
      TxDefaultProfile, TxProfile), stack-level precedence, minChargingRate
      handling (0 when headroom < min rate), W/A conversion,
      GetCompositeSchedule reconciliation, stored-not-applied honesty
      [S-124][S-141]. Evidence: composer spec cited; lifecycle-only.
- [x] **P17.06 — Vehicle-proposal ingestion.** NotifyEVChargingNeeds / (decision record: `records/21_p17_06_vehicle_proposal_ingestion.md`)
      NotifyEVChargingSchedule (ISO 15118 proposals) and
      NotifyChargingLimit / ClearedChargingLimit (locally imposed limits) →
      constraint records [S-124][S-125]. Evidence: inputs, never
      auto-commands (ADR-010).
- [x] **P17.07 — MeterValues ingestion + signed meter values hooks.** (decision record: `records/22_p17_07_meter_values_ingestion.md`)
      MeterValues cadence configuration, MeterValuesSign hooks
      (Eichrecht-grade path; block letters UNVERIFIED), six-state quality
      mapping at the adapter boundary [S-135][S-136]. Evidence: non-silencing
      rule intact.
- [x] **P17.08 — Dual-source SoC arbitration.** OCPP MeterValues SoC (decision record: `records/23_p17_08_soc_arbitration.md`)
      authoritative-while-plugged vs the telematics seam; staleness rules
      (STALE); Tesla 500 ms the outlier [S-137]; stale-when-sleeping
      documented [S-138]; plug-state as ground truth [S-139][S-140].
      Evidence: no vendor API is called.
- [x] **P17.09 — Departure-time constraint model.** Vehicle calendar/ (decision record: `records/24_p17_09_departure_time_constraints.md`)
      telematics ingestion → per-vehicle energy-by-time constraints feeding
      the composer (P17.05), mapped onto the domain model's HARD
      departure-energy policy [S-140]. Evidence: the hard policy is never
      relaxed.
- [x] **P17.10 — User-override precedence policy.** Driver manual starts > (decision record: `records/25_p17_10_user_override_precedence.md`)
      VPP curtailment, recorded as dispatch policy + negative tests
      (written-unrun); precedence resolves BEFORE issuance — adapters never
      adjudicate [S-140]. Evidence: lifecycle gates stay distinct.
- [x] **P17.11 — Depot/site load management.** Site import/transformer (decision record: `records/26_p17_11_depot_load_management.md`)
      limits, simultaneity-factor control, the P14.15 DOE envelope
      interplay, external-constraint profile composition [S-141][S-140].
      Evidence: maps onto the domain transformer limit.
- [x] **P17.12 — ISO 15118 PnC certificate management design (decision record: `records/41_p17_12_pnc_certificate_management_design.md`)
      (PHASE-GATED).** V2G Root CA trust store, OCPP certificate flows
      (Get15118EVCertificate, CertificateSigned, InstallCertificate,
      GetCertificateChainStatus; block letters UNVERIFIED), OCSP validation;
      GATED on PKI operations maturity (P14.03 interplay)
      [S-128][S-130][S-125]. Evidence: no PKI is stood up.
- [x] **P17.13 — V2G readiness modeling.** Bidirectional capability per (decision record: `records/42_p17_13_v2g_readiness_modeling.md`)
      EVSE/vehicle (five-condition `V2xConfig` gate exists); OCPP 2.1 hooks
      (NotifyAllowedEnergyTransfer, AFRRSignal, SetDERControl) as design
      records; the North-American AC path (2030.5 V2G-AC / J3072 / UL 1741
      SC) a SEPARATE future integration [S-125][S-4][S-131][S-144].
      Evidence: no commands produced.
- [x] **P17.14 — OCPP conformance test suite (written-unrun).** OCA (decision record: `records/43_p17_14_ocpp_conformance_suite.md`)
      use-case matrix subset (provisioning, transactions, smart charging,
      security events, firmware status) plus negative tests (malformed
      frames, subprotocol mismatch, cert failures) [S-121][S-127].
      Evidence: written-unrun files; certification is a human gate.
- [x] **P17.15 — Simulated charge points.** `flexcore-simulator` extension: (decision record: `records/44_p17_15_simulated_charge_points.md`)
      simulated OCPP charge points speaking OCPP-J against the CSMS adapter
      (hardware-free; fulfills the P14.17 promise), V2G behavior linkage
      added, honestly labeled synthetic. Evidence: simulated data is never
      evidence about real devices.
- [x] **P17.16 — AFIR/Eichrecht compliance assessment record.** EU hooks (decision record: `records/61_p17_16_afir_eichrecht_compliance_assessment.md`)
      (MeterValuesSign, ad-hoc payment awareness, price-per-kWh display
      adjacency); scope statement: a depot fleet behind a site meter is
      adjacent, not directly AFIR-scoped, with revisit conditions
      [S-135][S-136]. Evidence: no compliance claim.
- [x] **P17.17 — OCPI/OICP out-of-scope record.** Roaming (OCPI 2.3.0 (decision record: `records/62_p17_17_ocpi_oicp_out_of_scope.md`)
      [S-134], OICP) recorded out of scope with revisit conditions (public
      CPO network operation), matching gap analysis §8. Evidence: no roaming
      code is planned.
- [x] **P17.18 — Decision records and phase close.** P17-E-1 (OCPP editions (decision record: `records/64_phase_17_close_and_consistency_audit.md`)
      baseline), P17-E-2 (CSMS hosting boundary), P17-E-3 (telematics:
      vendor-agnostic port first), a consistency audit, and the D5 note
      (OCPP 2.0.1 is the CANDIDATE D5 would name — D5 stays human-closed).
      Evidence: no agent closes any decision.

## Decision register (all OPEN — agents never close E-decisions)

| ID | Question | DEFAULT (RECOMMENDATION) | What closes it |
|---|---|---|---|
| P17-E-1 | OCPP editions baseline | `ocpp2.0.1` baseline with `ocpp2.1` additive compatibility | D5 (human) names the OCPP edition as well as the version; the Edition 2 open verification task (`ledger/phase-06-edge-protocol/RESEARCH.md` §3) resolves |
| P17-E-2 | CSMS hosting boundary | Edge-agent-hosted for depot sites (offline tolerance); control-plane-hosted elsewhere | Human owner names the first depot site and its connectivity profile |
| P17-E-3 | Telematics approach | Vendor-agnostic port first (staleness rules at the port); named vendor adapters later on demand | Human owner names the first fleet vendor and accepts its API terms |
