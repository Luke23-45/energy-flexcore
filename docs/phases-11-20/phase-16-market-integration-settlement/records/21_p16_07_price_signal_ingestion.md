# P16.07 — Price-signal ingestion: DA/RT price records over the P15.04 seam, ISO publication seams

| Field | Value |
|---|---|
| Subtask | P16.07 (Day-ahead/real-time price records; ENTSO-E Transparency Platform adapter CONSUMING P15.04's output — never redefining it [S-111]; DataMiner2-class ISO price-publication seams [S-94]; prices are inputs, never commands (ADR-010)) |
| Wave / agent | Phase 16 wave 1, Agent B |
| Status | WRITTEN-UNRUN: the record layer, the decimal money type, and the unwired ISO publication seam are written, never compiled, never executed. No price feed is connected; no price record exists outside synthetic fixtures; no price is a command anywhere. |
| Sources | [S-111] (ENTSO-E Transparency Platform + API guide — the P15.04 adapter is the concrete source), [S-94] (PJM DataMiner2 reg_market_results — the ISO public-feed CLASS the seam models), [S-112] (the vendor price-forecast seam, P15-E-2, registered OPEN) |
| Files touched | `crates/flexcore-application/src/fixed_decimal.rs` (NEW); `crates/flexcore-application/src/market_price_records.rs` (NEW); `crates/flexcore-application/src/lib.rs` (additive module lines only) |

## 1. What landed

| Item | Content |
|---|---|
| `fixed_decimal::FixedDecimal` | The exact scaling-factor decimal (i128 mantissa × 10^−scale) every settlement-adjacent record shares. Parse-from-text is the only entry path; there is NO `From<f64>` anywhere — a float-derived money value is UNCONSTRUCTABLE by design (plan §4 non-goals: no money arithmetic in floats). Equality/ordering rescale to the common scale with checked multiplication; subtraction is alignment-first, overflow is a named refusal, downscales refuse rather than round |
| `fixed_decimal::MoneyAmount` | FixedDecimal + the DECLARED currency code as data (the contracts crate has no currency unit; plan §4). No default currency; currency-code shape validated (1–8 ASCII chars, carried verbatim — the code set is the source's, UNVERIFIED per source); cross-currency subtraction is a named refusal |
| `market_price_records::MarketPriceRecord` | The settlement-adjacent DA/RT price record: segment (closed DayAhead/RealTime vocabulary), interval (start-inclusive/end-exclusive), exact decimal price per MWh (negative prices carried as published), area, source class, document reference. Fail-closed construction |
| `market_price_records::classify_series_segment` | The P15.04 consumption seam: links an ingested `PriceSeries` to a segment WITHOUT carrying or converting its prices. A44 day-ahead → DayAhead ([S-111], the verified class); unverified document classes REFUSE (`PRICE_RECORD_SEGMENT_NOT_CLASSIFIABLE`) — the real-time document code set is NOT claimed (UNVERIFIED, re-verify at use); vendor FORECAST series link to NO segment (a forecast is not a cleared price) |
| `market_price_records::IsoPricePublication` + `records_from_publication` | The DataMiner2-class ISO price-publication SEAM ([S-94] names the class): declared, UNWIRED, carrying wire-faithful DECIMAL TEXT (never floats); all-or-nothing batch assembly |
| Tests (written-unrun) | Inline `#[cfg(test)]` suites in both modules: statement pins, decimal parsing/exactness, scale/refusal rules, two-copies discipline, all-or-nothing batch behavior, classification refusals |

## 2. Recorded decisions

1. **P15.04 is consumed, never redefined.** The price-ingestion port is `flexcore_ports::price_signal::PriceSignalSource` and the concrete adapter is `crates/adapters/protocol-entsoe` (P15.04, its own decision record). This module adds no second ingestion path, no second price type, no wire-schema knowledge.
2. **Two copies, never silently merged** (`TWO_COPIES_STATEMENT`, pinned by tests): the P15.04 analytics copy carries `f64` price points (Phase 15's recorded seam decision); the Phase 16 settlement-adjacent record carries an exact decimal populated only at the adapter boundary from wire-faithful text. No code converts between them — the honest response to the phase-16 no-float-money ban given Phase 15's f64 seam.
3. **Prices are inputs, never commands** (ADR-010; conventions §6.3): no bid, offer, order, dispatch, or device-write vocabulary exists anywhere in the module's surface (structural, not conventional).
4. **The ISO seam is a seam, not a connection** (`ISO_SEAM_UNWIRED_STATEMENT`): the trait is declared and unwired; no feed is contacted in this program; every wire detail is UNVERIFIED until a human-gated adapter lands; adapter selection requires the jurisdiction decision (D1, OPEN; ADR-011).
5. **The ENTSO-E real-time document class is NOT named** — only A44 day-ahead is verified for classification; everything else refuses rather than guesses (UNVERIFIED label, re-verify at use against the [S-111] API guide).

## 3. Honest boundaries

1. No price feed is connected; no publication has been received; no price record exists outside synthetic fixtures; every fixture value carries no market meaning.
2. No price was used to bid, dispatch, settle, or influence any decision — no such pathway exists in the module.
3. Every test is WRITTEN-UNRUN; execution is the human gate. The permitted-tool outputs are quoted in the batch close-out record (26).
