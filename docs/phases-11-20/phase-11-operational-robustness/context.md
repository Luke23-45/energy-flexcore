# Phase 11 — Operational Robustness and Observability — Context

Date: 2026-11-14. Status: **NOT STARTED** (0/22 — see `ledger_phase_11.md`).
This directory is PLANNING ONLY: no code, crates, or manifests are created by
this program's planning step (program conventions §3).

## 1. Why this phase exists

The engineering-practices research (gap analysis §5, Track D, Tier 0/1) found
that the platform has the *raw materials* of operations — a Prometheus-format
`/metrics` endpoint and JSON request-correlation logs — but none of the
*practice* built on them. Specific findings, each with its source:

| # | Finding | Source / evidence |
|---|---|---|
| 1 | No SLOs, no alert rules, no dashboards: the keyword sweep found zero hits for "slo" and "error budget"; the `/metrics` body is rendered by `MetricsRegistry` but scraped by nothing, and no rule files exist anywhere in the repo | gap analysis §5 Tier 0 row 1; §1 Track A |
| 2 | No distributed tracing: OpenTelemetry is a docs mention only; no SDK in any crate. Rust SDK maturity is uneven (metrics and logs Stable, traces Beta), so versions must be pinned deliberately at implementation | [S-63]; gap analysis §5 Tier 0 row 2 |
| 3 | Logs are JSON but carry no OTel-model `trace_id` or `SeverityNumber` mapping; the existing 32-hex id is a per-process correlation handle, explicitly not a cross-process trace id | [S-62]; `apps/control-plane/src/metrics.rs` recorded decision |
| 4 | Idempotency is mandatory and enforced on the event/outbox path, but the HTTP command-submission path neither requires nor honors an `Idempotency-Key` header | gap analysis §5 Tier 0 row 4; [S-61][S-65] |
| 5 | Audit chains are test-grade FNV-1a 64 (LR-203); production crypto is a DEPLOYMENT REQUIREMENT that is not yet present; hash-chained audit logs are the industry baseline | [S-88]; `crates/edge-agent/src/audit_proof.rs` (`fnv1a64-chain` label); LR-203 |
| 6 | Secrets live in Compose `.env`; the research default is dynamic, short-lived credentials with TTL | [S-74][S-75]; gap analysis §5 Tier 0 row 8 |
| 7 | RLS is present with transaction-scoped tenant context, but the RLS + connection-pooling interaction is unaudited | [S-69][S-70]; gap analysis §5 Tier 0 row 9 |
| 8 | Migrations 0002–0018 exist; no expand/contract policy document governs lifecycle-table changes | [S-71]; gap analysis §5 Tier 0 row 10 |
| 9 | JetStream hardening incomplete: a per-consumer DLQ path exists, but MaxDeliver limits, dedicated DLQ streams per subject family, DLQ-depth alerts, and schema-version headers are not assembled as a package | [S-65]; gap analysis §5 Tier 0 row 5 |
| 10 | Per-tenant rate limiting exists only for plugin call rates; there is no API-edge limiter and no per-tenant in-flight command cap | [S-85]; gap analysis §5 Tier 0 row 6 |

Keyword sweep results inherited from the gap analysis (75-term sweep, §1
Track A): "slo" / "error budget" — zero hits; "otel" — docs mention only, no
SDK; "webhook" — zero hits (outbound webhooks are Phase 13 scope; named here
only because the same sweep produced them).

Phase 11 is the foundation phase of the program: §9 of the gap analysis shows
every later phase consuming P11 outputs (SLOs as the Phase 12 steady state,
the audit chain for Phase 13, metrics for Phases 14–16, capacity signals for
Phase 20).

## 2. What exists today (the real seams)

All paths below exist in the Phase 0–10 workspace, which is WRITTEN-UNRUN —
these are written modules with records, not verified behavior.

| Seam | Path | What it gives Phase 11 |
|---|---|---|
| Metrics registry | `apps/control-plane/src/metrics.rs` | Hand-rolled `MetricsRegistry` rendering Prometheus text format: request/error families plus the P2.21 telemetry families with recorded cardinality decisions; `track_metrics` middleware emitting one JSON correlation log line per request (matched route pattern, tenant/actor, no `Authorization` header); `next_trace_id()` — 32 hex, per-process, explicitly not a UUID |
| Metrics route | `apps/control-plane/src/metrics_route.rs` | The `/metrics` handler (`text/plain; version=0.0.4`) |
| Control-plane probes | `apps/control-plane/src/probes.rs` | P1.22 `/health` (liveness) vs `/ready` (readiness) with named dependency checks — the base P11.21 expands |
| Worker probes | `apps/worker/src/probes.rs` | Same liveness/readiness semantics for the worker |
| Ingestion pipeline | `apps/worker/src/ingestion.rs` | The 7-stage pipeline with bounded-channel backpressure (shed + retry, never silent drop), quarantine counting, outbox-only publishing; composition in `apps/worker/src/main.rs` (`compose_ingestion`), hosting in `runner.rs` / `supervision.rs`, config in `config.rs` |
| Idempotent consumer | `crates/adapters/nats/src/consumer.rs` | `IdempotentConsumer`: inbox-check → process → ack-after-commit; `ProcessOutcome` / `ProcessReport` counters; poison path publishes to `flexcore.dlq.v1.<area>.<event_slug>` (captured by `FLEXCORE_DLQ`) then `+TERM`; `max_deliver` from `topology::ConsumerPolicy`; the `$JS.EVENT.ADVISORY.CONSUMER.MAX_DELIVERIES.>` advisory named as the operator's secondary signal |
| Audit record + port | `crates/flexcore-audit/src/record.rs`, `sink.rs` | `AuditRecord` (§10.1 field mapping, corrections as new records) and `AuditSink` — a one-method `append` port, append-only as a type-level fact |
| Audit persistence | `crates/adapters/postgres/src/audit.rs` | INSERT-only persistence under the `flexcore.tenant_id` RLS binding (append-only layers 1–3: grants, trigger, port) |
| Test-grade chain | `crates/edge-agent/src/audit_proof.rs` | P9.32 chain-verification proof surface; digest algorithm label `fnv1a64-chain`, explicitly non-cryptographic — the LR-203 upgrade target for P11.15 |
| Tenant context | `crates/flexcore-domain/src/tenant_context.rs` | `TenantContext` constructible only from authenticated claims (single greppable path) |
| Event envelope | `crates/flexcore-events/src/envelope.rs` | `EventEnvelope` with a mandatory validated `IdempotencyKey` and `SUPPORTED_SCHEMA_VERSION` = 1 |
| Command lifecycle | `crates/flexcore-domain/src/command_state.rs` | The frozen 14-state / 27-edge lifecycle; terminal states COMPLETED, REJECTED, CANCELLED, SUPERSEDED, FALLBACK (FAILED and EXPIRED exit only via FALLBACK) — the dedupe oracle (P11.12), the span taxonomy (P11.04), and the stuck-state domain (P11.20) |
| Tenant RLS binding | `crates/adapters/postgres/src/unit_of_work.rs` | Transaction-scoped `flexcore.tenant_id` (SET LOCAL semantics) applied before queries — the pattern P11.19 audits everywhere |
| Migrations | `crates/adapters/postgres/migrations/` | 0002–0018 (0002_tenants … 0018_edge_device_registry) — the P11.18 lock-risk audit target, including lifecycle tables (0016_commands_p4_10, 0017_feasibility_checks_p4_30) |
| Python workers | `python/workers/` | `worker_template` (`server.py`, `handler.py`, `health.py`, `config.py`), `contracts/`, `tests/` — the P11.05 / P11.08 surface for the Python SDK and emit-time redaction |
| Compose deployment | `deploy/compose/` | `docker-compose.yml`, control-plane and worker Dockerfiles — the P11.06 Collector gateway shape and the P11.16 Compose secrets floor |

## 3. Constraints

1. **Loop bans** (conventions §1): no installs, no builds, no test runs. Test
   files are written complete and stay UNRUN — their execution is a human
   gate. Only `verify_docs.py`, `validate_agent.py`, and
   `check_dependency_rule.py` are permitted as verification tools.
2. **Dependency rule** (conventions §6.1): domain depends on nothing
   infrastructure. OTel SDKs, the Collector, gitleaks, and Vault tooling are
   adapter / app / composition concerns; `flexcore-domain` stays pure.
3. **Production crypto vs test-grade** (conventions §6.6): the audit-chain
   upgrade (P11.15) moves the production path to real cryptography and marks
   it a DEPLOYMENT REQUIREMENT; the existing FNV-1a chain remains labeled
   test-grade and is never claimed as a release signature.
4. **ADR-010 and the simulation/shadow ceiling** (conventions §6.3, §6.7):
   nothing in this phase touches dispatch authority. Observability *reads*
   the 14-state lifecycle, its metrics and spans; it never drives it.
5. **The 0–10 base is WRITTEN-UNRUN**: the first human `cargo build
   --workspace` may surface recorded defects (F-P10.07-5 clock median
   defect; KN-0 register discrepancy). Every plan here assumes that base
   compiles; that assumption is a recorded program-level risk (program
   README, hard status boundary 1).
6. **Human gates and pre-registration** (conventions §5): every metric
   target, alert threshold, and dashboard acceptance is recorded UNRUN /
   EMPTY until a human run fills it; thresholds are registered before any
   run that could fill them. E-decisions are never closed by agents.

## 4. Glossary

| Term | Meaning as used in this phase |
|---|---|
| SLI | Service level indicator — a measured ratio (good events / total events) for one user-facing service |
| SLO | Service level objective — a target on an SLI over a window; in this phase a RECOMMENDATION pending operator sign-off (P11.02) |
| Error budget | 1 − SLO target: the tolerated imperfection that burn-rate alerts spend [S-61] |
| Burn-rate alerting | Multi-window multi-burn-rate pattern: fast burn (14.4x, 1h+5m windows) pages, slow burn (6x, 6h+30m) pages, low burn (1x, 3d) tickets [S-61] |
| OTel signals | The three OpenTelemetry signals: traces, metrics, logs [S-63] |
| OTLP | The OpenTelemetry protocol apps use to export signals to a Collector [S-63] |
| Collector gateway | Deployment shape where every process exports to a local Collector that batches, samples, and forwards [S-64] |
| Head/parent-based sampling | Sampling decided once at the root span and propagated to all children |
| Tail sampling | Sampling decided at the gateway after all spans of a trace have arrived |
| DLQ | Dead-letter queue: here the JetStream subject family `flexcore.dlq.v1.*` with `+TERM` semantics (existing consumer poison path) |
| Backpressure | Bounded-channel shedding with retry and a shed metric (existing ingestion behavior) — never silent dropping |
| Token bucket | Rate-limit algorithm: refill rate + burst capacity, per key |
| Expand/contract migration | Zero-downtime pattern: additive change → dual-write/backfill → switch → contract removal, never an in-place destructive ALTER [S-71] |
| Cryptographic vs test-grade digest | SHA-256/HMAC-class construction vs the non-cryptographic FNV-1a 64 chain (LR-203); only the former is deployable as tamper evidence |
| DSAR | Data subject access request (GDPR Art. 15 class) — NOT exercised in this phase; P11.08's emit-time redaction defines the PII boundary Phase 13's data-governance work will consume [S-86] |
| SeverityNumber | The OTel log-severity field; mapped from `tracing` / `structlog` levels in the log-schema work [S-62] |

## 5. Boundaries — what this phase does NOT do

1. **No HA deployment and no backup/DR** — Phase 12 (survivability) and
   Phase 20 (deployment architecture). P11.19 produces a written PgBouncer
   transaction-mode compatibility plan; it does not deploy a pooler.
2. **No Kubernetes, no GitOps** — Phase 20, when the recorded triggers fire
   [S-72].
3. **No IdP/SSO/SCIM, no MFA, no outbound webhooks** — Phase 13 ("webhook":
   zero hits today; it enters with the enterprise integration platform).
4. **No metrics-store deployment beyond Compose shape** — P11.10 is a
   selection record, not an installation.
5. **No control-authority change of any kind** — the release ladder,
   ADR-010, and the never-granted permission set are untouched.
