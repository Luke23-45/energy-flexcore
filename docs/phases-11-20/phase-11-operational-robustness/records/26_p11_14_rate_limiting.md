# P11.14 — Per-tenant token-bucket rate limiting + in-flight caps

| Field | Value |
|---|---|
| Subtask | P11.14 (per-tenant token-bucket rate limiting, 429/Retry-After contract, per-tenant in-flight command caps) [S-85] |
| Wave / agent | Phase 11 wave 1, Agent B |
| Status | WRITTEN-UNRUN: the middleware, threshold registration, wiring, and unit tests are written; nothing has been compiled, executed, or load-tested (the first human build is the gate; no load run exists). |
| Sources | [S-85] (Speakeasy API rate-limiting practice — the 429/Retry-After contract shape and per-principal budget framing; no claim about FlexCore); the gap analysis §5 Tier 0 row 6 |
| Files touched | `apps/control-plane/src/rate_limit.rs` (new), `apps/control-plane/src/state.rs` (shared, additive `rate_limit` field), `apps/control-plane/src/lib.rs` (shared, additive module line), `apps/control-plane/src/main.rs` (shared, additive layer) |

## 1. Principal classes BEFORE thresholds (the ordering rule)

Three classes, closed set, defined before any number:

| Class | Subject-prefix rule | Registered budget axis |
|---|---|---|
| `Operator` | everything else | refill 10 req/s, burst 40, in-flight cap 8 |
| `EdgeAgentUplink` | subject starts with `edge-agent:` or `edge:` | refill 50 req/s, burst 200, in-flight cap 64 |
| `Plugin` | subject starts with `plugin:` | refill 2 req/s, burst 10, in-flight cap 2 |

The `EdgeAgentUplink` class exists EXPLICITLY so the fleet uplink is
never starved by interactive-operator budgets (plan §5 risk 4) — its
budget is strictly larger on both axes, asserted by test. The
subject-prefix classification is an ASSUMPTION pending the real identity
integration (no production OIDC exists in 0.x; the recorded P1.17
status); `classify_principal` is the single amendment point.

## 2. The contract (as written)

| Situation | Behavior |
|---|---|
| Token bucket empty for the caller's (class, tenant) | **429** with `Retry-After` (whole seconds, ceil of the refill wait, minimum 1) and the stable envelope `rate_limited` [S-85 shape] |
| Per-tenant in-flight command-submission count over the class cap (on `COMMAND_SUBMISSION_PATHS` POSTs) | **429** with `Retry-After: 1` (poll-style; the cap releases as requests complete) — the [S-85] per-tenant in-flight cap practice |
| `/health`, `/ready`, `/metrics` | NEVER rate limited (probes and scraping must not depend on request budgets — recorded) |
| Unauthenticated request | passes through untouched; the route handler's 401 stays authoritative |
| In-flight guard | a Drop guard decrements the per-tenant counter on EVERY completion path including panic unwind; no unsafe anywhere in the module |

The in-flight caps reuse the same closed path list as P11.11
(`routes::COMMAND_SUBMISSION_PATHS` — the candidate-submission class;
see record 23 §1 for the scope rationale).

## 3. Thresholds are registered values, not tuned claims (X-8)

Every number in §1 is a REGISTERED RECOMMENDATION written down BEFORE
any run that could fill it, pending operator sign-off on real data. No
capacity, sufficiency, or performance claim attaches to any value; no
load test exists. The bucket-map and replay-cache bounds
(`MAX_TRACKED_BUCKETS` = 10,000; P11.11's `MAX_REPLAY_ENTRIES` = 256)
are registered the same way; eviction is oldest-first, never unbounded
growth.

## 4. Scale-out honesty

The limiter is IN-PROCESS and per-instance: with more than one control
plane instance the effective budget multiplies per instance. The
Redis-backed shared limiter is RECORDED as the multi-instance path —
deliberately NOT built here (no new dependency; the token bucket is
hand-rolled with `std::time::Instant` and mutexes, per the wave-1
dependency-contention rule). Single-instance is the recorded deployment
shape until the operator acts on this note.

## 5. Interaction with the other wave-1 layers

Mounted in `main.rs` between the correlation middleware (outermost, so
429s are counted in `flexcore_http_requests_total{status="429"}` — the
existing series, no new family) and the P11.11 idempotency middleware
(inner), so a 429 is never cached as a stored response. Rejection
counters exist on `RateLimitState::rejection_counters()` and (final
review, FRV1-R2-006 — the handoff note below is now closed) render on
`/metrics` under the families `flexcore_rate_limit_rejections_total` and
`flexcore_rate_limit_in_flight_rejections_total`, added to the P11.01
taxonomy record as its additive amendment (families 6 and 7) BEFORE the
families were implemented, per that record's §1 rule. The 429 series
above remains the scraped per-status signal.

## 6. Tests (written, never run)

`rate_limit.rs` `#[cfg(test)]`: the class set is closed and distinct;
the edge-agent budget is strictly larger than the operator budget; a
bucket takes its burst then rejects with Retry-After ≥ 1 s; buckets are
per (class, tenant); the in-flight gate admits to the cap, rejects over
it, and releases on drop.

## 7. Handoff notes

1. **Real subject formats**: when the identity integration lands,
   `classify_principal`'s prefix rules must be checked against the real
   subject grammar (the recorded ASSUMPTION).
2. **Redis-backed limiter**: build only when multi-instance becomes the
   deployment shape; the per-instance budget multiplication is the
   recorded interim behavior.
3. **P11.01 amendment**: if a dedicated rate-limit metric family is
   wanted on `/metrics`, amend A's taxonomy record first (the
   taxonomy's own rule), then render from `rejection_counters()`.
