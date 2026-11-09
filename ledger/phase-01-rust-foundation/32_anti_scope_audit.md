# P1.32 — What Phase 1 explicitly did NOT build, decide, or claim

Date: 2026-11-07.

## 1. Not built

- No telemetry ingestion, telemetry tables, or telemetry API (Phase 2).
- No registry UI or portfolio views (Phase 2/5) — API only.
- No simulator telemetry source (Phase 2).
- No flexibility envelopes, scheduling, optimizer, or solver integration (Phase 3).
- No grid feasibility, no command issue/acknowledgement path, no approvals
  (Phase 4). The P1.06 `DispatchCommand` + state machine are DOMAIN TYPES,
  not a working dispatch path — no ISSUED command can be produced by any
  Phase 1 code.
- No operator console (Phase 5); no edge agent or protocol adapters (Phase 6);
  no plugin host/WASM (Phase 7); no Python workers (Phase 7+); no market
  integration (region-specific adapters, D5/D1).
- No Kubernetes/Helm (Compose first; Helm from v1.0.0). No Redis. No
  TimescaleDB. No `.proto` files (recorded decision: Phase 7).
- No load/performance testing and no published performance numbers (§19.2
  layer 8 is a later activity).

## 2. Not decided

- D1–D6 all remain SCHEDULED, not decided (`.agent/DECISIONS.md`); no
  jurisdiction, market, protocol, or network-class decision exists.
- No license decision: the workspace `license` field is deliberately
  ABSENT (Apache-2.0 remains a recommendation); P1.28 delivered an
  inventory to the D4 owners, not a decision.
- No retention values in code (configuration only); no RPO/RTO invented;
  no benchmark acceptance thresholds set.

## 3. Not claimed

- No device-write capability; no field-read capability; no control
  authority above Simulation (the entire 0.x line is simulation-only;
  field write authority begins at v1.0.0 with operator approval).
- No conformance/certification claim for any protocol or standard.
- No hard-real-time claim (§14.5/§3.5). No safety or protection-grade claim.
- Nothing is "verified" or "tested" by this work: all code is written,
  unbuilt, and unrun — verification is entirely the human's.

## 4. Repository state

- Added: workspace manifests + toolchain pins; 8 core crates, 2 adapters,
  2 apps; `tools/check_dependency_rule.py`; migrations under
  `crates/adapters/postgres/`; `deploy/compose/`; `proto/`;
  `.github/workflows/ci.yml`; Phase 1 ledger documents.
- Untouched: `docs/foundations/*`, the report, `.agent/*` (except the
  DECISIONS.md gate-status update made in Phase 0).
- Nothing committed or pushed by this work — commits are the human's
  (Conventional Commits, per AGENTS.md §10).

This list is referenced from the Phase 2 handover pack
(`ledger/phase-02-asset-telemetry/ENTRY_NOTE.md` cites Phase 1's status;
`29_handover_pack.md` is the index).
