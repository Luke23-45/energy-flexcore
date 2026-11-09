# P0.11 — Command state machine freeze (states, transitions, invariants)

| Field | Value |
|---|---|
| Subtask | P0.11 — Freeze the command state machine (states, transitions, invariants) |
| Report trace | §14.1, §14.2, §14.3, §14.4, §14.5; §0.2 (kernel owns command lifecycle); §5.7 (plugin authority model); §17.5 (safety invariants) |
| Source | `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` (product baseline; foundations specification governs on conflict per AGENTS.md §2) |
| Inputs | P0.07 (canonical units/sign), P0.09 (time model) |
| Status | Draft for Stage 0 review |
| Role of this document | This table is the test oracle for the Phase 4 command state machine: Phase 4 implements and tests exactly the transitions in §2 below — no more, no fewer (ledger P0.11 "Risks / failure modes") |
| Decisions | D1–D6 remain OPEN (§25.2); this document decides none of them |

---

## 1. Scope and authority

This document freezes the protected command lifecycle exactly as report §14 defines it, before any dispatch code exists (ledger P0.11). The command lifecycle is a protected platform invariant: it is owned by the platform kernel and is not replaceable by plugins (§0.2 core principle). Plugins may propose (`dispatch.propose`, candidate only) and may request validation (`dispatch.validate.request`), but `dispatch.authorize` is never granted to general plugins, `dispatch.issue` is owned by the protected dispatch service under a signed authorization token, and the platform kernel owns final authorization and dispatch state transitions (§5.7).

Simulation status: the entire 0.x line is simulation-only; nothing in this document authorises field control or device writes (§0, §2.1, §21.4).

## 2. Transition table (§14.1 — complete extraction)

The table below reproduces every edge of the §14.1 state diagram (`stateDiagram-v2`, report lines 1372–1407). The 14 states are: the 8 forward states PROPOSED, VALIDATED, FEASIBLE, APPROVED, ISSUED, ACKNOWLEDGED, TRACKING, COMPLETED; and REJECTED, FAILED, EXPIRED, CANCELLED, SUPERSEDED, FALLBACK.

State-to-state transitions (27):

| # | From | To | Guard / trigger (as annotated in §14.1) |
|---|---|---|---|
| 1 | PROPOSED | VALIDATED | schema, freshness, asset checks pass |
| 2 | PROPOSED | REJECTED | malformed / unsupported |
| 3 | VALIDATED | FEASIBLE | network checks pass |
| 4 | VALIDATED | REJECTED | capability / network failure |
| 5 | VALIDATED | SUPERSEDED | newer approved schedule/command revision replaces it |
| 6 | FEASIBLE | APPROVED | policy / operator authorization |
| 7 | FEASIBLE | REJECTED | authorization denied |
| 8 | FEASIBLE | SUPERSEDED | newer approved schedule/command revision replaces it |
| 9 | APPROVED | ISSUED | signed, sequenced command sent |
| 10 | APPROVED | EXPIRED | validity ends before issue |
| 11 | APPROVED | SUPERSEDED | newer approved schedule/command revision replaces it |
| 12 | APPROVED | CANCELLED | operator/policy cancellation |
| 13 | ISSUED | ACKNOWLEDGED | device/gateway accepts |
| 14 | ISSUED | FAILED | NACK / timeout / transport failure |
| 15 | ISSUED | EXPIRED | validity ends before acknowledgement |
| 16 | ISSUED | SUPERSEDED | newer approved schedule/command revision replaces it |
| 17 | ISSUED | CANCELLED | supported cancellation semantics |
| 18 | ACKNOWLEDGED | TRACKING | telemetry observed |
| 19 | ACKNOWLEDGED | FAILED | no expected response |
| 20 | ACKNOWLEDGED | EXPIRED | validity ends before tracking completes |
| 21 | ACKNOWLEDGED | SUPERSEDED | newer approved schedule/command revision replaces it |
| 22 | TRACKING | COMPLETED | acceptance criterion satisfied |
| 23 | TRACKING | FAILED | tracking deviation / timeout |
| 24 | TRACKING | EXPIRED | validity ends |
| 25 | TRACKING | SUPERSEDED | newer approved schedule/command revision replaces it |
| 26 | FAILED | FALLBACK | asset/site policy activated |
| 27 | EXPIRED | FALLBACK | policy requires |

Initial and terminal markers (6, as drawn in §14.1; not state-to-state transitions):

| # | Edge | Meaning |
|---|---|---|
| 28 | [*] → PROPOSED | Initial state: every command enters as PROPOSED |
| 29 | COMPLETED → [*] | Terminal |
| 30 | REJECTED → [*] | Terminal |
| 31 | CANCELLED → [*] | Terminal |
| 32 | SUPERSEDED → [*] | Terminal |
| 33 | FALLBACK → [*] | Terminal |

### 2.1 Mechanical diff statement

A mechanical, edge-by-edge diff was performed between the §14.1 mermaid diagram (report lines 1372–1407) and the table above. Every diagram line of the form `A --> B: guard` appears exactly once in the table, with its guard reproduced; no transition has been added that the diagram does not show, and none has been omitted. Outgoing-edge counts per state were cross-checked as a second pass:

| State | Outgoing edges (to) | Count |
|---|---|---|
| PROPOSED | VALIDATED, REJECTED | 2 |
| VALIDATED | FEASIBLE, REJECTED, SUPERSEDED | 3 |
| FEASIBLE | APPROVED, REJECTED, SUPERSEDED | 3 |
| APPROVED | ISSUED, EXPIRED, SUPERSEDED, CANCELLED | 4 |
| ISSUED | ACKNOWLEDGED, FAILED, EXPIRED, SUPERSEDED, CANCELLED | 5 |
| ACKNOWLEDGED | TRACKING, FAILED, EXPIRED, SUPERSEDED | 4 |
| TRACKING | COMPLETED, FAILED, EXPIRED, SUPERSEDED | 4 |
| FAILED | FALLBACK | 1 |
| EXPIRED | FALLBACK | 1 |
| COMPLETED / REJECTED / CANCELLED / SUPERSEDED / FALLBACK | (terminal — no outgoing edges in §14.1) | 0 |

Total: 27 state-to-state edges + 1 initial + 5 terminal = 33 diagram transitions, matching the diagram line count.

### 2.2 Properties of the diagram worth recording

Recorded because later phases must not "fix" them by adding shortcuts:

- **No shortcut transitions.** The diagram shows no PROPOSED → FEASIBLE, PROPOSED → ISSUED, VALIDATED → APPROVED, or similar convenience edges. Each of the three gates is its own state transition (§14.1; §6.4 State machine row: every transition has explicit preconditions, timeout, and audit event).
- **FAILED and EXPIRED exit only via FALLBACK** in the diagram — the only outgoing edge drawn from each is FAILED → FALLBACK / EXPIRED → FALLBACK; the diagram does not draw a direct FAILED → [*] or EXPIRED → [*] edge. FALLBACK is terminal.
- **Three rejection points, not one:** PROPOSED → REJECTED (malformed / unsupported), VALIDATED → REJECTED (capability / network failure), and FEASIBLE → REJECTED (authorization denied). The ledger's illustrative list names the first two as examples; the diagram also shows the third, and it is included above.
- **CANCELLED is reachable only from APPROVED and ISSUED** (operator/policy cancellation, or supported cancellation semantics), consistent with the §14.2 definition ("before issue or while supported").
- **SUPERSEDED is reachable from six states:** VALIDATED, FEASIBLE, APPROVED, ISSUED, ACKNOWLEDGED, and TRACKING — i.e. any pre-completion state can be invalidated by a newer approved revision.
- **EXPIRED is reachable from four states:** APPROVED, ISSUED, ACKNOWLEDGED, TRACKING — the validity window can lapse at any point after approval before completion.

## 3. State definitions (§14.2 — verbatim in substance)

| State | Meaning (§14.2) |
|---|---|
| PROPOSED | Candidate setpoint/schedule generated by an optimizer, policy, adapter, or operator |
| VALIDATED | Schema, freshness, asset capability, ramp, SoC, and command policy checks pass |
| FEASIBLE | Relevant network model check passes within declared model scope and tolerances |
| APPROVED | Required human/policy authorization granted |
| ISSUED | Signed command was sent to the selected gateway/adapter |
| ACKNOWLEDGED | Gateway/device accepted the command at the application layer |
| TRACKING | Measured response is compared with requested behaviour |
| COMPLETED | Defined delivery criterion satisfied |
| REJECTED | Candidate rejected before issue |
| FAILED | Execution/communication/tracking failed |
| EXPIRED | Validity window ended before execution, application-level acceptance, or completion |
| CANCELLED | Explicitly cancelled by an operator or policy before issue or while supported |
| SUPERSEDED | Invalidated by a newer approved schedule/command revision before completion; retained for audit and distinct from explicit cancellation |
| FALLBACK | Resource/site-specific fallback policy activated |

### 3.1 SUPERSEDED is not CANCELLED

SUPERSEDED and CANCELLED are different states and must not be merged (`.agent/WRITING.md`; §14.2; P0.26 records this distinction as a compatible refinement of the foundations row):

- **CANCELLED** — explicitly cancelled by an operator or policy before issue or while supported (§14.2). The act of cancellation is the cause.
- **SUPERSEDED** — invalidated by a newer approved schedule/command revision before completion, retained for audit, and explicitly distinct from explicit cancellation (§14.2). The replacement, not an operator cancellation, is the cause; the record is kept for audit.

A command that was superseded was not cancelled, and a cancelled command was not superseded. Phase 4 must implement, display, and audit them as separate states.

## 4. The three-gate distinction

Validation, feasibility, and authorization are distinct (§14.1, closing statement):

| Gate | Produces | What it is not |
|---|---|---|
| Validation (→ VALIDATED) | Schema, freshness, asset capability, ramp, SoC, and command-policy checks passed (§14.2) | Not a network check, and not an approval |
| Feasibility (→ FEASIBLE) | Relevant network model check passed within declared model scope and tolerances (§14.2; §12.5) | **Network feasibility is not operator authorization** (§14.1) |
| Authorization (→ APPROVED) | Required human/policy authorization granted (§14.2) | Not derivable from a passing power-flow run |

And downstream: **an acknowledgement confirms receipt/acceptance at the application layer, not physical delivery** (§14.1, closing statement; §14.2 ACKNOWLEDGED row). ISSUED, ACKNOWLEDGED, and TRACKING measure progressively more evidence; none of them is COMPLETED until the defined delivery criterion is satisfied (§14.2).

The gates are also structurally protected: validation, feasibility, and authorization are requested from protected platform code, and the kernel owns final authorization and dispatch state transitions (§5.7; §6.4 Pipeline row: each stage records evidence, and failure does not silently skip the next gate).

## 5. Command invariants (§14.3 — every field)

Every command must include (§14.3):

| # | Invariant field |
|---|---|
| 1 | Unique command ID |
| 2 | Tenant, portfolio, site and asset IDs |
| 3 | Source subsystem and plugin/model version |
| 4 | Source schedule ID and schedule revision |
| 5 | Reason/action and actor or service identity |
| 6 | Target P/Q or supported control parameter |
| 7 | Units and sign convention |
| 8 | Valid-from and valid-until timestamps |
| 9 | Duration and ramp requirements |
| 10 | Monotonic sequence/version per asset or control channel |
| 11 | Idempotency key |
| 12 | Grid model and telemetry snapshot references |
| 13 | Validation evidence reference |
| 14 | Authorization decision reference, including approved scope, limits, and expiry |
| 15 | Signature or authenticated transport identity |
| 16 | Supersedes/superseded-by reference when a command replaces another |

Units and sign conventions follow the frozen contract in `01`-adjacent P0.07 output (§10.2): MW/MVAr/MVA/MWh, P > 0 = export at the point of connection, reactive-power sign declared per grid model and device profile, SOC a fraction in [0,1] at the API boundary. Valid-from/valid-until follow the P0.09 time model (§10.3).

## 6. Rejection rules (§14.3)

Commands must be rejected when:

| # | Rejection trigger |
|---|---|
| 1 | Stale |
| 2 | Duplicated with conflicting payloads |
| 3 | Out of sequence |
| 4 | Outside capability |
| 5 | Missing required evidence |

Fleet semantics: **partial fleet success is represented per asset; a portfolio-level acknowledgement is not inferred from the first responding device** (§14.3). Each asset's command follows its own lifecycle in §2; one device's acknowledgement never completes a portfolio.

## 7. Fallback rule (§14.4)

**There is no universal "set power to zero" fallback** (§14.4, first sentence).

Fallback policy is configured **per asset/site** and must be **reviewed with the equipment/site operator**. It can include (§14.4):

- hold the last accepted setpoint for a bounded interval where this is approved;
- return to local EMS autonomous control;
- preserve reserve/SOC constraints;
- stop accepting new external commands;
- request a safe local operating mode; or
- require operator intervention.

A last-known setpoint is not automatically safe when topology, asset state, or network constraints have changed (§14.4, closing sentence). Fallback activation appears in the state machine as FAILED → FALLBACK and EXPIRED → FALLBACK (§2, transitions 26–27); site-local safety and protection remain independent of FlexCore (§17.5, invariant 10).

## 8. Dispatch is not hard real-time (§14.5)

The system may implement scheduling and supervisory setpoint delivery, but **must not claim** primary-frequency, synthetic-inertia, relay, inverter inner-loop, or sub-cycle control capabilities. Those functions remain in local certified controllers and hardware unless a separate engineered and qualified architecture is developed (§14.5). This restates the §0.3 non-promises and is a protected boundary: no later phase, plugin, or document may re-attach these capabilities to dispatch (AGENTS.md §3 rule 3).

## 9. Safety invariants referenced by this freeze (§17.5)

The command state machine is one enforcement point for the §17.5 safety invariants, which are enforced by protected platform code and cannot be bypassed by plugins (§17.5). Directly relevant here: no command without a valid asset/site identity and current command context (1); no command without capability validation (2); no command without required network feasibility evidence for the declared operating profile (3); no command without authorization (4); no stale or expired command (5); no direct physical command from an optimizer, GNN, MARL, LLM, or general plugin (7). These are system requirements, not a certification claim (§17.5, closing sentence).

## 10. Acceptance-criteria coverage (ledger P0.11)

| Ledger acceptance criterion | Where satisfied |
|---|---|
| All 14 states appear (8 forward + 6 terminal/fallback states) | §2 state list; §3 definitions table (14 rows) |
| Every §14.1 transition in the table; none added | §2 (27 state-to-state + 6 initial/terminal rows); §2.1 mechanical diff statement and per-state outgoing-edge check |
| SUPERSEDED and CANCELLED distinguished | §3.1; §3 SUPERSEDED/CANCELLED rows |
| All §14.3 invariant fields listed, none dropped | §5 (16 rows) |
| Three-gate distinction and acknowledgement ≠ physical delivery stated | §4 |
| No-universal-fallback rule stated | §7 |

Verification method (ledger): transition-table diff against the §14.1 diagram — performed and recorded in §2.1; invariant checklist against §14.3 — performed in §5.
