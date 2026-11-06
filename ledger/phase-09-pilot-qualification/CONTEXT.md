# Phase 9 — Customer Pilot Qualification: Context

**Phase:** 9 (Stage 9, report §23)
**Ledger:** `ledger_phase_9.md` (P9.01–P9.30)

## 1. Background

Phases 0–8 built FlexCore as a simulation-first platform: the 0.x line is simulation-only,
v0.4.0 added the first site integration (read-only / test-lab by default), and v0.5.0 added
the plugin SDK. Phase 9 is the first phase that touches a real customer deployment with a
view to real control. Its job is qualification, not construction: prove, for one named
customer at named sites, that the integration, the network design, the security posture, the
OT/IT boundary, the commissioning, the HIL evidence, and the field acceptance all support
operator-approved control — and get that approved by the customer/site and the applicable
authorities (§23 Stage 9 exit gate).

Nothing in this phase grants field authority by itself. A version number does not imply
regulatory certification or operational qualification (§2.1). Promotion to a higher-authority
mode requires the §22.3 release gates plus site-specific acceptance (§21.4).

## 2. Key report sections to re-read

| Section | Why it matters here |
|---|---|
| §23 Stage 9 | The phase definition: deliverables and exit gate |
| §22.3 | Release gates; the pilot additionally requires site owner approval, device/vendor integration tests, local safety review, commissioning, and applicable market/interconnection qualification |
| §17.1–§17.5 | Security model, zones, required controls, threat model, safety invariants (all 10 invariants apply; invariant 9 and 10 are exercised in P9.18/P9.20) |
| §18.1–§18.3 | Edge responsibilities, trust boundary (edge is not trusted merely for belonging to a site), deployment profile |
| §21.1–§21.4 | Hosting modes, availability/recovery, observability, build form and control-authority modes |
| §19.1–§19.4 | Simulation/test tracks (HIL), test layers, benchmark cases, evaluation metrics |
| §14.1–§14.5 | Command lifecycle, state definitions, invariants, asset-specific fallback, "dispatch is not hard real-time" |
| §2.3, §11.3, §11.5 | Protocol posture; IEEE 1547-2018 capabilities; OCPP 2.1 scope |
| §10.1–§10.5 | Domain model, units/sign, time model, telemetry quality, schema governance |
| §15, §16 | APIs/events/messaging; storage and tenancy (RLS defence in depth) |
| §20.1–§20.2 | Operator console workspaces and design principles |
| §25, §25.2 | Pilot release definition; deferred decisions D1–D6 and their gates |
| §24 | The vertical slice this phase re-verifies (see `PLAN.md` §10) |
| §0.3, §1.6 | What is deliberately not promised; product boundaries and responsibility |

## 3. Canonical units, sign, and terminology

- **Units (§10.2):** MW, MVAr, MVA, MWh. `P > 0` = export at the point of connection.
  Reactive-power sign is declared per grid model and device profile, never assumed. SOC is
  a fraction in [0,1] at the API boundary. Voltage is kV at the nominal base, per-unit in
  solver representation. Current is A or per-unit on an explicit base.
- **Telemetry quality (§10.4):** exactly one of `GOOD`, `SUSPECT`, `BAD`, `MISSING`,
  `ESTIMATED`, `STALE` per value.
- **Command lifecycle (§14.1–§14.2):** `PROPOSED → VALIDATED → FEASIBLE → APPROVED →
  ISSUED → ACKNOWLEDGED → TRACKING → COMPLETED`, plus `REJECTED`, `FAILED`, `EXPIRED`,
  `CANCELLED`, `SUPERSEDED`, `FALLBACK`. Validation, feasibility, and authorization are
  distinct gates. Acknowledgement is not physical delivery. `SUPERSEDED` and `CANCELLED`
  are different states.
- **Control authority (§21.4):** Simulation, Read-only/shadow, Operator-approved, Bounded
  automation. Mode is a deployment/per-site setting; a plugin can never select it.
- **Terminology (`.agent/WRITING.md`):** "license" (US spelling); "acknowledgement" (noun);
  "operator-approved control" (not "manual control" or "human-in-the-loop"); "read-only /
  shadow mode" (not "passive mode"); "dispatch command" (not "setpoint" for the lifecycle
  object). "Certified" and "conformant" require a test result.
- **HIL terms:** CHIL (controller hardware-in-the-loop — real controller, simulated grid);
  PHIL (power hardware-in-the-loop — real power equipment via a power amplifier). Do not
  claim PHIL evidence from a CHIL-only run (P9.15).

## 4. Constraints

- **Operator-approved control only at v1.0.0.** The pilot defaults to operator-approved
  control (§2.1). Every command requires an authorized human approval (§21.4).
- **Bounded automation is post-1.0**, per deployment, site, and asset class, after separate
  qualification (§21.4, §25). It is not built, enabled, or qualified in this phase.
- **No certification claims.** FlexCore is not certified, not a safety case, and not
  permission for live grid control. HIL evidence is not equipment certification; adapter
  implementation is not conformance.
- **No protection-grade claims.** FlexCore is supervisory orchestration (§1.6). Protection,
  BMS cell-level protection, local droop/inner-loop control, and site PLC/DCS/SCADA
  interlocks remain with local systems.
- **Simulation-first 0.x.** Field write authority begins at v1.0.0 with operator approval
  (AGENTS.md). Nothing in this phase back-fits write authority into a 0.x release.
- **Solver discipline (AGENTS.md).** HiGHS is LP/QP/MIP, not SOCP. Clarabel is convex conic,
  not MISOCP. AC power flow is not AC-OPF. IPOPT is local, not a global-optimality guarantee.
  These matter in P9.03 (model/provider choice) and P9.19 (validation evidence).
- **D6 before any read-only pilot.** The OT/IT boundary review (P9.06) gates all read-only
  pilot activity at a site.

## 5. Prior-phase outputs consumed

| Output | From | Consumed by |
|---|---|---|
| Stage 6 exit: read-only end-to-end lab integration | Phase 6 | P9.14 (re-verified for the pilot configuration) |
| Plugin SDK, manifest, capability model, permission boundaries | Phase 7 | P9.08, P9.26 (no plugin may hold `dispatch.authorize` or select the mode) |
| Research modules validated against deterministic baselines | Phase 8 | P9.19 (research outputs remain proposals; the protected validation/authorization path is mandatory, §13.4) |
| Command lifecycle, approvals, audit (v0.3.0) | Phase 4 (report §23 Stage 4) | P9.19 (shadow dry run) |
| Operator console workspaces | Phase 5 (report §23 Stage 5) | P9.21 (training on the pilot system) |
| Edge agent, outbound mTLS enrolment, buffering | Phase 6 (report §23 Stage 6) | P9.05, P9.14, P9.24 |

If a prior-phase output is missing or does not match the pilot configuration, that is a
finding in the consuming subtask — not a reason to proceed on an assumed equivalent.

## 6. D1–D6 relevance

| Gate | Relevance to Phase 9 |
|---|---|
| D1 jurisdiction/market | Drives interconnection, market adapter, data residency, liability (§25.2). Must be closed for the deployment jurisdiction before promotion beyond read-only (§22.3). P9.01, P9.10, P9.11, P9.12. |
| D2 first portfolio/service | Determines asset mix, baseline method, acceptance criteria. P9.01, P9.11. |
| D3 network class/model fidelity | Determines the pilot grid model and provider (P9.03). Balanced radial is the recommended default, not a decision. |
| D4 license/dependency audit | Gated before v0.1.0; final closure for the v1.0.0 artifact set is Phase 10 (P10.19). Phase 9 records any new pilot dependency it introduces and hands it to P10.19. |
| D5 first protocol/asset mix | Selects the pilot adapter and conformance tests (P9.04). |
| D6 cyber/OT boundary review | **Closes in this phase (P9.06), before any read-only pilot.** Site-specific; signed by security and the customer OT owner. |

## 7. Open questions

1. Which customer and sites? (P9.01 — unanswered until the pilot is named; nothing else in
   this phase can be executed against a real site before it.)
2. Which RTO/ISO or market/program, and what is its Order 2222 implementation status on the
   pilot's go-live date? (P9.11; see `RESEARCH.md` — implementation is staggered to 2030.)
3. Which HIL platform and lab, and is CHIL sufficient or is PHIL required for the pilot
   equipment? (P9.15.)
4. Which interconnection standard/profile applies per site as locally adopted (IEEE 1547-2018
   category and settings)? (P9.12.)
5. What are the customer's contractual incident-notification deadlines? (P9.23.)
6. What are the customer's RPO/RTO requirements, against which the measured restore figures
   are judged? (P9.24 — §21.2 forbids inventing them.)
7. Does the Rev. 4 draft of NIST SP 800-82 (comments due 30 November 2026) change anything
   for this deployment once finalised? (P9.07 — tracked as a draft, re-checked in Phase 10.)
