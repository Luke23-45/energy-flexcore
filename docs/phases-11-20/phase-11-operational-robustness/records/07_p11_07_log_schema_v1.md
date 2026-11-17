# P11.07 — Structured-log schema v1

| Field | Value |
|---|---|
| Subtask | P11.07 (structured-log schema v1, mapped to the OTel logs data model) |
| Wave / agent | Phase 11 wave 1, Agent A |
| Status | WRITTEN-UNRUN: schema and emitters written; never executed. |
| Sources | [S-62] (OTel logs data model — SeverityNumber 1–24 bands, the Body/Timestamp/SeverityIndex/Attributes fields); [S-63] (the Rust logs bridge maturity note lives in record 04) |
| Files touched | `python/workers/worker_template/log_schema.py` (new), `apps/control-plane/src/metrics.rs` (correlation line + `severity_number_for_level`; P11.01 edit already touched this file — the schema fields are part of the same recording block), `python/workers/tests/test_log_schema_and_trace_propagation.py` (UNRUN tests) |

## 1. Schema v1 contract

Every structured log line carries, in this field set:

| Field | Type | Meaning |
|---|---|---|
| `schema_version` | string, constant `"flexcore.log.v1"` | The schema this line speaks; a consumer rejects/migrates on it, never guesses. |
| `severity_number` | integer, OTel SeverityNumber band base [S-62] | TRACE→1, DEBUG→5, INFO→9, WARN→13, ERROR→17, FATAL→21; unknown level → 0 (UNSPECIFIED), never a guessed band. |
| `severity_text` | string | The emitter's level name, uppercased (the human-readable half of the OTel severity pair). |
| `timestamp` | RFC 3339 UTC | Emit time (the OTel data model's Timestamp). |
| `message` | string | The body (OTel Body), REDACTED at emit (P11.08). |
| `tenant_id` | string | MANDATORY attribute. Empty string = not applicable on this event (absent-vs-empty is expressed by the value, not by dropping the key — consumers get a stable field set). |
| `command_id` | string | MANDATORY attribute (empty = not applicable). Populated by command-scoped emitters (the command routes — Agent B's files; the generic request middleware never parses bodies, the P1.20 rule, so it emits the empty value). |
| `trace_id` | string | MANDATORY attribute. On the OTel-exported path, the bridge's own records carry the W3C trace id of the active span [S-62]; the stdout correlation line's `trace_id` remains the recorded P1.20 32-hex per-process correlation handle (explicitly NOT a cross-process trace id — the recorded P1.20 decision stands; the cross-process id rides the `traceparent` header of the P11.05 seam and the OTel span context). |
| …extras | scalars / nested maps | Emitter-supplied attributes, redacted at emit (P11.08). |

## 2. SeverityNumber mappings (the two language halves)

- **Rust (`tracing`)**: `apps/control-plane/src/metrics.rs::severity_number_for_level`
  maps `TRACE`→1, `DEBUG`→5, `INFO`→9, `WARN`→13, `ERROR`→17,
  `FATAL`→21, unknown→0, pinned by a unit test. On the OTel-exported
  path, the `opentelemetry-appender-tracing` bridge performs the
  standard tracing→SeverityNumber conversion [S-62]/[S-63]; the local
  mapping exists for the stdout schema fields.
- **Python (stdlib `logging` / `structlog`)**:
  `worker_template/log_schema.py::severity_number_for_level` maps the
  logging/structlog level names (including the `warn` alias and
  `exception`→ERROR-band 17, `critical`/`fatal`→21) to the same band
  bases, unknown→0, pinned by UNRUN tests.

## 3. Implementation changes

1. **Control-plane correlation line** (`metrics.rs::track_metrics`): now
   emits `schema_version`, `severity_number`, `tenant_id` (RENAMED from
   the P1.20 `tenant` — the schema's canonical attribute name; recorded
   here as the schema-v1 change), `command_id` (empty value on the
   generic middleware line, per §1), alongside the existing
   `trace_id`/`method`/`route`/`status`/`duration_ms`/`actor`. The P1.20
   no-secrets rule is untouched (Authorization never logged; no body
   content).
2. **Python emitter** (`log_schema.py::emit_log_line`): builds the §1
   shape, applies emit-time redaction (P11.08) to the message and every
   attribute BEFORE serialization, serializes sorted-key JSON, writes
   one line to the given stream (DEFAULT stdout). Stdlib-only; the
   UNRUN tests pin the field set, severity mapping, and one-JSON-object-
   per-call shape.
3. **Worker template**: the emitter is available to every worker via the
   template; no existing call site was rewritten (the template had no
   log emitter before — the correlation fields previously traveled in
   the handler's response echo, which is unchanged).

## 4. Honesty boundary

- No log line has ever been emitted by any of this code; the schema is
  a written contract plus written emitters.
- The `tenant` → `tenant_id` rename is a compatibility-relevant change
  to the P1.20 correlation line; it is recorded HERE (and in the code
  comment) rather than silently shipped. Consumers of the old field
  name are the operator's grep/dash tooling, which does not exist as
  code in this workspace (the gap analysis: the metrics body is
  "scraped by nothing") — no code consumer is broken.
