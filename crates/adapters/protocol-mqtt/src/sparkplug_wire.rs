// FIM-F05.02 — the hand-written minimal Sparkplug B protobuf wire encoder
// (and symmetric decoder) for the payload subset the existing `model.rs`
// declares (docs/releases/final-implementation-before-publication.md §5
// F-05; ledger docs/releases/implementation_ledger/ledger_F05.md; FIM
// standing decision 1 — the Sparkplug B protobuf payload is a HAND-WRITTEN
// minimal codec, NOT prost/codegen and NOT a new dependency).
//
// **Implemented under FIM-F05, WRITTEN-UNRUN: this module has never been
// compiled or executed. Sparkplug B conformity against a real broker /
// consumer is UNPROVEN and remains a HUMAN LAB GATE** (crate conventions
// §7.3). The byte fixtures that pin the encoding live in
// `tests/sparkplug_wire_fixtures.rs` and in this module's test block; they
// were computed OFFLINE with `python -I` over a documented MANUAL protobuf
// encoding (the wire rules are the two public protobuf primitives: a
// tag/field-key varint `(field_number << 3) | wire_type`, and protobuf's
// varint / length-delimited / fixed64 wire types). No protobuf library is
// used anywhere — no `.proto` file, no codegen, no runtime.
//
// FIXTURE RECOMPUTATION (FIM2-R1-007, 2026-11-21): the field mapping below
// was VERIFIED against the primary text (see the register item that
// follows) and EVERY pinned fixture was RECOMPUTED with `python -I` over
// the SAME documented manual encoding — the recomputation script encodes
// exactly the primitives above (tag varint, varint, length-delimited,
// fixed64) with the verified numbers and its hex output is transcribed
// verbatim into the fixtures. The original fixture method (offline manual
// encoding, no protobuf library, no downloads, hand-transcribed output) is
// unchanged; only the table it used changed, in one recorded edit.
//
// THE VERIFIED FIELD MAPPING (module constants below): previously every
// field number and metric-type code was an UNVERIFIED declared design pin.
// On 2026-11-21 the mapping was checked against the PRIMARY SOURCES and
// corrected in this one recorded edit (the model.rs honesty discipline: a
// mismatch is a recorded edit, never a silent reinterpretation):
//
// - **The Eclipse Tahu `sparkplug_b.proto`** — the reference
//   implementation's protobuf definition, retrieved 2026-11-21 from
//   https://raw.githubusercontent.com/eclipse/tahu/master/sparkplug_b/
//   sparkplug_b.proto (Eclipse Tahu, EPL-2.0):
//   - Payload: `timestamp = 1` (uint64), `repeated Metric metrics = 2`,
//     `seq = 3` (uint64), `uuid = 4` (string), `body = 5` (bytes),
//     `extensions 6 to max`. There is NO payload-level bdSeq field — the
//     previously declared "payload field 5 = bdSeq" collided with `body`.
//   - Metric: `name = 1`, `alias = 2`, `timestamp = 3` (uint64; OUTSIDE
//     the declared subset — skipped by wire type), `datatype = 4`,
//     `is_historical = 5`, `is_transient = 6`, `is_null = 7`,
//     `metadata = 8`, `properties = 9`, then a `oneof value` group:
//     `int_value = 10` (uint32), `long_value = 11` (uint64),
//     `float_value = 12` (float), `double_value = 13` (double),
//     `boolean_value = 14` (bool), `string_value = 15` (string),
//     `bytes_value = 16`, `dataset_value = 17`, `template_value = 18`,
//     `extension_value = 19`. The previously declared value fields
//     (boolean=6 / long=7 / double=8 / string=9) collided with
//     is_transient / is_null / metadata / properties, and the declared
//     `type = 3` collided with the metric timestamp.
//   - `DataType` enum: Boolean = 11, Int32 = 3, Int64 = 4, Double = 10,
//     String = 12. The previously declared codes were wrong against this
//     enum: declared Boolean=1 (1 is Int8), Int32=4 (4 is Int64),
//     Int64=5 (5 is UInt8), Double=7 (7 is UInt32), String=8 (8 is
//     UInt64).
// - **The Sparkplug B specification** — retrieved 2026-11-21 from the
//   Eclipse Sparkplug project's specification chapters
//   (`Sparkplug_6_Payloads.adoc` and `Sparkplug_5_Operational_Behavior.adoc`
//   in github.com/eclipse-sparkplug/sparkplug):
//   - the storage rule: Int8/Int16/Int32/UInt8/UInt16/UInt32 ride
//     `int_value` (uint32); Int64/UInt64/DateTime ride `long_value`
//     (uint64) — so the declared subset's Int32 does NOT share the long
//     field with Int64 (the previous declared reading was wrong);
//   - the bdSeq mechanism: the Will Message payload (NDEATH) "MUST
//     include a metric with the name of 'bdSeq', the datatype of INT64"
//     and the NBIRTH "MUST include a metric with the name of 'bdSeq' the
//     datatype of INT64" — bdSeq is a METRIC named `bdSeq` (datatype
//     Int64, riding long_value), required in NBIRTH and NDEATH ONLY; no
//     bdSeq requirement exists for NDATA/DBIRTH/DDATA/DDEATH. The encoder
//     below emits it exactly there and nowhere else; the decoder extracts
//     it into the session surface by name and refuses a bdSeq-named
//     metric that does not carry the declared Int64 value.
//
// What the codec covers (the subset `model.rs` declares):
//
// - Payload: `timestamp` (unix millis), repeated `metrics`, `seq`, `uuid`,
//   and the bdSeq correlation METRIC (NBIRTH/NDEATH only — the
//   specification's mechanism; the model's `bd_seq` field supplies the
//   value). The encoder emits ONLY the fields the model declares; each
//   present field is written EXPLICITLY (including zero values — proto3's
//   default-omission rule is a re-serialization rule and every protobuf
//   parser accepts explicit zeros; the explicit encoding is the declared
//   deterministic output that the fixtures pin).
// - Metrics: `name`, `alias`, `datatype`, and exactly one typed value
//   field (Int32 → int_value / Int64 → long_value / double / boolean /
//   string — the verified storage rule).
// - A symmetric DECODER (used by the command path in `live.rs`) that
//   skips unknown fields BY WIRE TYPE (forward compatibility) and refuses
//   type/value disagreement by name.
//
// NOT a conformance claim: nothing here is Sparkplug B certification-style
// evidence; the pinned fixtures pin THIS table as verified against the
// primary text, and real-peer interop stays the human lab gate.

use crate::model::{
    BdSeq, MetricType, MetricValue, ModelError, SparkplugMessageKind, SparkplugMetric,
    SparkplugPayload, SparkplugSeq,
};

// --- The verified payload field mapping (see the module header register) -------

/// Payload field 1: timestamp (unix milliseconds; uint64 varint). VERIFIED —
/// the Tahu `sparkplug_b.proto` (see the module header for the source and
/// retrieval date).
pub const PAYLOAD_FIELD_TIMESTAMP: u8 = 1;
/// Payload field 2: repeated metric (length-delimited submessage).
/// VERIFIED (same source).
pub const PAYLOAD_FIELD_METRICS: u8 = 2;
/// Payload field 3: message sequence (uint64 varint). VERIFIED (same source).
pub const PAYLOAD_FIELD_SEQ: u8 = 3;
/// Payload field 4: uuid (length-delimited UTF-8). VERIFIED (same source).
pub const PAYLOAD_FIELD_UUID: u8 = 4;

/// The name of the birth/death correlation metric. VERIFIED — the
/// specification requires a METRIC named `bdSeq` (datatype Int64) in NBIRTH
/// and NDEATH payloads; there is NO payload-level bdSeq field in the
/// verified proto (payload field 5 is `body`, optional bytes — outside the
/// declared subset and skipped by wire type on decode).
pub const BD_SEQ_METRIC_NAME: &str = "bdSeq";

/// Metric field 1: name (length-delimited UTF-8). VERIFIED (same source).
pub const METRIC_FIELD_NAME: u8 = 1;
/// Metric field 2: alias (uint64 varint). VERIFIED (same source).
pub const METRIC_FIELD_ALIAS: u8 = 2;
/// Metric field 4: datatype code (uint32 varint). VERIFIED (same source) —
/// the previously declared table pinned this field at 3, which is the
/// metric's own `timestamp` field in the verified text.
pub const METRIC_FIELD_DATATYPE: u8 = 4;
/// Metric field 10: `int_value` (uint32 varint). VERIFIED (same source) —
/// the storage rule for Int8/Int16/Int32/UInt8/UInt16/UInt32; the declared
/// subset's Int32 rides here.
pub const METRIC_FIELD_INT_VALUE: u8 = 10;
/// Metric field 11: `long_value` (uint64 varint; the two's-complement
/// uint64 encoding). VERIFIED (same source) — the storage rule for
/// Int64/UInt64/DateTime; the declared subset's Int64 rides here.
pub const METRIC_FIELD_LONG_VALUE: u8 = 11;
/// Metric field 13: `double_value` (fixed 64-bit, little-endian). VERIFIED
/// (same source) — the previously declared table pinned 8 (`metadata`).
pub const METRIC_FIELD_DOUBLE_VALUE: u8 = 13;
/// Metric field 14: `boolean_value` (varint). VERIFIED (same source) — the
/// previously declared table pinned 6 (`is_transient`).
pub const METRIC_FIELD_BOOLEAN_VALUE: u8 = 14;
/// Metric field 15: `string_value` (length-delimited UTF-8). VERIFIED (same
/// source) — the previously declared table pinned 9 (`properties`).
pub const METRIC_FIELD_STRING_VALUE: u8 = 15;

/// Verified metric type codes (the proto's `DataType` enum — see the
/// module header for the source and retrieval date; the previously
/// UNVERIFIED declared codes 1/4/5/7/8 were replaced by these).
pub const METRIC_TYPE_BOOLEAN: u32 = 11;
pub const METRIC_TYPE_INT32: u32 = 3;
pub const METRIC_TYPE_INT64: u32 = 4;
pub const METRIC_TYPE_DOUBLE: u32 = 10;
pub const METRIC_TYPE_STRING: u32 = 12;

/// Wire types (the public protobuf primitives).
const WIRE_VARINT: u8 = 0;
const WIRE_FIXED64: u8 = 1;
const WIRE_LENGTH_DELIMITED: u8 = 2;
const WIRE_FIXED32: u8 = 5;

/// Errors the Sparkplug wire codec reports (named, never silent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SparkplugWireError {
    /// A field violates the declared encoding rules.
    Malformed {
        reason: String,
    },
    /// The declared field mapping's type/value agreement was violated.
    TypeValueMismatch {
        reason: String,
    },
    /// A decoded sequence number exceeded the declared modulus.
    SequenceOutOfRange {
        value: u64,
    },
}

impl std::fmt::Display for SparkplugWireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SparkplugWireError::Malformed { reason } => {
                write!(f, "malformed Sparkplug B payload: {reason}")
            }
            SparkplugWireError::TypeValueMismatch { reason } => {
                write!(f, "Sparkplug metric type/value disagreement: {reason}")
            }
            SparkplugWireError::SequenceOutOfRange { value } => {
                write!(f, "decoded sequence value {value} exceeds the declared modulus")
            }
        }
    }
}

impl std::error::Error for SparkplugWireError {}

impl From<ModelError> for SparkplugWireError {
    fn from(error: ModelError) -> Self {
        SparkplugWireError::Malformed {
            reason: error.to_string(),
        }
    }
}

// --- Protobuf primitives (hand-written; the two public wire rules) -------------

/// Encodes a protobuf varint (base-128 little-endian groups; the same
/// rule family as the MQTT remaining-length integer but unbounded).
fn pb_varint(mut value: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(10);
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            return out;
        }
    }
}

/// Encodes the tag/field-key varint: `(field_number << 3) | wire_type`.
fn pb_tag(field: u8, wire_type: u8) -> Vec<u8> {
    pb_varint(((field as u64) << 3) | wire_type as u64)
}

/// Encodes a length-delimited field.
fn pb_len_delimited(field: u8, bytes: &[u8]) -> Vec<u8> {
    let mut out = pb_tag(field, WIRE_LENGTH_DELIMITED);
    out.extend_from_slice(&pb_varint(bytes.len() as u64));
    out.extend_from_slice(bytes);
    out
}

/// Encodes a varint field.
fn pb_varint_field(field: u8, value: u64) -> Vec<u8> {
    let mut out = pb_tag(field, WIRE_VARINT);
    out.extend_from_slice(&pb_varint(value));
    out
}

/// Encodes a fixed-64-bit field (little-endian).
fn pb_fixed64_field(field: u8, bytes: [u8; 8]) -> Vec<u8> {
    let mut out = pb_tag(field, WIRE_FIXED64);
    out.extend_from_slice(&bytes);
    out
}

// --- Metric type code mapping ---------------------------------------------------

/// The wire type code for a declared [`MetricType`].
pub const fn metric_type_code(metric_type: MetricType) -> u32 {
    match metric_type {
        MetricType::Boolean => METRIC_TYPE_BOOLEAN,
        MetricType::Int32 => METRIC_TYPE_INT32,
        MetricType::Int64 => METRIC_TYPE_INT64,
        MetricType::Double => METRIC_TYPE_DOUBLE,
        MetricType::String => METRIC_TYPE_STRING,
    }
}

/// The declared metric type for a wire type code (the inverse map; an
/// unknown code is a named refusal).
fn metric_type_for_code(code: u32) -> Option<MetricType> {
    let metric_type = match code {
        METRIC_TYPE_BOOLEAN => MetricType::Boolean,
        METRIC_TYPE_INT32 => MetricType::Int32,
        METRIC_TYPE_INT64 => MetricType::Int64,
        METRIC_TYPE_DOUBLE => MetricType::Double,
        METRIC_TYPE_STRING => MetricType::String,
        _ => return None,
    };
    Some(metric_type)
}

// --- Encoding -------------------------------------------------------------------

/// Encodes one metric submessage: name, alias (when declared), datatype,
/// and exactly one typed value field. The metric is re-validated
/// defensively (`model.rs` already enforces type/value agreement at
/// construction; the wire layer refuses rather than trusts).
fn encode_metric(metric: &SparkplugMetric) -> Result<Vec<u8>, SparkplugWireError> {
    if metric.name.trim().is_empty() {
        return Err(SparkplugWireError::Malformed {
            reason: "metric name is empty".to_string(),
        });
    }
    let mut out = pb_len_delimited(METRIC_FIELD_NAME, metric.name.as_bytes());
    if let Some(alias) = metric.alias {
        out.extend(pb_varint_field(METRIC_FIELD_ALIAS, alias));
    }
    out.extend(pb_varint_field(
        METRIC_FIELD_DATATYPE,
        u64::from(metric_type_code(metric.metric_type)),
    ));
    match &metric.value {
        MetricValue::Boolean(value) if metric.metric_type == MetricType::Boolean => {
            out.extend(pb_varint_field(METRIC_FIELD_BOOLEAN_VALUE, u64::from(*value)));
        }
        MetricValue::Int(value) if metric.metric_type == MetricType::Int32 => {
            // The verified storage rule: Int32 rides `int_value` (field
            // 10, uint32) — the low 32 bits two's-complement (a negative
            // value round-trips through the uint32 re-interpretation).
            out.extend(pb_varint_field(
                METRIC_FIELD_INT_VALUE,
                u64::from(*value as u32),
            ));
        }
        MetricValue::Int(value) if metric.metric_type == MetricType::Int64 => {
            // Int64 rides `long_value` (field 11, uint64;
            // two's-complement — a negative value sign-extends into the
            // 10-byte varint).
            out.extend(pb_varint_field(METRIC_FIELD_LONG_VALUE, *value as u64));
        }
        MetricValue::Double(value) if metric.metric_type == MetricType::Double => {
            out.extend(pb_fixed64_field(
                METRIC_FIELD_DOUBLE_VALUE,
                value.to_bits().to_le_bytes(),
            ));
        }
        MetricValue::Text(value) if metric.metric_type == MetricType::String => {
            out.extend(pb_len_delimited(METRIC_FIELD_STRING_VALUE, value.as_bytes()));
        }
        _ => {
            return Err(SparkplugWireError::TypeValueMismatch {
                reason: format!(
                    "declared {:?} does not match value {:?}",
                    metric.metric_type, metric.value
                ),
            })
        }
    }
    Ok(out)
}

/// The encoding context the caller supplies: the payload-level timestamp
/// (unix milliseconds) and uuid are NOT part of the `model.rs` payload
/// type (it declares kind/seq/bd_seq/metrics only) — the transport seam
/// owns them. `None` omits the field entirely.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PayloadEncodingContext {
    pub timestamp_millis: Option<u64>,
    pub uuid: Option<String>,
}

/// Encodes one Sparkplug payload (the declared subset): timestamp (from
/// the context), the metrics, the bdSeq correlation METRIC (NBIRTH/NDEATH
/// only — the specification's mechanism; there is NO payload-level bdSeq
/// field in the verified proto), `seq` (BIRTH/DATA only — the model's own
/// `validate()` rule), and uuid (from the context).
pub fn encode_payload(
    payload: &SparkplugPayload,
    context: &PayloadEncodingContext,
) -> Result<Vec<u8>, SparkplugWireError> {
    payload.validate()?;
    let mut out = Vec::new();
    if let Some(timestamp) = context.timestamp_millis {
        out.extend(pb_varint_field(PAYLOAD_FIELD_TIMESTAMP, timestamp));
    }
    for metric in &payload.metrics {
        out.extend(pb_len_delimited(PAYLOAD_FIELD_METRICS, &encode_metric(metric)?));
    }
    // The bdSeq correlation metric: the specification requires a metric
    // named `bdSeq` (datatype Int64) in the NBIRTH and NDEATH payloads —
    // and in NO other kind (no bdSeq requirement exists for
    // NDATA/DBIRTH/DDATA/DDEATH; emitting it there would inject the
    // counter into a device's or a DATA message's metric inventory). The
    // model's `bd_seq` field is the session counter; the wire layer
    // publishes it exactly where the specification requires. The value is
    // bit-preserving (`u64 as i64` two's-complement; the verified
    // datatype is Int64 and the value rides `long_value`).
    if matches!(
        payload.kind,
        SparkplugMessageKind::NodeBirth | SparkplugMessageKind::NodeDeath
    ) {
        let bd_seq_metric = SparkplugMetric::new(
            BD_SEQ_METRIC_NAME.to_string(),
            None,
            MetricType::Int64,
            MetricValue::Int(payload.bd_seq.value() as i64),
        )?;
        out.extend(pb_len_delimited(
            PAYLOAD_FIELD_METRICS,
            &encode_metric(&bd_seq_metric)?,
        ));
    }
    if let Some(seq) = payload.seq {
        out.extend(pb_varint_field(PAYLOAD_FIELD_SEQ, seq.value()));
    }
    if let Some(uuid) = &context.uuid {
        out.extend(pb_len_delimited(PAYLOAD_FIELD_UUID, uuid.as_bytes()));
    }
    Ok(out)
}

// --- Decoding (the command path) --------------------------------------------------

/// A bounded reader over a protobuf slice.
struct PbReader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> PbReader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    fn varint(&mut self) -> Result<u64, SparkplugWireError> {
        let mut value: u64 = 0;
        let mut shift: u32 = 0;
        loop {
            if self.pos >= self.buf.len() {
                return Err(SparkplugWireError::Malformed {
                    reason: "varint runs past the end of the buffer".to_string(),
                });
            }
            let byte = self.buf[self.pos];
            self.pos += 1;
            value |= u64::from(byte & 0x7F) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
            shift += 7;
            if shift >= 64 {
                return Err(SparkplugWireError::Malformed {
                    reason: "varint exceeds 64 bits".to_string(),
                });
            }
        }
    }

    fn bytes(&mut self, len: usize) -> Result<&'a [u8], SparkplugWireError> {
        if self.remaining() < len {
            return Err(SparkplugWireError::Malformed {
                reason: format!("length-delimited field needs {len} bytes, {} remain", self.remaining()),
            });
        }
        let slice = &self.buf[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    fn utf8(&mut self, len: usize) -> Result<String, SparkplugWireError> {
        let bytes = self.bytes(len)?;
        String::from_utf8(bytes.to_vec()).map_err(|error| SparkplugWireError::Malformed {
            reason: format!("invalid UTF-8 string field: {error}"),
        })
    }

    /// Reads one tag: `(field_number, wire_type)`. A field number of 0 is
    /// a protocol violation (named refusal).
    fn tag(&mut self) -> Result<(u8, u8), SparkplugWireError> {
        let key = self.varint()?;
        let field = (key >> 3) as u8;
        let wire_type = (key & 0x07) as u8;
        if field == 0 {
            return Err(SparkplugWireError::Malformed {
                reason: "field number 0 is a protocol violation".to_string(),
            });
        }
        Ok((field, wire_type))
    }

    /// Skips one field's value by wire type (the forward-compatibility
    /// rule). Unknown wire types (3/4 groups, 6/7) are named refusals.
    fn skip(&mut self, wire_type: u8) -> Result<(), SparkplugWireError> {
        match wire_type {
            WIRE_VARINT => {
                self.varint()?;
            }
            WIRE_FIXED64 => {
                self.bytes(8)?;
            }
            WIRE_LENGTH_DELIMITED => {
                let len = self.varint()? as usize;
                self.bytes(len)?;
            }
            WIRE_FIXED32 => {
                self.bytes(4)?;
            }
            other => {
                return Err(SparkplugWireError::Malformed {
                    reason: format!("wire type {other} cannot be skipped"),
                })
            }
        }
        Ok(())
    }
}

/// Decodes one metric submessage into the declared model types.
fn decode_metric(bytes: &[u8]) -> Result<SparkplugMetric, SparkplugWireError> {
    let mut reader = PbReader::new(bytes);
    let mut name: Option<String> = None;
    let mut alias: Option<u64> = None;
    let mut type_code: Option<u32> = None;
    let mut boolean_value: Option<bool> = None;
    let mut int_value: Option<i64> = None;
    let mut long_value: Option<i64> = None;
    let mut double_value: Option<f64> = None;
    let mut string_value: Option<String> = None;
    while reader.remaining() > 0 {
        let (field, wire_type) = reader.tag()?;
        match (field, wire_type) {
            (METRIC_FIELD_NAME, WIRE_LENGTH_DELIMITED) => {
                let len = reader.varint()? as usize;
                name = Some(reader.utf8(len)?);
            }
            (METRIC_FIELD_ALIAS, WIRE_VARINT) => {
                alias = Some(reader.varint()?);
            }
            (METRIC_FIELD_DATATYPE, WIRE_VARINT) => {
                type_code = Some(u32::try_from(reader.varint()?).map_err(|error| {
                    SparkplugWireError::Malformed {
                        reason: format!("metric datatype code exceeds u32: {error}"),
                    }
                })?);
            }
            (METRIC_FIELD_BOOLEAN_VALUE, WIRE_VARINT) => {
                boolean_value = Some(reader.varint()? != 0);
            }
            (METRIC_FIELD_INT_VALUE, WIRE_VARINT) => {
                // int_value is uint32 storage (the verified rule); the
                // value re-interprets as the two's-complement i32.
                let raw = reader.varint()?;
                let truncated = u32::try_from(raw).map_err(|error| {
                    SparkplugWireError::Malformed {
                        reason: format!("int_value exceeds the uint32 storage: {error}"),
                    }
                })?;
                int_value = Some(i64::from(truncated as i32));
            }
            (METRIC_FIELD_LONG_VALUE, WIRE_VARINT) => {
                long_value = Some(reader.varint()? as i64);
            }
            (METRIC_FIELD_DOUBLE_VALUE, WIRE_FIXED64) => {
                let bytes = reader.bytes(8)?;
                let mut raw = [0u8; 8];
                raw.copy_from_slice(bytes);
                double_value = Some(f64::from_le_bytes(raw));
            }
            (METRIC_FIELD_STRING_VALUE, WIRE_LENGTH_DELIMITED) => {
                let len = reader.varint()? as usize;
                string_value = Some(reader.utf8(len)?);
            }
            // Unknown fields are skipped BY WIRE TYPE (forward
            // compatibility; a mismatched wire type for a KNOWN field is
            // also skipped — the declared subset re-reads nothing).
            _ => reader.skip(wire_type)?,
        }
    }
    let metric_type = type_code
        .and_then(metric_type_for_code)
        .ok_or_else(|| SparkplugWireError::TypeValueMismatch {
            reason: "metric carries no declared type code".to_string(),
        })?;
    let metric_name = name.ok_or_else(|| SparkplugWireError::TypeValueMismatch {
        reason: "metric carries no name".to_string(),
    })?;
    let value = match (
        metric_type,
        boolean_value,
        int_value,
        long_value,
        double_value,
        string_value,
    ) {
        (MetricType::Boolean, Some(value), ..) => MetricValue::Boolean(value),
        (MetricType::Int32, _, Some(value), ..) => MetricValue::Int(value),
        (MetricType::Int64, _, _, Some(value), ..) => MetricValue::Int(value),
        (MetricType::Double, _, _, _, Some(value), ..) => MetricValue::Double(value),
        (MetricType::String, .., Some(value)) => MetricValue::Text(value),
        (metric_type, ..) => {
            return Err(SparkplugWireError::TypeValueMismatch {
                reason: format!("declared {metric_type:?} carries no matching value field"),
            })
        }
    };
    // The model constructor re-enforces type/value agreement.
    SparkplugMetric::new(metric_name, alias, metric_type, value)
        .map_err(SparkplugWireError::from)
}

/// The decoded payload surface the command path consumes. The message
/// KIND is deliberately NOT decoded here: the kind is a TOPIC property in
/// the declared namespace, not a payload field — the command delivery
/// (`live.rs`) carries the topic alongside. The bdSeq correlation metric
/// is EXTRACTED into `bd_seq` (the specification carries it as a metric
/// named `bdSeq`; the session surface mirrors the model's separation — the
/// counter is not a point metric — and the extracted metric is excluded
/// from `metrics`).
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedPayload {
    pub timestamp_millis: Option<u64>,
    pub uuid: Option<String>,
    pub seq: Option<SparkplugSeq>,
    pub bd_seq: Option<BdSeq>,
    pub metrics: Vec<SparkplugMetric>,
}

/// Decodes a Sparkplug payload (the declared subset; unknown fields are
/// skipped by wire type, unknown metric type codes are named refusals).
/// The bdSeq correlation metric (named [`BD_SEQ_METRIC_NAME`], datatype
/// Int64 riding `long_value`) is extracted into [`DecodedPayload::bd_seq`];
/// a bdSeq-named metric that does not carry the declared Int64 value is a
/// named refusal.
pub fn decode_payload(bytes: &[u8]) -> Result<DecodedPayload, SparkplugWireError> {
    let mut reader = PbReader::new(bytes);
    let mut timestamp_millis: Option<u64> = None;
    let mut uuid: Option<String> = None;
    let mut seq: Option<u64> = None;
    let mut metrics = Vec::new();
    while reader.remaining() > 0 {
        let (field, wire_type) = reader.tag()?;
        match (field, wire_type) {
            (PAYLOAD_FIELD_TIMESTAMP, WIRE_VARINT) => {
                timestamp_millis = Some(reader.varint()?);
            }
            (PAYLOAD_FIELD_METRICS, WIRE_LENGTH_DELIMITED) => {
                let len = reader.varint()? as usize;
                metrics.push(decode_metric(reader.bytes(len)?)?);
            }
            (PAYLOAD_FIELD_SEQ, WIRE_VARINT) => {
                seq = Some(reader.varint()?);
            }
            (PAYLOAD_FIELD_UUID, WIRE_LENGTH_DELIMITED) => {
                let len = reader.varint()? as usize;
                uuid = Some(reader.utf8(len)?);
            }
            _ => reader.skip(wire_type)?,
        }
    }
    let seq = seq
        .map(|value| {
            SparkplugSeq::new(value).map_err(|_| SparkplugWireError::SequenceOutOfRange { value })
        })
        .transpose()?;
    // Extract the bdSeq correlation metric by name (the specification's
    // NBIRTH/NDEATH mechanism). The LAST occurrence wins (a repeated name
    // is not declared; the counter is a single value). The name is
    // reserved for the correlation metric: a bdSeq-named metric that does
    // not carry the declared Int64 value is a named refusal, never a
    // guess.
    let mut bd_seq: Option<BdSeq> = None;
    let mut point_metrics = Vec::with_capacity(metrics.len());
    for metric in metrics {
        if metric.name == BD_SEQ_METRIC_NAME {
            match metric.value {
                MetricValue::Int(value) => {
                    bd_seq = Some(BdSeq::from_raw(value as u64));
                }
                other => {
                    return Err(SparkplugWireError::TypeValueMismatch {
                        reason: format!(
                            "the {BD_SEQ_METRIC_NAME} correlation metric does not carry the \
                             declared Int64 value: {other:?}"
                        ),
                    })
                }
            }
        } else {
            point_metrics.push(metric);
        }
    }
    Ok(DecodedPayload {
        timestamp_millis,
        uuid,
        seq,
        bd_seq,
        metrics: point_metrics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    // The model types (now including SparkplugMessageKind — the encoder
    // is kind-aware for the bdSeq emission only) arrive via super::*.

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
    }

    /// Builds a metric through the model's validating constructor.
    fn metric(
        name: &str,
        alias: Option<u64>,
        metric_type: MetricType,
        value: MetricValue,
    ) -> SparkplugMetric {
        SparkplugMetric::new(name.to_string(), alias, metric_type, value).expect("valid metric")
    }

    #[test]
    fn nbirth_encodes_the_pinned_byte_fixture() {
        // Pinned fixture (recomputed with `python -I` over the verified
        // table — see the module header's FIXTURE RECOMPUTATION register):
        // timestamp 1700000000000, one Double metric "inverter-1.p.mw" =
        // 3.5 (datatype Double=10 riding double_value field 13, bits
        // little-endian), the bdSeq correlation metric (bdSeq 0, datatype
        // Int64=4 riding long_value field 11 — the NodeBirth requirement),
        // seq 0.
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
        let encoded = encode_payload(&payload, &PayloadEncodingContext {
            timestamp_millis: Some(1_700_000_000_000),
            uuid: None,
        })
        .expect("encodable");
        assert_eq!(
            hex(&encoded),
            "08 80 D0 95 FF BC 31 \
             12 1C 0A 0F 69 6E 76 65 72 74 65 72 2D 31 2E 70 2E 6D 77 20 0A \
             69 00 00 00 00 00 00 0C 40 \
             12 0B 0A 05 62 64 53 65 71 20 04 58 00 \
             18 00"
        );
    }

    #[test]
    fn ndeath_encodes_the_pinned_byte_fixture() {
        // Pinned fixture (python -I over the verified table): the DEATH
        // payload carries ONLY the bdSeq correlation metric (datatype
        // Int64 riding long_value — the specification's death-certificate
        // shape; no seq, no point metrics) — the LWT bytes the CONNECT
        // will carries. The decoder extracts the metric into the session
        // surface and out of the metrics list.
        let payload = SparkplugPayload {
            kind: SparkplugMessageKind::NodeDeath,
            seq: None,
            bd_seq: BdSeq::initial(),
            metrics: Vec::new(),
        };
        let encoded = encode_payload(&payload, &PayloadEncodingContext::default())
            .expect("encodable");
        assert_eq!(hex(&encoded), "12 0B 0A 05 62 64 53 65 71 20 04 58 00");
        let decoded = decode_payload(&encoded).expect("decodable");
        assert_eq!(decoded.bd_seq, Some(BdSeq::initial()));
        assert!(decoded.metrics.is_empty());
        assert_eq!(decoded.seq, None);
        assert_eq!(decoded.timestamp_millis, None);
    }

    #[test]
    fn dbirth_encodes_boolean_int32_int64_and_string_metrics() {
        // Pinned fixture (python -I over the verified table): four
        // metrics — a true Boolean (datatype Boolean=11, boolean_value
        // field 14), an Int32 (-42; datatype Int32=3 riding int_value
        // field 10 as the uint32 two's-complement — a 5-byte varint), an
        // Int64 (-1; datatype Int64=4 riding long_value field 11 — the
        // 10-byte varint), and a String "ok" (datatype String=12,
        // string_value field 15); seq 1. A DeviceBirth carries NO bdSeq
        // metric (the specification requires it in NBIRTH/NDEATH only).
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
        let encoded = encode_payload(&payload, &PayloadEncodingContext::default())
            .expect("encodable");
        assert_eq!(
            hex(&encoded),
            "12 24 0A 1E 69 6E 76 65 72 74 65 72 2D 31 2E 61 76 61 69 6C 61 62 6C 65 2E 61 \
             76 61 69 6C 61 62 6C 65 20 0B 70 01 \
             12 17 0A 0D 6D 65 74 65 72 2D 31 2E 65 2E 6D 77 68 20 03 50 D6 FF FF FF 0F \
             12 1C 0A 0D 6D 65 74 65 72 2D 31 2E 69 6E 74 36 34 20 04 58 FF FF FF FF FF \
             FF FF FF FF 01 \
             12 0E 0A 06 73 74 61 74 75 73 20 0C 7A 02 6F 6B \
             18 01"
        );
    }

    #[test]
    fn ndata_encodes_alias_and_round_trips_through_the_decoder() {
        // Pinned fixture (python -I over the verified table): timestamp
        // 1700000001000, one Double metric
        // "battery-1.soc.soc_fraction" = 0.82 (datatype Double=10 riding
        // double_value field 13) carrying the alias 7, seq 1. A NodeData
        // carries NO bdSeq metric (the specification requires it in
        // NBIRTH/NDEATH only). The canonical SOC fraction travels the
        // wire unchanged (no percent detour — the frozen rule).
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
        let encoded = encode_payload(&payload, &PayloadEncodingContext {
            timestamp_millis: Some(1_700_000_001_000),
            uuid: None,
        })
        .expect("encodable");
        assert_eq!(
            hex(&encoded),
            "08 E8 D7 95 FF BC 31 \
             12 29 0A 1A 62 61 74 74 65 72 79 2D 31 2E 73 6F 63 2E 73 6F 63 5F 66 72 61 63 \
             74 69 6F 6E 10 07 20 0A \
             69 3D 0A D7 A3 70 3D EA 3F \
             18 01"
        );
        // Round-trip: the decoder reads the same declared subset back.
        let decoded = decode_payload(&encoded).expect("decodable");
        assert_eq!(decoded.timestamp_millis, Some(1_700_000_001_000));
        assert_eq!(decoded.seq, Some(SparkplugSeq::new(1).expect("valid")));
        assert_eq!(decoded.bd_seq, None);
        assert_eq!(decoded.metrics.len(), 1);
        assert_eq!(decoded.metrics[0].name, "battery-1.soc.soc_fraction");
        assert_eq!(decoded.metrics[0].alias, Some(7));
        assert_eq!(decoded.metrics[0].value, MetricValue::Double(0.82));
    }

    #[test]
    fn decoder_skips_unknown_fields_by_wire_type() {
        // A payload with an unknown varint field (field 42) and an unknown
        // length-delimited field (field 43) interleaved with a known seq
        // field — forward compatibility is skipping, never guessing.
        let mut bytes = pb_varint_field(42, 12345);
        bytes.extend(pb_len_delimited(43, b"unknown"));
        bytes.extend(pb_varint_field(PAYLOAD_FIELD_SEQ, 9));
        let decoded = decode_payload(&bytes).expect("decodable");
        assert_eq!(decoded.seq, Some(SparkplugSeq::new(9).expect("valid")));
        assert!(decoded.metrics.is_empty());
        assert_eq!(decoded.bd_seq, None);
    }

    #[test]
    fn decoder_refuses_type_value_disagreement_and_unknown_type_codes() {
        // A metric with datatype code 99 (outside the declared table) is
        // a named refusal, never a guess.
        let mut bytes = pb_len_delimited(PAYLOAD_FIELD_METRICS, &{
            let mut metric_bytes = pb_len_delimited(METRIC_FIELD_NAME, b"m");
            metric_bytes.extend(pb_varint_field(METRIC_FIELD_DATATYPE, 99));
            metric_bytes
        });
        let error = decode_payload(&bytes).expect_err("unknown type refused");
        assert!(matches!(error, SparkplugWireError::TypeValueMismatch { .. }));
        // A metric with a declared type but no value field likewise.
        let mut bytes = pb_len_delimited(PAYLOAD_FIELD_METRICS, &{
            let mut metric_bytes = pb_len_delimited(METRIC_FIELD_NAME, b"m");
            metric_bytes.extend(pb_varint_field(METRIC_FIELD_DATATYPE, METRIC_TYPE_STRING));
            metric_bytes
        });
        let error = decode_payload(&bytes).expect_err("value-less metric refused");
        assert!(matches!(error, SparkplugWireError::TypeValueMismatch { .. }));
        // The bdSeq correlation-metric name is reserved (the
        // specification's NBIRTH/NDEATH mechanism, datatype Int64): a
        // metric named bdSeq that does not carry the declared Int64 value
        // is a named refusal.
        let mut bytes = pb_len_delimited(PAYLOAD_FIELD_METRICS, &{
            let mut metric_bytes =
                pb_len_delimited(METRIC_FIELD_NAME, BD_SEQ_METRIC_NAME.as_bytes());
            metric_bytes.extend(pb_varint_field(METRIC_FIELD_DATATYPE, METRIC_TYPE_STRING));
            metric_bytes.extend(pb_len_delimited(METRIC_FIELD_STRING_VALUE, b"x"));
            metric_bytes
        });
        let error = decode_payload(&bytes).expect_err("non-Int64 bdSeq metric refused");
        assert!(matches!(error, SparkplugWireError::TypeValueMismatch { .. }));
    }

    #[test]
    fn decoder_refuses_out_of_range_sequences() {
        let bytes = pb_varint_field(PAYLOAD_FIELD_SEQ, 256);
        let error = decode_payload(&bytes).expect_err("sequence above the declared modulus");
        assert_eq!(error, SparkplugWireError::SequenceOutOfRange { value: 256 });
    }
}
