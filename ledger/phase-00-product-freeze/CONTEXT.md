# CONTEXT — Phase 0: Product and Domain Freeze

## Background

Neryva FlexCore is an open-source, modular Virtual Power Plant orchestration and
distribution-aware DER coordination platform (report §0–§1). It aggregates
heterogeneous distributed energy resources into a portfolio, estimates available
flexibility, builds rolling-horizon schedules, validates them against asset and
network constraints, and manages the authorization → issue → acknowledgement →
measured-response lifecycle of dispatch commands.

Phase 0 is Report Stage 0 (§23): freeze the product and the domain before any
code exists. The repository is currently documentation-only (AGENTS.md §1).
Phase 0 produces frozen contracts, decisions (or scheduled decisions), and ADRs
— not software.

## Key report sections to re-read

| Section | Why it matters here |
|---|---|
| §0 Executive decision | Product statement, release path, what is deliberately not promised |
| §1 Product definition and application | Audiences, operating scenario, use cases, responsibility boundary |
| §2 Scope by release | Release ladder, initial asset support, protocol posture |
| §3 System context | Architecture style, context/deployment diagrams, primary workflow, time scales |
| §4 Core product modules | Sixteen bounded contexts and their ownership |
| §5 Plugin-oriented architecture | Capability seams, execution classes, manifest, lifecycle, authority, profiles |
| §6 Software architecture | DDD + hexagonal + modular monolith, dependency rule, outbox, storage strategy |
| §10 Domain model and data contracts | Canonical entities, units/sign, time model, telemetry quality, Protobuf governance |
| §11 Capability and flexibility modelling | Generic DER model, BESS, inverter P–Q, flexible load, EV fleet, aggregation |
| §12 Scheduling, optimization, grid validation | Objectives, tiers, provider selection, feasibility gate, result contract |
| §14 Command lifecycle | Protected state machine, state definitions, invariants, fallback |
| §15 APIs, events, messaging | API styles, REST resources, event vocabulary, NATS semantics, outbox |
| §16 Data storage and tenancy | Storage choices, multi-tenancy, RLS caveat, retention |
| §17 Security, safety, operational trust | NIST reference, zones, controls, threat model, safety invariants |
| §19 Simulation, testing, benchmarks | Tracks, test layers, benchmark cases, metrics, no-arbitrary-targets rule |
| §20 Operator application | Twelve workspaces, display rules |
| §21 Deployment and operations | Hosting modes, availability/recovery, observability, build form, control authority |
| §22 Governance and quality gates | ADR list, CI gates, release gates |
| §23 Stage 0 | Deliverables and exit gate for this phase |
| §24 Build-first recommendation | The ten-step vertical slice and its staging |
| §25 Recommended baseline decisions | Provisional choices (§25.1) and deferred decisions (§25.2, D1–D6) |
| §26 Glossary | DER, DERMS, VPP, BESS, PCC, DSO, AC-PF/AC-OPF, SOCP/MISOCP, DMPC/ADMM, GNN, MARL, CTDE, HIL, OT |
| §27 References | [R1]–[R29]; research cut-off 7 November 2026 |
| Foundations v1.0 §6 | Mandatory system conventions — governs where it conflicts with the report |

## Canonical units and sign (frozen in P0.07)

- Active power: **MW**. Reactive power: **MVAr**. Apparent power: **MVA**. Energy: **MWh**.
- Voltage: kV at the nominal base; per-unit in solver representation, base declared explicitly.
- Current: A or per-unit on an explicit base. Frequency: Hz.
- **P > 0 means export** (injection into the grid) at the point of connection; P < 0 means import/consumption.
- Reactive-power sign is **declared per grid model and device profile**, never assumed. Each adapter records the mapping it applies.
- **SOC is a fraction in [0,1]** at the API boundary. Percentages exist only as display or vendor-source values, converted at the adapter.
- BESS net power: P_net = P_dis − P_ch (both ≥ 0).
- Persisted values and API contracts use canonical units. The UI may display kW/kWh but must not persist them as canonical.

## Telemetry quality (frozen in P0.08)

Exactly one state per value: `GOOD`, `SUSPECT`, `BAD`, `MISSING`, `ESTIMATED`, `STALE`.

| State | Meaning |
|---|---|
| `GOOD` | Valid, fresh, within range, from a trusted source |
| `SUSPECT` | Received and parsed, but a plausibility, range, or consistency check failed |
| `BAD` | Source reports the value invalid, or a hard validation rule failed |
| `MISSING` | No value received for the expected interval |
| `ESTIMATED` | Derived, interpolated, substituted, or model-generated rather than measured |
| `STALE` | Last known value whose age exceeds the use-case freshness limit |

Richer source states (`SUBSTITUTED`, `OUT_OF_RANGE`, `UNSYNCHRONIZED`,
`COMMUNICATION_LOSS`, `UNRELIABLE`) map to one of the six at the adapter
boundary, with the original source code retained as diagnostic metadata. A
quality flag must not be silently converted into a normal reading.

## Terminology

- Use "license" (US spelling); "acknowledgement" (noun); "operator-approved
  control" (not "manual control" / "human-in-the-loop"); "read-only / shadow
  mode" (not "passive mode"); "dispatch command" (not "setpoint" for the
  lifecycle object); `SUPERSEDED` (not "replaced" as a state name).
- `SUPERSEDED` and `CANCELLED` are different states. Validation, feasibility,
  and authorization are different gates. Acknowledgement is not physical
  delivery.
- "Certified" and "conformant" require a test result. Implementing an adapter
  is neither.
- Solver discipline: HiGHS is LP/QP/MIP, not SOCP. Clarabel is convex conic
  (including SOCP), not MISOCP. AC power flow is not AC-OPF. IPOPT is local,
  not a global-optimality guarantee.
- Control-authority modes: Simulation; Read-only / shadow; Operator-approved;
  Bounded automation. Mode is a deployment/per-site setting; a plugin can
  never select it.

## Constraints (simulation-only 0.x)

- The entire 0.x line is simulation-only (§0, §2.1, §21.4). Field write
  authority begins at v1.0.0 with operator approval. Bounded automation is
  post-1.0, per deployment, site, and asset class, after qualification.
- A version number does not imply regulatory certification or operational
  qualification.
- FlexCore is not certified, not a safety case, and not permission for live
  grid control. It performs no primary frequency response, synthetic inertia,
  relay protection, inverter inner-loop control, or sub-cycle control.
- No hard-real-time claims: Tokio, Rust, gRPC, NATS, or a cloud service do not
  provide hard-real-time guarantees (§3.5).

## Prior-phase outputs consumed

None — Phase 0 is the first phase. Its inputs are the report v1.1, the
foundations specification v1.0, and the `.agent/` digests (which are never
authoritative on their own; AGENTS.md §2 rank 3).

## D1–D6 relevance

| Gate | Phase 0 action | Owner |
|---|---|---|
| D1 Target jurisdiction and market | Close or schedule (P0.03) | Product + legal |
| D2 First portfolio and service | Close or schedule (P0.04) | Product |
| D3 Network class and model fidelity | Close or schedule (P0.05) | Power systems |
| D4 License and dependency audit | Schedule only; gate before v0.1.0 (P0.06) | Legal + engineering |
| D5 First protocol and asset mix | Schedule only; gate before v0.4.0 (P0.06) | Product + integration |
| D6 Cyber and OT boundary review | Schedule only; gate before any read-only pilot (P0.06) | Security |

Recommended defaults (planning only, not decisions): balanced radial
distribution feeders for 0.x; pandapower as primary power-flow provider;
unbalanced three-phase from v0.4.0; meshed transmission out of scope;
Apache-2.0 core license (pending D4); Rust core, Python workers, TypeScript UI
(pending §25.1 review triggers).

## Open questions

1. D1–D6: decided or scheduled? (P0.03–P0.06, P0.28.) Nothing else in Phase 0
   may assume their answers.
2. Is the first buyer a VPP/DER aggregator, or another §1.3 audience? (P0.02.)
3. Which §1.5 first-generation use cases are in scope for v0.1.0 vs staged?
   (P0.02 — per §2.1, v0.1.0 delivers canonical models, simulator, telemetry,
   registry, portfolio views, and replay; portfolio visibility is one use case
   within that scope, not the only mandatory one. The §23 Stage 2 exit gate
   concerns telemetry ingestion, not use-case mandating, and must not be cited
   as the source for which use cases are mandatory.)
4. Does the foundations §6 cross-check (P0.26) surface any report conflict
   requiring a deviation ADR?
5. Has anything changed after the 7 November 2026 research cut-off that
   affects a freeze decision? (P0.25 — notably the NIST SP 800-82 Rev. 4
   comment period, open until 30 November 2026, and Rust 1.100 stable,
   planned 12 November 2026.)
