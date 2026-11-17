// P16.07 — Price-signal ingestion: day-ahead/real-time market price
//         RECORDS over the P15.04 seam (consumed, never redefined).
//
// Phase 16 plan §2 row P16.07; sources: [S-111] (ENTSO-E Transparency
// Platform + API guide — the P15.04 adapter is the concrete source),
// [S-94] (PJM DataMiner2 reg_market_results — the ISO price-publication
// seam CLASS this module models), [S-112] (the vendor price-forecast
// seam, P15-E-2, registered OPEN).
//
// THE SEAM RULE, DECIDED ONCE:
//
// - **P15.04 is consumed, never redefined.** The price-ingestion PORT is
//   `flexcore_ports::price_signal::PriceSignalSource` (P15.04); the
//   concrete ENTSO-E Transparency Platform adapter is
//   `crates/adapters/protocol-entsoe` (the A44 day-ahead price document,
//   [S-111]). This module adds NO second ingestion path, NO second price
//   type, and NO wire-schema knowledge: it CONSUMES ingested
//   `PriceSeries` records and classifies/links them into the Phase 16
//   market price record layer.
// - **Two copies, never silently merged** (the P16.10 dual-stream
//   discipline applied to prices): the P15.04 analytics copy carries
//   `f64` price points (Phase 15's recorded seam decision, an ANALYTICS
//   record); the Phase 16 settlement-adjacent price RECORD carries an
//   exact decimal ([`crate::fixed_decimal::FixedDecimal`]) populated at
//   the adapter boundary from wire-faithful text. No code converts the
//   f64 copy into the decimal record: a float-derived money value is
//   unconstructable (the phase-16 plan §4 non-goals; the hard ban). A
//   settlement-adjacent record exists only where a wire decimal exists.
// - **Prices are inputs, never commands** (ADR-010; conventions §6.3).
//   There is no bid, offer, order, dispatch, or device-write vocabulary
//   anywhere in this module's surface — structural, not conventional.
//   A price record feeds operator views, evidence, and reconciliation
//   (P16.09); it never authorizes, produces, or influences a command.
// - **Market segments are closed and jurisdiction-recorded** (ADR-011):
//   the segment vocabulary here is the two-class structural minimum
//   (day-ahead / real-time). Jurisdiction detail (the actual publication
//   cadence, zonal vs nodal shape) lives in per-jurisdiction record
//   packs, never in a claimed-universal core.
// - **The ISO price-publication seam is a named SEAM, not a connection**
//   ([S-94] names the class: an ISO public data feed for regulation
//   market results is the exemplar). The [`IsoPricePublication`] trait
//   is declared here and UNWIRED: no feed is contacted, no HTTP client
//   exists in this crate, and every wire detail is UNVERIFIED until a
//   human-gated adapter lands.
//
// Control status: READ-ONLY record layer over ingested price data for
// the simulation-only 0.x line. No price feed is connected; no market
// action is possible from this module; no price is a command.

use flexcore_contracts::time_model::EventTime;
use flexcore_ports::price_signal::{PriceSeries, PriceSeriesKind, PriceSourceClass};

use crate::fixed_decimal::MoneyAmount;

/// The market price record schema version (a change is a NEW version,
/// never a silent edit).
pub const MARKET_PRICE_RECORD_SCHEMA_VERSION: u32 = 1;

/// The two-copies discipline statement (pinned by tests).
pub const TWO_COPIES_STATEMENT: &str = "The P15.04 price seam (PriceSignalSource and its \
     f64 PricePoint series) is the ANALYTICS copy and is consumed as-is, never redefined. \
     The Phase 16 settlement-adjacent price record is a separate DECIMAL copy populated \
     only at the adapter boundary from wire-faithful decimal text. No code converts the \
     f64 copy into the decimal record: a float-derived money value is unconstructable \
     (phase-16 plan section 4 non-goals). The two copies are never silently merged.";

/// The ADR-010 statement (pinned by tests).
pub const PRICE_IS_NEVER_A_COMMAND_STATEMENT: &str = "A market price record is INGESTED \
     DATA. It never authorizes, produces, or influences a dispatch command, a bid, an \
     offer, or an order (ADR-010; program conventions 6.3). There is no bid, offer, \
     order, or dispatch vocabulary anywhere in this module's surface.";

/// The ISO-publication-seam statement (pinned by tests).
pub const ISO_SEAM_UNWIRED_STATEMENT: &str = "The ISO price-publication seam is a named \
     SEAM ([S-94] names the class — an ISO public data feed for market results), not a \
     connection: the IsoPricePublication trait is declared and UNWIRED, no feed is \
     contacted in this program, and every wire detail is UNVERIFIED until a human-gated \
     adapter lands.";

/// The closed market-segment vocabulary (the structural minimum; per-
/// jurisdiction detail lives in record packs — ADR-011).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarketSegment {
    /// Day-ahead market prices (the cleared-auction delivery-day price).
    DayAhead,
    /// Real-time / intra-day market prices (the running-market price).
    RealTime,
}

impl MarketSegment {
    /// The canonical label spelling (records and audit details).
    pub const fn as_str(self) -> &'static str {
        match self {
            MarketSegment::DayAhead => "day-ahead",
            MarketSegment::RealTime => "real-time",
        }
    }
}

/// The closed ENTSO-E document-class vocabulary as far as it is VERIFIED
/// for classification ([S-111] via the P15.04 record). Anything outside
/// the verified class refuses classification rather than guessing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransparencyDocumentClass {
    /// The A44 day-ahead price document ([S-111]; the P15.04 adapter's
    /// document type). VERIFIED per the P15.04 record; day-ahead is the
    /// only segment this class supports.
    A44DayAhead,
    /// Any other document class: UNVERIFIED for classification here — the
    /// real-time/intra-day document code set is NOT claimed (marked
    /// UNVERIFIED; re-verify against the [S-111] API guide at use).
    OtherUnverified,
}

impl TransparencyDocumentClass {
    /// The canonical label spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            TransparencyDocumentClass::A44DayAhead => "a44-day-ahead",
            TransparencyDocumentClass::OtherUnverified => "other-unverified",
        }
    }
}

/// Why a price-record operation was refused. Named, never absorbed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PriceRecordError {
    /// The record reference was empty or whitespace.
    EmptyReference { field: &'static str },
    /// The interval is inverted or empty (start must be before end).
    InvertedInterval,
    /// The decimal text failed to parse (the fixed-decimal error rides).
    DecimalInvalid { reason: String },
    /// A transparency series could not be classified into a segment
    /// (unverified document class) — a named refusal, never a guess.
    SegmentNotClassifiable { document_class: &'static str },
    /// The series' currency was empty (a currency-less price is
    /// unconstructable at every layer).
    EmptyCurrency,
}

impl PriceRecordError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            PriceRecordError::EmptyReference { .. } => "PRICE_RECORD_EMPTY_REFERENCE",
            PriceRecordError::InvertedInterval => "PRICE_RECORD_INVERTED_INTERVAL",
            PriceRecordError::DecimalInvalid { .. } => "PRICE_RECORD_DECIMAL_INVALID",
            PriceRecordError::SegmentNotClassifiable { .. } => {
                "PRICE_RECORD_SEGMENT_NOT_CLASSIFIABLE"
            }
            PriceRecordError::EmptyCurrency => "PRICE_RECORD_EMPTY_CURRENCY",
        }
    }
}

impl std::fmt::Display for PriceRecordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PriceRecordError::EmptyReference { field } => {
                write!(f, "the '{field}' reference must be a non-empty string")
            }
            PriceRecordError::InvertedInterval => write!(
                f,
                "the delivery interval is inverted or empty (start must be before end)"
            ),
            PriceRecordError::DecimalInvalid { reason } => {
                write!(f, "the wire decimal failed to parse: {reason}")
            }
            PriceRecordError::SegmentNotClassifiable { document_class } => write!(
                f,
                "the transparency document class '{document_class}' is UNVERIFIED for \
                 segment classification; the record is refused, never guessed"
            ),
            PriceRecordError::EmptyCurrency => write!(
                f,
                "the series carries no currency; a currency-less price is unconstructable"
            ),
        }
    }
}

impl std::error::Error for PriceRecordError {}

/// One settlement-adjacent market price RECORD: the exact decimal price
/// for one delivery interval, in the declared currency per MWh, with the
/// market segment and full provenance. Constructed ONLY from wire-faithful
/// decimal text (never from a float — see [`TWO_COPIES_STATEMENT`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketPriceRecord {
    /// The record reference (the adapter's publication identity; never
    /// credentials — the P7.05 secret-reference rule).
    pub record_ref: String,
    /// The market segment (closed vocabulary).
    pub segment: MarketSegment,
    /// Start of the delivery interval (UTC, start-inclusive).
    pub interval_start: EventTime,
    /// End of the delivery interval (UTC, end-exclusive — the P3.03
    /// interval semantics).
    pub interval_end: EventTime,
    /// The price for the interval, in `currency_code` per MWh. Sign is
    /// NOT constrained (negative prices are carried as published — the
    /// P15.04 posture mirrored).
    pub price: MoneyAmount,
    /// The delivery area / bidding-zone identifier as published (carried
    /// verbatim; the zone vocabulary is the source's).
    pub area: String,
    /// The source class (closed P15.04 vocabulary, carried over).
    pub source_class: PriceSourceClass,
    /// The source document reference (a reference, never the payload).
    pub document_ref: String,
}

impl MarketPriceRecord {
    /// Validates and constructs one record (fail-closed). The price is
    /// parsed from wire-faithful decimal text; there is no float path.
    pub fn new(
        record_ref: String,
        segment: MarketSegment,
        interval_start: EventTime,
        interval_end: EventTime,
        price_text: &str,
        currency_code: String,
        area: String,
        source_class: PriceSourceClass,
        document_ref: String,
    ) -> Result<Self, PriceRecordError> {
        if record_ref.trim().is_empty() {
            return Err(PriceRecordError::EmptyReference {
                field: "record_ref",
            });
        }
        if document_ref.trim().is_empty() {
            return Err(PriceRecordError::EmptyReference {
                field: "document_ref",
            });
        }
        if interval_start.unix_millis() >= interval_end.unix_millis() {
            return Err(PriceRecordError::InvertedInterval);
        }
        if currency_code.trim().is_empty() {
            return Err(PriceRecordError::EmptyCurrency);
        }
        let price = MoneyAmount::parse(price_text, currency_code).map_err(|e| {
            PriceRecordError::DecimalInvalid {
                reason: e.to_string(),
            }
        })?;
        if area.trim().is_empty() {
            return Err(PriceRecordError::EmptyReference { field: "area" });
        }
        Ok(Self {
            record_ref,
            segment,
            interval_start,
            interval_end,
            price,
            area,
            source_class,
            document_ref,
        })
    }
}

/// Links an ingested P15.04 [`PriceSeries`] to a market segment, WITHOUT
/// carrying or converting its prices. The series stays an analytics
/// record in the P15.04 store; the link is the only cross-reference this
/// layer builds. Classification rules:
///
/// - `VendorForecast` series are links to NO market segment (a forecast
///   is not a cleared price — the P15.04 kind discipline, mirrored).
/// - `TransparencyPlatform` + `A44DayAhead` → [`MarketSegment::DayAhead`]
///   ([S-111], the verified A44 class).
/// - `OtherUnverified` document classes REFUSE (`SegmentNotClassifiable`)
///   — the real-time document code set is UNVERIFIED and is never
///   guessed.
///
/// Returns the classified segment and the series' own document reference
/// (carried verbatim for the record linkage).
pub fn classify_series_segment(
    series: &PriceSeries,
    document_class: TransparencyDocumentClass,
) -> Result<(Option<MarketSegment>, String), PriceRecordError> {
    if series.currency_code.trim().is_empty() {
        return Err(PriceRecordError::EmptyCurrency);
    }
    if series.kind == PriceSeriesKind::VendorForecast {
        // A vendor price forecast is not a cleared price: it links to no
        // segment. This is the recorded posture, not a refusal.
        return Ok((None, series.document_ref.clone()));
    }
    match (series.source_class, document_class) {
        (PriceSourceClass::TransparencyPlatform, TransparencyDocumentClass::A44DayAhead) => {
            Ok((Some(MarketSegment::DayAhead), series.document_ref.clone()))
        }
        (_, TransparencyDocumentClass::OtherUnverified) => {
            Err(PriceRecordError::SegmentNotClassifiable {
                document_class: TransparencyDocumentClass::OtherUnverified.as_str(),
            })
        }
        (PriceSourceClass::CommercialVendor, TransparencyDocumentClass::A44DayAhead) => {
            Err(PriceRecordError::SegmentNotClassifiable {
                document_class: TransparencyDocumentClass::A44DayAhead.as_str(),
            })
        }
    }
}

/// The ISO price-publication SEAM (declared, UNWIRED — see
/// [`ISO_SEAM_UNWIRED_STATEMENT`]). The class exemplar is the ISO public
/// data feed for regulation-market results ([S-94] names the PJM
/// DataMiner2 feed class); the concrete jurisdiction adapter implements
/// this trait and is selected only after a jurisdiction decision (D1,
/// OPEN; ADR-011). The seam carries WIRE-FAITHFUL decimal text — never a
/// float — so the settlement-adjacent record layer can be populated
/// without a float conversion path.
///
/// No implementation ships in this crate; no feed is contacted in this
/// program (conventions §1 loop bans; the first live call is a human/lab
/// gate). Sync trait (the crate rule): no futures, no async runtime.
pub trait IsoPricePublication {
    /// The latest publication batch for one market segment, tenant-scoped
    /// by the caller. A publication carries wire-faithful decimal text
    /// plus its document reference; an absent publication is an empty
    /// result — never a fabricated price and never a stale replay.
    fn latest_publication(
        &self,
        segment: MarketSegment,
    ) -> Result<Option<PricePublicationBatch>, PriceRecordError>;
}

/// One unwired-seam publication batch (the wire-faithful shape).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PricePublicationBatch {
    /// The publishing document reference (verbatim from the feed).
    pub document_ref: String,
    /// The market segment the feed declared for this batch.
    pub segment: MarketSegment,
    /// The per-interval prices as published: wire-faithful DECIMAL TEXT
    /// (parsed only by [`MarketPriceRecord::new`] at the record boundary).
    pub entries: Vec<PricePublicationEntry>,
}

/// One publication entry (wire-faithful text, never a float).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PricePublicationEntry {
    /// Start of the delivery interval (UTC, start-inclusive).
    pub interval_start: EventTime,
    /// End of the delivery interval (UTC, end-exclusive).
    pub interval_end: EventTime,
    /// The price as published (decimal text, verbatim).
    pub price_text: String,
    /// The currency code as published (verbatim).
    pub currency_code: String,
    /// The delivery area as published (verbatim).
    pub area: String,
}

/// Assembles [`MarketPriceRecord`]s from an unwired-seam publication
/// batch. Every entry is validated independently; a failed entry refuses
/// the WHOLE batch (all-or-nothing — a partially accepted price batch
/// would be a fabricated record set). The source class is fixed to the
/// publication seam's class.
pub fn records_from_publication(
    batch: &PricePublicationBatch,
    source_class: PriceSourceClass,
) -> Result<Vec<MarketPriceRecord>, PriceRecordError> {
    if batch.document_ref.trim().is_empty() {
        return Err(PriceRecordError::EmptyReference {
            field: "document_ref",
        });
    }
    let mut records = Vec::with_capacity(batch.entries.len());
    for entry in &batch.entries {
        records.push(MarketPriceRecord::new(
            batch.document_ref.clone(),
            batch.segment,
            entry.interval_start,
            entry.interval_end,
            &entry.price_text,
            entry.currency_code.clone(),
            entry.area.clone(),
            source_class,
            batch.document_ref.clone(),
        )?);
    }
    Ok(records)
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is a
// hand-derived property of the validation rules or a literal copied from
// this module's constants. Nothing is fabricated.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_ports::price_signal::PricePoint;

    fn instant(millis: i64) -> EventTime {
        EventTime::from_unix_millis(millis).expect("fixture instant is representable")
    }

    fn fixture_series(kind: PriceSeriesKind) -> PriceSeries {
        let point = PricePoint::new(instant(0), instant(3_600_000), 42.5)
            .expect("fixture point is valid");
        PriceSeries::new(
            kind,
            PriceSourceClass::TransparencyPlatform,
            "prices://fixture/source".to_string(),
            "prices://fixture/document".to_string(),
            "FIXTURE-ZONE".to_string(),
            "EUR".to_string(),
            vec![point],
        )
        .expect("fixture series is valid")
    }

    #[test]
    fn statements_are_the_recorded_strings() {
        assert!(TWO_COPIES_STATEMENT.contains("consumed as-is, never redefined"));
        assert!(TWO_COPIES_STATEMENT.contains("unconstructable"));
        assert!(PRICE_IS_NEVER_A_COMMAND_STATEMENT.contains("never authorizes"));
        assert!(ISO_SEAM_UNWIRED_STATEMENT.contains("not a connection"));
        assert!(ISO_SEAM_UNWIRED_STATEMENT.contains("[S-94]"));
    }

    #[test]
    fn a_record_constructs_from_wire_decimal_text_only() {
        let record = MarketPriceRecord::new(
            "pub://fixture/a44/1".to_string(),
            MarketSegment::DayAhead,
            instant(0),
            instant(3_600_000),
            "42.50",
            "EUR".to_string(),
            "FIXTURE-ZONE".to_string(),
            PriceSourceClass::TransparencyPlatform,
            "doc://fixture/a44".to_string(),
        )
        .expect("valid record");
        assert_eq!(record.segment.as_str(), "day-ahead");
        assert_eq!(record.price.canonical_string(), "42.50 EUR");
        // Negative prices are carried as published, sign included.
        let record = MarketPriceRecord::new(
            "pub://fixture/a44/2".to_string(),
            MarketSegment::RealTime,
            instant(0),
            instant(900_000),
            "-12.5",
            "EUR".to_string(),
            "FIXTURE-ZONE".to_string(),
            PriceSourceClass::TransparencyPlatform,
            "doc://fixture/rt".to_string(),
        )
        .expect("valid record");
        assert_eq!(record.price.canonical_string(), "-12.5 EUR");
    }

    #[test]
    fn record_refusals_are_named() {
        let err = MarketPriceRecord::new(
            "  ".to_string(),
            MarketSegment::DayAhead,
            instant(0),
            instant(1),
            "1.0",
            "EUR".to_string(),
            "Z".to_string(),
            PriceSourceClass::TransparencyPlatform,
            "doc".to_string(),
        )
        .expect_err("empty record ref refused");
        assert_eq!(err.name(), "PRICE_RECORD_EMPTY_REFERENCE");
        let err = MarketPriceRecord::new(
            "r".to_string(),
            MarketSegment::DayAhead,
            instant(1),
            instant(1),
            "1.0",
            "EUR".to_string(),
            "Z".to_string(),
            PriceSourceClass::TransparencyPlatform,
            "doc".to_string(),
        )
        .expect_err("empty interval refused");
        assert_eq!(err.name(), "PRICE_RECORD_INVERTED_INTERVAL");
        let err = MarketPriceRecord::new(
            "r".to_string(),
            MarketSegment::DayAhead,
            instant(0),
            instant(1),
            "not-a-decimal",
            "EUR".to_string(),
            "Z".to_string(),
            PriceSourceClass::TransparencyPlatform,
            "doc".to_string(),
        )
        .expect_err("bad decimal refused");
        assert!(matches!(err, PriceRecordError::DecimalInvalid { .. }));
        let err = MarketPriceRecord::new(
            "r".to_string(),
            MarketSegment::DayAhead,
            instant(0),
            instant(1),
            "1.0",
            "  ".to_string(),
            "Z".to_string(),
            PriceSourceClass::TransparencyPlatform,
            "doc".to_string(),
        )
        .expect_err("empty currency refused");
        assert_eq!(err.name(), "PRICE_RECORD_EMPTY_CURRENCY");
    }

    #[test]
    fn series_classification_is_verified_only() {
        let series = fixture_series(PriceSeriesKind::Actual);
        let (segment, doc) = classify_series_segment(&series, TransparencyDocumentClass::A44DayAhead)
            .expect("A44 classifies");
        assert_eq!(segment, Some(MarketSegment::DayAhead));
        assert_eq!(doc, "prices://fixture/document");
        // An unverified document class refuses rather than guesses.
        let err = classify_series_segment(&series, TransparencyDocumentClass::OtherUnverified)
            .expect_err("unverified class refused");
        assert_eq!(err.name(), "PRICE_RECORD_SEGMENT_NOT_CLASSIFIABLE");
        // A vendor FORECAST links to no segment (it is not a cleared
        // price) — and never errors on the forecast itself.
        let forecast = fixture_series(PriceSeriesKind::VendorForecast);
        let (segment, _) =
            classify_series_segment(&forecast, TransparencyDocumentClass::A44DayAhead)
                .expect("forecast links to no segment");
        assert_eq!(segment, None);
    }

    #[test]
    fn publication_records_assemble_all_or_nothing() {
        let batch = PricePublicationBatch {
            document_ref: "pub://fixture/iso/1".to_string(),
            segment: MarketSegment::RealTime,
            entries: vec![
                PricePublicationEntry {
                    interval_start: instant(0),
                    interval_end: instant(300_000),
                    price_text: "55.25".to_string(),
                    currency_code: "USD".to_string(),
                    area: "FIXTURE-NODE".to_string(),
                },
                PricePublicationEntry {
                    interval_start: instant(300_000),
                    interval_end: instant(600_000),
                    price_text: "61.75".to_string(),
                    currency_code: "USD".to_string(),
                    area: "FIXTURE-NODE".to_string(),
                },
            ],
        };
        let records = records_from_publication(&batch, PriceSourceClass::CommercialVendor)
            .expect("valid batch");
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].price.canonical_string(), "55.25 USD");
        assert_eq!(records[1].segment.as_str(), "real-time");
        // One malformed entry refuses the WHOLE batch (all-or-nothing).
        let bad = PricePublicationBatch {
            document_ref: "pub://fixture/iso/2".to_string(),
            segment: MarketSegment::RealTime,
            entries: vec![PricePublicationEntry {
                interval_start: instant(0),
                interval_end: instant(300_000),
                price_text: "5x5".to_string(),
                currency_code: "USD".to_string(),
                area: "FIXTURE-NODE".to_string(),
            }],
        };
        let err = records_from_publication(&bad, PriceSourceClass::CommercialVendor)
            .expect_err("malformed batch refused whole");
        assert!(matches!(err, PriceRecordError::DecimalInvalid { .. }));
    }

    #[test]
    fn segment_labels_are_stable() {
        assert_eq!(MarketSegment::DayAhead.as_str(), "day-ahead");
        assert_eq!(MarketSegment::RealTime.as_str(), "real-time");
        assert_eq!(
            TransparencyDocumentClass::A44DayAhead.as_str(),
            "a44-day-ahead"
        );
        assert_eq!(
            TransparencyDocumentClass::OtherUnverified.as_str(),
            "other-unverified"
        );
    }
}
