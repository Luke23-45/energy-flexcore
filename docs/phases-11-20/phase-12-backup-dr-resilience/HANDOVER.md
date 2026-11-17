# HANDOVER — Phase 12: Survivability — Backup, DR and Resilience Engineering

Status: **implementation complete (expected 16/16), everything
WRITTEN-UNRUN.** The ledger's P12.01–P12.12 ticks are applied (by the
orchestrator, citing records 01–06 and 21–26); P12.13–P12.16 carry the
wave-2 decision records (Agent A: records 31–33; Agent B: records 51 and
the close record 52) and their ticks are the ORCHESTRATOR's action,
applied after this close (program conventions §2.2 — no agent ever ticks
a checkbox). The phase gate is **NOT MET** as a runtime claim: nothing has
been built, run, backed up, restored, drilled, injected, or measured — no
deployment exists to drill against (P9.24 §2.3), and the first human build
of the Phase 0–10 workspace remains the program gate for every statement
below.

## What was implemented (all WRITTEN, never run)

| Area | Subtasks | Records | Delivered (written) |
|---|---|---|---|
| Backup architecture + procedures | P12.01, P12.02 | 01, 02 | pgBackRest-vs-WAL-G selection (DEFAULT pgBackRest, every capability claim UNVERIFIED [S-68]); P9.24 §3 decision-template rows as RECOMMENDATION/DEFAULT; object-storage target shape; per-class retention structure with EMPTY periods (D1 OPEN); the `deploy/compose/backup/` procedure tree (backup/verify/snapshot scripts, stdlib-only sha256 manifest tool + complete unittest suite) — WRITTEN-UNRUN |
| Restore drill + RTO/RPO register | P12.03, P12.04 | 03, 04 | isolated-restore runbook citing P9.24 §5 R-0..R-10 / §6 E-1..E-6 (never rewritten) with EMPTY timed cells; per-tier RTO/RPO RECOMMENDATIONS (T1–T7) with implied cadences and EMPTY sign-off cells; the inherited judgement rule pre-printed |
| Standby + pooling designs | P12.05, P12.06 | 05, 06 | Patroni 1+1 topology design (DEPLOYMENT-gated), transaction-scoped `remote_apply` for the 0016/0008 transactions (conditional DEFAULT); transaction-mode pooling design consuming the P11.19 contract with the SQLx validation matrix V-1..V-5 (UNRUN) |
| Chaos + game days + fault injection | P12.07, P12.08, P12.09 | 21, 22, 23 | the four defining experiments (C-PG-1/C-NATS-1/C-WORKER-1/C-POOL-1) as pre-registered repeatable scripts with the shared harness + frozen 27-edge oracle transcription (`experiments/`); the game-day template + quarterly-cadence proposal; the extended fault taxonomy in `apps/worker/python/drill/fault_injection.py` (additive, stdlib-only, pure data-flow) with complete test mirrors — all UNRUN |
| Degradation + DR + edge review | P12.10, P12.11, P12.12 | 24, 25, 26 | the per-dependency-down matrix mapped to written code only (UNSPECIFIED cells recorded, never papered over); the DR site-loss runbook (restore-into-isolation, DNS/reconfiguration, communications — every cell EMPTY); the edge review with findings F-P12.12-1..5 (bounded, audited, fail-closed — never loss-free) |
| Evidence templates + rehearsal design | P12.13, P12.14 | 31, 32 | the consolidating evidence-template file (`runbooks/evidence_templates.md`: X-8 threshold register pre-printed and EMPTY, requirement-cells copy block, per-drill result-cell templates, pointers not rewrites); the JetStream stream-delete + restore-from-snapshot drill design (JS-0..JS-12), executing the P12-E-2 DEFAULT with the dependence recorded, restore authorization inherited from P10.11 §4.4 |
| Criteria + close | P12.15, P12.16 | 51, 52 | the resilience acceptance-criteria record (claims allowed vs claims NEVER allowed, under §13.3 semantics, with the claim-escalation rung ladder); this HANDOVER + CHECKLIST + the close record |

## Blockers carried (human gate inputs)

1. **The first human build** (`cargo build --workspace` → clippy → `cargo
   test --workspace`): every Rust/Python artifact in this phase is written
   against the WRITTEN-UNRUN 0–10 base; the first execution of the
   extended Python module, the manifest test suite, and the chaos scripts
   are human gates that may surface recorded defect expectations (record
   02 §6, record 21 §5, record 23 §4).
2. **No deployment exists** — every drill, restore, rehearsal, and game
   day requires one (P9.24 §2.3): the P12.03 restore drill, the P12.05
   failover drill (multi-host + DCS), the P12.14 stream drill (plus the
   scoped restore-authorization grant), the DR rehearsal, and the quarterly
   game-day schedule are all UNRUN/UNSCHEDULED/NOT GRANTED.
3. **Every threshold and value is a RECOMMENDATION**: the RTO/RPO register
   (record 04), the game-day cadence (record 22), the chaos CLI defaults
   (record 21 §5) — X-8 pre-registration (`runbooks/evidence_templates.md`
   §1) happens BEFORE any run that could fill them, by the human executor.
4. **The three E-decisions are OPEN** (P12-E-1 record 05 §4; P12-E-2
   record 02 §4 with the dependence recorded in record 32 §2; P12-E-3
   record 25 §7) — agents never close them; the closers and the closing
   evidence are named in each register entry.

## Decisions still needed (the closers)

- P12-E-1 (standby topology + sync level): human-owner sign-off, with the
  F-2/F-6 latency evidence (record 05 §5) and Phase 11 capacity signals;
  `remote_write` is the documented fallback.
- P12-E-2 (JetStream backup posture): the human owner closes it with the
  P12.14 drill's JS-9 event-plane loss-window evidence (record 32 §2, §6).
- P12-E-3 (second DR site): the human owner; the Phase 20 multi-region
  trigger consumes the decision unchanged.
- Binding RTO/RPO: the operator signs record 04's per-tier sign-off cells
  (against customer contracts and service requirements — the EMPTY basis
  cells); nothing is binding until then.

## Known risks / recorded deviations

See the close record `records/52_p12_16_phase_close.md` §3 for the full
audit list. Headlines: three stale expected-path citations to
`records/13_p12_13_evidence_templates.md` (records 03 §4 item 5 and §5
preamble; records 19 §3 item 1) point at a path the P12.13 template did
NOT land on — the template landed at `runbooks/evidence_templates.md`
(record 31); recorded, not fixed (file ownership). The `nats` CLI surface
([S-65]) and pgBackRest option names ([S-68]) are UNVERIFIED; PostgreSQL 18
remains an UNVERIFIED pin (P10.13); no restore-command CLI shape is
asserted anywhere (record 32 §2).

Full close position: `records/52_p12_16_phase_close.md`.
