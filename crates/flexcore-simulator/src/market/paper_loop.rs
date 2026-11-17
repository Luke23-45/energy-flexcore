// P16.20 — The paper-trading loop: SIGNAL → BID → DISPATCH → EVIDENCE →
//         STATEMENT → RECONCILIATION → PAYMENT-STATE reference, end to
//         end, with ZERO market connectivity.
//
// The loop composes the wave-1 application engines — it never redefines
// one:
//
// | Leg | Engine consumed (single home) | What the loop adds |
// |---|---|---|
// | SIGNAL | `flexcore_application::market_price_records` (P16.07): the simulated feed (see [`crate::market::price_feed`]) implements the UNWIRED `IsoPricePublication` seam; `records_from_publication` builds the exact-decimal records | nothing — the records ARE the seam's own output |
// | BID | `flexcore_application::bid_composition::compose_bid` (P16.06): operator-approved mode + ACTIVE `MarketBidGrant` enforced by the composer itself | nothing — the composed record is the composer's own output |
// | DISPATCH | — | a `PaperDispatchInstruction` RECORD of what WOULD be relayed; a real instruction enters ONLY through the 14-state command lifecycle (the P14.16 command-relay-only rule) — the simulator never issues, never authorizes, never touches the lifecycle |
// | EVIDENCE | `flexcore_contracts::quality` (the §10.4 vocabulary, consumed) | a `PaperEvidenceRecord` with deterministic synthetic delivered-energy texts and the canonical GOOD state |
// | STATEMENT | `crate::market::settlement_fixtures` generates the SYNTHETIC statement; `flexcore_application::settlement_models::extract_pjm_m29_bli_csv` (P16.09) parses it | nothing — the extractor is the single grammar home |
// | RECONCILIATION | `flexcore_application::settlement_reconciliation::reconcile_bli_statement` (P16.09) | nothing — the report (with its dispute queue) is the engine's own output |
// | PAYMENT-STATE reference | — | a `PaperLedgerEntry` carrying a REFERENCE to a payment-state record. The payment-state vocabulary's single home is the P16.14 payment-lifecycle module; this loop carries the reference STRING (validated non-empty), never a redefinition or a mirror of that vocabulary — no payment moves, no state is entered, no settlement number is claimed |
//
// Honesty invariants (pinned by tests, all carried in the module-tree
// header): records only, never a connection; no bid is submitted — none
// can be; every money value is integer arithmetic rendered as decimal
// text (no float, no RNG); determinism — the same config + same clock +
// same window ⇒ byte-identical stage outputs; nothing has ever run.

use flexcore_application::bid_composition::{
    compose_bid, AssetBidInput, ComposedBidRecord, MarketBidGrant, MarketStructureDeclaration,
};
use flexcore_application::market_price_records::{MarketPriceRecord, PriceSourceClass};
use flexcore_application::settlement_models::{
    extract_pjm_m29_bli_csv, PjmBliLineItem,
};
use flexcore_application::settlement_reconciliation::{
    reconcile_bli_statement, ExpectedLine, ReconciliationError, ReconciliationReport, Tolerance,
};
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_ports::clock::Clock;

use crate::market::price_feed::{SimulatedFeedError, SimulatedIsoPriceFeed};
use crate::market::settlement_fixtures::{
    render_cents, simulated_pjm_csv_statement, SIMULATED_STATEMENT_IDENTITY_PREFIX,
};

/// The paper-loop identity prefix (every record this module produces is
/// identifiable from its identity alone, forever).
pub const PAPER_LOOP_IDENTITY_PREFIX: &str = "sim://market/paper";

/// The paper-loop statement (pinned by tests).
pub const PAPER_LOOP_STATEMENT: &str = "The paper-trading loop is \
     SIMULATION ONLY: no market is connected, no bid is submitted (none \
     can be), no instruction is issued (a real instruction enters ONLY \
     through the command lifecycle), no payment moves, and no settlement \
     number is claimed. Every stage output is a deterministic record \
     built from integer arithmetic and wire-faithful decimal text; the \
     loop feeds replay/benchmark harnesses as RECORDS, never connections.";

/// The dispatch-leg statement (pinned by tests).
pub const PAPER_DISPATCH_STATEMENT: &str = "A PaperDispatchInstruction is a \
     RECORD of what WOULD be relayed — never a command. A real instruction \
     enters ONLY through the 14-state command lifecycle with validation, \
     feasibility, and authorization as distinct gates (the P14.16 \
     command-relay-only rule, consumed by citation); the simulator never \
     issues, never authorizes, and never constructs a forward lifecycle \
     state.";

/// The ledger-leg statement (pinned by tests).
pub const PAPER_LEDGER_STATEMENT: &str = "A PaperLedgerEntry carries a \
     REFERENCE to a payment-state record — the payment-state vocabulary's \
     single home is the P16.14 payment-lifecycle module. The loop never \
     redefines or mirrors that vocabulary: no payment state is entered, \
     no payment moves, and no settlement number is claimed (plan §8 \
     criterion 4).";

/// Why a paper cycle refused to run. Named, never absorbed; every
/// underlying engine error rides unchanged.
#[derive(Debug, Clone, PartialEq)]
pub enum PaperLoopError {
    /// A required text field was empty or whitespace.
    EmptyField(&'static str),
    /// The simulated feed published nothing for the configured segment —
    /// no signal, no cycle (an absent publication is never fabricated).
    NoSignalPublication,
    /// The feed refused the window (the feed's named error rides).
    SimulatedFeed(SimulatedFeedError),
    /// The bid composer refused (the composer's named error rides —
    /// mode, grant, provenance, quantities).
    BidComposition(flexcore_application::bid_composition::BidCompositionError),
    /// A price record could not be assembled at the record boundary.
    PriceRecord(flexcore_application::market_price_records::PriceRecordError),
    /// The generated statement failed its own grammar (a fixture-author
    /// bug — never silently absorbed).
    StatementParse(flexcore_application::settlement_models::SettlementParseError),
    /// The reconciliation engine refused the run (the engine's named
    /// error rides).
    Reconciliation(ReconciliationError),
    /// The interval bounds were empty or non-positive.
    IntervalBoundsInvalid,
}

impl std::fmt::Display for PaperLoopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PaperLoopError::EmptyField(field) => {
                write!(f, "the paper-loop field '{field}' must be non-empty")
            }
            PaperLoopError::NoSignalPublication => write!(
                f,
                "the simulated feed published nothing for the configured segment — no \
                 signal, no cycle (an absent publication is never fabricated)"
            ),
            PaperLoopError::SimulatedFeed(e) => write!(f, "the simulated feed refused: {e}"),
            PaperLoopError::BidComposition(e) => write!(f, "the bid composer refused: {e}"),
            PaperLoopError::PriceRecord(e) => {
                write!(f, "a price record failed the record boundary: {e}")
            }
            PaperLoopError::StatementParse(e) => {
                write!(f, "the generated statement failed its declared grammar: {e}")
            }
            PaperLoopError::Reconciliation(e) => write!(f, "the reconciliation engine refused: {e}"),
            PaperLoopError::IntervalBoundsInvalid => {
                write!(f, "the interval bounds are empty or non-positive")
            }
        }
    }
}

impl std::error::Error for PaperLoopError {}

/// The validated configuration of one paper cycle. Every field is
/// synthetic fixture data; nothing references a real market, counterparty,
/// or account.
#[derive(Debug, Clone)]
pub struct PaperCycleConfig {
    /// The cycle reference (built into every stage's identity strings
    /// with the [`PAPER_LOOP_IDENTITY_PREFIX`] prefix).
    pub cycle_ref: String,
    /// The tenant reference the cycle runs under (the grant's tenant).
    pub tenant_ref: String,
    /// The control-authority mode NAME (must be the kernel posture name
    /// `operator-approved` — one home, consumed by name; the composer
    /// enforces it).
    pub control_mode_name: String,
    /// The market structure to compose the bid for.
    pub structure: MarketStructureDeclaration,
    /// The explicit per-market grant record (P16.06; validated by its own
    /// constructor).
    pub grant: MarketBidGrant,
    /// The bid window's per-interval [start, end) bounds, end-exclusive.
    pub interval_bounds: Vec<(i64, i64)>,
    /// The asset bid inputs (envelope-derived quantities with the
    /// envelope-version reference — no provenance, no bid).
    pub asset_inputs: Vec<AssetBidInput>,
    /// The statement currency (currency as DATA).
    pub currency: String,
    /// The statement's billing lines: charge-type label (verbatim
    /// fixture data) and the amount in integer CENTS for BOTH the
    /// generated statement and the expected set (the positive path
    /// reconciles exactly; a deliberately-perturbed expected set is the
    /// mismatch negative case, driven by `expected_cents_override`).
    pub statement_lines: Vec<(String, i64)>,
    /// Per-line expected cents override (indexed alongside
    /// `statement_lines`; `None` entries take the statement value). The
    /// override exists for the mismatch/dispute negative cases ONLY.
    pub expected_cents_override: Vec<Option<i64>>,
    /// The deterministic synthetic delivered-energy base per interval, in
    /// integer TENTHS of MWh (evidence leg; interval `i` records
    /// `base + i` tenths — a pure integer recipe).
    pub delivered_energy_tenths_base: i64,
    /// The payment-state record reference the ledger entry carries (a
    /// REFERENCE — the P16.14 vocabulary's home owns the states; never a
    /// redefinition here).
    pub payment_state_ref: String,
}

impl PaperCycleConfig {
    /// Validates the config's own fields (fail-closed). The grant and
    /// asset inputs are validated by their own constructors/composer.
    pub fn validate(&self) -> Result<(), PaperLoopError> {
        if self.cycle_ref.trim().is_empty() {
            return Err(PaperLoopError::EmptyField("cycle_ref"));
        }
        if self.tenant_ref.trim().is_empty() {
            return Err(PaperLoopError::EmptyField("tenant_ref"));
        }
        if self.control_mode_name.trim().is_empty() {
            return Err(PaperLoopError::EmptyField("control_mode_name"));
        }
        if self.currency.trim().is_empty() {
            return Err(PaperLoopError::EmptyField("currency"));
        }
        if self.payment_state_ref.trim().is_empty() {
            return Err(PaperLoopError::EmptyField("payment_state_ref"));
        }
        if self.interval_bounds.is_empty()
            || self
                .interval_bounds
                .iter()
                .any(|(start, end)| *start < 0 || *end <= *start)
        {
            return Err(PaperLoopError::IntervalBoundsInvalid);
        }
        if self.statement_lines.is_empty() {
            return Err(PaperLoopError::EmptyField("statement_lines"));
        }
        if self.expected_cents_override.len() != self.statement_lines.len() {
            return Err(PaperLoopError::EmptyField("expected_cents_override"));
        }
        Ok(())
    }
}

/// The DISPATCH leg: a paper instruction RECORD of what WOULD be relayed
/// (see [`PAPER_DISPATCH_STATEMENT`] — never a command).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaperDispatchInstruction {
    /// The instruction record's reference (`sim://market/paper/dispatch/…`).
    pub instruction_ref: String,
    /// The composed bid record the instruction WOULD relay against.
    pub bid_record_ref: String,
    /// The market-structure name (the composer's own spelling).
    pub market_structure_name: String,
    /// The interval count of the bid window.
    pub interval_count: usize,
    /// The number of contributing assets (provenance).
    pub contributing_asset_count: usize,
}

/// The EVIDENCE leg: the deterministic synthetic delivered-energy record
/// (a fixture; every number is integer-derived decimal text — never a
/// float, never a claim).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaperEvidenceRecord {
    /// The evidence record's reference (`sim://market/paper/evidence/…`).
    pub evidence_ref: String,
    /// The bid record the evidence is synthetic delivery FOR.
    pub bid_record_ref: String,
    /// The per-interval delivered energy in MWh, as wire-faithful decimal
    /// text (integer tenths recipe).
    pub delivered_energy_mwh_text: Vec<String>,
    /// The canonical §10.4 quality state name carried WITH the numbers
    /// (the frozen vocabulary, consumed: `GOOD`).
    pub quality_canonical_name: String,
}

/// The PAYMENT-STATE leg: a REFERENCE to a payment-state record (see
/// [`PAPER_LEDGER_STATEMENT`] — never a state entry, never a payment).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaperLedgerEntry {
    /// The ledger entry's reference (`sim://market/paper/ledger/…`).
    pub ledger_entry_ref: String,
    /// The statement the ledger entry settles against (the simulated
    /// statement's own reference).
    pub statement_ref: String,
    /// The reconciliation report's headline counts (provenance — these
    /// are run counts, never settlement figures).
    pub matched_count: usize,
    pub mismatch_count: usize,
    /// The payment-state record REFERENCE (non-empty; the state
    /// vocabulary itself stays in the P16.14 module's single home).
    pub payment_state_ref: String,
}

/// The full paper-cycle record: one stage output per leg, in loop order.
/// Deterministic — the same config + same feed window + same clock ⇒
/// byte-identical values.
#[derive(Debug, Clone, PartialEq)]
pub struct PaperCycleRecord {
    pub cycle_ref: String,
    /// The SIGNAL leg: the exact-decimal price records assembled from the
    /// simulated publication (empty NEVER — an absent publication refuses
    /// the cycle).
    pub signal_records: Vec<MarketPriceRecord>,
    /// The BID leg: the composer's own composed record.
    pub bid: ComposedBidRecord,
    /// The DISPATCH leg: the paper instruction record.
    pub dispatch: PaperDispatchInstruction,
    /// The EVIDENCE leg: the synthetic delivery record.
    pub evidence: PaperEvidenceRecord,
    /// The STATEMENT leg: the generated SYNTHETIC statement text (the
    /// `sim://statement` identity rides inside it).
    pub statement_text: String,
    /// The STATEMENT leg: the parsed line items (the extractor's own
    /// output).
    pub statement_items: Vec<PjmBliLineItem>,
    /// The RECONCILIATION leg: the engine's own report.
    pub reconciliation: ReconciliationReport,
    /// The PAYMENT-STATE leg: the ledger reference record.
    pub ledger: PaperLedgerEntry,
    /// When the cycle ran (the INJECTED clock's instant — never a
    /// wall-clock read).
    pub completed_at_unix_ms: i64,
}

/// Runs one paper-trading cycle end to end. Deterministic; every stage
/// output is the consumed engine's own output except the three
/// simulator-local RECORD types (dispatch, evidence, ledger reference) —
/// none of which touches a command, a payment, or a real counterparty.
///
/// `window_start_millis` is the simulated publication window's start (the
/// injected-window rule; no wall clock anywhere).
pub fn run_paper_cycle(
    config: &PaperCycleConfig,
    feed: &mut SimulatedIsoPriceFeed,
    window_start_millis: i64,
    clock: &dyn Clock,
) -> Result<PaperCycleRecord, PaperLoopError> {
    config.validate()?;
    let completed_at = clock.now_event_time().unix_millis();
    if completed_at < 0 {
        return Err(PaperLoopError::EmptyField("clock_now_unix_ms"));
    }
    let completed_at = completed_at as i64;
    let statement_ref = format!("{SIMULATED_STATEMENT_IDENTITY_PREFIX}/m29/csv/{cycle}", cycle = config.cycle_ref);

    // --- SIGNAL: the simulated feed's publication → exact-decimal records.
    let batch = feed
        .publish_window(window_start_millis)
        .map_err(PaperLoopError::SimulatedFeed)?;
    let signal_records = flexcore_application::market_price_records::records_from_publication(
        &batch,
        PriceSourceClass::CommercialVendor,
    )
    .map_err(PaperLoopError::PriceRecord)?;
    if signal_records.is_empty() {
        return Err(PaperLoopError::NoSignalPublication);
    }

    // --- BID: the P16.06 composer (mode + grant + provenance enforced by
    //     the composer itself; this loop adds nothing).
    let bid = compose_bid(
        &config.structure,
        &config.grant,
        &config.control_mode_name,
        &config.tenant_ref,
        &config.interval_bounds,
        &config.asset_inputs,
        clock,
    )
    .map_err(PaperLoopError::BidComposition)?;
    let bid_record_ref = format!("{PAPER_LOOP_IDENTITY_PREFIX}/bid/{}", config.cycle_ref);
    let (market_structure_name, interval_count, contributing_asset_count) = match &bid {
        ComposedBidRecord::PjmAggregationBid {
            intervals,
            contributing_assets,
            ..
        } => (
            config.structure.name(),
            intervals.len(),
            contributing_assets.len(),
        ),
        ComposedBidRecord::CaisoPdrLoadShiftBid {
            intervals,
            contributing_assets,
            ..
        } => (
            config.structure.name(),
            intervals.len(),
            contributing_assets.len(),
        ),
        ComposedBidRecord::IsoNeDamAssetBid { per_asset, .. } => (
            config.structure.name(),
            per_asset.first().map(|e| e.intervals.len()).unwrap_or(0),
            per_asset.len(),
        ),
    };

    // --- DISPATCH: the paper instruction RECORD (never a command).
    let dispatch = PaperDispatchInstruction {
        instruction_ref: format!("{PAPER_LOOP_IDENTITY_PREFIX}/dispatch/{}", config.cycle_ref),
        bid_record_ref: bid_record_ref.clone(),
        market_structure_name: market_structure_name.to_string(),
        interval_count,
        contributing_asset_count,
    };

    // --- EVIDENCE: the deterministic synthetic delivery record (integer
    //     tenths recipe; the canonical §10.4 GOOD state travels WITH the
    //     numbers).
    let delivered_energy_mwh_text: Vec<String> = (0..interval_count)
        .map(|index| {
            let tenths = config.delivered_energy_tenths_base + index as i64;
            let sign = if tenths < 0 { "-" } else { "" };
            let magnitude = tenths.abs();
            format!("{sign}{}.{:01}", magnitude / 10, magnitude % 10)
        })
        .collect();
    let evidence = PaperEvidenceRecord {
        evidence_ref: format!("{PAPER_LOOP_IDENTITY_PREFIX}/evidence/{}", config.cycle_ref),
        bid_record_ref: bid_record_ref.clone(),
        delivered_energy_mwh_text,
        quality_canonical_name: TelemetryQuality::Good.name().to_string(),
    };

    // --- STATEMENT: the SYNTHETIC statement → the extractor's parse.
    let lines_for_fixture: Vec<(&str, i64)> = config
        .statement_lines
        .iter()
        .map(|(charge_type, cents)| (charge_type.as_str(), *cents))
        .collect();
    let statement_text =
        simulated_pjm_csv_statement(&statement_ref, &config.currency, &lines_for_fixture);
    let statement_items = extract_pjm_m29_bli_csv(&statement_text)
        .map_err(PaperLoopError::StatementParse)?;

    // --- RECONCILIATION: the P16.09 engine (the expected set is built
    //     from the same integer-cents recipe, with the caller's optional
    //     per-line override for the mismatch negative cases).
    let mut expected_lines = Vec::with_capacity(config.statement_lines.len());
    for (index, (_, cents)) in config.statement_lines.iter().enumerate() {
        let expected_cents = config.expected_cents_override[index].unwrap_or(*cents);
        let line_ref = format!("{statement_ref}#{index}");
        let amount = flexcore_application::fixed_decimal::MoneyAmount::parse(
            &render_cents(expected_cents),
            config.currency.clone(),
        )
        .map_err(|e| PaperLoopError::Reconciliation(ReconciliationError::DecimalInvalid {
            reason: e.to_string(),
        }))?;
        expected_lines.push(
            ExpectedLine::new(line_ref, amount).map_err(PaperLoopError::Reconciliation)?,
        );
    }
    let reconciliation = reconcile_bli_statement(
        &statement_ref,
        &expected_lines,
        &statement_items,
        Tolerance::Exact,
    )
    .map_err(PaperLoopError::Reconciliation)?;

    // --- PAYMENT-STATE reference: the ledger entry (a reference, never a
    //     state entry — see PAPER_LEDGER_STATEMENT).
    let ledger = PaperLedgerEntry {
        ledger_entry_ref: format!("{PAPER_LOOP_IDENTITY_PREFIX}/ledger/{}", config.cycle_ref),
        statement_ref: statement_ref.clone(),
        matched_count: reconciliation.matched_count,
        mismatch_count: reconciliation.mismatch_count,
        payment_state_ref: config.payment_state_ref.clone(),
    };

    Ok(PaperCycleRecord {
        cycle_ref: config.cycle_ref.clone(),
        signal_records,
        bid,
        dispatch,
        evidence,
        statement_text,
        statement_items,
        reconciliation,
        ledger,
        completed_at_unix_ms: completed_at,
    })
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is
// hand-derived from the declared recipes and the consumed engines' own
// rules. Nothing is fabricated as market data.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::price_feed::{SimulatedPriceFeedConfig, SIM_FEED_IS_SIMULATION_STATEMENT};
    use crate::market::settlement_fixtures::SIMULATED_STATEMENTS_ARE_SYNTHETIC_STATEMENT;
    use flexcore_application::bid_composition::BidInterval;
    use flexcore_contracts::time_model::EventTime;
    use flexcore_ports::clock::Clock;

    const TENANT: &str = "paper-tenant";
    const CURRENCY: &str = "EUR";

    /// A fixed-instant clock (the injected-clock rule; no wall clock).
    struct FixedClock(i64);

    impl Clock for FixedClock {
        fn now_event_time(&self) -> EventTime {
            EventTime::from_unix_millis(self.0).expect("fixture instant")
        }
        fn now_ingest_time(&self) -> flexcore_contracts::time_model::IngestTime {
            flexcore_contracts::time_model::IngestTime::from_unix_millis(self.0)
                .expect("fixture instant")
        }
    }

    fn grant(now: i64) -> MarketBidGrant {
        MarketBidGrant::new(
            "grant://paper/1".to_string(),
            TENANT.to_string(),
            "PJM_SINGLE_AGGREGATION_NODE".to_string(),
            "ACTIVE".to_string(),
            "paper-operator".to_string(),
            now,
            now + 3_600_000,
            "audit://paper/1".to_string(),
        )
        .expect("valid grant")
    }

    fn config(now: i64) -> PaperCycleConfig {
        let start = 1_700_000_000_000;
        let interval_bounds: Vec<(i64, i64)> =
            (0..4).map(|i| (start + i * 300_000, start + (i + 1) * 300_000)).collect();
        let asset_inputs = vec![AssetBidInput {
            asset_id: "paper-asset-1".to_string(),
            envelope_ref: "envelope://paper/1@v1".to_string(),
            interval_quantities_mw: vec![1.5, 1.5, 2.0, 2.0],
        }];
        PaperCycleConfig {
            cycle_ref: "cycle-1".to_string(),
            tenant_ref: TENANT.to_string(),
            control_mode_name: "operator-approved".to_string(),
            structure: MarketStructureDeclaration::PjmSingleAggregationNode {
                aggregation_node_ref: "sim-node-1".to_string(),
            },
            grant: grant(now),
            interval_bounds,
            asset_inputs,
            currency: CURRENCY.to_string(),
            statement_lines: vec![
                ("ENERGY_CHARGE".to_string(), 123456),
                ("CAPACITY_CHARGE".to_string(), 250),
            ],
            expected_cents_override: vec![None, None],
            delivered_energy_tenths_base: 15,
            payment_state_ref: "payment-state://paper/1".to_string(),
        }
    }

    fn feed() -> SimulatedIsoPriceFeed {
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

    #[test]
    fn statements_are_the_recorded_strings() {
        assert!(PAPER_LOOP_STATEMENT.contains("SIMULATION ONLY"));
        assert!(PAPER_LOOP_STATEMENT.contains("no market is connected"));
        assert!(PAPER_LOOP_STATEMENT.contains("never connections"));
        assert!(PAPER_DISPATCH_STATEMENT.contains("never a command"));
        assert!(PAPER_DISPATCH_STATEMENT.contains("14-state command lifecycle"));
        assert!(PAPER_LEDGER_STATEMENT.contains("P16.14 payment-lifecycle module"));
        assert!(PAPER_LEDGER_STATEMENT.contains("no payment moves"));
        assert!(SIM_FEED_IS_SIMULATION_STATEMENT.contains("IN-PROCESS"));
        assert!(SIMULATED_STATEMENTS_ARE_SYNTHETIC_STATEMENT.contains("SYNTHETIC"));
    }

    #[test]
    fn the_full_loop_runs_in_records_only() {
        let now = 1_700_000_000_000;
        let cfg = config(now);
        let mut simulated_feed = feed();
        let record = run_paper_cycle(&cfg, &mut simulated_feed, now, &FixedClock(now))
            .expect("the paper cycle completes");
        // SIGNAL: eight exact-decimal price records (the feed's recipe).
        assert_eq!(record.signal_records.len(), 8);
        assert_eq!(record.signal_records[0].price.canonical_string(), "425.0 EUR");
        assert!(record.signal_records[0].record_ref.starts_with("sim://market/"));
        // BID: the composer's own aggregation — ONE asset carrying
        // quantities [1.5, 1.5, 2.0, 2.0]; the PJM aggregation sums ACROSS
        // assets per interval, so the sums equal the single asset's
        // quantities exactly.
        match &record.bid {
            ComposedBidRecord::PjmAggregationBid { intervals, contributing_assets, envelope_refs } => {
                assert_eq!(intervals.len(), 4);
                let sums: Vec<f64> = intervals.iter().map(|b: &BidInterval| b.quantity_mw).collect();
                assert_eq!(sums, vec![1.5, 1.5, 2.0, 2.0]);
                assert_eq!(contributing_assets, &vec!["paper-asset-1".to_string()]);
                assert_eq!(envelope_refs, &vec!["envelope://paper/1@v1".to_string()]);
            }
            other => panic!("expected the PJM aggregation record, got {other:?}"),
        }
        // DISPATCH: a paper record, never a command.
        assert_eq!(record.dispatch.instruction_ref, "sim://market/paper/dispatch/cycle-1");
        assert_eq!(record.dispatch.interval_count, 4);
        assert_eq!(record.dispatch.contributing_asset_count, 1);
        // EVIDENCE: integer-tenths texts with the canonical GOOD state.
        assert_eq!(
            record.evidence.delivered_energy_mwh_text,
            vec!["1.5".to_string(), "1.6".to_string(), "1.7".to_string(), "1.8".to_string()]
        );
        assert_eq!(record.evidence.quality_canonical_name, "GOOD");
        // STATEMENT: the synthetic text parses back into the same lines.
        assert!(record.statement_text.starts_with("statement_ref,charge_type\n"));
        assert!(record.statement_text.contains("sim://statement/m29/csv/cycle-1"));
        assert_eq!(record.statement_items.len(), 2);
        assert_eq!(
            record.statement_items[0].amount.canonical_string(),
            "1234.56 EUR"
        );
        // RECONCILIATION: the engine's own report (exact match on the
        // positive path).
        assert_eq!(record.reconciliation.matched_count, 2);
        assert_eq!(record.reconciliation.mismatch_count, 0);
        assert!(record.reconciliation.dispute_queue.is_empty());
        // PAYMENT-STATE: a REFERENCE, never a state.
        assert_eq!(record.ledger.payment_state_ref, "payment-state://paper/1");
        assert_eq!(record.ledger.matched_count, 2);
        assert_eq!(
            record.ledger.ledger_entry_ref,
            "sim://market/paper/ledger/cycle-1"
        );
        assert_eq!(record.completed_at_unix_ms, now);
    }

    #[test]
    fn the_loop_is_deterministic_byte_for_byte() {
        let now = 1_700_000_000_000;
        let cfg = config(now);
        let mut a = feed();
        let mut b = feed();
        let first = run_paper_cycle(&cfg, &mut a, now, &FixedClock(now)).expect("cycle");
        let second = run_paper_cycle(&cfg, &mut b, now, &FixedClock(now)).expect("cycle");
        // The stage outputs are identical (the replay property): the
        // statement text byte-for-byte, the record vectors element-wise.
        assert_eq!(first.statement_text, second.statement_text);
        assert_eq!(first.signal_records, second.signal_records);
        assert_eq!(first.bid, second.bid);
        assert_eq!(first.dispatch, second.dispatch);
        assert_eq!(first.evidence, second.evidence);
        assert_eq!(first.statement_items, second.statement_items);
        assert_eq!(first.reconciliation, second.reconciliation);
        assert_eq!(first.ledger, second.ledger);
    }

    #[test]
    fn a_mismatched_line_becomes_a_queue_entry_never_a_correction() {
        let now = 1_700_000_000_000;
        let mut cfg = config(now);
        // The caller's override: the second line's EXPECTED value is
        // deliberately perturbed (the mismatch negative case).
        cfg.expected_cents_override = vec![None, Some(999)];
        let mut simulated_feed = feed();
        let record = run_paper_cycle(&cfg, &mut simulated_feed, now, &FixedClock(now))
            .expect("the paper cycle completes");
        assert_eq!(record.reconciliation.matched_count, 1);
        assert_eq!(record.reconciliation.mismatch_count, 1);
        assert_eq!(record.reconciliation.dispute_queue.len(), 1);
        let entry = &record.reconciliation.dispute_queue[0];
        assert_eq!(entry.reason_code, "LINE_AMOUNT_MISMATCH");
        // BOTH values verbatim, carried as exact decimals.
        assert_eq!(
            entry.stated.as_ref().expect("stated").canonical_string(),
            "2.50 EUR"
        );
        assert_eq!(
            entry.expected.as_ref().expect("expected").canonical_string(),
            "9.99 EUR"
        );
    }

    #[test]
    fn loop_refusals_are_named() {
        let now = 1_700_000_000_000;
        // Non-operator mode: the composer refuses (the grant rule is the
        // composer's own gate — the loop adds nothing).
        let mut cfg = config(now);
        cfg.control_mode_name = "simulation-shadow".to_string();
        let mut simulated_feed = feed();
        match run_paper_cycle(&cfg, &mut simulated_feed, now, &FixedClock(now)) {
            Err(PaperLoopError::BidComposition(
                flexcore_application::bid_composition::BidCompositionError::ControlModeInsufficient {
                    mode_name,
                },
            )) => assert_eq!(mode_name, "simulation-shadow"),
            other => panic!("expected the composer's mode refusal, got {other:?}"),
        }
        // An absent publication (negative window): no signal, no cycle.
        let cfg = config(now);
        let mut simulated_feed = feed();
        match run_paper_cycle(&cfg, &mut simulated_feed, -1, &FixedClock(now)) {
            Err(PaperLoopError::SimulatedFeed(e)) => {
                assert_eq!(e.name(), "SIM_FEED_WINDOW_UNREPRESENTABLE")
            }
            other => panic!("expected the feed refusal, got {other:?}"),
        }
        // An empty tenant: the loop's own named refusal.
        let mut cfg = config(now);
        cfg.tenant_ref = " ".to_string();
        let mut simulated_feed = feed();
        match run_paper_cycle(&cfg, &mut simulated_feed, now, &FixedClock(now)) {
            Err(PaperLoopError::EmptyField("tenant_ref")) => {}
            other => panic!("expected the tenant refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_revoked_grant_refuses_the_bid_leg() {
        let now = 1_700_000_000_000;
        let mut cfg = config(now);
        cfg.grant = MarketBidGrant::new(
            "grant://paper/2".to_string(),
            TENANT.to_string(),
            "PJM_SINGLE_AGGREGATION_NODE".to_string(),
            "REVOKED".to_string(),
            "paper-operator".to_string(),
            now,
            now + 3_600_000,
            "audit://paper/2".to_string(),
        )
        .expect("valid grant");
        let mut simulated_feed = feed();
        match run_paper_cycle(&cfg, &mut simulated_feed, now, &FixedClock(now)) {
            Err(PaperLoopError::BidComposition(
                flexcore_application::bid_composition::BidCompositionError::GrantNotActive {
                    grant_state,
                    ..
                },
            )) => assert_eq!(grant_state, "REVOKED"),
            other => panic!("expected the grant refusal, got {other:?}"),
        }
    }
}
