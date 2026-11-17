# Phase 16 — Wave 2, Agent A close-out (P16.13–P16.18)

| Field | Value |
|---|---|
| Wave / agent | Phase 16 wave 2, Agent A |
| Subtasks | P16.13 (revenue-share billing basics), P16.14 (payment lifecycle transparency), P16.15 (program event management), P16.16 (capacity-market participation records, RECORD-ONLY), P16.17 (UK/EU LFM integration design records), P16.18 (market jurisdiction register, register-as-data) |
| Status | WRITTEN-UNRUN — every file below is written, never compiled, never executed, never tested. The loop bans were honored in full: nothing installed, nothing built, no test run, no placeholders of any kind (`todo!()` / `unimplemented!()` / TODO / FIXME — zero occurrences in the batch, checked by grep at close), only the three permitted verification tools run. |
| Honesty | **No market is connected, no tender is placed, no qualification is claimed, and no invoice, statement, figure, or payment is issued or exists anywhere in this batch.** The jurisdiction register ships with ZERO markets enabled. The P11 audit chain is consumed (linked per record via the domain `AuditChainLink`, one home), never redefined. The frozen 14-state/27-edge command set is byte-identical (read-only consumption in P16.15's tests only); the frozen 26-event vocabulary and the never-granted permission set were not touched. No ledger checkbox was ticked (the ledger is outside this agent's ownership). Every date in the batch is SOURCED DATA carrying the re-verify-at-use rule; the LR-075 conflict is resolved BY RECORD (both dates carried, neither selected). Every external claim cites an [S-n] from the gap analysis §11 or is marked UNVERIFIED/ASSUMPTION. |

## 1. Files created

| Layer | File |
|---|---|
| P16.13 billing | `crates/flexcore-application/src/revenue_share_billing.rs` (NEW, with inline written-unrun test suite — 12 tests) |
| P16.14 payment lifecycle | `crates/flexcore-application/src/payment_lifecycle.rs` (NEW, with inline written-unrun test suite — 10 tests) |
| P16.15 program events | `crates/flexcore-application/src/program_event_management.rs` (NEW, with inline written-unrun test suite — 10 tests) |
| P16.16 capacity records | `crates/flexcore-application/src/capacity_market_records.rs` (NEW, with inline written-unrun test suite — 10 tests) |
| P16.17 UK/EU LFM | `crates/flexcore-application/src/uk_lfm_records.rs` (NEW, with inline written-unrun test suite — 5 tests) |
| P16.18 register types | `crates/flexcore-application/src/jurisdiction_register.rs` (NEW, with inline written-unrun test suite — 8 tests) |
| P16.18 register data | `crates/flexcore-application/src/jurisdiction_register_data.rs` (NEW — the checked-in DATA file, with inline written-unrun test suite — 6 tests) |
| Decision records | `records/41_p16_13_revenue_share_billing.md`, `records/42_p16_14_payment_lifecycle.md`, `records/43_p16_15_program_event_management.md`, `records/44_p16_16_capacity_market_records.md`, `records/45_p16_17_uk_eu_lfm_records.md`, `records/46_p16_18_market_jurisdiction_register.md`, this close-out (47) — all NEW under `docs/phases-11-20/phase-16-market-integration-settlement/records/` |

## 2. Files modified (additive only)

| File | Change |
|---|---|
| `crates/flexcore-application/src/lib.rs` | Additive `pub mod revenue_share_billing;` + `payment_lifecycle;` + `program_event_management;` + `capacity_market_records;` + `uk_lfm_records;` + `jurisdiction_register;` + `jurisdiction_register_data;` lines with comments (appended AFTER the wave-1 module blocks; the file was re-read immediately before the edit per the shared-surface agreement) |

Not touched: `ledger_phase_16.md`, `plan.md`, `context.md`, `docs/foundations/*`, `docs/plan/*`, `.agent/*`, `tools/*`, `crates/flexcore-domain/src/command_state.rs` (read-only), `crates/flexcore-domain/src/market_program_model.rs` (consumed, not edited), `crates/flexcore-storage/*`, `crates/flexcore-events/*` (the frozen 26), `crates/flexcore-authz/*` (the never-granted set), `crates/adapters/*` (the P14.05 mapping consumed by citation), any migration, any file owned by Agent B (records 61–79 territory; the P16.09/P16.12 modules consumed by citation), and Agent B's wave-1 modules (`fixed_decimal.rs`, `settlement_reconciliation.rs`, `dispute_workflow.rs`, `performance_evidence.rs`, `market_price_records.rs`, `telemetry_export_design.rs`, `settlement_meter_data.rs`) — all consumed by citation only.

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

(The `ù` character in the dependency-rule output is the tool's own console encoding of its separator glyph, quoted verbatim as produced — same as the wave-1 close-out.)

## 4. Key recorded decisions (details in records 41–46)

1. **P16.13**: exact disaggregation — `percent_of_amount` computes `total × percent` in checked integer mantissa arithmetic with additive scales (no division, no rounding, no float; an overflow is a named refusal); shares must sum to EXACTLY 100% (never normalized); the per-asset shares are verified to sum back to the pooled total exactly (zero remainder); owner statements carry a closed {DRAFT, RECONCILED} vocabulary with NO ISSUED/SENT/DELIVERED state existing structurally; invoicing records are counterparty-reference data only; reconciliation mismatches carry BOTH values verbatim with the P16.09 queue reason codes.
2. **P16.14**: closed {FORECASTED, EARNED, PAID} vocabulary with exactly two frozen edges (no FORECASTED → PAID shortcut — a payment without a recorded earned basis is unrepresentable); every transition carries the P11 audit-chain LINK plus the EXACT previous-transition chain link (a fabricated head is refused); one currency per chain; a state is never entered without its basis record.
3. **P16.15**: the linkage record's entry state is a CONSTANT `CommandState::Proposed` (not a field — the P14.05 pattern mirrored); the constructible set is exactly {PROPOSED_ENTERED, REJECTED_BY_GATE}; opt-in is FAIL-CLOSED (no decision = no candidate; the latest decision by (timestamp, id) wins; opt-outs produce named records; a reality-mismatched opt-out record is refused); M&V delivery is a closed two-state record with no TRANSMITTED state.
4. **P16.16**: `SourcedDate` makes the re-verify-at-use rule STRUCTURAL (a date without the flag is unconstructable); the LR-075 conflict is resolved BY RECORD — both sourced dates carried with sources, no selection field exists, PJM's tariff named as the re-verification authority; no QUALIFIED state exists anywhere; every checklist item starts UNSATISFIED with EMPTY evidence and a mandatory source label (the Phase 9 scope rule inherited).
5. **P16.17**: flow designs carry DECLARED status only (no executed stage is representable); the P483 stacking determination vocabulary has exactly ONE variant (RULE_APPLIED — a recorded rule can never be widened structurally; the refusal lives in the P16.02 enrollment store, consumed by citation); payment records are exact-decimal data, and none exists.
6. **P16.18**: the register is DATA (types + the checked-in `jurisdiction_register_data.rs` with ten jurisdiction rows and ZERO enablement rows); ENABLED_BY_D1 is constructable only with the human owner's decision reference + the P11 audit-chain link (the D1-class decision stays with the human owner; P16-E-1 stays OPEN); the NYISO and PJM date conflicts are carried on BOTH sides with conflict notes; the ERCOT ADER posture records the non-FERC pilot status [S-100].

## 5. Handoffs

**To Agent B (wave 2, concurrent):** (a) the P16.19 permission work can register `market.*` actions against the byte-identical never-granted set — my modules add no dispatch vocabulary and request none; the P16-E-2 vocabulary decision stays yours/the owner's. (b) The P16.22/P16.23 action/event lists can consume my module statements as the record-level invariants they must not contradict (in particular: external events enter at PROPOSED only — P16.15's constant entry state; no payment issuance vocabulary exists — P16.13/P16.14). (c) Agent B's wave-1 `fixed_decimal` layer was consumed through its public API only (`parse`/`from_parts`/`checked_sub`/`partial_cmp`/canonical rendering) — no B file edited; the one interoperability note is that `MoneyAmount` equality is value-based across scales (P16.13's reconciliation depends on it).

**To the orchestrator / human gates:** (a) all seven test suites (61 test functions total across the six subtask modules and the data file: 12 + 10 + 10 + 10 + 5 + 8 + 6) are WRITTEN-UNRUN — execution is the human build/test gate, alongside the wave-1 suites. (b) The first human build may surface recorded defects in the WRITTEN-UNRUN base; my modules reuse the domain's `AuditChainLink`/`ParticipationWindow` and B's `FixedDecimal`/`MoneyAmount` public APIs exactly as documented. (c) P16.21 (Agent B) should consume the inline suites' negative cases as its fixture base. (d) An additive 0023-class migration (billing/payment/enablement record classes per MIGRATION_POLICY expand-only) is the recorded home for persistence when a later batch lands it — deliberately NOT created in this batch (no schema pressure exists yet, and the money/enablement columns' shapes should follow the built modules, not precede them).

## 6. Deviations

None from the subtask scope or the hard bans. Two recorded scoping notes (not deviations): (1) all six modules landed in `flexcore-application` rather than split with `flexcore-domain` — the money-bearing records require the `fixed_decimal` layer, which lives in the application crate, and the dependency rule forbids the domain depending on it; the P11 chain link is consumed through the domain's exported `AuditChainLink` type (application → domain is an allowed edge; the checker stays green). (2) The register data file is a checked-in Rust data module (`jurisdiction_register_data.rs`), not a JSON/TOML file — the workspace carries no serde dependency and adding one is banned; the module is versioned, diffable data with zero runtime I/O, which satisfies the register-as-data intent honestly.
