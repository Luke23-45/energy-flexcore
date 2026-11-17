<!-- P12.08 — Game-day runbook template + quarterly cadence. TEMPLATE ONLY:
     every filled field below is a `<EMPTY>` cell; no game day has ever
     been scheduled, prepared, or run. [S-82] is cited for the game-day
     practice; it makes no claim about FlexCore. -->
# Game-Day Runbook Template + Quarterly Cadence (P12.08 — nothing has run)

Date: 2026-11-14. Subtask: P12.08. Status: **TEMPLATE + SCHEDULE PROPOSAL,
WRITTEN-UNRUN.** No game day has been scheduled, staffed, prepared,
executed, or reviewed; every field the template defines is `<EMPTY>` until
a scheduled game day fills it. Sources: [S-82] (Gremlin — the game-day
practice: pre-registered hypothesis, bounded blast radius, roles, abort
criteria, evidence capture); the P12.07 experiment pre-registrations
(`experiments/`, record 21) are the experiment inventory this template
consumes.

## 1. The template (one copy per game day; every cell starts `<EMPTY>`)

```markdown
# Game Day — <ID: GD-Q<n>-<year>> — <date> (all cells <EMPTY> until the day)

## 0. Human-gate statement
- Approving operator: <EMPTY — a game day is HUMAN-GATED end to end [S-82];
  no agent may schedule, approve, or run one>
- Environment: <EMPTY — an ISOLATED lab deployment; never a pilot posture;
  no field write authority (AGENTS.md §5; plan §3 constraint 5)>
- The first human build of the Phase 0-10 workspace is green and its date
  is: <EMPTY>

## 1. Hypotheses under test (pre-registered, never improvised)
| # | Experiment ID | Steady-state hypothesis (verbatim from the experiment script) | Phase 11 anchor | Pass criterion the human will judge |
|---|---|---|---|---|
| 1 | <EMPTY: C-PG-1 | C-NATS-1 | C-WORKER-1 | C-POOL-1> | <EMPTY> | <EMPTY> | <EMPTY> |

## 2. Blast radius (bounded by design; re-stated per experiment)
| Experiment | Components affected | Tenants | Data class | Network segments | NOT affected (explicit) |
|---|---|---|---|---|---|
| <EMPTY> | <EMPTY> | <EMPTY: one synthetic tenant> | <EMPTY: simulation-only> | <EMPTY> | <EMPTY> |

## 3. Roles (named people, never teams-of-convenience)
| Role | Person | Responsibilities | Contact |
|---|---|---|---|
| Director | <EMPTY> | owns the run, calls abort, owns the timeline | <EMPTY> |
| Operator | <EMPTY> | executes the lab-control commands (stop/start, load) | <EMPTY> |
| Observer | <EMPTY> | watches probes/metrics, calls out deviations; NEVER touches controls | <EMPTY> |
| Evidence keeper | <EMPTY> | fills the evidence JSON cells and this runbook from the run only | <EMPTY> |
| Witness (optional) | <EMPTY> | independent observer for sign-off runs | <EMPTY> |

## 4. Registered thresholds (X-8 — written BEFORE the run)
| Threshold | Value (registered pre-run) | Basis |
|---|---|---|
| outage holds / budgets / concurrency / durations | <EMPTY> | <EMPTY: CLI arguments of the experiment scripts, record 21 §3.4> |

## 5. Abort criteria (ANY one aborts; evidence kept verbatim)
- <EMPTY — copied verbatim from the experiment script's abort_criteria
  block; the director may add criteria, never remove one>

## 6. Timeline (filled DURING the run, from clocks)
| Clock (local) | Step | Who | Observation |
|---|---|---|---|
| <EMPTY> | pre-registration printed | <EMPTY> | <EMPTY> |
| <EMPTY> | keyboard gate (GO) | <EMPTY> | <EMPTY> |
| <EMPTY> | fault injected | <EMPTY> | <EMPTY> |
| <EMPTY> | abort called? (yes/no + criterion) | <EMPTY> | <EMPTY> |
| <EMPTY> | recovery confirmed | <EMPTY> | <EMPTY> |
| <EMPTY> | run closed | <EMPTY> | <EMPTY> |

## 7. Evidence capture
- Evidence JSON files (the experiment scripts' output): <EMPTY — file paths>
- Metrics screenshots / exposition snapshots: <EMPTY>
- Transition-journal excerpts (C-PG-1): <EMPTY>
- Outbox delta readings (C-NATS-1): <EMPTY>

## 8. Post-run review (the game day's actual product)
| Finding | Experiment | Disposition (fix / accept / re-run / escalate) |
|---|---|---|
| <EMPTY> | <EMPTY> | <EMPTY> |
- Zero findings is recorded as a TEST OUTCOME, never as a guarantee
  (§13.3 semantics; conventions §5).
- Any measured value that misses a registered threshold is a FINDING, a
  threshold is never re-defined to fit a result (the P9.24 §4 judgement
  rule, inherited).

## 9. Follow-ups
| Action | Owner | Due | Status |
|---|---|---|---|
| <EMPTY> | <EMPTY> | <EMPTY> | <EMPTY: OPEN | DONE — never pre-filled>
```

## 2. The quarterly cadence (PROPOSAL — a human decision to adopt)

| Quarter | Session | Experiments (from the P12.07 inventory) | Cadence rationale |
|---|---|---|---|
| Q1 | GD-Q1 | C-WORKER-1 + C-POOL-1 | the two lowest-blast-radius experiments first: no datastore fault, pure supervision + limiter contracts |
| Q2 | GD-Q2 | C-NATS-1 | adds the message-plane fault once the team has run two sessions |
| Q3 | GD-Q3 | C-PG-1 | the transactional-store fault, gated on the P12.01–P12.03 backup/restore artifacts being exercised at least once (a restore drill, P12.03) before the store itself is faulted |
| Q4 | GD-Q4 | a repeat of one experiment chosen by the Q1–Q3 findings + the DR restore-into-isolation rehearsal (`runbooks/dr_site_loss.md`, P12.11) | the annual DR rehearsal rides the Q4 game day; the DR runbook's own evidence template governs |

Cadence rules:

1. **Human-gated end to end** [S-82]: scheduling, approving, staffing, and running a game day are operator acts; the proposal above binds nothing until adopted.
2. **Never two datastore faults in one session** — one experiment per session until every experiment has run at least once; the blast radius stays legible.
3. **A game day does not run on an unfixed finding**: a failed experiment's follow-ups (template §9) close before the same experiment re-runs.
4. **The template is the record**: a game day whose §1–§7 cells were not filled from the run is a fabrication defect (the P9.24 §8 rule, inherited).
5. **No live-traffic game days exist in the 0.x posture**: every session targets the isolated lab; the pilot posture inherits this program only through the release ladder's own gates (AGENTS.md §4).

## 3. Honesty boundary

- [S-82] is cited for the game-day mechanics (roles, abort criteria,
  evidence capture, cadence); it makes no claim about FlexCore.
- No game day is scheduled; no date cell above is filled; no role is
  named; no threshold beyond the scripts' registered CLI defaults exists.
- The cadence table is a RECOMMENDATION pending operator adoption —
  adoption is a human decision recorded wherever the operator records it,
  never here.
