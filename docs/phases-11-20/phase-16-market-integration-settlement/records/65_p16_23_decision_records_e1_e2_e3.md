# P16.23 — Decision records: P16-E-1, P16-E-2, P16-E-3 registered OPEN with DEFAULTs — the consolidated register record (no E-decision is closed)

| Field | Value |
|---|---|
| Subtask | P16.23 (Decision records — P16-E-1 first market target, P16-E-2 event vocabulary v2, P16-E-3 settlement engine build-vs-buy; agents record options and defaults, close none) |
| Wave / agent | Phase 16 wave 2, Agent B |
| Status | REGISTER COMPILATION ONLY — this record closes nothing. **All three E-decisions are OPEN at close time; every closer named below is the human owner** (conventions §4). No jurisdiction is named, no budget exists, no vendor is selected (all EMPTY / NOT-STARTED). |
| Sources | `plan.md` §6 (the full E-decision text), `ledger_phase_16.md` §Decision points (the register), `context.md` §3 (constraints); [S-26] (FERC 2222 explainer — timeline class), [S-97] (CAISO Tariff §11 — live Nov 2024), [S-103] (ISO-NE Order 2222 readiness — effective Nov 2026), [S-90] (PJM Feb 2027/28 class), [S-100] (ERCOT ADER posture), [S-93]/[S-107] (the settlement format classes), [S-118] (revenue-share/settlement blueprint class) — all as recorded data with the re-verify-at-use rule |
| Files touched | This record only. `plan.md`, `ledger_phase_16.md`, `context.md`, and every other file are untouched by this subtask. |

## 1. The register compilation (what was checked, what was found)

Each E-decision was verified registered in BOTH places the conventions require it to
live — the plan's decision section (§6) and the ledger's decision register — as
OPEN with its DEFAULT and its named closer, and each DEFAULT was implemented ONLY
because a subtask explicitly said so, with the dependence recorded:

| ID | `plan.md` §6 | `ledger_phase_16.md` register | Implementing record(s) | Verdict |
|---|---|---|---|---|
| P16-E-1 (first market target / jurisdiction record pack) | OPEN entry, options (a)–(e), DEFAULT "paper-trading + ONE jurisdiction record pack — CAISO or ISO-NE"; live connections explicitly out of this program | OPEN row, DEFAULT matches | No record pack was built (no jurisdiction named): the paper-trading half of the DEFAULT is implemented as the P16.20 simulator (record 62) with ZERO connectivity; the ONE-record-pack half is NOT implemented — it cannot be, absent the owner's naming (D1-class) | VERIFIED — registered, OPEN, no agent closed it |
| P16-E-2 (market events + market.* actions vs the frozen surfaces) | OPEN entry, options (a)/(b)/(c), DEFAULT (a) additive v2 vocabulary + registered market.* actions | OPEN row, DEFAULT matches | `records/64_p16_22_market_v2_vocabulary_and_market_actions.md` compiles the implemented DEFAULT (the v2 module + the 6 market.* actions); `MARKET_EVENTS_V2_PROPOSAL_STATEMENT` pinned in code | VERIFIED — registered, OPEN, no agent closed it |
| P16-E-3 (settlement engine build vs buy) | OPEN entry, options (a)/(b)/(c), DEFAULT (a) build-by-spec with synthetic fixtures; revisit buy at first live jurisdiction | OPEN row, DEFAULT matches | The DEFAULT is implemented as the P16.09 parser specifications + synthetic-fixture grammars + reconciliation engine (record 23, Agent A's wave-1 store interface consumed by citation) and their P16.21 test suites (record 63) — no vendor engine, no vendor contact, no budget | VERIFIED — registered, OPEN, no agent closed it |

**No register entry was missing and none was edited** — the plan and ledger carry
the E-decisions as written at planning; nothing in this phase closes any of them.

## 2. What closes each E-decision (the consolidated closer statement)

**In every case the closer is the human owner; no agent, plugin, or automation
can close any of the three** (conventions §4; program-level decisions belong to
the human owner).

| ID | Options (verbatim from plan §6) | DEFAULT (label) | What closes it (human-gate inputs — ALL EMPTY / NOT-STARTED at close) | What closure unblocks |
|---|---|---|---|---|
| P16-E-1 | (a) CAISO record pack (live Nov 2024 [S-97]); (b) ISO-NE (Nov 2026 [S-103]); (c) PJM (Feb 2027/28 [S-26][S-90]); (d) ERCOT ADER posture [S-100]; (e) none — paper-trading only | **DEFAULT: paper-trading + ONE jurisdiction record pack — CAISO or ISO-NE given the live/newest timelines; live market connections are explicitly OUT of this program regardless** | The owner naming the jurisdiction (the D1-class decision) after reviewing the P16.18 register (Agent A, wave 2) and the sourced timelines — every timeline is recorded DATA with the re-verify-at-use rule, never a schedule | Building the ONE jurisdiction record pack (registration records, settlement pack, export posture) against a named counterparty class — and NOTHING else: live connectivity stays outside the program even after closure |
| P16-E-2 | (a) additive v2 vocabulary + registered market.* actions; (b) extend the frozen 26 in place (REJECTED — the vocabulary is frozen); (c) out-of-band side channel (REJECTED — breaks event/idempotency discipline) | **DEFAULT: additive v2 vocabulary + registered market.* actions (implemented as the P16.22 proposal)** | The owner reviewing the compiled proposal surface (record 64 §2: the 8 v2 event names + payloads, the 6 market.* actions + grants) against the versioning discipline — closure ratifies the lists (possibly with amendments, which land as v2-schema-version changes) | Wiring producers for ratified events (none exists today) and any API-surface routing that consumes the market.* actions |
| P16-E-3 | (a) build parsers/reconciliation by spec with synthetic fixtures; (b) license a settlement vendor engine; (c) defer until E-1 closes | **DEFAULT: build-by-spec with synthetic fixtures now; revisit buy when a live jurisdiction exists** | A budget/vendor decision (program-level, owner-owned) — only meaningful once E-1 names a jurisdiction and real statement samples exist to evaluate either path | The first live settlement pack implementation; until then the by-spec parsers stay validated ONLY against synthetic fixtures (no format-conformance claim — risk R-P16-2) |

## 3. Recorded decisions

1. **P16-E-1's DEFAULT is implemented EXACTLY as far as it can be without the
   owner's naming**: the paper-trading half (the P16.20 simulator, records only,
   zero connectivity) is done; the ONE-record-pack half is deliberately absent —
   building a CAISO or ISO-NE pack before the owner names the jurisdiction would
   pre-empt a D1-class decision (ADR-011; the no-hallucinated-facts rule).
2. **P16-E-2's DEFAULT is implemented as a PROPOSAL artifact**: the typed v2
   module and the registered actions exist so the review has substance; the
   register stays OPEN and the frozen 26 stay byte-identical regardless of the
   review's outcome.
3. **P16-E-3's DEFAULT required no new external dependency and no vendor
   contact**: the by-spec parsers + synthetic fixtures exist as wave-1
   specifications and wave-2 test suites; nothing was installed, nothing was
   purchased, no vendor was evaluated against real data.
4. **No settlement number, revenue figure, invoice, or payment exists anywhere**
   (plan §8 criterion 4) — the E-decision register itself carries no figures, so
   the honesty constraint holds at the register level too.

## 4. Honest boundaries

1. This record compiles and verifies the register; it is not a decision.
2. The closer inputs (jurisdiction naming, proposal review, budget/vendor
   evaluation) are all human-gate inputs recorded EMPTY / NOT-STARTED.
3. Every timeline cited in the options table ([S-26], [S-90], [S-97],
   [S-100], [S-103]) is recorded data with the re-verify-at-use rule
   (risk R-P16-1) — none is treated as a stable fact.
