# Class C — out-of-process gRPC workers

| Field | Value |
|---|---|
| Manifest `class` | `grpc-worker` |
| Entry point | `target` — the gRPC target URI (e.g. `https://forecast-worker.internal:9443`) |
| Contract | `python/workers/contracts/worker_api.proto`, package `flexcore.worker.v1`, service `FlexcoreWorkerService` |
| Template | `python/workers/worker_template/` (guide of record: `python/workers/README.md`) |
| Supervisor | the Rust supervisor (P7.18, `apps/worker/src/supervision.rs`) owns the lifecycle from the outside |

## 1. What a Class C plugin is

A separate process/container into which Python numerical/ML code is
isolated (ADR-004: Python is isolated to workers, never a backend), serving
one versioned gRPC/Protobuf contract. Its ONLY channel to the platform is
that contract: no database, no outbox, no audit access, no import path to
the Rust crates, no device-protocol access.

## 2. The worker contract (summary — the proto is the definition of record)

| RPC | Purpose | Notes |
|---|---|---|
| `Check` | health/readiness probe | cheap, side-effect free |
| `Execute` | one payload in, one §9.2-shaped result out | carries `contract_version`, `seed`, `request_id`, `idempotency_key`, `payload_type`, `correlation_id`, `scope` |
| `Cancel` | cooperative cancellation by `request_id` | best-effort; the supervisor's timeout is the authoritative backstop |
| `Shutdown` | graceful drain-and-exit | the supervisor is the legitimate caller |

Every message carries `contract_version`; a major-version mismatch is the
structured refusal `UNSUPPORTED_CONTRACT_VERSION`, never a best-effort
parse.

## 3. Using the worker template

The concrete worker implements ONE hook — `WorkerLogic.evaluate(request) ->
(payload_bytes, provenance)`:

1. Refuse unknown payload types
   (`errors.unsupported_payload_type`); never approximate.
2. Build the config (`config.from_env` — fail-closed defaults: an invalid
   value is fatal, a missing key is a recorded DEFAULT), the health
   machine (`STARTING → READY → DRAINING → STOPPING`; names mirror the
   kernel lifecycle vocabulary), and the shutdown sequence.
3. Pass a `service_addition_cb` that registers the GENERATED servicer into
   `serve()` — the generated import happens in the concrete worker, never
   in the template.
4. Register the SIGTERM hook (`server.make_signal_hook`) so a supervisor
   `Shutdown` or SIGTERM drains instead of kills.
5. Declare any cache in the worker's own README (isolation rule 2) and the
   accepted `payload_type` strings in the worker contract table.

The minimal concrete example is
`python/workers/worker_template/example_echo_worker.py` (a deterministic
SHA-256 payload echo); its walkthrough is
[examples/echo-worker/README.md](examples/echo-worker/README.md).

## 4. What a worker must never do (absolute)

1. **Never import control-plane code** — the process boundary IS the
   ADR-004 isolation.
2. **Never hold state across requests** beyond a cache declared in its own
   README; requests are identified by `request_id` + `idempotency_key`.
3. **Never write to platform tables** — PostgreSQL is the source of truth
   (ADR-007); the worker has no database access at all.
4. **Never hold control-plane authority** — no `dispatch.authorize`, no
   `secret.read`, no `network.any`, no direct device-protocol access (the
   §5.7 never-granted set; scope-freeze P-9). Health, config, and lifecycle
   come from the supervisor, never chosen by the worker.
5. **Never invent nondeterminism silently** — in deterministic mode (the
   default) the worker honours the request seed and places no wall-clock
   values in result payloads; if it cannot honour that, it says so in its
   determinism metadata (P7.42).

A worker's every output is a PROPOSAL (ADR-010): it enters the protected
candidate path like any other candidate.

## 5. Deployment requirement and run commands (PREPARED-UNRUN)

grpcio was never installed and no worker process was ever started in the
delivery loop (every grpcio call in `server.py` carries an UNVERIFIED
comment). The template raises the named deployment requirement when run
without the dependency:

```text
flexcore worker template requires the grpcio deployment dependency:
install the pins recorded in python/workers/requirements.txt
(UNPINNED-REVERIFY) before running any worker
```

The human build/test pass (from `python/workers/README.md` §7):

```sh
python -m venv .venv-workers            # never committed
# replace the UNPINNED-REVERIFY entries in requirements.txt with
# verified versions first (procedure in that file), then:
.venv-workers/Scripts/pip install -r python/workers/requirements.txt
.venv-workers/Scripts/pip install pytest   # record the version
# pure-logic suite (needs no grpcio):
PYTHONPATH=python/workers .venv-workers/Scripts/python -m pytest python/workers/tests/ -v
# buf gates (P1.18 governance, P7.27 CI):
buf lint proto/
buf breaking proto/ --against '.git#branch=main,subdir=proto'
```

Buf generation of `contracts/worker_api.proto` and the servicer wiring is
the FIRST human build step; until a recorded run exists, no claim that any
worker ever served a request is made anywhere.

## 6. Honest status

Everything in `python/workers/` is WRITTEN-UNRUN. The genuinely runnable
parts by code inspection are the stdlib-only pure-logic modules (`config`,
`health`, `errors`, `handler`, the echo worker's logic,
`server.ShutdownSequence`); the pytest suite is written, never executed.
The P7.17 acceptance checks that need a running stack (example worker
end-to-end, kill-mid-request containment, buf breaking detection) are the
human pass / P7.18 fault-injection verification.
