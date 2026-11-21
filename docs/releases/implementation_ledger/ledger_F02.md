# Ledger — F-02 Live OCPP CSMS WebSocket transport

**Owning code:** `crates/adapters/protocol-ocpp`
**Phase record:** P17.01/P17.02
**Source:** docs/releases/final-implementation-before-publication.md §5 (feature F-02); acceptance criteria cited per subtask.

| Subtask | Description | Status |
|---|---|---|
| FIM-F02.01 | RFC 6455 server-side WebSocket codec (hand-written, minimal subset: handshake, masking, fragmentation, close codes, ping/pong; no permessage-deflate) fixed (written, awaiting human build/test) — wave 1 |
| FIM-F02.02 | OCPP-J 1.6 subprotocol session layer over the codec: call/callresult/callerror framing, id correlation, bounded queues fixed (written, awaiting human build/test) — wave 1 |
| FIM-F02.03 | CsmsTransport seam wired: boot + heartbeat + charging-profile relay through the existing CSMS session state machine fixed (written, awaiting human build/test) — wave 1 |
| FIM-F02.02b | Refusal path: a charge point offering only ocpp1.6 is refused with the P17-E-1 reason (recorded vocabulary, byte-identical) fixed (written, awaiting human build/test) — wave 1 |
| FIM-F02.05 | Pinned tests: handshake acceptance/refusal, frame codec round-trips, boot/heartbeat/profile relay through the seam, protocol refusal reason fixed (written, awaiting human build/test) — wave 1 |
| FIM-F02.06 | Honesty header: RFC 6455 conformance + real charge-point interop is a HUMAN LAB GATE fixed (written, awaiting human build/test) — wave 1 |

---

**Status:** IMPLEMENTED (written) — 6/6 subtasks fixed (written, awaiting human build/test) — wave 1. Ticks are applied only as
**fixed (written, awaiting human build/test)** with the wave number.