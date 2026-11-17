# P16.22 — Additive event-vocabulary v2 + market.* authz actions registration (the P16-E-2 proposal, REGISTERED OPEN)

| Field | Value |
|---|---|
| Subtask | P16.22 (Additive event-vocabulary v2 + authz actions registration — market.* actions registered per the authz pattern; new market events versioned SEPARATELY from the frozen 26 — recorded as P16-E-2) |
| Wave / agent | Phase 16 wave 2, Agent B |
| Status | WRITTEN-UNRUN: the v2 vocabulary module and the market.* action registration are written, never compiled, never executed. **The frozen 26-event vocabulary is byte-identical (pinned by test in BOTH modules); no v2 event has a producer wired; no market is connected anywhere.** |
| Sources | `crates/flexcore-events/src/events.rs` (the frozen 26 — consumed, never edited); P16.19 (the market.* action set — record 61); plan §2 row P16.22 and §6 (P16-E-2); conventions §4; `context.md` §3.3 |
| Files touched | `crates/flexcore-events/src/market_events_v2.rs` (NEW — never an edit of `events.rs`); `crates/flexcore-events/src/lib.rs` (additive module line + comment); `crates/flexcore-authz/src/action.rs` + `phase16_market_policies.rs` (the market.* registration — see record 61) |

## 1. What landed

| Item | Content |
|---|---|
| `market_events_v2` module (NEW FILE) | The additive, separately versioned market event vocabulary: its own enum (`MarketEventV2`), its own wire tag (`"market_event_type"` — the frozen `"event_type"` is untouched), its own subject scheme (`flexcore.v2.market.<slug>` — the frozen `flexcore.v1` scheme is untouched), and its own schema-version constant (`MARKET_EVENTS_V2_SCHEMA_VERSION = 1`). |
| The proposed v2 event set (8 events) | `MarketBidComposed` (a bid RECORD was composed — never submitted), `MarketGrantIssued` / `MarketGrantRevoked` (the P16.06 grant model's lifecycle facts), `MarketPriceRecordIngested` (wire-faithful decimal TEXT price provenance — never a computed figure), `MarketStatementIngested` (statement + format label + line COUNT — NO amount travels on the event), `MarketReconciliationCompleted` (run outcome counts), `MarketDisputeOpened` / `MarketDisputeResolved` (the P16.12 workflow facts; the resolution carries its evidence-bundle reference). |
| Versioning discipline | Events are facts, never imperatives (the P0.14 rule carried to v2 — every name is a past-tense fact, pinned by test); a breaking payload change is a NEW v2 schema version, never a silent repurpose; the frozen 26 stay byte-identical (`EVENT_COUNT == 26` and the exact names pinned by test inside the v2 module itself). |
| P16-E-2 status | `MARKET_EVENTS_V2_PROPOSAL_STATEMENT` + `MARKET_EVENTS_V2_DISJOINT_STATEMENT` pinned by tests: the vocabulary is the P16-E-2 PROPOSAL, REGISTERED OPEN, closer = the human owner; nothing here closes it and no producer is wired. |
| market.* action registration | The six `market.price.read` / `market.bid.compose` / `market.grant.issue` / `market.grant.revoke` / `market.grant.read` / `market.settlement.read` actions registered per the authz pattern (variant + id arm + parse arm + explicit grants; nothing matches by default) — full detail in record 61; this record compiles the two lists as the P16-E-2 proposal's complete surface. |

## 2. The compiled lists (the P16-E-2 proposal surface, as registered)

**Proposed v2 market events (8):** `MarketBidComposed`, `MarketGrantIssued`, `MarketGrantRevoked`, `MarketPriceRecordIngested`, `MarketStatementIngested`, `MarketReconciliationCompleted`, `MarketDisputeOpened`, `MarketDisputeResolved` — each DISJOINT from the frozen 26 (pinned), each a recorded fact, none carrying imperative semantics, none touching the dispatch lifecycle, none carrying a settlement amount.

**Registered market.* authz actions (6):** `market.price.read`, `market.bid.compose`, `market.grant.issue`, `market.grant.revoke`, `market.grant.read`, `market.settlement.read` — each additive on the closed `Action` enum with explicit tenant-scope grants only (record 61 §1), each DISJOINT from the never-granted set, and NO `market.dispatch.*` action exists in any spelling (pinned unparseable; the P14.16 command-relay-only rule consumed by citation).

## 3. Recorded decisions

1. **The frozen 26 are byte-identical** (hard ban 8): `events.rs` was NOT edited — the v2 vocabulary is a NEW file (`market_events_v2.rs`) with a NEW module line in `lib.rs`; the freeze is pinned by test in the v2 module itself (`EVENT_COUNT == 26`, first/last frozen names, absence of every v2 name from the frozen list).
2. **The v2 tag and subject scheme are separate** (the versioning discipline): a v2 event serializes with `"market_event_type"` and routes on `flexcore.v2.market.<slug>`, so no consumer of the frozen vocabulary can ever receive or misroute a v2 event, and the v2 vocabulary can version independently of the freeze.
3. **No money figures on events** (plan §8 criterion 4): the only numeric payload fields are counts (line items, matched/mismatched, intervals, assets) and wire-faithful price TEXT (provenance, verbatim from the P16.07 record layer). No amount, total, or settlement figure travels on any v2 event.
4. **No producer is wired**: no code in this program emits any v2 event; the module is the proposal's typed shape so the human review of P16-E-2 happens against concrete names and payloads, not intentions.
5. **Agents never close E-decisions** (conventions §4): P16-E-2 remains OPEN with its DEFAULT (option (a) — additive v2 + registered actions, the option implemented here because the subtask explicitly says so); options (b) and (c) stand REJECTED by the plan's own analysis; the closer is the human owner (see record 65).

## 4. Honest boundaries

1. Nothing has compiled or run; the serde round-trip tests are WRITTEN-UNRUN (execution is the human gate).
2. The v2 payload field selections are the proposal's (an ASSUMPTION note mirrors the P1.09 assumptions pattern: adjusting a field is a v2-schema-version change recorded in the decision review, never a silent repurpose).
3. The permitted-tool outputs are quoted in the batch close-out record (66).
