// P6.06 — Edge device-identity registry + enrollment journal persistence
//         contract (report §18.1, §18.3, §17.3, §16.2; ledger P6.06;
//         migration 0018; decision record:
//         `ledger/phase-06-edge-protocol/06_07_identity_channel.md`).
//
// The control-plane side of the enrollment seam: the tenant-scoped device
// registry (one row per ISSUED identity term — the registry is append-only,
// so rotation and expiry are new rows under an increasing `enrollment_seq`,
// and the CURRENT status of a device is its highest-sequence row) and the
// append-only enrollment journal (every enrollment/rotation/revocation/
// expiry/rejection event, the §17.3 cross-boundary audit evidence).
//
// What this contract deliberately is NOT:
//
// - NOT a certificate store: no key material, no private keys, no
//   certificate PEM columns exist (the CA is a DEPLOYMENT REQUIREMENT; the
//   registry stores the identity RECORD — ids, binding, status, validity
//   window — never secrets, §17.3).
// - NOT a dispatch-state surface: the §15.2 prohibition is untouched; this
//   resource cannot execute anything.
// - NOT an edge-authoritative trust store: the binding recorded here is the
//   authoritative copy (§16.2); the edge stores what was ISSUED to it and
//   every session must be enforced server-side against this registry
//   (P6.21 isolation work).
//
// Append-only discipline (the 0008 audit_records pattern): both tables get
// flexcore_app INSERT + SELECT only, and migration 0018 adds the
// defence-in-depth UPDATE/DELETE trigger on the JOURNAL (the registry's
// lifecycle is also expressed as inserts — new rows at the next
// `enrollment_seq` — so it holds INSERT + SELECT grants too). Tenant
// scoping is the exact 0017 pattern: `tenant_id` on every row, RLS ENABLE +
// FORCE with the shared `tenant_isolation` policy, and every trait method
// carries the caller's `TenantContext`.

use flexcore_domain::ids::TenantId;

/// The closed device-status vocabulary (the CURRENT status of an identity
/// term, as stored text).
pub const EDGE_DEVICE_STATUSES: [&str; 3] = ["ACTIVE", "REVOKED", "EXPIRED"];

/// The closed enrollment-journal event vocabulary (the §17.3 audit events
/// of the identity lifecycle).
pub const EDGE_ENROLLMENT_EVENT_TYPES: [&str; 5] =
    ["ENROLLED", "ROTATED", "REVOKED", "EXPIRED", "REJECTED"];

// --- Errors -------------------------------------------------------------------

/// Errors of the edge-enrollment store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EdgeEnrollmentStoreError {
    /// The row argument is incomplete or malformed (`field` names the
    /// problem). Nothing was written.
    InvalidRecord {
        field: &'static str,
    },
    /// A device with the same (device_id, tenant_id) is already ACTIVE —
    /// no two edges share an identity (ledger P6.06 acceptance).
    Conflict {
        device_id: String,
    },
    /// The device does not exist in the caller's tenant scope (existence
    /// never leaks across a tenant boundary, P0.16 rule 5).
    NotFound {
        device_id: String,
    },
    /// The storage backend failed. `code` and `message` are opaque and
    /// human-safe — never raw storage text.
    Backend {
        code: String,
        message: String,
    },
}

impl std::fmt::Display for EdgeEnrollmentStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdgeEnrollmentStoreError::InvalidRecord { field } => write!(
                f,
                "the edge device record is invalid: '{field}' is missing or malformed"
            ),
            EdgeEnrollmentStoreError::Conflict { device_id } => write!(
                f,
                "device {device_id} is already enrolled and ACTIVE; no two edges share an \
                 identity (ledger P6.06)"
            ),
            EdgeEnrollmentStoreError::NotFound { device_id } => {
                write!(f, "device {device_id} does not exist in this tenant scope")
            }
            EdgeEnrollmentStoreError::Backend { code, message } => {
                write!(f, "storage backend failure ({code}): {message}")
            }
        }
    }
}

impl std::error::Error for EdgeEnrollmentStoreError {}

// --- Rows ---------------------------------------------------------------------

/// One ISSUED device-identity term (the registry row). Construction goes
/// exclusively through [`EdgeDeviceIdentityRow::new`], which enforces the
/// closed status vocabulary and the non-empty identity/binding fields
/// BEFORE anything is written (the database CHECKs are defence in depth).
///
/// NOTE: there is deliberately NO key or certificate column — the record is
/// the identity bookkeeping, never secret material (§17.3; the decision
/// record §3 records certificate issuance as a deployment requirement).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeDeviceIdentityRow {
    /// The owning tenant (P0.16 rule 1 — present on the row so RLS and the
    /// explicit filter agree).
    pub tenant_id: TenantId,
    /// The device identity (non-empty text; unique per tenant together with
    /// the sequence).
    pub device_id: String,
    /// The approved site this identity is bound to (canonical UUID text;
    /// §16.2: one edge, one tenant, one approved site).
    pub site_id: String,
    /// The monotonic term sequence (>= 1). Rotation/expiry append NEW rows
    /// at the next sequence; the device's CURRENT state is the highest
    /// sequence.
    pub enrollment_seq: u64,
    /// The lifecycle status of THIS term (closed vocabulary).
    pub status: String,
    /// The approved site/asset scope annotation carried with the issuance
    /// (§16.2; free-form deployment annotation, no scope semantics here).
    pub approved_asset_scope: Option<String>,
    /// When this term was issued (unix millis).
    pub enrolled_at_unix_millis: i64,
    /// When this identity term expires (unix millis), when a real validity
    /// term exists (the shipped seam issues no certificate → `None`, and
    /// `None` is audited as `NoExpiryRecorded` edge-side — never "valid
    /// forever").
    pub expires_at_unix_millis: Option<i64>,
}

impl EdgeDeviceIdentityRow {
    /// Validates and builds the row (fail-closed; see
    /// [`EdgeEnrollmentStoreError::InvalidRecord`]).
    pub fn new(
        tenant_id: TenantId,
        device_id: String,
        site_id: String,
        enrollment_seq: u64,
        status: String,
        approved_asset_scope: Option<String>,
        enrolled_at_unix_millis: i64,
        expires_at_unix_millis: Option<i64>,
    ) -> Result<Self, EdgeEnrollmentStoreError> {
        if device_id.trim().is_empty() {
            return Err(EdgeEnrollmentStoreError::InvalidRecord { field: "device_id" });
        }
        if site_id.trim().is_empty() {
            return Err(EdgeEnrollmentStoreError::InvalidRecord { field: "site_id" });
        }
        if !EDGE_DEVICE_STATUSES.contains(&status.as_str()) {
            return Err(EdgeEnrollmentStoreError::InvalidRecord { field: "status" });
        }
        if enrollment_seq == 0 {
            return Err(EdgeEnrollmentStoreError::InvalidRecord {
                field: "enrollment_seq",
            });
        }
        if enrolled_at_unix_millis <= 0 {
            return Err(EdgeEnrollmentStoreError::InvalidRecord {
                field: "enrolled_at_unix_millis",
            });
        }
        Ok(Self {
            tenant_id,
            device_id,
            site_id,
            enrollment_seq,
            status,
            approved_asset_scope,
            enrolled_at_unix_millis,
            expires_at_unix_millis,
        })
    }
}

/// One append-only enrollment-journal event (the §17.3 identity audit
/// trail). Construction goes exclusively through
/// [`EdgeEnrollmentEventRow::new`] (closed event vocabulary, non-empty
/// actor/correlation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeEnrollmentEventRow {
    /// The owning tenant.
    pub tenant_id: TenantId,
    /// The journal event id (canonical UUID text).
    pub event_id: String,
    /// The device the event concerns (non-empty text; a REJECTED event may
    /// name a device that was never enrolled, so the journal carries no
    /// registry foreign key — migration 0018 documents that choice).
    pub device_id: String,
    /// The event type (closed vocabulary).
    pub event_type: String,
    /// The acting principal (`edge:enrollment:<device>` shape or an
    /// operator id).
    pub actor: String,
    /// The reason (diagnostics for REJECTED/REVOKED; no secret ever).
    pub reason: Option<String>,
    /// The correlation (trace) id (§21.3).
    pub correlation_id: String,
    /// When the event occurred (unix millis).
    pub occurred_at_unix_millis: i64,
}

impl EdgeEnrollmentEventRow {
    /// Validates and builds the event row (fail-closed).
    pub fn new(
        tenant_id: TenantId,
        event_id: String,
        device_id: String,
        event_type: String,
        actor: String,
        reason: Option<String>,
        correlation_id: String,
        occurred_at_unix_millis: i64,
    ) -> Result<Self, EdgeEnrollmentStoreError> {
        if event_id.trim().is_empty() {
            return Err(EdgeEnrollmentStoreError::InvalidRecord { field: "event_id" });
        }
        if device_id.trim().is_empty() {
            return Err(EdgeEnrollmentStoreError::InvalidRecord { field: "device_id" });
        }
        if !EDGE_ENROLLMENT_EVENT_TYPES.contains(&event_type.as_str()) {
            return Err(EdgeEnrollmentStoreError::InvalidRecord { field: "event_type" });
        }
        if actor.trim().is_empty() {
            return Err(EdgeEnrollmentStoreError::InvalidRecord { field: "actor" });
        }
        if correlation_id.trim().is_empty() {
            return Err(EdgeEnrollmentStoreError::InvalidRecord {
                field: "correlation_id",
            });
        }
        if occurred_at_unix_millis <= 0 {
            return Err(EdgeEnrollmentStoreError::InvalidRecord {
                field: "occurred_at_unix_millis",
            });
        }
        Ok(Self {
            tenant_id,
            event_id,
            device_id,
            event_type,
            actor,
            reason,
            correlation_id,
            occurred_at_unix_millis,
        })
    }
}

// --- The store port ------------------------------------------------------------

/// The edge-enrollment persistence port. Implemented by the PostgreSQL
/// adapter (`crates/adapters/postgres`) on `FlexCoreStore`: each method
/// opens ITS OWN unit of work with the caller's tenant context (the
/// enrollment route composes this port directly — it is a control-plane
/// provisioning surface, not a per-request registry read inside the
/// command/telemetry unit-of-work bundle).
///
/// Every method is tenant-scoped TWICE: the transaction-scoped
/// `flexcore.tenant_id` GUC feeds the RLS policy (ENABLE + FORCE), and the
/// statements filter by the context's tenant explicitly (P0.16 rule 5; the
/// 0017 command-store discipline).
pub trait EdgeEnrollmentStore: Send + Sync {
    /// Appends one ISSUED identity term. A duplicate (device_id, tenant_id)
    /// ACTIVE pair is the named [`EdgeEnrollmentStoreError::Conflict`] —
    /// no two edges share an identity.
    fn enroll_device(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        row: &EdgeDeviceIdentityRow,
    ) -> Result<(), EdgeEnrollmentStoreError>;

    /// The device's CURRENT identity term (highest `enrollment_seq`) in the
    /// caller's tenant scope; a foreign-tenant device reads as `None` —
    /// existence never leaks (P0.16 rule 5).
    fn find_device(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        device_id: &str,
    ) -> Result<Option<EdgeDeviceIdentityRow>, EdgeEnrollmentStoreError>;

    /// Whether `site_id` is an approved site in the caller's tenant scope
    /// (the §16.2 binding check the enrollment route performs BEFORE any
    /// registry write).
    fn site_is_approved(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        site_id: &str,
    ) -> Result<bool, EdgeEnrollmentStoreError>;

    /// Appends one journal event (append-only; the journal is the §17.3
    /// identity audit trail).
    fn append_event(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        event: &EdgeEnrollmentEventRow,
    ) -> Result<(), EdgeEnrollmentStoreError>;

    /// Lists the journal events for one device, oldest first, bounded by
    /// `limit` (ALWAYS applied — no unbounded query).
    fn list_events(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        device_id: &str,
        limit: u32,
    ) -> Result<Vec<EdgeEnrollmentEventRow>, EdgeEnrollmentStoreError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_domain::ids::TenantId;
    use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};

    fn ctx() -> TenantContext {
        TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
            VerifiedClaims {
                tenant_id: TenantId::new(FlexUuid::from_bytes(1u128.to_be_bytes())),
                subject: "p6.06-test".to_string(),
            },
        ))
    }

    #[test]
    fn row_construction_enforces_the_closed_vocabularies_and_shapes() {
        let tenant = TenantId::new(FlexUuid::from_bytes(1u128.to_be_bytes()));
        // Closed status vocabulary.
        assert!(EdgeDeviceIdentityRow::new(
            tenant,
            "edge-01".to_string(),
            "0192a5e0-0000-7000-8000-0000000000a2".to_string(),
            1,
            "SUSPENDED".to_string(),
            None,
            1_700_000_000_000,
            None,
        )
        .is_err());
        // Empty identity refused.
        assert!(EdgeDeviceIdentityRow::new(
            tenant,
            "  ".to_string(),
            "0192a5e0-0000-7000-8000-0000000000a2".to_string(),
            1,
            "ACTIVE".to_string(),
            None,
            1_700_000_000_000,
            None,
        )
        .is_err());
        // Zero sequence refused.
        assert!(EdgeDeviceIdentityRow::new(
            tenant,
            "edge-01".to_string(),
            "0192a5e0-0000-7000-8000-0000000000a2".to_string(),
            0,
            "ACTIVE".to_string(),
            None,
            1_700_000_000_000,
            None,
        )
        .is_err());
        // Valid row builds.
        assert!(EdgeDeviceIdentityRow::new(
            tenant,
            "edge-01".to_string(),
            "0192a5e0-0000-7000-8000-0000000000a2".to_string(),
            1,
            "ACTIVE".to_string(),
            None,
            1_700_000_000_000,
            Some(1_700_006_048_000),
        )
        .is_ok());
        // Journal event: closed vocabulary + non-empty actor/correlation.
        assert!(EdgeEnrollmentEventRow::new(
            tenant,
            "0192a5e0-0000-7000-8000-0000000000e1".to_string(),
            "edge-01".to_string(),
            "REJECTED".to_string(),
            "edge:enrollment:edge-01".to_string(),
            Some("duplicate identity".to_string()),
            "corr-1".to_string(),
            1_700_000_000_000,
        )
        .is_ok());
        assert!(EdgeEnrollmentEventRow::new(
            tenant,
            "0192a5e0-0000-7000-8000-0000000000e2".to_string(),
            "edge-01".to_string(),
            "MINTED".to_string(),
            "edge:enrollment:edge-01".to_string(),
            None,
            "corr-2".to_string(),
            1_700_000_000_000,
        )
        .is_err());
        let _ = ctx();
    }
}
