# Ledger — F-13 ISO 15118 Plug & Charge certificate management (code surface)

**Owning code:** `crates/adapters/protocol-ocpp`
**Phase record:** P17.12
**Source:** docs/releases/final-implementation-before-publication.md §5 (feature F-13); acceptance criteria cited per subtask.

| Subtask | Description | Status |
|---|---|---|
| FIM-F13.01 | V2G Root CA trust store model with anchor management; fail-closed when no trust anchor is configured fixed (written, awaiting human build/test) — wave 6 |
| FIM-F13.02 | OCPP 2.1 certificate flow handlers: Get15118EVCertificate, CertificateSigned, InstallCertificate fixed (written, awaiting human build/test) — wave 6 |
| FIM-F13.03 | OCSP validation path: minimal DER reader for OCSP responses, revoked = refuse, fail-closed on unreachable responder fixed (written, awaiting human build/test) — wave 6 |
| FIM-F13.04 | The five record-41 §3 ungate conditions enforced as runtime gates: each must be recorded in configuration or the feature refuses; the conditions are NOT claimed met fixed (written, awaiting human build/test) — wave 6 |
| FIM-F13.05 | Pinned tests: trust-store refusal without anchors, certificate flow state, OCSP revoked refusal, ungate-condition gating fixed (written, awaiting human build/test) — wave 6 |
| FIM-F13.06 | Honesty header: PKI operator, trust-anchor agreement, OCSP service, OCA text verification, D5 closure are HUMAN GATES; owner authorized the CODE SURFACE only (recorded in ledger_master.md) fixed (written, awaiting human build/test) — wave 6 |

---

**Status:** IMPLEMENTED (written) — 6/6 subtasks fixed (written, awaiting human build/test) — wave 6. Ticks are applied only as
**fixed (written, awaiting human build/test)** with the wave number.