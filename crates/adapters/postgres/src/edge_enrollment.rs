// P6.06 — PostgreSQL implementation of the edge-enrollment persistence
//         contract (report §18.1, §18.3, §17.3, §16.2; ledger P6.06;
//         migration 0018; decision record:
//         `ledger/phase-06-edge-protocol/06_07_identity_channel.md`).
//
// Implements `flexcore_storage::edge_enrollment::EdgeEnrollmentStore` for
// [`FlexCoreStore`]. Unlike the repository ports (which run inside the
// caller's one unit of work), the enrollment surface opens ITS OWN unit of
// work per call with the caller's tenant context: the enrollment route is a
// control-plane provisioning surface, and each registry write + its paired
// journal event commit or roll back TOGETHER inside that unit (§15.5
// discipline applied at the enrollment boundary).
//
// Tenant scoping is the exact command-store discipline: the
// transaction-scoped `flexcore.tenant_id` GUC feeds the `tenant_isolation`
// RLS policy (ENABLE + FORCE, migration 0018), every statement filters by
// the context's tenant explicitly, and a foreign-tenant device reads as
// `None` — existence never leaks (P0.16 rule 5).
//
// Append-only (migration 0018): both tables carry flexcore_app
// INSERT + SELECT grants and unconditional UPDATE/DELETE guard triggers;
// a device's lifecycle state is its HIGHEST-sequence registry row, and a
// partial unique index permits at most ONE ACTIVE term per
// (device_id, tenant_id) — the ledger's "no two edges share an identity"
// enforced at the database level. A duplicate-identity insert maps to the
// named [`EdgeEnrollmentStoreError::Conflict`].
//
// VERIFY-BUILD (field-visibility precedent): this module accesses the
// private `tx`/`runtime` fields of `PgUnitOfWork` from a sibling module —
// exactly the pattern the existing `command_store.rs`, `telemetry.rs`,
// `schedule_store.rs`, and `feasibility_check.rs` modules use for their
// `impl` blocks.

use crate::error::{is_unique_violation, map_backend};
use crate::store::FlexCoreStore;
use crate::unit_of_work::tx_ref;
use crate::uuid_map::uuid_text;
use flexcore_domain::ids::TenantId;
use flexcore_domain::tenant_context::TenantContext;
use flexcore_storage::edge_enrollment::{
    EdgeDeviceIdentityRow, EdgeEnrollmentEventRow, EdgeEnrollmentStore, EdgeEnrollmentStoreError,
};

/// Maps a [`flexcore_ports::backend_error::BackendError`] (from begin/commit)
/// to the opaque, human-safe storage error.
fn map_backend_error_of(
    err: flexcore_ports::backend_error::BackendError,
) -> EdgeEnrollmentStoreError {
    EdgeEnrollmentStoreError::Backend {
        code: err.code().to_string(),
        message: err.message().to_string(),
    }
}

/// Maps a raw sqlx error during an INSERT to the store error; a unique
/// violation is the identity Conflict (the registry's uniqueness is the
/// ledger acceptance criterion, so it gets the named variant).
fn map_insert_error(err: sqlx::Error, device_id: &str) -> EdgeEnrollmentStoreError {
    if is_unique_violation(&err) {
        return EdgeEnrollmentStoreError::Conflict {
            device_id: device_id.to_string(),
        };
    }
    let backend = map_backend(err);
    EdgeEnrollmentStoreError::Backend {
        code: backend.code().to_string(),
        message: backend.message().to_string(),
    }
}

/// Maps a raw sqlx error during a SELECT to the store error.
fn map_select_error(err: sqlx::Error) -> EdgeEnrollmentStoreError {
    let backend = map_backend(err);
    EdgeEnrollmentStoreError::Backend {
        code: backend.code().to_string(),
        message: backend.message().to_string(),
    }
}

/// Decodes one `edge_device_registry` row into the storage row type.
fn decode_device_row(
    row: &sqlx::postgres::PgRow,
    tenant_id: TenantId,
) -> Result<EdgeDeviceIdentityRow, EdgeEnrollmentStoreError> {
    let decode = |column: &'static str| -> Result<String, EdgeEnrollmentStoreError> {
        row.try_get::<String, _>(column).map_err(|_| {
            EdgeEnrollmentStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    let enrollment_seq = i64::try_from(
        // The column reads back as i64; a negative value is impossible by
        // the migration CHECK — map defensively.
        row.try_get::<i64, _>("enrollment_seq").map_err(|_| {
            EdgeEnrollmentStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: "stored value in column enrollment_seq could not be decoded".to_string(),
            }
        })?,
    )
    .map_err(|_| EdgeEnrollmentStoreError::Backend {
        code: "E_STORE_DECODE".to_string(),
        message: "stored value in column enrollment_seq is out of range".to_string(),
    })?;
    EdgeDeviceIdentityRow::new(
        tenant_id,
        decode("device_id")?,
        decode("site_id")?,
        enrollment_seq as u64,
        decode("status")?,
        row.try_get::<Option<String>, _>("approved_asset_scope")
            .map_err(|_| EdgeEnrollmentStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: "stored value in column approved_asset_scope could not be decoded"
                    .to_string(),
            })?,
        row.try_get::<i64, _>("enrolled_at_unix_millis").map_err(|_| {
            EdgeEnrollmentStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: "stored value in column enrolled_at_unix_millis could not be decoded"
                    .to_string(),
            }
        })?,
        row.try_get::<Option<i64>, _>("expires_at_unix_millis")
            .map_err(|_| EdgeEnrollmentStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: "stored value in column expires_at_unix_millis could not be decoded"
                    .to_string(),
            })?,
    )
}

/// Decodes one `edge_enrollment_journal` row into the storage row type.
fn decode_event_row(
    row: &sqlx::postgres::PgRow,
    tenant_id: TenantId,
) -> Result<EdgeEnrollmentEventRow, EdgeEnrollmentStoreError> {
    let try_text = |column: &'static str| -> Result<String, EdgeEnrollmentStoreError> {
        row.try_get::<String, _>(column).map_err(|_| {
            EdgeEnrollmentStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    let occurred = row.try_get::<i64, _>("occurred_at_unix_millis").map_err(|_| {
        EdgeEnrollmentStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored value in column occurred_at_unix_millis could not be decoded"
                .to_string(),
        }
    })?;
    EdgeEnrollmentEventRow::new(
        tenant_id,
        try_text("event_id")?,
        try_text("device_id")?,
        try_text("event_type")?,
        try_text("actor")?,
        row.try_get::<Option<String>, _>("reason").map_err(|_| {
            EdgeEnrollmentStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: "stored value in column reason could not be decoded".to_string(),
            }
        })?,
        try_text("correlation_id")?,
        occurred,
    )
}

impl EdgeEnrollmentStore for FlexCoreStore {
    fn enroll_device(
        &self,
        ctx: &TenantContext,
        row: &EdgeDeviceIdentityRow,
    ) -> Result<(), EdgeEnrollmentStoreError> {
        let tenant = uuid_text(ctx.tenant_id().uuid());
        let device_id = row.device_id.clone();
        let mut unit = self.begin(Some(ctx)).map_err(map_backend_error_of)?;
        let outcome: Result<(), EdgeEnrollmentStoreError> = unit.runtime.block_on(async {
            let mut guard = unit.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| EdgeEnrollmentStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            sqlx::query(
                "INSERT INTO edge_device_registry \
                     (device_id, tenant_id, site_id, enrollment_seq, status, \
                      approved_asset_scope, enrolled_at_unix_millis, expires_at_unix_millis) \
                 VALUES ($1, $2::uuid, $3::uuid, $4, $5, $6, $7, $8)",
            )
            .bind(&row.device_id)
            .bind(&tenant)
            .bind(&row.site_id)
            .bind(row.enrollment_seq as i64)
            .bind(&row.status)
            .bind(row.approved_asset_scope.as_deref())
            .bind(row.enrolled_at_unix_millis)
            .bind(row.expires_at_unix_millis)
            .execute(&mut *tx)
            .await
            .map(|_| ())
            .map_err(|err| map_insert_error(err, &device_id))
        });
        match outcome {
            Ok(()) => unit.commit().map_err(map_backend_error_of)?,
            Err(error) => {
                // Drop the unfinalized unit of work → native rollback.
                drop(unit);
                return Err(error);
            }
        }
        Ok(())
    }

    fn find_device(
        &self,
        ctx: &TenantContext,
        device_id: &str,
    ) -> Result<Option<EdgeDeviceIdentityRow>, EdgeEnrollmentStoreError> {
        if device_id.trim().is_empty() {
            return Err(EdgeEnrollmentStoreError::InvalidRecord { field: "device_id" });
        }
        let tenant = uuid_text(ctx.tenant_id().uuid());
        let device_id = device_id.to_string();
        let unit = self.begin(Some(ctx)).map_err(map_backend_error_of)?;
        let outcome: Result<Option<EdgeDeviceIdentityRow>, EdgeEnrollmentStoreError> =
            unit.runtime.block_on(async {
                let mut guard = unit.tx.lock().await;
                let tx = tx_ref(&mut guard)
                    .await
                    .map_err(|err| EdgeEnrollmentStoreError::Backend {
                        code: err.code().to_string(),
                        message: err.message().to_string(),
                    })?;
                let row = sqlx::query(
                    "SELECT device_id, site_id::text AS site_id, enrollment_seq, status, \
                            approved_asset_scope, enrolled_at_unix_millis, \
                            expires_at_unix_millis \
                     FROM edge_device_registry \
                     WHERE tenant_id = $1::uuid AND device_id = $2 \
                     ORDER BY enrollment_seq DESC LIMIT 1",
                )
                .bind(&tenant)
                .bind(&device_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(map_select_error)?;
                match row {
                    // A foreign-tenant device reads as None — no existence
                    // leak (P0.16 rule 5; RLS already filtered it).
                    None => Ok(None),
                    Some(row) => decode_device_row(&row, ctx.tenant_id()).map(Some),
                }
            });
        // A read-only unit of work: commit is unnecessary, but a clean
        // close is free — drop rolls back an empty transaction harmlessly.
        drop(unit);
        outcome
    }

    fn site_is_approved(
        &self,
        ctx: &TenantContext,
        site_id: &str,
    ) -> Result<bool, EdgeEnrollmentStoreError> {
        if site_id.trim().is_empty() {
            return Err(EdgeEnrollmentStoreError::InvalidRecord { field: "site_id" });
        }
        let tenant = uuid_text(ctx.tenant_id().uuid());
        let site_id = site_id.to_string();
        let unit = self.begin(Some(ctx)).map_err(map_backend_error_of)?;
        let outcome = unit.runtime.block_on(async {
            let mut guard = unit.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| EdgeEnrollmentStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let exists: (bool,) = sqlx::query_as(
                "SELECT EXISTS( \
                        SELECT 1 FROM sites WHERE site_id = $1::uuid AND tenant_id = $2::uuid)",
            )
            .bind(&site_id)
            .bind(&tenant)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_select_error)?;
            Ok(exists.0)
        });
        drop(unit);
        outcome
    }

    fn append_event(
        &self,
        ctx: &TenantContext,
        event: &EdgeEnrollmentEventRow,
    ) -> Result<(), EdgeEnrollmentStoreError> {
        let tenant = uuid_text(ctx.tenant_id().uuid());
        let mut unit = self.begin(Some(ctx)).map_err(map_backend_error_of)?;
        let outcome: Result<(), EdgeEnrollmentStoreError> = unit.runtime.block_on(async {
            let mut guard = unit.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| EdgeEnrollmentStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            sqlx::query(
                "INSERT INTO edge_enrollment_journal \
                     (event_id, tenant_id, device_id, event_type, actor, reason, \
                      correlation_id, occurred_at_unix_millis) \
                 VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6, $7, $8)",
            )
            .bind(&event.event_id)
            .bind(&tenant)
            .bind(&event.device_id)
            .bind(&event.event_type)
            .bind(&event.actor)
            .bind(event.reason.as_deref())
            .bind(&event.correlation_id)
            .bind(event.occurred_at_unix_millis)
            .execute(&mut *tx)
            .await
            .map(|_| ())
            .map_err(map_select_error)
        });
        match outcome {
            Ok(()) => unit.commit().map_err(map_backend_error_of)?,
            Err(error) => {
                drop(unit);
                return Err(error);
            }
        }
        Ok(())
    }

    fn list_events(
        &self,
        ctx: &TenantContext,
        device_id: &str,
        limit: u32,
    ) -> Result<Vec<EdgeEnrollmentEventRow>, EdgeEnrollmentStoreError> {
        if device_id.trim().is_empty() {
            return Err(EdgeEnrollmentStoreError::InvalidRecord { field: "device_id" });
        }
        let tenant = uuid_text(ctx.tenant_id().uuid());
        let device_id = device_id.to_string();
        let limit = i64::from(limit);
        let unit = self.begin(Some(ctx)).map_err(map_backend_error_of)?;
        let outcome: Result<Vec<EdgeEnrollmentEventRow>, EdgeEnrollmentStoreError> =
            unit.runtime.block_on(async {
                let mut guard = unit.tx.lock().await;
                let tx = tx_ref(&mut guard)
                    .await
                    .map_err(|err| EdgeEnrollmentStoreError::Backend {
                        code: err.code().to_string(),
                        message: err.message().to_string(),
                    })?;
                let rows = sqlx::query(
                    "SELECT event_id::text AS event_id, device_id, event_type, actor, reason, \
                            correlation_id, occurred_at_unix_millis \
                     FROM edge_enrollment_journal \
                     WHERE tenant_id = $1::uuid AND device_id = $2 \
                     ORDER BY occurred_at_unix_millis ASC, event_id ASC LIMIT $3",
                )
                .bind(&tenant)
                .bind(&device_id)
                .bind(limit)
                .fetch_all(&mut *tx)
                .await
                .map_err(map_select_error)?;
                rows.iter()
                    .map(|row| decode_event_row(row, ctx.tenant_id()))
                    .collect()
            });
        drop(unit);
        outcome
    }
}

/// The object-safety evidence pattern (the same compile-level check the
/// other stores carry): `PgUnitOfWork`-sibling modules keep the trait
/// usable as a composition-root `Arc<dyn EdgeEnrollmentStore>`.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_edge_enrollment_store_is_object_safe() {
        fn takes_dyn_store(_: &dyn EdgeEnrollmentStore) {}
        let _ = takes_dyn_store as fn(&dyn EdgeEnrollmentStore);
    }
}
