# P15.04 — Price-signal ingestion: the ENTSO-E Transparency Platform adapter models (A44), the price port, the token HUMAN GATE (P15-E-2 registered OPEN with DEFAULT)

| Field | Value |
|---|---|
| Subtask | P15.04 (ENTSO-E Transparency Platform REST adapter DESIGN: A44 day-ahead price documents [S-111]; API token = HUMAN GATE input (UNSIGNED); vendor forecast seams as P15-E-2 [S-112]; prices are INGESTED DATA, never bid inputs; adapter implemented under `crates/adapters/` following the phase-14 adapter-crate pattern — real model types justified the crate) |
| Wave / agent | Phase 15 wave 1, Agent A |
| Status | WRITTEN-UNRUN: adapter models, extraction seam, token gate, transport seam, and scope are written, never compiled or executed; NO request is ever sent in this program (the transport is an UNWIRED seam); the token is UNSIGNED; no price data exists in this repository (conventions §1, §5) |
| Sources | [S-111] (ENTSO-E Transparency Platform + API guide, gap analysis §11 Track E — cited, NOT consulted page-by-page: every wire-level detail below carries an UNVERIFIED label naming the implementation-time check); [S-112] (Amperon price forecasting — the vendor-forecast seam analogy, P15-E-2) |
| Files touched | `crates/adapters/protocol-entsoe/` (NEW crate: `Cargo.toml`, `src/lib.rs`, `src/document.rs`, `src/xml_seam.rs`, `src/token.rs`, `src/transport.rs`, `src/scope.rs`, `tests/a44_extraction.rs`); `crates/flexcore-ports/src/price_signal.rs` (NEW); `crates/flexcore-ports/src/lib.rs` (additive); root `Cargo.toml` (additive member + workspace dependency entries); `tools/check_dependency_rule.py` (additive: `COMPOSITION_CRATES` + `MEMBER_DIRS`); `crates/flexcore-application/src/lib.rs` (additive) |

## 1. Why a new crate (the conventions §6.2 test, applied)

The A44 document model types, the deterministic extraction logic, the token gate, and the query assembly are REAL code with clear ownership (a market-data adapter, not a DER protocol) and a stable dependency direction (composition side; nothing depends on it except future composition roots — the P14.07/P14.12 precedent). Zero external dependencies: the XML navigation is a documented seam over a minimal internal node type (no XML parser library — the P14.12 CIM/CGMES precedent), and the REST transport is an UNWIRED seam (no HTTP client, no TLS stack). The checker was updated ADDITIVELY (`flexcore-entsoe-adapter` added to `COMPOSITION_CRATES` and `MEMBER_DIRS`; the member added to the root manifest with a placement note).

## 2. What landed

| Module | Content |
|---|---|
| `document.rs` | The A44 day-ahead price document model ([S-111], UNVERIFIED register of 6 wire-level facts): closed resolution vocabulary (PT60M/PT30M/PT15M — UNVERIFIED spellings), YYYYMMDDHHMM period bounds (UNVERIFIED format) parsed by the civil-from-days integer algorithm (hand-checked against the P3.31 fixture horizon constant 1_780_272_000_000 and a known leap day), the point-count-must-match-duration rule, 1-based contiguous positions, finite amounts (negative prices CARRIED, sign unconstrained — real market outcomes, UNVERIFIED per zone), the A44-only document-type gate (other types refused, never loosely parsed) |
| `xml_seam.rs` | The minimal internal `XmlNode` type (the composition root's parser fills it — no XML library) + the pure `extract_a44_price_points` extraction: recorded UNVERIFIED element names pinned by tests, document-order walk, whole-document refusals on any structural violation (never trimmed), resolved interval bounds per point, ascending order by construction — the input shape for `flexcore-ports::price_signal::PriceSeries` |
| `token.rs` | The API-token HUMAN GATE: `UNSIGNED` ships as the gate state; scheme-prefixed REFERENCES only (`env:`/`file:`/`secret-store:` — the P7.05 schemes); token MATERIAL structurally impossible (no field a key could hide in); a rejected reference's VALUE never echoed |
| `transport.rs` | The REST seam: the typed `A44PriceQuery` (recorded UNVERIFIED parameter names pinned by tests; NO token parameter rides the typed parameters — resolved out-of-band from the registered reference at call time), the sync `A44Transport` port trait (READ-ONLY; no write method — structural), and `UnwiredA44Transport` answering EVERY request with `A44_TRANSPORT_SEAM_NOT_WIRED` (the P8.32 fail-closed posture) |
| `scope.rs` | The ingest-only authority declaration: role `ingest-only`, control scope `none` (a DECLARED value, not an omission), resource class `market-data`; prices are INGESTED DATA for later phases, never bid inputs and never dispatch inputs (ADR-010 extension; ADR-011 unchanged) |
| `tests/a44_extraction.rs` | WRITTEN-UNRUN integration test: hand-built 24-hour document tree → extraction (the negative midday price carried), transport refusal, token gate, scope, resolution/period arithmetic — hand-computed expected values only |
| `crates/flexcore-ports/src/price_signal.rs` | The price PORT (source-neutral by construction — no platform/vendor name passes the ports grep gate): `PriceSeriesKind` (Actual vs VendorForecast — NEVER conflated), `PriceSeries` with DECLARED currency (no default exists anywhere) + source/document/area provenance + ascending-order validation (unsorted input refused, never silently sorted), the `PriceSignalSource` port (READ-ONLY ingestion; NO bid/offer/order/dispatch vocabulary anywhere in the trait surface — structural) |

## 3. P15-E-2 — Price-forecast vendor seam (registered OPEN — agents never close E-decisions, conventions §4)

- **Question:** are day-ahead/intra-day PRICE forecasts ingested from a vendor in addition to the ENTSO-E Transparency historical/actual prices?
- **Options:** (a) Amperon-class vendor forecasts ingested as data [S-112]; (b) ENTSO-E actuals only, no vendor forecasts; (c) in-platform price forecasting (explicitly premature — no data exists).
- **DEFAULT (RECOMMENDATION):** (a) alongside the [S-111] adapter; Phase 16 consumes whatever lands; nothing here bids.
- **What closes it:** the human owner's vendor selection and market-jurisdiction register entry (program README hard status boundary 4; conventions §4).
- **Recorded in:** this record; the port already carries the `VendorForecast` kind so the seam exists without any vendor integration.

## 4. UNVERIFIED register (implementation-time checks against [S-111] and a registered token)

| # | Detail | Where |
|---|---|---|
| 1 | The A44 document-type code names day-ahead prices | `document.rs::A44_DOCUMENT_TYPE` |
| 2 | The XML element names (`TimeSeries`, `Period`, `Point`, `position`, `price.amount`, `start`/`end`/`resolution`, domain mRID and currency elements) | `xml_seam.rs` constants |
| 3 | The period-bound query/publication format (YYYYMMDDHHMM, UTC) | `document.rs::parse_period_bound` |
| 4 | The resolution codes and interval arithmetic | `document.rs::Resolution` |
| 5 | The endpoint URL shape and the parameter names (`securityToken`, `documentType`, `in_Domain`, `out_Domain`, `periodStart`, `periodEnd`) | `transport.rs` constants |
| 6 | The response format (XML) and encoding | `xml_seam.rs` |
| 7 | The currency convention (a published currency element vs a per-zone default) | `document.rs::A44TimeSeries.currency_code` (DECLARED per series; no default) |

A mismatch at the implementation-time gate is a recorded code or record change — never a silent reinterpretation.

## 5. Honest boundaries

1. Prices are INGESTED DATA: this phase produces ingestion seams only; Phase 16 consumes them; nothing here bids, settles, or shapes any market action (the port trait and the adapter surface cannot express one — structural).
2. NO request is sent in this program; the first live call is a human/lab gate after P15-E-2 closes and a human registers the token (UNSIGNED).
3. No market qualification, conformance, or certification claim is made; the platform lists no prices it did not ingest.
4. Every test is WRITTEN-UNRUN; every expected value is hand-computed (the epoch/leap-day arithmetic is checked against known values, not observed outputs).
5. The checker and root manifest edits are ADDITIVE only; `python -I tools/check_dependency_rule.py` reports the 28th member and holds (output quoted in the close-out record 07).
