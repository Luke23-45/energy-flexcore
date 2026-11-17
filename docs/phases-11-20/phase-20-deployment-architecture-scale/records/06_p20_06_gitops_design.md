<!-- P20.06 — GitOps design: Argo CD vs Flux recommendation (DEFAULT:
     Argo CD for UI-driven onboarding [S-73]); environment promotion flow
     (dev -> staging -> prod, PR-based, digest-pinned). The design is
     INERT until the P20.04 trigger fires — recorded as such. -->
# P20.06 — GitOps Design (Argo CD DEFAULT — INERT until the P20.04 trigger fires)

| Field | Value |
|---|---|
| Subtask | P20.06 — GitOps design (`ledger_phase_20.md` §2, row P20.06) |
| Wave / agent | Phase 20 wave 1, Agent A |
| Status | **DESIGN RECORD — WRITTEN, INERT.** No GitOps operator exists, none is installed, no repository layout for it is created beyond the design spec (`deploy/k8s/GITOPS.md`). The design activates only when the P20.04 register's trigger fires and P20-E-1 closes. |
| Sources | [S-73] (Argo CD vs Flux — AWS Prescriptive Guidance comparison; the source of the UI-driven-onboarding basis for the DEFAULT); P20.04 register (records/04 §2 — the single trigger register, all UNFIRED); P20.05 chart design (records/05 and `deploy/k8s/README.md` — the artifacts the promotion flow would promote); P20.14 runbook v2 linkage (digest-pinned artifact promotion is the shared property — consumed there, not duplicated here) |
| Files touched | `deploy/k8s/GITOPS.md` (the design spec), this record |

## 1. Honest status — the plain statement

GitOps is a CLUSTER-ERA mechanism: it reconciles a cluster to a declared
desired state. No cluster exists, no trigger has fired, and the P20.04
DEFAULT keeps Compose until one does. This record therefore writes the
decision-ready design — the operator recommendation (§2), the promotion
flow (§3) — and records that it is INERT: nothing consumes it, nothing
installs it, and a design that never activates leaves no debt because it
was never a commitment.

## 2. Operator recommendation — Argo CD (DEFAULT)

| Criterion | Argo CD | Flux | Basis |
|---|---|---|---|
| UI-driven onboarding | First-class web UI (application topology, sync status, diff view) usable by operators new to GitOps | Primarily CLI/Git-centric; UI via separate ecosystem components | The recorded basis of the DEFAULT [S-73] |
| Reconciliation model | Application-level CRs (`Application`/`ApplicationSet`) with per-app health | Source + Kustomize/Helm controller composition | [S-73] |
| Fit to the P20.05 chart | Renders the umbrella chart directly with per-environment values | Renders Helm charts via the Helm controller | Either fits; the DEFAULT favors the onboarding surface |

**DEFAULT (RECOMMENDATION): Argo CD** for UI-driven onboarding [S-73] —
recorded as a recommendation the owner adopts or rejects at trigger time;
never an installation claim. No vendor performance, security-certification,
or superiority claim beyond the recorded onboarding basis is made
anywhere; other comparison dimensions are re-verified against primary
sources at trigger time (R-P20-2).

## 3. Environment promotion flow (dev → staging → prod)

| Property | Design |
|---|---|
| Promotion mechanism | PR-based: promoting an environment = merging a change to that environment's overlay branch/directory; no push-to-prod outside a reviewed PR |
| Artifact identity | Digest-pinned: the image reference in an environment overlay is an immutable digest, never a floating tag; promotion moves the SAME digest forward — no per-environment rebuilds (the P20.14 shared property) |
| Environment layout | `deploy/k8s/GITOPS.md` §4: a GitOps repository layout with one values file per environment under the P20.05 values-schema contract |
| Sync policy | dev: auto-sync; staging: auto-sync after review; prod: manual sync (the operator's button — the UI the DEFAULT favors) — recorded as the recommendation, tunable at trigger time |
| Secret material | Never in the GitOps repository; secret REFERENCES only (P11.16 posture consumed) |
| Drift handling | Operator-visible diff (the onboarding surface's purpose); drift on prod is an event for the runbook, never an auto-revert decision recorded here |

## 4. Inert-until-trigger record

| Condition | State |
|---|---|
| P20.04 register | ALL TRIGGERS UNFIRED (records/04 §2) |
| P20-E-1 | OPEN, DEFAULT (a) stay on Compose |
| This design's status | **INERT** — no repository layout created, no operator installed, no CI wiring referencing it; activation is gated on P20-E-1 closure and a re-verification pass against primary sources (R-P20-2) |

## 5. What this record does NOT establish

1. No GitOps operator exists or is installed; no Argo CD/Flux version is
   pinned as usable.
2. No repository layout exists beyond the spec; no PR, no digest, no
   promotion has ever flowed.
3. No claim that Argo CD is superior beyond the onboarding criterion
   cited from [S-73]; the comparison table records shape, not benchmarks.
4. No dispatch-authority change: the promotion flow promotes the SAME
   digest through environments with unchanged lifecycle/authorization/
   tenant-isolation/audit gates.

## 6. Reviewer-verification procedure (human)

1. Confirm §4's inert-until-trigger table matches records/04 §2 (all
   UNFIRED) and plan §6 P20-E-1 (OPEN).
2. Confirm `deploy/k8s/GITOPS.md` is a spec carrying the
   written-never-installed header.
3. Confirm the DEFAULT is labeled a recommendation with [S-73] as its
   basis and the human owner as the closer.
4. Confirm no CI/workflow file references a GitOps operator.

## 7. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to these files):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) Only AFTER P20-E-1 closes: re-verify the [S-73] comparison and tool
#     versions against primary sources, adopt or reject the DEFAULT as
#     the owner, and create the GitOps repository layout in an
#     implementing wave — never in this program.
```

## 8. Findings and assumptions

| # | Item | Note |
|---|---|---|
| A-P20.06-1 | The [S-73] comparison reflects the source's recorded framing at the research cut-off | Re-verified at trigger time; the source list is the gap analysis §11 register |
| F-P20.06-1 | The design has never activated; no operator has ever reconciled anything | Carried by design |

Status: **WRITTEN, INERT** — a GitOps design cannot reconcile a cluster
that does not exist; activation is the owner's trigger-recording act and
has not happened.
