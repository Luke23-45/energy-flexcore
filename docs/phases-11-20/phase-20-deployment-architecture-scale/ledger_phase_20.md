# Phase 20 — Deployment Architecture and Scale Readiness — Ledger

Date: 2026-11-14. Status: **IMPLEMENTED — awaiting human build/test** — 16 of 16 subtasks ticked. Phase close: HANDOVER.md, CHECKLIST.md, records/42_phase_20_close_and_consistency_audit.md. Every trigger UNFIRED, every measurement cell EMPTY; the human-gate queue is in CHECKLIST.md. This close completes the 11–20 planning set.

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

- [x] **P20.01 — Capacity model framework.** Measurement → published (decision record: `records/01_p20_01_capacity_model_framework.md`)
      capacity number ("N commands/s at p99 < X ms") → SLO derivation, in
      that order [S-61]; cardinality budget for metrics; cells EMPTY until
      the human run fills them (X-8); the single-client honesty boundary
      of `load_run.rs` carried forward. Evidence: the record cites the
      framework; no capacity number is claimed.
- [x] **P20.02 — Load-test plan (k6 + Locust, written-unrun).** k6 (decision record: `records/02_p20_02_load_test_plan.md`)
      scenarios for the Axum API ladder (baseline → ramp → stress → soak)
      + Locust scenarios for the Python-worker dispatch fan-out;
      production-shaped synthetic-data policy; scripts never executed.
      Evidence: the record cites the scripts; no load test has run.
- [x] **P20.03 — Latency budget allocation.** Accept-path vs terminal-state (decision record: `records/03_p20_03_latency_budget_allocation.md`)
      budgets as a per-hop table (DESIGN): the async dispatch honesty rule
      — accept SLA is not execution SLA; measured columns EMPTY (X-8).
      Evidence: the record cites the budget table; no latency figure
      exists.
- [x] **P20.04 — Kubernetes migration trigger register + decision record.** (decision record: `records/04_p20_04_kubernetes_trigger_register.md`)
      The register of K8s triggers (multi-host scheduling, second
      region/DR site, automated canary demand, team growth) with
      observable conditions; P20-E-1 registered — DEFAULT: stay on
      Compose until a trigger fires [S-72]. Evidence: the record cites
      the register; no cluster is provisioned.
- [x] **P20.05 — Helm chart design (written, not rendered).** Chart layout (decision record: `records/05_p20_05_helm_chart_design.md`)
      for control plane / worker / console / postgres / nats: values
      schema, stateful pieces flagged (StatefulSet/operator,
      CloudNativePG-class); P10.21 publication linkage recorded; never
      rendered [S-73]. Evidence: the record cites the design; no chart is
      rendered or published.
- [x] **P20.06 — GitOps design.** Argo CD vs Flux recommendation (DEFAULT: (decision record: `records/06_p20_06_gitops_design.md`)
      Argo CD for UI-driven onboarding [S-73]); environment promotion
      flow; inert until the P20.04 trigger fires. Evidence: the record
      cites the recommendation; no operator is installed.
- [x] **P20.07 — Postgres HA topology design (consumes P12.05).** Patroni (decision record: `records/07_p20_07_postgres_ha_topology.md`)
      layout, `synchronous_commit` per table class, failover drill plan —
      by citation on P12.05/P12-E-1, scale layer only; TRIGGER: first
      availability contract, never a default [S-67]. Evidence: the record
      cites the consumed P12 design; no standby is deployed.
- [x] **P20.08 — PgBouncer production config design (consumes (decision record: `records/08_p20_08_pgbouncer_production_config.md`)
      P12.06/P11.19).** Transaction mode, the `SET LOCAL` RLS contract
      consumed from P11.19, pool sizing; prepared-statement compatibility
      validation RESULTS gated on a real deployment. Evidence: the record
      cites the config design; no pooler runs and no result is claimed.
- [x] **P20.09 — Multi-region DR design.** Active/passive topology: (decision record: `records/21_p20_09_multi_region_dr.md`)
      Postgres + JetStream replication across regions, DNS failover with
      honest TTL/health-check bounds, RPO/RTO from the P12 registers;
      residency constraints consumed from P13.14; TRIGGER: second-region
      requirement (P12-E-3 linkage). Evidence: the record cites the
      design and linkage; no second region is stood up.
- [x] **P20.10 — TimescaleDB evaluation record.** Adoption criteria (decision record: `records/22_p20_10_timescaledb_evaluation.md`)
      (compression need, continuous-aggregates demand) recorded BEFORE any
      evaluation run; migration path from the current 0010 partitioning;
      TRIGGER: telemetry volume evidence [S-66]. Evidence: the record
      cites the criteria; the evaluation stays EMPTY until evidence
      exists.
- [x] **P20.11 — Blue-green/canary deployment design (control plane).** (decision record: `records/23_p20_11_progressive_delivery.md`)
      Progressive delivery for the CONTROL PLANE SOFTWARE: Argo
      Rollouts-class when on K8s; Compose-era two-stack switch with
      `/ready`-gated traffic move and drain; explicit scope-distinction
      note vs P18 device cohorts and vs the kernel plugin policy
      (cohort/percentage-free BY CONSTRUCTION). Evidence: the record
      cites the design and the three-way distinction; no rollout runs.
- [x] **P20.12 — Backups at scale.** Retention tiers per data class, (decision record: `records/24_p20_12_backups_at_scale.md`)
      restore-time objectives for large partitioned tables (0010 monthly
      partitions), verification cadence — scale layer only; P12 backup
      DESIGN consumed by citation (LR-204/LR-205 honored). Evidence: the
      record cites the tiers; no backup or restore runs.
- [x] **P20.13 — Environment matrix.** Dev/staging/prod parity (what (decision record: `records/25_p20_13_environment_matrix.md`)
      differs is recorded and deliberate), config management, secret
      injection per P11.16 (Vault DEFAULT, Compose floor consumed); the
      P10.24 mode table as base. Evidence: the record cites the matrix;
      no environment is stood up or verified.
- [x] **P20.14 — Release engineering runbook v2.** RC → release pipeline as (decision record: `records/26_p20_14_release_runbook_v2.md`)
      an operational procedure: artifact promotion (digest-pinned), the
      P10.18 gate sequence consumed as pipeline stages, rollback drill
      linkage (the P10.12 template becomes the drill section; its run
      record stays the human gate input). Evidence: the record cites the
      runbook; nothing is built, signed, or released.
- [x] **P20.15 — Observability at scale.** Collector tiering when >1 host (decision record: `records/27_p20_15_observability_at_scale.md`)
      (agent → gateway [S-64]); long-term store topology consuming
      P11.10/P11-E-2; cost-control notes consuming the P20.01 cardinality
      budget. Evidence: the record cites the topology; no collector or
      store is deployed.
- [x] **P20.16 — Decision records + close.** P20-E-1 (K8s trigger), P20-E-2 (decision record: `records/42_phase_20_close_and_consistency_audit.md`; decision register: `records/41_p20_16_decision_register_e1_e2_e3.md`)
      (multi-region necessity), P20-E-3 (long-term metrics topology,
      P11-E-2 linkage); consistency audit across every phase-20 artifact;
      program-level note: P20 close completes the 11–20 planning set.
      Evidence: the records compile options, defaults, and closing
      evidence; agents close none.

## Decision points (all OPEN — human-owned)

| ID | Question | DEFAULT (label) | Closer |
|---|---|---|---|
| P20-E-1 | Kubernetes migration timing: stay on Compose until a registered trigger fires? | Stay on Compose until a trigger fires (multi-host scheduling, second region/DR site, automated canary demand, team growth — each with an observable condition in the P20.04 register) [S-72][S-73] | human owner |
| P20-E-2 | Is a second region required, and what topology? | No second region — DR stays the P12 posture (runbook + restore-into-isolation) until a real requirement is recorded; P20.09's active/passive design is decision-ready for that day (P12-E-3 linkage) | human owner |
| P20-E-3 | Long-term metrics topology at scale: does the P11.10 single-node DEFAULT change? | Keep the P11.10 DEFAULT until cardinality/volume evidence exceeds the single-node envelope; the P20.15 record states what evidence would move it | human owner |
