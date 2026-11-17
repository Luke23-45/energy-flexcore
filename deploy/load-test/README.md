# P20.02 — Load-Test Suite (k6 + Locust) — WRITTEN, NEVER EXECUTED

Status: **WRITTEN-UNRUN** (decision record:
`docs/phases-11-20/phase-20-deployment-architecture-scale/records/02_p20_02_load_test_plan.md`).
Every script in this directory is complete and has never been executed; no
k6 or Locust binary has been installed or run by this program; no result
exists anywhere. Execution is a HUMAN GATE on a stack the operator has
deliberately brought up.

This suite generalizes the `deploy/compose/smoke.sh` shape: bring the
Compose stack up, wait for `/ready`, drive the API, tear down — extended
from one smoke pass to a declared load ladder.

## The ladder

| Order | Script | Shape | Target surfaces |
|---|---|---|---|
| 1 | `k6/01_baseline.js` | constant-arrival-rate, low level, short | `/ready`, `/health`, `GET /api/v1/telemetry/query`, `GET /api/v1/sites/{id}/view` |
| 2 | `k6/02_ramp.js` | ramping-arrival-rate stepping up | dispatch accept path: `POST /api/v1/dispatch-commands` (PROPOSED entry only) |
| 3 | `k6/03_stress.js` | stepped ramp climbing to find the latency knee | accept path + the P11.14 rate limiter (429s are recorded observations, not failures) |
| 4 | `k6/04_soak.js` | constant-arrival-rate, long duration (default 4 h) | accept path + `GET /api/v1/live` SSE streams held open + `/ready` over time |
| F | `locust/locustfile_worker_dispatch.py` | Locust HTTP users | worker dispatch fan-out observed END-TO-END through the API (accept → poll to `VALIDATED`) |

Hard rules carried by every script:

- **No thresholds.** No k6 `thresholds` block, no Locust duration
  assertion, no CI gate on any duration (the P3.27 no-runtime-threshold
  discipline). Scripts RECORD figures; the human decides what they mean
  through the P20.01 capacity framework (measurement → number → SLO, in
  that order).
- **The lifecycle is never bypassed.** Commands enter as `PROPOSED`
  through the authorized API path or not at all; no direct NATS injection
  exists in this suite.
- **Synthetic data only** (see `data/synthetic-data-policy.md`).
- **Results are structurally separate** (X-8): `handleSummary` writes JSON
  records into `results/` (gitignored), never into the scripts.

## Environment contract (all scripts)

| Variable | Meaning | Default |
|---|---|---|
| `FLEXCORE_LOAD_BASE_URL` | control-plane base URL | `http://localhost:8080` |
| `FLEXCORE_LOAD_TENANT_UUID` | the seeded fixture tenant | required by k6/Locust legs that need it (set by `seed_fixture.sh` output) |
| `FLEXCORE_LOAD_TOKEN` | dev-auth bearer token (`Bearer dev tenant=<uuid> subject=… role=…`) — requires the dev-auth build per `deploy/compose/smoke.sh` note | required |
| `FLEXCORE_LOAD_PORTFOLIO_ID`, `FLEXCORE_LOAD_SITE_ID`, `FLEXCORE_LOAD_ASSET_ID` | the seeded fixture ids the proposal/observation legs reference | set by `seed_fixture.sh` |
| `FLEXCORE_LOAD_RESULTS_DIR` | where summary JSON lands | `./results` |
| Leg-specific arrival rates, VU counts, durations | declared load inputs, operator-tunable | see each script header |

## Execution procedure (the human gate)

```sh
# 0. Install k6 and Locust at YOUR pinned versions, OUTSIDE this program.
#    The scripts have never been parsed by either tool; verify the
#    documented surface against your pinned version first.
# 1. Confirm FLEXCORE_LOAD_CONFIRM=1 (the run_ladder.sh human gate).
# 2. Bring up the dev-auth stack (see deploy/compose/smoke.sh header).
# 3. Seed the fixture:
./seed_fixture.sh
# 4. Export the env contract printed by the seeder, then run legs in
#    order (each script header documents its knobs):
./run_ladder.sh            # all four k6 legs, in order
#    or individual legs, then the Locust fan-out leg:
locust -f locust/locustfile_worker_dispatch.py --headless \
    --users 20 --spawn-rate 2 --run-time 10m \
    --host "$FLEXCORE_LOAD_BASE_URL"
# 5. Summaries land in $FLEXCORE_LOAD_RESULTS_DIR; feed them into the
#    P20.01 §5 template FROM THE RUN ONLY.
```

`results/` is a run artifact: never committed.

## Files

```text
README.md                              this file
run_ladder.sh                          orchestrates the four k6 legs (smoke.sh shape)
seed_fixture.sh                        creates the synthetic fixture chain via the API
k6/01_baseline.js                      baseline leg
k6/02_ramp.js                          ramp leg
k6/03_stress.js                        stress leg (knee finder)
k6/04_soak.js                          soak leg (incl. the SSE stream holder)
locust/locustfile_worker_dispatch.py   worker fan-out leg (API-observed)
data/synthetic-data-policy.md          the production-shaped synthetic data policy
```
