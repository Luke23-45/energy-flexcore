# FlexCore Context Brief (agent digest)

Derived from `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md`
and the canonical foundations specification. This file is a digest: when the two
disagree, the sources win.

## What the product is

**Neryva FlexCore** is an open-source, modular Virtual Power Plant orchestration
and distribution-aware DER coordination platform. It aggregates heterogeneous
distributed energy resources into a portfolio, estimates available flexibility,
builds rolling-horizon schedules, validates them against asset and network
constraints, and manages the authorization → issue → acknowledgement → measured
response lifecycle of dispatch commands.

It is an **enterprise web application backed by a central control plane**, with
optional site edge connectors and a simulation/research environment.

It is **not**: a mobile app, an ADMS/SCADA replacement, a protective relay
controller, an ISO/RTO market platform, a hard-real-time controller, or an
autonomous AI dispatcher.

## Who it is for

1. VPP and DER aggregators
2. C&I energy portfolio operators
3. Microgrid and multi-site operators
4. ESCOs and system integrators

Utilities/DSOs are a later audience. Direct market participation is a
region-specific integration, not a core function.

## Build form

| Surface | Technology | Host |
|---|---|---|
| Operator Console | React + TypeScript | Browser |
| Control Plane | Rust (Axum, Tokio, SQLx) | Linux |
| Workers | Python (pandapower, HiGHS, CVXPY, PyTorch) | Linux containers |
| Site Edge Agent | Rust | Linux at site / OT DMZ |
| CLI + Simulation Lab | Rust CLI + simulators | Cross-platform CLI, Linux simulators |

## Architecture in one screen

- Plugin-oriented **modular monolith**; services are extracted only for
  independent lifecycle, security isolation, scaling, or fault containment.
- **DDD + hexagonal**: domain core, application use cases, ports, adapters,
  composition roots.
- 16 bounded contexts: Identity & Tenancy, Portfolio & Sites, Asset Registry,
  Telemetry, Grid Model, Flexibility, Forecasting, Scheduling, Optimization,
  Feasibility, Dispatch, Market Integration, Protocol Gateway, Plugin Kernel,
  Audit & Evidence, Simulation & Evaluation.
- Capability seams are contract → provider → consumer. Scheduling depends on an
  `Optimizer` contract, not on HiGHS or Clarabel directly.
- Three plugin execution classes: trusted built-in Rust, sandboxed WASM/WIT,
  out-of-process gRPC workers.
- Protected kernel: authorization, tenant isolation, audit integrity, command
  lifecycle, final dispatch validation. No plugin may replace these.

## Data and messaging

- PostgreSQL is the transactional source of truth.
- NATS JetStream carries durable events and work; at-least-once delivery with
  idempotent consumers and a transactional outbox.
- gRPC/Protobuf internally, REST/OpenAPI externally, SSE/WebSocket for live UI.
- Every decision record carries model version, schedule revision, telemetry
  snapshot, validation evidence, and authorization evidence.

## Control authority

| Mode | Writes to devices | When |
|---|---|---|
| Simulation | No | 0.x line, development, benchmarks |
| Read-only / shadow | No | Early pilots, commissioning |
| Operator-approved | Yes, per-command human approval | v1.0.0 default |
| Bounded automation | Yes, inside an approved envelope | Post-1.0, per site and asset class |

Mode is a deployment/site setting. Plugins cannot select or raise it.

## Solver discipline

- HiGHS: LP/QP/MIP. Not SOCP.
- Clarabel: convex conic including SOCP. Not MISOCP.
- CVXPY: modelling layer; capabilities depend on the selected solver.
- IPOPT: local nonlinear solve; no global-optimality guarantee.
- AC power flow evaluates a network state; AC-OPF is a different problem.
- SOCP relaxation exactness is conditional and must be checked, not assumed.

## What is deliberately not promised

Hard real-time or protection-grade control; primary frequency response or
synthetic inertia; grid switching; universal AC-OPF optimality; universal market
qualification or FERC Order 2222 compliance; certified protocol conformance from
adapter implementation alone; safe field dispatch without site integration,
commissioning, cyber review, and HIL validation; safe-by-construction GNN/MARL
outputs.

## Where to read more

| Topic | Location |
|---|---|
| Product definition and scope | Report §0–§2 |
| Architecture and modules | Report §3–§13 |
| Command lifecycle and APIs | Report §14–§15 |
| Security, edge, simulation | Report §16–§19 |
| Operator app and deployment | Report §20–§21 |
| Roadmap and first build | Report §22–§24 |
| Decisions, glossary, sources | Report §25–§28 |
| Mandatory system conventions | `docs/foundations/...v1.0` §6 |
