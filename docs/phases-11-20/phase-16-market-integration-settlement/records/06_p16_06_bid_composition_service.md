# P16.06 — Bid composition service: asset envelopes → aggregation-level bids per market structure; the envelope is the truth layer; operator-approved mode + explicit per-market grant REQUIRED

| Field | Value |
|---|---|
| Subtask | P16.06 (Bid composition service — asset envelopes → aggregation-level bids per market structure: PJM single aggregation node [S-90]; CAISO PDR two-ID load-shift [S-96]; ISO-NE DAM Asset tab [S-103]; the envelope remains the physical truth layer; operator-approved mode + explicit per-market grant required before any bid is composed [S-120]) |
| Wave / agent | Phase 16 wave 1, Agent A |
| Status | WRITTEN-UNRUN: the composer and its test file are written, never compiled, never executed, never run. **No bid has been composed for any real market, and no bid is submitted — none can be** (`NO_BID_SUBMITTED_STATEMENT`; ADR-011; conventions §7.3). |
| Sources | [S-90] (PJM FERC 2222 participation model — single aggregation node), [S-96] (CAISO demand response — PDR two-ID load-shift structure), [S-103] (ISO-NE Order 2222 readiness — DAM Asset tab), [S-120] (ESIG MMS aggregated-DER bids — the grant/mode discipline framing); `crates/flexcore-kernel/src/profile.rs` (`ControlAuthorityPosture` — the mode vocabulary's one home, consumed BY NAME) |
| Files touched | `crates/flexcore-application/src/bid_composition.rs` (NEW); `crates/flexcore-application/tests/bid_composition.rs` (NEW, written-unrun); `crates/flexcore-application/src/lib.rs` (additive module line) |

## 1. What landed

| Item | Content |
|---|---|
| `MarketStructureDeclaration` | The closed structure set (ADR-011 — a fourth enters as a NEW variant in a jurisdiction record pack): `PjmSingleAggregationNode` (ONE aggregation node [S-90]), `CaisoPdrTwoIdLoadShift` (the two recorded identifier ROLES [S-96]; the market-facing semantics stay with the record pack — secondary-source specifics UNVERIFIED, marked), `IsoNeDamAssetTab` (per-asset entries [S-103]) |
| `MarketBidGrant` | The explicit per-market grant RECORD: identity, tenant, the market-structure name it covers, closed state vocabulary (`ACTIVE`/`REVOKED`/`EXPIRED`), granter, granted-at, END-EXCLUSIVE validity, P11 audit-chain LINK. The market.* permission ACTIONS that would back grants in the authz registry are P16.19's — proposed there, never registered here |
| `compose_bid` | Validates, in order: the structure's identifiers, the injected clock, the control mode (the exact kernel posture name `operator-approved` — one home, consumed by name, never restated), the grant (ACTIVE, unexpired, tenant-matched, structure-matched), the interval bounds, the asset inputs (provenance, finite non-negative quantities, interval alignment), then composes |
| `ComposedBidRecord` | PJM: ONE aggregated bid at the node with per-interval sums, contributing assets (ascending, deduplicated) and envelope refs (ascending, deduplicated). CAISO: the aggregated load-shift bid under the two recorded IDs. ISO-NE: per-asset Asset-tab entries — the per-asset quantities stay PER ASSET, no cross-asset aggregation |
| Truth layer | Every `AssetBidInput` carries its envelope-version reference FORCIBLY (`MissingEnvelopeProvenance` otherwise); the composed record carries the references forward; no adjustment factor exists to apply silently (`ENVELOPE_IS_TRUTH_LAYER_STATEMENT`) |
| Tests | `tests/bid_composition.rs`: 9 tests — PJM aggregation sums; CAISO two-ID carry; ISO-NE per-asset preservation; missing envelope provenance refused; the three non-operator modes refused; revoked/expired/foreign-tenant/wrong-market grants refused; BTreeMap determinism + replay byte-identity; the no-submission + mode-name statement pins; negative/non-finite/misaligned quantities refused |

## 2. Recorded decisions

1. **The envelope remains the physical truth layer** (`ENVELOPE_IS_TRUTH_LAYER_STATEMENT`): quantities are derived FROM the envelope by the caller and carry the envelope-version reference; composition validates and aggregates — it never adjusts, clips, or re-derives.
2. **Operator-approved mode + an explicit per-market grant are REQUIRED** (`OPERATOR_GRANT_REQUIRED_STATEMENT`; context §3.2; conventions §6.4; [S-120]). The control-authority vocabulary's single home is the kernel's `ControlAuthorityPosture`; this module consumes the exact posture NAME — the enum is not restated (no new dependency edge was added; flexcore-application keeps its recorded dependency set).
3. **No bid is submitted — none can be** (`NO_BID_SUBMITTED_STATEMENT`): the output is an internal record; submission would require a jurisdiction adapter (ADR-011), a named market counterparty (D1, OPEN), and an operator-approved deployment that this program does not have.
4. **No new solver classes** (conventions §6.5): composition is deterministic summation of envelope-derived quantities per interval — LP/QP results consumed, no optimization performed, no MISOCP/MINLP creep.
5. **The CAISO two-ID structure is recorded pack data** [S-96]: the identifier roles are structural; the market-facing semantics are the record pack's, and the secondary-source specifics are marked UNVERIFIED in the pack discipline.

## 3. Honest boundaries

1. No bid has ever been composed for a real market, no market structure declaration exists outside this vocabulary, and the composer has never executed; the tests have never run (their first run is the human verification obligation).
2. The grant RECORD is not an authorization mechanism: the authz surface (market.* actions, the never-granted set — byte-identical) is P16.19's proposal, and composition's grant check is a precondition on a record, not a permission decision.
3. The permitted-tool outputs are quoted in the close-out record (07).
