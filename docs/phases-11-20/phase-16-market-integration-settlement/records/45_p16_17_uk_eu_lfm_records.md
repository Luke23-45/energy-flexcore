# P16.17 — UK/EU LFM integration design: Piclo flow designs, availability/utilization payment records, P483 dual-participation stacking records

| Field | Value |
|---|---|
| Subtask | P16.17 (UK/EU LFM integration design — Piclo registration/tender/utilization/settlement flow designs, availability + utilization payment records, P483 dual-participation stacking records [S-36][S-114][S-115]; stacking rules per jurisdiction; the enrollment model refuses a violating combination — R-P16-7) |
| Wave / agent | Phase 16 wave 2, Agent A |
| Status | WRITTEN-UNRUN: the flow-design records, the payment record types, and the P483 stacking records are written, never compiled, never executed. **No LFM is contacted, no registration is filed, no tender is placed, no availability is offered, no utilization is delivered, and no payment has been made or received** — no payment record exists anywhere outside synthetic fixtures (ADR-011; conventions §7.3). |
| Sources | [S-36] (Piclo — the tender platform class), [S-37] (ENA flexibility overview), [S-114] (NGED flexibility procurement report 2025-26 — the availability/utilization payment structure and tender round class), [S-115] (Elexon P483, live 27 November 2025, via industry coverage — re-verified at use); the P16.02 `StackingRule` / `StackingRuleViolation` refusal (consumed by citation); the P16.16 `SourcedDate` type (one home for the re-verify rule) |
| Files touched | `crates/flexcore-application/src/uk_lfm_records.rs` (NEW); `crates/flexcore-application/src/lib.rs` (additive module line) |

## 1. What landed

| Item | Content |
|---|---|
| `LfmFlowStage` + `LfmFlowDesignRecord` | The Piclo journey as four DESIGN stages — REGISTRATION → TENDER → UTILIZATION → SETTLEMENT — each a design record with declared inputs/outputs and a mandatory source label. The store refuses a stage whose predecessor carries no design record (the design discipline). The records carry DECLARED status only — **no state vocabulary in the module can represent an executed stage** (structural) |
| `AvailabilityPaymentRecord` | Paid for OFFERED availability over a window ([S-36][S-114]): exact fixed-decimal money with currency as data, recorded basis label, tender reference, end-exclusive window. Recorded data only |
| `UtilizationPaymentRecord` | Paid for DELIVERED utilization against a named utilization event: same discipline, plus the event reference. Recorded data only |
| `P483StackingRecord` | The dual-participation stacking determination: names the P16.02 `StackingRule` identity it applies (mandatory — a determination without the rule is refused), carries the P483 sourced date (live 27 November 2025 [S-115], re-verify flag mandatory), and a closed determination vocabulary of exactly ONE value (`RULE_APPLIED`) — no ALLOWED/OVERRIDE variant exists, so a recorded rule can never be widened structurally |
| `LfmRecordStore` | BTreeMap-backed, register-only, deterministic |

## 2. Recorded decisions

1. **Flow designs, never executions** (`FLOW_DESIGNS_NEVER_EXECUTED_STATEMENT`, pinned by tests): the hard ban "no tender placed" is honored structurally — the design record has no executed status and the store registers designs only.
2. **Payment records are data, not payments** (`NO_PAYMENT_RECORD_EXISTS_STATEMENT`, pinned by tests): amounts are exact `MoneyAmount` values (never floats — hard ban 9); no payment has been made or received; fixtures are synthetic.
3. **Stacking refuses, never widens** (`P483_STACKING_REFUSES_STATEMENT`, pinned by tests): the refusal mechanism is the P16.02 enrollment store's `StackingRuleViolation` — consumed by citation, never redefined (R-P16-7). The determination vocabulary has exactly one variant, so this module cannot record an "allowed" override of a recorded rule.
4. **P483 is sourced data with the re-verify rule**: the 27 November 2025 live date travels on a `SourcedDate` (the P16.16 type reused — one home for the rule); the industry-coverage provenance label is carried verbatim and re-verified at use.
5. **Jurisdiction specificity**: this is the GB/EU record pack — a different market is a NEW pack (ADR-011). No generic flexibility-market engine is implied.

## 3. Honest boundaries

1. No LFM platform is integrated; nothing is registered, tendered, offered, delivered, or settled; every fixture value is synthetic and carries no market meaning.
2. The flow-stage input/output maps are DESIGN placeholders in the honest sense — declared structure recorded from the named sources, with no wire formats, credentials, or endpoints (those are UNVERIFIED until a human-gated adapter lands).
3. Every test is WRITTEN-UNRUN; execution is the human gate. The permitted-tool outputs are quoted in the close-out record (47).
