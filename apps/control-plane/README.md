# flexcore-control-plane (P1.17)

The Axum control-plane API — the composition root of Neryva FlexCore's
Phase 1 registry surface. This crate is the ONLY place that wires real
adapters to ports (§6.2): the PostgreSQL store (P1.13), the broker client
(P1.15), the authz policy set, and the identity resolver.

## Control status

Simulation/shadow-only infrastructure. The entire 0.x line is
simulation-only (AGENTS.md §5); this API performs supervisory registry
operations (tenants, portfolios, sites, assets) and authorises NO field
control. There is no command-dispatch endpoint in Phase 1.

## Authentication status (recorded, ledger P1.17)

Phase 1 ships **no production OIDC/OAuth2 integration**. The only identity
resolver is `DevAuthResolver`, compiled ONLY behind the `dev-auth` feature
(dev/test profiles; token format documented in `src/auth.rs`). It is not a
default feature and must never be enabled in release builds — the CI gate
(P1.27) fails if it appears in default features, and
`tests/api.rs::dev_auth_is_not_a_default_feature` pins the manifest
assertion. Without the feature, `DenyAllResolver` denies every request:
there is no anonymous fallback and no hard-coded tenant.

Authorization is deny-by-default with explicit grants
(`auth::phase1_default_policies`): PlatformAdmin gets the full registry
surface; TenantAdmin gets tenant-scoped writes/reads; Operator/Viewer get
reads. Every grant carries a stable rule reference as allow evidence.

## Recorded 403/404 decision (no existence leak)

- **401** — missing or invalid authentication (no verified claims).
- **403** — the CALLER's own context is the problem (policy denial; unknown
  caller tenant).
- **404** — cross-tenant or missing or unparsable resource references.
  The application layer deliberately makes a cross-tenant reference
  indistinguishable from a missing one; the API preserves that (CONTEXT.md
  open question 6).
- **422** — validation rejections (stable envelope `validation_rejected`).
- **500** — opaque infrastructure failures; backend error text never
  reaches the client; the trace id in the JSON logs is the correlation
  handle.

## Configuration (P1.21)

| Variable | Required | Default | Meaning |
|---|---|---|---|
| `FLEXCORE_DATABASE_URL` | yes | — | PostgreSQL connection string (secret; never logged) |
| `FLEXCORE_NATS_URL` | yes | — | NATS server URL (secret; never logged) |
| `FLEXCORE_BIND_ADDR` | no | `0.0.0.0:8080` | HTTP bind address |
| `FLEXCORE_LOG_LEVEL` | no | `info` | trace/debug/info/warn/error |
| `FLEXCORE_OUTBOX_POLL_INTERVAL_MS` | no | `1000` | reserved for the worker's outbox poll (worker-owned) |
| `FLEXCORE_DRAIN_TIMEOUT_SECS` | no | `30` | bounded graceful-shutdown drain window (P1.22) |

Missing required values fail fast at startup with a named error that never
contains the secret value; `Config`'s `Debug` impl redacts both URLs.

## Probes and lifecycle (P1.22)

- `GET /health` — liveness: 200 while the process is up (dependency state
  deliberately ignored, so a dependency blip fails readiness without
  restart-looping a healthy process).
- `GET /ready` — readiness: 200 only when every dependency check passes and
  the process is not shutting down. The database check is one real
  round trip (open a platform-level unit of work and commit it; the
  adapter exposes no dedicated ping). The broker check is one core request
  to `_flexcore.healthcheck.v1.ping` — a subject outside every stream
  filter; the "no responders" reply PROVES the round trip and is the
  healthy outcome.
- Graceful shutdown: on SIGTERM/SIGINT readiness flips false FIRST, in-flight
  requests drain within `FLEXCORE_DRAIN_TIMEOUT_SECS`, then the process
  exits 0.

## Observability (P1.20)

- `GET /metrics` — Prometheus text exposition of the request/error counters
  (`flexcore_http_requests_total`, `flexcore_http_errors_total`; route
  labels use the matched path pattern, never the raw path).
- One JSON log line per request: trace id, method, route, status, duration,
  tenant id, actor subject. The trace id is 32 hex chars from a std-library
  hash of (wall-clock nanoseconds, per-process counter) — a documented,
  dependency-free correlation handle, not a UUID. The `Authorization`
  header is never logged; identity is resolved for correlation only, and a
  resolution failure simply leaves tenant/actor absent.

## The unit-of-work boundary

The real repository ports are SYNCHRONOUS and TRANSACTION-BOUND: the
PostgreSQL adapter implements all four repository ports plus the outbox on
`PgUnitOfWork` — one native sqlx transaction. State therefore holds no
long-lived repositories; every request runs its use case or read through
`AppState::run_in_uow`, which on a blocking thread opens ONE unit of work
(with the transaction-scoped RLS tenant GUC when a `TenantContext` is
supplied), coerces the single `Arc<PgUnitOfWork>` into the five port trait
objects (all sharing that one transaction), builds the use cases, runs the
operation, and commits via `Arc::try_unwrap`. Any error drops the unit of
work, which spawns the native rollback (P1.10 adapter rule 4).

## Running

```sh
# dev build with the dev identity resolver (NEVER release):
cargo run -p flexcore-control-plane --features dev-auth

# release build (every request 401s until a real identity integration lands):
cargo run -p flexcore-control-plane
```

API surface: Stage 1 registry subset of `/api/v1` (tenants, portfolios,
sites, assets — create/list/get) plus `/api/v1/openapi.json` (generated
from the utoipa annotations, never hand-written).

## Tests

`tests/api.rs` composes the Test-variant backend (`flexcore_ports::fakes` +
a fixed identity resolver) and exercises the router in-process via
`tower::ServiceExt::oneshot` — no network, no database.
