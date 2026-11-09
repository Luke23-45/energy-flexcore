// P2.20 — Partition maintenance for the telemetry table: monthly
// pre-creation ahead of the ingest frontier and the approved-operation
// retention path (detach first, drop only in a second approved step), with
// one audit record per action written in the SAME transaction as the DDL.
//
// Decisions (D-R1..D-R6 in
// `ledger/phase-02-asset-telemetry/19_20_historical_retention.md`):
//
// - **D-R1 Scheduled pre-creation is the mechanism.** The worker's
//   maintenance loop calls [`PgUnitOfWork::create_telemetry_partition`] for
//   the months the pure planner ([`plan_pre_creation`]) names as missing.
//   The ingestion path's missing-partition behaviour is unchanged and
//   referenced: `telemetry::TelemetryError::OutOfRangeEventTime` (SQLSTATE
//   23514, no constraint name) — the pipeline quarantines and counts, never
//   silently drops.
// - **D-R2 Retention is a manual/approved operation.** The worker loop only
//   ever DETACHES, and only when a retention value is configured for the
//   data class AND the run is flagged approved. DROP is a separate,
//   explicitly invoked function that refuses partitions still attached to
//   `telemetry`. The regular `ALTER TABLE ... DETACH PARTITION` is used
//   rather than `DETACH CONCURRENTLY` because CONCURRENTLY cannot run
//   inside a transaction and the audit record must commit atomically with
//   the DDL (D-R5); the brief ACCESS EXCLUSIVE lock is acceptable at Phase
//   2 volumes.
// - **D-R3 Safety rule, enforced in code with an explicit refusal error:**
//   the current/in-flight partition (bounds contain the current instant)
//   and any partition whose upper bound is in the future are NEVER eligible;
//   a partition is eligible only when it lies entirely outside the retention
//   window (upper bound <= cutoff, computed from the catalog bounds). A
//   partition whose bounds cannot be parsed from the catalog is never
//   eligible (fail-safe — unknown bounds are never acted on).
// - **D-R4 No-retention-configured behaviour:** when no retention value is
//   configured for a data class, NO deletion occurs; the unconfigured state
//   is reported at every maintenance pass (the report value + log line are
//   the visibility; the scrapeable metric family is P2.21's surface).
//   Retention values are configuration, never hard-coded constants (P0.16
//   §4; D-P5: no retention value appears in this module or any DDL here).
// - **D-R5 Audit records:** one platform-level audit record
//   (`tenant_id` NULL — retention is not a tenant action) per detach/drop,
//   inserted in the same transaction as the DDL via the crate's audit
//   INSERT (the `AuditSink` implementation on `PgUnitOfWork` is the same
//   path). A platform-level row under the RLS policy is writable only
//   through the owner/ops path (P1.12 §5.2, migration 0008) — the
//   maintenance connection IS that path (D-R1: the loop runs only with the
//   maintenance database URL, the migrator/maintenance role).
// - **D-R6 Bounds source of truth:** the PostgreSQL catalog
//   (`pg_inherits` + `pg_get_expr(c.relpartbound, c.oid)`), parsed strictly
//   against the canonical `FOR VALUES FROM ('<ts>') TO ('<ts>')` rendering.
//   VERIFY-BUILD: the exact `relpartbound` text rendering is a recorded
//   assumption verified by the P2.20 partition tests on PostgreSQL 18; a
//   rendering change yields parse failure → the partition is reported with
//   unknown bounds and skipped from retention, never mis-bounded.
//
// No migration creates partitions and no DDL here carries a retention
// value: partition creation is runtime DDL by design (migration 0010's
// recorded reasoning), and D-P5 forbids retention values in schema or code.

use flexcore_audit::record::{
    ActorIdentity, AuditAction, AuditDecision, AuditObject, AuditRecord, AuditRecordFields,
    DecisionOutcome, RecordReference,
};
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{EventTime, IngestTime};
use flexcore_ports::backend_error::BackendError;
use time::OffsetDateTime;

use crate::error::{codes, map_backend};
use crate::unit_of_work::{tx_ref, PgUnitOfWork};

// --- Errors ------------------------------------------------------------------

/// The typed errors of the partition-maintenance path. `Refused` is the
/// D-R3 explicit safety refusal — a request the maintenance path declines
/// BEFORE any DDL runs, with the partition name and the reason.
#[derive(Debug)]
pub enum MaintenanceError {
    /// The named relation is not a telemetry partition (or does not exist).
    UnknownPartition { partition: String },
    /// The D-R3 safety refusal: current/in-flight partition, still-attached
    /// drop, or bounds that could not be determined (fail-safe).
    Refused { partition: String, reason: String },
    /// Any other storage failure, opaque (stable code + fixed message).
    Backend(BackendError),
}

impl std::fmt::Display for MaintenanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MaintenanceError::UnknownPartition { partition } => {
                write!(f, "'{partition}' is not a telemetry partition")
            }
            MaintenanceError::Refused { partition, reason } => {
                write!(f, "partition maintenance refused for '{partition}': {reason}")
            }
            MaintenanceError::Backend(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for MaintenanceError {}

fn maintenance_backend(e: BackendError) -> MaintenanceError {
    MaintenanceError::Backend(e)
}

fn maintenance_rejected(message: &'static str) -> MaintenanceError {
    MaintenanceError::Backend(BackendError::new(codes::REJECTED.to_string(), message.to_string()))
}

// --- Pure month arithmetic and naming (unit-testable, no I/O) ----------------

/// The zero-based month index of a (year, month) pair (month 1..=12).
/// Proleptic Gregorian arithmetic — years may be negative (the arithmetic
/// is only ever fed plausible frontier months, but stays total).
pub fn month_index(year: i32, month: u32) -> i64 {
    i64::from(year) * 12 + i64::from(month) - 1
}

/// The inverse of [`month_index`]: a zero-based index back to `(year,
/// month)` with month in 1..=12.
pub fn month_from_index(index: i64) -> (i32, u32) {
    let year = index.div_euclid(12);
    let month0 = index.rem_euclid(12);
    (year as i32, month0 as u32 + 1)
}

/// Shifts a (year, month) pair by `delta` months (`delta` may be negative).
pub fn month_add(year: i32, month: u32, delta: i64) -> (i32, u32) {
    month_from_index(month_index(year, month) + delta)
}

/// The first day of a month as a UTC midnight date, or `None` for a month
/// outside 1..=12 (the caller refuses rather than guessing).
fn first_of_month(year: i32, month: u32) -> Option<time::Date> {
    let month = match month {
        1 => time::Month::January,
        2 => time::Month::February,
        3 => time::Month::March,
        4 => time::Month::April,
        5 => time::Month::May,
        6 => time::Month::June,
        7 => time::Month::July,
        8 => time::Month::August,
        9 => time::Month::September,
        10 => time::Month::October,
        11 => time::Month::November,
        12 => time::Month::December,
        _ => return None,
    };
    time::Date::from_calendar_date(year, month, 1).ok()
}

/// The D-P2 monthly partition bounds `[first day of month, first day of
/// next month)` as UTC midnights. `None` for a month outside 1..=12 or a
/// year the calendar cannot represent (refused, never guessed).
pub fn month_bounds(year: i32, month: u32) -> Option<(OffsetDateTime, OffsetDateTime)> {
    let first = first_of_month(year, month)?;
    let next = first_of_month(month_add(year, month, 1).0, month_add(year, month, 1).1)?;
    Some((
        first.with_time(time::Time::MIDNIGHT).assume_utc(),
        next.with_time(time::Time::MIDNIGHT).assume_utc(),
    ))
}

/// The D-P2 partition name convention: `telemetry_yYYYYmMM`.
pub fn partition_name(year: i32, month: u32) -> String {
    format!("telemetry_y{year:04}m{month:02}")
}

/// Whether a name matches the D-P2 convention exactly. Every DDL statement
/// in this module interpolates a partition name, so every name is validated
/// against this pattern first (the names also come from the catalog, but
/// the validation does not rely on that).
pub fn is_telemetry_partition_name(name: &str) -> bool {
    let rest = match name.strip_prefix("telemetry_y") {
        Some(rest) => rest,
        None => return false,
    };
    let bytes = rest.as_bytes();
    if bytes.len() != 7 || bytes[4] != b'm' {
        return false;
    }
    bytes[..4].iter().all(u8::is_ascii_digit) && bytes[5..].iter().all(u8::is_ascii_digit)
}

/// Normalizes a PostgreSQL timestamptz text rendering to RFC 3339: the
/// date-time separator becomes `T`, and a whole-hour offset (`+HH` /
/// `-HH`) gains its mandatory `:00` minutes. VERIFY-BUILD: the rendering is
/// a recorded assumption (D-R6); a value that still fails to parse yields
/// `None` and the partition is treated as having unknown bounds (fail-safe).
fn normalize_pg_timestamp(raw: &str) -> String {
    let mut normalized = raw.replacen(' ', "T", 1);
    let tail_len = 3;
    if normalized.len() >= tail_len {
        let tail = &normalized[normalized.len() - tail_len..];
        let tail_bytes = tail.as_bytes();
        let is_offset = tail_bytes[0] == b'+' || tail_bytes[0] == b'-';
        let rest_digits = tail_bytes[1..].iter().all(u8::is_ascii_digit);
        if is_offset && rest_digits {
            normalized.push_str(":00");
        }
    }
    normalized
}

/// Parses one timestamp out of a `relpartbound` expression.
fn parse_bound_timestamp(raw: &str) -> Option<OffsetDateTime> {
    let normalized = normalize_pg_timestamp(raw);
    OffsetDateTime::parse(&normalized, &time::format_description::well_known::Rfc3339).ok()
}

/// Parses the canonical `relpartbound` rendering of a RANGE partition
/// bound: `FOR VALUES FROM ('<from>') TO ('<until>')`. `None` for any other
/// shape — the caller treats unknown bounds as ineligible (D-R3 fail-safe).
pub fn parse_partition_bounds(expr: &str) -> Option<(OffsetDateTime, OffsetDateTime)> {
    let rest = expr.trim().strip_prefix("FOR VALUES FROM ('")?;
    let (from_text, rest) = rest.split_once("') TO ('")?;
    let to_text = rest.strip_suffix("')")?;
    let from = parse_bound_timestamp(from_text)?;
    let until = parse_bound_timestamp(to_text)?;
    Some((from, until))
}

/// The retention cutoff for a retention horizon of `retention_months` from
/// the current (year, month): the first instant of that month minus the
/// horizon. A partition is eligible exactly when its upper bound is `<=`
/// this cutoff (D-R3). `None` for an out-of-vocabulary month or a horizon
/// that underflows the calendar.
pub fn retention_cutoff(year: i32, month: u32, retention_months: u32) -> Option<OffsetDateTime> {
    let index = month_index(year, month).checked_sub(i64::from(retention_months))?;
    let (cutoff_year, cutoff_month) = month_from_index(index);
    month_bounds(cutoff_year, cutoff_month).map(|(from, _)| from)
}

// --- Catalog-facing values -----------------------------------------------------

/// One telemetry partition as listed from the catalog. `bounds` is `None`
/// exactly when the `relpartbound` rendering could not be parsed — such a
/// partition is never eligible for retention (D-R3 fail-safe) and is
/// reported, never acted on.
#[derive(Debug, Clone, PartialEq)]
pub struct PartitionEntry {
    pub name: String,
    pub bounds: Option<PartitionBounds>,
}

/// The parsed bounds of one monthly partition: `[from_utc, until_utc)`.
#[derive(Debug, Clone, PartialEq)]
pub struct PartitionBounds {
    pub from_utc: OffsetDateTime,
    pub until_utc: OffsetDateTime,
}

impl PartitionBounds {
    /// Whether the instant falls inside `[from, until)`.
    pub fn contains(&self, instant: OffsetDateTime) -> bool {
        self.from_utc <= instant && instant < self.until_utc
    }

    /// Whether the partition lies entirely before the cutoff (the D-R3
    /// eligibility test: entirely outside the retention window).
    pub fn entirely_before(&self, cutoff: OffsetDateTime) -> bool {
        self.until_utc <= cutoff
    }
}

/// One partition the pre-creation planner named as missing (D-R1).
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedPartition {
    pub year: i32,
    pub month: u32,
    pub name: String,
    pub from_utc: OffsetDateTime,
    pub until_utc: OffsetDateTime,
}

/// Plans the monthly partitions needed to cover `months_ahead` months
/// starting AT the frontier month (the current month counts as ahead-of
/// coverage — a partition for the in-flight month may legitimately be
/// missing on a fresh deployment). Pure: the caller executes the plan.
pub fn plan_pre_creation(
    existing: &[PartitionEntry],
    frontier_year: i32,
    frontier_month: u32,
    months_ahead: u32,
) -> Vec<PlannedPartition> {
    let mut plan = Vec::new();
    for offset in 0..i64::from(months_ahead) {
        let (year, month) = month_add(frontier_year, frontier_month, offset);
        let name = partition_name(year, month);
        if existing.iter().any(|entry| entry.name == name) {
            continue;
        }
        if let Some((from_utc, until_utc)) = month_bounds(year, month) {
            plan.push(PlannedPartition {
                year,
                month,
                name,
                from_utc,
                until_utc,
            });
        }
    }
    plan
}

/// The D-R3 eligibility filter: partitions that lie entirely outside the
/// retention window (upper bound <= cutoff) and demonstrably do not contain
/// the current instant. Entries with unknown bounds are never eligible.
pub fn retention_eligible(
    existing: &[PartitionEntry],
    cutoff: OffsetDateTime,
    now: OffsetDateTime,
) -> Vec<PartitionBoundsWithName> {
    let mut eligible = Vec::new();
    for entry in existing {
        let Some(bounds) = &entry.bounds else {
            continue; // unknown bounds: fail-safe, never acted on (D-R3)
        };
        // The containment guard is belt and braces: a partition containing
        // `now` cannot also satisfy `until <= cutoff` (cutoff <= now), but
        // the D-R3 rule is stated as an explicit check, not an implication.
        if bounds.contains(now) || bounds.until_utc > now {
            continue;
        }
        if bounds.entirely_before(cutoff) {
            eligible.push(PartitionBoundsWithName {
                name: entry.name.clone(),
                bounds: bounds.clone(),
            });
        }
    }
    eligible
}

/// An eligible partition: its name together with its parsed bounds (the
/// caller passes both to the audited detach).
#[derive(Debug, Clone, PartialEq)]
pub struct PartitionBoundsWithName {
    pub name: String,
    pub bounds: PartitionBounds,
}

// --- Audit context -------------------------------------------------------------

/// The caller-supplied identity of ONE retention audit action (D-R5): one
/// context per action, with fresh identifiers per record (audit_records'
/// primary key is `audit_id`; the caller generates via its ID port).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionAuditContext {
    /// The fresh `audit_id` for THIS action's record.
    pub audit_id: FlexUuid,
    /// The correlation id of the approved run (fresh per action).
    pub correlation_id: FlexUuid,
    /// Who invoked the action (e.g. `flexcore-worker:maintenance` or an
    /// operator identity for an explicitly invoked drop step).
    pub actor: String,
    /// Which configuration source supplied the retention value (e.g. the
    /// environment variable name) — recorded so the audit trail shows what
    /// authorized the deletion.
    pub config_source: String,
}

impl PartitionAuditContext {
    /// Validates the human-supplied fields (the audit port rejects empty
    /// actors/actions upstream; validated here so a hollow context costs no
    /// round trip).
    fn validated(&self) -> Result<(), MaintenanceError> {
        if self.actor.trim().is_empty() || self.config_source.trim().is_empty() {
            return Err(maintenance_rejected(
                "a partition-maintenance audit record requires an actor and a configuration source",
            ));
        }
        Ok(())
    }
}

/// Builds the one audit record for a detach or drop (D-R5): partition name
/// as the object id, the action, the event/ingest timestamps, the config
/// source as the after-reference, and the bounds as the evidence reference.
fn build_audit_record(
    ctx: &PartitionAuditContext,
    action: &'static str,
    partition: &str,
    bounds_note: String,
    now: OffsetDateTime,
) -> Result<AuditRecord, MaintenanceError> {
    ctx.validated()?;
    let event_time = EventTime::new(now)
        .map_err(|_| maintenance_rejected("the maintenance instant is not a valid event time"))?;
    let ingest_time = IngestTime::new(now)
        .map_err(|_| maintenance_rejected("the maintenance instant is not a valid ingest time"))?;
    AuditRecord::new(AuditRecordFields {
        audit_id: ctx.audit_id,
        // Platform-level action: retention is not a tenant action (D-R5).
        tenant_id: None,
        actor: ActorIdentity::new(ctx.actor.clone())
            .map_err(|_| maintenance_rejected("the maintenance actor identity is invalid"))?,
        action: AuditAction::new(action.to_string())
            .map_err(|_| maintenance_rejected("the maintenance action name is invalid"))?,
        object: AuditObject::new("telemetry_partition".to_string(), partition.to_string())
            .map_err(|_| maintenance_rejected("the partition audit object is invalid"))?,
        before_ref: None,
        after_ref: Some(
            RecordReference::new(format!("retention-config:{}", ctx.config_source))
                .map_err(|_| maintenance_rejected("the config-source reference is invalid"))?,
        ),
        decision: Some(
            AuditDecision::new(
                DecisionOutcome::Allowed,
                format!("retention-window:{bounds_note}"),
            )
            .map_err(|_| maintenance_rejected("the maintenance decision is invalid"))?,
        ),
        evidence: Some(
            RecordReference::new(bounds_note)
                .map_err(|_| maintenance_rejected("the bounds evidence reference is invalid"))?,
        ),
        event_time,
        ingest_time,
        correlation_id: ctx.correlation_id,
    })
    .map_err(|_| maintenance_rejected("the partition-maintenance audit record is invalid"))
}

// --- The unit-of-work operations ------------------------------------------------

impl PgUnitOfWork {
    /// Lists the telemetry partitions from the catalog with their parsed
    /// bounds (D-R6). Unparseable bounds are reported as `None` (fail-safe,
    /// D-R3) — never guessed.
    pub fn list_telemetry_partitions(&self) -> Result<Vec<PartitionEntry>, MaintenanceError> {
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(maintenance_backend)?;
            let rows = sqlx::query(
                "SELECT c.relname AS name, pg_get_expr(c.relpartbound, c.oid) AS bound \
                 FROM pg_inherits i JOIN pg_class c ON c.oid = i.inhrelid \
                 WHERE i.inhparent = 'telemetry'::regclass \
                 ORDER BY c.relname",
            )
            .fetch_all(&mut *tx)
            .await
            .map_err(map_backend)
            .map_err(maintenance_backend)?;
            let mut entries = Vec::with_capacity(rows.len());
            for row in &rows {
                let name: String = row
                    .try_get("name")
                    .map_err(|_| crate::error::decode_failure("pg_class.relname"))?;
                let bound: String = row
                    .try_get("bound")
                    .map_err(|_| crate::error::decode_failure("pg_class.relpartbound"))?;
                let bounds = parse_partition_bounds(&bound).map(|(from_utc, until_utc)| {
                    PartitionBounds {
                        from_utc,
                        until_utc,
                    }
                });
                entries.push(PartitionEntry { name, bounds });
            }
            Ok(entries)
        })
    }

    /// Creates the monthly partition for (year, month) if it does not exist
    /// (D-R1; idempotent by name check and `IF NOT EXISTS`). The bounds are
    /// computed from the calendar arguments — never caller text; the name is
    /// the D-P2 convention. Runs inside THIS unit of work's transaction (the
    /// caller commits; the worker loop's pass is all-or-nothing and retried
    /// idempotently on the next pass).
    pub fn create_telemetry_partition(
        &self,
        year: i32,
        month: u32,
    ) -> Result<PartitionCreateOutcome, MaintenanceError> {
        if month_bounds(year, month).is_none() {
            return Err(maintenance_rejected(
                "a partition month must be a calendar month (1..=12) within the representable calendar",
            ));
        }
        let name = partition_name(year, month);
        if !is_telemetry_partition_name(&name) {
            return Err(maintenance_rejected(
                "the computed partition name does not match the D-P2 convention",
            ));
        }
        let existing = self.list_telemetry_partitions()?;
        if existing.iter().any(|entry| entry.name == name) {
            return Ok(PartitionCreateOutcome::AlreadyExists(name));
        }
        // The DDL interpolates only calendar-computed constants.
        let from_text = format!(
            "{:04}-{:02}-01 00:00:00+00",
            year,
            month
        );
        let (next_year, next_month) = month_add(year, month, 1);
        let until_text = format!("{:04}-{:02}-01 00:00:00+00", next_year, next_month);
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(maintenance_backend)?;
            let ddl = format!(
                "CREATE TABLE IF NOT EXISTS {name} PARTITION OF telemetry \
                 FOR VALUES FROM ('{from_text}') TO ('{until_text}')"
            );
            sqlx::query(&ddl)
                .execute(&mut *tx)
                .await
                .map_err(map_backend)
                .map_err(maintenance_backend)?;
            Ok(PartitionCreateOutcome::Created(name))
        })
    }

    /// Detaches one monthly partition with its audit record in the SAME
    /// transaction (D-R2/D-R5). Refuses (before any DDL) the current or
    /// in-flight partition, any partition with a future upper bound, and
    /// any partition whose bounds could not be determined (D-R3). The
    /// window eligibility itself is the caller's plan
    /// ([`retention_eligible`]); the refusals here are the explicit in-code
    /// enforcement of the safety rule.
    pub fn detach_telemetry_partition(
        &self,
        name: &str,
        now: OffsetDateTime,
        audit: &PartitionAuditContext,
    ) -> Result<PartitionBounds, MaintenanceError> {
        if !is_telemetry_partition_name(name) {
            return Err(maintenance_rejected(
                "the partition name does not match the D-P2 telemetry convention",
            ));
        }
        let entries = self.list_telemetry_partitions()?;
        let entry = entries
            .iter()
            .find(|entry| entry.name == name)
            .ok_or_else(|| MaintenanceError::UnknownPartition {
                partition: name.to_string(),
            })?;
        let bounds = entry.bounds.clone().ok_or_else(|| MaintenanceError::Refused {
            partition: name.to_string(),
            reason: "the partition bounds could not be determined from the catalog; retention \
                     never acts on unknown bounds (fail-safe, D-R3)"
                .to_string(),
        })?;
        if bounds.contains(now) {
            return Err(MaintenanceError::Refused {
                partition: name.to_string(),
                reason: "the partition contains the current instant (current/in-flight \
                         partition); retention never touches it (D-R3)"
                    .to_string(),
            });
        }
        if bounds.until_utc > now {
            return Err(MaintenanceError::Refused {
                partition: name.to_string(),
                reason: "the partition's upper bound is in the future (pre-created ahead of the \
                         ingest frontier); retention never touches it (D-R3)"
                    .to_string(),
            });
        }
        let record = build_audit_record(
            audit,
            "telemetry.partition.detach",
            name,
            format!("bounds:[{},{})", bounds.from_utc, bounds.until_utc),
            now,
        )?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(maintenance_backend)?;
            // Regular DETACH (transactional) — see D-R2 for the recorded
            // CONCURRENTLY trade-off.
            sqlx::query(&format!("ALTER TABLE telemetry DETACH PARTITION {name}"))
                .execute(&mut *tx)
                .await
                .map_err(map_backend)
                .map_err(maintenance_backend)?;
            crate::audit::insert_audit_record(&mut *tx, &record)
                .await
                .map_err(|e| {
                    MaintenanceError::Backend(BackendError::new(
                        codes::REJECTED.to_string(),
                        e.to_string(),
                    ))
                })?;
            Ok(bounds)
        })
    }

    /// Drops one ALREADY-DETACHED partition with its audit record in the
    /// SAME transaction — the second approved step of D-R2. Refuses any
    /// partition still attached to `telemetry` (detach first is structurally
    /// enforced) and any name outside the D-P2 convention. A detached
    /// partition no longer carries catalog bounds; the retention-window
    /// eligibility was enforced by the detach that produced it (D-R3), so
    /// this step requires no bounds — only the attach-state check and the
    /// audit record.
    pub fn drop_detached_telemetry_partition(
        &self,
        name: &str,
        now: OffsetDateTime,
        audit: &PartitionAuditContext,
    ) -> Result<(), MaintenanceError> {
        if !is_telemetry_partition_name(name) {
            return Err(maintenance_rejected(
                "the partition name does not match the D-P2 telemetry convention",
            ));
        }
        let record = build_audit_record(
            audit,
            "telemetry.partition.drop",
            name,
            "detached-partition".to_string(),
            now,
        )?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(maintenance_backend)?;
            let attached: Option<String> = sqlx::query_scalar(
                "SELECT c.relname FROM pg_inherits i JOIN pg_class c ON c.oid = i.inhrelid \
                 WHERE i.inhparent = 'telemetry'::regclass AND c.relname = $1",
            )
            .bind(name)
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_backend)
            .map_err(maintenance_backend)?;
            if attached.is_some() {
                return Err(MaintenanceError::Refused {
                    partition: name.to_string(),
                    reason: "the partition is still attached to telemetry; detach first — drop \
                             is the second approved step (D-R2)"
                        .to_string(),
                });
            }
            let exists: bool =
                sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
                    .bind(format!("public.{name}"))
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(map_backend)
                    .map_err(maintenance_backend)?;
            if !exists {
                return Err(MaintenanceError::UnknownPartition {
                    partition: name.to_string(),
                });
            }
            sqlx::query(&format!("DROP TABLE {name}"))
                .execute(&mut *tx)
                .await
                .map_err(map_backend)
                .map_err(maintenance_backend)?;
            crate::audit::insert_audit_record(&mut *tx, &record)
                .await
                .map_err(|e| {
                    MaintenanceError::Backend(BackendError::new(
                        codes::REJECTED.to_string(),
                        e.to_string(),
                    ))
                })?;
            Ok(())
        })
    }
}

/// The outcome of a pre-creation create (idempotent by name, D-R1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartitionCreateOutcome {
    /// The partition was created (name).
    Created(String),
    /// A partition with the same D-P2 name already existed (no DDL ran).
    AlreadyExists(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Pure month/name/bounds/window tests (no database) ------------------

    #[test]
    fn month_arithmetic_wraps_years_both_directions() {
        assert_eq!(month_add(2030, 1, -1), (2029, 12));
        assert_eq!(month_add(2030, 12, 1), (2031, 1));
        assert_eq!(month_add(2030, 6, 0), (2030, 6));
        assert_eq!(month_add(1999, 1, -13), (1997, 12));
        assert_eq!(month_from_index(month_index(2030, 7)), (2030, 7));
        assert_eq!(month_from_index(month_index(1, 1)), (1, 1));
    }

    #[test]
    fn month_bounds_are_first_to_first_and_reject_bad_months() {
        let (from, until) = month_bounds(2030, 1).expect("january");
        assert_eq!(from.year(), 2030);
        assert_eq!(u8::from(from.month()), 1);
        assert_eq!(from.day(), 1);
        assert_eq!(until.year(), 2030);
        assert_eq!(u8::from(until.month()), 2);
        // January has 31 days: [Jan 1, Feb 1) is exactly 31 days.
        assert_eq!((until - from).whole_days(), 31);
        let (feb_from, feb_until) = month_bounds(2030, 2).expect("february");
        assert_eq!((feb_until - feb_from).whole_days(), 28);
        assert_eq!(month_bounds(2030, 13), None);
        assert_eq!(month_bounds(2030, 0), None);
    }

    #[test]
    fn partition_names_follow_the_d_p2_convention() {
        assert_eq!(partition_name(2030, 1), "telemetry_y2030m01");
        assert_eq!(partition_name(2030, 12), "telemetry_y2030m12");
        assert!(is_telemetry_partition_name("telemetry_y2030m01"));
        assert!(is_telemetry_partition_name("telemetry_y2030m12"));
        // Injection-shaped or off-convention names are rejected before any
        // DDL interpolation.
        assert!(!is_telemetry_partition_name("telemetry_y2030m1"));
        assert!(!is_telemetry_partition_name("telemetry_y2030m013"));
        assert!(!is_telemetry_partition_name("telemetry_y203xm01"));
        assert!(!is_telemetry_partition_name("users; DROP TABLE tenants"));
        assert!(!is_telemetry_partition_name(""));
    }

    #[test]
    fn partition_bound_parsing_is_strict_and_fail_safe() {
        // The canonical relpartbound rendering (recorded assumption, D-R6).
        let (from, until) = parse_partition_bounds(
            "FOR VALUES FROM ('2030-01-01 00:00:00+00') TO ('2030-02-01 00:00:00+00')",
        )
        .expect("canonical rendering parses");
        assert_eq!(from.year(), 2030);
        assert_eq!(until.year(), 2030);
        assert_eq!(u8::from(until.month()), 2);
        // Whole-hour offsets without minutes normalize; sub-hour offsets
        // pass through.
        assert!(parse_partition_bounds(
            "FOR VALUES FROM ('2030-01-01 00:00:00+00') TO ('2030-02-01 00:00:00-05')"
        )
        .is_some());
        // Any other shape is None: the caller treats the partition as
        // unknown-bounds and never acts on it (fail-safe).
        assert_eq!(parse_partition_bounds("FOR VALUES IN ('a')"), None);
        assert_eq!(parse_partition_bounds(""), None);
        assert_eq!(
            parse_partition_bounds(
                "FOR VALUES FROM ('not-a-timestamp') TO ('2030-02-01 00:00:00+00')"
            ),
            None
        );
    }

    #[test]
    fn pre_creation_plan_covers_only_missing_months() {
        let existing = vec![PartitionEntry {
            name: "telemetry_y2030m01".to_string(),
            bounds: Some(PartitionBounds {
                from_utc: month_bounds(2030, 1).expect("bounds").0,
                until_utc: month_bounds(2030, 1).expect("bounds").1,
            }),
        }];
        let plan = plan_pre_creation(&existing, 2030, 1, 3);
        // January exists; February and March are planned.
        let names: Vec<&str> = plan.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["telemetry_y2030m02", "telemetry_y2030m03"]);
        let feb = &plan[0];
        assert_eq!((feb.year, feb.month), (2030, 2));
        assert_eq!(
            feb.from_utc,
            month_bounds(2030, 2).expect("bounds").0
        );
        // Nothing to do when everything exists.
        assert!(plan_pre_creation(&existing, 2030, 3, 0).is_empty());
    }

    #[test]
    fn retention_cutoff_and_eligibility_match_the_d_r3_rule() {
        // now = mid-April 2030; retention = 2 months → cutoff = 2030-02-01.
        let now = month_bounds(2030, 4).expect("bounds").0 + time::Duration::days(14);
        let cutoff = retention_cutoff(2030, 4, 2).expect("cutoff");
        assert_eq!(cutoff, month_bounds(2030, 2).expect("bounds").0);

        let jan = PartitionEntry {
            name: "telemetry_y2030m01".to_string(),
            bounds: Some(PartitionBounds {
                from_utc: month_bounds(2030, 1).expect("bounds").0,
                until_utc: month_bounds(2030, 1).expect("bounds").1,
            }),
        };
        let feb = PartitionEntry {
            name: "telemetry_y2030m02".to_string(),
            bounds: Some(PartitionBounds {
                from_utc: month_bounds(2030, 2).expect("bounds").0,
                until_utc: month_bounds(2030, 2).expect("bounds").1,
            }),
        };
        let mar = PartitionEntry {
            name: "telemetry_y2030m03".to_string(),
            bounds: Some(PartitionBounds {
                from_utc: month_bounds(2030, 3).expect("bounds").0,
                until_utc: month_bounds(2030, 3).expect("bounds").1,
            }),
        };
        let apr = PartitionEntry {
            name: "telemetry_y2030m04".to_string(),
            bounds: Some(PartitionBounds {
                from_utc: month_bounds(2030, 4).expect("bounds").0,
                until_utc: month_bounds(2030, 4).expect("bounds").1,
            }),
        };
        // February's upper bound is 2030-03-01 > cutoff (2030-02-01): only
        // January is entirely outside the window.
        let eligible = retention_eligible(&[jan.clone(), feb.clone(), mar.clone(), apr.clone()], cutoff, now);
        let names: Vec<&str> = eligible.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["telemetry_y2030m01"]);
        // The current month's partition is never eligible.
        assert!(retention_eligible(&[apr.clone()], cutoff, now).is_empty());
        // Unknown bounds are never eligible (fail-safe).
        let unknown = PartitionEntry {
            name: "telemetry_y2029m12".to_string(),
            bounds: None,
        };
        assert!(retention_eligible(&[unknown], cutoff, now).is_empty());
        // A zero-month retention keeps everything from this month on.
        let zero_cutoff = retention_cutoff(2030, 4, 0).expect("cutoff");
        assert_eq!(zero_cutoff, month_bounds(2030, 4).expect("bounds").0);
        assert!(retention_eligible(&[jan, feb, mar, apr], zero_cutoff, now).is_empty());
    }

    #[test]
    fn maintenance_error_display_is_human_safe() {
        let errors = [
            MaintenanceError::UnknownPartition {
                partition: "telemetry_y2030m01".to_string(),
            },
            MaintenanceError::Refused {
                partition: "telemetry_y2030m04".to_string(),
                reason: "current/in-flight partition".to_string(),
            },
            MaintenanceError::Backend(BackendError::new(
                codes::UNAVAILABLE.to_string(),
                "the storage backend is unavailable".to_string(),
            )),
        ];
        for error in &errors {
            let rendered = error.to_string();
            assert!(!rendered.contains("postgres://"));
            assert!(!rendered.contains("SQLSTATE"));
        }
    }
}
