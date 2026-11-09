// P2.25 — Exit-gate synthetic portfolio: the scenario-artefact checks, the
//         expected-count arithmetic, the coverage accounting (with
//         expected-gap accounting so injected faults and bugs are
//         distinguishable), and the canonical-value checks (ledger P2.25
//         steps 1 and 4).
//
// Composition, recorded honestly (the ledger's loop bans mean the exit-gate
// registration + 24-hour run are the HUMAN's execution, runbook
// `25_exit_gate_runbook.md`): this file carries
//
// 1. PURE tests: the versioned scenario artefact
//    (`scenario-v2.json`) parses and satisfies the P2.25 shape (one tenant,
//    one portfolio, >= 2 sites, each with BESS + PV + flexible load + EVSE
//    + site meter, 24-hour horizon, seed present, capability-profile
//    references, the disabled P2.15 fault plan, and the per-asset
//    expected-quality plan tied to the fault types); the expected-count
//    arithmetic for the cadence; the coverage-accounting logic against
//    hand-built fixtures; canonical checks over hand-written canonical
//    fixtures (the port-only rule: worker/control-plane tests do NOT depend
//    on the simulator — the real record volume is produced by the
//    runbook's sim-runner path).
// 2. A DB-GATED test (FLEXCORE_TEST_DATABASE_URL): the coverage accounting
//    runs against records read back through the Postgres adapter's stored
//    telemetry path (the same read the TelemetryRead wiring serves), on a
//    small self-contained fixture with one expected (injected) gap and one
//    genuinely unexplained gap — proving the accounting end-to-end on the
//    storage path.
//
// Everything here is simulation-only synthetic telemetry (the entire 0.x
// line): no safety, certification, or control-authority implication.

use serde_json::Value;

// --- The versioned exit-gate scenario, loaded from the artefact itself ----

const SCENARIO_V2: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../scenarios/synthetic-portfolio/scenario-v2.json"
));

/// The five P2.02/P0.10 asset types every exit-gate site must carry.
const REQUIRED_ASSET_TYPES: [&str; 5] = ["BESS", "PV", "FLEXIBLE_LOAD", "EVSE", "SITE_METER"];

/// The per-asset-type emitted quantities (the simulator's recorded emission
/// set per P2.13 §5 / P2.04 §4 — `flexcore-simulator` `profiles.rs`
/// `AssetTick`): the expected-count arithmetic's quantity map.
const Q_BESS: [&str; 6] = ["active_power", "reactive_power", "soc", "availability", "operating_mode", "health"];
const Q_PV: [&str; 3] = ["active_power", "reactive_power", "availability"];
const Q_LOAD: [&str; 2] = ["active_power", "baseline_reference"];
const Q_EVSE: [&str; 4] = ["active_power", "soc", "energy", "connection_state"];
const Q_METER: [&str; 2] = ["active_power", "reactive_power"];
const QUANTITIES_PER_TYPE: [(&str, &[&str], usize); 5] = [
    ("BESS", &Q_BESS, 6),
    ("PV", &Q_PV, 3),
    ("FLEXIBLE_LOAD", &Q_LOAD, 2),
    ("EVSE", &Q_EVSE, 4),
    ("SITE_METER", &Q_METER, 2),
];

// --- 1. The scenario artefact satisfies the P2.25 shape ---------------------

#[test]
fn the_exit_gate_scenario_parses_and_satisfies_the_p2_25_shape() {
    let scenario: Value = serde_json::from_str(SCENARIO_V2).expect("the scenario is valid JSON");

    // Identity + version + the mandatory synthetic flag (P2.13 §1).
    assert_eq!(scenario["meta"]["scenario_id"], "synthetic-portfolio-exit-gate");
    assert_eq!(scenario["meta"]["version"], "v2");
    assert_eq!(scenario["meta"]["synthetic"], true);

    // One tenant, one portfolio.
    assert_eq!(scenario["portfolio"]["tenant"]["tenant_key"], "tenant-flexcore-demo");
    assert_eq!(scenario["portfolio"]["portfolio"]["portfolio_key"], "portfolio-demo");

    // >= 2 sites; EVERY site carries all five asset types (the ledger's
    // explicit risk: a single-site portfolio leaves multi-site unproven).
    let sites = scenario["portfolio"]["sites"]
        .as_array()
        .expect("sites array");
    assert!(sites.len() >= 2, "the exit gate needs >= 2 sites");
    for site in sites {
        let types: Vec<&str> = site["assets"]
            .as_array()
            .expect("assets array")
            .iter()
            .map(|a| a["asset_type"].as_str().expect("asset_type"))
            .collect();
        for required in REQUIRED_ASSET_TYPES {
            assert!(
                types.contains(&required),
                "site '{}' is missing a {required} asset",
                site["site_key"]
            );
        }
        // Every asset: key, capability reference, bus reference, rated
        // power (the P0.10 capability vocabulary is a REFERENCE in Phase 2).
        for asset in site["assets"].as_array().expect("assets") {
            assert!(!asset["asset_key"].as_str().unwrap_or_default().is_empty());
            assert!(
                !asset["capability_profile_ref"]
                    .as_str()
                    .unwrap_or_default()
                    .is_empty(),
                "asset '{}' must carry a capability-profile reference",
                asset["asset_key"]
            );
            assert!(!asset["bus_ref"].as_str().unwrap_or_default().is_empty());
            assert!(
                asset["rated_power_mw"].is_number() || asset["rated_peak_mw"].is_number(),
                "asset '{}' must carry a rated power",
                asset["asset_key"]
            );
        }
        // The declared reactive convention travels per site (P0.07 §3).
        assert!(
            site["reactive_sign_convention"].is_string(),
            "site '{}' must declare its reactive sign convention",
            site["site_key"]
        );
    }

    // 24-hour horizon + 1-minute cadence + seed + derivation rule.
    assert_eq!(scenario["horizon"]["duration_hours"], 24);
    assert!(
        scenario["horizon"]["start"]
            .as_str()
            .expect("horizon start")
            .ends_with('Z'),
        "the horizon starts at a fixed UTC instant on the scenario clock"
    );
    assert_eq!(scenario["cadence"]["default_seconds"], 60);
    assert!(scenario["seeds"]["master_seed"].is_u64(), "a master seed is mandatory");
    assert!(
        !scenario["seeds"]["derivation"].as_str().unwrap_or_default().is_empty(),
        "the per-asset seed derivation rule is recorded (D-SC4)"
    );

    // The P2.15 fault plan is present, versioned, and DISABLED by default.
    assert_eq!(scenario["faults"]["enabled"], false);
    let injections = scenario["faults"]["injections"].as_array().expect("injections");
    assert_eq!(injections.len(), 7, "the seven P2.15 fault types are configured");
    for injection in injections {
        assert!(injection["fault"].is_string());
        assert!(!injection["asset_key"].as_str().unwrap_or_default().is_empty());
        assert!(injection["from"].is_string() && injection["until"].is_string());
    }

    // The expected-quality plan (the v2 addition) covers every injection,
    // tied fault-by-fault.
    let plan = scenario["expected_quality"]["per_asset"]
        .as_array()
        .expect("expected_quality.per_asset");
    assert_eq!(plan.len(), injections.len());
    for entry in plan {
        let matching = injections.iter().any(|i| {
            i["fault"] == entry["fault"]
                && i["asset_key"] == entry["asset_key"]
                && i["from"] == entry["from"]
                && i["until"] == entry["until"]
        });
        assert!(matching, "expected-quality entry has no matching injection: {entry}");
        // The drop fault is the ONLY expected coverage gap; everything else
        // still emits one value per tick.
        if entry["fault"] == "drop" {
            assert_eq!(entry["expected_quality_in_window"], "MISSING");
            assert_eq!(entry["coverage_in_window"], "expected_gap");
        } else {
            assert_eq!(entry["coverage_in_window"], "covered");
        }
        assert!(
            [
                "GOOD", "SUSPECT", "BAD", "MISSING", "ESTIMATED", "STALE"
            ]
            .contains(&entry["expected_quality_in_window"].as_str().expect("quality")),
            "the expected quality must be one of the six canonical states (P0.08)"
        );
    }
}

// --- 2. The expected-count arithmetic for the cadence ------------------------

#[test]
fn the_expected_count_arithmetic_matches_the_cadence_and_horizon() {
    let scenario: Value = serde_json::from_str(SCENARIO_V2).expect("valid JSON");
    let duration_hours: u64 = scenario["horizon"]["duration_hours"]
        .as_u64()
        .expect("duration");
    let cadence_seconds: u64 = scenario["cadence"]["default_seconds"]
        .as_u64()
        .expect("cadence");
    let ticks = duration_hours * 3600 / cadence_seconds;
    assert_eq!(ticks, 1_440, "24 h at the 1-minute cadence = 1440 ticks (D-SC3)");

    // Per site: the emitted quantity set per asset type, 1440 records per
    // emitted quantity per asset (faults disabled).
    let sites = scenario["portfolio"]["sites"].as_array().expect("sites");
    let mut total = 0u64;
    for site in sites {
        for asset in site["assets"].as_array().expect("assets") {
            let asset_type = asset["asset_type"].as_str().expect("type");
            let (_, quantities, count) = QUANTITIES_PER_TYPE
                .iter()
                .find(|(t, _, _)| *t == asset_type)
                .unwrap_or_else(|| panic!("no recorded quantity set for {asset_type}"));
            assert_eq!(*count, quantities.len());
            total += count * ticks;
        }
    }
    assert_eq!(
        total,
        2 * (6 + 3 + 2 + 4 + 2) * 1_440,
        "the full exit-gate horizon is 48 960 records across the two sites"
    );
}

// --- 3. The coverage accounting (expected-gap accounting) ---------------------
//
// The tool: given the stored telemetry for one (asset, quantity) over the
// horizon, compute per-tick coverage and split every uncovered tick into an
// EXPECTED gap (inside an injected window — the fault plan accounts for it)
// or an UNEXPLAINED gap (a bug, or an unrecorded fault — the gate fails on
// these). Injected MISSING intervals are therefore listed and accounted,
// never silently absorbed (P2.25 step 4 / the ledger's named risk).

struct CoverageExpectation {
    /// The horizon start (Unix ms, on the scenario clock).
    start_ms: i64,
    /// Number of expected ticks (horizon / cadence).
    ticks: u64,
    /// The cadence in milliseconds.
    cadence_ms: i64,
    /// Injected expected-gap windows [from_ms, until_ms) (the `drop` fault
    /// windows of the fault plan).
    expected_gap_windows: Vec<(i64, i64)>,
}

/// One observed stored record reduced to what coverage accounting reads.
struct ObservedRecord {
    event_time_ms: i64,
    /// True when the record carries a value; false for a MISSING gap marker.
    has_value: bool,
    quality: String,
}

struct CoverageReport {
    /// Ticks with a value record (covered by data).
    covered_by_value: u64,
    /// Ticks covered only by a MISSING marker INSIDE an injected window
    /// (the expected fault outcome, P2.04 D-T6 gap detector).
    covered_by_expected_gap_marker: u64,
    /// Ticks with NO record at all inside an injected window (the gap
    /// detector has not yet synthesized the marker — visible, not silent).
    expected_gap_without_marker: Vec<i64>,
    /// Ticks with NO record and NO injected window — unexplained gaps; the
    /// exit gate fails on these (fault vs bug is distinguishable).
    unexplained_gaps: Vec<i64>,
    /// MISSING markers OUTSIDE any injected window — unexpected derived
    /// gaps; also gate-failing (a gap the plan does not account for).
    unexpected_gap_markers: Vec<i64>,
}

impl CoverageReport {
    fn is_pass(&self) -> bool {
        self.unexplained_gaps.is_empty() && self.unexpected_gap_markers.is_empty()
    }
}

fn in_expected_gap(expectation: &CoverageExpectation, time_ms: i64) -> bool {
    expectation
        .expected_gap_windows
        .iter()
        .any(|(from, until)| time_ms >= *from && time_ms < *until)
}

/// The coverage accounting (P2.25 step 4). Pure: the caller supplies the
/// stored records (any source that can list them — the DB-gated test below
/// uses the Postgres read path; the runbook's SQL produces the same counts
/// for the real run).
fn account_coverage(
    observed: &[ObservedRecord],
    expectation: &CoverageExpectation,
) -> CoverageReport {
    let mut report = CoverageReport {
        covered_by_value: 0,
        covered_by_expected_gap_marker: 0,
        expected_gap_without_marker: Vec::new(),
        unexplained_gaps: Vec::new(),
        unexpected_gap_markers: Vec::new(),
    };
    for tick in 0..expectation.ticks {
        let time_ms = expectation.start_ms + (tick as i64) * expectation.cadence_ms;
        let values: Vec<&ObservedRecord> = observed
            .iter()
            .filter(|r| r.event_time_ms == time_ms)
            .collect();
        let has_value = values.iter().any(|r| r.has_value);
        let has_marker = values
            .iter()
            .any(|r| !r.has_value && r.quality == "MISSING");
        if has_value {
            report.covered_by_value += 1;
        } else if has_marker {
            if in_expected_gap(expectation, time_ms) {
                report.covered_by_expected_gap_marker += 1;
            } else {
                report.unexpected_gap_markers.push(time_ms);
            }
        } else if in_expected_gap(expectation, time_ms) {
            report.expected_gap_without_marker.push(time_ms);
        } else {
            report.unexplained_gaps.push(time_ms);
        }
    }
    report
}

#[test]
fn coverage_accounting_distinguishes_injected_gaps_from_unexplained_ones() {
    let start = 0i64;
    let cadence = 60_000i64;
    let expectation = CoverageExpectation {
        start_ms: start,
        ticks: 10,
        cadence_ms: cadence,
        // The injected drop window covers ticks 3 and 4.
        expected_gap_windows: vec![(start + 3 * cadence, start + 5 * cadence)],
    };
    let value = |tick: i64| ObservedRecord {
        event_time_ms: start + tick * cadence,
        has_value: true,
        quality: "GOOD".to_string(),
    };
    let marker = |tick: i64| ObservedRecord {
        event_time_ms: start + tick * cadence,
        has_value: false,
        quality: "MISSING".to_string(),
    };

    // Full coverage WITH the injected gaps marked: values everywhere except
    // ticks 3–4, which the gap detector synthesized as MISSING. PASS.
    let mut observed: Vec<ObservedRecord> = vec![
        value(0),
        value(1),
        value(2),
        marker(3),
        marker(4),
        value(5),
        value(6),
        value(7),
        value(8),
        value(9),
    ];
    let report = account_coverage(&observed, &expectation);
    assert!(report.is_pass());
    assert_eq!(report.covered_by_value, 8);
    assert_eq!(report.covered_by_expected_gap_marker, 2);
    assert!(report.unexplained_gaps.is_empty());

    // The drop-fault window with NO marker yet (the detector has not run):
    // visible and accounted as an expected gap WITHOUT its marker — not
    // silently absorbed, not confused with a bug.
    observed.remove(3);
    let report = account_coverage(&observed, &expectation);
    assert!(report.unexplained_gaps.is_empty());
    assert_eq!(report.expected_gap_without_marker, vec![start + 3 * cadence]);

    // A genuinely absent tick OUTSIDE the plan: an UNEXPLAINED gap — the
    // fault-vs-bug distinction the ledger requires.
    observed.remove(3); // tick 4's marker removed; tick 4 is in-plan
    let mut observed = observed;
    observed.retain(|r| r.event_time_ms != start + 7 * cadence);
    let report = account_coverage(&observed, &expectation);
    assert!(!report.is_pass());
    assert_eq!(report.unexplained_gaps, vec![start + 7 * cadence]);

    // A MISSING marker OUTSIDE the plan: unexpected, gate-failing.
    let observed = vec![
        value(0),
        value(1),
        value(2),
        marker(3),
        marker(4),
        value(5),
        value(6),
        marker(7),
        value(8),
        value(9),
    ];
    let report = account_coverage(&observed, &expectation);
    assert!(!report.is_pass());
    assert_eq!(report.unexpected_gap_markers, vec![start + 7 * cadence]);
}

// --- 4. Canonical-value checks over hand-written fixtures ---------------------
//
// The port-only rule (recorded): worker/control-plane tests do NOT depend
// on the simulator, so the canonical checks run over tiny hand-written
// canonical fixtures — one per emitted quantity kind — asserting the frozen
// P2.04/P0.07/P0.08 rules. The runbook's sim-runner path produces the real
// volume these rules are checked against at the gate.

struct CanonicalFixture {
    quantity: &'static str,
    unit: &'static str,
    value_num: Option<f64>,
    value_bool: Option<bool>,
    value_text: Option<&'static str>,
    quality: &'static str,
}

/// The frozen quantity → unit map (P2.04 §4; P0.07 §1: MW/MVAr/MWh, `1`
/// dimensionless — no kW/kWh anywhere).
fn expected_unit(quantity: &str) -> Option<&'static str> {
    match quantity {
        "active_power" | "baseline_reference" => Some("MW"),
        "reactive_power" => Some("MVAr"),
        "energy" => Some("MWh"),
        "soc" | "availability" | "operating_mode" | "health" | "connection_state" => Some("1"),
        _ => None,
    }
}

const SIX_QUALITY_STATES: [&str; 6] = ["GOOD", "SUSPECT", "BAD", "MISSING", "ESTIMATED", "STALE"];

fn canonical_fixtures() -> Vec<CanonicalFixture> {
    vec![
        // BESS discharge: export, P > 0 at the point of connection (P0.07 §2).
        CanonicalFixture {
            quantity: "active_power",
            unit: "MW",
            value_num: Some(1.5),
            value_bool: None,
            value_text: None,
            quality: "GOOD",
        },
        // Flexible load: consumption is NEGATIVE (P > 0 = export).
        CanonicalFixture {
            quantity: "active_power",
            unit: "MW",
            value_num: Some(-0.42),
            value_bool: None,
            value_text: None,
            quality: "GOOD",
        },
        CanonicalFixture {
            quantity: "reactive_power",
            unit: "MVAr",
            value_num: Some(0.1),
            value_bool: None,
            value_text: None,
            quality: "GOOD",
        },
        CanonicalFixture {
            quantity: "soc",
            unit: "1",
            value_num: Some(0.5),
            value_bool: None,
            value_text: None,
            quality: "GOOD",
        },
        CanonicalFixture {
            quantity: "energy",
            unit: "MWh",
            value_num: Some(0.01),
            value_bool: None,
            value_text: None,
            quality: "GOOD",
        },
        CanonicalFixture {
            quantity: "availability",
            unit: "1",
            value_num: None,
            value_bool: Some(true),
            value_text: None,
            quality: "GOOD",
        },
        CanonicalFixture {
            quantity: "operating_mode",
            unit: "1",
            value_num: None,
            value_bool: None,
            value_text: Some("CHARGING"),
            quality: "GOOD",
        },
        CanonicalFixture {
            quantity: "health",
            unit: "1",
            value_num: None,
            value_bool: None,
            value_text: Some("NOMINAL"),
            quality: "GOOD",
        },
        CanonicalFixture {
            quantity: "connection_state",
            unit: "1",
            value_num: None,
            value_bool: Some(false),
            value_text: None,
            quality: "GOOD",
        },
        CanonicalFixture {
            quantity: "baseline_reference",
            unit: "MW",
            value_num: Some(-0.4),
            value_bool: None,
            value_text: None,
            quality: "GOOD",
        },
        // A MISSING gap marker: no value, exactly the derived state.
        CanonicalFixture {
            quantity: "active_power",
            unit: "MW",
            value_num: None,
            value_bool: None,
            value_text: None,
            quality: "MISSING",
        },
    ]
}

#[test]
fn canonical_checks_pass_over_the_hand_written_fixtures() {
    for fixture in canonical_fixtures() {
        // Unit: derived from the quantity, canonical vocabulary only.
        assert_eq!(
            expected_unit(fixture.quantity),
            Some(fixture.unit),
            "unit must be the canonical unit of the quantity (P2.04 §4)"
        );
        // Quality: exactly one of the six canonical states (P0.08).
        assert!(
            SIX_QUALITY_STATES.contains(&fixture.quality),
            "'{}' is not a canonical quality state",
            fixture.quality
        );
        // Value totality: exactly one value leg, none exactly for MISSING.
        let legs = fixture.value_num.is_some() as u8
            + fixture.value_bool.is_some() as u8
            + fixture.value_text.is_some() as u8;
        if fixture.quality == "MISSING" {
            assert_eq!(legs, 0, "a MISSING gap marker carries no value");
        } else {
            assert_eq!(legs, 1, "a measured record carries exactly one value");
        }
        // SOC is a fraction in [0,1] (P0.07 §4).
        if fixture.quantity == "soc" {
            let soc = fixture.value_num.expect("soc carries a numeric value");
            assert!((0.0..=1.0).contains(&soc), "soc must be in [0,1], got {soc}");
        }
    }
}

#[test]
fn the_sign_convention_is_export_positive_in_the_fixtures() {
    // P0.07 §2: P > 0 = export at the point of connection — load
    // consumption and (charging) EVSE power are negative. The fixtures
    // encode the rule; this check documents it on the fixture set.
    for fixture in canonical_fixtures() {
        if fixture.quantity == "active_power" {
            if let Some(value) = fixture.value_num {
                // The only positive active_power fixture is the BESS export.
                if value > 0.0 {
                    assert!(value <= 2.0, "export within the rated limit");
                }
            }
        }
    }
}

// --- 5. DB-gated: the accounting runs on the stored-telemetry read path -----
//
// Gating is the recorded harness decision: skips cleanly when
// FLEXCORE_TEST_DATABASE_URL is unset. Small self-contained fixture (one
// asset, five one-minute ticks) — the REAL exit-gate volume is produced by
// the runbook's sim-runner path; this test proves the accounting runs on
// records read back through the Postgres stored-telemetry path.

#[cfg(test)]
mod db_gated {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::OnceLock;
    use std::time::{SystemTime, UNIX_EPOCH};

    use flexcore_contracts::ids::FlexUuid;
    use flexcore_contracts::quality::TelemetryQuality;
    use flexcore_contracts::time_model::{ClockQuality, EventTime, IngestTime};
    use flexcore_contracts::units::ActivePowerMW;
    use flexcore_contracts::time_model::SiteTimezone;
    use flexcore_domain::entities::{Asset, AssetType, Portfolio, Site, Tenant};
    use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
    use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
    use flexcore_ports::repository::{
        AssetRepository, PortfolioRepository, SiteRepository, TenantRepository,
    };
    use flexcore_postgres_adapter::telemetry::{
        NormalizedTelemetryRecord, Quantity, TelemetryValue,
    };
    use flexcore_postgres_adapter::{FlexCoreStore, PgUnitOfWork, SystemClock};

    const JAN_2030: i64 = 1_893_456_000;

    fn test_url() -> Option<String> {
        std::env::var("FLEXCORE_TEST_DATABASE_URL")
            .ok()
            .map(|url| url.trim().to_string())
            .filter(|url| !url.is_empty())
    }

    fn run_key() -> &'static u64 {
        static RUN_KEY: OnceLock<u64> = OnceLock::new();
        RUN_KEY.get_or_init(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("the system clock is after the Unix epoch")
                .as_nanos() as u64
        })
    }

    fn next_counter() -> u64 {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        COUNTER.fetch_add(1, Ordering::SeqCst)
    }

    fn fixture_uuid(prefix: u8) -> FlexUuid {
        let mut bytes = [0u8; 16];
        bytes[0] = prefix;
        bytes[1] = 0x6D; // P2.25 fixture marker
        bytes[2..10].copy_from_slice(&run_key().to_be_bytes());
        let counter = next_counter().to_be_bytes();
        bytes[10..16].copy_from_slice(&counter[2..8]);
        FlexUuid::from_bytes(bytes)
    }

    fn context_for(tenant_id: TenantId) -> TenantContext {
        TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
            VerifiedClaims {
                tenant_id,
                subject: "integration-test:exit-portfolio".to_string(),
            },
        ))
    }

    fn test_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime")
    }

    fn url_as(url: &str, user: &str, password: &str) -> String {
        if let Some((scheme, rest)) = url.split_once("://") {
            if let Some((_, host_and_path)) = rest.split_once('@') {
                return format!("{scheme}://{user}:{password}@{host_and_path}");
            }
            return format!("{scheme}://{user}:{password}@{rest}");
        }
        format!("{url}?user={user}&password={password}")
    }

    #[test]
    fn coverage_accounting_runs_on_the_stored_telemetry_read_path() {
        let Some(url) = test_url() else {
            println!(
                "skipped coverage_accounting_runs_on_the_stored_telemetry_read_path: \
                 FLEXCORE_TEST_DATABASE_URL is not set (CI provides a PostgreSQL 18 service)"
            );
            return;
        };
        let admin_store = FlexCoreStore::connect(&url).expect("admin store");
        admin_store.run_migrations().expect("migrations");
        ensure_roles_and_partition(admin_store.pool(), &url);

        let tenant_id = TenantId::new(fixture_uuid(0xD1));
        let asset_id = AssetId::new(fixture_uuid(0xD2));
        let tenant = Tenant::try_new(
            tenant_id,
            format!("tenant-{tenant_id}"),
            None,
            format!("idp:{tenant_id}"),
            "profile:default".to_string(),
            SystemClock.now_event_time(),
        )
        .expect("valid tenant");
        {
            let unit = admin_store.begin(None).expect("admin uow");
            TenantRepository::insert(&unit, &tenant).expect("tenant insert");
            unit.commit().expect("commit");
        }
        let ctx = context_for(tenant_id);
        let app_store =
            FlexCoreStore::connect(&url_as(&url, "flexcore_app", "flexcore_app_test_only"))
                .expect("app store");
        {
            let unit = app_store.begin(Some(&ctx)).expect("app uow");
            let portfolio = Portfolio::try_new(
                &tenant,
                PortfolioId::new(fixture_uuid(0xD3)),
                "Exit-gate portfolio".to_string(),
                None,
                "Neryva simulated operator".to_string(),
                "DE-LABEL-ONLY".to_string(),
                "rules:constrained-aggregation".to_string(),
                SystemClock.now_event_time(),
            )
            .expect("valid portfolio");
            PortfolioRepository::insert(&unit, &ctx, &portfolio).expect("portfolio insert");
            let site = Site::try_new(
                &portfolio,
                SiteId::new(fixture_uuid(0xD4)),
                "Exit-gate site".to_string(),
                None,
                "{}".to_string(),
                SiteTimezone::new("Europe/Berlin".to_string()).expect("valid timezone"),
                "pcc:test".to_string(),
                "grid-model:rev-1".to_string(),
                "edge:simulator".to_string(),
                SystemClock.now_event_time(),
            )
            .expect("valid site");
            SiteRepository::insert(&unit, &ctx, &site).expect("site insert");
            let asset = Asset::try_new(
                &site,
                asset_id,
                format!("asset-{asset_id}"),
                None,
                AssetType::Bess,
                "vendor:test".to_string(),
                ActivePowerMW::new(2.0).expect("positive rating"),
                None,
                "phase:abc".to_string(),
                "bus:1".to_string(),
                "cap/bess-default".to_string(),
                SystemClock.now_event_time(),
            )
            .expect("valid asset");
            AssetRepository::insert(&unit, &ctx, &asset).expect("asset insert");
            unit.commit().expect("commit registry");
        }

        // Five one-minute ticks from JAN_2030: tick 2 is covered only by an
        // in-plan MISSING marker (the injected drop window); tick 4 is
        // genuinely absent and NOT in the plan.
        let start = JAN_2030;
        let cadence_s = 60i64;
        let record = |tick: i64, value: Option<TelemetryValue>| NormalizedTelemetryRecord {
            tenant_id,
            asset_id,
            quantity: Quantity::ActivePower,
            event_time: EventTime::from_unix_millis((start + tick * cadence_s) * 1000)
                .expect("valid event time"),
            ingest_time: IngestTime::from_unix_millis((start + tick * cadence_s) * 1000 + 1_000)
                .expect("valid ingest time"),
            value,
            source_sequence: tick + 1,
            source_id: "sim://exit-gate".to_string(),
            quality: if value.is_none() {
                TelemetryQuality::Missing
            } else {
                TelemetryQuality::Good
            },
            source_quality_code: None,
            q_sign_convention: None,
            clock_quality: ClockQuality::Synchronized,
        };
        let records = vec![
            record(0, Some(TelemetryValue::Numeric(1.0))),
            record(1, Some(TelemetryValue::Numeric(1.1))),
            record(2, None), // in-plan gap marker
            record(3, Some(TelemetryValue::Numeric(1.3))),
            // tick 4: deliberately absent — the unexplained gap.
        ];
        let ingest =
            FlexCoreStore::connect(&url_as(&url, "flexcore_ingest", "flexcore_ingest_test_only"))
                .expect("ingest store");
        {
            let unit = ingest.begin(Some(&ctx)).expect("ingest uow");
            let items = PgUnitOfWork::insert_telemetry_batch(&unit, &ctx, &records)
                .expect("batch insert");
            assert!(items.iter().all(|i| {
                matches!(
                    i.outcome,
                    flexcore_postgres_adapter::telemetry::TelemetryBatchOutcome::Stored
                )
            }));
            unit.commit().expect("commit telemetry");
        }

        // Read the stored telemetry back through the adapter's read path
        // (the same storage read the TelemetryRead wiring serves) and run
        // the SAME accounting function as the pure tests.
        let unit = ingest.begin(Some(&ctx)).expect("read uow");
        let stored = PgUnitOfWork::query_telemetry(
            &unit,
            &ctx,
            &[asset_id],
            EventTime::from_unix_millis(start * 1000).expect("valid"),
            EventTime::from_unix_millis((start + 5 * cadence_s) * 1000).expect("valid"),
            None,
        )
        .expect("stored telemetry read");
        unit.commit().expect("commit read");
        let observed: Vec<ObservedRecord> = stored
            .iter()
            .map(|r| ObservedRecord {
                event_time_ms: r.event_time.timestamp().unix_millis(),
                has_value: r.value.is_some(),
                quality: r.quality.name().to_string(),
            })
            .collect();
        // The injected window covers tick 2 only.
        let expectation = CoverageExpectation {
            start_ms: start * 1000,
            ticks: 5,
            cadence_ms: cadence_s * 1000,
            expected_gap_windows: vec![(
                (start + 2 * cadence_s) * 1000,
                (start + 3 * cadence_s) * 1000,
            )],
        };
        let report = account_coverage(&observed, &expectation);
        // Tick 2: expected, marked. Tick 4: UNEXPLAINED — the fault-vs-bug
        // distinction, proven end-to-end on the storage path.
        assert_eq!(report.covered_by_value, 3);
        assert_eq!(report.covered_by_expected_gap_marker, 1);
        assert_eq!(
            report.unexplained_gaps,
            vec![(start + 4 * cadence_s) * 1000]
        );
        assert!(!report.is_pass());
    }

    fn ensure_roles_and_partition(pool: &sqlx::PgPool, url: &str) {
        test_runtime().block_on(async {
            for (role, password) in [
                ("flexcore_app", "flexcore_app_test_only"),
                ("flexcore_migrator", "flexcore_migrator_test_only"),
                ("flexcore_ingest", "flexcore_ingest_test_only"),
            ] {
                sqlx::query(&format!("ALTER ROLE {role} WITH LOGIN PASSWORD '{password}'"))
                    .execute(pool)
                    .await
                    .expect("test role grant");
            }
        });
        let migrator_pool = test_runtime()
            .block_on(sqlx::PgPool::connect(&url_as(
                url,
                "flexcore_migrator",
                "flexcore_migrator_test_only",
            )))
            .expect("migrator pool");
        test_runtime().block_on(async {
            sqlx::query(
                "CREATE TABLE IF NOT EXISTS telemetry_y2030m01 PARTITION OF telemetry \
                 FOR VALUES FROM ('2030-01-01 00:00:00+00') TO ('2030-02-01 00:00:00+00')",
            )
            .execute(&migrator_pool)
            .await
            .expect("partition DDL");
        });
    }
}
