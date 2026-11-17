# P16.19 — Market adapter permission model: market.* actions, operator-approved + per-market grants, never-granted REVIEW (byte-identical), the P14 command-relay-only rule consumed

| Field | Value |
|---|---|
| Subtask | P16.19 (Market adapter permission model — bidding/dispatch-relay requires operator-approved mode + explicit per-market grants; never-granted REVIEW, the set stays byte-identical; command-relay-only rule consumed from P14) |
| Wave / agent | Phase 16 wave 2, Agent B |
| Status | WRITTEN-UNRUN: the action variants, the grant module, and their tests are written, never compiled, never executed. **No market is connected, no bid is submitted (none can be), no grant is issued, and no dispatch authority is created anywhere** (`NO_BID_SUBMITTED_STATEMENT`; ADR-011; conventions §7.3). |
| Sources | [S-90] (PJM FERC 2222 participation model — the aggregation-bidding posture the grant model scopes), [S-100] (ERCOT ADER pilot — the per-pilot per-market posture class), [S-120] (ESIG MMS aggregated-DER bids — the grant/mode discipline framing); P16.06 `MarketBidGrant` (`crates/flexcore-application/src/bid_composition.rs`, consumed BY CITATION); P14.16 (`crates/flexcore-kernel/src/adapter_permissions.rs`, the command-relay-only rule + never-granted review pattern, consumed BY CITATION); P13 apikey.* precedent (`crates/flexcore-authz/src/phase13_policies.rs`); conventions §6.4 |
| Files touched | `crates/flexcore-authz/src/action.rs` (additive: six `Market*` variants + id arms + parse arms + extended tests); `crates/flexcore-authz/src/phase16_market_policies.rs` (NEW); `crates/flexcore-authz/src/lib.rs` (additive module line + comment) |

## 1. What landed

| Item | Content |
|---|---|
| Six `market.*` actions (additive closed-enum extension) | `market.price.read` (the P16.07 record layer), `market.bid.compose` (the P16.06 composition service — a RECORD action: no bid is submitted, none can be), `market.grant.issue` / `market.grant.revoke` / `market.grant.read` (the P16.06 grant model's administration), `market.settlement.read` (the P16.09/P16.12 record layer). Each follows the extension rule exactly: a variant, its `id` arm, its `parse` arm, and explicit policy grants — nothing matches by default |
| Deliberately ABSENT from the surface | `market.bid.submit` (no bid is submitted — none can be), `market.dispatch.*` in every spelling (the P14 command-relay-only rule — see §2), any market approval action (approvals are `command.approve`'s alone), any settlement write/adjust action (the engine never adjusts — P16.09). All pinned unparseable by test |
| `phase16_market_policies()` | The recorded RECOMMENDED-DEFAULT grant table (reviewable): `market.price.read` / `market.settlement.read` → PlatformAdmin, TenantAdmin, Operator, Viewer (tenant scope); `market.bid.compose` → Operator, TenantAdmin (tenant scope); `market.grant.issue/revoke/read` → PlatformAdmin, TenantAdmin (tenant scope — grant administration is administrative authority, the P13.08 posture mirrored). SiteApprover has NO market grant anywhere (the P13 separation stands). Every allow decision carries the stable rule reference `policy:phase16-market:<role>:<action>:tenant` |
| Grant-rule consumption | The RBAC grant is ONE necessary condition, never a sufficient one: the P16.06 composer separately enforces operator-approved mode (the exact kernel posture name `operator-approved`, consumed by name) plus an ACTIVE, unexpired, tenant-matched, market-matched `MarketBidGrant` RECORD on every composition call. The authz surface does not replace, restate, or bypass that record-level gate |
| Never-granted REVIEW | The set is quoted VERBATIM in the module header and in §2 below; the review verdict is that the market surface needs NONE of the four ids; a test pins the byte-identical set and the disjointness of every `market.*` id from it |
| Command-relay-only rule | CONSUMED BY CITATION from P14.16 (`adapter_permissions.rs`): commands reach devices only through the 14-state lifecycle with all three gates distinct; a market adapter may relay an already-APPROVED command and ingest external events at PROPOSED — never authorize, never issue. Structural consequence: no `market.dispatch.*` action exists and none may be added; every `market.*` grant carries zero dispatch reach |

## 2. THE NEVER-GRANTED REVIEW (conventions §6.4 — the set stays byte-identical)

Quoted verbatim from `crates/flexcore-kernel/src/manifest.rs` (the single home; the authz crate cannot depend on the kernel, so the module header carries the same byte-identical block):

```rust
pub const NEVER_GRANTED_PERMISSIONS: [&str; 4] = [
    "dispatch.authorize",
    "dispatch.issue",
    "secret.read",
    "network.any",
];
```

Review verdict (recorded): **UNCHANGED.** The Phase 16 market surface expresses every capability either as a `market.*` action on the authz surface (records and grant administration) or as a kernel-side adapter binding (`AdapterBinding`, the P14.16 model) — never as a §5.7 permission id, and never by widening the set. The `market.*` ids are DISJOINT from the four (pinned by test); the four parse to `None` and can only ever deny with `UnknownAction` (pinned by test, mirrored from the P13 module's pin).

## 3. Recorded decisions

1. **The P14.16 command-relay-only rule is consumed, never restated** (`P16_MARKET_COMMAND_RELAY_STATEMENT` implied by the module header): the rule lives in the kernel module; this surface's contribution is structural — no market dispatch action exists, so no `market.*` grant can ever be read as dispatch authority. A market adapter reaches dispatch ONLY through the command lifecycle.
2. **Grant administration is separated from market operation** (the P13 LR-049 direction carried forward): `market.grant.issue/revoke` land on PlatformAdmin/TenantAdmin only; the Operator role can compose bids under an existing grant but cannot mint one.
3. **The per-market grant is the P16.06 RECORD, not an authz rule**: `MarketBidGrant` (state vocabulary ACTIVE/REVOKED/EXPIRED, end-exclusive validity, P11 audit-chain link) stays in `bid_composition.rs` — one home. The authz actions authorize administering those records; the composer's own gate remains the enforcement point on every call.
4. **Deny-by-default is untouched**: the empty set denies everything; the `market.*` actions match nothing until `phase16_market_policies()` is explicitly composed by a composition root. No earlier grant set was edited (the phase4/phase13 modules are byte-identical).
5. **REVOKED is recorded via the supersede model** (P16.06): revoking is issuing a revoking record; the old grant row is retained — the authz surface records the ACTION, the record layer keeps the history.

## 4. Honest boundaries

1. No action has ever been evaluated at runtime (the crate is WRITTEN-UNRUN); no grant has been issued; no composition root composes `phase16_market_policies()` yet — the DEFAULT grant table is a RECOMMENDATION the human review may change.
2. No market is connected, no bid is composed or submitted, no settlement record exists; nothing here creates or implies dispatch authority, market connectivity, or qualification (ADR-011; conventions §7.3).
3. Every test is WRITTEN-UNRUN; execution is the human gate. The permitted-tool outputs are quoted in the batch close-out record (66).
