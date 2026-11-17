# P11.21 — Health-check expansion (readiness/liveness per service, dependency health, worker `/metrics`)

| Field | Value |
|---|---|
| Subtask | P11.21 (health-check expansion — readiness/liveness semantics per Compose service; dependency health surfacing in the `/health` vs `/ready` pair; semantics documented per service) |
| Wave / agent | Phase 11 wave 2, Agent B |
| Status | WRITTEN-UNRUN: the probe handlers, the worker `/metrics` route, and the tests are written code that has never been compiled or executed. The reachability logic is written against interfaces that have never executed; every dependency probe is recorded UNRUN. |
| Sources | The P1.22 control-plane probe baseline and the P1.19 worker probe baseline (the semantics this subtask extends, never regresses); the P11.12 record's handoff (`records/24_p11_12_command_consumer_idempotency.md` §5.1 — `CommandConsumerMetrics::render()` named as the worker `/metrics` seam) and the wave-1 close-out (record 27 §5.2); [S-61] (the SRE context for the liveness/readiness distinction — the P1.22 baseline already cites it; no new external claim is made here) |
| Files touched | `apps/control-plane/src/probes.rs`, `apps/worker/src/probes.rs`, `apps/worker/src/main.rs` (minimal marked composition edits) |

## 1. What was implemented (written, never run)

### 1.1 The per-service semantics table (the P11.21 deliverable)

The semantics are now a documented contract in BOTH probe modules
(module headers), reproduced here:

| Service | `/health` (liveness) | `/ready` (readiness) | `/metrics` |
|---|---|---|---|
| `flexcore-control-plane` | always 200 while the process runs — restarting cannot fix a dependency | 503 when a named dependency check fails (`database`, `broker`) or the process is shutting down; the body names EACH check's result | the P1.20/P2.21/P11.01 registry plus `flexcore_commands_stuck` (P11.20) |
| `flexcore-worker` | always 200 while the process runs | 503 when Postgres is unreachable (one-round-trip probe query), the NATS client is disconnected, or the worker is draining; the body names each dependency | the `flexcore_command_consumer_*` families (P11.12), exposed by this subtask |
| PostgreSQL / NATS / the P11.06 OTel Collector | their OWN Compose healthchecks — they are dependencies here, not probe hosts; this phase documents the two FlexCore application services only | | |

The liveness/readiness distinction is UNCHANGED from P1.19/P1.22: a
dependency blip fails readiness while liveness stays up, so an
orchestrator never restart-loops a healthy process over a stopped broker.

### 1.2 `apps/control-plane/src/probes.rs` (extend, never regress)

- `Readiness::evaluate_detailed()` — NEW: returns EVERY named check's
  result (`(name, Result<(), String>)`), passing checks included, plus
  the synthetic process-level shutdown entry. `evaluate()` is REBUILT ON
  it (same failure list as before — the P1.22 semantics are unchanged;
  a regressed `evaluate` would have been a regression, so it is
  unit-tested against `evaluate_detailed`).
- `Readiness::check_names()` — NEW: the registered check names (the
  per-service documentation and the `/ready` body derive from the same
  registration).
- The `/ready` handler body now additionally carries
  `checks: {<name>: "ok" | "failed: <error>"}` alongside the unchanged
  `failures` list. Status codes (200/503) and the failure semantics are
  exactly the P1.22 ones — the body only stops being empty (additive).
- Unit tests written complete and UNRUN (`#[tokio::test]`): detailed
  evaluation names every check and matches `evaluate`; the shutdown gate
  appears as the synthetic first entry; `check_names` matches the
  registration.

### 1.3 `apps/worker/src/probes.rs` (extend, never regress)

- **The worker `/metrics` route (the recorded P11.12 handoff, closed)**:
  `GET /metrics` renders `CommandConsumerMetrics::render()` — exactly the
  function the P11.12 record §5.1 and the wave-1 close-out §5.2 named as
  the wiring seam — under the same
  `text/plain; version=0.0.4; charset=utf-8` content type the control
  plane serves. Zero values always render (the always-render discipline).
  This is the worker's FIRST metrics surface: the P11.12 duplicate-
  delivery families (`flexcore_command_consumer_*`) become scrapeable
  (as written code) instead of idling unrendered.
- `ProbeState::new` takes the `Arc<CommandConsumerMetrics>` — the SAME
  Arc the consumer task drives (wired in `apps/worker/src/main.rs`),
  so the rendered counters are live, not a snapshot.
- The `/ready` response body now names each dependency:
  `{"status": "ready"|"not_ready", "draining": bool,
  "dependencies": {"postgres": "up"|"down", "nats": "up"|"down"}}`.
  Status codes unchanged (200/503); the underlying Postgres error stays
  in the logs, never in a response body (the P1.19 rule, kept).
- No new dependency check was invented: Postgres reachability (one-round-
  trip query) and NATS connection state were already the P1.19 checks;
  they are now VISIBLE per-dependency instead of collapsing into one
  status code.

### 1.4 `apps/worker/src/main.rs` (minimal marked composition edits)

- The `CommandConsumerMetrics` Arc is created BEFORE the probe state and
  shared with both the probe server (`ProbeState::new`) and the consumer
  task (`run_command_consumer`) — a two-line reorder plus the new
  argument, marked P11.21 with the record citation. The P11.12 comment's
  "no worker /metrics endpoint exists today" limitation note is updated
  to record the closure of that handoff.

## 2. Semantics that were deliberately NOT changed

1. Liveness never consults dependencies (P1.19/P1.22) — a stopped broker
   must not trigger worker restarts.
2. Readiness fails during drain/shutdown before the process exits.
3. The control plane's readiness check set is unchanged (`database`:
   one real round trip via a unit-of-work open+commit; `broker`: one
   core request to a subject outside every stream filter, where "no
   responders" IS the healthy outcome).
4. No Kubernetes probe contract is introduced (Kubernetes is Phase 20;
   the plan's boundary §5.2). The Compose services consume these HTTP
   endpoints only as written.

## 3. ASSUMPTION / UNRUN labels

- **UNRUN**: every dependency probe (the Postgres round trip, the NATS
  connection-state read, the broker health request) is written against
  interfaces that have never executed; the reachability logic is
  recorded UNRUN.
- **UNRUN**: the new probe unit tests (control plane) execute only at
  the first human `cargo test`.
- **UNRUN**: the worker `/metrics` body has never been scraped; the
  rendered families are written code, never observed output.
- **ASSUMPTION**: `axum` 0.8 `HeaderMap`/`IntoResponse` handler shapes
  compile as written — verified only at the first human build (the
  program-level recorded risk).

## 4. Handoff notes

1. **Orchestrator**: the ledger P11.21 tick cites this record; the tick
   is the orchestrator's action (program conventions §2.2).
2. **Operator / P11.09**: once the Compose scrape config targets the
   worker's probe port, the `flexcore_command_consumer_*` families
   appear in the metrics store; dashboards for them remain the recorded
   amendment handoff from record 24 §5.2 (NOT built here — no panel may
   invent a metric, and no taxonomy amendment was filed for a worker
   dashboard in this subtask).
3. **First human build**: the `ProbeState::new` signature change and the
   JSON readiness bodies join the VERIFY-BUILD watchlist.
