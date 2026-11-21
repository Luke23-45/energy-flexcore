// FIM-F05.02 / FIM-F05.06 — the pinned Sparkplug B byte-fixture suite
// (ledger docs/releases/implementation_ledger/ledger_F05.md).
//
// **WRITTEN-UNRUN.** These tests have never been compiled or executed —
// the no-build/no-test bans (program conventions §1) make execution a
// HUMAN GATE. Nothing here is a conformance result: Sparkplug B
// conformity against a real consumer is a HUMAN LAB GATE (crate
// conventions §7.3).
//
// FIXTURE METHOD: every byte vector below was computed OFFLINE with
// `python -I` over a documented MANUAL protobuf encoding (the two public
// protobuf wire rules: a tag varint `(field_number << 3) | wire_type`,
// and the varint / length-delimited / fixed64 wire types), using the
// field mapping pinned in `src/sparkplug_wire.rs`.
//
// FIXTURE RECOMPUTATION (FIM2-R1-007, 2026-11-21): the mapping was
// VERIFIED against the PRIMARY SOURCES and every byte vector below was
// RECOMPUTED with `python -I` over the SAME documented manual encoding —
// the recomputation script encodes exactly the primitives above (tag
// varint, varint, length-delimited, fixed64) with the verified numbers
// (payload: timestamp=1, metrics=2, seq=3, uuid=4, body=5; metric:
// name=1, alias=2, datatype=4, int_value=10, long_value=11,
// double_value=13, boolean_value=14, string_value=15; type codes:
// Boolean=11, Int32=3, Int64=4, Double=10, String=12; the bdSeq
// correlation metric named `bdSeq` datatype Int64 in NBIRTH/NDEATH only)
// and its hex output is transcribed verbatim into the vectors below.
// The previously pinned DECLARED table (payload bdSeq=5; metric type=3,
// boolean=6/long=7/double=8/string=9; codes 1/4/5/7/8) was WRONG against
// the primary text — see `src/sparkplug_wire.rs`'s module header for the
// full verified table and its sources. NO protobuf library, no prost, no
// codegen, no downloads.

use flexcore_mqtt_adapter::model::{
    BdSeq, MetricType, MetricValue, SparkplugMessageKind, SparkplugMetric, SparkplugPayload,
    SparkplugSeq,
};
use flexcore_mqtt_adapter::sparkplug_wire::{
    decode_payload, encode_payload, PayloadEncodingContext,
};

fn unhex(hex: &str) -> Vec<u8> {
    hex.split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).expect("valid hex byte"))
        .collect()
}

fn metric(
    name: &str,
    alias: Option<u64>,
    metric_type: MetricType,
    value: MetricValue,
) -> SparkplugMetric {
    SparkplugMetric::new(name.to_string(), alias, metric_type, value).expect("valid metric")
}

#[test]
fn nbirth_pins_the_verified_double_inventory() {
    let payload = SparkplugPayload {
        kind: SparkplugMessageKind::NodeBirth,
        seq: Some(SparkplugSeq::new(0).expect("valid")),
        bd_seq: BdSeq::initial(),
        metrics: vec![metric(
            "inverter-1.p.mw",
            None,
            MetricType::Double,
            MetricValue::Double(3.5),
        )],
    };
    let encoded = encode_payload(
        &payload,
        &PayloadEncodingContext {
            timestamp_millis: Some(1_700_000_000_000),
            uuid: None,
        },
    )
    .expect("encodable");
    // datatype Double=10 rides datatype field 4 (tag 0x20); the value
    // rides double_value field 13 (tag 0x69); the bdSeq correlation
    // metric (datatype Int64=4, long_value field 11 — tag 0x58) is the
    // NodeBirth requirement.
    assert_eq!(
        encoded,
        unhex(
            "08 80 D0 95 FF BC 31 \
             12 1C 0A 0F 69 6E 76 65 72 74 65 72 2D 31 2E 70 2E 6D 77 20 0A \
             69 00 00 00 00 00 00 0C 40 \
             12 0B 0A 05 62 64 53 65 71 20 04 58 00 \
             18 00"
        )
    );
    // Round-trip through the decoder (the command path's reader). The
    // bdSeq correlation metric is extracted into the session surface and
    // out of the metrics list.
    let decoded = decode_payload(&encoded).expect("decodable");
    assert_eq!(decoded.timestamp_millis, Some(1_700_000_000_000));
    assert_eq!(decoded.seq, Some(SparkplugSeq::new(0).expect("valid")));
    assert_eq!(decoded.bd_seq, Some(BdSeq::initial()));
    assert_eq!(decoded.metrics.len(), 1);
    assert_eq!(decoded.metrics[0].value, MetricValue::Double(3.5));
}

#[test]
fn ndeath_pins_the_lwt_payload_shape() {
    // The DEATH payload carries ONLY the bdSeq correlation metric (the
    // specification's death-certificate shape: a metric named `bdSeq`,
    // datatype Int64, riding long_value) — these exact bytes are what
    // the CONNECT will carries (the LWT the broker publishes on an
    // unclean disconnect; FIM-F05.04). The previously pinned "28 00"
    // (a payload-level bdSeq field 5 varint) was WRONG against the
    // verified proto — field 5 is `body` (bytes), not bdSeq.
    let payload = SparkplugPayload {
        kind: SparkplugMessageKind::NodeDeath,
        seq: None,
        bd_seq: BdSeq::initial(),
        metrics: Vec::new(),
    };
    let encoded =
        encode_payload(&payload, &PayloadEncodingContext::default()).expect("encodable");
    assert_eq!(encoded, unhex("12 0B 0A 05 62 64 53 65 71 20 04 58 00"));
    // The decoder reads the death certificate back into the session
    // surface.
    let decoded = decode_payload(&encoded).expect("decodable");
    assert_eq!(decoded.bd_seq, Some(BdSeq::initial()));
    assert!(decoded.metrics.is_empty());
    assert_eq!(decoded.seq, None);
}

#[test]
fn dbirth_pins_boolean_int32_int64_and_string_value_encodings() {
    let payload = SparkplugPayload {
        kind: SparkplugMessageKind::DeviceBirth,
        seq: Some(SparkplugSeq::new(1).expect("valid")),
        bd_seq: BdSeq::initial(),
        metrics: vec![
            metric(
                "inverter-1.available.available",
                None,
                MetricType::Boolean,
                MetricValue::Boolean(true),
            ),
            metric(
                "meter-1.e.mwh",
                None,
                MetricType::Int32,
                MetricValue::Int(-42),
            ),
            metric(
                "meter-1.int64",
                None,
                MetricType::Int64,
                MetricValue::Int(-1),
            ),
            metric(
                "status",
                None,
                MetricType::String,
                MetricValue::Text("ok".to_string()),
            ),
        ],
    };
    let encoded =
        encode_payload(&payload, &PayloadEncodingContext::default()).expect("encodable");
    // Verified storage: Int32 rides int_value (field 10, tag 0x50) as the
    // uint32 two's-complement (a 5-byte varint); Int64 rides long_value
    // (field 11, tag 0x58) as the 10-byte varint; Boolean=11 rides
    // boolean_value (field 14, tag 0x70); String=12 rides string_value
    // (field 15, tag 0x7A). A DeviceBirth carries NO bdSeq metric.
    assert_eq!(
        encoded,
        unhex(
            "12 24 0A 1E 69 6E 76 65 72 74 65 72 2D 31 2E 61 76 61 69 6C 61 62 6C 65 2E 61 \
             76 61 69 6C 61 62 6C 65 20 0B 70 01 \
             12 17 0A 0D 6D 65 74 65 72 2D 31 2E 65 2E 6D 77 68 20 03 50 D6 FF FF FF 0F \
             12 1C 0A 0D 6D 65 74 65 72 2D 31 2E 69 6E 74 36 34 20 04 58 FF FF FF FF FF \
             FF FF FF FF 01 \
             12 0E 0A 06 73 74 61 74 75 73 20 0C 7A 02 6F 6B \
             18 01"
        )
    );
    // The canonical MWh value round-trips (canonical units only — the
    // frozen rule travels the wire unchanged, no display conversion).
    let decoded = decode_payload(&encoded).expect("decodable");
    assert_eq!(decoded.metrics[1].value, MetricValue::Int(-42));
    assert_eq!(decoded.metrics[2].value, MetricValue::Int(-1));
    assert_eq!(decoded.metrics[3].value, MetricValue::Text("ok".to_string()));
    // No bdSeq correlation metric on a device message.
    assert_eq!(decoded.bd_seq, None);
}

#[test]
fn ndata_pins_the_soc_fraction_and_alias() {
    // The canonical SOC fraction (0.82) travels the wire unchanged (no
    // percent detour — the frozen rule), carrying the declared alias 7.
    // A NodeData carries NO bdSeq metric (the specification requires it
    // in NBIRTH/NDEATH only).
    let payload = SparkplugPayload {
        kind: SparkplugMessageKind::NodeData,
        seq: Some(SparkplugSeq::new(1).expect("valid")),
        bd_seq: BdSeq::initial(),
        metrics: vec![metric(
            "battery-1.soc.soc_fraction",
            Some(7),
            MetricType::Double,
            MetricValue::Double(0.82),
        )],
    };
    let encoded = encode_payload(
        &payload,
        &PayloadEncodingContext {
            timestamp_millis: Some(1_700_000_001_000),
            uuid: None,
        },
    )
    .expect("encodable");
    assert_eq!(
        encoded,
        unhex(
            "08 E8 D7 95 FF BC 31 \
             12 29 0A 1A 62 61 74 74 65 72 79 2D 31 2E 73 6F 63 2E 73 6F 63 5F 66 72 61 63 \
             74 69 6F 6E 10 07 20 0A \
             69 3D 0A D7 A3 70 3D EA 3F \
             18 01"
        )
    );
}

#[test]
fn a_birth_without_seq_is_refused_and_decoding_refuses_out_of_range_seq() {
    // The model's own validate() rule: BIRTH/DATA require seq — the wire
    // encoder refuses (never encodes a malformed payload).
    let payload = SparkplugPayload {
        kind: SparkplugMessageKind::NodeBirth,
        seq: None,
        bd_seq: BdSeq::initial(),
        metrics: Vec::new(),
    };
    assert!(encode_payload(&payload, &PayloadEncodingContext::default()).is_err());
    // A decoded seq above the declared modulus (256) is a named refusal
    // (field 3 varint 0x80 0x02 = 256).
    let over = decode_payload(&unhex("18 80 02"));
    assert!(over.is_err());
    // The in-range boundary (255 = 0xFF 0x01) decodes fine.
    let at_boundary = decode_payload(&unhex("18 FF 01")).expect("255 is in range");
    assert_eq!(at_boundary.seq, Some(SparkplugSeq::new(255).expect("valid")));
}
