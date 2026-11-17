# Phase 20 — Deployment Architecture and Scale Readiness — Context

Date: 2026-11-14. Status: **NOT STARTED** (0/16 — see `ledger_phase_20.md`).
This directory is PLANNING ONLY: no code, crates, manifests, charts, or
clusters are created by this program's planning step (program conventions
§3). Citations `[S-n]` resolve in `../00_GAP_ANALYSIS.md` §11; the rules in
`../01_PROGRAM_CONVENTIONS.md` bind everything below.

## 1. Why this phase exists

Phase 20 is the gap analysis' Tier 3 — scale-stage architecture — the tier
the engineering research DELIBERATELY DEFERRED (`00_GAP_ANALYSIS.md` §5,
Track D). The research verdicts for exactly this stack shape (Rust/Axum +
Python workers + TS console + Postgres RLS + NATS JetStream + Compose) are
the phase's foundation, and each is carried into a specific subtask:

| Research verdict for this shape | Source(s) | Where it lands |
|---|---|---|
| Do NOT migrate to Kubernetes now — Compose is correct until triggers fire: multi-host scheduling, a second region/DR site, automated canary demand, team growth | [S-72][S-73]; gap analysis §5 Tier 3 row 1, §8 revisit row | P20.04 (trigger register + decision record P20-E-1) |
| Postgres HA (Patroni, synchronous mode for lifecycle tables) is justified by availability contracts, never a default | [S-67] | P20.07 (consumes the P12.05 design) |
| PgBouncer transaction-mode pooling with the transaction-scoped `SET LOCAL` RLS contract | [S-69][S-70] | P20.08 (consumes P12.06/P11.19) |
| TimescaleDB for domain time-series only on evidence — extend the existing partitioning first | [S-66] | P20.10 (evaluation record + trigger) |
| Kafka migration: no — the 14-state lifecycle is state-machine-shaped; JetStream suffices | [S-65]; gap analysis §5 Tier 3 row 4, §8 | Recorded as a standing non-goal; revisited only on the recorded trigger |
| SPIFFE/SPIRE workload identity: deferred to multi-host | [S-76] | Named non-goal; becomes designable only when the multi-host trigger (P20.04) fires |
| Capacity must be MEASURED, then SLOs derived from it — never the reverse | [S-61] | P20.01 (capacity model framework; EMPTY until measured, X-8) |
| Load-test ladder: baseline → ramp → stress → soak, with concurrent-client tools | gap analysis §5 (Tier 0/1 rows); [S-61] | P20.02 (k6 + Locust scenario plan) |

Phase 20 is last in the phase ordering for exactly one reason (gap analysis
§9): "deployment architecture last because its triggers are growth-
dependent." Several subtasks are therefore TRIGGER-GATED design records —
they become implementation work only when their named trigger fires, and a
trigger that never fires honestly leaves the design as a decision-ready
record, not a backlog promise (constraint 1).

## 2. What exists today (the real seams)

All paths below exist in the Phase 0–10 workspace and were re-read for this
context. The workspace is WRITTEN-UNRUN — written modules with records, not
verified behavior; **no deployment exists at all** (gap analysis §3 row 12:
"no deployment exists at all (Stage 9 chain unheld)").

| Seam | Path | What it honestly contains |
|---|---|---|
| Compose deployment | `deploy/compose/docker-compose.yml` | P1.25 local-developer stack: postgres 18.6 and nats 2.15.2 pinned EXACT tags (JetStream file store), control-plane (embedded migrations run at startup via `FlexCoreStore::run_migrations`) and worker built from the workspace; `/ready`-based healthchecks; DEV-ONLY credentials, marked; production secrets path is a secrets manager (P0.16/P1.21) |
| Dockerfiles (2) | `deploy/compose/control-plane.Dockerfile`, `deploy/compose/worker.Dockerfile` | The two build definitions the Compose services reference; no other image build path exists |
| Smoke script | `deploy/compose/smoke.sh` | Stage 1 exit-gate smoke: stack up → `/ready` wait → tenant/portfolio create → cross-tenant 404 assertion → teardown; requires dev-auth build + overlay for the authenticated path |
| Edge systemd unit | `deploy/edge/flexcore-edge-agent.service` | P6.20 unit definition — WRITTEN, NOT INSTALLED, NOT EXECUTED; hardened (no inbound listener, sandboxing); bounded resource profile recorded as configuration, never a measured claim |
| Health probes | `apps/control-plane/src/probes.rs` | P1.22: `/health` liveness vs `/ready` readiness DELIBERATELY distinct; `begin_shutdown` flips readiness first so routing stops while the process drains — the drain gate P20.11's switch design consumes |
| Load harness | `apps/sim-runner/src/load_run.rs` | P5.24: N ingest iterations / M sequential query round trips, one `LoadMeasurementRecord`; HONESTLY NOT: no browser measurement, ONE sequential client ("never a deployment capacity claim"), no fan-out or restart-recovery harness — `not_measured` with reason (fail-loud); result tables EMPTY |
| Benchmark harness | `apps/sim-runner/src/benchmark.rs` | P3.27: pinned-scenario replay timing + ×2/×4 scaling probes (raw numbers only, no scaling law); `MeasuredField` with method or `not_measured` + reason; hardware capture; no threshold gates on runtime anywhere; baseline-not-target label |
| CI gate record | `ledger/phase-10-v1-release-hardening/18_ci_gate_run_record.md` | P10.18: the §22.2 gate checklist mapped to the ACTUAL `.github/workflows/ci.yml` jobs — 23 enumerated jobs plus the additive `release-candidate-build` job = 24 jobs, ALL WRITTEN-UNRUN; the §5 run record is an EMPTY template (F-1: zero jobs have ever executed) |
| Benchmark set | `benchmarks/research-set-v1.json` | P8.19 frozen set — `status: WRITTEN-UNRUN`, every `run_status: NOT_RUN`; anti-silent-growth rule (a change is a NEW version file); the rerun record `ledger/phase-10-v1-release-hardening/09_benchmark_rerun.md` carries the X-8 tolerance registry as PENDING |
| Deployment/upgrade model | `docs/operations/deployment-upgrade-model.md` | P10.24 — DESIGN document, NOT a deployed system; five-zone model (P9.05); every site value `<PLACEHOLDER>`; upgrade model is the design P10.12's test matrix was written against, with P10.12's run record EMPTY; the Helm chart it references is the P10.21 publication deliverable that does not exist yet |
| Upgrade/rollback record | `ledger/phase-10-v1-release-hardening/12_upgrade_rollback_rc.md` | P10.12: upgrade path + rollback register + test matrix written; run record EMPTY — no update built, signed, staged, or applied; P20.14 links its template to an operational procedure, never fills it |
| Telemetry partitioning | `crates/adapters/postgres/migrations/0010_telemetry_storage_p2_06.up.sql` | `telemetry` PARTITION BY RANGE (event_time), monthly partitions pre-created by the P2.20 maintenance job (no auto-creation); `telemetry_quarantine`; the D-P1..D-P4 decisions — the partitioning P20.10's TimescaleDB evaluation extends, never silently replaces |

## 3. Constraints (critical)

1. **Trigger-gated scope.** Several subtasks (P20.04–P20.06 K8s/Helm/GitOps,
   P20.07 Patroni deployment, P20.09 multi-region, P20.10 TimescaleDB) are
   design records that become implementation only when their named trigger
   fires — each subtask names its trigger, and the trigger register
   (P20.04) is the single place the K8s triggers are enumerated. A trigger
   that has not fired leaves its subtask as a decision-ready design record;
   treating an unfired design as a commitment, or a fired trigger as
   self-executing authority, is both a consistency-audit failure.
2. **Everything is written-unrun.** No chart is rendered, no cluster is
   provisioned, no load test is executed, no capacity number is claimed, no
   Patroni standby is deployed, no region is stood up. Load-test scripts,
   Helm values schemas, and drill plans are written artifacts whose
   execution is a human gate; every threshold named in them is
   pre-registered BEFORE any run that could fill it, and dynamic results
   stay structurally separate (X-8 carryover; conventions §5).
3. **Three-way scope distinction for progressive delivery — restated in
   every record that touches rollout.** P20.11's blue-green/canary is for
   the CONTROL PLANE deployment: progressive delivery of the SOFTWARE
   across environments. P18's cohort rollouts (P18.06) are for the DEVICE
   FLEET: staged OTA of device firmware with abort thresholds. The kernel
   plugin staged rollout stays cohort/percentage-free BY CONSTRUCTION
   (`crates/flexcore-kernel/src/revocation.rs`, P7.38). All three are
   separately-argued mechanisms; none reinterprets, relaxes, or borrows the
   authority of another, and P20.11 carries the explicit scope-distinction
   note.
4. **Phase 12 owns the backup/DR/Patroni DESIGN — P20 consumes it (cite,
   don't restate) and adds the scale/multi-region layer.** P12.05/P12-E-1
   (Patroni topology + `synchronous_commit` for the command-lifecycle
   tables), P12.06 (PgBouncer introduction design), P12-E-2 (JetStream
   snapshot posture), and P12-E-3 (the DR-site trigger input: "no second
   site in the current posture — revisit at the Phase 20 multi-region
   trigger") are consumed by citation; restating them is a duplication
   finding, and P20's records must say which P12 record they extend.
5. **Benchmark cells stay EMPTY (X-8).** The capacity model's measurement
   rows (P20.01), the load-test result tables (P20.02), and the per-hop
   budget table's measured columns (P20.03) are EMPTY templates until the
   human/lab run fills them; a published capacity number ("N commands/s at
   p99 < X ms") exists only AFTER measurement — until then P20.01 is a
   framework, honestly labeled. The `research-set-v1.json` anti-silent-
   growth rule carries over to any load-scenario set this phase defines.
6. **The release ladder does not move.** Simulation/shadow ceiling,
   operator-approved control by default at v1.0.0 with operator approval,
   bounded automation post-1.0 — unchanged (program README boundary 4).
   Nothing in this phase deploys anything anywhere; "scale readiness" means
   decision-ready records and written-unrun scripts, never a standing
   deployment.

## 4. Glossary (deployment and scale terms used in this phase)

| Term | Meaning |
|---|---|
| Capacity model | The framework that turns measured figures into a published capacity statement ("N commands/s at p99 < X ms") under recorded conditions — measurement first, number after [S-61] |
| Latency knee | The load level where latency stops growing linearly and turns sharply — the empirical boundary a stress test looks for, never assumed |
| Soak test | A long-duration load run at steady level that surfaces leaks, queue growth, and slow degradation invisible in short runs |
| HPA / KEDA | Kubernetes Horizontal Pod Autoscaler / event-driven autoscaler — scaling mechanics only meaningful AFTER a Kubernetes migration (P20.04 trigger); named in the Helm design, not claimed as deployed |
| GitOps | Declarative, version-controlled desired state with an operator (Argo CD / Flux) reconciling clusters to it [S-73] |
| Helm chart / Kustomize overlay | Packaging (templated chart with values) vs patching (base + per-environment overlays) — the two K8s packaging options P20.05 designs for |
| Blue-green | Two complete stacks, one live; the switch moves traffic after health gating — the Compose-era progressive-delivery shape (P20.11) |
| Canary analysis | Automated comparison of a small new-version slice against the current version on health/error metrics before wider promotion — demands routing and metric plumbing that only makes sense at multi-instance scale (a P20.04 trigger) |
| Patroni | The PostgreSQL HA template using a distributed configuration store for leader election and failover [S-67] |
| PgBouncer transaction mode | Pooling where a server connection is assigned per transaction — incompatible with session-level `SET`; hence the transaction-scoped `SET LOCAL` RLS contract [S-69][S-70] |
| StatefulSet / operator (CloudNativePG-class) | The Kubernetes primitives for stateful workloads: stable identity + storage, and a controller encoding operational knowledge (failover, backups) — the stateful pieces P20.05 flags |
| Active/passive DR | A second site holding a warm replica that is promoted on failover, versus cold restore-into-isolation — the P20.09 topology choice P12-E-3 left open |
| DNS failover | Routing-layer failover by changing DNS answers to the passive site — bounded by TTL and health-check honesty, never instant |
| Cardinality budget | The recorded ceiling on metric label combinations so the metrics store stays affordable and queryable — a P20.01 output consumed by alerting and dashboards |
| Environment parity | Dev/staging/prod differing only in recorded, deliberate values — the P20.13 matrix property that makes test evidence transferable |
| Artifact promotion | Moving the SAME immutable artifact (digest-pinned) through environments rather than rebuilding per environment — the P20.14 runbook property |
