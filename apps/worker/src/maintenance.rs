// P2.20 — The partition-maintenance job: scheduled pre-creation of monthly
// telemetry partitions ahead of the ingest frontier (D-R1) and the
// approved-operation retention path (detach first, drop only in a second
// approved step — D-R2), with the no-retention-configured state visible at
// every pass (D-R4).
//
// Decisions (full rationale in
// `ledger/phase-02-asset-telemetry/19_20_historical_retention.md` §2):
//
// - **Scheduled pre-creation is the mechanism** (D-R1): a config-driven
//   interval loop runs a pass that plans the missing months
//   ([`flexcore_postgres_adapter::partition_maintenance::plan_pre_creation`],
//   pure) and creates them idempotently by name. The ingestion path's
//   missing-partition behaviour is UNCHANGED and referenced: the repository
//   classifies the PostgreSQL routing error as
//   `TelemetryError::OutOfRangeEventTime` and the pipeline quarantines and
//   counts it — never a silent drop (P2.05 §7 item 2).
// - **The loop runs only with the maintenance database URL** (D-R1): the
//   pass needs DDL, which the runtime role must not hold (D-P6); the
//   composition root starts this loop only when
//   `FLEXCORE_MAINTENANCE_DATABASE_URL` is configured.
// - **Retention execution is a manual/approved operation** (D-R2): the loop
//   only ever DETACHES, and only when a retention value is configured for
//   the data class AND the run is flagged approved; configured-but-
//   unapproved is a visible warning and deletes nothing. DROP is the
//   second, explicitly invoked approved step ([`drop_step`]) — never wired
//   to the loop.
// - **No retention configured → no deletion, visibly** (D-R4): every pass
//   reports the unconfigured state (report status + log line; the
//   scrapeable metric family is P2.21's surface). Retention values are
//   configuration, never hard-coded constants (P0.16 §4; D-P5).
// - **One audit record per action** (D-R5), written by the adapter in the
//   same transaction as the DDL (platform-level record; the maintenance
//   connection is the recorded owner/ops path, P1.12 §5.2).
//
// Sync-port hosting: the adapter methods are synchronous over a private
// runtime and are called from this async loop via `tokio::task::
// spawn_blocking` — the same mechanism the runner loops use
// (`block_in_place` there; the passes here are self-contained blocking
// units, so a blocking task per pass is the cleaner spelling).
//
// Simulation-only scope (AGENTS.md §3 rule 5): this job manages storage
// partitions of simulated telemetry; it performs no device writes and
// grants no control authority.

use std::sync::Arc;
use std::time::Duration;

use flexcore_ports::backend_error::BackendError;
use flexcore_postgres_adapter::partition_maintenance::{
    plan_pre_creation, retention_cutoff, retention_eligible, MaintenanceError,
    PartitionAuditContext, PartitionCreateOutcome,
};
use flexcore_postgres_adapter::{FlexCoreStore, SystemClock};
use tokio::sync::watch;

use crate::config::ENV_RETENTION_NORMALIZED_TELEMETRY_MONTHS;
use crate::ingestion::UuidV7Generator;

/// The actor recorded on loop-driven retention audit records (an operator
/// invoking [`drop_step`] supplies their own identity).
pub const MAINTENANCE_ACTOR: &str = "flexcore-worker:maintenance";

/// The settings one maintenance loop runs with (derived from the worker
/// configuration; the retention value is whatever configuration supplied —
/// never a constant here).
#[derive(Debug, Clone)]
pub struct MaintenanceSettings {
    /// Monthly partitions kept created ahead of the ingest frontier (D-R1).
    pub months_ahead: u32,
    /// The retention horizon in months for the "normalized telemetry" data
    /// class; `None` = no retention configured (D-R4).
    pub retention_months: Option<u32>,
    /// Whether THIS run is flagged approved to execute retention (D-R2).
    pub retention_approved: bool,
    /// The configuration source the retention value came from (recorded on
    /// every audit record, D-R5).
    pub retention_config_source: String,
    /// The actor identity for loop-driven audit records.
    pub actor: String,
}

impl MaintenanceSettings {
    /// The settings from the worker configuration's maintenance fields.
    pub fn from_config(
        months_ahead: u32,
        retention_months: Option<u32>,
        retention_approved: bool,
    ) -> Self {
        Self {
            months_ahead,
            retention_months,
            retention_approved,
            retention_config_source: ENV_RETENTION_NORMALIZED_TELEMETRY_MONTHS.to_string(),
            actor: MAINTENANCE_ACTOR.to_string(),
        }
    }
}

/// The outcome class of one retention pass (D-R2/D-R4). The status IS the
/// recorded visibility: `Unconfigured` and `NotApproved` passes delete
/// nothing and are reported at every pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetentionStatus {
    /// No retention configured for any data class: no deletion occurs
    /// (D-R4).
    Unconfigured,
    /// Retention configured but this run is not flagged approved: no
    /// deletion occurs (D-R2).
    NotApproved,
    /// The approved pass executed (detach only; drop is a separate step).
    Executed,
    /// The approved pass failed before completing; nothing was detached.
    Failed,
}

impl RetentionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            RetentionStatus::Unconfigured => "unconfigured",
            RetentionStatus::NotApproved => "not_approved",
            RetentionStatus::Executed => "executed",
            RetentionStatus::Failed => "failed",
        }
    }
}

/// The report of one retention pass — the value the P2.21 metric family
/// will scrape and the log line renders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetentionReport {
    pub status: RetentionStatus,
    /// Partitions detached by this pass (empty unless `Executed`).
    pub detached: Vec<String>,
    /// Partitions the adapter refused (D-R3) during an executed pass.
    pub refused: Vec<String>,
    /// The human-readable note: what happened and why (the D-R4
    /// unconfigured-state line lives here).
    pub note: String,
}

fn maintenance_backend(e: BackendError) -> MaintenanceError {
    MaintenanceError::Backend(e)
}

/// One pre-creation pass (D-R1): plan the missing monthly partitions for
/// `[frontier, frontier + months_ahead)` and create them idempotently inside
/// one unit of work (all-or-nothing; a failure rolls back and the next pass
/// retries — every step is idempotent by partition name). Returns the names
/// actually created. Synchronous: invoke from a blocking context.
pub fn pre_creation_pass(
    store: &FlexCoreStore,
    months_ahead: u32,
) -> Result<Vec<String>, MaintenanceError> {
    let now = SystemClock.now_event_time().timestamp();
    let unit = store.begin(None).map_err(maintenance_backend)?;
    let shared = Arc::new(unit);
    let existing = shared.list_telemetry_partitions()?;
    let plan = plan_pre_creation(&existing, now.year(), u8::from(now.month()), months_ahead);
    let mut created = Vec::with_capacity(plan.len());
    for planned in plan {
        match shared.create_telemetry_partition(planned.year, planned.month)? {
            PartitionCreateOutcome::Created(name) => created.push(name),
            PartitionCreateOutcome::AlreadyExists(_) => {}
        }
    }
    match Arc::try_unwrap(shared) {
        Ok(unit) => {
            unit.commit().map_err(maintenance_backend)?;
        }
        Err(_still_shared) => {
            return Err(MaintenanceError::Backend(BackendError::new(
                "internal_error".to_string(),
                "the pre-creation unit of work escaped; nothing was committed".to_string(),
            )))
        }
    }
    Ok(created)
}

/// One retention pass (D-R2/D-R4). Synchronous: invoke from a blocking
/// context. Detach ONLY — the drop step is separate and explicitly invoked
/// ([`drop_step`]).
pub fn retention_pass(store: &FlexCoreStore, settings: &MaintenanceSettings) -> RetentionReport {
    let Some(retention_months) = settings.retention_months else {
        // D-R4: the recorded no-retention-configured behaviour — no
        // deletion occurs, and the state is visible (report + log line).
        return RetentionReport {
            status: RetentionStatus::Unconfigured,
            detached: Vec::new(),
            refused: Vec::new(),
            note: "no retention is configured for any data class; no deletion occurs and \
                   telemetry partitions accumulate unboundedly (D-R4: the unconfigured state \
                   is visible, never silent)"
                .to_string(),
        };
    };
    if !settings.retention_approved {
        return RetentionReport {
            status: RetentionStatus::NotApproved,
            detached: Vec::new(),
            refused: Vec::new(),
            note: format!(
                "retention for the normalized-telemetry data class is configured via {} \
                 ({} months) but this run is not flagged approved; no deletion occurs (D-R2)",
                settings.retention_config_source, retention_months
            ),
        };
    }
    match execute_detach_only(store, retention_months, settings) {
        Ok((detached, refused)) => RetentionReport {
            status: RetentionStatus::Executed,
            detached,
            refused,
            note: format!(
                "detach executed for the normalized-telemetry data class ({} months via {}); \
                 drop is a separate, explicitly invoked approved step (D-R2)",
                retention_months, settings.retention_config_source
            ),
        },
        Err(error) => RetentionReport {
            status: RetentionStatus::Failed,
            detached: Vec::new(),
            refused: Vec::new(),
            note: format!("the retention pass failed; nothing was detached: {error}"),
        },
    }
}

/// The approved detach execution: partitions entirely outside the retention
/// window (adapter [`retention_eligible`] over catalog bounds) are detached,
/// one audited transaction for the whole pass (D-R5). The adapter's D-R3
/// refusals (current/in-flight, future bounds, unknown bounds) are collected
/// as `refused`, never retried in-pass.
fn execute_detach_only(
    store: &FlexCoreStore,
    retention_months: u32,
    settings: &MaintenanceSettings,
) -> Result<(Vec<String>, Vec<String>), MaintenanceError> {
    let now = SystemClock.now_event_time().timestamp();
    let unit = store.begin(None).map_err(maintenance_backend)?;
    let shared = Arc::new(unit);
    let entries = shared.list_telemetry_partitions()?;
    let cutoff = retention_cutoff(now.year(), u8::from(now.month()), retention_months)
        .ok_or_else(|| MaintenanceError::Refused {
            partition: "*".to_string(),
            reason: "the retention horizon underflows the representable calendar".to_string(),
        })?;
    let eligible = retention_eligible(&entries, cutoff, now);
    let ids = UuidV7Generator::new(store.pool().clone());
    let mut detached = Vec::with_capacity(eligible.len());
    let mut refused = Vec::new();
    for target in eligible {
        let audit = PartitionAuditContext {
            audit_id: ids.new_uuid(),
            correlation_id: ids.new_uuid(),
            actor: settings.actor.clone(),
            config_source: settings.retention_config_source.clone(),
        };
        match shared.detach_telemetry_partition(&target.name, now, &audit) {
            Ok(_bounds) => detached.push(target.name),
            Err(MaintenanceError::Refused { partition, .. }) => refused.push(partition),
            Err(error) => return Err(error),
        }
    }
    match Arc::try_unwrap(shared) {
        Ok(unit) => {
            unit.commit().map_err(maintenance_backend)?;
        }
        Err(_still_shared) => {
            return Err(MaintenanceError::Backend(BackendError::new(
                "internal_error".to_string(),
                "the retention unit of work escaped; nothing was committed".to_string(),
            )))
        }
    }
    Ok((detached, refused))
}

/// The SECOND approved retention step (D-R2): drops named ALREADY-DETACHED
/// partitions, one audited transaction each (the adapter refuses any name
/// still attached to `telemetry`). Explicitly invoked by the approved ops
/// path with the operator identity — NEVER by the maintenance loop.
/// Synchronous: invoke from a blocking context.
pub fn drop_step(
    store: &FlexCoreStore,
    names: &[String],
    actor: &str,
    config_source: &str,
) -> Result<Vec<String>, MaintenanceError> {
    let now = SystemClock.now_event_time().timestamp();
    let ids = UuidV7Generator::new(store.pool().clone());
    let mut dropped = Vec::with_capacity(names.len());
    for name in names {
        let unit = store.begin(None).map_err(maintenance_backend)?;
        let audit = PartitionAuditContext {
            audit_id: ids.new_uuid(),
            correlation_id: ids.new_uuid(),
            actor: actor.to_string(),
            config_source: config_source.to_string(),
        };
        unit.drop_detached_telemetry_partition(name, now, &audit)?;
        unit.commit().map_err(maintenance_backend)?;
        dropped.push(name.clone());
    }
    Ok(dropped)
}

/// The maintenance loop until shutdown (D-R1/D-R4): one pre-creation pass
/// and one retention-status/execution pass per interval tick, each on a
/// blocking task. The first tick fires immediately (a pass at startup is
/// intended — partitions are cheap and idempotent). A failed pass is logged
/// and retried on the next tick; no pass can lose data (pre-creation only
/// creates, retention only detaches out-of-window partitions with audit
/// records committed atomically).
pub async fn run_maintenance_loop(
    store: FlexCoreStore,
    settings: MaintenanceSettings,
    interval: Duration,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut ticker = tokio::time::interval(interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = ticker.tick() => {}
            _ = shutdown.changed() => {
                // A change (or the sender dropping) means the worker is
                // draining: stop before starting a new pass.
                return;
            }
        }
        if *shutdown.borrow() {
            return;
        }

        let pass_store = store.clone();
        let months_ahead = settings.months_ahead;
        let created =
            tokio::task::spawn_blocking(move || pre_creation_pass(&pass_store, months_ahead))
                .await;
        match created {
            Ok(Ok(names)) if !names.is_empty() => {
                tracing::info!(
                    partitions = ?names,
                    months_ahead,
                    "partition pre-creation pass created monthly partitions (P2.20 D-R1)"
                );
            }
            Ok(Ok(_)) => {
                tracing::debug!("partition pre-creation pass: nothing to create");
            }
            Ok(Err(error)) => {
                tracing::error!(
                    error = %error,
                    "partition pre-creation pass failed; the next pass retries (every step \
                     is idempotent by partition name)"
                );
            }
            Err(join_error) => {
                tracing::error!(error = %join_error, "the pre-creation pass task panicked");
            }
        }

        let pass_store = store.clone();
        let pass_settings = settings.clone();
        let retention =
            tokio::task::spawn_blocking(move || retention_pass(&pass_store, &pass_settings))
                .await;
        match retention {
            Ok(report) => match report.status {
                RetentionStatus::Unconfigured => {
                    // The D-R4 visibility line, emitted at every pass.
                    tracing::info!(
                        status = report.status.as_str(),
                        note = %report.note,
                        "maintenance retention status: no deletion occurs (no retention \
                         configured)"
                    );
                }
                RetentionStatus::NotApproved => {
                    tracing::warn!(
                        status = report.status.as_str(),
                        note = %report.note,
                        "maintenance retention status: configured but not approved — no \
                         deletion occurs"
                    );
                }
                RetentionStatus::Executed => {
                    tracing::info!(
                        status = report.status.as_str(),
                        detached = ?report.detached,
                        refused = ?report.refused,
                        note = %report.note,
                        "retention pass executed (detach only)"
                    );
                }
                RetentionStatus::Failed => {
                    tracing::error!(
                        status = report.status.as_str(),
                        note = %report.note,
                        "retention pass failed"
                    );
                }
            },
            Err(join_error) => {
                tracing::error!(error = %join_error, "the retention pass task panicked");
            }
        }
    }
}
