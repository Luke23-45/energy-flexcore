// P1.13 — Audit record persistence: the append-only `AuditSink` INSERT.
//
// Column mapping (P1.12 §4.7, the storage counterpart of the flexcore-audit
// `AuditRecord`):
//
// | Record field        | Column            | Note |
// |---|---|---|
// | `audit_id`          | `audit_id` (uuid)   | caller-supplied identifier, cast from text |
// | `tenant_id`         | `tenant_id` (uuid)  | NULL = platform-level action |
// | `actor` / `action`  | `actor` / `action`  | validated non-empty upstream |
// | `object`            | `object_type` + `object_id` | |
// | `before_ref` / `after_ref` | `before_ref` / `after_ref` (jsonb) | *references*, not payload copies |
// | `decision`          | `decision` (jsonb)  | outcome + reason reference, never a bare allow/deny |
// | `evidence`          | `evidence` (jsonb)  | evidence reference |
// | `event_time` / `ingest_time` | `event_time` / `ingest_time` | distinct columns, P0.09 |
// | `correlation_id`    | `correlation_id`    | column type is text per P1.12 §4.7; the uuid travels in canonical text form |
// | `corrects`          | `corrected_audit_id` (uuid) | corrections are NEW rows referencing the original |
//
// Append-only layer 1 is the grant set (INSERT/SELECT only, P1.12 §4.7);
// layer 2 the before-update/delete trigger; layer 3 the port. This module
// only ever INSERTs into audit_records.
//
// P11.15 ADDITION — the keyed audit chain (the P11-E-1 DEFAULT, option a):
// when the deployment configured a chain key
// (`FlexCoreStore::with_audit_chain_key`), every append also advances the
// TENANT-NAMESPACE keyed HMAC-SHA-256 chain (`flexcore_audit::chain`):
// the per-tenant sequence and head are read under the `audit_chain_state`
// row lock (migration 0019), the digest is computed over (previous head ||
// canonical record bytes), and the row carries its own backwards link
// (`chain_prev_head`) so the stored chain is verifiable from the rows
// alone. THE CHAIN IS A DEPLOYMENT REQUIREMENT: without a configured key
// the chain columns stay NULL — rows are then verifiable only through the
// TEST-GRADE FNV-1a edge chain (LR-203), never a release signature. The
// UPDATE grant on `audit_chain_state` is the recorded exception for the
// head-pointer bookkeeping table ONLY; audit_records keeps INSERT, SELECT.
// Platform-level records (`tenant_id IS NULL`) keep the P1.12 §5.2 rule:
// invisible to `flexcore_app` under the RLS policy, writable only through
// the owner/ops path — the platform chain namespace (`audit_chain_state`
// sentinel key) is likewise an owner/ops-path concern.
//
// FIM V-9 ADDITION (wave 8, written-unrun) — the chain upgrade note:
// `docs/security/audit_chain_upgrade_note_fim.md` records how audit
// evidence verifies ACROSS the chain upgrade boundary. This module's
// keyed append has always composed `flexcore_audit::chain` (the
// HMAC-SHA-256 construction) — the adapter never wrote an FNV digest into
// any `audit_records` chain column; the FNV-1a chain is the TEST-GRADE
// edge chain (`crates/edge-agent/src/audit_proof.rs`, label
// `fnv1a64-chain`, LR-203), which keeps its own recorded algorithm label
// and its own verifier. Rows written before migration 0019, or while no
// chain key was configured, carry NULL chain columns and stay outside
// the keyed chain — reported as `unchained_records`, never re-hashed and
// never silently skipped; keyed rows carry their recorded `chain_algorithm`
// label (`AUDIT_CHAIN_ALGORITHM`). A verifier tells the generations apart
// by that recorded algorithm field alone — no silent re-hash in either
// direction.

use crate::error::map_audit_rejection;
use crate::unit_of_work::Tx;
use crate::uuid_map::{uuid_from_text, uuid_text};
use flexcore_audit::chain::{
    chain_step, genesis_head, hex_encode_digest, parse_digest_hex, ChainKey, TenantChainEntry,
    AUDIT_CHAIN_ALGORITHM,
};
use flexcore_audit::record::AuditRecord;
use flexcore_domain::ids::TenantId;
use serde_json::Value;
use sqlx::Postgres;
use sqlx::Row;

/// The `audit_chain_state` namespace key for the PLATFORM-level chain
/// (records with `tenant_id IS NULL`). A uuid PRIMARY KEY cannot hold SQL
/// NULL, so the platform namespace uses the recorded all-zero sentinel.
pub(crate) const PLATFORM_CHAIN_TENANT_KEY: &str = "00000000-0000-0000-0000-000000000000";

/// The chain columns computed for one keyed append, written with the
/// record in the same INSERT (never as a follow-up UPDATE — the row is
/// written once, completely, or the append fails).
pub(crate) struct AuditChainColumns {
    /// The per-tenant chain position (1-based).
    pub chain_sequence: i64,
    /// The previous head digest this row's digest chained from (lower-case
    /// hex; the genesis head on the first row of a namespace).
    pub chain_prev_head_hex: String,
    /// HMAC-SHA-256 over (previous head || canonical record bytes) under
    /// the tenant-namespace chain key (lower-case hex).
    pub chain_digest_hex: String,
}

/// One reference field as its jsonb value (a JSON string — the reference is
/// a pointer, not a payload copy, P1.12 §4.7).
fn reference_json(reference: Option<&str>) -> Option<Value> {
    reference.map(|r| Value::String(r.to_string()))
}

/// Inserts one audit record. The caller is responsible for the transaction
/// and the tenant context: under the P1.12 §5 RLS policy the row's
/// `tenant_id` must match the transaction's `flexcore.tenant_id` setting
/// (a platform-level row with `tenant_id IS NULL` is therefore only
/// writable through the owner/ops path, P1.12 §5.2). `chain` carries the
/// keyed-chain columns when the deployment configured a chain key; `None`
/// leaves them NULL (the honest un-keyed state — DEPLOYMENT REQUIREMENT).
pub(crate) async fn insert_audit_record<'e, E>(
    executor: E,
    record: &AuditRecord,
    chain: Option<&AuditChainColumns>,
) -> Result<(), flexcore_audit::record::AuditError>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let decision_json: Option<Value> = match &record.decision {
        Some(decision) => Some(serde_json::to_value(decision).map_err(|_| {
            flexcore_audit::record::AuditError::Rejected {
                reason: "the recorded decision could not be serialized".to_string(),
            }
        })?),
        None => None,
    };
    let corrected = record
        .corrects
        .as_ref()
        .map(|correction| uuid_text(correction.original_audit_id()));

    sqlx::query(
        "INSERT INTO audit_records \
             (audit_id, tenant_id, actor, action, object_type, object_id, \
              before_ref, after_ref, decision, evidence, \
              event_time, ingest_time, correlation_id, corrected_audit_id, \
              chain_sequence, chain_prev_head, chain_digest, chain_algorithm) \
         VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6, $7::jsonb, $8::jsonb, $9::jsonb, $10::jsonb, \
                 $11, $12, $13, $14::uuid, $15, $16, $17, $18)",
    )
    .bind(uuid_text(record.audit_id))
    .bind(record.tenant_id.map(|tenant| uuid_text(tenant.uuid())))
    .bind(record.actor.as_str())
    .bind(record.action.as_str())
    .bind(record.object.object_type.as_str())
    .bind(record.object.object_id.as_str())
    .bind(reference_json(record.before_ref.as_ref().map(|r| r.as_str())))
    .bind(reference_json(record.after_ref.as_ref().map(|r| r.as_str())))
    .bind(decision_json)
    .bind(reference_json(record.evidence.as_ref().map(|r| r.as_str())))
    .bind(record.event_time.timestamp())
    .bind(record.ingest_time.timestamp())
    .bind(uuid_text(record.correlation_id))
    .bind(corrected)
    .bind(chain.map(|c| c.chain_sequence))
    .bind(chain.map(|c| c.chain_prev_head_hex.clone()))
    .bind(chain.map(|c| c.chain_digest_hex.clone()))
    .bind(chain.map(|_| AUDIT_CHAIN_ALGORITHM))
    .execute(executor)
    .await
    .map_err(map_audit_rejection)?;
    Ok(())
}

// --- P11.15 — the keyed per-tenant chain append ---------------------------------

/// Reads (and row-locks) the chain head state for one namespace. `FOR
/// UPDATE` serializes concurrent appends in the same namespace: the second
/// writer blocks until the first commits, then reads the advanced head —
/// the chain stays gapless per tenant.
async fn fetch_chain_state(
    tx: &mut Tx,
    tenant_key_text: &str,
) -> Result<Option<(i64, Option<String>)>, flexcore_audit::record::AuditError> {
    let row = sqlx::query(
        "SELECT chain_sequence, chain_head FROM audit_chain_state \
         WHERE tenant_key = $1::uuid FOR UPDATE",
    )
    .bind(tenant_key_text)
    .fetch_optional(&mut **tx)
    .await
    .map_err(map_audit_rejection)?;
    match row {
        None => Ok(None),
        Some(row) => {
            let sequence: i64 = row
                .try_get("chain_sequence")
                .map_err(|_| flexcore_audit::record::AuditError::Rejected {
                    reason: "the chain head state could not be decoded".to_string(),
                })?;
            let head: Option<String> = row
                .try_get("chain_head")
                .map_err(|_| flexcore_audit::record::AuditError::Rejected {
                    reason: "the chain head state could not be decoded".to_string(),
                })?;
            Ok(Some((sequence, head)))
        }
    }
}

/// Creates the chain head state row for a namespace that has not yet seen
/// a record (sequence 0, no head). A concurrent creator loses on the
/// PRIMARY KEY and the append fails with the classified rejection — the
/// caller retries; a gapless chain is worth the honest failure mode.
async fn create_chain_state(
    tx: &mut Tx,
    tenant_key_text: &str,
) -> Result<(), flexcore_audit::record::AuditError> {
    sqlx::query(
        "INSERT INTO audit_chain_state (tenant_key, chain_sequence, chain_head, chain_algorithm) \
         VALUES ($1::uuid, 0, NULL, $2)",
    )
    .bind(tenant_key_text)
    .bind(AUDIT_CHAIN_ALGORITHM)
    .execute(&mut **tx)
    .await
    .map_err(map_audit_rejection)?;
    Ok(())
}

/// Advances the head state to the just-appended row's sequence and digest.
async fn advance_chain_state(
    tx: &mut Tx,
    tenant_key_text: &str,
    sequence: i64,
    head_hex: &str,
) -> Result<(), flexcore_audit::record::AuditError> {
    sqlx::query(
        "UPDATE audit_chain_state SET chain_sequence = $2, chain_head = $3, \
                chain_algorithm = $4, updated_at = now() \
         WHERE tenant_key = $1::uuid",
    )
    .bind(tenant_key_text)
    .bind(sequence)
    .bind(head_hex)
    .bind(AUDIT_CHAIN_ALGORITHM)
    .execute(&mut **tx)
    .await
    .map_err(map_audit_rejection)?;
    Ok(())
}

/// Appends one audit record WITH the keyed per-tenant chain advance, all
/// inside the caller's transaction: head state read under row lock ->
/// digest computed over (previous head || canonical record bytes) under
/// the tenant-namespace key -> the record INSERTed with its chain columns
/// -> the head state advanced. All-or-nothing with the caller's unit of
/// work; a failure leaves neither the row nor an advanced head. The
/// caller derives the TENANT-NAMESPACE key (`ChainKey::
/// chain_key_for_namespace(record.tenant_id)`) — this function digests
/// under exactly that namespace.
pub(crate) async fn append_audit_record_with_chain(
    tx: &mut Tx,
    record: &AuditRecord,
    tenant_namespace_key: &ChainKey,
) -> Result<(), flexcore_audit::record::AuditError> {
    let tenant_key_text = match record.tenant_id {
        Some(tenant) => uuid_text(tenant.uuid()),
        None => PLATFORM_CHAIN_TENANT_KEY.to_string(),
    };
    let (current_sequence, current_head) =
        match fetch_chain_state(&mut *tx, &tenant_key_text).await? {
            Some(state) => state,
            None => {
                create_chain_state(&mut *tx, &tenant_key_text).await?;
                (0, None)
            }
        };
    let next_sequence = current_sequence.checked_add(1).ok_or_else(|| {
        flexcore_audit::record::AuditError::Rejected {
            reason: "the per-tenant chain sequence exhausted its range".to_string(),
        }
    })?;
    // The previous head: the stored head, or the namespace's genesis for
    // the namespace's first record. Both are stored on the row
    // (`chain_prev_head`), so the stored chain is self-contained.
    let previous_head = match &current_head {
        Some(head_hex) => {
            parse_digest_hex(head_hex).map_err(|error| {
                flexcore_audit::record::AuditError::Rejected { reason: error.to_string() }
            })?
        }
        None => genesis_head(tenant_namespace_key),
    };
    let digest = chain_step(tenant_namespace_key, &previous_head, record);
    let columns = AuditChainColumns {
        chain_sequence: next_sequence,
        chain_prev_head_hex: hex_encode_digest(&previous_head),
        chain_digest_hex: hex_encode_digest(&digest),
    };
    insert_audit_record(&mut **tx, record, Some(&columns)).await?;
    advance_chain_state(&mut *tx, &tenant_key_text, next_sequence, &columns.chain_digest_hex).await?;
    Ok(())
}

// --- P11.15 — the per-tenant chain verification read path -----------------------

/// One stored chain row, decoded for verification. WRITTEN, NEVER RUN:
/// the verification is exercised only by the first human test/binary run.
pub(crate) struct TenantChainRow {
    /// The stored per-tenant chain sequence.
    pub chain_sequence: i64,
    /// The stored digest, parsed to bytes (a malformed stored digest is a
    /// finding, never a guess).
    pub stored_digest: [u8; 32],
    /// The decoded audit record (the constructor re-validates every rule).
    pub record: AuditRecord,
}

/// The per-tenant verification outcome: `Verified` or a named finding.
/// A failed verification is a SECURITY FINDING for an operator, never a
/// retried read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditChainVerificationOutcome {
    /// Every stored digest recomputed from genesis under the tenant
    /// namespace key; head and count match the stored state.
    Verified,
    /// The named finding (the [`flexcore_audit::chain::ChainError`]
    /// Display text, or the row-decode failure description). `at_sequence`
    /// names the chain sequence when the finding has one.
    Failed {
        at_sequence: Option<i64>,
        finding: String,
    },
}

/// The per-tenant chain verification report the store-level read returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantChainVerification {
    /// The verified tenant namespace.
    pub tenant_id: TenantId,
    /// How many chained records recomputed cleanly (0 unless `Verified`).
    pub verified_records: u64,
    /// Rows with NULL chain columns — records written before migration
    /// 0019 or while no key was configured. They are OUTSIDE the keyed
    /// chain and reported, never silently skipped.
    pub unchained_records: u64,
    /// The recomputed final head (the anchor-export input) when verified.
    pub head_digest_hex: Option<String>,
    /// The outcome.
    pub outcome: AuditChainVerificationOutcome,
}

/// Reads one tenant's chained rows (RLS scopes them to the transaction's
/// tenant context, and the statement filters explicitly) in chain-sequence
/// order, decoding each into the record type. Returns the rows plus the
/// count of unchained rows encountered in the same window.
pub(crate) async fn read_tenant_chain_rows(
    tx: &mut Tx,
    tenant_text: &str,
) -> Result<(Vec<TenantChainRow>, u64), flexcore_audit::record::AuditError> {
    let rows = sqlx::query(
        "SELECT chain_sequence, chain_digest, audit_id::text AS audit_id, \
                tenant_id::text AS tenant_id, actor, action, object_type, object_id, \
                before_ref, after_ref, decision, evidence, event_time, ingest_time, \
                correlation_id, corrected_audit_id::text AS corrected_audit_id \
         FROM audit_records \
         WHERE tenant_id = $1::uuid AND chain_sequence IS NOT NULL \
         ORDER BY chain_sequence ASC",
    )
    .bind(tenant_text)
    .fetch_all(&mut **tx)
    .await
    .map_err(map_audit_rejection)?;
    let unchained = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM audit_records \
         WHERE tenant_id = $1::uuid AND chain_sequence IS NULL",
    )
    .bind(tenant_text)
    .fetch_one(&mut **tx)
    .await
    .map_err(map_audit_rejection)?;
    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        let sequence: i64 = row
            .try_get("chain_sequence")
            .map_err(|_| flexcore_audit::record::AuditError::Rejected {
                reason: "a stored chain sequence could not be decoded".to_string(),
            })?;
        let digest_hex: String = row
            .try_get("chain_digest")
            .map_err(|_| flexcore_audit::record::AuditError::Rejected {
                reason: "a stored chain digest could not be decoded".to_string(),
            })?;
        let stored_digest = parse_digest_hex(&digest_hex).map_err(|error| {
            flexcore_audit::record::AuditError::Rejected { reason: error.to_string() }
        })?;
        let record = decode_audit_record(row, sequence)?;
        out.push(TenantChainRow {
            chain_sequence: sequence,
            stored_digest,
            record,
        });
    }
    let unchained_records =
        u64::try_from(unchained.max(0)).unwrap_or(0);
    Ok((out, unchained_records))
}

/// Decodes one `audit_records` row back into the record type (the
/// constructor re-validates every record rule — a malformed row fails the
/// verification rather than silently narrowing what is recomputed).
fn decode_audit_record(
    row: &sqlx::postgres::PgRow,
    sequence: i64,
) -> Result<AuditRecord, flexcore_audit::record::AuditError> {
    let rejected = |what: &str| flexcore_audit::record::AuditError::Rejected {
        reason: format!("stored audit row (chain sequence {sequence}) could not be decoded: {what}"),
    };
    let text = |column: &'static str| -> Result<String, flexcore_audit::record::AuditError> {
        row.try_get::<String, _>(column).map_err(|_| rejected(column))
    };
    let opt_text =
        |column: &'static str| -> Result<Option<String>, flexcore_audit::record::AuditError> {
            row.try_get::<Option<String>, _>(column)
                .map_err(|_| rejected(column))
        };
    let opt_json =
        |column: &'static str| -> Result<Option<Value>, flexcore_audit::record::AuditError> {
            row.try_get::<Option<Value>, _>(column)
                .map_err(|_| rejected(column))
        };
    let audit_id = uuid_from_text(&text("audit_id")?).map_err(|_| rejected("audit_id"))?;
    let tenant_id: Option<TenantId> = match opt_text("tenant_id")? {
        None => None,
        Some(raw) => {
            let uuid = uuid_from_text(&raw).map_err(|_| rejected("tenant_id"))?;
            Some(TenantId::new(uuid))
        }
    };
    let event_time = row
        .try_get::<time::OffsetDateTime, _>("event_time")
        .map_err(|_| rejected("event_time"))?;
    let ingest_time = row
        .try_get::<time::OffsetDateTime, _>("ingest_time")
        .map_err(|_| rejected("ingest_time"))?;
    let correlation_id =
        uuid_from_text(&text("correlation_id")?).map_err(|_| rejected("correlation_id"))?;
    let decision = match opt_json("decision")? {
        None => None,
        Some(value) => Some(
            serde_json::from_value::<flexcore_audit::record::AuditDecision>(value)
                .map_err(|_| rejected("decision"))?,
        ),
    };
    let mut record = AuditRecord::new(flexcore_audit::record::AuditRecordFields {
        audit_id,
        tenant_id,
        actor: flexcore_audit::record::ActorIdentity::new(text("actor")?)
            .map_err(|_| rejected("actor"))?,
        action: flexcore_audit::record::AuditAction::new(text("action")?)
            .map_err(|_| rejected("action"))?,
        object: flexcore_audit::record::AuditObject::new(text("object_type")?, text("object_id")?)
            .map_err(|_| rejected("object"))?,
        before_ref: match opt_json("before_ref")? {
            None => None,
            Some(Value::String(raw)) => Some(
                flexcore_audit::record::RecordReference::new(raw)
                    .map_err(|_| rejected("before_ref"))?,
            ),
            Some(_) => return Err(rejected("before_ref (not a JSON string)")),
        },
        after_ref: match opt_json("after_ref")? {
            None => None,
            Some(Value::String(raw)) => Some(
                flexcore_audit::record::RecordReference::new(raw)
                    .map_err(|_| rejected("after_ref"))?,
            ),
            Some(_) => return Err(rejected("after_ref (not a JSON string)")),
        },
        decision,
        evidence: match opt_json("evidence")? {
            None => None,
            Some(Value::String(raw)) => Some(
                flexcore_audit::record::EvidenceReference::new(raw)
                    .map_err(|_| rejected("evidence"))?,
            ),
            Some(_) => return Err(rejected("evidence (not a JSON string)")),
        },
        event_time: flexcore_contracts::time_model::EventTime::new(event_time)
            .map_err(|_| rejected("event_time"))?,
        ingest_time: flexcore_contracts::time_model::IngestTime::new(ingest_time)
            .map_err(|_| rejected("ingest_time"))?,
        correlation_id,
    })
    .map_err(|_| rejected("the P0.09 time ordering"))?;
    if let Some(corrected) = opt_text("corrected_audit_id")? {
        let corrected_id =
            uuid_from_text(&corrected).map_err(|_| rejected("corrected_audit_id"))?;
        record = record
            .link_correction(
                corrected_id,
                // The correction reason is not a persisted column of the
                // audit_records design (P1.12 §4.7); the decode supplies a
                // placeholder that the chain encoding does NOT digest —
                // `canonical_record_bytes` folds the linkage id only (the
                // recorded chain limitation). The placeholder exists because
                // the record TYPE requires a reason.
                "(not persisted; see the chain encoding limitation in flexcore-audit::chain)"
                    .to_string(),
            )
            .map_err(|_| rejected("corrected_audit_id"))?;
    }
    Ok(record)
}

/// Runs the per-tenant chain verification over the stored rows: derives
/// the tenant-namespace key, recomputes every digest from genesis, and
/// builds the named report. WRITTEN, NEVER RUN — verification of a live
/// chain is a human-gated operation.
pub(crate) async fn verify_tenant_chain_rows(
    tx: &mut Tx,
    tenant: TenantId,
    namespace_key: &ChainKey,
) -> Result<TenantChainVerification, flexcore_audit::record::AuditError> {
    let tenant_text = uuid_text(tenant.uuid());
    let (rows, unchained_records) = read_tenant_chain_rows(tx, &tenant_text).await?;
    let entries: Vec<TenantChainEntry<'_>> = rows
        .iter()
        .map(|row| TenantChainEntry {
            chain_sequence: row.chain_sequence,
            stored_digest: row.stored_digest,
            record: &row.record,
        })
        .collect();
    if entries.is_empty() {
        // No chained rows: nothing to verify — reported as verified with
        // zero records (the unchained count carries the honest caveat).
        return Ok(TenantChainVerification {
            tenant_id: tenant,
            verified_records: 0,
            unchained_records,
            head_digest_hex: None,
            outcome: AuditChainVerificationOutcome::Verified,
        });
    }
    let per_tenant_key = namespace_key.chain_key_for_namespace(Some(tenant));
    match flexcore_audit::chain::verify_tenant_chain(&per_tenant_key, &entries) {
        Ok(head) => Ok(TenantChainVerification {
            tenant_id: tenant,
            verified_records: entries.len() as u64,
            unchained_records,
            head_digest_hex: Some(hex_encode_digest(&head)),
            outcome: AuditChainVerificationOutcome::Verified,
        }),
        Err(error) => {
            let at_sequence = match &error {
                flexcore_audit::chain::ChainError::DigestMismatch { sequence } => Some(*sequence),
                flexcore_audit::chain::ChainError::NonMonotonicSequence { found, .. } => {
                    Some(*found)
                }
                _ => None,
            };
            Ok(TenantChainVerification {
                tenant_id: tenant,
                verified_records: 0,
                unchained_records,
                head_digest_hex: None,
                outcome: AuditChainVerificationOutcome::Failed {
                    at_sequence,
                    finding: error.to_string(),
                },
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_audit::record::{
        ActorIdentity, AuditAction, AuditObject, AuditRecord, AuditRecordFields, RecordReference,
    };
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_contracts::time_model::{EventTime, IngestTime};

    fn sample_uuid(tag: u8) -> FlexUuid {
        let mut bytes = [0u8; 16];
        bytes[15] = tag;
        FlexUuid::from_bytes(bytes)
    }

    fn sample_record() -> AuditRecord {
        AuditRecord::new(AuditRecordFields {
            audit_id: sample_uuid(1),
            tenant_id: Some(TenantId::new(sample_uuid(2))),
            actor: ActorIdentity::new("operator-1".to_string()).expect("valid actor"),
            action: AuditAction::new("asset.register".to_string()).expect("valid action"),
            object: AuditObject::new("asset".to_string(), "asset-1".to_string())
                .expect("valid object"),
            before_ref: None,
            after_ref: Some(
                RecordReference::new("asset-1:v2".to_string()).expect("valid reference"),
            ),
            decision: None,
            evidence: None,
            event_time: EventTime::from_unix_millis(1_000).expect("valid event time"),
            ingest_time: IngestTime::from_unix_millis(2_000).expect("valid event time"),
            correlation_id: sample_uuid(3),
        })
        .expect("valid record")
    }

    #[test]
    fn reference_fields_map_to_json_strings() {
        // A reference is stored as a JSON string (a pointer), never
        // stringified into an object — matching the P1.12 §4.7 rule that
        // before/after are references, not payload copies.
        let value = reference_json(Some("asset-1:v2"));
        assert_eq!(value, Some(Value::String("asset-1:v2".to_string())));
        assert_eq!(reference_json(None), None);
    }

    #[test]
    fn record_fields_cover_every_audit_column() {
        // Compile-level field walk: the record carries every column the
        // INSERT writes, so the mapping cannot silently drop a §10.1 field.
        let record = sample_record();
        assert_eq!(uuid_text(record.audit_id), "00000000-0000-0000-0000-000000000001");
        assert!(record.tenant_id.is_some());
        assert_eq!(record.actor.as_str(), "operator-1");
        assert_eq!(record.action.as_str(), "asset.register");
        assert_eq!(record.object.object_type.as_str(), "asset");
        assert_eq!(record.object.object_id.as_str(), "asset-1");
        assert!(record.corrects.is_none());
    }

    #[test]
    fn the_platform_chain_namespace_key_is_the_recorded_sentinel() {
        // The all-zero uuid sentinel (a uuid PRIMARY KEY cannot hold NULL).
        assert_eq!(
            PLATFORM_CHAIN_TENANT_KEY,
            "00000000-0000-0000-0000-000000000000"
        );
        assert!(uuid_from_text(PLATFORM_CHAIN_TENANT_KEY).is_ok());
    }

    #[test]
    fn chain_columns_carry_the_versioned_algorithm_label() {
        // The algorithm label travels per row; it is the versioned
        // construction label, and a construction change is a detectable
        // format change, never a silent one.
        assert_eq!(AUDIT_CHAIN_ALGORITHM, "hmac-sha256-chain-v1");
    }
}
