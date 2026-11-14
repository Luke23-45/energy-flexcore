// P5.25 — Live updates: the tenant-scoped SSE stream (§15.1, §21.3; ledger
//         P5.25) at `GET /api/v1/live`.
//
// What this route is (and is not):
//
// - The control plane exposes live updates by subscribing to the SAME event
//   subjects the NATS adapter's durable consumers process
//   (`flexcore.v1.>`, P1.14 §3) — as an EPHEMERAL core-NATS subscription
//   (no JetStream consumer, no inbox record, no side effect; P1.14 §5.1
//   reserves durable consumers for stateful work). The subscription runs in
//   the composition root ([`run_live_forwarder`], wired in `main.rs`) and
//   forwards every message into an in-process [`LiveEventBus`]. The SSE
//   handler filters the bus per caller tenant (D-J2: tenant scoping happens
//   at CONSUMPTION, from the payload — no tenant identifier is in any
//   subject).
// - Payloads are decoded ONLY into the frozen §15.3 vocabulary
//   (`flexcore_events::DomainEvent`): a message that does not decode, or
//   that does not carry a parsable `tenant_id`, is never forwarded to any
//   stream (fail closed; §16.2 — no cross-tenant leakage, no invented
//   events). The envelope re-extracts fields from the serde rendering of
//   the frozen event, so the SSE shape cannot drift from the event schema.
// - **Delivery is at-least-once and UNORDERED** (report §15.4): the bus may
//   duplicate and reorder events, and a slow subscriber can fall behind —
//   on a lagged stream the handler emits a `RESYNC_REQUIRED` event instead
//   of silently gap-filling (§15.1; P5.25 step 2). Ordering/dedup is
//   P5.33's scope and is deliberately NOT implemented here; the envelope
//   carries the honesty statement so every consumer sees the contract.
// - **Correlation (§21.3)**: every forwarded event carries the
//   `correlation_id` of the originating §15.3 event — the same id the audit
//   record carries — so a displayed dispatch state links to the same trace
//   as the audit trail. No secrets and no unnecessary personal/customer
//   data travel in the stream (the frozen payloads carry identities and
//   quality states, never credentials; P5.25 step 5).
// - **Authorization (recorded drift)**: the ledger action id for this
//   surface is `live.stream.read` (tenant-scoped, all roles), but the
//   `Action` vocabulary is a CLOSED enum owned by `crates/flexcore-authz`,
//   outside this subtask's file ownership. The gate therefore goes through
//   [`crate::auth::authorize_live_stream`], which currently evaluates
//   `telemetry.read` (already tenant-scoped and granted to all four roles)
//   and documents the one-line swap point. Recorded in the decision record
//   `ledger/phase-05-operator-console-simulator/01_25_scaffold_live.md`.
//
// Stream framing: SSE (`text/event-stream`, axum 0.8.9 `axum::response::
// sse` per the existing axum patterns). Data events carry the frozen
// SCREAMING_SNAKE wire tag as the SSE `event:` name and a
// [`LiveEventEnvelope`] JSON body; the idle heartbeat is the
// `KeepAlive` comment (interval below). Connection loss is honest by
// construction: an HTTP stream that ends is visible to the client, and the
// client hook (console `src/live/`) surfaces "live updates paused — data
// may be stale" and re-syncs explicitly — never silently gap-fills.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::get;
use axum::Router;
use flexcore_events::DomainEvent;
use serde::Serialize;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::sync::broadcast;
use tokio_stream::StreamExt as _;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::BroadcastStream;
use utoipa::ToSchema;

use crate::auth::CallerExtractor;
use crate::error::ApiError;
use crate::state::AppState;

/// The live forwarder's subject filter (P1.14 §3; D-J2). Deliberately the
/// whole `flexcore.v1` namespace: the SSE handler scopes per tenant from
/// the decoded payload, never from subject tokens.
pub const LIVE_SUBJECT_FILTER: &str = "flexcore.v1.>";

/// The SSE idle-heartbeat interval (a comment line the client reads as
/// "the stream is alive"). Recorded default, not a design constant: it is
/// deployment-configurable in a later subtask if needed.
const SSE_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);

/// The in-process bus capacity per subscriber. When a subscriber falls more
/// than this many events behind, the broadcast channel reports a lag and
/// the stream emits `RESYNC_REQUIRED` (no silent gap-filling, §15.1).
const BUS_CAPACITY: usize = 1024;

// ---------------------------------------------------------------------------
// The in-process live event bus
// ---------------------------------------------------------------------------

/// One forwarded broker message on the bus (the serialized §15.3 event,
/// verbatim, plus its subject).
#[derive(Debug, Clone)]
pub struct LiveBusMessage {
    /// The subject the event was published to
    /// (`flexcore.v1.<area>.<event_slug>`).
    pub subject: String,
    /// The serialized event payload, verbatim (JSON bytes).
    pub payload: Vec<u8>,
}

/// The in-process fan-out hub between the NATS forwarder and the SSE
/// handlers. Publishing never fails: with no subscribers the event is
/// dropped (a live view with no listeners owes nobody delivery), with
/// subscribers the broadcast channel buffers up to [`BUS_CAPACITY`] per
/// receiver and lags honestly beyond that (surfaced as `RESYNC_REQUIRED`).
///
/// The bus is shared as an `Arc<LiveEventBus>` (see `AppState::live`), so
/// the closed/open state lives behind interior mutability (a std Mutex —
/// no await while held): `close()` drops the sender through ANY handle,
/// and every open stream then drains its buffered events and ends cleanly
/// (tokio broadcast semantics: `recv` yields the buffered messages before
/// reporting `Closed`) — which is also how the integration tests
/// terminate a stream deterministically. The bus is deliberately NOT
/// `Clone`: a cloned handle could not propagate a close to its twin.
pub struct LiveEventBus {
    inner: std::sync::Mutex<Option<broadcast::Sender<LiveBusMessage>>>,
}

impl LiveEventBus {
    /// Creates an open bus with no subscribers.
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(BUS_CAPACITY);
        Self {
            inner: std::sync::Mutex::new(Some(tx)),
        }
    }

    /// Forwards one broker message to every open subscriber stream.
    /// A send with no receivers (or a closed bus) is a no-op.
    pub fn publish(&self, subject: String, payload: Vec<u8>) {
        // The lock guards only the handle clone — the send itself runs
        // unlocked (broadcast send is non-blocking).
        let tx = self.inner.lock().expect("the bus mutex is never poisoned").clone();
        if let Some(tx) = tx {
            let _ = tx.send(LiveBusMessage { subject, payload });
        }
    }

    /// Subscribes one SSE stream. Fails only when the bus is closed
    /// (process shutdown): the route then fails closed (infrastructure).
    pub fn subscribe(&self) -> Result<broadcast::Receiver<LiveBusMessage>, LiveBusClosed> {
        self.inner
            .lock()
            .expect("the bus mutex is never poisoned")
            .as_ref()
            .map(|tx| tx.subscribe())
            .ok_or(LiveBusClosed)
    }

    /// Closes the bus: every open stream drains its buffered events and
    /// then ends. Used by the integration tests (deterministic stream end)
    /// and available for composition-root shutdown wiring.
    pub fn close(&self) {
        *self.inner.lock().expect("the bus mutex is never poisoned") = None;
    }
}

/// The bus is closed; no new stream can subscribe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveBusClosed;

impl Default for LiveEventBus {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// The live event envelope (the SSE data body)
// ---------------------------------------------------------------------------

/// The SSE data envelope for one forwarded §15.3 event. Fields are
/// extracted from the serde rendering of the frozen event
/// (`flexcore_events::DomainEvent`) — the envelope cannot drift from the
/// event schema because both come from the same serde types.
#[derive(Debug, Serialize, ToSchema)]
pub struct LiveEventEnvelope {
    /// The frozen SCREAMING_SNAKE wire tag (`"TELEMETRY_RECEIVED"`, …).
    pub event_type: String,
    /// The frozen §15.3 PascalCase spelling (`"TelemetryReceived"`, …).
    pub event_name: String,
    /// The subject the event was published to.
    pub subject: String,
    /// The §15.3 correlation id — the SAME id the audit record carries
    /// (§21.3 trace correlation).
    pub correlation_id: String,
    /// The tenant the event belongs to (the stream is scoped to exactly
    /// this tenant; the caller's tenant must match or the event is not
    /// forwarded).
    pub tenant_id: String,
    /// The portfolio scope, where the frozen event carries one.
    pub portfolio_id: Option<String>,
    /// The site scope, where the frozen event carries one.
    pub site_id: Option<String>,
    /// The asset scope, where the frozen event carries one.
    pub asset_id: Option<String>,
    /// The §14.2 command id (dispatch-lifecycle events only).
    pub command_id: Option<String>,
    /// The plugin id (plugin events only).
    pub plugin_id: Option<String>,
    /// The event time (§10.3), RFC 3339 UTC.
    pub event_time: String,
    /// The server delivery time (when the SSE handler forwarded it), RFC
    /// 3339 UTC — the freshness anchor for "live vs stale" labelling
    /// (P5.10; distinct from `event_time`, never merged with it, §6.2).
    pub delivered_at: String,
    /// The delivery class of this stream. Always `at-least-once`: the
    /// stream may duplicate and reorder events (§15.4).
    pub delivery: String,
    /// The honesty statement every consumer sees: ordering/dedup is NOT
    /// implemented on this stream (P5.33 owns it) — consumers must not
    /// treat arrival order as occurrence order and must re-sync from the
    /// REST read APIs after a gap, never silently gap-fill.
    pub ordering_statement: String,
}

/// The `RESYNC_REQUIRED` SSE body: the stream fell behind and skipped
/// events; the client must re-sync from the REST read APIs (no silent
/// gap-filling, P5.25 step 2).
#[derive(Debug, Serialize, ToSchema)]
pub struct LiveResyncNotice {
    pub notice: String,
    /// How many events were skipped by the lag (observable diagnostics;
    /// P5.33 turns discard/skip counting into the full discipline).
    pub skipped_events: u64,
    pub statement: String,
}

const DELIVERY_CLASS: &str = "at-least-once";
const ORDERING_STATEMENT: &str = "events may be duplicated or reordered; do not treat \
     arrival order as occurrence order; after a gap or RESYNC_REQUIRED, re-sync from the \
     REST read APIs (ordering/dedup lands with P5.33)";

/// Renders one forwarded event as the SSE envelope, or `None` when the
/// payload must not reach any stream: not a decodable frozen §15.3 event,
/// or missing the tenant id (fail closed — an undecodable message is never
/// "best-effort forwarded", §16.2).
fn build_live_envelope(subject: &str, payload: &[u8]) -> Option<LiveEventEnvelope> {
    // Decode ONLY the frozen vocabulary: an unknown tag, an unknown field
    // shape, or any other payload never becomes a live event.
    let event: DomainEvent = serde_json::from_slice(payload).ok()?;
    let wire = serde_json::to_value(&event).ok()?;
    let tenant_id = wire
        .get("tenant_id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)?;
    Some(LiveEventEnvelope {
        event_type: wire
            .get("event_type")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)?,
        event_name: flexcore_events::event_name(&event).to_string(),
        subject: subject.to_string(),
        correlation_id: wire
            .get("correlation_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)?,
        tenant_id,
        portfolio_id: optional_id(&wire, "portfolio_id"),
        site_id: optional_id(&wire, "site_id"),
        asset_id: optional_id(&wire, "asset_id"),
        command_id: optional_id(&wire, "command_id"),
        plugin_id: optional_id(&wire, "plugin_id"),
        event_time: wire
            .get("event_time")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)?,
        delivered_at: now_rfc3339(),
        delivery: DELIVERY_CLASS.to_string(),
        ordering_statement: ORDERING_STATEMENT.to_string(),
    })
}

/// Reads an optional canonical UUID field from the wire rendering.
fn optional_id(wire: &serde_json::Value, field: &str) -> Option<String> {
    wire.get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

/// The server delivery timestamp (RFC 3339 UTC).
fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "<invalid-timestamp>".to_string())
}

/// Builds the `RESYNC_REQUIRED` SSE data event.
fn resync_required_event(skipped: u64) -> Event {
    let notice = LiveResyncNotice {
        notice: "resync_required".to_string(),
        skipped_events: skipped,
        statement: "the live stream fell behind and skipped events; re-sync from the REST \
             read APIs; no silent gap-filling (P5.25, §15.1)"
            .to_string(),
    };
    let data = serde_json::to_string(&notice).unwrap_or_else(|_| "{}".to_string());
    Event::default().event("RESYNC_REQUIRED").data(data)
}

// ---------------------------------------------------------------------------
// Router + handler
// ---------------------------------------------------------------------------

/// Builds the live-update router (merged into the stage-1 router). GET
/// only: the stream is a READ surface — there is no write, no
/// client-to-server message path, and no execute semantics anywhere on it
/// (§15.2).
pub fn live_router() -> Router<AppState> {
    Router::new().route("/api/v1/live", get(live_events))
}

/// The tenant-scoped live event stream (§15.1, §21.3; P5.25).
///
/// Every handler rule holds exactly as on the Phase 1–4 routes: resolve
/// identity (401 when absent or invalid) -> authorize (deny-by-default,
/// tenant-scoped; 403 on deny) -> subscribe. The stream itself is the read
/// surface: each forwarded event is decoded into the frozen §15.3
/// vocabulary, tenant-filtered against the CALLER's claims-derived tenant
/// (never a request field, §16.2), and delivered at-least-once with the
/// correlation id intact.
#[utoipa::path(
    get,
    path = "/api/v1/live",
    responses(
        (status = 200, description = "A tenant-scoped `text/event-stream` of frozen §15.3 \
             events (at-least-once, unordered; RESYNC_REQUIRED on lag; idle heartbeat as a \
             keep-alive comment). The stream is a READ surface — there is no write or \
             execute path anywhere on it.",
            content_type = "text/event-stream"),
        (status = 401, description = "Missing or invalid authentication"),
        (status = 403, description = "Denied by policy"),
    )
)]
async fn live_events(
    State(state): State<AppState>,
    CallerExtractor(caller): CallerExtractor,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    // The live.stream.read drift: see the module header and
    // `auth::authorize_live_stream` (the single swap point).
    crate::auth::authorize_live_stream(&state.policies, &caller)?;
    let tenant = caller.ctx.tenant_id().uuid().to_string();
    let receiver = state
        .live
        .subscribe()
        .map_err(|_| ApiError::infrastructure())?;
    let stream = BroadcastStream::new(receiver).filter_map(move |item| match item {
        Ok(message) => {
            // Decode -> tenant-filter -> envelope. Anything that does not
            // decode into the frozen vocabulary, or belongs to another
            // tenant, is silently NOT forwarded on this stream (it is
            // never forwarded to ANY tenant stream: the decode happens
            // before the filter, and both must pass).
            let envelope = build_live_envelope(&message.subject, &message.payload)
                .filter(|envelope| envelope.tenant_id == tenant);
            let Some(envelope) = envelope else {
                return None;
            };
            let data = match serde_json::to_string(&envelope) {
                Ok(data) => data,
                // An envelope that cannot be serialized is a server bug;
                // it is dropped, never replaced with invented content.
                Err(_) => return None,
            };
            Some(Ok(Event::default()
                .event(envelope.event_type.clone())
                .data(data)))
        }
        // Lagged: the subscriber fell behind and the channel skipped
        // events. Emit the resync notice and keep streaming (the stream is
        // still alive); the client re-syncs explicitly.
        Err(BroadcastStreamRecvError::Lagged(skipped)) => Some(Ok(resync_required_event(skipped))),
    });
    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(SSE_HEARTBEAT_INTERVAL)
            .text("heartbeat"),
    ))
}

// ---------------------------------------------------------------------------
// The composition-root forwarder (NATS -> bus)
// ---------------------------------------------------------------------------

/// Subscribes to [`LIVE_SUBJECT_FILTER`] on the core NATS client (ephemeral
/// — no JetStream consumer, no inbox, no side effect) and forwards every
/// message into the bus. Run from the composition root (`main.rs`) as a
/// background task; ends when the shutdown watch fires.
///
/// Resilience: a lost subscription is logged and re-established with a
/// short backoff — the stream degrades to heartbeats only while the broker
/// is unreachable, never to invented events. Message contents are never
/// logged (they may reference tenant identities; P5.25 step 5).
pub async fn run_live_forwarder(
    client: async_nats::Client,
    bus: Arc<LiveEventBus>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    const RESUBSCRIBE_BACKOFF: Duration = Duration::from_secs(1);
    loop {
        if *shutdown.borrow() {
            return;
        }
        let mut subscription = match client.subscribe(LIVE_SUBJECT_FILTER).await {
            Ok(subscription) => subscription,
            Err(error) => {
                tracing::error!(
                    error = %error,
                    "live forwarder: cannot subscribe; the live streams degrade to \
                     heartbeats until the broker is reachable"
                );
                if sleep_or_shutdown(RESUBSCRIBE_BACKOFF, &mut shutdown).await {
                    return;
                }
                continue;
            }
        };
        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    return;
                }
                message = subscription.next() => match message {
                    Some(Ok(message)) => {
                        bus.publish(message.subject.to_string(), message.payload.to_vec());
                    }
                    Some(Err(error)) => {
                        tracing::warn!(
                            error = %error,
                            "live forwarder: subscription error; resubscribing"
                        );
                        break;
                    }
                    None => {
                        tracing::warn!("live forwarder: subscription ended; resubscribing");
                        break;
                    }
                },
            }
        }
        if sleep_or_shutdown(RESUBSCRIBE_BACKOFF, &mut shutdown).await {
            return;
        }
    }
}

/// Sleeps for `duration`, cut short by shutdown. Returns `true` when the
/// caller should stop.
async fn sleep_or_shutdown(
    duration: Duration,
    shutdown: &mut tokio::sync::watch::Receiver<bool>,
) -> bool {
    tokio::select! {
        () = tokio::time::sleep(duration) => false,
        _ = shutdown.changed() => true,
    }
}

// ---------------------------------------------------------------------------
// Unit tests (written, NOT run in this loop — the human executes them with
// the first `cargo test`)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_contracts::time_model::EventTime;
    use flexcore_domain::ids::{AssetId, SiteId, TenantId};
    use flexcore_events::{TelemetryReceivedPayload, TelemetryQualityChangedPayload};
    use flexcore_contracts::quality::TelemetryQuality;

    const TENANT_A: FlexUuid = FlexUuid::from_bytes([
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x0a,
    ]);
    const TENANT_B: FlexUuid = FlexUuid::from_bytes([
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x0b,
    ]);
    const CORRELATION: FlexUuid = FlexUuid::from_bytes([
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x2a,
    ]);
    const SITE: FlexUuid = FlexUuid::from_bytes([
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x07,
    ]);
    const ASSET: FlexUuid = FlexUuid::from_bytes([
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x09,
    ]);

    fn telemetry_event(tenant: FlexUuid) -> Vec<u8> {
        let payload = TelemetryReceivedPayload {
            correlation_id: CORRELATION,
            tenant_id: TenantId::new(tenant),
            site_id: SiteId::new(SITE),
            asset_id: AssetId::new(ASSET),
            event_time: EventTime::from_unix_millis(1_700_000_000_000)
                .expect("valid timestamp"),
        };
        serde_json::to_vec(&DomainEvent::TelemetryReceived(payload)).expect("serializes")
    }

    #[test]
    fn envelope_extracts_the_frozen_fields() {
        let envelope = build_live_envelope(
            "flexcore.v1.telemetry.telemetry_received",
            &telemetry_event(TENANT_A),
        )
        .expect("a frozen event decodes");
        assert_eq!(envelope.event_type, "TELEMETRY_RECEIVED");
        assert_eq!(envelope.event_name, "TelemetryReceived");
        assert_eq!(envelope.tenant_id, TENANT_A.to_string());
        assert_eq!(envelope.correlation_id, CORRELATION.to_string());
        assert_eq!(envelope.site_id, Some(SITE.to_string()));
        assert_eq!(envelope.asset_id, Some(ASSET.to_string()));
        assert_eq!(envelope.portfolio_id, None);
        assert_eq!(envelope.command_id, None);
        assert_eq!(envelope.delivery, "at-least-once");
        assert!(!envelope.ordering_statement.is_empty());
    }

    #[test]
    fn an_undecodable_payload_is_never_forwarded() {
        // Not JSON at all.
        assert!(build_live_envelope("flexcore.v1.telemetry.telemetry_received", b"{nope").is_none());
        // A JSON object that is NOT a frozen event (the closed vocabulary
        // rejects invented event types).
        assert!(build_live_envelope(
            "flexcore.v1.telemetry.telemetry_received",
            br#"{"event_type":"NOT_A_REAL_EVENT"}"#
        )
        .is_none());
    }

    #[test]
    fn a_payload_without_a_parsable_tenant_is_never_forwarded() {
        // No payload in the frozen vocabulary lacks a tenant_id, so a
        // hand-crafted "tenant-less" JSON is simply not a frozen event —
        // the decode-then-extract discipline fails closed either way.
        assert!(build_live_envelope("x.y.z", br#"{"event_type":"TELEMETRY_RECEIVED"}"#).is_none());
    }

    #[test]
    fn the_envelope_is_rebuilt_from_the_wire_every_time() {
        // A frozen event with the P2.16 additive field still decodes; the
        // envelope carries the frozen names, never a hand-maintained copy.
        let payload = TelemetryQualityChangedPayload {
            correlation_id: FlexUuid::from_u128(1),
            tenant_id: TenantId::new(TENANT_B),
            site_id: SiteId::new(FlexUuid::from_u128(2)),
            asset_id: AssetId::new(FlexUuid::from_u128(3)),
            quality: TelemetryQuality::Stale,
            previous_quality: Some(TelemetryQuality::Good),
            event_time: EventTime::from_unix_millis(1_700_000_000_000)
                .expect("valid timestamp"),
        };
        let bytes = serde_json::to_vec(&DomainEvent::TelemetryQualityChanged(payload))
            .expect("serializes");
        let envelope = build_live_envelope("flexcore.v1.telemetry.telemetry_quality_changed", &bytes)
            .expect("decodes");
        assert_eq!(envelope.event_type, "TELEMETRY_QUALITY_CHANGED");
        assert_eq!(envelope.event_name, "TelemetryQualityChanged");
    }

    #[test]
    fn close_ends_the_bus_and_subscribe_fails_closed() {
        let bus = LiveEventBus::new();
        bus.close();
        assert_eq!(bus.subscribe(), Err(LiveBusClosed));
        // Publishing after close is a no-op, never a panic.
        bus.publish("flexcore.v1.a.b".to_string(), b"{}".to_vec());
    }

    #[test]
    fn buffered_events_drain_before_close_ends_the_receiver() {
        let bus = LiveEventBus::new();
        let rx = bus.subscribe().expect("open bus subscribes");
        bus.publish("flexcore.v1.telemetry.telemetry_received".to_string(), telemetry_event(TENANT_A));
        bus.publish("flexcore.v1.telemetry.telemetry_received".to_string(), telemetry_event(TENANT_B));
        bus.close();
        let mut rx = rx;
        // The receiver drains BOTH buffered messages before reporting
        // Closed — the drain-first contract the SSE handler relies on.
        assert!(tokio::runtime::Runtime::new()
            .expect("test runtime")
            .block_on(async {
                rx.recv().await.is_ok() && rx.recv().await.is_ok() && rx.recv().await.is_err()
            }));
    }
}
