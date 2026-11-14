//! Neryva FlexCore — canonical value types and API-facing contracts (P1.05).
//!
//! This crate is the shared value-type layer of the FlexCore control plane
//! (report §8.1). It implements the Phase 0 frozen contracts as Rust types:
//!
//! | Module | Freeze contract | Content |
//! |---|---|---|
//! | [`units`] | P0.07 — `07_units_sign_conventions.md` | Canonical units (MW, MVAr, MVA, MWh), P > 0 = export at the point of connection, reactive-power sign declared per grid model and device profile (no default), SOC fraction [0,1] with a separate `from_percent` constructor, BESS net power `P_net = P_dis − P_ch`, explicit per-unit base, display-only kW/kWh conversions |
//! | [`quality`] | P0.08 — `08_telemetry_quality_vocabulary.md` | Exactly six telemetry quality states, source-state mapping, non-silencing rule |
//! | [`time_model`] | P0.09 — `09_time_model.md` | `event_time` vs `ingest_time` as distinct types, validity windows, source sequence, clock quality, UTC enforced at construction, site timezone carried separately |
//! | [`ids`] | P1.05 — identifier representation | `FlexUuid` parse/format/nil over `[u8; 16]`; ID *generation* is an application-layer port concern (P1.07), not a contract concern |
//! | [`api_paths`] | P0.15 — `15_api_surface.md` | The 16 candidate REST resource paths under `/api/v1`; no arbitrary-execute endpoint |
//! | [`alarm`] | P5.31 — the minimal alarm model (ADDITIVE) | Closed alarm severity/code vocabularies, the alarm record the Overview workspace displays, and the model statement. Alarms are operator-attention records derived from existing platform facts — NOT protection signals; the frozen §15.3 event vocabulary is NOT extended (no alarm event exists or is added) |
//!
//! # Dependency policy
//!
//! Contracts depend on `serde` and `time` only (plus std). No infrastructure
//! dependency is permitted — see `tools/check_dependency_rule.py` (P1.04).
//!
//! # Scope and control status
//!
//! These are contract types for the design baseline only. The entire 0.x line
//! is simulation-only; nothing in this crate authorises field control or
//! device writes. FlexCore is a supervisory orchestration layer: it performs
//! no primary frequency response, synthetic inertia, relay protection,
//! inverter inner-loop control, or sub-cycle control.

// P5.31 — the minimal alarm model (additive; alarms are derived records,
// never §15.3 events, never protection signals).
pub mod alarm;
pub mod api_paths;
pub mod capability;
pub mod ids;
pub mod quality;
pub mod time_model;
pub mod units;
