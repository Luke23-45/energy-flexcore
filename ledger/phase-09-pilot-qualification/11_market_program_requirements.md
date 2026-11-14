<!-- P9.11 — Market / program requirements capture (capture TEMPLATE). No market contact exists; every answer cell EMPTY; no program rule captured from anywhere but program documentation; no universal qualification or Order 2222 compliance claim. -->
# P9.11 — Market / Program Requirements Capture (Capture Template)

Date: 2026-11-14. Subtask: P9.11 (`ledger_phase_9.md` §P9.11).
Owner role: Product (with legal for registration/agreement status).
Report trace: §23 Stage 9; §2.3; §15; §25.2 (D1); §11.4 (baseline rules are
region/product-specific).
Inputs: P9.01 freeze (TEMPLATE, UNSIGNED — `01_pilot_scope_freeze.md`,
§3.E market cell EMPTY); D1 position (OPEN — §2 below); the market operator /
program documentation for the pilot region — **which does not exist here, and
no market contact exists**; P9.04 integration design (TEMPLATE,
`04_customer_integration_design.md` §9: market interfaces BLOCKED ON D1).

## 1. Honest status — the plain statement

**No market or program is named, no operator has been contacted, no program
documentation has been obtained, and no rule has been captured.** D1 (target
jurisdiction and market) is OPEN, so even the ledger's step-1 alternative —
"record explicitly that the pilot has no market participation" — cannot be
recorded as a fact yet: whether the pilot participates is an open scope
question (P9.01 §3.E, both cells EMPTY). This record delivers the CAPTURE
TEMPLATE: the market/program identity sheet (§4), the operator question set
(§5, every answer cell EMPTY), the rule-to-adapter mapping instrument with
the §15 platform capabilities pre-cited (§6), the FERC Order 2222 per-region
status template from `RESEARCH.md` (§7), and the capture procedure (§8).

The ledger's acceptance criteria for P9.11 are NOT met and are NOT claimed:
(a) no market/program is named AND no "no participation" record exists;
(b) no baseline/measurement rule has been captured (there is no program
documentation to capture from); (c) the no-universal-qualification /
no-Order-2222-compliance rule is honoured by construction (nothing is
claimed); (d) no rule exists to map, so the mapping is a pre-cited
instrument, not a completed mapping.

## 2. D1 position — carried OPEN with DEFAULT/RECOMMENDATION labels only

| Field | Status |
|---|---|
| D1 status | **OPEN** — agent-undecided per ENTRY_NOTE boundary 2 (`.agent/DECISIONS.md`, read-only; P9.01 §3.H interim-position cell EMPTY) |
| RECOMMENDATION (DEFAULT label) | If D1 remains OPEN when the freeze is signed, record **"no market participation"** as the DEFAULT pilot posture: the pilot qualifies supervisory orchestration (P9.01 §3.D use case) without a market interface, and every §2.3 market-design failure mode (designing to a neighbouring region's rules; assuming Order 2222 liveness; choosing the baseline method for the program) is avoided by not designing a market leg at all |
| What would change it | A D1 closure naming jurisdiction + market, followed by §8's capture procedure against the program's own documentation — then §4/§5/§6/§7 fill and the note is re-issued |
| Consequence while OPEN | §4/§5/§6 answer cells stay EMPTY; the market rows in P9.04 §4/§9 stay BLOCKED ON D1; P9.12 receives no market-operator authority from this record; P9.22 (operational procedures) receives no market procedure |

## 3. Scope rule — a rule enters this note only from program documentation

Every captured rule traces to a page/section of the program's own
documentation (primary source, AGENTS.md rule 1). A rule filled from a
neighbouring region's program, from a summary document, from FlexCore design
preference, or from this template's structure is treated as UNVERIFIED and
the reviewer rejects it (§10, check 3). The baseline method is chosen by the
program, never by FlexCore (§11.4; ledger risk row).

## 4. Market / program identity sheet — ALL CELLS EMPTY

| Field | Value |
|---|---|
| Participation in scope? (P9.01 §3.E) | `<EMPTY — YES/NO>` |
| Market / program name | `<EMPTY>` |
| Region / jurisdiction (D1) | `<EMPTY>` |
| Product (energy / capacity / ancillary service / demand response) | `<EMPTY>` |
| Operator and its documentation reference (title, version, date) | `<EMPTY>` |
| Documentation obtained on (date) by (person) | `<EMPTY>` |
| If NO participation: the explicit "no market participation" record | `<EMPTY>` |

## 5. Operator question set — the capture instrument, EVERY ANSWER CELL EMPTY

Each question names why it drives FlexCore behaviour. The answer and
source-trace cells are filled ONLY from the program documentation during the
§8 procedure.

| # | Question to the operator / in the program rules | Why it drives FlexCore | Answer | Source trace (page/§) |
|---|---|---|---|---|
| Q1 | Settlement basis: settled per interval or instantaneous; settlement interval length; settlement data format and schema | determines what market records the platform must store (§16.3 market records class — no store exists, `10_data_residency_retention.md` §5.C) and what settlement data preparation P9.22 must document | `<EMPTY>` | `<EMPTY>` |
| Q2 | Telemetry cadence and quality: required metering/measurement rate; revenue-grade meter required or platform telemetry acceptable; which data-quality states are acceptable for settlement (§10.4 six-state vocabulary) | determines ingest configuration and which telemetry a settlement use may consume (per-use-case state acceptance, `04_customer_integration_design.md` §8) | `<EMPTY>` | `<EMPTY>` |
| Q3 | Baseline methodology: who computes the baseline; the method (e.g. CBL or jurisdiction's method); adjustment rules; re-basing triggers | §11.4: baseline and measurement rules MUST be region/product-specific; the flexible-load model carries `baseline demand` and availability fields but the METHOD is the program's | `<EMPTY>` | `<EMPTY>` |
| Q4 | Measurement & verification: performance measurement method; meter data source of record; rounding/tolerance rules | determines whether FlexCore's telemetry chain can serve M&V or only advisory views | `<EMPTY>` | `<EMPTY>` |
| Q5 | Dispatch port/protocol: the interface the program dispatches on (OpenADR 2.0b or 3.0; ISO/RTO API; other); profile/variant; authentication | §2.3: market dispatch is a region-specific market plugin, "not a generic OpenADR mapping"; feeds the P9.04 §4 adapter row | `<EMPTY>` | `<EMPTY>` |
| Q6 | Event/program signal format: signal schema, event types, dispatch granularity, notice/lead time | determines the market-signal ingest path (§15.3 `MarketSignalReceived`) and the scheduling cycle (P9.22) | `<EMPTY>` | `<EMPTY>` |
| Q7 | Availability requirements: delivery windows; must-offer/must-run obligations; minimum availability; recovery time after a failed event | constrains scheduling and the fallback policies (P9.20) | `<EMPTY>` | `<EMPTY>` |
| Q8 | Penalties: non-performance penalties; telemetry non-delivery penalties; registration/membership obligations with fees | determines whether the pilot's control ceiling (operator-approved at most, P9.01 §3.F) can even serve the program's response expectations honestly | `<EMPTY>` | `<EMPTY>` |
| Q9 | Reporting deadlines: settlement data submission deadlines; reporting channel; corrections/amendments procedure | determines §15.2 settlement-export obligations and P9.22 procedures | `<EMPTY>` | `<EMPTY>` |
| Q10 | Registration path: aggregation registration, metering approval, interconnection prerequisites the program requires of participants | feeds P9.12 (authority/agreement tracker) and the registration-status record in §6.3 | `<EMPTY>` | `<EMPTY>` |
| Q11 | Audit rights and evidence retention: what evidence the program may audit; minimum evidence retention period | feeds the §6 row for command/audit evidence and market records in `10_data_residency_retention.md` §6 (the ledger's named risk: a market rule requiring longer retention than the contract allows) | `<EMPTY>` | `<EMPTY>` |
| Q12 | Curtailment/recall rules: program's right to recall or curtail an enrolled aggregation | affects fallback behaviour and operator procedures (P9.20/P9.22) | `<EMPTY>` | `<EMPTY>` |

## 6. Rule-to-adapter mapping — §15 platform capabilities pre-cited, rule cells EMPTY

The mapping instrument pairs each §15/§2.3 platform surface with the program
rule it would implement. The LEFT half is filled (by citation, from the
report); the RIGHT half (which rule, and whether a gap remains) is EMPTY
until the §5 capture exists.

### 6.A Platform capability by citation

| Platform capability | Citation | Status for the pilot |
|---|---|---|
| Market/program adapter category | §5.3 plugin categories; §2.3 "ISO/RTO-specific APIs — region-specific market plugin; not a generic OpenADR mapping" | NO market adapter is in scope; any adapter enters only via a P9.04 in-scope row named by the signed freeze |
| Market signal ingest | §15.3 `MarketSignalReceived` event; §15.4 JetStream durable delivery; §15.5 transactional outbox | event vocabulary and delivery semantics exist; no market-adapter producer exists |
| Market commitments surface | §15.2 `/api/v1/market-commitments` | candidate REST resource, listed as such in §15.2; no implementation is cited here |
| Telemetry export to a program | §15.2 `/api/v1/telemetry/query`; §10.4 quality states; per-use-case acceptance | query API per §15.2; settlement acceptance rules are the program's (Q2) |
| Baseline/measurement model fields | §11.4 flexible-load model (baseline demand, reducible/increasable range, response time, availability windows, rebound energy) | model fields exist; the BASELINE METHOD is the program's (Q3), never FlexCore's |
| Dispatch command lifecycle | §14.1–§14.3 protected command state machine; §14.5 dispatch is not hard real-time | the pilot control ceiling is operator-approved at most (P9.01 §3.F); no program may be promised sub-cycle or automated response |
| Settlement/market records storage | §16.3 market records class — **no store implemented** (`10_data_residency_retention.md` §5.C) | GAP — recorded, not hidden |

### 6.B Per-rule mapping — ALL CELLS EMPTY

| Program rule (from §5 capture) | Implementing adapter/procedure | Gap? (rule FlexCore does not implement) | Disposition |
|---|---|---|---|
| `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |

A rule with no implementing adapter or procedure is a recorded GAP carried
to P9.27 (ledger step 4) — it is never silently dropped and never "mapped"
to a capability that does not implement it.

### 6.C Registration / agreement status — ALL CELLS EMPTY

| Item | Status | Reference | Plan to obtain |
|---|---|---|---|
| Market/program registration | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| Aggregation approval | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| Metering approval | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |

## 7. FERC Order 2222 status note — per-region TEMPLATE (facts from `RESEARCH.md`)

Facts below are as checked 2026-11-07 (the report's research cut-off) in
`RESEARCH.md` §1; they are restated here for the template and MUST be
re-verified against primary sources at the time of use. Order 2222 is a
regulatory framework, not a protocol; it "does not create a universal market
API" (§2.3, [R1]). **No claim of market qualification or Order 2222
compliance is made or may be made for FlexCore (§0.3, §2.3).**

| RTO/ISO | Status as checked 2026-11-07 (from `RESEARCH.md` §1) |
|---|---|
| CAISO | Fully implemented; effective 1 November 2024 — the only RTO/ISO fully live as of early/mid-2026 |
| ISO New England | Energy and ancillary-services participation targeted/effective 1 November 2026 |
| NYISO | Full implementation extended to 30 April 2027 (FERC accepted the extension 30 July 2026); partial implementation in the interim |
| PJM | DER aggregation participation expected effective 1 February 2028 (energy); capacity-market date conflicting between sources — **open verification task in `RESEARCH.md`, verify against PJM's tariff before use** |
| MISO | Two-phase: Phase 1 by 1 June 2027, Phase 2 by 1 June 2029 |
| SPP | Proposed second quarter of 2030 |
| ERCOT | Not under FERC jurisdiction; not subject to Order 2222 |

| Field | Value |
|---|---|
| The D1-named region | `<EMPTY>` |
| Status of the region's implementation (from the table above, re-verified at time of use against FERC/RTO primary sources) | `<EMPTY>` |
| Consequence for the pilot scope | `<EMPTY — e.g. in a footprint where implementation is not live, a wholesale-aggregation revenue justification is not credible; the pilot scope must reflect that>` |

## 8. Capture procedure (the human steps)

1. After the D1 closure names the jurisdiction and market, obtain the
   program's own rule documentation (tariff, manual, program rules) — the
   primary source. If no such documentation exists, there is nothing to
   capture and §4 records "no participation" instead.
2. Fill §4 from the documentation's identity facts; record the document
   title, version, and date.
3. Work §5 question by question; fill ONLY from the documentation; every
   answer carries a page/§ trace. Where the documentation is silent, record
   "not specified in the documentation" — do not infer a rule.
4. Map each captured rule in §6.B; every rule gets an implementing
   adapter/procedure or an explicit gap row carried to P9.27.
5. Fill §6.C registration/agreement status from legal/registration records —
   never from intent ("planned to register" is NOT a status; use the
   not-started vocabulary of P9.12 §8).
6. Fill §7 for the D1-named region, re-verifying the RESEARCH.md row against
   FERC/RTO primary sources on the day of use.
7. File the completed note as the P9.11 evidence named in `CHECKLIST.md`
   ("Market/program requirements captured"); re-issue on any program-rule
   change.
8. Downstream unblock order: this record feeds P9.12 (market-operator
   authority rows), P9.22 (market operational procedures), and P9.28
   (acceptance where market participation is in scope). While §4 is EMPTY,
   none of these receive a market input.

## 9. Reviewer verification procedure

1. Confirm §1's plain statement is unchanged; no market/program is named
   anywhere; no "no market participation" record exists either.
2. Confirm every answer/source-trace cell in §4, §5, §6.B, §6.C, §7's field
   table is `<EMPTY>`.
3. Traceability check for a filled (future) note: every §5 answer cites a
   page/§ of the program documentation; every §6.B rule row has an
   implementation or a gap disposition; no rule originates from a summary
   document (AGENTS.md §7 rule).
4. Confirm §7 carries the RESEARCH.md facts verbatim with the 2026-11-07
   check date and the PJM open-verification caveat; confirm no Order 2222
   compliance or market-qualification claim exists anywhere in the record.
5. Confirm §2's D1 row is OPEN with DEFAULT/RECOMMENDATION labels only.
6. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the template is written; no capture has happened.

## 10. Filling commands (for the human executor)

- Fill §4/§5/§6/§7 per §8's procedure from the program documentation;
  file it beside this record and reference it from `CHECKLIST.md`.
- Fill P9.01 §3.E from the §4 result (YES → the named program; NO → the
  explicit no-participation record).
- If the capture yields an adapter requirement, hand the protocol/profile
  cells to P9.04 §4 (market/program adapter row) — P9.04, not this record,
  owns the adapter design.
- Ledger tick (only after the capture): `- [x]` on P9.11 is justified by
  acceptance criteria (a)–(d); the current `[x]` tick means **implemented,
  awaiting human build/test**, i.e. the template exists; the capture does
  not.

## 11. Open risks carried to P9.27 (this subtask's contribution)

| Risk | Basis |
|---|---|
| D1 OPEN — no jurisdiction/market; the pilot scope rests on no recorded market decision, and no capture can begin | §2; `.agent/DECISIONS.md`; P9.01 §3.E |
| No market contact exists; even the "no participation" record is unfilled, so downstream P9.12/P9.22/P9.28 have an unresolvable input | §1, §4, §8 step 8 |
| Designing to a neighbouring region's program rules — the named failure mode, structurally gated by §3's scope rule but dependent on human discipline | ledger P9.11 risk row; §3 |
| Assuming Order 2222 implementation in an RTO/ISO where it is not yet live | §7; `RESEARCH.md` §1 |
| Baseline method chosen by FlexCore instead of by the program | ledger P9.11 risk row; §5 Q3; §11.4 |
| No market-records store exists — a captured settlement requirement (Q1/Q9) would expose a real capability gap late | §6.A; `10_data_residency_retention.md` §5.C |
| PJM capacity-market date conflicting between sources — carried as an open verification task, unresolved in this record | §7; `RESEARCH.md` §1 |

Status: **WRITTEN-UNRUN** — a capture template cannot be "run"; the only
executable steps (operator contact, documentation capture, registration
status recording) have not happened and are not scheduled. No market contact
exists and none is claimed.
