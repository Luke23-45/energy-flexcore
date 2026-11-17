# P20.05 — FlexCore Helm Chart DESIGN SPEC — WRITTEN, NEVER RENDERED

Status: **WRITTEN SPEC — NEVER RENDERED, INERT UNTIL TRIGGER**
(decision records:
`docs/phases-11-20/phase-20-deployment-architecture-scale/records/05_p20_05_helm_chart_design.md`
[P20.05] and
`records/04_p20_04_kubernetes_trigger_register.md` [the trigger register —
ALL TRIGGERS UNFIRED as of 2026-11-17]).

This file DESCRIBES the chart content. It is not a chart: no `Chart.yaml`,
no template, no rendered manifest exists anywhere, and none may be created
or rendered by this program (`helm template` / `helm install` / `kubectl` /
`docker` are banned; conventions §1). The chart is INERT until a P20.04
trigger fires and P20-E-1 closes; at trigger time every version pin and
assumption below is RE-VERIFIED against primary sources before
implementation (plan §7 R-P20-2).

## 1. What this chart packages (1:1 with the Compose shape)

Each chart component maps 1:1 to a service in
`deploy/compose/docker-compose.yml`, so the P20.13 environment-parity
matrix can compare the two shapes service by service:

| Chart component | Compose counterpart | K8s workload kind (design) | Stateful? |
|---|---|---|---|
| `control-plane` | `control-plane` (the Axum API) | Deployment, replicas ≥ 1 | No — stateless; state lives in Postgres/NATS |
| `worker` | `worker` (the NATS consumer) | Deployment, replicas ≥ 1 | No — stateless; durable JetStream consumers carry state in NATS |
| `console` | (the operator console's static TS build; not a Compose service today — it is served by the control-plane surface) | Deployment running a static file server, replicas ≥ 1 | No |
| `postgres` | `postgres:18.6` | **FLAGGED STATEFUL** — see §3 | Yes |
| `nats` | `nats:2.15.2` (JetStream file store) | **FLAGGED STATEFUL** — see §3 | Yes |
| `otel-collector` | `otel/opentelemetry-collector-contrib` (the P11.06 gateway) | Deployment, replicas 1 at this scale | No |

## 2. Chart layout (described, not created)

```text
charts/flexcore/
  Chart.yaml                 name flexcore; the version pins are RE-VERIFIED
                             AT TRIGGER TIME (R-P20-2) — none is recorded here
  values.yaml                defaults = the dev environment
  values.schema.json         the schema contract of §4
  values-dev.yaml            dev overlay (§5)
  values-staging.yaml        staging overlay (§5)
  values-prod.yaml           prod overlay (§5)
  templates/
    _helpers.tpl             names, labels, secret-reference helpers
    control-plane-deployment.yaml
    control-plane-service.yaml
    worker-deployment.yaml
    console-deployment.yaml
    console-service.yaml
    otel-collector-deployment.yaml
    otel-collector-service.yaml
    configmaps.yaml          non-secret configuration (the FLEXCORE_* env
                             surface as ConfigMap keys)
    secrets.yaml             REFERENCES ONLY — see §6
    NOTES.txt                post-install readiness notes (/ready gating)
```

Deliberately ABSENT from the layout: `postgres-*.yaml` and `nats-*.yaml`
templates — the stateful pieces are NOT chart templates by design (§3).

## 3. The stateful pieces (flagged)

| Piece | Design | Why not a plain Deployment + PVC |
|---|---|---|
| Postgres | A Postgres OPERATOR manages it (CloudNativePG-CLASS — a class reference, not a vendor commitment; vendor selection is an owner E-decision). The chart consumes the operator's CRs, it does not template the database workload | Stable identity + storage, failover, backup orchestration, and rolling upgrades are operational knowledge that belongs in a controller, not in a generic template |
| NATS JetStream | A StatefulSet with a persistent VOLUME for the JetStream file store (`-sd /data`, the Compose flag), stable pod identity, one replica at the single-node trigger-time scale — a JetStream cluster is a POST-migration scale decision with its own record | The file store MUST NOT live on ephemeral pod storage: stream state survives pod rescheduling only if the volume does |

Both pieces inherit the backup/DR posture from Phase 12 (P12.01–P12.03
consumed by citation — the P20.12 backups-at-scale record is the scale
layer). Nothing in this spec deploys them.

## 4. The values schema contract (described, not created)

`values.schema.json` (when authored at implementation time) enforces:

| Key group | Keys (design) | Rules |
|---|---|---|
| `global` | `environment` (dev/staging/prod), `imageRegistry`, `imagePullPolicy` | digest-pinned image references (see §7); floating tags REJECTED by the schema |
| `controlPlane` | `image.digest`, `replicas`, `resources`, `env.*`, `service.port` | replicas ≥ 1; probes wired to `/health` (liveness) and `/ready` (readiness) — the P1.22 distinct-probe contract, never merged |
| `worker` | `image.digest`, `replicas`, `resources`, `env.*` | same probe contract per `apps/worker/src/probes.rs` |
| `console` | `image.digest`, `replicas`, `service.port` | static content only |
| `nats` | `external` (bool — use a managed/externally-run NATS instead of in-chart StatefulSet), `storage.size`, `storage.className`, `jetstream.fileStore` | `external: true` disables the in-chart StatefulSet and requires a `nats.externalUrl` |
| `postgres` | `external` (bool — use the operator's CR instead), `operatorCRRef` | same external pattern; the chart never templates the database itself |
| `secrets` | `postgresAppPassword.secretRef`, `natsCreds.secretRef`, … | REFERENCES ONLY — see §6 |
| `autoscaling` | `enabled` (default false), `minReplicas`, `maxReplicas`, KEDA `natsLagThreshold` | see §8 — post-migration mechanics, off by default |
| `observability` | `otelEndpoint` (the P11.06 gateway address) | matches the Compose env surface |

Schema rule: every env value the Compose file carries
(`FLEXCORE_NATS_URL`, `FLEXCORE_BIND_ADDR`, `FLEXCORE_LOG_LEVEL`,
`FLEXCORE_OTEL_*`) has a typed schema key; adding an env knob without a
schema entry is a spec amendment, never a template-local escape hatch.

## 5. Per-environment overlays

Environments differ in RECORDED, DELIBERATE values only (the P20.13 parity
property); what must stay identical stays identical:

| Overlay | Differs in | Must stay identical |
|---|---|---|
| `values-dev.yaml` (defaults) | 1 replica per stateless service; small storage; debug-friendly log level; dev secret refs | the image digests' build provenance, probe contracts, env knob NAMES, resource-request structure |
| `values-staging.yaml` | prod-shaped replica counts at reduced storage; prod-like secret refs | everything structural |
| `values-prod.yaml` | full replica counts, real storage classes, production secret refs, `autoscaling` as the owner enables | everything structural |

Rule: a difference between environments that is NOT in an overlay file is
a parity defect (P20.13's matrix is the audit surface).

## 6. Secrets (the P11.16 posture consumed)

No secret MATERIAL ever appears as a Helm value, a ConfigMap entry, or a
template literal. The chart references Kubernetes Secrets by name
(`secretRef`), and those Secrets are provisioned OUTSIDE the chart — by
the secrets manager's integration (the recorded Vault DEFAULT,
P11.16) or the operator's deployment procedure. The Compose
file-per-host floor has no chart analogue: at chart time the floor is
already superseded by the secrets-manager posture.

## 7. Image identity

Image references are **digest-pinned** in values (the P20.14
artifact-promotion property, shared): an environment promotes the SAME
digest forward, never a rebuilt tag. The schema rejects mutable tags.
The images themselves are built by the existing Dockerfiles
(`deploy/compose/control-plane.Dockerfile`, `worker.Dockerfile`) — the
chart changes packaging, not the build.

## 8. HPA / KEDA — post-migration mechanics, named and claimed nowhere

- **HPA** scales the stateless Deployments (control-plane, worker) on the
  registered SLI metrics (`flexcore_api_in_flight_requests`,
  request-rate) — meaningful only once a metrics pipeline and a cluster
  exist.
- **KEDA** scales the worker on NATS JetStream consumer lag — the
  event-driven shape the worker's durable-consumer design points at.
- Both default OFF in the schema (`autoscaling.enabled: false`). They are
  designed-in POST-migration mechanics; this spec claims no scaling
  behavior, no measured capacity, and no autoscaling figure of any kind.

## 9. The P10.21 linkage

`docs/operations/deployment-upgrade-model.md` §1 references the chart as
the P10.21 publication deliverable. That gate stays HUMAN and unchanged:
this spec designs content; authoring `Chart.yaml` from it is an
implementing-wave act AFTER P20-E-1 closes, and publication remains the
human gate exactly as recorded in P10.21.

## 10. What this spec does NOT establish

1. No chart exists and none has been rendered, linted, or installed.
2. No cluster exists; no trigger has fired (P20.04 register — all
   UNFIRED); the spec is INERT.
3. No version pin is claimed: every tool/API version is re-verified at
   trigger time against primary sources (R-P20-2).
4. No autoscaling, capacity, or performance claim of any kind.
5. No dispatch-authority change: the chart packages the SAME services
   with unchanged lifecycle/authorization/tenant-isolation/audit gates.
