# P11.11 — Idempotency-Key enforcement on command-submission POSTs

| Field | Value |
|---|---|
| Subtask | P11.11 (Idempotency-Key enforcement on command-submission routes) |
| Wave / agent | Phase 11 wave 1, Agent B |
| Status | WRITTEN-UNRUN: the middleware, state, wiring, and unit tests are written; nothing has been compiled, executed, or exercised against a live API (the first human build is the gate). |
| Sources | [S-61] (operational-practice context for idempotent API edges), [S-65] (at-least-once delivery motivation — JetStream); the P1.09 validated envelope key discipline (`crates/flexcore-events/src/envelope.rs`); the gap analysis §5 Tier 0 row 4 |
| Files touched | `apps/control-plane/src/idempotency.rs` (new), `apps/control-plane/src/state.rs` (shared, additive `idempotency` field), `apps/control-plane/src/lib.rs` (shared, additive module line), `apps/control-plane/src/main.rs` (shared, additive layer), `apps/control-plane/src/routes.rs` (additive `COMMAND_SUBMISSION_PATHS`) |

## 1. Scope decision (recorded, honest)

The §15.2 dispatch-command resource is READ-ONLY by recorded decision
(`routes_phase4.rs`: "NO execute-command endpoint exists") and adding
one is outside this subtask's authority (P4.30's own record reserves a
REST write path as a future subtask decision). The "command-submission"
HTTP class governed by this contract is therefore the set of POST routes
that SUBMIT candidates which can enter the command lifecycle — today
exactly the three Phase 3 candidate-submission routes in
`crate::routes::COMMAND_SUBMISSION_PATHS`:

| Governed path | What it submits |
|---|---|
| `POST /api/v1/schedules` | a schedule revision (CANDIDATE) |
| `POST /api/v1/flexibility/envelopes` | a flexibility-envelope record + events |
| `POST /api/v1/optimization-runs` | a §12.6 optimization-run record (CANDIDATE) |

The list is a closed constant. A later phase that exposes a real
command-submission POST adds its path to the list; both the P11.11
contract and the P11.14 in-flight caps then govern it with zero code
change. Nothing is silently opted in.

## 2. The implemented contract (as written)

| Situation | Behavior |
|---|---|
| Governed POST without the `Idempotency-Key` header | **422** `validation_rejected` — the key is mandatory |
| Key empty, whitespace-containing (the P1.09 envelope rule), or over the registered 255-byte bound | **422** `validation_rejected` |
| First request with a valid key | processed normally; the terminal response (any 2xx/4xx) is stored in the bounded replay cache |
| Retry with the SAME key and SAME request fingerprint (method + path + body) | the stored response is replayed with `Idempotent-Replay: true`; no second candidate is created |
| Same key while the first request is still in flight | **409** `idempotency_replay_conflict` (concurrent-duplicate semantics) |
| Same key with a DIFFERENT fingerprint | **409** `idempotency_replay_conflict` — a key is reusable only for retries of the SAME request |
| 5xx response | NOT stored — a server-side failure stays retryable |
| 429 response (P11.14) | NOT stored — a rate-limited request must be retryable, not pinned |
| Unauthenticated request | passes through untouched; the route handler's 401 stays authoritative (budgets and cache keys are per verified tenant) |

Key representation: the P1.09 validated envelope key type
(`flexcore_events::envelope::IdempotencyKey`) is REUSED as the
validator — non-empty, whitespace-free — never a second validation
implementation. Handler-level documentation of the same contract lives
on the middleware module header (`idempotency.rs`).

## 3. Recorded limitations (honest)

1. **Test-grade fingerprint.** The request fingerprint is a std-library
   64-bit hash (the same class as the P1.20 trace id) — NOT a
   cryptographic digest; a collision could mis-replay. The DURABLE
   idempotency of the platform never rests on this HTTP cache: the
   outbox event keys (P1.09) and the consumer oracle (P11.12) remain the
   guarantee. This middleware is the API-edge convenience layer.
2. **In-process cache.** The replay cache is per process and bounded
   (`MAX_REPLAY_ENTRIES` = 256, registered RECOMMENDATION; oldest-first
   eviction). Multi-instance deployments need a shared store — the
   Redis-backed path is RECORDED, NOT BUILT (the same scale-out note as
   P11.14).
3. **A's handoff note 3 (accept-latency wiring) NOT applied.** Agent A's
   close-out asks B to call
   `MetricsRegistry::record_command_accept_duration_millis` "around the
   committed PROPOSED record" on the command-submission route. No
   command-submission route exists (§1); wiring the command-accept
   histogram into schedule-proposal routes would measure the wrong
   thing. HONEST RESOLUTION: the recording API exists (A's wave 1);
   its route-side wiring waits for the phase that exposes the command
   submission surface — recorded as an open handoff back to the
   orchestrator, not silently mis-wired.
4. **Unit tests are written, never run** (`idempotency.rs` `#[cfg(test)]`:
   key-discipline equivalence and fingerprint separation). Their
   execution is the human gate.

## 4. Layer position

Mounted in `main.rs` between the P11.14 rate limiter (outer) and the
timeout layer (inner), after the outermost correlation middleware — so
422/409/429 responses are all counted in the existing
`flexcore_http_requests_total` series, and a 429 can never be cached as
a stored response (the two layers' store rules agree).
