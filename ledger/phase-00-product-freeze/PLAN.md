# PLAN — Phase 0: Product and Domain Freeze

| Field | Value |
|---|---|
| Phase | Phase 0 — Report Stage 0 |
| Report trace | §23 Stage 0; §25.2; §22.1; §0–§2; §10; §14; §17 |
| Ledger | `ledger_phase_0.md` (P0.01–P0.30) |
| Status | Planning only — no code, no implementation |

## 1. Scope

### In scope

- One authoritative scope/non-goals note (§0–§2) that later phases test scope creep against.
- First buyer and first use-case dispositions (§1.3–§1.5).
- D1, D2, D3 closed or scheduled with owner, venue, and date; D4, D5, D6 scheduled with owner, venue, and date (§25.2).
- Frozen contracts: canonical units/sign (§10.2), telemetry quality (§10.4), time model (§10.3), asset/capability vocabulary (§10–§11), command state machine (§14), grid-model contract (§12.4–§12.5), solver-class discipline (§7.4, §12.3), event vocabulary (§15.3), API surface (§15.1–§15.2), tenancy/storage principles (§16), threat model and zones (§17), control-authority modes (§21.4), plugin model and protected kernel (§5), deployment/build form (§21), console scope (§20).
- ADRs ADR-001…ADR-012 in proposed status (§22.1).
- Simulation/testing/benchmark strategy as a plan (§19).
- The §24 vertical-slice map (steps → stages).
- Foundations cross-check (foundations v1.0 §6 governs on conflict).
- Stage 0 exit-gate review and handover pack for Phase 1.

### Non-goals (must NOT be built or decided here)

- No Rust workspace, crates, API, database, migrations, NATS streams, telemetry ingestion, registry, simulator, console, plugin host, or solver integration. AGENTS.md §1 is explicit: the repo is documentation-only and crates are introduced only when they have real code, clear ownership, and a stable dependency direction.
- No jurisdiction/market decision unless D1 is genuinely decided in its venue (P0.03); no license decision unless D4 closes (it does not close in Phase 0 — its gate is "before v0.1.0").
- No protocol selection (D5, gate before v0.4.0); no cyber/OT boundary review (D6, site-specific).
- No safety, certification, protection-grade, or protocol-conformance claims.
- No performance, scale, or availability targets (RPO/RTO values require customer requirements, §21.2).
- No acceptance thresholds for benchmarks (§19.4 — thresholds are set per benchmark later, after the model/service/benchmark/risk are defined).
- No edits to `docs/foundations/*`, no edits to the report, no commits or pushes from this ledger work.

## 2. Architecture decisions in play

| Decision | Status in Phase 0 | Report trace |
|---|---|---|
| Product is VPP/DER orchestration, not ADMS/SCADA | Frozen as ADR-001 (proposed) | §22.1 ADR-001; §0 |
| Rust modular monolith control plane | Frozen as ADR-002 (proposed); implementation is Phase 1 | §22.1 ADR-002; §6.1 |
| Site edge as separate deployment/security boundary | Frozen as ADR-003 (proposed) | §22.1 ADR-003; §3.3 |
| Python isolated to numerical/research workers | Frozen as ADR-004 (proposed) | §22.1 ADR-004; §7.2 |
| Plugin model: capability seams, protected kernel | Frozen as ADR-005 (proposed) | §22.1 ADR-005; §5 |
| No arbitrary Rust dylib loading | Frozen as ADR-006 (proposed) | §22.1 ADR-006; §5.4 |
| PostgreSQL source of truth; NATS JetStream transport | Frozen as ADR-007 (proposed); implementation is Phase 1 | §22.1 ADR-007; §16.1 |
| Protobuf/gRPC internal; REST/OpenAPI external | Frozen as ADR-008 (proposed) | §22.1 ADR-008; §15.1 |
| Deterministic optimizer + independent feasibility validation | Frozen as ADR-009 (proposed); implementation is Phases 3–4 | §22.1 ADR-009; §12.5 |
| ML/RL outputs are proposals only | Frozen as ADR-010 (proposed) | §22.1 ADR-010; §13 |
| Market adapters region-specific; Order 2222 is not a protocol | Frozen as ADR-011 (proposed) | §22.1 ADR-011; §2.3 |
| No hard-real-time or protection claims | Frozen as ADR-012 (proposed) | §22.1 ADR-012; §0.3 |
| D1 jurisdiction/market | OPEN — close or schedule (P0.03) | §25.2 |
| D2 first portfolio/service | OPEN — close or schedule (P0.04) | §25.2 |
| D3 network class/fidelity | OPEN — close or schedule (P0.05) | §25.2 |
| D4 license/dependency audit | OPEN — schedule only (gate before v0.1.0) | §25.2 |
| D5 first protocol/asset mix | OPEN — schedule only (gate before v0.4.0) | §25.2 |
| D6 cyber/OT boundary review | OPEN — schedule only (gate before any read-only pilot) | §25.2 |

All twelve ADRs are "proposed" until the Stage 0 exit-gate review (P0.29). None is "accepted" before the gate.

## 3. Component breakdown

Phase 0 produces documents only. Components are freeze records, each owned by the subtask that writes it:

| Component | Subtasks | Consumed by |
|---|---|---|
| Scope and non-goals note | P0.01 | Every later phase gate (scope-creep test) |
| First buyer / use-case dispositions | P0.02 | Phase 2 (registry), §24 slice |
| D1–D6 decision/gate records | P0.03–P0.06, P0.28 | Phase 1 entry; v0.1.0 release (D4); Stage 6 (D5); pilot (D6) |
| Units/sign contract | P0.07 | Phase 1 API contracts; Phase 2 telemetry model |
| Quality vocabulary | P0.08 | Phase 2 telemetry model and quality detection |
| Time model | P0.09 | Phase 1/2 schemas (event_time vs ingest_time) |
| Asset/capability vocabulary | P0.10 | Phase 2 registry schema |
| Command state machine | P0.11 | Phase 4 dispatch implementation (test oracle) |
| Grid-model contract | P0.12 | Phase 4 feasibility validation |
| Solver discipline | P0.13 | Phase 3 scheduler; any solver-facing text |
| Event vocabulary | P0.14 | Phase 1 outbox/events |
| API surface | P0.15 | Phase 1 Axum routes |
| Tenancy/storage principles | P0.16 | Phase 1 migrations and RLS |
| Threat model | P0.17 | Phase 1 security tests |
| Control-authority note | P0.18 | Every phase gate; Phase 4 dispatch |
| Plugin model | P0.19 | Phase 7 plugin SDK |
| Deployment/build form | P0.20 | Phase 1 Compose; deployment work |
| ADR set (001–012) | P0.21 | All later architecture work |
| Console scope | P0.22 | Phase 5 console |
| Test strategy | P0.23 | All later test work |
| Vertical-slice map | P0.24 | Stage gates 1–5 |
| Cut-off check | P0.25 | Each later phase gate (repeat) |
| Foundations cross-check | P0.26 | Any report/foundations conflict |
| Handover pack | P0.27 | Phase 1 entry |
| Gate table + exit-gate review | P0.28–P0.29 | Phase 1 authorisation |
| Anti-scope audit | P0.30 | Phase 1 entry checklist |

## 4. Data contracts

Phase 0 defines contracts as frozen text/tables, not as code. The contracts and their authoritative homes:

| Contract | Home | Format |
|---|---|---|
| Canonical units/sign | Phase 0 freeze record (P0.07) | Normative table |
| Telemetry quality (6 states) | Phase 0 freeze record (P0.08) | Normative table |
| Time model | Phase 0 freeze record (P0.09) | Field definitions |
| Asset/capability vocabulary | Phase 0 freeze record (P0.10) | Entity/field tables |
| Command lifecycle | Phase 0 freeze record (P0.11) | Transition table + invariants |
| Grid-model contract | Phase 0 freeze record (P0.12) | Field + rule list |
| Event vocabulary (26 events) | Phase 0 freeze record (P0.14) | Name list |
| REST resource list (16 resources) | Phase 0 freeze record (P0.15) | Path list |

Rule: later phases cite these contracts by ID; they do not re-define them. A change to a frozen contract requires a new ADR and a re-run of the affected gate, not a quiet edit.

## 5. Interfaces

- **Reviewers:** product, power systems, platform engineering, security/OT, legal (§Document control). The Stage 0 exit gate requires engineering, product, and power-systems reviewers (§23).
- **Decision venues:** D1 product + legal; D2 product; D3 power systems; D4 legal + engineering; D5 product + integration; D6 security (§25.2).
- **Inputs:** report v1.1 (authoritative product baseline); foundations v1.0 (governs on conflict); `.agent/DECISIONS.md` (D1–D6 status); `.agent/WRITING.md` (evidence, units, terminology rules); `RESEARCH.md` (primary-source checks).
- **Outputs:** the freeze record, the ADR set, the gate table, the handover pack — all under `ledger/phase-00-product-freeze/` plus updates to `.agent/DECISIONS.md` and (when the gate passes) ADR files under `docs/architecture/adr/`.

## 6. Sequencing

1. Scope + buyer/use cases (P0.01–P0.02) — everything else hangs off these.
2. Decisions D1–D6 (P0.03–P0.06) in parallel with the contracts, but D1/D2/D3 must be at least scheduled before the exit gate.
3. Contracts in dependency order: units/sign (P0.07) → quality (P0.08) + time (P0.09) → asset vocabulary (P0.10) → command machine (P0.11) → grid model (P0.12) → solver discipline (P0.13) → events (P0.14) → API surface (P0.15) → tenancy/storage (P0.16) → threat model (P0.17) → control authority (P0.18) → plugin model (P0.19) → deployment (P0.20).
4. ADRs (P0.21) once the contracts they record exist; console scope (P0.22), test strategy (P0.23), slice map (P0.24) in parallel.
5. Verification: cut-off check (P0.25), foundations cross-check (P0.26).
6. Handover (P0.27), gate table (P0.28), exit-gate review (P0.29), anti-scope audit (P0.30).

Relative effort: this is a review-and-writing phase, not a build phase. The dominant cost is reviewer time (five reviewer groups) and the D1–D3 decision venues, not authoring. No calendar dates are asserted here; dates are assigned in P0.28 and recorded in `.agent/DECISIONS.md`.

## 7. Roles

| Role | Responsibilities |
|---|---|
| Product | Scope, buyer/use cases, D1/D2 ownership, console scope |
| Power systems | Units/sign, capability vocabulary, grid model, solver discipline, D3 ownership, test strategy (benchmarks) |
| Platform engineering | Time model, events, API surface, tenancy/storage, plugin model, deployment, handover |
| Security | Threat model, D6 ownership, §17.3 controls review |
| Legal | D1 (with product), D4 (with engineering) |
| Principal engineering | ADR authorship, foundations cross-check, exit-gate chair, handover |

## 8. Risks and mitigations

| Risk | Mitigation |
|---|---|
| A D1–D3 decision is "assumed" because a default exists | Defaults are labelled recommendations; the gate table (P0.28) blocks Stage 1 until each is decided or scheduled |
| Scope note paraphrased into marketing language | P0.01 acceptance criteria require the §0.3 list verbatim |
| A frozen contract is quietly re-defined in a later phase | Citation-by-ID rule (§4); Phase 1/2 acceptance criteria cite P0 contract IDs |
| Foundations/report conflict discovered late | P0.26 cross-check before the exit gate; foundations govern |
| Reviewer group absent from the gate | P0.29 requires all three gate groups present or a recorded delegate |
| Freeze mistaken for implementation | P0.30 anti-scope audit; handover pack states entry conditions |

## 9. What must NOT be built yet

Anything in §1 of this plan's non-goals. In particular: no code, no crates, no database, no messaging, no telemetry, no solver, no console. The §24 vertical slice is mapped, not built.

## 10. How the §24 vertical slice lands here

The slice contributes nothing implemented in Phase 0. Phase 0's contribution is the vocabulary the slice runs on (assets, units, quality, command states, events) and the stage map (P0.24) that shows steps 1–2 landing in Stage 2, steps 3–4 in Stage 3, steps 5–8 in Stage 4, step 9 in Stage 5, and step 10 in Stages 2/5 — with the report's own caveat that the slice spans v0.1.0–v0.3.0 and is not all promised in v0.1.0.
