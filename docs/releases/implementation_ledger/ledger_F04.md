# Ledger — F-04 Live OpenADR 3.0 VTN endpoints

**Owning code:** `crates/adapters/protocol-openadr`
**Phase record:** P14.05
**Source:** docs/releases/final-implementation-before-publication.md §5 (feature F-04); acceptance criteria cited per subtask.

| Subtask | Description | Status |
|---|---|---|
| FIM-F04.01 | HTTP endpoint serving (hyper/axum over tokio) wiring the designed endpoint seam to the VEN registry fixed (written, awaiting human build/test) — wave 2 |
| FIM-F04.02 | VEN registration, event poll, and report upload endpoints mapped onto the existing event/report/registration models fixed (written, awaiting human build/test) — wave 2 |
| FIM-F04.03 | Event delivery recording with the event version in force fixed (written, awaiting human build/test) — wave 2 |
| FIM-F04.04 | Pinned tests: registration, poll delivering an event, report upload, delivery version recording, auth refusal path fixed (written, awaiting human build/test) — wave 2 |
| FIM-F04.05 | Honesty header: VEN interop is a HUMAN LAB GATE fixed (written, awaiting human build/test) — wave 2 |

---

**Status:** IMPLEMENTED (written) — 5/5 subtasks fixed (written, awaiting human build/test) — wave 2. Ticks are applied only as
**fixed (written, awaiting human build/test)** with the wave number.