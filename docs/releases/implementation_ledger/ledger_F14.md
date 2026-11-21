# Ledger — F-14 Dynamic operating envelopes from live DSO feeds

**Owning code:** `crates/flexcore-application + crates/flexcore-domain/doe_envelope.rs`
**Phase record:** P14.15
**Source:** docs/releases/final-implementation-before-publication.md §5 (feature F-14); acceptance criteria cited per subtask.

| Subtask | Description | Status |
|---|---|---|
| FIM-F14.01 | DOE envelope feed port implementation: per-connection-point import-export envelopes with issue timestamps and expiry fixed (written, awaiting human build/test) — wave 5 |
| FIM-F14.02 | Optimizer constraint wiring: live envelope constrains dispatch as constraint data (never a command, ADR-010 intact) fixed (written, awaiting human build/test) — wave 5 |
| FIM-F14.03 | Lapse behavior: expired envelope reverts to the static limit and the reversion is telemetered fixed (written, awaiting human build/test) — wave 5 |
| FIM-F14.04 | Breach/curtailment recording with the envelope version in force fixed (written, awaiting human build/test) — wave 5 |
| FIM-F14.05 | Pinned tests: envelope constrains static-allowed dispatch, expiry reversion + telemetry, version recording fixed (written, awaiting human build/test) — wave 5 |
| FIM-F14.06 | Honesty header: live DSO feed interop is a HUMAN GATE fixed (written, awaiting human build/test) — wave 5 |

---

**Status:** IMPLEMENTED (written) — 6/6 subtasks fixed (written, awaiting human build/test) — wave 5. Ticks are applied only as
**fixed (written, awaiting human build/test)** with the wave number.