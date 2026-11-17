# Final Review Ledger — Phase 20

**Ledger status:** **FULLY FIXED (written) — 3 of 3 subtasks fixed, awaiting human build/test.**

Phase scope: see the implementation ledgers (`ledger/phase-20-*` for phases
00–10, `docs/phases-11-20/phase-20-*` for 11–20). Findings originate in
`../01_reviews/` (citation = the finding ID); subtask IDs are
`FRV1-P20.<xx>`. Ticks read exactly **"fixed, awaiting human
build/test"** — never "tested"/"verified"/"works".

## Fix subtasks

| ID | Finding | Sev | Title | Fix surface | Status |
|---|---|---|---|---|---|
| FRV1-P20.01 | `FRV1-R8-001` | P1 | Observability tree ships with no consumer — P11.09 compose-wiring handoff dropped and untracked (phase-11 origin) | Wire Prometheus/Grafana/NATS-exporter into compose, or register the handoff as a tracked open item in the phase-11/20 close records | FIXED (written, awaiting human build/test) — fix wave 4 (docker-compose observability profile: Prometheus with the repo's only rule_files reference + scrape config, Grafana with the recorded provisioning paths, NATS exporter; inert until invoked; image tags recorded as ASSUMPTIONs re-verified at first build; P11.09 handoff closed; P11-E-2 stays OPEN) |
| FRV1-P20.02 | `FRV1-R8-006` | P1 | Stray git-tracked docs.rs snapshot files (ihv.html, jsidx.html) in the repository root | Remove them from the repo (git rm) — pure release hygiene | FIXED (written, awaiting human build/test) — fix wave 4 (ihv.html and jsidx.html verified as docs.rs snapshots and removed via git rm) |
| FRV1-P20.03 | `FRV1-R8-008` | P2 | flexcore-domain pins a literal version instead of workspace inheritance | Use workspace version inheritance | FIXED (written, awaiting human build/test) — fix wave 4 (flexcore-domain inherits version.workspace = true) |
