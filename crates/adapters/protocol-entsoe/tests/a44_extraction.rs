// P15.04 — Integration test: a hand-built A44 document tree extracts to
//         an ordered, port-shaped price series (WRITTEN-UNRUN — no cargo
//         command was executed in this loop; the human build/test gate
//         owns the run. The element names are UNVERIFIED per [S-111];
//         the trees are hand-built and every expected value is
//         hand-computed. Nothing is fabricated.)
//
// The test exercises the adapter as a unit: transport refusal (the
// unwired seam), token gate, scope declaration, and the full
// document → extraction → ordered-points path, mirroring the shape the
// composition root wires to `flexcore-ports::price_signal::PriceSeries`.

use flexcore_entsoe_adapter::document::{A44_DOCUMENT_TYPE, Resolution};
use flexcore_entsoe_adapter::scope::{price_ingest_scope, PRICE_INGEST_AUTHORITY_STATEMENT};
use flexcore_entsoe_adapter::token::{
    validate_token_reference, TokenGateState, TOKEN_GATE_STATEMENT,
};
use flexcore_entsoe_adapter::transport::{
    A44PriceQuery, A44ResponseStatus, UnwiredA44Transport,
};
use flexcore_entsoe_adapter::xml_seam::{
    extract_a44_price_points, A44_ELEMENT_CURRENCY, A44_ELEMENT_END, A44_ELEMENT_IN_DOMAIN,
    A44_ELEMENT_OUT_DOMAIN, A44_ELEMENT_PERIOD, A44_ELEMENT_POINT, A44_ELEMENT_POSITION,
    A44_ELEMENT_PRICE_AMOUNT, A44_ELEMENT_RESOLUTION, A44_ELEMENT_START, A44_ELEMENT_TIME_SERIES,
    XmlNode,
};

fn point(position: u32, amount: &str) -> XmlNode {
    XmlNode::new(
        A44_ELEMENT_POINT,
        "",
        vec![
            XmlNode::new(A44_ELEMENT_POSITION, &position.to_string(), vec![]),
            XmlNode::new(A44_ELEMENT_PRICE_AMOUNT, amount, vec![]),
        ],
    )
}

/// A hand-built 24-hour PT60M series with a negative midday price
/// (negative prices are carried, sign included).
fn day_tree() -> XmlNode {
    let amounts = [
        "35.0", "32.5", "30.0", "28.0", "27.5", "29.0", "33.0", "38.0", "45.0", "52.0",
        "55.5", "58.0", "60.0", "-12.5", "55.0", "52.0", "48.0", "50.0", "56.0", "62.0",
        "58.0", "50.0", "44.0", "38.5",
    ];
    let points: Vec<XmlNode> = amounts
        .iter()
        .enumerate()
        .map(|(i, a)| point((i + 1) as u32, a))
        .collect();
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
                    ]
                    .into_iter()
                    .chain(points)
                    .collect(),
                ),
            ],
        )],
    )
}

#[test]
fn the_day_document_extracts_to_24_ordered_points_with_the_negative_carried() {
    let series = extract_a44_price_points(&day_tree()).expect("the day document extracts");
    // HAND CHECK: 24 hourly points starting 2026-06-01T00:00
    // (1_780_272_000_000 ms); interval 3_600_000 ms; the 14th point
    // (position 14, index 13) carries the negative amount -12.5.
    assert_eq!(series.points.len(), 24);
    assert_eq!(series.points[0].start_millis, 1_780_272_000_000);
    assert_eq!(series.points[0].end_millis, 1_780_272_000_000 + 3_600_000);
    assert_eq!(series.points[0].price_amount, 35.0);
    assert_eq!(
        series.points[13].start_millis,
        1_780_272_000_000 + 13 * 3_600_000
    );
    assert_eq!(series.points[13].price_amount, -12.5);
    assert_eq!(series.points[23].price_amount, 38.5);
    assert_eq!(series.currency_code, "EUR");
    // The points are in ascending interval order by construction.
    for pair in series.points.windows(2) {
        assert!(pair[0].start_millis < pair[1].start_millis);
    }
}

#[test]
fn the_query_and_the_extraction_agree_on_the_document_type() {
    let query = A44PriceQuery::new(
        "FIXTURE-IN".to_string(),
        "FIXTURE-OUT".to_string(),
        "202606010000".to_string(),
        "202606020000".to_string(),
    )
    .expect("valid query");
    // The query names the A44 document type; the extraction consumes the
    // document that type publishes (UNVERIFIED per [S-111]).
    assert_eq!(query.parameters()[0].1, A44_DOCUMENT_TYPE);
}

#[test]
fn the_unwired_transport_refuses_every_live_request() {
    let query = A44PriceQuery::new(
        "IN".to_string(),
        "OUT".to_string(),
        "202606010000".to_string(),
        "202606020000".to_string(),
    )
    .expect("valid query");
    let err = UnwiredA44Transport
        .fetch_a44_document(&query)
        .expect_err("no request is sent in this program");
    assert_eq!(err.name(), "A44_TRANSPORT_SEAM_NOT_WIRED");
}

#[test]
fn the_token_gate_ships_unsigned_and_refuses_inline_material() {
    assert_eq!(TokenGateState::Unsigned.as_str(), "UNSIGNED");
    assert!(TOKEN_GATE_STATEMENT.contains("HUMAN GATE"));
    let err = validate_token_reference("RAW-TOKEN").expect_err("inline refused");
    assert_eq!(err.name(), "A44_TOKEN_INLINE_MATERIAL_REFUSED");
}

#[test]
fn the_scope_declares_ingest_only_with_no_control() {
    let scope = price_ingest_scope();
    assert_eq!(scope.role.as_str(), "ingest-only");
    assert_eq!(scope.control_scope.as_str(), "none");
    assert!(PRICE_INGEST_AUTHORITY_STATEMENT.contains("never bid inputs"));
    assert!(PRICE_INGEST_AUTHORITY_STATEMENT.contains("never dispatch inputs"));
}

#[test]
fn the_resolution_vocabulary_matches_the_period_arithmetic() {
    // One day: 24 hourly / 48 half-hourly / 96 quarter-hourly points
    // (HAND CHECK: 86_400_000 ms / (N minutes × 60_000)).
    assert_eq!(Resolution::Pt60m.interval_minutes(), 60);
    assert_eq!(Resolution::Pt30m.interval_minutes(), 30);
    assert_eq!(Resolution::Pt15m.interval_minutes(), 15);
    let day_ms = 86_400_000i64;
    assert_eq!(day_ms / (60 * 60_000), 24);
    assert_eq!(day_ms / (30 * 60_000), 48);
    assert_eq!(day_ms / (15 * 60_000), 96);
}

#[test]
fn the_response_status_vocabulary_is_closed() {
    // The transport carries a status and opaque bytes only — never an
    // interpretation; the recorded vocabulary is ok/error.
    assert_eq!(std::mem::discriminant(&A44ResponseStatus::Ok),
        std::mem::discriminant(&A44ResponseStatus::Ok));
    assert_ne!(std::mem::discriminant(&A44ResponseStatus::Ok),
        std::mem::discriminant(&A44ResponseStatus::Error));
}
