// P3.14 / P3.15 — Scheduling objective: acceptance tests (ledger P3.14,
//                 P3.15).
//
// Covers the ledger acceptance criteria:
//
// - the default v0.2.0 objective is NAMED and SYNTHETIC everywhere (config
//   name, provenance label, record fields, evaluation provenance) — not a
//   market product;
// - objective terms, weights, units, and sign conventions are recorded per
//   schedule (the P3.21 record fields); two schedules with different
//   objectives are never the same problem (`same_problem` is false);
// - no objective is hard-coded as universal (the name is required and
//   validated);
// - the foundations §7.5 extra terms are recorded as NOT in the v0.2.0
//   objective, with the reason, in the record output;
// - every ENABLED term traces to a named, versioned input — a term built
//   from a missing or unversioned input is a validation error;
// - the dominance warning fires on the designed fixture and is recorded in
//   the evaluation diagnostics;
// - zero-price and negative-price intervals behave as the documented
//   objective implies, explainable in the per-interval diagnostics;
// - a weight change changes the chosen schedule on the designed
//   weight-sensitive fixture.
//
// NOTE (VERIFY-BUILD, recorded): per the task constraint, cargo was NOT run
// while authoring these tests; the first `cargo test -p flexcore-application`
// run is the verification obligation.

use flexcore_application::scheduling_objective::{
    evaluate_objective, select_minimising_candidate, CandidatePortfolioSchedule,
    CurtailmentTermConfig, DegradationTermConfig, DegradationTermInput, DeviationTermConfig,
    EnergyTermConfig, ObjectiveConfig, ObjectiveError, ObjectiveInputs, ObjectiveProvenance,
    ObjectiveTerm, PriceRegime, RuleRef, ServiceTermConfig, SyntheticPriceCurve, SyntheticSeries,
    DEFAULT_DEVIATION_COEFFICIENT_PER_MW2_H, DEFAULT_DOMINANCE_THRESHOLD,
    DEFAULT_SYNTHETIC_OBJECTIVE_NAME, EXCLUDED_TERMS_STATEMENT, OBJECTIVE_SCHEMA_VERSION,
    SYNTHETIC_CURVE_ASSUMPTION_NOTE, SYNTHETIC_PRICE_CURVE_V1_HOURLY,
    SYNTHETIC_PRICE_CURVE_V1_VERSION,
};
use flexcore_domain::bess_model::{DegradationMetadata, DEGRADATION_PLACEHOLDER_STATEMENT};

/// A minimal valid config: only the energy term, a positive weight.
fn energy_only_config(name: &str) -> ObjectiveConfig {
    ObjectiveConfig::new(
        OBJECTIVE_SCHEMA_VERSION,
        name.to_string(),
        ObjectiveProvenance::Synthetic,
        EnergyTermConfig { enabled: true, weight: 1.0 },
        DegradationTermConfig { enabled: false, weight: 0.0 },
        DeviationTermConfig { enabled: false, coefficient_per_mw2_h: 0.0 },
        CurtailmentTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
        ServiceTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
        DEFAULT_DOMINANCE_THRESHOLD,
    )
    .unwrap()
}

/// The default synthetic objective (energy + deviation).
fn default_config() -> ObjectiveConfig {
    ObjectiveConfig::default_synthetic()
}

/// A P3.06 degradation metadata literal (all fields public; the statement
/// must be the recorded P3.06 one).
fn degradation_metadata(configured: bool, rate: f64, throughput_mwh: f64) -> DegradationMetadata {
    DegradationMetadata {
        configured,
        linear_cost_per_mwh_throughput: rate,
        throughput_mwh,
        cost: rate * throughput_mwh,
        is_placeholder: configured,
        placeholder_statement: DEGRADATION_PLACEHOLDER_STATEMENT,
    }
}

// --- P3.14: the default objective is named and SYNTHETIC everywhere --------

#[test]
fn default_objective_is_named_synthetic() {
    let config = default_config();
    assert_eq!(
        config.name(),
        DEFAULT_SYNTHETIC_OBJECTIVE_NAME,
        "the default objective is a NAMED synthetic objective"
    );
    assert!(config.name().starts_with("SYNTHETIC"), "the SYNTHETIC label is part of the name");
    assert_eq!(config.provenance(), ObjectiveProvenance::Synthetic);
    assert_eq!(config.provenance().as_str(), "SYNTHETIC");
    assert_eq!(config.schema_version(), OBJECTIVE_SCHEMA_VERSION);
}

#[test]
fn default_objective_enables_exactly_energy_and_deviation() {
    let config = default_config();
    assert!(config.energy().enabled, "the default objective minimises energy cost");
    assert!(config.deviation.enabled, "the default objective adds the deviation penalty");
    assert!(!config.degradation().enabled, "degradation is switchable and off by default");
    assert!(!config.curtailment().enabled, "curtailment is switchable and off by default");
    assert!(!config.service().enabled, "service is switchable and off by default");
    assert_eq!(
        config.deviation.coefficient_per_mw2_h,
        DEFAULT_DEVIATION_COEFFICIENT_PER_MW2_H
    );
}

#[test]
fn record_fields_carry_name_provenance_weights_units_and_signs() {
    let config = default_config();
    let fields = config.record_fields();
    let get = |key: &str| {
        fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| panic!("record field '{key}' missing"))
    };
    assert_eq!(get("objective.schema_version"), "1");
    assert_eq!(get("objective.name"), DEFAULT_SYNTHETIC_OBJECTIVE_NAME);
    assert_eq!(get("objective.provenance"), "SYNTHETIC");
    assert!(
        get("objective.not_a_market_product").contains("D1/D2 OPEN"),
        "the record states the objective is not a market product"
    );
    // Every §12.1 term is recorded with enablement, weight, unit, and sign.
    for term in [
        ObjectiveTerm::Energy,
        ObjectiveTerm::Degradation,
        ObjectiveTerm::Deviation,
        ObjectiveTerm::Curtailment,
        ObjectiveTerm::Service,
    ] {
        let enabled = get(&format!("term.{term}.enabled"));
        assert!(enabled == "true" || enabled == "false", "term {term} enabled flag recorded");
        let _ = get(&format!("term.{term}.weight"));
        assert!(!get(&format!("term.{term}.weight_unit")).is_empty());
        assert!(!get(&format!("term.{term}.sign_convention")).is_empty());
    }
    assert_eq!(get("term.energy.enabled"), "true");
    assert_eq!(get("term.deviation.enabled"), "true");
    assert_eq!(get("term.degradation.enabled"), "false");
}

#[test]
fn record_fields_carry_the_excluded_fundamentals_7_5_terms_with_reason() {
    let fields = default_config().record_fields();
    let excluded = fields
        .iter()
        .find(|(k, _)| k == "objective.excluded_terms")
        .map(|(_, v)| v.clone())
        .expect("the excluded-terms record field must be present");
    assert_eq!(excluded, EXCLUDED_TERMS_STATEMENT);
    for term in ["flex discomfort", "commitment deviation", "reserve shortfall", "switching costs"] {
        assert!(
            excluded.contains(term),
            "the §7.5 exclusion statement must name '{term}'"
        );
    }
    assert!(
        excluded.contains("NOT part of the v0.2.0 objective"),
        "the statement must record the exclusion itself"
    );
    assert!(
        excluded.contains("schema-version change"),
        "the statement must record that adoption is a schema-version change, not a silent field"
    );
}

#[test]
fn different_objectives_are_never_the_same_problem() {
    let a = default_config();
    let mut b = default_config();
    b.deviation.coefficient_per_mw2_h = 1.0;
    assert!(!a.same_problem(&b), "a changed weight is a different problem");
    let c = energy_only_config("SYNTHETIC_TEST_ENERGY_ONLY");
    assert!(!a.same_problem(&c), "a different name/term set is a different problem");
    let d = default_config();
    assert!(a.same_problem(&d), "identical configs declare the same problem");
    // And the records differ visibly, so schedules cannot be conflated.
    assert_ne!(a.record_fields(), b.record_fields());
}

#[test]
fn unnamed_objectives_are_rejected() {
    let err = ObjectiveConfig::new(
        OBJECTIVE_SCHEMA_VERSION,
        String::new(),
        ObjectiveProvenance::Synthetic,
        EnergyTermConfig { enabled: false, weight: 0.0 },
        DegradationTermConfig { enabled: false, weight: 0.0 },
        DeviationTermConfig { enabled: false, coefficient_per_mw2_h: 0.0 },
        CurtailmentTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
        ServiceTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
        DEFAULT_DOMINANCE_THRESHOLD,
    )
    .unwrap_err();
    assert_eq!(err, ObjectiveError::EmptyObjectiveName);
}

#[test]
fn config_validation_rejects_bad_weights_signs_and_schema() {
    let base = |energy: EnergyTermConfig,
                degradation: DegradationTermConfig,
                deviation: DeviationTermConfig,
                curtailment: CurtailmentTermConfig,
                service: ServiceTermConfig,
                schema: u32,
                threshold: f64| {
        ObjectiveConfig::new(
            schema,
            "SYNTHETIC_TEST".to_string(),
            ObjectiveProvenance::Synthetic,
            energy,
            degradation,
            deviation,
            curtailment,
            service,
            threshold,
        )
    };
    // Unsupported schema version: a term-set change cannot pass silently.
    assert_eq!(
        base(
            EnergyTermConfig { enabled: false, weight: 0.0 },
            DegradationTermConfig { enabled: false, weight: 0.0 },
            DeviationTermConfig { enabled: false, coefficient_per_mw2_h: 0.0 },
            CurtailmentTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
            ServiceTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
            OBJECTIVE_SCHEMA_VERSION + 1,
            DEFAULT_DOMINANCE_THRESHOLD,
        )
        .unwrap_err(),
        ObjectiveError::UnsupportedSchemaVersion {
            found: OBJECTIVE_SCHEMA_VERSION + 1,
            supported: OBJECTIVE_SCHEMA_VERSION,
        }
    );
    // Negative energy weight.
    assert_eq!(
        base(
            EnergyTermConfig { enabled: true, weight: -1.0 },
            DegradationTermConfig { enabled: false, weight: 0.0 },
            DeviationTermConfig { enabled: false, coefficient_per_mw2_h: 0.0 },
            CurtailmentTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
            ServiceTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
            OBJECTIVE_SCHEMA_VERSION,
            DEFAULT_DOMINANCE_THRESHOLD,
        )
        .unwrap_err(),
        ObjectiveError::NegativeWeight { term: ObjectiveTerm::Energy, value: -1.0 }
    );
    // Negative deviation coefficient (a negative penalty would reward deviation).
    assert_eq!(
        base(
            EnergyTermConfig { enabled: false, weight: 0.0 },
            DegradationTermConfig { enabled: false, weight: 0.0 },
            DeviationTermConfig { enabled: true, coefficient_per_mw2_h: -0.5 },
            CurtailmentTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
            ServiceTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
            OBJECTIVE_SCHEMA_VERSION,
            DEFAULT_DOMINANCE_THRESHOLD,
        )
        .unwrap_err(),
        ObjectiveError::NegativeWeight { term: ObjectiveTerm::Deviation, value: -0.5 }
    );
    // NaN service coefficient.
    assert_eq!(
        base(
            EnergyTermConfig { enabled: false, weight: 0.0 },
            DegradationTermConfig { enabled: false, weight: 0.0 },
            DeviationTermConfig { enabled: false, coefficient_per_mw2_h: 0.0 },
            CurtailmentTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
            ServiceTermConfig { enabled: true, coefficient_per_mwh: f64::NAN },
            OBJECTIVE_SCHEMA_VERSION,
            DEFAULT_DOMINANCE_THRESHOLD,
        )
        .unwrap_err(),
        ObjectiveError::NonFiniteWeight { term: ObjectiveTerm::Service, value: f64::NAN }
    );
    // The service sign is DECLARED: a negative (credit) coefficient is valid.
    assert!(base(
        EnergyTermConfig { enabled: false, weight: 0.0 },
        DegradationTermConfig { enabled: false, weight: 0.0 },
        DeviationTermConfig { enabled: false, coefficient_per_mw2_h: 0.0 },
        CurtailmentTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
        ServiceTermConfig { enabled: true, coefficient_per_mwh: -10.0 },
        OBJECTIVE_SCHEMA_VERSION,
        DEFAULT_DOMINANCE_THRESHOLD,
    )
    .is_ok());
    // Dominance threshold below 1.0 or non-finite is invalid.
    assert_eq!(
        base(
            EnergyTermConfig { enabled: false, weight: 0.0 },
            DegradationTermConfig { enabled: false, weight: 0.0 },
            DeviationTermConfig { enabled: false, coefficient_per_mw2_h: 0.0 },
            CurtailmentTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
            ServiceTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
            OBJECTIVE_SCHEMA_VERSION,
            0.5,
        )
        .unwrap_err(),
        ObjectiveError::InvalidDominanceThreshold { value: 0.5 }
    );
    assert_eq!(
        base(
            EnergyTermConfig { enabled: false, weight: 0.0 },
            DegradationTermConfig { enabled: false, weight: 0.0 },
            DeviationTermConfig { enabled: false, coefficient_per_mw2_h: 0.0 },
            CurtailmentTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
            ServiceTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
            OBJECTIVE_SCHEMA_VERSION,
            f64::INFINITY,
        )
        .unwrap_err(),
        ObjectiveError::InvalidDominanceThreshold { value: f64::INFINITY }
    );
}

// --- P3.15: provenance — no untracked constants ------------------------------

#[test]
fn unversioned_price_curve_is_a_validation_error() {
    let err = SyntheticPriceCurve::new("", vec![40.0]).unwrap_err();
    assert_eq!(
        err,
        ObjectiveError::UnversionedTermInput {
            term: ObjectiveTerm::Energy,
            detail: "the price curve version must be non-empty and whitespace-free".to_string(),
        }
    );
    let err = SyntheticPriceCurve::new("has space", vec![40.0]).unwrap_err();
    assert!(matches!(err, ObjectiveError::UnversionedTermInput { .. }));
}

#[test]
fn unversioned_reference_and_rules_are_validation_errors() {
    assert!(matches!(
        SyntheticSeries::new("", vec![0.0]),
        Err(ObjectiveError::UnversionedTermInput { term: ObjectiveTerm::Deviation, .. })
    ));
    assert_eq!(
        RuleRef::new("curtail-rule", "").unwrap_err(),
        ObjectiveError::InvalidRuleRef { field: "version" }
    );
    assert_eq!(
        RuleRef::new("", "v1").unwrap_err(),
        ObjectiveError::InvalidRuleRef { field: "name" }
    );
}

#[test]
fn enabled_term_without_input_is_a_validation_error() {
    let config = default_config(); // energy + deviation enabled
    let inputs = ObjectiveInputs::new(3600); // nothing supplied
    let candidate = CandidatePortfolioSchedule {
        label: "c".to_string(),
        net_export_mw: vec![1.0],
    };
    // The energy term is checked first.
    assert_eq!(
        evaluate_objective(&config, &candidate, &inputs).unwrap_err(),
        ObjectiveError::TermInputMissing {
            term: ObjectiveTerm::Energy,
            detail: "no price curve was supplied".to_string(),
        }
    );
    // With the curve supplied, the missing reference is reported.
    let inputs = ObjectiveInputs {
        price_curve: Some(SyntheticPriceCurve::new("test-curve", vec![40.0]).unwrap()),
        ..ObjectiveInputs::new(3600)
    };
    assert_eq!(
        evaluate_objective(&config, &candidate, &inputs).unwrap_err(),
        ObjectiveError::TermInputMissing {
            term: ObjectiveTerm::Deviation,
            detail: "no reference trajectory was supplied".to_string(),
        }
    );
}

#[test]
fn enabled_degradation_without_metadata_is_a_validation_error() {
    let config = ObjectiveConfig::new(
        OBJECTIVE_SCHEMA_VERSION,
        "SYNTHETIC_TEST_DEG".to_string(),
        ObjectiveProvenance::Synthetic,
        EnergyTermConfig { enabled: false, weight: 0.0 },
        DegradationTermConfig { enabled: true, weight: 1.0 },
        DeviationTermConfig { enabled: false, coefficient_per_mw2_h: 0.0 },
        CurtailmentTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
        ServiceTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
        DEFAULT_DOMINANCE_THRESHOLD,
    )
    .unwrap();
    let candidate = CandidatePortfolioSchedule { label: "c".to_string(), net_export_mw: vec![0.0] };
    let err = evaluate_objective(&config, &candidate, &ObjectiveInputs::new(3600)).unwrap_err();
    assert!(matches!(
        err,
        ObjectiveError::TermInputMissing { term: ObjectiveTerm::Degradation, .. }
    ));
}

#[test]
fn degradation_metadata_outside_the_p3_06_statement_is_rejected() {
    let config = ObjectiveConfig::new(
        OBJECTIVE_SCHEMA_VERSION,
        "SYNTHETIC_TEST_DEG".to_string(),
        ObjectiveProvenance::Synthetic,
        EnergyTermConfig { enabled: false, weight: 0.0 },
        DegradationTermConfig { enabled: true, weight: 1.0 },
        DeviationTermConfig { enabled: false, coefficient_per_mw2_h: 0.0 },
        CurtailmentTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
        ServiceTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
        DEFAULT_DOMINANCE_THRESHOLD,
    )
    .unwrap();
    let candidate = CandidatePortfolioSchedule { label: "c".to_string(), net_export_mw: vec![0.0] };
    let restated = DegradationMetadata {
        configured: true,
        linear_cost_per_mwh_throughput: 1.0,
        throughput_mwh: 1.0,
        cost: 1.0,
        is_placeholder: true,
        placeholder_statement: "my own statement", // NOT the recorded P3.06 statement
    };
    let inputs = ObjectiveInputs {
        degradation: vec![DegradationTermInput {
            asset_id: "bess-1".to_string(),
            metadata: restated,
        }],
        ..ObjectiveInputs::new(3600)
    };
    let err = evaluate_objective(&config, &candidate, &inputs).unwrap_err();
    assert!(
        matches!(err, ObjectiveError::TermInputMissing { term: ObjectiveTerm::Degradation, .. }),
        "metadata whose placeholder statement does not match the recorded P3.06 statement is \
         not the P3.06 input and must be rejected: {err}"
    );
}

#[test]
fn enabled_curtailment_and_service_require_rule_and_series() {
    let config = ObjectiveConfig::new(
        OBJECTIVE_SCHEMA_VERSION,
        "SYNTHETIC_TEST_CURT".to_string(),
        ObjectiveProvenance::Synthetic,
        EnergyTermConfig { enabled: false, weight: 0.0 },
        DegradationTermConfig { enabled: false, weight: 0.0 },
        DeviationTermConfig { enabled: false, coefficient_per_mw2_h: 0.0 },
        CurtailmentTermConfig { enabled: true, coefficient_per_mwh: 20.0 },
        ServiceTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
        DEFAULT_DOMINANCE_THRESHOLD,
    )
    .unwrap();
    let candidate = CandidatePortfolioSchedule { label: "c".to_string(), net_export_mw: vec![1.0] };
    assert!(matches!(
        evaluate_objective(&config, &candidate, &ObjectiveInputs::new(3600)).unwrap_err(),
        ObjectiveError::TermInputMissing { term: ObjectiveTerm::Curtailment, .. }
    ));
}

#[test]
fn provenance_records_carry_versions_and_the_synthetic_assumption() {
    let config = default_config();
    let curve = SyntheticPriceCurve::new(SYNTHETIC_PRICE_CURVE_V1_VERSION, vec![42.0, 38.0])
        .unwrap();
    let reference = SyntheticSeries::new("synthetic-reference-trajectory-v1", vec![0.0, 0.0])
        .unwrap();
    let inputs = ObjectiveInputs {
        price_curve: Some(curve),
        reference: Some(reference),
        ..ObjectiveInputs::new(3600)
    };
    let candidate = CandidatePortfolioSchedule { label: "c".to_string(), net_export_mw: vec![1.0, 1.0] };
    let evaluation = evaluate_objective(&config, &candidate, &inputs).unwrap();
    let energy = evaluation
        .term_provenance
        .iter()
        .find(|p| p.term == ObjectiveTerm::Energy)
        .expect("the enabled energy term must carry a provenance record");
    assert_eq!(energy.version, SYNTHETIC_PRICE_CURVE_V1_VERSION);
    assert_eq!(energy.note, SYNTHETIC_CURVE_ASSUMPTION_NOTE);
    assert!(
        energy.note.contains("not a market price"),
        "the synthetic label and the no-market claim travel in the provenance note"
    );
    let deviation = evaluation
        .term_provenance
        .iter()
        .find(|p| p.term == ObjectiveTerm::Deviation)
        .expect("the enabled deviation term must carry a provenance record");
    assert_eq!(deviation.version, "synthetic-reference-trajectory-v1");
}

// --- P3.14: evaluation totals match the hand-computed fixture ----------------

#[test]
fn evaluation_totals_match_the_hand_computed_fixture() {
    // Two 1-hour intervals. Prices 40 and −10 currency-units/MWh.
    // Candidate exports [2, 1] MW; reference [1, 1] MW; k_dev = 50.
    //   C_energy  = −(40·2·1) − (−10·1·1) = −80 + 10 = −70
    //   C_dev     = 50·((2−1)² + (1−1)²)·1 = 50
    //   total     = −20
    let config = default_config();
    let curve = SyntheticPriceCurve::new("test-curve", vec![40.0, -10.0]).unwrap();
    let reference = SyntheticSeries::new("test-reference", vec![1.0, 1.0]).unwrap();
    let inputs = ObjectiveInputs {
        price_curve: Some(curve),
        reference: Some(reference),
        ..ObjectiveInputs::new(3600)
    };
    let candidate = CandidatePortfolioSchedule {
        label: "two-interval".to_string(),
        net_export_mw: vec![2.0, 1.0],
    };
    let evaluation = evaluate_objective(&config, &candidate, &inputs).unwrap();
    assert!((evaluation.totals.energy - (-70.0)).abs() < 1e-12);
    assert!((evaluation.totals.deviation - 50.0).abs() < 1e-12);
    assert!((evaluation.total - (-20.0)).abs() < 1e-12);
    assert_eq!(evaluation.interval_count, 2);
    assert_eq!(evaluation.objective_name, DEFAULT_SYNTHETIC_OBJECTIVE_NAME);
    assert_eq!(evaluation.provenance, ObjectiveProvenance::Synthetic);
    // Per-interval breakdown matches the same arithmetic.
    assert!((evaluation.per_interval[0].energy - (-80.0)).abs() < 1e-12);
    assert!((evaluation.per_interval[1].energy - 10.0).abs() < 1e-12);
    assert!((evaluation.per_interval[0].deviation - 50.0).abs() < 1e-12);
    assert!((evaluation.per_interval[1].deviation - 0.0).abs() < 1e-12);
}

#[test]
fn degradation_total_is_the_weighted_p3_06_placeholder_cost() {
    let config = ObjectiveConfig::new(
        OBJECTIVE_SCHEMA_VERSION,
        "SYNTHETIC_TEST_DEG".to_string(),
        ObjectiveProvenance::Synthetic,
        EnergyTermConfig { enabled: false, weight: 0.0 },
        DegradationTermConfig { enabled: true, weight: 2.0 },
        DeviationTermConfig { enabled: false, coefficient_per_mw2_h: 0.0 },
        CurtailmentTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
        ServiceTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
        DEFAULT_DOMINANCE_THRESHOLD,
    )
    .unwrap();
    let candidate = CandidatePortfolioSchedule { label: "c".to_string(), net_export_mw: vec![0.0] };
    let inputs = ObjectiveInputs {
        degradation: vec![DegradationTermInput {
            asset_id: "bess-1".to_string(),
            metadata: degradation_metadata(true, 7.5, 4.0),
        }],
        ..ObjectiveInputs::new(3600)
    };
    let evaluation = evaluate_objective(&config, &candidate, &inputs).unwrap();
    assert!((evaluation.totals.degradation - 2.0 * 7.5 * 4.0).abs() < 1e-12);
    let note = evaluation.degradation_note.expect("the degradation note records the placeholder");
    assert!(note.contains("PLACEHOLDER"), "the P3.06 placeholder statement travels to the diagnostics");
    let provenance = evaluation
        .term_provenance
        .iter()
        .find(|p| p.term == ObjectiveTerm::Degradation)
        .expect("degradation provenance recorded");
    assert!(provenance.note.contains("is_placeholder = true"));
    assert!(provenance.note.contains("bess-1"));
}

#[test]
fn degradation_term_with_unconfigured_metadata_contributes_zero_and_says_so() {
    let config = ObjectiveConfig::new(
        OBJECTIVE_SCHEMA_VERSION,
        "SYNTHETIC_TEST_DEG".to_string(),
        ObjectiveProvenance::Synthetic,
        EnergyTermConfig { enabled: false, weight: 0.0 },
        DegradationTermConfig { enabled: true, weight: 1.0 },
        DeviationTermConfig { enabled: false, coefficient_per_mw2_h: 0.0 },
        CurtailmentTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
        ServiceTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
        DEFAULT_DOMINANCE_THRESHOLD,
    )
    .unwrap();
    let candidate = CandidatePortfolioSchedule { label: "c".to_string(), net_export_mw: vec![0.0] };
    let inputs = ObjectiveInputs {
        degradation: vec![DegradationTermInput {
            asset_id: "bess-1".to_string(),
            metadata: degradation_metadata(false, 0.0, 0.0),
        }],
        ..ObjectiveInputs::new(3600)
    };
    let evaluation = evaluate_objective(&config, &candidate, &inputs).unwrap();
    assert_eq!(evaluation.totals.degradation, 0.0);
    let note = evaluation.degradation_note.unwrap();
    assert!(
        note.contains("0 with the term configured"),
        "the note records that the placeholder was NOT configured (never a silent zero)"
    );
}

// --- P3.15: magnitude dominance warning --------------------------------------

#[test]
fn dominance_warning_fires_on_the_designed_fixture() {
    // Energy: prices 40, export 1 MW, 1 h -> C_energy = −40.
    // Degradation: placeholder cost 1_000_000 currency-units (rate 10_000
    // per MWh over 100 MWh of throughput).
    // Ratio |1e6| / |40| = 25_000 >= 100 -> warning on degradation.
    let config = ObjectiveConfig::new(
        OBJECTIVE_SCHEMA_VERSION,
        "SYNTHETIC_TEST_DOMINANCE".to_string(),
        ObjectiveProvenance::Synthetic,
        EnergyTermConfig { enabled: true, weight: 1.0 },
        DegradationTermConfig { enabled: true, weight: 1.0 },
        DeviationTermConfig { enabled: false, coefficient_per_mw2_h: 0.0 },
        CurtailmentTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
        ServiceTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
        DEFAULT_DOMINANCE_THRESHOLD,
    )
    .unwrap();
    let candidate = CandidatePortfolioSchedule { label: "c".to_string(), net_export_mw: vec![1.0] };
    let inputs = ObjectiveInputs {
        price_curve: Some(SyntheticPriceCurve::new("test-curve", vec![40.0]).unwrap()),
        degradation: vec![DegradationTermInput {
            asset_id: "bess-1".to_string(),
            metadata: degradation_metadata(true, 10_000.0, 100.0),
        }],
        ..ObjectiveInputs::new(3600)
    };
    let evaluation = evaluate_objective(&config, &candidate, &inputs).unwrap();
    assert_eq!(evaluation.scaling_warnings.len(), 1, "exactly the degradation term dominates");
    let warning = &evaluation.scaling_warnings[0];
    assert_eq!(warning.term, ObjectiveTerm::Degradation);
    assert_eq!(warning.other_term, ObjectiveTerm::Energy);
    assert!(warning.ratio >= warning.threshold);
    assert_eq!(warning.threshold, DEFAULT_DOMINANCE_THRESHOLD);
    assert!(warning.review_required);
    assert!(warning.statement.contains("P3.19"), "the statement scopes solver scaling to P3.19");
}

#[test]
fn no_dominance_warning_on_balanced_terms_or_single_terms() {
    // Balanced: both terms of comparable magnitude -> no warning.
    let config = ObjectiveConfig::new(
        OBJECTIVE_SCHEMA_VERSION,
        "SYNTHETIC_TEST_BALANCED".to_string(),
        ObjectiveProvenance::Synthetic,
        EnergyTermConfig { enabled: true, weight: 1.0 },
        DegradationTermConfig { enabled: true, weight: 1.0 },
        DeviationTermConfig { enabled: false, coefficient_per_mw2_h: 0.0 },
        CurtailmentTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
        ServiceTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
        DEFAULT_DOMINANCE_THRESHOLD,
    )
    .unwrap();
    let candidate = CandidatePortfolioSchedule { label: "c".to_string(), net_export_mw: vec![1.0] };
    let inputs = ObjectiveInputs {
        price_curve: Some(SyntheticPriceCurve::new("test-curve", vec![40.0]).unwrap()),
        degradation: vec![DegradationTermInput {
            asset_id: "bess-1".to_string(),
            metadata: degradation_metadata(true, 80.0, 1.0), // cost 80 vs energy 40
        }],
        ..ObjectiveInputs::new(3600)
    };
    let evaluation = evaluate_objective(&config, &candidate, &inputs).unwrap();
    assert!(evaluation.scaling_warnings.is_empty(), "ratio 2.0 is below the threshold 100.0");

    // Single enabled nonzero term: nothing to dominate against (recorded rule).
    let solo = energy_only_config("SYNTHETIC_TEST_SOLO");
    let inputs = ObjectiveInputs {
        price_curve: Some(SyntheticPriceCurve::new("test-curve", vec![1.0e9]).unwrap()),
        ..ObjectiveInputs::new(3600)
    };
    let evaluation = evaluate_objective(&solo, &candidate, &inputs).unwrap();
    assert!(evaluation.scaling_warnings.is_empty());
}

// --- P3.15: zero- and negative-price behaviour --------------------------------

#[test]
fn zero_price_interval_contributes_no_energy_cost_and_is_explained() {
    let config = energy_only_config("SYNTHETIC_TEST_ZERO");
    let inputs = ObjectiveInputs {
        price_curve: Some(SyntheticPriceCurve::new("test-curve", vec![0.0, 30.0]).unwrap()),
        ..ObjectiveInputs::new(3600)
    };
    let candidate = CandidatePortfolioSchedule { label: "c".to_string(), net_export_mw: vec![3.0, 3.0] };
    let evaluation = evaluate_objective(&config, &candidate, &inputs).unwrap();
    assert_eq!(evaluation.per_interval[0].price_regime, Some(PriceRegime::Zero));
    assert_eq!(evaluation.per_interval[0].energy, 0.0, "zero price: no energy contribution");
    assert_eq!(evaluation.per_interval[1].price_regime, Some(PriceRegime::Positive));
    assert!((evaluation.per_interval[1].energy - (-90.0)).abs() < 1e-12);
    let diagnostics = &evaluation.per_interval[0].diagnostics;
    assert!(
        diagnostics.contains("zero price"),
        "the diagnostics explain the zero-price behaviour: {diagnostics}"
    );
    assert!(
        diagnostics.contains("governed by the other enabled terms"),
        "the diagnostics state what governs the schedule on a zero-price interval"
    );
}

#[test]
fn negative_price_interval_flips_the_energy_signs_and_is_explained() {
    // Documented behaviour: at a negative price, export incurs a cost and
    // import earns a credit — the objective rewards consumption there.
    let config = energy_only_config("SYNTHETIC_TEST_NEGATIVE");
    let inputs = ObjectiveInputs {
        price_curve: Some(SyntheticPriceCurve::new("test-curve", vec![-12.0]).unwrap()),
        ..ObjectiveInputs::new(3600)
    };
    // Exporting 2 MW at price −12: C_energy = −(−12·2·1) = +24 (a cost).
    let exporter = CandidatePortfolioSchedule { label: "export".to_string(), net_export_mw: vec![2.0] };
    let evaluation = evaluate_objective(&config, &exporter, &inputs).unwrap();
    assert_eq!(evaluation.per_interval[0].price_regime, Some(PriceRegime::Negative));
    assert!((evaluation.totals.energy - 24.0).abs() < 1e-12);
    assert!(
        evaluation.per_interval[0].diagnostics.contains("negative price"),
        "the diagnostics explain the negative-price behaviour"
    );
    assert!(
        evaluation.per_interval[0].diagnostics.contains("rewards consumption"),
        "the diagnostics state that the objective rewards consumption on the interval"
    );
    // Importing 2 MW at price −12: C_energy = −(−12·(−2)·1) = −24 (a credit).
    let importer = CandidatePortfolioSchedule { label: "import".to_string(), net_export_mw: vec![-2.0] };
    let evaluation = evaluate_objective(&config, &importer, &inputs).unwrap();
    assert!((evaluation.totals.energy - (-24.0)).abs() < 1e-12);
    assert!(
        evaluation.totals.energy < evaluation_objective_total_of(&config, &exporter, &inputs),
        "consuming on a negative-price interval scores strictly better than exporting — the \
         documented objective's behaviour"
    );
}

fn evaluation_objective_total_of(
    config: &ObjectiveConfig,
    candidate: &CandidatePortfolioSchedule,
    inputs: &ObjectiveInputs,
) -> f64 {
    evaluate_objective(config, candidate, inputs).unwrap().total
}

#[test]
fn default_curve_records_the_zero_and_negative_price_intervals() {
    // The recorded v1 curve is designed to exercise both regimes: hour 12
    // is the zero-price interval, hour 20 the negative-price interval.
    assert_eq!(SYNTHETIC_PRICE_CURVE_V1_HOURLY[12], 0.0, "the recorded zero-price hour");
    assert!(SYNTHETIC_PRICE_CURVE_V1_HOURLY[20] < 0.0, "the recorded negative-price hour");
    let curve = SyntheticPriceCurve::v1_default(96, 900); // 24 h over 15 min
    assert_eq!(curve.version(), SYNTHETIC_PRICE_CURVE_V1_VERSION);
    assert_eq!(curve.prices_per_interval().len(), 96);
    assert_eq!(curve.prices_per_interval()[48], 0.0, "interval 48 = hour 12");
    assert!(curve.prices_per_interval()[80] < 0.0, "interval 80 = hour 20");
}

// --- P3.14: weight sensitivity changes the chosen schedule --------------------

#[test]
fn weight_sensitivity_changes_the_chosen_schedule() {
    // Designed weight-sensitive fixture: two 1-hour intervals, prices
    // [42, 38]; candidate A exports 5 MW in interval 0, candidate B exports
    // 5 MW in interval 1; the reference trajectory is B's shape [0, 5].
    //
    // Default weights (w_energy = 1, k_dev = 50):
    //   A: energy −(42·5) = −210; deviation 50·((5−0)² + (0−5)²) = 2500 → 2290
    //   B: energy −(38·5) = −190; deviation 0                      → −190
    //   → B is chosen.
    // With ONLY the deviation weight changed to 0.1:
    //   A: −210 + 0.1·50 = −205
    //   B: −190 + 0     = −190
    //   → A is chosen.
    // One weight change flips the selection — the ledger's requirement.
    let candidates = vec![
        CandidatePortfolioSchedule {
            label: "A-export-hour-0".to_string(),
            net_export_mw: vec![5.0, 0.0],
        },
        CandidatePortfolioSchedule {
            label: "B-export-hour-1".to_string(),
            net_export_mw: vec![0.0, 5.0],
        },
    ];
    let inputs = || ObjectiveInputs {
        price_curve: Some(SyntheticPriceCurve::new("test-curve", vec![42.0, 38.0]).unwrap()),
        reference: Some(SyntheticSeries::new("test-reference", vec![0.0, 5.0]).unwrap()),
        ..ObjectiveInputs::new(3600)
    };

    let default = default_config();
    let outcome =
        select_minimising_candidate(&default, &candidates, &inputs()).unwrap();
    assert_eq!(
        outcome.chosen_label, "B-export-hour-1",
        "under the default weights the deviation penalty dominates and the schedule \
         follows the reference"
    );
    let default_total_b = outcome.evaluations[outcome.chosen_index].total;

    // The flipped config differs ONLY in the deviation weight.
    let flipped = ObjectiveConfig::new(
        OBJECTIVE_SCHEMA_VERSION,
        DEFAULT_SYNTHETIC_OBJECTIVE_NAME.to_string(),
        ObjectiveProvenance::Synthetic,
        EnergyTermConfig { enabled: true, weight: 1.0 },
        DegradationTermConfig { enabled: false, weight: 0.0 },
        DeviationTermConfig { enabled: true, coefficient_per_mw2_h: 0.1 },
        CurtailmentTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
        ServiceTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
        DEFAULT_DOMINANCE_THRESHOLD,
    )
    .unwrap();
    assert!(
        !default.same_problem(&flipped),
        "a changed weight is a different problem, recorded as such"
    );
    assert_ne!(default.record_fields(), flipped.record_fields());
    let outcome = select_minimising_candidate(&flipped, &candidates, &inputs()).unwrap();
    assert_eq!(
        outcome.chosen_label, "A-export-hour-0",
        "with the deviation weight reduced, the energy term decides and the schedule \
         moves to the higher-price interval"
    );
    let flipped_total_a = outcome.evaluations[outcome.chosen_index].total;
    assert!((flipped_total_a - (-205.0)).abs() < 1e-9);
    assert!((default_total_b - (-190.0)).abs() < 1e-9);
}

// --- Validation of series and candidates -------------------------------------

#[test]
fn series_length_and_value_validation() {
    let config = default_config();
    let candidate = CandidatePortfolioSchedule { label: "c".to_string(), net_export_mw: vec![1.0, 1.0] };
    // Reference shorter than the schedule.
    let inputs = ObjectiveInputs {
        price_curve: Some(SyntheticPriceCurve::new("test-curve", vec![40.0, 40.0]).unwrap()),
        reference: Some(SyntheticSeries::new("test-reference", vec![0.0]).unwrap()),
        ..ObjectiveInputs::new(3600)
    };
    assert_eq!(
        evaluate_objective(&config, &candidate, &inputs).unwrap_err(),
        ObjectiveError::SeriesLengthMismatch { series: "reference", expected: 2, found: 1 }
    );
    // Non-finite price.
    let inputs = ObjectiveInputs {
        price_curve: Some(SyntheticPriceCurve::new("test-curve", vec![40.0, f64::NAN]).unwrap()),
        reference: Some(SyntheticSeries::new("test-reference", vec![0.0, 0.0]).unwrap()),
        ..ObjectiveInputs::new(3600)
    };
    assert_eq!(
        evaluate_objective(&config, &candidate, &inputs).unwrap_err(),
        ObjectiveError::NonFiniteSeriesValue {
            series: "price_curve",
            interval: 1,
            value: f64::NAN,
        }
    );
    // Non-finite candidate value.
    let inputs = ObjectiveInputs {
        price_curve: Some(SyntheticPriceCurve::new("test-curve", vec![40.0, 40.0]).unwrap()),
        reference: Some(SyntheticSeries::new("test-reference", vec![0.0, 0.0]).unwrap()),
        ..ObjectiveInputs::new(3600)
    };
    let bad = CandidatePortfolioSchedule { label: "c".to_string(), net_export_mw: vec![1.0, f64::INFINITY] };
    assert_eq!(
        evaluate_objective(&config, &bad, &inputs).unwrap_err(),
        ObjectiveError::NonFiniteCandidateValue { label: "c".to_string(), interval: 1, value: f64::INFINITY }
    );
    // Zero interval length.
    assert_eq!(
        evaluate_objective(&config, &candidate, &ObjectiveInputs::new(0)).unwrap_err(),
        ObjectiveError::NonPositiveIntervalSeconds
    );
}

#[test]
fn negative_curtailment_magnitude_is_rejected() {
    let config = ObjectiveConfig::new(
        OBJECTIVE_SCHEMA_VERSION,
        "SYNTHETIC_TEST_CURT".to_string(),
        ObjectiveProvenance::Synthetic,
        EnergyTermConfig { enabled: false, weight: 0.0 },
        DegradationTermConfig { enabled: false, weight: 0.0 },
        DeviationTermConfig { enabled: false, coefficient_per_mw2_h: 0.0 },
        CurtailmentTermConfig { enabled: true, coefficient_per_mwh: 20.0 },
        ServiceTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
        DEFAULT_DOMINANCE_THRESHOLD,
    )
    .unwrap();
    let candidate = CandidatePortfolioSchedule { label: "c".to_string(), net_export_mw: vec![1.0] };
    let inputs = ObjectiveInputs {
        curtailment_rule: Some(RuleRef::new("synthetic-curtailment-rule", "v1").unwrap()),
        curtailed_mw: Some(vec![-0.5]),
        ..ObjectiveInputs::new(3600)
    };
    assert_eq!(
        evaluate_objective(&config, &candidate, &inputs).unwrap_err(),
        ObjectiveError::NegativeMagnitude { series: "curtailed_mw", interval: 0, value: -0.5 }
    );
}

#[test]
fn selection_validates_the_candidate_set() {
    let config = default_config();
    let inputs = ObjectiveInputs::new(3600);
    assert_eq!(
        select_minimising_candidate(&config, &[], &inputs).unwrap_err(),
        ObjectiveError::EmptyCandidateSet
    );
    let candidates = vec![
        CandidatePortfolioSchedule { label: "a".to_string(), net_export_mw: vec![1.0, 1.0] },
        CandidatePortfolioSchedule { label: "b".to_string(), net_export_mw: vec![1.0] },
    ];
    assert_eq!(
        select_minimising_candidate(&config, &candidates, &inputs).unwrap_err(),
        ObjectiveError::CandidateSetInconsistent { label: "b".to_string(), expected: 2, found: 1 }
    );
}
