// P1.09 — Event envelope: schema version + mandatory idempotency key.
//
// Part of the flexcore-events P1.09 deliverable (P0.14 inputs; report §15.5,
// §6.3 step 4, §10.5 schema governance, §21.4 SemVer).
//
// Rules encoded here:
//
// - The envelope **owns** the schema version and the idempotency key; the
//   payload ([`crate::events::DomainEvent`]) stays schema-version-agnostic.
// - **The idempotency key is mandatory** (P1.09 acceptance: construction
//   fails without one). Because JetStream delivery is at-least-once
//   (report §15.4), consumers deduplicate on this key; the NATS adapter
//   (P1.15) will carry it as `Nats-Msg-Id` for broker-side deduplication
//   within the duplicate window — the durable guarantee stays here.
// - **Versioned serialization** (P1.09 step 4): the envelope carries a
//   schema version; a breaking change requires a new version, never a
//   silent field repurpose; an unknown version **fails to parse** — there
//   is no silent default and no fallback reading.
//
// The current (and only) supported schema version is
// [`SUPPORTED_SCHEMA_VERSION`] (1).

use crate::events::DomainEvent;
use serde::{Deserialize, Serialize};

/// The only schema version this crate produces and accepts.
pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

/// Errors returned by the envelope and the idempotency key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeError {
    /// The supplied schema version is not supported. No silent default: an
    /// unknown version never parses.
    UnsupportedSchemaVersion {
        /// The version that was supplied.
        found: u32,
        /// The version this crate supports.
        supported: u32,
    },
    /// The idempotency key was empty or contained whitespace; an event
    /// without a usable idempotency key fails construction.
    EmptyIdempotencyKey,
}

impl std::fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnvelopeError::UnsupportedSchemaVersion { found, supported } => write!(
                f,
                "schema version {found} is not supported (supported: {supported}); \
                 a breaking change requires a new schema version, never a silent default"
            ),
            EnvelopeError::EmptyIdempotencyKey => {
                write!(f, "idempotency key is mandatory and must be non-empty without whitespace")
            }
        }
    }
}

impl std::error::Error for EnvelopeError {}

/// The idempotency key of an event: mandatory, non-empty, whitespace-free.
///
/// A distinct validated newtype, not a bare `String`: every `EventEnvelope`
/// carries one, and no envelope can be constructed or deserialized without
/// passing this validation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct IdempotencyKey(String);

impl IdempotencyKey {
    /// Validates and wraps an idempotency key: non-empty, no whitespace.
    pub fn new(key: String) -> Result<Self, EnvelopeError> {
        if key.is_empty() || key.chars().any(|c| c.is_whitespace()) {
            return Err(EnvelopeError::EmptyIdempotencyKey);
        }
        Ok(Self(key))
    }

    /// The key as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for IdempotencyKey {
    type Error = EnvelopeError;

    fn try_from(key: String) -> Result<Self, Self::Error> {
        Self::new(key)
    }
}

impl From<IdempotencyKey> for String {
    fn from(v: IdempotencyKey) -> Self {
        v.0
    }
}

impl std::fmt::Display for IdempotencyKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The event envelope: the typed event plus the two things every durable
/// message needs — a schema version and an idempotency key.
///
/// Construction ([`EventEnvelope::new`]) and deserialization both enforce:
/// supported schema version (unknown version fails), non-empty idempotency
/// key. Events are facts; the envelope adds only delivery metadata, never
/// imperative semantics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "EnvelopeWire")]
pub struct EventEnvelope {
    /// The schema version of this envelope and its payload.
    pub schema_version: u32,
    /// The mandatory idempotency key (consumers deduplicate on it).
    pub idempotency_key: IdempotencyKey,
    /// The typed event payload.
    pub event: DomainEvent,
}

impl EventEnvelope {
    /// Constructs an envelope, rejecting an unsupported schema version and
    /// an empty/whitespace idempotency key.
    pub fn new(
        schema_version: u32,
        idempotency_key: &str,
        event: DomainEvent,
    ) -> Result<Self, EnvelopeError> {
        if schema_version != SUPPORTED_SCHEMA_VERSION {
            return Err(EnvelopeError::UnsupportedSchemaVersion {
                found: schema_version,
                supported: SUPPORTED_SCHEMA_VERSION,
            });
        }
        let key = IdempotencyKey::new(idempotency_key.to_string())?;
        Ok(Self {
            schema_version,
            idempotency_key: key,
            event,
        })
    }
}

/// The deserialization target for [`EventEnvelope`]; the version and key
/// checks run in the `TryFrom` conversion, so a parsed envelope always
/// satisfies the construction invariants.
#[derive(Deserialize)]
struct EnvelopeWire {
    schema_version: u32,
    idempotency_key: IdempotencyKey,
    event: DomainEvent,
}

impl TryFrom<EnvelopeWire> for EventEnvelope {
    type Error = EnvelopeError;

    fn try_from(wire: EnvelopeWire) -> Result<Self, Self::Error> {
        if wire.schema_version != SUPPORTED_SCHEMA_VERSION {
            return Err(EnvelopeError::UnsupportedSchemaVersion {
                found: wire.schema_version,
                supported: SUPPORTED_SCHEMA_VERSION,
            });
        }
        Ok(Self {
            schema_version: wire.schema_version,
            idempotency_key: wire.idempotency_key,
            event: wire.event,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{AssetRegisteredPayload, EVENT_COUNT};
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_contracts::time_model::EventTime;
    use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};

    fn sample_event() -> DomainEvent {
        DomainEvent::AssetRegistered(AssetRegisteredPayload {
            correlation_id: FlexUuid::nil(),
            tenant_id: TenantId::new(FlexUuid::nil()),
            portfolio_id: PortfolioId::new(FlexUuid::nil()),
            site_id: SiteId::new(FlexUuid::nil()),
            asset_id: AssetId::new(FlexUuid::nil()),
            event_time: EventTime::from_unix_millis(1_700_000_000_000)
                .expect("valid UTC timestamp"),
        })
    }

    #[test]
    fn envelope_constructs_with_key_and_supported_version() {
        let envelope = EventEnvelope::new(SUPPORTED_SCHEMA_VERSION, "tenant-a:asset-1:seq-7", sample_event())
            .unwrap();
        assert_eq!(envelope.schema_version, SUPPORTED_SCHEMA_VERSION);
        assert_eq!(envelope.idempotency_key.as_str(), "tenant-a:asset-1:seq-7");
    }

    #[test]
    fn envelope_construction_fails_without_an_idempotency_key() {
        // P1.09 acceptance: an event without an idempotency key fails
        // construction — empty key.
        assert_eq!(
            EventEnvelope::new(SUPPORTED_SCHEMA_VERSION, "", sample_event()),
            Err(EnvelopeError::EmptyIdempotencyKey)
        );
        // Whitespace-only keys are equally rejected.
        assert_eq!(
            EventEnvelope::new(SUPPORTED_SCHEMA_VERSION, "   ", sample_event()),
            Err(EnvelopeError::EmptyIdempotencyKey)
        );
        // A key with embedded whitespace is rejected: the key travels as a
        // single opaque token (Nats-Msg-Id header).
        assert_eq!(
            EventEnvelope::new(SUPPORTED_SCHEMA_VERSION, "key with space", sample_event()),
            Err(EnvelopeError::EmptyIdempotencyKey)
        );
    }

    #[test]
    fn envelope_construction_fails_with_unknown_schema_version() {
        assert_eq!(
            EventEnvelope::new(0, "k", sample_event()),
            Err(EnvelopeError::UnsupportedSchemaVersion {
                found: 0,
                supported: SUPPORTED_SCHEMA_VERSION
            })
        );
        assert_eq!(
            EventEnvelope::new(2, "k", sample_event()),
            Err(EnvelopeError::UnsupportedSchemaVersion {
                found: 2,
                supported: SUPPORTED_SCHEMA_VERSION
            })
        );
    }

    #[test]
    fn envelope_round_trips_through_serde_json() {
        let envelope = EventEnvelope::new(SUPPORTED_SCHEMA_VERSION, "key-1", sample_event()).unwrap();
        let json = serde_json::to_string(&envelope).unwrap();
        let back: EventEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(back, envelope);
        assert_eq!(back.schema_version, SUPPORTED_SCHEMA_VERSION);
        assert_eq!(back.idempotency_key.as_str(), "key-1");
    }

    #[test]
    fn unknown_schema_version_fails_to_parse() {
        // P1.09 acceptance: unknown event version fails to parse (no silent
        // default, no fallback reading).
        let event_json = serde_json::to_value(sample_event()).unwrap();
        let mut json = serde_json::Map::new();
        json.insert("schema_version".into(), serde_json::json!(2));
        json.insert("idempotency_key".into(), serde_json::json!("key-1"));
        json.insert("event".into(), event_json);
        let err = serde_json::from_value::<EventEnvelope>(serde_json::Value::Object(json))
            .unwrap_err();
        assert!(err.to_string().contains("schema version"), "error names the version problem: {err}");
    }

    #[test]
    fn missing_idempotency_key_fails_to_parse() {
        let envelope = EventEnvelope::new(SUPPORTED_SCHEMA_VERSION, "key-1", sample_event()).unwrap();
        let mut json = serde_json::to_value(&envelope).unwrap();
        json.as_object_mut().unwrap().remove("idempotency_key");
        // The key is a required field: absence is a parse error.
        assert!(serde_json::from_value::<EventEnvelope>(json).is_err());
    }

    #[test]
    fn empty_idempotency_key_fails_to_parse() {
        let envelope = EventEnvelope::new(SUPPORTED_SCHEMA_VERSION, "key-1", sample_event()).unwrap();
        let mut json = serde_json::to_value(&envelope).unwrap();
        json.as_object_mut().unwrap().insert(
            "idempotency_key".into(),
            serde_json::json!(""),
        );
        assert!(serde_json::from_value::<EventEnvelope>(json).is_err());
    }

    #[test]
    fn idempotency_key_type_validates_every_path() {
        assert!(IdempotencyKey::new(String::new()).is_err());
        assert!(IdempotencyKey::new("ok-key".to_string()).is_ok());
        let key = IdempotencyKey::new("ok-key".to_string()).unwrap();
        let back: IdempotencyKey =
            serde_json::from_value(serde_json::to_value(&key).unwrap()).unwrap();
        assert_eq!(back, key);
    }

    #[test]
    fn every_event_fits_the_envelope() {
        for event in crate::events::DomainEvent::all_variants().iter() {
            let envelope =
                EventEnvelope::new(SUPPORTED_SCHEMA_VERSION, "count-key", *event).unwrap();
            let json = serde_json::to_string(&envelope).unwrap();
            let back: EventEnvelope = serde_json::from_str(&json).unwrap();
            assert_eq!(back.event, *event);
        }
        assert_eq!(EVENT_COUNT, 26);
    }
}
