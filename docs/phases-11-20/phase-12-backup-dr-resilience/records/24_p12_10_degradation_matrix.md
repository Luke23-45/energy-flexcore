# P12.10 — Graceful-degradation matrix (per-dependency-down behavior, mapped to written code only)

| Field | Value |
|---|---|
| Subtask | P12.10 (graceful-degradation matrix — per-dependency-down behavior table mapped to the EXISTING degraded-mode vocabulary; no silent capability claims) |
| Wave / agent | Phase 12 wave 1, Agent B |
| Status | **STATIC CODE-REVIEW ARTIFACT.** Every cell below traces WRITTEN Phase 0–11 code cited by file and symbol; nothing has been executed, and no cell claims observed behavior. Where written code does NOT specify a behavior, the cell says UNSPECIFIED — the absence is recorded, never papered over (the matrix maps what code DOES, never what a system should do). |
| Sources | The degraded-mode vocabulary of record: `deployment-upgrade-model.md` §5 (bounded store-and-forward; the central-outage rule); `crates/edge-agent/src/runner.rs` (HEALTHY/DEGRADED loop, audited `DegradedBufferingEntered`/`Exited`, poll-paused backpressure); `crates/edge-agent/src/health.rs` (closed `HEALTHY / DEGRADED_<reason> / FAILING_<reason>` vocabulary, P11.21 semantics); `apps/worker/src/supervision.rs` (P7.18: `BackpressureRefused`, `PoolStatus::Healthy/Degraded/Empty`); `apps/worker/src/runner.rs` (exponential error backoff, shutdown-aware sleeps); `apps/worker/src/probes.rs` + `apps/control-plane/src/probes.rs` (the P11.21 per-service `/ready` named checks `database`/`broker`); `crates/adapters/nats/src/consumer.rs` (P1.16 ack-after-commit, DLQ poison path) |
| Files touched | none (this record is the deliverable) |

## 1. The matrix

Behavior cells trace written code as cited in Sources; "(EMPTY)" marks a
behavior no written code specifies. §13.3 semantics govern every cell:
these are code-review mappings, never runtime guarantees.

| Dependency down | Control plane (written behavior) | Worker (written behavior) | Edge agent (written behavior) | Vocabulary mapped |
|---|---|---|---|---|
| **PostgreSQL unreachable** | `/ready` returns 503 with the named check `database` failing (`apps/control-plane/src/probes.rs`, P11.21 table); `/health` liveness stays 200. Read/write business routes touching the store fail through the adapter's named errors — no silent substitution path exists in the adapter (P11.19 audit: every tenant context is transaction-scoped `set_config(..., true)`) | the outbox publisher and consumer loops record store failures and retry after exponential backoff with shutdown-aware sleeps; no entry is lost on a store failure — the runner comment states the retry contract verbatim (`apps/worker/src/runner.rs`); the P11.20 stuck-state scanner keeps its PREVIOUS gauge value on a scan error (a scanner outage never reads as "no stuck commands") | n/a (the edge has no Postgres dependency) | `/ready` 503 named-check semantics (P11.21); `DEGRADED_*` is NOT claimed here for the control plane — its probes vocabulary is the named-checks 503, not the edge health set |
| **NATS unreachable** | `/ready` returns 503 with the named check `broker` failing (P11.21 table); publications ride the transactional outbox (migration 0007), so a broker outage parks publications in the outbox by design — the P1.15/P1.16 publisher/consumer code paths, cited by `crates/adapters/nats/src/` | consumer loop errors back off exponentially and retry (`apps/worker/src/runner.rs`); no ack is sent before commit (P1.16 ack-after-commit — a crash before ack is safe), so unacknowledged deliveries redeliver after reconnect; DLQ receives only poison messages (final failed attempt → DLQ + `+TERM`) | n/a (the edge's command/audit uplink is its own wss/https channel, not NATS) | JetStream `Limits` retention + `File` storage (`topology.rs`, D-J4); P1.16 redelivery semantics [S-65] |
| **Worker dead/crashed** | unaffected as written — the control plane has no synchronous worker dependency; commands remain in the store | the supervised pool reports `PoolStatus::Degraded { failed_workers, unavailable }` (or `Empty`); a crash marks the worker FAILED immediately, removed from the ready set; recovery requires a successful probe AND a supervisor policy record — no silent restart path (`apps/worker/src/supervision.rs`, P7.18); over-cap work is refused with `BackpressureRefused`, audited, never queued unboundedly | n/a | `PoolStatus::Healthy/Degraded/Empty` (P7.18 vocabulary); `BACKPRESSURE_REFUSED` audit kind |
| **Edge offline / cloud (central) unreachable from the edge** | n/a | n/a | the supervision loop enters DEGRADED on forward failure: keep polling + buffering; recovery on a successful forward exits with the audit event — the audited `DegradedBufferingEntered`/`DegradedBufferingExited` transitions (`runner.rs`); the buffer bound is the ceiling: a full buffer PAUSES polling (audited backpressure, `poll_paused_backpressure`), overflow is audited drop-of-OLDEST with `dropped_oldest` + the latched `has_overflowed` (`buffer_file.rs`, `audit_buffer.rs`); **no new commands while disconnected** — the command channel is the outbound session itself (`deployment-upgrade-model.md` §5) | edge runner HEALTHY/DEGRADED loop; edge health vocabulary `HEALTHY / DEGRADED_<reason> / FAILING_<reason>` (`health.rs`); the central-outage rule (§5): site-local safety stays authoritative — FlexCore's absence degrades orchestration, never site safety |
| **Site EMS (Zone 4) unreachable from the edge** | n/a | n/a | the adapter-health vocabulary governs: `FAILING_UNRESPONSIVE` after the explicit detection window BLOCKS dependent actions (`health.rs :: check_action_allowed` — while `FAILING_*`, ingesting from that adapter is blocked, never guessed); `DEGRADED_STALE_DATA` when data ages past the staleness limit | the closed `DEGRADED_<reason>` / `FAILING_<reason>` set: UNVERIFIED, STALE_DATA, ERROR_RATE, POLICY_BLOCK, CREDENTIAL_EXPIRY, CLOCK_DRIFT; UNRESPONSIVE, CREDENTIAL_EXPIRY |

## 2. What the matrix does NOT claim (the no-silent-capability rule)

1. **No cell claims the behavior HAPPENS** — each traces code that says it
   would, written-unrun (the 0–10 base; conventions §7.1). The first
   human build/run is the gate for any observed-behavior claim.
2. **UNSPECIFIED cells stay empty by design.** Nothing in written code
   specifies, e.g., control-plane behavior degradation labels beyond the
   `/ready` named checks, or a recovery-order contract across components.
   Any such claim would be invented — the matrix records the absence.
3. **The central-outage rule is inherited verbatim** from
   `deployment-upgrade-model.md` §5 (cited, not re-decided): a central
   outage must not disable site-local protective controls or force an
   unsafe device state; FlexCore is supervisory.
4. **Zero observed violations from finite tests is a test outcome, never
   a guarantee** (§13.3): when the C-chaos experiments (record 21) run,
   their outcomes fill evidence records — they do not upgrade these
   cells into guarantees.
5. **ADR-010**: the matrix adds no dispatch authority; degraded modes
   refuse and park, they never issue.

## 3. Vocabulary of record (single source per name)

| Name | Defined once in | Consumed here |
|---|---|---|
| edge degraded-buffering transitions | `crates/edge-agent/src/runner.rs` (`DegradedBufferingEntered`/`Exited` audit kinds) | matrix row "edge offline / central unreachable" |
| edge adapter health set | `crates/edge-agent/src/health.rs` (`HEALTHY / DEGRADED_<reason> / FAILING_<reason>`; P11.21 semantics; the `CLOCK_DRIFT` reason rides the P6.31 hold-on-drift seam) | matrix rows "edge offline", "site EMS unreachable" |
| worker pool status | `apps/worker/src/supervision.rs` (`PoolStatus`) | matrix row "worker dead/crashed" |
| probe named checks | `apps/worker/src/probes.rs` + `apps/control-plane/src/probes.rs` (P11.21 per-service table) | matrix rows "PostgreSQL unreachable", "NATS unreachable" |
| degraded modes + central-outage rule | `deployment-upgrade-model.md` §5 | the matrix's framing row and rule 3 |

No second definition of any name exists in this record (the defined-once
discipline, gap analysis §9).

## 4. Handoff notes

1. **P12.15 (wave 2)**: the acceptance-criteria record consumes this
   matrix's §2 rules verbatim — "resilient" may be claimed only as
   written-code-traced behavior plus run outcomes, never as a guarantee.
2. **Human gate**: an observed-behavior annex to this matrix may be
   filled ONLY from a real run's evidence (a game day, record 22 §1),
   cell by cell, with the run's date and evidence file — never inferred.
3. **Reconciliation**: if Agent A's P12.05 standby design changes the
   Postgres-down row's recovery behavior (failover), the row is amended
   by citation of A's record, not by re-deciding here.
