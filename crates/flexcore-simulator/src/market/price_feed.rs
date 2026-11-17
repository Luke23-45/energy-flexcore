// P16.20 — The simulated ISO price feed: the paper-trading counterpart of
//         the UNWIRED `IsoPricePublication` seam (P16.07, decision record
//         21). The seam's own record (`21_p16_07_price_signal_ingestion.md`)
//         declares the trait and leaves it UNWIRED — no feed is contacted
//         in this program. THIS module is the simulator's in-process
//         counterpart: it produces deterministic SYNTHETIC publication
//         batches so the signal leg of the paper loop (P16.20) is testable
//         with zero market connectivity.
//
// Rules this module encodes:
//
// - **Deterministic integer recipe, never a float, never an RNG** (the
//   module-tree rules; plan §4): every simulated price is computed in
//   INTEGER TENTHS of the declared currency per MWh as
//   `base_tenths + amplitude_tenths * step(i)`, where `step(i)` is a
//   fixed integer triangle wave over an 8-interval period with values
//   `{0, 1, 2, 3, 2, 1, 0, -1}`. The tenths are rendered as wire-faithful
//   DECIMAL TEXT (scale 1) — the exact-decimal parse happens only at the
//   P16.07 record boundary (`fixed_decimal`), never here as a float.
//   Negative prices (base − 1×amplitude) are CARRIED AS PUBLISHED (the
//   P15.04/P16.07 posture), never clamped.
// - **The seam is exercised, not wired** (`SIM_FEED_IS_SIMULATION_
//   STATEMENT`): implementing the trait in-process is not connectivity —
//   no transport exists, no byte leaves the process, and the feed's
//   identity strings carry the `sim://market` prefix so every simulated
//   publication is identifiable from its identity alone, forever.
// - **Source class is declared, never disguised**: the simulated feed's
//   records carry `PriceSourceClass::CommercialVendor` — the closed
//   vocabulary's nearest class — with the `sim://market` record identity
//   carrying the simulation marking. The class vocabulary is NOT widened
//   for the simulator (the frozen P15.04 vocabulary is consumed, never
//   redefined).

use flexcore_application::market_price_records::{
    IsoPricePublication, MarketSegment, PricePublicationBatch, PricePublicationEntry,
    PriceRecordError,
};
use flexcore_contracts::time_model::EventTime;

/// The simulated-feed identity prefix (every publication this module
/// produces is identifiable from its identity alone, forever — the P14.17
/// identity rule carried to the market tier).
pub const SIMULATED_FEED_IDENTITY_PREFIX: &str = "sim://market";

/// The simulation statement (pinned by tests).
pub const SIM_FEED_IS_SIMULATION_STATEMENT: &str = "The simulated price \
     feed is an IN-PROCESS implementation of the UNWIRED IsoPricePublication \
     seam: no transport exists, no byte leaves the process, no feed is \
     contacted, and no publication is delivered to anyone. Every identity \
     string carries the 'sim://market' prefix; every price is a \
     deterministic synthetic fixture value with NO market meaning; the \
     simulator produces records only, never a connection.";

/// The price-recipe statement (pinned by tests).
pub const SIM_PRICE_RECIPE_STATEMENT: &str = "Every simulated price is \
     computed in INTEGER TENTHS as base_tenths + amplitude_tenths * step(i) \
     over a fixed 8-interval integer triangle wave {0,1,2,3,2,1,0,-1}, then \
     rendered as wire-faithful decimal text (scale 1). There is no float \
     arithmetic, no RNG, and no wall-clock read: the same config + same \
     window start ⇒ byte-identical publication text.";

/// Why a simulated-feed operation was refused. Named, never absorbed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimulatedFeedError {
    /// A required text field was empty or whitespace.
    EmptyField(&'static str),
    /// The declared interval length was not positive.
    IntervalNotPositive,
    /// The declared interval count was zero (an empty publication is a
    /// refusal at the generator, never an empty batch).
    EmptyIntervalCount,
    /// The amplitude was negative (a wave with a negative step size is an
    /// inconsistent declaration; negate the base instead).
    NegativeAmplitude,
    /// The window start was negative (outside the record timestamp
    /// domain) or the window does not fit the representable domain.
    WindowUnrepresentable,
    /// A generated price could not be rendered/parsed at the record
    /// boundary (would indicate a recipe bug — never silently absorbed).
    PriceTextInvalid { reason: String },
}

impl SimulatedFeedError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            SimulatedFeedError::EmptyField(_) => "SIM_FEED_EMPTY_FIELD",
            SimulatedFeedError::IntervalNotPositive => "SIM_FEED_INTERVAL_NOT_POSITIVE",
            SimulatedFeedError::EmptyIntervalCount => "SIM_FEED_EMPTY_INTERVAL_COUNT",
            SimulatedFeedError::NegativeAmplitude => "SIM_FEED_NEGATIVE_AMPLITUDE",
            SimulatedFeedError::WindowUnrepresentable => "SIM_FEED_WINDOW_UNREPRESENTABLE",
            SimulatedFeedError::PriceTextInvalid { .. } => "SIM_FEED_PRICE_TEXT_INVALID",
        }
    }
}

impl std::fmt::Display for SimulatedFeedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimulatedFeedError::EmptyField(field) => {
                write!(f, "the simulated-feed field '{field}' must be non-empty")
            }
            SimulatedFeedError::IntervalNotPositive => {
                write!(f, "the declared interval length must be positive")
            }
            SimulatedFeedError::EmptyIntervalCount => {
                write!(f, "the declared interval count must be at least 1")
            }
            SimulatedFeedError::NegativeAmplitude => {
                write!(
                    f,
                    "the declared amplitude must be non-negative (negate the base instead)"
                )
            }
            SimulatedFeedError::WindowUnrepresentable => write!(
                f,
                "the simulated window is outside the representable timestamp domain"
            ),
            SimulatedFeedError::PriceTextInvalid { reason } => write!(
                f,
                "the generated price text failed the record-boundary check: {reason} \
                 (a recipe bug is a named refusal, never a silently absorbed value)"
            ),
        }
    }
}

impl std::error::Error for SimulatedFeedError {}

/// The fixed 8-interval integer triangle wave (the recipe's step values,
/// in order — deterministic, no RNG).
const TRIANGLE_WAVE: [i64; 8] = [0, 1, 2, 3, 2, 1, 0, -1];

// The integer-tenths renderer has ONE home — the statement-recipe module
// `settlement_fixtures` (FRV1-R7-008: byte-identical duplicates can drift
// independently). Re-exported so the `price_feed::render_tenths` path
// keeps resolving to that single definition.
pub use super::settlement_fixtures::render_tenths;

/// The validated configuration of one simulated price feed: one segment,
/// one area, one currency, one declared cadence. All synthetic; nothing
/// here references any real market node, zone, or publication schedule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulatedPriceFeedConfig {
    /// The feed reference (built into every publication's document ref;
    /// kept distinct from [`SIMULATED_FEED_IDENTITY_PREFIX`] which the
    /// generator prepends itself).
    pub feed_ref: String,
    /// The market segment the feed publishes (one segment per feed — a
    /// feed that served two segments would blur the publication trail).
    pub segment: MarketSegment,
    /// The delivery area as published (verbatim; synthetic).
    pub area: String,
    /// The declared currency code (currency as DATA; the code set is the
    /// source's — here, the fixture author's).
    pub currency_code: String,
    /// The base price in integer tenths of the currency per MWh.
    pub base_price_tenths: i64,
    /// The wave amplitude in integer tenths (non-negative; negate the
    /// base instead of the amplitude).
    pub amplitude_tenths: i64,
    /// The interval length in milliseconds (positive; e.g. 1_800_000 for
    /// a half-hourly synthetic day-ahead shape, 300_000 for a 5-minute
    /// synthetic real-time shape — the shape is the FIXTURE AUTHOR's
    /// declaration, never a claim about any ISO's actual cadence).
    pub interval_millis: i64,
    /// The number of intervals per publication (≥ 1).
    pub interval_count: usize,
}

impl SimulatedPriceFeedConfig {
    /// Validates and constructs the configuration (fail-closed).
    pub fn new(
        feed_ref: String,
        segment: MarketSegment,
        area: String,
        currency_code: String,
        base_price_tenths: i64,
        amplitude_tenths: i64,
        interval_millis: i64,
        interval_count: usize,
    ) -> Result<Self, SimulatedFeedError> {
        if feed_ref.trim().is_empty() {
            return Err(SimulatedFeedError::EmptyField("feed_ref"));
        }
        if area.trim().is_empty() {
            return Err(SimulatedFeedError::EmptyField("area"));
        }
        if currency_code.trim().is_empty() {
            return Err(SimulatedFeedError::EmptyField("currency_code"));
        }
        if interval_millis <= 0 {
            return Err(SimulatedFeedError::IntervalNotPositive);
        }
        if interval_count == 0 {
            return Err(SimulatedFeedError::EmptyIntervalCount);
        }
        if amplitude_tenths < 0 {
            return Err(SimulatedFeedError::NegativeAmplitude);
        }
        Ok(Self {
            feed_ref,
            segment,
            area,
            currency_code,
            base_price_tenths,
            amplitude_tenths,
            interval_millis,
            interval_count,
        })
    }

    /// The simulated price for interval index `i`, in integer tenths:
    /// `base + amplitude * step(i)` (the recipe; pure integer arithmetic).
    pub fn price_tenths_at(&self, index: usize) -> i64 {
        self.base_price_tenths
            + self.amplitude_tenths * TRIANGLE_WAVE[index % TRIANGLE_WAVE.len()]
    }
}

/// The simulated ISO price feed: an in-process implementation of the
/// UNWIRED [`IsoPricePublication`] seam (the seam is exercised, never
/// wired — see [`SIM_FEED_IS_SIMULATION_STATEMENT`]). The feed holds ONE
/// latest publication (the last generated window); `latest_publication`
/// answers it for the configured segment and `Ok(None)` for any other
/// segment (an absent publication is an empty result — never a fabricated
/// price and never a stale replay, per the trait contract).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimulatedIsoPriceFeed {
    config: SimulatedPriceFeedConfig,
    latest: Option<PricePublicationBatch>,
}

impl SimulatedIsoPriceFeed {
    /// Constructs the feed with NO publication loaded (an absent
    /// publication is the honest initial state).
    pub fn new(config: SimulatedPriceFeedConfig) -> Self {
        Self { config, latest: None }
    }

    /// The feed's configuration.
    pub fn config(&self) -> &SimulatedPriceFeedConfig {
        &self.config
    }

    /// Generates one publication window starting at `window_start_millis`
    /// and installs it as the feed's latest publication. Deterministic:
    /// the same config + window start ⇒ byte-identical text (no RNG, no
    /// wall clock — the injected-window rule, the P2.13 discipline
    /// carried to the market tier).
    pub fn publish_window(
        &mut self,
        window_start_millis: i64,
    ) -> Result<PricePublicationBatch, SimulatedFeedError> {
        if window_start_millis < 0 {
            return Err(SimulatedFeedError::WindowUnrepresentable);
        }
        let document_ref = format!(
            "{SIMULATED_FEED_IDENTITY_PREFIX}/{}/{window_start_millis}",
            self.config.feed_ref
        );
        let mut entries = Vec::with_capacity(self.config.interval_count);
        for index in 0..self.config.interval_count {
            let start = window_start_millis
                + (index as i64) * self.config.interval_millis;
            let end = start + self.config.interval_millis;
            let interval_start = EventTime::from_unix_millis(start)
                .map_err(|_| SimulatedFeedError::WindowUnrepresentable)?;
            let interval_end = EventTime::from_unix_millis(end)
                .map_err(|_| SimulatedFeedError::WindowUnrepresentable)?;
            // Overflow-checked start (the window must fit the domain).
            if end < start {
                return Err(SimulatedFeedError::WindowUnrepresentable);
            }
            entries.push(PricePublicationEntry {
                interval_start,
                interval_end,
                price_text: render_tenths(self.config.price_tenths_at(index)),
                currency_code: self.config.currency_code.clone(),
                area: self.config.area.clone(),
            });
        }
        let batch = PricePublicationBatch {
            document_ref,
            segment: self.config.segment,
            entries,
        };
        self.latest = Some(batch.clone());
        Ok(batch)
    }
}

impl IsoPricePublication for SimulatedIsoPriceFeed {
    fn latest_publication(
        &self,
        segment: MarketSegment,
    ) -> Result<Option<PricePublicationBatch>, PriceRecordError> {
        // The trait contract: an absent (or segment-mismatched)
        // publication is an empty result — never a fabricated price and
        // never a stale replay.
        if segment != self.config.segment {
            return Ok(None);
        }
        Ok(self.latest.clone())
    }
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is
// hand-derived from the declared integer recipe. Nothing is fabricated as
// market data.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn config(segment: MarketSegment) -> SimulatedPriceFeedConfig {
        SimulatedPriceFeedConfig::new(
            "day-ahead-lab".to_string(),
            segment,
            "SIM-ZONE".to_string(),
            "EUR".to_string(),
            4250, // base 425.0 EUR/MWh in tenths
            150,  // amplitude 15.0 EUR/MWh in tenths
            1_800_000,
            8,
        )
        .expect("valid config")
    }

    #[test]
    fn statements_are_the_recorded_strings() {
        assert!(SIM_FEED_IS_SIMULATION_STATEMENT.contains("IN-PROCESS"));
        assert!(SIM_FEED_IS_SIMULATION_STATEMENT.contains("sim://market"));
        assert!(SIM_FEED_IS_SIMULATION_STATEMENT.contains("never a connection"));
        assert!(SIM_PRICE_RECIPE_STATEMENT.contains("INTEGER TENTHS"));
        assert!(SIM_PRICE_RECIPE_STATEMENT.contains("byte-identical"));
    }

    #[test]
    fn render_tenths_is_exact_decimal_text() {
        assert_eq!(render_tenths(4250), "425.0");
        assert_eq!(render_tenths(4255), "425.5");
        assert_eq!(render_tenths(-125), "-12.5");
        assert_eq!(render_tenths(0), "0.0");
        assert_eq!(render_tenths(-5), "-0.5");
        assert_eq!(render_tenths(7), "0.7");
    }

    #[test]
    fn the_recipe_is_the_declared_integer_triangle_wave() {
        let cfg = config(MarketSegment::DayAhead);
        // Wave {0,1,2,3,2,1,0,-1} × amplitude 150 over base 4250:
        let expected = [4250, 4400, 4550, 4700, 4550, 4400, 4250, 4100];
        for (index, value) in expected.iter().enumerate() {
            assert_eq!(cfg.price_tenths_at(index), *value);
        }
        // The wave repeats: index 8 wraps to step 0.
        assert_eq!(cfg.price_tenths_at(8), 4250);
    }

    #[test]
    fn a_published_window_is_wire_faithful_and_deterministic() {
        let mut feed = SimulatedIsoPriceFeed::new(config(MarketSegment::DayAhead));
        let batch = feed.publish_window(1_700_000_000_000).expect("window");
        assert_eq!(batch.entries.len(), 8);
        assert_eq!(batch.entries[0].price_text, "425.0");
        assert_eq!(batch.entries[3].price_text, "470.0");
        assert_eq!(batch.entries[7].price_text, "410.0");
        assert_eq!(batch.entries[0].currency_code, "EUR");
        assert_eq!(batch.entries[2].area, "SIM-ZONE");
        assert!(batch.document_ref.starts_with("sim://market/"));
        // Determinism: the same window start ⇒ byte-identical text.
        let again = feed.publish_window(1_700_000_000_000).expect("window");
        assert_eq!(again, batch);
        // The interval bounds are exact and contiguous.
        assert_eq!(
            batch.entries[0].interval_end.unix_millis(),
            batch.entries[0].interval_start.unix_millis() + 1_800_000
        );
        assert_eq!(
            batch.entries[1].interval_start.unix_millis(),
            batch.entries[0].interval_end.unix_millis()
        );
    }

    #[test]
    fn the_seam_answers_by_segment_and_absence_is_not_fabrication() {
        let mut feed = SimulatedIsoPriceFeed::new(config(MarketSegment::DayAhead));
        // No publication loaded: absent, never fabricated.
        assert_eq!(
            feed.latest_publication(MarketSegment::DayAhead).expect("absent"),
            None
        );
        feed.publish_window(1_700_000_000_000).expect("window");
        // The configured segment answers.
        let batch = feed
            .latest_publication(MarketSegment::DayAhead)
            .expect("present")
            .expect("some batch");
        assert_eq!(batch.entries.len(), 8);
        // A different segment is ABSENT (never a fabricated or replayed
        // publication).
        assert_eq!(
            feed.latest_publication(MarketSegment::RealTime).expect("absent"),
            None
        );
    }

    #[test]
    fn feed_refusals_are_named() {
        let err = SimulatedPriceFeedConfig::new(
            " ".to_string(),
            MarketSegment::DayAhead,
            "Z".to_string(),
            "EUR".to_string(),
            0,
            0,
            1_800_000,
            8,
        )
        .expect_err("empty feed_ref refused");
        assert_eq!(err.name(), "SIM_FEED_EMPTY_FIELD");
        let err = SimulatedPriceFeedConfig::new(
            "f".to_string(),
            MarketSegment::DayAhead,
            "Z".to_string(),
            "EUR".to_string(),
            0,
            -1,
            1_800_000,
            8,
        )
        .expect_err("negative amplitude refused");
        assert_eq!(err.name(), "SIM_FEED_NEGATIVE_AMPLITUDE");
        let err = SimulatedPriceFeedConfig::new(
            "f".to_string(),
            MarketSegment::RealTime,
            "Z".to_string(),
            "EUR".to_string(),
            0,
            0,
            0,
            8,
        )
        .expect_err("zero interval refused");
        assert_eq!(err.name(), "SIM_FEED_INTERVAL_NOT_POSITIVE");
        let mut feed = SimulatedIsoPriceFeed::new(config(MarketSegment::DayAhead));
        let err = feed
            .publish_window(-1)
            .expect_err("negative window refused");
        assert_eq!(err.name(), "SIM_FEED_WINDOW_UNREPRESENTABLE");
    }

    #[test]
    fn records_assemble_from_the_simulated_batch_at_the_boundary() {
        // The exact-decimal parse happens only at the P16.07 record
        // boundary; the float copy never exists anywhere in this path.
        use flexcore_application::market_price_records::records_from_publication;
        use flexcore_ports::price_signal::PriceSourceClass;
        let mut feed = SimulatedIsoPriceFeed::new(config(MarketSegment::DayAhead));
        let batch = feed.publish_window(1_700_000_000_000).expect("window");
        let records = records_from_publication(&batch, PriceSourceClass::CommercialVendor)
            .expect("valid records");
        assert_eq!(records.len(), 8);
        assert_eq!(records[0].price.canonical_string(), "425.0 EUR");
        assert!(records[0].record_ref.starts_with("sim://market/"));
    }
}
