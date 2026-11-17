// FRV1-R8-005 (final review) — the control-plane port implementation over
// the Postgres adapter's webhook-delivery bookkeeping store (migration
// 0020). The PORT (`WebhookDeliveryStore`) is single-homed in
// [`crate::webhook_service`] (its error vocabulary names the app's signing
// error, so the trait cannot live in the adapter); the STORAGE half is
// single-homed in
// `crates/adapters/postgres/src/webhook_delivery.rs` (the P13.09 decision
// record §5 step 1). This module is the thin impl that joins the two —
// record mapping, vocabulary validation through the service module's own
// `validate` rules, and the named-error mapping. No SQL here.
//
// ORPHAN-RULE SHAPE: the trait is local to this crate and the store type
// is foreign, so the impl lives here — the composition root binds
// `Arc<PgWebhookDeliveryStore>` directly as the `WebhookDeliveryStore`
// port object. Every call opens its own tenant-scoped unit of work in the
// adapter (the recorded edge-enrollment pattern; the 0020 tables carry
// FORCE ROW LEVEL SECURITY), so this module NEVER borrows a per-request
// unit of work.
//
// WRITTEN-UNRUN: no webhook has ever been booked, attempted, or
// dead-lettered against PostgreSQL; the first human build/test is the
// gate (program conventions §1). The composition status (what is bound,
// what stays a recorded follow-up) is recorded in `main.rs` and the
// P13.09 decision record §5.

use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::ids::TenantId;
use flexcore_postgres_adapter::webhook_delivery::{
    PgWebhookDeliveryStore, WebhookAttemptInsert, WebhookDeadLetterInsert,
    WebhookDeliveryInsert, WebhookDeliveryStored, KEY_CONFLICT_CODE,
};

use crate::webhook_service::{
    WebhookAttemptRecord, WebhookDeadLetterRecord, WebhookDeliveryError, WebhookDeliveryRecord,
    WebhookDeliveryStore, DELIVERY_STATUSES,
};

/// Maps an adapter backend error onto the port's opaque backend variant
/// (the codes and messages are already human-safe and storage-text-free —
/// the adapter's error discipline, P1.07/P1.08).
fn backend_error(err: flexcore_ports::backend_error::BackendError) -> WebhookDeliveryError {
    WebhookDeliveryError::Backend {
        code: err.code().to_string(),
        message: err.message().to_string(),
    }
}

/// A stored value that cannot be represented in the port's record shape
/// is the named DECODE-class failure — never a guessed or wrapped value.
fn decode_error(message: &str) -> WebhookDeliveryError {
    WebhookDeliveryError::Backend {
        code: "E_STORE_DECODE".to_string(),
        message: message.to_string(),
    }
}

/// Parses canonical UUID text into a `TenantId` (the stored rows carry
/// the tenant as canonical UUID text; the adapter's `::uuid` casts are
/// the shape authority, so a non-parse here is a DECODE-class failure).
fn parse_tenant(text: &str) -> Result<TenantId, WebhookDeliveryError> {
    FlexUuid::parse(text)
        .map(TenantId::new)
        .map_err(|_| decode_error("the stored tenant_id could not be decoded"))
}

/// Assembles the port record from the stored row (the tenant is the
/// stored row's own; a caller only ever sees its own tenant's rows —
/// P0.16 rule 5).
fn stored_to_record(
    stored: WebhookDeliveryStored,
) -> Result<WebhookDeliveryRecord, WebhookDeliveryError> {
    let tenant_id = parse_tenant(&stored.tenant_id)?;
    let next_attempt_seq = u32::try_from(stored.next_attempt_seq)
        .map_err(|_| decode_error("the stored next_attempt_seq is out of range"))?;
    let last_response_code = match stored.last_response_code {
        None => None,
        Some(code) => Some(
            u16::try_from(code)
                .map_err(|_| decode_error("the stored last_response_code is out of range"))?,
        ),
    };
    Ok(WebhookDeliveryRecord {
        delivery_id: stored.delivery_id,
        tenant_id,
        delivery_key: stored.delivery_key,
        endpoint_id: stored.endpoint_id,
        event_subject: stored.event_subject,
        raw_body: stored.raw_body,
        status: stored.status,
        next_attempt_seq,
        next_attempt_not_before_unix_millis: stored.next_attempt_not_before_unix_millis,
        last_response_code,
    })
}

impl WebhookDeliveryStore for PgWebhookDeliveryStore {
    fn book_delivery(
        &self,
        record: WebhookDeliveryRecord,
    ) -> Result<WebhookDeliveryRecord, WebhookDeliveryError> {
        // The service module's validate is the vocabulary authority (the
        // closed status list, the non-blank rules); the adapter's SQL
        // CHECKs are the database backstop.
        record.validate()?;
        let insert = WebhookDeliveryInsert {
            delivery_id: record.delivery_id.clone(),
            delivery_key: record.delivery_key.clone(),
            endpoint_id: record.endpoint_id.clone(),
            event_subject: record.event_subject.clone(),
            raw_body: record.raw_body.clone(),
            status: record.status.clone(),
            next_attempt_seq: i64::from(record.next_attempt_seq),
            next_attempt_not_before_unix_millis: record.next_attempt_not_before_unix_millis,
            last_response_code: record.last_response_code.map(i32::from),
        };
        match self.book_delivery(record.tenant_id, &insert) {
            Ok(stored) => stored_to_record(stored),
            Err(err) if err.code() == KEY_CONFLICT_CODE => {
                Err(WebhookDeliveryError::ConflictingDeliveryKey {
                    key: record.delivery_key,
                })
            }
            Err(err) => Err(backend_error(err)),
        }
    }

    fn mark_in_flight(
        &self,
        tenant_id: &TenantId,
        delivery_id: &str,
    ) -> Result<(), WebhookDeliveryError> {
        self.mark_in_flight(*tenant_id, delivery_id)
            .map_err(backend_error)
    }

    fn book_attempt(
        &self,
        tenant_id: &TenantId,
        record: WebhookAttemptRecord,
    ) -> Result<(), WebhookDeliveryError> {
        record.validate()?;
        let insert = WebhookAttemptInsert {
            delivery_id: record.delivery_id,
            attempt_seq: i64::from(record.attempt_seq),
            outcome: record.outcome,
            booked_at_unix_millis: record.booked_at_unix_millis,
        };
        self.book_attempt(*tenant_id, &insert).map_err(backend_error)
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
        // Vocabulary validation reuses the service module's own record
        // rules (one authority — no second closed list here). The probe's
        // handle fields are validation placeholders and are never stored.
        let probe = WebhookAttemptRecord {
            attempt_id: "validation-probe".to_string(),
            delivery_id: delivery_id.to_string(),
            attempt_seq,
            outcome: outcome.to_string(),
            booked_at_unix_millis: 1,
            concluded_at_unix_millis: Some(concluded_at_unix_millis),
            response_code,
            failure_class,
        };
        probe.validate()?;
        self.record_attempt_outcome(
            *tenant_id,
            delivery_id,
            attempt_seq,
            outcome,
            response_code,
            probe.failure_class.as_deref(),
            concluded_at_unix_millis,
        )
        .map_err(backend_error)
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
        self.set_delivery_schedule(
            *tenant_id,
            delivery_id,
            status,
            next_attempt_seq,
            next_attempt_not_before_unix_millis,
            last_response_code,
        )
        .map_err(backend_error)
    }

    fn list_due_deliveries(
        &self,
        now_unix_millis: i64,
        stale_booked_before_unix_millis: i64,
        limit: u32,
    ) -> Result<Vec<WebhookDeliveryRecord>, WebhookDeliveryError> {
        self.list_due_deliveries(now_unix_millis, stale_booked_before_unix_millis, limit)
            .map_err(backend_error)?
            .into_iter()
            .map(stored_to_record)
            .collect()
    }

    fn promote_to_dead_letter(
        &self,
        record: WebhookDeadLetterRecord,
    ) -> Result<(), WebhookDeliveryError> {
        record.validate()?;
        let insert = WebhookDeadLetterInsert {
            delivery_id: record.delivery_id,
            endpoint_id: record.endpoint_id,
            reason: record.reason,
            dead_lettered_at_unix_millis: record.dead_lettered_at_unix_millis,
        };
        self.promote_to_dead_letter(record.tenant_id, &insert)
            .map_err(backend_error)
    }

    fn count_dead_letters(&self) -> Result<u64, WebhookDeliveryError> {
        self.count_dead_letters().map_err(backend_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The port object shape: the adapter type is usable as the
    /// composition root's `Arc<dyn WebhookDeliveryStore>` (Clone + Send +
    /// Sync), and the tenant parse fails closed on non-UUID text.
    #[test]
    fn the_port_object_shape_and_tenant_parse_fail_closed() {
        fn assert_port_object<T: Clone + Send + Sync>() {}
        assert_port_object::<PgWebhookDeliveryStore>();
        assert!(parse_tenant("0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a01").is_ok());
        assert!(parse_tenant("not-a-uuid").is_err());
    }

    /// A stored row with an out-of-range sequence or response code is a
    /// decode failure, never a wrapped value (the fail-closed decode
    /// discipline).
    #[test]
    fn stored_to_record_rejects_out_of_range_fields() {
        let base = |seq: i64, code: Option<i32>| WebhookDeliveryStored {
            tenant_id: "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a01".to_string(),
            delivery_id: "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a02".to_string(),
            delivery_key: "k".to_string(),
            endpoint_id: "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a03".to_string(),
            event_subject: "flexcore.v1.tenant.a.telemetry".to_string(),
            raw_body: "{}".to_string(),
            status: "PENDING".to_string(),
            next_attempt_seq: seq,
            next_attempt_not_before_unix_millis: None,
            last_response_code: code,
        };
        assert!(matches!(
            stored_to_record(base(i64::from(u32::MAX) + 1, None)),
            Err(WebhookDeliveryError::Backend { code, .. }) if code == "E_STORE_DECODE"
        ));
        assert!(matches!(
            stored_to_record(base(1, Some(i32::from(u16::MAX) + 1))),
            Err(WebhookDeliveryError::Backend { code, .. }) if code == "E_STORE_DECODE"
        ));
        let ok = stored_to_record(base(1, Some(200))).expect("in-range fields decode");
        assert_eq!(ok.last_response_code, Some(200));
    }
}
