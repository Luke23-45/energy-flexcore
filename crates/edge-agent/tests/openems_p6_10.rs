// P6.10 — OpenEMS integration: EdgeConfig discovery and channel mapping —
// integration tests (ledger P6.10; test file written, NOT run — the loop
// bans in `ledger/phase-06-edge-protocol/ENTRY_NOTE.md` hold: nothing
// built, no tests executed. The human build/test pass runs these).
//
// Coverage (ledger P6.10 steps 1, 2, 4 + acceptance criteria):
//
// 1. **EdgeConfig parser against a realistic fixture** — a simulated PV +
//    battery EdgeConfig per the OpenEMS Getting Started shape (FIXTURE
//    JSON, labelled; the real lab EdgeConfig is the P6.11 lab run's input).
//    Every discovered channel lands in exactly ONE register class: MAPPED /
//    EXPLICITLY_UNMAPPED (with reason) / UNMAPPED (with reason) / AMBIGUOUS.
// 2. **No unit guessing** — a channel whose source-declared unit is
//    unnameable and differs from the profile spelling is AMBIGUOUS, never
//    mapped; a channel with no profile entry is UNMAPPED with that reason.
// 3. **Versioned profile load** — the shipped
//    `config/openems-mapping-profile.v1.json` parses with version 1; a
//    missing file, a wrong version, or a refused envelope is a named error.
// 4. **Fail-closed mapping rules** — duplicate profile entries, unknown
//    unit spellings, and a missing active-power sign declaration are all
//    AMBIGUOUS register entries, never guesses; a fresh reading for an
//    entry with no declared quality code carries NO quality and the ACL
//    refuses it (no default quality, P0.08).
// 5. **Read honesty** — reads refuse before discovery; unmapped points are
//    named errors; a stale feed is stamped with the declared `STALE` code.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::IngestTime;
use flexcore_edge_agent::adapters::acl::{
    AntiCorruptionLayer, NativeUnit, PointMapping,
};
use flexcore_edge_agent::adapters::openems::{
    ChannelMappingStatus, OpenEmsRestAdapter, OpenemsError, OpenemsMappingProfile,
    OpenemsRestTransport, RestResponse, OPENEMS_CHANNEL_PATH_PREFIX, OPENEMS_EDGE_CONFIG_PATH,
    OPENEMS_REST_ADAPTER_ID, OPENEMS_STALE_CODE, PROFILE_ID, SUPPORTED_PROFILE_VERSION,
};
use flexcore_edge_agent::adapters::port::{AdapterError, EmsAdapter, PointKey};
use flexcore_ports::fakes::FixedClock;
use flexcore_ports::telemetry_source::TelemetryQuantity;

const NOW_MILLIS: i64 = 1_700_000_000_000;
const FRESH_AGE_MILLIS: i64 = 1_000;

// --- The fixture EdgeConfig -----------------------------------------------------
//
// FIXTURE (labelled): a realistic EdgeConfig JSON for a simulated PV +
// battery site per the OpenEMS Getting Started shape. UNVERIFIED-ASSUMPTION:
// the field spellings (`components`, `factoryId`, `alias`, `channels`,
// `unit`) follow the recorded OpenEMS documentation shape (RESEARCH.md §1;
// report §27 OpenEMS references) and were NEVER checked against a live
// Edge — the P6.11 lab run verifies them.

const FIXTURE_EDGE_CONFIG: &str = r#"{
  "components": {
    "_meta": {
      "factoryId": "Core.Meta",
      "alias": "OpenEMS Edge metadata",
      "channels": {
        "ComponentState": { "unit": "", "text": "Component state" }
      }
    },
    "ess0": {
      "factoryId": "Simulator.Ess.Direct.Symmetric",
      "alias": "Lab battery ESS",
      "channels": {
        "ActivePower": { "unit": "W", "text": "Active Power" },
        "ReactivePower": { "unit": "var", "text": "Reactive Power" },
        "Soc": { "unit": "%", "text": "State of Charge" },
        "AllowedChargePower": { "unit": "W", "text": "Allowed Charge Power" },
        "AllowedDischargePower": { "unit": "W", "text": "Allowed Discharge Power" },
        "MaxApparentPower": { "unit": "VA", "text": "Max Apparent Power" }
      }
    },
    "pvInverter": {
      "factoryId": "Simulator.PvInverter.Direct",
      "alias": "Lab PV inverter",
      "channels": {
        "ActivePower": { "unit": "W", "text": "Active Power" },
        "ReactivePower": { "unit": "var", "text": "Reactive Power" }
      }
    },
    "meter0": {
      "factoryId": "Simulator.Meter.Direct",
      "alias": "Grid connection meter",
      "channels": {
        "ActivePower": { "unit": "W", "text": "Active Power" },
        "ReactivePower": { "unit": "var", "text": "Reactive Power" }
      }
    },
    "heatpump0": {
      "factoryId": "Simulator.HeatPump",
      "alias": "Lab heat pump",
      "channels": {
        "VolumeFlow": { "unit": "m3/h", "text": "Volume flow" }
      }
    }
  }
}"#;

/// FIXTURE (labelled): the per-channel values the test double serves. The
/// response shape `{"value": <number>, "timestamp": <unix millis>}` is an
/// UNVERIFIED-ASSUMPTION (RESEARCH.md §1 records no REST response shape).
struct ChannelFixture {
    value: f64,
    timestamp_millis: i64,
}

fn fixture_channels() -> HashMap<String, ChannelFixture> {
    let fresh = |value: f64| ChannelFixture {
        value,
        timestamp_millis: NOW_MILLIS - FRESH_AGE_MILLIS,
    };
    HashMap::from([
        ("ess0/ActivePower".to_string(), fresh(1_500.0)),
        ("ess0/ReactivePower".to_string(), fresh(200.0)),
        ("ess0/Soc".to_string(), fresh(87.0)),
        ("pvInverter/ActivePower".to_string(), fresh(900.0)),
        ("pvInverter/ReactivePower".to_string(), fresh(10.0)),
        ("meter0/ActivePower".to_string(), fresh(-450.0)),
        ("meter0/ReactivePower".to_string(), fresh(100.0)),
        ("heatpump0/VolumeFlow".to_string(), fresh(0.4)),
    ])
}

// --- The in-process test double ---------------------------------------------------
//
// A local test transport (NO sockets, NO external network): it serves the
// fixture EdgeConfig/channel JSON straight from memory. The trait it
// implements exposes GET only — a write call is not expressible.

struct FixtureTransport {
    state: Arc<Mutex<FixtureState>>,
}

struct FixtureState {
    config_json: String,
    channels: HashMap<String, ChannelFixture>,
    gets_served: u64,
}

impl FixtureTransport {
    fn new(config_json: &str, channels: HashMap<String, ChannelFixture>) -> Self {
        Self {
            state: Arc::new(Mutex::new(FixtureState {
                config_json: config_json.to_string(),
                channels,
                gets_served: 0,
            })),
        }
    }
}

impl OpenemsRestTransport for FixtureTransport {
    fn get(&mut self, path: &str) -> Result<RestResponse, OpenemsError> {
        let mut state = self.state.lock().expect("fixture state mutex");
        state.gets_served += 1;
        if path == OPENEMS_EDGE_CONFIG_PATH {
            return Ok(RestResponse {
                status: 200,
                body: state.config_json.clone(),
            });
        }
        if let Some(address) = path.strip_prefix(OPENEMS_CHANNEL_PATH_PREFIX) {
            return match state.channels.get(address) {
                Some(channel) => Ok(RestResponse {
                    status: 200,
                    body: format!(
                        "{{\"value\": {}, \"timestamp\": {}}}",
                        channel.value, channel.timestamp_millis
                    ),
                }),
                None => Ok(RestResponse {
                    status: 404,
                    body: "{}".to_string(),
                }),
            };
        }
        Ok(RestResponse {
            status: 404,
            body: "{}".to_string(),
        })
    }
}

// --- Profile helpers ---------------------------------------------------------------

fn shipped_profile_path() -> String {
    format!(
        "{}/config/openems-mapping-profile.v1.json",
        env!("CARGO_MANIFEST_DIR")
    )
}

/// Builds an inline profile JSON (FIXTURE) with the given mappings and
/// declared-unmapped entries, for the fail-closed variant cases.
fn profile_json(mappings: &str, declared_unmapped: &str) -> String {
    format!(
        r#"{{
        "profile": "{PROFILE_ID}",
        "version": {SUPPORTED_PROFILE_VERSION},
        "reactive_source_convention": "injection-positive",
        "cadence_millis": 5000,
        "stale_after_millis": 15000,
        "mappings": [{mappings}],
        "declared_unmapped": [{declared_unmapped}]
    }}"#
    )
}

fn adapter_over(profile: OpenemsMappingProfile, config_json: &str) -> OpenEmsRestAdapter {
    let transport = Box::new(FixtureTransport::new(config_json, fixture_channels()));
    OpenEmsRestAdapter::new(
        transport,
        profile,
        Arc::new(FixedClock::at_unix_millis(NOW_MILLIS)),
    )
    .expect("the adapter composes over a valid profile")
}

fn shipped_adapter() -> OpenEmsRestAdapter {
    let profile =
        OpenemsMappingProfile::from_path(shipped_profile_path()).expect("the shipped profile loads");
    adapter_over(profile, FIXTURE_EDGE_CONFIG)
}

fn count(register: &[flexcore_edge_agent::adapters::openems::ChannelMappingEntry], class: usize) -> usize {
    register
        .iter()
        .filter(|entry| match (entry.status.clone(), class) {
            (ChannelMappingStatus::Mapped, 0) => true,
            (ChannelMappingStatus::ExplicitlyUnmapped { .. }, 1) => true,
            (ChannelMappingStatus::Unmapped { .. }, 2) => true,
            (ChannelMappingStatus::Ambiguous { .. }, 3) => true,
            _ => false,
        })
        .count()
}

// --- 1. Full classification of the fixture EdgeConfig -------------------------------

#[test]
fn edge_config_fixture_fully_classifies_without_guessing() {
    let mut adapter = shipped_adapter();
    let descriptors = adapter.discover().expect("discovery succeeds");
    let register = adapter.mapping_register();

    // 12 discovered channels in the fixture; every one lands in exactly
    // one class, and the classes sum to the total.
    assert_eq!(register.len(), 12);
    assert_eq!(count(register, 0), 7, "mapped");
    assert_eq!(count(register, 1), 4, "explicitly unmapped with a reason");
    assert_eq!(count(register, 2), 1, "unmapped with a reason");
    assert_eq!(count(register, 3), 0, "ambiguous");
    assert_eq!(descriptors.len(), 7, "only mapped channels are descriptors");

    // The mapped set matches the shipped profile exactly (asset keys
    // included).
    let mapped_keys: Vec<String> = adapter
        .mapped_point_mappings()
        .iter()
        .map(|m| format!("{}/{} -> {}", m.point.component, m.point.channel, m.asset_key))
        .collect();
    for expected in [
        "ess0/ActivePower -> ess-01",
        "ess0/ReactivePower -> ess-01",
        "ess0/Soc -> ess-01",
        "pvInverter/ActivePower -> pv-01",
        "pvInverter/ReactivePower -> pv-01",
        "meter0/ActivePower -> grid-meter-01",
        "meter0/ReactivePower -> grid-meter-01",
    ] {
        assert!(
            mapped_keys.contains(&expected.to_string()),
            "expected mapping '{expected}' missing from {mapped_keys:?}"
        );
    }

    // The explicitly-unmapped inventory carries the profile's recorded
    // reasons (limit channels, metadata, static ratings — not telemetry).
    for point in [
        PointKey::new("_meta", "ComponentState"),
        PointKey::new("ess0", "AllowedChargePower"),
        PointKey::new("ess0", "AllowedDischargePower"),
        PointKey::new("ess0", "MaxApparentPower"),
    ] {
        let entry = register
            .iter()
            .find(|entry| entry.point == point)
            .unwrap_or_else(|| panic!("register entry for {point}"));
        assert!(
            matches!(entry.status, ChannelMappingStatus::ExplicitlyUnmapped { ref reason } if !reason.is_empty()),
            "explicitly unmapped with a reason: {entry:?}"
        );
    }

    // The heat-pump channel has NO profile entry: recorded UNMAPPED with
    // that reason — never mapped by guessing its unit ('m3/h' is not a
    // nameable unit and is never converted).
    let heatpump = register
        .iter()
        .find(|entry| entry.point == PointKey::new("heatpump0", "VolumeFlow"))
        .expect("heatpump register entry");
    assert!(
        matches!(heatpump.status, ChannelMappingStatus::Unmapped { ref reason }
            if reason.contains("no mapping-profile v1 entry")),
        "unmapped with a reason: {heatpump:?}"
    );
    assert_eq!(
        heatpump.source_declared_unit.as_deref(),
        Some("m3/h"),
        "the source-declared unit is retained as diagnostic, never acted on"
    );
    assert!(
        adapter
            .mapped_point_mappings()
            .iter()
            .all(|m| m.point != PointKey::new("heatpump0", "VolumeFlow")),
        "the unnameable-unit channel is never mapped"
    );
}

// --- 2. The shipped versioned profile ----------------------------------------------

#[test]
fn shipped_profile_loads_from_the_versioned_file() {
    let profile =
        OpenemsMappingProfile::from_path(shipped_profile_path()).expect("the shipped profile loads");
    assert_eq!(profile.version(), SUPPORTED_PROFILE_VERSION);
    assert_eq!(profile.mappings().len(), 7);
    assert_eq!(profile.declared_unmapped().len(), 4);
    assert!(
        !profile.assumptions().is_empty(),
        "the UNVERIFIED-ASSUMPTION register travels with the profile"
    );
    assert_eq!(
        profile.reactive_source_convention(),
        flexcore_contracts::units::ReactiveSignConvention::InjectionPositive
    );
    assert_eq!(profile.stale_after(), Duration::from_millis(15_000));
}

#[test]
fn missing_profile_file_is_a_named_error() {
    let error =
        OpenemsMappingProfile::from_path("no/such/profile-file.json").expect_err("refuses");
    assert!(matches!(error, OpenemsError::Io { .. }), "{error}");
    assert!(error.to_string().contains("mapping profile"));
}

#[test]
fn profile_with_a_wrong_version_is_refused() {
    // FIXTURE: a v2-shaped profile. A different version is a reviewed
    // migration, never an interpreted guess.
    let text = r#"{
        "profile": "openems-mapping-profile",
        "version": 2,
        "reactive_source_convention": "injection-positive",
        "cadence_millis": 5000,
        "stale_after_millis": 15000,
        "mappings": [{"component_id": "ess0", "channel_id": "Soc", "asset_key": "ess-01",
                      "quantity": "Soc", "native_unit": "%", "declared_quality_code": "GOOD"}]
    }"#;
    let error = OpenemsMappingProfile::from_json_str(text).expect_err("refuses");
    assert!(matches!(error, OpenemsError::ProfileEnvelope { .. }), "{error}");
    assert!(error.to_string().contains("version"));
}

// --- 3. Fail-closed mapping rules ---------------------------------------------------

#[test]
fn duplicate_profile_entries_are_ambiguous_never_mapped() {
    // FIXTURE: two profile entries for the SAME point — duplicate
    // declarations are ambiguous, never resolved by picking one.
    let profile =
        OpenemsMappingProfile::from_json_str(&profile_json(
            r#"{"component_id": "ess0", "channel_id": "Soc", "asset_key": "ess-01",
                "quantity": "Soc", "native_unit": "%", "declared_quality_code": "GOOD"},
               {"component_id": "ess0", "channel_id": "Soc", "asset_key": "ess-01-ALSO",
                "quantity": "Soc", "native_unit": "fraction", "declared_quality_code": "GOOD"}"#,
            "",
        ))
        .expect("the profile envelope is valid");
    let mut adapter = adapter_over(profile, FIXTURE_EDGE_CONFIG);
    adapter.discover().expect("discovery succeeds");
    let entry = adapter
        .mapping_register()
        .iter()
        .find(|entry| entry.point == PointKey::new("ess0", "Soc"))
        .expect("ess0/Soc register entry");
    assert!(
        matches!(entry.status, ChannelMappingStatus::Ambiguous { ref reason }
            if reason.contains("duplicate")),
        "duplicate profile entries are ambiguous: {entry:?}"
    );
    assert!(
        adapter
            .mapped_point_mappings()
            .iter()
            .all(|m| m.point != PointKey::new("ess0", "Soc")),
        "an ambiguous channel is never mapped"
    );
}

#[test]
fn unknown_unit_spelling_is_ambiguous_never_mapped() {
    let profile = OpenemsMappingProfile::from_json_str(&profile_json(
        r#"{"component_id": "ess0", "channel_id": "Soc", "asset_key": "ess-01",
            "quantity": "Soc", "native_unit": "flurb", "declared_quality_code": "GOOD"}"#,
        "",
    ))
    .expect("the profile envelope is valid");
    let mut adapter = adapter_over(profile, FIXTURE_EDGE_CONFIG);
    adapter.discover().expect("discovery succeeds");
    let entry = adapter
        .mapping_register()
        .iter()
        .find(|entry| entry.point == PointKey::new("ess0", "Soc"))
        .expect("register entry");
    assert!(
        matches!(entry.status, ChannelMappingStatus::Ambiguous { ref reason }
            if reason.contains("unknown native unit")),
        "an unnameable unit is flagged, never mapped: {entry:?}"
    );
}

#[test]
fn source_unit_conflict_is_ambiguous() {
    // The profile declares kvar; the EdgeConfig fixture declares 'var' for
    // ess0/ReactivePower: a conflict is flagged, never resolved by guessing.
    let profile = OpenemsMappingProfile::from_json_str(&profile_json(
        r#"{"component_id": "ess0", "channel_id": "ReactivePower", "asset_key": "ess-01",
            "quantity": "ReactivePower", "native_unit": "kvar", "declared_quality_code": "GOOD"}"#,
        "",
    ))
    .expect("valid");
    let mut adapter = adapter_over(profile, FIXTURE_EDGE_CONFIG);
    adapter.discover().expect("discovery succeeds");
    let entry = adapter
        .mapping_register()
        .iter()
        .find(|entry| entry.point == PointKey::new("ess0", "ReactivePower"))
        .expect("register entry");
    assert!(
        matches!(entry.status, ChannelMappingStatus::Ambiguous { ref reason }
            if reason.contains("conflicts")),
        "a unit conflict is ambiguous: {entry:?}"
    );
}

#[test]
fn unnameable_source_unit_that_differs_from_the_profile_is_ambiguous() {
    // The EdgeConfig declares 'm3/h'; the profile claims kWh — the source
    // unit is not nameable AND differs from the profile spelling: flagged.
    let profile = OpenemsMappingProfile::from_json_str(&profile_json(
        r#"{"component_id": "heatpump0", "channel_id": "VolumeFlow", "asset_key": "hp-01",
            "quantity": "Energy", "native_unit": "kWh", "declared_quality_code": "GOOD"}"#,
        "",
    ))
    .expect("valid");
    let mut adapter = adapter_over(profile, FIXTURE_EDGE_CONFIG);
    adapter.discover().expect("discovery succeeds");
    let entry = adapter
        .mapping_register()
        .iter()
        .find(|entry| entry.point == PointKey::new("heatpump0", "VolumeFlow"))
        .expect("register entry");
    assert!(
        matches!(entry.status, ChannelMappingStatus::Ambiguous { ref reason }
            if reason.contains("not in the named unit table")),
        "an unnameable conflicting source unit is ambiguous: {entry:?}"
    );
}

#[test]
fn active_power_without_a_declared_sign_is_ambiguous() {
    // P0.07 §2: the native sign meaning is a REQUIRED declaration — an
    // active-power entry without one is flagged, never mapped by guessing
    // the sign from observed values.
    let profile = OpenemsMappingProfile::from_json_str(&profile_json(
        r#"{"component_id": "ess0", "channel_id": "ActivePower", "asset_key": "ess-01",
            "quantity": "ActivePower", "native_unit": "W", "declared_quality_code": "GOOD"}"#,
        "",
    ))
    .expect("valid");
    let mut adapter = adapter_over(profile, FIXTURE_EDGE_CONFIG);
    adapter.discover().expect("discovery succeeds");
    let entry = adapter
        .mapping_register()
        .iter()
        .find(|entry| entry.point == PointKey::new("ess0", "ActivePower"))
        .expect("register entry");
    assert!(
        matches!(entry.status, ChannelMappingStatus::Ambiguous { ref reason }
            if reason.contains("sign")),
        "a missing sign declaration is ambiguous: {entry:?}"
    );
}

// --- 4. Read honesty -----------------------------------------------------------------

#[test]
fn read_refuses_before_discovery_and_unmapped_points_are_named_errors() {
    let mut adapter = shipped_adapter();
    // Before discovery the adapter has no usable connection: fail closed.
    let error = adapter
        .read(&PointKey::new("ess0", "ActivePower"))
        .expect_err("read before discovery");
    assert!(
        matches!(error, AdapterError::NotConnected { .. }),
        "{error}"
    );

    adapter.discover().expect("discovery succeeds");
    // An unmapped point is a named error, never an empty result.
    let error = adapter
        .read(&PointKey::new("heatpump0", "VolumeFlow"))
        .expect_err("unmapped point read");
    assert!(matches!(error, AdapterError::ReadFailed { .. }), "{error}");
    assert!(error.to_string().contains("heatpump0/VolumeFlow"));
    // A point that does not exist on the Edge at all likewise.
    let error = adapter
        .read(&PointKey::new("ghost", "Channel"))
        .expect_err("unknown point read");
    assert!(matches!(error, AdapterError::ReadFailed { .. }), "{error}");
}

#[test]
fn fresh_reading_without_a_declared_quality_code_is_refused_by_the_acl() {
    // An entry that declares NO quality code yields readings with NO
    // quality — and the ACL refuses them (no default quality, P0.08).
    let profile = OpenemsMappingProfile::from_json_str(&profile_json(
        r#"{"component_id": "ess0", "channel_id": "Soc", "asset_key": "ess-01",
            "quantity": "Soc", "native_unit": "%"}"#,
        "",
    ))
    .expect("valid");
    let mut adapter = adapter_over(profile, FIXTURE_EDGE_CONFIG);
    adapter.discover().expect("discovery succeeds");
    let reading = adapter
        .read(&PointKey::new("ess0", "Soc"))
        .expect("the adapter returns a native reading");
    assert_eq!(
        reading.native_quality_code, None,
        "no declared quality code -> no quality on the reading (never a default)"
    );
    let acl = AntiCorruptionLayer::new(
        vec![PointMapping {
            point: PointKey::new("ess0", "Soc"),
            asset_key: "ess-01".to_string(),
            quantity: TelemetryQuantity::Soc,
            native_unit: NativeUnit::SocPercent,
            native_sign: None,
            reactive_source_convention: None,
        }],
        "edge:test-site:openems-rest",
        Vec::new(),
    )
    .expect("valid mapping table");
    let error = acl
        .map_reading(
            &reading,
            IngestTime::from_unix_millis(NOW_MILLIS).expect("valid ingest time"),
        )
        .expect_err("the ACL refuses a reading with no quality");
    assert!(
        matches!(
            error,
            flexcore_edge_agent::adapters::acl::AclError::QualityNotDeclared
        ),
        "{error}"
    );
}

#[test]
fn stale_feed_is_stamped_with_the_declared_stale_code_and_maps_to_canonical_stale() {
    // The freshness limit: a reading older than stale_after is stamped
    // STALE by the adapter (an explicit declared limit), and the frozen ACL
    // table maps that code to the canonical STALE state.
    let mut adapter = shipped_adapter();
    adapter.discover().expect("discovery succeeds");
    // The fixture serves fresh timestamps (NOW - 1 s < 15 s): fresh.
    let fresh = adapter
        .read(&PointKey::new("ess0", "ActivePower"))
        .expect("fresh read");
    assert_eq!(fresh.native_quality_code, Some("GOOD".to_string()));

    // Force a stale feed: serve the channel with a 60 s-old timestamp
    // (beyond the 15 s freshness limit).
    let mut stale_channels = fixture_channels();
    stale_channels.insert(
        "ess0/ActivePower".to_string(),
        ChannelFixture {
            value: 1_500.0,
            timestamp_millis: NOW_MILLIS - 60_000,
        },
    );
    let transport = Box::new(FixtureTransport::new(
        FIXTURE_EDGE_CONFIG,
        stale_channels,
    ));
    let mut stale_adapter = OpenEmsRestAdapter::new(
        transport,
        OpenemsMappingProfile::from_path(shipped_profile_path()).expect("profile"),
        Arc::new(FixedClock::at_unix_millis(NOW_MILLIS)),
    )
    .expect("composes");
    stale_adapter.discover().expect("discovery succeeds");
    let stale = stale_adapter
        .read(&PointKey::new("ess0", "ActivePower"))
        .expect("stale read");
    assert_eq!(
        stale.native_quality_code,
        Some(OPENEMS_STALE_CODE.to_string()),
        "the adapter stamps the declared STALE code beyond the freshness limit"
    );

    // And the ACL maps it to the canonical STALE state (never upgraded to
    // GOOD, never silently converted into a normal reading).
    let acl = AntiCorruptionLayer::new(
        vec![PointMapping {
            point: PointKey::new("ess0", "ActivePower"),
            asset_key: "ess-01".to_string(),
            quantity: TelemetryQuantity::ActivePower,
            native_unit: NativeUnit::W,
            native_sign: Some(flexcore_edge_agent::adapters::acl::NativeSign::ExportPositive),
            reactive_source_convention: None,
        }],
        "edge:test-site:openems-rest",
        Vec::new(),
    )
    .expect("valid mapping table");
    let sample = acl
        .map_reading(
            &stale,
            IngestTime::from_unix_millis(NOW_MILLIS).expect("valid ingest time"),
        )
        .expect("maps");
    assert_eq!(sample.quality(), TelemetryQuality::Stale);
    assert_eq!(sample.source_quality_code(), Some(OPENEMS_STALE_CODE));
}

#[test]
fn adapter_identity_and_capabilities_are_declared() {
    let mut adapter = shipped_adapter();
    assert_eq!(adapter.id(), OPENEMS_REST_ADAPTER_ID);
    let capabilities = adapter.capabilities();
    assert_eq!(capabilities.adapter_id, OPENEMS_REST_ADAPTER_ID);
    // The declared quality mapping names every code the adapter can report
    // (the entries' declared codes plus the adapter-derived STALE code).
    let codes: Vec<&str> = capabilities
        .quality_mapping
        .mappings()
        .iter()
        .map(|m| m.source_code.as_str())
        .collect();
    assert!(codes.contains(&OPENEMS_STALE_CODE));
    assert!(codes.contains(&"GOOD"));
    assert!(
        codes.len() >= 2,
        "the declaration covers the adapter's full reportable code set"
    );
}
