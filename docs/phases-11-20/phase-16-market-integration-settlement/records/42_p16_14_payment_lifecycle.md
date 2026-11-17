# P16.14 — Payment lifecycle transparency: Forecast → earned → paid states per asset/program, closed vocabulary, chain-linked transitions

| Field | Value |
|---|---|
| Subtask | P16.14 (Payment lifecycle transparency — Forecast → earned → paid states per asset/program, the Voltus pattern [S-47]; statuses as a CLOSED vocabulary; every state transition chain-linked, the P11 chain treatment) |
| Wave / agent | Phase 16 wave 2, Agent A |
| Status | WRITTEN-UNRUN: the state machine, transition records, and chain-enforcing store are written, never compiled, never executed. **No payment lifecycle has ever opened; no transition has ever run; no payment has been made, promised, or received; no payment record exists anywhere** (the stores are empty; test fixtures are synthetic). No market is connected (ADR-011; conventions §7.3). |
| Sources | [S-47] (Voltus — the "forecast to earned to paid" payment-transparency pattern), [S-118]; the P0.11 frozen-transition-table pattern (consumed by citation — the command state machine is untouched); the P11 audit-chain treatment (context constraint 6); the P16.13 invoicing/statement records (basis references, consumed by citation) |
| Files touched | `crates/flexcore-application/src/payment_lifecycle.rs` (NEW); `crates/flexcore-application/src/lib.rs` (additive module line) |

## 1. What landed

| Item | Content |
|---|---|
| `PaymentState` | The CLOSED vocabulary: exactly FORECASTED, EARNED, PAID — three variants; there is no fourth state to reach. PAID is terminal |
| Frozen transition table | Exactly two edges: FORECASTED → EARNED, EARNED → PAID (the P0.11 pattern). No FORECASTED → PAID shortcut — a payment without a recorded earned basis is unrepresentable; no backwards edges; no exits from PAID |
| `PaymentLifecycle` | One lifecycle per (tenant, asset, program), opened at FORECASTED with the forecast amount as recorded data (exact decimal + currency as data; the chain's currency is established at opening) |
| `PaymentTransitionRecord` | Every transition carries: the frozen edge, a MANDATORY basis reference for the destination state (the settlement/owner-statement line for EARNED; the payment record for PAID), the optional recorded amount (exact decimal, never a float), the P11 audit-chain LINK for its own write, and the identity of the PREVIOUS transition in the chain |
| `PaymentLifecycleStore` | BTreeMap-backed; apply-time enforcement of: the frozen edge, the basis requirement, the EXACT chain linkage (the given `previous_transition_id` must equal the chain's actual head — a broken or fabricated chain is refused by name), and the one-currency-per-chain rule (a transition in a different currency than the chain's is refused — money of two currencies is never silently mixed) |

## 2. Recorded decisions

1. **The vocabulary is closed and small on purpose** (`CLOSED_PAYMENT_VOCABULARY_STATEMENT`, pinned by tests): an owner sees a forecast, then an earned figure, then a payment — no PENDING, PROCESSING, or review state exists between them. The closure is structural (three variants), not conventional.
2. **Every transition is chain-linked** (`EVERY_TRANSITION_CHAIN_LINKED_STATEMENT`, pinned by tests): the chain link is exact (head equality), not merely present — a fabricated head is refused. The P11 chain treatment is consumed, never redefined: the audit-link references stay references; the chain mechanics stay in flexcore-audit (P11) at the composition root (the dependency rule).
3. **A state is never entered without its basis**: both frozen edges require a non-empty basis reference; the refusals are named (`PAYMENT_EARNED_BASIS_REQUIRED` / `PAYMENT_PAID_BASIS_REQUIRED`).
4. **Money is the fixed-decimal layer; one currency per chain**: amounts are `MoneyAmount` (exact i128 mantissa × 10^−scale + currency as data); a currency change mid-chain is a named refusal (`PAYMENT_CURRENCY_MISMATCH_IN_CHAIN`).
5. **The command lifecycle is untouched**: the frozen 14-state/27-edge set is not referenced, extended, or restated here — payment states are a separate closed vocabulary over financial records, never command states.

## 3. Honest boundaries

1. No lifecycle is open; no transition has run; no payment exists anywhere; every fixture value in the tests is synthetic and carries no market meaning.
2. The basis references point at P16.13's record identities by citation; the composition root binds them.
3. Every test is WRITTEN-UNRUN; execution is the human gate. The permitted-tool outputs are quoted in the close-out record (47).
