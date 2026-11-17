<!-- P19.16 — Incident response plan v2. HONEST RECORD: a PLAN record.
     The severity ladder is REGISTERED here (X-8) before any exercise
     that could tune it; the escalation paths are PROCESS definitions
     over seams that exist only as WRITTEN-UNRUN design (the P13.11
     notification pipeline cannot send anything today — no channel
     provider exists, P13-E-2 OPEN); the tabletop exercise template has
     every cell EMPTY and the exercise is a HUMAN GATE that has never
     been scheduled or run. This record closes gap G-8's PROGRAM side
     only (records/21_p19_10_soc2_evidence_chain.md §5) — incident
     RESPONSE itself never happens in this program's history, and no
     incident, exercise, escalation, or notification has ever occurred.
     No certification, attestation, or audit outcome is claimed. -->
# P19.16 — Incident response plan v2: severity ladder (registered), grid-ops escalation paths (DSO / market / notification per P13.11 and the P16 seam), tabletop exercise template — human-gated, never run

| Field | Value |
|---|---|
| Subtask | P19.16 — Incident response plan v2 (`ledger_phase_19.md` §P19.16): severity ladder; grid-ops escalation paths (DSO/market/notification per the P13.11 pipeline and the P16 market-record interplay); tabletop exercise template — human-gated, never run |
| Wave / agent | Phase 19 wave 2, Agent B |
| Status | **PLAN RECORD — WRITTEN, NOTHING EVER ESCALATED, DECLARED, OR EXERCISED.** No incident has ever occurred in this program's history; no severity has ever been assigned; no escalation has ever been sent; no tabletop has been scheduled or run. The ladder is REGISTERED (X-8) before any exercise that could tune it; every exercise and escalation cell is EMPTY / NOT-STARTED. |
| Sources | The P13.11 notification-pipeline record (`../phase-13-enterprise-identity-integration/records/23_p13_11_notification_pipeline.md` — channel ports, severity vocabulary INFO/WARNING/CRITICAL, receipt-only return, never-a-dispatch-path rule, P13-E-2 OPEN; consumed BY CITATION); the P13.21 interplay record (`../phase-13-enterprise-identity-integration/records/47_p13_21_notification_audit_interplay.md` — the closed Notify/AuditOnly disposition vocabulary, exactly four Notify events; consumed BY CITATION); the P16 seam (`../phase-16-market-integration-settlement/records/61_p16_19_market_adapter_permission_model.md` — no market dispatch reach, `market.bid.submit` absent; and `records/01_p16_01_market_records_store.md` — market facts are records with audit-chain linkage); the P19.12 severity tiers (`records/23_p19_12_vulnerability_management_program.md` §3 — consumed for the vulnerability-triggers-incident linkage, never redefined); the P12.08 game-day runbook template (`../phase-12-backup-dr-resilience/records/22_p12_08_game_days.md` — the exercise-template style precedent, cited not restated); the P19.14 threat-model refresh (`records/25_p19_14_threat_model_refresh.md` — the surface registry IR scope derives from); the P19.10 evidence map gap G-8 (`records/21_p19_10_soc2_evidence_chain.md` §5); conventions §5 (human gates / X-8) |
| Files touched | This record only. No code, no config, no exercise file — the tabletop template is a shape defined HERE, instantiated at human exercise time. The P13.11/P13.21/P16/P12 producer artifacts are cited, never edited. |

## 1. Honest status — the plain statement

**No incident has ever been declared, triaged, escalated, or reviewed
anywhere in this program's history, and no exercise has ever run.** This
record is "v2" by posture only: the P0.17 threat model's §17.3 controls
and the P12.08 game-day template are the v1-generation material it sits
above — neither is edited, and the P0.17 file stays byte-identical (the
P19.14 refresh already extended it as design, by citation). What this
record adds is the standing incident-response PROGRAM:

1. the **severity ladder** — four levels, registered (X-8) before any
   exercise that could tune them, with explicit linkage to the P13.11
   notification severities and the P19.12 vulnerability tiers it
   consumes, never redefines (§3);
2. the **escalation paths** — grid-ops (DSO-facing), market-facing, and
   the notification pipeline, each defined honestly against what the
   seam can actually do today (§4);
3. the **tabletop exercise template** — shape only, every cell EMPTY,
   the exercise a human gate never scheduled (§5).

The program's most important honesty property: **FlexCore is a
supervisory orchestration layer with no field write authority at any
0.x release (AGENTS.md §3 rule 5; the simulation-only ceiling).** An
incident in this program's history can only be a simulation, lab, or
software/supply-chain incident — never a field event. The plan is
written so that the DSO-facing and market-facing paths are PROCESS
definitions for the human organization; no automated path in the
platform can declare, escalate, or respond to anything (§4).

## 2. What "v2" upgrades — and what it consumes unchanged

| Carried (cited at its path, never restated or edited) | Added by this plan |
|---|---|
| P0.17 five zones, §17.3 controls, §17.4 threats, §17.5 invariants (FROZEN; byte-identical; refreshed as design only by `records/25_p19_14_threat_model_refresh.md`) | §3 the standing severity ladder (new vocabulary — see the note below) |
| P12.08 game-day runbook template + quarterly cadence PROPOSAL (WRITTEN-UNRUN; no game day has run) | §5 the tabletop template, in the same human-gated style |
| P13.11 pipeline severities INFO/WARNING/CRITICAL; the receipt-only port shape; the never-a-dispatch-path rule | §3.2 the ladder→notification-severity mapping (consumed vocabulary, no redefinition) |
| P13.21 the closed disposition vocabulary (exactly four Notify events, `DispatchFailed` and `PluginFailed` at CRITICAL) | §4.3 the notification path's honest capability statement |
| P19.12 SLA tiers T-1..T-4 (registered, X-8) | §3.3 the vulnerability→incident linkage rule (consumes the tiers; never re-tiers a finding) |
| P16.19 the market permission posture (no dispatch reach; `market.bid.submit` absent) + P16.01 record-only market facts | §4.2 the market-facing escalation path |

**Vocabulary note (structural, deliberate):** the IR ladder
(SEC-1..SEC-4, §3.1) is a NEW registered vocabulary. It does not reuse
the INFO/WARNING/CRITICAL names (P13.11's, byte-identical) nor the
T-1..T-4 tier names (P19.12's, registered), because reusing a name for a
different meaning is exactly how two honest registers silently fork. The
mappings (§3.2, §3.3) are recorded as explicit crosswalk tables instead.

## 3. The severity ladder — REGISTERED (X-8) before any exercise

### 3.1 The ladder

All level definitions are **RECOMMENDATION-class registered
definitions**; the human owner tunes them at the first exercise or real
review, and any change is a recorded amendment to this table (never an
informal drift). No severity has ever been assigned to anything.

| Level | Definition (registered) | Examples in this program's honest world |
|---|---|---|
| SEC-1 | Compromise or integrity failure of a control-plane trust surface (authn/authz bypass, audit-chain integrity loss, signing-key compromise) OR any confirmed compromise of an edge-agent update path — regardless of environment | (none — nothing has ever occurred) |
| SEC-2 | Confirmed exposure of credential-class material, a tenant-isolation defect, or a supply-chain integrity failure (a compromised dependency/artifact actually ingested) | (none) |
| SEC-3 | A vulnerability or defect meeting the P19.12 T-2 trigger profile that is not yet a suspected compromise, or a failed drill/exercise revealing an unmitigated design gap | (none) |
| SEC-4 | Informational security events with no suspected impact (scanner findings triaged in their normal program, logged anomalies) | (none — and zero scanner findings exist to even classify: P10.20 §6 is EMPTY) |

Assigning a level is a HUMAN act by the incident owner (§6). No
automation in the platform assigns severities, and no test or scanner
output promotes itself to an incident.

### 3.2 Crosswalk to the P13.11 notification severities (consumed, not redefined)

The P13.11 vocabulary (INFO / WARNING / CRITICAL) and the P13.21
dispositions stay byte-identical; the crosswalk is recorded HERE as
data, and amending it is a recorded change to this table, never to the
pipeline:

| IR level | Notification severity floor (per the P13.11 preference model) | Honest note |
|---|---|---|
| SEC-1 | CRITICAL on all composed channels | The pipeline composes NOTHING today — no channel provider exists (P13-E-2 OPEN); this row is the process requirement on the human escalation, not a capability |
| SEC-2 | CRITICAL | same honest note |
| SEC-3 | WARNING | same honest note |
| SEC-4 | INFO (or audit-only — the P13.21 default for everything not in the four Notify rows) | The pipeline's conservative default is structural: absence from the disposition table = audit-only |

The crosswalk never makes a notification a dispatch path: the P13.11
rule (§2 of its record) is consumed verbatim in substance — a channel
provider physically has nothing to call and nothing to return, and IR
escalation messages carry facts that already happened (§4.3).

### 3.3 Crosswalk to the P19.12 vulnerability tiers (consumed, not redefined)

| Linkage rule (registered) | Content |
|---|---|
| VL-1 | A scanner/pen-test finding is triaged under the P19.12 tiers, ALWAYS — an IR level is assigned only when the finding additionally meets a §3.1 level definition (e.g. evidence of exploitation: T-1's actively-exploited trigger is a SEC-1 candidate; a T-4 advisory alone is SEC-4 at most) |
| VL-2 | The tier clock and the IR clock are the SAME remediation program (the P19.13 §6 one-program rule, carried): an incident never creates a second, weaker remediation path |
| VL-3 | A re-graded finding re-opens both clocks under their own registers — never re-tiered inside the IR ladder |

## 4. Escalation paths — process definitions over seams that exist only as design

### 4.1 Grid-ops / DSO-facing path

| Property | Registered content |
|---|---|
| Who escalates | The named human incident owner (§6) — never an automated path |
| To whom | The human operator's DSO/market-operator counterpart per the deployment's operating agreement — the counterpart's identity is a deployment fact, `<EMPTY>` here (no deployment exists) |
| What today's reality is | **No DSO connection exists anywhere in the program.** The v0.x line is simulation-only with no field write authority (AGENTS.md §3 rule 5; release ladder §4); a "grid incident" cannot occur in the program's history, and the path below is defined for the v1.0.0-class posture (operator-approved control, per-site qualification) that does not exist yet |
| What is sent | Facts that already happened, with platform-observed state only: which lifecycle states were reached, what the audit chain records — never a claim about physical grid state (AGENTS.md §4: acknowledgement is not physical delivery; the platform never records an outcome it did not observe) |
| Never sent | Any instruction, any control action, any request that a counterpart act on FlexCore's behalf — the escalation is informational by construction |

### 4.2 Market-facing path

| Property | Registered content |
|---|---|
| Who escalates | The named human incident owner |
| The seam's honest shape | Market adapters hold NO dispatch authority and cannot submit bids (`market.bid.submit` is deliberately absent — P16.19 §1); market facts are RECORDS with audit-chain linkage on every row (P16.01). An incident touching market records is therefore a RECORD-INTEGRITY incident, and the response is correction through the record layer |
| The response path | Suspected incorrect market records are corrected through the P16.12 dispute workflow's record-only mechanics (`../phase-16-market-integration-settlement/records/26_p16_12_dispute_workflow.md` — cited, never restated) and the audit chain; **no market "action" exists for IR to take** |
| External notification | To the market operator counterpart per the deployment's operating agreement — a human act, the counterpart `<EMPTY>`; no automated market notification exists |

### 4.3 Notification path (the P13.11 pipeline — what it can and cannot do today)

| Property | Honest state |
|---|---|
| The pipeline's capability | Written-unrun design. NO channel is configured, NO provider account exists (P13-E-2 OPEN), NO notification has ever been sent; the template registry ships empty; the hosting task is an unrecorded composition step (P13.11 §5) |
| What IR actually uses today | **The escalation paths are HUMAN-OPERATED** (email/phone by the human owner). The pipeline is recorded as the platform-side notification mechanism for the operator-approved posture; until composed, the honest silent state stands (P13.11 §5 step 3: no port composed → empty receipts) |
| Structural rule consumed | Notifications are never a dispatch path: the ports carry receipt-only returns; the escalation of an incident changes no command state, advances no lifecycle, and authorizes nothing |
| The four Notify events | Per P13.21, exactly `TelemetryQualityChanged`, `AssetAvailabilityChanged`, `PluginFailed`, `DispatchFailed` notify (all as facts already in the audit trail). An incident DECLARATION is not among them and must never be added as an automated emit — declaring an incident is the human gate of §6 |

## 5. Tabletop exercise template — shape only, every cell EMPTY, never run

The template follows the P12.08 game-day style (cited, not restated):
pre-registered scenario, observable decisions, post-exercise findings —
with every content cell EMPTY because no exercise has ever been
scheduled or run. **Running a tabletop is a HUMAN GATE; no exercise is
scheduled, and none may be claimed.**

| Section | What a completed tabletop record will contain | Cell status |
|---|---|---|
| T-1 Scenario | One inject per run, drawn from the §3.1 level definitions (e.g. a signed-update verification failure inject — the P18.02 seam) — the scenario is registered BEFORE the exercise (X-8) | `<EMPTY>` |
| T-2 Participants and roles | Named humans, including the incident owner (§6) and the DSO/market counterpart roles if exercised | `<EMPTY>` |
| T-3 Injects and timing | The inject schedule | `<EMPTY>` |
| T-4 Decisions observed | Which escalation paths (§4) were taken per inject, and by whom | `<EMPTY>` |
| T-5 Findings | Gaps found (process, tooling, contact availability) | `<EMPTY>` |
| T-6 Amendments | Ladder/escalation amendments arising (each a recorded change to §3/§4, versioned) | `<EMPTY>` |
| T-7 Evidence pack | Attendance, timeline, notes — the audit-export pack format (P13.12) is the recorded evidence vehicle | `<EMPTY>` |

Rules: an exercise without a pre-registered T-1 is not run; findings
that propose threshold changes enter as §3/§4 amendments with a version
note, never as side conclusions; an exercise produces FINDINGS, never
proof of readiness, and no exercise outcome may be worded as a
certification, an attestation, or an audit result.

## 6. Roles and the human-gate map

| # | Gate / role | Content | State |
|---|---|---|---|
| R-1 | Incident owner | The named human who assigns the SEC level, drives §4, and owns closure — named per deployment; none exists | `<EMPTY>` / NOT-STARTED |
| R-2 | Incident declaration | ALWAYS a human act; no automated declaration exists or may be added (the P13.21 structural rule for declarations, carried) | No incident has ever been declared |
| R-3 | Escalation execution | Human-operated until the P13.11 pipeline is composed with real channels (P13-E-2 closure is a prerequisite, not a substitute for the human act) | NOT-STARTED |
| R-4 | Tabletop scheduling + execution | Human gate (§5) | NOT-STARTED — never scheduled |
| R-5 | Contact register (DSO/market counterparts, internal escalation) | Deployment facts; every cell `<EMPTY>` | `<EMPTY>` |
| R-6 | Post-incident review | Human act; produces amendments (T-6-style) and, where a vulnerability was involved, entries in the P19.12 exception register under its §5 discipline | No incident exists to review |

## 7. Not-a-certification statement (verbatim discipline carried)

Quoted verbatim from `AGENTS.md` §3 (rules 2–3), as the P10.25 §7
discipline requires of every phase-19 record:

> 2. **No safety or certification claims.** FlexCore is not certified,
> not a safety case, and not permission for live grid control. Never
> write text that implies otherwise.
> 3. **No protection-grade claims.** FlexCore is a supervisory
> orchestration layer. It does not perform primary frequency response,
> synthetic inertia, relay protection, inverter inner-loop control, or
> sub-cycle control.

Specifically for this record: an incident-response PLAN is not an
incident-response CAPABILITY, an exercised plan is not a certified one,
and no statement here may be read as readiness, resilience, or
operational-security claims. No incident, escalation, exercise, or
notification has ever occurred; no exercise outcome exists to quote.

## 8. What this record does NOT do (honest limits)

1. It closes gap G-8's PROGRAM side only (`records/21_p19_10_soc2_evidence_chain.md`
   §5 G-8: "IR plan v2 lands in wave 2; no tabletop has ever run"); the
   operating evidence (exercises run, escalations executed) remains the
   human-gate queue tracked in the close record.
2. No producer artifact was edited or restated: P13.11, P13.21, P16.19,
   P16.01, P16.12, P12.08, P19.12, P19.14, and the frozen P0.17 file are
   consumed at their paths with their statuses carried unchanged.
3. No severity has been assigned, no escalation drafted, no contact
   recorded, no exercise scheduled; every dynamic cell is `<EMPTY>` /
   NOT-STARTED.
4. The ladder and crosswalks are REGISTERED RECOMMENDATIONS (X-8);
   tuning them is the human exercise/review act (§3, §5 T-6).
5. The never-granted permission set stays byte-identical — an IR plan
   grants no authority: `dispatch.authorize`, `dispatch.issue`,
   `secret.read`, `network.any` (quoted unchanged per conventions §6.4).

## 9. Reviewer-verification procedure (human)

1. Confirm §1/§7/§8: no incident, exercise, escalation, or notification
   is claimed; the AGENTS.md §3 rules 2–3 block is verbatim.
2. Confirm §3.1/§3.2/§3.3 are REGISTERED (X-8) crosswalks that consume
   (never redefine) the P13.11 severities and the P19.12 tiers, and that
   no example row contains a real event.
3. Confirm §4 states plainly that every path is human-operated today and
   that no automated declaration/escalation exists or may be added.
4. Confirm §5's cells are all `<EMPTY>` and no exercise is scheduled.
5. Confirm the cited producer records resolve on disk and are untouched
   (`git status` on the phase-13/phase-16/phase-12 records and P0.17).

## 10. Status

**WRITTEN / PLAN-ONLY — G-8's program side prepared; nothing has ever
been declared, escalated, or exercised.** Ticking P19.16 means
"implemented (plan record written), awaiting human exercises, contacts,
and review" — never "ready", never "exercised", never "resilient".
