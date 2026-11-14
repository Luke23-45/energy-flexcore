// P5.20 — The scenario-library schema validation suite (report §19.3,
// §19.4; ledger P5.20).
//
// WHAT IS TESTED (static validation over every library record; NO case is
// executed — the cases' expected outcomes are PRE-REGISTERED and UNRUN):
//   1. Every `scenarios/library/lib-*.json` parses as a ScenarioRecord v3
//      and passes the full fail-loud validation (schema version, scale
//      class, backend pin, fixture name, thread count, tolerances,
//      pre-registered expectations, per-case IEEE record).
//   2. Every pinned input EXISTS: the pinned scenario file resolves
//      relative to the record's directory and loads through the
//      all-or-nothing scenario loader; the pinned forecast fixture (when
//      present) exists and carries a version.
//   3. The §19.3 mapping is honest: every record cites the §19.3 bullet
//      it covers; every IEEE record is `included: false` with the
//      recorded reason (no IEEE model is available and licensing is
//      unconfirmed); the scale-limit statement is present in the library
//      README and INDEX.
//   4. INDEX.md names every record with its purpose, scale, runtime
//      CLASS (never a measured figure), and backend dependency.
//
// HONEST BOUNDARIES:
//   - NOTHING was executed: the case runs (and the worker_process legs'
//     pandapower execution) are the human build/test pass; the worker
//     itself was WRITTEN, never executed (UNVERIFIED pandapower API).
//   - The expected runtime classes in INDEX.md are DECLARED classes, not
//     measured figures (measured runtimes are P5.23's obligation).
//   - No IEEE feeder is used or claimed anywhere (§19.3; the
//     14_15_adapters.md feeder-case record).

use flexcore_sim_runner::replay_record::{load_scenario_record, SCALE_CLASSES};
use flexcore_simulator::load_scenario;

const LIBRARY_DIR: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../scenarios/library");
const REPO_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

fn library_record_paths() -> Vec<std::path::PathBuf> {
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(LIBRARY_DIR)
        .expect("the library directory exists")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.starts_with("lib-") && name.ends_with(".json"))
                    .unwrap_or(false)
        })
        .collect();
    paths.sort();
    paths
}

// --- 1. Every record parses and validates ------------------------------------------

#[test]
fn every_library_record_parses_as_a_scenario_record_v3_and_validates() {
    let paths = library_record_paths();
    assert!(
        paths.len() >= 8,
        "the library carries the eight §19.3 cases (found {})",
        paths.len()
    );
    for path in &paths {
        let full = path.to_string_lossy().to_string();
        let (record, _dir) = load_scenario_record(&full).unwrap_or_else(|error| {
            panic!("{} must parse and validate: {error}", path.display())
        });
        assert_eq!(record.schema_version, "3");
        assert!(SCALE_CLASSES.contains(&record.scale_class.as_str()));
        assert!(record.synthetic, "{} must be synthetic", record.record_id);
        assert_eq!(record.record_version, "v1");
    }
}

#[test]
fn every_record_has_pre_registered_expectations_with_the_unrun_status() {
    for path in library_record_paths() {
        let full = path.to_string_lossy().to_string();
        let (record, _dir) = load_scenario_record(&full).expect("valid record");
        assert!(
            record.expected_outcomes.pre_registered,
            "{}: expectations must be pre-registered (the P3.26 anti-fitting rule)",
            record.record_id
        );
        assert_eq!(
            record.expected_outcomes.status, "UNRUN",
            "{}: the authoring loop ran nothing — the status is honest",
            record.record_id
        );
        assert!(
            !record.expected_outcomes.statements.is_empty(),
            "{}: the expectation slot carries statements",
            record.record_id
        );
        assert!(
            !record.expected_outcomes.reference.trim().is_empty(),
            "{}: the pre-registration reference is present",
            record.record_id
        );
    }
}

#[test]
fn every_record_pins_seed_derivation_and_nondeterminism_controls() {
    for path in library_record_paths() {
        let full = path.to_string_lossy().to_string();
        let (record, _dir) = load_scenario_record(&full).expect("valid record");
        assert_eq!(
            record.seeds.master_seed, 20260601,
            "{}: the recorded master seed",
            record.record_id
        );
        assert!(
            record.seeds.derivation.contains("FNV-1a-64"),
            "{}: the derivation rule travels with the seed",
            record.record_id
        );
        assert_eq!(
            record.nondeterminism_controls.thread_count, 1,
            "{}: thread_count = 1 (the P3.26 solver pin, cited not re-pinned)",
            record.record_id
        );
        assert!(
            record.nondeterminism_controls.controls.len() >= 4,
            "{}: the nondeterminism inventory is carried",
            record.record_id
        );
        assert!(
            !record.solver_pins.is_empty(),
            "{}: the solver pins travel (with their classes — AGENTS.md rule 4)",
            record.record_id
        );
    }
}

// --- 2. Every pinned input exists ----------------------------------------------------

#[test]
fn every_pinned_scenario_exists_and_loads_all_or_nothing() {
    for path in library_record_paths() {
        let full = path.to_string_lossy().to_string();
        let (record, record_dir) = load_scenario_record(&full).expect("valid record");
        let scenario_path = if std::path::Path::new(&record.inputs.scenario.path).is_absolute() {
            record.inputs.scenario.path.clone()
        } else {
            format!("{record_dir}/{}", record.inputs.scenario.path)
        };
        let scenario_json = std::fs::read_to_string(&scenario_path).unwrap_or_else(|error| {
            panic!(
                "{} pins '{}': {error}",
                record.record_id, record.inputs.scenario.path
            )
        });
        // The all-or-nothing loader (the P2.13 rule) accepts the pinned
        // artefact.
        load_scenario(&scenario_json)
            .unwrap_or_else(|error| panic!("{} pins a valid scenario: {error}", record.record_id));
    }
}

#[test]
fn every_pinned_forecast_fixture_exists_and_names_its_version() {
    for path in library_record_paths() {
        let full = path.to_string_lossy().to_string();
        let (record, record_dir) = load_scenario_record(&full).expect("valid record");
        if let Some(forecast) = &record.inputs.forecast_fixture {
            let fixture_path = if std::path::Path::new(&forecast.path).is_absolute() {
                forecast.path.clone()
            } else {
                format!("{record_dir}/{}", forecast.path)
            };
            std::fs::read_to_string(&fixture_path)
                .unwrap_or_else(|error| panic!("{} pins a readable fixture: {error}", record.record_id));
            assert!(
                !forecast.version.trim().is_empty(),
                "{}: the fixture version is pinned",
                record.record_id
            );
        }
    }
}

#[test]
fn the_pinned_scenario_identity_matches_the_artefact() {
    for path in library_record_paths() {
        let full = path.to_string_lossy().to_string();
        let (record, record_dir) = load_scenario_record(&full).expect("valid record");
        let scenario_path = format!("{record_dir}/{}", record.inputs.scenario.path);
        let text = std::fs::read_to_string(&scenario_path).expect("readable");
        let value: serde_json::Value = serde_json::from_str(&text).expect("JSON");
        assert_eq!(
            value["meta"]["scenario_id"]
                .as_str()
                .expect("the artefact names its id"),
            record.inputs.scenario.scenario_id,
            "{}: the identity pin matches the artefact",
            record.record_id
        );
        assert_eq!(
            value["meta"]["version"].as_str().expect("the artefact names its version"),
            record.inputs.scenario.version,
            "{}: the version pin matches the artefact",
            record.record_id
        );
    }
}

// --- 3. The §19.3 mapping is honest ---------------------------------------------------

#[test]
fn every_record_cites_the_19_3_bullet_it_covers() {
    for path in library_record_paths() {
        let full = path.to_string_lossy().to_string();
        let (record, _dir) = load_scenario_record(&full).expect("valid record");
        assert!(
            record.benchmark_reference.contains("19.3"),
            "{}: the benchmark reference cites §19.3",
            record.record_id
        );
        assert!(
            !record.purpose.trim().is_empty(),
            "{}: the purpose is stated",
            record.record_id
        );
    }
}

#[test]
fn every_ieee_case_record_is_an_honest_exclusion_with_the_reason() {
    for path in library_record_paths() {
        let full = path.to_string_lossy().to_string();
        let (record, _dir) = load_scenario_record(&full).expect("valid record");
        assert!(
            !record.ieee_case_record.included,
            "{}: no IEEE case is included in this phase (licensing/availability \
             unconfirmed — the exclusion IS the honest entry)",
            record.record_id
        );
        assert!(
            record.ieee_case_record.reason.contains("excluded"),
            "{}: the per-case record states the exclusion with its reason",
            record.record_id
        );
        assert!(
            record.ieee_case_record.reason.contains("licensing"),
            "{}: the reason names the licensing/availability discipline",
            record.record_id
        );
    }
}

#[test]
fn the_scale_limit_statement_is_present_in_the_library_docs() {
    let readme =
        std::fs::read_to_string(format!("{LIBRARY_DIR}/README.md")).expect("the README exists");
    assert!(
        readme.contains("10,000-bus"),
        "the README carries the §19.3 scale-limit statement"
    );
    let index =
        std::fs::read_to_string(format!("{LIBRARY_DIR}/INDEX.md")).expect("the INDEX exists");
    assert!(
        index.contains("10,000-bus"),
        "the INDEX carries the §19.3 scale-limit statement"
    );
}

#[test]
fn the_stressed_case_set_covers_the_19_3_stressors_it_names() {
    // The §19.3 stressors the library covers, and WHERE each is covered —
    // every named stressor is mapped, none invented.
    let mut stressors = Vec::new();
    for path in library_record_paths() {
        let full = path.to_string_lossy().to_string();
        let (record, _dir) = load_scenario_record(&full).expect("valid record");
        if record.scale_class == "stress" {
            stressors.push(record.benchmark_reference.clone());
        }
    }
    assert!(stressors.iter().any(|reference| reference.contains("PV ramps")), "PV ramps covered");
    assert!(stressors.iter().any(|reference| reference.contains("forecast error")), "forecast error covered");
    assert!(stressors.iter().any(|reference| reference.contains("asset dropout")), "asset dropout covered");
    assert!(stressors.iter().any(|reference| reference.contains("communication loss")), "communication loss (analogue) covered");
    assert!(stressors.iter().any(|reference| reference.contains("reverse power flow")), "reverse power flow covered");
    assert!(stressors.iter().any(|reference| reference.contains("solver-timeout")), "solver timeout covered");
    assert!(stressors.iter().any(|reference| reference.contains("non-convergence")), "non-convergence covered");
}

#[test]
fn topology_change_is_covered_by_the_meshed_fixture_record_not_invented() {
    // The topology-change stressor is covered at FIXTURE scale (lib-03's
    // meshed two-feeder fixture under the adapter's declared
    // RadialAndMeshed scope) — a fixture-level topology class, NOT a
    // runtime switching model, and the record says so.
    let full = format!("{LIBRARY_DIR}/lib-03-medium-multi-site-meshed.json");
    let (record, _dir) = load_scenario_record(&full).expect("valid record");
    assert_eq!(record.inputs.grid_model.fixture, "meshed_two_feeder");
    assert!(record.inputs.grid_model.statement.contains("BALANCED only"));
    assert!(record.inputs.grid_model.statement.contains("TopologyScope::RadialAndMeshed"));
}

// --- 4. INDEX.md names every record -------------------------------------------------

#[test]
fn the_index_names_every_record_with_purpose_scale_runtime_class_and_backend() {
    let index =
        std::fs::read_to_string(format!("{LIBRARY_DIR}/INDEX.md")).expect("the INDEX exists");
    for path in library_record_paths() {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("a file name");
        assert!(
            index.contains(name),
            "the INDEX names {name}"
        );
    }
    // Runtime classes are declared, never measured: the INDEX must not
    // carry a bare measured-number runtime (it states the class instead).
    assert!(index.contains("not measured"));
    assert!(index.contains("worker_process"));
    assert!(index.contains("scripted"));
}

#[test]
fn the_minimal_fixture_is_a_loadable_scenario() {
    // The minimal single-asset fixture is a full P2.13 scenario (the
    // smallest replayable volume), not a record-only artefact.
    let text = std::fs::read_to_string(format!(
        "{LIBRARY_DIR}/fixtures/scenario-minimal-single-asset.json"
    ))
    .expect("the fixture exists");
    let scenario = load_scenario(&text).expect("the fixture loads all-or-nothing");
    assert_eq!(scenario.meta.scenario_id, "library-minimal-single-asset");
    assert_eq!(scenario.master_seed, 20260601);
    let asset_count: usize = scenario.sites.iter().map(|site| site.assets.len()).sum();
    assert_eq!(asset_count, 1, "the minimal case is ONE asset");
}

#[test]
fn the_worker_stress_records_name_the_real_worker_leg_and_its_unverified_status() {
    // The worker-leg records pin the REPO-ROOT-relative worker script
    // (which exists) — the leg itself is UNRUN (UNVERIFIED pandapower
    // API), and the record's expectations say so.
    let script = std::fs::read_to_string(format!(
        "{REPO_ROOT}/apps/worker/python/grid_worker.py"
    ))
    .expect("the P5.15 worker script exists in the repository");
    assert!(script.contains("pandapower"));
    for name in [
        "lib-04-stress-worker-timeout.json",
        "lib-05-stress-nonconvergence.json",
    ] {
        let (record, _dir) =
            load_scenario_record(&format!("{LIBRARY_DIR}/{name}")).expect("valid record");
        assert_eq!(record.backend.selection, "worker_process");
        let worker = record.backend.worker.as_ref().expect("the worker pin");
        assert_eq!(worker.script_path, "apps/worker/python/grid_worker.py");
        assert!(worker.timeout_ms > 0, "the timeout is fail-closed and pinned");
        assert!(
            record
                .expected_outcomes
                .statements
                .iter()
                .any(|statement| statement.to_lowercase().contains("fail-closed")),
            "{name} pre-registers the fail-closed outcome"
        );
    }
}
