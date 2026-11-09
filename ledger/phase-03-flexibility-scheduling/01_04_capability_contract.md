# P3.01–P3.04 — Capability contract: decisions and review record

- **Date:** 2026-11-07.
- **Scope:** P3.01 (capability-model contract freeze), P3.02 (provenance and
  unknown handling), P3.03 (time model and horizon), P3.04 (capability
  validation rules).
- **Normative sources:** report §11.1 (generic DER capability model), §10.2
  (units and sign), §10.3 (time model), §10.4 (telemetry quality), §10.5
  (schema governance), §11.2–§11.5 (asset classes), §14.2 (SUPERSEDED),
  §24 step 3 (24-hour horizon); foundations specification governs on
  conflict.
- **Files:** `crates/flexcore-contracts/src/capability.rs` (+ lib.rs module
  declaration), `crates/flexcore-domain/src/capability_provenance.rs`,
  `crates/flexcore-domain/src/scheduling_time.rs`,
  `crates/flexcore-domain/src/capability_validate.rs` (+ lib.rs module
  declarations), `crates/flexcore-contracts/tests/capability_schema.rs`,
  `crates/flexcore-domain/tests/capability_validation.rs`.

## 1. Decisions recorded here

| # | Decision | Where implemented |
|---|---|---|
| D-P3.01-1 | Capability schema frozen at version 1 (`CAPABILITY_SCHEMA_VERSION = 1`), serialized as the `schema_version` field; any other version is rejected at deserialization. Every schema change requires a version bump plus a migration note in this file (§10.5). | `flexcore-contracts/src/capability.rs` |
| D-P3.01-2 | Constraint categories are GENERIC slots (kind + unit + sign + provenance + validity window + expression); no field per technology. | same |
| D-P3.01-3 | Structural-vs-normative layering: the contract enforces structural sanity (ordered bounds, non-empty sets, kind/unit compatibility, UTC horizon start, sorted slices); the §10.2 normative boundary rules are P3.04 validators in flexcore-domain. | both crates |
| D-P3.02-1 | Unknown provenance NARROWS the feasible set to the recorded conservative default per asset class; unknown NEVER widens. Effective bound = declared ∩ default; empty intersection = infeasible, never repaired by widening. | `flexcore-domain/src/capability_provenance.rs` |
| D-P3.02-2 | Fallback for a (class, kind) pair without an explicit default row: collapse-to-zero in the constraint's unit — never "unconstrained". | same |
| D-P3.03-1 | Interval semantics: start-inclusive, end-exclusive; a boundary instant belongs to the LATER interval; a horizon-end instant belongs to no interval; UTC-only internally; horizon = exact integer multiple of the interval. | `flexcore-domain/src/scheduling_time.rs` |
| D-P3.03-2 | Quality-aggregation precedence (highest wins): MISSING > BAD > STALE > SUSPECT > ESTIMATED > GOOD; GOOD survives only if every input is GOOD; ESTIMATED stays ESTIMATED; empty bucket = MISSING. | same |
| D-P3.03-3 | Horizon rollover: a new horizon SUPERSEDES the prior revision and never silently overwrites or deletes it (§14.2); the pure rule returns the prior reference unchanged plus revision + 1. The store lands in P3.22. | same |
| D-P3.04-1 | Structured validation errors: field locator + offending value + frozen rule code — never a bare "invalid input"; messages carry no internal paths or secrets and are safe to log as audit-visible rejections. | `flexcore-domain/src/capability_validate.rs` |

## 2. P3.01 — what the schema expresses (version 1)

Every constraint is one generic slot carrying: `kind`, canonical `unit`,
declared `sign`, `provenance`, a `validity` window (§10.3), and a structural
`expression` (closed interval with inclusive bounds, numeric value set, or
declared mode set). The time-indexed set is a UTC horizon start, an interval
length in seconds (per-model configuration), an interval count, and
per-interval constraint slices (sorted, sparse coverage allowed: a missing
slice means "nothing declared here" and consumers must treat undeclared
dimensions per D-P3.02-1, never as unconstrained).

### 2.1 Constraint categories (all twelve §11.1 categories are representable)

| §11.1 category | `ConstraintKind` | Units | Sign |
|---|---|---|---|
| Active and reactive power | `ACTIVE_POWER` / `REACTIVE_POWER` | MW / MVAr | export-positive (frozen) / declared |
| Energy/SOC | `ENERGY_SOC` | MWh or SOC fraction [0,1] | magnitude / unsigned |
| Device mode (`u`) | `DEVICE_MODE` | dimensionless | unsigned; mode set DECLARED |
| Temperature and health | `TEMPERATURE_HEALTH` | MW (derate) or dimensionless | magnitude / unsigned |
| Ramp rate | `RAMP_RATE` | MW per minute | magnitude |
| Minimum up/down time | `MINIMUM_UP_TIME` / `MINIMUM_DOWN_TIME` | seconds | magnitude |
| Duration and recovery | `DURATION_RECOVERY` | seconds | magnitude |
| Site network constraints | `SITE_NETWORK` | MW or MVAr | declared |
| Forecast conditions | `FORECAST_CONDITION` | MW, MVAr, MWh, dimensionless | declared |
| Operator/customer commitments | `COMMITMENT` | MW, MWh, dimensionless | declared |

Cross-interval semantics (a minimum-up-time spanning several intervals) is
carried as the declared duration; its enforcement is the scheduler's
formulation concern (P3.17), not the schema's.

### 2.2 Kind/unit compatibility table (frozen; relaxing it is a version bump)

| Kind | Allowed units |
|---|---|
| `ACTIVE_POWER` | MW |
| `REACTIVE_POWER` | MVAr |
| `ENERGY_SOC` | MWh, SOC_FRACTION |
| `DEVICE_MODE` | DIMENSIONLESS |
| `TEMPERATURE_HEALTH` | MW, DIMENSIONLESS |
| `RAMP_RATE` | MW_PER_MINUTE |
| `MINIMUM_UP_TIME`, `MINIMUM_DOWN_TIME`, `DURATION_RECOVERY` | SECOND |
| `SITE_NETWORK` | MW, MVAr |
| `FORECAST_CONDITION` | MW, MVAr, MWh, DIMENSIONLESS |
| `COMMITMENT` | MW, MWh, DIMENSIONLESS |

### 2.3 Explicitly NOT expressed by schema version 1

| Not expressed | Reason |
|---|---|
| Intra-interval trajectories (shapes inside one interval) | Constraints are per-interval bounds; Tier 1 formulations (§12.3) decide per interval; no sub-cycle modelling (AGENTS.md rule 3). |
| Per-phase (unbalanced three-phase) quantities | Balanced aggregation only; unbalanced three-phase enters from v0.4.0 (phase CONTEXT, D3). |
| Embedded probability distributions | Forecast quantiles/scenarios live in the §10.1 ForecastBundle and are referenced, not embedded. |
| Thermal/health models | Temperature/health enter as declared bounds with provenance, not as a thermal model. |
| Network topology / impedance | Site-network constraints are declared bounds at the point of connection; the grid model is a separate §10.1 entity consumed by the Phase 4 feasibility gate (§12.4–§12.5). |
| Reactive-sign mapping profile | The schema carries the DECLARED convention enum only; the grid-model/device-profile mapping that produced it lives with those artifacts (§10.2). |

### 2.4 Vocabulary units on the wire

Canonical units only: `MW`, `MVAr`, `MWh`, `SOC_FRACTION`, `MW_PER_MINUTE`,
`SECOND`, `DIMENSIONLESS`. There is no kW/kWh variant in the enum, so a scaled
unit is unrepresentable in a typed model; the text boundary
(`validate_unit`, P3.04) rejects `kW`/`kWh`/other spellings before they map
into the enum.

## 3. P3.02 — the unknown-narrows rule (D-P3.02-1/D-P3.02-2)

**Rule:** an unknown-provenance capability narrows the feasible set to the
documented conservative default per asset class; unknown never widens —
relative to the declared bounds and relative to treating the dimension as
unconstrained. `MEASURED`/`CONFIGURED`/`INFERRED` use the declared bounds as
declared (configured values come from a named configuration source; inferred
values from a declared derivation rule; §11.1 requires adapters to state
which is which, and all are visible downstream).

**Effective bound** = declared ∩ conservative default; empty intersection =
INFEASIBLE (never widened back).

**Conservative-default table** (per class; identical values today, recorded
per class so P3.05–P3.11 can diverge them):

| Asset class | `RAMP_RATE` (MW/min) | `TEMPERATURE_HEALTH` | Any other kind (fallback) |
|---|---|---|---|
| BESS (§11.2) | [0, 0] — no ramp flexibility; power holds constant | [0, 0] — no temperature/health credit | [0, 0] in the constraint's unit |
| Inverter P-Q (§11.3) | [0, 0] | [0, 0] | [0, 0] |
| Flexible load (§11.4) | [0, 0] | [0, 0] | [0, 0] |
| EV fleet (§11.5) | [0, 0] | [0, 0] | [0, 0] |

The collapse-to-zero fallback is intentionally over-restrictive for a real
asset — acceptable in simulation and recorded as a limitation, not fixed by
widening (ledger P3.02 risk note). P3.05–P3.11 record richer per-asset
defaults on top of this table.

**Discrete declarations narrow too:** an unknown numeric value set is filtered
to values inside the default interval; an unknown mode set credits NO modes
(mode credit requires known provenance — `u` is declared, never assumed,
§11.1).

**Propagation:** `EnvelopeProvenance` (per-limit provenance + effective bound
+ summary counts) is the record the P3.12 envelope embeds;
`ScheduleProvenanceEntry` (asset ID + capability schema version + envelope
provenance) is the record the P3.21/P3.22 schedule embeds. Both are pure
domain types; P3.02 is the single implementation of the rule.

## 4. P3.03 — time model decisions (D-P3.03-1/2/3)

| Decision | Content |
|---|---|
| Horizon default | 24 hours (§24 step 3). Configuration value, not a hard requirement. |
| Interval default | 15 minutes — the phase ledger's PROPOSAL (open question 1, "confirm with product; it is configuration, not a report requirement"). Configuration value; any positive length dividing the horizon is valid. |
| Timezone | UTC-only internally (§10.3); the grid does no local-time arithmetic, so the DST rule (a local day is not always 24 hours) never applies to it. |
| Interval semantics | Start-inclusive, end-exclusive. A boundary instant belongs to the LATER interval; a horizon-end instant belongs to no interval. Horizon = exact integer multiple of the interval, so identical configurations always produce identical boundaries. |
| Arithmetic | Pure integer-millisecond arithmetic over the contract timestamp types; timestamps are rebuilt through the validating contract constructors (the domain crate takes no `time` dependency). |
| Aggregation precedence | MISSING > BAD > STALE > SUSPECT > ESTIMATED > GOOD (highest wins). MISSING outranks BAD: an incomplete bucket cannot be evaluated at all, so incompleteness is the honest state. GOOD survives only if every input is GOOD; ESTIMATED stays ESTIMATED (never upgrades); empty bucket = MISSING. Non-silencing by construction: no path returns GOOD unless all inputs are GOOD. |
| Value aggregation | Arithmetic mean (power-like over the interval) or sum (energy-like). Recorded assumption: sub-interval inputs are uniform-duration; non-uniform callers pass duration-weighted values. |
| Rollover | A new horizon supersedes the prior revision — never silently overwrites or deletes it (§14.2 SUPERSEDED, distinct from CANCELLED). `plan_horizon_rollover` returns the prior reference unchanged plus revision + 1 (first revision = 1); rollover applies per schedule; the store persisting both revisions lands in P3.22. |

## 5. P3.04 — validation catalogue (D-P3.04-1)

| Rule code | Check | Source |
|---|---|---|
| `UNIT_NOT_CANONICAL` | Text-payload unit label must be exactly `MW`, `MVAr`, `MVA`, `MWh`; kW/kWh and other spellings rejected | §10.2 |
| `VALUE_NOT_FINITE` | No NaN/infinity in any validated quantity | §10.2 (finite canonical quantities) |
| `ACTIVE_POWER_SIGN_NOT_EXPORT` | Every active-power constraint declares `EXPORT_POSITIVE` (P > 0 = export) | §10.2 |
| `REACTIVE_SIGN_NOT_DECLARED` | Every reactive-power constraint declares `INJECTION_POSITIVE` or `CONSUMPTION_POSITIVE`; never a default, never inferred from the sign alone | §10.2 |
| `SOC_OUT_OF_RANGE` | SOC fraction in [0,1], boundary-inclusive (0 and 1 valid); delegated to the frozen `StateOfCharge` type | §10.2 |
| `NEGATIVE_CHARGE_POWER` | `P_ch ≥ 0` | §11.2 |
| `NEGATIVE_DISCHARGE_POWER` | `P_dis ≥ 0` | §11.2 |
| `ENERGY_LIMITS_INVERTED` | `E_min ≤ E_max` (equal bounds valid) | §11.2 |
| `NEGATIVE_RAMP_RATE` | Ramp ≥ 0 (zero valid) | §11.1 |

`validate_capability_model` walks a whole `CapabilityModel` (sign conventions
per constraint, SOC bounds in [0,1], ramp lower bounds ≥ 0) so malformed data
is rejected before it can enter an envelope or a solve. Rejection messages
carry only the field locator, the offending value, and the rule — no internal
paths or secrets; emitting them as audit events is the later §15.5 wiring.

## 6. Review status and verification

- **Power-systems review sign-off (ledger P3.01 step 4): PENDING.** This
  record was written by the implementing agent; the expressible/unsupported
  table (§2 above) is the review input. A human power-systems review must
  confirm the table before the ledger P3.01 checkbox is ticked.
- **Terminology:** a solved schedule is a CANDIDATE, never "feasible" —
  feasibility is the Phase 4 gate; the capability model is not a `max_mw`
  field. Nothing in this record implies field control (0.x is
  simulation-only).
- **Not run (VERIFY-BUILD):** per the task constraint, `cargo build`, `cargo
  test`, `cargo clippy`, and `cargo fmt` were NOT executed for these files.
  All tests (inline + the two integration suites) are written to compile
  against the recorded APIs; the first `cargo test -p flexcore-contracts -p
  flexcore-domain` run must confirm, and any fix must keep the recorded
  decisions unchanged.
- **lib.rs doc tables:** the module tables in both crates' `lib.rs` were left
  untouched on purpose (ownership limited this task to additive module
  declarations); the three new domain modules and the capability contract
  module should be added to those tables in a later docs pass.
- **PLAN.md data-contract table:** the capability-model row (§4) already
  matches this contract (time-indexed feasible set, per-constraint
  provenance, units, sign, validity window, schema-version versioning); no
  edit was needed.
- **CONTEXT.md time-model note (ledger P3.03 step 4):** not edited in this
  task (outside file ownership); §4 of this record is the time-model
  documentation the later CONTEXT.md pass should incorporate.
- **Open item carried forward:** the 15-minute/24-hour defaults remain a
  product confirmation item (ledger open question 1); they are configuration
  values in code and labelled as proposals here.
