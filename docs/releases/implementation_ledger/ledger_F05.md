# Ledger — F-05 Live MQTT 5 + Sparkplug B edge uplink

**Owning code:** `crates/adapters/protocol-mqtt + crates/edge-agent/src/adapters/edge_to_cloud.rs`
**Phase record:** P14.11
**Source:** docs/releases/final-implementation-before-publication.md §5 (feature F-05); acceptance criteria cited per subtask.

| Subtask | Description | Status |
|---|---|---|
| FIM-F05.01 | MQTT 5 client codec (hand-written: CONNECT/CONNACK, PUBLISH/PUBACK, SUBSCRIBE/SUBACK, PINGREQ/PINGRESP, DISCONNECT, QoS 1, property subset used; TLS via tokio-rustls) fixed (written, awaiting human build/test) — wave 3 |
| FIM-F05.02 | Minimal Sparkplug B protobuf wire encoder (hand-written varint/tag/length-delimited encoder for the payload subset: BIRTH, DATA, DEATH) fixed (written, awaiting human build/test) — wave 3 |
| FIM-F05.03 | MqttUplinkTransport real implementation replacing UnwiredUplinkTransport as the composed default when configured fixed (written, awaiting human build/test) — wave 3 |
| FIM-F05.04 | Birth/data/death session semantics end-to-end: NBIRTH+DBIRTH on connect, NDATA/DDATA on telemetry, NDEATH on unclean disconnect (LWT) fixed (written, awaiting human build/test) — wave 3 |
| FIM-F05.05 | Command path: subscribed command topic delivers into the edge agent; no secret material held by the client (never-granted rule intact) fixed (written, awaiting human build/test) — wave 3 |
| FIM-F05.06 | Pinned tests: codec round-trips, birth/data/death sequences, Sparkplug byte fixtures, credential-binding absence fixed (written, awaiting human build/test) — wave 3 |
| FIM-F05.07 | Honesty header: broker interop + Sparkplug B conformity is a HUMAN LAB GATE fixed (written, awaiting human build/test) — wave 3 |

---

**Status:** IMPLEMENTED (written) — 7/7 subtasks fixed (written, awaiting human build/test) — wave 3. Ticks are applied only as
**fixed (written, awaiting human build/test)** with the wave number.