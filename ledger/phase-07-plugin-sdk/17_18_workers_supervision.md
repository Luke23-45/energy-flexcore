# P7.17/P7.18 — Python gRPC worker template (Class C) and worker supervision

Date: 2026-11-13. Subtasks: P7.17, P7.18 (`ledger_phase_7.md` §P7.17,
§P7.18). Owner roles: Python engineer / Rust engineer. Report trace:
§5.4 Class C; §5.6; §5.7; §7.2; §7.4; §8.1 (`python/workers/`); §9.2;
§15.1; §19.2(6)(8); §21.3; §22.1 (ADR-004, ADR-006, ADR-008, ADR-010).
Inputs: P7.09 (`09_wit_worlds.md`, the data vocabulary); P7.02
(`02_scope_freeze.md`, prohibitions P-4/P-6/P-8/P-9); P7.04
(`04_seam_inventory.md`, the worker reality); RESEARCH.md (§2, the
grpcio pin question); PLAN.md §3 (ADR-004/ADR-008); the P5.15 worker
reality (`apps/worker/python/grid_worker.py`,
`crates/flexcore-grid-api/src/worker_backend.rs`); the kernel lifecycle
vocabulary (`crates/flexcore-kernel/src/lifecycle.rs` — read, NOT
modified).

## 0. Honest status — READ FIRST

**Everything in this wave is WRITTEN-UNRUN.** No grpcio was installed,
no Python worker process was ever started, no `buf lint`/`buf breaking`
ever ran against the proto, no cargo command (build/test/clippy/fmt)
was executed, and both test suites (`python/workers/tests/
test_worker_template.py`, `apps/worker/tests/supervision_p7_18.rs`)
are WRITTEN-UNRUN. The genuinely runnable-by-inspection surface is the
stdlib-only pure-logic Python modules and the std-only Rust supervision
module; every grpcio call carries an `UNVERIFIED — grpcio API` comment.
The first human build/test pass (procedure in `python/workers/README.md`
§7 and the requirements header) is the verification gate for everything
below. No claim in this record is a tool-verified fact until then.

## 1. Per-subtask coverage

| Subtask step | Delivered | Where | Status |
|---|---|---|---|
| P7.17 step 1 (template: contract, config, logging stance, health, graceful shutdown) | The template package: env config with fail-closed defaults, the closed health state machine, structured errors, correlation echo, the `WorkerLogic` hook, the gRPC server skeleton (grpcio seam), the pure `ShutdownSequence`, and a stdlib-runnable example worker | `python/workers/worker_template/{__init__,config,health,errors,handler,server}.py`, `example_echo_worker.py` | Written-unrun (grpcio parts UNVERIFIED; pure parts runnable) |
| P7.17 step 2 (worker contract carrying §9.2 result requirements) | `flexcore.worker.v1` proto: `ExecuteRequest`/`ExecuteResponse` with `contract_version` on EVERY message, `ResultProvenance` (§9.2 versions/snapshot/termination/diagnostics/execution metadata), `DeterminismMetadata` (seed, request_id, idempotency_key, deterministic, seed_supported) | `python/workers/contracts/worker_api.proto` | Written-unlinted; buf-governance registration recorded (§4) |
| P7.17 step 3 (isolation: own process, crash contained, reported as Failed) | Isolation rules fixed in writing (template README §1, five absolute rules) and structurally enforced (a Python process has no import path into the Rust control plane); crash containment is the P7.18 supervisor's `record_crash` → FAILED → degraded pool | `python/workers/README.md` §1; `apps/worker/src/supervision.rs` | Logic written-unrun; containment behaviour is the P7.18 fault-injection verification (ledger), not evidenced here |
| P7.17 step 4 (`buf lint` + `buf breaking` in CI) | NOT DONE in this loop — no buf tooling was executed. The proto is written to the `proto/` module's STANDARD lint shape; registration with the `proto/` buf module (P1.18 governance) and the CI wiring are recorded as the activation steps (P7.27 owns the CI gate) | `proto/README.md` (P1.18); §4 below | Recorded, not run |
| P7.18 step 1 (per-contract timeouts; recorded failure, not a hung schedule) | `SupervisionPolicy.call_timeout_ms` (DEFAULT 30 000), `CallDeadline` computed from the injected `Clock`, `record_timeout` → audited `FailureRecorded` + bounded retry decision | `apps/worker/src/supervision.rs` | Written-unrun |
| P7.18 step 2 (bounded retries + backoff; idempotency keys; no retry of non-idempotent calls) | `CallKind::{Idempotent, NonIdempotent}` — a non-idempotent call is NEVER retried regardless of any worker hint; `RetryScheduler` = `min(base·2^attempt, cap) + seeded jitter`, a pure function of (seed, attempt); an `Idempotent` call without a key is refused fail-closed | `supervision.rs` (`RetryScheduler`, `record_failure`) | Written-unrun |
| P7.18 step 3 (backpressure: visible depth or recorded refusal — never silent) | Bounded in-flight cap per worker (DEFAULT 4, mirroring the template's `DEFAULT_MAX_INFLIGHT`); beyond it, `acquire` REFUSES with the named `BackpressureRefused` error and an audited `BackpressureRefused` event carrying the depth; work is never queued unboundedly | `supervision.rs` (`WorkerPool::acquire`) | Written-unrun |
| P7.18 step 4 (tests: timeout, crash mid-request, restart, saturation) | The integration suite covers all four fault modes plus the no-retry rule, the recovery gate, the threshold, the audit ordering, and audit-or-refuse | `apps/worker/tests/supervision_p7_18.rs` | Written-unrun |

Ledger acceptance items that need a running stack and are NOT evidenced
here (recorded, not hidden): the P7.17 kill-mid-request containment run,
`buf lint`/`buf breaking` execution and the detected-breaking-change
demonstration, and the P7.18 fault-injection run against the example
worker with queue-depth metrics. These remain the human build pass /
P7.26–P7.28 owners.

## 2. Placement decisions (recorded)

| Decision | Choice | Basis |
|---|---|---|
| Where the supervisor lands | The EXISTING worker-serving app `apps/worker` (`flexcore-worker`), additive module `src/supervision.rs` + integration test `tests/supervision_p7_18.rs`; lib.rs module registration only. **No new crate, no root `Cargo.toml` change** | The P7.18 ledger hint names `apps/worker/src/`; `apps/worker` is the background-process composition root (P1.19); a new crate would violate the minimal-edit rule and the dependency-direction review this loop cannot run |
| Where the worker proto lives | `python/workers/contracts/worker_api.proto` (the P7.17 file plan), with the `proto/` buf module (P1.18) recorded as the governance home: activation registers the file with that module (or a documented include path) so lint/breaking run in CI | PLAN.md §4 says ".proto under `proto/`"; the P7.17 file plan puts the template under `python/workers/`. The seam is recorded rather than silently chosen; until registration, the file is a design artifact, not a governed schema |
| Why the audit port is in-module | `SupervisionAuditSink` is defined in `supervision.rs`, shaped after `flexcore-audit::sink::AuditSink` (append-only, `Result`-returning `record`), because adding a `flexcore-audit` dependency would require a `Cargo.toml` change outside this loop's edit scope. Promotion is mechanical; the decision record IS the seam note | audit-or-refuse preserved either way: a failing sink refuses the supervised action |
| Kernel lifecycle | READ only. The worker state names (READY/DRAINING/FAILED; STARTING/STOPPING endpoints) MIRROR `crates/flexcore-kernel/src/lifecycle.rs::LifecycleState` via the recorded constant `LIFECYCLE_CONSISTENCY_SEAM`; the kernel state machine is neither implemented nor modified here. A worker plugin's `Failed` lifecycle transition remains the kernel's (P7.07), reported through the lifecycle registry — this module supervises processes, not plugin lifecycle | Task boundary (kernel owned by another agent); consistency without coupling |

## 3. The proto contract table (`flexcore.worker.v1`)

Service `FlexcoreWorkerService` — four rpcs. Every message carries
`contract_version`; major-version mismatch is the structured refusal
`UNSUPPORTED_CONTRACT_VERSION`, never a best-effort parse.

| RPC | Request | Response | Notes |
|---|---|---|---|
| `Check` | `HealthCheckRequest` | `HealthCheckResponse` (`state`, `ready`, `detail`) | Maps to the template health state machine; the P7.18 probe and the P7.20 aggregation consume it |
| `Execute` | `ExecuteRequest` (determinism block, `payload_type`, payload, snapshot id, deadline, correlation id, scope tags) | `ExecuteResponse` (correlation/request echo, `ExecuteStatus`, payload, `ResultProvenance`, determinism echo, `WorkerErrorDetail`) | The §9.2 envelope; payload bytes are typed by `payload_type` agreement, never redefined here |
| `Cancel` | `CancelRequest` (request_id, correlation, reason) | `CancelResponse` (accepted, detail) | Cooperative; the supervisor's timeout is the authoritative backstop |
| `Shutdown` | `ShutdownRequest` (grace_ms, reason) | `ShutdownResponse` (accepted) | Worker enters DRAINING immediately; the supervisor is the legitimate caller |

Envelope vocabulary (each mirrors a named platform concept; no second
shape is minted):

| Field | Mirrors |
|---|---|
| `DeterminismMetadata.seed` / `request_id` | P7.42 seeded replay keys; §9.2 reproducibility metadata |
| `DeterminismMetadata.idempotency_key` | P7.18 retry contract (`CallKind::Idempotent` on the Rust side) |
| `ResultProvenance` | §9.2 result requirements; the WIT result vocabulary (`wit/flexcore-optimizer@1.0.0`) |
| `correlation_id` | §21.3 correlation; echoed on every response (template `handler.py` enforces presence) |
| `scope` tags | §5.7 tenant/site/asset scoping — recorded by the worker, ENFORCED by the host/supervisor, never by the worker |
| `WorkerErrorCode` | `worker_template/errors.py::WORKER_ERROR_CODES` — name-aligned, pinned by the template test that parses the proto |

Consistency with the existing worker reality (not a duplicate): the
P5.15 pandapower worker speaks one-JSON-per-stdin/stdout behind
`WorkerProcessBackend` (`crates/flexcore-grid-api/src/worker_backend.rs`)
with the `flexcore-grid-worker/1` tag, in-band failure results, and a
recorded NO_RETRY_POLICY. This template is the GENERIC Class C shape a
new worker starts from; it does not re-wrap the grid worker. The grid
worker's eventual migration onto `flexcore.worker.v1` (or its
continuation as the process backend) is a future decision, not made
here; its fail-closed and no-retry disciplines are preserved
conceptually by the supervisor's rules.

## 4. Isolation rules (template README §1, absolute)

1. The worker NEVER imports control-plane code — the process boundary
   IS the ADR-004 isolation; there is no import path from Python to the
   Rust crates.
2. The worker NEVER holds state across requests beyond caches declared
   in its own README.
3. The worker NEVER writes to platform tables (ADR-007; scope-freeze
   P-8) — its only channel is the gRPC contract.
4. The worker NEVER has control-plane authority: no `dispatch.authorize`,
   no `secret.read`, no `network.any`, no direct device-protocol access
   (§5.7; scope-freeze P-9); control-authority mode is never selectable
   by a worker or plugin (§21.4).
5. The worker NEVER invents nondeterminism silently: in deterministic
   mode (the default) it honours the seed and excludes wall-clock
   values from result payloads, and admits any limitation in
   `DeterminismMetadata` (P7.42).

Config is env-driven with fail-closed defaults (invalid value = fatal,
missing key = recorded DEFAULT; loopback bind by DEFAULT; no secret
ever passes through the config keys — secrets are resolved by reference
per the P7.19 discipline).

## 5. The supervision policy (defaults are decisions, marked DEFAULT)

| Parameter | DEFAULT | Recorded rationale |
|---|---|---|
| `call_timeout_ms` | 30 000 | Same magnitude as the kernel's `DEFAULT_DRAIN_DEADLINE_MS` and the template's `DEFAULT_DRAIN_GRACE_MS` — a coincidence of defaults, recorded as such, not a shared constant. Per-contract overrides come from the manifest `resources.timeoutMs` at the composition layer that builds the policy |
| `max_retries` | 2 (3 attempts total) | Bounded amplification; hard-capped at 8 by `validate` — the ledger risk "unbounded retries amplifying a failure" is structurally refused |
| `backoff_base_ms` / `backoff_cap_ms` | 250 / 5 000 | Exponential growth stops at the cap; only deterministic jitter follows |
| `jitter_cap_ms` / `seed` | 250 / `0x464C_5843_5037_3138` ("FLXP718") | Jitter is a pure function of (seed, attempt) — xorshift64 — never the wall clock, so a P7.42 replay reproduces the schedule exactly |
| `inflight_cap` | 4 | Mirrors the template's `DEFAULT_MAX_INFLIGHT`; beyond it, refusal (never unbounded queueing) |
| `failure_threshold` | 3 consecutive | Timeouts/contract violations accumulate; a CRASH bypasses the threshold and marks FAILED immediately |
| `probe_interval_ms` | 5 000 | The `Check` rpc cadence; readiness = the READY state only |

Named behaviours (each an explicit refusal, never a silent one):
`BackpressureRefused` (work refused, not queued), `WorkerNotReady`
(state named), `MissingIdempotencyKey` (retry contract unmet — fail
closed), `RecoveryNotAuthorized` (no silent restart), `AuditUnavailable`
(audit-or-refuse: a broken sink refuses the supervised action),
`InvalidPolicy` (fail-closed validation), `ClockOutOfRange` (no silent
timestamp truncation).

Recovery rule: a FAILED worker returns to service ONLY via
`recover_on_probe_success(worker, policy_ref, …)` — a successful probe
AND a non-empty policy record authorizing the restart — then
STARTING → `mark_ready` → READY. A probe success alone changes nothing.

## 6. Dependency status (D4 discipline; the P7.03 record stays closed)

- **grpcio / protobuf: UNPINNED-REVERIFY.** RESEARCH.md records no
  grpcio/protobuf findings, so per the no-hallucinated-facts rule NO
  version is claimed anywhere. `python/workers/requirements.txt` carries
  the deliberately un-installable `==UNPINNED-REVERIFY` entries and the
  re-verify procedure (check PyPI at activation, record date + versions
  here and in the requirements header, confirm the grpc/protobuf
  compatibility table, install only into a scratch venv).
- No Rust dependency was added (the supervisor is std + the existing
  `flexcore-ports` Clock); the future gRPC client is the recorded
  deployment-stage seam (`TransportOutcome` documents the outcome
  contract; marked UNVERIFIED).
- This record does NOT extend `03_sdk_license_audit.md` (P7.03 is
  closed); the audit rows for the verified grpcio/protobuf versions are
  the activation-time human pass, recorded HERE when they exist.

## 7. ADR statements (verbatim positions, restated for this wave)

- **ADR-004:** Python is isolated to scientific/numerical/research
  workers. This template is the isolation mechanism and MUST NOT become
  a second general-purpose Python backend (§7.2; scope-freeze P-6). The
  template contains no backend capability: no database, no HTTP API, no
  scheduling logic — a worker computes and answers, nothing else.
- **ADR-008:** Worker contracts are Protobuf/gRPC (`flexcore.worker.v1`);
  plugin management remains REST/OpenAPI. The proto is born under the
  P1.18 buf governance (lint + breaking in CI at activation).
- **ADR-010:** Every worker output is a PROPOSAL. The template structurally
  cannot dispatch (the contract has no command path; the response is a
  §9.2 candidate), and the supervisor refuses rather than relays
  authority. A dispatch-bypassing worker is unbuildable on this
  contract, not merely discouraged.
- **ADR-006 (unchanged):** no dylib loading anywhere in this wave; the
  worker seam is a process boundary, not a library boundary.

## 8. Verification status (what was and was not run)

| Check | Status |
|---|---|
| `python tools/verify_docs.py` | Run at the end of this loop; actual output recorded in the completion report |
| `python tools/validate_agent.py` | Run; actual output recorded |
| `python -I tools/check_dependency_rule.py` | Run; actual output recorded (workspace unchanged except the additive module in `apps/worker`, an existing member) |
| Python syntax sanity (`python -I -m py_compile` on the new files, stdlib-only) | Run; actual output recorded |
| grpcio-dependent paths (`server.serve`, `graceful_stop`, `map_error_to_grpc_status`) | UNVERIFIED — never executed; every call site carries the marker |
| `buf lint` / `buf breaking` | NOT RUN — no buf tooling in this loop; P7.17 step 4 recorded as the activation step |
| `cargo build/test/clippy/fmt` | NOT RUN — banned in this loop; both Rust files are written-unrun |

## 9. Open items carried (each owned, none silent)

1. **Buf registration + first lint/breaking run** — the human build
   pass / P7.27 CI wiring; the proto file header records the seam.
2. **Generated-stub wiring** — the `service_addition_cb` in
   `example_echo_worker.py` names the exact registration point; the
   generated import belongs to the concrete worker, never the template.
3. **Real gRPC transport** — the deployment-stage client that implements
   `TransportOutcome` and drives `WorkerPool`; no client crate was added.
4. **Audit promotion** — `SupervisionAuditSink` → `flexcore-audit`
   (mechanical, shape-mirrored) at the first `Cargo.toml` edit window.
5. **P7.42 replay** — the determinism fields (`seed`, `request_id`,
   idempotency key, deterministic flag) are in place on both sides; the
   replay record itself is P7.42's deliverable.
6. **Supervision metrics surfacing** — `timeouts_recorded`,
   `backpressure_refusals`, `crashes_recorded` exist as counters; the
   P7.20 observability wiring consumes them.
