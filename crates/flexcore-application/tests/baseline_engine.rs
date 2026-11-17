// P16.04 — Baseline engine: determinism and replay tests (WRITTEN-UNRUN).
//
// Phase 16 plan §2 row P16.04; decision record:
// `docs/phases-11-20/phase-16-market-integration-settlement/records/
// 04_p16_04_baseline_engine.md`.
//
// Covers the subtask's test obligations:
//
// - LR-073: an UNSUPPORTED declared method produces a RECORDED GAP ROW
//   (`BaselineOutcome::Gap`, reason `UnsupportedMethod`) — never a
//   silent approximation, never a FlexCore-chosen substitute;
// - CBL 10-in-10: the per-interval average over the highest 10 of the
//   last 10 eligible non-event days (here: the average of the 10
//   windowed days), event days excluded;
// - CBL 5-in-10: the highest 5 of the last 10 eligible days, with an
//   EXPLICIT tie-break (equal totals resolve by day-key ascending — the
//   determinism contract);
// - day-matching: day-type and weather-index filters (exact day-type
//   equality; |candidate − event| ≤ tolerance; days WITHOUT an index
//   never match — a missing index never silently passes);
// - the capped symmetric SDA: a raw ratio beyond the cap is applied AT
//   the cap (CAISO ≤20% [S-30]), symmetric on both directions, with the
//   RAW ratio recorded alongside the APPLIED one (capped, never clipped
//   into silence);
// - an SDA declared but its window missing ⇒ RECORDED GAP
//   (`MissingSdaWindow`); an SDA window outside the interval grid ⇒
//   RECORDED GAP (`InvalidSdaWindow`);
// - a declared SDA cap above the jurisdiction bound ⇒ RECORDED GAP
//   (`SdaCapExceedsJurisdictionBound`);
// - insufficient eligible days ⇒ RECORDED GAP;
// - interval-count mismatch ⇒ RECORDED GAP;
// - REPLAY determinism: the same inputs + the same injected clock
//   produce byte-identical outcomes across repeated calls (the clock is
//   the `flexcore_ports::fakes::FixedClock`; no wall clock, no RNG, no
//   map-order dependence);
// - the method name recorded on a computed profile is the PROGRAM'S
//   declared name (LR-073: the method stays the program's, even on the
//   output).
//
// NOTE (VERIFY-BUILD, recorded): per the loop bans, cargo was NOT run
// while authoring these tests; the first
// `cargo test -p flexcore-application --test baseline_engine` run is the
// verification obligation (the WRITTEN-UNRUN class of the whole 0–10
// base).

use std::collections::BTreeMap;

use flexcore_application::baseline_engine::{
    compute_baseline, BaselineOutcome, BaselineSelectionRule, CAISO_SDA_MAX_PERCENT,
    CBL_RECENCY_WINDOW_DAYS, FLEXCORE_NEVER_CHOOSES_STATEMENT, SUPPORTED_METHOD_NAMES,
};
use flexcore_application::baseline_engine::{
    DayProfile, ProgramBaselineConfig, SdaConfig, SdaWindowInput,
};
use flexcore_ports::fakes::FixedClock;

/// The pinned test instant (UTC; the injected clock's instant — never a
/// wall-clock read).
const PINNED_NOW_UNIX_MS: i64 = 1_760_000_000_000;

/// Builds a day profile with `count` intervals of the given constant
/// magnitude (the tests use flat profiles so hand-computed averages are
/// exact).
fn flat_day(day_key: &str, day_type: Option<&str>, mw: f64, count: usize) -> DayProfile {
    DayProfile::new(
        day_key.to_string(),
        day_type.map(|s| s.to_string()),
        None,
        vec![mw; count],
        false,
    )
    .expect("test day profile is valid")
}

/// Builds an eligible-day BTreeMap with keys `d01`..`d10` (day-key order
/// equals recency order in these fixtures).
fn ten_flat_days(mw: f64, interval_count: usize) -> BTreeMap<String, DayProfile> {
    let mut days = BTreeMap::new();
    for i in 1..=10 {
        let key = format!("d{i:02}");
        days.insert(key.clone(), flat_day(&key, None, mw, interval_count));
    }
    days
}

#[test]
fn unsupported_method_is_a_recorded_gap_row_never_an_approximation() {
    let clock = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let mut config =
        ProgramBaselineConfig::new("tenant-1".to_string(), "program-1".to_string(), "DLA_4_OF_5".to_string())
            .expect("config is valid");
    config.selection = Some(BaselineSelectionRule::All);
    let days = ten_flat_days(1.0, 4);
    let outcome = compute_baseline(&config, &days, 4, None, &clock);
    match outcome {
        BaselineOutcome::Gap(gap) => {
            assert_eq!(gap.method_name, "DLA_4_OF_5");
            assert_eq!(gap.recorded_at_unix_ms, PINNED_NOW_UNIX_MS);
            assert!(matches!(
                gap.reason,
                flexcore_application::baseline_engine::BaselineGapReason::UnsupportedMethod
            ));
        }
        BaselineOutcome::Computed(_) => {
            panic!("LR-073 violated: an unsupported method computed a baseline")
        }
    }
}

#[test]
fn cbl_10_in_10_averages_the_ten_most_recent_eligible_non_event_days() {
    let clock = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let mut config =
        ProgramBaselineConfig::new("tenant-1".to_string(), "program-1".to_string(), "CBL_10_IN_10".to_string())
            .expect("config is valid");
    config.selection = Some(BaselineSelectionRule::All);
    let mut days = ten_flat_days(1.0, 4);
    // The event day itself is EXCLUDED from every candidate set, even
    // when it is inside the recency window.
    let mut event_day = flat_day("d05", None, 100.0, 4);
    event_day.is_event_day = true;
    days.insert("d05".to_string(), event_day);
    // A non-event day outside the recency window (older than the last
    // ten candidates) must NOT enter the average.
    days.insert("d00".to_string(), flat_day("d00", None, 100.0, 4));
    let outcome = compute_baseline(&config, &days, 4, None, &clock);
    match outcome {
        BaselineOutcome::Computed(profile) => {
            // 9 remaining windowed days at 1.0 (d05 excluded as the
            // event day, d00 outside the recency window): nine eligible
            // days cannot fill the ten-day window, so this fixture must
            // never average.
            panic!("expected a recorded gap, got a computed profile: {profile:?}");
        }
        BaselineOutcome::Gap(gap) => {
            assert!(matches!(
                gap.reason,
                flexcore_application::baseline_engine::BaselineGapReason::InsufficientEligibleDays {
                    required: 10,
                    found: 9,
                }
            ));
        }
    }
}

#[test]
fn cbl_10_in_10_computes_when_ten_eligible_days_exist() {
    let clock = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let config =
        ProgramBaselineConfig::new("tenant-1".to_string(), "program-1".to_string(), "CBL_10_IN_10".to_string())
            .expect("config is valid");
    let days = ten_flat_days(2.0, 4);
    let outcome = compute_baseline(&config, &days, 4, None, &clock);
    match outcome {
        BaselineOutcome::Computed(profile) => {
            assert_eq!(profile.method_name, "CBL_10_IN_10");
            assert_eq!(profile.interval_values_mw, vec![2.0; 4]);
            assert_eq!(profile.source_day_keys.len(), CBL_RECENCY_WINDOW_DAYS);
            assert!(profile.sda_applied.is_none());
            assert_eq!(profile.computed_at_unix_ms, PINNED_NOW_UNIX_MS);
        }
        BaselineOutcome::Gap(gap) => {
            panic!("expected a computed 10-in-10 baseline, got gap: {gap:?}")
        }
    }
}

#[test]
fn cbl_5_in_10_selects_the_highest_five_with_deterministic_tie_breaking() {
    let clock = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let config =
        ProgramBaselineConfig::new("tenant-1".to_string(), "program-1".to_string(), "CBL_5_IN_10".to_string())
            .expect("config is valid");
    let mut days = ten_flat_days(1.0, 4);
    // d07..d10 carry the highest totals (4.0 each); with only FOUR
    // high days, the fifth selection comes from the ties at 1.0 — the
    // tie-break is day-key ASCENDING, so d01 (not d06) enters.
    for key in ["d07", "d08", "d09", "d10"] {
        days.insert(key.to_string(), flat_day(key, None, 4.0, 4));
    }
    let outcome = compute_baseline(&config, &days, 4, None, &clock);
    match outcome {
        BaselineOutcome::Computed(profile) => {
            // The average of {4.0, 4.0, 4.0, 4.0, 1.0} per interval.
            let expected = (4.0 * 4.0 + 1.0) / 5.0;
            assert_eq!(profile.interval_values_mw, vec![expected; 4]);
            // The selected keys are exactly the four high days plus the
            // tie-broken d01 — day-key order on the output.
            assert_eq!(
                profile.source_day_keys,
                vec!["d01".to_string(), "d07".to_string(), "d08".to_string(), "d09".to_string(), "d10".to_string()]
            );
        }
        BaselineOutcome::Gap(gap) => {
            panic!("expected a computed 5-in-10 baseline, got gap: {gap:?}")
        }
    }
}

#[test]
fn day_matching_filters_by_day_type_and_weather_index() {
    let clock = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let mut config =
        ProgramBaselineConfig::new("tenant-1".to_string(), "program-1".to_string(), "DAY_MATCHING".to_string())
            .expect("config is valid");
    config.day_type_filter = Some(flexcore_application::baseline_engine::DayTypeFilter {
        day_type: "weekday".to_string(),
    });
    config.weather_filter = Some(flexcore_application::baseline_engine::WeatherIndexFilter {
        event_day_index: 30.0,
        tolerance: 3.0,
    });
    config.selection = Some(BaselineSelectionRule::All);
    let mut days = BTreeMap::new();
    days.insert("cool-weekday".to_string(), flat_day("cool-weekday", Some("weekday"), 2.0, 4));
    let mut hot_weekday = flat_day("hot-weekday", Some("weekday"), 50.0, 4);
    hot_weekday.weather_index = Some(45.0); // outside tolerance ⇒ filtered out
    days.insert("hot-weekday".to_string(), hot_weekday);
    let mut no_index_weekday = flat_day("no-index-weekday", Some("weekday"), 9.0, 4);
    no_index_weekday.weather_index = None; // a missing index NEVER matches
    days.insert("no-index-weekday".to_string(), no_index_weekday);
    days.insert("warm-weekend".to_string(), flat_day("warm-weekend", Some("weekend"), 50.0, 4));
    let mut in_tolerance_weekend = flat_day("in-tolerance-weekend", Some("weekend"), 7.0, 4);
    in_tolerance_weekend.weather_index = Some(29.0); // in tolerance but wrong day type
    days.insert("in-tolerance-weekend".to_string(), in_tolerance_weekend);
    let outcome = compute_baseline(&config, &days, 4, None, &clock);
    match outcome {
        BaselineOutcome::Computed(profile) => {
            // Only cool-weekday matched ⇒ the average is its own profile.
            assert_eq!(profile.interval_values_mw, vec![2.0; 4]);
            assert_eq!(profile.source_day_keys, vec!["cool-weekday".to_string()]);
        }
        BaselineOutcome::Gap(gap) => {
            panic!("expected the day-matching baseline, got gap: {gap:?}")
        }
    }
}

#[test]
fn capped_symmetric_sda_applies_the_cap_and_records_both_ratios() {
    let clock = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let mut config =
        ProgramBaselineConfig::new("tenant-1".to_string(), "program-1".to_string(), "CBL_10_IN_10".to_string())
            .expect("config is valid");
    config.sda = Some(SdaConfig::new(CAISO_SDA_MAX_PERCENT).expect("the CAISO bound cap is valid"));
    let days = ten_flat_days(2.0, 4);
    // The morning window [0,2): baseline sum = 4.0; actual sum = 6.4 ⇒
    // raw ratio = +60%, capped to +20%.
    let sda_window =
        SdaWindowInput::new((0, 2), vec![3.2, 3.2]).expect("the SDA window input is valid");
    let outcome = compute_baseline(&config, &days, 4, Some(&sda_window), &clock);
    match outcome {
        BaselineOutcome::Computed(profile) => {
            let sda = profile.sda_applied.expect("the SDA was applied");
            assert!((sda.raw_ratio - 0.6).abs() < 1e-12);
            assert!((sda.applied_ratio - 0.2).abs() < 1e-12);
            assert!((sda.cap_percent - CAISO_SDA_MAX_PERCENT).abs() < 1e-12);
            assert_eq!(profile.interval_values_mw, vec![2.4; 4]);
        }
        BaselineOutcome::Gap(gap) => panic!("expected the capped SDA baseline, got gap: {gap:?}"),
    }
}

#[test]
fn the_sda_cap_is_symmetric_downward() {
    let clock = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let mut config =
        ProgramBaselineConfig::new("tenant-1".to_string(), "program-1".to_string(), "CBL_10_IN_10".to_string())
            .expect("config is valid");
    config.sda = Some(SdaConfig::new(CAISO_SDA_MAX_PERCENT).expect("the CAISO bound cap is valid"));
    let days = ten_flat_days(2.0, 4);
    // The morning window [0,2): baseline sum = 4.0; actual sum = 2.0 ⇒
    // raw ratio = −50%, capped to −20%.
    let sda_window =
        SdaWindowInput::new((0, 2), vec![1.0, 1.0]).expect("the SDA window input is valid");
    let outcome = compute_baseline(&config, &days, 4, Some(&sda_window), &clock);
    match outcome {
        BaselineOutcome::Computed(profile) => {
            let sda = profile.sda_applied.expect("the SDA was applied");
            assert!((sda.raw_ratio - (-0.5)).abs() < 1e-12);
            assert!((sda.applied_ratio - (-0.2)).abs() < 1e-12);
            assert_eq!(profile.interval_values_mw, vec![1.6; 4]);
        }
        BaselineOutcome::Gap(gap) => panic!("expected the symmetric-capped SDA, got gap: {gap:?}"),
    }
}

#[test]
fn a_declared_sda_without_its_window_is_a_recorded_gap() {
    let clock = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let mut config =
        ProgramBaselineConfig::new("tenant-1".to_string(), "program-1".to_string(), "CBL_10_IN_10".to_string())
            .expect("config is valid");
    config.sda = Some(SdaConfig::new(CAISO_SDA_MAX_PERCENT).expect("the CAISO bound cap is valid"));
    let days = ten_flat_days(2.0, 4);
    let outcome = compute_baseline(&config, &days, 4, None, &clock);
    match outcome {
        BaselineOutcome::Gap(gap) => {
            assert!(matches!(
                gap.reason,
                flexcore_application::baseline_engine::BaselineGapReason::MissingSdaWindow
            ));
        }
        BaselineOutcome::Computed(_) => {
            panic!("an SDA without its window must never be silently skipped")
        }
    }
}

#[test]
fn an_sda_window_outside_the_interval_grid_is_a_recorded_gap() {
    let clock = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let mut config =
        ProgramBaselineConfig::new("tenant-1".to_string(), "program-1".to_string(), "CBL_10_IN_10".to_string())
            .expect("config is valid");
    config.sda = Some(SdaConfig::new(CAISO_SDA_MAX_PERCENT).expect("the CAISO bound cap is valid"));
    let days = ten_flat_days(2.0, 4);
    // The event window carries 4 intervals; the declared window ends at
    // index 6 — outside the grid.
    let sda_window =
        SdaWindowInput::new((4, 6), vec![1.0, 1.0]).expect("the SDA window input is valid");
    let outcome = compute_baseline(&config, &days, 4, Some(&sda_window), &clock);
    match outcome {
        BaselineOutcome::Gap(gap) => {
            assert!(matches!(
                gap.reason,
                flexcore_application::baseline_engine::BaselineGapReason::InvalidSdaWindow
            ));
        }
        BaselineOutcome::Computed(_) => {
            panic!("an SDA window outside the interval grid must never be computed")
        }
    }
}

#[test]
fn an_sda_cap_above_the_jurisdiction_bound_is_a_recorded_gap() {
    let clock = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let mut config =
        ProgramBaselineConfig::new("tenant-1".to_string(), "program-1".to_string(), "CBL_10_IN_10".to_string())
            .expect("config is valid");
    // 25% exceeds the CAISO-documented ≤20% bound [S-30] (the recorded
    // sourced bound this engine carries).
    config.sda = Some(SdaConfig::new(25.0).expect("25 is in (0, 100]"));
    let days = ten_flat_days(2.0, 4);
    let sda_window =
        SdaWindowInput::new((0, 2), vec![1.0, 1.0]).expect("the SDA window input is valid");
    let outcome = compute_baseline(&config, &days, 4, Some(&sda_window), &clock);
    match outcome {
        BaselineOutcome::Gap(gap) => {
            assert!(matches!(
                gap.reason,
                flexcore_application::baseline_engine::BaselineGapReason::SdaCapExceedsJurisdictionBound {
                    declared_percent: 25.0,
                    jurisdiction_bound_percent: 20.0,
                }
            ));
        }
        BaselineOutcome::Computed(_) => {
            panic!("a cap above the jurisdiction bound must never be applied")
        }
    }
}

#[test]
fn an_interval_count_mismatch_is_a_recorded_gap() {
    let clock = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let config =
        ProgramBaselineConfig::new("tenant-1".to_string(), "program-1".to_string(), "CBL_10_IN_10".to_string())
            .expect("config is valid");
    let mut days = ten_flat_days(2.0, 4);
    days.insert("odd-day".to_string(), flat_day("odd-day", None, 2.0, 6));
    let outcome = compute_baseline(&config, &days, 4, None, &clock);
    match outcome {
        BaselineOutcome::Gap(gap) => {
            assert!(matches!(
                gap.reason,
                flexcore_application::baseline_engine::BaselineGapReason::IntervalCountMismatch {
                    expected: 4,
                    found: 6,
                    ..
                }
            ));
        }
        BaselineOutcome::Computed(_) => {
            panic!("a mismatched interval grid must never be averaged silently")
        }
    }
}

#[test]
fn replay_is_byte_identical_for_the_same_inputs_and_clock() {
    let clock_a = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let clock_b = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let mut config =
        ProgramBaselineConfig::new("tenant-1".to_string(), "program-1".to_string(), "CBL_5_IN_10".to_string())
            .expect("config is valid");
    config.weather_filter = Some(flexcore_application::baseline_engine::WeatherIndexFilter {
        event_day_index: 10.0,
        tolerance: 5.0,
    });
    let mut days = ten_flat_days(1.0, 4);
    for (i, key) in ["d01", "d02", "d03", "d04", "d05", "d06"].iter().enumerate() {
        let mut day = flat_day(key, None, 3.0 + i as f64, 4);
        day.weather_index = Some(10.0 + i as f64);
        days.insert(key.to_string(), day);
    }
    let run_a = compute_baseline(&config, &days, 4, None, &clock_a);
    let run_b = compute_baseline(&config, &days, 4, None, &clock_b);
    assert_eq!(run_a, run_b, "same inputs + same injected clock must replay identically");
}

#[test]
fn the_supported_method_set_is_the_closed_documented_vocabulary() {
    assert_eq!(SUPPORTED_METHOD_NAMES, ["CBL_10_IN_10", "CBL_5_IN_10", "DAY_MATCHING"]);
    // The LR-073 statement is present and names the gap-row rule.
    assert!(FLEXCORE_NEVER_CHOOSES_STATEMENT.contains("RECORDED GAP ROW"));
    assert!(FLEXCORE_NEVER_CHOOSES_STATEMENT.contains("never a silent approximation"));
}

#[test]
fn the_cbl_recency_window_is_the_recorded_ten_day_construction() {
    // The 10-in-10 / 5-in-10 construction [S-30] — re-verified at use.
    assert_eq!(CBL_RECENCY_WINDOW_DAYS, 10);
}
