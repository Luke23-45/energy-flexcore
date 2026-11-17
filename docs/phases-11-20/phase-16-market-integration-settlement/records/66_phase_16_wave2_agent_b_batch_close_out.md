# Phase 16 — Wave 2, Agent B batch close-out (P16.19–P16.23)

| Field | Value |
|---|---|
| Wave / agent | Phase 16 wave 2, Agent B |
| Subtasks | P16.19 (market adapter permission model), P16.20 (market simulator, paper-trading), P16.21 (market integration test suite), P16.22 (additive event-vocabulary v2 + market.* actions), P16.23 (decision records P16-E-1/E-2/E-3) |
| Status | WRITTEN-UNRUN — every file below is written, never compiled, never executed, never tested. The loop bans were honored in full: nothing installed, nothing built, no test run, and the banned placeholders scan (`todo!` / `unimplemented!` / `TODO` / `FIXME`) returned ZERO matches across every file this batch created. Only the three permitted verification tools were run. |
| Honesty | **No market is connected, no registration is filed, no qualification or conformance is claimed, no bid is submitted (none can be), no instruction is issued, no payment moves, and no settlement number, revenue figure, invoice, or payment exists anywhere in this batch.** The simulator produces records only, never a connection. The frozen 26-event vocabulary and the never-granted set were not modified — both are pinned byte-identical by test. No ledger checkbox was ticked (the ledger is outside this agent's ownership). |

## 1. Files created

| Layer | File |
|---|---|
| Authz actions + grants (P16.19) | `crates/flexcore-authz/src/phase16_market_policies.rs` (NEW) |
| v2 event vocabulary (P16.22) | `crates/flexcore-events/src/market_events_v2.rs` (NEW, NOT an edit of `events.rs`) |
| Simulator (P16.20) | `crates/flexcore-simulator/src/market/mod.rs`, `market/price_feed.rs`, `market/settlement_fixtures.rs`, `market/paper_loop.rs`, `market/export_sink.rs` (all NEW) |
| Application tests (P16.21) | `crates/flexcore-application/tests/market_integration_p16.rs` (NEW, written-unrun, 23 tests — including the two families added at the second wait-check against Agent A's LANDED P16.14/P16.18 modules) |
| Simulator tests (P16.21) | `crates/flexcore-simulator/tests/market_paper_loop_p16.rs` (NEW, written-unrun, 9 tests) |
| Decision records | `records/61_p16_19_market_adapter_permission_model.md`, `62_p16_20_market_simulator.md`, `63_p16_21_market_integration_test_suite.md`, `64_p16_22_market_v2_vocabulary_and_market_actions.md`, `65_p16_23_decision_records_e1_e2_e3.md`, this close-out (66) — all NEW under `docs/phases-11-20/phase-16-market-integration-settlement/records/` |

## 2. Files modified (additive only)

| File | Change |
|---|---|
| `crates/flexcore-authz/src/action.rs` | Six `Market*` variants + `id` arms + `parse` arms; both test blocks extended additively (the round-trip list, the deny list). The four never-granted names' absence is untouched and re-pinned. |
| `crates/flexcore-authz/src/lib.rs` | Additive `pub mod phase16_market_policies;` + comment |
| `crates/flexcore-events/src/lib.rs` | Additive `pub mod market_events_v2;` + comment (the module tree is otherwise untouched; `events.rs` was NOT edited) |
| `crates/flexcore-simulator/src/lib.rs` | Additive `pub mod market;` + comment |
| `crates/flexcore-simulator/Cargo.toml` | Additive `flexcore-application = { workspace = true }` — an EXISTING workspace member (no new external dependency); the composition→application edge is legal with NO checker change (composition crates are not edge-restricted; the checker was not edited) |

Not touched: `ledger_phase_16.md`, `plan.md`, `context.md`, `docs/foundations/*`, `docs/plan/*`, `.agent/*`, `tools/check_dependency_rule.py`, `crates/flexcore-events/src/events.rs` (the frozen 26), `crates/flexcore-kernel/*` (the never-granted set's home, the P14.16 rule consumed by citation), any file owned by Agent A (no A wave-2 file existed or was edited), `crates/flexcore-application/src/*` (wave-1 modules consumed by citation only — no wave-1 source file was modified).

## 3. Permitted verification tools — verbatim outputs

Run at batch close (the only commands executed in this batch besides read-only file inspection and the placeholder grep above):

```text
$ python tools/verify_docs.py
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC
EXIT=0

$ python tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid
EXIT=0

$ python -I tools/check_dependency_rule.py
OK ù dependency rule holds for 28 workspace members.
EXIT=0
```

(The `ù` character in the dependency-rule output is the tool's own console
encoding of its separator glyph on this Windows Git Bash console, quoted
verbatim as produced.)

## 4. Key recorded decisions (details in records 61–65)

1. **P16.19**: six `market.*` actions registered additively per the authz pattern (the P13 apikey.* precedent); `market.bid.compose` is a RECORD action — no bid is submitted, none can be; NO `market.dispatch.*` action exists in any spelling (the P14.16 command-relay-only rule consumed by citation); the never-granted set quoted VERBATIM and unchanged (review verdict: the market surface needs NONE of the four ids); grants at tenant scope only, SiteApprover excluded (the P13 separation stands); the per-market grant remains the P16.06 RECORD checked by the composer — the RBAC grant is one necessary condition, never a sufficient one.
2. **P16.20**: the simulated price feed implements the UNWIRED `IsoPricePublication` seam in-process; synthetic settlement statements generated IN exactly the P16.09 v1 fixture grammars; the paper loop composes the wave-1 engines (compose_bid, reconcile_bli_statement, records_from_publication) and adds exactly three simulator-local RECORD types (paper dispatch instruction, evidence record, ledger reference); integer-only money arithmetic rendered as decimal text (no float, no RNG, replay-deterministic); export receipts are a bounded in-memory ring with the same posture checks.
3. **P16.21**: 32 written-unrun tests across two files consuming the wave-1 grammars/cases/negative tests; TWO wait-checks recorded (record 63 §3): at the first, Agent A's wave-2 modules had NOT landed, so the payment-lifecycle and enablement test families were recorded as a handoff (APIs never guessed); at the second (batch close) they HAD landed (`payment_lifecycle.rs`, `jurisdiction_register.rs`, records 41–47), and the pre-committed families were written against the LANDED APIs by citation — the handoff was honored in the same batch.
4. **P16.22**: the v2 vocabulary is a NEW file with its own tag + subject scheme + schema version; the frozen 26 pinned byte-identical by test inside the v2 module itself; the compiled proposal surface (8 v2 events + 6 market.* actions) is the P16-E-2 artifact, REGISTERED OPEN.
5. **P16.23**: all three E-decisions verified registered OPEN with DEFAULTs and human closers (record 65); none closed; P16-E-1's DEFAULT implemented exactly as far as the owner's naming allows (the paper-trading half; no record pack), P16-E-2's implemented as the proposal, P16-E-3's as the by-spec parsers + synthetic fixtures.

## 5. Handoffs

**To the orchestrator (ledger ticks).** The expected tick citations for
P16.19–P16.23 are: `records/61_p16_19_market_adapter_permission_model.md`,
`records/62_p16_20_market_simulator.md`, `records/63_p16_21_market_integration_
test_suite.md`, `records/64_p16_22_market_v2_vocabulary_and_market_actions.md`,
`records/65_p16_23_decision_records_e1_e2_e3.md`; P16.24 cites the close record
`records/79_phase_16_close_and_consistency_audit.md`. Full expected-citation
table in `HANDOVER.md`.

**To Agent A (wave 2, concurrent) — wait-check reconciliation.** Two wait-checks
were performed (record 63 §3). At the second (batch close), A's wave-2 artifacts
had LANDED: `crates/flexcore-application/src/revenue_share_billing.rs`,
`payment_lifecycle.rs`, `program_event_management.rs`, `capacity_market_records.rs`
(+ `jurisdiction_register_data.rs`), `uk_eu_lfm_records.rs` (module lines under
the recorded names in `flexcore-application/src/lib.rs`), and records 41–47
under `records/`. No A file was edited; no A module line was touched; the
P16.21 payment-lifecycle and enablement test families were written against the
LANDED APIs by citation (see §4 decision 3). The paper loop's ledger leg keeps
its reference-only posture: `PaperLedgerEntry.payment_state_ref` carries a
non-empty REFERENCE and never mirrors A's FORECASTED/EARNED/PAID vocabulary —
when the composition root binds the loop, it can validate the reference
against A's own closed vocabulary.

**To the P16.13/P16.14 owners (event vocabulary)**: the v2 event set
deliberately carries no payment/billing event — if P16.13/P16.14 need one, it
enters as a NEW v2 schema version via the P16-E-2 review, never a silent
widening.

**To the human build/test gate**: every Rust file in this batch is
WRITTEN-UNRUN; the first workspace build compiles the six new authz variants,
the v2 events module, the four simulator market modules, and the two test
files. Failures at that gate are recorded and fixed as code edits — never
silently.

## 6. Deviations

None from the subtask scope. Two recorded scoping notes (not deviations): (1)
the P16.21 payment-lifecycle state-machine and enablement opt-in/out tests are
recorded as a handoff pending Agent A's landed modules (record 63 §3) — the
grant-model opt-in/out semantics that COULD be tested against landed seams are
tested; (2) the brief's file-ownership listed records 61–79; this batch uses
61–66, leaving 67–78 free (the close record takes 79 per the brief).
