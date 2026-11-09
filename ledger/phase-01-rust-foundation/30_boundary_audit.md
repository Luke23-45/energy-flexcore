# P1.30 — Dependency-rule and crate-boundary audit

Date: 2026-11-07. Method: STATIC audit (the loop bans forbid running cargo;
`cargo tree`/`cargo metadata` evidence is the human's). The mechanical
manifest check — `python tools/check_dependency_rule.py` — was run after
every implementation batch; its recorded output at Phase 1 close:

```
OK — dependency rule holds for 12 workspace members.  (exit 0)
```

## 1. Per-crate dependency check

| Crate | Internal deps (allowed set) | Infra deps | Result |
|---|---|---|---|
| flexcore-contracts | — | serde, serde_json, time (allowed value-type deps) | PASS |
| flexcore-domain | contracts | none | PASS |
| flexcore-events | contracts, domain | serde, serde_json, time | PASS |
| flexcore-ports | contracts, domain | none | PASS |
| flexcore-application | contracts, domain, events, ports | none | PASS |
| flexcore-storage | contracts, domain, events | serde | PASS |
| flexcore-audit | contracts, domain | serde | PASS |
| flexcore-authz | domain (contracts dev-only) | none | PASS |
| adapters/postgres | ports, storage, audit, events, domain, contracts | sqlx, tokio (allowed — adapter) | PASS |
| adapters/nats | ports, events, domain, contracts | async-nats, tokio (allowed) | PASS |
| apps/control-plane | everything (composition root) | axum, tokio, tower-http, utoipa, tracing (allowed — app) | PASS |
| apps/worker | everything (composition root) | axum, tokio, sqlx, async-nats (allowed — app) | PASS |

## 2. Empty-crate check

All 12 members carry real code with clear ownership and a stable dependency
direction (§8.1). No member was left as a doc-only skeleton: the skeleton
lib.rs files created by P1.03 were replaced by their owning subtasks within
the phase. **Zero empty crates.**

## 3. Second-domain-layer check

No `flexcore-kernel` crate exists — the kernel is a composition concern
that lands with plugin composition (Phase 7); recorded per the plan. No
crate drifted into a "second domain layer".

## 4. Duplication check

- Repository traits: ONE home — `flexcore-ports` (`P1.10` decision;
  `flexcore-storage` holds only UnitOfWork/TransactionalWrite/retention).
- Audit port: ONE home — `flexcore-audit::sink::AuditSink` (recorded
  exception: not a repository trait). `flexcore-ports` references rather
  than redefines it.
- `InboxStore` trait: home = `flexcore-nats-adapter` (consumer-side
  infrastructure; recorded single-home decision in that crate; the durable
  `PgInboxStore` implementation lives in `apps/worker` with an explicit
  promotion note into the postgres adapter).
- `OutboxWrite` (ports) mirrors `TransactionalWrite` (storage) 1:1 — the
  ports→storage edge is forbidden by the dependency rule, so the mirror is
  recorded as deliberate (both READMEs document it).
- `TenantContext`/`AuthenticatedClaims`: ONE home — flexcore-domain; authz
  and application reuse, never duplicate.

## 5. Violations found and fixed during the phase

1. Event vocabulary originally lived in `flexcore-domain` (Phase 0 seed) —
   moved to `flexcore-events` (domain → events is a forbidden edge).
2. `AuthenticatedClaims` had no constructor anywhere (nothing could build
   a TenantContext) — coordinator added the single public
   `AuthenticatedClaims::from_verified(VerifiedClaims)` path with the
   reviewed-call-site contract (API middleware).
3. Subject areas in `flexcore-events` diverged from the frozen P1.14
   design on 8 of 26 events — reconciled to the P1.14 table, with the 4
   stale literals elsewhere updated.

All fixes were in-place edits; no violation remains. Every residual risk
is in the "unbuilt code" category: the human's first `cargo build` is the
real boundary proof (the static check covers manifests, not source-level
`use` statements).
