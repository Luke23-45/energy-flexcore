# P1.23 / P1.24 — Security and fault-injection test coverage note

| Field | Value |
|---|---|
| Subtasks | P1.23 (security test suite), P1.24 (fault-injection test slice) |
| Report trace | §19.2 (test layers 6–7), §17.4, §16.2, §6.3–§6.4, §21.2 |
| Source | Ledger P1.23/P1.24; threat table frozen in `ledger/phase-00-product-freeze/17_threat_model.md` §5 (report §17.4) |
| Status | Test files delivered; executable runs gated per file (see "How to run") |

## How to run

- The pure test files (authz, domain, application, ports, postgres clock-skew)
  run with the normal `cargo test` flow.
- `crates/adapters/nats/tests/fault_slice.rs` runs only with
  `FLEXCORE_TEST_NATS_URL` set; otherwise every test skips cleanly with a
  printed reason.
- `crates/adapters/postgres/tests/fault_slice.rs` database tests run only with
  `FLEXCORE_TEST_DATABASE_URL` set; the clock-skew tests need no infrastructure
  and always run.

## Coverage mapping: test file → acceptance criterion → threats

| Test file | P1.23 / P1.24 acceptance criterion | §17.4 threats covered |
|---|---|---|
| `crates/flexcore-authz/tests/security_authz.rs` | P1.23 deny-by-default: unknown action id denies; missing scope denies; missing context denies; cross-tenant scope denies (incl. PlatformAdmin, no bypass); unknown role-to-action pairs deny; every denial carries reason + evidence; `is_allowed()` false on all denials | Row 4 (cross-tenant leakage), Row 7 (unauthorized operator action) |
| `crates/flexcore-domain/tests/security_tenant_context.rs` | P1.23 tenant-context boundary: `from_verified` is the only public construction path (compile-fact negative test + positive-path test); tenant/subject round-trip verbatim; `VerifiedClaims` misuse documented (public fields by design, single greppable path) | Row 4 (tenant context half), Row 7 |
| `crates/flexcore-application/tests/security_isolation.rs` | P1.23 tenant isolation per repository method and per use case (checklist in the file header): cross-tenant `find_by_id` → `None` → `ParentOutsideTenantScope`; `list_for_tenant` → empty; duplicate registration rejected for tenant/portfolio/site/asset; outbox record journalled with the state change in one unit of work (fake-journal ordering) | Row 4 (access tests half), Row 6 (outbox half of the pattern) |
| `crates/flexcore-ports/tests/security_vendor_free.rs` | P1.23 file-level vendor-freedom check (independent mirror of the in-crate grep gate, P1.28): no sqlx/nats/axum/postgres/tokio identifier in ports src; module-list guard so a new source file cannot escape the check | Row 5 (lateral-movement containment: ports stay vendor-free), Row 8 (supply-chain containment) |
| `crates/adapters/nats/tests/fault_slice.rs` | P1.24: (a) duplicate delivery → single side effect (broker-dedup asserted first, then inbox dedup by idempotency key); (b) reordered publish per asset stream → both handled, arrival order asserted, no state corruption, re-presented older event skipped; (c) broker outage → entries stay pending with attempt counts and nothing marked (fault asserted first), recovery drains with zero loss; (d) poison message → `FLEXCORE_DLQ` after `max_deliver`, metadata present, quarantine counted in `ProcessReport`, original terminated (no infinite redelivery) | Row 1 (replay/duplication half), Row 6 (broker duplication/replay), Row 10 (central-outage Phase 1 slice: outbox backlog drains with zero loss) |
| `crates/adapters/postgres/tests/fault_slice.rs` | P1.24: (e) unreachable DB fails cleanly with the opaque `E_STORE_UNAVAILABLE` code (fault asserted first); mid-workflow injected failure actually fires inside the open unit of work, rollback leaves zero partial writes (neither state change nor outbox row), recovery commits cleanly; (f) far-future `event_time` is rejected by the frozen `ingest_time >= event_time` rule, never silently accepted; far-past `event_time` is a recorded gap (see below), clock quality is recordable | Row 9 (clock drift), Row 10 (storage-outage slice) |

## Replay tests (P1.23 step 3 ↔ P1.16)

The replay case ("a replayed request with the same idempotency key does not
duplicate the side effect") is covered in the fault slice,
`crates/adapters/nats/tests/fault_slice.rs` case (a): the same
`Nats-Msg-Id` published twice is broker-deduplicated (asserted), and the
re-delivered message is skipped by the consumer inbox (asserted), yielding
exactly one side effect and one inbox record.

## RLS tests (P1.23 step 4)

Database-level RLS for the runtime role and the owner role is the P1.13
subtask's tested obligation
(`crates/adapters/postgres/tests/integration.rs`); P1.23's "every service
role the deployment defines" case is covered by that file's
`flexcore_app` / `flexcore_migrator` cases. No additional RLS test is
duplicated here; the deployment defines exactly those two service roles in
Phase 1.

## Auth-bypass tests (P1.23 step 2)

The four token-failure cases (missing token, expired token, wrong
audience/issuer, token for a different tenant) belong to the API middleware
(P1.17), which owns OIDC verification; it does not exist in this slice.
Covered here: the "token for a different tenant" consequence at the
use-case and authorization boundary (tenant B's claims-derived context
cannot touch tenant A's data; cross-tenant scope denies). Recorded gap,
owned by P1.17.

## Not-yet-covered list (P1.23 step 5 / P1.24), with owning phases

| Item | Why not in Phase 1 | Owning phase |
|---|---|---|
| Command forgery (signed commands, sequence numbers, expirations) | No commands exist yet; the command lifecycle and the protected dispatch kernel are later work | Phase 4 |
| Plugin privilege escalation / sandbox escape | No plugin host exists yet (WASM host is P1.05-deliverable of the plugin SDK line) | Phase 7 |
| Load / performance / soak tests | §19.2 places them in a later test layer once the compose stack and worker exist | Later phases per §19.2 |
| Secrets exposure (leak scanning, secrets-manager integration tests) | Covered by the observability/config subtasks P1.20/P1.21, not by test files here | P1.20 / P1.21 (verification), later phases for continuous scanning |
| Hard process-kill / server-kill fault variants (consumer killed mid-batch against a live broker; full NATS/Postgres kill) | Need the Compose stack to kill services deterministically; this slice uses the recorded failure-path proxies | P1.25 (Compose stack) / P1.26 (exit-gate outage variant) |
| Clock-skew SUSPECT/STALE flagging of far-past events | P0.09 defines no maximum-age or future/past window rule and no automatic quality flagging; staleness travels as recorded quality fields (`TelemetryQuality::Stale`, `ClockQuality`) for downstream stale-data gates | Phase 2 (telemetry quality / stale-data gates, §17.4 rows 3 and 9) |
| Multi-tenant RLS under every future service role | Phase 1 defines exactly two service roles; new roles extend the P1.13 test matrix | Per new role, from Phase 2 on |

## Recorded gaps found while writing the tests (no existing file was edited)

- The `flexcore-application` in-crate tests and the `flexcore-ports` fakes
  carry "blocked tests" notes stating no `TenantContext` can be constructed
  outside `flexcore-domain`; the domain now exposes the public
  `AuthenticatedClaims::from_verified(VerifiedClaims)` path, so this slice
  exercises the cross-tenant test bodies those notes describe (as
  integration tests, without editing the stale comments).
- `crates/adapters/nats/tests/integration.rs` calls
  `FixedClock::new(0)`, but the public fake constructor is
  `FixedClock::at_unix_millis(...)` — the existing file likely does not
  compile as-is. The new fault-slice file uses the existing public
  constructor. Pre-existing issue, owned by the P1.15/P1.16 owners.

## Scope and control status

These tests cover the design baseline only. FlexCore is a supervisory
orchestration layer; the entire 0.x line is simulation-only, and nothing
here constitutes a certification claim, a safety case, or permission for
live grid control.
