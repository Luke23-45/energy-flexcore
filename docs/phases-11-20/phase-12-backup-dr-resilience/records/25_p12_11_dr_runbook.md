# P12.11 — DR runbook (site-loss scenario, restore-into-isolation, DNS/reconfiguration, communications; P12-E-3 registered OPEN)

| Field | Value |
|---|---|
| Subtask | P12.11 (DR runbook: site-loss scenario, restore-into-isolation procedure, DNS/reconfiguration steps, communication checklist) |
| Wave / agent | Phase 12 wave 1, Agent B |
| Status | **WRITTEN-UNRUN.** The runbook is written; no site loss has occurred or been simulated, no restore performed, no DNS change made, no communication sent — every execution cell in the runbook is EMPTY/UNRUN. The restore mechanics are P9.24 §5–§6 by citation, never rewritten; the backup/restore model of record is `deployment-upgrade-model.md` §6, same discipline. |
| Sources | P9.24 §5 (R-0..R-10), §6 (E-1..E-6), §8 (the EMPTY-cell rule) — `ledger/phase-09-pilot-qualification/24_backup_dr_rehearsal.md`; `docs/operations/deployment-upgrade-model.md` §5 (degraded modes + central-outage rule) and §6 (backup/restore model); P10.11 §4.1–4.4 (RC-specific checks); [S-82] (game-day framing for the rehearsal's human-gate discipline) |
| Files touched | `docs/phases-11-20/phase-12-backup-dr-resilience/runbooks/dr_site_loss.md` (new) |
| Sibling citations (reconciled) | Agent A's P12.02 backup procedure (`records/02_p12_02_backup_procedures.md`, artifacts under `deploy/compose/backup/`: `backup_postgres.sh`, `pgbackrest.conf`, `jetstream_snapshot.sh`, `snapshot_manifest.py`, `verify_backup.sh`) and P12.01 selection record (`records/01_p12_01_backup_architecture.md`) — first cited by EXPECTED PATH per plan §2.1, reconciled to A's landed names at close |

## 1. What was delivered

1. **Site-loss scenario** (runbook §1): the Zone 2 central-application
   site as the lost element; the site-local EMS explicitly NOT lost (the
   central-outage rule, cited verbatim in intent); edges hold their
   E-1..E-6 bounded store-and-forward posture with the audited
   drop-of-oldest loss window (R-P9.24-5 consumed, not re-derived); no
   recovery objective is set — the DR scenario judges against P9.24 §4's
   PLACEHOLDER requirement cells and the P12.04 RECOMMENDATION register,
   never against invented numbers.
2. **Trigger and decision gates** (§2): five human gates (declaration,
   activation, artifact scoping, isolation, go/no-go for reconfiguration),
   each with a named owner role and an EMPTY record cell.
3. **Restore-into-isolation procedure** (§3): DR-1..DR-12, each row
   NAMING the P9.24 §5/§6 R-step or P10.11 §4 check it executes — the
   criteria live in the cited documents; the JetStream leg follows the
   P12-E-2 DEFAULT posture with the dependence recorded.
4. **DNS/reconfiguration steps** (§4): N-1..N-5 with the guard that no
   live DNS change happens before the T-4 go/no-go; edge reconnection is
   by outbound session only (the F1 rule — no inbound listener exists);
   the rollback is the reverse named act under the P10.12 register rules.
5. **Communication checklist** (§5): four audience/trigger rows with
   channels as deployment decisions and every "Sent" cell
   `<EMPTY — NOT SENT>`; the customer-facing row carries the honesty
   constraint that no RTO/RPO promise may be made while the register's
   sign-off cell is EMPTY.
6. **Rehearsal record template** (§6): all cells EMPTY; a filled cell
   without a run is a fabrication defect (the P9.24 §8 rule, inherited).
7. **P12-E-3 registered OPEN** (§7): the second-DR-site question with its
   DEFAULT ("no second site in the current posture — DR is the documented
   site-loss runbook plus the restore-into-isolation rehearsal; revisit at
   the Phase 20 multi-region trigger"), closer = the human owner; an agent
   does not close it.

## 2. Honest boundaries

- No capability or recovery-time claim of any kind: the runbook's only
  numbers are `<EMPTY>` cells and the cited registers' RECOMMENDATION
  labels.
- The runbook does not re-decide append-only properties, restore criteria,
  or rollback semantics — it ORCHESTRATES the cited procedures and adds
  the DR-specific structure (triggers, DNS, communications).
- Nothing here adds dispatch authority (ADR-010): a DR-restored control
  plane resumes under the same disabled-by-default command path and
  per-site enablement gates as any deployment.
- §13.3 semantics: a successful rehearsal is a tested-day outcome, never
  a guarantee.

## 3. Handoff notes

1. **Agent A reconciliation (DONE at close)**: A's records landed; the
   expected-path citations were reconciled to the actual names
   (`records/02_p12_02_backup_procedures.md`,
   `records/01_p12_01_backup_architecture.md`, artifacts under
   `deploy/compose/backup/`) in the runbook header, §2/§3 rows, and this
   record.
2. **Wave 2 / P12.13**: the §6 rehearsal-record template is a consumer
   input to the evidence-templates subtask — same EMPTY-cell discipline.
3. **Wave 2 / P12.14**: the JetStream leg's drill design belongs to
   P12.14; this runbook's DR-12 row stays a pointer, not a design.
4. **Operator**: the Q4 game-day slot (record 22 §2) is the proposed
   rehearsal vehicle; adoption is the operator's decision.
