# Phase 20 — Deployment Architecture and Scale Readiness — Plan

Date: 2026-11-14. Status: **NOT STARTED** (0/16). Planning baseline only.
Ledger: `ledger_phase_20.md` (all boxes unticked). Conventions:
`../01_PROGRAM_CONVENTIONS.md` (binding). Context: `context.md` (seams and
constraints).

## 1. Scope statement

Phase 20 implements the gap analysis' Tier 3 verdicts (§5, Track D) as
decision-ready deployment records plus written-unrun measurement tooling:
the Kubernetes migration trigger register and decision record (DEFAULT: stay
on Compose [S-72][S-73]), the Helm chart and GitOps designs that become
usable only when a trigger fires, the capacity model framework that turns
MEASURED figures into published capacity numbers and derived SLOs [S-61],
the k6/Locust load-test ladder over the existing harness seams, the latency
budget allocation, the Postgres HA and PgBouncer designs consuming the
Phase 12 records, the multi-region DR design with its second-region trigger,
the TimescaleDB evaluation record against the current partitioning [S-66],
control-plane progressive delivery with its three-way scope distinction, the
backups-at-scale and environment-parity layers, release-engineering runbook
v2, and observability-at-scale topology.

Three deliverable depths exist, stated up front so no record overclaims.
**Design-and-record depth** (most subtasks): trigger registers, topology
designs, evaluation records, matrices, and runbooks — written, nothing
deployed. **Written-unrun tooling depth** (P20.02, and the values schemas
of P20.05): load-test scenario scripts and config structures, complete and
never executed. **Record-only depth** (P20.16): the decision/close records,
because the honest deliverable there is the argued decision.

The phase is TRIGGER-GATED by design (context constraint 1): the
Tier 3 research verdict is that Compose is correct for this single-host
shape until multi-host scheduling, a second region/DR site, automated
canary demand, or team growth fires [S-72][S-73] — so several subtasks
produce the decision machinery, not the migration. The honest center of
gravity is stated once and repeated in the acceptance criteria: **no
deployment exists to measure** (`deploy/compose/` has never been brought
up by this program; the CI record's 24 jobs are UNRUN; every benchmark
cell is EMPTY), so the capacity work is a framework with EMPTY cells until
the human/lab run fills them (X-8).

The expected artifact inventory, so the acceptance criteria in §8 are
checkable: a capacity model framework with EMPTY measurement cells and a
cardinality budget (P20.01); k6 + Locust scenario scripts and a
production-shaped synthetic-data policy (P20.02); a per-hop latency budget
table (P20.03); the trigger register + P20-E-1 decision record (P20.04);
Helm values schemas and a chart layout, written not rendered (P20.05); a
GitOps recommendation and promotion flow (P20.06); the HA topology design
consuming P12.05 (P20.07); the PgBouncer production config design consuming
P12.06/P11.19 (P20.08); the multi-region DR design (P20.09); the
TimescaleDB evaluation record (P20.10); the control-plane progressive
delivery design with the scope-distinction note (P20.11); the backups-at-
scale record (P20.12); the environment matrix (P20.13); release-engineering
runbook v2 (P20.14); the observability-at-scale topology (P20.15); the
decision/close records (P20.16). Implementation-time `records/` files are
created per batch under the phase directory, never during planning
(conventions §8).

## 2. Subtask outline

| ID | Title | Summary | Key seams / sources |
|---|---|---|---|
| P20.01 | Capacity model framework | Measurement → published capacity number ("N commands/s at p99 < X ms") → SLO derivation, in that order, never reversed [S-61]; cardinality budget for metrics (label-combination ceiling per metric class); the measurement/result cells stay EMPTY until the human run fills them (X-8) — the single-client honesty of `load_run.rs` ("never a deployment capacity claim") is the boundary this framework extends, honestly | [S-61]; `apps/sim-runner/src/load_run.rs` §1 scope notes; `apps/sim-runner/src/benchmark.rs` `MeasuredField`; P11 golden-signal/SLO records (consumed, not redefined) |
| P20.02 | Load-test plan (k6 + Locust, written-unrun) | k6 scenarios for the Axum API ladder (baseline → ramp → stress → soak, targeting `/ready`, the dispatch accept path, and the SSE stream) + Locust scenarios for the Python-worker dispatch fan-out; production-shaped synthetic data policy (volumes, tenant spread, telemetry cadence derived from the P16/P14 shapes — synthetic only, simulation-only ceiling); scripts complete and NEVER executed | gap analysis §5 (Tier 0/1 rows); [S-61]; `apps/sim-runner/src/load_run.rs` (the concurrent-client layer this plan adds); `deploy/compose/smoke.sh` (the shape it generalizes); conventions §1 |
| P20.03 | Latency budget allocation | Accept-path vs terminal-state budgets as a per-hop budget table (DESIGN): the async dispatch honesty rule — the accept SLA (request → `VALIDATED`/queued acknowledgement) is NOT an execution SLA (→ terminal state), and no budget row may conflate them; measured columns EMPTY (X-8) until P20.01/P20.02 produce figures | `crates/flexcore-domain` command lifecycle (the 14 states); P20.01 framework; P20.02 scenario outputs (EMPTY) |
| P20.04 | Kubernetes migration trigger register + decision record | The single register of K8s triggers — multi-host scheduling, second region/DR site, automated canary demand, team growth — each with its observable condition; decision record P20-E-1 (DEFAULT: stay on Compose until a trigger fires [S-72]); the register is what makes the future migration a decision, not a drift | [S-72][S-73]; gap analysis §5 Tier 3 row 1, §8 revisit row; `context.md` §1 |
| P20.05 | Helm chart design (written, not rendered) | Chart layout for control plane / worker / console / postgres / nats: values schema, per-environment overlays, stateful pieces flagged (postgres and JetStream file store as StatefulSet/operator concerns — CloudNativePG-class); HPA/KEDA named as post-migration mechanics; the chart is WRITTEN SPEC, never rendered (`helm template` is banned here); P10.21 linkage recorded (the chart is a DESCRIBED publication deliverable that does not exist — P20.05 designs content, P10.21's publication gate stays human) | [S-73]; `docs/operations/deployment-upgrade-model.md` §1 (the P10.21-pending reference); `deploy/compose/docker-compose.yml` (the service set it packages) |
| P20.06 | GitOps design | Argo CD vs Flux recommendation (DEFAULT: Argo CD for UI-driven onboarding [S-73]); environment promotion flow (dev → staging → prod, PR-based, digest-pinned); the design is inert until the P20.04 trigger fires — recorded as such | [S-73]; P20.04 register; P20.05 chart layout |
| P20.07 | Postgres HA topology design (consumes P12.05) | Patroni layout, `synchronous_commit` per table class (the lifecycle tables vs telemetry split), failover drill plan — built by CITATION on the P12.05/P12-E-1 design, adding only the scale layer (witness/arbiter placement, drill cadence); TRIGGER: the first availability contract — never a default [S-67]; nothing deploys on single-host Compose | [S-67]; `docs/phases-11-20/phase-12-backup-dr-resilience/plan.md` P12.05/P12-E-1; `crates/adapters/postgres/migrations/0016_commands_p4_10.up.sql`, `0008` audit tables (the lifecycle table classes) |
| P20.08 | PgBouncer production config design (consumes P12.06/P11.19) | Transaction-mode pooling config design: the `SET LOCAL` RLS contract consumed from P11.19 (cited, never restated), pool sizing per service, prepared-statement compatibility validation RESULTS gated on a REAL deployment (transaction mode + SQLx prepared statements is exactly the class of behavior that must be measured on the deployed pair, not asserted) | [S-69][S-70]; P12.06; P11.19; `crates/adapters/postgres/src/unit_of_work.rs` (the transaction-scope seam) |
| P20.09 | Multi-region DR design | Active/passive topology: Postgres + JetStream replication across regions, DNS failover with honest TTL/health-check bounds, RPO/RTO framing consumed from the P12 registers (LR-205), failover/failed-back runbook; data-residency constraints consumed from P13.14's jurisdiction registry (per-tenant residency as data); TRIGGER: a second-region requirement — the P12-E-3 revisit condition, linked not restated | `docs/phases-11-20/phase-12-backup-dr-resilience/plan.md` P12-E-3; P13.14 (jurisdiction registry); [S-65] JetStream mirror; `docs/operations/deployment-upgrade-model.md` §2 zone model |
| P20.10 | TimescaleDB evaluation record | Adoption criteria recorded BEFORE any evaluation run (compression need demonstrated on partition volumes, continuous-aggregates demand from named consumers), migration path from the current 0010 partitioning (what moves, what stays, rollback), trigger: telemetry volume evidence [S-66]; the evaluation stays EMPTY until evidence exists | [S-66]; `crates/adapters/postgres/migrations/0010_telemetry_storage_p2_06.up.sql` (D-P1..D-P4); P2.20 maintenance job seam |
| P20.11 | Blue-green/canary deployment design (control plane) | Progressive delivery plan for the CONTROL PLANE SOFTWARE: K8s-era shape (Argo Rollouts-class, metric-gated canary analysis) and Compose-era shape (two-stack switch with `/ready`-gated traffic move and drain via the P1.22 readiness flip); the explicit scope-distinction note: distinct from P18's device-fleet cohort rollouts and from the kernel plugin policy (cohort/percentage-free BY CONSTRUCTION) — three mechanisms, three arguments | [S-73]; `apps/control-plane/src/probes.rs` (`begin_shutdown` drain gate); P18.06 (device cohorts, cited as distinct); `crates/flexcore-kernel/src/revocation.rs` P7.38; `context.md` constraint 3 |
| P20.12 | Backups at scale | Retention tiers per data class, restore-time objectives for large partitioned tables (`telemetry` monthly partitions: restore-into-isolation timing, partition-level restore options), backup verification cadence — the SCALE layer only; the P12 backup/restore DESIGN is consumed by citation (P12.01/P12.02, LR-204/LR-205), never restated | P12.01–P12.03; `crates/adapters/postgres/migrations/0010_telemetry_storage_p2_06.up.sql`; LR-204/LR-205 |
| P20.13 | Environment matrix | Dev/staging/prod parity table: what differs per environment (recorded, deliberate) vs what must stay identical; config management shape; secret injection per P11.16 (Vault DEFAULT, Compose file-per-host floor — consumed, not reinvented); the P10.24 mode table consumed as the base | P11.16; `docs/operations/deployment-upgrade-model.md` §1 (mode table); `deploy/compose/docker-compose.yml` (the dev environment it defines) |
| P20.14 | Release engineering runbook v2 | RC → release pipeline as an operational procedure: artifact promotion (digest-pinned, no per-env rebuilds), the P10.18 gate sequence consumed as the pipeline's stages, rollback drill linkage — the P10.12 upgrade/rollback TEMPLATE becomes the operational procedure's drill section (the template's EMPTY run record stays the human gate input) | `ledger/phase-10-v1-release-hardening/18_ci_gate_run_record.md` (24 UNRUN jobs); `ledger/phase-10-v1-release-hardening/12_upgrade_rollback_rc.md`; P10.28 (prepared-NOT-EXECUTED publication) |
| P20.15 | Observability at scale | Collector tiering when >1 host (agent → gateway pattern [S-64]); long-term store topology consuming P11.10/P11-E-2 (the single-node DEFAULT and the scale conditions that would change it); cost-control notes (cardinality budget consumption from P20.01, retention vs storage) | [S-61][S-64]; P11.10/P11-E-2 (cited); P20.01 cardinality budget |
| P20.16 | Decision records + close | P20-E-1 (K8s trigger register close-out conditions), P20-E-2 (multi-region necessity), P20-E-3 (long-term metrics topology at scale, with P11-E-2 linkage); consistency audit across every phase-20 artifact (including the trigger-gating honesty and the three-way rollout distinction); program-level note: P20 close completes the 11–20 planning set | conventions §4, §2; `context.md` constraints 1/3 |

## 3. Blocking order and shared surfaces

| Producer (blocks) | Consumers (what they take, never redefine) |
|---|---|
| P20.01 capacity framework + cardinality budget | P20.03 budget table (measured columns), P20.02 scenario pass/fail shapes, P20.15 cost-control notes |
| P20.04 trigger register + P20-E-1 DEFAULT | P20.05 chart design, P20.06 GitOps design, P20.11 K8s-era shape (all inert until a trigger fires) |
| P20.05 chart layout | P20.06 promotion flow (artifacts it promotes), P20.11 K8s-era rollout shape |
| P20.02 load-test plan | P20.01 measurement rows (the runs that fill them), P20.03 measured columns (EMPTY until then) |
| P12.05/P12-E-1 + P12.06 (external, consumed) | P20.07 HA topology, P20.08 PgBouncer config |
| P12-E-3 (external, consumed) + P13.14 residency | P20.09 multi-region design |
| P20.11 scope-distinction note | P20.16 consistency audit |
| P10.18/P10.12 records (external, consumed) | P20.14 runbook v2 |

## 4. Deliberate non-goals (this phase does not)

- No Kubernetes cluster is provisioned, no chart is rendered, no GitOps
  operator is installed — the P20.04 DEFAULT keeps Compose until a trigger
  fires [S-72], and the designs are inert until then.
- No load test is executed and no capacity number is published — P20.01's
  cells and P20.02's scripts are written-unrun (X-8); the first human/lab
  run is the gate that fills them.
- No Patroni standby, no PgBouncer, no second region, and no TimescaleDB
  instance is deployed; P20.07/P20.08/P20.09/P20.10 are design records
  gated on their named triggers.
- No Kafka migration, no SPIFFE/SPIRE deployment — the gap analysis §5
  Tier 3 verdicts stand (JetStream suffices [S-65]; workload identity
  deferred to multi-host [S-76]); a fired multi-host trigger would make
  SPIFFE designable, not deployed.
- No device is updated and no plugin rollout policy changes: P20.11's
  progressive delivery is the control plane only; P18's device cohorts and
  the kernel's cohort/percentage-free policy stay exactly as argued
  (context constraint 3).
- No dispatch-authority change: the never-granted permission set stays
  byte-identical; nothing in a deployment topology bypasses the command
  lifecycle, authorization gates, tenant isolation, or audit.
- No release-ladder movement: everything stays simulation-only at this
  stage; no deployment, site, or scale claim is made.

## 5. Wave plan

Two agents, per the standing loop discipline.

| Wave | Agent A | Agent B | Shared note |
|---|---|---|---|
| 1 | P20.01–P20.08 (capacity + infrastructure: capacity model, load-test plan, latency budgets, trigger register, Helm design, GitOps design, HA topology, PgBouncer) | P20.09–P20.15 (DR + release engineering: multi-region DR, TimescaleDB evaluation, progressive delivery, backups at scale, environment matrix, runbook v2, observability at scale) | Disjoint seams: Agent A owns the measurement/capacity and K8s-machinery surfaces, Agent B the survivability/release surfaces; the P20.01 cardinality budget is defined once by Agent A and consumed by Agent B's P20.15; Agent B's P20.11 consumes Agent A's P20.04/P20.05 shapes by citation |
| 2 | P20.16 (decision records + consistency audit + close) | (none) | The consistency audit covers every phase-20 artifact: trigger-gating honesty, EMPTY-cell discipline, the three-way rollout distinction, and the consumed-not-restated P11/P12/P13 records |

### 5.1 Batch discipline during implementation

- Every implemented batch closes with a decision record under `records/`
  citing the subtask IDs, the seams touched, and the honest status
  ("implemented, awaiting human build/test") — conventions §2.4.
- Trigger-gated subtasks are recorded with their trigger UNFIRED unless the
  owner has recorded otherwise; a fired trigger is a recorded event, never
  an agent inference.
- Thresholds named in designs (capacity targets, load levels, retention
  values, budget splits) are registered BEFORE any run that could fill
  them; dynamic results stay structurally separate (X-8; conventions §5).
- Phase close follows conventions §2.5: HANDOVER, CHECKLIST, and the
  close/exit-gate record are added under this directory only at
  implementation completion (P20.16) — never during planning. The close
  record states that P20 completes the 11–20 planning set.
- No new crates are expected in this phase; any design that would need one
  records it as a proposal with the additive `tools/check_dependency_rule.py`
  update required at implementation time (conventions §6.2), and the
  dependency rule stays green after every batch.

## 6. Decision points (E-decisions — agents never close them)

| ID | Question | Options | DEFAULT / RECOMMENDATION | What would close it | Closer |
|---|---|---|---|---|---|
| P20-E-1 | When does the Kubernetes migration actually happen, if ever? | (a) stay on Compose until a registered trigger fires (multi-host scheduling, second region/DR site, automated canary demand, team growth), then migrate; (b) migrate preemptively (rejected — the Tier 3 verdict is explicit that Compose is correct for this single-host shape [S-72]); (c) never migrate regardless of triggers (rejected — dishonest under trigger (a) conditions) | **DEFAULT: (a)** — the trigger register (P20.04) is the decision machinery; each trigger names its observable condition so the future migration is a decision, not a drift | The owner recording that a named trigger has fired (with the evidence), or recording that none will | human owner |
| P20-E-2 | Is a second region required, and if so active/passive or something else? | (a) no second region — DR stays the P12 posture (documented site-loss runbook + restore-into-isolation rehearsal) until a real requirement exists; (b) warm active/passive DR region (P20.09 topology); (c) active/active multi-region (rejected at this scale — consistency cost without a stated requirement) | **DEFAULT: (a)** — the P12-E-3 revisit condition; P20.09's design is decision-ready against the day the requirement lands, with the P13.14 residency constraints pre-consumed | The owner recording the second-region requirement (customer, regulator, or contract evidence) | human owner |
| P20-E-3 | Long-term metrics topology at scale: does the P11.10 single-node DEFAULT change? | (a) keep the P11.10 DEFAULT (single-node VictoriaMetrics-class) until cardinality/volume evidence says otherwise; (b) clustered/ha store topology (paired with the P20.04 multi-host trigger); (c) managed vendor service (a budget/vendor E-decision that belongs to the owner) | **DEFAULT: (a)** — extends P11-E-2 with the scale conditions recorded; the P20.15 topology record says exactly what evidence would move it (cardinality budget consumption, retention-vs-storage economics) | The owner recording volume/cost evidence exceeding the single-node envelope, or naming a vendor | human owner |

## 7. Risks

| # | Risk | Basis / mitigation |
|---|---|---|
| R-P20-1 | The triggers may never fire — most of this phase's machinery stays unused | Honest by design: the records are decision-ready, not commitments; the trigger register makes non-firing a recorded state, and the close record says plainly if nothing fired |
| R-P20-2 | Designs age — tool versions, chart APIs, and managed-service shapes drift between the record and the day a trigger fires | Every design carries a re-verify date rule: at trigger time, the design's version pins and assumptions are re-checked against primary sources BEFORE implementation, and stale rows are re-recorded (never silently edited) |
| R-P20-3 | No deployment exists to measure — the capacity framework and load-test plan have nothing real to measure | The framework is built to be filled: EMPTY cells with pre-registered thresholds (X-8), the single-client honesty boundary of `load_run.rs` carried forward, and no figure invented anywhere (program README boundary 1; gap analysis §3 row 12) |
| R-P20-4 | Charts and configs are unrendered — Helm values schemas and PgBouncer/HA configs have never passed a renderer, a planner, or a real connection pool | Written-unrun is the recorded status; P20.08 explicitly gates prepared-statement validation RESULTS on a real deployment; rendering/deploying is the human gate, never an agent claim |
| R-P20-5 | The 0–10 base is WRITTEN-UNRUN — the first build may surface recorded defects (e.g., F-P10.07-5), and the probes/harness seams P20 consumes are unverified behavior | Program-level standing risk (program README boundary 1); planning proceeds, implementation cannot precede a green human build; P20's load plans consume the seams as DESIGNED, honestly labeled |
| R-P20-6 | Honesty wording — any text implying a cluster, a deployment, a capacity figure, a rendered chart, or a completed load test exists is a compliance failure | Context constraints 2/6 + §8 acceptance criteria; every phase-20 record carries the written-unrun statement and the trigger status |

## 8. Acceptance criteria (honest)

1. All designs, registers, matrices, runbooks, evaluation records, config
   designs, and load-test scripts exist **written-unrun**; no subtask
   claims a run, a rendered chart, a provisioned cluster, a deployed
   standby, a load-test result, or a measured latency.
2. **No capacity number is claimed.** P20.01 publishes a framework whose
   measurement and result cells are EMPTY (X-8); no "N commands/s" figure
   exists anywhere until the human/lab run fills it, and SLOs are recorded
   as DERIVATION-PENDING on those cells — never asserted first [S-61].
3. **No chart is rendered and no cluster is provisioned**; the P20.04
   DEFAULT (stay on Compose) is recorded, the trigger register names every
   trigger with its observable condition, and the K8s-era designs are
   marked inert-until-trigger.
4. **No HA, pooling, multi-region, or TimescaleDB component is deployed**;
   P20.07/P20.08/P20.09/P20.10 cite the consumed P12/P13 records rather
   than restating them, and each names its trigger UNFIRED (unless the
   owner recorded otherwise).
5. **The three-way rollout distinction holds**: P20.11's progressive
   delivery is the control plane only; P18's device cohorts and the kernel
   plugin policy (cohort/percentage-free BY CONSTRUCTION) stay byte-
   identical and separately-argued (context constraint 3), stated in
   P20.11 and the P20.16 audit.
6. **The load-test ladder is complete and unexecuted**: k6 (API) and
   Locust (worker fan-out) scenarios written against the named endpoints
   with production-shaped SYNTHETIC data only; no threshold gates a CI
   pipeline on a duration (the P3.27 no-runtime-threshold discipline).
7. The release ladder, ADR-010, and the never-granted permission set are
   unchanged; no deployment topology in any record bypasses the command
   lifecycle, authorization, tenant isolation, or audit.
8. `python tools/verify_docs.py` and `python tools/validate_agent.py` are
   green after every documentation change in this phase (results quoted,
   never asserted), and the dependency rule stays green after every
   implementation batch.
