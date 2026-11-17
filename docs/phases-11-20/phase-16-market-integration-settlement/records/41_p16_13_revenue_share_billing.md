# P16.13 — Revenue-share billing basics: contract-configurable splits, pooled-revenue disaggregation per asset, owner statements, invoicing records, reconciliation to ISO/DSO statements

| Field | Value |
|---|---|
| Subtask | P16.13 (Revenue-share billing basics — contract-configurable splits, pooled-revenue disaggregation per asset, owner statements, invoicing records, reconciliation back to ISO/DSO statements [S-118]; NOT a retail CIS — plan §4. Models + workflow; no invoice/statement/figure issued) |
| Wave / agent | Phase 16 wave 2, Agent A |
| Status | WRITTEN-UNRUN: the models, the exact-disaggregation workflow, and the reconciliation engine are written, never compiled, never executed. **No invoice, statement, or figure is issued anywhere; no billing run has executed; no money value exists outside synthetic fixtures; the stores are empty.** No market is connected (ADR-011; conventions §7.3). |
| Sources | [S-118] (Navitasoft VPP settlement blueprint — the pooled-revenue disaggregation pattern), [S-47] (Voltus payment transparency — the owner-facing posture); P16.02 contract terms (declared key/value data, consumed by citation); P16.01 settlement rows (`MarketSettlementRow`, consumed by citation); the P16.09 queue reason codes and the P16.12 dispute-queue pattern (consumed by citation) |
| Files touched | `crates/flexcore-application/src/revenue_share_billing.rs` (NEW); `crates/flexcore-application/src/lib.rs` (additive module line) |

## 1. What landed

| Item | Content |
|---|---|
| `RevenueShareConfiguration` | The contract-configurable split record: declared decimal-text percentages per asset, recorded exactly as the contract states them. The ONE structural check is the closed-world usability check: the shares sum to EXACTLY 100% at the common scale — never normalized, padded, clamped, or rounded. A share outside [0, 100] or an unparseable share text is a named refusal |
| `percent_of_amount` | The exact share computation `total × percent` in CHECKED INTEGER arithmetic on the decimal mantissas: mantissas multiply (overflow-checked), scales ADD (+2 for the percent denominator) — no division, no rounding, no float anywhere (hard ban 9; the `fixed_decimal` layer's rules). A scale or mantissa overflow is a named refusal |
| `PooledRevenueDisaggregation` | The per-asset disaggregation record over ONE ISO/DSO settlement statement row (the P16.01 row shape, consumed by citation): per-asset exact share amounts, verified to sum back to the pooled total EXACTLY (zero remainder via chained overflow-checked subtraction — the fixed-decimal layer has no accumulation add, and the zero-remainder proof is exact) |
| `OwnerStatementRecord` | The owner statement as a record with the closed vocabulary {DRAFT, RECONCILED} — **no ISSUED, SENT, or DELIVERED state exists structurally** (the enum has two variants). Mandatory disaggregation linkage (a statement is never self-invented); uniform currency enforced |
| `StatementReconciliationOutcome` | Reconciliation back to the ISO/DSO statement lines: exact string line identity, exact value equality, cross-currency run refusal; every mismatch carries BOTH values verbatim plus the exact deviation; discrepancy reason codes are the P16.09 queue vocabulary (`LINE_AMOUNT_MISMATCH` / `LINE_MISSING_IN_STATEMENT` / `LINE_UNEXPECTED_IN_STATEMENT`) — the P16.12 dispute queue is fed by citation |
| `InvoicingRecord` | The invoicing record is RECORDED data only: it references a COUNTERPARTY-issued invoice number tied to a statement. No CREATED/SENT/PAID/VOID state exists — issuance and transmission are the counterparty's process, never this module's |
| `BillingRecordStore` | BTreeMap-backed, register-only, supersede model (a correction is a NEW record), deterministic reads, no wall-clock input |

## 2. Recorded decisions

1. **NOT a retail CIS** (`REVENUE_SHARE_NOT_RETAIL_CIS_STATEMENT`, pinned by tests): no tariff engine, no retail rate calculation, no customer account, no utility-bill replacement — plan §4 and gap analysis §8's recorded non-goal; ADR-011's jurisdiction-specificity boundary restated.
2. **Disaggregation is EXACT, never rounded** (`EXACT_DISAGGREGATION_STATEMENT`, pinned by tests): shares sum to exactly 100% at the declared scales, so the exact per-asset products sum back to the total exactly — there is no rounding step to take. An overflow is a refusal, never a wrapped or rounded value. This is the honest resolution of "percent-of-money requires division" without floats: the percent denominator is absorbed into the scale arithmetic.
3. **No issuance vocabulary exists** (`NO_STATEMENT_ISSUED_STATEMENT`, pinned by tests): the statement enum has exactly two variants; the invoice record exactly one. The honesty rule "no invoice/statement/figure issued" is structural, not conventional.
4. **Every mismatch is a named record with BOTH values** (`EVERY_MISMATCH_NAMED_STATEMENT`, pinned by tests); the reason-code spelling is the P16.09 record's (one vocabulary across the phase); the engine never settles or corrects in place.
5. **The P11 audit chain is consumed, never redefined**: every record carries the domain's `AuditChainLink` (one home — the domain type reused); persistence design (an additive 0023-class migration) is recorded as the wave handoff below, not implemented here.
6. **Splits are contract data, interpreted downstream only**: the configuration consumes the P16.02 contract's declared terms by citation; the module is the FIRST interpreter of the split data and performs only the closed-world sum check — it never widens, defaults, or invents a share.

## 3. Honest boundaries

1. No billing run has executed; no statement, invoice, or revenue figure exists; every fixture value in the tests is synthetic and carries no market meaning.
2. The P16.01 settlement-row interface is consumed by citation (the same discipline as the wave-1 records): the composition root binds the disaggregation inputs to `MarketSettlementRow` at the store boundary; no store trait is redefined here.
3. Persistence (an additive 0023-class migration naming the billing/payment/enablement record classes) is NOT in this batch — the migration is the recorded handoff below.
4. Every test is WRITTEN-UNRUN; execution is the human gate. The permitted-tool outputs are quoted in the close-out record (47).
