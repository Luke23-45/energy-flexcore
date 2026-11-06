# PLAN — Phase 1: Modular Rust Foundation

| Field | Value |
|---|---|
| Phase | Phase 1 — Report Stage 1 |
| Report trace | §23 Stage 1; §6–§9; §15–§16; §22.2; §8 |
| Ledger | `ledger_phase_1.md` (P1.01–P1.32) |
| Status | Planning only — no code written by this ledger |

## 1. Scope

### In scope

- Cargo workspace (root manifest, lockfile, shared dependency policy, pinned toolchain Rust 1.99.1, edition 2024).
- Crates with real code only: `flexcore-contracts`, `flexcore-domain`,
  `flexcore-ports`, `flexcore-application`, `flexcore-events`,
  `flexcore-storage`, `flexcore-audit`, `flexcore-authz`, plus adapters
  `adapters/postgres`, `adapters/nats`.
- Apps: `apps/control-plane` (Axum API, composition root) and `apps/worker`
  (outbox publisher + event consumers).
- PostgreSQL migrations + tenant context + RLS; transactional outbox; NATS
  JetStream topology + idempotent consumers + inbox.
- Structured tracing/metrics (OpenTelemetry exporters pinned), config/secrets
  handling, health/readiness + graceful shutdown.
- Security + fault-injection test slices for the foundation.
- Local developer Compose stack (control plane, Postgres 18, NATS 2.15.x,
  worker).
- CI with the applicable §22.2 gates; dependency/license inventory as D4 input.
- Stage 1 exit gate: one end-to-end non-control workflow through API,
  application use case, database, and event publication.

### Non-goals (must NOT be built yet)

- No telemetry ingestion, telemetry tables, registry UI, portfolio views, or
  simulator telemetry source (Phase 2).
- No flexibility envelopes, scheduling, optimizer, or solver integration
  (Phase 3).
- No grid feasibility, command issue/acknowledgement path, approvals, or audit
  UI beyond the domain state machine and append-only audit port (Phase 4).
  The P1.06 state machine is a domain type, not a working dispatch path.
- No operator console (Phase 5), no edge agent or protocol adapters (Phase 6),
  no plugin host/WASM (Phase 7), no Python workers (Phase 7+).
- No Kubernetes/Helm (Helm chart from v1.0.0, §21.4; Compose first, §21.1).
- No Redis (only if measurements justify it, §16.1 — no measurements exist yet).
- No TimescaleDB as a mandatory dependency (optional deployment/profile only,
  §16.1).
- No load/performance testing with published numbers (§19.2 layer 8 is a later
  activity; no latency/throughput claim is made in Phase 1).
- No license decision (D4 stays open; P1.28 delivers an inventory, not a decision).
- No edits to `docs/foundations/*`, no edits to the report, no commits/pushes
  from this ledger work.

## 2. Architecture decisions in play

| Decision | Status | Report trace |
|---|---|---|
| DDD + hexagonal + modular monolith | Implemented and CI-enforced (P1.04) | §6.1–§6.2, §8.2 |
| Dependency rule (domain → nothing infrastructure) | Enforced by script + CI (P1.04, P1.30) | §6.2, §8.2 |
| PostgreSQL source of truth; NATS JetStream transport | Implemented (P1.13, P1.15) | §16.1, §15.4–§15.5, ADR-007 |
| Transactional outbox + idempotent consumers | Implemented (P1.13, P1.15–P1.16) | §6.3, §15.5 |
| At-least-once delivery; exactly-once NOT promised | Design rule (P1.14–P1.16) | §6.3, §15.4, [R6] |
| Axum + Tower; Tokio; SQLx; async-nats; utoipa; tracing | Pinned per `RESEARCH.md` (P1.02–P1.03) | §7.3 |
| RLS as defence in depth; app role ≠ table owner | Implemented + tested (P1.12–P1.13, P1.23) | §16.2, [R25] |
| OpenAPI generated from code (utoipa) | Implemented (P1.17) | §7.3, §15.1 |
| Buf lint + breaking-change detection | CI gates from day one (P1.18, P1.27) | §10.5, §22.2 |
| Worker as a separate process | Justified in P1.19 (background work distinct from request handling) | §8.1, §8.3 |
| D4 license audit | Inventory delivered; decision OPEN | §25.2 |

## 3. Component breakdown

| Component | Location | Subtasks |
|---|---|---|
| Toolchain + workspace root | `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`, `Cargo.toml`, `Cargo.lock` | P1.02–P1.03 |
| Contracts (value types) | `crates/flexcore-contracts/` | P1.05 |
| Domain (entities, invariants, command state machine) | `crates/flexcore-domain/` | P1.06 |
| Ports (capability interfaces) | `crates/flexcore-ports/` | P1.07 |
| Application (use cases) | `crates/flexcore-application/` | P1.08 |
| Events + outbox types | `crates/flexcore-events/` | P1.09 |
| Storage abstractions + audit | `crates/flexcore-storage/`, `crates/flexcore-audit/` | P1.10 |
| Authorization types | `crates/flexcore-authz/` | P1.11 |
| PostgreSQL adapter + migrations | `crates/adapters/postgres/`, `migrations/` | P1.12–P1.13 |
| NATS adapter + outbox publisher | `crates/adapters/nats/` | P1.14–P1.15 |
| Idempotent consumers + inbox | (adapter + migration) | P1.16 |
| Control-plane API | `apps/control-plane/` | P1.17, P1.20–P1.22 |
| Worker | `apps/worker/` | P1.19 |
| Protobuf/Buf governance | `proto/`, `buf.yaml`, `buf.gen.yaml` | P1.18 |
| Compose stack | `deploy/compose/` | P1.25 |
| CI | (workflow files) | P1.27 |

Only crates with real code are created (§8.1). `flexcore-kernel`,
`flexcore-optimization-api`, and `flexcore-grid-api` are NOT created in Phase 1:
the kernel is a composition concern that lands with plugin composition, and the
optimizer/grid contracts land with Phases 3–4. The §8.1 tree is a target
structure, not a Phase 1 checklist.

## 4. Data contracts

| Contract | Home | Notes |
|---|---|---|
| Canonical value types (MW/MVAr/MVA/MWh, SOC [0,1], Q convention, six-state quality, event/ingest time) | `flexcore-contracts` (P1.05) | Implements P0.07–P0.09; newtypes prevent unit confusion at compile time |
| Domain entities (Tenant, Portfolio, Site, Asset, AssetState) | `flexcore-domain` (P1.06) | §10.1 subset for Stage 1; `tenant_id` on every tenant-owned entity |
| Command lifecycle (domain state machine) | `flexcore-domain` (P1.06) | Implements P0.11; no issue/ack path yet |
| Event vocabulary (26 events) | `flexcore-events` (P1.09) | Implements P0.14; versioned serialization |
| Outbox record | `flexcore-events` + Postgres adapter | Idempotency key (unique), delivery state, attempt count |
| Audit record (append-only) | `flexcore-audit` (P1.10) | §10.1 fields; no update/delete on the port |
| Authorization decision/scope/context | `flexcore-authz` (P1.11) | Deny-by-default; context from authenticated claims only |
| REST resources (Stage 1 subset) | `apps/control-plane` (P1.17) | Tenants/portfolios/sites/assets only; full 16-resource list is later phases |
| PostgreSQL schema + RLS | `migrations/` (P1.12–P1.13) | No telemetry tables in Phase 1 |
| JetStream subjects/streams/consumers | Design doc + `adapters/nats` (P1.14–P1.15) | At-least-once; per-asset ordering; DLQ |

## 5. Interfaces

- **External:** REST/JSON under `/api/v1` (OpenAPI generated by utoipa);
  `/health`, `/ready`, `/metrics` on the control plane and worker.
- **Internal:** gRPC/Protobuf is governed by Buf from day one (P1.18), but no
  `.proto` files are required in Phase 1 unless the first worker contract
  needs one — the decision is recorded in P1.18.
- **Events:** NATS JetStream subjects per the P1.14 topology; outbox publisher
  drains PostgreSQL outbox → JetStream; consumers use the inbox pattern.
- **Composition roots:** `apps/control-plane` and `apps/worker` are the only
  places that wire adapters to ports.

## 6. Sequencing

1. Entry gate (P1.01) → toolchain/workspace (P1.02–P1.03) → dependency-rule
   check (P1.04).
2. Core crates bottom-up: contracts (P1.05) → domain (P1.06) → ports (P1.07)
   → application (P1.08) → events (P1.09) → storage/audit (P1.10) → authz
   (P1.11). Events and authz can proceed in parallel once contracts exist.
3. Schema design (P1.12) → Postgres adapter (P1.13) → JetStream design
   (P1.14) → NATS adapter/publisher (P1.15) → idempotent consumers (P1.16).
4. API (P1.17) + Buf governance (P1.18) + worker (P1.19) in parallel once
   adapters exist.
5. Cross-cutting: observability (P1.20), config/secrets (P1.21), probes/
   shutdown (P1.22), security tests (P1.23), fault-injection tests (P1.24).
6. Compose stack (P1.25) → exit-gate demonstration (P1.26) → CI (P1.27) →
   license inventory (P1.28) → handover (P1.29) → boundary audit (P1.30) →
   exit-gate review (P1.31) → anti-scope audit (P1.32).

Relative effort: the dominant cost is the Postgres/NATS integration work and
its tests (P1.13–P1.16, P1.23–P1.24), then the API and Compose stack. No
calendar dates are asserted; sequencing is by dependency, per §23.

## 7. Roles

| Role | Responsibilities |
|---|---|
| Platform engineering (Rust) | Workspace, crates, adapters, API, worker, Compose, CI |
| Principal engineering | Entry/exit gates, dependency-rule audit, handover, ADR for exceptions |
| Security (reviewer) | RLS design, authz review, security test suite, secret handling |
| Power systems | Reviews domain types (units, SOC, quality) — no solver work in this phase |
| Legal (recipient) | Receives D4 dependency/license inventory (P1.28) |

## 8. Risks and mitigations

| Risk | Mitigation |
|---|---|
| Domain crate imports infrastructure for convenience | P1.04 CI-enforced dependency check with a negative test |
| RLS bypassed (app runs as table owner) | Separate owner/runtime roles; RLS tests per role (P1.13, P1.23) |
| Outbox dual-write hole (state without event, or vice versa) | Single-transaction write + atomicity test (P1.13) |
| Exactly-once assumed from broker acks | At-least-once design + idempotent consumers + restart tests (P1.14–P1.16, P1.24) |
| Auth stub ships to production | Auth-integration status recorded; no hard-coded test tenant in the app (P1.17) |
| Secrets in logs or source control | Allowlist log review + no-secrets test + secret scan (P1.20–P1.21) |
| Floating toolchain/dependency versions | Exact pins; lockfile committed; CI verifies (P1.02–P1.03, P1.27) |
| Empty crates created "to be ready" | P1.30 boundary audit removes any member without real code |
| License asserted before D4 | Workspace license field left unset with a D4 TODO (P1.03); inventory is input only (P1.28) |

## 9. What must NOT be built yet

See §1 non-goals. In particular: no telemetry, no registry UI, no scheduling,
no feasibility, no dispatch path beyond the domain state machine, no edge,
no plugins, no console, no Python workers, no Helm/Kubernetes, no Redis,
no TimescaleDB mandate.

## 10. How the §24 vertical slice lands here

Phase 1 builds the foundation the slice runs on and nothing of the slice
itself. The slice's step 1 (create tenant/portfolio/site/grid model/four
simulated assets) is partially enabled: tenant/portfolio/site/asset creation
exists via the API (P1.17, P1.26); the grid-model reference and the four
simulated assets are Phase 2 work. Steps 2–10 do not land in Phase 1. The
Stage 1 exit gate (P1.26) is the foundation-level rehearsal: one non-control
workflow end-to-end through API → use case → database → event publication.
