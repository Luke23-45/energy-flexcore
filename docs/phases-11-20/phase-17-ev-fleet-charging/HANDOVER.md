# HANDOVER — Phase 17: EV Fleet and Charging Integration

Status: **implementation complete (expected 18/18), everything WRITTEN-UNRUN.**
The ledger's P17.01–P17.11 ticks are applied (by the orchestrator, citing
records 01–05 and 21–26); the P17.12–P17.18 ticks are the ORCHESTRATOR's
action, applied after this close (program conventions §2 — no agent ever
ticks a checkbox), expected to cite records 41–44 and 59 (wave 2, Agent A),
61–63 and 65 (wave 2, Agent B), and the close record 64. The phase gate is
**NOT MET** as a runtime claim: no charge point is connected, no OCPP
message was ever exchanged with a real device, no certificate was issued, no
CA exists, no V2G session ever existed, no conformance/certification/
metrology/safety claim is made anywhere, no test has run — the first human
build of the Phase 0–10 workspace is the verification gate for every
statement below.

## What was implemented (all WRITTEN, never run)

| Area | Subtasks | Records | Delivered (written) |
|---|---|---|---|
| CSMS core + composer (wave 1, Agent A) | P17.01–P17.05 | 01, 02, 03, 04, 05 | NEW crate `crates/adapters/protocol-ocpp/` (`flexcore-ocpp-adapter`): OCPP-J framing codec (CALL 2 / CALLRESULT 3 / CALLERROR 4, closed action vocabulary, whole-refusal), one-outstanding-CALL discipline, both-direction initiation, subprotocol negotiation (`ocpp2.0.1`/`ocpp2.1`; `ocpp1.6` refused with the P17-E-1 reason), session state machine, server skeleton with the designed UNRUN `CsmsTransport` seam; both CSMS hosting shapes designed (P17-E-2 DEFAULT edge-agent-hosted for depots implemented as `CsmsPlacement::depot_default`); Profile 3 onboarding (certificate-flow ledger with EXTERNAL issuance, identity proofing fail-closed, security-event ingestion as records-never-reactions); boot/status/heartbeat/transaction → domain EVSE registry extension with the CONNECTION-LOSS-IS-NOT-UNAVAILABILITY rule; the smart-charging composer in the domain (purpose selection, stack precedence, minChargingRate to-zero rule, W/A conversion with verbatim formulas) + the lifecycle-typed ISSUED-only relay and the stored-not-applied / GetCompositeSchedule reconciliation |
| Ingestion + constraints + depot (wave 1, Agent B) | P17.06–P17.11 | 21, 22, 23, 24, 25, 26 | vehicle-proposal ingestion (NotifyEVChargingNeeds/Schedule, NotifyChargingLimit/ClearedChargingLimit → constraint records, structurally pinned PROPOSED entry — proposals never auto-commands); MeterValues ingestion (cadence config, measurand mapping onto the frozen six-state vocabulary, non-silencing rule) with the opaque MeterValuesSign hook seam; dual-source SoC arbitration (plug-state ground truth, OCPP-authoritative-while-plugged, per-source staleness, stale-when-sleeping; P17-E-3 DEFAULT vendor-agnostic port); departure-time constraints (HARD policy consumed, tightening-only mapping); user-override precedence (resolves BEFORE issuance, adapters never adjudicate — structural negative tests); depot load management (site import cap = min(transformer, DOE), simultaneity shaping, external-constraint profile) |
| PnC design + V2G readiness + conformance + simulator (wave 2, Agent A) | P17.12–P17.15 | 41, 42, 43, 44, 59 | P17.12 RECORD-ONLY/PHASE-GATED: the ISO 15118 PnC trust model FlexCore would CONSUME (anchor labels only, external issuance, no CA — the five-condition ungate checklist, all NOT-STARTED); V2G readiness modeling (NEW `ev_v2g_readiness.rs` consuming the frozen five-condition `V2xConfig` gate — gate consumed, never redefined; OCPP 2.1 hooks NotifyAllowedEnergyTransfer / AFRRSignal / SetDERControl as DESIGN records with UNVERIFIED labels, NOT adapter actions; the NA 2030.5 V2G-AC path a SEPARATE future integration; ADR-010: readiness never authorizes dispatch); the 23-row OCA use-case matrix SUBSET + negative family as a WRITTEN-UNRUN suite (`conformance.rs` + `tests/conformance_suite.rs` — evidence preparation, never a result); simulated charge points (`sim_ocpp.rs`) wired THROUGH the `CsmsTransport` seam as an in-process loopback with the deterministic depot script and the SYNTHETIC V2G behaviour profile gated by the five-condition gate (the real WebSocket/TLS transport stays the human gate) |
| AFIR/OCPI records + decisions + close (wave 2, Agent B) | P17.16–P17.18 | 61, 62, 63, 64, 65 | the AFIR/Eichrecht assessment (verdict: a depot fleet behind a site meter is ADJACENT, not directly AFIR-scoped; the three EU hooks assessed at addressability level; four verdict-change conditions; no metrology claim); the OCPI/OICP out-of-scope decision record (matching gap analysis §8; concrete exclusions; three revisit conditions); the consolidated E-decision register record (P17-E-1/E-2/E-3 verified OPEN with DEFAULTs in both register locations; the D5 interplay note — OCPP 2.0.1 is the CANDIDATE D5 would name, D5 stays human-closed, the Edition-2 date conflict cited never resolved); this close set with the consistency audit |

## Blockers carried (human gate inputs)

1. **The first human build** (`cargo build --workspace` → clippy →
   `cargo test --workspace`): every Rust artifact in this phase is written
   against the WRITTEN-UNRUN 0–10 base. Failures at that gate are recorded
   and fixed as code edits — never silently. The wave close-outs (06, 27,
   59, 65) carry the static-review correction lists.
2. **No charge point is connected and none can be within this program**: no
   real WebSocket/TLS transport exists (the `UnwiredCsmsTransport` seam
   refuses everything by design); the only OCPP traffic anywhere is the
   in-process simulated loopback (record 44). Any wording implying device
   connectivity, conformance, or certification is a compliance failure.
3. **The three E-decisions are OPEN** (P17-E-1, P17-E-2, P17-E-3 — record
   63 compiles the register; every closer is the human owner).
4. **D5 is OPEN and human-closed** (`ledger/MASTER_LEDGER.md` row D5): the
   Edition-2 publication-date conflict in
   `ledger/phase-06-edge-protocol/RESEARCH.md` §3 must resolve first
   (open verification task — cited in record 63 §3, resolved by nobody).
5. **The PnC gate is NOT-STARTED** (record 41 §3): a named PKI operator,
   anchor distribution, an OCSP service, the OCA-text pass, and D5 — none
   exists; no CA is stood up anywhere in this program.
6. **The OCA-text verification pass** over every UNVERIFIED register in the
   phase records (01/03/04/05/21/22/23/26/41/42/43) is a human
   implementation-time gate; a mismatch is a recorded edit, never a silent
   reinterpretation.
7. **Every test is WRITTEN-UNRUN** (the framing/adapter suites, the
   composer suite, the domain module suites, the conformance suite, the
   simulator loopback suite): execution is the human gate.

## Expected orchestrator citations (ledger ticks, applied AFTER this close)

| Ledger row | Expected citation (decision record) |
|---|---|
| P17.12 | `records/41_p17_12_pnc_certificate_management_design.md` |
| P17.13 | `records/42_p17_13_v2g_readiness_modeling.md` |
| P17.14 | `records/43_p17_14_ocpp_conformance_suite.md` |
| P17.15 | `records/44_p17_15_simulated_charge_points.md` |
| P17.16 | `records/61_p17_16_afir_eichrecht_compliance_assessment.md` |
| P17.17 | `records/62_p17_17_ocpi_oicp_out_of_scope.md` |
| P17.18 | `records/63_p17_18_decision_records_e1_e2_e3_d5_interplay.md` (decision-register half) and `records/64_phase_17_close_and_consistency_audit.md` (close/audit half); the recommended tick citation for the row is the close record 64, with 63 named alongside |

(The wave-2 record numbering landed as 41–44 + close-out 59 for Agent A —
the plan's anticipated "records 41–59" band compressed to one record per
subtask plus the close-out, the same shape wave 1 used — and 61–63 + 65 for
Agent B, with 64 the close record. Records 45–58 and 66–79 remain free.)
