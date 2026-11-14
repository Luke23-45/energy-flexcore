# P6.01 — D5 disposition record (first protocol and asset mix)

Date: 2026-11-12. Subtask: P6.01, Phase 6 ledger (`ledger_phase_6.md` §P6.01).
Decision owner for D5: **Product + integration** (`.agent/DECISIONS.md`).
Agents do not decide D5; this record does not close it, does not select an
adapter, and creates no approval.

## 1. Status: D5 is STILL OPEN

| Field | Value |
|---|---|
| Decision | D5 — First protocol and asset mix |
| Status | **OPEN — recorded, not closed** (this file changes nothing in `.agent/DECISIONS.md`; the gate table there remains authoritative) |
| Gate | Before v0.4.0 (report §25.2; `.agent/DECISIONS.md`) |
| Owner | Product + integration |
| Venue | The Stage 0 gate venue recorded in `ledger/phase-00-product-freeze/03_decision_records.md` §5; per `.agent/DECISIONS.md`, named person + date are assigned at the Stage 0 gate meeting |
| Date | Not assigned by this record — the ledger forbids inventing owners and dates |
| ADR | None written. P6.01's step 4 offers two paths: (a) an ADR closing D5, or (b) a dated OPEN record. This is path (b). An ADR is written only when a human decision actually closes D5. |

## 2. Candidate list (P6.01 step 1 — factual state, not a scoring verdict)

Candidates and their verified state per this phase's `RESEARCH.md`
(checked 7 November 2026):

| Candidate | Asset mix it would serve | Verified state (source: `RESEARCH.md` §1–§5) |
|---|---|---|
| OpenEMS | Site EMS integration: PV, battery/ESS, meters at a pilot site | 2026.11.0 released 31 Oct 2026 (Zenodo record); Edge Natures/Channels/EdgeConfig JSON documented; Edge↔Backend auth is API-key over WebSocket, NOT mTLS (R6-C1) |
| OpenADR 3.0 | Demand-response / VTN–VEN programme participation | 3.0 normative reference is the Alliance's OpenAPI YAML; certification is Alliance-program only; 3.1 existence UNVERIFIED (`RESEARCH.md` §2, §8) |
| SunSpec Modbus | Direct DER/inverter device integration (700-series models) | Device information models, not a WAN protocol; certification via SunSpec Authorized Test Laboratories (`RESEARCH.md` §5) |
| OCPP 2.0.1 / 2.1 | EV charging fleets | 2.0.1 = IEC 63584:2024; 2.1 released Jan 2025 (secondary sources); Edition 2 dates CONFLICT between secondary sources — open verification task (`RESEARCH.md` §3, §8) |
| IEEE 2030.5 / CSIP | DER grid-interconnection functions (later adapter per §2.3) | IEEE 2030.5-2023 is the active revision; CSIP is the SunSpec test profile; explicitly a LATER adapter per report §2.3 (`RESEARCH.md` §4) |

P6.01 step 2 (scoring) is **not executed here**: the scoring criteria — named
pilot partner, lab hardware, obtainable conformance suite, license
compatibility (D4/P6.02), OT boundary (D6/P6.03) — cannot be honestly
scored while no pilot partner is named. No scoring table is fabricated.

## 3. Recommended default (RECOMMENDATION — not a decision)

The report's §2.1 v0.4.0 scope and §2.3 protocol table make OpenEMS the
priority first integration ("First site adapter through OpenEMS"), and
`.agent/DECISIONS.md` carries this as a recommended default, not a decision:
*"OpenEMS priority is not a decision."*

**RECOMMENDATION: OpenEMS first** — a single OpenEMS site-EMS adapter
serving a simulated/lab PV + battery + meter asset mix, consistent with the
v0.4.0 release-ladder row "First site integration (OpenEMS)". The OpenEMS
version would be pinned at the lab instance's actual version (candidate:
2026.11.0 per `RESEARCH.md` §1). OpenADR, OCPP, SunSpec Modbus, and
IEEE 2030.5/CSIP are recorded as not selected for this phase — the last
explicitly so per report §2.3.

This is the engineering default Phase 6's planning proceeds against. It is
wrong the moment a pilot partner or customer says otherwise
(`.agent/DECISIONS.md` rule).

## 4. Pilot site: PLACEHOLDER

No pilot partner is named. Wherever a site name is structurally required in
Phase 6 records, **`pilot-site-A`** is used and is labelled
**PLACEHOLDER** in every occurrence. `pilot-site-A` is not a site, has no
owner, no network, and no jurisdiction; it must never be read as a
commitment or as D6 input. Naming a real partner/site is a D5 input
(`CONTEXT.md` §9 open question 1; `ENTRY_NOTE.md` boundary 5).

## 5. What proceeds under the RECOMMENDATION and what is blocked on D5

Proceeds (design/implementation work against the recommended default, all
"implemented, awaiting human build/test" per `ENTRY_NOTE.md`):

- Edge agent crate, composition root, identity/enrollment design (P6.05, P6.06) — independent of which protocol wins.
- Outbound mTLS channel (P6.07) — required for every candidate protocol.
- OpenEMS-targeted adapter work (P6.12 interface/ACL, P6.13 read path) — written against the OpenEMS documented REST API (EdgeConfig), usage UNVERIFIED, never executed (`ENTRY_NOTE.md` boundary 3).
- Read path, buffering, audit, isolation, docs, CI wiring (P6.08–P6.09, P6.17–P6.18, P6.21, P6.26–P6.27).

Blocked on D5 closing (cannot honestly proceed or claim anything):

- **Naming a pilot partner/site or closing D6 for a real site** — a site/partner must be named by the D5 decision, not assumed.
- **Adapter conformance claims or conformance-suite commitments (P6.19)** — the applicable suite depends on the selected protocol; OpenADR's certification license (~US $1,250, Alliance page) is already an example of a budget-relevant access term.
- **Any statement that OpenEMS is "the selected adapter"** — it remains the RECOMMENDATION.
- **Lab/pilot site connection work (P6.24 exit-gate run against a real site)** — requires D5 closed and D6 signed.

## 6. Re-open trigger

D5 must be re-opened to review (and this record superseded by an ADR) when
any of:

1. A pilot partner is named or a real site is identified — the recommendation must be re-scored against the actual partner, its asset mix, and its jurisdiction.
2. The selected-protocol facts change — e.g. an OpenEMS release changes component IDs/channels relevant to the mapping (monthly release train, `RESEARCH.md` §1), or OCPP Edition 2 verification resolves and changes candidate scoring.
3. D4's license audit finds an incompatibility in the recommended stack (P6.02 records OpenEMS's license as UNVERIFIED).
4. D6's site-specific review shows the site's OT boundary rules out the recommended integration pattern.
5. The report's §2.3 protocol posture is revised by an approved design decision.

## 7. Honesty boundary

Nothing in this record closes D5, selects a protocol, names a partner, or
asserts any licence or conformance fact. Every protocol fact above cites
`ledger/phase-06-edge-protocol/RESEARCH.md` (checked 2026-11-07); anything
outside it would be an ASSUMPTION and is not made here. The workspace
asserts no license; D4 remains OPEN (`ledger/phase-01-rust-foundation/28_dependency_license_inventory.md`).
