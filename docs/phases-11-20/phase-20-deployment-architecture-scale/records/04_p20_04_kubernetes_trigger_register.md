<!-- P20.04 — Kubernetes migration trigger register + decision record
     P20-E-1. THE single register of K8s triggers. P20-E-1 is registered
     OPEN with DEFAULT (a) stay on Compose until a trigger fires
     [S-72][S-73]. Agents never close E-decisions (conventions §4). -->
# P20.04 — Kubernetes Migration Trigger Register (the single register) + Decision Record P20-E-1 (OPEN, DEFAULT (a))

| Field | Value |
|---|---|
| Subtask | P20.04 — Kubernetes migration trigger register + decision record (`ledger_phase_20.md` §2, row P20.04) |
| Wave / agent | Phase 20 wave 1, Agent A |
| Status | **DECISION RECORD — P20-E-1 OPEN, DEFAULT (a) recorded, trigger register live. ALL TRIGGERS UNFIRED.** No Kubernetes cluster exists, none is provisioned, no migration work has started. |
| Sources | [S-72] (when not to use Kubernetes — the Compose-correct-until-triggers verdict for this single-host shape); [S-73] (Argo CD vs Flux — the GitOps posture that becomes relevant only after a migration); gap analysis §5 Tier 3 row 1 and §8 revisit row; `context.md` §1 table and constraint 1; plan §6 P20-E-1 wording (consumed verbatim-in-substance) |
| Files touched | this record only (the register lives HERE and is the single place the K8s triggers are enumerated) |

## 1. Honest status — the plain statement

The Tier 3 engineering verdict is explicit: **do NOT migrate to Kubernetes
now** — Compose is correct for this single-host shape until a registered
trigger fires [S-72]. This record therefore delivers the decision
machinery, not the migration: the single trigger register (§2) with each
trigger's observable condition, and the P20-E-1 decision entry (§3),
registered OPEN with DEFAULT (a). A fired trigger is a RECORDED EVENT — the
owner records it with evidence; an agent inference that a trigger "looks
fired" is a consistency-audit failure (context constraint 1). No trigger
has fired as of 2026-11-17.

## 2. The trigger register (single definition)

Consumers (P20.05 Helm design, P20.06 GitOps design, P20.11 K8s-era
rollout shape, Agent B's P20.09) cite THIS section; they never enumerate a
second trigger list. The four triggers and their observable conditions:

| # | Trigger | Observable condition (all parts, evidenced) | Status | Evidence pointer |
|---|---|---|---|---|
| T-1 | Multi-host scheduling demand | The deployment needs service instances across ≥ 2 hosts where the Compose single-host placement is a recorded availability or capacity constraint (e.g. a host-loss window exceeds the operator's accepted availability exposure), evidenced by an operator record | **UNFIRED** | `<EMPTY>` |
| T-2 | Second region / DR site | The owner records a second-region requirement per P20-E-2 (customer, regulator, or contract evidence) — the P12-E-3 revisit condition, linked not restated | **UNFIRED** | `<EMPTY>` |
| T-3 | Automated canary demand | The operator records that control-plane releases need automated canary analysis (metric-gated comparison of a small new-version slice), which demands routing/metric plumbing only meaningful at multi-instance scale | **UNFIRED** | `<EMPTY>` |
| T-4 | Team growth | The owner records that the number of engineers deploying to the stack has grown past the point where Compose's single-operator workflow is a recorded bottleneck (onboarding, parallel release trains) | **UNFIRED** | `<EMPTY>` |

Register rules:

1. Each trigger's evidence pointer is filled by the OWNER with a recorded
   event (decision record, contract, incident) — never by an agent.
2. Any ONE firing makes the migration DEPLOYABLE, not self-executing:
   the migration itself is an implementing wave gated on P20-E-1 closure
   (the owner closes it with the evidence).
3. A trigger that never fires leaves the downstream designs (P20.05,
   P20.06, P20.11's K8s shape) as decision-ready records, honestly INERT —
   recorded states, not backlog promises.

## 3. Decision record P20-E-1

| Field | Value |
|---|---|
| Decision ID | **P20-E-1** (registered in plan §6; OPEN) |
| Question | When does the Kubernetes migration actually happen, if ever? |
| Options | (a) stay on Compose until a registered trigger fires, then migrate; (b) migrate preemptively (REJECTED — the Tier 3 verdict is explicit [S-72]); (c) never migrate regardless of triggers (REJECTED — dishonest under (a) conditions) |
| **DEFAULT (RECOMMENDATION)** | **(a)** — the trigger register (§2) is the decision machinery; each trigger names its observable condition so the future migration is a decision, not a drift [S-72][S-73] |
| What would close it | The owner recording that a named trigger has fired (with the evidence), or recording that none will |
| Closer | human owner |
| Status | **OPEN** |

## 4. What this record does NOT establish

1. No cluster, no chart, no GitOps operator: nothing is provisioned,
   rendered, or installed; the downstream designs are written specs
   (P20.05, P20.06) and are INERT until closure.
2. No trigger has fired; the evidence pointers are EMPTY.
3. No migration plan of record: the designs describe a FUTURE shape; at
   trigger time they are re-verified against primary sources before
   implementation (plan §7 R-P20-2 — version pins and assumptions re-checked,
   stale rows re-recorded).
4. No dispatch-authority or release-ladder change: the migration question
   is deployment shape only; the lifecycle, authorization, tenant
   isolation, and audit gates hold in any topology.

## 5. Reviewer-verification procedure (human)

1. Confirm §2 is the only trigger register in the repository and all four
   triggers read UNFIRED with EMPTY evidence pointers.
2. Confirm §3 records P20-E-1 OPEN with DEFAULT (a) and the human closer.
3. Confirm plan §6's P20-E-1 row still reads OPEN (agents never close it).
4. Confirm no K8s manifest, chart file, or cluster artifact exists outside
   the WRITTEN-SPEC tree (`deploy/k8s/`, marked never-rendered).

## 6. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) When a trigger fires: record the evidence (decision record /
#     contract / incident reference) in §2's evidence pointer, close
#     P20-E-1 as the owner, and gate the implementing wave on that
#     closure — re-verifying the P20.05/P20.06 designs against primary
#     sources first (R-P20-2).
```

## 7. Findings and assumptions

| # | Item | Note |
|---|---|---|
| A-P20.04-1 | The four-trigger enumeration matches the gap analysis Tier 3 verdict and the context table | [S-72][S-73]; consistent across plan §6 and context.md §1 |
| F-P20.04-1 | All triggers UNFIRED; P20-E-1 OPEN | The honest steady state until the owner records otherwise |

Status: **REGISTER LIVE — ALL TRIGGERS UNFIRED, P20-E-1 OPEN with DEFAULT (a)**;
the DEFAULT is a recommendation the owner may adopt or reject; nothing
deploys on it either way.
