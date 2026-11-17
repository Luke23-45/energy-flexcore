// P16.06 — Bid composition service: tests (WRITTEN-UNRUN).
//
// Phase 16 plan §2 row P16.06; decision record:
// `docs/phases-11-20/phase-16-market-integration-settlement/records/
// 06_p16_06_bid_composition_service.md`.
//
// Covers the subtask's obligations:
//
// - PJM single aggregation node: the composed record aggregates every
//   asset's envelope-derived quantities per interval into ONE bid at
//   the node [S-90];
// - CAISO PDR two-ID load-shift: the aggregated load-shift bid carries
//   the two recorded identifiers [S-96];
// - ISO-NE DAM Asset tab: the per-asset quantities stay PER ASSET (no
//   cross-asset aggregation) [S-103];
// - the envelope remains the physical truth layer: an input without its
//   envelope-version reference is REFUSED (`MissingEnvelopeProvenance`),
//   and the composed record carries the envelope references forward;
// - operator-approved mode + explicit per-market grant REQUIRED: a
//   non-operator mode, a non-ACTIVE grant, an expired grant, a wrong
//   tenant, or a wrong market structure is a named refusal;
// - determinism: BTreeMap asset ordering; the same inputs + clock
//   replay identically;
// - NO bid is submitted — none can be (the output is a record; the
//   statement is pinned).
//
// NOTE (VERIFY-BUILD, recorded): per the loop bans, cargo was NOT run
// while authoring these tests; the first
// `cargo test -p flexcore-application --test bid_composition` run is
// the verification obligation.

use flexcore_application::bid_composition::{
    compose_bid, AssetBidInput, BidCompositionError, ComposedBidRecord, MarketBidGrant,
    MarketStructureDeclaration, NO_BID_SUBMITTED_STATEMENT, REQUIRED_CONTROL_MODE_NAME,
};
use flexcore_ports::fakes::FixedClock;

/// The pinned test instant (UTC; the injected clock's instant).
const PINNED_NOW_UNIX_MS: i64 = 1_760_000_000_000;

/// The grant validity window used by every positive fixture (ends one
/// hour after the pinned now).
const GRANT_VALID_UNTIL: i64 = PINNED_NOW_UNIX_MS + 3_600_000;

/// A two-interval bid window [start, end) per interval.
const INTERVAL_BOUNDS: [(i64, i64); 2] = [
    (1_760_003_600_000, 1_760_003_900_000),
    (1_760_003_900_000, 1_760_004_200_000),
];

fn active_grant(tenant: &str, structure_name: &str) -> MarketBidGrant {
    MarketBidGrant::new(
        "grant-1".to_string(),
        tenant.to_string(),
        structure_name.to_string(),
        "ACTIVE".to_string(),
        "operator-a".to_string(),
        PINNED_NOW_UNIX_MS - 60_000,
        GRANT_VALID_UNTIL,
        "audit-record-1".to_string(),
    )
    .expect("the fixture grant is valid")
}

fn asset_input(asset_id: &str, quantities: [f64; 2]) -> AssetBidInput {
    AssetBidInput {
        asset_id: asset_id.to_string(),
        envelope_ref: format!("envelope-v7-{asset_id}"),
        interval_quantities_mw: quantities.to_vec(),
    }
}

#[test]
fn pjm_composition_aggregates_every_asset_to_the_single_node() {
    let clock = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let structure = MarketStructureDeclaration::PjmSingleAggregationNode {
        aggregation_node_ref: "pjm-node-A".to_string(),
    };
    let grant = active_grant("tenant-1", structure.name());
    let inputs = vec![
        asset_input("asset-a", [1.5, 2.5]),
        asset_input("asset-b", [0.5, 1.0]),
    ];
    let record = compose_bid(
        &structure,
        &grant,
        REQUIRED_CONTROL_MODE_NAME,
        "tenant-1",
        &INTERVAL_BOUNDS,
        &inputs,
        &clock,
    )
    .expect("the PJM composition is valid");
    match record {
        ComposedBidRecord::PjmAggregationBid {
            aggregation_node_ref,
            intervals,
            contributing_assets,
            envelope_refs,
        } => {
            assert_eq!(aggregation_node_ref, "pjm-node-A");
            assert_eq!(intervals.len(), 2);
            assert!((intervals[0].quantity_mw - 2.0).abs() < 1e-12);
            assert!((intervals[1].quantity_mw - 3.5).abs() < 1e-12);
            assert_eq!(intervals[0].interval_end_millis, INTERVAL_BOUNDS[0].1);
            assert_eq!(contributing_assets, vec!["asset-a".to_string(), "asset-b".to_string()]);
            assert_eq!(envelope_refs.len(), 2);
        }
        other => panic!("expected a PJM aggregation bid, got: {other:?}"),
    }
}

#[test]
fn caiso_composition_carries_the_two_recorded_identifiers() {
    let clock = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let structure = MarketStructureDeclaration::CaisoPdrTwoIdLoadShift {
        pdr_resource_id: "pdr-resource-1".to_string(),
        load_shift_id: "load-shift-1".to_string(),
    };
    let grant = active_grant("tenant-1", structure.name());
    let inputs = vec![asset_input("asset-a", [1.0, 1.0])];
    let record = compose_bid(
        &structure,
        &grant,
        REQUIRED_CONTROL_MODE_NAME,
        "tenant-1",
        &INTERVAL_BOUNDS,
        &inputs,
        &clock,
    )
    .expect("the CAISO composition is valid");
    match record {
        ComposedBidRecord::CaisoPdrLoadShiftBid {
            pdr_resource_id,
            load_shift_id,
            intervals,
            ..
        } => {
            assert_eq!(pdr_resource_id, "pdr-resource-1");
            assert_eq!(load_shift_id, "load-shift-1");
            assert!((intervals[0].quantity_mw - 1.0).abs() < 1e-12);
        }
        other => panic!("expected a CAISO load-shift bid, got: {other:?}"),
    }
}

#[test]
fn isone_composition_keeps_quantities_per_asset() {
    let clock = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let structure = MarketStructureDeclaration::IsoNeDamAssetTab {
        asset_tab_registration_ref: "isone-asset-tab-1".to_string(),
    };
    let grant = active_grant("tenant-1", structure.name());
    let inputs = vec![
        asset_input("asset-a", [1.5, 2.5]),
        asset_input("asset-b", [0.5, 1.0]),
    ];
    let record = compose_bid(
        &structure,
        &grant,
        REQUIRED_CONTROL_MODE_NAME,
        "tenant-1",
        &INTERVAL_BOUNDS,
        &inputs,
        &clock,
    )
    .expect("the ISO-NE composition is valid");
    match record {
        ComposedBidRecord::IsoNeDamAssetBid {
            asset_tab_registration_ref,
            per_asset,
        } => {
            assert_eq!(asset_tab_registration_ref, "isone-asset-tab-1");
            assert_eq!(per_asset.len(), 2);
            // Ascending asset_id order (determinism).
            assert_eq!(per_asset[0].asset_id, "asset-a");
            assert_eq!(per_asset[1].asset_id, "asset-b");
            // PER ASSET: asset-a's quantities stay asset-a's — no
            // cross-asset aggregation anywhere in the record.
            assert!((per_asset[0].intervals[0].quantity_mw - 1.5).abs() < 1e-12);
            assert!((per_asset[1].intervals[0].quantity_mw - 0.5).abs() < 1e-12);
            assert_eq!(per_asset[0].envelope_ref, "envelope-v7-asset-a");
        }
        other => panic!("expected an ISO-NE Asset-tab bid, got: {other:?}"),
    }
}

#[test]
fn an_input_without_envelope_provenance_is_refused() {
    let clock = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let structure = MarketStructureDeclaration::PjmSingleAggregationNode {
        aggregation_node_ref: "pjm-node-A".to_string(),
    };
    let grant = active_grant("tenant-1", structure.name());
    let inputs = vec![AssetBidInput {
        asset_id: "asset-a".to_string(),
        envelope_ref: String::new(),
        interval_quantities_mw: vec![1.0, 1.0],
    }];
    let error = compose_bid(
        &structure,
        &grant,
        REQUIRED_CONTROL_MODE_NAME,
        "tenant-1",
        &INTERVAL_BOUNDS,
        &inputs,
        &clock,
    )
    .expect_err("a bid without envelope provenance must be refused");
    assert!(matches!(
        error,
        BidCompositionError::MissingEnvelopeProvenance { ref asset_id } if asset_id == "asset-a"
    ));
}

#[test]
fn composition_requires_operator_approved_mode() {
    let clock = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let structure = MarketStructureDeclaration::PjmSingleAggregationNode {
        aggregation_node_ref: "pjm-node-A".to_string(),
    };
    let grant = active_grant("tenant-1", structure.name());
    let inputs = vec![asset_input("asset-a", [1.0, 1.0])];
    for mode in ["simulation-shadow", "read-only-shadow", "bounded-automation"] {
        let error = compose_bid(
            &structure,
            &grant,
            mode,
            "tenant-1",
            &INTERVAL_BOUNDS,
            &inputs,
            &clock,
        )
        .expect_err("composition outside operator-approved mode must be refused");
        assert!(
            matches!(error, BidCompositionError::ControlModeInsufficient { ref mode_name } if mode_name == mode),
            "unexpected error for mode {mode}: {error:?}"
        );
    }
}

#[test]
fn composition_requires_an_active_unexpired_matching_grant() {
    let clock = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let structure = MarketStructureDeclaration::PjmSingleAggregationNode {
        aggregation_node_ref: "pjm-node-A".to_string(),
    };
    let inputs = vec![asset_input("asset-a", [1.0, 1.0])];

    // A REVOKED grant is refused.
    let revoked = MarketBidGrant::new(
        "grant-1".to_string(),
        "tenant-1".to_string(),
        structure.name().to_string(),
        "REVOKED".to_string(),
        "operator-a".to_string(),
        PINNED_NOW_UNIX_MS - 60_000,
        GRANT_VALID_UNTIL,
        "audit-record-1".to_string(),
    )
    .expect("the fixture grant is valid");
    let error = compose_bid(
        &structure,
        &revoked,
        REQUIRED_CONTROL_MODE_NAME,
        "tenant-1",
        &INTERVAL_BOUNDS,
        &inputs,
        &clock,
    )
    .expect_err("a revoked grant must be refused");
    assert!(matches!(
        error,
        BidCompositionError::GrantNotActive { ref grant_state, .. } if grant_state == "REVOKED"
    ));

    // An EXPIRED grant (validity ended before the injected now) is
    // refused.
    let expired = MarketBidGrant::new(
        "grant-1".to_string(),
        "tenant-1".to_string(),
        structure.name().to_string(),
        "ACTIVE".to_string(),
        "operator-a".to_string(),
        PINNED_NOW_UNIX_MS - 60_000,
        PINNED_NOW_UNIX_MS - 1,
        "audit-record-1".to_string(),
    )
    .expect("the fixture grant is valid");
    let error = compose_bid(
        &structure,
        &expired,
        REQUIRED_CONTROL_MODE_NAME,
        "tenant-1",
        &INTERVAL_BOUNDS,
        &inputs,
        &clock,
    )
    .expect_err("an expired grant must be refused");
    assert!(matches!(error, BidCompositionError::GrantExpired { .. }));

    // A grant scoped to ANOTHER tenant is refused.
    let foreign = active_grant("tenant-2", structure.name());
    let error = compose_bid(
        &structure,
        &foreign,
        REQUIRED_CONTROL_MODE_NAME,
        "tenant-1",
        &INTERVAL_BOUNDS,
        &inputs,
        &clock,
    )
    .expect_err("a foreign-tenant grant must be refused");
    assert!(matches!(error, BidCompositionError::GrantTenantMismatch { .. }));

    // A grant for a DIFFERENT market structure is refused.
    let wrong_market = active_grant(
        "tenant-1",
        "CAISO_PDR_TWO_ID_LOAD_SHIFT",
    );
    let error = compose_bid(
        &structure,
        &wrong_market,
        REQUIRED_CONTROL_MODE_NAME,
        "tenant-1",
        &INTERVAL_BOUNDS,
        &inputs,
        &clock,
    )
    .expect_err("a wrong-market grant must be refused");
    assert!(matches!(error, BidCompositionError::GrantMarketMismatch { .. }));
}

#[test]
fn composition_is_deterministic_and_replays_identically() {
    let clock_a = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let clock_b = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let structure = MarketStructureDeclaration::PjmSingleAggregationNode {
        aggregation_node_ref: "pjm-node-A".to_string(),
    };
    let grant = active_grant("tenant-1", structure.name());
    let inputs = vec![
        asset_input("asset-c", [1.0, 1.0]),
        asset_input("asset-a", [2.0, 2.0]),
        asset_input("asset-b", [3.0, 3.0]),
    ];
    let run_a = compose_bid(
        &structure,
        &grant,
        REQUIRED_CONTROL_MODE_NAME,
        "tenant-1",
        &INTERVAL_BOUNDS,
        &inputs,
        &clock_a,
    )
    .expect("the composition is valid");
    let run_b = compose_bid(
        &structure,
        &grant,
        REQUIRED_CONTROL_MODE_NAME,
        "tenant-1",
        &INTERVAL_BOUNDS,
        &inputs,
        &clock_b,
    )
    .expect("the composition is valid");
    assert_eq!(run_a, run_b, "same inputs + same clock must replay identically");
    if let ComposedBidRecord::PjmAggregationBid {
        contributing_assets, ..
    } = run_a
    {
        // BTreeMap ordering: ascending regardless of input order.
        assert_eq!(
            contributing_assets,
            vec!["asset-a".to_string(), "asset-b".to_string(), "asset-c".to_string()]
        );
    } else {
        panic!("expected the PJM record");
    }
}

#[test]
fn no_bid_is_submitted_and_none_can_be() {
    // The statement is pinned: the output is a RECORD; no transmission
    // path exists anywhere in this module.
    assert!(NO_BID_SUBMITTED_STATEMENT.contains("No bid is submitted — none can be"));
    // The required mode name is the kernel posture's exact name.
    assert_eq!(REQUIRED_CONTROL_MODE_NAME, "operator-approved");
}

#[test]
fn invalid_quantities_are_refused() {
    let clock = FixedClock::at_unix_millis(PINNED_NOW_UNIX_MS);
    let structure = MarketStructureDeclaration::PjmSingleAggregationNode {
        aggregation_node_ref: "pjm-node-A".to_string(),
    };
    let grant = active_grant("tenant-1", structure.name());
    let negative = vec![asset_input("asset-a", [-1.0, 1.0])];
    let error = compose_bid(
        &structure,
        &grant,
        REQUIRED_CONTROL_MODE_NAME,
        "tenant-1",
        &INTERVAL_BOUNDS,
        &negative,
        &clock,
    )
    .expect_err("a negative quantity must be refused");
    assert!(matches!(
        error,
        BidCompositionError::InvalidQuantity { ref asset_id, interval_index: 0 } if asset_id == "asset-a"
    ));
    let not_finite = vec![asset_input("asset-a", [f64::NAN, 1.0])];
    let error = compose_bid(
        &structure,
        &grant,
        REQUIRED_CONTROL_MODE_NAME,
        "tenant-1",
        &INTERVAL_BOUNDS,
        &not_finite,
        &clock,
    )
    .expect_err("a non-finite quantity must be refused");
    assert!(matches!(error, BidCompositionError::InvalidQuantity { .. }));
    let mismatched = vec![asset_input("asset-a", [1.0])];
    let error = compose_bid(
        &structure,
        &grant,
        REQUIRED_CONTROL_MODE_NAME,
        "tenant-1",
        &INTERVAL_BOUNDS,
        &mismatched,
        &clock,
    )
    .expect_err("a misaligned interval grid must be refused");
    assert!(matches!(
        error,
        BidCompositionError::IntervalCountMismatch { expected: 2, found: 1, .. }
    ));
}
