// P2.20 — Integration tests for the partition-maintenance path (D-R1..D-R6):
// pre-creation creates partitions with the expected names and bounds, the
// D-R3 safety refusals (current/in-flight partition, future bounds), detach
// of out-of-window partitions, the second-step drop of DETACHED partitions,
// and one audit record per action written atomically with the DDL.
//
// Gating is the recorded harness decision (CONTEXT.md open question 5):
// every test skips cleanly when FLEXCORE_TEST_DATABASE_URL is unset.
//
// Test months are FIXED constants (2027–2029) chosen to NOT overlap the
// months the P2.06 telemetry tests create (2030 m01..m04, 2020 m01): the
// maintenance DDL must never disturb the other suites' partitions, and
// nothing here detaches a partition another test created.
//
// The maintenance store connects as `flexcore_migrator` (the table owner —
// the recorded P2.20 maintenance role, D-R1). Writing the platform-level
// audit record (tenant_id NULL) under FORCE RLS requires the owner/ops path
// (P1.12 §5.2): the test bootstrap grants BYPASSRLS to the test maintenance
// role — the production analogue is the recorded ops path; CI's
// `flexcore` bootstrap superuser grants it.

use std::sync::Arc;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use flexcore_postgres_adapter::partition_maintenance::{
    month_bounds, parse_partition_bounds, partition_name, plan_pre_creation,
    retention_cutoff, retention_eligible, MaintenanceError, PartitionAuditContext,
    PartitionCreateOutcome,
};
use flexcore_postgres_adapter::{FlexCoreStore, PgUnitOfWork};
use sqlx::PgPool;

// --- Fixed test months (2027–2029, UTC) ------------------------------------------

/// 2027-01-01T00:00:00Z and 2028-04-10T12:00:00Z (Unix seconds).
const JAN_2027: i64 = 1_799_865_600;
/// The "now" of the retention scenarios: mid-April 2028 — every detach in
/// this file uses this CONSTANT, never the wall clock.
const APR_2028: i64 = 1_836_724_800;

fn now_of_test() -> time::OffsetDateTime {
    time::OffsetDateTime::from_unix_timestamp(APR_2028).expect("constant timestamp")
}

// --- Harness helpers (the recorded P2.06 pattern) ---------------------------------

fn test_url() -> Option<String> {
    std::env::var("FLEXCORE_TEST_DATABASE_URL")
        .ok()
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty())
}

fn require_db(test: &str) -> String {
    match test_url() {
        Some(url) => url,
        None => {
            println!(
                "skipped {test}: FLEXCORE_TEST_DATABASE_URL is not set (CI provides a PostgreSQL 18 service)"
            );
            String::new()
        }
    }
}

fn skipped(url: &str) -> bool {
    url.is_empty()
}

fn run_key() -> &'static u64 {
    static RUN_KEY: OnceLock<u64> = OnceLock::new();
    RUN_KEY.get_or_init(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("the system clock is after the Unix epoch")
            .as_nanos() as u64
    })
}

fn test_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
}

fn url_as(url: &str, user: &str, password: &str) -> String {
    if let Some((scheme, rest)) = url.split_once("://") {
        if let Some((_, host_and_path)) = rest.split_once('@') {
            return format!("{scheme}://{user}:{password}@{host_and_path}");
        }
        return format!("{scheme}://{user}:{password}@{rest}");
    }
    format!("{url}?user={user}&password={password}")
}

/// Grants LOGIN, a test-only password, and BYPASSRLS to the maintenance
/// role. BYPASSRLS is the recorded owner/ops-path analogue (P1.12 §5.2):
/// the platform-level retention audit record (`tenant_id` NULL) is writable
/// only through a session that bypasses the tenant_isolation policy. The
/// grant requires the bootstrap superuser (CI's `flexcore` user).
fn ensure_test_roles(pool: &PgPool) {
    let result = test_runtime().block_on(async {
        sqlx::query(&format!(
            "ALTER ROLE flexcore_migrator WITH LOGIN PASSWORD 'flexcore_migrator_test_only' \
             BYPASSRLS"
        ))
        .execute(pool)
        .await?;
        Ok::<(), sqlx::Error>(())
    });
    result.expect("the maintenance role must be grantable from the admin connection");
}

/// The maintenance store: the migrator/maintenance role (table owner).
fn maintenance_store(url: &str) -> FlexCoreStore {
    FlexCoreStore::connect(&url_as(
        url,
        "flexcore_migrator",
        "flexcore_migrator_test_only",
    ))
    .expect("maintenance store")
}

fn audit_context(run_tag: u8) -> PartitionAuditContext {
    let mut bytes = [0u8; 16];
    bytes[0] = run_tag;
    bytes[1] = 0x6B; // P2.20 fixture marker
    let key = *run_key();
    bytes[2..10].copy_from_slice(&key.to_be_bytes());
    let mut correlation = bytes;
    correlation[0] ^= 0xFF;
    PartitionAuditContext {
        audit_id: flexcore_contracts::ids::FlexUuid::from_bytes(bytes),
        correlation_id: flexcore_contracts::ids::FlexUuid::from_bytes(correlation),
        actor: "integration-test:partition-maintenance".to_string(),
        config_source: "FLEXCORE_RETENTION_NORMALIZED_TELEMETRY_MONTHS".to_string(),
    }
}

/// `to_regclass` existence check via the admin (bootstrap) connection.
fn relation_exists(pool: &PgPool, name: &str) -> bool {
    test_runtime().block_on(async {
        sqlx::query_scalar::<_, bool>("SELECT to_regclass($1) IS NOT NULL")
            .bind(format!("public.{name}"))
            .fetch_one(pool)
            .await
            .expect("to_regclass query")
    })
}

/// Platform-level audit records are readable through the bootstrap
/// superuser (FORCE RLS keeps every other session out — that IS the P1.12
/// §5.2 recorded path).
fn audit_count_for(pool: &PgPool, action: &str, object_id: &str) -> i64 {
    test_runtime().block_on(async {
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM audit_records WHERE action = $1 AND object_id = $2",
        )
        .bind(action)
        .bind(object_id)
        .fetch_one(pool)
        .await
        .expect("audit count query")
    })
}

/// Creates the given (year, month) partitions idempotently through the
/// maintenance path and commits.
fn ensure_partitions(store: &FlexCoreStore, months: &[(i32, u32)]) {
    let unit = store.begin(None).expect("maintenance unit of work");
    let shared = Arc::new(unit);
    for (year, month) in months {
        match PgUnitOfWork::create_telemetry_partition(&shared, *year, *month)
            .expect("partition create")
        {
            PartitionCreateOutcome::Created(_) | PartitionCreateOutcome::AlreadyExists(_) => {}
        }
    }
    match Arc::try_unwrap(shared) {
        Ok(unit) => {
            unit.commit().expect("commit partition creation");
        }
        Err(_) => panic!("the shared unit of work was still held"),
    }
}

fn attached_names(unit: &PgUnitOfWork) -> Vec<String> {
    unit.list_telemetry_partitions()
        .expect("partition listing")
        .into_iter()
        .map(|entry| entry.name)
        .collect()
}

// --- 1. Pre-creation (D-R1) ----------------------------------------------------------

#[test]
fn pre_creation_creates_partitions_with_expected_names_and_bounds() {
    let url = require_db("pre_creation_creates_partitions_with_expected_names_and_bounds");
    if skipped(&url) {
        return;
    }
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    let store = maintenance_store(&url);

    let unit = store.begin(None).expect("unit of work");
    let shared = Arc::new(unit);
    // Fixed frontier 2029-01, three months ahead: the planner names exactly
    // the missing months (already-created months are skipped).
    let existing = PgUnitOfWork::list_telemetry_partitions(&shared).expect("listing");
    let plan = plan_pre_creation(&existing, 2029, 1, 3);
    let mut created = Vec::new();
    for planned in &plan {
        assert_eq!((planned.year, planned.month), expected_of(&planned.name));
        match PgUnitOfWork::create_telemetry_partition(&shared, planned.year, planned.month)
            .expect("create")
        {
            PartitionCreateOutcome::Created(name) => created.push(name),
            PartitionCreateOutcome::AlreadyExists(name) => created.push(name),
        }
    }
    PgUnitOfWork::commit(Arc::try_unwrap(shared).expect("unique")).expect("commit");

    // Every planned month now exists with the D-P2 bounds.
    let unit = store.begin(None).expect("unit of work");
    let listed = unit.list_telemetry_partitions().expect("listing");
    for planned in &plan {
        assert!(
            created.contains(&planned.name),
            "the planner's month must be created: {planned:?}"
        );
        let entry = listed
            .iter()
            .find(|entry| entry.name == planned.name)
            .unwrap_or_else(|| panic!("{} must be attached", planned.name));
        let bounds = entry
            .bounds
            .as_ref()
            .unwrap_or_else(|| panic!("{} must have parseable bounds", planned.name));
        let (from, until) = month_bounds(planned.year, planned.month).expect("calendar bounds");
        assert_eq!(bounds.from_utc, from, "lower bound is the first of month");
        assert_eq!(bounds.until_utc, until, "upper bound is exclusive first of next month");
    }
    unit.commit().expect("commit listing unit");

    // Idempotency: creating an existing month again reports AlreadyExists
    // and runs no conflicting DDL.
    let unit = store.begin(None).expect("unit of work");
    match unit.create_telemetry_partition(2029, 1).expect("create again") {
        PartitionCreateOutcome::AlreadyExists(name) => {
            assert_eq!(name, "telemetry_y2029m01");
        }
        PartitionCreateOutcome::Created(name) => {
            panic!("an existing partition must not be re-created: {name}")
        }
    }
    unit.commit().expect("commit");
}

/// The D-P2 name convention encodes the month — decode it back (the
/// planner's (year, month) must agree with its own name).
fn expected_of(name: &str) -> (i32, u32) {
    assert!(name.starts_with("telemetry_y"));
    let rest = &name["telemetry_y".len()..];
    let year: i32 = rest[..4].parse().expect("year digits");
    let month: u32 = rest[5..7].parse().expect("month digits");
    (year, month)
}

// --- 2. Retention eligibility, refusals, detach, second-step drop (D-R2/D-R3) --------

#[test]
fn retention_detaches_only_out_of_window_partitions_and_refuses_the_rest() {
    let url = require_db("retention_detaches_only_out_of_window_partitions_and_refuses_the_rest");
    if skipped(&url) {
        return;
    }
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    let store = maintenance_store(&url);

    // January–April 2028 (April is the "current" month for the constant
    // now; May is pre-created ahead of the frontier).
    ensure_partitions(&store, &[(2028, 1), (2028, 2), (2028, 3), (2028, 4), (2028, 5)]);
    let now = now_of_test();
    let cutoff = retention_cutoff(now.year(), u8::from(now.month()), 2)
        .expect("cutoff for a two-month horizon");
    // Cutoff = first day of (April − 2) = 2028-02-01: only January lies
    // entirely outside the window.
    assert_eq!(
        parse_partition_bounds(&format!(
            "FOR VALUES FROM ('2028-02-01 00:00:00+00') TO ('2028-03-01 00:00:00+00')"
        ))
        .expect("sanity parse"),
        (cutoff, month_bounds(2028, 3).expect("march").0)
    );

    let unit = store.begin(None).expect("unit of work");
    let shared = Arc::new(unit);
    let entries = PgUnitOfWork::list_telemetry_partitions(&shared).expect("listing");
    let eligible = retention_eligible(&entries, cutoff, now);
    let eligible_names: Vec<&str> = eligible.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(
        eligible_names,
        vec!["telemetry_y2028m01"],
        "exactly the out-of-window partition is eligible"
    );

    // D-R3 refusals BEFORE any DDL: the current/in-flight partition and a
    // pre-created future partition are refused with the explicit error.
    for refused_name in ["telemetry_y2028m04", "telemetry_y2028m05"] {
        match PgUnitOfWork::detach_telemetry_partition(
            &shared,
            refused_name,
            now,
            &audit_context(0x21),
        ) {
            Err(MaintenanceError::Refused { partition, reason }) => {
                assert_eq!(partition, refused_name);
                assert!(!reason.is_empty());
            }
            other => panic!("detach of {refused_name} must be refused, got {other:?}"),
        }
    }
    // Unknown names are UnknownPartition, not a silent success.
    match PgUnitOfWork::detach_telemetry_partition(
        &shared,
        "telemetry_y2028m06",
        now,
        &audit_context(0x22),
    ) {
        Err(MaintenanceError::UnknownPartition { partition }) => {
            assert_eq!(partition, "telemetry_y2028m06");
        }
        other => panic!("detach of an unknown partition must be UnknownPartition, got {other:?}"),
    }

    // The eligible detach: DDL + audit record in the SAME transaction.
    let detached_bounds = PgUnitOfWork::detach_telemetry_partition(
        &shared,
        "telemetry_y2028m01",
        now,
        &audit_context(0x23),
    )
    .expect("detach of the out-of-window partition");
    assert_eq!(
        detached_bounds.from_utc,
        month_bounds(2028, 1).expect("january").0
    );
    PgUnitOfWork::commit(Arc::try_unwrap(shared).expect("unique")).expect("commit");

    // January is no longer attached (but still exists as a table); the
    // in-window months remain attached.
    let unit = store.begin(None).expect("unit of work");
    let names = attached_names(&unit);
    assert!(!names.contains(&"telemetry_y2028m01".to_string()));
    for kept in ["telemetry_y2028m02", "telemetry_y2028m03", "telemetry_y2028m04"] {
        assert!(names.contains(&kept.to_string()), "{kept} must stay attached");
    }
    unit.commit().expect("commit");
    let admin_pool = admin_store.pool();
    assert!(
        relation_exists(admin_pool, "telemetry_y2028m01"),
        "a detached partition is dropped only by the second approved step"
    );

    // The second approved step: a STILL-ATTACHED partition is refused; the
    // DETACHED one is dropped (with its audit record).
    let unit = store.begin(None).expect("unit of work");
    match unit.drop_detached_telemetry_partition("telemetry_y2028m02", now, &audit_context(0x24)) {
        Err(MaintenanceError::Refused { partition, .. }) => {
            assert_eq!(partition, "telemetry_y2028m02");
        }
        other => panic!("dropping an attached partition must be refused, got {other:?}"),
    }
    unit.rollback().expect("rollback");
    let unit = store.begin(None).expect("unit of work");
    unit.drop_detached_telemetry_partition("telemetry_y2028m01", now, &audit_context(0x25))
        .expect("drop of the detached partition");
    unit.commit().expect("commit");
    assert!(
        !relation_exists(admin_pool, "telemetry_y2028m01"),
        "the dropped partition must be gone"
    );
    // Dropping it again is UnknownPartition (it is gone), never a success.
    let unit = store.begin(None).expect("unit of work");
    match unit.drop_detached_telemetry_partition("telemetry_y2028m01", now, &audit_context(0x26)) {
        Err(MaintenanceError::UnknownPartition { partition }) => {
            assert_eq!(partition, "telemetry_y2028m01");
        }
        other => panic!("re-drop must be UnknownPartition, got {other:?}"),
    }
    unit.rollback().expect("rollback");
}

// --- 3. Audit records per action (D-R5) ----------------------------------------------

#[test]
fn every_retention_action_has_an_audit_record() {
    let url = require_db("every_retention_action_has_an_audit_record");
    if skipped(&url) {
        return;
    }
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    let store = maintenance_store(&url);

    // January 2027: one detach action.
    ensure_partitions(&store, &[(2027, 1)]);
    let now = now_of_test();
    let unit = store.begin(None).expect("unit of work");
    let before_detach = audit_count_for(admin_store.pool(), "telemetry.partition.detach", "telemetry_y2027m01");
    unit.detach_telemetry_partition("telemetry_y2027m01", now, &audit_context(0x31))
        .expect("detach");
    unit.commit().expect("commit");

    let after_detach =
        audit_count_for(admin_store.pool(), "telemetry.partition.detach", "telemetry_y2027m01");
    assert_eq!(
        after_detach,
        before_detach + 1,
        "exactly one audit record per detach action"
    );

    // The refused detach of the CURRENT month relative to the constant
    // `now` (April 2028) writes NO audit record (the refusal happens
    // before any DDL — nothing happened to audit). y2028m04 is only ever
    // REFUSED in this file (test 2 refuses it too), so the parallel suites
    // never race on it.
    ensure_partitions(&store, &[(2028, 4)]);
    let unit = store.begin(None).expect("unit of work");
    match unit.detach_telemetry_partition("telemetry_y2028m04", now, &audit_context(0x32)) {
        Err(MaintenanceError::Refused { .. }) => {}
        other => panic!("current-month detach must be refused, got {other:?}"),
    }
    unit.rollback().expect("rollback");
    assert_eq!(
        audit_count_for(admin_store.pool(), "telemetry.partition.detach", "telemetry_y2028m04"),
        0,
        "a refused action is not audited as executed"
    );

    // The drop of the detached January partition has its own record.
    let before_drop = audit_count_for(admin_store.pool(), "telemetry.partition.drop", "telemetry_y2027m01");
    let unit = store.begin(None).expect("unit of work");
    unit.drop_detached_telemetry_partition("telemetry_y2027m01", now, &audit_context(0x33))
        .expect("drop");
    unit.commit().expect("commit");
    assert_eq!(
        audit_count_for(admin_store.pool(), "telemetry.partition.drop", "telemetry_y2027m01"),
        before_drop + 1,
        "exactly one audit record per drop action"
    );
    let _ = JAN_2027;
}

// --- 4. Pure-name sanity against the live catalog -------------------------------------

#[test]
fn catalog_names_match_the_d_p2_convention_and_bounds_parse() {
    let url = require_db("catalog_names_match_the_d_p2_convention_and_bounds_parse");
    if skipped(&url) {
        return;
    }
    let admin_store = FlexCoreStore::connect(&url).expect("admin store");
    admin_store.run_migrations().expect("migrations");
    ensure_test_roles(admin_store.pool());
    let store = maintenance_store(&url);

    ensure_partitions(&store, &[(2027, 7)]);
    let unit = store.begin(None).expect("unit of work");
    let entries = unit.list_telemetry_partitions().expect("listing");
    unit.commit().expect("commit");
    let entry = entries
        .iter()
        .find(|entry| entry.name == "telemetry_y2027m07")
        .expect("the created partition is listed");
    // The catalog rendering parses back to the calendar bounds (the D-R6
    // recorded assumption, verified here on the live catalog output).
    let (from, until) = month_bounds(2027, 7).expect("bounds");
    assert_eq!(
        entry.bounds.as_ref().expect("parseable bounds").from_utc,
        from
    );
    assert_eq!(
        entry.bounds.as_ref().expect("parseable bounds").until_utc,
        until
    );
    assert!(partition_name(2027, 7) == "telemetry_y2027m07");
}
