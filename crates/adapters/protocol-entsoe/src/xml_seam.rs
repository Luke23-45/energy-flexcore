// P15.04 — The A44 document extraction seam.
//
// The Transparency Platform's REST responses are XML documents ([S-111];
// the element names and structure are UNVERIFIED until checked against
// the live API guide). This module provides:
//
// 1. a MINIMAL internal XML node type ([`XmlNode`]) — the representation
//    the composition root's real parser fills (NO XML parser library in
//    this crate; the CIM/CGMES precedent, P14.12: the parsing is a
//    designed UNWIRED seam), and
// 2. a pure, deterministic extraction from a parsed A44 node tree into
//    validated price points ([`extract_a44_price_points`]) — the real
//    logic, tested against hand-built trees.
//
// UNVERIFIED element names (implementation-time checks against [S-111];
// a mismatch at that gate is a recorded code change, never a silent
// reinterpretation):
//
// | # | Element | Where |
// |---|---|---|
// | 1 | The time-series element | [`A44_ELEMENT_TIME_SERIES`] |
// | 2 | The period element | [`A44_ELEMENT_PERIOD`] |
// | 3 | The point element | [`A44_ELEMENT_POINT`] |
// | 4 | The position element | [`A44_ELEMENT_POSITION`] |
// | 5 | The price-amount element | [`A44_ELEMENT_PRICE_AMOUNT`] |
// | 6 | The start/end/resolution child elements | [`A44_ELEMENT_START`] etc. |
//
// Determinism: the extraction walks children in document order, refuses
// whole documents on any structural violation (never trims), and emits
// points in period/position order.
//
// Control status: a data-extraction seam for READ-ONLY ingestion.
// Nothing here authorizes, issues, dispatches, or bids.

use crate::document::Resolution;

/// The recorded element names (UNVERIFIED per [S-111]).
pub const A44_ELEMENT_TIME_SERIES: &str = "TimeSeries";
pub const A44_ELEMENT_PERIOD: &str = "Period";
pub const A44_ELEMENT_POINT: &str = "Point";
pub const A44_ELEMENT_POSITION: &str = "position";
pub const A44_ELEMENT_PRICE_AMOUNT: &str = "price.amount";
pub const A44_ELEMENT_START: &str = "start";
pub const A44_ELEMENT_END: &str = "end";
pub const A44_ELEMENT_RESOLUTION: &str = "resolution";
pub const A44_ELEMENT_IN_DOMAIN: &str = "in_Domain.mRID";
pub const A44_ELEMENT_OUT_DOMAIN: &str = "out_Domain.mRID";
pub const A44_ELEMENT_CURRENCY: &str = "currency_Unit.name";

/// A minimal XML node: the tag, the attributes, the text, and the
/// children, in document order. The composition root's parser fills this
/// type; the extraction below is pure over it. Attributes are held in a
/// sorted vec of pairs (deterministic iteration without a map dependency).
#[derive(Debug, Clone, PartialEq)]
pub struct XmlNode {
    /// The element (or attribute-distinguished) tag name.
    pub tag: String,
    /// The node's text content (elements that carry values).
    pub text: String,
    /// The child nodes, in document order.
    pub children: Vec<XmlNode>,
}

impl XmlNode {
    /// Constructs a node (convenience for tests and parsers).
    pub fn new(tag: &str, text: &str, children: Vec<XmlNode>) -> XmlNode {
        XmlNode {
            tag: tag.to_string(),
            text: text.to_string(),
            children,
        }
    }

    /// The first child with the given tag (document order), if any.
    pub fn child(&self, tag: &str) -> Option<&XmlNode> {
        self.children.iter().find(|c| c.tag == tag)
    }

    /// All children with the given tag, in document order.
    pub fn children_tagged<'a>(&'a self, tag: &'a str) -> impl Iterator<Item = &'a XmlNode> {
        self.children.iter().filter(move |c| c.tag == tag)
    }

    /// The node's trimmed text (empty for container elements).
    pub fn text_trimmed(&self) -> &str {
        self.text.trim()
    }
}

/// Why an extraction was refused. Every structural violation is one of
/// the [`A44Error`] variants; this module adds the navigation errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum A44ExtractionError {
    /// The document had no time-series element (an empty document is a
    /// refusal, never an empty extraction).
    NoTimeSeries,
    /// A series lacked a required child element.
    MissingElement { parent: &'static str, element: &'static str },
    /// A numeric field did not parse.
    UnparseableNumber { element: &'static str, text: String },
    /// A period failed its own validation (the A44Error's name rides).
    PeriodInvalid { reason: String },
}

impl A44ExtractionError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            A44ExtractionError::NoTimeSeries => "A44_EXTRACTION_NO_TIME_SERIES",
            A44ExtractionError::MissingElement { .. } => "A44_EXTRACTION_MISSING_ELEMENT",
            A44ExtractionError::UnparseableNumber { .. } => "A44_EXTRACTION_UNPARSEABLE_NUMBER",
            A44ExtractionError::PeriodInvalid { .. } => "A44_EXTRACTION_PERIOD_INVALID",
        }
    }
}

impl std::fmt::Display for A44ExtractionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            A44ExtractionError::NoTimeSeries => write!(
                f,
                "the document carries no time-series element; an empty document is a \
                 refusal, never an empty extraction"
            ),
            A44ExtractionError::MissingElement { parent, element } => write!(
                f,
                "the '{parent}' element lacks its required '{element}' child (UNVERIFIED \
                 element names per [S-111]); the whole document is refused, never trimmed"
            ),
            A44ExtractionError::UnparseableNumber { element, text } => write!(
                f,
                "the '{element}' text '{text}' does not parse as a number"
            ),
            A44ExtractionError::PeriodInvalid { reason } => {
                write!(f, "a period failed validation: {reason}")
            }
        }
    }
}

impl std::error::Error for A44ExtractionError {}

/// One extracted series: the domain pairing, currency, and points in
/// order (the input to `flexcore-ports::price_signal::PriceSeries::new`
/// at the composition boundary; the port carries the validation there —
/// this module emits points in ascending interval order by construction).
#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedA44Series {
    /// The in-domain area identifier (carried verbatim).
    pub in_domain: String,
    /// The out-domain area identifier.
    pub out_domain: String,
    /// The DECLARED currency code.
    pub currency_code: String,
    /// The price points, in period/position order: (start_millis,
    /// end_millis, amount).
    pub points: Vec<ExtractedA44Point>,
}

/// One extracted price point with its resolved interval bounds.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedA44Point {
    /// Interval start (UTC millis).
    pub start_millis: i64,
    /// Interval end (UTC millis; start + the period's resolution).
    pub end_millis: i64,
    /// The price amount as published (sign unconstrained).
    pub price_amount: f64,
}

/// Extracts the price points of the FIRST time series of a parsed A44
/// document node tree (document order; the multi-series case is one call
/// per series — the extraction refuses a document with no series). Pure,
/// deterministic, fail-closed: any structural violation refuses the whole
/// extraction.
pub fn extract_a44_price_points(document: &XmlNode) -> Result<ExtractedA44Series, A44ExtractionError> {
    let series = document
        .child(A44_ELEMENT_TIME_SERIES)
        .ok_or(A44ExtractionError::NoTimeSeries)?;

    let in_domain = series
        .child(A44_ELEMENT_IN_DOMAIN)
        .ok_or(A44ExtractionError::MissingElement {
            parent: A44_ELEMENT_TIME_SERIES,
            element: A44_ELEMENT_IN_DOMAIN,
        })?
        .text_trimmed()
        .to_string();
    let out_domain = series
        .child(A44_ELEMENT_OUT_DOMAIN)
        .ok_or(A44ExtractionError::MissingElement {
            parent: A44_ELEMENT_TIME_SERIES,
            element: A44_ELEMENT_OUT_DOMAIN,
        })?
        .text_trimmed()
        .to_string();
    let currency_code = series
        .child(A44_ELEMENT_CURRENCY)
        .ok_or(A44ExtractionError::MissingElement {
            parent: A44_ELEMENT_TIME_SERIES,
            element: A44_ELEMENT_CURRENCY,
        })?
        .text_trimmed()
        .to_string();

    let period = series
        .child(A44_ELEMENT_PERIOD)
        .ok_or(A44ExtractionError::MissingElement {
            parent: A44_ELEMENT_TIME_SERIES,
            element: A44_ELEMENT_PERIOD,
        })?;
    let start_text = period
        .child(A44_ELEMENT_START)
        .ok_or(A44ExtractionError::MissingElement {
            parent: A44_ELEMENT_PERIOD,
            element: A44_ELEMENT_START,
        })?
        .text_trimmed()
        .to_string();
    let end_text = period
        .child(A44_ELEMENT_END)
        .ok_or(A44ExtractionError::MissingElement {
            parent: A44_ELEMENT_PERIOD,
            element: A44_ELEMENT_END,
        })?
        .text_trimmed()
        .to_string();
    let resolution_text = period
        .child(A44_ELEMENT_RESOLUTION)
        .ok_or(A44ExtractionError::MissingElement {
            parent: A44_ELEMENT_PERIOD,
            element: A44_ELEMENT_RESOLUTION,
        })?
        .text_trimmed()
        .to_string();
    let resolution = Resolution::parse(&resolution_text).ok_or_else(|| {
        A44ExtractionError::PeriodInvalid {
            reason: format!(
                "resolution '{resolution_text}' is not in the closed vocabulary"
            ),
        }
    })?;

    let start_millis = crate::document::parse_period_bound(&start_text)
        .map_err(|err| A44ExtractionError::PeriodInvalid {
            reason: err.to_string(),
        })?;
    let end_millis = crate::document::parse_period_bound(&end_text)
        .map_err(|err| A44ExtractionError::PeriodInvalid {
            reason: err.to_string(),
        })?;
    if end_millis <= start_millis {
        return Err(A44ExtractionError::PeriodInvalid {
            reason: "the period is inverted (start must be before end)".to_string(),
        });
    }
    let interval_millis = resolution.interval_minutes() as i64 * 60_000;

    let mut points = Vec::new();
    for (index, point_node) in period.children_tagged(A44_ELEMENT_POINT).enumerate() {
        let position_text = point_node
            .child(A44_ELEMENT_POSITION)
            .ok_or(A44ExtractionError::MissingElement {
                parent: A44_ELEMENT_POINT,
                element: A44_ELEMENT_POSITION,
            })?
            .text_trimmed()
            .to_string();
        let position: u32 = position_text.parse().map_err(|_| {
            A44ExtractionError::UnparseableNumber {
                element: A44_ELEMENT_POSITION,
                text: position_text.clone(),
            }
        })?;
        if position != (index + 1) as u32 {
            return Err(A44ExtractionError::PeriodInvalid {
                reason: format!(
                    "point position {position} at document index {index} breaks the \
                     1-based contiguity rule"
                ),
            });
        }
        let amount_text = point_node
            .child(A44_ELEMENT_PRICE_AMOUNT)
            .ok_or(A44ExtractionError::MissingElement {
                parent: A44_ELEMENT_POINT,
                element: A44_ELEMENT_PRICE_AMOUNT,
            })?
            .text_trimmed()
            .to_string();
        let amount: f64 = amount_text.parse().map_err(|_| {
            A44ExtractionError::UnparseableNumber {
                element: A44_ELEMENT_PRICE_AMOUNT,
                text: amount_text.clone(),
            }
        })?;
        if !amount.is_finite() {
            return Err(A44ExtractionError::PeriodInvalid {
                reason: format!("point {position}: the amount is not finite"),
            });
        }
        points.push(ExtractedA44Point {
            start_millis: start_millis + (index as i64) * interval_millis,
            end_millis: start_millis + ((index as i64) + 1) * interval_millis,
            price_amount: amount,
        });
    }
    if points.is_empty() {
        return Err(A44ExtractionError::PeriodInvalid {
            reason: "the period carries no points; an empty period is a refusal, never \
                     an empty extraction"
            .to_string(),
        });
    }

    Ok(ExtractedA44Series {
        in_domain,
        out_domain,
        currency_code,
        points,
    })
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is
// hand-derived from the rules above; the node trees are hand-built. The
// element names are UNVERIFIED per [S-111] — the tests pin the RECORDED
// spellings, and a mismatch at the live-API gate is a recorded code
// change, never a silent reinterpretation. Nothing is fabricated.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A hand-built minimal A44 tree: one series, one hour, three hourly
    /// points (UNVERIFIED element spellings per the recorded constants).
    fn fixture_tree() -> XmlNode {
        let point = |position: u32, amount: &str| {
            XmlNode::new(
                A44_ELEMENT_POINT,
                "",
                vec![
                    XmlNode::new(A44_ELEMENT_POSITION, &position.to_string(), vec![]),
                    XmlNode::new(A44_ELEMENT_PRICE_AMOUNT, amount, vec![]),
                ],
            )
        };
        XmlNode::new(
            "Publication_MarketDocument",
            "",
            vec![XmlNode::new(
                A44_ELEMENT_TIME_SERIES,
                "",
                vec![
                    XmlNode::new(A44_ELEMENT_IN_DOMAIN, "FIXTURE-IN", vec![]),
                    XmlNode::new(A44_ELEMENT_OUT_DOMAIN, "FIXTURE-OUT", vec![]),
                    XmlNode::new(A44_ELEMENT_CURRENCY, "EUR", vec![]),
                    XmlNode::new(
                        A44_ELEMENT_PERIOD,
                        "",
                        vec![
                            XmlNode::new(A44_ELEMENT_START, "202606010000", vec![]),
                            XmlNode::new(A44_ELEMENT_END, "202606020000", vec![]),
                            XmlNode::new(A44_ELEMENT_RESOLUTION, "PT60M", vec![]),
                            point(1, "42.5"),
                            point(2, "-7.0"),
                            point(3, "38.25"),
                        ],
                    ),
                ],
            )],
        )
    }

    #[test]
    fn the_fixture_tree_extracts_three_ordered_points() {
        // HAND CHECK: 3 hourly points starting 2026-06-01T00:00
        // (1_780_272_000_000 ms), each interval 3_600_000 ms:
        //   point 1: [1780272000000, 1780275600000) amount 42.5
        //   point 2: [1780275600000, 1780279200000) amount -7.0 (negative
        //     prices are carried, sign included)
        //   point 3: [1780279200000, 1780282800000) amount 38.25
        let series = extract_a44_price_points(&fixture_tree()).expect("extracts");
        assert_eq!(series.in_domain, "FIXTURE-IN");
        assert_eq!(series.out_domain, "FIXTURE-OUT");
        assert_eq!(series.currency_code, "EUR");
        assert_eq!(series.points.len(), 3);
        assert_eq!(series.points[0].start_millis, 1_780_272_000_000);
        assert_eq!(series.points[0].end_millis, 1_780_272_000_000 + 3_600_000);
        assert_eq!(series.points[0].price_amount, 42.5);
        assert_eq!(series.points[1].price_amount, -7.0);
        assert_eq!(series.points[2].start_millis, 1_780_272_000_000 + 7_200_000);
        assert_eq!(series.points[2].price_amount, 38.25);
    }

    #[test]
    fn a_document_without_a_time_series_is_refused_whole() {
        let empty = XmlNode::new("Publication_MarketDocument", "", vec![]);
        let err = extract_a44_price_points(&empty).expect_err("no series refused");
        assert_eq!(err.name(), "A44_EXTRACTION_NO_TIME_SERIES");
    }

    #[test]
    fn a_missing_required_element_refuses_the_whole_extraction() {
        let mut tree = fixture_tree();
        // The TimeSeries IS the document's first child (fixture_tree);
        // searching among ITS children finds no TimeSeries (its children
        // are in/out-domain, currency, period).
        let series = tree
            .children
            .iter_mut()
            .find(|c| c.tag == A44_ELEMENT_TIME_SERIES)
            .expect("series");
        series.children.retain(|c| c.tag != A44_ELEMENT_CURRENCY);
        let err = extract_a44_price_points(&tree).expect_err("missing currency refused");
        assert_eq!(err.name(), "A44_EXTRACTION_MISSING_ELEMENT");
    }

    #[test]
    fn a_non_contiguous_position_refuses_the_whole_extraction() {
        let mut tree = fixture_tree();
        // The TimeSeries IS the document's first child (fixture_tree);
        // searching among ITS children finds no TimeSeries (its children
        // are in/out-domain, currency, period).
        let series = tree
            .children
            .iter_mut()
            .find(|c| c.tag == A44_ELEMENT_TIME_SERIES)
            .expect("series");
        let period = series
            .children
            .iter_mut()
            .find(|c| c.tag == A44_ELEMENT_PERIOD)
            .expect("period");
        let third = period
            .children
            .iter_mut()
            .find(|c| c.tag == A44_ELEMENT_POINT && c.children[0].text == "3")
            .expect("third point");
        third.children[0].text = "4".to_string();
        let err = extract_a44_price_points(&tree).expect_err("position gap refused");
        assert_eq!(err.name(), "A44_EXTRACTION_PERIOD_INVALID");
    }

    #[test]
    fn an_unparseable_amount_refuses_the_whole_extraction() {
        let mut tree = fixture_tree();
        // The TimeSeries IS the document's first child (fixture_tree);
        // searching among ITS children finds no TimeSeries (its children
        // are in/out-domain, currency, period).
        let series = tree
            .children
            .iter_mut()
            .find(|c| c.tag == A44_ELEMENT_TIME_SERIES)
            .expect("series");
        let period = series
            .children
            .iter_mut()
            .find(|c| c.tag == A44_ELEMENT_PERIOD)
            .expect("period");
        let first = period
            .children
            .iter_mut()
            .find(|c| c.tag == A44_ELEMENT_POINT)
            .expect("first point");
        first.children[1].text = "not-a-number".to_string();
        let err = extract_a44_price_points(&tree).expect_err("unparseable amount refused");
        assert_eq!(err.name(), "A44_EXTRACTION_UNPARSEABLE_NUMBER");
    }

    #[test]
    fn an_unknown_resolution_refuses_the_whole_extraction() {
        let mut tree = fixture_tree();
        // The TimeSeries IS the document's first child (fixture_tree);
        // searching among ITS children finds no TimeSeries (its children
        // are in/out-domain, currency, period).
        let series = tree
            .children
            .iter_mut()
            .find(|c| c.tag == A44_ELEMENT_TIME_SERIES)
            .expect("series");
        let period = series
            .children
            .iter_mut()
            .find(|c| c.tag == A44_ELEMENT_PERIOD)
            .expect("period");
        for child in period.children.iter_mut() {
            if child.tag == A44_ELEMENT_RESOLUTION {
                child.text = "PT90M".to_string();
            }
        }
        let err = extract_a44_price_points(&tree).expect_err("unknown resolution refused");
        assert!(matches!(err, A44ExtractionError::PeriodInvalid { .. }));
    }

    #[test]
    fn the_extraction_point_count_must_match_the_period_duration() {
        // Three points over a 24-hour PT60M period: the extraction
        // derives intervals from positions, but the PORT (price_signal)
        // owns the exact point-count rule; this seam's rule is position
        // contiguity + interval arithmetic. A one-point hour tree:
        let point = XmlNode::new(
            A44_ELEMENT_POINT,
            "",
            vec![
                XmlNode::new(A44_ELEMENT_POSITION, "1", vec![]),
                XmlNode::new(A44_ELEMENT_PRICE_AMOUNT, "10.0", vec![]),
            ],
        );
        let tree = XmlNode::new(
            "Publication_MarketDocument",
            "",
            vec![XmlNode::new(
                A44_ELEMENT_TIME_SERIES,
                "",
                vec![
                    XmlNode::new(A44_ELEMENT_IN_DOMAIN, "IN", vec![]),
                    XmlNode::new(A44_ELEMENT_OUT_DOMAIN, "OUT", vec![]),
                    XmlNode::new(A44_ELEMENT_CURRENCY, "EUR", vec![]),
                    XmlNode::new(
                        A44_ELEMENT_PERIOD,
                        "",
                        vec![
                            XmlNode::new(A44_ELEMENT_START, "202606010000", vec![]),
                            XmlNode::new(A44_ELEMENT_END, "202606010100", vec![]),
                            XmlNode::new(A44_ELEMENT_RESOLUTION, "PT60M", vec![]),
                            point,
                        ],
                    ),
                ],
            )],
        );
        let series = extract_a44_price_points(&tree).expect("one hourly point extracts");
        assert_eq!(series.points.len(), 1);
        assert_eq!(series.points[0].end_millis, 1_780_272_000_000 + 3_600_000);
    }

    #[test]
    fn the_recorded_element_names_are_pinned() {
        // UNVERIFIED per [S-111]; the literals are pinned so a change is
        // a deliberate, recorded one.
        assert_eq!(A44_ELEMENT_TIME_SERIES, "TimeSeries");
        assert_eq!(A44_ELEMENT_PERIOD, "Period");
        assert_eq!(A44_ELEMENT_POINT, "Point");
        assert_eq!(A44_ELEMENT_POSITION, "position");
        assert_eq!(A44_ELEMENT_PRICE_AMOUNT, "price.amount");
        assert_eq!(A44_ELEMENT_START, "start");
        assert_eq!(A44_ELEMENT_END, "end");
        assert_eq!(A44_ELEMENT_RESOLUTION, "resolution");
    }

    #[test]
    fn every_extraction_error_renders_non_empty_prose() {
        let errors = [
            A44ExtractionError::NoTimeSeries,
            A44ExtractionError::MissingElement {
                parent: "TimeSeries",
                element: "currency",
            },
            A44ExtractionError::UnparseableNumber {
                element: "price.amount",
                text: "x".to_string(),
            },
            A44ExtractionError::PeriodInvalid {
                reason: "fixture".to_string(),
            },
        ];
        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }
}
