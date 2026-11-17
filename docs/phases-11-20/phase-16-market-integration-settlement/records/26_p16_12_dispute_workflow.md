# P16.12 — Dispute workflow: queue-linked challenges, chain-linked evidence bundles, resolutions

| Field | Value |
|---|---|
| Subtask | P16.12 (Line-item challenge tracking, evidence-bundle assembly (chain-linked), resolution records; disputes reference the reconciliation queue from P16.09; every bundle inherits the P11 chain treatment) |
| Wave / agent | Phase 16 wave 1, Agent B |
| Status | WRITTEN-UNRUN: the workflow vocabulary, frozen transition table, bundle assembly, and resolution records are written, never compiled, never executed. No dispute exists — no statement has ever been received, so none could be disputed. |
| Sources | The P16.09 record (`23_p16_09_settlement_ingestion.md`) for the reconciliation queue; the P11 audit-chain treatment (phase-16 context constraint 6; the LR-203 production-crypto requirement inherited); P16.01 audit-chain linkage (Agent A's store design, consumed by citation) |
| Files touched | `crates/flexcore-application/src/dispute_workflow.rs` (NEW); `crates/flexcore-application/src/lib.rs` (additive module line only) |

## 1. What landed

| Item | Content |
|---|---|
| `DisputeState` + `transition_dispute` | The frozen transition table (the P0.11 pattern): Opened→EvidenceAssembled→Submitted→Resolved, with Withdrawn reachable from Opened/EvidenceAssembled/Submitted; Resolved and Withdrawn are terminal; NO shortcut edges (no Opened→Submitted, no EvidenceAssembled→Resolved — a resolution requires submission) |
| `LineItemChallenge` + `challenge_kind_from_reason_code` | The challenge record with a MANDATORY P16.09 queue-entry reference (`DISPUTES_REFERENCE_THE_QUEUE_STATEMENT` — disputes are never self-invented); the kind is DERIVED from the queue entry's reason codes (`LINE_AMOUNT_MISMATCH` / `LINE_MISSING_IN_STATEMENT` / `LINE_UNEXPECTED_IN_STATEMENT`); an unknown code is a named refusal, never a generic "other" |
| `EvidenceBundle` + `EvidenceBundleEntry` | Chain-linked assembly: every entry carries a P11 chain-digest REFERENCE validated as exactly 64 hex characters (the P11 chain's 32-byte digest rendering); a bundle with no evidence is refused (`BUNDLES_INHERIT_THE_P11_CHAIN_STATEMENT`); entries group deterministically by chain namespace (BTreeMap); bundles are immutable — new evidence is a NEW bundle |
| `DisputeResolutionRecord` | The recorded outcome (Upheld / AdjustedByCounterparty / Rejected) with a MANDATORY bundle reference (`DISPUTE_RESOLUTION_WITHOUT_BUNDLE` refusal: no dispute is resolved without its chain-linked evidence) and a mandatory resolver identity. An `AdjustedByCounterparty` outcome is recorded as data — the adjusted values re-enter through the P16.09 pipeline, never applied in place |
| Tests (written-unrun) | Inline suite: statement pins, the frozen table's positive and shortcut-refusal cases, terminal states, queue-code derivation and unknown-code refusal, digest-ref validation (length and hex charset), empty-bundle refusal, bundle-less resolution refusal |

## 2. Recorded decisions

1. **Disputes reference the reconciliation queue — never self-invented** (`DISPUTES_REFERENCE_THE_QUEUE_STATEMENT`, pinned by tests): the queue-entry linkage is mandatory and exact; the challenge kind is derived from the P16.09 reason-code vocabulary, never generalized.
2. **Every bundle inherits the P11 chain treatment by citation** (`BUNDLES_INHERIT_THE_P11_CHAIN_STATEMENT`, pinned by tests): each entry carries a chain-digest reference or the entry is refused; the append/verify mechanics stay in flexcore-audit (P11) at the composition root — the application crate cannot depend on it (the dependency rule), so this module stores REFERENCES and never redefines the chain. The LR-203 production-crypto requirement is inherited unchanged.
3. **The state machine is frozen** (the P0.11 pattern): exactly the recorded transitions; terminal states stay terminal; every refusal names the edge it refused.
4. **A resolution without its evidence is unconstructable**: the bundle reference is mandatory on every resolution record.
5. **No settlement adjustment is ever applied by this module**: an accepted adjustment re-enters through the P16.09 ingestion/reconciliation pipeline as data — the workflow's output is records only.

## 3. Honest boundaries

1. No dispute exists: no settlement statement has ever been received (none exists here), so no challenge, bundle, or resolution is real; every fixture value is synthetic and labeled.
2. No dispute has ever been submitted to any counterparty process; no adjustment, credit, or settlement outcome is claimed.
3. Every test is WRITTEN-UNRUN; execution is the human gate.

## 4. Agent B batch close-out (P16.07–P16.12): A-reconciliation, deviations, handoffs

**A-reconciliation (P16.01 wait-check).** The wave-1 split defines Agent A's P16.01 market-records store (migration 0022) as the one interface Agent B consumes. Wait-check performed at batch start (not present) and again at batch close: Agent A's artifacts LANDED during the batch — `crates/flexcore-storage/src/market_records_store.rs` (the `MarketRecordsStore` trait incl. `insert_market_settlement` / `market_settlement_rows_for_program`, `MarketSettlementRow`, the `MarketRecordReceipt`, and the closed `MARKET_SETTLEMENT_SOURCE_FORMATS` vocabulary `PJM_MANUAL_29` / `CAISO_SQMD` / `EBGL_ISP` / `PROGRAM_DECLARED`) and `crates/adapters/postgres/migrations/0022_market_records_p16_01.{up,down}.sql` (tables `market_programs`, `market_contracts`, `market_enrollments`, `market_events`, `market_deliveries`, `market_settlements`, `market_record_retention_config`). This batch consumes P16.01 BY CITATION ONLY: no Agent B module declares a store trait of its own (so nothing can conflict), no A file was edited, and no store type is redefined or duplicated. The alignment is structural: A's `amount_text` is validated decimal text with `currency_code` as data — the same wire-faithful-text, no-float-money discipline as the Agent B `fixed_decimal` record layer; A's settlement source-format vocabulary maps one-to-one onto the P16.09 `SettlementFormat` labels (SCREAMING_SNAKE persistence spelling vs kebab-case record spelling, recorded in `23_p16_09_settlement_ingestion.md` decision 5); and the A trait doc explicitly designates the P16.09 ingestion write as Agent B's consumer.

**Deviations from the brief.** None against the hard bans. Notes: (1) the brief's file-ownership listed records 21–39; this batch uses 21–26 (one record per subtask + this close-out section), leaving 27–39 free for wave 2. (2) The P15.04 price seam carries f64 price points (Phase 15's recorded decision); the phase-16 no-float-money ban is honored by the two-copies discipline (`21_p16_07_price_signal_ingestion.md` decision 2) — the decimal record is populated only from wire-faithful text, and no float→decimal conversion path exists (A's `MarketSettlementRow.amount_text` upholds the same discipline on the store side).

**Handoffs for wave 2.** (a) To the composition root / P16.01 store binding: the P16.09 extraction functions produce `PjmBliLineItem` / `CaisoSqmdIntervalRecord` / `EbglIspImbalanceRecord` shapes whose identity, decimal-text amount, currency, and format fields map onto A's `MarketSettlementRow`; the reconciliation engine consumes persisted rows as its `stated` side and caller-computed `ExpectedLine` records as its `expected` side. (b) To P16.19 (permissions, wave 2 Agent B): the price/settlement/evidence/dispute modules contain no dispatch vocabulary and need none; market.* actions should register at the P16-E-2 vocabulary decision. (c) To P16.20 (simulator): `records_from_publication` and the synthetic fixture grammars are the shapes the paper-trading simulator should emit; the unwired `IsoPricePublication`/`TelemetryExportSink` seams are its integration points. (d) To P16.21 (test suite): the fixture grammars and reconciliation cases here are the reusable synthetic-fixture base; the dispute state machine and chain-digest validation rules are its negative-test material.

**Permitted-tool outputs (verbatim, run at batch close):** see §5 below.

## 5. Permitted-tool outputs (verbatim)

Run at batch close, after all Agent B files and records were in place:

```text
$ python tools/verify_docs.py
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC
verify_docs exit=0

$ python tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid
validate_agent exit=0

$ python -I tools/check_dependency_rule.py
OK ù dependency rule holds for 28 workspace members.
check_dependency_rule exit=0
```

(The `ù` in the dependency-rule line is the tool's own output as printed
on this Windows Git Bash console, quoted verbatim. No cargo command, no
test execution, and no installation was performed — the loop bans hold;
all execution is the human gate.)
