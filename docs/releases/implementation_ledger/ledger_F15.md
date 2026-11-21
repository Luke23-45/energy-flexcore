# Ledger — F-15 FDO-pattern zero-touch onboarding (in-repo surface)

**Owning code:** `crates/edge-agent`
**Phase record:** P18.09
**Source:** docs/releases/final-implementation-before-publication.md §5 (feature F-15); acceptance criteria cited per subtask.

| Subtask | Description | Status |
|---|---|---|
| FIM-F15.01 | Ownership voucher register: persisted voucher records keyed per device; claim refused without a voucher entry fixed (written, awaiting human build/test) — wave 6 |
| FIM-F15.02 | Rendezvous resolver implementation behind the existing seam (NoRendezvousConfigured replaced when configured; still refuses when not) fixed (written, awaiting human build/test) — wave 6 |
| FIM-F15.03 | Claim flow: factory-provisioned device claims with its voucher and joins the fleet with no on-site human enrollment fixed (written, awaiting human build/test) — wave 6 |
| FIM-F15.04 | Manual P9.33 path preserved as the default AND the fallback fixed (written, awaiting human build/test) — wave 6 |
| FIM-F15.05 | Audit trail for every claim attempt fixed (written, awaiting human build/test) — wave 6 |
| FIM-F15.06 | Pinned tests: voucher-refusal, claim success path, fallback preservation, audit records fixed (written, awaiting human build/test) — wave 6 |
| FIM-F15.07 | Honesty header: an external rendezvous service and a voucher-issuance process are HUMAN/INFRA GATES fixed (written, awaiting human build/test) — wave 6 |

---

**Status:** IMPLEMENTED (written) — 7/7 subtasks fixed (written, awaiting human build/test) — wave 6. Ticks are applied only as
**fixed (written, awaiting human build/test)** with the wave number.