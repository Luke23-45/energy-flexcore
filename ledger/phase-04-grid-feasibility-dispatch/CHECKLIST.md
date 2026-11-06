# Exit-Gate Checklist — Phase 4: Grid Feasibility and Dispatch Assurance

- **Exit gate (§23 Stage 4):** "Invalid, stale, infeasible, unauthorized, duplicate, and expired candidate commands are rejected in deterministic tests."
- **How to use:** check an item only when the evidence named in its row exists and
  is linked or filed. Evidence is a file path, a test-run output, or a review note —
  never a bare assertion.

## Gate items

- [ ] **G4.1 Primary provider selected and pinned** — Evidence: decision note (pandapower 3.5.6) + lockfile; D3 still OPEN (P4.01).
- [ ] **G4.2 Grid-model port + adapter** — Evidence: port definition; adapter scope declaration; contract tests incl. stub second adapter (P4.02).
- [ ] **G4.3 Versioned network model + health** — Evidence: immutability/hash tests; stale-model blocks a pass (P4.03).
- [ ] **G4.4 Pipeline steps 1–2** — Evidence: schema/units/timestamp and capability/ramp negative-case tests (P4.04).
- [ ] **G4.5 Pipeline steps 3–4 (AC power flow)** — Evidence: run records; non-convergence reported with diagnostics; no silent retry; AC power flow never labelled AC-OPF (P4.05).
- [ ] **G4.6 Pipeline step 5 (limits + margins)** — Evidence: single-violation fixture results (low/high voltage, branch, transformer); margins recorded (P4.06).
- [ ] **G4.7 Pipeline steps 6–8 (outcomes)** — Evidence: reject/constrain/return routing tests; coverage limitations stated; `FeasibilityCheckCompleted` emitted (P4.07).
- [ ] **G4.8 Fail-closed policy** — Evidence: policy table; per-trigger tests; zero triggers yield a pass (P4.08).
- [ ] **G4.9 SOCP relaxation boundary** — Evidence: Tier 2 provenance + condition checklist; inexactness fixture rejected by power flow (P4.09).
- [ ] **G4.10 State machine exact** — Evidence: transition-matrix tests (legal + illegal); no state outside §14.2; terminal states have no outgoing transitions (P4.10).
- [ ] **G4.11 Validation gate** — Evidence: gate tests; superseded-schedule command rejected at creation (P4.11).
- [ ] **G4.12 Feasibility gate for commands** — Evidence: separate feasibility evidence ref; APPROVED unreachable without FEASIBLE (P4.12).
- [ ] **G4.13 Human authorization gate** — Evidence: actor + scope + limits + expiry recorded; denial rejects; self-approval blocked where configured (P4.13).
- [ ] **G4.14 §14.3 invariants enforced** — Evidence: field-completeness tests; per-rule rejection tests; per-asset acknowledgement test (P4.14).
- [ ] **G4.15 Expiry / cancel / supersede** — Evidence: controlled-clock expiry tests; SUPERSEDED vs CANCELLED distinguishable in audit (P4.15).
- [ ] **G4.16 Simulator-only dispatch adapter** — Evidence: end-to-end lifecycle to COMPLETED; NACK/timeout → FAILED; partial failure per-asset; non-simulator-target guard test passes; no device write path (P4.16).
- [ ] **G4.17 Fallback policies per asset/site** — Evidence: policy configuration; three variants tested; no universal fallback in code (P4.17).
- [ ] **G4.18 Bounded hold + recovery** — Evidence: hold-expiry escalation test; recovery requires full gate chain; FALLBACK → ISSUED rejected (P4.18).
- [ ] **G4.19 Timing honesty** — Evidence: latency measurements with distribution; no hard-real-time/frequency/relay claim in Phase 4 artefacts (P4.19).
- [ ] **G4.20 Invalid commands rejected (deterministic)** — Evidence: suite output, pre-registered expectations, two identical runs, CI (P4.20).
- [ ] **G4.21 Stale commands rejected (deterministic)** — Evidence: suite output incl. clock-skew case; controlled clock (P4.21).
- [ ] **G4.22 Infeasible commands rejected (deterministic)** — Evidence: suite output; failing constraint named matches designed violation (P4.22).
- [ ] **G4.23 Unauthorized commands rejected (deterministic)** — Evidence: suite output; denials audit-recorded; none reaches ISSUED (P4.23).
- [ ] **G4.24 Duplicate commands handled (deterministic)** — Evidence: suite output; exact replay deduplicated; conflicting duplicate rejected; redelivery causes no duplicate side effect (P4.24).
- [ ] **G4.25 Expired commands rejected (deterministic)** — Evidence: suite output per lifecycle stage; events emitted (P4.25).
- [ ] **G4.26 Combined/adversarial cases rejected** — Evidence: suite output; forged/replayed cases never issue; none reaches ISSUED (P4.26).
- [ ] **G4.27 Exit-gate evidence pack** — Evidence: pack with two clean full runs + per-suite reviewer sign-offs (P4.27).
- [ ] **G4.28 Security controls** — Evidence: tampered/missing-signature rejection; replay rejection; cross-tenant denial; secret scan (P4.28).
- [ ] **G4.29 §17.5 invariants 1–9 tested** — Evidence: invariant-to-test matrix; plugin-bypass test passes; invariant 10 boundary documented (P4.29).
- [ ] **G4.30 Audit/events/persistence/observability** — Evidence: transition persistence; idempotent event consumption; API contract tests; correlation IDs on sampled trace; retention configurable (P4.30).
- [ ] **G4.31 Fault injection + shadow** — Evidence: per-fault expected outcomes; integration tests; shadow report with limits stated (P4.31).
- [ ] **G4.32 Handover to Phase 5 filed** — Evidence: handover note; `verify_docs.py` + `validate_agent.py` output recorded (P4.32).
- [ ] **G4.33 Exhaustive transition matrix** — Evidence: 14×14 matrix report, every cell tested against the P4.10 table; mutation check passes (P4.33).
- [ ] **G4.34 Unbalanced-model deferral/trigger** — Evidence: deferral note with named trigger cases; model class on every feasibility result; D3 still OPEN (P4.34).
- [ ] **G4.35 Feasibility runtime measured** — Evidence: per-check durations with conditions on §24 slice + largest library case; baseline in handover; CI regression check (P4.35).
- [ ] **G4.36 Audit-events read API** — Evidence: exactly one `/api/v1/audit-events` resource recorded (or verified existing); tenant-scoped; no update/delete endpoint (P4.36).
- [ ] **G4.37 Constrain/return consumption contract** — Evidence: contract type + emission; stub-consumer test passes; unconsumable case rejects with reason; Phase 5 closed-loop item recorded (P4.37).

## Sign-off

| Role | Name | Date | Evidence reviewed |
|---|---|---|---|
| Architect | | | G4.1, G4.8, G4.27, G4.32, G4.36 |
| Power-systems engineer | | | G4.3–G4.9, G4.22, G4.34, G4.37 |
| QA | | | G4.20–G4.27, G4.31, G4.33, G4.35 |
| Security engineer | | | G4.13, G4.23, G4.28–G4.29 |
