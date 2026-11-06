# Plan — Phase 4: Grid Feasibility and Dispatch Assurance

- **Report:** `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` (§23 Stage 4)
- **Release target:** v0.3.0, simulation and shadow (§2.1, §21.4). Simulation-only dispatch adapter; no device writes.
- **Ledger:** `ledger_phase_4.md` (P4.01–P4.32). This plan is the architecture companion; the ledger is the executable checklist.

## 1. Scope

### In scope

- Grid-model provider selection (pandapower primary, §12.4) behind a port, with a versioned network model and health checks.
- Network feasibility gate (§12.5, steps 1–8): candidate validation, capability/ramp checks, AC power-flow evaluation, voltage/thermal/transformer/equipment limits, margins, explicit non-convergence and coverage reporting, reject/constrain/return routing, fail-closed policy.
- SOCP relaxation boundary for Tier 2 candidates (§12.5).
- Command lifecycle (§14.1–§14.2): full state machine, validation gate, feasibility gate, human approval/authorization gate, issue/acknowledgement/tracking via the simulation-only dispatch adapter, expiry/cancellation/supersede, per-asset fallback (§14.4).
- §14.3 command invariants; §14.5 timing honesty; §17.5 safety invariants.
- Deterministic rejection tests for invalid, stale, infeasible, unauthorized, duplicate, and expired commands (§23 Stage 4 exit gate), plus combined/adversarial cases.
- Audit, events, persistence, observability (§15, §16, §21.3); fault injection, integration, and shadow evaluation (§19.2); handover to Phase 5.

### Non-goals (explicitly out of scope)

- Operator console UI — Phase 5 (§20). Phase 4 exposes APIs and audit data only.
- Real protocol adapters, edge agent, OpenEMS integration — Stage 6. The only dispatch target in this phase is the simulator.
- AC-OPF as a scheduler or validator (§12.3, §12.5). AC power flow evaluates; it does not optimize.
- Tier 3/4, GNN, MARL, ADMM (§13, Stage 8).
- Operator-approved field control — v1.0.0 default (§21.4), not this phase. Approval here authorizes a simulated issue.
- Hard real-time, protection-grade, frequency/inertia/relay claims (§14.5).
- Certified protocol conformance from adapter implementation alone (§0.3, §17.3).

## 2. Architecture decisions in play

| Decision | Position in this phase | Report trace |
|---|---|---|
| One primary deterministic path (pandapower) | Other providers only on a named test case | §12.4, §25.2 |
| Grid-model port, not direct pandapower calls | Feasibility depends on the contract; adapters are replaceable | §9.1, §5.2 |
| AC power flow is evaluation, never optimization | Non-convergence is a reported result; no silent retry to force convergence | §12.5, §7.4 |
| Fail-closed | Validator failure, stale/incomplete model, missing telemetry → REJECT or HOLD, never a pass | §12.5, §17.5 inv. 9 |
| Validation, feasibility, authorization are distinct gates | Separate evidence references; APPROVED unreachable without FEASIBLE | §14.1, §17.5 inv. 3–4 |
| State machine is exhaustive and typed | Illegal transitions are rejected, not ignored | §14.1, §7.1 |
| No universal fallback | Per-asset/site policies only; hold is bounded and conditional | §14.4 |
| Simulation-only dispatch | Adapter target is the simulator; a non-simulator target fails a guard test | §23 Stage 4, §21.4 |

## 3. Component breakdown

| Component | Owner crate/worker (per §8.1) | Ledger |
|---|---|---|
| Grid-model port | `flexcore-grid-api` | P4.02 |
| pandapower adapter | `python/workers/powerflow-pandapower/` | P4.01–P4.02 |
| Network model store + health | `flexcore-domain`, `flexcore-storage` | P4.03 |
| Feasibility gate pipeline | `flexcore-application` (Feasibility context) | P4.04–P4.09 |
| Command state machine + invariants | `flexcore-domain` | P4.10, P4.14–P4.15 |
| Validation / feasibility / authorization gates | `flexcore-application` (Dispatch context), `flexcore-authz` | P4.11–P4.13 |
| Simulator dispatch adapter | `adapters/` (simulator) + `apps/worker` | P4.16 |
| Fallback policies | `flexcore-domain` + site configuration | P4.17–P4.18 |
| Rejection suites | `scenarios/` + test suites | P4.20–P4.27 |
| Security / invariants | `flexcore-authz`, `flexcore-audit`, protected kernel | P4.28–P4.29 |
| Audit/events/persistence/observability | `flexcore-audit`, `flexcore-events`, `apps/control-plane` | P4.30 |

Dependency direction follows §6.2/§8.2. The protected kernel (authorization, tenant isolation, audit integrity, command lifecycle, final dispatch validation — §5.7) is never replaced by a plugin.

## 4. Data contracts

| Contract | Fields (minimum) | Versioned by |
|---|---|---|
| Network model | buses, branches, transformers, ratings, voltage bases, version + content hash, completeness/staleness/coverage health | Model version + hash (P4.03) |
| Feasibility result | candidate ID, model version + hash, provider + version, convergence status, iterations, residuals, per-limit pass/fail + value + limit + margin, worst margin, binding constraint, coverage limitations, outcome (pass/reject/constrain/return) | Result record (P4.07) |
| Dispatch command | all §14.3 fields (see P4.14), state, validation evidence ref, feasibility evidence ref, authorization decision ref (scope, limits, expiry), signature/transport identity, supersedes link | Command ID + sequence/version (P4.14) |
| Fallback policy | per-asset/site policy type, parameters, bounded-hold interval, validity conditions, operator-review reference | Policy version (P4.17) |
| Audit event | transition, actor, reason, evidence refs, correlation IDs (§21.3) | Event record (P4.30) |

Canonical units (§10.2): MW, MVAr, MVA, MWh; `P > 0` export; SOC [0,1]; telemetry quality GOOD / SUSPECT / BAD / MISSING / ESTIMATED / STALE.

## 5. Interfaces

- **Inbound:** REST `/api/v1/feasibility-checks`, `/api/v1/dispatch-commands` (§15.2); NATS events consumed: `ScheduleProposed`, `ScheduleValidated`, `TelemetryReceived`, `TelemetryQualityChanged`.
- **Outbound:** events `FeasibilityCheckCompleted`, `DispatchProposed`, `DispatchApproved`, `DispatchRejected`, `DispatchIssued`, `DispatchAcknowledged`, `DispatchTrackingUpdated`, `DispatchCompleted`, `DispatchFailed`, `DispatchExpired`, `DispatchCancelled`, `DispatchSuperseded`, `DispatchFallbackActivated` (§15.3) via transactional outbox (§15.5).
- **Internal:** grid-model port (§9.1) between the feasibility application service and the pandapower Python worker (gRPC/Protobuf, §15.1); command lifecycle in the Rust domain; simulator adapter as the sole dispatch target.
- **Consumed from Phase 3:** schedule ID + revision, envelope version, §12.6 result records. **Provided to Phase 5:** command lifecycle states, feasibility evidence, audit trail, simulator integration points.

## 6. Sequencing

1. Provider selection + port + versioned model (P4.01–P4.03).
2. Feasibility pipeline steps 1–8 + fail-closed + SOCP boundary (P4.04–P4.09).
3. State machine + creation/validation/feasibility/authorization gates + invariants (P4.10–P4.14).
4. Expiry/cancel/supersede + simulator adapter + fallback (P4.15–P4.18).
5. Timing honesty (P4.19).
6. Deterministic rejection suites + evidence pack (P4.20–P4.27) — the exit gate; run in full, twice, from clean state.
7. Security controls + invariant matrix (P4.28–P4.29).
8. Audit/events/persistence/observability + fault injection/shadow (P4.30–P4.31).
9. Handover (P4.32).

Relative effort (no calendar dates): the feasibility pipeline and the rejection suites are the largest shares; the state machine is precise but bounded; the simulator adapter is small by design (one target, no protocol work). Do not shrink the rejection suites to "save time" — they are the exit gate.

## 7. Roles

| Role | Owns |
|---|---|
| Architect | Provider selection, fail-closed policy, invariant matrix, handover, exit-gate sign-off |
| Power-systems engineer | Network model, limit sets, feasibility pipeline, SOCP boundary, fallback policies, infeasible-suite design |
| Rust engineer | Domain state machine, gates, adapter interface, API, persistence, outbox |
| Numerical (Python) engineer | pandapower adapter, power-flow execution, diagnostics |
| QA | Rejection suites, fault injection, determinism, evidence pack |
| Security engineer | P4.13 authorization, P4.28 controls, P4.29 invariants |
| Product | Approval-workflow expectations; D1/D2/D3 status tracking |

## 8. Risks and mitigations

| Risk | Mitigation |
|---|---|
| AC power flow mistaken for AC-OPF | Stated in code/docs; no optimization in the validation path (P4.05) |
| Stale/incomplete model treated as a pass | Fail-closed policy + health checks (P4.03, P4.08) |
| SOCP relaxation assumed exact | Condition checklist + independent power-flow verification (P4.09) |
| Validation/feasibility/authorization conflated | Separate evidence references; state-machine preconditions (P4.11–P4.13) |
| Acknowledgement mistaken for physical delivery | State definitions enforced in code and display contracts (P4.16, Phase 5 P5.13) |
| Universal fallback smuggled in | No universal fallback in code; per-asset policies only (P4.17) |
| Simulator target swapped for a real device | Target guard test (P4.16) + invariant 6 test (P4.29) |
| Rejection suite fitted after the run | Pre-registered expectations; full-pack re-runs (P4.20–P4.27) |

## 9. What must NOT be built yet

- Any console UI (Phase 5).
- Any real protocol/edge adapter (Stage 6).
- AC-OPF, Tier 3/4, GNN/MARL/ADMM (Stage 8).
- Field control authority (v1.0.0).
- Kubernetes/Helm deployment artefacts (§21.1 — after core contracts stabilize; not this phase's work).

## 10. How the §24 vertical slice lands here

§24 steps 5–8 land in this phase: "validate the candidate against asset capability and a feeder power-flow model" (P4.04–P4.07); "create a PROPOSED command and demonstrate validation/rejection/approval transitions" (P4.11–P4.13); "send it only to a simulator adapter" (P4.16); "simulate acknowledgement, tracking, partial failure, and timeout" (P4.16, P4.31). Steps 9–10 (display audit trail, replay reproducibility) are Phase 5.
