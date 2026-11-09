// P3.16 — Market/PCC tracking: acceptance tests (ledger P3.16).
//
// Covers the ledger acceptance criteria:
//
// - PCC identity is a REQUIRED, VALIDATED field (empty/whitespace
//   identities rejected; the identity comes from the Phase 2 site
//   registry);
// - tracking is computed AT the declared PCC, with the declared loss model
//   included — the loss fixture: a schedule that meets the asset sum but
//   misses the PCC target (because of the modelled loss) scores WORSE than
//   one that meets the PCC target;
// - multiple delivery points stay SEPARATE terms, never merged;
// - the sign convention (P > 0 = export at the PCC) is enforced and
//   visible in the per-interval records;
// - commitments are SYNTHETIC test commitments, labelled as such in the
//   type and in the record; no market qualification is claimed anywhere;
// - configuration validation (λ ≥ 0, loss fraction in [0, 1), unique
//   identities, series lengths).
//
// NOTE (VERIFY-BUILD, recorded): per the task constraint, cargo was NOT run
// while authoring these tests; the first `cargo test -p flexcore-application`
// run is the verification obligation.

use flexcore_application::pcc_tracking::{
    evaluate_pcc_tracking, tracking_record_fields, LossModel, PccRef, PccTrackingConfig,
    PccTrackingError, PointSchedule, SyntheticCommitment, TrackingInputs, TrackingPoint,
    COMMITMENTS_SYNTHETIC_LABEL, LOSS_MODEL_NOTE, LAMBDA_TRACK_UNIT, SYNTHETIC_COMMITMENT_NOTE,
    TRACKING_SIGN_CONVENTION, TRACKING_TERM_STATEMENT,
};
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{EventTime, SiteTimezone};
use flexcore_domain::entities::{Portfolio, Site, Tenant};
use flexcore_domain::ids::{PortfolioId, SiteId, TenantId};

const UUID_TENANT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a8b";
const UUID_PORTFOLIO: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a8c";
const UUID_SITE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a8d";

fn uuid(text: &str) -> FlexUuid {
    FlexUuid::parse(text).unwrap()
}

/// A Phase 2 site whose declared PCC is "PCC-7" (the P2.02 registry
/// vocabulary; the site is the authority for the PCC identity).
fn site_with_pcc_7() -> Site {
    let tenant = Tenant::try_new(
        TenantId::new(uuid(UUID_TENANT)),
        "Acme Energy".to_string(),
        None,
        "idp:acme".to_string(),
        "profile:default".to_string(),
        EventTime::from_unix_millis(1_000).unwrap(),
    )
    .unwrap();
    let portfolio = Portfolio::try_new(
        &tenant,
        PortfolioId::new(uuid(UUID_PORTFOLIO)),
        "North portfolio".to_string(),
        None,
        "Acme Operations".to_string(),
        "DE-AMPRION".to_string(),
        "rules:constrained-aggregation".to_string(),
        EventTime::from_unix_millis(2_000).unwrap(),
    )
    .unwrap();
    Site::try_new(
        &portfolio,
        SiteId::new(uuid(UUID_SITE)),
        "Substation 7 site".to_string(),
        None,
        "52.52N 13.40E".to_string(),
        SiteTimezone::new("Europe/Berlin".to_string()).unwrap(),
        "PCC-7".to_string(),
        "grid-model:rev-42".to_string(),
        "edge-0007".to_string(),
        EventTime::from_unix_millis(3_000).unwrap(),
    )
    .unwrap()
}

/// A one-point config at PCC-7 with the given λ and loss model, 1-hour
/// intervals.
fn single_point_config(lambda: f64, loss: LossModel) -> PccTrackingConfig {
    PccTrackingConfig::new(vec![TrackingPoint::new(PccRef::new("PCC-7").unwrap(), lambda, loss).unwrap()])
        .unwrap()
}

// --- PCC identity: required and validated -------------------------------------

#[test]
fn empty_pcc_identity_is_rejected() {
    assert_eq!(PccRef::new(""), Err(PccTrackingError::EmptyPccIdentity));
}

#[test]
fn whitespace_pcc_identity_is_rejected() {
    assert_eq!(PccRef::new(" PCC-7"), Err(PccTrackingError::PccIdentityWhitespace));
    assert_eq!(PccRef::new("PCC-7 "), Err(PccTrackingError::PccIdentityWhitespace));
}

#[test]
fn pcc_identity_comes_from_the_declared_site_reference() {
    let site = site_with_pcc_7();
    let pcc = PccRef::from_site(&site);
    assert_eq!(pcc.as_str(), "PCC-7");
    assert_eq!(pcc, PccRef::new("PCC-7").unwrap(), "the registry identity is used verbatim");
}

// --- Synthetic commitment labelling --------------------------------------------

#[test]
fn commitments_are_labelled_synthetic_on_every_access() {
    let commitment = SyntheticCommitment::new("test-commitment-v1", vec![2.0, 0.5]).unwrap();
    let labeled = commitment.labeled_values();
    assert_eq!(labeled.provenance, COMMITMENTS_SYNTHETIC_LABEL);
    assert_eq!(labeled.provenance, "SYNTHETIC");
    assert_eq!(labeled.version, "test-commitment-v1");
    assert_eq!(labeled.values_mw, &[2.0, 0.5]);
    assert_eq!(labeled.note, SYNTHETIC_COMMITMENT_NOTE);
    assert!(
        labeled.note.contains("no market qualification is claimed"),
        "the type carries the no-market-qualification statement"
    );
    assert!(labeled.note.contains("D1 OPEN"));
}

#[test]
fn unversioned_commitments_are_rejected() {
    assert_eq!(
        SyntheticCommitment::new("", vec![1.0]).unwrap_err(),
        PccTrackingError::EmptyCommitmentVersion
    );
    assert_eq!(
        SyntheticCommitment::new("v 1", vec![1.0]).unwrap_err(),
        PccTrackingError::CommitmentVersionWhitespace
    );
}

// --- The loss fixture: asset sum vs PCC target ---------------------------------

#[test]
fn schedule_meeting_asset_sum_but_missing_pcc_target_scores_worse() {
    // 1-hour interval, λ = 10 currency-units/(MW²·h), loss fraction 0.05.
    // Commitment: export 2.0 MW at the PCC.
    //   Schedule A meets the ASSET sum (2.0 MW injected) but the modelled
    //   loss delivers only 1.9 MW at the PCC -> mismatch −0.1 MW ->
    //   cost 10 · 0.01 · 1 = 0.1.
    //   Schedule B injects 2.0/0.95 MW so the PCC sees exactly 2.0 MW ->
    //   cost 0. A scores strictly worse — the ledger's fixture.
    let config = single_point_config(10.0, LossModel::NetExportFraction { fraction: 0.05 });
    let commitment = SyntheticCommitment::new("test-commitment-v1", vec![2.0]).unwrap();
    let schedule_a = PointSchedule {
        pcc: PccRef::new("PCC-7").unwrap(),
        asset_net_export_mw: vec![2.0],
    };
    let schedule_b = PointSchedule {
        pcc: PccRef::new("PCC-7").unwrap(),
        asset_net_export_mw: vec![2.0 / 0.95],
    };
    let inputs_for = |schedule: &PointSchedule| TrackingInputs {
        interval_seconds: 3600,
        commitments: vec![(PccRef::new("PCC-7").unwrap(), commitment.clone())],
        schedules: vec![schedule.clone()],
    };

    let evaluation_a = evaluate_pcc_tracking(&config, &inputs_for(&schedule_a)).unwrap();
    let evaluation_b = evaluate_pcc_tracking(&config, &inputs_for(&schedule_b)).unwrap();

    let point_a = &evaluation_a.points[0];
    let point_b = &evaluation_b.points[0];
    assert!(
        (point_a.per_interval[0].p_pcc_mw - 1.9).abs() < 1e-12,
        "the modelled loss delivers 1.9 MW at the PCC"
    );
    assert!((point_a.per_interval[0].mismatch_mw - (-0.1)).abs() < 1e-12);
    assert!((point_a.total_cost - 0.1).abs() < 1e-12);
    assert!(
        point_b.per_interval[0].mismatch_mw.abs() < 1e-9,
        "schedule B meets the PCC target (mismatch ~ 0)"
    );
    assert!(point_b.total_cost < 1e-9);
    assert!(
        evaluation_a.total > evaluation_b.total,
        "meeting the asset sum but missing the PCC target scores WORSE than meeting the \
         PCC target ({} > {})",
        evaluation_a.total,
        evaluation_b.total
    );
}

#[test]
fn tracking_is_computed_at_the_pcc_not_the_asset_sum() {
    // With a loss model the PCC position differs from the attributed asset
    // sum; the recorded per-interval values are the PCC position, and the
    // mismatch is against the commitment AT the PCC.
    let config = single_point_config(1.0, LossModel::NetExportFraction { fraction: 0.5 });
    let commitment = SyntheticCommitment::new("test-commitment-v1", vec![1.0]).unwrap();
    let inputs = TrackingInputs {
        interval_seconds: 3600,
        commitments: vec![(PccRef::new("PCC-7").unwrap(), commitment)],
        schedules: vec![PointSchedule {
            pcc: PccRef::new("PCC-7").unwrap(),
            asset_net_export_mw: vec![2.0], // asset sum 2.0, PCC sees 1.0
        }],
    };
    let evaluation = evaluate_pcc_tracking(&config, &inputs).unwrap();
    let interval = &evaluation.points[0].per_interval[0];
    assert!((interval.p_pcc_mw - 1.0).abs() < 1e-12);
    assert!(interval.mismatch_mw.abs() < 1e-12);
    assert_eq!(interval.cost, 0.0);
    assert_eq!(evaluation.points[0].loss_fraction, 0.5);
}

// --- Multiple delivery points stay separate ------------------------------------

#[test]
fn multi_pcc_case_keeps_the_terms_separate() {
    // Two delivery points with different λ and loss models. Point A is met
    // exactly; point B misses on both intervals. The per-point records stay
    // independent; the grand total is the labelled sum of the separate
    // terms.
    let config = PccTrackingConfig::new(vec![
        TrackingPoint::new(PccRef::new("PCC-A").unwrap(), 5.0, LossModel::None).unwrap(),
        TrackingPoint::new(
            PccRef::new("PCC-B").unwrap(),
            20.0,
            LossModel::NetExportFraction { fraction: 0.1 },
        )
        .unwrap(),
    ])
    .unwrap();
    let commitments = vec![
        (
            PccRef::new("PCC-A").unwrap(),
            SyntheticCommitment::new("test-commitment-v1", vec![1.0, -1.0]).unwrap(),
        ),
        (
            PccRef::new("PCC-B").unwrap(),
            SyntheticCommitment::new("test-commitment-v2", vec![0.5, 0.5]).unwrap(),
        ),
    ];
    let schedules = vec![
        PointSchedule {
            pcc: PccRef::new("PCC-A").unwrap(),
            asset_net_export_mw: vec![1.0, -1.0], // meets the commitment exactly
        },
        PointSchedule {
            pcc: PccRef::new("PCC-B").unwrap(),
            asset_net_export_mw: vec![0.6, 0.4], // sums to 1.0 but the PCC sees 0.54, 0.36
        },
    ];
    let inputs = TrackingInputs { interval_seconds: 3600, commitments, schedules };
    let evaluation = evaluate_pcc_tracking(&config, &inputs).unwrap();

    assert_eq!(evaluation.points.len(), 2, "one separate term per delivery point");
    let a = &evaluation.points[0];
    let b = &evaluation.points[1];
    assert_eq!(a.pcc, "PCC-A");
    assert_eq!(b.pcc, "PCC-B");
    assert_eq!(a.lambda_track, 5.0);
    assert_eq!(b.lambda_track, 20.0);
    // Point A: met exactly, zero cost.
    assert!(a.total_cost.abs() < 1e-12);
    assert!((a.per_interval[0].mismatch_mw).abs() < 1e-12);
    assert!((a.per_interval[1].mismatch_mw).abs() < 1e-12);
    // Point B: mismatches 0.04 and −0.14 -> cost 20·(0.0016 + 0.0196) = 0.424.
    assert!((b.per_interval[0].mismatch_mw - 0.04).abs() < 1e-12);
    assert!((b.per_interval[1].mismatch_mw - (-0.14)).abs() < 1e-12);
    assert!((b.total_cost - 0.424).abs() < 1e-12);
    // The per-point series are separate: A's import interval stays A's.
    assert!(a.per_interval[1].commitment_mw < 0.0, "A's import commitment is recorded at A");
    assert!(b.per_interval[0].commitment_mw > 0.0, "B's export commitment is recorded at B");
    // The grand total is the explicit sum of the separate terms.
    assert!((evaluation.total - (a.total_cost + b.total_cost)).abs() < 1e-12);
    assert!((evaluation.total - 0.424).abs() < 1e-12);
    assert_eq!(evaluation.synthetic_label, "SYNTHETIC");
}

#[test]
fn duplicate_pcc_identities_are_rejected() {
    let point = || TrackingPoint::new(PccRef::new("PCC-7").unwrap(), 1.0, LossModel::None).unwrap();
    assert_eq!(
        PccTrackingConfig::new(vec![point(), point()]).unwrap_err(),
        PccTrackingError::DuplicatePcc { pcc: "PCC-7".to_string() }
    );
    let config = single_point_config(1.0, LossModel::None);
    let commitment = SyntheticCommitment::new("v", vec![1.0]).unwrap();
    let inputs = TrackingInputs {
        interval_seconds: 3600,
        commitments: vec![
            (PccRef::new("PCC-7").unwrap(), commitment.clone()),
            (PccRef::new("PCC-7").unwrap(), commitment),
        ],
        schedules: vec![PointSchedule {
            pcc: PccRef::new("PCC-7").unwrap(),
            asset_net_export_mw: vec![1.0],
        }],
    };
    assert_eq!(
        evaluate_pcc_tracking(&config, &inputs).unwrap_err(),
        PccTrackingError::DuplicatePcc { pcc: "PCC-7".to_string() }
    );
}

#[test]
fn every_configured_point_needs_its_commitment_and_schedule() {
    let config = PccTrackingConfig::new(vec![
        TrackingPoint::new(PccRef::new("PCC-A").unwrap(), 1.0, LossModel::None).unwrap(),
        TrackingPoint::new(PccRef::new("PCC-B").unwrap(), 1.0, LossModel::None).unwrap(),
    ])
    .unwrap();
    let inputs = TrackingInputs {
        interval_seconds: 3600,
        commitments: vec![(PccRef::new("PCC-A").unwrap(), SyntheticCommitment::new("v", vec![1.0]).unwrap())],
        schedules: vec![PointSchedule {
            pcc: PccRef::new("PCC-A").unwrap(),
            asset_net_export_mw: vec![1.0],
        }],
    };
    assert_eq!(
        evaluate_pcc_tracking(&config, &inputs).unwrap_err(),
        PccTrackingError::MissingCommitment { pcc: "PCC-B".to_string() }
    );
    let inputs = TrackingInputs {
        interval_seconds: 3600,
        commitments: vec![
            (PccRef::new("PCC-A").unwrap(), SyntheticCommitment::new("v", vec![1.0]).unwrap()),
            (PccRef::new("PCC-B").unwrap(), SyntheticCommitment::new("v", vec![1.0]).unwrap()),
        ],
        schedules: vec![PointSchedule {
            pcc: PccRef::new("PCC-A").unwrap(),
            asset_net_export_mw: vec![1.0],
        }],
    };
    assert_eq!(
        evaluate_pcc_tracking(&config, &inputs).unwrap_err(),
        PccTrackingError::MissingSchedule { pcc: "PCC-B".to_string() }
    );
}

// --- Sign convention -------------------------------------------------------------

#[test]
fn sign_convention_export_positive_import_negative_is_enforced() {
    // Commitment: export 1.0 MW. A schedule that IMPORTS 1.0 MW misses the
    // target by 2.0 MW (sign matters in the record even though the square
    // is sign-symmetric).
    let config = single_point_config(1.0, LossModel::None);
    let commitment = SyntheticCommitment::new("test-commitment-v1", vec![1.0]).unwrap();
    let inputs = TrackingInputs {
        interval_seconds: 3600,
        commitments: vec![(PccRef::new("PCC-7").unwrap(), commitment)],
        schedules: vec![PointSchedule {
            pcc: PccRef::new("PCC-7").unwrap(),
            asset_net_export_mw: vec![-1.0],
        }],
    };
    let evaluation = evaluate_pcc_tracking(&config, &inputs).unwrap();
    let interval = &evaluation.points[0].per_interval[0];
    assert!((interval.mismatch_mw - (-2.0)).abs() < 1e-12, "importing against an export commitment is a −2 MW mismatch");
    assert!((interval.cost - 4.0).abs() < 1e-12, "λ·(−2)²·1 h = 4");
}

// --- Configuration validation ----------------------------------------------------

#[test]
fn config_validation_rejects_invalid_parameters() {
    let pcc = PccRef::new("PCC-7").unwrap();
    assert_eq!(
        TrackingPoint::new(pcc.clone(), -1.0, LossModel::None).unwrap_err(),
        PccTrackingError::InvalidLambdaTrack { value: -1.0 }
    );
    assert_eq!(
        TrackingPoint::new(pcc.clone(), f64::NAN, LossModel::None).unwrap_err(),
        PccTrackingError::InvalidLambdaTrack { value: f64::NAN }
    );
    assert_eq!(
        TrackingPoint::new(pcc.clone(), 1.0, LossModel::NetExportFraction { fraction: 1.0 })
            .unwrap_err(),
        PccTrackingError::InvalidLossFraction { value: 1.0 }
    );
    assert_eq!(
        TrackingPoint::new(pcc.clone(), 1.0, LossModel::NetExportFraction { fraction: -0.1 })
            .unwrap_err(),
        PccTrackingError::InvalidLossFraction { value: -0.1 }
    );
    assert!(TrackingPoint::new(pcc.clone(), 1.0, LossModel::NetExportFraction { fraction: 0.0 }).is_ok());
    assert_eq!(
        PccTrackingConfig::new(vec![]).unwrap_err(),
        PccTrackingError::EmptyTrackingPoints
    );
}

#[test]
fn evaluation_series_validation() {
    let config = single_point_config(1.0, LossModel::None);
    // Zero interval length.
    let inputs = TrackingInputs {
        interval_seconds: 0,
        commitments: vec![(PccRef::new("PCC-7").unwrap(), SyntheticCommitment::new("v", vec![1.0]).unwrap())],
        schedules: vec![PointSchedule {
            pcc: PccRef::new("PCC-7").unwrap(),
            asset_net_export_mw: vec![1.0],
        }],
    };
    assert_eq!(
        evaluate_pcc_tracking(&config, &inputs).unwrap_err(),
        PccTrackingError::NonPositiveIntervalSeconds
    );
    // Commitment shorter than the schedule.
    let inputs = TrackingInputs {
        interval_seconds: 3600,
        commitments: vec![(PccRef::new("PCC-7").unwrap(), SyntheticCommitment::new("v", vec![1.0]).unwrap())],
        schedules: vec![PointSchedule {
            pcc: PccRef::new("PCC-7").unwrap(),
            asset_net_export_mw: vec![1.0, 1.0],
        }],
    };
    assert_eq!(
        evaluate_pcc_tracking(&config, &inputs).unwrap_err(),
        PccTrackingError::SeriesLengthMismatch {
            pcc: "PCC-7".to_string(),
            expected: 2,
            found: 1,
        }
    );
    // Non-finite attributed injection.
    let inputs = TrackingInputs {
        interval_seconds: 3600,
        commitments: vec![(PccRef::new("PCC-7").unwrap(), SyntheticCommitment::new("v", vec![1.0]).unwrap())],
        schedules: vec![PointSchedule {
            pcc: PccRef::new("PCC-7").unwrap(),
            asset_net_export_mw: vec![f64::INFINITY],
        }],
    };
    assert_eq!(
        evaluate_pcc_tracking(&config, &inputs).unwrap_err(),
        PccTrackingError::NonFiniteSeriesValue {
            pcc: "PCC-7".to_string(),
            series: "asset_net_export_mw",
            interval: 0,
            value: f64::INFINITY,
        }
    );
}

// --- The P3.21 record fields -----------------------------------------------------

#[test]
fn record_fields_carry_the_term_sign_loss_and_synthetic_labels_per_point() {
    let config = PccTrackingConfig::new(vec![
        TrackingPoint::new(PccRef::new("PCC-A").unwrap(), 5.0, LossModel::None).unwrap(),
        TrackingPoint::new(
            PccRef::new("PCC-B").unwrap(),
            20.0,
            LossModel::NetExportFraction { fraction: 0.1 },
        )
        .unwrap(),
    ])
    .unwrap();
    let commitments = vec![
        (
            PccRef::new("PCC-A").unwrap(),
            SyntheticCommitment::new("test-commitment-v1", vec![1.0]).unwrap(),
        ),
        (
            PccRef::new("PCC-B").unwrap(),
            SyntheticCommitment::new("test-commitment-v2", vec![0.5]).unwrap(),
        ),
    ];
    let fields = tracking_record_fields(&config, &commitments).unwrap();
    let get = |key: &str| {
        fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| panic!("record field '{key}' missing"))
    };
    assert_eq!(get("tracking.term"), TRACKING_TERM_STATEMENT);
    assert!(TRACKING_TERM_STATEMENT.contains("DECLARED point of common coupling"));
    assert!(TRACKING_TERM_STATEMENT.contains("never the sum of arbitrary bus injections"));
    assert_eq!(get("tracking.sign_convention"), TRACKING_SIGN_CONVENTION);
    assert!(TRACKING_SIGN_CONVENTION.contains("P > 0 means export at the PCC"));
    assert_eq!(get("tracking.loss_model_note"), LOSS_MODEL_NOTE);
    assert!(LOSS_MODEL_NOTE.contains("ASSUMPTION"), "the loss simplification is an ASSUMPTION record");
    assert!(LOSS_MODEL_NOTE.contains("Phase 4 grid model"));
    // Per-point entries stay separate, with λ unit, loss, and commitment labels.
    assert_eq!(get("tracking.point[0].pcc"), "PCC-A");
    assert_eq!(get("tracking.point[1].pcc"), "PCC-B");
    assert_eq!(get("tracking.point[1].lambda_track"), "20");
    assert_eq!(get("tracking.point[1].lambda_track_unit"), LAMBDA_TRACK_UNIT);
    assert_eq!(get("tracking.point[1].loss_fraction"), "0.1");
    assert_eq!(get("tracking.point[1].commitment_version"), "test-commitment-v2");
    assert_eq!(get("tracking.point[1].commitment_provenance"), "SYNTHETIC");
    assert_eq!(get("tracking.point[1].commitment_note"), SYNTHETIC_COMMITMENT_NOTE);
    assert_eq!(get("tracking.commitment_provenance"), "SYNTHETIC");
    assert!(
        get("tracking.commitments_are").contains("no market qualification is claimed"),
        "the record states the no-market-qualification rule"
    );
    assert_eq!(
        get("tracking.multiple_points"),
        "one separate tracking term per delivery point; never merged into one number"
    );
}

#[test]
fn record_fields_require_a_commitment_for_every_point() {
    let config = single_point_config(1.0, LossModel::None);
    let err = tracking_record_fields(&config, &[]).unwrap_err();
    assert_eq!(err, PccTrackingError::MissingCommitment { pcc: "PCC-7".to_string() });
}
