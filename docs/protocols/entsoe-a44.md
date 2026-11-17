# ENTSO-E Transparency Platform A44 Price-Ingestion Adapter Guide

| Field | Value |
|---|---|
| Scope | The FlexCore `protocol-entsoe` adapter (`crates/adapters/protocol-entsoe/`): READ-ONLY ingestion of ENTSO-E Transparency Platform A44 day-ahead price documents for later-phase analytics (Phase 16 consumes the series; nothing here bids) |
| Owner record | `docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/04_p15_04_price_ingestion_entsoe.md` (registers P15-E-2 OPEN with DEFAULT) |
| Implementation status | **WRITTEN-UNRUN.** A44 document model, XML extraction seam, token-reference boundary, query assembly, and the transport SEAM exist as written code; nothing has compiled, no request was ever sent, no document was ever fetched (`UnwiredA44Transport` refuses every request with `A44_TRANSPORT_SEAM_NOT_WIRED`) |
| Conformance status | **No conformance, certification, or accuracy claim of any kind.** A price series is ingested DATA — never a FlexCore forecast-accuracy claim and never a market action |
| Control status | Ingest-only, structurally: the adapter has no bid, offer, order, or dispatch vocabulary anywhere in its surface (the ADR-010 extension / ADR-011 boundary). Nothing here authorizes, issues, dispatches, or bids |

## 1. What the adapter is (and is not)

| Aspect | Recorded state |
|---|---|
| Role | Price-data ingester (READ-ONLY; `PriceAdapterRole` / `price_ingest_scope()` — the ingest-only authority statement is a recorded constant) |
| Document model | The A44 day-ahead price document: document identity, time series, periods, and points (`A44Document` / `A44TimeSeries` / `A44Period` / `A44Point`); the `A44` document-type code and every wire-level detail are UNVERIFIED until checked against the live API guide [S-111] |
| Resolutions | The declared `Resolution` vocabulary with per-resolution interval minutes; an unknown resolution code is refused, never guessed |
| Query shape | One A44 query per domain pairing and period (`A44PriceQuery`): the period bounds must parse as YYYYMMDDHHMM (UTC) and be ordered — inverted or malformed periods are refused by name (`A44QueryError`) |
| Extraction seam | A minimal internal XML node type plus a pure extraction (`xml_seam`); NO XML parser library (the CIM/CGMES precedent) — the composition root's parser fills the node tree |

## 2. Deployment (what a deployment would configure)

| Configuration | Honesty note |
|---|---|
| The registered API token | a HUMAN GATE input: the adapter ships with the gate UNSIGNED (see §3); registering it is a deployment-configuration act, never a code act |
| The transport implementation | UNWIRED — a composition root implements `A44Transport` over a real HTTP client ONLY after P15-E-2 closes and a human registers the token |
| Domain pairings / query windows | per-deployment declared queries; the recorded endpoint and parameter names (`documentType`, `in_Domain`, `out_Domain`, `periodStart`, `periodEnd`, `securityToken`) are UNVERIFIED per [S-111] and pinned by tests so a change is a deliberate, recorded one |
| Document size bound | `MAX_A44_DOCUMENT_BYTES` (8 MiB) — a declared refusal bound, not a measured limit |

## 3. Security (token gate, no claims)

| Topic | Design position |
|---|---|
| Gate state | STRUCTURALLY UNSIGNED: `TOKEN_GATE_STATEMENT` records the gate; no request is ever sent in this program (the first live call is a human/lab gate after P15-E-2 closes and a human registers the token) |
| Reference-only custody | the adapter carries scheme-prefixed token REFERENCES only (`env:`, `file:`, `secret-store:` — the P7.05 secret-reference schemes); token MATERIAL is structurally impossible: the closed field vocabulary has no field a key could hide in |
| Refusal hygiene | a reference without a scheme prefix is refused as `InlineMaterialRefused`, an empty reference as `EmptyReference`; a rejected reference's VALUE is never echoed back (it may be the token itself) |
| Out-of-band token | the token parameter is appended LAST at call time by the deployment from the registered reference; the token value never passes through this crate's types |

## 4. Operations (budgets, observability, failure modes)

| Topic | Design position |
|---|---|
| Backpressure | No module and no budget (the recorded no-module decision, record 83 §5 — FRV1-R6-002): poll-based READ-ONLY ingest over an UNWIRED transport seam — no recurring message rate exists to budget until the transport seam is wired; the bounded surface today is the query/period validation (`A44QueryError`) and the one-response-at-a-time call shape; a rate budget is added WITH the transport implementation, derived from the declared cadence |
| Failure modes | empty/inverted periods refused; malformed period bounds refused by name; unknown resolution codes refused; oversized documents refused; a mismatched extraction shape is a refusal, never a silent trim or a guessed reconstruction |
| Connection states | The common `ProtocolConnection` vocabulary is not exercised: the adapter's transport reports through its own `A44TransportError::SeamNotWired` refusal; the common `ProtocolConnection` port itself is the recorded FRV1-R6-001 deferral — no adapter implements it in this baseline, and the shipped `UnwiredProtocolConnection` default refuses fail-closed by name |

## 5. Honest boundary

No request was ever sent, no document was ever fetched, no price point
was ever extracted from a live platform. The endpoint shape, parameter
names, element names, period format, and resolution codes are cited
from [S-111] and carry UNVERIFIED labels naming the implementation-time
check against the live Transparency Platform API guide; a mismatch at
that gate is a recorded code or record change — never a silent
reinterpretation. A price series is an input to later-phase analytics,
never a command, a FlexCore accuracy claim, or a market action; the
adapter makes no conformance, qualification, or certification claim.
