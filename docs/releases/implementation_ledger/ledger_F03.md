# Ledger — F-03 Live Modbus TCP client

**Owning code:** `crates/adapters/protocol-modbus`
**Phase record:** P14.07
**Source:** docs/releases/final-implementation-before-publication.md §5 (feature F-03); acceptance criteria cited per subtask.

| Subtask | Description | Status |
|---|---|---|
| FIM-F03.01 | MBAP header + PDU frame codec (hand-written, full Modbus TCP subset used by the adapter: read/write FCs, exception responses) fixed (written, awaiting human build/test) — wave 2 |
| FIM-F03.02 | Tokio TCP transport implementing the existing transport port trait: connect, poll, timeout, reconnect budget fixed (written, awaiting human build/test) — wave 2 |
| FIM-F03.03 | SunSpec model read through the live client into the register-map registry with unit conversion at the boundary fixed (written, awaiting human build/test) — wave 2 |
| FIM-F03.04 | Named refusal path: unreachable device / timeout / exception code surfaces a named refusal, never a hang fixed (written, awaiting human build/test) — wave 2 |
| FIM-F03.05 | Pinned tests: frame codec round-trips incl. exception responses, SunSpec read mapping, refusal taxonomy fixed (written, awaiting human build/test) — wave 2 |
| FIM-F03.06 | Honesty header: device interop is a HUMAN LAB GATE fixed (written, awaiting human build/test) — wave 2 |

---

**Status:** IMPLEMENTED (written) — 6/6 subtasks fixed (written, awaiting human build/test) — wave 2. Ticks are applied only as
**fixed (written, awaiting human build/test)** with the wave number.