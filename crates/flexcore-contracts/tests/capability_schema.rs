// P3.01 — Capability-schema contract test suite (wire-level, serde_json).
//
// Tests the P3.01 frozen capability-model contract from OUTSIDE the crate, at
// the serialized boundary a real adapter sees (ledger P3.01 acceptance: the
// schema exists and is versioned; the vocabulary rejects scaled units; every
// constraint carries units, sign, provenance, and a validity window).
//
// Decision record: `ledger/phase-03-flexibility-scheduling/01_04_capability_contract.md`.

use flexcore_contracts::capability::{
    CAPABILITY_SCHEMA_VERSION, CapabilityModel, ConstraintExpression, ConstraintKind,
    ConstraintSignConvention, ConstraintUnit, DeclaredConstraint, Provenance,
};
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{ValidFrom, ValidUntil, ValidityWindow};

const ASSET: &str = "0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8b";

fn window(from_millis: i64, until_millis: i64) -> ValidityWindow {
    ValidityWindow::new(
        ValidFrom::from_unix_millis(from_millis).unwrap(),
        ValidUntil::from_unix_millis(until_millis).unwrap(),
    )
    .unwrap()
}

fn p_constraint(lower: f64, upper: f64) -> DeclaredConstraint {
    DeclaredConstraint::new(
        ConstraintKind::ActivePower,
        ConstraintUnit::Megawatt,
        ConstraintSignConvention::ExportPositive,
        Provenance::Measured,
        window(0, 3_600_000),
        ConstraintExpression::Interval { lower: Some(lower), upper: Some(upper) },
    )
    .unwrap()
}

fn sample_model_json() -> serde_json::Value {
    let set = flexcore_contracts::capability::TimeIndexedFeasibleSet::new(
        time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
        900,
        4,
        vec![
            flexcore_contracts::capability::IntervalConstraints {
                interval_index: 0,
                constraints: vec![p_constraint(-2.0, 3.0)],
            },
            flexcore_contracts::capability::IntervalConstraints {
                interval_index: 1,
                constraints: vec![p_constraint(-2.0, 3.0)],
            },
            flexcore_contracts::capability::IntervalConstraints {
                interval_index: 2,
                constraints: vec![p_constraint(-2.0, 3.0)],
            },
            flexcore_contracts::capability::IntervalConstraints {
                interval_index: 3,
                constraints: vec![p_constraint(-2.0, 3.0)],
            },
        ],
    )
    .unwrap();
    let model =
        CapabilityModel::new(CAPABILITY_SCHEMA_VERSION, FlexUuid::parse(ASSET).unwrap(), window(0, 3_600_000), set)
            .unwrap();
    serde_json::to_value(&model).unwrap()
}

#[test]
fn schema_version_is_frozen_at_one() {
    assert_eq!(CAPABILITY_SCHEMA_VERSION, 1);
}

#[test]
fn serialized_payload_carries_the_schema_version_field() {
    let json = sample_model_json();
    assert_eq!(json["schema_version"], serde_json::json!(1));
}

#[test]
fn payload_round_trips_through_the_wire_format() {
    let json = sample_model_json();
    let model: CapabilityModel = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(model.asset_id, FlexUuid::parse(ASSET).unwrap());
    assert_eq!(model.feasible_set.interval_count, 4);
    let again = serde_json::to_value(&model).unwrap();
    assert_eq!(again, json);
}

#[test]
fn wire_payload_rejects_a_foreign_schema_version() {
    let mut json = sample_model_json();
    json["schema_version"] = serde_json::json!(99);
    let err = serde_json::from_value::<CapabilityModel>(json).unwrap_err();
    assert!(err.to_string().contains("schema version 99 is not supported"));
}

#[test]
fn wire_payload_rejects_a_non_utc_horizon_start() {
    let mut json = sample_model_json();
    json["feasible_set"]["horizon_start"] = serde_json::json!("2026-01-01T12:00:00+02:00");
    let err = serde_json::from_value::<CapabilityModel>(json).unwrap_err();
    assert!(err.to_string().contains("UTC"));
}

#[test]
fn wire_payload_rejects_structural_violations() {
    // Inverted interval bound.
    let mut json = sample_model_json();
    json["feasible_set"]["slices"][0]["constraints"][0]["expression"] =
        serde_json::json!({ "interval": { "lower": 4.0, "upper": 1.0 } });
    assert!(serde_json::from_value::<CapabilityModel>(json).is_err());

    // Empty declared mode set (u must be declared non-empty, §11.1).
    let mut json = sample_model_json();
    json["feasible_set"]["slices"][0]["constraints"][0]["expression"] =
        serde_json::json!({ "ModeSet": { "modes": [] } });
    assert!(serde_json::from_value::<CapabilityModel>(json).is_err());
}

#[test]
fn wire_payload_rejects_inverted_validity_windows() {
    let mut json = sample_model_json();
    json["feasible_set"]["slices"][0]["constraints"][0]["validity"] = serde_json::json!({
        "valid_from": "1970-01-01T01:00:00Z",
        "valid_until": "1970-01-01T00:00:00Z",
    });
    let err = serde_json::from_value::<CapabilityModel>(json).unwrap_err();
    assert!(err.to_string().contains("valid_from"));
}

#[test]
fn interval_boundaries_from_a_deserialized_payload() {
    let model: CapabilityModel = serde_json::from_value(sample_model_json()).unwrap();
    let start = model.feasible_set.horizon_start;
    let (s0, e0) = model.feasible_set.interval_span(0).unwrap();
    assert_eq!(s0, start);
    assert_eq!(e0 - s0, time::Duration::seconds(900));
    // Start-inclusive, end-exclusive: interval 1 starts exactly at the
    // exclusive end of interval 0.
    let (s1, _) = model.feasible_set.interval_span(1).unwrap();
    assert_eq!(s1, e0);
    let (s_last, e_last) = model.feasible_set.interval_span(3).unwrap();
    assert_eq!(e_last, model.feasible_set.horizon_end());
    assert_eq!(
        model.feasible_set.interval_span(4).unwrap_err().to_string(),
        "capability model interval index 4 is outside the horizon (interval_count = 4)"
    );
    // Silence unused warning paths: s_last is verified above via e_last.
    assert_eq!(s_last, e_last - time::Duration::seconds(900));
}

#[test]
fn the_wire_vocabulary_has_no_scaled_unit() {
    // kW/kWh cannot be expressed: no variant serializes to them and no
    // string deserializes into a scaled unit (report §10.2: scaled units are
    // display-only and converted at the adapter).
    for u in ConstraintUnit::ALL {
        let json = serde_json::to_value(&u).unwrap();
        assert!(serde_json::from_value::<ConstraintUnit>(json).is_ok());
    }
    assert!(serde_json::from_value::<ConstraintUnit>(serde_json::json!("kW")).is_err());
    assert!(serde_json::from_value::<ConstraintUnit>(serde_json::json!("kWh")).is_err());
    assert!(serde_json::from_value::<ConstraintUnit>(serde_json::json!("MW")).is_ok());
}

#[test]
fn every_constraint_field_group_is_present_on_the_wire() {
    let json = sample_model_json();
    let constraint = &json["feasible_set"]["slices"][0]["constraints"][0];
    for field in ["kind", "unit", "sign", "provenance", "validity", "expression"] {
        assert!(
            constraint.get(field).is_some(),
            "constraint payload must carry '{field}' (units, sign, provenance, validity window are frozen, P3.01)"
        );
    }
    assert_eq!(constraint["provenance"], serde_json::json!("MEASURED"));
    assert_eq!(constraint["sign"], serde_json::json!("EXPORT_POSITIVE"));
}

#[test]
fn provenance_vocabulary_round_trips_on_the_wire() {
    for p in Provenance::ALL {
        let json = serde_json::to_value(&p).unwrap();
        let back: Provenance = serde_json::from_value(json).unwrap();
        assert_eq!(back, p);
    }
    assert!(serde_json::from_value::<Provenance>(serde_json::json!("UNKNOWN")).is_ok());
    assert!(serde_json::from_value::<Provenance>(serde_json::json!("ASSUMED")).is_err());
}
