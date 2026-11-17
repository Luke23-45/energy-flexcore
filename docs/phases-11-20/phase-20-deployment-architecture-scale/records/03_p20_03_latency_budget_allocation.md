<!-- P20.03 — Latency budget allocation: per-hop DESIGN table for the
     dispatch accept path and the terminal-state path. THE ASYNC DISPATCH
     HONESTY RULE: the accept SLA (request -> VALIDATED/queued ack) is NOT
     an execution SLA (-> terminal state). No budget row conflates them.
     Measured columns EMPTY (X-8) until P20.01/P20.02 produce figures. -->
# P20.03 — Latency Budget Allocation (DESIGN — measured columns EMPTY; accept ≠ execution)

| Field | Value |
|---|---|
| Subtask | P20.03 — Latency budget allocation (`ledger_phase_20.md` §2, row P20.03) |
| Wave / agent | Phase 20 wave 1, Agent A |
| Status | **DESIGN RECORD.** The budget ALLOCATIONS are registered design values (X-8 pre-registration, written before any run that could measure against them); every measured column is EMPTY. No measurement exists, no SLA is asserted, no SLO is derived (that is P20.01 §6's post-measurement step). |
| Sources | The closed 14-state command lifecycle (`crates/flexcore-domain` command_state; migration `0016_commands_p4_10.up.sql`); P20.01 framework (records/01 — the measurement-first order and the cardinality budget); P20.02 plan (records/02 — the scenarios that will produce the measured columns); `apps/control-plane/src/metrics.rs` (the registered SLI histogram `flexcore_command_accept_duration_milliseconds` with the fixed 12-bucket set — the existing instrument for the accept leg); P11.12/P11.13 (consumer idempotency and inbox records — the worker hop's seams); P11.20 (the registered stuck-state threshold pattern: a REGISTERED duration is not a measured duration) |
| Files touched | this record only |

## 1. The async dispatch honesty rule (restated per context constraint 3 discipline)

**The accept SLA and the execution SLA are two different budgets and never
one number.**

| Budget | Definition | Endpoint state | Instrument |
|---|---|---|---|
| ACCEPT budget | Proposal ENTRY → the command is durably `VALIDATED`/queued. Entry is the application-layer gate path, NOT an HTTP execute endpoint: `/api/v1/dispatch-commands` is READ-ONLY by recorded contract (`dto_phase4.rs` `NO_EXECUTE_ENDPOINT_STATEMENT` — there is no execute-command endpoint anywhere on the API surface), so the "request" of the accept SLA is the proposal record entering the validation gate from its origin (the schedule-driven pipeline or operator tooling) | `VALIDATED` (the queued state; the READ-ONLY evidence surface is where it is observed) | `flexcore_command_accept_duration_milliseconds` (registered, fixed buckets) |
| EXECUTION budget | The queued command → its TERMINAL lifecycle state (`COMPLETED`, `REJECTED`, `FAILED`, `EXPIRED`, `CANCELLED`, `SUPERSEDED`, `FALLBACK`) | a terminal state of the frozen 14-state vocabulary | No single registered instrument today; observed via lifecycle transitions — the P11.20 stuck-state threshold (24 h REGISTERED default, RECOMMENDATION pending operator tuning) is the dwell guard, never an execution SLA |

No row below contributes to both tables. The accept path's budget is spent
before the acknowledgement returns; everything after the ack is execution,
and its latency is dominated by human/policy gates (authorization,
feasibility) whose legitimate dwell time is an operator answer (the P11.20
reasoning, consumed) — the design therefore does NOT allocate a numeric
budget to the authorization/feasibility human-gate hops, and never will
present their dwell as a service-latency failure.

## 2. Accept-path per-hop budget table (DESIGN allocations)

The total accept envelope is a REGISTERED DESIGN DEFAULT (operator-tunable
at deployment; never a measured or achieved figure):

| Hop | What happens | Design allocation (ms) | Share of the 200 ms envelope | Measured (P20.02 leg) |
|---|---|---|---|---|
| H1 | Proposal entry: the origin pipeline hands the proposal record to the application-layer validation gate (the origin's own transport is OUTSIDE this budget; no HTTP execute endpoint exists — §1's READ-ONLY note) | 20 | 10% | `<EMPTY — UNMEASURED>` |
| H2 | The VALIDATION gate: gate checks (schedule-revision usability, envelope freshness, model-version status — the `command_gates` seams) | 40 | 20% | `<EMPTY — UNMEASURED>` |
| H3 | Unit-of-work open + tenant GUC (`SET LOCAL` pattern) + command INSERT (PROPOSED → VALIDATED as the first recorded transition) + outbox append (same transaction — the §6.3 transactional-outbox rule) | 60 | 30% | `<EMPTY — UNMEASURED>` |
| H4 | Commit (+ audit append inside the unit of work) | 50 | 25% | `<EMPTY — UNMEASURED>` |
| H5 | Gate completion: the journal/receipt return to the origin (no HTTP response is part of this path) | 10 | 5% | `<EMPTY — UNMEASURED>` |
| Slack | Unallocated reserve (GC, scheduling, clock skew between hops) | 20 | 10% | `<EMPTY — UNMEASURED>` |
| **Total** | proposal entry → `VALIDATED`/queued | **200** | 100% | `<EMPTY — UNMEASURED>` |

Notes:
- H3+H4 assume the INSTANCE-DEFAULT `synchronous_commit`. If P12-E-1
  closes to `remote_apply` for the command-lifecycle transactions
  (P12.05 §3), the H4 allocation is re-derived at deployment — the added
  standby wait lands in H4 and the envelope is either re-registered or the
  hop re-balanced; the re-derivation is recorded, never silently absorbed.
- H2 is the SYNCHRONOUS validation only. The FEASIBILITY gate (P4.12) and
  the APPROVAL gate are separate lifecycle gates AFTER the ack — they
  belong to the execution table's path and carry NO accept allocation.
- The allocations sum exactly; no row is shared with §3.

## 3. Terminal-state path (execution) — qualitative design, no numeric budget

| Hop | What happens | Design statement | Measured |
|---|---|---|---|
| E1 | Outbox → NATS publish → worker JetStream delivery (the P1.14/P11.13 idempotent consumer path) | Broker-bounded; observed via the consumer metrics; no numeric allocation — a queue-depth/backpressure observation, not an SLA row | `<EMPTY — UNMEASURED>` |
| E2 | Worker feasibility computation (the numerical work; Python workers when the P3 worker lands) | Solver-runtime is the P3.27/`MeasuredField` regime (per-run, per-scenario, baseline-not-target) — NEVER an API latency row | `<EMPTY — UNMEASURED>` |
| E3 | Lifecycle transitions to a terminal state (each a compare-and-swap on `dispatch_commands.state`) | Transition counts are lifecycle metrics; DURATION through the human gates is an operator answer (P11.20 discipline), never a service SLA | `<EMPTY — UNMEASURED>` |
| E4 | Dwell guard | The P11.20 registered stuck-state threshold (24 h default) bounds DETECTION of abnormal dwell — a detection guard, not a budget | n/a (registered constant, not a measurement) |

## 4. What this record does NOT establish

1. No measured latency of any kind — every measured cell is EMPTY (X-8).
2. No SLA or SLO: §2's allocations are design pre-registration inputs for
   P20.01's framework; the SLO (if any) is derived AFTER a run (P20.01 §6)
   and closed by the owner.
3. No conflation: §2 and §3 share no row, no allocation, and no
   instrument; the human-gate dwell is never presented as a latency
   failure.
4. No code change: the `flexcore_command_accept_duration_milliseconds`
   instrument already exists; nothing new is wired by this record.
5. No release-ladder or dispatch-authority change.

## 5. Reviewer-verification procedure (human)

1. Confirm §1 states the two-budget rule and that §2/§3 share no row.
2. Confirm every Measured cell is EMPTY/UNMEASURED.
3. Confirm the allocations are labeled registered design defaults and sum
   to the envelope.
4. Confirm the P12-E-1/`remote_apply` interaction is recorded (H4 note).
5. Confirm the P11.20 threshold is cited as a detection guard, not a
   budget.

## 6. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) After a P20.02 run on a real stack: fill the measured columns FROM
#     THE RUN ONLY (nearest-rank percentiles on the accept SLI and the
#     scenario summaries), then take them to P20.01 §5/§6 — never fill a
#     cell from a different run's conditions.
```

## 7. Findings and assumptions

| # | Item | Note |
|---|---|---|
| A-P20.03-1 | The hop split (H1–H5) matches the accept path as composed in the application layer (`crates/flexcore-application/src/command_gates.rs` → the adapter unit of work), with NO HTTP hop: `/api/v1/dispatch-commands` is READ-ONLY (`dto_phase4.rs` recorded statement) | Static inspection 2026-11-17; re-verified at first build |
| A-P20.03-2 | The 200 ms envelope and per-hop shares are design defaults | Operator-tunable; X-8 pre-registered; no measurement supports or contradicts them |
| F-P20.03-1 | No measured latency exists; the instrument is registered but unexercised | Carried by design |

Status: **DESIGN — WRITTEN-UNRUN** — allocations cannot measure a hop by
being written; the human run fills the columns and has not happened and is
not scheduled.
