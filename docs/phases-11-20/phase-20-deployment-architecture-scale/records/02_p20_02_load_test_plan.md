<!-- P20.02 — Load-test plan (k6 + Locust): the API ladder (baseline -> ramp
     -> stress -> soak) and the worker dispatch fan-out, WRITTEN AND NEVER
     EXECUTED. Scripts live under deploy/load-test/ and generalize the
     deploy/compose/smoke.sh shape. No threshold anywhere gates a CI
     pipeline on a duration (P3.27 no-runtime-threshold discipline). -->
# P20.02 — Load-Test Plan (k6 + Locust — WRITTEN-UNRUN; no run has happened, no result exists)

| Field | Value |
|---|---|
| Subtask | P20.02 — Load-test plan (`ledger_phase_20.md` §2, row P20.02) |
| Wave / agent | Phase 20 wave 1, Agent A |
| Status | **WRITTEN-UNRUN TOOLING.** Every script under `deploy/load-test/` is complete and has NEVER been executed; no k6 or Locust binary has been run in this program (loop bans, conventions §1); no result table has any entry. Execution is a human gate. |
| Sources | [S-61] (the load-ladder discipline: baseline → ramp → stress → soak with concurrent-client tools); gap analysis §5 (Tier 0/1 rows — concurrent-client load testing deferred to this phase); `apps/sim-runner/src/load_run.rs` §1 (the single-client honesty boundary this plan extends with concurrent clients — the extension is EXPLICITLY recorded, never smuggled); `deploy/compose/smoke.sh` (the orchestration shape generalized); P11.14 rate limiting (`docs/phases-11-20/phase-11-operational-robustness/records/`, the API-edge limiter the stress leg will surface); P16.01/P16.07/P16.08 and P14.11 records (the market/telemetry shapes the synthetic data policy derives from — consumed by citation) |
| Files touched | `deploy/load-test/README.md`, `deploy/load-test/run_ladder.sh`, `deploy/load-test/seed_fixture.sh`, `deploy/load-test/k6/01_baseline.js`, `deploy/load-test/k6/02_ramp.js`, `deploy/load-test/k6/03_stress.js`, `deploy/load-test/k6/04_soak.js`, `deploy/load-test/locust/locustfile_worker_dispatch.py`, `deploy/load-test/data/synthetic-data-policy.md`, this record |

## 1. Honest status — the plain statement

**Nothing has run.** The scripts are written artifacts whose execution is a
human gate on a stack the operator has deliberately brought up. No result
exists, no latency figure exists, no concurrency ceiling exists. The plan
EXTENDS the `load_run.rs` boundary — that harness measures ONE sequential
client and its figures are "never a deployment capacity claim"; the
concurrent-client layer this plan adds is what makes a deployment-capacity
measurement POSSIBLE, and its results feed P20.01's framework (measurement
→ number → SLO, in that order). Until a run happens, the honest statement
is exactly: the concurrent-client layer is written and unexercised.

## 2. The ladder (k6 — the Axum API surface)

All k6 scripts share the configuration contract (env vars, no thresholds —
see §5) and target the real routes (`apps/control-plane/src`):

| Leg | Script | Shape | Targeted surfaces | Status |
|---|---|---|---|---|
| 1. Baseline | `k6/01_baseline.js` | `constant-arrival-rate` at a LOW fixed arrival rate, short duration | `/ready`, `/health`, GET `/api/v1/telemetry/query`, GET `/api/v1/sites/{id}/view` | WRITTEN, NEVER RUN |
| 2. Ramp | `k6/02_ramp.js` | `ramping-arrival-rate` stepping up through pre-registered levels | the schedule-proposal accept path (the heaviest synchronous API write): POST `/api/v1/optimization-runs` + POST `/api/v1/schedules` per iteration (unique `candidate_id`, revision 1, new chain), plus the READ-ONLY dispatch-command read surface (`GET /api/v1/dispatch-commands`) | WRITTEN, NEVER RUN |
| 3. Stress | `k6/03_stress.js` | stepped `ramping-arrival-rate` climbing until an operator-chosen stop; the leg LOOKS FOR the latency knee (context.md §4) — it never assumes one | the schedule-proposal accept path + the P11.14 rate limiter's backpressure (429s are an EXPECTED observation, recorded, not judged) | WRITTEN, NEVER RUN |

The dispatch-command honesty note (consumed from `dto_phase4.rs`'s recorded
`NO_EXECUTE_ENDPOINT_STATEMENT`): **`/api/v1/dispatch-commands` is
READ-ONLY** — there is no execute-command endpoint anywhere on the API
surface; the §14.1 transitions happen exclusively through the
application-layer gates. The loadable write path that drives
dispatch-adjacent work is therefore the schedule-proposal accept path, and
the dispatch-command surface is loaded as a READ path only. No scenario
fabricates a lifecycle state, and no scenario POSTs to
`/api/v1/dispatch-commands` (the route has no POST handler to call).
| 4. Soak | `k6/04_soak.js` | `constant-arrival-rate` at a steady env-configured level for a long duration (default 4 h, operator-tunable) | `/ready` liveness over time, SSE stream `GET /api/v1/live` held open, dispatch accept path | WRITTEN, NEVER RUN |

The SSE leg (`04_soak.js`) holds N long-lived streams open against
`GET /api/v1/live` (the tenant-scoped SSE stream, `routes_live.rs`) using
k6's HTTP streaming callback; the stream's at-least-once/UNORDERED delivery
contract and the `RESYNC_REQUIRED` mechanism are consumed as documented
behavior, and the leg observes connection lifetime and heartbeat cadence —
it asserts nothing about event ordering (that contract is at-least-once
unordered by design).

The worker fan-out leg is Locust (§3); k6 does not talk to NATS in this
plan — injecting synthetic messages onto `flexcore.v1.>` subjects directly
would bypass the command lifecycle and authorization gates (hard ban 9),
so the fan-out is measured THROUGH the honest API path end-to-end.

## 3. The worker dispatch fan-out (Locust)

`locust/locustfile_worker_dispatch.py`:

| Element | Design |
|---|---|
| Client shape | `HttpUser` driving the schedule-proposal accept path: POST `/api/v1/optimization-runs` (unique `candidate_id` per iteration, `status: feasible`) then POST `/api/v1/schedules` (revision 1, new chain, realizing that candidate) — the recorded accept write (P3.21 persist-before-use; `schedule.propose` authorization), followed by a poll of the READ-ONLY evidence surfaces: `GET /api/v1/dispatch-commands` (list) and `GET /api/v1/dispatch-commands/{command_id}` for any command the downstream pipeline recorded |
| What "fan-out" means here | The measured quantity is end-to-end: accept latency of the proposal writes plus the observed appearance of dispatch-command records and lifecycle transitions in the READ-ONLY API evidence (the worker's NATS consumption drives the transitions; its effects surface through the API). The worker is a consumer of NATS, not an HTTP endpoint; the fan-out is observed through its API-visible effects, never by injecting broker traffic. The exact proposal → command pipeline depth is verified at execution (written-unrun — see A-P20.02-4) |
| Wait model | `constant_pacing` with an env-configured pace; locust count and spawn rate are operator inputs, not script-embedded targets |
| Data | The seeded fixture (§4) — tenant/portfolio/site/asset chain created once by `seed_fixture.sh` with dev-auth, read back by env-configured ids; the schedule/optimization payloads are generated per iteration from the fixture ids (the data policy) |
| What it never does | No direct NATS publish, no lifecycle fabrication (no scenario ever asserts a command state; the READ-ONLY surface is only ever read), no cross-tenant tokens |

## 4. Production-shaped SYNTHETIC data policy

`deploy/load-test/data/synthetic-data-policy.md` (the full policy; summary
here):

1. **Synthetic only.** Every identity, payload, and time series is
   generated by the seed scripts with the `SYN-` prefix convention. No
   production data ever enters a load run; the entire 0.x line is
   simulation-only and stays there.
2. **Production-SHAPED, not production-scaled by assertion.** Volumes,
   tenant spread, and telemetry cadence defaults derive from the recorded
   P16 market-record shapes and P14 uplink-cadence shapes (consumed by
   citation), and every default is operator-tunable via env — a shape
   assumption is labeled ASSUMPTION, never presented as a measured
   production profile.
3. **Versioned set.** The generated fixture set is versioned; changing a
   shape is a NEW version (the `research-set-v1.json` anti-silent-growth
   rule carried over), so two runs' results are comparable only within a
   version.
4. **Tenant spread.** A multi-tenant synthetic spread (default 8 tenants
   in the dev-auth fixture) exists so RLS and per-tenant behavior are
   exercised under load — never to claim multi-tenant capacity.

## 5. The threshold discipline (P3.27 carried)

- **No `thresholds` block exists in any k6 script. No Locust assertion
  fails a run on a duration. No CI gate reads a load-test duration.** The
  scripts record figures via the runner's output configuration (`k6 run -o
  json=…` / `--summary-export`, wired by `run_ladder.sh` into a results
  directory that is structurally separate from the scripts — X-8); what
  the figures MEAN is decided by the human through P20.01's framework.
- Load LEVELS (arrival rates, VU counts, durations) are inputs declared in
  the scripts and tunable by env — they are the declared load, not pass
  criteria.
- If a future operator wants a judged threshold, it is pre-registered (X-8)
  in P20.01's template BEFORE the run that could fill it.

## 6. Tool-version honesty

| Tool | Version basis | Status |
|---|---|---|
| k6 | No version pinned in this program (nothing installed); the scripts use only the documented OSS API surface (HTTP, `constant-arrival-rate`, streaming callback) | The operator pins the binary at execution; streaming-callback semantics are verified against the pinned version THEN — written-unrun means the script has never parsed a response |
| Locust | Same posture; stdlib + the `locust` package at the operator's pinned version | The `locustfile` uses the documented `HttpUser`/`task` surface only |
| The stack under test | The Compose services as defined in `deploy/compose/docker-compose.yml` (pinned tags there), brought up by `run_ladder.sh` in the same shape as `smoke.sh` | NEVER BROUGHT UP by this program |

## 7. What this plan does NOT establish

1. No result: every result table in this record and in the scripts'
   summary output is EMPTY until a human run fills it.
2. No capacity claim: the concurrent-client layer does not publish a
   capacity number; P20.01's order (measure → number → SLO) owns that.
3. No CI gate: nothing in `.github/workflows/` is touched; no threshold
   gates any pipeline on a duration.
4. No lifecycle bypass: the dispatch-command REST surface is READ-ONLY
   and is loaded as such; no scenario fabricates a lifecycle state; the
   never-granted permission set, the lifecycle gates, tenant isolation,
   and audit are untouched.
5. No tool installation: k6 and Locust are the operator's execution-time
   tools; this program installs nothing.

## 8. Reviewer-verification procedure (human)

1. Confirm every file in `deploy/load-test/` exists and carries the
   written-unrun header; confirm `git log` shows no execution artifact
   (no results directory committed).
2. Confirm no `thresholds` block in any k6 script and no duration-based
   assertion in the Locust file; confirm `run_ladder.sh` requires an
   explicit human confirmation env flag.
3. Confirm the targeted routes match `apps/control-plane/src` (including
   the READ-ONLY dispatch-command surface — no scenario POSTs to it) and
   that no scenario fabricates a lifecycle state.
4. Confirm the data policy marks every volume/cadence default as an
   ASSUMPTION or a citation, never a measured production profile.
5. Confirm §4's hard ban on direct NATS injection in the fan-out leg.

## 9. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to these files):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) Execution is the human gate (per deploy/load-test/README.md):
#     install k6/Locust at pinned versions OUTSIDE this program, set the
#     env contract, run seed_fixture.sh, then run_ladder.sh with the
#     confirmation flag set. Results land in the results directory; feed
#     them into P20.01 §5 FROM THE RUN ONLY.
```

## 10. Findings and assumptions

| # | Item | Note |
|---|---|---|
| A-P20.02-1 | k6's OSS HTTP streaming callback supports holding SSE connections open | Documented API surface; UNVERIFIED against the operator's pinned binary — verified at execution |
| A-P20.02-2 | The dev-auth token shape (`Bearer dev tenant=… subject=… role=…`) is accepted by a dev-auth build per `smoke.sh` | Consumed from the existing script; the dev-auth build requirement carries over |
| A-P20.02-3 | Synthetic volume/cadence defaults (data policy) are ASSUMPTIONS derived from the cited record shapes, not measurements | Every default is operator-tunable |
| A-P20.02-4 | The schedule-proposal write path (POST `/api/v1/optimization-runs` + POST `/api/v1/schedules`) is the loadable proxy for dispatch-adjacent accept work because `/api/v1/dispatch-commands` is READ-ONLY (`NO_EXECUTE_ENDPOINT_STATEMENT`, `dto_phase4.rs`) | The proposal → dispatch-command pipeline depth driven by the worker is verified against the pinned build AT EXECUTION (written-unrun) |
| A-P20.02-5 | One optimization-run candidate per iteration (unique `candidate_id`, revision 1, new schedule chain) avoids the duplicate-candidate 409 | Verified at execution against the pinned build |
| F-P20.02-1 | No load-test result exists anywhere | Carried by design — the human run is the generator |

Status: **WRITTEN-UNRUN** — a load script cannot produce a result by being
written; the execution is the human gate and has not happened and is not
scheduled.
