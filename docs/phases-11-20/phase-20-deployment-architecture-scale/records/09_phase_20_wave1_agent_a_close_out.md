<!-- Phase 20 wave 1, Agent A close-out: P20.01–P20.08 (capacity +
     infrastructure). Written-unrun everywhere; every trigger UNFIRED;
     every measurement/result/measured-count cell EMPTY (X-8). No ledger
     checkbox was touched (per the task's hard ban 6); no E-decision was
     closed. -->
# Phase 20 — Wave 1, Agent A Close-Out (P20.01–P20.08)

| Field | Value |
|---|---|
| Wave / agent | Phase 20 wave 1, Agent A |
| Subtasks | P20.01 capacity model framework; P20.02 load-test plan (k6 + Locust, written-unrun); P20.03 latency budget allocation; P20.04 K8s trigger register + P20-E-1; P20.05 Helm chart design; P20.06 GitOps design; P20.07 Postgres HA topology; P20.08 PgBouncer production config design |
| Status | **IMPLEMENTED (as design records and written-unrun tooling), AWAITING HUMAN REVIEW** — nothing executed, nothing rendered, nothing provisioned, no capacity number, no measured latency, every trigger UNFIRED |
| Ledger | `ledger_phase_20.md` NOT touched (hard ban 6: never tick a checkbox) — the ledger ticks remain the human gate's act |
| Shared surfaces | P20.01 §4 cardinality budget defined ONCE (Agent B's P20.15, `records/27_p20_15_observability_at_scale.md`, consumes it by citation — verified during this close-out); P20.04 §2 trigger register defined ONCE (P20.05/P20.06/P20.11 [Agent B] consume by citation); P20.05/P20.06 specs under `deploy/k8s/` |

## 1. Files created

| File | Subtask |
|---|---|
| `docs/phases-11-20/phase-20-deployment-architecture-scale/records/01_p20_01_capacity_model_framework.md` | P20.01 |
| `docs/phases-11-20/phase-20-deployment-architecture-scale/records/02_p20_02_load_test_plan.md` | P20.02 |
| `docs/phases-11-20/phase-20-deployment-architecture-scale/records/03_p20_03_latency_budget_allocation.md` | P20.03 |
| `docs/phases-11-20/phase-20-deployment-architecture-scale/records/04_p20_04_kubernetes_trigger_register.md` | P20.04 (P20-E-1 registered OPEN, DEFAULT (a)) |
| `docs/phases-11-20/phase-20-deployment-architecture-scale/records/05_p20_05_helm_chart_design.md` | P20.05 |
| `docs/phases-11-20/phase-20-deployment-architecture-scale/records/06_p20_06_gitops_design.md` | P20.06 |
| `docs/phases-11-20/phase-20-deployment-architecture-scale/records/07_p20_07_postgres_ha_topology.md` | P20.07 |
| `docs/phases-11-20/phase-20-deployment-architecture-scale/records/08_p20_08_pgbouncer_production_config.md` | P20.08 |
| `deploy/load-test/README.md`, `deploy/load-test/run_ladder.sh`, `deploy/load-test/seed_fixture.sh` | P20.02 (written-unrun suite, smoke.sh shape) |
| `deploy/load-test/k6/01_baseline.js`, `02_ramp.js`, `03_stress.js`, `04_soak.js` | P20.02 (the API ladder; no `thresholds` blocks anywhere) |
| `deploy/load-test/locust/locustfile_worker_dispatch.py` | P20.02 (fan-out leg, API-observed only) |
| `deploy/load-test/data/synthetic-data-policy.md` | P20.02 (the data policy) |
| `deploy/k8s/README.md` | P20.05 (chart design spec — written, never rendered) |
| `deploy/k8s/GITOPS.md` | P20.06 (GitOps design spec — written, inert) |
| this record | close-out |

No existing file was modified outside this ownership (verified: the ledger,
plan.md, context.md, `.agent/*`, `docs/foundations/*`, `docs/plan/*`,
`deploy/compose/*`, and all source trees are untouched by this wave).

## 2. Honesty statement (per-record, carried)

Every record 01–08 carries the written-unrun statement and, where
applicable, the trigger-UNFIRED statement. Specifics:

1. **No capacity figure exists** (P20.01 §5 all EMPTY; §4 measured-count
   column all EMPTY).
2. **No load test was executed**; the scripts have never been parsed by a
   k6 or Locust binary; `results/` does not exist.
3. **No measured latency exists** (P20.03 measured columns all EMPTY).
4. **No chart was rendered and no cluster exists** (P20.04 all triggers
   UNFIRED; P20.05/P20.06 specs marked never-rendered/never-installed and
   INERT).
5. **No HA or pooling component exists** (P20.07/P20.08: nothing deployed,
   no validation result; P12.05/P12.06/P11.19 consumed by citation, never
   restated).
6. **No dispatch-authority, release-ladder, or never-granted-set change**:
   no deployment topology in any record bypasses the command lifecycle,
   authorization, tenant isolation, or audit; ADR-010 untouched.

## 3. Key finding during implementation (recorded in the records)

The plan's P20.02 wording ("the dispatch accept path") required seam
verification: **`/api/v1/dispatch-commands` is READ-ONLY by recorded
contract** (`apps/control-plane/src/dto_phase4.rs`,
`NO_EXECUTE_ENDPOINT_STATEMENT` — there is no execute-command endpoint
anywhere on the API surface; transitions happen exclusively through the
application-layer gates). The load-test plan and the latency-budget record
were written against the REAL seams: the loadable accept write is the
schedule-proposal path (POST `/api/v1/optimization-runs` +
POST `/api/v1/schedules`, the P3.21 persist-before-use accept), the
dispatch-command surface is loaded READ-ONLY, and the fan-out leg observes
the worker's effects through the API (no NATS injection — hard ban 9).
Records 02/03 document this (A-P20.02-4/A-P20.02-5, A-P20.03-1); the
accept-budget hop table is application-layer, with no HTTP hop claimed.

## 4. Permitted-tool verification — verbatim outputs

Run 1 (after all Agent A artifacts were in place, before this record was
written):

```text
$ python tools/verify_docs.py
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC

$ python tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid

$ python -I tools/check_dependency_rule.py
OK ù dependency rule holds for 29 workspace members.
```

(The `ù` in the dependency-rule line is the tool's own output as captured
in this shell — a mojibake'd separator glyph in the Git Bash console; the
tool reports OK.)

Run 2 (after this record was written — the final state, quoted verbatim
below in §7).

## 5. Deviations from the task brief

| # | Deviation | Reason |
|---|---|---|
| D-1 | The brief's P20.02 sketch said "the dispatch accept path" targeting a command POST; implemented as the schedule-proposal accept path + READ-ONLY dispatch reads | The command POST does not exist (READ-ONLY recorded contract, §3 above); writing it would have violated the no-hallucinated-facts rule and the READ-ONLY statement |
| D-2 | The brief's k6 sketch implied result recording inside the scripts; implemented via the runner's output configuration (`-o json=…` in `run_ladder.sh`) | Keeps the scripts free of any summary logic and the results structurally separate (X-8); no `handleSummary` dependency on remote jslib summary modules |
| D-3 | `seed_fixture.sh` asserts HTTP 201 per creation and fails loud on anything else | The smoke.sh shape; a failed seed is not a load measurement |

No other deviations. Nothing was installed, built, run, rendered, or
provisioned.

## 6. Handoffs

**For Agent B (P20.09–P20.15) — already consumed where applicable:**
- P20.01 §4 cardinality budget: consumed by citation in `records/27`
  (verified; the amendment rule is the only legal growth path).
- P20.04 §2 trigger register: the single trigger list; P20.11's K8s-era
  shape consumes it by citation.
- P20.05 `deploy/k8s/README.md` chart layout: the shape P20.11's rollout
  design and P20.13's parity matrix reference.

**For wave 2 (P20.16 consistency audit):**
- Verify the P20.02/P20.03 records' correction trail (§3 here) against the
  plan's wording — the plan text is NOT edited (not my ownership); the
  records carry the corrected seam honestly.
- Verify the three-way rollout distinction: P20.11 (Agent B, `records/23`)
  carries it; my records touch rollout only via P20.04/P20.05/P20.06
  (inert-until-trigger designs).
- The trigger register (records/04 §2) and the cardinality budget
  (records/01 §4) are the two SINGLE definitions; confirm no restatement
  anywhere in phase-20 artifacts.

**For the human executor:** the filling commands are in each record (§8/§9
in 01; §9 in 02; §6 in 03–08) and `deploy/load-test/README.md` documents
the execution procedure. Every execution, deployment, drill, and
publication act is a human gate; none is scheduled by this program.

## 7. Final permitted-tool run (verbatim, final state)

```text
$ python tools/verify_docs.py
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC

$ python tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid

$ python -I tools/check_dependency_rule.py
OK ù dependency rule holds for 29 workspace members.
```

Status: **WAVE 1 (AGENT A) COMPLETE — WRITTEN-UNRUN, ALL TRIGGERS UNFIRED,
ALL MEASUREMENT CELLS EMPTY**; the human review, build, and run gates have
not happened and are not scheduled by this program.
