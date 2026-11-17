# Phase 17 — EV Fleet and Charging Integration — Plan

Date: 2026-11-14. Status: **NOT STARTED** (0/18). Planning baseline only.
Ledger: `ledger_phase_17.md` (all boxes unticked). Conventions:
`../01_PROGRAM_CONVENTIONS.md` (binding). Context: `context.md` (seams and
constraints).

## 1. Scope statement

Phase 17 closes the EV build-list (gap analysis §7, Track F1): an OCPP CSMS
adapter — WebSocket server, OCPP-J JSON framing, subprotocol negotiation
(`ocpp2.0.1` baseline, `ocpp2.1` additive) [S-121][S-123][S-125]; security
Profile 3 onboarding [S-126][S-127][S-133]; the smart-charging schedule
composer (purposes, stack levels, minChargingRate, W/A conversion,
GetCompositeSchedule reconciliation) as the dispatch path [S-124][S-141];
vehicle-proposal and locally-imposed-limit ingestion; MeterValues ingestion
with signed-meter-value hooks [S-135][S-136]; dual-source SoC arbitration
[S-137][S-138][S-139]; departure-time constraints; user-override precedence;
depot load management under site import limits [S-140][S-141]; ISO 15118 PnC
certificate-management design, PHASE-GATED on PKI maturity [S-128][S-130];
V2G readiness records [S-125][S-4][S-131][S-144]; simulated charge points;
an AFIR/Eichrecht assessment [S-135][S-136]; the OCPI/OICP out-of-scope
record [S-134]; and the phase's decision records.

Two deliverable depths exist, stated up front so no record overclaims.
**Design-and-model depth** (most subtasks): adapter architecture, message
mappings, the composer specification, written-unrun test suites, and
simulator profile specifications — the WRITTEN-UNRUN class of the entire
program. **Record-only depth** (P17.12's gated design, P17.13's hook
records, P17.16, P17.17): assessment and decision records where the honest
deliverable is the analysis, because implementation without a named depot,
a named fleet operator, or a PKI operator would produce protocol code with
nothing to talk to.

**D5 interplay (explicit).** OCPP 2.0.1 is the CANDIDATE first field
protocol that D5 would name; P17-E-1 records the edition baseline and the
evidence D5 needs. D5 itself stays OPEN and human-closed
(`ledger/MASTER_LEDGER.md` row D5) — nothing in this phase closes it.

## 2. Subtask outline

| ID | Title | Summary | Key seams / sources |
|---|---|---|---|
| P17.01 | OCPP CSMS adapter architecture | WebSocket server design, OCPP-J JSON framing (`[2,"id","Action",{...}]` CALL / CALLRESULT / CALLERROR), subprotocol negotiation (`ocpp2.0.1` baseline, `ocpp2.1`-ready; 1.6 support = P17-E-1), the one-outstanding-CALL discipline, and both-direction initiation (CSMS-originated requests, not only charge-point-originated) [S-121][S-123][S-125] | Proposed `crates/adapters/protocol-ocpp/`; `crates/edge-agent/src/adapters/port.rs` pattern; `ledger/phase-06-edge-protocol/RESEARCH.md` §3 |
| P17.02 | CSMS hosting boundary decision + design | Edge-agent-hosted vs control-plane-hosted (P17-E-2); DEFAULT: the edge agent hosts the CSMS connection for depot sites — offline tolerance matters and the edge agent's store-and-forward buffer is reused; both shapes designed, the default implemented per conventions §4 | `crates/edge-agent/src/` (offline buffer, mTLS uplink); `apps/control-plane/`; decision record |
| P17.03 | OCPP security Profile 3 onboarding | Per-device client certificates, SignCertificate / CertificateSigned / InstallCertificate flows, SecurityEventNotification ingestion, and identity proofing at registration [S-126][S-127][S-133] | P17.01 framing; P14.03 PKI patterns; decision record |
| P17.04 | Boot/provisioning/status/transaction mapping | BootNotification, StatusNotification, Heartbeat, and TransactionEvent mapped into a domain EVSE registry extension; offline behavior recorded (connection loss is not inferred asset unavailability; statuses map to named states, never guessed) [S-121][S-122] | `crates/flexcore-domain/src/` registry rules; new EVSE registry extension file |
| P17.05 | Smart-charging schedule composer | Dispatch targets/envelopes → SetChargingProfile payloads: purpose selection (ChargingStationExternalConstraints at evseId 0 with isGridCritical, TxDefaultProfile, TxProfile), stack-level/purpose precedence, minChargingRate handling (0 elsewhere when headroom < min rate), W/A conversion (AC A·V·phases vs DC A·V), GetCompositeSchedule reconciliation, and "Accepted" = stored-not-applied honesty [S-124] | `crates/flexcore-domain/src/ev_fleet_model.rs`; `crates/flexcore-domain/src/command_state.rs`; [S-141] |
| P17.06 | Vehicle-proposal ingestion | NotifyEVChargingNeeds / NotifyEVChargingSchedule (ISO 15118 proposals relayed by the station) and NotifyChargingLimit / ClearedChargingLimit (locally imposed limits) → constraint records; proposals are inputs and records, never auto-commands (ADR-010 extension) [S-124][S-125] | P17.01 adapter inbound path; `crates/flexcore-domain/src/`; `docs/architecture/adr/ADR-010.md` |
| P17.07 | MeterValues ingestion + signed meter values hooks | MeterValues cadence configuration, MeterValuesSign hooks (the Eichrecht-grade path; exact block letters UNVERIFIED against the OCA text), and the adapter-boundary mapping onto the frozen six-state quality vocabulary — the non-silencing rule intact [S-135][S-136] | `apps/worker/src/ingestion.rs`; `crates/flexcore-contracts/src/quality.rs`; `crates/flexcore-ports/src/telemetry_source.rs` |
| P17.08 | Dual-source SoC arbitration | OCPP MeterValues SoC authoritative-while-plugged vs the telematics-API seam; staleness rules (STALE per the quality vocabulary); Tesla 500 ms telemetry the outlier [S-137]; telematics stale-when-sleeping documented [S-138]; plug-state as participation ground truth [S-139][S-140] | `crates/flexcore-contracts/src/quality.rs`; P17.06 constraint records; decision record |
| P17.09 | Departure-time constraint model | Vehicle calendar/telematics ingestion → per-vehicle energy-by-time constraints feeding the composer (P17.05); maps onto the domain model's HARD departure-energy policy (required departure SOC by the departure instant — never relaxed by ingestion) [S-140] | `crates/flexcore-domain/src/ev_fleet_model.rs`; P17.08 seam |
| P17.10 | User-override precedence policy | Driver manual starts > VPP curtailment, recorded as dispatch policy + negative tests (written-unrun); precedence resolves BEFORE issuance — an adapter never adjudicates and never refuses a lifecycle-issued command on its own authority [S-140] | `crates/flexcore-domain/src/ev_fleet_model.rs` (opt-out); `crates/flexcore-domain/src/command_state.rs`; test files |
| P17.11 | Depot/site load management | Site import/transformer limits, simultaneity-factor control across chargers, the P14.15 DOE envelope interplay, and external-constraint profile composition [S-141][S-140] | `crates/flexcore-domain/src/ev_fleet_model.rs` (site transformer limit); P14.15 DOE interface; P17.05 composer |
| P17.12 | ISO 15118 PnC certificate management design (PHASE-GATED) | V2G Root CA trust store, OCPP certificate flows (Get15118EVCertificate, CertificateSigned, InstallCertificate, GetCertificateChainStatus for 2.1; block letters UNVERIFIED against the OCA text), OCSP validation; implementation GATED on PKI operations maturity (P14.03 interplay — no CA is stood up anywhere in this program) [S-128][S-130][S-125] | P14.03 record; P17.03 flows; decision record |
| P17.13 | V2G readiness modeling | Bidirectional capability per EVSE/vehicle in the domain model (the five-condition `V2xConfig` gate already exists); OCPP 2.1 bidirectional/DER hooks (NotifyAllowedEnergyTransfer, AFRRSignal, SetDERControl) as design records; the North-American AC path (IEEE 2030.5 V2G-AC / SAE J3072 / UL 1741 SC) recorded as a SEPARATE future integration [S-125][S-4][S-131][S-144] | `crates/flexcore-domain/src/ev_fleet_model.rs` (`V2xConfig`); decision records; ADR-010 |
| P17.14 | OCPP conformance test suite (written-unrun) | OCA conformance use-case matrix subset (provisioning, transactions, smart charging, security events, firmware status) plus negative tests (malformed frames, subprotocol mismatch, certificate failures); execution and certification are human lab gates [S-121][S-127] | P17.01 framing; P17.03 flows; written-unrun test files |
| P17.15 | Simulated charge points | `flexcore-simulator` extension: simulated OCPP charge points speaking OCPP-J against the CSMS adapter (hardware-free testing; fulfills the P14.17 promise that named the OCPP counterpart for Phase 17); V2G behavior profile linkage added — the existing EVSE profile is charging-only, and the addition is honestly labeled synthetic | `crates/flexcore-simulator/src/profiles.rs`, `scenario.rs`; `crates/edge-agent/src/adapters/simulated.rs` pattern |
| P17.16 | AFIR/Eichrecht compliance assessment record | EU hooks (MeterValuesSign, ad-hoc payment awareness, price-per-kWh display adjacency) and the scope statement: a depot fleet behind a site meter is adjacent, not directly AFIR-scoped — with the conditions that would change the verdict [S-135][S-136] | Assessment record; P17.07 hooks |
| P17.17 | OCPI/OICP out-of-scope record | Roaming (OCPI 2.3.0 [S-134], OICP) recorded out of scope with revisit conditions (public CPO network operation), matching gap analysis §8 | Decision record; `../00_GAP_ANALYSIS.md` §8 |
| P17.18 | Decision records + close | P17-E-1 (OCPP editions baseline), P17-E-2 (CSMS hosting boundary), P17-E-3 (telematics approach: vendor-agnostic port first, named vendor adapters later); consistency audit; the D5 interplay note (OCPP 2.0.1 is the CANDIDATE first field protocol D5 would name — D5 itself stays human-closed) | Phase `records/` (created at implementation time); `ledger/MASTER_LEDGER.md` D5 row |

## 3. Implementation wave plan (2 agents)

| Wave | Agent A | Agent B |
|---|---|---|
| 1 | P17.01–P17.05 (CSMS core + composer) | P17.06–P17.11 (ingestion, constraints, depot) |
| 2 | P17.12–P17.15 (PnC design, V2G readiness, conformance suite, simulator) | P17.16–P17.18 (compliance records, decisions, close) |

Disjoint ownership:

- **Wave 1, Agent A** owns the proposed `crates/adapters/protocol-ocpp/`
  crate (the P17.01 layout record lands first in the batch), the CSMS
  adapter architecture, the hosting-boundary design, the Profile 3
  onboarding design, the boot/status/transaction mapping, and the composer.
- **Wave 1, Agent B** owns the ingestion and constraint modules (the
  `apps/worker/src/ingestion.rs` seam extensions, domain constraint
  records), the SoC arbitration and departure-time models, the
  user-override policy, and the depot load-management modules. Agent B
  consumes A's P17.01 layout record (A lands it first; B re-reads and
  records the dependence).
- **Wave 2, Agent A** owns the PnC design record, the V2G readiness
  records, the written-unrun conformance suite, and the
  `crates/flexcore-simulator/src/` extensions (P17.15).
- **Wave 2, Agent B** owns the AFIR/Eichrecht and OCPI/OICP records, the
  three E-decision records, the consistency audit, and the close —
  documentation and records only, no file overlap with Agent A.
- Shared files across waves: the workspace manifest (member additions) and
  `tools/check_dependency_rule.py` (additive checker entries), edited as
  single-line additive entries within each agent's own batch with a re-read
  before edit and the dependence recorded. The EVSE registry extension
  (P17.04, Agent A) and the constraint records (P17.06, Agent B) are
  separate new files — no overlap.

## 4. Decision points (registered OPEN — agents never close E-decisions)

### P17-E-1 — OCPP editions baseline

- **Question:** which OCPP editions does the CSMS adapter speak?
- **Options:** (a) `ocpp2.0.1` baseline with `ocpp2.1` additive
  compatibility (2.1 is backward compatible per the Phase 6 research);
  (b) 2.0.1 only; (c) a 1.6 + 2.0.1 dual stack (1.6 is NOT backward
  compatible with 2.0.1).
- **DEFAULT (RECOMMENDATION):** (a).
- **What closes it:** D5 (human) naming the OCPP EDITION as well as the
  version — the master-ledger D5 row requires exactly that; the "Edition 2"
  publication-date conflict in `ledger/phase-06-edge-protocol/RESEARCH.md`
  §3 must resolve first (open verification task).

### P17-E-2 — CSMS hosting boundary

- **Question:** does the CSMS connection live in the control plane or in
  the edge agent?
- **Options:** (a) edge-agent-hosted for depot sites (offline tolerance;
  store-and-forward buffer reuse), control-plane-hosted elsewhere;
  (b) control-plane-hosted only; (c) fully per-site configurable.
- **DEFAULT (RECOMMENDATION):** (a) — depot sites must tolerate backhaul
  loss, and the edge agent's offline buffer and mTLS uplink already exist.
- **What closes it:** the human owner naming the first depot site and its
  connectivity profile.

### P17-E-3 — Telematics approach

- **Question:** which vehicle-telematics integrations, in what order?
- **Options:** (a) a vendor-agnostic port first (staleness rules specified
  at the port; no vendor named), named vendor adapters later on demand;
  (b) a named vendor first (e.g., Tesla Fleet Telemetry [S-137]);
  (c) OCPP-only SoC (no telematics seam).
- **DEFAULT (RECOMMENDATION):** (a) — vendor APIs carry commercial terms
  that are human-gate inputs, and the port pattern (`crates/flexcore-ports`)
  keeps vendors out of the core.
- **What closes it:** the human owner naming the first fleet vendor whose
  API terms are accepted.

## 5. Risks

| # | Risk | Treatment |
|---|---|---|
| 1 | OCPP 2.1's additive surface keeps growing (bidirectional power, DER control, battery swapping) — building against a moving target [S-123][S-125] | 2.0.1 baseline (P17-E-1); 2.1-only features are design records, never half-implementations; every secondary-sourced message detail carries an UNVERIFIED label |
| 2 | CSMS-at-edge vs cloud changes the security story: certificate custody, security-event handling, and depot attack surface differ by hosting [S-126][S-127][S-133] | P17-E-2 decides the boundary before implementation deepens; Profile 3 identity proofing and security-event ingestion are required in either shape |
| 3 | Telematics vendor terms are commercial contracts, not code | P17-E-3 vendor-agnostic port first; no vendor promised; terms acceptance recorded as a human gate input |
| 4 | Conformance testing needs labs — OCA/DEKRA-class processes, fees, human sign-off | P17.14 is written-unrun only; conventions §7.3; zero conformance claims anywhere |
| 5 | The 0–10 base is WRITTEN-UNRUN: the adapter is written against a workspace that has never compiled | Same treatment as every phase (program README, hard status boundary 1); records describe written code, never observed behavior |
| 6 | Composition honesty: a charge point returning "Accepted" has stored the profile, not necessarily applied it | P17.05 records the stored-not-applied rule and reconciles against GetCompositeSchedule; mismatches surface as named records, never silently |

## 6. Acceptance criteria (honest)

1. Every subtask has a decision record under `records/` (created at
   implementation time) citing the files touched and the [S-n] sources for
   every external claim.
2. The deliverables are adapter design + message mappings + composer
   specification + test files written-unrun + simulator profile
   specifications + assessment/decision records — **no charge point is ever
   connected, no OCPP message is ever exchanged with a real device, no
   certification is claimed.**
3. The command-relay-only rule holds: charging-profile payloads exist only
   downstream of the 14-state lifecycle's gates; the never-granted set in
   `crates/flexcore-kernel/src/manifest.rs` is byte-identical before and
   after; adapters get dispatch reach only through the command lifecycle
   (conventions §6.4).
4. `python -I tools/check_dependency_rule.py` stays green after every batch;
   `python tools/verify_docs.py` and `python tools/validate_agent.py` report
   no failures; every new crate is an additive workspace member.
5. P17-E-1, P17-E-2, and P17-E-3 remain OPEN with DEFAULTs recorded; D5
   remains OPEN and human-closed; no conformance, certification, metrology,
   or safety claim appears anywhere.

## 7. Execution notes for implementers

1. Decision records go under `phase-17-ev-fleet-charging/records/`, created
   at implementation time (conventions §2.4), numbered in batch order.
2. New crates follow conventions §6.2 — P17.01 names the
   `crates/adapters/protocol-ocpp/` layout as a proposal; code lands only in
   a batch that gives it real code.
3. The CSMS adapter's scope declaration mirrors
   `crates/flexcore-grid-api/src/scope.rs` exactly: no Default impl, a
   mandatory non-empty `not_modelled` list, fail-closed refusal outside
   scope.
4. Charging profiles are commands: they enter the lifecycle at PROPOSED and
   pass all three gates like any other candidate. An adapter that constructs
   APPROVED or ISSUED from external input is a defect, not a shortcut.
   User-override precedence (P17.10) resolves before issuance — never inside
   the adapter.
5. Message-detail honesty (the `openems.rs` UNVERIFIED-ASSUMPTION pattern):
   every OCPP field name, block letter (A–P plus the 2.1 additions), and
   profile-purpose behavior cited from secondary sources [S-124][S-125]
   carries an UNVERIFIED label naming the implementation-time check against
   the OCA specification text; a mismatch is a code or record edit, never a
   silent reinterpretation.
6. Simulator-first: P17.15's simulated charge points land in the same
   program as the adapter they exercise; hardware and site claims belong to
   human lab gates entirely.
7. Every batch ends with the three permitted checks quoted verbatim in the
   batch's decision record (conventions §1.3, §6.1) — no "verified" without
   output (AGENTS.md §3 rule 7).
