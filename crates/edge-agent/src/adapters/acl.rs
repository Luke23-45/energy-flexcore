// P6.12 — The anti-corruption layer (report §6.4, §10.2–§10.4; ledger
// P6.12 step 2).
//
// The ACL is where EMS-native terms become canonical FlexCore contracts —
// and the ONLY place translation happens. Its rules, each enforced by
// construction below:
//
// 1. **Every mapping is explicit.** A point is mapped only through a
//    declared [`PointMapping`] (quantity, native unit, native sign
//    meaning, declared reactive convention). A reading for an unmapped
//    point is a named error — never a best-effort guess.
// 2. **Units are named or refused.** [`NativeUnit::parse`] accepts exactly
//    the native unit spellings the mapping table below defines; an unknown
//    unit string is [`AclError::UnknownNativeUnit`] — no invented unit,
//    no default scale factor.
// 3. **Quality is mapped through a declaration or refused.** The mapping
//    table:
//
//    | Native code | Canonical | Rule source |
//    |---|---|---|
//    | `GOOD` | `GOOD` | trusted measured source state |
//    | `BAD` | `BAD` | source reports invalid / hard rule failed |
//    | `STALE` | `STALE` | age beyond freshness |
//    | `MISSING` | refused (named error) | a `MISSING` marker is platform-derived (P2.04 D-T6); the edge emits no gap markers |
//    | `SUBSTITUTED` | `ESTIMATED` | `flexcore_contracts::quality::map_source_state` |
//    | `OUT_OF_RANGE` | `SUSPECT` | `map_source_state` |
//    | `UNSYNCHRONIZED` | `SUSPECT` | `map_source_state` |
//    | `COMMUNICATION_LOSS` | `STALE` | `map_source_state` |
//    | `UNRELIABLE` | `SUSPECT` | `map_source_state` |
//
//    A reading with NO quality code is [`AclError::QualityNotDeclared`];
//    an unknown code is [`AclError::UnknownQualityCode`]. There is NO
//    default quality anywhere. Non-silencing (P0.08): no path produces
//    `GOOD` from a degraded source state; the original code is retained as
//    `source_quality_code` diagnostic metadata.
// 4. **Time is UTC or refused.** [`NativeTimestamp`] carries either a Unix
//    millisecond value or an RFC 3339 string that MUST carry a zero UTC
//    offset; a non-UTC timestamp is
//    [`AclError::NonUtcNativeTimestamp`] — never silently converted
//    (P0.09; the site timezone travels as metadata, not as a canonical
//    timestamp). `event_time` ≠ `ingest_time`: the sample carries both,
//    distinct types, ingest ≥ event enforced by the sample constructor.
// 5. **Signs are declared, not inferred.** Active power: canonical is
//    P > 0 = export at the point of connection (P0.07 §2); a mapping whose
//    native sign means import-positive converts EXPLICITLY
//    ([`NativeSign::ImportPositive`] → negation) and the conversion is the
//    mapping's recorded choice. Reactive power: the value passes through
//    in MVAr together with the DECLARED source convention
//    ([`PointMapping::reactive_source_convention`] — required for reactive
//    points; [`AclError::ReactiveConventionNotDeclared`] otherwise); the
//    convention travels on the value (P0.07 §3), it is never applied as a
//    silent flip.
// 6. **Quantity/unit pairing is enforced.** W/kW/MW only map to
//    `ActivePower`; var/kvar/Mvar only to `ReactivePower`; Wh/kWh/MWh only
//    to `Energy`; `%`/`fraction` only to `Soc`. Anything else is
//    [`AclError::UnitQuantityMismatch`].
// 7. **Non-finite raw values are refused** ([`AclError::NonFiniteValue`])
//    — the canonical unit newtypes would reject them anyway, but the ACL
//    names the failure before any conversion.
//
// The output is a [`crate::ports::EdgeTelemetrySample`] — the same frozen
// vocabulary the control plane's canonical record uses (`TelemetryQuantity`
// / `QuantityValue` from `flexcore-ports`), so nothing downstream needs a
// second mapping.
//
// Specification-version note (P6.12 step 4): the mapping table above relies
// on FlexCore's OWN frozen contracts (P0.07/P0.08/P0.09; report §10.2–
// §10.4) — the six-state vocabulary, the unit set, and the time model are
// FlexCore norms. The protocol-side spellings (`GOOD`, `BAD`, the §10.4
// example source states) are the report's own §10.4 examples and the P0.08
// frozen document; the OpenEMS-specific channel/quality vocabulary is NOT
// encoded here — it is P6.10's job against the VERIFIED EdgeConfig API
// (currently UNVERIFIED, RESEARCH.md §1), and anything it adds extends the
// adapter's declared `QualityMappingDeclaration`, never this table
// implicitly. Anything not checked against a specification is an
// ASSUMPTION and is labelled at its use site.

use std::collections::HashMap;
use std::collections::HashSet;

use flexcore_contracts::quality::{map_source_state, TelemetryQuality};
use flexcore_contracts::time_model::{EventTime, IngestTime};
use flexcore_contracts::units::{
    ActivePowerMW, EnergyMWh, ReactivePowerMVAr, ReactiveSignConvention, StateOfCharge,
};
use flexcore_ports::telemetry_source::{QuantityValue, TelemetryQuantity};

use crate::adapters::port::PointKey;
use crate::ports::{EdgeSampleError, EdgeTelemetrySample, EdgeTelemetrySampleParts};

// --- Native vocabulary ------------------------------------------------------

/// The native units the ACL names (and therefore accepts). Any unit string
/// outside [`NativeUnit::parse`] is a named error — the ACL never invents a
/// unit or an implicit scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeUnit {
    /// Watts (active power).
    W,
    /// Kilowatts (active power) — display/vendor scale, converted here.
    Kw,
    /// Megawatts (canonical scale).
    Mw,
    /// Volt-amperes reactive (reactive power).
    Var,
    /// Kilovolt-amperes reactive (reactive power).
    Kvar,
    /// Megavolt-amperes reactive (canonical scale).
    Mvar,
    /// Watt-hours (energy).
    Wh,
    /// Kilowatt-hours (energy) — display/vendor scale, converted here.
    Kwh,
    /// Megawatt-hours (canonical scale).
    Mwh,
    /// State of charge as a percentage (vendor scale; converted through the
    /// explicit `from_percent` boundary constructor — never persisted as
    /// percent).
    SocPercent,
    /// State of charge as a fraction in [0,1] (canonical scale).
    SocFraction,
}

/// Errors of the anti-corruption layer. Every variant rejects the WHOLE
/// reading — there is no partial mapping (the P2.11 step-3 rule at the
/// adapter boundary).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AclError {
    /// The reading's point has no declared [`PointMapping`]. Unmapped means
    /// refused — never a guess.
    PointUnmapped { point: String },
    /// The mapping's declared point does not match the reading's point.
    PointMappingMismatch,
    /// The native unit string is not in the ACL's named unit table.
    UnknownNativeUnit { code: String },
    /// The native unit cannot map to the declared quantity (e.g. W →
    /// Energy).
    UnitQuantityMismatch {
        unit: NativeUnit,
        quantity: TelemetryQuantity,
    },
    /// The native quality code is not in the adapter's declared mapping.
    UnknownQualityCode { code: String },
    /// The reading carried NO quality code. There is no default quality.
    QualityNotDeclared,
    /// The source declared `MISSING`: a gap marker is platform-derived
    /// (P2.04 D-T6) and the edge agent is a measured source — the adapter
    /// must surface the gap through its own means (a failed read), not
    /// emit a MISSING sample.
    NativeMissingRequiresGapMarker,
    /// The native timestamp was not UTC (zero offset) or not parseable as
    /// RFC 3339. Local time is never silently converted (P0.09).
    NonUtcNativeTimestamp { reason: String },
    /// The raw value was NaN or infinite.
    NonFiniteValue,
    /// A reactive-power point was mapped without a declared source
    /// convention (P0.07 §3: required, no default).
    ReactiveConventionNotDeclared,
    /// The active-power mapping declared no native sign meaning.
    ActiveSignNotDeclared,
    /// The canonical sample constructor rejected the mapped record
    /// (ordering, namespace, quantity/value pairing — see
    /// [`EdgeSampleError`]).
    Sample(EdgeSampleError),
}

impl std::fmt::Display for AclError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AclError::PointUnmapped { point } => {
                write!(f, "point '{point}' has no declared mapping table entry; refusing to map")
            }
            AclError::PointMappingMismatch => {
                write!(f, "the reading's point does not match the mapping's point")
            }
            AclError::UnknownNativeUnit { code } => {
                write!(f, "unknown native unit '{code}'; no invented unit, no default scale")
            }
            AclError::UnitQuantityMismatch { unit, quantity } => {
                write!(f, "native unit {unit:?} cannot map to quantity {quantity:?}")
            }
            AclError::UnknownQualityCode { code } => {
                write!(f, "unknown native quality code '{code}'; no default quality exists")
            }
            AclError::QualityNotDeclared => {
                write!(f, "the reading carried no quality code; there is no default quality")
            }
            AclError::NativeMissingRequiresGapMarker => {
                write!(f, "native quality MISSING is a platform-derived gap marker (P2.04 D-T6); the edge agent cannot emit it")
            }
            AclError::NonUtcNativeTimestamp { reason } => {
                write!(f, "native event timestamp is not UTC (refused, not converted): {reason}")
            }
            AclError::NonFiniteValue => write!(f, "the raw native value is NaN or infinite"),
            AclError::ReactiveConventionNotDeclared => {
                write!(f, "a reactive-power mapping requires a declared ReactiveSignConvention (P0.07 §3)")
            }
            AclError::ActiveSignNotDeclared => {
                write!(f, "an active-power mapping requires a declared native sign meaning (P0.07 §2)")
            }
            AclError::Sample(error) => write!(f, "the mapped sample was rejected: {error}"),
        }
    }
}

impl std::error::Error for AclError {}

impl NativeUnit {
    /// Parses a native unit string into the ACL's named unit vocabulary.
    /// The accepted spellings are the mapping table below; anything else is
    /// [`AclError::UnknownNativeUnit`] — no invented unit.
    pub fn parse(code: &str) -> Result<Self, AclError> {
        match code.trim() {
            "W" => Ok(NativeUnit::W),
            "kW" => Ok(NativeUnit::Kw),
            "MW" => Ok(NativeUnit::Mw),
            "var" => Ok(NativeUnit::Var),
            "kvar" => Ok(NativeUnit::Kvar),
            "Mvar" => Ok(NativeUnit::Mvar),
            "Wh" => Ok(NativeUnit::Wh),
            "kWh" => Ok(NativeUnit::Kwh),
            "MWh" => Ok(NativeUnit::Mwh),
            "%" => Ok(NativeUnit::SocPercent),
            "fraction" => Ok(NativeUnit::SocFraction),
            other => Err(AclError::UnknownNativeUnit {
                code: other.to_string(),
            }),
        }
    }

    /// The scale factor to the CANONICAL unit of the same quantity family,
    /// or `None` when this unit belongs to another family (the caller must
    /// treat `None` as [`AclError::UnitQuantityMismatch`], never as 1.0).
    fn scale_to_canonical(self) -> Option<f64> {
        match self {
            // Active power family → MW.
            NativeUnit::W => Some(1.0e-6),
            NativeUnit::Kw => Some(1.0e-3),
            NativeUnit::Mw => Some(1.0),
            // Reactive power family → MVAr.
            NativeUnit::Var => Some(1.0e-6),
            NativeUnit::Kvar => Some(1.0e-3),
            NativeUnit::Mvar => Some(1.0),
            // Energy family → MWh.
            NativeUnit::Wh => Some(1.0e-6),
            NativeUnit::Kwh => Some(1.0e-3),
            NativeUnit::Mwh => Some(1.0),
            // SOC: handled by the dedicated constructors, not a scale.
            NativeUnit::SocPercent | NativeUnit::SocFraction => None,
        }
    }

    fn family(self) -> QuantityFamily {
        match self {
            NativeUnit::W | NativeUnit::Kw | NativeUnit::Mw => QuantityFamily::Power,
            NativeUnit::Var | NativeUnit::Kvar | NativeUnit::Mvar => QuantityFamily::Reactive,
            NativeUnit::Wh | NativeUnit::Kwh | NativeUnit::Mwh => QuantityFamily::Energy,
            NativeUnit::SocPercent | NativeUnit::SocFraction => QuantityFamily::Soc,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuantityFamily {
    Power,
    Reactive,
    Energy,
    Soc,
}

/// The native timestamp an adapter reports for `event_time` (P0.09). Two
/// accepted shapes; a local-time string is NOT accepted — the adapter must
/// resolve it against the site timezone metadata itself (and record the
/// timezone), because the canonical contract is UTC-only and the ACL
/// refuses non-UTC rather than converting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeTimestamp {
    /// Milliseconds since the Unix epoch (UTC by definition).
    UnixMillis(i64),
    /// An RFC 3339 string that must carry the zero UTC offset (e.g.
    /// `2026-11-12T10:00:00Z`). A non-zero offset is a named error.
    UtcRfc3339(String),
}

impl NativeTimestamp {
    /// Maps the native timestamp to the canonical [`EventTime`]. Refuses
    /// anything that is not UTC-representable (P0.09; never converted).
    pub fn to_event_time(&self) -> Result<EventTime, AclError> {
        match self {
            NativeTimestamp::UnixMillis(millis) => {
                EventTime::from_unix_millis(*millis)
                    .map_err(|error| AclError::NonUtcNativeTimestamp {
                        reason: error.to_string(),
                    })
            }
            NativeTimestamp::UtcRfc3339(text) => {
                // `time`'s RFC 3339 parser accepts offsets; we require the
                // ZERO offset — a +01:00 timestamp is refused, not shifted.
                let parsed = time::OffsetDateTime::parse(
                    text,
                    &time::format_description::well_known::Rfc3339,
                )
                .map_err(|error| AclError::NonUtcNativeTimestamp {
                    reason: format!("not a valid RFC 3339 timestamp: {error}"),
                })?;
                if !parsed.offset().is_utc() {
                    return Err(AclError::NonUtcNativeTimestamp {
                        reason: format!(
                            "the timestamp carries offset {} (only +00:00/Z is accepted; \
                             local time is never silently converted, P0.09)",
                            parsed.offset()
                        ),
                    });
                }
                EventTime::new(parsed).map_err(|error| AclError::NonUtcNativeTimestamp {
                    reason: error.to_string(),
                })
            }
        }
    }
}

/// What the NATIVE sign of an active-power reading means at the point of
/// connection (P0.07 §2: canonical is P > 0 = export; the mapping records
/// what the source means and converts EXPLICITLY).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeSign {
    /// The native value already follows the canonical rule: positive =
    /// export. No conversion.
    ExportPositive,
    /// The native value uses the opposite convention: positive = import
    /// (consumption). The ACL negates — an explicit, recorded conversion.
    ImportPositive,
}

/// The mapping-table entry for one point: the ONLY way a reading of that
/// point becomes a canonical sample. Every field is a declaration; nothing
/// is inferred.
#[derive(Debug, Clone, PartialEq)]
pub struct PointMapping {
    /// The point this entry maps.
    pub point: PointKey,
    /// The edge-local asset key the point belongs to (registry-ID mapping
    /// happens control-plane side, P2.25 step-2 pattern).
    pub asset_key: String,
    /// The canonical quantity the point maps to (frozen vocabulary).
    pub quantity: TelemetryQuantity,
    /// The native unit the point reports.
    pub native_unit: NativeUnit,
    /// What the native active-power sign means (REQUIRED for `ActivePower`
    /// points; ignored for others — the mismatch is
    /// [`AclError::ActiveSignNotDeclared`]).
    pub native_sign: Option<NativeSign>,
    /// The declared reactive sign convention of the source for THIS point
    /// (REQUIRED for `ReactivePower` points — P0.07 §3, no default).
    pub reactive_source_convention: Option<ReactiveSignConvention>,
}

/// The anti-corruption layer: the point mapping table plus the source
/// identity the mapped samples carry. One instance per composed adapter
/// configuration (P6.16's allowlist config is the enforcement point for
/// WHICH mappings ship; this type is the mechanism).
#[derive(Debug, Clone)]
pub struct AntiCorruptionLayer {
    mappings: HashMap<PointKey, PointMapping>,
    source_id_prefix: String,
    /// Points this ACL explicitly does NOT map (the unmapped inventory —
    /// P6.12 step 2: every unmapped field is listed as unmapped, never
    /// silently dropped).
    declared_unmapped: HashSet<PointKey>,
}

impl AntiCorruptionLayer {
    /// Builds the ACL over the declared mapping table. Every mapping must
    /// be internally valid (unit matches quantity; the required sign /
    /// convention declarations are present) — an invalid table is a
    /// construction-time error, never a runtime surprise.
    pub fn new(
        mappings: Vec<PointMapping>,
        source_id_prefix: impl Into<String>,
        declared_unmapped: Vec<PointKey>,
    ) -> Result<Self, AclError> {
        let prefix = source_id_prefix.into();
        if prefix.trim().is_empty() {
            return Err(AclError::Sample(EdgeSampleError::EmptySourceId));
        }
        if prefix.starts_with("flexcore:") {
            return Err(AclError::Sample(EdgeSampleError::ReservedSourceNamespace));
        }
        let mut table = HashMap::new();
        for mapping in mappings {
            Self::validate_mapping(&mapping)?;
            table.insert(mapping.point.clone(), mapping);
        }
        Ok(Self {
            mappings: table,
            source_id_prefix: prefix,
            declared_unmapped: declared_unmapped.into_iter().collect(),
        })
    }

    /// Validates one mapping entry against the quantity/unit pairing and
    /// sign-declaration rules (§6.4: the table itself is reviewable).
    fn validate_mapping(mapping: &PointMapping) -> Result<(), AclError> {
        match mapping.quantity {
            TelemetryQuantity::ActivePower => {
                if mapping.native_unit.family() != QuantityFamily::Power {
                    return Err(AclError::UnitQuantityMismatch {
                        unit: mapping.native_unit,
                        quantity: mapping.quantity,
                    });
                }
                if mapping.native_sign.is_none() {
                    return Err(AclError::ActiveSignNotDeclared);
                }
            }
            TelemetryQuantity::ReactivePower => {
                if mapping.native_unit.family() != QuantityFamily::Reactive {
                    return Err(AclError::UnitQuantityMismatch {
                        unit: mapping.native_unit,
                        quantity: mapping.quantity,
                    });
                }
                if mapping.reactive_source_convention.is_none() {
                    return Err(AclError::ReactiveConventionNotDeclared);
                }
            }
            TelemetryQuantity::Energy => {
                if mapping.native_unit.family() != QuantityFamily::Energy {
                    return Err(AclError::UnitQuantityMismatch {
                        unit: mapping.native_unit,
                        quantity: mapping.quantity,
                    });
                }
            }
            TelemetryQuantity::Soc => {
                if mapping.native_unit.family() != QuantityFamily::Soc {
                    return Err(AclError::UnitQuantityMismatch {
                        unit: mapping.native_unit,
                        quantity: mapping.quantity,
                    });
                }
            }
            // The remaining frozen quantities (Availability, OperatingMode,
            // Health, ConnectionState, BaselineReference) have no named
            // native-unit mapping in this ACL yet; a mapping that names one
            // is refused rather than guessed. Extending the table is an
            // explicit, reviewed change (the frozen vocabulary extension
            // rule, P2.04 §4).
            _ => {
                return Err(AclError::UnitQuantityMismatch {
                    unit: mapping.native_unit,
                    quantity: mapping.quantity,
                });
            }
        }
        Ok(())
    }

    /// The mapping for a point, if declared.
    pub fn mapping_for(&self, point: &PointKey) -> Option<&PointMapping> {
        self.mappings.get(point)
    }

    /// Whether the point is in the DECLARED-unmapped inventory (recorded as
    /// unmapped by the mapping-table review — the honest counterpart of a
    /// mapping).
    pub fn is_declared_unmapped(&self, point: &PointKey) -> bool {
        self.declared_unmapped.contains(point)
    }

    /// The declared-unmapped inventory (diagnostics/health visibility).
    pub fn unmapped_points(&self) -> Vec<&PointKey> {
        let mut points: Vec<&PointKey> = self.declared_unmapped.iter().collect();
        points.sort_by(|a, b| format!("{a}").cmp(&format!("{b}")));
        points
    }

    /// Maps one native quality code through the ACL table (§4 above). The
    /// canonical state is returned together with the decision source so the
    /// caller can retain the original code as metadata.
    pub fn map_quality_code(&self, code: Option<&str>) -> Result<TelemetryQuality, AclError> {
        let Some(code) = code else {
            return Err(AclError::QualityNotDeclared);
        };
        match code.trim() {
            "GOOD" => Ok(TelemetryQuality::Good),
            "BAD" => Ok(TelemetryQuality::Bad),
            "STALE" => Ok(TelemetryQuality::Stale),
            "MISSING" => Err(AclError::NativeMissingRequiresGapMarker),
            "SUBSTITUTED" => Ok(map_source_state(
                flexcore_contracts::quality::SourceQualityState::Substituted,
            )),
            "OUT_OF_RANGE" => Ok(map_source_state(
                flexcore_contracts::quality::SourceQualityState::OutOfRange,
            )),
            "UNSYNCHRONIZED" => Ok(map_source_state(
                flexcore_contracts::quality::SourceQualityState::Unsynchronized,
            )),
            "COMMUNICATION_LOSS" => Ok(map_source_state(
                flexcore_contracts::quality::SourceQualityState::CommunicationLoss,
            )),
            "UNRELIABLE" => Ok(map_source_state(
                flexcore_contracts::quality::SourceQualityState::Unreliable,
            )),
            other => Err(AclError::UnknownQualityCode {
                code: other.to_string(),
            }),
        }
    }

    /// Maps one native reading to a canonical edge sample — or a named
    /// error. This is the ONLY translation path in the edge agent.
    pub fn map_reading(
        &self,
        reading: &crate::adapters::port::EmsNativeReading,
        ingest_time: IngestTime,
    ) -> Result<EdgeTelemetrySample, AclError> {
        // 1. The point must be mapped (fail-closed on unmapped).
        let mapping = self.mappings.get(&reading.point).ok_or_else(|| {
            AclError::PointUnmapped {
                point: reading.point.to_string(),
            }
        })?;
        if mapping.point != reading.point {
            return Err(AclError::PointMappingMismatch);
        }

        // 2. The raw value must be finite.
        if !reading.raw_value.is_finite() {
            return Err(AclError::NonFiniteValue);
        }

        // 3. Quality: mapped or refused; the original code travels as
        //    diagnostic metadata.
        let quality = self.map_quality_code(reading.native_quality_code.as_deref())?;

        // 4. Time: UTC or refused; ingest >= event enforced by the sample
        //    constructor.
        let event_time = reading.event_time.to_event_time()?;

        // 5. Value: unit-family-checked conversion to the canonical unit
        //    newtype, with the declared sign handling.
        let value = self.map_value(mapping, reading.raw_value)?;

        // 6. The canonical sample (constructor enforces the remaining
        //    canonical rules: non-empty key, quantity/value pairing, time
        //    ordering, source namespace).
        let source_id = format!(
            "{prefix}/{component}/{channel}",
            prefix = self.source_id_prefix,
            component = reading.point.component,
            channel = reading.point.channel,
        );
        EdgeTelemetrySample::new(EdgeTelemetrySampleParts {
            asset_key: mapping.asset_key.clone(),
            quantity: mapping.quantity,
            value,
            quality,
            source_quality_code: reading.native_quality_code.clone(),
            event_time,
            ingest_time,
            source_sequence: reading.source_sequence,
            source_id,
        })
        .map_err(AclError::Sample)
    }

    /// The declared quantity/value conversion for one mapping — explicit
    /// per family, with the sign rules of §5 above.
    fn map_value(&self, mapping: &PointMapping, raw: f64) -> Result<QuantityValue, AclError> {
        match mapping.quantity {
            TelemetryQuantity::ActivePower => {
                let scale = mapping
                    .native_unit
                    .scale_to_canonical()
                    .ok_or(AclError::UnitQuantityMismatch {
                        unit: mapping.native_unit,
                        quantity: mapping.quantity,
                    })?;
                let native_sign =
                    mapping
                        .native_sign
                        .ok_or(AclError::ActiveSignNotDeclared)?;
                // Canonical rule (P0.07 §2): P > 0 = export. An
                // import-positive source is negated EXPLICITLY — this is
                // the recorded conversion, not an inference.
                let canonical = match native_sign {
                    NativeSign::ExportPositive => raw * scale,
                    NativeSign::ImportPositive => -(raw * scale),
                };
                Ok(QuantityValue::Power(ActivePowerMW::new(canonical).map_err(
                    |_| AclError::NonFiniteValue,
                )?))
            }
            TelemetryQuantity::ReactivePower => {
                let scale = mapping
                    .native_unit
                    .scale_to_canonical()
                    .ok_or(AclError::UnitQuantityMismatch {
                        unit: mapping.native_unit,
                        quantity: mapping.quantity,
                    })?;
                // The declared convention MUST be present (validated at
                // table construction; re-checked here so the function stands
                // alone). The value passes through in MVAr WITH its
                // convention — the contract carries both (P0.07 §3).
                let convention = mapping
                    .reactive_source_convention
                    .ok_or(AclError::ReactiveConventionNotDeclared)?;
                Ok(QuantityValue::Reactive {
                    power: ReactivePowerMVAr::new(raw * scale)
                        .map_err(|_| AclError::NonFiniteValue)?,
                    convention,
                })
            }
            TelemetryQuantity::Energy => {
                let scale = mapping
                    .native_unit
                    .scale_to_canonical()
                    .ok_or(AclError::UnitQuantityMismatch {
                        unit: mapping.native_unit,
                        quantity: mapping.quantity,
                    })?;
                Ok(QuantityValue::Energy(
                    EnergyMWh::new(raw * scale).map_err(|_| AclError::NonFiniteValue)?,
                ))
            }
            TelemetryQuantity::Soc => {
                // Percent is a vendor/display scale: the explicit
                // `from_percent` boundary constructor converts it (P0.07 §4)
                // — a percent value is never persisted as canonical SOC.
                let soc = match mapping.native_unit {
                    NativeUnit::SocPercent => {
                        StateOfCharge::from_percent(raw).map_err(|_| AclError::NonFiniteValue)?
                    }
                    NativeUnit::SocFraction => {
                        StateOfCharge::new(raw).map_err(|_| AclError::NonFiniteValue)?
                    }
                    _ => {
                        return Err(AclError::UnitQuantityMismatch {
                            unit: mapping.native_unit,
                            quantity: mapping.quantity,
                        })
                    }
                };
                Ok(QuantityValue::Fraction(soc))
            }
            // Other quantities are refused at table validation; this arm is
            // unreachable through a validated table. Defensive refusal keeps
            // the function total.
            _ => Err(AclError::UnitQuantityMismatch {
                unit: mapping.native_unit,
                quantity: mapping.quantity,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acl() -> AntiCorruptionLayer {
        AntiCorruptionLayer::new(
            vec![
                // Active power in kW, native sign import-positive (explicit
                // negation under the canonical export-positive rule).
                PointMapping {
                    point: PointKey::new("ess0", "ActivePower"),
                    asset_key: "ess-01".to_string(),
                    quantity: TelemetryQuantity::ActivePower,
                    native_unit: NativeUnit::Kw,
                    native_sign: Some(NativeSign::ImportPositive),
                    reactive_source_convention: None,
                },
                // Reactive power in kvar under the declared
                // injection-positive convention (value passes through WITH
                // its convention — no flip).
                PointMapping {
                    point: PointKey::new("ess0", "ReactivePower"),
                    asset_key: "ess-01".to_string(),
                    quantity: TelemetryQuantity::ReactivePower,
                    native_unit: NativeUnit::Kvar,
                    native_sign: None,
                    reactive_source_convention: Some(ReactiveSignConvention::InjectionPositive),
                },
                // SOC as a vendor percent (explicit from_percent boundary).
                PointMapping {
                    point: PointKey::new("ess0", "Soc"),
                    asset_key: "ess-01".to_string(),
                    quantity: TelemetryQuantity::Soc,
                    native_unit: NativeUnit::SocPercent,
                    native_sign: None,
                    reactive_source_convention: None,
                },
            ],
            "edge:pilot-site-A:openems-rest",
            vec![PointKey::new("ess0", "InternalDebugTemp")],
        )
        .expect("valid mapping table")
    }

    fn reading(
        component: &str,
        channel: &str,
        unit: NativeUnit,
        raw: f64,
        quality: Option<&str>,
        sequence: u64,
    ) -> crate::adapters::port::EmsNativeReading {
        crate::adapters::port::EmsNativeReading {
            point: PointKey::new(component, channel),
            native_unit: unit,
            raw_value: raw,
            native_quality_code: quality.map(str::to_string),
            event_time: NativeTimestamp::UnixMillis(1_000),
            source_sequence: sequence,
        }
    }

    fn ingest() -> IngestTime {
        IngestTime::from_unix_millis(2_000).expect("valid")
    }

    // --- Canonical conformance (§10.2 units/signs) -------------------------

    #[test]
    fn active_power_kw_import_positive_maps_to_mw_export_positive() {
        // -5 000 kW import-native → +5 MW canonical export (explicit
        // negation of an import-positive source; P > 0 = export holds).
        let sample = acl()
            .map_reading(
                &reading("ess0", "ActivePower", NativeUnit::Kw, -5_000.0, Some("GOOD"), 7),
                ingest(),
            )
            .expect("maps");
        assert_eq!(sample.quantity(), TelemetryQuantity::ActivePower);
        match sample.value() {
            QuantityValue::Power(power) => assert_eq!(power.value(), 5.0),
            other => panic!("expected a power value, got {other:?}"),
        }
        assert_eq!(sample.quality(), TelemetryQuality::Good);
        assert_eq!(sample.asset_key(), "ess-01");
        assert_eq!(sample.source_sequence(), 7);
        assert_eq!(sample.event_time().unix_millis(), 1_000);
        assert_eq!(sample.ingest_time().unix_millis(), 2_000);
        // The source identity is the edge prefix + the point — never the
        // reserved flexcore: namespace.
        assert_eq!(
            sample.source_id(),
            "edge:pilot-site-A:openems-rest/ess0/ActivePower"
        );
    }

    #[test]
    fn export_positive_source_maps_without_negation() {
        let export_acl = AntiCorruptionLayer::new(
            vec![PointMapping {
                point: PointKey::new("ess0", "ActivePower"),
                asset_key: "ess-01".to_string(),
                quantity: TelemetryQuantity::ActivePower,
                native_unit: NativeUnit::W,
                native_sign: Some(NativeSign::ExportPositive),
                reactive_source_convention: None,
            }],
            "edge:pilot-site-A:openems-rest",
            vec![],
        )
        .expect("valid");
        // 1 500 000 W → 1.5 MW, sign preserved (already canonical).
        let sample = export_acl
            .map_reading(
                &reading("ess0", "ActivePower", NativeUnit::W, 1_500_000.0, Some("GOOD"), 1),
                ingest(),
            )
            .expect("maps");
        match sample.value() {
            QuantityValue::Power(power) => assert_eq!(power.value(), 1.5),
            other => panic!("expected a power value, got {other:?}"),
        }
    }

    #[test]
    fn reactive_power_carries_its_declared_convention_without_flip() {
        // +2 000 kvar under injection-positive → 2 MVAr injection-positive;
        // the same value under consumption-positive would MEAN the opposite
        // — the convention travels, it is never applied as a silent flip
        // (P0.07 §3).
        let sample = acl()
            .map_reading(
                &reading("ess0", "ReactivePower", NativeUnit::Kvar, 2_000.0, Some("GOOD"), 3),
                ingest(),
            )
            .expect("maps");
        match sample.value() {
            QuantityValue::Reactive { power, convention } => {
                assert_eq!(power.value(), 2.0);
                assert_eq!(*convention, ReactiveSignConvention::InjectionPositive);
            }
            other => panic!("expected a reactive value, got {other:?}"),
        }
    }

    #[test]
    fn soc_percent_uses_the_explicit_boundary_constructor() {
        // 87 % → fraction 0.87 (the explicit from_percent boundary path;
        // percent is never persisted as canonical SOC, P0.07 §4).
        let sample = acl()
            .map_reading(
                &reading("ess0", "Soc", NativeUnit::SocPercent, 87.0, Some("GOOD"), 5),
                ingest(),
            )
            .expect("maps");
        match sample.value() {
            QuantityValue::Fraction(soc) => assert!((soc.value() - 0.87).abs() < 1e-12),
            other => panic!("expected a fraction value, got {other:?}"),
        }
    }

    // --- Quality mapping (§10.4) -------------------------------------------

    #[test]
    fn quality_table_maps_every_named_code() {
        let acl = acl();
        assert_eq!(acl.map_quality_code(Some("GOOD")).unwrap(), TelemetryQuality::Good);
        assert_eq!(acl.map_quality_code(Some("BAD")).unwrap(), TelemetryQuality::Bad);
        assert_eq!(acl.map_quality_code(Some("STALE")).unwrap(), TelemetryQuality::Stale);
        // The §10.4 richer states, through the frozen map_source_state rule.
        assert_eq!(
            acl.map_quality_code(Some("SUBSTITUTED")).unwrap(),
            TelemetryQuality::Estimated
        );
        assert_eq!(
            acl.map_quality_code(Some("OUT_OF_RANGE")).unwrap(),
            TelemetryQuality::Suspect
        );
        assert_eq!(
            acl.map_quality_code(Some("UNSYNCHRONIZED")).unwrap(),
            TelemetryQuality::Suspect
        );
        assert_eq!(
            acl.map_quality_code(Some("COMMUNICATION_LOSS")).unwrap(),
            TelemetryQuality::Stale
        );
        assert_eq!(
            acl.map_quality_code(Some("UNRELIABLE")).unwrap(),
            TelemetryQuality::Suspect
        );
        // Non-silencing: no source-state path produces GOOD.
        for code in ["SUBSTITUTED", "OUT_OF_RANGE", "UNSYNCHRONIZED", "COMMUNICATION_LOSS", "UNRELIABLE"] {
            assert_ne!(
                acl.map_quality_code(Some(code)).unwrap(),
                TelemetryQuality::Good,
                "a degraded source state must never map to GOOD"
            );
        }
    }

    #[test]
    fn unknown_absent_and_missing_quality_fail_closed() {
        let acl = acl();
        // Unknown code → named error, no default quality.
        assert_eq!(
            acl.map_quality_code(Some("WEIRD_VENDOR_FLAG")),
            Err(AclError::UnknownQualityCode {
                code: "WEIRD_VENDOR_FLAG".to_string()
            })
        );
        // Absent code → named error, no default quality.
        assert_eq!(acl.map_quality_code(None), Err(AclError::QualityNotDeclared));
        // Native MISSING is refused: gap markers are platform-derived.
        assert_eq!(
            acl.map_quality_code(Some("MISSING")),
            Err(AclError::NativeMissingRequiresGapMarker)
        );
        // And the same refusals hold through map_reading.
        let result = acl.map_reading(
            &reading("ess0", "ActivePower", NativeUnit::Kw, 1.0, Some("WEIRD_VENDOR_FLAG"), 1),
            ingest(),
        );
        assert_eq!(
            result,
            Err(AclError::UnknownQualityCode {
                code: "WEIRD_VENDOR_FLAG".to_string()
            })
        );
        let result = acl.map_reading(
            &reading("ess0", "ActivePower", NativeUnit::Kw, 1.0, None, 1),
            ingest(),
        );
        assert_eq!(result, Err(AclError::QualityNotDeclared));
        // The original code is retained as diagnostic metadata on success.
        let sample = acl
            .map_reading(
                &reading("ess0", "ActivePower", NativeUnit::Kw, 1.0, Some("UNRELIABLE"), 1),
                ingest(),
            )
            .expect("maps");
        assert_eq!(sample.source_quality_code(), Some("UNRELIABLE"));
        assert_eq!(sample.quality(), TelemetryQuality::Suspect);
    }

    // --- Units and pairing ---------------------------------------------------

    #[test]
    fn unknown_unit_string_is_a_named_error() {
        assert_eq!(
            NativeUnit::parse("flurb"),
            Err(AclError::UnknownNativeUnit {
                code: "flurb".to_string()
            })
        );
        // The named table parses (case-sensitive spellings, as documented).
        assert_eq!(NativeUnit::parse("kW"), Ok(NativeUnit::Kw));
        assert_eq!(NativeUnit::parse("%"), Ok(NativeUnit::SocPercent));
    }

    #[test]
    fn invalid_mapping_table_is_a_construction_error() {
        // W against Energy: wrong family.
        let bad = AntiCorruptionLayer::new(
            vec![PointMapping {
                point: PointKey::new("meter0", "EnergyOut"),
                asset_key: "meter-01".to_string(),
                quantity: TelemetryQuantity::Energy,
                native_unit: NativeUnit::W,
                native_sign: None,
                reactive_source_convention: None,
            }],
            "edge:pilot-site-A:openems-rest",
            vec![],
        );
        assert_eq!(
            bad,
            Err(AclError::UnitQuantityMismatch {
                unit: NativeUnit::W,
                quantity: TelemetryQuantity::Energy
            })
        );
        // Reactive without a declared convention: refused (no default).
        let bad = AntiCorruptionLayer::new(
            vec![PointMapping {
                point: PointKey::new("ess0", "ReactivePower"),
                asset_key: "ess-01".to_string(),
                quantity: TelemetryQuantity::ReactivePower,
                native_unit: NativeUnit::Kvar,
                native_sign: None,
                reactive_source_convention: None,
            }],
            "edge:pilot-site-A:openems-rest",
            vec![],
        );
        assert_eq!(bad, Err(AclError::ReactiveConventionNotDeclared));
        // Active without a declared native sign: refused (no default).
        let bad = AntiCorruptionLayer::new(
            vec![PointMapping {
                point: PointKey::new("ess0", "ActivePower"),
                asset_key: "ess-01".to_string(),
                quantity: TelemetryQuantity::ActivePower,
                native_unit: NativeUnit::Kw,
                native_sign: None,
                reactive_source_convention: None,
            }],
            "edge:pilot-site-A:openems-rest",
            vec![],
        );
        assert_eq!(bad, Err(AclError::ActiveSignNotDeclared));
        // Reserved-namespace source prefix: refused.
        let bad = AntiCorruptionLayer::new(vec![], "flexcore:sneaky", vec![]);
        assert_eq!(
            bad,
            Err(AclError::Sample(EdgeSampleError::ReservedSourceNamespace))
        );
    }

    // --- Unmapped points and mismatched points ------------------------------

    #[test]
    fn unmapped_point_is_refused_and_listed() {
        let acl = acl();
        // A reading for a point with no mapping: named error.
        let result = acl.map_reading(
            &reading("meter9", "VoltageL1", NativeUnit::Kw, 1.0, Some("GOOD"), 1),
            ingest(),
        );
        assert_eq!(
            result,
            Err(AclError::PointUnmapped {
                point: "meter9/VoltageL1".to_string()
            })
        );
        // The declared-unmapped inventory records the known-unmapped point.
        assert!(acl.is_declared_unmapped(&PointKey::new("ess0", "InternalDebugTemp")));
        assert!(!acl.is_declared_unmapped(&PointKey::new("ess0", "ActivePower")));
        assert_eq!(acl.unmapped_points().len(), 1);
    }

    // --- Time mapping (§10.3) ------------------------------------------------

    #[test]
    fn utc_rfc3339_maps_and_non_utc_is_refused_not_converted() {
        let utc = NativeTimestamp::UtcRfc3339("2026-11-12T10:00:00Z".to_string());
        let event = utc.to_event_time().expect("UTC maps");
        assert!(event.timestamp().offset().is_utc());

        let non_utc = NativeTimestamp::UtcRfc3339("2026-11-12T11:00:00+01:00".to_string());
        let error = non_utc.to_event_time().unwrap_err();
        assert!(
            matches!(error, AclError::NonUtcNativeTimestamp { .. }),
            "a +01:00 timestamp is refused, never shifted: {error}"
        );
        // A local-time-shaped string without a valid RFC 3339 form too.
        let garbage = NativeTimestamp::UtcRfc3339("12.11.2026 10:00".to_string());
        assert!(matches!(
            garbage.to_event_time(),
            Err(AclError::NonUtcNativeTimestamp { .. })
        ));
        // And through map_reading: ingest before event is rejected by the
        // sample constructor (P0.09 ordering).
        let acl = acl();
        let result = acl.map_reading(
            &crate::adapters::port::EmsNativeReading {
                point: PointKey::new("ess0", "ActivePower"),
                native_unit: NativeUnit::Kw,
                raw_value: 1.0,
                native_quality_code: Some("GOOD".to_string()),
                event_time: NativeTimestamp::UnixMillis(5_000),
                source_sequence: 1,
            },
            IngestTime::from_unix_millis(2_000).expect("valid"),
        );
        assert_eq!(
            result,
            Err(AclError::Sample(EdgeSampleError::TimeOrdering))
        );
    }

    #[test]
    fn non_finite_value_is_refused() {
        let acl = acl();
        let result = acl.map_reading(
            &reading("ess0", "ActivePower", NativeUnit::Kw, f64::NAN, Some("GOOD"), 1),
            ingest(),
        );
        assert_eq!(result, Err(AclError::NonFiniteValue));
    }
}
