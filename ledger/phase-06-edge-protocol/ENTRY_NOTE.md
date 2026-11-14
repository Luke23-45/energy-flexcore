# ENTRY NOTE — Phase 6: Edge and Protocol Integration

Date: 2026-11-12. Coordinator: implementation-coordinator. Loop rules in
force: nothing installed, nothing built, no tests run — complete code + test
files written only. Ticks are "implemented, awaiting human build/test".

## Gate exception (same pattern as Phases 1–5)

Stage 5 exit gate is **NOT HELD** (demonstration UNRUN, sign-off NOT SIGNED —
`ledger/phase-05-operator-console-simulator/30_stage5_exit_record.md`). Phase
6 starts on the recorded exception; P6.04 (readiness check) records it
honestly rather than asserting Stage 1–5 completion.

## Phase-6-specific boundaries

1. **D5 (protocol/asset mix) and D6 (cyber/OT boundary) are NOT decided by
   agents.** P6.01/P6.03 RECORD them as still-open with recommended defaults
   labelled DEFAULT; the protocol adapter work (P6.12–P6.13) proceeds against
   the RECOMMENDED default (OpenEMS first per the report) while D5 stays OPEN.
2. **Read-only/test-lab by default** (v0.4.0 release ladder): the command
   path (P6.14–P6.16) is DISABLED by default, test-env-gated, never a field
   write path. No conformance claims (P6.19 records the boundary).
3. **No new infrastructure deps without re-verify labels**: OpenEMS is an
   external Java system — integration is written against its documented REST
   API (EdgeConfig), UNVERIFIED, never executed. mTLS via rustls (already in
   the workspace through sqlx's tls-rustls) — no new crate additions without
   the D4 audit note.
4. **Everything at the edge is offline-tolerant, fail-closed, tenant-scoped** —
   reuse the P4.24/P4.28 discipline (sequence, replay protection, signature
   verification) — cite, don't reinvent.
5. Site/partner naming is a D5 input — placeholder `pilot-site-A` labelled
   PLACEHOLDER wherever a name is structurally required.

## Wave plan (max 2 subagents; disjoint file ownership per batch)

| Wave | Subtasks | Files owned | Notes |
|---|---|---|---|
| 0 (coordinator) | — | this file, HANDOVER.md plan | Gate exception + boundaries |
| 1a | P6.01, P6.03, P6.02, P6.04 | decision records in ledger dir; license-audit note (static); readiness check | D5/D6 recorded OPEN; scans = human inputs |
| 1b | P6.05, P6.12, P6.20 | NEW `crates/edge-agent/**` (+ workspace member wiring), adapter port + ACL, deployment profile docs/config | P6.04's readiness inputs supplied by coordinator prompt |
| 2a | P6.06, P6.07 | edge identity/enrollment/certs, outbound mTLS channel | Depends on wave 1b crate |
| 2b | P6.08, P6.09 | telemetry read path, buffering/store-and-forward | Depends on P6.07 |
| 3a | P6.10, P6.11 | OpenEMS discovery + read path e2e | Depends on P6.08/10 |
| 3b | P6.13, P6.14, P6.15, P6.16 | adapter read impl; command path disabled-by-default + test-env verification + allowlisted forwarding | Depends on P6.12 |
| 4a | P6.17, P6.18, P6.21 | local policy/health, audit buffer/offline, multi-tenant edge isolation | |
| 4b | P6.19, P6.23, P6.22 | conformance boundary tests, security tests, performance/scale TEMPLATES (EMPTY cells) | |
| 5a | P6.25, P6.31, P6.32 | console surfacing, time sync, decommissioning | |
| 5b | P6.27, P6.28, P6.26 | CI gates, failure drill, docs | |
| 6a | P6.33, P6.34, P6.29 | evidence-pack completeness, secure-boot record, Phase 7 handover | |
| 7 (coordinator) | P6.30 | retrospective + v0.4.0 release-gate record (UNRUN) | All P6.01–P6.34 |
