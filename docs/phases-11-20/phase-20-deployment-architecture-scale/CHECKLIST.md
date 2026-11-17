# Phase 20 — Deployment Architecture and Scale Readiness: Exit-Gate Checklist

**Exit gate (plan §8, the phase's own acceptance criteria): all 16
subtasks have complete written artifacts ticked only as "implemented,
awaiting human build/test"; no subtask claims a run, a rendered chart, a
provisioned cluster, a deployed standby, a load-test result, or a
measured latency; no capacity number is claimed; every trigger-gated
design is marked INERT with its trigger UNFIRED; the three-way rollout
distinction holds; the three permitted checks stay green.**

Each item states the evidence and its status. A checked item without
evidence is not checked. **Nothing in this phase has been built, run,
rendered, provisioned, deployed, measured, or executed** — the first
human build and the human gates named below are the program gates; no
item below claims runtime behavior, a deployment, or a measurement.

## Plan §8 acceptance criteria (one-for-one)

- [x] **Criterion 1 — all designs, registers, matrices, runbooks,
  evaluation records, config designs, and load-test scripts exist
  WRITTEN-UNRUN; no subtask claims a run, a rendered chart, a
  provisioned cluster, a deployed standby, a load-test result, or a
  measured latency.** Evidence: the 17 wave-1 records (01–08, 21–27) +
  close-outs 09/28 + the wave-2 records 41/42 exist under `records/`;
  the 10 `deploy/load-test/` files and 2 `deploy/k8s/` specs exist with
  WRITTEN-NEVER-EXECUTED / NEVER-RENDERED headers; the result-claim and
  capacity-figure sweeps (close record §3.6) found zero positive claims
  and zero figures. Status: **satisfied by written artifacts (UNRUN)**.
- [x] **Criterion 2 — no capacity number is claimed; P20.01 publishes a
  framework with EMPTY measurement/result cells and SLOs recorded as
  DERIVATION-PENDING.** Evidence: record 01 §5 template cells all
  `<EMPTY — NO RUN HAS HAPPENED>`; §4 measured-count column EMPTY in
  every row; the figure sweep (`[0-9]+ ?commands/s`, `p99 < [0-9]+ ms`)
  across all phase-20 artifacts returned ZERO matches (the phrase
  "N commands/s at p99 < X ms" appears only as the lettered template
  form). Status: **satisfied by the sweep** (close record §3.3/§3.6).
- [x] **Criterion 3 — no chart is rendered and no cluster is
  provisioned; the P20.04 DEFAULT (stay on Compose) is recorded; the
  trigger register names every trigger with its observable condition;
  the K8s-era designs are marked inert-until-trigger.** Evidence: no
  `Chart.yaml`, `*.tpl`, or `values.schema.json` file exists anywhere in
  the repository (repo-wide find, zero hits); record 04 §2 is the single
  register (T-1..T-4 with observable conditions, ALL UNFIRED, evidence
  pointers `<EMPTY>`); records 05/06 and record 23 §4 are marked INERT;
  P20-E-1 registered OPEN DEFAULT (a) at record 04 §3. Status:
  **satisfied by the register and the file sweep**.
- [x] **Criterion 4 — no HA, pooling, multi-region, or TimescaleDB
  component is deployed; P20.07/P20.08/P20.09/P20.10 cite the consumed
  P12/P13 records rather than restating them; each names its trigger
  UNFIRED.** Evidence: records 07/08/21/22 each carry the plain
  not-deployed statement and the UNFIRED trigger; the 15 cross-phase
  citation paths (P11.06/P11.10/P11.16/P11.19, P12.01–P12.05/P12.06/
  P12.11, P13.14, P18.06, P10.12/P10.18/P10.28) all resolve on disk
  (close record §3.9); no Patroni/PgBouncer/TimescaleDB/DNS-failover
  configuration exists in any deployment tree. Status: **satisfied by
  the audit**.
- [x] **Criterion 5 — the three-way rollout distinction holds:
  P20.11's progressive delivery is the control plane only; P18's device
  cohorts and the kernel plugin policy (cohort/percentage-free BY
  CONSTRUCTION) stay byte-identical and separately-argued.** Evidence:
  record 23 §2 carries the three-mechanism table and quotes the kernel
  `ROLLOUT_STATEMENT` verbatim (verified against the source lines
  `crates/flexcore-kernel/src/revocation.rs` 147–151, close record
  §3.7); the kernel file is byte-identical to HEAD (sha256 match, close
  record §3.8); P18.06 is cited as DISTINCT and untouched. Status:
  **satisfied by the quoted evidence**.
- [x] **Criterion 6 — the load-test ladder is complete and unexecuted:
  k6 (API) and Locust (worker fan-out) scenarios written against the
  named endpoints with production-shaped SYNTHETIC data only; no
  threshold gates a CI pipeline on a duration.** Evidence: the 10 files
  under `deploy/load-test/` exist; every k6 script carries the
  "Deliberately NO `thresholds` key" note (zero `thresholds` blocks);
  `run_ladder.sh` refuses to run without `FLEXCORE_LOAD_CONFIRM=1`;
  no `.github/workflows/` file was touched; no results directory
  exists. Status: **satisfied by the file checks**.
- [x] **Criterion 7 — the release ladder, ADR-010, and the
  never-granted permission set are unchanged; no deployment topology in
  any record bypasses the command lifecycle, authorization, tenant
  isolation, or audit.** Evidence: every phase-20 record carries the
  no-dispatch-authority/no-release-ladder-movement item (records 01–08
  §"does NOT establish", 21–27 §5-§7); the frozen 14-state/27-edge set
  (`crates/flexcore-domain/src/command_state.rs`) and the kernel
  `revocation.rs` are byte-identical to HEAD (sha256 matches, close
  record §3.8); the never-granted set (`dispatch.authorize`,
  `dispatch.issue`, `secret.read`, `network.any`) remains off the
  `Action` surface (quoted asserts, close record §3.8). Status:
  **satisfied by the quoted evidence**.
- [x] **Criterion 8 — verify_docs.py and validate_agent.py green after
  every documentation change; the dependency rule stays green after
  every implementation batch.** Evidence: the three outputs quoted
  verbatim in the wave-1 close-outs (09 §4/§7, 28 §5) and in the close
  record (§5, final state at close). Status: **satisfied by the quoted
  outputs**.

## Subtask-record coverage (15 ticked + P20.16 pending the orchestrator)

| Subtask | Record(s) | Ticked? |
|---|---|---|
| P20.01–P20.08 | 01–08 (+ close-out 09) | yes — orchestrator, citations verified |
| P20.09–P20.15 | 21–27 (+ close-out 28) | yes — orchestrator, citations verified |
| P20.16 | 41 + 42 (+ HANDOVER.md + CHECKLIST.md) | no — orchestrator's post-close action (expected citation in HANDOVER.md) |

## Human-gate inputs still pending (none closable by an agent)

| # | Gate input | Unblocks |
|---|---|---|
| 1 | First human build/test cycle of the 0–10 workspace | every WRITTEN-UNRUN claim in the phase; the seam behavior P20's records consume as DESIGNED |
| 2 | The P20.02 load-ladder execution (operator-pinned k6/Locust, `FLEXCORE_LOAD_CONFIRM=1`, a deliberately brought-up stack) → P20.01 §5 filled FROM THE RUN ONLY → P20.01 §6 SLO derivation + P20.03 measured columns | the capacity statement, the derived SLOs, the accept-budget validation |
| 3 | The trigger register's owner acts: T-1..T-4 evidence recorded or `none will` recorded → P20-E-1 closure → the K8s-era implementing wave (charts, GitOps, canary) after the R-P20-2 re-verification pass | records 05/06/23 §4 activation; the HPA/KEDA mechanics |
| 4 | The P12-side gates: first availability contract (record 07 §4) → Patroni drill; deployment gate (P11.19 §3.6) → PgBouncer V-1..V-5 run; second-region requirement (P12-E-3/P20-E-2) → record 21's runbook; telemetry volume evidence → record 22's evaluation | P20.07/P20.08/P20.09/P20.10 implementation |
| 5 | The three E-decisions (P20-E-1/E-2/E-3) and the consumed upstream registers (P12-E-1/E-2/E-3, P11-E-2) — all human-owned | every DEFAULT posture's adoption or rejection |
| 6 | The orchestrator's ledger tick for P20.16, citing records 41 + 42 | the expected 16/16 end state — **the phases 11–20 planning set complete** |

Full close position and the consistency-audit results:
`records/42_phase_20_close_and_consistency_audit.md`.
