# Phase 11 — Operational Robustness and Observability — Plan

Date: 2026-11-14. Status: **NOT STARTED** (0/22). Planning baseline only.
Ledger: `ledger_phase_11.md` (all boxes unticked). Conventions:
`../01_PROGRAM_CONVENTIONS.md` (binding).

## 1. Scope statement

Phase 11 turns the platform's existing observability raw materials — the
Prometheus-format `/metrics` endpoint, the JSON correlation logs, the
idempotent-consumer flow, the append-only audit trail — into an operational
practice, and closes the Tier 0 correctness gaps the gap analysis records:
SLOs and burn-rate alerting [S-61], the three OTel signals with a Collector
gateway [S-62][S-63][S-64], an idempotent command API, JetStream hardening
[S-65], per-tenant rate limiting [S-85], a production-grade audit chain
[S-88] (DEPLOYMENT REQUIREMENT), secrets out of `.env` [S-74][S-75], RLS +
pooling reconciliation [S-69][S-70], and an expand/contract migration policy
[S-71]. Everything lands simulation-only; nothing here changes dispatch
authority, ADR-010, the release ladder, or the never-granted permission set.

## 2. Subtask outline

| ID | Title | Summary | Key seams/files |
|---|---|---|---|
| P11.01 | Golden-signal metric taxonomy + instrumentation plan | Latency / traffic / errors / saturation per user-facing service, plus command-lifecycle SLIs: accept latency, terminal-state ratio, stuck-state count; the plan names every new family and its cardinality decision BEFORE any family is added (the P2.21 recorded-cardinality discipline carried forward) | [S-61]; `apps/control-plane/src/metrics.rs`, `apps/worker/src/ingestion.rs` |
| P11.02 | SLO register + error-budget policy | SLO register keyed to the P11.01 taxonomy; targets labeled RECOMMENDATION pending operator sign-off; error-budget policy states what happens at exhaustion; low-traffic services need synthetic traffic per [S-61] | phase records; P11.01 taxonomy |
| P11.03 | Multi-window multi-burn-rate Prometheus alert rules | Checked-in Prometheus rule files implementing the 14.4x / 1h+5m page, 6x / 6h+30m page, 1x / 3d ticket pattern [S-61], bound to P11.02 SLOs and the P11.20 stuck-state metric | new checked-in rule files (proposed under `deploy/compose/observability/`) |
| P11.04 | OTel tracing in control plane | OTLP export via `opentelemetry-otlp`; crate versions pinned (metrics + logs Stable, traces Beta [S-63]); span taxonomy over the 14-state lifecycle — one span per gate transition, no span invents a state | [S-63]; `apps/control-plane/src/main.rs`, `state.rs` |
| P11.05 | OTel in Python workers | Stable Python SDK [S-63]; `trace_id` propagation through JetStream message headers (publisher sets, consumer reads) so a request is followable end-to-end | [S-63]; `python/workers/worker_template/`, NATS publisher seam in `crates/adapters/nats/src/` |
| P11.06 | OTel Collector gateway deployment config | Single gateway container shape in Compose per [S-64]; sampling policy record: head/parent-based first, tail sampling deferred with its trigger recorded | [S-64]; `deploy/compose/docker-compose.yml` |
| P11.07 | Structured-log schema v1 | Log schema v1 mapped to the OTel logs data model [S-62]: mandatory `tenant_id` / `command_id` / `trace_id` attributes, `SeverityNumber` mapping for `tracing` / `structlog` levels | [S-62]; `apps/control-plane/src/metrics.rs` (correlation line), `python/workers/worker_template/server.py` |
| P11.08 | PII/secret redaction at emit | Redaction at emit time (especially Python workers); redaction unit tests written complete and UNRUN; no redaction-at-read promises | `python/workers/worker_template/` + `python/workers/tests/`, worker log paths |
| P11.09 | Dashboard-as-code | Grafana provisioning JSONs + recording rules checked into git; dashboards render only P11.01 families (no panel invents a metric) | new checked-in ops tree (proposed under `deploy/compose/observability/`) |
| P11.10 | Long-term metrics store selection record | VictoriaMetrics single-node vs second Prometheus with `remote_write`; DEFAULT: VictoriaMetrics single-node; registered as P11-E-2; vendor benchmarks stay UNVERIFIED (gap analysis §11) | decision record; [S-61] |
| P11.11 | Idempotency-Key enforcement on command POSTs | API contract addition on command-submission routes: key required, stored-response replay semantics, 422 (missing/invalid key) and 409 (replay conflict) semantics documented; key representation reuses the validated envelope key discipline | `apps/control-plane/src/routes.rs` + command routes; `crates/flexcore-events/src/envelope.rs` |
| P11.12 | Consumer idempotency keyed on command_id | The lifecycle state machine is the dedupe oracle: an invalid-state transition on a redelivered command is a no-op plus a duplicate-delivery metric; duplicate-delivery test suite written complete and UNRUN | `crates/flexcore-domain/src/command_state.rs`, command consumer under `apps/worker/src/`, `crates/adapters/nats/src/consumer.rs` |
| P11.13 | JetStream hardening | MaxDeliver limits aligned between helper and provisioned policy, dedicated DLQ streams per subject family, DLQ-depth alerts, bounded retention, `schema_version` headers on published messages | [S-65]; `crates/adapters/nats/src/topology.rs`, `consumer.rs` |
| P11.14 | Per-tenant token-bucket rate limiting + in-flight caps | Token bucket at the Axum edge + 429 / `Retry-After` contract + per-tenant in-flight command caps [S-85]; in-process now, Redis-backed when multi-instance (recorded); the edge-agent uplink is an explicit principal class so fleet uplink is never starved by interactive-operator budgets | [S-85]; `apps/control-plane/src/main.rs` middleware stack |
| P11.15 | Production-grade audit chain upgrade | Real crypto per P11-E-1 (SHA-256 / HMAC or better) as a DEPLOYMENT REQUIREMENT; INSERT-only DB role on audit tables; per-tenant chain verification; the FNV-1a chain stays labeled test-grade (LR-203) | [S-88]; `crates/edge-agent/src/audit_proof.rs`, `crates/flexcore-audit/src/`, `crates/adapters/postgres/src/audit.rs` + new migration |
| P11.16 | Secrets management | Vault DEFAULT recommendation (dynamic Postgres credentials TTL 1h, NATS creds, PKI short-lived certs later [S-74][S-75]); Compose floor: file-per-host secrets with restricted permissions; SQLx pool credential-refresh design (written, not deployed) | [S-74][S-75]; `deploy/compose/docker-compose.yml`, pool seam in `crates/adapters/postgres/src/store.rs` |
| P11.17 | Secret scanning + pre-commit + CI config | gitleaks-class scanning config, pre-commit hook config, and CI job config written and checked in — never run here; findings are a human gate | new config files (checked in, UNRUN) |
| P11.18 | Expand/contract migration policy + lock-risk audit | Policy document [S-71] + audit of migrations 0002–0018 for lock risk against lifecycle tables (0008_audit_records, 0016_commands_p4_10, 0017_feasibility_checks_p4_30); findings recorded, remediation scheduled | `crates/adapters/postgres/migrations/` |
| P11.19 | RLS + pooling reconciliation | Transaction-scoped `SET LOCAL` tenant context verified everywhere; audit of all SQLx session-level `SET` usage; PgBouncer transaction-mode compatibility plan (written, not deployed) | [S-69][S-70]; `crates/adapters/postgres/src/unit_of_work.rs`, `store.rs` |
| P11.20 | Stuck-state detection | Commands parked in non-terminal states (FAILED and EXPIRED included — they exit only via FALLBACK) beyond a registered threshold → gauge + alarm feeding the P11.03 rules | `crates/flexcore-domain/src/command_state.rs`, `apps/control-plane/src/metrics.rs` |
| P11.21 | Health-check expansion | Readiness/liveness semantics per Compose service; dependency health (Postgres / NATS reachability) surfacing in the `/health` / `/ready` pair; semantics documented per service | `apps/control-plane/src/probes.rs`, `apps/worker/src/probes.rs` |
| P11.22 | Decision records + close | Record P11-E-1 (audit crypto provider), P11-E-2 (metrics store), P11-E-3 (alert routing channel — no PagerDuty/Slack conformance claims); consistency audit across the phase's records | phase `records/` (created at implementation time) |

## 3. Implementation wave plan (2 agents)

| Wave | Agent A | Agent B |
|---|---|---|
| 1 | P11.01–P11.08 (observability) | P11.09–P11.14 (eventing / API hardening) |
| 2 | P11.15–P11.19 (security / data integrity) | P11.20–P11.22 (health + close) |

Disjoint ownership:

- **Wave 1, Agent A** owns: `apps/control-plane/src/metrics.rs`,
  `metrics_route.rs` (SLI families), the new tracing and log-schema modules
  in `apps/control-plane/src/` and `apps/worker/src/`,
  `python/workers/worker_template/` and `python/workers/tests/`, and the
  Collector gateway service in `deploy/compose/`.
- **Wave 1, Agent B** owns: the new checked-in ops tree (dashboards,
  recording rules, alert rule files, metrics-store record), the
  command-submission route files in `apps/control-plane/src/`,
  `crates/adapters/nats/src/topology.rs` and `consumer.rs` (hardening), the
  command consumer under `apps/worker/src/`, and the rate-limit middleware
  in `apps/control-plane/src/`.
- The two wave-1 ownership sets intersect only at composition roots
  (`apps/control-plane/src/main.rs`, `state.rs`; `apps/worker/src/main.rs`)
  and at the NATS adapter's publish/consumer pair (A's P11.05 header
  propagation, B's P11.13 hardening). Protocol: the Agent A change lands in
  its batch first; Agent B re-reads before editing and records the
  dependence in its decision record. No other shared files exist.
- **Wave 2, Agent A** owns `crates/flexcore-audit/src/`,
  `crates/edge-agent/src/audit_proof.rs`,
  `crates/adapters/postgres/src/` (audit, unit_of_work, store, new
  migration), the migration policy document, and the secrets config in
  `deploy/compose/`.
- **Wave 2, Agent B** owns `apps/control-plane/src/metrics.rs` (stuck-state
  family — sequential after wave 1, so no concurrency),
  `apps/control-plane/src/probes.rs`, `apps/worker/src/probes.rs`, and the
  close records. No file overlap with Agent A in wave 2.

## 4. Decision points (registered OPEN — agents never close E-decisions)

### P11-E-1 — Audit-chain crypto provider

- **Question:** which construction replaces the test-grade FNV-1a 64 chain
  (`crates/edge-agent/src/audit_proof.rs`, LR-203) in production?
- **Options:** (a) keyed HMAC-SHA-256 linear chain with per-tenant chain
  namespace and periodic anchor export; (b) SHA-256 Merkle tree with
  periodically anchored roots; (c) external timestamping / notary service.
- **DEFAULT (RECOMMENDATION):** (a) — the smallest upgrade from the existing
  linear chain shape, no new service dependency, per-tenant verification
  natural.
- **What closes it:** the human owner's sign-off on the provider and its
  key-management plan. P11.15 implements the DEFAULT per conventions §4 and
  records the dependence; production use remains a DEPLOYMENT REQUIREMENT
  gate, never a code-only claim.

### P11-E-2 — Long-term metrics store

- **Question:** where do metrics live beyond the Prometheus scrape window?
- **Options:** (a) VictoriaMetrics single-node alongside the existing
  Compose shape; (b) a second Prometheus instance with `remote_write` to it;
  (c) a Mimir / Thanos-class sharded store (explicitly premature here).
- **DEFAULT (RECOMMENDATION):** (a) VictoriaMetrics single-node.
- **What closes it:** operator acceptance of the storage footprint and
  retention design. VictoriaMetrics/Mimir vendor benchmarks are on the gap
  analysis UNVERIFIED list (§11) — the record carries no performance claims.

### P11-E-3 — Alert routing channel

- **Question:** how do page-class and ticket-class alerts reach a human?
- **Options:** (a) generic operator-configured webhook receiver with a
  documented receiver contract; (b) a NATS-internal alert topic consumed by
  ops tooling; (c) none in-phase — alerts surface as metrics and dashboard
  panels only.
- **DEFAULT (RECOMMENDATION):** (a) generic webhook, explicitly unbranded —
  no PagerDuty or Slack conformance claims of any kind.
- **What closes it:** the human operator runbook (who is paged, where the
  receiver lives). "Who is on call" is an operator answer, not a platform
  one.

## 5. Risks

| # | Risk | Treatment |
|---|---|---|
| 1 | The 0–10 base is WRITTEN-UNRUN: instrumentation is written against code that has never executed, and the first build may surface recorded defects (F-P10.07-5 and others) | Every count/render statement in records describes written code, never observed behavior; the first human build is the program gate (program README boundary 1) |
| 2 | OTel Rust 0.x churn: traces Beta vs metrics/logs Stable [S-63]; crate families drift against each other | Versions pinned at implementation; ASSUMPTION that the pinned set compiles together — verified only at the first build; the sampling/config seam (P11.06) isolates the churn |
| 3 | Alert fatigue without baseline data: burn-rate windows on services with no traffic history produce noise or silence | Thresholds registered before any run (X-8 pre-registration); low-traffic services get synthetic traffic per [S-61]; all targets are RECOMMENDATION until an operator tunes them on real data |
| 4 | Rate limiting vs edge-agent uplink interplay: a bucket sized for interactive operators can starve the fleet uplink that shares the same edge | P11.14 defines principal classes (operator, edge-agent uplink, plugin) before thresholds; the edge-agent class gets its own explicit budget |
| 5 | Compose-shape only: rules, dashboards, and Collector config are validated structurally, never executed in this program | A live Prometheus / Grafana / Collector integration check is a human gate input, recorded UNRUN |

## 6. Acceptance criteria (honest)

1. Every subtask has a decision record under `records/` (created at
   implementation time) citing the actual files touched and the [S-n]
   sources for every external claim.
2. Alert rule files, dashboard JSONs, recording rules, the gitleaks-class
   config, and the Collector config are checked in — written, never executed
   in this program.
3. Test files called for by P11.08 (redaction) and P11.12
   (duplicate-delivery) are complete and UNRUN; their execution is a human
   gate.
4. `python -I tools/check_dependency_rule.py` stays green after every
   implementation batch; `python tools/verify_docs.py` and
   `python tools/validate_agent.py` report no failures.
5. No run, build, scan, or conformance claim appears anywhere; SLO targets
   carry the RECOMMENDATION label pending operator sign-off; P11-E-1, P11-E-2,
   and P11-E-3 remain OPEN with their DEFAULTs recorded.

## 7. Execution notes for implementers

1. Decision records go under `phase-11-operational-robustness/records/`,
   created at implementation time (conventions §2.4); file naming follows
   the 0–10 record conventions (`NN_slug.md`, numbered in batch order).
2. Alert thresholds, stuck-state limits, and SLO targets are registered
   (written down) BEFORE any run that could fill them (X-8 pre-registration,
   conventions §5); observed values then live structurally separate from the
   registered thresholds.
3. This phase is expected to add no new workspace crate: the OTel SDKs land
   inside existing adapter/app crates (`apps/control-plane`,
   `apps/worker`), the Collector and store are deployment shapes in
   `deploy/compose/`, and the Python SDK lands in `python/workers/`. If an
   implementer finds a new crate justified, it follows conventions §6.2 with
   the additive `check_dependency_rule.py` update that requires.
4. New checked-in artifacts (rule files, dashboard JSONs, gitleaks config,
   Collector config) are static text validated by inspection and by the
   permitted tools only; no tool download executes them (conventions §1).
5. Every batch ends with the three permitted checks quoted verbatim in the
   batch's decision record (conventions §1.3, §6.1) — no "verified" without
   output (AGENTS.md §3 rule 7).
