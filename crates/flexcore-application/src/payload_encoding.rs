// P1.08 — The event-payload encoding seam.
//
// The transactional-outbox record (P1.09) stores the **serialized** event:
// the envelope rendered to its wire form (JSON per the P1.09 schema). This
// crate depends on no serialization library — the dependency rule and the
// P1.08 manifest allow only the four internal crates plus std — so the
// rendering is an injected seam: the composition root (API layer / worker,
// P1.19) implements this trait over its serialization library and hands the
// use cases an instance. The use cases own everything else: the typed
// event, the envelope, the schema version, and the idempotency key.
//
// Implementer obligations: render the envelope **exactly** per the
// flexcore-events schema (P1.09; versioned — a breaking change is a new
// schema version, never a silent field repurpose) and fail with
// [`ApplicationError::Infrastructure`] if rendering is impossible. A test
// fake may produce any deterministic non-empty marker string; production
// implementations produce the wire JSON.

use crate::error::ApplicationError;
use flexcore_events::envelope::EventEnvelope;

/// Renders one event envelope into its wire payload (P1.09 schema).
pub trait EventPayloadEncoder {
    /// Serializes the envelope. The returned string is stored verbatim as
    /// the outbox record's payload and published by the broker adapter.
    fn encode(&self, envelope: &EventEnvelope) -> Result<String, ApplicationError>;
}
