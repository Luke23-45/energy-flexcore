# P14.20 — Backpressure and rate limiting per protocol

| Field | Value |
|---|---|
| Subtask | P14.20 (wave 3, Agent A) — polling budgets per gateway, fleet-scale connection limits, bounded queues per adapter, per-adapter metrics in the P11 taxonomy (trace_id correlation consumed, never redefined) [S-85] |
| Deliverable class | Shared budget/observability vocabulary (model types) + per-adapter binding modules + WRITTEN-UNRUN tests |
| Honesty boundary | No limiter runs, no queue holds a real entry, no metric was ever observed, scraped, or alerted on. The budget types are DECLARED data an adapter checks against its own surface; there is no shared limiter runtime, no token-bucket thread, no shared mutable state. |

## 1. Sources

[S-85] (the gap-analysis rate-limiting/backpressure row: "Plugin
call-rate limits only; no API-edge limiter" — this subtask closes the
PROTOCOL-tier gap as designed budget declarations; the API-edge limiter
itself is the control-plane's `rate_limit.rs`, out of this phase's
scope and untouched). The budget discipline inherits the P14.07
gateway-polling-budget record (declared, never defaulted, fail-closed)
and the P6.08/P6.09 bounded-queue vocabulary.

## 2. The shared-module placement decision

The shared vocabulary lives in **`crates/flexcore-protocol-ports/src/
observability.rs`** — an ADDITIVE module on the common CORE port layer,
with the owning rationale in its header: every adapter already depends
on this crate, so one module gives every adapter the SAME family names,
label rules, and budget types without any adapter depending on another
adapter, and without a core crate ever depending on an adapter (the
forbidden direction). A new shared crate was rejected (conventions
§6.2: no crate without a stable dependency direction — a composition
crate cannot be depended on by adapter crates under the documented
"nothing depends ON composition crates except apps" rule, and adapters
depending on a shared composition crate would be a first-of-kind edge).
The module is pure model types: serde-only, no metrics library, no
runtime — the metric families are NAMES and label RULES; rendering is a
composition-root concern exactly as the P11.01 families are rendered in
`apps/control-plane/src/metrics.rs` (untouched).

The common crate's API surface is otherwise UNCHANGED (no vocabulary
variant added, no existing module touched — the publish-capability gap
of record 43 §2 stays the reviewed human handoff it already was).

## 3. The registered taxonomy (consumes the P11, never redefines it)

Metric families (bound VERBATIM by `deploy/compose/observability/
prometheus/rules/protocol_adapter_alerts.yml`, record 84):

| Family | Shape | Labels (closed sets only) |
|---|---|---|
| `flexcore_protocol_adapter_connections` | gauge | `adapter_id` (the registry's allowlisted ids), `state` (EXACTLY `ConnectionState::name()`: CLOSED/CONNECTING/CONNECTED/DEGRADED/UNREACHABLE) |
| `flexcore_protocol_adapter_messages_total` | counter | `adapter_id`, `direction` (`inbound`/`outbound`) |
| `flexcore_protocol_adapter_messages_refused_total` | counter | `adapter_id`, `reason` (the shared `RateRefusal` set) |
| `flexcore_protocol_adapter_queue_depth` | gauge | `adapter_id` |
| `flexcore_protocol_adapter_queue_dropped_total` | counter | `adapter_id`, `policy` (`fail_closed`/`drop_oldest_audited`) |

Cardinality decisions (recorded once, the P2.21/P11.01 discipline):
`adapter_id` is the ONLY per-adapter label and its value set is the
composition registry's allowlisted ids; refusal/overflow reasons are
closed vocabularies (raw error text never becomes a label); per-device,
per-point, and per-peer breakdowns go to structured LOGS, never to
labels. **trace_id correlation is CONSUMED**: the shared module carries
the P11.07 schema version (`flexcore.log.v1`) and the `trace_id` field
name verbatim and adds exactly ONE tier-specific field (`adapter_id`);
it never redefines `trace_id`, `tenant_id`, `command_id`, or
`schema_version`. A unit test pins the family spellings and the
consumed vocabulary names.

## 4. The budget types (declared, validated, fail-closed, no defaults)

- `MessageRateBudget` — max messages per interval; refuses overages
  with the named `RateRefusal::RateExceeded`.
- `BoundedQueuePolicy` — capacity + explicit overflow behaviour
  (`FailClosed` refuses; `DropOldestAudited` reports the audited
  oldest-drop so the adapter can count it on the dropped-total family).
  Zero capacity is a construction error.
- `ConnectionLimit` + `FleetConnectionRegistry` — the fleet-scale
  connection budgets, BTreeMap-keyed; an adapter WITHOUT a declared
  limit admits NO connections (fail-closed, never an unlimited default);
  a zero-maximum hand-built limit is treated as not-declared (zero-cap
  defence in depth).

## 5. The per-adapter bindings (each consumes its own owning budget module)

| Adapter | Module | What it binds |
|---|---|---|
| Modbus (`crates/adapters/protocol-modbus/src/backpressure.rs`) | CREATED | Derives the shared `MessageRateBudget` FROM the declared `GatewayPollingBudget` (one read request per device per scan, at the declared minimum interval — derived, never independently declared, so the two cannot drift); maps every `polling::BudgetError` onto the closed refusal vocabulary (total, constant mapping); declares the poll-plan queue policy. `polling.rs` stays the OWNING checker; the binding only adds the shared shapes. |
| MQTT (`crates/adapters/protocol-mqtt/src/backpressure.rs`) | CREATED | One budget per EDGE NODE (`EdgeNodeUplinkBudget`: derived publish rate + outbound queue policy) and a `FleetUplinkBudgetRegistry` keyed by node id — fleet scale is a registry of per-node budgets, never a shared pool one noisy node can exhaust; an unbudgeted node does not publish (fail-closed). Under `DropOldestAudited` the OLDEST un-published payload makes room (the P6.09 store-and-forward continuity rule: never drop the newest telemetry). |
| DNP3 (`crates/adapters/protocol-dnp3/src/backpressure.rs`) | CREATED | The report-rate budget plus the event-queue cap policy applied AT THE ADMISSION CHECK, before `OutstationModel::queue_event` — this module never reaches into the outstation's queue state. Under `DropOldestAudited` the caller drains and REPORTS the oldest event through the seam (an overflow is reporting pressure, never silent data loss); overflow behaviour itself stays UNVERIFIED against IEEE 1815.2-2025 [S-14]. |
| OpenADR (`crates/adapters/protocol-openadr/src/backpressure.rs`) | CREATED | The per-VEN poll budget is DERIVED from the VEN's AGREED poll frequency; a VEN agreeing a faster poll than the VTN's declared floor is REFUSED at planning time, never silently accelerated (and never silently slowed — the enrolment record is the fix). Fleet connections ride the shared `FleetConnectionRegistry`. |
| IEEE 2030.5 | no new module (recorded decision) | The server skeleton's own declared budget surface IS the cap: `ServerLimits` bounds the device-facing lists (oldest-dropped, record 02 §4) and `MtlsRequirements` bounds the session posture (fail-closed). The fleet-scale connection budget rides the shared registry; adding a parallel module would duplicate a cap the crate already declares. |
| CIM | no new module (recorded decision) | Model import is a one-way batch parse of supplied bytes — there is no recurring message rate to budget and no queue. The bounded surface is the profile document itself (the model layer's own empty/duplicate refusals, record 44 §2). |

## 6. Files created / modified

| File | Status |
|---|---|
| `crates/flexcore-protocol-ports/src/observability.rs` | CREATED (families, label rules, budget types, tests) |
| `crates/flexcore-protocol-ports/src/lib.rs` | MODIFIED (ONE additive module declaration + comment) |
| `crates/adapters/protocol-modbus/src/backpressure.rs` + `src/lib.rs` line | CREATED / MODIFIED |
| `crates/adapters/protocol-mqtt/src/backpressure.rs` + `src/lib.rs` line | CREATED / MODIFIED |
| `crates/adapters/protocol-dnp3/src/backpressure.rs` + `src/lib.rs` line | CREATED / MODIFIED |
| `crates/adapters/protocol-openadr/src/backpressure.rs` + `src/lib.rs` line | CREATED / MODIFIED |
| `crates/edge-agent/src/adapters/edge_to_cloud.rs` (record 82's module) | consumes `MessageRateBudget` — the edge-agent gained the `flexcore-protocol-ports` dependency line (record 82 §3) |

Byte-untouched: `apps/control-plane/src/metrics.rs` (the P11 families
live there and are not modified — the protocol families will render in
the SAME registry when the first real adapter composition lands, the
P2.21 "no second metrics surface" rule); `protocol-modbus/src/polling.rs`
(the owning budget module — consumed, not edited); the never-granted
set; `ledger_phase_14.md`.

## 7. Human gates named

(a) the first workspace build (everything here is WRITTEN-UNRUN);
(b) the Modbus protocol-text check of the 125-register read limit
(record 23's UNVERIFIED anchor — unchanged by this batch); (c) the DNP3
event-overflow semantics check against IEEE 1815.2-2025 (record 24's
UNVERIFIED anchor — the policy here is declared, the standard's own
overflow behaviour is still unchecked); (d) operator sign-off of the
registered thresholds (record 84's rule file — X-8 RECOMMENDATIONS).
