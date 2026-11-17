# Phase 16 — Wave 1, Agent A close-out (P16.01–P16.06)

| Field | Value |
|---|---|
| Wave / agent | Phase 16 wave 1, Agent A |
| Subtasks | P16.01 (market-records store), P16.02 (program/enrollment domain model), P16.03 (registration data management), P16.04 (baseline engine), P16.05 (M&V event reports), P16.06 (bid composition service) |
| Status | WRITTEN-UNRUN — every file below is written, never compiled, never migrated, never executed, never tested. The loop bans were honored in full: nothing installed, nothing built, no test run, no placeholders of any kind (`todo!()` / `unimplemented!()` / TODO / FIXME — zero occurrences in the batch), only the three permitted verification tools run. |
| Honesty | **No market is connected, no registration is filed, no qualification or conformance is claimed, and no settlement number, revenue figure, invoice, or payment exists anywhere in this batch.** The P11 audit chain is consumed (linked per record), never redefined. The frozen 26-event vocabulary and the never-granted set were not touched. No ledger checkbox was ticked (the ledger is outside this agent's ownership). |

## 1. Files created

| Layer | File |
|---|---|
| Migration | `crates/adapters/postgres/migrations/0022_market_records_p16_01.up.sql` / `.down.sql` (NEW) |
| Storage contract | `crates/flexcore-storage/src/market_records_store.rs` (NEW) |
| Postgres adapter | `crates/adapters/postgres/src/market_records_store.rs` (NEW) |
| Domain model | `crates/flexcore-domain/src/market_program_model.rs` (NEW, P16.02) |
| Domain records | `crates/flexcore-domain/src/market_registration_records.rs` (NEW, P16.03) |
| Baseline engine | `crates/flexcore-application/src/baseline_engine.rs` (NEW, P16.04) |
| Baseline tests | `crates/flexcore-application/tests/baseline_engine.rs` (NEW, written-unrun, 13 tests) |
| M&V reports | `crates/flexcore-application/src/mv_event_report.rs` (NEW, P16.05) |
| Bid composition | `crates/flexcore-application/src/bid_composition.rs` (NEW, P16.06) |
| Bid tests | `crates/flexcore-application/tests/bid_composition.rs` (NEW, written-unrun, 9 tests) |
| Decision records | `records/01_p16_01_market_records_store.md`, `02_p16_02_program_enrollment_domain_model.md`, `03_p16_03_registration_data_management.md`, `04_p16_04_baseline_engine.md`, `05_p16_05_mv_event_reports.md`, `06_p16_06_bid_composition_service.md`, this close-out (07) — all NEW under `docs/phases-11-20/phase-16-market-integration-settlement/records/` |

## 2. Files modified (additive only)

| File | Change |
|---|---|
| `crates/flexcore-storage/src/lib.rs` | Additive `pub mod market_records_store;` line + comment |
| `crates/flexcore-domain/src/lib.rs` | Additive `pub mod market_program_model;` + `pub mod market_registration_records;` lines + comments |
| `crates/adapters/postgres/src/lib.rs` | Additive `pub mod market_records_store;` line + comment |
| `crates/flexcore-application/src/lib.rs` | Additive `pub mod baseline_engine;` + `pub mod mv_event_report;` + `pub mod bid_composition;` lines + comments (appended AFTER Agent B's concurrent P16.07–P16.12 module lines; the file was re-read immediately before the edit per the shared-surface agreement) |

Not touched: `ledger_phase_16.md`, `plan.md`, `context.md`, `docs/foundations/*`, `docs/plan/*`, `.agent/*`, `tools/check_dependency_rule.py`, `crates/flexcore-events/*` (the frozen 26), `crates/flexcore-authz/*` (the never-granted set), any migration below 0022, any file owned by Agent B.

## 3. Permitted verification tools — verbatim outputs

Run at close (the only commands executed in this batch besides read-only file inspection):

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

(The `ù` character in the dependency-rule output is the tool's own console encoding of its separator glyph, quoted verbatim as produced.)

## 4. Key recorded decisions (details in records 01–06)

1. **P16.01**: append-only supersede model on all six record tables (INSERT + SELECT grants; a correction is a NEW row); `audit_record_id uuid NOT NULL` on every record row — the P11 chain consumed, never redefined; settlement amounts as DECIMAL TEXT + currency code as DATA (no float money anywhere); retention rows UNSET-by-default with CONFIGURED requiring period AND basis (P0.16 §4).
2. **P16.02**: closed lifecycle transition tables per record class; records immutable, transitions supersede; recorded stacking rules enforced by refusal (R-P16-7); the program's baseline-method declaration carried as data (LR-073).
3. **P16.03**: three jurisdiction-specific record types (PJM per-DER + aggregation + append-only EDC review journal with the sourced 45-day window [S-90], re-verify-at-use; ERCOT dual legs [S-100]; ISO-NE CAMS/eMarket [S-103]); attestations are signed RECORDS — never validated claims; test-grade signature labels refused at construction (LR-203 inherited).
4. **P16.04**: LR-073 as the first invariant — unsupported method ⇒ recorded gap row, never a silent approximation; DLA [S-96] named and NOT implemented; CAISO ≤20% SDA bound enforced as sourced data; the flexible-load model's Phase 3 restriction untouched; replay determinism via injected Clock + BTreeMap ordering.
5. **P16.05**: no chain link ⇒ no report; exclusions recorded, never silent; FINALIZED requires closed window + zero exclusions; same-day feedback as a structural report class [S-43]; the interval numbers persist as P16.01 delivery rows (the report is deterministically re-derivable); MV_REPORT row persistence is recorded wave-2 scope.
6. **P16.06**: envelope provenance forcibly carried (no provenance ⇒ refusal); mode consumed BY NAME from the kernel's `ControlAuthorityPosture` (no new dependency edge, the enum not restated); grant record checked for ACTIVE + unexpired + tenant + market structure; the authz surface stays P16.19's; deterministic aggregation (PJM/CAISO summed per interval, ISO-NE kept per-asset).

## 5. Handoffs

**To Agent B (wave 1, concurrent):** the P16.01 settlement interface is `flexcore_storage::market_records_store::MarketSettlementRow` + `insert_market_settlement` / `market_settlement_rows_for_program` over the `market_settlements` table (migration 0022). Consume by citation; the closed `source_format` vocabulary (`PJM_MANUAL_29` / `CAISO_SQMD` / `EBGL_ISP` / `PROGRAM_DECLARED`), the decimal-text amount + currency-code-as-data discipline, and the `supersedes_settlement_id` correction model are binding for the ingestion design. The `audit_record_id NOT NULL` linkage and the append-only grants apply to every row B writes.

**To wave 2 (Agents A/B):** (a) P16.13/P16.14 consume the P16.02 contract terms (declared key/value data) and the P16.01 settlement rows — splits enter as recorded data, never interpreted upstream; (b) P16.19 consumes the P16.06 `MarketBidGrant` record and registers the market.* actions against the byte-identical never-granted set; (c) a `market_mv_reports`-class additive migration (0023-class, per MIGRATION_POLICY expand-only) is the recorded home for the P16.05 report row if wave 2 lands one — the `MV_REPORT` retention class already names it; (d) P16.21 consumes the two written-unrun test files (`tests/baseline_engine.rs`, `tests/bid_composition.rs`) — execution is the human gate.

## 6. Deviations

None from the subtask scope. Two recorded scoping notes (not deviations): the M&V report row table was deliberately not created in migration 0022 (re-derivation posture, decision 3 of record 05, wave-2 note above), and the P16.06 grant is a record-level precondition — the authz registry is untouched by design (P16.19 owns it).
