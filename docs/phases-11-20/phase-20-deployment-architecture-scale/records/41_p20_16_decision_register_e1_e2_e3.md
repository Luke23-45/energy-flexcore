<!-- P20.16(a) — Decision records. HONEST RECORD: a DECISION-REGISTER
     record. It VERIFIES that P20-E-1 and P20-E-3 are already registered
     OPEN with their DEFAULTs in their owning wave-1 records (records/04 §3
     and records/27 §5 respectively — verified on disk 2026-11-17), and
     REGISTERS P20-E-2 OPEN with DEFAULT (a) here, because no owning record
     exists for it anywhere in the phase (plan §6 and the ledger decision
     table both list it; records/04 row T-2 and records/21 reference it —
     neither is a registration; the grep evidence is recorded in §1). NO
     E-decision is closed by this record or by any agent; all three closers
     are the human owner (conventions §4). No deployment, cluster, region,
     or metrics-store decision is made anywhere in this record. -->
# P20.16(a) — Decision records: P20-E-1 and P20-E-3 verified registered OPEN (records 04, 27); P20-E-2 registered OPEN with DEFAULT (a) here — all three remain human-owned

| Field | Value |
|---|---|
| Subtask | P20.16 — Decision records + close, part (a): the three phase-20 E-decisions (`ledger_phase_20.md` §P20.16; plan §6) |
| Wave / agent | Phase 20 wave 2, Agent A |
| Status | **DECISION-REGISTER RECORD — ALL THREE E-DECISIONS OPEN, NONE CLOSED BY ANY AGENT.** P20-E-1 and P20-E-3 are verified registered in their owning records at their recorded paths; P20-E-2 is registered HERE because no owning record exists. The consistency audit (close record `records/42_phase_20_close_and_consistency_audit.md`) covers part (c). |
| Sources | plan §6 (the full E-decision text — consumed verbatim in substance, never re-worded); record `records/04_p20_04_kubernetes_trigger_register.md` §3 (P20-E-1's owning registration — verified on disk 2026-11-17); record `records/27_p20_15_observability_at_scale.md` §5 (P20-E-3's owning registration — verified on disk 2026-11-17); record `records/21_p20_09_multi_region_dr.md` (the P20-E-2 references that motivated the ownership check — header field, §2, §7 item 5, §9 A-P20.09-3); record `records/28_phase_20_wave1_agent_b_close_out.md` §7 item 2 (the wave-1 handoff note); conventions §4 (agents never close an E-decision) |
| Files touched | This record only (no ledger edit — the decision-points table in `ledger_phase_20.md` already lists all three as OPEN and is the orchestrator's file) |

## 1. Why this record exists

Conventions §4: an E-decision's registration records the question, the
options, the DEFAULT/RECOMMENDATION label, and what evidence would close
it — and agents never close it. Wave 1 registered P20-E-1 (records/04 §3)
and P20-E-3 (records/27 §5) in their owning records. P20-E-2, however,
had NO owning registration: plan §6 and the ledger decision table list
it, record 04's trigger row T-2 references it once ("per P20-E-2"), and
record 21 references it four times (the header Trigger field, §2's
topology-choice sentence, §7 item 5, and finding A-P20.09-3, which
correctly states it "stays OPEN with DEFAULT (a)") — but no record
carries the formal registration table. The ownership grep (2026-11-17):

```text
$ grep -rn "P20-E-2" docs/phases-11-20/phase-20-deployment-architecture-scale/
ledger_phase_20.md:102,114   (the unticked P20.16 row + the decision table)
plan.md:79,153               (the subtask row + the §6 registration text)
records/04_...md:36          (trigger row T-2 — a reference)
records/21_...md:18,38,131,158 (four references; A-P20.09-3 carries the
                              DEFAULT (a) substance without the table)
records/28_...md:113         (wave-1 close-out handoff note)
```

This record performs the P20.16(a) deliverable:

1. **verification** of the two existing registrations (§2, §4);
2. **registration** of P20-E-2 OPEN with DEFAULT (a) (§3);
3. the **consolidated register state at close** (§5) for the close
   record and HANDOVER to consume.

## 2. P20-E-1 — verified registered OPEN, DEFAULT (a) — record 04 §3 (owning record)

| Item | Verified content (record `records/04_p20_04_kubernetes_trigger_register.md` §3, on disk 2026-11-17) |
|---|---|
| Question | When does the Kubernetes migration actually happen, if ever? |
| Options | (a) stay on Compose until a registered trigger fires, then migrate; (b) migrate preemptively (REJECTED — the Tier 3 verdict is explicit [S-72]); (c) never migrate regardless of triggers (REJECTED — dishonest under (a) conditions) |
| DEFAULT | **(a)** — the trigger register (record 04 §2, T-1..T-4, ALL UNFIRED with EMPTY evidence pointers) is the decision machinery; each trigger names its observable condition so the future migration is a decision, not a drift [S-72][S-73] |
| What would close it | The owner recording that a named trigger has fired (with the evidence), or recording that none will |
| Status | **OPEN** — closer: human owner |

Verification method: direct read of record 04 §3 (its table) and §2
(all four triggers UNFIRED); the ledger decision table reads OPEN
(read-only check). This record adds nothing to the registration — the
owning record stands.

## 3. P20-E-2 — REGISTERED HERE: OPEN, DEFAULT (a) — no owning record exists

| Item | Recorded content |
|---|---|
| ID | **P20-E-2** (phase-20 decision register; plan §6; ledger §Decision points) |
| Question | Is a second region required, and if so active/passive or something else? |
| Options | (a) no second region — DR stays the P12 posture (documented site-loss runbook + restore-into-isolation rehearsal) until a real requirement exists; (b) warm active/passive DR region (the P20.09 topology); (c) active/active multi-region (rejected at this scale — consistency cost without a stated requirement) |
| **DEFAULT** | **(a) — no second region until a real requirement is recorded.** Rationale (carried from plan §6, in substance): this is the P12-E-3 revisit condition; record 21's active/passive design (`records/21_p20_09_multi_region_dr.md`) is decision-ready against the day the requirement lands, with the P13.14 residency constraints pre-consumed; no second region is stood up and no cross-region replication exists today |
| What would close it | The owner recording the second-region requirement (customer, regulator, or contract evidence) — a RECORDED OWNER EVENT, never an agent inference (record 04 §2's T-2 row is the trigger-register mirror of the same condition, linked not restated) |
| Status | **OPEN — never closed by this record or by any agent** (conventions §4). Closer: human owner. |
| Dependence note | Whether DEFAULT (a) or (b) is eventually chosen, record 21's design deploys nothing by itself; the second region is an implementing wave gated on the owner-recorded requirement, and the P12-E-3 question wording in the Phase 12 plan remains the authoritative register for the same condition (record 21 F-P20.09-2 — two registers LINK one condition, by design) |

## 4. P20-E-3 — verified registered OPEN, DEFAULT (a) — record 27 §5 (owning record)

| Item | Verified content (record `records/27_p20_15_observability_at_scale.md` §5, on disk 2026-11-17) |
|---|---|
| Question | Long-term metrics topology at scale: does the P11.10 single-node DEFAULT change? |
| Options | (a) keep the P11.10 DEFAULT (single-node VictoriaMetrics-class) until cardinality/volume evidence says otherwise; (b) clustered/ha store topology (paired with the P20.04 multi-host trigger); (c) managed vendor service (a budget/vendor E-decision that belongs to the owner) |
| DEFAULT | **(a)** — extends P11-E-2 with the scale conditions recorded in record 27 §3 (SC-1 cardinality-budget consumption, SC-2 retention-vs-storage economics, SC-3 an explicit availability contract — every condition cell EMPTY, no evidence exists) |
| What would close it | The owner recording volume/cost evidence exceeding the single-node envelope, or naming a vendor |
| Status | **OPEN** — closer: human owner |

Verification method: direct read of record 27 §5 and §3 (read-only);
the ledger decision table reads OPEN. This record adds nothing to the
registration — the owning record stands.

## 5. Consolidated register state at close (2026-11-17)

| ID | Where registered | Status | DEFAULT | Closer |
|---|---|---|---|---|
| P20-E-1 | record `records/04_p20_04_kubernetes_trigger_register.md` §3 | **OPEN** | (a) stay on Compose until a registered trigger fires (T-1..T-4, ALL UNFIRED) | human owner |
| P20-E-2 | THIS record §3 (owning registration; no prior owning record) | **OPEN** | (a) no second region — DR stays the P12 posture until a real requirement is recorded; the P20.09 design is decision-ready for that day (P12-E-3 linkage) | human owner |
| P20-E-3 | record `records/27_p20_15_observability_at_scale.md` §5 | **OPEN** | (a) keep the P11.10 single-node DEFAULT until cardinality/volume evidence exceeds the envelope (SC-1..SC-3, all EMPTY) | human owner |

No agent closed any of the three; the ledger's decision table reads OPEN
for all three (verified read-only). If the orchestrator wishes the
ledger decision table to name an owning record per row, P20-E-2's row
may cite this record — that edit is the orchestrator's, never this
agent's.

## 6. What this record does NOT establish

1. No decision is made: registering an OPEN E-decision with a DEFAULT
   records a recommendation, never a choice — no migration happens, no
   second region is stood up, no metrics store is selected, and no
   vendor, budget, or date is named anywhere in this record.
2. No trigger status changes: T-1..T-4 (record 04 §2), the TimescaleDB
   volume trigger (record 22), the availability-contract trigger
   (record 07 §4), and the SC-1..SC-3 conditions (record 27 §3) all
   remain UNFIRED with EMPTY evidence cells as of 2026-11-17.
3. No deployment, release-ladder, or dispatch-authority change: the
   never-granted permission set, the command lifecycle gates, tenant
   isolation, and audit are untouched by any decision posture here.
4. It does not re-word, re-open, or close P12-E-3 (or any Phase 12
   register): P20-E-2 is the phase-level mirror of the same question,
   recorded per conventions §4 — the linkage discipline of record 21
   (linked, not restated) carries.

## 7. Reviewer-verification procedure (human)

1. Confirm the §1 grep description: P20-E-2 had no owning registration
   before this record (check `grep -rn "P20-E-2" records/` yourself).
2. Confirm records 04 §3 and 27 §5 carry the P20-E-1/P20-E-3
   registrations as verified above, and that the ledger's decision
   table reads OPEN for all three.
3. Confirm §5's DEFAULTs match plan §6 verbatim in substance.
4. Confirm no closer other than "human owner" appears anywhere.

## 8. Status

**REGISTER RECORD — P20-E-1/E-3 verified OPEN in their owning records;
P20-E-2 registered OPEN with DEFAULT (a) here; zero decisions closed by
any agent.** Ticking the P20.16 decision-records half means "the
register is complete and consistent" — never "decided".
