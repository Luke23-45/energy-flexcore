# Record 06 — P18.06: Staged rollout orchestrator (control plane) + the scope-distinction record vs the kernel plugin-rollout policy

Date: 2026-11-17. Subtask: P18.06 (`ledger_phase_18.md` §P18.06). Status:
**IMPLEMENTED-UNRUN** — real control-plane code
(`crates/flexcore-application/src/rollout_orchestrator.rs`, additive
module entry in `crates/flexcore-application/src/lib.rs`); **no rollout
runs, no cohort exists, no device is authorized, no fleet exists.**

| Field | Value |
|---|---|
| Files | `crates/flexcore-application/src/rollout_orchestrator.rs` (NEW); `crates/flexcore-application/src/lib.rs` (additive entry) |
| Consumed by citation | P18.12 fleet registry (Agent B, record 25; `crates/edge-agent/src/fleet_registry.rs` — `FleetStateView::current_cohort` / `CohortMembership`; migration `0023_edge_fleet_registry_p18_12`); P18.13 attestation policy (Agent B, record 26; `crates/edge-agent/src/attestation.rs` — `AttestationDecision`). Both through injected ports (`FleetCohortDirectory`; the `AttestationVerdict` mapped from `AttestationDecision` by the composition root) because the dependency rule forbids core→composition edges |
| Kernel policy | **UNTOUCHED — byte-identical** (`crates/flexcore-kernel/src/revocation.rs` verified unchanged) |

## 1. THE SCOPE DISTINCTION — the kernel policy quoted VERBATIM

The kernel plugin-rollout policy (`crates/flexcore-kernel/src/revocation.rs`,
P7.38) records, verbatim:

> `ROLLOUT_STATEMENT`: "v0.5.0 supports ONLY explicit per-profile staging: a \
> new plugin version is added to ONE profile first, then promoted profile-by-profile in \
> the plan's declared order. There is NO percentage canary — a percentage rollout is \
> not constructible from the StagedRollout type surface — and promoting to any profile \
> other than the exact next un-promoted one is refused."

and, from the same module's `StagedRollout` documentation:

> "The staging units are the P7.15 profiles (explicit per-profile staging — the \
> recorded v0.5.0 model; see `ROLLOUT_STATEMENT` for the no-percentage- \
> canary rule, which the type surface enforces: there is no cohort, \
> percentage, or bulk field anywhere on this type)."

**This policy is NOT reinterpreted, relaxed, or cited as permission.**
The kernel file is byte-identical after this wave (nothing in
`crates/flexcore-kernel/` was edited); the plugin staged rollout remains
cohort/percentage-free BY CONSTRUCTION — its type surface still cannot
express a percentage canary, and nothing in this phase changes that.

## 2. Why the device-fleet mechanism is a NEW, separately-argued mechanism

The two mechanisms govern different risk domains:

| | Kernel plugin rollout (P7.38, unchanged) | Device-fleet rollout (P18.06, new) |
|---|---|---|
| Subject | Plugin swapping INSIDE one host composition | Device software/firmware across a FLEET |
| Failure domain | A bad plugin version in one host — recoverable by drain-and-remove in place (`remove_failed_plugin` is drain-first) | A bad device update can BRICK devices that cannot be recalled for hands-on recovery (field-deployed gateways) |
| Blast radius | One composition | Potentially every device in the fleet |
| Why a canary IS expressible here | A percentage canary adds nothing to per-profile staging inside one host | The staged cohort progression (canary → blocks, each gated on the previous block's health) is the standard mitigation for exactly this bricking risk; the cohort structure with pre-registered abort thresholds is the mechanism's whole point |
| Parameters | Plan-declared profile order; no percentages (type-level) | Pre-registered configuration: canary size band, abort thresholds, minimum observations (X-8 — refused at plan time if unregistered) |

The separate argument is recorded in code as
`DEVICE_FLEET_ROLLOUT_SCOPE_STATEMENT` (pinned by tests) and repeated in
every audit line the orchestrator emits. The kernel's `NoOpRollout` and
out-of-order-promotion DISCIPLINES are mirrored (named refusals, stage
decisions audited) — the discipline carries over; the policy does not.

## 3. The orchestrator's mechanism (design summary)

- **Cohorts**: `RolloutPlan` declares the cohort order, canary first;
  advancement is in the declared order only (no skipping, no
  flag-flip full rollout; a half-judged cohort cannot be advanced:
  `ROLLOUT_COHORT_INCOMPLETE`). Membership comes from the directory
  port at activation and re-checked at each authorization (the
  registry's membership is authoritative; consumed by citation from
  P18.12).
- **Pre-registration (X-8, structural)**: `RolloutPlan::plan` REQUIRES
  the abort thresholds and the canary size — a plan without them cannot
  be constructed. The validator accepts 500..=1000 basis points as the
  pre-registered DEFAULT band. **The "canary 5–10% of a block" figure is
  a DEFAULT RECOMMENDATION, NOT a sourced figure** (the phase plan's
  P18.06 row) — it is pre-registered before any run; every actual value
  is the owner's configuration, never code.
- **Abort thresholds**: evaluated after EVERY recorded outcome; abort
  when the cohort failure rate EXCEEDS `max_failure_rate_permille` after
  at least `min_observations` outcomes (an equal rate does not abort —
  `ABORT_SEMANTICS_STATEMENT`). Abort is TERMINAL: no further
  authorization, no further advance; the abort reason rides the audit
  trail and the snapshot.
- **Per-device attestation before authorization**: a device is
  authorized only with a PASSED attestation verdict (the P18.13
  policy's `AuthorizeUpdate` mapped by the composition); `Failed` and
  `Unavailable` both refuse (`ROLLOUT_ATTESTATION_NOT_PASSED`) — the
  absence of evidence is never a pass. Authorization is a RECORD for the
  update pipeline; never a command, never dispatch authority.
- **Fleet-state visualization data**: `FleetRolloutSnapshot` /
  `CohortSnapshot` (per-cohort member/authorized/succeeded/failed/
  rolled-back/pending counts, phase, abort reason) — plain data for the
  console adapter to render.
- **Audit**: the in-module `RolloutAuditSink` port (the kernel-local
  audit-port pattern) — plan created, cohort activated, device
  authorized/refused, outcome recorded, threshold evaluated (every
  evaluation visible), abort, cohort advanced/refused. Injected clock
  only.

## 4. P18-E-3 interplay

P18-E-3 (rollout cohort policy defaults) stays OPEN — the DEFAULT
(option (a): canary 5–10% → blocks, failure-rate abort thresholds
pre-registered) is what this module implements as configuration-shaped
mechanism; the closer is the human owner recording the thresholds after
reviewing this design. Agents close nothing.

## 5. Honest status

Implemented-unrun with in-module tests (plan-validation battery,
authorization guard battery, directory-failure fail-closed, abort
threshold walk incl. the equal-rate continuation, declared-order
advancement to completion, reason/actor requirements, statement pins).
No rollout has ever run; no cohort exists; no fleet; the never-granted
permission set is untouched; the kernel policy is byte-identical.
