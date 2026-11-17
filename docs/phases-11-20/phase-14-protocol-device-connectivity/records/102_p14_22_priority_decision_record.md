# P14.22 — Cross-protocol priority decision record (P14-E-1 registered OPEN)

| Field | Value |
|---|---|
| Subtask | P14.22 — P14-E-1 recorded: DEFAULT recommendation IEEE 2030.5 + OpenADR 3.0 first (DR program surfaces), DNP3 next (utility SCADA), 104/61850 on EU demand; the record names what evidence would close it |
| Wave / agent | Phase 14 wave 3, Agent B (P14.19, P14.22–P14.24) |
| Status | **DECISION REGISTER RECORD. P14-E-1 remains OPEN.** Agents never close E-decisions (conventions §4); this record registers the question, the options, the DEFAULT (RECOMMENDATION), the closer, and the closing evidence — nothing more. No priority was applied to any protocol beyond what the plan already assigned: every Phase 14 adapter landed at the depth its plan row stated |
| Sources | plan §4 (P14-E-1 as registered at planning time); conventions §4 (the decision-register discipline); the phase's own delivered records (01–04, 21–24, 41–44, 61–64; Agent A wave 3 records 81–99 — wait-checked at close); [S-5][S-10] (the 2030.5/OpenADR certification-infrastructure context), [S-14] (DNP3), [S-12][S-13] (61850), the gap-analysis table-stakes rows #1 and #11 |
| Files touched | This record only |

## 1. The decision (verbatim from the plan register, verified on disk)

`ledger_phase_14.md` §Decision register carries P14-E-1 as **OPEN**;
`plan.md` §4 carries the same entry. This record is the decision-REGISTER
RECORD the plan's P14.22 row calls for (plan §2 row P14.22: "P14-E-1
recorded ... the record names what evidence would close it"), consolidating
the register entry with the phase's own delivery evidence. The register
table itself was NOT edited (the ledger is the orchestrator's file — hard
ban; this record does not edit it).

| Field | Value |
|---|---|
| ID | P14-E-1 |
| Question | Which protocol adapters get implementation effort first, given that all land as design + models + written-unrun tests in Phase 14? |
| Options | (a) IEEE 2030.5 + OpenADR 3.0 first (DR program surfaces), DNP3 next (utility SCADA), 104/61850 on EU demand; (b) DNP3 first (utility SCADA before DR programs); (c) equal-depth implementation of all |
| **DEFAULT (RECOMMENDATION)** | **(a)** — DR programs are the nearest commercial surface for the table-stakes rows (gap analysis §3 #1 and #11), and the 2030.5/OpenADR certification infrastructure [S-5][S-10] is the most documented path (the evidence basis is record 101 §2) |
| What closes it | The HUMAN OWNER naming the first DR program and the first utility SCADA integration (if any). Program-level priorities are human E-decisions (conventions §4) |
| Status | **OPEN** — registered in `plan.md` §4 and `ledger_phase_14.md`; this record does not close it |

## 2. How the phase treated the DEFAULT (recorded, never applied as a close)

The DEFAULT is a RECOMMENDATION for FUTURE implementation-effort ordering;
it is not a filter on the phase's own scope. Phase 14's delivery is
evidence of exactly that: every protocol named in the scope landed at the
depth its plan row stated, in the wave the plan assigned — the
priority order changed NOTHING about what was built in Phase 14:

| Surface | Phase 14 delivery | Depth class |
|---|---|---|
| IEEE 2030.5 (AggregatorServer + security + test suite) | crates + records 01–04 | design-and-model + written-unrun tests |
| OpenADR 3.0 | crate + record 21; 2.0b assessment record 22 (P14-E-2 OPEN) | design-and-model; record-only for 2.0b |
| SunSpec Modbus | crate + record 23 | design-and-model |
| DNP3 (outstation first) | crate + record 24 (master role deferred in-record) | design-and-model; assessment for the master role |
| IEC 104 / IEC 61850 | design records 41/42; NO crate (P14-E-3 OPEN, DEFAULT design-only) | record-only |
| MQTT 5 + Sparkplug B | crate + record 43 | design-and-model |
| CIM/CGMES | crate + record 44 | design-and-model |
| Simulator / seam / budgets / observability | Agent A wave 3 (records 81–99) | design-and-model |

Consequence for the future: when P14-E-1 closes, the closing evidence
names the first funded implementation surfaces; the implementation batch
for a surface follows the crate pattern of records 01/21/23/24/43/44 (or,
for 104/61850, implements records 41/42 after P14-E-3 closes). The
priority DEFAULT and P14-E-3's design-only DEFAULT compose cleanly: the
EU surfaces are LAST in the DEFAULT order and design-only until the EU
counterparty is named — consistent, not redundant.

## 3. The evidence that would close it (named, per the subtask)

The decision closes when the human owner provides ALL of:

1. **The first named DR program** (counterparty, program/market, the
   protocol it mandates — 2030.5, OpenADR 2.0b [S-11], or a proprietary
   surface), which confirms or overturns the 2030.5/OpenADR-first half of
   the DEFAULT. If the program mandates 2.0b, P14-E-2 closes in the same
   act.
2. **The first named utility SCADA integration, if any** (counterparty
   utility, its DNP3/104 profile requirements, its point list), which
   sets the DNP3 timeline and could reorder option (b) ahead of (a).
3. **A budget/effort statement** covering which surfaces get engineering
   time first — the program-level "certification order" is explicitly an
   owner-owned E-decision class (conventions §4).

Evidence that closes P14-E-3 (the EU timing) may arrive in the same
package (a named EU DSO integration) but is a SEPARATE decision with its
own register entry (record 42 §6) — closing one never closes the other.

## 4. Honest boundaries

No implementation effort was reordered or applied by this record; no
protocol work was started, stopped, connected, or contacted because of
it; no counterparty exists; no DR program, utility SCADA integration, or
EU DSO integration is named anywhere in this program. All three
E-decisions (P14-E-1 here, P14-E-2 in record 22 §3, P14-E-3 in record 42
§6) are OPEN at close — verified in the close record (104) §1.
