# Final Review Ledger — Phase 14

**Ledger status:** **FULLY FIXED (written) — 3 of 3 subtasks fixed, awaiting human build/test.**

Phase scope: see the implementation ledgers (`ledger/phase-14-*` for phases
00–10, `docs/phases-11-20/phase-14-*` for 11–20). Findings originate in
`../01_reviews/` (citation = the finding ID); subtask IDs are
`FRV1-P14.<xx>`. Ticks read exactly **"fixed, awaiting human
build/test"** — never "tested"/"verified"/"works".

## Fix subtasks

| ID | Finding | Sev | Title | Fix surface | Status |
|---|---|---|---|---|---|
| FRV1-P14.01 | `FRV1-R6-001` | P1 | ProtocolConnection port has zero implementations; five adapter guides claim connection states ride it | Add an impl or a named Unwired* refusing default + recorded deferral; correct the five guides | FIXED (written, awaiting human build/test) — fix wave 3 (UnwiredProtocolConnection named refusing default + PROTOCOL_CONNECTION_SEAM_NOT_WIRED recorded deferral; five adapter guides corrected) |
| FRV1-P14.02 | `FRV1-R6-004` | P2 | protocol-20305 is the only adapter without a non-test scope/manifest constructor | Add non-test constructors | FIXED (written, awaiting human build/test) — fix wave 3 (non-test scope.rs constructors; test helper and proposal.rs delegate to the reviewed spelling) |
| FRV1-P14.03 | `FRV1-R6-006` | P2 | MQTT CredentialBindingSet doc says BTreeSet; implementation is a sorted Vec | Align doc and implementation | FIXED (written, awaiting human build/test) — fix wave 3 (doc corrected to describe the BTreeSet-uniqueness/Vec-sorted-storage reality) |
