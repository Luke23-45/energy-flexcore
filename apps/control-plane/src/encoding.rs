// P1.17 — Production event payload encoder (serde_json).
//
// Lives in the composition root, not in flexcore-application: the
// application layer stays serialization-free by design; the envelope
// rendering to wire JSON (P1.09 schema) is an API-side concern implemented
// with serde_json, the same library the REST surface uses.
use flexcore_application::error::ApplicationError;
use flexcore_application::payload_encoding::EventPayloadEncoder;
use flexcore_events::envelope::EventEnvelope;

/// Renders one event envelope to its wire JSON (P1.09 schema versioning).
#[derive(Debug, Default, Clone, Copy)]
pub struct JsonEventPayloadEncoder;

impl EventPayloadEncoder for JsonEventPayloadEncoder {
    fn encode(&self, envelope: &EventEnvelope) -> Result<String, ApplicationError> {
        serde_json::to_string(envelope).map_err(|_e| {
            // Opaque by contract: no serializer internals reach callers; the
            // trace id in the logs is the correlation handle.
            ApplicationError::Infrastructure(
                flexcore_application::error::InfrastructureError::new(
                    "event_payload_encode_failed".to_string(),
                    "the event payload could not be rendered".to_string(),
                ),
            )
        })
    }
}
