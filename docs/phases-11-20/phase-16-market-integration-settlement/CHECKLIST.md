# Phase 16 — Market Integration, Settlement and Revenue: Exit-Gate Checklist

**Exit gate (plan §8, the phase's own acceptance criteria): every subtask
has a decision record; the deliverables are schemas, domain models, parser
specifications, pipeline designs, permission/decision records, and the
paper-trading simulator WRITTEN-UNRUN — no market is connected, no
registration is filed, no qualification or conformance is claimed, no bid
is submitted, no settlement number, revenue figure, invoice, or payment is
claimed anywhere; the simulator produces simulated prices and synthetic
statements ONLY (records, never connectivity); the ADR-011 boundary is
restated and honored (no generic tariff/Order-2222 engine); the frozen
26-event vocabulary and the never-granted permission set stay
byte-identical; P16-E-1, P16-E-2, and P16-E-3 remain OPEN with DEFAULTs;
the dependency checker stays green; `verify_docs.py` and
`validate_agent.py` report no failures.**

Each item states the evidence and its status. A checked item without
evidence is not checked. **Nothing in this phase has been built, run,
connected, settled, paid, or transmitted** — the first human build is the
program gate; every status below is a written-artifact status, never a
runtime claim.

## Plan §8 acceptance criteria (one-for-one)

- [x] **Criterion 1 — schemas, domain models, parser specifications,
  pipeline designs, and test files exist WRITTEN-UNRUN; no subtask claims
  a run, a test pass, or a verified behavior.** Evidence: the file
  inventory in the two close-out records (07, 47, 66); every test file
  and inline suite carries the written-unrun header; the close record §3
  audit sweep found zero run/pass/verified claims. Status: **satisfied by
  written artifacts**.
- [x] **Criterion 2 — the market simulator produces simulated prices and
  synthetic settlement statements ONLY (records, never connectivity).**
  Evidence: record 62 (the four modules' statements pinned by tests:
  `SIM_FEED_IS_SIMULATION_STATEMENT`, `SIMULATED_STATEMENTS_ARE_SYNTHETIC_
  STATEMENT`, `SIM_EXPORT_SINK_STATEMENT`, `PAPER_LOOP_STATEMENT`); no
  socket, transport, or broker dependency exists in the simulator tree
  (the only manifest change is the additive `flexcore-application`
  workspace edge, legal for a COMPOSITION crate). Status: **satisfied by
  the written design and its pinned statements**.
- [x] **Criterion 3 — no market is connected, no registration is filed,
  no qualification or conformance is claimed** (ADR-011; conventions
  §7.3). Evidence: the honesty sweep (close record §3.4) across all 26
  phase records + HANDOVER + CHECKLIST: every record carries its own
  named honesty boundary (no-market-connected, no-registration-filed,
  no-price-feed-connected, no-pipeline-transmits, no-meter-certified,
  no-dispute-exists, no-payment-moves); the register ships ZERO
  enablements; no format-conformance claim exists in either direction.
  Status: **satisfied by the audit** as of the close record's date (an
  inspection, not a tool run).
- [x] **Criterion 4 — no settlement number, revenue figure, invoice, or
  payment is claimed anywhere.** Evidence: the money-shape discipline at
  every layer (decimal TEXT + currency as data in migration 0022;
  `FixedDecimal`/`MoneyAmount` in the record layer; integer-only recipes
  in the simulator); the sweep found zero computed settlement figures;
  the payment lifecycle entered NO state; the v2 events carry counts and
  wire-faithful price TEXT only. Status: **satisfied by the audit**.
- [x] **Criterion 5 — the ADR-011 boundary is restated and honored.**
  Evidence: every record carries the jurisdiction-specificity statement;
  the market structures, settlement packs, export postures, and register
  rows are per-jurisdiction RECORDS; a fourth jurisdiction enters as a
  NEW variant/row, never a widened generic. Status: **satisfied by the
  written records**.
- [x] **Criterion 6 — the frozen 26-event vocabulary stays byte-identical;
  the never-granted set stays byte-identical; market events/actions exist
  only as the P16-E-2 proposal.** Evidence: `events.rs` untouched (the v2
  vocabulary is `market_events_v2.rs`, a NEW file whose tests pin
  `EVENT_COUNT == 26` and the disjointness of every v2 name);
  `NEVER_GRANTED_PERMISSIONS` quoted verbatim and unchanged (records 61
  §2; the kernel manifest untouched); the market.* actions carry zero
  dispatch reach (no `market.dispatch.*` action, pinned unparseable).
  Status: **satisfied by the pinned tests and the audit**.
- [x] **Criterion 7 — `verify_docs.py` and `validate_agent.py` green
  after every documentation change (results quoted, never asserted).**
  Evidence: the verbatim outputs in close-out records 07, 47, and 66, and
  the final close-record run (§6) — all green (EXIT=0 ×3). Status:
  **satisfied, quoted**.

## Subtask coverage (24/24 with decision records)

| Subtask | Record | Status |
|---|---|---|
| P16.01 | 01 | written-unrun |
| P16.02 | 02 | written-unrun |
| P16.03 | 03 | written-unrun |
| P16.04 | 04 (+ `tests/baseline_engine.rs`) | written-unrun |
| P16.05 | 05 | written-unrun |
| P16.06 | 06 (+ `tests/bid_composition.rs`) | written-unrun |
| P16.07 | 21 | written-unrun |
| P16.08 | 22 | written-unrun |
| P16.09 | 23 | written-unrun |
| P16.10 | 24 | written-unrun |
| P16.11 | 25 | written-unrun |
| P16.12 | 26 | written-unrun |
| P16.13 | 41 | written-unrun |
| P16.14 | 42 | written-unrun |
| P16.15 | 43 | written-unrun |
| P16.16 | 44 | written-unrun |
| P16.17 | 45 | written-unrun |
| P16.18 | 46 | written-unrun |
| P16.19 | 61 | written-unrun |
| P16.20 | 62 | written-unrun (simulator depth: executable-by-design, never run) |
| P16.21 | 63 (+ the two integration test files) | written-unrun |
| P16.22 | 64 | written-unrun |
| P16.23 | 65 | register compiled; E-decisions OPEN |
| P16.24 | 79 (this close) | close + consistency audit |

## Human-gate queue (NOT-STARTED / EMPTY / UNSIGNED)

1. First workspace human build + clippy + `cargo test --workspace`
   (covers every written-unrun suite, including the 32-test market
   integration suite).
2. Migration 0022 apply/down cycle (expand-only; the destructive down on
   evidence-class tables would additionally require operator sign-off).
3. P16-E-1/E-2/E-3 review and closure by the human owner (record 65 §2
   names what closes each; every closer input is EMPTY / NOT-STARTED).
4. Any future benchmark/replay run fed by the simulator registers its
   thresholds BEFORE the run (X-8); no benchmark figure exists.
