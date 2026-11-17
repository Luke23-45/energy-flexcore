// P15.04 — The A44 day-ahead price document model ([S-111], UNVERIFIED).
//
// The ENTSO-E Transparency Platform publishes day-ahead prices as
// documents of type A44 ("Day ahead prices") [S-111]. The types below
// model that document's price-relevant content: the document identity,
// the time series (one per domain pairing / currency), the periods (the
// publication's time structure), and the points (interval position +
// price amount).
//
// UNVERIFIED register (implementation-time checks against the live API
// guide [S-111] and a registered token; a mismatch at that gate is a
// recorded change, never a silent reinterpretation):
//
// | # | Detail | Where |
// |---|---|---|
// | 1 | The document-type code "A44" names Day-ahead prices | [`A44_DOCUMENT_TYPE`] |
// | 2 | The XML element names used by the extraction seam | `xml_seam` |
// | 3 | The period start/end format (YYYYMMDDHHMM, UTC) | [`Period::parse_bound`] |
// | 4 | The resolution codes (PT60M/PT30M/PT15M) and their mapping to interval minutes | [`Resolution::parse`] |
// | 5 | The currency convention (a currency field or a fixed EUR default per zone) | [`TimeSeries::currency_code`] |
// | 6 | Whether amounts are "price.amount" per point or per period | `xml_seam::extract_a44_price_points` |
//
// Determinism: every parse/validate is fail-closed over closed
// vocabularies with named errors; no float formatting tricks; the
// extraction order follows document order.
//
// Control status: model types for READ-ONLY data ingestion. Nothing here
// authorizes, issues, dispatches, or bids; a price point is ingested
// DATA, never a command and never a bid input.

/// The A44 document-type code: day-ahead prices ([S-111], UNVERIFIED).
pub const A44_DOCUMENT_TYPE: &str = "A44";

/// The maximum accepted document size in bytes (fail-closed bound; a
/// price document is metadata-plus-numeric-points, not a payload).
pub const MAX_A44_DOCUMENT_BYTES: usize = 8 * 1024 * 1024;

/// The resolution vocabulary of an A44 period ([S-111], UNVERIFIED).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resolution {
    /// One-hour resolution (PT60M).
    Pt60m,
    /// 30-minute resolution (PT30M).
    Pt30m,
    /// 15-minute resolution (PT15M).
    Pt15m,
}

impl Resolution {
    /// Parses the resolution code (UNVERIFIED spelling per [S-111];
    /// unknown codes are refused, never guessed).
    pub fn parse(code: &str) -> Option<Resolution> {
        match code {
            "PT60M" => Some(Resolution::Pt60m),
            "PT30M" => Some(Resolution::Pt30m),
            "PT15M" => Some(Resolution::Pt15m),
            _ => None,
        }
    }

    /// The interval length this resolution implies, minutes.
    pub const fn interval_minutes(self) -> u32 {
        match self {
            Resolution::Pt60m => 60,
            Resolution::Pt30m => 30,
            Resolution::Pt15m => 15,
        }
    }

    /// The wire spelling of the resolution (records).
    pub const fn as_str(self) -> &'static str {
        match self {
            Resolution::Pt60m => "PT60M",
            Resolution::Pt30m => "PT30M",
            Resolution::Pt15m => "PT15M",
        }
    }
}

/// Why an A44 fact was refused. Every variant is a named error; refusals
/// never degrade into a default value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum A44Error {
    /// A code, reference, or currency was empty or whitespace.
    EmptyField { field: &'static str },
    /// A resolution code was unknown (closed vocabulary).
    UnknownResolution { value: String },
    /// A period bound did not parse as the recorded format (UNVERIFIED
    /// format per the register above).
    InvalidPeriodBound { field: &'static str, value: String },
    /// The period's point count disagreed with its resolution and
    /// duration (a period that declares N minutes at resolution R must
    /// carry exactly N / interval(R) points).
    PointCountMismatch {
        period_index: usize,
        expected: usize,
        found: usize,
    },
    /// A price amount was NaN or infinite.
    NonFiniteAmount { series_index: usize, point_index: usize },
    /// The document carried a document-type other than A44 (the adapter
    /// ingests day-ahead prices only; other document types are out of
    /// scope and refused, never loosely parsed).
    UnsupportedDocumentType { found: String },
}

impl A44Error {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            A44Error::EmptyField { .. } => "A44_EMPTY_FIELD",
            A44Error::UnknownResolution { .. } => "A44_UNKNOWN_RESOLUTION",
            A44Error::InvalidPeriodBound { .. } => "A44_INVALID_PERIOD_BOUND",
            A44Error::PointCountMismatch { .. } => "A44_POINT_COUNT_MISMATCH",
            A44Error::NonFiniteAmount { .. } => "A44_NON_FINITE_AMOUNT",
            A44Error::UnsupportedDocumentType { .. } => "A44_UNSUPPORTED_DOCUMENT_TYPE",
        }
    }
}

impl std::fmt::Display for A44Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            A44Error::EmptyField { field } => {
                write!(f, "the '{field}' field must be a non-empty string")
            }
            A44Error::UnknownResolution { value } => write!(
                f,
                "resolution '{value}' is not in the closed vocabulary (PT60M, PT30M, \
                 PT15M — UNVERIFIED per [S-111]); unknown resolutions are refused, never \
                 guessed"
            ),
            A44Error::InvalidPeriodBound { field, value } => write!(
                f,
                "period bound '{field}' value '{value}' does not parse as the recorded \
                 format (YYYYMMDDHHMM, UTC — UNVERIFIED per [S-111])"
            ),
            A44Error::PointCountMismatch {
                period_index,
                expected,
                found,
            } => write!(
                f,
                "period {period_index} carries {found} points but its duration and \
                 resolution imply {expected} (a structural mismatch is refused, never \
                 papered over)"
            ),
            A44Error::NonFiniteAmount {
                series_index,
                point_index,
            } => write!(
                f,
                "series {series_index} point {point_index}: price amount is not finite"
            ),
            A44Error::UnsupportedDocumentType { found } => write!(
                f,
                "document type '{found}' is not {A44_DOCUMENT_TYPE} (day-ahead prices); \
                 this adapter ingests A44 documents only, never loosely parses others"
            ),
        }
    }
}

impl std::error::Error for A44Error {}

/// Parses a YYYYMMDDHHMM period bound into UTC epoch milliseconds
/// (UNVERIFIED format per the register; the format is treated as UTC —
/// the platform's publication times are UTC per [S-111], UNVERIFIED).
/// Pure integer arithmetic; no date library.
pub fn parse_period_bound(value: &str) -> Result<i64, A44Error> {
    const DIGITS: usize = 12; // YYYYMMDDHHMM
    if value.len() != DIGITS || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(A44Error::InvalidPeriodBound {
            field: "period-bound",
            value: value.to_string(),
        });
    }
    let year: i64 = value[0..4].parse().map_err(|_| A44Error::InvalidPeriodBound {
        field: "year",
        value: value.to_string(),
    })?;
    let month: u32 = value[4..6].parse().map_err(|_| A44Error::InvalidPeriodBound {
        field: "month",
        value: value.to_string(),
    })?;
    let day: u32 = value[6..8].parse().map_err(|_| A44Error::InvalidPeriodBound {
        field: "day",
        value: value.to_string(),
    })?;
    let hour: u32 = value[8..10].parse().map_err(|_| A44Error::InvalidPeriodBound {
        field: "hour",
        value: value.to_string(),
    })?;
    let minute: u32 = value[10..12].parse().map_err(|_| A44Error::InvalidPeriodBound {
        field: "minute",
        value: value.to_string(),
    })?;
    if !(1..=12).contains(&month) {
        return Err(A44Error::InvalidPeriodBound {
            field: "month",
            value: value.to_string(),
        });
    }
    if !(1..=31).contains(&day) {
        return Err(A44Error::InvalidPeriodBound {
            field: "day",
            value: value.to_string(),
        });
    }
    if hour > 23 {
        return Err(A44Error::InvalidPeriodBound {
            field: "hour",
            value: value.to_string(),
        });
    }
    if minute > 59 {
        return Err(A44Error::InvalidPeriodBound {
            field: "minute",
            value: value.to_string(),
        });
    }
    // Days-since-epoch via the civil-from-days algorithm (Howard Hinnant's
    // formulation, hand-verifiable): exact integer arithmetic, no
    // leap-year surprises beyond the formula.
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let m_shifted = if month > 2 { month - 3 } else { month + 9 } as i64; // [0, 11]
    let doy = (153 * m_shifted + 2) / 5 + day as i64 - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    let days = era * 146_097 + doe - 719_468; // days since 1970-01-01
    Ok(days * 86_400_000 + hour as i64 * 3_600_000 + minute as i64 * 60_000)
}

/// One price point of an A44 period: the position within the period and
/// the price amount (UNVERIFIED field spellings per the register).
#[derive(Debug, Clone, PartialEq)]
pub struct A44Point {
    /// The 1-based position within the period (as published; UNVERIFIED).
    pub position: u32,
    /// The price amount as published (the currency and per-energy unit
    /// are the series' declaration; sign is NOT constrained — negative
    /// prices are real market outcomes, UNVERIFIED for any specific
    /// bidding zone).
    pub price_amount: f64,
}

/// One period of an A44 time series: the time structure and the points.
#[derive(Debug, Clone, PartialEq)]
pub struct A44Period {
    /// The period start (parsed from the recorded format; UNVERIFIED).
    pub start_millis: i64,
    /// The period end (parsed; UNVERIFIED).
    pub end_millis: i64,
    /// The resolution (closed vocabulary).
    pub resolution: Resolution,
    /// The points, in position order (validated).
    pub points: Vec<A44Point>,
}

/// The period constructor: every structural violation is one of the
/// [`A44Error`] variants.
impl A44Period {
    /// Validates and constructs one period (fail-closed): bounds parse
    /// and are ordered; the resolution is known; the point count matches
    /// the duration/resolution arithmetic; positions are 1-based
    /// contiguous; amounts are finite.
    pub fn new(
        start_text: &str,
        end_text: &str,
        resolution_code: &str,
        points: Vec<(u32, f64)>,
    ) -> Result<Self, A44Error> {
        let start_millis = parse_period_bound(start_text)?;
        let end_millis = parse_period_bound(end_text)?;
        let resolution = Resolution::parse(resolution_code).ok_or_else(|| {
            A44Error::UnknownResolution {
                value: resolution_code.to_string(),
            }
        })?;
        if end_millis <= start_millis {
            return Err(A44Error::InvalidPeriodBound {
                field: "period-end",
                value: end_text.to_string(),
            });
        }
        let expected =
            ((end_millis - start_millis) / 60_000 / resolution.interval_minutes() as i64) as usize;
        if points.len() != expected {
            return Err(A44Error::PointCountMismatch {
                period_index: 0,
                expected,
                found: points.len(),
            });
        }
        let mut validated = Vec::with_capacity(points.len());
        for (index, (position, amount)) in points.into_iter().enumerate() {
            if !amount.is_finite() {
                return Err(A44Error::NonFiniteAmount {
                    series_index: 0,
                    point_index: index,
                });
            }
            if position != (index + 1) as u32 {
                return Err(A44Error::PointCountMismatch {
                    period_index: 0,
                    expected: index + 1,
                    found: position as usize,
                });
            }
            validated.push(A44Point {
                position,
                price_amount: amount,
            });
        }
        Ok(Self {
            start_millis,
            end_millis,
            resolution,
            points: validated,
        })
    }
}

/// One A44 time series: the domain pairing, the currency, and its
/// periods (UNVERIFIED field spellings per the register).
#[derive(Debug, Clone, PartialEq)]
pub struct A44TimeSeries {
    /// The in-domain area identifier as published (carried verbatim; the
    /// area vocabulary is the platform's, not the platform's — UNVERIFIED
    /// until the zone map is checked).
    pub in_domain: String,
    /// The out-domain area identifier as published.
    pub out_domain: String,
    /// The DECLARED currency code (UNVERIFIED convention; no default
    /// currency exists anywhere — a currency-less series is refused).
    pub currency_code: String,
    /// The periods, in document order (validated).
    pub periods: Vec<A44Period>,
}

/// One A44 document: the identity and its time series (UNVERIFIED field
/// spellings per the register).
#[derive(Debug, Clone, PartialEq)]
pub struct A44Document {
    /// The document identity as published (a reference string; carried
    /// verbatim).
    pub document_id: String,
    /// The document type (must be exactly [`A44_DOCUMENT_TYPE`]).
    pub document_type: String,
    /// The time series, in document order (validated).
    pub time_series: Vec<A44TimeSeries>,
}

impl A44Document {
    /// Validates the document-type gate (the adapter ingests A44 only).
    pub fn new(
        document_id: String,
        document_type: String,
        time_series: Vec<A44TimeSeries>,
    ) -> Result<Self, A44Error> {
        if document_id.trim().is_empty() {
            return Err(A44Error::EmptyField {
                field: "document-id",
            });
        }
        if document_type != A44_DOCUMENT_TYPE {
            return Err(A44Error::UnsupportedDocumentType { found: document_type });
        }
        if time_series.is_empty() {
            return Err(A44Error::EmptyField {
                field: "time-series",
            });
        }
        for series in &time_series {
            for (field, value) in [
                ("in-domain", &series.in_domain),
                ("out-domain", &series.out_domain),
                ("currency-code", &series.currency_code),
            ] {
                if value.trim().is_empty() {
                    return Err(A44Error::EmptyField { field });
                }
            }
            if series.periods.is_empty() {
                return Err(A44Error::EmptyField {
                    field: "periods",
                });
            }
        }
        Ok(Self {
            document_id,
            document_type,
            time_series,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every numeric expectation below is
// HAND-COMPUTED from the rules above — the civil-from-days arithmetic is
// checked against known dates. Nothing is fabricated.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn period_bounds_parse_as_hand_computed_utc_millis() {
        // HAND CHECK: 1970-01-01T00:00 = 0 ms (the epoch itself).
        assert_eq!(parse_period_bound("197001010000").expect("epoch"), 0);
        // HAND CHECK: 2026-06-01T00:00 — the P3.31 fixture horizon start.
        // The fixture records 1_780_272_000_000 ms for
        // 2026-06-01T00:00:00Z; the parser must reproduce the recorded
        // value (a cross-artifact consistency pin, not an invention).
        assert_eq!(
            parse_period_bound("202606010000").expect("fixture start"),
            1_780_272_000_000
        );
        // HAND CHECK: 2026-06-02T00:00 = fixture start + 86_400_000 ms.
        assert_eq!(
            parse_period_bound("202606020000").expect("next day"),
            1_780_272_000_000 + 86_400_000
        );
        // HAND CHECK: 2024-02-29 (a leap day) = 2024-03-01 minus one day.
        // 2024-03-01T00:00 = 1_709_251_200_000 ms (known value:
        // 1709251200 s). The leap day is one day earlier.
        assert_eq!(
            parse_period_bound("202402290000").expect("leap day"),
            1_709_251_200_000 - 86_400_000
        );
    }

    #[test]
    fn malformed_period_bounds_are_refused() {
        for value in ["2026060100", "2026060100000", "2026-06-01", "20260601000A"] {
            let err = parse_period_bound(value).expect_err("malformed bound refused");
            assert_eq!(err.name(), "A44_INVALID_PERIOD_BOUND");
        }
        let err = parse_period_bound("202613010000").expect_err("month 13 refused");
        assert_eq!(err.name(), "A44_INVALID_PERIOD_BOUND");
        let err = parse_period_bound("202606012500").expect_err("hour 25 refused");
        assert_eq!(err.name(), "A44_INVALID_PERIOD_BOUND");
    }

    #[test]
    fn the_resolution_vocabulary_is_closed() {
        assert_eq!(Resolution::parse("PT60M"), Some(Resolution::Pt60m));
        assert_eq!(Resolution::parse("PT30M"), Some(Resolution::Pt30m));
        assert_eq!(Resolution::parse("PT15M"), Some(Resolution::Pt15m));
        assert_eq!(Resolution::parse("PT90M"), None);
        assert_eq!(Resolution::Pt60m.interval_minutes(), 60);
        assert_eq!(Resolution::Pt15m.interval_minutes(), 15);
        assert_eq!(Resolution::Pt60m.as_str(), "PT60M");
    }

    #[test]
    fn a_well_formed_period_constructs_with_the_exact_point_count() {
        // One day at hourly resolution: exactly 24 points (UNVERIFIED
        // positions 1..=24 per the register).
        let points: Vec<(u32, f64)> = (1..=24).map(|p| (p, 42.0)).collect();
        let period = A44Period::new("202606010000", "202606020000", "PT60M", points)
            .expect("24 hourly points over one day");
        assert_eq!(period.points.len(), 24);
        assert_eq!(period.resolution, Resolution::Pt60m);
        assert_eq!(period.start_millis, 1_780_272_000_000);
        // HAND CHECK: 30-minute resolution over one day = 48 points.
        let points: Vec<(u32, f64)> = (1..=48).map(|p| (p, 10.0)).collect();
        let period = A44Period::new("202606010000", "202606020000", "PT30M", points)
            .expect("48 half-hourly points over one day");
        assert_eq!(period.points.len(), 48);
    }

    #[test]
    fn point_count_mismatches_are_refused() {
        // 23 points for a 24-point day: refused, never padded.
        let points: Vec<(u32, f64)> = (1..=23).map(|p| (p, 42.0)).collect();
        let err = A44Period::new("202606010000", "202606020000", "PT60M", points)
            .expect_err("short day refused");
        assert_eq!(
            err,
            A44Error::PointCountMismatch {
                period_index: 0,
                expected: 24,
                found: 23,
            }
        );
        // Positions must be 1-based contiguous (a gap or a repeat is a
        // structural mismatch).
        let points: Vec<(u32, f64)> = (1..=24).map(|p| (p, 1.0)).collect();
        let mut broken = points.clone();
        broken[4] = (9, 1.0);
        let err = A44Period::new("202606010000", "202606020000", "PT60M", broken)
            .expect_err("non-contiguous positions refused");
        assert_eq!(err.name(), "A44_POINT_COUNT_MISMATCH");
    }

    #[test]
    fn non_finite_amounts_are_refused() {
        let points: Vec<(u32, f64)> = (1..=24)
            .map(|p| (p, if p == 5 { f64::NAN } else { 1.0 }))
            .collect();
        let err = A44Period::new("202606010000", "202606020000", "PT60M", points)
            .expect_err("NaN amount refused");
        assert_eq!(err.name(), "A44_NON_FINITE_AMOUNT");
    }

    #[test]
    fn the_document_type_gate_accepts_a44_only() {
        let series = A44TimeSeries {
            in_domain: "FIXTURE-IN".to_string(),
            out_domain: "FIXTURE-OUT".to_string(),
            currency_code: "EUR".to_string(),
            periods: vec![A44Period::new(
                "202606010000",
                "202606020000",
                "PT60M",
                (1..=24).map(|p| (p, 42.0)).collect(),
            )
            .expect("fixture period")],
        };
        let doc = A44Document::new(
            "fixture-doc-1".to_string(),
            A44_DOCUMENT_TYPE.to_string(),
            vec![series.clone()],
        )
        .expect("an A44 document is accepted");
        assert_eq!(doc.document_type, "A44");
        let err = A44Document::new(
            "fixture-doc-2".to_string(),
            "A43".to_string(),
            vec![series],
        )
        .expect_err("non-A44 refused");
        assert_eq!(err.name(), "A44_UNSUPPORTED_DOCUMENT_TYPE");
    }

    #[test]
    fn negative_prices_are_carried_not_refused() {
        // Negative prices are real market outcomes in several
        // jurisdictions (UNVERIFIED for any specific bidding zone); the
        // model carries the published amount, sign included.
        let points: Vec<(u32, f64)> = (1..=24).map(|p| (p, if p == 3 { -12.5 } else { 1.0 })).collect();
        let period =
            A44Period::new("202606010000", "202606020000", "PT60M", points).expect("carried");
        assert_eq!(period.points[2].price_amount, -12.5);
    }

    #[test]
    fn empty_fields_are_refused() {
        let series = A44TimeSeries {
            in_domain: "  ".to_string(),
            out_domain: "FIXTURE-OUT".to_string(),
            currency_code: "EUR".to_string(),
            periods: vec![A44Period::new(
                "202606010000",
                "202606020000",
                "PT60M",
                (1..=24).map(|p| (p, 1.0)).collect(),
            )
            .expect("fixture period")],
        };
        let err = A44Document::new(
            "fixture-doc".to_string(),
            A44_DOCUMENT_TYPE.to_string(),
            vec![series],
        )
        .expect_err("empty in-domain refused");
        assert_eq!(err, A44Error::EmptyField { field: "in-domain" });
    }

    #[test]
    fn every_a44_error_renders_non_empty_prose() {
        let errors = [
            A44Error::EmptyField { field: "x" },
            A44Error::UnknownResolution {
                value: "PT90M".to_string(),
            },
            A44Error::InvalidPeriodBound {
                field: "day",
                value: "x".to_string(),
            },
            A44Error::PointCountMismatch {
                period_index: 0,
                expected: 24,
                found: 23,
            },
            A44Error::NonFiniteAmount {
                series_index: 0,
                point_index: 0,
            },
            A44Error::UnsupportedDocumentType {
                found: "A43".to_string(),
            },
        ];
        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }
}
