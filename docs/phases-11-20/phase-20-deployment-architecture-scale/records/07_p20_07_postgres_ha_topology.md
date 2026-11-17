<!-- P20.07 — Postgres HA topology design: built by CITATION on the P12.05 /
     P12-E-1 design (docs/phases-11-20/phase-12-backup-dr-resilience/
     records/05), adding only the SCALE layer (witness/arbiter placement,
     drill cadence). TRIGGER: the first availability contract — never a
     default; nothing deploys on single-host Compose [S-67]. Restating the
     P12.05 design is a duplication finding (context constraint 4). -->
# P20.07 — Postgres HA Topology Design (consumes P12.05 by citation — scale layer only; TRIGGER: first availability contract, UNFIRED)

| Field | Value |
|---|---|
| Subtask | P20.07 — Postgres HA topology design (`ledger_phase_20.md` §2, row P20.07) |
| Wave / agent | Phase 20 wave 1, Agent A |
| Status | **DESIGN RECORD — WRITTEN, TRIGGER UNFIRED, NOTHING DEPLOYED.** No Patroni instance, no standby, no DCS node, no witness exists; single-host Compose remains the (never-brought-up) deployment shape. |
| Sources | [S-67] (Patroni replication modes); **P12.05 record** `docs/phases-11-20/phase-12-backup-dr-resilience/records/05_p12_05_standby_replication.md` (the Patroni topology, the `synchronous_commit` per-transaction-class design, the failover drill F-1..F-7 — DEFINED THERE, consumed here by citation); **P12-E-1** (OPEN with its conditional DEFAULT — consumed, not re-decided); migrations `0016_commands_p4_10.up.sql` and `0008_audit_records.up.sql` (the lifecycle table classes the P12.05 §3.1 design scopes — cited, not restated); P12.04 register (T1-variant framing) |
| Files touched | this record only |

## 1. Honest status — the plain statement

Phase 12 OWNS the backup/DR/Patroni design; this record consumes it by
citation and adds ONLY the scale layer (context constraint 4). Nothing
deploys on single-host Compose [S-67]: HA is justified by an availability
contract — a real customer/operational requirement the owner records — and
never as a default posture. The trigger is therefore: **the first recorded
availability contract**. It has not been recorded; the trigger is UNFIRED;
every evidence cell is EMPTY.

## 2. What is consumed from P12.05 (citations, never restated)

| P12.05 element | Consumed as |
|---|---|
| §2 the topology (one Patroni primary + one standby, physical streaming replication, DCS-backed election, standby-is-not-a-backup) | The BASE topology this record's scale layer extends — cited, unchanged |
| §3 the `synchronous_commit` design (`SET LOCAL` per transaction class; `remote_apply` DEFAULT conditional on measured cost, `remote_write` fallback; instance default unchanged) | The durability design for the lifecycle table classes (0016 commands/transitions, 0008 audit incl. 0019 chain) — cited; THIS record adds no level, no class, no re-decision |
| §5 the failover drill F-1..F-7 | The drill PROCEDURE this record adds a CADENCE and a scale addendum to — the steps live in P12.05 |
| P12-E-1 (OPEN, conditional DEFAULT) | The decision this record feeds evidence into; agents never close it |

Any reader needing the durability or topology substance reads record 05 —
this record deliberately does not repeat it.

## 3. The scale layer THIS record adds (design, nothing deployed)

| Element | Design | Trigger gate |
|---|---|---|
| DCS / witness placement | At the scale the trigger implies (multi-host), the DCS runs as a 3-node quorum (etcd/Consul-class, P12.05 §2's UNVERIFIED choice) placed on failure domains SEPARATE from the Postgres nodes; a witness-class member may stand in for a full DCS node only if the operator's failure-domain count is 2 — the witness/arbiter question P12.05 left `<EMPTY>` is answered here as the DESIGN rule: quorum members must not share a failure domain with both Postgres nodes | Deployable only when the availability contract trigger fires |
| Node count ladder | (1) P12.05's primary + standby (the DEFAULT at contract time); (2) a second standby enters only when a recorded read-scaling or RPO-tightening need exists — never added speculatively | Same gate |
| Drill cadence | The P12.05 §5 drill runs (a) once at deployment acceptance, then (b) on a RECORDED cadence proposed as QUARTERLY-DEFAULT (operator-tunable at deployment) — each run fills P12.05's EMPTY evidence cells from that run only; cadence changes are recorded amendments, never silent | Same gate |
| Drill scale addendum | One added step beyond F-1..F-7: F-8 — repeat F-6 (failover interval + loss window) with the DCS quorum degraded by one member, to observe failover behavior under partial quorum; pass criterion and evidence cell per the P12.05 conventions, `<EMPTY — UNRUN>` | Same gate |
| Compose boundary | Nothing in this record adds any service, container, or config to `deploy/compose/` — the boundary P12.05 §2 recorded is inherited verbatim | HARD GATE |

## 4. Trigger register row (cross-referenced, not re-registered)

The availability-contract trigger is distinct from the P20.04 K8s trigger
T-2 (second region): T-2 is about a second SITE; this trigger is about
first-site availability. Either may fire first; neither implies the other.

| Trigger | Observable condition | Status |
|---|---|---|
| First availability contract | The owner records a customer/operational availability requirement that single-host Compose does not satisfy (evidence: contract, SLA, or incident record) | **UNFIRED** — evidence pointer `<EMPTY>` |

## 5. What this record does NOT establish

1. No deployment: no Patroni, no standby, no DCS, no witness — every §3
   row is design gated on the unfired trigger.
2. No latency, RPO, or failover-interval number exists; P12.05's drill and
   this record's F-8 are the generators, and they have not run.
3. No re-decision of P12-E-1: the conditional DEFAULT stands as recorded;
   this record only names the cadence and placement rules the eventual
   deployment will use.
4. No table-class change: the classes P12.05 §3.1 scopes are cited
   unchanged; adding a class is a P12.05 amendment, not a P20 one.
5. No dispatch-authority change: failover is data-plane availability; the
   command lifecycle, authorization, tenant isolation, and audit gates are
   topology-independent.

## 6. Reviewer-verification procedure (human)

1. Confirm §2 cites record 05 element-by-element with no restatement
   (trace each row; the durability substance lives only in P12.05).
2. Confirm §3 adds only witness/DCS placement, the node ladder, drill
   cadence, and drill step F-8.
3. Confirm §4's trigger is UNFIRED with an EMPTY evidence pointer and is
   distinct from P20.04's T-2.
4. Confirm `deploy/compose/` is untouched.
5. Confirm every evidence cell in §3 (including F-8) is EMPTY/UNRUN.

## 7. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) When the owner records the first availability contract: deploy per
#     P12.05 §2 + this record §3, run the P12.05 §5 drill plus F-8 on the
#     recorded cadence, fill evidence cells FROM THE RUN ONLY, and take
#     the numbers to the P12-E-1 closure decision.
```

## 8. Findings and assumptions

| # | Item | Note |
|---|---|---|
| A-P20.07-1 | The DCS/witness placement rule (quorum members off the Postgres failure domains) is standard HA design reasoning, not a measured result | Verified for the chosen DCS at deployment (P12.05 A-3 carries the UNVERIFIED DCS specifics) |
| A-P20.07-2 | Quarterly drill cadence is a RECOMMENDATION | Operator-tunable at deployment; recorded as such |
| F-P20.07-1 | The trigger is UNFIRED; no HA component exists or is scheduled | Carried by design |

Status: **DESIGN — TRIGGER UNFIRED, NOTHING DEPLOYED**; a topology cannot
fail over by being designed; the availability contract, the deployment,
and the drills are the human gates and have not happened and are not
scheduled.
