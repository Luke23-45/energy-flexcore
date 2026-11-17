# P20.06 — GitOps DESIGN SPEC (Argo CD DEFAULT) — WRITTEN, NEVER INSTALLED, INERT

Status: **WRITTEN SPEC — NEVER INSTALLED, INERT UNTIL TRIGGER**
(decision record:
`docs/phases-11-20/phase-20-deployment-architecture-scale/records/06_p20_06_gitops_design.md`;
trigger register: `records/04_p20_04_kubernetes_trigger_register.md` —
ALL TRIGGERS UNFIRED as of 2026-11-17; P20-E-1 OPEN, DEFAULT (a) stay on
Compose).

GitOps reconciles a CLUSTER to a declared desired state. No cluster
exists; no operator exists; nothing below has ever run or reconciled
anything. This spec is decision-ready content for the day the P20.04
trigger fires and P20-E-1 closes — at that day, every tool comparison and
version assumption here is RE-VERIFIED against primary sources (plan §7
R-P20-2) before any implementation.

## 1. Operator recommendation — Argo CD (DEFAULT, a recommendation)

Basis: [S-73] (Argo CD vs Flux, AWS Prescriptive Guidance) — the recorded
DEFAULT favors Argo CD for UI-driven onboarding: the first-class web UI
(application topology, sync status, diff view) lowers the onboarding cost
for operators new to GitOps, which is the dominant criterion at the scale
a first migration implies.

| Dimension | Argo CD (the DEFAULT) | Flux | Recorded basis |
|---|---|---|---|
| Onboarding surface | Web UI first-class | CLI/Git-centric | [S-73]; the DEFAULT's stated basis |
| Reconciliation unit | Application CRs (`Application`/`ApplicationSet`) with per-app health | Source + Kustomize/Helm controllers | [S-73]; shape comparison only |
| Fit to the P20.05 chart | Renders the umbrella chart with per-environment values files | Renders Helm charts via the Helm controller | Either fits the §3 layout |

No vendor performance, superiority-beyond-onboarding, or
security-certification claim is made anywhere. The owner adopts or rejects
this DEFAULT at trigger time; closer: human owner (conventions §4).

## 2. Environment promotion flow (dev → staging → prod)

| Property | Design |
|---|---|
| Mechanism | PR-based: promoting an environment = merging a change into that environment's overlay; no push-to-prod outside a reviewed PR |
| Artifact identity | Digest-pinned image references in each environment's values (the P20.05 §7 contract, shared with P20.14's runbook): promotion moves the SAME digest forward; no per-environment rebuilds |
| Sync policy (recommendation, tunable at trigger time) | dev: auto-sync; staging: auto-sync after review; prod: manual sync (the operator's button — the UI surface the DEFAULT favors) |
| Secret material | Never in the GitOps repository — secret REFERENCES only (the P20.05 §6 posture consumed); the secrets manager provisions the actual Secrets |
| Drift | Operator-visible diff; prod drift is an event for the operational runbook, never an auto-revert rule recorded here |

## 3. Repository layout (described, not created)

```text
gitops/                      a SEPARATE repository, created only after
  overlays/                  P20-E-1 closes (described here for
    dev/                     decision-readiness)
      kustomization.yaml     or the per-env values file, per the chosen
      values-dev.yaml        rendering mode (chart-values vs overlay —
    staging/                 decided at trigger time with the chart)
      ...
    prod/
      ...
  applications/              the operator's Application definitions
    flexcore-dev.yaml
    flexcore-staging.yaml
    flexcore-prod.yaml
```

Deliberately NOT created by this program: the `gitops/` tree, any
`Application` manifest, any operator install. The layout above is the
described starting point, re-verified at trigger time.

## 4. Inert-until-trigger record

| Condition | State |
|---|---|
| P20.04 register | ALL TRIGGERS UNFIRED (records/04 §2) |
| P20-E-1 | OPEN, DEFAULT (a) stay on Compose |
| This spec | **INERT** — no repository created, no operator installed, no CI wiring references it; activation is gated on P20-E-1 closure + a re-verification pass |

## 5. What this spec does NOT establish

1. No operator, cluster, repository, or reconciliation exists.
2. No Argo CD/Flux version is pinned as usable; the comparison records
   shape, not benchmarks.
3. No promotion has ever flowed; no digest has ever been promoted.
4. No dispatch-authority change: the flow promotes the SAME digest with
   unchanged lifecycle/authorization/tenant-isolation/audit gates.
