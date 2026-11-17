<!-- P12 wave-1 Agent B close-out (records 21–26 + this record). No subtask
     was executed: no fault was injected into a running system, no game day
     ran, no drill executed, no experiment ran, no backup/restore was
     performed, no edge behavior was observed. No ledger checkbox was
     ticked by this agent — tick flips are the conventions-disciplined
     process, not a wave-1 act. -->
# Phase 12 — Wave 1 Agent B Close-Out (P12.07–P12.12: chaos / degradation / runbooks / review)

| Field | Value |
|---|---|
| Scope | P12.07–P12.12 (plan §3 wave 1, Agent B — chaos/degradation); records 21–26 + this close-out |
| Date | 2026-11-14 |
| Honesty summary | Everything delivered is WRITTEN-UNRUN. Every evidence cell is EMPTY/UNRUN; every threshold is a registered RECOMMENDATION (X-8); P12-E-3 remains OPEN (registered in record 25 §7); every capability statement traces written code under §13.3 semantics; ADR-010 holds — nothing adds dispatch authority |

## 1. Files created / modified

| File | Kind | Subtask |
|---|---|---|
| `docs/phases-11-20/phase-12-backup-dr-resilience/experiments/lab_harness.py` | new (stdlib-only harness + frozen 27-edge oracle transcription with the copy rule) | P12.07 |
| `docs/phases-11-20/phase-12-backup-dr-resilience/experiments/c_pg_1_postgres_kill.py` | new (written-unrun experiment script) | P12.07 |
| `docs/phases-11-20/phase-12-backup-dr-resilience/experiments/c_nats_1_nats_outage.py` | new | P12.07 |
| `docs/phases-11-20/phase-12-backup-dr-resilience/experiments/c_worker_1_worker_crash.py` | new | P12.07 |
| `docs/phases-11-20/phase-12-backup-dr-resilience/experiments/c_pool_1_pool_exhaustion.py` | new | P12.07 |
| `docs/phases-11-20/phase-12-backup-dr-resilience/runbooks/game_day_template.md` | new (template + quarterly cadence proposal) | P12.08 |
| `apps/worker/python/drill/fault_injection.py` | MODIFIED (additive extension: `partial_write`, `slow_consumer`, `PartitionInjector`, `scheduled_skew_clock`, `exceeds_supervision_timeout`, transcription constant; P8.24 self-tests and `stats()` keys unchanged byte-for-byte) | P12.09 |
| `apps/worker/python/drill/tests/test_fault_injection.py` | MODIFIED (additive mirror tests, incl. parametrized refusal cases) | P12.09 |
| `docs/phases-11-20/phase-12-backup-dr-resilience/runbooks/dr_site_loss.md` | new (DR runbook) | P12.11 |
| `docs/phases-11-20/phase-12-backup-dr-resilience/records/21_p12_07_chaos_program.md` … `26_p12_12_edge_resilience_review.md` | new decision records, one per subtask | P12.07–P12.12 |
| `docs/phases-11-20/phase-12-backup-dr-resilience/records/27_phase_12_wave1_agent_b_close_out.md` | this record | — |

NOT touched (the hard bans): `ledger_phase_12.md`, `plan.md`, `context.md`,
`docs/foundations/*`, `docs/plan/*`, `.agent/*`, any Cargo manifest, any
workspace member, Agent A's files (`records/01`–`06`, `19`,
`deploy/compose/backup/*`). No checkbox was ticked anywhere.

## 2. Permitted verification tools — VERBATIM outputs (2026-11-14)

```
$ python tools/verify_docs.py
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC
EXIT=0

$ python tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid
EXIT=0

$ python -I tools/check_dependency_rule.py
OK ù dependency rule holds for 20 workspace members.
EXIT=0
```

(The `ù` in the third output is the tool's own byte for the arrow glyph on
this console — quoted verbatim as required. All three exit 0.)

## 3. Deviations and why

1. **No execution of the extended Python code.** The loop bans running
   tests; the extended `fault_injection.py` and its pytest mirrors were
   hand-reviewed line by line instead. The P8.24 precedent (self-tests
   written, never run, execution = human gate) is followed exactly; the
   first human run is the gate that validates them.
2. **`network_partition` added to `FAULT_KINDS`.** The task named
   "network partition" as a fault to add; it is carried as a WINDOW kind
   by `PartitionInjector` (a partition is a duration, not a single call —
   the `ReorderInjector` precedent for kind-specific injectors). A plain
   plan entry of that kind would reach the fail-closed guard, which the
   module comment states honestly (the original P8.24 comment claimed a
   construction validation that does not exist; the reworded comment
   states the actual behavior — recorded here as a deliberate
   documentation correction, not a code change).
3. **Chaos scripts shell out only to operator-supplied lab-control
   commands.** The fault step (stopping a container) is unavoidable at
   the process level; the scripts refuse to invent the command
   (`FLEXCORE_LAB_*_STOP_CMD` / `START_CMD` env) and refuse to run
   without `--execute` plus the keyboard `GO` gate. Nothing in this loop
   executed any of it.
4. **Expected-path citations reconciled mid-task.** Agent A's records
   landed while this subtask ran; the P12.02/P12.06 sibling citations
   were reconciled to the landed names
   (`records/02_p12_02_backup_procedures.md`,
   `records/06_p12_06_pgbouncer_introduction.md`, artifacts under
   `deploy/compose/backup/`) in records 21/22/25, the DR runbook, and
   the C-POOL-1 script header.

## 4. Handoff notes for Agent A / wave 2

1. **Wave 2 / P12.13 (evidence templates)**: consume the EMPTY-cell
   shapes from `lab_harness.Evidence` (record 21 §3.5), the game-day
   template §6–§7, and the DR runbook §6 — one discipline, three
   instantiations.
2. **Wave 2 / P12.15 (acceptance criteria)**: records 24 §2 and 26 §4–§5
   are the traceability inputs ("resilient" = written-code-traced +
   audited + fail-closed + run-outcome-judged; never a guarantee).
3. **Wave 2 / P12.14 (JetStream drill)**: the C-NATS-1 experiment and
   A's `jetstream_snapshot.sh` meet at the P12-E-2 DEFAULT posture; the
   drill design (wave 2) should cite both rather than re-derive.
4. **Human gates pending** (all recorded UNRUN/UNSIGNED): the first
   build/test of the extended Python module; every chaos experiment;
   every game day (schedule adoption is the operator's); the DR
   rehearsal; operator sign-off on any SLO/threshold this work touches.
5. **P12-E-3 stays OPEN** (record 25 §7); the Phase 20 trigger consumes
   it unchanged.

## 5. Non-claims (stated once, governing everything above)

- No fault was injected into any running system; no experiment, game
  day, or drill ran; no backup or restore was performed; no timing was
  measured; no edge behavior was observed.
- Zero observed violations from any finite test would be a test outcome,
  never a guarantee (§13.3 semantics).
- Nothing here is a safety, certification, or protection-grade claim, and
  nothing adds dispatch authority (ADR-010; the never-granted permission
  set is untouched).
