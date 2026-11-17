<!-- P12.04 — RTO/RPO register per system tier. EVERY value below is a
     RECOMMENDATION pending operator sign-off — the sign-off cell is EMPTY
     in every row (human gate; LR-205; the §21.2 rule inherited from P9.24
     §4 / P10.11 §5: do not set RPO/RTO without customer and service
     requirements). No measurement exists anywhere in this record. The
     P9.24 §4 judgement rule is inherited: a measured value worse than the
     requirement is a FINDING, never a re-defined requirement. -->
# P12.04 — RTO/RPO Register per System Tier (RECOMMENDATIONS — sign-off EMPTY; nothing measured)

| Field | Value |
|---|---|
| Subtask | P12.04 — RTO/RPO register per system tier (`ledger_phase_12.md` §P12.04) |
| Wave / agent | Phase 12 wave 1, Agent A |
| Status | **RECOMMENDATION REGISTER — WRITTEN, NEVER MEASURED.** Every value is a RECOMMENDATION; every sign-off cell is EMPTY; no RTO/RPO has ever been measured; no requirement is set. |
| Sources | LR-205 (RPO/RTO unsettable — no customer, D1 OPEN; `ledger/phase-10-v1-release-hardening/17_operational_risk_review.md`); P9.24 §4 (placeholder requirements + the pre-printed judgement rule); P10.11 §5 (RC requirement cells re-opened, EMPTY); [S-82] (game days — recovery exercised by scheduled rehearsal, not asserted); [S-81] (steady-state measurement discipline); P12.01 §3/§5 (the cadence rows this register's RPO column implies); `docs/operations/deployment-upgrade-model.md` §5–§6 (degraded modes; the backup/restore model with placeholder RPO/RTO) |
| Files touched | this record only |

## 1. Honest status — the plain statement

**No RTO or RPO has ever been measured, and none is set here.** The §21.2
rule stands (P9.24 §4, P10.11 §5): do not set RPO/RTO without customer and
service requirements. What this register delivers is what plan §2 assigns:
per-tier RECOMMENDATION values with the backup/replication cadence each
value implies (which is what makes the P12.01 §3/§5 cadence rows and the
P12.02 procedure cadences derivable rather than arbitrary), plus the
inherited judgement rule. A value below becomes binding ONLY when the
operator signs the sign-off cell for that tier — a human gate whose cell
stays EMPTY here. LR-205 stays OPEN.

## 2. How to read the recommendation values

Each RPO RECOMMENDATION is derived from the recovery mechanism's bounding
mechanics, not from any measurement:

- **WAL-archived tiers:** the achievable-RPO bound is the WAL archive
  interval (the last archived segment) — the P9.24 §4 "achievable-RPO
  bound implied by the cadence/WAL cells" row, now filled with a
  RECOMMENDATION cadence instead of an EMPTY cadence.
- **Snapshot tiers (JetStream):** the bound is the snapshot cadence (the
  P12-E-2 DEFAULT posture).
- **Replication tiers:** the bound is the standby lag — governed by the
  P12-E-1 sync-mode choice (remote_apply bounds it to near-zero at a
  commit-latency cost [S-67]; the register's control-plane row states the
  two variants and stays agnostic until P12-E-1 closes).
- **Edge tier:** the bound is the audited bounded-buffer window (the
  drop-of-oldest loss window, R-P9.24-5) — a RECOMMENDATION about what the
  code can already bound, never a new capability.

Every RTO RECOMMENDATION is an engineering default for the restore drill
to measure against — nothing more.

## 3. The register — per tier

All sign-off cells EMPTY. Basis cells EMPTY (no customer contract exists).

| Tier | Components (written code) | RTO RECOMMENDATION | RPO RECOMMENDATION | Cadence / mechanism implied | Basis (cited) | Operator sign-off |
|---|---|---|---|---|---|---|
| T1 — Transactional store (control plane data) | PostgreSQL 18 (UNVERIFIED pin, P10.13); migrations 0001–0019 | ≤ 4 h (drill-measured restore into isolation) | ≤ 15 min (WAL archive interval bound) | pgBackRest full weekly + incr daily, WAL continuous (P12.01 §3) | `<EMPTY>` | `<EMPTY — human gate>` |
| T1-variant — after P12-E-1 closes with remote_apply | same store, standby in place | failover-level RTO (minutes-class, drill-measured) | near-zero lag bound for command-lifecycle transactions (remote_apply) | Patroni 1 primary + 1 standby (DEPLOYMENT-gated) | `<EMPTY>` | `<EMPTY — human gate>` |
| T2 — Event plane | NATS JetStream, four provisioned streams (`topology.rs`) | ≤ 4 h (same drill window as T1; the P12.14 drill measures stream restore) | ≤ 60 min (snapshot cadence bound; the P12-E-2 DEFAULT) | per-stream snapshot cadence into the shared object-storage target | `<EMPTY>` | `<EMPTY — human gate>` |
| T3 — Control-plane service / API | `apps/control-plane` (stateless behind the store + broker) | ≤ 30 min (redeploy from artifacts; no data copy) | 0 (stateless — the store carries state) | artifact redeploy; health-gated per P10.12 §3.1 | `<EMPTY>` | `<EMPTY — human gate>` |
| T4 — Python workers | `apps/worker` (stateless consumers; command POST + consumer idempotency per the P11.11/P11.12 records) | ≤ 30 min (redeploy) | 0 (durability is the broker's + outbox's) | artifact redeploy; redelivery on reconnect | `<EMPTY>` | `<EMPTY — human gate>` |
| T5 — Edge agent | `crates/edge-agent` (bounded store-and-forward, audited drop-of-oldest) | site-local: edge keeps operating offline by design (deployment-upgrade-model §5) — no central RTO applies | = the bounded buffer window (outage-duration × telemetry-rate sizing, `<PLACEHOLDER>` per P9.18/P9.39 measurement — never assumed) | buffer capacity bound configured per site; journals self-recover; no separate backup (P12.01 §3) | `<EMPTY>` | `<EMPTY — human gate>` |
| T6 — Operator console | `console` (client of the API) | ≤ 30 min (redeploy/reload; no independent state) | 0 (stateless) | follows T3 | `<EMPTY>` | `<EMPTY — human gate>` |
| T7 — Observability | OTel collector gateway, metrics store (P11.10 record) | best-effort — observability data loss is not a dispatch-safety item; no recommendation asserted | n/a | collector buffers only; restore not designed | `<EMPTY>` | `<EMPTY — human gate>` |

Notes on specific rows:

- **T1 RPO:** 15 min is the WAL-interval engineering default, chosen to be
  plausibly reachable by continuous archiving; it is NOT a claim that the
  mechanism achieves it — the drill measures, the judgement rule decides.
- **T2 RPO:** 60 min matches the event-plane snapshot cadence implied by
  the P12-E-2 DEFAULT; the R-P9.24-7 gap (the DB-vs-stream loss-window
  mismatch) is measured only by the P12.14 drill.
- **T1-variant row:** exists only while P12-E-1 is OPEN; if the DEFAULT
  (remote_apply) is accepted after latency evidence, the row merges into
  T1; if remote_write or async is chosen, the RPO cell re-derives.
- **T5 RPO:** the bounded buffer is an audited LOSS WINDOW (R-P9.24-5,
  carried unchanged): a central outage longer than the bound provably
  loses the OLDEST records — counted, latched, never silent. Whether that
  window is acceptable is a customer/authority decision (the EMPTY basis
  and sign-off cells).
- **T7:** deliberately carries no asserted recommendation — inventing one
  would contradict the no-invention rule; the row records the posture.

## 4. The judgement rule — inherited, pre-printed

From P9.24 §4 (and P10.11 §5), verbatim-in-substance: **the measured values
are recorded against the requirements in force; a measured value WORSE
than the requirement is a FINDING with a disposition (fix and re-run /
signed time-bounded acceptance / blocks the gate) — never a re-defined
requirement, never a requirement fitted to a result.** This register
adopts the rule unchanged for every tier row: when the first real drill
fills its evidence (P12.03 §5), each measured value is judged against the
SIGNED cell for that tier — and if nothing is signed, the drill measures
against nothing, which is exactly the P9.24 §11 R-P9.24-2 finding.

## 5. What this register does NOT establish

1. It does not set requirements — RECOMMENDATION is the operative label on
   every value; LR-205 stays OPEN until a customer contract and service
   requirements exist and the operator signs.
2. It does not measure anything; no timing, no lag, no window exists.
3. It does not commit a cadence — the cadence column states what each
   recommendation IMPLIES; the deployment scheduler wiring is the human
   executor's act.
4. It does not claim the recovery mechanics work — every mechanism row
   cites written-unrun artifacts (P12.01/P12.02/P12.05) whose first
   execution is the human gate.
5. It adds no dispatch authority; a failover or restore event grants
   nothing (ADR-010; the command lifecycle gates are untouched).

## 6. Reviewer-verification procedure (human)

1. Confirm §1's plain statement: no measured RTO/RPO anywhere; no signed
   cell anywhere.
2. Confirm every value row carries the RECOMMENDATION framing and an EMPTY
   sign-off cell; confirm the T1-variant row's dependence on P12-E-1 is
   stated.
3. Confirm §4 restates the judgement rule as inherited (no softening) and
   that §3's notes preserve the R-P9.24-5 loss-window honesty for T5.
4. Confirm the cadence column matches the P12.01 §3/§5 rows and the
   P12-E-2 DEFAULT; no contradiction between records 01/02/04.
5. Confirm §5 does not claim any mechanism works.

## 7. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) At contract time: fill the basis cells from the customer contract and
#     service requirements (§21.2 / P9.10 §6); sign or amend each tier's
#     RECOMMENDATION into a requirement; the sign-off cell records the
#     operator, date, and row version.
# (c) After each real drill (P12.03/P12.14): judge measured values per §4;
#     a worse-than-requirement result is a FINDING with a disposition.
# (d) Re-derive the P12.01 §3/§5 cadence rows from the signed register.
```

## 8. Findings and assumptions

| # | Item | Note |
|---|---|---|
| A-P12.04-1 | The numeric recommendations are engineering defaults chosen for internal consistency with the cadence rows | No source external to this program supports them; they are assumptions until signed |
| A-P12.04-2 | T1 RTO presumes the drill-measured restore path of the P12.03 runbook | Unmeasured; the first drill may produce a worse value — that is a FINDING, per §4 |
| F-P12.04-1 | All basis and sign-off cells are EMPTY while D1 is OPEN and no customer exists | LR-205 unchanged; carried, not hidden |

Status: **WRITTEN-UNMEASURED** — a register cannot measure by being
written; the first measurement is a human-gated drill outcome and has not
happened and is not scheduled.
