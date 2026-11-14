<!-- P9.25 — Observability and alerting verification at the pilot. VERIFICATION TEMPLATE ONLY: the metric inventory cites the REAL metric families in apps/control-plane/src/metrics.rs and metrics_route.rs and the REAL edge health vocabulary in crates/edge-agent/src/health.rs; the alert-rule catalog is DESIGN with thresholds PLACEHOLDER per Phase 8 X-8; the verification record (trigger→route→acknowledge evidence) is EMPTY — no alert infrastructure is deployed or verified here. No pilot exists; no alert has ever fired, been routed, or been acknowledged. -->
# P9.25 — Observability and Alerting Verification at the Pilot (Verification Template — record EMPTY, no alert infrastructure deployed)

Date: 2026-11-14. Subtask: P9.25 (`ledger_phase_9.md` §P9.25).
Owner role: Platform engineering. Report trace: §23 Stage 9; §21.3
(observability: correlation fields, measured signals, no secrets in
logs). Inputs: P9.17 shadow operation (**NOT EXECUTED — no pilot, no
shadow data**), P9.20 drill results (**UNRUN**), the pilot deployment's
observability configuration (**DOES NOT EXIST — no deployment, no
configuration**).

## 1. Honest status — the plain statement

**No pilot observability configuration exists, no alert infrastructure
is deployed, no alert has ever fired, been routed, or been
acknowledged, and no verification has been executed.** There is no
pilot, no deployment, no named recipient, and no escalation rota. This
record delivers what an agent can deliver under `ENTRY_NOTE.md`
boundary 6:

- the **metric inventory** the pilot observability configuration MUST
  build on — traced to the real metric families in
  `apps/control-plane/src/metrics.rs` and `metrics_route.rs` and the
  real edge health vocabulary in `crates/edge-agent/src/health.rs`
  (§2), never to an imagined exporter;
- the **§21.3 correlation-field coverage map** — which correlation
  fields the existing surfaces carry and which they do NOT (§3);
- the **alert-rule catalog DESIGN** — signal, cited source, threshold
  (PLACEHOLDER per X-8), recipient (`<EMPTY>`), escalation (`<EMPTY>`)
  (§4);
- the **verification procedure** — trigger → routed → acknowledged, per
  alert class, with the record cells EMPTY (§5); and the **P9.36
  dependency** (§6).

No alerting system exists in the codebase: the `/metrics` endpoint and
the derived-on-read alarm resource EXPOSE data, but there is no alert
evaluation, no routing, no escalation, and no acknowledgement mechanism
anywhere in the repository. Every alerting capability below the raw
signal is a named deployment/code requirement, not an existing feature
(§4.3).

## 2. The metric inventory (cited — the real surfaces)

### 2.1 Control-plane metric families (`apps/control-plane/src/metrics.rs`)

`MetricsRegistry` renders ALL of the following, always (zero values
included, so an idle scrape still exposes the full inventory):

| Family (exact name) | Type | Meaning | Source |
|---|---|---|---|
| `flexcore_http_requests_total{method,route,status}` | counter | Total HTTP requests, route = matched path pattern (no raw-path cardinality explosion) | `metrics.rs` `render()`; recorded by `track_metrics` |
| `flexcore_http_errors_total` | counter | Total HTTP 5xx/4xx error responses | `metrics.rs` |
| `flexcore_telemetry_events_total{event}` | counter | Outbox telemetry events by the frozen §15.3 vocabulary — exactly `TelemetryReceived`, `TelemetryQualityChanged`, `AssetAvailabilityChanged`; anything else is not counted | `metrics.rs` `TELEMETRY_EVENT_TYPES` |
| `flexcore_telemetry_quarantine_total` | counter | Total quarantined telemetry records | `metrics.rs` |
| `flexcore_telemetry_quarantine_by_reason_total{reason_class}` | counter | Quarantined records by closed reason class (the free-text reason is NEVER a label) | `metrics.rs` `record_quarantine` |
| `flexcore_telemetry_queue_depth` | gauge | Ingestion bounded-channel length at last observation | `metrics.rs` `set_queue_depth` |
| `flexcore_telemetry_quality_states_total{quality}` | counter | Stored records per canonical quality state — exactly the six P0.08 states `GOOD`, `SUSPECT`, `BAD`, `MISSING`, `ESTIMATED`, `STALE` (a seventh value is ignored, never invented as a series) | `metrics.rs` `QUALITY_STATES` |
| `flexcore_telemetry_stale_assets` | gauge | Assets currently evaluated stale | `metrics.rs` `render()` |
| `flexcore_telemetry_asset_stale_total{asset_id}` | counter | Staleness observations per asset; a series exists ONLY while the asset is currently stale (bounded cardinality, recorded decision) | `metrics.rs` `record_staleness` |
| `flexcore_telemetry_ingestion_delay_milliseconds_sum` / `_count` | summary | Per-batch `(ingest_time - event_time)` in milliseconds; per-asset breakdown goes to logs, never to labels | `metrics.rs` `record_ingestion_delay_millis` |

Served at `/metrics` in Prometheus text exposition (`Content-Type:
text/plain; version=0.0.4; charset=utf-8`) by
`apps/control-plane/src/metrics_route.rs` — the ONLY metrics surface
(there is no second registry). Cardinality decisions are the recorded
P2.21 ones (`metrics.rs` module header).

### 2.2 Edge health vocabulary (`crates/edge-agent/src/health.rs`)

The CLOSED per-adapter health vocabulary — exactly, no seventh state:

| Status | Meaning |
|---|---|
| `HEALTHY` | observed recently, data fresh, error rate inside limits, credential valid |
| `DEGRADED_<reason>` | up but impaired: `UNVERIFIED`, `STALE_DATA`, `ERROR_RATE`, `POLICY_BLOCK`, `CREDENTIAL_EXPIRY`, and (P6.31 additive) `CLOCK_DRIFT` |
| `FAILING_<reason>` | dependent actions BLOCKED: `UNRESPONSIVE`, `CREDENTIAL_EXPIRY` |

The hard rule the alert design MUST inherit: health is NEVER inferred
from the absence of errors — an adapter with no observation yet is
`DEGRADED_UNVERIFIED`, never healthy; an adapter the monitor has never
seen returns `HealthError::UnknownAdapter` (a named error, not a
status). While `FAILING_*`, ingest and command targeting are blocked
with an audited reason (`check_action_allowed`). Snapshots render as
JSON (`HealthSnapshot::json()`) for the console (P6.25).

### 2.3 The other existing observability surfaces

| Surface | What it provides | Source |
|---|---|---|
| `/health` (liveness) and `/ready` (readiness, named dependency checks + shutdown gate) | Process-up vs dependencies-usable, deliberately distinct | `apps/control-plane/src/probes.rs` (P1.22) |
| `/api/v1/alarms` (derived-on-read, READ-ONLY) | `TELEMETRY_LOSS`, `TELEMETRY_STALENESS`, `DISPATCH_FAILURE`, `DISPATCH_EXPIRY`, `DISPATCH_FALLBACK`, `FEASIBILITY_LIMIT_VIOLATED`, `FEASIBILITY_FAIL_CLOSED` — re-derived per request from existing platform facts; absent sources are named in `sources`, never invented | `apps/control-plane/src/routes_alarms.rs` (P5.31); no acknowledgement path exists (P9.08 gap G3) |
| Per-request JSON correlation log line | One line per served request (§3) | `metrics.rs` `track_metrics` |

## 3. §21.3 correlation-field coverage map (honest: existing vs missing)

§21.3 verbatim: *"Correlate every workflow with: tenant/site/portfolio/
asset IDs; schedule and command IDs; plugin/solver/model versions;
telemetry snapshot and grid-model version; trace ID and actor identity;
and validation/authorization evidence."* What the EXISTING surfaces
actually carry:

| §21.3 field | Carried today? | Where (citation) |
|---|---|---|
| Trace ID | YES — 32-hex per-process id on every request log line | `metrics.rs` `next_trace_id` / `track_metrics`; recorded decision: NOT a UUID, **no cross-process guarantee** |
| Actor identity, tenant ID | YES on the request log line (resolved for correlation only; a resolution failure leaves them absent, never fabricated) | `track_metrics` |
| HTTP route/status/duration | YES | `track_metrics` |
| Site / portfolio / asset IDs | NOT on the request log line — carried in the domain records (site/asset registry, telemetry records) and audit evidence, addressable through the trace id only within the same process | Named gap G-C1 (§4.3) |
| Schedule and command IDs | NOT on the request log line — carried on the command lifecycle records (P4.10 store) and audit events | Named gap G-C1 |
| Plugin/solver/model versions | NOT carried by any metrics surface — version records live in the plugin/registry/model artifacts | Named gap G-C2 |
| Telemetry snapshot and grid-model version | Carried on domain records (envelope input snapshots, P9.03 model version), NOT on the log line | Named gap G-C2 |
| Validation/authorization evidence | Carried as `EvidenceRef` on every `AuthorizationDecision` and on the audit trail (`crates/flexcore-authz/src/decision.rs`), retrievable via `audit.event.read` — NOT on the request log line | P9.08 §4.3 |
| Cross-edge/central correlation | NOT guaranteed — the trace id has explicitly no cross-process guarantee; edge logs carry the adapter id and the correlation context the composition supplies | `metrics.rs` header; `health.rs` module note — named gap G-C3 |

The ledger's risk row ("correlation IDs missing across the
edge/central boundary, making a pilot incident untraceable") is a LIVE
risk, not a hypothetical: G-C3 is open until a correlation mechanism
that spans the boundary exists and is verified.

## 4. Alert-rule catalog DESIGN (thresholds PLACEHOLDER per X-8)

### 4.1 The X-8 rule applied to alert thresholds

Phase 8's binding prohibition X-8 (`ledger/phase-08-advanced-coordination/02_research_scope_freeze.md`):
**no arbitrary universal targets — thresholds are set after defining
the model, service, benchmark, and operational risk.** An alert
threshold invented here ("stale if > 60 s") would be exactly the
forbidden arbitrary number, fitted to no measured baseline (no run has
ever produced one). Every threshold in this catalog is therefore
`<PLACEHOLDER — pre-registered via dated amendment BEFORE the pilot
configuration goes live, per X-8 and the P9.16/P9.39 amendment
discipline>`.

### 4.2 The catalog (DESIGN — every non-signal cell EMPTY)

| Alert ID | Signal class (§21.3 / ledger step 3) | Existing source (citation) | Threshold | Recipient (named person) | Escalation | Trigger-test evidence |
|---|---|---|---|---|---|---|
| A-1 | Stale telemetry (CRITICAL — ledger step 3) | `flexcore_telemetry_stale_assets` + `flexcore_telemetry_asset_stale_total{asset_id}`; alarm `TELEMETRY_STALENESS` (routes_alarms.rs); edge `DEGRADED_STALE_DATA` | `<PLACEHOLDER per X-8>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY — UNRUN>` |
| A-2 | Command acknowledgement timeout (CRITICAL — ledger step 3) | **NO metric family exists** — acknowledgement state lives on command records (TRACKING requires ack; `DISPATCH_EXPIRY` alarm exists on command records) | `<PLACEHOLDER per X-8>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY — UNRUN>` |
| A-3 | Solver failure (CRITICAL — ledger step 3) | **NO metric family exists** — solver duration/status is not measured anywhere (`flexcore_http_errors_total` is the nearest existing proxy and is NOT equivalent) | `<PLACEHOLDER per X-8>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY — UNRUN>` |
| A-4 | Ingestion delay (§21.3 measured signal) | `flexcore_telemetry_ingestion_delay_milliseconds_sum/_count` (per-batch summary) | `<PLACEHOLDER per X-8>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY — UNRUN>` |
| A-5 | Queue depth (§21.3 measured signal) | `flexcore_telemetry_queue_depth` gauge | `<PLACEHOLDER per X-8>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY — UNRUN>` |
| A-6 | Adapter/edge health (§21.3 "plugin health") | `HealthSnapshot` closed vocabulary — alert on any `FAILING_*` (dependent actions blocked) and on `DEGRADED_CREDENTIAL_EXPIRY` / `DEGRADED_CLOCK_DRIFT` | `<PLACEHOLDER per X-8>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY — UNRUN>` |
| A-7 | Command delivery/tracking deviation (§21.3 measured signal) | **NO metric family exists** — tracking deviation is a §21.3 signal with no instrumentation today | `<PLACEHOLDER per X-8>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY — UNRUN>` |
| A-8 | Quarantine rate (telemetry quality) | `flexcore_telemetry_quarantine_total` + `..._by_reason_total{reason_class}` | `<PLACEHOLDER per X-8>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY — UNRUN>` |
| A-9 | Readiness loss (dependencies unusable) | `/ready` probe (`probes.rs`) — the orchestrator gate | `<PLACEHOLDER per X-8>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY — UNRUN>` |

### 4.3 Named instrumentation gaps (deployment/code requirements, never silently assumed)

| # | Gap | Why it matters |
|---|---|---|
| G-A1 | No alert evaluation/routing/escalation/acknowledgement mechanism exists anywhere in the repository — `/metrics` and `/api/v1/alarms` EXPOSE data; nothing consumes them into alerts. Recipient routing and escalation are pure design cells until a deployment composes them | The ledger's named failure mode "alerts routed to a mailbox nobody reads" cannot even be tested yet — there is no mailbox |
| G-A2 | No metric family for solver duration/status (A-3's source) — a NEW counter/summary is a code requirement before that alert can exist | Ledger step 3 requires a solver-failure alert test that currently has no instrument |
| G-A3 | No metric family for command delivery, acknowledgement, or tracking deviation (A-2/A-7's source) | Same as G-A2 for the command path |
| G-C1/G-C2/G-C3 | The §3 correlation gaps (asset/schedule/command IDs and versions off the log line; no cross-process trace guarantee) | Ledger verification method: "inspect a sampled trace end-to-end for the correlation fields" — impossible end-to-end today without the domain records |
| G-L1 | Log cleanliness (§21.3 "Never put secrets or unnecessary personal/customer data in logs") is enforced where inspected: the `Authorization` header is NEVER logged and no other header/body content is logged (`metrics.rs` rules); edge logs carry no secrets, no raw backend text (P1.08). The pilot-scale log SAMPLE inspection is still an execution cell (§5 step L) — the code rules bound what CAN be logged, not what WAS logged | Ledger acceptance (c) |

## 5. Verification procedure (TEMPLATE — the record is EMPTY; nothing has run)

Every step below is a HUMAN execution against the pilot deployment.
Every result cell is `<EMPTY — UNRUN>`; no date is invented.

| Step | Procedure | Record cell |
|---|---|---|
| V-1 | Confirm the deployed observability configuration EXISTS: which system scrapes `/metrics`, at what interval; which system evaluates the A-1..A-9 rules; which channel routes to whom. (Without a deployment this step cannot start.) | Deployed-config manifest reference: `<EMPTY>` |
| V-2 | Verify the §4.2 thresholds were pre-registered by dated amendment BEFORE go-live (X-8); a threshold changed after a failing observation voids the record | Amendment reference: `<EMPTY>` |
| V-3 | Per CRITICAL class (A-1 stale telemetry, A-2 command acknowledgement timeout, A-3 solver failure): TRIGGER the condition in a controlled way, confirm the alert ROUTES, and confirm the named recipient RECEIVES it within the expected time. The recipient witnesses at least one trigger-and-receive test (the ledger's verification method) | Per-class evidence (trigger time, route time, receipt time, recipient name): `<EMPTY — UNRUN>` |
| V-4 | Verify escalation for one unacknowledged alert: the escalation path fires after the configured window | Escalation evidence: `<EMPTY>` |
| V-5 | Inspect a sampled pilot record/trace end-to-end for the §21.3 correlation fields (ledger acceptance (a)); record which fields were absent (the §3 gaps are expected to show) | Sample reference + field checklist: `<EMPTY>` |
| V-6 | Inspect a sample of pilot logs for secrets and unnecessary personal/customer data (ledger step 4); the sample, its size, and its selection rule are recorded | Log-sample reference + inspector: `<EMPTY>` |
| V-7 | Record the FINAL alert catalogue: signal, threshold, recipient, escalation, and the test evidence per alert (ledger step 5) — i.e. §4.2 with every EMPTY cell filled from the actual run | Catalogue version: `<EMPTY>` |

**Verification record: EMPTY. No alert infrastructure is deployed here** —
this record deploys nothing, configures nothing, and fires nothing. The
P9.25 ledger tick reads "implemented, awaiting human build/test"; the
verification itself is the human gate.

## 6. The P9.36 dependency (recorded)

The live incident drill (`36_forged_command_drill.md`) has an
ALERT-PATH leg (its §3 leg 4), and its own risk table already records
the dependency (R-P9.36-2: *"The alert-path leg depends on P9.25-
verified alerting infrastructure (not yet built); a drill run before
P9.25 would have an unverified alert leg — recorded as
infrastructure-PENDING per case"*). This record formalises the other
side of it: **P9.36's alert leg MUST NOT run before this record's §5
steps V-1–V-4 are complete for at least the alert class the drill
exercises.** A P9.36 run with an infrastructure-PENDING alert leg is a
recorded finding against the drill, and P9.28 must not accept it as a
completed containment chain. No alert infrastructure is built by this
wave — the dependency stays OPEN until a deployment closes it.

## 7. Reviewer-verification procedure (human)

1. Confirm §1's plain statement: no observability configuration, no
   alert infrastructure, no fired/routed/acknowledged alert exists, and
   every §5 cell is `<EMPTY — UNRUN>`.
2. Confirm §2.1/§2.2 cite REAL surfaces: every family name in §2.1
   appears verbatim in `apps/control-plane/src/metrics.rs` `render()`;
   the health statuses in §2.2 match `crates/edge-agent/src/health.rs`
   exactly (no seventh state, no invented reason token).
3. Confirm §3 does not claim a correlation field the log line does not
   carry, and that the three gaps G-C1–G-C3 are named, not smoothed
   over.
4. Confirm every §4.2 threshold cell is `<PLACEHOLDER per X-8>` — no
   numeric threshold was invented anywhere in this record.
5. Confirm §4.3 lists the missing alert sources as code/deployment
   requirements rather than assuming them; confirm §6 records the
   P9.36 dependency in both directions.
6. Confirm no certification/safety wording appears, and the ledger tick
   wording is "implemented, awaiting human build/test".

## 8. Filling commands (for the human executor)

- Fill §5 V-1/V-2 after the pilot observability stack is deployed;
  attach the deployed-config manifest reference.
- Run V-3/V-4 per critical class with the named recipient present;
  file the trigger/route/receipt evidence into the §5 record and the
  CHECKLIST item "Observability verified" (evidence: verification
  report + alert catalogue).
- Fill §4.2 thresholds by DATED AMENDMENT before go-live (X-8) — never
  after seeing a failing observation.
- File the §3/G-C1–G-C3 and §4.3/G-A2–G-A3 follow-ups to the platform
  backlog with owners; they are also carried to P9.27 (§9).

## 9. Open risks carried to P9.27 (this subtask's contribution)

| Risk | Basis |
|---|---|
| No alert evaluation/routing/escalation/acknowledgement mechanism exists in the codebase — observability is EXPOSE-ONLY (`/metrics`, `/api/v1/alarms`); every alert capability below the raw signal is future deployment/code work | §1, §4.3 G-A1 |
| Three of the ledger's own alert-class requirements (A-2 command-ack timeout, A-3 solver failure, A-7 tracking deviation) have NO metric source today — the required instrumentation does not exist | §4.2, §4.3 G-A2/G-A3 |
| Cross-edge/central correlation is not guaranteed (trace id is per-process by recorded decision) — a pilot incident spanning the boundary may be untraceable | §3 G-C3; the ledger's named risk row |
| All thresholds are PLACEHOLDER per X-8; a threshold fitted after a failing observation would void the verification | §4.1; P9.39 R-1 (no pre-registered thresholds anywhere in the phase yet) |
| No pilot, no observability configuration, no recipient, no escalation rota, no trigger-and-receive evidence — every ledger acceptance criterion is UNMET and not claimed | §1, §5 |

Status: **WRITTEN-UNRUN** — a verification template cannot be "run";
the alert-infrastructure deployment, threshold pre-registration,
trigger-and-receive tests, trace inspection, and log inspection are the
human gates and have not happened and are not scheduled. No alert fired,
no pilot exists and none is claimed.
