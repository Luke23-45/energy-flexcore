# P11.01 — Golden-signal metric taxonomy and instrumentation plan

| Field | Value |
|---|---|
| Subtask | P11.01 (golden-signal metric taxonomy + instrumentation plan) |
| Wave / agent | Phase 11 wave 1, Agent A |
| Status | WRITTEN-UNRUN: this record names the taxonomy FIRST (the P2.21 recorded-cardinality discipline carried forward); the code below is written exactly to this naming and has never been compiled or executed. |
| Sources | [S-61] (Google SRE Workbook, alerting on SLOs — golden signals and SLI shapes); the frozen 14-state / 27-edge command lifecycle (`crates/flexcore-domain/src/command_state.rs`); the P2.21 recorded-cardinality precedent (`ledger/phase-02-asset-telemetry/21_22_observability_security.md`) |
| Files touched | `apps/control-plane/src/metrics.rs` (implementation, after this record) |

## 1. Purpose and ordering discipline

The taxonomy is NAMED HERE before any family is added to the registry. The
P2.21 rule carries forward unchanged: cardinality is decided ONCE, per
family, and the implementation may contain only what this record names. If
a later subtask needs a family not listed here, it amends this record
first. No dashboard (P11.09), alert rule (P11.03), or SLO (P11.02) may
reference a family that is not in this record.

The golden-signal grouping (latency / traffic / errors / saturation per
user-facing service) follows [S-61]. Two user-facing services are named for
Phase 11:

| Service | Definition | Golden signals available |
|---|---|---|
| `control-plane-api` | The §15.2 operator-facing HTTP API served by the Axum control plane | latency (new histogram), traffic (existing request counter), errors (existing error counter), saturation (new in-flight gauge) |
| `command-lifecycle` | The §15.2 dispatch-command pipeline over the frozen 14-state lifecycle | accept latency (new histogram), terminal-state ratio (derived from the new transition counter), stuck-state count (gauge named here; implemented in wave 2, P11.20) |

The worker ingestion families of P2.21 (queue depth, delay summary,
quarantine, quality states) are ALREADY part of the registry and are
referenced by name only — they are not re-declared here.

## 2. The named families (the complete Phase 11 wave 1 inventory)

Every family renders on the control plane's `/metrics` (the P1.20 registry,
extended — not replaced), always, zero values included, so a scrape exposes
the full inventory on an idle system.

| # | Metric family | Type | Labels | Golden signal / SLI | Cardinality decision (DECIDED ONCE) |
|---|---|---|---|---|---|
| 1 | `flexcore_api_request_duration_milliseconds` | histogram (fixed buckets, `_bucket{le}` cumulative + `_sum` + `_count`) | none | control-plane-api latency SLI | NO route or method label. The per-route latency breakdown travels in the per-request JSON correlation log line (P1.20), never as labels — the same decision the P2.21 delay summary made. A route label would be bounded by the matched-pattern set today but the pattern set grows every phase; service-level SLI math does not need it. Buckets are a FIXED registered set (ms): 1, 5, 10, 25, 50, 100, 250, 500, 1000, 2500, 5000, 10000. Changing the bucket set is a recorded amendment to this table (X-8: the buckets are registered before any run that could fill them). |
| 2 | `flexcore_api_in_flight_requests` | gauge | none | control-plane-api saturation | No labels: one in-process gauge for the current number of requests inside the middleware. Per-route in-flight counts are not exposed (they need a second label axis and answer a debugging question the correlation logs already answer). This is an honest in-process saturation signal; database-pool and broker saturation are NOT exposed by this family (the pool lives behind the synchronous adapter and has no observable counter surface in 0.x — recorded as a known gap, not faked). |
| 3 | `flexcore_command_accept_duration_milliseconds` | histogram (same fixed bucket set as family 1) | none | command-lifecycle accept-latency SLI | No labels. Accept latency is the wall-clock time from the arrival of the command-submission request to the command's PROPOSED record being committed (the lifecycle's first state). Recorded on the command-submission path. No per-tenant label: tenant-correlated accept-latency questions go through the correlation logs (tenant_id is in every log line per P11.07). |
| 4 | `flexcore_command_state_transitions_total` | counter | `from`, `to` (both from the frozen 14-state vocabulary; only the 27 legal frozen edges are counted) | command-lifecycle SLIs: terminal-state ratio (derived), per-gate throughput | The label pair is BOUNDED BY CONSTRUCTION: `crates/flexcore-domain/src/command_state.rs` freezes the 14 states and the 27 legal edges; an illegal edge is a vocabulary violation, never a series. No span, event, or metric may invent a state name — the label values come from `CommandState::name()` only. Terminal-state ratio is a PromQL derivation (`transitions{to="<terminal>"}` over total transitions), not a second metric — no derived ratio is stored. |
| 5 | `flexcore_commands_stuck` | gauge | none | command-lifecycle stuck-state count | No labels: the gauge counts commands parked in a non-terminal state beyond the registered stuck threshold (FAILED and EXPIRED included — they exit only via FALLBACK). Per-state stuck breakdowns go to logs/queries, not labels. NAMED HERE, implemented in wave 2 (P11.20, `apps/control-plane/src/metrics.rs` + `crates/flexcore-domain/src/command_state.rs` — sequential after wave 1, no concurrency). Wave 1 does NOT add this family to code. |
| 6 | `flexcore_rate_limit_rejections_total` | counter | none | control-plane-api errors (the P11.14 token-bucket 429s, per instance) | AMENDMENT (final review, FRV1-R2-006 — added per this record's §1 rule "a later subtask amends this record first", closing the P11.14 record §5 handoff note): no labels — the rejection counters on `RateLimitState` are plain totals; per-class breakdowns travel in the correlation log line, never as a label axis. The 429 responses remain counted in the existing `flexcore_http_requests_total{status="429"}` series by the outermost middleware; this family names the limiter's own rejections explicitly. Rendered by `RateLimitState::render_rejection_metrics` appended to the single `/metrics` body (no second metrics surface). |
| 7 | `flexcore_rate_limit_in_flight_rejections_total` | counter | none | control-plane-api errors (the P11.14 per-tenant in-flight-cap 429s on the command-submission POST class) | AMENDMENT (same FRV1-R2-006 additive edit as family 6): no labels — per-tenant breakdowns go to the correlation logs (P1.07), never as labels. |

## 3. Instrumentation plan (who records what, and where)

| Family | Recording call site | Owner of the call site | Wave 1 status |
|---|---|---|---|
| 1, 2 | `track_metrics` middleware (`apps/control-plane/src/metrics.rs`) — the middleware already wraps every response; it now also records the request duration into the histogram and maintains the in-flight gauge | Agent A (metrics.rs is Agent A's) | Implemented in wave 1 |
| 3 | The command-submission route handler, around the unit of work that commits the PROPOSED record | Agent B (command route files are B's wave-1 ownership) | Recording API implemented; route-side wiring is a HANDOFF NOTE to B (§5) |
| 4 | The command consumer's lifecycle-transition commit path (every legal transition increments `from`,`to` exactly once per transition) | Agent B (command consumer under `apps/worker/src/` is B's) | Recording API implemented; consumer-side wiring is a HANDOFF NOTE to B (§5) |
| 5 | The P11.20 stuck-state scanner (wave 2) | Wave 2 Agent B | Not implemented in wave 1 |

## 4. Names Agent B's P11.03 alert rules bind to (P11.03 handoff input)

P11.03 (multi-window multi-burn-rate Prometheus rule files, [S-61]) binds
to SLI expressions over the families above. The precise names, so B's rule
files reference exactly these strings:

- `flexcore_api_request_duration_milliseconds` — latency SLI numerator uses
  `_bucket{le="1000"}` / `_count` (the 1000 ms bucket is the registered
  good-event threshold candidate for the API; the SLO register P11.02
  carries the RECOMMENDATION label).
- `flexcore_http_errors_total` — API error SLI (existing P1.20 family,
  unchanged name).
- `flexcore_http_requests_total` — traffic (existing P1.20 family;
  rate-of-requests for burn denominators).
- `flexcore_command_accept_duration_milliseconds` — accept-latency SLI,
  same bucket form (the registered threshold candidate is `le="500"`).
- `flexcore_command_state_transitions_total` — terminal-state-ratio and
  per-gate signals.
- `flexcore_commands_stuck` — the stuck-state alarm input (family exists
  from wave 2; until it renders, any rule referencing it must be written
  but cannot fire — recorded honestly in B's record).

The burn-rate window pattern (14.4x over 1h+5m pages, 6x over 6h+30m
pages, 1x over 3d tickets) is B's implementation; this record fixes only
the metric names and the SLI shapes they compute over.

## 5. Handoff notes to Agent B (wave 1)

1. Record family 3 (`flexcore_command_accept_duration_milliseconds`) on the
   command-submission path: `MetricsRegistry::record_command_accept_duration_millis(u64)`
   exists after wave 1; call it with the elapsed milliseconds from request
   arrival to the committed PROPOSED record.
2. Record family 4 on the command consumer's committed transitions:
   `MetricsRegistry::record_command_state_transition(&str, &str)` exists
   after wave 1 and IGNORES illegal edges (deny-by-default, same as the
   P2.21 quality-state recorder); pass the `CommandState::name()` values of
   the transition the consumer committed. The consumer owns a second
   registry instance question — the worker process has no `/metrics`
   surface today (recorded P2.21 limitation), so until a worker metrics
   surface exists, family 4 renders on the control-plane registry only if
   the control plane records its own lifecycle transitions (the Phase 4
   command routes). This is an honest recorded limitation: the wave-1
   record does not silently promise worker-side rendering.
3. P11.03 rule files: use the §4 names verbatim; no rule may reference a
   family absent from this record.

## 6. Honesty boundary

- Every statement above describes WRITTEN code and recorded decisions;
  nothing has been compiled, rendered, or scraped (the 0–10 base and this
  phase are WRITTEN-UNRUN). The first human build is the verification gate.
- The 1000 ms / 500 ms bucket thresholds named in §4 are RECOMMENDATIONS
  registered before any run (X-8); no observed latency distribution exists.
- [S-61] is cited for the golden-signal grouping and the SLI shapes; it
  makes no claim about FlexCore.
