# Ledger — F-08 Live integration platform: machine identity, API keys, webhook delivery

**Owning code:** `apps/control-plane`
**Phase record:** P13.07-P13.09
**Source:** docs/releases/final-implementation-before-publication.md §5 (feature F-08); acceptance criteria cited per subtask.

| Subtask | Description | Status |
|---|---|---|
| FIM-F08.01 | OAuth2 client-credentials JWT issuance and validation for grid-operator / market-adapter clients; revocation refuses scoped calls fixed (written, awaiting human build/test) — wave 4 |
| FIM-F08.02 | Per-tenant API keys: issuance, rotation without dropping in-flight callers, last-used evidence fixed (written, awaiting human build/test) — wave 4 |
| FIM-F08.03 | Webhook live delivery over HTTP with HMAC-SHA256 signing and 5-minute tolerance, wired to the existing webhook bookkeeping port fixed (written, awaiting human build/test) — wave 4 |
| FIM-F08.04 | Dead-letter landing after the retry budget (existing permanent-rejection classifier intact) fixed (written, awaiting human build/test) — wave 4 |
| FIM-F08.05 | Secret posture: webhook secrets never enter the database (P11.16 posture verified) fixed (written, awaiting human build/test) — wave 4 |
| FIM-F08.06 | Pinned tests: token issue/validate/revoke, key rotation + last-used, signed delivery verification, dead-letter path, secret absence fixed (written, awaiting human build/test) — wave 4 |
| FIM-F08.07 | Honesty header: live receiver interop is a HUMAN GATE fixed (written, awaiting human build/test) — wave 4 |

---

**Status:** IMPLEMENTED (written) — 7/7 subtasks fixed (written, awaiting human build/test) — wave 4. Ticks are applied only as
**fixed (written, awaiting human build/test)** with the wave number.