<!-- P20.14 — Release engineering runbook v2 (RC → release pipeline as an
     operational procedure). The P10.18 gate sequence is consumed AS THE
     PIPELINE'S STAGES (cited, never restated — 24 UNRUN jobs stay the
     property of that record); artifact promotion is digest-pinned with no
     per-env rebuilds; the P10.12 upgrade/rollback TEMPLATE becomes this
     runbook's drill section, with its EMPTY run record remaining the
     human gate input. NOTHING was built, promoted, tagged, published, or
     drilled. -->
# P20.14 — Release Engineering Runbook v2 (RC → release as an operational procedure — WRITTEN-UNRUN; nothing built, promoted, published, or drilled)

| Field | Value |
|---|---|
| Subtask | P20.14 — Release engineering runbook v2 (`ledger_phase_20.md` §P20.14) |
| Wave / agent | Phase 20 wave 1, Agent B |
| Status | **OPERATIONAL-PROCEDURE RECORD — WRITTEN-UNRUN.** No release candidate exists (P10.18 §1), no CI job has ever run (P10.18 F-1: zero of the jobs have ever executed), no artifact exists, no promotion has happened, v1.0.0 is NOT released (P10.28), and no drill has executed. This runbook describes the pipeline; it executes nothing and fills nothing. |
| Consumed by citation (never restated) | P10.18 CI-gate record (`ledger/phase-10-v1-release-hardening/18_ci_gate_run_record.md` — the §2 gate checklist (24 UNRUN jobs) IS the pipeline's stage list; the §5 run record stays an EMPTY template); P10.12 upgrade/rollback record (`ledger/phase-10-v1-release-hardening/12_upgrade_rollback_rc.md` — the §7 EMPTY run-record TEMPLATE becomes this runbook's drill section input; nothing re-decided); P10.28 publication record (`ledger/phase-10-v1-release-hardening/28_release_publication.md` — PREPARED-NOT-EXECUTED; the precondition chain and publication gates are consumed verbatim in substance) |
| Sources | `docs/operations/deployment-upgrade-model.md` §1–§3 (the mode table and upgrade model the environments upgrade within); `deploy/compose/docker-compose.yml` (the image identities promotion carries); P20.13 §2 (I-1 digest-pinned parity invariant — this runbook operationalizes it) |
| Files touched | this record only |

## 1. Honest status — the plain statement

**There is no candidate, no artifact, no CI execution, no promotion, and
no release.** The 0–10 records own the SEMANTICS: P10.18 owns the gate
checklist and its EMPTY run record; P10.12 owns the upgrade/rollback test
matrix and its EMPTY run record; P10.28 owns publication and its
UNSIGNED authorization gate. This runbook adds the OPERATIONAL SHAPE that
connects them into a repeatable RC → release pipeline (§3), the
digest-pinned promotion discipline (§4), and the rollback-drill linkage
(§5). It restates no gate, no refusal order, no publication precondition
— every such element is cited by path. Nothing in this record runs,
builds, promotes, or publishes anything.

## 2. The pipeline at a glance

```text
RC BUILD          PROMOTION (digest-pinned, no rebuilds)        RELEASE
P10.18 §3 §7  -->  stage gates per environment (P20.13 parity) --> P10.28 §3
  |                    |                                            |
  24 UNRUN jobs        drill section (P10.12 template)              UNSIGNED
  EMPTY run record     per-environment rollout records              human gate
```

The pipeline is a HUMAN-EXECUTED procedure at every arrow. No agent
executes any stage; no stage self-executes on a green predecessor; every
stage's evidence lands in the record that owns it.

## 3. Stages — consumed from P10.18, not restated

| Stage | Content (the owner of the semantics) | Operational addition (this runbook) | Evidence |
|---|---|---|---|
| S-1 Candidate build | P10.18 §3 (the exact-command build procedure) + the §7 `release-candidate-build` dispatch job | The dispatcher is the human; the build ID and hashes land in P10.18 §4's identity procedure — never in a local note | `<EMPTY — UNRUN>` |
| S-2 Gate run | P10.18 §2 (the §22.2 gate checklist mapped to the 24 actual jobs; a skipped gate is not a passed gate) | The stage ordering rule: the FULL gate set runs on the SAME commit the candidate builds from (P10.18 §4's clean-checkout rule, consumed) | `<EMPTY — UNRUN>` |
| S-3 Identity fill | P10.18 §4 fills the `<PENDING P10.18>` cells of the P10.01 §6.1 freeze | No stage may proceed on a provisional freeze | `<EMPTY>` |
| S-4 Go/no-go | The P10.27 decision record (its decision meeting has not been held — P10.28 §1) | CI green is an INPUT to the decision, never the decision (P10.18 §11, consumed) | `<EMPTY — NOT HELD>` |
| S-5 Publication | P10.28 §3 (the exact publication procedure) behind its §4 UNSIGNED authorization gate and the §9 precondition chain (zero links satisfied) | Publication happens ONCE; environments promote FROM the published digest, never from a local build | `<EMPTY — UNSIGNED>` |

## 4. Artifact promotion — digest-pinned, no per-environment rebuilds

The promotion discipline this runbook operationalizes (and P20.13 I-1
requires):

| Rule | Statement |
|---|---|
| P-1 One artifact set | ONE digest-pinned artifact set per release; every environment runs the same digests — a digest mismatch between environments is a parity FINDING (P20.13 §2 I-1), never a rebuilt image |
| P-2 Promotion = reference change | Moving an artifact between environments changes a REFERENCE (the environment's pinned digest), never rebuilds the artifact; no per-env rebuild exists in this pipeline and none may be added silently |
| P-3 Promotion record | Each promotion is a recorded event: which digest, into which environment, by whom, with the §5 drill (or the environment's rollout record) attached — an unpromoted-on-paper environment is a finding |
| P-4 Promotion order | dev → staging → prod; an environment may not run a digest no lower environment has exercised (the order makes test evidence transferable; it is a recorded rule, not a tooling property) |
| P-5 Parity gates between environments | The P20.13 §2 invariants are the between-environment checklist; §3's recorded differences are the only permitted deltas |
| P-6 Source honesty | No artifact exists today; the first promotion happens when P10.18's first run produces one — every promotion cell in this record is `<EMPTY — NEVER PROMOTED>` |

## 5. Rollback drill linkage — the P10.12 TEMPLATE becomes the drill section

The P10.12 record owns the upgrade/rollback SEMANTICS and the 15-case
matrix (U-1..U-15). This runbook makes that record's §7 EMPTY run-record
TEMPLATE the drill section of the operational procedure — the template's
EMPTY cells remain the human gate input, exactly as that record §9
prescribes:

| Drill linkage | Rule |
|---|---|
| D-1 | The drill executes P10.12 §5's matrix against a promoted candidate; results are filled INTO the P10.12 §7 template FROM THE RUN ONLY — this runbook never receives or duplicates results |
| D-2 | The central rows (U-12..U-15) require TWO buildable versions; the upgrade SOURCE does not exist (P10.12 §3.1) — until a prior release exists, those rows stay BLOCKED_SOURCE_BUILD and no drill of them is scheduled or claimed |
| D-3 | Timings come from the run's clocks only; every manual intervention is recorded verbatim (P10.12 §6, consumed) |
| D-4 | A drill failure produces a FINDING with a disposition (the inherited P9.24 §4 judgement rule) — never a re-worded criterion, never a silent re-run |
| D-5 | A failed promotion rolls BACK by re-pointing the environment reference to the previous digest (P-2's reference-change rule) — and an irreversible-migration-bearing failure rolls back by restore-from-backup, never an improvised down-migration (the P10.13 rule, consumed) |

## 6. What this runbook does NOT establish

1. No candidate, artifact, digest, promotion, drill, or release exists —
   every execution cell is EMPTY; v1.0.0 remains NOT RELEASED (P10.28).
2. It does not restate the P10.18 gate list, the P10.12 refusal orders or
   matrix, or the P10.28 precondition chain — those records own them.
3. It does not close any gate, decision, or signature: the P10.27 go
   decision has not been held; the P10.28 authorization is UNSIGNED; the
   D4 trap (the CI license gate is only the automated scan) carries
   verbatim in substance from P10.18 §6.
4. It adds no dispatch authority (ADR-010) and touches no workflow file,
   no build script, and no release artifact (hard bans 1/2/6).
5. §13.3 semantics: a future green pipeline is a tested-day outcome,
   never a guarantee.

## 7. Reviewer-verification procedure (human)

1. Confirm §1's plain statement and that every evidence/promotion cell is
   EMPTY.
2. Confirm §3 consumes the P10.18 stages by citation (the 24-job list is
   NOT duplicated here) and keeps "CI green is an input" intact.
3. Confirm §4's P-1/P-2 digest-pinning rules and P-4's promotion order;
   confirm no per-env rebuild is described anywhere.
4. Confirm §5 makes the P10.12 §7 template the drill input without
   copying or filling any cell; confirm D-2's BLOCKED_SOURCE_BUILD
   honesty and D-5's restore-based rollback.
5. Confirm no ledger checkbox was ticked by this agent and no file
   outside this record was touched.

## 8. Findings and assumptions

| # | Item | Note |
|---|---|---|
| A-P20.14-1 | The pipeline assumes the P10.18/P10.12/P10.28 records remain the semantic owners | If those records change, this runbook's citations re-resolve; the anti-silent-edit discipline governs |
| A-P20.14-2 | Promotion-order rule P-4 is this runbook's recorded discipline | No external source; it is a deliberate recorded rule, labeled as such |
| F-P20.14-1 | Zero CI executions, zero artifacts, zero promotions exist; the pipeline has never run | Binding honesty statement (P10.18 F-1; P10.28 §1) |
| F-P20.14-2 | The central drill rows (U-12..U-15) are blocked by the missing upgrade SOURCE, which no part of this runbook can produce | Carried unchanged from P10.12 §3.1/§10 F-P10.12-4 |

Status: **WRITTEN-UNRUN** — a runbook cannot release by being written;
the candidate build, the gate run, the go/no-go, the publication, and the
drills are human gates that have not happened and are not scheduled.
