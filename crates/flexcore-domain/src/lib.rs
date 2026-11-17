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

// P5.31 — the alarm derivation rules (additive): deterministic pure
// functions deriving the minimal alarm records (`flexcore-contracts::alarm`)
// from existing platform facts — telemetry quality states, dispatch
// outcomes, feasibility verdicts. Alarms are operator-attention records,
// never protection signals, and the frozen §15.3 event vocabulary is not
// extended.
pub mod alarm_rules;
// P15.07 — the BESS degradation data model (additive): monotonic counters
// (cumulative discharge MWh, FEC), the rainflow-fed DOD x mean-SOC histogram,
// temperature exposure, high-SOC dwell, and periodic SoH observations — all
// INPUT DATA with provenance (unknown narrows, never widens), never a
// degradation law (decision record
// docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
// 21_p15_07_bess_degradation_data_model.md).
pub mod bess_degradation;
// P3.05/P3.06/P3.07 — the BESS capability model (additive; FRV1-R1-005):
// the pure power/energy capability rules with degradation and reserve
// treatment (report §11.2, the foundations §7.2 auxiliary-load term, §12.1)
// and their edge-case behaviour — the P3.05/P3.06/P3.07 model, no I/O
// (decision record ledger/phase-03-flexibility-scheduling/
// 05_08_bess_pq_models.md).
pub mod bess_model;
// P15.09 — BMS-reported SoH ingestion vocabulary (additive): provenance and
// staleness rules per the envelope-staleness max-age pattern; platform-side
// SoH estimation is DEFERRED and recorded, never implemented (decision record
// docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
// 23_p15_09_soh_ingestion.md).
pub mod bess_soh;
// P15.10 — the warranty object model (additive): capacity-retention schedule,
// cycle/throughput limits, RTE/availability commitments, augmentation events,
// and the warranty-headroom computation against the P15.07 counters.
// Megapack CMA and Fluence Dispatchable Energy Guarantee are reference
// STRUCTURES only — no vendor conformance claim (decision record
// docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
// 24_p15_10_warranty_object_model.md).
pub mod bess_warranty;
pub mod capability_provenance;
pub mod capability_validate;
// P4.10 — the command lifecycle engine over the frozen §14.1 edges (gate
// separation, controlled-clock timestamping, journalled transitions). The
// frozen transition table in `command_state` stays the single source of
// truth; this module adds no edge.
pub mod command_lifecycle;
pub mod command_state;
pub mod dispatch_command;
// P14.15 — the DOE consumption interface (additive): per-connection-point /
// per-group import-export envelopes as optimizer constraint data with issue
// timestamps, expiry, declared update cadence, lapse-to-static-limit
// behavior, and breach/curtailment telemetry records (decision record
// docs/phases-11-20/phase-14-protocol-device-connectivity/records/
// 63_p14_15_doe_consumption_interface.md). Constraint data only — context
// §3 constraint 3.
pub mod doe_envelope;
pub mod entities;
// P17.05 — the smart-charging schedule composer (additive, wave 1 Agent A):
// dispatch envelopes (canonical MW) -> charging-profile plans (purpose
// selection, stack-level precedence, minChargingRate pairing, W/A
// conversion with the declared formula, GetCompositeSchedule
// reconciliation, stored-not-applied honesty). The plan is a payload
// CANDIDATE for the command lifecycle — the composer produces candidate
// shapes, never lifecycle states (decision record
// docs/phases-11-20/phase-17-ev-fleet-charging/records/
// 05_p17_05_smart_charging_composer.md). The frozen transition table in
// `command_state` stays the single source of truth; this module adds no
// edge.
pub mod ev_charging_composer;
pub mod ev_fleet_model;
// P17.06 — the vehicle-proposal constraint-record vocabulary (additive):
// NotifyEVChargingNeeds / NotifyEVChargingSchedule /
// NotifyChargingLimit / ClearedChargingLimit as RECORDS, never
// auto-commands (the ADR-010-EXT-01 pattern applied to the EV proposal
// surfaces); the lifecycle entry state is structurally pinned to PROPOSED
// (decision record
// docs/phases-11-20/phase-17-ev-fleet-charging/records/
// 21_p17_06_vehicle_proposal_ingestion.md).
pub mod ev_proposal_records;
// P17.08 — the dual-source SoC arbitration (additive): OCPP MeterValues SoC
// authoritative-while-plugged vs the vendor-agnostic telematics port
// (P17-E-3 OPEN, DEFAULT (a)); plug-state as participation ground truth,
// per-source staleness via the frozen quality vocabulary (STALE consumed,
// never redefined), telematics stale-when-sleeping, Tesla 500 ms telemetry
// recorded as the outlier (decision record
// docs/phases-11-20/phase-17-ev-fleet-charging/records/
// 23_p17_08_soc_arbitration.md).
pub mod ev_soc_arbitration;
// P17.09 — the departure-time constraint model (additive): per-vehicle
// energy-by-time constraints from vehicle calendars and the telematics port,
// mapped onto the HARD departure-energy policy of `ev_fleet_model` —
// ingestion may tighten and may never relax (decision record
// docs/phases-11-20/phase-17-ev-fleet-charging/records/
// 24_p17_09_departure_time_constraints.md).
pub mod ev_departure_constraints;
// P17.10 — the user-override precedence policy (additive): driver manual
// starts outrank VPP curtailment, resolved BEFORE issuance as a dispatch
// policy record — never inside an adapter, never against a lifecycle-issued
// command; negative tests pin the refusal shape (decision record
// docs/phases-11-20/phase-17-ev-fleet-charging/records/
// 25_p17_10_user_override_precedence.md).
pub mod ev_override_policy;
// P17.11 — the depot/site load-management model (additive): the site import
// cap composed from the transformer limit and the P14.15 DOE envelope
// (static fallback governing on lapse), simultaneity-factor shaping of
// per-charger limits, and the external-constraint profile the P17.05
// composer consumes — constraint data only, never a charging-profile
// payload (decision record
// docs/phases-11-20/phase-17-ev-fleet-charging/records/
// 26_p17_11_depot_load_management.md).
pub mod ev_depot_load;
// P17.13 — the V2G readiness model (additive, wave 2 Agent A): per-EVSE /
// per-vehicle bidirectional capability declarations and derived readiness
// verdicts CONSUMING the frozen five-condition `V2xConfig` gate (extended
// additively, never redefined), the OCPP 2.1 bidirectional/DER hooks as
// DESIGN records (NotifyAllowedEnergyTransfer, AFRRSignal, SetDERControl —
// UNVERIFIED, never exercised), and the North-American AC path recorded as
// a SEPARATE future integration. ADR-010 discipline: a readiness record is
// an assessment — it never authorizes dispatch (decision record
// docs/phases-11-20/phase-17-ev-fleet-charging/records/
// 42_p17_13_v2g_readiness_modeling.md).
pub mod ev_v2g_readiness;
// P17.04 — the EVSE registry extension (additive, wave 1 Agent A):
// boot/provisioning, link-vs-operational state separation (connection loss
// is NOT asset unavailability), the declared StatusNotification mapping
// that refuses unmapped states, and the transaction view — the vocabulary
// the OCPP CSMS adapter maps its provisioning messages into (decision
// record docs/phases-11-20/phase-17-ev-fleet-charging/records/
// 04_p17_04_boot_status_transaction_mapping.md). Additive to, not a change
// of, `registry_rules` (P2.02) or the entity vocabulary (P0.10).
pub mod evse_registry;
pub mod flexible_load_model;
pub mod ids;
// P14.13 — the IEEE 1547-2018 settings-profile repository (additive): the
// function catalog, jurisdiction default tables as a document store, and
// per-device settings state — declared data, never a claim that any device
// accepts any function (decision record
// docs/phases-11-20/phase-14-protocol-device-connectivity/records/
// 61_p14_13_settings_profile_repository.md).
pub mod ieee1547_settings;
pub mod inverter_pq_model;
pub mod quality_rules;
pub mod registry_rules;
pub mod scheduling_time;
// P14.14 — the inverter settings-management workflow (additive): certified
// baseline (utility-owned) vs aggregator schedules, primacy/precedence, the
// utility-override hook (1547-2018 Clause 10), and PROPOSED-entry lifecycle
// proposals — a settings change is a lifecycle command, never a direct
// adapter write (decision record
// docs/phases-11-20/phase-14-protocol-device-connectivity/records/
// 62_p14_14_settings_management_workflow.md). The frozen transition table
// in `command_state` stays the single source of truth; this module adds no
// edge.
pub mod settings_management;
pub mod tenant_context;
// P15.11 — the warranty evidence-pack generator (additive): the P15.07
// counters -> a timestamped, digest-chained claim-SUPPORT pack export with
// the P8.32 provenance-record shape (mirrored by citation). The pack
// SUPPORTS human warranty claims, it NEVER asserts them; the chain digest
// is test-grade FNV-1a 64, never claimed as a release signature (decision
// record docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
// 25_p15_11_warranty_evidence_pack.md).
pub mod volt_var_scoping;
// P15.15 — the interconnection-record vocabulary (additive): UL 1741 SB
// COCs, IEEE 1547.1-2020 test reports, certified function categories
// (the P14.13 catalog reused), and settings-profile references per DEVICE
// MODEL — an exact-linkage document store. The platform LISTS certified
// equipment; it NEVER claims certification for itself
// [S-21][S-169][S-170] (decision record
// docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
// 44_p15_15_interconnection_record_ingestion.md).
pub mod interconnection_records;
pub mod warranty_evidence;
// P16.02 — the market program/enrollment domain model (additive):
// Program, Contract, Enrollment, QualificationEvidence, and per-asset
// ParticipationWindow types with closed lifecycle states (the GE Program
// Manager analog [S-41]) — immutable records with a supersede model,
// recorded stacking rules (R-P16-7), the program's OWN baseline-method
// declaration as data (LR-073), and the P11 audit-chain LINK carried per
// record (decision record
// docs/phases-11-20/phase-16-market-integration-settlement/records/
// 02_p16_02_program_enrollment_domain_model.md).
pub mod market_program_model;
// P16.03 — the per-ISO registration-record vocabulary (additive): PJM
// per-DER + aggregation registration with EDC 45-day review tracking
// [S-90], ERCOT PUCT+ERCOT dual registration [S-100], ISO-NE CAMS/
// eMarket asset lifecycle [S-103], and attestations as signed records
// (recorded data, never validated claims). No registration is filed,
// transmitted, or claimed anywhere — ADR-011 jurisdiction-specific
// records written before any jurisdiction is named (D1, OPEN) (decision
// record docs/phases-11-20/phase-16-market-integration-settlement/
// records/03_p16_03_registration_data_management.md).
pub mod market_registration_records;

#[cfg(test)]
pub(crate) mod test_rng;
