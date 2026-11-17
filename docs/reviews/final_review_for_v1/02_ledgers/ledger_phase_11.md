# Final Review Ledger — Phase 11

**Ledger status:** **FULLY FIXED (written) — 7 of 7 subtasks fixed, awaiting human build/test.**

Phase scope: see the implementation ledgers (`ledger/phase-11-*` for phases
00–10, `docs/phases-11-20/phase-11-*` for 11–20). Findings originate in
`../01_reviews/` (citation = the finding ID); subtask IDs are
`FRV1-P11.<xx>`. Ticks read exactly **"fixed, awaiting human
build/test"** — never "tested"/"verified"/"works".

## Fix subtasks

| ID | Finding | Sev | Title | Fix surface | Status |
|---|---|---|---|---|---|
| FRV1-P11.01 | `FRV1-R2-003` | P1 | Nine metric families have no producer or no renderer; worker ingestion metrics never rendered | Drive the registry call sites and render IngestionMetrics at the worker /metrics route | FIXED (written, awaiting human build/test) — fix wave 2 (worker renders both metric sets; no-producer families carry honest NO-PRODUCER HELP text per the finding's recommendation) |
| FRV1-P11.02 | `FRV1-R5-004` | P1 | Worker inbox table created by runtime ensure_table outside the embedded migration set | Promote the inbox table into the embedded migration set | FIXED (written, awaiting human build/test) — fix wave 2 (additive migration 0024 owns the inbox table; runtime ensure_table removed; compat suite now pins the true 24-migration set) |
| FRV1-P11.03 | `FRV1-R2-005` | P2 | MetricsRegistry::leave_request doc claims saturating but fetch_sub wraps | Make it saturating or fix the doc | FIXED (written, awaiting human build/test) — fix wave 4 (fetch_update + checked_sub saturation; the pinned lopsided-decrement property now true) |
| FRV1-P11.04 | `FRV1-R2-006` | P2 | RateLimitState::rejection_counters rendered nowhere; eviction check runs after insertion | Render the counters; reorder the eviction check | FIXED (written, awaiting human build/test) — fix wave 4 (eviction check precedes insertion with a bounded map; rejection counters rendered at /metrics; the P11.01 taxonomy record amended additively first per the P11.14 rule) |
| FRV1-P11.05 | `FRV1-R2-011` | P2 | Idempotency middleware returns an empty body when terminal-response buffering fails | Return a named error body/status on buffering failure | FIXED (written, awaiting human build/test) — fix wave 4 (buffering failure returns the named infrastructure error shaping, never an empty body) |
| FRV1-P11.06 | `FRV1-R5-006` | P2 | Stale 8-of-26 subject-divergence defect record in the NATS crate | Remove/correct the stale record | FIXED (written, awaiting human build/test) — fix wave 4 (stale divergence record corrected to the reconciled truth with subject.rs as authority) |
| FRV1-P11.07 | `FRV1-R5-007` | P2 | NATS README records the wrong single home for the durable InboxStore | Fix the README | FIXED (written, awaiting human build/test) — fix wave 4 (NATS README names the true InboxStore home and the 0024 promotion path) |
