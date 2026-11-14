// P4.30 — PostgreSQL implementation of the feasibility-check evidence
//         record persistence contract (report §15.2–§15.3, §16.1–§16.2,
//         §21.3; ledger P4.30; migration 0017; decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/30_persistence_wiring.md`).
//
// Implements `flexcore_storage::feasibility_check::FeasibilityCheckStore`
// for [`PgUnitOfWork`], the same unit-of-work pattern as the command store:
// every statement runs inside the caller's one native transaction, so a
// check record and the paired outbox row (`FeasibilityCheckCompleted`)
// commit or roll back TOGETHER (§15.5).
//
// The row is append-only: flexcore_app holds INSERT + SELECT only, and
// migration 0017's trigger refuses UPDATE and DELETE unconditionally (the
// 0008 audit_records pattern). Tenant scoping is the exact command-store
// discipline: the transaction-scoped `flexcore.tenant_id` GUC feeds the
// `tenant_isolation` RLS policy (ENABLE + FORCE), every statement filters
// by the context's tenant explicitly, and a foreign-tenant check reads as
// `None` — existence never leaks (P0.16 rule 5).
//
// VERIFY-BUILD (field-visibility precedent): this module accesses the
// private `tx`/`runtime` fields of `PgUnitOfWork` from a sibling module —
// exactly the pattern the existing `command_store.rs`, `telemetry.rs`, and
// `schedule_store.rs` modules use for their `impl PgUnitOfWork` blocks.

use crate::error::{is_unique_violation, map_backend};
use crate::unit_of_work::{tx_ref, PgUnitOfWork};
use crate::uuid_map::{uuid_from_text, uuid_text};
use flexcore_domain::tenant_context::TenantContext;
use flexcore_storage::feasibility_check::{
    FeasibilityCheckFilter, FeasibilityCheckRow, FeasibilityCheckStore,
    FeasibilityCheckStoreError,
};

/// The column projection of one feasibility-check record (uuid columns
/// selected back as `::text` — the recorded FlexUuid mapping).
const FEASIBILITY_CHECK_SELECT: &str = "SELECT check_id, tenant_id::text AS tenant_id, \
     command_id::text AS command_id, portfolio_id::text AS portfolio_id, \
     site_id::text AS site_id, asset_id::text AS asset_id, \
     schedule_id::text AS schedule_id, schedule_revision, model_id, model_version, \
     model_content_hash, outcome, failing_constraint, reason, margins, coverage, \
     actor, correlation_id, checked_at_unix_millis FROM feasibility_check_records";

/// Validates one canonical-UUID text field, mapping a malformed value to
/// the named storage error (the adapter fails closed before the database
/// sees a value it could only reject with raw engine text).
fn require_uuid_text(value: &str, field: &'static str) -> Result<(), FeasibilityCheckStoreError> {
    uuid_from_text(value)
        .map(|_| ())
        .map_err(|_| FeasibilityCheckStoreError::InvalidRecord { field })
}

fn map_backend_error(err: sqlx::Error, check_id: &str) -> FeasibilityCheckStoreError {
    if is_unique_violation(&err) {
        return FeasibilityCheckStoreError::Conflict {
            check_id: check_id.to_string(),
        };
    }
    let backend = map_backend(err);
    FeasibilityCheckStoreError::Backend {
        code: backend.code().to_string(),
        message: backend.message().to_string(),
    }
}

/// Decodes one `feasibility_check_records` row into the storage row type,
/// re-validating through the row constructor (fail closed on a malformed
/// row — the same discipline as the command store).
fn decode_feasibility_check_row(
    row: &sqlx::postgres::PgRow,
) -> Result<FeasibilityCheckRow, FeasibilityCheckStoreError> {
    let try_text = |column: &'static str| -> Result<String, FeasibilityCheckStoreError> {
        row.try_get::<String, _>(column).map_err(|_| {
            FeasibilityCheckStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    let try_opt_text =
        |column: &'static str| -> Result<Option<String>, FeasibilityCheckStoreError> {
            row.try_get::<Option<String>, _>(column).map_err(|_| {
                FeasibilityCheckStoreError::Backend {
                    code: "E_STORE_DECODE".to_string(),
                    message: format!("stored value in column {column} could not be decoded"),
                }
            })
        };
    let check_id = try_text("check_id")?;
    let tenant_text = try_text("tenant_id")?;
    let tenant_uuid = uuid_from_text(&tenant_text).map_err(|_| {
        FeasibilityCheckStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored value in column tenant_id could not be decoded".to_string(),
        }
    })?;
    let schedule_revision = u64::try_from(
        row.try_get::<i64, _>("schedule_revision").map_err(|_| {
            FeasibilityCheckStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: "stored value in column schedule_revision could not be decoded"
                    .to_string(),
            }
        })?,
    )
    .map_err(|_| FeasibilityCheckStoreError::Backend {
        code: "E_STORE_DECODE".to_string(),
        message: "stored value in column schedule_revision is out of range".to_string(),
    })?;
    FeasibilityCheckRow::new(
        flexcore_domain::ids::TenantId::new(flexcore_contracts::ids::FlexUuid::from_bytes(
            tenant_uuid.to_bytes(),
        )),
        check_id,
        try_opt_text("command_id")?,
        try_text("portfolio_id")?,
        try_text("site_id")?,
        try_text("asset_id")?,
        try_text("schedule_id")?,
        schedule_revision,
        try_opt_text("model_id")?,
        try_opt_text("model_version")?,
        try_opt_text("model_content_hash")?,
        try_text("outcome")?,
        try_opt_text("failing_constraint")?,
        try_opt_text("reason")?,
        try_opt_text("margins")?,
        try_opt_text("coverage")?,
        try_text("actor")?,
        try_text("correlation_id")?,
        row.try_get::<i64, _>("checked_at_unix_millis")
            .map_err(|_| FeasibilityCheckStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: "stored value in column checked_at_unix_millis could not be decoded"
                    .to_string(),
            })?,
    )
    .map_err(|_| FeasibilityCheckStoreError::Backend {
        code: "E_STORE_DECODE".to_string(),
        message: "the stored feasibility-check row failed the storage-level rules".to_string(),
    })
}

impl FeasibilityCheckStore for PgUnitOfWork {
    fn insert_feasibility_check(
        &self,
        ctx: &TenantContext,
        row: &FeasibilityCheckRow,
    ) -> Result<(), FeasibilityCheckStoreError> {
        if let Some(command) = row.command_id.as_deref() {
            require_uuid_text(command, "command_id")?;
        }
        require_uuid_text(&row.portfolio_id, "portfolio_id")?;
        require_uuid_text(&row.site_id, "site_id")?;
        require_uuid_text(&row.asset_id, "asset_id")?;
        require_uuid_text(&row.schedule_id, "schedule_id")?;
        let schedule_revision = i64::try_from(row.schedule_revision).map_err(|_| {
            FeasibilityCheckStoreError::InvalidRecord {
                field: "schedule_revision",
            }
        })?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| FeasibilityCheckStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            // Append-only insert: the (check_id, tenant_id) pair is the
            // primary key; a duplicate is the immutability conflict (a
            // re-checked candidate is a NEW check id). The verdict-shape
            // rules were validated by the row constructor; the migration's
            // CHECKs re-state them as defence in depth.
            sqlx::query(
                "INSERT INTO feasibility_check_records \
                     (check_id, tenant_id, command_id, portfolio_id, site_id, asset_id, \
                      schedule_id, schedule_revision, model_id, model_version, \
                      model_content_hash, outcome, failing_constraint, reason, margins, \
                      coverage, actor, correlation_id, checked_at_unix_millis) \
                 VALUES ($1, $2::uuid, $3::uuid, $4::uuid, $5::uuid, $6::uuid, \
                         $7::uuid, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)",
            )
            .bind(&row.check_id)
            .bind(uuid_text(ctx.tenant_id().uuid()))
            .bind(row.command_id.as_deref())
            .bind(&row.portfolio_id)
            .bind(&row.site_id)
            .bind(&row.asset_id)
            .bind(&row.schedule_id)
            .bind(schedule_revision)
            .bind(row.model_id.as_deref())
            .bind(row.model_version.as_deref())
            .bind(row.model_content_hash.as_deref())
            .bind(&row.outcome)
            .bind(row.failing_constraint.as_deref())
            .bind(row.reason.as_deref())
            .bind(row.margins.as_deref())
            .bind(row.coverage.as_deref())
            .bind(&row.actor)
            .bind(&row.correlation_id)
            .bind(row.checked_at_unix_millis)
            .execute(&mut *tx)
            .await
            .map_err(|err| map_backend_error(err, &row.check_id))?;
            Ok(())
        })
    }

    fn find_feasibility_check(
        &self,
        ctx: &TenantContext,
        check_id: &str,
    ) -> Result<Option<FeasibilityCheckRow>, FeasibilityCheckStoreError> {
        if check_id.trim().is_empty() {
            return Err(FeasibilityCheckStoreError::InvalidRecord { field: "check_id" });
        }
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| FeasibilityCheckStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let row = sqlx::query(&format!(
                "{FEASIBILITY_CHECK_SELECT} WHERE check_id = $1 AND tenant_id = $2::uuid"
            ))
            .bind(check_id)
            .bind(uuid_text(ctx.tenant_id().uuid()))
            .fetch_optional(&mut *tx)
            .await
            .map_err(|err| map_backend_error(err, check_id))?;
            match row {
                // A foreign-tenant row reads as None — no existence leak
                // (P0.16 rule 5; RLS already filtered it).
                None => Ok(None),
                Some(row) => decode_feasibility_check_row(&row).map(Some),
            }
        })
    }

    fn list_feasibility_checks(
        &self,
        ctx: &TenantContext,
        filter: &FeasibilityCheckFilter,
        limit: u32,
    ) -> Result<Vec<FeasibilityCheckRow>, FeasibilityCheckStoreError> {
        if let Some(command) = filter.command_id.as_deref() {
            require_uuid_text(command, "command_id")?;
        }
        if let Some(schedule) = filter.schedule_id.as_deref() {
            require_uuid_text(schedule, "schedule_id")?;
        }
        let limit = i64::from(limit);
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| FeasibilityCheckStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            // Ordered by (checked_at, check_id) — the trait's documented
            // listing order — with the explicit tenant filter on top of RLS
            // (P0.16 rule 5). Every filter value is a bound parameter; the
            // limit is ALWAYS applied (no unbounded query).
            let mut statement = format!(
                "{FEASIBILITY_CHECK_SELECT} WHERE tenant_id = $1::uuid"
            );
            let mut binds: Vec<String> = Vec::new();
            if let Some(command) = filter.command_id.as_deref() {
                binds.push(command.to_string());
                statement.push_str(&format!(" AND command_id = ${}::uuid", binds.len() + 1));
            }
            if let Some(schedule) = filter.schedule_id.as_deref() {
                binds.push(schedule.to_string());
                statement.push_str(&format!(" AND schedule_id = ${}::uuid", binds.len() + 1));
            }
            if let Some(after) = filter.after_check_id.as_deref() {
                binds.push(after.to_string());
                statement.push_str(&format!(
                    " AND (checked_at_unix_millis, check_id) > \
                     ((SELECT checked_at_unix_millis FROM feasibility_check_records \
                       WHERE check_id = ${} AND tenant_id = $1::uuid), ${})",
                    binds.len() + 1,
                    binds.len() + 1
                ));
            }
            statement.push_str(&format!(
                " ORDER BY checked_at_unix_millis ASC, check_id ASC LIMIT ${}",
                binds.len() + 2
            ));
            let mut query = sqlx::query(&statement).bind(uuid_text(ctx.tenant_id().uuid()));
            for bind in &binds {
                query = query.bind(bind);
            }
            let rows = query
                .bind(limit)
                .fetch_all(&mut *tx)
                .await
                .map_err(|err| map_backend_error(err, "listing"))?;
            rows.iter().map(decode_feasibility_check_row).collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The composition root holds ports as trait objects; object safety is
    // part of the contract (the same evidence pattern as the adapter's
    // `every_implemented_port_is_object_safe`). Compile-level evidence that
    // PgUnitOfWork carries the feasibility-check store.
    #[test]
    fn the_feasibility_check_store_is_object_safe_for_the_unit_of_work() {
        fn takes_dyn_store(_: &dyn FeasibilityCheckStore) {}
        let _ = takes_dyn_store as fn(&dyn FeasibilityCheckStore);
    }
}
