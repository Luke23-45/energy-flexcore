// P4.10 — PostgreSQL implementation of the command-record persistence
//         contract (report §14.1–§14.2, §16.1, §16.2; ledger P4.10 step 6;
//         migration 0016; decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/10_command_state_machine.md`).
//
// Implements `flexcore_storage::command_store::CommandRecordStore` for
// [`PgUnitOfWork`], the same unit-of-work pattern as the P3.21/P3.22
// schedule stores: every statement runs inside the caller's one native
// transaction, so a command-row state transition and its journal row commit
// or roll back TOGETHER (the §15.5 atomicity discipline; the journal is
// written in the SAME transaction as the state change, ledger P4.30 risk
// note — audit from the transition, never after the fact).
//
// Write model (D-P4.10-2): the command row is INSERTed exactly once, in the
// state `PROPOSED` (the migration's BEFORE INSERT trigger rejects any other
// initial state); the ONLY permitted update of a command row is the
// compare-and-swap state transition (`UPDATE ... WHERE state = <from>` —
// the one-active-transition invariant's first half; the migration's BEFORE
// UPDATE trigger is the second: one frozen-table state change per UPDATE,
// no content change). The frozen 27-edge table in that trigger is database
// DEFENCE IN DEPTH only — the single source of truth is the domain's P0.11
// table (`flexcore_domain::command_state`), which this module validates
// every transition through at the journal-record level; a divergence
// between the two encodings is a BUG against the oracle, never an
// extension.
//
// Supersede discipline (D-P4.10-3): the backward link
// `superseded_by_command_id` is set EXACTLY by the SUPERSEDED transition
// (the migration's trigger enforces the link/state pairing). The link value
// is derived from the journal record's evidence reference — the domain
// engine (`CommandLifecycle::record_superseded_by`) records the replacing
// command as `command:<uuid>` — and is validated to parse as a canonical
// UUID before the statement runs; the database FK additionally requires the
// replacing command to exist in the same tenant scope.
//
// Tenant scoping (P1.12 §5.1 conventions): the unit of work's
// transaction-scoped `flexcore.tenant_id` GUC feeds the `tenant_isolation`
// RLS policy on both tables (ENABLE + FORCE); on top of RLS every statement
// filters by the context's tenant explicitly, and a foreign-tenant command
// reads as `None` — existence never leaks (P0.16 rule 5).
//
// Immutability: the runtime grants are INSERT + SELECT on both tables plus
// a COLUMN-LEVEL UPDATE grant on exactly the two transition columns
// (`state`, `superseded_by_command_id` — the recorded exception anticipated
// by migration 0013, same shape as migration 0014). Journal rows have NO
// update and NO delete.
//
// P4.12 pre-wiring (D-P4.10-5; NO feasibility gate here): a transition on a
// command whose source schedule revision is SUPERSEDED is rejected at the
// gate-check level via `ensure_schedule_revision_usable`, which calls the
// P3.22 `is_revision_superseded` surface
// (`flexcore_postgres_adapter::schedule_store`). The §12.5 network
// feasibility pipeline itself is P4.12's.
//
// VERIFY-BUILD (field-visibility precedent): this module accesses the
// private `tx`/`runtime` fields of `PgUnitOfWork` from a sibling module —
// exactly the pattern the existing `telemetry.rs`, `corrections.rs`, and
// `schedule_store.rs` modules use for their `impl PgUnitOfWork` blocks. If
// rustc rejects that access (E0616), the single-line fix is `pub(crate)` on
// the two field declarations in `unit_of_work.rs`, which repairs every
// module at once; that file was outside this subtask's ownership.

use crate::error::{is_unique_violation, map_backend};
use crate::unit_of_work::{tx_ref, PgUnitOfWork};
use crate::uuid_map::{uuid_from_text, uuid_text};
use flexcore_domain::command_state::{CommandState, try_transition};
use flexcore_domain::tenant_context::TenantContext;
use flexcore_storage::command_store::{
    CommandRecord, CommandRecordRow, CommandReceipt, CommandStoreError,
    CommandTransitionRecord, CommandTransitionReceipt, CommandRecordStore,
};
use flexcore_storage::schedule_revision::{ScheduleRevisionChainStore, ScheduleRevisionError};
use serde_json::Value;

/// The column projection of one command record (uuid columns selected back
/// as `::text` — the recorded FlexUuid mapping).
const COMMAND_SELECT: &str = "SELECT command_id::text AS command_id, \
     tenant_id::text AS tenant_id, portfolio_id::text AS portfolio_id, \
     site_id::text AS site_id, asset_id::text AS asset_id, \
     schedule_id::text AS schedule_id, schedule_revision, valid_until_millis, \
     supersedes_command_id::text AS supersedes_command_id, \
     superseded_by_command_id::text AS superseded_by_command_id, state, \
     command_payload FROM dispatch_commands";

/// The column projection of one journal row.
const COMMAND_JOURNAL_SELECT: &str = "SELECT command_id::text AS command_id, \
     tenant_id::text AS tenant_id, seq, from_state, to_state, actor, \
     at_unix_millis, gate, reason, evidence_reference \
     FROM dispatch_command_transitions";

/// Validates one canonical-UUID text field, mapping a malformed value to
/// the named storage error (the adapter fails closed before the database
/// sees a value it could only reject with raw engine text).
fn require_uuid_text(value: &str, field: &'static str) -> Result<(), CommandStoreError> {
    uuid_from_text(value)
        .map(|_| ())
        .map_err(|_| CommandStoreError::InvalidRecord { field })
}

/// Maps a sqlx failure onto the storage error. A uniqueness violation on
/// the insert path is the immutability conflict (the (command_id, tenant_id)
/// pair is the primary key; command rows are immutable and a re-proposal is
/// a NEW command); everything else is the opaque backend class
/// (the no-raw-storage-text rule).
fn map_command_store_error(err: sqlx::Error, command_id: &str) -> CommandStoreError {
    if is_unique_violation(&err) {
        return CommandStoreError::Conflict {
            command_id: command_id.to_string(),
        };
    }
    let backend = map_backend(err);
    CommandStoreError::Backend {
        code: backend.code().to_string(),
        message: backend.message().to_string(),
    }
}

/// Maps the P3.22 revision-check error onto the command-store error (the
/// check is the P4.12 pre-wiring surface over `is_revision_superseded`).
fn map_revision_check_error(err: ScheduleRevisionError) -> CommandStoreError {
    match err {
        ScheduleRevisionError::InvalidRevision { field } => {
            CommandStoreError::InvalidRecord { field }
        }
        ScheduleRevisionError::Backend { code, message } => {
            CommandStoreError::Backend { code, message }
        }
        // The remaining variants cannot arise from a pure state read; they
        // are still mapped to the named, human-safe classes (never raw
        // engine text).
        ScheduleRevisionError::NotFound { .. } => CommandStoreError::Backend {
            code: "E_STORE_REJECTED".to_string(),
            message: "the schedule revision lookup did not return a state".to_string(),
        },
        ScheduleRevisionError::Conflict { .. }
        | ScheduleRevisionError::ChainBroken { .. }
        | ScheduleRevisionError::AlreadySuperseded { .. } => CommandStoreError::Backend {
            code: "E_STORE_REJECTED".to_string(),
            message: "the schedule revision lookup was refused by the revision store".to_string(),
        },
    }
}

/// Parses a stored state name back into the closed `CommandState`
/// vocabulary (a stored row that is outside the 14-state vocabulary cannot
/// be decoded — fail closed).
fn state_from_name(name: &str) -> Option<CommandState> {
    CommandState::ALL
        .iter()
        .copied()
        .find(|state| state.name() == name)
}

/// Derives the `superseded_by_command_id` link for a SUPERSEDED transition
/// from the journal record's evidence reference (D-P4.10-3: the domain
/// engine records the replacing command as `command:<uuid>`). Any other
/// transition carries NO link (the migration trigger enforces the pairing).
fn superseded_by_link(
    to_state: &str,
    evidence_reference: Option<&str>,
) -> Result<Option<String>, CommandStoreError> {
    if to_state != flexcore_storage::command_store::COMMAND_STATE_SUPERSEDED {
        return Ok(None);
    }
    let reference = evidence_reference.ok_or(CommandStoreError::InvalidRecord {
        field: "evidence_reference",
    })?;
    let replacing = reference
        .strip_prefix("command:")
        .ok_or(CommandStoreError::InvalidRecord {
            field: "evidence_reference",
        })?;
    require_uuid_text(replacing, "superseded_by_command_id")?;
    Ok(Some(replacing.to_string()))
}

/// Decodes one `dispatch_commands` row into the storage record type,
/// re-validating through the row constructors (fail closed on a malformed
/// row — the same discipline as the schedule stores).
fn decode_command_record_row(
    row: &sqlx::postgres::PgRow,
) -> Result<CommandRecord, CommandStoreError> {
    let try_text =
        |column: &'static str| -> Result<String, CommandStoreError> {
            row.try_get::<String, _>(column).map_err(|_| {
                CommandStoreError::Backend {
                    code: "E_STORE_DECODE".to_string(),
                    message: format!("stored value in column {column} could not be decoded"),
                }
            })
        };
    let try_opt_text =
        |column: &'static str| -> Result<Option<String>, CommandStoreError> {
            row.try_get::<Option<String>, _>(column).map_err(|_| {
                CommandStoreError::Backend {
                    code: "E_STORE_DECODE".to_string(),
                    message: format!("stored value in column {column} could not be decoded"),
                }
            })
        };
    let command_id = try_text("command_id")?;
    let tenant_text = try_text("tenant_id")?;
    let tenant_uuid = uuid_from_text(&tenant_text).map_err(|_| CommandStoreError::Backend {
        code: "E_STORE_DECODE".to_string(),
        message: "stored value in column tenant_id could not be decoded".to_string(),
    })?;
    let portfolio_id = try_text("portfolio_id")?;
    let site_id = try_text("site_id")?;
    let asset_id = try_text("asset_id")?;
    let schedule_id = try_text("schedule_id")?;
    let schedule_revision = u64::try_from(
        row.try_get::<i64, _>("schedule_revision").map_err(|_| {
            CommandStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: "stored value in column schedule_revision could not be decoded"
                    .to_string(),
            }
        })?,
    )
    .map_err(|_| CommandStoreError::Backend {
        code: "E_STORE_DECODE".to_string(),
        message: "stored value in column schedule_revision is out of range".to_string(),
    })?;
    let valid_until_millis =
        row.try_get::<i64, _>("valid_until_millis").map_err(|_| {
            CommandStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: "stored value in column valid_until_millis could not be decoded"
                    .to_string(),
            }
        })?;
    let supersedes_command_id = try_opt_text("supersedes_command_id")?;
    let superseded_by_command_id = try_opt_text("superseded_by_command_id")?;
    let state = try_text("state")?;
    let payload: Value = row
        .try_get::<Value, _>("command_payload")
        .map_err(|_| CommandStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored value in column command_payload could not be decoded".to_string(),
        })?;
    let command_payload_json = serde_json::to_string(&payload).map_err(|_| {
        CommandStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "the stored command payload could not be rendered".to_string(),
        }
    })?;
    let row = CommandRecordRow::new(
        flexcore_domain::ids::TenantId::new(flexcore_contracts::ids::FlexUuid::from_bytes(
            tenant_uuid.to_bytes(),
        )),
        command_id,
        portfolio_id,
        site_id,
        asset_id,
        schedule_id,
        schedule_revision,
        valid_until_millis,
        supersedes_command_id,
        command_payload_json,
    )
    .map_err(|_| CommandStoreError::Backend {
        code: "E_STORE_DECODE".to_string(),
        message: "the stored command row failed the storage-level completeness rules".to_string(),
    })?;
    CommandRecord::new(row, state, superseded_by_command_id).map_err(|_| {
        CommandStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "the stored command row failed the storage-level consistency rules"
                .to_string(),
        }
    })
}

/// Decodes one journal row into the storage journal record, re-validating
/// the closed vocabulary and the frozen edge (the domain table is the
/// oracle; a stored journal row that is not a frozen edge cannot be
/// returned — fail closed).
fn decode_journal_row(
    row: &sqlx::postgres::PgRow,
) -> Result<CommandTransitionRecord, CommandStoreError> {
    let try_text =
        |column: &'static str| -> Result<String, CommandStoreError> {
            row.try_get::<String, _>(column).map_err(|_| {
                CommandStoreError::Backend {
                    code: "E_STORE_DECODE".to_string(),
                    message: format!("stored value in column {column} could not be decoded"),
                }
            })
        };
    let try_opt_text =
        |column: &'static str| -> Result<Option<String>, CommandStoreError> {
            row.try_get::<Option<String>, _>(column).map_err(|_| {
                CommandStoreError::Backend {
                    code: "E_STORE_DECODE".to_string(),
                    message: format!("stored value in column {column} could not be decoded"),
                }
            })
        };
    let command_id = try_text("command_id")?;
    let tenant_text = try_text("tenant_id")?;
    let tenant_uuid = uuid_from_text(&tenant_text).map_err(|_| CommandStoreError::Backend {
        code: "E_STORE_DECODE".to_string(),
        message: "stored value in column tenant_id could not be decoded".to_string(),
    })?;
    let _seq = row.try_get::<i64, _>("seq").map_err(|_| {
        CommandStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored value in column seq could not be decoded".to_string(),
        }
    })?;
    let from_state = try_text("from_state")?;
    let to_state = try_text("to_state")?;
    let actor = try_text("actor")?;
    let at_unix_millis = row
        .try_get::<i64, _>("at_unix_millis")
        .map_err(|_| CommandStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored value in column at_unix_millis could not be decoded".to_string(),
        })?;
    let gate = try_opt_text("gate")?;
    let reason = try_opt_text("reason")?;
    let evidence_reference = try_opt_text("evidence_reference")?;

    // Closed vocabulary + the frozen edge (the domain table is the oracle).
    let from = state_from_name(&from_state).ok_or(CommandStoreError::Backend {
        code: "E_STORE_DECODE".to_string(),
        message: "stored value in column from_state is outside the frozen state vocabulary"
            .to_string(),
    })?;
    let to = state_from_name(&to_state).ok_or(CommandStoreError::Backend {
        code: "E_STORE_DECODE".to_string(),
        message: "stored value in column to_state is outside the frozen state vocabulary"
            .to_string(),
    })?;
    try_transition(from, to).map_err(|_| CommandStoreError::Backend {
        code: "E_STORE_DECODE".to_string(),
        message: "the stored journal row is not an edge of the frozen command state machine"
            .to_string(),
    })?;
    if !COMMAND_STATES.contains(&to_state.as_str()) {
        return Err(CommandStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored value in column to_state is outside the stored vocabulary"
                .to_string(),
        });
    }
    if let Some(gate) = gate.as_deref() {
        if !flexcore_storage::command_store::COMMAND_GATES.contains(&gate) {
            return Err(CommandStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: "stored value in column gate is outside the gate vocabulary".to_string(),
            });
        }
    }
    Ok(CommandTransitionRecord {
        tenant_id: flexcore_domain::ids::TenantId::new(
            flexcore_contracts::ids::FlexUuid::from_bytes(tenant_uuid.to_bytes()),
        ),
        command_id,
        from_state,
        to_state,
        actor,
        at_unix_millis,
        gate,
        reason,
        evidence_reference,
    })
}

impl CommandRecordStore for PgUnitOfWork {
    fn insert_command_record(
        &self,
        ctx: &TenantContext,
        row: &CommandRecordRow,
    ) -> Result<CommandReceipt, CommandStoreError> {
        require_uuid_text(&row.command_id, "command_id")?;
        require_uuid_text(&row.portfolio_id, "portfolio_id")?;
        require_uuid_text(&row.site_id, "site_id")?;
        require_uuid_text(&row.asset_id, "asset_id")?;
        require_uuid_text(&row.schedule_id, "schedule_id")?;
        if let Some(superseded) = row.supersedes_command_id.as_deref() {
            require_uuid_text(superseded, "supersedes_command_id")?;
        }
        let schedule_revision = i64::try_from(row.schedule_revision).map_err(|_| {
            CommandStoreError::InvalidRecord {
                field: "schedule_revision",
            }
        })?;
        // The payload must be valid JSON before it reaches the database (the
        // storage crate treats it as opaque text; this adapter owns the
        // jsonb binding and therefore the validation).
        let payload: Value = serde_json::from_str(&row.command_payload_json)
            .map_err(|_| CommandStoreError::InvalidRecord {
                field: "command_payload_json",
            })?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| CommandStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            // The initial state is always PROPOSED — the freeze's initial
            // marker (P0.11 §2 row 28); the migration's BEFORE INSERT
            // trigger rejects any other value as defence in depth.
            let result = sqlx::query(
                "INSERT INTO dispatch_commands \
                     (command_id, tenant_id, portfolio_id, site_id, asset_id, \
                      schedule_id, schedule_revision, valid_until_millis, \
                      supersedes_command_id, state, command_payload) \
                 VALUES ($1::uuid, $2::uuid, $3::uuid, $4::uuid, $5::uuid, \
                         $6::uuid, $7, $8, $9::uuid, 'PROPOSED', $10)",
            )
            .bind(&row.command_id)
            .bind(uuid_text(ctx.tenant_id().uuid()))
            .bind(&row.portfolio_id)
            .bind(&row.site_id)
            .bind(&row.asset_id)
            .bind(&row.schedule_id)
            .bind(schedule_revision)
            .bind(row.valid_until_millis)
            .bind(row.supersedes_command_id.as_deref())
            .bind(payload)
            .execute(&mut *tx)
            .await
            .map_err(|err| map_command_store_error(err, &row.command_id))?;
            if result.rows_affected() != 1 {
                return Err(CommandStoreError::Backend {
                    code: "E_STORE_REJECTED".to_string(),
                    message: "the command-record insert affected no rows".to_string(),
                });
            }
            Ok(CommandReceipt::new(
                row.command_id.clone(),
                ctx.tenant_id(),
            ))
        })
    }

    fn record_command_transition(
        &self,
        ctx: &TenantContext,
        transition: &CommandTransitionRecord,
    ) -> Result<CommandTransitionReceipt, CommandStoreError> {
        require_uuid_text(&transition.command_id, "command_id")?;
        // The backward link for the SUPERSEDED transition (D-P4.10-3).
        let superseded_by =
            superseded_by_link(&transition.to_state, transition.evidence_reference.as_deref())?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| CommandStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            // The ONE-ACTIVE-TRANSITION invariant, first half: the
            // compare-and-swap — the row moves only from the state the
            // caller saw; a concurrent transition makes this UPDATE affect
            // zero rows. The migration's BEFORE UPDATE trigger is the
            // second half (one frozen-table state change per UPDATE, no
            // content change, the link/state pairing).
            let moved = sqlx::query(
                "UPDATE dispatch_commands \
                 SET state = $3, superseded_by_command_id = $4::uuid \
                 WHERE command_id = $1::uuid AND tenant_id = $2::uuid AND state = $5 \
                 RETURNING command_id::text AS command_id",
            )
            .bind(&transition.command_id)
            .bind(uuid_text(ctx.tenant_id().uuid()))
            .bind(&transition.to_state)
            .bind(superseded_by.as_deref())
            .bind(&transition.from_state)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|err| map_command_store_error(err, &transition.command_id))?;
            if moved.is_none() {
                // Distinguish not-found from a lost compare-and-swap without
                // leaking existence across tenants (the explicit tenant
                // filter is in this query too; RLS is the outer gate).
                let existing = sqlx::query(
                    "SELECT state FROM dispatch_commands \
                     WHERE command_id = $1::uuid AND tenant_id = $2::uuid",
                )
                .bind(&transition.command_id)
                .bind(uuid_text(ctx.tenant_id().uuid()))
                .fetch_optional(&mut *tx)
                .await
                .map_err(|err| map_command_store_error(err, &transition.command_id))?;
                return match existing {
                    None => Err(CommandStoreError::NotFound {
                        command_id: transition.command_id.clone(),
                    }),
                    Some(row) => {
                        let actual: String = row.try_get("state").map_err(|_| {
                            CommandStoreError::Backend {
                                code: "E_STORE_DECODE".to_string(),
                                message:
                                    "stored value in column state could not be decoded"
                                        .to_string(),
                            }
                        })?;
                        Err(CommandStoreError::TransitionConflict {
                            command_id: transition.command_id.clone(),
                            expected_from: transition.from_state.clone(),
                            actual_state: actual,
                        })
                    }
                };
            }
            // The journal row, in the SAME transaction (ledger P4.10 step
            // 6): from, to, actor, timestamp, reason, evidence reference,
            // gate. INSERT-only for the runtime role — journal rows are
            // never updated or deleted.
            let seq: i64 = sqlx::query(
                "INSERT INTO dispatch_command_transitions \
                     (command_id, tenant_id, from_state, to_state, actor, \
                      at_unix_millis, gate, reason, evidence_reference) \
                 VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6, $7, $8, $9) \
                 RETURNING seq",
            )
            .bind(&transition.command_id)
            .bind(uuid_text(ctx.tenant_id().uuid()))
            .bind(&transition.from_state)
            .bind(&transition.to_state)
            .bind(&transition.actor)
            .bind(transition.at_unix_millis)
            .bind(transition.gate.as_deref())
            .bind(transition.reason.as_deref())
            .bind(transition.evidence_reference.as_deref())
            .fetch_one(&mut *tx)
            .await
            .map_err(|err| map_command_store_error(err, &transition.command_id))?
            .try_get::<i64, _>("seq")
            .map_err(|_| CommandStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: "the journal sequence number could not be decoded".to_string(),
            })?;
            Ok(CommandTransitionReceipt {
                command_id: transition.command_id.clone(),
                seq,
                tenant_id: ctx.tenant_id(),
            })
        })
    }

    fn find_command(
        &self,
        ctx: &TenantContext,
        command_id: &str,
    ) -> Result<Option<CommandRecord>, CommandStoreError> {
        require_uuid_text(command_id, "command_id")?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| CommandStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let row = sqlx::query(&format!(
                "{COMMAND_SELECT} WHERE command_id = $1::uuid AND tenant_id = $2::uuid"
            ))
            .bind(command_id)
            .bind(uuid_text(ctx.tenant_id().uuid()))
            .fetch_optional(&mut *tx)
            .await
            .map_err(|err| map_command_store_error(err, command_id))?;
            match row {
                // A foreign-tenant row reads as None — no existence leak
                // (P0.16 rule 5; RLS already filtered it).
                None => Ok(None),
                Some(row) => decode_command_record_row(&row).map(Some),
            }
        })
    }

    fn transition_journal(
        &self,
        ctx: &TenantContext,
        command_id: &str,
    ) -> Result<Vec<CommandTransitionRecord>, CommandStoreError> {
        require_uuid_text(command_id, "command_id")?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| CommandStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let rows = sqlx::query(&format!(
                "{COMMAND_JOURNAL_SELECT} WHERE command_id = $1::uuid \
                 AND tenant_id = $2::uuid ORDER BY seq ASC"
            ))
            .bind(command_id)
            .bind(uuid_text(ctx.tenant_id().uuid()))
            .fetch_all(&mut *tx)
            .await
            .map_err(|err| map_command_store_error(err, command_id))?;
            rows.iter().map(decode_journal_row).collect()
        })
    }

    fn ensure_schedule_revision_usable(
        &self,
        ctx: &TenantContext,
        schedule_id: &str,
        revision: u64,
    ) -> Result<(), CommandStoreError> {
        require_uuid_text(schedule_id, "schedule_id")?;
        // The P4.12 pre-wiring gate check over the P3.22 surface
        // (D-P4.10-5): `Some(true)` = SUPERSEDED (rejected),
        // `Some(false)` = active (usable), `None` = unknown in this tenant
        // scope (ALSO rejected — unknown is never "usable"). NO feasibility
        // gate here — the §12.5 network pipeline is P4.12's.
        match ScheduleRevisionChainStore::is_revision_superseded(self, ctx, schedule_id, revision) {
            Ok(Some(true)) => Err(CommandStoreError::ScheduleRevisionSuperseded {
                schedule_id: schedule_id.to_string(),
                revision,
            }),
            Ok(Some(false)) => Ok(()),
            Ok(None) => Err(CommandStoreError::ScheduleRevisionUnknown {
                schedule_id: schedule_id.to_string(),
                revision,
            }),
            Err(err) => Err(map_revision_check_error(err)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_storage::command_store::COMMAND_STATES;

    // The composition root holds ports as trait objects; object safety is
    // part of the contract (the same evidence pattern as the adapter's
    // `every_implemented_port_is_object_safe`). Compile-level evidence that
    // PgUnitOfWork carries the command-record store.
    #[test]
    fn the_command_record_store_is_object_safe_for_the_unit_of_work() {
        fn takes_dyn_store(_: &dyn CommandRecordStore) {}
        let _ = takes_dyn_store as fn(&dyn CommandRecordStore);
    }

    // The backward-link derivation is total and validated: only the
    // SUPERSEDED state carries a link, and only a `command:<uuid>` evidence
    // reference yields one (D-P4.10-3).
    #[test]
    fn the_superseded_link_is_derived_only_from_the_supersede_evidence() {
        let replacing = "0192a5e0-0000-7000-8000-0000000000a2";
        assert_eq!(
            superseded_by_link(
                flexcore_storage::command_store::COMMAND_STATE_SUPERSEDED,
                Some(&format!("command:{replacing}")),
            )
            .expect("the link derives"),
            Some(replacing.to_string())
        );
        // Any other state carries NO link.
        assert_eq!(
            superseded_by_link("ISSUED", Some(&format!("command:{replacing}")))
                .expect("no link for a non-supersede transition"),
            None
        );
        // A SUPERSEDED transition without the reference is refused.
        assert_eq!(
            superseded_by_link(flexcore_storage::command_store::COMMAND_STATE_SUPERSEDED, None),
            Err(CommandStoreError::InvalidRecord {
                field: "evidence_reference"
            })
        );
        // A non-`command:` reference is refused.
        assert_eq!(
            superseded_by_link(
                flexcore_storage::command_store::COMMAND_STATE_SUPERSEDED,
                Some("audit:entry-7")
            ),
            Err(CommandStoreError::InvalidRecord {
                field: "evidence_reference"
            })
        );
        // A non-UUID replacing id is refused.
        assert_eq!(
            superseded_by_link(
                flexcore_storage::command_store::COMMAND_STATE_SUPERSEDED,
                Some("command:not-a-uuid")
            ),
            Err(CommandStoreError::InvalidRecord {
                field: "superseded_by_command_id"
            })
        );
    }

    // The stored vocabulary parses back into the closed domain vocabulary.
    #[test]
    fn every_stored_state_name_parses_back() {
        for stored in COMMAND_STATES {
            assert!(
                state_from_name(stored).is_some(),
                "stored state {stored} must parse back into the domain vocabulary"
            );
        }
        assert!(state_from_name("SUPERCEDED").is_none());
        assert!(state_from_name("").is_none());
    }
}
