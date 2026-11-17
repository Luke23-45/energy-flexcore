// P14.11 — Sparkplug B payload and identity model types (decision record
// 43_p14_11_mqtt_sparkplug_b_uplink.md; sources [S-39][S-40]).
//
// UNVERIFIED HONESTY (plan §7 note 6): NO Sparkplug B specification text is
// in the repository. Every message-type string, sequence semantic, payload
// field, and metric-type spelling below is a secondary-source reading
// ([S-39], the OpenFMB overview [S-40], common public descriptions of
// Sparkplug B) and carries an UNVERIFIED label at its definition naming
// exactly what the implementation-time check against the specification
// text must confirm. The types are the platform-side DECLARED vocabulary:
// they exist so the BIRTH/DEATH state machine ([`crate::birth_death`]) and
// the uplink mapping ([`crate::uplink`]) have typed inputs — not as a
// claim about the wire.

use serde::{Deserialize, Serialize};

// --- Identity parts ---------------------------------------------------------

/// Errors returned by identity and payload construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    /// An identity field is empty. Silence is never a declared identity.
    EmptyField(&'static str),
    /// An identity field exceeds the declared bound. The bound is a
    /// declared design constant, not a specification claim.
    TooLong {
        field: &'static str,
        max: usize,
    },
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelError::EmptyField(field) => write!(f, "identity field '{field}' is empty"),
            ModelError::TooLong { field, max } => {
                write!(f, "identity field '{field}' exceeds the declared bound of {max}")
            }
        }
    }
}

impl std::error::Error for ModelError {}

/// Declared identity bound: 128 characters. UNVERIFIED — the Sparkplug B
/// specification's actual length constraints for group/edge-node/device
/// identifiers are an implementation-time check against the text.
pub const IDENTITY_MAX_CHARS: usize = 128;

fn validated_id(field: &'static str, raw: &str) -> Result<String, ModelError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ModelError::EmptyField(field));
    }
    if trimmed.chars().count() > IDENTITY_MAX_CHARS {
        return Err(ModelError::TooLong {
            field,
            max: IDENTITY_MAX_CHARS,
        });
    }
    Ok(trimmed.to_string())
}

/// The Sparkplug **group id** — the first level below the namespace in the
/// topic hierarchy (see [`crate::topic`]). UNVERIFIED spelling rules; the
/// implementation-time check confirms the allowed character set and length
/// against the specification text.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GroupId(String);

impl GroupId {
    pub fn new(raw: &str) -> Result<Self, ModelError> {
        Ok(Self(validated_id("group_id", raw)?))
    }

    pub fn value(&self) -> &str {
        &self.0
    }
}

/// The Sparkplug **edge node id** — the broker client identity of one edge
/// node (one site deployment). UNVERIFIED uniqueness rules; the
/// implementation-time check confirms whether the specification constrains
/// the edge node id against the MQTT client id.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeNodeId(String);

impl EdgeNodeId {
    pub fn new(raw: &str) -> Result<Self, ModelError> {
        Ok(Self(validated_id("edge_node_id", raw)?))
    }

    pub fn value(&self) -> &str {
        &self.0
    }
}

/// The Sparkplug **device id** — one device under an edge node. UNVERIFIED
/// character-set rules (as for [`GroupId`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId(String);

impl DeviceId {
    pub fn new(raw: &str) -> Result<Self, ModelError> {
        Ok(Self(validated_id("device_id", raw)?))
    }

    pub fn value(&self) -> &str {
        &self.0
    }
}

// --- Message kinds ----------------------------------------------------------

/// The Sparkplug B message-kind vocabulary (decision record 43 §1).
///
/// UNVERIFIED — the exact topic message-type strings below (the
/// `NBIRTH`/`DBIRTH`/`NDATA`/`DDATA`/`NDEATH`/`DDEATH`/`NCMD`/`DCMD`/
/// `STATE` spellings and their segment positions) are secondary-source
/// readings; the implementation-time check confirms each spelling and its
/// topic segment against the specification text. `Command` kinds are
/// recorded for vocabulary completeness ONLY: this adapter is an UPLINK
/// (MonitorOnly) and declares no command subscription — the command path
/// does not exist in this batch (the [`crate::scope`] gate refuses it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SparkplugMessageKind {
    /// Edge-node BIRTH (the full metric inventory of an edge node).
    NodeBirth,
    /// Device BIRTH (the full metric inventory of one device).
    DeviceBirth,
    /// Edge-node DATA (changed metrics of an edge node).
    NodeData,
    /// Device DATA (changed metrics of one device).
    DeviceData,
    /// Edge-node DEATH (the MQTT will of the edge node).
    NodeDeath,
    /// Device DEATH.
    DeviceDeath,
    /// The STATE topic (primary-host state: the edge observes it to know
    /// whether the platform's primary host is online).
    State,
    /// Command kinds (NCMD/DCMD) — vocabulary completeness ONLY; NOT
    /// declared by this adapter's scope (MonitorOnly uplink).
    Command,
}

impl SparkplugMessageKind {
    /// The topic message-type segment this kind uses.
    ///
    /// UNVERIFIED — each spelling is a secondary-source reading; the
    /// implementation-time check confirms every string against the
    /// Sparkplug B specification text.
    pub const fn topic_segment(self) -> &'static str {
        match self {
            SparkplugMessageKind::NodeBirth => "NBIRTH",
            SparkplugMessageKind::DeviceBirth => "DBIRTH",
            SparkplugMessageKind::NodeData => "NDATA",
            SparkplugMessageKind::DeviceData => "DDATA",
            SparkplugMessageKind::NodeDeath => "NDEATH",
            SparkplugMessageKind::DeviceDeath => "DDEATH",
            SparkplugMessageKind::State => "STATE",
            SparkplugMessageKind::Command => "NCMD",
        }
    }

    /// Whether this kind is a BIRTH message (declares a full inventory).
    pub const fn is_birth(self) -> bool {
        matches!(
            self,
            SparkplugMessageKind::NodeBirth | SparkplugMessageKind::DeviceBirth
        )
    }

    /// Whether this kind is a DEATH message.
    pub const fn is_death(self) -> bool {
        matches!(
            self,
            SparkplugMessageKind::NodeDeath | SparkplugMessageKind::DeviceDeath
        )
    }

    /// Whether this kind is a DATA message (changed metrics only).
    pub const fn is_data(self) -> bool {
        matches!(
            self,
            SparkplugMessageKind::NodeData | SparkplugMessageKind::DeviceData
        )
    }
}

// --- Sequence counters ------------------------------------------------------

/// The declared modulus of the Sparkplug sequence counter.
///
/// UNVERIFIED — public secondary descriptions commonly state that the
/// Sparkplug B message sequence (`seq`) counts 0..255 and wraps at 256;
/// the implementation-time check confirms the wrap point and whether the
/// specification forbids reuse of specific values (e.g. reserving one
/// value for DEATH). This constant is the DECLARED wrap this crate tests
/// against — it is a design pin, not a specification quote.
pub const SEQ_MODULUS: u64 = 256;

/// The Sparkplug message sequence number (`seq`) carried by BIRTH/DATA
/// messages. Wraps at the DECLARED [`SEQ_MODULUS`] (UNVERIFIED wrap point
/// — see the constant's documentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SparkplugSeq(u64);

impl SparkplugSeq {
    /// Constructs a sequence number; rejects values at or above the
    /// declared modulus.
    pub fn new(value: u64) -> Result<Self, ModelError> {
        if value >= SEQ_MODULUS {
            return Err(ModelError::TooLong {
                field: "sparkplug_seq",
                max: SEQ_MODULUS as usize,
            });
        }
        Ok(Self(value))
    }

    /// The raw sequence value.
    pub fn value(self) -> u64 {
        self.0
    }

    /// The successor, wrapping at the declared [`SEQ_MODULUS`].
    pub fn next(self) -> Self {
        Self((self.0 + 1) % SEQ_MODULUS)
    }
}

/// The Sparkplug **birth/death sequence** (`bdSeq`) — a monotonically
/// increasing counter carried by every BIRTH and DEATH of one edge node so
/// a consumer can detect a stale DEATH or a reconnect.
///
/// UNVERIFIED — the bdSeq width, its wrap semantic, and whether bdSeq is
/// an independent counter from `seq` are implementation-time checks
/// against the specification text. This type models it as a saturating
/// 64-bit counter that does NOT wrap (a declared design choice pending the
/// check — recorded in the decision record).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BdSeq(u64);

impl BdSeq {
    pub const fn initial() -> Self {
        Self(0)
    }

    /// Constructs a bdSeq from its raw counter value (any u64 is a valid
    /// generation identifier; there is no range invariant to enforce).
    pub const fn from_raw(value: u64) -> Self {
        Self(value)
    }

    pub fn value(self) -> u64 {
        self.0
    }

    /// The successor. Saturates at `u64::MAX` instead of wrapping (the
    /// declared design choice recorded above).
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

// --- Metrics and payload ----------------------------------------------------

/// The declared metric-type vocabulary.
///
/// UNVERIFIED — the variant names mirror common public descriptions of the
/// Sparkplug B metric types; the implementation-time check confirms the
/// exact type set and spellings against the specification text. The set
/// here is deliberately SMALL: only the types the canonical uplink
/// quantities need (booleans, integers, floating point, strings).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricType {
    /// Boolean.
    Boolean,
    /// 32-bit signed integer.
    Int32,
    /// 64-bit signed integer.
    Int64,
    /// Double-precision floating point.
    Double,
    /// UTF-8 string.
    String,
}

/// One metric value (typed; the payload model rejects a value whose
/// variant does not match the declared [`MetricType`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricValue {
    Boolean(bool),
    Int(i64),
    Double(f64),
    Text(String),
}

/// One Sparkplug metric: a named, typed value.
///
/// UNVERIFIED — the Sparkplug B payload carries metrics with an `alias`
/// (a numeric short form agreed in the BIRTH) and additional flags (e.g.
/// transience); the implementation-time check confirms the alias
/// mechanism and flag set. This model carries the name always and the
/// alias as an optional declared field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SparkplugMetric {
    /// The metric name (the dot-separated point path the namespace
    /// declares; see [`crate::uplink`]).
    pub name: String,
    /// The optional declared alias (UNVERIFIED mechanism — see above).
    pub alias: Option<u64>,
    /// The declared metric type.
    pub metric_type: MetricType,
    /// The value (must match [`SparkplugMetric::metric_type`]).
    pub value: MetricValue,
}

impl SparkplugMetric {
    /// Constructs a metric and enforces the type/value agreement — a
    /// payload whose value does not match its declared type is a
    /// construction error, never a wire surprise.
    pub fn new(
        name: String,
        alias: Option<u64>,
        metric_type: MetricType,
        value: MetricValue,
    ) -> Result<Self, ModelError> {
        if name.trim().is_empty() {
            return Err(ModelError::EmptyField("metric.name"));
        }
        let agrees = match (&metric_type, &value) {
            (MetricType::Boolean, MetricValue::Boolean(_)) => true,
            (MetricType::Int32 | MetricType::Int64, MetricValue::Int(_)) => true,
            (MetricType::Double, MetricValue::Double(_)) => true,
            (MetricType::String, MetricValue::Text(_)) => true,
            _ => false,
        };
        if !agrees {
            return Err(ModelError::EmptyField("metric.type_value_agreement"));
        }
        Ok(Self {
            name,
            alias,
            metric_type,
            value,
        })
    }
}

/// The declared Sparkplug payload model: the metrics plus the sequence
/// and bdSeq counters the session state machine consumes.
///
/// UNVERIFIED — the real Sparkplug B payload is a protobuf (`Payload`
/// with `metrics`, `seq`, `bdSeq`, `timestamp`, ...); the implementation-
/// time check confirms the field set and encoding. This model is the
/// platform-side declaration of the SAME content, in serde terms, so the
/// session machine and the uplink plan have typed inputs. The protobuf
/// ENCODING is a transport-seam concern (the wire seam serializes; this
/// crate never touches protobuf).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SparkplugPayload {
    /// The message kind this payload belongs to.
    pub kind: SparkplugMessageKind,
    /// The message sequence (BIRTH/DATA; DEATH carries it UNVERIFIED —
    /// see [`SparkplugSeq`]).
    pub seq: Option<SparkplugSeq>,
    /// The birth/death sequence of the publishing edge node.
    pub bd_seq: BdSeq,
    /// The metrics of this message (a BIRTH carries the FULL inventory; a
    /// DATA carries CHANGED metrics only — UNVERIFIED rule, confirmed by
    /// the session machine's design in [`crate::birth_death`]).
    pub metrics: Vec<SparkplugMetric>,
}

impl SparkplugPayload {
    /// Validates the payload against its own kind: BIRTH and DATA messages
    /// MUST carry a sequence number; DEATH and STATE may not (UNVERIFIED
    /// — the implementation-time check confirms which messages carry
    /// `seq`).
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.kind.is_birth() || self.kind.is_data() {
            if self.seq.is_none() {
                return Err(ModelError::EmptyField("payload.seq (required for BIRTH/DATA)"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_rejects_empty_and_oversized() {
        assert_eq!(GroupId::new("   "), Err(ModelError::EmptyField("group_id")));
        let oversized = "x".repeat(IDENTITY_MAX_CHARS + 1);
        assert!(matches!(
            EdgeNodeId::new(&oversized),
            Err(ModelError::TooLong { .. })
        ));
        let at_bound = "x".repeat(IDENTITY_MAX_CHARS);
        assert!(EdgeNodeId::new(&at_bound).is_ok());
    }

    #[test]
    fn topic_segments_are_the_declared_spellings() {
        // These spellings are UNVERIFIED secondary-source readings; the
        // test pins the DECLARED values so a specification-confirmed
        // correction is a visible one-line edit, never a silent drift.
        assert_eq!(SparkplugMessageKind::NodeBirth.topic_segment(), "NBIRTH");
        assert_eq!(SparkplugMessageKind::DeviceBirth.topic_segment(), "DBIRTH");
        assert_eq!(SparkplugMessageKind::NodeData.topic_segment(), "NDATA");
        assert_eq!(SparkplugMessageKind::DeviceData.topic_segment(), "DDATA");
        assert_eq!(SparkplugMessageKind::NodeDeath.topic_segment(), "NDEATH");
        assert_eq!(SparkplugMessageKind::DeviceDeath.topic_segment(), "DDEATH");
        assert_eq!(SparkplugMessageKind::State.topic_segment(), "STATE");
    }

    #[test]
    fn seq_wraps_at_the_declared_modulus_and_rejects_overflow() {
        let last = SparkplugSeq::new(SEQ_MODULUS - 1).expect("valid");
        assert_eq!(last.next().value(), 0, "wraps at the declared modulus");
        assert!(SparkplugSeq::new(SEQ_MODULUS).is_err());
    }

    #[test]
    fn bd_seq_saturates_never_wraps() {
        let max = BdSeq(u64::MAX);
        assert_eq!(max.next(), BdSeq(u64::MAX));
        assert_eq!(BdSeq::initial().next(), BdSeq(1));
    }

    #[test]
    fn metric_rejects_type_value_disagreement() {
        assert!(SparkplugMetric::new(
            "inverter_1.p_mw".to_string(),
            None,
            MetricType::Double,
            MetricValue::Text("3.5".to_string()),
        )
        .is_err());
        assert!(SparkplugMetric::new(
            "inverter_1.p_mw".to_string(),
            None,
            MetricType::Double,
            MetricValue::Double(3.5),
        )
        .is_ok());
    }

    #[test]
    fn payload_requires_seq_for_birth_and_data() {
        let birth_without_seq = SparkplugPayload {
            kind: SparkplugMessageKind::NodeBirth,
            seq: None,
            bd_seq: BdSeq::initial(),
            metrics: Vec::new(),
        };
        assert!(birth_without_seq.validate().is_err());
        let data_with_seq = SparkplugPayload {
            kind: SparkplugMessageKind::NodeData,
            seq: SparkplugSeq::new(1).ok(),
            bd_seq: BdSeq::initial(),
            metrics: Vec::new(),
        };
        assert!(data_with_seq.validate().is_ok());
    }
}
