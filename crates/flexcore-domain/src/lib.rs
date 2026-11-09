//! Neryva FlexCore — domain crate (P1.06 scope; restructured by P1.05).
//!
//! The domain layer of the hexagonal architecture (report §6.1–§6.2, §8.1):
//! entities, aggregates, invariants, and the frozen Phase 0 vocabulary that
//! the domain owns. **Domain depends on nothing infrastructure** (report
//! §6.2): the only dependency is `flexcore-contracts`
//! (`tools/check_dependency_rule.py` enforces this; the Phase 0 seed modules
//! — units, quality, time model, API paths, event names — moved to
//! flexcore-contracts in P1.05).
//!
//! | Module | Contract | Content |
//! |---|---|---|
//! | [`command_state`] | P0.11 — `11_command_state_machine.md` | The 14-state protected command lifecycle and its exact 27-transition table (the test oracle), plus deterministic property tests over generated state sequences |
//! | [`command_lifecycle`] | P4.10 — report §14.1–§14.2 | The command lifecycle engine over the frozen edges: the three gates as structurally separate transition functions (validation / feasibility / authorization, each with its own evidence), actor + controlled-clock timestamping, journalled transitions, and the EXPIRED / SUPERSEDED / CANCELLED / FALLBACK state support. Reuses the P0.11 table as the single source of truth — no edge is added or re-encoded here |
//! | [`entities`] | P0.10 — `10_asset_capability_vocabulary.md` §10.1 | The Stage 1 entities (Tenant, Portfolio, Site, Asset, AssetState) with the P0.16 `tenant_id` scoping chain and validating constructors |
//! | [`dispatch_command`] | P0.10 §1 row 10; report §14.3 | `DispatchCommand` construction validation: every §14.3 field group required and validated; a domain type, **not** a working dispatch path |
//! | [`tenant_context`] | P0.16 — `16_tenancy_storage_principles.md` rule 5 | `TenantContext`, constructible only from an `AuthenticatedClaims` value that only the application layer can produce; no raw-string/tenant-ID path exists |
//! | [`ids`] | P1.05/P1.06 — typed identifiers | `TenantId` / `PortfolioId` / `SiteId` / `AssetId` / `CommandId` / `ScheduleId` wrapping `flexcore_contracts::FlexUuid`; no bare UUID confusion |
//! | [`registry_rules`] | P2.02 — registry deletion rule | The RESTRICT deletion rule (decided once: a tenant with portfolios, a portfolio with sites, or a site with assets cannot be deleted) as pure parent-emptiness guards, plus the rationale record |
//! | [`quality_rules`] | P2.08 — data-quality assignment and stale detection (P0.08 as oracle) | Pure per-value quality rules: `assign_quality` (source-reported invalid / hard validation → BAD; range / plausibility / consistency on a well-formed value → SUSPECT), event-time staleness with per-use-case `FreshnessLimit` parameters (age == limit not stale; limit + 1s stale), MISSING gap markers as `flexcore:derived:` records (P2.04 D-T6), ESTIMATED only via an explicit `DerivationRule` (Phase 2 configures none), and the pure `quality_state_changed` predicate for the P2.16 outbox trigger. The SUSPECT-vs-BAD boundary is decided once in the module header |
//!
//! Property tests use the in-crate deterministic splitmix64 generator
//! (`test_rng`, compiled only under `cfg(test)`); the dependency rule
//! forbids external test dependencies in this crate.
//!
//! # Scope and control status
//!
//! Vocabulary and invariants for the design baseline only. The entire 0.x
//! line is simulation-only; nothing in this crate authorises field control
//! or device writes. The command state machine is the protected lifecycle
//! oracle and `DispatchCommand` is a validated record type — neither is a
//! dispatch path. FlexCore is a supervisory orchestration layer and performs
//! no primary frequency response, synthetic inertia, relay protection,
//! inverter inner-loop control, or sub-cycle control.

pub mod bess_model;
pub mod capability_provenance;
pub mod capability_validate;
// P4.10 — the command lifecycle engine over the frozen §14.1 edges (gate
// separation, controlled-clock timestamping, journalled transitions). The
// frozen transition table in `command_state` stays the single source of
// truth; this module adds no edge.
pub mod command_lifecycle;
pub mod command_state;
pub mod dispatch_command;
pub mod entities;
pub mod ev_fleet_model;
pub mod flexible_load_model;
pub mod ids;
pub mod inverter_pq_model;
pub mod quality_rules;
pub mod registry_rules;
pub mod scheduling_time;
pub mod tenant_context;
pub mod volt_var_scoping;

#[cfg(test)]
pub(crate) mod test_rng;
