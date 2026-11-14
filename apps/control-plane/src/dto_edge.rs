// P6.06 — The `/api/v1/edge/enroll` DTOs (report §18.1, §16.2; ledger
//         P6.06). ADDITIVE — no existing DTO is modified.
//
// P6.25/P6.32 (ADDITIVE) — the edge-status read DTO (P6.25's per-edge
// status surface, `GET /api/v1/edge/status`) and the decommission DTOs
// (`POST /api/v1/edge/decommission`). See the handler module headers in
// `routes_edge.rs` for the auth models and the fail-closed matrices, and
// `ledger/phase-06-edge-protocol/25_31_32_console_clock_decommission.md`
// for the recorded decisions.
//
// The response carries the enrollment RECORD honestly: with the shipped
// seam NO certificate is issued (`certificate_issued: false` + the record
// note), because certificate issuance/CA and OCSP/CRL revocation checking
// are DEPLOYMENT REQUIREMENTS recorded in
// `ledger/phase-06-edge-protocol/06_07_identity_channel.md` §3 — nothing
// here may look like a working PKI.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The enrollment request body. The edge submits its CONFIGURED binding
/// (tenant + site, §16.2) and its requested device identity; the control
/// plane validates the provisioning token's approved scope against it.
#[derive(Debug, Deserialize, ToSchema)]
pub struct EnrollRequestBody {
    /// The requested device identity (non-empty text; unique per tenant).
    pub device_id: String,
    /// The tenant the edge claims (canonical UUID text; must match the
    /// token's approved scope).
    pub tenant_id: String,
    /// The approved site the edge claims (canonical UUID text; must match
    /// the token's approved scope and exist in the tenant).
    pub site_id: String,
    /// The approved site/asset scope annotation carried with the issuance
    /// (§16.2; free-form deployment annotation).
    #[serde(default)]
    pub approved_asset_scope: Option<String>,
    /// The identity validity term (unix millis), when a real issuance
    /// exists. With the shipped seam this stays `None` and the record says
    /// so honestly — never "valid forever".
    #[serde(default)]
    pub expires_at_unix_millis: Option<i64>,
}

/// The enrollment response: the issued identity RECORD (no secret, no
/// certificate material — the registry stores bookkeeping only, §17.3).
#[derive(Debug, Serialize, ToSchema)]
pub struct EnrollResponseDto {
    pub device_id: String,
    pub tenant_id: String,
    pub site_id: String,
    /// The identity term sequence (1 for a first enrollment; the next
    /// sequence for a rotation after an EXPIRED term).
    pub enrollment_seq: u64,
    /// Always `ACTIVE` on a 201 (REVOKED/EXPIRED terms are read via the
    /// registry, not created by this route).
    pub status: String,
    pub enrolled_at_unix_millis: i64,
    pub expires_at_unix_millis: Option<i64>,
    /// Always `false` with the shipped seam: no CA is implemented —
    /// certificate issuance is a deployment requirement.
    pub certificate_issued: bool,
    /// The honest record statement, verbatim.
    pub record_note: String,
}

// --- P6.25 — the per-edge status read DTOs (`GET /api/v1/edge/status`) ----

/// The status query: the edge identity to report. There is deliberately NO
/// tenant or site parameter — the server derives the tenant from the
/// caller's verified claims (§16.2) and the site from the registry row
/// (the P6.21 no-request-supplied-scope rule).
#[derive(Debug, Deserialize, ToSchema)]
pub struct EdgeStatusParams {
    /// The device identity (non-empty text; must exist in the caller's
    /// tenant scope — an unknown identity is 404, never a guessed scope).
    pub device_id: String,
}

/// The connectivity report of one edge: last-seen plus the SOURCE NOTE.
/// A missing reporter renders the gap (`last_seen_unix_millis: null` and
/// the note says so) — the console never renders a plausible "connected"
/// (the P6.25 risk row: a green indicator that hides a stale feed).
#[derive(Debug, Serialize, ToSchema)]
pub struct EdgeConnectivityReportDto {
    /// When the edge was last seen by the channel/health surface (unix
    /// millis). `null` = never seen / no reporter composed — rendered as
    /// the gap, never as "now".
    pub last_seen_unix_millis: Option<i64>,
    /// Where this connectivity fact comes from (verbatim; honest source).
    pub source_note: String,
}

/// The data-freshness report: the last event time the edge reported plus
/// the freshness basis note (§20.2 — freshness is displayed with its
/// basis, never presented as live without a timestamp).
#[derive(Debug, Serialize, ToSchema)]
pub struct DataFreshnessDto {
    /// The event_time of the last telemetry the edge produced (unix
    /// millis), when reported.
    pub last_event_unix_millis: Option<i64>,
    /// The freshness basis note (verbatim).
    pub note: String,
}

/// The P6.14 command-path state of the edge (the console MUST be able to
/// show that a proposed setpoint is not an executed command, §20.2).
#[derive(Debug, Serialize, ToSchema)]
pub struct CommandPathStateDto {
    /// The closed command-path state vocabulary: `DISABLED` |
    /// `TEST_ENV_ENABLED` | `NOT_REPORTED` (no reporter composed — the
    /// gap is named, never silently filled with `DISABLED`).
    pub state: String,
    /// When a TEST_ENV_ENABLED enablement expires (unix millis) — the
    /// P6.14 gate is time-bounded; the console renders the expiry.
    pub enablement_valid_until_unix_millis: Option<i64>,
    /// The verbatim statement: the console can never enable the command
    /// path; enablement takes effect only through the P6.14 procedure.
    pub note: String,
}

/// The per-edge status record (P6.25). Every optional field is an HONEST
/// gap when absent (`null` + the note) — nothing is defaulted to a
/// plausible healthy-looking value. This surface is POLLING (a plain GET;
/// no live SSE requirement — the recorded decision, decision record §1).
#[derive(Debug, Serialize, ToSchema)]
pub struct EdgeStatusDto {
    pub device_id: String,
    /// The approved site (from the registry row — never from a request).
    pub site_id: String,
    /// The CURRENT identity term sequence (the highest).
    pub enrollment_seq: u64,
    /// The CURRENT identity status (closed registry vocabulary:
    /// ACTIVE / REVOKED / EXPIRED, rendered verbatim).
    pub enrollment_status: String,
    /// When the identity term expires (unix millis), when recorded.
    pub identity_expires_at_unix_millis: Option<i64>,
    /// Certificate expiry (unix millis), when the edge-status reporter
    /// supplies one (`null` = not reported — the gap is named).
    pub certificate_expiry_unix_millis: Option<i64>,
    /// The connectivity report (last-seen + source note).
    pub connectivity: EdgeConnectivityReportDto,
    /// The P6.17 adapter health snapshot, VERBATIM (the edge's own JSON
    /// rendering of the closed HEALTHY / DEGRADED_* / FAILING_* vocabulary
    /// — the console renders it as-is, never prettified into
    /// healthy-looking synonyms). `null` = no reporter composed.
    pub adapter_health_snapshot_json: Option<String>,
    /// Buffered-telemetry depth (records), when reported.
    pub buffer_depth: Option<u64>,
    /// Buffered-telemetry capacity (records), when reported.
    pub buffer_capacity: Option<u64>,
    /// The data-freshness report (last event time + basis note).
    pub data_freshness: DataFreshnessDto,
    /// The P6.14 command-path state (disabled / test-env enabled WITH
    /// expiry).
    pub command_path: CommandPathStateDto,
    /// The honest surface note (polling, seam status).
    pub status_note: String,
}

// --- P6.32 — the decommission DTOs (`POST /api/v1/edge/decommission`) ----

/// The operator-initiated decommission request. The confirmation payload
/// MUST name the edge identity, the tenant, the site, the acting
/// operator, and the confirmation time — a confirmation that cannot name
/// what it confirms is refused (ledger P6.32 step 1). The actor must MATCH
/// the authenticated caller's subject (the step-up binding; a mismatch is
/// a refusal, never a proxy confirmation).
#[derive(Debug, Deserialize, ToSchema)]
pub struct DecommissionRequestBody {
    /// The edge identity to decommission (non-empty).
    pub device_id: String,
    /// The tenant the identity is enrolled under (canonical UUID text;
    /// must match the caller's verified tenant).
    pub tenant_id: String,
    /// The approved site the identity is bound to (canonical UUID text;
    /// must match the registry row — a wrong site is a refusal).
    pub site_id: String,
    /// The acting operator (must match the authenticated caller's subject).
    pub actor: String,
    /// The confirmation instant (unix millis; must be positive).
    pub confirmed_at_unix_millis: i64,
    /// The operator's stated reason (diagnostics; no secret material).
    #[serde(default)]
    pub reason: Option<String>,
}

/// The decommission response: the revocation record. The confirmation
/// RECORD is persisted in the append-only enrollment journal (event type
/// REVOKED, reason = the full confirmation naming identity/tenant/site/
/// actor/time).
#[derive(Debug, Serialize, ToSchema)]
pub struct DecommissionResponseDto {
    pub device_id: String,
    pub tenant_id: String,
    pub site_id: String,
    /// The NEW (revoked) term's sequence (the registry is append-only:
    /// revocation appends a row at the next sequence, migration 0018).
    pub enrollment_seq: u64,
    /// Always `REVOKED` on a 200 (the closed registry vocabulary).
    pub status: String,
    /// The confirmed actor (echoed, matches the authenticated subject).
    pub actor: String,
    pub confirmed_at_unix_millis: i64,
    pub revoked_at_unix_millis: i64,
    /// The honest record statement (what this seam does and does not do —
    /// the edge-side wipe is a separate operator-driven step).
    pub record_note: String,
}
