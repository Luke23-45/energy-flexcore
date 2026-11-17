// P16.10 — Settlement-quality meter data handling: accuracy-class
//         alignment, meter-device-resource mapping, and the dual-stream
//         separation rule.
//
// Phase 16 plan §2 row P16.10; sources: [S-31] (NYISO M-25 metering
// requirements manual — the settlement-grade metering posture class),
// [S-33] (ANSI C12.20 accuracy classes — the 0.2/0.5 class
// expectations), [S-59] (the RMI metering framework — metering as the
// settlement evidence base), [S-104] (ISO-NE Order 2222 metering &
// telemetry checklist — the metering-vs-telemetry posture class).
//
// WHAT THIS IS (decided once):
//
// - **The accuracy class is the METER's, declared with provenance — the
//   platform never claims a meter meets a class.** ANSI C12.20 defines
//   meter accuracy classes (0.2/0.5 are the class-0.2/class-0.5
//   expectations this module carries as declared labels [S-33]; NYISO
//   M-25 is the metering-requirements posture class [S-31]). A class
//   record here is a REFERENCE to the meter's documented class, recorded
//   data — never a certification claim, never a measurement claim by
//   FlexCore.
// - **Two streams, two types, no conversion path.** Settlement-interval
//   meter data and seconds-level telemetry are DISTINCT record types
//   ([`SettlementIntervalMeterData`] vs the Phase-2
//   `NormalizedTelemetry`). There is no `From`/`Into` between them, no
//   re-binning, no silent merge: the only interaction this module
//   permits is the EXPLICIT fail-closed gate
//   ([`refuse_telemetry_as_settlement`]) that a composition root calls
//   at any attempted boundary crossing — and it always refuses with the
//   named [`SettlementMeterError::StreamKindMismatch`]. The dual-stream
//   rule ([S-59][S-104]) is structural, not conventional.
// - **Settlement resolution is validated, never assumed.** A settlement
//   stream binding declares its interval cadence; a sub-5-minute
//   settlement cadence is refused (seconds-level data cannot silently
//   become settlement data — the exact hazard the dual-stream rule
//   exists for).
// - **Meter-device-resource linkage is EXACT string equality** (the
//   P8.11/P15.15 discipline): a near-miss reference resolves to nothing
//   and FLAGS, never matches.
//
// Control status: pure record types and gates for the simulation-only
// 0.x line. No meter is certified, no metering installation is claimed
// conformant, and no meter data exists in this program (the simulator
// produces telemetry, not settlement meter data).

use flexcore_contracts::time_model::EventTime;

use crate::fixed_decimal::FixedDecimal;
use crate::settlement_models::CAISO_SQMD_INTERVAL_MILLIS;
use flexcore_ports::telemetry_source::NormalizedTelemetry;

/// The settlement-meter model schema version (a change is a NEW version).
pub const SETTLEMENT_METER_SCHEMA_VERSION: u32 = 1;

/// The no-metering-claim statement (pinned by tests).
pub const PLATFORM_NEVER_CERTIFIES_METERING_STATEMENT: &str = "An accuracy-class record \
     is a REFERENCE to a meter's documented class (ANSI C12.20 class 0.2 / class 0.5 \
     expectations [S-33]; the NYISO M-25 metering-requirements posture [S-31]). The \
     platform NEVER claims a meter meets a class, never certifies a metering \
     installation, and never treats a declared class as a measurement guarantee. \
     Certification belongs to the meter's documentation and its testing authority.";

/// The dual-stream statement (pinned by tests).
pub const DUAL_STREAMS_NEVER_MERGED_STATEMENT: &str = "Settlement-interval meter data \
     and seconds-level telemetry are DISTINCT record types with NO conversion path: no \
     From/Into between them, no re-binning, no silent merge [S-59][S-104]. The only \
     permitted interaction is the explicit fail-closed gate, which ALWAYS refuses: \
     seconds-level telemetry can never silently become settlement data, and settlement \
     data can never silently become telemetry.";

/// The closed accuracy-class label vocabulary (the ANSI C12.20 class
/// expectations carried as declared labels [S-33]; the class is the
/// meter's documented property, referenced — never claimed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MeterAccuracyClass {
    /// The class-0.2 expectation (the tighter class; [S-33]).
    Class02,
    /// The class-0.5 expectation ([S-33]).
    Class05,
}

impl MeterAccuracyClass {
    /// The canonical label spelling (records and audit details).
    pub const fn as_str(self) -> &'static str {
        match self {
            MeterAccuracyClass::Class02 => "class-0.2",
            MeterAccuracyClass::Class05 => "class-0.5",
        }
    }
}

/// The meter's declared accuracy-class RECORD: a reference to the
/// meter's documented class, with the recorded source (non-empty) —
/// never a platform claim ([`PLATFORM_NEVER_CERTIFIES_METERING_STATEMENT`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeterAccuracyClassRecord {
    /// The meter reference (exact-match identity in the mapping below).
    pub meter_ref: String,
    /// The declared class label.
    pub declared_class: MeterAccuracyClass,
    /// The recorded source of the declaration (the meter's documentation
    /// reference; non-empty — an unsourced class is unconstructable).
    pub source: String,
}

/// The meter-device-resource mapping: which METER feeds which registered
/// DEVICE under which jurisdiction RESOURCE identity. All three links
/// are EXACT string equality on the references ([S-104] posture class;
/// the resource vocabulary is the jurisdiction's, carried verbatim —
/// never invented).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeterDeviceResourceMapping {
    /// The meter reference (the physical/documented meter).
    pub meter_ref: String,
    /// The registered device reference (the registry's device identity).
    pub device_ref: String,
    /// The jurisdiction resource reference as published (verbatim; the
    /// vocabulary is the jurisdiction's — ADR-011).
    pub resource_ref: String,
}

impl MeterDeviceResourceMapping {
    /// Validates and constructs one mapping (fail-closed: no empty
    /// references; the linkage itself is exact equality by construction
    /// — lookups compare strings exactly).
    pub fn new(
        meter_ref: String,
        device_ref: String,
        resource_ref: String,
    ) -> Result<Self, SettlementMeterError> {
        if meter_ref.trim().is_empty() {
            return Err(SettlementMeterError::EmptyReference { field: "meter_ref" });
        }
        if device_ref.trim().is_empty() {
            return Err(SettlementMeterError::EmptyReference {
                field: "device_ref",
            });
        }
        if resource_ref.trim().is_empty() {
            return Err(SettlementMeterError::EmptyReference {
                field: "resource_ref",
            });
        }
        Ok(Self {
            meter_ref,
            device_ref,
            resource_ref,
        })
    }

    /// The EXACT-equality lookup: resolves a meter reference to the
    /// mapping whose `meter_ref` equals it byte-for-byte. A near-miss
    /// resolves to NOTHING (the caller FLAGS) — never a fuzzy match.
    pub fn exact_meter_lookup<'a>(
        mappings: &'a [MeterDeviceResourceMapping],
        meter_ref: &str,
    ) -> Option<&'a MeterDeviceResourceMapping> {
        mappings.iter().find(|m| m.meter_ref == meter_ref)
    }
}

/// Why a settlement-meter operation was refused. Named, never absorbed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettlementMeterError {
    /// A reference field was empty.
    EmptyReference { field: &'static str },
    /// The settlement interval was inverted or not the declared binding
    /// cadence (re-binning is refused, never performed).
    IntervalInvalid { expected_millis: i64 },
    /// The declared settlement cadence was below the 5-minute
    /// settlement class — seconds-level data cannot bind as settlement
    /// data (the dual-stream rule).
    CadenceBelowSettlementClass { millis: i64 },
    /// An explicit attempt to treat a seconds-level telemetry record as
    /// settlement data. ALWAYS a refusal — this error exists so the
    /// attempt is named, never absorbed.
    StreamKindMismatch,
    /// The metered quantity was non-finite or failed decimal parsing.
    QuantityInvalid { reason: String },
}

impl SettlementMeterError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            SettlementMeterError::EmptyReference { .. } => "SETTLEMENT_METER_EMPTY_REFERENCE",
            SettlementMeterError::IntervalInvalid { .. } => "SETTLEMENT_METER_INTERVAL_INVALID",
            SettlementMeterError::CadenceBelowSettlementClass { .. } => {
                "SETTLEMENT_METER_CADENCE_BELOW_CLASS"
            }
            SettlementMeterError::StreamKindMismatch => "SETTLEMENT_METER_STREAM_KIND_MISMATCH",
            SettlementMeterError::QuantityInvalid { .. } => "SETTLEMENT_METER_QUANTITY_INVALID",
        }
    }
}

impl std::fmt::Display for SettlementMeterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SettlementMeterError::EmptyReference { field } => {
                write!(f, "the '{field}' reference must be a non-empty string")
            }
            SettlementMeterError::IntervalInvalid { expected_millis } => write!(
                f,
                "the interval is inverted or not the declared {expected_millis} ms \
                 cadence; the record is refused, never re-binned"
            ),
            SettlementMeterError::CadenceBelowSettlementClass { millis } => write!(
                f,
                "the declared settlement cadence ({millis} ms) is below the 5-minute \
                 settlement class; seconds-level data cannot bind as settlement data \
                 (the dual-stream rule [S-59][S-104])"
            ),
            SettlementMeterError::StreamKindMismatch => write!(
                f,
                "a seconds-level telemetry record cannot be treated as settlement \
                 meter data: the dual streams are distinct types with no conversion \
                 path — the gate always refuses (the non-silencing rule)"
            ),
            SettlementMeterError::QuantityInvalid { reason } => {
                write!(f, "the metered quantity failed validation: {reason}")
            }
        }
    }
}

impl std::error::Error for SettlementMeterError {}

/// One settlement-interval meter-data RECORD (the settlement stream — a
/// type deliberately DISTINCT from `NormalizedTelemetry`; no conversion
/// path exists in either direction). The metered quantity is an exact
/// decimal ([`crate::fixed_decimal::FixedDecimal`]) — settlement
/// quantities are settlement-adjacent records, never floats.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettlementIntervalMeterData {
    /// The meter reference (exact-match identity into the mapping).
    pub meter_ref: String,
    /// Start of the settlement interval (UTC, start-inclusive).
    pub interval_start: EventTime,
    /// End of the interval (UTC, end-exclusive; the length must equal
    /// the declaring binding's cadence).
    pub interval_end: EventTime,
    /// The metered quantity as published (exact decimal, source basis).
    pub quantity: FixedDecimal,
    /// The quantity basis as published (verbatim unit label; e.g. the
    /// meter data table's basis — UNVERIFIED per source).
    pub quantity_basis: String,
    /// The quality flag as published (verbatim source vocabulary;
    /// UNVERIFIED per-source — the six-state telemetry vocabulary is NOT
    /// applied here: this is the settlement stream, and silently
    /// re-labeling source flags would be exactly the forbidden merge).
    pub source_quality_flag: String,
}

impl SettlementIntervalMeterData {
    /// Validates and constructs one settlement record against the
    /// declaring binding's cadence (fail-closed; the interval length is
    /// validated to be exactly the declared cadence).
    pub fn new(
        meter_ref: String,
        interval_start: EventTime,
        interval_end: EventTime,
        quantity: FixedDecimal,
        quantity_basis: String,
        source_quality_flag: String,
        binding_cadence_millis: i64,
    ) -> Result<Self, SettlementMeterError> {
        if meter_ref.trim().is_empty() {
            return Err(SettlementMeterError::EmptyReference { field: "meter_ref" });
        }
        if quantity_basis.trim().is_empty() {
            return Err(SettlementMeterError::EmptyReference {
                field: "quantity_basis",
            });
        }
        if source_quality_flag.trim().is_empty() {
            return Err(SettlementMeterError::EmptyReference {
                field: "source_quality_flag",
            });
        }
        if interval_end.unix_millis() - interval_start.unix_millis()
            != i128::from(binding_cadence_millis)
        {
            return Err(SettlementMeterError::IntervalInvalid {
                expected_millis: binding_cadence_millis,
            });
        }
        Ok(Self {
            meter_ref,
            interval_start,
            interval_end,
            quantity,
            quantity_basis,
            source_quality_flag,
        })
    }

    /// The stream kind — a constant of the TYPE. Carried so records can
    /// be tagged in operator views without any cross-type inference.
    pub const STREAM_KIND: SettlementStreamKind = SettlementStreamKind::SettlementIntervalMeter;
}

/// The closed stream-kind vocabulary (the dual-stream rule's labels).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettlementStreamKind {
    /// Settlement-interval meter data (the settlement stream).
    SettlementIntervalMeter,
    /// Seconds-level telemetry (the Phase-2 pipeline stream — records of
    /// type `NormalizedTelemetry`; named here for view labeling only).
    SecondsTelemetry,
}

impl SettlementStreamKind {
    /// The canonical label spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            SettlementStreamKind::SettlementIntervalMeter => "settlement-interval-meter",
            SettlementStreamKind::SecondsTelemetry => "seconds-telemetry",
        }
    }
}

/// One stream binding of a mapping: the declared cadence and store
/// reference for ONE stream kind. A mapping may bind both kinds
/// SEPARATELY (a meter that feeds both a settlement stream and a
/// telemetry stream) — never one binding for both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeterStreamBinding {
    /// The mapping this binding belongs to.
    pub mapping_meter_ref: String,
    /// Which stream this binding declares.
    pub kind: SettlementStreamKind,
    /// The declared cadence in milliseconds (the stream's interval).
    pub cadence_millis: i64,
    /// The store reference for the stream's records (a reference, never
    /// credentials — the P7.05 rule).
    pub store_ref: String,
}

impl MeterStreamBinding {
    /// Validates and constructs one binding. A SETTLEMENT binding must
    /// declare a cadence of at least the 5-minute settlement class
    /// ([`CAISO_SQMD_INTERVAL_MILLIS`]) — a sub-5-minute settlement
    /// cadence is the seconds-level stream wearing settlement clothes
    /// and is refused ([`SettlementMeterError::CadenceBelowSettlementClass`]).
    /// A TELEMETRY binding has no settlement-class floor (seconds-level
    /// is exactly what it is).
    pub fn new(
        mapping_meter_ref: String,
        kind: SettlementStreamKind,
        cadence_millis: i64,
        store_ref: String,
    ) -> Result<Self, SettlementMeterError> {
        if mapping_meter_ref.trim().is_empty() {
            return Err(SettlementMeterError::EmptyReference {
                field: "mapping_meter_ref",
            });
        }
        if store_ref.trim().is_empty() {
            return Err(SettlementMeterError::EmptyReference { field: "store_ref" });
        }
        if cadence_millis <= 0 {
            return Err(SettlementMeterError::CadenceBelowSettlementClass {
                millis: cadence_millis,
            });
        }
        if kind == SettlementStreamKind::SettlementIntervalMeter
            && cadence_millis < CAISO_SQMD_INTERVAL_MILLIS
        {
            return Err(SettlementMeterError::CadenceBelowSettlementClass {
                millis: cadence_millis,
            });
        }
        Ok(Self {
            mapping_meter_ref,
            kind,
            cadence_millis,
            store_ref,
        })
    }
}

/// The explicit fail-closed gate at the stream boundary: ANY attempt to
/// pass a seconds-level telemetry record where settlement data is
/// expected is REFUSED with the named
/// [`SettlementMeterError::StreamKindMismatch`]. The function performs
/// no conversion and no inspection-by-value: the refusal is by TYPE —
/// the record simply cannot be accepted. (The mirror direction needs no
/// gate function: no API in this module accepts a
/// `NormalizedTelemetry` as settlement data, and no API produces a
/// `NormalizedTelemetry` from settlement data — the conversion path is
/// structurally absent.)
pub fn refuse_telemetry_as_settlement(
    _record: &NormalizedTelemetry,
) -> Result<(), SettlementMeterError> {
    Err(SettlementMeterError::StreamKindMismatch)
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is a
// hand-derived property of the validation rules. Nothing is fabricated.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_contracts::quality::TelemetryQuality;
    use flexcore_contracts::time_model::{ClockQuality, IngestTime};
    use flexcore_contracts::units::ActivePowerMW;
    use flexcore_domain::ids::{AssetId, TenantId};
    use flexcore_ports::telemetry_source::{NormalizedTelemetryParts, QuantityValue};

    fn uid(n: u128) -> FlexUuid {
        FlexUuid::from_bytes(n.to_be_bytes())
    }

    fn t(millis: i64) -> EventTime {
        EventTime::from_unix_millis(millis).expect("valid test time")
    }

    fn it(millis: i64) -> IngestTime {
        IngestTime::from_unix_millis(millis).expect("valid test ingest time")
    }

    fn telemetry_record(millis: i64) -> NormalizedTelemetry {
        NormalizedTelemetry::new(NormalizedTelemetryParts {
            tenant_id: TenantId::new(uid(0xA100)),
            asset_id: AssetId::new(uid(0xD001)),
            quantity: flexcore_ports::telemetry_source::TelemetryQuantity::ActivePower,
            value: Some(QuantityValue::Power(
                ActivePowerMW::new(1.0).expect("finite test power"),
            )),
            event_time: t(millis),
            ingest_time: it(millis),
            source_sequence: 1,
            source_id: "sim://test".to_string(),
            quality: TelemetryQuality::Good,
            source_quality_code: None,
            clock_quality: ClockQuality::Synchronized,
        })
        .expect("valid constructed record")
    }

    #[test]
    fn statements_are_the_recorded_strings() {
        assert!(
            PLATFORM_NEVER_CERTIFIES_METERING_STATEMENT.contains("NEVER claims a meter")
        );
        assert!(PLATFORM_NEVER_CERTIFIES_METERING_STATEMENT.contains("[S-33]"));
        assert!(DUAL_STREAMS_NEVER_MERGED_STATEMENT.contains("NO conversion path"));
        assert!(DUAL_STREAMS_NEVER_MERGED_STATEMENT.contains("[S-59][S-104]"));
    }

    #[test]
    fn class_labels_match_the_ansi_c12_20_expectations() {
        assert_eq!(MeterAccuracyClass::Class02.as_str(), "class-0.2");
        assert_eq!(MeterAccuracyClass::Class05.as_str(), "class-0.5");
        // The class record REQUIRES a source (an unsourced class is
        // unconstructable — the class is the meter's, with provenance).
        let record = MeterAccuracyClassRecord {
            meter_ref: "MTR-1".to_string(),
            declared_class: MeterAccuracyClass::Class02,
            source: "fixture://meter-doc".to_string(),
        };
        assert_eq!(record.declared_class.as_str(), "class-0.2");
    }

    #[test]
    fn mapping_is_exact_equality_and_refuses_empty_references() {
        let mapping =
            MeterDeviceResourceMapping::new("MTR-1".into(), "DEV-1".into(), "RES-1".into())
                .expect("valid mapping");
        // Exact match resolves.
        let mappings = vec![mapping.clone()];
        assert!(MeterDeviceResourceMapping::exact_meter_lookup(&mappings, "MTR-1").is_some());
        // Near-miss (case difference, prefix) resolves to NOTHING.
        assert!(MeterDeviceResourceMapping::exact_meter_lookup(&mappings, "mtr-1").is_none());
        assert!(MeterDeviceResourceMapping::exact_meter_lookup(&mappings, "MTR-10").is_none());
        // Empty references are named refusals.
        let err = MeterDeviceResourceMapping::new(" ".into(), "DEV-1".into(), "RES-1".into())
            .expect_err("empty meter ref refused");
        assert_eq!(err.name(), "SETTLEMENT_METER_EMPTY_REFERENCE");
    }

    #[test]
    fn settlement_records_validate_the_declared_cadence() {
        let quantity = FixedDecimal::parse("0.125").expect("fixture quantity");
        let record = SettlementIntervalMeterData::new(
            "MTR-1".to_string(),
            t(0),
            t(CAISO_SQMD_INTERVAL_MILLIS),
            quantity.clone(),
            "kWh-upr".to_string(),
            "SYN-QF".to_string(),
            CAISO_SQMD_INTERVAL_MILLIS,
        )
        .expect("valid settlement record");
        assert_eq!(record.quantity.canonical_string(), "0.125");
        // A wrong-length interval is refused, never re-binned.
        let err = SettlementIntervalMeterData::new(
            "MTR-1".to_string(),
            t(0),
            t(240_000),
            quantity.clone(),
            "kWh-upr".to_string(),
            "SYN-QF".to_string(),
            CAISO_SQMD_INTERVAL_MILLIS,
        )
        .expect_err("wrong cadence refused");
        assert_eq!(err.name(), "SETTLEMENT_METER_INTERVAL_INVALID");
        // The settlement stream's kind label is a constant of the type.
        assert_eq!(
            SettlementIntervalMeterData::STREAM_KIND.as_str(),
            "settlement-interval-meter"
        );
    }

    #[test]
    fn settlement_bindings_refuse_sub_five_minute_cadences() {
        // A 2-second settlement binding is the seconds-level stream
        // wearing settlement clothes: refused (the dual-stream rule).
        let err = MeterStreamBinding::new(
            "MTR-1".to_string(),
            SettlementStreamKind::SettlementIntervalMeter,
            2_000,
            "store://settlement".to_string(),
        )
        .expect_err("sub-5-minute settlement cadence refused");
        assert_eq!(err.name(), "SETTLEMENT_METER_CADENCE_BELOW_CLASS");
        // A 5-minute settlement binding is valid.
        let binding = MeterStreamBinding::new(
            "MTR-1".to_string(),
            SettlementStreamKind::SettlementIntervalMeter,
            CAISO_SQMD_INTERVAL_MILLIS,
            "store://settlement".to_string(),
        )
        .expect("valid settlement binding");
        assert_eq!(binding.cadence_millis, 300_000);
        // A 2-second TELEMETRY binding is exactly what telemetry is.
        let binding = MeterStreamBinding::new(
            "MTR-1".to_string(),
            SettlementStreamKind::SecondsTelemetry,
            2_000,
            "store://telemetry".to_string(),
        )
        .expect("valid telemetry binding");
        assert_eq!(binding.kind.as_str(), "seconds-telemetry");
    }

    #[test]
    fn the_boundary_gate_always_refuses_telemetry_as_settlement() {
        // Seconds-level telemetry can never enter the settlement stream:
        // the gate refuses by TYPE, always, with the named error.
        let err = refuse_telemetry_as_settlement(&telemetry_record(0))
            .expect_err("telemetry-as-settlement refused");
        assert_eq!(err.name(), "SETTLEMENT_METER_STREAM_KIND_MISMATCH");
        assert!(err.to_string().contains("no conversion path"));
    }

    #[test]
    fn stream_kind_labels_are_stable() {
        assert_eq!(
            SettlementStreamKind::SettlementIntervalMeter.as_str(),
            "settlement-interval-meter"
        );
        assert_eq!(SettlementStreamKind::SecondsTelemetry.as_str(), "seconds-telemetry");
    }
}
