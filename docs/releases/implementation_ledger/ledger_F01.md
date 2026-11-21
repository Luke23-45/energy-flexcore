# Ledger — F-01 Live IEEE 2030.5 server transport

**Owning code:** `crates/adapters/protocol-20305`
**Phase record:** P14.02
**Source:** docs/releases/final-implementation-before-publication.md §5 (feature F-01); acceptance criteria cited per subtask.

| Subtask | Description | Status |
|---|---|---|
| FIM-F01.01 | HTTPS + mTLS listener: tokio-rustls accept loop with mandatory client-certificate validation, fail-closed on absent/untrusted certs fixed (written, awaiting human build/test) — wave 1 |
| FIM-F01.02 | hyper/HTTP1 serving of the CSIP resource tree through the AggregatorTransport seam wired to the deterministic resource store fixed (written, awaiting human build/test) — wave 1 |
| FIM-F01.03 | DERControl request flow end-to-end through the live server into the PROPOSED external-event mapping fixed (written, awaiting human build/test) — wave 1 |
| FIM-F01.04 | Pinned tests: refused request without client cert; DERControl flow over the live server; certificate-fingerprint allowlist enforcement fixed (written, awaiting human build/test) — wave 1 |
| FIM-F01.05 | Honesty header: conformance against a real 2030.5 client is a HUMAN LAB GATE fixed (written, awaiting human build/test) — wave 1 |

---

**Status:** IMPLEMENTED (written) — 5/5 subtasks fixed (written, awaiting human build/test) — wave 1. Ticks are applied only as
**fixed (written, awaiting human build/test)** with the wave number.