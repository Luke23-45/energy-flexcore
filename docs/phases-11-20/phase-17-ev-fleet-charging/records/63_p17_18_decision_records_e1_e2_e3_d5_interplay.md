# P17.18 (decision records) — P17-E-1, P17-E-2, P17-E-3 verified registered OPEN with DEFAULTs — the consolidated register record and the D5 interplay note (no E-decision and no D-decision is closed)

| Field | Value |
|---|---|
| Subtask | P17.18, decision-records half (P17-E-1 OCPP editions baseline, P17-E-2 CSMS hosting boundary, P17-E-3 telematics approach; agents record options and defaults, close none; the D5 interplay note) |
| Wave / agent | Phase 17 wave 2, Agent B (P17.16–P17.18) |
| Status | REGISTER COMPILATION ONLY — this record closes nothing. **All three E-decisions are OPEN at close time and D5 remains OPEN and human-closed; every closer named below is the human owner** (conventions §4). No OCPP edition is named, no depot site is named, no fleet vendor's terms are accepted (all EMPTY / NOT-STARTED). |
| Sources | `plan.md` §4 (the full E-decision text — READ-ONLY, not edited), `ledger_phase_17.md` §Decision register (READ-ONLY, not edited), `ledger/MASTER_LEDGER.md` row D5 (READ-ONLY), `ledger/phase-06-edge-protocol/RESEARCH.md` §3 (the Edition-2 open verification task — cited, never resolved); the implementing records 01, 02, 23 |
| Files touched | This record only. `plan.md`, `ledger_phase_17.md`, `context.md`, `ledger/MASTER_LEDGER.md`, and every other file are untouched by this subtask. |

## 1. The register compilation (what was checked, what was found)

Each E-decision was verified registered in BOTH places the conventions
require it to live — the phase plan's decision section (§4) and the phase
ledger's decision register — as OPEN with its DEFAULT and its named closer,
and each DEFAULT was implemented ONLY because a subtask explicitly said so,
with the dependence recorded:

| ID | `plan.md` §4 | `ledger_phase_17.md` register | Implementing record(s) | Verdict |
|---|---|---|---|---|
| P17-E-1 (OCPP editions baseline) | OPEN entry, options (a)–(c), DEFAULT (a) `ocpp2.0.1` baseline with `ocpp2.1` additive compatibility | OPEN row, DEFAULT matches | `records/01_p17_01_ocpp_csms_adapter_architecture.md` §8: the (a) shape is implemented in the subprotocol negotiation — `ocpp2.0.1`/`ocpp2.1` accepted, `ocpp1.6` refused WITH the P17-E-1 reason attached; 2.1-only features remain design records, never half-implementations | VERIFIED — registered, OPEN, no agent closed it |
| P17-E-2 (CSMS hosting boundary) | OPEN entry, options (a)–(c), DEFAULT (a) edge-agent-hosted for depot sites, control-plane-hosted elsewhere | OPEN row, DEFAULT matches | `records/02_p17_02_csms_hosting_boundary.md` §2: both shapes designed; the (a) shape implemented as `CsmsPlacement::depot_default` with the structural store-and-forward requirement; option (c) expressible per site but no per-site registry built | VERIFIED — registered, OPEN, no agent closed it |
| P17-E-3 (telematics approach) | OPEN entry, options (a)–(c), DEFAULT (a) vendor-agnostic port first, named vendor adapters later on demand | OPEN row, DEFAULT matches | `records/23_p17_08_soc_arbitration.md` §2: the (a) shape implemented as `SocSource::TelematicsPort` with declared staleness rules; no vendor named, no vendor promised, no API terms accepted (a human gate) | VERIFIED — registered, OPEN, no agent closed it |

**No register entry was missing and none was edited** — plan §4 and the
ledger register carry the three E-decisions exactly as written at planning;
the plan and ledger are outside every agent's edit scope and are
byte-identical to their planning state. No other E-decision arose in phase
17 implementation (checked: no record introduces an `P17-E-4+` id).

## 2. What closes each E-decision (the consolidated closer statement)

**In every case the closer is the human owner; no agent, plugin, or
automation can close any of the three** (conventions §4; program-level
decisions belong to the human owner).

| ID | Options (verbatim classes from plan §4) | DEFAULT (label) | What closes it (human-gate inputs — ALL EMPTY / NOT-STARTED at close) | What closure unblocks |
|---|---|---|---|---|
| P17-E-1 | (a) `ocpp2.0.1` baseline + `ocpp2.1` additive; (b) 2.0.1 only; (c) 1.6 + 2.0.1 dual stack (1.6 NOT backward compatible) | **DEFAULT (a)** — implemented as the adapter's accepted subprotocol set | **D5 (human)** naming the OCPP EDITION as well as the version — the master-ledger D5 row requires exactly that; the Edition-2 publication-date conflict (§3 below) must resolve first | Concretizing the conformance target (P6.19-class) and widening the subprotocol set if the human decision names something other than (a) |
| P17-E-2 | (a) edge-agent-hosted for depot sites, control-plane-hosted elsewhere; (b) control-plane-hosted only; (c) fully per-site configurable | **DEFAULT (a)** — implemented as the depot placement | The human owner naming the FIRST DEPOT SITE and its connectivity profile | The per-site placement registry (a composition-root concern once sites exist) and the depot security-posture review |
| P17-E-3 | (a) vendor-agnostic port first; (b) a named vendor first; (c) OCPP-only SoC | **DEFAULT (a)** — implemented as the telematics port | The human owner naming the FIRST FLEET VENDOR whose API terms are accepted (commercial terms are human-gate inputs, conventions §5) | A named vendor adapter implementing the port; until then the port has no implementation and no vendor is promised |

## 3. The D5 interplay note (said once, exactly — nothing closes D5)

OCPP 2.0.1 is the **CANDIDATE** first field protocol that D5 would name;
phase 17's adapter design, message mappings, and written-unrun suites are
the **EVIDENCE** D5 needs — they are not a closure. The master-ledger row
(`ledger/MASTER_LEDGER.md`, row D5: "First protocol and asset mix", gate
"Before v0.4.0", owner "Product + integration", status **OPEN**, ledger
owner "P6.01 (must name OCPP *edition* as well as version); P9.04") is the
authority; this record only carries the interplay note the phase plan §1
commits to:

1. **D5 itself stays OPEN and human-closed.** Nothing in phase 17 — no
   record, no code, no agent — closes D5, names the protocol, or selects
   the edition. D-decisions are human-closed.
2. **The Edition-2 publication-date conflict must resolve FIRST.** Per
   `ledger/phase-06-edge-protocol/RESEARCH.md` §3: secondary sources report
   an Edition 2 of OCPP 2.1 (and 2.0.1) published in 2026 with parts 5/6
   carrying certification profiles and an April 2026 errata — one secondary
   source dates Edition 2 to 8 June 2026, another to December 2025; the
   dates CONFLICT, and the research ledger's open verification task requires
   confirming against the Open Charge Alliance the exact Edition 2
   publication date, the parts 5/6 contents, and the errata — **before D5
   selects OCPP**. That task is CITED here and NOT resolved: no publication
   date is asserted anywhere in phase 17, and no agent performed the
   verification (it is an open verification task owned by the human
   reviewers).
3. **P17-E-1's closure is downstream of D5, not a substitute for it.** When
   the human closes D5 naming the edition and version, P17-E-1's register
   entry resolves with it (or, if D5 names a different protocol shape, the
   adapter's subprotocol set changes as a reviewed, recorded code edit).

## 4. Honest boundaries

1. This record compiles and verifies the register; it is not a decision.
2. The closer inputs (edition naming via D5, first depot site, first fleet
   vendor terms) are all human-gate inputs recorded EMPTY / NOT-STARTED.
3. The Edition-2 dates in §3 are quoted from the research ledger's recorded
   findings (secondary sources with conflicting dates) — none is asserted
   as fact here, matching the research ledger's own "do not cite a single
   Edition 2 date as fact" instruction.
4. No register file was edited; the verification was by direct inspection
   of `plan.md` §4 and `ledger_phase_17.md` (both READ-ONLY for every
   agent).
