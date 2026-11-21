# Ledger — F-09 Live observability: Python OTEL instrumentation + propagation

**Owning code:** `apps/worker/python (Rust side already wired)`
**Phase record:** P11.01-P11.09/P11.20
**Source:** docs/releases/final-implementation-before-publication.md §5 (feature F-09); acceptance criteria cited per subtask.

| Subtask | Description | Status |
|---|---|---|
| FIM-F09.01 | Shared pure-Python OTLP/trace propagation module: trace_id extraction from JetStream message headers and W3C traceparent, guarded imports (OTel SDK optional, fail-open to no-op) — the torch.load guard pattern fixed (written, awaiting human build/test) — wave 7 |
| FIM-F09.02 | Instrumentation of every worker entrypoint: spans carry trace_id through to logs; no PII/secret in emitted lines (P11.08 redaction posture) fixed (written, awaiting human build/test) — wave 7 |
| FIM-F09.03 | Collector config validated structurally (three signals wired: traces, metrics, logs) — a static validation tool, not a run fixed (written, awaiting human build/test) — wave 7 |
| FIM-F09.04 | Pinned tests (self-test flags): propagation round-trip, redaction, no-op fallback without SDK fixed (written, awaiting human build/test) — wave 7 |
| FIM-F09.05 | Honesty header: a running Collector and firing SLO alerts are HUMAN GATES (acceptance criteria 1-3 need live infra) fixed (written, awaiting human build/test) — wave 7 |

---

**Status:** IMPLEMENTED (written) — 5/5 subtasks fixed (written, awaiting human build/test) — wave 7. Ticks are applied only as
**fixed (written, awaiting human build/test)** with the wave number.