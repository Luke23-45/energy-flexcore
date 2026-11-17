# P12.07 — Chaos experiment program (the four defining experiments, written-unrun)

| Field | Value |
|---|---|
| Subtask | P12.07 (chaos experiment program — steady-state hypotheses from Phase 11 SLOs; the four defining experiments as repeatable scripts, written-unrun) |
| Wave / agent | Phase 12 wave 1, Agent B |
| Status | **WRITTEN-UNRUN.** Four pre-registered experiment scripts plus a shared harness are written; nothing has been compiled, executed, probed, or injected. No fault has been injected into any system anywhere in this program; every evidence cell in the scripts' JSON output and in this record is EMPTY/UNRUN. Execution is a HUMAN GATE against an isolated lab deployment that does not exist yet (plan §1; conventions §5). |
| Sources | [S-81] (Litmus — the steady-state-hypothesis discipline: hypothesis first, fault second, verified during AND after); [S-82] (Gremlin — game-day framing, blast-radius bounding); [S-65] (JetStream at-least-once/redelivery model, cited by the NATS experiment); [S-61] (the SLO register the hypotheses anchor on) |
| Files touched | `docs/phases-11-20/phase-12-backup-dr-resilience/experiments/lab_harness.py` (new), `experiments/c_pg_1_postgres_kill.py` (new), `experiments/c_nats_1_nats_outage.py` (new), `experiments/c_worker_1_worker_crash.py` (new), `experiments/c_pool_1_pool_exhaustion.py` (new) |

## 1. Steady-state hypotheses — Phase 11 anchoring (the PENDING-P11-ANCHOR rule)

The Phase 11 SLO register EXISTS (`../phase-11-operational-robustness/records/02_p11_02_slo_register.md`), so the hypotheses below are ANCHORED, not pending. Every anchor is a RECOMMENDATION-labeled SLO with a pre-registered threshold (X-8) — the experiments measure against the register as written; no threshold is re-declared here.

| Experiment | Steady-state hypothesis (pre-registered in the script) | Phase 11 anchor |
|---|---|---|
| C-PG-1 | Commands survive a PostgreSQL kill in the same or a strictly-later frozen state; no state outside the 14-state vocabulary; no already-issued command re-issued by outbox replay | SLO 3 (accept latency ≤ 500 ms), SLO 5 (stuck count — the P11.20 24 h RECOMMENDATION threshold); the frozen lifecycle (`crates/flexcore-domain/src/command_state.rs`, P0.11) |
| C-NATS-1 | During a NATS outage commands queue/park safely; after reconnect durable consumers resume with redelivery, the outbox backlog drains, zero message loss is OBSERVED, DLQ receives only poison | SLO 1 (control-plane availability 99.9%), SLO 3; P11.13 DLQ-family hardening (`topology.rs`); P1.16 ack-after-commit (`consumer.rs`) |
| C-WORKER-1 | A worker killed mid-transition produces no duplicate dispatch (redelivery + inbox + the P11.12 state-machine oracle neutralize it); the crash is recorded; the pool degrades visibly | SLO 1, SLO 4 (terminal-state ratio), SLO 5; P7.18 supervision (`supervision.rs`); P11.12 (`records/24`) |
| C-POOL-1 | Under over-budget load callers get 429 + Retry-After + the `rate_limited` envelope, never hangs; `/health`, `/ready`, `/metrics` are never rate-limited | SLO 1, SLO 2 (API latency ≤ 1000 ms); P11.14 limiter seam (`apps/control-plane/src/rate_limit.rs`, `records/26`) |

Not anchored (recorded honestly, never invented): no Phase 11 SLO exists for pool latency under exhaustion or for outbox drain duration — the C-POOL-1 and C-NATS-1 scripts record their deltas as run outcomes for the human to judge, and any future SLO for those signals must be ADDED to the P11.02 register by its own amendment process, never by a chaos script.

The P12.06 pooling design is cited by its landed path: `records/06_p12_06_pgbouncer_introduction.md` (Agent A, wave 1) — the sibling-citation pattern (plan §2.1); the expected-path citation `records/06_p12_06_pgbouncer_pooling.md` used at writing time was reconciled to A's actual record name (the pool-exhaustion experiment consumes the design; the pool-cap leg remains a lab-profile input).

## 2. The four experiments (plan §2.2 rows, one script each)

| ID | Script | Fault (env-parameterized, never invented) | Blast radius (bounded by design) | In-code oracle |
|---|---|---|---|---|
| C-PG-1 | `c_pg_1_postgres_kill.py` | stop the lab Postgres container mid-command-flow, hold, restart | one lab instance, one synthetic tenant, simulation-only data, no enrolled edges | the frozen 14-state / 27-edge table (transcribed in `lab_harness.py` with the copy rule); migration 0016 command/transition rows; migration 0007 outbox (R-6 zero-re-issue criterion, P9.24 §5 — cited, not rewritten) |
| C-NATS-1 | `c_nats_1_nats_outage.py` | stop the lab NATS container, hold, restart | the lab NATS instance and its consumers only | `/ready` named checks `database`/`broker` (P11.21 probe table); `flexcore_command_consumer_duplicate_deliveries_total` (P11.12); `flexcore_commands_stuck` (P11.20); outbox row delta |
| C-WORKER-1 | `c_worker_1_worker_crash.py` | kill the lab worker container mid-flow, restart | the supervised worker pool in the lab | `supervision.rs` (`record_crash`, `BackpressureRefused`, `PoolStatus::Degraded`); `flexcore_command_state_transitions_total{from,to}`; the duplicate-delivery counter |
| C-POOL-1 | `c_pool_1_pool_exhaustion.py` | concurrent load over the governed candidate-submission POSTs beyond the registered class budget (no container fault) | the API edge + pool of ONE lab instance (the P11.14 in-process, single-instance limiter shape) | `rate_limit.rs` 429/Retry-After/`rate_limited` contract; `flexcore_http_requests_total{status="429"}`; probes never rate-limited |

The pool-exhaustion leg of C-POOL-1 that exercises the P12.06 pooled-connection ceiling is a lab-profile input (an env/operator value), NOT improvised in the script — the script tests the API-edge limiter contract as written code today.

## 3. Script mechanics (what the human gate gets)

1. **Pre-registration before execution.** Every script prints its hypothesis, anchoring, blast radius, fault, oracle, abort criteria, and roles VERBATIM before anything runs ([S-81] order: hypothesis first, fault second). Without `--execute` a script prints the pre-registration and exits — nothing is touched.
2. **Keyboard gate.** With `--execute`, the director must type `GO` (or pass `--yes` on a witnessed run, recorded as assumed-yes in the evidence JSON).
3. **Lab-control commands are operator input.** The stop/start commands come from environment variables (`FLEXCORE_LAB_PG_STOP_CMD` etc.); a script refuses to run a fault whose lab-control command is not provided — nothing is invented.
4. **Thresholds registered before the run (X-8).** Outage windows, budgets, concurrency, and durations are CLI arguments recorded in the evidence JSON; they are registered BEFORE the run that could fill them, per conventions §5.
5. **Evidence recorder.** `lab_harness.Evidence` writes a JSON record whose cells start `(EMPTY — UNRUN)` and are filled only from the run's own probes and clocks; oracle findings are recorded verbatim, including failures (a failed check is a FINDING, never hidden, never auto-retried).
6. **The oracle copy rule.** The 14-state / 27-edge table in `lab_harness.py` is a TRANSCRIPTION of `crates/flexcore-domain/src/command_state.rs`'s frozen table — a lab-side copy, never a second authority; a mismatch with the Rust table is a FINDING.

## 4. What running these would and would not establish

Would establish (once run, by humans, on an isolated lab): that on the tested day, at the tested scale, with the injected faults named above, the written-code behaviors held — same-or-later command states, resumed consumers, no observed duplicate dispatch, 429s not hangs.

Would NOT establish: any guarantee (§13.3 semantics — zero observed violations from a finite test set is a test outcome, never a guarantee); any claim about deployments not tested; any protection-grade or safety property (the central-outage rule, `deployment-upgrade-model.md` §5, is site-local EMS authority — nothing here tests or provides it); any new dispatch authority (ADR-010 — these experiments add zero).

## 5. ASSUMPTION / UNRUN labels

- **UNRUN**: every script, probe, fault, and oracle check; the evidence JSON files do not exist anywhere.
- **ASSUMPTION**: the lab exposes `/health`, `/ready`, `/metrics` per the P11.21 per-service table, and the worker's readiness body names its failing dependency checks as written (`apps/worker/src/probes.rs`); verified at the first human build.
- **ASSUMPTION**: the psql CSV shape used by C-PG-1 matches the 0016 schema's column names at query time; the query text is operator-amendable at the game day without code change.
- **RECOMMENDATION**: every CLI default (30 s PG hold, 60 s NATS hold, 64-way concurrency) is a pre-registered default awaiting operator tuning, never a measured claim.

## 6. Handoff notes

1. **Wave 2 / P12.13**: the evidence JSON shape here is an input to the backup/restore + DR evidence templates — the templates carry the same EMPTY-cell discipline.
2. **Agent A reconciliation (DONE at close)**: A's records landed as `records/01_p12_01_backup_architecture.md`, `records/02_p12_02_backup_procedures.md`, `records/06_p12_06_pgbouncer_introduction.md`; the C-POOL-1 sibling citation was reconciled to the actual P12.06 record name. A's artifacts (`deploy/compose/backup/backup_postgres.sh`, `jetstream_snapshot.sh`, `pgbackrest.conf`, `verify_backup.sh`) are the backup-procedure of record the C-PG-1/C-NATS-1 lab profiles consume when the human gate runs them.
3. **Human gate**: first build of the Phase 0–10 workspace precedes any experiment (hard boundary 1); experiments run only against the ISOLATED lab, never a pilot posture.
