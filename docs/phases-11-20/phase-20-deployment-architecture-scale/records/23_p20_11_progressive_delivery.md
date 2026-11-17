<!-- P20.11 — Blue-green/canary deployment design for the CONTROL PLANE
     SOFTWARE. Design record only: no rollout was executed, no stack was
     switched, no traffic was moved, no Argo Rollouts-class tool was
     installed, and no cluster exists. The K8s-era shape is INERT until the
     P20.04 trigger fires (Agent A's register — consumed by expected path).
     The THREE-WAY scope distinction is mandatory and carried in §2 with
     the kernel ROLLOUT_STATEMENT quoted VERBATIM and P18.06 cited as
     DISTINCT. -->
# P20.11 — Control-Plane Progressive Delivery Design (Compose-era switch + K8s-era shape, INERT until the P20.04 trigger; three-way scope distinction; nothing executed)

| Field | Value |
|---|---|
| Subtask | P20.11 — Blue-green/canary deployment design (control plane) (`ledger_phase_20.md` §P20.11) |
| Wave / agent | Phase 20 wave 1, Agent B |
| Status | **DESIGN RECORD — WRITTEN-UNRUN.** No blue-green switch has ever been thrown, no canary has ever run, no traffic has ever been moved between stacks, no Argo Rollouts-class tool exists in this repository, and no cluster exists. The K8s-era shape is INERT until Agent A's P20.04 trigger register records a fired trigger. |
| Scope (one sentence) | This record designs progressive delivery for the CONTROL-PLANE SOFTWARE — the rollout of a new version of the control plane / worker / console DEPLOYMENT across environments. It is NOT device-firmware rollout and NOT plugin-rollout policy (§2 — three mechanisms, three arguments). |
| Consumed by citation (never restated) | `apps/control-plane/src/probes.rs` (P1.22/P11.21 — `/ready` readiness with named checks and the `begin_shutdown` drain gate); P18.06 device-cohort record (`docs/phases-11-20/phase-18-edge-fleet-lifecycle-ota/records/06_p18_06_rollout_orchestrator_scope_distinction.md` — cited as DISTINCT); `crates/flexcore-kernel/src/revocation.rs` P7.38 `ROLLOUT_STATEMENT` (read-only, quoted verbatim in §2); P20.04 trigger register + P20.05 chart layout (Agent A — consumed by expected path; reconciliation note in the close-out record); P10.12 §3.1 (the rolling-upgrade health-gating rule consumed by the Compose-era shape) |
| Sources | [S-73] (Argo CD/Flux-class GitOps tooling context — tool-class names only, no conformance or capability claim); `deploy/compose/docker-compose.yml` (the service set the Compose-era switch generalizes); `docs/operations/deployment-upgrade-model.md` §3.1 (the one-component-at-a-time rule this design extends); [S-61] (metric-gated analysis consumes golden-signal-class metrics) |
| Files touched | this record only |

## 1. Honest status — the plain statement

**No progressive delivery has ever happened for any FlexCore component.**
The only deployment shape in the repository is single-host Compose
(`deploy/compose/docker-compose.yml`), the only upgrade procedure of
record is the P10.24 §3.1 rolling-upgrade design with an EMPTY run record
(P10.12 §7), and no second stack, load balancer, or rollout controller
exists anywhere. This record delivers the Compose-era switch design
(§3), the K8s-era canary design marked INERT (§4), and the mandatory
three-way scope distinction (§2). No threshold value is registered here
that a run could fill: every gate parameter cell is `<EMPTY —
pre-register at deployment>` (X-8), and the two metric families the
canary analysis consumes are the EXISTING `/metrics` registry families —
no new metric is invented.

## 2. The THREE-WAY scope distinction — mandatory, three mechanisms, three arguments

Context constraint 3 requires this note in every record that touches
rollout. The three mechanisms are separately-argued; none reinterprets,
relaxes, or borrows the authority of another.

**Mechanism 1 — kernel plugin rollout policy (P7.38,
`crates/flexcore-kernel/src/revocation.rs`, read-only).** The
`ROLLOUT_STATEMENT` is quoted VERBATIM (as recorded in the P18.06 record §1):

> `ROLLOUT_STATEMENT`: "v0.5.0 supports ONLY explicit per-profile staging: a \
> new plugin version is added to ONE profile first, then promoted profile-by-profile in \
> the plan's declared order. There is NO percentage canary — a percentage rollout is \
> not constructible from the StagedRollout type surface — and promoting to any profile \
> other than the exact next un-promoted one is refused."

**This policy is NOT reinterpreted, relaxed, or cited as permission** for
anything in this record. The kernel file is untouched by this wave
(read-only seam); the plugin staged rollout remains cohort/percentage-free
BY CONSTRUCTION. Argument: the subject is plugin swapping INSIDE one host
composition; blast radius is one composition; recovery is in-place
drain-and-remove — per-profile staging is the correct and sufficient
mechanism, and no percentage field exists on the type surface.

**Mechanism 2 — device-fleet cohort rollouts (P18.06,
`docs/phases-11-20/phase-18-edge-fleet-lifecycle-ota/records/06_p18_06_rollout_orchestrator_scope_distinction.md` — cited as DISTINCT).**
Argument (the P18.06 record's own, consumed not restated): the subject is
device software/firmware across a FLEET; a bad device update can BRICK
devices that cannot be recalled for hands-on recovery, so the staged
cohort progression with pre-registered abort thresholds is the mechanism's
whole point. That is a DEVICE-FLEET mechanism with X-8 pre-registered
configuration (canary band 500..=1000 basis points as the DEFAULT band —
a DEFAULT RECOMMENDATION, not a sourced figure) — it does not transfer to
control-plane software and this record does not borrow it.

**Mechanism 3 — control-plane software progressive delivery (THIS
record).** Argument: the subject is the control plane / worker / console
deployment — stateless (T3/T4 in the P12.04 register sense: stateless
behind the store + broker) and recoverable by artifact redeploy; a bad
control-plane version does not brick anything, it degrades orchestration
while the store and broker hold state. The correct mechanism is therefore
a health-gated traffic move between two complete deployments (Compose-era,
§3) or a metric-gated progressive weight shift (K8s-era, §4) — with the
state-carrying surfaces (Postgres, JetStream) NEVER canaried, because the
command lifecycle's durability gates (P12.05 §3.1) do not admit a partial
weight state.

| | Kernel plugin (P7.38) | Device fleet (P18.06) | Control-plane software (P20.11) |
|---|---|---|---|
| Subject | Plugin inside one host composition | Device firmware across a fleet | Control-plane software across environments |
| Failure domain | One composition | Bricked field devices | Degraded orchestration; state stays in store/broker |
| Mechanism | Per-profile staging, no percentage (type-level) | Staged cohorts with abort thresholds | Two-stack switch / metric-gated weight shift |
| Status | UNCHANGED — byte-identical | UNCHANGED — its own record | DESIGN only; nothing executed |

## 3. The Compose-era shape — two-stack switch with `/ready`-gated traffic move

The design for the CURRENT deployment shape (no cluster, no trigger
fired):

| Step | Design | Seam consumed |
|---|---|---|
| CS-1 | Two complete stacks — `blue` (current) and `green` (candidate) — each the full service set; the candidate's images are the SAME digest-pinned artifacts as any other environment (the P20.14 promotion property, consumed) | P20.14 (expected path) |
| CS-2 | Only ONE stack serves traffic. The "router" is the single reverse-proxy / port publisher the deployment runs; there is no service-mesh or weighted routing — the switch is a whole-traffic move, never a percentage | The Compose shape has no weighted-routing primitive; honesty over capability |
| CS-3 | Pre-switch gate: the green stack's `/ready` must report every named check ok (the P11.21 detailed body — `database`, `broker`, no `process shutting down`) | `apps/control-plane/src/probes.rs` |
| CS-4 | The switch: stop routing to blue; blue's control plane invokes `begin_shutdown` — readiness flips false FIRST so routing stops while the process drains (the P1.22 drain gate, consumed verbatim in substance); in-flight requests drain; the store and broker stay UP (blue and green share the SAME Postgres and JetStream — state is not canaried, ever) | `begin_shutdown` (probes.rs); P10.12 §3.1 rule 3 (in-flight dispatch transactions drained or failed, never corrupted) |
| CS-5 | Post-switch verification: version identifiers match the candidate; command/audit continuity; a smoke dispatch-lifecycle pass in shadow (the P10.12 U-12/U-13 criteria, consumed by citation) | P10.12 §5 (cited) |
| CS-6 | Rollback: the reverse named act — route back to blue, which re-readies and re-serves; no improvised down-migration is implied (the P10.13 irreversible-migration rule governs; a migration-bearing failure rolls back by restore per the recorded model) | `deployment-upgrade-model.md` §3.3 |

Honest limits: a whole-traffic switch has NO partial exposure — that is a
property of the Compose shape, recorded as a limitation, not a virtue.
Two stacks on one host double the footprint; whether that is practical on
the single host is a deployment decision `<EMPTY>`. The database-schema
dimension of a blue-green switch (expand-only migrations shared by both
stacks, the P11.18 discipline) is consumed, not re-derived.

## 4. The K8s-era shape — Argo Rollouts-class metric-gated canary (INERT until the P20.04 trigger)

Becomes designable ONLY when Agent A's P20.04 register records a fired
trigger (the automated-canary-demand trigger is one of the register's
named triggers). Until then this section is a decision-ready sketch and
nothing more:

| Element | Sketch (INERT) |
|---|---|
| Controller class | Argo Rollouts-class progressive-delivery controller [S-73] — a tool-class name, never a selection or conformance claim |
| Canary analysis | Automated comparison of a small new-version slice against the current version on EXISTING `/metrics` families (the P11.01 registry classes: request/error-rate and command-lifecycle families — the exact metric NAMES come from the registry, never invented here) before wider promotion; analysis thresholds pre-register (X-8) BEFORE the first canary run |
| Cardinality constraint | The canary labels must stay WITHIN Agent A's P20.01 cardinality budget — a per-version routing label is exactly the kind of label combination the budget governs (consumed by expected path) |
| Stateless slice only | Only the stateless services (control plane, worker, console) canary; Postgres and JetStream never canary (§3 CS-4 invariant carries) |
| Rollout object semantics | Steps, pauses, and aborts are the controller's mechanics — none is claimed as implemented, rendered, or available |

## 5. What this record does NOT establish

1. No rollout controller, second stack, router, or cluster exists; no
   traffic move, drain, switch, or canary has ever been executed; every
   gate-parameter cell is `<EMPTY>` (X-8).
2. The K8s-era shape is INERT until the P20.04 trigger register records a
   fired trigger — an unfired design is not a commitment (context
   constraint 1).
3. The kernel policy (P7.38) and the P18.06 orchestrator are UNTOUCHED —
   no file under `crates/flexcore-kernel/` or the P18 records was edited,
   and no cohort, percentage, or bulk field is introduced anywhere.
4. No dispatch authority: a rollout grants nothing; the command lifecycle
   gates, authorization, tenant isolation, audit, and the never-granted
   permission set are untouched by any topology in this record.
5. §13.3 semantics: a future successful switch or canary is a tested-day
   outcome, never a guarantee.

## 6. Reviewer-verification procedure (human)

1. Confirm §2 quotes the `ROLLOUT_STATEMENT` verbatim, cites P18.06 as
   DISTINCT, and carries the three-mechanism table — the three-way
   distinction is present and complete.
2. Confirm §3's CS-4 consumes the `begin_shutdown` drain gate
   (`apps/control-plane/src/probes.rs`) and that no percentage routing is
   claimed in the Compose shape.
3. Confirm §4 is marked INERT and consumes Agent A's P20.04/P20.05/P20.01
   by expected path only.
4. Confirm no file outside this record was touched and no ledger checkbox
   was ticked by this agent.

## 7. Findings and assumptions

| # | Item | Note |
|---|---|---|
| A-P20.11-1 | Argo Rollouts-class controllers perform metric-gated canary analysis [S-73-class context] | Tool-class context only; no tool is installed, selected, or claimed conformant |
| A-P20.11-2 | The Compose shape has no weighted-routing primitive | Recorded as a limitation shaping CS-2's whole-traffic move — honesty over capability invention |
| F-P20.11-1 | Agent A's P20.04 (trigger register) and P20.05 (chart layout) records were first consumed by EXPECTED PATH and reconciled to the landed paths during the same wave | The landed register is `records/04_p20_04_kubernetes_trigger_register.md` (§2 — the single trigger list; T-3 is the automated-canary trigger this record's §4 depends on) and `records/05_p20_05_helm_chart_design.md`; §4 remains INERT under the register's UNFIRED statuses |
| F-P20.11-2 | The three-way distinction's authoritative kernel text lives in `crates/flexcore-kernel/src/revocation.rs` (read-only seam) and the P18.06 record | Quoted via the P18.06 record §1's verbatim capture; the kernel file is byte-identical |

Status: **WRITTEN-UNRUN** — a design cannot roll out by being written;
the switch, the canary, and the first traffic move are human-gated acts
that have not happened and are not scheduled.
