<!-- P20.09 — Multi-region DR design (active/passive; Postgres + JetStream
     replication across regions; DNS failover with honest bounds; RPO/RTO
     consumed from the P12 registers; failover/failed-back runbook;
     residency consumed from P13.14). TRIGGER-GATED DESIGN RECORD:
     the trigger — a second-region requirement — is the P12-E-3 revisit
     condition, LINKED not restated, and it is UNFIRED. Nothing is
     deployed, no region is stood up, no DNS change exists, no replica
     exists. Every RTO/RPO/latency/TTL value cell is EMPTY or an explicitly
     labeled design shape consumed from a P12 register whose sign-off cells
     are themselves EMPTY. -->
# P20.09 — Multi-Region DR Design (TRIGGER-GATED — second-region requirement UNFIRED; decision-ready design, not a deployment)

| Field | Value |
|---|---|
| Subtask | P20.09 — Multi-region DR design (`ledger_phase_20.md` §P20.09) |
| Wave / agent | Phase 20 wave 1, Agent B |
| Status | **DESIGN RECORD — WRITTEN-UNRUN; TRIGGER UNFIRED.** No second region exists, no cross-region replica exists, no DNS failover configuration exists, no region has ever been stood up, and no runbook step has executed. The design is decision-ready against the day the trigger fires; it is not a commitment and it deploys nothing. |
| Trigger (named, UNFIRED) | A second-region requirement — the P12-E-3 revisit condition. **Linked, not restated:** the question, options, DEFAULT ("no second site in the current posture — DR is the documented site-loss runbook plus the restore-into-isolation rehearsal; revisit at the Phase 20 multi-region trigger"), and closer live in `docs/phases-11-20/phase-12-backup-dr-resilience/plan.md` §Decision points (P12-E-3) and in the P12.11 DR runbook record §7 (`docs/phases-11-20/phase-12-backup-dr-resilience/records/25_p12_11_dr_runbook.md`). This record supplies the decision-ready design that record points forward to; it does NOT re-open, re-word, or close P12-E-2/P12-E-3 (P20-E-2 in the phase-20 plan is the phase-level mirror of the same question and stays with the human owner — agents never close E-decisions, conventions §4). A fired trigger is a RECORDED OWNER EVENT with evidence (customer, regulator, or contract requirement), never an agent inference. |
| Consumed by citation (never restated) | P12.05 standby design (`docs/phases-11-20/phase-12-backup-dr-resilience/records/05_p12_05_standby_replication.md` — Patroni topology, P12-E-1 OPEN with DEFAULT); P12.04 RTO/RPO register (`records/04_p12_04_rto_rpo_register.md` — RECOMMENDATIONS, sign-off EMPTY, LR-205 OPEN); P12-E-2 JetStream snapshot posture (`docs/phases-11-20/phase-12-backup-dr-resilience/plan.md` §Decision points — DEFAULT snapshot-based backup; "replication revisited at a second-cluster trigger (Phase 20)"); P12.11 DR runbook (site-loss scenario, restore-into-isolation, DNS/reconfiguration N-steps — the single-site DR posture this design extends); P13.14 jurisdiction registry (`docs/phases-11-20/phase-13-enterprise-identity-integration/records/26_p13_14_jurisdiction_registry.md` — per-tenant residency as signed data) |
| Sources | [S-65] (NATS JetStream — mirror on a second cluster is a documented mechanism class; mechanism-level only, no capability claim); [S-67] (Patroni replication modes — the cross-region standby is the same mechanism family P12.05 cites, at wider distance); `docs/operations/deployment-upgrade-model.md` §2 (five-zone model — a second region replicates the Zone 2 shape, never a new zone vocabulary); `deploy/compose/docker-compose.yml` (the single-host shape that does NOT host any of this today) |
| Files touched | this record only |

## 1. Honest status — the plain statement

**No second region exists; no cross-region replication of anything has
ever run; no DNS record, health check, or failover automation exists; and
the trigger for any of it is UNFIRED.** The gap analysis Tier 3 posture
puts multi-region design behind growth-dependent triggers (`00_GAP_ANALYSIS.md` §5, §9), and Phase 12 recorded the specific
second-site question as P12-E-3 with the DEFAULT "no second site in the
current posture". This record is therefore exactly what plan §2 assigns:
the topology design that becomes implementable IF the owner records a
second-region requirement — nothing more. Single-host Compose
(`deploy/compose/docker-compose.yml`) remains the entire real deployment
shape; the P12.11 site-loss runbook remains the DR posture in force.

## 2. Topology — warm active/passive, consuming P12.05 (cited, not re-derived)

The recorded topology choice is **warm active/passive** (plan §6 P20-E-2
option (b) shape), designed so each mechanism extends a Phase 12 design
rather than replacing it:

| Element | Design | What it consumes / extends (citation) | Status |
|---|---|---|---|
| Regions | Region A (primary, the existing Zone 2 shape) + Region B (passive, the same five-zone shape with empty Zone 1/3–5 membership cells until a real site review) | `deployment-upgrade-model.md` §2 — the zone model is replicated, not re-invented | DESIGN; nothing exists |
| PostgreSQL | Patroni in EACH region; Region B holds a cascading standby of Region A's primary (or, at the option recorded at trigger time, an independent cluster fed by cross-region replication — the choice re-opens P12-E-1's sync-mode question at cross-region distance) | P12.05 §2 (the one-primary/one-standby DEFAULT, DEPLOYMENT-gated); [S-67] | DESIGN; extends, does not relax |
| Synchronous-commit scoping | The `SET LOCAL synchronous_commit` discipline of P12.05 §3.1 applies UNCHANGED inside each region; cross-region commits are NEVER synchronous by default — the commit-latency cost over wide-area distance is an UNMEASURED question that only the P12.05 F-2-class drill, re-run at cross-region distance, may answer | P12.05 §3 (cited); P12-E-1 (OPEN — not closed by this record) | DESIGN |
| JetStream | Region B runs its own NATS server with JetStream streams MIRRORED from Region A [S-65] — this is exactly the P12-E-2 "replication revisited at a second-cluster trigger (Phase 20)" revisit landing: the snapshot DEFAULT posture of P12-E-2 remains the single-region backup mechanism; mirroring is the cross-region ADDITION, and `num_replicas: 1` in-region is unchanged unless a deployment re-derives it | P12-E-2 (cited); [S-65]; `crates/adapters/nats/src/topology.rs` (the provisioned stream set mirrors) | DESIGN |
| Command lifecycle across failover | The 14-state vocabulary, the three gates, and the never-granted permission set are untouched; a promoted Region B control plane resumes in the same control-authority mode the deployment records — the mode is a deployment/per-site setting and a failover never RAISES it | `deployment-upgrade-model.md` §4 (mode ceilings); AGENTS.md §4 | INVARIANT |
| Backups | pgBackRest + stream snapshots (the P12-E-2 DEFAULT posture) stay per-region INTO EACH REGION'S OWN object-storage target; a cross-region backup copy is a residency question resolved per-tenant by the registry (§5 below) — never assumed legal | P12.01 §4 (the bucket shape, cited); P13.14 | DESIGN |

The standby-is-not-a-backup rule (P12.05 §2) carries verbatim in
substance: Region B's replicas bound failover for availability events;
PITR remains the recovery mechanism for corruption and operator error —
a replica faithfully replicates damage too.

## 3. DNS failover — honest bounds, no invented numbers

Failover at the routing layer happens by changing DNS answers toward
Region B. The honesty bounds are structural, and every value cell is a
deployment decision (`<EMPTY>`):

| Property | Honest bound | Value cell |
|---|---|---|
| Propagation | A DNS failover is bounded below by the record's TTL — clients and resolvers hold stale answers up to the full TTL; failover is NEVER instant, and no "near-zero downtime" claim is made | TTL per record: `<EMPTY — deployment decision>` |
| Health checking | The failover decision consumes a health check against Region B's `/ready` (the P1.22 semantics: named dependency checks + the shutdown gate — `apps/control-plane/src/probes.rs`); the check's interval/timeout/threshold pre-register BEFORE any drill that fills them (X-8) | Check parameters: `<EMPTY — pre-register at deployment>` |
| Failover trigger | DNS is moved by an explicit runbook act (human-gated, mirroring P12.11 §2's declaration/activation gates) — no automated global-traffic-manager is claimed, and none is asserted as available | Automation choice: `<EMPTY>` |
| Failback | The reverse named act under the same gates (the runbook §6); DNS TTL bounds the failback symmetrically | `<EMPTY>` |
| Edge reconnection | Edges reconnect by outbound session only (the F1 rule — no inbound listener exists); an edge holds its bounded store-and-forward posture across the failover window, with the audited drop-of-oldest loss window UNCHANGED | `deployment-upgrade-model.md` §5; P12.04 T5 row (cited) |

## 4. RPO/RTO framing — consumed from the P12 registers, never re-set

**This record sets no RPO and no RTO.** The framing is consumed from the
P12.04 register (`docs/phases-11-20/phase-12-backup-dr-resilience/records/04_p12_04_rto_rpo_register.md`), whose every value is a RECOMMENDATION with
an EMPTY operator sign-off cell, and whose basis cells are EMPTY because
LR-205 (RPO/RTO unsettable — no customer, D1 OPEN) is OPEN. The
multi-region layer's only RPO/RTO contribution is structural: cross-region
replication is the mechanism class that would re-derive the T1/T2 rows
from standby lag and mirror lag respectively, at a wider distance than the
P12.05 single-standby design assumed. The inherited judgement rule
(P12.04 §4 — a measured value worse than the requirement is a FINDING,
never a re-defined requirement) applies unchanged to any future
multi-region drill. Nothing here is measured; no lag number exists.

## 5. Data residency — consumed from P13.14 as data, never assumed

Residency constraints on a second region are **per-tenant data**, read
from the jurisdiction registry designed by P13.14
(`docs/phases-11-20/phase-13-enterprise-identity-integration/records/26_p13_14_jurisdiction_registry.md` — `residency_constraint` and
`cross_border_transfer` per (tenant, jurisdiction, data class), signed by
a named human or `UNSIGNED`):

| Rule | Statement |
|---|---|
| Placement gate | A tenant's data may live in Region B ONLY where a SIGNED registry row for that (tenant, jurisdiction, data class) permits the placement; an UNSIGNED or absent row blocks placement (fail closed — the P13.14 semantics consumed verbatim in substance) |
| No platform default | No "Region B is fine for everyone" row exists or may exist — the registry has no platform-default row by construction (P13.14 §2 rule 1) |
| Backup copies | Cross-region backup copies (§2 last row) resolve through the same signed rows; where the rows do not permit them, backups stay in-region |
| D1 | D1 is OPEN; no jurisdiction, regime, or residency constraint is named anywhere in this record (the P13.14 §5 limits carry) |

## 6. Failover / failed-back runbook — structure with every execution cell EMPTY

The runbook STRUCTURE is recorded; the steps become executable only on a
deployed second region, and every evidence cell is `<EMPTY — UNRUN>`:

| # | Step | Gate / oracle | Evidence |
|---|---|---|---|
| MR-1 | Pre-registration: TTL, health-check parameters, and any drill threshold recorded (X-8) BEFORE the drill | Conventions §5 | `<EMPTY>` |
| MR-2 | Steady-state capture: primary/standby identities, cross-region lag counters, command/transition counts per tenant, mirror lag per stream (the R-0 discipline cited from P9.24 §5) | State recorded | `<EMPTY — UNRUN>` |
| MR-3 | Declare + activate failover (the P12.11 §2 gate classes, consumed); DNS moved to Region B | Human gates, named owners | `<EMPTY — UNRUN>` |
| MR-4 | Region B readiness verification: `/ready` named-check body green; promoted Postgres preserves the 0008/0016 schema-level properties (the P12.05 F-3 inspection, re-run in Region B) | Inspection transcript | `<EMPTY — UNRUN>` |
| MR-5 | Command continuity: every command observed pre-failover is observed in the same or a strictly later valid 14-state lifecycle state; zero re-issued commands (the C-PG-1 oracle and R-6 criteria, cited — not re-specified) | Zero anomalies | `<EMPTY — UNRUN>` |
| MR-6 | Audit continuity: pre/post audit counts equal; the 0019 chain verifies on the promoted node (P12.05 F-5, consumed) | Zero deltas; chain verifies | `<EMPTY — UNRUN>` |
| MR-7 | JetStream continuity: mirrored streams deliver in-order at-least-once; consumer idempotency holds across the cutover (the P11.12 consumer idempotency seam, cited) | Transcript | `<EMPTY — UNRUN>` |
| MR-8 | Failed-back: repair Region A, re-establish replication Region B → A, judge go/no-go, move DNS back under the same gates | Human gates | `<EMPTY — UNRUN>` |
| MR-9 | Judge per the inherited P9.24 §4 rule; every manual intervention recorded verbatim (the P10.12 §6 rule, consumed) | Disposition recorded | `<EMPTY — UNRUN>` |

## 7. What this record does NOT establish

1. No region, replica, mirror, DNS record, health check, or runbook
   execution exists — every mechanism row is DESIGN; the trigger is
   UNFIRED and a fired trigger is the owner's recorded event.
2. No RPO/RTO value, no TTL, no lag bound, no failover-interval figure —
   every numeric cell is `<EMPTY>` or a RECOMMENDATION consumed from a
   P12 register whose own sign-off cells are EMPTY.
3. No residency constraint, jurisdiction, or regime is named; D1 stays
   OPEN; the P13.14 registry holds UNSIGNED rows only (nothing exists to
   sign).
4. No control-authority change: a failover never raises the mode; the
   command lifecycle gates, the never-granted permission set
   (`dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any`),
   and ADR-010 are untouched.
5. P12-E-3, P12-E-1, P12-E-2, P20-E-2 all remain OPEN — this record is
   the design those decisions point at, not a decision.
6. §13.3 semantics: a future successful drill is a tested-day outcome,
   never a guarantee.

## 8. Reviewer-verification procedure (human)

1. Confirm §1's plain statement and that the trigger is recorded UNFIRED
   with the P12-E-3 linkage (linked, not restated — the question text
   lives in the Phase 12 plan, not here).
2. Confirm §2 consumes P12.05/P12-E-2 by citation (the citations resolve;
   no topology row re-derives what the P12 records own) and carries the
   standby-is-not-a-backup rule and the no-mode-raise invariant.
3. Confirm §3 states the TTL/health-check bounds structurally with every
   value cell EMPTY, and no instant-failover or downtime claim.
4. Confirm §4 sets no RPO/RTO and cites the P12.04 register + LR-205.
5. Confirm §5 reads residency from the P13.14 registry as signed data
   with the fail-closed rule, and that D1 is not presupposed.
6. Confirm every §6 evidence cell is `<EMPTY — UNRUN>` and no ledger
   checkbox was ticked by this agent.

## 9. Findings and assumptions

| # | Item | Note |
|---|---|---|
| A-P20.09-1 | JetStream mirror on a second cluster is a documented mechanism class of NATS JetStream [S-65] | Mechanism-level citation only; no FlexCore mirror exists; specifics UNVERIFIED at deployment |
| A-P20.09-2 | Cross-region Patroni replication is the same mechanism family as the P12.05 single-standby design at wider distance | The commit-latency cost at wide-area distance is UNMEASURED; only a re-run drill may answer it |
| A-P20.09-3 | The topology choice (warm active/passive) matches plan P20-E-2 option (b)'s shape | Recorded as the decision-ready shape; P20-E-2 itself stays OPEN with DEFAULT (a) — no second region until the owner records the requirement |
| F-P20.09-1 | The trigger is UNFIRED and every execution cell in §6 is EMPTY | Binding honesty statement; a fired trigger is a recorded owner event |
| F-P20.09-2 | The second-region trigger appears in BOTH the P12-E-3 register (consumed here) and Agent A's landed P20.04 register (row T-2, `records/04_p20_04_kubernetes_trigger_register.md`, UNFIRED) | The two registers LINK the same condition (the P20.04 row explicitly cites P12-E-3); this record enumerates no second trigger list — the P20.04 §2 register is the single K8s-side list, the P12-E-3 register the authoritative question wording |

Status: **WRITTEN-UNRUN, TRIGGER UNFIRED** — a design cannot fail over by
being written; the second-region requirement, the deployment, and the
first drill are the human gates and have not happened and are not
scheduled.
