// P17.07 (application mapping) — MeterValues ingestion: cadence
// configuration, the MeterValuesSign hooks, and the adapter-boundary
// quality mapping onto the frozen six-state vocabulary.
//
// Implements ledger P17.07 (the application half; the worker wiring is
// `apps/worker/src/ev_ingest.rs`, which feeds the EXISTING P2.07 pipeline —
// no parallel ingestion path). Decision record:
// `docs/phases-11-20/phase-17-ev-fleet-charging/records/
// 22_p17_07_meter_values_ingestion.md`.
//
// # Cadence configuration
//
// The CSMS adapter configures the charge point's MeterValues reporting
// cadence from [`MeterValuesCadenceConfig`] (per-quantity sampled-value
// cadence plus a freshness limit per quantity). The exact OCPP variable
// names a charge point exposes for reporting configuration are
// SECONDARY-SOURCED [S-124][S-125] and UNVERIFIED against the OCA text —
// this module defines the DEPLOYMENT CONFIGURATION, not the wire
// interaction; the adapter's implementation-time check is recorded in the
// decision record. Freshness limits are per-quantity deployment
// configuration, never hard-coded (the P2.08/P2.21 rule).
//
// # The adapter-boundary quality mapping (the non-silencing rule intact)
//
// OCPP MeterValues sampled values carry no quality flag of their own
// (SECONDARY-SOURCED [S-124], UNVERIFIED); whatever richer source states a
// charge point or its meter reports surface as source-side codes through
// the adapter. Per the Phase 0 freeze (`flexcore-contracts::quality`,
// consumed — NEVER redefined, no seventh state):
//
// - every source-side code the adapter can report is DECLARED in
//   [`MeterValuesQualityMapping`] with the ONE canonical state it maps to;
//   an UNDECLARED code is a named error — never a default assignment (the
//   ACL rule: unknown quality codes are named errors, never defaults);
// - a sampled value with no source-side code maps to the DECLARED
//   no-code state (the adapter declares what a plain measured reading
//   maps to — typically `GOOD`, which is a legitimate assignment for a
//   measured value, NOT a conversion of a degradation into normal);
// - staleness never upgrades: a value whose age exceeds the quantity's
//   freshness limit is carried as `STALE` unless the mapped state is
//   already worse (`SUSPECT` stays `SUSPECT`, `BAD` stays `BAD` — a
//   source-reported degradation always survives the boundary);
// - no function here invents a seventh state, and no degradation code
//   maps to `GOOD` by construction of the recorded EV default mapping.
//
// # MeterValuesSign hooks (the Eichrecht-grade path; UNVERIFIED details)
//
// OCPP 2.1's signed meter values (the `MeterValuesSign` data hook) keep
// the Eichrecht/AFIR-caliber metrological integrity surface addressable
// without claiming compliance [S-135][S-136]. The exact block letters,
// message names, and field spellings are UNVERIFIED against the OCA text
// — the hook below is the PLATFORM-SIDE SEAM only: a signed meter value
// arrives OPAQUE (verbatim bytes as delivered by the adapter), is recorded
// with provenance, and is forwarded to a [`SignedMeterValueSink`]. The
// hook NEVER verifies a signature: signature verification is a
// metrological process (a calibration-body / lab concern, a HUMAN GATE),
// and this platform never claims Eichrecht or AFIR compliance.
//
// Control status: simulation-only application mapping. No charge point is
// connected, no meter value was ever observed, no signed meter value was
// ever received; the AFIR/Eichrecht verdict itself is P17.16's assessment
// record (wave 2) — this module claims nothing.

use flexcore_contracts::quality::TelemetryQuality;
use flexcore_ports::telemetry_source::TelemetryQuantity;
use std::collections::BTreeMap;

/// The recorded honesty statement for the signed-meter-value seam
/// (decided once).
pub const SIGNED_METER_VALUE_SEAM_STATEMENT: &str =
    "The MeterValuesSign seam records signed meter values OPAQUE with \
provenance and forwards them to a sink; it NEVER verifies a signature and \
NEVER claims Eichrecht or AFIR compliance. Signature verification is a \
metrological (calibration-body / lab) process — a HUMAN GATE. The exact \
OCPP 2.1 block letters and field spellings are UNVERIFIED against the OCA \
text [S-135][S-136]; a confirmed mismatch is a recorded edit, never a \
silent reinterpretation.";

/// Errors of the MeterValues mapping. Every variant names the failure —
/// the boundary refuses, it never defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeterValuesIngestError {
    /// A cadence or freshness value was zero or negative (per-quantity
    /// configuration is positive; there is no default).
    NonPositiveCadence {
        /// Which configuration field.
        field: &'static str,
        /// The offending value in ms.
        value: i64,
    },
    /// A source-side quality code arrived that the adapter never declared
    /// — a named error, never a default assignment (the ACL rule).
    UndeclaredSourceCode {
        /// The undeclared code (verbatim, for the quarantine record).
        source_code: String,
    },
    /// The signed meter value's opaque payload was empty — a signed value
    /// that signs nothing is not an evidence record.
    EmptySignedMeterValuePayload,
    /// The signed meter value's declared origin was empty.
    EmptySignedMeterValueOrigin,
}

impl std::fmt::Display for MeterValuesIngestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MeterValuesIngestError::NonPositiveCadence { field, value } => {
                write!(f, "the MeterValues {field} must be positive in ms (found {value})")
            }
            MeterValuesIngestError::UndeclaredSourceCode { source_code } => write!(
                f,
                "the source quality code '{source_code}' is not declared by the adapter's \
                 quality mapping — refusing, never defaulting (the ACL rule)"
            ),
            MeterValuesIngestError::EmptySignedMeterValuePayload => {
                write!(f, "the signed meter value payload is mandatory and must be non-empty")
            }
            MeterValuesIngestError::EmptySignedMeterValueOrigin => {
                write!(f, "the signed meter value origin is mandatory and must be non-empty")
            }
        }
    }
}

impl std::error::Error for MeterValuesIngestError {}

/// The MeterValues reporting-cadence configuration (deployment
/// configuration; per-quantity, never hard-coded). The CSMS adapter
/// translates this into the charge point's reporting configuration at
/// provisioning (the exact OCPP variable names UNVERIFIED [S-124][S-125]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeterValuesCadenceConfig {
    /// The SoC sampled-value cadence in milliseconds (positive).
    pub soc_cadence_millis: i64,
    /// The active-power sampled-value cadence in milliseconds (positive).
    pub power_cadence_millis: i64,
    /// The energy-register sampled-value cadence in milliseconds
    /// (positive).
    pub energy_cadence_millis: i64,
    /// The SoC freshness limit in milliseconds (the staleness evaluation
    /// uses the P2.08 rule; the limit is deployment configuration).
    pub soc_freshness_limit_millis: i64,
    /// The active-power freshness limit in milliseconds.
    pub power_freshness_limit_millis: i64,
    /// The energy-register freshness limit in milliseconds.
    pub energy_freshness_limit_millis: i64,
}

impl MeterValuesCadenceConfig {
    /// Validates the configuration: every cadence and freshness limit is
    /// positive. No default exists — a missing value is a configuration
    /// error, not a silent fallback.
    pub fn validate(&self) -> Result<(), MeterValuesIngestError> {
        let checks = [
            ("soc_cadence_millis", self.soc_cadence_millis),
            ("power_cadence_millis", self.power_cadence_millis),
            ("energy_cadence_millis", self.energy_cadence_millis),
            ("soc_freshness_limit_millis", self.soc_freshness_limit_millis),
            ("power_freshness_limit_millis", self.power_freshness_limit_millis),
            ("energy_freshness_limit_millis", self.energy_freshness_limit_millis),
        ];
        for (field, value) in checks {
            if value <= 0 {
                return Err(MeterValuesIngestError::NonPositiveCadence { field, value });
            }
        }
        Ok(())
    }

    /// The freshness limit for one quantity (the arbitration and staleness
    /// consumers use this per-quantity lookup; an unmapped quantity is a
    /// programming error — the three configured quantities are the ones
    /// MeterValues ingestion tracks).
    pub fn freshness_limit_for(&self, quantity: TelemetryQuantity) -> Option<i64> {
        match quantity {
            TelemetryQuantity::Soc => Some(self.soc_freshness_limit_millis),
            TelemetryQuantity::ActivePower => Some(self.power_freshness_limit_millis),
            TelemetryQuantity::Energy => Some(self.energy_freshness_limit_millis),
            _ => None,
        }
    }

    /// The cadence for one quantity.
    pub fn cadence_for(&self, quantity: TelemetryQuantity) -> Option<i64> {
        match quantity {
            TelemetryQuantity::Soc => Some(self.soc_cadence_millis),
            TelemetryQuantity::ActivePower => Some(self.power_cadence_millis),
            TelemetryQuantity::Energy => Some(self.energy_cadence_millis),
            _ => None,
        }
    }
}

/// The adapter-boundary quality mapping for MeterValues source-side codes
/// (the P2.11 provider rule in application form: every code the adapter
/// can report is DECLARED with the one canonical state it maps to —
/// one of the frozen six; the vocabulary is consumed, never redefined).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeterValuesQualityMapping {
    /// What a sampled value with NO source-side quality code maps to (the
    /// declared assignment for a plain measured reading — typically
    /// `GOOD`; a legitimate measured-value assignment, not a conversion).
    no_source_code_quality: TelemetryQuality,
    /// The declared per-code mappings (BTreeMap — deterministic).
    by_code: BTreeMap<String, TelemetryQuality>,
}

impl MeterValuesQualityMapping {
    /// Declares the mapping. Refuses a mapping whose per-code targets are
    /// not among the frozen six (impossible by the type, but the
    /// constructor is the single declaration seam) — and this is the
    /// place a composition root declares its adapter's codes.
    pub fn new(
        no_source_code_quality: TelemetryQuality,
        by_code: BTreeMap<String, TelemetryQuality>,
    ) -> Self {
        Self {
            no_source_code_quality,
            by_code,
        }
    }

    /// The declared no-code assignment.
    pub fn no_source_code_quality(&self) -> TelemetryQuality {
        self.no_source_code_quality
    }

    /// The declared mapping for one code, when declared.
    pub fn canonical_for(&self, source_code: &str) -> Option<TelemetryQuality> {
        self.by_code.get(source_code).copied()
    }
}

/// The adapter-boundary quality assignment for one MeterValues sampled
/// value (the non-silencing rule, in one pure function):
///
/// - an undeclared source-side code is a named error (never a default);
/// - the mapped state is the declared one;
/// - when the value is stale (age > the quantity's freshness limit, the
///   P2.08 rule — the caller supplies the verdict), the mapped state is
///   held-or-lowered to `STALE`: `SUSPECT`/`BAD` degradation survives,
///   `GOOD` becomes `STALE`;
/// - the output is exactly one of the frozen six states — no seventh
///   state exists.
pub fn sampled_value_quality(
    mapping: &MeterValuesQualityMapping,
    source_code: Option<&str>,
    stale: bool,
) -> Result<TelemetryQuality, MeterValuesIngestError> {
    let mapped = match source_code {
        Some(code) => mapping.canonical_for(code).ok_or_else(|| {
            MeterValuesIngestError::UndeclaredSourceCode {
                source_code: code.to_string(),
            }
        })?,
        None => mapping.no_source_code_quality(),
    };
    Ok(match (mapped, stale) {
        // Degradation survives staleness (hold-or-lower; never upgrade).
        (TelemetryQuality::Suspect, true) => TelemetryQuality::Suspect,
        (TelemetryQuality::Bad, true) => TelemetryQuality::Bad,
        (_, true) => TelemetryQuality::Stale,
        (other, false) => other,
    })
}

/// The measurand mapping: the OCPP sampled-value measurand spellings the
/// adapter decodes onto the canonical telemetry quantities (SECONDARY-
/// SOURCED [S-124], UNVERIFIED against the OCA text — the recorded EV set;
/// an unknown measurand is a named error, never a default quantity).
pub fn sampled_value_quantity(measurand: &str) -> Result<TelemetryQuantity, MeterValuesIngestError> {
    match measurand {
        "SoC" => Ok(TelemetryQuantity::Soc),
        "Power.Active.Import" => Ok(TelemetryQuantity::ActivePower),
        "Energy.Active.Import.Register" => Ok(TelemetryQuantity::Energy),
        other => Err(MeterValuesIngestError::UndeclaredSourceCode {
            source_code: other.to_string(),
        }),
    }
}

/// One signed meter value, as the adapter delivered it (OPAQUE evidence
/// data — the verbatim payload is never parsed, checked, or verified
/// here; see [`SIGNED_METER_VALUE_SEAM_STATEMENT`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedMeterValueRecord {
    /// The verbatim signed-meter-value payload as the adapter received it
    /// (opaque; non-empty).
    pub verbatim_payload: String,
    /// The declared origin (adapter/station identity; non-empty).
    pub origin: String,
    /// When the platform received it (UTC ms).
    pub received_at_unix_ms: i64,
    /// The meter reading the payload relates to, when the adapter
    /// extracted one (canonical MW for power quantities is NOT assumed —
    /// the field is the raw declared value and its declared measurand
    /// spelling, carried verbatim for the evidence trail).
    pub related_measurand: Option<String>,
}

impl SignedMeterValueRecord {
    /// Validates the record: non-empty payload and origin. The payload is
    /// NOT parsed — an opaque evidence record.
    pub fn new(
        verbatim_payload: String,
        origin: String,
        received_at_unix_ms: i64,
        related_measurand: Option<String>,
    ) -> Result<Self, MeterValuesIngestError> {
        if verbatim_payload.trim().is_empty() {
            return Err(MeterValuesIngestError::EmptySignedMeterValuePayload);
        }
        if origin.trim().is_empty() {
            return Err(MeterValuesIngestError::EmptySignedMeterValueOrigin);
        }
        if let Some(measurand) = &related_measurand {
            if measurand.trim().is_empty() {
                return Err(MeterValuesIngestError::EmptySignedMeterValueOrigin);
            }
        }
        Ok(Self {
            verbatim_payload,
            origin,
            received_at_unix_ms,
            related_measurand,
        })
    }
}

/// The signed-meter-value sink: the hook the evidence store (or the
/// adapter's store-and-forward buffer) implements. The hook RECEIVES the
/// opaque record; it never verifies, never annotates, and never drops a
/// record silently (a failed write is an `Err` the caller counts and
/// retries — the no-silent-skip rule).
pub trait SignedMeterValueSink {
    /// Records one signed meter value for the evidence trail.
    fn record_signed_meter_value(
        &self,
        record: &SignedMeterValueRecord,
    ) -> Result<(), MeterValuesIngestError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping() -> MeterValuesQualityMapping {
        // The recorded EV adapter mapping used in the tests: a plain
        // measured reading (no code) is GOOD; the degradation codes the
        // adapter declares are non-GOOD (never a silent upgrade).
        MeterValuesQualityMapping::new(
            TelemetryQuality::Good,
            BTreeMap::from([
                ("OUT_OF_RANGE".to_string(), TelemetryQuality::Suspect),
                ("METER_FAULT".to_string(), TelemetryQuality::Bad),
                ("COMM_LOSS".to_string(), TelemetryQuality::Stale),
            ]),
        )
    }

    #[test]
    fn the_seam_statement_carries_the_never_verify_never_claim_rules() {
        assert!(SIGNED_METER_VALUE_SEAM_STATEMENT.contains("NEVER verifies a signature"));
        assert!(SIGNED_METER_VALUE_SEAM_STATEMENT.contains("NEVER claims Eichrecht"));
        assert!(SIGNED_METER_VALUE_SEAM_STATEMENT.contains("UNVERIFIED"));
    }

    #[test]
    fn cadence_configuration_is_per_quantity_and_refuses_non_positive_values() {
        let cfg = MeterValuesCadenceConfig {
            soc_cadence_millis: 60_000,
            power_cadence_millis: 10_000,
            energy_cadence_millis: 900_000,
            soc_freshness_limit_millis: 300_000,
            power_freshness_limit_millis: 60_000,
            energy_freshness_limit_millis: 3_600_000,
        };
        assert!(cfg.validate().is_ok());
        assert_eq!(
            cfg.freshness_limit_for(TelemetryQuantity::Soc),
            Some(300_000)
        );
        assert_eq!(
            cfg.cadence_for(TelemetryQuantity::Energy),
            Some(900_000)
        );
        // Quantities MeterValues ingestion does not track return None.
        assert_eq!(cfg.freshness_limit_for(TelemetryQuantity::Availability), None);
        // No default: every value must be positive.
        let mut bad = cfg.clone();
        bad.soc_cadence_millis = 0;
        assert_eq!(
            bad.validate(),
            Err(MeterValuesIngestError::NonPositiveCadence {
                field: "soc_cadence_millis",
                value: 0
            })
        );
        let mut bad = cfg;
        bad.energy_freshness_limit_millis = -1;
        assert!(bad.validate().is_err());
    }

    #[test]
    fn an_undeclared_source_code_is_a_named_error_never_a_default() {
        let err = sampled_value_quality(&mapping(), Some("MYSTERY_CODE"), false).unwrap_err();
        assert_eq!(
            err,
            MeterValuesIngestError::UndeclaredSourceCode {
                source_code: "MYSTERY_CODE".to_string()
            }
        );
    }

    #[test]
    fn a_plain_measured_value_maps_to_the_declared_no_code_state() {
        let q = sampled_value_quality(&mapping(), None, false).unwrap();
        assert_eq!(q, TelemetryQuality::Good);
    }

    #[test]
    fn degradation_codes_survive_staleness_and_never_upgrade() {
        // SUSPECT stays SUSPECT when stale; BAD stays BAD; a stale GOOD
        // becomes STALE. No seventh state, no upgrade anywhere.
        assert_eq!(
            sampled_value_quality(&mapping(), Some("OUT_OF_RANGE"), true).unwrap(),
            TelemetryQuality::Suspect
        );
        assert_eq!(
            sampled_value_quality(&mapping(), Some("METER_FAULT"), true).unwrap(),
            TelemetryQuality::Bad
        );
        assert_eq!(
            sampled_value_quality(&mapping(), None, true).unwrap(),
            TelemetryQuality::Stale
        );
        // Fresh mapped states pass through unchanged.
        assert_eq!(
            sampled_value_quality(&mapping(), Some("OUT_OF_RANGE"), false).unwrap(),
            TelemetryQuality::Suspect
        );
        assert_eq!(
            sampled_value_quality(&mapping(), Some("COMM_LOSS"), false).unwrap(),
            TelemetryQuality::Stale
        );
    }

    #[test]
    fn every_mapped_state_is_one_of_the_frozen_six() {
        // The vocabulary is consumed, never redefined: whatever the
        // mapping declares, the output set is the frozen six.
        for code in [None, Some("OUT_OF_RANGE"), Some("METER_FAULT"), Some("COMM_LOSS")] {
            for stale in [false, true] {
                let q = sampled_value_quality(&mapping(), code, stale).unwrap();
                assert!(TelemetryQuality::ALL.contains(&q));
            }
        }
    }

    #[test]
    fn the_measurand_mapping_covers_the_recorded_ev_set() {
        assert_eq!(
            sampled_value_quantity("SoC").unwrap(),
            TelemetryQuantity::Soc
        );
        assert_eq!(
            sampled_value_quantity("Power.Active.Import").unwrap(),
            TelemetryQuantity::ActivePower
        );
        assert_eq!(
            sampled_value_quantity("Energy.Active.Import.Register").unwrap(),
            TelemetryQuantity::Energy
        );
        // An unknown measurand is a named error, never a default quantity.
        assert!(sampled_value_quantity("Voltage").is_err());
    }

    #[test]
    fn a_signed_meter_value_record_is_opaque_and_refuses_empty_payloads() {
        let record = SignedMeterValueRecord::new(
            "MEIER0SIGNEDPAYLOAD==".to_string(),
            "cs-depot-01".to_string(),
            1_700_000_000_000,
            Some("Energy.Active.Import.Register".to_string()),
        )
        .unwrap();
        assert_eq!(record.verbatim_payload, "MEIER0SIGNEDPAYLOAD==");
        assert_eq!(record.related_measurand.as_deref(), Some("Energy.Active.Import.Register"));
        assert_eq!(
            SignedMeterValueRecord::new(
                "  ".to_string(),
                "cs".to_string(),
                0,
                None
            ),
            Err(MeterValuesIngestError::EmptySignedMeterValuePayload)
        );
        assert_eq!(
            SignedMeterValueRecord::new(
                "payload".to_string(),
                String::new(),
                0,
                None
            ),
            Err(MeterValuesIngestError::EmptySignedMeterValueOrigin)
        );
    }

    /// A counting fake sink (the hook's contract test: receives the
    /// record verbatim, never verifies).
    struct CountingSink {
        records: std::sync::Mutex<Vec<SignedMeterValueRecord>>,
        fail_after: Option<usize>,
    }

    impl CountingSink {
        fn new() -> Self {
            Self {
                records: std::sync::Mutex::new(Vec::new()),
                fail_after: None,
            }
        }
    }

    impl SignedMeterValueSink for CountingSink {
        fn record_signed_meter_value(
            &self,
            record: &SignedMeterValueRecord,
        ) -> Result<(), MeterValuesIngestError> {
            let mut records = self.records.lock().unwrap();
            if let Some(limit) = self.fail_after {
                if records.len() >= limit {
                    // A failed write is an Err the caller counts and
                    // retries — the no-silent-skip rule.
                    return Err(MeterValuesIngestError::EmptySignedMeterValueOrigin);
                }
            }
            records.push(record.clone());
            Ok(())
        }
    }

    #[test]
    fn the_sink_receives_the_opaque_record_and_fails_loudly() {
        let sink = CountingSink::new();
        let record = SignedMeterValueRecord::new(
            "SIG".to_string(),
            "cs-depot-01".to_string(),
            1_700_000_000_000,
            None,
        )
        .unwrap();
        sink.record_signed_meter_value(&record).unwrap();
        assert_eq!(sink.records.lock().unwrap().len(), 1);
        assert_eq!(sink.records.lock().unwrap()[0].verbatim_payload, "SIG");
        // The failure path is a named Err, never a silent drop.
        let failing = CountingSink {
            records: std::sync::Mutex::new(Vec::new()),
            fail_after: Some(0),
        };
        assert!(failing.record_signed_meter_value(&record).is_err());
    }
}
