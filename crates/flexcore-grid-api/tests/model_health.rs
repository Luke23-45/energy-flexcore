// P4.03 — Model-versioning and model-health integration tests (ledger P4.03
// acceptance: "Model versions are immutable and hashed; health check covers
// completeness, staleness, and coverage; stale/incomplete model blocks a
// feasibility pass in tests; version appears on every result and command").
//
// The tests use the in-crate fixtures (`tests_support::three_bus_model`) —
// deterministic, no real solver, no I/O.
//
// 1. Hash change invalidates: a content change changes the hash, the
//    registry refuses the in-place re-registration, and a silently edited
//    model fails `verify_content_hash` (the evidence-invalidation hook the
//    P4.12 dispatch commands rely on).
// 2. The stale-model health check fails closed: a source-data change after
//    the model's last verification, and an exceeded age bound, each block
//    usability; the gate-facing `assert_usable_for_feasibility` refuses
//    with the finding carried.
// 3. An uncovered registry element is a health FINDING (never silently
//    dropped) and blocks usability.
// 4. Lifecycle: only an Active model is usable; Draft and Retired are not;
//    illegal transitions are refused; versions are immutable.

mod common;

use common::model_ref;
use flexcore_grid_api::model_health::{
    assert_usable_for_feasibility, check_model_health, model_content_hash, HealthFinding,
    ModelCoverage, ModelHealthConfig, ModelHealthError, ModelLifecycleState, ModelVersionRegistry,
    RegistryElement, RegistryElementKind, VersionedModel,
};
use flexcore_grid_api::port::VersionedNetworkModel;
use flexcore_grid_api::tests_support::three_bus_model;

const NOW: i64 = 10_000;

fn coverage() -> ModelCoverage {
    ModelCoverage {
        covered_feeder_ids: vec!["feeder-a".to_string()],
        covered_element_ids: vec!["asset-1".to_string()],
        coverage_statement: "covers feeder-a (three-bus radial fixture) and asset-1 at bus_3"
            .to_string(),
    }
}

fn fresh_model() -> VersionedNetworkModel {
    // Fresh GridModelRef: the versioning layer assigns the hash.
    let mut reference = model_ref();
    reference.content_hash = String::new();
    three_bus_model(reference)
}

fn registry_with_active_model() -> (ModelVersionRegistry, VersionedModel) {
    let versioned =
        VersionedModel::register(fresh_model(), ModelLifecycleState::Active, coverage(), NOW)
            .expect("registers");
    let mut registry = ModelVersionRegistry::new();
    registry
        .register_version(versioned.clone())
        .expect("first registration succeeds");
    (registry, versioned)
}

fn portfolio() -> Vec<RegistryElement> {
    vec![
        RegistryElement {
            element_id: "feeder-a".to_string(),
            kind: RegistryElementKind::Feeder,
        },
        RegistryElement {
            element_id: "asset-1".to_string(),
            kind: RegistryElementKind::Element,
        },
    ]
}

// ---------------------------------------------------------------------------
// 1. Immutability + hashing: a hash change invalidates
// ---------------------------------------------------------------------------

/// The hash is assigned by the versioning layer, is content-derived, and a
/// content change changes it (a silently edited model is detectable).
#[test]
fn a_content_change_changes_the_content_hash() {
    let mut model = fresh_model();
    model.model_ref.version = "1".to_string();
    let hash_a = model_content_hash(&model);
    // The silent edit: one branch parameter changes.
    model.branches[0].r_ohm += 0.001;
    let hash_b = model_content_hash(&model);
    assert!(hash_a.starts_with("fnv1a64:"));
    assert!(hash_b.starts_with("fnv1a64:"));
    assert_ne!(hash_a, hash_b, "a parameter change must change the hash");
}

/// The versioning layer owns the hash: a caller-supplied hash is
/// overwritten at registration, never trusted.
#[test]
fn registration_overwrites_a_caller_supplied_hash() {
    let mut model = fresh_model();
    model.model_ref.content_hash = "i-swear-this-is-right".to_string();
    let versioned =
        VersionedModel::register(model, ModelLifecycleState::Active, coverage(), NOW)
            .expect("registers");
    assert_ne!(versioned.model_ref().content_hash, "i-swear-this-is-right");
    assert!(versioned.model_ref().content_hash.starts_with("fnv1a64:"));
    assert!(versioned.verify_content_hash());
}

/// Versions are immutable: ANY re-registration of the same
/// (model_id, version) is refused — a change must be a NEW version
/// (ledger P4.03 step 1; the recorded risk: "a silently edited model
/// invalidates past verdicts").
#[test]
fn re_registering_a_version_is_refused_even_with_identical_content() {
    let (mut registry, versioned) = registry_with_active_model();
    // Identical content, same version: still refused (immutable versions).
    let error = registry
        .register_version(versioned.clone())
        .expect_err("re-registration must be refused");
    assert!(matches!(error, ModelHealthError::VersionAlreadyExists { .. }));

    // Different content under the SAME version (the in-place edit attempt):
    // refused as well.
    let mut edited = fresh_model();
    edited.branches[0].rated_current_a = 123.0;
    let edited_versioned =
        VersionedModel::register(edited, ModelLifecycleState::Active, coverage(), NOW)
            .expect("registers with a new hash");
    let error = registry
        .register_version(edited_versioned)
        .expect_err("an in-place content change under the same version is refused");
    match error {
        ModelHealthError::VersionAlreadyExists { model_id, version } => {
            assert_eq!(model_id, "feeder-contract-test");
            assert_eq!(version, "1");
        }
        other => panic!("expected VersionAlreadyExists, got {other:?}"),
    }

    // The prior version is untouched: its hash still verifies.
    assert!(versioned.verify_content_hash());
}

/// A model edited AFTER registration (the P4.12 hook): the recorded hash no
/// longer matches the live content, so any evidence citing it is
/// invalidated — `verify_content_hash` reports false.
#[test]
fn a_silently_edited_model_fails_the_hash_verification() {
    let (_, mut versioned) = registry_with_active_model();
    assert!(versioned.verify_content_hash());
    // The silent edit happens after evidence was produced.
    versioned.model.transformers[0].rated_mva = 9.9;
    assert!(
        !versioned.verify_content_hash(),
        "a silently edited model must fail the hash verification"
    );
}

// ---------------------------------------------------------------------------
// 2. Staleness: the health check fails closed
// ---------------------------------------------------------------------------

/// A source-data change (topology/capability) recorded AFTER the model's
/// last verification makes the model STALE for new candidates: the
/// health check reports it and the gate-facing refusal fires (ledger P4.03
/// step 3; §17.5 invariant 9).
#[test]
fn a_source_change_after_verification_makes_the_model_stale() {
    let (mut registry, versioned) = registry_with_active_model();
    // Before the change: healthy.
    let report = check_model_health(
        &versioned,
        &registry,
        &portfolio(),
        &ModelHealthConfig::default(),
        NOW,
    );
    assert!(report.is_usable(), "expected no findings, got {:?}", report.findings);
    assert_usable_for_feasibility(&report).expect("healthy model usable");

    // The topology change after the last verification.
    registry
        .record_source_change("chg-1".to_string(), NOW + 1, "line_2_3 reconfigured".to_string())
        .expect("records");
    let report = check_model_health(
        &versioned,
        &registry,
        &portfolio(),
        &ModelHealthConfig::default(),
        NOW + 10,
    );
    assert!(!report.is_usable(), "a stale model must not be usable");
    assert!(report
        .findings
        .iter()
        .any(|f| matches!(f, HealthFinding::Stale { unverified_changes, .. }
            if unverified_changes.contains(&"chg-1".to_string()))));
    // The gate-facing refusal carries the findings (fail-closed).
    let error = assert_usable_for_feasibility(&report).expect_err("stale model refused");
    assert!(matches!(error, ModelHealthError::NotUsable { .. }));
}

/// The verification-age bound: `age == limit` is NOT stale; beyond the
/// limit IS (the recorded P2.08/P3.13 convention).
#[test]
fn the_age_bound_uses_the_recorded_staleness_convention() {
    let (registry, versioned) = registry_with_active_model();
    let config = ModelHealthConfig {
        max_age_since_verification_millis: 1_000,
    };
    // age == limit: not stale.
    let report = check_model_health(&versioned, &registry, &portfolio(), &config, NOW + 1_000);
    assert!(report.is_usable(), "age == limit is not stale, got {:?}", report.findings);
    // age > limit: stale.
    let report = check_model_health(&versioned, &registry, &portfolio(), &config, NOW + 1_001);
    assert!(!report.is_usable());
    assert!(report
        .findings
        .iter()
        .any(|f| matches!(f, HealthFinding::Stale { age_millis: Some(1_001), .. })));

    // Re-verification (a newer last_verified_at) clears the age staleness:
    // the composition root re-verifies against source data and re-registers
    // as a NEW version.
    let reverified = VersionedModel {
        last_verified_at_millis: NOW + 900,
        ..versioned.clone()
    };
    let report =
        check_model_health(&reverified, &registry, &portfolio(), &config, NOW + 1_001);
    assert!(report.is_usable(), "re-verified model is fresh again, got {:?}", report.findings);
}

// ---------------------------------------------------------------------------
// 3. Coverage: an uncovered element is a FINDING
// ---------------------------------------------------------------------------

/// A registry element the portfolio contains but the model does not cover
/// is recorded as a health FINDING — never silently dropped — and blocks
/// usability (fail-closed).
#[test]
fn an_uncovered_element_is_a_finding_and_blocks_usability() {
    let (registry, versioned) = registry_with_active_model();
    let mut elements = portfolio();
    elements.push(RegistryElement {
        element_id: "feeder-z".to_string(),
        kind: RegistryElementKind::Feeder,
    });
    let report = check_model_health(
        &versioned,
        &registry,
        &elements,
        &ModelHealthConfig::default(),
        NOW,
    );
    assert!(!report.is_usable());
    let uncovered: Vec<&HealthFinding> = report
        .findings
        .iter()
        .filter(|f| matches!(f, HealthFinding::UncoveredElement { element_id, .. } if element_id == "feeder-z"))
        .collect();
    assert_eq!(
        uncovered.len(),
        1,
        "exactly one finding names the uncovered element, got {:?}",
        report.findings
    );
    assert_usable_for_feasibility(&report).expect_err("uncovered element blocks");
}

/// ALL findings are reported at once — the first problem never hides the
/// rest (a stale model AND an uncovered element both appear).
#[test]
fn all_findings_are_reported_together() {
    let (mut registry, versioned) = registry_with_active_model();
    registry
        .record_source_change("chg-9".to_string(), NOW + 1, "capability update".to_string())
        .expect("records");
    let elements = vec![RegistryElement {
        element_id: "asset-unknown".to_string(),
        kind: RegistryElementKind::Element,
    }];
    let report = check_model_health(
        &versioned,
        &registry,
        &elements,
        &ModelHealthConfig::default(),
        NOW + 2,
    );
    assert!(report
        .findings
        .iter()
        .any(|f| matches!(f, HealthFinding::Stale { .. })));
    assert!(report
        .findings
        .iter()
        .any(|f| matches!(f, HealthFinding::UncoveredElement { element_id, .. }
            if element_id == "asset-unknown")));
}

// ---------------------------------------------------------------------------
// 4. Lifecycle: only Active models are usable
// ---------------------------------------------------------------------------

/// A Draft model is not usable; activating it makes it usable; retiring it
/// makes it unusable again — permanently (no re-activation).
#[test]
fn only_an_active_model_is_usable() {
    let mut registry = ModelVersionRegistry::new();
    let versioned =
        VersionedModel::register(fresh_model(), ModelLifecycleState::Draft, coverage(), NOW)
            .expect("registers");
    registry.register_version(versioned.clone()).expect("registers");

    // Draft: refused.
    let report = check_model_health(
        &versioned,
        &registry,
        &portfolio(),
        &ModelHealthConfig::default(),
        NOW,
    );
    assert!(report
        .findings
        .iter()
        .any(|f| matches!(f, HealthFinding::NotActive { state } if state == "draft")));
    assert_usable_for_feasibility(&report).expect_err("a draft model is refused");

    // Draft → Active: usable.
    registry
        .transition("feeder-contract-test", "1", ModelLifecycleState::Active)
        .expect("draft activates");
    let active = registry.get("feeder-contract-test", "1").expect("present");
    let report = check_model_health(
        active,
        &registry,
        &portfolio(),
        &ModelHealthConfig::default(),
        NOW,
    );
    assert!(report.is_usable(), "active model usable, got {:?}", report.findings);

    // Active → Retired: unusable, permanently.
    registry
        .transition("feeder-contract-test", "1", ModelLifecycleState::Retired)
        .expect("active retires");
    let retired = registry.get("feeder-contract-test", "1").expect("present");
    let report = check_model_health(
        retired,
        &registry,
        &portfolio(),
        &ModelHealthConfig::default(),
        NOW,
    );
    assert!(report
        .findings
        .iter()
        .any(|f| matches!(f, HealthFinding::NotActive { state } if state == "retired")));
    assert_usable_for_feasibility(&report).expect_err("a retired model is refused");

    // Retired → Active is an illegal transition (no re-activation).
    assert!(matches!(
        registry.transition("feeder-contract-test", "1", ModelLifecycleState::Active),
        Err(ModelHealthError::IllegalTransition { .. })
    ));
}

/// An unknown version cannot be transitioned (named error).
#[test]
fn transitioning_an_unknown_version_is_a_named_error() {
    let mut registry = ModelVersionRegistry::new();
    let error = registry
        .transition("feeder-a", "99", ModelLifecycleState::Active)
        .expect_err("unknown version");
    match error {
        ModelHealthError::UnknownVersion { model_id, version } => {
            assert_eq!(model_id, "feeder-a");
            assert_eq!(version, "99");
        }
        other => panic!("expected UnknownVersion, got {other:?}"),
    }
}
