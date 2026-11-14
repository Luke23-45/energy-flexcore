# P7.17 — The FlexCore Class C Python gRPC worker template

Date: 2026-11-13. Subtask: P7.17 (`ledger/phase-07-plugin-sdk/
ledger_phase_7.md` §P7.17). Companion record:
`ledger/phase-07-plugin-sdk/17_18_workers_supervision.md` (the decision
record; this README is the template guide the record references).

## 0. Honest status — READ FIRST

**Everything here is WRITTEN-UNRUN.** grpcio was never installed, no
worker process was ever started, `buf lint` / `buf breaking` never ran
against the proto, and the pytest suite was never executed. The only
genuinely runnable parts are the stdlib-only pure-logic modules
(`config`, `health`, `errors`, `handler`, the example worker's logic,
and `server.ShutdownSequence`) — runnable by code inspection and by the
written-but-unrun test suite. Every grpcio call in `server.py` carries
an `UNVERIFIED — grpcio API` comment. Nothing in this tree is evidence
that any worker ever served a request.

## 1. What a FlexCore worker is

A Class C plugin (report §5.4) is an **out-of-process worker**: a
separate process/container, serving one versioned gRPC/Protobuf
contract (`contracts/worker_api.proto`, package `flexcore.worker.v1`),
into which Python numerical/ML code is isolated (ADR-004: Python is
isolated to workers, never a backend — the template must not become a
second general-purpose backend, §7.2; scope-freeze prohibition P-6).

What a worker IS:

- A proposal generator. Every output is a **proposal** (ADR-010): it
  enters the protected candidate path (validation → feasibility →
  authorization) like any other candidate. A worker cannot dispatch,
  authorize, or issue anything.
- A §9.2-conformant result producer: every result carries provenance
  (versions, snapshot id, termination status, diagnostics,
  reproducibility metadata). A result without provenance is not
  eligible for operational consideration.
- A supervised process. The Rust supervisor (P7.18,
  `apps/worker/src/supervision.rs`) owns its lifecycle from the
  outside: per-call timeouts, bounded retries, crash isolation,
  backpressure, and readiness probing.

What a worker is NOT (the isolation rules — absolute):

1. **Never imports control-plane code.** There is no import path from a
   Python process to the Rust crates; the ADR-004 isolation IS this
   process boundary. A worker that reaches for platform internals is
   not a worker, it is a violation.
2. **Never holds state across requests** beyond caches declared in its
   own README. Requests are independent; the request id and
   idempotency key identify them, nothing else.
3. **Never writes to platform tables.** PostgreSQL is the source of
   truth (ADR-007); a worker has no database, no outbox, no audit
   access. Its only channel is the gRPC contract.
4. **Never has control-plane authority.** No `dispatch.authorize`, no
   `secret.read`, no `network.any`, no direct device-protocol access
   (§5.7 never-granted set; scope-freeze P-9). Health, config, and
   lifecycle come from the supervisor, never chosen by the worker.
5. **Never invents nondeterminism silently.** In deterministic mode
   (the default) the worker honours the request seed and places no
   wall-clock values in result payloads; if it cannot honour that, it
   says so in `DeterminismMetadata` (P7.42).

## 2. Layout

| File | Content |
|---|---|
| `contracts/worker_api.proto` | The versioned contract: service `FlexcoreWorkerService` (`Check`, `Execute`, `Cancel`, `Shutdown`), `contract_version` on every message, determinism fields (`seed`, `request_id`, `idempotency_key`) on the envelope |
| `worker_template/config.py` | Env-driven `WorkerConfig`, fail-closed defaults (invalid value = fatal; missing key = recorded DEFAULT) |
| `worker_template/health.py` | The closed health state machine: `STARTING → READY → DRAINING → STOPPING` (names mirror the kernel lifecycle vocabulary — the recorded consistency seam) |
| `worker_template/errors.py` | The structured error vocabulary (name-aligned with the proto `WorkerErrorCode`), refusal vs in-band classification, the gRPC status mapping (grpcio seam) |
| `worker_template/handler.py` | Envelope validation, correlation-id echo, determinism stamping, the `WorkerLogic` hook the concrete worker implements |
| `worker_template/server.py` | The ONLY grpcio touchpoint: `serve()` / `graceful_stop()` (UNVERIFIED) plus the pure `ShutdownSequence` (drain/grace decisions) |
| `worker_template/example_echo_worker.py` | A minimal concrete worker: deterministic SHA-256 payload echo; stdlib logic runnable today, `main()` needs grpcio |
| `tests/test_worker_template.py` | Written, NEVER RUN: pure-logic tests + import-guarded server tests |
| `worker_template/requirements.txt` | grpcio/protobuf pins **UNPINNED-REVERIFY** — deliberately un-installable until a human verifies versions |

## 3. The lifecycle

```
STARTING ──bound + configured──▶ READY ──shutdown requested──▶ DRAINING ──drained/grace──▶ STOPPING
    │                               │                                                        ▲
    └────────startup failed──────────┴──────────────immediate stop (idle)───────────────────┘
```

- Only `READY` accepts work (`accepts_work()`); `DRAINING`/`STOPPING`
  answer `EXECUTE_STATUS_UNAVAILABLE`.
- `DRAINING` never recovers to `READY` in-process: once a worker drains
  it is on its way out. Recovery is a **supervisor decision** (P7.18):
  a failed/drained worker returns to service only through an explicit
  policy-recorded reinstatement after a successful probe — never a
  silent restart.
- A crash (process death) is not a state here: it is observed by the
  supervisor, which marks the worker `FAILED` (mirroring the kernel
  lifecycle name) and degrades the pool (P7.18).

## 4. The contract (summary — the proto is the definition of record)

| RPC | Purpose | Notes |
|---|---|---|
| `Check` | Health/readiness probe | Cheap, side-effect free; maps to the P7.18 readiness probing and the P7.20 health aggregation |
| `Execute` | One payload in, one §9.2-shaped result out | Carries `contract_version`, `seed`, `request_id`, `idempotency_key`, `payload_type`, `correlation_id`, `scope` |
| `Cancel` | Cooperative cancellation by `request_id` | Best-effort; the supervisor's timeout is the authoritative backstop |
| `Shutdown` | Graceful drain-and-exit | The supervisor is the legitimate caller; the worker moves to `DRAINING` immediately |

Every message carries `contract_version`; a major-version mismatch is a
structured refusal (`UNSUPPORTED_CONTRACT_VERSION`), never a
best-effort parse. The determinism fields (`seed`, `request_id`) are
first-class so P7.42's seeded replay can key on them without schema
changes.

**Buf governance seam (recorded):** the proto lives here per the P7.17
file plan; the `proto/` buf module (P1.18) is the governance home —
activation registers this file with that module (or a documented
include path from it) so `buf lint` / `buf breaking` run on it in CI
(P7.27), before any generated stub is committed.

## 5. How a concrete worker uses the template

1. Implement `WorkerLogic.evaluate(request) -> (payload_bytes,
   provenance)` — the ONLY custom part. Refuse unknown payload types
   (`errors.unsupported_payload_type`); never approximate.
2. Build the config (`config.from_env`), the health machine, and the
   shutdown sequence; pass a `service_addition_cb` that registers the
   generated servicer into `serve()`.
3. Register the SIGTERM hook (`server.make_signal_hook`) so a
   supervisor `Shutdown` or a SIGTERM drains instead of kills.
4. Declare any cache in the worker's own README (isolation rule 2) and
   its `payload_type` strings in the worker contract table of the
   decision record.

The example (`example_echo_worker.py`) shows the whole shape with a
deterministic SHA-256 echo — the same-input-same-output property P7.42
replays.

## 6. Deployment requirement (verbatim, also raised by the code)

```text
flexcore worker template requires the grpcio deployment dependency:
install the pins recorded in python/workers/requirements.txt
(UNPINNED-REVERIFY) before running any worker
```

## 7. How the human build/test pass runs this

```sh
python -m venv .venv-workers            # never committed
# replace the UNPINNED-REVERIFY entries in requirements.txt with
# verified versions first (procedure in that file), then:
.venv-workers/Scripts/pip install -r python/workers/requirements.txt
.venv-workers/Scripts/pip install pytest   # record the version
# pure-logic suite (needs no grpcio):
PYTHONPATH=python/workers .venv-workers/Scripts/python -m pytest python/workers/tests/ -v
# buf gates (P1.18 governance, P7.27 CI):
buf lint proto/    # once the worker proto is registered with the module
buf breaking proto/ --against '.git#branch=main,subdir=proto'
```

The P7.17 acceptance checks that need a running stack (example worker
end-to-end, kill-mid-request containment, buf breaking detection) are
the human pass / P7.18 fault-injection verification, not this loop's.

## 8. Simulation-only standing note

Everything here is simulation/shadow infrastructure (v0.5.0). No field
read, no field write, no control authority, no safety or certification
implication (AGENTS.md §3; the ledger header). A worker's output is a
proposal that still passes the protected gates.
