# flexcore-ports (P1.07)

The capability interfaces of the FlexCore control plane (report §6.2, §8.1,
§9.1; ledger P1.07). **Ports define interfaces, not implementations**: every
port is a sync trait, every implementation detail belongs to an adapter
(P1.13 persistence, P1.15 broker, P1.19 API composition). The one exception
is the public `fakes` module — documented **test doubles** for the contract
tests, never for deployment.

## Port inventory

| Trait | Module | Stage 1 role |
|---|---|---|
| `TenantRepository` / `PortfolioRepository` / `SiteRepository` / `AssetRepository` | `repository` | CRUD + tenant-scoped queries, one port per aggregate |
| `EventOutbox` | `event_outbox` | Append one complete transactional write; read pending entries for the publisher; record attempts; mark delivered/failed |
| `Clock` | `clock` | Deterministic time for tests (`now_event_time` / `now_ingest_time`) |
| `IdGenerator` | `id_generator` | New `FlexUuid` values and typed entity identifiers |
| `TelemetrySource` | `telemetry_source` | **Real contract (P2.11)**: tenant-scoped query, canonical normalized record, subscribe/decode/source_id/health + provider declaration. No implementation in this crate — the Phase 2 provider set is exactly one (the simulator, P2.14) |
| `TenantContextSource` | `tenant_context_source` | Documented integration point for the P1.06 claims boundary (no implementation possible yet — see below) |
| `AuditSink` | **not here** | Lives in flexcore-audit (see the single-home note) |

## Recorded decisions

### One port per aggregate

One repository port per Stage 1 aggregate (Tenant, Portfolio, Site, Asset)
rather than a single mega-repository. Each aggregate has its own lifecycle
and its own tenant-scoping rule; separate traits keep the `TenantContext`
obligation precise (P1.07 task recommendation, accepted).

### Tenant scoping is structural (P0.16 rules 1, 5, 7)

Every method that touches tenant-owned data takes the domain `TenantContext`;
**no method for tenant-owned data exists without one** (no raw-`TenantId`
query path). The one recorded exception is `TenantRepository`: the tenant
registry root is not tenant-owned data — it *is* the tenant (P1.12 §4.1: no
`tenant_id` column, no per-tenant row filter in Phase 1). Platform-level
tenant administration is an application-authorization concern (P1.11).
Adapter behaviour: cross-tenant **writes** are refused with
`RepositoryError::TenantMismatch`; cross-tenant **reads** read as
"not found" — existence is never leaked across a tenant boundary.

### Sync traits; asynchrony is an adapter decision

No port takes or returns a future, spawns a task, or names an async
runtime. If a real adapter needs asynchrony (streams, callbacks,
concurrency), **the adapter decides how to bridge it** — e.g. an async-side
wrapper around the sync trait, or a worker thread with blocking hand-off —
and records that decision with the adapter. The ports stay sync so the
domain and application layers keep depending on nothing runtime-shaped.

### The repository traits live HERE (single-home rule)

Repository-abstraction traits live in this crate, **not** in
flexcore-storage — decided in P1.07 and recorded by P1.10 step 1, which
resolved the "may live here or in storage" ambiguity in favour of ports.
flexcore-storage holds only the non-port persistence abstractions
(`UnitOfWork`, `TransactionalWrite`, `RetentionPolicy`); the P1.30 boundary
audit checks that no repository trait is defined twice.

The append-only audit write port `AuditSink` also does **not** live here:
it is a recorded P1.10 exception and single-homes in **flexcore-audit**
(it is a one-way write valve, not a repository trait). This crate does not
re-export it — the ports → flexcore-audit edge is not in the allowed
dependency set (`tools/check_dependency_rule.py`), so application and
composition code references `flexcore_audit::sink::AuditSink` directly.

### `OutboxWrite` mirrors the storage `TransactionalWrite` (dependency rule)

The transactional-outbox rule (report §6.3 step 1, §15.5) wants the port to
carry "a state change + an outbox record in one transaction". The natural
types — the storage `TransactionalWrite`/`StateChangeDescriptor` and the
event-crate `OutboxRecord` — are one dependency edge away, and the rule
allows flexcore-ports only the contracts and domain edges. The port
therefore defines `OutboxWrite` with exactly the parts it can name, and the
P1.13 adapter maps them 1:1:

| Port-side | Adapter-side |
|---|---|
| `StateChangeReceipt` (entity, operation, tenant scope) | storage `StateChangeDescriptor` |
| `OutboxWrite.idempotency_key` / `destination_subject` / `payload` / `created_at` | event-crate `OutboxRecord` fields (payload stored verbatim) |
| `OutboxWrite` (both halves) | storage `TransactionalWrite` |

The structural half of the rule is preserved here: a repository write
returns a `StateChangeReceipt` (`#[must_use]`), and `OutboxWrite::new`
consumes it — an outbox record cannot be assembled without the state change
having been written first.

### "The caller's unit of work" is a composition-root + adapter obligation

A port value cannot name a transaction. The binding rule, stated on the
`EventOutbox` port: the composition root opens one unit of work (the
flexcore-storage `UnitOfWork`) per use-case invocation and obtains
unit-of-work-scoped port instances from the adapter; every port call inside
the invocation executes inside that one native transaction, all-or-nothing.
Mechanical enforcement is the P1.13 adapter atomicity test ("an injected
failure leaves neither state nor outbox, or both"); the in-crate
`FakeWriteJournal` provides the ordering evidence for the application tests.

### Clock: two methods, not one `now()`

`now_event_time` and `now_ingest_time` exist because the P0.09 time model
makes the two timestamps **distinct types with no conversion** — one
`now()` returning a single type would invite call sites to reuse it for the
other meaning. Implementations derive both from the same instant
(`ingest_time >= event_time` holds trivially for a fake tick).

### `TelemetrySource`: the real contract (P2.11)

P1.07 defined the port as a **shape-only seam**; P2.11 replaced that shape
with the real contract (the superseded `DecodedSample`/`TelemetryScope`
types had no consumer anywhere — verified by grep at change time — and the
transition is recorded in
`ledger/phase-02-asset-telemetry/11_telemetry_source_contract.md`). The
contract, in four load-bearing pieces:

- **`TelemetryQuery` — tenant scoping is structural.** The only
  constructor takes the domain `TenantContext` and derives the tenant from
  it; there is no constructor without one, so a query without a tenant
  context is a compile error (P0.16 rules 1 and 5, same pattern as the
  repository ports). The query names a non-empty asset set, a
  `from <= until` range over `event_time`, and a non-empty quantity set
  (no implicit "all quantities" wildcard — decided once, recorded in the
  P2.11 document).
- **`NormalizedTelemetry` — the canonical record, or nothing.** One record
  is one quantity of one asset at one `event_time` (P2.04 D-T2). Values
  travel in the `flexcore-contracts` canonical unit newtypes (`ActivePowerMW`,
  `ReactivePowerMVAr` **with** its declared `ReactiveSignConvention`,
  `StateOfCharge`, `EnergyMWh`) — a bare number or bare string cannot
  exist, and NaN/out-of-range values cannot be constructed. Construction
  is a `Result`: empty `source_id`, a measured record without a value, a
  value on a `MISSING` marker, a quantity/value kind mismatch, an empty
  text state, `ingest_time < event_time`, or a misused `flexcore:`
  namespace each reject the whole record. A provider that cannot produce a
  canonical record fails decoding — it never emits a partial record
  (P2.11 step 3).
- **Decode failure is `Err`.** `decode` returns the complete set of
  records one frame carries (a multi-quantity sample is a set sharing
  `source_id`, `source_sequence`, `event_time`, P2.04 D-T2); a successful
  result is non-empty and complete. There is no partial-record variant in
  the result type and no silent quality downgrade (P0.08 non-silencing).
- **The provider rule is structural.** `provider_declaration()` is a
  required trait method: every provider declares its cadence (expected
  per-asset emission interval; per-asset exceptions live in the provider's
  own configuration — the scenario, P2.13), its quality-state mapping onto
  the six canonical states (P0.08 §3, original code retained as
  `source_quality_code`), and its clock-quality behaviour (recorded, never
  assumed — P2.04 D-T5). Provider source IDs must never use the reserved
  `flexcore:` namespace (P2.04 D-T6); only the platform's gap/derivation
  detectors (P2.08) emit `flexcore:derived:<detector>` records.

Sync choice (unchanged from P1.07): the port is synchronous. `subscribe`
returns an opaque handle, not a stream; delivery, buffering, and ordering
are the adapter's concern (blocking hand-off, worker thread, or an async
wrapper around the sync trait), recorded with the adapter. Phase 2
provider set: **exactly one** — the simulator (P2.14). Real adapters
(MQTT, Modbus, OpenEMS) are later phases. No implementation ships in this
crate; the in-module contract tests use a minimal fake that pins the
decode-failure rule.

### `TenantContextSource`: the recorded claims-boundary gap

The domain `AuthenticatedClaims` type (flexcore-domain, P1.06) has private
fields and **no public constructor anywhere** — by deliberate design, so no
request field, plugin, or adapter can fabricate a `TenantContext` (P0.16
rule 5). The intended producer is the API middleware, constructing claims
from a *verified* token; that constructor is a deliberate, reviewed
decision scheduled with the tenant-context integration work (ledger P1.11
scope). Consequences, recorded honestly:

- **No implementation of this port can exist yet** — an implementation
  would have to build a `TenantContext`, and the only constructor requires
  an `AuthenticatedClaims` that is unconstructible outside flexcore-domain.
- The use cases receive `TenantContext` values as parameters from the
  composition root; nothing in Stage 1 depends on this port existing.
- The port is the seam the middleware implements once the domain exposes
  the claims constructor; every downstream layer keeps compiling unchanged.
- This task **did not edit flexcore-domain** (file ownership); the gap is
  recorded here and in the flexcore-application README.

### Fakes are public, and they are test doubles

The `fakes` module is compiled unconditionally and public so the
flexcore-application tests compose the same doubles (P1.07 step 5; P1.08
acceptance). They enforce the same rules the real adapter must (tenant
scoping, key uniqueness, delivery states) and journal every write into a
shared `FakeWriteJournal` — the ordering evidence for the §6.3 step-1 rule
(state change + outbox append adjacent, both before the single `Commit`
marker). They deliberately do not emulate rollback: all-or-nothing is the
real adapter's obligation and its atomicity test.

## No vendor types — grep check

The rule (ledger P1.07 acceptance): no port trait mentions a database
engine, message broker, HTTP framework, or async runtime. A source-text
test in `src/lib.rs` asserts the same set of identifiers mechanically; the
CI gate (P1.28) runs the grep. Recorded output of the required check:

```
$ grep -rinE "sqlx|nats|axum|postgres|tokio" crates/flexcore-ports/src/
(no matches — exit status 1; 0 hits across 10 source files)
```

## Verification (recorded, run 7 November 2026 work session)

```
$ python tools/check_dependency_rule.py
OK — dependency rule holds for 12 workspace members.

$ grep -rinE "sqlx|nats|axum|postgres|tokio" crates/flexcore-ports/src/
(no output; exit status 1 = zero matches)

$ grep -rinE "TODO|FIXME|todo!|unimplemented!" crates/flexcore-ports/src crates/flexcore-application/src
(no output; exit status 1 = zero matches)
```

`cargo build`/`cargo test` were **not** run for this subtask (hard task
constraint — the human builds and tests). The source-text vendor check runs
inside the crate's own test suite when the human builds it.

## Scope and control status

Capability contracts for the design baseline only. The entire 0.x line is
simulation-only; nothing here touches a database, a broker, a device, or a
live grid. FlexCore is a supervisory orchestration layer and performs no
primary frequency response, synthetic inertia, relay protection, inverter
inner-loop control, or sub-cycle control.
