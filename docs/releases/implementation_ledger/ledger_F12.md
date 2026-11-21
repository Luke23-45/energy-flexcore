# Ledger — F-12 Secure OTA: TUF-shaped bundle producer + staged rollout

**Owning code:** `crates/edge-agent + apps/control-plane`
**Phase record:** P18.02-P18.06
**Source:** docs/releases/final-implementation-before-publication.md §5 (feature F-12); acceptance criteria cited per subtask.

| Subtask | Description | Status |
|---|---|---|
| FIM-F12.01 | Bundle producer: image + metadata + Ed25519 signatures (ed25519-dalek, existing), TUF-shaped metadata repository layout fixed (written, awaiting human build/test) — wave 6 |
| FIM-F12.02 | Verification through the existing edge verifier: produced bundle verifies, tampered bundle refuses fixed (written, awaiting human build/test) — wave 6 |
| FIM-F12.03 | Cohort rollout coordinator (control plane): canary cohorts, abort thresholds, per-device assignment fixed (written, awaiting human build/test) — wave 6 |
| FIM-F12.04 | Device-side A/B slots: download - verify - install - status reporting, auto-rollback to previous slot on failed install fixed (written, awaiting human build/test) — wave 6 |
| FIM-F12.05 | Audit trail for every attempt (P18.07) fixed (written, awaiting human build/test) — wave 6 |
| FIM-F12.06 | Pinned tests: producer-verifier round-trip, tamper refusal, abort threshold, rollback reporting, audit records fixed (written, awaiting human build/test) — wave 6 |
| FIM-F12.07 | Honesty header: key ceremonies are HUMAN GATES (P18.03); no key material ships fixed (written, awaiting human build/test) — wave 6 |

---

**Status:** IMPLEMENTED (written) — 7/7 subtasks fixed (written, awaiting human build/test) — wave 6. Ticks are applied only as
**fixed (written, awaiting human build/test)** with the wave number.