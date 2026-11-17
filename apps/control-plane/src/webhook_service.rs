// P13.09 — Outbound webhook DELIVERY service: delivery bookkeeping under an
//          idempotency key, book-before-attempt discipline, bounded
//          exponential-backoff retries, a dead-letter table with a depth
//          metric, and the endpoint/rolling-secret model.
//
// Phase 13 wave 1, Agent B. WRITTEN-UNRUN: no delivery has ever been made,
// no attempt has ever been booked, no dead letter has ever landed. The
// first human build/test is the gate (program conventions §1). The actual
// HTTP transmission is a PORT ([`OutboundHttpPost`]) the composition
// injects — this batch adds no HTTP-client dependency and performs no I/O.
//
// Contract source: plan §2.3 (the webhook delivery contract) and the
// ledger P13.09 row. Signing/verification lives in [`crate::webhook_signing`]
// (the same subtask); the secret FORMAT is Agent A's record
// `08_p13_08_api_keys_webhook_secret_format.md` §4 (`whsec_<43 unpadded
// base64url>`, 32 bytes, strict parse) CONSUMED by citation — never
// redefined here (plan §2.1 ordering). The material itself never enters
// this module's types: endpoints carry provider REFERENCES and the
// [`WebhookSecretResolver`] port resolves them to bytes at attempt time
// (record 08 §4's storage rule — material in the P11.16 posture, never
// the database).
//
// The book-BEFORE-attempt rule (plan §2.3, "Retries" row): every attempt is
// booked in Postgres BEFORE the HTTP call is made, under the delivery's
// idempotency key. Concretely: the caller appends an attempt row
// (outcome `BOOKED`) through [`WebhookDeliveryStore::book_attempt`], and
// only after that row is durably booked does the dispatcher call the HTTP
// port; the attempt row is then updated to its outcome. A crash between
// booking and calling therefore leaves an honestly-stale `BOOKED` row (the
// recovery scan treats over-age `BOOKED` rows as retryable — see
// [`STALE_BOOKED_TIMEOUT`]), never an unbooked transmission. Duplicate
// bookings under one idempotency key collapse to one delivery row.
//
// Handler idempotency (plan §2.3, last row): receivers may observe
// duplicates; handlers must be idempotent — asserted by the P13.19
// idempotent-handler tests (wave 2). This module does not deduplicate on
// the receiver's behalf; it only guarantees the SENDER books every attempt.

use std::collections::BTreeMap;
use std::sync::Mutex;

use flexcore_domain::ids::TenantId;

use crate::webhook_signing::{WebhookSecretSet, WebhookSigningError, WebhookSigningKey};

/// The closed delivery-status vocabulary (the state machine a delivery row
/// walks; every transition is named in this module's dispatcher).
pub const DELIVERY_STATUSES: [&str; 4] = ["PENDING", "IN_FLIGHT", "DELIVERED", "DEAD_LETTERED"];

/// The closed attempt-outcome vocabulary. `BOOKED` is written BEFORE the
/// HTTP call (the book-before-attempt rule); the row is then updated to its
/// observed outcome.
pub const ATTEMPT_OUTCOMES: [&str; 4] = [
    "BOOKED",
    "SUCCESS",
    "RETRYABLE_FAILURE",
    "FATAL_FAILURE",
];

/// The bounded attempt count (plan §2.3: "exponential backoff with a
/// bounded attempt count"). An attempt sequence beyond this bound
/// dead-letters the delivery.
///
/// REGISTERED DEFAULT (X-8 discipline: registered before any run that could
/// depend on it), pending operator tuning — no delivery has ever been made
/// against any receiver, so no observed success-rate distribution exists.
/// Changing the value is a recorded amendment to the P13.09 decision
/// record.
pub const MAX_ATTEMPTS: u32 = 10;

/// The base delay of the exponential backoff schedule, in seconds.
/// REGISTERED DEFAULT, same discipline as [`MAX_ATTEMPTS`].
pub const BACKOFF_BASE_SECS: u64 = 2;

/// The backoff growth factor (exponential, base 2). The schedule is
/// `BACKOFF_BASE_SECS * 2^(attempt-1)`, capped at [`BACKOFF_CAP_SECS`].
pub const BACKOFF_FACTOR: u32 = 2;

/// The backoff cap, in seconds: the delay stops growing at one attempt
/// every five minutes even though the schedule would keep doubling.
/// REGISTERED DEFAULT.
pub const BACKOFF_CAP_SECS: u64 = 300;

/// How long an `IN_FLIGHT`-or-`BOOKED` attempt may stay unresolved before
/// the recovery scan treats it as stale and retryable, in milliseconds.
/// A crash between booking and outcome leaves exactly this state; the
/// value bounds the honest-stale window (registered default; the value is
/// a recovery-latency budget, not a timeout the HTTP port enforces — an
/// in-flight call outliving it may still complete, and the outcome update
/// wins over the recovery re-drive because the attempt row is keyed).
pub const STALE_BOOKED_TIMEOUT_MILLIS: i64 = 60_000;

/// The registered dead-letter depth alert threshold: the metric
/// `flexcore_webhook_dead_letters` (rendered by the single P1.17 metrics
/// registry, `crate::metrics`) is a gauge; its alert threshold is an
/// OPERATOR DECISION on the monitoring side (registered here as a
/// RECOMMENDATION so the P11.02 SLO-register pattern has a number to bind,
/// never claimed as an approved operating number). The depth is never a
/// silent loss: every dead letter is a row with its reason (plan §2.3).
pub const DEAD_LETTER_ALERT_THRESHOLD_RECOMMENDED: u64 = 100;

/// The pure exponential-backoff function: the delay before attempt
/// `attempt` (1-based), capped at [`BACKOFF_CAP_SECS`]. Attempt 1 has no
/// backoff delay (the first try follows booking immediately).
pub fn backoff_delay_secs(attempt: u32) -> u64 {
    if attempt <= 1 {
        return 0;
    }
    // 2^(attempt-2) growth from the base: attempt 2 delays
    // BACKOFF_BASE_SECS, attempt 3 twice that, and so on; saturating
    // arithmetic caps any overflow, then the cap bounds the schedule.
    let exponent = attempt - 2;
    let delay = (BACKOFF_BASE_SECS as u64)
        .saturating_mul(1u64 << exponent.min(62))
        .min(BACKOFF_CAP_SECS);
    delay
}

/// Whether attempt `attempt` may still be made (the bounded attempt count).
pub fn attempt_allowed(attempt: u32) -> bool {
    attempt >= 1 && attempt <= MAX_ATTEMPTS
}

// --- Errors -------------------------------------------------------------------

/// Errors of the delivery service and its store port. Closed vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebhookDeliveryError {
    /// The record argument was malformed (`field` names the problem).
    /// Nothing was written.
    InvalidRecord {
        field: &'static str,
    },
    /// A delivery with the same idempotency key already exists with
    /// CONFLICTING content (same key, different payload/endpoint). The
    /// existing delivery wins — the idempotency key collapses duplicates.
    ConflictingDeliveryKey {
        key: String,
    },
    /// The endpoint does not exist in the caller's tenant scope (existence
    /// never leaks across a tenant boundary, P0.16 rule 5).
    EndpointNotFound,
    /// A referenced delivery does not exist in the caller's tenant scope.
    DeliveryNotFound,
    /// The endpoint has no active secret set (a rolled secret pair whose
    /// overlap closed with no current secret is a configuration fault —
    /// fail closed, never sign with an empty key).
    NoActiveSecret,
    /// The signing layer rejected the operation (the error is the
    /// signing module's, passed through unchanged).
    Signing(WebhookSigningError),
    /// The storage backend failed. `code` and `message` are opaque and
    /// human-safe — never raw storage text.
    Backend {
        code: String,
        message: String,
    },
}

impl std::fmt::Display for WebhookDeliveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebhookDeliveryError::InvalidRecord { field } => {
                write!(f, "the webhook delivery record is invalid: '{field}' is missing or malformed")
            }
            WebhookDeliveryError::ConflictingDeliveryKey { key } => {
                write!(f, "a delivery with idempotency key {key} already exists with different content")
            }
            WebhookDeliveryError::EndpointNotFound => {
                write!(f, "the webhook endpoint does not exist in this tenant scope")
            }
            WebhookDeliveryError::DeliveryNotFound => {
                write!(f, "the delivery does not exist in this tenant scope")
            }
            WebhookDeliveryError::NoActiveSecret => {
                write!(f, "the endpoint has no active signing secret (configuration fault)")
            }
            WebhookDeliveryError::Signing(err) => write!(f, "webhook signing failed: {err}"),
            WebhookDeliveryError::Backend { code, message } => {
                write!(f, "storage backend failure ({code}): {message}")
            }
        }
    }
}

impl std::error::Error for WebhookDeliveryError {}

// --- Rows ---------------------------------------------------------------------

/// One registered webhook endpoint (the P13.09 endpoint model; the admin
/// UI that manages endpoints is P13.18, wave 2 — this type is the data the
/// UI would render).
///
/// SECRET HANDLING (the P13.08 → P13.09 seam, RESOLVED by record
/// `08_p13_08_api_keys_webhook_secret_format.md` §4 — cited, never
/// redefined): the secret MATERIAL never lives in this model, the bookkeeping
/// store, or the database. The endpoint row carries opaque PROVIDER
/// REFERENCES (into the deployment's P11.16 secrets posture) plus
/// fingerprints; the material is resolved at attempt time through the
/// [`WebhookSecretResolver`] port, which returns the decoded 32 bytes (the
/// `whsec_<43 unpadded base64url>` literal form of record 08 §4 is the
/// resolver composition's input format — its strict parse rule is that
/// record's, consumed there, not re-implemented here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookEndpointRecord {
    /// The endpoint identifier (opaque non-empty text, as stored).
    pub endpoint_id: String,
    /// The owning tenant (P0.16 rule 1 — RLS and the explicit filter
    /// agree).
    pub tenant_id: TenantId,
    /// The delivery URL (https assumed by deployment policy; this module
    /// does not police the scheme — the outbound HTTP port and the
    /// deployment configuration own egress rules).
    pub url: String,
    /// The CURRENT secret's provider reference (opaque; the material
    /// never travels through this type).
    pub secret_current_ref: String,
    /// The PREVIOUS secret's provider reference while a rolling overlap
    /// is open; `None` once the overlap closes. The overlap DURATION is
    /// deployment configuration (plan §2.3).
    pub secret_previous_ref: Option<String>,
    /// Whether the endpoint accepts deliveries.
    pub active: bool,
}

/// The secret-material resolver port: maps a provider reference to the
/// DECODED signing bytes at attempt time. The composition binds the real
/// resolver to the P11.16 secrets posture (record
/// `32_p11_16_secrets_management.md` — consumed, never re-decided); the
/// reference resolution NEVER logs the material and NEVER persists it.
/// The decoded length MUST be at least
/// [`crate::webhook_signing::SECRET_MIN_BYTES`] — which, per record 08 §4,
/// is EXACTLY 32 bytes for the defined format.
pub trait WebhookSecretResolver: Send + Sync {
    fn resolve(&self, reference: &str) -> Result<Vec<u8>, WebhookDeliveryError>;
}

impl WebhookEndpointRecord {
    /// Builds the rolling secret set for this endpoint through the
    /// resolver, failing closed when no usable current secret exists.
    pub fn signing_set(
        &self,
        secrets: &dyn WebhookSecretResolver,
    ) -> Result<WebhookSecretSet, WebhookDeliveryError> {
        let current_material = secrets.resolve(&self.secret_current_ref)?;
        let current = WebhookSigningKey::from_bytes(&current_material)
            .map_err(WebhookDeliveryError::Signing)?;
        match &self.secret_previous_ref {
            Some(previous_ref) => {
                let previous_material = secrets.resolve(previous_ref)?;
                let previous = WebhookSigningKey::from_bytes(&previous_material)
                    .map_err(WebhookDeliveryError::Signing)?;
                Ok(WebhookSecretSet::rolling(current, previous))
            }
            None => Ok(WebhookSecretSet::single(current)),
        }
    }
}

/// One outbound delivery (the bookkeeping unit; one row per idempotency
/// key per tenant).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookDeliveryRecord {
    /// The delivery row identifier (opaque non-empty text).
    pub delivery_id: String,
    /// The owning tenant.
    pub tenant_id: TenantId,
    /// The delivery IDEMPOTENCY KEY (plan §2.3): unique per tenant; every
    /// booking under an existing key collapses to the existing row.
    pub delivery_key: String,
    /// The target endpoint.
    pub endpoint_id: String,
    /// The event subject/type the payload carries (a flexcore event
    /// subject per the frozen §15.3 vocabulary's subject rules — the
    /// payload is the RAW body exactly as transmitted).
    pub event_subject: String,
    /// The RAW request body (the exact bytes the HTTP call transmits; the
    /// signature is computed over these bytes — never a re-serialization).
    pub raw_body: String,
    /// The current status (the closed [`DELIVERY_STATUSES`] vocabulary).
    pub status: String,
    /// The next attempt sequence to make (1-based). `attempt_count` is
    /// derivable: `next_attempt_seq - 1` attempts have concluded.
    pub next_attempt_seq: u32,
    /// Unix millis of the earliest time the next attempt may run (the
    /// backoff schedule's output); `None` = due immediately.
    pub next_attempt_not_before_unix_millis: Option<i64>,
    /// The most recent HTTP response status code, when one was received.
    pub last_response_code: Option<u16>,
}

impl WebhookDeliveryRecord {
    /// Validates the closed vocabularies and the key/non-empty rules.
    pub fn validate(&self) -> Result<(), WebhookDeliveryError> {
        for (field, value) in [
            ("delivery_id", self.delivery_id.as_str()),
            ("delivery_key", self.delivery_key.as_str()),
            ("endpoint_id", self.endpoint_id.as_str()),
            ("event_subject", self.event_subject.as_str()),
            ("status", self.status.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(WebhookDeliveryError::InvalidRecord { field });
            }
        }
        if !DELIVERY_STATUSES.contains(&self.status.as_str()) {
            return Err(WebhookDeliveryError::InvalidRecord { field: "status" });
        }
        if self.next_attempt_seq < 1 {
            return Err(WebhookDeliveryError::InvalidRecord { field: "next_attempt_seq" });
        }
        Ok(())
    }
}

/// One booked attempt row (the book-before-attempt record).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookAttemptRecord {
    /// The attempt row identifier (opaque non-empty text). This is the
    /// caller's bookkeeping HANDLE — the in-memory fake's map key and a
    /// log correlation id. The Postgres implementation generates the
    /// attempt row's own identity as a D-S1 `uuidv7()` (the 0020 column
    /// default is the recorded fallback; FRV1-R5-008) and keys outcomes
    /// by the UNIQUE `(delivery_id, attempt_seq)` constraint instead —
    /// see [`WebhookDeliveryStore::record_attempt_outcome`].
    pub attempt_id: String,
    /// The delivery this attempt belongs to.
    pub delivery_id: String,
    /// The 1-based attempt sequence.
    pub attempt_seq: u32,
    /// The outcome (the closed [`ATTEMPT_OUTCOMES`] vocabulary; `BOOKED`
    /// until the call concludes).
    pub outcome: String,
    /// Unix millis when the attempt was BOOKED (before the call — the
    /// contract's ordering evidence).
    pub booked_at_unix_millis: i64,
    /// Unix millis when the call concluded, when it has.
    pub concluded_at_unix_millis: Option<i64>,
    /// The HTTP response status code, when one was received.
    pub response_code: Option<u16>,
    /// The failure classification, when the outcome was a failure (closed
    /// free of free text: `TIMEOUT`, `CONNECT_ERROR`, `NON_2XX`, `TLS`,
    /// `UNKNOWN` — operators alert on classes, never on payloads).
    pub failure_class: Option<String>,
}

impl WebhookAttemptRecord {
    /// Validates the closed vocabularies.
    pub fn validate(&self) -> Result<(), WebhookDeliveryError> {
        for (field, value) in
            [("attempt_id", self.attempt_id.as_str()), ("delivery_id", self.delivery_id.as_str())]
        {
            if value.trim().is_empty() {
                return Err(WebhookDeliveryError::InvalidRecord { field });
            }
        }
        if !ATTEMPT_OUTCOMES.contains(&self.outcome.as_str()) {
            return Err(WebhookDeliveryError::InvalidRecord { field: "outcome" });
        }
        if self.attempt_seq < 1 {
            return Err(WebhookDeliveryError::InvalidRecord { field: "attempt_seq" });
        }
        match self.failure_class.as_deref() {
            None => {}
            Some(class) => {
                if !["TIMEOUT", "CONNECT_ERROR", "NON_2XX", "TLS", "UNKNOWN"]
                    .contains(&class)
                {
                    return Err(WebhookDeliveryError::InvalidRecord { field: "failure_class" });
                }
            }
        }
        Ok(())
    }
}

/// One dead-letter row: a delivery that exhausted its retries (plan §2.3:
/// "nothing is silently dropped").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookDeadLetterRecord {
    /// The dead-letter row identifier (opaque non-empty text). Like the
    /// attempt handle above: the memory fake's map key; the Postgres
    /// implementation generates the row's own identity as a D-S1
    /// `uuidv7()` and keys the one-per-delivery rule on the UNIQUE
    /// `(delivery_id)` constraint (migration 0020).
    pub dead_letter_id: String,
    /// The delivery that dead-lettered (one dead letter per delivery).
    pub delivery_id: String,
    /// The owning tenant.
    pub tenant_id: TenantId,
    /// The target endpoint.
    pub endpoint_id: String,
    /// The reason the delivery exhausted (closed classes as in
    /// [`WebhookAttemptRecord`]'s failure classes, plus `ATTEMPTS_EXHAUSTED`).
    pub reason: String,
    /// Unix millis when the delivery dead-lettered.
    pub dead_lettered_at_unix_millis: i64,
    /// Unix millis when an operator re-drove or dismissed the dead letter,
    /// when that has happened (re-drive is an OPERATOR action, wave-2
    /// admin surface P13.18 — the re-drive re-enters the same state machine
    /// as a NEW delivery booking under a NEW idempotency key; the dead
    /// letter is never silently deleted).
    pub resolved_at_unix_millis: Option<i64>,
}

impl WebhookDeadLetterRecord {
    /// Validates the closed reason classes.
    pub fn validate(&self) -> Result<(), WebhookDeliveryError> {
        for (field, value) in [
            ("dead_letter_id", self.dead_letter_id.as_str()),
            ("delivery_id", self.delivery_id.as_str()),
            ("reason", self.reason.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(WebhookDeliveryError::InvalidRecord { field });
            }
        }
        if ![
            "ATTEMPTS_EXHAUSTED",
            "TIMEOUT",
            "CONNECT_ERROR",
            "NON_2XX",
            "TLS",
            "UNKNOWN",
        ]
        .contains(&self.reason.as_str())
        {
            return Err(WebhookDeliveryError::InvalidRecord { field: "reason" });
        }
        Ok(())
    }
}

// --- The HTTP port ------------------------------------------------------------

/// The outcome of one outbound HTTP POST (the port's whole vocabulary; the
/// port never retries and never logs the body).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpPostOutcome {
    /// A response was received (`code` is the HTTP status; 2xx = success).
    Responded {
        code: u16,
    },
    /// The call failed before a response (transport-level).
    TransportFailure {
        /// The closed failure class (`TIMEOUT`, `CONNECT_ERROR`, `TLS`,
        /// `UNKNOWN`).
        class: String,
    },
}

impl HttpPostOutcome {
    /// `true` when the delivery succeeded (2xx).
    pub fn is_success(&self) -> bool {
        matches!(self, HttpPostOutcome::Responded { code } if (200..300).contains(code))
    }

    /// The failure classification for the bookkeeping row (2xx responses
    /// have none; non-2xx are `NON_2XX`; transport failures carry their
    /// class).
    pub fn failure_class(&self) -> Option<String> {
        match self {
            HttpPostOutcome::Responded { code } if (200..300).contains(code) => None,
            HttpPostOutcome::Responded { .. } => Some("NON_2XX".to_string()),
            HttpPostOutcome::TransportFailure { class } => Some(class.clone()),
        }
    }

    /// `true` when the receiver itself rejected the delivery PERMANENTLY:
    /// a 4xx response other than 408 (request timeout) and 429 (too many
    /// requests). Retrying a permanent rejection only burns the attempt
    /// budget against a receiver that has said no — the attempt outcome is
    /// `FATAL_FAILURE` and the delivery dead-letters immediately.
    /// Transport failures and 408/429/5xx remain retryable (plan §2.3's
    /// bounded backoff exists for exactly those).
    ///
    /// (Final review FRV1-R2-008: `FATAL_FAILURE` was a validated-but-
    /// never-produced vocabulary value; this classifier produces it.)
    pub fn is_permanent_rejection(&self) -> bool {
        matches!(self, HttpPostOutcome::Responded { code }
            if (400..500).contains(code) && *code != 408 && *code != 429)
    }
}

/// The outbound HTTP POST port. The composition root injects the real
/// client (no HTTP-client dependency is added by this batch; the port is
/// the only I/O seam). Implementations MUST NOT retry (the dispatcher owns
/// the retry schedule) and MUST NOT log or persist the body.
pub trait OutboundHttpPost: Send + Sync {
    /// POSTs `raw_body` to `url` with the given headers (name/value pairs;
    /// the signature header among them). Returns the outcome; never panics
    /// on receiver misbehavior.
    fn post(
        &self,
        url: &str,
        headers: &[(String, String)],
        raw_body: &[u8],
    ) -> Result<HttpPostOutcome, WebhookDeliveryError>;
}

/// The fail-closed UNWIRED transport (FRV1-R8-005): the composed stand-in
/// for the real HTTP client. The workspace has NO HTTP-client dependency
/// (grep-verified for the finding; the record `21_p13_09` §5 names the
/// real client as the composition step), so this seam REFUSES every
/// delivery with the named backend error instead of pretending a
/// transmission happened — an unwired seam stays recorded, never faked.
///
/// Effect on the dispatcher's bookkeeping: `run_attempt` books the
/// attempt row (BOOKED) before calling this transport; the refusal
/// propagates as the named error BEFORE any outcome is recorded, so the
/// row stays honestly `BOOKED` and the stale-booking recovery scan
/// (`STALE_BOOKED_TIMEOUT_MILLIS`) treats it as retryable — no fake
/// outcome is ever written. NOTE: the port's closed outcome vocabulary
/// (`ATTEMPT_OUTCOMES`) and failure-class vocabulary have no
/// `REFUSED_UNWIRED` value; adding one would amend a frozen bookkeeping
/// vocabulary for a seam that has never delivered, so the refusal is the
/// error path, not an outcome row.
#[derive(Debug, Default, Clone, Copy)]
pub struct UnwiredOutboundHttpPost;

impl OutboundHttpPost for UnwiredOutboundHttpPost {
    fn post(
        &self,
        url: &str,
        _headers: &[(String, String)],
        _raw_body: &[u8],
    ) -> Result<HttpPostOutcome, WebhookDeliveryError> {
        Err(WebhookDeliveryError::Backend {
            code: "E_TRANSPORT_UNWIRED".to_string(),
            message: format!(
                "no outbound HTTP transport is composed for webhook delivery; the request to \
                 {url} was refused by the unwired transport seam (FRV1-R8-005)"
            ),
        })
    }
}

// --- The store port -----------------------------------------------------------

/// The delivery-bookkeeping persistence contract (the storage design this
/// subtask lands; the Postgres DDL is migration 0020
/// `crates/adapters/postgres/migrations/0020_webhook_delivery_p13_09.*.sql`,
/// expand-only per the P11.18 policy in
/// `crates/adapters/postgres/MIGRATION_POLICY.md`).
///
/// The Postgres implementation lives in
/// `crates/adapters/postgres/src/webhook_delivery.rs` (on `FlexCoreStore`,
/// the recorded edge-enrollment pattern: a standalone store that opens its
/// own tenant-scoped unit of work per call, OUTSIDE the per-request UoW
/// bundle — delivery bookkeeping is background-store state, not
/// request-transaction state). FRV1-R8-005 (final review): the
/// implementation, the fail-closed [`UnwiredOutboundHttpPost`] transport
/// seam, and the composition-root registration are LANDED here; what
/// stays a recorded follow-up is the hosting loop (the scan cadence) and
/// the secret-resolver composition at the P11.16 seam — see the P13.09
/// decision record §5.
///
/// TENANT THREADING (FRV1-R8-005, the 0020 RLS reality): the four
/// webhook tables carry `FORCE ROW LEVEL SECURITY` (migration 0020), so
/// every delivery-scoped operation names its tenant explicitly — the
/// adapter opens its unit of work with that tenant's context and the
/// `tenant_isolation` policy applies. A delivery row always carries its
/// tenant, and the dispatcher has it at every call site; a foreign-tenant
/// row reads as absent (P0.16 rule 5). The two cross-tenant operations
/// (`list_due_deliveries`, `count_dead_letters`) iterate the registered
/// tenants per-tenant inside the adapter (the P11.20 stuck-scan
/// `tenant_ids` pattern) — no statement ever runs without a tenant scope.
pub trait WebhookDeliveryStore: Send + Sync {
    /// Books a NEW delivery under the idempotency key, or returns the
    /// EXISTING delivery when the key is already booked (the idempotency
    /// rule). `created_at_unix_millis` is the injected clock's now.
    fn book_delivery(
        &self,
        record: WebhookDeliveryRecord,
    ) -> Result<WebhookDeliveryRecord, WebhookDeliveryError>;

    /// Marks a delivery `IN_FLIGHT` (an attempt is about to run). The
    /// tenant scopes the update (RLS); a foreign-tenant id reads as
    /// [`WebhookDeliveryError::DeliveryNotFound`].
    fn mark_in_flight(
        &self,
        tenant_id: &TenantId,
        delivery_id: &str,
    ) -> Result<(), WebhookDeliveryError>;

    /// Appends the BOOKED attempt row BEFORE the HTTP call. The booking is
    /// durable before this returns (the contract's ordering rule). The
    /// tenant scopes the insert (RLS).
    fn book_attempt(
        &self,
        tenant_id: &TenantId,
        record: WebhookAttemptRecord,
    ) -> Result<(), WebhookDeliveryError>;

    /// Updates the booked attempt row to its outcome, keyed by the
    /// delivery's UNIQUE `(delivery_id, attempt_seq)` constraint (the
    /// 0020 row identity; the caller's `attempt_id` handle is the memory
    /// fake's map key and a log correlation id, not the Postgres row
    /// identity — the Postgres attempt row's id is generated by the
    /// adapter as a D-S1 `uuidv7()`).
    fn record_attempt_outcome(
        &self,
        tenant_id: &TenantId,
        delivery_id: &str,
        attempt_seq: u32,
        outcome: &str,
        response_code: Option<u16>,
        failure_class: Option<String>,
        concluded_at_unix_millis: i64,
    ) -> Result<(), WebhookDeliveryError>;

    /// Sets the delivery's post-attempt status and next-attempt schedule.
    /// The tenant scopes the update (RLS).
    fn set_delivery_schedule(
        &self,
        tenant_id: &TenantId,
        delivery_id: &str,
        status: &str,
        next_attempt_seq: u32,
        next_attempt_not_before_unix_millis: Option<i64>,
        last_response_code: Option<u16>,
    ) -> Result<(), WebhookDeliveryError>;

    /// Lists deliveries due for an attempt at `now_unix_millis` whose
    /// status is `PENDING` or `IN_FLIGHT`-with-stale-booking (the recovery
    /// scan's feed; bounded listing — the P2.12/P3.28/P4.30 convention).
    /// Cross-tenant by design (the scan feeds the platform's hosting
    /// loop); the Postgres implementation walks the registered tenants
    /// per-tenant under RLS and merges.
    fn list_due_deliveries(
        &self,
        now_unix_millis: i64,
        stale_booked_before_unix_millis: i64,
        limit: u32,
    ) -> Result<Vec<WebhookDeliveryRecord>, WebhookDeliveryError>;

    /// Promotes an exhausted delivery to the dead-letter table. The
    /// record's own tenant scopes the insert (RLS).
    fn promote_to_dead_letter(
        &self,
        record: WebhookDeadLetterRecord,
    ) -> Result<(), WebhookDeliveryError>;

    /// The dead-letter depth (the metric's source; platform-wide — the
    /// depth is an operations signal, not tenant data). The Postgres
    /// implementation sums the per-tenant unresolved depths under RLS.
    fn count_dead_letters(&self) -> Result<u64, WebhookDeliveryError>;
}

// --- The dispatcher -----------------------------------------------------------

/// The delivery dispatcher: drives booked deliveries through the
/// attempt/dead-letter state machine over the injected ports. Constructed
/// sealed (all fields at construction; no builder creep).
pub struct WebhookDispatcher<'a> {
    store: &'a dyn WebhookDeliveryStore,
    http: &'a dyn OutboundHttpPost,
    secrets: &'a dyn WebhookSecretResolver,
    clock_millis: Box<dyn Fn() -> i64 + 'a>,
    endpoints: &'a dyn Fn(&TenantId, &str) -> Result<WebhookEndpointRecord, WebhookDeliveryError>,
}

impl<'a> WebhookDispatcher<'a> {
    /// Composes the dispatcher over its ports and the injected clock. The
    /// secret resolver resolves the endpoint's provider references into
    /// signing material at attempt time (never earlier, never persisted).
    pub fn new(
        store: &'a dyn WebhookDeliveryStore,
        http: &'a dyn OutboundHttpPost,
        secrets: &'a dyn WebhookSecretResolver,
        clock_millis: Box<dyn Fn() -> i64 + 'a>,
        endpoints: &'a dyn Fn(&TenantId, &str) -> Result<WebhookEndpointRecord, WebhookDeliveryError>,
    ) -> Self {
        Self { store, http, secrets, clock_millis, endpoints }
    }

    /// Books a new delivery (or returns the existing one under the same
    /// idempotency key) with status `PENDING`, due immediately. The RAW
    /// body is stored exactly as given; the signature is computed at
    /// attempt time over these bytes (plan §2.3).
    pub fn enqueue(
        &self,
        tenant_id: TenantId,
        delivery_key: String,
        endpoint_id: String,
        event_subject: String,
        raw_body: String,
        delivery_id: String,
    ) -> Result<WebhookDeliveryRecord, WebhookDeliveryError> {
        let record = WebhookDeliveryRecord {
            delivery_id,
            tenant_id,
            delivery_key,
            endpoint_id,
            event_subject,
            raw_body,
            status: "PENDING".to_string(),
            next_attempt_seq: 1,
            next_attempt_not_before_unix_millis: None,
            last_response_code: None,
        };
        record.validate()?;
        self.store.book_delivery(record)
    }

    /// Runs ONE attempt against ONE delivery: books the attempt row
    /// (BEFORE the call), signs the RAW body with the endpoint's rolling
    /// secret set, makes the HTTP call, records the outcome, and advances
    /// the delivery (delivered / scheduled / dead-lettered). Returns the
    /// delivery's post-attempt status.
    ///
    /// The attempt loop itself (scanning `list_due_deliveries` and calling
    /// this per due delivery) is the hosting task the composition runs —
    /// the scheduler cadence is deployment configuration, recorded in the
    /// P13.09 decision record.
    pub fn run_attempt(
        &self,
        delivery: &WebhookDeliveryRecord,
    ) -> Result<String, WebhookDeliveryError> {
        delivery.validate()?;
        let now = (self.clock_millis)();
        let seq = delivery.next_attempt_seq;
        if !attempt_allowed(seq) {
            return Err(WebhookDeliveryError::InvalidRecord { field: "next_attempt_seq" });
        }
        let endpoint = (self.endpoints)(&delivery.tenant_id, &delivery.endpoint_id)?;
        if !endpoint.active {
            return Err(WebhookDeliveryError::EndpointNotFound);
        }
        let signing_set = endpoint.signing_set(self.secrets)?;

        // BOOK BEFORE ATTEMPT (plan §2.3): the attempt row is durably
        // booked with outcome BOOKED before the HTTP call is made.
        let attempt_id = format!("att:{}:{}", delivery.delivery_id, seq);
        let booking = WebhookAttemptRecord {
            attempt_id: attempt_id.clone(),
            delivery_id: delivery.delivery_id.clone(),
            attempt_seq: seq,
            outcome: "BOOKED".to_string(),
            booked_at_unix_millis: now,
            concluded_at_unix_millis: None,
            response_code: None,
            failure_class: None,
        };
        booking.validate()?;
        self.store.book_attempt(&delivery.tenant_id, booking)?;
        self.store.mark_in_flight(&delivery.tenant_id, &delivery.delivery_id)?;

        // Sign over the RAW body bytes exactly as transmitted.
        let timestamp_secs = now.div_euclid(1_000);
        let signature_header = signing_set.sign(timestamp_secs, delivery.raw_body.as_bytes());
        let headers = vec![
            ("content-type".to_string(), "application/json".to_string()),
            (crate::webhook_signing::scheme::SIGNATURE_HEADER.to_string(), signature_header),
        ];
        let outcome = self
            .http
            .post(&endpoint.url, &headers, delivery.raw_body.as_bytes())?;

        // Record the observed outcome on the booked row.
        let response_code = match &outcome {
            HttpPostOutcome::Responded { code } => Some(*code),
            HttpPostOutcome::TransportFailure { .. } => None,
        };
        let failure_class = outcome.failure_class();
        let concluded_at = (self.clock_millis)();
        let outcome_text = if outcome.is_success() {
            "SUCCESS"
        } else if outcome.is_permanent_rejection() {
            // FRV1-R2-008: the recorded FATAL_FAILURE outcome — the receiver
            // permanently rejected the delivery; retrying would only burn
            // the attempt budget.
            "FATAL_FAILURE"
        } else {
            "RETRYABLE_FAILURE"
        };
        self.store.record_attempt_outcome(
            &delivery.tenant_id,
            &delivery.delivery_id,
            seq,
            outcome_text,
            response_code,
            failure_class.clone(),
            concluded_at,
        )?;

        if outcome.is_success() {
            self.store.set_delivery_schedule(
                &delivery.tenant_id,
                &delivery.delivery_id,
                "DELIVERED",
                seq + 1,
                None,
                response_code,
            )?;
            return Ok("DELIVERED".to_string());
        }

        // Failure: dead-letter immediately on a permanent rejection
        // (FRV1-R2-008), otherwise schedule the next attempt or dead-letter
        // at the bound.
        if !outcome.is_permanent_rejection() && attempt_allowed(seq + 1) {
            let delay_secs = backoff_delay_secs(seq + 1);
            self.store.set_delivery_schedule(
                &delivery.tenant_id,
                &delivery.delivery_id,
                "PENDING",
                seq + 1,
                Some(concluded_at + (delay_secs as i64) * 1_000),
                response_code,
            )?;
            Ok("PENDING".to_string())
        } else {
            let dead_letter = WebhookDeadLetterRecord {
                dead_letter_id: format!("dl:{}", delivery.delivery_id),
                delivery_id: delivery.delivery_id.clone(),
                tenant_id: delivery.tenant_id,
                endpoint_id: delivery.endpoint_id.clone(),
                reason: failure_class.unwrap_or_else(|| "UNKNOWN".to_string()),
                dead_lettered_at_unix_millis: concluded_at,
                resolved_at_unix_millis: None,
            };
            dead_letter.validate()?;
            self.store.promote_to_dead_letter(dead_letter)?;
            self.store.set_delivery_schedule(
                &delivery.tenant_id,
                &delivery.delivery_id,
                "DEAD_LETTERED",
                seq + 1,
                None,
                response_code,
            )?;
            Ok("DEAD_LETTERED".to_string())
        }
    }

    /// One scan step: due deliveries from the store, one attempt each.
    /// Returns the per-delivery post-attempt statuses. A per-delivery
    /// error is RECORDED (returned) but does not abort the scan — one
    /// poisoned delivery never wedges the queue.
    pub fn run_scan(&self, limit: u32) -> Result<Vec<(String, String)>, WebhookDeliveryError> {
        let now = (self.clock_millis)();
        let stale_before = now - STALE_BOOKED_TIMEOUT_MILLIS;
        let due = self
            .store
            .list_due_deliveries(now, stale_before, limit)?;
        let mut results = Vec::with_capacity(due.len());
        for delivery in due {
            let status = match self.run_attempt(&delivery) {
                Ok(status) => status,
                // A per-delivery error becomes the scan's recorded status
                // for that delivery — never an abort of the whole scan.
                Err(err) => format!("{err}"),
            };
            results.push((delivery.delivery_id.clone(), status));
        }
        Ok(results)
    }
}

// --- The in-memory fake (the test path; the reference implementation) ---------

/// An in-memory [`WebhookDeliveryStore`]: the reference implementation for
/// tests, and the proof the bookkeeping contract needs nothing but the
/// operations above. Ordering: `BTreeMap` keyed by id (deterministic —
/// conventions §6.6 kernel patterns).
#[derive(Default)]
pub struct MemoryWebhookDeliveryStore {
    deliveries: Mutex<BTreeMap<String, WebhookDeliveryRecord>>,
    attempts: Mutex<BTreeMap<String, WebhookAttemptRecord>>,
    dead_letters: Mutex<BTreeMap<String, WebhookDeadLetterRecord>>,
}

impl MemoryWebhookDeliveryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl WebhookDeliveryStore for MemoryWebhookDeliveryStore {
    fn book_delivery(
        &self,
        record: WebhookDeliveryRecord,
    ) -> Result<WebhookDeliveryRecord, WebhookDeliveryError> {
        let mut deliveries = self.deliveries.lock().expect("poisoned mutex in test fake");
        match deliveries.get(&record.delivery_key) {
            Some(existing) => {
                if existing.raw_body == record.raw_body
                    && existing.endpoint_id == record.endpoint_id
                {
                    Ok(existing.clone())
                } else {
                    Err(WebhookDeliveryError::ConflictingDeliveryKey {
                        key: record.delivery_key,
                    })
                }
            }
            None => {
                deliveries.insert(record.delivery_key.clone(), record.clone());
                Ok(record)
            }
        }
    }

    fn mark_in_flight(
        &self,
        tenant_id: &TenantId,
        delivery_id: &str,
    ) -> Result<(), WebhookDeliveryError> {
        let mut deliveries = self.deliveries.lock().expect("poisoned mutex in test fake");
        // The tenant scopes the lookup like the Postgres RLS filter does:
        // a foreign-tenant id reads as absent (P0.16 rule 5).
        let delivery = deliveries
            .values_mut()
            .find(|d| d.delivery_id == delivery_id && d.tenant_id == *tenant_id)
            .ok_or(WebhookDeliveryError::DeliveryNotFound)?;
        delivery.status = "IN_FLIGHT".to_string();
        Ok(())
    }

    fn book_attempt(
        &self,
        _tenant_id: &TenantId,
        record: WebhookAttemptRecord,
    ) -> Result<(), WebhookDeliveryError> {
        record.validate()?;
        self.attempts
            .lock()
            .expect("poisoned mutex in test fake")
            .insert(record.attempt_id.clone(), record);
        Ok(())
    }

    fn record_attempt_outcome(
        &self,
        tenant_id: &TenantId,
        delivery_id: &str,
        attempt_seq: u32,
        outcome: &str,
        response_code: Option<u16>,
        failure_class: Option<String>,
        concluded_at_unix_millis: i64,
    ) -> Result<(), WebhookDeliveryError> {
        // The tenant scopes the operation like the Postgres RLS filter:
        // the delivery must exist in the caller's tenant before its
        // attempt row is updated (P0.16 rule 5).
        let in_scope = self
            .deliveries
            .lock()
            .expect("poisoned mutex in test fake")
            .values()
            .any(|d| d.delivery_id == delivery_id && d.tenant_id == *tenant_id);
        if !in_scope {
            return Err(WebhookDeliveryError::DeliveryNotFound);
        }
        let mut attempts = self.attempts.lock().expect("poisoned mutex in test fake");
        // Keyed by the same (delivery_id, attempt_seq) identity the
        // Postgres UNIQUE constraint carries.
        let attempt = attempts
            .values_mut()
            .find(|a| a.delivery_id == delivery_id && a.attempt_seq == attempt_seq)
            .ok_or(WebhookDeliveryError::DeliveryNotFound)?;
        attempt.outcome = outcome.to_string();
        attempt.response_code = response_code;
        attempt.failure_class = failure_class;
        attempt.concluded_at_unix_millis = Some(concluded_at_unix_millis);
        attempt.validate()?;
        Ok(())
    }

    fn set_delivery_schedule(
        &self,
        tenant_id: &TenantId,
        delivery_id: &str,
        status: &str,
        next_attempt_seq: u32,
        next_attempt_not_before_unix_millis: Option<i64>,
        last_response_code: Option<u16>,
    ) -> Result<(), WebhookDeliveryError> {
        if !DELIVERY_STATUSES.contains(&status) {
            return Err(WebhookDeliveryError::InvalidRecord { field: "status" });
        }
        let mut deliveries = self.deliveries.lock().expect("poisoned mutex in test fake");
        let delivery = deliveries
            .values_mut()
            .find(|d| d.delivery_id == delivery_id && d.tenant_id == *tenant_id)
            .ok_or(WebhookDeliveryError::DeliveryNotFound)?;
        delivery.status = status.to_string();
        delivery.next_attempt_seq = next_attempt_seq;
        delivery.next_attempt_not_before_unix_millis = next_attempt_not_before_unix_millis;
        delivery.last_response_code = last_response_code;
        Ok(())
    }

    fn list_due_deliveries(
        &self,
        now_unix_millis: i64,
        stale_booked_before_unix_millis: i64,
        limit: u32,
    ) -> Result<Vec<WebhookDeliveryRecord>, WebhookDeliveryError> {
        let deliveries = self.deliveries.lock().expect("poisoned mutex in test fake");
        let mut due: Vec<WebhookDeliveryRecord> = deliveries
            .values()
            .filter(|d| {
                if d.status == "DEAD_LETTERED" || d.status == "DELIVERED" {
                    return false;
                }
                match d.next_attempt_not_before_unix_millis {
                    Some(not_before) if not_before > now_unix_millis => false,
                    _ => true,
                }
            })
            .cloned()
            .collect();
        due.sort_by(|a, b| a.delivery_id.cmp(&b.delivery_id));
        due.truncate(limit as usize);
        // RECORDED FAKE LIMITATION: the stale-booking recovery filter
        // (`stale_booked_before_unix_millis`) is a Postgres-implementation
        // concern (a WHERE clause over the attempt rows); the fake serves
        // every due delivery, so the recovery semantics are exercised by
        // the real adapter, not here. Recorded in the P13.09 decision
        // record's wiring section.
        let _ = stale_booked_before_unix_millis;
        Ok(due)
    }

    fn promote_to_dead_letter(
        &self,
        record: WebhookDeadLetterRecord,
    ) -> Result<(), WebhookDeliveryError> {
        record.validate()?;
        self.dead_letters
            .lock()
            .expect("poisoned mutex in test fake")
            .insert(record.dead_letter_id.clone(), record);
        Ok(())
    }

    fn count_dead_letters(&self) -> Result<u64, WebhookDeliveryError> {
        Ok(self
            .dead_letters
            .lock()
            .expect("poisoned mutex in test fake")
            .values()
            .filter(|d| d.resolved_at_unix_millis.is_none())
            .count() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;

    fn tenant(seed: u8) -> TenantId {
        let mut bytes = [0u8; 16];
        bytes[15] = seed;
        TenantId::new(FlexUuid::from_bytes(bytes))
    }

    fn endpoint(tenant_id: TenantId) -> WebhookEndpointRecord {
        WebhookEndpointRecord {
            endpoint_id: "ep-1".to_string(),
            tenant_id,
            url: "https://receiver.example.test/hook".to_string(),
            // A PROVIDER REFERENCE, never material (record 08 §4).
            secret_current_ref: "vault:webhook/ep-1/current".to_string(),
            secret_previous_ref: None,
            active: true,
        }
    }

    /// A fixed-material resolver fake (the test stand-in for the P11.16
    /// posture's composition; material is TEST-ONLY bytes, never a real
    /// secret). Static so a `&'static` reference satisfies any lifetime.
    static RESOLVER: FixedResolver = FixedResolver { material: [0x42u8; 32] };

    struct FixedResolver {
        material: [u8; 32],
    }

    impl WebhookSecretResolver for FixedResolver {
        fn resolve(&self, reference: &str) -> Result<Vec<u8>, WebhookDeliveryError> {
            if reference.starts_with("vault:") {
                Ok(self.material.to_vec())
            } else {
                Err(WebhookDeliveryError::NoActiveSecret)
            }
        }
    }

    /// A recording HTTP fake: every call is captured (url, headers, body).
    struct RecordingHttp {
        calls: Mutex<Vec<(String, Vec<(String, String)>, Vec<u8>)>>,
        outcome: Mutex<HttpPostOutcome>,
    }

    impl RecordingHttp {
        fn succeed() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                outcome: Mutex::new(HttpPostOutcome::Responded { code: 200 }),
            }
        }

        fn fail_non_2xx() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                outcome: Mutex::new(HttpPostOutcome::Responded { code: 500 }),
            }
        }

        fn permanent_rejection() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                // 410 Gone: the receiver said no, permanently.
                outcome: Mutex::new(HttpPostOutcome::Responded { code: 410 }),
            }
        }
    }

    impl OutboundHttpPost for RecordingHttp {
        fn post(
            &self,
            url: &str,
            headers: &[(String, String)],
            raw_body: &[u8],
        ) -> Result<HttpPostOutcome, WebhookDeliveryError> {
            self.calls.lock().expect("poisoned mutex in test fake").push((
                url.to_string(),
                headers.to_vec(),
                raw_body.to_vec(),
            ));
            Ok(self.outcome.lock().expect("poisoned mutex in test fake").clone())
        }
    }

    fn dispatcher<'a>(
        store: &'a MemoryWebhookDeliveryStore,
        http: &'a RecordingHttp,
        now: i64,
    ) -> WebhookDispatcher<'a> {
        let tenant_seed = 7u8;
        let t = tenant(tenant_seed);
        let ep = endpoint(t);
        WebhookDispatcher::new(
            store,
            http,
            &RESOLVER,
            Box::new(move || now),
            Box::new(move |tid, eid| {
                if *tid == t && eid == "ep-1" {
                    Ok(ep.clone())
                } else {
                    Err(WebhookDeliveryError::EndpointNotFound)
                }
            }),
        )
    }

    #[test]
    fn the_backoff_schedule_is_exponential_and_capped() {
        assert_eq!(backoff_delay_secs(1), 0, "the first attempt follows booking immediately");
        assert_eq!(backoff_delay_secs(2), 2);
        assert_eq!(backoff_delay_secs(3), 4);
        assert_eq!(backoff_delay_secs(4), 8);
        assert_eq!(backoff_delay_secs(9), 256);
        // The cap binds: no attempt ever waits longer than the cap.
        assert_eq!(backoff_delay_secs(10), BACKOFF_CAP_SECS);
        assert_eq!(backoff_delay_secs(20), BACKOFF_CAP_SECS);
        // The attempt bound is honored.
        assert!(attempt_allowed(1));
        assert!(attempt_allowed(MAX_ATTEMPTS));
        assert!(!attempt_allowed(MAX_ATTEMPTS + 1));
        assert!(!attempt_allowed(0));
    }

    #[test]
    fn a_delivery_books_once_per_idempotency_key_and_collapses_duplicates() {
        let store = MemoryWebhookDeliveryStore::new();
        let http = RecordingHttp::succeed();
        let disp = dispatcher(&store, &http, 1_000);
        let first = disp
            .enqueue(
                tenant(7),
                "evt:outbox:0001".to_string(),
                "ep-1".to_string(),
                "flexcore.v1.ScheduleProposed".to_string(),
                "{\"event\":\"ScheduleProposed\"}".to_string(),
                "d-1".to_string(),
            )
            .expect("first booking");
        let second = disp
            .enqueue(
                tenant(7),
                "evt:outbox:0001".to_string(),
                "ep-1".to_string(),
                "flexcore.v1.ScheduleProposed".to_string(),
                "{\"event\":\"ScheduleProposed\"}".to_string(),
                "d-1".to_string(),
            )
            .expect("duplicate booking collapses to the existing row");
        assert_eq!(first, second);
        // A different payload under the same key is a CONFLICT.
        let conflicting = disp.enqueue(
            tenant(7),
            "evt:outbox:0001".to_string(),
            "ep-1".to_string(),
            "flexcore.v1.ScheduleProposed".to_string(),
            "{\"event\":\"other\"}".to_string(),
            "d-2".to_string(),
        );
        assert_eq!(
            conflicting,
            Err(WebhookDeliveryError::ConflictingDeliveryKey { key: "evt:outbox:0001".to_string() })
        );
    }

    #[test]
    fn every_attempt_is_booked_before_the_call_and_updated_after() {
        let store = MemoryWebhookDeliveryStore::new();
        let http = RecordingHttp::succeed();
        let disp = dispatcher(&store, &http, 1_000);
        let delivery = disp
            .enqueue(
                tenant(7),
                "evt:outbox:0002".to_string(),
                "ep-1".to_string(),
                "flexcore.v1.ScheduleProposed".to_string(),
                "{\"event\":\"ScheduleProposed\"}".to_string(),
                "d-1".to_string(),
            )
            .expect("booked");
        let status = disp.run_attempt(&delivery).expect("attempt runs");
        assert_eq!(status, "DELIVERED");
        // Exactly one attempt row, concluded as SUCCESS, booked at the
        // injected clock (1_000) and concluded at the same clock.
        let attempts = store.attempts.lock().unwrap();
        assert_eq!(attempts.len(), 1);
        let attempt = attempts.values().next().expect("the attempt row");
        assert_eq!(attempt.attempt_seq, 1);
        assert_eq!(attempt.outcome, "SUCCESS");
        assert_eq!(attempt.booked_at_unix_millis, 1_000);
        assert_eq!(attempt.concluded_at_unix_millis, Some(1_000));
        assert_eq!(attempt.response_code, Some(200));
        // The delivery is DELIVERED with the response recorded.
        let deliveries = store.deliveries.lock().unwrap();
        let d = deliveries.values().next().unwrap();
        assert_eq!(d.status, "DELIVERED");
        assert_eq!(d.last_response_code, Some(200));
    }

    #[test]
    fn a_permanent_rejection_produces_fatal_failure_and_dead_letters_immediately() {
        // FRV1-R2-008: FATAL_FAILURE must be PRODUCED, not merely validated.
        let store = MemoryWebhookDeliveryStore::new();
        let http = RecordingHttp::permanent_rejection();
        let disp = dispatcher(&store, &http, 1_000);
        let delivery = disp
            .enqueue(
                tenant(7),
                "evt:outbox:0003".to_string(),
                "ep-1".to_string(),
                "flexcore.v1.ScheduleProposed".to_string(),
                "{\"event\":\"ScheduleProposed\"}".to_string(),
                "d-1".to_string(),
            )
            .expect("booked");
        let status = disp.run_attempt(&delivery).expect("attempt runs");
        assert_eq!(status, "DEAD_LETTERED", "a permanent rejection never retries");
        // Exactly ONE attempt row (the retry budget is not burned), concluded
        // as FATAL_FAILURE with the rejecting response code recorded.
        let attempts = store.attempts.lock().unwrap();
        assert_eq!(attempts.len(), 1, "no second attempt is scheduled");
        let attempt = attempts.values().next().expect("the attempt row");
        assert_eq!(attempt.outcome, "FATAL_FAILURE");
        assert_eq!(attempt.response_code, Some(410));
        // The delivery is DEAD_LETTERED.
        let deliveries = store.deliveries.lock().unwrap();
        let d = deliveries.values().next().unwrap();
        assert_eq!(d.status, "DEAD_LETTERED");
        // Exactly one HTTP call happened.
        assert_eq!(http.calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn the_permanent_rejection_classifier_spares_the_retryable_4xx_and_transport_failures() {
        // 408 and 429 are receiver-side but RETRYABLE; 5xx is retryable;
        // transport failures never classify as permanent.
        assert!(!HttpPostOutcome::Responded { code: 408 }.is_permanent_rejection());
        assert!(!HttpPostOutcome::Responded { code: 429 }.is_permanent_rejection());
        assert!(!HttpPostOutcome::Responded { code: 500 }.is_permanent_rejection());
        assert!(!HttpPostOutcome::Responded { code: 200 }.is_permanent_rejection());
        assert!(!HttpPostOutcome::TransportFailure { class: "TIMEOUT".to_string() }
            .is_permanent_rejection());
        for code in [400u16, 404, 410, 422] {
            assert!(HttpPostOutcome::Responded { code }.is_permanent_rejection());
        }
    }

    #[test]
    fn the_signature_header_travels_over_the_raw_body() {
        let store = MemoryWebhookDeliveryStore::new();
        let http = RecordingHttp::succeed();
        let disp = dispatcher(&store, &http, 1_700_000_000_000);
        let raw_body = "{\"event\":\"ScheduleProposed\"}".to_string();
        let delivery = disp
            .enqueue(
                tenant(7),
                "k".to_string(),
                "ep-1".to_string(),
                "flexcore.v1.ScheduleProposed".to_string(),
                raw_body.clone(),
                "d-1".to_string(),
            )
            .expect("booked");
        disp.run_attempt(&delivery).expect("attempt runs");
        let calls = http.calls.lock().unwrap();
        let (url, headers, body) = &calls[0];
        assert_eq!(url, "https://receiver.example.test/hook");
        assert_eq!(body.as_slice(), raw_body.as_bytes(), "the RAW body bytes are transmitted");
        let sig = headers
            .iter()
            .find(|(name, _)| name == crate::webhook_signing::scheme::SIGNATURE_HEADER)
            .expect("signature header present");
        // The signature verifies against the endpoint's secret over the
        // exact transmitted bytes at the attempt timestamp.
        let set = endpoint(tenant(7)).signing_set(&RESOLVER).expect("secret set");
        set.verify(&sig.1, 1_700_000_000, body).expect("signature verifies");
    }

    #[test]
    fn failures_schedule_backoff_then_dead_letter_at_the_bound() {
        let store = MemoryWebhookDeliveryStore::new();
        let http = RecordingHttp::fail_non_2xx();
        let disp = dispatcher(&store, &http, 1_000);
        let delivery = disp
            .enqueue(
                tenant(7),
                "k".to_string(),
                "ep-1".to_string(),
                "flexcore.v1.ScheduleProposed".to_string(),
                "{}".to_string(),
                "d-1".to_string(),
            )
            .expect("booked");

        let mut current = delivery;
        let mut statuses = Vec::new();
        for _ in 0..MAX_ATTEMPTS {
            let status = disp.run_attempt(&current).expect("attempt runs");
            statuses.push(status.clone());
            let deliveries = store.deliveries.lock().unwrap();
            current = deliveries.values().next().unwrap().clone();
            drop(deliveries);
            if status == "DEAD_LETTERED" {
                break;
            }
        }
        // The last status is the dead letter; every earlier one scheduled.
        assert_eq!(statuses.last().map(String::as_str), Some("DEAD_LETTERED"));
        assert_eq!(statuses.len(), MAX_ATTEMPTS as usize, "the attempt bound holds");
        // Exactly MAX_ATTEMPTS attempt rows, all RETRYABLE_FAILURE.
        let attempts = store.attempts.lock().unwrap();
        assert_eq!(attempts.len(), MAX_ATTEMPTS as usize);
        assert!(attempts
            .values()
            .all(|a| a.outcome == "RETRYABLE_FAILURE" && a.failure_class.as_deref() == Some("NON_2XX")));
        // The dead letter exists with its reason; the depth metric reads 1.
        let dead = store.dead_letters.lock().unwrap();
        assert_eq!(dead.len(), 1);
        let row = dead.values().next().unwrap();
        assert_eq!(row.reason, "NON_2XX");
        assert_eq!(store.count_dead_letters().unwrap(), 1);
    }

    #[test]
    fn a_dead_lettered_delivery_is_not_rescanned() {
        let store = MemoryWebhookDeliveryStore::new();
        let http = RecordingHttp::fail_non_2xx();
        let disp = dispatcher(&store, &http, 1_000);
        let delivery = disp
            .enqueue(
                tenant(7),
                "k".to_string(),
                "ep-1".to_string(),
                "flexcore.v1.ScheduleProposed".to_string(),
                "{}".to_string(),
                "d-1".to_string(),
            )
            .expect("booked");
        // Drive to the dead letter.
        let mut current = delivery;
        for _ in 0..MAX_ATTEMPTS {
            let _ = disp.run_attempt(&current).expect("attempt runs");
            let deliveries = store.deliveries.lock().unwrap();
            current = deliveries.values().next().unwrap().clone();
            drop(deliveries);
        }
        // The scan no longer returns it.
        let due = store
            .list_due_deliveries(1_000_000, 900_000, 100)
            .expect("scan");
        assert!(due.is_empty(), "dead-lettered deliveries never rescan");
    }

    #[test]
    fn an_inactive_endpoint_fails_closed() {
        let store = MemoryWebhookDeliveryStore::new();
        let http = RecordingHttp::succeed();
        let t = tenant(7);
        let mut ep = endpoint(t);
        ep.active = false;
        let ep_inactive = ep;
        let disp = WebhookDispatcher::new(
            &store,
            &http,
            &RESOLVER,
            Box::new(|| 1_000),
            Box::new(move |tid, eid| {
                if *tid == t && eid == "ep-1" {
                    Ok(ep_inactive.clone())
                } else {
                    Err(WebhookDeliveryError::EndpointNotFound)
                }
            }),
        );
        let delivery = disp
            .enqueue(
                tenant(7),
                "k".to_string(),
                "ep-1".to_string(),
                "flexcore.v1.ScheduleProposed".to_string(),
                "{}".to_string(),
                "d-1".to_string(),
            )
            .expect("booked");
        // The attempt refuses BEFORE any call: no attempt row, no HTTP
        // call (fail closed, never a signed delivery to a disabled
        // endpoint).
        assert!(matches!(
            disp.run_attempt(&delivery),
            Err(WebhookDeliveryError::EndpointNotFound)
        ));
        assert!(http.calls.lock().unwrap().is_empty());
        assert!(store.attempts.lock().unwrap().is_empty());
    }

    #[test]
    fn a_scan_records_per_delivery_errors_without_wedging() {
        let store = MemoryWebhookDeliveryStore::new();
        let http = RecordingHttp::succeed();
        // Endpoint "ep-broken" does not resolve → its delivery errors;
        // endpoint "ep-1" succeeds. One poisoned delivery must not abort
        // the scan.
        let t = tenant(7);
        let ep = endpoint(t);
        let disp = WebhookDispatcher::new(
            &store,
            &http,
            &RESOLVER,
            Box::new(|| 1_000),
            Box::new(move |tid, eid| {
                if *tid == t && eid == "ep-1" {
                    Ok(ep.clone())
                } else {
                    Err(WebhookDeliveryError::EndpointNotFound)
                }
            }),
        );
        disp.enqueue(
            tenant(7),
            "k-good".to_string(),
            "ep-1".to_string(),
            "flexcore.v1.ScheduleProposed".to_string(),
            "{}".to_string(),
            "d-good".to_string(),
        )
        .expect("booked");
        disp.enqueue(
            tenant(7),
            "k-bad".to_string(),
            "ep-broken".to_string(),
            "flexcore.v1.ScheduleProposed".to_string(),
            "{}".to_string(),
            "d-bad".to_string(),
        )
        .expect("booked");
        let results = disp.run_scan(10).expect("scan completes");
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|(id, s)| id == "d-good" && s == "DELIVERED"));
        assert!(results.iter().any(|(id, s)| id == "d-bad" && s.starts_with("the webhook endpoint")));
        // The good delivery delivered despite the poisoned one.
        assert_eq!(http.calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn the_closed_vocabularies_are_exactly_the_registered_sets() {
        assert_eq!(
            DELIVERY_STATUSES,
            ["PENDING", "IN_FLIGHT", "DELIVERED", "DEAD_LETTERED"]
        );
        assert_eq!(
            ATTEMPT_OUTCOMES,
            ["BOOKED", "SUCCESS", "RETRYABLE_FAILURE", "FATAL_FAILURE"]
        );
    }

    #[test]
    fn record_validation_rejects_off_vocabulary_values() {
        let t = tenant(7);
        let mut delivery = WebhookDeliveryRecord {
            delivery_id: "d".to_string(),
            tenant_id: t,
            delivery_key: "k".to_string(),
            endpoint_id: "ep-1".to_string(),
            event_subject: "flexcore.v1.ScheduleProposed".to_string(),
            raw_body: "{}".to_string(),
            status: "SENT".to_string(),
            next_attempt_seq: 1,
            next_attempt_not_before_unix_millis: None,
            last_response_code: None,
        };
        assert_eq!(
            delivery.validate(),
            Err(WebhookDeliveryError::InvalidRecord { field: "status" })
        );
        delivery.status = "PENDING".to_string();
        delivery.next_attempt_seq = 0;
        assert_eq!(
            delivery.validate(),
            Err(WebhookDeliveryError::InvalidRecord { field: "next_attempt_seq" })
        );
        let mut attempt = WebhookAttemptRecord {
            attempt_id: "a".to_string(),
            delivery_id: "d".to_string(),
            attempt_seq: 1,
            outcome: "BOOKED".to_string(),
            booked_at_unix_millis: 1,
            concluded_at_unix_millis: None,
            response_code: None,
            failure_class: Some("mystery".to_string()),
        };
        assert_eq!(
            attempt.validate(),
            Err(WebhookDeliveryError::InvalidRecord { field: "failure_class" })
        );
        attempt.failure_class = None;
        attempt.validate().expect("valid");
        let mut dead = WebhookDeadLetterRecord {
            dead_letter_id: "dl".to_string(),
            delivery_id: "d".to_string(),
            tenant_id: t,
            endpoint_id: "ep-1".to_string(),
            reason: "deleted".to_string(),
            dead_lettered_at_unix_millis: 1,
            resolved_at_unix_millis: None,
        };
        assert_eq!(
            dead.validate(),
            Err(WebhookDeliveryError::InvalidRecord { field: "reason" })
        );
        dead.reason = "ATTEMPTS_EXHAUSTED".to_string();
        dead.validate().expect("valid");
    }
}
