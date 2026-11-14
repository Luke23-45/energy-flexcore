<!-- P9.22 — Market / product-specific operational procedures (CONDITIONAL procedure DRAFTS). D1 is OPEN and no market/program exists per P9.11: every procedure below carries an ACTIVATION CONDITION cell that is EMPTY until the P9.11 capture names a market, every operational value (windows, deadlines, baselines, signals, contacts) is EMPTY, and NO procedure is presented as active. No walkthrough against a shadow case has happened (no shadow case exists). No universal market qualification or Order 2222 compliance claim appears anywhere. -->
# P9.22 — Market / Product-Specific Operational Procedures (Conditional Drafts — no market exists)

Date: 2026-11-14. Subtask: P9.22 (`ledger_phase_9.md` §P9.22).
Owner role: Product / operations (with the customer operator).
Report trace: §23 Stage 9; §1.5; §15; §11.4.
Inputs: P9.11 market/program requirements (**capture TEMPLATE, D1 OPEN, no
market contact — `11_market_program_requirements.md` §1/§2; "P9.22 receives
no market procedure" while §4 of that record is EMPTY**); P9.01 service scope
(TEMPLATE, UNSIGNED — `01_pilot_scope_freeze.md`); P9.17 shadow experience
(NOT SCHEDULED — no site, no shadow data).

## 1. Honest status — the plain statement

**No market or program is named (D1 OPEN), no program rule has been
captured, no operator exists to walk a procedure with, and no procedure in
this record is active or may be treated as active.** What this record
delivers is the CONDITIONAL DRAFT SET for the ledger's six operational
situations (§4–§9): each procedure is written as the SKELETON the human
gate completes and activates ONLY when the P9.11 capture has named the
market/program from its own documentation. Every procedure carries:

- an **Activation condition** — "when a market/program exists per P9.11";
  the concrete condition cell is EMPTY until the P9.11 §4 identity sheet is
  filled;
- **EMPTY operational values** — every window, deadline, interval, signal
  schema, baseline method, contact, and report format is `<EMPTY>` and is
  filled ONLY from the program documentation (P9.11 §3 scope rule: a rule
  filled from a neighbouring region's program or from FlexCore design
  preference is UNVERIFIED and rejected);
- **no active status** — the register in §11 records every procedure as
  `DRAFT — INACTIVE`.

The ledger's acceptance criteria for P9.22 are NOT met and are NOT claimed:
(a) no in-scope §1.5 use case is confirmed (P9.01 §3.D EMPTY), so the
procedure SET is provisional; (b) triggers and failure paths are written,
but approval points and operational inputs are EMPTY cells; (c) no baseline
method is recorded — §11.4 places the method with the program, and no
program exists; (d) no walkthrough against a shadow case has happened —
there is no shadow case. The walkthrough (§10) and the version record
(§11) are the human gates.

## 2. D1 position — carried OPEN, inherited from P9.11

| Field | Status |
|---|---|
| D1 status | **OPEN** — agent-undecided (`.agent/DECISIONS.md`, read-only; `11_market_program_requirements.md` §2) |
| P9.11 RECOMMENDATION (DEFAULT label) | If D1 remains OPEN when the freeze is signed, the pilot posture is **"no market participation"** — in which case §4–§9 of this record stay permanently INACTIVE and §3's no-market row becomes the operative procedure |
| What would change it | The P9.11 capture (operator question set Q1–Q12 answered from program documentation) fills this record's `<EMPTY>` cells; the procedures are then re-issued at version 1.0-draft and walked with the customer operator (§10) |
| Consequence while OPEN | No dispatch window, no settlement deadline, no baseline method, no report format may be stated; no procedure may be trained against as if it were operative (P9.21 §5 receives these procedures "in the form the operator will actually use" only AFTER activation) |

## 3. Scope rule — what activates a procedure, and the no-market row

A procedure in §4–§9 ACTIVATES only when ALL of: (i) the P9.11 §4 identity
sheet names a market/program from its documentation; (ii) the specific
program rules the procedure consumes are captured in the P9.11 §5 answers
with page/§ traces; (iii) the customer operator has walked the procedure
against a real shadow case (P9.17) and the corrections are applied; (iv)
the ledger tick context records the procedure version the pilot operates
under (§11). Absent any of the four, the procedure stays `DRAFT —
INACTIVE`.

**The no-market row (pre-printed, valid today):** if the P9.11 capture
records "no market participation" (the P9.11 DEFAULT posture), then the
pilot's operational procedures are the NON-market ones — scheduling,
fallback (P9.20), incident (P9.23), backup/DR (P9.24) — and §4–§9 of this
record are filed as unused drafts with the no-participation record as the
activation-disposition. A no-participation decision is itself a recorded
decision (P9.11 §4 last row), not a silent default.

## 4. Procedure M-1: Day-ahead / intraday scheduling cycle (DRAFT — INACTIVE)

| Element | Content |
|---|---|
| Activation condition | **when a market/program exists per P9.11** — specific condition: `<EMPTY>` |
| Trigger | The program's market-signal / gate-closure schedule arrives and is accepted as a `MarketSignalReceived` event (§5 below). Signal schema, granularity, and lead time: `<EMPTY>` (P9.11 Q6) |
| Inputs | Schedules and telemetry from the pilot deployment; the §10.4 quality states each consuming use case accepts: `<EMPTY>` (filled per use case from P9.11 Q2 — a settlement-facing input never accepts `ESTIMATED`/`STALE` unless the program documentation says so) |
| Steps | 1. Receive/observe the market signal (§5 seam). 2. Confirm the signal is well-formed and not a duplicate (failure path below). 3. Build/refresh the day-ahead or intraday schedule per the program's delivery windows (P9.11 Q7) — windows `<EMPTY>`. 4. Run the schedule through the existing §14 gates. 5. Record the market commitment against the platform's commitment surface (§6, M-2). 6. File the schedule revision and its inputs. |
| Approval point | `<EMPTY>` — every dispatch approval follows the P9.08 chain (propose ≠ approve; `command.approve` TenantAdmin-only at site/asset scope); whether the program requires an ADDITIONAL approval before a commitment is recorded is a program rule (P9.11 Q8/Q10) |
| Expected outcome | A recorded, approved schedule for the declared delivery window, with the commitment tracked — window and commitment content `<EMPTY>` |
| Failure path — signal late | If the signal arrives after the program's gate-closure deadline (`<EMPTY>`, P9.11 Q6/Q9): do NOT substitute an assumed window; record the miss, notify per the program's procedure (`<EMPTY>`), and hold the cycle |
| Failure path — signal duplicated | §15.4 delivery is at-least-once: a repeated signal is matched on its program-assigned event identity (`<EMPTY>` — identity field per P9.11 Q6) and processed once; a duplicate that cannot be distinguished is held, never processed twice |
| Failure path — signal malformed | Reject and hold; a malformed signal never produces a schedule. The platform-side event vocabulary (`crates/flexcore-events/src/events.rs` :: `MarketSignalReceivedPayload`) carries correlation/tenant/portfolio/event-time only — the PROGRAM payload schema does not exist in code and is not invented here |
| Baseline/measurement (§11.4) | The method is the PROGRAM's (P9.11 Q3/Q4). FlexCore does not choose it. Cell `<EMPTY>` |

## 5. The market-signal seam — what exists (cited) and what does not

| Surface | Citation | Status |
|---|---|---|
| `MarketSignalReceived` domain event | `crates/flexcore-events/src/events.rs` :: `MarketSignalReceivedPayload` (fields: `correlation_id`, `tenant_id`, `portfolio_id`, `event_time`) and `DomainEvent::MarketSignalReceived`; subject mapping `EventArea::Market` in `crates/flexcore-events/src/subject.rs` | The event vocabulary and subject area EXIST |
| Delivery semantics | §15.4 JetStream durable, at-least-once, in-order; §15.5 transactional outbox (`crates/adapters/postgres/migrations/0007_outbox.up.sql`) | Semantics EXIST; the outbox replay rule (idempotent, §6.3/§15.5) is what M-1's duplicate path relies on |
| Market-adapter producer | §5.3 market/program adapter category; §2.3 "region-specific market plugin; not a generic OpenADR mapping" | **DOES NOT EXIST** — no market-adapter producer is in scope; any adapter enters only via a P9.04 in-scope row named by the signed freeze (`11_market_program_requirements.md` §6.A) |
| Market-records store | §16.3 market-records class | **DOES NOT EXIST** (`10_data_residency_retention.md` §5.C) — settlement evidence storage (M-4) has no implemented store today; this is a recorded GAP, not a hidden one |
| Market-commitments REST surface | §15.2 `/api/v1/market-commitments` (listed as a candidate resource) | NOT IMPLEMENTED in the routed API — `apps/control-plane/src/routes.rs` :: `stage1_router` routes tenants/portfolios/sites/assets, Phase 2–4 routers, live SSE, alarms, edge, plugins, research; there is no market-commitments route and none is claimed |
| Telemetry export to a program | `GET /api/v1/telemetry/query` and `/api/v1/telemetry/history` (`apps/control-plane/src/routes_phase2.rs`); §10.4 quality states; per-use-case acceptance | The query surface EXISTS; settlement acceptance rules are the program's (P9.11 Q2), `<EMPTY>` |

**Consequence:** M-1/M-2/M-4 are written against a seam that is one event
vocabulary plus a read-only telemetry API. The heavy lifting (adapter,
store, commitment resource) is future work gated on D1 + P9.04 + P9.11 —
recorded as open risks in §12, never assumed done.

## 6. Procedure M-2: Demand-response event receipt and response (DRAFT — INACTIVE)

| Element | Content |
|---|---|
| Activation condition | **when a market/program exists per P9.11** — specific condition: `<EMPTY>` |
| Trigger | A demand-response / program event is received on the program's dispatch port (P9.11 Q5 — port/protocol/variant `<EMPTY>`); mapped to a `MarketSignalReceived` event by the (future, D1-gated) market adapter |
| Inputs | The event payload (schema `<EMPTY>`, P9.11 Q6); the affected assets' flexibility envelopes; acceptable telemetry quality states for the response decision: `<EMPTY>` per use case |
| Steps | 1. Receive and de-duplicate the event (at-least-once, §15.4 — same rule as M-1). 2. Check the event's declared delivery window against the pilot's control ceiling: **the pilot is operator-approved control AT MOST** (v1.0.0 ladder; ENTRY_NOTE boundary 3) — no program event can move the site beyond operator-approved control, and no response may be promised at sub-cycle or automated timescales (§14.5). 3. Compute the proposed response through the existing §14 chain. 4. Obtain the operator approval per the P9.08 chain. 5. Issue per §14.1 only after APPROVED. 6. Track acknowledgement and telemetry evidence — acknowledgement is not physical delivery (§14.1; AGENTS.md §4). |
| Approval point | `<EMPTY>` (program-side) + the P9.08 chain (platform-side, cited) — both apply |
| Expected outcome | A tracked, acknowledged response inside the program's window; window `<EMPTY>` |
| Failure path — late event | If the event's notice/lead time (P9.11 Q6) is shorter than the operator-approval cycle can honestly serve, the response is recorded as NOT attempted with the reason — never attempted unsupervised and never automatable to fit the window |
| Failure path — duplicated event | Program event identity `<EMPTY>`; processed once; an indistinguishable duplicate is held and escalated to the operator, never processed twice |
| Failure path — malformed event | Reject, hold, record; the affected delivery window is reported as missed per the program's procedure (`<EMPTY>`) |
| Curtailment/recall | The program's recall/curtailment right (P9.11 Q12) is handled per `<EMPTY>`; a recall follows the same §14 chain — it is not an out-of-band write |

## 7. Procedure M-3: Peak-management action (DRAFT — INACTIVE)

| Element | Content |
|---|---|
| Activation condition | **when a market/program exists per P9.11, or when the signed P9.01 scope names peak management as the piloted §1.5 use case** — specific condition: `<EMPTY>` (P9.01 §3.D) |
| Trigger | A peak-management obligation observed in the pilot's own schedule (customer peak clause `<EMPTY>`) or a program signal (as M-2) |
| Inputs | Site/asset envelopes; telemetry with quality states acceptable for the action: `<EMPTY>`; the customer's peak clause (contract, cited) `<EMPTY>` |
| Steps | 1. Identify the peak window and the affected assets. 2. Compute the proposed action through the §14 chain (validation, feasibility, authorization — distinct gates). 3. Operator approval per P9.08. 4. Issue, track, and record tracking deviation. 5. End-of-day reconciliation (M-5) closes the record. |
| Approval point | `<EMPTY>` + P9.08 chain |
| Expected outcome | Measured peak within the recorded limit; limit `<EMPTY>` |
| Failure path | If the action cannot be approved in time, the peak is exceeded and RECORDED as exceeded — the pilot does not relax a limit to make an outcome pass; if telemetry degrades below the accepted states, the action is held (§17.5 invariant 9: a missing/uncertain/stale model is not a passing feasibility check) |

## 8. Procedure M-4: Market commitment tracking and settlement-data preparation (DRAFT — INACTIVE)

| Element | Content |
|---|---|
| Activation condition | **when a market/program exists per P9.11** — specific condition: `<EMPTY>` |
| Trigger | A commitment exists (M-1/M-2) and a settlement period closes (settlement interval per P9.11 Q1: `<EMPTY>`) |
| Inputs | The commitment records; the delivery-window telemetry (query per §5, `GET /api/v1/telemetry/query|history`); the quality states the program accepts for settlement: `<EMPTY>` (P9.11 Q2); the baseline/measurement method as the PROGRAM defines it: `<EMPTY>` (P9.11 Q3/Q4 — §11.4) |
| Steps | 1. Assemble the delivery-window evidence: commitments, approved commands, acknowledgements, telemetry (with §10.4 quality states and versioned corrections — `crates/adapters/postgres/migrations/0012_correction_versions_p2_31.up.sql`: a correction never mutates the original row). 2. Apply the program's baseline/measurement method — never a FlexCore-chosen method. 3. Prepare the settlement dataset in the program's required format/schema (P9.11 Q1: `<EMPTY>`). 4. Submit by the program's deadline and channel (P9.11 Q9: `<EMPTY>`). 5. Retain the evidence per the retention row for market records (`10_data_residency_retention.md` §6 — period and basis `<EMPTY>`; the store itself is a recorded GAP, §5 above). |
| Approval point | `<EMPTY>` — who signs a settlement submission is a program rule (P9.11 Q9/Q11) plus a customer-contract rule; both cells EMPTY |
| Expected outcome | A submitted, evidenced settlement dataset accepted by the program; acceptance criteria `<EMPTY>` |
| Failure path — late | If the dataset cannot be assembled by the deadline (`<EMPTY>`), submit per the program's corrections/amendments procedure (P9.11 Q9: `<EMPTY>`) or record the miss — never fabricate or interpolate missing telemetry; a gap is `MISSING` (§10.4), and a settlement built on fabricated values is prohibited |
| Failure path — evidence in a form the program does not accept | The ledger's named risk. Mitigation: the format cell is filled from the program documentation BEFORE the first settlement cycle, and the first settlement dataset is verified against a program-supplied sample (`<EMPTY>`) before submission |

## 9. Procedure M-5: End-of-day reconciliation (DRAFT — INACTIVE)

| Element | Content |
|---|---|
| Activation condition | **when the signed P9.01 scope names a scheduling/market use case in scope** (with or without a market) — specific condition: `<EMPTY>` |
| Trigger | The end of the pilot site's operating day (site timezone per the site record; UTC internally per §10.3) |
| Inputs | The day's schedule revisions, commands and their transitions, telemetry quality summary, any market signals/commitments, alarms |
| Steps | 1. Compare issued vs acknowledged vs measured per command (acknowledgement ≠ delivery; §14.1). 2. List every rejected/expired/superseded command with its reason. 3. List telemetry gaps (MISSING windows) and corrections applied. 4. For market days: confirm M-4 evidence is filed per commitment. 5. Record the reconciliation outcome and carry unresolved items to the incident procedure (P9.23) where they are incidents, or to the operations log where they are not. |
| Approval point | `<EMPTY>` (who signs the reconciliation — customer review cell) |
| Expected outcome | A signed daily reconciliation record; format `<EMPTY>` |
| Failure path | An unresolved discrepancy is an OPEN item with an owner — never closed by silence, never papered over by a correction row (0012's correction is versioned, not erasing) |

## 10. The walkthrough requirement (human gate) — ledger step 4

Each activated procedure is walked with the customer operator against a
REAL shadow case (P9.17) and corrected where the walkthrough diverges from
practice. **No walkthrough has happened; none is scheduled; the shadow
cases do not exist.** The walkthrough record per procedure carries:
procedure version, shadow-case reference, divergences found, corrections
applied, operator name, date, signature — all `<EMPTY — UNSIGNED>`.

## 11. Procedure register and version — the version the pilot operates under

| Procedure | Activation status | Version | Walked against a shadow case? |
|---|---|---|---|
| M-1 day-ahead/intraday scheduling cycle | DRAFT — INACTIVE | 0.1-draft (this record) | `<EMPTY — NO>` |
| M-2 demand-response event receipt and response | DRAFT — INACTIVE | 0.1-draft | `<EMPTY — NO>` |
| M-3 peak-management action | DRAFT — INACTIVE | 0.1-draft | `<EMPTY — NO>` |
| M-4 market commitment tracking + settlement-data preparation | DRAFT — INACTIVE | 0.1-draft | `<EMPTY — NO>` |
| M-5 end-of-day reconciliation | DRAFT — INACTIVE | 0.1-draft | `<EMPTY — NO>` |
| No-market row (§3) | operative ONLY upon a recorded no-participation decision (P9.11 §4) | `<EMPTY>` | `<EMPTY — NO>` |

**The version the pilot operates under: `<EMPTY>`** — the pilot is not
operating; no procedure is active. The version is recorded here only after
activation conditions (i)–(iv) of §3 are met. M-1–M-5 inherit the P9.11
re-issue rule: a program-rule change re-opens the affected procedure.

## 12. Reviewer-verification procedure (human)

1. Confirm §1's plain statement is unchanged and every procedure in §11 is
   `DRAFT — INACTIVE`; no procedure is described as operating, trained-on,
   or active anywhere in this record or in text derived from it.
2. Confirm every operational value cell (windows, deadlines, schemas,
   baseline methods, contacts, formats, acceptance rules) in §4–§9 is
   `<EMPTY>`, and that no value has been filled from a neighbouring
   region's program or from FlexCore preference (the P9.11 §3 scope rule).
3. Trace the §5 seam citations (`grep -n "<symbol>" <file>`): the
   `MarketSignalReceivedPayload` fields, `EventArea::Market`, migration
   0007, migration 0012, and the routed routes in `routes.rs`/
   `routes_phase2.rs`; confirm the record nowhere claims a market adapter,
   a market-records store, or a market-commitments route EXISTS.
4. Confirm §11.4 discipline: no baseline/measurement method is chosen by
   FlexCore anywhere in this record.
5. Confirm the control ceiling: no procedure promises automated,
   sub-cycle, or bounded-automation response (v1.0.0 ladder; ENTRY_NOTE
   boundary 3; §14.5; §21.4).
6. Confirm no universal market qualification or Order 2222 compliance
   claim appears anywhere (§0.3, §2.3).
7. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the conditional drafts are written; no procedure is
   activated and no walkthrough has run.

## 13. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) AFTER the P9.11 capture names a market (or records no participation):
#     fill the activation-condition cells (§4–§9), the operational values
#     from the program documentation ONLY (each with a page/§ trace), and
#     the baseline/measurement cells from the program's method.
# (c) Walk each activated procedure with the customer operator against a
#     real P9.17 shadow case; apply corrections; fill §10 and §11 per
#     procedure; record the version the pilot operates under.
# (d) File the register (§11) as the P9.22 evidence named in CHECKLIST.md
#     ("Operational procedures written") — the checklist item closes only
#     when the walked procedures exist, not when this draft set does.
# (e) Re-issue (version bump + revision note) on any program-rule change
#     (the P9.11 re-issue rule inherited here).
```

## 14. Open risks carried to P9.27 (this subtask's contribution)

| # | Risk | Basis |
|---|---|---|
| R-P9.22-1 | D1 OPEN — no jurisdiction/market; the procedure set rests on no recorded market decision, and even the no-participation disposition is unfilled | §2, §3; `11_market_program_requirements.md` §2/§4 |
| R-P9.22-2 | No market adapter, no market-records store, no market-commitments route exist — the settlement and commitment procedures reference seams that are one event vocabulary plus a read-only telemetry API; a late D1 closure exposes real capability work under schedule pressure | §5; `10_data_residency_retention.md` §5.C |
| R-P9.22-3 | A procedure written from the generic product manual instead of the program's rules — the ledger's named failure mode; structurally gated by the §3 activation conditions, dependent on human discipline | §3, §12 check 2 |
| R-P9.22-4 | An event signal with no defined duplicate handling — mitigated by the at-least-once rule (§15.4) pre-printed in M-1/M-2, but the program's event-identity field is `<EMPTY>` until Q6 is answered | §4, §6; P9.11 Q6 |
| R-P9.22-5 | Settlement evidence retained in a form the program does not accept — the ledger's named risk; the pre-verification step (M-4) is procedural until a human runs it against a program-supplied sample | §8 |
| R-P9.22-6 | A program whose response expectations cannot be served honestly by an operator-approved-at-most pilot (penalties for automated-speed response) — discovered only via P9.11 Q8; the procedures pre-refuse automation as a fix | §6; P9.11 Q8 |
| R-P9.22-7 | P9.21 operator training references these procedures "in the form the operator will actually use" — while the drafts are INACTIVE, that input does not exist; a training run before activation trains on drafts | §2; `ledger_phase_9.md` §P9.21 step 5 |

Status: **WRITTEN-UNRUN** — conditional procedure drafts cannot be "run";
the activation conditions (P9.11 capture), the operator walkthrough, and
the version record are the human gates and have not happened and are not
scheduled. No market exists; no procedure is active; none is claimed to be.
