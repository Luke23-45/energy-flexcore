<!-- P20.05 — Helm chart design: WRITTEN SPEC, NEVER RENDERED. helm
     template / helm install / docker / kubectl are BANNED in this program
     (hard bans). The chart content design lives in deploy/k8s/README.md;
     this record is the decision record around it and records the P10.21
     linkage. The chart is INERT until a P20.04 trigger fires. -->
# P20.05 — Helm Chart Design (WRITTEN SPEC — never rendered; helm template banned)

| Field | Value |
|---|---|
| Subtask | P20.05 — Helm chart design (`ledger_phase_20.md` §2, row P20.05) |
| Wave / agent | Phase 20 wave 1, Agent A |
| Status | **WRITTEN SPEC, NEVER RENDERED.** The chart layout, values schema, and overlay design exist as spec documents (`deploy/k8s/`); no `Chart.yaml`, no template file, no rendered manifest exists; `helm template` / `helm install` / `kubectl` / `docker` are banned in this program and none has run. The chart is INERT until a P20.04 trigger fires and P20-E-1 closes. |
| Sources | [S-73] (the GitOps-era packaging context); `docs/operations/deployment-upgrade-model.md` §1 (the P10.21-pending chart publication reference — consumed); `deploy/compose/docker-compose.yml` (the service set the chart packages: postgres 18.6, nats 2.15.2 JetStream, control-plane, worker, otel-collector — plus the console the P5 operator surface defines); P20.04 register (records/04 — the trigger the chart is gated on); P11.16 (secret-injection posture the values schema must respect) |
| Files touched | `deploy/k8s/README.md` (the chart design spec), this record |

## 1. Honest status — the plain statement

The chart is a **described publication deliverable that does not exist**:
`docs/operations/deployment-upgrade-model.md` §1 references the P10.21
chart publication, which is pending and stays human-gated. This subtask
designs the chart's CONTENT — layout, values schema, per-environment
overlays, stateful-piece flags, and post-migration autoscaling mechanics —
as written spec files under `deploy/k8s/`, so the day a trigger fires the
owner has a decision-ready starting point that has still never touched a
renderer. Nothing in `deploy/k8s/` is consumed by any current deployment;
the Compose stack is the only (never-brought-up) shape that exists.

## 2. The chart design (summary — full spec in `deploy/k8s/README.md`)

| Element | Design | Status |
|---|---|---|
| Chart layout | One umbrella chart, `charts/flexcore/`, with per-component templates: `control-plane` (Deployment), `worker` (Deployment), `console` (static TS build served behind the control-plane surface), `postgres` (FLAGGED STATEFUL — see row below), `nats` (FLAGGED STATEFUL — JetStream file store), `otel-collector` (gateway Deployment) | WRITTEN SPEC; no file rendered |
| Stateful pieces flagged | **postgres** and **NATS JetStream file storage** are StatefulSet / operator concerns (CloudNativePG-class for postgres; a NATS JetStream StatefulSet with a persistent file store): stable identity + storage, NOT a plain Deployment with a PVC afterthought | WRITTEN SPEC |
| Values schema | `values.schema.json` shape specified (the spec defines required keys, types, and defaults-by-environment); secrets NEVER as values — secret references only, per the P11.16 posture (secret material from a secrets manager; the Compose file-per-host floor has no chart analogue) | WRITTEN SPEC |
| Per-environment overlays | Helm VALUES-layer environments (`values-dev.yaml`, `values-staging.yaml`, `values-prod.yaml`) rather than Kustomize overlays — the chart stays the single packaging unit, environments differ in recorded, deliberate values (feeds P20.13's parity matrix) | WRITTEN SPEC |
| HPA / KEDA | Named as POST-MIGRATION mechanics: HPA on the stateless Deployments (control-plane, worker) and KEDA for event-driven worker scaling on NATS lag are designed-in annotations/values, but they are meaningless before a cluster exists and are claimed nowhere | WRITTEN SPEC |
| Compose parity rule | Every chart component maps 1:1 to a Compose service so P20.13's parity matrix can compare the two shapes service by service | WRITTEN SPEC |

## 3. P10.21 linkage (recorded, not dissolved)

| P10.21 element | How this design treats it |
|---|---|
| The chart is a PUBLICATION deliverable (publication gate HUMAN) | P20.05 designs content only; the P10.21 publication gate stays exactly as recorded — writing the chart from this spec is an implementing-wave act AFTER P20-E-1 closes, and its publication remains the human gate |
| The upgrade model references the chart | `docs/operations/deployment-upgrade-model.md` is untouched by this record; the linkage is recorded here so the audit can trace reference → design → gate |

## 4. What this record does NOT establish

1. No chart exists: no `Chart.yaml`, no templates, no `values.schema.json`
   file — only the spec documents describing them.
2. Nothing is rendered, linted (`helm lint` included), or installed; the
   ban list (helm/docker/kubectl) ran zero commands.
3. No cluster shape is claimed working: every design row is a
   recommendation to be re-verified against primary sources at trigger
   time (plan §7 R-P20-2 — tool versions and chart APIs drift).
4. INERT until trigger: no subtask consumes the chart before P20-E-1
   closes; the design is decision-ready, not a commitment.
5. No dispatch-authority change: a chart packages the SAME services with
   the SAME lifecycle/authorization/tenant-isolation/audit gates; no
   deployment topology bypasses them.

## 5. Reviewer-verification procedure (human)

1. Confirm `deploy/k8s/README.md` is a SPEC document carrying the
   written-never-rendered header and contains no rendered manifest
   output.
2. Confirm no Helm chart files exist anywhere (search: `Chart.yaml`,
   `*.tpl`, `values.schema.json` as an actual file).
3. Confirm the stateful-piece flags (§2) name postgres and NATS JetStream
   as StatefulSet/operator concerns.
4. Confirm the P10.21 publication gate is unchanged.
5. Confirm the trigger gate: P20.04 §2 all UNFIRED; this design marked
   INERT.

## 6. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to these files):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) Only AFTER a P20.04 trigger fires and P20-E-1 closes: re-verify the
#     spec's version pins and assumptions against primary sources
#     (R-P20-2), author the chart from deploy/k8s/README.md in an
#     implementing wave, and run the human publication gate (P10.21).
#     helm template/lint/install are the operator's acts, never this
#     program's.
```

## 7. Findings and assumptions

| # | Item | Note |
|---|---|---|
| A-P20.05-1 | Helm/Kustomize tooling shapes drift; the spec is a starting point that must be re-verified at trigger time | plan §7 R-P20-2; recorded, not hidden |
| A-P20.05-2 | CloudNativePG-class operator naming is a CLASS reference (stateful-workload operator pattern), not a vendor commitment | Vendor selection is an owner E-decision |
| F-P20.05-1 | The chart has never been rendered and no cluster has validated the design | Carried by design — the implementing wave is the generator |

Status: **WRITTEN SPEC — NEVER RENDERED, INERT UNTIL TRIGGER**; the chart
cannot package a service by being specified; the implementing wave and the
P10.21 human publication gate are the gates and have not happened and are
not scheduled.
