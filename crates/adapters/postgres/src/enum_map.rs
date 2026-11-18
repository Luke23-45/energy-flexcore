// P1.13 — Vocabulary mappings between the contracts enums and the database.
//
// - `AssetType` (P0.10 §2) ↔ the `assets.asset_type` CHECK list (P1.12
//   §4.4): exactly the seven first-class 0.x types; backup generation is
//   deliberately absent and can only be added by a migration.
// - `TelemetryQuality` (P0.08) ↔ the `telemetry_quality` PostgreSQL enum
//   (D-S4): the six canonical names, verbatim from the contracts crate's
//   frozen `name()` spellings. Source-state mapping (P0.08 §3) happens
//   upstream at the telemetry adapter; the database never sees a seventh
//   value.

use crate::error::decode_failure;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_domain::entities::{AssetLifecycleStatus, AssetType};
use flexcore_ports::backend_error::BackendError;
use flexcore_ports::repository::RepositoryError;

/// The contracts asset type as the `assets.asset_type` text value.
pub(crate) fn asset_type_text(asset_type: AssetType) -> &'static str {
    match asset_type {
        AssetType::Bess => "bess",
        AssetType::PvInverter => "pv_inverter",
        AssetType::FlexibleCiLoad => "flexible_load",
        AssetType::EvseEvFleet => "evse_ev_fleet",
        AssetType::SiteMeter => "site_meter",
        AssetType::WindGeneration => "wind",
        AssetType::ThermalStorageHeatPump => "thermal_storage_heat_pump",
    }
}

/// The `assets.asset_type` text value as the contracts asset type. An
/// unknown value is a decode failure (the CHECK constraint makes one
/// impossible from this schema's own writers).
pub(crate) fn asset_type_from_text(raw: &str) -> Result<AssetType, RepositoryError> {
    match raw {
        "bess" => Ok(AssetType::Bess),
        "pv_inverter" => Ok(AssetType::PvInverter),
        "flexible_load" => Ok(AssetType::FlexibleCiLoad),
        "evse_ev_fleet" => Ok(AssetType::EvseEvFleet),
        "site_meter" => Ok(AssetType::SiteMeter),
        "wind" => Ok(AssetType::WindGeneration),
        "thermal_storage_heat_pump" => Ok(AssetType::ThermalStorageHeatPump),
        _ => Err(RepositoryError::Backend(decode_failure("assets.asset_type"))),
    }
}

/// The contracts registry lifecycle status as the `assets.lifecycle_status`
/// text value (the migration 0009 spellings, backed by the
/// `assets_lifecycle_consistency` CHECK).
pub(crate) fn lifecycle_status_text(status: AssetLifecycleStatus) -> &'static str {
    match status {
        AssetLifecycleStatus::Active => "active",
        AssetLifecycleStatus::Deactivated => "deactivated",
    }
}

/// The contracts quality state as the `telemetry_quality` enum text (the
/// frozen P0.08 names, taken from the contracts crate itself so the two
/// spellings cannot drift).
pub(crate) fn quality_text(quality: TelemetryQuality) -> &'static str {
    quality.name()
}

/// The `telemetry_quality` enum text as the contracts quality state. A
/// seventh value cannot exist in the enum (D-S4), so any mismatch is a
/// decode failure of a row written outside this adapter. The error is a
/// plain `BackendError` (a decode failure, like the sibling
/// `clock_quality_from_text` / `q_sign_from_text` in telemetry.rs) —
/// `RepositoryError` added nothing here: its only construction was the
/// `Backend` variant wrapping this same decode failure.
pub(crate) fn quality_from_text(raw: &str) -> Result<TelemetryQuality, BackendError> {
    TelemetryQuality::ALL
        .iter()
        .copied()
        .find(|q| q.name() == raw)
        .ok_or_else(|| decode_failure("asset_state.quality"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_type_vocabulary_is_the_first_class_set() {
        // P0.10 §2 first-class set, verbatim against the DDL CHECK list.
        let pairs: [(AssetType, &str); 7] = [
            (AssetType::Bess, "bess"),
            (AssetType::PvInverter, "pv_inverter"),
            (AssetType::FlexibleCiLoad, "flexible_load"),
            (AssetType::EvseEvFleet, "evse_ev_fleet"),
            (AssetType::SiteMeter, "site_meter"),
            (AssetType::WindGeneration, "wind"),
            (AssetType::ThermalStorageHeatPump, "thermal_storage_heat_pump"),
        ];
        for (variant, text) in pairs {
            assert_eq!(asset_type_text(variant), text);
            assert_eq!(asset_type_from_text(text).expect("known value"), variant);
        }
        // Deliberately absent: backup generation (P0.10 §2).
        assert!(asset_type_from_text("backup_generation").is_err());
        assert!(asset_type_from_text("BESS").is_err(), "mapping is exact, never case-normalised");
    }

    #[test]
    fn quality_vocabulary_is_exactly_the_six_canonical_states() {
        assert_eq!(TelemetryQuality::ALL.len(), 6);
        for quality in TelemetryQuality::ALL {
            let text = quality_text(quality);
            assert_eq!(quality_from_text(text).expect("known value"), quality);
            // The spelling must come from the contracts crate, not a copy.
            assert_eq!(text, quality.name());
        }
        // A seventh state is unrepresentable in the enum; the mapping
        // rejects anything outside the six names.
        assert!(quality_from_text("SUBSTITUTED").is_err());
        assert!(quality_from_text("OUT_OF_RANGE").is_err());
        assert!(quality_from_text("good").is_err(), "mapping is exact");
    }
}
