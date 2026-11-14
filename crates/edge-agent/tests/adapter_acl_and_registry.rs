// P6.12 — Anti-corruption layer and adapter-registry tests (ledger P6.12;
// test file written, NOT run — the loop bans in
// `ledger/phase-06-edge-protocol/ENTRY_NOTE.md` hold. The human build/test
// pass runs these).
//
// Coverage:
//
// 1. **Canonical conformance (§10.2)** — MW/MVAr/MWh/SOC-fraction outputs,
//    P > 0 = export (an import-positive native sign is negated EXPLICITLY),
//    the declared reactive convention travels with the value (never a
//    silent flip).
// 2. **Canonical conformance (§10.4)** — exactly one canonical quality per
//    sample; the named table (GOOD/BAD/STALE + the §10.4 richer states
//    through `map_source_state`); no default quality (absent and unknown
//    codes are named errors); native `MISSING` refused (gap markers are
//    platform-derived, P2.04 D-T6); the original code retained as
//    diagnostic metadata.
// 3. **Time (§10.3)** — UTC-only event times; non-UTC refused, never
//    converted; event_time/ingest_time distinct and ordered.
// 4. **Unknown/unmappable fail-closed** — unmapped point, unknown unit,
//    unit/quantity mismatch, non-finite value: each a named error.
// 5. **Registry allow/deny** — unknown id denied; an allowlisted-but-PLANNED
//    id refused with a named reason (exercised via the `register` seam —
//    `openems-rest` itself is IMPLEMENTED in the DEFAULT table since P6.10);
//    `generic-rest` RESERVED and never constructible; an `Implemented`
//    entry constructs through its factory (the P6.10 change is a table edit).

use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::IngestTime;
use flexcore_contracts::units::{ActivePowerMW, ReactivePowerMVAr, ReactiveSignConvention, StateOfCharge};
use flexcore_edge_agent::adapters::acl::{
    AclError, AntiCorruptionLayer, NativeSign, NativeTimestamp, NativeUnit, PointMapping,
};
use flexcore_edge_agent::adapters::port::{EmsNativeReading, PointKey};
use flexcore_edge_agent::adapters::registry::{AdapterRegistry, AdapterStatus, RegistryError};
use flexcore_ports::telemetry_source::{QuantityValue, TelemetryQuantity};

fn ingest() -> IngestTime {
    IngestTime::from_unix_millis(2_000).expect("valid")
}

fn reading(
    component: &str,
    channel: &str,
    unit: NativeUnit,
    raw: f64,
    quality: Option<&str>,
    sequence: u64,
) -> EmsNativeReading {
    EmsNativeReading {
        point: PointKey::new(component, channel),
        native_unit: unit,
        raw_value: raw,
        native_quality_code: quality.map(str::to_string),
        event_time: NativeTimestamp::UnixMillis(1_000),
        source_sequence: sequence,
    }
}

fn acl() -> AntiCorruptionLayer {
    AntiCorruptionLayer::new(
        vec![
            // Active power in W, already export-positive (canonical rule
            // holds without conversion).
            PointMapping {
                point: PointKey::new("ess0", "ActivePower"),
                asset_key: "ess-01".to_string(),
                quantity: TelemetryQuantity::ActivePower,
                native_unit: NativeUnit::W,
                native_sign: Some(NativeSign::ExportPositive),
                reactive_source_convention: None,
            },
            // Reactive power in var under the declared
            // consumption-positive convention.
            PointMapping {
                point: PointKey::new("ess0", "ReactivePower"),
                asset_key: "ess-01".to_string(),
                quantity: TelemetryQuantity::ReactivePower,
                native_unit: NativeUnit::Var,
                native_sign: None,
                reactive_source_convention: Some(ReactiveSignConvention::ConsumptionPositive),
            },
            // SOC as a vendor percent.
            PointMapping {
                point: PointKey::new("ess0", "Soc"),
                asset_key: "ess-01".to_string(),
                quantity: TelemetryQuantity::Soc,
                native_unit: NativeUnit::SocPercent,
                native_sign: None,
                reactive_source_convention: None,
            },
            // Energy in kWh.
            PointMapping {
                point: PointKey::new("meter0", "EnergyImport"),
                asset_key: "meter-01".to_string(),
                quantity: TelemetryQuantity::Energy,
                native_unit: NativeUnit::Kwh,
                native_sign: None,
                reactive_source_convention: None,
            },
        ],
        "edge:pilot-site-A:openems-rest",
        // The honest unmapped inventory: these EMS channels are KNOWN and
        // deliberately not mapped (P6.12: every unmapped field is listed).
        vec![
            PointKey::new("ess0", "InternalTemperature"),
            PointKey::new("meter0", "PhaseCurrentL3"),
        ],
    )
    .expect("valid mapping table")
}

// --- 1. Canonical conformance: units and signs (§10.2) ------------------------

#[test]
fn active_power_converts_w_to_mw_under_the_export_rule() {
    let sample = acl()
        .map_reading(
            &reading("ess0", "ActivePower", NativeUnit::W, 2_500_000.0, Some("GOOD"), 11),
            ingest(),
        )
        .expect("maps");
    assert_eq!(sample.quantity(), TelemetryQuantity::ActivePower);
    match sample.value() {
        QuantityValue::Power(power) => {
            let _: &ActivePowerMW = power; // the canonical MW newtype, P0.07
            assert_eq!(power.value(), 2.5);
        }
        other => panic!("expected a canonical MW power value, got {other:?}"),
    }
}

#[test]
fn import_positive_native_sign_is_negated_explicitly() {
    let import_acl = AntiCorruptionLayer::new(
        vec![PointMapping {
            point: PointKey::new("pv0", "ActivePower"),
            asset_key: "pv-01".to_string(),
            quantity: TelemetryQuantity::ActivePower,
            native_unit: NativeUnit::Kw,
            native_sign: Some(NativeSign::ImportPositive),
            reactive_source_convention: None,
        }],
        "edge:pilot-site-A:openems-rest",
        vec![],
    )
    .expect("valid");
    // -4 000 kW in an import-positive world = 4 000 kW export → +4 MW.
    let sample = import_acl
        .map_reading(
            &reading("pv0", "ActivePower", NativeUnit::Kw, -4_000.0, Some("GOOD"), 1),
            ingest(),
        )
        .expect("maps");
    match sample.value() {
        QuantityValue::Power(power) => assert_eq!(power.value(), 4.0),
        other => panic!("expected a power value, got {other:?}"),
    }
}

#[test]
fn reactive_value_travels_with_its_declared_convention() {
    let sample = acl()
        .map_reading(
            &reading("ess0", "ReactivePower", NativeUnit::Var, -1_500_000.0, Some("GOOD"), 4),
            ingest(),
        )
        .expect("maps");
    match sample.value() {
        QuantityValue::Reactive { power, convention } => {
            let _: &ReactivePowerMVAr = power;
            assert_eq!(power.value(), -1.5);
            // The DECLARED convention travels on the value (P0.07 §3) — the
            // sign is never interpreted or flipped by the ACL.
            assert_eq!(*convention, ReactiveSignConvention::ConsumptionPositive);
        }
        other => panic!("expected a reactive value with a convention, got {other:?}"),
    }
}

#[test]
fn soc_percent_becomes_a_canonical_fraction() {
    let sample = acl()
        .map_reading(
            &reading("ess0", "Soc", NativeUnit::SocPercent, 62.5, Some("GOOD"), 2),
            ingest(),
        )
        .expect("maps");
    match sample.value() {
        QuantityValue::Fraction(soc) => {
            let _: &StateOfCharge = soc;
            assert_eq!(soc.value(), 0.625);
        }
        other => panic!("expected a canonical SOC fraction, got {other:?}"),
    }
}

#[test]
fn energy_kwh_becomes_mwh() {
    let sample = acl()
        .map_reading(
            &reading("meter0", "EnergyImport", NativeUnit::Kwh, 7_500.0, Some("GOOD"), 9),
            ingest(),
        )
        .expect("maps");
    match sample.value() {
        QuantityValue::Energy(energy) => assert_eq!(energy.value(), 7.5),
        other => panic!("expected a canonical MWh energy value, got {other:?}"),
    }
}

#[test]
fn sample_shape_matches_the_p204_record_contract() {
    let sample = acl()
        .map_reading(
            &reading("ess0", "ActivePower", NativeUnit::W, 1_000_000.0, Some("OUT_OF_RANGE"), 21),
            ingest(),
        )
        .expect("maps");
    // One quantity, one asset key, one event_time, distinct ingest_time
    // (P2.04 D-T2 shape, carried edge-side).
    assert_eq!(sample.quantity(), TelemetryQuantity::ActivePower);
    assert_eq!(sample.asset_key(), "ess-01");
    assert_eq!(sample.event_time().unix_millis(), 1_000);
    assert_eq!(sample.ingest_time().unix_millis(), 2_000);
    assert_eq!(sample.source_sequence(), 21);
    // Exactly one canonical quality, with the source code retained as
    // diagnostic metadata (P0.08 §3).
    assert_eq!(sample.quality(), TelemetryQuality::Suspect);
    assert_eq!(sample.source_quality_code(), Some("OUT_OF_RANGE"));
    // Source identity: the edge prefix + the point; never `flexcore:`.
    assert!(sample.source_id().starts_with("edge:pilot-site-A:openems-rest/"));
    assert!(!sample.source_id().starts_with("flexcore:"));
}

// --- 2. Quality conformance (§10.4) --------------------------------------------

#[test]
fn the_named_quality_table_covers_good_bad_stale_and_the_richer_states() {
    let acl = acl();
    let cases: &[(&str, TelemetryQuality)] = &[
        ("GOOD", TelemetryQuality::Good),
        ("BAD", TelemetryQuality::Bad),
        ("STALE", TelemetryQuality::Stale),
        ("SUBSTITUTED", TelemetryQuality::Estimated),
        ("OUT_OF_RANGE", TelemetryQuality::Suspect),
        ("UNSYNCHRONIZED", TelemetryQuality::Suspect),
        ("COMMUNICATION_LOSS", TelemetryQuality::Stale),
        ("UNRELIABLE", TelemetryQuality::Suspect),
    ];
    for (code, expected) in cases {
        assert_eq!(acl.map_quality_code(Some(code)).unwrap(), *expected, "code {code}");
    }
}

#[test]
fn no_default_quality_exists() {
    let acl = acl();
    assert_eq!(acl.map_quality_code(None), Err(AclError::QualityNotDeclared));
    assert_eq!(
        acl.map_quality_code(Some("UNKNOWN_VENDOR_FLAG")),
        Err(AclError::UnknownQualityCode {
            code: "UNKNOWN_VENDOR_FLAG".to_string()
        })
    );
    assert_eq!(
        acl.map_quality_code(Some("MISSING")),
        Err(AclError::NativeMissingRequiresGapMarker)
    );
}

// --- 3. Time conformance (§10.3) -----------------------------------------------

#[test]
fn utc_event_times_map_and_non_utc_are_refused_not_converted() {
    let good = NativeTimestamp::UtcRfc3339("2026-11-12T09:30:00Z".to_string());
    let event = good.to_event_time().expect("UTC maps");
    assert_eq!(event.unix_millis(), 1_780_000_000_000);

    let shifted = NativeTimestamp::UtcRfc3339("2026-11-12T10:30:00+01:00".to_string());
    assert!(matches!(
        shifted.to_event_time(),
        Err(AclError::NonUtcNativeTimestamp { .. })
    ));

    let malformed = NativeTimestamp::UtcRfc3339("not-a-timestamp".to_string());
    assert!(matches!(
        malformed.to_event_time(),
        Err(AclError::NonUtcNativeTimestamp { .. })
    ));
}

#[test]
fn ingest_before_event_is_rejected_by_the_sample_constructor() {
    let result = acl().map_reading(
        &reading("ess0", "ActivePower", NativeUnit::W, 1.0, Some("GOOD"), 1),
        IngestTime::from_unix_millis(500).expect("valid"),
    );
    assert!(matches!(result, Err(AclError::Sample(_))));
}

// --- 4. Unknown / unmappable fail-closed ----------------------------------------

#[test]
fn unmapped_point_is_a_named_error_and_is_listed() {
    let acl = acl();
    let result = acl.map_reading(
        &reading("ess0", "InternalTemperature", NativeUnit::Kw, 1.0, Some("GOOD"), 1),
        ingest(),
    );
    assert_eq!(
        result,
        Err(AclError::PointUnmapped {
            point: "ess0/InternalTemperature".to_string()
        })
    );
    // And the same point sits in the declared-unmapped inventory (the
    // mapping-table review lists it — it is never silently dropped).
    assert!(acl.is_declared_unmapped(&PointKey::new("ess0", "InternalTemperature")));
    assert!(acl.is_declared_unmapped(&PointKey::new("meter0", "PhaseCurrentL3")));
    assert_eq!(acl.unmapped_points().len(), 2);
}

#[test]
fn unknown_unit_string_and_unit_quantity_mismatch_fail() {
    assert_eq!(
        NativeUnit::parse("gallons"),
        Err(AclError::UnknownNativeUnit {
            code: "gallons".to_string()
        })
    );
    // A mapping whose unit cannot feed its quantity is refused at
    // construction (kWh against ActivePower).
    assert_eq!(
        AntiCorruptionLayer::new(
            vec![PointMapping {
                point: PointKey::new("ess0", "ActivePower"),
                asset_key: "ess-01".to_string(),
                quantity: TelemetryQuantity::ActivePower,
                native_unit: NativeUnit::Kwh,
                native_sign: Some(NativeSign::ExportPositive),
                reactive_source_convention: None,
            }],
            "edge:pilot-site-A:openems-rest",
            vec![],
        ),
        Err(AclError::UnitQuantityMismatch {
            unit: NativeUnit::Kwh,
            quantity: TelemetryQuantity::ActivePower
        })
    );
}

#[test]
fn non_finite_raw_value_is_refused() {
    let result = acl().map_reading(
        &reading("ess0", "ActivePower", NativeUnit::W, f64::INFINITY, Some("GOOD"), 1),
        ingest(),
    );
    assert_eq!(result, Err(AclError::NonFiniteValue));
}

// --- 5. Registry allow / deny ----------------------------------------------------

#[test]
fn registry_allows_openems_rest_and_the_recorded_reserved_slot() {
    // P6.10 coordinator follow-up: the DEFAULT table carries `openems-rest`
    // as Implemented (P6.13's real-protocol fault-injection pass remains
    // open and is NOT implied by this status).
    let registry = AdapterRegistry::with_default_allowlist();
    assert_eq!(
        registry.status_of("openems-rest"),
        Ok(AdapterStatus::Implemented)
    );
    assert_eq!(registry.status_of("generic-rest"), Ok(AdapterStatus::Reserved));
    assert_eq!(
        registry.status_of("generic-rest").unwrap().describe(),
        "RESERVED (not implemented; placeholder slot)"
    );
}

#[test]
fn registry_denies_unknown_ids_and_unimplemented_allowlisted_ids() {
    let mut registry = AdapterRegistry::with_default_allowlist();
    // Unknown id → NotAllowlisted.
    assert_eq!(
        registry.status_of("modbus-tcp"),
        Err(RegistryError::NotAllowlisted {
            id: "modbus-tcp".to_string()
        })
    );
    // Allowlisted-but-PLANNED → NotYetImplemented naming the wave; the
    // factory must never run for a refused id. Since the P6.10 DEFAULT
    // flip, the PLANNED arm is exercised through the documented
    // `register` seam (`openems-rest` itself is now Implemented).
    registry.register(
        "legacy-planned",
        AdapterStatus::Planned {
            implementing_wave: "P6.10/P6.13",
        },
    );
    let error = registry
        .construct(
            "legacy-planned",
            &|| -> Result<Box<dyn flexcore_edge_agent::adapters::port::EmsAdapter>, String> {
                panic!("the factory must never run for a refused id")
            },
        )
        .unwrap_err();
    assert!(error.to_string().contains("P6.10/P6.13"));
}

#[test]
fn implemented_registry_entry_constructs_through_the_factory() {
    // The Implemented arm is proven with a stand-in so the P6.10/P6.13
    // change is a registry-table edit plus a constructor (the decision doc
    // records this as the extension path).
    struct StandInAdapter;

    impl flexcore_edge_agent::adapters::port::EmsAdapter for StandInAdapter {
        fn id(&self) -> &str {
            "stand-in"
        }
        fn capabilities(&self) -> &flexcore_edge_agent::adapters::port::AdapterCapabilities {
            unreachable!("never read in this test")
        }
        fn discover(
            &mut self,
        ) -> Result<Vec<flexcore_edge_agent::adapters::port::ChannelDescriptor>, flexcore_edge_agent::adapters::port::AdapterError>
        {
            unreachable!("never read in this test")
        }
        fn read(
            &mut self,
            _point: &PointKey,
        ) -> Result<EmsNativeReading, flexcore_edge_agent::adapters::port::AdapterError> {
            unreachable!("never read in this test")
        }
        fn health(&self) -> flexcore_edge_agent::adapters::port::AdapterHealth {
            unreachable!("never read in this test")
        }
    }

    let mut registry = AdapterRegistry::with_default_allowlist();
    registry.register("stand-in", AdapterStatus::Implemented);
    let adapter = registry
        .construct(
            "stand-in",
            &|| Ok(Box::new(StandInAdapter) as Box<dyn flexcore_edge_agent::adapters::port::EmsAdapter>),
        )
        .expect("constructs");
    assert_eq!(adapter.id(), "stand-in");
}
