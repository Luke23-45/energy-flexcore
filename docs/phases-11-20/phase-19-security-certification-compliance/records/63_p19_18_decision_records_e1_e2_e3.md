<!-- P19.18(a) — Decision records. HONEST RECORD: a DECISION-REGISTER
     record. It VERIFIES that P19-E-1 and P19-E-3 are already registered
     OPEN with their DEFAULTs in their owning wave-1 records (01 §4 and
     24 §4 respectively — verified on disk 2026-11-17), and REGISTERS
     P19-E-2 OPEN with DEFAULT (a) here, because no owning record exists
     for it anywhere in the phase (plan §6 lists it; the only prior
     mentions are a reference in record 21 §6 and a handoff note in
     record 19 — neither is a registration). NO E-decision is closed by
     this record or by any agent; all three closers are the human owner.
     No certification, attestation, or audit outcome is claimed. -->
# P19.18(a) — Decision records: P19-E-1 and P19-E-3 verified registered OPEN (records 01, 24); P19-E-2 registered OPEN with DEFAULT (a) here — all three remain human-owned

| Field | Value |
|---|---|
| Subtask | P19.18 — Decision records + close, part (a): the three phase-19 E-decisions (`ledger_phase_19.md` §P19.18; plan §6) |
| Wave / agent | Phase 19 wave 2, Agent B |
| Status | **DECISION-REGISTER RECORD — ALL THREE E-DECISIONS OPEN, NONE CLOSED BY ANY AGENT.** P19-E-1 and P19-E-3 are verified registered in their owning records at their recorded paths; P19-E-2 is registered HERE because no owning record exists. The consistency audit (close record 65) covers part (c). |
| Sources | plan §6 (the full E-decision text — consumed verbatim in substance, never re-worded); record `01_p19_01_certification_strategy.md` §4 (P19-E-1's owning registration — verified on disk 2026-11-17); record `24_p19_13_penetration_test_program.md` §4 (P19-E-3's owning registration — verified on disk 2026-11-17); record `21_p19_10_soc2_evidence_chain.md` §6 (the P19-E-2 reference that motivated this check); record `19_phase_19_wave1_agent_a_close_out.md` §4 (the handoff note deferring P19-E-2 to B's range); conventions §4 (agents never close an E-decision; program-level decisions with human closers are always E-decisions) |
| Files touched | This record only (no ledger edit — the decision-points table in `ledger_phase_19.md` already lists all three as OPEN and is the orchestrator's file) |

## 1. Why this record exists

Conventions §4: an E-decision's registration records the question, the
options, the DEFAULT/RECOMMENDATION label, and what evidence would close
it — and agents never close it. Wave 1 registered P19-E-1 and P19-E-3 in
their owning records. P19-E-2, however, had NO owning registration: plan
§6 lists it, record 21 §6 references it once, and Agent A's close-out
(record 19 §4) correctly deferred it to B's range — but no record carried
the formal registration. This record performs the P19.18(a) deliverable:

1. **verification** of the two existing registrations (§2, §4);
2. **registration** of P19-E-2 OPEN with DEFAULT (a) (§3);
3. the **consolidated register state at close** (§5) for the close
   record and HANDOVER to consume.

## 2. P19-E-1 — verified registered OPEN, DEFAULT (a) — record 01 §4 (owning record)

| Item | Verified content (record `01_p19_01_certification_strategy.md` §4, on disk 2026-11-17) |
|---|---|
| Question | Certification order: SOC 2 Type II first, ISO 27001 first, or parallel tracks? |
| Options | (a) SOC 2 Type II first, ISO 27001 follow-on; (b) ISO 27001 first (EU-first market posture); (c) parallel tracks |
| DEFAULT | **(a)** — SOC 2 Type II first for US-market traction ([S-44][S-45]); ISO 27001 follow-on; ISASecure SDLA the edge agent's longer-horizon target [S-22]; timelines human decisions |
| What would close it | The owner recording the market priority and engaging the first auditor |
| Status | **OPEN** — implemented as the working posture only; closer: human owner |

Verification method: direct read of record 01 §4 (its table and §9
reviewer items 2–3); the ledger's decision table reads OPEN (read-only
check). This record adds nothing to the registration — the owning record
stands.

## 3. P19-E-2 — REGISTERED HERE: OPEN, DEFAULT (a) — no owning record exists

Check performed first (the task brief's instruction): a grep over the
whole phase directory confirms `P19-E-2` appears only in plan §6, the
ledger decision table, record 19 §4 (the deferral note), and record 21
§6 (a single reference). **No record registers it.** This record is its
owning registration:

| Item | Recorded content |
|---|---|
| ID | **P19-E-2** (phase-19 decision register; plan §6; ledger §Decision points) |
| Question | Adopt a compliance-automation platform (Vanta/Drata class [S-80]) at certification time, or manual evidence collection? |
| Options | (a) adopt a platform at certification kickoff; (b) manual collection with the P19.10 map as the index; (c) re-decide at kickoff |
| **DEFAULT** | **(a) — adopt at certification kickoff.** Rationale (carried from plan §6, in substance): evidence over an observation window is exactly what these platforms automate [S-80]; the P19.10 mapping (`records/21_p19_10_soc2_evidence_chain.md`) remains the platform-independent index either way; budget is the owner's call |
| What would close it | The owner recording the budget decision at certification kickoff (plan §6 closer column) |
| Status | **OPEN — never closed by this record or by any agent** (conventions §4). Closer: human owner. |
| Dependence note | Whether DEFAULT (a) or (b) is chosen, the P19.10 map stays the index and no platform is adopted, budgeted, or engaged by this phase — the adoption is a certification-time act that presupposes the human gates of P19-E-1 (auditor engaged) and the first green build |

## 4. P19-E-3 — verified registered OPEN, DEFAULT (a) — record 24 §4 (owning record)

| Item | Verified content (record `24_p19_13_penetration_test_program.md` §4, on disk 2026-11-17) |
|---|---|
| Question | What criteria govern penetration-test vendor selection? |
| Options | (a) criteria-based with recorded minimums; (b) incumbent/cheapest qualifying vendor; (c) broker-managed |
| DEFAULT | **(a)** — criteria-based, with the recorded minimums: methodology disclosure; OT/grid competence; retest terms; reporting quality |
| What would close it | The owner accepting the criteria and selecting the vendor at engagement time |
| Status | **OPEN** — the vendor, the engagement date, and the findings remain human gates; every engagement cell NOT-STARTED (record 24 §7) |

Verification method: direct read of record 24 §4 and §7 (read-only);
the ledger's decision table reads OPEN.

## 5. Consolidated register state at close (2026-11-17)

| ID | Where registered | Status | DEFAULT | Closer |
|---|---|---|---|---|
| P19-E-1 | record `01_p19_01_certification_strategy.md` §4 | **OPEN** | (a) SOC 2 Type II first, ISO 27001 follow-on; ISASecure SDLA longer-horizon target | human owner |
| P19-E-2 | THIS record §3 (owning registration; no prior owning record) | **OPEN** | (a) adopt the platform at certification kickoff; the P19.10 map stays the platform-independent index | human owner |
| P19-E-3 | record `24_p19_13_penetration_test_program.md` §4 | **OPEN** | (a) criteria-based with recorded minimums | human owner |

No agent closed any of the three; the ledger's decision table reads OPEN
for all three (verified read-only). If the orchestrator wishes the
ledger decision table to name an owning record per row, P19-E-2's row
may cite this record — that edit is the orchestrator's, never this
agent's.

## 6. Not-a-certification statement (verbatim discipline carried)

Quoted verbatim from `AGENTS.md` §3 (rules 2–3), as the P10.25 §7
discipline requires of every phase-19 record:

> 2. **No safety or certification claims.** FlexCore is not certified,
> not a safety case, and not permission for live grid control. Never
> write text that implies otherwise.
> 3. **No protection-grade claims.** FlexCore is a supervisory
> orchestration layer. It does not perform primary frequency response,
> synthetic inertia, relay protection, inverter inner-loop control, or
> sub-cycle control.

Registering an OPEN decision with a DEFAULT records a preference, not a
choice: no certification order is decided, no platform is adopted, and
no vendor is selected anywhere in this record.

## 7. Reviewer-verification procedure (human)

1. Confirm the §1 grep description: P19-E-2 had no owning registration
   before this record (check `grep -rn "P19-E-2" records/` yourself).
2. Confirm records 01 §4 and 24 §4 carry the P19-E-1/P19-E-3
   registrations as verified above, and that the ledger's decision
   table reads OPEN for all three.
3. Confirm §5's DEFAULTs match plan §6 verbatim in substance.
4. Confirm no closer other than "human owner" appears anywhere.

## 8. Status

**REGISTER RECORD — P19-E-1/E-3 verified OPEN in their owning records;
P19-E-2 registered OPEN with DEFAULT (a) here; zero decisions closed by
any agent.** Ticking the P19.18 decision-records half means "the
register is complete and consistent" — never "decided".
