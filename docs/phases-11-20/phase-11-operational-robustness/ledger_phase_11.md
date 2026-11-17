# Phase 11 — Operational Robustness and Observability — Ledger

Date: 2026-11-14. Status: **IMPLEMENTED — awaiting human build/test** — 22 of 22 subtasks ticked. Phase close: HANDOVER.md, CHECKLIST.md, records/53_p11_22_phase_close.md. The first human `cargo build --workspace` remains the program gate.

**Gate.** No implementation of this phase starts before (a) human review of
this roadmap and (b) the first human build of the Phase 0–10 workspace is
green (program conventions §3; program README, hard status boundary 1).
Every run, scan, sign-off, threshold, and date produced during
implementation is a HUMAN GATE INPUT recorded UNRUN / EMPTY / UNSIGNED.

**Tick convention (carried verbatim from Phases 0–10).** A checkbox is
ticked only as "implemented, awaiting human build/test" with a
decision-record citation — `- [x] ... (decision record:
\`records/NN_slug.md\`)` — never as "tested", "verified", "passed", or
"works". Tick flips use the exact-match count==1 method or Write/Edit;
decision records are created at implementation time under `records/`.

## Subtask register

- [x] **P11.01 — Golden-signal metric taxonomy and instrumentation plan.** (decision record: `records/01_p11_01_metric_taxonomy.md`)
      Latency / traffic / errors / saturation per user-facing service plus
      the command-lifecycle SLIs (accept latency, terminal-state ratio,
      stuck-state count), with every new metric family and its cardinality
      decision named before code changes. Evidence: the decision record
      cites the taxonomy table, the families added to
      `apps/control-plane/src/metrics.rs`, and their recorded cardinality
      decisions.
- [x] **P11.02 — SLO register and error-budget policy.** The register keyed (decision record: `records/02_p11_02_slo_register.md`)
      to the P11.01 taxonomy with targets labeled RECOMMENDATION pending
      operator sign-off, and the error-budget policy stating what happens at
      exhaustion; low-traffic services note the synthetic-traffic need
      [S-61]. Evidence: the record cites the register, the RECOMMENDATION
      labels, and the policy text; no target claims operator approval.
- [x] **P11.03 — Multi-window multi-burn-rate Prometheus alert rules.** (decision record: `records/21_p11_09_dashboards_as_code.md`)
      Checked-in rule files implementing the 14.4x / 1h+5m page, 6x / 6h+30m
      page, and 1x / 3d ticket pattern [S-61], bound to the P11.02 SLOs and
      the P11.20 stuck-state metric. Evidence: the record cites the rule
      file paths and the burn-rate windows encoded; the files are written,
      never evaluated by a live Prometheus.
- [x] **P11.04 — OTel tracing in the control plane.** OTLP export via (decision record: `records/04_p11_04_otel_tracing_rust.md`)
      `opentelemetry-otlp` with pinned crate versions (metrics + logs
      Stable, traces Beta [S-63]) and a span taxonomy over the 14-state
      lifecycle, one span per gate transition. Evidence: the record cites
      the pinned versions, the exporter wiring, and the span-to-state
      mapping.
- [x] **P11.05 — OTel in the Python workers.** Stable Python SDK [S-63]; (decision record: `records/05_p11_05_otel_python_and_trace_headers.md`)
      `trace_id` propagated through JetStream message headers from publisher
      to consumer so a request is followable end-to-end. Evidence: the
      record cites the SDK pins, the header-propagation seam, and the
      control-plane-to-worker trace path as written.
- [x] **P11.06 — OTel Collector gateway deployment config.** A single (decision record: `records/06_p11_06_collector_gateway.md`)
      gateway container shape in Compose [S-64] and a sampling policy record
      (head/parent-based first, tail sampling deferred with its trigger
      recorded). Evidence: the record cites the Compose service addition and
      the sampling policy decision.
- [x] **P11.07 — Structured-log schema v1.** Log schema v1 mapped to the (decision record: `records/07_p11_07_log_schema_v1.md`)
      OTel logs data model [S-62] with mandatory `tenant_id` / `command_id`
      / `trace_id` attributes and a `SeverityNumber` mapping for `tracing` /
      `structlog` levels. Evidence: the record cites the schema field table,
      the mandatory-attribute list, and the severity mapping table.
- [x] **P11.08 — PII/secret redaction at emit.** Redaction applied at emit (decision record: `records/08_p11_08_redaction.md`)
      time, especially in the Python workers, with redaction unit tests
      written complete and UNRUN. Evidence: the record cites the redaction
      module paths and the written-unrun test files; no claim that redaction
      was exercised against real output.
- [x] **P11.09 — Dashboard-as-code.** Grafana provisioning JSONs and (decision record: `records/21_p11_09_dashboards_as_code.md`)
      recording rules checked into git, rendering only P11.01 families — no
      panel invents a metric. Evidence: the record cites the checked-in
      dashboard and rule file paths and the family-to-panel mapping.
- [x] **P11.10 — Long-term metrics store selection record.** VictoriaMetrics (decision record: `records/22_p11_10_metrics_store.md`)
      single-node vs second Prometheus with `remote_write`; DEFAULT:
      VictoriaMetrics single-node, registered as P11-E-2. Evidence: the
      record cites the options, the DEFAULT, the UNVERIFIED status of vendor
      benchmarks, and what the operator must accept to close the decision.
- [x] **P11.11 — Idempotency-Key enforcement on command-submission POSTs.** (decision record: `records/23_p11_11_command_post_idempotency.md`)
      API contract addition: key required, stored-response replay semantics,
      documented 422 (missing/invalid key) and 409 (replay conflict)
      behavior. Evidence: the record cites the route files changed, the
      status-code semantics table, and the replay behavior as written.
- [x] **P11.12 — Consumer idempotency keyed on command_id.** The lifecycle (decision record: `records/24_p11_12_command_consumer_idempotency.md`)
      state machine as the dedupe oracle: an invalid-state transition on a
      redelivered command is a no-op plus a duplicate-delivery metric;
      duplicate-delivery test suite written complete and UNRUN. Evidence:
      the record cites the state-machine-oracle rule, the no-op + metric
      behavior, and the written-unrun test files.
- [x] **P11.13 — JetStream hardening.** MaxDeliver limits aligned between (decision record: `records/25_p11_13_jetstream_hardening.md`)
      helper and provisioned policy, dedicated DLQ streams per subject
      family, DLQ-depth alerts, bounded retention, `schema_version` headers
      on published messages [S-65]. Evidence: the record cites the topology
      changes, the alignment rule, the DLQ naming, and the retention bounds.
- [x] **P11.14 — Per-tenant token-bucket rate limiting and in-flight caps.** (decision record: `records/26_p11_14_rate_limiting.md`)
      Token bucket at the Axum edge, the 429 / `Retry-After` contract,
      per-tenant in-flight command caps [S-85]; in-process now with the
      Redis-backed-when-multi-instance path recorded; the edge-agent uplink
      is an explicit principal class. Evidence: the record cites the
      middleware, the principal classes, the contract, and the recorded
      scale-out note.
- [x] **P11.15 — Production-grade audit chain upgrade.** Real cryptography (decision record: `records/31_p11_15_audit_chain_upgrade.md`)
      per P11-E-1 (SHA-256 / HMAC or better) as a DEPLOYMENT REQUIREMENT, an
      INSERT-only DB role on audit tables, per-tenant chain verification
      [S-88]; the FNV-1a chain stays labeled test-grade (LR-203). Evidence:
      the record cites the P11-E-1 dependence, the construction, the role
      grants, the verification path, and the DEPLOYMENT REQUIREMENT label.
- [x] **P11.16 — Secrets management.** Vault DEFAULT recommendation (dynamic (decision record: `records/32_p11_16_secrets_management.md`)
      Postgres credentials TTL 1h, NATS creds, short-lived PKI later
      [S-74][S-75]), the Compose file-per-host restricted-permissions floor,
      and the SQLx pool credential-refresh design as written. Evidence: the
      record cites the recommendation, the Compose floor, and the refresh
      design; nothing is deployed.
- [x] **P11.17 — Secret scanning, pre-commit, and CI config.** gitleaks-class (decision record: `records/33_p11_17_secret_scanning.md`)
      scanning config, pre-commit hook config, and CI job config checked in
      and never run here. Evidence: the record cites the checked-in config
      files and states that no scan was run and no findings exist.
- [x] **P11.18 — Expand/contract migration policy and lock-risk audit.** The (decision record: `records/34_p11_18_migration_policy_lock_audit.md`)
      policy document [S-71] plus an audit of migrations 0002–0018 for lock
      risk against lifecycle tables (0008_audit_records, 0016_commands_p4_10,
      0017_feasibility_checks_p4_30). Evidence: the record cites the policy
      text and the per-migration findings with their remediation scheduling.
- [x] **P11.19 — RLS and connection-pooling reconciliation.** (decision record: `records/35_p11_19_rls_pooling_reconciliation.md`)
      Transaction-scoped `SET LOCAL` tenant context verified everywhere, all
      SQLx session-level `SET` usage audited, and the PgBouncer
      transaction-mode compatibility plan written, not deployed [S-69][S-70].
      Evidence: the record cites the audit findings, the verified pattern,
      and the plan as a written artifact.
- [x] **P11.20 — Stuck-state detection.** Commands parked in non-terminal (decision record: `records/51_p11_20_stuck_state.md`)
      states (FAILED and EXPIRED included — they exit only via FALLBACK)
      beyond a registered threshold surface as a gauge and an alarm feeding
      the P11.03 rule files. Evidence: the record cites the gauge and alarm
      definitions, the pre-registered threshold, and the rule-file binding.
- [x] **P11.21 — Health-check expansion.** Readiness/liveness semantics per (decision record: `records/52_p11_21_health_checks.md`)
      Compose service and dependency health (Postgres / NATS reachability)
      surfaced in the `/health` / `/ready` pair. Evidence: the record cites
      the per-service semantics table and the dependency checks wired into
      the existing probe modules.
- [x] **P11.22 — Decision records and phase close.** P11-E-1 (audit crypto (decision record: `records/53_p11_22_phase_close.md`)
      provider), P11-E-2 (metrics store), and P11-E-3 (alert routing
      channel — no PagerDuty/Slack conformance claims) recorded OPEN with
      DEFAULTs, plus a consistency audit across the phase's records.
      Evidence: the record cites the three E-decision entries and the
      consistency-audit result; no E-decision is closed by an agent.
