// P19.17 — the RETENTION DESIGN-CONTRACT suite (P13.13), WRITTEN-UNRUN
// (conventions §1 item 3): never compiled or executed — execution is the
// human build/test gate.
//
// HONEST SCOPE STATEMENT (read before trusting anything here):
//
// Retention ENFORCEMENT is DESIGN-ONLY in this workspace. The P13.13
// record (`docs/phases-11-20/phase-13-enterprise-identity-integration/
// records/25_p13_13_gdpr_privacy.md` §2/§4) and the P1.10 type's own
// header (`crates/flexcore-storage/src/retention.rs`) state that
// enforcement of a retention setting (partition detach/drop, deletion
// jobs, backup purge) is infrastructure work owned by the deployment and
// the adapters — NO enforcement module exists in any crate, and this file
// deliberately does NOT reference one (no nonexistent API is tested,
// P19.17 hard-ban 8). What DOES exist — and what these tests pin — is the
// CONFIGURATION CONTRACT the design rests on: the P1.10
// `RetentionPolicy`/`RetentionSetting` types, whose documented semantics
// are the load-bearing half of the P13.13 §2 matrix:
//
// - P0.16 §4 / report §16.3: NO universal retention period in code, ever.
// - Every class defaults to `Unset`; `Unset` means "no retention action
//   authorized by this configuration" — never "keep forever", never a
//   built-in default period.
// - Values arrive ONLY from deployment configuration (`#[serde(default)]`:
//   a missing class never silently becomes a period). Per P13.14 (record
//   26 §4), the expected upstream of that configuration is the
//   jurisdiction registry's SIGNED rows — an UNSIGNED row authorizes no
//   retention action (fail closed) — and that projection is a DEPLOYMENT
//   LOADER design, not code here.
// - The deployability rule (P13.13 §2, restating the P9.10 §11 check-4
//   discipline): a retention configuration that cannot cite a matrix
//   row's basis is NOT deployable; an absent/unsigned resolution is
//   `UNSET` — which authorizes NO retention action.
//
// The file lives in `crates/flexcore-storage/tests/` (a deliberate,
// recorded extension of the P19.17 test-file ownership, which named the
// audit/authz test dirs): these are the ONLY existing types carrying the
// retention contract, and importing them from anywhere else would need a
// new internal dependency edge (forbidden). No manifest is edited.

use std::time::Duration;

use flexcore_storage::retention::{RetentionPolicy, RetentionSetting};

// --- The all-unset default (the P0.16 §4 rule the P13.13 matrix rests on) --------

#[test]
fn every_data_class_defaults_to_unset_no_universal_period_exists_in_code() {
    // The seven §16.3 classes in report order — the same class set the
    // P13.13 §2 matrix keys its stores onto. NONE carries a period by
    // default; this is the rule, not an omission.
    let policy = RetentionPolicy::default();
    assert!(policy.is_fully_unset(), "the default policy is fully unset");
    assert_eq!(policy, RetentionPolicy::unset());
    assert_eq!(policy.raw_high_frequency_telemetry, RetentionSetting::Unset);
    assert_eq!(policy.normalized_telemetry, RetentionSetting::Unset);
    assert_eq!(policy.aggregated_intervals, RetentionSetting::Unset);
    assert_eq!(policy.command_audit_evidence, RetentionSetting::Unset);
    assert_eq!(policy.market_records, RetentionSetting::Unset);
    assert_eq!(policy.simulation_artifacts, RetentionSetting::Unset);
    assert_eq!(policy.model_forecast_outputs, RetentionSetting::Unset);
}

#[test]
fn the_command_audit_evidence_class_has_no_code_default_either() {
    // The P13.13 §2 `audit_records` / `dispatch_commands` /
    // `feasibility_check_records` rows: command/audit evidence is
    // append-only (deletion is never a runtime path) and its retention is
    // a deployment/legal decision. Even THIS class is unset by default —
    // the append-only rule governs writes, not deletion, and no code
    // default stands in for the missing jurisdiction decision.
    let policy = RetentionPolicy::unset();
    assert_eq!(policy.command_audit_evidence, RetentionSetting::Unset);
    // `Unset` authorizes NO retention action (the type's documented
    // semantics): the match below is exhaustive over what the setting
    // means — there is no "keep forever" and no "apply a default period"
    // arm to fall into.
    match policy.command_audit_evidence {
        RetentionSetting::Unset => {} // the only state constructible without deployment input
        RetentionSetting::Configured { .. } => {
            panic!("an unset-constructed policy cannot carry a configured class")
        }
    }
}

#[test]
fn unset_is_the_only_state_constructible_without_deployment_input() {
    // The P13.14 §4 handoff: `Unset` remains the only state this crate
    // constructs without deployment input. The two constructors agree —
    // there is no second, quietly pre-configured entry point.
    assert_eq!(RetentionPolicy::default(), RetentionPolicy::unset());
    assert!(RetentionPolicy::unset().is_fully_unset());
}

// --- Deployment configuration is the ONLY source of a period ----------------------

#[test]
fn a_configured_class_is_configured_only_because_the_deployment_supplied_it() {
    // Standing in for a deployment loader (which, per P13.14 §4, projects
    // SIGNED registry rows into this type): a period appears ONLY where
    // the configuration supplied one, and every other class stays unset —
    // no class inherits a sibling's period as a fallback.
    let mut policy = RetentionPolicy::unset();
    policy.market_records = RetentionSetting::Configured {
        period: Duration::from_secs(86_400),
    };
    assert!(!policy.is_fully_unset());
    assert!(matches!(
        policy.market_records,
        RetentionSetting::Configured { .. }
    ));
    assert_eq!(policy.raw_high_frequency_telemetry, RetentionSetting::Unset);
    assert_eq!(policy.command_audit_evidence, RetentionSetting::Unset);
    assert_eq!(policy.model_forecast_outputs, RetentionSetting::Unset);
}

#[test]
fn a_partial_configuration_document_leaves_every_unnamed_class_unset() {
    // `#[serde(default)]`: a missing class can never silently become a
    // built-in period. An EMPTY document is the all-unset policy.
    let empty: RetentionPolicy =
        serde_json::from_value(serde_json::json!({})).expect("an empty document parses");
    assert_eq!(empty, RetentionPolicy::unset());
    assert!(empty.is_fully_unset());

    // A partial document configures exactly the classes it names
    // (serde renders a Duration as {"secs", "nanos"}; the variant tag is
    // SCREAMING_SNAKE_CASE per the serde rule).
    let partial: RetentionPolicy = serde_json::from_value(serde_json::json!({
        "raw_high_frequency_telemetry": {
            "CONFIGURED": { "period": { "secs": 2_592_000, "nanos": 0 } }
        }
    }))
    .expect("a partial document parses");
    assert!(matches!(
        partial.raw_high_frequency_telemetry,
        RetentionSetting::Configured { .. }
    ));
    // Every unnamed class — including command/audit evidence — stays
    // unset: the partial document authorizes no retention action for them.
    assert_eq!(partial.command_audit_evidence, RetentionSetting::Unset);
    assert_eq!(partial.market_records, RetentionSetting::Unset);
    assert_eq!(partial.normalized_telemetry, RetentionSetting::Unset);
}

#[test]
fn configured_values_round_trip_and_unset_stays_unset_through_serde() {
    // A deployment configuration survives a serialize/deserialize round
    // trip unchanged — and classes the document did not name stay unset on
    // BOTH passes (a round trip cannot conjure a period).
    let mut policy = RetentionPolicy::unset();
    policy.command_audit_evidence = RetentionSetting::Configured {
        period: Duration::from_secs(3_600),
    };
    let json = serde_json::to_value(&policy).expect("the policy serializes");
    let back: RetentionPolicy = serde_json::from_value(json).expect("the policy parses back");
    assert_eq!(back, policy);
    assert!(matches!(
        back.command_audit_evidence,
        RetentionSetting::Configured { .. }
    ));
    assert_eq!(back.market_records, RetentionSetting::Unset);

    // The all-unset policy survives too.
    let unset = RetentionPolicy::unset();
    let back: RetentionPolicy = serde_json::from_value(serde_json::to_value(&unset).unwrap())
        .expect("the all-unset policy parses back");
    assert!(back.is_fully_unset());
}

// --- The fail-closed reading the P13.13/P13.14 design consumes ---------------------

#[test]
fn unset_is_never_keep_forever_and_never_a_default_period() {
    // The RetentionSetting doc semantics, pinned behaviorally: `Unset` is
    // distinguishable from every configured state, so a consumer CANNOT
    // accidentally treat it as a period. The P13.13 §2 deployability rule
    // (an absent/unsigned resolution authorizes NO retention action) reads
    // exactly this value.
    let policy = RetentionPolicy::unset();
    let classes = [
        &policy.raw_high_frequency_telemetry,
        &policy.normalized_telemetry,
        &policy.aggregated_intervals,
        &policy.command_audit_evidence,
        &policy.market_records,
        &policy.simulation_artifacts,
        &policy.model_forecast_outputs,
    ];
    for class in classes {
        assert!(
            matches!(*class, RetentionSetting::Unset),
            "no data class may carry a built-in retention period (P0.16 §4)"
        );
    }
    // A period is visible as data ONLY when configuration supplied one —
    // and then it carries no provenance in this type: the basis lives in
    // the deployment configuration / signed registry row (P13.14 §2), not
    // here. That separation is the design contract; this assertion merely
    // records that the type exposes nothing else to hang a basis on.
    let configured = RetentionSetting::Configured {
        period: Duration::from_secs(60),
    };
    assert!(matches!(configured, RetentionSetting::Configured { period: Duration { .. } }));
}
