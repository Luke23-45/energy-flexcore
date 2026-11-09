// P3.21 — PostgreSQL implementation of the schedule-record persistence
//         contract (report §12.6, §16.1, §15.5, §16.2; ledger P3.21 step 3;
//         migration 0013).
//
// Implements `flexcore_storage::schedule_store::ScheduleRecordStore` for
// [`PgUnitOfWork`], the same unit-of-work pattern as the P1.13 repositories
// and the P2.06 telemetry store: every statement runs inside the caller's
// one native transaction, so the record insert and the paired
// `ScheduleProposed` outbox row (appended through the unit of work's
// `EventOutbox` implementation) are atomic — state + outbox, or neither
// (§15.5; the P1.13 atomicity test is the mechanical proof of the pattern).
//
// Tenant scoping (P1.12 §5.1 conventions): the unit of work's
// transaction-scoped `flexcore.tenant_id` GUC feeds the `tenant_isolation`
// RLS policy on `schedule_results` (FORCEd, so even owner sessions are
// subject); on top of RLS both statements filter by the context's tenant
// explicitly, and a foreign-tenant record reads as `None` — existence never
// leaks across a tenant boundary (P0.16 rule 5).
//
// Immutability: INSERT and SELECT only (the migration grants
// `flexcore_app` exactly those); a candidate-identity collision maps to
// [`ScheduleStoreError::Conflict`] — records are immutable and a re-solve
// is a new candidate (P3.22).
//
// Payload rule: the row's `record_payload_json` is validated to parse as
// JSON here (the storage crate cannot — no serialization dependency), then
// bound as `jsonb`. On read the jsonb is re-rendered to its canonical text
// form.
//
// P3.22/P3.23 (appended additively): the same file implements
// `ScheduleRevisionChainStore` (the schedule identity/revision chain with
// the dedicated §14.2 SUPERSEDED transition; migration 0014) and
// `ScheduleComparisonStore` (immutable comparison records; migration 0014),
// both over the same unit-of-work transaction pattern, the same tenant
// scoping (RLS + explicit filter), and the same no-raw-storage-text error
// mapping. The SUPERSEDED marking runs through the column-level UPDATE grant
// on exactly the three transition columns; revision content has no update
// path (the immutability trigger is the database backstop).
//
// P3.28 (appended additively): the per-tenant keyset listing reads
// `ScheduleRecordStore::list_schedule_records` and
// `ScheduleRevisionChainStore::list_revisions` behind
// `GET /api/v1/optimization-runs` and `GET /api/v1/schedules` — keyset
// pagination over the records' primary keys, no offset pagination, same
// tenant scoping and error mapping.
//
// VERIFY-BUILD (field-visibility precedent): this module accesses the
// private `tx`/`runtime` fields of `PgUnitOfWork` from a sibling module —
// exactly the pattern the existing `telemetry.rs` and `corrections.rs`
// modules use for their `impl PgUnitOfWork` blocks. If rustc rejects that
// access (E0616), the single-line fix is `pub(crate)` on the two field
// declarations in `unit_of_work.rs`, which repairs every module at once;
// that file was outside this subtask's ownership.

use crate::error::{is_unique_violation, map_backend};
use crate::unit_of_work::{tx_ref, PgUnitOfWork};
use crate::uuid_map::uuid_from_text;
use flexcore_domain::tenant_context::TenantContext;
use flexcore_storage::schedule_revision::{
    ScheduleRevisionChainStore, ScheduleRevisionError, ScheduleRevisionReceipt,
    ScheduleRevisionRecord, ScheduleRevisionRow, ScheduleSupersedeTarget,
};
use flexcore_storage::schedule_store::{
    ComparisonStoreError, ScheduleComparisonReceipt, ScheduleComparisonRow,
    ScheduleComparisonStore, ScheduleRecordReceipt, ScheduleRecordRow, ScheduleRecordStore,
    ScheduleStoreError,
};
use serde_json::Value;

/// The column projection (uuid columns selected back as `::text` — the
/// recorded FlexUuid mapping).
const SCHEDULE_RESULT_SELECT: &str = "SELECT candidate_id::text AS candidate_id, \
     tenant_id::text AS tenant_id, portfolio_id::text AS portfolio_id, \
     site_id::text AS site_id, status, problem_class, solver_name, \
     solver_version, provider_id, provider_version, objective_value, \
     objective_units, input_snapshot_id, grid_model_version, \
     grid_model_site_id::text AS grid_model_site_id, \
     grid_model_site_reference, schedule_proposed_key, record_payload \
     FROM schedule_results";

impl ScheduleRecordStore for PgUnitOfWork {
    /// Inserts one §12.6 schedule-result record inside THIS unit of work's
    /// transaction (adapter obligation 1: the same native transaction must
    /// carry the paired `ScheduleProposed` outbox row). The candidate
    /// identity is the primary key: a collision maps to
    /// [`ScheduleStoreError::Conflict`] (records are immutable).
    fn insert_schedule_record(
        &self,
        ctx: &TenantContext,
        row: &ScheduleRecordRow,
    ) -> Result<ScheduleRecordReceipt, ScheduleStoreError> {
        // The payload must be valid JSON before it reaches the database —
        // the storage crate treats it as opaque text, this adapter owns the
        // jsonb binding and therefore the validation.
        let payload: Value = serde_json::from_str(&row.record_payload_json)
            .map_err(|_| ScheduleStoreError::InvalidRecord {
                field: "record_payload_json",
            })?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| ScheduleStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let result = sqlx::query(
                "INSERT INTO schedule_results \
                     (candidate_id, tenant_id, portfolio_id, site_id, status, \
                      problem_class, solver_name, solver_version, provider_id, \
                      provider_version, objective_value, objective_units, \
                      input_snapshot_id, grid_model_version, grid_model_site_id, \
                      grid_model_site_reference, schedule_proposed_key, record_payload) \
                 VALUES ($1::uuid, $2::uuid, $3::uuid, $4::uuid, $5, $6, $7, $8, $9, \
                         $10, $11, $12, $13, $14, $15::uuid, $16, $17, $18)",
            )
            .bind(&row.candidate_id)
            .bind(uuid_text(ctx.tenant_id().uuid()))
            .bind(&row.portfolio_id)
            .bind(&row.site_id)
            .bind(&row.status)
            .bind(&row.problem_class)
            .bind(&row.solver_name)
            .bind(&row.solver_version)
            .bind(&row.provider_id)
            .bind(&row.provider_version)
            .bind(row.objective_value)
            .bind(row.objective_units.as_deref())
            .bind(&row.input_snapshot_id)
            .bind(&row.grid_model_version)
            .bind(&row.grid_model_site_id)
            .bind(&row.grid_model_site_reference)
            .bind(&row.schedule_proposed_key)
            .bind(payload)
            .execute(&mut *tx)
            .await
            .map_err(|err| map_schedule_store_error(err, &row.candidate_id))?;
            if result.rows_affected() != 1 {
                return Err(ScheduleStoreError::Backend {
                    code: "E_STORE_REJECTED".to_string(),
                    message: "the schedule-result insert affected no rows".to_string(),
                });
            }
            Ok(ScheduleRecordReceipt::new(
                row.candidate_id.clone(),
                ctx.tenant_id(),
            ))
        })
    }

    /// Looks a record up by candidate identity within the caller's tenant
    /// scope (RLS GUC + explicit tenant filter; a foreign-tenant record
    /// reads as `None`).
    fn find_schedule_record(
        &self,
        ctx: &TenantContext,
        candidate_id: &str,
    ) -> Result<Option<ScheduleRecordRow>, ScheduleStoreError> {
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| ScheduleStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let row = sqlx::query(&format!(
                "{SCHEDULE_RESULT_SELECT} WHERE candidate_id = $1::uuid AND tenant_id = $2::uuid"
            ))
            .bind(candidate_id)
            .bind(uuid_text(ctx.tenant_id().uuid()))
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_schedule_store_error)?;
            match row {
                None => Ok(None),
                Some(row) => decode_schedule_record_row(&row).map(Some),
            }
        })
    }

    /// Lists records tenant-scoped (P3.28, added additively): keyset over
    /// `candidate_id` ascending, bounded by `limit`, filtered by the
    /// optional portfolio/site/status filters and the optional cursor
    /// position. No offset pagination anywhere.
    fn list_schedule_records(
        &self,
        ctx: &TenantContext,
        portfolio_id: Option<&str>,
        site_id: Option<&str>,
        status: Option<&str>,
        after_candidate_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ScheduleRecordRow>, ScheduleStoreError> {
        // The adapter validates the uuid-shaped filters and fails closed
        // with a named field before the database could reject them with raw
        // engine text; `status` is validated against the closed vocabulary.
        for (field, value) in [
            ("portfolio_id", portfolio_id),
            ("site_id", site_id),
            ("after_candidate_id", after_candidate_id),
        ] {
            if let Some(value) = value {
                uuid_from_text(value).map_err(|_| ScheduleStoreError::InvalidRecord { field })?;
            }
        }
        if let Some(status) = status {
            if !SCHEDULE_RESULT_STATUSES.contains(&status) {
                return Err(ScheduleStoreError::InvalidRecord { field: "status" });
            }
        }
        let limit_i64 = i64::try_from(limit).map_err(|_| ScheduleStoreError::Backend {
            code: "E_STORE_LIMIT".to_string(),
            message: "the page limit is out of range".to_string(),
        })?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| ScheduleStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            // One parameterized shape: the filters are always bound (None =
            // no constraint), so the statement text is constant.
            let rows = sqlx::query(&format!(
                "{SCHEDULE_RESULT_SELECT} \
                 WHERE tenant_id = $1::uuid \
                   AND ($2::uuid IS NULL OR portfolio_id = $2::uuid) \
                   AND ($3::uuid IS NULL OR site_id = $3::uuid) \
                   AND ($4::text IS NULL OR status = $4::text) \
                   AND ($5::uuid IS NULL OR candidate_id > $5::uuid) \
                 ORDER BY candidate_id ASC \
                 LIMIT $6"
            ))
            .bind(uuid_text(ctx.tenant_id().uuid()))
            .bind(portfolio_id)
            .bind(site_id)
            .bind(status)
            .bind(after_candidate_id)
            .bind(limit_i64)
            .fetch_all(&mut *tx)
            .await
            .map_err(|err| map_schedule_store_error(err, "<listing>"))?;
            rows.iter().map(decode_schedule_record_row).collect()
        })
    }
}

/// Maps a sqlx failure onto the storage error. A uniqueness violation on
/// the insert path is the immutability conflict (the candidate identity is
/// the primary key; the colliding identity is the caller-supplied value —
/// the adapter never reads engine text, per the no-raw-storage-text rule);
/// everything else is the opaque backend class.
fn map_schedule_store_error(err: sqlx::Error, candidate_id: &str) -> ScheduleStoreError {
    if is_unique_violation(&err) {
        return ScheduleStoreError::Conflict {
            candidate_id: candidate_id.to_string(),
        };
    }
    let backend = map_backend(err);
    ScheduleStoreError::Backend {
        code: backend.code().to_string(),
        message: backend.message().to_string(),
    }
}

/// Decodes one `schedule_results` row into the storage row type.
fn decode_schedule_record_row(
    row: &sqlx::postgres::PgRow,
) -> Result<ScheduleRecordRow, ScheduleStoreError> {
    let try_text =
        |column: &'static str| -> Result<String, ScheduleStoreError> {
            row.try_get::<String, _>(column).map_err(|_| {
                ScheduleStoreError::Backend {
                    code: "E_STORE_DECODE".to_string(),
                    message: format!("stored value in column {column} could not be decoded"),
                }
            })
        };
    let try_opt_text =
        |column: &'static str| -> Result<Option<String>, ScheduleStoreError> {
            row.try_get::<Option<String>, _>(column).map_err(|_| {
                ScheduleStoreError::Backend {
                    code: "E_STORE_DECODE".to_string(),
                    message: format!("stored value in column {column} could not be decoded"),
                }
            })
        };
    let candidate_id = try_text("candidate_id")?;
    let tenant_text = try_text("tenant_id")?;
    let tenant_uuid = crate::uuid_map::uuid_from_text(&tenant_text).map_err(|_| {
        ScheduleStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored value in column tenant_id could not be decoded".to_string(),
        }
    })?;
    let portfolio_id = try_text("portfolio_id")?;
    let site_id = try_text("site_id")?;
    let status = try_text("status")?;
    let problem_class = try_text("problem_class")?;
    let solver_name = try_text("solver_name")?;
    let solver_version = try_text("solver_version")?;
    let provider_id = try_text("provider_id")?;
    let provider_version = try_text("provider_version")?;
    let objective_value = row
        .try_get::<Option<f64>, _>("objective_value")
        .map_err(|_| ScheduleStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored value in column objective_value could not be decoded".to_string(),
        })?;
    let objective_units = try_opt_text("objective_units")?;
    let input_snapshot_id = try_text("input_snapshot_id")?;
    let grid_model_version = try_text("grid_model_version")?;
    let grid_model_site_id = try_text("grid_model_site_id")?;
    let grid_model_site_reference = try_text("grid_model_site_reference")?;
    let schedule_proposed_key = try_text("schedule_proposed_key")?;
    let payload: Value = row
        .try_get::<Value, _>("record_payload")
        .map_err(|_| ScheduleStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored value in column record_payload could not be decoded".to_string(),
        })?;
    let record_payload_json = serde_json::to_string(&payload).map_err(|_| {
        ScheduleStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "the stored record payload could not be rendered".to_string(),
        }
    })?;
    // Re-validate through the row constructor: a row that does not satisfy
    // the storage-level completeness rules cannot be returned (the adapter
    // fails closed on a malformed row rather than smuggling it through).
    ScheduleRecordRow::new(
        flexcore_domain::ids::TenantId::new(flexcore_contracts::ids::FlexUuid::from_bytes(
            tenant_uuid.to_bytes(),
        )),
        portfolio_id,
        site_id,
        candidate_id,
        status,
        problem_class,
        solver_name,
        solver_version,
        provider_id,
        provider_version,
        objective_value,
        objective_units,
        input_snapshot_id,
        grid_model_version,
        grid_model_site_id,
        grid_model_site_reference,
        schedule_proposed_key,
        record_payload_json,
    )
    .map_err(|_| ScheduleStoreError::Backend {
        code: "E_STORE_DECODE".to_string(),
        message: "the stored schedule-result row failed the storage-level completeness rules"
            .to_string(),
    })
}

// --- P3.22 — ScheduleRevisionChainStore (migration 0014) ----------------------
//
// The revision chain over `schedule_revisions`: append-only revisions, the
// dedicated §14.2 SUPERSEDED transition, and the tenant-scoped audit
// queries. Same transaction pattern as the record store above: every
// statement runs inside the CALLER's unit of work, so the SUPERSEDED marking
// of revision n − 1 and the insert of revision n commit or roll back
// TOGETHER (the partial unique index `schedule_revisions_one_active` is the
// database backstop for the one-active invariant). The BEFORE UPDATE trigger
// `schedule_revisions_immutability` (migration 0014) rejects any content
// change and any state movement other than `active → superseded` — the
// database-level immutability behind the INSERT-only grant shape.

const SCHEDULE_REVISION_SELECT: &str = "SELECT schedule_id::text AS schedule_id, \
     revision, tenant_id::text AS tenant_id, portfolio_id::text AS portfolio_id, \
     site_id::text AS site_id, horizon_start_millis, horizon_end_millis, \
     roll_trigger, supersedes_revision, \
     supersedes_candidate_id::text AS supersedes_candidate_id, \
     superseded_by_revision, \
     superseded_by_candidate_id::text AS superseded_by_candidate_id, \
     candidate_id::text AS candidate_id, schedule_payload \
     FROM schedule_revisions";

/// Validates one canonical-UUID text field, mapping a malformed value to the
/// named storage error (the adapter fails closed before the database sees a
/// value it could only reject with raw engine text).
fn require_uuid_text(
    value: &str,
    field: &'static str,
) -> Result<(), ScheduleRevisionError> {
    uuid_from_text(value)
        .map(|_| ())
        .map_err(|_| ScheduleRevisionError::InvalidRevision { field })
}

/// Maps a sqlx failure onto the revision error: a uniqueness violation on the
/// insert path is the immutability conflict (the (schedule, revision) pair
/// is the primary key); everything else is the opaque backend class.
fn map_schedule_revision_error(
    err: sqlx::Error,
    schedule_id: &str,
    revision: u64,
) -> ScheduleRevisionError {
    if is_unique_violation(&err) {
        return ScheduleRevisionError::Conflict {
            schedule_id: schedule_id.to_string(),
            revision,
        };
    }
    let backend = map_backend(err);
    ScheduleRevisionError::Backend {
        code: backend.code().to_string(),
        message: backend.message().to_string(),
    }
}

impl ScheduleRevisionChainStore for PgUnitOfWork {
    fn insert_schedule_revision(
        &self,
        ctx: &TenantContext,
        row: &ScheduleRevisionRow,
    ) -> Result<ScheduleRevisionReceipt, ScheduleRevisionError> {
        require_uuid_text(&row.schedule_id, "schedule_id")?;
        require_uuid_text(&row.portfolio_id, "portfolio_id")?;
        require_uuid_text(&row.site_id, "site_id")?;
        require_uuid_text(&row.candidate_id, "candidate_id")?;
        if let Some(superseded) = row.supersedes_candidate_id.as_deref() {
            require_uuid_text(superseded, "supersedes_candidate_id")?;
        }
        let revision_i64 = i64::try_from(row.revision).map_err(|_| {
            ScheduleRevisionError::InvalidRevision { field: "revision" }
        })?;
        // The payload must be valid JSON before it reaches the database (the
        // storage crate treats it as opaque text; this adapter owns the
        // jsonb binding and therefore the validation).
        let payload: Value = serde_json::from_str(&row.schedule_payload_json)
            .map_err(|_| ScheduleRevisionError::InvalidRevision {
                field: "schedule_payload_json",
            })?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| ScheduleRevisionError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            // Chain verification inside THIS transaction (D-P3.22-1): a
            // revision > 1 must find its exact predecessor — present,
            // already SUPERSEDED, and pointing its forward link at THIS
            // revision. Together with the partial unique index
            // `schedule_revisions_one_active` this makes a gapped or
            // double-active chain unrepresentable.
            if row.revision > 1 {
                let prior = sqlx::query(
                    "SELECT state, superseded_by_revision FROM schedule_revisions \
                     WHERE schedule_id = $1::uuid AND tenant_id = $2::uuid AND revision = $3",
                )
                .bind(&row.schedule_id)
                .bind(uuid_text(ctx.tenant_id().uuid()))
                .bind(revision_i64 - 1)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|err| {
                    map_schedule_revision_error(err, &row.schedule_id, row.revision)
                })?;
                let (prior_state, prior_link): (String, Option<i64>) = match prior {
                    Some(row) => {
                        let state: String = row.try_get("state").map_err(|_| {
                            ScheduleRevisionError::Backend {
                                code: "E_STORE_DECODE".to_string(),
                                message:
                                    "stored value in column state could not be decoded"
                                        .to_string(),
                            }
                        })?;
                        let link: Option<i64> =
                            row.try_get("superseded_by_revision").map_err(|_| {
                                ScheduleRevisionError::Backend {
                                    code: "E_STORE_DECODE".to_string(),
                                    message:
                                        "stored value in column superseded_by_revision could not be decoded"
                                            .to_string(),
                                }
                            })?;
                        (state, link)
                    }
                    None => {
                        return Err(ScheduleRevisionError::ChainBroken {
                            schedule_id: row.schedule_id.clone(),
                            revision: row.revision,
                            detail: format!(
                                "the predecessor revision {} does not exist",
                                row.revision - 1
                            ),
                        });
                    }
                };
                if prior_state != flexcore_storage::schedule_revision::SCHEDULE_REVISION_STATE_SUPERSEDED
                {
                    return Err(ScheduleRevisionError::ChainBroken {
                        schedule_id: row.schedule_id.clone(),
                        revision: row.revision,
                        detail: format!(
                            "the predecessor revision {} is still active — mark it \
                             SUPERSEDED in the same transaction before inserting this \
                             revision",
                            row.revision - 1
                        ),
                    });
                }
                if prior_link != Some(revision_i64) {
                    return Err(ScheduleRevisionError::ChainBroken {
                        schedule_id: row.schedule_id.clone(),
                        revision: row.revision,
                        detail: format!(
                            "the predecessor revision {} is superseded by a different \
                             revision than this one",
                            row.revision - 1
                        ),
                    });
                }
            }
            let result = sqlx::query(
                "INSERT INTO schedule_revisions \
                     (schedule_id, revision, tenant_id, portfolio_id, site_id, \
                      horizon_start_millis, horizon_end_millis, roll_trigger, state, \
                      supersedes_revision, supersedes_candidate_id, candidate_id, \
                      schedule_payload) \
                 VALUES ($1::uuid, $2, $3::uuid, $4::uuid, $5::uuid, $6, $7, $8, 'active', \
                         $9, $10::uuid, $11::uuid, $12)",
            )
            .bind(&row.schedule_id)
            .bind(revision_i64)
            .bind(uuid_text(ctx.tenant_id().uuid()))
            .bind(&row.portfolio_id)
            .bind(&row.site_id)
            .bind(row.horizon_start_millis)
            .bind(row.horizon_end_millis)
            .bind(&row.roll_trigger)
            .bind(
                row.supersedes_revision
                    .and_then(|r| i64::try_from(r).ok()),
            )
            .bind(row.supersedes_candidate_id.as_deref())
            .bind(&row.candidate_id)
            .bind(payload)
            .execute(&mut *tx)
            .await
            .map_err(|err| {
                map_schedule_revision_error(err, &row.schedule_id, row.revision)
            })?;
            if result.rows_affected() != 1 {
                return Err(ScheduleRevisionError::Backend {
                    code: "E_STORE_REJECTED".to_string(),
                    message: "the schedule-revision insert affected no rows".to_string(),
                });
            }
            Ok(ScheduleRevisionReceipt::new(
                row.schedule_id.clone(),
                row.revision,
                ctx.tenant_id(),
            ))
        })
    }

    fn mark_revision_superseded(
        &self,
        ctx: &TenantContext,
        schedule_id: &str,
        revision: u64,
        superseded_by: &ScheduleSupersedeTarget,
    ) -> Result<ScheduleRevisionReceipt, ScheduleRevisionError> {
        // The forward link must point exactly one step ahead: the chain is
        // gapless in BOTH directions (the new revision's row CHECK requires
        // supersedes_revision = revision - 1, so the marking must match).
        if superseded_by.revision != revision.checked_add(1).ok_or(
            ScheduleRevisionError::InvalidRevision { field: "superseded_by_revision" },
        )? {
            return Err(ScheduleRevisionError::InvalidRevision {
                field: "superseded_by_revision",
            });
        }
        require_uuid_text(schedule_id, "schedule_id")?;
        require_uuid_text(&superseded_by.candidate_id, "candidate_id")?;
        let revision_i64 = i64::try_from(revision)
            .map_err(|_| ScheduleRevisionError::InvalidRevision { field: "revision" })?;
        let target_i64 = i64::try_from(superseded_by.revision).map_err(|_| {
            ScheduleRevisionError::InvalidRevision { field: "superseded_by_revision" }
        })?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| ScheduleRevisionError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            // The dedicated transition: exactly the three state/link columns,
            // only from the `active` state (the column-level grant in
            // migration 0014 covers these columns and nothing else; the
            // immutability trigger rejects any other movement).
            let marked = sqlx::query(
                "UPDATE schedule_revisions \
                 SET state = 'superseded', \
                     superseded_by_revision = $4, \
                     superseded_by_candidate_id = $5::uuid \
                 WHERE schedule_id = $1::uuid AND tenant_id = $2::uuid \
                   AND revision = $3 AND state = 'active' \
                 RETURNING candidate_id::text AS candidate_id",
            )
            .bind(schedule_id)
            .bind(uuid_text(ctx.tenant_id().uuid()))
            .bind(revision_i64)
            .bind(target_i64)
            .bind(&superseded_by.candidate_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|err| map_schedule_revision_error(err, schedule_id, revision))?;
            match marked {
                Some(_) => Ok(ScheduleRevisionReceipt::new(
                    schedule_id.to_string(),
                    revision,
                    ctx.tenant_id(),
                )),
                None => {
                    // Distinguish already-superseded from not-found without
                    // leaking existence across tenants (the explicit tenant
                    // filter is in the query below as well; RLS is the outer
                    // gate).
                    let existing = sqlx::query(
                        "SELECT state FROM schedule_revisions \
                         WHERE schedule_id = $1::uuid AND tenant_id = $2::uuid AND revision = $3",
                    )
                    .bind(schedule_id)
                    .bind(uuid_text(ctx.tenant_id().uuid()))
                    .bind(revision_i64)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(|err| {
                        map_schedule_revision_error(err, schedule_id, revision)
                    })?;
                    match existing {
                        None => Err(ScheduleRevisionError::NotFound {
                            schedule_id: schedule_id.to_string(),
                            revision,
                        }),
                        Some(_) => Err(ScheduleRevisionError::AlreadySuperseded {
                            schedule_id: schedule_id.to_string(),
                            revision,
                        }),
                    }
                }
            }
        })
    }

    fn find_revision(
        &self,
        ctx: &TenantContext,
        schedule_id: &str,
        revision: u64,
    ) -> Result<Option<ScheduleRevisionRecord>, ScheduleRevisionError> {
        let revision_i64 = i64::try_from(revision)
            .map_err(|_| ScheduleRevisionError::InvalidRevision { field: "revision" })?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| ScheduleRevisionError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let row = sqlx::query(&format!(
                "{SCHEDULE_REVISION_SELECT} WHERE schedule_id = $1::uuid \
                 AND tenant_id = $2::uuid AND revision = $3"
            ))
            .bind(schedule_id)
            .bind(uuid_text(ctx.tenant_id().uuid()))
            .bind(revision_i64)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|err| map_schedule_revision_error(err, schedule_id, revision))?;
            match row {
                None => Ok(None),
                Some(row) => decode_schedule_revision_row(&row).map(Some),
            }
        })
    }

    fn current_revision(
        &self,
        ctx: &TenantContext,
        schedule_id: &str,
    ) -> Result<Option<ScheduleRevisionRecord>, ScheduleRevisionError> {
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| ScheduleRevisionError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            // The partial unique index admits at most one active revision
            // per (tenant, schedule); `FETCH 1` makes that explicit.
            let row = sqlx::query(&format!(
                "{SCHEDULE_REVISION_SELECT} WHERE schedule_id = $1::uuid \
                 AND tenant_id = $2::uuid AND state = 'active' FETCH FIRST 1 ROW ONLY"
            ))
            .bind(schedule_id)
            .bind(uuid_text(ctx.tenant_id().uuid()))
            .fetch_optional(&mut *tx)
            .await
            .map_err(|err| map_schedule_revision_error(err, schedule_id, 0))?;
            match row {
                None => Ok(None),
                Some(row) => decode_schedule_revision_row(&row).map(Some),
            }
        })
    }

    fn revision_chain(
        &self,
        ctx: &TenantContext,
        schedule_id: &str,
    ) -> Result<Vec<ScheduleRevisionRecord>, ScheduleRevisionError> {
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| ScheduleRevisionError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let rows = sqlx::query(&format!(
                "{SCHEDULE_REVISION_SELECT} WHERE schedule_id = $1::uuid \
                 AND tenant_id = $2::uuid ORDER BY revision ASC"
            ))
            .bind(schedule_id)
            .bind(uuid_text(ctx.tenant_id().uuid()))
            .fetch_all(&mut *tx)
            .await
            .map_err(|err| map_schedule_revision_error(err, schedule_id, 0))?;
            rows.iter()
                .map(decode_schedule_revision_row)
                .collect::<Result<Vec<_>, _>>()
        })
    }

    fn is_revision_superseded(
        &self,
        ctx: &TenantContext,
        schedule_id: &str,
        revision: u64,
    ) -> Result<Option<bool>, ScheduleRevisionError> {
        let revision_i64 = i64::try_from(revision)
            .map_err(|_| ScheduleRevisionError::InvalidRevision { field: "revision" })?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| ScheduleRevisionError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let row = sqlx::query(
                "SELECT state FROM schedule_revisions \
                 WHERE schedule_id = $1::uuid AND tenant_id = $2::uuid AND revision = $3",
            )
            .bind(schedule_id)
            .bind(uuid_text(ctx.tenant_id().uuid()))
            .bind(revision_i64)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|err| map_schedule_revision_error(err, schedule_id, revision))?;
            match row {
                None => Ok(None),
                Some(row) => {
                    let state: String = row.try_get("state").map_err(|_| {
                        ScheduleRevisionError::Backend {
                            code: "E_STORE_DECODE".to_string(),
                            message:
                                "stored value in column state could not be decoded"
                                    .to_string(),
                        }
                    })?;
                    Ok(Some(
                        state
                            == flexcore_storage::schedule_revision::SCHEDULE_REVISION_STATE_SUPERSEDED,
                    ))
                }
            }
        })
    }

    /// Lists revisions tenant-scoped (P3.28, added additively): keyset
    /// over `(schedule_id, revision)` ascending — the row primary key —
    /// bounded by `limit`, filtered by the optional portfolio filter and
    /// the optional cursor position. No offset pagination anywhere.
    fn list_revisions(
        &self,
        ctx: &TenantContext,
        portfolio_id: Option<&str>,
        after_schedule_id: Option<&str>,
        after_revision: Option<u64>,
        limit: usize,
    ) -> Result<Vec<ScheduleRevisionRecord>, ScheduleRevisionError> {
        for (field, value) in [("portfolio_id", portfolio_id), ("after_schedule_id", after_schedule_id)] {
            if let Some(value) = value {
                require_uuid_text(value, field)?;
            }
        }
        let after_revision_i64 = match after_revision {
            None => None,
            Some(revision) => Some(i64::try_from(revision).map_err(|_| {
                ScheduleRevisionError::InvalidRevision { field: "after_revision" }
            })?),
        };
        let limit_i64 = i64::try_from(limit).map_err(|_| ScheduleRevisionError::Backend {
            code: "E_STORE_LIMIT".to_string(),
            message: "the page limit is out of range".to_string(),
        })?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| ScheduleRevisionError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            // One parameterized shape: the filters are always bound (None =
            // no constraint), so the statement text is constant. The keyset
            // comparison is the row-value pair comparison over the primary
            // key (D-P3.22-1).
            let rows = sqlx::query(&format!(
                "{SCHEDULE_REVISION_SELECT} \
                 WHERE tenant_id = $1::uuid \
                   AND ($2::uuid IS NULL OR portfolio_id = $2::uuid) \
                   AND ($3::uuid IS NULL \
                        OR (schedule_id, revision) > ($3::uuid, $4)) \
                 ORDER BY schedule_id ASC, revision ASC \
                 LIMIT $5"
            ))
            .bind(uuid_text(ctx.tenant_id().uuid()))
            .bind(portfolio_id)
            .bind(after_schedule_id)
            .bind(after_revision_i64)
            .bind(limit_i64)
            .fetch_all(&mut *tx)
            .await
            .map_err(|err| map_schedule_revision_error(err, "<listing>", 0))?;
            rows.iter().map(decode_schedule_revision_row).collect()
        })
    }
}

/// Decodes one `schedule_revisions` row into the storage record type,
/// re-validating through the row constructors (fail closed on a malformed
/// row — the same discipline as the record store above).
fn decode_schedule_revision_row(
    row: &sqlx::postgres::PgRow,
) -> Result<ScheduleRevisionRecord, ScheduleRevisionError> {
    let try_text = |column: &'static str| -> Result<String, ScheduleRevisionError> {
        row.try_get::<String, _>(column).map_err(|_| {
            ScheduleRevisionError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    let try_opt_text =
        |column: &'static str| -> Result<Option<String>, ScheduleRevisionError> {
            row.try_get::<Option<String>, _>(column).map_err(|_| {
                ScheduleRevisionError::Backend {
                    code: "E_STORE_DECODE".to_string(),
                    message: format!("stored value in column {column} could not be decoded"),
                }
            })
        };
    let try_i64 = |column: &'static str| -> Result<i64, ScheduleRevisionError> {
        row.try_get::<i64, _>(column).map_err(|_| {
            ScheduleRevisionError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    let try_opt_i64 = |column: &'static str| -> Result<Option<i64>, ScheduleRevisionError> {
        row.try_get::<Option<i64>, _>(column).map_err(|_| {
            ScheduleRevisionError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    let to_u64 = |value: i64, column: &'static str| -> Result<u64, ScheduleRevisionError> {
        u64::try_from(value).map_err(|_| ScheduleRevisionError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: format!("stored value in column {column} is out of range"),
        })
    };
    let schedule_id = try_text("schedule_id")?;
    let revision = to_u64(try_i64("revision")?, "revision")?;
    let tenant_text = try_text("tenant_id")?;
    let tenant_uuid = uuid_from_text(&tenant_text).map_err(|_| {
        ScheduleRevisionError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored value in column tenant_id could not be decoded".to_string(),
        }
    })?;
    let portfolio_id = try_text("portfolio_id")?;
    let site_id = try_text("site_id")?;
    let horizon_start_millis = try_i64("horizon_start_millis")?;
    let horizon_end_millis = try_i64("horizon_end_millis")?;
    let roll_trigger = try_text("roll_trigger")?;
    let supersedes_revision = match try_opt_i64("supersedes_revision")? {
        Some(value) => Some(to_u64(value, "supersedes_revision")?),
        None => None,
    };
    let supersedes_candidate_id = try_opt_text("supersedes_candidate_id")?;
    let superseded_by_revision = match try_opt_i64("superseded_by_revision")? {
        Some(value) => Some(to_u64(value, "superseded_by_revision")?),
        None => None,
    };
    let superseded_by_candidate_id = try_opt_text("superseded_by_candidate_id")?;
    let candidate_id = try_text("candidate_id")?;
    let payload: Value = row
        .try_get::<Value, _>("schedule_payload")
        .map_err(|_| ScheduleRevisionError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored value in column schedule_payload could not be decoded"
                .to_string(),
        })?;
    let schedule_payload_json = serde_json::to_string(&payload).map_err(|_| {
        ScheduleRevisionError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "the stored schedule payload could not be rendered".to_string(),
        }
    })?;
    let row = ScheduleRevisionRow::new(
        flexcore_domain::ids::TenantId::new(flexcore_contracts::ids::FlexUuid::from_bytes(
            tenant_uuid.to_bytes(),
        )),
        portfolio_id,
        site_id,
        schedule_id,
        revision,
        horizon_start_millis,
        horizon_end_millis,
        roll_trigger,
        supersedes_revision,
        supersedes_candidate_id,
        candidate_id,
        schedule_payload_json,
    )?;
    ScheduleRevisionRecord::new(
        row,
        try_text("state")?,
        superseded_by_revision,
        superseded_by_candidate_id,
    )
    .map_err(|_| ScheduleRevisionError::Backend {
        code: "E_STORE_DECODE".to_string(),
        message: "the stored schedule-revision row failed the storage-level consistency rules"
            .to_string(),
    })
}

// --- P3.23 — ScheduleComparisonStore (migration 0014) --------------------------
//
// Immutable comparison records over `schedule_comparisons`: insert + a
// tenant-scoped find, no update, no delete (INSERT/SELECT grants only). The
// same unit-of-work transaction pattern and the same no-raw-storage-text
// error mapping as the record store above.

const SCHEDULE_COMPARISON_SELECT: &str = "SELECT comparison_id::text AS comparison_id, \
     tenant_id::text AS tenant_id, portfolio_id::text AS portfolio_id, \
     site_id::text AS site_id, shared_input_snapshot_id, shared_envelope_version, \
     shared_forecast_version, shared_forecast_provenance, shared_objective_name, \
     candidate_count, comparison_payload \
     FROM schedule_comparisons";

fn map_comparison_store_error(
    err: sqlx::Error,
    comparison_id: &str,
) -> ComparisonStoreError {
    if is_unique_violation(&err) {
        return ComparisonStoreError::Conflict {
            comparison_id: comparison_id.to_string(),
        };
    }
    let backend = map_backend(err);
    ComparisonStoreError::Backend {
        code: backend.code().to_string(),
        message: backend.message().to_string(),
    }
}

impl ScheduleComparisonStore for PgUnitOfWork {
    fn insert_schedule_comparison(
        &self,
        ctx: &TenantContext,
        row: &ScheduleComparisonRow,
    ) -> Result<ScheduleComparisonReceipt, ComparisonStoreError> {
        uuid_from_text(&row.comparison_id).map_err(|_| ComparisonStoreError::InvalidRecord {
            field: "comparison_id",
        })?;
        // The payload must be valid JSON before it reaches the database.
        let payload: Value = serde_json::from_str(&row.comparison_payload_json)
            .map_err(|_| ComparisonStoreError::InvalidRecord {
                field: "comparison_payload_json",
            })?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| ComparisonStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let result = sqlx::query(
                "INSERT INTO schedule_comparisons \
                     (comparison_id, tenant_id, portfolio_id, site_id, \
                      shared_input_snapshot_id, shared_envelope_version, \
                      shared_forecast_version, shared_forecast_provenance, \
                      shared_objective_name, candidate_count, comparison_payload) \
                 VALUES ($1::uuid, $2::uuid, $3::uuid, $4::uuid, $5, $6, $7, $8, $9, $10, $11)",
            )
            .bind(&row.comparison_id)
            .bind(uuid_text(ctx.tenant_id().uuid()))
            .bind(&row.portfolio_id)
            .bind(&row.site_id)
            .bind(&row.shared_input_snapshot_id)
            .bind(&row.shared_envelope_version)
            .bind(&row.shared_forecast_version)
            .bind(&row.shared_forecast_provenance)
            .bind(&row.shared_objective_name)
            .bind(i32::try_from(row.candidate_count).map_err(|_| {
                ComparisonStoreError::InvalidRecord { field: "candidate_count" }
            })?)
            .bind(payload)
            .execute(&mut *tx)
            .await
            .map_err(|err| map_comparison_store_error(err, &row.comparison_id))?;
            if result.rows_affected() != 1 {
                return Err(ComparisonStoreError::Backend {
                    code: "E_STORE_REJECTED".to_string(),
                    message: "the comparison insert affected no rows".to_string(),
                });
            }
            Ok(ScheduleComparisonReceipt::new(
                row.comparison_id.clone(),
                ctx.tenant_id(),
            ))
        })
    }

    fn find_schedule_comparison(
        &self,
        ctx: &TenantContext,
        comparison_id: &str,
    ) -> Result<Option<ScheduleComparisonRow>, ComparisonStoreError> {
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| ComparisonStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let row = sqlx::query(&format!(
                "{SCHEDULE_COMPARISON_SELECT} WHERE comparison_id = $1::uuid \
                 AND tenant_id = $2::uuid"
            ))
            .bind(comparison_id)
            .bind(uuid_text(ctx.tenant_id().uuid()))
            .fetch_optional(&mut *tx)
            .await
            .map_err(|err| map_comparison_store_error(err, comparison_id))?;
            match row {
                None => Ok(None),
                Some(row) => decode_schedule_comparison_row(&row).map(Some),
            }
        })
    }
}

/// Decodes one `schedule_comparisons` row into the storage row type,
/// re-validating through the row constructor (fail closed).
fn decode_schedule_comparison_row(
    row: &sqlx::postgres::PgRow,
) -> Result<ScheduleComparisonRow, ComparisonStoreError> {
    let try_text = |column: &'static str| -> Result<String, ComparisonStoreError> {
        row.try_get::<String, _>(column).map_err(|_| {
            ComparisonStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    let comparison_id = try_text("comparison_id")?;
    let tenant_text = try_text("tenant_id")?;
    let tenant_uuid = uuid_from_text(&tenant_text).map_err(|_| {
        ComparisonStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored value in column tenant_id could not be decoded".to_string(),
        }
    })?;
    let portfolio_id = try_text("portfolio_id")?;
    let site_id = try_text("site_id")?;
    let shared_input_snapshot_id = try_text("shared_input_snapshot_id")?;
    let shared_envelope_version = try_text("shared_envelope_version")?;
    let shared_forecast_version = try_text("shared_forecast_version")?;
    let shared_forecast_provenance = try_text("shared_forecast_provenance")?;
    let shared_objective_name = try_text("shared_objective_name")?;
    let candidate_count_i32: i32 = row
        .try_get::<i32, _>("candidate_count")
        .map_err(|_| ComparisonStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored value in column candidate_count could not be decoded"
                .to_string(),
        })?;
    let candidate_count =
        u32::try_from(candidate_count_i32).map_err(|_| ComparisonStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored value in column candidate_count is out of range".to_string(),
        })?;
    let payload: Value = row
        .try_get::<Value, _>("comparison_payload")
        .map_err(|_| ComparisonStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored value in column comparison_payload could not be decoded"
                .to_string(),
        })?;
    let comparison_payload_json = serde_json::to_string(&payload).map_err(|_| {
        ComparisonStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "the stored comparison payload could not be rendered".to_string(),
        }
    })?;
    ScheduleComparisonRow::new(
        flexcore_domain::ids::TenantId::new(flexcore_contracts::ids::FlexUuid::from_bytes(
            tenant_uuid.to_bytes(),
        )),
        comparison_id,
        portfolio_id,
        site_id,
        shared_input_snapshot_id,
        shared_envelope_version,
        shared_forecast_version,
        shared_forecast_provenance,
        shared_objective_name,
        candidate_count,
        comparison_payload_json,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // The composition root holds ports as trait objects; object safety is
    // part of the contract (the same evidence pattern as the adapter's
    // `every_implemented_port_is_object_safe`). Compile-level evidence that
    // PgUnitOfWork carries the schedule-record store — and, additively (P3.22
    // /P3.23), the revision-chain and comparison stores.
    #[test]
    fn the_schedule_record_store_is_object_safe_for_the_unit_of_work() {
        fn takes_dyn_store(_: &dyn ScheduleRecordStore) {}
        let _ = takes_dyn_store as fn(&dyn ScheduleRecordStore);
    }

    #[test]
    fn the_revision_chain_and_comparison_stores_are_object_safe() {
        fn takes_revision_store(_: &dyn ScheduleRevisionChainStore) {}
        let _ = takes_revision_store as fn(&dyn ScheduleRevisionChainStore);
        fn takes_comparison_store(_: &dyn ScheduleComparisonStore) {}
        let _ = takes_comparison_store as fn(&dyn ScheduleComparisonStore);
    }
}
