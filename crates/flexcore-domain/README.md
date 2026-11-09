# flexcore-domain

The domain layer of the Neryva FlexCore hexagonal architecture (report
§6.1–§6.2, §8.1): entities, aggregates, and invariants.

## Dependency rule

**Domain depends on nothing infrastructure** (report §6.2). The only crate
dependency is `flexcore-contracts`; everything else is the Rust standard
library. `tools/check_dependency_rule.py` (P1.04) enforces this edge set
mechanically, including for dev-dependencies.

## What it implements

| Module | Contract | Content |
|---|---|---|
| `command_state` | **P0.11** — `ledger/phase-00-product-freeze/11_command_state_machine.md` | The 14-state protected command lifecycle (`PROPOSED → VALIDATED → FEASIBLE → APPROVED → ISSUED → ACKNOWLEDGED → TRACKING → COMPLETED`, plus `REJECTED`, `FAILED`, `EXPIRED`, `CANCELLED`, `SUPERSEDED`, `FALLBACK`) with the exact 27-transition table as the test oracle: every frozen edge allowed, no shortcuts (no `PROPOSED → ISSUED`), terminal states as frozen, `FAILED`/`EXPIRED` exiting only via `FALLBACK`. Three-gate distinction preserved: validation ≠ feasibility ≠ authorization. Acknowledgement is not physical delivery; `SUPERSEDED` is not `CANCELLED`. **P1.06 added** deterministic property tests over generated state sequences (every accepted pair is a frozen edge; every random legal walk terminates in a terminal state). |
| `entities` | **P0.10** — `10_asset_capability_vocabulary.md` §10.1 (added in **P1.06**) | The Stage 1 §10.1 entities as vocabulary-faithful domain types (not database models): Tenant, Portfolio, Site, Asset, AssetState. Parent ownership is structural — Portfolio requires a Tenant, Site requires a Portfolio, Asset requires a Site — and `tenant_id` is carried on every tenant-owned entity (P0.16 rule 1). Constructors validate and return `Result`; no `Default` impls. Units are the contracts types (`ActivePowerMW`, `EnergyMWh`, `StateOfCharge`); AssetState carries operational state plus exactly one contracts `TelemetryQuality`, and reactive power only with its declared sign convention (P0.07). |
| `dispatch_command` | **P0.10** §1 row 10; report §14.3 (added in **P1.06**) | `DispatchCommand` construction validation (`try_new`): every §14.3 field group is required and validated — command ID, tenant/portfolio/site/asset scope, source subsystem + version, schedule ID + revision, reason/action/actor, P/Q or supported-parameter target in contracts unit types with recorded sign convention (P0.07: `P > 0` = export frozen; Q convention declared, never a bare `f64`), validity window, duration + ramp, positive sequence, idempotency key, grid-model/telemetry/validation-evidence references, authorization reference (scope, limits, expiry), signature or authenticated transport identity, supersedes/superseded-by. Per-field-group rejection tests plus deterministic property tests asserting `try_new` rejects exactly the invalid combinations. Two invariants are documented caller-side duties: command-ID uniqueness and per-asset/channel sequence monotonicity (a constructor cannot observe a registry). |
| `tenant_context` | **P0.16** — `16_tenancy_storage_principles.md` rule 5 (added in **P1.06**) | `TenantContext` is constructible **only** via `from_authenticated_claims(AuthenticatedClaims)`; `AuthenticatedClaims` has private fields and no public constructor in this crate — the application layer constructs it from real authentication (public wiring lands with the tenant-context integration subtask). No `From<String>`, no `From<TenantId>`, no `Default`: deriving tenant context from an untrusted request field is a compile error, not a runtime check. |
| `ids` | **P1.05** (restructure; extended in **P1.06**) | `TenantId` / `PortfolioId` / `SiteId` / `AssetId` (P1.05) and `CommandId` / `ScheduleId` (P1.06) typed identifier newtypes wrapping `flexcore_contracts::FlexUuid`, so a bare UUID can never be confused between entity kinds. |
| `quality_rules` | **P2.08** — `ledger/phase-02-asset-telemetry/ledger_phase_2.md` (added in **P2.08**; P0.08 vocabulary as the test oracle) | Pure data-quality assignment and stale detection, no I/O. **SUSPECT-vs-BAD boundary decided once in the module header:** a hard validation failure (schema/type, raw-f64 SOC outside [0,1] on the pre-construction path, source reports the value invalid) is BAD; a range or consistency/plausibility failure on an otherwise well-formed value (P outside rated limits, BESS discharging while SOC is 0) is SUSPECT. `assign_quality` returns only GOOD/SUSPECT/BAD; staleness is computed from `event_time` with per-use-case `FreshnessLimit` parameters passed in (age == limit is not stale; limit + 1s is — boundary-tested); MISSING gap markers are derived records under the reserved `flexcore:derived:` source namespace (P2.04 D-T6) and carry no value field; ESTIMATED is reachable only through `derive_estimated` with an explicit `DerivationRule` argument (Phase 2 configures none, so no code path produces it); `quality_state_changed` is the pure predicate behind the P2.16 `TelemetryQualityChanged` outbox trigger. `producer_of` is an exhaustive match over the six frozen states pinning the producer separation. Freshness limits and cadence are caller-supplied configuration, never hard-coded constants. |

**Property-test generator (P1.06 choice, recorded):** the dependency rule
forbids external test dependencies in this crate, so property tests use a
small in-crate deterministic generator (`src/test_rng.rs`, splitmix64 with
fixed seeds, compiled only under `cfg(test)`). All property runs are
reproducible: same seed, same sequence, same verdict.

The Phase 0 seed modules (`units`, `quality`, `time_model`, `api_paths`,
event names) moved to `flexcore-contracts` in P1.05 with their frozen
semantics unchanged; the P0.14 event vocabulary now lives as typed payloads
in `flexcore-events` (P1.09).

## Scope and control status

Vocabulary and invariants for the design baseline only. The entire 0.x line
is simulation-only; nothing here authorises field control or device writes.
The command state machine is the protected lifecycle oracle, and
`DispatchCommand` is a validated domain record — the crate is a **domain
type, not a working dispatch path**: there is no issue/acknowledge/track
execution logic here, and no plugin may select the control-authority mode.
FlexCore is a supervisory orchestration layer and performs no primary
frequency response, synthetic inertia, relay protection, inverter
inner-loop control, or sub-cycle control.
