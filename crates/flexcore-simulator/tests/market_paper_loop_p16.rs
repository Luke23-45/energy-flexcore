// P16.21 — Market integration test suite, part 2 (the paper-trading
//          simulator, P16.20): the full paper loop as RECORDS ONLY, the
//          simulated feed ON the unwired `IsoPricePublication` seam, the
//          synthetic settlement fixtures round-tripping through the P16.09
//          grammars, the simulated export sink ON the unwired
//          `TelemetryExportSink` seam (in-memory, bounded, same checks),
//          the loop's replay determinism, and the paper-ledger reference
//          behavior. WRITTEN-UNRUN.
//
// Phase 16 plan §2 row P16.21; decision record:
// `docs/phases-11-20/phase-16-market-integration-settlement/records/
// 63_p16_21_market_integration_test_suite.md`.
//
// HONESTY (recorded in the decision record): no market is connected; no
// bid is submitted (none can be); no instruction is issued (a real
// instruction enters ONLY through the command lifecycle); no payment
// moves and no payment state is entered (the ledger leg carries a
// REFERENCE — the P16.14 vocabulary's single home owns the states); no
// settlement number carries market meaning. Execution is a HUMAN GATE —
// per the loop bans, cargo was NOT run while authoring these tests; the
// first `cargo test -p flexcore-simulator --test market_paper_loop_p16`
// run is the verification obligation.
//
// WAIT-CHECK (recorded): the payment-lifecycle (P16.14) module of wave-2
// Agent A had NOT landed when this suite was written (directory
// inspection); the payment-lifecycle state-machine tests against THAT
// module's own vocabulary are a recorded handoff (decision record 63 §3),
// never guessed here.

use flexcore_application::bid_composition::{
    AssetBidInput, ComposedBidRecord, MarketBidGrant, MarketStructureDeclaration,
};
use flexcore_application::settlement_models::{
    extract_caiso_sqmd_row, extract_ebgl_isp_row, extract_pjm_m29_bli_csv,
};
use flexcore_application::telemetry_export_design::{
    ExportMarket, ExportPostureRecord, ExportQuantity, ExportSeamError, TelemetryExportSink,
    ERCOT_ICCP_BUDGET_MILLIS,
};
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{ClockQuality, EventTime, IngestTime};
use flexcore_contracts::units::ActivePowerMW;
use flexcore_domain::ids::{AssetId, TenantId};
use flexcore_ports::fakes::FixedClock;
use flexcore_ports::telemetry_source::{
    NormalizedTelemetry, NormalizedTelemetryParts, QuantityValue, TelemetryQuantity,
};
use flexcore_simulator::market::export_sink::{
    SimulatedTelemetryExportSink, RECEIPTS_ARE_NOT_A_STORE_STATEMENT, RING_IS_BOUNDED_STATEMENT,
    SIM_EXPORT_SINK_STATEMENT,
};
use flexcore_simulator::market::paper_loop::{
    run_paper_cycle, PaperCycleConfig, PaperLoopError, PAPER_DISPATCH_STATEMENT,
    PAPER_LEDGER_STATEMENT, PAPER_LOOP_STATEMENT,
};
use flexcore_simulator::market::price_feed::{
    SimulatedIsoPriceFeed, SimulatedPriceFeedConfig, SIM_FEED_IS_SIMULATION_STATEMENT,
};
use flexcore_simulator::market::settlement_fixtures::{
    simulated_caiso_sqmd_rows, simulated_ebgl_isp_rows, simulated_pjm_csv_statement,
    SIMULATED_STATEMENTS_ARE_SYNTHETIC_STATEMENT,
};

/// The pinned test instant (UTC; the injected clock — never a wall clock).
const PINNED_NOW_UNIX_MS: i64 = 1_760_000_000_000;
const TENANT: &str = "paper-tenant";
const CURRENCY: &str = "USD";

// --- Fixtures -----------------------------------------------------------------

fn grant(now: i64) -> MarketBidGrant {
    MarketBidGrant::new(
        "grant://p16-20/1".to_string(),
        TENANT.to_string(),
        "PJM_SINGLE_AGGREGATION_NODE".to_string(),
        "ACTIVE".to_string(),
        "paper-operator".to_string(),
        now,
        now + 3_600_000,
        "audit://p16-20/1".to_string(),
    )
    .expect("valid grant")
}

fn paper_config(now: i64) -> PaperCycleConfig {
    let start = 1_760_000_000_000;
    PaperCycleConfig {
        cycle_ref: "p16-21-cycle".to_string(),
        tenant_ref: TENANT.to_string(),
        control_mode_name: "operator-approved".to_string(),
        structure: MarketStructureDeclaration::PjmSingleAggregationNode {
            aggregation_node_ref: "sim-node-1".to_string(),
        },
        grant: grant(now),
        interval_bounds: (0..4)
            .map(|i| (start + i * 300_000, start + (i + 1) * 300_000))
            .collect(),
        asset_inputs: vec![AssetBidInput {
            asset_id: "paper-asset-1".to_string(),
            envelope_ref: "envelope://paper/1@v1".to_string(),
            interval_quantities_mw: vec![1.5, 1.5, 2.0, 2.0],
        }],
        currency: CURRENCY.to_string(),
        statement_lines: vec![
            ("ENERGY_CHARGE".to_string(), 123_456),
            ("CAPACITY_CHARGE".to_string(), 25_000),
        ],
        expected_cents_override: vec![None, None],
        delivered_energy_tenths_base: 15,
        payment_state_ref: "payment-state://paper/1".to_string(),
    }
}

fn paper_feed() -> SimulatedIsoPriceFeed {
    SimulatedIsoPriceFeed::new(
        SimulatedPriceFeedConfig::new(
            "paper-lab".to_string(),
            flexcore_application::market_price_records::MarketSegment::DayAhead,
            "SIM-ZONE".to_string(),
            CURRENCY.to_string(),
            4250,
            150,
            300_000,
            8,
        )
        .expect("valid feed config"),
    )
}

/// One canonical power telemetry record from the simulated fleet (the
/// same record shape a real `TelemetrySource` decodes — the simulator is
/// its provider; the identity stays `sim://`).
fn paper_power_record(sequence: u64, power_mw: f64) -> NormalizedTelemetry {
    NormalizedTelemetry::new(NormalizedTelemetryParts {
        tenant_id: TenantId::new(FlexUuid::nil()),
        asset_id: AssetId::new(FlexUuid::nil()),
        quantity: TelemetryQuantity::ActivePower,
        value: Some(QuantityValue::Power(
            ActivePowerMW::new(power_mw).expect("valid fixture power"),
        )),
        event_time: EventTime::from_unix_millis(PINNED_NOW_UNIX_MS).expect("valid instant"),
        ingest_time: IngestTime::from_unix_millis(PINNED_NOW_UNIX_MS + 1_000)
            .expect("valid instant"),
        source_sequence: sequence,
        source_id: "sim://market/paper".to_string(),
        quality: TelemetryQuality::Good,
        source_quality_code: None,
        clock_quality: ClockQuality::Synchronized,
    })
    .expect("valid fixture record")
}

fn ercot_posture() -> ExportPostureRecord {
    ExportPostureRecord::new(
        ExportMarket::ErcotAder,
        ERCOT_ICCP_BUDGET_MILLIS,
        vec![ExportQuantity::ActivePower],
        "sim://market/export/ercot-iccp-class".to_string(),
        "simulated posture (P16.20); the real budget cites [S-100][S-101] with the \
         re-verify-at-use rule"
            .to_string(),
    )
    .expect("valid posture")
}

// --- The loop -----------------------------------------------------------------

#[test]
fn the_full_paper_loop_runs_in_records_only() {
    let now = PINNED_NOW_UNIX_MS;
    let cfg = paper_config(now);
    let mut simulated_feed = paper_feed();
    let clock = FixedClock::at_unix_millis(now);
    let record =
        run_paper_cycle(&cfg, &mut simulated_feed, now, &clock).expect("the paper cycle completes");
    // SIGNAL: the feed's recipe, as exact-decimal records.
    assert_eq!(record.signal_records.len(), 8);
    assert_eq!(record.signal_records[0].price.canonical_string(), "425.0 USD");
    assert!(record.signal_records[0].record_ref.starts_with("sim://market/"));
    // BID: the composer's own output (one asset; sums equal its inputs).
    match &record.bid {
        ComposedBidRecord::PjmAggregationBid { intervals, .. } => {
            assert_eq!(intervals.len(), 4);
            assert_eq!(intervals[0].quantity_mw, 1.5);
            assert_eq!(intervals[3].quantity_mw, 2.0);
        }
        other => panic!("expected the PJM aggregation record, got {other:?}"),
    }
    // DISPATCH: a paper record, never a command.
    assert_eq!(
        record.dispatch.instruction_ref,
        "sim://market/paper/dispatch/p16-21-cycle"
    );
    assert_eq!(record.dispatch.market_structure_name, "PJM_SINGLE_AGGREGATION_NODE");
    // EVIDENCE: integer-tenths texts with the canonical GOOD state.
    assert_eq!(record.evidence.delivered_energy_mwh_text[0], "1.5");
    assert_eq!(record.evidence.quality_canonical_name, "GOOD");
    // STATEMENT: the synthetic text carries the sim identity and parses.
    assert!(record.statement_text.contains("sim://statement/m29/csv/p16-21-cycle"));
    assert_eq!(record.statement_items.len(), 2);
    // RECONCILIATION: the positive path matches exactly.
    assert_eq!(record.reconciliation.matched_count, 2);
    assert_eq!(record.reconciliation.mismatch_count, 0);
    // PAYMENT-STATE: a REFERENCE, never a state entry.
    assert_eq!(record.ledger.payment_state_ref, "payment-state://paper/1");
    // The honesty statements pin the loop's boundaries.
    assert!(PAPER_LOOP_STATEMENT.contains("SIMULATION ONLY"));
    assert!(PAPER_DISPATCH_STATEMENT.contains("never a command"));
    assert!(PAPER_LEDGER_STATEMENT.contains("no payment moves"));
    assert!(SIM_FEED_IS_SIMULATION_STATEMENT.contains("never a connection"));
    assert!(SIMULATED_STATEMENTS_ARE_SYNTHETIC_STATEMENT.contains("SYNTHETIC"));
}

#[test]
fn the_paper_loop_replays_byte_identically() {
    let now = PINNED_NOW_UNIX_MS;
    let cfg = paper_config(now);
    let mut a = paper_feed();
    let mut b = paper_feed();
    let first =
        run_paper_cycle(&cfg, &mut a, now, &FixedClock::at_unix_millis(now)).expect("cycle");
    let second =
        run_paper_cycle(&cfg, &mut b, now, &FixedClock::at_unix_millis(now)).expect("cycle");
    assert_eq!(first.signal_records, second.signal_records);
    assert_eq!(first.bid, second.bid);
    assert_eq!(first.dispatch, second.dispatch);
    assert_eq!(first.evidence, second.evidence);
    assert_eq!(first.statement_text, second.statement_text);
    assert_eq!(first.statement_items, second.statement_items);
    assert_eq!(first.reconciliation, second.reconciliation);
    assert_eq!(first.ledger, second.ledger);
}

#[test]
fn a_mismatched_statement_line_becomes_a_queue_entry_never_a_correction() {
    let now = PINNED_NOW_UNIX_MS;
    let mut cfg = paper_config(now);
    cfg.expected_cents_override = vec![None, Some(99_999)];
    let mut simulated_feed = paper_feed();
    let record = run_paper_cycle(&cfg, &mut simulated_feed, now, &FixedClock::at_unix_millis(now))
        .expect("the paper cycle completes");
    assert_eq!(record.reconciliation.matched_count, 1);
    assert_eq!(record.reconciliation.mismatch_count, 1);
    assert_eq!(record.reconciliation.dispute_queue.len(), 1);
    let entry = &record.reconciliation.dispute_queue[0];
    assert_eq!(entry.reason_code, "LINE_AMOUNT_MISMATCH");
    assert_eq!(
        entry.stated.as_ref().expect("stated").canonical_string(),
        "250.00 USD"
    );
    assert_eq!(
        entry.expected.as_ref().expect("expected").canonical_string(),
        "999.99 USD"
    );
}

#[test]
fn the_loop_refuses_without_a_signal_and_outside_the_grant() {
    let now = PINNED_NOW_UNIX_MS;
    // No signal: an unrepresentable window refuses (never fabricated).
    let cfg = paper_config(now);
    let mut simulated_feed = paper_feed();
    match run_paper_cycle(&cfg, &mut simulated_feed, -1, &FixedClock::at_unix_millis(now)) {
        Err(PaperLoopError::SimulatedFeed(e)) => {
            assert_eq!(e.name(), "SIM_FEED_WINDOW_UNREPRESENTABLE")
        }
        other => panic!("expected the feed refusal, got {other:?}"),
    }
    // Opt-out: a REVOKED grant refuses the bid leg (the composer's own
    // gate; the loop adds nothing).
    let mut cfg = paper_config(now);
    cfg.grant = MarketBidGrant::new(
        "grant://p16-20/2".to_string(),
        TENANT.to_string(),
        "PJM_SINGLE_AGGREGATION_NODE".to_string(),
        "REVOKED".to_string(),
        "paper-operator".to_string(),
        now,
        now + 3_600_000,
        "audit://p16-20/2".to_string(),
    )
    .expect("valid grant");
    let mut simulated_feed = paper_feed();
    match run_paper_cycle(&cfg, &mut simulated_feed, now, &FixedClock::at_unix_millis(now)) {
        Err(PaperLoopError::BidComposition(
            flexcore_application::bid_composition::BidCompositionError::GrantNotActive {
                grant_state,
                ..
            },
        )) => assert_eq!(grant_state, "REVOKED"),
        other => panic!("expected the grant refusal, got {other:?}"),
    }
}

// --- The simulated feed ON the unwired seam ------------------------------------

#[test]
fn the_simulated_feed_exercises_the_unwired_price_seam() {
    use flexcore_application::market_price_records::{IsoPricePublication, MarketSegment};
    let mut feed = paper_feed();
    // Absent before publication — never fabricated, never replayed stale.
    assert_eq!(
        IsoPricePublication::latest_publication(&feed, MarketSegment::DayAhead).expect("absent"),
        None
    );
    let batch = feed.publish_window(PINNED_NOW_UNIX_MS).expect("window");
    // The configured segment answers the batch; another segment is ABSENT.
    let latest = IsoPricePublication::latest_publication(&feed, MarketSegment::DayAhead)
        .expect("present")
        .expect("some batch");
    assert_eq!(latest, batch);
    assert_eq!(
        IsoPricePublication::latest_publication(&feed, MarketSegment::RealTime).expect("absent"),
        None
    );
    // Deterministic: the same window start ⇒ byte-identical text.
    let again = feed.publish_window(PINNED_NOW_UNIX_MS).expect("window");
    assert_eq!(again, batch);
}

// --- The synthetic settlement fixtures THROUGH the P16.09 grammars -------------

#[test]
fn the_simulated_statements_round_trip_through_the_fixture_grammars() {
    // PJM M29 CSV: generated, parsed, line refs exactly as the extractor
    // derives them.
    let statement_ref = "sim://statement/m29/csv/p16-21";
    let text = simulated_pjm_csv_statement(
        statement_ref,
        CURRENCY,
        &[("ENERGY_CHARGE", 100_00), ("CAPACITY_CHARGE", -50)],
    );
    let items = extract_pjm_m29_bli_csv(&text).expect("valid statement");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].line_ref, format!("{statement_ref}#0"));
    assert_eq!(items[1].amount.canonical_string(), "-0.50 USD");
    // CAISO SQMD: exactly 5-minute intervals; re-binning stays impossible.
    // Quantities are INTEGER TENTHS (the generator's documented unit):
    // 4250 tenths round-trips as 425.0, and each row adds one tenth.
    let rows = simulated_caiso_sqmd_rows(1_000, 2, 4250, "kWh", "ESTIMATED");
    let first = extract_caiso_sqmd_row(&rows[0]).expect("valid row");
    assert_eq!(first.meter_quantity.canonical_string(), "425.0");
    assert_eq!(
        first.interval_end.unix_millis() - first.interval_start.unix_millis(),
        300_000
    );
    let second = extract_caiso_sqmd_row(&rows[1]).expect("valid row");
    assert_eq!(second.meter_quantity.canonical_string(), "425.1");
    // EBGL ISP: exactly 15-minute ISPs; the alternating sign pattern
    // extracts as SURPLUS/DEFICIT under the declared convention.
    // Imbalances are INTEGER TENTHS: 9000 tenths round-trips as 900.0.
    let isp_rows = simulated_ebgl_isp_rows(1_000, 2, 9000);
    let surplus = extract_ebgl_isp_row(&isp_rows[0]).expect("valid row");
    assert_eq!(surplus.imbalance_energy_mwh.canonical_string(), "900.0");
    let deficit = extract_ebgl_isp_row(&isp_rows[1]).expect("valid row");
    assert_eq!(deficit.imbalance_energy_mwh.canonical_string(), "-900.1");
    assert_eq!(
        deficit.isp_end.unix_millis() - deficit.isp_start.unix_millis(),
        900_000
    );
}

// --- The simulated export sink ON the unwired seam ------------------------------

#[test]
fn the_simulated_export_sink_records_in_memory_and_transmits_nothing() {
    let sink =
        SimulatedTelemetryExportSink::new(ExportMarket::ErcotAder, 4).expect("valid capacity");
    // The same checks, same gates: a mismatched posture refuses BEFORE
    // anything is recorded (the trait's own PostureMarketMismatch error).
    let wrong_posture = ExportPostureRecord::new(
        ExportMarket::NyisoEsr,
        6_000,
        vec![ExportQuantity::ActivePower, ExportQuantity::Soc],
        "sim://market/export/nyiso-scada-class".to_string(),
        "simulated posture (P16.20); the real budget cites [S-105][S-106]".to_string(),
    )
    .expect("valid posture");
    let err = sink
        .export_batch(&wrong_posture, &[paper_power_record(1, 1.5)])
        .expect_err("a mismatched posture refuses");
    assert!(matches!(err, ExportSeamError::PostureMarketMismatch { .. }));
    assert!(sink.receipts().is_empty());
    // A matching posture is ACCEPTED IN MEMORY — a receipt with the sim
    // identity, the record COUNT (never the payloads), and the posture's
    // transport reference.
    sink.export_batch(&ercot_posture(), &[paper_power_record(1, 1.5), paper_power_record(2, -0.5)])
        .expect("the batch is recorded in memory");
    let receipts = sink.receipts();
    assert_eq!(receipts.len(), 1);
    assert!(receipts[0].receipt_ref.starts_with("sim-export://"));
    assert_eq!(receipts[0].record_count, 2);
    assert_eq!(
        receipts[0].posture_transport_ref,
        "sim://market/export/ercot-iccp-class"
    );
    assert_eq!(receipts[0].identity_prefix(), "sim-export");
    // The honesty statements pin the boundary.
    assert!(SIM_EXPORT_SINK_STATEMENT.contains("IN-MEMORY"));
    assert!(SIM_EXPORT_SINK_STATEMENT.contains("transmits nothing"));
    assert!(RING_IS_BOUNDED_STATEMENT.contains("OLDEST receipt is dropped"));
    assert!(RECEIPTS_ARE_NOT_A_STORE_STATEMENT.contains("second telemetry store"));
}

#[test]
fn the_export_sink_ring_drops_the_oldest_receipt_when_declared_full() {
    let sink =
        SimulatedTelemetryExportSink::new(ExportMarket::ErcotAder, 2).expect("valid capacity");
    let posture = ercot_posture();
    for sequence in 1..=3u64 {
        sink.export_batch(&posture, &[paper_power_record(sequence, 1.0)])
            .expect("recorded");
    }
    let receipts = sink.receipts();
    // Capacity 2: the FIRST receipt was displaced by the third (the
    // declared ring behavior), and the receipts carry their sim identity.
    assert_eq!(sink.capacity(), 2);
    assert_eq!(receipts.len(), 2);
    assert!(receipts.iter().all(|r| r.receipt_ref.starts_with("sim-export://")));
    // The two survivors are the second and third exports (the ring keeps
    // order oldest-first among the survivors).
    assert_eq!(receipts[0].record_count, 1);
    assert_eq!(receipts[1].record_count, 1);
}

#[test]
fn a_zero_capacity_export_sink_is_unconstructable() {
    assert!(SimulatedTelemetryExportSink::new(ExportMarket::ErcotAder, 0).is_none());
}
