# Phase 9 — Customer Pilot Qualification: Plan

**Phase:** 9 (Stage 9, report §23)
**Release target:** v1.0.0 pilot-ready; operator-approved control by default (§2.1, §21.4)
**Ledger:** `ledger_phase_9.md` (P9.01–P9.30)

## 1. Scope

### In scope

- Qualifying **one named customer deployment at named sites** (P9.01) for operator-approved
  control: customer-specific integration and network design, security assessment, OT/IT
  boundary review (D6 closure), site commissioning plan, HIL and field acceptance tests,
  and market/product-specific operational procedures (§23 Stage 9).
- The full evidence chain from scope freeze to the exit gate: customer/site and applicable
  authorities approve the deployment and its control scope (§23 Stage 9 exit gate).
- Shadow/read-only operation first, then promotion to operator-approved mode per site, only
  after shadow acceptance (P9.17, P9.26; §21.4).

### Non-goals (explicitly out of scope)

- **Bounded automation.** Not built, not enabled, not qualified in this phase. Bounded
  automation is post-1.0, per deployment, site, and asset class, after separate
  qualification (§21.4, §25). The pilot configuration must contain no automation envelope
  (P9.26).
- **Certification of anything.** This phase does not certify FlexCore, any adapter, any
  inverter, or any site. Implementing an adapter is not a conformance claim (§2.3); HIL
  evidence is not equipment certification (§11.3). No text produced in this phase may imply
  otherwise (AGENTS.md non-negotiables).
- **Protection, primary frequency response, synthetic inertia, grid switching, hard-real-time
  control.** These remain with local certified controllers and hardware (§0.3, §1.6).
- **A second customer or additional sites.** Any site or customer beyond P9.01 requires its
  own qualification pass; D6 closure is per deployment (§25.2).
- **Re-running Phases 0–8.** This phase consumes their outputs (notably the Stage 6 lab
  integration gate) and re-verifies them for the pilot configuration (P9.14); it does not
  redo the product build.

## 2. Architecture decisions in play

| Decision | Status in this phase | Report |
|---|---|---|
| D1 target jurisdiction and market | Must be closed (or a written interim position recorded with its change condition) for the deployment jurisdiction before promotion beyond read-only | §25.2 |
| D2 first portfolio and service | Same — the pilot service is one of the §1.5 first-generation use cases | §25.2 |
| D3 network class and model fidelity | Drives the pilot grid model (P9.03) and the provider choice (§12.4) | §25.2 |
| D4 license and dependency audit | Gated before v0.1.0; final closure for the v1.0.0 artifact set is Phase 10 (P10.19), not here | §25.2 |
| D5 first protocol and asset mix | Selects the pilot adapter and conformance tests (P9.04) | §25.2 |
| D6 cyber and OT boundary review | **Must close before any read-only pilot**; site-specific, signed by security and the customer OT owner (P9.06) | §25.2, §22.3 |
| Control-authority mode | Operator-approved is the v1.0.0 default; mode is a deployment/per-site setting, never selectable by a plugin (§21.4, §5.7) | §21.4 |
| Network class default | Balanced radial distribution feeders is the recommended default for 0.x (§25.2) — wrong the moment the pilot requires otherwise; P9.03 records the actual class | §25.2 |

No decision in this phase is recorded as made unless its status says so
(`.agent/DECISIONS.md` rule). Interim positions are recorded with the condition that would
change them.

## 3. Component breakdown

| Component / workstream | Subtasks | Notes |
|---|---|---|
| Scope and registry | P9.01–P9.02 | Freeze the pilot; reconcile the registry against physical reality |
| Network model | P9.03 | Versioned, validated grid model per site; coverage limitations explicit |
| Integration and connectivity | P9.04–P9.05 | Per-adapter design; zone/flow network design, outbound-only |
| Security and boundary | P9.06–P9.10 | D6 closure, security assessment, identity/access, secrets/certs, data governance |
| Market and authorities | P9.11–P9.12 | Program rules; authority engagement tracker |
| Commissioning and test | P9.13–P9.17 | Commissioning plan, lab re-verification, HIL, field acceptance (shadow first) |
| Operational readiness | P9.18–P9.25 | Telemetry/clock verification, shadow command dry run, fallback drills, training, procedures, incident response, backup/restore rehearsal, observability |
| Approval and handover | P9.26–P9.30 | Control-scope document, limitations/risks register, acceptance review, authority pack, exit-gate record |

## 4. Data contracts

- **Canonical units and signs (§10.2):** MW, MVAr, MVA, MWh; `P > 0` = export at the point
  of connection; reactive-power sign declared per grid model and device profile; SOC as a
  fraction in [0,1] at the API boundary. Every adapter mapping is recorded per device
  profile (P9.04).
- **Telemetry quality (§10.4):** exactly one of `GOOD`, `SUSPECT`, `BAD`, `MISSING`,
  `ESTIMATED`, `STALE` per value; source codes retained as diagnostic metadata.
- **Time model (§10.3):** `event_time` vs `ingest_time` distinguished; UTC internally; site
  timezone retained for display.
- **Command invariants (§14.3):** unique ID, tenant/portfolio/site/asset IDs, source and
  schedule revision, validity window, sequence, idempotency key, model/telemetry snapshot
  references, validation and authorization evidence references. Verified in shadow in P9.19.
- **Grid model (§10.1):** versioned; buses, branches, transformers, switches, phases, base
  values, limits. Frozen for acceptance (P9.03, P9.16).
- **Audit (§10.1):** actor, action, object, before/after references, decision, evidence,
  timestamp, correlation ID — for every control-plane and dispatch action (§17.3).

## 5. Interfaces

- **Site EMS / protocol adapters (§5.3, §2.3):** OpenEMS, SunSpec Modbus, OpenADR 3.0,
  OCPP 2.0.1/2.1, IEEE 2030.5 — per the D5 selection; each with a named conformance/
  interoperability suite. No adapter is described as certified without a test result.
- **Edge ↔ central (§3.3, §18):** outbound-initiated mTLS channel from the edge; approved
  commands only; edge validates every command (§18.2).
- **Operator console (§20):** the pilot operator's working surface; proposed/feasible/
  approved/issued/acknowledged/measured states visually distinct (§20.2).
- **Market/program adapter (§5.3):** region-specific; FERC Order 2222 is a regulatory
  framework, not a protocol (§2.3, [R1]).
- **Identity provider (§17.3):** OIDC/OAuth2 integration with the customer IdP; roles from
  authenticated claims, never from untrusted request fields (§16.2).

## 6. Sequencing

1. **Freeze** (P9.01–P9.02): scope, sites, assets, registry reconciliation.
2. **Design** (P9.03–P9.05, P9.11): network model, integration design, site network design,
   market/program requirements — in parallel with the security workstream.
3. **Assess and close the boundary** (P9.06–P9.10): security assessment, D6 closure,
   identity/access, secrets/certs, data governance. **No read-only pilot activity at a site
   before D6 closes for that site.**
4. **Plan** (P9.12–P9.13, P9.16): authority engagement, commissioning plan, field acceptance
   test plan (thresholds set before any run).
5. **Prove in the lab** (P9.14–P9.15): read-only lab integration, then HIL — HIL only after
   simulation, integration, and cyber reviews pass (§19.2 item 10).
6. **Prove in the field, shadow first** (P9.17–P9.20): field acceptance in shadow, telemetry/
   clock verification, command-lifecycle dry run, fallback drills.
7. **Ready the operators** (P9.21–P9.25): training and qualification, operational procedures,
   incident response, backup/restore rehearsal, observability verification.
8. **Approve** (P9.26–P9.30): control-scope document, limitations/risks register, customer
   acceptance review, authority approval pack, exit-gate record and handover to Phase 10.

Relative effort is ordered by dependency, not by calendar: the HIL and field-acceptance
steps dominate elapsed time because they depend on lab access, site access, and authority
response times, none of which this plan controls. No calendar dates are promised in this
ledger; dates are set per deployment in the pilot plan and recorded against the subtasks.

## 7. Roles

| Role | Owns |
|---|---|
| Product | Scope (P9.01), market/program (P9.11), authority engagement (P9.12, P9.29), acceptance review (P9.28), exit gate (P9.30), training (P9.21) |
| Power systems | Network model (P9.03), HIL (P9.15), technical authority submissions |
| Integration engineering | Registry reconciliation (P9.02), integration design (P9.04), commissioning (P9.13), lab/field tests (P9.14, P9.16–P9.17), fallback drills (P9.20) |
| Platform engineering | Network design (P9.05), secrets/certs (P9.09), telemetry verification (P9.18), command dry run (P9.19), backup/restore (P9.24), observability (P9.25) |
| Security | D6 closure (P9.06), security assessment (P9.07), identity/access (P9.08), incident response (P9.23) |
| Legal | Customer agreement, data residency/retention (P9.10), authority submissions |
| Customer (site owner, OT owner, operator) | Reviews and approvals at P9.05–P9.07, P9.13, P9.17, P9.21, P9.28; owns site safety and local control |

## 8. Risks and mitigations

| Risk | Mitigation (in this phase) |
|---|---|
| D1/D2/D3/D5 still open at pilot start | Record interim positions with change conditions (P9.01–P9.04); carry as open risks in P9.27; do not describe them as decided |
| D6 treated as a formality | Requires the customer OT owner's signature and a per-boundary decision (P9.06); blocks all read-only pilot work |
| Lab configuration differs from field | P9.14 re-verifies the pilot configuration; versions frozen for acceptance (P9.16); HIL on pilot firmware versions (P9.15) |
| HIL run before cyber review passes | §19.2 item 10 ordering enforced in P9.15 acceptance criteria |
| Shadow mode skipped to save time | P9.17 acceptance criteria require shadow acceptance before any promotion request |
| Fallback assumed safe | Per-asset/site policies reviewed with the equipment/site operator (P9.20); no universal zero-setpoint fallback (§14.4) |
| Authority approval assumed from silence | P9.29 requires a recorded decision or a named follow-up date; a pending precondition blocks operator-approved operation at that site |
| Pilot scope creep | P9.01 freeze; any addition re-opens qualification for the added scope |

## 9. What must NOT be built yet

- Bounded automation in any form (no automation envelope in the pilot configuration).
- Any new protocol adapter or market adapter beyond the D5 selection.
- Any claim of certification, conformance, or safety-case status for FlexCore, an adapter,
  or a site.
- Production dispatch beyond the approved control scope (P9.26) — including "one more
  site, same config" without qualification.
- A second pilot customer inside this phase.

## 10. How the §24 vertical slice lands here

The §24 vertical slice (tenant → portfolio → site → grid model → four simulated assets →
telemetry with quality flags → flexibility over 24 h → LP/MILP/QP schedule → feeder
power-flow validation → PROPOSED command with validation/rejection/approval transitions →
simulator-only dispatch → acknowledgement/tracking/partial-failure/timeout simulation →
console audit trail → reproducible replay) was the 0.x target slice. In Phase 9 it lands as
the **proven baseline the pilot re-verifies, not as new work**: P9.14 re-runs the slice's
decision chain against the pilot configuration in the lab; P9.19 dry-runs its command
lifecycle in shadow against real site conditions; P9.17 executes its acceptance in the
field. Steps 7–8 of the slice (simulator-only dispatch, simulated acknowledgement) are
replaced at the pilot by real acknowledgement and tracking evidence — the slice's simulated
steps are never presented as field evidence.
