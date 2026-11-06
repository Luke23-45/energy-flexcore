# CONTEXT — Phase 1: Modular Rust Foundation

## Background

Phase 1 is Report Stage 1 (§23): build the modular Rust foundation — Cargo
workspace, contracts/domain/application/ports/adapters separation, Axum API,
PostgreSQL migrations and tenant context, NATS event integration and outbox,
structured tracing and metrics, CI and test harness. Its exit gate is one
end-to-end non-control workflow through API, application use case, database,
and event publication.

The architecture is DDD + hexagonal (ports and adapters) + modular monolith
(§6.1). The domain layer must not depend on Axum, SQLx, NATS, pandapower, a
specific inverter vendor, or the React application (§6.2). Services are
extracted only for independent lifecycle, security isolation, scaling, or
fault containment (§6.1, §8.3) — the control plane stays a modular monolith in
Phase 1, with the worker as a separate process for background work and the
edge agent deferred to Phase 6.

## Key report sections to re-read

| Section | Why it matters here |
|---|---|
| §6 Software architecture | DDD, hexagonal, modular monolith, dependency rule, outbox, storage strategy, patterns |
| §7 Rust-first technology architecture | Why Rust, language allocation, recommended Rust stack, solver rule |
| §8 Workspace and repository structure | Target tree, crate dependency rules, service-extraction policy |
| §9 Plugin contracts and core interfaces | Capability contracts; result requirements (later phases) |
| §10 Domain model and data contracts | Entities, units/sign, time model, quality, Protobuf governance |
| §14 Command lifecycle | State machine as a domain type only in this phase (no dispatch path) |
| §15 APIs, events, messaging | API styles, REST resources, event vocabulary, NATS semantics, outbox |
| §16 Data storage and tenancy | Storage choices, RLS, retention |
| §17 Security | Zones, controls, threat model (security tests derive from §17.4) |
| §19.2 Test layers | Unit, property, contract, integration, deterministic scenario, fault injection, security, performance/load (only the Phase 1 slices apply) |
| §21 Deployment and operations | Compose-first, probes, observability correlation, build form |
| §22.2 CI quality gates | The gates Phase 1 must implement |
| §23 Stage 1 | Deliverables and exit gate |
| §24 Vertical slice | Foundation only — see PLAN.md §10 |
| §25.1 Provisional technology choices | Rust/NATS/PostgreSQL/gRPC/React/Wasmtime/Apache-2.0 with revisit triggers |

## Canonical units/sign (from Phase 0, P0.07 — cited, not re-defined)

MW, MVAr, MVA, MWh. P > 0 = export at the point of connection. Reactive-power
sign declared per grid model/device profile, never assumed. SOC is a fraction
in [0,1] at the API boundary. Persisted values and API contracts use canonical
units.

## Telemetry quality (from Phase 0, P0.08 — cited, not re-defined)

Exactly one per value: `GOOD`, `SUSPECT`, `BAD`, `MISSING`, `ESTIMATED`,
`STALE`. Phase 1 stores the enum (PostgreSQL enum, exactly six values) but
ingests no telemetry — quality *detection* is Phase 2.

## Terminology

- "license" (US spelling); "acknowledgement" (noun).
- Validation, feasibility, and authorization are distinct gates — Phase 1
  implements none of them as a dispatch path; the domain state machine
  (P1.06) is a type-level model only.
- At-least-once delivery: JetStream redelivers after missed
  acknowledgements; consumers must be idempotent. Exactly-once side effects
  are never promised from broker acknowledgements alone.
- Outbox: state change + outbox record in one PostgreSQL transaction; the
  publisher marks delivered only after broker acknowledgement.
- RLS: PostgreSQL Row-Level Security as defence in depth; the application
  runtime role must not own tenant tables (table owners bypass RLS unless
  `FORCE ROW LEVEL SECURITY` is configured).

## Constraints (simulation-only 0.x)

- The entire 0.x line is simulation-only. Phase 1 adds no device writes and
  no control authority above Simulation.
- No hard-real-time claims: Tokio, Rust, gRPC, NATS, or a cloud service do
  not provide hard-real-time guarantees (§3.5).
- The workspace `license` field must not assert Apache-2.0 while D4 is open
  (P1.03). The repository has no license file (`.agent/PROVENANCE.md`).

## Prior-phase outputs consumed (Phase 0)

| Phase 0 output | Consumed by |
|---|---|
| Scope/non-goals note (P0.01) | All Phase 1 scope decisions |
| First buyer/use cases (P0.02) | Choice of the Stage 1 use case (P1.08) |
| D1–D3 records (P0.03–P0.05) | Entry gate (P1.01); no jurisdiction-specific code while D1 is open |
| D4–D6 gate records (P0.06) | P1.28 (D4 inventory input) |
| Units/sign contract (P0.07) | `flexcore-contracts` (P1.05) |
| Quality vocabulary (P0.08) | Postgres quality enum (P1.12–P1.13) |
| Time model (P0.09) | `flexcore-contracts` time types (P1.05) |
| Asset/capability vocabulary (P0.10) | Domain entities (P1.06); schema design (P1.12) |
| Command state machine (P0.11) | Domain state machine (P1.06) |
| Event vocabulary (P0.14) | `flexcore-events` (P1.09); JetStream topology (P1.14) |
| API surface (P0.15) | Axum routes (P1.17) |
| Tenancy/storage principles (P0.16) | Schema + RLS + tenant context (P1.12–P1.13, P1.11) |
| Threat model (P0.17) | Security tests (P1.23); secret handling (P1.21) |
| Control-authority note (P0.18) | Anti-scope audit (P1.32) |
| Plugin model (P0.19) | Ports as capability seams (P1.07) |
| Deployment/build form (P0.20) | Compose stack (P1.25); observability (P1.20) |
| ADRs 001–012 (P0.21) | Architecture decisions in PLAN.md §2 |
| Handover pack (P0.27) | Entry gate (P1.01) |

## D1–D6 relevance

| Gate | Phase 1 relevance |
|---|---|
| D1 jurisdiction/market | OPEN or scheduled — no jurisdiction-specific code in Phase 1 |
| D2 first portfolio/service | Shapes the Stage 1 use case only via P0.02 |
| D3 network class/fidelity | No grid-model code in Phase 1; D3 gates Phase 4 |
| D4 license/dependency audit | **P1.28 delivers the inventory**; decision stays OPEN |
| D5 first protocol/asset mix | No protocol code in Phase 1; gates Stage 6 |
| D6 cyber/OT boundary review | No edge/OT code in Phase 1; gates any read-only pilot |

## Open questions

1. Which CI system hosts the §22.2 gates (P1.27 — decide once, record it)?
2. Repository-port traits: do they live in `flexcore-ports` or
   `flexcore-storage` (P1.10 — decide once, record it, do not duplicate)?
3. First `.proto` files now or with the first worker (P1.18 — decide once)?
4. Outbox publisher hosting: worker process (P1.19) — confirmed in the plan;
   the alternative (standalone publisher) is rejected in P1.19's record.
5. Integration-test harness: testcontainers or CI PostgreSQL/NATS services
   (P1.13/P1.15 — decide once, record it)?
6. Cross-tenant denial status: 403 vs 404 (P1.17 — decide once, record it).
7. UUID strategy: UUIDv7 vs UUIDv4 for primary keys (P1.12 — decide once;
   note PostgreSQL 18 has built-in `uuidv7()`).
