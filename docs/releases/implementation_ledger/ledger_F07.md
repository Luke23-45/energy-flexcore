# Ledger — F-07 Live enterprise identity: OIDC login, SCIM 2.0, enforced step-up

**Owning code:** `apps/control-plane`
**Phase record:** P13.01-P13.04
**Source:** docs/releases/final-implementation-before-publication.md §5 (feature F-07); acceptance criteria cited per subtask.

| Subtask | Description | Status |
|---|---|---|
| FIM-F07.01 | OidResolver composed production path: JWKS fetch, issuer/audience validation, TenantContext resolution; DenyAllResolver remains the fail-closed default until an IdP is configured fixed (written, awaiting human build/test) — wave 4 |
| FIM-F07.02 | SCIM 2.0 endpoints (Users, Groups) with tenant-scoped persistence, group-to-role mapping, create/deactivate round-trip fixed (written, awaiting human build/test) — wave 4 |
| FIM-F07.03 | Step-up enforcement server-side at the APPROVED gate: high-impact approval without fresh IdP evidence is refused fixed (written, awaiting human build/test) — wave 4 |
| FIM-F07.04 | dev-auth stays non-default and feature-gated (verify, do not regress) fixed (written, awaiting human build/test) — wave 4 |
| FIM-F07.05 | Pinned tests: wrong-issuer/audience refusal, step-up refusal, SCIM round-trip, resolver composition fixed (written, awaiting human build/test) — wave 4 |
| FIM-F07.06 | Honesty header: real-IdP live validation is a HUMAN GATE (P13-E-3 stays OPEN) fixed (written, awaiting human build/test) — wave 4 |

---

**Status:** IMPLEMENTED (written) — 6/6 subtasks fixed (written, awaiting human build/test) — wave 4. Ticks are applied only as
**fixed (written, awaiting human build/test)** with the wave number.