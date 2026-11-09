// P3.28 — EnvelopeRecordStore: the persistence contract for the portfolio
//         flexibility-envelope records (report §11.6, §16.1, §16.2; ledger
//         P3.28 step 3).
//
// Phase 3 produced the envelope COMPUTATION (P3.12) and its event assembly,
// but no persistence surface: this module closes that gap, following the
// recorded `schedule_store` home decision (D-P3.21-4) — a record-store
// trait here in flexcore-storage, with the same relocation path to
// flexcore-ports if a later subtask re-homes it. The envelope record is an
// immutable, append-oriented computation artifact (insert + tenant-scoped
// read/list; no update, no delete): a recomputation publishes a NEW
// envelope version, never an overwrite (§14.2 supersede discipline by
// analogy; the version uniqueness rule below makes a silent overwrite
// unrepresentable).
//
// ## Adapter obligations (binding — identical to `schedule_store`)
//
// 1. `insert_envelope_record` executes inside the CALLER's unit of work
//    ([`crate::unit_of_work::UnitOfWork`]) — the same native transaction
//    that carries the paired `FlexibilityEnvelopeUpdated` outbox rows
//    (report §15.5; one per covered site, assembled by the P3.12
//    `envelope_updated_publications`). An adapter that opens its own
//    transaction creates the dual-write hole §15.5 forbids.
// 2. Every query is tenant-scoped: RLS via the transaction-scoped
//    `flexcore.tenant_id` setting (P1.12 §5.1) PLUS an explicit tenant
//    filter; a record of another tenant never reads (P0.16 rule 5 —
//    existence never leaks across a tenant boundary).
// 3. Storage failures surface as [`EnvelopeStoreError::Backend`] with an
//    opaque code + message only (never raw SQL or driver text).
// 4. The records are immutable: adapters grant INSERT and SELECT only.
//
// ## Row shape
//
// Structured columns carry the identity and horizon fields a reader
// filters on (portfolio, envelope version, horizon, interval geometry,
// covered sites); the FULL envelope computation result (the P3.12
// `PortfolioFlexibilityEnvelope` as rendered by the composition root)
// travels as [`EnvelopeRecordRow::envelope_payload_json`] — opaque JSON
// text at this layer (this crate imports no serialization library; the
// adapter validates it parses as JSON on insert).
//
// ## Listing / keyset pagination (P3.28, recorded)
//
// `list_envelope_records` is the P3.28 per-tenant listing read: keyset over
// `envelope_id` (a caller-supplied UUIDv7, whose embedded timestamp makes
// identifier ordering a stable time order — the same recorded rationale as
// the P2.12 keyset choice). No offset pagination anywhere.

use flexcore_domain::ids::TenantId;

/// Errors of the envelope-record store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvelopeStoreError {
    /// The row is incomplete or malformed (`field` names the problem). The
    /// record was NOT written.
    InvalidRecord {
        field: &'static str,
    },
    /// An envelope with the same (portfolio, version) identity already
    /// exists for the tenant. The record was NOT written — envelope
    /// versions are immutable and a recomputation is a NEW version.
    Conflict {
        portfolio_id: String,
        envelope_version: String,
    },
    /// The storage backend failed. `code` and `message` are opaque and
    /// human-safe — never raw storage text.
    Backend {
        code: String,
        message: String,
    },
}

impl std::fmt::Display for EnvelopeStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnvelopeStoreError::InvalidRecord { field } => {
                write!(f, "the flexibility-envelope record is invalid: '{field}' is missing or malformed")
            }
            EnvelopeStoreError::Conflict {
                portfolio_id,
                envelope_version,
            } => write!(
                f,
                "a flexibility-envelope record for portfolio {portfolio_id} version \
                 {envelope_version} already exists; envelope versions are immutable \
                 and a recomputation is a new version"
            ),
            EnvelopeStoreError::Backend { code, message } => {
                write!(f, "storage backend failure ({code}): {message}")
            }
        }
    }
}

impl std::error::Error for EnvelopeStoreError {}

/// One persisted portfolio flexibility-envelope record (the insert/read
/// row; ledger P3.28 step 3). Construction goes exclusively through
/// [`EnvelopeRecordRow::new`], which validates the storage-level
/// completeness rules. Identifier fields are the canonical hyphenated UUID
/// text forms (the recorded FlexUuid ↔ PG uuid mapping binds text and
/// casts `::uuid` in SQL).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvelopeRecordRow {
    /// The owning tenant (P0.16 rule 1 — `tenant_id` on every tenant-owned
    /// record, present on the row itself so RLS and the explicit filter
    /// agree).
    pub tenant_id: TenantId,
    /// The owning portfolio (canonical UUID text).
    pub portfolio_id: String,
    /// The record identity (canonical UUID text; caller-supplied UUIDv7 so
    /// identifier order is a stable time order for the keyset listing).
    pub envelope_id: String,
    /// The envelope version (P3.12 `EnvelopeVersion`, validated there).
    pub envelope_version: String,
    /// The start-inclusive horizon start, UTC millis.
    pub horizon_start_millis: i64,
    /// The end-exclusive horizon end, UTC millis (strictly after the
    /// start; §10.3 interval semantics).
    pub horizon_end_millis: i64,
    /// The interval length, UTC millis (strictly positive).
    pub interval_millis: i64,
    /// The number of intervals in the envelope (at least 1).
    pub interval_count: u32,
    /// The covered sites (canonical UUID text; one
    /// `FlexibilityEnvelopeUpdated` event per site, in list order).
    pub covered_site_ids: Vec<String>,
    /// The full envelope computation result as JSON text — opaque to this
    /// crate (no serialization dependency here); the adapter validates it
    /// parses as JSON on insert.
    pub envelope_payload_json: String,
}

impl EnvelopeRecordRow {
    /// Builds and validates one row. Storage-level rules: every identifier
    /// non-empty; horizon end strictly after horizon start; positive
    /// interval length; at least one interval; at least one covered site
    /// (an envelope covers at least the portfolio's own scope — the
    /// P3.12 event assembly refuses an empty site list too); non-empty
    /// payload.
    pub fn new(
        tenant_id: TenantId,
        portfolio_id: String,
        envelope_id: String,
        envelope_version: String,
        horizon_start_millis: i64,
        horizon_end_millis: i64,
        interval_millis: i64,
        interval_count: u32,
        covered_site_ids: Vec<String>,
        envelope_payload_json: String,
    ) -> Result<Self, EnvelopeStoreError> {
        for (field, value) in [
            ("portfolio_id", portfolio_id.as_str()),
            ("envelope_id", envelope_id.as_str()),
            ("envelope_version", envelope_version.as_str()),
            ("envelope_payload_json", envelope_payload_json.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(EnvelopeStoreError::InvalidRecord { field });
            }
        }
        if horizon_end_millis <= horizon_start_millis {
            return Err(EnvelopeStoreError::InvalidRecord {
                field: "horizon_end_millis",
            });
        }
        if interval_millis <= 0 {
            return Err(EnvelopeStoreError::InvalidRecord {
                field: "interval_millis",
            });
        }
        if interval_count == 0 {
            return Err(EnvelopeStoreError::InvalidRecord {
                field: "interval_count",
            });
        }
        if covered_site_ids.is_empty() {
            return Err(EnvelopeStoreError::InvalidRecord {
                field: "covered_site_ids",
            });
        }
        for (index, site) in covered_site_ids.iter().enumerate() {
            if site.trim().is_empty() {
                return Err(EnvelopeStoreError::InvalidRecord {
                    field: "covered_site_ids",
                });
            }
            let _ = index;
        }
        Ok(Self {
            tenant_id,
            portfolio_id,
            envelope_id,
            envelope_version,
            horizon_start_millis,
            horizon_end_millis,
            interval_millis,
            interval_count,
            covered_site_ids,
            envelope_payload_json,
        })
    }
}

/// The receipt of one persisted envelope record (the receipt convention of
/// this crate: its existence proves the insert was ISSUED inside the
/// caller's unit of work).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvelopeRecordReceipt {
    /// The persisted record's identity (canonical UUID text).
    pub envelope_id: String,
    /// The owning tenant.
    pub tenant_id: TenantId,
}

impl EnvelopeRecordReceipt {
    /// Builds the receipt (adapter-side; both fields are taken as given
    /// from the executed insert).
    pub fn new(envelope_id: String, tenant_id: TenantId) -> Self {
        Self {
            envelope_id,
            tenant_id,
        }
    }
}

/// The per-tenant listing filter (P3.28): the optional portfolio filter and
/// the keyset cursor position (the `envelope_id` of the last record of the
/// previous page). Records are returned in ascending `envelope_id` order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EnvelopeListFilter {
    /// Restrict to one portfolio (canonical UUID text; `None` = all of the
    /// caller's tenant).
    pub portfolio_id: Option<String>,
    /// The keyset cursor: return records with `envelope_id` strictly after
    /// this one (`None` = first page).
    pub after_envelope_id: Option<String>,
}

/// The envelope-record persistence contract (§16.1: PostgreSQL is the
/// primary transactional source of truth). One trait, three operations —
/// an append-oriented record store, not a CRUD repository: records are
/// immutable (see the adapter obligations in the module header).
///
/// ## Home decision (recorded deviation from the P1.10 single-home rule)
///
/// Same recorded deviation as [`crate::schedule_store::ScheduleRecordStore`]
/// (D-P3.21-4): the trait lives in flexcore-storage; it is shaped to move
/// to flexcore-ports verbatim if a later subtask relocates it.
pub trait EnvelopeRecordStore {
    /// Inserts one envelope record inside the CALLER's unit of work
    /// (adapter obligation 1: the same native transaction carries the
    /// paired `FlexibilityEnvelopeUpdated` outbox rows). A
    /// (tenant, portfolio, version) collision is
    /// [`EnvelopeStoreError::Conflict`]; storage failures are
    /// [`EnvelopeStoreError::Backend`] with an opaque code + message.
    fn insert_envelope_record(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        row: &EnvelopeRecordRow,
    ) -> Result<EnvelopeRecordReceipt, EnvelopeStoreError>;

    /// Looks a record up by envelope identity within the caller's tenant
    /// scope. A record of another tenant reads as `None` (no existence
    /// leak, P0.16 rule 5).
    fn find_envelope_record(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        envelope_id: &str,
    ) -> Result<Option<EnvelopeRecordRow>, EnvelopeStoreError>;

    /// Lists records tenant-scoped (P3.28 per-tenant listing): keyset over
    /// `envelope_id` ascending, bounded by `limit` (the route layer owns
    /// the page-size limits), filtered by [`EnvelopeListFilter`]. Records
    /// of other tenants never appear (RLS + explicit filter).
    fn list_envelope_records(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        filter: &EnvelopeListFilter,
        limit: usize,
    ) -> Result<Vec<EnvelopeRecordRow>, EnvelopeStoreError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;

    fn tenant() -> TenantId {
        let mut bytes = [0u8; 16];
        bytes[15] = 1;
        TenantId::new(FlexUuid::from_bytes(bytes))
    }

    fn row_fields() -> (TenantId, String, String, String, i64, i64, i64, u32, Vec<String>, String)
    {
        (
            tenant(),
            "0192a5e0-0000-7000-8000-000000000001".to_string(),
            "0192a5e0-0000-7000-8000-0000000000e0".to_string(),
            "env-v1".to_string(),
            1_700_000_000_000,
            1_700_008_640_000,
            900_000,
            96,
            vec!["0192a5e0-0000-7000-8000-000000000002".to_string()],
            "{\"intervals\":[]}".to_string(),
        )
    }

    #[test]
    fn a_complete_row_builds_and_carries_every_field() {
        let p = row_fields();
        let row = EnvelopeRecordRow::new(
            p.0, p.1.clone(), p.2.clone(), p.3.clone(), p.4, p.5, p.6, p.7, p.8.clone(), p.9,
        )
        .expect("complete row builds");
        assert_eq!(row.envelope_version, "env-v1");
        assert_eq!(row.interval_count, 96);
        assert_eq!(row.covered_site_ids.len(), 1);
    }

    #[test]
    fn an_inverted_horizon_is_rejected() {
        let mut p = row_fields();
        p.5 = p.4; // horizon_end <= horizon_start
        assert_eq!(
            EnvelopeRecordRow::new(p.0, p.1, p.2, p.3, p.4, p.5, p.6, p.7, p.8, p.9),
            Err(EnvelopeStoreError::InvalidRecord {
                field: "horizon_end_millis"
            })
        );
    }

    #[test]
    fn empty_required_fields_are_rejected_with_the_field_named() {
        for blanked in [
            "portfolio_id",
            "envelope_id",
            "envelope_version",
            "envelope_payload_json",
        ] {
            let mut p = row_fields();
            match blanked {
                "portfolio_id" => p.1 = String::new(),
                "envelope_id" => p.2 = String::new(),
                "envelope_version" => p.3 = "  ".to_string(),
                "envelope_payload_json" => p.9 = String::new(),
                _ => unreachable!(),
            }
            assert_eq!(
                EnvelopeRecordRow::new(p.0, p.1, p.2, p.3, p.4, p.5, p.6, p.7, p.8, p.9),
                Err(EnvelopeStoreError::InvalidRecord { field: blanked }),
                "blanking {blanked} must name the field"
            );
        }
    }

    #[test]
    fn an_envelope_without_covered_sites_is_rejected() {
        let mut p = row_fields();
        p.8 = Vec::new();
        assert_eq!(
            EnvelopeRecordRow::new(p.0, p.1, p.2, p.3, p.4, p.5, p.6, p.7, p.8, p.9),
            Err(EnvelopeStoreError::InvalidRecord {
                field: "covered_site_ids"
            })
        );
    }

    #[test]
    fn zero_intervals_and_non_positive_interval_length_are_rejected() {
        let mut p = row_fields();
        p.7 = 0;
        assert_eq!(
            EnvelopeRecordRow::new(p.0, p.1, p.2, p.3, p.4, p.5, p.6, p.7, p.8, p.9),
            Err(EnvelopeStoreError::InvalidRecord {
                field: "interval_count"
            })
        );
        let mut p = row_fields();
        p.6 = 0;
        assert_eq!(
            EnvelopeRecordRow::new(p.0, p.1, p.2, p.3, p.4, p.5, p.6, p.7, p.8, p.9),
            Err(EnvelopeStoreError::InvalidRecord {
                field: "interval_millis"
            })
        );
    }

    #[test]
    fn the_receipt_carries_the_record_identity() {
        let receipt = EnvelopeRecordReceipt::new("env-1".to_string(), tenant());
        assert_eq!(receipt.envelope_id, "env-1");
        assert_eq!(receipt.tenant_id, tenant());
    }
}
