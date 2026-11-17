# forecast_worker — the P15.02 statistical baseline worker, completed by P15.17

Status: **WRITTEN-UNRUN** (program conventions §1: nothing here has been
built, installed, or executed; the first execution is a human gate). The
pattern source is `../gnn_worker/` (guarded deps, one JSON request on
stdin → one result envelope on stdout, malformed input → NOTHING on
stdout, exit 2, proposal-only emission).

## P15.17 completion (additive)

The worker now serves TWO modes (one request document selects one):

| Mode | Envelope kind | What it is |
|---|---|---|
| `baseline_forecast` | `baseline_forecast_result` | The P15.02 skeleton contract, unchanged |
| `publish_forecast_record` | `forecast_publication_record` | The same frozen baseline run, wrapped as a provenance-stamped forecast-artifact RECORD (the P15.01 provenance layer mirrored: the REQUIRED `input_data_ref` / `producing_model_ref` / `eval_set_ref` links are request fields validated BEFORE any computation, so provenance cannot silently detach; `artifact_kind` is the closed `load` / `pv_generation` / `price` vocabulary; `forecast_version` is whitespace-free) |

The publication record's digest is the FNV-1a 64 TEST-GRADE checksum over
the record's canonical rendering (the published vectors pin the Python
mirror to the Rust helper). It is an integrity stamp for replay
comparison, explicitly NOT a security primitive and NOT a signature: no
signing identity exists, and the record is `UNSIGNED` — a production
digest (SHA-256 with a managed key) is a recorded deployment requirement.
The publication RECORD is an input to scheduling analytics, never a
command; `assert_proposal_only` scans the publication envelope exactly as
it scans the baseline envelope.

Additional tests: `tests/test_forecast_publication.py` (same hand-computed
mirror discipline; WRITTEN, never run).

## What this worker is

The out-of-process execution seam for the three statistical load/PV
baseline forecasts the P15.02 design freezes as deterministic,
registry-frozen references:

| Baseline | Version | Rule |
|---|---|---|
| `forecast-baseline/persistence` | `persistence.v1` | `forecast(t) = last observed value`, carried forward for every horizon step |
| `forecast-baseline/seasonal-naive` | `seasonal-naive.v1` | `forecast(t) = value one declared season earlier`; longer horizons wrap within the last season |
| `forecast-baseline/degree-day-regression` | `degree-day.v1` | OLS of load on (intercept, HDD, CDD), deterministic Gaussian elimination, first-max pivoting |

The Rust side is the SINGLE HOME of the rule sets
(`crates/flexcore-application/src/forecast_baselines.rs`); this worker
MIRRORS them and the mirror is pinned by identical hand-computed test
vectors (the same numbers, two languages, one rule set).

## Honesty lines (binding)

1. **No skill claim.** Every output carries the no-skill note: a baseline
   forecast is the REFERENCE other models are compared against; skill
   exists only as an X-8-gated evaluation result (P15.05), and every
   evaluation cell is EMPTY.
2. **UNTRAINED.** The model registry state of every baseline is UNTRAINED
   (P15.06 first-class state). The P3.10
   `SYNTHETIC_BASELINE_DISCLAIMER` is superseded only by EVALUATED
   baselines, never silently.
3. **Proposal-only.** Outputs are INPUTS, RECORDS, and PROPOSALS (ADR-010
   extension): `assert_proposal_only` refuses emission of any
   authorization/command-shaped key. No field-write path, no device
   channel.
4. **Permissions.** The worker's declared permission vocabulary is
   exactly `telemetry.read` + `forecast.publish` (the forecast-worker
   manifest's classes). A request declaring `dispatch.authorize`,
   `dispatch.issue`, `secret.read`, or `network.any` (the NEVER-GRANTED
   set, mirrored verbatim from the kernel) is refused first.
5. **Mode-locked** to simulation/shadow; no request field can change it.
6. **Guarded deps.** numpy is the ONLY framework guarded import (INSIDE
   its function, raising the NAMED `NUMPY_DEPLOYMENT_REQUIREMENT`); the
   frozen rule sets are pure stdlib and never import a framework. No
   pandas exists anywhere in this worker — the token appears in no
   import, guard, or comment in the code (final-review finding
   FRV1-R4-007 corrected this line).

## Transport contract

```sh
python apps/worker/python/forecast_worker/forecast_baseline_worker.py < request.json
```

One JSON request on stdin (`flexcore-forecast-worker/1`): `mode`
(`baseline_forecast`), `baseline` (a frozen registry id), `scope`
(`scope_kind` site/portfolio + `scope_id`), `horizon` (`steps`,
`interval_millis`), `history` (finite numbers, the caller's declared
sign convention — `P > 0` export, never normalized), `season_length`
(seasonal-naive), `degree_day` (degree-day), `permissions_declared`
(validated against the producer vocabulary), `provenance`
(`input_snapshot_id`, `request_id`).

One JSON envelope on stdout; malformed input → NOTHING on stdout,
reason on stderr, exit 2. An in-band rule refusal (empty history, short
season, singular design matrix) is a `termination_status: "failed"`
envelope — a refusal is a result, never a crash.

## Self-tests (human gate)

```sh
python apps/worker/python/forecast_worker/forecast_baseline_worker.py --self-test
pytest apps/worker/python/forecast_worker/tests/
```

The pytest runner itself is recorded, never installed, in
`apps/worker/python/requirements-dev.txt` (SECONDARY-SOURCE PIN;
RE-VERIFY AT FIRST INSTALL).

The `--self-test` flag runs the in-module `run_self_tests()` (bare
invocation runs the documented stdin→stdout transport instead); the
pytest suite carries the same hand-computed cases. Both were WRITTEN,
never run in this loop.
